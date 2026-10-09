//! Capturing output, so a program's printing can be tested.
//!
//! Testing what a CLI *prints* is normally the hard part: the output goes to a
//! file descriptor, so the usual answer is to spawn the binary as a subprocess
//! and read its pipes — which is slow, awkward to arrange, and cannot see
//! anything a library printed on the program's behalf.
//!
//! [`capture`] redirects this crate's output paths into a buffer for the
//! duration of a closure, so the assertion is an ordinary one:
//!
//! ```
//! use cli_forge::{capture, ok, out, warn};
//!
//! let (_, log) = capture(|| {
//!     out("building...");
//!     ok("done");
//!     warn("2 tests skipped");
//! });
//!
//! assert_eq!(log.out(), "building...\n✓ done\n");
//! assert_eq!(log.err(), "! 2 tests skipped\n");
//! ```
//!
//! Note what that test demonstrates without arranging anything: the warning went
//! to standard error and the success did not, which is the stream discipline a
//! theme is responsible for and the thing most worth having a test for.
//!
//! ## What it captures
//!
//! Everything that goes through [`out`](crate::out), [`err`](crate::err),
//! [`write_to`](crate::write_to), and the themed printers — including the help
//! and error reports [`App::run`](crate::App::run) and
//! [`App::parse`](crate::App::parse) print, so an invocation's whole
//! user-visible behaviour can be asserted in one place.
//!
//! Note which entry points print at all:
//! [`try_run_from`](crate::App::try_run_from) and
//! [`try_parse_from`](crate::App::try_parse_from) deliberately do not, so a
//! capture around one of those sees a command's *handler* output and nothing
//! else. That is usually what a test wants; where the report itself is the thing
//! under test, print it from the captured closure.
//!
//! It does **not** capture `println!`, a direct write to
//! [`std::io::stdout`], or anything a child process writes. Those never passed
//! through this crate, so it has no way to see them.
//!
//! ## Threads
//!
//! Capturing is per-thread: a capture on one thread never swallows another
//! thread's output, so tests that capture can run in parallel with tests that
//! print. Output from a thread *spawned inside* the closure is likewise not
//! captured, because it belongs to a different thread.
//!
//! While no capture is active anywhere in the process, the output path checks a
//! single relaxed atomic and proceeds exactly as before.

use std::cell::RefCell;
use std::fmt::Display;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::terminal::Stream;

/// How many captures are active across the whole process.
///
/// Checked before the thread-local, so a program that never captures pays one
/// relaxed load on the output path and nothing else.
static ACTIVE: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    /// This thread's capture buffers, if it is capturing.
    static BUFFERS: RefCell<Option<Buffers>> = const { RefCell::new(None) };
}

/// The buffers one capture writes into.
#[derive(Debug, Default)]
struct Buffers {
    out: String,
    err: String,
    combined: String,
}

/// Output recorded by [`capture`].
///
/// # Examples
///
/// ```
/// use cli_forge::{capture, err, out};
///
/// let (_, log) = capture(|| {
///     out("data");
///     err("a complaint");
/// });
///
/// assert_eq!(log.out(), "data\n");
/// assert_eq!(log.err(), "a complaint\n");
/// assert_eq!(log.combined(), "data\na complaint\n");
/// assert!(!log.is_empty());
/// ```
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Captured {
    out: String,
    err: String,
    combined: String,
}

impl Captured {
    /// Everything written to standard output, newlines included.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{capture, out};
    ///
    /// let (_, log) = capture(|| out("one"));
    /// assert_eq!(log.out(), "one\n");
    /// ```
    #[must_use]
    pub fn out(&self) -> &str {
        &self.out
    }

    /// Everything written to standard error, newlines included.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{capture, fail, Theme};
    ///
    /// # Theme::new().install();
    /// let (_, log) = capture(|| fail("it broke"));
    /// assert!(log.err().contains("it broke"));
    /// assert_eq!(log.out(), "", "a failure must not go to standard output");
    /// ```
    #[must_use]
    pub fn err(&self) -> &str {
        &self.err
    }

