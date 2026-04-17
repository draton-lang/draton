use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use draton_ast::Item;
use draton_lexer::Lexer;
use draton_parser::Parser;
use serde_json::Value;

struct ParseFixture {
    name: &'static str,
    source: &'static str,
}

#[test]
#[ignore = "blocked until hidden stage0 parse stops bridging through host_parse_json and compiler/parser selfhost tree typechecks under stage0 again"]
fn parser_selfhost_parity_on_representative_fixtures() {
    let repo_root = repo_root();
    let fixtures = [
        ParseFixture {
            name: "multi_item_success",
            source: r#"
import {
    fs as f
} from std.io
@type {
    greet: () -> String
}
class Dog extends Animal implements Drawable {
    let name

    layer Voice {
        fn speak() { return "Woof!" }
    }

    @type {
        name: String
        speak: () -> String
    }
}
interface Drawable {
    fn draw()
}
enum Color { Red, Green, Blue }
error NotFound(msg)
const MAX = 100
fn greet() { return "hi" }
"#,
        },
        ParseFixture {
            name: "imports_and_extern",
            source: r#"
import {
    fs as f
    net as n
} from std.io
@extern "C" {
    fn malloc(size: UInt64) -> @pointer
    fn free(ptr: @pointer)
}
"#,
        },
        ParseFixture {
            name: "deprecated_warning",
            source: r#"
fn add(a: Int) -> Int {
    let value: Int = a
    return value
}
"#,
        },
        ParseFixture {
            name: "parse_error_recovery",
            source: r#"
layer Validation {
    fn validate() { }
}

fn main() {
    return 0
}
"#,
        },
        ParseFixture {
            name: "lex_error",
            source: r#"
fn main() {
    return $
}
"#,
        },
    ];

    for fixture in fixtures {
        assert_fixture_parity(&repo_root, &fixture);
    }
}

