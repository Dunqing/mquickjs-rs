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

- [x] 5.1 Implement `Object` constructor and prototype (partial: Object.keys, Object.values, Object.entries)
- [x] 5.2 Implement `Function` prototype (call, apply, bind)
- [x] 5.3 Implement `Array` constructor and methods (push, pop, shift, unshift, indexOf, join, reverse, slice, length, Array.isArray, map, filter, forEach, reduce, find, findIndex, some, every, includes)
- [x] 5.4 Implement `String` constructor and methods (length, charAt, indexOf, slice, substring, toUpperCase, toLowerCase, trim, split)
- [x] 5.5 Implement `Number` constructor and methods (isInteger, isNaN, isFinite, MAX_VALUE, MIN_VALUE)
- [x] 5.6 Implement `Boolean` constructor (Boolean/Number/String as functions)
- [x] 5.7 Implement global functions (partial: parseInt, isNaN)

**Status**: In Progress (native function infrastructure complete)

---

### Stage 6: Extended Builtins
**Goal**: Complete built-in library

- [x] 6.1 Implement `Error` hierarchy (Error, TypeError, ReferenceError, SyntaxError, RangeError)
- [x] 6.2 Implement `Math` object (partial: abs, floor, ceil, round, sqrt, pow, max, min)
- [x] 6.3 Implement `JSON` object (stringify, parse)
- [x] 6.4 Implement `RegExp` object (constructor, test, exec)
- [ ] 6.5 Implement `TypedArray` objects
- [x] 6.6 Implement `Date.now()`

**Status**: In Progress (Error, Math, JSON, Date, RegExp complete)

---

### Stage 7: Advanced Features
**Goal**: Complete language features

- [x] 7.1 Implement `for-in` iteration
- [x] 7.2 Implement `for-of` iteration
- [x] 7.3 Implement `try-catch-finally`
- [x] 7.4 Implement closure variable capture
- [x] 7.5 Implement array literals and operations
- [x] 7.6 Implement `new` operator and basic object support
- [x] 7.7 Implement `delete` and `in` operators
- [x] 7.8 Implement `instanceof`

**Status**: Complete

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

**Last Updated**: Stage 5-6 In Progress (Type coercion complete)

**Files Created/Updated**:
- `src/lib.rs` - Main library entry
- `src/value.rs` - JSValue tagged union with string, closure, array support
- `src/context.rs` - JSContext with closure, try-catch, array tests
- `src/gc/mod.rs`, `allocator.rs`, `collector.rs` - GC system
- `src/vm/mod.rs`, `opcode.rs`, `interpreter.rs`, `stack.rs` - VM with closure, exception, array support
- `src/parser/mod.rs`, `lexer.rs`, `compiler.rs` - Parser with closure capture, try-catch-finally, arrays
- `src/builtins/` - Builtin stubs
- `src/runtime/mod.rs` - Runtime module
- `src/runtime/object.rs` - JSObject, ClassId, Property types
- `src/runtime/string.rs` - JSString, StringTable
- `src/runtime/property.rs` - PropertyTable with hash table
- `src/runtime/array.rs` - JSArray with no-hole semantics
- `src/runtime/function.rs` - CFunction, Closure, FunctionBytecode with CaptureInfo
- `src/util/mod.rs`, `dtoa.rs`, `unicode.rs` - Utilities
- `src/bin/mqjs.rs` - REPL binary

**Test Count**: 306 passing

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
- typeof operator (returns proper string values)
- String literals with concatenation support
- print statement for output

**Stage 7.4 Closure Features**:
- Closure variable capture (value capture semantics)
- CaptureInfo struct for tracking captured variables
- GetVarRef/PutVarRef opcodes for accessing captured variables
- ClosureData structure in interpreter for storing captured values
- FClosure opcode creates closures with captured variable values
- Call opcode handles closure calls with proper frame setup
- Nested closures that capture from outer function's locals or captures
- typeof closure returns "function"

**Stage 7.3 Try-Catch-Finally Features**:
- throw statement for raising exceptions
- try-catch statement for catching exceptions
- try-catch-finally statement with finally block
- Catch opcode sets up exception handler
- DropCatch opcode removes exception handler when try completes normally
- Throw opcode triggers exception unwinding to nearest handler
- ExceptionHandler struct tracks frame depth, catch PC, and stack depth
- Exception value passed to catch block as parameter
- Nested try-catch with proper handler chaining
- Exception propagation through function calls

**Stage 7.5 Array Features**:
- Array value type using special tag encoding
- Array storage in interpreter (Vec<Vec<Value>>)
- ArrayFrom opcode creates array from stack elements
- GetArrayEl/GetArrayEl2 opcodes for element access
- PutArrayEl opcode for element assignment with auto-extend
- Array literal parsing: [expr, expr, ...]
- Array access parsing: arr[idx] and arr[idx] = value
- Out-of-bounds access returns undefined
- Trailing comma support in array literals

