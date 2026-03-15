use std::fs;
use std::path::{Path, PathBuf};

/// A discovered workspace package.
pub struct WorkspacePackage {
    /// Absolute path to the package directory.
    pub path: PathBuf,
    /// Package name from package.json.
    pub name: String,
    /// Package version from package.json.
    pub version: String,
    /// Whether this is the workspace root package.
    pub is_root: bool,
    /// Whether package.json has "private": true.
    pub is_private: bool,
}

/// Detect pnpm workspace and return all workspace packages.
/// Returns None if not a pnpm workspace root.
pub fn detect_pnpm_workspace(dir: &Path) -> Option<Vec<WorkspacePackage>> {
    let workspace_yaml = dir.join("pnpm-workspace.yaml");
    if !workspace_yaml.exists() {
        return None;
    }

    let content = fs::read_to_string(&workspace_yaml).ok()?;
    let globs = parse_workspace_yaml(&content);

    if globs.is_empty() {
        return None;
    }

    let mut packages = Vec::new();

    // Add root package if it has a name
    if let Some(root_pkg) = read_workspace_package(dir, true) {
        packages.push(root_pkg);
    }

    // Resolve each glob pattern
    for glob in &globs {
        resolve_workspace_glob(dir, glob, &mut packages);
    }

    Some(packages)
}

/// Hand-parse pnpm-workspace.yaml to extract the packages list.
///
/// Format:
/// ```yaml
/// packages:
///   - "packages/*"
///   - "libs/*"
/// ```
///
/// Also handles unquoted values:
/// ```yaml
/// packages:
///   - packages/*
///   - libs/*
/// ```
fn parse_workspace_yaml(content: &str) -> Vec<String> {
    let mut globs = Vec::new();
    let mut in_packages = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed == "packages:" {
            in_packages = true;
            continue;
        }

        // Stop if we hit another top-level key
        if in_packages && !line.starts_with(' ') && !line.starts_with('\t') && !trimmed.is_empty() {
            break;
        }

        if in_packages {
            if let Some(value) = trimmed.strip_prefix("- ") {
                // Strip quotes if present
                let value = value.trim();
                let value = value
                    .strip_prefix('"')
                    .and_then(|v| v.strip_suffix('"'))
                    .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
                    .unwrap_or(value);

                if !value.is_empty() {
                    globs.push(value.to_string());
                }
            }
        }
    }

    globs
}

fn resolve_workspace_glob(root: &Path, glob: &str, packages: &mut Vec<WorkspacePackage>) {
    // Handle negation patterns (e.g., "!packages/internal-*")
    if glob.starts_with('!') {
        // Negation patterns exclude packages — we skip them for now
        // A proper implementation would filter previously matched packages
        return;
    }

    // Handle simple patterns like "packages/*" or "libs/**"
    // Strip trailing /** or /* to get the base directory
    let clean_glob = glob
        .trim_end_matches("/**")
        .trim_end_matches("/*")
        .trim_end_matches('/');

    if glob.contains('*') {
        // It's a glob — find the base directory (part before the first *)
        if let Some(star_pos) = clean_glob.find('*') {
            let base = &clean_glob[..star_pos].trim_end_matches('/');
            let base_dir = root.join(base);
            if base_dir.is_dir() {
                scan_for_packages(&base_dir, packages);
            }
        } else {
            // Glob was only in the suffix we stripped (e.g., "packages/*")
            let base_dir = root.join(clean_glob);
            if base_dir.is_dir() {
                scan_for_packages(&base_dir, packages);
            }
        }
    } else {
        // Direct path, not a glob
        let pkg_dir = root.join(clean_glob);
        if let Some(pkg) = read_workspace_package(&pkg_dir, false) {
            packages.push(pkg);
        }
    }
}

/// Scan a directory for immediate subdirectories that contain package.json.
fn scan_for_packages(dir: &Path, packages: &mut Vec<WorkspacePackage>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(pkg) = read_workspace_package(&path, false) {
                packages.push(pkg);
            }
        }
    }
}

fn read_workspace_package(dir: &Path, is_root: bool) -> Option<WorkspacePackage> {
    let pkg_json_path = dir.join("package.json");
    if !pkg_json_path.exists() {
        return None;
    }

    let content = fs::read_to_string(&pkg_json_path).ok()?;
    let pkg: crate::pack::PublishPackageJson = serde_json::from_str(&content).ok()?;

    let name = pkg.name?;
    let version = pkg.version.unwrap_or_else(|| "0.0.0".into());
    let is_private = pkg.private.unwrap_or(false);

    Some(WorkspacePackage {
        path: dir.to_path_buf(),
        name,
        version,
        is_root,
        is_private,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_quoted_globs() {
        let yaml = r#"
packages:
  - "packages/*"
  - "libs/*"
"#;
        assert_eq!(parse_workspace_yaml(yaml), vec!["packages/*", "libs/*"]);
    }

    #[test]
    fn parse_unquoted_globs() {
        let yaml = r#"
packages:
  - packages/*
  - libs/*
"#;
        assert_eq!(parse_workspace_yaml(yaml), vec!["packages/*", "libs/*"]);
    }

    #[test]
    fn parse_single_quoted_globs() {
        let yaml = r#"
packages:
  - 'packages/*'
"#;
        assert_eq!(parse_workspace_yaml(yaml), vec!["packages/*"]);
    }

    #[test]
    fn parse_stops_at_next_key() {
        let yaml = r#"
packages:
  - packages/*
other:
  - something
"#;
        assert_eq!(parse_workspace_yaml(yaml), vec!["packages/*"]);
    }

    #[test]
    fn parse_empty_packages() {
        let yaml = "packages:\n";
        assert!(parse_workspace_yaml(yaml).is_empty());
    }
}
