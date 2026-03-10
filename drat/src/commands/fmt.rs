use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub(crate) fn run(project_root: &Path) -> Result<()> {
    let files = collect_dt_files(project_root)?;
    let mut changed = 0usize;
    let mut skipped = 0usize;

    for file in &files {
        match format_file(file) {
            Ok(Some(formatted)) => {
                fs::write(file, formatted)
                    .with_context(|| format!("khong the ghi {}", file.display()))?;
                changed += 1;
            }
            Ok(None) => {}
            Err(error) => {
                eprintln!("warning: skipping {} — {error}", file.display());
                skipped += 1;
            }
        }
    }

    println!("formatted {changed} file(s)");
    if skipped > 0 {
        println!("skipped {skipped} file(s) due to parse errors");
    }
    Ok(())
}

pub(crate) fn format_file(path: &Path) -> Result<Option<String>> {
    let source =
        fs::read_to_string(path).with_context(|| format!("khong the doc {}", path.display()))?;
    let formatted = crate::fmt::format_source(&source);
    if formatted == source {
        Ok(None)
    } else {
        Ok(Some(formatted))
    }
}

fn collect_dt_files(project_root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    walk(project_root, &mut files)?;
    files.sort();
    Ok(files)
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("khong the doc {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let name = path
                .file_name()
                .and_then(|item| item.to_str())
                .unwrap_or_default();
            if matches!(name, ".git" | "target" | "build" | ".drat") {
                continue;
            }
            walk(&path, out)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("dt") {
            out.push(path);
        }
    }
    Ok(())
}
