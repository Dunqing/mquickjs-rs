//! JavaScript execution context
//!
//! The Context is the main entry point for the JavaScript engine.
//! It owns all memory and provides the API for evaluating JavaScript code.

use crate::gc::Heap;
use crate::parser::compiler::{CompileError, Compiler};
use crate::runtime::FunctionBytecode;
use crate::value::Value;
use crate::vm::Interpreter;

/// JavaScript execution context
///
/// The Context owns all memory used by the JavaScript engine.
/// Memory layout: [JSContext | Heap (grows up) | ... free ... | Stack (grows down)]
pub struct Context {
    /// The memory heap for GC-managed objects
    heap: Heap,

    /// Bytecode interpreter
    interpreter: Interpreter,

    /// Current exception (if any)
    current_exception: Value,

    /// Whether we're in the process of handling out-of-memory
    in_out_of_memory: bool,
}

/// Error from JavaScript evaluation
#[derive(Debug)]
pub enum EvalError {
    /// Compilation error
    CompileError(CompileError),
    /// Runtime error
    RuntimeError(String),
}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvalError::CompileError(e) => write!(f, "Compile error: {}", e),
            EvalError::RuntimeError(msg) => write!(f, "Runtime error: {}", msg),
        }
    }
}

impl std::error::Error for EvalError {}

impl From<CompileError> for EvalError {
    fn from(e: CompileError) -> Self {
        EvalError::CompileError(e)
    }
}

impl Context {
    /// Create a new JavaScript context with the given memory size
    ///
    /// # Arguments
    /// * `mem_size` - Total memory available for the JS engine in bytes
    ///
    /// # Panics
    /// Panics if mem_size is too small (minimum ~4KB recommended)
    pub fn new(mem_size: usize) -> Self {
        const MIN_MEM_SIZE: usize = 4096;
        assert!(
            mem_size >= MIN_MEM_SIZE,
            "Memory size must be at least {} bytes",
            MIN_MEM_SIZE
        );

        Context {
            heap: Heap::new(mem_size),
            interpreter: Interpreter::new(),
            current_exception: Value::undefined(),
            in_out_of_memory: false,
        }
    }

    /// Evaluate JavaScript source code
    ///
    /// # Arguments
    /// * `source` - JavaScript source code as a string
    ///
    /// # Returns
    /// The result of evaluating the code, or an error
    pub fn eval(&mut self, source: &str) -> Result<Value, EvalError> {
        // Compile the source code
        let compiled = Compiler::new(source).compile()?;

        // Convert to FunctionBytecode for the interpreter
        let bytecode = FunctionBytecode {
            name: None,
            arg_count: 0,
            local_count: compiled.local_count as u16,
            stack_size: 64, // Default stack size
            has_arguments: false,
            bytecode: compiled.bytecode,
            constants: compiled.constants,
            source_file: None,
            line_numbers: Vec::new(),
        };

        // Execute the bytecode
        self.interpreter
            .execute(&bytecode)
            .map_err(|e| EvalError::RuntimeError(e.to_string()))
    }

    /// Compile JavaScript source code without executing
    ///
    /// Returns the compiled bytecode for inspection or later execution.
    pub fn compile(&self, source: &str) -> Result<FunctionBytecode, CompileError> {
        let compiled = Compiler::new(source).compile()?;

        Ok(FunctionBytecode {
            name: None,
            arg_count: 0,
            local_count: compiled.local_count as u16,
            stack_size: 64,
            has_arguments: false,
            bytecode: compiled.bytecode,
            constants: compiled.constants,
            source_file: None,
            line_numbers: Vec::new(),
        })
    }

    /// Execute pre-compiled bytecode
    pub fn execute(&mut self, bytecode: &FunctionBytecode) -> Result<Value, EvalError> {
        self.interpreter
            .execute(bytecode)
            .map_err(|e| EvalError::RuntimeError(e.to_string()))
    }

