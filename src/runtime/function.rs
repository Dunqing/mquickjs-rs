//! JavaScript function types
//!
//! This module implements the different function types:
//! - Closures (JavaScript functions with captured variables)
//! - C functions (native Rust functions)
//! - Function bytecode

use crate::value::Value;

/// Maximum number of function arguments
pub const MAX_ARGS: u16 = 65535;

/// C function signature
///
/// Native functions take context, this value, and arguments.
pub type CFunctionPtr = fn(ctx: *mut (), this: Value, args: &[Value]) -> Value;

/// C function with index into function table
#[derive(Debug, Clone, Copy)]
pub struct CFunction {
    /// Index into the C function table
    pub idx: u32,
    /// Optional parameter (for closures over C functions)
    pub params: Value,
}

impl CFunction {
    /// Create a new C function reference
    pub fn new(idx: u32) -> Self {
        CFunction {
            idx,
            params: Value::undefined(),
        }
    }

    /// Create a new C function with parameters
    pub fn with_params(idx: u32, params: Value) -> Self {
        CFunction { idx, params }
    }
}

/// Variable reference for closures
///
/// When a closure captures a variable from an outer scope,
/// a VarRef is created to hold the variable's value.
#[derive(Debug, Clone, Copy)]
pub struct VarRef {
    /// Whether the variable has been "detached" from the stack
    pub is_detached: bool,
    /// The value (when detached) or stack slot info (when attached)
    pub value: Value,
}

impl VarRef {
    /// Create an attached var ref (still on stack)
    pub fn attached(slot: i32) -> Self {
        VarRef {
            is_detached: false,
            value: Value::int(slot),
        }
    }

    /// Create a detached var ref with a value
    pub fn detached(value: Value) -> Self {
        VarRef {
            is_detached: true,
            value,
        }
    }

    /// Detach the var ref and store a value
    pub fn detach(&mut self, value: Value) {
        self.is_detached = true;
        self.value = value;
    }
}

/// Closure data for JavaScript functions
#[derive(Debug)]
pub struct Closure {
    /// Reference to the function bytecode
    pub bytecode: Value,
    /// Captured variable references
    pub var_refs: Vec<VarRef>,
}

impl Closure {
    /// Create a new closure
    pub fn new(bytecode: Value) -> Self {
        Closure {
            bytecode,
            var_refs: Vec::new(),
        }
    }

    /// Create a closure with captured variables
    pub fn with_var_refs(bytecode: Value, var_refs: Vec<VarRef>) -> Self {
        Closure { bytecode, var_refs }
    }

    /// Get a captured variable
    pub fn get_var(&self, index: usize) -> Option<&VarRef> {
        self.var_refs.get(index)
    }

    /// Get a mutable captured variable
    pub fn get_var_mut(&mut self, index: usize) -> Option<&mut VarRef> {
        self.var_refs.get_mut(index)
    }
}

/// Capture information for closures
#[derive(Debug, Clone)]
pub struct CaptureInfo {
    /// Index in the outer function's locals (or captures)
    pub outer_index: usize,
    /// Whether this captures from outer's locals (true) or outer's captures (false)
    pub is_local: bool,
}

pub struct FunctionBytecode {
    /// Function name (for debugging)
    pub name: Option<String>,
    /// Number of arguments
    pub arg_count: u16,
    /// Number of local variables
    pub local_count: u16,
    /// Stack size needed
    pub stack_size: u16,
    /// Whether function uses 'arguments' object
    pub has_arguments: bool,
    /// The compiled bytecode
    pub bytecode: Vec<u8>,
    /// Constant pool
    pub constants: Vec<Value>,
    /// String constant pool
    pub string_constants: Vec<String>,
    /// Debug info: source filename
    pub source_file: Option<String>,
    /// Debug info: line number table (pc -> line)
    pub line_numbers: Vec<(u32, u32)>,
    /// Inner functions defined within this function
    pub inner_functions: Vec<FunctionBytecode>,
    /// Capture information for closures
    pub captures: Vec<CaptureInfo>,
}

impl FunctionBytecode {
    /// Create a new function bytecode
    pub fn new(arg_count: u16, local_count: u16) -> Self {
        FunctionBytecode {
            name: None,
            arg_count,
            local_count,
            stack_size: 0,
            has_arguments: false,
            bytecode: Vec::new(),
            constants: Vec::new(),
            string_constants: Vec::new(),
            source_file: None,
            line_numbers: Vec::new(),
            inner_functions: Vec::new(),
            captures: Vec::new(),
        }
    }

