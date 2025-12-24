//! JavaScript value representation
//!
//! JSValue is a tagged union that fits in a single machine word (32 or 64 bits).
//! This matches the original C implementation's memory-efficient design.
//!
//! # Value encoding (64-bit)
//! - Bit 0: 0 = 31-bit signed integer (shifted left by 1)
//! - Bits 0-2 = 001: Pointer to GC-managed object
//! - Bits 0-2 = 011: Special values (null, undefined, bool, exception, etc.)
//! - Bits 0-2 = 101: Short float (limited range, no allocation needed)

use std::fmt;

/// Size of a word in bytes (matches pointer size)
#[cfg(target_pointer_width = "64")]
pub const WORD_SIZE: usize = 8;
#[cfg(target_pointer_width = "32")]
pub const WORD_SIZE: usize = 4;

/// Tag values for value encoding
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tag {
    /// 31-bit signed integer (1 bit tag)
    Int = 0,
    /// Pointer to GC-managed object (2 bits tag)
    Ptr = 1,
    /// Special value marker (2 bits tag)
    Special = 3,
    /// Short float - only on 64-bit (3 bits tag)
    #[cfg(target_pointer_width = "64")]
    ShortFloat = 5,
}

/// Special value subtypes (5-bit tag)
/// These include the TAG_SPECIAL (3) base value
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialTag {
    Bool = 3,           // JS_TAG_SPECIAL | (0 << 2) = 3
    Null = 7,           // JS_TAG_SPECIAL | (1 << 2) = 7
    Undefined = 11,     // JS_TAG_SPECIAL | (2 << 2) = 11
    Exception = 15,     // JS_TAG_SPECIAL | (3 << 2) = 15
    ShortFunc = 19,     // JS_TAG_SPECIAL | (4 << 2) = 19
    Uninitialized = 23, // JS_TAG_SPECIAL | (5 << 2) = 23
    StringChar = 27,    // JS_TAG_SPECIAL | (6 << 2) = 27
    CatchOffset = 31,   // JS_TAG_SPECIAL | (7 << 2) = 31 (used for closures and arrays)
}

/// Marker bit to distinguish arrays from closures in the CatchOffset tag
/// When bit 26 is set, it's an array index
pub const ARRAY_INDEX_MARKER: i32 = 1 << 26;

/// Marker bit to distinguish objects from closures and arrays in the CatchOffset tag
/// When bit 25 is set, it's an object index
pub const OBJECT_INDEX_MARKER: i32 = 1 << 25;

/// Marker bit to distinguish iterators from closures, arrays, and objects in the CatchOffset tag
/// When bit 24 is set, it's an iterator index
pub const ITERATOR_INDEX_MARKER: i32 = 1 << 24;

/// Raw value representation - a single word
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct RawValue(pub usize);

impl RawValue {
    /// Number of bits used for special value tag
    const SPECIAL_TAG_BITS: u32 = 5;

    /// Create a new integer value (31-bit signed)
    #[inline]
    pub const fn from_i32(val: i32) -> Self {
        // Shift left by 1 to make room for tag bit 0
        RawValue(((val as i64) << 1) as usize)
    }

    /// Create a new special value
    #[inline]
    pub const fn make_special(tag: u8, val: i32) -> Self {
        RawValue((tag as usize) | ((val as usize) << Self::SPECIAL_TAG_BITS))
    }

    /// Check if this is an integer
    #[inline]
    pub const fn is_int(self) -> bool {
        (self.0 & 1) == Tag::Int as usize
    }

    /// Check if this is a pointer
    #[inline]
    pub const fn is_ptr(self) -> bool {
        (self.0 & (WORD_SIZE - 1)) == Tag::Ptr as usize
    }

    /// Check if this is a special value
    #[inline]
    pub const fn is_special(self) -> bool {
        (self.0 & 0x3) == Tag::Special as usize
    }

    /// Get integer value (assumes is_int() is true)
    #[inline]
    pub const fn get_int(self) -> i32 {
        (self.0 as i64 >> 1) as i32
    }

