//! A smuggle session: the packages to answer for, and the proxy that answers.
//!
//! smuggle does not install anything. It holds a set of packages hijacked for
//! as long as the session runs, and you drive your own package manager. The
//! proxy needs root, so it runs as a child process under sudo, and it reads
//! tarballs from the store on every request, which means a repack is picked up
//! without any further coordination: the store is the control plane.

use console::style;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use std::path::Path;

use std::collections::HashMap;

use crate::net::{control, hijack};
use crate::{lockfile, pack, store, watch};

/// A registration with the daemon. Dropping it closes the connection, which
/// is how the daemon learns the session is over and takes the redirect down.
pub struct Registration {
    stream: UnixStream,
    registries: Vec<String>,
}

impl Registration {
    /// Change what this session hijacks, without dropping the connection.
    /// Reconnecting would leave the daemon briefly with no sessions, which
    /// would tear the proxy down and rebuild it on every change.
    pub fn set_packages(&mut self, packages: &[String]) -> Result<(), String> {
        let request = serde_json::to_string(&control::Request::Register {
            version: control::version(),
            packages: packages.to_vec(),
            registries: self.registries.clone(),
            verbose: false,
        })
        .map_err(|e| format!("could not encode the request: {e}"))?;

        writeln!(&self.stream, "{request}").map_err(|e| format!("could not reach the daemon: {e}"))
    }
}

/// Register with the daemon so it hijacks `packages` for as long as we live.
pub fn start(
    packages: &[String],
    registries: &[String],
    verbose: bool,
) -> Result<Registration, String> {
    start_with_sink(packages, registries, verbose, None)
}

/// As [`start`], but delivering replies to `sink` instead of stdout.
///
/// Events reach the connection that registered, so a UI has to read them here
/// rather than opening a second connection, which would receive nothing.
pub fn start_with_sink(
    packages: &[String],
    registries: &[String],
    verbose: bool,
    sink: Option<std::sync::mpsc::Sender<control::Reply>>,
) -> Result<Registration, String> {
    let path = control::socket_path();
    let stream = UnixStream::connect(&path).map_err(|e| {
        if !crate::net::launchd::is_installed() {
            "the smuggle daemon is not installed. Run `smuggle setup` first.".to_string()
        } else if !crate::net::launchd::is_loaded() {
            "the smuggle daemon is installed but not running. Run `smuggle setup` again."
                .to_string()
        } else {
            format!(
                "could not reach the smuggle daemon at {} ({e})",
                path.display()
            )
        }
    })?;

    let mut writer = stream
        .try_clone()
        .map_err(|e| format!("could not use the daemon socket: {e}"))?;

    let request = serde_json::to_string(&control::Request::Register {
        version: control::version(),
        packages: packages.to_vec(),
        registries: registries.to_vec(),
        verbose,
    })
    .map_err(|e| format!("could not encode the request: {e}"))?;

    writeln!(writer, "{request}").map_err(|e| format!("could not talk to the daemon: {e}"))?;

    let mut reader = BufReader::new(
        stream
            .try_clone()
            .map_err(|e| format!("could not use the daemon socket: {e}"))?,
    );

    // The first reply says whether we are registered; everything after it is
    // proxy output to print.
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|e| format!("the daemon closed the connection: {e}"))?;

    match serde_json::from_str::<control::Reply>(line.trim()) {
        Ok(control::Reply::Ok) => {}
        Ok(control::Reply::Error { message }) => return Err(message),
        Ok(_) | Err(_) => {
            return Err("unexpected reply from the daemon".into());
        }
    }

    std::thread::spawn(move || {
        for line in reader.lines().map_while(Result::ok) {
            let Ok(reply) = serde_json::from_str::<control::Reply>(&line) else {
                continue;
            };
            match &sink {
                // A UI renders replies itself; printing here would corrupt it.
                Some(sink) => {
                    if sink.send(reply).is_err() {
                        break;
                    }
                }
                None => match reply {
                    control::Reply::Event { event } => println!("{}", event.to_line()),
                    control::Reply::Log { line } => println!("{line}"),
                    _ => {}
                },
            }
        }
    });

    Ok(Registration {
        stream,
        registries: registries.to_vec(),
    })
}

/// Ask npm which registries this project actually resolves through.
///
/// A configured registry is easy to miss: npm reads `.npmrc` from the project,
/// the user's home, and the npm prefix, and a corporate mirror there means
/// registry.npmjs.org is never contacted at all. Asking npm is the only way to
/// see the same answer it will use.
pub fn detect_registries(consumer_dir: &Path) -> Vec<String> {
    let Ok(output) = Command::new("npm")
        .args(["config", "list", "--json"])
        .current_dir(consumer_dir)
        .output()
    else {
        return Vec::new();
    };

    let Ok(config) = serde_json::from_slice::<serde_json::Value>(&output.stdout) else {
        return Vec::new();
    };
    let Some(map) = config.as_object() else {
        return Vec::new();
    };

    let mut found: Vec<String> = map
        .iter()
        .filter(|(key, _)| *key == "registry" || key.ends_with(":registry"))
        .filter_map(|(_, value)| value.as_str())
        .map(str::to_string)
        .collect();

    found.sort();
    found.dedup();
    found
}

