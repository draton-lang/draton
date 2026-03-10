use draton_lexer::Lexer;
use draton_parser::Parser;
use draton_typeck::{TypeChecker, TypeError};

fn parse_and_check(source: &str) -> draton_typeck::TypeCheckResult {
    let lexed = Lexer::new(source).tokenize();
    assert!(lexed.errors.is_empty(), "lexer errors: {:?}", lexed.errors);
    let parsed = Parser::new(lexed.tokens).parse();
    assert!(
        parsed.errors.is_empty(),
        "parser errors: {:?}",
        parsed.errors
    );
    TypeChecker::new().check(parsed.program)
}

#[test]
fn reports_mismatch_undefined_argcount_badcast_and_infinite_type() {
    let result = parse_and_check(
        r#"
@type { fn add(a: Int, b: Int) -> Int }
fn add(a, b) { a + b }
fn bad() {
    add("hello", 42)
    print(x)
    add(1)
    1 as String
    let looped = lambda x => x(x)
}
"#,
    );
    assert!(result.errors.len() >= 5, "errors: {:?}", result.errors);
    assert!(result
        .errors
        .iter()
        .any(|error| matches!(error, TypeError::Mismatch { .. })));
    assert!(result
        .errors
        .iter()
        .any(|error| matches!(error, TypeError::UndefinedVar { name, .. } if name == "x")));
    assert!(result
        .errors
        .iter()
        .any(|error| matches!(error, TypeError::ArgCount { .. })));
    assert!(result
        .errors
        .iter()
        .any(|error| matches!(error, TypeError::BadCast { .. })));
    assert!(result
        .errors
        .iter()
        .any(|error| matches!(error, TypeError::InfiniteType { .. })));
}

#[test]
fn reports_incompatible_error_propagation_for_mixed_nullish_errors() {
    let result = parse_and_check(
        r#"
fn first() { Ok(1) }
fn second() { Ok(2) }
fn load() {
    let a = first() ?? "missing"
    let b = second() ?? 404
    b
}
"#,
    );
    assert!(result
        .errors
        .iter()
        .any(|error| matches!(error, TypeError::IncompatibleErrors { .. })));
}

#[test]
fn reports_cannot_infer_for_ambiguous_empty_array_literal() {
    let result = parse_and_check(
        r#"
fn main() {
    let x = []
}
"#,
    );
    assert!(result
        .errors
        .iter()
        .any(|error| matches!(error, TypeError::CannotInfer { name, .. } if name == "x")));
}

#[test]
fn reports_missing_interface_method_error() {
    let result = parse_and_check(
        r#"
interface Drawable {
    fn draw()
}
class Rect implements Drawable {
}
"#,
    );
    assert!(result.errors.iter().any(|error| matches!(
        error,
        TypeError::MissingInterfaceMethod {
            class,
            interface,
            method,
            ..
        } if class == "Rect" && interface == "Drawable" && method == "draw"
    )));
}

#[test]
fn reports_non_exhaustive_enum_match() {
    let result = parse_and_check(
        r#"
enum Direction { North, South, East, West }
fn main() {
    let dir = Direction.North
    match dir {
        Direction.North => print("up")
        Direction.South => print("down")
    }
}
"#,
    );
    assert!(result.errors.iter().any(
        |error| matches!(error, TypeError::NonExhaustiveMatch { missing, .. }
            if missing.contains("Direction.East") && missing.contains("Direction.West"))
    ));
}

#[test]
fn wildcard_makes_match_exhaustive() {
    let result = parse_and_check(
        r#"
enum Direction { North, South, East, West }
fn main() {
    let dir = Direction.North
    match dir {
        Direction.North => print("up")
        _ => print("other")
    }
}
"#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
}

#[test]
fn reports_bool_exhaustiveness_and_int_wildcard_requirement() {
    let bool_ok = parse_and_check(
        r#"
fn main() {
    let b = true
    match b {
        true => print("yes")
        false => print("no")
    }
}
"#,
    );
    assert!(bool_ok.errors.is_empty(), "errors: {:?}", bool_ok.errors);

    let bool_bad = parse_and_check(
        r#"
fn main() {
    let b = true
    match b {
        true => print("yes")
    }
}
"#,
    );
    assert!(bool_bad
        .errors
        .iter()
        .any(|error| matches!(error, TypeError::NonExhaustiveMatch { .. })));

    let int_bad = parse_and_check(
        r#"
fn main() {
    let x = 42
    match x {
        1 => print("one")
        2 => print("two")
    }
}
"#,
    );
    assert!(int_bad.errors.iter().any(|error| matches!(
        error,
        TypeError::NonExhaustiveMatch { missing, .. }
        if missing.contains("wildcard required")
    )));
}

#[test]
fn result_match_is_exhaustive_with_ok_and_err() {
    let result = parse_and_check(
        r#"
fn main() {
    let r = Ok(42)
    match r {
        Ok(v) => print("ok")
        Err(e) => print("err")
    }
}
"#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
}

#[test]
fn redundant_pattern_is_reported_as_warning() {
    let result = parse_and_check(
        r#"
enum Direction { North, South }
fn main() {
    let dir = Direction.North
    match dir {
        Direction.North => print("up")
        Direction.North => print("still up")
        _ => print("other")
    }
}
"#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    assert!(result
        .warnings
        .iter()
        .any(|warning| matches!(warning, TypeError::RedundantPattern { .. })));
}
