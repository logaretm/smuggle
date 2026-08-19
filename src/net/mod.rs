pub mod ca;
pub mod hijack;
pub mod hosts;
pub mod loopback;
pub mod proxy;
pub mod trust;

use std::path::PathBuf;

/// Root of smuggle's on-disk state (`~/.smuggle`).
///
/// The proxy runs under sudo, where `HOME` may point at root's home depending
/// on the sudoers config. Resolve the invoking user's home instead so the
/// proxy reads the same CA and package store the CLI wrote.
pub fn smuggle_home() -> PathBuf {
    real_home().join(".smuggle")
}

fn real_home() -> PathBuf {
    if let Ok(user) = std::env::var("SUDO_USER") {
        if let Some(home) = home_of(&user) {
            return home;
        }
    }
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()))
}

fn home_of(user: &str) -> Option<PathBuf> {
    use std::ffi::{CStr, CString, OsStr};
    use std::os::unix::ffi::OsStrExt;

    let name = CString::new(user).ok()?;
    // SAFETY: getpwnam returns a pointer into a static buffer that stays valid
    // until the next call; we copy out of it immediately.
    let entry = unsafe { libc::getpwnam(name.as_ptr()) };
    if entry.is_null() {
        return None;
    }
    let dir = unsafe { CStr::from_ptr((*entry).pw_dir) };
    Some(PathBuf::from(OsStr::from_bytes(dir.to_bytes())))
}

/// Loopback address the proxy listens on. Deliberately not 127.0.0.1 so we
/// don't squat on the machine's primary loopback :443. macOS configures only
/// 127.0.0.1 on lo0, so the proxy adds this as an interface alias while it
/// runs (see [`loopback`]).
pub const LISTEN_IP: &str = "127.0.0.2";

/// Registry hosts intercepted by default.
pub const DEFAULT_REGISTRY_HOSTS: &[&str] = &["registry.npmjs.org", "registry.yarnpkg.com"];
