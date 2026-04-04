use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use draton_lexer::Lexer;
use draton_parser::Parser;
use serde_json::Value;

#[test]
#[ignore]
fn parser_selfhost_parity() {
    let repo_root = repo_root();
    let files = collect_source_files(&repo_root);
    let mut checked_files = 0usize;
    let mut skipped_lex_files = 0usize;
    let mut skipped_parse_files = 0usize;

    for path in files {
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
        let lex_result = Lexer::new(&source).tokenize();
        if !lex_result.errors.is_empty() {
            skipped_lex_files += 1;
            continue;
        }

        let parse_result = Parser::new(lex_result.tokens).parse();
        if !parse_result.errors.is_empty() {
            skipped_parse_files += 1;
            continue;
        }

        let rust_program = serde_json::to_value(&parse_result.program).unwrap_or_else(|err| {
            panic!("failed to serialize rust AST for {}: {err}", path.display())
        });
        let rust_warnings = serde_json::to_value(&parse_result.warnings).unwrap_or_else(|err| {
            panic!(
                "failed to serialize rust warnings for {}: {err}",
                path.display()
            )
        });
        let (selfhost_program, selfhost_warnings) = run_selfhost_parser(&repo_root, &path);
        assert_json_match(&path, "program", &rust_program, &selfhost_program);
        assert_json_match(&path, "parse_warnings", &rust_warnings, &selfhost_warnings);
        checked_files += 1;
    }

    println!(
        "checked {checked_files} files, skipped {skipped_lex_files} lexer fixtures, skipped {skipped_parse_files} parser fixtures"
    );
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| {
            panic!(
                "failed to derive repo root from {}",
                env!("CARGO_MANIFEST_DIR")
            )
        })
}

fn collect_source_files(repo_root: &Path) -> Vec<PathBuf> {
    let roots = [
        repo_root.join("tests/programs/jit"),
        repo_root.join("tests/programs/gc"),
        repo_root.join("tests/programs/compile"),
        repo_root.join("examples"),
    ];
    let mut files = Vec::new();
    for root in roots {
        collect_dt_files(&root, &mut files);
    }
    files.sort();
    files
}

fn collect_dt_files(root: &Path, files: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(root)
        .unwrap_or_else(|err| panic!("failed to read directory {}: {err}", root.display()));
    for entry in entries {
        let entry = entry
            .unwrap_or_else(|err| panic!("failed to read entry under {}: {err}", root.display()));
        let path = entry.path();
        if path.is_dir() {
            collect_dt_files(&path, files);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("dt") {
            files.push(path);
        }
    }
}

fn run_selfhost_parser(repo_root: &Path, path: &Path) -> (Value, Value) {
    let absolute_path = path
        .canonicalize()
        .unwrap_or_else(|err| panic!("failed to canonicalize {}: {err}", path.display()));
    let output = Command::new("cargo")
        .current_dir(repo_root)
        .args([
            "run",
            "-p",
            "drat",
            "--",
            "selfhost-stage0",
            "parse",
            "--json",
            absolute_path
                .to_str()
                .unwrap_or_else(|| panic!("non-utf8 path: {}", absolute_path.display())),
        ])
        .output()
        .unwrap_or_else(|err| {
            panic!(
                "failed to run self-host parser for {}: {err}",
                path.display()
            )
        });

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!(
            "self-host parser failed for {}\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
            path.display(),
            output.status,
            stdout,
            stderr
        );
    }

    let json: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
        panic!(
            "self-host parser returned invalid JSON for {}\nerror: {}\nstdout:\n{}",
            path.display(),
            err,
            String::from_utf8_lossy(&output.stdout)
        )
    });

    extract_parse_payload(path, &absolute_path, &json)
}

