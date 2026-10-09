//! Argument and flag definitions.
//!
//! An [`Arg`] describes one input a [`Command`](crate::Command) accepts. There
//! are four kinds, each with its own constructor:
//!
//! - [`Arg::flag`] — a boolean switch, `--verbose` / `-v`, present or absent.
//! - [`Arg::count`] — a repeatable switch whose occurrences are counted, `-vvv`.
//! - [`Arg::option`] — a named value, `--output file` / `-o file` / `--output=file`.
//! - [`Arg::positional`] — a bare value identified by position.
//!
//! Everything else is a refinement that chains: where a value may come from
//! ([`default`](Arg::default), [`env`](Arg::env)), what it may be
//! ([`possible_values`](Arg::possible_values), [`validate`](Arg::validate)), how
//! many there may be ([`multiple`](Arg::multiple)), whether it is needed
//! ([`required`](Arg::required), [`required_unless`](Arg::required_unless)), how
//! it relates to other arguments ([`conflicts_with`](Arg::conflicts_with),
//! [`requires`](Arg::requires)), how far it reaches
//! ([`global`](Arg::global)), and how it appears in help
//! ([`help`](Arg::help), [`value_name`](Arg::value_name),
//! [`hide`](Arg::hide)).
//!
//! ## Validate at the edge
//!
//! Declaring what a value may be is worth the line it costs. A validated
//! argument fails with a proper command-line error naming the flag, the bad
//! value, and the acceptable ones — before any of the program's own code runs —
//! instead of panicking three functions deep on an `unwrap`:
//!
//! ```
//! use cli_forge::{App, Arg, Command};
//!
//! let mut app = App::new("serve");
//! app.register(
//!     Command::new("start")
//!         .arg(Arg::option("port").validate(|v| {
//!             v.parse::<u16>().map(|_| ()).map_err(|_| "expected a port number".to_string())
//!         }))
//!         .arg(Arg::option("mode").possible_values(["dev", "prod"])),
//! );
//!
//! let err = app.try_parse_from(["start", "--port", "70000"]).unwrap_err();
//! assert!(err.report().contains("expected a port number"));
//!
//! // And a near miss in a fixed set is corrected rather than merely refused.
//! let typo = app.try_parse_from(["start", "--mode", "prd"]).unwrap_err();
//! assert_eq!(typo.suggestion(), Some("prod"));
//! ```
//!
//! Once an argument is validated, reading it back with
//! [`Matches::get`](crate::Matches::get) cannot fail for reasons the user caused.

use std::sync::Arc;

use crate::error::{ErrorKind, ParseError, did_you_mean};
use crate::shim::String;

/// Which form an [`Arg`] takes on the command line.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ArgKind {
    /// A boolean switch that takes no value.
    Flag,
    /// A repeatable switch whose occurrences are counted.
    Count,
    /// A named argument that takes a value.
    Option,
    /// A value identified by its position.
    Positional,
}

/// A value check, shared rather than cloned so an [`Arg`] stays cheap to copy
/// around the command tree.
type Validator = Arc<dyn Fn(&str) -> Result<(), String> + Send + Sync>;

/// A single argument a command accepts.
///
/// Build one with [`Arg::flag`], [`Arg::count`], [`Arg::option`], or
/// [`Arg::positional`], then attach it with
/// [`Command::arg`](crate::Command::arg). The `name` is the key used to read the
/// parsed result back out of a [`Matches`](crate::Matches).
///
/// # Examples
///
/// ```
/// use cli_forge::Arg;
///
/// let verbose = Arg::count("verbose").short('v').help("increase verbosity");
/// let define = Arg::option("define").short('D').multiple(true).value_name("KEY=VALUE");
/// let files = Arg::positional("files").multiple(true).required(true);
/// let token = Arg::option("token").env("FORGE_TOKEN").hide(true);
/// ```
#[derive(Clone)]
pub struct Arg {
    pub(crate) name: String,
    pub(crate) kind: ArgKind,
    pub(crate) short: Option<char>,
    pub(crate) long: Option<String>,
    pub(crate) help: Option<String>,
    pub(crate) value_name: Option<String>,
    pub(crate) required: bool,
    pub(crate) multiple: bool,
    pub(crate) default: Option<String>,
    pub(crate) env: Option<String>,
    pub(crate) possible: Vec<String>,
    pub(crate) validator: Option<Validator>,
    pub(crate) hidden: bool,
    pub(crate) global: bool,
    pub(crate) conflicts: Vec<String>,
    pub(crate) requires: Vec<String>,
    pub(crate) required_unless: Vec<String>,
    pub(crate) delimiter: Option<char>,
}