    /// Both streams, in the order the lines were actually written.
    ///
    /// Useful for asserting on a sequence that crosses streams — a warning
    /// between two successes, for instance — which neither buffer shows on its
    /// own.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{capture, err, out};
    ///
    /// let (_, log) = capture(|| {
    ///     out("first");
    ///     err("second");
    ///     out("third");
    /// });
    ///
    /// assert_eq!(log.combined(), "first\nsecond\nthird\n");
    /// ```
    #[must_use]
    pub fn combined(&self) -> &str {
        &self.combined
    }

    /// Whether nothing was printed at all.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::capture;
    ///
    /// let (_, log) = capture(|| {});
    /// assert!(log.is_empty());
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.combined.is_empty()
    }

    /// The captured lines of one stream, without their trailing newlines.
    ///
    /// The form most assertions want, because it compares a `Vec` of lines
    /// rather than one string with embedded newlines — which produces a far more
    /// readable failure.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{capture, ok, out, Theme};
    ///
    /// # Theme::new().install();
    /// let (_, log) = capture(|| {
    ///     out("building...");
    ///     ok("done");
    /// });
    ///
    /// assert_eq!(log.lines(cli_forge::Stream::Stdout), ["building...", "✓ done"]);
    /// ```
    #[must_use]
    pub fn lines(&self, stream: Stream) -> Vec<&str> {
        let text = match stream {
            Stream::Stdout => &self.out,
            Stream::Stderr => &self.err,
        };
        if text.is_empty() {
            return Vec::new();
        }
        text.trim_end_matches('\n').split('\n').collect()
    }
}

/// Restores the previous capture state when it goes out of scope.
///
/// A guard rather than a plain pair of calls so that a panic inside the captured
/// closure cannot leave the thread permanently redirected.
struct Guard {
    previous: Option<Buffers>,
}

impl Guard {
    /// Begin capturing on this thread, remembering whatever was already there.
    fn install() -> Guard {
        let previous = BUFFERS.with(|cell| cell.borrow_mut().replace(Buffers::default()));
        // Raised after the thread-local is in place, so this thread cannot
        // observe "capturing" before its own buffers exist.
        let _ = ACTIVE.fetch_add(1, Ordering::Relaxed);
        Guard { previous }
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = ACTIVE.fetch_sub(1, Ordering::Relaxed);
        BUFFERS.with(|cell| *cell.borrow_mut() = self.previous.take());
    }
}

/// Capture everything this crate prints while `body` runs.
///
/// Returns whatever `body` returned, alongside the recorded output — so an
/// invocation's exit status and its printing can be asserted together.
///
/// # Examples
///
/// Testing an invocation's printing and its outcome together:
///
/// ```
/// use cli_forge::{capture, ok, App, Command, Theme};
///
/// # Theme::new().install();
/// let mut app = App::new("forge");
/// app.register(Command::new("build").run(|_| ok("compiled 3 targets")));
///
/// let (outcome, log) = capture(|| app.try_run_from(["build"]));
///
/// assert!(outcome.unwrap().is_ok());
/// assert!(log.out().contains("compiled 3 targets"));
/// ```
///
/// A report is only printed by the entry points that print —
/// [`App::run`](crate::App::run) and [`App::parse`](crate::App::parse).
/// [`try_run_from`](crate::App::try_run_from) hands the failure back instead, so
/// print it yourself if that is what you mean to assert on:
///
/// ```
/// use cli_forge::{capture, err, App, Command};
///
/// let mut app = App::new("forge");
/// app.register(Command::new("pull").run(|_| Err("not a repository")));
///
/// let (outcome, log) = capture(|| {
///     match app.try_run_from(["pull"]) {
///         Ok(Err(failure)) => err(failure.message()),
///         _ => {}
///     }
/// });
///
/// let _ = outcome;
/// assert_eq!(log.err(), "not a repository\n");
/// ```
///
/// Nesting is allowed: the inner capture takes over for its duration, and the
/// outer one resumes afterwards without the inner's content.
///
/// ```
/// use cli_forge::{capture, out};
///
/// let (_, outer) = capture(|| {
///     out("before");
///     let (_, inner) = capture(|| out("inside"));
///     assert_eq!(inner.out(), "inside\n");
///     out("after");
/// });
///
/// assert_eq!(outer.out(), "before\nafter\n");
/// ```
///
/// # Panics
///
/// Never, and a panic raised by `body` propagates with the thread's output
/// redirection already undone.
pub fn capture<T, F: FnOnce() -> T>(body: F) -> (T, Captured) {
    let guard = Guard::install();
    let value = body();
    // Taken before the guard restores, so this is the inner capture's content.
    let buffers = BUFFERS
        .with(|cell| cell.borrow_mut().take())
        .unwrap_or_default();
    drop(guard);

    (
        value,
        Captured {
            out: buffers.out,
            err: buffers.err,
            combined: buffers.combined,
        },
    )
}

