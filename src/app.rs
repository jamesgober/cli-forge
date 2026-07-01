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
use crate::parser::{self, Cli};

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
pub struct App {
    name: String,
    version: Option<String>,
    help_header: Option<String>,
    help_footer: Option<String>,
    commands: Vec<Command>,
    #[cfg(feature = "auth")]
    auth_hook: Option<crate::auth::AuthHook>,
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
            version: None,
            help_header: None,
            help_footer: None,
            commands: Vec::new(),
            #[cfg(feature = "auth")]
            auth_hook: None,
        }
    }

    /// Set the authorization hook that enforces
    /// [`Command::requires_auth`](crate::Command::requires_auth).
    ///
    /// The hook receives an [`AuthRequest`](crate::AuthRequest) naming the command
    /// being authorized and returns whether to allow it. An auth-gated command
    /// runs only if the hook returns `true`; otherwise parsing yields
    /// [`ParseError::Unauthorized`] and the handler does not run. Without a hook,
    /// auth-gated commands are never authorized (the seam fails closed).
    ///
    /// Requires the `auth` feature.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "auth")]
    /// # {
    /// use cli_forge::{App, Command, ParseError};
    ///
    /// let mut app = App::new("demo").auth(|req| req.command() != "publish");
    /// app.register(Command::new("publish").requires_auth(true).run(|_| {}));
    ///
    /// let err = app.try_parse_from(["publish"]).unwrap_err();
    /// assert!(matches!(err, ParseError::Unauthorized { .. }));
    /// # }
    /// ```
    #[cfg(feature = "auth")]
    #[must_use]
    pub fn auth(mut self, hook: impl Fn(&crate::auth::AuthRequest<'_>) -> bool + 'static) -> App {
        self.auth_hook = Some(Box::new(hook));
        self
    }

    /// Set the version reported by `-V` / `--version`.
    ///
    /// Without this, the version flags are treated as ordinary unknown flags.
    /// A common idiom is to pass the crate version:
    ///
    /// ```
    /// use cli_forge::App;
    /// let app = App::new("forge").version(env!("CARGO_PKG_VERSION"));
    /// ```
    #[must_use]
    pub fn version(mut self, version: impl Into<String>) -> App {
        self.version = Some(version.into());
        self
    }

    /// Set the header shown at the top of every generated help page.
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

    /// Set the footer shown at the bottom of every generated help page.
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
    /// `-h` / `--help` and `-V` / `--version` are handled here: the rendered help
    /// or version is printed to standard output and the process exits `0`. On
    /// malformed input the structured [`ParseError`] is printed to standard error
    /// and the process exits `2`. This never panics. For a non-exiting variant —
    /// for embedding or tests — use [`try_parse_from`](App::try_parse_from).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use cli_forge::{App, Command, out};
    ///
    /// let mut app = App::new("demo").version(env!("CARGO_PKG_VERSION"));
    /// app.register(Command::new("hello").run(|_| out("hello")));
    /// let _matches = app.parse();
    /// ```
    #[must_use]
    pub fn parse(&self) -> Matches {
        let args: Vec<String> = std::env::args().skip(1).collect();
        match self.try_parse_from(args) {
            Ok(matches) => matches,
            Err(ParseError::HelpRequested(text) | ParseError::VersionRequested(text)) => {
                crate::out(text);
                std::process::exit(0);
            }
            Err(error) => {
                crate::err(format_args!("error: {error}"));
                std::process::exit(2);
            }
        }
    }

    /// Render the top-level help as a string.
    ///
    /// Useful for printing help on demand — for example, when no command was
    /// given:
    ///
    /// ```
    /// use cli_forge::{App, Command, out};
    ///
    /// let mut app = App::new("demo");
    /// app.register(Command::new("build").about("compile the project"));
    ///
    /// let help = app.help();
    /// assert!(help.contains("build"));
    /// assert!(help.contains("compile the project"));
    /// ```
    #[must_use]
    pub fn help(&self) -> String {
        crate::help::render_app(&self.cli())
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
        let matches = parser::parse_app(&self.cli(), &tokens)?;
        #[cfg(feature = "auth")]
        self.enforce_auth(&matches)?;
        self.dispatch(&matches);
        Ok(matches)
    }

    /// Assemble the borrowed context the parser and help engine need.
    fn cli(&self) -> Cli<'_> {
        Cli {
            app_name: &self.name,
            header: self.help_header.as_deref(),
            footer: self.help_footer.as_deref(),
            version: self.version.as_deref(),
            commands: &self.commands,
            #[cfg(feature = "auth")]
            authorizer: self.auth_hook.as_ref(),
        }
    }

    /// Refuse the resolved command if it is auth-gated and the hook does not
    /// authorize it. Fails closed when no hook is set.
    #[cfg(feature = "auth")]
    fn enforce_auth(&self, matches: &Matches) -> Result<(), ParseError> {
        if let Some((path, leaf)) = self.resolve_path(matches) {
            if leaf.requires_auth {
                let request = crate::auth::AuthRequest::new(&path);
                let authorized = self.auth_hook.as_ref().is_some_and(|hook| hook(&request));
                if !authorized {
                    return Err(ParseError::Unauthorized {
                        command: leaf.name.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Walk the resolved subcommand chain, returning the command-name path and
    /// the deepest (leaf) command.
    #[cfg(feature = "auth")]
    fn resolve_path(&self, matches: &Matches) -> Option<(Vec<&str>, &Command)> {
        let (name, mut sub) = matches.subcommand()?;
        let mut command = self.commands.iter().find(|c| c.name == name)?;
        let mut path = vec![command.name.as_str()];
        while let Some((sub_name, next)) = sub.subcommand() {
            command = command.find_subcommand(sub_name)?;
            path.push(command.name.as_str());
            sub = next;
        }
        Some((path, command))
    }

    /// Run the handler of the deepest command the parse resolved to.
    fn dispatch(&self, matches: &Matches) {
        if let Some((name, sub)) = matches.subcommand() {
            if let Some(command) = self.commands.iter().find(|c| c.name == name) {
                dispatch_command(command, sub);
            }
        }
    }

    /// The registered commands that are not hidden. Used in tests to verify
    /// hidden commands are excluded from listings.
    #[cfg(test)]
    pub(crate) fn visible_commands(&self) -> impl Iterator<Item = &Command> {
        self.commands.iter().filter(|c| !c.hidden)
    }
}

impl std::fmt::Debug for App {
    // `DebugStruct::field` returns `&mut Self` for chaining; discarding those
    // returns is the builder pattern, not a dropped result.
    #[allow(unused_results)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("App");
        s.field("name", &self.name);
        s.field("version", &self.version);
        s.field("help_header", &self.help_header);
        s.field("help_footer", &self.help_footer);
        s.field("commands", &self.commands);
        #[cfg(feature = "auth")]
        s.field("has_auth_hook", &self.auth_hook.is_some());
        s.finish()
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

    #[cfg(not(feature = "auth"))]
    #[test]
    fn test_requires_auth_is_inert_without_auth_feature() {
        let mut app = App::new("demo");
        static RAN: AtomicUsize = AtomicUsize::new(0);
        app.register(Command::new("publish").requires_auth(true).run(|_| {
            let _ = RAN.fetch_add(1, Ordering::SeqCst);
        }));
        // Without the `auth` feature the flag does nothing: the command runs.
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

    fn help_demo() -> App {
        let mut app = App::new("demo")
            .version("1.0.0")
            .help_header("HEADER LINE")
            .help_footer("FOOTER LINE");
        app.register(Command::new("build").about("compile the project"));
        app.register(
            Command::new("remove")
                .aliases(["rm", "del"])
                .about("delete a thing"),
        );
        app.register(Command::new("secret").hidden(true).about("do not show me"));
        app.register(Command::new("publish").requires_auth(true).about("gated"));
        app
    }

    #[test]
    fn test_help_respects_header_footer_and_lists_options() {
        let help = help_demo().help();
        assert!(help.contains("HEADER LINE"));
        assert!(help.contains("FOOTER LINE"));
        assert!(help.contains("USAGE: demo <command> [options]"));
        assert!(help.contains("-h, --help"));
        assert!(help.contains("-V, --version"));
    }

    #[test]
    fn test_help_hides_hidden_commands() {
        let help = help_demo().help();
        assert!(help.contains("build"));
        assert!(help.contains("compile the project"));
        // Hidden commands are always absent from help.
        assert!(!help.contains("secret"));
        assert!(!help.contains("do not show me"));
    }

    #[cfg(not(feature = "auth"))]
    #[test]
    fn test_help_shows_auth_command_without_auth_feature() {
        // Without the `auth` feature, `requires_auth` is inert — the command is
        // listed like any other.
        let help = help_demo().help();
        assert!(help.contains("publish"));
    }

    #[test]
    fn test_help_shows_command_aliases() {
        let help = help_demo().help();
        assert!(help.contains("remove, rm, del"));
    }

    #[test]
    fn test_help_omits_version_line_without_version() {
        let mut app = App::new("demo");
        app.register(Command::new("build"));
        let help = app.help();
        assert!(help.contains("-h, --help"));
        assert!(!help.contains("--version"));
    }

    #[test]
    fn test_help_flag_returns_help_signal() {
        let app = help_demo();
        // Top level.
        let err = app.try_parse_from(["--help"]).unwrap_err();
        assert!(matches!(err, ParseError::HelpRequested(ref text) if text.contains("USAGE")));
        // Command level renders that command's help.
        let err = app.try_parse_from(["build", "-h"]).unwrap_err();
        assert!(matches!(err, ParseError::HelpRequested(ref text) if text.contains("demo build")));
    }

    #[test]
    fn test_version_flag_returns_version_signal() {
        let app = help_demo();
        let err = app.try_parse_from(["--version"]).unwrap_err();
        assert_eq!(err, ParseError::VersionRequested("1.0.0".into()));
        let err = app.try_parse_from(["build", "-V"]).unwrap_err();
        assert_eq!(err, ParseError::VersionRequested("1.0.0".into()));
    }

    #[test]
    fn test_version_flag_is_unknown_without_version_set() {
        let mut app = App::new("demo");
        app.register(Command::new("build"));
        let err = app.try_parse_from(["build", "--version"]).unwrap_err();
        assert_eq!(
            err,
            ParseError::UnknownFlag {
                flag: "--version".into()
            }
        );
    }

    #[test]
    fn test_alias_dispatches_to_canonical_command() {
        static HITS: AtomicUsize = AtomicUsize::new(0);
        let mut app = App::new("demo");
        app.register(Command::new("remove").aliases(["rm", "del"]).run(|_| {
            let _ = HITS.fetch_add(1, Ordering::SeqCst);
        }));

        let matches = app.try_parse_from(["rm"]).unwrap();
        // The alias resolves to the canonical name in the parsed result.
        assert_eq!(matches.subcommand().map(|(name, _)| name), Some("remove"));
        assert_eq!(HITS.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_user_defined_help_flag_overrides_builtin() {
        let mut app = App::new("demo");
        // A command that defines its own `--help` flag suppresses the built-in.
        app.register(Command::new("run").arg(Arg::flag("help")));
        let matches = app.try_parse_from(["run", "--help"]).unwrap();
        assert!(matches.subcommand().unwrap().1.flag("help"));
    }

    // --- Auth seam (requires the `auth` feature) ---

    #[cfg(feature = "auth")]
    fn auth_app(ran: &'static AtomicUsize) -> App {
        let mut app = App::new("demo");
        app.register(Command::new("publish").requires_auth(true).run(move |_| {
            let _ = ran.fetch_add(1, Ordering::SeqCst);
        }));
        app
    }

    #[cfg(feature = "auth")]
    #[test]
    fn test_auth_gated_command_blocked_without_hook() {
        static RAN: AtomicUsize = AtomicUsize::new(0);
        let app = auth_app(&RAN);
        // No hook set → fail closed.
        let err = app.try_parse_from(["publish"]).unwrap_err();
        assert_eq!(
            err,
            ParseError::Unauthorized {
                command: "publish".into()
            }
        );
        assert_eq!(RAN.load(Ordering::SeqCst), 0);
    }

    #[cfg(feature = "auth")]
    #[test]
    fn test_auth_gated_command_refused_when_hook_denies() {
        static RAN: AtomicUsize = AtomicUsize::new(0);
        let app = auth_app(&RAN).auth(|_| false);
        let err = app.try_parse_from(["publish"]).unwrap_err();
        assert!(matches!(err, ParseError::Unauthorized { .. }));
        assert_eq!(RAN.load(Ordering::SeqCst), 0);
    }

    #[cfg(feature = "auth")]
    #[test]
    fn test_auth_gated_command_runs_when_authorized() {
        static RAN: AtomicUsize = AtomicUsize::new(0);
        let app = auth_app(&RAN).auth(|_| true);
        let _ = app.try_parse_from(["publish"]).unwrap();
        assert_eq!(RAN.load(Ordering::SeqCst), 1);
    }

    #[cfg(feature = "auth")]
    #[test]
    fn test_auth_hook_receives_command_name() {
        static RAN: AtomicUsize = AtomicUsize::new(0);
        // Authorize everything except `publish`.
        let app = auth_app(&RAN).auth(|req| req.command() != "publish");
        let err = app.try_parse_from(["publish"]).unwrap_err();
        assert!(matches!(err, ParseError::Unauthorized { .. }));
        assert_eq!(RAN.load(Ordering::SeqCst), 0);
    }

    #[cfg(feature = "auth")]
    #[test]
    fn test_non_auth_command_ignores_hook() {
        static RAN: AtomicUsize = AtomicUsize::new(0);
        let mut app = App::new("demo").auth(|_| false);
        app.register(Command::new("status").run(move |_| {
            let _ = RAN.fetch_add(1, Ordering::SeqCst);
        }));
        // A command without `requires_auth` runs regardless of the (denying) hook.
        let _ = app.try_parse_from(["status"]).unwrap();
        assert_eq!(RAN.load(Ordering::SeqCst), 1);
    }

    #[cfg(feature = "auth")]
    #[test]
    fn test_help_lists_auth_command_only_when_authorized() {
        let build = |authorize: bool| {
            let mut app = App::new("demo").auth(move |_| authorize);
            app.register(Command::new("publish").requires_auth(true).about("ship it"));
            app.register(Command::new("build").about("compile"));
            app
        };
        assert!(!build(false).help().contains("publish"));
        assert!(build(true).help().contains("publish"));
        // A non-gated command is listed either way.
        assert!(build(false).help().contains("build"));
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
