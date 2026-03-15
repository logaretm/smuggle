use console::style;
use std::path::{Path, PathBuf};

use crate::{backup, pack, pm, store, watch, workspace};

pub fn cmd_install(consumer_dir: &Path, select_all: bool, strict: bool) -> Result<(), String> {
    let consumer_dir = consumer_dir
        .canonicalize()
        .map_err(|e| format!("invalid path: {e}"))?;

    // Detect workspace (pnpm or yarn)
    if let Some(ws) = workspace::detect_workspace(&consumer_dir) {
        return cmd_install_workspace(&consumer_dir, ws, select_all, strict);
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

    run_install_flow(&consumer_dir, &selected, &[], strict)
}

fn cmd_install_workspace(
    root: &Path,
    ws: workspace::DetectedWorkspace,
    select_all: bool,
    strict: bool,
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
    run_install_flow(root, &selected, &ws_dirs, strict)
}

/// Shared install flow: expand deps, overwrite in node_modules, watch for changes.
/// No registry, no .npmrc, no lockfile changes — just direct file replacement.
fn run_install_flow(
    install_dir: &Path,
    selected: &[&store::StoreEntry],
    workspace_pkg_dirs: &[PathBuf],
    strict: bool,
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

    // Check for version mismatches before any changes
    let mismatches = check_version_mismatches(&all_refs, &targets);
    if !mismatches.is_empty() {
        for msg in &mismatches {
            let _ = cliclack::log::warning(msg.clone());
        }
        if strict {
            return Err("version mismatch detected (--strict mode)".into());
        }
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

    // Extract tarballs into node_modules
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
    watch::watch_and_reinstall(&all_refs, &targets, install_dir, workspace_pkg_dirs)?;

    // Cleanup on normal exit — restore originals
    let restore_spinner = cliclack::spinner();
    restore_spinner.start("Restoring originals...");
    backup::restore_all(&backup_base, &backup_pairs);
    restore_spinner.stop("Restored originals");

    let _ = cliclack::outro("Done");

    Ok(())
}

/// Resolve each package to its real location in node_modules.
fn resolve_targets(
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
            let pkg_path = nm_dir.join(&entry.name);
            if !pkg_path.exists() {
                continue;
            }
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
        if !found {
            let _ = cliclack::log::warning(format!(
                "{} not found in node_modules — skipping",
                style(&entry.name).cyan(),
            ));
        }
    }

    Ok(targets)
}

/// Expand the selected set to include any registered packages that are
/// transitive dependencies of the selected packages.
fn expand_with_registered_deps(
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

/// Read the version from an installed package's package.json in node_modules.
fn read_installed_version(target_dir: &Path) -> Option<String> {
    let pkg_json_path = target_dir.join("package.json");
    let raw = std::fs::read_to_string(&pkg_json_path).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&raw).ok()?;
    parsed["version"].as_str().map(|s| s.to_string())
}

/// Compare local (store) versions against installed (node_modules) versions.
/// Returns a list of warning messages for each mismatch.
fn check_version_mismatches(
    entries: &[&store::StoreEntry],
    targets: &[watch::OverrideTarget],
) -> Vec<String> {
    let mut warnings = Vec::new();
    for entry in entries {
        let Some(target) = targets.iter().find(|t| t.name == entry.name) else {
            continue;
        };
        let Some(installed_version) = read_installed_version(&target.target_dir) else {
            continue;
        };
        if entry.version != installed_version {
            warnings.push(format!(
                "\u{26a0} {}: local version {} differs from installed {}",
                entry.name, entry.version, installed_version
            ));
        }
    }
    warnings
}
