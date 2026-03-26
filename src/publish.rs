use console::style;
use std::path::Path;
use std::time::Instant;

use crate::{ci, pack, store, workspace};

pub fn cmd_publish(
    pkg_dir: &Path,
    select_all: bool,
    json: bool,
    summary: &mut ci::SummaryCollector,
) -> Result<(), String> {
    let pkg_dir = pkg_dir
        .canonicalize()
        .map_err(|e| format!("invalid path: {e}"))?;

    // In CI, always select all packages in a workspace
    let select_all = select_all || ci::is_ci();

    // Check for workspace (pnpm or yarn)
    if let Some(ws) = workspace::detect_workspace(&pkg_dir) {
        return cmd_publish_workspace(&pkg_dir, ws, select_all, json, summary);
    }

    // Single package publish
    publish_single_package(&pkg_dir, json, summary)
}

fn publish_single_package(
    pkg_dir: &Path,
    json: bool,
    summary: &mut ci::SummaryCollector,
) -> Result<(), String> {
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

    let start = Instant::now();

    if json {
        let tarball = pack::pack(pkg_dir, &pkg_json)?;
        store::save(name, version, pkg_dir, &tarball, &pkg_json.dependencies())?;
        let ms = ci::elapsed_ms(start);
        ci::emit(&ci::Event::Publish {
            package: name,
            version,
            status: ci::Status::Ok,
            error: None,
            duration_ms: Some(ms),
        });
        summary.push("publish", name, version, "ok", ms);
    } else {
        let spinner = cliclack::spinner();
        spinner.start(format!("Packing {name}@{version}..."));

        let tarball = pack::pack(pkg_dir, &pkg_json)?;
        store::save(name, version, pkg_dir, &tarball, &pkg_json.dependencies())?;

        spinner.stop(format!(
            "Published {} -> ~/.smuggle/packages/{name}/",
            style(format!("{name}@{version}")).cyan(),
        ));
    }

    Ok(())
}

fn cmd_publish_workspace(
    _root: &Path,
    ws: workspace::DetectedWorkspace,
    select_all: bool,
    json: bool,
    summary: &mut ci::SummaryCollector,
) -> Result<(), String> {
    if !json {
        let _ = cliclack::intro(style(" smuggle publish ").on_cyan().black());
        cliclack::log::info(format!("Detected {} workspace", ws.kind))
            .map_err(|e| e.to_string())?;
    }

    // Filter out the root package and private packages — they're never publishable
    let packages: Vec<workspace::WorkspacePackage> = ws
        .packages
        .into_iter()
        .filter(|p| !p.is_root && !p.is_private)
        .collect();

    if packages.is_empty() {
        return Err("no publishable packages found in workspace".into());
    }

    let selected_indices: Vec<usize> = if select_all {
        (0..packages.len()).collect()
    } else {
        let initial: Vec<usize> = vec![];

        let mut prompt = cliclack::multiselect(format!(
            "Select packages to publish {}",
            style("(space to toggle, enter to confirm)").dim()
        ));

        for (i, p) in packages.iter().enumerate() {
            let label = format!("{} @ {}", p.name, p.version);
            prompt = prompt.item(i, label, "");
        }

        prompt = prompt.initial_values(initial);

        prompt
            .interact()
            .map_err(|e| format!("selection cancelled: {e}"))?
    };

    if selected_indices.is_empty() {
        if !json {
            let _ = cliclack::outro("No packages selected, nothing to do.");
        }
        return Ok(());
    }

    // Publish each selected package
    let mut published = 0;
    let mut errors = Vec::new();

    for &idx in &selected_indices {
        let pkg = &packages[idx];
        match publish_single_package(&pkg.path, json, summary) {
            Ok(()) => published += 1,
            Err(e) => {
                if json {
                    ci::emit(&ci::Event::Publish {
                        package: &pkg.name,
                        version: &pkg.version,
                        status: ci::Status::Error,
                        error: Some(&e),
                        duration_ms: None,
                    });
                    summary.push("publish", &pkg.name, &pkg.version, "error", 0);
                } else {
                    cliclack::log::warning(format!("Failed to publish {}: {e}", pkg.name))
                        .map_err(|e| e.to_string())?;
                }
                errors.push(pkg.name.clone());
            }
        }
    }

    if json {
        ci::emit(&ci::Event::Summary {
            published,
            installed: 0,
            failed: errors.len(),
            duration_ms: Some(ci::elapsed_ms(summary.start)),
        });
    } else if errors.is_empty() {
        let _ = cliclack::outro(format!(
            "Published {} package(s)",
            style(published).green().bold()
        ));
    } else {
        let _ = cliclack::outro(format!(
            "Published {} package(s), {} failed",
            style(published).green().bold(),
            style(errors.len()).red().bold(),
        ));
    }

    if !errors.is_empty() {
        return Err(format!("failed to publish: {}", errors.join(", ")));
    }

    Ok(())
}
