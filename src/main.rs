mod pack;
mod pm;
mod store;
mod workspace;

use clap::{Parser, Subcommand};
use console::style;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "smuggle",
    about = "Smuggle local npm packages into your projects — no symlinks, no lockfile pollution"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Path to the consumer project (defaults to current directory)
    #[arg(short, long, global = true)]
    path: Option<PathBuf>,

    /// Select all matching packages without prompting
    #[arg(long, global = true)]
    all: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Pack and register a local package for later use
    Publish {
        /// Path to the package directory (defaults to current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// In a workspace, publish all non-private packages without prompting
        #[arg(long)]
        all: bool,
    },

    /// List all registered local packages
    List,

    /// Remove a registered package
    Unpublish {
        /// Package name (e.g. @scope/my-pkg)
        name: String,
    },

    /// Install registered packages into a consumer project
    Install {
        /// Path to the consumer project (defaults to current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Select all matching packages without prompting
        #[arg(long)]
        all: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Publish { path, all }) => {
            let pkg_dir = path.unwrap_or_else(|| std::env::current_dir().unwrap());
            if let Err(e) = cmd_publish(&pkg_dir, all || cli.all) {
                let _ = cliclack::outro(format!("{}", style(e).red()));
                std::process::exit(1);
            }
        }
        Some(Commands::List) => {
            cmd_list();
        }
        Some(Commands::Unpublish { name }) => {
            if let Err(e) = cmd_unpublish(&name) {
                let _ = cliclack::outro(format!("{}", style(e).red()));
                std::process::exit(1);
            }
        }
        Some(Commands::Install { path, all }) => {
            let consumer_dir = path.or(cli.path)
                .unwrap_or_else(|| std::env::current_dir().unwrap());
            let all = all || cli.all;
            if let Err(e) = cmd_install(&consumer_dir, all) {
                let _ = cliclack::outro(format!("{}", style(e).red()));
                std::process::exit(1);
            }
        }
        None => {
            // bare `smuggle` = `smuggle install`
            let consumer_dir = cli.path
                .unwrap_or_else(|| std::env::current_dir().unwrap());
            if let Err(e) = cmd_install(&consumer_dir, cli.all) {
                let _ = cliclack::outro(format!("{}", style(e).red()));
                std::process::exit(1);
            }
        }
    }
}

fn cmd_publish(pkg_dir: &PathBuf, select_all: bool) -> Result<(), String> {
    let pkg_dir = pkg_dir
        .canonicalize()
        .map_err(|e| format!("invalid path: {e}"))?;

    // Check for pnpm workspace
    if let Some(workspace_packages) = workspace::detect_pnpm_workspace(&pkg_dir) {
        return cmd_publish_workspace(&pkg_dir, workspace_packages, select_all);
    }

    // Single package publish
    publish_single_package(&pkg_dir)
}

fn publish_single_package(pkg_dir: &std::path::Path) -> Result<(), String> {
    let pkg_json_path = pkg_dir.join("package.json");
    if !pkg_json_path.exists() {
        return Err("no package.json found in this directory".into());
    }

    let pkg_json: pack::PublishPackageJson =
        serde_json::from_str(&std::fs::read_to_string(&pkg_json_path).map_err(|e| e.to_string())?)
            .map_err(|e| format!("failed to parse package.json: {e}"))?;

    let name = pkg_json
        .name
        .as_ref()
        .ok_or("package.json missing 'name' field")?;

    let version = pkg_json
        .version
        .as_ref()
        .ok_or("package.json missing 'version' field")?;

    let spinner = cliclack::spinner();
    spinner.start(format!("Packing {name}@{version}..."));

    let tarball = pack::pack(pkg_dir, &pkg_json)?;

    store::save(name, version, &pkg_dir.to_path_buf(), &tarball, &pkg_json.dependencies())?;

    spinner.stop(format!(
        "Published {} -> ~/.smuggle/packages/{name}/",
        style(format!("{name}@{version}")).cyan(),
    ));

    Ok(())
}

