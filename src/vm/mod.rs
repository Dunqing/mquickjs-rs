//! Virtual machine module
//!
//! The VM executes JavaScript bytecode using a stack-based architecture.

pub mod interpreter;
pub mod opcode;
pub mod stack;

pub use interpreter::{
    CallFrame, Interpreter, InterpreterError, InterpreterResult, InterpreterStats,
};
pub use opcode::OpCode;
pub use stack::Stack;
