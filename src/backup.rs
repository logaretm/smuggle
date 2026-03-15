use std::path::{Path, PathBuf};

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
/// Returns the backup base path.
pub fn backup_targets(targets: &[(String, PathBuf)]) -> Result<PathBuf, String> {
    let backup_base = std::env::temp_dir()
        .join("smuggle-backup")
        .join(std::process::id().to_string());
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

/// Set up a ctrl-c handler that restores all backups on interrupt.
pub fn setup_ctrlc_restore(backup_base: PathBuf, targets: Vec<(PathBuf, PathBuf)>) {
    let _ = ctrlc::set_handler(move || {
        for (backup, target) in &targets {
            let _ = restore_dir(backup, target);
        }
        let _ = std::fs::remove_dir_all(&backup_base);
        eprintln!("\n  Restored originals");
        std::process::exit(0);
    });
}