    /// Set the function name
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = Some(name.into());
    }

    /// Add a constant to the constant pool
    pub fn add_constant(&mut self, value: Value) -> u16 {
        let idx = self.constants.len() as u16;
        self.constants.push(value);
        idx
    }

    /// Get a constant from the constant pool
    pub fn get_constant(&self, idx: u16) -> Option<Value> {
        self.constants.get(idx as usize).copied()
    }

    /// Add a string to the string constant pool
    pub fn add_string(&mut self, s: String) -> u16 {
        let idx = self.string_constants.len() as u16;
        self.string_constants.push(s);
        idx
    }

    /// Get a string from the string constant pool
    pub fn get_string(&self, idx: u16) -> Option<&str> {
        self.string_constants.get(idx as usize).map(|s| s.as_str())
    }

    /// Emit a single byte
    pub fn emit_u8(&mut self, byte: u8) {
        self.bytecode.push(byte);
    }

    /// Emit a u16 (little-endian)
    pub fn emit_u16(&mut self, value: u16) {
        self.bytecode.extend_from_slice(&value.to_le_bytes());
    }

    /// Emit a u32 (little-endian)
    pub fn emit_u32(&mut self, value: u32) {
        self.bytecode.extend_from_slice(&value.to_le_bytes());
    }

    /// Get current bytecode offset
    pub fn current_offset(&self) -> u32 {
        self.bytecode.len() as u32
    }

    /// Patch a u16 at a given offset
    pub fn patch_u16(&mut self, offset: u32, value: u16) {
        let bytes = value.to_le_bytes();
        self.bytecode[offset as usize] = bytes[0];
        self.bytecode[offset as usize + 1] = bytes[1];
    }

    /// Add a line number entry
    pub fn add_line_number(&mut self, pc: u32, line: u32) {
        self.line_numbers.push((pc, line));
    }

    /// Get line number for a PC value
    pub fn get_line_number(&self, pc: u32) -> Option<u32> {
        // Binary search for the entry with pc <= target
        let idx = self
            .line_numbers
            .partition_point(|&(p, _)| p <= pc)
            .saturating_sub(1);

        self.line_numbers.get(idx).map(|&(_, line)| line)
    }

    /// Calculate required stack size (simple estimate)
    pub fn calculate_stack_size(&mut self) {
        // This is a simplified version - a real implementation
        // would analyze the bytecode to find max stack depth
        self.stack_size = self.local_count.saturating_add(16);
    }

    /// Serialize to bytes for bytecode file format
    pub fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::new();

        // Function name
        if let Some(ref name) = self.name {
            out.extend_from_slice(&(name.len() as u32).to_le_bytes());
            out.extend_from_slice(name.as_bytes());
        } else {
            out.extend_from_slice(&0u32.to_le_bytes());
        }

        // Function metadata
        out.extend_from_slice(&self.arg_count.to_le_bytes());
        out.extend_from_slice(&self.local_count.to_le_bytes());
        out.extend_from_slice(&self.stack_size.to_le_bytes());
        out.push(self.has_arguments as u8);

        // Bytecode
        out.extend_from_slice(&(self.bytecode.len() as u32).to_le_bytes());
        out.extend_from_slice(&self.bytecode);

        // Constants (only serialize integers and special values for now)
        out.extend_from_slice(&(self.constants.len() as u32).to_le_bytes());
        for val in &self.constants {
            // Serialize the raw value
            let raw = val.0.0 as u64;
            out.extend_from_slice(&raw.to_le_bytes());
        }

        // String constants
        out.extend_from_slice(&(self.string_constants.len() as u32).to_le_bytes());
        for s in &self.string_constants {
            out.extend_from_slice(&(s.len() as u32).to_le_bytes());
            out.extend_from_slice(s.as_bytes());
        }

        // Source file (optional)
        if let Some(ref file) = self.source_file {
            out.extend_from_slice(&(file.len() as u32).to_le_bytes());
            out.extend_from_slice(file.as_bytes());
        } else {
            out.extend_from_slice(&0u32.to_le_bytes());
        }

        // Line numbers
        out.extend_from_slice(&(self.line_numbers.len() as u32).to_le_bytes());
        for &(pc, line) in &self.line_numbers {
            out.extend_from_slice(&pc.to_le_bytes());
            out.extend_from_slice(&line.to_le_bytes());
        }

        // Inner functions (recursive)
        out.extend_from_slice(&(self.inner_functions.len() as u32).to_le_bytes());
        for func in &self.inner_functions {
            let func_bytes = func.serialize();
            out.extend_from_slice(&(func_bytes.len() as u32).to_le_bytes());
            out.extend_from_slice(&func_bytes);
        }

        // Captures
        out.extend_from_slice(&(self.captures.len() as u32).to_le_bytes());
        for cap in &self.captures {
            out.extend_from_slice(&(cap.outer_index as u32).to_le_bytes());
            out.push(cap.is_local as u8);
        }

        out
    }

    /// Deserialize from bytes
    pub fn deserialize(data: &[u8]) -> Result<(Self, usize), String> {
        let mut pos = 0;

        // Helper to read bytes
        let read_u32 = |pos: &mut usize| -> Result<u32, String> {
            if *pos + 4 > data.len() {
                return Err("unexpected end of data".to_string());
            }
            let val = u32::from_le_bytes([data[*pos], data[*pos + 1], data[*pos + 2], data[*pos + 3]]);
            *pos += 4;
            Ok(val)
        };
        let read_u16 = |pos: &mut usize| -> Result<u16, String> {
            if *pos + 2 > data.len() {
                return Err("unexpected end of data".to_string());
            }
            let val = u16::from_le_bytes([data[*pos], data[*pos + 1]]);
            *pos += 2;
            Ok(val)
        };
        let read_u8 = |pos: &mut usize| -> Result<u8, String> {
            if *pos >= data.len() {
                return Err("unexpected end of data".to_string());
            }
            let val = data[*pos];
            *pos += 1;
            Ok(val)
        };
        let read_u64 = |pos: &mut usize| -> Result<u64, String> {
            if *pos + 8 > data.len() {
                return Err("unexpected end of data".to_string());
            }
            let val = u64::from_le_bytes([
                data[*pos], data[*pos + 1], data[*pos + 2], data[*pos + 3],
                data[*pos + 4], data[*pos + 5], data[*pos + 6], data[*pos + 7],
            ]);
            *pos += 8;
            Ok(val)
        };
        let read_string = |pos: &mut usize| -> Result<String, String> {
            let len = read_u32(pos)? as usize;
            if *pos + len > data.len() {
                return Err("unexpected end of data".to_string());
            }
            let s = String::from_utf8(data[*pos..*pos + len].to_vec())
                .map_err(|e| format!("invalid utf-8: {}", e))?;
            *pos += len;
            Ok(s)
        };

        // Function name
        let name_len = read_u32(&mut pos)? as usize;
        let name = if name_len > 0 {
            if pos + name_len > data.len() {
                return Err("unexpected end of data".to_string());
            }
            let s = String::from_utf8(data[pos..pos + name_len].to_vec())
                .map_err(|e| format!("invalid utf-8: {}", e))?;
            pos += name_len;
            Some(s)
        } else {
            None
        };

        // Function metadata
        let arg_count = read_u16(&mut pos)?;
        let local_count = read_u16(&mut pos)?;
        let stack_size = read_u16(&mut pos)?;
        let has_arguments = read_u8(&mut pos)? != 0;

        // Bytecode
        let bytecode_len = read_u32(&mut pos)? as usize;
        if pos + bytecode_len > data.len() {
            return Err("unexpected end of data".to_string());
        }
        let bytecode = data[pos..pos + bytecode_len].to_vec();
        pos += bytecode_len;

        // Constants
        let const_count = read_u32(&mut pos)? as usize;
        let mut constants = Vec::with_capacity(const_count);
        for _ in 0..const_count {
            let raw = read_u64(&mut pos)? as usize;
            constants.push(Value(crate::value::RawValue(raw)));
        }

        // String constants
        let str_count = read_u32(&mut pos)? as usize;
        let mut string_constants = Vec::with_capacity(str_count);
        for _ in 0..str_count {
            string_constants.push(read_string(&mut pos)?);
        }

        // Source file
        let source_file = {
            let len = read_u32(&mut pos)? as usize;
            if len > 0 {
                if pos + len > data.len() {
                    return Err("unexpected end of data".to_string());
                }
                let s = String::from_utf8(data[pos..pos + len].to_vec())
                    .map_err(|e| format!("invalid utf-8: {}", e))?;
                pos += len;
                Some(s)
            } else {
                None
            }
        };

        // Line numbers
        let line_count = read_u32(&mut pos)? as usize;
        let mut line_numbers = Vec::with_capacity(line_count);
        for _ in 0..line_count {
            let pc = read_u32(&mut pos)?;
            let line = read_u32(&mut pos)?;
            line_numbers.push((pc, line));
        }

        // Inner functions
        let inner_count = read_u32(&mut pos)? as usize;
        let mut inner_functions = Vec::with_capacity(inner_count);
        for _ in 0..inner_count {
            let func_len = read_u32(&mut pos)? as usize;
            if pos + func_len > data.len() {
                return Err("unexpected end of data".to_string());
            }
            let (func, _) = Self::deserialize(&data[pos..pos + func_len])?;
            inner_functions.push(func);
            pos += func_len;
        }

        // Captures
        let cap_count = read_u32(&mut pos)? as usize;
        let mut captures = Vec::with_capacity(cap_count);
        for _ in 0..cap_count {
            let outer_index = read_u32(&mut pos)? as usize;
            let is_local = read_u8(&mut pos)? != 0;
            captures.push(CaptureInfo { outer_index, is_local });
        }

        Ok((FunctionBytecode {
            name,
            arg_count,
            local_count,
            stack_size,
            has_arguments,
            bytecode,
            constants,
            string_constants,
            source_file,
            line_numbers,
            inner_functions,
            captures,
        }, pos))
    }
}

