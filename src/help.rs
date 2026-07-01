//! Help rendering.
//!
//! Auto-generated help for the application and for any command, rendered through
//! the same output layer as everything else — section headers are styled, and on
//! a pipe or under `NO_COLOR` the whole thing degrades to plain text. The
//! injectable [`App::help_header`](crate::App::help_header) /
//! [`App::help_footer`](crate::App::help_footer) wrap every page.
//!
//! Commands marked [`hidden`](crate::Command::hidden) are always omitted from the
//! listings. With the `auth` feature, a
//! [`requires_auth`](crate::Command::requires_auth) command is also omitted unless
//! the auth hook authorizes it; without the feature the flag is inert and the
//! command is listed normally.

use crate::arg::{Arg, ArgKind};
use crate::command::Command;
use crate::parser::Cli;
use crate::style::style;

/// Render the top-level application help.
pub(crate) fn render_app(cli: &Cli) -> String {
    let mut out = String::new();
    push_header(&mut out, cli);

    let commands = visible(cli.commands, cli, &[]);
    let width = app_column_width(&commands, cli.version.is_some());

    out.push_str(&heading("USAGE:"));
    out.push_str(&format!(" {} <command> [options]\n", cli.app_name));

    if !commands.is_empty() {
        out.push('\n');
        out.push_str(&heading("COMMANDS:"));
        out.push('\n');
        for command in &commands {
            push_row(&mut out, &invocation(command), about(command), width);
        }
    }

    out.push('\n');
    out.push_str(&heading("OPTIONS:"));
    out.push('\n');
    push_row(&mut out, "-h, --help", "show this help", width);
    if cli.version.is_some() {
        push_row(&mut out, "-V, --version", "show the version", width);
    }

    push_footer(&mut out, cli);
    out
}

/// Render help for one command, reached via `path` (the command names from the
/// app root down to and including this command).
pub(crate) fn render_command(cli: &Cli, path: &[&str], command: &Command) -> String {
    let mut out = String::new();
    push_header(&mut out, cli);

    if let Some(text) = command.about.as_deref() {
        out.push_str(text);
        out.push_str("\n\n");
    }

    let positionals: Vec<&Arg> = command.positionals().collect();
    let options: Vec<&Arg> = command
        .args
        .iter()
        .filter(|a| a.kind != ArgKind::Positional)
        .collect();
    let subcommands = visible(&command.subcommands, cli, path);
    let width = command_column_width(&positionals, &options, &subcommands);

    // USAGE line.
    out.push_str(&heading("USAGE:"));
    out.push_str(&format!(" {} {}", cli.app_name, path.join(" ")));
    if !options.is_empty() {
        out.push_str(" [options]");
    }
    for arg in &positionals {
        out.push_str(&format!(" {}", positional_slot(arg)));
    }
    if !subcommands.is_empty() {
        out.push_str(" <command>");
    }
    out.push('\n');

    if !positionals.is_empty() {
        out.push('\n');
        out.push_str(&heading("ARGUMENTS:"));
        out.push('\n');
        for arg in &positionals {
            push_row(&mut out, &positional_slot(arg), help_text(arg), width);
        }
    }

    out.push('\n');
    out.push_str(&heading("OPTIONS:"));
    out.push('\n');
    for arg in &options {
        push_row(&mut out, &option_signature(arg), help_text(arg), width);
    }
    push_row(&mut out, "-h, --help", "show this help", width);

    if !subcommands.is_empty() {
        out.push('\n');
        out.push_str(&heading("COMMANDS:"));
        out.push('\n');
        for sub in &subcommands {
            push_row(&mut out, &invocation(sub), about(sub), width);
        }
    }

    push_footer(&mut out, cli);
    out
}

/// The commands to show in a listing: never hidden ones, and — with the `auth`
/// feature — auth-gated ones only when the hook authorizes them. `parent_path` is
/// the command-name chain leading to these commands, used to build the auth
/// request.
fn visible<'a>(commands: &'a [Command], cli: &Cli, parent_path: &[&str]) -> Vec<&'a Command> {
    commands
        .iter()
        .filter(|c| is_visible(c, cli, parent_path))
        .collect()
}

