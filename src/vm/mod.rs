//! Virtual machine module
//!
//! The VM executes JavaScript bytecode using a stack-based architecture.

pub mod opcode;
pub mod interpreter;
pub mod stack;

pub use opcode::OpCode;
