//! The protocol between a session and the root daemon.
//!
//! A session opens the socket, registers what it wants hijacked, and holds the
//! connection for as long as it runs. The connection is the liveness signal:
//! when it drops, for any reason including a kill, the daemon deregisters that
//! session. Interception therefore still lasts exactly as long as a session,
//! with no pid markers to reconcile.
//!
//! Everything the proxy reports travels as a structured [`Event`] rather than
//! a formatted line, so a plain session and a terminal UI can render the same
//! facts differently without either one parsing the other's text.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const SOCKET_PATH: &str = "/var/run/smuggle.sock";

/// The control socket. `SMUGGLE_SOCKET` overrides it, which allows running a
/// daemon somewhere writable for debugging without touching the installed one.
pub fn socket_path() -> PathBuf {
    match std::env::var("SMUGGLE_SOCKET") {
        Ok(path) if !path.is_empty() => PathBuf::from(path),
        _ => PathBuf::from(SOCKET_PATH),
    }
}

/// True when the daemon is running at the standard location, where it needs
/// root. A socket somewhere else is a debugging one and does not.
pub fn is_default_socket() -> bool {
    socket_path() == Path::new(SOCKET_PATH)
}

/// Bumped whenever the wire format changes.
///
/// The package version alone cannot catch this: two builds of the same version
/// can speak different protocols, and the daemon runs a staged copy that only
/// changes when setup is re-run. Without this, a stale daemon fails to parse a
/// newer request and reports a parse error instead of saying it is out of date.
const PROTOCOL_REVISION: u32 = 2;

pub fn version() -> String {
    format!("{}+p{PROTOCOL_REVISION}", env!("CARGO_PKG_VERSION"))
}

/// Sent once, as the first line on a connection.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "request", rename_all = "lowercase")]
pub enum Request {
    /// Hijack these packages for as long as this connection is held.
    Register {
        /// The CLI's version. A daemon staged from an older build would behave
        /// differently from the binary the user just installed, so a mismatch
        /// is reported rather than silently tolerated.
        version: String,
        packages: Vec<String>,
        registries: Vec<String>,
        #[serde(default)]
        verbose: bool,
    },
    /// Ask what the daemon is currently doing and disconnect.
    Status { version: String },
}

impl Request {
    pub fn version(&self) -> &str {
        match self {
            Request::Register { version, .. } | Request::Status { version } => version,
        }
    }
}

/// Sent by the daemon.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Reply {
    Ok,
    Error {
        message: String,
    },
    Status {
        status: DaemonStatus,
    },
    Event {
        event: Event,
    },
    /// Proxy output that was not structured, so it is passed through verbatim
    /// rather than dropped.
    Log {
        line: String,
    },
}

/// What the daemon is doing right now.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct DaemonStatus {
    pub version: String,
    pub sessions: usize,
    pub packages: Vec<String>,
    pub registries: Vec<String>,
    pub proxy_running: bool,
}

/// Something the proxy did, reported as facts rather than prose.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "event", rename_all = "lowercase")]
pub enum Event {
    /// The proxy is up and the redirect is installed.
    Listening {
        hosts: Vec<String>,
        addr: String,
    },
    /// A tarball was served from the local store.
    Served {
        package: String,
        bytes: usize,
    },
    /// A packument or manifest was rewritten to our integrity.
    Rewrote {
        package: String,
        document: String,
    },
    /// A request we do not answer for, forwarded upstream.
    Passthrough {
        host: String,
        path: String,
        status: u16,
    },
    Error {
        message: String,
    },
}

impl Event {
    /// Render for a plain session, matching what the proxy used to print.
    pub fn to_line(&self) -> String {
        match self {
            Event::Listening { hosts, addr } => {
                format!("intercepting {} on {addr}", hosts.join(", "))
            }
            Event::Served { package, bytes } => {
                format!("served {package} from the store ({bytes} bytes)")
            }
            Event::Rewrote { package, document } => {
                format!("rewrote {document} integrity for {package}")
            }
            Event::Passthrough { host, path, status } => {
                format!("{status} {host}{path}")
            }
            Event::Error { message } => message.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_round_trip() {
        let request = Request::Register {
            version: "1.2.3".into(),
            packages: vec!["a".into()],
            registries: vec!["https://r/".into()],
            verbose: true,
        };
        let encoded = serde_json::to_string(&request).unwrap();
        let decoded: Request = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded.version(), "1.2.3");
        assert!(matches!(decoded, Request::Register { verbose: true, .. }));
    }

    #[test]
    fn a_status_request_carries_a_version_too() {
        let decoded: Request =
            serde_json::from_str(r#"{"request":"status","version":"9.9.9"}"#).unwrap();
        assert_eq!(decoded.version(), "9.9.9");
    }

    #[test]
    fn verbose_defaults_to_off() {
        let decoded: Request = serde_json::from_str(
            r#"{"request":"register","version":"1","packages":[],"registries":[]}"#,
        )
        .unwrap();
        assert!(matches!(decoded, Request::Register { verbose: false, .. }));
    }

    #[test]
    fn the_version_carries_the_protocol_revision() {
        // A daemon staged from a build with a different wire format must be
        // detected even when the package version is identical.
        let version = version();
        assert!(version.starts_with(env!("CARGO_PKG_VERSION")));
        assert!(version.contains("+p"));
    }

    #[test]
    fn events_round_trip() {
        let event = Event::Served {
            package: "@scope/pkg".into(),
            bytes: 42,
        };
        let encoded = serde_json::to_string(&event).unwrap();
        assert_eq!(serde_json::from_str::<Event>(&encoded).unwrap(), event);
    }

    #[test]
    fn events_render_the_lines_sessions_used_to_print() {
        assert_eq!(
            Event::Served {
                package: "is-number".into(),
                bytes: 294
            }
            .to_line(),
            "served is-number from the store (294 bytes)"
        );
        assert_eq!(
            Event::Rewrote {
                package: "is-number".into(),
                document: "packument".into()
            }
            .to_line(),
            "rewrote packument integrity for is-number"
        );
    }
}
