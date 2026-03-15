use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_case_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir()
        .join("draton")
        .join("strict_syntax_tests")
        .join(format!("{name}_{}_{}", std::process::id(), unique));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn legacy_source() -> &'static str {
    r#"
fn main() -> Int {
    let value: Int = 1
    return value
}
"#
}

#[test]
fn compatibility_mode_still_builds_legacy_inline_types() {
    let dir = temp_case_dir("compat");
    let src = dir.join("main.dt");
    let out = dir.join("compat_out");
    fs::write(&src, legacy_source()).expect("write source");

    let status = Command::new(env!("CARGO_BIN_EXE_drat"))
        .arg("build")
        .arg(&src)
        .arg("--output")
        .arg(&out)
        .env("DRATON_DISABLE_GCROOT", "1")
        .env("DRATON_ALLOW_MULTIPLE_RUNTIME_DEFS", "1")
        .status()
        .expect("run drat build");

    assert!(status.success(), "expected compatibility build to succeed");
    assert!(out.exists(), "expected output binary at {}", out.display());
}

#[test]
fn strict_mode_rejects_legacy_inline_types() {
    let dir = temp_case_dir("strict");
    let src = dir.join("main.dt");
    let out = dir.join("strict_out");
    fs::write(&src, legacy_source()).expect("write source");

    let output = Command::new(env!("CARGO_BIN_EXE_drat"))
        .arg("build")
        .arg("--strict-syntax")
        .arg(&src)
        .arg("--output")
        .arg(&out)
        .env("DRATON_DISABLE_GCROOT", "1")
        .env("DRATON_ALLOW_MULTIPLE_RUNTIME_DEFS", "1")
        .output()
        .expect("run drat build --strict-syntax");

    assert!(
        !output.status.success(),
        "expected strict syntax build to fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("deprecated syntax"),
        "expected deprecated syntax diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("@type"),
        "expected @type migration hint, got:\n{stderr}"
    );
}
