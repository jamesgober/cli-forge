//! The argument parser.
//!
//! [`parse_command`] turns a slice of raw tokens into a [`Matches`] for a
//! command, recursing into subcommands. It handles the standard forms —
//! `--long`, `--long=value`, `--long value`, `-s`, `-s value`, `-svalue`,
//! bundled short flags `-abc`, positionals, and the `--` end-of-options marker —
//! and reports every malformed case as a structured [`ParseError`] rather than
//! panicking. `-h` / `--help` and `-V` / `--version` short-circuit into the
//! corresponding [`ParseError`] control signals, carrying rendered text.

use crate::arg::{Arg, ArgKind};
use crate::command::Command;
use crate::error::ParseError;
use crate::help;
use crate::matches::Matches;

/// The application context threaded through parsing, so a help or version
/// request can be rendered with the app name, header, footer, and version.
pub(crate) struct Cli<'a> {
    pub(crate) app_name: &'a str,
    pub(crate) header: Option<&'a str>,
    pub(crate) footer: Option<&'a str>,
    pub(crate) version: Option<&'a str>,
    pub(crate) commands: &'a [Command],
}

fn is_help(token: &str) -> bool {
    token == "-h" || token == "--help"
}

fn is_version(token: &str) -> bool {
    token == "-V" || token == "--version"
}

/// Resolve and parse a top-level invocation against the app's registered
/// commands. The app level has no arguments of its own: the first token selects
/// a command (by name or alias), and the rest are parsed by it.
pub(crate) fn parse_app(cli: &Cli, tokens: &[String]) -> Result<Matches, ParseError> {
    let mut matches = Matches::default();
    let first = match tokens.first() {
        Some(token) => token,
        None => return Ok(matches),
    };

    if is_help(first) {
        return Err(ParseError::HelpRequested(help::render_app(cli)));
    }
    if let Some(version) = cli.version {
        if is_version(first) {
            return Err(ParseError::VersionRequested(version.to_owned()));
        }
    }

    if first.len() > 1 && first.starts_with('-') {
        return Err(ParseError::UnknownFlag {
            flag: first.clone(),
        });
    }

    let command = cli
        .commands
        .iter()
        .find(|c| c.matches_name(first))
        .ok_or_else(|| ParseError::UnknownCommand {
            name: first.clone(),
        })?;
    let sub = parse_command(cli, &[command.name.as_str()], command, &tokens[1..])?;
    matches.subcommand = Some((command.name.clone(), Box::new(sub)));
    Ok(matches)
}

/// Parse `tokens` against `command`, recursing into any invoked subcommand.
/// `path` is the command-name chain from the app root down to `command`, used to
/// render the usage line if help is requested here.
pub(crate) fn parse_command(
    cli: &Cli,
    path: &[&str],
    command: &Command,
    tokens: &[String],
) -> Result<Matches, ParseError> {
    let mut matches = Matches::default();
    let positionals: Vec<&Arg> = command.positionals().collect();
    let mut next_positional = 0;
    let mut end_of_options = false;
    let mut i = 0;

    while i < tokens.len() {
        let token = &tokens[i];

        if !end_of_options {
            if token == "--" {
                end_of_options = true;
                i += 1;
                continue;
            }

            // Help / version short-circuit, unless the command defines a
            // conflicting argument of the same name.
            if is_help(token)
                && command.find_long("help").is_none()
                && command.find_short('h').is_none()
            {
                return Err(ParseError::HelpRequested(help::render_command(
                    cli, path, command,
                )));
            }
            if let Some(version) = cli.version {
                if is_version(token)
                    && command.find_long("version").is_none()
                    && command.find_short('V').is_none()
                {
                    return Err(ParseError::VersionRequested(version.to_owned()));
                }
            }

            if let Some(body) = token.strip_prefix("--") {
                i = parse_long(command, &mut matches, body, tokens, i)?;
                continue;
            }
            if token.len() > 1 && token.starts_with('-') {
                i = parse_short(command, &mut matches, token, tokens, i)?;
                continue;
            }
            if let Some(sub) = command.find_subcommand(token) {
                let mut sub_path = path.to_vec();
                sub_path.push(sub.name.as_str());
                let sub_matches = parse_command(cli, &sub_path, sub, &tokens[i + 1..])?;
                matches.subcommand = Some((sub.name.clone(), Box::new(sub_matches)));
                break;
            }
        }

        if next_positional < positionals.len() {
            let arg = positionals[next_positional];
            let _ = matches.values.insert(arg.name.clone(), token.clone());
            next_positional += 1;
            i += 1;
            continue;
        }

        return Err(if command.subcommands.is_empty() {
            ParseError::UnexpectedArgument {
                value: token.clone(),
            }
        } else {
            ParseError::UnknownCommand {
                name: token.clone(),
            }
        });
    }

    apply_defaults_and_required(command, &mut matches)?;
    Ok(matches)
}

