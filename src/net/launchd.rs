//! Installing the root daemon as a launchd job.
//!
//! This is the one privileged step, done once by `smuggle setup`. Afterwards a
//! session needs no sudo at all.

use std::path::{Path, PathBuf};

pub const LABEL: &str = "dev.smuggle.daemon";

pub fn plist_path() -> PathBuf {
    PathBuf::from("/Library/LaunchDaemons").join(format!("{LABEL}.plist"))
}

/// Where the daemon's copy of the binary lives.
///
/// launchd runs it as root, so it must not sit anywhere the user can write.
/// `~/.cargo/bin/smuggle` is user-owned, and pointing a root job at it would
/// hand passwordless root to anything that can overwrite that file.
pub fn staged_binary() -> PathBuf {
    PathBuf::from("/usr/local/lib/smuggle/smuggle")
}

pub fn is_installed() -> bool {
    plist_path().exists()
}

/// True when launchd reports the job as loaded.
pub fn is_loaded() -> bool {
    std::process::Command::new("launchctl")
        .args(["print", &format!("system/{LABEL}")])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn plist(binary: &Path, owner_uid: u32) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{LABEL}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{}</string>
    <string>daemon</string>
    <string>--owner-uid</string>
    <string>{owner_uid}</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>StandardOutPath</key>
  <string>/var/log/smuggle.log</string>
  <key>StandardErrorPath</key>
  <string>/var/log/smuggle.log</string>
</dict>
</plist>
"#,
        binary.display()
    )
}

/// Stage the binary and write the plist. Must run as root.
pub fn install(source: &Path, owner_uid: u32) -> Result<(), String> {
    let target = staged_binary();
    let dir = target.parent().expect("staged binary has a parent");

    std::fs::create_dir_all(dir).map_err(|e| format!("could not create {}: {e}", dir.display()))?;

    // Replace rather than overwrite in place, so a running daemon keeps its
    // open image and a restart picks up the new one.
    let _ = std::fs::remove_file(&target);
    std::fs::copy(source, &target)
        .map_err(|e| format!("could not stage the binary at {}: {e}", target.display()))?;

    set_root_owned(dir)?;
    set_root_owned(&target)?;

    std::fs::write(plist_path(), plist(&target, owner_uid))
        .map_err(|e| format!("could not write {}: {e}", plist_path().display()))?;
    set_root_owned(&plist_path())?;

    Ok(())
}

/// Root ownership with no write bit for anyone else, so the daemon's image
/// cannot be swapped by an unprivileged process.
fn set_root_owned(path: &Path) -> Result<(), String> {
    use std::os::unix::ffi::OsStrExt;

    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|e| format!("bad path {}: {e}", path.display()))?;

    // SAFETY: the path is a valid NUL-terminated string for the call's lifetime.
    if unsafe { libc::chown(c_path.as_ptr(), 0, 0) } != 0 {
        return Err(format!(
            "could not set root ownership on {}: {}",
            path.display(),
            std::io::Error::last_os_error()
        ));
    }

    if unsafe { libc::chmod(c_path.as_ptr(), 0o755) } != 0 {
        return Err(format!(
            "could not set permissions on {}: {}",
            path.display(),
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

pub fn load() -> Result<(), String> {
    launchctl(&["bootstrap", "system", &plist_path().to_string_lossy()])
}

pub fn unload() -> Result<(), String> {
    // Not an error if it was never loaded.
    let _ = launchctl(&["bootout", &format!("system/{LABEL}")]);
    Ok(())
}

fn launchctl(args: &[&str]) -> Result<(), String> {
    let output = std::process::Command::new("launchctl")
        .args(args)
        .output()
        .map_err(|e| format!("failed to run launchctl: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "launchctl {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

/// Remove the job and everything staged for it. Must run as root.
pub fn remove() -> Result<(), String> {
    unload()?;
    let _ = std::fs::remove_file(plist_path());
    let _ = std::fs::remove_file(staged_binary());
    if let Some(dir) = staged_binary().parent() {
        let _ = std::fs::remove_dir(dir);
    }
    let _ = std::fs::remove_file(super::control::SOCKET_PATH);
    Ok(())
}
