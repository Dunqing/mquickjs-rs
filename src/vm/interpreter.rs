//! Bytecode interpreter
//!
//! Executes JavaScript bytecode using a stack-based virtual machine.

use crate::runtime::FunctionBytecode;
use crate::value::Value;
use crate::vm::opcode::OpCode;
use crate::vm::stack::Stack;

/// Call frame information
#[derive(Debug, Clone)]
pub struct CallFrame {
    /// Function bytecode being executed
    pub bytecode: *const FunctionBytecode,
    /// Program counter (offset into bytecode)
    pub pc: usize,
    /// Frame pointer (index into stack where locals start)
    pub frame_ptr: usize,
    /// Number of arguments
    pub arg_count: u16,
    /// Return address (pc to return to, or usize::MAX for top-level)
    pub return_pc: usize,
    /// Previous frame pointer
    pub prev_frame_ptr: usize,
    /// `this` value for this call
    pub this_val: Value,
    /// The function value itself (for self-reference/recursion)
    pub this_func: Value,
}

impl CallFrame {
    /// Create a new call frame
    pub fn new(
        bytecode: *const FunctionBytecode,
        frame_ptr: usize,
        arg_count: u16,
        this_val: Value,
        this_func: Value,
    ) -> Self {
        CallFrame {
            bytecode,
            pc: 0,
            frame_ptr,
            arg_count,
            return_pc: usize::MAX,
            prev_frame_ptr: 0,
            this_val,
            this_func,
        }
    }
}

/// Interpreter error
#[derive(Debug, Clone)]
pub enum InterpreterError {
    /// Stack underflow
    StackUnderflow,
    /// Stack overflow
    StackOverflow,
    /// Invalid opcode
    InvalidOpcode(u8),
    /// Division by zero
    DivisionByZero,
    /// Type error
    TypeError(String),
    /// Reference error
    ReferenceError(String),
    /// Internal error
    InternalError(String),
}

impl std::fmt::Display for InterpreterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StackUnderflow => write!(f, "stack underflow"),
            Self::StackOverflow => write!(f, "stack overflow"),
            Self::InvalidOpcode(op) => write!(f, "invalid opcode: {}", op),
            Self::DivisionByZero => write!(f, "division by zero"),
            Self::TypeError(msg) => write!(f, "TypeError: {}", msg),
            Self::ReferenceError(msg) => write!(f, "ReferenceError: {}", msg),
            Self::InternalError(msg) => write!(f, "InternalError: {}", msg),
        }
    }
}

impl std::error::Error for InterpreterError {}

/// Result type for interpreter operations
pub type InterpreterResult<T> = Result<T, InterpreterError>;

/// Interpreter state
pub struct Interpreter {
    /// Value stack
    stack: Stack,
    /// Call stack (frames)
    call_stack: Vec<CallFrame>,
    /// Maximum call recursion depth
    max_recursion: usize,
}

impl Interpreter {
    /// Default stack capacity
    const DEFAULT_STACK_SIZE: usize = 1024;
    /// Default max recursion
    const DEFAULT_MAX_RECURSION: usize = 512;

    /// Create a new interpreter
    pub fn new() -> Self {
        Interpreter {
            stack: Stack::new(Self::DEFAULT_STACK_SIZE),
            call_stack: Vec::with_capacity(64),
            max_recursion: Self::DEFAULT_MAX_RECURSION,
        }
    }

    /// Create an interpreter with custom settings
    pub fn with_config(stack_size: usize, max_recursion: usize) -> Self {
        Interpreter {
            stack: Stack::new(stack_size),
            call_stack: Vec::with_capacity(64),
            max_recursion,
        }
    }

    /// Execute bytecode and return the result
    ///
    /// # Safety
    /// The bytecode pointer must be valid for the duration of execution.
    pub fn execute(&mut self, bytecode: &FunctionBytecode) -> InterpreterResult<Value> {
        self.call_function(bytecode, Value::undefined(), &[])
    }

    /// Call a function with the given `this` value and arguments
    pub fn call_function(
        &mut self,
        bytecode: &FunctionBytecode,
        this_val: Value,
        args: &[Value],
    ) -> InterpreterResult<Value> {
        // Check recursion limit
        if self.call_stack.len() >= self.max_recursion {
            return Err(InterpreterError::InternalError(
                "maximum call stack size exceeded".to_string(),
            ));
        }

        let frame_ptr = self.stack.len();

        // Push arguments (pad with undefined if needed)
        for i in 0..bytecode.arg_count as usize {
            let arg = args.get(i).copied().unwrap_or(Value::undefined());
            self.stack.push(arg);
        }

        // Allocate space for locals (beyond arguments)
        let extra_locals = bytecode.local_count.saturating_sub(bytecode.arg_count);
        for _ in 0..extra_locals {
            self.stack.push(Value::undefined());
        }

        let frame = CallFrame::new(
            bytecode as *const _,
            frame_ptr,
            args.len().min(u16::MAX as usize) as u16,
            this_val,
            Value::undefined(), // Top-level call has no function value
        );
        self.call_stack.push(frame);

        // Run the interpreter loop
        self.run()
    }

