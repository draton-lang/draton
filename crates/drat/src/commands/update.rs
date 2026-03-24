use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::config::DratonConfig;

pub(crate) fn run(project_root: &Path, subject: Option<&str>) -> Result<()> {
    match subject {
        None => update_self(),
        Some("packages") => update_packages(project_root, None),
        Some(pkg) => update_packages(project_root, Some(pkg)),
    }
}

fn update_self() -> Result<()> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let output = Command::new("cargo")
        .arg("install")
        .arg("--path")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")))
        .arg("--force")
        .output()
        .with_context(|| format!("failed to run cargo install for {}", manifest.display()))?;
    if !output.status.success() {
        bail!(
            "drat update failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

fn update_packages(project_root: &Path, only: Option<&str>) -> Result<()> {
    let mut config = DratonConfig::load(project_root)?;
    rewrite_map(&mut config.dependencies, only);
    rewrite_map(&mut config.dev_dependencies, only);
    config.save(project_root)
}

fn rewrite_map(map: &mut std::collections::BTreeMap<String, String>, only: Option<&str>) {
    for (name, spec) in map.iter_mut() {
        if only.map(|target| target != name).unwrap_or(false) {
            continue;
        }
        if spec.starts_with("stdlib@") {
            continue;
        }
        if let Some((prefix, _)) = spec.rsplit_once('@') {
            *spec = format!("{prefix}@latest");
        }
    }
}
