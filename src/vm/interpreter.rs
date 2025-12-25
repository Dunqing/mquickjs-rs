//! Bytecode interpreter
//!
//! Executes JavaScript bytecode using a stack-based virtual machine.

use crate::runtime::FunctionBytecode;
use crate::value::Value;
use crate::vm::opcode::OpCode;
use crate::vm::stack::Stack;

// Builtin object indices
/// Math object index
pub const BUILTIN_MATH: u32 = 0;
/// JSON object index (for future use)
pub const BUILTIN_JSON: u32 = 1;
/// Number object index
pub const BUILTIN_NUMBER: u32 = 2;
/// Boolean object index
pub const BUILTIN_BOOLEAN: u32 = 3;
/// console object index
pub const BUILTIN_CONSOLE: u32 = 4;
/// Error constructor index
pub const BUILTIN_ERROR: u32 = 5;
/// TypeError constructor index
pub const BUILTIN_TYPE_ERROR: u32 = 6;
/// ReferenceError constructor index
pub const BUILTIN_REFERENCE_ERROR: u32 = 7;
/// SyntaxError constructor index
pub const BUILTIN_SYNTAX_ERROR: u32 = 8;
/// RangeError constructor index
pub const BUILTIN_RANGE_ERROR: u32 = 9;
/// Date object index
pub const BUILTIN_DATE: u32 = 10;
/// String object index
pub const BUILTIN_STRING: u32 = 11;
/// Object object index
pub const BUILTIN_OBJECT: u32 = 12;
/// Array object index
pub const BUILTIN_ARRAY: u32 = 13;
/// RegExp object index
pub const BUILTIN_REGEXP: u32 = 14;
/// Map object index
pub const BUILTIN_MAP: u32 = 15;
/// Set object index
pub const BUILTIN_SET: u32 = 16;
/// globalThis object index
pub const BUILTIN_GLOBAL_THIS: u32 = 17;

/// Native function signature
///
/// Native functions take an interpreter reference, this value, and arguments.
/// Returns a Result with the value or an error message.
pub type NativeFn = fn(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String>;

/// Native function entry in the registry
#[derive(Clone)]
pub struct NativeFunction {
    /// The name of the function
    pub name: &'static str,
    /// The native function implementation
    pub func: NativeFn,
    /// Number of expected arguments (for arity checking, 0 = variadic)
    pub arity: u8,
}

/// Object instance storing properties and constructor reference
#[derive(Debug, Clone)]
pub struct ObjectInstance {
    /// Constructor that created this object (closure index), if any
    pub constructor: Option<Value>,
    /// Object properties as key-value pairs
    pub properties: Vec<(String, Value)>,
    /// Whether the object is frozen (non-extensible, properties non-writable/non-configurable)
    pub frozen: bool,
    /// Whether the object is sealed (non-extensible, properties non-configurable)
    pub sealed: bool,
}

impl ObjectInstance {
    /// Create a new empty object
    pub fn new() -> Self {
        ObjectInstance {
            constructor: None,
            properties: Vec::new(),
            frozen: false,
            sealed: false,
        }
    }

    /// Create a new object with a constructor reference
    pub fn with_constructor(constructor: Value) -> Self {
        ObjectInstance {
            constructor: Some(constructor),
            properties: Vec::new(),
            frozen: false,
            sealed: false,
        }
    }
}

/// For-in iterator state
#[derive(Debug, Clone)]
pub struct ForInIterator {
    /// Keys to iterate over
    pub keys: Vec<String>,
    /// Current index in keys array
    pub index: usize,
}

impl ForInIterator {
    /// Create a new for-in iterator from an object
    pub fn from_object(obj: &ObjectInstance) -> Self {
        let keys = obj.properties.iter().map(|(k, _)| k.clone()).collect();
        ForInIterator { keys, index: 0 }
    }

    /// Create a new for-in iterator from an array
    pub fn from_array(arr: &[Value]) -> Self {
        let keys = (0..arr.len()).map(|i| i.to_string()).collect();
        ForInIterator { keys, index: 0 }
    }

    /// Get the next key, or None if done
    pub fn next(&mut self) -> Option<String> {
        if self.index < self.keys.len() {
            let key = self.keys[self.index].clone();
            self.index += 1;
            Some(key)
        } else {
            None
        }
    }

    /// Check if iteration is done
    pub fn is_done(&self) -> bool {
        self.index >= self.keys.len()
    }
}

/// For-of iterator state (iterates over values)
#[derive(Debug, Clone)]
pub struct ForOfIterator {
    /// Values to iterate over
    pub values: Vec<Value>,
    /// Current index in values array
    pub index: usize,
}

impl ForOfIterator {
    /// Create a new for-of iterator from an array
    pub fn from_array(arr: &[Value]) -> Self {
        ForOfIterator {
            values: arr.to_vec(),
            index: 0,
        }
    }

    /// Create a new for-of iterator from an object (iterates over property values)
    pub fn from_object(obj: &ObjectInstance) -> Self {
        let values = obj.properties.iter().map(|(_, v)| *v).collect();
        ForOfIterator { values, index: 0 }
    }

    /// Get the next value, or None if done
    pub fn next(&mut self) -> Option<Value> {
        if self.index < self.values.len() {
            let val = self.values[self.index];
            self.index += 1;
            Some(val)
        } else {
            None
        }
    }
}

/// Closure data storing captured variable values
#[derive(Debug, Clone)]
pub struct ClosureData {
    /// Reference to the function bytecode
    pub bytecode: *const FunctionBytecode,
    /// Captured variable values
    pub var_refs: Vec<Value>,
}

impl ClosureData {
    /// Create a new closure with captured values
    pub fn new(bytecode: *const FunctionBytecode, var_refs: Vec<Value>) -> Self {
        ClosureData { bytecode, var_refs }
    }

    /// Get a captured variable value
    pub fn get_var(&self, index: usize) -> Option<Value> {
        self.var_refs.get(index).copied()
    }

    /// Set a captured variable value
    pub fn set_var(&mut self, index: usize, value: Value) {
        if index < self.var_refs.len() {
            self.var_refs[index] = value;
        }
    }
}

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
    /// Index into closures array if this frame is executing a closure
    pub closure_idx: Option<usize>,
    /// Whether this is a constructor call (new operator)
    pub is_constructor: bool,
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
            closure_idx: None,
            is_constructor: false,
        }
    }

    /// Create a call frame for a closure
    pub fn new_closure(
        bytecode: *const FunctionBytecode,
        frame_ptr: usize,
        arg_count: u16,
        this_val: Value,
        this_func: Value,
        closure_idx: usize,
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
            closure_idx: Some(closure_idx),
            is_constructor: false,
        }
    }

    /// Create a call frame for a constructor
    pub fn new_constructor(
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
            closure_idx: None,
            is_constructor: true,
        }
    }

    /// Create a call frame for a closure used as constructor
    pub fn new_closure_constructor(
        bytecode: *const FunctionBytecode,
        frame_ptr: usize,
        arg_count: u16,
        this_val: Value,
        this_func: Value,
        closure_idx: usize,
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
            closure_idx: Some(closure_idx),
            is_constructor: true,
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

/// Exception handler info
#[derive(Debug, Clone)]
pub struct ExceptionHandler {
    /// Call stack depth when handler was registered
    pub frame_depth: usize,
    /// Program counter to jump to when exception is caught
    pub catch_pc: usize,
    /// Stack depth when handler was registered (to restore stack)
    pub stack_depth: usize,
}

/// Interpreter state
pub struct Interpreter {
    /// Value stack
    stack: Stack,
    /// Call stack (frames)
    call_stack: Vec<CallFrame>,
    /// Maximum call recursion depth
    max_recursion: usize,
    /// Runtime strings (created during execution, e.g., from concatenation)
    /// Indices start from 0x8000 to distinguish from compile-time strings
    runtime_strings: Vec<String>,
    /// Closures created during execution
    /// Values on the stack can reference closures by index
    closures: Vec<ClosureData>,
    /// Exception handler stack
    exception_handlers: Vec<ExceptionHandler>,
    /// Arrays created during execution
    /// Values on the stack can reference arrays by index
    arrays: Vec<Vec<Value>>,
    /// Objects created during execution
    /// Values on the stack can reference objects by index
    objects: Vec<ObjectInstance>,
    /// For-in iterators created during execution
    for_in_iterators: Vec<ForInIterator>,
    /// For-of iterators created during execution
    for_of_iterators: Vec<ForOfIterator>,
    /// Native function registry
    native_functions: Vec<NativeFunction>,
    /// Error objects created during execution
    /// Stores (error_type, message) pairs
    error_objects: Vec<ErrorObject>,
    /// RegExp objects created during execution
    regex_objects: Vec<RegExpObject>,
    /// Map objects created during execution
    map_objects: Vec<MapObject>,
    /// Set objects created during execution
    set_objects: Vec<SetObject>,
    /// Current compile-time string constants (set during bytecode execution)
    /// Used by native functions to look up compile-time strings
    current_string_constants: Option<*const Vec<String>>,
    /// Target call stack depth for nested call_value invocations
    /// When set, do_return will return early when reaching this depth
    nested_call_target_depth: Option<usize>,
}

/// Error object storage
#[derive(Debug, Clone)]
pub struct ErrorObject {
    /// Error type name (e.g., "Error", "TypeError")
    pub name: String,
    /// Error message
    pub message: String,
}

/// RegExp object storage
#[derive(Clone)]
pub struct RegExpObject {
    /// The compiled regex pattern
    pub regex: regex::Regex,
    /// Original pattern string
    pub pattern: String,
    /// Flags string (e.g., "gi")
    pub flags: String,
    /// Global flag
    pub global: bool,
    /// Case-insensitive flag
    pub ignore_case: bool,
    /// Multiline flag
    pub multiline: bool,
}

impl std::fmt::Debug for RegExpObject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegExpObject")
            .field("pattern", &self.pattern)
            .field("flags", &self.flags)
            .finish()
    }
}

/// Map object storage - stores key-value pairs with insertion order
#[derive(Debug, Clone)]
pub struct MapObject {
    /// Entries stored as (key, value) pairs in insertion order
    /// We use a simple Vec to maintain insertion order
    pub entries: Vec<(Value, Value)>,
}

impl MapObject {
    pub fn new() -> Self {
        MapObject { entries: Vec::new() }
    }

    /// Get a value by key (using strict equality comparison)
    pub fn get(&self, key: Value) -> Option<Value> {
        for (k, v) in &self.entries {
            if Self::values_equal(*k, key) {
                return Some(*v);
            }
        }
        None
    }

    /// Set a key-value pair
    pub fn set(&mut self, key: Value, value: Value) {
        // Check if key exists
        for (k, v) in &mut self.entries {
            if Self::values_equal(*k, key) {
                *v = value;
                return;
            }
        }
        // Key doesn't exist, add new entry
        self.entries.push((key, value));
    }

    /// Check if key exists
    pub fn has(&self, key: Value) -> bool {
        self.entries.iter().any(|(k, _)| Self::values_equal(*k, key))
    }

    /// Delete a key
    pub fn delete(&mut self, key: Value) -> bool {
        let initial_len = self.entries.len();
        self.entries.retain(|(k, _)| !Self::values_equal(*k, key));
        self.entries.len() < initial_len
    }

    /// Get size
    pub fn size(&self) -> usize {
        self.entries.len()
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Simple value equality check
    fn values_equal(a: Value, b: Value) -> bool {
        a.0 == b.0 || (a.to_i32().is_some() && a.to_i32() == b.to_i32())
    }
}

/// Set object storage - stores unique values in insertion order
#[derive(Debug, Clone)]
pub struct SetObject {
    /// Values stored in insertion order
    pub values: Vec<Value>,
}

impl SetObject {
    pub fn new() -> Self {
        SetObject { values: Vec::new() }
    }

    /// Add a value (no-op if already exists)
    pub fn add(&mut self, value: Value) {
        if !self.has(value) {
            self.values.push(value);
        }
    }

    /// Check if value exists
    pub fn has(&self, value: Value) -> bool {
        self.values.iter().any(|v| Self::values_equal(*v, value))
    }

    /// Delete a value
    pub fn delete(&mut self, value: Value) -> bool {
        let initial_len = self.values.len();
        self.values.retain(|v| !Self::values_equal(*v, value));
        self.values.len() < initial_len
    }

    /// Get size
    pub fn size(&self) -> usize {
        self.values.len()
    }

    /// Clear all values
    pub fn clear(&mut self) {
        self.values.clear();
    }

    /// Simple value equality check
    fn values_equal(a: Value, b: Value) -> bool {
        a.0 == b.0 || (a.to_i32().is_some() && a.to_i32() == b.to_i32())
    }
}

impl Interpreter {
    /// Default stack capacity
    const DEFAULT_STACK_SIZE: usize = 1024;
    /// Default max recursion
    const DEFAULT_MAX_RECURSION: usize = 512;

    /// Create a new interpreter
    pub fn new() -> Self {
        let mut interp = Interpreter {
            stack: Stack::new(Self::DEFAULT_STACK_SIZE),
            call_stack: Vec::with_capacity(64),
            max_recursion: Self::DEFAULT_MAX_RECURSION,
            runtime_strings: Vec::new(),
            closures: Vec::new(),
            exception_handlers: Vec::new(),
            arrays: Vec::new(),
            objects: Vec::new(),
            for_in_iterators: Vec::new(),
            for_of_iterators: Vec::new(),
            native_functions: Vec::new(),
            error_objects: Vec::new(),
            regex_objects: Vec::new(),
            map_objects: Vec::new(),
            set_objects: Vec::new(),
            current_string_constants: None,
            nested_call_target_depth: None,
        };
        interp.register_builtins();
        interp
    }

    /// Create an interpreter with custom settings
    pub fn with_config(stack_size: usize, max_recursion: usize) -> Self {
        let mut interp = Interpreter {
            stack: Stack::new(stack_size),
            call_stack: Vec::with_capacity(64),
            max_recursion,
            runtime_strings: Vec::new(),
            closures: Vec::new(),
            exception_handlers: Vec::new(),
            arrays: Vec::new(),
            objects: Vec::new(),
            for_in_iterators: Vec::new(),
            for_of_iterators: Vec::new(),
            native_functions: Vec::new(),
            error_objects: Vec::new(),
            regex_objects: Vec::new(),
            map_objects: Vec::new(),
            set_objects: Vec::new(),
            current_string_constants: None,
            nested_call_target_depth: None,
        };
        interp.register_builtins();
        interp
    }

    /// Closure index marker (indices into closures vec are stored as negative values)
    const CLOSURE_INDEX_MARKER: u32 = 0x8000_0000;

    /// Runtime string index offset (indices >= this are runtime strings)
    const RUNTIME_STRING_OFFSET: u16 = 0x8000;

    /// Get string content from a string value
    fn get_string_content<'a>(&'a self, val: Value, bytecode: &'a FunctionBytecode) -> Option<&'a str> {
        if !val.is_string() {
            return None;
        }
        let idx = val.to_string_idx()?;

        // Check if it's a built-in string
        if let Some(s) = crate::value::get_builtin_string(idx) {
            return Some(s);
        }

        // Check if it's a runtime string
        if idx >= Self::RUNTIME_STRING_OFFSET {
            let runtime_idx = (idx - Self::RUNTIME_STRING_OFFSET) as usize;
            return self.runtime_strings.get(runtime_idx).map(|s| s.as_str());
        }

        // Otherwise it's a compile-time string
        bytecode.string_constants.get(idx as usize).map(|s| s.as_str())
    }

    /// Create a runtime string and return its Value
    fn create_runtime_string(&mut self, s: String) -> Value {
        let idx = self.runtime_strings.len();
        self.runtime_strings.push(s);
        Value::string(Self::RUNTIME_STRING_OFFSET + idx as u16)
    }

    /// Get a string by its index (works for both compile-time and runtime strings)
    /// For compile-time strings, uses current_string_constants if set.
    pub fn get_string_by_idx(&self, str_idx: u16) -> Option<&str> {
        if str_idx >= Self::RUNTIME_STRING_OFFSET {
            let runtime_idx = (str_idx - Self::RUNTIME_STRING_OFFSET) as usize;
            self.runtime_strings.get(runtime_idx).map(|s| s.as_str())
        } else {
            // Compile-time string - use current_string_constants if available
            if let Some(constants_ptr) = self.current_string_constants {
                // SAFETY: The pointer is valid during bytecode execution
                let constants = unsafe { &*constants_ptr };
                constants.get(str_idx as usize).map(|s| s.as_str())
            } else {
                None
            }
        }
    }

    /// Create a closure and return a Value that references it
    fn create_closure(&mut self, bytecode: *const FunctionBytecode, var_refs: Vec<Value>) -> Value {
        let idx = self.closures.len();
        self.closures.push(ClosureData::new(bytecode, var_refs));
        // Use high bit to mark as closure index
        Value::closure_idx(idx as u32)
    }

    /// Get a closure by index
    fn get_closure(&self, idx: u32) -> Option<&ClosureData> {
        self.closures.get(idx as usize)
    }

    /// Create an array and return a Value that references it
    fn create_array(&mut self, elements: Vec<Value>) -> Value {
        let idx = self.arrays.len();
        self.arrays.push(elements);
        Value::array_idx(idx as u32)
    }

    /// Get an array by index
    fn get_array(&self, idx: u32) -> Option<&Vec<Value>> {
        self.arrays.get(idx as usize)
    }

    /// Get a mutable array by index
    fn get_array_mut(&mut self, idx: u32) -> Option<&mut Vec<Value>> {
        self.arrays.get_mut(idx as usize)
    }

    /// Create a new object and return its value
    fn create_object(&mut self) -> Value {
        let idx = self.objects.len();
        self.objects.push(ObjectInstance::new());
        Value::object_idx(idx as u32)
    }

    /// Create a new object with a constructor reference and return its value
    fn create_object_with_constructor(&mut self, constructor: Value) -> Value {
        let idx = self.objects.len();
        self.objects.push(ObjectInstance::with_constructor(constructor));
        Value::object_idx(idx as u32)
    }

    /// Get an object by index
    fn get_object(&self, idx: u32) -> Option<&ObjectInstance> {
        self.objects.get(idx as usize)
    }

    /// Get a mutable object by index
    fn get_object_mut(&mut self, idx: u32) -> Option<&mut ObjectInstance> {
        self.objects.get_mut(idx as usize)
    }

    /// Get a property from an object
    fn object_get_property(&self, obj_idx: u32, key: &str) -> Value {
        if let Some(obj) = self.get_object(obj_idx) {
            for (k, v) in obj.properties.iter() {
                if k == key {
                    return *v;
                }
            }
        }
        Value::undefined()
    }

    /// Set a property on an object
    fn object_set_property(&mut self, obj_idx: u32, key: String, value: Value) {
        if let Some(obj) = self.get_object_mut(obj_idx) {
            // Check if property already exists
            for (k, v) in obj.properties.iter_mut() {
                if k == &key {
                    *v = value;
                    return;
                }
            }
            // Add new property
            obj.properties.push((key, value));
        }
    }

    /// Get a mutable closure by index
    fn get_closure_mut(&mut self, idx: u32) -> Option<&mut ClosureData> {
        self.closures.get_mut(idx as usize)
    }

    /// Call a function value with the given `this` value and arguments
    ///
    /// This handles closures, function pointers, and function indices.
    pub fn call_value(
        &mut self,
        func: Value,
        this_val: Value,
        args: &[Value],
    ) -> InterpreterResult<Value> {
        // Save current call stack depth to return when we're back to this level
        let saved_target = self.nested_call_target_depth;
        self.nested_call_target_depth = Some(self.call_stack.len());

        let result = self.call_value_inner(func, this_val, args);

        // Restore the previous target depth
        self.nested_call_target_depth = saved_target;

        result
    }

