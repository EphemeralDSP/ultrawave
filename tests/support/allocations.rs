use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

struct CountingAllocator;
thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static OPS: Cell<usize> = const { Cell::new(0) };
}

// SAFETY: forward unchanged pointer/layout pairs to System; counters are
// allocation-free and thread-local, so background workers are not charged.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        COUNTING.with(|on| {
            if on.get() {
                OPS.with(|n| n.set(n.get() + 1));
            }
        });
        // SAFETY: forward the allocator caller's valid layout.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        COUNTING.with(|on| {
            if on.get() {
                OPS.with(|n| n.set(n.get() + 1));
            }
        });
        // SAFETY: forward the allocation's original pointer and layout.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

pub fn assert_no_alloc(f: impl FnOnce()) {
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            COUNTING.with(|on| on.set(false));
        }
    }
    OPS.with(|n| n.set(0));
    COUNTING.with(|on| on.set(true));
    let guard = Guard;
    f();
    drop(guard);
    assert_eq!(OPS.with(Cell::get), 0, "audio-thread allocations or frees");
}