/// Whether any capture is active in this process.
///
/// One relaxed load, which is all a program that never captures ever pays.
///
/// Relaxed is sufficient, and not a shortcut: the counter only decides whether a
/// thread bothers to look at its *own* thread-local, and program order within
/// that thread already guarantees the buffers are in place before the count
/// rises. A thread that sees a stale zero prints normally; one that sees a stale
/// non-zero finds its own slot empty and prints normally. Neither reads another
/// thread's buffers, so there is nothing to synchronise.
#[inline]
pub(crate) fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed) != 0
}

/// Record `value` and a newline into this thread's capture, reporting whether it
/// was taken.
///
/// `false` means this thread is not capturing and the caller should write to the
/// real stream.
pub(crate) fn record<T: Display>(stream: Stream, value: &T) -> bool {
    BUFFERS.with(|cell| {
        // A capture that is being torn down, or a reentrant write from within
        // `Display`, must not deadlock the cell; falling through to the real
        // stream is the safe answer.
        let Ok(mut slot) = cell.try_borrow_mut() else {
            return false;
        };
        let Some(buffers) = slot.as_mut() else {
            return false;
        };
        let target = match stream {
            Stream::Stdout => &mut buffers.out,
            Stream::Stderr => &mut buffers.err,
        };
        // Writing to a `String` is infallible.
        let _ = writeln!(target, "{value}");
        let _ = writeln!(buffers.combined, "{value}");
        true
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Theme, err, out};

    /// Whether *this* thread is redirected.
    ///
    /// The tests assert on this rather than on [`active`], which is
    /// process-wide: the test harness runs tests on parallel threads, so another
    /// test's capture can legitimately hold the global count above zero at any
    /// moment. Asserting on the counter here passed locally by timing luck and
    /// failed on faster CI runners. The per-thread state is the invariant that
    /// actually matters; the counter's balance is checked in `tests/capture.rs`,
    /// a binary of its own where nothing else runs concurrently.
    fn redirected() -> bool {
        BUFFERS.with(|cell| cell.borrow().is_some())
    }

    /// Serialises the tests in this module.
    ///
    /// Only these tests capture within the library's own test binary, so holding
    /// this while asserting on [`active`] makes the process-wide counter
    /// observable without a race. A poisoned lock — the panic test poisons
    /// nothing, but a failing assertion would — is recovered rather than
    /// cascading into every later test.
    fn serial() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        LOCK.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    #[test]
    fn test_the_global_counter_balances() {
        let _serial = serial();
        assert!(!active(), "nothing should be capturing at the start");

        let _ = capture(|| out("once"));
        assert!(!active(), "a capture must release the counter");

        let _ = capture(|| {
            let _ = capture(|| out("nested"));
            assert!(active(), "the outer capture is still running");
        });
        assert!(!active(), "nested captures must release it twice");

        let _ = std::panic::catch_unwind(|| {
            let _ = capture(|| panic!("deliberate"));
        });
        assert!(!active(), "a panic must still release the counter");
    }

    #[test]
    fn test_nothing_is_captured_outside_a_capture() {
        let _serial = serial();
        assert!(!redirected());
    }

    #[test]
    fn test_streams_are_recorded_separately_and_in_order() {
        let _serial = serial();
        let (value, log) = capture(|| {
            out("one");
            err("two");
            out("three");
            7
        });

        assert_eq!(value, 7);
        assert_eq!(log.out(), "one\nthree\n");
        assert_eq!(log.err(), "two\n");
        assert_eq!(log.combined(), "one\ntwo\nthree\n");
    }

    #[test]
    fn test_lines_drops_the_trailing_newline() {
        let _serial = serial();
        let (_, log) = capture(|| {
            out("a");
            out("b");
        });
        assert_eq!(log.lines(Stream::Stdout), ["a", "b"]);
        assert_eq!(log.lines(Stream::Stderr), Vec::<&str>::new());
    }

    #[test]
    fn test_an_empty_capture_is_empty() {
        let _serial = serial();
        let (_, log) = capture(|| {});
        assert!(log.is_empty());
        assert_eq!(log.out(), "");
        assert_eq!(log.err(), "");
        assert_eq!(log.lines(Stream::Stdout), Vec::<&str>::new());
    }

    #[test]
    fn test_state_is_restored_afterwards() {
        let _serial = serial();
        let _ = capture(|| out("inside"));
        assert!(!redirected(), "the thread must be un-redirected afterwards");
    }

    #[test]
    fn test_nesting_keeps_the_two_apart() {
        let _serial = serial();
        let (_, outer) = capture(|| {
            out("before");
            let (_, inner) = capture(|| out("inside"));
            assert_eq!(inner.out(), "inside\n");
            out("after");
        });
        assert_eq!(outer.out(), "before\nafter\n");
        assert!(!redirected());
    }

    #[test]
    fn test_a_panic_inside_the_body_still_restores() {
        let _serial = serial();
        let result = std::panic::catch_unwind(|| {
            let _ = capture(|| {
                out("before the panic");
                panic!("deliberate");
            });
        });
        assert!(result.is_err());
        assert!(
            !redirected(),
            "a panic must not leave the thread redirected"
        );
    }

    #[test]
    fn test_themed_output_lands_on_the_right_stream() {
        let _serial = serial();
        // The property worth testing in a real program, and the reason this
        // module exists: the stream split is the theme's responsibility, and
        // nothing else can observe it.
        let saved = Theme::current();
        Theme::new().install();

        let (_, log) = capture(|| {
            crate::ok("success");
            crate::info("context");
            crate::warn("attention");
            crate::fail("broken");
        });

        assert!(log.out().contains("success"));
        assert!(log.out().contains("context"));
        assert!(!log.out().contains("attention"));
        assert!(!log.out().contains("broken"));

        assert!(log.err().contains("attention"));
        assert!(log.err().contains("broken"));

        saved.install();
    }

    #[test]
    fn test_write_to_is_captured_and_still_reports_success() {
        let _serial = serial();
        let (result, log) = capture(|| crate::write_to(Stream::Stdout, "accountable"));
        assert!(result.is_ok());
        assert_eq!(log.out(), "accountable\n");
    }

    #[test]
    fn test_a_value_of_any_type_comes_back() {
        let _serial = serial();
        let (value, _) = capture(|| "borrowed");
        assert_eq!(value, "borrowed");

        let (value, _) = capture(|| vec![1, 2, 3]);
        assert_eq!(value, [1, 2, 3]);
    }
}
