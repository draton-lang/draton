use draton_ast::Span;

/// Ownership-specific errors produced during inferred ownership checking.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum OwnershipError {
    /// '{name}' was moved here and cannot be used again
    #[error("'{name}' was moved here and cannot be used again\nhint: use '{name}' before the move or assign a new value to it first")]
    UseAfterMove {
        name: String,
        move_span: Span,
        use_span: Span,
    },

    /// cannot move '{name}' while it is still borrowed
    #[error("cannot move '{name}' while it is still borrowed\nhint: finish the earlier read first, or move '{name}' after the borrow ends")]
    MoveWhileBorrowed {
        name: String,
        borrow_span: Span,
        move_span: Span,
    },

    /// cannot read '{name}' here because it is still being modified
    #[error("cannot read '{name}' here because it is still being modified\nhint: move this read after the modification finishes")]
    ReadDuringExclusiveBorrow {
        name: String,
        borrow_span: Span,
        read_span: Span,
    },

    /// cannot modify '{name}' here because it is still being read
    #[error("cannot modify '{name}' here because it is still being read\nhint: move the modification later, or shorten the earlier read")]
    ExclusiveBorrowDuringRead {
        name: String,
        read_span: Span,
        modify_span: Span,
    },

    /// cannot move field '{field}' out of '{base}' without moving the whole value
    #[error("cannot move field '{field}' out of '{base}' without moving the whole value\nhint: move '{base}' as a whole, duplicate '{field}' explicitly, or use @pointer")]
    PartialMove {
        field: String,
        base: String,
        span: Span,
    },

    /// cannot decide whether this call should borrow or move '{name}'
    #[error("cannot decide whether this call should borrow or move '{name}'\nhint: call a more specific function, split the control flow, or use @pointer")]
    AmbiguousCallOwnership { name: String, span: Span },

    /// '{name}' does not live long enough for this use
    #[error("'{name}' does not live long enough for this use\nhint: return or store an owned value instead, or move '{name}' into the closure")]
    BorrowedValueEscapes { name: String, span: Span },

    /// '{name}' would end up with more than one owner
    #[error("'{name}' would end up with more than one owner\nhint: keep exactly one owner, duplicate the value explicitly, or use @pointer for shared access")]
    MultipleOwners { name: String, span: Span },

    /// this assignment would create an ownership cycle
    #[error("this assignment would create an ownership cycle\nhint: keep the ownership graph acyclic, or use @pointer for cyclic structures")]
    OwnershipCycle { span: Span },

    /// '{name}' is moved in one loop iteration but the loop may use it again
    #[error("'{name}' is moved in one loop iteration but the loop may use it again\nhint: reassign '{name}' before the next iteration, or move the value outside the loop")]
    LoopMoveWithoutReinit { name: String, span: Span },

    /// cannot pass owned value '{name}' to external code
    #[error("cannot pass owned value '{name}' to external code\nhint: convert it to @pointer inside @unsafe, or keep the call in safe Draton")]
    ExternalBoundaryRejection { name: String, span: Span },

    /// owned value '{name}' cannot cross into @pointer while a safe owner still exists
    #[error("owned value '{name}' cannot cross into @pointer while a safe owner still exists\nhint: move '{name}' completely, or create the raw value entirely inside @pointer")]
    SafeToRawAliasRejection { name: String, span: Span },
}

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

    #[error("deprecated syntax '{syntax}' at line {line}, col {col}\n  use: {replacement}")]
    DeprecatedSyntax {
        syntax: String,
        replacement: String,
        line: usize,
        col: usize,
    },

    #[error(transparent)]
    Ownership(#[from] OwnershipError),
}
