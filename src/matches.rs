//! The parsed result.
//!
//! A [`Matches`] is what the parser produces for one command level: the flags
//! that were set, the counts of counting flags, the values that options and
//! positionals received, and — if a subcommand was invoked — the [`Matches`] for
//! that subcommand, nested. A command's `run` handler receives the `Matches` for
//! its own level.

use std::collections::{HashMap, HashSet};

/// Parsed arguments for one command level.
///
/// Read flags with [`flag`](Matches::flag), counting flags with
/// [`count`](Matches::count), single values with [`value`](Matches::value),
/// repeated/variadic values with [`values`](Matches::values), and descend into an
/// invoked subcommand with [`subcommand`](Matches::subcommand).
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
///         .arg(Arg::count("verbose").short('v'))
///         .arg(Arg::option("jobs").short('j').default("1")),
/// );
///
/// let matches = app.try_parse_from(["build", "-r", "-vv", "--jobs", "8"]).unwrap();
/// let (name, build) = matches.subcommand().unwrap();
/// assert_eq!(name, "build");
/// assert!(build.flag("release"));
/// assert_eq!(build.count("verbose"), 2);
/// assert_eq!(build.value("jobs"), Some("8"));
/// ```
#[derive(Clone, Debug, Default)]
pub struct Matches {
    pub(crate) flags: HashSet<String>,
    pub(crate) counts: HashMap<String, usize>,
    pub(crate) values: HashMap<String, Vec<String>>,
    pub(crate) subcommand: Option<(String, Box<Matches>)>,
}

impl Matches {
    /// Whether the flag named `name` was set.
    ///
    /// Returns `false` for an unset flag or an unknown name. A
    /// [counting flag](crate::Arg::count) reports `true` once its count reaches
    /// one, so `flag` works for either kind.
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
        self.flags.contains(name) || self.count(name) > 0
    }

    /// How many times the [counting flag](crate::Arg::count) named `name` was
    /// given.
    ///
    /// Returns `0` for a flag that was not given or an unknown name.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg, Command};
    ///
    /// let mut app = App::new("demo");
    /// app.register(Command::new("run").arg(Arg::count("verbose").short('v')));
    ///
    /// let quiet = app.try_parse_from(["run"]).unwrap();
    /// assert_eq!(quiet.subcommand().unwrap().1.count("verbose"), 0);
    ///
    /// let loud = app.try_parse_from(["run", "-vvv"]).unwrap();
    /// assert_eq!(loud.subcommand().unwrap().1.count("verbose"), 3);
    /// ```
    #[must_use]
    pub fn count(&self, name: &str) -> usize {
        self.counts.get(name).copied().unwrap_or(0)
    }

    /// The value given for an option or positional named `name`, or its default.
    ///
    /// Returns `None` if the argument was not provided and has no default, or if
    /// the name is unknown. For a [`multiple`](crate::Arg::multiple) argument this
    /// is the first value; use [`values`](Matches::values) for all of them.
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
        self.values
            .get(name)
            .and_then(|values| values.first())
            .map(String::as_str)
    }

    /// Every value collected for `name`, in the order given.
    ///
    /// Yields all values of a [`multiple`](crate::Arg::multiple) option or
    /// variadic positional; for a single-valued argument it yields its one value
    /// (or its default). The iterator is empty for an argument that was not
    /// provided or an unknown name.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg, Command};
    ///
    /// let mut app = App::new("cc");
    /// app.register(Command::new("build").arg(Arg::option("define").short('D').multiple(true)));
    ///
    /// let m = app.try_parse_from(["build", "-D", "A=1", "-D", "B=2"]).unwrap();
    /// let defines: Vec<&str> = m.subcommand().unwrap().1.values("define").collect();
    /// assert_eq!(defines, ["A=1", "B=2"]);
    /// ```
    ///
    /// Iterate directly without collecting:
    ///
    /// ```
    /// # use cli_forge::{App, Arg, Command};
    /// # let mut app = App::new("cc");
    /// # app.register(Command::new("build").arg(Arg::positional("files").multiple(true)));
    /// # let m = app.try_parse_from(["build", "a", "b"]).unwrap();
    /// # let (_, build) = m.subcommand().unwrap();
    /// let mut count = 0;
    /// for file in build.values("files") {
    ///     let _ = file;
    ///     count += 1;
    /// }
    /// assert_eq!(count, 2);
    /// ```
    pub fn values(&self, name: &str) -> impl Iterator<Item = &str> {
        self.values
            .get(name)
            .into_iter()
            .flatten()
            .map(String::as_str)
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
