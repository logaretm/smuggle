#![allow(clippy::collapsible_if)]

mod backup;
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
            if let Err(e) = publish::cmd_publish(&pkg_dir, all || cli.all) {
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
            let consumer_dir = path
                .or(cli.path)
                .unwrap_or_else(|| std::env::current_dir().unwrap());
            let all = all || cli.all;
            if let Err(e) = install::cmd_install(&consumer_dir, all) {
                let _ = cliclack::outro(format!("{}", style(e).red()));
                std::process::exit(1);
            }
        }
        None => {
            // bare `smuggle` = `smuggle install`
            let consumer_dir = cli.path.unwrap_or_else(|| std::env::current_dir().unwrap());
            if let Err(e) = install::cmd_install(&consumer_dir, cli.all) {
                let _ = cliclack::outro(format!("{}", style(e).red()));
                std::process::exit(1);
            }
        }
    }
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
