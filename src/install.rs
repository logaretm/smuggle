use console::style;
use std::path::{Path, PathBuf};

use crate::{backup, pack, pm, store, watch, workspace};

pub fn cmd_add(consumer_dir: &Path, name: &str, dev: bool, once: bool) -> Result<(), String> {
    let consumer_dir = consumer_dir
        .canonicalize()
        .map_err(|e| format!("invalid path: {e}"))?;

    let pkg_json_path = consumer_dir.join("package.json");
    if !pkg_json_path.exists() {
        return Err("no package.json found in consumer directory".into());
    }

    let _ = cliclack::intro(style(" smuggle add ").on_cyan().black());

    // Look up the package in the store
    let registered = store::list();
    let entry = registered.iter().find(|e| e.name == name).ok_or_else(|| {
        format!(
            "{} is not registered. Run {} in the package directory first.",
            style(name).cyan(),
            style("smuggle publish").cyan(),
        )
    })?;

    // Check if the package is already in dependencies
    let consumer_pkg: pack::ConsumerPackageJson =
        serde_json::from_str(&std::fs::read_to_string(&pkg_json_path).map_err(|e| e.to_string())?)
            .map_err(|e| format!("failed to parse consumer package.json: {e}"))?;

    if consumer_pkg.all_dependency_names().contains(&entry.name) {
        return Err(format!(
            "{} is already in your dependencies. Use {} instead.",
            style(name).cyan(),
            style("smuggle install").cyan(),
        ));
    }

    // Add to package.json
    let dep_key = if dev {
        "devDependencies"
    } else {
        "dependencies"
    };
    let version_spec = format!("^{}", entry.version);

    let original_pkg_json = std::fs::read_to_string(&pkg_json_path).map_err(|e| e.to_string())?;
    let mut pkg_value: serde_json::Value =
        serde_json::from_str(&original_pkg_json).map_err(|e| e.to_string())?;

    if pkg_value.get(dep_key).is_none() {
        pkg_value[dep_key] = serde_json::json!({});
    }
    pkg_value[dep_key][name] = serde_json::Value::String(version_spec.clone());

    let new_pkg_json = serde_json::to_string_pretty(&pkg_value).map_err(|e| e.to_string())?;
    std::fs::write(&pkg_json_path, format!("{new_pkg_json}\n")).map_err(|e| e.to_string())?;

    cliclack::log::info(format!(
        "Added {} to {} as {}",
        style(name).cyan(),
        style(dep_key).dim(),
        style(&version_spec).dim(),
    ))
    .map_err(|e| e.to_string())?;

    // Ensure node_modules exists
    let nm_dir = consumer_dir.join("node_modules");
    std::fs::create_dir_all(&nm_dir).map_err(|e| format!("failed to create node_modules: {e}"))?;

    // Create target directory and extract tarball
    let target_dir = nm_dir.join(name);
    std::fs::create_dir_all(&target_dir)
        .map_err(|e| format!("failed to create {}: {e}", target_dir.display()))?;

    let extract_spinner = cliclack::spinner();
    extract_spinner.start(format!("Smuggling {}...", style(name).cyan()));

    let tarball = store::load_tarball(name)?;
    pack::extract_tarball_to(&tarball, &target_dir)?;

    extract_spinner.stop(format!(
        "Smuggled {} into node_modules",
        style(name).green()
    ));

    if once {
        let _ = cliclack::outro("Done (package.json was modified — remember to revert if needed)");
        return Ok(());
    }

    // Set up cleanup on ctrl-c
    let added_dirs = vec![target_dir.clone()];
    backup::setup_ctrlc_add_cleanup(added_dirs, pkg_json_path.clone(), original_pkg_json.clone());

    // Clear bundler caches
    pm::clear_bundler_caches(&consumer_dir, &[]);
    pm::touch_vite_configs(&consumer_dir, &[]);

    cliclack::log::success(format!(
        "Watching for changes... {}",
        style("(ctrl-c to stop and revert)").dim()
    ))
    .map_err(|e| e.to_string())?;

    // Watch for changes
    let targets = vec![watch::OverrideTarget {
        name: entry.name.clone(),
        target_dir,
    }];
    let entry_refs: Vec<&store::StoreEntry> = vec![entry];
    watch::watch_and_reinstall(
        &entry_refs,
        &targets,
        &mut watch::PostSwapAction::ClearCachesAndTouch {
            consumer_dir: &consumer_dir,
            workspace_pkg_dirs: &[],
        },
    )?;

    // Cleanup on normal exit
    let restore_spinner = cliclack::spinner();
    restore_spinner.start("Reverting package.json and removing added packages...");

    for target in &targets {
        let _ = std::fs::remove_dir_all(&target.target_dir);
        if let Some(parent) = target.target_dir.parent() {
            let _ = std::fs::remove_dir(parent); // only removes if empty (scoped packages)
        }
    }
    std::fs::write(&pkg_json_path, &original_pkg_json).map_err(|e| e.to_string())?;

    restore_spinner.stop("Reverted package.json and removed added packages");

    let _ = cliclack::outro("Done");

    Ok(())
}

