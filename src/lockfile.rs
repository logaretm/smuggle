//! Pinning smuggled packages in the consumer's lockfile.
//!
//! Package managers are content-addressed: the lockfile records an integrity
//! hash, and npm's cacache and pnpm's store are keyed by that same hash. With
//! the original hash in place the manager resolves from cache and never asks
//! the network, so the proxy is never consulted.
//!
//! Rewriting the integrity to the hash of the tarball we serve fixes both
//! halves at once. The manager sees the tree no longer matches what is
//! installed, and the new hash is by construction absent from every cache, so
//! the fetch has to go out and reaches the proxy. Clearing caches by hand is
//! never necessary.
//!
//! The rewritten lockfile is only satisfiable while the proxy runs, so the
//! original is backed up under `~/.smuggle/sessions/` and restored on exit.
//! A crashed session leaves a backup whose owning pid is gone, which
//! [`reconcile`] undoes on the next invocation.

use console::style;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::net::{hosts, smuggle_home};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Npm,
    Pnpm,
}

impl Kind {
    fn command(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Kind::Npm => ("npm", &["install"]),
            Kind::Pnpm => ("pnpm", &["install", "--no-frozen-lockfile"]),
        }
    }
}

#[derive(Debug)]
pub struct Lockfile {
    pub path: PathBuf,
    pub kind: Kind,
}

/// Find the lockfile to pin. Yarn and bun are recognised so they can be
/// reported rather than silently skipped.
pub fn detect(consumer_dir: &Path) -> Result<Lockfile, String> {
    let candidates = [
        ("pnpm-lock.yaml", Some(Kind::Pnpm)),
        ("package-lock.json", Some(Kind::Npm)),
        ("yarn.lock", None),
        ("bun.lockb", None),
        ("bun.lock", None),
    ];

    for (name, kind) in candidates {
        let path = consumer_dir.join(name);
        if !path.exists() {
            continue;
        }
        return match kind {
            Some(kind) => Ok(Lockfile { path, kind }),
            None => Err(format!(
                "{name} is not supported yet — smuggle can pin npm and pnpm lockfiles.\nWithout pinning, your package manager resolves from cache and never reaches the proxy."
            )),
        };
    }

    Err("no package-lock.json or pnpm-lock.yaml found in the consumer directory".into())
}

/// Point every entry for a smuggled package at the hash of the tarball we
/// serve. Returns the new contents and how many entries changed.
pub fn rewrite(
    kind: Kind,
    contents: &str,
    hashes: &HashMap<String, String>,
) -> Result<(String, usize), String> {
    match kind {
        Kind::Npm => rewrite_npm(contents, hashes),
        Kind::Pnpm => Ok(rewrite_pnpm(contents, hashes)),
    }
}

fn rewrite_npm(
    contents: &str,
    hashes: &HashMap<String, String>,
) -> Result<(String, usize), String> {
    let mut doc: serde_json::Value =
        serde_json::from_str(contents).map_err(|e| format!("lockfile is not valid JSON: {e}"))?;
    let mut changed = 0;

    // lockfileVersion 2 and 3: a flat map keyed by install path.
    if let Some(packages) = doc.get_mut("packages").and_then(|p| p.as_object_mut()) {
        for (path, entry) in packages.iter_mut() {
            let Some(name) = npm_name_from_path(path) else {
                continue;
            };
            if set_integrity(entry, hashes.get(name)) {
                changed += 1;
            }
        }
    }

    // lockfileVersion 1 and 2 also carry a nested tree keyed by name.
    if let Some(deps) = doc.get_mut("dependencies").and_then(|d| d.as_object_mut()) {
        changed += rewrite_npm_tree(deps, hashes);
    }

    let out = serde_json::to_string_pretty(&doc)
        .map_err(|e| format!("could not re-encode the lockfile: {e}"))?;
    Ok((out + "\n", changed))
}

fn rewrite_npm_tree(
    deps: &mut serde_json::Map<String, serde_json::Value>,
    hashes: &HashMap<String, String>,
) -> usize {
    let mut changed = 0;
    for (name, entry) in deps.iter_mut() {
        if set_integrity(entry, hashes.get(name.as_str())) {
            changed += 1;
        }
        if let Some(nested) = entry
            .get_mut("dependencies")
            .and_then(|d| d.as_object_mut())
        {
            changed += rewrite_npm_tree(nested, hashes);
        }
    }
    changed
}

