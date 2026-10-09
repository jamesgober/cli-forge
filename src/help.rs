//! Help rendering.
//!
//! Auto-generated help for the application and for any command, rendered through
//! the same output layer as everything else — section headings are styled, and on
//! a pipe or under `NO_COLOR` the whole page degrades to plain text. The
//! injectable slots ([`App::help_header`](crate::App::help_header),
//! [`App::help_footer`](crate::App::help_footer),
//! [`Command::before_help`](crate::Command::before_help),
//! [`Command::after_help`](crate::Command::after_help)) wrap every page.
//!
//! Three details make the difference between help that reads and help that
//! merely exists:
//!
//! - **Columns are measured in display columns, not bytes.** A command named
//!   `ünïcödé` or `日本語` lines up with its neighbours. `format!("{:<width$}")`
//!   counts bytes and skews every row after the first non-ASCII one.
//! - **Descriptions wrap to the terminal.** A long description folds under its
//!   own column instead of running off the right edge, and when the left column
//!   is wide enough to leave no room, the description moves to its own line
//!   rather than being squeezed into four characters.
//! - **Fallbacks are stated.** An argument's default, environment variable, and
//!   allowed values are appended to its description, because they are the part a
//!   reader most often came to find out.
//!
//! Commands marked [`hidden`](crate::Command::hidden) and arguments marked
//! [`hide`](crate::Arg::hide) are always omitted. With the `auth` feature, a
//! [`requires_auth`](crate::Command::requires_auth) command is also omitted
//! unless the auth hook authorizes it; without the feature the flag is inert and
//! the command is listed normally.

use crate::arg::{Arg, ArgKind};
use crate::command::Command;
use crate::parser::Cli;
use crate::style::style;
use crate::text::{self, Align};

/// The width help is laid out for when the terminal's is unknown.
const FALLBACK_WIDTH: u16 = 80;

/// The widest a page is laid out, however wide the terminal: prose past this is
/// harder to read, not easier.
const MAX_WIDTH: usize = 100;

/// The narrowest a description column is worth keeping. Below it, descriptions
/// move to their own line.
const MIN_DESCRIPTION: usize = 24;

/// The narrowest a page is laid out for, however narrow the terminal: below this
/// nothing would fit beside anything.
const MIN_WIDTH: usize = 40;

/// Two spaces of left margin, then the term, then two spaces before its
/// description.
const MARGIN: usize = 2;
const GUTTER: usize = 2;

/// The page width to lay out for.
fn page_width() -> usize {
    usize::from(terminal_width()).clamp(MIN_WIDTH, MAX_WIDTH)
}

#[cfg(feature = "std")]
fn terminal_width() -> u16 {
    crate::terminal::width_or(FALLBACK_WIDTH)
}

#[cfg(not(feature = "std"))]
fn terminal_width() -> u16 {
    FALLBACK_WIDTH
}

/// Render the top-level application help.
pub(crate) fn render_app(cli: &Cli) -> String {
    let mut out = String::new();
    let width = page_width();

    push_header(&mut out, cli);
    push_description(&mut out, cli.long_about.or(cli.about), width);

    let commands = visible(cli.commands, cli, &[]);
    let options = listable(cli.globals);
    let column = app_column_width(&commands, &options, cli.version.is_some());

    out.push_str(&heading("USAGE:"));
    out.push(' ');
    out.push_str(&usage_app(cli)[7..]);
    out.push('\n');

    let (plain_commands, command_sections) = sections(&commands, |c| c.category.as_deref());
    if !plain_commands.is_empty() {
        out.push('\n');
        out.push_str(&heading("COMMANDS:"));
        out.push('\n');
        for command in &plain_commands {
            push_row(
                &mut out,
                &invocation(command),
                about(command),
                column,
                width,
            );
        }
    }
    for (name, list) in &command_sections {
        out.push('\n');
        out.push_str(&section_heading(name));
        out.push('\n');
        for command in list {
            push_row(
                &mut out,
                &invocation(command),
                about(command),
                column,
                width,
            );
        }
    }

    let (plain_options, option_sections) = sections(&options, |a| a.category.as_deref());
    out.push('\n');
    out.push_str(&heading("OPTIONS:"));
    out.push('\n');
    for arg in &plain_options {
        push_row(
            &mut out,
            &option_signature(arg),
            &arg_description(arg),
            column,
            width,
        );
    }
    push_row(&mut out, "-h, --help", "show this help", column, width);
    if cli.version.is_some() {
        push_row(&mut out, "-V, --version", "show the version", column, width);
    }
    push_option_sections(&mut out, &option_sections, column, width);

    push_footer(&mut out, cli);
    out
}