/// Function kind
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionKind {
    /// Regular function
    Normal,
    /// Arrow function (lexical this)
    Arrow,
    /// Method
    Method,
    /// Getter
    Getter,
    /// Setter
    Setter,
    /// Constructor
    Constructor,
}

impl FunctionKind {
    /// Check if this function kind has its own 'this' binding
    pub fn has_this_binding(self) -> bool {
        !matches!(self, FunctionKind::Arrow)
    }

    /// Check if this function can be used with 'new'
    pub fn is_constructor(self) -> bool {
        matches!(self, FunctionKind::Normal | FunctionKind::Constructor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_c_function() {
        let cfunc = CFunction::new(42);
        assert_eq!(cfunc.idx, 42);
        assert!(cfunc.params.is_undefined());

        let cfunc = CFunction::with_params(10, Value::int(5));
        assert_eq!(cfunc.idx, 10);
        assert_eq!(cfunc.params, Value::int(5));
    }

    #[test]
    fn test_var_ref() {
        let mut var_ref = VarRef::attached(5);
        assert!(!var_ref.is_detached);

        var_ref.detach(Value::int(100));
        assert!(var_ref.is_detached);
        assert_eq!(var_ref.value, Value::int(100));
    }

    #[test]
    fn test_closure() {
        let bytecode = Value::null(); // Placeholder
        let var_refs = vec![VarRef::detached(Value::int(1)), VarRef::detached(Value::int(2))];

        let closure = Closure::with_var_refs(bytecode, var_refs);
        assert_eq!(closure.var_refs.len(), 2);
    }

    #[test]
    fn test_function_bytecode() {
        let mut fb = FunctionBytecode::new(2, 3);
        fb.set_name("myFunction");

        assert_eq!(fb.arg_count, 2);
        assert_eq!(fb.local_count, 3);
        assert_eq!(fb.name, Some("myFunction".to_string()));

        let idx = fb.add_constant(Value::int(42));
        assert_eq!(fb.get_constant(idx), Some(Value::int(42)));
    }

    #[test]
    fn test_bytecode_emit() {
        let mut fb = FunctionBytecode::new(0, 0);

        fb.emit_u8(0x01);
        fb.emit_u16(0x1234);
        fb.emit_u32(0x12345678);

        assert_eq!(fb.bytecode.len(), 7);
        assert_eq!(fb.bytecode[0], 0x01);
        assert_eq!(fb.bytecode[1], 0x34);
        assert_eq!(fb.bytecode[2], 0x12);
    }

    #[test]
    fn test_line_numbers() {
        let mut fb = FunctionBytecode::new(0, 0);

        fb.add_line_number(0, 1);
        fb.add_line_number(10, 5);
        fb.add_line_number(20, 10);

        assert_eq!(fb.get_line_number(0), Some(1));
        assert_eq!(fb.get_line_number(5), Some(1));
        assert_eq!(fb.get_line_number(10), Some(5));
        assert_eq!(fb.get_line_number(15), Some(5));
        assert_eq!(fb.get_line_number(25), Some(10));
    }

    #[test]
    fn test_function_kind() {
        assert!(FunctionKind::Normal.has_this_binding());
        assert!(!FunctionKind::Arrow.has_this_binding());

        assert!(FunctionKind::Normal.is_constructor());
        assert!(FunctionKind::Constructor.is_constructor());
        assert!(!FunctionKind::Arrow.is_constructor());
        assert!(!FunctionKind::Method.is_constructor());
    }
}
