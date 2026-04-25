use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use draton_lexer::Lexer;
use draton_parser::Parser;
use draton_typeck::typed_ast::{TypedExpr, TypedExprKind, TypedFnDef, TypedItem, TypedStmtKind};
use draton_typeck::{TypeCheckResult, TypeChecker, TypeError, UseEffect};
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
    static STAGE0_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let _guard = STAGE0_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("lock selfhost stage0 test execution");
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

fn compile_with_rust_typechecker(source: &str) -> TypeCheckResult {
    let lexed = Lexer::new(source).tokenize();
    assert!(lexed.errors.is_empty(), "lexer errors: {:?}", lexed.errors);
    let parsed = Parser::new(lexed.tokens).parse();
    assert!(
        parsed.errors.is_empty(),
        "parser errors: {:?}",
        parsed.errors
    );
    assert!(
        parsed.warnings.is_empty(),
        "parser warnings: {:?}",
        parsed.warnings
    );
    TypeChecker::new().check(parsed.program)
}

fn rust_function<'a>(result: &'a TypeCheckResult, name: &str) -> &'a TypedFnDef {
    result
        .typed_program
        .items
        .iter()
        .find_map(|item| match item {
            TypedItem::Fn(function) if function.name == name => Some(function),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing rust function '{name}'"))
}

fn stage0_function<'a>(typed_program: &'a Value, name: &str) -> &'a Value {
    typed_program["items"]
        .as_array()
        .expect("typed program items")
        .iter()
        .find(|item| item["Fn"]["name"] == name)
        .unwrap_or_else(|| {
            panic!(
                "missing stage0 function '{name}' in\n{}",
                serde_json::to_string_pretty(typed_program)
                    .unwrap_or_else(|_| typed_program.to_string())
            )
        })
}

fn use_effect_name(effect: &UseEffect) -> &'static str {
    match effect {
        UseEffect::Copy => "Copy",
        UseEffect::BorrowShared => "BorrowShared",
        UseEffect::BorrowExclusive => "BorrowExclusive",
        UseEffect::Move => "Move",
    }
}

fn rust_first_ownership_error_kind(result: &TypeCheckResult) -> Option<&'static str> {
    result.errors.iter().find_map(|error| match error {
        TypeError::Ownership(ownership) => Some(match ownership {
            draton_typeck::OwnershipError::UseAfterMove { .. } => "UseAfterMove",
            draton_typeck::OwnershipError::MoveWhileBorrowed { .. } => "MoveWhileBorrowed",
            draton_typeck::OwnershipError::ReadDuringExclusiveBorrow { .. } => {
                "ReadDuringExclusiveBorrow"
            }
            draton_typeck::OwnershipError::ExclusiveBorrowDuringRead { .. } => {
                "ExclusiveBorrowDuringRead"
            }
            draton_typeck::OwnershipError::PartialMove { .. } => "PartialMove",
            draton_typeck::OwnershipError::AmbiguousCallOwnership { .. } => {
                "AmbiguousCallOwnership"
            }
            draton_typeck::OwnershipError::BorrowedValueEscapes { .. } => "BorrowedValueEscapes",
            draton_typeck::OwnershipError::MultipleOwners { .. } => "MultipleOwners",
            draton_typeck::OwnershipError::OwnershipCycle { .. } => "OwnershipCycle",
            draton_typeck::OwnershipError::LoopMoveWithoutReinit { .. } => "LoopMoveWithoutReinit",
            draton_typeck::OwnershipError::ExternalBoundaryRejection { .. } => {
                "ExternalBoundaryRejection"
            }
            draton_typeck::OwnershipError::SafeToRawAliasRejection { .. } => {
                "SafeToRawAliasRejection"
            }
        }),
        _ => None,
    })
}

fn stage0_first_error_kind(result: &Value) -> Option<&str> {
    let errors = result["type_errors"].as_array()?;
    let error = errors.first()?.as_object()?;
    if let Some(ownership) = error.get("Ownership").and_then(Value::as_object) {
        return ownership.keys().next().map(|kind| kind.as_str());
    }
    error.keys().next().map(|kind| kind.as_str())
}

fn rust_let_value<'a>(function: &'a TypedFnDef, index: usize) -> &'a TypedExpr {
    match &function
        .body
        .as_ref()
        .expect("function body")
        .stmts
        .get(index)
        .expect("stmt")
        .kind
    {
        TypedStmtKind::Let(stmt) => stmt.value.as_ref().expect("let value"),
        other => panic!("expected let stmt, found {other:?}"),
    }
}

fn rust_return_value<'a>(function: &'a TypedFnDef, index: usize) -> &'a TypedExpr {
    match &function
        .body
        .as_ref()
        .expect("function body")
        .stmts
        .get(index)
        .expect("stmt")
        .kind
    {
        TypedStmtKind::Return(stmt) => stmt.value.as_ref().expect("return value"),
        other => panic!("expected return stmt, found {other:?}"),
    }
}

