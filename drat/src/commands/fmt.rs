use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::tooling::files::collect_draton_files;

pub(crate) fn run(cwd: &Path, paths: &[PathBuf], check: bool) -> Result<()> {
    let files = collect_draton_files(cwd, paths)?;
    let mut changed = 0usize;
    let mut skipped = 0usize;
    let mut needs_write = Vec::new();

    for file in &files {
        match format_file(file) {
            Ok(Some(formatted)) => needs_write.push((file.clone(), formatted)),
            Ok(None) => {}
            Err(error) => {
                eprintln!("warning: skipping {} — {error}", file.display());
                skipped += 1;
            }
        }
    }

    if check {
        changed = needs_write.len();
        println!("checked {} file(s)", files.len());
        if changed > 0 {
            for (file, _) in &needs_write {
                println!("would reformat {}", file.display());
            }
            bail!("{} file(s) need formatting", changed);
        }
    } else {
        for (file, formatted) in needs_write {
            fs::write(&file, formatted)
                .with_context(|| format!("khong the ghi {}", file.display()))?;
            changed += 1;
        }
        println!("formatted {changed} file(s)");
    }
    if skipped > 0 {
        println!("skipped {skipped} file(s) to preserve comments or parse safety");
    }
    Ok(())
}

pub(crate) fn format_file(path: &Path) -> Result<Option<String>> {
    let source =
        fs::read_to_string(path).with_context(|| format!("khong the doc {}", path.display()))?;
    if source.contains("//") || source.contains("/*") {
        bail!("comment-preserving formatting is not yet available for this file");
    }
    let formatted = crate::fmt::format_source(&source);
    if formatted == source {
        Ok(None)
    } else {
        Ok(Some(formatted))
    }
}