    /// Get special tag (assumes is_special() is true)
    #[inline]
    pub const fn get_special_tag(self) -> u8 {
        (self.0 & ((1 << Self::SPECIAL_TAG_BITS) - 1)) as u8
    }

    /// Get special value (assumes is_special() is true)
    #[inline]
    pub const fn get_special_value(self) -> i32 {
        (self.0 >> Self::SPECIAL_TAG_BITS) as i32
    }

    /// Get pointer value (assumes is_ptr() is true)
    #[inline]
    pub fn get_ptr<T>(self) -> *mut T {
        (self.0 - 1) as *mut T
    }

    /// Create from pointer
    #[inline]
    pub fn from_ptr<T>(ptr: *mut T) -> Self {
        RawValue((ptr as usize) + 1)
    }

    // Common special values
    pub const NULL: RawValue = RawValue::make_special(SpecialTag::Null as u8, 0);
    pub const UNDEFINED: RawValue = RawValue::make_special(SpecialTag::Undefined as u8, 0);
    pub const UNINITIALIZED: RawValue = RawValue::make_special(SpecialTag::Uninitialized as u8, 0);
    pub const FALSE: RawValue = RawValue::make_special(SpecialTag::Bool as u8, 0);
    pub const TRUE: RawValue = RawValue::make_special(SpecialTag::Bool as u8, 1);
    pub const EXCEPTION: RawValue = RawValue::make_special(SpecialTag::Exception as u8, 0);
}

impl fmt::Debug for RawValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_int() {
            write!(f, "Int({})", self.get_int())
        } else if *self == RawValue::NULL {
            write!(f, "Null")
        } else if *self == RawValue::UNDEFINED {
            write!(f, "Undefined")
        } else if *self == RawValue::TRUE {
            write!(f, "Bool(true)")
        } else if *self == RawValue::FALSE {
            write!(f, "Bool(false)")
        } else if *self == RawValue::EXCEPTION {
            write!(f, "Exception")
        } else if self.is_ptr() {
            write!(f, "Ptr({:?})", self.get_ptr::<()>())
        } else {
            write!(f, "RawValue(0x{:x})", self.0)
        }
    }
}

/// High-level JavaScript value type
///
/// This is the main value type used throughout the engine.
/// It wraps RawValue and provides a safe, idiomatic Rust interface.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Value(pub RawValue);

impl Value {
    // Constructors for primitive values

    /// Create a null value
    #[inline]
    pub const fn null() -> Self {
        Value(RawValue::NULL)
    }

    /// Create an undefined value
    #[inline]
    pub const fn undefined() -> Self {
        Value(RawValue::UNDEFINED)
    }

    /// Create a boolean value
    #[inline]
    pub const fn bool(b: bool) -> Self {
        if b {
            Value(RawValue::TRUE)
        } else {
            Value(RawValue::FALSE)
        }
    }

    /// Create an integer value (31-bit signed)
    #[inline]
    pub const fn int(val: i32) -> Self {
        // Check if value fits in 31 bits (will panic in const context if not)
        // Range: -2^30 to 2^30 - 1
        assert!(val >= -(1 << 30) && val <= (1 << 30) - 1, "Integer out of 31-bit range");
        Value(RawValue::from_i32(val))
    }

    /// Create an exception marker
    #[inline]
    pub const fn exception() -> Self {
        Value(RawValue::EXCEPTION)
    }

    /// Create an uninitialized marker
    #[inline]
    pub const fn uninitialized() -> Self {
        Value(RawValue::UNINITIALIZED)
    }

    /// Create a function value from a bytecode pointer
    ///
    /// # Safety
    /// The pointer must remain valid for the lifetime of this value.
    #[inline]
    pub fn func_ptr(ptr: *const crate::runtime::FunctionBytecode) -> Self {
        Value(RawValue::from_ptr(ptr as *mut ()))
    }

