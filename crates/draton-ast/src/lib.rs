//! Abstract syntax tree for the Draton programming language.

pub mod expr;
pub mod item;
pub mod stmt;
pub mod types;

pub use expr::{BinOp, Expr, FStrPart, MatchArm, MatchArmBody, UnOp};
pub use item::{
    ClassDef, ClassMember, ConstDef, EnumDef, ErrorDef, ExternBlock, FieldDef, FnDef, ImportDef,
    ImportItem, InterfaceDef, Item, LayerDef, Param, Program, TypeBlock, TypeMember,
};
pub use stmt::{
    AssignOp, AssignStmt, Block, DestructureBinding, ElseBranch, ForStmt, GcConfigEntry,
    GcConfigStmt, IfCompileStmt, IfStmt, LetDestructureStmt, LetStmt, ReturnStmt, SpawnBody,
    SpawnStmt, Stmt, WhileStmt,
};
pub use types::TypeExpr;

/// A byte span pointing into a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub col: usize,
}
