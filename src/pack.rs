use flate2::Compression;
use flate2::write::GzEncoder;
use ignore::gitignore::GitignoreBuilder;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Default, Debug)]
pub struct PublishPackageJson {
    pub name: Option<String>,
    pub version: Option<String>,
    #[serde(default)]
    pub private: Option<bool>,
    #[serde(default)]
    files: Option<Vec<String>>,
    #[serde(default)]
    dependencies: Option<HashMap<String, String>>,
    #[serde(default, rename = "peerDependencies")]
    peer_dependencies: Option<HashMap<String, String>>,
    #[serde(default, rename = "optionalDependencies")]
    optional_dependencies: Option<HashMap<String, String>>,
    #[serde(default)]
    main: Option<String>,
    #[serde(default)]
    module: Option<String>,
    #[serde(default)]
    types: Option<String>,
    #[serde(default)]
    typings: Option<String>,
    #[serde(default)]
    bin: Option<serde_json::Value>,
    #[serde(default)]
    exports: Option<serde_json::Value>,
}

impl PublishPackageJson {
    /// Return the `files` field patterns, if present.
    pub fn files_list(&self) -> Option<&Vec<String>> {
        self.files.as_ref()
    }

    pub fn dependencies(&self) -> HashMap<String, String> {
        let mut deps = HashMap::new();
        if let Some(d) = &self.dependencies {
            deps.extend(d.clone());
        }
        if let Some(d) = &self.peer_dependencies {
            deps.extend(d.clone());
        }
        if let Some(d) = &self.optional_dependencies {
            deps.extend(d.clone());
        }
        deps
    }
}

/// Consumer-side package.json — we only care about dependency names
#[derive(Deserialize, Default)]
pub struct ConsumerPackageJson {
    #[serde(default)]
    dependencies: Option<HashMap<String, String>>,
    #[serde(default, rename = "devDependencies")]
    dev_dependencies: Option<HashMap<String, String>>,
    #[serde(default, rename = "peerDependencies")]
    peer_dependencies: Option<HashMap<String, String>>,
    #[serde(default, rename = "optionalDependencies")]
    optional_dependencies: Option<HashMap<String, String>>,
}

impl ConsumerPackageJson {
    pub fn all_dependency_names(&self) -> std::collections::HashSet<String> {
        let mut names = std::collections::HashSet::new();
        for map in [
            &self.dependencies,
            &self.dev_dependencies,
            &self.peer_dependencies,
            &self.optional_dependencies,
        ]
        .into_iter()
        .flatten()
        {
            names.extend(map.keys().cloned());
        }
        names
    }
}

/// Collect the list of files that npm would include in a pack/publish.
///
/// Priority order (matching npm behavior):
/// 1. package.json, README*, LICENSE*, LICENCE*, CHANGELOG* — always included
/// 2. If `files` field exists in package.json — whitelist those globs
/// 3. Else if .npmignore exists — use it as a blacklist
/// 4. Else if .gitignore exists — use it as a fallback blacklist
fn collect_files(pkg_dir: &Path, pkg_json: &PublishPackageJson) -> Result<Vec<PathBuf>, String> {
    let mut files: Vec<PathBuf> = Vec::new();

    // Always include package.json
    files.push(PathBuf::from("package.json"));

    // Always include README*, LICENSE*, LICENCE*, CHANGELOG*
    if let Ok(entries) = fs::read_dir(pkg_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_upper = name.to_string_lossy().to_uppercase();
            if name_upper.starts_with("README")
                || name_upper.starts_with("LICENSE")
                || name_upper.starts_with("LICENCE")
                || name_upper.starts_with("CHANGELOG")
            {
                files.push(PathBuf::from(entry.file_name()));
            }
        }
    }

    if let Some(file_patterns) = &pkg_json.files {
        // files field: whitelist mode
        for raw_pattern in file_patterns {
            // Skip negation patterns (e.g. "!dist/types.d.ts")
            if raw_pattern.starts_with('!') {
                continue;
            }

            // Patterns are always relative to package root — strip leading slashes
            let pattern = raw_pattern.trim_start_matches('/');
            let full_pattern = pkg_dir.join(pattern);
            let pattern_str = full_pattern.to_string_lossy();

            // Try as a glob
            match glob_match(pkg_dir, pattern) {
                Ok(matched) => files.extend(matched),
                Err(_) => {
                    // Try as a direct path
                    let path = PathBuf::from(pattern);
                    let abs = pkg_dir.join(&path);
                    if abs.exists() {
                        if abs.is_dir() {
                            // Include entire directory
                            collect_dir_recursive(&abs, pkg_dir, &mut files)?;
                        } else {
                            files.push(path);
                        }
                    } else {
                        eprintln!(
                            "  warn: files pattern '{}' matched nothing ({})",
                            pattern, pattern_str
                        );
                    }
                }
            }
        }

        // Also include files referenced by main, module, types, bin, exports
        collect_entry_points(pkg_dir, pkg_json, &mut files);
    } else {
        // No files field: use .npmignore or .gitignore as blacklist
        let ignore = build_ignore(pkg_dir);

        collect_with_ignore(pkg_dir, pkg_dir, &ignore, &mut files)?;
    }

    // Deduplicate
    files.sort();
    files.dedup();

    // Filter out node_modules and .git
    files.retain(|f| {
        let s = f.to_string_lossy();
        !s.starts_with("node_modules") && !s.starts_with(".git/") && s != ".git"
    });

    Ok(files)
}

