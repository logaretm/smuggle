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

use super::control::{Register, Reply, socket_path};

/// One connected session.
struct Session {
    register: Register,
    logs: Sender<String>,
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
        std::thread::spawn(move || serve_session(stream, state));
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

fn serve_session(stream: UnixStream, state: Shared) {
    let Ok(write_half) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(stream);

    let mut line = String::new();
    if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
        return;
    }

    let register: Register = match serde_json::from_str(line.trim()) {
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

    if register.version != super::control::version() {
        let _ = send(
            &write_half,
            &Reply::Error {
                message: format!(
                    "daemon is running {} but you are on {}. Run `smuggle setup` to restage it.",
                    super::control::version(),
                    register.version,
                ),
            },
        );
        return;
    }

    // Log lines are pushed from the proxy-reading thread, so the connection
    // gets its own channel rather than being written to from there directly.
    let (logs, incoming) = channel::<String>();
    let id = {
        let mut state = state.lock().unwrap();
        let id = state.next_id;
        state.next_id += 1;
        state.sessions.insert(id, Session { register, logs });
        id
    };

    if let Err(e) = resync(&state) {
        let _ = send(&write_half, &Reply::Error { message: e });
        drop_session(&state, id);
        return;
    }

    if send(&write_half, &Reply::Ok).is_err() {
        drop_session(&state, id);
        return;
    }

    // Forward proxy output to this session for as long as it is connected.
    let forwarder = std::thread::spawn(move || {
        for line in incoming {
            if send(&write_half, &Reply::Log { line }).is_err() {
                break;
            }
        }
    });

    // The session holds the connection open. Reading to EOF is how we learn it
    // is gone, whether it exited cleanly or was killed.
    let mut sink = String::new();
    while matches!(reader.read_line(&mut sink), Ok(n) if n > 0) {
        sink.clear();
    }

    drop_session(&state, id);
    drop(forwarder);
}

fn drop_session(state: &Shared, id: u64) {
    state.lock().unwrap().sessions.remove(&id);
    if let Err(e) = resync(state) {
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
fn resync(state: &Shared) -> Result<(), String> {
    let (wanted, verbose) = {
        let state = state.lock().unwrap();
        let mut packages: Vec<String> = Vec::new();
        let mut registries: Vec<String> = Vec::new();
        let mut verbose = false;
        for session in state.sessions.values() {
            packages.extend(session.register.packages.iter().cloned());
            registries.extend(session.register.registries.iter().cloned());
            verbose |= session.register.verbose;
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

    let child = spawn_proxy(&packages, &registries, verbose)?;
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

fn spawn_proxy(packages: &[String], registries: &[String], verbose: bool) -> Result<Child, String> {
    let exe = super::launchd::staged_binary();

    let mut command = Command::new(&exe);
    command.arg("proxy");
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
        let guard = state.lock().unwrap();
        for session in guard.sessions.values() {
            let _ = session.logs.send(line.clone());
        }
    }
}
