//! Arena allocator for the JavaScript heap
//!
//! Memory layout:
//! ```text
//! +------------------+
//! |    JSContext     |  (not in this buffer)
//! +------------------+
//! |   Heap (grows ↓) |  <- heap_base
//! |       ...        |
//! |   [allocated]    |  <- heap_ptr
//! +------------------+
//! |   Free space     |
//! +------------------+
//! |   [stack top]    |  <- stack_ptr
//! |       ...        |
//! |  Stack (grows ↑) |  <- stack_base
//! +------------------+
//! ```
//!
//! All allocations are word-aligned and have a memory tag in the first word.

use crate::value::WORD_SIZE;

/// Memory block tags - stored in the first few bits of each block header
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryTag {
    /// Free block
    Free = 0,
    /// JavaScript object
    Object = 1,
    /// 64-bit float (boxed)
    Float64 = 2,
    /// String
    String = 3,
    /// Function bytecode
    FunctionBytecode = 4,
    /// Array of JSValues
    ValueArray = 5,
    /// Array of bytes
    ByteArray = 6,
    /// Variable reference (for closures)
    VarRef = 7,
}

impl MemoryTag {
    pub const COUNT: usize = 8;
}

/// Number of bits reserved for memory tag
const MTAG_BITS: u32 = 4;

/// Memory block header
///
/// Every allocated block starts with this header.
/// Layout (in a single word):
/// - Bit 0: GC mark bit
/// - Bits 1-3: Memory tag
/// - Remaining bits: Block-specific data (usually size)
#[repr(C)]
#[derive(Clone, Copy)]
pub struct BlockHeader {
    pub bits: usize,
}

impl BlockHeader {
    /// Create a new block header
    #[inline]
    pub const fn new(tag: MemoryTag, size_words: usize) -> Self {
        BlockHeader {
            bits: ((tag as usize) << 1) | (size_words << MTAG_BITS),
        }
    }

    /// Get the GC mark bit
    #[inline]
    pub const fn is_marked(&self) -> bool {
        (self.bits & 1) != 0
    }

    /// Set the GC mark bit
    #[inline]
    pub fn set_marked(&mut self, marked: bool) {
        if marked {
            self.bits |= 1;
        } else {
            self.bits &= !1;
        }
    }

    /// Get the memory tag
    #[inline]
    pub const fn tag(&self) -> MemoryTag {
        // SAFETY: We only store valid MemoryTag values
        unsafe { std::mem::transmute(((self.bits >> 1) & 0x7) as u8) }
    }

    /// Get the block size in words (excluding header)
    #[inline]
    pub const fn size_words(&self) -> usize {
        self.bits >> MTAG_BITS
    }

    /// Get the block size in bytes (including header)
    #[inline]
    pub const fn size_bytes(&self) -> usize {
        (self.size_words() + 1) * WORD_SIZE
    }
}

/// Free block structure
#[repr(C)]
pub struct FreeBlock {
    pub header: BlockHeader,
    // Size is stored in header
}

/// The JavaScript heap
///
/// Manages memory allocation and garbage collection.
pub struct Heap {
    /// Raw memory buffer
    buffer: Vec<u8>,

    /// Total size of the buffer
    pub total_size: usize,

    /// Current heap pointer (end of allocated heap)
    heap_ptr: usize,

    /// Current stack pointer (bottom of stack)
    stack_ptr: usize,

    /// Minimum free space to maintain
    min_free_size: usize,
}

/// Minimum free space between heap and stack
const MIN_FREE_SIZE: usize = 512;

/// Additional stack slack for safety
const STACK_SLACK: usize = 16;

impl Heap {
    /// Create a new heap with the given total size
    pub fn new(total_size: usize) -> Self {
        let mut buffer = vec![0u8; total_size];

        // Ensure alignment
        let _align_offset = buffer.as_ptr().align_offset(WORD_SIZE);

        Heap {
            buffer,
            total_size,
            heap_ptr: 0,
            stack_ptr: total_size,
            min_free_size: MIN_FREE_SIZE,
        }
    }

    /// Get base pointer of the buffer
    #[inline]
    pub fn base(&self) -> *mut u8 {
        self.buffer.as_ptr() as *mut u8
    }

    /// Get the amount of heap memory used
    #[inline]
    pub fn heap_used(&self) -> usize {
        self.heap_ptr
    }

    /// Get the amount of stack memory used
    #[inline]
    pub fn stack_used(&self) -> usize {
        self.total_size - self.stack_ptr
    }

    /// Get the amount of free space
    #[inline]
    pub fn free_space(&self) -> usize {
        if self.stack_ptr > self.heap_ptr {
            self.stack_ptr - self.heap_ptr
        } else {
            0
        }
    }

    /// Check if we have enough free memory
    #[inline]
    fn check_free(&self, size: usize) -> bool {
        self.free_space() >= size + self.min_free_size
    }

    /// Allocate a block of memory
    ///
    /// Returns None if out of memory.
    pub fn alloc(&mut self, size: usize, tag: MemoryTag) -> Option<*mut u8> {
        if size == 0 {
            return None;
        }

        // Round up to word alignment, add header size
        let aligned_size = (size + WORD_SIZE - 1) & !(WORD_SIZE - 1);
        let total_size = aligned_size + WORD_SIZE; // +1 word for header

        if !self.check_free(total_size) {
            return None;
        }

        // Allocate from heap
        let ptr = unsafe { self.base().add(self.heap_ptr) };
        self.heap_ptr += total_size;

        // Initialize header
        let header = ptr as *mut BlockHeader;
        unsafe {
            *header = BlockHeader::new(tag, aligned_size / WORD_SIZE);
        }

        // Return pointer to data (after header)
        Some(unsafe { ptr.add(WORD_SIZE) })
    }

