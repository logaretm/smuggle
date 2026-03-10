mod pack;
mod pm;
mod registry;
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

    /// Also clear cache for proxied packages that depend on other proxied packages
    #[arg(long, global = true)]
    deep: bool,

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

        /// Also clear cache for proxied packages that depend on other proxied packages
        #[arg(long)]
        deep: bool,

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
                eprintln!("{} {e}", style("error:").red().bold());
                std::process::exit(1);
            }
        }
        Some(Commands::List) => {
            cmd_list();
        }
        Some(Commands::Unpublish { name }) => {
            if let Err(e) = cmd_unpublish(&name) {
                eprintln!("{} {e}", style("error:").red().bold());
                std::process::exit(1);
            }
        }
        Some(Commands::Install { path, deep, all }) => {
            let consumer_dir = path.or(cli.path)
                .unwrap_or_else(|| std::env::current_dir().unwrap());
            let deep = deep || cli.deep;
            let all = all || cli.all;
            if let Err(e) = cmd_install(&consumer_dir, deep, all) {
                eprintln!("{} {e}", style("error:").red().bold());
                std::process::exit(1);
            }
        }
        None => {
            // bare `smuggle` = `smuggle install`
            let consumer_dir = cli.path
                .unwrap_or_else(|| std::env::current_dir().unwrap());
            if let Err(e) = cmd_install(&consumer_dir, cli.deep, cli.all) {
                eprintln!("{} {e}", style("error:").red().bold());
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

    let spinner = indicatif::ProgressBar::new_spinner();
    spinner.set_style(
        indicatif::ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    spinner.set_message(format!("Packing {}...", style(format!("{name}@{version}")).cyan()));
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    let tarball = pack::pack(pkg_dir, &pkg_json)?;

    store::save(name, version, &pkg_dir.to_path_buf(), &tarball, &pkg_json.dependencies())?;

    spinner.finish_with_message(format!(
        "{} Published {} -> {}",
        style("*").green().bold(),
        style(format!("{name}@{version}")).cyan().bold(),
        style(format!("~/.smuggle/packages/{name}/")).dim(),
    ));

    Ok(())
}

fn cmd_publish_workspace(
    root: &std::path::Path,
    packages: Vec<workspace::WorkspacePackage>,
    select_all: bool,
) -> Result<(), String> {
    eprintln!("{} Detected pnpm workspace\n",
        style("*").cyan().bold(),
    );

    if packages.is_empty() {
        return Err("no packages found in workspace".into());
    }

    // Build display items
    let selected_indices: Vec<usize> = if select_all {
        // Select all non-private packages
        let selected: Vec<usize> = packages
            .iter()
            .enumerate()
            .filter(|(_, p)| !p.is_private)
            .map(|(i, _)| i)
            .collect();

        eprintln!("{} Selecting all {} publishable package(s)",
            style("*").cyan().bold(),
            style(selected.len()).bold(),
        );

        for &i in &selected {
            let p = &packages[i];
            let rel = p.path.strip_prefix(root).unwrap_or(&p.path);
            eprintln!("  {} {} {}",
                style("|").dim(),
                style(&p.name).cyan().bold(),
                style(format!("@ {} ({})", p.version, rel.display())).dim(),
            );
        }

        selected
    } else {
        let items: Vec<String> = packages
            .iter()
            .map(|p| {
                let rel = p.path.strip_prefix(root).unwrap_or(&p.path);
                let suffix = if p.is_root { " (root)" } else { "" };
                let private_tag = if p.is_private { " [private]" } else { "" };
                format!("{} @ {} ({}){}{}", p.name, p.version, rel.display(), suffix, private_tag)
            })
            .collect();

        // Default: select non-private packages
        let defaults: Vec<bool> = packages.iter().map(|p| !p.is_private).collect();

        let selections = dialoguer::MultiSelect::new()
            .with_prompt("Select packages to publish")
            .items(&items)
            .defaults(&defaults)
            .interact()
            .map_err(|e| format!("selection cancelled: {e}"))?;

        if selections.is_empty() {
            eprintln!("{} No packages selected, nothing to do.", style("*").dim());
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
                eprintln!("  {} failed to publish {}: {e}",
                    style("warn:").yellow(),
                    style(&pkg.name).cyan(),
                );
                errors.push(pkg.name.clone());
            }
        }
    }

    eprintln!();
    if errors.is_empty() {
        eprintln!("{} Published {} package(s)",
            style("*").green().bold(),
            style(published).bold(),
        );
    } else {
        eprintln!("{} Published {} package(s), {} failed",
            style("*").yellow().bold(),
            style(published).bold(),
            style(errors.len()).red().bold(),
        );
    }

    Ok(())
}

fn cmd_list() {
    let packages = store::list();
    if packages.is_empty() {
        eprintln!("{} No packages registered. Run {} in a package directory first.",
            style("*").dim(),
            style("smuggle publish").cyan(),
        );
        return;
    }

    eprintln!("{}\n", style("Registered packages:").bold());
    for entry in packages {
        eprintln!(
            "  {} {} {} {}",
            style("*").green(),
            style(&entry.name).cyan().bold(),
            style(format!("@ {}", entry.version)).dim(),
            style(format!("({})", entry.source_dir.display())).dim(),
        );
    }
}

fn cmd_unpublish(name: &str) -> Result<(), String> {
    store::remove(name)?;
    eprintln!("{} Removed {} from local store",
        style("*").green().bold(),
        style(name).cyan().bold(),
    );
    Ok(())
}

fn cmd_install(consumer_dir: &PathBuf, deep: bool, select_all: bool) -> Result<(), String> {
    let consumer_dir = consumer_dir
        .canonicalize()
        .map_err(|e| format!("invalid path: {e}"))?;

    // Detect pnpm workspace
    if let Some(workspace_packages) = workspace::detect_pnpm_workspace(&consumer_dir) {
        return cmd_install_workspace(&consumer_dir, workspace_packages, deep, select_all);
    }

    let pkg_json_path = consumer_dir.join("package.json");
    if !pkg_json_path.exists() {
        return Err("no package.json found in consumer directory".into());
    }

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
        eprintln!("{} Selecting all {} matching package(s)",
            style("*").cyan().bold(),
            style(matches.len()).bold(),
        );
        for e in &matches {
            eprintln!("  {} {} {}",
                style("|").dim(),
                style(&e.name).cyan().bold(),
                style(format!("@ {} ({})", e.version, e.source_dir.display())).dim(),
            );
        }
        matches.clone()
    } else {
        let items: Vec<String> = matches
            .iter()
            .map(|e| format!("{} @ {} ({})", e.name, e.version, e.source_dir.display()))
            .collect();

        let defaults: Vec<bool> = vec![true; items.len()];

        let selections = dialoguer::MultiSelect::new()
            .with_prompt("Select packages to proxy locally")
            .items(&items)
            .defaults(&defaults)
            .interact()
            .map_err(|e| format!("selection cancelled: {e}"))?;

        if selections.is_empty() {
            eprintln!("{} No packages selected, nothing to do.", style("*").dim());
            return Ok(());
        }

        selections.iter().map(|&i| matches[i]).collect()
    };

    let pm = pm::detect_package_manager(&consumer_dir);
    eprintln!("\n{} Detected package manager: {}",
        style("*").cyan().bold(),
        style(pm.name()).green().bold(),
    );

    let reverse_deps = if deep {
        build_reverse_dep_map(&selected)
    } else {
        std::collections::HashMap::new()
    };

    // Start registry server
    let registry_packages: Vec<registry::RegistryPackage> = selected
        .iter()
        .map(|entry| {
            let tarball = store::load_tarball(&entry.name)
                .unwrap_or_else(|e| panic!("failed to load tarball for {}: {e}", entry.name));
            registry::RegistryPackage {
                name: entry.name.clone(),
                version: entry.version.clone(),
                tarball,
                dependencies: entry.dependencies.clone(),
            }
        })
        .collect();

    let server = registry::start(registry_packages)?;
    let port = server.port;
    eprintln!("{} Local registry started on port {}",
        style("*").green().bold(),
        style(port).cyan(),
    );

    // Backup and write .npmrc
    let npmrc_path = consumer_dir.join(".npmrc");
    let original_npmrc = if npmrc_path.exists() {
        Some(std::fs::read_to_string(&npmrc_path).unwrap_or_default())
    } else {
        None
    };

    write_npmrc(&npmrc_path, port, &selected)?;

    // Cleanup on ctrl-c
    let cleanup_npmrc = npmrc_path.clone();
    let cleanup_original = original_npmrc.clone();
    let _ = ctrlc::set_handler(move || {
        restore_npmrc(&cleanup_npmrc, cleanup_original.as_deref());
        std::process::exit(0);
    });

    // Clear cache for selected packages
    let selected_names: Vec<String> = selected.iter().map(|e| e.name.clone()).collect();
    let mut to_clear = selected_names.clone();

    if deep {
        for name in &selected_names {
            if let Some(dependents) = reverse_deps.get(name) {
                for dep in dependents {
                    if !to_clear.contains(dep) {
                        to_clear.push(dep.clone());
                    }
                }
            }
        }
    }

    eprintln!("\n{} Clearing cache for {} package(s)...",
        style("*").cyan().bold(),
        style(to_clear.len()).bold(),
    );
    pm::clear_cache(pm, &to_clear, &consumer_dir);

    // Run install
    eprintln!("\n{} Running {} install...",
        style("*").cyan().bold(),
        style(pm.name()).green().bold(),
    );
    pm::run_install(pm, &consumer_dir)?;

    eprintln!("\n{} Watching for changes... {}\n",
        style("*").green().bold(),
        style("(ctrl-c to stop)").dim(),
    );

    // Watch for changes
    watch_and_reinstall(&selected, &consumer_dir, pm, deep, &reverse_deps, &server)?;

    // Cleanup on normal exit
    restore_npmrc(&npmrc_path, original_npmrc.as_deref());
    eprintln!("\n{} Cleaned up .npmrc", style("*").dim());

    Ok(())
}

