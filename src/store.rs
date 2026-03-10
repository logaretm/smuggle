use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

fn store_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".smuggle").join("packages")
}

fn pkg_dir(name: &str) -> PathBuf {
    // Scoped packages: @scope/name → @scope/name (preserve as-is, fs handles it)
    store_dir().join(name)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StoreEntry {
    pub name: String,
    pub version: String,
    pub source_dir: PathBuf,
    pub dependencies: HashMap<String, String>,
}

pub fn save(
    name: &str,
    version: &str,
    source_dir: &PathBuf,
    tarball: &[u8],
    dependencies: &HashMap<String, String>,
) -> Result<(), String> {
    let dir = pkg_dir(name);
    fs::create_dir_all(&dir).map_err(|e| format!("failed to create store dir: {e}"))?;

    // Write tarball
    let tarball_path = dir.join("package.tgz");
    fs::write(&tarball_path, tarball).map_err(|e| format!("failed to write tarball: {e}"))?;

    // Write metadata
    let meta = StoreEntry {
        name: name.to_string(),
        version: version.to_string(),
        source_dir: source_dir.clone(),
        dependencies: dependencies.clone(),
    };
    let meta_path = dir.join("metadata.json");
    let meta_json =
        serde_json::to_string_pretty(&meta).map_err(|e| format!("failed to serialize: {e}"))?;
    fs::write(&meta_path, meta_json).map_err(|e| format!("failed to write metadata: {e}"))?;

    Ok(())
}

pub fn load_tarball(name: &str) -> Result<Vec<u8>, String> {
    let tarball_path = pkg_dir(name).join("package.tgz");
    fs::read(&tarball_path).map_err(|e| format!("failed to read tarball: {e}"))
}

pub fn list() -> Vec<StoreEntry> {
    let dir = store_dir();
    if !dir.exists() {
        return vec![];
    }

    let mut entries = Vec::new();
    collect_entries(&dir, &mut entries);
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}

fn collect_entries(dir: &PathBuf, entries: &mut Vec<StoreEntry>) {
    let Ok(read) = fs::read_dir(dir) else {
        return;
    };

    for entry in read.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let meta_path = path.join("metadata.json");
        if meta_path.exists() {
            if let Ok(content) = fs::read_to_string(&meta_path) {
                if let Ok(meta) = serde_json::from_str::<StoreEntry>(&content) {
                    entries.push(meta);
                }
            }
        } else {
            // Recurse into subdirectories (for @scope/ dirs)
            collect_entries(&path, entries);
        }
    }
}

pub fn remove(name: &str) -> Result<(), String> {
    let dir = pkg_dir(name);
    if !dir.exists() {
        return Err(format!("{name} is not registered"));
    }
    fs::remove_dir_all(&dir).map_err(|e| format!("failed to remove: {e}"))?;

    // Clean up empty scope directory if applicable
    if let Some(parent) = dir.parent() {
        if parent != store_dir() {
            let _ = fs::remove_dir(parent); // Only removes if empty
        }
    }

    Ok(())
}
