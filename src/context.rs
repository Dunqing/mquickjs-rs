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
        let bytecode = Self::compiled_to_bytecode(compiled);

        // Execute the bytecode
        self.interpreter
            .execute(&bytecode)
            .map_err(|e| EvalError::RuntimeError(e.to_string()))
    }

    /// Convert CompiledFunction to FunctionBytecode (recursive for inner functions)
    fn compiled_to_bytecode(compiled: crate::parser::compiler::CompiledFunction) -> FunctionBytecode {
        let inner_functions = compiled
            .functions
            .into_iter()
            .map(Self::compiled_to_bytecode)
            .collect();

        FunctionBytecode {
            name: None,
            arg_count: compiled.arg_count as u16,
            local_count: compiled.local_count as u16,
            stack_size: 64, // Default stack size
            has_arguments: false,
            bytecode: compiled.bytecode,
            constants: compiled.constants,
            string_constants: compiled.string_constants,
            source_file: None,
            line_numbers: Vec::new(),
            inner_functions,
        }
    }

    /// Compile JavaScript source code without executing
    ///
    /// Returns the compiled bytecode for inspection or later execution.
    pub fn compile(&self, source: &str) -> Result<FunctionBytecode, CompileError> {
        let compiled = Compiler::new(source).compile()?;
        Ok(Self::compiled_to_bytecode(compiled))
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

    #[test]
    fn test_function_declaration() {
        let mut ctx = Context::new(64 * 1024);

        // Simple function that returns a constant
        let result = ctx.eval("
            function five() {
                return 5;
            }
            return five();
        ").unwrap();
        assert_eq!(result.to_i32(), Some(5));
    }

    #[test]
    fn test_function_with_args() {
        let mut ctx = Context::new(64 * 1024);

        // Function with arguments
        let result = ctx.eval("
            function add(a, b) {
                return a + b;
            }
            return add(10, 20);
        ").unwrap();
        assert_eq!(result.to_i32(), Some(30));
    }

    #[test]
    fn test_function_with_local() {
        let mut ctx = Context::new(64 * 1024);

        // Function with local variable
        let result = ctx.eval("
            function double(x) {
                var result = x * 2;
                return result;
            }
            return double(7);
        ").unwrap();
        assert_eq!(result.to_i32(), Some(14));
    }

    #[test]
    fn test_recursive_function() {
        let mut ctx = Context::new(64 * 1024);

        // Recursive factorial
        let result = ctx.eval("
            function factorial(n) {
                if (n < 2) {
                    return 1;
                }
                return n * factorial(n - 1);
            }
            return factorial(5);
        ").unwrap();
        assert_eq!(result.to_i32(), Some(120)); // 5! = 120
    }

    #[test]
    fn test_multiple_functions() {
        let mut ctx = Context::new(64 * 1024);

        // Multiple independent functions (cross-function calls require closures - Stage 7)
        let result = ctx.eval("
            function triple(x) {
                return x * 3;
            }
            function negate(x) {
                return 0 - x;
            }
            var a = triple(5);
            var b = negate(7);
            return a + b;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(8)); // 15 + (-7) = 8
    }

    #[test]
    fn test_nested_function_calls() {
        let mut ctx = Context::new(64 * 1024);

        // Test that we can call the same function multiple times
        let result = ctx.eval("
            function add(a, b) {
                return a + b;
            }
            return add(add(1, 2), add(3, 4));
        ").unwrap();
        assert_eq!(result.to_i32(), Some(10)); // (1+2) + (3+4) = 10
    }

    #[test]
    fn test_break_in_while() {
        let mut ctx = Context::new(64 * 1024);

        // Break out of while loop
        let result = ctx.eval("
            var i = 0;
            while (i < 100) {
                if (i === 5) {
                    break;
                }
                i = i + 1;
            }
            return i;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(5));
    }

    #[test]
    fn test_break_in_for() {
        let mut ctx = Context::new(64 * 1024);

        // Break out of for loop
        let result = ctx.eval("
            var sum = 0;
            for (var i = 0; i < 100; i = i + 1) {
                if (i === 5) {
                    break;
                }
                sum = sum + i;
            }
            return sum;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(10)); // 0 + 1 + 2 + 3 + 4 = 10
    }

    #[test]
    fn test_continue_in_while() {
        let mut ctx = Context::new(64 * 1024);

        // Skip even numbers
        let result = ctx.eval("
            var sum = 0;
            var i = 0;
            while (i < 10) {
                i = i + 1;
                if (i % 2 === 0) {
                    continue;
                }
                sum = sum + i;
            }
            return sum;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(25)); // 1 + 3 + 5 + 7 + 9 = 25
    }

    #[test]
    fn test_continue_in_for() {
        let mut ctx = Context::new(64 * 1024);

        // Skip multiples of 3
        let result = ctx.eval("
            var sum = 0;
            for (var i = 1; i < 10; i = i + 1) {
                if (i % 3 === 0) {
                    continue;
                }
                sum = sum + i;
            }
            return sum;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(27)); // 1+2+4+5+7+8 = 27
    }

    #[test]
    fn test_typeof_operator() {
        use crate::value::{STR_UNDEFINED, STR_OBJECT, STR_BOOLEAN, STR_NUMBER, STR_FUNCTION};

        let mut ctx = Context::new(64 * 1024);

        // typeof now returns string values
        // typeof undefined
        let result = ctx.eval("var x; return typeof x;").unwrap();
        assert!(result.is_string());
        assert_eq!(result.to_string_idx(), Some(STR_UNDEFINED));

        // typeof null
        let result = ctx.eval("return typeof null;").unwrap();
        assert!(result.is_string());
        assert_eq!(result.to_string_idx(), Some(STR_OBJECT)); // JS quirk

        // typeof boolean
        let result = ctx.eval("return typeof true;").unwrap();
        assert!(result.is_string());
        assert_eq!(result.to_string_idx(), Some(STR_BOOLEAN));

        // typeof number
        let result = ctx.eval("return typeof 42;").unwrap();
        assert!(result.is_string());
        assert_eq!(result.to_string_idx(), Some(STR_NUMBER));

        // typeof function
        let result = ctx.eval("function f() {} return typeof f;").unwrap();
        assert!(result.is_string());
        assert_eq!(result.to_string_idx(), Some(STR_FUNCTION));
    }

    #[test]
    fn test_string_literal() {
        use crate::value::STR_STRING;

        let mut ctx = Context::new(64 * 1024);

        // typeof string
        let result = ctx.eval("return typeof \"hello\";").unwrap();
        assert!(result.is_string());
        assert_eq!(result.to_string_idx(), Some(STR_STRING));
    }

    #[test]
    fn test_empty_string() {
        use crate::value::STR_STRING;

        let mut ctx = Context::new(64 * 1024);

        // Empty string
        let result = ctx.eval("return typeof \"\";").unwrap();
        assert!(result.is_string());
        assert_eq!(result.to_string_idx(), Some(STR_STRING));
    }

    #[test]
    fn test_string_variable() {
        use crate::value::STR_STRING;

        let mut ctx = Context::new(64 * 1024);

        // Store string in variable and check type
        let result = ctx.eval("var s = \"world\"; return typeof s;").unwrap();
        assert!(result.is_string());
        assert_eq!(result.to_string_idx(), Some(STR_STRING));
    }

    #[test]
    fn test_string_self_equality() {
        let mut ctx = Context::new(64 * 1024);

        // String variable equals itself
        let result = ctx.eval("var s = \"test\"; return s === s;").unwrap();
        assert_eq!(result.to_bool(), Some(true));
    }

    #[test]
    fn test_empty_string_equality() {
        let mut ctx = Context::new(64 * 1024);

        // Two empty strings are equal (both map to same sentinel index)
        let result = ctx.eval("return \"\" === \"\";").unwrap();
        assert_eq!(result.to_bool(), Some(true));
    }

    #[test]
    fn test_string_concat() {
        let mut ctx = Context::new(64 * 1024);

        // Basic string concatenation
        let result = ctx.eval("return \"hello\" + \" world\";").unwrap();
        assert!(result.is_string());

        // Concat with number
        let result = ctx.eval("return \"value: \" + 42;").unwrap();
        assert!(result.is_string());

        // Number + string
        let result = ctx.eval("return 123 + \"abc\";").unwrap();
        assert!(result.is_string());
    }

    #[test]
    fn test_string_concat_in_variable() {
        let mut ctx = Context::new(64 * 1024);

        // Store concatenated string and check type
        let result = ctx.eval("var s = \"a\" + \"b\"; return typeof s;").unwrap();
        assert!(result.is_string());
        assert_eq!(result.to_string_idx(), Some(crate::value::STR_STRING));
    }

    #[test]
    fn test_string_concat_chain() {
        let mut ctx = Context::new(64 * 1024);

        // Multiple concatenations
        let result = ctx.eval("return \"a\" + \"b\" + \"c\";").unwrap();
        assert!(result.is_string());
    }

    #[test]
    fn test_string_concat_with_bool() {
        let mut ctx = Context::new(64 * 1024);

        // String + boolean
        let result = ctx.eval("return \"value: \" + true;").unwrap();
        assert!(result.is_string());
    }

    #[test]
    fn test_string_concat_with_null() {
        let mut ctx = Context::new(64 * 1024);

        // String + null
        let result = ctx.eval("return \"value: \" + null;").unwrap();
        assert!(result.is_string());
    }

    #[test]
    fn test_print_statement() {
        let mut ctx = Context::new(64 * 1024);

        // Print should execute without error and return undefined
        let result = ctx.eval("print 42; return 1;").unwrap();
        assert_eq!(result.to_i32(), Some(1));
    }

    #[test]
    fn test_print_string() {
        let mut ctx = Context::new(64 * 1024);

        // Print a string
        let result = ctx.eval("print \"hello world\"; return 1;").unwrap();
        assert_eq!(result.to_i32(), Some(1));
    }

    #[test]
    fn test_print_expression() {
        let mut ctx = Context::new(64 * 1024);

        // Print result of expression
        let result = ctx.eval("print 2 + 3; return 1;").unwrap();
        assert_eq!(result.to_i32(), Some(1));
    }
}
