use console::style;
use std::path::{Path, PathBuf};

/// All bundler/framework cache directories we know about.
/// Each entry is (relative_path, human_label).
const CACHE_DIRS: &[(&str, &str)] = &[
    ("node_modules/.vite", "vite"),
    (".next/cache", "next.js"),
    ("node_modules/.cache", "webpack/tools"),
    (".turbo", "turbopack"),
    ("node_modules/.rspack", "rspack"),
    (".parcel-cache", "parcel"),
];

/// Vite config file names in priority order.
const VITE_CONFIG_NAMES: &[&str] = &[
    "vite.config.ts",
    "vite.config.js",
    "vite.config.mts",
    "vite.config.mjs",
];

/// Return the list of cache directories that exist under `base`.
pub fn detect_cache_dirs(base: &Path) -> Vec<(PathBuf, &'static str)> {
    CACHE_DIRS
        .iter()
        .filter_map(|&(rel, label)| {
            let path = base.join(rel);
            if path.exists() {
                Some((path, label))
            } else {
                None
            }
        })
        .collect()
}

/// Detect which vite config file exists in `dir`, returning the first match.
pub fn detect_vite_config(dir: &Path) -> Option<PathBuf> {
    VITE_CONFIG_NAMES.iter().find_map(|name| {
        let path = dir.join(name);
        if path.exists() { Some(path) } else { None }
    })
}

/// Clear bundler/framework caches that might hold stale dependency artifacts.
/// Checks the root directory and all additional directories (e.g. workspace packages).
pub fn clear_bundler_caches(root: &Path, extra_dirs: &[&Path]) {
    let mut dirs = vec![root];
    dirs.extend_from_slice(extra_dirs);

    for dir in dirs {
        for (path, label) in detect_cache_dirs(dir) {
            let display = path.strip_prefix(root).unwrap_or(&path);
            match std::fs::remove_dir_all(&path) {
                Ok(()) => {
                    let _ = cliclack::log::remark(format!(
                        "cleared {} cache ({})",
                        style(label).dim(),
                        style(display.display()).dim(),
                    ));
                }
                Err(e) => {
                    let _ = cliclack::log::warning(format!(
                        "failed to clear {} cache at {}: {e}",
                        label,
                        display.display(),
                    ));
                }
            }
        }
    }
}

/// Detect which package manager is used in the given directory.
/// Checks for lockfiles in order: pnpm, yarn, bun, npm (fallback).
pub fn detect_package_manager(dir: &Path) -> &'static str {
    if dir.join("pnpm-lock.yaml").exists() {
        "pnpm"
    } else if dir.join("yarn.lock").exists() {
        "yarn"
    } else if dir.join("bun.lockb").exists() || dir.join("bun.lock").exists() {
        "bun"
    } else {
        "npm"
    }
}

/// Return the lockfile paths that may exist for a given package manager.
/// We back up every candidate a PM may touch (e.g. pnpm may create both
/// pnpm-lock.yaml and node_modules/.modules.yaml).
pub fn lockfile_candidates(dir: &Path, pm: &str) -> Vec<PathBuf> {
    match pm {
        "pnpm" => vec![dir.join("pnpm-lock.yaml")],
        "yarn" => vec![dir.join("yarn.lock")],
        "bun" => vec![dir.join("bun.lockb"), dir.join("bun.lock")],
        _ => vec![dir.join("package-lock.json")],
    }
}

/// Run the package manager's install in `consumer_dir`. Used after writing
/// a `file:` reference into package.json so the PM resolves and installs
/// the smuggled package's transitive deps. Callers must back up the
/// lockfile beforehand and restore it afterwards.
pub fn run_install(consumer_dir: &Path) -> Result<(), String> {
    let pm = detect_package_manager(consumer_dir);
    let (cmd, args): (&str, &[&str]) = match pm {
        "pnpm" => ("pnpm", &["install", "--no-frozen-lockfile"]),
        "yarn" => ("yarn", &["install"]),
        "bun" => ("bun", &["install"]),
        _ => ("npm", &["install"]),
    };

    let status = std::process::Command::new(cmd)
        .args(args)
        .current_dir(consumer_dir)
        .status()
        .map_err(|e| format!("failed to run `{cmd} {}`: {e}", args.join(" ")))?;

    if !status.success() {
        return Err(format!(
            "`{cmd} {}` exited with {}",
            args.join(" "),
            status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "signal".into()),
        ));
    }
    Ok(())
}

