use console::style;
use std::path::Path;
use std::process::{Child, Command};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::{install, pm, proxy_session, watch};

/// Hold packages hijacked and run the project's dev server alongside.
pub fn cmd_dev(
    consumer_dir: &Path,
    select_all: bool,
    restart: bool,
    command: &[String],
) -> Result<(), String> {
    let consumer_dir = consumer_dir
        .canonicalize()
        .map_err(|e| format!("invalid path: {e}"))?;

    let _ = cliclack::intro(style(" smuggle dev ").on_cyan().black());

    let dev_cmd = resolve_dev_command(&consumer_dir, command)?;
    let _ = cliclack::log::info(format!(
        "Dev command: {}",
        style(shell_join(&dev_cmd)).cyan()
    ));

    let selected = install::select_packages(&consumer_dir, select_all, &[])?;
    let names: Vec<String> = selected.iter().map(|e| e.name.clone()).collect();
    let refs: Vec<&_> = selected.iter().collect();

    let _proxy = proxy_session::start(&names)?;
    let _ = cliclack::log::success(format!(
        "Smuggling {}",
        style(names.join(", ")).cyan().bold()
    ));

    let mut server = spawn_dev_server(&dev_cmd, &consumer_dir)?;

    let shutdown = Arc::new(AtomicBool::new(false));
    let flag = shutdown.clone();
    let _ = ctrlc::set_handler(move || flag.store(true, Ordering::SeqCst));

    // Scoped so the closure's borrow of `server` ends before we shut it down.
    let watched = {
        let mut on_repack = |changed: &[String]| {
            if !restart {
                return;
            }
            let _ = cliclack::log::remark(format!(
                "Restarting the dev server after {}",
                style(changed.join(", ")).cyan()
            ));
            let _ = server.kill();
            let _ = server.wait();
            match spawn_dev_server(&dev_cmd, &consumer_dir) {
                Ok(next) => server = next,
                Err(e) => {
                    let _ =
                        cliclack::log::warning(format!("could not restart the dev server: {e}"));
                }
            }
        };

        watch::watch_and_repack(&refs, &mut on_repack, &shutdown)
    };

    // Stop the dev server whether the watcher exited cleanly or failed.
    let _ = server.kill();
    let _ = server.wait();
    watched?;

    let _ = cliclack::outro("Stopped. Nothing is intercepted any more.");
    Ok(())
}

fn resolve_dev_command(consumer_dir: &Path, explicit: &[String]) -> Result<Vec<String>, String> {
    if !explicit.is_empty() {
        return Ok(explicit.to_vec());
    }

    if pm::has_script(consumer_dir, "dev") {
        let pm_name = pm::detect_package_manager(consumer_dir);
        return Ok(vec![
            pm_name.to_string(),
            "run".to_string(),
            "dev".to_string(),
        ]);
    }

    Err(format!(
        "no {} script found in package.json. Pass your dev command after {}: smuggle dev -- <command>",
        style("dev").cyan(),
        style("--").cyan(),
    ))
}

fn spawn_dev_server(cmd: &[String], cwd: &Path) -> Result<Child, String> {
    let (program, args) = cmd
        .split_first()
        .ok_or_else(|| "empty dev command".to_string())?;

    Command::new(program)
        .args(args)
        .current_dir(cwd)
        .spawn()
        .map_err(|e| format!("failed to start `{}`: {e}", shell_join(cmd)))
}

fn shell_join(parts: &[String]) -> String {
    parts
        .iter()
        .map(|s| {
            if s.contains(' ') {
                format!("\"{s}\"")
            } else {
                s.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