fn cmd_install_workspace(
    root: &std::path::Path,
    workspace_packages: Vec<workspace::WorkspacePackage>,
    deep: bool,
    select_all: bool,
) -> Result<(), String> {
    eprintln!("{} Detected pnpm workspace\n",
        style("*").cyan().bold(),
    );

    let registered = store::list();
    if registered.is_empty() {
        return Err(
            "no registered packages. publish some first with `smuggle publish`.".into(),
        );
    }

    // Collect deps from all workspace packages and track which workspace pkg uses which proxied dep
    let mut all_deps = std::collections::HashSet::new();
    let mut workspace_dep_map: Vec<(String, Vec<String>)> = Vec::new(); // (workspace_pkg_name, [proxied_dep_names])

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
        eprintln!("{} Found {} proxied package(s) across {} workspace package(s)\n",
            style("*").cyan().bold(),
            style(matches.len()).bold(),
            style(workspace_dep_map.len()).bold(),
        );
        for (wp_name, dep_names) in &workspace_dep_map {
            eprintln!("  {} {}",
                style("|").dim(),
                style(wp_name).bold(),
            );
            for dep in dep_names {
                eprintln!("    {} {}",
                    style("|").dim(),
                    style(dep).cyan(),
                );
            }
        }
        matches.clone()
    } else {
        let items: Vec<String> = matches
            .iter()
            .map(|e| format!("{} @ {} ({})", e.name, e.version, e.source_dir.display()))
            .collect();

        let defaults: Vec<bool> = vec![true; items.len()];

        let selections = dialoguer::MultiSelect::new()
            .with_prompt("Select packages to proxy locally")
            .items(&items)
            .defaults(&defaults)
            .interact()
            .map_err(|e| format!("selection cancelled: {e}"))?;

        if selections.is_empty() {
            eprintln!("{} No packages selected, nothing to do.", style("*").dim());
            return Ok(());
        }

        selections.iter().map(|&i| matches[i]).collect()
    };

    let pm = pm::detect_package_manager(root);
    eprintln!("\n{} Detected package manager: {}",
        style("*").cyan().bold(),
        style(pm.name()).green().bold(),
    );

    let reverse_deps = if deep {
        build_reverse_dep_map(&selected)
    } else {
        std::collections::HashMap::new()
    };

    // Start registry server
    let registry_packages: Vec<registry::RegistryPackage> = selected
        .iter()
        .map(|entry| {
            let tarball = store::load_tarball(&entry.name)
                .unwrap_or_else(|e| panic!("failed to load tarball for {}: {e}", entry.name));
            registry::RegistryPackage {
                name: entry.name.clone(),
                version: entry.version.clone(),
                tarball,
                dependencies: entry.dependencies.clone(),
            }
        })
        .collect();

    let server = registry::start(registry_packages)?;
    let port = server.port;
    eprintln!("{} Local registry started on port {}",
        style("*").green().bold(),
        style(port).cyan(),
    );

    // Backup and write .npmrc at workspace root
    let npmrc_path = root.join(".npmrc");
    let original_npmrc = if npmrc_path.exists() {
        Some(std::fs::read_to_string(&npmrc_path).unwrap_or_default())
    } else {
        None
    };

    write_npmrc(&npmrc_path, port, &selected)?;

    // Cleanup on ctrl-c
    let cleanup_npmrc = npmrc_path.clone();
    let cleanup_original = original_npmrc.clone();
    let _ = ctrlc::set_handler(move || {
        restore_npmrc(&cleanup_npmrc, cleanup_original.as_deref());
        std::process::exit(0);
    });

    // Clear cache for selected packages
    let selected_names: Vec<String> = selected.iter().map(|e| e.name.clone()).collect();
    let mut to_clear = selected_names.clone();

    if deep {
        for name in &selected_names {
            if let Some(dependents) = reverse_deps.get(name) {
                for dep in dependents {
                    if !to_clear.contains(dep) {
                        to_clear.push(dep.clone());
                    }
                }
            }
        }
    }

    eprintln!("\n{} Clearing cache for {} package(s)...",
        style("*").cyan().bold(),
        style(to_clear.len()).bold(),
    );
    pm::clear_cache(pm, &to_clear, root);

    // Run install at workspace root
    eprintln!("\n{} Running {} install...",
        style("*").cyan().bold(),
        style(pm.name()).green().bold(),
    );
    pm::run_install(pm, root)?;

    eprintln!("\n{} Watching for changes... {}\n",
        style("*").green().bold(),
        style("(ctrl-c to stop)").dim(),
    );

    // Watch for changes
    watch_and_reinstall(&selected, root, pm, deep, &reverse_deps, &server)?;

    // Cleanup on normal exit
    restore_npmrc(&npmrc_path, original_npmrc.as_deref());
    eprintln!("\n{} Cleaned up .npmrc", style("*").dim());

    Ok(())
}

