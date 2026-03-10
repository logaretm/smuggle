use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PackageManager {
    Pnpm,
    Yarn,
    Npm,
}

impl PackageManager {
    pub fn name(&self) -> &str {
        match self {
            Self::Pnpm => "pnpm",
            Self::Yarn => "yarn",
            Self::Npm => "npm",
        }
    }

    pub fn cache_delete_args(&self, pkg: &str) -> Vec<String> {
        match self {
            Self::Pnpm => vec!["cache".into(), "delete".into(), pkg.into()],
            Self::Yarn => vec!["cache".into(), "clean".into(), pkg.into()],
            Self::Npm => vec![
                "cache".into(),
                "clean".into(),
                pkg.into(),
                "--force".into(),
            ],
        }
    }

    pub fn install_args(&self) -> Vec<&str> {
        match self {
            Self::Pnpm => vec!["install"],
            Self::Yarn => vec!["install"],
            Self::Npm => vec!["install"],
        }
    }
}

pub fn detect_package_manager(dir: &Path) -> PackageManager {
    if dir.join("pnpm-lock.yaml").exists() || dir.join("pnpm-workspace.yaml").exists() {
        PackageManager::Pnpm
    } else if dir.join("yarn.lock").exists() {
        PackageManager::Yarn
    } else {
        PackageManager::Npm
    }
}

const MAX_CONCURRENT: usize = 8;

pub fn clear_cache(pm: PackageManager, packages: &[String], cwd: &Path) {
    let mut children = Vec::with_capacity(MAX_CONCURRENT);

    for pkg in packages {
        let args = pm.cache_delete_args(pkg);
        eprintln!("  {} {}", pm.name(), args.join(" "));

        match Command::new(pm.name())
            .args(&args)
            .current_dir(cwd)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => children.push((pkg.clone(), child)),
            Err(e) => eprintln!("  error spawning {}: {e}", pm.name()),
        }

        if children.len() >= MAX_CONCURRENT {
            for (name, mut child) in children.drain(..) {
                match child.wait() {
                    Ok(s) if s.success() => {}
                    Ok(s) => eprintln!("  warning: cache clear failed for {}: {}", name, s),
                    Err(e) => eprintln!("  error waiting for {}: {e}", name),
                }
            }
        }
    }

    for (name, mut child) in children {
        match child.wait() {
            Ok(s) if s.success() => {}
            Ok(s) => eprintln!("  warning: cache clear failed for {}: {}", name, s),
            Err(e) => eprintln!("  error waiting for {}: {e}", name),
        }
    }
}

pub fn run_install(pm: PackageManager, cwd: &Path) -> Result<(), String> {
    let status = Command::new(pm.name())
        .args(pm.install_args())
        .current_dir(cwd)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| format!("failed to run {} install: {e}", pm.name()))?;

    if !status.success() {
        return Err(format!("{} install failed with {}", pm.name(), status));
    }

    Ok(())
}

/// Force update specific packages to latest from the registry.
/// This tells the package manager to re-resolve these packages,
/// bypassing lockfile pins.
pub fn run_update(pm: PackageManager, packages: &[String], cwd: &Path) -> Result<(), String> {
    if packages.is_empty() {
        return Ok(());
    }

    let args: Vec<String> = match pm {
        // npm install <pkg>@latest <pkg2>@latest — forces re-fetch
        PackageManager::Npm => {
            let mut a = vec!["install".to_string()];
            for pkg in packages {
                a.push(format!("{pkg}@latest"));
            }
            a
        }
        // pnpm update <pkg> <pkg2>
        PackageManager::Pnpm => {
            let mut a = vec!["update".to_string()];
            a.extend(packages.iter().cloned());
            a
        }
        // yarn up <pkg> <pkg2> (berry) or yarn upgrade <pkg> (classic)
        PackageManager::Yarn => {
            let mut a = vec!["up".to_string()];
            a.extend(packages.iter().cloned());
            a
        }
    };

    let str_args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

    let status = Command::new(pm.name())
        .args(&str_args)
        .current_dir(cwd)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| format!("failed to run {} update: {e}", pm.name()))?;

    if !status.success() {
        return Err(format!("{} update failed with {}", pm.name(), status));
    }

    Ok(())
}
