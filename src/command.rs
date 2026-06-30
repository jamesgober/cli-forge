//! The command tree.
//!
//! A [`Command`] is one node: a name, optional help text, the [`Arg`]s it
//! accepts, any nested subcommands, the `hidden` and `requires_auth` flags, and
//! an optional `run` handler. Commands compose recursively through
//! [`subcommand`](Command::subcommand), so an arbitrarily deep tree is just
//! values built with the same builder.
//!
//! Commands are registered into an [`App`](crate::App) from anywhere — a command
//! built in one module behaves identically to one built in `main`.

use std::fmt;

use crate::arg::{Arg, ArgKind};
use crate::matches::Matches;

/// A handler invoked when its command is the one the user selected.
type Handler = Box<dyn Fn(&Matches)>;

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
/// ```
pub struct Command {
    pub(crate) name: String,
    pub(crate) about: Option<String>,
    pub(crate) args: Vec<Arg>,
    pub(crate) subcommands: Vec<Command>,
    pub(crate) hidden: bool,
    pub(crate) requires_auth: bool,
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
    /// ```
    #[must_use]
    pub fn new(name: impl Into<String>) -> Command {
        Command {
            name: name.into(),
            about: None,
            args: Vec::new(),
            subcommands: Vec::new(),
            hidden: false,
            requires_auth: false,
            handler: None,
        }
    }

    /// Set the one-line description shown in help.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Command;
    /// let cmd = Command::new("init").about("bootstrap a new project");
    /// ```
    #[must_use]
    pub fn about(mut self, text: impl Into<String>) -> Command {
        self.about = Some(text.into());
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
    /// ```
    #[must_use]
    pub fn arg(mut self, arg: Arg) -> Command {
        self.args.push(arg);
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
    /// ```
    #[must_use]
    pub fn subcommand(mut self, cmd: Command) -> Command {
        self.subcommands.push(cmd);
        self
    }

    /// Hide the command from generated help while leaving it invokable.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Command;
    /// let cmd = Command::new("debug-dump").hidden(true);
    /// ```
    #[must_use]
    pub fn hidden(mut self, yes: bool) -> Command {
        self.hidden = yes;
        self
    }

    /// Mark the command as requiring authentication. The flag is recorded now;
    /// enforcement arrives with the auth seam (v0.5.0). Until then the command
    /// runs normally.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Command;
    /// let cmd = Command::new("publish").requires_auth(true);
    /// ```
    #[must_use]
    pub fn requires_auth(mut self, yes: bool) -> Command {
        self.requires_auth = yes;
        self
    }

    /// Attach the handler run when this command is selected. It receives the
    /// [`Matches`] parsed for this command's level.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{out, Command};
    /// let cmd = Command::new("hello").run(|_| out("hello"));
    /// ```
    #[must_use]
    pub fn run(mut self, handler: impl Fn(&Matches) + 'static) -> Command {
        self.handler = Some(Box::new(handler));
        self
    }

    /// Find an argument by its long form.
    pub(crate) fn find_long(&self, long: &str) -> Option<&Arg> {
        self.args.iter().find(|a| a.long_name() == Some(long))
    }

    /// Find an argument by its short form.
    pub(crate) fn find_short(&self, short: char) -> Option<&Arg> {
        self.args.iter().find(|a| a.short == Some(short))
    }

    /// Find a direct subcommand by name.
    pub(crate) fn find_subcommand(&self, name: &str) -> Option<&Command> {
        self.subcommands.iter().find(|c| c.name == name)
    }

    /// The positional arguments, in declaration order.
    pub(crate) fn positionals(&self) -> impl Iterator<Item = &Arg> {
        self.args.iter().filter(|a| a.kind == ArgKind::Positional)
    }
}

impl fmt::Debug for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Command")
            .field("name", &self.name)
            .field("about", &self.about)
            .field("args", &self.args)
            .field("subcommands", &self.subcommands)
            .field("hidden", &self.hidden)
            .field("requires_auth", &self.requires_auth)
            .field("has_handler", &self.handler.is_some())
            .finish()
    }
}