pub fn cmd_install(consumer_dir: &Path, select_all: bool, once: bool) -> Result<(), String> {
    let consumer_dir = consumer_dir
        .canonicalize()
        .map_err(|e| format!("invalid path: {e}"))?;

    // Detect workspace (pnpm or yarn)
    if let Some(ws) = workspace::detect_workspace(&consumer_dir) {
        return cmd_install_workspace(&consumer_dir, ws, select_all, once);
    }

    let pkg_json_path = consumer_dir.join("package.json");
    if !pkg_json_path.exists() {
        return Err("no package.json found in consumer directory".into());
    }

    let _ = cliclack::intro(style(" smuggle install ").on_cyan().black());

    let consumer_pkg: pack::ConsumerPackageJson =
        serde_json::from_str(&std::fs::read_to_string(&pkg_json_path).map_err(|e| e.to_string())?)
            .map_err(|e| format!("failed to parse consumer package.json: {e}"))?;

    let all_deps = consumer_pkg.all_dependency_names();

    let registered = store::list();
    let mut matches: Vec<&store::StoreEntry> = registered
        .iter()
        .filter(|entry| all_deps.contains(&entry.name))
        .collect();

    if matches.is_empty() {
        return Err(
            "no registered packages found in consumer's dependencies. publish some first with `smuggle publish`."
                .into(),
        );
    }

    matches.sort_by(|a, b| a.name.cmp(&b.name));

    let selected: Vec<&store::StoreEntry> = if select_all {
        let list: Vec<String> = matches
            .iter()
            .map(|e| {
                format!(
                    "{} @ {} ({})",
                    style(&e.name).cyan(),
                    e.version,
                    e.source_dir.display()
                )
            })
            .collect();
        cliclack::log::info(format!(
            "Selecting all {} matching package(s)\n{}",
            matches.len(),
            list.join("\n"),
        ))
        .map_err(|e| e.to_string())?;
        matches.clone()
    } else {
        let mut prompt = cliclack::multiselect("Select packages to proxy locally");

        for (i, e) in matches.iter().enumerate() {
            let label = format!("{} @ {}", e.name, e.version);
            let hint = e.source_dir.display().to_string();
            prompt = prompt.item(i, label, hint);
        }

        let all_indices: Vec<usize> = (0..matches.len()).collect();
        prompt = prompt.initial_values(all_indices);

        let selections: Vec<usize> = prompt
            .interact()
            .map_err(|e| format!("selection cancelled: {e}"))?;

        if selections.is_empty() {
            let _ = cliclack::outro("No packages selected, nothing to do.");
            return Ok(());
        }

        selections.iter().map(|&i| matches[i]).collect()
    };

    run_install_flow(&consumer_dir, &selected, &[], once)
}

