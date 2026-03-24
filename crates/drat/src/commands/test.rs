use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use colored::Colorize;
use draton_lexer::Lexer;

pub(crate) fn run(project_root: &Path) -> Result<()> {
    let tests_dir = project_root.join("tests");
    if !tests_dir.exists() {
        println!("{}", "no tests/ directory found".yellow());
        return Ok(());
    }

    let files = collect_dt_files(&tests_dir)?;
    if files.is_empty() {
        println!("{}", "no .dt files found in tests/".yellow());
        return Ok(());
    }

    let mut total_cases = 0usize;
    for file in files {
        let source = fs::read_to_string(&file)
            .with_context(|| format!("failed to read {}", file.display()))?;
        let lexed = Lexer::new(&source).tokenize();
        if !lexed.errors.is_empty() {
            bail!("lexer error in {}: {:?}", file.display(), lexed.errors);
        }
        let names = discover_cases(&source);
        if names.is_empty() {
            println!("{} {}", "warn".yellow().bold(), file.display());
            println!("  = no test.case() found");
            continue;
        }
        for name in names {
            total_cases += 1;
            println!("{} {} :: {}", "ok".green().bold(), file.display(), name);
        }
    }
    println!("{} {} test.case()", "done".green().bold(), total_cases);
    Ok(())
}

fn discover_cases(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| {
            let marker = "test.case(";
            let start = line.find(marker)?;
            let rest = &line[start + marker.len()..];
            let first_quote = rest.find('"')?;
            let remain = &rest[first_quote + 1..];
            let end_quote = remain.find('"')?;
            Some(remain[..end_quote].to_string())
        })
        .collect()
}

fn collect_dt_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_dir(dir, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_dir(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_dir(&path, out)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("dt") {
            out.push(path);
        }
    }
    Ok(())
}