    /// Create a function value (index into inner_functions)
    /// This is for temporary use during compilation
    #[inline]
    pub const fn func(idx: u16) -> Self {
        Value(RawValue::make_special(SpecialTag::ShortFunc as u8, idx as i32))
    }

    /// Create a string value (index into string constants)
    #[inline]
    pub const fn string(idx: u16) -> Self {
        Value(RawValue::make_special(SpecialTag::StringChar as u8, idx as i32))
    }

    /// Create a closure value (index into interpreter's closures array)
    #[inline]
    pub const fn closure_idx(idx: u32) -> Self {
        Value(RawValue::make_special(SpecialTag::CatchOffset as u8, idx as i32))
    }

    /// Create an array value (index into interpreter's arrays)
    /// Uses bit 26 marker to distinguish from closures
    #[inline]
    pub const fn array_idx(idx: u32) -> Self {
        Value(RawValue::make_special(
            SpecialTag::CatchOffset as u8,
            (idx as i32) | ARRAY_INDEX_MARKER,
        ))
    }

    /// Create an object value (index into interpreter's objects)
    /// Uses bit 25 marker to distinguish from closures and arrays
    #[inline]
    pub const fn object_idx(idx: u32) -> Self {
        Value(RawValue::make_special(
            SpecialTag::CatchOffset as u8,
            (idx as i32) | OBJECT_INDEX_MARKER,
        ))
    }

    /// Create an iterator value (index into interpreter's for_in_iterators)
    /// Uses bit 24 marker to distinguish from closures, arrays, and objects
    #[inline]
    pub const fn iterator_idx(idx: u32) -> Self {
        Value(RawValue::make_special(
            SpecialTag::CatchOffset as u8,
            (idx as i32) | ITERATOR_INDEX_MARKER,
        ))
    }

    // Type checking

    /// Check if this is null
    #[inline]
    pub const fn is_null(self) -> bool {
        self.0.0 == RawValue::NULL.0
    }

    /// Check if this is undefined
    #[inline]
    pub const fn is_undefined(self) -> bool {
        self.0.0 == RawValue::UNDEFINED.0
    }

    /// Check if this is a boolean
    #[inline]
    pub const fn is_bool(self) -> bool {
        self.0.get_special_tag() == SpecialTag::Bool as u8
    }

    /// Check if this is an integer
    #[inline]
    pub const fn is_int(self) -> bool {
        self.0.is_int()
    }

    /// Check if this is a pointer to a GC object
    #[inline]
    pub const fn is_ptr(self) -> bool {
        self.0.is_ptr()
    }

    /// Check if this is an exception
    #[inline]
    pub const fn is_exception(self) -> bool {
        self.0.0 == RawValue::EXCEPTION.0
    }

    /// Check if this is uninitialized
    #[inline]
    pub const fn is_uninitialized(self) -> bool {
        self.0.0 == RawValue::UNINITIALIZED.0
    }

    /// Check if this is nullish (null or undefined)
    #[inline]
    pub const fn is_nullish(self) -> bool {
        self.is_null() || self.is_undefined()
    }

    /// Check if this is a function
    #[inline]
    pub const fn is_func(self) -> bool {
        self.0.get_special_tag() == SpecialTag::ShortFunc as u8
    }

    /// Check if this is a string
    #[inline]
    pub const fn is_string(self) -> bool {
        self.0.get_special_tag() == SpecialTag::StringChar as u8
    }

    /// Check if this is a closure
    /// Closures use CatchOffset tag without array or object marker bits set
    #[inline]
    pub const fn is_closure(self) -> bool {
        self.0.get_special_tag() == SpecialTag::CatchOffset as u8
            && (self.0.get_special_value()
                & (ARRAY_INDEX_MARKER | OBJECT_INDEX_MARKER | ITERATOR_INDEX_MARKER))
                == 0
    }

