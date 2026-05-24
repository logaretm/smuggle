use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Copy a directory tree, preserving structure.
/// Skips nested node_modules (pnpm dep symlinks).
pub fn copy_dir(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| format!("mkdir {}: {e}", dst.display()))?;
    for entry in std::fs::read_dir(src).map_err(|e| format!("read {}: {e}", src.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            if entry.file_name() == "node_modules" {
                continue;
            }
            copy_dir(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)
                .map_err(|e| format!("copy {}: {e}", src_path.display()))?;
        }
    }
    Ok(())
}

/// Restore a directory from backup, replacing current contents.
/// Preserves node_modules subdirs managed by the package manager.
pub fn restore_dir(backup: &Path, target: &Path) -> Result<(), String> {
    if let Ok(entries) = std::fs::read_dir(target) {
        for entry in entries.flatten() {
            if entry.file_name() == "node_modules" {
                continue;
            }
            let path = entry.path();
            if path.is_dir() {
                let _ = std::fs::remove_dir_all(&path);
            } else {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    if let Ok(entries) = std::fs::read_dir(backup) {
        for entry in entries.flatten() {
            let src = entry.path();
            let dst = target.join(entry.file_name());
            if src.is_dir() {
                copy_dir(&src, &dst)?;
            } else {
                std::fs::copy(&src, &dst).map_err(|e| format!("restore {}: {e}", dst.display()))?;
            }
        }
    }
    Ok(())
}

/// Back up a list of override targets to a temporary directory.
/// Returns the backup base path. Each call gets a unique directory
/// so that watch-mode re-extractions don't collide with prior backups.
pub fn backup_targets(targets: &[(String, PathBuf)]) -> Result<PathBuf, String> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let backup_base =
        std::env::temp_dir()
            .join("smuggle-backup")
            .join(format!("{}-{}", std::process::id(), id));
    std::fs::create_dir_all(&backup_base)
        .map_err(|e| format!("failed to create backup dir: {e}"))?;

    for (name, target_dir) in targets {
        let backup_name = name.replace('/', "__");
        let backup_path = backup_base.join(&backup_name);
        copy_dir(target_dir, &backup_path)
            .map_err(|e| format!("failed to backup {}: {e}", name))?;
    }

    Ok(backup_base)
}

/// Restore all targets from their backups and clean up the backup directory.
pub fn restore_all(backup_base: &Path, targets: &[(String, PathBuf)]) {
    for (name, target_dir) in targets {
        let backup_name = name.replace('/', "__");
        let backup_path = backup_base.join(&backup_name);
        let _ = restore_dir(&backup_path, target_dir);
    }
    let _ = std::fs::remove_dir_all(backup_base);
}

/// Snapshot a file's current contents (or record its absence) so it can be
/// restored later. Used for lockfiles that a package manager is about to
/// rewrite.
#[derive(Clone)]
pub struct FileSnapshot {
    pub path: PathBuf,
    pub original: Option<Vec<u8>>,
}

impl FileSnapshot {
    pub fn capture(path: PathBuf) -> Self {
        let original = std::fs::read(&path).ok();
        Self { path, original }
    }

    pub fn restore(&self) {
        match &self.original {
            Some(bytes) => {
                let _ = std::fs::write(&self.path, bytes);
            }
            None => {
                let _ = std::fs::remove_file(&self.path);
            }
        }
    }
}

pub struct AddCleanupInfo {
    pub added_dirs: Vec<PathBuf>,
    pub pkg_json_path: PathBuf,
    pub original_pkg_json: String,
    pub lockfile_snapshots: Vec<FileSnapshot>,
}

/// Set up a ctrl-c handler that restores node_modules backups and optionally
/// reverts injected package.json/lockfile changes for new packages.
pub fn setup_ctrlc_combined_cleanup(
    backup_base: PathBuf,
    targets: Vec<(PathBuf, PathBuf)>,
    add_cleanup: Option<AddCleanupInfo>,
) {
    let _ = ctrlc::set_handler(move || {
        for (backup, target) in &targets {
            let _ = restore_dir(backup, target);
        }
        let _ = std::fs::remove_dir_all(&backup_base);

        if let Some(ref cleanup) = add_cleanup {
            for dir in &cleanup.added_dirs {
                let _ = std::fs::remove_dir_all(dir);
                if let Some(parent) = dir.parent() {
                    let _ = std::fs::remove_dir(parent);
                }
            }
            let _ = std::fs::write(&cleanup.pkg_json_path, &cleanup.original_pkg_json);
            for snap in &cleanup.lockfile_snapshots {
                snap.restore();
            }
            eprintln!("\n  Restored originals and reverted package.json/lockfile");
        } else {
            eprintln!("\n  Restored originals");
        }
        std::process::exit(0);
    });
}
