//! The application: a registry of commands and the entry point to parsing.
//!
//! An [`App`] holds the top-level commands, the app's own arguments, and the help
//! text around them. Commands are added with [`register`](App::register) or
//! [`command`](App::command) — from anywhere, at any point before parsing, which
//! is the property that makes a command defined in a non-`main` module behave
//! identically to one defined in `main`.
//!
//! ## Which entry point
//!
//! Four, because programs genuinely want different things:
//!
//! | Method | Parses | Runs handlers | Prints | Exits |
//! |---|---|---|---|---|
//! | [`run`](App::run) | process args | yes | yes | returns a code |
//! | [`parse`](App::parse) | process args | yes | yes | yes |
//! | [`try_run_from`](App::try_run_from) | given args | yes | no | no |
//! | [`try_parse_from`](App::try_parse_from) | given args | no | no | no |
//!
//! [`run`](App::run) is the one to reach for:
//!
//! ```no_run
//! use cli_forge::{App, Command, out};
//! use std::process::ExitCode;
//!
//! fn main() -> ExitCode {
//!     let mut app = App::new("forge").version(env!("CARGO_PKG_VERSION"));
//!     app.register(Command::new("build").run(|_| out("building...")));
//!     app.run()
//! }
//! ```
//!
//! [`try_parse_from`](App::try_parse_from) parses and nothing else, which is what
//! makes it the one to test with: no output, no handlers, no exit — just the
//! [`Matches`] or a [`ParseError`].

use std::process::ExitCode;

use crate::arg::Arg;
use crate::command::Command;
use crate::error::{CommandError, ErrorKind, Outcome, ParseError, did_you_mean};
use crate::external::External;
use crate::matches::Matches;
use crate::parser::{self, Cli};
use crate::terminal::ColorChoice;
use crate::theme::Theme;

/// A command-line application.
///
/// Build with [`App::new`], add commands, then call [`run`](App::run).
///
/// # Examples
///
/// ```no_run
/// use cli_forge::{out, App, Arg, Command};
///
/// let mut app = App::new("forge")
///     .version(env!("CARGO_PKG_VERSION"))
///     .about("a project constructor")
///     .help_footer("docs: https://github.com/jamesgober/cli-forge");
///
/// app.register(
///     Command::new("init")
///         .about("bootstrap a new project")
///         .arg(Arg::positional("name").required(true))
///         .run(|m| out(format!("initializing {}", m.value("name").unwrap_or("?")))),
/// );
///
/// let code = app.run();
/// ```
pub struct App {
    name: String,
    version: Option<String>,
    about: Option<String>,
    long_about: Option<String>,
    help_header: Option<String>,
    help_footer: Option<String>,
    commands: Vec<Command>,
    globals: Vec<Arg>,
    help_command: bool,
    theme: Option<Theme>,
    color: Option<ColorChoice>,
    external: Option<ExternalHook>,
    #[cfg(feature = "auth")]
    auth_hook: Option<crate::auth::AuthHook>,
}