fn extract_parse_payload(path: &Path, absolute_path: &Path, json: &Value) -> (Value, Value) {
    assert_eq!(
        json["schema"],
        Value::String("draton.selfhost.stage0/v1".to_string()),
        "parser parity contract break for {}: unexpected schema",
        path.display()
    );
    assert_eq!(
        json["stage"],
        Value::String("parse".to_string()),
        "parser parity contract break for {}: unexpected stage",
        path.display()
    );
    assert_eq!(
        json["input_path"].as_str(),
        Some(absolute_path.to_string_lossy().as_ref()),
        "parser parity contract break for {}: unexpected input_path",
        path.display()
    );
    assert_eq!(
        json["bridge"]["kind"],
        Value::String("host".to_string()),
        "parser parity contract break for {}: unexpected bridge kind",
        path.display()
    );
    assert_eq!(
        json["bridge"]["builtin"],
        Value::String("host_parse_json".to_string()),
        "parser parity contract break for {}: unexpected bridge builtin",
        path.display()
    );
    assert_eq!(
        json["success"],
        Value::Bool(true),
        "parser parity contract break for {}: expected success=true",
        path.display()
    );

    let result = json.get("result").unwrap_or_else(|| {
        panic!(
            "parser parity contract break for {}: missing result payload\n{}",
            path.display(),
            serde_json::to_string_pretty(json).unwrap_or_else(|_| json.to_string())
        )
    });
    assert_eq!(
        result["lex_errors"],
        Value::Array(Vec::new()),
        "parser parity contract break for {}: expected empty lex_errors",
        path.display()
    );
    assert_eq!(
        result["parse_errors"],
        Value::Array(Vec::new()),
        "parser parity contract break for {}: expected empty parse_errors",
        path.display()
    );

    (
        result.get("program").cloned().unwrap_or(Value::Null),
        result
            .get("parse_warnings")
            .cloned()
            .unwrap_or_else(|| Value::Array(Vec::new())),
    )
}

fn assert_json_match(path: &Path, label: &str, rust_program: &Value, selfhost_program: &Value) {
    if rust_program == selfhost_program {
        return;
    }

    let diff = first_json_diff(label, rust_program, selfhost_program).unwrap_or_else(|| {
        JsonDiff::new(
            label.to_string(),
            Some(rust_program.clone()),
            Some(selfhost_program.clone()),
        )
    });
    panic!(
        "parser parity mismatch for {}\nlabel: {}\nfirst differing path: {}\nrust: {}\nselfhost: {}",
        path.display(),
        label,
        diff.path,
        display_json_side(diff.left.as_ref()),
        display_json_side(diff.right.as_ref())
    );
}

struct JsonDiff {
    path: String,
    left: Option<Value>,
    right: Option<Value>,
}

impl JsonDiff {
    fn new(path: String, left: Option<Value>, right: Option<Value>) -> Self {
        Self { path, left, right }
    }
}

fn first_json_diff(path: &str, left: &Value, right: &Value) -> Option<JsonDiff> {
    if left == right {
        return None;
    }

    match (left, right) {
        (Value::Object(left_map), Value::Object(right_map)) => {
            let keys = left_map
                .keys()
                .chain(right_map.keys())
                .cloned()
                .collect::<BTreeSet<_>>();
            for key in keys {
                let next_path = format!("{path}.{key}");
                match (left_map.get(&key), right_map.get(&key)) {
                    (Some(left_value), Some(right_value)) => {
                        if let Some(diff) = first_json_diff(&next_path, left_value, right_value) {
                            return Some(diff);
                        }
                    }
                    (left_value, right_value) => {
                        return Some(JsonDiff::new(
                            next_path,
                            left_value.cloned(),
                            right_value.cloned(),
                        ));
                    }
                }
            }
            Some(JsonDiff::new(
                path.to_string(),
                Some(left.clone()),
                Some(right.clone()),
            ))
        }
        (Value::Array(left_items), Value::Array(right_items)) => {
            let max_len = left_items.len().max(right_items.len());
            for index in 0..max_len {
                let next_path = format!("{path}[{index}]");
                match (left_items.get(index), right_items.get(index)) {
                    (Some(left_value), Some(right_value)) => {
                        if let Some(diff) = first_json_diff(&next_path, left_value, right_value) {
                            return Some(diff);
                        }
                    }
                    (left_value, right_value) => {
                        return Some(JsonDiff::new(
                            next_path,
                            left_value.cloned(),
                            right_value.cloned(),
                        ));
                    }
                }
            }
            Some(JsonDiff::new(
                path.to_string(),
                Some(left.clone()),
                Some(right.clone()),
            ))
        }
        _ => Some(JsonDiff::new(
            path.to_string(),
            Some(left.clone()),
            Some(right.clone()),
        )),
    }
}

fn display_json_side(value: Option<&Value>) -> String {
    match value {
        Some(value) => serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string()),
        None => "<missing>".to_string(),
    }
}