    /// Main interpreter loop
    fn run(&mut self) -> InterpreterResult<Value> {
        loop {
            // Get current frame
            let frame = self.call_stack.last_mut().ok_or_else(|| {
                InterpreterError::InternalError("no active call frame".to_string())
            })?;

            // Safety: bytecode pointer is valid for frame lifetime
            let bytecode = unsafe { &*frame.bytecode };
            let bc = &bytecode.bytecode;

            // Check if we've reached the end
            if frame.pc >= bc.len() {
                // Implicit return undefined
                return Ok(Value::undefined());
            }

            // Fetch opcode
            let opcode_byte = bc[frame.pc];
            frame.pc += 1;

            // Decode and execute
            match opcode_byte {
                // Invalid
                op if op == OpCode::Invalid as u8 => {
                    return Err(InterpreterError::InvalidOpcode(op));
                }

                // Push integer constants
                op if op == OpCode::PushMinus1 as u8 => {
                    self.stack.push(Value::int(-1));
                }
                op if op == OpCode::Push0 as u8 => {
                    self.stack.push(Value::int(0));
                }
                op if op == OpCode::Push1 as u8 => {
                    self.stack.push(Value::int(1));
                }
                op if op == OpCode::Push2 as u8 => {
                    self.stack.push(Value::int(2));
                }
                op if op == OpCode::Push3 as u8 => {
                    self.stack.push(Value::int(3));
                }
                op if op == OpCode::Push4 as u8 => {
                    self.stack.push(Value::int(4));
                }
                op if op == OpCode::Push5 as u8 => {
                    self.stack.push(Value::int(5));
                }
                op if op == OpCode::Push6 as u8 => {
                    self.stack.push(Value::int(6));
                }
                op if op == OpCode::Push7 as u8 => {
                    self.stack.push(Value::int(7));
                }

                // Push 8-bit signed integer
                op if op == OpCode::PushI8 as u8 => {
                    let frame = self.call_stack.last_mut().unwrap();
                    let bytecode = unsafe { &*frame.bytecode };
                    let val = bytecode.bytecode[frame.pc] as i8 as i32;
                    frame.pc += 1;
                    self.stack.push(Value::int(val));
                }

                // Push 16-bit signed integer
                op if op == OpCode::PushI16 as u8 => {
                    let frame = self.call_stack.last_mut().unwrap();
                    let bytecode = unsafe { &*frame.bytecode };
                    let bc = &bytecode.bytecode;
                    let val = i16::from_le_bytes([bc[frame.pc], bc[frame.pc + 1]]) as i32;
                    frame.pc += 2;
                    self.stack.push(Value::int(val));
                }

                // Push constant from pool
                op if op == OpCode::PushConst as u8 => {
                    let frame = self.call_stack.last_mut().unwrap();
                    let bytecode = unsafe { &*frame.bytecode };
                    let bc = &bytecode.bytecode;
                    let idx = u16::from_le_bytes([bc[frame.pc], bc[frame.pc + 1]]) as usize;
                    frame.pc += 2;
                    let val = bytecode
                        .constants
                        .get(idx)
                        .copied()
                        .unwrap_or(Value::undefined());
                    self.stack.push(val);
                }

                // Push constant (8-bit index)
                op if op == OpCode::PushConst8 as u8 => {
                    let frame = self.call_stack.last_mut().unwrap();
                    let bytecode = unsafe { &*frame.bytecode };
                    let idx = bytecode.bytecode[frame.pc] as usize;
                    frame.pc += 1;
                    let val = bytecode
                        .constants
                        .get(idx)
                        .copied()
                        .unwrap_or(Value::undefined());
                    self.stack.push(val);
                }

                // Push undefined
                op if op == OpCode::Undefined as u8 => {
                    self.stack.push(Value::undefined());
                }

                // Push null
                op if op == OpCode::Null as u8 => {
                    self.stack.push(Value::null());
                }

                // Push false
                op if op == OpCode::PushFalse as u8 => {
                    self.stack.push(Value::bool(false));
                }

                // Push true
                op if op == OpCode::PushTrue as u8 => {
                    self.stack.push(Value::bool(true));
                }

                // Stack manipulation: Drop
                op if op == OpCode::Drop as u8 => {
                    self.stack
                        .pop()
                        .ok_or(InterpreterError::StackUnderflow)?;
                }

                // Stack manipulation: Dup
                op if op == OpCode::Dup as u8 => {
                    self.stack.dup().ok_or(InterpreterError::StackUnderflow)?;
                }

                // Stack manipulation: Swap
                op if op == OpCode::Swap as u8 => {
                    self.stack.swap().ok_or(InterpreterError::StackUnderflow)?;
                }

                // Get local variable (16-bit index)
                op if op == OpCode::GetLoc as u8 => {
                    let frame = self.call_stack.last_mut().unwrap();
                    let bytecode = unsafe { &*frame.bytecode };
                    let bc = &bytecode.bytecode;
                    let idx = u16::from_le_bytes([bc[frame.pc], bc[frame.pc + 1]]) as usize;
                    frame.pc += 2;
                    let frame_ptr = frame.frame_ptr;
                    let val = self
                        .stack
                        .get_local_at(frame_ptr, idx)
                        .unwrap_or(Value::undefined());
                    self.stack.push(val);
                }

                // Set local variable (16-bit index)
                op if op == OpCode::PutLoc as u8 => {
                    let val = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let frame = self.call_stack.last_mut().unwrap();
                    let bytecode = unsafe { &*frame.bytecode };
                    let bc = &bytecode.bytecode;
                    let idx = u16::from_le_bytes([bc[frame.pc], bc[frame.pc + 1]]) as usize;
                    frame.pc += 2;
                    let frame_ptr = frame.frame_ptr;
                    self.stack.set_local_at(frame_ptr, idx, val);
                }

                // Get local 0-3 (optimized)
                op if op == OpCode::GetLoc0 as u8 => {
                    let frame = self.call_stack.last().unwrap();
                    let frame_ptr = frame.frame_ptr;
                    let val = self
                        .stack
                        .get_local_at(frame_ptr, 0)
                        .unwrap_or(Value::undefined());
                    self.stack.push(val);
                }
                op if op == OpCode::GetLoc1 as u8 => {
                    let frame = self.call_stack.last().unwrap();
                    let frame_ptr = frame.frame_ptr;
                    let val = self
                        .stack
                        .get_local_at(frame_ptr, 1)
                        .unwrap_or(Value::undefined());
                    self.stack.push(val);
                }
                op if op == OpCode::GetLoc2 as u8 => {
                    let frame = self.call_stack.last().unwrap();
                    let frame_ptr = frame.frame_ptr;
                    let val = self
                        .stack
                        .get_local_at(frame_ptr, 2)
                        .unwrap_or(Value::undefined());
                    self.stack.push(val);
                }
                op if op == OpCode::GetLoc3 as u8 => {
                    let frame = self.call_stack.last().unwrap();
                    let frame_ptr = frame.frame_ptr;
                    let val = self
                        .stack
                        .get_local_at(frame_ptr, 3)
                        .unwrap_or(Value::undefined());
                    self.stack.push(val);
                }

                // Set local 0-3 (optimized)
                op if op == OpCode::PutLoc0 as u8 => {
                    let val = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let frame = self.call_stack.last().unwrap();
                    let frame_ptr = frame.frame_ptr;
                    self.stack.set_local_at(frame_ptr, 0, val);
                }
                op if op == OpCode::PutLoc1 as u8 => {
                    let val = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let frame = self.call_stack.last().unwrap();
                    let frame_ptr = frame.frame_ptr;
                    self.stack.set_local_at(frame_ptr, 1, val);
                }
                op if op == OpCode::PutLoc2 as u8 => {
                    let val = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let frame = self.call_stack.last().unwrap();
                    let frame_ptr = frame.frame_ptr;
                    self.stack.set_local_at(frame_ptr, 2, val);
                }
                op if op == OpCode::PutLoc3 as u8 => {
                    let val = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let frame = self.call_stack.last().unwrap();
                    let frame_ptr = frame.frame_ptr;
                    self.stack.set_local_at(frame_ptr, 3, val);
                }

                // Get local (8-bit index)
                op if op == OpCode::GetLoc8 as u8 => {
                    let frame = self.call_stack.last_mut().unwrap();
                    let bytecode = unsafe { &*frame.bytecode };
                    let idx = bytecode.bytecode[frame.pc] as usize;
                    frame.pc += 1;
                    let frame_ptr = frame.frame_ptr;
                    let val = self
                        .stack
                        .get_local_at(frame_ptr, idx)
                        .unwrap_or(Value::undefined());
                    self.stack.push(val);
                }

                // Set local (8-bit index)
                op if op == OpCode::PutLoc8 as u8 => {
                    let val = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let frame = self.call_stack.last_mut().unwrap();
                    let bytecode = unsafe { &*frame.bytecode };
                    let idx = bytecode.bytecode[frame.pc] as usize;
                    frame.pc += 1;
                    let frame_ptr = frame.frame_ptr;
                    self.stack.set_local_at(frame_ptr, idx, val);
                }

                // Get argument (16-bit index)
                op if op == OpCode::GetArg as u8 => {
                    let frame = self.call_stack.last_mut().unwrap();
                    let bytecode = unsafe { &*frame.bytecode };
                    let bc = &bytecode.bytecode;
                    let idx = u16::from_le_bytes([bc[frame.pc], bc[frame.pc + 1]]) as usize;
                    frame.pc += 2;
                    let frame_ptr = frame.frame_ptr;
                    // Arguments are at the start of the frame
                    let val = self
                        .stack
                        .get_local_at(frame_ptr, idx)
                        .unwrap_or(Value::undefined());
                    self.stack.push(val);
                }

                // Set argument (16-bit index)
                op if op == OpCode::PutArg as u8 => {
                    let val = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let frame = self.call_stack.last_mut().unwrap();
                    let bytecode = unsafe { &*frame.bytecode };
                    let bc = &bytecode.bytecode;
                    let idx = u16::from_le_bytes([bc[frame.pc], bc[frame.pc + 1]]) as usize;
                    frame.pc += 2;
                    let frame_ptr = frame.frame_ptr;
                    self.stack.set_local_at(frame_ptr, idx, val);
                }

                // Get argument 0-3 (optimized)
                op if op == OpCode::GetArg0 as u8 => {
                    let frame = self.call_stack.last().unwrap();
                    let frame_ptr = frame.frame_ptr;
                    let val = self
                        .stack
                        .get_local_at(frame_ptr, 0)
                        .unwrap_or(Value::undefined());
                    self.stack.push(val);
                }
                op if op == OpCode::GetArg1 as u8 => {
                    let frame = self.call_stack.last().unwrap();
                    let frame_ptr = frame.frame_ptr;
                    let val = self
                        .stack
                        .get_local_at(frame_ptr, 1)
                        .unwrap_or(Value::undefined());
                    self.stack.push(val);
                }
                op if op == OpCode::GetArg2 as u8 => {
                    let frame = self.call_stack.last().unwrap();
                    let frame_ptr = frame.frame_ptr;
                    let val = self
                        .stack
                        .get_local_at(frame_ptr, 2)
                        .unwrap_or(Value::undefined());
                    self.stack.push(val);
                }
                op if op == OpCode::GetArg3 as u8 => {
                    let frame = self.call_stack.last().unwrap();
                    let frame_ptr = frame.frame_ptr;
                    let val = self
                        .stack
                        .get_local_at(frame_ptr, 3)
                        .unwrap_or(Value::undefined());
                    self.stack.push(val);
                }

                // Set argument 0-3 (optimized)
                op if op == OpCode::PutArg0 as u8 => {
                    let val = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let frame = self.call_stack.last().unwrap();
                    let frame_ptr = frame.frame_ptr;
                    self.stack.set_local_at(frame_ptr, 0, val);
                }
                op if op == OpCode::PutArg1 as u8 => {
                    let val = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let frame = self.call_stack.last().unwrap();
                    let frame_ptr = frame.frame_ptr;
                    self.stack.set_local_at(frame_ptr, 1, val);
                }
                op if op == OpCode::PutArg2 as u8 => {
                    let val = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let frame = self.call_stack.last().unwrap();
                    let frame_ptr = frame.frame_ptr;
                    self.stack.set_local_at(frame_ptr, 2, val);
                }
                op if op == OpCode::PutArg3 as u8 => {
                    let val = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let frame = self.call_stack.last().unwrap();
                    let frame_ptr = frame.frame_ptr;
                    self.stack.set_local_at(frame_ptr, 3, val);
                }

                // Push this value
                op if op == OpCode::PushThis as u8 => {
                    let frame = self.call_stack.last().unwrap();
                    self.stack.push(frame.this_val);
                }

                // Push current function (for self-reference/recursion)
                op if op == OpCode::ThisFunc as u8 => {
                    let frame = self.call_stack.last().unwrap();
                    // Push the function index that created this frame
                    self.stack.push(frame.this_func);
                }

                // Arithmetic: Negate
                op if op == OpCode::Neg as u8 => {
                    let val = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let result = self.op_neg(val)?;
                    self.stack.push(result);
                }

                // Arithmetic: Add
                op if op == OpCode::Add as u8 => {
                    let b = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let a = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let result = self.op_add(a, b)?;
                    self.stack.push(result);
                }

                // Arithmetic: Subtract
                op if op == OpCode::Sub as u8 => {
                    let b = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let a = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let result = self.op_sub(a, b)?;
                    self.stack.push(result);
                }

                // Arithmetic: Multiply
                op if op == OpCode::Mul as u8 => {
                    let b = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let a = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let result = self.op_mul(a, b)?;
                    self.stack.push(result);
                }

                // Arithmetic: Divide
                op if op == OpCode::Div as u8 => {
                    let b = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let a = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let result = self.op_div(a, b)?;
                    self.stack.push(result);
                }

                // Arithmetic: Modulo
                op if op == OpCode::Mod as u8 => {
                    let b = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let a = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let result = self.op_mod(a, b)?;
                    self.stack.push(result);
                }

                // Comparison: Less than
                op if op == OpCode::Lt as u8 => {
                    let b = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let a = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let result = self.op_lt(a, b)?;
                    self.stack.push(result);
                }

                // Comparison: Less than or equal
                op if op == OpCode::Lte as u8 => {
                    let b = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let a = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let result = self.op_lte(a, b)?;
                    self.stack.push(result);
                }

                // Comparison: Greater than
                op if op == OpCode::Gt as u8 => {
                    let b = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let a = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let result = self.op_gt(a, b)?;
                    self.stack.push(result);
                }

                // Comparison: Greater than or equal
                op if op == OpCode::Gte as u8 => {
                    let b = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let a = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let result = self.op_gte(a, b)?;
                    self.stack.push(result);
                }

                // Comparison: Equal (==)
                op if op == OpCode::Eq as u8 => {
                    let b = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let a = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let result = self.op_eq(a, b)?;
                    self.stack.push(result);
                }

                // Comparison: Not equal (!=)
                op if op == OpCode::Neq as u8 => {
                    let b = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let a = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let result = self.op_neq(a, b)?;
                    self.stack.push(result);
                }

                // Comparison: Strict equal (===)
                op if op == OpCode::StrictEq as u8 => {
                    let b = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let a = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let result = Value::bool(a == b);
                    self.stack.push(result);
                }

                // Comparison: Strict not equal (!==)
                op if op == OpCode::StrictNeq as u8 => {
                    let b = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let a = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let result = Value::bool(a != b);
                    self.stack.push(result);
                }

                // Logical NOT
                op if op == OpCode::LNot as u8 => {
                    let val = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let result = Value::bool(!Self::value_to_bool(val));
                    self.stack.push(result);
                }

                // Bitwise NOT
                op if op == OpCode::Not as u8 => {
                    let val = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let result = self.op_bitwise_not(val)?;
                    self.stack.push(result);
                }

                // Bitwise AND
                op if op == OpCode::And as u8 => {
                    let b = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let a = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let result = self.op_bitwise_and(a, b)?;
                    self.stack.push(result);
                }

                // Bitwise OR
                op if op == OpCode::Or as u8 => {
                    let b = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let a = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let result = self.op_bitwise_or(a, b)?;
                    self.stack.push(result);
                }

                // Bitwise XOR
                op if op == OpCode::Xor as u8 => {
                    let b = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let a = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let result = self.op_bitwise_xor(a, b)?;
                    self.stack.push(result);
                }

                // Left shift
                op if op == OpCode::Shl as u8 => {
                    let b = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let a = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let result = self.op_shl(a, b)?;
                    self.stack.push(result);
                }

                // Arithmetic right shift
                op if op == OpCode::Sar as u8 => {
                    let b = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let a = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let result = self.op_sar(a, b)?;
                    self.stack.push(result);
                }

                // Logical right shift
                op if op == OpCode::Shr as u8 => {
                    let b = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let a = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let result = self.op_shr(a, b)?;
                    self.stack.push(result);
                }

                // Increment
                op if op == OpCode::Inc as u8 => {
                    let val = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let result = self.op_add(val, Value::int(1))?;
                    self.stack.push(result);
                }

                // Decrement
                op if op == OpCode::Dec as u8 => {
                    let val = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let result = self.op_sub(val, Value::int(1))?;
                    self.stack.push(result);
                }

                // Control flow: Goto
                op if op == OpCode::Goto as u8 => {
                    let frame = self.call_stack.last_mut().unwrap();
                    let bytecode = unsafe { &*frame.bytecode };
                    let bc = &bytecode.bytecode;
                    let offset = i32::from_le_bytes([
                        bc[frame.pc],
                        bc[frame.pc + 1],
                        bc[frame.pc + 2],
                        bc[frame.pc + 3],
                    ]);
                    frame.pc += 4;
                    // offset is relative to the end of this instruction (after the 4-byte offset)
                    frame.pc = (frame.pc as i32 + offset) as usize;
                }

                // Control flow: If false
                op if op == OpCode::IfFalse as u8 => {
                    let val = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let is_truthy = Self::value_to_bool(val);
                    let frame = self.call_stack.last_mut().unwrap();
                    let bytecode = unsafe { &*frame.bytecode };
                    let bc = &bytecode.bytecode;
                    let offset = i32::from_le_bytes([
                        bc[frame.pc],
                        bc[frame.pc + 1],
                        bc[frame.pc + 2],
                        bc[frame.pc + 3],
                    ]);
                    frame.pc += 4;
                    if !is_truthy {
                        // offset is relative to the end of this instruction (after the 4-byte offset)
                        frame.pc = (frame.pc as i32 + offset) as usize;
                    }
                }

                // Control flow: If true
                op if op == OpCode::IfTrue as u8 => {
                    let val = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let is_truthy = Self::value_to_bool(val);
                    let frame = self.call_stack.last_mut().unwrap();
                    let bytecode = unsafe { &*frame.bytecode };
                    let bc = &bytecode.bytecode;
                    let offset = i32::from_le_bytes([
                        bc[frame.pc],
                        bc[frame.pc + 1],
                        bc[frame.pc + 2],
                        bc[frame.pc + 3],
                    ]);
                    frame.pc += 4;
                    if is_truthy {
                        // offset is relative to the end of this instruction (after the 4-byte offset)
                        frame.pc = (frame.pc as i32 + offset) as usize;
                    }
                }

                // Return
                op if op == OpCode::Return as u8 => {
                    let result = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    return self.do_return(result);
                }

                // Return undefined
                op if op == OpCode::ReturnUndef as u8 => {
                    return self.do_return(Value::undefined());
                }

                // Function closure creation (16-bit function index)
                op if op == OpCode::FClosure as u8 => {
                    let frame = self.call_stack.last_mut().unwrap();
                    let bytecode = unsafe { &*frame.bytecode };
                    let bc = &bytecode.bytecode;
                    let func_idx = u16::from_le_bytes([bc[frame.pc], bc[frame.pc + 1]]) as usize;
                    frame.pc += 2;

                    // Get the inner function bytecode pointer
                    let inner_func = bytecode
                        .inner_functions
                        .get(func_idx)
                        .ok_or_else(|| {
                            InterpreterError::InternalError(format!(
                                "invalid function index in FClosure: {}",
                                func_idx
                            ))
                        })?;

                    // Push a function value with the bytecode pointer
                    self.stack.push(Value::func_ptr(inner_func as *const _));
                }

                // Function call (16-bit argc)
                op if op == OpCode::Call as u8 => {
                    let frame = self.call_stack.last_mut().unwrap();
                    let bytecode = unsafe { &*frame.bytecode };
                    let bc = &bytecode.bytecode;
                    let argc = u16::from_le_bytes([bc[frame.pc], bc[frame.pc + 1]]) as usize;
                    frame.pc += 2;

                    // Collect arguments (they were pushed in order)
                    let mut args = Vec::with_capacity(argc);
                    for _ in 0..argc {
                        args.push(self.stack.pop().ok_or(InterpreterError::StackUnderflow)?);
                    }
                    args.reverse(); // Arguments were pushed left-to-right

                    // Pop the function value
                    let func_val = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;

                    // Get the function bytecode - either from pointer or index
                    let callee_bytecode: &FunctionBytecode = if let Some(ptr) = func_val.to_func_ptr() {
                        // Pointer-based function (from FClosure or ThisFunc)
                        unsafe { &*ptr }
                    } else if let Some(idx) = func_val.to_func_idx() {
                        // Index-based function (legacy, shouldn't happen anymore)
                        bytecode
                            .inner_functions
                            .get(idx as usize)
                            .ok_or_else(|| {
                                InterpreterError::InternalError(format!(
                                    "invalid function index: {}",
                                    idx
                                ))
                            })?
                    } else {
                        return Err(InterpreterError::TypeError("not a function".to_string()));
                    };

                    // Check recursion limit
                    if self.call_stack.len() >= self.max_recursion {
                        return Err(InterpreterError::InternalError(
                            "maximum call stack size exceeded".to_string(),
                        ));
                    }

                    let callee_frame_ptr = self.stack.len();

                    // Push arguments (pad with undefined if needed)
                    for i in 0..callee_bytecode.arg_count as usize {
                        let arg = args.get(i).copied().unwrap_or(Value::undefined());
                        self.stack.push(arg);
                    }

                    // Allocate space for locals (beyond arguments)
                    let extra_locals = callee_bytecode.local_count.saturating_sub(callee_bytecode.arg_count);
                    for _ in 0..extra_locals {
                        self.stack.push(Value::undefined());
                    }

                    let callee_frame = CallFrame::new(
                        callee_bytecode as *const _,
                        callee_frame_ptr,
                        args.len().min(u16::MAX as usize) as u16,
                        Value::undefined(), // this value
                        func_val,           // the function value for self-reference
                    );
                    self.call_stack.push(callee_frame);

                    // Continue execution in the new frame (run loop will pick it up)
                }

                // Nop
                op if op == OpCode::Nop as u8 => {
                    // Do nothing
                }

                // Unknown opcode
                op => {
                    return Err(InterpreterError::InvalidOpcode(op));
                }
            }
        }
    }

