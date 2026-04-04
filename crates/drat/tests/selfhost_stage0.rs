use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

fn temp_case_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir()
        .join("draton")
        .join("selfhost_stage0_tests")
        .join(format!("{name}_{}_{}", std::process::id(), unique));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn describe_output(output: &std::process::Output) -> String {
    format!(
        "status={:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn run_stage0(args: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_drat"))
        .args(args)
        .output()
        .expect("run drat selfhost-stage0");

    assert!(
        output.status.success(),
        "command failed\n{}",
        describe_output(&output)
    );

    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stage0 command returned invalid JSON: {error}\n{}",
            describe_output(&output)
        )
    })
}

fn expect_envelope<'a>(
    json: &'a Value,
    stage: &str,
    input_path: &Path,
    bridge_kind: &str,
    bridge_builtin: Option<&str>,
    success: bool,
) -> &'a Value {
    assert_eq!(
        json["schema"],
        Value::String("draton.selfhost.stage0/v1".to_string()),
        "unexpected schema"
    );
    assert_eq!(
        json["stage"],
        Value::String(stage.to_string()),
        "unexpected stage"
    );
    assert_eq!(
        json["input_path"].as_str(),
        Some(input_path.to_string_lossy().as_ref()),
        "unexpected input_path"
    );
    assert_eq!(
        json["bridge"]["kind"],
        Value::String(bridge_kind.to_string()),
        "unexpected bridge kind"
    );
    match bridge_builtin {
        Some(builtin) => assert_eq!(
            json["bridge"]["builtin"],
            Value::String(builtin.to_string()),
            "unexpected bridge builtin"
        ),
        None => assert_eq!(
            json["bridge"]["builtin"],
            Value::Null,
            "expected null builtin"
        ),
    }
    assert_eq!(
        json["success"],
        Value::Bool(success),
        "unexpected success flag"
    );
    assert_eq!(
        json["error"],
        Value::Null,
        "expected top-level error to be null"
    );
    let result = json.get("result").unwrap_or_else(|| {
        panic!(
            "missing result payload\n{}",
            serde_json::to_string_pretty(json).unwrap_or_else(|_| json.to_string())
        )
    });
    assert!(result.is_object(), "expected result object");
    result
}

#[test]
fn lex_json_returns_machine_readable_tokens() {
    let dir = temp_case_dir("lex");
    let src = dir.join("main.dt");
    fs::write(
        &src,
        r#"
fn main() {
    return 0
}
"#,
    )
    .expect("write source");

    let json = run_stage0(&[
        "selfhost-stage0",
        "lex",
        "--json",
        src.to_str().expect("utf8 path"),
    ]);
    let result = expect_envelope(&json, "lex", &src, "selfhost", None, true);
    assert!(result["tokens"].is_array(), "expected tokens array");
    assert!(result["errors"].is_array(), "expected errors array");
    assert_eq!(
        result["errors"],
        Value::Array(Vec::new()),
        "expected no lex errors"
    );
    assert_eq!(result["tokens"][0]["kind"], Value::String("Fn".to_string()));
    assert!(
        result["tokens"]
            .as_array()
            .expect("tokens array")
            .iter()
            .any(|token| token["kind"] == Value::String("Return".to_string())),
        "expected return token"
    );
}

#[test]
fn parse_json_returns_machine_readable_program() {
    let dir = temp_case_dir("parse");
    let src = dir.join("main.dt");
    fs::write(
        &src,
        r#"
fn main() {
    return 0
}
"#,
    )
    .expect("write source");

    let json = run_stage0(&[
        "selfhost-stage0",
        "parse",
        "--json",
        src.to_str().expect("utf8 path"),
    ]);
    let result = expect_envelope(&json, "parse", &src, "selfhost", None, true);
    assert!(result["lex_errors"].is_array(), "expected lex_errors array");
    assert!(
        result["parse_errors"].is_array(),
        "expected parse_errors array"
    );
    assert!(
        result["parse_warnings"].is_array(),
        "expected parse_warnings array"
    );
    assert!(
        result["program"]["items"].is_array(),
        "expected parsed program items"
    );
}

#[test]
fn typeck_json_returns_machine_readable_program() {
    let dir = temp_case_dir("typeck");
    let src = dir.join("main.dt");
    fs::write(
        &src,
        r#"
fn main() {
    return 0
}
"#,
    )
    .expect("write source");

    let json = run_stage0(&[
        "selfhost-stage0",
        "typeck",
        "--json",
        src.to_str().expect("utf8 path"),
    ]);
    let result = expect_envelope(&json, "typeck", &src, "host", Some("host_type_json"), true);
    assert!(result["lex_errors"].is_array(), "expected lex_errors array");
    assert!(
        result["parse_errors"].is_array(),
        "expected parse_errors array"
    );
    assert!(
        result["parse_warnings"].is_array(),
        "expected parse_warnings array"
    );
    assert!(
        result["type_errors"].is_array(),
        "expected type_errors array"
    );
    assert!(
        result["type_warnings"].is_array(),
        "expected type_warnings array"
    );
    assert!(
        result["typed_program"]["items"].is_array(),
        "expected typed program items"
    );
}

#[test]
fn build_json_produces_runnable_binary() {
    let dir = temp_case_dir("build");
    let src = dir.join("main.dt");
    let out = dir.join(if cfg!(windows) { "hello.exe" } else { "hello" });
    fs::write(
        &src,
        r#"
fn main() {
    println("hello, selfhost!")
    return 0
}
"#,
    )
    .expect("write source");

    let json = run_stage0(&[
        "selfhost-stage0",
        "build",
        "--json",
        "-o",
        out.to_str().expect("utf8 path"),
        src.to_str().expect("utf8 path"),
    ]);
    let result = expect_envelope(&json, "build", &src, "host", Some("host_build_json"), true);
    assert_eq!(result["error"], Value::Null, "expected null build error");
    assert_eq!(
        result["output"]["binary_path"].as_str(),
        Some(out.to_string_lossy().as_ref()),
        "expected binary path in output"
    );
    assert!(out.exists(), "expected output binary");

    let built = Command::new(&out).output().expect("run built binary");
    assert!(built.status.success(), "built binary failed");
    assert_eq!(
        String::from_utf8_lossy(&built.stdout),
        "hello, selfhost!\n",
        "unexpected built binary stdout"
    );
}

#[test]
fn build_json_reports_machine_readable_failure() {
    let dir = temp_case_dir("build_error");
    let src = dir.join("main.dt");
    let out = dir.join(if cfg!(windows) {
        "broken.exe"
    } else {
        "broken"
    });
    fs::write(
        &src,
        r#"
fn main() {
    let value = missing_symbol()
    return value
}
"#,
    )
    .expect("write source");

    let json = run_stage0(&[
        "selfhost-stage0",
        "build",
        "--json",
        "-o",
        out.to_str().expect("utf8 path"),
        src.to_str().expect("utf8 path"),
    ]);
    let result = expect_envelope(&json, "build", &src, "host", Some("host_build_json"), false);
    assert_eq!(
        result["output"],
        Value::Null,
        "expected null output on build failure"
    );
    assert_eq!(
        result["error"]["kind"],
        Value::String("build_failed".to_string()),
        "unexpected build error kind"
    );
    assert!(
        result["error"]["message"]
            .as_str()
            .map(|message| !message.trim().is_empty())
            .unwrap_or(false),
        "expected non-empty build error message"
    );
}
