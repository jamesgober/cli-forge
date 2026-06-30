//! The application: a registry of commands and the entry point to parsing.
//!
//! An [`App`] holds the top-level commands and the (optional) help header and
//! footer. Commands are added with [`register`](App::register) — from anywhere,
//! at any point before parsing, which is the property that makes a command
//! defined in a non-`main` module behave identically to one defined in `main`.
//!
//! [`parse`](App::parse) reads the process arguments, resolves the command,
//! parses its arguments, and runs the selected command's handler. Malformed
//! input is reported as a structured [`ParseError`]: [`parse`](App::parse) prints
//! it through the output layer and exits, while
//! [`try_parse_from`](App::try_parse_from) returns it for the caller to handle.

use crate::command::Command;
use crate::error::ParseError;
use crate::matches::Matches;
use crate::parser;

/// A command-line application.
///
/// Build with [`App::new`], add commands with [`register`](App::register), then
/// call [`parse`](App::parse).
///
/// # Examples
///
/// ```no_run
/// use cli_forge::{App, Arg, Command, out};
///
/// let mut app = App::new("forge")
///     .help_header("forge — project constructor")
///     .help_footer("docs: https://github.com/jamesgober/cli-forge");
///
/// app.register(
///     Command::new("init")
///         .about("bootstrap a new project")
///         .arg(Arg::positional("name").required(true))
///         .run(|m| out(format!("initializing {}", m.value("name").unwrap_or("?")))),
/// );
///
/// let _matches = app.parse();
/// ```
#[derive(Debug)]
pub struct App {
    // `name`, `help_header`, and `help_footer` are stored now and read by the
    // help engine (v0.4.0); they are part of the frozen surface today.
    #[allow(dead_code)]
    name: String,
    #[allow(dead_code)]
    help_header: Option<String>,
    #[allow(dead_code)]
    help_footer: Option<String>,
    commands: Vec<Command>,
}

impl App {
    /// Create an application with the given program name.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::App;
    /// let app = App::new("forge");
    /// ```
    #[must_use]
    pub fn new(name: impl Into<String>) -> App {
        App {
            name: name.into(),
            help_header: None,
            help_footer: None,
            commands: Vec::new(),
        }
    }

    /// Set the header shown above generated help. Rendered by the help engine
    /// (v0.4.0).
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::App;
    /// let app = App::new("forge").help_header("forge — project constructor");
    /// ```
    #[must_use]
    pub fn help_header(mut self, text: impl Into<String>) -> App {
        self.help_header = Some(text.into());
        self
    }

    /// Set the footer shown below generated help. Rendered by the help engine
    /// (v0.4.0).
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::App;
    /// let app = App::new("forge").help_footer("see the docs for more");
    /// ```
    #[must_use]
    pub fn help_footer(mut self, text: impl Into<String>) -> App {
        self.help_footer = Some(text.into());
        self
    }

    /// Register a top-level command.
    ///
    /// Call this from anywhere with access to the `App` — a different module, a
    /// plugin's setup function, a loop over a config — at any point before
    /// parsing. A command registered outside `main` is reachable and behaves
    /// identically to one registered in `main`.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Command};
    ///
    /// let mut app = App::new("demo");
    /// app.register(Command::new("status").about("show status"));
    /// app.register(Command::new("sync").about("synchronize"));
    /// ```
    pub fn register(&mut self, cmd: Command) {
        self.commands.push(cmd);
    }

    /// Parse the process arguments, run the selected command's handler, and
    /// return the [`Matches`].
    ///
    /// On malformed input the structured [`ParseError`] is printed to standard
    /// error through the output layer and the process exits with status `2`.
    /// This never panics. For a non-exiting variant — for embedding or tests —
    /// use [`try_parse_from`](App::try_parse_from).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use cli_forge::{App, Command, out};
    ///
    /// let mut app = App::new("demo");
    /// app.register(Command::new("hello").run(|_| out("hello")));
    /// let _matches = app.parse();
    /// ```
    #[must_use]
    pub fn parse(&self) -> Matches {
        let args: Vec<String> = std::env::args().skip(1).collect();
        match self.try_parse_from(args) {
            Ok(matches) => matches,
            Err(error) => {
                crate::err(format_args!("error: {error}"));
                std::process::exit(2);
            }
        }
    }

