//! The command tree.
//!
//! A [`Command`] is one node: a name and aliases, the help text around it, the
//! [`Arg`]s it accepts, any nested subcommands, the `hidden` and `requires_auth`
//! flags, and an optional `run` handler. Commands compose recursively through
//! [`subcommand`](Command::subcommand), so an arbitrarily deep tree is just
//! values built with the same builder.
//!
//! Commands are registered into an [`App`](crate::App) from anywhere — a command
//! built in one module behaves identically to one built in `main`, which is what
//! lets a program's commands live next to the code they drive instead of all
//! piling up in one file.
//!
//! ## Handlers report failure
//!
//! A [`run`](Command::run) handler may return nothing, or any `Result` whose
//! error can be printed. The error becomes the program's exit status and its
//! message, so the ordinary Rust `?` works inside a command:
//!
//! ```
//! use cli_forge::{Command, out};
//!
//! let read = Command::new("read").run(|m| -> std::io::Result<()> {
//!     let path = m.value("path").unwrap_or("notes.txt");
//!     out(std::fs::read_to_string(path)?);
//!     Ok(())
//! });
//! # let _ = read;
//! ```

use core::fmt;

use crate::arg::{Arg, ArgKind};
use crate::error::{CommandError, Outcome};
use crate::group::ArgGroup;
use crate::matches::Matches;

/// A handler invoked when its command is the one the user selected.
type Handler = Box<dyn Fn(&Matches) -> Result<(), CommandError>>;

/// One node in the command tree.
///
/// Build with [`Command::new`] and refine with the chaining methods. Attach a
/// [`run`](Command::run) handler to do the work, [`arg`](Command::arg) to accept
/// input, and [`subcommand`](Command::subcommand) to nest.
///
/// # Examples
///
/// ```
/// use cli_forge::{Arg, Command};
///
/// let build = Command::new("build")
///     .about("compile the project")
///     .arg(Arg::flag("release").short('r'))
///     .run(|m| {
///         let _ = m.flag("release");
///     });
/// # let _ = build;
/// ```
pub struct Command {
    pub(crate) name: String,
    pub(crate) aliases: Vec<String>,
    pub(crate) about: Option<String>,
    pub(crate) long_about: Option<String>,
    pub(crate) before_help: Option<String>,
    pub(crate) after_help: Option<String>,
    pub(crate) usage: Option<String>,
    pub(crate) args: Vec<Arg>,
    pub(crate) groups: Vec<ArgGroup>,
    pub(crate) subcommands: Vec<Command>,
    pub(crate) hidden: bool,
    pub(crate) requires_auth: bool,
    pub(crate) subcommand_required: bool,
    pub(crate) order: i32,
    pub(crate) category: Option<String>,
    pub(crate) handler: Option<Handler>,
}

impl Command {
    /// Create a command with the given invocation name.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Command;
    /// let cmd = Command::new("init");
    /// # let _ = cmd;
    /// ```
    #[must_use]
    pub fn new(name: impl Into<String>) -> Command {
        Command {
            name: name.into(),
            aliases: Vec::new(),
            about: None,
            long_about: None,
            before_help: None,
            after_help: None,
            usage: None,
            args: Vec::new(),
            groups: Vec::new(),
            subcommands: Vec::new(),
            hidden: false,
            requires_auth: false,
            subcommand_required: false,
            order: 0,
            category: None,
            handler: None,
        }
    }

    /// Add an alternative name that also invokes this command. Chain it to add
    /// several. Aliases are shown alongside the name in help.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Command;
    /// let cmd = Command::new("remove").alias("rm").alias("del");
    /// # let _ = cmd;
    /// ```
    #[must_use]
    pub fn alias(mut self, alias: impl Into<String>) -> Command {
        self.aliases.push(alias.into());
        self
    }

