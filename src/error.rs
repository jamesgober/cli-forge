//! Errors, and the reports they render into.
//!
//! Two different things can go wrong, and conflating them is a common mistake in
//! CLI libraries:
//!
//! - A [`ParseError`] means the *invocation* was wrong — an unknown flag, a
//!   missing value, a value outside the allowed set. The program never ran. The
//!   right response is to explain the mistake, suggest the fix, show the usage
//!   line, and exit `2`.
//! - A [`CommandError`] means the invocation was fine and the *work* failed. The
//!   right response is to print the message and exit with a code the program
//!   chose.
//!
//! A `ParseError` carries more than a message. It knows what it was about, what
//! would have been valid, the nearest spelling to what the user typed, and the
//! usage line for the command they were invoking — so the report answers "what
//! now?" rather than only "what happened":
//!
//! ```text
//! error: unknown flag '--verbsoe'
//!
//!   did you mean '--verbose'?
//!
//! USAGE: forge build [options] [targets]...
//! ```
//!
//! Nothing here panics, and nothing here prints: a `ParseError` is a value the
//! caller decides what to do with. [`App::parse`](crate::App::parse) and
//! [`App::run`](crate::App::run) are the ones that print and exit.

use std::error::Error;
use std::fmt;

use crate::shim::{String, Vec};
use crate::terminal::Stream;
use crate::theme::{Level, Theme};

/// The exit status a failed invocation produces, following the convention that
/// `2` means "the command line was wrong" and `1` means "the command failed".
pub(crate) const USAGE_EXIT_CODE: i32 = 2;

/// What kind of thing went wrong while parsing an invocation.
///
/// Marked `#[non_exhaustive]` so later versions can distinguish new cases
/// without a breaking change. Match on it when a program needs to react
/// differently to different mistakes; most programs only need
/// [`ParseError::report`].
///
/// # Examples
///
/// ```
/// use cli_forge::{App, Arg, Command, ErrorKind};
///
/// let mut app = App::new("demo");
/// app.register(Command::new("build").arg(Arg::option("jobs")));
///
/// let err = app.try_parse_from(["build", "--bogus"]).unwrap_err();
/// assert_eq!(err.kind(), ErrorKind::UnknownFlag);
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
pub enum ErrorKind {
    /// A `-x` / `--name` was given that nothing at this level declares.
    UnknownFlag,
    /// An option that takes a value was given without one.
    MissingValue,
    /// A required argument was not provided.
    MissingRequired,
    /// A command name was expected but nothing matched.
    UnknownCommand,
    /// A bare value was given that no positional or subcommand can accept.
    UnexpectedArgument,
    /// A value was provided that the argument rejects — outside its allowed set,
    /// or refused by its validator.
    InvalidValue,
    /// Two arguments were given that cannot be used together.
    Conflict,
    /// An argument was given that requires another which was not.
    MissingDependency,
    /// A subcommand is required and none was given.
    MissingSubcommand,
    /// An argument was not valid UTF-8, so it cannot be interpreted.
    NonUtf8,
    /// An auth-gated command was invoked without authorization.
    Unauthorized,
    /// Not an error: `-h` / `--help` was requested.
    HelpRequested,
    /// Not an error: `-V` / `--version` was requested.
    VersionRequested,
}

impl ErrorKind {
    /// Whether this is a request for information rather than a mistake.
    ///
    /// Help and version are reported through the same channel as errors —
    /// parsing stops either way — but they are successes, and a program that
    /// treats them as failures exits non-zero for `--help`, which breaks every
    /// script that checks the status.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::ErrorKind;
    ///
    /// assert!(ErrorKind::HelpRequested.is_request());
    /// assert!(!ErrorKind::UnknownFlag.is_request());
    /// ```
    #[must_use]
    pub const fn is_request(self) -> bool {
        matches!(self, ErrorKind::HelpRequested | ErrorKind::VersionRequested)
    }
}