**Stage 7.6 Object and New Operator Features**:
- Object value type using special tag encoding (bit 25 marker)
- Object storage in interpreter (Vec<(String, Value)> for properties)
- GetField/PutField opcodes for property access (obj.prop and obj.prop = val)
- new_expr_target() parses constructor without consuming call
- CallConstructor opcode creates object and calls constructor with this=object
- typeof returns "object" for objects
- Built-in string constants for typeof comparisons

**Stage 7.8 InstanceOf Features**:
- ObjectInstance struct stores constructor reference when created via `new`
- InstanceOf opcode compares stored constructor with right operand
- Multiple instances of same constructor correctly recognized
- Works with closures and regular functions

**Stage 7.1 For-In Features**:
- ForInIterator struct stores keys and iteration position
- Iterator index stored in hidden local variable
- ForInStart opcode creates iterator from object/array
- ForInNext opcode returns next key and done flag
- Iterates over object property names or array indices
- Supports break and continue in for-in loops

**Stage 7.2 For-Of Features**:
- ForOfIterator struct stores values and iteration position
- Iterator index stored in hidden local variable (like for-in)
- ForOfStart opcode creates iterator from object/array
- ForOfNext opcode returns next value and done flag
- Iterates over array elements or object property values
- Supports break and continue in for-of loops
- Token::Of keyword added to lexer

**Constructor Return Fix**:
- Added is_constructor flag to CallFrame
- CallConstructor now uses new_constructor/new_closure_constructor frame creators
- do_return automatically returns 'this' if constructor doesn't return an object
- Enables standard JavaScript constructor behavior (implicit this return)

**Stage 5.7 Native Function Features**:
- Native function type (`NativeFn`) and registry (`NativeFunction` struct)
- `native_functions: Vec<NativeFunction>` registry in Interpreter
- `register_native()` method for adding native functions
- `get_native_func()` method for looking up functions by name
- `call_native_func()` method for calling native functions
- Native function support in Call opcode handler
- `GetGlobal` opcode for looking up global variables/functions
- Global value handling (undefined, NaN, Infinity)
- Initial native functions implemented:
  - `parseInt` - parse integer from value
  - `isNaN` - check if value is not a number
- Compiler emits `GetGlobal` for unresolved identifiers

**Stage 6.2 Math Object Features**:
- BUILTIN_OBJECT_MARKER for encoding builtin objects in Value
- Value::builtin_object() constructor and to_builtin_object_idx() extractor
- BUILTIN_MATH constant for Math object index
- GetGlobal handler returns Math builtin object for "Math" name
- GetField handler checks for builtin objects and dispatches to get_builtin_property()
- Math methods implemented: abs, floor, ceil, round, sqrt, pow, max, min
- 6 new Math object tests

**Stage 5.3 Array Method Features**:
- GetField2 opcode keeps object on stack for method calls
- CallMethod opcode passes object as 'this' to method
- Compiler detects method call pattern (obj.method()) and emits GetField2+CallMethod
- get_array_property() dispatches to Array.prototype methods
- Array.prototype.push() - add elements, return new length
- Array.prototype.pop() - remove and return last element
- Array.prototype.shift() - remove and return first element
- Array.prototype.unshift() - add to front, return new length
- Array.prototype.indexOf() - find element, return index or -1
- Array.prototype.join() - join elements with separator
- Array.prototype.reverse() - reverse array in place
- Array.prototype.slice() - return shallow copy of portion
- arr.length - property returns array length
- 13 array method tests

**Stage 5.4 String Method Features**:
- get_string_by_idx() helper for string lookup
- get_string_property() dispatches to String.prototype methods
- String.prototype.length - returns string length
- String.prototype.charAt(index) - get character at position
- String.prototype.indexOf(search) - find substring position
- String.prototype.slice(start, end) - extract portion with negative index support
- String.prototype.substring(start, end) - extract portion (swaps if start > end)
- String.prototype.toUpperCase() - convert to uppercase
- String.prototype.toLowerCase() - convert to lowercase
- String.prototype.trim() - remove whitespace from both ends
- String.prototype.split(separator) - split into array
- String.prototype.concat(...args) - concatenate strings
- String.prototype.repeat(count) - repeat string n times
- String.prototype.startsWith(search, position) - check prefix
- String.prototype.endsWith(search, endPosition) - check suffix
- String.prototype.padStart(targetLength, padString) - pad from start
- String.prototype.padEnd(targetLength, padString) - pad from end
- String.prototype.replace(search, replacement) - replace first occurrence
- String.prototype.includes(search, position) - check contains
- Note: Methods work on runtime strings (from concatenation); compile-time literal support pending
- 19 String method tests

