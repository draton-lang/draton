use std::fs;
use std::path::PathBuf;
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

    let output = Command::new(env!("CARGO_BIN_EXE_drat"))
        .arg("selfhost-stage0")
        .arg("lex")
        .arg("--json")
        .arg(&src)
        .output()
        .expect("run drat selfhost-stage0 lex");

    assert!(output.status.success(), "command failed");
    let json: Value = serde_json::from_slice(&output.stdout).expect("parse json");
    assert!(json["tokens"].is_array(), "expected tokens array");
    assert!(json["errors"].is_array(), "expected errors array");
    assert_eq!(json["errors"], Value::Array(Vec::new()), "expected no lex errors");
    assert_eq!(json["tokens"][0]["kind"], Value::String("Fn".to_string()));
    assert_eq!(
        json["tokens"]
            .as_array()
            .expect("tokens array")
            .iter()
            .any(|token| token["kind"] == Value::String("Return".to_string())),
        true,
        "expected return token"
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

    let output = Command::new(env!("CARGO_BIN_EXE_drat"))
        .arg("selfhost-stage0")
        .arg("typeck")
        .arg("--json")
        .arg(&src)
        .output()
        .expect("run drat selfhost-stage0 typeck");

    assert!(output.status.success(), "command failed");
    let json: Value = serde_json::from_slice(&output.stdout).expect("parse json");
    assert!(json["lex_errors"].is_array(), "expected lex_errors array");
    assert!(json["parse_errors"].is_array(), "expected parse_errors array");
    assert!(
        json["typecheck_result"]["typed_program"]["items"].is_array(),
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

    let output = Command::new(env!("CARGO_BIN_EXE_drat"))
        .arg("selfhost-stage0")
        .arg("build")
        .arg("--json")
        .arg("-o")
        .arg(&out)
        .arg(&src)
        .output()
        .expect("run drat selfhost-stage0 build");

    assert!(output.status.success(), "command failed");
    let json: Value = serde_json::from_slice(&output.stdout).expect("parse json");
    assert_eq!(json["ok"], Value::Bool(true), "expected ok=true");
    assert_eq!(
        json["output"]["binary_path"].as_str(),
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
