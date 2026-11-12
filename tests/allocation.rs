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
thread_local! {
    /// Whether allocations on this thread are currently being counted. A `const`
    /// initializer keeps the first access from allocating and recursing.
    static RECORDING: Cell<bool> = const { Cell::new(false) };

    /// Allocations counted while `RECORDING` is armed on this thread. Per
    /// thread, so tests measuring in parallel cannot reset each other's count.
    static ALLOC_COUNT: Cell<usize> = const { Cell::new(0) };
}

struct CountingAllocator;

// SAFETY-equivalent note: this allocator only adds bookkeeping around the system
// allocator; all real allocation/deallocation is delegated to `System`.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if RECORDING.with(Cell::get) {
            ALLOC_COUNT.with(|count| count.set(count.get() + 1));
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
    ALLOC_COUNT.with(|count| count.set(0));
    RECORDING.with(|r| r.set(true));
    body();
    RECORDING.with(|r| r.set(false));
    ALLOC_COUNT.with(Cell::get)
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

#[test]
fn test_a_counting_flag_bundle_does_not_allocate_per_character() {
    use cli_forge::{App, Arg, Command};

    let app = App::new("forge").command(Command::new("b").arg(Arg::count("verbose").short('v')));
    let parse = |repeats: usize| {
        let argv = vec!["b".to_owned(), format!("-{}", "v".repeat(repeats))];
        let mut count = 0;
        let allocations = count_allocations(|| {
            let m = app.try_parse_from(argv).expect("parses");
            count = m.leaf().count("verbose");
        });
        assert_eq!(count, repeats);
        allocations
    };

    let short = parse(4);
    let long = parse(4_000);
    // Collecting the bundle's characters grows one buffer a few times; that is
    // all the extra length may cost.
    assert!(
        long < short + 32,
        "-v x4 made {short} allocations, -v x4000 made {long}"
    );
}