    /// Allocate and zero-initialize a block of memory
    pub fn alloc_zeroed(&mut self, size: usize, tag: MemoryTag) -> Option<*mut u8> {
        let ptr = self.alloc(size, tag)?;

        // Zero the data portion (header is already set)
        unsafe {
            std::ptr::write_bytes(ptr, 0, size);
        }

        Some(ptr)
    }

    /// Get the header for an allocated block
    ///
    /// # Safety
    /// Caller must ensure ptr points to a valid allocated block.
    #[inline]
    pub unsafe fn get_header(&self, ptr: *mut u8) -> &mut BlockHeader {
        unsafe { &mut *(ptr.sub(WORD_SIZE) as *mut BlockHeader) }
    }

    /// Get block size in bytes from data pointer
    ///
    /// # Safety
    /// Caller must ensure ptr points to a valid allocated block.
    #[inline]
    pub unsafe fn block_size(&self, ptr: *mut u8) -> usize {
        unsafe { self.get_header(ptr).size_bytes() }
    }

    /// Check if a pointer is in ROM (not in our heap)
    #[inline]
    pub fn is_rom_ptr(&self, ptr: *const u8) -> bool {
        let base = self.base() as usize;
        let end = base + self.total_size;
        let ptr_val = ptr as usize;
        ptr_val < base || ptr_val >= end
    }

    /// Push a value onto the stack
    ///
    /// Returns None if out of stack space.
    pub fn stack_push(&mut self, words: usize) -> Option<*mut u8> {
        let size = words * WORD_SIZE;

        if self.stack_ptr < self.heap_ptr + size + self.min_free_size {
            return None;
        }

        self.stack_ptr -= size;
        Some(unsafe { self.base().add(self.stack_ptr) })
    }

    /// Pop values from the stack
    pub fn stack_pop(&mut self, words: usize) {
        let size = words * WORD_SIZE;
        self.stack_ptr = (self.stack_ptr + size).min(self.total_size);
    }

    /// Get the current stack pointer
    #[inline]
    pub fn stack_ptr(&self) -> *mut u8 {
        unsafe { self.base().add(self.stack_ptr) }
    }

    /// Iterator over all allocated blocks in the heap
    pub fn iter_blocks(&self) -> BlockIterator<'_> {
        BlockIterator {
            heap: self,
            offset: 0,
        }
    }
}

/// Iterator over allocated blocks in the heap
pub struct BlockIterator<'a> {
    heap: &'a Heap,
    offset: usize,
}

impl<'a> Iterator for BlockIterator<'a> {
    type Item = (*mut u8, &'a BlockHeader);

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.heap.heap_ptr {
            return None;
        }

        unsafe {
            let header_ptr = self.heap.base().add(self.offset) as *mut BlockHeader;
            let header = &*header_ptr;
            let data_ptr = self.heap.base().add(self.offset + WORD_SIZE);

            self.offset += header.size_bytes();

            Some((data_ptr, header))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heap_creation() {
        let heap = Heap::new(1024);
        assert_eq!(heap.total_size, 1024);
        assert_eq!(heap.heap_used(), 0);
        assert!(heap.free_space() > 0);
    }

    #[test]
    fn test_alloc() {
        let mut heap = Heap::new(4096);

        let ptr = heap.alloc(64, MemoryTag::Object);
        assert!(ptr.is_some());
        assert!(heap.heap_used() > 0);

        unsafe {
            let header = heap.get_header(ptr.unwrap());
            assert_eq!(header.tag(), MemoryTag::Object);
            assert!(!header.is_marked());
        }
    }

    #[test]
    fn test_alloc_zeroed() {
        let mut heap = Heap::new(4096);

        let ptr = heap.alloc_zeroed(32, MemoryTag::String).unwrap();

        // Check that memory is zeroed
        unsafe {
            for i in 0..32 {
                assert_eq!(*ptr.add(i), 0);
            }
        }
    }

    #[test]
    fn test_out_of_memory() {
        let mut heap = Heap::new(1024);

        // Try to allocate more than available
        let ptr = heap.alloc(2048, MemoryTag::Object);
        assert!(ptr.is_none());
    }

    #[test]
    fn test_block_iterator() {
        let mut heap = Heap::new(4096);

        heap.alloc(32, MemoryTag::Object);
        heap.alloc(64, MemoryTag::String);
        heap.alloc(16, MemoryTag::Float64);

        let blocks: Vec<_> = heap.iter_blocks().collect();
        assert_eq!(blocks.len(), 3);
    }

    #[test]
    fn test_stack_operations() {
        let mut heap = Heap::new(4096);

        let initial_free = heap.free_space();

        let ptr = heap.stack_push(4);
        assert!(ptr.is_some());
        assert!(heap.free_space() < initial_free);

        heap.stack_pop(4);
        assert_eq!(heap.free_space(), initial_free);
    }

    #[test]
    fn test_header_mark_bit() {
        let mut header = BlockHeader::new(MemoryTag::Object, 8);
        assert!(!header.is_marked());

        header.set_marked(true);
        assert!(header.is_marked());
        assert_eq!(header.tag(), MemoryTag::Object);
        assert_eq!(header.size_words(), 8);

        header.set_marked(false);
        assert!(!header.is_marked());
    }
}