    /// Handle return from a function
    ///
    /// If this is a nested call, pops the frame and continues execution.
    /// If this is the top-level call, returns the result.
    fn do_return(&mut self, result: Value) -> InterpreterResult<Value> {
        // Pop the current frame
        let frame = self.call_stack.pop().ok_or_else(|| {
            InterpreterError::InternalError("no call frame to return from".to_string())
        })?;

        // Clean up locals from the stack
        let local_count = unsafe { (*frame.bytecode).local_count } as usize;
        self.stack.drop_n(local_count);

        // If there are no more frames, this is the final result
        if self.call_stack.is_empty() {
            return Ok(result);
        }

        // Otherwise, push the result for the caller and continue
        self.stack.push(result);

        // Continue running the caller
        self.run()
    }

    // Helper: Convert value to boolean (static method to avoid borrow issues)
    fn value_to_bool(val: Value) -> bool {
        if val.is_bool() {
            val.to_bool().unwrap_or(false)
        } else if val.is_int() {
            val.to_i32().map(|n| n != 0).unwrap_or(false)
        } else if val.is_null() || val.is_undefined() {
            false
        } else {
            // Objects are truthy
            true
        }
    }

    // Arithmetic operations

    fn op_neg(&self, val: Value) -> InterpreterResult<Value> {
        if let Some(n) = val.to_i32() {
            if n == i32::MIN {
                // Overflow: would need f64
                return Err(InterpreterError::InternalError(
                    "integer overflow".to_string(),
                ));
            }
            Ok(Value::int(-n))
        } else {
            Err(InterpreterError::TypeError(
                "cannot negate non-number".to_string(),
            ))
        }
    }

