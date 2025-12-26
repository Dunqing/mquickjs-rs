# Benchmark Analysis: Rust vs C Implementation

This document explains the performance differences between MQuickJS-RS (Rust) and the original MQuickJS (C).

## Benchmark Results

**Machine**: Apple M4 Max, 64 GB RAM, macOS

| Benchmark | Rust (s) | C (s) | Ratio | Winner |
|-----------|----------|-------|-------|--------|
| fib | 0.019 | 0.058 | 0.32x | Rust 3.1x faster |
| loop | 0.019 | 0.036 | 0.53x | Rust 1.9x faster |
| json | 0.021 | 0.024 | 0.89x | Rust 12% faster |
| object | 0.017 | 0.017 | 1.03x | Equal |
| string | 0.016 | 0.016 | 1.04x | Equal |
| closure | 0.017 | 0.016 | 1.08x | Equal |
| array | 0.020 | 0.016 | 1.26x | C 26% faster |
| sieve | 0.038 | 0.021 | 1.82x | C 82% faster |

## Why Rust is Faster on `fib` (3.1x)

### The Key Difference: Recursion Handling

**C Implementation** (mquickjs.c, line 68):
```c
#define JS_MAX_CALL_RECURSE 8
```

The original MQuickJS limits C stack recursion to **only 8 levels**. When this limit is reached, it throws "C stack overflow". For `fib(30)` which requires thousands of recursive calls, the C implementation must use a complex trampoline/continuation mechanism to avoid actual stack overflow.

**Rust Implementation**:
```rust
call_stack: Vec<CallFrame>,  // Heap-allocated frame stack
```

The Rust version uses a **heap-allocated call stack** (a `Vec<CallFrame>`). Each function call just pushes a lightweight `CallFrame` struct to the vector - no actual stack recursion. This is a "stackless" interpreter design.

### Comparison

| Aspect | C (mquickjs) | Rust (mquickjs-rs) |
|--------|--------------|-------------------|
| Recursion limit | 8 C stack frames | Limited only by heap memory |
| Deep recursion | Must save/restore interpreter state | Just push/pop Vec element |
| Function call overhead | Complex continuation handling | Simple `Vec::push()` |
| Memory locality | State scattered across C stack | Compact CallFrame structs |

The stackless design in Rust means:
- No C/Rust stack growth during JS recursion
- Constant-time function call overhead
- Better cache locality for call frames

## Why Rust is Faster on `loop` (1.9x)

### 1. LLVM Optimizations

Rust's release build with LTO (`lto = true` in Cargo.toml) aggressively optimizes the interpreter loop. The `opt-level = 3` setting enables maximum optimization.

### 2. Simpler Integer Fast Path

**Rust** - Clean pattern matching:
```rust
fn op_add(&self, a: Value, b: Value) -> InterpreterResult<Value> {
    match (a.to_i32(), b.to_i32()) {
        (Some(va), Some(vb)) => {
            if let Some(result) = va.checked_add(vb) {
                Ok(Value::int(result))
            } else {
                Err(InterpreterError::InternalError("integer overflow".into()))
            }
        }
        _ => Err(InterpreterError::TypeError("cannot add non-numbers".into())),
    }
}
```

**C** - Must check both integer AND short float paths:
```c
CASE(OP_add):
    if (likely(JS_VALUE_IS_BOTH_INT(op1, op2))) {
        int r;
        if (unlikely(__builtin_add_overflow((int)op1, (int)op2, &r)))
            goto add_slow;
        sp[1] = (uint32_t)r;
    } else if (JS_VALUE_IS_BOTH_SHORT_FLOAT(op1, op2)) {
        // Short float path...
    } else {
        goto add_slow;
    }
```

The C version has additional branching for short float support that the Rust version doesn't need.

### 3. Branch Prediction

Rust's `match` on opcode compiles to efficient jump tables. The LLVM backend can better optimize the dispatch loop compared to the C switch statement with computed gotos.

## Why C is Faster on `sieve` (1.8x) and `array` (1.3x)

### Bounds Checking Overhead

Rust's safety guarantees require bounds checking on every array access:

```rust
// Every array[i] access in Rust checks: i < array.len()
let value = array.get(i)?;  // Returns Option, must handle None
```

The C implementation uses unchecked pointer arithmetic:

```c
// Direct memory access, no bounds check
val = arr->values[i];
```

For the sieve benchmark which performs ~100,000+ array accesses, this overhead accumulates significantly.

### Memory Allocation Patterns

The C implementation uses a custom arena allocator optimized for the specific allocation patterns of a JS engine. The Rust version uses standard `Vec` which, while efficient, isn't as specialized.

## Summary

| Category | Winner | Reason |
|----------|--------|--------|
| **Recursion-heavy** (fib) | Rust | Stackless interpreter design vs 8-frame limit |
| **Loop-heavy** (loop) | Rust | LLVM optimizations, simpler integer path |
| **JSON parsing** | Rust | String handling optimizations |
| **Object/String/Closure** | Tie | Similar implementation strategies |
| **Array-heavy** (array, sieve) | C | No bounds checking, custom allocator |

## Potential Optimizations for Rust

1. **Unsafe array access**: Use `get_unchecked()` in hot paths after validating bounds once
2. **Custom allocator**: Implement arena allocation similar to C version
3. **Short float support**: Add inline float optimization for numeric benchmarks
4. **Profile-guided optimization**: Use PGO to optimize the interpreter dispatch loop
