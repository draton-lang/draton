use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use draton_lexer::Lexer;
use serde_json::Value;

type ComparableToken = (String, String, usize, usize);

#[test]
#[ignore]
fn lexer_selfhost_parity() {
    let repo_root = repo_root();
    let files = collect_source_files(&repo_root);
    let mut checked_files = 0usize;
    let mut skipped_files = 0usize;

    for path in files {
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
        let rust_result = Lexer::new(&source).tokenize();
        if !rust_result.errors.is_empty() {
            skipped_files += 1;
            continue;
        }

        let rust_tokens = rust_result
            .tokens
            .into_iter()
            .map(|token| {
                (
                    format!("{:?}", token.kind),
                    token.lexeme,
                    token.span.line,
                    token.span.col,
                )
            })
            .collect::<Vec<_>>();
        let selfhost_tokens = run_selfhost_lexer(&repo_root, &path);
        assert_token_streams_match(&path, &rust_tokens, &selfhost_tokens);
        checked_files += 1;
    }

    println!("checked {checked_files} files, skipped {skipped_files} expected-error fixtures");
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

fn run_selfhost_lexer(repo_root: &Path, path: &Path) -> Vec<ComparableToken> {
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
            "lex",
            "--json",
            absolute_path
                .to_str()
                .unwrap_or_else(|| panic!("non-utf8 path: {}", absolute_path.display())),
        ])
        .output()
        .unwrap_or_else(|err| {
            panic!(
                "failed to run self-host lexer for {}: {err}",
                path.display()
            )
        });

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!(
            "self-host lexer failed for {}\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
            path.display(),
            output.status,
            stdout,
            stderr
        );
    }

    let json: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
        panic!(
            "self-host lexer returned invalid JSON for {}\nerror: {}\nstdout:\n{}",
            path.display(),
            err,
            String::from_utf8_lossy(&output.stdout)
        )
    });
    let result = expect_lex_envelope(path, &absolute_path, &json);
    let array = result
        .get("tokens")
        .and_then(Value::as_array)
        .unwrap_or_else(|| {
            panic!(
                "self-host lexer returned JSON without result.tokens array for {}\nstdout:\n{}",
                path.display(),
                String::from_utf8_lossy(&output.stdout)
            )
        });

    array
        .iter()
        .enumerate()
        .map(|(index, token)| comparable_token_from_json(path, index, token))
        .collect()
}

fn expect_lex_envelope<'a>(
    display_path: &Path,
    absolute_path: &Path,
    json: &'a Value,
) -> &'a Value {
    assert_eq!(
        json["schema"],
        Value::String("draton.selfhost.stage0/v1".to_string()),
        "lexer parity contract break for {}: unexpected schema",
        display_path.display()
    );
    assert_eq!(
        json["stage"],
        Value::String("lex".to_string()),
        "lexer parity contract break for {}: unexpected stage",
        display_path.display()
    );
    assert_eq!(
        json["input_path"].as_str(),
        Some(absolute_path.to_string_lossy().as_ref()),
        "lexer parity contract break for {}: unexpected input_path",
        display_path.display()
    );
    assert_eq!(
        json["bridge"]["kind"],
        Value::String("selfhost".to_string()),
        "lexer parity contract break for {}: unexpected bridge kind",
        display_path.display()
    );
    assert_eq!(
        json["bridge"]["builtin"],
        Value::Null,
        "lexer parity contract break for {}: expected null bridge builtin",
        display_path.display()
    );
    assert_eq!(
        json["success"],
        Value::Bool(true),
        "lexer parity contract break for {}: expected success=true",
        display_path.display()
    );
    let result = json.get("result").unwrap_or_else(|| {
        panic!(
            "lexer parity contract break for {}: missing result payload\n{}",
            display_path.display(),
            serde_json::to_string_pretty(json).unwrap_or_else(|_| json.to_string())
        )
    });
    assert_eq!(
        result["errors"],
        Value::Array(Vec::new()),
        "lexer parity contract break for {}: expected empty result.errors",
        display_path.display()
    );
    result
}

fn comparable_token_from_json(path: &Path, index: usize, token: &Value) -> ComparableToken {
    let kind = token
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing string kind for {} token #{index}", path.display()))
        .to_string();
    let lexeme = token
        .get("lexeme")
        .and_then(Value::as_str)
        .unwrap_or_else(|| {
            panic!(
                "missing string lexeme for {} token #{index}",
                path.display()
            )
        })
        .to_string();
    let line = token
        .get("span")
        .and_then(|value| value.get("line"))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing numeric line for {} token #{index}", path.display()))
        as usize;
    let col = token
        .get("span")
        .and_then(|value| value.get("col"))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing numeric col for {} token #{index}", path.display()))
        as usize;
    (kind, lexeme, line, col)
}

fn assert_token_streams_match(
    path: &Path,
    rust_tokens: &[ComparableToken],
    selfhost_tokens: &[ComparableToken],
) {
    if rust_tokens == selfhost_tokens {
        return;
    }

    let first_diff = rust_tokens
        .iter()
        .zip(selfhost_tokens.iter())
        .position(|(rust_token, selfhost_token)| rust_token != selfhost_token);

    match first_diff {
        Some(index) => panic!(
            "lexer parity mismatch for {}\nfirst differing token at index {}\nrust: {:?}\nselfhost: {:?}\nrust_len: {}\nselfhost_len: {}",
            path.display(),
            index,
            rust_tokens[index],
            selfhost_tokens[index],
            rust_tokens.len(),
            selfhost_tokens.len()
        ),
        None => panic!(
            "lexer parity length mismatch for {}\nrust_len: {}\nselfhost_len: {}\nnext_rust: {:?}\nnext_selfhost: {:?}",
            path.display(),
            rust_tokens.len(),
            selfhost_tokens.len(),
            rust_tokens.get(selfhost_tokens.len()),
            selfhost_tokens.get(rust_tokens.len())
        ),
    }
}