/// A command line that could not be parsed, or a request to print help.
///
/// Returned by [`App::try_parse_from`](crate::App::try_parse_from). Construct
/// nothing by hand: read it with [`kind`](ParseError::kind) and the other
/// accessors, or render it with [`report`](ParseError::report).
///
/// # Examples
///
/// ```
/// use cli_forge::{App, Arg, Command, ErrorKind};
///
/// let mut app = App::new("forge");
/// app.register(Command::new("build").arg(Arg::flag("release")));
///
/// let err = app.try_parse_from(["build", "--releaze"]).unwrap_err();
/// assert_eq!(err.kind(), ErrorKind::UnknownFlag);
/// assert_eq!(err.subject(), "--releaze");
/// // The report names the nearest spelling rather than only refusing.
/// assert!(err.report().contains("--release"));
/// assert_eq!(err.exit_code(), 2);
/// ```
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ParseError {
    /// Boxed so that `Result<Matches, ParseError>` stays small. Six string
    /// fields is a lot to carry through every successful parse for the sake of
    /// the failing one, and an error path is cold enough to afford the
    /// allocation.
    inner: Box<Report>,
}

/// A [`ParseError`]'s contents, behind one allocation.
#[derive(Clone, PartialEq, Eq, Debug)]
struct Report {
    kind: ErrorKind,
    subject: String,
    detail: Option<String>,
    suggestion: Option<String>,
    usage: Option<String>,
    /// The rendered help or version text, for the two request kinds.
    payload: Option<String>,
    /// A specific one-line message replacing the kind's generic one, for the
    /// cases where the generic wording would mislead.
    headline: Option<String>,
}

impl ParseError {
    /// Build an error of `kind` about `subject`.
    pub(crate) fn new(kind: ErrorKind, subject: impl Into<String>) -> ParseError {
        ParseError {
            inner: Box::new(Report {
                kind,
                subject: subject.into(),
                detail: None,
                suggestion: None,
                usage: None,
                payload: None,
                headline: None,
            }),
        }
    }

    /// Build a help or version request carrying its rendered text.
    pub(crate) fn request(kind: ErrorKind, text: impl Into<String>) -> ParseError {
        let mut error = ParseError::new(kind, "");
        error.inner.payload = Some(text.into());
        error
    }

    /// Attach the explanation of what would have been acceptable.
    pub(crate) fn with_detail(mut self, detail: impl Into<String>) -> ParseError {
        self.inner.detail = Some(detail.into());
        self
    }

    /// Replace the kind's generic one-line message with a specific one.
    ///
    /// For an error whose subject is not something the user typed — a group's
    /// name — or whose generic wording says less than the specific facts do.
    pub(crate) fn with_headline(mut self, headline: impl Into<String>) -> ParseError {
        self.inner.headline = Some(headline.into());
        self
    }

    /// Attach the nearest valid spelling to what the user typed.
    pub(crate) fn with_suggestion(mut self, suggestion: impl Into<String>) -> ParseError {
        self.inner.suggestion = Some(suggestion.into());
        self
    }

    /// Attach the usage line of the command being invoked, unless one is already
    /// set — the innermost command to add it is the most specific, so the first
    /// wins as the error travels outward.
    pub(crate) fn with_usage(mut self, usage: impl Into<String>) -> ParseError {
        if self.inner.usage.is_none() {
            self.inner.usage = Some(usage.into());
        }
        self
    }

