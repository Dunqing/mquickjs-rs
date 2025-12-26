//! JavaScript Array implementation
//!
//! Arrays in MQuickJS use a "no-hole" semantics where all elements
//! from index 0 to length-1 are defined (no sparse arrays).

use crate::value::Value;

/// Maximum array length (2^30 - 1)
pub const MAX_ARRAY_LENGTH: u32 = (1 << 30) - 1;

/// JavaScript Array
///
/// This is a dense array implementation where all indices from 0 to length-1
/// are defined. This is simpler and more efficient than sparse arrays.
#[derive(Debug)]
pub struct JSArray {
    /// Element storage
    elements: Vec<Value>,
    /// Logical length (can be different from elements.len() when shrinking)
    len: u32,
}

impl JSArray {
    /// Create a new empty array
    pub fn new() -> Self {
        JSArray {
            elements: Vec::new(),
            len: 0,
        }
    }

    /// Create an array with initial capacity
    pub fn with_capacity(capacity: usize) -> Self {
        JSArray {
            elements: Vec::with_capacity(capacity),
            len: 0,
        }
    }

    /// Create an array with specified length, filled with undefined
    pub fn with_length(length: u32) -> Self {
        let len = length.min(MAX_ARRAY_LENGTH);
        let mut elements = Vec::with_capacity(len as usize);
        elements.resize(len as usize, Value::undefined());
        JSArray { elements, len }
    }

    /// Create an array from a vector of values
    pub fn from_values(values: Vec<Value>) -> Self {
        let len = values.len().min(MAX_ARRAY_LENGTH as usize) as u32;
        JSArray {
            elements: values,
            len,
        }
    }

    /// Get the array length
    #[inline]
    pub fn len(&self) -> u32 {
        self.len
    }

    /// Check if the array is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get an element at the specified index
    #[inline]
    pub fn get(&self, index: u32) -> Option<Value> {
        if index < self.len {
            // SAFETY: We just checked that index < self.len, and self.len <= self.elements.len()
            Some(unsafe { *self.elements.get_unchecked(index as usize) })
        } else {
            None
        }
    }

    /// Get an element without bounds checking
    ///
    /// # Safety
    /// Caller must ensure index < self.len()
    #[inline]
    pub unsafe fn get_unchecked(&self, index: u32) -> Value {
        // SAFETY: Caller guarantees index < self.len()
        unsafe { *self.elements.get_unchecked(index as usize) }
    }

    /// Set an element at the specified index
    ///
    /// If index >= length, the array is extended with undefined values.
    pub fn set(&mut self, index: u32, value: Value) -> bool {
        if index >= MAX_ARRAY_LENGTH {
            return false;
        }

        // Extend if necessary
        if index >= self.len {
            let new_len = index + 1;
            if new_len as usize > self.elements.len() {
                self.elements.resize(new_len as usize, Value::undefined());
            }
            self.len = new_len;
        }

        // SAFETY: We ensured index < self.len and self.len <= self.elements.len()
        unsafe { *self.elements.get_unchecked_mut(index as usize) = value };
        true
    }

    /// Set an element without bounds checking
    ///
    /// # Safety
    /// Caller must ensure index < self.len()
    #[inline]
    pub unsafe fn set_unchecked(&mut self, index: u32, value: Value) {
        // SAFETY: Caller guarantees index < self.len()
        unsafe { *self.elements.get_unchecked_mut(index as usize) = value };
    }

    /// Push a value onto the end of the array
    #[inline]
    pub fn push(&mut self, value: Value) -> bool {
        if self.len >= MAX_ARRAY_LENGTH {
            return false;
        }

        if self.len as usize >= self.elements.len() {
            self.elements.push(value);
        } else {
            // SAFETY: We checked self.len < self.elements.len()
            unsafe { *self.elements.get_unchecked_mut(self.len as usize) = value };
        }
        self.len += 1;
        true
    }

    /// Pop a value from the end of the array
    #[inline]
    pub fn pop(&mut self) -> Option<Value> {
        if self.len == 0 {
            return None;
        }

        self.len -= 1;
        // SAFETY: self.len was > 0, so self.len (after decrement) < old len <= elements.len()
        Some(unsafe { *self.elements.get_unchecked(self.len as usize) })
    }

    /// Shift a value from the beginning of the array
    pub fn shift(&mut self) -> Option<Value> {
        if self.len == 0 {
            return None;
        }

        // SAFETY: len > 0 so index 0 is valid
        let value = unsafe { *self.elements.get_unchecked(0) };

        // Shift all elements left
        let len = self.len as usize;
        for i in 1..len {
            // SAFETY: i < len and i-1 < len, both are valid indices
            unsafe {
                let v = *self.elements.get_unchecked(i);
                *self.elements.get_unchecked_mut(i - 1) = v;
            }
        }

        self.len -= 1;
        Some(value)
    }

