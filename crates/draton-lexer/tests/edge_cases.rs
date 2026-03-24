use draton_lexer::{LexError, Lexer, Span, TokenKind};
use pretty_assertions::assert_eq;

#[test]
fn empty_input_only_emits_eof() {
    let result = Lexer::new("").tokenize();

    assert_eq!(result.errors, Vec::new());
    assert_eq!(result.tokens.len(), 1);
    assert_eq!(result.tokens[0].kind, TokenKind::Eof);
}

#[test]
fn whitespace_only_emits_eof() {
    let result = Lexer::new("  \t\r\n\n").tokenize();

    assert_eq!(result.errors, Vec::new());
    assert_eq!(result.tokens.len(), 1);
    assert_eq!(result.tokens[0].kind, TokenKind::Eof);
}

#[test]
fn collects_multiple_errors() {
    let result = Lexer::new("\"oops\n0xZZZ #").tokenize();

    assert_eq!(
        result.errors,
        vec![
            LexError::UnterminatedString { line: 1, col: 1 },
            LexError::InvalidNumericLiteral {
                lexeme: "0xZZZ".to_string(),
                line: 2,
                col: 1,
            },
            LexError::UnexpectedChar {
                found: '#',
                line: 2,
                col: 7,
            },
        ]
    );
}

#[test]
fn tracks_token_spans_across_lines() {
    let result = Lexer::new("let x = 1\nmut y = 2").tokenize();

    assert_eq!(result.errors, Vec::new());
    assert_eq!(
        result.tokens[0].span,
        Span {
            start: 0,
            end: 3,
            line: 1,
            col: 1,
        }
    );
    assert_eq!(
        result.tokens[4].span,
        Span {
            start: 10,
            end: 13,
            line: 2,
            col: 1,
        }
    );
    assert_eq!(
        result.tokens[7].span,
        Span {
            start: 18,
            end: 19,
            line: 2,
            col: 9,
        }
    );
}

#[test]
fn reports_unterminated_string() {
    let result = Lexer::new("\"hello").tokenize();

    assert_eq!(
        result.errors,
        vec![LexError::UnterminatedString { line: 1, col: 1 }]
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

#[test]
fn none_is_not_an_identifier() {
    let result = Lexer::new("None NoneValue").tokenize();

    assert_eq!(result.errors, Vec::new());
    assert_eq!(result.tokens[0].kind, TokenKind::NoneLit);
    assert_eq!(result.tokens[1].kind, TokenKind::Ident);
}

#[test]
fn range_operator_is_not_two_dots() {
    let result = Lexer::new("1..2").tokenize();

    assert_eq!(result.errors, Vec::new());
    assert_eq!(
        result
            .tokens
            .into_iter()
            .map(|token| token.kind)
            .collect::<Vec<_>>(),
        vec![
            TokenKind::IntLit,
            TokenKind::DotDot,
            TokenKind::IntLit,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn null_coalescing_operator_is_not_two_questions() {
    let result = Lexer::new("a ?? b").tokenize();

    assert_eq!(result.errors, Vec::new());
    assert_eq!(
        result
            .tokens
            .into_iter()
            .map(|token| token.kind)
            .collect::<Vec<_>>(),
        vec![
            TokenKind::Ident,
            TokenKind::QuestionQuestion,
            TokenKind::Ident,
            TokenKind::Eof,
        ]
    );
}