fn rust_expr_stmt<'a>(function: &'a TypedFnDef, index: usize) -> &'a TypedExpr {
    match &function
        .body
        .as_ref()
        .expect("function body")
        .stmts
        .get(index)
        .expect("stmt")
        .kind
    {
        TypedStmtKind::Expr(expr) => expr,
        other => panic!("expected expr stmt, found {other:?}"),
    }
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
    let result = expect_envelope(
        &json,
        "parse",
        &src,
        "selfhost",
        None,
        true,
    );
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
    if json["success"] != Value::Bool(true) {
        panic!(
            "unexpected stage0 payload\n{}",
            serde_json::to_string_pretty(&json).unwrap_or_else(|_| json.to_string())
        );
    }
    let result = expect_envelope(
        &json,
        "typeck",
        &src,
        "selfhost",
        None,
        true,
    );
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
fn typeck_json_respects_strict_syntax_deprecated_diagnostics() {
    let dir = temp_case_dir("typeck_strict");
    let src = dir.join("main.dt");
    fs::write(
        &src,
        r#"
fn add(a: Int) -> Int {
    let value: Int = a
    return value
}
"#,
    )
    .expect("write source");

    let warn_json = run_stage0(&[
        "selfhost-stage0",
        "typeck",
        "--json",
        src.to_str().expect("utf8 path"),
    ]);
    let warn_result = expect_envelope(
        &warn_json,
        "typeck",
        &src,
        "selfhost",
        None,
        true,
    );
    assert_eq!(
        warn_result["type_errors"],
        Value::Array(Vec::new()),
        "expected no strict-mode type errors"
    );
    assert!(
        warn_result["type_warnings"]
            .as_array()
            .expect("type warnings array")
            .iter()
            .any(|warning| warning.get("DeprecatedSyntax").is_some()),
        "expected DeprecatedSyntax warning in warn mode"
    );

    let strict_json = run_stage0(&[
        "selfhost-stage0",
        "typeck",
        "--json",
        "--strict-syntax",
        src.to_str().expect("utf8 path"),
    ]);
    let strict_result = expect_envelope(
        &strict_json,
        "typeck",
        &src,
        "selfhost",
        None,
        false,
    );
    assert!(
        strict_result["type_errors"]
            .as_array()
            .expect("type errors array")
            .iter()
            .any(|error| error.get("DeprecatedSyntax").is_some()),
        "expected DeprecatedSyntax error in strict mode"
    );
}

#[test]
fn typeck_json_emits_recursive_borrow_ownership_summary() {
    let dir = temp_case_dir("typeck_ownership_borrow");
    let src = dir.join("main.dt");
    let source = r#"@type {
    walk: (String, Int) -> Unit
}

fn walk(text, n) {
    walk(text, n)
}
    "#;
    fs::write(&src, source).expect("write source");

    let rust_checked = compile_with_rust_typechecker(source);
    assert!(
        rust_checked.errors.is_empty(),
        "rust typechecker errors: {:?}",
        rust_checked.errors
    );

    let json = run_stage0(&[
        "selfhost-stage0",
        "typeck",
        "--json",
        src.to_str().expect("utf8 path"),
    ]);
    let result = expect_envelope(
        &json,
        "typeck",
        &src,
        "selfhost",
        None,
        rust_checked.errors.is_empty(),
    );
    assert_eq!(
        result["type_errors"],
        Value::Array(Vec::new()),
        "unexpected stage0 type errors\n{}",
        serde_json::to_string_pretty(&json).unwrap_or_else(|_| json.to_string())
    );
    let stage0_function = stage0_function(&result["typed_program"], "walk");
    let stage0_summary = &stage0_function["Fn"]["ownership_summary"];
    assert!(stage0_summary.is_object(), "expected ownership summary");

    let rust_function = rust_function(&rust_checked, "walk");
    let rust_summary = rust_function
        .ownership_summary
        .as_ref()
        .expect("rust ownership summary");

    assert_eq!(
        stage0_summary["returns_owned"].as_bool(),
        Some(rust_summary.returns_owned),
        "returns_owned drift"
    );
    assert_eq!(
        stage0_summary["params"][0]["effect"].as_str(),
        Some(use_effect_name(&rust_summary.params[0].effect)),
        "parameter effect drift"
    );
}

#[test]
fn typeck_json_emits_recursive_move_ownership_summary() {
    let dir = temp_case_dir("typeck_ownership_move");
    let src = dir.join("main.dt");
    let source = r#"@type {
    pass_down: (String, Int) -> String
}

