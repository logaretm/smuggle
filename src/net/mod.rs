pub mod ca;
pub mod control;
pub mod daemon;
pub mod hijack;
pub mod hosts;
pub mod launchd;
pub mod loopback;
pub mod proxy;
pub mod trust;

use std::path::PathBuf;

/// Environment variable the daemon uses to tell the proxy whose state to read.
pub const HOME_VAR: &str = "SMUGGLE_HOME";

/// Root of smuggle's on-disk state (`~/.smuggle`).
///
/// This has to resolve to the user's directory even when the reader is not the
/// user. The proxy is started by the launchd daemon, which runs as root with
/// no meaningful `HOME`, so without an explicit answer it would look for the
/// CA and the package store somewhere they will never be.
pub fn smuggle_home() -> PathBuf {
    if let Ok(explicit) = std::env::var(HOME_VAR) {
        if !explicit.is_empty() {
            return PathBuf::from(explicit);
        }
    }
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

/// The home directory of a uid, for resolving the owner's state from a root
/// process that has no relationship to their environment.
pub fn home_of_uid(uid: u32) -> Option<PathBuf> {
    use std::ffi::{CStr, OsStr};
    use std::os::unix::ffi::OsStrExt;

    // SAFETY: getpwuid returns a pointer into a static buffer valid until the
    // next call; we copy out of it immediately.
    let entry = unsafe { libc::getpwuid(uid) };
    if entry.is_null() {
        return None;
    }
    let dir = unsafe { CStr::from_ptr((*entry).pw_dir) };
    Some(PathBuf::from(OsStr::from_bytes(dir.to_bytes())))
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

/// Registry URLs intercepted when none are detected or given.
pub const DEFAULT_REGISTRIES: &[&str] = &[
    "https://registry.npmjs.org/",
    "https://registry.yarnpkg.com/",
];

/// A registry to intercept.
///
/// Corporate registries are often mounted under a path, as in
/// `https://host/npm/`. Requests then arrive as `/npm/@scope%2fname`, so the
/// prefix has to come off before a path can be read as a package name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registry {
    pub host: String,
    pub prefix: String,
}

impl Registry {
    /// Parse a registry URL. Accepts a bare host for convenience.
    pub fn parse(url: &str) -> Result<Self, String> {
        let rest = url
            .strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))
            .unwrap_or(url);

        let (authority, path) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, ""),
        };

        // Credentials and ports are not part of the name we redirect.
        let host = authority
            .rsplit('@')
            .next()
            .unwrap_or(authority)
            .split(':')
            .next()
            .unwrap_or(authority);

        if host.is_empty() {
            return Err(format!("{url} has no host"));
        }

        Ok(Self {
            host: host.to_string(),
            prefix: path.trim_end_matches('/').to_string(),
        })
    }

    /// Strip this registry's mount path from a request path, so what remains
    /// can be read as a package route. Returns None when the path is outside
    /// the mount.
    pub fn strip_prefix<'a>(&self, path: &'a str) -> Option<&'a str> {
        if self.prefix.is_empty() {
            return Some(path);
        }
        match path.strip_prefix(&self.prefix) {
            Some("") => Some("/"),
            Some(rest) if rest.starts_with('/') => Some(rest),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_plain_registry() {
        let r = Registry::parse("https://registry.npmjs.org/").unwrap();
        assert_eq!(r.host, "registry.npmjs.org");
        assert_eq!(r.prefix, "");
    }

    #[test]
    fn parses_a_mounted_registry() {
        let r = Registry::parse("https://sfw.security.sentry.io/npm/").unwrap();
        assert_eq!(r.host, "sfw.security.sentry.io");
        assert_eq!(r.prefix, "/npm");
    }

    #[test]
    fn drops_credentials_and_port() {
        let r = Registry::parse("https://user:pass@host.example:8080/npm/").unwrap();
        assert_eq!(r.host, "host.example");
        assert_eq!(r.prefix, "/npm");
    }

    #[test]
    fn accepts_a_bare_host() {
        let r = Registry::parse("registry.npmjs.org").unwrap();
        assert_eq!(r.host, "registry.npmjs.org");
        assert_eq!(r.prefix, "");
    }

    #[test]
    fn strips_the_mount_path() {
        let r = Registry::parse("https://host/npm/").unwrap();
        assert_eq!(
            r.strip_prefix("/npm/@sentry%2fbrowser"),
            Some("/@sentry%2fbrowser")
        );
        assert_eq!(r.strip_prefix("/npm"), Some("/"));
        // Outside the mount: not a package route for this registry.
        assert_eq!(r.strip_prefix("/other/thing"), None);
    }

    #[test]
    fn an_unmounted_registry_passes_paths_through() {
        let r = Registry::parse("https://registry.npmjs.org/").unwrap();
        assert_eq!(r.strip_prefix("/is-number"), Some("/is-number"));
    }
}

#[cfg(test)]
mod home_tests {
    use super::*;

    #[test]
    fn an_explicit_home_wins_over_the_environment() {
        // SAFETY: single-threaded test.
        unsafe {
            std::env::set_var(HOME_VAR, "/tmp/explicit-smuggle");
            std::env::set_var("HOME", "/var/root");
        }
        assert_eq!(smuggle_home(), PathBuf::from("/tmp/explicit-smuggle"));
        unsafe { std::env::remove_var(HOME_VAR) };
    }

    #[test]
    fn an_empty_explicit_home_is_ignored() {
        // SAFETY: single-threaded test.
        unsafe {
            std::env::set_var(HOME_VAR, "");
            std::env::set_var("HOME", "/tmp/fallback");
            std::env::remove_var("SUDO_USER");
        }
        assert_eq!(smuggle_home(), PathBuf::from("/tmp/fallback/.smuggle"));
        unsafe { std::env::remove_var(HOME_VAR) };
    }

    #[test]
    fn a_uid_resolves_to_its_home() {
        // root always exists and has a home, unlike an arbitrary test user.
        assert!(home_of_uid(0).is_some());
        assert_eq!(home_of_uid(u32::MAX), None);
    }
}