fn build_reverse_dep_map(
    selected: &[&store::StoreEntry],
) -> std::collections::HashMap<String, Vec<String>> {
    let mut map: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    let selected_names: std::collections::HashSet<String> =
        selected.iter().map(|e| e.name.clone()).collect();

    for entry in selected {
        for dep_name in entry.dependencies.keys() {
            if selected_names.contains(dep_name) {
                map.entry(dep_name.clone())
                    .or_default()
                    .push(entry.name.clone());
            }
        }
    }

    map
}

fn write_npmrc(
    path: &std::path::Path,
    port: u16,
    selected: &[&store::StoreEntry],
) -> Result<(), String> {
    let mut content = String::from("# managed by smuggle — do not edit\n");

    let scopes: std::collections::HashSet<&str> = selected
        .iter()
        .filter_map(|e| {
            if e.name.starts_with('@') {
                e.name.split('/').next()
            } else {
                None
            }
        })
        .collect();

    if scopes.len() == 1 && selected.iter().all(|e| e.name.starts_with('@')) {
        let scope = scopes.into_iter().next().unwrap();
        content.push_str(&format!("{scope}:registry=http://localhost:{port}\n"));
    } else {
        content.push_str(&format!("registry=http://localhost:{port}\n"));
    }

    // Preserve non-smuggle lines from existing .npmrc
    if path.exists() {
        let existing = std::fs::read_to_string(path).unwrap_or_default();
        for line in existing.lines() {
            if !line.contains("managed by smuggle") && !line.contains(&format!("localhost:{port}")) {
                content.push_str(line);
                content.push('\n');
            }
        }
    }

    std::fs::write(path, content).map_err(|e| format!("failed to write .npmrc: {e}"))
}