fn glob_match(pkg_dir: &Path, pattern: &str) -> Result<Vec<PathBuf>, String> {
    let mut results = Vec::new();

    // Use the `glob` crate pattern matching via walkdir-like approach
    // Since we don't have the glob crate, do manual matching
    let abs_pattern = pkg_dir.join(pattern);

    if abs_pattern.exists() {
        if abs_pattern.is_dir() {
            collect_dir_recursive(&abs_pattern, pkg_dir, &mut results)?;
        } else {
            let rel = abs_pattern
                .strip_prefix(pkg_dir)
                .unwrap_or(&abs_pattern)
                .to_path_buf();
            results.push(rel);
        }
        return Ok(results);
    }

    // Try basic glob: "dist/**", "*.js", etc.
    // Walk the directory and match
    let pattern_str = pattern.trim_end_matches('/');
    walk_and_match(pkg_dir, pkg_dir, pattern_str, &mut results)?;

    if results.is_empty() {
        return Err("no matches".into());
    }

    Ok(results)
}

fn walk_and_match(
    base: &Path,
    dir: &Path,
    pattern: &str,
    results: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(());
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let rel = path.strip_prefix(base).unwrap_or(&path);
        let rel_str = rel.to_string_lossy();

        // Skip node_modules and .git
        if rel_str.starts_with("node_modules") || rel_str.starts_with(".git") {
            continue;
        }

        if path.is_dir() {
            // Check if directory name matches pattern prefix (e.g., "dist" matches "dist/**")
            walk_and_match(base, &path, pattern, results)?;
        }

        if matches_simple_glob(pattern, &rel_str) {
            if path.is_dir() {
                collect_dir_recursive(&path, base, results)?;
            } else {
                results.push(rel.to_path_buf());
            }
        }
    }

    Ok(())
}

/// Simple glob matching: supports *, **, and basic patterns
fn matches_simple_glob(pattern: &str, path: &str) -> bool {
    if pattern == "**" {
        return true;
    }

    // "dist" matches "dist" and "dist/anything"
    if !pattern.contains('*') && !pattern.contains('?') {
        return path == pattern || path.starts_with(&format!("{pattern}/"));
    }

    // "dist/**" matches anything under dist/
    if let Some(prefix) = pattern.strip_suffix("/**") {
        return path.starts_with(&format!("{prefix}/")) || path == prefix;
    }

    // "*.js" matches "foo.js" but not "dir/foo.js"
    if let Some(suffix) = pattern.strip_prefix("*.") {
        if !path.contains('/') {
            return path.ends_with(&format!(".{suffix}"));
        }
    }

    // "**/*.js" matches any .js file at any depth
    if let Some(rest) = pattern.strip_prefix("**/") {
        let file_name = path.rsplit('/').next().unwrap_or(path);
        return matches_simple_glob(rest, file_name) || matches_simple_glob(rest, path);
    }

    path == pattern
}

fn collect_dir_recursive(dir: &Path, base: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(());
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let rel = path.strip_prefix(base).unwrap_or(&path);
            let rel_str = rel.to_string_lossy();
            if rel_str.starts_with("node_modules") || rel_str.starts_with(".git") {
                continue;
            }
            collect_dir_recursive(&path, base, files)?;
        } else {
            let rel = path.strip_prefix(base).unwrap_or(&path).to_path_buf();
            files.push(rel);
        }
    }

    Ok(())
}