fn assert_fixture_parity(repo_root: &Path, fixture: &ParseFixture) {
    let lexed = Lexer::new(fixture.source).tokenize();
    let rust_lex_errors = serde_json::to_value(&lexed.errors).unwrap_or_else(|err| {
        panic!(
            "failed to serialize rust lex errors for {}: {err}",
            fixture.name
        )
    });
    let parsed = if lexed.errors.is_empty() {
        Some(Parser::new(lexed.tokens).parse())
    } else {
        None
    };

    let expected_success = parsed
        .as_ref()
        .map(|result| result.errors.is_empty())
        .unwrap_or(false)
        && lexed.errors.is_empty();

    let expected_parse_errors = parsed
        .as_ref()
        .map(|result| serde_json::to_value(&result.errors).expect("serialize rust parse errors"))
        .unwrap_or_else(|| Value::Array(Vec::new()));
    let expected_parse_warnings = parsed
        .as_ref()
        .map(|result| {
            serde_json::to_value(&result.warnings).expect("serialize rust parse warnings")
        })
        .unwrap_or_else(|| Value::Array(Vec::new()));
    let expected_item_kinds = parsed
        .as_ref()
        .map(|result| rust_item_kinds(&result.program.items))
        .unwrap_or_else(|| Value::Null);

    let dir = temp_case_dir(fixture.name);
    let src = dir.join("main.dt");
    fs::write(&src, fixture.source).unwrap_or_else(|err| {
        panic!(
            "failed to write fixture {} to {}: {err}",
            fixture.name,
            src.display()
        )
    });
    let json = run_selfhost_parser(repo_root, &src);
    let result = extract_parse_payload(fixture.name, &src, &json, expected_success);

    assert_eq!(
        result["lex_errors"],
        rust_lex_errors,
        "lex error drift for fixture {}\n{}",
        fixture.name,
        serde_json::to_string_pretty(result).unwrap_or_else(|_| result.to_string())
    );
    assert_eq!(
        result["parse_errors"],
        expected_parse_errors,
        "parse error drift for fixture {}\n{}",
        fixture.name,
        serde_json::to_string_pretty(result).unwrap_or_else(|_| result.to_string())
    );
    assert_eq!(
        result["parse_warnings"],
        expected_parse_warnings,
        "parse warning drift for fixture {}\n{}",
        fixture.name,
        serde_json::to_string_pretty(result).unwrap_or_else(|_| result.to_string())
    );
    assert_eq!(
        selfhost_item_kinds(&result["program"]),
        expected_item_kinds,
        "top-level item kind drift for fixture {}\n{}",
        fixture.name,
        serde_json::to_string_pretty(result).unwrap_or_else(|_| result.to_string())
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

fn temp_case_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir()
        .join("draton")
        .join("parser_selfhost_parity")
        .join(format!("{name}_{}_{}", std::process::id(), unique));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn run_selfhost_parser(repo_root: &Path, path: &Path) -> Value {
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

    serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
        panic!(
            "self-host parser returned invalid JSON for {}\nerror: {}\nstdout:\n{}",
            path.display(),
            err,
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

fn extract_parse_payload<'a>(
    fixture_name: &str,
    input_path: &Path,
    json: &'a Value,
    expected_success: bool,
) -> &'a Value {
    let expected_input_path =
        normalize_windows_verbatim_path(input_path.to_string_lossy().as_ref());
    let actual_input_path = json["input_path"]
        .as_str()
        .map(normalize_windows_verbatim_path);
    assert_eq!(
        json["schema"],
        Value::String("draton.selfhost.stage0/v1".to_string()),
        "parser parity contract break for fixture {}: unexpected schema",
        fixture_name
    );
    assert_eq!(
        json["stage"],
        Value::String("parse".to_string()),
        "parser parity contract break for fixture {}: unexpected stage",
        fixture_name
    );
    assert_eq!(
        actual_input_path.as_deref(),
        Some(expected_input_path.as_str()),
        "parser parity contract break for fixture {}: unexpected input_path",
        fixture_name
    );
    assert_eq!(
        json["bridge"]["kind"],
        Value::String("selfhost".to_string()),
        "parser parity contract break for fixture {}: unexpected bridge kind",
        fixture_name
    );
    assert_eq!(
        json["bridge"]["builtin"],
        Value::String("host_parse_json".to_string()),
        "parser parity contract break for fixture {}: unexpected bridge builtin",
        fixture_name
    );
    assert_eq!(
        json["success"],
        Value::Bool(expected_success),
        "parser parity contract break for fixture {}: unexpected success flag",
        fixture_name
    );

    json.get("result").unwrap_or_else(|| {
        panic!(
            "parser parity contract break for fixture {}: missing result payload\n{}",
            fixture_name,
            serde_json::to_string_pretty(json).unwrap_or_else(|_| json.to_string())
        )
    })
}

fn normalize_windows_verbatim_path(path: &str) -> String {
    if cfg!(windows) {
        path.strip_prefix(r"\\?\").unwrap_or(path).to_string()
    } else {
        path.to_string()
    }
}

fn rust_item_kinds(items: &[Item]) -> Value {
    Value::Array(
        items
            .iter()
            .map(|item| {
                Value::String(
                    match item {
                        Item::Fn(_) => "Fn",
                        Item::Class(_) => "Class",
                        Item::Interface(_) => "Interface",
                        Item::Enum(_) => "Enum",
                        Item::Error(_) => "Error",
                        Item::Const(_) => "Const",
                        Item::Import(_) => "Import",
                        Item::Extern(_) => "Extern",
                        Item::TypeBlock(_) => "TypeBlock",
                        Item::PanicHandler(_) => "PanicHandler",
                        Item::OomHandler(_) => "OomHandler",
                    }
                    .to_string(),
                )
            })
            .collect(),
    )
}

fn selfhost_item_kinds(program: &Value) -> Value {
    match program.get("items").and_then(Value::as_array) {
        Some(items) => Value::Array(
            items
                .iter()
                .map(|item| match item {
                    Value::Object(object) if object.len() == 1 => object
                        .keys()
                        .next()
                        .map(|key| Value::String(key.clone()))
                        .unwrap_or(Value::Null),
                    _ => Value::Null,
                })
                .collect(),
        ),
        None => Value::Null,
    }
}
