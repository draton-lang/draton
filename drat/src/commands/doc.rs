use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub(crate) fn run(project_root: &Path) -> Result<()> {
    let src_dir = project_root.join("src");
    let files = collect_dt_files(&src_dir)?;
    let out_dir = project_root.join("build/doc");
    fs::create_dir_all(&out_dir).with_context(|| format!("khong the tao {}", out_dir.display()))?;

    let mut index_entries = Vec::new();
    for file in files {
        let module_name = file
            .file_stem()
            .and_then(|item| item.to_str())
            .unwrap_or("module")
            .to_string();
        let source = fs::read_to_string(&file)
            .with_context(|| format!("khong the doc {}", file.display()))?;
        let docs = extract_docs(&source);
        let page = render_module_page(&module_name, &docs);
        let out_file = out_dir.join(format!("{module_name}.html"));
        fs::write(&out_file, page)
            .with_context(|| format!("khong the ghi {}", out_file.display()))?;
        index_entries.push((
            module_name,
            out_file.file_name().unwrap().to_string_lossy().into_owned(),
        ));
    }

    let index = render_index(&index_entries);
    fs::write(out_dir.join("index.html"), index)
        .with_context(|| format!("khong the ghi {}", out_dir.join("index.html").display()))?;
    println!("docs -> {}", out_dir.join("index.html").display());
    Ok(())
}

fn extract_docs(source: &str) -> BTreeMap<String, String> {
    let mut docs = BTreeMap::new();
    let mut pending = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(doc) = trimmed.strip_prefix("///") {
            pending.push(doc.trim().to_string());
            continue;
        }
        if trimmed.starts_with("pub fn ")
            || trimmed.starts_with("fn ")
            || trimmed.starts_with("class ")
            || trimmed.starts_with("enum ")
            || trimmed.starts_with("error ")
        {
            let normalized = trimmed.strip_prefix("pub ").unwrap_or(trimmed);
            let name = normalized
                .split_whitespace()
                .nth(1)
                .unwrap_or(normalized)
                .split('(')
                .next()
                .unwrap_or(normalized)
                .to_string();
            let doc = pending.join(" ");
            docs.insert(name, doc);
            pending.clear();
        } else if !trimmed.is_empty() {
            pending.clear();
        }
    }
    docs
}

fn render_module_page(module: &str, docs: &BTreeMap<String, String>) -> String {
    let mut body = String::new();
    for (name, doc) in docs {
        body.push_str(&format!(
            "<section><h2>{}</h2><p>{}</p></section>",
            escape_html(name),
            escape_html(doc)
        ));
    }
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{}</title></head>\
         <body><h1>Module {}</h1>{}</body></html>",
        escape_html(module),
        escape_html(module),
        body
    )
}

fn render_index(entries: &[(String, String)]) -> String {
    let mut items = String::new();
    for (name, file) in entries {
        items.push_str(&format!(
            "<li><a href=\"{}\">{}</a></li>",
            escape_html(file),
            escape_html(name)
        ));
    }
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Draton Docs</title></head>\
         <body><h1>Draton Docs</h1><ul>{}</ul></body></html>",
        items
    )
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn collect_dt_files(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("khong the doc {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("dt") {
            out.push(path);
        }
    }
    out.sort();
    Ok(out)
}