/// Render help for one command, reached via `path` (the command names from the
/// app root down to and including this command).
pub(crate) fn render_command(cli: &Cli, path: &[&str], command: &Command) -> String {
    let mut out = String::new();
    let width = page_width();

    push_header(&mut out, cli);
    if let Some(text) = command.before_help.as_deref() {
        out.push_str(text);
        out.push_str("\n\n");
    }
    push_description(
        &mut out,
        command.long_about.as_deref().or(command.about.as_deref()),
        width,
    );

    let positionals: Vec<&Arg> = command.positionals().filter(|a| !a.hidden).collect();
    let flags: Vec<&Arg> = command
        .args
        .iter()
        .filter(|a| a.kind != ArgKind::Positional && !a.hidden)
        .collect();
    // The inherited app-level arguments are listed too: a user reading this page
    // needs to know the command accepts them.
    let inherited: Vec<&Arg> = cli
        .globals
        .iter()
        .filter(|arg| arg.global && !arg.hidden)
        .collect();
    let subcommands = visible(&command.subcommands, cli, path);
    let column = command_column_width(&positionals, &flags, &inherited, &subcommands);

    out.push_str(&heading("USAGE:"));
    out.push(' ');
    out.push_str(&usage_command(cli, path, command)[7..]);
    out.push('\n');

    if !positionals.is_empty() {
        out.push('\n');
        out.push_str(&heading("ARGUMENTS:"));
        out.push('\n');
        for arg in &positionals {
            push_row(
                &mut out,
                &positional_slot(arg),
                &arg_description(arg),
                column,
                width,
            );
        }
    }

    let all_options: Vec<&Arg> = flags.iter().chain(&inherited).copied().collect();
    let (plain_options, option_sections) = sections(&all_options, |a| a.category.as_deref());
    out.push('\n');
    out.push_str(&heading("OPTIONS:"));
    out.push('\n');
    for arg in &plain_options {
        push_row(
            &mut out,
            &option_signature(arg),
            &arg_description(arg),
            column,
            width,
        );
    }
    push_row(&mut out, "-h, --help", "show this help", column, width);
    push_option_sections(&mut out, &option_sections, column, width);

    let (plain_subs, sub_sections) = sections(&subcommands, |c| c.category.as_deref());
    if !plain_subs.is_empty() {
        out.push('\n');
        out.push_str(&heading("COMMANDS:"));
        out.push('\n');
        for sub in &plain_subs {
            push_row(&mut out, &invocation(sub), about(sub), column, width);
        }
    }
    for (name, list) in &sub_sections {
        out.push('\n');
        out.push_str(&section_heading(name));
        out.push('\n');
        for sub in list {
            push_row(&mut out, &invocation(sub), about(sub), column, width);
        }
    }

    if let Some(text) = command.after_help.as_deref() {
        out.push('\n');
        out.push_str(text);
        out.push('\n');
    }

    push_footer(&mut out, cli);
    out
}

/// The app's usage line, including the `USAGE: ` label in plain text.
///
/// Errors carry this so a refusal says what would have been accepted. It is plain
/// rather than styled because it is embedded in reports that render their own
/// styling.
pub(crate) fn usage_app(cli: &Cli) -> String {
    let mut line = crate::shim::format!("USAGE: {}", cli.app_name);
    if cli.globals.iter().any(|arg| !arg.hidden) {
        line.push_str(" [options]");
    }
    if !cli.commands.is_empty() {
        line.push_str(" <command>");
    }
    line
}

