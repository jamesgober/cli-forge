//! The plain output path: [`out`] and [`err`].
//!
//! These are the hot path and the common case — one call, no ceremony. They do no
//! markup parsing and no styling work: the value is formatted straight to the
//! stream and followed by a newline. Passing a plain `&str` is a near-direct
//! write with no heap allocation, which `tests/allocation.rs` asserts by
//! measurement rather than by claim. Styling, when wanted, is paid for inside the
//! value's own [`Display`] — a [`Style`](crate::Style), a
//! [`Painted`](crate::Painted), or the `String` from
//! [`markup`](crate::markup) — never here.
//!
//! A failed write is deliberately ignored. A print helper has no way to report
//! failure that the caller could act on, and the usual cause is a closed pipe
//! (`yourtool | head`), where aborting would turn an ordinary shell idiom into a
//! crash. Where the result matters, [`write_to`] hands it back.

use std::fmt::Display;
use std::io::Write;

use crate::terminal::Stream;

/// Print `value` to standard output, followed by a newline.
///
/// The common case is a string literal, which is written without parsing or
/// allocation. Because the argument is anything [`Display`], a
/// [`Style`](crate::Style) drops straight in and renders on the way out.
///
/// # Examples
///
/// ```
/// use cli_forge::{out, style};
///
/// out("building...");                  // plain, allocation-free
/// out(style("done").green().bold());   // styled, rendered on write
/// out(format!("built {} targets", 3)); // any Display value
/// ```
pub fn out<T: Display>(value: T) {
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    // A broken pipe or closed stream is unrecoverable from a print helper and
    // must not abort the program, so the write result is deliberately dropped.
    let _ = writeln!(handle, "{value}");
}

/// Print `value` to standard error, followed by a newline.
///
/// The standard-error counterpart of [`out`], with the same contract: no
/// parsing, no allocation for plain strings, and write errors ignored.
///
/// Note that a [`Style`](crate::Style) renders at the depth detected for standard
/// *output*, because [`Display`] cannot know where it is being written. When the
/// two streams may differ — one redirected, the other not — render explicitly
/// with [`Style::render_for`](crate::Style::render_for), or use the themed
/// printers such as [`fail`](crate::fail), which already do.
///
/// # Examples
///
/// ```
/// use cli_forge::{err, style, Stream};
///
/// err("something went wrong");
/// err(style("ERROR:").red().bold().render_for(Stream::Stderr));
/// ```
pub fn err<T: Display>(value: T) {
    let stderr = std::io::stderr();
    let mut handle = stderr.lock();
    // See `out`: a failed write from a print helper is intentionally ignored.
    let _ = writeln!(handle, "{value}");
}

/// Write `value` and a newline to `stream`, reporting whether it succeeded.
///
/// The accountable counterpart of [`out`] and [`err`], for the callers that
/// genuinely need to know: a program whose exit status must reflect a failed
/// write, or one draining to a pipe that may have gone away.
///
/// # Examples
///
/// ```
/// use cli_forge::{write_to, Stream};
///
/// if write_to(Stream::Stdout, "a line that matters").is_err() {
///     // The pipe is gone; stop producing output.
/// }
/// ```
pub fn write_to<T: Display>(stream: Stream, value: T) -> std::io::Result<()> {
    match stream {
        Stream::Stdout => {
            let stdout = std::io::stdout();
            let mut handle = stdout.lock();
            writeln!(handle, "{value}")
        }
        Stream::Stderr => {
            let stderr = std::io::stderr();
            let mut handle = stderr.lock();
            writeln!(handle, "{value}")
        }
    }
}

/// Write an already-rendered line to `stream`, ignoring failure.
///
/// The themed printers' way out: they have resolved the stream themselves, so
/// they must not go back through [`out`]/[`err`] and re-decide it.
pub(crate) fn write_line(stream: Stream, line: &str) {
    match stream {
        Stream::Stdout => out(line),
        Stream::Stderr => err(line),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_to_reports_success_on_a_live_stream() {
        // Under the test harness both streams are captured pipes that accept
        // writes; the point is that the result is surfaced rather than dropped.
        assert!(write_to(Stream::Stdout, "probe").is_ok());
        assert!(write_to(Stream::Stderr, "probe").is_ok());
    }

    #[test]
    fn test_printers_accept_any_display_value() {
        // A compile-time guarantee as much as a runtime one: these must stay
        // generic over `Display` rather than narrowing to `&str`.
        out("literal");
        out(42);
        out(format_args!("{}+{}", 1, 2));
        out(crate::style("styled").green());
        err("literal");
        err(crate::Style::new().red().paint("painted"));
    }
}