    /// Inner implementation of call_value
    fn call_value_inner(
        &mut self,
        func: Value,
        this_val: Value,
        args: &[Value],
    ) -> InterpreterResult<Value> {
        // Handle closures
        if let Some(closure_idx) = func.to_closure_idx() {
            let closure = self.get_closure(closure_idx).ok_or_else(|| {
                InterpreterError::InternalError(format!("invalid closure index: {}", closure_idx))
            })?;
            let bytecode = unsafe { &*closure.bytecode };

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

            // Create frame with closure
            let frame = CallFrame::new_closure(
                bytecode as *const _,
                frame_ptr,
                args.len().min(u16::MAX as usize) as u16,
                this_val,
                func,
                closure_idx as usize,
            );
            self.call_stack.push(frame);

            // Run the interpreter loop
            return self.run();
        }

        // Handle function pointers
        if let Some(ptr) = func.to_func_ptr() {
            let bytecode = unsafe { &*ptr };
            return self.call_function(bytecode, this_val, args);
        }

        Err(InterpreterError::TypeError("not a function".to_string()))
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

            // Set current string constants for native functions to access
            self.current_string_constants = Some(&bytecode.string_constants as *const _);

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

                // Push empty string
                op if op == OpCode::PushEmptyString as u8 => {
                    self.stack.push(Value::string(crate::value::STR_EMPTY));
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

                // Get captured variable (16-bit index)
                op if op == OpCode::GetVarRef as u8 => {
                    let frame = self.call_stack.last_mut().unwrap();
                    let bytecode = unsafe { &*frame.bytecode };
                    let bc = &bytecode.bytecode;
                    let idx = u16::from_le_bytes([bc[frame.pc], bc[frame.pc + 1]]) as usize;
                    frame.pc += 2;

                    // Get the closure for this frame
                    let closure_idx = frame.closure_idx;
                    let val = if let Some(closure_idx) = closure_idx {
                        self.get_closure(closure_idx as u32)
                            .and_then(|c| c.get_var(idx))
                            .unwrap_or(Value::undefined())
                    } else {
                        Value::undefined()
                    };
                    self.stack.push(val);
                }

                // Set captured variable (16-bit index)
                op if op == OpCode::PutVarRef as u8 => {
                    let val = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let frame = self.call_stack.last_mut().unwrap();
                    let bytecode = unsafe { &*frame.bytecode };
                    let bc = &bytecode.bytecode;
                    let idx = u16::from_le_bytes([bc[frame.pc], bc[frame.pc + 1]]) as usize;
                    frame.pc += 2;

                    // Set the captured variable in the closure
                    if let Some(closure_idx) = frame.closure_idx {
                        if let Some(closure) = self.get_closure_mut(closure_idx as u32) {
                            closure.set_var(idx, val);
                        }
                    }
                }

                // Arithmetic: Negate
                op if op == OpCode::Neg as u8 => {
                    let val = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let result = self.op_neg(val)?;
                    self.stack.push(result);
                }

                // Arithmetic: Add (also handles string concatenation)
                op if op == OpCode::Add as u8 => {
                    let b = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let a = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;

                    // String concatenation: if either operand is a string, convert both to strings and concat
                    if a.is_string() || b.is_string() {
                        let frame = self.call_stack.last().unwrap();
                        let bytecode = unsafe { &*frame.bytecode };

                        let str_a = if a.is_string() {
                            self.get_string_content(a, bytecode).unwrap_or("").to_string()
                        } else if let Some(n) = a.to_i32() {
                            n.to_string()
                        } else if a.is_bool() {
                            if a.to_bool().unwrap_or(false) { "true" } else { "false" }.to_string()
                        } else if a.is_null() {
                            "null".to_string()
                        } else if a.is_undefined() {
                            "undefined".to_string()
                        } else {
                            "[object]".to_string()
                        };

                        let str_b = if b.is_string() {
                            self.get_string_content(b, bytecode).unwrap_or("").to_string()
                        } else if let Some(n) = b.to_i32() {
                            n.to_string()
                        } else if b.is_bool() {
                            if b.to_bool().unwrap_or(false) { "true" } else { "false" }.to_string()
                        } else if b.is_null() {
                            "null".to_string()
                        } else if b.is_undefined() {
                            "undefined".to_string()
                        } else {
                            "[object]".to_string()
                        };

                        let result = self.create_runtime_string(str_a + &str_b);
                        self.stack.push(result);
                    } else {
                        let result = self.op_add(a, b)?;
                        self.stack.push(result);
                    }
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
                    let frame = self.call_stack.last().unwrap();
                    let bytecode = unsafe { &*frame.bytecode };
                    let bc = &bytecode.bytecode;
                    let pc = frame.pc;
                    let frame_ptr = frame.frame_ptr;
                    let closure_idx_current = frame.closure_idx;

                    let func_idx = u16::from_le_bytes([bc[pc], bc[pc + 1]]) as usize;

                    // Get the inner function bytecode
                    let inner_func = bytecode
                        .inner_functions
                        .get(func_idx)
                        .ok_or_else(|| {
                            InterpreterError::InternalError(format!(
                                "invalid function index in FClosure: {}",
                                func_idx
                            ))
                        })?;

                    // Capture variables based on inner function's capture info
                    let mut var_refs = Vec::with_capacity(inner_func.captures.len());
                    for capture in &inner_func.captures {
                        let val = if capture.is_local {
                            // Capture from outer's locals (current frame)
                            self.stack
                                .get_local_at(frame_ptr, capture.outer_index)
                                .unwrap_or(Value::undefined())
                        } else {
                            // Capture from outer's captures (current frame's closure)
                            if let Some(closure_idx) = closure_idx_current {
                                self.get_closure(closure_idx as u32)
                                    .and_then(|c| c.get_var(capture.outer_index))
                                    .unwrap_or(Value::undefined())
                            } else {
                                Value::undefined()
                            }
                        };
                        var_refs.push(val);
                    }

                    // Update PC after we're done reading
                    let frame = self.call_stack.last_mut().unwrap();
                    frame.pc += 2;

                    // Create closure or simple function reference based on whether there are captures
                    let func_val = if !var_refs.is_empty() {
                        self.create_closure(inner_func as *const _, var_refs)
                    } else {
                        Value::func_ptr(inner_func as *const _)
                    };

                    self.stack.push(func_val);
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

                    // Check if this is a native function call
                    if let Some(native_idx) = func_val.to_native_func_idx() {
                        let result = self.call_native_func(native_idx, Value::undefined(), &args)?;
                        self.stack.push(result);
                        continue;
                    }

                    // Check if this is a builtin object called as a function
                    if let Some(builtin_idx) = func_val.to_builtin_object_idx() {
                        let result = self.call_builtin_as_function(builtin_idx, &args)?;
                        self.stack.push(result);
                        continue;
                    }

                    // Determine if this is a closure or a regular function
                    let (callee_bytecode, callee_closure_idx): (&FunctionBytecode, Option<usize>) =
                        if let Some(closure_idx) = func_val.to_closure_idx() {
                            // Closure call - get bytecode from closure
                            let closure = self.get_closure(closure_idx).ok_or_else(|| {
                                InterpreterError::InternalError(format!(
                                    "invalid closure index: {}",
                                    closure_idx
                                ))
                            })?;
                            (unsafe { &*closure.bytecode }, Some(closure_idx as usize))
                        } else if let Some(ptr) = func_val.to_func_ptr() {
                            // Pointer-based function (from FClosure without captures or ThisFunc)
                            (unsafe { &*ptr }, None)
                        } else if let Some(idx) = func_val.to_func_idx() {
                            // Index-based function (legacy, shouldn't happen anymore)
                            let bc = bytecode
                                .inner_functions
                                .get(idx as usize)
                                .ok_or_else(|| {
                                    InterpreterError::InternalError(format!(
                                        "invalid function index: {}",
                                        idx
                                    ))
                                })?;
                            (bc, None)
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

                    // Create frame - with closure_idx if this is a closure call
                    let callee_frame = if let Some(closure_idx) = callee_closure_idx {
                        CallFrame::new_closure(
                            callee_bytecode as *const _,
                            callee_frame_ptr,
                            args.len().min(u16::MAX as usize) as u16,
                            Value::undefined(), // this value
                            func_val,           // the function value for self-reference
                            closure_idx,
                        )
                    } else {
                        CallFrame::new(
                            callee_bytecode as *const _,
                            callee_frame_ptr,
                            args.len().min(u16::MAX as usize) as u16,
                            Value::undefined(), // this value
                            func_val,           // the function value for self-reference
                        )
                    };
                    self.call_stack.push(callee_frame);

                    // Continue execution in the new frame (run loop will pick it up)
                }

                // CallConstructor - new operator: func args -> new_object
                op if op == OpCode::CallConstructor as u8 => {
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

                    // Pop the constructor function value
                    let func_val = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;

                    // Check if this is a builtin Error constructor
                    if let Some(builtin_idx) = func_val.to_builtin_object_idx() {
                        if builtin_idx >= BUILTIN_ERROR && builtin_idx <= BUILTIN_RANGE_ERROR {
                            // Create an error object
                            let error_name = match builtin_idx {
                                BUILTIN_ERROR => "Error",
                                BUILTIN_TYPE_ERROR => "TypeError",
                                BUILTIN_REFERENCE_ERROR => "ReferenceError",
                                BUILTIN_SYNTAX_ERROR => "SyntaxError",
                                BUILTIN_RANGE_ERROR => "RangeError",
                                _ => "Error",
                            };

                            // Get message from first argument (if present)
                            let message = if let Some(msg_val) = args.first() {
                                if let Some(str_idx) = msg_val.to_string_idx() {
                                    self.get_string_by_idx(str_idx)
                                        .map(|s| s.to_string())
                                        .unwrap_or_default()
                                } else if let Some(n) = msg_val.to_i32() {
                                    n.to_string()
                                } else {
                                    String::new()
                                }
                            } else {
                                String::new()
                            };

                            // Create and store the error object
                            let error_idx = self.error_objects.len() as u32;
                            self.error_objects.push(ErrorObject {
                                name: error_name.to_string(),
                                message,
                            });

                            // Push the error object value
                            self.stack.push(Value::error_object(error_idx));
                            continue;
                        }

                        // Check if this is the RegExp constructor
                        if builtin_idx == BUILTIN_REGEXP {
                            // Get pattern from first argument
                            let pattern = if let Some(pattern_val) = args.first() {
                                if let Some(str_idx) = pattern_val.to_string_idx() {
                                    self.get_string_by_idx(str_idx)
                                        .map(|s| s.to_string())
                                        .unwrap_or_default()
                                } else {
                                    String::new()
                                }
                            } else {
                                String::new()
                            };

                            // Get flags from second argument (if present)
                            let flags = if let Some(flags_val) = args.get(1) {
                                if let Some(str_idx) = flags_val.to_string_idx() {
                                    self.get_string_by_idx(str_idx)
                                        .map(|s| s.to_string())
                                        .unwrap_or_default()
                                } else {
                                    String::new()
                                }
                            } else {
                                String::new()
                            };

                            // Parse flags
                            let global = flags.contains('g');
                            let ignore_case = flags.contains('i');
                            let multiline = flags.contains('m');

                            // Build regex pattern with flags
                            let mut regex_pattern = String::new();
                            if ignore_case || multiline {
                                regex_pattern.push_str("(?");
                                if ignore_case { regex_pattern.push('i'); }
                                if multiline { regex_pattern.push('m'); }
                                regex_pattern.push(')');
                            }
                            regex_pattern.push_str(&pattern);

                            // Compile the regex
                            match regex::Regex::new(&regex_pattern) {
                                Ok(regex) => {
                                    let regex_idx = self.regex_objects.len() as u32;
                                    self.regex_objects.push(RegExpObject {
                                        regex,
                                        pattern,
                                        flags,
                                        global,
                                        ignore_case,
                                        multiline,
                                    });
                                    self.stack.push(Value::regexp_object(regex_idx));
                                }
                                Err(e) => {
                                    // Invalid regex - return a SyntaxError
                                    return Err(InterpreterError::InternalError(
                                        format!("Invalid regular expression: {}", e)
                                    ));
                                }
                            }
                            continue;
                        }

                        // Check if this is the Map constructor
                        if builtin_idx == BUILTIN_MAP {
                            let map_idx = self.map_objects.len() as u32;
                            let mut map = MapObject::new();

                            // If an iterable is provided, populate the map
                            if let Some(iterable) = args.first() {
                                if let Some(arr_idx) = iterable.to_array_idx() {
                                    // Array of [key, value] pairs
                                    if let Some(arr) = self.get_array(arr_idx).cloned() {
                                        for entry in arr {
                                            if let Some(entry_arr_idx) = entry.to_array_idx() {
                                                if let Some(entry_arr) = self.get_array(entry_arr_idx).cloned() {
                                                    if entry_arr.len() >= 2 {
                                                        map.set(entry_arr[0], entry_arr[1]);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            self.map_objects.push(map);
                            self.stack.push(Value::map_object(map_idx));
                            continue;
                        }

                        // Check if this is the Set constructor
                        if builtin_idx == BUILTIN_SET {
                            let set_idx = self.set_objects.len() as u32;
                            let mut set = SetObject::new();

                            // If an iterable is provided, populate the set
                            if let Some(iterable) = args.first() {
                                if let Some(arr_idx) = iterable.to_array_idx() {
                                    if let Some(arr) = self.get_array(arr_idx).cloned() {
                                        for value in arr {
                                            set.add(value);
                                        }
                                    }
                                }
                            }

                            self.set_objects.push(set);
                            self.stack.push(Value::set_object(set_idx));
                            continue;
                        }
                    }

                    // Create a new object for 'this', storing the constructor reference for instanceof
                    let new_obj = self.create_object_with_constructor(func_val);

                    // Determine if this is a closure or a regular function
                    let (callee_bytecode, callee_closure_idx): (&FunctionBytecode, Option<usize>) =
                        if let Some(closure_idx) = func_val.to_closure_idx() {
                            let closure = self.get_closure(closure_idx).ok_or_else(|| {
                                InterpreterError::InternalError(format!(
                                    "invalid closure index: {}",
                                    closure_idx
                                ))
                            })?;
                            (unsafe { &*closure.bytecode }, Some(closure_idx as usize))
                        } else if let Some(ptr) = func_val.to_func_ptr() {
                            (unsafe { &*ptr }, None)
                        } else if let Some(idx) = func_val.to_func_idx() {
                            let bc = bytecode
                                .inner_functions
                                .get(idx as usize)
                                .ok_or_else(|| {
                                    InterpreterError::InternalError(format!(
                                        "invalid function index: {}",
                                        idx
                                    ))
                                })?;
                            (bc, None)
                        } else {
                            return Err(InterpreterError::TypeError(
                                "not a constructor".to_string(),
                            ));
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
                    let extra_locals =
                        callee_bytecode.local_count.saturating_sub(callee_bytecode.arg_count);
                    for _ in 0..extra_locals {
                        self.stack.push(Value::undefined());
                    }

                    // Create frame with new object as 'this' - marked as constructor call
                    let callee_frame = if let Some(closure_idx) = callee_closure_idx {
                        CallFrame::new_closure_constructor(
                            callee_bytecode as *const _,
                            callee_frame_ptr,
                            args.len().min(u16::MAX as usize) as u16,
                            new_obj, // 'this' is the new object
                            func_val,
                            closure_idx,
                        )
                    } else {
                        CallFrame::new_constructor(
                            callee_bytecode as *const _,
                            callee_frame_ptr,
                            args.len().min(u16::MAX as usize) as u16,
                            new_obj, // 'this' is the new object
                            func_val,
                        )
                    };
                    self.call_stack.push(callee_frame);

                    // Continue execution in the new frame
                    // When the constructor returns, do_return handles returning 'this'
                    // if the return value isn't an object
                }

                // CallMethod - method call: obj method args... -> ret
                // Stack before: [obj, method, arg0, arg1, ...]
                op if op == OpCode::CallMethod as u8 => {
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

                    // Pop the method value
                    let method_val = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;

                    // Pop the object (this value)
                    let this_val = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;

                    // Check if this is a native function call
                    if let Some(native_idx) = method_val.to_native_func_idx() {
                        let result = self.call_native_func(native_idx, this_val, &args)?;
                        self.stack.push(result);
                        continue;
                    }

                    // Determine if this is a closure or a regular function
                    let (callee_bytecode, callee_closure_idx): (&FunctionBytecode, Option<usize>) =
                        if let Some(closure_idx) = method_val.to_closure_idx() {
                            let closure = self.get_closure(closure_idx).ok_or_else(|| {
                                InterpreterError::InternalError(format!(
                                    "invalid closure index: {}",
                                    closure_idx
                                ))
                            })?;
                            (unsafe { &*closure.bytecode }, Some(closure_idx as usize))
                        } else if let Some(ptr) = method_val.to_func_ptr() {
                            (unsafe { &*ptr }, None)
                        } else if let Some(idx) = method_val.to_func_idx() {
                            let bc = bytecode
                                .inner_functions
                                .get(idx as usize)
                                .ok_or_else(|| {
                                    InterpreterError::InternalError(format!(
                                        "invalid function index: {}",
                                        idx
                                    ))
                                })?;
                            (bc, None)
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

                    // Create frame with the object as 'this'
                    let callee_frame = if let Some(closure_idx) = callee_closure_idx {
                        CallFrame::new_closure(
                            callee_bytecode as *const _,
                            callee_frame_ptr,
                            args.len().min(u16::MAX as usize) as u16,
                            this_val,  // Pass the object as 'this'
                            method_val,
                            closure_idx,
                        )
                    } else {
                        CallFrame::new(
                            callee_bytecode as *const _,
                            callee_frame_ptr,
                            args.len().min(u16::MAX as usize) as u16,
                            this_val,  // Pass the object as 'this'
                            method_val,
                        )
                    };
                    self.call_stack.push(callee_frame);
                }

                // TypeOf operator
                op if op == OpCode::TypeOf as u8 => {
                    use crate::value::{STR_UNDEFINED, STR_OBJECT, STR_BOOLEAN, STR_NUMBER, STR_FUNCTION, STR_STRING};

                    let val = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let type_idx = if val.is_undefined() {
                        STR_UNDEFINED
                    } else if val.is_null() {
                        STR_OBJECT // typeof null === "object" (JavaScript quirk)
                    } else if val.is_bool() {
                        STR_BOOLEAN
                    } else if val.is_int() {
                        STR_NUMBER
                    } else if val.is_string() {
                        STR_STRING
                    } else if val.is_func() || val.to_func_ptr().is_some() || val.is_closure() || val.is_native_func() {
                        STR_FUNCTION
                    } else if val.is_object() || val.is_array() {
                        STR_OBJECT
                    } else {
                        STR_OBJECT // Default for pointers/objects
                    };
                    self.stack.push(Value::string(type_idx));
                }

                // Nop
                op if op == OpCode::Nop as u8 => {
                    // Do nothing
                }

                // Print (built-in print statement)
                op if op == OpCode::Print as u8 => {
                    let val = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let frame = self.call_stack.last().unwrap();
                    let bytecode = unsafe { &*frame.bytecode };

                    // Convert value to string representation
                    let output = if val.is_string() {
                        self.get_string_content(val, bytecode).unwrap_or("").to_string()
                    } else if let Some(n) = val.to_i32() {
                        n.to_string()
                    } else if val.is_bool() {
                        if val.to_bool().unwrap_or(false) { "true" } else { "false" }.to_string()
                    } else if val.is_null() {
                        "null".to_string()
                    } else if val.is_undefined() {
                        "undefined".to_string()
                    } else if val.is_func() || val.to_func_ptr().is_some() {
                        "[function]".to_string()
                    } else {
                        "[object]".to_string()
                    };

                    println!("{}", output);
                }

                // GetGlobal - look up global variable by name
                op if op == OpCode::GetGlobal as u8 => {
                    let frame = self.call_stack.last_mut().unwrap();
                    let bytecode = unsafe { &*frame.bytecode };
                    let bc = &bytecode.bytecode;
                    let name_idx = u16::from_le_bytes([bc[frame.pc], bc[frame.pc + 1]]);
                    frame.pc += 2;

                    // Get the name from constant pool
                    let name = bytecode
                        .constants
                        .get(name_idx as usize)
                        .and_then(|v| {
                            if v.is_string() {
                                let str_idx = v.to_string_idx()?;
                                bytecode.string_constants.get(str_idx as usize).map(|s| s.as_str())
                            } else {
                                None
                            }
                        })
                        .ok_or_else(|| {
                            InterpreterError::InternalError(format!(
                                "invalid global name constant: {}",
                                name_idx
                            ))
                        })?;

                    // Look up the global by name
                    // First check for special global values and builtin objects
                    let val = match name {
                        "undefined" => Some(Value::undefined()),
                        "NaN" => Some(Value::int(0)), // TODO: proper NaN when floats are added
                        "Infinity" => Some(Value::int(i32::MAX)), // TODO: proper infinity when floats are added
                        "Math" => Some(Value::builtin_object(BUILTIN_MATH)),
                        "JSON" => Some(Value::builtin_object(BUILTIN_JSON)),
                        "Number" => Some(Value::builtin_object(BUILTIN_NUMBER)),
                        "Boolean" => Some(Value::builtin_object(BUILTIN_BOOLEAN)),
                        "String" => Some(Value::builtin_object(BUILTIN_STRING)),
                        "Object" => Some(Value::builtin_object(BUILTIN_OBJECT)),
                        "Array" => Some(Value::builtin_object(BUILTIN_ARRAY)),
                        "console" => Some(Value::builtin_object(BUILTIN_CONSOLE)),
                        "Date" => Some(Value::builtin_object(BUILTIN_DATE)),
                        "Error" => Some(Value::builtin_object(BUILTIN_ERROR)),
                        "TypeError" => Some(Value::builtin_object(BUILTIN_TYPE_ERROR)),
                        "ReferenceError" => Some(Value::builtin_object(BUILTIN_REFERENCE_ERROR)),
                        "SyntaxError" => Some(Value::builtin_object(BUILTIN_SYNTAX_ERROR)),
                        "RangeError" => Some(Value::builtin_object(BUILTIN_RANGE_ERROR)),
                        "RegExp" => Some(Value::builtin_object(BUILTIN_REGEXP)),
                        "Map" => Some(Value::builtin_object(BUILTIN_MAP)),
                        "Set" => Some(Value::builtin_object(BUILTIN_SET)),
                        "globalThis" => Some(Value::builtin_object(BUILTIN_GLOBAL_THIS)),
                        _ => self.get_native_func(name),
                    };

                    if let Some(v) = val {
                        self.stack.push(v);
                    } else {
                        return Err(InterpreterError::ReferenceError(format!(
                            "{} is not defined",
                            name
                        )));
                    }
                }

                // Catch - set up exception handler
                op if op == OpCode::Catch as u8 => {
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

                    // Calculate catch PC (relative to end of instruction)
                    let catch_pc = (frame.pc as i32 + offset) as usize;

                    // Push exception handler
                    self.exception_handlers.push(ExceptionHandler {
                        frame_depth: self.call_stack.len(),
                        catch_pc,
                        stack_depth: self.stack.len(),
                    });
                }

                // DropCatch - remove exception handler
                op if op == OpCode::DropCatch as u8 => {
                    // Pop the top exception handler
                    self.exception_handlers.pop();
                }

                // Throw - throw exception
                op if op == OpCode::Throw as u8 => {
                    let exception = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;

                    // Find the nearest exception handler
                    if let Some(handler) = self.exception_handlers.pop() {
                        // Unwind call stack to the handler's frame
                        while self.call_stack.len() > handler.frame_depth {
                            self.call_stack.pop();
                        }

                        // Restore stack to handler's depth
                        while self.stack.len() > handler.stack_depth {
                            self.stack.pop();
                        }

                        // Push the exception value for the catch block
                        self.stack.push(exception);

                        // Jump to catch block
                        if let Some(frame) = self.call_stack.last_mut() {
                            frame.pc = handler.catch_pc;
                        } else {
                            // No more frames - unhandled exception
                            return Err(InterpreterError::InternalError(
                                format!("Uncaught exception: {:?}", exception)
                            ));
                        }
                    } else {
                        // No handler - unhandled exception
                        return Err(InterpreterError::InternalError(
                            format!("Uncaught exception: {:?}", exception)
                        ));
                    }
                }

                // ArrayFrom - create array from stack elements
                op if op == OpCode::ArrayFrom as u8 => {
                    let frame = self.call_stack.last_mut().unwrap();
                    let bytecode = unsafe { &*frame.bytecode };
                    let bc = &bytecode.bytecode;
                    // Read number of elements (16-bit)
                    let count = u16::from_le_bytes([bc[frame.pc], bc[frame.pc + 1]]) as usize;
                    frame.pc += 2;

                    // Pop elements from stack (they were pushed in order)
                    let mut elements = Vec::with_capacity(count);
                    for _ in 0..count {
                        elements.push(self.stack.pop().ok_or(InterpreterError::StackUnderflow)?);
                    }
                    elements.reverse(); // Elements were pushed left-to-right

                    // Create array and push reference
                    let arr_val = self.create_array(elements);
                    self.stack.push(arr_val);
                }

                // GetArrayEl - get array element or object property: obj idx/key -> val
                op if op == OpCode::GetArrayEl as u8 => {
                    let idx = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let obj = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;

                    // Handle arrays
                    if let Some(arr_idx) = obj.to_array_idx() {
                        if let Some(array) = self.get_array(arr_idx) {
                            // Try string key for array methods
                            if let Some(str_idx) = idx.to_string_idx() {
                                if let Some(key) = self.get_string_by_idx(str_idx) {
                                    let val = self.get_array_property(obj, key);
                                    self.stack.push(val);
                                    continue;
                                }
                            }
                            // Numeric index
                            let index = idx.to_i32().unwrap_or(0) as usize;
                            let val = array.get(index).copied().unwrap_or(Value::undefined());
                            self.stack.push(val);
                            continue;
                        }
                    }

                    // Handle objects
                    if let Some(obj_idx) = obj.to_object_idx() {
                        let key = if let Some(str_idx) = idx.to_string_idx() {
                            self.get_string_by_idx(str_idx).unwrap_or("").to_string()
                        } else if let Some(n) = idx.to_i32() {
                            n.to_string()
                        } else {
                            String::new()
                        };

                        let val = self.objects
                            .get(obj_idx as usize)
                            .and_then(|o| o.properties.iter().find(|(k, _)| k == &key).map(|(_, v)| *v))
                            .unwrap_or(Value::undefined());
                        self.stack.push(val);
                        continue;
                    }

                    // Handle builtin objects (like Array, Object)
                    if let Some(builtin_idx) = obj.to_builtin_object_idx() {
                        if let Some(str_idx) = idx.to_string_idx() {
                            if let Some(key) = self.get_string_by_idx(str_idx) {
                                let val = self.get_builtin_property(builtin_idx, key);
                                self.stack.push(val);
                                continue;
                            }
                        }
                    }

                    // Handle strings
                    if let Some(str_idx) = obj.to_string_idx() {
                        if let Some(n) = idx.to_i32() {
                            if let Some(s) = self.get_string_by_idx(str_idx) {
                                if let Some(c) = s.chars().nth(n as usize) {
                                    let val = self.create_runtime_string(c.to_string());
                                    self.stack.push(val);
                                    continue;
                                }
                            }
                        }
                    }

                    self.stack.push(Value::undefined());
                }

                // GetArrayEl2 - get array element, keep object: arr idx -> arr val
                op if op == OpCode::GetArrayEl2 as u8 => {
                    let idx = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let arr = self.stack.peek().ok_or(InterpreterError::StackUnderflow)?;

                    // Get the array
                    let arr_idx = arr.to_array_idx().ok_or_else(|| {
                        InterpreterError::TypeError("cannot read property of non-array".to_string())
                    })?;

                    let array = self.get_array(arr_idx).ok_or_else(|| {
                        InterpreterError::InternalError("invalid array index".to_string())
                    })?;

                    // Get the element
                    let index = idx.to_i32().ok_or_else(|| {
                        InterpreterError::TypeError("array index must be a number".to_string())
                    })? as usize;

                    let val = array.get(index).copied().unwrap_or(Value::undefined());
                    self.stack.push(val);
                }

                // PutArrayEl - set array element: arr idx val -> val
                op if op == OpCode::PutArrayEl as u8 => {
                    let val = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let idx = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let arr = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;

                    // Get the array
                    let arr_idx = arr.to_array_idx().ok_or_else(|| {
                        InterpreterError::TypeError("cannot set property of non-array".to_string())
                    })?;

                    // Get the index
                    let index = idx.to_i32().ok_or_else(|| {
                        InterpreterError::TypeError("array index must be a number".to_string())
                    })? as usize;

                    // Set the element, extending if necessary
                    let array = self.get_array_mut(arr_idx).ok_or_else(|| {
                        InterpreterError::InternalError("invalid array index".to_string())
                    })?;

                    // Extend array if index is out of bounds
                    if index >= array.len() {
                        array.resize(index + 1, Value::undefined());
                    }
                    array[index] = val;

                    // Push the assigned value back (assignment is an expression)
                    self.stack.push(val);
                }

                // GetField - get object property: obj -> value
                op if op == OpCode::GetField as u8 => {
                    let frame = self.call_stack.last_mut().unwrap();
                    let bytecode = unsafe { &*frame.bytecode };
                    let bc = &bytecode.bytecode;
                    let str_idx = u16::from_le_bytes([bc[frame.pc], bc[frame.pc + 1]]) as usize;
                    frame.pc += 2;

                    let obj = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;

                    // Get property name from string constants
                    let prop_name = bytecode.string_constants.get(str_idx).ok_or_else(|| {
                        InterpreterError::InternalError(format!("invalid string index: {}", str_idx))
                    })?;

                    // Check if this is a builtin object (Math, JSON, etc.)
                    if let Some(builtin_idx) = obj.to_builtin_object_idx() {
                        let val = self.get_builtin_property(builtin_idx, prop_name);
                        self.stack.push(val);
                    } else if obj.is_array() {
                        // Array property access - check for Array.prototype methods
                        let val = self.get_array_property(obj, prop_name);
                        // Fall back to Object.prototype methods for arrays
                        let val = if val.is_undefined() {
                            self.get_object_prototype_property(prop_name)
                        } else {
                            val
                        };
                        self.stack.push(val);
                    } else if let Some(err_idx) = obj.to_error_object_idx() {
                        // Error object property access
                        let val = self.get_error_property(err_idx, prop_name);
                        self.stack.push(val);
                    } else if let Some(regex_idx) = obj.to_regexp_object_idx() {
                        // RegExp object property access
                        let val = self.get_regexp_property(regex_idx, prop_name);
                        self.stack.push(val);
                    } else if let Some(map_idx) = obj.to_map_object_idx() {
                        // Map object property access
                        let val = self.get_map_property(map_idx, prop_name);
                        self.stack.push(val);
                    } else if let Some(set_idx) = obj.to_set_object_idx() {
                        // Set object property access
                        let val = self.get_set_property(set_idx, prop_name);
                        self.stack.push(val);
                    } else if let Some(obj_idx) = obj.to_object_idx() {
                        // Get property from regular object
                        let val = self.object_get_property(obj_idx, prop_name);
                        // Check Object.prototype methods if property not found
                        let val = if val.is_undefined() {
                            self.get_object_prototype_property(prop_name)
                        } else {
                            val
                        };
                        self.stack.push(val);
                    } else if obj.is_string() {
                        // String property access - check for String.prototype methods
                        let val = self.get_string_property(obj, prop_name);
                        self.stack.push(val);
                    } else if obj.is_int() {
                        // Number property access - check for Number.prototype methods
                        let val = self.get_number_property(prop_name);
                        self.stack.push(val);
                    } else {
                        // For non-objects, return undefined
                        self.stack.push(Value::undefined());
                    }
                }

                // GetField2 - get object property but keep object: obj -> obj value
                // Used for method calls where we need the object as 'this'
                op if op == OpCode::GetField2 as u8 => {
                    let frame = self.call_stack.last_mut().unwrap();
                    let bytecode = unsafe { &*frame.bytecode };
                    let bc = &bytecode.bytecode;
                    let str_idx = u16::from_le_bytes([bc[frame.pc], bc[frame.pc + 1]]) as usize;
                    frame.pc += 2;

                    // Peek at the object (don't pop - we need to keep it for 'this')
                    let obj = self.stack.peek().ok_or(InterpreterError::StackUnderflow)?;

                    // Get property name from string constants
                    let prop_name = bytecode.string_constants.get(str_idx).ok_or_else(|| {
                        InterpreterError::InternalError(format!("invalid string index: {}", str_idx))
                    })?;

                    // Get the property value (same logic as GetField)
                    let val = if let Some(builtin_idx) = obj.to_builtin_object_idx() {
                        self.get_builtin_property(builtin_idx, prop_name)
                    } else if obj.is_array() {
                        let val = self.get_array_property(obj, prop_name);
                        // Fall back to Object.prototype methods for arrays
                        if val.is_undefined() {
                            self.get_object_prototype_property(prop_name)
                        } else {
                            val
                        }
                    } else if let Some(regex_idx) = obj.to_regexp_object_idx() {
                        self.get_regexp_property(regex_idx, prop_name)
                    } else if let Some(map_idx) = obj.to_map_object_idx() {
                        self.get_map_property(map_idx, prop_name)
                    } else if let Some(set_idx) = obj.to_set_object_idx() {
                        self.get_set_property(set_idx, prop_name)
                    } else if let Some(obj_idx) = obj.to_object_idx() {
                        // First check object's own properties
                        let val = self.object_get_property(obj_idx, prop_name);
                        if val.is_undefined() {
                            // Check Object.prototype methods
                            self.get_object_prototype_property(prop_name)
                        } else {
                            val
                        }
                    } else if obj.is_string() {
                        self.get_string_property(obj, prop_name)
                    } else if obj.is_int() {
                        // Number.prototype methods
                        self.get_number_property(prop_name)
                    } else if obj.is_closure() || obj.to_func_ptr().is_some() {
                        // Function.prototype methods (call, apply, bind)
                        self.get_function_property(prop_name)
                    } else {
                        Value::undefined()
                    };

                    // Push the property value (object is still on stack below it)
                    self.stack.push(val);
                }

                // PutField - set object property: obj val -> val
                op if op == OpCode::PutField as u8 => {
                    let frame = self.call_stack.last_mut().unwrap();
                    let bytecode = unsafe { &*frame.bytecode };
                    let bc = &bytecode.bytecode;
                    let str_idx = u16::from_le_bytes([bc[frame.pc], bc[frame.pc + 1]]) as usize;
                    frame.pc += 2;

                    let val = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let obj = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;

                    // Get property name from string constants
                    let prop_name = bytecode.string_constants.get(str_idx).ok_or_else(|| {
                        InterpreterError::InternalError(format!("invalid string index: {}", str_idx))
                    })?.clone();

                    // Set property on object
                    if let Some(obj_idx) = obj.to_object_idx() {
                        self.object_set_property(obj_idx, prop_name, val);
                    }
                    // Push the assigned value back (assignment is an expression)
                    self.stack.push(val);
                }

                // In operator: prop in obj -> bool
                op if op == OpCode::In as u8 => {
                    let frame = self.call_stack.last().unwrap();
                    let bytecode = unsafe { &*frame.bytecode };

                    let obj = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let prop = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;

                    let result = if let Some(obj_idx) = obj.to_object_idx() {
                        // Check if property exists in object
                        // Convert prop to string, checking bytecode string constants
                        let prop_name = if prop.is_string() {
                            if let Some(str_idx) = prop.to_string_idx() {
                                // Check built-in strings first
                                use crate::value::{STR_UNDEFINED, STR_OBJECT, STR_BOOLEAN, STR_NUMBER, STR_FUNCTION, STR_STRING, STR_EMPTY};
                                match str_idx {
                                    STR_UNDEFINED => Some("undefined".to_string()),
                                    STR_OBJECT => Some("object".to_string()),
                                    STR_BOOLEAN => Some("boolean".to_string()),
                                    STR_NUMBER => Some("number".to_string()),
                                    STR_FUNCTION => Some("function".to_string()),
                                    STR_STRING => Some("string".to_string()),
                                    STR_EMPTY => Some(String::new()),
                                    _ => {
                                        if str_idx >= 0x8000 {
                                            self.runtime_strings.get((str_idx - 0x8000) as usize).cloned()
                                        } else {
                                            // Compile-time string constant
                                            bytecode.string_constants.get(str_idx as usize).cloned()
                                        }
                                    }
                                }
                            } else {
                                None
                            }
                        } else if let Some(n) = prop.to_i32() {
                            Some(n.to_string())
                        } else {
                            None
                        };

                        if let Some(name) = prop_name {
                            let obj_props = self.get_object(obj_idx);
                            let exists = obj_props
                                .map(|props| props.properties.iter().any(|(k, _)| k == &name))
                                .unwrap_or(false);
                            Value::bool(exists)
                        } else {
                            Value::bool(false)
                        }
                    } else if let Some(arr_idx) = obj.to_array_idx() {
                        // Check if index exists in array
                        if let Some(idx) = prop.to_i32() {
                            let arr = self.get_array(arr_idx);
                            let exists = arr
                                .map(|a| idx >= 0 && (idx as usize) < a.len())
                                .unwrap_or(false);
                            Value::bool(exists)
                        } else {
                            Value::bool(false)
                        }
                    } else {
                        Value::bool(false)
                    };
                    self.stack.push(result);
                }

                // Delete operator: obj prop -> bool
                op if op == OpCode::Delete as u8 => {
                    let frame = self.call_stack.last().unwrap();
                    let bytecode = unsafe { &*frame.bytecode };

                    let prop = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let obj = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;

                    let result = if let Some(obj_idx) = obj.to_object_idx() {
                        // Convert prop to string, checking bytecode string constants
                        let prop_name = if prop.is_string() {
                            if let Some(str_idx) = prop.to_string_idx() {
                                use crate::value::{STR_UNDEFINED, STR_OBJECT, STR_BOOLEAN, STR_NUMBER, STR_FUNCTION, STR_STRING, STR_EMPTY};
                                match str_idx {
                                    STR_UNDEFINED => Some("undefined".to_string()),
                                    STR_OBJECT => Some("object".to_string()),
                                    STR_BOOLEAN => Some("boolean".to_string()),
                                    STR_NUMBER => Some("number".to_string()),
                                    STR_FUNCTION => Some("function".to_string()),
                                    STR_STRING => Some("string".to_string()),
                                    STR_EMPTY => Some(String::new()),
                                    _ => {
                                        if str_idx >= 0x8000 {
                                            self.runtime_strings.get((str_idx - 0x8000) as usize).cloned()
                                        } else {
                                            bytecode.string_constants.get(str_idx as usize).cloned()
                                        }
                                    }
                                }
                            } else {
                                None
                            }
                        } else if let Some(n) = prop.to_i32() {
                            Some(n.to_string())
                        } else {
                            None
                        };

                        // Delete property from object
                        if let Some(name) = prop_name {
                            if let Some(obj_props) = self.get_object_mut(obj_idx) {
                                let orig_len = obj_props.properties.len();
                                obj_props.properties.retain(|(k, _)| k != &name);
                                Value::bool(obj_props.properties.len() < orig_len)
                            } else {
                                Value::bool(false)
                            }
                        } else {
                            Value::bool(false)
                        }
                    } else if let Some(arr_idx) = obj.to_array_idx() {
                        // For arrays, set element to undefined (don't actually remove)
                        if let Some(idx) = prop.to_i32() {
                            if let Some(arr) = self.get_array_mut(arr_idx) {
                                if idx >= 0 && (idx as usize) < arr.len() {
                                    arr[idx as usize] = Value::undefined();
                                    Value::bool(true)
                                } else {
                                    Value::bool(true) // Deleting non-existent index returns true
                                }
                            } else {
                                Value::bool(false)
                            }
                        } else {
                            Value::bool(false)
                        }
                    } else {
                        Value::bool(true) // delete on non-object returns true
                    };
                    self.stack.push(result);
                }

                // InstanceOf operator: obj ctor -> bool
                op if op == OpCode::InstanceOf as u8 => {
                    let ctor = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;
                    let obj = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;

                    let result = if let Some(obj_idx) = obj.to_object_idx() {
                        // Get the constructor stored when the object was created
                        if let Some(obj_instance) = self.get_object(obj_idx) {
                            if let Some(stored_ctor) = obj_instance.constructor {
                                // Compare if the stored constructor matches the right operand
                                // For closures, compare the closure indices
                                if let (Some(stored_idx), Some(ctor_idx)) =
                                    (stored_ctor.to_closure_idx(), ctor.to_closure_idx())
                                {
                                    // Same closure instance
                                    Value::bool(stored_idx == ctor_idx)
                                } else {
                                    // For non-closure functions, compare raw values
                                    Value::bool(stored_ctor.0 == ctor.0)
                                }
                            } else {
                                // Object was not created with new
                                Value::bool(false)
                            }
                        } else {
                            Value::bool(false)
                        }
                    } else {
                        // Left operand is not an object
                        Value::bool(false)
                    };
                    self.stack.push(result);
                }

                // ForInStart - Start for-in iteration: obj -> iter
                op if op == OpCode::ForInStart as u8 => {
                    let obj = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;

                    // Create iterator based on value type
                    let iter = if let Some(obj_idx) = obj.to_object_idx() {
                        if let Some(obj_instance) = self.get_object(obj_idx) {
                            ForInIterator::from_object(obj_instance)
                        } else {
                            ForInIterator { keys: Vec::new(), index: 0 }
                        }
                    } else if let Some(arr_idx) = obj.to_array_idx() {
                        if let Some(arr) = self.get_array(arr_idx) {
                            ForInIterator::from_array(arr)
                        } else {
                            ForInIterator { keys: Vec::new(), index: 0 }
                        }
                    } else {
                        // For non-objects/arrays, create empty iterator
                        ForInIterator { keys: Vec::new(), index: 0 }
                    };

                    // Store iterator and push reference
                    let iter_idx = self.for_in_iterators.len();
                    self.for_in_iterators.push(iter);
                    self.stack.push(Value::iterator_idx(iter_idx as u32));
                }

                // ForInNext - Get next for-in key: iter -> key done
                op if op == OpCode::ForInNext as u8 => {
                    let iter_val = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;

                    if let Some(iter_idx) = iter_val.to_iterator_idx() {
                        if let Some(iter) = self.for_in_iterators.get_mut(iter_idx as usize) {
                            if let Some(key) = iter.next() {
                                // Push key and false (not done)
                                let key_val = self.create_runtime_string(key);
                                self.stack.push(key_val);
                                self.stack.push(Value::bool(false)); // not done
                            } else {
                                // Push undefined and true (done)
                                self.stack.push(Value::undefined());
                                self.stack.push(Value::bool(true)); // done
                            }
                        } else {
                            // Invalid iterator, push done
                            self.stack.push(Value::undefined());
                            self.stack.push(Value::bool(true));
                        }
                    } else {
                        // Not an iterator, push done
                        self.stack.push(Value::undefined());
                        self.stack.push(Value::bool(true));
                    }
                }

                // ForOfStart - Start for-of iteration: obj -> iter
                op if op == OpCode::ForOfStart as u8 => {
                    let obj = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;

                    // Create iterator based on value type
                    let iter = if let Some(arr_idx) = obj.to_array_idx() {
                        if let Some(arr) = self.get_array(arr_idx) {
                            ForOfIterator::from_array(arr)
                        } else {
                            ForOfIterator { values: Vec::new(), index: 0 }
                        }
                    } else if let Some(obj_idx) = obj.to_object_idx() {
                        if let Some(obj_instance) = self.get_object(obj_idx) {
                            ForOfIterator::from_object(obj_instance)
                        } else {
                            ForOfIterator { values: Vec::new(), index: 0 }
                        }
                    } else {
                        // For non-objects/arrays, create empty iterator
                        ForOfIterator { values: Vec::new(), index: 0 }
                    };

                    // Store iterator and push reference
                    let iter_idx = self.for_of_iterators.len();
                    self.for_of_iterators.push(iter);
                    self.stack.push(Value::for_of_iterator_idx(iter_idx as u32));
                }

                // ForOfNext - Get next for-of value: iter -> value done
                op if op == OpCode::ForOfNext as u8 => {
                    let iter_val = self.stack.pop().ok_or(InterpreterError::StackUnderflow)?;

                    if let Some(iter_idx) = iter_val.to_for_of_iterator_idx() {
                        if let Some(iter) = self.for_of_iterators.get_mut(iter_idx as usize) {
                            if let Some(val) = iter.next() {
                                // Push value and false (not done)
                                self.stack.push(val);
                                self.stack.push(Value::bool(false)); // not done
                            } else {
                                // Push undefined and true (done)
                                self.stack.push(Value::undefined());
                                self.stack.push(Value::bool(true)); // done
                            }
                        } else {
                            // Invalid iterator, push done
                            self.stack.push(Value::undefined());
                            self.stack.push(Value::bool(true));
                        }
                    } else {
                        // Not an iterator, push done
                        self.stack.push(Value::undefined());
                        self.stack.push(Value::bool(true));
                    }
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

        // For constructor calls: if result is not an object, return 'this' instead
        let final_result = if frame.is_constructor && !result.is_object() {
            frame.this_val
        } else {
            result
        };

        // If there are no more frames, this is the final result
        if self.call_stack.is_empty() {
            return Ok(final_result);
        }

        // Check if we've reached the target depth for a nested call_value
        if let Some(target_depth) = self.nested_call_target_depth {
            if self.call_stack.len() == target_depth {
                // Don't push result or continue - just return to call_value
                return Ok(final_result);
            }
        }

        // Otherwise, push the result for the caller and continue
        self.stack.push(final_result);

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

    /// Convert a value to a string for property access
    fn value_to_string(&self, val: &Value) -> Option<String> {
        if val.is_string() {
            // Get string from string constants or runtime strings
            let str_idx = val.to_string_idx()?;
            // Check if it's a built-in string
            use crate::value::{STR_UNDEFINED, STR_OBJECT, STR_BOOLEAN, STR_NUMBER, STR_FUNCTION, STR_STRING, STR_EMPTY};
            match str_idx {
                STR_UNDEFINED => Some("undefined".to_string()),
                STR_OBJECT => Some("object".to_string()),
                STR_BOOLEAN => Some("boolean".to_string()),
                STR_NUMBER => Some("number".to_string()),
                STR_FUNCTION => Some("function".to_string()),
                STR_STRING => Some("string".to_string()),
                STR_EMPTY => Some(String::new()),
                _ => {
                    // Check runtime strings first (high indices)
                    if str_idx >= 0x8000 {
                        self.runtime_strings.get((str_idx - 0x8000) as usize).cloned()
                    } else {
                        // It's a compile-time string - we need bytecode access
                        // For now, return None (caller should handle)
                        None
                    }
                }
            }
        } else if let Some(n) = val.to_i32() {
            Some(n.to_string())
        } else {
            None
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

    // =========================================================================
    // Native function support
    // =========================================================================

    /// Register a native function and return its index
    pub fn register_native(&mut self, name: &'static str, func: NativeFn, arity: u8) -> u32 {
        let idx = self.native_functions.len() as u32;
        self.native_functions.push(NativeFunction { name, func, arity });
        idx
    }

    /// Get a native function value by name
    pub fn get_native_func(&self, name: &str) -> Option<Value> {
        for (idx, nf) in self.native_functions.iter().enumerate() {
            if nf.name == name {
                return Some(Value::native_func(idx as u32));
            }
        }
        None
    }

    /// Get a property from an array (Array.prototype methods or length)
    fn get_array_property(&self, arr: Value, prop_name: &str) -> Value {
        match prop_name {
            "length" => {
                // Return the array length
                if let Some(arr_idx) = arr.to_array_idx() {
                    if let Some(arr_data) = self.arrays.get(arr_idx as usize) {
                        return Value::int(arr_data.len() as i32);
                    }
                }
                Value::undefined()
            }
            "push" => self.get_native_func("Array.prototype.push").unwrap_or(Value::undefined()),
            "pop" => self.get_native_func("Array.prototype.pop").unwrap_or(Value::undefined()),
            "shift" => self.get_native_func("Array.prototype.shift").unwrap_or(Value::undefined()),
            "unshift" => self.get_native_func("Array.prototype.unshift").unwrap_or(Value::undefined()),
            "indexOf" => self.get_native_func("Array.prototype.indexOf").unwrap_or(Value::undefined()),
            "join" => self.get_native_func("Array.prototype.join").unwrap_or(Value::undefined()),
            "reverse" => self.get_native_func("Array.prototype.reverse").unwrap_or(Value::undefined()),
            "slice" => self.get_native_func("Array.prototype.slice").unwrap_or(Value::undefined()),
            "map" => self.get_native_func("Array.prototype.map").unwrap_or(Value::undefined()),
            "filter" => self.get_native_func("Array.prototype.filter").unwrap_or(Value::undefined()),
            "forEach" => self.get_native_func("Array.prototype.forEach").unwrap_or(Value::undefined()),
            "reduce" => self.get_native_func("Array.prototype.reduce").unwrap_or(Value::undefined()),
            "reduceRight" => self.get_native_func("Array.prototype.reduceRight").unwrap_or(Value::undefined()),
            "find" => self.get_native_func("Array.prototype.find").unwrap_or(Value::undefined()),
            "findIndex" => self.get_native_func("Array.prototype.findIndex").unwrap_or(Value::undefined()),
            "some" => self.get_native_func("Array.prototype.some").unwrap_or(Value::undefined()),
            "every" => self.get_native_func("Array.prototype.every").unwrap_or(Value::undefined()),
            "includes" => self.get_native_func("Array.prototype.includes").unwrap_or(Value::undefined()),
            "concat" => self.get_native_func("Array.prototype.concat").unwrap_or(Value::undefined()),
            "sort" => self.get_native_func("Array.prototype.sort").unwrap_or(Value::undefined()),
            "flat" => self.get_native_func("Array.prototype.flat").unwrap_or(Value::undefined()),
            "fill" => self.get_native_func("Array.prototype.fill").unwrap_or(Value::undefined()),
            "splice" => self.get_native_func("Array.prototype.splice").unwrap_or(Value::undefined()),
            "lastIndexOf" => self.get_native_func("Array.prototype.lastIndexOf").unwrap_or(Value::undefined()),
            "flatMap" => self.get_native_func("Array.prototype.flatMap").unwrap_or(Value::undefined()),
            "copyWithin" => self.get_native_func("Array.prototype.copyWithin").unwrap_or(Value::undefined()),
            "at" => self.get_native_func("Array.prototype.at").unwrap_or(Value::undefined()),
            "findLast" => self.get_native_func("Array.prototype.findLast").unwrap_or(Value::undefined()),
            "findLastIndex" => self.get_native_func("Array.prototype.findLastIndex").unwrap_or(Value::undefined()),
            "toSorted" => self.get_native_func("Array.prototype.toSorted").unwrap_or(Value::undefined()),
            "toReversed" => self.get_native_func("Array.prototype.toReversed").unwrap_or(Value::undefined()),
            "with" => self.get_native_func("Array.prototype.with").unwrap_or(Value::undefined()),
            "toSpliced" => self.get_native_func("Array.prototype.toSpliced").unwrap_or(Value::undefined()),
            "toString" => self.get_native_func("Array.prototype.toString").unwrap_or(Value::undefined()),
            "keys" => self.get_native_func("Array.prototype.keys").unwrap_or(Value::undefined()),
            "values" => self.get_native_func("Array.prototype.values").unwrap_or(Value::undefined()),
            "entries" => self.get_native_func("Array.prototype.entries").unwrap_or(Value::undefined()),
            _ => Value::undefined(),
        }
    }

    /// Get a property from a string (String.prototype methods or length)
    fn get_string_property(&self, str_val: Value, prop_name: &str) -> Value {
        match prop_name {
            "length" => {
                // Get string length
                if let Some(str_idx) = str_val.to_string_idx() {
                    if let Some(s) = self.get_string_by_idx(str_idx) {
                        return Value::int(s.len() as i32);
                    }
                }
                Value::int(0)
            }
            "charAt" => self.get_native_func("String.prototype.charAt").unwrap_or(Value::undefined()),
            "indexOf" => self.get_native_func("String.prototype.indexOf").unwrap_or(Value::undefined()),
            "slice" => self.get_native_func("String.prototype.slice").unwrap_or(Value::undefined()),
            "substring" => self.get_native_func("String.prototype.substring").unwrap_or(Value::undefined()),
            "toUpperCase" => self.get_native_func("String.prototype.toUpperCase").unwrap_or(Value::undefined()),
            "toLowerCase" => self.get_native_func("String.prototype.toLowerCase").unwrap_or(Value::undefined()),
            "trim" => self.get_native_func("String.prototype.trim").unwrap_or(Value::undefined()),
            "split" => self.get_native_func("String.prototype.split").unwrap_or(Value::undefined()),
            "concat" => self.get_native_func("String.prototype.concat").unwrap_or(Value::undefined()),
            "repeat" => self.get_native_func("String.prototype.repeat").unwrap_or(Value::undefined()),
            "startsWith" => self.get_native_func("String.prototype.startsWith").unwrap_or(Value::undefined()),
            "endsWith" => self.get_native_func("String.prototype.endsWith").unwrap_or(Value::undefined()),
            "padStart" => self.get_native_func("String.prototype.padStart").unwrap_or(Value::undefined()),
            "padEnd" => self.get_native_func("String.prototype.padEnd").unwrap_or(Value::undefined()),
            "replace" => self.get_native_func("String.prototype.replace").unwrap_or(Value::undefined()),
            "includes" => self.get_native_func("String.prototype.includes").unwrap_or(Value::undefined()),
            "match" => self.get_native_func("String.prototype.match").unwrap_or(Value::undefined()),
            "search" => self.get_native_func("String.prototype.search").unwrap_or(Value::undefined()),
            "lastIndexOf" => self.get_native_func("String.prototype.lastIndexOf").unwrap_or(Value::undefined()),
            "trimStart" => self.get_native_func("String.prototype.trimStart").unwrap_or(Value::undefined()),
            "trimEnd" => self.get_native_func("String.prototype.trimEnd").unwrap_or(Value::undefined()),
            "replaceAll" => self.get_native_func("String.prototype.replaceAll").unwrap_or(Value::undefined()),
            "at" => self.get_native_func("String.prototype.at").unwrap_or(Value::undefined()),
            "charCodeAt" => self.get_native_func("String.prototype.charCodeAt").unwrap_or(Value::undefined()),
            "codePointAt" => self.get_native_func("String.prototype.codePointAt").unwrap_or(Value::undefined()),
            _ => Value::undefined(),
        }
    }

    /// Get a property from a number (Number.prototype methods)
    fn get_number_property(&self, prop_name: &str) -> Value {
        match prop_name {
            "toFixed" => self.get_native_func("Number.prototype.toFixed").unwrap_or(Value::undefined()),
            "toString" => self.get_native_func("Number.prototype.toString").unwrap_or(Value::undefined()),
            _ => Value::undefined(),
        }
    }

    /// Get a property from Object.prototype (for all objects)
    fn get_object_prototype_property(&self, prop_name: &str) -> Value {
        match prop_name {
            "hasOwnProperty" => self.get_native_func("Object.prototype.hasOwnProperty").unwrap_or(Value::undefined()),
            "toString" => self.get_native_func("Object.prototype.toString").unwrap_or(Value::undefined()),
            "valueOf" => self.get_native_func("Object.prototype.valueOf").unwrap_or(Value::undefined()),
            _ => Value::undefined(),
        }
    }

    /// Get a property from an error object
    fn get_error_property(&mut self, err_idx: u32, prop_name: &str) -> Value {
        if let Some(err) = self.error_objects.get(err_idx as usize).cloned() {
            match prop_name {
                "name" => {
                    // Return the error name as a runtime string
                    self.create_runtime_string(err.name)
                }
                "message" => {
                    // Return the error message as a runtime string
                    self.create_runtime_string(err.message)
                }
                "toString" => {
                    self.get_native_func("Error.prototype.toString").unwrap_or(Value::undefined())
                }
                _ => Value::undefined(),
            }
        } else {
            Value::undefined()
        }
    }

    /// Get a property from a function (Function.prototype methods)
    fn get_function_property(&self, prop_name: &str) -> Value {
        match prop_name {
            "call" => self.get_native_func("Function.prototype.call").unwrap_or(Value::undefined()),
            "apply" => self.get_native_func("Function.prototype.apply").unwrap_or(Value::undefined()),
            "bind" => self.get_native_func("Function.prototype.bind").unwrap_or(Value::undefined()),
            _ => Value::undefined(),
        }
    }

    /// Get a property from a RegExp object
    fn get_regexp_property(&self, regex_idx: u32, prop_name: &str) -> Value {
        if let Some(re) = self.regex_objects.get(regex_idx as usize) {
            match prop_name {
                "test" => self.get_native_func("RegExp.prototype.test").unwrap_or(Value::undefined()),
                "exec" => self.get_native_func("RegExp.prototype.exec").unwrap_or(Value::undefined()),
                "global" => Value::bool(re.global),
                "ignoreCase" => Value::bool(re.ignore_case),
                "multiline" => Value::bool(re.multiline),
                "source" => {
                    // Return pattern as a string - but we need mutable access for runtime strings
                    // For now, just return undefined
                    Value::undefined()
                }
                _ => Value::undefined(),
            }
        } else {
            Value::undefined()
        }
    }

    /// Get a property from a Map object
    fn get_map_property(&self, map_idx: u32, prop_name: &str) -> Value {
        match prop_name {
            "get" => self.get_native_func("Map.prototype.get").unwrap_or(Value::undefined()),
            "set" => self.get_native_func("Map.prototype.set").unwrap_or(Value::undefined()),
            "has" => self.get_native_func("Map.prototype.has").unwrap_or(Value::undefined()),
            "delete" => self.get_native_func("Map.prototype.delete").unwrap_or(Value::undefined()),
            "clear" => self.get_native_func("Map.prototype.clear").unwrap_or(Value::undefined()),
            "forEach" => self.get_native_func("Map.prototype.forEach").unwrap_or(Value::undefined()),
            "size" => {
                // size is a getter
                let size = self.map_objects.get(map_idx as usize)
                    .map(|m| m.size())
                    .unwrap_or(0);
                Value::int(size as i32)
            }
            _ => Value::undefined(),
        }
    }

    /// Get a property from a Set object
    fn get_set_property(&self, set_idx: u32, prop_name: &str) -> Value {
        match prop_name {
            "add" => self.get_native_func("Set.prototype.add").unwrap_or(Value::undefined()),
            "has" => self.get_native_func("Set.prototype.has").unwrap_or(Value::undefined()),
            "delete" => self.get_native_func("Set.prototype.delete").unwrap_or(Value::undefined()),
            "clear" => self.get_native_func("Set.prototype.clear").unwrap_or(Value::undefined()),
            "forEach" => self.get_native_func("Set.prototype.forEach").unwrap_or(Value::undefined()),
            "size" => {
                // size is a getter
                let size = self.set_objects.get(set_idx as usize)
                    .map(|s| s.size())
                    .unwrap_or(0);
                Value::int(size as i32)
            }
            _ => Value::undefined(),
        }
    }

    /// Get a property from a builtin object (Math, JSON, etc.)
    fn get_builtin_property(&self, builtin_idx: u32, prop_name: &str) -> Value {
        match builtin_idx {
            BUILTIN_MATH => {
                // Math object properties
                match prop_name {
                    "abs" => self.get_native_func("Math.abs").unwrap_or(Value::undefined()),
                    "floor" => self.get_native_func("Math.floor").unwrap_or(Value::undefined()),
                    "ceil" => self.get_native_func("Math.ceil").unwrap_or(Value::undefined()),
                    "max" => self.get_native_func("Math.max").unwrap_or(Value::undefined()),
                    "min" => self.get_native_func("Math.min").unwrap_or(Value::undefined()),
                    "round" => self.get_native_func("Math.round").unwrap_or(Value::undefined()),
                    "sqrt" => self.get_native_func("Math.sqrt").unwrap_or(Value::undefined()),
                    "pow" => self.get_native_func("Math.pow").unwrap_or(Value::undefined()),
                    "random" => self.get_native_func("Math.random").unwrap_or(Value::undefined()),
                    "sign" => self.get_native_func("Math.sign").unwrap_or(Value::undefined()),
                    "trunc" => self.get_native_func("Math.trunc").unwrap_or(Value::undefined()),
                    "log" => self.get_native_func("Math.log").unwrap_or(Value::undefined()),
                    "log10" => self.get_native_func("Math.log10").unwrap_or(Value::undefined()),
                    "log2" => self.get_native_func("Math.log2").unwrap_or(Value::undefined()),
                    "exp" => self.get_native_func("Math.exp").unwrap_or(Value::undefined()),
                    "clz32" => self.get_native_func("Math.clz32").unwrap_or(Value::undefined()),
                    "imul" => self.get_native_func("Math.imul").unwrap_or(Value::undefined()),
                    "sin" => self.get_native_func("Math.sin").unwrap_or(Value::undefined()),
                    "cos" => self.get_native_func("Math.cos").unwrap_or(Value::undefined()),
                    "tan" => self.get_native_func("Math.tan").unwrap_or(Value::undefined()),
                    "asin" => self.get_native_func("Math.asin").unwrap_or(Value::undefined()),
                    "acos" => self.get_native_func("Math.acos").unwrap_or(Value::undefined()),
                    "atan" => self.get_native_func("Math.atan").unwrap_or(Value::undefined()),
                    "atan2" => self.get_native_func("Math.atan2").unwrap_or(Value::undefined()),
                    "hypot" => self.get_native_func("Math.hypot").unwrap_or(Value::undefined()),
                    "cbrt" => self.get_native_func("Math.cbrt").unwrap_or(Value::undefined()),
                    "expm1" => self.get_native_func("Math.expm1").unwrap_or(Value::undefined()),
                    "log1p" => self.get_native_func("Math.log1p").unwrap_or(Value::undefined()),
                    "fround" => self.get_native_func("Math.fround").unwrap_or(Value::undefined()),
                    // Math constants (scaled by 1000 for precision)
                    "PI" => Value::int(3142),  // 3.14159... * 1000
                    "E" => Value::int(2718),   // 2.71828... * 1000
                    "LN2" => Value::int(693),  // 0.693... * 1000
                    "LN10" => Value::int(2303), // 2.302... * 1000
                    "LOG2E" => Value::int(1443), // 1.442... * 1000
                    "LOG10E" => Value::int(434), // 0.434... * 1000
                    "SQRT2" => Value::int(1414), // 1.414... * 1000
                    "SQRT1_2" => Value::int(707), // 0.707... * 1000
                    _ => Value::undefined(),
                }
            }
            BUILTIN_JSON => {
                // JSON object properties
                match prop_name {
                    "stringify" => self.get_native_func("JSON.stringify").unwrap_or(Value::undefined()),
                    "parse" => self.get_native_func("JSON.parse").unwrap_or(Value::undefined()),
                    _ => Value::undefined(),
                }
            }
            BUILTIN_NUMBER => {
                // Number object properties
                match prop_name {
                    "isInteger" => self.get_native_func("Number.isInteger").unwrap_or(Value::undefined()),
                    "isNaN" => self.get_native_func("Number.isNaN").unwrap_or(Value::undefined()),
                    "isFinite" => self.get_native_func("Number.isFinite").unwrap_or(Value::undefined()),
                    "parseInt" => self.get_native_func("parseInt").unwrap_or(Value::undefined()),
                    // Use 31-bit safe values (our Value::int only supports 31-bit signed integers)
                    "MAX_VALUE" => Value::int((1 << 30) - 1), // 1073741823
                    "MIN_VALUE" => Value::int(-(1 << 30)),    // -1073741824
                    "MAX_SAFE_INTEGER" => Value::int((1 << 30) - 1),
                    "MIN_SAFE_INTEGER" => Value::int(-(1 << 30)),
                    _ => Value::undefined(),
                }
            }
            BUILTIN_BOOLEAN => {
                // Boolean object - currently no static methods
                Value::undefined()
            }
            BUILTIN_CONSOLE => {
                // console object properties
                match prop_name {
                    "log" => self.get_native_func("console.log").unwrap_or(Value::undefined()),
                    "error" => self.get_native_func("console.error").unwrap_or(Value::undefined()),
                    "warn" => self.get_native_func("console.warn").unwrap_or(Value::undefined()),
                    "info" => self.get_native_func("console.info").unwrap_or(Value::undefined()),
                    "debug" => self.get_native_func("console.debug").unwrap_or(Value::undefined()),
                    "trace" => self.get_native_func("console.trace").unwrap_or(Value::undefined()),
                    "assert" => self.get_native_func("console.assert").unwrap_or(Value::undefined()),
                    _ => Value::undefined(),
                }
            }
            BUILTIN_DATE => {
                // Date object properties
                match prop_name {
                    "now" => self.get_native_func("Date.now").unwrap_or(Value::undefined()),
                    _ => Value::undefined(),
                }
            }
            BUILTIN_OBJECT => {
                // Object static methods
                match prop_name {
                    "keys" => self.get_native_func("Object.keys").unwrap_or(Value::undefined()),
                    "values" => self.get_native_func("Object.values").unwrap_or(Value::undefined()),
                    "entries" => self.get_native_func("Object.entries").unwrap_or(Value::undefined()),
                    "assign" => self.get_native_func("Object.assign").unwrap_or(Value::undefined()),
                    "create" => self.get_native_func("Object.create").unwrap_or(Value::undefined()),
                    "freeze" => self.get_native_func("Object.freeze").unwrap_or(Value::undefined()),
                    "seal" => self.get_native_func("Object.seal").unwrap_or(Value::undefined()),
                    "isFrozen" => self.get_native_func("Object.isFrozen").unwrap_or(Value::undefined()),
                    "isSealed" => self.get_native_func("Object.isSealed").unwrap_or(Value::undefined()),
                    "getOwnPropertyNames" => self.get_native_func("Object.getOwnPropertyNames").unwrap_or(Value::undefined()),
                    "fromEntries" => self.get_native_func("Object.fromEntries").unwrap_or(Value::undefined()),
                    "hasOwn" => self.get_native_func("Object.hasOwn").unwrap_or(Value::undefined()),
                    "is" => self.get_native_func("Object.is").unwrap_or(Value::undefined()),
                    "getPrototypeOf" => self.get_native_func("Object.getPrototypeOf").unwrap_or(Value::undefined()),
                    "getOwnPropertyDescriptor" => self.get_native_func("Object.getOwnPropertyDescriptor").unwrap_or(Value::undefined()),
                    "defineProperty" => self.get_native_func("Object.defineProperty").unwrap_or(Value::undefined()),
                    "preventExtensions" => self.get_native_func("Object.preventExtensions").unwrap_or(Value::undefined()),
                    "isExtensible" => self.get_native_func("Object.isExtensible").unwrap_or(Value::undefined()),
                    _ => Value::undefined(),
                }
            }
            BUILTIN_ARRAY => {
                // Array static methods
                match prop_name {
                    "isArray" => self.get_native_func("Array.isArray").unwrap_or(Value::undefined()),
                    "from" => self.get_native_func("Array.from").unwrap_or(Value::undefined()),
                    "of" => self.get_native_func("Array.of").unwrap_or(Value::undefined()),
                    _ => Value::undefined(),
                }
            }
            BUILTIN_STRING => {
                // String static methods
                match prop_name {
                    "fromCharCode" => self.get_native_func("String.fromCharCode").unwrap_or(Value::undefined()),
                    "fromCodePoint" => self.get_native_func("String.fromCodePoint").unwrap_or(Value::undefined()),
                    _ => Value::undefined(),
                }
            }
            BUILTIN_GLOBAL_THIS => {
                // globalThis provides access to all global objects and functions
                match prop_name {
                    "undefined" => Value::undefined(),
                    "NaN" => Value::int(0),
                    "Infinity" => Value::int(i32::MAX),
                    "Math" => Value::builtin_object(BUILTIN_MATH),
                    "JSON" => Value::builtin_object(BUILTIN_JSON),
                    "Number" => Value::builtin_object(BUILTIN_NUMBER),
                    "Boolean" => Value::builtin_object(BUILTIN_BOOLEAN),
                    "String" => Value::builtin_object(BUILTIN_STRING),
                    "Object" => Value::builtin_object(BUILTIN_OBJECT),
                    "Array" => Value::builtin_object(BUILTIN_ARRAY),
                    "console" => Value::builtin_object(BUILTIN_CONSOLE),
                    "Date" => Value::builtin_object(BUILTIN_DATE),
                    "Error" => Value::builtin_object(BUILTIN_ERROR),
                    "RegExp" => Value::builtin_object(BUILTIN_REGEXP),
                    "Map" => Value::builtin_object(BUILTIN_MAP),
                    "Set" => Value::builtin_object(BUILTIN_SET),
                    "globalThis" => Value::builtin_object(BUILTIN_GLOBAL_THIS),
                    // Global functions
                    "parseInt" => self.get_native_func("parseInt").unwrap_or(Value::undefined()),
                    "parseFloat" => self.get_native_func("parseFloat").unwrap_or(Value::undefined()),
                    "isNaN" => self.get_native_func("isNaN").unwrap_or(Value::undefined()),
                    "isFinite" => self.get_native_func("isFinite").unwrap_or(Value::undefined()),
                    "encodeURIComponent" => self.get_native_func("encodeURIComponent").unwrap_or(Value::undefined()),
                    "decodeURIComponent" => self.get_native_func("decodeURIComponent").unwrap_or(Value::undefined()),
                    "encodeURI" => self.get_native_func("encodeURI").unwrap_or(Value::undefined()),
                    "decodeURI" => self.get_native_func("decodeURI").unwrap_or(Value::undefined()),
                    _ => Value::undefined(),
                }
            }
            _ => Value::undefined(),
        }
    }

    /// Call a native function by index
    fn call_native_func(&mut self, idx: u32, this: Value, args: &[Value]) -> InterpreterResult<Value> {
        let func = self.native_functions.get(idx as usize)
            .ok_or_else(|| InterpreterError::InternalError(format!("invalid native function index: {}", idx)))?
            .clone();

        (func.func)(self, this, args)
            .map_err(|e| InterpreterError::TypeError(e))
    }

    /// Call a builtin object as a function (e.g., Boolean(value), Number(value))
    fn call_builtin_as_function(&mut self, builtin_idx: u32, args: &[Value]) -> InterpreterResult<Value> {
        match builtin_idx {
            BUILTIN_BOOLEAN => {
                // Boolean(value) - coerces value to boolean
                let arg = args.first().copied().unwrap_or(Value::undefined());
                Ok(Value::bool(self.to_boolean(arg)))
            }
            BUILTIN_NUMBER => {
                // Number(value) - coerces value to number
                let arg = args.first().copied().unwrap_or(Value::undefined());
                Ok(self.to_number(arg))
            }
            BUILTIN_STRING => {
                // String(value) - coerces value to string
                let arg = args.first().copied().unwrap_or(Value::undefined());
                Ok(self.to_string_value(arg))
            }
            _ => Err(InterpreterError::TypeError(format!(
                "Builtin {} is not callable as a function",
                builtin_idx
            ))),
        }
    }

    /// Convert a value to boolean
    fn to_boolean(&self, val: Value) -> bool {
        if val.is_undefined() || val.is_null() {
            false
        } else if let Some(b) = val.to_bool() {
            b
        } else if let Some(n) = val.to_i32() {
            n != 0
        } else if let Some(str_idx) = val.to_string_idx() {
            // Empty string is falsy
            if let Some(s) = self.get_string_by_idx(str_idx) {
                !s.is_empty()
            } else {
                true
            }
        } else {
            // Objects, arrays, closures are truthy
            true
        }
    }

    /// Convert a value to number
    fn to_number(&self, val: Value) -> Value {
        if let Some(n) = val.to_i32() {
            Value::int(n)
        } else if let Some(b) = val.to_bool() {
            Value::int(if b { 1 } else { 0 })
        } else if val.is_undefined() {
            Value::int(0) // Should be NaN but we use 0
        } else if val.is_null() {
            Value::int(0)
        } else if let Some(str_idx) = val.to_string_idx() {
            // Try to parse string as number
            if let Some(s) = self.get_string_by_idx(str_idx) {
                s.trim().parse::<i32>().map(Value::int).unwrap_or(Value::int(0))
            } else {
                Value::int(0)
            }
        } else {
            Value::int(0) // Should be NaN for objects
        }
    }

    /// Convert a value to string
    fn to_string_value(&mut self, val: Value) -> Value {
        let s = if val.is_undefined() {
            "undefined".to_string()
        } else if val.is_null() {
            "null".to_string()
        } else if let Some(b) = val.to_bool() {
            b.to_string()
        } else if let Some(n) = val.to_i32() {
            n.to_string()
        } else if val.to_string_idx().is_some() {
            // Already a string - return as-is
            return val;
        } else if val.is_array() {
            "[object Array]".to_string()
        } else if val.is_object() {
            "[object Object]".to_string()
        } else if val.is_closure() {
            "[object Function]".to_string()
        } else {
            "".to_string()
        };
        self.create_runtime_string(s)
    }

    /// Register built-in native functions
    fn register_builtins(&mut self) {
        // Array methods
        self.register_native("Array.prototype.push", native_array_push, 0);
        self.register_native("Array.prototype.pop", native_array_pop, 0);
        self.register_native("Array.prototype.length", native_array_length, 0);
        self.register_native("Array.prototype.shift", native_array_shift, 0);
        self.register_native("Array.prototype.unshift", native_array_unshift, 0);
        self.register_native("Array.prototype.indexOf", native_array_index_of, 1);
        self.register_native("Array.prototype.join", native_array_join, 0);
        self.register_native("Array.prototype.reverse", native_array_reverse, 0);
        self.register_native("Array.prototype.slice", native_array_slice, 0);
        self.register_native("Array.prototype.map", native_array_map, 1);
        self.register_native("Array.prototype.filter", native_array_filter, 1);
        self.register_native("Array.prototype.forEach", native_array_foreach, 1);
        self.register_native("Array.prototype.reduce", native_array_reduce, 1);
        self.register_native("Array.prototype.reduceRight", native_array_reduce_right, 1);
        self.register_native("Array.prototype.find", native_array_find, 1);
        self.register_native("Array.prototype.findIndex", native_array_find_index, 1);
        self.register_native("Array.prototype.some", native_array_some, 1);
        self.register_native("Array.prototype.every", native_array_every, 1);
        self.register_native("Array.prototype.includes", native_array_includes, 1);
        self.register_native("Array.prototype.concat", native_array_concat, 0);
        self.register_native("Array.prototype.sort", native_array_sort, 0);
        self.register_native("Array.prototype.flat", native_array_flat, 0);
        self.register_native("Array.prototype.fill", native_array_fill, 1);
        self.register_native("Array.prototype.splice", native_array_splice, 2);
        self.register_native("Array.prototype.lastIndexOf", native_array_last_index_of, 1);
        self.register_native("Array.prototype.flatMap", native_array_flat_map, 1);
        self.register_native("Array.prototype.copyWithin", native_array_copy_within, 2);
        self.register_native("Array.prototype.at", native_array_at, 1);
        self.register_native("Array.prototype.findLast", native_array_find_last, 1);
        self.register_native("Array.prototype.findLastIndex", native_array_find_last_index, 1);
        self.register_native("Array.prototype.toSorted", native_array_to_sorted, 0);
        self.register_native("Array.prototype.toReversed", native_array_to_reversed, 0);
        self.register_native("Array.prototype.with", native_array_with, 2);
        self.register_native("Array.prototype.toSpliced", native_array_to_spliced, 2);
        self.register_native("Array.prototype.toString", native_array_to_string, 0);
        self.register_native("Array.prototype.keys", native_array_keys, 0);
        self.register_native("Array.prototype.values", native_array_values, 0);
        self.register_native("Array.prototype.entries", native_array_entries, 0);

        // Global functions
        self.register_native("parseInt", native_parse_int, 1);
        self.register_native("isNaN", native_is_nan, 1);
        self.register_native("isFinite", native_is_finite, 1);
        self.register_native("parseFloat", native_parse_float, 1);
        self.register_native("encodeURIComponent", native_encode_uri_component, 1);
        self.register_native("decodeURIComponent", native_decode_uri_component, 1);
        self.register_native("encodeURI", native_encode_uri, 1);
        self.register_native("decodeURI", native_decode_uri, 1);

        // Math functions
        self.register_native("Math.abs", native_math_abs, 1);
        self.register_native("Math.floor", native_math_floor, 1);
        self.register_native("Math.ceil", native_math_ceil, 1);
        self.register_native("Math.round", native_math_round, 1);
        self.register_native("Math.sqrt", native_math_sqrt, 1);
        self.register_native("Math.pow", native_math_pow, 2);
        self.register_native("Math.max", native_math_max, 0);
        self.register_native("Math.min", native_math_min, 0);
        self.register_native("Math.random", native_math_random, 0);
        self.register_native("Math.sign", native_math_sign, 1);
        self.register_native("Math.trunc", native_math_trunc, 1);
        self.register_native("Math.log", native_math_log, 1);
        self.register_native("Math.log10", native_math_log10, 1);
        self.register_native("Math.log2", native_math_log2, 1);
        self.register_native("Math.exp", native_math_exp, 1);
        self.register_native("Math.clz32", native_math_clz32, 1);
        self.register_native("Math.imul", native_math_imul, 2);
        self.register_native("Math.sin", native_math_sin, 1);
        self.register_native("Math.cos", native_math_cos, 1);
        self.register_native("Math.tan", native_math_tan, 1);
        self.register_native("Math.asin", native_math_asin, 1);
        self.register_native("Math.acos", native_math_acos, 1);
        self.register_native("Math.atan", native_math_atan, 1);
        self.register_native("Math.atan2", native_math_atan2, 2);
        self.register_native("Math.hypot", native_math_hypot, 0);
        self.register_native("Math.cbrt", native_math_cbrt, 1);
        self.register_native("Math.expm1", native_math_expm1, 1);
        self.register_native("Math.log1p", native_math_log1p, 1);
        self.register_native("Math.fround", native_math_fround, 1);

        // Number methods
        self.register_native("Number.prototype.toFixed", native_number_to_fixed, 1);
        self.register_native("Number.prototype.toString", native_number_to_string, 1);

        // String methods
        self.register_native("String.prototype.charAt", native_string_char_at, 1);
        self.register_native("String.prototype.indexOf", native_string_index_of, 1);
        self.register_native("String.prototype.slice", native_string_slice, 0);
        self.register_native("String.prototype.substring", native_string_substring, 0);
        self.register_native("String.prototype.toUpperCase", native_string_to_upper_case, 0);
        self.register_native("String.prototype.toLowerCase", native_string_to_lower_case, 0);
        self.register_native("String.prototype.trim", native_string_trim, 0);
        self.register_native("String.prototype.split", native_string_split, 0);
        self.register_native("String.prototype.concat", native_string_concat, 0);
        self.register_native("String.prototype.repeat", native_string_repeat, 1);
        self.register_native("String.prototype.startsWith", native_string_starts_with, 1);
        self.register_native("String.prototype.endsWith", native_string_ends_with, 1);
        self.register_native("String.prototype.padStart", native_string_pad_start, 1);
        self.register_native("String.prototype.padEnd", native_string_pad_end, 1);
        self.register_native("String.prototype.replace", native_string_replace, 2);
        self.register_native("String.prototype.includes", native_string_includes, 1);
        self.register_native("String.prototype.match", native_string_match, 1);
        self.register_native("String.prototype.search", native_string_search, 1);
        self.register_native("String.prototype.lastIndexOf", native_string_last_index_of, 1);
        self.register_native("String.prototype.trimStart", native_string_trim_start, 0);
        self.register_native("String.prototype.trimEnd", native_string_trim_end, 0);
        self.register_native("String.prototype.replaceAll", native_string_replace_all, 2);
        self.register_native("String.prototype.at", native_string_at, 1);
        self.register_native("String.prototype.charCodeAt", native_string_char_code_at, 1);
        self.register_native("String.prototype.codePointAt", native_string_code_point_at, 1);
        self.register_native("String.fromCharCode", native_string_from_char_code, 0);
        self.register_native("String.fromCodePoint", native_string_from_code_point, 0);

        // Number static methods
        self.register_native("Number.isInteger", native_number_is_integer, 1);
        self.register_native("Number.isNaN", native_number_is_nan, 1);
        self.register_native("Number.isFinite", native_number_is_finite, 1);

        // console methods
        self.register_native("console.log", native_console_log, 0);
        self.register_native("console.error", native_console_error, 0);
        self.register_native("console.warn", native_console_warn, 0);
        self.register_native("console.info", native_console_info, 0);
        self.register_native("console.debug", native_console_debug, 0);
        self.register_native("console.trace", native_console_trace, 0);
        self.register_native("console.assert", native_console_assert, 1);

        // JSON methods
        self.register_native("JSON.stringify", native_json_stringify, 1);
        self.register_native("JSON.parse", native_json_parse, 1);

        // Date methods
        self.register_native("Date.now", native_date_now, 0);

        // RegExp methods
        self.register_native("RegExp.prototype.test", native_regexp_test, 1);
        self.register_native("RegExp.prototype.exec", native_regexp_exec, 1);

        // Map methods
        self.register_native("Map.prototype.get", native_map_get, 1);
        self.register_native("Map.prototype.set", native_map_set, 2);
        self.register_native("Map.prototype.has", native_map_has, 1);
        self.register_native("Map.prototype.delete", native_map_delete, 1);
        self.register_native("Map.prototype.clear", native_map_clear, 0);
        self.register_native("Map.prototype.size", native_map_size, 0);
        self.register_native("Map.prototype.forEach", native_map_foreach, 1);

        // Set methods
        self.register_native("Set.prototype.add", native_set_add, 1);
        self.register_native("Set.prototype.has", native_set_has, 1);
        self.register_native("Set.prototype.delete", native_set_delete, 1);
        self.register_native("Set.prototype.clear", native_set_clear, 0);
        self.register_native("Set.prototype.size", native_set_size, 0);
        self.register_native("Set.prototype.forEach", native_set_foreach, 1);

        // Object static methods
        self.register_native("Object.keys", native_object_keys, 1);
        self.register_native("Object.values", native_object_values, 1);
        self.register_native("Object.entries", native_object_entries, 1);
        self.register_native("Object.assign", native_object_assign, 2);
        self.register_native("Object.create", native_object_create, 1);
        self.register_native("Object.freeze", native_object_freeze, 1);
        self.register_native("Object.seal", native_object_seal, 1);
        self.register_native("Object.isFrozen", native_object_is_frozen, 1);
        self.register_native("Object.isSealed", native_object_is_sealed, 1);
        self.register_native("Object.getOwnPropertyNames", native_object_get_own_property_names, 1);
        self.register_native("Object.fromEntries", native_object_from_entries, 1);
        self.register_native("Object.hasOwn", native_object_has_own, 2);
        self.register_native("Object.prototype.hasOwnProperty", native_object_prototype_has_own_property, 1);
        self.register_native("Object.prototype.toString", native_object_prototype_to_string, 0);
        self.register_native("Object.prototype.valueOf", native_object_prototype_value_of, 0);
        self.register_native("Object.is", native_object_is, 2);
        self.register_native("Object.getPrototypeOf", native_object_get_prototype_of, 1);
        self.register_native("Object.getOwnPropertyDescriptor", native_object_get_own_property_descriptor, 2);
        self.register_native("Object.defineProperty", native_object_define_property, 3);
        self.register_native("Object.preventExtensions", native_object_prevent_extensions, 1);
        self.register_native("Object.isExtensible", native_object_is_extensible, 1);

        // Array static methods
        self.register_native("Array.isArray", native_array_is_array, 1);
        self.register_native("Array.from", native_array_from, 1);
        self.register_native("Array.of", native_array_of, 0);

        // Function.prototype methods
        self.register_native("Function.prototype.call", native_function_call, 0);
        self.register_native("Function.prototype.apply", native_function_apply, 0);
        self.register_native("Function.prototype.bind", native_function_bind, 0);
    }
}

// =============================================================================
// Native function implementations
// =============================================================================

/// Array.prototype.push - add elements to end of array
fn native_array_push(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "push called on non-array".to_string())?;

    if let Some(arr) = interp.arrays.get_mut(arr_idx as usize) {
        for arg in args {
            arr.push(*arg);
        }
        Ok(Value::int(arr.len() as i32))
    } else {
        Err("invalid array".to_string())
    }
}

/// Array.prototype.pop - remove and return last element
fn native_array_pop(interp: &mut Interpreter, this: Value, _args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "pop called on non-array".to_string())?;

    if let Some(arr) = interp.arrays.get_mut(arr_idx as usize) {
        Ok(arr.pop().unwrap_or(Value::undefined()))
    } else {
        Err("invalid array".to_string())
    }
}

/// Array.prototype.length - get array length
fn native_array_length(interp: &mut Interpreter, this: Value, _args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "length called on non-array".to_string())?;

    if let Some(arr) = interp.arrays.get(arr_idx as usize) {
        Ok(Value::int(arr.len() as i32))
    } else {
        Err("invalid array".to_string())
    }
}

/// Array.prototype.shift - remove and return first element
fn native_array_shift(interp: &mut Interpreter, this: Value, _args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "shift called on non-array".to_string())?;

    if let Some(arr) = interp.arrays.get_mut(arr_idx as usize) {
        if arr.is_empty() {
            Ok(Value::undefined())
        } else {
            Ok(arr.remove(0))
        }
    } else {
        Err("invalid array".to_string())
    }
}

/// Array.prototype.unshift - add elements to beginning, return new length
fn native_array_unshift(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "unshift called on non-array".to_string())?;

    if let Some(arr) = interp.arrays.get_mut(arr_idx as usize) {
        // Insert arguments at beginning in order
        for (i, arg) in args.iter().enumerate() {
            arr.insert(i, *arg);
        }
        Ok(Value::int(arr.len() as i32))
    } else {
        Err("invalid array".to_string())
    }
}

/// Array.prototype.indexOf - find index of element
fn native_array_index_of(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "indexOf called on non-array".to_string())?;

    let search_val = args.get(0).copied().unwrap_or(Value::undefined());

    if let Some(arr) = interp.arrays.get(arr_idx as usize) {
        for (i, val) in arr.iter().enumerate() {
            // Simple equality check (comparing raw values)
            if val.0 == search_val.0 {
                return Ok(Value::int(i as i32));
            }
        }
        Ok(Value::int(-1)) // Not found
    } else {
        Err("invalid array".to_string())
    }
}

/// Array.prototype.join - join elements with separator
fn native_array_join(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "join called on non-array".to_string())?;

    // Get separator (default is ",")
    let separator = if let Some(sep_val) = args.get(0) {
        if let Some(str_idx) = sep_val.to_string_idx() {
            // We'd need access to string table here, for now use ","
            let _ = str_idx;
            ","
        } else {
            ","
        }
    } else {
        ","
    };

    if let Some(arr) = interp.arrays.get(arr_idx as usize) {
        let parts: Vec<String> = arr.iter().map(|v| {
            if let Some(n) = v.to_i32() {
                n.to_string()
            } else if v.is_undefined() {
                String::new()
            } else if v.is_null() {
                String::new()
            } else if let Some(b) = v.to_bool() {
                b.to_string()
            } else {
                String::new()
            }
        }).collect();

        let result = parts.join(separator);

        // Store result string and return string value
        let str_idx = interp.runtime_strings.len() as u16;
        interp.runtime_strings.push(result);
        Ok(Value::string(str_idx))
    } else {
        Err("invalid array".to_string())
    }
}

/// Array.prototype.reverse - reverse array in place
fn native_array_reverse(interp: &mut Interpreter, this: Value, _args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "reverse called on non-array".to_string())?;

    if let Some(arr) = interp.arrays.get_mut(arr_idx as usize) {
        arr.reverse();
        Ok(this) // Return the array itself
    } else {
        Err("invalid array".to_string())
    }
}

/// Array.prototype.slice - return shallow copy of portion of array
fn native_array_slice(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "slice called on non-array".to_string())?;

    if let Some(arr) = interp.arrays.get(arr_idx as usize) {
        let len = arr.len() as i32;

        // Get start index (default 0)
        let mut start = args.get(0).and_then(|v| v.to_i32()).unwrap_or(0);
        if start < 0 {
            start = (len + start).max(0);
        }
        let start = start.min(len) as usize;

        // Get end index (default length)
        let mut end = args.get(1).and_then(|v| v.to_i32()).unwrap_or(len);
        if end < 0 {
            end = (len + end).max(0);
        }
        let end = end.min(len) as usize;

        // Create new array with slice
        let slice: Vec<Value> = if start < end {
            arr[start..end].to_vec()
        } else {
            Vec::new()
        };

        // Store the new array
        let new_idx = interp.arrays.len() as u32;
        interp.arrays.push(slice);
        Ok(Value::array_idx(new_idx))
    } else {
        Err("invalid array".to_string())
    }
}

/// Array.prototype.map - create new array with callback applied to each element
fn native_array_map(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "map called on non-array".to_string())?;

    let callback = args.first().copied()
        .ok_or_else(|| "map requires a callback function".to_string())?;

    if !callback.is_closure() && callback.to_func_ptr().is_none() {
        return Err("map callback must be a function".to_string());
    }

    // Clone the array to avoid borrow issues
    let arr_clone = interp.arrays.get(arr_idx as usize)
        .ok_or_else(|| "invalid array".to_string())?
        .clone();

    let mut result = Vec::with_capacity(arr_clone.len());

    for (i, element) in arr_clone.iter().enumerate() {
        let call_args = vec![*element, Value::int(i as i32), this];
        let mapped = interp.call_value(callback, Value::undefined(), &call_args)
            .map_err(|e| e.to_string())?;
        result.push(mapped);
    }

    let new_idx = interp.arrays.len() as u32;
    interp.arrays.push(result);
    Ok(Value::array_idx(new_idx))
}

/// Array.prototype.filter - create new array with elements that pass the test
fn native_array_filter(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "filter called on non-array".to_string())?;

    let callback = args.first().copied()
        .ok_or_else(|| "filter requires a callback function".to_string())?;

    if !callback.is_closure() && callback.to_func_ptr().is_none() {
        return Err("filter callback must be a function".to_string());
    }

    // Clone the array to avoid borrow issues
    let arr_clone = interp.arrays.get(arr_idx as usize)
        .ok_or_else(|| "invalid array".to_string())?
        .clone();

    let mut result = Vec::new();

    for (i, element) in arr_clone.iter().enumerate() {
        let call_args = vec![*element, Value::int(i as i32), this];
        let keep = interp.call_value(callback, Value::undefined(), &call_args)
            .map_err(|e| e.to_string())?;

        // Convert to boolean
        if Interpreter::value_to_bool(keep) {
            result.push(*element);
        }
    }

    let new_idx = interp.arrays.len() as u32;
    interp.arrays.push(result);
    Ok(Value::array_idx(new_idx))
}

/// Array.prototype.forEach - call callback for each element
fn native_array_foreach(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "forEach called on non-array".to_string())?;

    let callback = args.first().copied()
        .ok_or_else(|| "forEach requires a callback function".to_string())?;

    if !callback.is_closure() && callback.to_func_ptr().is_none() {
        return Err("forEach callback must be a function".to_string());
    }

    // Clone the array to avoid borrow issues
    let arr_clone = interp.arrays.get(arr_idx as usize)
        .ok_or_else(|| "invalid array".to_string())?
        .clone();

    for (i, element) in arr_clone.iter().enumerate() {
        let call_args = vec![*element, Value::int(i as i32), this];
        interp.call_value(callback, Value::undefined(), &call_args)
            .map_err(|e| e.to_string())?;
    }

    Ok(Value::undefined())
}

/// Array.prototype.reduce - reduce array to single value
fn native_array_reduce(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "reduce called on non-array".to_string())?;

    let callback = args.first().copied()
        .ok_or_else(|| "reduce requires a callback function".to_string())?;

    if !callback.is_closure() && callback.to_func_ptr().is_none() {
        return Err("reduce callback must be a function".to_string());
    }

    // Clone the array to avoid borrow issues
    let arr_clone = interp.arrays.get(arr_idx as usize)
        .ok_or_else(|| "invalid array".to_string())?
        .clone();

    if arr_clone.is_empty() && args.len() < 2 {
        return Err("reduce of empty array with no initial value".to_string());
    }

    // Get initial value or first element
    let (mut accumulator, start_idx) = if args.len() >= 2 {
        (args[1], 0)
    } else {
        (arr_clone[0], 1)
    };

    for (i, element) in arr_clone.iter().enumerate().skip(start_idx) {
        let call_args = vec![accumulator, *element, Value::int(i as i32), this];
        accumulator = interp.call_value(callback, Value::undefined(), &call_args)
            .map_err(|e| e.to_string())?;
    }

    Ok(accumulator)
}

/// Array.prototype.reduceRight - reduce array from right to left
fn native_array_reduce_right(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "reduceRight called on non-array".to_string())?;

    let callback = args.first().copied()
        .ok_or_else(|| "reduceRight requires a callback function".to_string())?;

    if !callback.is_closure() && callback.to_func_ptr().is_none() {
        return Err("reduceRight callback must be a function".to_string());
    }

    // Clone the array to avoid borrow issues
    let arr_clone = interp.arrays.get(arr_idx as usize)
        .ok_or_else(|| "invalid array".to_string())?
        .clone();

    if arr_clone.is_empty() && args.len() < 2 {
        return Err("reduceRight of empty array with no initial value".to_string());
    }

    let len = arr_clone.len();

    // Get initial value or last element
    let (mut accumulator, skip_last) = if args.len() >= 2 {
        (args[1], false)
    } else {
        (arr_clone[len - 1], true)
    };

    // Iterate from right to left
    let iter_range: Box<dyn Iterator<Item = usize>> = if skip_last {
        Box::new((0..len - 1).rev())
    } else {
        Box::new((0..len).rev())
    };

    for i in iter_range {
        let element = arr_clone[i];
        let call_args = vec![accumulator, element, Value::int(i as i32), this];
        accumulator = interp.call_value(callback, Value::undefined(), &call_args)
            .map_err(|e| e.to_string())?;
    }

    Ok(accumulator)
}

/// Array.prototype.find - find first element that satisfies the test
fn native_array_find(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "find called on non-array".to_string())?;

