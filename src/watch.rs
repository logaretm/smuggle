//! Source watching for smuggled packages.
//!
//! When a package's sources change, it is repacked and written back to the
//! store. The proxy reads tarballs from the store on every request, so a
//! repack is all it takes for the next install to serve new content.

use console::style;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::{pack, store};

/// Watch the sources of `selected` and repack into the store on change.
/// Blocks until `shutdown` is set. `on_repack` is called with the names of the
/// packages whose packed content actually changed.
pub fn watch_and_repack(
    selected: &[&store::StoreEntry],
    on_repack: &mut dyn FnMut(&[String]),
    shutdown: &Arc<AtomicBool>,
) -> Result<(), String> {
    use notify::{RecursiveMode, Watcher};
    use std::collections::HashMap;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    let (tx, rx) = mpsc::channel();

    let mut watcher =
        notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                if matches!(
                    event.kind,
                    notify::EventKind::Create(_)
                        | notify::EventKind::Modify(_)
                        | notify::EventKind::Remove(_)
                ) {
                    let _ = tx.send(event);
                }
            }
        })
        .map_err(|e| format!("failed to create watcher: {e}"))?;

    for entry in selected {
        let watch_dirs = resolve_watch_dirs(&entry.source_dir);
        for dir in &watch_dirs {
            watcher
                .watch(dir, RecursiveMode::Recursive)
                .map_err(|e| format!("failed to watch {}: {e}", dir.display()))?;
        }
        // Watch package root non-recursively for package.json changes
        if !watch_dirs.contains(&entry.source_dir) {
            watcher
                .watch(&entry.source_dir, RecursiveMode::NonRecursive)
                .map_err(|e| format!("failed to watch {}: {e}", entry.source_dir.display()))?;
        }
    }

    // Track tarball hashes so an edit that does not change packed output is
    // not reported as a change.
    let mut last_hashes: HashMap<String, u64> = HashMap::new();
    for entry in selected {
        if let Ok(tarball) = store::load_tarball(&entry.name) {
            last_hashes.insert(entry.name.clone(), hash_bytes(&tarball));
        }
    }

    let batch_window = Duration::from_secs(5);
    let stabilize_delay = Duration::from_secs(1);
    let poll_interval = Duration::from_millis(200);

    loop {
        if shutdown.load(Ordering::SeqCst) {
            break;
        }

        let first_event = match rx.recv_timeout(poll_interval) {
            Ok(event) => event,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };

        let batch_start = Instant::now();
        let mut changed_paths: Vec<PathBuf> = first_event.paths;

        // Batch events within window
        loop {
            let remaining = batch_window.saturating_sub(batch_start.elapsed());
            if remaining.is_zero() {
                break;
            }
            match rx.recv_timeout(remaining) {
                Ok(event) => changed_paths.extend(event.paths),
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
            }
        }

        // Stabilization wait
        std::thread::sleep(stabilize_delay);
        while rx.try_recv().is_ok() {}

        let changed_packages: Vec<&&store::StoreEntry> = selected
            .iter()
            .filter(|entry| {
                changed_paths.iter().any(|p| {
                    p.starts_with(&entry.source_dir) && !is_ignored_path(p, &entry.source_dir)
                })
            })
            .collect();

        if changed_packages.is_empty() {
            continue;
        }

        let pkg_list = changed_packages
            .iter()
            .map(|e| style(&e.name).cyan().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let _ = cliclack::log::warning(format!("Change detected in: {pkg_list}"));

        let spinner = cliclack::spinner();
        spinner.start("Repacking...");

        let mut repacked: Vec<String> = Vec::new();
        for entry in changed_packages {
            match repack(entry, &mut last_hashes) {
                Ok(true) => repacked.push(entry.name.clone()),
                Ok(false) => {}
                Err(e) => {
                    let _ = cliclack::log::warning(e);
                }
            }
        }

        if repacked.is_empty() {
            spinner.stop(format!("{}", style("No content changes").dim()));
            continue;
        }

        spinner.stop(format!("Repacked {}", style(repacked.join(", ")).cyan()));
        on_repack(&repacked);
    }

    Ok(())
}

/// Repack one package into the store. Returns whether the packed bytes
/// differed from what was already there.
fn repack(
    entry: &store::StoreEntry,
    last_hashes: &mut std::collections::HashMap<String, u64>,
) -> Result<bool, String> {
    let pkg_json_path = entry.source_dir.join("package.json");
    let raw = std::fs::read_to_string(&pkg_json_path)
        .map_err(|e| format!("could not read {}: {e}", pkg_json_path.display()))?;
    let pkg_json: pack::PublishPackageJson = serde_json::from_str(&raw)
        .map_err(|e| format!("could not parse {}: {e}", pkg_json_path.display()))?;

    let tarball = pack::pack(&entry.source_dir, &pkg_json)
        .map_err(|e| format!("failed to pack {}: {e}", entry.name))?;

    let hash = hash_bytes(&tarball);
    if last_hashes.get(&entry.name) == Some(&hash) {
        return Ok(false);
    }
    last_hashes.insert(entry.name.clone(), hash);

    let version = pkg_json.version.as_deref().unwrap_or("0.0.0");
    store::save(
        &entry.name,
        version,
        &entry.source_dir,
        &tarball,
        &pkg_json.dependencies(),
    )?;

    Ok(true)
}

