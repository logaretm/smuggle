use console::style;
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
        eprintln!("  {} {} {}",
            style("|").dim(),
            style(pm.name()).dim(),
            style(args.join(" ")).dim(),
        );

        match Command::new(pm.name())
            .args(&args)
            .current_dir(cwd)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => children.push((pkg.clone(), child)),
            Err(e) => eprintln!("  {} spawning {}: {e}",
                style("error:").red().bold(),
                pm.name(),
            ),
        }

        if children.len() >= MAX_CONCURRENT {
            for (name, mut child) in children.drain(..) {
                match child.wait() {
                    Ok(s) if s.success() => {}
                    Ok(s) => eprintln!("  {} cache clear failed for {}: {}",
                        style("warn:").yellow(),
                        style(&name).cyan(),
                        s,
                    ),
                    Err(e) => eprintln!("  {} waiting for {}: {e}",
                        style("error:").red().bold(),
                        name,
                    ),
                }
            }
        }
    }

    for (name, mut child) in children {
        match child.wait() {
            Ok(s) if s.success() => {}
            Ok(s) => eprintln!("  {} cache clear failed for {}: {}",
                style("warn:").yellow(),
                style(&name).cyan(),
                s,
            ),
            Err(e) => eprintln!("  {} waiting for {}: {e}",
                style("error:").red().bold(),
                name,
            ),
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