    let callback = args.first().copied()
        .ok_or_else(|| "find requires a callback function".to_string())?;

    if !callback.is_closure() && callback.to_func_ptr().is_none() {
        return Err("find callback must be a function".to_string());
    }

    // Clone the array to avoid borrow issues
    let arr_clone = interp.arrays.get(arr_idx as usize)
        .ok_or_else(|| "invalid array".to_string())?
        .clone();

    for (i, element) in arr_clone.iter().enumerate() {
        let call_args = vec![*element, Value::int(i as i32), this];
        let result = interp.call_value(callback, Value::undefined(), &call_args)
            .map_err(|e| e.to_string())?;

        if Interpreter::value_to_bool(result) {
            return Ok(*element);
        }
    }

    Ok(Value::undefined())
}

/// Array.prototype.findIndex - find index of first element that satisfies the test
fn native_array_find_index(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "findIndex called on non-array".to_string())?;

    let callback = args.first().copied()
        .ok_or_else(|| "findIndex requires a callback function".to_string())?;

    if !callback.is_closure() && callback.to_func_ptr().is_none() {
        return Err("findIndex callback must be a function".to_string());
    }

    // Clone the array to avoid borrow issues
    let arr_clone = interp.arrays.get(arr_idx as usize)
        .ok_or_else(|| "invalid array".to_string())?
        .clone();