fn collect_entry_points(pkg_dir: &Path, pkg_json: &PublishPackageJson, files: &mut Vec<PathBuf>) {
    // Collect paths from main, module, types, typings
    for path_str in [
        &pkg_json.main,
        &pkg_json.module,
        &pkg_json.types,
        &pkg_json.typings,
    ]
    .iter()
    .copied()
    .flatten()
    {
        let p = PathBuf::from(path_str);
        if pkg_dir.join(&p).exists() {
            files.push(p);
        }
    }

    // Collect from bin
    if let Some(bin) = &pkg_json.bin {
        match bin {
            serde_json::Value::String(s) => {
                let p = PathBuf::from(s);
                if pkg_dir.join(&p).exists() {
                    files.push(p);
                }
            }
            serde_json::Value::Object(map) => {
                for v in map.values() {
                    if let Some(s) = v.as_str() {
                        let p = PathBuf::from(s);
                        if pkg_dir.join(&p).exists() {
                            files.push(p);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // Collect from exports (recursively find string values)
    if let Some(exports) = &pkg_json.exports {
        collect_export_paths(pkg_dir, exports, files);
    }
}

fn collect_export_paths(pkg_dir: &Path, value: &serde_json::Value, files: &mut Vec<PathBuf>) {
    match value {
        serde_json::Value::String(s) => {
            if s.starts_with("./") || s.starts_with("../") {
                let p = PathBuf::from(s);
                if pkg_dir.join(&p).exists() {
                    files.push(p);
                }
            }
        }
        serde_json::Value::Object(map) => {
            for v in map.values() {
                collect_export_paths(pkg_dir, v, files);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                collect_export_paths(pkg_dir, v, files);
            }
        }
        _ => {}
    }
}

fn build_ignore(pkg_dir: &Path) -> Option<ignore::gitignore::Gitignore> {
    let npmignore = pkg_dir.join(".npmignore");
    let gitignore = pkg_dir.join(".gitignore");

    let ignore_file = if npmignore.exists() {
        Some(npmignore)
    } else if gitignore.exists() {
        Some(gitignore)
    } else {
        None
    };

    ignore_file.and_then(|path| {
        let mut builder = GitignoreBuilder::new(pkg_dir);
        builder.add(&path);
        builder.build().ok()
    })
}

fn collect_with_ignore(
    base: &Path,
    dir: &Path,
    ignore: &Option<ignore::gitignore::Gitignore>,
    files: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(());
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let rel = path.strip_prefix(base).unwrap_or(&path);
        let rel_str = rel.to_string_lossy();

        // Always skip node_modules and .git
        if rel_str == "node_modules" || rel_str == ".git" {
            continue;
        }

        // Check ignore rules
        if let Some(ig) = ignore {
            let is_dir = path.is_dir();
            if ig.matched(rel, is_dir).is_ignore() {
                continue;
            }
        }

        if path.is_dir() {
            collect_with_ignore(base, &path, ignore, files)?;
        } else {
            files.push(rel.to_path_buf());
        }
    }

    Ok(())
}

/// Pack a package directory into a gzipped tarball (like `npm pack`).
/// The tarball contains files under a `package/` prefix, matching npm convention.
pub fn pack(pkg_dir: &Path, pkg_json: &PublishPackageJson) -> Result<Vec<u8>, String> {
    let files = collect_files(pkg_dir, pkg_json)?;

    if files.is_empty() {
        return Err("no files to pack".into());
    }

    let mut buf = Vec::new();
    {
        let encoder = GzEncoder::new(&mut buf, Compression::fast());
        let mut tar = tar::Builder::new(encoder);

        for rel_path in &files {
            let abs_path = pkg_dir.join(rel_path);
            if !abs_path.exists() {
                continue;
            }

            let tar_path = Path::new("package").join(rel_path);
            let metadata = fs::metadata(&abs_path)
                .map_err(|e| format!("failed to read {}: {e}", rel_path.display()))?;

            let mut header = tar::Header::new_gnu();
            header.set_size(metadata.len());
            header.set_mode(0o644);
            header.set_mtime(0); // Reproducible builds
            header.set_cksum();

            let content = fs::read(&abs_path)
                .map_err(|e| format!("failed to read {}: {e}", rel_path.display()))?;

            tar.append_data(&mut header, &tar_path, content.as_slice())
                .map_err(|e| format!("failed to add {} to tarball: {e}", rel_path.display()))?;
        }

        tar.finish()
            .map_err(|e| format!("failed to finalize tarball: {e}"))?;

        let encoder = tar
            .into_inner()
            .map_err(|e| format!("encoder error: {e}"))?;
        encoder
            .finish()
            .map_err(|e| format!("gzip finish error: {e}"))?;
    }

    Ok(buf)
}

/// Extract a tarball (created by `pack`) into a target directory.
/// Strips the `package/` prefix from tar entries.
/// Removes existing files before writing to break hard links (pnpm).
pub fn extract_tarball_to(tarball: &[u8], target_dir: &Path) -> Result<(), String> {
    use flate2::read::GzDecoder;

    let decoder = GzDecoder::new(tarball);
    let mut archive = tar::Archive::new(decoder);

    for entry in archive
        .entries()
        .map_err(|e| format!("tar read error: {e}"))?
    {
        let mut entry = entry.map_err(|e| format!("tar entry error: {e}"))?;

        let path = entry
            .path()
            .map_err(|e| format!("tar path error: {e}"))?
            .into_owned();

        // Strip "package/" prefix
        let rel = path.strip_prefix("package").unwrap_or(&path);

        if rel.as_os_str().is_empty() {
            continue;
        }

        let target = target_dir.join(rel);

        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
        }

        // Remove existing file to break hard links (pnpm uses hard links)
        let _ = fs::remove_file(&target);

        entry
            .unpack(&target)
            .map_err(|e| format!("extract {}: {e}", rel.display()))?;
    }

    Ok(())
}
