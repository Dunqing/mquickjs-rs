//! Bytecode opcode definitions
//!
//! Ported from mquickjs_opcode.h
//!
//! The bytecode is stack-based. Each opcode has:
//! - A size in bytes
//! - Number of values popped from stack (n_pop)
//! - Number of values pushed to stack (n_push)
//! - An operand format

/// Opcode operand formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OpFormat {
    /// No operand
    None,
    /// No operand, but represents an integer constant (-1 to 7)
    NoneInt,
    /// No operand, but represents a local variable (0 to 3)
    NoneLoc,
    /// No operand, but represents an argument (0 to 3)
    NoneArg,
    /// No operand, but represents a var ref
    NoneVarRef,
    /// Unsigned 8-bit operand
    U8,
    /// Signed 8-bit operand
    I8,
    /// 8-bit local variable index
    Loc8,
    /// 8-bit constant pool index
    Const8,
    /// 8-bit label offset
    Label8,
    /// Unsigned 16-bit operand
    U16,
    /// Signed 16-bit operand
    I16,
    /// 16-bit label offset
    Label16,
    /// 16-bit argument count for calls
    NPop,
    /// Implicit argument count (0-3)
    NPopX,
    /// 16-bit local variable index
    Loc,
    /// 16-bit argument index
    Arg,
    /// 16-bit var ref index
    VarRef,
    /// Unsigned 32-bit operand
    U32,
    /// Signed 32-bit operand
    I32,
    /// 16-bit constant pool index
    Const16,
    /// 32-bit label offset
    Label,
    /// Inline JSValue (word-sized)
    Value,
}