/// Only rewrite entries that already carry an integrity. Workspace links and
/// the root project have none, and inventing one would break the install.
fn set_integrity(entry: &mut serde_json::Value, hash: Option<&String>) -> bool {
    let (Some(hash), Some(object)) = (hash, entry.as_object_mut()) else {
        return false;
    };
    if !object.contains_key("integrity") {
        return false;
    }
    object.insert("integrity".into(), serde_json::Value::String(hash.clone()));
    true
}

/// `node_modules/foo/node_modules/@scope/bar` describes `@scope/bar`.
fn npm_name_from_path(path: &str) -> Option<&str> {
    let name = path.rsplit_once("node_modules/").map(|(_, n)| n)?;
    if name.is_empty() { None } else { Some(name) }
}

/// pnpm lockfiles are edited line by line rather than round-tripped, so
/// everything we do not target keeps its exact formatting.
fn rewrite_pnpm(contents: &str, hashes: &HashMap<String, String>) -> (String, usize) {
    let mut out = String::with_capacity(contents.len());
    let mut current: Option<String> = None;
    let mut changed = 0;

    for line in contents.lines() {
        if let Some(name) = pnpm_key_name(line) {
            current = hashes.get(&name).cloned();
        } else if line.trim_start().starts_with("resolution:") {
            if let Some(hash) = &current {
                if let Some(rewritten) = replace_integrity(line, hash) {
                    out.push_str(&rewritten);
                    out.push('\n');
                    changed += 1;
                    continue;
                }
            }
        }
        out.push_str(line);
        out.push('\n');
    }

    (out, changed)
}

/// Read a package name from a lockfile key, for both the v9 form
/// (`'@scope/name@1.2.3':`) and the older v6 form (`/@scope/name/1.2.3:`).
fn pnpm_key_name(line: &str) -> Option<String> {
    // Package keys sit at exactly two spaces of indent under `packages:`.
    let rest = line.strip_prefix("  ")?;
    if rest.starts_with(' ') {
        return None;
    }
    let key = rest.strip_suffix(':')?.trim_matches('\'').trim_matches('"');

    if let Some(v6) = key.strip_prefix('/') {
        // `/@scope/name/1.2.3` or `/name/1.2.3`
        let (name, _version) = v6.rsplit_once('/')?;
        return Some(name.to_string());
    }

    // `@scope/name@1.2.3` or `name@1.2.3`: the version's `@` is the last one,
    // and a leading `@` belongs to the scope.
    let at = key.rfind('@').filter(|i| *i > 0)?;
    Some(key[..at].to_string())
}

fn replace_integrity(line: &str, hash: &str) -> Option<String> {
    let start = line.find("integrity: ")? + "integrity: ".len();
    let end = start + line[start..].find(['}', ','])?;
    Some(format!("{}{hash}{}", &line[..start], &line[end..]))
}

/// A pinned lockfile. Dropping this restores the original.
pub struct Pin {
    session: PathBuf,
    lockfile: PathBuf,
}

impl Drop for Pin {
    fn drop(&mut self) {
        if let Err(e) = restore(&self.session, &self.lockfile) {
            let _ = cliclack::log::warning(format!("failed to restore the lockfile: {e}"));
        }
    }
}

fn sessions_dir() -> PathBuf {
    smuggle_home().join("sessions")
}

