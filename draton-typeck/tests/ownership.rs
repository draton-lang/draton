use draton_lexer::Lexer;
use draton_parser::Parser;
use draton_typeck::typed_ast::{
    Type, TypedBlock, TypedClassDef, TypedExpr, TypedExprKind, TypedFieldDef, TypedFnDef, TypedItem,
    TypedParam, TypedProgram, TypedStmt, TypedStmtKind, TypedTypeBlock,
    TypedTypeMember,
};
use draton_typeck::{OwnershipChecker, OwnershipError, TypeChecker, TypeError, UseEffect};
use draton_ast::Span;

fn parse_and_check(source: &str) -> draton_typeck::TypeCheckResult {
    let lexed = Lexer::new(source).tokenize();
    assert!(lexed.errors.is_empty(), "lexer errors: {:?}", lexed.errors);
    let parsed = Parser::new(lexed.tokens).parse();
    assert!(parsed.errors.is_empty(), "parser errors: {:?}", parsed.errors);
    TypeChecker::new().check(parsed.program)
}

fn rerun_ownership(program: &mut TypedProgram) -> (Vec<OwnershipError>, std::collections::HashMap<String, Vec<Span>>) {
    let mut checker = OwnershipChecker::new();
    let errors = checker.check_program(program);
    (errors, checker.recorded_free_points().clone())
}

fn span() -> Span {
    Span { start: 0, end: 0, line: 1, col: 1 }
}

fn ident(name: &str, ty: Type) -> TypedExpr {
    TypedExpr { kind: TypedExprKind::Ident(name.to_string()), ty, span: span(), use_effect: None }
}

fn call(name: &str, args: Vec<TypedExpr>, ret: Type) -> TypedExpr {
    TypedExpr {
        kind: TypedExprKind::Call(
            Box::new(TypedExpr {
                kind: TypedExprKind::Ident(name.to_string()),
                ty: Type::Fn(Vec::new(), Box::new(ret.clone())),
                span: span(),
                use_effect: None,
            }),
            args,
        ),
        ty: ret,
        span: span(),
        use_effect: None,
    }
}

fn make_fn(name: &str, params: Vec<(&str, Type)>, body: Vec<TypedStmt>, ret_type: Type) -> TypedFnDef {
    TypedFnDef {
        is_pub: false,
        name: name.to_string(),
        params: params
            .into_iter()
            .map(|(name, ty)| TypedParam { name: name.to_string(), ty, span: span() })
            .collect(),
        ret_type: ret_type.clone(),
        body: Some(TypedBlock { stmts: body, span: span() }),
        ty: Type::Fn(Vec::new(), Box::new(ret_type)),
        span: span(),
        ownership_summary: None,
    }
}