    /// Unshift values onto the beginning of the array
    pub fn unshift(&mut self, values: &[Value]) -> bool {
        let new_len = self.len as usize + values.len();
        if new_len > MAX_ARRAY_LENGTH as usize {
            return false;
        }

        // Make room
        self.elements.resize(new_len, Value::undefined());

        // Shift existing elements right
        for i in (0..self.len as usize).rev() {
            self.elements[i + values.len()] = self.elements[i];
        }

        // Insert new values
        for (i, &value) in values.iter().enumerate() {
            self.elements[i] = value;
        }

        self.len = new_len as u32;
        true
    }

    /// Set the length of the array
    ///
    /// If length is greater than current, extends with undefined.
    /// If length is less than current, truncates.
    pub fn set_length(&mut self, length: u32) -> bool {
        let length = length.min(MAX_ARRAY_LENGTH);

        if length > self.len {
            self.elements.resize(length as usize, Value::undefined());
        }
        // Note: we don't shrink the vector, just update the logical length

        self.len = length;
        true
    }

    /// Get a slice of the array
    pub fn slice(&self, start: i32, end: i32) -> JSArray {
        let len = self.len as i32;

        // Normalize negative indices
        let start = if start < 0 {
            (len + start).max(0) as u32
        } else {
            (start as u32).min(self.len)
        };

        let end = if end < 0 {
            (len + end).max(0) as u32
        } else {
            (end as u32).min(self.len)
        };

        if start >= end {
            return JSArray::new();
        }

        let slice_len = (end - start) as usize;
        let mut result = Vec::with_capacity(slice_len);

        for i in start..end {
            result.push(self.elements[i as usize]);
        }

        JSArray::from_values(result)
    }

    /// Splice the array (remove and/or insert elements)
    ///
    /// Returns the removed elements.
    pub fn splice(&mut self, start: i32, delete_count: u32, items: &[Value]) -> JSArray {
        let len = self.len as i32;

        // Normalize start
        let start = if start < 0 {
            (len + start).max(0) as usize
        } else {
            (start as usize).min(self.len as usize)
        };

        // Calculate actual delete count
        let delete_count = delete_count.min((self.len as usize - start) as u32) as usize;

        // Save deleted elements
        let mut deleted = Vec::with_capacity(delete_count);
        for i in start..(start + delete_count) {
            deleted.push(self.elements[i]);
        }

        // Calculate new length
        let new_len = self.len as usize - delete_count + items.len();
        if new_len > MAX_ARRAY_LENGTH as usize {
            return JSArray::from_values(deleted);
        }

        // Shift elements if needed
        if items.len() > delete_count {
            // Need to make room
            let shift = items.len() - delete_count;
            self.elements.resize(new_len, Value::undefined());
            for i in ((start + delete_count)..self.len as usize).rev() {
                self.elements[i + shift] = self.elements[i];
            }
        } else if items.len() < delete_count {
            // Need to shrink
            let shift = delete_count - items.len();
            for i in (start + delete_count)..self.len as usize {
                self.elements[i - shift] = self.elements[i];
            }
        }

        // Insert new items
        for (i, &item) in items.iter().enumerate() {
            self.elements[start + i] = item;
        }

        self.len = new_len as u32;
        JSArray::from_values(deleted)
    }

    /// Reverse the array in place
    pub fn reverse(&mut self) {
        let len = self.len as usize;
        for i in 0..len / 2 {
            self.elements.swap(i, len - 1 - i);
        }
    }

    /// Concatenate with another array
    pub fn concat(&self, other: &JSArray) -> Option<JSArray> {
        let new_len = self.len as usize + other.len as usize;
        if new_len > MAX_ARRAY_LENGTH as usize {
            return None;
        }

        let mut result = Vec::with_capacity(new_len);
        result.extend_from_slice(&self.elements[..self.len as usize]);
        result.extend_from_slice(&other.elements[..other.len as usize]);

        Some(JSArray::from_values(result))
    }

    /// Get an iterator over the elements
    pub fn iter(&self) -> impl Iterator<Item = Value> + '_ {
        self.elements[..self.len as usize].iter().copied()
    }

    /// Get index of a value (using strict equality)
    pub fn index_of(&self, value: Value, from_index: u32) -> Option<u32> {
        let len = self.len;
        (from_index..len).find(|&i| unsafe { *self.elements.get_unchecked(i as usize) } == value)
    }

    /// Get last index of a value (using strict equality)
    pub fn last_index_of(&self, value: Value, from_index: u32) -> Option<u32> {
        if self.len == 0 {
            return None;
        }

        let start = from_index.min(self.len - 1);
        (0..=start)
            .rev()
            .find(|&i| self.elements[i as usize] == value)
    }

    /// Check if array includes a value
    pub fn includes(&self, value: Value, from_index: u32) -> bool {
        self.index_of(value, from_index).is_some()
    }
}

