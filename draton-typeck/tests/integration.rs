use draton_lexer::Lexer;
use draton_parser::Parser;
use draton_typeck::{Type, TypeChecker, TypedItem, TypedStmtKind};

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
fn propagates_result_error_type_through_nullish() {
    let result = parse_and_check(
        r#"
@type { fn getUser() -> Result[String, String] }
fn getUser() { Ok("alice") }
fn load() {
    let user = getUser() ?? "missing"
    user
}
"#,
    );
    assert!(result.errors.is_empty(), "type errors: {:?}", result.errors);
    let TypedItem::Fn(load_fn) = &result.typed_program.items[1] else {
        panic!("expected load fn");
    };
    assert_eq!(
        load_fn.ret_type,
        Type::Result(Box::new(Type::String), Box::new(Type::String))
    );
}

#[test]
fn uses_type_annotations_and_checks_inside_unsafe() {
    let result = parse_and_check(
        r#"
@type { fn add(a: Int, b: Int) -> Int }
fn add(a, b) { a + b }
fn main() {
    @unsafe {
        let x: Int = add(1, 2)
    }
}
"#,
    );
    assert!(result.errors.is_empty(), "type errors: {:?}", result.errors);
    let TypedItem::Fn(add_fn) = &result.typed_program.items[1] else {
        panic!("expected add");
    };
    assert_eq!(add_fn.ret_type, Type::Int);
    assert_eq!(add_fn.params[0].ty, Type::Int);
    assert_eq!(add_fn.params[1].ty, Type::Int);
}

#[test]
fn checks_class_fields_and_methods() {
    let result = parse_and_check(
        r#"
class User {
    let name: String
    fn getName() { self.name }
}
"#,
    );
    assert!(result.errors.is_empty(), "type errors: {:?}", result.errors);
    let TypedItem::Class(class_def) = &result.typed_program.items[0] else {
        panic!("expected class");
    };
    assert_eq!(class_def.fields[0].ty, Type::String);
    assert_eq!(class_def.methods[0].ret_type, Type::String);
    let body = class_def.methods[0].body.as_ref().expect("body");
    match &body.stmts[0].kind {
        TypedStmtKind::Expr(expr) => assert_eq!(expr.ty, Type::String),
        other => panic!("unexpected stmt: {other:?}"),
    }
}