    /// Run the garbage collector
    pub fn gc(&mut self) {
        self.heap.collect();
    }

    /// Get the current exception (if any)
    pub fn get_exception(&self) -> Value {
        self.current_exception
    }

    /// Clear the current exception
    pub fn clear_exception(&mut self) {
        self.current_exception = Value::undefined();
    }

    /// Get memory usage statistics
    pub fn memory_stats(&self) -> MemoryStats {
        self.heap.stats()
    }
}

/// Memory usage statistics
#[derive(Debug, Clone, Copy)]
pub struct MemoryStats {
    /// Total memory size
    pub total: usize,
    /// Currently used heap memory
    pub heap_used: usize,
    /// Currently used stack memory
    pub stack_used: usize,
    /// Free memory available
    pub free: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_context() {
        let ctx = Context::new(64 * 1024);
        let stats = ctx.memory_stats();
        assert!(stats.total >= 64 * 1024);
    }

    #[test]
    fn test_eval_empty() {
        let mut ctx = Context::new(64 * 1024);
        let result = ctx.eval("").unwrap();
        assert!(result.is_undefined());
    }

    #[test]
    fn test_eval_literal() {
        let mut ctx = Context::new(64 * 1024);

        // Test integer literal
        let result = ctx.eval("42;").unwrap();
        assert!(result.is_undefined()); // Expression statement drops result

        // Test return statement
        let result = ctx.eval("return 42;").unwrap();
        assert_eq!(result.to_i32(), Some(42));
    }

    #[test]
    fn test_eval_arithmetic() {
        let mut ctx = Context::new(64 * 1024);

        let result = ctx.eval("return 2 + 3;").unwrap();
        assert_eq!(result.to_i32(), Some(5));

        let result = ctx.eval("return 10 - 4;").unwrap();
        assert_eq!(result.to_i32(), Some(6));

        let result = ctx.eval("return 3 * 4;").unwrap();
        assert_eq!(result.to_i32(), Some(12));

        let result = ctx.eval("return 20 / 5;").unwrap();
        assert_eq!(result.to_i32(), Some(4));
    }

    #[test]
    fn test_eval_precedence() {
        let mut ctx = Context::new(64 * 1024);

        // 2 + 3 * 4 = 2 + 12 = 14
        let result = ctx.eval("return 2 + 3 * 4;").unwrap();
        assert_eq!(result.to_i32(), Some(14));

        // (2 + 3) * 4 = 5 * 4 = 20
        let result = ctx.eval("return (2 + 3) * 4;").unwrap();
        assert_eq!(result.to_i32(), Some(20));
    }

    #[test]
    fn test_eval_comparison() {
        let mut ctx = Context::new(64 * 1024);

        let result = ctx.eval("return 5 < 10;").unwrap();
        assert_eq!(result.to_bool(), Some(true));

        let result = ctx.eval("return 10 < 5;").unwrap();
        assert_eq!(result.to_bool(), Some(false));

        let result = ctx.eval("return 5 === 5;").unwrap();
        assert_eq!(result.to_bool(), Some(true));
    }

    #[test]
    fn test_eval_variables() {
        let mut ctx = Context::new(64 * 1024);

        let result = ctx.eval("var x = 10; return x;").unwrap();
        assert_eq!(result.to_i32(), Some(10));

        let result = ctx.eval("var x = 5; var y = 3; return x + y;").unwrap();
        assert_eq!(result.to_i32(), Some(8));
    }

    #[test]
    fn test_eval_if_else() {
        let mut ctx = Context::new(64 * 1024);

        let result = ctx.eval("var x = 5; if (x < 10) { return 1; } else { return 2; }").unwrap();
        assert_eq!(result.to_i32(), Some(1));

        let result = ctx.eval("var x = 15; if (x < 10) { return 1; } else { return 2; }").unwrap();
        assert_eq!(result.to_i32(), Some(2));
    }

