//! Proof, by measurement rather than by claim, that the plain output path does
//! not allocate.
//!
//! A counting global allocator records heap allocations that happen while a
//! per-thread flag is armed. The test arms the flag only around the exact
//! formatting operation `out`/`err` perform — `writeln!(writer, "{value}")` with
//! a `&str` value — writing to a pre-sized buffer that stands in for the
//! stdout/stderr handle (whose line buffer is likewise pre-allocated). If that
//! window records a single allocation, the plain path is not allocation-free and
//! the test fails.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Allocations counted while `RECORDING` is armed on the current thread.
static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    /// Whether allocations on this thread are currently being counted. A `const`
    /// initializer keeps the first access from allocating and recursing.
    static RECORDING: Cell<bool> = const { Cell::new(false) };
}

struct CountingAllocator;

// SAFETY-equivalent note: this allocator only adds bookkeeping around the system
// allocator; all real allocation/deallocation is delegated to `System`.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if RECORDING.with(Cell::get) {
            ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: `layout` is forwarded unchanged to the system allocator.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr`/`layout` came from `System.alloc` above.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// Count the allocations made while running `body`.
fn count_allocations(body: impl FnOnce()) -> usize {
    // Touch the thread-local before arming so its initialization is not counted.
    RECORDING.with(|r| r.set(false));
    ALLOC_COUNT.store(0, Ordering::Relaxed);
    RECORDING.with(|r| r.set(true));
    body();
    RECORDING.with(|r| r.set(false));
    ALLOC_COUNT.load(Ordering::Relaxed)
}

#[test]
fn test_plain_write_path_is_allocation_free() {
    // Stands in for the stdout/stderr handle: a pre-allocated, reused buffer.
    let mut buffer: Vec<u8> = Vec::with_capacity(256);
    let line = "deploying release artifacts to the staging environment";

    // Warm the buffer so its backing allocation predates the measured window.
    writeln!(buffer, "{line}").expect("write to buffer");
    buffer.clear();

    let allocations = count_allocations(|| {
        // Exactly what `out`/`err` do with a `&str`: format the value and a
        // newline straight to the writer.
        writeln!(buffer, "{line}").expect("write to buffer");
    });

    assert_eq!(
        allocations, 0,
        "the plain output path allocated {allocations} time(s); it must be allocation-free"
    );
    assert_eq!(buffer, format!("{line}\n").into_bytes());
}

#[test]
fn test_counter_detects_allocations() {
    // Guards the harness itself: a real allocation must be observed, otherwise a
    // zero result above would be meaningless.
    let allocations = count_allocations(|| {
        let v: Vec<u8> = Vec::with_capacity(64);
        std::hint::black_box(v);
    });
    assert!(
        allocations >= 1,
        "counting allocator failed to observe an allocation"
    );
}