fn session_id(lockfile: &Path) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    lockfile.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Back up the lockfile and rewrite it to point at the tarballs we serve.
pub fn pin(lockfile: &Lockfile, hashes: &HashMap<String, String>) -> Result<Pin, String> {
    let original = std::fs::read_to_string(&lockfile.path)
        .map_err(|e| format!("could not read {}: {e}", lockfile.path.display()))?;

    let (rewritten, changed) = rewrite(lockfile.kind, &original, hashes)?;

    if changed == 0 {
        return Err(format!(
            "none of the selected packages appear in {}. Are they dependencies of this project?",
            lockfile.path.display()
        ));
    }

    let session = sessions_dir().join(session_id(&lockfile.path));
    std::fs::create_dir_all(&session)
        .map_err(|e| format!("could not create {}: {e}", session.display()))?;
    std::fs::write(session.join("lockfile.bak"), &original)
        .map_err(|e| format!("could not write the lockfile backup: {e}"))?;
    std::fs::write(
        session.join("meta.json"),
        serde_json::json!({
            "pid": std::process::id(),
            "lockfile": lockfile.path,
        })
        .to_string(),
    )
    .map_err(|e| format!("could not write the session marker: {e}"))?;

    std::fs::write(&lockfile.path, rewritten)
        .map_err(|e| format!("could not write {}: {e}", lockfile.path.display()))?;

    let _ = cliclack::log::info(format!(
        "Pinned {} entr{} in {} to your local build",
        style(changed).cyan(),
        if changed == 1 { "y" } else { "ies" },
        style(
            lockfile
                .path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
        )
        .dim(),
    ));

    Ok(Pin {
        session,
        lockfile: lockfile.path.clone(),
    })
}

fn restore(session: &Path, lockfile: &Path) -> Result<(), String> {
    let backup = session.join("lockfile.bak");
    if backup.exists() {
        std::fs::copy(&backup, lockfile)
            .map_err(|e| format!("could not restore {}: {e}", lockfile.display()))?;
    }
    let _ = std::fs::remove_dir_all(session);
    Ok(())
}

/// Undo a pin left behind by a session that died without restoring. A pinned
/// lockfile that outlives its proxy is worse than a stranded redirect: it
/// fails for anyone else who installs, or quietly ships a local build.
pub fn reconcile() {
    let Ok(entries) = std::fs::read_dir(sessions_dir()) else {
        return;
    };

    for entry in entries.flatten() {
        let session = entry.path();
        let Ok(raw) = std::fs::read_to_string(session.join("meta.json")) else {
            continue;
        };
        let Ok(meta) = serde_json::from_str::<serde_json::Value>(&raw) else {
            continue;
        };
        let (Some(pid), Some(lockfile)) = (
            meta.get("pid").and_then(|p| p.as_i64()),
            meta.get("lockfile").and_then(|p| p.as_str()),
        ) else {
            continue;
        };

        if hosts::pid_alive(pid as i32) {
            continue;
        }

        let _ = cliclack::log::warning(format!(
            "Restoring {} — a smuggle session left it pinned to a local build",
            style(lockfile).cyan(),
        ));
        if let Err(e) = restore(&session, Path::new(lockfile)) {
            let _ = cliclack::log::warning(e);
        }
    }
}

