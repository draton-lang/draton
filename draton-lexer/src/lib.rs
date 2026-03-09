//! Tokenization for the Draton programming language.

pub mod error;
pub mod lexer;
pub mod token;

pub use error::{LexError, LexResult};
pub use lexer::Lexer;
pub use token::{Span, Token, TokenKind};
