use draton_lexer::{Lexer, TokenKind};
use pretty_assertions::assert_eq;

#[test]
fn lexes_every_operator_and_delimiter() {
    let source = "+ - * / % = += -= *= /= %= ++ -- == != < <= > >= && || ! & | ^ ~ << >> ? ?? => -> .. @ ( ) [ ] { } , ; : .";
    let result = Lexer::new(source).tokenize();

    assert_eq!(result.errors, Vec::new());
    assert_eq!(
        result
            .tokens
            .into_iter()
            .map(|token| token.kind)
            .collect::<Vec<_>>(),
        vec![
            TokenKind::Plus,
            TokenKind::Minus,
            TokenKind::Star,
            TokenKind::Slash,
            TokenKind::Percent,
            TokenKind::Eq,
            TokenKind::PlusEq,
            TokenKind::MinusEq,
            TokenKind::StarEq,
            TokenKind::SlashEq,
            TokenKind::PercentEq,
            TokenKind::PlusPlus,
            TokenKind::MinusMinus,
            TokenKind::EqEq,
            TokenKind::BangEq,
            TokenKind::Lt,
            TokenKind::LtEq,
            TokenKind::Gt,
            TokenKind::GtEq,
            TokenKind::AmpAmp,
            TokenKind::PipePipe,
            TokenKind::Bang,
            TokenKind::Amp,
            TokenKind::Pipe,
            TokenKind::Caret,
            TokenKind::Tilde,
            TokenKind::LtLt,
            TokenKind::GtGt,
            TokenKind::Question,
            TokenKind::QuestionQuestion,
            TokenKind::FatArrow,
            TokenKind::Arrow,
            TokenKind::DotDot,
            TokenKind::At,
            TokenKind::LParen,
            TokenKind::RParen,
            TokenKind::LBracket,
            TokenKind::RBracket,
            TokenKind::LBrace,
            TokenKind::RBrace,
            TokenKind::Comma,
            TokenKind::Semicolon,
            TokenKind::Colon,
            TokenKind::Dot,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn prefers_longest_operator_match() {
    let source = "++ -- += -= == != <= >= && || ?? .. << >> => ->";
    let result = Lexer::new(source).tokenize();

    assert_eq!(result.errors, Vec::new());
    assert_eq!(
        result
            .tokens
            .into_iter()
            .map(|token| token.lexeme)
            .collect::<Vec<_>>(),
        vec![
            "++".to_string(),
            "--".to_string(),
            "+=".to_string(),
            "-=".to_string(),
            "==".to_string(),
            "!=".to_string(),
            "<=".to_string(),
            ">=".to_string(),
            "&&".to_string(),
            "||".to_string(),
            "??".to_string(),
            "..".to_string(),
            "<<".to_string(),
            ">>".to_string(),
            "=>".to_string(),
            "->".to_string(),
            String::new(),
        ]
    );
}

#[test]
fn does_not_split_compound_operators() {
    let result = Lexer::new("a??b x..y z++ --w").tokenize();

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
            TokenKind::Ident,
            TokenKind::DotDot,
            TokenKind::Ident,
            TokenKind::Ident,
            TokenKind::PlusPlus,
            TokenKind::MinusMinus,
            TokenKind::Ident,
            TokenKind::Eof,
        ]
    );
}
