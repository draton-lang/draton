//! LLVM IR code generation for Draton.

pub mod builtins;
pub mod codegen;
pub mod error;
pub mod expr;
pub mod gc;
pub mod item;
pub mod stmt;
pub mod types;

pub use codegen::{BuildMode, CodeGen};
pub use error::CodeGenError;
