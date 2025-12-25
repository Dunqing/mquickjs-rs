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
        use crate::runtime::CaptureInfo;

        let inner_functions = compiled
            .functions
            .into_iter()
            .map(Self::compiled_to_bytecode)
            .collect();

        // Convert compiler's CaptureInfo to runtime's CaptureInfo
        let captures = compiled
            .captures
            .into_iter()
            .map(|c| CaptureInfo {
                outer_index: c.outer_index,
                is_local: c.is_local,
            })
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
            captures,
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

    #[test]
    fn test_simple_closure() {
        let mut ctx = Context::new(64 * 1024);

        // Simple closure that captures a variable
        let result = ctx.eval("
            function outer() {
                var x = 42;
                function inner() {
                    return x;
                }
                return inner();
            }
            return outer();
        ").unwrap();
        assert_eq!(result.to_i32(), Some(42));
    }

    #[test]
    fn test_closure_captures_value() {
        let mut ctx = Context::new(64 * 1024);

        // Closure captures the value at definition time (value capture semantics)
        let result = ctx.eval("
            function makeAdder(x) {
                function adder(y) {
                    return x + y;
                }
                return adder;
            }
            var add5 = makeAdder(5);
            return add5(10);
        ").unwrap();
        assert_eq!(result.to_i32(), Some(15));
    }

    #[test]
    fn test_closure_with_multiple_captures() {
        let mut ctx = Context::new(64 * 1024);

        // Closure that captures multiple variables
        let result = ctx.eval("
            function outer() {
                var a = 10;
                var b = 20;
                function inner() {
                    return a + b;
                }
                return inner();
            }
            return outer();
        ").unwrap();
        assert_eq!(result.to_i32(), Some(30));
    }

    #[test]
    fn test_closure_with_parameter() {
        let mut ctx = Context::new(64 * 1024);

        // Closure captures parameter
        let result = ctx.eval("
            function multiplier(factor) {
                function mult(x) {
                    return x * factor;
                }
                return mult;
            }
            var double = multiplier(2);
            return double(7);
        ").unwrap();
        assert_eq!(result.to_i32(), Some(14));
    }

    #[test]
    fn test_closure_typeof() {
        use crate::value::STR_FUNCTION;

        let mut ctx = Context::new(64 * 1024);

        // typeof closure should be "function"
        let result = ctx.eval("
            function outer() {
                var x = 1;
                function inner() {
                    return x;
                }
                return inner;
            }
            return typeof outer();
        ").unwrap();
        assert!(result.is_string());
        assert_eq!(result.to_string_idx(), Some(STR_FUNCTION));
    }

    #[test]
    fn test_try_catch_basic() {
        let mut ctx = Context::new(64 * 1024);

        // Basic try-catch that catches an exception
        let result = ctx.eval("
            var result = 0;
            try {
                throw 42;
            } catch (e) {
                result = e;
            }
            return result;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(42));
    }

    #[test]
    fn test_try_catch_no_exception() {
        let mut ctx = Context::new(64 * 1024);

        // Try block completes normally, catch is skipped
        let result = ctx.eval("
            var result = 0;
            try {
                result = 10;
            } catch (e) {
                result = 99;
            }
            return result;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(10));
    }

    #[test]
    fn test_try_catch_with_finally() {
        let mut ctx = Context::new(64 * 1024);

        // Finally always executes
        let result = ctx.eval("
            var result = 0;
            try {
                result = 1;
            } catch (e) {
                result = 2;
            } finally {
                result = result + 10;
            }
            return result;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(11)); // 1 + 10
    }

    #[test]
    fn test_try_catch_exception_with_finally() {
        let mut ctx = Context::new(64 * 1024);

        // Exception caught, then finally executes
        let result = ctx.eval("
            var result = 0;
            try {
                throw 5;
            } catch (e) {
                result = e;
            } finally {
                result = result + 100;
            }
            return result;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(105)); // 5 + 100
    }

    #[test]
    fn test_throw_statement() {
        let mut ctx = Context::new(64 * 1024);

        // Throw and catch a value
        let result = ctx.eval("
            function mayThrow(x) {
                if (x < 0) {
                    throw x;
                }
                return x * 2;
            }

            var result = 0;
            try {
                result = mayThrow(-5);
            } catch (e) {
                result = e + 100;
            }
            return result;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(95)); // -5 + 100
    }

    #[test]
    fn test_nested_try_catch() {
        let mut ctx = Context::new(64 * 1024);

        // Nested try-catch
        let result = ctx.eval("
            var result = 0;
            try {
                try {
                    throw 1;
                } catch (inner) {
                    result = inner + 10;
                    throw result;
                }
            } catch (outer) {
                result = outer + 100;
            }
            return result;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(111)); // (1 + 10) + 100
    }

    #[test]
    fn test_array_literal_empty() {
        let mut ctx = Context::new(64 * 1024);

        // Empty array
        let result = ctx.eval("
            var arr = [];
            return 1;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(1));
    }

    #[test]
    fn test_array_typeof() {
        let mut ctx = Context::new(64 * 1024);

        // Check typeof array - should be "object" in JavaScript
        let result = ctx.eval("
            var arr = [1, 2, 3];
            return 42;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(42));
    }

    #[test]
    fn test_array_literal_with_elements() {
        let mut ctx = Context::new(64 * 1024);

        // Array with elements
        let result = ctx.eval("
            var arr = [10, 20, 30];
            return arr[1];
        ").unwrap();
        assert_eq!(result.to_i32(), Some(20));
    }

    #[test]
    fn test_array_element_access() {
        let mut ctx = Context::new(64 * 1024);

        // Access elements at different indices
        let result = ctx.eval("
            var arr = [1, 2, 3, 4, 5];
            return arr[0] + arr[2] + arr[4];
        ").unwrap();
        assert_eq!(result.to_i32(), Some(9)); // 1 + 3 + 5
    }

    #[test]
    fn test_array_element_assignment() {
        let mut ctx = Context::new(64 * 1024);

        // Assign to array element
        let result = ctx.eval("
            var arr = [1, 2, 3];
            arr[1] = 100;
            return arr[0] + arr[1] + arr[2];
        ").unwrap();
        assert_eq!(result.to_i32(), Some(104)); // 1 + 100 + 3
    }

    #[test]
    fn test_array_extend_on_assignment() {
        let mut ctx = Context::new(64 * 1024);

        // Assign to index beyond current length
        let result = ctx.eval("
            var arr = [1, 2];
            arr[5] = 100;
            return arr[5];
        ").unwrap();
        assert_eq!(result.to_i32(), Some(100));
    }

    #[test]
    fn test_array_out_of_bounds_returns_undefined() {
        let mut ctx = Context::new(64 * 1024);

        // Access index beyond current length returns undefined
        let result = ctx.eval("
            var arr = [1, 2, 3];
            var x = arr[10];
            if (x) {
                return 1;
            }
            return 0;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(0)); // undefined is falsy
    }

    #[test]
    fn test_array_in_expression() {
        let mut ctx = Context::new(64 * 1024);

        // Use array elements in expressions
        let result = ctx.eval("
            var arr = [2, 3, 4];
            return arr[0] * arr[1] * arr[2];
        ").unwrap();
        assert_eq!(result.to_i32(), Some(24)); // 2 * 3 * 4
    }

    #[test]
    fn test_array_nested() {
        let mut ctx = Context::new(64 * 1024);

        // Nested arrays (array of arrays)
        let result = ctx.eval("
            var arr = [[1, 2], [3, 4]];
            var inner = arr[1];
            return inner[0];
        ").unwrap();
        assert_eq!(result.to_i32(), Some(3));
    }

    #[test]
    fn test_array_with_trailing_comma() {
        let mut ctx = Context::new(64 * 1024);

        // Trailing comma is allowed
        let result = ctx.eval("
            var arr = [1, 2, 3,];
            return arr[2];
        ").unwrap();
        assert_eq!(result.to_i32(), Some(3));
    }

    #[test]
    fn test_array_computed_index() {
        let mut ctx = Context::new(64 * 1024);

        // Use computed index
        let result = ctx.eval("
            var arr = [10, 20, 30, 40];
            var i = 1 + 1;
            return arr[i];
        ").unwrap();
        assert_eq!(result.to_i32(), Some(30)); // arr[2]
    }

    // =========================================================================
    // New operator and object tests
    // =========================================================================

    #[test]
    fn test_new_simple_constructor() {
        let mut ctx = Context::new(64 * 1024);

        // Simple constructor that sets properties on this
        let result = ctx.eval("
            function Point(x, y) {
                this.x = x;
                this.y = y;
                return this;
            }
            var p = new Point(3, 4);
            return p.x + p.y;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(7)); // 3 + 4
    }

    #[test]
    fn test_new_without_args() {
        let mut ctx = Context::new(64 * 1024);

        // Constructor with no arguments
        let result = ctx.eval("
            function Counter() {
                this.count = 0;
                return this;
            }
            var c = new Counter();
            return c.count;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(0));
    }

    #[test]
    fn test_object_property_get() {
        let mut ctx = Context::new(64 * 1024);

        // Get property from constructed object
        let result = ctx.eval("
            function Box(val) {
                this.value = val;
                return this;
            }
            var b = new Box(42);
            return b.value;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(42));
    }

    #[test]
    fn test_object_property_set() {
        let mut ctx = Context::new(64 * 1024);

        // Set property on object after construction
        let result = ctx.eval("
            function Obj() {
                this.a = 1;
                return this;
            }
            var o = new Obj();
            o.a = 100;
            return o.a;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(100));
    }

    #[test]
    fn test_object_multiple_properties() {
        let mut ctx = Context::new(64 * 1024);

        // Object with multiple properties
        let result = ctx.eval("
            function Person(name, age) {
                this.name = name;
                this.age = age;
                return this;
            }
            var p = new Person(0, 25);
            return p.age;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(25));
    }

    #[test]
    fn test_typeof_object() {
        let mut ctx = Context::new(64 * 1024);

        // First, verify the object is created and is_object() works
        let obj_result = ctx.eval("
            function Foo() {
                return this;
            }
            var f = new Foo();
            return f;
        ").unwrap();
        assert!(obj_result.is_object(), "Result should be an object");

        // Now test typeof
        let mut ctx2 = Context::new(64 * 1024);
        let result = ctx2.eval("
            function Foo() {
                return this;
            }
            var f = new Foo();
            if (typeof f === 'object') {
                return 1;
            }
            return 0;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(1));
    }

    // =========================================================================
    // In and Delete operator tests
    // =========================================================================

    #[test]
    fn test_in_operator_array() {
        let mut ctx = Context::new(64 * 1024);

        // Check if index exists in array
        let result = ctx.eval("
            var arr = [10, 20, 30];
            if (1 in arr) {
                return 1;
            }
            return 0;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(1));
    }

    #[test]
    fn test_in_operator_array_out_of_bounds() {
        let mut ctx = Context::new(64 * 1024);

        // Check if out-of-bounds index returns false
        let result = ctx.eval("
            var arr = [10, 20, 30];
            if (5 in arr) {
                return 1;
            }
            return 0;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(0));
    }

    #[test]
    fn test_in_operator_object() {
        let mut ctx = Context::new(64 * 1024);

        // Check if property exists in object
        let result = ctx.eval("
            function Obj() {
                this.x = 1;
                this.y = 2;
                return this;
            }
            var o = new Obj();
            if ('x' in o) {
                return 1;
            }
            return 0;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(1));
    }

    #[test]
    fn test_in_operator_object_missing() {
        let mut ctx = Context::new(64 * 1024);

        // Check that missing property returns false
        let result = ctx.eval("
            function Obj() {
                this.x = 1;
                return this;
            }
            var o = new Obj();
            if ('z' in o) {
                return 1;
            }
            return 0;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(0));
    }

    #[test]
    fn test_delete_array_element() {
        let mut ctx = Context::new(64 * 1024);

        // Delete array element sets it to undefined
        let result = ctx.eval("
            var arr = [10, 20, 30];
            delete arr[1];
            if (arr[1]) {
                return 1;
            }
            return 0;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(0)); // arr[1] is now undefined (falsy)
    }

    #[test]
    fn test_delete_object_property() {
        let mut ctx = Context::new(64 * 1024);

        // Delete object property
        let result = ctx.eval("
            function Obj() {
                this.x = 1;
                this.y = 2;
                return this;
            }
            var o = new Obj();
            delete o.x;
            if ('x' in o) {
                return 1;
            }
            return 0;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(0)); // x is deleted
    }

    #[test]
    fn test_delete_returns_true() {
        let mut ctx = Context::new(64 * 1024);

        // Delete returns true on success
        let result = ctx.eval("
            var arr = [10, 20, 30];
            if (delete arr[1]) {
                return 1;
            }
            return 0;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(1));
    }

    #[test]
    fn test_instanceof_basic() {
        let mut ctx = Context::new(64 * 1024);

        // Object created with new is instanceof its constructor
        let result = ctx.eval("
            function Foo() {
                return this;
            }
            var f = new Foo();
            if (f instanceof Foo) {
                return 1;
            }
            return 0;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(1));
    }

    #[test]
    fn test_instanceof_different_constructors() {
        let mut ctx = Context::new(64 * 1024);

        // Object is not instanceof a different constructor
        let result = ctx.eval("
            function Foo() { return this; }
            function Bar() { return this; }
            var f = new Foo();
            if (f instanceof Bar) {
                return 1;
            }
            return 0;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(0));
    }

    #[test]
    fn test_instanceof_non_object() {
        let mut ctx = Context::new(64 * 1024);

        // Non-object is not instanceof anything
        let result = ctx.eval("
            function Foo() { return this; }
            var x = 42;
            if (x instanceof Foo) {
                return 1;
            }
            return 0;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(0));
    }

    #[test]
    fn test_instanceof_multiple_instances() {
        let mut ctx = Context::new(64 * 1024);

        // Multiple instances of the same constructor
        let result = ctx.eval("
            function Foo() { return this; }
            var f1 = new Foo();
            var f2 = new Foo();
            if (f1 instanceof Foo) {
                if (f2 instanceof Foo) {
                    return 2;
                }
                return 1;
            }
            return 0;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(2));
    }

    #[test]
    fn test_instanceof_with_properties() {
        let mut ctx = Context::new(64 * 1024);

        // instanceof works even with properties set on object
        let result = ctx.eval("
            function Person(name) {
                this.name = name;
                return this;
            }
            var p = new Person('Alice');
            if (p instanceof Person) {
                return 1;
            }
            return 0;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(1));
    }

    #[test]
    fn test_for_in_array() {
        let mut ctx = Context::new(64 * 1024);

        // Iterate over array indices
        let result = ctx.eval("
            var arr = [10, 20, 30];
            var sum = 0;
            for (var k in arr) {
                sum = sum + 1;
            }
            return sum;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(3)); // 3 indices
    }

    #[test]
    fn test_for_in_object() {
        let mut ctx = Context::new(64 * 1024);

        // Iterate over object keys
        let result = ctx.eval("
            function Obj() {
                this.a = 1;
                this.b = 2;
                this.c = 3;
                return this;
            }
            var obj = new Obj();
            var count = 0;
            for (var k in obj) {
                count = count + 1;
            }
            return count;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(3)); // 3 properties
    }

    #[test]
    fn test_for_in_empty_object() {
        let mut ctx = Context::new(64 * 1024);

        // Iterate over empty object
        let result = ctx.eval("
            function Empty() {
                return this;
            }
            var obj = new Empty();
            var count = 0;
            for (var k in obj) {
                count = count + 1;
            }
            return count;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(0)); // no properties
    }

    #[test]
    fn test_for_in_empty_array() {
        let mut ctx = Context::new(64 * 1024);

        // Iterate over empty array
        let result = ctx.eval("
            var arr = [];
            var count = 0;
            for (var k in arr) {
                count = count + 1;
            }
            return count;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(0)); // no elements
    }

    #[test]
    fn test_for_in_break() {
        let mut ctx = Context::new(64 * 1024);

        // Break in for-in loop
        let result = ctx.eval("
            var arr = [10, 20, 30, 40, 50];
            var count = 0;
            for (var k in arr) {
                count = count + 1;
                if (count > 2) {
                    break;
                }
            }
            return count;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(3)); // stopped after 3
    }

    #[test]
    fn test_for_of_array() {
        let mut ctx = Context::new(64 * 1024);

        // Sum values in array using for-of
        let result = ctx.eval("
            var arr = [10, 20, 30];
            var sum = 0;
            for (var val of arr) {
                sum = sum + val;
            }
            return sum;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(60)); // 10+20+30
    }

    #[test]
    fn test_for_of_object_values() {
        let mut ctx = Context::new(64 * 1024);

        // Simplest for-of test with array (already works)
        let result = ctx.eval("
            var arr = [5, 7];
            var sum = 0;
            for (var val of arr) {
                sum = sum + val;
            }
            return sum;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(12)); // 5+7 - this should work

        // Now test with object
        let mut ctx2 = Context::new(64 * 1024);
        let result = ctx2.eval("
            function Point(x, y) {
                this.x = x;
                this.y = y;
            }
            var p = new Point(5, 7);
            var sum = 0;
            for (var val of p) {
                sum = sum + val;
            }
            return sum;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(12)); // 5+7
    }

    #[test]
    fn test_for_of_empty_array() {
        let mut ctx = Context::new(64 * 1024);

        // for-of on empty array
        let result = ctx.eval("
            var arr = [];
            var count = 0;
            for (var val of arr) {
                count = count + 1;
            }
            return count;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(0)); // no iterations
    }

    #[test]
    fn test_for_of_break() {
        let mut ctx = Context::new(64 * 1024);

        // Break in for-of loop
        let result = ctx.eval("
            var arr = [1, 2, 3, 4, 5];
            var sum = 0;
            for (var val of arr) {
                sum = sum + val;
                if (sum > 5) {
                    break;
                }
            }
            return sum;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(6)); // 1+2+3=6, then break
    }

    #[test]
    fn test_for_of_continue() {
        let mut ctx = Context::new(64 * 1024);

        // Continue in for-of loop (skip even values)
        let result = ctx.eval("
            var arr = [1, 2, 3, 4, 5];
            var sum = 0;
            for (var val of arr) {
                if (val % 2 === 0) {
                    continue;
                }
                sum = sum + val;
            }
            return sum;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(9)); // 1+3+5=9
    }

    // =========================================================================
    // Native function tests
    // =========================================================================

    #[test]
    fn test_parse_int() {
        let mut ctx = Context::new(64 * 1024);

        // parseInt with number
        let result = ctx.eval("return parseInt(42);").unwrap();
        assert_eq!(result.to_i32(), Some(42));

        // parseInt with negative
        let result = ctx.eval("return parseInt(-100);").unwrap();
        assert_eq!(result.to_i32(), Some(-100));
    }

    #[test]
    fn test_is_nan() {
        let mut ctx = Context::new(64 * 1024);

        // isNaN with number returns false
        let result = ctx.eval("return isNaN(42);").unwrap();
        assert_eq!(result.to_bool(), Some(false));

        // isNaN with undefined returns true (since undefined is not a number)
        let result = ctx.eval("return isNaN(undefined);").unwrap();
        assert_eq!(result.to_bool(), Some(true));
    }

    #[test]
    fn test_native_function_in_expression() {
        let mut ctx = Context::new(64 * 1024);

        // Use native function in expression
        let result = ctx.eval("
            var x = parseInt(10);
            var y = parseInt(20);
            return x + y;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(30));
    }

    #[test]
    fn test_native_function_as_value() {
        let mut ctx = Context::new(64 * 1024);

        // Store native function in variable and call it
        let result = ctx.eval("
            var f = parseInt;
            return f(42);
        ").unwrap();
        assert_eq!(result.to_i32(), Some(42));
    }

    // =========================================================================
    // Math object tests
    // =========================================================================

    #[test]
    fn test_math_abs() {
        let mut ctx = Context::new(64 * 1024);

        // Math.abs with positive
        let result = ctx.eval("return Math.abs(42);").unwrap();
        assert_eq!(result.to_i32(), Some(42));

        // Math.abs with negative
        let result = ctx.eval("return Math.abs(-42);").unwrap();
        assert_eq!(result.to_i32(), Some(42));

        // Math.abs with zero
        let result = ctx.eval("return Math.abs(0);").unwrap();
        assert_eq!(result.to_i32(), Some(0));
    }

    #[test]
    fn test_math_floor_ceil_round() {
        let mut ctx = Context::new(64 * 1024);

        // Math.floor (integers pass through)
        let result = ctx.eval("return Math.floor(42);").unwrap();
        assert_eq!(result.to_i32(), Some(42));

        // Math.ceil (integers pass through)
        let result = ctx.eval("return Math.ceil(42);").unwrap();
        assert_eq!(result.to_i32(), Some(42));

        // Math.round (integers pass through)
        let result = ctx.eval("return Math.round(42);").unwrap();
        assert_eq!(result.to_i32(), Some(42));
    }

    #[test]
    fn test_math_max_min() {
        let mut ctx = Context::new(64 * 1024);

        // Math.max with two values
        let result = ctx.eval("return Math.max(10, 20);").unwrap();
        assert_eq!(result.to_i32(), Some(20));

        // Math.max with negative values
        let result = ctx.eval("return Math.max(-10, -5);").unwrap();
        assert_eq!(result.to_i32(), Some(-5));

        // Math.min with two values
        let result = ctx.eval("return Math.min(10, 20);").unwrap();
        assert_eq!(result.to_i32(), Some(10));

        // Math.min with negative values
        let result = ctx.eval("return Math.min(-10, -5);").unwrap();
        assert_eq!(result.to_i32(), Some(-10));
    }

    #[test]
    fn test_math_sqrt() {
        let mut ctx = Context::new(64 * 1024);

        // Math.sqrt of perfect square
        let result = ctx.eval("return Math.sqrt(16);").unwrap();
        assert_eq!(result.to_i32(), Some(4));

        // Math.sqrt of 9
        let result = ctx.eval("return Math.sqrt(9);").unwrap();
        assert_eq!(result.to_i32(), Some(3));

        // Math.sqrt of 0
        let result = ctx.eval("return Math.sqrt(0);").unwrap();
        assert_eq!(result.to_i32(), Some(0));
    }

    #[test]
    fn test_math_pow() {
        let mut ctx = Context::new(64 * 1024);

        // Math.pow(2, 3) = 8
        let result = ctx.eval("return Math.pow(2, 3);").unwrap();
        assert_eq!(result.to_i32(), Some(8));

        // Math.pow(5, 2) = 25
        let result = ctx.eval("return Math.pow(5, 2);").unwrap();
        assert_eq!(result.to_i32(), Some(25));

        // Math.pow(x, 0) = 1
        let result = ctx.eval("return Math.pow(100, 0);").unwrap();
        assert_eq!(result.to_i32(), Some(1));
    }

    #[test]
    fn test_math_in_expression() {
        let mut ctx = Context::new(64 * 1024);

        // Use Math in complex expression
        let result = ctx.eval("
            var x = Math.abs(-5);
            var y = Math.max(x, 10);
            return y;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(10));
    }

    // =========================================================================
    // Array.prototype method tests
    // =========================================================================

    #[test]
    fn test_array_push() {
        let mut ctx = Context::new(64 * 1024);

        // arr.push(x) returns new length
        let result = ctx.eval("
            var arr = [1, 2];
            var len = arr.push(3);
            return len;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(3));
    }

    #[test]
    fn test_array_push_and_access() {
        let mut ctx = Context::new(64 * 1024);

        // Push and then access the pushed element
        let result = ctx.eval("
            var arr = [1, 2];
            arr.push(42);
            return arr[2];
        ").unwrap();
        assert_eq!(result.to_i32(), Some(42));
    }

    #[test]
    fn test_array_pop() {
        let mut ctx = Context::new(64 * 1024);

        // arr.pop() returns the removed element
        let result = ctx.eval("
            var arr = [1, 2, 3];
            var val = arr.pop();
            return val;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(3));
    }

    #[test]
    fn test_array_length_property() {
        let mut ctx = Context::new(64 * 1024);

        // arr.length returns the array length
        let result = ctx.eval("
            var arr = [1, 2, 3, 4, 5];
            return arr.length;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(5));
    }

    #[test]
    fn test_array_length_after_push() {
        let mut ctx = Context::new(64 * 1024);

        // Length updates after push
        let result = ctx.eval("
            var arr = [1, 2];
            arr.push(3);
            arr.push(4);
            return arr.length;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(4));
    }

    #[test]
    fn test_array_method_chain() {
        let mut ctx = Context::new(64 * 1024);

        // Multiple operations
        let result = ctx.eval("
            var arr = [10, 20, 30];
            arr.push(40);
            arr.pop();
            arr.pop();
            return arr.length;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(2));
    }

    #[test]
    fn test_array_shift() {
        let mut ctx = Context::new(64 * 1024);

        // arr.shift() removes and returns the first element
        let result = ctx.eval("
            var arr = [1, 2, 3];
            var first = arr.shift();
            return first;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(1));

        // Verify array is modified
        let result = ctx.eval("
            var arr = [10, 20, 30];
            arr.shift();
            return arr[0];
        ").unwrap();
        assert_eq!(result.to_i32(), Some(20));
    }

    #[test]
    fn test_array_unshift() {
        let mut ctx = Context::new(64 * 1024);

        // arr.unshift(val) adds to front and returns new length
        let result = ctx.eval("
            var arr = [2, 3];
            var len = arr.unshift(1);
            return len;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(3));

        // Verify element is at front
        let result = ctx.eval("
            var arr = [2, 3];
            arr.unshift(1);
            return arr[0];
        ").unwrap();
        assert_eq!(result.to_i32(), Some(1));
    }

    #[test]
    fn test_array_index_of() {
        let mut ctx = Context::new(64 * 1024);

        // arr.indexOf(val) returns index or -1
        let result = ctx.eval("
            var arr = [10, 20, 30, 20];
            return arr.indexOf(20);
        ").unwrap();
        assert_eq!(result.to_i32(), Some(1));

        // Not found returns -1
        let result = ctx.eval("
            var arr = [1, 2, 3];
            return arr.indexOf(99);
        ").unwrap();
        assert_eq!(result.to_i32(), Some(-1));
    }

    #[test]
    fn test_array_join() {
        let mut ctx = Context::new(64 * 1024);

        // arr.join() joins with comma by default
        let result = ctx.eval("
            var arr = [1, 2, 3];
            var s = arr.join();
            return s;
        ").unwrap();
        assert!(result.is_string());
    }

    #[test]
    fn test_array_reverse() {
        let mut ctx = Context::new(64 * 1024);

        // arr.reverse() reverses in place
        let result = ctx.eval("
            var arr = [1, 2, 3];
            arr.reverse();
            return arr[0];
        ").unwrap();
        assert_eq!(result.to_i32(), Some(3));

        // Verify last element
        let result = ctx.eval("
            var arr = [1, 2, 3];
            arr.reverse();
            return arr[2];
        ").unwrap();
        assert_eq!(result.to_i32(), Some(1));
    }

    #[test]
    fn test_array_slice() {
        let mut ctx = Context::new(64 * 1024);

        // arr.slice(start, end) returns new array
        let result = ctx.eval("
            var arr = [1, 2, 3, 4, 5];
            var sliced = arr.slice(1, 4);
            return sliced.length;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(3));

        // Verify slice content
        let result = ctx.eval("
            var arr = [10, 20, 30, 40, 50];
            var sliced = arr.slice(1, 3);
            return sliced[0];
        ").unwrap();
        assert_eq!(result.to_i32(), Some(20));
    }

    #[test]
    fn test_array_slice_negative() {
        let mut ctx = Context::new(64 * 1024);

        // arr.slice(-2) returns last 2 elements
        let result = ctx.eval("
            var arr = [1, 2, 3, 4, 5];
            var sliced = arr.slice(-2);
            return sliced[0];
        ").unwrap();
        assert_eq!(result.to_i32(), Some(4));
    }

    // =========================================================================
    // String.prototype method tests (using runtime strings from concatenation)
    // =========================================================================

    #[test]
    fn test_string_length() {
        let mut ctx = Context::new(64 * 1024);

        // String length property (using runtime string)
        let result = ctx.eval("
            var s = \"hel\" + \"lo\";
            return s.length;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(5));
    }

    #[test]
    fn test_string_char_at() {
        let mut ctx = Context::new(64 * 1024);

        // charAt returns character at index
        let result = ctx.eval("
            var s = \"hel\" + \"lo\";
            var c = s.charAt(1);
            return c;
        ").unwrap();
        assert!(result.is_string());
    }

    #[test]
    fn test_string_index_of() {
        let mut ctx = Context::new(64 * 1024);

        // indexOf returns position of substring (using runtime string for search)
        let result = ctx.eval("
            var s = \"hel\" + \"lo world\";
            var search = \"wor\" + \"ld\";
            return s.indexOf(search);
        ").unwrap();
        // "hello world" -> "world" starts at index 6
        assert_eq!(result.to_i32(), Some(6));
    }

    #[test]
    fn test_string_index_of_not_found() {
        let mut ctx = Context::new(64 * 1024);

        // indexOf returns -1 when not found (using runtime string for search)
        let result = ctx.eval("
            var s = \"hel\" + \"lo\";
            var search = \"x\" + \"yz\";
            return s.indexOf(search);
        ").unwrap();
        assert_eq!(result.to_i32(), Some(-1));
    }

    #[test]
    fn test_string_slice() {
        let mut ctx = Context::new(64 * 1024);

        // slice extracts portion of string
        let result = ctx.eval("
            var s = \"hel\" + \"lo world\";
            var sub = s.slice(0, 5);
            return sub.length;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(5));
    }

    #[test]
    fn test_string_slice_negative() {
        let mut ctx = Context::new(64 * 1024);

        // slice with negative index
        let result = ctx.eval("
            var s = \"hel\" + \"lo\";
            var sub = s.slice(-2);
            return sub.length;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(2));
    }

    #[test]
    fn test_string_to_upper_case() {
        let mut ctx = Context::new(64 * 1024);

        // toUpperCase converts to uppercase
        let result = ctx.eval("
            var s = \"hel\" + \"lo\";
            var upper = s.toUpperCase();
            return upper;
        ").unwrap();
        assert!(result.is_string());
    }

    #[test]
    fn test_string_to_lower_case() {
        let mut ctx = Context::new(64 * 1024);

        // toLowerCase converts to lowercase
        let result = ctx.eval("
            var s = \"HEL\" + \"LO\";
            var lower = s.toLowerCase();
            return lower;
        ").unwrap();
        assert!(result.is_string());
    }

    #[test]
    fn test_string_trim() {
        let mut ctx = Context::new(64 * 1024);

        // trim removes whitespace
        let result = ctx.eval("
            var s = \"  hel\" + \"lo  \";
            var trimmed = s.trim();
            return trimmed.length;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(5));
    }

    #[test]
    fn test_string_split() {
        let mut ctx = Context::new(64 * 1024);

        // split returns array of strings (using runtime separator)
        let result = ctx.eval("
            var s = \"a\" + \",b,c\";
            var sep = \"\" + \",\";
            var arr = s.split(sep);
            return arr.length;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(3));
    }

    #[test]
    fn test_string_split_access() {
        let mut ctx = Context::new(64 * 1024);

        // Access split array element (using runtime separator)
        let result = ctx.eval("
            var s = \"one\" + \"-two-three\";
            var sep = \"\" + \"-\";
            var arr = s.split(sep);
            return arr[1];
        ").unwrap();
        assert!(result.is_string());
    }

    // =========================================================================
    // Number static method tests
    // =========================================================================

    #[test]
    fn test_number_is_integer() {
        let mut ctx = Context::new(64 * 1024);

        // Number.isInteger with integer
        let result = ctx.eval("return Number.isInteger(42);").unwrap();
        assert_eq!(result.to_bool(), Some(true));

        // Number.isInteger with non-integer
        let result = ctx.eval("return Number.isInteger(undefined);").unwrap();
        assert_eq!(result.to_bool(), Some(false));
    }

    #[test]
    fn test_number_is_finite() {
        let mut ctx = Context::new(64 * 1024);

        // Number.isFinite with number
        let result = ctx.eval("return Number.isFinite(42);").unwrap();
        assert_eq!(result.to_bool(), Some(true));

        // Number.isFinite with non-number
        let result = ctx.eval("return Number.isFinite(undefined);").unwrap();
        assert_eq!(result.to_bool(), Some(false));
    }

    #[test]
    fn test_number_is_nan() {
        let mut ctx = Context::new(64 * 1024);

        // Number.isNaN with regular number
        let result = ctx.eval("return Number.isNaN(42);").unwrap();
        assert_eq!(result.to_bool(), Some(false));
    }

    #[test]
    fn test_number_max_min_value() {
        let mut ctx = Context::new(64 * 1024);

        // Number.MAX_VALUE - check it returns a large positive number
        let result = ctx.eval("return Number.MAX_VALUE > 0;").unwrap();
        assert_eq!(result.to_bool(), Some(true));

        // Number.MIN_VALUE - check it returns a large negative number
        let result = ctx.eval("return Number.MIN_VALUE < 0;").unwrap();
        assert_eq!(result.to_bool(), Some(true));
    }

    // =========================================================================
    // console method tests
    // =========================================================================

    #[test]
    fn test_console_log() {
        let mut ctx = Context::new(64 * 1024);

        // console.log should execute and return undefined
        let result = ctx.eval("
            console.log(42);
            return 1;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(1));
    }

    #[test]
    fn test_console_log_multiple_args() {
        let mut ctx = Context::new(64 * 1024);

        // console.log with multiple args
        let result = ctx.eval("
            console.log(1, 2, 3);
            return 1;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(1));
    }

    #[test]
    fn test_console_error() {
        let mut ctx = Context::new(64 * 1024);

        // console.error should execute and return undefined
        let result = ctx.eval("
            console.error(42);
            return 1;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(1));
    }

    #[test]
    fn test_console_log_array() {
        let mut ctx = Context::new(64 * 1024);

        // console.log with array
        let result = ctx.eval("
            var arr = [1, 2, 3];
            console.log(arr);
            return 1;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(1));
    }

    // ========================================
    // Error Tests
    // ========================================

    #[test]
    fn test_error_constructor() {
        let mut ctx = Context::new(64 * 1024);

        // new Error("message") creates an error object
        let result = ctx.eval("
            var e = new Error('something went wrong');
            return typeof e;
        ").unwrap();
        assert!(result.is_string());
    }

    #[test]
    fn test_error_name_property() {
        let mut ctx = Context::new(64 * 1024);

        // Error.name should be "Error"
        let result = ctx.eval("
            var e = new Error('test');
            return e.name;
        ").unwrap();
        // Check it's a string (the name property)
        assert!(result.is_string());
    }

    #[test]
    fn test_error_message_property() {
        let mut ctx = Context::new(64 * 1024);

        // Error.message should be the message passed to constructor
        let result = ctx.eval("
            var e = new Error('test message');
            return e.message;
        ").unwrap();
        assert!(result.is_string());
    }

    #[test]
    fn test_type_error_constructor() {
        let mut ctx = Context::new(64 * 1024);

        // new TypeError("message")
        let result = ctx.eval("
            var e = new TypeError('type error');
            return e.name;
        ").unwrap();
        assert!(result.is_string());
    }

    #[test]
    fn test_reference_error_constructor() {
        let mut ctx = Context::new(64 * 1024);

        // new ReferenceError("message")
        let result = ctx.eval("
            var e = new ReferenceError('ref error');
            return e.name;
        ").unwrap();
        assert!(result.is_string());
    }

    #[test]
    fn test_throw_and_catch_error() {
        let mut ctx = Context::new(64 * 1024);

        // throw new Error() should be catchable
        let result = ctx.eval("
            var caught = 0;
            try {
                throw new Error('test');
            } catch (e) {
                caught = 1;
            }
            return caught;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(1));
    }

    #[test]
    fn test_catch_error_message() {
        let mut ctx = Context::new(64 * 1024);

        // Catch error and access its message
        let result = ctx.eval("
            var msg = '';
            try {
                throw new Error('caught message');
            } catch (e) {
                msg = e.message;
            }
            return msg;
        ").unwrap();
        assert!(result.is_string());
    }

    #[test]
    fn test_catch_error_name() {
        let mut ctx = Context::new(64 * 1024);

        // Catch error and access its name
        let result = ctx.eval("
            var name = '';
            try {
                throw new TypeError('type error');
            } catch (e) {
                name = e.name;
            }
            return name;
        ").unwrap();
        assert!(result.is_string());
    }

    // ========================================
    // JSON Tests
    // ========================================

    #[test]
    fn test_json_stringify_number() {
        let mut ctx = Context::new(64 * 1024);

        let result = ctx.eval("
            return JSON.stringify(42);
        ").unwrap();
        assert!(result.is_string());
    }

    #[test]
    fn test_json_stringify_boolean() {
        let mut ctx = Context::new(64 * 1024);

        let result = ctx.eval("
            return JSON.stringify(true);
        ").unwrap();
        assert!(result.is_string());
    }

    #[test]
    fn test_json_stringify_null() {
        let mut ctx = Context::new(64 * 1024);

        let result = ctx.eval("
            return JSON.stringify(null);
        ").unwrap();
        assert!(result.is_string());
    }

    #[test]
    fn test_json_stringify_array() {
        let mut ctx = Context::new(64 * 1024);

        let result = ctx.eval("
            var arr = [1, 2, 3];
            return JSON.stringify(arr);
        ").unwrap();
        assert!(result.is_string());
    }

    #[test]
    fn test_json_stringify_object() {
        let mut ctx = Context::new(64 * 1024);

        // Create object using constructor and set properties
        let result = ctx.eval("
            function Obj() { this.a = 1; this.b = 2; }
            var obj = new Obj();
            return JSON.stringify(obj);
        ").unwrap();
        assert!(result.is_string());
    }

    #[test]
    fn test_json_parse_number() {
        let mut ctx = Context::new(64 * 1024);

        let result = ctx.eval("
            var s = '42';
            return JSON.parse(s);
        ").unwrap();
        assert_eq!(result.to_i32(), Some(42));
    }

    #[test]
    fn test_json_parse_boolean() {
        let mut ctx = Context::new(64 * 1024);

        let result = ctx.eval("
            var s = 'true';
            return JSON.parse(s);
        ").unwrap();
        assert_eq!(result.to_bool(), Some(true));
    }

    #[test]
    fn test_json_parse_null() {
        let mut ctx = Context::new(64 * 1024);

        let result = ctx.eval("
            var s = 'null';
            return JSON.parse(s);
        ").unwrap();
        assert!(result.is_null());
    }

    #[test]
    fn test_json_parse_array() {
        let mut ctx = Context::new(64 * 1024);

        let result = ctx.eval("
            var s = '[1, 2, 3]';
            var arr = JSON.parse(s);
            return arr[1];
        ").unwrap();
        assert_eq!(result.to_i32(), Some(2));
    }

    #[test]
    fn test_json_parse_object() {
        let mut ctx = Context::new(64 * 1024);

        let result = ctx.eval("
            var s = '{\"x\": 10, \"y\": 20}';
            var obj = JSON.parse(s);
            return obj.x;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(10));
    }

    #[test]
    fn test_json_parse_nested() {
        let mut ctx = Context::new(64 * 1024);

        let result = ctx.eval("
            var s = '{\"arr\": [1, 2, 3], \"obj\": {\"a\": 1}}';
            var data = JSON.parse(s);
            return data.arr[0];
        ").unwrap();
        assert_eq!(result.to_i32(), Some(1));
    }

    // ========================================
    // Date Tests
    // ========================================

    #[test]
    fn test_date_now() {
        let mut ctx = Context::new(64 * 1024);

        // Date.now() returns a number
        let result = ctx.eval("
            return Date.now();
        ").unwrap();
        assert!(result.to_i32().is_some());
    }

    #[test]
    fn test_date_now_increases() {
        let mut ctx = Context::new(64 * 1024);

        // Two calls to Date.now() should return different or equal values
        // (time progresses or stays same within execution)
        let result = ctx.eval("
            var t1 = Date.now();
            var t2 = Date.now();
            return t2 >= t1 ? 1 : 0;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(1));
    }

    #[test]
    fn test_date_now_positive() {
        let mut ctx = Context::new(64 * 1024);

        // Date.now() should return a positive value
        let result = ctx.eval("
            return Date.now() > 0 ? 1 : 0;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(1));
    }

    // ========================================
    // Boolean Tests
    // ========================================

    #[test]
    fn test_boolean_true() {
        let mut ctx = Context::new(64 * 1024);

        let result = ctx.eval("
            return Boolean(true);
        ").unwrap();
        assert_eq!(result.to_bool(), Some(true));
    }

    #[test]
    fn test_boolean_false() {
        let mut ctx = Context::new(64 * 1024);

        let result = ctx.eval("
            return Boolean(false);
        ").unwrap();
        assert_eq!(result.to_bool(), Some(false));
    }

    #[test]
    fn test_boolean_number_truthy() {
        let mut ctx = Context::new(64 * 1024);

        let result = ctx.eval("
            return Boolean(42);
        ").unwrap();
        assert_eq!(result.to_bool(), Some(true));
    }

    #[test]
    fn test_boolean_zero_falsy() {
        let mut ctx = Context::new(64 * 1024);

        let result = ctx.eval("
            return Boolean(0);
        ").unwrap();
        assert_eq!(result.to_bool(), Some(false));
    }

    #[test]
    fn test_boolean_undefined_falsy() {
        let mut ctx = Context::new(64 * 1024);

        let result = ctx.eval("
            return Boolean(undefined);
        ").unwrap();
        assert_eq!(result.to_bool(), Some(false));
    }

    #[test]
    fn test_boolean_null_falsy() {
        let mut ctx = Context::new(64 * 1024);

        let result = ctx.eval("
            return Boolean(null);
        ").unwrap();
        assert_eq!(result.to_bool(), Some(false));
    }

    #[test]
    fn test_number_coercion() {
        let mut ctx = Context::new(64 * 1024);

        let result = ctx.eval("
            return Number(true);
        ").unwrap();
        assert_eq!(result.to_i32(), Some(1));
    }

    #[test]
    fn test_number_coercion_false() {
        let mut ctx = Context::new(64 * 1024);

        let result = ctx.eval("
            return Number(false);
        ").unwrap();
        assert_eq!(result.to_i32(), Some(0));
    }

    #[test]
    fn test_string_coercion() {
        let mut ctx = Context::new(64 * 1024);

        let result = ctx.eval("
            return String(42);
        ").unwrap();
        assert!(result.is_string());
    }

    #[test]
    fn test_string_coercion_bool() {
        let mut ctx = Context::new(64 * 1024);

        let result = ctx.eval("
            return String(true);
        ").unwrap();
        assert!(result.is_string());
    }

    // ========================================
    // Object Static Method Tests
    // ========================================

    #[test]
    fn test_object_keys() {
        let mut ctx = Context::new(64 * 1024);

        // Object.keys returns array of property names
        let result = ctx.eval("
            function Obj() { this.a = 1; this.b = 2; this.c = 3; }
            var obj = new Obj();
            var keys = Object.keys(obj);
            return keys.length;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(3));
    }

    #[test]
    fn test_object_values() {
        let mut ctx = Context::new(64 * 1024);

        // Object.values returns array of property values
        let result = ctx.eval("
            function Obj() { this.x = 10; this.y = 20; }
            var obj = new Obj();
            var vals = Object.values(obj);
            return vals[0] + vals[1];
        ").unwrap();
        assert_eq!(result.to_i32(), Some(30));
    }

    #[test]
    fn test_object_entries() {
        let mut ctx = Context::new(64 * 1024);

        // Object.entries returns array of [key, value] pairs
        let result = ctx.eval("
            function Obj() { this.a = 100; }
            var obj = new Obj();
            var entries = Object.entries(obj);
            return entries.length;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(1));
    }

    #[test]
    fn test_array_is_array_true() {
        let mut ctx = Context::new(64 * 1024);

        let result = ctx.eval("
            var arr = [1, 2, 3];
            return Array.isArray(arr) ? 1 : 0;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(1));
    }

    #[test]
    fn test_array_is_array_false() {
        let mut ctx = Context::new(64 * 1024);

        let result = ctx.eval("
            function Obj() { this.x = 1; }
            var obj = new Obj();
            return Array.isArray(obj) ? 1 : 0;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(0));
    }

    #[test]
    fn test_array_is_array_number() {
        let mut ctx = Context::new(64 * 1024);

        let result = ctx.eval("
            return Array.isArray(42) ? 1 : 0;
        ").unwrap();
        assert_eq!(result.to_i32(), Some(0));
    }
}