/// Run the consumer's package manager so the pin takes effect.
pub fn install(consumer_dir: &Path, kind: Kind) -> Result<(), String> {
    let (program, args) = kind.command();

    let _ = cliclack::log::remark(format!(
        "Running {}",
        style(format!("{program} {}", args.join(" "))).cyan()
    ));

    let status = std::process::Command::new(program)
        .args(args)
        .current_dir(consumer_dir)
        .status()
        .map_err(|e| format!("failed to run `{program}`: {e}"))?;

    if !status.success() {
        return Err(format!("`{program} {}` failed", args.join(" ")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hashes() -> HashMap<String, String> {
        HashMap::from([("@sentry/browser".to_string(), "sha512-NEW".to_string())])
    }

    #[test]
    fn npm_rewrites_by_install_path() {
        let lock = r#"{
          "lockfileVersion": 3,
          "packages": {
            "": {"name": "app"},
            "node_modules/@sentry/browser": {"version": "10.0.0", "integrity": "sha512-OLD"},
            "node_modules/other": {"version": "1.0.0", "integrity": "sha512-KEEP"}
          }
        }"#;

        let (out, changed) = rewrite_npm(lock, &hashes()).unwrap();
        assert_eq!(changed, 1);
        let doc: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(
            doc["packages"]["node_modules/@sentry/browser"]["integrity"],
            "sha512-NEW"
        );
        assert_eq!(
            doc["packages"]["node_modules/other"]["integrity"],
            "sha512-KEEP"
        );
    }

    #[test]
    fn npm_rewrites_nested_copies_too() {
        let lock = r#"{"packages": {
            "node_modules/a/node_modules/@sentry/browser": {"integrity": "sha512-OLD"}
        }}"#;
        let (_, changed) = rewrite_npm(lock, &hashes()).unwrap();
        assert_eq!(changed, 1);
    }

    #[test]
    fn npm_leaves_entries_without_integrity_alone() {
        // Workspace links carry no integrity; inventing one breaks the install.
        let lock = r#"{"packages": {
            "node_modules/@sentry/browser": {"link": true, "resolved": "packages/browser"}
        }}"#;
        let (_, changed) = rewrite_npm(lock, &hashes()).unwrap();
        assert_eq!(changed, 0);
    }

    #[test]
    fn npm_rewrites_the_legacy_tree() {
        let lock = r#"{"dependencies": {
            "@sentry/browser": {"version": "10.0.0", "integrity": "sha512-OLD",
              "dependencies": {"@sentry/browser": {"integrity": "sha512-OLD"}}}
        }}"#;
        let (_, changed) = rewrite_npm(lock, &hashes()).unwrap();
        assert_eq!(changed, 2);
    }

    #[test]
    fn pnpm_rewrites_the_v9_form() {
        let lock = "packages:\n\n  '@sentry/browser@10.70.0':\n    resolution: {integrity: sha512-OLD==}\n    engines: {node: '>=18'}\n\n  'other@1.0.0':\n    resolution: {integrity: sha512-KEEP==}\n";
        let (out, changed) = rewrite_pnpm(lock, &hashes());
        assert_eq!(changed, 1);
        assert!(out.contains("resolution: {integrity: sha512-NEW}"));
        assert!(out.contains("sha512-KEEP=="));
        // Untargeted lines keep their exact formatting.
        assert!(out.contains("    engines: {node: '>=18'}"));
    }

    #[test]
    fn pnpm_rewrites_the_v6_form() {
        let lock =
            "packages:\n  /@sentry/browser/10.70.0:\n    resolution: {integrity: sha512-OLD==}\n";
        let (out, changed) = rewrite_pnpm(lock, &hashes());
        assert_eq!(changed, 1);
        assert!(out.contains("sha512-NEW"));
    }

    #[test]
    fn pnpm_ignores_deeper_lines_that_look_like_keys() {
        let lock = "packages:\n  'other@1.0.0':\n    dependencies:\n      '@sentry/browser@10.70.0': 10.70.0\n    resolution: {integrity: sha512-KEEP==}\n";
        let (out, changed) = rewrite_pnpm(lock, &hashes());
        assert_eq!(changed, 0);
        assert!(out.contains("sha512-KEEP=="));
    }

    #[test]
    fn pnpm_key_names_parse_both_forms() {
        assert_eq!(
            pnpm_key_name("  '@scope/n@1.2.3':").as_deref(),
            Some("@scope/n")
        );
        assert_eq!(pnpm_key_name("  n@1.2.3:").as_deref(), Some("n"));
        assert_eq!(
            pnpm_key_name("  /@scope/n/1.2.3:").as_deref(),
            Some("@scope/n")
        );
        assert_eq!(pnpm_key_name("    nested@1.0.0:"), None);
        assert_eq!(pnpm_key_name("packages:"), None);
    }

    #[test]
    fn npm_names_come_from_the_last_node_modules_segment() {
        assert_eq!(npm_name_from_path("node_modules/@a/b"), Some("@a/b"));
        assert_eq!(
            npm_name_from_path("node_modules/x/node_modules/y"),
            Some("y")
        );
        assert_eq!(npm_name_from_path(""), None);
    }

    #[test]
    fn yarn_and_bun_are_reported_rather_than_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("yarn.lock"), "").unwrap();
        let err = detect(tmp.path()).unwrap_err();
        assert!(err.contains("not supported yet"), "{err}");
    }

    #[test]
    fn pnpm_wins_over_npm_when_both_exist() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("pnpm-lock.yaml"), "").unwrap();
        std::fs::write(tmp.path().join("package-lock.json"), "{}").unwrap();
        assert_eq!(detect(tmp.path()).unwrap().kind, Kind::Pnpm);
    }
}
