//! The parsed result.
//!
//! A [`Matches`] is what the parser produces for one command level: the flags
//! that were set, the values that options and positionals received, and — if a
//! subcommand was invoked — the [`Matches`] for that subcommand, nested. A
//! command's `run` handler receives the `Matches` for its own level.

use std::collections::{HashMap, HashSet};

/// Parsed arguments for one command level.
///
/// Read flags with [`flag`](Matches::flag), option and positional values with
/// [`value`](Matches::value), and descend into an invoked subcommand with
/// [`subcommand`](Matches::subcommand).
///
/// # Examples
///
/// ```
/// use cli_forge::{App, Arg, Command};
///
/// let mut app = App::new("demo");
/// app.register(
///     Command::new("build")
///         .arg(Arg::flag("release").short('r'))
///         .arg(Arg::option("jobs").short('j').default("1")),
/// );
///
/// let matches = app.try_parse_from(["build", "-r", "--jobs", "8"]).unwrap();
/// let (name, build) = matches.subcommand().unwrap();
/// assert_eq!(name, "build");
/// assert!(build.flag("release"));
/// assert_eq!(build.value("jobs"), Some("8"));
/// ```
#[derive(Clone, Debug, Default)]
pub struct Matches {
    pub(crate) flags: HashSet<String>,
    pub(crate) values: HashMap<String, String>,
    pub(crate) subcommand: Option<(String, Box<Matches>)>,
}

impl Matches {
    /// Whether the flag named `name` was set.
    ///
    /// Returns `false` for an unset flag or an unknown name.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg, Command};
    ///
    /// let mut app = App::new("demo");
    /// app.register(Command::new("run").arg(Arg::flag("verbose").short('v')));
    ///
    /// let m = app.try_parse_from(["run", "-v"]).unwrap();
    /// assert!(m.subcommand().unwrap().1.flag("verbose"));
    /// ```
    #[must_use]
    pub fn flag(&self, name: &str) -> bool {
        self.flags.contains(name)
    }

    /// The value given for an option or positional named `name`, or its default.
    ///
    /// Returns `None` if the argument was not provided and has no default, or if
    /// the name is unknown.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg, Command};
    ///
    /// let mut app = App::new("demo");
    /// app.register(Command::new("greet").arg(Arg::positional("name").default("world")));
    ///
    /// let provided = app.try_parse_from(["greet", "Ada"]).unwrap();
    /// assert_eq!(provided.subcommand().unwrap().1.value("name"), Some("Ada"));
    ///
    /// let defaulted = app.try_parse_from(["greet"]).unwrap();
    /// assert_eq!(defaulted.subcommand().unwrap().1.value("name"), Some("world"));
    /// ```
    #[must_use]
    pub fn value(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    /// The invoked subcommand's name and its own [`Matches`], if one was given.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Command};
    ///
    /// let mut app = App::new("demo");
    /// app.register(Command::new("status"));
    ///
    /// let m = app.try_parse_from(["status"]).unwrap();
    /// assert_eq!(m.subcommand().map(|(name, _)| name), Some("status"));
    /// ```
    #[must_use]
    pub fn subcommand(&self) -> Option<(&str, &Matches)> {
        self.subcommand
            .as_ref()
            .map(|(name, matches)| (name.as_str(), matches.as_ref()))
    }
}
