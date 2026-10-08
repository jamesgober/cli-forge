//! The parsed result.
//!
//! A [`Matches`] is what the parser produces for one command level: the flags
//! that were set, the tallies of counting flags, the values that options and
//! positionals received, where each of those values came from, and — if a
//! subcommand was invoked — the [`Matches`] for that subcommand, nested. A
//! command's `run` handler receives the `Matches` for its own level.
//!
//! Reading values back is deliberately boring, because the interesting work
//! happened earlier: an argument that declared
//! [`possible_values`](crate::Arg::possible_values) or a
//! [`validator`](crate::Arg::validate) has already been checked, so
//! [`get`](Matches::get) can hand back a typed value without a second error path
//! for the program to handle. [`source`](Matches::source) answers the question
//! that otherwise needs a sentinel default: did the user actually say this, or is
//! it just what we fell back to?

use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use crate::error::{ErrorKind, ParseError};

/// Where a value came from.
///
/// # Examples
///
/// ```
/// use cli_forge::{App, Arg, Command, ValueSource};
///
/// let mut app = App::new("demo");
/// app.register(Command::new("build").arg(Arg::option("jobs").default("1")));
///
/// let defaulted = app.try_parse_from(["build"]).unwrap();
/// assert_eq!(defaulted.subcommand().unwrap().1.source("jobs"), Some(ValueSource::Default));
///
/// let chosen = app.try_parse_from(["build", "--jobs", "8"]).unwrap();
/// assert_eq!(chosen.subcommand().unwrap().1.source("jobs"), Some(ValueSource::CommandLine));
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ValueSource {
    /// The user wrote it on the command line.
    CommandLine,
    /// It came from the argument's [`env`](crate::Arg::env) variable.
    Environment,
    /// Nobody supplied it, so the argument's [`default`](crate::Arg::default) was
    /// used.
    Default,
}

/// Parsed arguments for one command level.
///
/// Read flags with [`flag`](Matches::flag), counting flags with
/// [`count`](Matches::count), single values with [`value`](Matches::value) or
/// typed with [`get`](Matches::get), repeated and variadic values with
/// [`values`](Matches::values), and descend into an invoked subcommand with
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
/// assert_eq!(build.get::<u32>("jobs"), Some(8));
/// ```
#[derive(Clone, Debug, Default)]
pub struct Matches {
    pub(crate) flags: HashSet<String>,
    pub(crate) counts: HashMap<String, usize>,
    pub(crate) values: HashMap<String, Vec<String>>,
    pub(crate) sources: HashMap<String, ValueSource>,
    pub(crate) subcommand: Option<(String, Box<Matches>)>,
}

impl Matches {
    /// Whether the flag named `name` was set.
    ///
    /// Returns `false` for an unset flag or an unknown name. A
    /// [counting flag](crate::Arg::count) reports `true` once its count reaches
    /// one, so this works for either kind.
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

    /// Whether `name` has a value at all, from any source.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg, Command};
    ///
    /// let mut app = App::new("demo");
    /// app.register(Command::new("run").arg(Arg::option("out")));
    ///
    /// assert!(!app.try_parse_from(["run"]).unwrap().subcommand().unwrap().1.present("out"));
    /// let given = app.try_parse_from(["run", "--out", "f"]).unwrap();
    /// assert!(given.subcommand().unwrap().1.present("out"));
    /// ```
    #[must_use]
    pub fn present(&self, name: &str) -> bool {
        self.values.contains_key(name)
            || self.flags.contains(name)
            || self.counts.contains_key(name)
    }

    /// The value given for an option or positional named `name`, or its fallback.
    ///
    /// Returns `None` if the argument was not provided and has no environment or
    /// default fallback, or if the name is unknown. For a
    /// [`multiple`](crate::Arg::multiple) argument this is the first value; use
    /// [`values`](Matches::values) for all of them.
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
    /// variadic positional; for a single-valued argument it yields its one value.
    /// The iterator is empty for an argument that was not provided or an unknown
    /// name.
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
    pub fn values(&self, name: &str) -> impl Iterator<Item = &str> {
        self.values
            .get(name)
            .into_iter()
            .flatten()
            .map(String::as_str)
    }

