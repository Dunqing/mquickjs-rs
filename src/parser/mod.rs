//! JavaScript parser and compiler
//!
//! Single-pass parser that generates bytecode directly.

pub mod lexer;
pub mod compiler;

// Re-exports
pub use lexer::{Lexer, Token};
pub use compiler::Compiler;
