//! The store: everything `smuggle publish` has registered.

use std::path::Path;

use crate::{pack, store};

pub struct StoreItem {
    pub entry: store::StoreEntry,
    pub bytes: u64,
    /// True when the directory the package was packed from is gone, which
    /// makes the entry unrepackable and almost certainly stale.
    pub source_missing: bool,
}

pub fn load() -> Vec<StoreItem> {
    store::list()
        .into_iter()
        .map(|entry| {
            let bytes = std::fs::metadata(store::tarball_path(&entry.name))
                .map(|m| m.len())
                .unwrap_or(0);
            let source_missing = !entry.source_dir.exists();
            StoreItem {
                entry,
                bytes,
                source_missing,
            }
        })
        .collect()
}

/// Re-pack a registered package from its source directory.
pub fn repack(source_dir: &Path) -> Result<String, String> {
    let pkg_json_path = source_dir.join("package.json");
    let raw = std::fs::read_to_string(&pkg_json_path)
        .map_err(|e| format!("could not read {}: {e}", pkg_json_path.display()))?;
    let pkg_json: pack::PublishPackageJson =
        serde_json::from_str(&raw).map_err(|e| format!("could not parse package.json: {e}"))?;

    let name = pkg_json.name.clone().ok_or("package.json has no name")?;
    let version = pkg_json.version.clone().unwrap_or_else(|| "0.0.0".into());

    let tarball = pack::pack(source_dir, &pkg_json)?;
    store::save(
        &name,
        &version,
        source_dir,
        &tarball,
        &pkg_json.dependencies(),
    )?;

    Ok(name)
}

pub fn human_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    match bytes {
        b if b >= MB => format!("{:.1} MB", b as f64 / MB as f64),
        b if b >= KB => format!("{} KB", b / KB),
        b => format!("{b} B"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_the_way_people_write_them() {
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(2048), "2 KB");
        assert_eq!(human_bytes(1024 * 1024 * 3 / 2), "1.5 MB");
    }
}
