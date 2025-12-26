//! Unicode utilities
//!
//! String handling with UTF-8 and UTF-16 support.

/// Check if a code point is a line terminator
#[inline]
pub fn is_line_terminator(c: u32) -> bool {
    matches!(c, 0x000A | 0x000D | 0x2028 | 0x2029)
}

/// Check if a code point is whitespace
#[inline]
pub fn is_whitespace(c: u32) -> bool {
    matches!(
        c,
        0x0009  // Tab
        | 0x000B // Vertical Tab
        | 0x000C // Form Feed
        | 0x0020 // Space
        | 0x00A0 // No-Break Space
        | 0xFEFF // BOM
    ) || is_line_terminator(c)
        || is_unicode_space(c)
}

/// Check if a code point is a Unicode space character
#[inline]
pub fn is_unicode_space(c: u32) -> bool {
    matches!(c, 0x1680 | 0x2000..=0x200A | 0x202F | 0x205F | 0x3000)
}

/// Check if a code point can start an identifier
#[inline]
pub fn is_id_start(c: u32) -> bool {
    matches!(c, 0x61..=0x7A | 0x41..=0x5A | 0x5F | 0x24) // a-z, A-Z, _, $
        || (c >= 0x80 && is_unicode_id_start(c))
}

/// Check if a code point can continue an identifier
#[inline]
pub fn is_id_continue(c: u32) -> bool {
    matches!(c, 0x61..=0x7A | 0x41..=0x5A | 0x30..=0x39 | 0x5F | 0x24) // a-z, A-Z, 0-9, _, $
        || (c >= 0x80 && is_unicode_id_continue(c))
}

/// Check Unicode ID_Start property (simplified)
fn is_unicode_id_start(c: u32) -> bool {
    // Simplified check - in a full implementation, we'd use Unicode tables
    matches!(c, 0x00C0..=0x00D6 | 0x00D8..=0x00F6 | 0x00F8..=0x00FF)
}

/// Check Unicode ID_Continue property (simplified)
fn is_unicode_id_continue(c: u32) -> bool {
    is_unicode_id_start(c) || matches!(c, 0x0300..=0x036F)
}

/// Get UTF-16 length from UTF-8 string
pub fn utf16_len(s: &str) -> usize {
    s.chars().map(|c| c.len_utf16()).sum()
}

/// Convert UTF-8 index to UTF-16 index
pub fn utf8_to_utf16_index(s: &str, utf8_index: usize) -> usize {
    s[..utf8_index].chars().map(|c| c.len_utf16()).sum()
}

/// Convert UTF-16 index to UTF-8 index
pub fn utf16_to_utf8_index(s: &str, utf16_index: usize) -> usize {
    let mut utf16_pos = 0;
    for (utf8_pos, c) in s.char_indices() {
        if utf16_pos >= utf16_index {
            return utf8_pos;
        }
        utf16_pos += c.len_utf16();
    }
    s.len()
}

/// Check if a string needs UTF-16 encoding (contains non-BMP characters)
pub fn needs_surrogate_pairs(s: &str) -> bool {
    s.chars().any(|c| c.len_utf16() > 1)
}

/// Get a character at UTF-16 index
pub fn char_at_utf16(s: &str, utf16_index: usize) -> Option<char> {
    let mut utf16_pos = 0;
    for c in s.chars() {
        if utf16_pos == utf16_index {
            return Some(c);
        }
        utf16_pos += c.len_utf16();
        if utf16_pos > utf16_index {
            // We're in the middle of a surrogate pair
            return None;
        }
    }
    None
}

/// Get code unit at UTF-16 index (returns surrogate if needed)
pub fn code_unit_at_utf16(s: &str, utf16_index: usize) -> Option<u16> {
    let mut utf16_pos = 0;
    for c in s.chars() {
        let len = c.len_utf16();
        if utf16_pos == utf16_index {
            if len == 1 {
                return Some(c as u16);
            } else {
                // Return high surrogate
                let code = c as u32;
                return Some((0xD800 + ((code - 0x10000) >> 10)) as u16);
            }
        } else if utf16_pos + 1 == utf16_index && len == 2 {
            // Return low surrogate
            let code = c as u32;
            return Some((0xDC00 + ((code - 0x10000) & 0x3FF)) as u16);
        }
        utf16_pos += len;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_utf16_len() {
        assert_eq!(utf16_len("hello"), 5);
        assert_eq!(utf16_len("中文"), 2);
        assert_eq!(utf16_len("😀"), 2); // Emoji is 2 UTF-16 code units
    }

    #[test]
    fn test_utf16_index_conversion() {
        let s = "a中😀b";

        // "a" is at UTF-8 0, UTF-16 0
        // "中" is at UTF-8 1, UTF-16 1
        // "😀" is at UTF-8 4, UTF-16 2
        // "b" is at UTF-8 8, UTF-16 4

        assert_eq!(utf8_to_utf16_index(s, 0), 0);
        assert_eq!(utf8_to_utf16_index(s, 1), 1);
        assert_eq!(utf8_to_utf16_index(s, 4), 2);
        assert_eq!(utf8_to_utf16_index(s, 8), 4);

        assert_eq!(utf16_to_utf8_index(s, 0), 0);
        assert_eq!(utf16_to_utf8_index(s, 1), 1);
        assert_eq!(utf16_to_utf8_index(s, 2), 4);
        assert_eq!(utf16_to_utf8_index(s, 4), 8);
    }

    #[test]
    fn test_code_unit_at() {
        let s = "a😀b";

        assert_eq!(code_unit_at_utf16(s, 0), Some(b'a' as u16));
        assert_eq!(code_unit_at_utf16(s, 1), Some(0xD83D)); // High surrogate for 😀
        assert_eq!(code_unit_at_utf16(s, 2), Some(0xDE00)); // Low surrogate for 😀
        assert_eq!(code_unit_at_utf16(s, 3), Some(b'b' as u16));
    }
}
