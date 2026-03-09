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