impl Default for JSArray {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for JSArray {
    fn clone(&self) -> Self {
        JSArray {
            elements: self.elements[..self.len as usize].to_vec(),
            len: self.len,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let arr = JSArray::new();
        assert!(arr.is_empty());
        assert_eq!(arr.len(), 0);
    }

    #[test]
    fn test_with_length() {
        let arr = JSArray::with_length(5);
        assert_eq!(arr.len(), 5);
        assert!(arr.get(0).unwrap().is_undefined());
    }

    #[test]
    fn test_push_pop() {
        let mut arr = JSArray::new();

        arr.push(Value::int(1));
        arr.push(Value::int(2));
        arr.push(Value::int(3));

        assert_eq!(arr.len(), 3);
        assert_eq!(arr.pop(), Some(Value::int(3)));
        assert_eq!(arr.pop(), Some(Value::int(2)));
        assert_eq!(arr.len(), 1);
    }

    #[test]
    fn test_get_set() {
        let mut arr = JSArray::new();

        arr.set(0, Value::int(10));
        arr.set(2, Value::int(30));

        assert_eq!(arr.len(), 3);
        assert_eq!(arr.get(0), Some(Value::int(10)));
        assert!(arr.get(1).unwrap().is_undefined());
        assert_eq!(arr.get(2), Some(Value::int(30)));
        assert_eq!(arr.get(3), None);
    }

    #[test]
    fn test_shift_unshift() {
        let mut arr = JSArray::from_values(vec![Value::int(1), Value::int(2), Value::int(3)]);

        assert_eq!(arr.shift(), Some(Value::int(1)));
        assert_eq!(arr.len(), 2);
        assert_eq!(arr.get(0), Some(Value::int(2)));

        arr.unshift(&[Value::int(0), Value::int(1)]);
        assert_eq!(arr.len(), 4);
        assert_eq!(arr.get(0), Some(Value::int(0)));
        assert_eq!(arr.get(1), Some(Value::int(1)));
    }

    #[test]
    fn test_slice() {
        let arr = JSArray::from_values(vec![
            Value::int(0),
            Value::int(1),
            Value::int(2),
            Value::int(3),
            Value::int(4),
        ]);

        let slice = arr.slice(1, 4);
        assert_eq!(slice.len(), 3);
        assert_eq!(slice.get(0), Some(Value::int(1)));
        assert_eq!(slice.get(2), Some(Value::int(3)));

        // Negative indices
        let slice = arr.slice(-2, -1);
        assert_eq!(slice.len(), 1);
        assert_eq!(slice.get(0), Some(Value::int(3)));
    }

    #[test]
    fn test_splice() {
        let mut arr = JSArray::from_values(vec![
            Value::int(0),
            Value::int(1),
            Value::int(2),
            Value::int(3),
        ]);

        let removed = arr.splice(1, 2, &[Value::int(10), Value::int(20), Value::int(30)]);

        assert_eq!(removed.len(), 2);
        assert_eq!(removed.get(0), Some(Value::int(1)));
        assert_eq!(removed.get(1), Some(Value::int(2)));

        assert_eq!(arr.len(), 5);
        assert_eq!(arr.get(0), Some(Value::int(0)));
        assert_eq!(arr.get(1), Some(Value::int(10)));
        assert_eq!(arr.get(2), Some(Value::int(20)));
        assert_eq!(arr.get(3), Some(Value::int(30)));
        assert_eq!(arr.get(4), Some(Value::int(3)));
    }

    #[test]
    fn test_reverse() {
        let mut arr = JSArray::from_values(vec![Value::int(1), Value::int(2), Value::int(3)]);

        arr.reverse();

        assert_eq!(arr.get(0), Some(Value::int(3)));
        assert_eq!(arr.get(1), Some(Value::int(2)));
        assert_eq!(arr.get(2), Some(Value::int(1)));
    }

    #[test]
    fn test_concat() {
        let arr1 = JSArray::from_values(vec![Value::int(1), Value::int(2)]);
        let arr2 = JSArray::from_values(vec![Value::int(3), Value::int(4)]);

        let result = arr1.concat(&arr2).unwrap();
        assert_eq!(result.len(), 4);
        assert_eq!(result.get(2), Some(Value::int(3)));
    }

    #[test]
    fn test_index_of() {
        let arr = JSArray::from_values(vec![
            Value::int(1),
            Value::int(2),
            Value::int(3),
            Value::int(2),
        ]);

        assert_eq!(arr.index_of(Value::int(2), 0), Some(1));
        assert_eq!(arr.index_of(Value::int(2), 2), Some(3));
        assert_eq!(arr.index_of(Value::int(5), 0), None);

        assert_eq!(arr.last_index_of(Value::int(2), 3), Some(3));
        assert_eq!(arr.last_index_of(Value::int(2), 2), Some(1));
    }

    #[test]
    fn test_set_length() {
        let mut arr = JSArray::from_values(vec![Value::int(1), Value::int(2), Value::int(3)]);

        arr.set_length(5);
        assert_eq!(arr.len(), 5);
        assert!(arr.get(3).unwrap().is_undefined());

        arr.set_length(1);
        assert_eq!(arr.len(), 1);
        assert_eq!(arr.get(0), Some(Value::int(1)));
    }
}
