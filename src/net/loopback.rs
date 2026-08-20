//! Loopback alias management.
//!
//! Unlike Linux, macOS only configures `127.0.0.1` on `lo0`. Listening on any
//! other loopback address needs an explicit interface alias, which we add on
//! proxy startup and remove on exit alongside the `/etc/hosts` redirect.

use std::net::IpAddr;

/// True when `ip` is already configured on `lo0`.
pub fn is_configured(ip: IpAddr) -> bool {
    let Ok(out) = std::process::Command::new("ifconfig").arg("lo0").output() else {
        return false;
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.trim().strip_prefix("inet "))
        .any(|l| l.split_whitespace().next() == Some(&ip.to_string()))
}

/// Add `ip` to `lo0` unless it is already there. Requires root.
/// Returns true when an alias was actually added, so the caller knows whether
/// it owns the teardown.
pub fn add_alias(ip: IpAddr) -> Result<bool, String> {
    if ip.is_loopback() && ip.to_string() == "127.0.0.1" {
        return Ok(false);
    }
    if is_configured(ip) {
        return Ok(false);
    }

    let status = std::process::Command::new("ifconfig")
        .args(["lo0", "alias", &ip.to_string(), "up"])
        .status()
        .map_err(|e| format!("failed to run ifconfig: {e}"))?;

    if !status.success() {
        return Err(format!("failed to add {ip} to lo0"));
    }
    Ok(true)
}

/// Remove `ip` from `lo0`. Requires root.
pub fn remove_alias(ip: IpAddr) -> Result<(), String> {
    let status = std::process::Command::new("ifconfig")
        .args(["lo0", "-alias", &ip.to_string()])
        .status()
        .map_err(|e| format!("failed to run ifconfig: {e}"))?;

    if !status.success() {
        return Err(format!("failed to remove {ip} from lo0"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Reads `ifconfig lo0`, which is a macOS interface name. CI runs on Linux,
    // where the interface is `lo` and ifconfig may not be installed at all.
    #[cfg(target_os = "macos")]
    #[test]
    fn primary_loopback_is_always_configured() {
        assert!(is_configured("127.0.0.1".parse().unwrap()));
    }

    #[test]
    fn primary_loopback_is_never_aliased() {
        // Would need root if it tried, so this also proves it short-circuits.
        assert_eq!(add_alias("127.0.0.1".parse().unwrap()), Ok(false));
    }
}
