//! Utility functions
//!
//! Various helper functions used throughout the engine.

pub mod dtoa;
pub mod unicode;

// Re-export commonly used utilities

/// Maximum bytes for a UTF-8 character
pub const UTF8_CHAR_LEN_MAX: usize = 4;

/// Encode a Unicode code point to UTF-8
///
/// Returns the number of bytes written (1-4).
/// The buffer must have at least UTF8_CHAR_LEN_MAX bytes available.
#[inline]
pub fn unicode_to_utf8(buf: &mut [u8], c: u32) -> usize {
    if c < 0x80 {
        buf[0] = c as u8;
        1
    } else if c < 0x800 {
        buf[0] = (0xC0 | (c >> 6)) as u8;
        buf[1] = (0x80 | (c & 0x3F)) as u8;
        2
    } else if c < 0x10000 {
        buf[0] = (0xE0 | (c >> 12)) as u8;
        buf[1] = (0x80 | ((c >> 6) & 0x3F)) as u8;
        buf[2] = (0x80 | (c & 0x3F)) as u8;
        3
    } else {
        buf[0] = (0xF0 | (c >> 18)) as u8;
        buf[1] = (0x80 | ((c >> 12) & 0x3F)) as u8;
        buf[2] = (0x80 | ((c >> 6) & 0x3F)) as u8;
        buf[3] = (0x80 | (c & 0x3F)) as u8;
        4
    }
}

/// Decode a UTF-8 character from bytes
///
/// Returns (code point, bytes consumed) or None if invalid.
pub fn unicode_from_utf8(buf: &[u8]) -> Option<(u32, usize)> {
    if buf.is_empty() {
        return None;
    }

    let b0 = buf[0];
    if b0 < 0x80 {
        return Some((b0 as u32, 1));
    }

    if !(0xC0..0xF8).contains(&b0) {
        return None; // Invalid start byte
    }

    let (len, min_cp) = if b0 < 0xE0 {
        (2, 0x80)
    } else if b0 < 0xF0 {
        (3, 0x800)
    } else {
        (4, 0x10000)
    };

    if buf.len() < len {
        return None;
    }

    // Check continuation bytes
    for byte in buf.iter().take(len).skip(1) {
        if byte & 0xC0 != 0x80 {
            return None;
        }
    }

    let cp = match len {
        2 => ((b0 & 0x1F) as u32) << 6 | (buf[1] & 0x3F) as u32,
        3 => ((b0 & 0x0F) as u32) << 12 | ((buf[1] & 0x3F) as u32) << 6 | (buf[2] & 0x3F) as u32,
        4 => {
            ((b0 & 0x07) as u32) << 18
                | ((buf[1] & 0x3F) as u32) << 12
                | ((buf[2] & 0x3F) as u32) << 6
                | (buf[3] & 0x3F) as u32
        }
        _ => unreachable!(),
    };

    // Check for overlong encoding
    if cp < min_cp {
        return None;
    }

    // Check for invalid code points
    if cp > 0x10FFFF {
        return None;
    }

    Some((cp, len))
}

/// Minimum of two values
#[inline]
pub const fn min_usize(a: usize, b: usize) -> usize {
    if a < b { a } else { b }
}

/// Maximum of two values
#[inline]
pub const fn max_usize(a: usize, b: usize) -> usize {
    if a > b { a } else { b }
}

/// Count leading zeros (32-bit)
#[inline]
pub const fn clz32(x: u32) -> u32 {
    x.leading_zeros()
}

/// Count leading zeros (64-bit)
#[inline]
pub const fn clz64(x: u64) -> u32 {
    x.leading_zeros()
}

/// Count trailing zeros (32-bit)
#[inline]
pub const fn ctz32(x: u32) -> u32 {
    x.trailing_zeros()
}

/// Count trailing zeros (64-bit)
#[inline]
pub const fn ctz64(x: u64) -> u32 {
    x.trailing_zeros()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unicode_to_utf8() {
        let mut buf = [0u8; 4];

        assert_eq!(unicode_to_utf8(&mut buf, 0x41), 1);
        assert_eq!(buf[0], b'A');

        assert_eq!(unicode_to_utf8(&mut buf, 0x00E9), 2); // é
        assert_eq!(&buf[..2], &[0xC3, 0xA9]);

        assert_eq!(unicode_to_utf8(&mut buf, 0x4E2D), 3); // 中
        assert_eq!(&buf[..3], &[0xE4, 0xB8, 0xAD]);

        assert_eq!(unicode_to_utf8(&mut buf, 0x1F600), 4); // 😀
        assert_eq!(&buf[..4], &[0xF0, 0x9F, 0x98, 0x80]);
    }

    #[test]
    fn test_unicode_from_utf8() {
        assert_eq!(unicode_from_utf8(b"A"), Some((0x41, 1)));
        assert_eq!(unicode_from_utf8(&[0xC3, 0xA9]), Some((0x00E9, 2)));
        assert_eq!(unicode_from_utf8(&[0xE4, 0xB8, 0xAD]), Some((0x4E2D, 3)));
        assert_eq!(
            unicode_from_utf8(&[0xF0, 0x9F, 0x98, 0x80]),
            Some((0x1F600, 4))
        );

        // Invalid sequences
        assert_eq!(unicode_from_utf8(&[0x80]), None); // Invalid start
        assert_eq!(unicode_from_utf8(&[0xC3]), None); // Truncated
    }
}
