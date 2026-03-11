use console::style;
use std::path::Path;

/// Clear bundler/framework caches that might hold stale dependency artifacts.
/// Checks the root directory and all additional directories (e.g. workspace packages).
pub fn clear_bundler_caches(root: &Path, extra_dirs: &[&Path]) {
    let caches: &[(&str, &str)] = &[
        ("node_modules/.vite", "vite"),
        (".next/cache", "next.js"),
        ("node_modules/.cache", "webpack/tools"),
    ];

    let mut dirs = vec![root];
    dirs.extend_from_slice(extra_dirs);

    for dir in dirs {
        for &(cache_rel, label) in caches {
            let path = dir.join(cache_rel);
            if path.exists() {
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
}