/// Parse a `--long` token (possibly `--long=value`). Returns the next index.
fn parse_long(
    command: &Command,
    matches: &mut Matches,
    body: &str,
    tokens: &[String],
    i: usize,
) -> Result<usize, ParseError> {
    let (name, inline) = match body.split_once('=') {
        Some((name, value)) => (name, Some(value)),
        None => (body, None),
    };

    let arg = command
        .find_long(name)
        .ok_or_else(|| ParseError::UnknownFlag {
            flag: format!("--{body}"),
        })?;

    match arg.kind {
        ArgKind::Flag => {
            if inline.is_some() {
                return Err(ParseError::UnexpectedArgument {
                    value: format!("--{body}"),
                });
            }
            let _ = matches.flags.insert(arg.name.clone());
            Ok(i + 1)
        }
        ArgKind::Option => match inline {
            Some(value) => {
                let _ = matches.values.insert(arg.name.clone(), value.to_owned());
                Ok(i + 1)
            }
            None => {
                let value = tokens.get(i + 1).ok_or_else(|| ParseError::MissingValue {
                    option: arg.name.clone(),
                })?;
                let _ = matches.values.insert(arg.name.clone(), value.clone());
                Ok(i + 2)
            }
        },
        // `find_long` never returns a positional (positionals have no long form).
        ArgKind::Positional => Err(ParseError::UnknownFlag {
            flag: format!("--{body}"),
        }),
    }
}

/// Parse a `-short` token: a single flag, bundled flags `-abc`, or an option
/// with an attached or following value (`-o value` / `-ovalue`). Returns the
/// next index.
fn parse_short(
    command: &Command,
    matches: &mut Matches,
    token: &str,
    tokens: &[String],
    i: usize,
) -> Result<usize, ParseError> {
    let chars: Vec<char> = token[1..].chars().collect();
    let mut idx = 0;

    while idx < chars.len() {
        let c = chars[idx];
        let arg = command
            .find_short(c)
            .ok_or_else(|| ParseError::UnknownFlag {
                flag: format!("-{c}"),
            })?;

        match arg.kind {
            ArgKind::Flag => {
                let _ = matches.flags.insert(arg.name.clone());
                idx += 1;
            }
            ArgKind::Option => {
                let rest: String = chars[idx + 1..].iter().collect();
                if rest.is_empty() {
                    let value = tokens.get(i + 1).ok_or_else(|| ParseError::MissingValue {
                        option: arg.name.clone(),
                    })?;
                    let _ = matches.values.insert(arg.name.clone(), value.clone());
                    return Ok(i + 2);
                }
                let _ = matches.values.insert(arg.name.clone(), rest);
                return Ok(i + 1);
            }
            // `find_short` never returns a positional.
            ArgKind::Positional => {
                return Err(ParseError::UnknownFlag {
                    flag: format!("-{c}"),
                });
            }
        }
    }

    Ok(i + 1)
}

/// Fill in defaults for omitted options/positionals and verify required ones
/// were supplied.
fn apply_defaults_and_required(command: &Command, matches: &mut Matches) -> Result<(), ParseError> {
    for arg in &command.args {
        if arg.kind == ArgKind::Flag || matches.values.contains_key(&arg.name) {
            continue;
        }
        if let Some(default) = &arg.default {
            let _ = matches.values.insert(arg.name.clone(), default.clone());
        } else if arg.required {
            return Err(ParseError::MissingRequired {
                arg: arg.name.clone(),
            });
        }
    }
    Ok(())
}
