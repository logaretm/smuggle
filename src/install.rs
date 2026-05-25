use console::style;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::{backup, ci, pack, pm, store, watch, workspace};

pub fn cmd_install(
    consumer_dir: &Path,
    select_all: bool,
    once: bool,
    ci: bool,
    dev: bool,
    names: &[String],
    summary: &mut ci::SummaryCollector,
) -> Result<(), String> {
    let consumer_dir = consumer_dir
        .canonicalize()
        .map_err(|e| format!("invalid path: {e}"))?;

    let select_all = select_all || ci::is_ci();

    if let Some(ws) = workspace::detect_workspace(&consumer_dir) {
        return cmd_install_workspace(&consumer_dir, ws, select_all, once, ci, dev, names, summary);
    }

    let pkg_json_path = consumer_dir.join("package.json");
    if !pkg_json_path.exists() {
        return Err("no package.json found in consumer directory".into());
    }

    if !ci {
        let _ = cliclack::intro(style(" smuggle install ").on_cyan().black());
    }

    let consumer_pkg: pack::ConsumerPackageJson =
        serde_json::from_str(&std::fs::read_to_string(&pkg_json_path).map_err(|e| e.to_string())?)
            .map_err(|e| format!("failed to parse consumer package.json: {e}"))?;

    let all_deps = consumer_pkg.all_dependency_names();
    let registered = store::list();

    if !names.is_empty() {
        let mut entries: Vec<&store::StoreEntry> = Vec::with_capacity(names.len());
        for name in names {
            let entry = registered.iter().find(|e| e.name == *name).ok_or_else(|| {
                format!(
                    "{} is not registered. Run {} in the package directory first.",
                    style(name).cyan(),
                    style("smuggle publish").cyan(),
                )
            })?;

            let tarball_path = store::tarball_path(name);
            if !tarball_path.exists() {
                return Err(format!(
                    "tarball for {} is missing from the store — try `smuggle publish` again",
                    style(name).cyan(),
                ));
            }

            entries.push(entry);
        }

        let new_names: Vec<&str> = names
            .iter()
            .filter(|n| !all_deps.contains(&**n))
            .map(|n| n.as_str())
            .collect();

        return run_install_flow(
            &consumer_dir,
            &entries,
            &[],
            once,
            ci,
            dev,
            &new_names,
            summary,
        );
    }

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
        if !ci {
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
        }
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
            if !ci {
                let _ = cliclack::outro("No packages selected, nothing to do.");
            }
            return Ok(());
        }

        selections.iter().map(|&i| matches[i]).collect()
    };

    run_install_flow(&consumer_dir, &selected, &[], once, ci, dev, &[], summary)
}

#[allow(clippy::too_many_arguments)]
fn cmd_install_workspace(
    root: &Path,
    ws: workspace::DetectedWorkspace,
    select_all: bool,
    once: bool,
    ci: bool,
    dev: bool,
    names: &[String],
    summary: &mut ci::SummaryCollector,
) -> Result<(), String> {
    if !ci {
        let _ = cliclack::intro(style(" smuggle install ").on_cyan().black());
        cliclack::log::info(format!("Detected {} workspace", ws.kind))
            .map_err(|e| e.to_string())?;
    }

    let workspace_packages = ws.packages;

    let registered = store::list();
    if registered.is_empty() {
        return Err("no registered packages. publish some first with `smuggle publish`.".into());
    }

    if !names.is_empty() {
        let mut entries: Vec<&store::StoreEntry> = Vec::with_capacity(names.len());
        for name in names {
            let entry = registered.iter().find(|e| e.name == *name).ok_or_else(|| {
                format!(
                    "{} is not registered. Run {} in the package directory first.",
                    style(name).cyan(),
                    style("smuggle publish").cyan(),
                )
            })?;

            let tarball_path = store::tarball_path(name);
            if !tarball_path.exists() {
                return Err(format!(
                    "tarball for {} is missing from the store — try `smuggle publish` again",
                    style(name).cyan(),
                ));
            }

            entries.push(entry);
        }

        let mut all_deps = std::collections::HashSet::new();
        for wp in &workspace_packages {
            let pkg_json_path = wp.path.join("package.json");
            let Ok(raw) = std::fs::read_to_string(&pkg_json_path) else {
                continue;
            };
            let Ok(consumer_pkg) = serde_json::from_str::<pack::ConsumerPackageJson>(&raw) else {
                continue;
            };
            all_deps.extend(consumer_pkg.all_dependency_names());
        }

        let new_names: Vec<&str> = names
            .iter()
            .filter(|n| !all_deps.contains(&**n))
            .map(|n| n.as_str())
            .collect();

        let ws_dirs: Vec<PathBuf> = workspace_packages
            .iter()
            .map(|wp| wp.path.clone())
            .collect();

        return run_install_flow(root, &entries, &ws_dirs, once, ci, dev, &new_names, summary);
    }

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

    let selected: Vec<&store::StoreEntry> = if select_all {
        if !ci {
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
        }
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
            if !ci {
                let _ = cliclack::outro("No packages selected, nothing to do.");
            }
            return Ok(());
        }

        selections.iter().map(|&i| matches[i]).collect()
    };

    let ws_dirs: Vec<PathBuf> = workspace_packages
        .iter()
        .map(|wp| wp.path.clone())
        .collect();
    run_install_flow(root, &selected, &ws_dirs, once, ci, dev, &[], summary)
}

