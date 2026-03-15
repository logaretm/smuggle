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
