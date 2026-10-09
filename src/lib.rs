//! # cli-forge
//!
//! A unified command-line framework where argument parsing and styled output
//! speak one API. Two things make it different from the alternatives: output is
//! *themed and reusable* rather than restyled at every call site, and the whole
//! styling layer is a seam that sibling crates — tables, progress bars,
//! gradients — build on, so everything a program prints speaks one system.
//!
//! ## Output, four ways
//!
//! Plain text is the common case and stays cheap: [`out`] and [`err`] do no
//! parsing and no allocation for a string literal.
//!
//! ```
//! # #[cfg(feature = "std")] fn main() {
//! use cli_forge::{err, out};
//!
//! out("building...");
//! err("something went wrong");
//! # }
//! # #[cfg(not(feature = "std"))] fn main() {}
//! ```
//!
//! When you want colour, pick whichever path suits the call — all of them render
//! to the same bytes for the same intent:
//!
//! ```
//! # #[cfg(feature = "std")] fn main() {
//! use cli_forge::{define, markup, named, out, style, Style};
//!
//! // 1. The builder — chain, then drop the result into `out`.
//! out(style("done").green().bold());
//!
//! // 2. Inline markup — parsed only here, never in `out`.
//! out(markup("<c=red><b>ERROR:</b></c> <c=#ff8800>disk almost full</c>"));
//!
//! // 3. A named style — described once, recalled anywhere, glyph and width
//! //    included, so no call site hand-builds a marker.
//! define("ok", Style::new().green().bold().prefix("✓ ").pad_to(10));
//! out(named("ok").paint("resolve dependencies"));
//!
//! // 4. A theme level — the vocabulary every program already has.
//! cli_forge::ok("deployed to staging");
//! cli_forge::warn("2 tests skipped");
//! # }
//! # #[cfg(not(feature = "std"))] fn main() {}
//! ```
//!
//! ## Themed, reusable responses
//!
//! The fourth path is the one most programs want. A [`Theme`] maps the levels a
//! CLI actually speaks in — success, error, warning, info, hint, and the rest —
//! onto a style and a glyph, once, and [`ok`], [`fail`], [`warn`], [`info`],
//! [`hint`], [`note`], [`debug`], and [`trace`] print through it. Replace the
//! theme and every line in the program changes together:
//!
//! ```
//! # #[cfg(feature = "std")] fn main() {
//! use cli_forge::{Level, Style, Theme};
//!
//! Theme::new()
//!     .set(Level::Success, Style::new().bright_green().bold(), "✓")
//!     .set(Level::Error, Style::new().bright_red().bold(), "✗")
//!     .install();
//!
//! cli_forge::ok("nothing in this call names a colour");
//! # }
//! # #[cfg(not(feature = "std"))] fn main() {}
//! ```
//!
//! Glyphs fall back to ASCII where the destination cannot render them, and
//! diagnostics go to standard error while data goes to standard output, so piped
//! output stays clean without the program arranging it.
//!
//! ## Colours and terminals
//!
//! Colours are the sixteen terminal names, any 256-palette index, or any 24-bit
//! value via [`Style::hex`] / [`Style::rgb`] / [`Style::fg`]. Capability is
//! detected *per stream*, so redirecting one does not silence the other, and an
//! exact colour degrades to the nearest the terminal can render rather than being
//! dropped. [`terminal::set_color_choice`] is what a `--color` flag drives. The
//! Windows console is handled behind the same API as Unix terminals.
//!
//! ## Commands
//!
//! Build a recursive [`Command`] tree, register commands into an [`App`] from
//! anywhere, and let [`App::parse`] resolve the invocation, parse arguments, and
//! run the selected command's handler:
//!
//! ```no_run
//! # #[cfg(feature = "std")] fn main() {
//! use cli_forge::{out, App, Arg, Command};
//!
//! let mut app = App::new("forge");
//! app.register(
//!     Command::new("build")
//!         .about("compile the project")
//!         .arg(Arg::flag("release").short('r'))
//!         .arg(Arg::option("jobs").short('j').default("1"))
//!         .run(|m| out(format!("release={} jobs={}", m.flag("release"), m.value("jobs").unwrap_or("?")))),
//! );
//! let _ = app.parse();
//! # }
//! # #[cfg(not(feature = "std"))] fn main() {}
//! ```
//!
//! Malformed input never panics: [`App::parse`] prints a structured
//! [`ParseError`] and exits, while [`App::try_parse_from`] returns it.
//!
//! ## Untrusted text
//!
//! A string that came from outside the program — a filename, a server response,
//! a commit message — can contain escape sequences, and printing it verbatim
//! hands the terminal to whoever wrote it. [`text::sanitize`] neutralises that,
//! and [`text::width`] / [`text::strip`] / [`text::truncate`] measure and shape
//! text that is already styled.
//!
//! ## Feature flags
//!
//! - **`std`** *(default)* — terminal detection, the stdout/stderr writers, and
//!   the command layer. Without it, the styling core still works on `alloc`.
//! - **`color`** *(default)* — ANSI styled output. Disable for plain output; the
//!   API stays complete and every styled value renders as its plain text.
//! - **`unicode`** *(default)* — correct display widths for CJK, emoji, and
//!   combining marks.
//! - **`termsize`** *(default)* — wrap help to the real terminal width.
//! - **`auth`** — the authorization seam: [`App::auth`], [`AuthRequest`], and
//!   enforcement of [`Command::requires_auth`].

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![deny(missing_debug_implementations)]
#![deny(unused_must_use)]
#![deny(unused_results)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::todo)]
#![deny(clippy::unimplemented)]
#![deny(clippy::unreachable)]
#![deny(clippy::print_stdout)]
#![deny(clippy::print_stderr)]
#![deny(clippy::dbg_macro)]

