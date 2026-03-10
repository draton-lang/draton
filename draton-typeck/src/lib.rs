//! Type inference and checking for Draton programs.

pub mod check;
pub mod env;
pub mod error;
pub mod exhaust;
pub mod infer;
pub mod typed_ast;
pub mod unify;

pub use check::{TypeCheckResult, TypeChecker};
pub use env::{Scheme, TypeEnv};
pub use error::TypeError;
pub use infer::Substitution;
pub use typed_ast::{
    Type, TypedBlock, TypedDestructureBinding, TypedExpr, TypedExprKind, TypedFStrPart, TypedFnDef,
    TypedItem, TypedLetDestructureStmt, TypedMatchArm, TypedMatchArmBody, TypedParam, TypedProgram,
    TypedStmt, TypedStmtKind,
};
