//! **Test-only: a per-thread live-byte counter.** RFC-054 PR-054-C measures what
//! a terminal's scrollback actually costs in memory -- the cap is chosen from
//! that number, not computed from a struct size -- and the only honest way to
//! ask "how many bytes does this `Term` hold" is to count the allocator's own
//! answer. Counting is **per thread** (a `const`-initialised, destructor-free
//! thread-local, safe to touch from inside an allocator), so the test binary's
//! other threads cannot move the number being read.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static LIVE: Cell<isize> = const { Cell::new(0) };
}

fn add(delta: isize) {
    let _ = LIVE.try_with(|live| live.set(live.get() + delta));
}

/// Bytes currently allocated *by this thread* and not yet freed.
pub(crate) fn live_bytes() -> isize {
    LIVE.try_with(Cell::get).unwrap_or(0)
}

pub(crate) struct CountingAllocator;

// SAFETY: every method delegates to `System` unchanged and only adjusts a
// thread-local counter around it.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            add(layout.size() as isize);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
        add(-(layout.size() as isize));
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            add(layout.size() as isize);
        }
        pointer
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let moved = unsafe { System.realloc(pointer, layout, new_size) };
        if !moved.is_null() {
            add(new_size as isize - layout.size() as isize);
        }
        moved
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
