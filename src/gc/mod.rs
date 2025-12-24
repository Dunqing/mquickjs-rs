//! Garbage collector module
//!
//! MQuickJS uses a tracing and compacting garbage collector.
//! This is different from QuickJS which uses reference counting.
//!
//! Benefits of tracing GC:
//! - Smaller object headers (no reference count)
//! - No memory fragmentation (compaction)
//! - Handles cycles automatically

mod allocator;
mod collector;

pub use allocator::{Heap, MemoryTag};
pub use collector::GcRef;

use crate::context::MemoryStats;

impl Heap {
    /// Run garbage collection
    pub fn collect(&mut self) {
        collector::collect(self);
    }

    /// Get memory statistics
    pub fn stats(&self) -> MemoryStats {
        MemoryStats {
            total: self.total_size,
            heap_used: self.heap_used(),
            stack_used: self.stack_used(),
            free: self.free_space(),
        }
    }
}