fn cmd_install_workspace(
    root: &Path,
    ws: workspace::DetectedWorkspace,
    select_all: bool,
    once: bool,
) -> Result<(), String> {
    let _ = cliclack::intro(style(" smuggle install ").on_cyan().black());

    cliclack::log::info(format!("Detected {} workspace", ws.kind)).map_err(|e| e.to_string())?;

    let workspace_packages = ws.packages;

    let registered = store::list();
    if registered.is_empty() {
        return Err("no registered packages. publish some first with `smuggle publish`.".into());
    }

    // Collect deps from all workspace packages and track which workspace pkg uses which proxied dep
    let mut all_deps = std::collections::HashSet::new();
    let mut workspace_dep_map: Vec<(String, Vec<String>)> = Vec::new();

    for wp in &workspace_packages {
        let pkg_json_path = wp.path.join("package.json");
        let Ok(raw) = std::fs::read_to_string(&pkg_json_path) else {
            continue;
        };
        let Ok(consumer_pkg) = serde_json::from_str::<pack::ConsumerPackageJson>(&raw) else {
            continue;
        };
        let deps = consumer_pkg.all_dependency_names();
        let matched: Vec<String> = registered
            .iter()
            .filter(|e| deps.contains(&e.name))
            .map(|e| e.name.clone())
            .collect();

        if !matched.is_empty() {
            all_deps.extend(matched.clone());
            workspace_dep_map.push((wp.name.clone(), matched));
        }
    }

    let mut matches: Vec<&store::StoreEntry> = registered
        .iter()
        .filter(|entry| all_deps.contains(&entry.name))
        .collect();

    if matches.is_empty() {
        return Err(
            "no registered packages found in any workspace package's dependencies. publish some first with `smuggle publish`."
                .into(),
        );
    }

    matches.sort_by(|a, b| a.name.cmp(&b.name));

    // Show which workspace packages use which proxied deps
    let selected: Vec<&store::StoreEntry> = if select_all {
        let mut lines = Vec::new();
        for (wp_name, dep_names) in &workspace_dep_map {
            lines.push(format!("{}:", style(wp_name).bold()));
            for dep in dep_names {
                lines.push(format!("  {}", style(dep).cyan()));
            }
        }
        cliclack::log::info(format!(
            "Found {} proxied package(s) across {} workspace package(s)\n{}",
            matches.len(),
            workspace_dep_map.len(),
            lines.join("\n"),
        ))
        .map_err(|e| e.to_string())?;
        matches.clone()
    } else {
        let mut prompt = cliclack::multiselect("Select packages to proxy locally");

        for (i, e) in matches.iter().enumerate() {
            let label = format!("{} @ {}", e.name, e.version);
            let hint = e.source_dir.display().to_string();
            prompt = prompt.item(i, label, hint);
        }

        let all_indices: Vec<usize> = (0..matches.len()).collect();
        prompt = prompt.initial_values(all_indices);

        let selections: Vec<usize> = prompt
            .interact()
            .map_err(|e| format!("selection cancelled: {e}"))?;

        if selections.is_empty() {
            let _ = cliclack::outro("No packages selected, nothing to do.");
            return Ok(());
        }

        selections.iter().map(|&i| matches[i]).collect()
    };

    let ws_dirs: Vec<PathBuf> = workspace_packages
        .iter()
        .map(|wp| wp.path.clone())
        .collect();
    run_install_flow(root, &selected, &ws_dirs, once)
}