    for (i, element) in arr_clone.iter().enumerate() {
        let call_args = vec![*element, Value::int(i as i32), this];
        let result = interp.call_value(callback, Value::undefined(), &call_args)
            .map_err(|e| e.to_string())?;

        if Interpreter::value_to_bool(result) {
            return Ok(Value::int(i as i32));
        }
    }

    Ok(Value::int(-1))
}

/// Array.prototype.some - check if any element satisfies the test
fn native_array_some(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "some called on non-array".to_string())?;

    let callback = args.first().copied()
        .ok_or_else(|| "some requires a callback function".to_string())?;

    if !callback.is_closure() && callback.to_func_ptr().is_none() {
        return Err("some callback must be a function".to_string());
    }

    // Clone the array to avoid borrow issues
    let arr_clone = interp.arrays.get(arr_idx as usize)
        .ok_or_else(|| "invalid array".to_string())?
        .clone();

    for (i, element) in arr_clone.iter().enumerate() {
        let call_args = vec![*element, Value::int(i as i32), this];
        let result = interp.call_value(callback, Value::undefined(), &call_args)
            .map_err(|e| e.to_string())?;

        if Interpreter::value_to_bool(result) {
            return Ok(Value::bool(true));
        }
    }

    Ok(Value::bool(false))
}

