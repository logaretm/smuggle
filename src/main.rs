#![allow(clippy::collapsible_if)]

mod lockfile;
mod net;
mod pack;
mod publish;
mod session;
mod setup;
mod store;
mod watch;
mod workspace;

use clap::{Parser, Subcommand};
use console::style;
use std::path::PathBuf;

/// Hidden subcommands used to re-invoke ourselves under sudo for the steps
/// that need root. Installing the daemon is the only one a user ever triggers,
/// and only once.
pub const DAEMON_INSTALL_CMD: &str = "__install-daemon";
pub const DAEMON_REMOVE_CMD: &str = "__remove-daemon";

#[derive(Parser)]
#[command(
    name = "smuggle",
    about = "Hijack npm registry requests so your local packages are served instead",
    after_help = "smuggle does not install anything. It intercepts registry traffic for the\npackages you select and serves your local copies, for as long as it runs.\nRun your own package manager's install while it is up.\n\nRun `smuggle setup` once before first use. It asks for sudo a single time to
install a background daemon; nothing after that ever asks again."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Package name(s) to hijack (e.g. `smuggle @scope/pkg`)
    names: Vec<String>,

    /// Path to the consumer project (defaults to current directory)
    #[arg(short, long, global = true)]
    path: Option<PathBuf>,

    /// Skip interactive prompts and select all matching packages
    #[arg(long, global = true)]
    all: bool,

    /// Log every request the proxy handles, not just hijacked ones
    #[arg(long, short, global = true)]
    verbose: bool,

    /// Registry URL to intercept on top of the ones npm reports. Repeatable.
    #[arg(long = "registry", global = true)]
    registries: Vec<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Pack and register a local package so it can be hijacked (non-blocking)
    Publish {
        /// Path to the package directory (defaults to current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// In a workspace, publish all non-private packages (skip interactive selection)
        #[arg(long)]
        all: bool,
    },

    /// Hijack registered packages until ctrl-c. This is what bare `smuggle` runs.
    Hijack {
        /// Package name(s) to hijack. Defaults to registered packages the project depends on.
        names: Vec<String>,

        /// Path to the consumer project (defaults to current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Skip interactive prompts and select all matching packages
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

    /// Install the local CA that lets smuggle intercept registry traffic (one-time, non-blocking)
    Setup,

    /// Remove the local CA, the shell profile entry, and any leftover registry redirect
    Cleanup,

    /// Run the interception proxy in the foreground until ctrl-c (needs sudo)
    Proxy {
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
        #[arg(long)]
        verbose: bool,

        /// Package answered from the local store instead of upstream. Repeatable.
        #[arg(long = "hijack")]
        hijack: Vec<String>,

        /// Emit machine-readable events instead of formatted lines. Used by
        /// the daemon so sessions can render the facts themselves.
        #[arg(long, hide = true)]
        events: bool,
    },

    /// Run the root daemon. Internal: started by launchd, never by hand.
    #[command(hide = true)]
    Daemon {
        /// The user allowed to drive the daemon over its control socket.
        #[arg(long)]
        owner_uid: u32,
    },

    /// Install the daemon. Internal: re-invoked under sudo by setup.
    #[command(name = DAEMON_INSTALL_CMD, hide = true)]
    InstallDaemon,

    /// Remove the daemon. Internal: re-invoked under sudo by cleanup.
    #[command(name = DAEMON_REMOVE_CMD, hide = true)]
    RemoveDaemon,
}

fn main() {
    let cli = Cli::parse();
    let all = cli.all;

    // A proxy killed with SIGKILL cannot clean up after itself, so every
    // invocation checks for a redirect whose owner is gone before doing
    // anything that might route through it.
    if !matches!(
        cli.command,
        Some(
            Commands::Daemon { .. }
                | Commands::InstallDaemon
                | Commands::RemoveDaemon
                | Commands::Cleanup
        )
    ) {
        setup::reconcile_stale_redirect();
    }

    let cwd = || std::env::current_dir().expect("current directory is readable");

    let result: Result<(), String> = match cli.command {
        Some(Commands::Publish { path, all: pub_all }) => {
            publish::cmd_publish(&path.unwrap_or_else(cwd), pub_all || all)
        }
        Some(Commands::Hijack {
            names,
            path,
            all: hijack_all,
        }) => session::run(
            &path.or(cli.path).unwrap_or_else(cwd),
            hijack_all || all,
            &names,
            &cli.registries,
            cli.verbose,
        ),
        Some(Commands::List) => {
            cmd_list();
            Ok(())
        }
        Some(Commands::Unpublish {
            name,
            all: unpub_all,
        }) => cmd_unpublish(name.as_deref(), unpub_all || all),
        Some(Commands::Setup) => setup::cmd_setup(),
        Some(Commands::Cleanup) => setup::cmd_cleanup(),
        Some(Commands::Daemon { owner_uid }) => net::daemon::run(owner_uid),
        Some(Commands::InstallDaemon) => setup::cmd_install_daemon(),
        Some(Commands::RemoveDaemon) => setup::cmd_remove_daemon(),
        Some(Commands::Proxy {
            port,
            listen,
            no_redirect,
            verbose,
            hijack,
            events,
        }) => {
            let sources: Vec<String> = if cli.registries.is_empty() {
                net::DEFAULT_REGISTRIES
                    .iter()
                    .map(|r| r.to_string())
                    .collect()
            } else {
                cli.registries.clone()
            };
            let registries = match sources
                .iter()
                .map(|url| net::Registry::parse(url))
                .collect::<Result<Vec<_>, _>>()
            {
                Ok(parsed) => parsed,
                Err(e) => {
                    let _ = cliclack::outro(format!("{}", style(&e).red()));
                    std::process::exit(1);
                }
            };
            net::proxy::run(net::proxy::Config {
                listen_ip: listen.unwrap_or_else(|| {
                    net::LISTEN_IP
                        .parse()
                        .expect("LISTEN_IP is a valid address")
                }),
                port,
                registries,
                no_redirect,
                verbose,
                hijack,
                events,
            })
        }
        // bare `smuggle` or `smuggle <names>` is the same as `smuggle hijack`
        None => session::run(
            &cli.path.unwrap_or_else(cwd),
            all,
            &cli.names,
            &cli.registries,
            cli.verbose,
        ),
    };

    if let Err(e) = result {
        let _ = cliclack::outro(format!("{}", style(e).red()));
        std::process::exit(1);
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

fn cmd_unpublish(name: Option<&str>, all: bool) -> Result<(), String> {
    let packages = store::list();

    let names = if let Some(n) = name {
        vec![n.to_string()]
    } else if packages.is_empty() {
        return Err("no packages registered".to_string());
    } else if all {
        packages.iter().map(|e| e.name.clone()).collect()
    } else {
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