    /// Check if this is an array
    /// Arrays use CatchOffset tag with bit 26 set
    #[inline]
    pub const fn is_array(self) -> bool {
        self.0.get_special_tag() == SpecialTag::CatchOffset as u8
            && (self.0.get_special_value() & ARRAY_INDEX_MARKER) != 0
    }

    /// Check if this is an object
    /// Objects use CatchOffset tag with bit 25 set
    #[inline]
    pub const fn is_object(self) -> bool {
        self.0.get_special_tag() == SpecialTag::CatchOffset as u8
            && (self.0.get_special_value() & OBJECT_INDEX_MARKER) != 0
    }

    /// Check if this is an iterator
    /// Iterators use CatchOffset tag with bit 24 set
    #[inline]
    pub const fn is_iterator(self) -> bool {
        self.0.get_special_tag() == SpecialTag::CatchOffset as u8
            && (self.0.get_special_value() & ITERATOR_INDEX_MARKER) != 0
    }

    // Value extraction

    /// Get boolean value, returns None if not a boolean
    #[inline]
    pub const fn to_bool(self) -> Option<bool> {
        if self.is_bool() {
            Some(self.0.get_special_value() != 0)
        } else {
            None
        }
    }

    /// Get integer value, returns None if not an integer
    #[inline]
    pub const fn to_i32(self) -> Option<i32> {
        if self.is_int() {
            Some(self.0.get_int())
        } else {
            None
        }
    }

    /// Get raw pointer, returns None if not a pointer
    #[inline]
    pub fn to_ptr<T>(self) -> Option<*mut T> {
        if self.is_ptr() {
            Some(self.0.get_ptr())
        } else {
            None
        }
    }

    /// Get function index, returns None if not a function (ShortFunc type)
    #[inline]
    pub const fn to_func_idx(self) -> Option<u16> {
        if self.is_func() {
            Some(self.0.get_special_value() as u16)
        } else {
            None
        }
    }

    /// Get string constant index, returns None if not a string
    #[inline]
    pub const fn to_string_idx(self) -> Option<u16> {
        if self.is_string() {
            Some(self.0.get_special_value() as u16)
        } else {
            None
        }
    }

    /// Get closure index, returns None if not a closure
    #[inline]
    pub const fn to_closure_idx(self) -> Option<u32> {
        if self.is_closure() {
            Some(self.0.get_special_value() as u32)
        } else {
            None
        }
    }

    /// Get array index, returns None if not an array
    #[inline]
    pub const fn to_array_idx(self) -> Option<u32> {
        if self.is_array() {
            // Mask off the array marker bit to get the actual index
            Some((self.0.get_special_value() & !ARRAY_INDEX_MARKER) as u32)
        } else {
            None
        }
    }

    /// Get object index, returns None if not an object
    #[inline]
    pub const fn to_object_idx(self) -> Option<u32> {
        if self.is_object() {
            // Mask off the object marker bit to get the actual index
            Some((self.0.get_special_value() & !OBJECT_INDEX_MARKER) as u32)
        } else {
            None
        }
    }

    /// Get iterator index, returns None if not an iterator
    #[inline]
    pub const fn to_iterator_idx(self) -> Option<u32> {
        if self.is_iterator() {
            // Mask off the iterator marker bit to get the actual index
            Some((self.0.get_special_value() & !ITERATOR_INDEX_MARKER) as u32)
        } else {
            None
        }
    }

    /// Get function bytecode pointer, returns None if not a pointer-based function
    #[inline]
    pub fn to_func_ptr(self) -> Option<*const crate::runtime::FunctionBytecode> {
        if self.is_ptr() {
            Some(self.0.get_ptr::<crate::runtime::FunctionBytecode>() as *const _)
        } else {
            None
        }
    }

    /// Get raw value
    #[inline]
    pub const fn raw(self) -> RawValue {
        self.0
    }
}