/// Shared install flow: expand deps, overwrite in node_modules, watch for changes.
/// No registry, no .npmrc, no lockfile changes — just direct file replacement.
fn run_install_flow(
    install_dir: &Path,
    selected: &[&store::StoreEntry],
    workspace_pkg_dirs: &[PathBuf],
    once: bool,
) -> Result<(), String> {
    // Expand: include registered transitive dependencies
    let registered = store::list();
    let all_entries = expand_with_registered_deps(selected, &registered);
    let all_refs: Vec<&store::StoreEntry> = all_entries.iter().collect();

    if all_refs.len() > selected.len() {
        let extra: Vec<&str> = all_refs
            .iter()
            .filter(|e| !selected.iter().any(|s| s.name == e.name))
            .map(|e| e.name.as_str())
            .collect();
        cliclack::log::info(format!(
            "Also proxying {} transitive dep(s): {}",
            extra.len(),
            extra
                .iter()
                .map(|n| style(n).cyan().to_string())
                .collect::<Vec<_>>()
                .join(", "),
        ))
        .map_err(|e| e.to_string())?;
    }

    // Resolve each package's location in node_modules
    let targets = resolve_targets(&all_refs, install_dir, workspace_pkg_dirs)?;

    if targets.is_empty() {
        return Err("none of the selected packages are installed in node_modules".into());
    }

    if once {
        // One-shot mode: swap and exit, no backup/restore, no cache clearing, no watch
        let extract_spinner = cliclack::spinner();
        extract_spinner.start(format!("Smuggling {} package(s)...", targets.len()));

        for target in &targets {
            let tarball = store::load_tarball(&target.name)?;
            pack::extract_tarball_to(&tarball, &target.target_dir)?;
        }

        extract_spinner.stop(format!(
            "Smuggled {} package(s) into node_modules",
            style(targets.len()).green()
        ));

        let _ = cliclack::outro("Done");

        return Ok(());
    }

    // Backup original directories
    let backup_pairs: Vec<(String, PathBuf)> = targets
        .iter()
        .map(|t| (t.name.clone(), t.target_dir.clone()))
        .collect();

    let spinner = cliclack::spinner();
    spinner.start("Backing up originals...");
    let backup_base = backup::backup_targets(&backup_pairs)?;
    spinner.stop("Originals backed up");

    // Set up cleanup on ctrl-c
    let cleanup_targets: Vec<(PathBuf, PathBuf)> = targets
        .iter()
        .map(|t| {
            let backup_name = t.name.replace('/', "__");
            (backup_base.join(&backup_name), t.target_dir.clone())
        })
        .collect();
    backup::setup_ctrlc_restore(backup_base.clone(), cleanup_targets);

    // Extract tarballs into node_modules (transactional: rollback all on failure)
    let extract_spinner = cliclack::spinner();
    extract_spinner.start(format!("Smuggling {} package(s)...", targets.len()));

    if let Err(e) = extract_all(&targets) {
        extract_spinner.stop(format!("{} extraction failed: {e}", style("✗").red()));
        let restore_spinner = cliclack::spinner();
        restore_spinner.start("Rolling back all packages...");
        backup::restore_all(&backup_base, &backup_pairs);
        restore_spinner.stop("Rolled back all packages to originals");
        return Err(format!("install aborted: {e}"));
    }

    extract_spinner.stop(format!(
        "Smuggled {} package(s) into node_modules",
        style(targets.len()).green()
    ));

    // Clear bundler caches and trigger vite restart
    let extra: Vec<&Path> = workspace_pkg_dirs.iter().map(|p| p.as_path()).collect();
    pm::clear_bundler_caches(install_dir, &extra);
    pm::touch_vite_configs(install_dir, workspace_pkg_dirs);

    cliclack::log::success(format!(
        "Watching for changes... {}",
        style("(ctrl-c to stop)").dim()
    ))
    .map_err(|e| e.to_string())?;

    // Watch for changes
    watch::watch_and_reinstall(
        &all_refs,
        &targets,
        &mut watch::PostSwapAction::ClearCachesAndTouch {
            consumer_dir: install_dir,
            workspace_pkg_dirs,
        },
    )?;

    // Cleanup on normal exit — restore originals
    let restore_spinner = cliclack::spinner();
    restore_spinner.start("Restoring originals...");
    backup::restore_all(&backup_base, &backup_pairs);
    restore_spinner.stop("Restored originals");

    let _ = cliclack::outro("Done");

    Ok(())
}

/// Resolve each package to its real location in node_modules.
pub fn resolve_targets(
    entries: &[&store::StoreEntry],
    install_dir: &Path,
    workspace_pkg_dirs: &[PathBuf],
) -> Result<Vec<watch::OverrideTarget>, String> {
    let mut nm_dirs: Vec<PathBuf> = vec![install_dir.join("node_modules")];
    for ws_dir in workspace_pkg_dirs {
        let nm = ws_dir.join("node_modules");
        if nm.exists() {
            nm_dirs.push(nm);
        }
    }

    if !nm_dirs.iter().any(|d| d.exists()) {
        return Err("node_modules not found — run your package manager's install first".into());
    }

    let mut targets: Vec<watch::OverrideTarget> = Vec::new();
    for entry in entries {
        let mut found = false;
        for nm_dir in &nm_dirs {
            for pkg_path in find_package_in_node_modules(&entry.name, nm_dir) {
                let real_path = pkg_path
                    .canonicalize()
                    .map_err(|e| format!("failed to resolve {}: {e}", pkg_path.display()))?;

                if !targets.iter().any(|t| t.target_dir == real_path) {
                    targets.push(watch::OverrideTarget {
                        name: entry.name.clone(),
                        target_dir: real_path,
                    });
                }
                found = true;
            }
        }
        if !found {
            let _ = cliclack::log::warning(format!(
                "{} not found in node_modules — skipping",
                style(&entry.name).cyan(),
            ));
        }
    }

    Ok(targets)
}

