/// Errors that can be produced while parsing Draton tokens.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ParseError {
    #[error("unexpected token '{found}' at line {line}, col {col}, expected {expected}")]
    UnexpectedToken {
        found: String,
        expected: String,
        line: usize,
        col: usize,
    },

    #[error("unexpected end of file at line {line}, col {col}, expected {expected}")]
    UnexpectedEof {
        expected: String,
        line: usize,
        col: usize,
    },

    #[error("invalid expression at line {line}, col {col}")]
    InvalidExpr { line: usize, col: usize },
}
