# MQuickJS-RS

A Rust port of [MQuickJS](https://bellard.org/quickjs/) - Fabrice Bellard's minimalist JavaScript engine designed for embedded systems.

## Features

- **Minimal footprint**: Can run JavaScript with as low as 10KB of RAM
- **ES5 subset**: "Stricter mode" JavaScript with essential features
- **Tracing GC**: Mark-compact garbage collector (no reference counting)
- **Stack-based VM**: Efficient bytecode interpreter
- **UTF-8 strings**: Memory-efficient string storage
- **No unsafe code**: Pure safe Rust implementation

## Installation

```bash
# Clone the repository
git clone https://github.com/user/mquickjs-rs.git
cd mquickjs-rs

# Build
cargo build --release

# Run tests
cargo test
```

## Usage

### Command Line

```bash
# Run a JavaScript file
mqjs script.js

# Evaluate an expression
mqjs -e "1 + 2 * 3"

# Interactive REPL
mqjs

# Compile to bytecode
mqjs -c script.js    # Creates script.qbc

# Run bytecode
mqjs script.qbc

# Show memory usage
mqjs -d script.js

# Set memory limit
mqjs --memory-limit 512k script.js
```

### CLI Options

```
-h, --help         Show help
-e, --eval EXPR    Evaluate expression
-i, --interactive  Enter REPL after running script
-I, --include FILE Include file before main script
-d, --dump         Dump memory usage stats
-c, --compile      Compile to bytecode (.qbc file)
--memory-limit N   Limit memory (supports k/K, m/M suffixes)
```

### Library API

```rust
use mquickjs::{Context, Value};

fn main() {
    // Create context with 64KB memory
    let mut ctx = Context::new(64 * 1024);

    // Evaluate JavaScript
    let result = ctx.eval("1 + 2").unwrap();
    assert_eq!(result.to_i32(), Some(3));

    // Run more complex code
    let result = ctx.eval(r#"
        function factorial(n) {
            if (n <= 1) return 1;
            return n * factorial(n - 1);
        }
        factorial(5)
    "#).unwrap();
    assert_eq!(result.to_i32(), Some(120));
}
```

## Supported Features

### Language Features

- Variables: `var`, `let`, `const`
- Functions: declarations, expressions, closures, arrow functions
- Control flow: `if/else`, `while`, `for`, `for-in`, `for-of`
- Operators: arithmetic, comparison, logical, bitwise, ternary
- Exception handling: `try/catch/finally`, `throw`
- Object literals and property access
- Array literals and operations
- `new` operator for object construction
- `typeof`, `instanceof`, `in`, `delete` operators

### Built-in Objects

| Object | Methods/Properties |
|--------|-------------------|
| **Object** | `keys`, `values`, `entries`, `create`, `defineProperty`, `getPrototypeOf`, `setPrototypeOf`, `hasOwnProperty`, `toString` |
| **Array** | `push`, `pop`, `shift`, `unshift`, `slice`, `splice`, `indexOf`, `lastIndexOf`, `join`, `reverse`, `concat`, `map`, `filter`, `forEach`, `reduce`, `reduceRight`, `find`, `findIndex`, `some`, `every`, `includes`, `sort`, `flat`, `fill`, `isArray`, `toString` |
| **String** | `length`, `charAt`, `charCodeAt`, `codePointAt`, `indexOf`, `lastIndexOf`, `slice`, `substring`, `toUpperCase`, `toLowerCase`, `trim`, `trimStart`, `trimEnd`, `split`, `concat`, `repeat`, `startsWith`, `endsWith`, `includes`, `padStart`, `padEnd`, `replace`, `replaceAll`, `match`, `search`, `fromCharCode`, `fromCodePoint` |
| **Number** | `isInteger`, `isNaN`, `isFinite`, `parseInt`, `MAX_VALUE`, `MIN_VALUE`, `MAX_SAFE_INTEGER`, `MIN_SAFE_INTEGER`, `toString`, `toFixed`, `toExponential`, `toPrecision` |
| **Math** | `abs`, `floor`, `ceil`, `round`, `sqrt`, `pow`, `max`, `min`, `sign`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `exp`, `log`, `log2`, `log10`, `random`, `imul`, `clz32`, `fround`, `trunc`, `PI`, `E`, `LN2`, `LN10`, `LOG2E`, `LOG10E`, `SQRT2`, `SQRT1_2` |
| **JSON** | `parse`, `stringify` |
| **RegExp** | `test`, `exec`, `source`, `flags`, `lastIndex` |
| **Error** | `Error`, `TypeError`, `ReferenceError`, `SyntaxError`, `RangeError`, `EvalError`, `URIError`, `InternalError` (with `name`, `message`, `stack`, `toString`) |
| **TypedArray** | `Int8Array`, `Uint8Array`, `Uint8ClampedArray`, `Int16Array`, `Uint16Array`, `Int32Array`, `Uint32Array`, `Float32Array`, `Float64Array` (with `length`, `byteLength`, `BYTES_PER_ELEMENT`, `subarray`) |
| **ArrayBuffer** | `byteLength` |
| **Date** | `now` |
| **Function** | `call`, `apply`, `bind`, `toString` |

### Global Functions

- `parseInt`, `parseFloat`
- `isNaN`, `isFinite`
- `Boolean`, `Number`, `String` (type coercion)
- `print`, `console.log`, `console.error`, `console.warn`
- `gc` (trigger garbage collection)
- `load` (load and execute JavaScript file)
- `setTimeout`, `clearTimeout`
- `performance.now`
- `globalThis`

## Architecture

```
src/
├── lib.rs           # Library entry point
├── value.rs         # Tagged union value representation
├── context.rs       # JavaScript context and evaluation
├── gc/
│   ├── allocator.rs # Arena allocator
│   └── collector.rs # Mark-compact GC
├── vm/
│   ├── opcode.rs    # Bytecode opcodes (~80)
│   ├── interpreter.rs # Bytecode interpreter
│   └── stack.rs     # Value stack
├── parser/
│   ├── lexer.rs     # Tokenizer
│   └── compiler.rs  # Parser & bytecode generator
├── runtime/
│   ├── object.rs    # Object representation
│   ├── string.rs    # String handling
│   ├── array.rs     # Array with no-hole semantics
│   ├── function.rs  # Function & closure types
│   └── property.rs  # Property hash table
├── util/
│   ├── dtoa.rs      # Number to string conversion
│   └── unicode.rs   # UTF-8/UTF-16 handling
└── bin/
    └── mqjs.rs      # CLI/REPL application
```

## Bytecode Format

MQuickJS-RS can compile JavaScript to bytecode for faster loading:

```bash
# Compile
mqjs -c app.js        # Creates app.qbc

# Run compiled bytecode
mqjs app.qbc
```

Bytecode files use the `.qbc` extension with a simple binary format:
- Magic bytes: `MQJS`
- Version: 1 byte
- Serialized function bytecode

## Memory Model

Values are represented as tagged unions fitting in a single machine word:

- **Integers**: 31-bit signed integers (inline)
- **Special values**: `null`, `undefined`, `true`, `false`
- **Objects**: Pointer to GC-managed heap object
- **Strings**: UTF-8 encoded, interned

The garbage collector uses mark-compact collection, which:
- Has smaller object headers than reference counting
- Eliminates memory fragmentation
- Handles cycles automatically

## Tests

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture
```

Currently **373 tests** covering all implemented features.

## Differences from QuickJS

MQuickJS (and this Rust port) is a **minimalist subset** of QuickJS:

| Feature | QuickJS | MQuickJS-RS |
|---------|---------|-------------|
| ES version | ES2020+ | ES5 subset |
| Reference counting | Yes | No (tracing GC) |
| Generators | Yes | No |
| Async/await | Yes | No |
| Modules | Yes | No |
| BigInt | Yes | No |
| Proxies | Yes | No |
| Size | ~200KB | ~10KB RAM capable |

## Learning Resources

- **[How It Works](docs/HOW_IT_WORKS.md)** - Deep dive into JavaScript engine internals for learners: lexer, parser, bytecode, VM, garbage collection, closures, and more
- **[Benchmark Analysis](docs/BENCHMARK_ANALYSIS.md)** - Why Rust is 3x faster on recursion (stackless interpreter) and why C is faster on arrays (no bounds checking)

## Benchmarks

Performance comparison between MQuickJS-RS (Rust) and original MQuickJS (C).

**Machine**: Apple M4 Max, 64 GB RAM, macOS

| Benchmark | Rust (s) | C (s) | Ratio | Notes |
|-----------|----------|-------|-------|-------|
| fib | 0.018 | 0.059 | **0.30x** | Rust 3.3x faster |
| loop | 0.019 | 0.035 | **0.54x** | Rust 1.9x faster |
| json | 0.022 | 0.024 | **0.93x** | Rust 8% faster |
| string | 0.017 | 0.017 | 1.01x | Equal |
| object | 0.018 | 0.017 | 1.08x | Equal |
| closure | 0.018 | 0.016 | 1.10x | Equal |
| array | 0.019 | 0.017 | 1.15x | C 15% faster |
| sieve | 0.039 | 0.022 | 1.73x | C 73% faster |

**Summary**: The Rust port is competitive with the original C implementation. It's significantly faster on recursive function calls (stackless interpreter) and loops (LLVM optimizations), roughly equal on object/string/closure operations, and slower on array-heavy workloads (bounds checking overhead).

### Running Benchmarks

```bash
# Build original C implementation
git submodule update --init
make -C vendor/mquickjs

# Run comparison
./benches/compare.sh

# Run detailed Rust benchmarks (Criterion)
cargo bench
```

## License

MIT License

## Credits

- [Fabrice Bellard](https://bellard.org/) - Original MQuickJS C implementation
- **This entire Rust port was written by [Claude](https://claude.ai)** (Anthropic's AI assistant), using [Claude Code](https://claude.ai/claude-code) to autonomously implement all 373 tests and ~20,000 lines of Rust code based on the original C reference implementation
