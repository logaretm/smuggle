use console::style;
use std::path::{Path, PathBuf};

use crate::{backup, pack, pm, store};

/// A resolved target in node_modules to override with a local package.
pub struct OverrideTarget {
    pub name: String,
    pub target_dir: PathBuf,
}

/// Watch source directories for changes and re-extract tarballs when content changes.
pub fn watch_and_reinstall(
    selected: &[&store::StoreEntry],
    targets: &[OverrideTarget],
    consumer_dir: &Path,
    workspace_pkg_dirs: &[PathBuf],
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
    }

    // Track tarball hashes to avoid unnecessary restarts
    let mut last_hashes: HashMap<String, u64> = HashMap::new();
    for entry in selected {
        if let Ok(tarball) = store::load_tarball(&entry.name) {
            last_hashes.insert(entry.name.clone(), hash_bytes(&tarball));
        }
    }

    let batch_window = Duration::from_secs(5);
    let stabilize_delay = Duration::from_secs(1);

    loop {
        let Ok(first_event) = rx.recv() else {
            break;
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

        // Determine which packages changed
        let mut changed_packages: Vec<String> = Vec::new();
        for entry in selected {
            let source = &entry.source_dir;
            if changed_paths
                .iter()
                .any(|p| p.starts_with(source) && !is_ignored_path(p, source))
            {
                changed_packages.push(entry.name.clone());
            }
        }

        if changed_packages.is_empty() {
            continue;
        }

        let pkg_list = changed_packages
            .iter()
            .map(|p| style(p).cyan().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let _ = cliclack::log::warning(format!("Change detected in: {pkg_list}"));

        let spinner = cliclack::spinner();

        // Re-pack and check if content actually changed
        spinner.start("Packing changed packages...");
        let mut to_extract: Vec<(String, Vec<u8>, PathBuf)> = Vec::new();
        for pkg_name in &changed_packages {
            let entry = selected.iter().find(|e| &e.name == pkg_name).unwrap();
            let Some(target) = targets.iter().find(|t| t.name == *pkg_name) else {
                continue;
            };

            let pkg_json_path = entry.source_dir.join("package.json");
            let Ok(raw) = std::fs::read_to_string(&pkg_json_path) else {
                let _ =
                    cliclack::log::warning(format!("Could not read {}", pkg_json_path.display()));
                continue;
            };
            let Ok(pkg_json) = serde_json::from_str::<pack::PublishPackageJson>(&raw) else {
                let _ =
                    cliclack::log::warning(format!("Could not parse {}", pkg_json_path.display()));
                continue;
            };

            match pack::pack(&entry.source_dir, &pkg_json) {
                Ok(tarball) => {
                    let new_hash = hash_bytes(&tarball);
                    let old_hash = last_hashes.get(pkg_name).copied();

                    if old_hash == Some(new_hash) {
                        continue;
                    }

                    last_hashes.insert(pkg_name.clone(), new_hash);
                    let version = pkg_json.version.as_deref().unwrap_or("0.0.0");
                    let _ = store::save(
                        &entry.name,
                        version,
                        &entry.source_dir,
                        &tarball,
                        &pkg_json.dependencies(),
                    );

                    to_extract.push((pkg_name.clone(), tarball, target.target_dir.clone()));
                }
                Err(e) => {
                    let _ = cliclack::log::warning(format!("Failed to pack {}: {e}", entry.name));
                }
            }
        }

        // Transactional extraction: backup, extract all, rollback on failure
        let mut actually_changed: Vec<String> = Vec::new();
        if !to_extract.is_empty() {
            let backup_pairs: Vec<(String, PathBuf)> = to_extract
                .iter()
                .map(|(name, _, target)| (name.clone(), target.clone()))
                .collect();

            let backup_base = match backup::backup_targets(&backup_pairs) {
                Ok(b) => b,
                Err(e) => {
                    spinner.stop(format!("{} backup failed: {e}", style("✗").red()));
                    continue;
                }
            };

            spinner.start(format!("Smuggling {} package(s)...", to_extract.len()));
            let mut failed = false;
            for (pkg_name, tarball, target_dir) in &to_extract {
                if let Err(e) = pack::extract_tarball_to(tarball, target_dir) {
                    spinner.stop(format!(
                        "{} extraction failed: {e} — rolling back",
                        style("✗").red()
                    ));
                    backup::restore_all(&backup_base, &backup_pairs);
                    failed = true;
                    break;
                }
                actually_changed.push(pkg_name.clone());
            }

            if !failed {
                // Extraction succeeded, remove backup
                let _ = std::fs::remove_dir_all(&backup_base);
            } else {
                actually_changed.clear();
            }
        }

        if actually_changed.is_empty() {
            spinner.stop(format!("{}", style("No content changes").dim()));
            continue;
        }

        // Clear bundler caches and trigger vite restart
        let extra: Vec<&Path> = workspace_pkg_dirs.iter().map(|p| p.as_path()).collect();
        pm::clear_bundler_caches(consumer_dir, &extra);
        pm::touch_vite_configs(consumer_dir, workspace_pkg_dirs);

        spinner.stop(format!(
            "Smuggled {}",
            style(actually_changed.join(", ")).cyan()
        ));
    }

    Ok(())
}

/// Determine which directories to watch for a package.
/// If the package.json has a `files` field, watch only those directories.
/// Otherwise, watch the entire package root.
fn resolve_watch_dirs(pkg_dir: &Path) -> Vec<PathBuf> {
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
    dirs.push(pkg_dir.to_path_buf());

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
