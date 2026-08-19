use console::style;

use crate::net::{ca, hosts, trust};

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

    if trust::env_var_active() {
        let _ = cliclack::outro("Setup complete.");
    } else {
        let _ = cliclack::outro(format!(
            "Setup complete. Restart your shell (or {}) so npm picks up the CA.",
            style(format!("source {}", profile.display())).cyan(),
        ));
    }

    Ok(())
}

/// Undo everything `smuggle setup` did, plus any redirect a crashed proxy left
/// behind. Safe to run at any time, including when nothing is installed.
pub fn cmd_cleanup() -> Result<(), String> {
    let _ = cliclack::intro(style(" smuggle cleanup ").on_cyan().black());

    match hosts::read_block() {
        Some(_) => {
            clear_hosts_as_root()?;
            let _ = cliclack::log::success("Removed the registry redirect from /etc/hosts");
        }
        None => {
            let _ = cliclack::log::info("No registry redirect in /etc/hosts");
        }
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
fn clear_hosts_as_root() -> Result<(), String> {
    if is_root() {
        return hosts::remove();
    }

    let exe =
        std::env::current_exe().map_err(|e| format!("could not locate the smuggle binary: {e}"))?;

    let _ = cliclack::log::remark("Removing the /etc/hosts redirect needs sudo");
    let status = std::process::Command::new("sudo")
        .arg(exe)
        .arg(crate::HOSTS_CLEAR_CMD)
        .status()
        .map_err(|e| format!("failed to run sudo: {e}"))?;

    if !status.success() {
        return Err("failed to clear the /etc/hosts redirect".into());
    }
    Ok(())
}

pub fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

/// Strip a redirect left behind by a proxy that died without cleaning up.
/// Called at the start of every command so a crash can never keep intercepting.
pub fn reconcile_stale_redirect() {
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
    if clear_hosts_as_root().is_err() {
        let _ = cliclack::log::warning(format!(
            "Could not remove it. Run {} to clear it.",
            style("smuggle cleanup").cyan()
        ));
    }
}
