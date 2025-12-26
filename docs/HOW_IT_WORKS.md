# How a JavaScript Engine Works

A deep dive into the internals of MQuickJS-RS for learners interested in programming language implementation.

## Table of Contents

1. [Overview](#overview)
2. [The Pipeline](#the-pipeline)
3. [Lexer (Tokenizer)](#lexer-tokenizer)
4. [Parser & Compiler](#parser--compiler)
5. [Bytecode](#bytecode)
6. [Virtual Machine](#virtual-machine)
7. [Value Representation](#value-representation)
8. [Garbage Collection](#garbage-collection)
9. [Built-in Objects](#built-in-objects)
10. [Closures](#closures)
11. [Exception Handling](#exception-handling)

---

## Overview

A JavaScript engine transforms human-readable JavaScript code into something a computer can execute. The process involves several stages:

```
Source Code → Lexer → Tokens → Parser → AST → Compiler → Bytecode → VM → Result
```

MQuickJS simplifies this by combining parsing and compilation into a single pass (no explicit AST), which reduces memory usage - critical for embedded systems.

---

## The Pipeline

### What happens when you run `1 + 2 * 3`?

```javascript
// Input
1 + 2 * 3

// Step 1: Lexer produces tokens
[NUMBER(1), PLUS, NUMBER(2), STAR, NUMBER(3)]

// Step 2: Parser/Compiler produces bytecode
push_i8 1      // Push 1 onto stack
push_i8 2      // Push 2 onto stack
push_i8 3      // Push 3 onto stack
mul            // Pop 2,3 → Push 6
add            // Pop 1,6 → Push 7
return         // Return top of stack (7)

// Step 3: VM executes bytecode
Stack: [] → [1] → [1,2] → [1,2,3] → [1,6] → [7]
Result: 7
```

---

## Lexer (Tokenizer)

The lexer (`src/parser/lexer.rs`) breaks source code into tokens - the smallest meaningful units.

### Token Types

```rust
pub enum Token {
    // Literals
    Number(f64),           // 42, 3.14
    String(String),        // "hello"
    Identifier(String),    // foo, myVar

    // Keywords
    Var, Let, Const,       // Variable declarations
    Function, Return,      // Functions
    If, Else, While, For,  // Control flow
    True, False, Null,     // Literals

    // Operators
    Plus, Minus, Star, Slash,  // + - * /
    Equal, EqualEqual,         // = ==
    Less, Greater,             // < >

    // Punctuation
    LeftParen, RightParen,     // ( )
    LeftBrace, RightBrace,     // { }
    LeftBracket, RightBracket, // [ ]
    Semicolon, Comma, Dot,     // ; , .
}
```

### How Lexing Works

```rust
// Simplified lexer logic
fn next_token(&mut self) -> Token {
    self.skip_whitespace();

    match self.current_char() {
        '0'..='9' => self.read_number(),
        'a'..='z' | 'A'..='Z' | '_' => self.read_identifier(),
        '"' | '\'' => self.read_string(),
        '+' => Token::Plus,
        '-' => Token::Minus,
        // ... etc
    }
}
```

### Example

```javascript
var x = 10 + 20;
```

Produces:
```
VAR, IDENTIFIER("x"), EQUAL, NUMBER(10), PLUS, NUMBER(20), SEMICOLON
```

---

## Parser & Compiler

The parser (`src/parser/compiler.rs`) reads tokens and generates bytecode. MQuickJS uses a **single-pass compiler** - it parses and emits bytecode simultaneously without building an AST.

### Operator Precedence

For expressions like `1 + 2 * 3`, we need to handle operator precedence (multiplication before addition).

MQuickJS uses **Pratt parsing** (precedence climbing):

```rust
// Precedence levels (higher = binds tighter)
fn precedence(op: &Token) -> u8 {
    match op {
        Token::Or => 1,              // ||
        Token::And => 2,             // &&
        Token::EqualEqual => 3,      // ==
        Token::Less | Token::Greater => 4,  // < >
        Token::Plus | Token::Minus => 5,    // + -
        Token::Star | Token::Slash => 6,    // * /
        Token::StarStar => 7,        // ** (right associative)
        _ => 0,
    }
}

// Parse expression with minimum precedence
fn parse_expression(&mut self, min_prec: u8) {
    // Parse left operand (atom: number, variable, parenthesized expr)
    self.parse_atom();

    // While next operator has higher precedence, continue
    while precedence(self.current()) >= min_prec {
        let op = self.advance();
        // Parse right side with higher precedence
        self.parse_expression(precedence(&op) + 1);
        // Emit operator bytecode
        self.emit_binary_op(op);
    }
}
```

### Parsing Statements

```rust
fn parse_statement(&mut self) {
    match self.current() {
        Token::Var => self.parse_var_declaration(),
        Token::If => self.parse_if_statement(),
        Token::While => self.parse_while_loop(),
        Token::For => self.parse_for_loop(),
        Token::Function => self.parse_function_declaration(),
        Token::Return => self.parse_return(),
        Token::LeftBrace => self.parse_block(),
        _ => self.parse_expression_statement(),
    }
}
```

### Control Flow Compilation

For `if/else`, we need **jump instructions**:

```javascript
if (x > 0) {
    print("positive");
} else {
    print("non-positive");
}
```

Compiles to:
```
get_local x           // Push x
push_i8 0             // Push 0
greater               // x > 0 → true/false
if_false [ELSE_ADDR]  // Jump to else if false
push_string "positive"
call print
goto [END_ADDR]       // Skip else block
[ELSE_ADDR]:
push_string "non-positive"
call print
[END_ADDR]:
```

The compiler uses **backpatching**: emit a placeholder jump address, then fill it in later when we know the target.

---

## Bytecode

Bytecode is a compact, efficient representation of the program. Each instruction is 1-3 bytes.

### Instruction Format

```
[opcode: 1 byte] [operand: 0-2 bytes]
```

### Core Opcodes (`src/vm/opcode.rs`)

```rust
pub enum OpCode {
    // Stack operations
    Push0, Push1, Push2, ..., Push7,  // Push small integers (0 bytes)
    PushI8(i8),                        // Push signed byte (1 byte)
    PushI16(i16),                      // Push signed short (2 bytes)
    PushConst(u16),                    // Push from constant pool
    PushUndefined, PushNull, PushTrue, PushFalse,

    Pop,                               // Discard top
    Dup,                               // Duplicate top
    Swap,                              // Swap top two

    // Arithmetic
    Add, Sub, Mul, Div, Mod,
    Neg,                               // Unary minus

    // Comparison
    Lt, Le, Gt, Ge, Eq, Ne, StrictEq, StrictNe,

    // Logical
    Not,                               // !x

    // Bitwise
    BitAnd, BitOr, BitXor, BitNot,
    Shl, Sar, Shr,                     // Shifts

    // Variables
    GetLocal(u8),                      // Get local variable
    SetLocal(u8),                      // Set local variable
    GetGlobal(u16),                    // Get global by name

    // Control flow
    Goto(i16),                         // Unconditional jump
    IfFalse(i16),                      // Jump if top is falsy
    IfTrue(i16),                       // Jump if top is truthy

    // Functions
    Call(u8),                          // Call with N arguments
    Return,                            // Return from function

    // Objects
    GetField(u16),                     // obj.property
    PutField(u16),                     // obj.property = value
    GetArrayEl,                        // arr[index]
    PutArrayEl,                        // arr[index] = value
}
```

### Example Bytecode

```javascript
function add(a, b) {
    return a + b;
}
add(3, 4);
```

```
# Function 'add' bytecode:
00: get_local 0      # Push parameter 'a'
02: get_local 1      # Push parameter 'b'
04: add              # a + b
05: return           # Return result

# Main code:
00: push_i8 3        # Push argument 3
02: push_i8 4        # Push argument 4
04: call 2           # Call with 2 arguments
06: return
```

---

## Virtual Machine

The VM (`src/vm/interpreter.rs`) executes bytecode using a **stack-based architecture**.

### Stack Machine Basics

Unlike register machines (like x86), stack machines use an operand stack:

```
Operation: 3 + 4 * 2

push 3       Stack: [3]
push 4       Stack: [3, 4]
push 2       Stack: [3, 4, 2]
mul          Stack: [3, 8]      (4 * 2 = 8)
add          Stack: [11]        (3 + 8 = 11)
```

### The Interpreter Loop

```rust
fn execute(&mut self, bytecode: &[u8]) -> Result<Value, Error> {
    let mut pc = 0;  // Program counter

    loop {
        let opcode = bytecode[pc];
        pc += 1;

        match opcode {
            OP_PUSH_I8 => {
                let value = bytecode[pc] as i8;
                pc += 1;
                self.stack.push(Value::int(value as i32));
            }

            OP_ADD => {
                let b = self.stack.pop();
                let a = self.stack.pop();
                self.stack.push(a + b);
            }

            OP_GET_LOCAL => {
                let index = bytecode[pc];
                pc += 1;
                let value = self.get_local(index);
                self.stack.push(value);
            }

            OP_IF_FALSE => {
                let offset = read_i16(&bytecode[pc..]);
                pc += 2;
                if self.stack.pop().is_falsy() {
                    pc = (pc as i32 + offset as i32) as usize;
                }
            }

            OP_CALL => {
                let argc = bytecode[pc];
                pc += 1;
                let func = self.stack.pop();
                let args = self.stack.pop_n(argc);
                self.call_function(func, args)?;
            }

            OP_RETURN => {
                let result = self.stack.pop();
                return Ok(result);
            }

            // ... ~80 more opcodes
        }
    }
}
```

### Call Stack

For function calls, we maintain a **call stack** of frames:

```rust
struct CallFrame {
    return_pc: usize,      // Where to return to
    base_pointer: usize,   // Start of locals on stack
    function: FunctionRef, // Current function
}
```

```
# Calling add(3, 4):

Main stack:  [... | 3 | 4]
                   ↑ base_pointer

Call 'add':
- Save return address
- Set base_pointer to arguments
- Execute 'add' bytecode
- Pop frame, restore state
```

---

## Value Representation

JavaScript has dynamic types. Every value must carry its type at runtime.

### Tagged Values (`src/value.rs`)

MQuickJS uses **tagged pointers** - stealing bits from pointers to encode type information:

```rust
// 64-bit value representation
// Bit 0: 0 = 31-bit integer (value << 1)
// Bits 0-2 = 001: Pointer to heap object
// Bits 0-2 = 011: Special value (null, undefined, bool)

pub struct Value(usize);

impl Value {
    // Small integers stored inline (no allocation!)
    pub fn int(n: i32) -> Value {
        Value((n as usize) << 1)  // Shift left, bit 0 = 0
    }

    pub fn to_i32(&self) -> Option<i32> {
        if self.0 & 1 == 0 {  // Check tag bit
            Some((self.0 as i32) >> 1)
        } else {
            None
        }
    }

    // Special values use tag bits
    pub fn null() -> Value {
        Value(0b0111)  // Special tag + null subtype
    }

    pub fn undefined() -> Value {
        Value(0b1011)  // Special tag + undefined subtype
    }
}
```

### Why Tagged Values?

1. **Small integers are free** - no memory allocation
2. **Single word** - fits in registers, cache-friendly
3. **Type checks are fast** - just check bits

### Heap Objects

Larger values (strings, arrays, objects) live on the heap:

```rust
// Heap-allocated string
struct JSString {
    header: GcHeader,  // For garbage collector
    length: u32,
    data: [u8],        // UTF-8 bytes
}

// Heap-allocated object
struct JSObject {
    header: GcHeader,
    properties: HashMap<String, Value>,
}
```

---

## Garbage Collection

JavaScript automatically manages memory. MQuickJS uses **mark-compact** collection.

### Why Mark-Compact?

| Approach | Pros | Cons |
|----------|------|------|
| Reference counting | Immediate cleanup | Cycles leak, overhead per write |
| Mark-sweep | Handles cycles | Fragmentation |
| **Mark-compact** | No fragmentation, handles cycles | Pause time |

### How It Works

```
1. MARK PHASE: Find all reachable objects
   - Start from "roots" (stack, globals)
   - Recursively mark everything reachable

2. COMPACT PHASE: Move live objects together
   - Slide objects to eliminate gaps
   - Update all pointers
```

### Example

```
Before GC:
[A][garbage][B][garbage][C][garbage]

After mark: A, B, C are live

After compact:
[A][B][C][free space...]
```

### The Algorithm (`src/gc/collector.rs`)

```rust
fn collect(heap: &mut Heap) {
    // Mark phase
    for root in get_roots() {
        mark(root);
    }

    // Compact phase
    let mut write_ptr = heap.start;
    for obj in heap.objects() {
        if obj.is_marked() {
            // Move object to write_ptr
            if write_ptr != obj.address() {
                copy(obj, write_ptr);
                update_references(obj.address(), write_ptr);
            }
            write_ptr += obj.size();
            obj.unmark();
        }
    }
    heap.free_ptr = write_ptr;
}

fn mark(obj: &Object) {
    if obj.is_marked() { return; }  // Already visited
    obj.set_marked();

    // Recursively mark children
    for child in obj.references() {
        mark(child);
    }
}
```

---

## Built-in Objects

JavaScript has many built-in objects. MQuickJS implements them as native Rust functions.

### Native Function Interface

```rust
// Native function signature
type NativeFn = fn(
    interp: &mut Interpreter,
    this: Value,
    args: &[Value]
) -> Result<Value, String>;

// Example: Array.prototype.push
fn array_push(
    interp: &mut Interpreter,
    this: Value,
    args: &[Value]
) -> Result<Value, String> {
    let array = this.as_array()?;
    for arg in args {
        array.push(*arg);
    }
    Ok(Value::int(array.len() as i32))
}

// Registration
interp.register_native("Array.prototype.push", array_push);
```

### Method Dispatch

When you call `arr.push(x)`:

```
1. GetField2 "push"     // Get method, keep arr on stack
2. // Stack: [arr, push_function]
3. CallMethod 1         // Call with 1 arg, this=arr
```

```rust
fn get_field(&self, obj: Value, name: &str) -> Value {
    if obj.is_array() {
        match name {
            "length" => Value::int(obj.len()),
            "push" => self.get_native("Array.prototype.push"),
            "pop" => self.get_native("Array.prototype.pop"),
            // ...
        }
    } else if obj.is_string() {
        // String methods...
    }
    // etc
}
```

---

## Closures

Closures are functions that capture variables from their enclosing scope.

### The Challenge

```javascript
function makeCounter() {
    var count = 0;
    return function() {
        count = count + 1;  // Accesses outer 'count'
        return count;
    };
}

var counter = makeCounter();
counter();  // 1
counter();  // 2
```

When `makeCounter` returns, its local `count` should be gone... but the inner function still needs it!

### Solution: Captured Variables

```rust
struct Closure {
    function: FunctionBytecode,
    captured: Vec<Value>,  // Captured variable values
}
```

The compiler tracks which variables are captured:

```rust
struct CaptureInfo {
    outer_index: usize,  // Index in outer function
    is_local: bool,      // From locals or outer captures
}
```

### Compilation

```javascript
function outer() {
    var x = 10;
    return function inner() {
        return x;  // Captures x
    };
}
```

Compiles to:

```
# outer:
push_i8 10
set_local 0          # x = 10
fclosure [inner], [CaptureInfo { index: 0, is_local: true }]
return

# inner:
get_var_ref 0        # Get captured x
return
```

### Runtime

```rust
fn execute_fclosure(&mut self, func_idx: usize, captures: &[CaptureInfo]) {
    let mut captured_values = Vec::new();

    for cap in captures {
        let value = if cap.is_local {
            self.get_local(cap.outer_index)
        } else {
            // Already a capture - get from current closure
            self.current_closure().captured[cap.outer_index]
        };
        captured_values.push(value);
    }

    let closure = Closure {
        function: self.get_function(func_idx),
        captured: captured_values,
    };
    self.stack.push(Value::closure(closure));
}
```

---

## Exception Handling

JavaScript has `try/catch/finally` for error handling.

### The Mechanism

```javascript
try {
    throw new Error("oops");
} catch (e) {
    print(e.message);
} finally {
    print("cleanup");
}
```

### Exception Handlers

```rust
struct ExceptionHandler {
    catch_pc: usize,       // Where to jump on exception
    stack_depth: usize,    // Stack depth to restore
    frame_depth: usize,    // Call frame depth
}

// Handler stack
exception_handlers: Vec<ExceptionHandler>
```

### Bytecode

```
00: catch [20]         # Register handler at PC 20
02: # try block
    push_string "oops"
    new Error 1
    throw              # Jump to catch
10: drop_catch         # Remove handler (normal exit)
12: goto [30]          # Skip catch block
20: # catch block
    set_local 0        # e = exception value
    get_local 0
    get_field "message"
    call print 1
30: # finally
    push_string "cleanup"
    call print 1
```

### Throw Implementation

```rust
fn do_throw(&mut self, exception: Value) -> Result<(), Error> {
    // Find matching handler
    while let Some(handler) = self.exception_handlers.pop() {
        // Unwind call stack to handler's frame
        while self.call_stack.len() > handler.frame_depth {
            self.call_stack.pop();
        }

        // Restore stack depth
        self.stack.truncate(handler.stack_depth);

        // Push exception for catch block
        self.stack.push(exception);

        // Jump to catch
        self.pc = handler.catch_pc;
        return Ok(());
    }

    // No handler found - propagate error
    Err(Error::UncaughtException(exception))
}
```

---

## Further Reading

### Books
- *Crafting Interpreters* by Robert Nystrom - excellent free online book
- *Engineering a Compiler* by Cooper & Torczon
- *Modern Compiler Implementation* by Andrew Appel

### Papers
- "A No-Frills Introduction to Lua 5.1 VM Instructions" - great bytecode explanation
- "Efficient Implementation of the Smalltalk-80 System" - pioneering VM paper

### Source Code
- [MQuickJS (C)](https://bellard.org/quickjs/) - the original
- [QuickJS (C)](https://bellard.org/quickjs/) - full-featured predecessor
- [LuaJIT](https://luajit.org/) - extremely optimized Lua VM
- [V8](https://v8.dev/) - Google's JavaScript engine

---

## Exercises for Learners

1. **Add a new operator**: Implement the `**` (exponentiation) operator
   - Add token to lexer
   - Add precedence (right associative!)
   - Emit bytecode
   - Implement in VM

2. **Add a built-in function**: Implement `Math.sin()`
   - Register native function
   - Handle in `get_builtin_property`
   - Add test

3. **Trace execution**: Add a debug mode that prints each opcode as it executes

4. **Optimize**: Find a frequently-executed bytecode sequence and add a specialized opcode

5. **Add a type**: Implement a simple `Set` object
   - Create `SetObject` struct
   - Add marker bit for value encoding
   - Implement `add`, `has`, `delete` methods

---

*This documentation is part of MQuickJS-RS, a JavaScript engine written entirely by Claude.*
