#![allow(clippy::collapsible_if)]

mod backup;
mod ci;
mod dev;
mod install;
mod net;
mod pack;
mod pm;
mod publish;
mod setup;
mod store;
mod watch;
mod workspace;

use clap::{Parser, Subcommand};
use console::style;
use std::path::PathBuf;

/// Hidden subcommand used to re-invoke ourselves under sudo for the one step
/// that needs root: editing the /etc/hosts redirect.
pub const HOSTS_CLEAR_CMD: &str = "__hosts-clear";

#[derive(Parser)]
#[command(
    name = "smuggle",
    about = "Smuggle local npm packages into your projects — no symlinks, no lockfile pollution",
    after_help = "By default, install/dev start a file watcher that blocks until you press ctrl-c.\nUse --once to swap packages and exit immediately (useful for scripts and non-interactive environments).\nUse --ci for CI pipelines (implies --all --once, emits NDJSON, writes GitHub Actions summary)."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Package name(s) to install (e.g. `smuggle @scope/pkg`)
    names: Vec<String>,

    /// Path to the consumer project (defaults to current directory)
    #[arg(short, long, global = true)]
    path: Option<PathBuf>,

    /// Skip interactive prompts and select all matching packages automatically
    #[arg(long, global = true)]
    all: bool,

    /// Swap packages once and exit immediately (without starting the file watcher)
    #[arg(long, global = true)]
    once: bool,

    /// CI mode: implies --all --once, emits NDJSON events to stdout, writes GitHub Actions job summary
    #[arg(long, global = true)]
    ci: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Pack and register a local package for later use (non-blocking, exits after packing)
    Publish {
        /// Path to the package directory (defaults to current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// In a workspace, publish all non-private packages (skip interactive selection)
        #[arg(long)]
        all: bool,
    },

    /// List all registered local packages (non-blocking)
    List,

    /// Remove a registered package (non-blocking)
    Unpublish {
        /// Package name (e.g. @scope/my-pkg). If omitted, shows interactive selection.
        name: Option<String>,

        /// Remove all registered packages (skip interactive selection)
        #[arg(long)]
        all: bool,
    },

    /// Swap registered packages into node_modules (blocks with file watcher unless --once is passed)
    Install {
        /// Package name(s) to install. If not in package.json, they will be injected temporarily.
        names: Vec<String>,

        /// Path to the consumer project (defaults to current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Skip interactive prompts and select all matching packages
        #[arg(long)]
        all: bool,

        /// Swap packages and exit immediately (don't start the file watcher)
        #[arg(long)]
        once: bool,

        /// Add new packages as devDependencies instead of dependencies
        #[arg(long)]
        dev: bool,
    },

    /// Swap local packages and run your dev server (blocks until ctrl-c)
    Dev {
        /// Path to the consumer project (defaults to current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Skip interactive prompts and select all matching packages
        #[arg(long)]
        all: bool,

        /// Kill and restart the dev server on each package change (instead of relying on HMR)
        #[arg(long)]
        restart: bool,

        /// Dev server command (auto-detected from package.json "dev" script if omitted)
        #[arg(last = true)]
        command: Vec<String>,
    },

    /// Install the local CA that lets smuggle intercept registry traffic (one-time, non-blocking)
    Setup,

    /// Run the interception proxy in the foreground until ctrl-c (needs sudo)
    Proxy {
        /// Registry host to intercept. Repeatable. Defaults to the npm and yarn registries.
        #[arg(long = "host")]
        hosts: Vec<String>,

        /// Port to listen on (default 443)
        #[arg(long, default_value_t = 443)]
        port: u16,

        /// Loopback address to listen on (default 127.0.0.2, added to lo0 while running)
        #[arg(long)]
        listen: Option<std::net::IpAddr>,

        /// Listen without editing /etc/hosts. Nothing is intercepted; clients
        /// have to be pointed at the proxy explicitly.
        #[arg(long)]
        no_redirect: bool,

        /// Log every request that passes through
        #[arg(long, short)]
        verbose: bool,
    },

    /// Remove the local CA, the shell profile entry, and any leftover registry redirect
    Cleanup,

    /// Clear the /etc/hosts redirect. Internal: re-invoked under sudo by cleanup.
    #[command(name = HOSTS_CLEAR_CMD, hide = true)]
    HostsClear,
}

fn main() {
    let cli = Cli::parse();

    let ci = cli.ci;
    let all = cli.all || ci;
    let once = cli.once || ci;

    let mut summary = ci::SummaryCollector::new();

    // A proxy killed with SIGKILL cannot clean up after itself, so every
    // invocation checks for a redirect whose owner is gone before doing
    // anything that might route through it.
    if !ci && !matches!(cli.command, Some(Commands::HostsClear | Commands::Cleanup)) {
        setup::reconcile_stale_redirect();
    }

    let result: Result<(), String> = match cli.command {
        Some(Commands::Publish { path, all: pub_all }) => {
            let pkg_dir = path.unwrap_or_else(|| std::env::current_dir().unwrap());
            publish::cmd_publish(&pkg_dir, pub_all || all, ci, &mut summary)
        }
        Some(Commands::List) => {
            cmd_list(ci);
            Ok(())
        }
        Some(Commands::Unpublish {
            name,
            all: unpub_all,
        }) => cmd_unpublish(name.as_deref(), unpub_all || all),
        Some(Commands::Install {
            names,
            path,
            all: inst_all,
            once: inst_once,
            dev,
        }) => {
            let consumer_dir = path
                .or(cli.path)
                .unwrap_or_else(|| std::env::current_dir().unwrap());
            let all = inst_all || all;
            let once = inst_once || once;
            install::cmd_install(&consumer_dir, all, once, ci, dev, &names, &mut summary)
        }
        Some(Commands::Setup) => setup::cmd_setup(),
        Some(Commands::Cleanup) => setup::cmd_cleanup(),
        Some(Commands::Proxy {
            hosts,
            port,
            listen,
            no_redirect,
            verbose,
        }) => {
            let hosts = if hosts.is_empty() {
                net::DEFAULT_REGISTRY_HOSTS
                    .iter()
                    .map(|h| h.to_string())
                    .collect()
            } else {
                hosts
            };
            net::proxy::run(net::proxy::Config {
                listen_ip: listen.unwrap_or_else(|| {
                    net::LISTEN_IP
                        .parse()
                        .expect("LISTEN_IP is a valid address")
                }),
                port,
                hosts,
                no_redirect,
                verbose,
            })
        }
        Some(Commands::HostsClear) => net::hosts::remove(),
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
        None => {
            // bare `smuggle` or `smuggle <names>` = `smuggle install`
            let consumer_dir = cli.path.unwrap_or_else(|| std::env::current_dir().unwrap());
            install::cmd_install(
                &consumer_dir,
                all,
                once,
                ci,
                false,
                &cli.names,
                &mut summary,
            )
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

fn cmd_unpublish(name: Option<&str>, all: bool) -> Result<(), String> {
    let packages = store::list();

    let names = if let Some(n) = name {
        vec![n.to_string()]
    } else if all {
        if packages.is_empty() {
            return Err("no packages registered".to_string());
        }
        packages.iter().map(|e| e.name.clone()).collect()
    } else {
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

        selections
            .iter()
            .map(|&i| packages[i].name.clone())
            .collect()
    };

    for name in &names {
        store::remove(name)?;
        let _ = cliclack::log::success(format!("Removed {} from local store", style(name).cyan()));
    }

    Ok(())
}