    fn op_add(&self, a: Value, b: Value) -> InterpreterResult<Value> {
        match (a.to_i32(), b.to_i32()) {
            (Some(va), Some(vb)) => {
                if let Some(result) = va.checked_add(vb) {
                    Ok(Value::int(result))
                } else {
                    Err(InterpreterError::InternalError(
                        "integer overflow".to_string(),
                    ))
                }
            }
            _ => Err(InterpreterError::TypeError(
                "cannot add non-numbers".to_string(),
            )),
        }
    }

    fn op_sub(&self, a: Value, b: Value) -> InterpreterResult<Value> {
        match (a.to_i32(), b.to_i32()) {
            (Some(va), Some(vb)) => {
                if let Some(result) = va.checked_sub(vb) {
                    Ok(Value::int(result))
                } else {
                    Err(InterpreterError::InternalError(
                        "integer overflow".to_string(),
                    ))
                }
            }
            _ => Err(InterpreterError::TypeError(
                "cannot subtract non-numbers".to_string(),
            )),
        }
    }

    fn op_mul(&self, a: Value, b: Value) -> InterpreterResult<Value> {
        match (a.to_i32(), b.to_i32()) {
            (Some(va), Some(vb)) => {
                if let Some(result) = va.checked_mul(vb) {
                    Ok(Value::int(result))
                } else {
                    Err(InterpreterError::InternalError(
                        "integer overflow".to_string(),
                    ))
                }
            }
            _ => Err(InterpreterError::TypeError(
                "cannot multiply non-numbers".to_string(),
            )),
        }
    }

