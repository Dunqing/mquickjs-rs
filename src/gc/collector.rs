//! Mark-compact garbage collector
//!
//! The GC works in two phases:
//! 1. Mark: Traverse all reachable objects starting from roots, set mark bit
//! 2. Compact: Slide all marked objects to eliminate gaps, update pointers
//!
//! This design eliminates memory fragmentation and allows for smaller object headers
//! compared to reference counting.

use super::allocator::{BlockHeader, Heap, MemoryTag};
use crate::value::{Value, WORD_SIZE};
use std::marker::PhantomData;

/// A GC-safe reference to a value
///
/// This type ensures that values are properly rooted during GC.
/// When the GC runs, it updates all GcRefs to point to the new locations.
pub struct GcRef<'ctx> {
    /// The stored value
    pub value: Value,
    /// Previous GcRef in the stack
    prev: Option<*mut GcRef<'ctx>>,
    /// Marker for lifetime
    _marker: PhantomData<&'ctx ()>,
}

impl<'ctx> GcRef<'ctx> {
    /// Create a new GC reference
    pub fn new(value: Value) -> Self {
        GcRef {
            value,
            prev: None,
            _marker: PhantomData,
        }
    }

    /// Get the value
    #[inline]
    pub fn get(&self) -> Value {
        self.value
    }

    /// Set the value
    #[inline]
    pub fn set(&mut self, value: Value) {
        self.value = value;
    }
}

/// Run garbage collection on the heap
///
/// This performs a mark-compact collection:
/// 1. Clear all mark bits
/// 2. Mark all reachable objects
/// 3. Compact the heap by sliding objects
/// 4. Update all pointers
pub fn collect(heap: &mut Heap) {
    // Phase 1: Clear marks
    clear_marks(heap);

    // Phase 2: Mark reachable objects
    // TODO: Need access to roots (context, stack, GcRef list)
    // For now, mark everything (conservative)
    mark_all(heap);

    // Phase 3: Compact
    compact(heap);
}

/// Clear all mark bits
fn clear_marks(heap: &mut Heap) {
    for (ptr, header) in heap.iter_blocks() {
        unsafe {
            let header_mut = heap.get_header(ptr);
            header_mut.set_marked(false);
        }
    }
}

/// Mark all objects as reachable (temporary: conservative collection)
fn mark_all(heap: &mut Heap) {
    for (ptr, _) in heap.iter_blocks() {
        unsafe {
            let header = heap.get_header(ptr);
            if header.tag() != MemoryTag::Free {
                header.set_marked(true);
            }
        }
    }
}

/// Mark an object and recursively mark its references
fn mark_object(heap: &mut Heap, ptr: *mut u8) {
    if heap.is_rom_ptr(ptr) {
        return;
    }

    unsafe {
        let header = heap.get_header(ptr);
        if header.is_marked() {
            return; // Already marked
        }
        header.set_marked(true);

        // Recursively mark contained values based on object type
        match header.tag() {
            MemoryTag::Object => {
                // TODO: Mark prototype, properties, and extra fields
            }
            MemoryTag::ValueArray => {
                // Mark all values in the array
                let size_words = header.size_words();
                let values = ptr as *mut Value;
                for i in 0..size_words {
                    let val = *values.add(i);
                    if val.is_ptr() {
                        if let Some(child_ptr) = val.to_ptr::<u8>() {
                            mark_object(heap, child_ptr);
                        }
                    }
                }
            }
            MemoryTag::VarRef => {
                // TODO: Mark referenced value
            }
            MemoryTag::FunctionBytecode => {
                // TODO: Mark constant pool, variables, etc.
            }
            // Leaf objects - no references to mark
            MemoryTag::Float64 | MemoryTag::String | MemoryTag::ByteArray | MemoryTag::Free => {}
        }
    }
}

/// Compact the heap by sliding marked objects
///
/// This eliminates gaps left by freed objects and updates all pointers.
fn compact(heap: &mut Heap) {
    // First pass: Calculate new addresses
    let mut write_offset = 0usize;
    let mut forwarding: Vec<(usize, usize)> = Vec::new(); // (old_offset, new_offset)

    let mut read_offset = 0usize;
    while read_offset < heap.heap_used() {
        unsafe {
            let header_ptr = heap.base().add(read_offset) as *mut BlockHeader;
            let header = &*header_ptr;
            let block_size = header.size_bytes();

            if header.is_marked() && header.tag() != MemoryTag::Free {
                if read_offset != write_offset {
                    forwarding.push((read_offset, write_offset));
                }
                write_offset += block_size;
            }

            read_offset += block_size;
        }
    }

    // If nothing to compact, we're done
    if forwarding.is_empty() {
        return;
    }

    // Second pass: Update all pointers in objects
    // TODO: Implement pointer updating using forwarding table

    // Third pass: Slide objects to new positions
    for (old_offset, new_offset) in forwarding.iter() {
        if old_offset != new_offset {
            unsafe {
                let src = heap.base().add(*old_offset);
                let dst = heap.base().add(*new_offset);
                let header = &*(src as *const BlockHeader);
                let size = header.size_bytes();
                std::ptr::copy(src, dst, size);
            }
        }
    }

    // Update heap pointer
    // heap.heap_ptr = write_offset; // Can't modify directly, need different approach
}

/// Statistics about a GC run
#[derive(Debug, Clone, Copy, Default)]
pub struct GcStats {
    /// Number of objects before collection
    pub objects_before: usize,
    /// Number of objects after collection
    pub objects_after: usize,
    /// Bytes freed
    pub bytes_freed: usize,
    /// Bytes moved during compaction
    pub bytes_moved: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gc_ref() {
        let mut r = GcRef::new(Value::int(42));
        assert_eq!(r.get().to_i32(), Some(42));

        r.set(Value::int(100));
        assert_eq!(r.get().to_i32(), Some(100));
    }

    #[test]
    fn test_collect_empty_heap() {
        let mut heap = Heap::new(4096);
        collect(&mut heap);
        // Should not crash on empty heap
    }

    #[test]
    fn test_collect_with_objects() {
        let mut heap = Heap::new(4096);

        // Allocate some objects
        heap.alloc(32, MemoryTag::Object);
        heap.alloc(64, MemoryTag::String);
        heap.alloc(16, MemoryTag::Float64);

        let used_before = heap.heap_used();

        collect(&mut heap);

        // Since we mark everything, nothing should be freed
        assert_eq!(heap.heap_used(), used_before);
    }
}
