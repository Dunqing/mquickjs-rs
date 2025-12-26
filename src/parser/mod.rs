//! JavaScript parser and compiler
//!
//! Single-pass parser that generates bytecode directly.

pub mod compiler;
pub mod lexer;

// Re-exports
pub use compiler::Compiler;
pub use lexer::{Lexer, Token};
