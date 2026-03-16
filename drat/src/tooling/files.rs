use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

const IGNORED_DIRS: &[&str] = &[".git", "target", "build", ".drat"];

pub(crate) fn collect_draton_files(cwd: &Path, inputs: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let roots = if inputs.is_empty() {
        vec![cwd.to_path_buf()]
    } else {
        inputs
            .iter()
            .map(|path| resolve_path(cwd, path))
            .collect::<Vec<_>>()
    };
    let mut out = BTreeSet::new();
    for root in roots {
        if !root.exists() {
            bail!("path does not exist: {}", root.display());
        }
        collect_path(&root, &mut out)?;
    }
    Ok(out.into_iter().collect())
}

pub(crate) fn find_upwards(start: &Path, name: &str) -> Option<PathBuf> {
    let mut current = Some(start);
    while let Some(dir) = current {
        let candidate = dir.join(name);
        if candidate.exists() {
            return Some(candidate);
        }
        current = dir.parent();
    }
    None
}

fn resolve_path(cwd: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

fn collect_path(path: &Path, out: &mut BTreeSet<PathBuf>) -> Result<()> {
    if path.is_file() {
        if is_draton_file(path) {
            out.insert(path.to_path_buf());
        }
        return Ok(());
    }

    for entry in fs::read_dir(path).with_context(|| format!("cannot read {}", path.display()))? {
        let entry = entry?;
        let child = entry.path();
        if child.is_dir() {
            let name = child
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default();
            if IGNORED_DIRS.contains(&name) {
                continue;
            }
            collect_path(&child, out)?;
        } else if is_draton_file(&child) {
            out.insert(child);
        }
    }
    Ok(())
}

fn is_draton_file(path: &Path) -> bool {
    path.extension().and_then(|value| value.to_str()) == Some("dt")
}
