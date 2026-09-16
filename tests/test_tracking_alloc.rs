#![cfg(feature = "std")]

use anyhow::{anyhow, Context, Error};
use std::alloc::{GlobalAlloc, Layout, System};
use std::backtrace::BacktraceStatus;
use std::cell::Cell;
use std::hint::black_box;

thread_local! {
    static ALLOCATIONS: Cell<Option<usize>> = const { Cell::new(None) };
}

struct CountingAllocator;

// Count only the measuring thread; the test harness may allocate concurrently.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = ALLOCATIONS.try_with(|count| {
            if let Some(value) = count.get() {
                count.set(Some(value + 1));
            }
        });
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn allocations<T>(f: impl FnOnce() -> T) -> (T, usize) {
    ALLOCATIONS.with(|count| count.set(Some(0)));
    let value = black_box(f());
    let count = ALLOCATIONS.with(|count| count.take().unwrap());
    (value, count)
}

#[test]
fn tracking_uses_existing_allocations_only() {
    // Initialize the backtrace environment cache before measuring allocations.
    let warmup = Error::msg("warmup");
    assert_ne!(
        warmup.backtrace().status(),
        BacktraceStatus::Captured,
        "run allocation tests with RUST_LIB_BACKTRACE=0",
    );
    drop(warmup);

    let (error, count) = allocations(|| Error::msg("root"));
    assert_eq!(count, 1);
    let (error, count) = allocations(|| error.context("context"));
    assert_eq!(count, 1);
    let (error, count) = allocations(|| anyhow!(error));
    assert_eq!(count, 0);
    assert_eq!(error.to_string(), "context");

    let (value, count) = allocations(|| Ok::<_, Error>(42).with_context(|| "unused"));
    assert_eq!(count, 0);
    assert_eq!(value.unwrap(), 42);

    let (error, count) = allocations(|| None::<()>.context("missing").unwrap_err());
    assert_eq!(count, 1);
    assert_eq!(error.to_string(), "missing");
}