/// Without the `auth` feature, `requires_auth` is inert: only `hidden` hides a
/// command from help.
#[cfg(not(feature = "auth"))]
fn is_visible(command: &Command, _cli: &Cli, _parent_path: &[&str]) -> bool {
    !command.hidden
}

/// With the `auth` feature, an auth-gated command is listed only when the hook
/// authorizes it.
#[cfg(feature = "auth")]
fn is_visible(command: &Command, cli: &Cli, parent_path: &[&str]) -> bool {
    if command.hidden {
        return false;
    }
    if !command.requires_auth {
        return true;
    }
    let mut path: Vec<&str> = parent_path.to_vec();
    path.push(command.name.as_str());
    let request = crate::auth::AuthRequest::new(&path);
    cli.authorizer.is_some_and(|hook| hook(&request))
}

fn push_header(out: &mut String, cli: &Cli) {
    if let Some(text) = cli.header {
        out.push_str(text);
        out.push_str("\n\n");
    }
}

fn push_footer(out: &mut String, cli: &Cli) {
    if let Some(text) = cli.footer {
        out.push('\n');
        out.push_str(text);
        out.push('\n');
    }
}

/// A bold section header (plain when color is off).
fn heading(label: &str) -> String {
    style(label).bold().render()
}

/// A two-column row, left-padded to `width`, with no trailing whitespace.
fn push_row(out: &mut String, left: &str, right: &str, width: usize) {
    if right.is_empty() {
        out.push_str(&format!("  {left}\n"));
    } else {
        out.push_str(&format!("  {left:<width$}  {right}\n"));
    }
}

/// A command's invocation column: its name plus any aliases.
fn invocation(command: &Command) -> String {
    if command.aliases.is_empty() {
        command.name.clone()
    } else {
        format!("{}, {}", command.name, command.aliases.join(", "))
    }
}

fn about(command: &Command) -> &str {
    command.about.as_deref().unwrap_or("")
}

fn help_text(arg: &Arg) -> &str {
    arg.help.as_deref().unwrap_or("")
}

/// A positional's usage slot: `<name>` when required, `[name]` otherwise, with a
/// trailing `...` for a variadic (`multiple`) positional.
fn positional_slot(arg: &Arg) -> String {
    let slot = if arg.required && arg.default.is_none() {
        format!("<{}>", arg.name)
    } else {
        format!("[{}]", arg.name)
    };
    if arg.multiple {
        format!("{slot}...")
    } else {
        slot
    }
}

/// A flag/count/option's left column, e.g. `-o, --output <OUTPUT>`,
/// `-D, --define <DEFINE>...` (repeatable), or `-v, --verbose` (flag/count).
fn option_signature(arg: &Arg) -> String {
    let mut left = match arg.short {
        Some(c) => format!("-{c}, "),
        None => "    ".to_string(),
    };
    if let Some(long) = arg.long_name() {
        left.push_str(&format!("--{long}"));
    }
    if arg.kind == ArgKind::Option {
        left.push_str(&format!(" <{}>", arg.name.to_uppercase()));
        if arg.multiple {
            left.push_str("...");
        }
    }
    left
}

fn app_column_width(commands: &[&Command], has_version: bool) -> usize {
    let mut width = commands
        .iter()
        .map(|c| invocation(c).len())
        .max()
        .unwrap_or(0);
    width = width.max("-h, --help".len());
    if has_version {
        width = width.max("-V, --version".len());
    }
    width
}

fn command_column_width(positionals: &[&Arg], options: &[&Arg], subcommands: &[&Command]) -> usize {
    let positional = positionals.iter().map(|a| positional_slot(a).len());
    let option = options.iter().map(|a| option_signature(a).len());
    let sub = subcommands.iter().map(|c| invocation(c).len());
    positional
        .chain(option)
        .chain(sub)
        .chain(std::iter::once("-h, --help".len()))
        .max()
        .unwrap_or(0)
}