    /// What kind of mistake this is.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Command, ErrorKind};
    ///
    /// let app = App::new("demo");
    /// let err = app.try_parse_from(["nope"]).unwrap_err();
    /// assert_eq!(err.kind(), ErrorKind::UnknownCommand);
    /// ```
    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        self.inner.kind
    }

    /// What the error is about: the flag as the user wrote it, the argument's
    /// name, the command name, or the offending value.
    ///
    /// Empty for [`HelpRequested`](ErrorKind::HelpRequested) and
    /// [`VersionRequested`](ErrorKind::VersionRequested), which are about nothing.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Command};
    ///
    /// let app = App::new("demo");
    /// let err = app.try_parse_from(["buidl"]).unwrap_err();
    /// assert_eq!(err.subject(), "buidl");
    /// ```
    #[must_use]
    pub fn subject(&self) -> &str {
        &self.inner.subject
    }

    /// What would have been acceptable instead, when that can be stated: the
    /// allowed values, or a validator's complaint.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg, Command};
    ///
    /// let mut app = App::new("demo");
    /// app.register(
    ///     Command::new("log").arg(Arg::option("level").possible_values(["warn", "info"])),
    /// );
    ///
    /// let err = app.try_parse_from(["log", "--level", "trace"]).unwrap_err();
    /// assert!(err.detail().unwrap().contains("warn"));
    /// ```
    #[must_use]
    pub fn detail(&self) -> Option<&str> {
        self.inner.detail.as_deref()
    }

    /// The nearest valid spelling to what the user typed, when one is close
    /// enough to be worth offering.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Command};
    ///
    /// let mut app = App::new("demo");
    /// app.register(Command::new("build"));
    ///
    /// let err = app.try_parse_from(["buidl"]).unwrap_err();
    /// assert_eq!(err.suggestion(), Some("build"));
    ///
    /// // Nothing is suggested when nothing is close.
    /// let far = app.try_parse_from(["xyzzy"]).unwrap_err();
    /// assert_eq!(far.suggestion(), None);
    /// ```
    #[must_use]
    pub fn suggestion(&self) -> Option<&str> {
        self.inner.suggestion.as_deref()
    }

    /// The usage line of the command being invoked, when the error happened
    /// somewhere specific enough to have one.
    #[must_use]
    pub fn usage(&self) -> Option<&str> {
        self.inner.usage.as_deref()
    }

    /// The rendered help or version text, for the two request kinds.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Command, ErrorKind};
    ///
    /// let mut app = App::new("demo");
    /// app.register(Command::new("build"));
    ///
    /// let err = app.try_parse_from(["--help"]).unwrap_err();
    /// assert_eq!(err.kind(), ErrorKind::HelpRequested);
    /// assert!(err.text().unwrap().contains("build"));
    /// ```
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        self.inner.payload.as_deref()
    }

    /// Whether this is a help or version request rather than a mistake.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Command};
    ///
    /// let mut app = App::new("demo");
    /// app.register(Command::new("build"));
    ///
    /// assert!(app.try_parse_from(["--help"]).unwrap_err().is_request());
    /// assert!(!app.try_parse_from(["nope"]).unwrap_err().is_request());
    /// ```
    #[must_use]
    pub fn is_request(&self) -> bool {
        self.inner.kind.is_request()
    }

    /// The process exit status this outcome should produce: `0` for a help or
    /// version request, `2` for a bad command line.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Command};
    ///
    /// let mut app = App::new("demo");
    /// app.register(Command::new("build"));
    ///
    /// assert_eq!(app.try_parse_from(["--help"]).unwrap_err().exit_code(), 0);
    /// assert_eq!(app.try_parse_from(["nope"]).unwrap_err().exit_code(), 2);
    /// ```
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        if self.inner.kind.is_request() {
            0
        } else {
            USAGE_EXIT_CODE
        }
    }

    /// Which stream this outcome belongs on: requested information goes to
    /// standard output, mistakes to standard error.
    ///
    /// Getting this wrong is why some tools cannot have their `--help` piped.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Command, Stream};
    ///
    /// let mut app = App::new("demo");
    /// app.register(Command::new("build"));
    ///
    /// assert_eq!(app.try_parse_from(["--help"]).unwrap_err().stream(), Stream::Stdout);
    /// assert_eq!(app.try_parse_from(["nope"]).unwrap_err().stream(), Stream::Stderr);
    /// ```
    #[must_use]
    pub fn stream(&self) -> Stream {
        if self.inner.kind.is_request() {
            Stream::Stdout
        } else {
            Stream::Stderr
        }
    }

    /// The full plain-text report: the message, the suggestion, and the usage
    /// line.
    ///
    /// For a help or version request this is the requested text itself, so a
    /// caller can print the report unconditionally and get the right thing
    /// either way.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg, Command};
    ///
    /// let mut app = App::new("forge");
    /// app.register(Command::new("build").arg(Arg::flag("release")));
    ///
    /// let report = app.try_parse_from(["build", "--releaze"]).unwrap_err().report();
    /// assert!(report.starts_with("error: unknown flag"));
    /// assert!(report.contains("did you mean '--release'?"));
    /// assert!(report.contains("USAGE: forge build"));
    /// ```
    #[must_use]
    pub fn report(&self) -> String {
        self.render(&Theme::plain(), crate::terminal::ColorLevel::None)
    }

    /// The full report, styled through the process theme at the depth detected
    /// for the stream it is going to.
    ///
    /// This is what [`App::parse`](crate::App::parse) prints.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Command};
    ///
    /// let mut app = App::new("demo");
    /// app.register(Command::new("build"));
    ///
    /// let err = app.try_parse_from(["nope"]).unwrap_err();
    /// // Styled or not depending on the terminal, but always containing the text.
    /// assert!(err.styled_report().contains("unknown command"));
    /// ```
    #[cfg(feature = "std")]
    #[must_use]
    pub fn styled_report(&self) -> String {
        self.render(&Theme::current(), crate::terminal::level(self.stream()))
    }

    /// Render the report through `theme` at `depth`.
    ///
    /// The label is styled but carries no glyph: `error: ...` is the convention
    /// the whole Rust toolchain uses, and a marker as well would only repeat it.
    fn render(&self, theme: &Theme, depth: crate::terminal::ColorLevel) -> String {
        // A request is the text it carries; there is no error to decorate.
        if let Some(text) = &self.inner.payload {
            return text.clone();
        }

        let mut out = String::new();
        out.push_str(&crate::shim::format!(
            "{}",
            theme.style(Level::Error).paint_at("error:", depth)
        ));
        out.push(' ');
        out.push_str(&self.message());

        if let Some(detail) = &self.inner.detail {
            out.push_str("\n\n  ");
            out.push_str(detail);
        }
        if let Some(suggestion) = &self.inner.suggestion {
            let hint = crate::shim::format!("did you mean '{suggestion}'?");
            out.push_str("\n\n  ");
            out.push_str(&crate::shim::format!(
                "{}",
                theme.style(Level::Hint).paint_at(&hint, depth)
            ));
        }
        if let Some(usage) = &self.inner.usage {
            out.push_str("\n\n");
            out.push_str(usage);
        }
        out
    }

    /// The one-line description of what went wrong, without decoration.
    fn message(&self) -> String {
        if let Some(headline) = &self.inner.headline {
            return headline.clone();
        }
        let subject = &self.inner.subject;
        match self.inner.kind {
            ErrorKind::UnknownFlag => crate::shim::format!("unknown flag '{subject}'"),
            ErrorKind::MissingValue => {
                crate::shim::format!("the option '{subject}' needs a value")
            }
            ErrorKind::MissingRequired => {
                crate::shim::format!("the required argument '{subject}' was not provided")
            }
            ErrorKind::UnknownCommand => crate::shim::format!("unknown command '{subject}'"),
            ErrorKind::UnexpectedArgument => {
                crate::shim::format!("unexpected argument '{subject}'")
            }
            ErrorKind::InvalidValue => crate::shim::format!("invalid value '{subject}'"),
            ErrorKind::Conflict => {
                crate::shim::format!(
                    "'{subject}' cannot be used with the arguments it conflicts with"
                )
            }
            ErrorKind::MissingDependency => {
                crate::shim::format!("'{subject}' requires another argument that was not provided")
            }
            ErrorKind::MissingSubcommand => {
                crate::shim::format!("'{subject}' needs a subcommand")
            }
            ErrorKind::NonUtf8 => {
                crate::shim::format!("argument '{subject}' is not valid UTF-8")
            }
            ErrorKind::Unauthorized => {
                crate::shim::format!("not authorized to run '{subject}'")
            }
            // Handled by `render` before it reaches here; an empty message is
            // better than a wrong one if it ever does.
            ErrorKind::HelpRequested | ErrorKind::VersionRequested => String::new(),
        }
    }
}

