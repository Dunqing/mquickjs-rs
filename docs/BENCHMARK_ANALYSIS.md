# Benchmark Analysis: Rust vs C Implementation

This document explains the performance differences between MQuickJS-RS (Rust) and the original MQuickJS (C).

## Benchmark Results

**Machine**: Apple M4 Max, 64 GB RAM, macOS

| Benchmark | Rust (s) | C (s) | Ratio | Winner |
|-----------|----------|-------|-------|--------|
| json | 0.021 | 0.024 | 0.88x | Rust 12% faster |
| string | 0.016 | 0.016 | 1.01x | Equal |
| closure | 0.016 | 0.016 | 1.02x | Equal |
| object | 0.019 | 0.017 | 1.12x | C 12% faster |
| array | 0.019 | 0.016 | 1.21x | C 21% faster |
| sieve | 0.039 | 0.021 | 1.84x | C 84% faster |
| fib | 0.132 | 0.059 | 2.25x | C 2.25x faster |
| loop | 0.070 | 0.030 | 2.33x | C 2.33x faster |

## Why C is Generally Faster

### 1. Computed Gotos vs Match Statement

**C Implementation** uses computed gotos (GCC extension):
```c
#define CASE(op)  op_label:
#define NEXT      goto *dispatch_table[*pc++]

static void *dispatch_table[] = {
    &&op_push_i32, &&op_push_const, ...
};

// Direct jump to next opcode
CASE(OP_add):
    // ... add code
    NEXT;
```

**Rust Implementation** uses match statement:
```rust
loop {
    let opcode = bc[frame.pc];
    frame.pc += 1;

    match opcode {
        op if op == OpCode::Push0 as u8 => { ... }
        op if op == OpCode::Add as u8 => { ... }
        // 80+ more arms
    }
}
```

**Impact**: Computed gotos eliminate the dispatch overhead of a central switch/match. Each opcode handler jumps directly to the next handler without returning to the dispatch loop. This saves ~2-3 instructions per opcode.

### 2. Inline Caching and Short-Circuit Paths

The C version uses aggressive inline caching for property lookups:
```c
// C: Fast path with cached shape
if (likely(prop_cache->shape == obj->shape)) {
    return obj->props[prop_cache->slot];
}
// Slow path
```

The Rust version does full property lookup each time:
```rust
// Rust: Full lookup every time
self.objects[obj_idx].properties.get(&key)
```

### 3. Tagged Value Representation

Both use tagged values, but the C version has more optimized tagging:

**C** - 32-bit values with NaN boxing or pointer tagging:
```c
// Short int: fits in 31 bits, no allocation
// Short float: fits in IEEE-754 quiet NaN payload
typedef uint32_t JSValue;
```

**Rust** - 64-bit values with simpler tagging:
```rust
// All values are 64 bits, simpler but more memory
struct RawValue(usize);  // 64-bit on modern systems
```

The C version's compact representation improves cache efficiency.

## Why Rust is Faster on `json` (12%)

### Efficient String Handling

Rust's `serde_json` (conceptually similar approach in our parser) handles JSON parsing efficiently:

```rust
// Rust: Zero-copy string parsing where possible
let s: &str = ...;  // Borrowed slice, no allocation

// Efficient string building
let mut s = String::with_capacity(estimated_len);
```

The C version must manage string memory manually, potentially with more allocations.

## Why C is Much Faster on `loop` (2.3x) and `fib` (2.25x)

### Loop Benchmark

The `loop` benchmark runs 1 million iterations of simple arithmetic:
```javascript
for (var i = 0; i < 1000000; i = i + 1) {
    sum = (sum + i) % mod;
}
```

**Why C is faster:**
1. **Tighter dispatch loop**: Computed gotos eliminate match overhead
2. **Better branch prediction**: Direct jumps have predictable patterns
3. **Smaller code**: C opcode handlers are more compact, better I-cache usage

### Fib Benchmark

The `fib` benchmark does recursive function calls:
```javascript
function fib(n) {
    if (n <= 1) return n;
    return fib(n-1) + fib(n-2);
}
fib(30);
```

**Why C is faster:**
1. **Optimized call/return**: C version has hand-tuned function call code paths
2. **Smaller call frame**: C uses compact 8-byte call frames
3. **Register allocation**: C compiler can keep more values in registers

**Note**: The Rust version uses a stackless interpreter design (heap-allocated call frames), which handles deep recursion without stack overflow. This trades some performance for correctness on deeply nested calls.

## Why C is Faster on `sieve` (1.84x) and `array` (1.21x)

### Array Access Patterns

Both benchmarks are array-intensive:

**C** - Direct pointer arithmetic:
```c
// No bounds check, direct memory access
val = arr->values[i];
arr->values[i] = val;
```

**Rust** - Safe access with bounds checking:
```rust
// Bounds check on every access
let val = self.arrays[arr_idx].get(i)?;
self.arrays[arr_idx].set(i, val)?;
```

Even with `unsafe` optimizations in hot paths, Rust still has more indirection:
```rust
// Rust optimized path still involves more steps:
// 1. Get array reference from interpreter
// 2. Check array type
// 3. Access underlying Vec
// 4. Get/set element
```

### Method Call Overhead

Each `array.push()` in JavaScript requires:
1. Property lookup for "push"
2. Function call setup
3. Native function dispatch

The C version optimizes common array methods with special opcodes, while Rust uses generic property lookup.

## Summary

| Category | Winner | Reason |
|----------|--------|--------|
| **JSON parsing** | Rust | Efficient string handling |
| **String/Closure operations** | Tie | Similar implementation strategies |
| **Object access** | C | Inline caching, smaller objects |
| **Array operations** | C | Direct pointer arithmetic, no bounds checks |
| **Loops** | C | Computed gotos, tighter dispatch |
| **Recursion** | C | Optimized call/return paths |

## Design Trade-offs

The Rust implementation prioritizes:
- **Safety**: Bounds checking, no undefined behavior
- **Correctness**: Handles edge cases (deep recursion, large values)
- **Maintainability**: Clear, idiomatic Rust code
- **Learning**: Well-documented for educational purposes

The C implementation prioritizes:
- **Performance**: Every cycle counts in embedded systems
- **Memory efficiency**: Minimal footprint for constrained devices
- **Compatibility**: Proven on many platforms

## Potential Further Optimizations

1. **Computed goto equivalent**: Use `#[cold]` and profile-guided optimization
2. **Inline caching**: Add shape-based property caching
3. **Register-based VM**: Convert from stack-based to register-based bytecode
4. **Unsafe hot paths**: More aggressive use of unsafe in the interpreter loop
5. **Profile-guided optimization**: Use PGO to optimize dispatch patterns
