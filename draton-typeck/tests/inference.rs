use draton_lexer::Lexer;
use draton_parser::Parser;
use draton_typeck::{Type, TypeChecker, TypedExprKind, TypedItem, TypedStmtKind};

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
fn infers_basic_let_bindings() {
    let result = parse_and_check(
        r#"
fn main() {
    let x = 42
    let y = 3.14
    let z = "hello"
    let ok = true
}
"#,
    );
    assert!(result.errors.is_empty(), "type errors: {:?}", result.errors);
    let TypedItem::Fn(function) = &result.typed_program.items[0] else {
        panic!("expected function");
    };
    let body = function.body.as_ref().expect("body");
    match &body.stmts[0].kind {
        TypedStmtKind::Let(let_stmt) => assert_eq!(let_stmt.ty, Type::Int),
        other => panic!("unexpected stmt: {other:?}"),
    }
    match &body.stmts[1].kind {
        TypedStmtKind::Let(let_stmt) => assert_eq!(let_stmt.ty, Type::Float),
        other => panic!("unexpected stmt: {other:?}"),
    }
    match &body.stmts[2].kind {
        TypedStmtKind::Let(let_stmt) => assert_eq!(let_stmt.ty, Type::String),
        other => panic!("unexpected stmt: {other:?}"),
    }
    match &body.stmts[3].kind {
        TypedStmtKind::Let(let_stmt) => assert_eq!(let_stmt.ty, Type::Bool),
        other => panic!("unexpected stmt: {other:?}"),
    }
}

#[test]
fn infers_function_lambda_and_collections() {
    let result = parse_and_check(
        r#"
@type { fn add(a: Int, b: Int) -> Int }
fn add(a, b) { a + b }
fn main() {
    let arr = [1, 2, 3]
    let tup = (1, "hello")
    let mapper = lambda x => x * 2
    arr.map(mapper)
}
"#,
    );
    assert!(result.errors.is_empty(), "type errors: {:?}", result.errors);
    let TypedItem::Fn(add_fn) = &result.typed_program.items[1] else {
        panic!("expected add function");
    };
    assert_eq!(
        add_fn.ty,
        Type::Fn(vec![Type::Int, Type::Int], Box::new(Type::Int))
    );

    let TypedItem::Fn(main_fn) = &result.typed_program.items[2] else {
        panic!("expected main function");
    };
    let body = main_fn.body.as_ref().expect("body");
    match &body.stmts[0].kind {
        TypedStmtKind::Let(let_stmt) => {
            assert_eq!(let_stmt.ty, Type::Array(Box::new(Type::Int)));
        }
        other => panic!("unexpected stmt: {other:?}"),
    }
    match &body.stmts[1].kind {
        TypedStmtKind::Let(let_stmt) => {
            assert_eq!(let_stmt.ty, Type::Tuple(vec![Type::Int, Type::String]));
        }
        other => panic!("unexpected stmt: {other:?}"),
    }
    match &body.stmts[2].kind {
        TypedStmtKind::Let(let_stmt) => {
            assert_eq!(let_stmt.ty, Type::Fn(vec![Type::Int], Box::new(Type::Int)));
        }
        other => panic!("unexpected stmt: {other:?}"),
    }
    match &body.stmts[3].kind {
        TypedStmtKind::Expr(expr) => match &expr.kind {
            TypedExprKind::MethodCall(_, name, _) => {
                assert_eq!(name, "map");
                assert_eq!(expr.ty, Type::Array(Box::new(Type::Int)));
            }
            other => panic!("unexpected expr: {other:?}"),
        },
        other => panic!("unexpected stmt: {other:?}"),
    }
}

#[test]
fn infers_function_without_type_block_from_usage() {
    let result = parse_and_check(
        r#"
fn add(a, b) { a + b }
fn main() {
    let sum = add(1, 2)
}
"#,
    );
    assert!(result.errors.is_empty(), "type errors: {:?}", result.errors);
    let TypedItem::Fn(main_fn) = &result.typed_program.items[1] else {
        panic!("expected main function");
    };
    let body = main_fn.body.as_ref().expect("body");
    match &body.stmts[0].kind {
        TypedStmtKind::Let(let_stmt) => assert_eq!(let_stmt.ty, Type::Int),
        other => panic!("unexpected stmt: {other:?}"),
    }
}

#[test]
fn infers_map_and_tuple_types() {
    let result = parse_and_check(
        r#"
fn main() {
    let person = {name: "alice"}
    let pair = (1, "hello")
}
"#,
    );
    assert!(result.errors.is_empty(), "type errors: {:?}", result.errors);
    let TypedItem::Fn(main_fn) = &result.typed_program.items[0] else {
        panic!("expected function");
    };
    let body = main_fn.body.as_ref().expect("body");
    match &body.stmts[0].kind {
        TypedStmtKind::Let(let_stmt) => {
            assert_eq!(
                let_stmt.ty,
                Type::Map(Box::new(Type::String), Box::new(Type::String))
            );
        }
        other => panic!("unexpected stmt: {other:?}"),
    }
    match &body.stmts[1].kind {
        TypedStmtKind::Let(let_stmt) => {
            assert_eq!(let_stmt.ty, Type::Tuple(vec![Type::Int, Type::String]));
        }
        other => panic!("unexpected stmt: {other:?}"),
    }
}

#[test]
fn infers_lambda_params_from_annotated_function_type() {
    let result = parse_and_check(
        r#"
fn main() {
    let values = [1, 2, 3]
    values.map(lambda x => x * 2)
}
"#,
    );
    assert!(result.errors.is_empty(), "type errors: {:?}", result.errors);
    let TypedItem::Fn(main_fn) = &result.typed_program.items[0] else {
        panic!("expected function");
    };
    let body = main_fn.body.as_ref().expect("body");
    match &body.stmts[1].kind {
        TypedStmtKind::Expr(expr) => match &expr.kind {
            TypedExprKind::MethodCall(_, name, args) => {
                assert_eq!(name, "map");
                let lambda = &args[0];
                match &lambda.kind {
                    TypedExprKind::Lambda(params, body) => {
                        assert_eq!(params[0].ty, Type::Int);
                        assert_eq!(body.ty, Type::Int);
                    }
                    other => panic!("unexpected expr: {other:?}"),
                }
                assert_eq!(expr.ty, Type::Array(Box::new(Type::Int)));
            }
            other => panic!("unexpected expr: {other:?}"),
        },
        other => panic!("unexpected stmt: {other:?}"),
    }
}

#[test]
fn arithmetic_without_annotation_defaults_to_int() {
    let result = parse_and_check(
        r#"
fn double(x) { x * 2 }
fn main() {
    let n = double(5)
}
"#,
    );
    assert!(result.errors.is_empty(), "type errors: {:?}", result.errors);
    let TypedItem::Fn(double_fn) = &result.typed_program.items[0] else {
        panic!("expected function");
    };
    assert_eq!(double_fn.ty, Type::Fn(vec![Type::Int], Box::new(Type::Int)));
}
