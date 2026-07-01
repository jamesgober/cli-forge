//! # cli-forge
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
//! use cli_forge::{out, err};
//!
//! out("building...");
//! err("something went wrong");
//! ```
//!
//! When you want color, opt into one of three paths that all render to the same
//! bytes for the same intent:
//!
//! ```
//! use cli_forge::{define_tag, out, parse, style, tag};
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
//! ## Commands
//!
//! Build a recursive [`Command`] tree, register commands into an [`App`] from
//! anywhere, and let [`App::parse`] resolve the invocation, parse arguments, and
//! run the selected command's handler:
//!
//! ```no_run
//! use cli_forge::{App, Arg, Command, out};
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
//! ```
//!
//! Malformed input never panics: [`App::parse`] prints a structured
//! [`ParseError`] and exits, while [`App::try_parse_from`] returns it.
//!
//! ## Feature flags
//!
//! - **`std`** *(default)* — terminal detection, the stdout/stderr writers, and
//!   the command layer.
//! - **`color`** *(default)* — ANSI styled output. Disable for plain output; the
//!   API stays complete and every styled value renders as its plain text.
//! - **`auth`** — the authorization seam: `App::auth`, `AuthRequest`, and
//!   enforcement of [`Command::requires_auth`].

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
mod app;
#[cfg(feature = "std")]
mod arg;
#[cfg(feature = "auth")]
mod auth;
#[cfg(feature = "std")]
mod color;
#[cfg(feature = "std")]
mod command;
#[cfg(feature = "std")]
mod error;
#[cfg(feature = "std")]
mod help;
#[cfg(feature = "std")]
mod matches;
#[cfg(feature = "std")]
mod output;
#[cfg(feature = "std")]
mod parser;
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
pub use crate::app::App;
#[cfg(feature = "std")]
pub use crate::arg::Arg;
#[cfg(feature = "auth")]
pub use crate::auth::AuthRequest;
#[cfg(feature = "std")]
pub use crate::command::Command;
#[cfg(feature = "std")]
pub use crate::error::ParseError;
#[cfg(feature = "std")]
pub use crate::matches::Matches;
#[cfg(feature = "std")]
pub use crate::output::{err, out};
#[cfg(feature = "std")]
pub use crate::registry::{Tag, define_tag, tag};
#[cfg(feature = "std")]
pub use crate::style::{Style, style};
#[cfg(feature = "std")]
pub use crate::tags::parse;
