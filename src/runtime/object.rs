//! JavaScript object representation
//!
//! This module implements the JSObject struct and related types for
//! representing JavaScript objects in the engine.

use crate::gc::MemoryTag;
use crate::value::Value;

/// JavaScript class IDs
///
/// These identify the type of a JavaScript object and determine its behavior.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassId {
    /// Plain object
    Object = 0,
    /// Array
    Array = 1,
    /// C function (native)
    CFunction = 2,
    /// Closure (JS function with captured variables)
    Closure = 3,
    /// Boxed Number
    Number = 4,
    /// Boxed Boolean
    Boolean = 5,
    /// Boxed String
    String = 6,
    /// Date object
    Date = 7,
    /// RegExp object
    RegExp = 8,

    /// Error types
    Error = 9,
    EvalError = 10,
    RangeError = 11,
    ReferenceError = 12,
    SyntaxError = 13,
    TypeError = 14,
    UriError = 15,
    InternalError = 16,

    /// ArrayBuffer
    ArrayBuffer = 17,
    /// TypedArray (base)
    TypedArray = 18,

    /// Specific typed array types
    Uint8ClampedArray = 19,
    Int8Array = 20,
    Uint8Array = 21,
    Int16Array = 22,
    Uint16Array = 23,
    Int32Array = 24,
    Uint32Array = 25,
    Float32Array = 26,
    Float64Array = 27,

    /// User-defined classes start here
    User = 28,
}

impl ClassId {
    /// First typed array class ID
    pub const TYPED_ARRAY_FIRST: ClassId = ClassId::Uint8ClampedArray;
    /// Last typed array class ID
    pub const TYPED_ARRAY_LAST: ClassId = ClassId::Float64Array;

    /// Check if this is a typed array class
    #[inline]
    pub fn is_typed_array(self) -> bool {
        (self as u8) >= (Self::TYPED_ARRAY_FIRST as u8)
            && (self as u8) <= (Self::TYPED_ARRAY_LAST as u8)
    }

    /// Check if this is an error class
    #[inline]
    pub fn is_error(self) -> bool {
        (self as u8) >= (ClassId::Error as u8) && (self as u8) <= (ClassId::InternalError as u8)
    }

    /// Check if this is a function class
    #[inline]
    pub fn is_function(self) -> bool {
        matches!(self, ClassId::CFunction | ClassId::Closure)
    }
}

/// Property type
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyType {
    /// Normal property with value
    Normal = 0,
    /// Getter/setter property (value is array of [getter, setter])
    GetSet = 1,
    /// Variable reference (for closures)
    VarRef = 2,
    /// Index property (for arrays)
    Index = 3,
}

/// A property in an object's property table
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Property {
    /// Property key (string or integer as Value)
    pub key: Value,
    /// Property value (meaning depends on prop_type)
    pub value: Value,
    /// Hash chain next pointer (30 bits) and property type (2 bits)
    /// Layout: hash_next (30 bits) | prop_type (2 bits)
    hash_next_and_type: u32,
}

impl Property {
    /// Create a new normal property
    #[inline]
    pub fn new(key: Value, value: Value) -> Self {
        Property {
            key,
            value,
            hash_next_and_type: 0, // Normal type, no hash chain
        }
    }

    /// Get the property type
    #[inline]
    pub fn prop_type(&self) -> PropertyType {
        match self.hash_next_and_type & 0x3 {
            0 => PropertyType::Normal,
            1 => PropertyType::GetSet,
            2 => PropertyType::VarRef,
            3 => PropertyType::Index,
            _ => unreachable!(),
        }
    }

    /// Set the property type
    #[inline]
    pub fn set_prop_type(&mut self, prop_type: PropertyType) {
        self.hash_next_and_type =
            (self.hash_next_and_type & !0x3) | (prop_type as u32);
    }

    /// Get hash chain next index
    #[inline]
    pub fn hash_next(&self) -> u32 {
        self.hash_next_and_type >> 2
    }

    /// Set hash chain next index
    #[inline]
    pub fn set_hash_next(&mut self, next: u32) {
        self.hash_next_and_type = (next << 2) | (self.hash_next_and_type & 0x3);
    }
}

/// Object header - common fields for all GC-managed objects
#[repr(C)]
pub struct ObjectHeader {
    /// GC mark bit (1 bit) and memory tag (3 bits)
    /// Layout: gc_mark (1 bit) | mtag (3 bits) | ... (remaining bits)
    pub header_bits: usize,
}

impl ObjectHeader {
    /// Number of bits for memory tag
    const MTAG_BITS: u32 = 4;

    /// Create a new header
    #[inline]
    pub fn new(tag: MemoryTag) -> Self {
        ObjectHeader {
            header_bits: (tag as usize) << 1,
        }
    }

    /// Get the GC mark bit
    #[inline]
    pub fn is_marked(&self) -> bool {
        (self.header_bits & 1) != 0
    }

    /// Set the GC mark bit
    #[inline]
    pub fn set_marked(&mut self, marked: bool) {
        if marked {
            self.header_bits |= 1;
        } else {
            self.header_bits &= !1;
        }
    }

    /// Get the memory tag
    #[inline]
    pub fn mtag(&self) -> MemoryTag {
        // Safety: We only store valid MemoryTag values
        unsafe { std::mem::transmute(((self.header_bits >> 1) & 0x7) as u8) }
    }
}

