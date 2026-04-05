use draton_ast::{BinOp, ElseBranch, Expr, Item, MatchArmBody, SpawnBody, Stmt};
use draton_lexer::Lexer;
use draton_parser::Parser;

fn parse_body(source: &str) -> Vec<Stmt> {
    let wrapped = format!("fn main() {{ {source} }}");
    let lexed = Lexer::new(&wrapped).tokenize();
    assert!(lexed.errors.is_empty(), "lexer errors: {:?}", lexed.errors);
    let result = Parser::new(lexed.tokens).parse();
    assert!(
        result.errors.is_empty(),
        "parser errors: {:?}",
        result.errors
    );
    let Item::Fn(function) = &result.program.items[0] else {
        panic!("expected function");
    };
    function
        .body
        .as_ref()
        .map(|body| body.stmts.clone())
        .unwrap_or_default()
}

#[test]
fn parses_if_else_if_else() {
    let stmts = parse_body("if (x > 10) { print(\"big\") } else if (x == 10) { print(\"ten\") } else { print(\"small\") }");
    match &stmts[0] {
        Stmt::If(if_stmt) => {
            assert!(matches!(if_stmt.condition, Expr::BinOp(_, BinOp::Gt, _, _)));
            assert!(matches!(if_stmt.else_branch, Some(ElseBranch::If(_))));
        }
        other => panic!("expected if, got {other:?}"),
    }
}

#[test]
fn parses_for_while_and_spawn() {
    let stmts = parse_body(
        "for i in 0..10 { print(i) } while (count < 10) { count++ } spawn fetchData(url) spawn { ch.send(42) }",
    );
    assert!(
        matches!(&stmts[0], Stmt::For(for_stmt) if matches!(for_stmt.iter, Expr::BinOp(_, BinOp::Range, _, _)))
    );
    assert!(matches!(&stmts[1], Stmt::While(_)));
    assert!(matches!(&stmts[2], Stmt::Spawn(spawn) if matches!(spawn.body, SpawnBody::Expr(_))));
    assert!(matches!(&stmts[3], Stmt::Spawn(spawn) if matches!(spawn.body, SpawnBody::Block(_))));
}

#[test]
fn parses_doc_comment_only_plain_and_spawn_blocks_as_empty_blocks() {
    let stmts = parse_body(
        r#"
{
    /// gap
}
spawn {
    /// gap
}
"#,
    );
    assert!(matches!(&stmts[0], Stmt::Block(block) if block.stmts.is_empty()));
    assert!(
        matches!(&stmts[1], Stmt::Spawn(spawn) if matches!(&spawn.body, SpawnBody::Block(block) if block.stmts.is_empty()))
    );
}

#[test]
fn parses_match_with_tuple_and_result_patterns() {
    let stmts = parse_body(
        "match point { (0, 0) => print(\"origin\"), (x, y) => print(f\"{x},{y}\") } match getUser(0) { Ok(u) => print(u.name), Err(NotFound(msg)) => print(msg), _ => print(\"other\") }",
    );
    assert!(
        matches!(&stmts[0], Stmt::Expr(Expr::Match(_, arms, _)) if matches!(arms[0].pattern, Expr::Tuple(_, _)) && matches!(arms[1].body, MatchArmBody::Expr(_)))
    );
    assert!(matches!(&stmts[1], Stmt::Expr(Expr::Match(_, arms, _)) if arms.len() == 3));
}