/// Select packages and hold them hijacked until interrupted, repacking
/// whenever their sources change.
pub fn run(
    consumer_dir: &Path,
    select_all: bool,
    names: &[String],
    extra_registries: &[String],
    verbose: bool,
) -> Result<(), String> {
    let consumer_dir = consumer_dir
        .canonicalize()
        .map_err(|e| format!("invalid path: {e}"))?;

    let _ = cliclack::intro(style(" smuggle ").on_cyan().black());

    let selected = select_packages(&consumer_dir, select_all, names)?;
    let names: Vec<String> = selected.iter().map(|e| e.name.clone()).collect();
    let refs: Vec<&store::StoreEntry> = selected.iter().collect();

    // Fail before touching anything if we cannot pin this project's lockfile,
    // since without pinning the package manager resolves from cache and the
    // proxy is never consulted.
    let lock = lockfile::detect(&consumer_dir)?;

    let registries = resolve_registries(&consumer_dir, extra_registries);
    let registration = start(&names, &registries, verbose)?;

    let _ = cliclack::log::success(format!(
        "Hijacking {}",
        style(names.join(", ")).cyan().bold(),
    ));

    // The pin is only satisfiable while the proxy runs, so it is undone during
    // teardown below, before interception stops.
    let pin = lockfile::pin(&lock, &tarball_hashes(&names), true)?;
    lockfile::install(&consumer_dir, lock.kind)?;

    let _ = cliclack::log::info(format!(
        "Your local build is installed. Edits are repacked and reinstalled automatically.\nPress {} to stop and put the real packages back.",
        style("ctrl-c").cyan(),
    ));

    let shutdown = Arc::new(AtomicBool::new(false));
    let flag = shutdown.clone();
    let _ = ctrlc::set_handler(move || flag.store(true, Ordering::SeqCst));

    // A repack changes the tarball, so the pin has to be rewritten and the
    // install re-run for the new bytes to reach node_modules.
    let mut on_repack = |_changed: &[String]| {
        if let Err(e) = repin(&lock, &names, &consumer_dir) {
            let _ = cliclack::log::warning(e);
        }
    };

    let watched = watch::watch_and_repack(&refs, &mut on_repack, &shutdown);

    // Tear down even if the watcher failed, otherwise a pinned lockfile
    // outlives the session that could satisfy it.
    teardown(pin, registration, &consumer_dir, lock.kind);
    watched
}

/// Put the project back the way it was found.
///
/// Restoring the lockfile is not enough on its own: node_modules still holds
/// the smuggled build, so the project keeps running local code until something
/// reinstalls. Reinstalling here is cheap, because the original tarball is
/// still in the package manager's cache under its original integrity.
fn teardown(
    pin: lockfile::Pin,
    registration: Registration,
    consumer_dir: &Path,
    kind: lockfile::Kind,
) {
    // Order matters. The lockfile goes back to the real integrity first, then
    // interception stops, and only then is it safe to reinstall: a fetch made
    // while the proxy is still up would return smuggled bytes and fail the
    // restored integrity check.
    drop(pin);
    drop(registration);

    if !wait_for_interception_to_stop() {
        let _ = cliclack::log::warning(
            "Interception is still active, so the packages may not restore cleanly. Another smuggle session may be running.",
        );
    }

    let _ = cliclack::log::remark("Putting the real packages back");
    match lockfile::install(consumer_dir, kind) {
        Ok(()) => {
            let _ = cliclack::outro("Stopped. Your project is back to its published dependencies.");
        }
        Err(e) => {
            let _ = cliclack::log::warning(format!("could not reinstall: {e}"));
            let _ = cliclack::outro(
                "Stopped, but node_modules may still hold your local build. Run your package manager's install.",
            );
        }
    }
}

/// The daemon tears the redirect down asynchronously once we deregister, so
/// give it a moment before reinstalling. Returns false if it is still up,
/// which is legitimate when another session holds it.
pub fn wait_for_interception_to_stop() -> bool {
    for _ in 0..30 {
        if crate::net::hosts::read_block().is_none() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    false
}

/// Re-pin and reinstall after a repack.
fn repin(lock: &lockfile::Lockfile, names: &[String], consumer_dir: &Path) -> Result<(), String> {
    let original = std::fs::read_to_string(&lock.path)
        .map_err(|e| format!("could not read {}: {e}", lock.path.display()))?;
    let (rewritten, _) = lockfile::rewrite(lock.kind, &original, &tarball_hashes(names))?;
    std::fs::write(&lock.path, rewritten)
        .map_err(|e| format!("could not write {}: {e}", lock.path.display()))?;

    lockfile::install(consumer_dir, lock.kind)
}

/// The integrity of what the proxy will serve for each package, which is what
/// the lockfile has to claim for the fetch to be accepted.
fn tarball_hashes(names: &[String]) -> HashMap<String, String> {
    names
        .iter()
        .filter_map(|name| {
            let tarball = store::load_tarball(name).ok()?;
            Some((name.clone(), hijack::integrity_of(&tarball)))
        })
        .collect()
}

/// The registries to intercept: whatever npm is configured to use, plus the
/// public defaults so a project that switches registries mid-session, or pulls
/// a scoped package from elsewhere, is still covered.
fn resolve_registries(consumer_dir: &Path, extra: &[String]) -> Vec<String> {
    let detected = detect_registries(consumer_dir);

    for url in &detected {
        if !url.contains("registry.npmjs.org") {
            let _ = cliclack::log::info(format!("npm is configured to use {}", style(url).cyan()));
        }
    }

    merge_registries(detected, extra)
}

/// As [`resolve_registries`], but printing nothing. A UI owns the terminal, so
/// stray log lines would corrupt the frame.
pub fn resolve_registries_quietly(consumer_dir: &Path, extra: &[String]) -> Vec<String> {
    merge_registries(detect_registries(consumer_dir), extra)
}

fn merge_registries(detected: Vec<String>, extra: &[String]) -> Vec<String> {
    let mut all = detected;
    all.extend(extra.iter().cloned());
    all.extend(crate::net::DEFAULT_REGISTRIES.iter().map(|r| r.to_string()));
    all.sort();
    all.dedup();
    all
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
