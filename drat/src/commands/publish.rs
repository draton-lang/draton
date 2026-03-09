use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::config::DratonConfig;

pub(crate) fn run(project_root: &Path) -> Result<()> {
    let config = DratonConfig::load(project_root)?;
    let tag = format!("v{}", config.project.version);
    ensure_git(project_root)?;
    maybe_commit(project_root, &config.project.name, &config.project.version)?;
    run_git(
        project_root,
        &["tag", "-a", &tag, "-m", &format!("release {tag}")],
    )?;
    run_git(project_root, &["push", "origin", "HEAD", "--tags"])?;
    Ok(())
}

fn ensure_git(project_root: &Path) -> Result<()> {
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("--is-inside-work-tree")
        .current_dir(project_root)
        .output()
        .context("khong the kiem tra git repo")?;
    if !output.status.success() {
        bail!("thu muc hien tai khong nam trong git repository");
    }
    Ok(())
}

fn maybe_commit(project_root: &Path, name: &str, version: &str) -> Result<()> {
    let status = Command::new("git")
        .arg("status")
        .arg("--porcelain")
        .current_dir(project_root)
        .output()
        .context("khong the doc git status")?;
    if status.stdout.is_empty() {
        return Ok(());
    }
    run_git(project_root, &["add", "."])?;
    run_git(
        project_root,
        &[
            "commit",
            "-m",
            &format!("release: publish {name} {version}"),
        ],
    )
}

fn run_git(project_root: &Path, args: &[&str]) -> Result<()> {
    let output = Command::new("git")
        .args(args)
        .current_dir(project_root)
        .output()
        .with_context(|| format!("khong the chay git {}", args.join(" ")))?;
    if !output.status.success() {
        bail!(
            "git {} that bai:\n{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}
