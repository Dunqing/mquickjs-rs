//! JavaScript execution context
//!
//! The Context is the main entry point for the JavaScript engine.
//! It owns all memory and provides the API for evaluating JavaScript code.

use crate::gc::Heap;
use crate::value::Value;

/// JavaScript execution context
///
/// The Context owns all memory used by the JavaScript engine.
/// Memory layout: [JSContext | Heap (grows up) | ... free ... | Stack (grows down)]
pub struct Context {
    /// The memory heap for GC-managed objects
    heap: Heap,

    /// Current exception (if any)
    current_exception: Value,

    /// Whether we're in the process of handling out-of-memory
    in_out_of_memory: bool,
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
    /// The result of evaluating the code, or an exception value
    pub fn eval(&mut self, _source: &str) -> Result<Value, Value> {
        // TODO: Implement parser and evaluator
        // For now, return undefined
        Ok(Value::undefined())
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
    fn test_eval_returns_undefined() {
        let mut ctx = Context::new(64 * 1024);
        let result = ctx.eval("").unwrap();
        assert!(result.is_undefined());
    }
}
