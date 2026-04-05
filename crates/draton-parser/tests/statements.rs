use draton_ast::{AssignOp, DestructureBinding, Expr, Stmt};
use draton_lexer::Lexer;
use draton_parser::{ParseWarning, Parser};

fn parse_stmt(source: &str) -> Stmt {
    let wrapped = format!("fn main() {{ {source} }}");
    let lexed = Lexer::new(&wrapped).tokenize();
    assert!(lexed.errors.is_empty(), "lexer errors: {:?}", lexed.errors);
    let result = Parser::new(lexed.tokens).parse();
    assert!(
        result.errors.is_empty(),
        "parser errors: {:?}",
        result.errors
    );
    let draton_ast::Item::Fn(function) = &result.program.items[0] else {
        panic!("expected function item");
    };
    function
        .body
        .as_ref()
        .and_then(|body| body.stmts.first())
        .cloned()
        .expect("statement")
}

#[test]
fn parses_let_statements() {
    assert!(
        matches!(parse_stmt("let x = 42"), Stmt::Let(let_stmt) if !let_stmt.is_mut && let_stmt.name == "x")
    );
    assert!(
        matches!(parse_stmt("let mut x = 0"), Stmt::Let(let_stmt) if let_stmt.is_mut && let_stmt.name == "x")
    );
    assert!(matches!(
        parse_stmt("let (x, y) = (1, 2)"),
        Stmt::LetDestructure(let_stmt)
            if matches!(let_stmt.names.as_slice(), [DestructureBinding::Name(x), DestructureBinding::Name(y)] if x == "x" && y == "y")
    ));
    assert!(matches!(
        parse_stmt("let (_, y) = (1, 2)"),
        Stmt::LetDestructure(let_stmt)
            if matches!(let_stmt.names.as_slice(), [DestructureBinding::Wildcard, DestructureBinding::Name(y)] if y == "y")
    ));
}

#[test]
fn parses_assignment_variants() {
    assert!(
        matches!(parse_stmt("count += 1"), Stmt::Assign(assign) if assign.op == AssignOp::AddAssign)
    );
    assert!(matches!(parse_stmt("count++"), Stmt::Assign(assign) if assign.op == AssignOp::Inc));
}

#[test]
fn parses_return_and_expression_statements() {
    assert!(
        matches!(parse_stmt("return x"), Stmt::Return(ret) if matches!(&ret.value, Some(Expr::Ident(name, _)) if name == "x"))
    );
    assert!(matches!(
        parse_stmt("print(\"hello\")"),
        Stmt::Expr(Expr::Call(_, _, _))
    ));
}

#[test]
fn warns_that_gc_config_has_no_effect() {
    let wrapped = "fn main() { @gc_config { threshold = 1024 } }";
    let lexed = Lexer::new(wrapped).tokenize();
    assert!(lexed.errors.is_empty(), "lexer errors: {:?}", lexed.errors);
    let result = Parser::new(lexed.tokens).parse();
    assert!(
        result.errors.is_empty(),
        "parser errors: {:?}",
        result.errors
    );
    assert!(result.warnings.iter().any(|warning| matches!(
        warning,
        ParseWarning::DeprecatedSyntax { syntax, replacement, .. }
            if syntax == "@gc_config has no effect"
                && replacement == "Draton uses Inferred Ownership and has no GC runtime"
    )));
}
