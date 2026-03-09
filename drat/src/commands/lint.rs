use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use colored::Colorize;

pub(crate) fn run(project_root: &Path) -> Result<()> {
    let files = collect_dt_files(project_root)?;
    let mut total = 0usize;
    for file in files {
        let source = fs::read_to_string(&file)
            .with_context(|| format!("khong the doc {}", file.display()))?;
        let warnings = lint_source(&source);
        for warning in warnings {
            total += 1;
            println!(
                "{} {}:{}:{} {}",
                "warning".yellow().bold(),
                file.display(),
                warning.line,
                warning.col,
                warning.message
            );
        }
    }
    if total == 0 {
        println!("{}", "khong co canh bao lint".green());
    } else {
        println!("{} {} canh bao", "done".yellow().bold(), total);
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct LintWarning {
    line: usize,
    col: usize,
    message: String,
}

fn lint_source(source: &str) -> Vec<LintWarning> {
    let lines = source.lines().collect::<Vec<_>>();
    let words = words_with_counts(source);
    let mut warnings = Vec::new();
    let mut seen_vars = BTreeSet::new();
    let mut seen_imports = BTreeSet::new();
    let mut after_return = false;
    let mut function_start = None::<usize>;
    let mut brace_depth = 0usize;
    let mut previous_non_empty = "";

    for (index, line) in lines.iter().enumerate() {
        let line_no = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if after_return && !trimmed.starts_with('}') {
            warnings.push(LintWarning {
                line: line_no,
                col: 1,
                message: "unreachable code sau return".to_string(),
            });
            after_return = false;
        }
        if let Some(name) = parse_let_name(trimmed) {
            if !seen_vars.insert(name.clone()) {
                warnings.push(LintWarning {
                    line: line_no,
                    col: line.find(&name).map(|col| col + 1).unwrap_or(1),
                    message: format!("bien '{name}' bi shadow"),
                });
            }
            if words.get(&name).copied().unwrap_or(0) <= 1 {
                warnings.push(LintWarning {
                    line: line_no,
                    col: line.find(&name).map(|col| col + 1).unwrap_or(1),
                    message: format!("bien '{name}' khong duoc su dung"),
                });
            }
        }
        if trimmed.starts_with("import {") {
            for name in parse_import_names(trimmed) {
                if !seen_imports.insert(name.clone()) {
                    continue;
                }
                if words.get(&name).copied().unwrap_or(0) <= 1 {
                    warnings.push(LintWarning {
                        line: line_no,
                        col: line.find(&name).map(|col| col + 1).unwrap_or(1),
                        message: format!("import '{name}' khong duoc su dung"),
                    });
                }
            }
        }
        if trimmed.starts_with("@unsafe")
            && !trimmed.contains("//")
            && !previous_non_empty.trim_start().starts_with("//")
        {
            warnings.push(LintWarning {
                line: line_no,
                col: 1,
                message: "@unsafe block nen co comment giai thich".to_string(),
            });
        }
        if trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ") {
            function_start = Some(line_no);
        }
        if trimmed.starts_with("return") {
            after_return = true;
        }
        let open = trimmed.matches('{').count();
        let close = trimmed.matches('}').count();
        brace_depth += open;
        if close > 0 {
            brace_depth = brace_depth.saturating_sub(close);
            if brace_depth == 0 {
                if let Some(start) = function_start.take() {
                    let len = line_no.saturating_sub(start) + 1;
                    if len > 50 {
                        warnings.push(LintWarning {
                            line: start,
                            col: 1,
                            message: format!("ham dai {len} dong, nen tach nho"),
                        });
                    }
                }
            }
        }
        previous_non_empty = line;
    }
    warnings
}

fn words_with_counts(source: &str) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    let mut current = String::new();
    for ch in source.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            current.push(ch);
        } else if !current.is_empty() {
            *counts.entry(current.clone()).or_insert(0) += 1;
            current.clear();
        }
    }
    if !current.is_empty() {
        *counts.entry(current).or_insert(0) += 1;
    }
    counts
}

fn parse_let_name(line: &str) -> Option<String> {
    let line = line.strip_prefix("let ")?;
    let line = line.strip_prefix("mut ").unwrap_or(line);
    let mut name = String::new();
    for ch in line.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            name.push(ch);
        } else {
            break;
        }
    }
    (!name.is_empty()).then_some(name)
}

fn parse_import_names(line: &str) -> Vec<String> {
    let inner = line
        .trim_start_matches("import")
        .trim()
        .trim_start_matches('{')
        .trim_end_matches('}')
        .trim();
    inner
        .split(',')
        .filter_map(|chunk| {
            let chunk = chunk.trim();
            if chunk.is_empty() {
                return None;
            }
            chunk
                .split(" as ")
                .nth(1)
                .or_else(|| chunk.split_whitespace().next())
                .map(|name| name.trim().to_string())
        })
        .collect()
}

fn collect_dt_files(project_root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    walk(project_root, &mut out)?;
    out.sort();
    Ok(out)
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
