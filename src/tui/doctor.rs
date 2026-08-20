//! Setup diagnostics.
//!
//! Every check answers a question that has actually cost a debugging session:
//! is the CA there, does the daemon match this binary, is the registry this
//! project uses actually the one being intercepted.

use std::path::Path;

use crate::net::{ca, control, hosts, launchd, trust};
use crate::{lockfile, session};

#[derive(PartialEq, Clone, Copy)]
pub enum Level {
    Ok,
    Warn,
    Bad,
}

pub struct Check {
    pub level: Level,
    pub label: String,
    pub detail: String,
}

impl Check {
    fn new(level: Level, label: &str, detail: impl Into<String>) -> Self {
        Self {
            level,
            label: label.to_string(),
            detail: detail.into(),
        }
    }
}

pub struct Report {
    pub machine: Vec<Check>,
    pub project: Vec<Check>,
}

impl Report {
    /// Shown until the first real report arrives. Building one shells out to
    /// npm, launchctl and security, which is far too slow to do on the event
    /// loop, so it happens on a worker thread.
    pub fn pending() -> Self {
        Self {
            machine: vec![Check::new(Level::Ok, "checking…", "")],
            project: Vec::new(),
        }
    }
}

impl Report {
    pub fn worst(&self) -> Level {
        let mut levels = self.machine.iter().chain(&self.project).map(|c| c.level);
        if levels.clone().any(|l| l == Level::Bad) {
            Level::Bad
        } else if levels.any(|l| l == Level::Warn) {
            Level::Warn
        } else {
            Level::Ok
        }
    }
}

pub fn run(consumer_dir: &Path, daemon: Option<&control::DaemonStatus>) -> Report {
    Report {
        machine: machine_checks(daemon),
        project: project_checks(consumer_dir),
    }
}

fn machine_checks(daemon: Option<&control::DaemonStatus>) -> Vec<Check> {
    let mut checks = Vec::new();

    checks.push(if ca::exists() {
        Check::new(Level::Ok, "local CA", ca::cert_path().display().to_string())
    } else {
        Check::new(Level::Bad, "local CA", "missing — run `smuggle setup`")
    });

    checks.push(if trust::is_in_keychain() {
        Check::new(Level::Ok, "keychain trust", "trusted as a root")
    } else {
        Check::new(
            Level::Warn,
            "keychain trust",
            "not trusted — npm still works, other tools may not",
        )
    });

    checks.push(match (trust::is_in_profile(), trust::env_var_active()) {
        (_, true) => Check::new(Level::Ok, "NODE_EXTRA_CA_CERTS", "active in this shell"),
        (true, false) => Check::new(
            Level::Ok,
            "NODE_EXTRA_CA_CERTS",
            format!(
                "in {} — smuggle passes it directly, so this only affects installs you run",
                trust::profile_path().display()
            ),
        ),
        (false, false) => Check::new(
            Level::Warn,
            "NODE_EXTRA_CA_CERTS",
            "not configured — installs you run yourself will not trust the CA",
        ),
    });

    checks.push(match (launchd::is_installed(), launchd::is_loaded()) {
        (true, true) => Check::new(Level::Ok, "daemon", "installed and running"),
        (true, false) => Check::new(
            Level::Bad,
            "daemon",
            "installed but not running — run `smuggle setup`",
        ),
        (false, _) => Check::new(Level::Bad, "daemon", "not installed — run `smuggle setup`"),
    });

    checks.push(match daemon {
        Some(status) if status.version == control::version() => Check::new(
            Level::Ok,
            "daemon version",
            format!("{} (matches this binary)", status.version),
        ),
        Some(status) => Check::new(
            Level::Bad,
            "daemon version",
            format!(
                "{} but this binary is {} — run `smuggle setup`",
                status.version,
                control::version()
            ),
        ),
        None => Check::new(Level::Bad, "daemon version", "could not reach the daemon"),
    });

    checks.push(match hosts::read_block() {
        None => Check::new(Level::Ok, "redirect", "/etc/hosts clean"),
        Some(block) if hosts::pid_alive(block.pid) => Check::new(
            Level::Ok,
            "redirect",
            format!("active for {}", block.hosts.join(", ")),
        ),
        Some(block) => Check::new(
            Level::Bad,
            "redirect",
            format!(
                "stale, owned by dead pid {} — run `smuggle cleanup`",
                block.pid
            ),
        ),
    });

    checks
}

fn project_checks(consumer_dir: &Path) -> Vec<Check> {
    let mut checks = Vec::new();

    match lockfile::detect(consumer_dir) {
        Ok(lock) => checks.push(Check::new(
            Level::Ok,
            "package manager",
            format!(
                "{}",
                lock.path.file_name().unwrap_or_default().to_string_lossy()
            ),
        )),
        Err(e) => checks.push(Check::new(Level::Bad, "package manager", e)),
    }

    let registries = session::detect_registries(consumer_dir);
    if registries.is_empty() {
        checks.push(Check::new(
            Level::Warn,
            "registry",
            "npm reported none — the public default will be intercepted",
        ));
    } else {
        for registry in &registries {
            checks.push(Check::new(Level::Ok, "registry", registry.clone()));
        }
    }

    checks
}