/// One command's usage line, including the `USAGE: ` label in plain text.
pub(crate) fn usage_command(cli: &Cli, path: &[&str], command: &Command) -> String {
    if let Some(custom) = command.usage.as_deref() {
        return crate::shim::format!("USAGE: {custom}");
    }
    let mut line = crate::shim::format!("USAGE: {} {}", cli.app_name, path.join(" "));
    let has_options = command
        .args
        .iter()
        .any(|a| a.kind != ArgKind::Positional && !a.hidden)
        || cli.globals.iter().any(|arg| arg.global && !arg.hidden);
    if has_options {
        line.push_str(" [options]");
    }
    for arg in command.positionals().filter(|a| !a.hidden) {
        line.push(' ');
        line.push_str(&positional_slot(arg));
    }
    // A required group is a choice the user must make, so it belongs in the
    // usage line rather than being discovered from an error.
    for group in command.groups.iter().filter(|g| g.required) {
        let choices: Vec<String> = group
            .members
            .iter()
            .map(|name| {
                command
                    .args
                    .iter()
                    .find(|arg| &arg.name == name)
                    .and_then(Arg::long_name)
                    .map_or_else(|| name.clone(), |long| crate::shim::format!("--{long}"))
            })
            .collect();
        line.push_str(" <");
        line.push_str(&choices.join("|"));
        line.push('>');
        if group.multiple {
            line.push_str("...");
        }
    }
    if !command.subcommands.is_empty() {
        line.push_str(if command.subcommand_required {
            " <command>"
        } else {
            " [command]"
        });
    }
    line
}

/// Split `items` into the uncategorised ones and the named sections, keeping
/// each section in the order its first item appears.
///
/// Order of appearance rather than alphabetical, so the program decides what
/// comes first — through declaration order, or `display_order` for commands.
fn sections<'a, T>(
    items: &[&'a T],
    category: impl Fn(&'a T) -> Option<&'a str>,
) -> (Vec<&'a T>, Vec<(&'a str, Vec<&'a T>)>) {
    let mut plain = Vec::new();
    let mut named: Vec<(&'a str, Vec<&'a T>)> = Vec::new();
    for &item in items {
        match category(item) {
            None => plain.push(item),
            Some(name) => match named.iter_mut().find(|(existing, _)| *existing == name) {
                Some((_, list)) => list.push(item),
                None => named.push((name, vec![item])),
            },
        }
    }
    (plain, named)
}

/// The heading for a named section, in the same shape as the built-in ones.
fn section_heading(name: &str) -> String {
    heading(&crate::shim::format!("{}:", name.to_uppercase()))
}

/// The commands to show in a listing: never hidden ones, and — with the `auth`
/// feature — auth-gated ones only when the hook authorizes them.
///
/// `parent_path` is the command-name chain leading to these commands, used to
/// build the auth request. The result is ordered by
/// [`display_order`](crate::Command::display_order) and then by registration,
/// which a stable sort gives for free.
fn visible<'a>(commands: &'a [Command], cli: &Cli, parent_path: &[&str]) -> Vec<&'a Command> {
    let mut listed: Vec<&Command> = commands
        .iter()
        .filter(|c| is_visible(c, cli, parent_path))
        .collect();
    listed.sort_by_key(|c| c.order);
    listed
}

/// The arguments worth listing: everything not hidden.
fn listable(args: &[Arg]) -> Vec<&Arg> {
    args.iter().filter(|arg| !arg.hidden).collect()
}

/// Without the `auth` feature, `requires_auth` is inert: only `hidden` hides a
/// command from help.
#[cfg(not(feature = "auth"))]
fn is_visible(command: &Command, _cli: &Cli, _parent_path: &[&str]) -> bool {
    !command.hidden
}

/// With the `auth` feature, an auth-gated command is listed only when the hook
/// authorizes it — so a user who cannot run a command is not told it exists.
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

