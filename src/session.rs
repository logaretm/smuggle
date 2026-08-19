//! A smuggle session: the packages to answer for, and the proxy that answers.
//!
//! smuggle does not install anything. It holds a set of packages hijacked for
//! as long as the session runs, and you drive your own package manager. The
//! proxy needs root, so it runs as a child process under sudo, and it reads
//! tarballs from the store on every request, which means a repack is picked up
//! without any further coordination: the store is the control plane.

use console::style;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use std::path::Path;

use crate::{pack, store, watch};

/// A running proxy. Dropping this closes the pipe the proxy is reading, which
/// is how it learns to shut down and undo the redirect.
pub struct Proxy {
    child: Child,
}

impl Drop for Proxy {
    fn drop(&mut self) {
        // Closing stdin is the shutdown signal. The proxy runs as root, so an
        // unprivileged parent cannot signal it directly.
        drop(self.child.stdin.take());
        let _ = self.child.wait();
    }
}

/// Start the proxy with `packages` hijacked, prompting for sudo if needed.
pub fn start(packages: &[String], verbose: bool) -> Result<Proxy, String> {
    let exe =
        std::env::current_exe().map_err(|e| format!("could not locate the smuggle binary: {e}"))?;

    prime_sudo()?;

    let mut command = Command::new("sudo");
    command.arg("-n").arg(&exe).arg("proxy");
    for name in packages {
        command.arg("--hijack").arg(name);
    }
    if verbose {
        command.arg("--verbose");
    }
    command
        .arg("--exit-on-parent-close")
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let child = command
        .spawn()
        .map_err(|e| format!("failed to start the proxy: {e}"))?;

    Ok(Proxy { child })
}

