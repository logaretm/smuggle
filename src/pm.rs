use std::path::Path;

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn detects_pnpm_from_lockfile() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("pnpm-lock.yaml"), "").unwrap();
        assert_eq!(detect_package_manager(tmp.path()), "pnpm");
    }

    #[test]
    fn detects_yarn_from_lockfile() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("yarn.lock"), "").unwrap();
        assert_eq!(detect_package_manager(tmp.path()), "yarn");
    }

    #[test]
    fn detects_bun_from_either_lockfile() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("bun.lock"), "").unwrap();
        assert_eq!(detect_package_manager(tmp.path()), "bun");
    }

    #[test]
    fn falls_back_to_npm() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(detect_package_manager(tmp.path()), "npm");
    }

    #[test]
    fn pnpm_takes_priority_over_yarn() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("pnpm-lock.yaml"), "").unwrap();
        fs::write(tmp.path().join("yarn.lock"), "").unwrap();
        assert_eq!(detect_package_manager(tmp.path()), "pnpm");
    }

    #[test]
    fn finds_a_declared_script() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("package.json"),
            r#"{"scripts":{"dev":"vite"}}"#,
        )
        .unwrap();
        assert!(has_script(tmp.path(), "dev"));
        assert!(!has_script(tmp.path(), "build"));
    }
}
