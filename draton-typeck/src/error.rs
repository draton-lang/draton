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

    #[error("destructure mismatch at line {line}, col {col}: pattern has {pattern_len} bindings but type has {tuple_len} slots")]
    DestructureArity {
        pattern_len: usize,
        tuple_len: usize,
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

    #[error("incompatible error propagation types at line {line}, col {col}\n  left:  {lhs}\n  right: {rhs}\n  hint:  wrap both in a shared error type")]
    IncompatibleErrors {
        lhs: String,
        rhs: String,
        line: usize,
        col: usize,
    },

    #[error("class '{class}' does not fully implement interface '{interface}' at line {line}, col {col}\n  missing method: {method}")]
    MissingInterfaceMethod {
        class: String,
        interface: String,
        method: String,
        line: usize,
        col: usize,
    },

    #[error("circular inheritance at line {line}, col {col}: class {class} inherits from itself")]
    CircularInheritance {
        class: String,
        line: usize,
        col: usize,
    },

    #[error("undefined parent class '{parent}' for class '{class}' at line {line}, col {col}")]
    UndefinedParent {
        class: String,
        parent: String,
        line: usize,
        col: usize,
    },

    #[error("non-exhaustive match at line {line}, col {col}\n  missing patterns: {missing}\n  hint: add a wildcard arm `_ => ...` or cover the missing cases")]
    NonExhaustiveMatch {
        missing: String,
        line: usize,
        col: usize,
    },

    #[error("redundant pattern '{pattern}' at line {line}, col {col}")]
    RedundantPattern {
        pattern: String,
        line: usize,
        col: usize,
    },
}