fn cmd_publish_workspace(
    root: &std::path::Path,
    packages: Vec<workspace::WorkspacePackage>,
    select_all: bool,
) -> Result<(), String> {
    let _ = cliclack::intro(style(" smuggle publish ").on_cyan().black());

    cliclack::log::info("Detected pnpm workspace").map_err(|e| e.to_string())?;

    if packages.is_empty() {
        return Err("no packages found in workspace".into());
    }

    let selected_indices: Vec<usize> = if select_all {
        let selected: Vec<usize> = packages
            .iter()
            .enumerate()
            .filter(|(_, p)| !p.is_private)
            .map(|(i, _)| i)
            .collect();

        let list: Vec<String> = selected
            .iter()
            .map(|&i| {
                let p = &packages[i];
                let rel = p.path.strip_prefix(root).unwrap_or(&p.path);
                format!("{} @ {} ({})", style(&p.name).cyan(), p.version, rel.display())
            })
            .collect();

        cliclack::log::info(format!(
            "Selecting all {} publishable package(s)\n{}",
            selected.len(),
            list.join("\n"),
        )).map_err(|e| e.to_string())?;

        selected
    } else {
        let mut prompt = cliclack::multiselect("Select packages to publish");

        for (i, p) in packages.iter().enumerate() {
            let rel = p.path.strip_prefix(root).unwrap_or(&p.path);
            let suffix = if p.is_root { " (root)" } else { "" };
            let private_tag = if p.is_private { " [private]" } else { "" };
            let label = format!("{} @ {} ({}){}{}", p.name, p.version, rel.display(), suffix, private_tag);
            prompt = prompt.item(i, label, "");
        }

        let defaults: Vec<usize> = packages
            .iter()
            .enumerate()
            .filter(|(_, p)| !p.is_private)
            .map(|(i, _)| i)
            .collect();
        prompt = prompt.initial_values(defaults);

        let selections: Vec<usize> = prompt
            .interact()
            .map_err(|e| format!("selection cancelled: {e}"))?;

        if selections.is_empty() {
            let _ = cliclack::outro("No packages selected, nothing to do.");
            return Ok(());
        }

        selections
    };

    // Publish each selected package
    let mut published = 0;
    let mut errors = Vec::new();

    for &idx in &selected_indices {
        let pkg = &packages[idx];
        match publish_single_package(&pkg.path) {
            Ok(()) => published += 1,
            Err(e) => {
                cliclack::log::warning(format!("Failed to publish {}: {e}", pkg.name))
                    .map_err(|e| e.to_string())?;
                errors.push(pkg.name.clone());
            }
        }
    }

    if errors.is_empty() {
        let _ = cliclack::outro(format!("Published {} package(s)", style(published).green().bold()));
    } else {
        let _ = cliclack::outro(format!(
            "Published {} package(s), {} failed",
            style(published).green().bold(),
            style(errors.len()).red().bold(),
        ));
    }

    Ok(())
}

fn cmd_list() {
    let packages = store::list();
    if packages.is_empty() {
        let _ = cliclack::log::info(format!(
            "No packages registered. Run {} in a package directory first.",
            style("smuggle publish").cyan(),
        ));
        return;
    }

    let _ = cliclack::intro(style(" smuggle list ").on_cyan().black());

    for entry in &packages {
        let _ = cliclack::log::info(format!(
            "{} {} {}",
            style(&entry.name).cyan().bold(),
            style(format!("@ {}", entry.version)).dim(),
            style(format!("({})", entry.source_dir.display())).dim(),
        ));
    }

    let _ = cliclack::outro(format!("{} package(s) registered", packages.len()));
}

fn cmd_unpublish(name: &str) -> Result<(), String> {
    store::remove(name)?;
    let _ = cliclack::log::success(format!("Removed {} from local store", style(name).cyan()));
    Ok(())
}

fn cmd_install(consumer_dir: &PathBuf, select_all: bool) -> Result<(), String> {
    let consumer_dir = consumer_dir
        .canonicalize()
        .map_err(|e| format!("invalid path: {e}"))?;

    // Detect pnpm workspace
    if let Some(workspace_packages) = workspace::detect_pnpm_workspace(&consumer_dir) {
        return cmd_install_workspace(&consumer_dir, workspace_packages, select_all);
    }

    let pkg_json_path = consumer_dir.join("package.json");
    if !pkg_json_path.exists() {
        return Err("no package.json found in consumer directory".into());
    }

    let _ = cliclack::intro(style(" smuggle install ").on_cyan().black());

    let consumer_pkg: pack::ConsumerPackageJson = serde_json::from_str(
        &std::fs::read_to_string(&pkg_json_path).map_err(|e| e.to_string())?,
    )
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
            .map(|e| format!("{} @ {} ({})", style(&e.name).cyan(), e.version, e.source_dir.display()))
            .collect();
        cliclack::log::info(format!(
            "Selecting all {} matching package(s)\n{}",
            matches.len(),
            list.join("\n"),
        )).map_err(|e| e.to_string())?;
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

    run_install_flow(&consumer_dir, &selected, &[])
}