/// Check if a package.json in the given directory has a specific script.
pub fn has_script(dir: &Path, script: &str) -> bool {
    let pkg_json_path = dir.join("package.json");
    let Ok(raw) = std::fs::read_to_string(pkg_json_path) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return false;
    };
    value
        .get("scripts")
        .and_then(|s| s.get(script))
        .and_then(|v| v.as_str())
        .is_some()
}

/// Touch the vite config file (if any) to trigger a dev server restart.
/// Checks root and all workspace member directories.
pub fn touch_vite_configs(root: &Path, workspace_pkg_dirs: &[PathBuf]) {
    let mut dirs = vec![root.to_path_buf()];
    dirs.extend_from_slice(workspace_pkg_dirs);

    for dir in &dirs {
        if let Some(config_path) = detect_vite_config(dir) {
            let now = filetime::FileTime::now();
            let _ = filetime::set_file_mtime(&config_path, now);
            let display = config_path.strip_prefix(root).unwrap_or(&config_path);
            let _ = cliclack::log::remark(format!(
                "touched {} to trigger reload",
                style(display.display()).dim(),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn detect_cache_dirs_finds_vite() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("node_modules/.vite")).unwrap();

        let found = detect_cache_dirs(root);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].1, "vite");
    }

    #[test]
    fn detect_cache_dirs_finds_turbopack() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join(".turbo")).unwrap();

        let found = detect_cache_dirs(root);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].1, "turbopack");
    }

    #[test]
    fn detect_cache_dirs_finds_rspack() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("node_modules/.rspack")).unwrap();

        let found = detect_cache_dirs(root);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].1, "rspack");
    }

    #[test]
    fn detect_cache_dirs_finds_parcel() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join(".parcel-cache")).unwrap();

        let found = detect_cache_dirs(root);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].1, "parcel");
    }

    #[test]
    fn detect_cache_dirs_finds_multiple() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("node_modules/.vite")).unwrap();
        fs::create_dir_all(root.join(".next/cache")).unwrap();
        fs::create_dir_all(root.join(".turbo")).unwrap();
        fs::create_dir_all(root.join(".parcel-cache")).unwrap();

        let found = detect_cache_dirs(root);
        assert_eq!(found.len(), 4);
        let labels: Vec<&str> = found.iter().map(|(_, l)| *l).collect();
        assert!(labels.contains(&"vite"));
        assert!(labels.contains(&"next.js"));
        assert!(labels.contains(&"turbopack"));
        assert!(labels.contains(&"parcel"));
    }

    #[test]
    fn detect_cache_dirs_returns_empty_when_none_exist() {
        let tmp = tempfile::tempdir().unwrap();
        let found = detect_cache_dirs(tmp.path());
        assert!(found.is_empty());
    }

    #[test]
    fn detect_vite_config_finds_ts() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join("vite.config.ts"), "").unwrap();

        let result = detect_vite_config(root);
        assert_eq!(result, Some(root.join("vite.config.ts")));
    }

    #[test]
    fn detect_vite_config_finds_js() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join("vite.config.js"), "").unwrap();

        let result = detect_vite_config(root);
        assert_eq!(result, Some(root.join("vite.config.js")));
    }

    #[test]
    fn detect_vite_config_finds_mts() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join("vite.config.mts"), "").unwrap();

        let result = detect_vite_config(root);
        assert_eq!(result, Some(root.join("vite.config.mts")));
    }

    #[test]
    fn detect_vite_config_finds_mjs() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join("vite.config.mjs"), "").unwrap();

        let result = detect_vite_config(root);
        assert_eq!(result, Some(root.join("vite.config.mjs")));
    }

    #[test]
    fn detect_vite_config_prefers_ts_over_js() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join("vite.config.ts"), "").unwrap();
        fs::write(root.join("vite.config.js"), "").unwrap();

        let result = detect_vite_config(root);
        assert_eq!(result, Some(root.join("vite.config.ts")));
    }

    #[test]
    fn detect_vite_config_returns_none_when_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let result = detect_vite_config(tmp.path());
        assert_eq!(result, None);
    }
}
