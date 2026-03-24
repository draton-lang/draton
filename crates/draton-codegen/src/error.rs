use std::io;

/// Errors raised while lowering typed Draton AST into LLVM IR.
#[derive(Debug, thiserror::Error)]
pub enum CodeGenError {
    /// The typed AST contains a type that this backend does not lower yet.
    #[error("unsupported type in codegen: {0}")]
    UnsupportedType(String),

    /// The typed AST contains an expression that this backend does not lower yet.
    #[error("unsupported expression in codegen: {0}")]
    UnsupportedExpr(String),

    /// The typed AST contains a statement that this backend does not lower yet.
    #[error("unsupported statement in codegen: {0}")]
    UnsupportedStmt(String),

    /// A referenced symbol was not declared in the module or local environment.
    #[error("missing symbol during codegen: {0}")]
    MissingSymbol(String),

    /// LLVM rejected the generated IR.
    #[error("llvm verification failed: {0}")]
    Verify(String),

    /// LLVM API returned an error while building IR.
    #[error("llvm error: {0}")]
    Llvm(String),

    /// Writing IR or object output failed.
    #[error(transparent)]
    Io(#[from] io::Error),
}
