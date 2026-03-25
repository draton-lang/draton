use crate::token::Token;
use serde::Serialize;

/// Errors that can be produced while lexing Draton source.
#[derive(Debug, thiserror::Error, PartialEq, Eq, Serialize)]
pub enum LexError {
    #[error("unexpected character '{found}' at line {line}, col {col}")]
    UnexpectedChar {
        found: char,
        line: usize,
        col: usize,
    },

    #[error("unterminated string literal starting at line {line}, col {col}")]
    UnterminatedString { line: usize, col: usize },

    #[error("unterminated block comment starting at line {line}, col {col}")]
    UnterminatedBlockComment { line: usize, col: usize },

    #[error("invalid numeric literal '{lexeme}' at line {line}, col {col}")]
    InvalidNumericLiteral {
        lexeme: String,
        line: usize,
        col: usize,
    },
}

/// The full result of tokenizing a source file.
#[derive(Debug, Default, PartialEq, Eq, Serialize)]
pub struct LexResult {
    pub tokens: Vec<Token>,
    pub errors: Vec<LexError>,
}
