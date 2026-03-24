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

    #[error("nested layer not allowed at line {line}, col {col}")]
    NestedLayerNotAllowed { line: usize, col: usize },

    #[error("layer outside class at line {line}, col {col}")]
    LayerOutsideClass { line: usize, col: usize },
}

/// Warnings that can be produced while parsing Draton tokens.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ParseWarning {
    #[error("deprecated syntax '{syntax}' at line {line}, col {col}\n  use: {replacement}")]
    DeprecatedSyntax {
        syntax: String,
        replacement: String,
        line: usize,
        col: usize,
    },
}