/// JavaScript object
///
/// This is the core object representation. All JavaScript objects
/// (including arrays, functions, etc.) use this structure.
#[repr(C)]
pub struct JSObject {
    /// GC header bits
    header_bits: usize,

    /// Prototype of this object (JSObject pointer or null)
    pub proto: Value,

    /// Properties array (pointer to property table)
    /// Structure: prop_count, hash_mask, hash_table[], props[]
    pub props: Value,
    // Class-specific data follows (variable size based on class_id)
    // The union data is stored inline after the fixed fields
}

impl JSObject {
    /// Bits reserved for memory tag and GC mark
    const HEADER_BITS: u32 = 4;

    /// Create header bits with class ID and extra size
    #[inline]
    fn make_header(class_id: ClassId, extra_size: usize) -> usize {
        let tag = MemoryTag::Object as usize;
        // gc_mark (1) | mtag (3) | class_id (8) | extra_size (remaining)
        (tag << 1) | ((class_id as usize) << Self::HEADER_BITS) | (extra_size << (Self::HEADER_BITS + 8))
    }

    /// Get class ID
    #[inline]
    pub fn class_id(&self) -> ClassId {
        let id = ((self.header_bits >> Self::HEADER_BITS) & 0xFF) as u8;
        // Safety: We only store valid ClassId values
        if id >= ClassId::User as u8 {
            ClassId::User
        } else {
            unsafe { std::mem::transmute::<u8, ClassId>(id) }
        }
    }

    /// Get extra size (in words)
    #[inline]
    pub fn extra_size(&self) -> usize {
        self.header_bits >> (Self::HEADER_BITS + 8)
    }

    /// Check if this is an array
    #[inline]
    pub fn is_array(&self) -> bool {
        self.class_id() == ClassId::Array
    }

    /// Check if this is a function
    #[inline]
    pub fn is_function(&self) -> bool {
        self.class_id().is_function()
    }

    /// Check if this is an error
    #[inline]
    pub fn is_error(&self) -> bool {
        self.class_id().is_error()
    }

    /// Get GC mark bit
    #[inline]
    pub fn is_marked(&self) -> bool {
        (self.header_bits & 1) != 0
    }

    /// Set GC mark bit
    #[inline]
    pub fn set_marked(&mut self, marked: bool) {
        if marked {
            self.header_bits |= 1;
        } else {
            self.header_bits &= !1;
        }
    }
}

/// Closure data for JavaScript functions
#[repr(C)]
pub struct ClosureData {
    /// Reference to function bytecode
    pub func_bytecode: Value,
    // var_refs[] follows (variable length array)
}

/// C function data for native functions
#[repr(C)]
pub struct CFunctionData {
    /// Index into C function table
    pub idx: u32,
    /// Optional parameters
    pub params: Value,
}

/// Array data for JavaScript arrays
#[repr(C)]
pub struct ArrayData {
    /// Elements array (JS_NULL or pointer to JSValueArray)
    pub tab: Value,
    /// Array length (max 2^30 - 1)
    pub len: u32,
}

/// Error data
#[repr(C)]
pub struct ErrorData {
    /// Error message (string or null)
    pub message: Value,
    /// Stack trace (string or null)
    pub stack: Value,
}

/// ArrayBuffer data
#[repr(C)]
pub struct ArrayBufferData {
    /// Byte buffer
    pub byte_buffer: Value,
}

/// TypedArray data
#[repr(C)]
pub struct TypedArrayData {
    /// Underlying buffer
    pub buffer: Value,
    /// Length in elements
    pub len: u32,
    /// Offset in elements
    pub offset: u32,
}

/// RegExp data
#[repr(C)]
pub struct RegExpData {
    /// Source pattern (string)
    pub source: Value,
    /// Compiled bytecode
    pub byte_code: Value,
    /// Last match index
    pub last_index: i32,
}

/// User data for user-defined classes
#[repr(C)]
pub struct UserData {
    /// Opaque pointer for user data
    pub opaque: *mut (),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_class_id_typed_array() {
        assert!(ClassId::Uint8Array.is_typed_array());
        assert!(ClassId::Float64Array.is_typed_array());
        assert!(!ClassId::Array.is_typed_array());
        assert!(!ClassId::Object.is_typed_array());
    }

    #[test]
    fn test_class_id_error() {
        assert!(ClassId::Error.is_error());
        assert!(ClassId::TypeError.is_error());
        assert!(!ClassId::Object.is_error());
        assert!(!ClassId::Array.is_error());
    }

    #[test]
    fn test_class_id_function() {
        assert!(ClassId::CFunction.is_function());
        assert!(ClassId::Closure.is_function());
        assert!(!ClassId::Object.is_function());
    }

    #[test]
    fn test_property_type() {
        let mut prop = Property::new(Value::null(), Value::int(42));
        assert_eq!(prop.prop_type(), PropertyType::Normal);

        prop.set_prop_type(PropertyType::GetSet);
        assert_eq!(prop.prop_type(), PropertyType::GetSet);

        prop.set_hash_next(100);
        assert_eq!(prop.hash_next(), 100);
        assert_eq!(prop.prop_type(), PropertyType::GetSet);
    }

    #[test]
    fn test_object_header() {
        let header = ObjectHeader::new(MemoryTag::Object);
        assert!(!header.is_marked());
        assert_eq!(header.mtag(), MemoryTag::Object);
    }
}
