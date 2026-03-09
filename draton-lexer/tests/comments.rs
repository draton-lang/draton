use draton_lexer::{LexError, Lexer, TokenKind};
use pretty_assertions::assert_eq;

#[test]
fn skips_single_line_comments() {
    let result = Lexer::new("let // ignored\nmut").tokenize();

    assert_eq!(result.errors, Vec::new());
    assert_eq!(
        result
            .tokens
            .into_iter()
            .map(|token| token.kind)
            .collect::<Vec<_>>(),
        vec![TokenKind::Let, TokenKind::Mut, TokenKind::Eof]
    );
}

#[test]
fn skips_block_comments() {
    let result = Lexer::new("let /* ignored */ mut").tokenize();

    assert_eq!(result.errors, Vec::new());
    assert_eq!(
        result
            .tokens
            .into_iter()
            .map(|token| token.kind)
            .collect::<Vec<_>>(),
        vec![TokenKind::Let, TokenKind::Mut, TokenKind::Eof]
    );
}

#[test]
fn emits_doc_comments() {
    let result = Lexer::new("/// docs here\nlet").tokenize();

    assert_eq!(result.errors, Vec::new());
    assert_eq!(result.tokens[0].kind, TokenKind::DocComment);
    assert_eq!(result.tokens[0].lexeme, " docs here".to_string());
    assert_eq!(result.tokens[1].kind, TokenKind::Let);
}

#[test]
fn reports_unterminated_block_comments() {
    let result = Lexer::new("/* missing end").tokenize();

    assert_eq!(
        result.errors,
        vec![LexError::UnterminatedBlockComment { line: 1, col: 1 }]
    );
    assert_eq!(
        result
            .tokens
            .into_iter()
            .map(|token| token.kind)
            .collect::<Vec<_>>(),
        vec![TokenKind::Eof]
    );
}
