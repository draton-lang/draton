use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub(crate) fn run(project_root: &Path) -> Result<()> {
    let files = collect_dt_files(project_root)?;
    let mut changed = 0usize;
    for file in files {
        let source = fs::read_to_string(&file)
            .with_context(|| format!("khong the doc {}", file.display()))?;
        let formatted = format_source(&source);
        if formatted != source {
            fs::write(&file, formatted)
                .with_context(|| format!("khong the ghi {}", file.display()))?;
            changed += 1;
        }
    }
    println!("formatted {changed} file(s)");
    Ok(())
}

pub(crate) fn format_source(source: &str) -> String {
    let mut lines = source
        .lines()
        .map(|line| line.trim_end().to_string())
        .collect::<Vec<_>>();
    lines = normalize_type_blocks(&lines);
    let mut out = Vec::new();
    let mut indent = 0usize;
    let mut previous_top_level = false;
    for raw in lines {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            if !out
                .last()
                .map(|line: &String| line.is_empty())
                .unwrap_or(false)
            {
                out.push(String::new());
            }
            continue;
        }
        if trimmed.starts_with('}') {
            indent = indent.saturating_sub(1);
        }
        let is_top_level = indent == 0 && starts_top_level(trimmed);
        if is_top_level
            && previous_top_level
            && !out.last().map(|line| line.is_empty()).unwrap_or(false)
        {
            out.push(String::new());
        }
        let rendered =
            if trimmed.starts_with('@') && trimmed.contains('{') && trimmed.ends_with('}') {
                format!("{}{}", " ".repeat(indent * 4), trimmed)
            } else {
                wrap_line(trimmed, indent)
            };
        out.push(rendered);
        let open = trimmed.matches('{').count();
        let close = trimmed.matches('}').count();
        if open > close {
            indent += open - close;
        } else if close > open && !trimmed.starts_with('}') {
            indent = indent.saturating_sub(close - open);
        }
        previous_top_level = is_top_level;
    }
    let mut rendered = out.join("\n");
    if !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    rendered
}

fn wrap_line(trimmed: &str, indent: usize) -> String {
    if trimmed.len() <= 100 {
        return format!("{}{}", " ".repeat(indent * 4), trimmed);
    }
    let mut rendered = String::new();
    let mut current = String::new();
    let prefix = " ".repeat(indent * 4);
    for word in trimmed.split_whitespace() {
        let candidate = if current.is_empty() {
            word.to_string()
        } else {
            format!("{current} {word}")
        };
        if candidate.len() > 100 - prefix.len() && !current.is_empty() {
            if !rendered.is_empty() {
                rendered.push('\n');
            }
            rendered.push_str(&prefix);
            rendered.push_str(&current);
            current = word.to_string();
        } else {
            current = candidate;
        }
    }
    if !current.is_empty() {
        if !rendered.is_empty() {
            rendered.push('\n');
        }
        rendered.push_str(&prefix);
        rendered.push_str(&current);
    }
    rendered
}

fn normalize_type_blocks(lines: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.starts_with("@type {") && trimmed.ends_with('}') && !trimmed.contains('\n') {
            let inner = trimmed
                .trim_start_matches("@type {")
                .trim_end_matches('}')
                .trim();
            out.push("@type {".to_string());
            for item in inner.split("  ").filter(|item| !item.trim().is_empty()) {
                out.push(format!("    {}", item.trim()));
            }
            out.push("}".to_string());
        } else {
            out.push(line.clone());
        }
    }
    out
}

fn starts_top_level(line: &str) -> bool {
    line.starts_with("fn ")
        || line.starts_with("pub fn ")
        || line.starts_with("class ")
        || line.starts_with("interface ")
        || line.starts_with("enum ")
        || line.starts_with("error ")
        || line.starts_with("const ")
        || line.starts_with("import ")
        || line.starts_with("@type")
        || line.starts_with("@extern")
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