fn pass_down(text, n) {
    if n == 0 {
        return text
    }
    return pass_down(text, n)
}
    "#;
    fs::write(&src, source).expect("write source");

    let rust_checked = compile_with_rust_typechecker(source);
    let rust_error_kind = rust_first_ownership_error_kind(&rust_checked);

    let json = run_stage0(&[
        "selfhost-stage0",
        "typeck",
        "--json",
        src.to_str().expect("utf8 path"),
    ]);
    let result = expect_envelope(
        &json,
        "typeck",
        &src,
        "selfhost",
        None,
        rust_checked.errors.is_empty(),
    );
    assert_eq!(
        result["type_errors"]
            .as_array()
            .expect("stage0 type errors array")
            .len(),
        rust_checked.errors.len(),
        "stage0/rust error count drift"
    );
    assert_eq!(
        stage0_first_error_kind(result),
        rust_error_kind,
        "stage0/rust first ownership error drift"
    );
    let stage0_function = stage0_function(&result["typed_program"], "pass_down");
    let stage0_summary = &stage0_function["Fn"]["ownership_summary"];
    assert!(stage0_summary.is_object(), "expected ownership summary");

    let rust_function = rust_function(&rust_checked, "pass_down");
    let rust_summary = rust_function
        .ownership_summary
        .as_ref()
        .expect("rust ownership summary");

    assert_eq!(
        stage0_summary["returns_owned"].as_bool(),
        Some(rust_summary.returns_owned),
        "returns_owned drift"
    );
    assert_eq!(
        stage0_summary["params"][0]["effect"].as_str(),
        Some(use_effect_name(&rust_summary.params[0].effect)),
        "parameter effect drift"
    );
}

#[test]
fn typeck_json_emits_use_effect_metadata() {
    let dir = temp_case_dir("typeck_use_effects");
    let src = dir.join("main.dt");
    let source = r#"fn forward(text) {
    return text
}

fn main() {
    let name = input("name: ")
    let out = forward(name)
    print(out.len())
}
"#;
    fs::write(&src, source).expect("write source");

    let json = run_stage0(&[
        "selfhost-stage0",
        "typeck",
        "--json",
        src.to_str().expect("utf8 path"),
    ]);
    let result = expect_envelope(
        &json,
        "typeck",
        &src,
        "selfhost",
        None,
        true,
    );
    assert_eq!(
        result["lex_errors"],
        Value::Array(Vec::new()),
        "unexpected lex errors\n{}",
        serde_json::to_string_pretty(&json).unwrap_or_else(|_| json.to_string())
    );
    assert_eq!(
        result["parse_errors"],
        Value::Array(Vec::new()),
        "unexpected parse errors\n{}",
        serde_json::to_string_pretty(&json).unwrap_or_else(|_| json.to_string())
    );
    assert_eq!(
        result["type_errors"],
        Value::Array(Vec::new()),
        "unexpected selfhost type errors\n{}",
        serde_json::to_string_pretty(&json).unwrap_or_else(|_| json.to_string())
    );

    let rust_checked = compile_with_rust_typechecker(source);
    assert!(
        rust_checked.errors.is_empty(),
        "rust typechecker errors: {:?}",
        rust_checked.errors
    );

    let rust_forward = rust_function(&rust_checked, "forward");
    let rust_forward_return = rust_return_value(rust_forward, 0);
    let stage0_forward = stage0_function(&result["typed_program"], "forward");
    let selfhost_forward_return =
        &stage0_forward["Fn"]["body"]["stmts"][0]["kind"]["Return"]["value"];
    assert_eq!(
        selfhost_forward_return["use_effect"].as_str(),
        rust_forward_return.use_effect.as_ref().map(use_effect_name),
        "forward return use_effect drift"
    );

    let rust_main = rust_function(&rust_checked, "main");
    let rust_name_init = rust_let_value(rust_main, 0);
    let stage0_main = stage0_function(&result["typed_program"], "main");
    let selfhost_name_init = &stage0_main["Fn"]["body"]["stmts"][0]["kind"]["Let"]["value"];
    assert_eq!(
        selfhost_name_init["use_effect"].as_str(),
        rust_name_init.use_effect.as_ref().map(use_effect_name),
        "input initializer use_effect drift"
    );

    let rust_out_init = rust_let_value(rust_main, 1);
    let rust_move_arg = match &rust_out_init.kind {
        TypedExprKind::Call(_, args) => args.first().expect("forward arg"),
        other => panic!("expected call expr, found {other:?}"),
    };
    let selfhost_move_arg =
        &stage0_main["Fn"]["body"]["stmts"][1]["kind"]["Let"]["value"]["kind"]["Call"][1][0];
    assert_eq!(
        selfhost_move_arg["use_effect"].as_str(),
        rust_move_arg.use_effect.as_ref().map(use_effect_name),
        "forward argument use_effect drift"
    );

    let rust_print_expr = rust_expr_stmt(rust_main, 2);
    let rust_method_target = match &rust_print_expr.kind {
        TypedExprKind::Call(_, args) => match &args.first().expect("print arg").kind {
            TypedExprKind::MethodCall(target, _, _) => target.as_ref(),
            other => panic!("expected method call arg, found {other:?}"),
        },
        other => panic!("expected print call, found {other:?}"),
    };
    let selfhost_method_target = &stage0_main["Fn"]["body"]["stmts"][2]["kind"]["Expr"]["kind"]
        ["Call"][1][0]["kind"]["MethodCall"][0];
    assert_eq!(
        selfhost_method_target["use_effect"].as_str(),
        rust_method_target.use_effect.as_ref().map(use_effect_name),
        "method receiver use_effect drift"
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
    let expected_stdout = if cfg!(windows) {
        "hello, selfhost!\r\n"
    } else {
        "hello, selfhost!\n"
    };
    assert_eq!(
        String::from_utf8_lossy(&built.stdout),
        expected_stdout,
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
