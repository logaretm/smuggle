mod pack;
mod pm;
mod registry;
mod store;

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
        Some(Commands::Publish { path }) => {
            let pkg_dir = path.unwrap_or_else(|| std::env::current_dir().unwrap());
            if let Err(e) = cmd_publish(&pkg_dir) {
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

fn cmd_publish(pkg_dir: &PathBuf) -> Result<(), String> {
    let pkg_dir = pkg_dir
        .canonicalize()
        .map_err(|e| format!("invalid path: {e}"))?;

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

    let tarball = pack::pack(&pkg_dir, &pkg_json)?;

    store::save(name, version, &pkg_dir, &tarball, &pkg_json.dependencies())?;

    spinner.finish_with_message(format!(
        "{} Published {} -> {}",
        style("*").green().bold(),
        style(format!("{name}@{version}")).cyan().bold(),
        style(format!("~/.smuggle/packages/{name}/")).dim(),
    ));

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