/// JavaScript bytecode opcodes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OpCode {
    /// Invalid opcode (never emitted)
    Invalid = 0,

    // Push values
    /// Push inline value
    PushValue,
    /// Push constant from pool (16-bit index)
    PushConst,
    /// Create closure from constant pool function
    FClosure,
    /// Push undefined
    Undefined,
    /// Push null
    Null,
    /// Push `this` value
    PushThis,
    /// Push false
    PushFalse,
    /// Push true
    PushTrue,
    /// Create empty object with class (16-bit class id)
    Object,
    /// Push current function
    ThisFunc,
    /// Push arguments object
    Arguments,
    /// Push new.target
    NewTarget,

    // Stack manipulation
    /// Drop top value: a ->
    Drop,
    /// Remove second value: a b -> b
    Nip,
    /// Duplicate top: a -> a a
    Dup,
    /// Duplicate second: a b -> a a b
    Dup1,
    /// Duplicate top two: a b -> a b a b
    Dup2,
    /// Insert copy: obj a -> a obj a
    Insert2,
    /// Insert copy: obj prop a -> a obj prop a
    Insert3,
    /// Permute: obj a b -> a obj b
    Perm3,
    /// Permute: obj prop a b -> a obj prop b
    Perm4,
    /// Swap top two: a b -> b a
    Swap,
    /// Rotate left 3: x a b -> a b x
    Rot3L,

    // Function calls
    /// Call constructor: func args... -> ret
    CallConstructor,
    /// Call function: func args... -> ret
    Call,
    /// Call method: this func args... -> ret
    CallMethod,
    /// Create array from stack values
    ArrayFrom,
    /// Return from function
    Return,
    /// Return undefined
    ReturnUndef,
    /// Throw exception
    Throw,
    /// Create RegExp from pattern and bytecode strings
    Regexp,

    // Property access
    /// Get property by name: obj -> val
    GetField,
    /// Get property, keep object: obj -> obj val
    GetField2,
    /// Set property by name: obj val ->
    PutField,
    /// Get array element: obj prop -> val
    GetArrayEl,
    /// Get array element, keep object: obj prop -> obj val
    GetArrayEl2,
    /// Set array element: obj prop val ->
    PutArrayEl,
    /// Get length property: obj -> val
    GetLength,
    /// Get length, keep object: obj -> obj val
    GetLength2,
    /// Define property: obj val -> obj
    DefineField,
    /// Define getter: obj val -> obj
    DefineGetter,
    /// Define setter: obj val -> obj
    DefineSetter,
    /// Set prototype: obj proto -> obj
    SetProto,

    // Local/argument access
    /// Get local variable (16-bit index)
    GetLoc,
    /// Set local variable (16-bit index)
    PutLoc,
    /// Get argument (16-bit index)
    GetArg,
    /// Set argument (16-bit index)
    PutArg,
    /// Get var ref (16-bit index)
    GetVarRef,
    /// Set var ref (16-bit index)
    PutVarRef,
    /// Get var ref (no TDZ check)
    GetVarRefNoCheck,
    /// Set var ref (no TDZ check)
    PutVarRefNoCheck,

    // Control flow
    /// Jump if false (32-bit offset)
    IfFalse,
    /// Jump if true (32-bit offset)
    IfTrue,
    /// Unconditional jump (32-bit offset)
    Goto,
    /// Set up catch handler (32-bit offset to catch block)
    Catch,
    /// Remove catch handler (when try block completes normally)
    DropCatch,
    /// Jump to finally block (push return address)
    Gosub,
    /// Return from finally block (pop return address)
    Ret,

    // Iteration
    /// Start for-in iteration: obj -> iter
    ForInStart,
    /// Start for-of iteration: obj -> iter
    ForOfStart,
    /// Get next for-of value: iter -> iter val done
    ForOfNext,

    // Arithmetic/logic operations
    /// Negate: -a
    Neg,
    /// Unary plus: +a
    Plus,
    /// Decrement: a - 1
    Dec,
    /// Increment: a + 1
    Inc,
    /// Post-decrement: a -> (a-1) a
    PostDec,
    /// Post-increment: a -> (a+1) a
    PostInc,
    /// Bitwise NOT: ~a
    Not,
    /// Logical NOT: !a
    LNot,
    /// typeof operator
    TypeOf,
    /// delete operator: obj prop -> bool
    Delete,

    // Binary arithmetic
    /// Multiply: a * b
    Mul,
    /// Divide: a / b
    Div,
    /// Modulo: a % b
    Mod,
    /// Add: a + b
    Add,
    /// Subtract: a - b
    Sub,
    /// Power: a ** b
    Pow,
    /// Left shift: a << b
    Shl,
    /// Arithmetic right shift: a >> b
    Sar,
    /// Logical right shift: a >>> b
    Shr,

    // Comparison
    /// Less than: a < b
    Lt,
    /// Less than or equal: a <= b
    Lte,
    /// Greater than: a > b
    Gt,
    /// Greater than or equal: a >= b
    Gte,
    /// instanceof operator
    InstanceOf,
    /// in operator
    In,
    /// Equal: a == b
    Eq,
    /// Not equal: a != b
    Neq,
    /// Strict equal: a === b
    StrictEq,
    /// Strict not equal: a !== b
    StrictNeq,

    // Bitwise
    /// Bitwise AND: a & b
    And,
    /// Bitwise XOR: a ^ b
    Xor,
    /// Bitwise OR: a | b
    Or,

    /// No operation
    Nop,

    // Short forms (optimized)
    /// Push -1
    PushMinus1,
    /// Push 0
    Push0,
    /// Push 1
    Push1,
    /// Push 2
    Push2,
    /// Push 3
    Push3,
    /// Push 4
    Push4,
    /// Push 5
    Push5,
    /// Push 6
    Push6,
    /// Push 7
    Push7,
    /// Push 8-bit signed integer
    PushI8,
    /// Push 16-bit signed integer
    PushI16,
    /// Push constant (8-bit index)
    PushConst8,
    /// Create closure (8-bit index)
    FClosure8,
    /// Push empty string
    PushEmptyString,

    /// Get local 8-bit index
    GetLoc8,
    /// Set local 8-bit index
    PutLoc8,

    /// Get local 0
    GetLoc0,
    /// Get local 1
    GetLoc1,
    /// Get local 2
    GetLoc2,
    /// Get local 3
    GetLoc3,
    /// Set local 0
    PutLoc0,
    /// Set local 1
    PutLoc1,
    /// Set local 2
    PutLoc2,
    /// Set local 3
    PutLoc3,

    /// Get argument 0
    GetArg0,
    /// Get argument 1
    GetArg1,
    /// Get argument 2
    GetArg2,
    /// Get argument 3
    GetArg3,
    /// Set argument 0
    PutArg0,
    /// Set argument 1
    PutArg1,
    /// Set argument 2
    PutArg2,
    /// Set argument 3
    PutArg3,

    // Built-in operations
    /// Print value to stdout (pops value, pushes undefined)
    Print,
}

impl OpCode {
    /// Total number of opcodes
    pub const COUNT: usize = OpCode::Print as usize + 1;
}

