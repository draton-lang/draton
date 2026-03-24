use draton_lexer::{Lexer, TokenKind};
use pretty_assertions::assert_eq;

fn kinds(source: &str) -> Vec<TokenKind> {
    Lexer::new(source)
        .tokenize()
        .tokens
        .into_iter()
        .map(|token| token.kind)
        .collect()
}

#[test]
fn lexes_integer_literals() {
    assert_eq!(
        kinds("42 0 9223372036854775807"),
        vec![
            TokenKind::IntLit,
            TokenKind::IntLit,
            TokenKind::IntLit,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn lexes_float_literals() {
    let result = Lexer::new("3.14 0.0 1.0").tokenize();
    assert_eq!(result.errors, Vec::new());
    assert_eq!(
        result
            .tokens
            .into_iter()
            .map(|token| token.kind)
            .collect::<Vec<_>>(),
        vec![
            TokenKind::FloatLit,
            TokenKind::FloatLit,
            TokenKind::FloatLit,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn lexes_hex_literals() {
    let result = Lexer::new("0xFF 0xDEADBEEF").tokenize();
    assert_eq!(result.errors, Vec::new());
    assert_eq!(result.tokens[0].lexeme, "0xFF".to_string());
    assert_eq!(result.tokens[1].lexeme, "0xDEADBEEF".to_string());
    assert_eq!(result.tokens[0].kind, TokenKind::HexLit);
    assert_eq!(result.tokens[1].kind, TokenKind::HexLit);
}

#[test]
fn lexes_binary_literals() {
    let result = Lexer::new("0b1010").tokenize();
    assert_eq!(result.errors, Vec::new());
    assert_eq!(result.tokens[0].kind, TokenKind::BinLit);
    assert_eq!(result.tokens[0].lexeme, "0b1010".to_string());
}

#[test]
fn lexes_string_literals() {
    let result = Lexer::new("\"hello\" \"\"").tokenize();
    assert_eq!(result.errors, Vec::new());
    assert_eq!(result.tokens[0].kind, TokenKind::StrLit);
    assert_eq!(result.tokens[0].lexeme, "\"hello\"".to_string());
    assert_eq!(result.tokens[1].kind, TokenKind::StrLit);
    assert_eq!(result.tokens[1].lexeme, "\"\"".to_string());
}

#[test]
fn lexes_fstring_literals() {
    let result = Lexer::new("f\"hello {name}\"").tokenize();
    assert_eq!(result.errors, Vec::new());
    assert_eq!(result.tokens[0].kind, TokenKind::FStrLit);
    assert_eq!(result.tokens[0].lexeme, "f\"hello {name}\"".to_string());
}

#[test]
fn lexes_boolean_literals() {
    assert_eq!(
        kinds("true false"),
        vec![TokenKind::BoolLit, TokenKind::BoolLit, TokenKind::Eof]
    );
}

#[test]
fn lexes_none_literal() {
    let result = Lexer::new("None").tokenize();
    assert_eq!(result.errors, Vec::new());
    assert_eq!(result.tokens[0].kind, TokenKind::NoneLit);
    assert_eq!(result.tokens[0].lexeme, "None".to_string());
}
