#![allow(clippy::collapsible_if)]

mod backup;
mod ci;
mod dev;
mod install;
mod pack;
mod pm;
mod publish;
mod store;
mod watch;
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

    /// Swap packages once and exit without watching for changes
    #[arg(long, global = true)]
    once: bool,

    /// CI mode: implies --all --once, emits NDJSON events, writes GitHub Actions summary
    #[arg(long, global = true)]
    ci: bool,
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
        /// Package name (e.g. @scope/my-pkg). If omitted, prompts for selection.
        name: Option<String>,
    },

    /// Install registered packages into a consumer project
    Install {
        /// Path to the consumer project (defaults to current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Select all matching packages without prompting
        #[arg(long)]
        all: bool,

        /// Swap packages once and exit without watching for changes
        #[arg(long)]
        once: bool,
    },

    /// Add a registered package that isn't yet in your dependencies
    Add {
        /// Package name(s) (must be registered via `smuggle publish`)
        names: Vec<String>,

        /// Add as a devDependency instead of a dependency
        #[arg(long)]
        dev: bool,

        /// Path to the consumer project (defaults to current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Swap packages once and exit without watching for changes
        #[arg(long)]
        once: bool,
    },

    /// Swap local packages and run your dev server
    Dev {
        /// Path to the consumer project (defaults to current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Select all matching packages without prompting
        #[arg(long)]
        all: bool,

        /// Kill and restart the dev server on each package change (instead of relying on HMR)
        #[arg(long)]
        restart: bool,

        /// Dev server command (auto-detected from package.json "dev" script if omitted)
        #[arg(last = true)]
        command: Vec<String>,
    },
}

fn main() {
    let cli = Cli::parse();

    let ci = cli.ci;
    let all = cli.all || ci;
    let once = cli.once || ci;

    let mut summary = ci::SummaryCollector::new();

    let result: Result<(), String> = match cli.command {
        Some(Commands::Publish { path, all: pub_all }) => {
            let pkg_dir = path.unwrap_or_else(|| std::env::current_dir().unwrap());
            publish::cmd_publish(&pkg_dir, pub_all || all, ci, &mut summary)
        }
        Some(Commands::List) => {
            cmd_list(ci);
            Ok(())
        }
        Some(Commands::Unpublish { name }) => cmd_unpublish(name.as_deref()),
        Some(Commands::Install {
            path,
            all: inst_all,
            once: inst_once,
        }) => {
            let consumer_dir = path
                .or(cli.path)
                .unwrap_or_else(|| std::env::current_dir().unwrap());
            let all = inst_all || all;
            let once = inst_once || once;
            install::cmd_install(&consumer_dir, all, once, ci, &mut summary)
        }
        Some(Commands::Dev {
            path,
            all,
            restart,
            command,
        }) => {
            let consumer_dir = path
                .or(cli.path)
                .unwrap_or_else(|| std::env::current_dir().unwrap());
            let all = all || cli.all;
            if let Err(e) = dev::cmd_dev(&consumer_dir, all, restart, &command) {
                let _ = cliclack::outro(format!("{}", style(e).red()));
                std::process::exit(1);
            }
            Ok(())
        }
        Some(Commands::Add {
            names,
            dev,
            path,
            once: add_once,
        }) => {
            let consumer_dir = path
                .or(cli.path)
                .unwrap_or_else(|| std::env::current_dir().unwrap());
            let once = add_once || once;
            install::cmd_add(&consumer_dir, &names, dev, once)
        }
        None => {
            // bare `smuggle` = `smuggle install`
            let consumer_dir = cli.path.unwrap_or_else(|| std::env::current_dir().unwrap());
            install::cmd_install(&consumer_dir, all, once, ci, &mut summary)
        }
    };

    // Write GitHub Actions job summary if applicable
    if ci {
        summary.write_github_summary();
    }

    if let Err(e) = result {
        if ci {
            ci::emit(&ci::Event::Error { message: &e });
        } else {
            let _ = cliclack::outro(format!("{}", style(e).red()));
        }
        std::process::exit(1);
    }
}

fn cmd_list(ci: bool) {
    let packages = store::list();

    if ci {
        // Output as a JSON array
        if let Ok(out) = serde_json::to_string(&packages) {
            println!("{out}");
        }
        return;
    }

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

fn cmd_unpublish(name: Option<&str>) -> Result<(), String> {
    let names = match name {
        Some(n) => vec![n.to_string()],
        None => {
            let packages = store::list();
            if packages.is_empty() {
                return Err("no packages registered".to_string());
            }

            let mut prompt = cliclack::multiselect("Select packages to remove");
            for (i, entry) in packages.iter().enumerate() {
                let label = format!("{} @ {}", entry.name, entry.version);
                let hint = entry.source_dir.display().to_string();
                prompt = prompt.item(i, label, hint);
            }

            let selections: Vec<usize> = prompt
                .interact()
                .map_err(|e| format!("selection cancelled: {e}"))?;

            if selections.is_empty() {
                return Err("no packages selected".to_string());
            }

            selections.iter().map(|&i| packages[i].name.clone()).collect()
        }
    };

    for name in &names {
        store::remove(name)?;
        let _ = cliclack::log::success(format!("Removed {} from local store", style(name).cyan()));
    }

    Ok(())
}