struct AddCleanupState {
    added_dirs: Vec<PathBuf>,
    pkg_json_path: PathBuf,
    original_pkg_json: String,
    lockfile_snapshots: Vec<backup::FileSnapshot>,
}

impl AddCleanupState {
    fn revert(&self) {
        for dir in &self.added_dirs {
            let _ = std::fs::remove_dir_all(dir);
            if let Some(parent) = dir.parent() {
                let _ = std::fs::remove_dir(parent);
            }
        }
        let _ = std::fs::write(&self.pkg_json_path, &self.original_pkg_json);
        for snap in &self.lockfile_snapshots {
            snap.restore();
        }
    }
}

fn inject_and_install_new_packages(
    install_dir: &Path,
    new_names: &[&str],
    dev: bool,
    ci: bool,
) -> Result<Option<AddCleanupState>, String> {
    if new_names.is_empty() {
        return Ok(None);
    }

    let pkg_json_path = install_dir.join("package.json");
    let original_pkg_json = std::fs::read_to_string(&pkg_json_path).map_err(|e| e.to_string())?;
    let mut pkg_value: serde_json::Value =
        serde_json::from_str(&original_pkg_json).map_err(|e| e.to_string())?;

    let dep_key = if dev {
        "devDependencies"
    } else {
        "dependencies"
    };

    if pkg_value.get(dep_key).is_none() {
        pkg_value[dep_key] = serde_json::json!({});
    }

    for name in new_names {
        let tarball_path = store::tarball_path(name);
        let version_spec = format!("file:{}", tarball_path.display());
        pkg_value[dep_key][name] = serde_json::Value::String(version_spec.clone());

        if !ci {
            let _ = cliclack::log::info(format!(
                "Adding {} to {} as {}",
                style(name).cyan(),
                style(dep_key).dim(),
                style(&version_spec).dim(),
            ));
        }
    }

    let detected_pm = pm::detect_package_manager(install_dir);
    let lockfile_snapshots: Vec<backup::FileSnapshot> =
        pm::lockfile_candidates(install_dir, detected_pm)
            .into_iter()
            .map(backup::FileSnapshot::capture)
            .collect();

    let new_pkg_json = serde_json::to_string_pretty(&pkg_value).map_err(|e| e.to_string())?;
    std::fs::write(&pkg_json_path, format!("{new_pkg_json}\n")).map_err(|e| e.to_string())?;

    let new_display = new_names
        .iter()
        .map(|n| style(n).cyan().to_string())
        .collect::<Vec<_>>()
        .join(", ");

    if !ci {
        let install_spinner = cliclack::spinner();
        install_spinner.start(format!(
            "Running `{} install` to resolve dependencies for {}...",
            detected_pm, new_display,
        ));
        if let Err(e) = pm::run_install(install_dir) {
            install_spinner.stop(format!("{} install failed: {e}", style("✗").red()));
            let _ = std::fs::write(&pkg_json_path, &original_pkg_json);
            for snap in &lockfile_snapshots {
                snap.restore();
            }
            return Err(format!("install aborted: {e}"));
        }
        install_spinner.stop(format!(
            "Installed transitive dependencies for {}",
            new_display,
        ));
    } else {
        if let Err(e) = pm::run_install(install_dir) {
            let _ = std::fs::write(&pkg_json_path, &original_pkg_json);
            for snap in &lockfile_snapshots {
                snap.restore();
            }
            return Err(format!("install aborted: {e}"));
        }
    }

    let nm_dir = install_dir.join("node_modules");
    let added_dirs: Vec<PathBuf> = new_names
        .iter()
        .map(|n| {
            let target_dir = nm_dir.join(n);
            let _ = std::fs::create_dir_all(&target_dir);
            target_dir
        })
        .collect();

    Ok(Some(AddCleanupState {
        added_dirs,
        pkg_json_path,
        original_pkg_json,
        lockfile_snapshots,
    }))
}

