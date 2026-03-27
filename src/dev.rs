use console::style;
use std::path::Path;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::{install, pack, pm, store, watch};

pub fn cmd_dev(
    consumer_dir: &Path,
    select_all: bool,
    restart: bool,
    command: &[String],
) -> Result<(), String> {
    let consumer_dir = consumer_dir
        .canonicalize()
        .map_err(|e| format!("invalid path: {e}"))?;

    let pkg_json_path = consumer_dir.join("package.json");
    if !pkg_json_path.exists() {
        return Err("no package.json found in consumer directory".into());
    }

    let _ = cliclack::intro(style(" smuggle dev ").on_cyan().black());

    // Resolve the dev command
    let dev_cmd = resolve_dev_command(&consumer_dir, command)?;
    let _ = cliclack::log::info(format!(
        "Dev command: {}",
        style(shell_join(&dev_cmd)).cyan()
    ));

    // Select packages (reuse install's matching logic)
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

    // Expand transitive deps
    let all_entries = install::expand_with_registered_deps(&selected, &registered);
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

    // Resolve targets
    let mut dummy_summary = crate::ci::SummaryCollector::new();
    let targets =
        install::resolve_targets(&all_refs, &consumer_dir, &[], false, &mut dummy_summary)?;
    if targets.is_empty() {
        return Err("none of the selected packages are installed in node_modules".into());
    }

    // Initial one-shot extraction (no backup)
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

    // Spawn dev server
    let child: Arc<Mutex<Option<Child>>> =
        Arc::new(Mutex::new(Some(spawn_dev_server(&dev_cmd, &consumer_dir)?)));

    // Set up ctrl-c handler
    let should_exit = Arc::new(AtomicBool::new(false));
    {
        let should_exit = should_exit.clone();
        let child = child.clone();
        let _ = ctrlc::set_handler(move || {
            should_exit.store(true, Ordering::SeqCst);
            if let Ok(mut guard) = child.lock() {
                if let Some(ref mut c) = *guard {
                    let _ = c.kill();
                    let _ = c.wait();
                }
            }
        });
    }

    cliclack::log::success(format!(
        "Watching for changes... {}",
        style("(ctrl-c to stop)").dim()
    ))
    .map_err(|e| e.to_string())?;

    if restart {
        let child_for_swap = child.clone();
        let dev_cmd_clone = dev_cmd.clone();
        let consumer_dir_clone = consumer_dir.clone();

        let mut action = watch::PostSwapAction::Notify {
            on_swap: Box::new(move |changed: &[String]| {
                let _ = cliclack::log::info(format!(
                    "Restarting dev server (changed: {})...",
                    changed.join(", ")
                ));
                if let Ok(mut guard) = child_for_swap.lock() {
                    if let Some(ref mut c) = *guard {
                        let _ = c.kill();
                        let _ = c.wait();
                    }
                    match spawn_dev_server(&dev_cmd_clone, &consumer_dir_clone) {
                        Ok(new_child) => *guard = Some(new_child),
                        Err(e) => {
                            let _ = cliclack::log::warning(format!(
                                "Failed to restart dev server: {e}"
                            ));
                            *guard = None;
                        }
                    }
                }
            }),
        };

        watch::watch_and_reinstall_until(&all_refs, &targets, &mut action, &should_exit)?;
    } else {
        let mut action = watch::PostSwapAction::ClearCachesAndTouch {
            consumer_dir: &consumer_dir,
            workspace_pkg_dirs: &[],
        };
        watch::watch_and_reinstall_until(&all_refs, &targets, &mut action, &should_exit)?;
    }

    // Kill dev server on normal exit
    if let Ok(mut guard) = child.lock() {
        if let Some(ref mut c) = *guard {
            let _ = c.kill();
            let _ = c.wait();
        }
    }

    let _ = cliclack::outro("Done");
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