impl fmt::Display for ParseError {
    /// The one-line message, so `{err}` reads well inside a larger sentence. Use
    /// [`report`](ParseError::report) for the full multi-line form.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner.payload {
            Some(text) => f.write_str(text),
            None => f.write_str(&self.message()),
        }
    }
}

impl Error for ParseError {}

/// A command's work failed.
///
/// Produced by returning an `Err` from a [`run`](crate::Command::run) handler.
/// Any error type that implements [`Display`](fmt::Display) converts into one,
/// so handlers can return `io::Result`, a `Box<dyn Error>`, or whatever the rest
/// of the program already uses.
///
/// # Examples
///
/// ```
/// use cli_forge::{App, Command};
///
/// let mut app = App::new("demo");
/// app.register(Command::new("deploy").run(|_| Err("no credentials configured")));
///
/// let outcome = app.try_run_from(["deploy"]).unwrap();
/// let failure = outcome.unwrap_err();
/// assert_eq!(failure.message(), "no credentials configured");
/// assert_eq!(failure.exit_code(), 1);
/// ```
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CommandError {
    message: String,
    code: i32,
}

impl CommandError {
    /// The default exit status for a command that failed.
    const DEFAULT_CODE: i32 = 1;

    /// Describe a failure that should exit `1`.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::CommandError;
    ///
    /// let err = CommandError::new("the server refused the connection");
    /// assert_eq!(err.exit_code(), 1);
    /// ```
    #[must_use]
    pub fn new(message: impl Into<String>) -> CommandError {
        CommandError {
            message: message.into(),
            code: CommandError::DEFAULT_CODE,
        }
    }