/// Array.prototype.every - check if all elements satisfy the test
fn native_array_every(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "every called on non-array".to_string())?;

    let callback = args.first().copied()
        .ok_or_else(|| "every requires a callback function".to_string())?;

    if !callback.is_closure() && callback.to_func_ptr().is_none() {
        return Err("every callback must be a function".to_string());
    }

    // Clone the array to avoid borrow issues
    let arr_clone = interp.arrays.get(arr_idx as usize)
        .ok_or_else(|| "invalid array".to_string())?
        .clone();

    for (i, element) in arr_clone.iter().enumerate() {
        let call_args = vec![*element, Value::int(i as i32), this];
        let result = interp.call_value(callback, Value::undefined(), &call_args)
            .map_err(|e| e.to_string())?;

        if !Interpreter::value_to_bool(result) {
            return Ok(Value::bool(false));
        }
    }

    Ok(Value::bool(true))
}

/// Array.prototype.includes - check if array includes a value
fn native_array_includes(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "includes called on non-array".to_string())?;

    let search_val = args.first().copied().unwrap_or(Value::undefined());

    if let Some(arr) = interp.arrays.get(arr_idx as usize) {
        for element in arr.iter() {
            // Simple equality check
            if element.raw() == search_val.raw() {
                return Ok(Value::bool(true));
            }
        }
        Ok(Value::bool(false))
    } else {
        Err("invalid array".to_string())
    }
}

/// Array.prototype.concat - concatenate arrays
fn native_array_concat(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "concat called on non-array".to_string())?;

    // Clone the original array
    let original = interp.arrays.get(arr_idx as usize)
        .ok_or_else(|| "invalid array".to_string())?
        .clone();

    let mut result = original;

    // Concatenate each argument
    for arg in args {
        if let Some(other_idx) = arg.to_array_idx() {
            // Argument is an array - append all elements
            if let Some(other_arr) = interp.arrays.get(other_idx as usize) {
                result.extend(other_arr.iter().cloned());
            }
        } else {
            // Argument is a single value - append it
            result.push(*arg);
        }
    }

    let new_arr_idx = interp.arrays.len() as u32;
    interp.arrays.push(result);
    Ok(Value::array_idx(new_arr_idx))
}

/// Array.prototype.sort - sort array in place
fn native_array_sort(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "sort called on non-array".to_string())?;

    // Get optional compare function
    let compare_fn = args.first().copied();

    if let Some(arr) = interp.arrays.get_mut(arr_idx as usize) {
        if compare_fn.is_some() && (compare_fn.unwrap().is_closure() || compare_fn.unwrap().to_func_ptr().is_some()) {
            // Custom comparator - need to call the function for each comparison
            // For now, just do default sort without custom comparator support
            // TODO: Implement custom comparator
            arr.sort_by(|a, b| {
                // Default: convert to strings and compare
                let a_val = a.to_i32().unwrap_or(0);
                let b_val = b.to_i32().unwrap_or(0);
                a_val.cmp(&b_val)
            });
        } else {
            // Default sort - numeric comparison for integers
            arr.sort_by(|a, b| {
                let a_val = a.to_i32().unwrap_or(0);
                let b_val = b.to_i32().unwrap_or(0);
                a_val.cmp(&b_val)
            });
        }
    }

    // Return the array itself (sort is in-place)
    Ok(this)
}

/// Array.prototype.flat - flatten nested arrays
fn native_array_flat(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "flat called on non-array".to_string())?;

    // Get depth (default 1)
    let depth = args.first()
        .and_then(|v| v.to_i32())
        .unwrap_or(1)
        .max(0) as usize;

    let original = interp.arrays.get(arr_idx as usize)
        .ok_or_else(|| "invalid array".to_string())?
        .clone();

    fn flatten_recursive(interp: &Interpreter, arr: &[Value], depth: usize) -> Vec<Value> {
        let mut result = Vec::new();
        for elem in arr {
            if depth > 0 {
                if let Some(nested_idx) = elem.to_array_idx() {
                    if let Some(nested) = interp.arrays.get(nested_idx as usize) {
                        result.extend(flatten_recursive(interp, nested, depth - 1));
                        continue;
                    }
                }
            }
            result.push(*elem);
        }
        result
    }

    let flattened = flatten_recursive(interp, &original, depth);

    let new_arr_idx = interp.arrays.len() as u32;
    interp.arrays.push(flattened);
    Ok(Value::array_idx(new_arr_idx))
}

/// Array.prototype.fill - fill array with a value
fn native_array_fill(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "fill called on non-array".to_string())?;

    let fill_value = args.first().copied().unwrap_or(Value::undefined());

    // Get start and end indices
    let arr_len = interp.arrays.get(arr_idx as usize)
        .map(|a| a.len())
        .unwrap_or(0) as i32;

    let start = args.get(1)
        .and_then(|v| v.to_i32())
        .map(|s| if s < 0 { (arr_len + s).max(0) } else { s.min(arr_len) })
        .unwrap_or(0) as usize;

    let end = args.get(2)
        .and_then(|v| v.to_i32())
        .map(|e| if e < 0 { (arr_len + e).max(0) } else { e.min(arr_len) })
        .unwrap_or(arr_len) as usize;

    if let Some(arr) = interp.arrays.get_mut(arr_idx as usize) {
        for i in start..end.min(arr.len()) {
            arr[i] = fill_value;
        }
    }

    // Return the array itself (fill is in-place)
    Ok(this)
}

/// Array.prototype.splice - remove/replace elements and return removed elements
fn native_array_splice(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "splice called on non-array".to_string())?;

    let arr_len = interp.arrays.get(arr_idx as usize)
        .map(|a| a.len())
        .unwrap_or(0) as i32;

    // Get start index (normalize negative values)
    let start = args.get(0)
        .and_then(|v| v.to_i32())
        .map(|s| if s < 0 { (arr_len + s).max(0) } else { s.min(arr_len) })
        .unwrap_or(0) as usize;

    // Get delete count
    let delete_count = args.get(1)
        .and_then(|v| v.to_i32())
        .map(|d| d.max(0).min(arr_len - start as i32) as usize)
        .unwrap_or((arr_len - start as i32).max(0) as usize);

    // Items to insert
    let insert_items: Vec<Value> = args.iter().skip(2).copied().collect();

    // Clone the array to work with
    let removed: Vec<Value>;
    if let Some(arr) = interp.arrays.get_mut(arr_idx as usize) {
        // Remove elements
        let end = (start + delete_count).min(arr.len());
        removed = arr.drain(start..end).collect();

        // Insert new elements
        for (i, item) in insert_items.into_iter().enumerate() {
            arr.insert(start + i, item);
        }
    } else {
        removed = Vec::new();
    }

    // Return array of removed elements
    let result_idx = interp.arrays.len() as u32;
    interp.arrays.push(removed);
    Ok(Value::array_idx(result_idx))
}

/// Array.prototype.lastIndexOf - find last index of element
fn native_array_last_index_of(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "lastIndexOf called on non-array".to_string())?;

    let search_element = args.first().copied().unwrap_or(Value::undefined());

    let arr = interp.arrays.get(arr_idx as usize)
        .map(|a| a.clone())
        .unwrap_or_default();

    // Optional fromIndex
    let from_index = args.get(1)
        .and_then(|v| v.to_i32())
        .map(|i| if i < 0 { (arr.len() as i32 + i).max(-1) } else { i.min(arr.len() as i32 - 1) })
        .unwrap_or(arr.len() as i32 - 1);

    if from_index < 0 {
        return Ok(Value::int(-1));
    }

    // Search backwards
    for i in (0..=from_index as usize).rev() {
        if i < arr.len() {
            let elem = arr[i];
            // Simple value comparison (strict equality)
            let is_equal = elem.0 == search_element.0 ||
                (elem.to_i32().is_some() && elem.to_i32() == search_element.to_i32());
            if is_equal {
                return Ok(Value::int(i as i32));
            }
        }
    }

    Ok(Value::int(-1))
}

/// Array.prototype.flatMap - map then flatten by 1 level
fn native_array_flat_map(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "flatMap called on non-array".to_string())?;

    let callback = args.first().copied()
        .ok_or_else(|| "flatMap requires a callback function".to_string())?;

    let elements: Vec<Value> = interp.get_array(arr_idx)
        .map(|arr| arr.clone())
        .unwrap_or_default();

    let mut result: Vec<Value> = Vec::new();

    for (i, elem) in elements.into_iter().enumerate() {
        // Call the callback
        let mapped = interp.call_value(callback, this, &[elem, Value::int(i as i32), this])
            .map_err(|e| e.to_string())?;

        // Flatten one level if result is an array
        if let Some(mapped_arr_idx) = mapped.to_array_idx() {
            if let Some(inner_arr) = interp.get_array(mapped_arr_idx) {
                result.extend(inner_arr.iter().copied());
            }
        } else {
            result.push(mapped);
        }
    }

    let result_idx = interp.arrays.len() as u32;
    interp.arrays.push(result);
    Ok(Value::array_idx(result_idx))
}

/// Array.prototype.copyWithin - copy array elements within the array
fn native_array_copy_within(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "copyWithin called on non-array".to_string())?;

    let arr_len = interp.arrays.get(arr_idx as usize)
        .map(|a| a.len())
        .unwrap_or(0) as i32;

    if arr_len == 0 {
        return Ok(this);
    }

    // Target index (where to copy to)
    let target = args.get(0)
        .and_then(|v| v.to_i32())
        .map(|t| if t < 0 { (arr_len + t).max(0) } else { t.min(arr_len) })
        .unwrap_or(0) as usize;

    // Start index (where to copy from)
    let start = args.get(1)
        .and_then(|v| v.to_i32())
        .map(|s| if s < 0 { (arr_len + s).max(0) } else { s.min(arr_len) })
        .unwrap_or(0) as usize;

    // End index (where to stop copying from)
    let end = args.get(2)
        .and_then(|v| v.to_i32())
        .map(|e| if e < 0 { (arr_len + e).max(0) } else { e.min(arr_len) })
        .unwrap_or(arr_len) as usize;

    // Calculate actual copy count
    let copy_count = (end.saturating_sub(start)).min(arr_len as usize - target);

    if copy_count == 0 {
        return Ok(this);
    }

    // Copy elements (handle overlapping correctly)
    if let Some(arr) = interp.arrays.get_mut(arr_idx as usize) {
        // Create a temporary copy of the source elements
        let source: Vec<Value> = arr[start..start + copy_count].to_vec();

        // Copy to target position
        for (i, val) in source.into_iter().enumerate() {
            if target + i < arr.len() {
                arr[target + i] = val;
            }
        }
    }

    Ok(this)
}

/// Array.prototype.at - get element at index (supports negative indices)
fn native_array_at(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "at called on non-array".to_string())?;

    let arr = interp.arrays.get(arr_idx as usize)
        .map(|a| a.clone())
        .unwrap_or_default();

    let len = arr.len() as i32;

    let index = args.get(0)
        .and_then(|v| v.to_i32())
        .unwrap_or(0);

    // Normalize negative index
    let actual_index = if index < 0 {
        (len + index) as usize
    } else {
        index as usize
    };

    Ok(arr.get(actual_index).copied().unwrap_or(Value::undefined()))
}

/// Array.prototype.findLast - find last matching element
fn native_array_find_last(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "findLast called on non-array".to_string())?;

    let callback = args.first().copied()
        .ok_or_else(|| "findLast requires a callback function".to_string())?;

    let elements: Vec<Value> = interp.get_array(arr_idx)
        .map(|arr| arr.clone())
        .unwrap_or_default();

    // Iterate in reverse
    for i in (0..elements.len()).rev() {
        let elem = elements[i];
        let result = interp.call_value(callback, this, &[elem, Value::int(i as i32), this])
            .map_err(|e| e.to_string())?;

        if Interpreter::value_to_bool(result) {
            return Ok(elem);
        }
    }

    Ok(Value::undefined())
}

/// Array.prototype.findLastIndex - find index of last matching element
fn native_array_find_last_index(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "findLastIndex called on non-array".to_string())?;

    let callback = args.first().copied()
        .ok_or_else(|| "findLastIndex requires a callback function".to_string())?;

    let elements: Vec<Value> = interp.get_array(arr_idx)
        .map(|arr| arr.clone())
        .unwrap_or_default();

    // Iterate in reverse
    for i in (0..elements.len()).rev() {
        let elem = elements[i];
        let result = interp.call_value(callback, this, &[elem, Value::int(i as i32), this])
            .map_err(|e| e.to_string())?;

        if Interpreter::value_to_bool(result) {
            return Ok(Value::int(i as i32));
        }
    }

    Ok(Value::int(-1))
}

/// Array.prototype.toSorted - returns a new sorted array (ES2023, non-mutating)
fn native_array_to_sorted(interp: &mut Interpreter, this: Value, _args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "toSorted called on non-array".to_string())?;

    // Clone the array
    let mut arr_copy = interp.arrays.get(arr_idx as usize)
        .ok_or_else(|| "invalid array".to_string())?
        .clone();

    // Sort the copy
    arr_copy.sort_by(|a, b| {
        let a_val = a.to_i32().unwrap_or(0);
        let b_val = b.to_i32().unwrap_or(0);
        a_val.cmp(&b_val)
    });

    // Create new array
    let new_arr_idx = interp.arrays.len() as u32;
    interp.arrays.push(arr_copy);
    Ok(Value::array_idx(new_arr_idx))
}

/// Array.prototype.toReversed - returns a new reversed array (ES2023, non-mutating)
fn native_array_to_reversed(interp: &mut Interpreter, this: Value, _args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "toReversed called on non-array".to_string())?;

    // Clone and reverse the array
    let mut arr_copy = interp.arrays.get(arr_idx as usize)
        .ok_or_else(|| "invalid array".to_string())?
        .clone();

    arr_copy.reverse();

    // Create new array
    let new_arr_idx = interp.arrays.len() as u32;
    interp.arrays.push(arr_copy);
    Ok(Value::array_idx(new_arr_idx))
}

/// Array.prototype.with - returns a new array with element at index replaced (ES2023)
fn native_array_with(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "with called on non-array".to_string())?;

    let index = args.get(0).and_then(|v| v.to_i32()).unwrap_or(0);
    let value = args.get(1).copied().unwrap_or(Value::undefined());

    // Clone the array
    let mut arr_copy = interp.arrays.get(arr_idx as usize)
        .ok_or_else(|| "invalid array".to_string())?
        .clone();

    let len = arr_copy.len() as i32;

    // Handle negative indices
    let actual_index = if index < 0 {
        len + index
    } else {
        index
    };

    if actual_index < 0 || actual_index >= len {
        return Err("Invalid index".to_string());
    }

    // Set the value
    arr_copy[actual_index as usize] = value;

    // Create new array
    let new_arr_idx = interp.arrays.len() as u32;
    interp.arrays.push(arr_copy);
    Ok(Value::array_idx(new_arr_idx))
}

/// Array.prototype.toSpliced - returns new array with elements spliced (ES2023, non-mutating)
fn native_array_to_spliced(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "toSpliced called on non-array".to_string())?;

    // Clone the array
    let mut arr_copy = interp.arrays.get(arr_idx as usize)
        .ok_or_else(|| "invalid array".to_string())?
        .clone();

    let len = arr_copy.len() as i32;

    // Get start index
    let start = args.get(0).and_then(|v| v.to_i32()).unwrap_or(0);
    let actual_start = if start < 0 {
        (len + start).max(0) as usize
    } else {
        (start.min(len)) as usize
    };

    // Get delete count
    let delete_count = args.get(1).and_then(|v| v.to_i32())
        .map(|n| n.max(0) as usize)
        .unwrap_or((len - actual_start as i32).max(0) as usize);
    let actual_delete = delete_count.min(arr_copy.len() - actual_start);

    // Get items to insert (remaining arguments)
    let insert_items: Vec<Value> = args.iter().skip(2).copied().collect();

    // Remove elements
    arr_copy.drain(actual_start..actual_start + actual_delete);

    // Insert new elements
    for (i, item) in insert_items.into_iter().enumerate() {
        arr_copy.insert(actual_start + i, item);
    }

    // Create new array
    let new_arr_idx = interp.arrays.len() as u32;
    interp.arrays.push(arr_copy);
    Ok(Value::array_idx(new_arr_idx))
}

/// Array.prototype.toString - returns string representation of array
fn native_array_to_string(interp: &mut Interpreter, this: Value, _args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "toString called on non-array".to_string())?;

    if let Some(arr) = interp.arrays.get(arr_idx as usize) {
        let parts: Vec<String> = arr.iter().map(|v| {
            if let Some(n) = v.to_i32() {
                n.to_string()
            } else if v.is_undefined() {
                String::new()
            } else if v.is_null() {
                String::new()
            } else if let Some(b) = v.to_bool() {
                b.to_string()
            } else if let Some(str_idx) = v.to_string_idx() {
                interp.get_string_by_idx(str_idx)
                    .map(|s| s.to_string())
                    .unwrap_or_default()
            } else {
                String::new()
            }
        }).collect();
        let joined = parts.join(",");
        Ok(interp.create_runtime_string(joined))
    } else {
        Ok(interp.create_runtime_string(String::new()))
    }
}

/// Array.prototype.keys - returns array of indices
fn native_array_keys(interp: &mut Interpreter, this: Value, _args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "keys called on non-array".to_string())?;

    let len = interp.arrays.get(arr_idx as usize)
        .map(|a| a.len())
        .unwrap_or(0);

    // Return an array of indices
    let keys: Vec<Value> = (0..len as i32).map(Value::int).collect();
    let new_arr_idx = interp.arrays.len() as u32;
    interp.arrays.push(keys);
    Ok(Value::array_idx(new_arr_idx))
}

/// Array.prototype.values - returns array of values (iterator-like)
fn native_array_values(interp: &mut Interpreter, this: Value, _args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "values called on non-array".to_string())?;

    // Return a copy of the array (simulates iterator behavior)
    let values = interp.arrays.get(arr_idx as usize)
        .cloned()
        .unwrap_or_default();
    let new_arr_idx = interp.arrays.len() as u32;
    interp.arrays.push(values);
    Ok(Value::array_idx(new_arr_idx))
}

/// Array.prototype.entries - returns array of [index, value] pairs
fn native_array_entries(interp: &mut Interpreter, this: Value, _args: &[Value]) -> Result<Value, String> {
    let arr_idx = this.to_array_idx()
        .ok_or_else(|| "entries called on non-array".to_string())?;

    let elements = interp.arrays.get(arr_idx as usize)
        .cloned()
        .unwrap_or_default();

    // Create array of [index, value] pairs
    let mut entries: Vec<Value> = Vec::new();
    for (i, val) in elements.into_iter().enumerate() {
        // Create pair array [index, value]
        let pair = vec![Value::int(i as i32), val];
        let pair_idx = interp.arrays.len() as u32;
        interp.arrays.push(pair);
        entries.push(Value::array_idx(pair_idx));
    }

    let entries_idx = interp.arrays.len() as u32;
    interp.arrays.push(entries);
    Ok(Value::array_idx(entries_idx))
}

/// parseInt - parse string to integer
fn native_parse_int(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let val = args.get(0).copied().unwrap_or(Value::undefined());

    if let Some(n) = val.to_i32() {
        Ok(Value::int(n))
    } else {
        // Return NaN for non-parseable values (use 0 for now since we don't have NaN)
        Ok(Value::int(0))
    }
}

/// isNaN - check if value is NaN
fn native_is_nan(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let val = args.get(0).copied().unwrap_or(Value::undefined());

    // We don't have real NaN support yet, so just check if it's a number
    if val.to_i32().is_some() {
        Ok(Value::bool(false))
    } else {
        Ok(Value::bool(true))
    }
}

/// isFinite - check if value is a finite number
fn native_is_finite(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let val = args.get(0).copied().unwrap_or(Value::undefined());

    // For our integer-only implementation, all valid integers are finite
    if val.to_i32().is_some() {
        Ok(Value::bool(true))
    } else {
        Ok(Value::bool(false))
    }
}

/// parseFloat - parse string to float
fn native_parse_float(interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let val = args.get(0).copied().unwrap_or(Value::undefined());

    if let Some(n) = val.to_i32() {
        // Already a number
        Ok(Value::int(n))
    } else if let Some(str_idx) = val.to_string_idx() {
        // Try to parse as string
        if let Some(s) = interp.get_string_by_idx(str_idx) {
            let s = s.trim();
            if let Ok(n) = s.parse::<i32>() {
                Ok(Value::int(n))
            } else {
                // Return NaN (represented as 0 for now since we don't have NaN)
                Ok(Value::int(0))
            }
        } else {
            Ok(Value::int(0))
        }
    } else {
        Ok(Value::int(0))
    }
}

/// encodeURIComponent - encode URI component
fn native_encode_uri_component(interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let val = args.first().copied().unwrap_or(Value::undefined());

    let s = if let Some(str_idx) = val.to_string_idx() {
        interp.get_string_by_idx(str_idx)
            .map(|s| s.to_string())
            .unwrap_or_default()
    } else if let Some(n) = val.to_i32() {
        n.to_string()
    } else if val.is_undefined() {
        "undefined".to_string()
    } else if val.is_null() {
        "null".to_string()
    } else {
        String::new()
    };

    // Encode the string - preserve unreserved characters per RFC 3986
    let mut encoded = String::with_capacity(s.len() * 3);
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '!' | '~' | '*' | '\'' | '(' | ')') {
            encoded.push(c);
        } else {
            // Percent-encode the character
            for byte in c.to_string().as_bytes() {
                encoded.push_str(&format!("%{:02X}", byte));
            }
        }
    }

    Ok(interp.create_runtime_string(encoded))
}

/// decodeURIComponent - decode URI component
fn native_decode_uri_component(interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let val = args.first().copied().unwrap_or(Value::undefined());

    let s = if let Some(str_idx) = val.to_string_idx() {
        interp.get_string_by_idx(str_idx)
            .map(|s| s.to_string())
            .unwrap_or_default()
    } else {
        return Ok(interp.create_runtime_string(String::new()));
    };

    // Decode percent-encoded characters
    let mut decoded = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            // Try to parse hex digits
            let hex_str = std::str::from_utf8(&bytes[i+1..i+3]).unwrap_or("");
            if let Ok(byte_val) = u8::from_str_radix(hex_str, 16) {
                decoded.push(byte_val);
                i += 3;
            } else {
                decoded.push(bytes[i]);
                i += 1;
            }
        } else {
            decoded.push(bytes[i]);
            i += 1;
        }
    }

    // Convert bytes back to string
    let result = String::from_utf8_lossy(&decoded).to_string();
    Ok(interp.create_runtime_string(result))
}

/// encodeURI - encode URI (preserves reserved characters)
fn native_encode_uri(interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let val = args.first().copied().unwrap_or(Value::undefined());

    let s = if let Some(str_idx) = val.to_string_idx() {
        interp.get_string_by_idx(str_idx)
            .map(|s| s.to_string())
            .unwrap_or_default()
    } else if let Some(n) = val.to_i32() {
        n.to_string()
    } else {
        String::new()
    };

    // Encode the string - preserve unreserved + reserved characters
    let mut encoded = String::with_capacity(s.len() * 3);
    for c in s.chars() {
        if c.is_ascii_alphanumeric()
            || matches!(c, '-' | '_' | '.' | '!' | '~' | '*' | '\'' | '(' | ')')
            || matches!(c, ';' | ',' | '/' | '?' | ':' | '@' | '&' | '=' | '+' | '$' | '#')
        {
            encoded.push(c);
        } else {
            // Percent-encode the character
            for byte in c.to_string().as_bytes() {
                encoded.push_str(&format!("%{:02X}", byte));
            }
        }
    }

    Ok(interp.create_runtime_string(encoded))
}

/// decodeURI - decode URI
fn native_decode_uri(interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    // For now, same as decodeURIComponent (should preserve reserved chars but we decode all)
    native_decode_uri_component(interp, _this, args)
}

/// Math.abs - absolute value
fn native_math_abs(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let val = args.get(0).copied().unwrap_or(Value::undefined());

    if let Some(n) = val.to_i32() {
        Ok(Value::int(n.abs()))
    } else {
        Err("Math.abs requires a number".to_string())
    }
}

/// Math.floor - floor value
fn native_math_floor(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let val = args.get(0).copied().unwrap_or(Value::undefined());

    if let Some(n) = val.to_i32() {
        Ok(Value::int(n)) // Already an integer
    } else {
        Err("Math.floor requires a number".to_string())
    }
}

/// Math.ceil - ceiling value
fn native_math_ceil(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let val = args.get(0).copied().unwrap_or(Value::undefined());

    if let Some(n) = val.to_i32() {
        Ok(Value::int(n)) // Already an integer
    } else {
        Err("Math.ceil requires a number".to_string())
    }
}

/// Math.max - maximum of values
fn native_math_max(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    if args.is_empty() {
        return Ok(Value::int(i32::MIN)); // -Infinity for no args
    }

    let mut max = i32::MIN;
    for arg in args {
        if let Some(n) = arg.to_i32() {
            if n > max {
                max = n;
            }
        } else {
            return Err("Math.max requires numbers".to_string());
        }
    }
    Ok(Value::int(max))
}

/// Math.min - minimum of values
fn native_math_min(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    if args.is_empty() {
        return Ok(Value::int(i32::MAX)); // Infinity for no args
    }

    let mut min = i32::MAX;
    for arg in args {
        if let Some(n) = arg.to_i32() {
            if n < min {
                min = n;
            }
        } else {
            return Err("Math.min requires numbers".to_string());
        }
    }
    Ok(Value::int(min))
}

/// Math.round - round to nearest integer
fn native_math_round(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let val = args.get(0).copied().unwrap_or(Value::undefined());

    if let Some(n) = val.to_i32() {
        Ok(Value::int(n)) // Already an integer
    } else {
        Err("Math.round requires a number".to_string())
    }
}

/// Math.sqrt - square root (integer approximation for now)
fn native_math_sqrt(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let val = args.get(0).copied().unwrap_or(Value::undefined());

    if let Some(n) = val.to_i32() {
        if n < 0 {
            Ok(Value::int(0)) // NaN for negative (return 0 for now)
        } else {
            // Integer square root
            Ok(Value::int((n as f64).sqrt() as i32))
        }
    } else {
        Err("Math.sqrt requires a number".to_string())
    }
}

/// Math.pow - power function (integer only for now)
fn native_math_pow(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let base = args.get(0).copied().unwrap_or(Value::undefined());
    let exp = args.get(1).copied().unwrap_or(Value::undefined());

    if let (Some(b), Some(e)) = (base.to_i32(), exp.to_i32()) {
        if e < 0 {
            Ok(Value::int(0)) // Integer division for negative exponents
        } else if e == 0 {
            Ok(Value::int(1))
        } else {
            let result = (b as i64).pow(e as u32);
            Ok(Value::int(result.min(i32::MAX as i64).max(i32::MIN as i64) as i32))
        }
    } else {
        Err("Math.pow requires numbers".to_string())
    }
}

/// Math.random - return random number between 0 and 1 (returns integer 0 or 1 for now)
fn native_math_random(_interp: &mut Interpreter, _this: Value, _args: &[Value]) -> Result<Value, String> {
    // Simple pseudo-random using system time
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    // Return a pseudo-random integer 0-999 (scaled to simulate 0-1 range)
    Ok(Value::int((nanos % 1000) as i32))
}

/// Math.sign - return sign of number (-1, 0, or 1)
fn native_math_sign(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let n = args.get(0).and_then(|v| v.to_i32()).unwrap_or(0);
    Ok(Value::int(n.signum()))
}

/// Math.trunc - truncate decimal part (integer already, so identity for i32)
fn native_math_trunc(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let n = args.get(0).and_then(|v| v.to_i32()).unwrap_or(0);
    Ok(Value::int(n))
}

