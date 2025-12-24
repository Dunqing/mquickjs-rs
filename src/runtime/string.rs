//! JavaScript string representation
//!
//! Strings in MQuickJS are stored as UTF-8 internally.
//! This module provides the JSString type and string operations.

use crate::gc::MemoryTag;
use std::hash::{Hash, Hasher};

/// Memory tag bits for header
const MTAG_BITS: u32 = 4;

/// JavaScript string
///
/// Strings are stored with a header containing metadata flags and length,
/// followed by the UTF-8 encoded content.
#[repr(C)]
pub struct JSString {
    /// Header bits layout (from LSB):
    /// - Bit 0: GC mark
    /// - Bits 1-3: Memory tag (JS_MTAG_STRING = 3)
    /// - Bit 4: is_unique (interned)
    /// - Bit 5: is_ascii (all bytes < 128)
    /// - Bit 6: is_numeric (represents a number)
    /// - Remaining bits: length
    header_bits: usize,
    // buf[] follows (flexible array member)
}

impl JSString {
    /// Bit position for is_unique flag
    const UNIQUE_BIT: u32 = MTAG_BITS;
    /// Bit position for is_ascii flag
    const ASCII_BIT: u32 = MTAG_BITS + 1;
    /// Bit position for is_numeric flag
    const NUMERIC_BIT: u32 = MTAG_BITS + 2;
    /// Bit position where length starts
    const LEN_SHIFT: u32 = MTAG_BITS + 3;

    /// Maximum string length
    pub const MAX_LEN: usize = (1 << (usize::BITS - Self::LEN_SHIFT)) - 1;

    /// Create header bits for a new string
    #[inline]
    pub fn make_header(len: usize, is_ascii: bool, is_unique: bool) -> usize {
        let tag = MemoryTag::String as usize;
        let mut bits = (tag << 1) | (len << Self::LEN_SHIFT);
        if is_ascii {
            bits |= 1 << Self::ASCII_BIT;
        }
        if is_unique {
            bits |= 1 << Self::UNIQUE_BIT;
        }
        bits
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

    /// Check if string is interned (unique)
    #[inline]
    pub fn is_unique(&self) -> bool {
        (self.header_bits & (1 << Self::UNIQUE_BIT)) != 0
    }

    /// Set the unique (interned) flag
    #[inline]
    pub fn set_unique(&mut self, unique: bool) {
        if unique {
            self.header_bits |= 1 << Self::UNIQUE_BIT;
        } else {
            self.header_bits &= !(1 << Self::UNIQUE_BIT);
        }
    }

    /// Check if string is ASCII-only
    #[inline]
    pub fn is_ascii(&self) -> bool {
        (self.header_bits & (1 << Self::ASCII_BIT)) != 0
    }

    /// Check if string represents a numeric value
    #[inline]
    pub fn is_numeric(&self) -> bool {
        (self.header_bits & (1 << Self::NUMERIC_BIT)) != 0
    }

    /// Set the numeric flag
    #[inline]
    pub fn set_numeric(&mut self, numeric: bool) {
        if numeric {
            self.header_bits |= 1 << Self::NUMERIC_BIT;
        } else {
            self.header_bits &= !(1 << Self::NUMERIC_BIT);
        }
    }

    /// Get the byte length of the string
    #[inline]
    pub fn len(&self) -> usize {
        self.header_bits >> Self::LEN_SHIFT
    }

    /// Check if string is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get the string content as bytes
    ///
    /// # Safety
    /// Caller must ensure the string buffer has been properly initialized.
    #[inline]
    pub unsafe fn as_bytes(&self) -> &[u8] {
        unsafe {
            let ptr = (self as *const Self).add(1) as *const u8;
            std::slice::from_raw_parts(ptr, self.len())
        }
    }

    /// Get the string content as a mutable byte slice
    ///
    /// # Safety
    /// Caller must ensure the string buffer has been properly initialized.
    #[inline]
    pub unsafe fn as_bytes_mut(&mut self) -> &mut [u8] {
        unsafe {
            let ptr = (self as *mut Self).add(1) as *mut u8;
            std::slice::from_raw_parts_mut(ptr, self.len())
        }
    }

    /// Get the string content as a str
    ///
    /// # Safety
    /// Caller must ensure the string buffer contains valid UTF-8.
    #[inline]
    pub unsafe fn as_str(&self) -> &str {
        unsafe { std::str::from_utf8_unchecked(self.as_bytes()) }
    }

    /// Get pointer to string buffer
    #[inline]
    pub fn buf_ptr(&self) -> *const u8 {
        unsafe { (self as *const Self).add(1) as *const u8 }
    }

    /// Get mutable pointer to string buffer
    #[inline]
    pub fn buf_ptr_mut(&mut self) -> *mut u8 {
        unsafe { (self as *mut Self).add(1) as *mut u8 }
    }

    /// Calculate the total allocation size needed for a string
    #[inline]
    pub fn alloc_size(len: usize) -> usize {
        std::mem::size_of::<Self>() + len + 1 // +1 for null terminator
    }

    /// Compare two strings for equality
    ///
    /// # Safety
    /// Both strings must be properly initialized.
    pub unsafe fn eq(&self, other: &JSString) -> bool {
        if self.len() != other.len() {
            return false;
        }
        unsafe { self.as_bytes() == other.as_bytes() }
    }

    /// Hash the string content
    ///
    /// # Safety
    /// String must be properly initialized.
    pub unsafe fn hash_content(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        unsafe { self.as_bytes().hash(&mut hasher) };
        hasher.finish()
    }
}

/// Check if a string slice is ASCII-only
#[inline]
pub fn is_ascii_string(s: &str) -> bool {
    s.bytes().all(|b| b < 128)
}

/// Check if a string represents a valid array index
#[inline]
pub fn is_array_index(s: &str) -> Option<u32> {
    if s.is_empty() {
        return None;
    }

    // Fast path for single digits
    if s.len() == 1 {
        let b = s.as_bytes()[0];
        if b.is_ascii_digit() {
            return Some((b - b'0') as u32);
        }
        return None;
    }

    // Leading zeros are not valid (except "0")
    if s.starts_with('0') {
        return None;
    }

    // Parse as u32
    s.parse::<u32>().ok().filter(|&n| n < (1 << 30))
}

/// Check if a byte is a valid identifier start character
#[inline]
pub fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_' || c == b'$'
}