fn cmd_install_workspace(
    root: &std::path::Path,
    workspace_packages: Vec<workspace::WorkspacePackage>,
    select_all: bool,
) -> Result<(), String> {
    let _ = cliclack::intro(style(" smuggle install ").on_cyan().black());

    cliclack::log::info("Detected pnpm workspace").map_err(|e| e.to_string())?;

    let registered = store::list();
    if registered.is_empty() {
        return Err(
            "no registered packages. publish some first with `smuggle publish`.".into(),
        );
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
        )).map_err(|e| e.to_string())?;
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

    let ws_dirs: Vec<PathBuf> = workspace_packages.iter().map(|wp| wp.path.clone()).collect();
    run_install_flow(root, &selected, &ws_dirs)
}

/// Shared install flow: expand deps, overwrite in node_modules, watch for changes.
/// No registry, no .npmrc, no lockfile changes — just direct file replacement.
fn run_install_flow(
    install_dir: &std::path::Path,
    selected: &[&store::StoreEntry],
    workspace_pkg_dirs: &[PathBuf],
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
            extra.iter().map(|n| style(n).cyan().to_string()).collect::<Vec<_>>().join(", "),
        )).map_err(|e| e.to_string())?;
    }

    // Resolve each package's location in node_modules.
    // In workspaces, packages may be in member node_modules, not the root.
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

    let mut targets: Vec<OverrideTarget> = Vec::new();
    for entry in &all_refs {
        // Find the package in any node_modules directory
        let mut found = false;
        for nm_dir in &nm_dirs {
            let pkg_path = nm_dir.join(&entry.name);
            if !pkg_path.exists() {
                continue;
            }
            // Resolve symlinks (pnpm uses symlinks to .pnpm virtual store)
            let real_path = pkg_path.canonicalize()
                .map_err(|e| format!("failed to resolve {}: {e}", pkg_path.display()))?;

            // Avoid duplicates (pnpm workspace members may resolve to the same .pnpm path)
            if !targets.iter().any(|t| t.target_dir == real_path) {
                targets.push(OverrideTarget {
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

    if targets.is_empty() {
        return Err("none of the selected packages are installed in node_modules".into());
    }

    // Backup original directories
    let backup_base = std::env::temp_dir()
        .join("smuggle-backup")
        .join(std::process::id().to_string());
    std::fs::create_dir_all(&backup_base)
        .map_err(|e| format!("failed to create backup dir: {e}"))?;

    let spinner = cliclack::spinner();
    spinner.start("Backing up originals...");

    for target in &targets {
        let backup_name = target.name.replace('/', "__");
        let backup_path = backup_base.join(&backup_name);
        copy_dir(&target.target_dir, &backup_path)
            .map_err(|e| format!("failed to backup {}: {e}", target.name))?;
    }

    spinner.stop("Originals backed up");

    // Set up cleanup on ctrl-c
    let cleanup_targets: Vec<(PathBuf, PathBuf)> = targets
        .iter()
        .map(|t| {
            let backup_name = t.name.replace('/', "__");
            (backup_base.join(&backup_name), t.target_dir.clone())
        })
        .collect();
    let cleanup_backup_base = backup_base.clone();
    let _ = ctrlc::set_handler(move || {
        for (backup, target) in &cleanup_targets {
            let _ = restore_dir(backup, target);
        }
        let _ = std::fs::remove_dir_all(&cleanup_backup_base);
        eprintln!("\n  Restored originals");
        std::process::exit(0);
    });

    // Extract tarballs into node_modules
    let extract_spinner = cliclack::spinner();
    extract_spinner.start(format!("Smuggling {} package(s)...", targets.len()));

    for target in &targets {
        let tarball = store::load_tarball(&target.name)?;
        pack::extract_tarball_to(&tarball, &target.target_dir)?;
    }

    extract_spinner.stop(format!("Smuggled {} package(s) into node_modules", style(targets.len()).green()));

    // Clear bundler caches and trigger vite restart
    let extra: Vec<&std::path::Path> = workspace_pkg_dirs.iter().map(|p| p.as_path()).collect();
    pm::clear_bundler_caches(install_dir, &extra);
    touch_vite_configs(install_dir, workspace_pkg_dirs);

    cliclack::log::success(format!("Watching for changes... {}", style("(ctrl-c to stop)").dim()))
        .map_err(|e| e.to_string())?;

    // Watch for changes
    watch_and_reinstall(&all_refs, &targets, install_dir, workspace_pkg_dirs)?;

    // Cleanup on normal exit — restore originals
    let restore_spinner = cliclack::spinner();
    restore_spinner.start("Restoring originals...");
    for target in &targets {
        let backup_name = target.name.replace('/', "__");
        let backup_path = backup_base.join(&backup_name);
        let _ = restore_dir(&backup_path, &target.target_dir);
    }
    let _ = std::fs::remove_dir_all(&backup_base);
    restore_spinner.stop("Restored originals");

    let _ = cliclack::outro("Done");

    Ok(())
}

struct OverrideTarget {
    name: String,
    target_dir: PathBuf,
}

/// Copy a directory tree, preserving structure.
fn copy_dir(src: &std::path::Path, dst: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| format!("mkdir {}: {e}", dst.display()))?;
    for entry in std::fs::read_dir(src).map_err(|e| format!("read {}: {e}", src.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            // Skip nested node_modules (pnpm dep symlinks)
            if entry.file_name() == "node_modules" {
                continue;
            }
            copy_dir(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)
                .map_err(|e| format!("copy {}: {e}", src_path.display()))?;
        }
    }
    Ok(())
}

/// Restore a directory from backup, replacing current contents.
fn restore_dir(backup: &std::path::Path, target: &std::path::Path) -> Result<(), String> {
    // Remove current files (but not node_modules subdirs managed by the package manager)
    if let Ok(entries) = std::fs::read_dir(target) {
        for entry in entries.flatten() {
            if entry.file_name() == "node_modules" {
                continue;
            }
            let path = entry.path();
            if path.is_dir() {
                let _ = std::fs::remove_dir_all(&path);
            } else {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    // Copy backup files back
    if let Ok(entries) = std::fs::read_dir(backup) {
        for entry in entries.flatten() {
            let src = entry.path();
            let dst = target.join(entry.file_name());
            if src.is_dir() {
                copy_dir(&src, &dst)?;
            } else {
                std::fs::copy(&src, &dst).map_err(|e| format!("restore {}: {e}", dst.display()))?;
            }
        }
    }
    Ok(())
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

    // Collect the full entries in a stable order
    let mut result: Vec<store::StoreEntry> = registered
        .iter()
        .filter(|e| included.contains(&e.name))
        .cloned()
        .collect();
    result.sort_by(|a, b| a.name.cmp(&b.name));
    result
}


fn watch_and_reinstall(
    selected: &[&store::StoreEntry],
    targets: &[OverrideTarget],
    consumer_dir: &std::path::Path,
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
        let mut actually_changed: Vec<String> = Vec::new();
        for pkg_name in &changed_packages {
            let entry = selected.iter().find(|e| &e.name == pkg_name).unwrap();
            let Some(target) = targets.iter().find(|t| t.name == *pkg_name) else {
                continue;
            };

            spinner.start(format!("Packing {}...", style(&entry.name).cyan()));

            let pkg_json_path = entry.source_dir.join("package.json");
            let Ok(raw) = std::fs::read_to_string(&pkg_json_path) else {
                let _ = cliclack::log::warning(format!("Could not read {}", pkg_json_path.display()));
                continue;
            };
            let Ok(pkg_json) = serde_json::from_str::<pack::PublishPackageJson>(&raw) else {
                let _ = cliclack::log::warning(format!("Could not parse {}", pkg_json_path.display()));
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
                    let _ = store::save(&entry.name, version, &entry.source_dir, &tarball, &pkg_json.dependencies());

                    spinner.start(format!("Smuggling {}...", style(&entry.name).cyan()));
                    if let Err(e) = pack::extract_tarball_to(&tarball, &target.target_dir) {
                        let _ = cliclack::log::warning(format!("Failed to extract {}: {e}", entry.name));
                    } else {
                        actually_changed.push(pkg_name.clone());
                    }
                }
                Err(e) => {
                    let _ = cliclack::log::warning(format!("Failed to pack {}: {e}", entry.name));
                }
            }
        }

        if actually_changed.is_empty() {
            spinner.stop(format!("{}", style("No content changes").dim()));
            continue;
        }

        // Clear bundler caches and trigger vite restart
        let extra: Vec<&std::path::Path> = workspace_pkg_dirs.iter().map(|p| p.as_path()).collect();
        pm::clear_bundler_caches(consumer_dir, &extra);
        touch_vite_configs(consumer_dir, workspace_pkg_dirs);

        spinner.stop(format!("Smuggled {}", style(actually_changed.join(", ")).cyan()));
    }

    Ok(())
}

/// Determine which directories to watch for a package.
/// If the package.json has a `files` field, watch only those directories.
/// Otherwise, watch the entire package root (package.json changes, etc.).
fn resolve_watch_dirs(pkg_dir: &std::path::Path) -> Vec<PathBuf> {
    let pkg_json_path = pkg_dir.join("package.json");
    let Ok(raw) = std::fs::read_to_string(&pkg_json_path) else {
        return vec![pkg_dir.to_path_buf()];
    };
    let Ok(pkg_json) = serde_json::from_str::<pack::PublishPackageJson>(&raw) else {
        return vec![pkg_dir.to_path_buf()];
    };

    let Some(files) = pkg_json.files_list() else {
        // No files field — watch the whole package root
        return vec![pkg_dir.to_path_buf()];
    };

    let mut dirs = Vec::new();

    // Always watch package.json itself (it's in the pack output)
    // We watch the parent dir non-recursively for this, but since we can't
    // do per-file watches easily, we collect the unique directories.
    dirs.push(pkg_dir.to_path_buf());

    for pattern in files {
        // Skip negation patterns (e.g. "!dist/types.d.ts")
        if pattern.starts_with('!') {
            continue;
        }

        // Strip glob suffixes to get the base directory
        let clean: &str = pattern
            .trim_start_matches('/')
            .trim_end_matches("/**")
            .trim_end_matches("/*")
            .trim_end_matches('/');

        // Strip anything after first glob character to get the base
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
            // For individual files, watch the parent directory
            if let Some(parent) = abs.parent() {
                dirs.push(parent.to_path_buf());
            }
        }
        // If the path doesn't exist yet, skip — it'll appear after a build
    }

    // Deduplicate
    dirs.sort();
    dirs.dedup();

    // Remove subdirectories if a parent is already watched
    let mut filtered = Vec::new();
    for dir in &dirs {
        let dominated = filtered.iter().any(|parent: &PathBuf| dir.starts_with(parent) && dir != parent);
        if !dominated {
            // Also remove any existing entries that this new dir is a parent of
            filtered.retain(|existing: &PathBuf| !existing.starts_with(dir) || existing == dir);
            filtered.push(dir.clone());
        }
    }

    filtered
}

/// Touch vite config files to trigger a dev server restart.
/// Checks root and all workspace member directories.
fn touch_vite_configs(root: &std::path::Path, workspace_pkg_dirs: &[PathBuf]) {
    let config_names = ["vite.config.ts", "vite.config.js", "vite.config.mts", "vite.config.mjs"];

    let mut dirs = vec![root.to_path_buf()];
    dirs.extend_from_slice(workspace_pkg_dirs);

    for dir in &dirs {
        for name in &config_names {
            let path = dir.join(name);
            if path.exists() {
                let now = filetime::FileTime::now();
                let _ = filetime::set_file_mtime(&path, now);
                let display = path.strip_prefix(root).unwrap_or(&path);
                let _ = cliclack::log::remark(format!(
                    "touched {} to trigger reload",
                    style(display.display()).dim(),
                ));
                break;
            }
        }
    }
}

fn hash_bytes(data: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    data.hash(&mut hasher);
    hasher.finish()
}

fn is_ignored_path(path: &std::path::Path, source_root: &std::path::Path) -> bool {
    let rel = path.strip_prefix(source_root).unwrap_or(path);
    let rel_str = rel.to_string_lossy();
    rel_str.starts_with("node_modules")
        || rel_str.starts_with(".git")
        || rel_str.starts_with("target")
}