    fn op_div(&self, a: Value, b: Value) -> InterpreterResult<Value> {
        match (a.to_i32(), b.to_i32()) {
            (Some(va), Some(vb)) => {
                if vb == 0 {
                    Err(InterpreterError::DivisionByZero)
                } else if let Some(result) = va.checked_div(vb) {
                    Ok(Value::int(result))
                } else {
                    Err(InterpreterError::InternalError(
                        "integer overflow".to_string(),
                    ))
                }
            }
            _ => Err(InterpreterError::TypeError(
                "cannot divide non-numbers".to_string(),
            )),
        }
    }

    fn op_mod(&self, a: Value, b: Value) -> InterpreterResult<Value> {
        match (a.to_i32(), b.to_i32()) {
            (Some(va), Some(vb)) => {
                if vb == 0 {
                    Err(InterpreterError::DivisionByZero)
                } else if let Some(result) = va.checked_rem(vb) {
                    Ok(Value::int(result))
                } else {
                    Err(InterpreterError::InternalError(
                        "integer overflow".to_string(),
                    ))
                }
            }
            _ => Err(InterpreterError::TypeError(
                "cannot modulo non-numbers".to_string(),
            )),
        }
    }

    // Comparison operations

    fn op_lt(&self, a: Value, b: Value) -> InterpreterResult<Value> {
        match (a.to_i32(), b.to_i32()) {
            (Some(va), Some(vb)) => Ok(Value::bool(va < vb)),
            _ => Err(InterpreterError::TypeError(
                "cannot compare non-numbers".to_string(),
            )),
        }
    }

