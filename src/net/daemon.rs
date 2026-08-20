//! The root daemon.
//!
//! It runs permanently under launchd but is inert: no redirect, no listener,
//! nothing intercepted, until a session registers. It then supervises the same
//! proxy process `smuggle proxy` runs, so all the redirect and teardown
//! behaviour is shared. When the last session disconnects the proxy is stopped
//! and the machine is back to normal.
//!
//! Interception therefore still lasts exactly as long as a session, and no
//! session ever needs sudo.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};

use super::control::{DaemonStatus, Event, Reply, Request, socket_path};

/// One connected session.
struct Session {
    packages: Vec<String>,
    registries: Vec<String>,
    verbose: bool,
    events: Sender<Reply>,
}

#[derive(Default)]
struct State {
    sessions: HashMap<u64, Session>,
    proxy: Option<Child>,
    next_id: u64,
}

type Shared = Arc<Mutex<State>>;

/// Run the daemon. Must be root, and is normally started by launchd.
pub fn run(owner_uid: u32) -> Result<(), String> {
    if super::control::is_default_socket() && unsafe { libc::geteuid() } != 0 {
        return Err("the daemon must run as root".into());
    }

    let path = socket_path();
    // A stale socket from an unclean shutdown would block the bind.
    let _ = std::fs::remove_file(&path);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }

    let listener =
        UnixListener::bind(&path).map_err(|e| format!("could not bind {}: {e}", path.display()))?;
    restrict_socket(&path, owner_uid)?;

    println!("smuggle daemon listening on {}", path.display());

    let state: Shared = Arc::new(Mutex::new(State::default()));

    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let state = state.clone();
        std::thread::spawn(move || serve_session(stream, state, owner_uid));
    }

    Ok(())
}

/// Only the user who ran setup may drive the daemon. The socket is the door to
/// a root process that can rewrite what every install on this machine
/// receives, so it is not world-writable.
fn restrict_socket(path: &Path, owner_uid: u32) -> Result<(), String> {
    use std::os::unix::ffi::OsStrExt;

    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|e| format!("bad socket path: {e}"))?;

    if unsafe { libc::chown(c_path.as_ptr(), owner_uid, libc::gid_t::MAX) } != 0 {
        return Err(format!(
            "could not set socket ownership: {}",
            std::io::Error::last_os_error()
        ));
    }
    if unsafe { libc::chmod(c_path.as_ptr(), 0o600) } != 0 {
        return Err(format!(
            "could not set socket permissions: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

fn serve_session(stream: UnixStream, state: Shared, owner_uid: u32) {
    let Ok(write_half) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(stream);

    let mut line = String::new();
    if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
        return;
    }

    let request: Request = match serde_json::from_str(line.trim()) {
        Ok(r) => r,
        Err(e) => {
            let _ = send(
                &write_half,
                &Reply::Error {
                    message: format!("bad request: {e}"),
                },
            );
            return;
        }
    };

    if request.version() != super::control::version() {
        let _ = send(
            &write_half,
            &Reply::Error {
                message: format!(
                    "daemon is running {} but you are on {}. Run `smuggle setup` to restage it.",
                    super::control::version(),
                    request.version(),
                ),
            },
        );
        return;
    }

    let (packages, registries, verbose) = match request {
        Request::Register {
            packages,
            registries,
            verbose,
            ..
        } => (packages, registries, verbose),
        // A status request is answered and closed; it registers nothing, so it
        // never affects what is intercepted.
        Request::Status { .. } => {
            let status = snapshot(&state);
            let _ = send(&write_half, &Reply::Status { status });
            return;
        }
    };

    // Everything the daemon sends after the first reply goes through this
    // channel, so exactly one thread ever writes to the socket. Two writers
    // would interleave and corrupt the stream.
    let (events, incoming) = channel::<Reply>();
    let acks = events.clone();
    let id = {
        let mut state = state.lock().unwrap();
        let id = state.next_id;
        state.next_id += 1;
        state.sessions.insert(
            id,
            Session {
                packages,
                registries,
                verbose,
                events,
            },
        );
        id
    };

    if let Err(e) = resync(&state, owner_uid) {
        let _ = send(&write_half, &Reply::Error { message: e });
        drop_session(&state, id, owner_uid);
        return;
    }

    if send(&write_half, &Reply::Ok).is_err() {
        drop_session(&state, id, owner_uid);
        return;
    }

    // Forward proxy output to this session for as long as it is connected.
    let forwarder = std::thread::spawn(move || {
        for reply in incoming {
            if send(&write_half, &reply).is_err() {
                break;
            }
        }
    });

    // The session holds the connection open. Further lines re-register it,
    // which is how a session changes what it hijacks without dropping the
    // connection and making the proxy flap. Reading to EOF is how we learn the
    // session is gone, whether it exited cleanly or was killed.
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }

        let Ok(Request::Register {
            packages,
            registries,
            verbose,
            ..
        }) = serde_json::from_str::<Request>(line.trim())
        else {
            continue;
        };

        {
            let mut guard = state.lock().unwrap();
            if let Some(session) = guard.sessions.get_mut(&id) {
                session.packages = packages;
                session.registries = registries;
                session.verbose = verbose;
            }
        }

        let reply = match resync(&state, owner_uid) {
            Ok(()) => Reply::Ok,
            Err(message) => Reply::Error { message },
        };
        if acks.send(reply).is_err() {
            break;
        }
    }

    drop_session(&state, id, owner_uid);
    drop(forwarder);
}