/// Determine which directories to watch for a package.
/// If the package.json has a `files` field, watch only those directories.
/// Otherwise, watch the entire package root.
pub(crate) fn resolve_watch_dirs(pkg_dir: &Path) -> Vec<PathBuf> {
    let pkg_json_path = pkg_dir.join("package.json");
    let Ok(raw) = std::fs::read_to_string(&pkg_json_path) else {
        return vec![pkg_dir.to_path_buf()];
    };
    let Ok(pkg_json) = serde_json::from_str::<pack::PublishPackageJson>(&raw) else {
        return vec![pkg_dir.to_path_buf()];
    };

    let Some(files) = pkg_json.files_list() else {
        return vec![pkg_dir.to_path_buf()];
    };

    let mut dirs = Vec::new();

    for pattern in files {
        if pattern.starts_with('!') {
            continue;
        }

        let clean: &str = pattern
            .trim_start_matches('/')
            .trim_end_matches("/**")
            .trim_end_matches("/*")
            .trim_end_matches('/');

        let base = if let Some(pos) = clean.find('*') {
            clean[..pos].trim_end_matches('/')
        } else {
            clean
        };

        if base.is_empty() {
            dirs.push(pkg_dir.to_path_buf());
            continue;
        }

        let abs = pkg_dir.join(base);
        if abs.is_dir() {
            dirs.push(abs);
        } else if abs.is_file() {
            if let Some(parent) = abs.parent() {
                dirs.push(parent.to_path_buf());
            }
        }
    }

    // Fallback: if no directories resolved, watch the package root
    if dirs.is_empty() {
        return vec![pkg_dir.to_path_buf()];
    }

    dirs.sort();
    dirs.dedup();

    // Remove subdirectories if a parent is already watched
    let mut filtered = Vec::new();
    for dir in &dirs {
        let dominated = filtered
            .iter()
            .any(|parent: &PathBuf| dir.starts_with(parent) && dir != parent);
        if !dominated {
            filtered.retain(|existing: &PathBuf| !existing.starts_with(dir) || existing == dir);
            filtered.push(dir.clone());
        }
    }

    filtered
}

fn hash_bytes(data: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    data.hash(&mut hasher);
    hasher.finish()
}

fn is_ignored_path(path: &Path, source_root: &Path) -> bool {
    let rel = path.strip_prefix(source_root).unwrap_or(path);
    let rel_str = rel.to_string_lossy();
    rel_str.starts_with("node_modules")
        || rel_str.starts_with(".git")
        || rel_str.starts_with("target")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_pkg(tmp: &Path, pkg_json: &str, dirs: &[&str], files: &[&str]) {
        fs::create_dir_all(tmp).unwrap();
        fs::write(tmp.join("package.json"), pkg_json).unwrap();
        for d in dirs {
            fs::create_dir_all(tmp.join(d)).unwrap();
        }
        for f in files {
            let path = tmp.join(f);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, "").unwrap();
        }
    }

    #[test]
    fn no_files_field_watches_entire_root() {
        let tmp = std::env::temp_dir().join("lpm_test_watch_no_files");
        let _ = fs::remove_dir_all(&tmp);
        setup_pkg(&tmp, r#"{"name": "pkg"}"#, &[], &[]);

        let dirs = resolve_watch_dirs(&tmp);
        assert_eq!(dirs, vec![tmp.clone()]);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn files_field_watches_only_listed_dirs() {
        let tmp = std::env::temp_dir().join("lpm_test_watch_files_field");
        let _ = fs::remove_dir_all(&tmp);
        setup_pkg(
            &tmp,
            r#"{"name": "pkg", "files": ["dist", "lib"]}"#,
            &["dist", "lib", "src", "tests"],
            &[],
        );

        let dirs = resolve_watch_dirs(&tmp);
        assert!(dirs.contains(&tmp.join("dist")));
        assert!(dirs.contains(&tmp.join("lib")));
        assert!(!dirs.contains(&tmp.clone()), "should not watch entire root");
        assert!(!dirs.contains(&tmp.join("src")));
        assert!(!dirs.contains(&tmp.join("tests")));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn subdirectory_dedup_works() {
        let tmp = std::env::temp_dir().join("lpm_test_watch_dedup");
        let _ = fs::remove_dir_all(&tmp);
        setup_pkg(
            &tmp,
            r#"{"name": "pkg", "files": ["dist", "dist/sub"]}"#,
            &["dist", "dist/sub"],
            &[],
        );

        let dirs = resolve_watch_dirs(&tmp);
        // dist/sub is a subdirectory of dist, so only dist should be watched
        assert!(dirs.contains(&tmp.join("dist")));
        assert!(!dirs.contains(&tmp.join("dist/sub")));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn wildcard_pattern_resolves_to_root() {
        let tmp = std::env::temp_dir().join("lpm_test_watch_wildcard");
        let _ = fs::remove_dir_all(&tmp);
        setup_pkg(
            &tmp,
            r#"{"name": "pkg", "files": ["*.js"]}"#,
            &[],
            &["index.js"],
        );

        let dirs = resolve_watch_dirs(&tmp);
        // *.js has empty base, so it should resolve to pkg_dir
        assert!(dirs.contains(&tmp.clone()));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn fallback_when_no_patterns_resolve() {
        let tmp = std::env::temp_dir().join("lpm_test_watch_fallback");
        let _ = fs::remove_dir_all(&tmp);
        setup_pkg(
            &tmp,
            r#"{"name": "pkg", "files": ["nonexistent"]}"#,
            &[],
            &[],
        );

        let dirs = resolve_watch_dirs(&tmp);
        // No patterns matched anything, should fallback to package root
        assert_eq!(dirs, vec![tmp.clone()]);

        let _ = fs::remove_dir_all(&tmp);
    }
}
