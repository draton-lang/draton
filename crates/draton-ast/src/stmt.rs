use crate::expr::Expr;
use crate::item::TypeBlock;
use crate::Span;
use crate::TypeExpr;
use serde::Serialize;

/// A statement node.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Stmt {
    /// A local variable declaration.
    Let(LetStmt),
    /// A tuple destructuring `let`.
    LetDestructure(LetDestructureStmt),
    /// An assignment-like statement.
    Assign(AssignStmt),
    /// A return statement.
    Return(ReturnStmt),
    /// An expression statement.
    Expr(Expr),
    /// An if statement.
    If(IfStmt),
    /// A for loop.
    For(ForStmt),
    /// A while loop.
    While(WhileStmt),
    /// A spawn statement.
    Spawn(SpawnStmt),
    /// A nested block statement.
    Block(Block),
    /// An unsafe block.
    UnsafeBlock(Block),
    /// A pointer block.
    PointerBlock(Block),
    /// An inline assembly block.
    AsmBlock(String, Span),
    /// A compile-time block.
    ComptimeBlock(Block),
    /// A compile-time if statement.
    IfCompile(IfCompileStmt),
    /// A GC configuration block.
    GcConfig(GcConfigStmt),
    /// A local `@type` block.
    TypeBlock(TypeBlock),
}

/// A block of statements.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

/// A `let` statement.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LetStmt {
    pub is_mut: bool,
    pub name: String,
    pub type_hint: Option<TypeExpr>,
    pub value: Option<Expr>,
    pub span: Span,
}

/// A tuple destructuring `let` statement.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LetDestructureStmt {
    pub is_mut: bool,
    pub names: Vec<DestructureBinding>,
    pub value: Expr,
    pub span: Span,
}

/// A single tuple destructuring binding.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum DestructureBinding {
    /// Bind the slot to a local variable.
    Name(String),
    /// Discard the slot.
    Wildcard,
}

/// An assignment statement.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AssignStmt {
    pub target: Expr,
    pub op: AssignOp,
    pub value: Option<Expr>,
    pub span: Span,
}

/// An assignment operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum AssignOp {
    /// `=`
    Assign,
    /// `+=`
    AddAssign,
    /// `-=`
    SubAssign,
    /// `*=`
    MulAssign,
    /// `/=`
    DivAssign,
    /// `%=`
    ModAssign,
    /// `++`
    Inc,
    /// `--`
    Dec,
}

/// A return statement.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReturnStmt {
    pub value: Option<Expr>,
    pub span: Span,
}

/// An if statement.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IfStmt {
    pub condition: Expr,
    pub then_branch: Block,
    pub else_branch: Option<ElseBranch>,
    pub span: Span,
}

/// A trailing else branch.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ElseBranch {
    /// An `else if`.
    If(Box<IfStmt>),
    /// A plain `else`.
    Block(Block),
}

/// A for loop statement.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ForStmt {
    pub name: String,
    pub iter: Expr,
    pub body: Block,
    pub span: Span,
}

/// A while loop statement.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WhileStmt {
    pub condition: Expr,
    pub body: Block,
    pub span: Span,
}

/// A spawn statement.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpawnStmt {
    pub body: SpawnBody,
    pub span: Span,
}

/// The body of a spawn statement.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum SpawnBody {
    /// A spawned expression.
    Expr(Expr),
    /// A spawned block.
    Block(Block),
}

/// A compile-time if statement.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IfCompileStmt {
    pub condition: Expr,
    pub body: Block,
    pub span: Span,
}

/// A GC configuration block.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GcConfigStmt {
    pub entries: Vec<GcConfigEntry>,
    pub span: Span,
}

/// A single GC configuration entry.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GcConfigEntry {
    pub key: String,
    pub value: Expr,
    pub span: Span,
}
