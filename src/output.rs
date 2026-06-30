//! The plain output path: [`out`] and [`err`].
//!
//! These are the hot path and the common case — one call, no ceremony. They do
//! no tag parsing and no styling work: the value is formatted straight to the
//! stream and followed by a newline. Passing a plain `&str` is a near-direct
//! write with no heap allocation. Styling, when wanted, is paid for inside the
//! value's own [`Display`] (a [`Style`](crate::Style) or the `String` from
//! [`parse`](crate::parse) / [`tag`](crate::tag)), never here.

use std::fmt::Display;
use std::io::Write;

/// Print `value` to standard output, followed by a newline.
///
/// The common case is a string literal, which is written without parsing or
/// allocation. Because the argument is anything [`Display`], a
/// [`Style`](crate::Style) drops straight in and renders on the way out.
///
/// A failed write — a closed pipe, for instance — is silently ignored: a
/// fire-and-forget print must not panic or abort the program.
///
/// # Examples
///
/// ```
/// use cli_forge::{out, style};
///
/// out("building...");                 // plain, allocation-free
/// out(style("done").green().bold());  // styled, rendered on write
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
/// parsing, no allocation for plain strings, and write errors silently ignored.
///
/// # Examples
///
/// ```
/// use cli_forge::{err, style};
///
/// err("something went wrong");
/// err(style("ERROR:").red().bold());
/// ```
pub fn err<T: Display>(value: T) {
    let stderr = std::io::stderr();
    let mut handle = stderr.lock();
    // See `out`: a failed write to a print helper is intentionally ignored.
    let _ = writeln!(handle, "{value}");
}