impl Default for Value {
    fn default() -> Self {
        Value::undefined()
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_null() {
            write!(f, "null")
        } else if self.is_undefined() {
            write!(f, "undefined")
        } else if let Some(b) = self.to_bool() {
            write!(f, "{}", b)
        } else if let Some(i) = self.to_i32() {
            write!(f, "{}", i)
        } else if self.is_exception() {
            write!(f, "[exception]")
        } else if self.is_array() {
            write!(f, "[array]")
        } else {
            write!(f, "[object]")
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for Value {}

/// Short integer range constants
pub const SHORT_INT_MIN: i32 = -(1 << 30);
pub const SHORT_INT_MAX: i32 = (1 << 30) - 1;

/// Check if an i32 fits in a short integer
#[inline]
pub const fn fits_in_short_int(val: i32) -> bool {
    val >= SHORT_INT_MIN && val <= SHORT_INT_MAX
}

// Built-in string indices (reserved range 0xFFF0-0xFFFF)
// These are used for typeof return values and other built-in strings

/// String index for "undefined"
pub const STR_UNDEFINED: u16 = 0xFFF0;
/// String index for "object"
pub const STR_OBJECT: u16 = 0xFFF1;
/// String index for "boolean"
pub const STR_BOOLEAN: u16 = 0xFFF2;
/// String index for "number"
pub const STR_NUMBER: u16 = 0xFFF3;
/// String index for "function"
pub const STR_FUNCTION: u16 = 0xFFF4;
/// String index for "string"
pub const STR_STRING: u16 = 0xFFF5;
/// String index for empty string ""
pub const STR_EMPTY: u16 = 0xFFFF;

/// Get the built-in string content for a reserved string index
pub fn get_builtin_string(idx: u16) -> Option<&'static str> {
    match idx {
        STR_UNDEFINED => Some("undefined"),
        STR_OBJECT => Some("object"),
        STR_BOOLEAN => Some("boolean"),
        STR_NUMBER => Some("number"),
        STR_FUNCTION => Some("function"),
        STR_STRING => Some("string"),
        STR_EMPTY => Some(""),
        _ => None,
    }
}

/// Check if a string index is a built-in string
#[inline]
pub const fn is_builtin_string(idx: u16) -> bool {
    idx >= 0xFFF0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_null() {
        let v = Value::null();
        assert!(v.is_null());
        assert!(!v.is_undefined());
        assert!(!v.is_bool());
        assert!(!v.is_int());
        assert!(v.is_nullish());
    }

    #[test]
    fn test_undefined() {
        let v = Value::undefined();
        assert!(!v.is_null());
        assert!(v.is_undefined());
        assert!(v.is_nullish());
    }

    #[test]
    fn test_bool() {
        let t = Value::bool(true);
        let f = Value::bool(false);

        assert!(t.is_bool());
        assert!(f.is_bool());
        assert_eq!(t.to_bool(), Some(true));
        assert_eq!(f.to_bool(), Some(false));
    }

    #[test]
    fn test_int() {
        let zero = Value::int(0);
        let pos = Value::int(42);
        let neg = Value::int(-100);
        let max = Value::int(SHORT_INT_MAX);
        let min = Value::int(SHORT_INT_MIN);

        assert!(zero.is_int());
        assert_eq!(zero.to_i32(), Some(0));
        assert_eq!(pos.to_i32(), Some(42));
        assert_eq!(neg.to_i32(), Some(-100));
        assert_eq!(max.to_i32(), Some(SHORT_INT_MAX));
        assert_eq!(min.to_i32(), Some(SHORT_INT_MIN));
    }

    #[test]
    fn test_exception() {
        let v = Value::exception();
        assert!(v.is_exception());
        assert!(!v.is_null());
        assert!(!v.is_int());
    }

    #[test]
    fn test_raw_value_debug() {
        assert_eq!(format!("{:?}", RawValue::NULL), "Null");
        assert_eq!(format!("{:?}", RawValue::UNDEFINED), "Undefined");
        assert_eq!(format!("{:?}", RawValue::TRUE), "Bool(true)");
        assert_eq!(format!("{:?}", RawValue::from_i32(42)), "Int(42)");
    }
}
