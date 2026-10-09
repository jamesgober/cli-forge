//! External subcommands: `forge foo` handing off to something outside the app.
//!
//! This is how `cargo` and `git` grow plugins. `cargo watch` is not part of
//! cargo; it is a separate `cargo-watch` binary that cargo finds and runs,
//! passing along everything after the command name. The app does not need to
//! know the plugin exists in advance, which is the point.
//!
//! [`App::external`](crate::App::external) turns it on. Any first token that
//! names no registered command is handed to the hook, together with the tokens
//! after it and the app-level arguments that were parsed before it:
//!
//! ```no_run
//! use cli_forge::{App, Command, External};
//!
//! let mut app = App::new("forge").external(|ext: &External<'_>| {
//!     let program = format!("forge-{}", ext.name());
//!     match std::process::Command::new(&program).args(ext.args()).status() {
//!         Ok(status) if status.success() => Ok(()),
//!         Ok(status) => Err(format!("{program} exited with {status}")),
//!         Err(_) => Err(format!("no such command: '{}'", ext.name())),
//!     }
//! });
//! app.register(Command::new("build"));
//! let _ = app.run();
//! ```
//!
//! ## Typos
//!
//! Once unknown names are handed off, a misspelt *built-in* is handed off too —
//! `forge buidl` reaches the hook rather than earning "did you mean 'build'?".
//! The hook is the only place that knows whether `forge-buidl` exists, so it is
//! the one that should fall back. [`External::suggestion`] carries the nearest
//! registered command for exactly that purpose.
//!
//! Flags are never handed off: a token beginning with `-` is still an app-level
//! argument or an error. Only a bare name reaches the hook.

use crate::matches::Matches;

/// An invocation of a command the app does not define, handed to the hook set
/// with [`App::external`](crate::App::external).
///
/// Marked `#[non_exhaustive]` so later versions can carry more context without a
/// breaking change.
///
/// # Examples
///
/// ```
/// use cli_forge::{App, Command};
///
/// let app = App::new("forge")
///     .external(|ext| {
///         assert_eq!(ext.name(), "watch");
///         assert_eq!(ext.args(), ["--clear", "src"]);
///     })
///     .command(Command::new("build"));
///
/// assert!(app.try_run_from(["watch", "--clear", "src"]).unwrap().is_ok());
/// ```
#[derive(Debug)]
#[non_exhaustive]
pub struct External<'a> {
    name: &'a str,
    args: &'a [String],
    matches: &'a Matches,
    suggestion: Option<&'a str>,
}

impl<'a> External<'a> {
    /// Build the request the hook receives.
    pub(crate) fn new(
        name: &'a str,
        args: &'a [String],
        matches: &'a Matches,
        suggestion: Option<&'a str>,
    ) -> External<'a> {
        External {
            name,
            args,
            matches,
            suggestion,
        }
    }

    /// The command name the user typed.
    #[must_use]
    pub fn name(&self) -> &str {
        self.name
    }

    /// Everything after the command name, untouched.
    ///
    /// Not parsed at all — flags included — because only the external program
    /// knows what its arguments mean.
    #[must_use]
    pub fn args(&self) -> &[String] {
        self.args
    }

    /// The app-level arguments parsed before the command name, so a plugin can
    /// inherit `--verbose` or `--color` without re-parsing them.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg};
    ///
    /// let app = App::new("forge")
    ///     .arg(Arg::count("verbose").short('v').global(true))
    ///     .external(|ext| assert_eq!(ext.matches().count("verbose"), 2));
    ///
    /// assert!(app.try_run_from(["-vv", "plugin"]).unwrap().is_ok());
    /// ```
    #[must_use]
    pub fn matches(&self) -> &Matches {
        self.matches
    }

    /// The registered command nearest to [`name`](External::name), if one is
    /// close enough to suggest.
    ///
    /// For a hook that finds no program by that name and wants to say what the
    /// user probably meant, the way it would have without the hook.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Command};
    ///
    /// let app = App::new("forge")
    ///     .external(|ext| match ext.suggestion() {
    ///         Some(nearest) => Err(format!("unknown command '{}'; did you mean '{nearest}'?", ext.name())),
    ///         None => Ok(()),
    ///     })
    ///     .command(Command::new("build"));
    ///
    /// let failure = app.try_run_from(["buidl"]).unwrap().unwrap_err();
    /// assert!(failure.message().contains("did you mean 'build'?"));
    /// ```
    #[must_use]
    pub fn suggestion(&self) -> Option<&str> {
        self.suggestion
    }
}