#[cfg(not(feature = "std"))]
extern crate alloc;

/// The owning types the crate uses, sourced from `alloc` or `std` depending on
/// the build.
///
/// The styling core needs heap allocation but not an operating system, so it is
/// written against these aliases rather than against `std` directly. That is the
/// whole mechanism behind the `no_std` build — there is no second
/// implementation to keep in step.
// Which aliases are referenced depends on the feature set and on whether the
// test modules are being compiled, so a shim always has some that this build
// happens not to need.
#[allow(unused_imports)]
pub(crate) mod shim {
    #[cfg(not(feature = "std"))]
    pub(crate) use alloc::{
        borrow::Cow,
        boxed::Box,
        format,
        string::{String, ToString},
        vec::Vec,
    };
    #[cfg(feature = "std")]
    pub(crate) use std::{
        borrow::Cow,
        boxed::Box,
        format,
        string::{String, ToString},
        vec::Vec,
    };
}

mod color;
mod style;
mod tags;
pub mod terminal;
pub mod text;
mod theme;

#[cfg(feature = "std")]
mod app;
#[cfg(feature = "std")]
mod arg;
#[cfg(feature = "auth")]
mod auth;
#[cfg(feature = "std")]
mod capture;
#[cfg(feature = "std")]
mod command;
#[cfg(feature = "std")]
mod error;
#[cfg(feature = "std")]
mod external;
#[cfg(feature = "std")]
mod group;
#[cfg(feature = "std")]
mod help;
#[cfg(feature = "std")]
mod matches;
#[cfg(feature = "std")]
mod output;
#[cfg(feature = "std")]
mod parser;
// The named-style store is process-global, which needs a lock; without `std`
// there is none, so styles are held and passed as values instead.
#[cfg(feature = "std")]
mod registry;

#[cfg(all(test, feature = "color"))]
mod crosspath_tests;

/// Serialises the tests that change process-wide presentation state.
///
/// The colour choice, the forced depth, and the installed theme are global by
/// design, and the test harness runs tests on parallel threads. A test that sets
/// `ColorChoice::Never` while another asserts on what `Always` renders is a race,
/// and several of them passed only by timing luck until CI's runners exposed it.
/// Every test that writes that state, or asserts on output that depends on it,
/// takes this lock first. A poisoned lock is recovered rather than cascading.
#[cfg(test)]
pub(crate) fn global_state_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The prose documentation, compiled as doctests.
///
/// Documentation that is never executed rots, and a guide whose first example
/// does not compile is worse than no guide. Including these files here means
/// every Rust block in them is built and run by `cargo test`, so an API change
/// that invalidates one is a failing test rather than a bad first impression.
///
/// `cfg(doctest)` means the module exists only while doctests are being
/// collected, so none of this appears in the rendered documentation.
#[cfg(all(doctest, feature = "std"))]
mod prose {
    #[doc = include_str!("../docs/README.md")]
    mod index {}
    #[doc = include_str!("../docs/GUIDE.md")]
    mod guide {}
    #[doc = include_str!("../docs/OUTPUT.md")]
    mod output {}
    #[doc = include_str!("../docs/COMMANDS.md")]
    mod commands {}
    #[doc = include_str!("../docs/RECIPES.md")]
    mod recipes {}
}

pub use crate::color::Color;
pub use crate::style::{Painted, Style, style};
pub use crate::tags::{markup, markup_at};
pub use crate::terminal::{ColorChoice, ColorLevel, Stream};
pub use crate::theme::{Glyphs, Level, Theme};

#[cfg(feature = "std")]
pub use crate::app::App;
#[cfg(feature = "std")]
pub use crate::arg::Arg;
#[cfg(feature = "auth")]
pub use crate::auth::AuthRequest;
#[cfg(feature = "std")]
pub use crate::capture::{Captured, capture};
#[cfg(feature = "std")]
pub use crate::command::Command;
#[cfg(feature = "std")]
pub use crate::error::{CommandError, ErrorKind, Outcome, ParseError};
#[cfg(feature = "std")]
pub use crate::external::External;
#[cfg(feature = "std")]
pub use crate::group::ArgGroup;
#[cfg(feature = "std")]
pub use crate::matches::{Matches, ValueSource};
#[cfg(feature = "std")]
pub use crate::output::{err, out, write_to};
#[cfg(feature = "std")]
pub use crate::registry::{define, defined, named};
#[cfg(feature = "std")]
pub use crate::theme::{debug, fail, hint, info, note, ok, trace, warn};
