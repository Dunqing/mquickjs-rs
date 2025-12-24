//! JavaScript compiler
//!
//! Generates bytecode from source code in a single pass.

use super::lexer::{Lexer, Token};
use crate::value::Value;

/// Compiler state
pub struct Compiler<'a> {
    lexer: Lexer<'a>,
    current_token: Token,
    bytecode: Vec<u8>,
    constants: Vec<Value>,
}

impl<'a> Compiler<'a> {
    /// Create a new compiler for the given source
    pub fn new(source: &'a str) -> Self {
        let mut lexer = Lexer::new(source);
        let current_token = lexer.next_token();

        Compiler {
            lexer,
            current_token,
            bytecode: Vec::new(),
            constants: Vec::new(),
        }
    }

    /// Compile the source and return bytecode
    pub fn compile(mut self) -> Result<CompiledFunction, CompileError> {
        // TODO: Implement compilation
        Ok(CompiledFunction {
            bytecode: self.bytecode,
            constants: self.constants,
        })
    }

    /// Advance to the next token
    fn advance(&mut self) {
        self.current_token = self.lexer.next_token();
    }

    /// Check if current token matches expected
    fn check(&self, expected: &Token) -> bool {
        std::mem::discriminant(&self.current_token) == std::mem::discriminant(expected)
    }

    /// Expect a specific token, advance if matched
    fn expect(&mut self, expected: Token) -> Result<(), CompileError> {
        if self.check(&expected) {
            self.advance();
            Ok(())
        } else {
            Err(CompileError::UnexpectedToken {
                expected: format!("{:?}", expected),
                found: format!("{:?}", self.current_token),
            })
        }
    }

    /// Emit a single byte
    fn emit_byte(&mut self, byte: u8) {
        self.bytecode.push(byte);
    }

    /// Emit two bytes
    fn emit_bytes(&mut self, b1: u8, b2: u8) {
        self.bytecode.push(b1);
        self.bytecode.push(b2);
    }

    /// Emit a 16-bit value
    fn emit_u16(&mut self, val: u16) {
        self.bytecode.push((val & 0xff) as u8);
        self.bytecode.push((val >> 8) as u8);
    }

    /// Emit a 32-bit value
    fn emit_u32(&mut self, val: u32) {
        self.bytecode.push((val & 0xff) as u8);
        self.bytecode.push(((val >> 8) & 0xff) as u8);
        self.bytecode.push(((val >> 16) & 0xff) as u8);
        self.bytecode.push((val >> 24) as u8);
    }

    /// Add a constant to the pool and return its index
    fn add_constant(&mut self, value: Value) -> u16 {
        let index = self.constants.len();
        self.constants.push(value);
        index as u16
    }
}

/// Compiled function
pub struct CompiledFunction {
    /// Bytecode bytes
    pub bytecode: Vec<u8>,
    /// Constant pool
    pub constants: Vec<Value>,
}

/// Compilation error
#[derive(Debug)]
pub enum CompileError {
    UnexpectedToken { expected: String, found: String },
    SyntaxError(String),
    TooManyConstants,
    TooManyLocals,
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompileError::UnexpectedToken { expected, found } => {
                write!(f, "Expected {}, found {}", expected, found)
            }
            CompileError::SyntaxError(msg) => write!(f, "Syntax error: {}", msg),
            CompileError::TooManyConstants => write!(f, "Too many constants"),
            CompileError::TooManyLocals => write!(f, "Too many local variables"),
        }
    }
}

impl std::error::Error for CompileError {}