/// The hook that receives an external subcommand.
type ExternalHook = Box<dyn Fn(&External<'_>) -> Result<(), CommandError>>;

impl App {
    /// Create an application with the given program name.
    ///
    /// The name appears in usage lines, so it should be what the user types —
    /// `env!("CARGO_BIN_NAME")` where that matches.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::App;
    /// let app = App::new("forge");
    /// # let _ = app;
    /// ```
    #[must_use]
    pub fn new(name: impl Into<String>) -> App {
        App {
            name: name.into(),
            version: None,
            about: None,
            long_about: None,
            help_header: None,
            help_footer: None,
            commands: Vec::new(),
            globals: Vec::new(),
            help_command: true,
            theme: None,
            color: None,
            external: None,
            #[cfg(feature = "auth")]
            auth_hook: None,
        }
    }

    /// Set the version reported by `-V` / `--version`.
    ///
    /// Without this, the version flags are ordinary unknown flags. The usual
    /// idiom is to take it from the manifest so the two cannot drift:
    ///
    /// ```
    /// use cli_forge::App;
    /// let app = App::new("forge").version(env!("CARGO_PKG_VERSION"));
    /// # let _ = app;
    /// ```
    #[must_use]
    pub fn version(mut self, version: impl Into<String>) -> App {
        self.version = Some(version.into());
        self
    }

    /// Set the one-line description shown at the top of the app's help.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::App;
    ///
    /// let app = App::new("forge").about("a project constructor");
    /// assert!(app.help().contains("a project constructor"));
    /// ```
    #[must_use]
    pub fn about(mut self, text: impl Into<String>) -> App {
        self.about = Some(text.into());
        self
    }

    /// Set the fuller description shown on the app's help page in place of
    /// [`about`](App::about), wrapped to the terminal.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::App;
    ///
    /// let app = App::new("forge")
    ///     .about("a project constructor")
    ///     .long_about("Builds, tests, and publishes projects described by a forge.toml.");
    /// assert!(app.help().contains("forge.toml"));
    /// ```
    #[must_use]
    pub fn long_about(mut self, text: impl Into<String>) -> App {
        self.long_about = Some(text.into());
        self
    }

    /// Set the header shown at the top of every generated help page.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::App;
    /// let app = App::new("forge").help_header("forge — project constructor");
    /// # let _ = app;
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
    /// # let _ = app;
    /// ```
    #[must_use]
    pub fn help_footer(mut self, text: impl Into<String>) -> App {
        self.help_footer = Some(text.into());
        self
    }

    /// Accept an argument at the app level, before any command name.
    ///
    /// Mark it [`global`](Arg::global) to have every subcommand accept it too,
    /// written on either side of the command name — which is what `--verbose`
    /// and `--color` want to be.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg, Command};
    ///
    /// let mut app = App::new("forge")
    ///     .arg(Arg::count("verbose").short('v').global(true))
    ///     .arg(Arg::option("config").short('c').global(true));
    /// app.register(Command::new("build"));
    ///
    /// let m = app.try_parse_from(["-vv", "build", "-c", "forge.toml"]).unwrap();
    /// assert_eq!(m.count("verbose"), 2);
    /// assert_eq!(m.leaf().value("config"), Some("forge.toml"));
    /// ```
    #[must_use]
    pub fn arg(mut self, arg: Arg) -> App {
        self.globals.push(arg);
        self
    }

    /// Accept several app-level arguments at once.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg};
    ///
    /// let app = App::new("forge").args([
    ///     Arg::count("verbose").short('v').global(true),
    ///     Arg::flag("quiet").short('q').global(true),
    /// ]);
    /// # let _ = app;
    /// ```
    #[must_use]
    pub fn args<I>(mut self, args: I) -> App
    where
        I: IntoIterator<Item = Arg>,
    {
        self.globals.extend(args);
        self
    }

    /// Add a top-level command, chaining.
    ///
    /// The builder-style counterpart of [`register`](App::register), for the
    /// common case where the whole app is one expression.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Command};
    ///
    /// let app = App::new("forge")
    ///     .command(Command::new("build").about("compile the project"))
    ///     .command(Command::new("test").about("run the tests"));
    ///
    /// assert_eq!(app.commands().len(), 2);
    /// ```
    #[must_use]
    pub fn command(mut self, cmd: Command) -> App {
        self.commands.push(cmd);
        self
    }

    /// Register a top-level command.
    ///
    /// Call this from anywhere with access to the `App` — a different module, a
    /// plugin's setup function, a loop over a config file — at any point before
    /// parsing. A command registered outside `main` is reachable and behaves
    /// identically to one registered in `main`, which is what lets each command
    /// live beside the code it drives.
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

    /// Whether to accept `prog help [command]`, and to show the help when the
    /// program is run with no arguments at all. On by default.
    ///
    /// Both are what users expect; turn it off for a program whose bare
    /// invocation is meaningful, or that declares its own `help`.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Command, ErrorKind};
    ///
    /// let mut app = App::new("demo");
    /// app.register(Command::new("build"));
    ///
    /// // `help build` renders that command's page.
    /// let err = app.try_parse_from(["help", "build"]).unwrap_err();
    /// assert_eq!(err.kind(), ErrorKind::HelpRequested);
    /// assert!(err.text().unwrap().contains("demo build"));
    ///
    /// // And a bare invocation shows the app help rather than doing nothing.
    /// let bare = app.try_parse_from([] as [&str; 0]).unwrap_err();
    /// assert_eq!(bare.kind(), ErrorKind::HelpRequested);
    ///
    /// // Unless the program says otherwise.
    /// let quiet = App::new("demo").help_command(false).command(Command::new("build"));
    /// assert!(quiet.try_parse_from([] as [&str; 0]).is_ok());
    /// ```
    #[must_use]
    pub fn help_command(mut self, enabled: bool) -> App {
        self.help_command = enabled;
        self
    }

    /// Use `theme` for this app's own output: its help, its errors, and every
    /// themed printer the program calls.
    ///
    /// Installed as the process default when parsing begins, so the builder
    /// itself stays free of side effects.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Command, Glyphs, Theme};
    ///
    /// let mut app = App::new("demo").theme(Theme::new().set_glyphs(Glyphs::Ascii));
    /// app.register(Command::new("build"));
    /// let _ = app.try_parse_from(["build"]);
    /// # Theme::new().install();
    /// ```
    #[must_use]
    pub fn theme(mut self, theme: Theme) -> App {
        self.theme = Some(theme);
        self
    }

    /// Decide this app's colour output, overriding detection.
    ///
    /// What a `--color` flag parses into. Applied when parsing begins, so help
    /// and errors honour it too.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, ColorChoice, Command};
    ///
    /// let mut app = App::new("demo").color(ColorChoice::Never);
    /// app.register(Command::new("build"));
    /// let _ = app.try_parse_from(["build"]);
    /// # cli_forge::terminal::set_color_choice(ColorChoice::Auto);
    /// ```
    #[must_use]
    pub fn color(mut self, choice: ColorChoice) -> App {
        self.color = Some(choice);
        self
    }

    /// Hand any command name the app does not define to `hook`, rather than
    /// reporting it as unknown.
    ///
    /// The plugin mechanism `cargo` and `git` use: `forge watch` runs a separate
    /// `forge-watch` program, found at run time, without the app knowing it
    /// exists in advance. The hook receives the name, the untouched tokens after
    /// it, and the app-level arguments parsed before it. It may return anything a
    /// [`run`](Command::run) handler may, so a failure becomes the exit status.
    ///
    /// A misspelt built-in reaches the hook too, since the hook is the only
    /// place that knows whether a matching program exists;
    /// [`External::suggestion`] gives it the nearest registered command to fall
    /// back on. See the [`External`] documentation for a complete hook.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Command};
    ///
    /// let app = App::new("forge")
    ///     .external(|ext| {
    ///         // In a real program: spawn format!("forge-{}", ext.name()).
    ///         if ext.name() == "watch" { Ok(()) } else { Err("no such command") }
    ///     })
    ///     .command(Command::new("build"));
    ///
    /// assert!(app.try_run_from(["watch", "src"]).unwrap().is_ok());
    /// assert!(app.try_run_from(["nonexistent"]).unwrap().is_err());
    /// // Registered commands are untouched.
    /// assert!(app.try_run_from(["build"]).unwrap().is_ok());
    /// ```
    #[must_use]
    pub fn external<F, R>(mut self, hook: F) -> App
    where
        F: Fn(&External<'_>) -> R + 'static,
        R: Outcome,
    {
        self.external = Some(Box::new(move |ext| hook(ext).into_outcome()));
        self
    }

    /// The registered command nearest to `name`, if one is close enough to
    /// suggest.
    ///
    /// The same suggestion an unknown-command error carries, for a program that
    /// reports such mistakes itself.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Command};
    ///
    /// let app = App::new("forge").command(Command::new("build"));
    /// assert_eq!(app.suggest("buidl"), Some("build"));
    /// assert_eq!(app.suggest("xyzzy"), None);
    /// ```
    #[must_use]
    pub fn suggest(&self, name: &str) -> Option<&str> {
        did_you_mean(
            name,
            self.commands
                .iter()
                .filter(|c| !c.hidden)
                .flat_map(Command::invocation_names),
        )
    }

    /// Set the authorization hook that enforces
    /// [`Command::requires_auth`](crate::Command::requires_auth).
    ///
    /// The hook receives an [`AuthRequest`](crate::AuthRequest) naming the command
    /// being authorized and returns whether to allow it. An auth-gated command
    /// runs only if the hook returns `true`; otherwise parsing yields
    /// [`ErrorKind::Unauthorized`] and the handler does not run. Without a hook,
    /// auth-gated commands are never authorized — the seam fails closed, because
    /// the alternative is a gate that silently opens when nobody wired it up.
    ///
    /// Requires the `auth` feature.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "auth")]
    /// # {
    /// use cli_forge::{App, Command, ErrorKind};
    ///
    /// let mut app = App::new("demo").auth(|req| req.command() != "publish");
    /// app.register(Command::new("publish").requires_auth(true).run(|_| {}));
    ///
    /// let err = app.try_run_from(["publish"]).unwrap_err();
    /// assert_eq!(err.kind(), ErrorKind::Unauthorized);
    /// # }
    /// ```
    #[cfg(feature = "auth")]
    #[must_use]
    pub fn auth(mut self, hook: impl Fn(&crate::auth::AuthRequest<'_>) -> bool + 'static) -> App {
        self.auth_hook = Some(Box::new(hook));
        self
    }

    /// This app's program name.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::App;
    /// assert_eq!(App::new("forge").name(), "forge");
    /// ```
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// This app's version, if one was set.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::App;
    /// assert_eq!(App::new("forge").version("1.2.3").version_text(), Some("1.2.3"));
    /// ```
    #[must_use]
    pub fn version_text(&self) -> Option<&str> {
        self.version.as_deref()
    }

    /// The registered top-level commands, in registration order.
    ///
    /// The root of the read-only view a sibling crate walks to generate shell
    /// completions, manual pages, or documentation from the live command tree
    /// rather than from a second description of it.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg, Command};
    ///
    /// let app = App::new("forge")
    ///     .command(Command::new("build").arg(Arg::flag("release")));
    ///
    /// // Exactly what a completion generator needs.
    /// for command in app.commands() {
    ///     for arg in command.arguments() {
    ///         let _ = (arg.long_form(), arg.expects_value(), arg.allowed_values());
    ///     }
    /// }
    /// assert_eq!(app.commands()[0].name(), "build");
    /// ```
    #[must_use]
    pub fn commands(&self) -> &[Command] {
        &self.commands
    }

    /// The app-level arguments, in declaration order.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg};
    ///
    /// let app = App::new("forge").arg(Arg::count("verbose").global(true));
    /// assert!(app.global_arguments()[0].is_global());
    /// ```
    #[must_use]
    pub fn global_arguments(&self) -> &[Arg] {
        &self.globals
    }

    /// Render the top-level help as a string.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Command};
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

    /// Render one command's help as a string, or `None` if `path` names no
    /// command.
    ///
    /// `path` is the chain of command names from the app down, so a nested
    /// command's page is reachable without re-deriving the tree.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Command};
    ///
    /// let mut app = App::new("forge");
    /// app.register(
    ///     Command::new("remote")
    ///         .subcommand(Command::new("add").about("add a remote")),
    /// );
    ///
    /// let help = app.command_help(["remote", "add"]).unwrap();
    /// assert!(help.contains("forge remote add"));
    /// assert!(help.contains("add a remote"));
    /// assert!(app.command_help(["nope"]).is_none());
    /// ```
    #[must_use]
    pub fn command_help<I, S>(&self, path: I) -> Option<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let wanted: Vec<String> = path
            .into_iter()
            .map(|name| name.as_ref().to_owned())
            .collect();
        let mut names: Vec<&str> = Vec::with_capacity(wanted.len());
        let mut current: Option<&Command> = None;

        for name in &wanted {
            let next = match current {
                None => self.commands.iter().find(|c| c.matches_name(name)),
                Some(command) => command.find_subcommand(name),
            }?;
            names.push(next.name.as_str());
            current = Some(next);
        }
        let command = current?;
        Some(crate::help::render_command(&self.cli(), &names, command))
    }

    /// Parse an explicit argument list, excluding the program name.
    ///
    /// Parses and nothing else: no output, no handlers, no exit. That is what
    /// makes it the one to test with, and it is the one behaviour change from 1.x
    /// worth noting — the old version ran handlers as a side effect of parsing.
    /// Use [`try_run_from`](App::try_run_from) when handlers should run.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg, Command, ErrorKind};
    ///
    /// let mut app = App::new("demo");
    /// app.register(Command::new("build").arg(Arg::option("jobs").short('j')));
    ///
    /// // Well-formed input parses.
    /// let matches = app.try_parse_from(["build", "-j", "4"]).unwrap();
    /// assert_eq!(matches.subcommand().unwrap().1.value("jobs"), Some("4"));
    ///
    /// // Malformed input returns a structured error, and never panics.
    /// let err = app.try_parse_from(["build", "--bogus"]).unwrap_err();
    /// assert_eq!(err.kind(), ErrorKind::UnknownFlag);
    /// ```
    pub fn try_parse_from<I, S>(&self, args: I) -> Result<Matches, ParseError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.apply_presentation();
        let tokens: Vec<String> = args.into_iter().map(Into::into).collect();
        let matches = parser::parse_app(&self.cli(), &tokens)?;
        #[cfg(feature = "auth")]
        self.enforce_auth(&matches)?;
        Ok(matches)
    }

    /// Parse an explicit argument list and run the selected command's handler.
    ///
    /// The outer `Result` is about the command line; the inner one is about the
    /// work. Keeping them apart is the point: a bad invocation and a failed
    /// command deserve different messages and different exit codes.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Command};
    ///
    /// let mut app = App::new("demo");
    /// app.register(Command::new("ok").run(|_| {}));
    /// app.register(Command::new("bad").run(|_| Err("it broke")));
    ///
    /// assert!(app.try_run_from(["ok"]).unwrap().is_ok());
    ///
    /// let failure = app.try_run_from(["bad"]).unwrap().unwrap_err();
    /// assert_eq!(failure.message(), "it broke");
    ///
    /// // A bad invocation never reaches a handler at all.
    /// assert!(app.try_run_from(["nope"]).is_err());
    /// ```
    pub fn try_run_from<I, S>(&self, args: I) -> Result<Result<(), CommandError>, ParseError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let matches = self.try_parse_from(args)?;
        Ok(self.dispatch(&matches))
    }

    /// Run the handler of the deepest command `matches` resolved to.
    ///
    /// For a program that parses and dispatches in separate steps — to inspect or
    /// adjust the `Matches` in between, or to decide not to run at all.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Command};
    ///
    /// let mut app = App::new("demo");
    /// app.register(Command::new("build").run(|_| {}));
    ///
    /// let matches = app.try_parse_from(["build"]).unwrap();
    /// assert!(app.dispatch(&matches).is_ok());
    /// ```
    pub fn dispatch(&self, matches: &Matches) -> Result<(), CommandError> {
        if let Some((name, args)) = matches.external() {
            return match &self.external {
                Some(hook) => hook(&External::new(name, args, matches, self.suggest(name))),
                // Recorded only when a hook exists, so this is a `Matches` from a
                // different app; there is nothing sensible to run.
                None => Ok(()),
            };
        }
        let Some((name, sub)) = matches.subcommand() else {
            return Ok(());
        };
        let Some(command) = self.commands.iter().find(|c| c.name == name) else {
            return Ok(());
        };
        dispatch_command(command, sub)
    }

    /// Parse the process arguments, run the selected command, and report the
    /// outcome as an exit status.
    ///
    /// Everything a `main` needs: help and version are printed to standard output
    /// and reported as success; a bad command line is reported to standard error
    /// with its suggestion and usage line, as status `2`; a failed command prints
    /// its message and its own status. Nothing panics, and nothing is left for
    /// the caller to arrange.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use cli_forge::{App, Command, out};
    /// use std::process::ExitCode;
    ///
    /// fn main() -> ExitCode {
    ///     let mut app = App::new("forge").version(env!("CARGO_PKG_VERSION"));
    ///     app.register(Command::new("build").run(|_| out("building...")));
    ///     app.run()
    /// }
    /// ```
    #[must_use]
    pub fn run(&self) -> ExitCode {
        let args = match process_args() {
            Ok(args) => args,
            Err(error) => return self.report(&error),
        };
        match self.try_run_from(args) {
            Ok(Ok(())) => ExitCode::SUCCESS,
            Ok(Err(failure)) => {
                crate::theme::emit(crate::theme::Level::Error, failure.message());
                exit_code(failure.exit_code())
            }
            Err(error) => self.report(&error),
        }
    }

    /// Parse the process arguments, run the selected command's handler, and
    /// return the [`Matches`].
    ///
    /// Prints and exits on anything that stops parsing: help and version to
    /// standard output with status `0`, a bad command line to standard error with
    /// status `2`. Prefer [`run`](App::run), which reports the same outcomes
    /// without taking the exit decision away from `main`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use cli_forge::{App, Command, out};
    ///
    /// let mut app = App::new("demo").version(env!("CARGO_PKG_VERSION"));
    /// app.register(Command::new("hello").run(|_| out("hello")));
    /// let matches = app.parse();
    /// # let _ = matches;
    /// ```
    #[must_use]
    pub fn parse(&self) -> Matches {
        let args = match process_args() {
            Ok(args) => args,
            Err(error) => {
                let _ = self.report(&error);
                std::process::exit(error.exit_code());
            }
        };
        match self.try_parse_from(args) {
            Ok(matches) => {
                if let Err(failure) = self.dispatch(&matches) {
                    crate::theme::emit(crate::theme::Level::Error, failure.message());
                    std::process::exit(failure.exit_code());
                }
                matches
            }
            Err(error) => {
                let _ = self.report(&error);
                std::process::exit(error.exit_code());
            }
        }
    }

    /// Print `error`'s report to the stream it belongs on, and report its status.
    fn report(&self, error: &ParseError) -> ExitCode {
        crate::output::write_line(error.stream(), &error.styled_report());
        exit_code(error.exit_code())
    }

    /// Install the theme and colour choice this app asked for, if any.
    ///
    /// Done here rather than in the builder so that constructing an `App` has no
    /// side effects, while help and errors still render through the program's own
    /// theme.
    fn apply_presentation(&self) {
        if let Some(choice) = self.color {
            crate::terminal::set_color_choice(choice);
        }
        if let Some(theme) = &self.theme {
            theme.clone().install();
        }
    }

    /// Assemble the borrowed context the parser and help engine need.
    fn cli(&self) -> Cli<'_> {
        Cli {
            app_name: &self.name,
            about: self.about.as_deref(),
            long_about: self.long_about.as_deref(),
            header: self.help_header.as_deref(),
            footer: self.help_footer.as_deref(),
            version: self.version.as_deref(),
            commands: &self.commands,
            globals: &self.globals,
            help_command: self.help_command,
            external: self.external.is_some(),
            #[cfg(feature = "auth")]
            authorizer: self.auth_hook.as_ref(),
        }
    }

    /// Refuse the resolved command if it is auth-gated and the hook does not
    /// authorize it. Fails closed when no hook is set.
    #[cfg(feature = "auth")]
    fn enforce_auth(&self, matches: &Matches) -> Result<(), ParseError> {
        let Some((path, leaf)) = self.resolve_path(matches) else {
            return Ok(());
        };
        if !leaf.requires_auth {
            return Ok(());
        }
        let request = crate::auth::AuthRequest::new(&path);
        if self.auth_hook.as_ref().is_some_and(|hook| hook(&request)) {
            return Ok(());
        }
        Err(ParseError::new(ErrorKind::Unauthorized, &leaf.name))
    }

    /// Walk the resolved subcommand chain, returning the command-name path and
    /// the deepest command.
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
}