fn restore_npmrc(path: &std::path::Path, original: Option<&str>) {
    match original {
        Some(content) => {
            let _ = std::fs::write(path, content);
        }
        None => {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn watch_and_reinstall(
    selected: &[&store::StoreEntry],
    consumer_dir: &std::path::Path,
    pm: pm::PackageManager,
    deep: bool,
    reverse_deps: &std::collections::HashMap<String, Vec<String>>,
    server: &registry::Server,
) -> Result<(), String> {
    use notify::{RecursiveMode, Watcher};
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
        watcher
            .watch(&entry.source_dir, RecursiveMode::Recursive)
            .map_err(|e| format!("failed to watch {}: {e}", entry.source_dir.display()))?;
        eprintln!("  {} {}",
            style("|").dim(),
            style(entry.source_dir.display()).dim(),
        );
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
            .map(|p| style(p).cyan().bold().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        eprintln!("\n{} Change detected in: {}", style("*").yellow().bold(), pkg_list);

        let spinner = indicatif::ProgressBar::new_spinner();
        spinner.set_style(
            indicatif::ProgressStyle::default_spinner()
                .template("{spinner:.cyan} {msg}")
                .unwrap(),
        );
        spinner.enable_steady_tick(std::time::Duration::from_millis(80));

        // Re-pack changed packages
        for pkg_name in &changed_packages {
            let entry = selected.iter().find(|e| &e.name == pkg_name).unwrap();
            spinner.set_message(format!("Re-packing {}...", style(&entry.name).cyan()));

            let pkg_json_path = entry.source_dir.join("package.json");
            let Ok(raw) = std::fs::read_to_string(&pkg_json_path) else {
                eprintln!("  {} could not read {}",
                    style("warn:").yellow(),
                    pkg_json_path.display(),
                );
                continue;
            };
            let Ok(pkg_json) = serde_json::from_str::<pack::PublishPackageJson>(&raw) else {
                eprintln!("  {} could not parse {}",
                    style("warn:").yellow(),
                    pkg_json_path.display(),
                );
                continue;
            };

            match pack::pack(&entry.source_dir, &pkg_json) {
                Ok(tarball) => {
                    let version = pkg_json.version.as_deref().unwrap_or("0.0.0");
                    let _ =
                        store::save(&entry.name, version, &entry.source_dir, &tarball, &pkg_json.dependencies());
                    server.update_tarball(&entry.name, tarball);
                }
                Err(e) => {
                    eprintln!("  {} failed to pack {}: {e}",
                        style("warn:").yellow(),
                        style(&entry.name).cyan(),
                    );
                }
            }
        }

        // Cache clear
        let mut to_clear = changed_packages.clone();
        if deep {
            for name in &changed_packages {
                if let Some(dependents) = reverse_deps.get(name) {
                    for dep in dependents {
                        if !to_clear.contains(dep) {
                            to_clear.push(dep.clone());
                        }
                    }
                }
            }
        }

        spinner.set_message(format!("Clearing cache for {} package(s)...", style(to_clear.len()).bold()));
        pm::clear_cache(pm, &to_clear, consumer_dir);

        spinner.set_message(format!("Running {} update...", style(pm.name()).green()));
        match pm::run_update(pm, &to_clear, consumer_dir) {
            Ok(()) => spinner.finish_with_message(format!("{} Updated successfully", style("*").green().bold())),
            Err(e) => spinner.finish_with_message(format!("{} Update failed: {e}", style("error:").red().bold())),
        }
    }

    Ok(())
}

fn is_ignored_path(path: &std::path::Path, source_root: &std::path::Path) -> bool {
    let rel = path.strip_prefix(source_root).unwrap_or(path);
    let rel_str = rel.to_string_lossy();
    rel_str.starts_with("node_modules")
        || rel_str.starts_with(".git")
        || rel_str.starts_with("target")
}