fn drop_session(state: &Shared, id: u64, owner_uid: u32) {
    state.lock().unwrap().sessions.remove(&id);
    if let Err(e) = resync(state, owner_uid) {
        eprintln!("smuggle daemon: {e}");
    }
}

fn send(mut stream: &UnixStream, reply: &Reply) -> std::io::Result<()> {
    let mut line = serde_json::to_string(reply).unwrap_or_default();
    line.push('\n');
    stream.write_all(line.as_bytes())
}

/// Bring the proxy in line with what is currently registered: start it, stop
/// it, or restart it with a new set of packages and registries.
fn resync(state: &Shared, owner_uid: u32) -> Result<(), String> {
    let (wanted, verbose) = {
        let state = state.lock().unwrap();
        let mut packages: Vec<String> = Vec::new();
        let mut registries: Vec<String> = Vec::new();
        let mut verbose = false;
        for session in state.sessions.values() {
            packages.extend(session.packages.iter().cloned());
            registries.extend(session.registries.iter().cloned());
            verbose |= session.verbose;
        }
        packages.sort();
        packages.dedup();
        registries.sort();
        registries.dedup();
        ((packages, registries), verbose)
    };

    let (packages, registries) = wanted;

    // Stopping the proxy is what removes the redirect, so with no sessions
    // left the machine goes back to normal.
    stop_proxy(state);

    if packages.is_empty() {
        return Ok(());
    }

    let child = spawn_proxy(&packages, &registries, verbose, owner_uid)?;
    let stdout = child.stdout.as_ref().map(|_| ());
    state.lock().unwrap().proxy = Some(child);

    if stdout.is_some() {
        fan_out_logs(state.clone());
    }
    Ok(())
}

fn stop_proxy(state: &Shared) {
    let child = state.lock().unwrap().proxy.take();
    let Some(mut child) = child else { return };

    // The proxy removes the redirect on SIGTERM. Killing it outright would
    // strand the hosts entry.
    unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };
    let _ = child.wait();
}

fn spawn_proxy(
    packages: &[String],
    registries: &[String],
    verbose: bool,
    owner_uid: u32,
) -> Result<Child, String> {
    let exe = super::launchd::staged_binary();

    // launchd gives this daemon no useful HOME, so the proxy is told outright
    // where the owner's CA and package store live.
    let home = super::home_of_uid(owner_uid)
        .ok_or_else(|| format!("could not resolve the home directory of uid {owner_uid}"))?;

    let mut command = Command::new(&exe);
    command.env(super::HOME_VAR, home.join(".smuggle"));
    command.arg("proxy").arg("--events");
    for name in packages {
        command.arg("--hijack").arg(name);
    }
    for registry in registries {
        command.arg("--registry").arg(registry);
    }
    if verbose {
        command.arg("--verbose");
    }

    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    command
        .spawn()
        .map_err(|e| format!("failed to start the proxy at {}: {e}", exe.display()))
}

/// Read the proxy's output and copy each line to every connected session, so a
/// session sees the same hijack log it saw when it owned the proxy directly.
fn fan_out_logs(state: Shared) {
    let (stdout, stderr) = {
        let mut guard = state.lock().unwrap();
        match guard.proxy.as_mut() {
            Some(child) => (child.stdout.take(), child.stderr.take()),
            None => (None, None),
        }
    };

    for source in [stdout.map(Box::new)].into_iter().flatten() {
        let state = state.clone();
        std::thread::spawn(move || pump(BufReader::new(source), state));
    }
    for source in [stderr.map(Box::new)].into_iter().flatten() {
        let state = state.clone();
        std::thread::spawn(move || pump(BufReader::new(source), state));
    }
}

fn pump<R: std::io::Read>(reader: BufReader<R>, state: Shared) {
    for line in reader.lines().map_while(Result::ok) {
        // The proxy emits structured events; anything else is passed through
        // verbatim rather than dropped.
        let reply = match serde_json::from_str::<Event>(&line) {
            Ok(event) => Reply::Event { event },
            Err(_) => Reply::Log { line },
        };

        let guard = state.lock().unwrap();
        for session in guard.sessions.values() {
            let _ = session.events.send(reply.clone());
        }
    }
}

/// What the daemon is currently doing, for `Request::Status`.
fn snapshot(state: &Shared) -> DaemonStatus {
    let state = state.lock().unwrap();

    let mut packages: Vec<String> = Vec::new();
    let mut registries: Vec<String> = Vec::new();
    for session in state.sessions.values() {
        packages.extend(session.packages.iter().cloned());
        registries.extend(session.registries.iter().cloned());
    }
    packages.sort();
    packages.dedup();
    registries.sort();
    registries.dedup();

    DaemonStatus {
        version: super::control::version(),
        sessions: state.sessions.len(),
        packages,
        registries,
        proxy_running: state.proxy.is_some(),
    }
}