    /// Where `name`'s value came from, or `None` if it has none.
    ///
    /// The distinction a sentinel default cannot express: whether to overwrite a
    /// config file's setting (the user asked) or leave it alone (we only fell
    /// back).
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg, Command, ValueSource};
    ///
    /// let mut app = App::new("demo");
    /// app.register(Command::new("build").arg(Arg::option("jobs").default("1")));
    ///
    /// let m = app.try_parse_from(["build", "--jobs", "4"]).unwrap();
    /// let (_, build) = m.subcommand().unwrap();
    /// assert_eq!(build.source("jobs"), Some(ValueSource::CommandLine));
    /// assert_eq!(build.source("nothing"), None);
    /// ```
    #[must_use]
    pub fn source(&self, name: &str) -> Option<ValueSource> {
        self.sources.get(name).copied()
    }

    /// `name`'s value parsed into `T`, or `None` if it is absent or will not
    /// parse.
    ///
    /// Infallible by design: an argument that declared a
    /// [`validator`](crate::Arg::validate) or
    /// [`possible_values`](crate::Arg::possible_values) has already been checked
    /// against the user, so there is nothing left for the program to report.
    /// Where no check was declared, use [`try_get`](Matches::try_get) to see why
    /// a value would not parse.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg, Command};
    ///
    /// let mut app = App::new("serve");
    /// app.register(
    ///     Command::new("start")
    ///         .arg(Arg::option("port").default("8080"))
    ///         .arg(Arg::option("ratio").default("0.5")),
    /// );
    ///
    /// let m = app.try_parse_from(["start"]).unwrap();
    /// let (_, start) = m.subcommand().unwrap();
    /// assert_eq!(start.get::<u16>("port"), Some(8080));
    /// assert_eq!(start.get::<f64>("ratio"), Some(0.5));
    /// assert_eq!(start.get::<u16>("absent"), None);
    /// ```
    #[must_use]
    pub fn get<T: FromStr>(&self, name: &str) -> Option<T> {
        self.value(name)?.parse().ok()
    }

    /// `name`'s value parsed into `T`, reporting why it would not parse.
    ///
    /// `Ok(None)` means the argument was absent. An `Err` carries an
    /// [`ErrorKind::InvalidValue`] naming the value and the reason, ready to be
    /// reported like any other command-line mistake — which is the right shape
    /// for a value the program could not have validated earlier.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg, Command, ErrorKind};
    ///
    /// let mut app = App::new("demo");
    /// app.register(Command::new("wait").arg(Arg::option("seconds")));
    ///
    /// let m = app.try_parse_from(["wait", "--seconds", "soon"]).unwrap();
    /// let (_, wait) = m.subcommand().unwrap();
    ///
    /// let err = wait.try_get::<u32>("seconds").unwrap_err();
    /// assert_eq!(err.kind(), ErrorKind::InvalidValue);
    /// assert_eq!(err.subject(), "soon");
    ///
    /// assert_eq!(wait.try_get::<u32>("absent").unwrap(), None);
    /// ```
    pub fn try_get<T>(&self, name: &str) -> Result<Option<T>, ParseError>
    where
        T: FromStr,
        T::Err: core::fmt::Display,
    {
        let Some(raw) = self.value(name) else {
            return Ok(None);
        };
        raw.parse::<T>().map(Some).map_err(|error| {
            ParseError::new(ErrorKind::InvalidValue, raw)
                .with_detail(crate::shim::format!("{name}: {error}"))
        })
    }

    /// Every value for `name`, each parsed into `T`, skipping any that will not
    /// parse.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg, Command};
    ///
    /// let mut app = App::new("demo");
    /// app.register(Command::new("sum").arg(Arg::positional("n").multiple(true)));
    ///
    /// let m = app.try_parse_from(["sum", "1", "2", "3"]).unwrap();
    /// let numbers: Vec<i64> = m.subcommand().unwrap().1.get_all("n");
    /// assert_eq!(numbers.iter().sum::<i64>(), 6);
    /// ```
    #[must_use]
    pub fn get_all<T: FromStr>(&self, name: &str) -> Vec<T> {
        self.values(name)
            .filter_map(|raw| raw.parse().ok())
            .collect()
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

    /// The invoked subcommand's name, if one was given.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Command};
    ///
    /// let mut app = App::new("demo");
    /// app.register(Command::new("status"));
    ///
    /// assert_eq!(app.try_parse_from(["status"]).unwrap().subcommand_name(), Some("status"));
    ///
    /// // A bare invocation shows the help instead, unless the program turns
    /// // that off — in which case no command was selected.
    /// let quiet = App::new("demo").help_command(false).command(Command::new("status"));
    /// assert_eq!(quiet.try_parse_from([] as [&str; 0]).unwrap().subcommand_name(), None);
    /// ```
    #[must_use]
    pub fn subcommand_name(&self) -> Option<&str> {
        self.subcommand.as_ref().map(|(name, _)| name.as_str())
    }

    /// The chain of command names the invocation resolved to, outermost first.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Command};
    ///
    /// let mut app = App::new("demo");
    /// app.register(Command::new("remote").subcommand(Command::new("add")));
    ///
    /// let m = app.try_parse_from(["remote", "add"]).unwrap();
    /// assert_eq!(m.command_path(), ["remote", "add"]);
    /// ```
    #[must_use]
    pub fn command_path(&self) -> Vec<&str> {
        let mut path = Vec::new();
        let mut level = self;
        while let Some((name, next)) = level.subcommand() {
            path.push(name);
            level = next;
        }
        path
    }

    /// The deepest [`Matches`] in the chain: the one belonging to the command
    /// that will actually run.
    ///
    /// Saves walking `subcommand()` in a loop when a program dispatches itself
    /// rather than attaching handlers.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg, Command};
    ///
    /// let mut app = App::new("demo");
    /// app.register(
    ///     Command::new("remote")
    ///         .subcommand(Command::new("add").arg(Arg::positional("url"))),
    /// );
    ///
    /// let m = app.try_parse_from(["remote", "add", "https://example.com"]).unwrap();
    /// assert_eq!(m.leaf().value("url"), Some("https://example.com"));
    /// ```
    #[must_use]
    pub fn leaf(&self) -> &Matches {
        let mut level = self;
        while let Some((_, next)) = level.subcommand() {
            level = next;
        }
        level
    }

    /// Copy every entry of `globals` into this level and all nested ones.
    ///
    /// A global argument is parsed into one shared slot rather than into
    /// whichever level it happened to be written at, so `-v build`, `build -v`,
    /// and `-v build -v` all mean the same thing. Distributing that slot
    /// afterwards is what makes `m.count("verbose")` answer identically at every
    /// level, so a handler never has to walk back up the tree to find out.
    pub(crate) fn merge_globals(&mut self, globals: &Matches) {
        for name in &globals.flags {
            let _ = self.flags.insert(name.clone());
        }
        for (name, &count) in &globals.counts {
            let _ = self.counts.insert(name.clone(), count);
        }
        for (name, values) in &globals.values {
            let _ = self.values.insert(name.clone(), values.clone());
        }
        for (name, &source) in &globals.sources {
            let _ = self.sources.insert(name.clone(), source);
        }
        if let Some((_, sub)) = self.subcommand.as_mut() {
            sub.merge_globals(globals);
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    // The tests build `Matches` trees by hand, which is what the type's private
    // fields are for; the suggested struct-literal form would be far less
    // readable for a three-level chain.
    #![allow(clippy::field_reassign_with_default)]

    use super::*;

    /// A level holding one option value from the command line.
    fn with_value(name: &str, value: &str) -> Matches {
        let mut m = Matches::default();
        let _ = m.values.insert(name.to_owned(), vec![value.to_owned()]);
        let _ = m.sources.insert(name.to_owned(), ValueSource::CommandLine);
        m
    }

    #[test]
    fn test_absent_and_unknown_names_are_empty_rather_than_an_error() {
        let m = Matches::default();
        assert!(!m.flag("nope"));
        assert_eq!(m.count("nope"), 0);
        assert_eq!(m.value("nope"), None);
        assert_eq!(m.values("nope").count(), 0);
        assert_eq!(m.source("nope"), None);
        assert_eq!(m.get::<u32>("nope"), None);
        assert_eq!(m.get_all::<u32>("nope"), Vec::<u32>::new());
        assert!(!m.present("nope"));
        assert_eq!(m.subcommand_name(), None);
        assert!(m.command_path().is_empty());
    }

    #[test]
    fn test_typed_reads() {
        let m = with_value("port", "8080");
        assert_eq!(m.get::<u16>("port"), Some(8080));
        assert_eq!(m.get::<String>("port").as_deref(), Some("8080"));
        // A value that does not fit the type is `None`, not a panic.
        assert_eq!(m.get::<u8>("port"), None);
    }

    #[test]
    fn test_try_get_explains_a_bad_value() {
        let m = with_value("seconds", "soon");
        let error = m.try_get::<u32>("seconds").unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidValue);
        assert_eq!(error.subject(), "soon");
        assert!(error.detail().unwrap().contains("seconds"));
        // And an absent argument is not an error.
        assert_eq!(m.try_get::<u32>("absent").unwrap(), None);
    }

    #[test]
    fn test_get_all_skips_what_will_not_parse() {
        let mut m = Matches::default();
        let _ = m.values.insert(
            "n".to_owned(),
            vec!["1".to_owned(), "two".to_owned(), "3".to_owned()],
        );
        assert_eq!(m.get_all::<i32>("n"), vec![1, 3]);
    }

    #[test]
    fn test_flag_is_true_for_a_count_that_reached_one() {
        let mut m = Matches::default();
        let _ = m.counts.insert("verbose".to_owned(), 2);
        assert!(m.flag("verbose"));
        assert_eq!(m.count("verbose"), 2);
    }

    #[test]
    fn test_leaf_and_path_walk_the_whole_chain() {
        let mut leaf = with_value("url", "https://example.com");
        let _ = leaf.flags.insert("force".to_owned());

        let mut middle = Matches::default();
        middle.subcommand = Some(("add".to_owned(), Box::new(leaf)));

        let mut root = Matches::default();
        root.subcommand = Some(("remote".to_owned(), Box::new(middle)));

        assert_eq!(root.command_path(), ["remote", "add"]);
        assert_eq!(root.subcommand_name(), Some("remote"));
        assert_eq!(root.leaf().value("url"), Some("https://example.com"));
        assert!(root.leaf().flag("force"));
    }

    #[test]
    fn test_merged_globals_reach_every_level() {
        let mut globals = Matches::default();
        let _ = globals.counts.insert("verbose".to_owned(), 2);
        let _ = globals
            .values
            .insert("color".to_owned(), vec!["never".to_owned()]);
        let _ = globals
            .sources
            .insert("color".to_owned(), ValueSource::CommandLine);
        let _ = globals.flags.insert("quiet".to_owned());

        let mut middle = Matches::default();
        middle.subcommand = Some(("add".to_owned(), Box::new(Matches::default())));
        let mut root = Matches::default();
        root.subcommand = Some(("remote".to_owned(), Box::new(middle)));

        root.merge_globals(&globals);

        for level in [&root, root.subcommand().unwrap().1, root.leaf()] {
            assert_eq!(level.count("verbose"), 2);
            assert_eq!(level.value("color"), Some("never"));
            assert_eq!(level.source("color"), Some(ValueSource::CommandLine));
            assert!(level.flag("quiet"));
        }
    }

    #[test]
    fn test_merging_globals_into_a_single_level_tree_is_harmless() {
        let mut globals = Matches::default();
        let _ = globals.flags.insert("force".to_owned());
        let mut root = Matches::default();
        root.merge_globals(&globals);
        assert!(root.flag("force"));
    }

    #[test]
    fn test_merging_nothing_changes_nothing() {
        let mut root = Matches::default();
        let _ = root.flags.insert("own".to_owned());
        root.merge_globals(&Matches::default());
        assert!(root.flag("own"));
        assert_eq!(root.values.len(), 0);
    }

    #[test]
    fn test_present_covers_every_kind_of_argument() {
        let mut m = Matches::default();
        let _ = m.flags.insert("f".to_owned());
        let _ = m.counts.insert("c".to_owned(), 1);
        let _ = m.values.insert("v".to_owned(), vec!["x".to_owned()]);
        for name in ["f", "c", "v"] {
            assert!(m.present(name), "{name}");
        }
        assert!(!m.present("absent"));
    }
}
