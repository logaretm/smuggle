use console::style;

use crate::net::{ca, hosts, launchd, trust};

/// Install the machine trust that lets the proxy terminate TLS for registry
/// hosts. Everything here is inert on its own: nothing is intercepted until
/// the proxy process is running.
pub fn cmd_setup() -> Result<(), String> {
    let _ = cliclack::intro(style(" smuggle setup ").on_cyan().black());

    if ca::exists() {
        let _ = cliclack::log::info(format!(
            "Reusing the existing CA at {}",
            style(ca::cert_path().display()).dim()
        ));
    } else {
        ca::create()?;
        let _ = cliclack::log::success(format!(
            "Created a local CA at {}",
            style(ca::cert_path().display()).dim()
        ));
    }

    // Keychain trust is a bonus: it covers curl, Bun, and Node run with
    // --use-system-ca. npm, pnpm and yarn all go through NODE_EXTRA_CA_CERTS
    // below, so a keychain failure is not worth aborting setup over.
    if trust::is_in_keychain() {
        let _ = cliclack::log::info("CA already trusted in the login keychain");
    } else {
        let _ = cliclack::log::remark(
            "Adding the CA to your login keychain (macOS will ask for your password)",
        );
        match trust::add_to_keychain() {
            Ok(()) => {
                let _ = cliclack::log::success("Trusted the CA in the login keychain");
            }
            Err(e) => {
                let _ = cliclack::log::warning(format!(
                    "Skipped keychain trust ({e}). npm, pnpm and yarn will still work."
                ));
            }
        }
    }

    let profile = trust::add_to_profile()?;
    let _ = cliclack::log::success(format!(
        "Pointed {} at the CA in {}",
        style("NODE_EXTRA_CA_CERTS").cyan(),
        style(profile.display()).dim(),
    ));

    install_daemon()?;

    if trust::env_var_active() {
        let _ = cliclack::outro("Setup complete. No further sudo prompts.");
    } else {
        let _ = cliclack::outro(format!(
            "Setup complete. Restart your shell (or {}) so npm picks up the CA.",
            style(format!("source {}", profile.display())).cyan(),
        ));
    }

    Ok(())
}

/// Install the root daemon. This is the only step that needs sudo, and it only
/// happens here: once it is running, a session registers over its socket and
/// never escalates.
fn install_daemon() -> Result<(), String> {
    let exe =
        std::env::current_exe().map_err(|e| format!("could not locate the smuggle binary: {e}"))?;

    if is_root() {
        launchd::install(&exe, invoking_uid())?;
        launchd::unload()?;
        launchd::load()?;
    } else {
        let _ = cliclack::log::remark(
            "Installing the background daemon needs sudo, once. Sessions will not ask again.",
        );
        let status = std::process::Command::new("sudo")
            .arg(&exe)
            .arg(crate::DAEMON_INSTALL_CMD)
            .status()
            .map_err(|e| format!("failed to run sudo: {e}"))?;

        if !status.success() {
            return Err("failed to install the smuggle daemon".into());
        }
    }

    let _ = cliclack::log::success(format!(
        "Installed the background daemon ({})",
        style(launchd::LABEL).dim()
    ));
    Ok(())
}

/// The user the daemon should answer to. Under sudo this is the invoking user,
/// not root.
pub fn invoking_uid() -> u32 {
    std::env::var("SUDO_UID")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| unsafe { libc::getuid() })
}

/// Root half of `setup`, re-invoked under sudo.
pub fn cmd_install_daemon() -> Result<(), String> {
    if !is_root() {
        return Err("this command must run as root".into());
    }
    let exe =
        std::env::current_exe().map_err(|e| format!("could not locate the smuggle binary: {e}"))?;
    launchd::install(&exe, invoking_uid())?;
    launchd::unload()?;
    launchd::load()
}

/// Root half of `cleanup`, re-invoked under sudo. Also clears the redirect,
/// so teardown is a single escalation.
pub fn cmd_remove_daemon() -> Result<(), String> {
    if !is_root() {
        return Err("this command must run as root".into());
    }
    hosts::remove()?;
    launchd::remove()
}

/// Undo everything `smuggle setup` did, plus any redirect a crashed proxy left
/// behind. Safe to run at any time, including when nothing is installed.
pub fn cmd_cleanup() -> Result<(), String> {
    let _ = cliclack::intro(style(" smuggle cleanup ").on_cyan().black());

    if launchd::is_installed() || hosts::read_block().is_some() {
        remove_daemon_as_root()?;
        let _ = cliclack::log::success("Removed the background daemon and any registry redirect");
    } else {
        let _ = cliclack::log::info("No background daemon installed");
    }

    if trust::is_in_keychain() {
        trust::remove_from_keychain()?;
        let _ = cliclack::log::success("Removed the CA from the login keychain");
    } else {
        let _ = cliclack::log::info("CA not present in the login keychain");
    }

    if trust::is_in_profile() {
        trust::remove_from_profile()?;
        let _ = cliclack::log::success(format!(
            "Removed {} from {}",
            style("NODE_EXTRA_CA_CERTS").cyan(),
            style(trust::profile_path().display()).dim(),
        ));
    } else {
        let _ = cliclack::log::info("No shell profile entry to remove");
    }

    if ca::exists() {
        ca::delete()?;
        let _ = cliclack::log::success("Deleted the local CA");
    } else {
        let _ = cliclack::log::info("No local CA on disk");
    }

    let _ = cliclack::outro("Cleanup complete.");
    Ok(())
}

/// Editing /etc/hosts needs root. Re-invoke ourselves under sudo for that one
/// step rather than asking the user to run all of cleanup as root, which would
/// touch the wrong keychain and the wrong shell profile.
fn remove_daemon_as_root() -> Result<(), String> {
    if is_root() {
        return cmd_remove_daemon();
    }

    let exe =
        std::env::current_exe().map_err(|e| format!("could not locate the smuggle binary: {e}"))?;

    let _ = cliclack::log::remark("Removing the daemon needs sudo");
    let status = std::process::Command::new("sudo")
        .arg(exe)
        .arg(crate::DAEMON_REMOVE_CMD)
        .status()
        .map_err(|e| format!("failed to run sudo: {e}"))?;

    if !status.success() {
        return Err("failed to remove the smuggle daemon".into());
    }
    Ok(())
}

pub fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

/// Strip a redirect left behind by a proxy that died without cleaning up.
/// Called at the start of every command so a crash can never keep intercepting.
pub fn reconcile_stale_redirect() {
    // A lockfile left pinned by a dead session is worse than a stranded
    // redirect: it fails for anyone else who installs, or quietly ships a
    // local build.
    crate::lockfile::reconcile();

    let Some(block) = hosts::read_block() else {
        return;
    };
    if hosts::pid_alive(block.pid) {
        return;
    }

    let _ = cliclack::log::warning(format!(
        "Found a leftover redirect for {} from a dead smuggle process (pid {})",
        block.hosts.join(", "),
        block.pid,
    ));
    if remove_daemon_as_root().is_err() {
        let _ = cliclack::log::warning(format!(
            "Could not remove it. Run {} to clear it.",
            style("smuggle cleanup").cyan()
        ));
    }
}