/// Opcode metadata
#[derive(Debug, Clone, Copy)]
pub struct OpCodeInfo {
    /// Opcode size in bytes
    pub size: u8,
    /// Number of values popped
    pub n_pop: u8,
    /// Number of values pushed
    pub n_push: u8,
    /// Operand format
    pub format: OpFormat,
}

impl OpCodeInfo {
    const fn new(size: u8, n_pop: u8, n_push: u8, format: OpFormat) -> Self {
        OpCodeInfo { size, n_pop, n_push, format }
    }
}

/// Opcode information table
pub static OPCODE_INFO: [OpCodeInfo; OpCode::COUNT] = [
    // Invalid
    OpCodeInfo::new(1, 0, 0, OpFormat::None),
    // PushValue
    OpCodeInfo::new(5, 0, 1, OpFormat::Value),
    // PushConst
    OpCodeInfo::new(3, 0, 1, OpFormat::Const16),
    // FClosure
    OpCodeInfo::new(3, 0, 1, OpFormat::Const16),
    // Undefined
    OpCodeInfo::new(1, 0, 1, OpFormat::None),
    // Null
    OpCodeInfo::new(1, 0, 1, OpFormat::None),
    // PushThis
    OpCodeInfo::new(1, 0, 1, OpFormat::None),
    // PushFalse
    OpCodeInfo::new(1, 0, 1, OpFormat::None),
    // PushTrue
    OpCodeInfo::new(1, 0, 1, OpFormat::None),
    // Object
    OpCodeInfo::new(3, 0, 1, OpFormat::U16),
    // ThisFunc
    OpCodeInfo::new(1, 0, 1, OpFormat::None),
    // Arguments
    OpCodeInfo::new(1, 0, 1, OpFormat::None),
    // NewTarget
    OpCodeInfo::new(1, 0, 1, OpFormat::None),
    // Drop
    OpCodeInfo::new(1, 1, 0, OpFormat::None),
    // Nip
    OpCodeInfo::new(1, 2, 1, OpFormat::None),
    // Dup
    OpCodeInfo::new(1, 1, 2, OpFormat::None),
    // Dup1
    OpCodeInfo::new(1, 2, 3, OpFormat::None),
    // Dup2
    OpCodeInfo::new(1, 2, 4, OpFormat::None),
    // Insert2
    OpCodeInfo::new(1, 2, 3, OpFormat::None),
    // Insert3
    OpCodeInfo::new(1, 3, 4, OpFormat::None),
    // Perm3
    OpCodeInfo::new(1, 3, 3, OpFormat::None),
    // Perm4
    OpCodeInfo::new(1, 4, 4, OpFormat::None),
    // Swap
    OpCodeInfo::new(1, 2, 2, OpFormat::None),
    // Rot3L
    OpCodeInfo::new(1, 3, 3, OpFormat::None),
    // CallConstructor
    OpCodeInfo::new(3, 1, 1, OpFormat::NPop),
    // Call
    OpCodeInfo::new(3, 1, 1, OpFormat::NPop),
    // CallMethod
    OpCodeInfo::new(3, 2, 1, OpFormat::NPop),
    // ArrayFrom
    OpCodeInfo::new(3, 0, 1, OpFormat::NPop),
    // Return
    OpCodeInfo::new(1, 1, 0, OpFormat::None),
    // ReturnUndef
    OpCodeInfo::new(1, 0, 0, OpFormat::None),
    // Throw
    OpCodeInfo::new(1, 1, 0, OpFormat::None),
    // Regexp
    OpCodeInfo::new(1, 2, 1, OpFormat::None),
    // GetField
    OpCodeInfo::new(3, 1, 1, OpFormat::Const16),
    // GetField2
    OpCodeInfo::new(3, 1, 2, OpFormat::Const16),
    // PutField
    OpCodeInfo::new(3, 2, 0, OpFormat::Const16),
    // GetArrayEl
    OpCodeInfo::new(1, 2, 1, OpFormat::None),
    // GetArrayEl2
    OpCodeInfo::new(1, 2, 2, OpFormat::None),
    // PutArrayEl
    OpCodeInfo::new(1, 3, 0, OpFormat::None),
    // GetLength
    OpCodeInfo::new(1, 1, 1, OpFormat::None),
    // GetLength2
    OpCodeInfo::new(1, 1, 2, OpFormat::None),
    // DefineField
    OpCodeInfo::new(3, 2, 1, OpFormat::Const16),
    // DefineGetter
    OpCodeInfo::new(3, 2, 1, OpFormat::Const16),
    // DefineSetter
    OpCodeInfo::new(3, 2, 1, OpFormat::Const16),
    // SetProto
    OpCodeInfo::new(1, 2, 1, OpFormat::None),
    // GetLoc
    OpCodeInfo::new(3, 0, 1, OpFormat::Loc),
    // PutLoc
    OpCodeInfo::new(3, 1, 0, OpFormat::Loc),
    // GetArg
    OpCodeInfo::new(3, 0, 1, OpFormat::Arg),
    // PutArg
    OpCodeInfo::new(3, 1, 0, OpFormat::Arg),
    // GetVarRef
    OpCodeInfo::new(3, 0, 1, OpFormat::VarRef),
    // PutVarRef
    OpCodeInfo::new(3, 1, 0, OpFormat::VarRef),
    // GetVarRefNoCheck
    OpCodeInfo::new(3, 0, 1, OpFormat::VarRef),
    // PutVarRefNoCheck
    OpCodeInfo::new(3, 1, 0, OpFormat::VarRef),
    // IfFalse
    OpCodeInfo::new(5, 1, 0, OpFormat::Label),
    // IfTrue
    OpCodeInfo::new(5, 1, 0, OpFormat::Label),
    // Goto
    OpCodeInfo::new(5, 0, 0, OpFormat::Label),
    // Catch
    OpCodeInfo::new(5, 0, 1, OpFormat::Label),
    // DropCatch
    OpCodeInfo::new(1, 0, 0, OpFormat::None),
    // Gosub
    OpCodeInfo::new(5, 0, 0, OpFormat::Label),
    // Ret
    OpCodeInfo::new(1, 1, 0, OpFormat::None),
    // ForInStart
    OpCodeInfo::new(1, 1, 1, OpFormat::None),
    // ForOfStart
    OpCodeInfo::new(1, 1, 1, OpFormat::None),
    // ForOfNext
    OpCodeInfo::new(1, 1, 3, OpFormat::None),
    // Neg
    OpCodeInfo::new(1, 1, 1, OpFormat::None),
    // Plus
    OpCodeInfo::new(1, 1, 1, OpFormat::None),
    // Dec
    OpCodeInfo::new(1, 1, 1, OpFormat::None),
    // Inc
    OpCodeInfo::new(1, 1, 1, OpFormat::None),
    // PostDec
    OpCodeInfo::new(1, 1, 2, OpFormat::None),
    // PostInc
    OpCodeInfo::new(1, 1, 2, OpFormat::None),
    // Not
    OpCodeInfo::new(1, 1, 1, OpFormat::None),
    // LNot
    OpCodeInfo::new(1, 1, 1, OpFormat::None),
    // TypeOf
    OpCodeInfo::new(1, 1, 1, OpFormat::None),
    // Delete
    OpCodeInfo::new(1, 2, 1, OpFormat::None),
    // Mul
    OpCodeInfo::new(1, 2, 1, OpFormat::None),
    // Div
    OpCodeInfo::new(1, 2, 1, OpFormat::None),
    // Mod
    OpCodeInfo::new(1, 2, 1, OpFormat::None),
    // Add
    OpCodeInfo::new(1, 2, 1, OpFormat::None),
    // Sub
    OpCodeInfo::new(1, 2, 1, OpFormat::None),
    // Pow
    OpCodeInfo::new(1, 2, 1, OpFormat::None),
    // Shl
    OpCodeInfo::new(1, 2, 1, OpFormat::None),
    // Sar
    OpCodeInfo::new(1, 2, 1, OpFormat::None),
    // Shr
    OpCodeInfo::new(1, 2, 1, OpFormat::None),
    // Lt
    OpCodeInfo::new(1, 2, 1, OpFormat::None),
    // Lte
    OpCodeInfo::new(1, 2, 1, OpFormat::None),
    // Gt
    OpCodeInfo::new(1, 2, 1, OpFormat::None),
    // Gte
    OpCodeInfo::new(1, 2, 1, OpFormat::None),
    // InstanceOf
    OpCodeInfo::new(1, 2, 1, OpFormat::None),
    // In
    OpCodeInfo::new(1, 2, 1, OpFormat::None),
    // Eq
    OpCodeInfo::new(1, 2, 1, OpFormat::None),
    // Neq
    OpCodeInfo::new(1, 2, 1, OpFormat::None),
    // StrictEq
    OpCodeInfo::new(1, 2, 1, OpFormat::None),
    // StrictNeq
    OpCodeInfo::new(1, 2, 1, OpFormat::None),
    // And
    OpCodeInfo::new(1, 2, 1, OpFormat::None),
    // Xor
    OpCodeInfo::new(1, 2, 1, OpFormat::None),
    // Or
    OpCodeInfo::new(1, 2, 1, OpFormat::None),
    // Nop
    OpCodeInfo::new(1, 0, 0, OpFormat::None),
    // PushMinus1
    OpCodeInfo::new(1, 0, 1, OpFormat::NoneInt),
    // Push0
    OpCodeInfo::new(1, 0, 1, OpFormat::NoneInt),
    // Push1
    OpCodeInfo::new(1, 0, 1, OpFormat::NoneInt),
    // Push2
    OpCodeInfo::new(1, 0, 1, OpFormat::NoneInt),
    // Push3
    OpCodeInfo::new(1, 0, 1, OpFormat::NoneInt),
    // Push4
    OpCodeInfo::new(1, 0, 1, OpFormat::NoneInt),
    // Push5
    OpCodeInfo::new(1, 0, 1, OpFormat::NoneInt),
    // Push6
    OpCodeInfo::new(1, 0, 1, OpFormat::NoneInt),
    // Push7
    OpCodeInfo::new(1, 0, 1, OpFormat::NoneInt),
    // PushI8
    OpCodeInfo::new(2, 0, 1, OpFormat::I8),
    // PushI16
    OpCodeInfo::new(3, 0, 1, OpFormat::I16),
    // PushConst8
    OpCodeInfo::new(2, 0, 1, OpFormat::Const8),
    // FClosure8
    OpCodeInfo::new(2, 0, 1, OpFormat::Const8),
    // PushEmptyString
    OpCodeInfo::new(1, 0, 1, OpFormat::None),
    // GetLoc8
    OpCodeInfo::new(2, 0, 1, OpFormat::Loc8),
    // PutLoc8
    OpCodeInfo::new(2, 1, 0, OpFormat::Loc8),
    // GetLoc0
    OpCodeInfo::new(1, 0, 1, OpFormat::NoneLoc),
    // GetLoc1
    OpCodeInfo::new(1, 0, 1, OpFormat::NoneLoc),
    // GetLoc2
    OpCodeInfo::new(1, 0, 1, OpFormat::NoneLoc),
    // GetLoc3
    OpCodeInfo::new(1, 0, 1, OpFormat::NoneLoc),
    // PutLoc0
    OpCodeInfo::new(1, 1, 0, OpFormat::NoneLoc),
    // PutLoc1
    OpCodeInfo::new(1, 1, 0, OpFormat::NoneLoc),
    // PutLoc2
    OpCodeInfo::new(1, 1, 0, OpFormat::NoneLoc),
    // PutLoc3
    OpCodeInfo::new(1, 1, 0, OpFormat::NoneLoc),
    // GetArg0
    OpCodeInfo::new(1, 0, 1, OpFormat::NoneArg),
    // GetArg1
    OpCodeInfo::new(1, 0, 1, OpFormat::NoneArg),
    // GetArg2
    OpCodeInfo::new(1, 0, 1, OpFormat::NoneArg),
    // GetArg3
    OpCodeInfo::new(1, 0, 1, OpFormat::NoneArg),
    // PutArg0
    OpCodeInfo::new(1, 1, 0, OpFormat::NoneArg),
    // PutArg1
    OpCodeInfo::new(1, 1, 0, OpFormat::NoneArg),
    // PutArg2
    OpCodeInfo::new(1, 1, 0, OpFormat::NoneArg),
    // PutArg3
    OpCodeInfo::new(1, 1, 0, OpFormat::NoneArg),
    // Print
    OpCodeInfo::new(1, 1, 0, OpFormat::None),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opcode_count() {
        assert_eq!(OPCODE_INFO.len(), OpCode::COUNT);
    }

    #[test]
    fn test_opcode_sizes() {
        // Verify some known opcode sizes
        assert_eq!(OPCODE_INFO[OpCode::Drop as usize].size, 1);
        assert_eq!(OPCODE_INFO[OpCode::PushI8 as usize].size, 2);
        assert_eq!(OPCODE_INFO[OpCode::PushConst as usize].size, 3);
        assert_eq!(OPCODE_INFO[OpCode::Goto as usize].size, 5);
    }
}
