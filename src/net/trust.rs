//! One-time machine trust for the smuggle CA.
//!
//! Node ignores the macOS keychain by default, so trusting the CA takes two
//! independent steps: the keychain (covers curl, Bun, and Node run with
//! `--use-system-ca`) and `NODE_EXTRA_CA_CERTS` (covers every Node process,
//! which is what npm, pnpm and yarn actually are).
//!
//! Neither step intercepts anything on its own. Trust is inert until the
//! daemon is running.

use std::path::PathBuf;

use super::ca;

const CERT_COMMON_NAME: &str = "smuggle local development CA";
const BEGIN: &str = "# smuggle:begin";
const END: &str = "# smuggle:end";
const ENV_VAR: &str = "NODE_EXTRA_CA_CERTS";

/// Add the CA to the user's login keychain as a trusted root.
/// Prompts for the login password via the system dialog.
pub fn add_to_keychain() -> Result<(), String> {
    let keychain = default_user_keychain()?;
    let status = std::process::Command::new("security")
        .arg("add-trusted-cert")
        .args(["-r", "trustRoot"])
        .arg("-k")
        .arg(&keychain)
        .arg(ca::cert_path())
        .status()
        .map_err(|e| format!("failed to run `security add-trusted-cert`: {e}"))?;

    if !status.success() {
        return Err(
            "`security add-trusted-cert` failed (was the password prompt dismissed?)".into(),
        );
    }
    Ok(())
}

/// Remove every copy of the CA from the user's login keychain.
/// `security delete-certificate` removes one match per call, so loop until
/// none are left, in case setup ran more than once.
pub fn remove_from_keychain() -> Result<(), String> {
    let keychain = default_user_keychain()?;
    while is_in_keychain() {
        let status = std::process::Command::new("security")
            .arg("delete-certificate")
            .args(["-c", CERT_COMMON_NAME])
            .arg("-t")
            .arg(&keychain)
            .status()
            .map_err(|e| format!("failed to run `security delete-certificate`: {e}"))?;

        if !status.success() {
            return Err("`security delete-certificate` failed".into());
        }
    }
    Ok(())
}

pub fn is_in_keychain() -> bool {
    std::process::Command::new("security")
        .args(["find-certificate", "-c", CERT_COMMON_NAME])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn default_user_keychain() -> Result<String, String> {
    let out = std::process::Command::new("security")
        .args(["default-keychain", "-d", "user"])
        .output()
        .map_err(|e| format!("failed to run `security default-keychain`: {e}"))?;

    let path = String::from_utf8_lossy(&out.stdout)
        .trim()
        .trim_matches('"')
        .to_string();

    if path.is_empty() {
        return Err("could not determine the login keychain".into());
    }
    Ok(path)
}

/// The shell profile we write the env var into, chosen from `$SHELL`.
pub fn profile_path() -> PathBuf {
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()));
    let shell = std::env::var("SHELL").unwrap_or_default();

    if shell.ends_with("fish") {
        home.join(".config/fish/config.fish")
    } else if shell.ends_with("bash") {
        // macOS login shells read .bash_profile, not .bashrc.
        home.join(".bash_profile")
    } else {
        home.join(".zshrc")
    }
}

fn export_line() -> String {
    let cert = ca::cert_path();
    let shell = std::env::var("SHELL").unwrap_or_default();
    if shell.ends_with("fish") {
        format!("set -gx {ENV_VAR} \"{}\"", cert.display())
    } else {
        format!("export {ENV_VAR}=\"{}\"", cert.display())
    }
}

/// Append the `NODE_EXTRA_CA_CERTS` export to the shell profile, replacing any
/// block a previous setup left behind.
pub fn add_to_profile() -> Result<PathBuf, String> {
    let path = profile_path();
    let existing = std::fs::read_to_string(&path).unwrap_or_default();

    let mut out = strip_block(&existing);
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&format!("{BEGIN}\n{}\n{END}\n", export_line()));

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
    }
    std::fs::write(&path, out).map_err(|e| format!("failed to write {}: {e}", path.display()))?;

    Ok(path)
}

/// Remove the block from the shell profile. Not an error if absent.
pub fn remove_from_profile() -> Result<(), String> {
    let path = profile_path();
    let Ok(existing) = std::fs::read_to_string(&path) else {
        return Ok(());
    };
    if !existing.contains(BEGIN) {
        return Ok(());
    }
    std::fs::write(&path, strip_block(&existing))
        .map_err(|e| format!("failed to write {}: {e}", path.display()))
}

pub fn is_in_profile() -> bool {
    std::fs::read_to_string(profile_path())
        .map(|c| c.contains(BEGIN))
        .unwrap_or(false)
}

/// True when the current process already has the env var pointing at our CA.
/// Used to tell the user whether they need to reload their shell.
pub fn env_var_active() -> bool {
    std::env::var(ENV_VAR)
        .map(|v| std::path::Path::new(&v) == ca::cert_path())
        .unwrap_or(false)
}

fn strip_block(contents: &str) -> String {
    let mut out = String::with_capacity(contents.len());
    let mut inside = false;

    for line in contents.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with(BEGIN) {
            inside = true;
            continue;
        }
        if inside {
            if trimmed.starts_with(END) {
                inside = false;
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_removes_only_our_block() {
        let input =
            format!("export FOO=1\n{BEGIN}\nexport {ENV_VAR}=\"/x\"\n{END}\nexport BAR=2\n");
        assert_eq!(strip_block(&input), "export FOO=1\nexport BAR=2\n");
    }

    #[test]
    fn strip_is_a_noop_without_a_block() {
        assert_eq!(strip_block("export FOO=1\n"), "export FOO=1\n");
    }

    #[test]
    fn profile_path_follows_shell() {
        // SAFETY: single-threaded test, and we restore nothing the suite reads.
        unsafe { std::env::set_var("SHELL", "/opt/homebrew/bin/fish") };
        assert!(profile_path().ends_with(".config/fish/config.fish"));
        unsafe { std::env::set_var("SHELL", "/bin/zsh") };
        assert!(profile_path().ends_with(".zshrc"));
    }
}