    fn op_lte(&self, a: Value, b: Value) -> InterpreterResult<Value> {
        match (a.to_i32(), b.to_i32()) {
            (Some(va), Some(vb)) => Ok(Value::bool(va <= vb)),
            _ => Err(InterpreterError::TypeError(
                "cannot compare non-numbers".to_string(),
            )),
        }
    }

    fn op_gt(&self, a: Value, b: Value) -> InterpreterResult<Value> {
        match (a.to_i32(), b.to_i32()) {
            (Some(va), Some(vb)) => Ok(Value::bool(va > vb)),
            _ => Err(InterpreterError::TypeError(
                "cannot compare non-numbers".to_string(),
            )),
        }
    }

    fn op_gte(&self, a: Value, b: Value) -> InterpreterResult<Value> {
        match (a.to_i32(), b.to_i32()) {
            (Some(va), Some(vb)) => Ok(Value::bool(va >= vb)),
            _ => Err(InterpreterError::TypeError(
                "cannot compare non-numbers".to_string(),
            )),
        }
    }

    fn op_eq(&self, a: Value, b: Value) -> InterpreterResult<Value> {
        // Simple equality for now (strict equality)
        Ok(Value::bool(a == b))
    }

    fn op_neq(&self, a: Value, b: Value) -> InterpreterResult<Value> {
        Ok(Value::bool(a != b))
    }

