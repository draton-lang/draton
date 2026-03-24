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
fn destructures_tuple_literal() {
    let result = parse_and_check(
        r#"
fn main() {
    let (x, y) = (1, 2)
    print(x)
    print(y)
}
"#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
}

#[test]
fn destructures_with_wildcard() {
    let result = parse_and_check(
        r#"
fn main() {
    let (_, y) = (1, 2)
    print(y)
}
"#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
}

#[test]
fn destructures_function_return_tuple() {
    let result = parse_and_check(
        r#"
fn getCoords() { (10, 20) }
fn main() {
    let (x, y) = getCoords()
    print(x)
    print(y)
}
"#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
}

#[test]
fn reports_destructure_arity_mismatch() {
    let result = parse_and_check(
        r#"
fn main() {
    let (x, y, z) = (1, 2)
}
"#,
    );
    assert!(result
        .errors
        .iter()
        .any(|error| matches!(error, TypeError::DestructureArity { .. })));
}

#[test]
fn reports_non_tuple_destructure_error() {
    let result = parse_and_check(
        r#"
fn main() {
    let (x, y) = 42
}
"#,
    );
    assert!(!result.errors.is_empty(), "expected type errors");
}