**Stage 5.5 Number Static Methods**:
- BUILTIN_NUMBER constant for Number object
- Number.isInteger(value) - check if value is an integer
- Number.isNaN(value) - check if value is NaN
- Number.isFinite(value) - check if value is finite
- Number.parseInt(value) - alias for global parseInt
- Number.MAX_VALUE, MIN_VALUE, MAX_SAFE_INTEGER, MIN_SAFE_INTEGER constants
- 4 Number method tests

**console Object**:
- BUILTIN_CONSOLE constant for console object
- console.log(...args) - print to stdout with value formatting
- console.error(...args) - print to stderr
- console.warn(...args) - print to stderr
- format_value() helper for converting values to strings
- Array and object formatting support
- 4 console method tests

**Stage 6.1 Error Hierarchy**:
- ErrorObject struct with name and message fields
- error_objects: Vec<ErrorObject> storage in Interpreter
- ERROR_OBJECT_MARKER (bit 20) for value encoding
- Value::error_object(), is_error_object(), to_error_object_idx() methods
- BUILTIN_ERROR, BUILTIN_TYPE_ERROR, BUILTIN_REFERENCE_ERROR, BUILTIN_SYNTAX_ERROR, BUILTIN_RANGE_ERROR constants
- GetGlobal handler returns builtin Error types
- CallConstructor handles Error builtin constructors (creates ErrorObject)
- get_error_property() returns name and message as runtime strings
- GetField handler dispatches to get_error_property for error objects
- format_value() handles error object formatting
- throw new Error("msg") works with try-catch
- 8 Error tests

**Stage 6.3 JSON Object**:
- JSON.stringify(value) - serialize value to JSON string
- JSON.parse(string) - parse JSON string to value
- Supports: numbers, booleans, null, strings, arrays, objects
- JsonParser struct for parsing JSON with proper escape handling
- json_stringify_value() helper for recursive serialization
- escape_json_string() for proper JSON string escaping
- current_string_constants field for native function access to compile-time strings
- 11 JSON tests

**Stage 6.6 Date Object**:
- BUILTIN_DATE constant for Date object
- Date.now() returns current timestamp in milliseconds
- Value capped to 2^30 range for 31-bit integer compatibility
- Useful for relative timing within ~12 day windows
- 3 Date tests

**Stage 5.6 Type Coercion Functions**:
- Boolean(value) - coerces value to boolean
- Number(value) - coerces value to number
- String(value) - coerces value to string
- to_boolean(), to_number(), to_string_value() helper methods
- call_builtin_as_function() for calling builtins without `new`
- BUILTIN_STRING constant for String object
- 10 type coercion tests

**Stage 5.1 Object Static Methods**:
- BUILTIN_OBJECT constant for Object global
- BUILTIN_ARRAY constant for Array global
- Object.keys(obj) - returns array of property names
- Object.values(obj) - returns array of property values
- Object.entries(obj) - returns array of [key, value] pairs
- Array.isArray(value) - check if value is an array
- 6 Object/Array static method tests

**Stage 5.2 Function.prototype Methods**:
- Function.prototype.call(thisArg, ...args) - call function with specified this
- Function.prototype.apply(thisArg, argsArray) - call function with array of args
- Function.prototype.bind(thisArg, ...args) - create bound function
- call_value() method for calling function values from native code
- nested_call_target_depth field for proper return handling in nested calls
- get_function_property() to dispatch prototype method lookups
- 5 Function.prototype tests

**Stage 5.3 Array Higher-Order Methods**:
- Array.prototype.map(callback) - create new array with callback applied
- Array.prototype.filter(callback) - filter elements by predicate
- Array.prototype.forEach(callback) - call callback for each element
- Array.prototype.reduce(callback, initial) - reduce to single value
- Array.prototype.find(callback) - find first matching element
- Array.prototype.findIndex(callback) - find index of first match
- Array.prototype.some(callback) - check if any element matches
- Array.prototype.every(callback) - check if all elements match
- Array.prototype.includes(value) - check if array contains value
- 12 Array higher-order function tests

**Stage 6.4 RegExp Features**:
- RegExpObject struct with compiled regex, pattern, flags, and flag booleans
- REGEXP_OBJECT_MARKER (bit 19) for value encoding
- Value::regexp_object(), is_regexp_object(), to_regexp_object_idx() methods
- RegExp constructor: new RegExp(pattern, flags)
- Support for flags: g (global), i (ignoreCase), m (multiline)
- Regex compilation using Rust's regex crate
- RegExp.prototype.test(string) - returns boolean match result
- RegExp.prototype.exec(string) - returns match array or null
- get_regexp_property() for property access
- 8 RegExp tests

**Next Action**: Implement TypedArray (Stage 6.5) or more String methods
