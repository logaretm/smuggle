//! The protocol between a session and the root daemon.
//!
//! A session opens the socket, registers what it wants hijacked, and holds the
//! connection for as long as it runs. The connection is the liveness signal:
//! when it drops, for any reason including a kill, the daemon deregisters that
//! session. Interception therefore still lasts exactly as long as a session,
//! with no pid markers to reconcile.

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

/// Sent once, when a session registers.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Register {
    pub packages: Vec<String>,
    pub registries: Vec<String>,
    #[serde(default)]
    pub verbose: bool,
    /// The CLI's version. A daemon staged from an older build would behave
    /// differently from the binary the user just installed, so the mismatch is
    /// reported rather than silently tolerated.
    pub version: String,
}

/// Sent by the daemon.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Reply {
    Ok,
    Error { message: String },
    Log { line: String },
}

pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