    // Bitwise operations

    fn op_bitwise_not(&self, val: Value) -> InterpreterResult<Value> {
        if let Some(n) = val.to_i32() {
            Ok(Value::int(!n))
        } else {
            Err(InterpreterError::TypeError(
                "cannot apply bitwise NOT to non-number".to_string(),
            ))
        }
    }

    fn op_bitwise_and(&self, a: Value, b: Value) -> InterpreterResult<Value> {
        match (a.to_i32(), b.to_i32()) {
            (Some(va), Some(vb)) => Ok(Value::int(va & vb)),
            _ => Err(InterpreterError::TypeError(
                "cannot apply bitwise AND to non-numbers".to_string(),
            )),
        }
    }

    fn op_bitwise_or(&self, a: Value, b: Value) -> InterpreterResult<Value> {
        match (a.to_i32(), b.to_i32()) {
            (Some(va), Some(vb)) => Ok(Value::int(va | vb)),
            _ => Err(InterpreterError::TypeError(
                "cannot apply bitwise OR to non-numbers".to_string(),
            )),
        }
    }

    fn op_bitwise_xor(&self, a: Value, b: Value) -> InterpreterResult<Value> {
        match (a.to_i32(), b.to_i32()) {
            (Some(va), Some(vb)) => Ok(Value::int(va ^ vb)),
            _ => Err(InterpreterError::TypeError(
                "cannot apply bitwise XOR to non-numbers".to_string(),
            )),
        }
    }

    fn op_shl(&self, a: Value, b: Value) -> InterpreterResult<Value> {
        match (a.to_i32(), b.to_i32()) {
            (Some(va), Some(vb)) => {
                let shift = (vb & 0x1f) as u32;
                Ok(Value::int(va << shift))
            }
            _ => Err(InterpreterError::TypeError(
                "cannot apply left shift to non-numbers".to_string(),
            )),
        }
    }

    fn op_sar(&self, a: Value, b: Value) -> InterpreterResult<Value> {
        match (a.to_i32(), b.to_i32()) {
            (Some(va), Some(vb)) => {
                let shift = (vb & 0x1f) as u32;
                Ok(Value::int(va >> shift))
            }
            _ => Err(InterpreterError::TypeError(
                "cannot apply arithmetic right shift to non-numbers".to_string(),
            )),
        }
    }

    fn op_shr(&self, a: Value, b: Value) -> InterpreterResult<Value> {
        match (a.to_i32(), b.to_i32()) {
            (Some(va), Some(vb)) => {
                let shift = (vb & 0x1f) as u32;
                let result = (va as u32) >> shift;
                Ok(Value::int(result as i32))
            }
            _ => Err(InterpreterError::TypeError(
                "cannot apply logical right shift to non-numbers".to_string(),
            )),
        }
    }
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bytecode(bytecode: Vec<u8>) -> FunctionBytecode {
        let mut fb = FunctionBytecode::new(0, 4);
        fb.bytecode = bytecode;
        fb
    }

    #[test]
    fn test_push_integers() {
        let mut interp = Interpreter::new();

        // Push 3, Push 2, Add, Return
        let bc = make_bytecode(vec![
            OpCode::Push3 as u8,
            OpCode::Push2 as u8,
            OpCode::Add as u8,
            OpCode::Return as u8,
        ]);

        let result = interp.execute(&bc).unwrap();
        assert_eq!(result.to_i32(), Some(5));
    }

    #[test]
    fn test_push_i8() {
        let mut interp = Interpreter::new();

        // PushI8 10, PushI8 -5, Add, Return
        let bc = make_bytecode(vec![
            OpCode::PushI8 as u8,
            10u8,
            OpCode::PushI8 as u8,
            (-5i8) as u8,
            OpCode::Add as u8,
            OpCode::Return as u8,
        ]);

        let result = interp.execute(&bc).unwrap();
        assert_eq!(result.to_i32(), Some(5));
    }

    #[test]
    fn test_arithmetic() {
        let mut interp = Interpreter::new();

        // 10 - 3 * 2 = 4 (but we do it manually: push 10, push 3, push 2, mul, sub)
        let bc = make_bytecode(vec![
            OpCode::PushI8 as u8,
            10,
            OpCode::Push3 as u8,
            OpCode::Push2 as u8,
            OpCode::Mul as u8,
            OpCode::Sub as u8,
            OpCode::Return as u8,
        ]);

        let result = interp.execute(&bc).unwrap();
        assert_eq!(result.to_i32(), Some(4));
    }

