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
fn instantiates_polymorphic_identity_for_multiple_types() {
    let result = parse_and_check(
        r#"
fn id(x) { x }
fn main() {
    let a = id(1)
    let b = id("hello")
}
"#,
    );
    assert!(result.errors.is_empty(), "type errors: {:?}", result.errors);
    let TypedItem::Fn(main_fn) = &result.typed_program.items[1] else {
        panic!("expected main");
    };
    let body = main_fn.body.as_ref().expect("body");
    match &body.stmts[0].kind {
        TypedStmtKind::Let(let_stmt) => assert_eq!(let_stmt.ty, Type::Int),
        other => panic!("unexpected stmt: {other:?}"),
    }
    match &body.stmts[1].kind {
        TypedStmtKind::Let(let_stmt) => assert_eq!(let_stmt.ty, Type::String),
        other => panic!("unexpected stmt: {other:?}"),
    }
}

#[test]
fn infers_some_none_and_result_shapes() {
    let result = parse_and_check(
        r#"
fn main() {
    let some = Some(1)
    let none = None
    let ok = Ok("x")
    let err = Err("bad")
}
"#,
    );
    assert!(result.errors.is_empty(), "type errors: {:?}", result.errors);
    let TypedItem::Fn(main_fn) = &result.typed_program.items[0] else {
        panic!("expected main");
    };
    let body = main_fn.body.as_ref().expect("body");
    match &body.stmts[0].kind {
        TypedStmtKind::Let(let_stmt) => {
            assert_eq!(let_stmt.ty, Type::Option(Box::new(Type::Int)));
        }
        other => panic!("unexpected stmt: {other:?}"),
    }
    match &body.stmts[1].kind {
        TypedStmtKind::Let(let_stmt) => {
            assert!(matches!(let_stmt.ty, Type::Option(_)));
        }
        other => panic!("unexpected stmt: {other:?}"),
    }
    match &body.stmts[2].kind {
        TypedStmtKind::Let(let_stmt) => {
            assert!(matches!(let_stmt.ty, Type::Result(_, _)));
        }
        other => panic!("unexpected stmt: {other:?}"),
    }
    match &body.stmts[3].kind {
        TypedStmtKind::Let(let_stmt) => {
            assert!(matches!(let_stmt.ty, Type::Result(_, _)));
        }
        other => panic!("unexpected stmt: {other:?}"),
    }
}

#[test]
fn infers_open_row_type_from_field_access_without_annotations() {
    let result = parse_and_check(
        r#"
fn getName(x) { x.name }
"#,
    );
    assert!(result.errors.is_empty(), "type errors: {:?}", result.errors);
    let TypedItem::Fn(get_name) = &result.typed_program.items[0] else {
        panic!("expected function");
    };
    match &get_name.params[0].ty {
        Type::Row { fields, rest } => {
            let field_ty = fields.get("name").expect("row field");
            assert_eq!(field_ty, &get_name.ret_type);
            assert!(rest.is_some(), "expected open row");
        }
        other => panic!("expected row type, got {other:?}"),
    }
}

#[test]
fn keeps_generic_class_type_params_as_class_scope_type_vars() {
    let result = parse_and_check(
        r#"
class Stack[T] {
    let items: Array[T]
    fn push(item: T) { item }
}
"#,
    );
    assert!(result.errors.is_empty(), "type errors: {:?}", result.errors);
    let TypedItem::Class(class_def) = &result.typed_program.items[0] else {
        panic!("expected class");
    };
    assert!(matches!(class_def.fields[0].ty, Type::Array(_)));
    assert!(matches!(class_def.methods[0].params[0].ty, Type::Var(_)));
}
