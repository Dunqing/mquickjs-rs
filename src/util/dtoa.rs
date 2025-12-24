//! Double to ASCII conversion
//!
//! Functions for converting floating-point numbers to strings.

/// Convert a 32-bit signed integer to decimal string
///
/// Returns the number of characters written.
pub fn i32_to_str(buf: &mut [u8], mut val: i32) -> usize {
    if buf.is_empty() {
        return 0;
    }

    let mut i = 0;
    let negative = val < 0;

    if negative {
        buf[i] = b'-';
        i += 1;
        val = -val;
    }

    let start = i;
    loop {
        if i >= buf.len() {
            break;
        }
        buf[i] = b'0' + (val % 10) as u8;
        i += 1;
        val /= 10;
        if val == 0 {
            break;
        }
    }

    // Reverse the digits
    let end = i;
    let mut left = start;
    let mut right = end - 1;
    while left < right {
        buf.swap(left, right);
        left += 1;
        right -= 1;
    }

    end
}

/// Convert a 32-bit unsigned integer to decimal string
pub fn u32_to_str(buf: &mut [u8], mut val: u32) -> usize {
    if buf.is_empty() {
        return 0;
    }

    let mut i = 0;
    loop {
        if i >= buf.len() {
            break;
        }
        buf[i] = b'0' + (val % 10) as u8;
        i += 1;
        val /= 10;
        if val == 0 {
            break;
        }
    }

    // Reverse the digits
    let end = i;
    let mut left = 0;
    let mut right = end - 1;
    while left < right {
        buf.swap(left, right);
        left += 1;
        right -= 1;
    }

    end
}

/// Convert a 64-bit signed integer to decimal string
pub fn i64_to_str(buf: &mut [u8], mut val: i64) -> usize {
    if buf.is_empty() {
        return 0;
    }

    let mut i = 0;
    let negative = val < 0;

    if negative {
        buf[i] = b'-';
        i += 1;
        val = -val;
    }

    let start = i;
    loop {
        if i >= buf.len() {
            break;
        }
        buf[i] = b'0' + (val % 10) as u8;
        i += 1;
        val /= 10;
        if val == 0 {
            break;
        }
    }

    // Reverse the digits
    let end = i;
    let mut left = start;
    let mut right = end - 1;
    while left < right {
        buf.swap(left, right);
        left += 1;
        right -= 1;
    }

    end
}

/// Convert a 64-bit unsigned integer to decimal string
pub fn u64_to_str(buf: &mut [u8], mut val: u64) -> usize {
    if buf.is_empty() {
        return 0;
    }

    let mut i = 0;
    loop {
        if i >= buf.len() {
            break;
        }
        buf[i] = b'0' + (val % 10) as u8;
        i += 1;
        val /= 10;
        if val == 0 {
            break;
        }
    }

    // Reverse the digits
    let end = i;
    let mut left = 0;
    let mut right = end - 1;
    while left < right {
        buf.swap(left, right);
        left += 1;
        right -= 1;
    }

    end
}

/// Convert an unsigned integer to string with given radix (2-36)
pub fn u64_to_str_radix(buf: &mut [u8], mut val: u64, radix: u32) -> usize {
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";

    if buf.is_empty() || radix < 2 || radix > 36 {
        return 0;
    }

    let mut i = 0;
    loop {
        if i >= buf.len() {
            break;
        }
        buf[i] = DIGITS[(val % radix as u64) as usize];
        i += 1;
        val /= radix as u64;
        if val == 0 {
            break;
        }
    }

    // Reverse the digits
    let end = i;
    let mut left = 0;
    let mut right = end - 1;
    while left < right {
        buf.swap(left, right);
        left += 1;
        right -= 1;
    }

    end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_i32_to_str() {
        let mut buf = [0u8; 32];

        let n = i32_to_str(&mut buf, 0);
        assert_eq!(&buf[..n], b"0");

        let n = i32_to_str(&mut buf, 42);
        assert_eq!(&buf[..n], b"42");

        let n = i32_to_str(&mut buf, -123);
        assert_eq!(&buf[..n], b"-123");

        let n = i32_to_str(&mut buf, i32::MAX);
        assert_eq!(&buf[..n], b"2147483647");
    }

    #[test]
    fn test_u64_to_str_radix() {
        let mut buf = [0u8; 64];

        let n = u64_to_str_radix(&mut buf, 255, 16);
        assert_eq!(&buf[..n], b"ff");

        let n = u64_to_str_radix(&mut buf, 255, 2);
        assert_eq!(&buf[..n], b"11111111");

        let n = u64_to_str_radix(&mut buf, 35, 36);
        assert_eq!(&buf[..n], b"z");
    }
}
