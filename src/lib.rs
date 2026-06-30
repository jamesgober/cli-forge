//! # cli-core
//!
//! A unified command-line framework where argument parsing and styled output
//! speak one API. This release delivers the output layer every other piece — and
//! every sibling crate in the cli collection — is built on: one styling system,
//! reached three ways, over a single cross-platform terminal backend.
//!
//! ## The three styling paths
//!
//! Plain text is the common case and stays cheap — [`out`] and [`err`] do no
//! parsing and no allocation for a string literal:
//!
//! ```
//! use cli_core::{out, err};
//!
//! out("building...");
//! err("something went wrong");
//! ```
//!
//! When you want color, opt into one of three paths that all render to the same
//! bytes for the same intent:
//!
//! ```
//! use cli_core::{define_tag, out, parse, style, tag};
//!
//! // 1. The builder — chain methods, drop the result into `out`.
//! out(style("done").green().bold());
//!
//! // 2. Inline tags — markup parsed only here, never in `out`.
//! parse("<c=red><b>ERROR:</b></c> <c=#ff8800>disk almost full</c>");
//!
//! // 3. A named style — define once, reuse anywhere.
//! define_tag("error", style("").red().bold());
//! out(tag("error").render_with("build failed"));
//! ```
//!
//! ## Colors and terminals
//!
//! Colors are the eight standard names, plus any 24-bit value via
//! [`Style::hex`] / [`Style::rgb`] or a `<c=#rrggbb>` / `<c=r,g,b>` tag. The
//! terminal's capability is detected once: on a true-color terminal the exact
//! value is emitted; on a 256- or 16-color terminal it is downgraded to the
//! nearest representable color; on a pipe, under `NO_COLOR`, or with the `color`
//! feature off, styling is dropped and only text is written. The Windows console
//! is handled behind the same API as Unix terminals.
//!
//! ## Feature flags
//!
//! - **`std`** *(default)* — terminal detection and the stdout/stderr writers.
//! - **`color`** *(default)* — ANSI styled output. Disable for plain output; the
//!   API stays complete and every styled value renders as its plain text.
//! - **`auth`** — reserved for the `requires_auth` command flag (v0.5.0); no
//!   effect yet.

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]
#![deny(missing_docs)]
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

#[cfg(feature = "std")]
mod color;
#[cfg(feature = "std")]
mod output;
#[cfg(feature = "std")]
mod registry;
#[cfg(feature = "std")]
mod style;
#[cfg(feature = "std")]
mod tags;
#[cfg(feature = "std")]
mod terminal;

#[cfg(all(test, feature = "color"))]
mod crosspath_tests;

#[cfg(feature = "std")]
pub use crate::output::{err, out};
#[cfg(feature = "std")]
pub use crate::registry::{Tag, define_tag, tag};
#[cfg(feature = "std")]
pub use crate::style::{Style, style};
#[cfg(feature = "std")]
pub use crate::tags::parse;