/// Math.log - natural logarithm (integer approximation)
fn native_math_log(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let n = args.get(0).and_then(|v| v.to_i32()).unwrap_or(0);
    if n <= 0 {
        Ok(Value::int(i32::MIN)) // Represents -Infinity/NaN
    } else {
        // Integer approximation of ln(n)
        Ok(Value::int((n as f64).ln() as i32))
    }
}

/// Math.log10 - base 10 logarithm (integer approximation)
fn native_math_log10(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let n = args.get(0).and_then(|v| v.to_i32()).unwrap_or(0);
    if n <= 0 {
        Ok(Value::int(i32::MIN))
    } else {
        Ok(Value::int((n as f64).log10() as i32))
    }
}

/// Math.log2 - base 2 logarithm (integer approximation)
fn native_math_log2(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let n = args.get(0).and_then(|v| v.to_i32()).unwrap_or(0);
    if n <= 0 {
        Ok(Value::int(i32::MIN))
    } else {
        Ok(Value::int((n as f64).log2() as i32))
    }
}

/// Math.exp - e^x (integer approximation)
fn native_math_exp(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let n = args.get(0).and_then(|v| v.to_i32()).unwrap_or(0);
    let result = (n as f64).exp();
    if result > i32::MAX as f64 {
        Ok(Value::int(i32::MAX))
    } else {
        Ok(Value::int(result as i32))
    }
}

/// Math.clz32 - count leading zeros in 32-bit integer
fn native_math_clz32(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let n = args.get(0).and_then(|v| v.to_i32()).unwrap_or(0) as u32;
    Ok(Value::int(n.leading_zeros() as i32))
}

/// Math.imul - 32-bit integer multiplication
fn native_math_imul(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let a = args.get(0).and_then(|v| v.to_i32()).unwrap_or(0);
    let b = args.get(1).and_then(|v| v.to_i32()).unwrap_or(0);
    Ok(Value::int(a.wrapping_mul(b)))
}

/// Math.sin - sine function (returns scaled integer * 1000)
fn native_math_sin(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let x = args.get(0).and_then(|v| v.to_i32()).unwrap_or(0) as f64;
    // Input is in degrees, scale result to integer range (-1000 to 1000)
    let result = (x.to_radians().sin() * 1000.0).round() as i32;
    Ok(Value::int(result))
}

/// Math.cos - cosine function (returns scaled integer * 1000)
fn native_math_cos(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let x = args.get(0).and_then(|v| v.to_i32()).unwrap_or(0) as f64;
    let result = (x.to_radians().cos() * 1000.0).round() as i32;
    Ok(Value::int(result))
}

/// Math.tan - tangent function (returns scaled integer * 1000)
fn native_math_tan(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let x = args.get(0).and_then(|v| v.to_i32()).unwrap_or(0) as f64;
    let tan_val = x.to_radians().tan();
    // Clamp to prevent overflow
    let result = (tan_val * 1000.0).clamp(i32::MIN as f64, i32::MAX as f64).round() as i32;
    Ok(Value::int(result))
}

/// Math.asin - arc sine (input scaled by 1000, returns degrees)
fn native_math_asin(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let x = args.get(0).and_then(|v| v.to_i32()).unwrap_or(0) as f64 / 1000.0;
    let x = x.clamp(-1.0, 1.0);
    let result = x.asin().to_degrees().round() as i32;
    Ok(Value::int(result))
}

/// Math.acos - arc cosine (input scaled by 1000, returns degrees)
fn native_math_acos(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let x = args.get(0).and_then(|v| v.to_i32()).unwrap_or(0) as f64 / 1000.0;
    let x = x.clamp(-1.0, 1.0);
    let result = x.acos().to_degrees().round() as i32;
    Ok(Value::int(result))
}

/// Math.atan - arc tangent (input scaled by 1000, returns degrees)
fn native_math_atan(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let x = args.get(0).and_then(|v| v.to_i32()).unwrap_or(0) as f64 / 1000.0;
    let result = x.atan().to_degrees().round() as i32;
    Ok(Value::int(result))
}

/// Math.atan2 - arc tangent of y/x (returns degrees)
fn native_math_atan2(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let y = args.get(0).and_then(|v| v.to_i32()).unwrap_or(0) as f64;
    let x = args.get(1).and_then(|v| v.to_i32()).unwrap_or(0) as f64;
    let result = y.atan2(x).to_degrees().round() as i32;
    Ok(Value::int(result))
}

/// Math.hypot - hypotenuse (sqrt of sum of squares)
fn native_math_hypot(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let mut sum = 0.0f64;
    for arg in args {
        if let Some(n) = arg.to_i32() {
            sum += (n as f64).powi(2);
        }
    }
    Ok(Value::int(sum.sqrt().round() as i32))
}

/// Math.cbrt - cube root
fn native_math_cbrt(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let x = args.get(0).and_then(|v| v.to_i32()).unwrap_or(0) as f64;
    Ok(Value::int(x.cbrt().round() as i32))
}

/// Math.expm1 - e^x - 1
fn native_math_expm1(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let x = args.get(0).and_then(|v| v.to_i32()).unwrap_or(0) as f64;
    let result = x.exp_m1().clamp(i32::MIN as f64, i32::MAX as f64).round() as i32;
    Ok(Value::int(result))
}

/// Math.log1p - ln(1 + x)
fn native_math_log1p(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let x = args.get(0).and_then(|v| v.to_i32()).unwrap_or(0) as f64;
    let result = x.ln_1p().round() as i32;
    Ok(Value::int(result))
}

/// Math.fround - round to nearest 32-bit float (returns integer approximation)
fn native_math_fround(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let x = args.get(0).and_then(|v| v.to_i32()).unwrap_or(0);
    // For integer-only implementation, just return the value
    Ok(Value::int(x))
}

// =============================================================================
// Number.prototype methods
// =============================================================================

/// Number.prototype.toFixed - format number with fixed decimal places
fn native_number_to_fixed(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let num = if let Some(n) = this.to_i32() {
        n as f64
    } else {
        return Err("toFixed called on non-number".to_string());
    };

    let digits = args.get(0).and_then(|v| v.to_i32()).unwrap_or(0);

    if digits < 0 || digits > 100 {
        return Err("toFixed() digits argument must be between 0 and 100".to_string());
    }

    // Format with fixed decimal places using integer math
    let multiplier = 10_i64.pow(digits as u32);
    let scaled = (num * multiplier as f64).round() as i64;

    let result = if digits == 0 {
        format!("{}", scaled)
    } else {
        let int_part = scaled / multiplier;
        let frac_part = (scaled % multiplier).abs();
        format!("{}.{:0>width$}", int_part, frac_part, width = digits as usize)
    };

    Ok(interp.create_runtime_string(result))
}

/// Number.prototype.toString - convert number to string with optional radix
fn native_number_to_string(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let num = if let Some(n) = this.to_i32() {
        n
    } else {
        return Err("toString called on non-number".to_string());
    };

    let radix = args.get(0).and_then(|v| v.to_i32()).unwrap_or(10);

    if radix < 2 || radix > 36 {
        return Err("toString() radix must be between 2 and 36".to_string());
    }

    let result = if radix == 10 {
        format!("{}", num)
    } else {
        // Convert to string with given radix
        let mut n = num.abs() as u32;
        let mut digits = Vec::new();

        if n == 0 {
            digits.push('0');
        } else {
            while n > 0 {
                let digit = (n % radix as u32) as u8;
                let c = if digit < 10 {
                    (b'0' + digit) as char
                } else {
                    (b'a' + digit - 10) as char
                };
                digits.push(c);
                n /= radix as u32;
            }
        }

        digits.reverse();
        let s: String = digits.into_iter().collect();
        if num < 0 {
            format!("-{}", s)
        } else {
            s
        }
    };

    Ok(interp.create_runtime_string(result))
}

// =============================================================================
// String.prototype methods
// =============================================================================

/// String.prototype.charAt - get character at index
fn native_string_char_at(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let str_idx = this.to_string_idx()
        .ok_or_else(|| "charAt called on non-string".to_string())?;

    let s = interp.get_string_by_idx(str_idx)
        .ok_or_else(|| "invalid string".to_string())?;

    let index = args.get(0).and_then(|v| v.to_i32()).unwrap_or(0) as usize;

    if index < s.len() {
        // Get the character at index (for ASCII strings)
        let ch = s.chars().nth(index).map(|c| c.to_string()).unwrap_or_default();
        let new_idx = interp.runtime_strings.len() as u16 + Interpreter::RUNTIME_STRING_OFFSET;
        interp.runtime_strings.push(ch);
        Ok(Value::string(new_idx))
    } else {
        // Return empty string for out of bounds
        let new_idx = interp.runtime_strings.len() as u16 + Interpreter::RUNTIME_STRING_OFFSET;
        interp.runtime_strings.push(String::new());
        Ok(Value::string(new_idx))
    }
}

/// String.prototype.indexOf - find substring
fn native_string_index_of(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let str_idx = this.to_string_idx()
        .ok_or_else(|| "indexOf called on non-string".to_string())?;

    let s = interp.get_string_by_idx(str_idx)
        .ok_or_else(|| "invalid string".to_string())?;

    // Get search string
    let search = if let Some(search_val) = args.get(0) {
        if let Some(search_idx) = search_val.to_string_idx() {
            interp.get_string_by_idx(search_idx).unwrap_or("").to_string()
        } else if let Some(n) = search_val.to_i32() {
            n.to_string()
        } else {
            return Ok(Value::int(-1));
        }
    } else {
        return Ok(Value::int(-1));
    };

    // Find the substring
    match s.find(&search) {
        Some(pos) => Ok(Value::int(pos as i32)),
        None => Ok(Value::int(-1)),
    }
}

/// String.prototype.slice - extract portion of string
fn native_string_slice(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let str_idx = this.to_string_idx()
        .ok_or_else(|| "slice called on non-string".to_string())?;

    let s = interp.get_string_by_idx(str_idx)
        .ok_or_else(|| "invalid string".to_string())?;

    let len = s.len() as i32;

    // Get start index
    let mut start = args.get(0).and_then(|v| v.to_i32()).unwrap_or(0);
    if start < 0 {
        start = (len + start).max(0);
    }
    let start = start.min(len) as usize;

    // Get end index
    let mut end = args.get(1).and_then(|v| v.to_i32()).unwrap_or(len);
    if end < 0 {
        end = (len + end).max(0);
    }
    let end = end.min(len) as usize;

    // Extract slice
    let result = if start < end {
        s[start..end].to_string()
    } else {
        String::new()
    };

    let new_idx = interp.runtime_strings.len() as u16 + Interpreter::RUNTIME_STRING_OFFSET;
    interp.runtime_strings.push(result);
    Ok(Value::string(new_idx))
}

/// String.prototype.substring - extract portion of string (similar to slice but different negative handling)
fn native_string_substring(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let str_idx = this.to_string_idx()
        .ok_or_else(|| "substring called on non-string".to_string())?;

    let s = interp.get_string_by_idx(str_idx)
        .ok_or_else(|| "invalid string".to_string())?;

    let len = s.len() as i32;

    // Get start index (negative becomes 0)
    let start = args.get(0).and_then(|v| v.to_i32()).unwrap_or(0).max(0).min(len) as usize;

    // Get end index (negative becomes 0)
    let end = args.get(1).and_then(|v| v.to_i32()).unwrap_or(len).max(0).min(len) as usize;

    // Swap if start > end
    let (start, end) = if start > end { (end, start) } else { (start, end) };

    let result = s[start..end].to_string();

    let new_idx = interp.runtime_strings.len() as u16 + Interpreter::RUNTIME_STRING_OFFSET;
    interp.runtime_strings.push(result);
    Ok(Value::string(new_idx))
}

/// String.prototype.toUpperCase - convert to uppercase
fn native_string_to_upper_case(interp: &mut Interpreter, this: Value, _args: &[Value]) -> Result<Value, String> {
    let str_idx = this.to_string_idx()
        .ok_or_else(|| "toUpperCase called on non-string".to_string())?;

    let s = interp.get_string_by_idx(str_idx)
        .ok_or_else(|| "invalid string".to_string())?;

    let result = s.to_uppercase();

    let new_idx = interp.runtime_strings.len() as u16 + Interpreter::RUNTIME_STRING_OFFSET;
    interp.runtime_strings.push(result);
    Ok(Value::string(new_idx))
}

/// String.prototype.toLowerCase - convert to lowercase
fn native_string_to_lower_case(interp: &mut Interpreter, this: Value, _args: &[Value]) -> Result<Value, String> {
    let str_idx = this.to_string_idx()
        .ok_or_else(|| "toLowerCase called on non-string".to_string())?;

    let s = interp.get_string_by_idx(str_idx)
        .ok_or_else(|| "invalid string".to_string())?;

    let result = s.to_lowercase();

    let new_idx = interp.runtime_strings.len() as u16 + Interpreter::RUNTIME_STRING_OFFSET;
    interp.runtime_strings.push(result);
    Ok(Value::string(new_idx))
}

/// String.prototype.trim - remove whitespace from both ends
fn native_string_trim(interp: &mut Interpreter, this: Value, _args: &[Value]) -> Result<Value, String> {
    let str_idx = this.to_string_idx()
        .ok_or_else(|| "trim called on non-string".to_string())?;

    let s = interp.get_string_by_idx(str_idx)
        .ok_or_else(|| "invalid string".to_string())?;

    let result = s.trim().to_string();

    let new_idx = interp.runtime_strings.len() as u16 + Interpreter::RUNTIME_STRING_OFFSET;
    interp.runtime_strings.push(result);
    Ok(Value::string(new_idx))
}

/// String.prototype.split - split string into array
fn native_string_split(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let str_idx = this.to_string_idx()
        .ok_or_else(|| "split called on non-string".to_string())?;

    // Clone the string to avoid borrow issues
    let s = interp.get_string_by_idx(str_idx)
        .ok_or_else(|| "invalid string".to_string())?
        .to_string();

    // Get separator
    let separator = if let Some(sep_val) = args.get(0) {
        if let Some(sep_idx) = sep_val.to_string_idx() {
            interp.get_string_by_idx(sep_idx).unwrap_or(",").to_string()
        } else {
            ",".to_string()
        }
    } else {
        // No separator - return array with whole string
        let new_str_idx = interp.runtime_strings.len() as u16 + Interpreter::RUNTIME_STRING_OFFSET;
        interp.runtime_strings.push(s);

        let arr_idx = interp.arrays.len() as u32;
        interp.arrays.push(vec![Value::string(new_str_idx)]);
        return Ok(Value::array_idx(arr_idx));
    };

    // Split and create array of strings
    let string_parts: Vec<String> = s.split(&separator).map(|p| p.to_string()).collect();
    let mut parts: Vec<Value> = Vec::with_capacity(string_parts.len());
    for part in string_parts {
        let new_str_idx = interp.runtime_strings.len() as u16 + Interpreter::RUNTIME_STRING_OFFSET;
        interp.runtime_strings.push(part);
        parts.push(Value::string(new_str_idx));
    }

    let arr_idx = interp.arrays.len() as u32;
    interp.arrays.push(parts);
    Ok(Value::array_idx(arr_idx))
}

/// String.prototype.concat - concatenate strings
fn native_string_concat(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let str_idx = this.to_string_idx()
        .ok_or_else(|| "concat called on non-string".to_string())?;

    let mut result = interp.get_string_by_idx(str_idx)
        .ok_or_else(|| "invalid string".to_string())?
        .to_string();

    // Concatenate all arguments
    for arg in args {
        if let Some(arg_idx) = arg.to_string_idx() {
            if let Some(arg_str) = interp.get_string_by_idx(arg_idx) {
                result.push_str(arg_str);
            }
        } else if let Some(n) = arg.to_i32() {
            result.push_str(&n.to_string());
        } else if arg.is_undefined() {
            result.push_str("undefined");
        } else if arg.is_null() {
            result.push_str("null");
        } else if arg.is_bool() {
            result.push_str(if arg.to_bool().unwrap_or(false) { "true" } else { "false" });
        }
    }

    let new_str_idx = interp.runtime_strings.len() as u16 + Interpreter::RUNTIME_STRING_OFFSET;
    interp.runtime_strings.push(result);
    Ok(Value::string(new_str_idx))
}

/// String.prototype.repeat - repeat string n times
fn native_string_repeat(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let str_idx = this.to_string_idx()
        .ok_or_else(|| "repeat called on non-string".to_string())?;

    let s = interp.get_string_by_idx(str_idx)
        .ok_or_else(|| "invalid string".to_string())?
        .to_string();

    let count = args.get(0)
        .and_then(|v| v.to_i32())
        .unwrap_or(0)
        .max(0) as usize;

    let result = s.repeat(count);

    let new_str_idx = interp.runtime_strings.len() as u16 + Interpreter::RUNTIME_STRING_OFFSET;
    interp.runtime_strings.push(result);
    Ok(Value::string(new_str_idx))
}

/// String.prototype.startsWith - check if string starts with search string
fn native_string_starts_with(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let str_idx = this.to_string_idx()
        .ok_or_else(|| "startsWith called on non-string".to_string())?;

    let s = interp.get_string_by_idx(str_idx)
        .ok_or_else(|| "invalid string".to_string())?;

    let search = if let Some(search_val) = args.get(0) {
        if let Some(search_idx) = search_val.to_string_idx() {
            interp.get_string_by_idx(search_idx).unwrap_or("").to_string()
        } else {
            return Ok(Value::bool(false));
        }
    } else {
        return Ok(Value::bool(false));
    };

    // Optional position argument
    let position = args.get(1)
        .and_then(|v| v.to_i32())
        .unwrap_or(0)
        .max(0) as usize;

    if position >= s.len() {
        return Ok(Value::bool(search.is_empty()));
    }

    Ok(Value::bool(s[position..].starts_with(&search)))
}

/// String.prototype.endsWith - check if string ends with search string
fn native_string_ends_with(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let str_idx = this.to_string_idx()
        .ok_or_else(|| "endsWith called on non-string".to_string())?;

    let s = interp.get_string_by_idx(str_idx)
        .ok_or_else(|| "invalid string".to_string())?;

    let search = if let Some(search_val) = args.get(0) {
        if let Some(search_idx) = search_val.to_string_idx() {
            interp.get_string_by_idx(search_idx).unwrap_or("").to_string()
        } else {
            return Ok(Value::bool(false));
        }
    } else {
        return Ok(Value::bool(false));
    };

    // Optional end position argument
    let end_position = args.get(1)
        .and_then(|v| v.to_i32())
        .map(|v| v.max(0) as usize)
        .unwrap_or(s.len());

    let end = end_position.min(s.len());

    if search.len() > end {
        return Ok(Value::bool(false));
    }

    Ok(Value::bool(s[..end].ends_with(&search)))
}

/// String.prototype.padStart - pad string from start to target length
fn native_string_pad_start(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let str_idx = this.to_string_idx()
        .ok_or_else(|| "padStart called on non-string".to_string())?;

    let s = interp.get_string_by_idx(str_idx)
        .ok_or_else(|| "invalid string".to_string())?
        .to_string();

    let target_length = args.get(0)
        .and_then(|v| v.to_i32())
        .unwrap_or(0)
        .max(0) as usize;

    if s.len() >= target_length {
        let new_str_idx = interp.runtime_strings.len() as u16 + Interpreter::RUNTIME_STRING_OFFSET;
        interp.runtime_strings.push(s);
        return Ok(Value::string(new_str_idx));
    }

    let pad_string = if let Some(pad_val) = args.get(1) {
        if let Some(pad_idx) = pad_val.to_string_idx() {
            interp.get_string_by_idx(pad_idx).unwrap_or(" ").to_string()
        } else {
            " ".to_string()
        }
    } else {
        " ".to_string()
    };

    if pad_string.is_empty() {
        let new_str_idx = interp.runtime_strings.len() as u16 + Interpreter::RUNTIME_STRING_OFFSET;
        interp.runtime_strings.push(s);
        return Ok(Value::string(new_str_idx));
    }

    let pad_needed = target_length - s.len();
    let full_pads = pad_needed / pad_string.len();
    let partial_pad = pad_needed % pad_string.len();

    let mut result = pad_string.repeat(full_pads);
    result.push_str(&pad_string[..partial_pad]);
    result.push_str(&s);

    let new_str_idx = interp.runtime_strings.len() as u16 + Interpreter::RUNTIME_STRING_OFFSET;
    interp.runtime_strings.push(result);
    Ok(Value::string(new_str_idx))
}

/// String.prototype.padEnd - pad string from end to target length
fn native_string_pad_end(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let str_idx = this.to_string_idx()
        .ok_or_else(|| "padEnd called on non-string".to_string())?;

    let s = interp.get_string_by_idx(str_idx)
        .ok_or_else(|| "invalid string".to_string())?
        .to_string();

    let target_length = args.get(0)
        .and_then(|v| v.to_i32())
        .unwrap_or(0)
        .max(0) as usize;

    if s.len() >= target_length {
        let new_str_idx = interp.runtime_strings.len() as u16 + Interpreter::RUNTIME_STRING_OFFSET;
        interp.runtime_strings.push(s);
        return Ok(Value::string(new_str_idx));
    }

    let pad_string = if let Some(pad_val) = args.get(1) {
        if let Some(pad_idx) = pad_val.to_string_idx() {
            interp.get_string_by_idx(pad_idx).unwrap_or(" ").to_string()
        } else {
            " ".to_string()
        }
    } else {
        " ".to_string()
    };

    if pad_string.is_empty() {
        let new_str_idx = interp.runtime_strings.len() as u16 + Interpreter::RUNTIME_STRING_OFFSET;
        interp.runtime_strings.push(s);
        return Ok(Value::string(new_str_idx));
    }

    let pad_needed = target_length - s.len();
    let full_pads = pad_needed / pad_string.len();
    let partial_pad = pad_needed % pad_string.len();

    let mut result = s;
    result.push_str(&pad_string.repeat(full_pads));
    result.push_str(&pad_string[..partial_pad]);

    let new_str_idx = interp.runtime_strings.len() as u16 + Interpreter::RUNTIME_STRING_OFFSET;
    interp.runtime_strings.push(result);
    Ok(Value::string(new_str_idx))
}

/// String.prototype.replace - replace first occurrence of search with replacement
fn native_string_replace(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let str_idx = this.to_string_idx()
        .ok_or_else(|| "replace called on non-string".to_string())?;

    let s = interp.get_string_by_idx(str_idx)
        .ok_or_else(|| "invalid string".to_string())?
        .to_string();

    let search = if let Some(search_val) = args.get(0) {
        if let Some(search_idx) = search_val.to_string_idx() {
            interp.get_string_by_idx(search_idx).unwrap_or("").to_string()
        } else {
            "".to_string()
        }
    } else {
        "".to_string()
    };

    let replacement = if let Some(replace_val) = args.get(1) {
        if let Some(replace_idx) = replace_val.to_string_idx() {
            interp.get_string_by_idx(replace_idx).unwrap_or("").to_string()
        } else {
            "".to_string()
        }
    } else {
        "".to_string()
    };

    // Replace first occurrence only
    let result = s.replacen(&search, &replacement, 1);

    let new_str_idx = interp.runtime_strings.len() as u16 + Interpreter::RUNTIME_STRING_OFFSET;
    interp.runtime_strings.push(result);
    Ok(Value::string(new_str_idx))
}

/// String.prototype.includes - check if string contains search string
fn native_string_includes(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let str_idx = this.to_string_idx()
        .ok_or_else(|| "includes called on non-string".to_string())?;

    let s = interp.get_string_by_idx(str_idx)
        .ok_or_else(|| "invalid string".to_string())?;

    let search = if let Some(search_val) = args.get(0) {
        if let Some(search_idx) = search_val.to_string_idx() {
            interp.get_string_by_idx(search_idx).unwrap_or("").to_string()
        } else {
            return Ok(Value::bool(false));
        }
    } else {
        return Ok(Value::bool(true)); // includes() with no args returns true
    };

    // Optional position argument
    let position = args.get(1)
        .and_then(|v| v.to_i32())
        .unwrap_or(0)
        .max(0) as usize;

    if position >= s.len() {
        return Ok(Value::bool(search.is_empty()));
    }

    Ok(Value::bool(s[position..].contains(&search)))
}

/// String.prototype.match - match string against a RegExp
fn native_string_match(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let str_idx = this.to_string_idx()
        .ok_or_else(|| "match called on non-string".to_string())?;

    let s = interp.get_string_by_idx(str_idx)
        .ok_or_else(|| "invalid string".to_string())?
        .to_string();

    // Get the RegExp argument
    let regex_arg = args.get(0).copied().unwrap_or(Value::undefined());

    // Check if it's a RegExp object
    if let Some(regex_idx) = regex_arg.to_regexp_object_idx() {
        let re = interp.regex_objects.get(regex_idx as usize)
            .ok_or_else(|| "invalid RegExp object".to_string())?
            .clone();

        if re.global {
            // Global match - return array of all matches
            let matches: Vec<String> = re.regex.find_iter(&s)
                .map(|m| m.as_str().to_string())
                .collect();

            if matches.is_empty() {
                return Ok(Value::null());
            }

            // Create array of matched strings
            let mut result_arr: Vec<Value> = Vec::with_capacity(matches.len());
            for matched in matches {
                let str_idx = interp.runtime_strings.len() as u16 + Interpreter::RUNTIME_STRING_OFFSET;
                interp.runtime_strings.push(matched);
                result_arr.push(Value::string(str_idx));
            }

            let arr_idx = interp.arrays.len() as u32;
            interp.arrays.push(result_arr);
            Ok(Value::array_idx(arr_idx))
        } else {
            // Non-global match - return first match with groups (like exec)
            if let Some(m) = re.regex.find(&s) {
                let matched = m.as_str().to_string();
                let str_idx = interp.runtime_strings.len() as u16 + Interpreter::RUNTIME_STRING_OFFSET;
                interp.runtime_strings.push(matched);

                let arr_idx = interp.arrays.len() as u32;
                interp.arrays.push(vec![Value::string(str_idx)]);
                Ok(Value::array_idx(arr_idx))
            } else {
                Ok(Value::null())
            }
        }
    } else if let Some(pattern_idx) = regex_arg.to_string_idx() {
        // String argument - convert to RegExp
        let pattern = interp.get_string_by_idx(pattern_idx)
            .ok_or_else(|| "invalid pattern string".to_string())?
            .to_string();

        match regex::Regex::new(&pattern) {
            Ok(re) => {
                if let Some(m) = re.find(&s) {
                    let matched = m.as_str().to_string();
                    let str_idx = interp.runtime_strings.len() as u16 + Interpreter::RUNTIME_STRING_OFFSET;
                    interp.runtime_strings.push(matched);

                    let arr_idx = interp.arrays.len() as u32;
                    interp.arrays.push(vec![Value::string(str_idx)]);
                    Ok(Value::array_idx(arr_idx))
                } else {
                    Ok(Value::null())
                }
            }
            Err(_) => Ok(Value::null()),
        }
    } else {
        Ok(Value::null())
    }
}

/// String.prototype.search - search for a match and return index
fn native_string_search(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let str_idx = this.to_string_idx()
        .ok_or_else(|| "search called on non-string".to_string())?;

    let s = interp.get_string_by_idx(str_idx)
        .ok_or_else(|| "invalid string".to_string())?
        .to_string();

    // Get the RegExp argument
    let regex_arg = args.get(0).copied().unwrap_or(Value::undefined());

    // Check if it's a RegExp object
    if let Some(regex_idx) = regex_arg.to_regexp_object_idx() {
        let re = interp.regex_objects.get(regex_idx as usize)
            .ok_or_else(|| "invalid RegExp object".to_string())?
            .clone();

        if let Some(m) = re.regex.find(&s) {
            Ok(Value::int(m.start() as i32))
        } else {
            Ok(Value::int(-1))
        }
    } else if let Some(pattern_idx) = regex_arg.to_string_idx() {
        // String argument - convert to RegExp
        let pattern = interp.get_string_by_idx(pattern_idx)
            .ok_or_else(|| "invalid pattern string".to_string())?
            .to_string();

        match regex::Regex::new(&pattern) {
            Ok(re) => {
                if let Some(m) = re.find(&s) {
                    Ok(Value::int(m.start() as i32))
                } else {
                    Ok(Value::int(-1))
                }
            }
            Err(_) => Ok(Value::int(-1)),
        }
    } else {
        Ok(Value::int(-1))
    }
}