/// Ask for the sudo password once, up front, so the proxy itself can be
/// launched with a piped stdin it will use only as a shutdown signal.
fn prime_sudo() -> Result<(), String> {
    let already_valid = Command::new("sudo")
        .args(["-n", "true"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if already_valid {
        return Ok(());
    }

    let _ = cliclack::log::remark("The proxy binds :443 and edits /etc/hosts, so it needs sudo");
    let status = Command::new("sudo")
        .arg("-v")
        .status()
        .map_err(|e| format!("failed to run sudo: {e}"))?;

    if !status.success() {
        return Err("sudo was declined".into());
    }
    Ok(())
}

/// Select packages and hold them hijacked until interrupted, repacking
/// whenever their sources change.
pub fn run(
    consumer_dir: &Path,
    select_all: bool,
    names: &[String],
    verbose: bool,
) -> Result<(), String> {
    let consumer_dir = consumer_dir
        .canonicalize()
        .map_err(|e| format!("invalid path: {e}"))?;

    let _ = cliclack::intro(style(" smuggle ").on_cyan().black());

    let selected = select_packages(&consumer_dir, select_all, names)?;
    let names: Vec<String> = selected.iter().map(|e| e.name.clone()).collect();
    let refs: Vec<&store::StoreEntry> = selected.iter().collect();

    let _proxy = start(&names, verbose)?;

    let _ = cliclack::log::success(format!(
        "Hijacking {}",
        style(names.join(", ")).cyan().bold(),
    ));
    let _ = cliclack::log::info(format!(
        "Run your package manager's install and it will resolve to your local copy.\nEdits are repacked automatically. Press {} to stop.",
        style("ctrl-c").cyan(),
    ));

    let shutdown = Arc::new(AtomicBool::new(false));
    let flag = shutdown.clone();
    let _ = ctrlc::set_handler(move || flag.store(true, Ordering::SeqCst));

    watch::watch_and_repack(&refs, &mut |_| {}, &shutdown)?;

    let _ = cliclack::outro("Stopped. Nothing is intercepted any more.");
    Ok(())
}

/// Resolve which registered packages this session should answer for.
///
/// Explicit names are taken as given. Otherwise the consumer's dependencies
/// are matched against the store. Either way the result is expanded with any
/// registered package the selection depends on, so a hijacked package that
/// depends on another hijacked package gets both.
fn select_packages(
    consumer_dir: &Path,
    select_all: bool,
    names: &[String],
) -> Result<Vec<store::StoreEntry>, String> {
    let registered = store::list();

    let chosen: Vec<&store::StoreEntry> = if names.is_empty() {
        let all_deps = consumer_dependencies(consumer_dir)?;
        let mut matches: Vec<&store::StoreEntry> = registered
            .iter()
            .filter(|entry| all_deps.contains(&entry.name))
            .collect();

        if matches.is_empty() {
            return Err(
                "no registered packages found in the consumer's dependencies. publish some first with `smuggle publish`."
                    .into(),
            );
        }

        matches.sort_by(|a, b| a.name.cmp(&b.name));

        if select_all {
            matches
        } else {
            prompt_for_packages(&matches)?
        }
    } else {
        let mut chosen = Vec::with_capacity(names.len());
        for name in names {
            let entry = registered.iter().find(|e| e.name == *name).ok_or_else(|| {
                format!(
                    "{} is not registered. Run {} in the package directory first.",
                    style(name).cyan(),
                    style("smuggle publish").cyan(),
                )
            })?;
            chosen.push(entry);
        }
        chosen
    };

    for entry in &chosen {
        if !store::tarball_path(&entry.name).exists() {
            return Err(format!(
                "tarball for {} is missing from the store — run `smuggle publish` again",
                style(&entry.name).cyan(),
            ));
        }
    }

    Ok(expand_with_registered_deps(&chosen, &registered))
}

fn consumer_dependencies(consumer_dir: &Path) -> Result<std::collections::HashSet<String>, String> {
    let pkg_json_path = consumer_dir.join("package.json");
    if !pkg_json_path.exists() {
        return Err("no package.json found in the consumer directory".into());
    }

    let raw = std::fs::read_to_string(&pkg_json_path).map_err(|e| e.to_string())?;
    let consumer: pack::ConsumerPackageJson = serde_json::from_str(&raw)
        .map_err(|e| format!("failed to parse the consumer package.json: {e}"))?;

    Ok(consumer.all_dependency_names())
}

fn prompt_for_packages<'a>(
    matches: &[&'a store::StoreEntry],
) -> Result<Vec<&'a store::StoreEntry>, String> {
    let mut prompt = cliclack::multiselect("Select packages to hijack");
    for (i, entry) in matches.iter().enumerate() {
        let label = format!("{} @ {}", entry.name, entry.version);
        let hint = entry.source_dir.display().to_string();
        prompt = prompt.item(i, label, hint);
    }

    let selections: Vec<usize> = prompt
        .interact()
        .map_err(|e| format!("selection cancelled: {e}"))?;

    if selections.is_empty() {
        return Err("no packages selected".into());
    }

    Ok(selections.iter().map(|&i| matches[i]).collect())
}

/// Pull in any registered package the selection depends on, transitively.
fn expand_with_registered_deps(
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn entry(name: &str, deps: &[&str]) -> store::StoreEntry {
        store::StoreEntry {
            name: name.to_string(),
            version: "1.0.0".into(),
            source_dir: PathBuf::from("/tmp"),
            dependencies: deps
                .iter()
                .map(|d| (d.to_string(), "^1.0.0".to_string()))
                .collect::<HashMap<_, _>>(),
        }
    }

    #[test]
    fn pulls_in_registered_transitive_dependencies() {
        let registered = vec![entry("a", &["b"]), entry("b", &["c"]), entry("c", &[])];
        let expanded = expand_with_registered_deps(&[&registered[0]], &registered);
        let names: Vec<&str> = expanded.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["a", "b", "c"]);
    }

    #[test]
    fn ignores_dependencies_that_are_not_registered() {
        let registered = vec![entry("a", &["not-registered"])];
        let expanded = expand_with_registered_deps(&[&registered[0]], &registered);
        let names: Vec<&str> = expanded.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["a"]);
    }

    #[test]
    fn survives_a_dependency_cycle() {
        let registered = vec![entry("a", &["b"]), entry("b", &["a"])];
        let expanded = expand_with_registered_deps(&[&registered[0]], &registered);
        let names: Vec<&str> = expanded.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["a", "b"]);
    }
}