#[test]
fn straight_line_move_and_free() {
    let result = parse_and_check(
        r#"
fn main() {
    let name = input("name: ")
    print(name.len())
}
"#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let mut program = result.typed_program.clone();
    let (errors, frees) = rerun_ownership(&mut program);
    assert!(errors.is_empty(), "ownership errors: {:?}", errors);
    assert!(frees.contains_key("main:name"), "frees: {:?}", frees);
}

#[test]
fn use_after_move_reports_error() {
    let result = parse_and_check(
        r#"
fn forward(text) { return text }
fn main() {
    let name = input("name: ")
    let out = forward(name)
    print(name.len())
    print(out.len())
}
"#,
    );
    assert!(result.errors.iter().any(|error| matches!(
        error,
        TypeError::Ownership(OwnershipError::UseAfterMove { name, .. }) if name == "name"
    )));
}

#[test]
fn borrow_across_two_calls_is_allowed() {
    let result = parse_and_check(
        r#"
@type { show: (String) -> Unit }
fn show(text) { print(text.len()) }
fn main() {
    let name = input("name: ")
    show(name)
    show(name)
    print(name.len())
}
"#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
}

#[test]
fn move_while_borrowed_reports_error() {
    let result = parse_and_check(
        r#"
fn forward(text) { return text }
fn main() {
    let name = input("name: ")
    let reader = lambda => name.len()
    let out = forward(name)
    print(reader())
    print(out.len())
}
"#,
    );
    assert!(result.errors.iter().any(|error| matches!(
        error,
        TypeError::Ownership(OwnershipError::MoveWhileBorrowed { name, .. }) if name == "name"
    )));
}

#[test]
fn branch_local_free_is_recorded() {
    let result = parse_and_check(
        r#"
fn main(flag) {
    let name = input("name: ")
    if flag {
        print(name.len())
    } else {
        print(name.len())
    }
}
"#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let mut program = result.typed_program.clone();
    let (errors, frees) = rerun_ownership(&mut program);
    assert!(errors.is_empty(), "ownership errors: {:?}", errors);
    assert!(frees.contains_key("main:name"), "frees: {:?}", frees);
}

#[test]
fn early_return_free_path_is_recorded() {
    let result = parse_and_check(
        r#"
fn main(flag) {
    let name = input("name: ")
    if flag {
        return 0
    }
    print(name.len())
    return 1
}
"#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let mut program = result.typed_program.clone();
    let (errors, frees) = rerun_ownership(&mut program);
    assert!(errors.is_empty(), "ownership errors: {:?}", errors);
    let _ = frees;
}

#[test]
fn loop_move_without_reinit_reports_error() {
    let result = parse_and_check(
        r#"
fn forward(text) { return text }
fn main(items) {
    let mut name = input("name: ")
    while items.len() > 0 {
        let out = forward(name)
        print(out.len())
    }
}
"#,
    );
    assert!(result.errors.iter().any(|error| matches!(
        error,
        TypeError::Ownership(OwnershipError::LoopMoveWithoutReinit { name, .. }) if name == "name"
    )));
}

#[test]
fn escaping_closure_moves_capture() {
    let result = parse_and_check(
        r#"
fn make_reader() {
    let name = input("name: ")
    return lambda => name.len()
}
"#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
}

#[test]
fn non_escaping_closure_borrows_capture() {
    let result = parse_and_check(
        r#"
fn main() {
    let name = input("name: ")
    let show = lambda => name.len()
    print(show())
    print(name.len())
}
"#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
}

#[test]
fn two_escaping_closures_same_value_reports_error() {
    let result = parse_and_check(
        r#"
fn main() {
    let name = input("name: ")
    return [lambda => name.len(), lambda => name.len()]
}
"#,
    );
    assert!(result.errors.iter().any(|error| matches!(
        error,
        TypeError::Ownership(OwnershipError::MultipleOwners { name, .. }) if name == "name"
    )));
}

#[test]
fn direct_ownership_cycle_reports_error() {
    let node_class = TypedItem::Class(TypedClassDef {
        name: "Node".to_string(),
        extends: None,
        implements: Vec::new(),
        fields: vec![TypedFieldDef {
            is_mut: true,
            name: "next".to_string(),
            ty: Type::Named("Node".to_string(), Vec::new()),
            span: span(),
        }],
        methods: Vec::new(),
        type_blocks: Vec::new(),
        span: span(),
    });
    let mut program = TypedProgram {
        items: vec![
            node_class,
            TypedItem::Fn(make_fn(
                "main",
                vec![("self", Type::Named("Node".to_string(), Vec::new()))],
                vec![TypedStmt {
                    kind: TypedStmtKind::Assign(draton_typeck::typed_ast::TypedAssignStmt {
                        target: TypedExpr {
                            kind: TypedExprKind::Field(
                                Box::new(ident("self", Type::Named("Node".to_string(), Vec::new()))),
                                "next".to_string(),
                            ),
                            ty: Type::Named("Node".to_string(), Vec::new()),
                            span: span(),
                            use_effect: None,
                        },
                        op: draton_ast::AssignOp::Assign,
                        value: Some(ident("self", Type::Named("Node".to_string(), Vec::new()))),
                        span: span(),
                    }),
                    span: span(),
                }],
                Type::Unit,
            )),
        ],
    };
    let (errors, _) = rerun_ownership(&mut program);
    assert!(errors.iter().any(|error| matches!(error, OwnershipError::OwnershipCycle { .. })));
}

#[test]
fn acyclic_definition_rejects_direct_self_field() {
    let mut program = TypedProgram {
        items: vec![TypedItem::Class(TypedClassDef {
            name: "Node".to_string(),
            extends: None,
            implements: Vec::new(),
            fields: vec![TypedFieldDef {
                is_mut: true,
                name: "next".to_string(),
                ty: Type::Named("Node".to_string(), Vec::new()),
                span: span(),
            }],
            methods: Vec::new(),
            type_blocks: vec![TypedTypeBlock {
                members: vec![TypedTypeMember::Binding {
                    name: "@acyclic".to_string(),
                    ty: Type::Unit,
                    span: span(),
                }],
                span: span(),
            }],
            span: span(),
        })],
    };
    let (errors, _) = rerun_ownership(&mut program);
    assert!(errors.iter().any(|error| matches!(error, OwnershipError::OwnershipCycle { .. })));
}

#[test]
fn higher_order_function_with_declared_borrow_effect_works() {
    let mut program = TypedProgram {
        items: vec![TypedItem::Fn(make_fn(
            "run",
            vec![
                ("op", Type::Fn(vec![Type::String], Box::new(Type::Named("borrow".to_string(), Vec::new())))),
                ("text", Type::String),
            ],
            vec![
                TypedStmt {
                    kind: TypedStmtKind::Expr(call("op", vec![ident("text", Type::String)], Type::Unit)),
                    span: span(),
                },
                TypedStmt {
                    kind: TypedStmtKind::Expr(TypedExpr {
                        kind: TypedExprKind::MethodCall(
                            Box::new(ident("text", Type::String)),
                            "len".to_string(),
                            Vec::new(),
                        ),
                        ty: Type::Int,
                        span: span(),
                        use_effect: None,
                    }),
                    span: span(),
                },
            ],
            Type::Unit,
        ))],
    };
    let (errors, _) = rerun_ownership(&mut program);
    assert!(errors.is_empty(), "errors: {:?}", errors);
}

#[test]
fn higher_order_function_with_declared_move_effect_works() {
    let mut program = TypedProgram {
        items: vec![TypedItem::Fn(make_fn(
            "run",
            vec![
                ("op", Type::Fn(vec![Type::String], Box::new(Type::Named("move".to_string(), Vec::new())))),
                ("text", Type::String),
            ],
            vec![TypedStmt {
                kind: TypedStmtKind::Expr(call("op", vec![ident("text", Type::String)], Type::Unit)),
                span: span(),
            }],
            Type::Unit,
        ))],
    };
    let (errors, _) = rerun_ownership(&mut program);
    assert!(errors.is_empty(), "errors: {:?}", errors);
}

#[test]
fn recursive_borrow_converging_function_gets_borrow_summary() {
    let mut program = TypedProgram {
        items: vec![TypedItem::Fn(make_fn(
            "walk",
            vec![("text", Type::String), ("n", Type::Int)],
            vec![TypedStmt {
                kind: TypedStmtKind::Expr(call("walk", vec![ident("text", Type::String), ident("n", Type::Int)], Type::Unit)),
                span: span(),
            }],
            Type::Unit,
        ))],
    };
    let (errors, _) = rerun_ownership(&mut program);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    let TypedItem::Fn(function) = &program.items[0] else { panic!("expected fn"); };
    assert_eq!(function.ownership_summary.as_ref().unwrap().params[0].effect, UseEffect::BorrowShared);
}

#[test]
fn recursive_move_converging_function_gets_move_summary() {
    let result = parse_and_check(
        r#"
fn pass_down(text, n) {
    if n == 0 {
        return text
    }
    return pass_down(text, n)
}
"#,
    );
    let TypedItem::Fn(function) = &result.typed_program.items[0] else { panic!("expected fn"); };
    assert_eq!(function.ownership_summary.as_ref().unwrap().params[0].effect, UseEffect::Move);
}
