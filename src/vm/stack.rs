//! Value stack for the VM
//!
//! The stack grows downward in memory (toward lower addresses).

use crate::value::Value;

/// Value stack for bytecode execution
pub struct Stack {
    /// Stack storage
    values: Vec<Value>,
    /// Current frame pointer (index into values)
    frame_ptr: usize,
}

impl Stack {
    /// Create a new stack with the given capacity
    pub fn new(capacity: usize) -> Self {
        Stack {
            values: Vec::with_capacity(capacity),
            frame_ptr: 0,
        }
    }

    /// Push a value onto the stack
    #[inline]
    pub fn push(&mut self, value: Value) {
        self.values.push(value);
    }

    /// Pop a value from the stack
    #[inline]
    pub fn pop(&mut self) -> Option<Value> {
        self.values.pop()
    }

    /// Peek at the top value without removing it
    #[inline]
    pub fn peek(&self) -> Option<Value> {
        self.values.last().copied()
    }

    /// Peek at a value at offset from top (0 = top)
    #[inline]
    pub fn peek_at(&self, offset: usize) -> Option<Value> {
        let len = self.values.len();
        if offset < len {
            Some(self.values[len - 1 - offset])
        } else {
            None
        }
    }

    /// Get the current stack depth
    #[inline]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Check if the stack is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Drop n values from the stack
    pub fn drop_n(&mut self, n: usize) {
        let new_len = self.values.len().saturating_sub(n);
        self.values.truncate(new_len);
    }

    /// Duplicate the top value
    pub fn dup(&mut self) -> Option<()> {
        let val = self.peek()?;
        self.push(val);
        Some(())
    }

    /// Swap the top two values
    pub fn swap(&mut self) -> Option<()> {
        let len = self.values.len();
        if len < 2 {
            return None;
        }
        self.values.swap(len - 1, len - 2);
        Some(())
    }

    /// Get value at index relative to frame pointer
    #[inline]
    pub fn get_local(&self, index: usize) -> Option<Value> {
        let abs_index = self.frame_ptr + index;
        self.values.get(abs_index).copied()
    }

    /// Set value at index relative to frame pointer
    #[inline]
    pub fn set_local(&mut self, index: usize, value: Value) -> Option<()> {
        let abs_index = self.frame_ptr + index;
        if abs_index < self.values.len() {
            self.values[abs_index] = value;
            Some(())
        } else {
            None
        }
    }

    /// Push a new frame
    pub fn push_frame(&mut self, locals: usize) {
        let new_frame_ptr = self.values.len();

        // Initialize locals to undefined
        for _ in 0..locals {
            self.values.push(Value::undefined());
        }

        self.frame_ptr = new_frame_ptr;
    }

    /// Pop a frame, returning to the previous frame pointer
    pub fn pop_frame(&mut self, prev_frame_ptr: usize, locals: usize) {
        self.drop_n(locals);
        self.frame_ptr = prev_frame_ptr;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_pop() {
        let mut stack = Stack::new(16);

        stack.push(Value::int(1));
        stack.push(Value::int(2));
        stack.push(Value::int(3));

        assert_eq!(stack.len(), 3);
        assert_eq!(stack.pop().unwrap().to_i32(), Some(3));
        assert_eq!(stack.pop().unwrap().to_i32(), Some(2));
        assert_eq!(stack.pop().unwrap().to_i32(), Some(1));
        assert!(stack.is_empty());
    }

    #[test]
    fn test_peek() {
        let mut stack = Stack::new(16);

        stack.push(Value::int(1));
        stack.push(Value::int(2));

        assert_eq!(stack.peek().unwrap().to_i32(), Some(2));
        assert_eq!(stack.peek_at(0).unwrap().to_i32(), Some(2));
        assert_eq!(stack.peek_at(1).unwrap().to_i32(), Some(1));
        assert!(stack.peek_at(2).is_none());
    }

    #[test]
    fn test_dup() {
        let mut stack = Stack::new(16);

        stack.push(Value::int(42));
        stack.dup();

        assert_eq!(stack.len(), 2);
        assert_eq!(stack.pop().unwrap().to_i32(), Some(42));
        assert_eq!(stack.pop().unwrap().to_i32(), Some(42));
    }

    #[test]
    fn test_swap() {
        let mut stack = Stack::new(16);

        stack.push(Value::int(1));
        stack.push(Value::int(2));
        stack.swap();

        assert_eq!(stack.pop().unwrap().to_i32(), Some(1));
        assert_eq!(stack.pop().unwrap().to_i32(), Some(2));
    }

    #[test]
    fn test_locals() {
        let mut stack = Stack::new(16);

        stack.push_frame(3);
        assert_eq!(stack.len(), 3);

        stack.set_local(0, Value::int(10));
        stack.set_local(1, Value::int(20));
        stack.set_local(2, Value::int(30));

        assert_eq!(stack.get_local(0).unwrap().to_i32(), Some(10));
        assert_eq!(stack.get_local(1).unwrap().to_i32(), Some(20));
        assert_eq!(stack.get_local(2).unwrap().to_i32(), Some(30));
    }
}
