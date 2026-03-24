//! Type inference and checking for Draton programs.

pub mod check;
pub mod env;
pub mod error;
pub mod exhaust;
pub mod infer;
pub mod ownership;
pub mod typed_ast;
pub mod unify;

pub use check::{DeprecatedSyntaxMode, TypeCheckResult, TypeChecker};
pub use env::{Scheme, TypeEnv};
pub use error::{OwnershipError, TypeError};
pub use infer::Substitution;
pub use ownership::OwnershipChecker;
pub use typed_ast::{
    FnOwnershipSummary, OwnershipState, ParamOwnershipSummary, Type, TypedBlock,
    TypedDestructureBinding, TypedExpr, TypedExprKind, TypedFStrPart, TypedFnDef, TypedItem,
    TypedLetDestructureStmt, TypedMatchArm, TypedMatchArmBody, TypedParam, TypedProgram, TypedStmt,
    TypedStmtKind, UseEffect,
};