/// Write each named option section after the default one.
fn push_option_sections(
    out: &mut String,
    named: &[(&str, Vec<&Arg>)],
    column: usize,
    width: usize,
) {
    for (name, list) in named {
        out.push('\n');
        out.push_str(&section_heading(name));
        out.push('\n');
        for arg in list {
            push_row(
                out,
                &option_signature(arg),
                &arg_description(arg),
                column,
                width,
            );
        }
    }
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

/// Write a description paragraph, wrapped to the page.
fn push_description(out: &mut String, text: Option<&str>, width: usize) {
    let Some(text) = text else {
        return;
    };
    for line in text::wrap(text, width) {
        out.push_str(line);
        out.push('\n');
    }
    out.push('\n');
}

/// A bold section heading (plain when colour is off).
fn heading(label: &str) -> String {
    style(label).bold().render()
}

/// A two-column row: the term, then its description, wrapped and aligned under
/// its own column.
///
/// When the term column leaves less than [`MIN_DESCRIPTION`] for the text, the
/// description moves to its own indented line instead of being crushed.
fn push_row(out: &mut String, left: &str, right: &str, column: usize, page: usize) {
    if right.is_empty() {
        out.push_str("  ");
        out.push_str(left);
        out.push('\n');
        return;
    }

    let indent = MARGIN + column + GUTTER;
    let available = page.saturating_sub(indent);

    if available < MIN_DESCRIPTION {
        // No room beside the term, so put the description under it.
        out.push_str("  ");
        out.push_str(left);
        out.push('\n');
        for line in text::wrap(right, page.saturating_sub(MARGIN + GUTTER).max(1)) {
            out.push_str(&" ".repeat(MARGIN + GUTTER));
            out.push_str(line);
            out.push('\n');
        }
        return;
    }

    let mut lines = text::wrap(right, available).into_iter();
    let first = lines.next().unwrap_or("");
    out.push_str("  ");
    out.push_str(&text::pad(left, column, Align::Left));
    out.push_str("  ");
    out.push_str(first);
    out.push('\n');
    for line in lines {
        out.push_str(&" ".repeat(indent));
        out.push_str(line);
        out.push('\n');
    }
}

/// A command's invocation column: its name plus any aliases.
fn invocation(command: &Command) -> String {
    if command.aliases.is_empty() {
        command.name.clone()
    } else {
        crate::shim::format!("{}, {}", command.name, command.aliases.join(", "))
    }
}

fn about(command: &Command) -> &str {
    command.about.as_deref().unwrap_or("")
}

/// An argument's description: its help text, then whatever fallbacks and
/// restrictions it has, which is the part readers most often came for.
fn arg_description(arg: &Arg) -> String {
    let mut text = arg.help.as_deref().unwrap_or("").to_owned();
    let mut note = |label: &str, value: &str| {
        if !text.is_empty() {
            text.push(' ');
        }
        text.push('[');
        text.push_str(label);
        text.push_str(": ");
        text.push_str(value);
        text.push(']');
    };

    if let Some(delimiter) = arg.delimiter {
        note("delimiter", &crate::shim::format!("'{delimiter}'"));
    }
    if !arg.possible.is_empty() {
        note("possible", &arg.possible.join(", "));
    }
    if let Some(default) = &arg.default {
        note("default", default);
    }
    if let Some(env) = &arg.env {
        note("env", env);
    }
    text
}

/// A positional's usage slot: `<name>` when required, `[name]` otherwise, with a
/// trailing `...` for a variadic one.
fn positional_slot(arg: &Arg) -> String {
    let name = arg.placeholder_lower();
    let slot = if arg.required && arg.default.is_none() {
        crate::shim::format!("<{name}>")
    } else {
        crate::shim::format!("[{name}]")
    };
    if arg.multiple {
        crate::shim::format!("{slot}...")
    } else {
        slot
    }
}

/// A flag, count, or option's left column, e.g. `-o, --output <FILE>` or
/// `-D, --define <KEY=VALUE>...`.
fn option_signature(arg: &Arg) -> String {
    let mut left = match arg.short {
        Some(c) => crate::shim::format!("-{c}, "),
        // Four spaces, so the long forms of the short-less arguments still line
        // up with the others.
        None => "    ".to_owned(),
    };
    if let Some(long) = arg.long_name() {
        left.push_str("--");
        // The conventional way to say one entry accepts both spellings.
        if arg.is_negatable() {
            left.push_str("[no-]");
        }
        left.push_str(long);
    }
    if arg.kind == ArgKind::Option {
        left.push_str(" <");
        left.push_str(&arg.placeholder());
        left.push('>');
        if arg.multiple {
            left.push_str("...");
        }
    }
    left
}

fn app_column_width(commands: &[&Command], options: &[&Arg], has_version: bool) -> usize {
    let from_commands = commands.iter().map(|c| text::width(&invocation(c)));
    let from_options = options.iter().map(|a| text::width(&option_signature(a)));
    let mut width = from_commands
        .chain(from_options)
        .max()
        .unwrap_or(0)
        .max(text::width("-h, --help"));
    if has_version {
        width = width.max(text::width("-V, --version"));
    }
    width
}

fn command_column_width(
    positionals: &[&Arg],
    options: &[&Arg],
    inherited: &[&Arg],
    subcommands: &[&Command],
) -> usize {
    let from_positionals = positionals.iter().map(|a| text::width(&positional_slot(a)));
    let from_options = options
        .iter()
        .chain(inherited)
        .map(|a| text::width(&option_signature(a)));
    let from_subcommands = subcommands.iter().map(|c| text::width(&invocation(c)));
    from_positionals
        .chain(from_options)
        .chain(from_subcommands)
        .chain(core::iter::once(text::width("-h, --help")))
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_a_row_wraps_its_description_under_its_own_column() {
        let mut out = String::new();
        push_row(
            &mut out,
            "-j, --jobs <N>",
            "the number of parallel jobs to run, which defaults to the number of cores",
            14,
            48,
        );
        let lines: Vec<&str> = out.lines().collect();
        assert!(lines.len() > 1, "expected wrapping: {out:?}");
        assert!(lines[0].starts_with("  -j, --jobs <N>  the number"));
        // Continuation lines are indented to the description column.
        for line in &lines[1..] {
            assert!(
                line.starts_with(&" ".repeat(MARGIN + 14 + GUTTER)),
                "{line:?}"
            );
            assert!(text::width(line) <= 48, "{line:?} exceeds the page");
        }
    }

    #[test]
    fn test_a_row_with_no_room_puts_the_description_underneath() {
        let mut out = String::new();
        // A term so wide that nothing useful fits beside it.
        push_row(
            &mut out,
            "--an-extremely-long-option-name",
            "the description",
            31,
            40,
        );
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "  --an-extremely-long-option-name");
        assert_eq!(lines[1], "    the description");
    }

    #[test]
    fn test_a_row_without_a_description_has_no_trailing_space() {
        let mut out = String::new();
        push_row(&mut out, "build", "", 20, 80);
        assert_eq!(out, "  build\n");
    }

    #[test]
    fn test_columns_are_measured_in_display_columns() {
        // The confirmed bug this replaced: byte lengths skewed every row after a
        // non-ASCII one.
        let commands = [
            Command::new("ünïcödé"),
            Command::new("日本語"),
            Command::new("ab"),
        ];
        let refs: Vec<&Command> = commands.iter().collect();
        let width = app_column_width(&refs, &[], false);
        // `日本語` is three characters but six columns, and the widest here.
        assert_eq!(width, text::width("-h, --help").max(6).max(7));
    }

    #[test]
    fn test_positional_slot_shows_what_is_required_and_repeatable() {
        assert_eq!(
            positional_slot(&Arg::positional("path").required(true)),
            "<path>"
        );
        assert_eq!(positional_slot(&Arg::positional("path")), "[path]");
        // A default makes it optional however it was marked.
        assert_eq!(
            positional_slot(&Arg::positional("path").required(true).default(".")),
            "[path]"
        );
        assert_eq!(
            positional_slot(&Arg::positional("files").multiple(true).required(true)),
            "<files>..."
        );
    }

    #[test]
    fn test_option_signature_forms() {
        assert_eq!(
            option_signature(&Arg::flag("force").short('f')),
            "-f, --force"
        );
        assert_eq!(option_signature(&Arg::flag("force")), "    --force");
        assert_eq!(
            option_signature(&Arg::option("output").short('o')),
            "-o, --output <OUTPUT>"
        );
        assert_eq!(
            option_signature(&Arg::option("output").value_name("FILE")),
            "    --output <FILE>"
        );
        assert_eq!(
            option_signature(&Arg::option("define").short('D').multiple(true)),
            "-D, --define <DEFINE>..."
        );
    }

    #[test]
    fn test_arg_description_states_the_fallbacks() {
        let arg = Arg::option("level")
            .help("how much to log")
            .possible_values(["warn", "info"])
            .default("warn")
            .env("FORGE_LEVEL");
        let description = arg_description(&arg);
        assert_eq!(
            description,
            "how much to log [possible: warn, info] [default: warn] [env: FORGE_LEVEL]"
        );
        // And says nothing when there is nothing to say.
        assert_eq!(arg_description(&Arg::flag("force")), "");
    }

    #[test]
    fn test_hidden_arguments_are_not_listed() {
        let args = [Arg::flag("shown"), Arg::flag("secret").hide(true)];
        let listed = listable(&args);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name(), "shown");
    }
}
