use console::style;
use std::path::Path;

use crate::{pack, store, workspace};

pub fn cmd_publish(pkg_dir: &Path, select_all: bool) -> Result<(), String> {
    let pkg_dir = pkg_dir
        .canonicalize()
        .map_err(|e| format!("invalid path: {e}"))?;

    // Check for workspace (pnpm or yarn)
    if let Some(ws) = workspace::detect_workspace(&pkg_dir) {
        return cmd_publish_workspace(&pkg_dir, ws, select_all);
    }

    // Single package publish
    publish_single_package(&pkg_dir)
}

fn publish_single_package(pkg_dir: &Path) -> Result<(), String> {
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

    let spinner = cliclack::spinner();
    spinner.start(format!("Packing {name}@{version}..."));

    let tarball = pack::pack(pkg_dir, &pkg_json)?;

    store::save(name, version, pkg_dir, &tarball, &pkg_json.dependencies())?;

    spinner.stop(format!(
        "Published {} -> ~/.smuggle/packages/{name}/",
        style(format!("{name}@{version}")).cyan(),
    ));

    Ok(())
}

fn cmd_publish_workspace(
    _root: &Path,
    ws: workspace::DetectedWorkspace,
    select_all: bool,
) -> Result<(), String> {
    let _ = cliclack::intro(style(" smuggle publish ").on_cyan().black());

    cliclack::log::info(format!("Detected {} workspace", ws.kind)).map_err(|e| e.to_string())?;

    // Filter out the root package and private packages — they're never publishable
    let packages: Vec<workspace::WorkspacePackage> = ws
        .packages
        .into_iter()
        .filter(|p| !p.is_root && !p.is_private)
        .collect();

    if packages.is_empty() {
        return Err("no publishable packages found in workspace".into());
    }

    let initial: Vec<usize> = if select_all {
        (0..packages.len()).collect()
    } else {
        vec![]
    };

    let mut prompt = cliclack::multiselect(format!(
        "Select packages to publish {}",
        style("(space to toggle, enter to confirm)").dim()
    ));

    for (i, p) in packages.iter().enumerate() {
        let label = format!("{} @ {}", p.name, p.version);
        prompt = prompt.item(i, label, "");
    }

    prompt = prompt.initial_values(initial);

    let selected_indices: Vec<usize> = prompt
        .interact()
        .map_err(|e| format!("selection cancelled: {e}"))?;

    if selected_indices.is_empty() {
        let _ = cliclack::outro("No packages selected, nothing to do.");
        return Ok(());
    }

    // Publish each selected package
    let mut published = 0;
    let mut errors = Vec::new();

    for &idx in &selected_indices {
        let pkg = &packages[idx];
        match publish_single_package(&pkg.path) {
            Ok(()) => published += 1,
            Err(e) => {
                cliclack::log::warning(format!("Failed to publish {}: {e}", pkg.name))
                    .map_err(|e| e.to_string())?;
                errors.push(pkg.name.clone());
            }
        }
    }

    if errors.is_empty() {
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

    Ok(())
}