    /// Add several alternative names at once.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Command;
    /// let cmd = Command::new("remove").aliases(["rm", "del"]);
    /// # let _ = cmd;
    /// ```
    #[must_use]
    pub fn aliases<I, S>(mut self, aliases: I) -> Command
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.aliases.extend(aliases.into_iter().map(Into::into));
        self
    }

    /// Set the one-line description shown in the parent's command list.
    ///
    /// Keep it short: it shares a line with the command's name.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Command;
    /// let cmd = Command::new("init").about("bootstrap a new project");
    /// # let _ = cmd;
    /// ```
    #[must_use]
    pub fn about(mut self, text: impl Into<String>) -> Command {
        self.about = Some(text.into());
        self
    }

    /// Set the fuller description shown at the top of this command's own help
    /// page, where [`about`](Command::about) is too short to explain anything.
    ///
    /// Written as paragraphs; the help engine wraps it to the terminal.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Command};
    ///
    /// let mut app = App::new("forge");
    /// app.register(
    ///     Command::new("init")
    ///         .about("bootstrap a new project")
    ///         .long_about(
    ///             "Creates a forge.toml, a source directory, and a first \
    ///              commit. Existing files are never overwritten.",
    ///         ),
    /// );
    ///
    /// let help = app.command_help(["init"]).unwrap();
    /// assert!(help.contains("never overwritten"));
    /// ```
    #[must_use]
    pub fn long_about(mut self, text: impl Into<String>) -> Command {
        self.long_about = Some(text.into());
        self
    }

    /// Text shown above everything else on this command's help page.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Command};
    ///
    /// let mut app = App::new("forge");
    /// app.register(Command::new("build").before_help("forge build — the compiler driver"));
    /// assert!(app.command_help(["build"]).unwrap().contains("compiler driver"));
    /// ```
    #[must_use]
    pub fn before_help(mut self, text: impl Into<String>) -> Command {
        self.before_help = Some(text.into());
        self
    }

    /// Text shown below everything else on this command's help page.
    ///
    /// The natural home for examples, which are the part of a help page people
    /// actually read.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Command};
    ///
    /// let mut app = App::new("forge");
    /// app.register(
    ///     Command::new("build").after_help("EXAMPLES:\n  forge build --release"),
    /// );
    /// assert!(app.command_help(["build"]).unwrap().contains("EXAMPLES:"));
    /// ```
    #[must_use]
    pub fn after_help(mut self, text: impl Into<String>) -> Command {
        self.after_help = Some(text.into());
        self
    }

    /// Replace the generated usage line with an exact one.
    ///
    /// The generated line is right for most commands; this is for the ones whose
    /// real grammar it cannot express, such as a command that takes a trailing
    /// command of its own.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg, Command};
    ///
    /// let mut app = App::new("forge");
    /// app.register(
    ///     Command::new("exec")
    ///         .arg(Arg::positional("argv").multiple(true))
    ///         .usage("forge exec [options] -- <program> [args]..."),
    /// );
    /// assert!(app.command_help(["exec"]).unwrap().contains("-- <program>"));
    /// ```
    #[must_use]
    pub fn usage(mut self, text: impl Into<String>) -> Command {
        self.usage = Some(text.into());
        self
    }

    /// Accept an argument. Add as many as the command needs; positionals are
    /// filled in the order they are added.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{Arg, Command};
    /// let cmd = Command::new("copy")
    ///     .arg(Arg::positional("from").required(true))
    ///     .arg(Arg::positional("to").required(true))
    ///     .arg(Arg::flag("force").short('f'));
    /// # let _ = cmd;
    /// ```
    #[must_use]
    pub fn arg(mut self, arg: Arg) -> Command {
        self.args.push(arg);
        self
    }

    /// Accept several arguments at once.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{Arg, Command};
    ///
    /// let shared = [Arg::flag("force").short('f'), Arg::flag("dry-run")];
    /// let cmd = Command::new("sync").args(shared);
    /// # let _ = cmd;
    /// ```
    #[must_use]
    pub fn args<I>(mut self, args: I) -> Command
    where
        I: IntoIterator<Item = Arg>,
    {
        self.args.extend(args);
        self
    }

    /// Add a constraint over a set of this command's arguments.
    ///
    /// For the rules that are about a set rather than a pair — "exactly one
    /// output format", "at least one source". See [`ArgGroup`].
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg, ArgGroup, Command};
    ///
    /// let mut app = App::new("export");
    /// app.register(
    ///     Command::new("dump")
    ///         .args([Arg::flag("json"), Arg::flag("yaml")])
    ///         .group(ArgGroup::new("format").args(["json", "yaml"]).required(true)),
    /// );
    ///
    /// let m = app.try_parse_from(["dump", "--json"]).unwrap();
    /// assert_eq!(m.leaf().group("format"), Some("json"));
    /// ```
    #[must_use]
    pub fn group(mut self, group: ArgGroup) -> Command {
        self.groups.push(group);
        self
    }

    /// Nest a subcommand. Subcommands compose recursively to any depth.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Command;
    /// let remote = Command::new("remote")
    ///     .subcommand(Command::new("add"))
    ///     .subcommand(Command::new("remove"));
    /// # let _ = remote;
    /// ```
    #[must_use]
    pub fn subcommand(mut self, cmd: Command) -> Command {
        self.subcommands.push(cmd);
        self
    }

    /// Refuse to run this command without one of its subcommands.
    ///
    /// The right behaviour for a command that is only a grouping — `git remote`
    /// on its own is a mistake, not a no-op. Invoking it bare reports
    /// [`ErrorKind::MissingSubcommand`](crate::ErrorKind::MissingSubcommand) with
    /// the command's help, instead of silently doing nothing.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Command, ErrorKind};
    ///
    /// let mut app = App::new("forge");
    /// app.register(
    ///     Command::new("remote")
    ///         .subcommand_required(true)
    ///         .subcommand(Command::new("add")),
    /// );
    ///
    /// let err = app.try_parse_from(["remote"]).unwrap_err();
    /// assert_eq!(err.kind(), ErrorKind::MissingSubcommand);
    /// assert!(app.try_parse_from(["remote", "add"]).is_ok());
    /// ```
    #[must_use]
    pub fn subcommand_required(mut self, required: bool) -> Command {
        self.subcommand_required = required;
        self
    }

    /// Hide the command from generated help while leaving it invokable.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Command};
    ///
    /// let mut app = App::new("demo");
    /// app.register(Command::new("debug-dump").hidden(true));
    ///
    /// assert!(!app.help().contains("debug-dump"));
    /// assert!(app.try_parse_from(["debug-dump"]).is_ok());
    /// ```
    #[must_use]
    pub fn hidden(mut self, yes: bool) -> Command {
        self.hidden = yes;
        self
    }

    /// Set where this command sits in a help listing.
    ///
    /// Commands are listed by ascending order and then by registration order, so
    /// leaving this alone keeps the order they were registered in. Use it to put
    /// the commands people reach for first at the top, rather than wherever the
    /// source happened to put them.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Command};
    ///
    /// let mut app = App::new("forge");
    /// app.register(Command::new("clean").display_order(10));
    /// app.register(Command::new("build").display_order(1));
    ///
    /// let help = app.help();
    /// assert!(help.find("build").unwrap() < help.find("clean").unwrap());
    /// ```
    #[must_use]
    pub fn display_order(mut self, order: i32) -> Command {
        self.order = order;
        self
    }

    /// List this command under its own heading in its parent's help, rather
    /// than under `COMMANDS:`.
    ///
    /// For an app with enough commands that one list stops being readable. The
    /// heading is the name upper-cased with a colon, matching the built-in ones.
    /// Uncategorised commands come first, under `COMMANDS:`; the named sections
    /// follow in the order their first command appears, which
    /// [`display_order`](Command::display_order) controls as usual.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Command};
    ///
    /// let mut app = App::new("forge");
    /// app.register(Command::new("build"));
    /// app.register(Command::new("publish").category("Release"));
    /// app.register(Command::new("yank").category("Release"));
    ///
    /// let help = cli_forge::text::strip(&app.help()).into_owned();
    /// assert!(help.contains("COMMANDS:"));
    /// assert!(help.contains("RELEASE:"));
    /// assert!(help.find("build").unwrap() < help.find("RELEASE:").unwrap());
    /// ```
    #[must_use]
    pub fn category(mut self, name: impl Into<String>) -> Command {
        self.category = Some(name.into());
        self
    }

    /// Mark the command as requiring authentication.
    ///
    /// With the `auth` feature enabled, the command runs — and appears in help —
    /// only when the app's [`App::auth`](crate::App::auth) hook authorizes it;
    /// otherwise invoking it reports
    /// [`ErrorKind::Unauthorized`](crate::ErrorKind::Unauthorized). Without the
    /// `auth` feature the flag is inert and the command runs and shows normally.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Command;
    /// let cmd = Command::new("publish").requires_auth(true);
    /// # let _ = cmd;
    /// ```
    #[must_use]
    pub fn requires_auth(mut self, yes: bool) -> Command {
        self.requires_auth = yes;
        self
    }

    /// Attach the handler run when this command is selected.
    ///
    /// It receives the [`Matches`] parsed for this command's level and may return
    /// nothing, or any `Result` whose error implements
    /// [`Display`](fmt::Display) — including `io::Result`, `Box<dyn Error>`, and
    /// whatever error type the rest of the program uses — so `?` works inside the
    /// handler and the message becomes the program's complaint. A failure reported
    /// this way exits `1`; use [`run_status`](Command::run_status) when the exact
    /// status matters.
    ///
    /// # A handler that only diverges
    ///
    /// `run(|_| todo!())` does not compile, and neither does any closure whose
    /// body only diverges — a bare `panic!`, `unreachable!`, or
    /// `unimplemented!`. Such a closure returns the never type, and Rust picks
    /// the [`Outcome`] implementation before it would coerce that to `()`.
    /// Adding a semicolon does not help, because a block that unconditionally
    /// diverges still has that type. Annotate the return instead:
    ///
    /// ```
    /// use cli_forge::Command;
    ///
    /// // Command::new("x").run(|_| todo!());                  // will not compile
    /// let scaffold = Command::new("x").run(|_| -> () { todo!() });
    /// let later = Command::new("y").run(|_| -> Result<(), String> { todo!() });
    /// # let _ = (scaffold, later);
    /// ```
    ///
    /// This is a limitation of trait selection on a diverging closure rather
    /// than a choice; `impl Outcome for !` would fix it and is not stable.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{out, Command};
    ///
    /// // Nothing to report.
    /// let hello = Command::new("hello").run(|_| out("hello"));
    ///
    /// // A message, and exit 1.
    /// let pull = Command::new("pull").run(|_| Err("not a repository"));
    ///
    /// // Or `?` against any error type.
    /// let read = Command::new("read").run(|_| -> std::io::Result<()> {
    ///     out(std::fs::read_to_string("notes.txt")?);
    ///     Ok(())
    /// });
    /// # let _ = (hello, pull, read);
    /// ```
    #[must_use]
    pub fn run<F, R>(mut self, handler: F) -> Command
    where
        F: Fn(&Matches) -> R + 'static,
        R: Outcome,
    {
        self.handler = Some(Box::new(move |matches| handler(matches).into_outcome()));
        self
    }

    /// Attach a handler that names its own exit status.
    ///
    /// The same as [`run`](Command::run) but with a concrete signature, which is
    /// what lets a [`CommandError`]'s
    /// [`with_code`](CommandError::with_code) survive — for the tools whose exit
    /// codes are part of their contract. Convert from another error type with
    /// `map_err`.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Command, CommandError};
    ///
    /// let mut app = App::new("demo");
    /// app.register(Command::new("diff").run_status(|_| {
    ///     Err(CommandError::new("files differ").with_code(2))
    /// }));
    ///
    /// let failure = app.try_run_from(["diff"]).unwrap().unwrap_err();
    /// assert_eq!(failure.exit_code(), 2);
    /// assert_eq!(failure.message(), "files differ");
    /// ```
    #[must_use]
    pub fn run_status<F>(mut self, handler: F) -> Command
    where
        F: Fn(&Matches) -> Result<(), CommandError> + 'static,
    {
        self.handler = Some(Box::new(handler));
        self
    }

    /// This command's invocation name.
    ///
    /// Part of the read-only view sibling crates use to generate completions,
    /// manual pages, and documentation from a live command tree.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Command;
    /// assert_eq!(Command::new("build").name(), "build");
    /// ```
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// This command's alternative names.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Command;
    /// assert_eq!(Command::new("remove").aliases(["rm"]).alias_names(), ["rm"]);
    /// ```
    #[must_use]
    pub fn alias_names(&self) -> &[String] {
        &self.aliases
    }

    /// This command's one-line description, if it has one.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Command;
    /// assert_eq!(Command::new("init").about("set up").about_text(), Some("set up"));
    /// ```
    #[must_use]
    pub fn about_text(&self) -> Option<&str> {
        self.about.as_deref()
    }

    /// The arguments this command accepts, in declaration order.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{Arg, Command};
    ///
    /// let cmd = Command::new("build").arg(Arg::flag("release"));
    /// assert_eq!(cmd.arguments().len(), 1);
    /// assert_eq!(cmd.arguments()[0].name(), "release");
    /// ```
    #[must_use]
    pub fn arguments(&self) -> &[Arg] {
        &self.args
    }

    /// The argument groups this command declares.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{ArgGroup, Command};
    /// let cmd = Command::new("dump").group(ArgGroup::new("format"));
    /// assert_eq!(cmd.groups()[0].name(), "format");
    /// ```
    #[must_use]
    pub fn groups(&self) -> &[ArgGroup] {
        &self.groups
    }

    /// This command's direct subcommands, in declaration order.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Command;
    ///
    /// let cmd = Command::new("remote").subcommand(Command::new("add"));
    /// assert_eq!(cmd.subcommands().len(), 1);
    /// assert_eq!(cmd.subcommands()[0].name(), "add");
    /// ```
    #[must_use]
    pub fn subcommands(&self) -> &[Command] {
        &self.subcommands
    }

    /// The help heading this command is listed under, if not the default.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Command;
    /// assert_eq!(Command::new("yank").category("Release").category_name(), Some("Release"));
    /// ```
    #[must_use]
    pub fn category_name(&self) -> Option<&str> {
        self.category.as_deref()
    }

    /// Whether this command is hidden from generated help.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Command;
    /// assert!(Command::new("x").hidden(true).is_hidden());
    /// ```
    #[must_use]
    pub const fn is_hidden(&self) -> bool {
        self.hidden
    }

    /// Whether this command is gated behind the auth hook.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Command;
    /// assert!(Command::new("publish").requires_auth(true).is_auth_gated());
    /// ```
    #[must_use]
    pub const fn is_auth_gated(&self) -> bool {
        self.requires_auth
    }

    /// Find an argument by its long form, searching `extra` as well — the app's
    /// global arguments, which every command accepts.
    ///
    /// `extra` holds borrows rather than values so that the caller can assemble
    /// it without copying the arguments themselves.
    pub(crate) fn find_long<'a>(&'a self, long: &str, extra: &[&'a Arg]) -> Option<&'a Arg> {
        self.args
            .iter()
            .chain(extra.iter().copied())
            .find(|a| a.long_name() == Some(long))
    }

    /// Find an argument by its short form, searching `extra` as well.
    pub(crate) fn find_short<'a>(&'a self, short: char, extra: &[&'a Arg]) -> Option<&'a Arg> {
        self.args
            .iter()
            .chain(extra.iter().copied())
            .find(|a| a.short == Some(short))
    }

    /// Whether `name` matches this command's name or any of its aliases.
    pub(crate) fn matches_name(&self, name: &str) -> bool {
        self.name == name || self.aliases.iter().any(|a| a == name)
    }

    /// Find a direct subcommand by name or alias.
    pub(crate) fn find_subcommand(&self, name: &str) -> Option<&Command> {
        self.subcommands.iter().find(|c| c.matches_name(name))
    }

    /// The positional arguments, in declaration order.
    pub(crate) fn positionals(&self) -> impl Iterator<Item = &Arg> {
        self.args.iter().filter(|a| a.kind == ArgKind::Positional)
    }

    /// Every name this command answers to, for suggesting a near miss.
    pub(crate) fn invocation_names(&self) -> impl Iterator<Item = &str> {
        core::iter::once(self.name.as_str()).chain(self.aliases.iter().map(String::as_str))
    }

    /// Run this command's handler, if it has one.
    pub(crate) fn invoke(&self, matches: &Matches) -> Result<(), CommandError> {
        match &self.handler {
            Some(handler) => handler(matches),
            None => Ok(()),
        }
    }
}

impl fmt::Debug for Command {
    /// A handler is a closure and cannot be shown, so its presence is reported
    /// instead.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = f.debug_struct("Command");
        let _ = s.field("name", &self.name);
        let _ = s.field("aliases", &self.aliases);
        let _ = s.field("about", &self.about);
        let _ = s.field("args", &self.args);
        let _ = s.field("groups", &self.groups);
        let _ = s.field("subcommands", &self.subcommands);
        let _ = s.field("hidden", &self.hidden);
        let _ = s.field("requires_auth", &self.requires_auth);
        let _ = s.field("subcommand_required", &self.subcommand_required);
        let _ = s.field("order", &self.order);
        let _ = s.field("category", &self.category);
        let _ = s.field("has_handler", &self.handler.is_some());
        s.finish()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::arg::Arg;

    #[test]
    fn test_name_matching_covers_aliases() {
        let cmd = Command::new("remove").aliases(["rm", "del"]);
        for name in ["remove", "rm", "del"] {
            assert!(cmd.matches_name(name), "{name}");
        }
        assert!(!cmd.matches_name("delete"));
        assert_eq!(
            cmd.invocation_names().collect::<Vec<_>>(),
            ["remove", "rm", "del"]
        );
    }

    #[test]
    fn test_argument_lookup_searches_globals_too() {
        let cmd = Command::new("build").arg(Arg::flag("release").short('r'));
        let verbose = Arg::count("verbose").short('v');
        let globals = [&verbose];

        assert_eq!(
            cmd.find_long("release", &globals).map(|a| a.name.as_str()),
            Some("release")
        );
        assert_eq!(
            cmd.find_long("verbose", &globals).map(|a| a.name.as_str()),
            Some("verbose")
        );
        assert_eq!(
            cmd.find_short('r', &globals).map(|a| a.name.as_str()),
            Some("release")
        );
        assert_eq!(
            cmd.find_short('v', &globals).map(|a| a.name.as_str()),
            Some("verbose")
        );
        assert!(cmd.find_long("nope", &globals).is_none());
        assert!(cmd.find_short('z', &globals).is_none());
    }

    #[test]
    fn test_a_commands_own_argument_shadows_a_global_of_the_same_name() {
        // The command's arguments are searched first, so a command can override
        // an app-level spelling rather than being stuck with it.
        let cmd = Command::new("build").arg(Arg::option("verbose"));
        let verbose = Arg::count("verbose");
        let globals = [&verbose];
        let found = cmd.find_long("verbose", &globals).unwrap();
        assert_eq!(found.kind, ArgKind::Option);
    }

    #[test]
    fn test_positionals_are_filtered_in_declaration_order() {
        let cmd = Command::new("copy")
            .arg(Arg::flag("force"))
            .arg(Arg::positional("from"))
            .arg(Arg::option("mode"))
            .arg(Arg::positional("to"));
        let names: Vec<&str> = cmd.positionals().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["from", "to"]);
    }

    #[test]
    fn test_handler_shapes_all_reach_the_dispatcher() {
        let matches = Matches::default();

        assert!(Command::new("a").run(|_| {}).invoke(&matches).is_ok());
        assert!(
            Command::new("b")
                .run(|_| Ok::<(), &str>(()))
                .invoke(&matches)
                .is_ok()
        );

        let failed = Command::new("c")
            .run(|_| Err("broken"))
            .invoke(&matches)
            .unwrap_err();
        assert_eq!(failed.message(), "broken");
        assert_eq!(failed.exit_code(), 1);

        // The general path flattens every failure to `1`...
        let flattened = Command::new("d")
            .run(|_| Err(CommandError::new("differ").with_code(3)))
            .invoke(&matches)
            .unwrap_err();
        assert_eq!(flattened.exit_code(), 1);

        // ...and the concrete one carries the status the command chose.
        let exact = Command::new("d2")
            .run_status(|_| Err(CommandError::new("differ").with_code(3)))
            .invoke(&matches)
            .unwrap_err();
        assert_eq!(exact.exit_code(), 3);

        // A command with no handler is not a failure; it simply does nothing.
        assert!(Command::new("e").invoke(&matches).is_ok());
    }

    #[test]
    fn test_handler_receives_the_matches_for_its_own_level() {
        let mut matches = Matches::default();
        let _ = matches.flags.insert("release".to_owned());
        let cmd = Command::new("build").run(|m| {
            if m.flag("release") {
                Ok(())
            } else {
                Err("expected the flag to be visible")
            }
        });
        assert!(cmd.invoke(&matches).is_ok());
    }

    #[test]
    fn test_args_adds_several_at_once() {
        let cmd = Command::new("sync").args([Arg::flag("force"), Arg::flag("dry-run")]);
        assert_eq!(cmd.arguments().len(), 2);
    }

    #[test]
    fn test_introspection_exposes_the_tree_without_exposing_the_internals() {
        // The surface a completions or manual-page generator needs.
        let cmd = Command::new("remote")
            .alias("r")
            .about("manage remotes")
            .arg(Arg::flag("verbose"))
            .subcommand(Command::new("add"))
            .hidden(true)
            .requires_auth(true);

        assert_eq!(cmd.name(), "remote");
        assert_eq!(cmd.alias_names(), ["r"]);
        assert_eq!(cmd.about_text(), Some("manage remotes"));
        assert_eq!(cmd.arguments().len(), 1);
        assert_eq!(cmd.subcommands()[0].name(), "add");
        assert!(cmd.is_hidden());
        assert!(cmd.is_auth_gated());
    }

    #[test]
    fn test_debug_reports_the_handler_without_trying_to_show_it() {
        let shown = crate::shim::format!("{:?}", Command::new("x").run(|_| {}));
        assert!(shown.contains("has_handler: true"));
        assert!(crate::shim::format!("{:?}", Command::new("x")).contains("has_handler: false"));
    }
}
