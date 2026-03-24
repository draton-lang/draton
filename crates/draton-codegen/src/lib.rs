//! LLVM IR code generation for Draton.

pub mod builtins;
pub mod closure;
pub mod codegen;
pub mod error;
pub mod expr;
pub mod gc;
pub mod item;
pub mod mangle;
pub mod mono;
pub mod stmt;
pub mod types;
pub mod vtable;

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
mod windows_target_stubs;

pub use codegen::{BuildMode, CodeGen};
pub use error::CodeGenError;