impl Arg {
    fn new(name: impl Into<String>, kind: ArgKind) -> Arg {
        let name = name.into();
        // Flags, counts, and options match `--name` by default; a positional has
        // no long form.
        let long = match kind {
            ArgKind::Flag | ArgKind::Count | ArgKind::Option => Some(name.clone()),
            ArgKind::Positional => None,
        };
        Arg {
            name,
            kind,
            short: None,
            long,
            help: None,
            value_name: None,
            required: false,
            multiple: false,
            default: None,
            env: None,
            possible: Vec::new(),
            validator: None,
            hidden: false,
            global: false,
            conflicts: Vec::new(),
            requires: Vec::new(),
            required_unless: Vec::new(),
            delimiter: None,
        }
    }

    /// Define a boolean flag, e.g. `--verbose`. The long form defaults to the
    /// name; add a [`short`](Arg::short) for a one-letter alias.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Arg;
    /// let force = Arg::flag("force").short('f');
    /// ```
    #[must_use]
    pub fn flag(name: impl Into<String>) -> Arg {
        Arg::new(name, ArgKind::Flag)
    }

    /// Define a counting flag: a switch that may be repeated, whose occurrences
    /// are tallied. `-v`, `-vv`, `-vvv` (or `-v -v -v`, or `--verbose --verbose`)
    /// count 1, 2, 3. Read the count with
    /// [`Matches::count`](crate::Matches::count).
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg, Command};
    ///
    /// let mut app = App::new("demo");
    /// app.register(Command::new("run").arg(Arg::count("verbose").short('v')));
    ///
    /// let m = app.try_parse_from(["run", "-vvv"]).unwrap();
    /// assert_eq!(m.subcommand().unwrap().1.count("verbose"), 3);
    /// ```
    #[must_use]
    pub fn count(name: impl Into<String>) -> Arg {
        Arg::new(name, ArgKind::Count)
    }

    /// Define a value-taking option, e.g. `--output file`. Accepts `--name v`,
    /// `--name=v`, `-x v`, and `-xv` at parse time. Mark it
    /// [`multiple`](Arg::multiple) to accept it more than once.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Arg;
    /// let out = Arg::option("output").short('o').required(true);
    /// ```
    #[must_use]
    pub fn option(name: impl Into<String>) -> Arg {
        Arg::new(name, ArgKind::Option)
    }

    /// Define a positional argument, filled by bare values in order.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Arg;
    /// let path = Arg::positional("path").default(".");
    /// ```
    #[must_use]
    pub fn positional(name: impl Into<String>) -> Arg {
        Arg::new(name, ArgKind::Positional)
    }

    /// Set a one-letter short form (`-x`). Ignored for positionals.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Arg;
    /// let jobs = Arg::option("jobs").short('j');
    /// ```
    #[must_use]
    pub fn short(mut self, short: char) -> Arg {
        self.short = Some(short);
        self
    }

    /// Override the long form (`--name`). Defaults to the argument's name, so
    /// this is for the cases where the two should differ — a name that is a Rust
    /// keyword, or a flag spelled differently from the field it fills.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg, Command};
    ///
    /// let mut app = App::new("demo");
    /// app.register(Command::new("run").arg(Arg::flag("use_cache").long("cache")));
    ///
    /// let m = app.try_parse_from(["run", "--cache"]).unwrap();
    /// assert!(m.subcommand().unwrap().1.flag("use_cache"));
    /// ```
    #[must_use]
    pub fn long(mut self, long: impl Into<String>) -> Arg {
        self.long = Some(long.into());
        self
    }

    /// Attach help text, shown in generated help.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Arg;
    /// let jobs = Arg::option("jobs").help("number of parallel jobs");
    /// ```
    #[must_use]
    pub fn help(mut self, help: impl Into<String>) -> Arg {
        self.help = Some(help.into());
        self
    }

    /// Name the value placeholder shown in help and usage, replacing the
    /// upper-cased argument name.
    ///
    /// `--output <FILE>` tells the reader more than `--output <OUTPUT>`.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg, Command};
    ///
    /// let mut app = App::new("demo");
    /// app.register(Command::new("build").arg(Arg::option("output").value_name("FILE")));
    ///
    /// let help = app.command_help(["build"]).unwrap();
    /// assert!(help.contains("--output <FILE>"));
    /// ```
    #[must_use]
    pub fn value_name(mut self, value_name: impl Into<String>) -> Arg {
        self.value_name = Some(value_name.into());
        self
    }

    /// Require the argument. Parsing fails with
    /// [`ErrorKind::MissingRequired`](crate::ErrorKind::MissingRequired) if it is
    /// absent and has no default or environment fallback. Has no effect on flags
    /// or counts, which are simply present or not.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg, Command, ErrorKind};
    ///
    /// let mut app = App::new("demo");
    /// app.register(Command::new("copy").arg(Arg::positional("from").required(true)));
    ///
    /// let err = app.try_parse_from(["copy"]).unwrap_err();
    /// assert_eq!(err.kind(), ErrorKind::MissingRequired);
    /// ```
    #[must_use]
    pub fn required(mut self, required: bool) -> Arg {
        self.required = required;
        self
    }

    /// Require the argument unless one of `others` was provided.
    ///
    /// The shape of every tool that accepts its input one of two ways: a file
    /// path, or `--stdin`. Without this the choice has to be re-checked by hand
    /// after parsing, and the error message is worse.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg, Command};
    ///
    /// let mut app = App::new("demo");
    /// app.register(
    ///     Command::new("read")
    ///         .arg(Arg::positional("file").required(true).required_unless(["stdin"]))
    ///         .arg(Arg::flag("stdin")),
    /// );
    ///
    /// // Either form parses.
    /// assert!(app.try_parse_from(["read", "notes.txt"]).is_ok());
    /// assert!(app.try_parse_from(["read", "--stdin"]).is_ok());
    /// // Neither does not.
    /// assert!(app.try_parse_from(["read"]).is_err());
    /// ```
    #[must_use]
    pub fn required_unless<I, S>(mut self, others: I) -> Arg
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.required_unless
            .extend(others.into_iter().map(Into::into));
        self
    }

    /// Refuse the argument when any of `others` was also provided.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg, Command, ErrorKind};
    ///
    /// let mut app = App::new("demo");
    /// app.register(
    ///     Command::new("log")
    ///         .arg(Arg::flag("quiet").conflicts_with(["verbose"]))
    ///         .arg(Arg::flag("verbose")),
    /// );
    ///
    /// let err = app.try_parse_from(["log", "--quiet", "--verbose"]).unwrap_err();
    /// assert_eq!(err.kind(), ErrorKind::Conflict);
    /// ```
    #[must_use]
    pub fn conflicts_with<I, S>(mut self, others: I) -> Arg
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.conflicts.extend(others.into_iter().map(Into::into));
        self
    }

    /// Require all of `others` whenever this argument is provided.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg, Command, ErrorKind};
    ///
    /// let mut app = App::new("demo");
    /// app.register(
    ///     Command::new("push")
    ///         .arg(Arg::flag("sign").requires(["key"]))
    ///         .arg(Arg::option("key")),
    /// );
    ///
    /// let err = app.try_parse_from(["push", "--sign"]).unwrap_err();
    /// assert_eq!(err.kind(), ErrorKind::MissingDependency);
    /// assert!(app.try_parse_from(["push", "--sign", "--key", "k"]).is_ok());
    /// ```
    #[must_use]
    pub fn requires<I, S>(mut self, others: I) -> Arg
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.requires.extend(others.into_iter().map(Into::into));
        self
    }

    /// Collect every occurrence into a list instead of keeping a single value.
    ///
    /// For an [`option`](Arg::option), each `--name v` appends a value:
    /// `-D A -D B` yields `["A", "B"]`. For a [`positional`](Arg::positional) it
    /// becomes variadic and absorbs every remaining bare value: `a b c` yields
    /// `["a", "b", "c"]` (put it last). Read the values with
    /// [`Matches::values`](crate::Matches::values). Ignored for flags and counts,
    /// which count instead.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg, Command};
    ///
    /// let mut app = App::new("cc");
    /// app.register(
    ///     Command::new("build")
    ///         .arg(Arg::option("include").short('I').multiple(true))
    ///         .arg(Arg::positional("sources").multiple(true)),
    /// );
    ///
    /// let m = app.try_parse_from(["build", "-I", "a", "-I", "b", "x.c", "y.c"]).unwrap();
    /// let (_, build) = m.subcommand().unwrap();
    /// assert_eq!(build.values("include").collect::<Vec<_>>(), ["a", "b"]);
    /// assert_eq!(build.values("sources").collect::<Vec<_>>(), ["x.c", "y.c"]);
    /// ```
    #[must_use]
    pub fn multiple(mut self, multiple: bool) -> Arg {
        self.multiple = multiple;
        self
    }

    /// Split each value on `delimiter`, so one occurrence can carry several.
    ///
    /// `--features a,b,c` yields `["a", "b", "c"]`, which is how most tools
    /// accept a list without making the user repeat the flag. Each piece is
    /// validated on its own, so [`possible_values`](Arg::possible_values) and
    /// [`validate`](Arg::validate) see `a`, `b`, and `c` rather than `a,b,c`.
    /// Repeating the flag still works and accumulates.
    ///
    /// Empty pieces are kept rather than dropped — `a,,b` is three values, the
    /// middle one empty — because silently discarding input hides a typo that a
    /// validator could otherwise catch. A default is split the same way.
    ///
    /// Implies [`multiple`](Arg::multiple): a list in one occurrence is several
    /// values, so they are all kept. Read them with
    /// [`Matches::values`](crate::Matches::values).
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg, Command};
    ///
    /// let mut app = App::new("cargo");
    /// app.register(
    ///     Command::new("build")
    ///         .arg(Arg::option("features").value_delimiter(',')),
    /// );
    ///
    /// let m = app.try_parse_from(["build", "--features", "serde,tokio", "--features", "log"]).unwrap();
    /// assert_eq!(m.leaf().values("features").collect::<Vec<_>>(), ["serde", "tokio", "log"]);
    /// ```
    #[must_use]
    pub fn value_delimiter(mut self, delimiter: char) -> Arg {
        self.delimiter = Some(delimiter);
        self.multiple = true;
        self
    }

    /// Provide a default used when an option or positional is omitted.
    ///
    /// A default makes the argument effectively optional even if
    /// [`required`](Arg::required) was set, and is reported by
    /// [`Matches::source`](crate::Matches::source) as
    /// [`ValueSource::Default`](crate::ValueSource::Default) so a program can
    /// tell "the user chose this" from "nobody said".
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg, Command, ValueSource};
    ///
    /// let mut app = App::new("demo");
    /// app.register(Command::new("build").arg(Arg::option("jobs").default("1")));
    ///
    /// let m = app.try_parse_from(["build"]).unwrap();
    /// let (_, build) = m.subcommand().unwrap();
    /// assert_eq!(build.value("jobs"), Some("1"));
    /// assert_eq!(build.source("jobs"), Some(ValueSource::Default));
    /// ```
    #[must_use]
    pub fn default(mut self, value: impl Into<String>) -> Arg {
        self.default = Some(value.into());
        self
    }

    /// Fall back to the environment variable `name` when the argument is omitted.
    ///
    /// Checked after the command line and before the default, which is the
    /// precedence every tool with both is expected to follow. For a
    /// [`flag`](Arg::flag) or [`count`](Arg::count), any value other than empty
    /// or `0` counts as set.
    ///
    /// This is how a secret stays out of the process list: `--token` is visible
    /// to every user on the machine, and `FORGE_TOKEN` is not.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg, Command};
    ///
    /// let mut app = App::new("demo");
    /// app.register(Command::new("push").arg(Arg::option("token").env("FORGE_TOKEN")));
    ///
    /// // With nothing in the environment and nothing on the command line:
    /// let m = app.try_parse_from(["push"]).unwrap();
    /// assert_eq!(m.subcommand().unwrap().1.value("token"), None);
    /// ```
    #[must_use]
    pub fn env(mut self, name: impl Into<String>) -> Arg {
        self.env = Some(name.into());
        self
    }

    /// Restrict the argument to a fixed set of values.
    ///
    /// The set is shown in help, used to correct a near miss, and checked before
    /// any of the program's code runs.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg, Command, ErrorKind};
    ///
    /// let mut app = App::new("demo");
    /// app.register(
    ///     Command::new("log").arg(Arg::option("level").possible_values(["warn", "info", "debug"])),
    /// );
    ///
    /// assert!(app.try_parse_from(["log", "--level", "info"]).is_ok());
    ///
    /// let err = app.try_parse_from(["log", "--level", "inof"]).unwrap_err();
    /// assert_eq!(err.kind(), ErrorKind::InvalidValue);
    /// assert_eq!(err.suggestion(), Some("info"));
    /// ```
    #[must_use]
    pub fn possible_values<I, S>(mut self, values: I) -> Arg
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.possible.extend(values.into_iter().map(Into::into));
        self
    }

    /// Check each value with `check`, which returns `Err` with an explanation to
    /// reject it.
    ///
    /// The general form of [`possible_values`](Arg::possible_values): a port
    /// range, a parseable number, a well-formed URL. The explanation becomes the
    /// error's [`detail`](crate::ParseError::detail), so it is worth writing as
    /// the sentence the user should read.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg, Command};
    ///
    /// let mut app = App::new("serve");
    /// app.register(Command::new("start").arg(Arg::option("port").validate(|value| {
    ///     match value.parse::<u16>() {
    ///         Ok(port) if port >= 1024 => Ok(()),
    ///         Ok(_) => Err("ports below 1024 need privileges".to_string()),
    ///         Err(_) => Err("expected a number from 1024 to 65535".to_string()),
    ///     }
    /// })));
    ///
    /// assert!(app.try_parse_from(["start", "--port", "8080"]).is_ok());
    /// let err = app.try_parse_from(["start", "--port", "80"]).unwrap_err();
    /// assert!(err.report().contains("need privileges"));
    /// ```
    #[must_use]
    pub fn validate<F>(mut self, check: F) -> Arg
    where
        F: Fn(&str) -> Result<(), String> + Send + Sync + 'static,
    {
        self.validator = Some(Arc::new(check));
        self
    }

    /// Hide the argument from generated help while leaving it usable.
    ///
    /// For an argument that exists but should not be advertised: a debugging
    /// switch, or one kept working for compatibility.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg, Command};
    ///
    /// let mut app = App::new("demo");
    /// app.register(Command::new("run").arg(Arg::flag("dump-ast").hide(true)));
    ///
    /// let help = app.command_help(["run"]).unwrap();
    /// assert!(!help.contains("dump-ast"));
    /// // Still accepted.
    /// assert!(app.try_parse_from(["run", "--dump-ast"]).is_ok());
    /// ```
    #[must_use]
    pub fn hide(mut self, hidden: bool) -> Arg {
        self.hidden = hidden;
        self
    }

    /// Make an app-level argument available to every subcommand too.
    ///
    /// Only meaningful on an argument added with [`App::arg`](crate::App::arg).
    /// A global argument may be written before or after the command name, and
    /// its value appears in the [`Matches`](crate::Matches) at every level, so a
    /// handler reads it without walking back up the tree.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg, Command};
    ///
    /// let mut app = App::new("forge").arg(Arg::count("verbose").short('v').global(true));
    /// app.register(Command::new("build"));
    ///
    /// // Before the command, or after it — both work.
    /// for argv in [["-vv", "build"], ["build", "-vv"]] {
    ///     let m = app.try_parse_from(argv).unwrap();
    ///     assert_eq!(m.count("verbose"), 2);
    ///     assert_eq!(m.subcommand().unwrap().1.count("verbose"), 2);
    /// }
    /// ```
    #[must_use]
    pub fn global(mut self, global: bool) -> Arg {
        self.global = global;
        self
    }

    /// This argument's name: the key its value is read back under.
    ///
    /// The first of the read-only accessors that let a sibling crate generate
    /// shell completions, manual pages, or documentation from a live command
    /// tree. Without them the tree is write-only and every such generator has to
    /// be handed a second, hand-maintained description of the same CLI.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Arg;
    /// assert_eq!(Arg::option("output").name(), "output");
    /// ```
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The one-letter short form, if it has one.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Arg;
    /// assert_eq!(Arg::option("output").short('o').short_form(), Some('o'));
    /// assert_eq!(Arg::option("output").short_form(), None);
    /// ```
    #[must_use]
    pub const fn short_form(&self) -> Option<char> {
        self.short
    }

    /// The long form, if it has one. Positionals do not.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Arg;
    /// assert_eq!(Arg::flag("force").long_form(), Some("force"));
    /// assert_eq!(Arg::positional("path").long_form(), None);
    /// ```
    #[must_use]
    pub fn long_form(&self) -> Option<&str> {
        self.long.as_deref()
    }

    /// The help text, if any.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Arg;
    /// assert_eq!(Arg::flag("f").help("force it").help_text(), Some("force it"));
    /// ```
    #[must_use]
    pub fn help_text(&self) -> Option<&str> {
        self.help.as_deref()
    }

    /// The default value, if any.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Arg;
    /// assert_eq!(Arg::option("jobs").default("1").default_value(), Some("1"));
    /// ```
    #[must_use]
    pub fn default_value(&self) -> Option<&str> {
        self.default.as_deref()
    }

    /// The environment variable consulted as a fallback, if any.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Arg;
    /// assert_eq!(Arg::option("t").env("FORGE_TOKEN").env_var(), Some("FORGE_TOKEN"));
    /// ```
    #[must_use]
    pub fn env_var(&self) -> Option<&str> {
        self.env.as_deref()
    }

    /// The fixed set of acceptable values, empty when unrestricted.
    ///
    /// Exactly what a completion generator needs in order to complete an
    /// option's values rather than only its name.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Arg;
    ///
    /// let level = Arg::option("level").possible_values(["warn", "info"]);
    /// assert_eq!(level.allowed_values(), ["warn", "info"]);
    /// assert!(Arg::option("message").allowed_values().is_empty());
    /// ```
    #[must_use]
    pub fn allowed_values(&self) -> &[String] {
        &self.possible
    }

    /// Whether this argument must be provided.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Arg;
    /// assert!(Arg::positional("from").required(true).is_required());
    /// ```
    #[must_use]
    pub const fn is_required(&self) -> bool {
        self.required
    }

    /// Whether this argument collects every occurrence.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Arg;
    /// assert!(Arg::option("define").multiple(true).is_multiple());
    /// ```
    #[must_use]
    pub const fn is_multiple(&self) -> bool {
        self.multiple
    }

    /// The character values are split on, if any.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Arg;
    /// assert_eq!(Arg::option("features").value_delimiter(',').delimiter(), Some(','));
    /// ```
    #[must_use]
    pub const fn delimiter(&self) -> Option<char> {
        self.delimiter
    }

    /// Whether this argument is hidden from generated help.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Arg;
    /// assert!(Arg::flag("dump").hide(true).is_hidden());
    /// ```
    #[must_use]
    pub const fn is_hidden(&self) -> bool {
        self.hidden
    }

    /// Whether this app-level argument is inherited by every subcommand.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Arg;
    /// assert!(Arg::count("verbose").global(true).is_global());
    /// ```
    #[must_use]
    pub const fn is_global(&self) -> bool {
        self.global
    }

    /// Whether this argument expects a value after it on the command line.
    ///
    /// Flags and counting flags do not; options and positionals do. A completion
    /// generator needs this to know whether to offer a value next.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Arg;
    ///
    /// assert!(Arg::option("output").expects_value());
    /// assert!(Arg::positional("path").expects_value());
    /// assert!(!Arg::flag("force").expects_value());
    /// assert!(!Arg::count("verbose").expects_value());
    /// ```
    #[must_use]
    pub const fn expects_value(&self) -> bool {
        matches!(self.kind, ArgKind::Option | ArgKind::Positional)
    }

    /// Whether this argument is filled by position rather than by name.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Arg;
    /// assert!(Arg::positional("path").is_positional());
    /// assert!(!Arg::option("output").is_positional());
    /// ```
    #[must_use]
    pub const fn is_positional(&self) -> bool {
        matches!(self.kind, ArgKind::Positional)
    }

    /// The long form to match, if any.
    pub(crate) fn long_name(&self) -> Option<&str> {
        self.long.as_deref()
    }

    /// The placeholder shown for an option's value, upper-cased by convention:
    /// `--output <FILE>`.
    pub(crate) fn placeholder(&self) -> String {
        match &self.value_name {
            Some(name) => name.clone(),
            None => self.name.to_uppercase(),
        }
    }

    /// The placeholder shown for a positional's slot, which convention leaves in
    /// lower case: `<path>` rather than `<PATH>`.
    pub(crate) fn placeholder_lower(&self) -> String {
        match &self.value_name {
            Some(name) => name.clone(),
            None => self.name.clone(),
        }
    }

    /// Check `value` against the allowed set and the validator.
    ///
    /// `written` is how the user named this argument, so the error says what they
    /// wrote rather than the internal name. It is passed unformatted because the
    /// happy path is every value that parses: building the display form eagerly
    /// meant a string allocation per flag for a message almost never shown.
    pub(crate) fn check(&self, value: &str, written: Written<'_>) -> Result<(), ParseError> {
        if !self.possible.is_empty() && !self.possible.iter().any(|allowed| allowed == value) {
            let allowed = self.possible.join(", ");
            let mut error = ParseError::new(ErrorKind::InvalidValue, value).with_detail(
                crate::shim::format!("'{}' accepts one of: {allowed}", written.display()),
            );
            if let Some(nearest) = did_you_mean(value, self.possible.iter().map(String::as_str)) {
                error = error.with_suggestion(nearest);
            }
            return Err(error);
        }
        if let Some(validator) = &self.validator {
            if let Err(complaint) = validator(value) {
                return Err(ParseError::new(ErrorKind::InvalidValue, value)
                    .with_detail(crate::shim::format!("{}: {complaint}", written.display())));
            }
        }
        Ok(())
    }
}