    #[test]
    fn test_eval_while_loop() {
        let mut ctx = Context::new(64 * 1024);

        // Sum 1 to 5
        let result = ctx.eval("
            var sum = 0;
            var i = 1;
            while (i < 6) {
                sum = sum + i;
                i = i + 1;
            }
            return sum;
        ").unwrap();

        assert_eq!(result.to_i32(), Some(15));
    }

    #[test]
    fn test_eval_assignment() {
        let mut ctx = Context::new(64 * 1024);

        // Simple assignment
        let result = ctx.eval("var x = 5; x = 10; return x;").unwrap();
        assert_eq!(result.to_i32(), Some(10));

        // Assignment returns the assigned value
        let result = ctx.eval("var x = 0; return x = 42;").unwrap();
        assert_eq!(result.to_i32(), Some(42));
    }

    #[test]
    fn test_eval_compound_assignment() {
        let mut ctx = Context::new(64 * 1024);

        let result = ctx.eval("var x = 10; x += 5; return x;").unwrap();
        assert_eq!(result.to_i32(), Some(15));

        let result = ctx.eval("var x = 10; x -= 3; return x;").unwrap();
        assert_eq!(result.to_i32(), Some(7));

        let result = ctx.eval("var x = 4; x *= 3; return x;").unwrap();
        assert_eq!(result.to_i32(), Some(12));

        let result = ctx.eval("var x = 20; x /= 4; return x;").unwrap();
        assert_eq!(result.to_i32(), Some(5));
    }

    #[test]
    fn test_eval_for_loop() {
        let mut ctx = Context::new(64 * 1024);

        // Simple for loop test - just count iterations
        let result = ctx.eval("
            var count = 0;
            for (var i = 0; i < 3; i = i + 1) {
                count = count + 1;
            }
            return count;
        ").unwrap();

        assert_eq!(result.to_i32(), Some(3));
    }

    #[test]
    fn test_eval_for_loop_sum() {
        let mut ctx = Context::new(64 * 1024);

        // Sum 1 to 5 using for loop
        let result = ctx.eval("
            var sum = 0;
            for (var i = 1; i < 6; i = i + 1) {
                sum = sum + i;
            }
            return sum;
        ").unwrap();

        assert_eq!(result.to_i32(), Some(15));
    }

    #[test]
    fn test_eval_ternary() {
        let mut ctx = Context::new(64 * 1024);

        let result = ctx.eval("return 1 ? 100 : 200;").unwrap();
        assert_eq!(result.to_i32(), Some(100));

        let result = ctx.eval("return 0 ? 100 : 200;").unwrap();
        assert_eq!(result.to_i32(), Some(200));
    }

    #[test]
    fn test_eval_boolean_literals() {
        let mut ctx = Context::new(64 * 1024);

        let result = ctx.eval("return true;").unwrap();
        assert_eq!(result.to_bool(), Some(true));

        let result = ctx.eval("return false;").unwrap();
        assert_eq!(result.to_bool(), Some(false));
    }

    #[test]
    fn test_eval_null() {
        let mut ctx = Context::new(64 * 1024);

        let result = ctx.eval("return null;").unwrap();
        assert!(result.is_null());
    }

    #[test]
    fn test_eval_unary() {
        let mut ctx = Context::new(64 * 1024);

        let result = ctx.eval("return -5;").unwrap();
        assert_eq!(result.to_i32(), Some(-5));

        let result = ctx.eval("return !false;").unwrap();
        assert_eq!(result.to_bool(), Some(true));

        let result = ctx.eval("return !true;").unwrap();
        assert_eq!(result.to_bool(), Some(false));
    }

    #[test]
    fn test_compile_error() {
        let mut ctx = Context::new(64 * 1024);

        // Missing semicolon should cause compile error
        let result = ctx.eval("return 1 +");
        assert!(result.is_err());
    }
}
