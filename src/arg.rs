//! Argument and flag definitions.
//!
//! An [`Arg`] describes one input a [`Command`](crate::Command) accepts. There
//! are four kinds, each with its own constructor:
//!
//! - [`Arg::flag`] — a boolean switch, `--verbose` / `-v`, present or absent.
//! - [`Arg::count`] — a repeatable flag whose occurrences are counted, `-vvv`.
//! - [`Arg::option`] — a named value, `--output file` / `-o file` / `--output=file`.
//! - [`Arg::positional`] — a bare value identified by position.
//!
//! Options and positionals may be marked [`multiple`](Arg::multiple) to collect
//! every occurrence into a list instead of keeping one value. The builder methods
//! ([`short`](Arg::short), [`long`](Arg::long), [`help`](Arg::help),
//! [`required`](Arg::required), [`default`](Arg::default),
//! [`multiple`](Arg::multiple)) refine the definition and chain. The parser reads
//! these definitions to turn raw tokens into a [`Matches`](crate::Matches).

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

/// A single argument a command accepts.
///
/// Build one with [`Arg::flag`], [`Arg::option`], or [`Arg::positional`], then
/// attach it with [`Command::arg`](crate::Command::arg). The `name` is the key
/// used to read the parsed result back out of a [`Matches`](crate::Matches).
///
/// # Examples
///
/// ```
/// use cli_forge::Arg;
///
/// let verbose = Arg::count("verbose").short('v').help("increase verbosity");
/// let define = Arg::option("define").short('D').multiple(true);
/// let files = Arg::positional("files").multiple(true).required(true);
/// ```
#[derive(Clone, Debug)]
pub struct Arg {
    pub(crate) name: String,
    pub(crate) kind: ArgKind,
    pub(crate) short: Option<char>,
    pub(crate) long: Option<String>,
    pub(crate) help: Option<String>,
    pub(crate) required: bool,
    pub(crate) multiple: bool,
    pub(crate) default: Option<String>,
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
            required: false,
            multiple: false,
            default: None,
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
    #[must_use]
    pub fn short(mut self, short: char) -> Arg {
        self.short = Some(short);
        self
    }

    /// Override the long form (`--name`). Defaults to the argument's name.
    /// Ignored for positionals.
    #[must_use]
    pub fn long(mut self, long: impl Into<String>) -> Arg {
        self.long = Some(long.into());
        self
    }

    /// Attach help text, shown in generated help.
    #[must_use]
    pub fn help(mut self, help: impl Into<String>) -> Arg {
        self.help = Some(help.into());
        self
    }

    /// Require the argument. Parsing fails with
    /// [`ParseError::MissingRequired`](crate::ParseError::MissingRequired) if it
    /// is absent and has no default. Has no effect on flags or counts (they are
    /// simply present or not).
    #[must_use]
    pub fn required(mut self, required: bool) -> Arg {
        self.required = required;
        self
    }

    /// Collect every occurrence into a list instead of keeping a single value.
    ///
    /// For an [`option`](Arg::option), each `--name v` appends a value:
    /// `-D A -D B` yields `["A", "B"]`. For a [`positional`](Arg::positional), it
    /// becomes variadic and absorbs every remaining bare value: `a b c` yields
    /// `["a", "b", "c"]` (put it last). Read the values with
    /// [`Matches::values`](crate::Matches::values). Ignored for flags and counts.
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

    /// Provide a default value used when an option or positional is omitted. A
    /// default makes the argument effectively optional even if
    /// [`required`](Arg::required) was set.
    #[must_use]
    pub fn default(mut self, value: impl Into<String>) -> Arg {
        self.default = Some(value.into());
        self
    }

    /// The long form to match, if any.
    pub(crate) fn long_name(&self) -> Option<&str> {
        self.long.as_deref()
    }
}
