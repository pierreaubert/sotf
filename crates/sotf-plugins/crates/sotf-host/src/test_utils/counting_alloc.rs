use super::misc::ALLOC_COUNT;
use super::misc::COUNTING_ENABLED;
use std::alloc::{GlobalAlloc, Layout, System};

pub struct CountingAlloc;

/// # Safety
/// This implementation is safe as it only increments a thread-local counter.
/// It uses `System` allocator for the actual memory operations.
unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = COUNTING_ENABLED.try_with(|enabled| {
            if enabled.get() {
                let _ = ALLOC_COUNT.try_with(|count| count.set(count.get() + 1));
            }
        });
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}