fn ensure_deps_installed(install_dir: &Path, ci: bool) -> Result<(), String> {
    let nm_dir = install_dir.join("node_modules");
    if nm_dir.exists()
        && std::fs::read_dir(&nm_dir)
            .map(|mut d| d.next().is_some())
            .unwrap_or(false)
    {
        return Ok(());
    }

    let detected_pm = pm::detect_package_manager(install_dir);

    if !ci {
        let install_spinner = cliclack::spinner();
        install_spinner.start(format!(
            "No node_modules found — running `{} install`...",
            detected_pm,
        ));
        if let Err(e) = pm::run_install(install_dir) {
            install_spinner.stop(format!("{} install failed: {e}", style("✗").red()));
            return Err(format!("install aborted: {e}"));
        }
        install_spinner.stop(format!("Installed dependencies with {}", detected_pm));
    } else {
        pm::run_install(install_dir)?;
    }

    Ok(())
}

/// Shared install flow: expand deps, overwrite in node_modules, watch for changes.
#[allow(clippy::too_many_arguments)]
fn run_install_flow(
    install_dir: &Path,
    selected: &[&store::StoreEntry],
    workspace_pkg_dirs: &[PathBuf],
    once: bool,
    ci: bool,
    dev: bool,
    new_names: &[&str],
    summary: &mut ci::SummaryCollector,
) -> Result<(), String> {
    ensure_deps_installed(install_dir, ci)?;

    let add_cleanup = inject_and_install_new_packages(install_dir, new_names, dev, ci)?;

    let registered = store::list();
    let all_entries = expand_with_registered_deps(selected, &registered);
    let all_refs: Vec<&store::StoreEntry> = all_entries.iter().collect();

    if all_refs.len() > selected.len() && !ci {
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

    let targets = resolve_targets(&all_refs, install_dir, workspace_pkg_dirs, ci, summary)?;

    if targets.is_empty() {
        if let Some(ref cleanup) = add_cleanup {
            cleanup.revert();
        }
        return Err("none of the selected packages are installed in node_modules".into());
    }

    if once {
        if ci {
            let mut installed = 0;
            let mut failed = 0;
            for target in &targets {
                let start = Instant::now();
                let version = all_entries
                    .iter()
                    .find(|e| e.name == target.name)
                    .map(|e| e.version.as_str())
                    .unwrap_or("?");

                match store::load_tarball(&target.name)
                    .and_then(|t| pack::extract_tarball_to(&t, &target.target_dir))
                {
                    Ok(()) => {
                        let ms = ci::elapsed_ms(start);
                        let loc = target.target_dir.display().to_string();
                        ci::emit(&ci::Event::Install {
                            package: &target.name,
                            location: Some(&loc),
                            status: ci::Status::Ok,
                            error: None,
                            duration_ms: Some(ms),
                        });
                        summary.push("install", &target.name, version, "ok", ms);
                        installed += 1;
                    }
                    Err(e) => {
                        let ms = ci::elapsed_ms(start);
                        ci::emit(&ci::Event::Install {
                            package: &target.name,
                            location: None,
                            status: ci::Status::Error,
                            error: Some(&e),
                            duration_ms: Some(ms),
                        });
                        summary.push("install", &target.name, version, "error", ms);
                        failed += 1;
                    }
                }
            }
            ci::emit(&ci::Event::Summary {
                published: 0,
                installed,
                failed,
                duration_ms: Some(ci::elapsed_ms(summary.start)),
            });
            if let Some(ref cleanup) = add_cleanup {
                cleanup.revert();
            }
            if failed > 0 {
                return Err(format!("{failed} package(s) failed to install"));
            }
        } else {
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

            if let Some(ref cleanup) = add_cleanup {
                cleanup.revert();
                let _ = cliclack::log::remark("Reverted package.json and lockfile");
            }

            let _ = cliclack::outro("Done");
        }

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
    backup::setup_ctrlc_combined_cleanup(
        backup_base.clone(),
        cleanup_targets,
        add_cleanup.as_ref().map(|c| backup::AddCleanupInfo {
            added_dirs: c.added_dirs.clone(),
            pkg_json_path: c.pkg_json_path.clone(),
            original_pkg_json: c.original_pkg_json.clone(),
            lockfile_snapshots: c.lockfile_snapshots.clone(),
        }),
    );

    // Extract tarballs into node_modules
    let extract_spinner = cliclack::spinner();
    extract_spinner.start(format!("Smuggling {} package(s)...", targets.len()));

    if let Err(e) = extract_all(&targets) {
        extract_spinner.stop(format!("{} extraction failed: {e}", style("✗").red()));
        let restore_spinner = cliclack::spinner();
        restore_spinner.start("Rolling back all packages...");
        backup::restore_all(&backup_base, &backup_pairs);
        restore_spinner.stop("Rolled back all packages to originals");
        if let Some(ref cleanup) = add_cleanup {
            cleanup.revert();
        }
        return Err(format!("install aborted: {e}"));
    }

    extract_spinner.stop(format!(
        "Smuggled {} package(s) into node_modules",
        style(targets.len()).green()
    ));

    let extra: Vec<&Path> = workspace_pkg_dirs.iter().map(|p| p.as_path()).collect();
    pm::clear_bundler_caches(install_dir, &extra);
    pm::touch_vite_configs(install_dir, workspace_pkg_dirs);

    cliclack::log::success(format!(
        "Watching for changes... {}",
        style("(ctrl-c to stop)").dim()
    ))
    .map_err(|e| e.to_string())?;

    watch::watch_and_reinstall(
        &all_refs,
        &targets,
        &mut watch::PostSwapAction::ClearCachesAndTouch {
            consumer_dir: install_dir,
            workspace_pkg_dirs,
        },
    )?;

    // Cleanup on normal exit
    let restore_spinner = cliclack::spinner();
    if add_cleanup.is_some() {
        restore_spinner.start("Restoring originals and reverting package.json/lockfile...");
    } else {
        restore_spinner.start("Restoring originals...");
    }
    backup::restore_all(&backup_base, &backup_pairs);
    if let Some(ref cleanup) = add_cleanup {
        cleanup.revert();
    }
    if add_cleanup.is_some() {
        restore_spinner.stop("Restored originals and reverted package.json/lockfile");
    } else {
        restore_spinner.stop("Restored originals");
    }

    let _ = cliclack::outro("Done");

    Ok(())
}

/// Resolve each package to its real location in node_modules.
pub fn resolve_targets(
    entries: &[&store::StoreEntry],
    install_dir: &Path,
    workspace_pkg_dirs: &[PathBuf],
    ci: bool,
    summary: &mut ci::SummaryCollector,
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
            if ci {
                ci::emit(&ci::Event::Install {
                    package: &entry.name,
                    location: None,
                    status: ci::Status::Skipped,
                    error: Some("not found in node_modules"),
                    duration_ms: None,
                });
                summary.push("install", &entry.name, &entry.version, "skipped", 0);
            } else {
                let _ = cliclack::log::warning(format!(
                    "{} not found in node_modules — skipping",
                    style(&entry.name).cyan(),
                ));
            }
        }
    }

    Ok(targets)
}

/// Find all locations of a package inside a node_modules directory.
fn find_package_in_node_modules(name: &str, nm_dir: &Path) -> Vec<PathBuf> {
    let mut results = Vec::new();

    let direct = nm_dir.join(name);
    if direct.exists() {
        results.push(direct);
        return results;
    }

    let pnpm_dir = nm_dir.join(".pnpm");
    if pnpm_dir.exists() {
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

    if let Ok(entries) = std::fs::read_dir(nm_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let entry_name = entry.file_name();
            let entry_str = entry_name.to_string_lossy();

            if entry_str.starts_with('.') {
                continue;
            }

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

/// Extract all tarballs into their targets.
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