    /// Set the process exit status this failure produces.
    ///
    /// For the many tools whose exit codes are part of their contract — `diff`
    /// exiting `1` for "different" and `2` for "trouble", `grep` the same.
    ///
    /// The status is carried by
    /// [`Command::run_status`](crate::Command::run_status), whose signature names
    /// this type exactly. A handler attached with the general
    /// [`run`](crate::Command::run) reports every failure as `1`, because the
    /// error it accepts is only required to be printable.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Command, CommandError};
    ///
    /// let mut app = App::new("demo");
    /// app.register(
    ///     Command::new("diff")
    ///         .run_status(|_| Err(CommandError::new("files differ").with_code(2))),
    /// );
    ///
    /// let failure = app.try_run_from(["diff"]).unwrap().unwrap_err();
    /// assert_eq!(failure.exit_code(), 2);
    /// ```
    #[must_use]
    pub fn with_code(mut self, code: i32) -> CommandError {
        self.code = code;
        self
    }

    /// What went wrong.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The process exit status this failure should produce.
    #[must_use]
    pub const fn exit_code(&self) -> i32 {
        self.code
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for CommandError {}

/// What a [`run`](crate::Command::run) handler may return.
///
/// Implemented for `()` and for `Result<(), E>` where `E` is anything printable,
/// so a handler can ignore failure or report it without the signature forcing a
/// choice — and `?` works inside it against whatever error type the rest of the
/// program already uses:
///
/// ```
/// use cli_forge::{App, Command, CommandError};
///
/// let mut app = App::new("demo");
///
/// // Nothing to report.
/// app.register(Command::new("ping").run(|_| {}));
///
/// // A plain message.
/// app.register(Command::new("pull").run(|_| Err("not a repository")));
///
/// // Any existing error type, so `?` works.
/// app.register(Command::new("read").run(|_| {
///     std::fs::read_to_string("/nonexistent").map(|_| ())
/// }));
///
/// // An exact exit status needs the concrete signature.
/// app.register(Command::new("diff").run_status(|_| {
///     Err(CommandError::new("files differ").with_code(2))
/// }));
/// ```
pub trait Outcome {
    /// Convert into the uniform result the dispatcher works with.
    fn into_outcome(self) -> Result<(), CommandError>;
}

impl Outcome for () {
    fn into_outcome(self) -> Result<(), CommandError> {
        Ok(())
    }
}

impl<E: fmt::Display> Outcome for Result<(), E> {
    /// Any printable error becomes a failure at the conventional status `1`.
    ///
    /// An exact status needs [`Command::run_status`](crate::Command::run_status),
    /// whose signature is concrete enough to carry one.
    fn into_outcome(self) -> Result<(), CommandError> {
        self.map_err(|error| CommandError::new(crate::shim::format!("{error}")))
    }
}

/// The closest of `candidates` to `typed`, if any is close enough to suggest.
///
/// Uses edit distance with a threshold that scales with length: a one-character
/// slip in a short word is worth catching, while two unrelated five-letter words
/// are not "nearly" each other. Suggesting something wrong is worse than
/// suggesting nothing, because the user will try it.
pub(crate) fn did_you_mean<'a, I>(typed: &str, candidates: I) -> Option<&'a str>
where
    I: IntoIterator<Item = &'a str>,
{
    if typed.is_empty() {
        return None;
    }
    let typed_lower = typed.to_lowercase();
    let budget = 1 + typed.chars().count() / 3;

    let mut best: Option<(usize, &str)> = None;
    for candidate in candidates {
        if candidate.is_empty() {
            continue;
        }
        let distance = edit_distance(&typed_lower, &candidate.to_lowercase());
        if distance > budget {
            continue;
        }
        if best.is_none_or(|(best_distance, _)| distance < best_distance) {
            best = Some((distance, candidate));
        }
    }
    best.map(|(_, candidate)| candidate)
}

/// Levenshtein distance between two strings, counted in characters.
///
/// Uses a single row of state rather than a full matrix, so the cost is linear
/// in memory — the inputs are short, but this runs once per candidate and a
/// program may have many commands.
fn edit_distance(a: &str, b: &str) -> usize {
    let b_chars: Vec<char> = b.chars().collect();
    if a.is_empty() {
        return b_chars.len();
    }

    let mut previous: Vec<usize> = (0..=b_chars.len()).collect();
    let mut current = previous.clone();

    for (i, a_char) in a.chars().enumerate() {
        current[0] = i + 1;
        for (j, &b_char) in b_chars.iter().enumerate() {
            let substitute = previous[j] + usize::from(a_char != b_char);
            let insert = current[j] + 1;
            let delete = previous[j + 1] + 1;
            current[j + 1] = substitute.min(insert).min(delete);
        }
        core::mem::swap(&mut previous, &mut current);
    }
    previous[b_chars.len()]
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn test_report_layers_message_detail_suggestion_and_usage() {
        let error = ParseError::new(ErrorKind::UnknownFlag, "--releaze")
            .with_suggestion("--release")
            .with_usage("USAGE: forge build [options]");
        let report = error.report();
        assert!(
            report.starts_with("error: unknown flag '--releaze'"),
            "{report}"
        );
        assert!(report.contains("did you mean '--release'?"));
        assert!(report.contains("USAGE: forge build [options]"));
    }

    #[test]
    fn test_report_omits_absent_sections() {
        let report = ParseError::new(ErrorKind::UnknownCommand, "nope").report();
        assert_eq!(report, "error: unknown command 'nope'");
    }

    #[test]
    fn test_usage_keeps_the_first_most_specific_line() {
        // The innermost command attaches its usage first; an outer level must not
        // overwrite it with something vaguer.
        let error = ParseError::new(ErrorKind::MissingValue, "jobs")
            .with_usage("USAGE: forge build")
            .with_usage("USAGE: forge");
        assert_eq!(error.usage(), Some("USAGE: forge build"));
    }

    #[test]
    fn test_requests_are_successes_on_standard_output() {
        for kind in [ErrorKind::HelpRequested, ErrorKind::VersionRequested] {
            let request = ParseError::request(kind, "the text");
            assert!(request.is_request());
            assert_eq!(request.exit_code(), 0);
            assert_eq!(request.stream(), Stream::Stdout);
            // The report is the text itself, undecorated.
            assert_eq!(request.report(), "the text");
            assert_eq!(request.text(), Some("the text"));
        }
    }

    #[test]
    fn test_mistakes_are_failures_on_standard_error() {
        let error = ParseError::new(ErrorKind::UnknownFlag, "-z");
        assert!(!error.is_request());
        assert_eq!(error.exit_code(), 2);
        assert_eq!(error.stream(), Stream::Stderr);
    }

    #[test]
    fn test_every_kind_has_a_message_naming_its_subject() {
        let kinds = [
            ErrorKind::UnknownFlag,
            ErrorKind::MissingValue,
            ErrorKind::MissingRequired,
            ErrorKind::UnknownCommand,
            ErrorKind::UnexpectedArgument,
            ErrorKind::InvalidValue,
            ErrorKind::Conflict,
            ErrorKind::MissingDependency,
            ErrorKind::MissingSubcommand,
            ErrorKind::NonUtf8,
            ErrorKind::Unauthorized,
        ];
        for kind in kinds {
            let message = ParseError::new(kind, "SUBJECT").message();
            assert!(!message.is_empty(), "{kind:?} has no message");
            assert!(message.contains("SUBJECT"), "{kind:?} drops its subject");
        }
    }

    #[test]
    fn test_display_is_one_line() {
        let error = ParseError::new(ErrorKind::UnknownFlag, "--x")
            .with_suggestion("--y")
            .with_usage("USAGE: z");
        let shown = crate::shim::format!("{error}");
        assert_eq!(shown, "unknown flag '--x'");
        assert!(!shown.contains('\n'));
    }

    #[test]
    fn test_suggestion_catches_a_near_miss_and_rejects_a_far_one() {
        let commands = ["build", "remove", "status"];
        assert_eq!(did_you_mean("buidl", commands), Some("build"));
        assert_eq!(did_you_mean("BUILD", commands), Some("build"));
        assert_eq!(did_you_mean("statu", commands), Some("status"));
        assert_eq!(did_you_mean("xyzzy", commands), None);
        assert_eq!(did_you_mean("", commands), None);
    }

    #[test]
    fn test_suggestion_picks_the_nearest_candidate() {
        // Both are within budget; the closer one must win.
        assert_eq!(
            did_you_mean("reease", ["release", "rebase"]),
            Some("release")
        );
    }

    #[test]
    fn test_suggestion_budget_scales_with_length() {
        // One slip in a three-letter word is offered.
        assert_eq!(did_you_mean("ad", ["add"]), Some("add"));
        // But two short words that merely rhyme are not "nearly" each other.
        assert_eq!(did_you_mean("rm", ["ls"]), None);
    }

    #[test]
    fn test_edit_distance_is_symmetric_and_zero_on_equality() {
        assert_eq!(edit_distance("kitten", "sitting"), 3);
        assert_eq!(edit_distance("sitting", "kitten"), 3);
        assert_eq!(edit_distance("same", "same"), 0);
        assert_eq!(edit_distance("", "abc"), 3);
        assert_eq!(edit_distance("abc", ""), 3);
    }

    #[test]
    fn test_edit_distance_counts_characters_not_bytes() {
        // A multi-byte character is one edit, not two or three.
        assert_eq!(edit_distance("é", "e"), 1);
        assert_eq!(edit_distance("日本", "日本"), 0);
    }

    #[test]
    fn test_command_error_carries_its_exit_code() {
        let failure = CommandError::new("files differ").with_code(3);
        assert_eq!(failure.message(), "files differ");
        assert_eq!(failure.exit_code(), 3);
        assert_eq!(CommandError::new("x").exit_code(), 1);
    }

    #[test]
    fn test_outcome_accepts_the_shapes_a_handler_returns() {
        assert!(().into_outcome().is_ok());
        assert!(Ok::<(), &str>(()).into_outcome().is_ok());

        let from_str = Err::<(), &str>("broken").into_outcome().unwrap_err();
        assert_eq!(from_str.message(), "broken");
        assert_eq!(from_str.exit_code(), 1);

        // A real error type converts through its Display, which is what lets
        // `?` be used inside a handler.
        let io = Err::<(), std::io::Error>(std::io::Error::other("disk gone"))
            .into_outcome()
            .unwrap_err();
        assert_eq!(io.message(), "disk gone");

        // The general path reports every failure as `1`; an exact status travels
        // by the concrete signature instead.
        let flattened = Err::<(), CommandError>(CommandError::new("differ").with_code(2))
            .into_outcome()
            .unwrap_err();
        assert_eq!(flattened.exit_code(), 1);
    }
}

#[cfg(test)]
mod proptests {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        /// Suggesting never panics and never returns something outside the
        /// candidate set.
        #[test]
        fn test_suggestion_is_always_a_candidate(typed in ".{0,12}") {
            let candidates = ["build", "remove", "status", "日本語"];
            if let Some(found) = did_you_mean(&typed, candidates) {
                prop_assert!(candidates.contains(&found));
            }
        }

        /// Edit distance obeys the triangle of identity and symmetry, and is
        /// bounded by the longer input.
        #[test]
        fn test_edit_distance_bounds(a in ".{0,10}", b in ".{0,10}") {
            let d = edit_distance(&a, &b);
            prop_assert_eq!(d, edit_distance(&b, &a));
            prop_assert!(d <= a.chars().count().max(b.chars().count()));
            prop_assert_eq!(d == 0, a == b);
        }

        /// A report always contains the message, whatever decoration is attached.
        #[test]
        fn test_report_always_contains_the_subject(subject in "[a-z-]{1,12}") {
            let error = ParseError::new(ErrorKind::UnknownFlag, subject.clone());
            prop_assert!(error.report().contains(&subject));
        }
    }
}