/// How the user named an argument, for an error message.
///
/// Carried unformatted so that the common case — a value that is fine — costs
/// nothing. Only the failing branch builds a string.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Written<'a> {
    /// A `--long` flag, named without its dashes.
    Long(&'a str),
    /// A `-s` flag.
    Short(char),
    /// A positional, or a default, named by the argument's own name.
    Name(&'a str),
    /// An environment variable, named without its sigil.
    Env(&'a str),
}

impl Written<'_> {
    /// The form to show the user.
    pub(crate) fn display(self) -> crate::shim::String {
        match self {
            Written::Long(name) => crate::shim::format!("--{name}"),
            Written::Short(c) => crate::shim::format!("-{c}"),
            Written::Name(name) => crate::shim::ToString::to_string(name),
            Written::Env(name) => crate::shim::format!("${name}"),
        }
    }
}

impl core::fmt::Debug for Arg {
    /// A validator is a closure and cannot be shown, so its presence is reported
    /// instead — which is the part that matters when reading a dump.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut s = f.debug_struct("Arg");
        let _ = s.field("name", &self.name);
        let _ = s.field("kind", &self.kind);
        let _ = s.field("short", &self.short);
        let _ = s.field("long", &self.long);
        let _ = s.field("help", &self.help);
        let _ = s.field("value_name", &self.value_name);
        let _ = s.field("required", &self.required);
        let _ = s.field("multiple", &self.multiple);
        let _ = s.field("default", &self.default);
        let _ = s.field("env", &self.env);
        let _ = s.field("possible", &self.possible);
        let _ = s.field("has_validator", &self.validator.is_some());
        let _ = s.field("hidden", &self.hidden);
        let _ = s.field("global", &self.global);
        let _ = s.field("conflicts", &self.conflicts);
        let _ = s.field("requires", &self.requires);
        let _ = s.field("required_unless", &self.required_unless);
        let _ = s.field("delimiter", &self.delimiter);
        s.finish()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn test_long_form_defaults_to_the_name_except_for_positionals() {
        assert_eq!(Arg::flag("force").long_name(), Some("force"));
        assert_eq!(Arg::count("verbose").long_name(), Some("verbose"));
        assert_eq!(Arg::option("output").long_name(), Some("output"));
        assert_eq!(Arg::positional("path").long_name(), None);
        assert_eq!(
            Arg::flag("use_cache").long("cache").long_name(),
            Some("cache")
        );
    }

    #[test]
    fn test_expects_value_matches_the_kind() {
        assert!(!Arg::flag("f").expects_value());
        assert!(!Arg::count("c").expects_value());
        assert!(Arg::option("o").expects_value());
        assert!(Arg::positional("p").expects_value());
    }

    #[test]
    fn test_placeholder_prefers_an_explicit_value_name() {
        assert_eq!(Arg::option("output").placeholder(), "OUTPUT");
        assert_eq!(
            Arg::option("output").value_name("FILE").placeholder(),
            "FILE"
        );
    }

    #[test]
    fn test_possible_values_accepts_members_and_rejects_others() {
        let arg = Arg::option("level").possible_values(["warn", "info"]);
        assert!(arg.check("warn", Written::Long("level")).is_ok());
        assert!(arg.check("info", Written::Long("level")).is_ok());

        let error = arg.check("trace", Written::Long("level")).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidValue);
        assert_eq!(error.subject(), "trace");
        // The report names the flag as written and the whole allowed set.
        let detail = error.detail().unwrap();
        assert!(detail.contains("--level"));
        assert!(detail.contains("warn, info"));
    }

    #[test]
    fn test_a_near_miss_in_a_fixed_set_is_corrected() {
        let arg = Arg::option("level").possible_values(["warn", "info", "debug"]);
        assert_eq!(
            arg.check("inof", Written::Long("level"))
                .unwrap_err()
                .suggestion(),
            Some("info")
        );
        // Nothing close enough gets no suggestion rather than a misleading one.
        assert_eq!(
            arg.check("xyzzy", Written::Long("level"))
                .unwrap_err()
                .suggestion(),
            None
        );
    }

    #[test]
    fn test_validator_complaint_reaches_the_report() {
        let arg = Arg::option("port").validate(|value| {
            value
                .parse::<u16>()
                .map(|_| ())
                .map_err(|_| "expected a port number".to_string())
        });
        assert!(arg.check("8080", Written::Long("port")).is_ok());

        let error = arg.check("http", Written::Long("port")).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidValue);
        assert!(error.detail().unwrap().contains("expected a port number"));
        assert!(error.report().contains("--port"));
    }

    #[test]
    fn test_possible_values_are_checked_before_the_validator() {
        // Otherwise a validator would have to repeat the set membership check.
        let arg = Arg::option("mode")
            .possible_values(["fast"])
            .validate(|_| Err("validator should not run".to_string()));
        let error = arg.check("slow", Written::Long("mode")).unwrap_err();
        assert!(error.detail().unwrap().contains("accepts one of"));
    }

    #[test]
    fn test_an_unconstrained_argument_accepts_anything() {
        let arg = Arg::option("message");
        for value in ["", "anything", "--looks-like-a-flag", "日本語"] {
            assert!(
                arg.check(value, Written::Long("message")).is_ok(),
                "{value:?}"
            );
        }
    }

    #[test]
    fn test_relationships_accumulate_rather_than_replace() {
        let arg = Arg::flag("a")
            .conflicts_with(["b"])
            .conflicts_with(["c"])
            .requires(["d"])
            .requires(["e"])
            .required_unless(["f"])
            .required_unless(["g"]);
        assert_eq!(arg.conflicts, ["b", "c"]);
        assert_eq!(arg.requires, ["d", "e"]);
        assert_eq!(arg.required_unless, ["f", "g"]);
    }

    #[test]
    fn test_debug_reports_the_validator_without_trying_to_show_it() {
        let shown = crate::shim::format!("{:?}", Arg::option("p").validate(|_| Ok(())));
        assert!(shown.contains("has_validator: true"));
        assert!(crate::shim::format!("{:?}", Arg::option("p")).contains("has_validator: false"));
    }

    #[test]
    fn test_an_arg_stays_cheap_to_clone_with_a_validator_attached() {
        // The validator is shared, not duplicated, so cloning the command tree
        // for help rendering cannot be expensive.
        let arg = Arg::option("p").validate(|_| Ok(()));
        let copy = arg.clone();
        assert!(copy.validator.is_some());
        assert!(Arc::ptr_eq(
            arg.validator.as_ref().unwrap(),
            copy.validator.as_ref().unwrap()
        ));
    }
}
