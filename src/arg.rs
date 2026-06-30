//! Argument and flag definitions.
//!
//! An [`Arg`] describes one input a [`Command`](crate::Command) accepts. There
//! are three kinds, each with its own constructor:
//!
//! - [`Arg::flag`] — a boolean switch, `--verbose` / `-v`, present or absent.
//! - [`Arg::option`] — a named value, `--output file` / `-o file` / `--output=file`.
//! - [`Arg::positional`] — a bare value identified by position.
//!
//! The builder methods ([`short`](Arg::short), [`long`](Arg::long),
//! [`help`](Arg::help), [`required`](Arg::required), [`default`](Arg::default))
//! refine the definition and chain. The parser reads these definitions to turn
//! raw tokens into a [`Matches`](crate::Matches).

/// Which form an [`Arg`] takes on the command line.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ArgKind {
    /// A boolean switch that takes no value.
    Flag,
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
/// let verbose = Arg::flag("verbose").short('v').help("print extra detail");
/// let output = Arg::option("output").short('o').required(true);
/// let path = Arg::positional("path").default(".");
/// ```
#[derive(Clone, Debug)]
pub struct Arg {
    pub(crate) name: String,
    pub(crate) kind: ArgKind,
    pub(crate) short: Option<char>,
    pub(crate) long: Option<String>,
    pub(crate) help: Option<String>,
    pub(crate) required: bool,
    pub(crate) default: Option<String>,
}

impl Arg {
    fn new(name: impl Into<String>, kind: ArgKind) -> Arg {
        let name = name.into();
        // Flags and options match `--name` by default; a positional has no long.
        let long = match kind {
            ArgKind::Flag | ArgKind::Option => Some(name.clone()),
            ArgKind::Positional => None,
        };
        Arg {
            name,
            kind,
            short: None,
            long,
            help: None,
            required: false,
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

    /// Define a value-taking option, e.g. `--output file`. Accepts `--name v`,
    /// `--name=v`, `-x v`, and `-xv` at parse time.
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

    /// Attach help text. Surfaced by the help engine (v0.4.0); stored now so
    /// definitions are complete.
    #[must_use]
    pub fn help(mut self, help: impl Into<String>) -> Arg {
        self.help = Some(help.into());
        self
    }

    /// Require the argument. Parsing fails with
    /// [`ParseError::MissingRequired`](crate::ParseError::MissingRequired) if it
    /// is absent and has no default. Has no effect on flags (a flag is simply
    /// present or not).
    #[must_use]
    pub fn required(mut self, required: bool) -> Arg {
        self.required = required;
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
