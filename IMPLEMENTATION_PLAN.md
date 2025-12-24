# MQuickJS Rust Port - Implementation Plan

## Project Overview

**Goal**: Full feature parity Rust port of MQuickJS (Fabrice Bellard's minimalist JS engine)
**Approach**: Idiomatic Rust rewrite with performance matching C
**API**: Native Rust API only

**Source Stats**: ~28K lines C -> estimated ~20-25K lines Rust
**Reference**: `/Users/qing/p/github/mquickjs-ref/`

---

## Implementation Stages

### Stage 1: Foundation
**Goal**: Core types and memory infrastructure

- [x] 1.1 Create Cargo project with workspace structure
- [x] 1.2 Implement `JSValue` enum (tagged union matching C layout)
- [x] 1.3 Implement arena allocator (`gc/allocator.rs`)
- [x] 1.4 Implement basic GC traits and collector (`gc/collector.rs`)
- [x] 1.5 Implement `JSContext` struct with memory layout
- [x] 1.6 Add cutils equivalents (`util/mod.rs`)

**Status**: Complete

---

### Stage 2: Object System
**Goal**: JavaScript object representation and property access

- [x] 2.1 Implement `JSObject` struct with class system
- [x] 2.2 Implement `JSString` with UTF-8 storage
- [x] 2.3 Implement property hash table
- [x] 2.4 Implement basic property operations
- [x] 2.5 Implement `JSArray` with no-hole semantics
- [x] 2.6 Implement `JSFunction` types (closure, C function)

**Status**: Complete

---

### Stage 3: Bytecode & VM Core
**Goal**: Execute bytecode instructions

- [x] 3.1 Define opcode enum (port `mquickjs_opcode.h`)
- [x] 3.2 Implement `JSFunctionBytecode` struct
- [x] 3.3 Implement value stack
- [x] 3.4 Implement bytecode interpreter loop
- [x] 3.5 Implement function call mechanism

**Status**: Complete

---

### Stage 4: Parser & Compiler
**Goal**: Parse JavaScript source to bytecode

- [x] 4.1 Implement lexer/tokenizer
- [x] 4.2 Implement parser state machine
- [x] 4.3 Implement expression parsing
- [x] 4.4 Implement statement parsing
- [x] 4.5 Implement bytecode generation
- [x] 4.6 Implement scope and variable resolution (local variables)

**Status**: Complete (closures pending for Stage 7)

---

### Stage 5: Core Builtins
**Goal**: Essential JavaScript built-in objects

- [ ] 5.1 Implement `Object` constructor and prototype
- [ ] 5.2 Implement `Function` prototype
- [ ] 5.3 Implement `Array` constructor and methods
- [ ] 5.4 Implement `String` constructor and methods
- [ ] 5.5 Implement `Number` constructor and methods
- [ ] 5.6 Implement `Boolean` constructor
- [ ] 5.7 Implement global functions

**Status**: Not Started

---

### Stage 6: Extended Builtins
**Goal**: Complete built-in library

- [ ] 6.1 Implement `Error` hierarchy
- [ ] 6.2 Implement `Math` object
- [ ] 6.3 Implement `JSON` object
- [ ] 6.4 Implement `RegExp` object
- [ ] 6.5 Implement `TypedArray` objects
- [ ] 6.6 Implement `Date.now()`

**Status**: Not Started

---

### Stage 7: Advanced Features
**Goal**: Complete language features

- [ ] 7.1 Implement `for-in` iteration
- [ ] 7.2 Implement `for-of` iteration
- [ ] 7.3 Implement `try-catch-finally`
- [ ] 7.4 Implement closure variable capture
- [ ] 7.5 Implement `new` operator
- [ ] 7.6 Implement `delete` and `in` operators
- [ ] 7.7 Implement `typeof` and `instanceof`

**Status**: Not Started

---

### Stage 8: REPL & CLI
**Goal**: Usable command-line tool

- [x] 8.1 Implement CLI skeleton
- [ ] 8.2 Implement argument parsing
- [ ] 8.3 Implement line editing
- [ ] 8.4 Implement bytecode serialization
- [ ] 8.5 Implement memory stats

**Status**: In Progress

---

### Stage 9: Optimization & Polish
**Goal**: Performance parity with C version

- [ ] 9.1 Profile and optimize hot paths
- [ ] 9.2 Optimize GC performance
- [ ] 9.3 Reduce memory usage
- [ ] 9.4 Add benchmarks
- [ ] 9.5 Documentation

**Status**: Not Started

---

## Current Progress

**Last Updated**: Stage 4 Mostly Complete

**Files Created/Updated**:
- `src/lib.rs` - Main library entry
- `src/value.rs` - JSValue tagged union
- `src/context.rs` - JSContext
- `src/gc/mod.rs`, `allocator.rs`, `collector.rs` - GC system
- `src/vm/mod.rs`, `opcode.rs`, `interpreter.rs`, `stack.rs` - VM
- `src/parser/mod.rs`, `lexer.rs`, `compiler.rs` - Parser with expression/statement parsing
- `src/builtins/` - Builtin stubs
- `src/runtime/mod.rs` - Runtime module
- `src/runtime/object.rs` - JSObject, ClassId, Property types
- `src/runtime/string.rs` - JSString, StringTable
- `src/runtime/property.rs` - PropertyTable with hash table
- `src/runtime/array.rs` - JSArray with no-hole semantics
- `src/runtime/function.rs` - CFunction, Closure, FunctionBytecode
- `src/util/mod.rs`, `dtoa.rs`, `unicode.rs` - Utilities
- `src/bin/mqjs.rs` - REPL binary

**Test Count**: 130 passing

**Stage 4 Compiler Features**:
- Precedence climbing expression parser
- All binary operators (+, -, *, /, %, **, &, |, ^, <<, >>, >>>)
- Comparison operators (<, <=, >, >=, ==, !=, ===, !==)
- Unary operators (-, +, !, ~, typeof, ++, --)
- Ternary operator (?:)
- Short-circuit logical operators (&&, ||)
- Assignment expressions (=, +=, -=, *=, /=, %=, &=, |=, ^=, <<=, >>=, >>>=)
- Statement parsing (var/let/const, if/else, while, for, return, block)
- Local variable tracking with max_locals for proper frame allocation
- Optimized integer emission (Push0-7, PushI8, PushI16)
- Jump patching for control flow
- Context.eval() for end-to-end JavaScript execution
- Function declarations with parameters
- Function calls with argument passing
- Recursive functions (via ThisFunc opcode)
- break and continue statements in loops
- typeof operator (returns type code integers)

**Next Action**: Begin Stage 5 (Core Builtins) or add closures (Stage 7)
