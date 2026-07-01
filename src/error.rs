//! Parse errors.
//!
//! Every way command-line input can be malformed maps to a [`ParseError`]
//! variant — never a panic. The error carries enough context to tell the user
//! what was wrong (which flag, which argument), and it renders through the same
//! output system as everything else: [`App::parse`](crate::App::parse) prints it
//! to standard error and exits, while
//! [`App::try_parse_from`](crate::App::try_parse_from) hands it back for the
//! caller to handle.

use std::error::Error;
use std::fmt;

/// A failure to parse command-line arguments.
///
/// Returned by [`App::try_parse_from`](crate::App::try_parse_from). The variants
/// are marked `#[non_exhaustive]` so future versions can add cases (for example,
/// as the help and auth seams land) without a breaking change.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParseError {
    /// A `-x` / `--name` flag was given that no argument at this level declares.
    UnknownFlag {
        /// The flag as the user wrote it, e.g. `--verbsoe`.
        flag: String,
    },
    /// An option that takes a value was given without one (it was the last
    /// token, or followed only by another option).
    MissingValue {
        /// The option's name.
        option: String,
    },
    /// A required argument was not provided.
    MissingRequired {
        /// The missing argument's name.
        arg: String,
    },
    /// A subcommand name was expected but the token matched no registered
    /// command.
    UnknownCommand {
        /// The unrecognized command name.
        name: String,
    },
    /// A bare value was given that no positional argument or subcommand can
    /// accept.
    UnexpectedArgument {
        /// The surplus value.
        value: String,
    },
    /// Not an error: `-h` / `--help` was requested. Carries the rendered help
    /// text. [`App::parse`](crate::App::parse) prints it to standard output and
    /// exits `0`; callers of
    /// [`App::try_parse_from`](crate::App::try_parse_from) should do the same.
    HelpRequested(String),
    /// Not an error: `-V` / `--version` was requested. Carries the version
    /// string. Handled like [`HelpRequested`](ParseError::HelpRequested).
    VersionRequested(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::UnknownFlag { flag } => write!(f, "unknown flag: {flag}"),
            ParseError::MissingValue { option } => {
                write!(f, "missing value for option: {option}")
            }
            ParseError::MissingRequired { arg } => {
                write!(f, "missing required argument: {arg}")
            }
            ParseError::UnknownCommand { name } => write!(f, "unknown command: {name}"),
            ParseError::UnexpectedArgument { value } => {
                write!(f, "unexpected argument: {value}")
            }
            ParseError::HelpRequested(text) | ParseError::VersionRequested(text) => {
                write!(f, "{text}")
            }
        }
    }
}

impl Error for ParseError {}