    /// Parse an explicit argument list (excluding the program name), run the
    /// selected command's handler, and return the [`Matches`] — or a structured
    /// [`ParseError`] on malformed input. Never exits the process; never panics.
    ///
    /// This is the testable, embeddable counterpart to [`parse`](App::parse).
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg, Command, ParseError};
    ///
    /// let mut app = App::new("demo");
    /// app.register(Command::new("build").arg(Arg::option("jobs").short('j')));
    ///
    /// // Well-formed input parses.
    /// let matches = app.try_parse_from(["build", "-j", "4"]).unwrap();
    /// assert_eq!(matches.subcommand().unwrap().1.value("jobs"), Some("4"));
    ///
    /// // Malformed input returns a structured error.
    /// let err = app.try_parse_from(["build", "--bogus"]).unwrap_err();
    /// assert!(matches!(err, ParseError::UnknownFlag { .. }));
    /// ```
    pub fn try_parse_from<I, S>(&self, args: I) -> Result<Matches, ParseError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let tokens: Vec<String> = args.into_iter().map(Into::into).collect();
        let matches = parser::parse_app(&self.commands, &tokens)?;
        self.dispatch(&matches);
        Ok(matches)
    }

    /// Run the handler of the deepest command the parse resolved to.
    fn dispatch(&self, matches: &Matches) {
        if let Some((name, sub)) = matches.subcommand() {
            if let Some(command) = self.commands.iter().find(|c| c.name == name) {
                dispatch_command(command, sub);
            }
        }
    }

    /// The registered commands that are not hidden. Drives the help engine
    /// (v0.4.0); used today to verify hidden commands are excluded from listings.
    #[cfg(test)]
    pub(crate) fn visible_commands(&self) -> impl Iterator<Item = &Command> {
        self.commands.iter().filter(|c| !c.hidden)
    }
}