/// String.prototype.lastIndexOf - find last index of substring
fn native_string_last_index_of(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let str_idx = this.to_string_idx()
        .ok_or_else(|| "lastIndexOf called on non-string".to_string())?;

    let s = interp.get_string_by_idx(str_idx)
        .ok_or_else(|| "invalid string".to_string())?
        .to_string();

    let search = if let Some(search_val) = args.get(0) {
        if let Some(search_idx) = search_val.to_string_idx() {
            interp.get_string_by_idx(search_idx).unwrap_or("").to_string()
        } else if let Some(n) = search_val.to_i32() {
            n.to_string()
        } else {
            return Ok(Value::int(-1));
        }
    } else {
        return Ok(Value::int(-1));
    };

    if search.is_empty() {
        return Ok(Value::int(s.len() as i32));
    }

    // Optional position argument
    let position = args.get(1)
        .and_then(|v| v.to_i32())
        .map(|p| p.max(0) as usize)
        .unwrap_or(s.len());

    // Search backwards from position
    let search_end = (position + search.len()).min(s.len());
    if let Some(pos) = s[..search_end].rfind(&search) {
        Ok(Value::int(pos as i32))
    } else {
        Ok(Value::int(-1))
    }
}

/// String.prototype.trimStart - remove leading whitespace
fn native_string_trim_start(interp: &mut Interpreter, this: Value, _args: &[Value]) -> Result<Value, String> {
    let str_idx = this.to_string_idx()
        .ok_or_else(|| "trimStart called on non-string".to_string())?;

    let s = interp.get_string_by_idx(str_idx)
        .ok_or_else(|| "invalid string".to_string())?;

    let trimmed = s.trim_start().to_string();

    let new_str_idx = interp.runtime_strings.len() as u16 + Interpreter::RUNTIME_STRING_OFFSET;
    interp.runtime_strings.push(trimmed);
    Ok(Value::string(new_str_idx))
}

/// String.prototype.trimEnd - remove trailing whitespace
fn native_string_trim_end(interp: &mut Interpreter, this: Value, _args: &[Value]) -> Result<Value, String> {
    let str_idx = this.to_string_idx()
        .ok_or_else(|| "trimEnd called on non-string".to_string())?;

    let s = interp.get_string_by_idx(str_idx)
        .ok_or_else(|| "invalid string".to_string())?;

    let trimmed = s.trim_end().to_string();

    let new_str_idx = interp.runtime_strings.len() as u16 + Interpreter::RUNTIME_STRING_OFFSET;
    interp.runtime_strings.push(trimmed);
    Ok(Value::string(new_str_idx))
}

/// String.prototype.replaceAll - replace all occurrences
fn native_string_replace_all(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let str_idx = this.to_string_idx()
        .ok_or_else(|| "replaceAll called on non-string".to_string())?;

    let s = interp.get_string_by_idx(str_idx)
        .ok_or_else(|| "invalid string".to_string())?
        .to_string();

    // Get search string
    let search = if let Some(search_val) = args.get(0) {
        if let Some(search_idx) = search_val.to_string_idx() {
            interp.get_string_by_idx(search_idx).unwrap_or("").to_string()
        } else {
            return Ok(this); // Return original if search is not a string
        }
    } else {
        return Ok(this);
    };

    // Get replacement string
    let replacement = if let Some(replace_val) = args.get(1) {
        if let Some(replace_idx) = replace_val.to_string_idx() {
            interp.get_string_by_idx(replace_idx).unwrap_or("").to_string()
        } else if let Some(n) = replace_val.to_i32() {
            n.to_string()
        } else {
            "undefined".to_string()
        }
    } else {
        "undefined".to_string()
    };

    let result = s.replace(&search, &replacement);

    let new_str_idx = interp.runtime_strings.len() as u16 + Interpreter::RUNTIME_STRING_OFFSET;
    interp.runtime_strings.push(result);
    Ok(Value::string(new_str_idx))
}

/// String.prototype.at - get character at index (supports negative indices)
fn native_string_at(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let str_idx = this.to_string_idx()
        .ok_or_else(|| "at called on non-string".to_string())?;

    let s = interp.get_string_by_idx(str_idx)
        .ok_or_else(|| "invalid string".to_string())?;

    let len = s.chars().count() as i32;

    let index = args.get(0)
        .and_then(|v| v.to_i32())
        .unwrap_or(0);

    // Normalize negative index
    let actual_index = if index < 0 {
        if (len + index) < 0 {
            return Ok(Value::undefined());
        }
        (len + index) as usize
    } else {
        index as usize
    };

    if let Some(c) = s.chars().nth(actual_index) {
        let result = c.to_string();
        let new_str_idx = interp.runtime_strings.len() as u16 + Interpreter::RUNTIME_STRING_OFFSET;
        interp.runtime_strings.push(result);
        Ok(Value::string(new_str_idx))
    } else {
        Ok(Value::undefined())
    }
}

/// String.prototype.charCodeAt - get character code at index
fn native_string_char_code_at(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let str_idx = this.to_string_idx()
        .ok_or_else(|| "charCodeAt called on non-string".to_string())?;

    let s = interp.get_string_by_idx(str_idx)
        .ok_or_else(|| "invalid string".to_string())?;

    let index = args.get(0)
        .and_then(|v| v.to_i32())
        .unwrap_or(0) as usize;

    if let Some(c) = s.chars().nth(index) {
        Ok(Value::int(c as i32))
    } else {
        // Return NaN in real JS, but we use -1 as sentinel
        Ok(Value::int(-1))
    }
}

/// String.fromCharCode - create string from char codes
fn native_string_from_char_code(interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let mut result = String::new();

    for arg in args {
        if let Some(code) = arg.to_i32() {
            if code >= 0 && code <= 0x10FFFF {
                if let Some(c) = char::from_u32(code as u32) {
                    result.push(c);
                }
            }
        }
    }

    let new_str_idx = interp.runtime_strings.len() as u16 + Interpreter::RUNTIME_STRING_OFFSET;
    interp.runtime_strings.push(result);
    Ok(Value::string(new_str_idx))
}

/// String.prototype.codePointAt - get unicode code point at position
fn native_string_code_point_at(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let str_idx = this.to_string_idx()
        .ok_or_else(|| "codePointAt called on non-string".to_string())?;

    let s = interp.get_string_by_idx(str_idx)
        .ok_or_else(|| "invalid string".to_string())?
        .to_string();

    let index = args.first().and_then(|v| v.to_i32()).unwrap_or(0) as usize;

    // Get the code point at the given index
    if let Some(c) = s.chars().nth(index) {
        Ok(Value::int(c as i32))
    } else {
        Ok(Value::undefined())
    }
}

/// String.fromCodePoint - create string from code points
fn native_string_from_code_point(interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let mut result = String::new();

    for arg in args {
        if let Some(code) = arg.to_i32() {
            if code >= 0 && code <= 0x10FFFF {
                if let Some(c) = char::from_u32(code as u32) {
                    result.push(c);
                } else {
                    return Err("Invalid code point".to_string());
                }
            } else {
                return Err("Invalid code point".to_string());
            }
        }
    }

    let new_str_idx = interp.runtime_strings.len() as u16 + Interpreter::RUNTIME_STRING_OFFSET;
    interp.runtime_strings.push(result);
    Ok(Value::string(new_str_idx))
}

// =============================================================================
// Number static methods
// =============================================================================

/// Number.isInteger - check if value is an integer
fn native_number_is_integer(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let val = args.get(0).copied().unwrap_or(Value::undefined());

    // In our implementation, all numbers are integers (32-bit signed)
    if val.to_i32().is_some() {
        Ok(Value::bool(true))
    } else {
        Ok(Value::bool(false))
    }
}

/// Number.isNaN - check if value is NaN
fn native_number_is_nan(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let val = args.get(0).copied().unwrap_or(Value::undefined());

    // NaN is represented as a special value in our implementation
    // For now, we don't have true NaN, so this returns false for numbers
    if val.is_undefined() || val.is_null() {
        Ok(Value::bool(false)) // undefined/null are not NaN per spec
    } else if val.to_i32().is_some() {
        Ok(Value::bool(false)) // Regular numbers are not NaN
    } else if val.to_bool().is_some() {
        Ok(Value::bool(false)) // Booleans are not NaN
    } else {
        Ok(Value::bool(false)) // For now, nothing is NaN
    }
}

/// Number.isFinite - check if value is a finite number
fn native_number_is_finite(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let val = args.get(0).copied().unwrap_or(Value::undefined());

    // All our integers are finite (we don't have Infinity representation yet)
    if val.to_i32().is_some() {
        Ok(Value::bool(true))
    } else {
        Ok(Value::bool(false))
    }
}

// =============================================================================
// console methods
// =============================================================================

/// console.log - print values to stdout
fn native_console_log(interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let output = format_console_args(interp, args);
    println!("{}", output);
    Ok(Value::undefined())
}

/// console.error - print values to stderr
fn native_console_error(interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let output = format_console_args(interp, args);
    eprintln!("{}", output);
    Ok(Value::undefined())
}

/// console.warn - print values to stderr with warning
fn native_console_warn(interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let output = format_console_args(interp, args);
    eprintln!("{}", output);
    Ok(Value::undefined())
}

/// console.info - print values to stdout (alias for log)
fn native_console_info(interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let output = format_console_args(interp, args);
    println!("{}", output);
    Ok(Value::undefined())
}

/// console.debug - print values to stdout (alias for log)
fn native_console_debug(interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let output = format_console_args(interp, args);
    println!("{}", output);
    Ok(Value::undefined())
}

/// console.trace - print stack trace
fn native_console_trace(interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let output = format_console_args(interp, args);
    if !output.is_empty() {
        println!("Trace: {}", output);
    } else {
        println!("Trace");
    }
    // Print call stack (simplified)
    for (i, frame) in interp.call_stack.iter().rev().enumerate() {
        println!("  at frame {}", i);
        let _ = frame; // We don't have function names stored
    }
    Ok(Value::undefined())
}

/// console.assert - conditionally print error message
fn native_console_assert(interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let condition = args.first().and_then(|v| v.to_bool()).unwrap_or(false);
    if !condition {
        let message = if args.len() > 1 {
            format_console_args(interp, &args[1..])
        } else {
            "Assertion failed".to_string()
        };
        eprintln!("Assertion failed: {}", message);
    }
    Ok(Value::undefined())
}

/// Format arguments for console output
fn format_console_args(interp: &Interpreter, args: &[Value]) -> String {
    args.iter()
        .map(|v| format_value(interp, *v))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Format a single value for output
fn format_value(interp: &Interpreter, val: Value) -> String {
    if let Some(n) = val.to_i32() {
        n.to_string()
    } else if let Some(b) = val.to_bool() {
        b.to_string()
    } else if val.is_null() {
        "null".to_string()
    } else if val.is_undefined() {
        "undefined".to_string()
    } else if let Some(str_idx) = val.to_string_idx() {
        if let Some(s) = interp.get_string_by_idx(str_idx) {
            s.to_string()
        } else {
            // Compile-time string - can't look up without bytecode
            "<string>".to_string()
        }
    } else if val.is_array() {
        if let Some(arr_idx) = val.to_array_idx() {
            if let Some(arr) = interp.arrays.get(arr_idx as usize) {
                let items: Vec<String> = arr.iter().map(|v| format_value(interp, *v)).collect();
                format!("[{}]", items.join(", "))
            } else {
                "[Array]".to_string()
            }
        } else {
            "[Array]".to_string()
        }
    } else if val.is_error_object() {
        if let Some(err_idx) = val.to_error_object_idx() {
            if let Some(err) = interp.error_objects.get(err_idx as usize) {
                if err.message.is_empty() {
                    err.name.clone()
                } else {
                    format!("{}: {}", err.name, err.message)
                }
            } else {
                "Error".to_string()
            }
        } else {
            "Error".to_string()
        }
    } else if val.is_object() {
        "[object Object]".to_string()
    } else if val.is_closure() {
        "[Function]".to_string()
    } else {
        format!("{:?}", val)
    }
}

// ===========================================
// JSON Functions
// ===========================================

/// JSON.stringify - convert a value to a JSON string
fn native_json_stringify(interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    if args.is_empty() {
        return Ok(Value::undefined());
    }
    let val = args[0];
    let json_str = json_stringify_value(interp, val);
    Ok(interp.create_runtime_string(json_str))
}

/// Helper function to stringify a value to JSON format
fn json_stringify_value(interp: &Interpreter, val: Value) -> String {
    if let Some(n) = val.to_i32() {
        n.to_string()
    } else if let Some(b) = val.to_bool() {
        b.to_string()
    } else if val.is_null() {
        "null".to_string()
    } else if val.is_undefined() {
        // undefined values are excluded in JSON.stringify
        "undefined".to_string()
    } else if let Some(str_idx) = val.to_string_idx() {
        if let Some(s) = interp.get_string_by_idx(str_idx) {
            // Escape the string for JSON
            format!("\"{}\"", escape_json_string(s))
        } else {
            "\"\"".to_string()
        }
    } else if val.is_array() {
        if let Some(arr_idx) = val.to_array_idx() {
            if let Some(arr) = interp.arrays.get(arr_idx as usize) {
                let items: Vec<String> = arr.iter()
                    .map(|v| {
                        let s = json_stringify_value(interp, *v);
                        // Replace undefined with null in arrays
                        if s == "undefined" { "null".to_string() } else { s }
                    })
                    .collect();
                format!("[{}]", items.join(","))
            } else {
                "[]".to_string()
            }
        } else {
            "[]".to_string()
        }
    } else if val.is_object() {
        if let Some(obj_idx) = val.to_object_idx() {
            if let Some(obj) = interp.objects.get(obj_idx as usize) {
                let items: Vec<String> = obj.properties.iter()
                    .filter_map(|(k, v)| {
                        let val_str = json_stringify_value(interp, *v);
                        // Skip undefined values in objects
                        if val_str == "undefined" {
                            None
                        } else {
                            Some(format!("\"{}\":{}", escape_json_string(k), val_str))
                        }
                    })
                    .collect();
                format!("{{{}}}", items.join(","))
            } else {
                "{}".to_string()
            }
        } else {
            "{}".to_string()
        }
    } else if val.is_closure() {
        // Functions are excluded in JSON.stringify
        "undefined".to_string()
    } else {
        "null".to_string()
    }
}

/// Escape a string for JSON output
fn escape_json_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if c < ' ' => result.push_str(&format!("\\u{:04x}", c as u32)),
            c => result.push(c),
        }
    }
    result
}

/// JSON.parse - parse a JSON string into a value
fn native_json_parse(interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    if args.is_empty() {
        return Err("JSON.parse requires a string argument".to_string());
    }
    let val = args[0];

    // Get the string to parse
    let json_str = if let Some(str_idx) = val.to_string_idx() {
        if let Some(s) = interp.get_string_by_idx(str_idx) {
            s.to_string()
        } else {
            return Err("Invalid string argument".to_string());
        }
    } else if let Some(n) = val.to_i32() {
        // Numbers can be parsed as JSON
        return Ok(Value::int(n));
    } else {
        return Err("JSON.parse requires a string argument".to_string());
    };

    // Parse the JSON string
    let mut parser = JsonParser::new(&json_str);
    parser.parse_value(interp)
}

/// Simple JSON parser
struct JsonParser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> JsonParser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn parse_value(&mut self, interp: &mut Interpreter) -> Result<Value, String> {
        self.skip_whitespace();

        if self.pos >= self.input.len() {
            return Err("Unexpected end of JSON input".to_string());
        }

        let c = self.peek_char();
        match c {
            '"' => self.parse_string(interp),
            '[' => self.parse_array(interp),
            '{' => self.parse_object(interp),
            't' | 'f' => self.parse_boolean(),
            'n' => self.parse_null(),
            '-' | '0'..='9' => self.parse_number(),
            _ => Err(format!("Unexpected character '{}' in JSON", c)),
        }
    }

    fn peek_char(&self) -> char {
        self.input[self.pos..].chars().next().unwrap_or('\0')
    }

    fn next_char(&mut self) -> char {
        let c = self.peek_char();
        if c != '\0' {
            self.pos += c.len_utf8();
        }
        c
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.input.len() {
            match self.peek_char() {
                ' ' | '\t' | '\n' | '\r' => { self.next_char(); }
                _ => break,
            }
        }
    }

    fn parse_string(&mut self, interp: &mut Interpreter) -> Result<Value, String> {
        self.next_char(); // consume opening quote
        let mut result = String::new();

        loop {
            if self.pos >= self.input.len() {
                return Err("Unterminated string in JSON".to_string());
            }

            let c = self.next_char();
            match c {
                '"' => break,
                '\\' => {
                    let escaped = self.next_char();
                    match escaped {
                        '"' => result.push('"'),
                        '\\' => result.push('\\'),
                        '/' => result.push('/'),
                        'n' => result.push('\n'),
                        'r' => result.push('\r'),
                        't' => result.push('\t'),
                        'b' => result.push('\x08'),
                        'f' => result.push('\x0C'),
                        'u' => {
                            // Parse unicode escape \uXXXX
                            let hex: String = (0..4).filter_map(|_| {
                                let c = self.next_char();
                                if c.is_ascii_hexdigit() { Some(c) } else { None }
                            }).collect();
                            if hex.len() == 4 {
                                if let Ok(code) = u32::from_str_radix(&hex, 16) {
                                    if let Some(c) = char::from_u32(code) {
                                        result.push(c);
                                    }
                                }
                            }
                        }
                        _ => result.push(escaped),
                    }
                }
                _ => result.push(c),
            }
        }

        Ok(interp.create_runtime_string(result))
    }

    fn parse_number(&mut self) -> Result<Value, String> {
        let start = self.pos;

        // Handle negative sign
        if self.peek_char() == '-' {
            self.next_char();
        }

        // Parse digits
        while self.pos < self.input.len() && self.peek_char().is_ascii_digit() {
            self.next_char();
        }

        // Check for decimal point (we only support integers for now)
        if self.peek_char() == '.' {
            // Skip decimal part but parse as integer
            self.next_char();
            while self.pos < self.input.len() && self.peek_char().is_ascii_digit() {
                self.next_char();
            }
        }

        // Check for exponent
        if self.peek_char() == 'e' || self.peek_char() == 'E' {
            self.next_char();
            if self.peek_char() == '+' || self.peek_char() == '-' {
                self.next_char();
            }
            while self.pos < self.input.len() && self.peek_char().is_ascii_digit() {
                self.next_char();
            }
        }

        let num_str = &self.input[start..self.pos];

        // Parse as integer (truncating decimals)
        if let Ok(n) = num_str.parse::<i32>() {
            Ok(Value::int(n))
        } else if let Ok(f) = num_str.parse::<f64>() {
            // Truncate to integer
            Ok(Value::int(f as i32))
        } else {
            Err(format!("Invalid number in JSON: {}", num_str))
        }
    }

    fn parse_boolean(&mut self) -> Result<Value, String> {
        if self.input[self.pos..].starts_with("true") {
            self.pos += 4;
            Ok(Value::bool(true))
        } else if self.input[self.pos..].starts_with("false") {
            self.pos += 5;
            Ok(Value::bool(false))
        } else {
            Err("Invalid boolean in JSON".to_string())
        }
    }

    fn parse_null(&mut self) -> Result<Value, String> {
        if self.input[self.pos..].starts_with("null") {
            self.pos += 4;
            Ok(Value::null())
        } else {
            Err("Invalid null in JSON".to_string())
        }
    }

    fn parse_array(&mut self, interp: &mut Interpreter) -> Result<Value, String> {
        self.next_char(); // consume '['
        self.skip_whitespace();

        let mut items: Vec<Value> = Vec::new();

        // Empty array
        if self.peek_char() == ']' {
            self.next_char();
            let arr_idx = interp.arrays.len() as u32;
            interp.arrays.push(items);
            return Ok(Value::array_idx(arr_idx));
        }

        loop {
            let value = self.parse_value(interp)?;
            items.push(value);

            self.skip_whitespace();
            let c = self.next_char();

            match c {
                ',' => { self.skip_whitespace(); }
                ']' => break,
                _ => return Err(format!("Expected ',' or ']' in array, found '{}'", c)),
            }
        }

        let arr_idx = interp.arrays.len() as u32;
        interp.arrays.push(items);
        Ok(Value::array_idx(arr_idx))
    }

    fn parse_object(&mut self, interp: &mut Interpreter) -> Result<Value, String> {
        self.next_char(); // consume '{'
        self.skip_whitespace();

        let mut props: Vec<(String, Value)> = Vec::new();

        // Empty object
        if self.peek_char() == '}' {
            self.next_char();
            let obj_idx = interp.objects.len() as u32;
            let obj = ObjectInstance {
                constructor: None,
                properties: props,
                frozen: false,
                sealed: false,
            };
            interp.objects.push(obj);
            return Ok(Value::object_idx(obj_idx));
        }

        loop {
            self.skip_whitespace();

            // Parse key (must be a string)
            if self.peek_char() != '"' {
                return Err("Expected string key in object".to_string());
            }

            // Parse the key string directly
            self.next_char(); // consume opening quote
            let mut key = String::new();
            loop {
                if self.pos >= self.input.len() {
                    return Err("Unterminated string key in JSON".to_string());
                }
                let c = self.next_char();
                match c {
                    '"' => break,
                    '\\' => {
                        let escaped = self.next_char();
                        match escaped {
                            '"' => key.push('"'),
                            '\\' => key.push('\\'),
                            'n' => key.push('\n'),
                            _ => key.push(escaped),
                        }
                    }
                    _ => key.push(c),
                }
            }

            self.skip_whitespace();

            // Expect colon
            if self.next_char() != ':' {
                return Err("Expected ':' after key in object".to_string());
            }

            self.skip_whitespace();

            // Parse value
            let value = self.parse_value(interp)?;
            props.push((key, value));

            self.skip_whitespace();
            let c = self.next_char();

            match c {
                ',' => { self.skip_whitespace(); }
                '}' => break,
                _ => return Err(format!("Expected ',' or '}}' in object, found '{}'", c)),
            }
        }

        let obj_idx = interp.objects.len() as u32;
        let obj = ObjectInstance {
            constructor: None,
            properties: props,
            frozen: false,
            sealed: false,
        };
        interp.objects.push(obj);
        Ok(Value::object_idx(obj_idx))
    }
}

// ===========================================
// Date Functions
// ===========================================

/// Date.now - returns current timestamp in milliseconds
/// Note: Due to 31-bit integer limitation, we return milliseconds modulo 2^30
/// This allows for relative timing within ~12 day windows
fn native_date_now(_interp: &mut Interpreter, _this: Value, _args: &[Value]) -> Result<Value, String> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("Time error: {}", e))?;

    // Return milliseconds modulo 2^30 (about 12.4 days worth)
    // This fits in 31-bit signed range and allows relative timing
    let millis = now.as_millis() as i64;
    let max_val = 1 << 30; // 2^30 = 1073741824

    Ok(Value::int((millis % max_val) as i32))
}

// ===========================================
// RegExp Methods
// ===========================================

/// RegExp.prototype.test - tests if the regex matches the string
fn native_regexp_test(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let regex_idx = this.to_regexp_object_idx()
        .ok_or_else(|| "test called on non-RegExp".to_string())?;

    let re = interp.regex_objects.get(regex_idx as usize)
        .ok_or_else(|| "invalid RegExp object".to_string())?
        .clone();

    // Get string to test
    let test_str = if let Some(str_val) = args.first() {
        if let Some(str_idx) = str_val.to_string_idx() {
            interp.get_string_by_idx(str_idx)
                .ok_or_else(|| "invalid string".to_string())?
                .to_string()
        } else if let Some(n) = str_val.to_i32() {
            n.to_string()
        } else {
            "undefined".to_string()
        }
    } else {
        "undefined".to_string()
    };

    Ok(Value::bool(re.regex.is_match(&test_str)))
}

/// RegExp.prototype.exec - executes the regex and returns match result
fn native_regexp_exec(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let regex_idx = this.to_regexp_object_idx()
        .ok_or_else(|| "exec called on non-RegExp".to_string())?;

    let re = interp.regex_objects.get(regex_idx as usize)
        .ok_or_else(|| "invalid RegExp object".to_string())?
        .clone();

    // Get string to match
    let match_str = if let Some(str_val) = args.first() {
        if let Some(str_idx) = str_val.to_string_idx() {
            interp.get_string_by_idx(str_idx)
                .ok_or_else(|| "invalid string".to_string())?
                .to_string()
        } else if let Some(n) = str_val.to_i32() {
            n.to_string()
        } else {
            "undefined".to_string()
        }
    } else {
        "undefined".to_string()
    };

    // Find the match
    if let Some(m) = re.regex.find(&match_str) {
        // Create result array with matched string
        let matched = m.as_str().to_string();
        let str_idx = interp.runtime_strings.len() as u16 + Interpreter::RUNTIME_STRING_OFFSET;
        interp.runtime_strings.push(matched);

        let arr_idx = interp.arrays.len() as u32;
        interp.arrays.push(vec![Value::string(str_idx)]);

        // Create result object with index and input properties
        let _result_obj_idx = interp.objects.len() as u32;
        interp.objects.push(crate::vm::interpreter::ObjectInstance {
            constructor: None,
            properties: vec![
                ("index".to_string(), Value::int(m.start() as i32)),
            ],
            frozen: false,
            sealed: false,
        });

        // For now, just return the array (input property would require more work)
        Ok(Value::array_idx(arr_idx))
    } else {
        Ok(Value::null())
    }
}

// ===========================================
// Object Static Methods
// ===========================================

/// Object.keys - returns array of object's own property names
fn native_object_keys(interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let obj = args.first().copied().unwrap_or(Value::undefined());

    if let Some(obj_idx) = obj.to_object_idx() {
        // Clone keys first to avoid borrow issues
        let key_strings: Vec<String> = interp.objects
            .get(obj_idx as usize)
            .map(|obj| obj.properties.iter().map(|(k, _)| k.clone()).collect())
            .unwrap_or_default();

        // Now create string values
        let keys: Vec<Value> = key_strings.into_iter()
            .map(|k| interp.create_runtime_string(k))
            .collect();

        let arr_idx = interp.arrays.len() as u32;
        interp.arrays.push(keys);
        return Ok(Value::array_idx(arr_idx));
    } else if let Some(arr_idx) = obj.to_array_idx() {
        // For arrays, get length first
        let len = interp.arrays.get(arr_idx as usize).map(|a| a.len()).unwrap_or(0);

        // Create index strings
        let keys: Vec<Value> = (0..len)
            .map(|i| interp.create_runtime_string(i.to_string()))
            .collect();

        let new_arr_idx = interp.arrays.len() as u32;
        interp.arrays.push(keys);
        return Ok(Value::array_idx(new_arr_idx));
    }

    // Return empty array for non-objects
    let arr_idx = interp.arrays.len() as u32;
    interp.arrays.push(Vec::new());
    Ok(Value::array_idx(arr_idx))
}

/// Object.values - returns array of object's own property values
fn native_object_values(interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let obj = args.first().copied().unwrap_or(Value::undefined());

    if let Some(obj_idx) = obj.to_object_idx() {
        // Clone values to avoid borrow issues
        let values: Vec<Value> = interp.objects
            .get(obj_idx as usize)
            .map(|obj| obj.properties.iter().map(|(_, v)| *v).collect())
            .unwrap_or_default();

        let arr_idx = interp.arrays.len() as u32;
        interp.arrays.push(values);
        return Ok(Value::array_idx(arr_idx));
    } else if let Some(arr_idx) = obj.to_array_idx() {
        // For arrays, return a copy of values
        let arr_copy = interp.arrays.get(arr_idx as usize).cloned().unwrap_or_default();
        let new_arr_idx = interp.arrays.len() as u32;
        interp.arrays.push(arr_copy);
        return Ok(Value::array_idx(new_arr_idx));
    }

    // Return empty array for non-objects
    let arr_idx = interp.arrays.len() as u32;
    interp.arrays.push(Vec::new());
    Ok(Value::array_idx(arr_idx))
}

/// Object.entries - returns array of [key, value] pairs
fn native_object_entries(interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let obj = args.first().copied().unwrap_or(Value::undefined());

    if let Some(obj_idx) = obj.to_object_idx() {
        // Clone properties to avoid borrow issues
        let props: Vec<(String, Value)> = interp.objects
            .get(obj_idx as usize)
            .map(|obj| obj.properties.clone())
            .unwrap_or_default();

        // Create array of [key, value] pairs
        let mut entries: Vec<Value> = Vec::new();

        for (k, v) in props {
            let key_val = interp.create_runtime_string(k);
            // Create inner array [key, value]
            let pair_idx = interp.arrays.len() as u32;
            interp.arrays.push(vec![key_val, v]);
            entries.push(Value::array_idx(pair_idx));
        }

        let arr_idx = interp.arrays.len() as u32;
        interp.arrays.push(entries);
        return Ok(Value::array_idx(arr_idx));
    }

    // Return empty array for non-objects
    let arr_idx = interp.arrays.len() as u32;
    interp.arrays.push(Vec::new());
    Ok(Value::array_idx(arr_idx))
}

