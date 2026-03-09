use draton_ast::{Expr, Item};
use draton_lexer::Lexer;
use draton_parser::Parser;

fn parse_program(source: &str) -> draton_parser::ParseResult {
    let lexed = Lexer::new(source).tokenize();
    assert!(lexed.errors.is_empty(), "lexer errors: {:?}", lexed.errors);
    Parser::new(lexed.tokens).parse()
}

#[test]
fn parses_empty_or_comment_only_files() {
    let empty = parse_program("");
    assert!(empty.errors.is_empty());
    assert!(empty.program.items.is_empty());

    let comments = parse_program("/// docs\n/// more docs");
    assert!(comments.errors.is_empty());
    assert!(comments.program.items.is_empty());
}

#[test]
fn recovers_after_bad_top_level_tokens() {
    let result = parse_program("fn broken( { }\nconst MAX = 1");
    assert!(!result.errors.is_empty());
    assert!(matches!(result.program.items.last(), Some(Item::Const(_))));
}

#[test]
fn parses_deeply_nested_expressions_and_chains() {
    let result = parse_program("const VALUE = (((((((1 + 2))))))).toString().trim().len()");
    assert!(
        result.errors.is_empty(),
        "parser errors: {:?}",
        result.errors
    );
    assert!(
        matches!(&result.program.items[0], Item::Const(const_def) if matches!(const_def.value, Expr::MethodCall(_, _, _, _)))
    );
}