/// Walk to the leaf of the resolved path and run its handler, if any.
fn dispatch_command(command: &Command, matches: &Matches) {
    if let Some((name, sub)) = matches.subcommand() {
        if let Some(child) = command.find_subcommand(name) {
            dispatch_command(child, sub);
            return;
        }
    }
    if let Some(handler) = &command.handler {
        handler(matches);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::arg::Arg;

    #[test]
    fn test_unknown_command_is_structured_error() {
        let app = App::new("demo");
        let err = app.try_parse_from(["nope"]).unwrap_err();
        assert_eq!(
            err,
            ParseError::UnknownCommand {
                name: "nope".into()
            }
        );
    }

    #[test]
    fn test_empty_args_yield_no_subcommand() {
        let app = App::new("demo");
        let matches = app.try_parse_from(Vec::<String>::new()).unwrap();
        assert!(matches.subcommand().is_none());
    }

    #[test]
    fn test_hidden_command_is_invokable_but_not_listed() {
        let mut app = App::new("demo");
        app.register(Command::new("secret").hidden(true));
        app.register(Command::new("visible"));

        // Still invokable.
        let matches = app.try_parse_from(["secret"]).unwrap();
        assert_eq!(matches.subcommand().map(|(name, _)| name), Some("secret"));

        // Absent from the visible listing the help engine will render.
        let listed: Vec<&str> = app.visible_commands().map(|c| c.name.as_str()).collect();
        assert!(listed.contains(&"visible"));
        assert!(!listed.contains(&"secret"));
    }

    #[test]
    fn test_handler_runs_for_selected_command_only() {
        static INIT_HITS: AtomicUsize = AtomicUsize::new(0);
        static OTHER_HITS: AtomicUsize = AtomicUsize::new(0);

        let mut app = App::new("demo");
        app.register(Command::new("init").run(|_| {
            let _ = INIT_HITS.fetch_add(1, Ordering::SeqCst);
        }));
        app.register(Command::new("other").run(|_| {
            let _ = OTHER_HITS.fetch_add(1, Ordering::SeqCst);
        }));

        let _ = app.try_parse_from(["init"]).unwrap();
        assert_eq!(INIT_HITS.load(Ordering::SeqCst), 1);
        assert_eq!(OTHER_HITS.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_nested_subcommand_dispatch() {
        static ADD_HITS: AtomicUsize = AtomicUsize::new(0);

        let mut app = App::new("demo");
        app.register(
            Command::new("remote")
                .subcommand(Command::new("add").run(|_| {
                    let _ = ADD_HITS.fetch_add(1, Ordering::SeqCst);
                }))
                .subcommand(Command::new("remove")),
        );

        let matches = app.try_parse_from(["remote", "add"]).unwrap();
        let (_, remote) = matches.subcommand().unwrap();
        assert_eq!(remote.subcommand().map(|(name, _)| name), Some("add"));
        assert_eq!(ADD_HITS.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_missing_required_argument() {
        let mut app = App::new("demo");
        app.register(Command::new("greet").arg(Arg::positional("name").required(true)));
        let err = app.try_parse_from(["greet"]).unwrap_err();
        assert_eq!(err, ParseError::MissingRequired { arg: "name".into() });
    }

    #[test]
    fn test_requires_auth_flag_is_stored_not_enforced() {
        let mut app = App::new("demo");
        static RAN: AtomicUsize = AtomicUsize::new(0);
        app.register(Command::new("publish").requires_auth(true).run(|_| {
            let _ = RAN.fetch_add(1, Ordering::SeqCst);
        }));
        // Enforcement arrives in v0.5.0; for now the command still runs.
        let _ = app.try_parse_from(["publish"]).unwrap();
        assert_eq!(RAN.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_combined_short_flags_and_attached_option_value() {
        let mut app = App::new("demo");
        app.register(
            Command::new("run")
                .arg(Arg::flag("all").short('a'))
                .arg(Arg::flag("verbose").short('v'))
                .arg(Arg::option("output").short('o')),
        );
        // `-av` bundles two flags; `-ofile` attaches the option value.
        let matches = app.try_parse_from(["run", "-av", "-ofile"]).unwrap();
        let (_, run) = matches.subcommand().unwrap();
        assert!(run.flag("all"));
        assert!(run.flag("verbose"));
        assert_eq!(run.value("output"), Some("file"));
    }

    #[test]
    fn test_end_of_options_marker_treats_rest_as_positional() {
        let mut app = App::new("demo");
        app.register(Command::new("echo").arg(Arg::positional("text")));
        let matches = app.try_parse_from(["echo", "--", "--not-a-flag"]).unwrap();
        assert_eq!(
            matches.subcommand().unwrap().1.value("text"),
            Some("--not-a-flag")
        );
    }
}

#[cfg(test)]
mod proptests {
    use proptest::prelude::*;

    use super::*;
    use crate::arg::Arg;

    fn sample_app() -> App {
        let mut app = App::new("demo");
        app.register(
            Command::new("build")
                .arg(Arg::flag("release").short('r'))
                .arg(Arg::option("jobs").short('j'))
                .arg(Arg::positional("target"))
                .subcommand(Command::new("clean")),
        );
        app
    }

    proptest! {
        /// No argument vector — however malformed — may panic the parser.
        #[test]
        fn test_try_parse_never_panics(tokens in proptest::collection::vec(".*", 0..8)) {
            let app = sample_app();
            let _ = app.try_parse_from(tokens);
        }
    }
}