    #[test]
    fn test_local_variables() {
        let mut interp = Interpreter::new();

        // var x = 5; var y = 3; return x + y;
        // PutLoc0 5, PutLoc1 3, GetLoc0, GetLoc1, Add, Return
        let bc = make_bytecode(vec![
            OpCode::Push5 as u8,
            OpCode::PutLoc0 as u8,
            OpCode::Push3 as u8,
            OpCode::PutLoc1 as u8,
            OpCode::GetLoc0 as u8,
            OpCode::GetLoc1 as u8,
            OpCode::Add as u8,
            OpCode::Return as u8,
        ]);

        let result = interp.execute(&bc).unwrap();
        assert_eq!(result.to_i32(), Some(8));
    }

    #[test]
    fn test_comparison() {
        let mut interp = Interpreter::new();

        // 5 < 10 => true
        let bc = make_bytecode(vec![
            OpCode::Push5 as u8,
            OpCode::PushI8 as u8,
            10,
            OpCode::Lt as u8,
            OpCode::Return as u8,
        ]);

        let result = interp.execute(&bc).unwrap();
        assert!(result.to_bool().unwrap());
    }

    #[test]
    fn test_conditional_jump() {
        let mut interp = Interpreter::new();

        // if (false) { return 1; } return 2;
        // Layout:
        // 0: PushFalse
        // 1: IfFalse (5 bytes: opcode + 4-byte offset)
        // 2-5: offset (4 bytes)
        // 6: Push1
        // 7: Return
        // 8: Push2
        // 9: Return
        //
        // When IfFalse executes:
        // - pc is at 2 (pointing to offset bytes)
        // - we read offset, pc becomes 6
        // - if condition is false, pc = 6 + offset should go to 8 (Push2)
        // - so offset = 2
        let bc = make_bytecode(vec![
            OpCode::PushFalse as u8,     // 0
            OpCode::IfFalse as u8,       // 1
            2, 0, 0, 0,                  // 2-5: offset = 2
            OpCode::Push1 as u8,         // 6
            OpCode::Return as u8,        // 7
            OpCode::Push2 as u8,         // 8
            OpCode::Return as u8,        // 9
        ]);

        let result = interp.execute(&bc).unwrap();
        assert_eq!(result.to_i32(), Some(2));
    }

    #[test]
    fn test_bitwise_operations() {
        let mut interp = Interpreter::new();

        // 5 & 3 = 1
        let bc = make_bytecode(vec![
            OpCode::Push5 as u8,
            OpCode::Push3 as u8,
            OpCode::And as u8,
            OpCode::Return as u8,
        ]);

        let result = interp.execute(&bc).unwrap();
        assert_eq!(result.to_i32(), Some(1));
    }

    #[test]
    fn test_return_undefined() {
        let mut interp = Interpreter::new();

        let bc = make_bytecode(vec![OpCode::ReturnUndef as u8]);

        let result = interp.execute(&bc).unwrap();
        assert!(result.is_undefined());
    }

    #[test]
    fn test_logical_not() {
        let mut interp = Interpreter::new();

        // !false = true
        let bc = make_bytecode(vec![
            OpCode::PushFalse as u8,
            OpCode::LNot as u8,
            OpCode::Return as u8,
        ]);

        let result = interp.execute(&bc).unwrap();
        assert!(result.to_bool().unwrap());
    }

    #[test]
    fn test_function_with_args() {
        let mut interp = Interpreter::new();

        // function add(a, b) { return a + b; }
        // Called with args [10, 20]
        let mut fb = FunctionBytecode::new(2, 2); // 2 args, 2 locals (args are locals)
        fb.bytecode = vec![
            OpCode::GetArg0 as u8,
            OpCode::GetArg1 as u8,
            OpCode::Add as u8,
            OpCode::Return as u8,
        ];

        let result = interp
            .call_function(&fb, Value::undefined(), &[Value::int(10), Value::int(20)])
            .unwrap();
        assert_eq!(result.to_i32(), Some(30));
    }

    #[test]
    fn test_function_with_this() {
        let mut interp = Interpreter::new();

        // function getThis() { return this; }
        let mut fb = FunctionBytecode::new(0, 0);
        fb.bytecode = vec![OpCode::PushThis as u8, OpCode::Return as u8];

        let this_val = Value::int(42);
        let result = interp.call_function(&fb, this_val, &[]).unwrap();
        assert_eq!(result.to_i32(), Some(42));
    }

    #[test]
    fn test_function_missing_args() {
        let mut interp = Interpreter::new();

        // function add(a, b) { return a + b; }
        // Called with only 1 arg - b should be undefined
        let mut fb = FunctionBytecode::new(2, 2);
        fb.bytecode = vec![
            OpCode::GetArg1 as u8, // Get b (should be undefined)
            OpCode::Return as u8,
        ];

        let result = interp
            .call_function(&fb, Value::undefined(), &[Value::int(10)])
            .unwrap();
        assert!(result.is_undefined());
    }

    #[test]
    fn test_recursion_limit() {
        let mut interp = Interpreter::with_config(1024, 5); // Max 5 calls deep

        // Fill up call stack
        let fb = FunctionBytecode::new(0, 0);
        for _ in 0..5 {
            interp.call_stack.push(CallFrame::new(
                &fb as *const _,
                0,
                0,
                Value::undefined(),
                Value::undefined(),
            ));
        }

        // Next call should fail
        let result = interp.call_function(&fb, Value::undefined(), &[]);
        assert!(result.is_err());
    }
}