/// Find all locations of a package inside a node_modules directory, checking:
/// 1. Direct: `node_modules/{name}` (works for direct deps in all package managers)
/// 2. pnpm virtual store: `node_modules/.pnpm/{encoded}@*/node_modules/{name}`
///    (collects all matches — multiple versions or peer-dep contexts may exist)
/// 3. Nested: `node_modules/{parent}/node_modules/{name}` (npm non-hoisted deps)
fn find_package_in_node_modules(name: &str, nm_dir: &Path) -> Vec<PathBuf> {
    let mut results = Vec::new();

    // 1. Direct lookup
    let direct = nm_dir.join(name);
    if direct.exists() {
        results.push(direct);
        return results;
    }

    // 2. pnpm virtual store — collect all matching versions/peer-dep contexts
    let pnpm_dir = nm_dir.join(".pnpm");
    if pnpm_dir.exists() {
        // pnpm encodes scoped packages: @scope/name → @scope+name
        let encoded_prefix = name.replace('/', "+");
        if let Ok(entries) = std::fs::read_dir(&pnpm_dir) {
            for entry in entries.flatten() {
                let dir_name = entry.file_name();
                let dir_name_str = dir_name.to_string_lossy();
                if dir_name_str.starts_with(&format!("{encoded_prefix}@")) {
                    let candidate = entry.path().join("node_modules").join(name);
                    if candidate.exists() {
                        results.push(candidate);
                    }
                }
            }
        }
        if !results.is_empty() {
            return results;
        }
    }

    // 3. Nested node_modules (npm non-hoisted transitive deps)
    if let Ok(entries) = std::fs::read_dir(nm_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let entry_name = entry.file_name();
            let entry_str = entry_name.to_string_lossy();

            // Skip hidden dirs (.pnpm, .cache, etc.)
            if entry_str.starts_with('.') {
                continue;
            }

            // Handle scoped packages: check inside @scope/ dirs
            if entry_str.starts_with('@') && path.is_dir() {
                if let Ok(scope_entries) = std::fs::read_dir(&path) {
                    for scope_entry in scope_entries.flatten() {
                        let nested = scope_entry.path().join("node_modules").join(name);
                        if nested.exists() {
                            results.push(nested);
                        }
                    }
                }
                continue;
            }

            if path.is_dir() {
                let nested = path.join("node_modules").join(name);
                if nested.exists() {
                    results.push(nested);
                }
            }
        }
    }

    results
}

/// Extract all tarballs into their targets. Returns an error on the first failure.
fn extract_all(targets: &[watch::OverrideTarget]) -> Result<(), String> {
    for target in targets {
        let tarball = store::load_tarball(&target.name)?;
        pack::extract_tarball_to(&tarball, &target.target_dir)?;
    }
    Ok(())
}

/// Expand the selected set to include any registered packages that are
/// transitive dependencies of the selected packages.
pub fn expand_with_registered_deps(
    selected: &[&store::StoreEntry],
    registered: &[store::StoreEntry],
) -> Vec<store::StoreEntry> {
    let registered_names: std::collections::HashSet<String> =
        registered.iter().map(|e| e.name.clone()).collect();

    let mut included: std::collections::HashSet<String> =
        selected.iter().map(|e| e.name.clone()).collect();

    // BFS to find transitive registered deps
    let mut queue: Vec<String> = included.iter().cloned().collect();
    while let Some(name) = queue.pop() {
        let Some(entry) = registered.iter().find(|e| e.name == name) else {
            continue;
        };
        for dep_name in entry.dependencies.keys() {
            if registered_names.contains(dep_name) && !included.contains(dep_name) {
                included.insert(dep_name.clone());
                queue.push(dep_name.clone());
            }
        }
    }

    let mut result: Vec<store::StoreEntry> = registered
        .iter()
        .filter(|e| included.contains(&e.name))
        .cloned()
        .collect();
    result.sort_by(|a, b| a.name.cmp(&b.name));
    result
}
