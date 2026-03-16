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
        .join("tooling_suite_tests")
        .join(format!("{name}_{}_{}", std::process::id(), unique));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

#[test]
fn fmt_check_and_write_work_on_canonical_file() {
    let dir = temp_case_dir("fmt");
    let src = dir.join("sample.dt");
    fs::write(
        &src,
        "import { foo } from sample\n\n@type { main: () -> Int }\nfn main(){return 1+2}\n",
    )
    .expect("write source");

    let check = Command::new(env!("CARGO_BIN_EXE_drat"))
        .arg("fmt")
        .arg("--check")
        .arg(&src)
        .output()
        .expect("run drat fmt --check");
    assert!(!check.status.success(), "expected formatter check to fail");
    let stderr = String::from_utf8_lossy(&check.stderr);
    assert!(
        stderr.contains("need formatting"),
        "expected formatting failure message, got:\n{stderr}"
    );

    let format = Command::new(env!("CARGO_BIN_EXE_drat"))
        .arg("fmt")
        .arg(&src)
        .status()
        .expect("run drat fmt");
    assert!(format.success(), "expected formatter write to succeed");

    let formatted = fs::read_to_string(&src).expect("read formatted source");
    assert!(formatted.contains("fn main() {"), "{formatted}");
    assert!(formatted.contains("return 1 + 2"), "{formatted}");
}

#[test]
fn lint_reports_canonical_findings() {
    let dir = temp_case_dir("lint");
    let src = dir.join("sample.dt");
    fs::write(
        &src,
        "import { unused } from sample\nfn main() {\n    let value: Int = 1\n    return value\n}\n",
    )
    .expect("write source");

    let output = Command::new(env!("CARGO_BIN_EXE_drat"))
        .arg("lint")
        .arg(&src)
        .output()
        .expect("run drat lint");
    assert!(output.status.success(), "lint should be advisory");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("deprecated-syntax"), "{stdout}");
    assert!(stdout.contains("unused-import"), "{stdout}");
}

#[test]
fn task_lists_and_runs_project_tasks() {
    let dir = temp_case_dir("task");
    fs::write(
        dir.join("drat.tasks"),
        "[tasks.hello]\ndescription = \"print from shell\"\nrun = \"echo tooling\"\n",
    )
    .expect("write task file");

    let list = Command::new(env!("CARGO_BIN_EXE_drat"))
        .current_dir(&dir)
        .arg("task")
        .output()
        .expect("run drat task");
    assert!(list.status.success(), "task listing should succeed");
    let stdout = String::from_utf8_lossy(&list.stdout);
    assert!(stdout.contains("hello"), "{stdout}");

    let run = Command::new(env!("CARGO_BIN_EXE_drat"))
        .current_dir(&dir)
        .arg("task")
        .arg("hello")
        .output()
        .expect("run drat task hello");
    assert!(run.status.success(), "task execution should succeed");
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(stdout.contains("tooling"), "{stdout}");
}