impl std::fmt::Debug for App {
    // `DebugStruct::field` returns `&mut Self` for chaining; discarding those
    // returns is the builder pattern, not a dropped result.
    #[allow(unused_results)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("App");
        s.field("name", &self.name);
        s.field("version", &self.version);
        s.field("about", &self.about);
        s.field("help_header", &self.help_header);
        s.field("help_footer", &self.help_footer);
        s.field("commands", &self.commands);
        s.field("globals", &self.globals);
        s.field("help_command", &self.help_command);
        s.field("has_external_hook", &self.external.is_some());
        #[cfg(feature = "auth")]
        s.field("has_auth_hook", &self.auth_hook.is_some());
        s.finish()
    }
}

/// The process arguments as strings, excluding the program name.
///
/// `std::env::args` panics on an argument that is not valid UTF-8, which on both
/// Windows and Unix is something a user can cause from the shell — so this reads
/// the raw form and reports the problem instead. A program that must handle such
/// arguments can read `args_os` itself and decide.
fn process_args() -> Result<Vec<String>, ParseError> {
    let mut args = Vec::new();
    for raw in std::env::args_os().skip(1) {
        match raw.into_string() {
            Ok(arg) => args.push(arg),
            Err(lossy) => {
                return Err(ParseError::new(
                    ErrorKind::NonUtf8,
                    lossy.to_string_lossy().into_owned(),
                )
                .with_detail("arguments must be valid UTF-8".to_owned()));
            }
        }
    }
    Ok(args)
}

/// Map an exit status onto the byte a process can actually return, keeping a
/// failure a failure rather than letting it wrap around to success.
fn exit_code(code: i32) -> ExitCode {
    match u8::try_from(code) {
        Ok(0) if code != 0 => ExitCode::FAILURE,
        Ok(byte) => ExitCode::from(byte),
        Err(_) => ExitCode::FAILURE,
    }
}

/// Walk to the leaf of the resolved path and run its handler, if any.
fn dispatch_command(command: &Command, matches: &Matches) -> Result<(), CommandError> {
    if let Some((name, sub)) = matches.subcommand() {
        if let Some(child) = command.find_subcommand(name) {
            return dispatch_command(child, sub);
        }
    }
    command.invoke(matches)
}

#[cfg(test)]
mod tests;
