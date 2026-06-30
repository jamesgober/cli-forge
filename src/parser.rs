//! The argument parser.
//!
//! One function, [`parse_command`], turns a slice of raw tokens into a
//! [`Matches`] for a command, recursing into subcommands. It handles the
//! standard forms — `--long`, `--long=value`, `--long value`, `-s`, `-s value`,
//! `-svalue`, bundled short flags `-abc`, positionals, and the `--`
//! end-of-options marker — and reports every malformed case as a structured
//! [`ParseError`] rather than panicking.

use crate::arg::{Arg, ArgKind};
use crate::command::Command;
use crate::error::ParseError;
use crate::matches::Matches;

/// Resolve and parse a top-level invocation against an app's registered
/// commands. The app level has no arguments of its own: the first token selects
/// a command, and the rest are parsed by it.
pub(crate) fn parse_app(commands: &[Command], tokens: &[String]) -> Result<Matches, ParseError> {
    let mut matches = Matches::default();
    let first = match tokens.first() {
        Some(token) => token,
        None => return Ok(matches),
    };

    if first.len() > 1 && first.starts_with('-') {
        return Err(ParseError::UnknownFlag {
            flag: first.clone(),
        });
    }

    let command =
        commands
            .iter()
            .find(|c| &c.name == first)
            .ok_or_else(|| ParseError::UnknownCommand {
                name: first.clone(),
            })?;
    let sub = parse_command(command, &tokens[1..])?;
    matches.subcommand = Some((command.name.clone(), Box::new(sub)));
    Ok(matches)
}

/// Parse `tokens` against `command`, recursing into any invoked subcommand.
pub(crate) fn parse_command(command: &Command, tokens: &[String]) -> Result<Matches, ParseError> {
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
            if let Some(body) = token.strip_prefix("--") {
                i = parse_long(command, &mut matches, body, tokens, i)?;
                continue;
            }
            if token.len() > 1 && token.starts_with('-') {
                i = parse_short(command, &mut matches, token, tokens, i)?;
                continue;
            }
            if let Some(sub) = command.find_subcommand(token) {
                let sub_matches = parse_command(sub, &tokens[i + 1..])?;
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