/// Check if a byte is a valid identifier continuation character
#[inline]
pub fn is_ident_continue(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$'
}

/// String interning table
///
/// Maintains a set of unique strings for fast comparison.
pub struct StringTable {
    /// Hash table of interned strings (indices into strings vec)
    hash_table: Vec<u32>,
    /// Mask for hash table indexing
    hash_mask: u32,
    /// Count of interned strings
    count: usize,
}

impl StringTable {
    /// Initial hash table size (power of 2)
    const INITIAL_SIZE: usize = 256;

    /// Create a new string table
    pub fn new() -> Self {
        StringTable {
            hash_table: vec![0; Self::INITIAL_SIZE],
            hash_mask: (Self::INITIAL_SIZE - 1) as u32,
            count: 0,
        }
    }

    /// Get the number of interned strings
    #[inline]
    pub fn count(&self) -> usize {
        self.count
    }

    /// Hash a string for table lookup
    #[inline]
    pub fn hash_string(s: &str) -> u32 {
        let mut h: u32 = 0;
        for b in s.bytes() {
            h = h.wrapping_mul(31).wrapping_add(b as u32);
        }
        h
    }
}

impl Default for StringTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_ascii_string() {
        assert!(is_ascii_string("hello"));
        assert!(is_ascii_string(""));
        assert!(is_ascii_string("123"));
        assert!(!is_ascii_string("héllo"));
        assert!(!is_ascii_string("中文"));
    }

    #[test]
    fn test_is_array_index() {
        assert_eq!(is_array_index("0"), Some(0));
        assert_eq!(is_array_index("1"), Some(1));
        assert_eq!(is_array_index("42"), Some(42));
        assert_eq!(is_array_index("12345"), Some(12345));
        assert_eq!(is_array_index(""), None);
        assert_eq!(is_array_index("01"), None); // Leading zero
        assert_eq!(is_array_index("-1"), None); // Negative
        assert_eq!(is_array_index("abc"), None);
        assert_eq!(is_array_index("1.5"), None);
    }

    #[test]
    fn test_is_ident() {
        assert!(is_ident_start(b'a'));
        assert!(is_ident_start(b'Z'));
        assert!(is_ident_start(b'_'));
        assert!(is_ident_start(b'$'));
        assert!(!is_ident_start(b'0'));
        assert!(!is_ident_start(b'-'));

        assert!(is_ident_continue(b'a'));
        assert!(is_ident_continue(b'0'));
        assert!(is_ident_continue(b'_'));
        assert!(!is_ident_continue(b'-'));
    }

    #[test]
    fn test_string_hash() {
        let h1 = StringTable::hash_string("hello");
        let h2 = StringTable::hash_string("hello");
        let h3 = StringTable::hash_string("world");

        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_string_table_creation() {
        let table = StringTable::new();
        assert_eq!(table.count(), 0);
    }

    #[test]
    fn test_jsstring_header() {
        let header = JSString::make_header(10, true, false);
        // Check length is encoded correctly
        assert_eq!(header >> JSString::LEN_SHIFT, 10);
        // Check ASCII flag
        assert!((header & (1 << JSString::ASCII_BIT)) != 0);
        // Check unique flag is not set
        assert!((header & (1 << JSString::UNIQUE_BIT)) == 0);
    }

    #[test]
    fn test_jsstring_max_len() {
        // Just verify the constant is reasonable
        assert!(JSString::MAX_LEN > 1_000_000);
    }
}
