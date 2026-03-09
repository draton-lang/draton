/// Errors produced during Draton type inference and checking.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum TypeError {
    #[error("type mismatch at line {line}, col {col}\n  expected: {expected}\n  found:    {found}\n  hint:     {hint}")]
    Mismatch {
        expected: String,
        found: String,
        hint: String,
        line: usize,
        col: usize,
    },

    #[error("undefined variable '{name}' at line {line}, col {col}")]
    UndefinedVar {
        name: String,
        line: usize,
        col: usize,
    },

    #[error("undefined function '{name}' at line {line}, col {col}")]
    UndefinedFn {
        name: String,
        line: usize,
        col: usize,
    },

    #[error("field '{field}' not found on type '{ty}' at line {line}, col {col}")]
    NoField {
        field: String,
        ty: String,
        line: usize,
        col: usize,
    },

    #[error("cannot apply '{op}' to types '{lhs}' and '{rhs}' at line {line}, col {col}")]
    BadBinOp {
        op: String,
        lhs: String,
        rhs: String,
        line: usize,
        col: usize,
    },

    #[error("wrong number of arguments: expected {expected}, got {got} at line {line}, col {col}")]
    ArgCount {
        expected: usize,
        got: usize,
        line: usize,
        col: usize,
    },

    #[error("cannot infer type for '{name}' at line {line}, col {col} — add a @type annotation")]
    CannotInfer {
        name: String,
        line: usize,
        col: usize,
    },

    #[error("infinite type detected involving '{var}' at line {line}, col {col}")]
    InfiniteType {
        var: String,
        line: usize,
        col: usize,
    },

    #[error("cannot cast '{from}' to '{to}' at line {line}, col {col}")]
    BadCast {
        from: String,
        to: String,
        line: usize,
        col: usize,
    },
}