/// Object.assign - copies properties from source objects to target object
fn native_object_assign(interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let target = args.first().copied().unwrap_or(Value::undefined());

    // Target must be an object
    let target_idx = match target.to_object_idx() {
        Some(idx) => idx as usize,
        None => {
            // If target is not an object, create a new object
            let obj_idx = interp.objects.len();
            interp.objects.push(ObjectInstance::new());
            obj_idx
        }
    };

    // Copy properties from each source object
    for source in args.iter().skip(1) {
        if source.is_undefined() || source.is_null() {
            continue;
        }

        if let Some(src_idx) = source.to_object_idx() {
            // Clone source properties to avoid borrow issues
            let props: Vec<(String, Value)> = interp.objects
                .get(src_idx as usize)
                .map(|obj| obj.properties.clone())
                .unwrap_or_default();

            for (key, value) in props {
                // Set property on target
                if let Some(target_obj) = interp.objects.get_mut(target_idx) {
                    // Check if property already exists
                    let mut found = false;
                    for (k, v) in target_obj.properties.iter_mut() {
                        if k == &key {
                            *v = value;
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        target_obj.properties.push((key, value));
                    }
                }
            }
        }
    }

    Ok(Value::object_idx(target_idx as u32))
}

/// Object.create - creates a new object with the specified prototype
fn native_object_create(interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let proto = args.first().copied().unwrap_or(Value::null());

    // Create new object
    let mut obj = ObjectInstance::new();

    // Set prototype (simplified - just store as __proto__ property)
    if !proto.is_null() {
        obj.properties.push(("__proto__".to_string(), proto));
    }

    // If second argument is provided, add properties from it
    if let Some(props_desc) = args.get(1) {
        if let Some(props_idx) = props_desc.to_object_idx() {
            // Clone properties to avoid borrow issues
            let props: Vec<(String, Value)> = interp.objects
                .get(props_idx as usize)
                .map(|o| o.properties.clone())
                .unwrap_or_default();

            // For each property descriptor, get the 'value' property
            for (key, desc_val) in props {
                if let Some(desc_idx) = desc_val.to_object_idx() {
                    let desc_props: Vec<(String, Value)> = interp.objects
                        .get(desc_idx as usize)
                        .map(|o| o.properties.clone())
                        .unwrap_or_default();

                    // Find 'value' in the descriptor
                    for (dk, dv) in desc_props {
                        if dk == "value" {
                            obj.properties.push((key.clone(), dv));
                            break;
                        }
                    }
                }
            }
        }
    }

    let obj_idx = interp.objects.len() as u32;
    interp.objects.push(obj);
    Ok(Value::object_idx(obj_idx))
}

/// Object.freeze - freezes an object (makes it immutable)
fn native_object_freeze(interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let obj = args.first().copied().unwrap_or(Value::undefined());

    if let Some(obj_idx) = obj.to_object_idx() {
        if let Some(obj_ref) = interp.objects.get_mut(obj_idx as usize) {
            obj_ref.frozen = true;
            obj_ref.sealed = true; // Frozen implies sealed
        }
        return Ok(obj);
    }

    // Return non-objects unchanged
    Ok(obj)
}

/// Object.seal - seals an object (prevents adding/removing properties)
fn native_object_seal(interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let obj = args.first().copied().unwrap_or(Value::undefined());

    if let Some(obj_idx) = obj.to_object_idx() {
        if let Some(obj_ref) = interp.objects.get_mut(obj_idx as usize) {
            obj_ref.sealed = true;
        }
        return Ok(obj);
    }

    // Return non-objects unchanged
    Ok(obj)
}

/// Object.isFrozen - check if object is frozen
fn native_object_is_frozen(interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let obj = args.first().copied().unwrap_or(Value::undefined());

    if let Some(obj_idx) = obj.to_object_idx() {
        if let Some(obj_ref) = interp.objects.get(obj_idx as usize) {
            return Ok(Value::bool(obj_ref.frozen));
        }
    }

    // Non-objects are considered frozen
    Ok(Value::bool(true))
}

/// Object.isSealed - check if object is sealed
fn native_object_is_sealed(interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let obj = args.first().copied().unwrap_or(Value::undefined());

    if let Some(obj_idx) = obj.to_object_idx() {
        if let Some(obj_ref) = interp.objects.get(obj_idx as usize) {
            return Ok(Value::bool(obj_ref.sealed));
        }
    }

    // Non-objects are considered sealed
    Ok(Value::bool(true))
}

/// Object.getOwnPropertyNames - returns array of all property names (including non-enumerable)
fn native_object_get_own_property_names(interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let obj = args.first().copied().unwrap_or(Value::undefined());

    if let Some(obj_idx) = obj.to_object_idx() {
        // Clone property names to avoid borrow issues
        let key_strings: Vec<String> = interp.objects
            .get(obj_idx as usize)
            .map(|obj| obj.properties.iter().map(|(k, _)| k.clone()).collect())
            .unwrap_or_default();

        // Create string values
        let keys: Vec<Value> = key_strings.into_iter()
            .map(|k| interp.create_runtime_string(k))
            .collect();

        let arr_idx = interp.arrays.len() as u32;
        interp.arrays.push(keys);
        return Ok(Value::array_idx(arr_idx));
    } else if let Some(arr_idx) = obj.to_array_idx() {
        // For arrays, include 'length' as a property name
        let len = interp.arrays.get(arr_idx as usize).map(|a| a.len()).unwrap_or(0);

        let mut keys: Vec<Value> = (0..len)
            .map(|i| interp.create_runtime_string(i.to_string()))
            .collect();
        keys.push(interp.create_runtime_string("length".to_string()));

        let new_arr_idx = interp.arrays.len() as u32;
        interp.arrays.push(keys);
        return Ok(Value::array_idx(new_arr_idx));
    }

    // Return empty array for non-objects
    let arr_idx = interp.arrays.len() as u32;
    interp.arrays.push(Vec::new());
    Ok(Value::array_idx(arr_idx))
}

/// Object.fromEntries - creates object from array of [key, value] pairs
fn native_object_from_entries(interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let entries = args.first().copied().unwrap_or(Value::undefined());

    let mut obj = ObjectInstance::new();

    if let Some(arr_idx) = entries.to_array_idx() {
        // Clone the entries array
        let entries_arr = interp.arrays.get(arr_idx as usize)
            .ok_or_else(|| "invalid entries array".to_string())?
            .clone();

        for entry in entries_arr {
            if let Some(pair_idx) = entry.to_array_idx() {
                let pair = interp.arrays.get(pair_idx as usize)
                    .ok_or_else(|| "invalid entry pair".to_string())?
                    .clone();

                if pair.len() >= 2 {
                    // Get the key as string
                    let key = if let Some(str_idx) = pair[0].to_string_idx() {
                        interp.get_string_by_idx(str_idx)
                            .map(|s| s.to_string())
                            .unwrap_or_default()
                    } else if let Some(n) = pair[0].to_i32() {
                        n.to_string()
                    } else {
                        continue;
                    };

                    obj.properties.push((key, pair[1]));
                }
            }
        }
    }

    let obj_idx = interp.objects.len() as u32;
    interp.objects.push(obj);
    Ok(Value::object_idx(obj_idx))
}

/// Object.hasOwn - check if object has own property (ES2022)
fn native_object_has_own(interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let obj = args.first().copied().unwrap_or(Value::undefined());
    let prop = args.get(1).copied().unwrap_or(Value::undefined());

    // Get property name as string
    let prop_name = if let Some(str_idx) = prop.to_string_idx() {
        interp.get_string_by_idx(str_idx)
            .map(|s| s.to_string())
            .unwrap_or_default()
    } else if let Some(n) = prop.to_i32() {
        n.to_string()
    } else {
        return Ok(Value::bool(false));
    };

    if let Some(obj_idx) = obj.to_object_idx() {
        let has_prop = interp.objects
            .get(obj_idx as usize)
            .map(|o| o.properties.iter().any(|(k, _)| k == &prop_name))
            .unwrap_or(false);
        return Ok(Value::bool(has_prop));
    } else if let Some(arr_idx) = obj.to_array_idx() {
        // For arrays, check if index exists
        if let Ok(idx) = prop_name.parse::<usize>() {
            let len = interp.arrays.get(arr_idx as usize).map(|a| a.len()).unwrap_or(0);
            return Ok(Value::bool(idx < len));
        } else if prop_name == "length" {
            return Ok(Value::bool(true));
        }
    }

    Ok(Value::bool(false))
}

/// Object.prototype.hasOwnProperty - check if object has own property
fn native_object_prototype_has_own_property(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let prop = args.first().copied().unwrap_or(Value::undefined());

    // Get property name as string
    let prop_name = if let Some(str_idx) = prop.to_string_idx() {
        interp.get_string_by_idx(str_idx)
            .map(|s| s.to_string())
            .unwrap_or_default()
    } else if let Some(n) = prop.to_i32() {
        n.to_string()
    } else {
        return Ok(Value::bool(false));
    };

    if let Some(obj_idx) = this.to_object_idx() {
        let has_prop = interp.objects
            .get(obj_idx as usize)
            .map(|o| o.properties.iter().any(|(k, _)| k == &prop_name))
            .unwrap_or(false);
        return Ok(Value::bool(has_prop));
    } else if let Some(arr_idx) = this.to_array_idx() {
        // For arrays, check if index exists
        if let Ok(idx) = prop_name.parse::<usize>() {
            let len = interp.arrays.get(arr_idx as usize).map(|a| a.len()).unwrap_or(0);
            return Ok(Value::bool(idx < len));
        } else if prop_name == "length" {
            return Ok(Value::bool(true));
        }
    }

    Ok(Value::bool(false))
}

/// Object.prototype.toString - returns string representation of object
fn native_object_prototype_to_string(interp: &mut Interpreter, this: Value, _args: &[Value]) -> Result<Value, String> {
    let type_str = if this.is_undefined() {
        "[object Undefined]"
    } else if this.is_null() {
        "[object Null]"
    } else if this.to_array_idx().is_some() {
        "[object Array]"
    } else if this.to_object_idx().is_some() {
        "[object Object]"
    } else if this.to_func_idx().is_some() || this.to_native_func_idx().is_some() {
        "[object Function]"
    } else if this.to_string_idx().is_some() {
        "[object String]"
    } else if this.to_i32().is_some() {
        "[object Number]"
    } else if this.is_bool() {
        "[object Boolean]"
    } else {
        "[object Object]"
    };
    Ok(interp.create_runtime_string(type_str.to_string()))
}

/// Object.prototype.valueOf - returns the primitive value of the object
fn native_object_prototype_value_of(_interp: &mut Interpreter, this: Value, _args: &[Value]) -> Result<Value, String> {
    // valueOf for objects typically returns the object itself
    Ok(this)
}

/// Object.is - strict equality comparison (like === but handles NaN and -0)
fn native_object_is(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let value1 = args.first().copied().unwrap_or(Value::undefined());
    let value2 = args.get(1).copied().unwrap_or(Value::undefined());

    // Compare the raw values - same behavior as ===
    // (Note: We don't have NaN or -0 distinction in our integer-only impl)
    let is_same = value1.0 == value2.0;
    Ok(Value::bool(is_same))
}

/// Object.getPrototypeOf - get the prototype of an object
fn native_object_get_prototype_of(interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let obj = args.first().copied().unwrap_or(Value::undefined());

    // For our implementation, objects created via `new` store their constructor
    // We return null for most cases since we don't have true prototype chains
    if let Some(obj_idx) = obj.to_object_idx() {
        if let Some(obj_data) = interp.objects.get(obj_idx as usize) {
            if let Some(constructor) = obj_data.constructor {
                // Return a placeholder "prototype" object
                // In a full implementation, this would return the constructor's prototype
                return Ok(Value::null());
            }
        }
        Ok(Value::null())
    } else if obj.to_array_idx().is_some() {
        // Arrays have Array.prototype
        Ok(Value::null())
    } else {
        Ok(Value::null())
    }
}

/// Object.getOwnPropertyDescriptor - get property descriptor
fn native_object_get_own_property_descriptor(interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let obj = args.first().copied().unwrap_or(Value::undefined());
    let prop = args.get(1).copied().unwrap_or(Value::undefined());

    // Get property name as string
    let prop_name = if let Some(str_idx) = prop.to_string_idx() {
        interp.get_string_by_idx(str_idx)
            .map(|s| s.to_string())
            .unwrap_or_default()
    } else if let Some(n) = prop.to_i32() {
        n.to_string()
    } else {
        return Ok(Value::undefined());
    };

    if let Some(obj_idx) = obj.to_object_idx() {
        if let Some(obj_data) = interp.objects.get(obj_idx as usize) {
            // Find the property
            for (key, value) in &obj_data.properties {
                if key == &prop_name {
                    // Create a descriptor object with value, writable, enumerable, configurable
                    let mut descriptor = ObjectInstance {
                        constructor: None,
                        properties: vec![
                            ("value".to_string(), *value),
                            ("writable".to_string(), Value::bool(!obj_data.frozen)),
                            ("enumerable".to_string(), Value::bool(true)),
                            ("configurable".to_string(), Value::bool(!obj_data.sealed)),
                        ],
                        frozen: false,
                        sealed: false,
                    };
                    let desc_idx = interp.objects.len() as u32;
                    interp.objects.push(descriptor);
                    return Ok(Value::object_idx(desc_idx));
                }
            }
        }
    } else if let Some(arr_idx) = obj.to_array_idx() {
        if prop_name == "length" {
            let len = interp.arrays.get(arr_idx as usize).map(|a| a.len()).unwrap_or(0);
            let descriptor = ObjectInstance {
                constructor: None,
                properties: vec![
                    ("value".to_string(), Value::int(len as i32)),
                    ("writable".to_string(), Value::bool(true)),
                    ("enumerable".to_string(), Value::bool(false)),
                    ("configurable".to_string(), Value::bool(false)),
                ],
                frozen: false,
                sealed: false,
            };
            let desc_idx = interp.objects.len() as u32;
            interp.objects.push(descriptor);
            return Ok(Value::object_idx(desc_idx));
        } else if let Ok(idx) = prop_name.parse::<usize>() {
            if let Some(arr) = interp.arrays.get(arr_idx as usize) {
                if idx < arr.len() {
                    let descriptor = ObjectInstance {
                        constructor: None,
                        properties: vec![
                            ("value".to_string(), arr[idx]),
                            ("writable".to_string(), Value::bool(true)),
                            ("enumerable".to_string(), Value::bool(true)),
                            ("configurable".to_string(), Value::bool(true)),
                        ],
                        frozen: false,
                        sealed: false,
                    };
                    let desc_idx = interp.objects.len() as u32;
                    interp.objects.push(descriptor);
                    return Ok(Value::object_idx(desc_idx));
                }
            }
        }
    }

    Ok(Value::undefined())
}

/// Object.defineProperty - define a property on an object
fn native_object_define_property(interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let obj = args.first().copied().unwrap_or(Value::undefined());
    let prop = args.get(1).copied().unwrap_or(Value::undefined());
    let descriptor = args.get(2).copied().unwrap_or(Value::undefined());

    // Get property name as string
    let prop_name = if let Some(str_idx) = prop.to_string_idx() {
        interp.get_string_by_idx(str_idx)
            .map(|s| s.to_string())
            .unwrap_or_default()
    } else if let Some(n) = prop.to_i32() {
        n.to_string()
    } else {
        return Err("Property name must be a string".to_string());
    };

    // Get value from descriptor
    let value = if let Some(desc_idx) = descriptor.to_object_idx() {
        if let Some(desc_obj) = interp.objects.get(desc_idx as usize) {
            desc_obj.properties.iter()
                .find(|(k, _)| k == "value")
                .map(|(_, v)| *v)
                .unwrap_or(Value::undefined())
        } else {
            Value::undefined()
        }
    } else {
        Value::undefined()
    };

    if let Some(obj_idx) = obj.to_object_idx() {
        if let Some(obj_data) = interp.objects.get_mut(obj_idx as usize) {
            if obj_data.frozen {
                return Err("Cannot define property on frozen object".to_string());
            }
            // Check if property already exists
            let mut found = false;
            for (key, val) in &mut obj_data.properties {
                if key == &prop_name {
                    *val = value;
                    found = true;
                    break;
                }
            }
            if !found {
                if obj_data.sealed {
                    return Err("Cannot add property to sealed object".to_string());
                }
                obj_data.properties.push((prop_name, value));
            }
        }
        return Ok(obj);
    }

    Err("Object.defineProperty requires an object".to_string())
}

/// Object.preventExtensions - prevent new properties from being added
fn native_object_prevent_extensions(interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let obj = args.first().copied().unwrap_or(Value::undefined());

    if let Some(obj_idx) = obj.to_object_idx() {
        if let Some(obj_data) = interp.objects.get_mut(obj_idx as usize) {
            obj_data.sealed = true; // Reusing sealed flag for non-extensible
        }
        Ok(obj)
    } else {
        // Return the argument for non-objects (ES6 behavior)
        Ok(obj)
    }
}

/// Object.isExtensible - check if object can have new properties added
fn native_object_is_extensible(interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let obj = args.first().copied().unwrap_or(Value::undefined());

    if let Some(obj_idx) = obj.to_object_idx() {
        if let Some(obj_data) = interp.objects.get(obj_idx as usize) {
            // An object is extensible if not sealed and not frozen
            return Ok(Value::bool(!obj_data.sealed && !obj_data.frozen));
        }
    }
    // Non-objects are not extensible
    Ok(Value::bool(false))
}

// ===========================================
// Array Static Methods
// ===========================================

/// Array.from - creates an array from an iterable or array-like object
fn native_array_from(interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let source = args.first().copied().unwrap_or(Value::undefined());
    let map_fn = args.get(1).copied();

    let mut result: Vec<Value> = Vec::new();

    if let Some(arr_idx) = source.to_array_idx() {
        // Source is an array - copy its elements
        let elements: Vec<Value> = interp.get_array(arr_idx)
            .map(|arr| arr.clone())
            .unwrap_or_default();

        if let Some(mapper) = map_fn {
            // Apply map function to each element
            for (i, elem) in elements.into_iter().enumerate() {
                let mapped = interp.call_value(mapper, Value::undefined(), &[elem, Value::int(i as i32)])
                    .map_err(|e| e.to_string())?;
                result.push(mapped);
            }
        } else {
            result = elements;
        }
    } else if let Some(str_idx) = source.to_string_idx() {
        // Source is a string - convert to array of characters
        let s = interp.get_string_by_idx(str_idx)
            .unwrap_or("")
            .to_string();

        for (i, c) in s.chars().enumerate() {
            let char_str = interp.create_runtime_string(c.to_string());
            if let Some(mapper) = map_fn {
                let mapped = interp.call_value(mapper, Value::undefined(), &[char_str, Value::int(i as i32)])
                    .map_err(|e| e.to_string())?;
                result.push(mapped);
            } else {
                result.push(char_str);
            }
        }
    } else if let Some(obj_idx) = source.to_object_idx() {
        // Source is an array-like object with 'length' property
        let (length, props) = {
            let obj = interp.objects.get(obj_idx as usize);
            let len = obj.and_then(|o| {
                o.properties.iter()
                    .find(|(k, _)| k == "length")
                    .and_then(|(_, v)| v.to_i32())
            }).unwrap_or(0);
            let props = obj.map(|o| o.properties.clone()).unwrap_or_default();
            (len, props)
        };

        for i in 0..length {
            let key = i.to_string();
            let elem = props.iter()
                .find(|(k, _)| k == &key)
                .map(|(_, v)| *v)
                .unwrap_or(Value::undefined());

            if let Some(mapper) = map_fn {
                let mapped = interp.call_value(mapper, Value::undefined(), &[elem, Value::int(i)])
                    .map_err(|e| e.to_string())?;
                result.push(mapped);
            } else {
                result.push(elem);
            }
        }
    }

    let arr_idx = interp.arrays.len() as u32;
    interp.arrays.push(result);
    Ok(Value::array_idx(arr_idx))
}

/// Array.of - creates an array from the given arguments
fn native_array_of(interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let elements: Vec<Value> = args.iter().copied().collect();
    let arr_idx = interp.arrays.len() as u32;
    interp.arrays.push(elements);
    Ok(Value::array_idx(arr_idx))
}

/// Array.isArray - check if value is an array
fn native_array_is_array(_interp: &mut Interpreter, _this: Value, args: &[Value]) -> Result<Value, String> {
    let val = args.first().copied().unwrap_or(Value::undefined());
    Ok(Value::bool(val.is_array()))
}

// ===========================================
// Map.prototype Methods
// ===========================================

/// Map.prototype.get - get value by key
fn native_map_get(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let map_idx = this.to_map_object_idx()
        .ok_or_else(|| "get called on non-Map".to_string())?;

    let key = args.first().copied().unwrap_or(Value::undefined());

    let result = interp.map_objects.get(map_idx as usize)
        .and_then(|m| m.get(key))
        .unwrap_or(Value::undefined());

    Ok(result)
}

/// Map.prototype.set - set key-value pair
fn native_map_set(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let map_idx = this.to_map_object_idx()
        .ok_or_else(|| "set called on non-Map".to_string())?;

    let key = args.first().copied().unwrap_or(Value::undefined());
    let value = args.get(1).copied().unwrap_or(Value::undefined());

    if let Some(map) = interp.map_objects.get_mut(map_idx as usize) {
        map.set(key, value);
    }

    Ok(this) // Return the Map for chaining
}

/// Map.prototype.has - check if key exists
fn native_map_has(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let map_idx = this.to_map_object_idx()
        .ok_or_else(|| "has called on non-Map".to_string())?;

    let key = args.first().copied().unwrap_or(Value::undefined());

    let has = interp.map_objects.get(map_idx as usize)
        .map(|m| m.has(key))
        .unwrap_or(false);

    Ok(Value::bool(has))
}

/// Map.prototype.delete - delete key
fn native_map_delete(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let map_idx = this.to_map_object_idx()
        .ok_or_else(|| "delete called on non-Map".to_string())?;

    let key = args.first().copied().unwrap_or(Value::undefined());

    let deleted = interp.map_objects.get_mut(map_idx as usize)
        .map(|m| m.delete(key))
        .unwrap_or(false);

    Ok(Value::bool(deleted))
}

/// Map.prototype.clear - remove all entries
fn native_map_clear(interp: &mut Interpreter, this: Value, _args: &[Value]) -> Result<Value, String> {
    let map_idx = this.to_map_object_idx()
        .ok_or_else(|| "clear called on non-Map".to_string())?;

    if let Some(map) = interp.map_objects.get_mut(map_idx as usize) {
        map.clear();
    }

    Ok(Value::undefined())
}

/// Map.prototype.size - get number of entries (as a getter)
fn native_map_size(interp: &mut Interpreter, this: Value, _args: &[Value]) -> Result<Value, String> {
    let map_idx = this.to_map_object_idx()
        .ok_or_else(|| "size called on non-Map".to_string())?;

    let size = interp.map_objects.get(map_idx as usize)
        .map(|m| m.size())
        .unwrap_or(0);

    Ok(Value::int(size as i32))
}

/// Map.prototype.forEach - call callback for each entry
fn native_map_foreach(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let map_idx = this.to_map_object_idx()
        .ok_or_else(|| "forEach called on non-Map".to_string())?;

    let callback = args.first().copied()
        .ok_or_else(|| "forEach requires a callback function".to_string())?;

    let entries: Vec<(Value, Value)> = interp.map_objects.get(map_idx as usize)
        .map(|m| m.entries.clone())
        .unwrap_or_default();

    for (key, value) in entries {
        interp.call_value(callback, this, &[value, key, this])
            .map_err(|e| e.to_string())?;
    }

    Ok(Value::undefined())
}

// ===========================================
// Set.prototype Methods
// ===========================================

/// Set.prototype.add - add value to set
fn native_set_add(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let set_idx = this.to_set_object_idx()
        .ok_or_else(|| "add called on non-Set".to_string())?;

    let value = args.first().copied().unwrap_or(Value::undefined());

    if let Some(set) = interp.set_objects.get_mut(set_idx as usize) {
        set.add(value);
    }

    Ok(this) // Return the Set for chaining
}

/// Set.prototype.has - check if value exists
fn native_set_has(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let set_idx = this.to_set_object_idx()
        .ok_or_else(|| "has called on non-Set".to_string())?;

    let value = args.first().copied().unwrap_or(Value::undefined());

    let has = interp.set_objects.get(set_idx as usize)
        .map(|s| s.has(value))
        .unwrap_or(false);

    Ok(Value::bool(has))
}

/// Set.prototype.delete - remove value from set
fn native_set_delete(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let set_idx = this.to_set_object_idx()
        .ok_or_else(|| "delete called on non-Set".to_string())?;

    let value = args.first().copied().unwrap_or(Value::undefined());

    let deleted = interp.set_objects.get_mut(set_idx as usize)
        .map(|s| s.delete(value))
        .unwrap_or(false);

    Ok(Value::bool(deleted))
}

/// Set.prototype.clear - remove all values
fn native_set_clear(interp: &mut Interpreter, this: Value, _args: &[Value]) -> Result<Value, String> {
    let set_idx = this.to_set_object_idx()
        .ok_or_else(|| "clear called on non-Set".to_string())?;

    if let Some(set) = interp.set_objects.get_mut(set_idx as usize) {
        set.clear();
    }

    Ok(Value::undefined())
}

/// Set.prototype.size - get number of values (as a getter)
fn native_set_size(interp: &mut Interpreter, this: Value, _args: &[Value]) -> Result<Value, String> {
    let set_idx = this.to_set_object_idx()
        .ok_or_else(|| "size called on non-Set".to_string())?;

    let size = interp.set_objects.get(set_idx as usize)
        .map(|s| s.size())
        .unwrap_or(0);

    Ok(Value::int(size as i32))
}

/// Set.prototype.forEach - call callback for each value
fn native_set_foreach(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    let set_idx = this.to_set_object_idx()
        .ok_or_else(|| "forEach called on non-Set".to_string())?;

    let callback = args.first().copied()
        .ok_or_else(|| "forEach requires a callback function".to_string())?;

    let values: Vec<Value> = interp.set_objects.get(set_idx as usize)
        .map(|s| s.values.clone())
        .unwrap_or_default();

    for value in values {
        interp.call_value(callback, this, &[value, value, this])
            .map_err(|e| e.to_string())?;
    }

    Ok(Value::undefined())
}

// ===========================================
// Function.prototype Methods
// ===========================================

/// Function.prototype.call - call function with specified this value and arguments
/// Usage: func.call(thisArg, arg1, arg2, ...)
fn native_function_call(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    // 'this' is the function to call
    if !this.is_closure() && this.to_func_ptr().is_none() {
        return Err("call() called on non-function".to_string());
    }

    // First argument is the new 'this' value
    let new_this = args.first().copied().unwrap_or(Value::undefined());

    // Remaining arguments are passed to the function
    let call_args: Vec<Value> = args.iter().skip(1).copied().collect();

    interp.call_value(this, new_this, &call_args)
        .map_err(|e| e.to_string())
}

/// Function.prototype.apply - call function with specified this value and arguments array
/// Usage: func.apply(thisArg, [argsArray])
fn native_function_apply(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    // 'this' is the function to call
    if !this.is_closure() && this.to_func_ptr().is_none() {
        return Err("apply() called on non-function".to_string());
    }

    // First argument is the new 'this' value
    let new_this = args.first().copied().unwrap_or(Value::undefined());

    // Second argument should be an array of arguments
    let call_args: Vec<Value> = if let Some(arr_val) = args.get(1) {
        if let Some(arr_idx) = arr_val.to_array_idx() {
            interp.get_array(arr_idx)
                .map(|arr| arr.clone())
                .unwrap_or_default()
        } else if arr_val.is_undefined() || arr_val.is_null() {
            Vec::new()
        } else {
            return Err("second argument to apply() must be an array".to_string());
        }
    } else {
        Vec::new()
    };

    interp.call_value(this, new_this, &call_args)
        .map_err(|e| e.to_string())
}

/// Function.prototype.bind - create a new function with bound this value
/// Usage: func.bind(thisArg, arg1, arg2, ...) -> boundFunction
/// Note: Returns a value that stores the bound function, this, and args
fn native_function_bind(interp: &mut Interpreter, this: Value, args: &[Value]) -> Result<Value, String> {
    // 'this' is the function to bind
    if !this.is_closure() && this.to_func_ptr().is_none() {
        return Err("bind() called on non-function".to_string());
    }

    // Create a bound function object
    // We store: original function, bound this, and bound args
    let bound_this = args.first().copied().unwrap_or(Value::undefined());
    let bound_args: Vec<Value> = args.iter().skip(1).copied().collect();

    // Create an object to store the bound function info
    let obj_idx = interp.objects.len() as u32;
    let mut obj = ObjectInstance::new();
    obj.properties.push(("__bound_func__".to_string(), this));
    obj.properties.push(("__bound_this__".to_string(), bound_this));

    // Store bound args in an array
    let arr_idx = interp.arrays.len() as u32;
    interp.arrays.push(bound_args);
    obj.properties.push(("__bound_args__".to_string(), Value::array_idx(arr_idx)));

    // Mark as bound function
    obj.properties.push(("__is_bound__".to_string(), Value::bool(true)));

    interp.objects.push(obj);

    // Return as object (will be callable via special handling)
    Ok(Value::object_idx(obj_idx))
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
