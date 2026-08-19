//! Running the interception proxy for the length of a smuggle session.
//!
//! The proxy needs root, so it runs as a child process under sudo. It reads
//! tarballs from the store on every request, which means a repack is picked up
//! without any further coordination: the store is the control plane.

use console::style;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::{store, watch};

/// A running proxy. Dropping this closes the pipe the proxy is reading, which
/// is how it learns to shut down and undo the redirect.
pub struct Proxy {
    child: Child,
}

impl Drop for Proxy {
    fn drop(&mut self) {
        // Closing stdin is the shutdown signal. The proxy runs as root, so an
        // unprivileged parent cannot signal it directly.
        drop(self.child.stdin.take());
        let _ = self.child.wait();
    }
}

/// Start the proxy with `packages` hijacked, prompting for sudo if needed.
pub fn start(packages: &[String]) -> Result<Proxy, String> {
    let exe =
        std::env::current_exe().map_err(|e| format!("could not locate the smuggle binary: {e}"))?;

    prime_sudo()?;

    let mut command = Command::new("sudo");
    command.arg("-n").arg(&exe).arg("proxy");
    for name in packages {
        command.arg("--hijack").arg(name);
    }
    command
        .arg("--exit-on-parent-close")
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let child = command
        .spawn()
        .map_err(|e| format!("failed to start the proxy: {e}"))?;

    Ok(Proxy { child })
}

/// Ask for the sudo password once, up front, so the proxy itself can be
/// launched with a piped stdin it will use only as a shutdown signal.
fn prime_sudo() -> Result<(), String> {
    let already_valid = Command::new("sudo")
        .args(["-n", "true"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if already_valid {
        return Ok(());
    }

    let _ = cliclack::log::remark("The proxy binds :443 and edits /etc/hosts, so it needs sudo");
    let status = Command::new("sudo")
        .arg("-v")
        .status()
        .map_err(|e| format!("failed to run sudo: {e}"))?;

    if !status.success() {
        return Err("sudo was declined".into());
    }
    Ok(())
}

/// Hold a set of packages hijacked until interrupted, repacking on change.
/// `on_repack` runs after each successful repack.
pub fn run(
    selected: &[store::StoreEntry],
    on_repack: &mut dyn FnMut(&[String]),
) -> Result<(), String> {
    let names: Vec<String> = selected.iter().map(|e| e.name.clone()).collect();
    let refs: Vec<&store::StoreEntry> = selected.iter().collect();

    let _proxy = start(&names)?;

    let _ = cliclack::log::success(format!(
        "Smuggling {}",
        style(names.join(", ")).cyan().bold(),
    ));
    let _ = cliclack::log::info(format!(
        "Install or reinstall in your project and it will resolve to your local copy.\nPress {} to stop.",
        style("ctrl-c").cyan(),
    ));

    let shutdown = Arc::new(AtomicBool::new(false));
    let flag = shutdown.clone();
    let _ = ctrlc::set_handler(move || flag.store(true, Ordering::SeqCst));

    watch::watch_and_repack(&refs, on_repack, &shutdown)?;

    let _ = cliclack::outro("Stopped. Nothing is intercepted any more.");
    Ok(())
}
