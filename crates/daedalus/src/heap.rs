//! This file is responsible for initialising
//! the rust heap allocator for `Lepton3` usage.

use linked_list_allocator::LockedHeap;

// The linker tells us where our static heap is
unsafe extern "C" {
    static __heap_start: u8;
    static __heap_end: u8;
}

// The global allocator, we just make use of the
// LockedHeap provided by linked_list_allocator.
#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

/// Initialises the global heap allocator for
/// rust `alloc` usage.
pub fn initialise_heap() {
    unsafe {
        let heap_start = &__heap_start as *const u8 as usize;
        let heap_end = &__heap_end as *const u8 as usize;
        ALLOCATOR
            .lock()
            .init(heap_start as *mut u8, heap_end - heap_start);
    }
}
