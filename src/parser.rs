//! The argument parser.
//!
//! [`parse_app`] turns a slice of raw tokens into a [`Matches`], resolving the
//! command, recursing into subcommands, and filling in what the command line did
//! not say. It handles the standard forms — `--long`, `--long=value`,
//! `--long value`, `-s`, `-s value`, `-svalue`, bundled short flags `-abc`,
//! counting flags `-vvv`, repeatable options, variadic positionals, and the `--`
//! end-of-options marker — and reports every malformed case as a structured
//! [`ParseError`] rather than panicking.
//!
//! Five behaviours here are worth knowing about, because each is a case the
//! obvious implementation gets wrong:
//!
//! - **Negative numbers are values, not flags.** `calc add -5 3` has to work. A
//!   token beginning with `-` is only treated as a flag when it could be one:
//!   `-5` is a value unless the command actually declares a short `-5`.
//! - **`-` alone is a value.** The long-standing convention for "read standard
//!   input", so it must not be mistaken for an empty flag.
//! - **A global argument has one slot, wherever it was written.** `-v build`,
//!   `build -v`, and `-v build -v` all mean the same thing, and the result is
//!   visible at every level — so a handler never walks back up the tree to find
//!   out how verbose it was asked to be.
//! - **A parent's required arguments are not required once a subcommand takes
//!   over.** `remote list` cannot be expected to also supply `remote`'s own
//!   positionals, because the subcommand name occupies that position.
//! - **Precedence for a value is command line, then environment, then default.**
//!   Anything else surprises someone, and which one happened is recorded on the
//!   value so the program can tell.

use crate::arg::{Arg, ArgKind, Written};
use crate::command::Command;
use crate::error::{ErrorKind, ParseError, did_you_mean};
use crate::help;
use crate::matches::{Matches, ValueSource};

/// The application context threaded through parsing, so any level can render
/// help, resolve a global argument, or name the program in an error.
pub(crate) struct Cli<'a> {
    pub(crate) app_name: &'a str,
    pub(crate) about: Option<&'a str>,
    pub(crate) long_about: Option<&'a str>,
    pub(crate) header: Option<&'a str>,
    pub(crate) footer: Option<&'a str>,
    pub(crate) version: Option<&'a str>,
    pub(crate) commands: &'a [Command],
    /// The app-level arguments. Those marked [`Arg::global`] are also accepted by
    /// every subcommand.
    pub(crate) globals: &'a [Arg],
    /// Whether `prog help [command]` is accepted, and whether a bare invocation
    /// shows the help.
    pub(crate) help_command: bool,
    /// The authorization hook, consulted when generating help so auth-gated
    /// commands appear only when authorized.
    #[cfg(feature = "auth")]
    pub(crate) authorizer: Option<&'a crate::auth::AuthHook>,
}

impl<'a> Cli<'a> {
    /// The app-level arguments a subcommand inherits.
    ///
    /// Borrowed rather than cloned, and built once per parse rather than once per
    /// command level: `parse_command` recurses, so cloning here charged every
    /// level for a deep copy of every global argument.
    fn inherited(&self) -> Vec<&'a Arg> {
        self.globals.iter().filter(|arg| arg.global).collect()
    }

    /// Every app-level argument, inherited or not. Only the app level sees these.
    fn app_level(&self) -> Vec<&'a Arg> {
        self.globals.iter().collect()
    }
}

/// Where a parsed value goes.
///
/// A global argument is recorded in one shared slot rather than in whichever
/// level it happened to be written at; everything else goes to the level being
/// parsed. Routing it here, once, is what keeps the rest of the parser from
/// having to care which side of the command name a flag appeared on.
struct Sink<'m> {
    level: &'m mut Matches,
    globals: &'m mut Matches,
}

impl Sink<'_> {
    /// The matches `arg`'s value belongs in.
    fn target(&mut self, arg: &Arg) -> &mut Matches {
        if arg.global { self.globals } else { self.level }
    }

    /// Whether `name` has a value in either slot.
    fn present(&self, name: &str) -> bool {
        self.level.present(name) || self.globals.present(name)
    }

    /// Where `name`'s value came from, looking in both slots.
    fn source(&self, name: &str) -> Option<ValueSource> {
        self.level
            .source(name)
            .or_else(|| self.globals.source(name))
    }

    /// Whether the flag `name` is set in either slot.
    fn flag(&self, name: &str) -> bool {
        self.level.flag(name) || self.globals.flag(name)
    }

    /// Whether the flag `name` was explicitly turned off in either slot.
    fn negated(&self, name: &str) -> bool {
        self.level.negated.contains(name) || self.globals.negated.contains(name)
    }

    /// Whether `name` was supplied on the command line *and* left on.
    ///
    /// The test for taking part in a conflict: `--no-verbose --quiet` must not
    /// be refused as "verbose conflicts with quiet", because the user said the
    /// opposite of verbose.
    fn asserted(&self, name: &str) -> bool {
        self.source(name) == Some(ValueSource::CommandLine) && !self.negated(name)
    }
}

fn is_help(token: &str) -> bool {
    token == "-h" || token == "--help"
}

fn is_version(token: &str) -> bool {
    token == "-V" || token == "--version"
}

/// Whether `token` should be read as a flag rather than as a value.
///
/// A leading `-` is not enough. `-` alone is the read-standard-input convention,
/// and `-5` or `-.5` is a number unless the command really does declare a short
/// flag by that character — which is the distinction that makes a calculator or a
/// seek offset expressible at all.
fn is_flag_like(token: &str, command: &Command, globals: &[&Arg]) -> bool {
    if token == "-" || !token.starts_with('-') || token.len() < 2 {
        return false;
    }
    if token.starts_with("--") {
        return true;
    }
    let Some(first) = token[1..].chars().next() else {
        return false;
    };
    if first.is_ascii_digit() || first == '.' {
        // Only a declared short flag reclaims it.
        return command.find_short(first, globals).is_some();
    }
    true
}

/// Record a value for `arg` after checking it, replacing for a single-valued
/// argument (last wins) and appending for a `multiple` one.
fn record(
    matches: &mut Matches,
    arg: &Arg,
    value: String,
    written: Written<'_>,
    source: ValueSource,
) -> Result<(), ParseError> {
    if let Some(delimiter) = arg.delimiter {
        // Every piece is checked before any is stored, so a bad piece leaves no
        // partial list behind.
        for piece in value.split(delimiter) {
            arg.check(piece, written)?;
        }
        let list = matches.values.entry(arg.name.clone()).or_default();
        list.extend(value.split(delimiter).map(str::to_owned));
        let _ = matches.sources.insert(arg.name.clone(), source);
        return Ok(());
    }

    arg.check(&value, written)?;
    if arg.multiple {
        matches
            .values
            .entry(arg.name.clone())
            .or_default()
            .push(value);
    } else {
        let _ = matches.values.insert(arg.name.clone(), vec![value]);
    }
    let _ = matches.sources.insert(arg.name.clone(), source);
    Ok(())
}

/// Increment a counting flag's tally, saturating rather than overflowing on
/// pathological input.
///
/// Looks the key up before reaching for `entry`, because `entry` needs an owned
/// key and `-vvv` would otherwise allocate the name once per repeat.
fn bump_count(matches: &mut Matches, name: &str) {
    if let Some(counter) = matches.counts.get_mut(name) {
        *counter = counter.saturating_add(1);
        return;
    }
    let _ = matches.counts.insert(name.to_owned(), 1);
}

/// Mark a boolean flag as set, undoing any earlier `--no-NAME` so the last one
/// written wins.
fn set_flag(matches: &mut Matches, arg: &Arg, source: ValueSource) {
    let _ = matches.negated.remove(&arg.name);
    let _ = matches.flags.insert(arg.name.clone());
    let _ = matches.sources.insert(arg.name.clone(), source);
}

/// Mark a negatable flag as explicitly off, undoing any earlier `--NAME`.
fn negate(matches: &mut Matches, arg: &Arg, source: ValueSource) {
    let _ = matches.flags.remove(&arg.name);
    let _ = matches.negated.insert(arg.name.clone());
    let _ = matches.sources.insert(arg.name.clone(), source);
}

/// Resolve and parse a top-level invocation.
///
/// The app level accepts its own arguments before the command name, then the
/// first bare token selects a command by name or alias and the rest are parsed by
/// it.
pub(crate) fn parse_app(cli: &Cli, tokens: &[String]) -> Result<Matches, ParseError> {
    let mut root = Matches::default();
    let mut globals = Matches::default();
    // A synthetic command standing in for the app level, so one code path
    // handles `--long value` whether it was written before or after a command.
    let app_level = Command::new(cli.app_name);
    // Both sets are built once here: the app level accepts all of its own
    // arguments, and subcommands inherit the ones marked global.
    let app_args = cli.app_level();
    let inherited = cli.inherited();
    let mut command_parsed: Option<(String, Matches)> = None;
    let mut i = 0;
    let mut end_of_options = false;

    while i < tokens.len() {
        let token = &tokens[i];

        if !end_of_options {
            if token == "--" {
                end_of_options = true;
                i += 1;
                continue;
            }
            if is_help(token) {
                return Err(ParseError::request(
                    ErrorKind::HelpRequested,
                    help::render_app(cli),
                ));
            }
            if let Some(version) = cli.version {
                if is_version(token) {
                    return Err(ParseError::request(ErrorKind::VersionRequested, version));
                }
            }
            if is_flag_like(token, &app_level, &app_args) {
                let mut sink = Sink {
                    level: &mut root,
                    globals: &mut globals,
                };
                i = parse_flag(cli, &app_level, &app_args, &mut sink, tokens, i)
                    .map_err(|error| error.with_usage(help::usage_app(cli)))?;
                continue;
            }
            // `prog help [command...]` renders help, unless the program declares
            // a command of its own by that name.
            if cli.help_command
                && token == "help"
                && !cli.commands.iter().any(|c| c.matches_name("help"))
            {
                return Err(help_for_path(cli, &tokens[i + 1..]));
            }
        }

        let command = resolve(cli, token)?;
        let sub = parse_command(
            cli,
            &[command.name.as_str()],
            command,
            &tokens[i + 1..],
            &mut globals,
            &inherited,
        )?;
        command_parsed = Some((command.name.clone(), sub));
        break;
    }

    // The app's own arguments and the global ones are both finalized once, here:
    // a global has a single slot, so filling it in at every level would mean
    // doing it several times and disagreeing about the result.
    {
        let mut sink = Sink {
            level: &mut root,
            globals: &mut globals,
        };
        finalize(&app_level, &app_args, &mut sink, false)
            .map_err(|error| error.with_usage(help::usage_app(cli)))?;
    }

    if let Some((name, sub)) = command_parsed {
        root.subcommand = Some((name, Box::new(sub)));
    } else if cli.help_command && !cli.commands.is_empty() && tokens.is_empty() {
        // Nothing to do and nothing said: showing the help is more use than
        // exiting silently, which leaves the user none the wiser.
        return Err(ParseError::request(
            ErrorKind::HelpRequested,
            help::render_app(cli),
        ));
    }

    root.merge_globals(&globals);
    Ok(root)
}

/// Find the command `token` names, or explain what was meant instead.
fn resolve<'a>(cli: &'a Cli, token: &str) -> Result<&'a Command, ParseError> {
    if let Some(command) = cli.commands.iter().find(|c| c.matches_name(token)) {
        return Ok(command);
    }
    let kind = if token.starts_with('-') {
        ErrorKind::UnknownFlag
    } else {
        ErrorKind::UnknownCommand
    };
    let mut error = ParseError::new(kind, token).with_usage(help::usage_app(cli));
    if let Some(nearest) = did_you_mean(
        token,
        cli.commands.iter().flat_map(Command::invocation_names),
    ) {
        error = error.with_suggestion(nearest);
    }
    Err(error)
}

/// Render help for the command path `rest` names, for `prog help remote add`.
fn help_for_path(cli: &Cli, rest: &[String]) -> ParseError {
    let mut path: Vec<&str> = Vec::new();
    let mut current: Option<&Command> = None;

    for name in rest {
        let next = match current {
            None => cli.commands.iter().find(|c| c.matches_name(name)),
            Some(command) => command.find_subcommand(name),
        };
        match next {
            Some(command) => {
                path.push(command.name.as_str());
                current = Some(command);
            }
            // An unknown name in a help request is not worth an error: the app
            // help is what the user needs to see anyway.
            None => return ParseError::request(ErrorKind::HelpRequested, help::render_app(cli)),
        }
    }

    match current {
        Some(command) => ParseError::request(
            ErrorKind::HelpRequested,
            help::render_command(cli, &path, command),
        ),
        None => ParseError::request(ErrorKind::HelpRequested, help::render_app(cli)),
    }
}

/// Parse `tokens` against `command`, recursing into any invoked subcommand.
///
/// `path` is the command-name chain from the app root down to `command`, used for
/// the usage line in help and in errors. `globals` is the shared slot every
/// global argument is recorded in, whatever level it was written at.
pub(crate) fn parse_command(
    cli: &Cli,
    path: &[&str],
    command: &Command,
    tokens: &[String],
    globals: &mut Matches,
    inherited: &[&Arg],
) -> Result<Matches, ParseError> {
    let usage = || help::usage_command(cli, path, command);

    let mut level = Matches::default();
    let positionals: Vec<&Arg> = command.positionals().collect();
    let mut next_positional = 0;
    let mut end_of_options = false;
    let mut subcommand: Option<(String, Matches)> = None;
    let mut i = 0;

    while i < tokens.len() {
        let token = &tokens[i];

        if !end_of_options {
            if token == "--" {
                end_of_options = true;
                i += 1;
                continue;
            }

            // Help and version short-circuit, unless the command declares a
            // conflicting argument by the same name.
            if is_help(token)
                && command.find_long("help", inherited).is_none()
                && command.find_short('h', inherited).is_none()
            {
                return Err(ParseError::request(
                    ErrorKind::HelpRequested,
                    help::render_command(cli, path, command),
                ));
            }
            if let Some(version) = cli.version {
                if is_version(token)
                    && command.find_long("version", inherited).is_none()
                    && command.find_short('V', inherited).is_none()
                {
                    return Err(ParseError::request(ErrorKind::VersionRequested, version));
                }
            }

            if is_flag_like(token, command, inherited) {
                let mut sink = Sink {
                    level: &mut level,
                    globals,
                };
                i = parse_flag(cli, command, inherited, &mut sink, tokens, i)
                    .map_err(|error| error.with_usage(usage()))?;
                continue;
            }

            // A subcommand name claims the rest of the tokens — but only once no
            // positional of this command is still waiting, so a command whose
            // argument happens to share a subcommand's name still parses.
            if next_positional >= positionals.len() {
                if let Some(sub) = command.find_subcommand(token) {
                    let mut sub_path = path.to_vec();
                    sub_path.push(sub.name.as_str());
                    let sub_matches =
                        parse_command(cli, &sub_path, sub, &tokens[i + 1..], globals, inherited)?;
                    subcommand = Some((sub.name.clone(), sub_matches));
                    break;
                }
            }
        }

        if next_positional < positionals.len() {
            let arg = positionals[next_positional];
            let target = if arg.global {
                &mut *globals
            } else {
                &mut level
            };
            record(
                target,
                arg,
                token.clone(),
                Written::Name(&arg.name),
                ValueSource::CommandLine,
            )
            .map_err(|error| error.with_usage(usage()))?;
            // A variadic positional keeps absorbing the remaining bare values.
            if !arg.multiple {
                next_positional += 1;
            }
            i += 1;
            continue;
        }

        return Err(surplus(command, token).with_usage(usage()));
    }

    let subcommand_invoked = subcommand.is_some();
    if command.subcommand_required && !subcommand_invoked {
        return Err(missing_subcommand(command).with_usage(usage()));
    }

    {
        let declared: Vec<&Arg> = command.args.iter().collect();
        let mut sink = Sink {
            level: &mut level,
            globals,
        };
        finalize(command, &declared, &mut sink, subcommand_invoked)
            .map_err(|error| error.with_usage(usage()))?;
    }

    if let Some((name, sub)) = subcommand {
        level.subcommand = Some((name, Box::new(sub)));
    }
    Ok(level)
}

/// Explain a command invoked bare that needs a subcommand.
fn missing_subcommand(command: &Command) -> ParseError {
    let available: Vec<&str> = command
        .subcommands
        .iter()
        .filter(|c| !c.hidden)
        .map(|c| c.name.as_str())
        .collect();
    ParseError::new(ErrorKind::MissingSubcommand, &command.name).with_detail(crate::shim::format!(
        "expected one of: {}",
        available.join(", ")
    ))
}

/// Explain a bare value that nothing can accept.
fn surplus(command: &Command, token: &str) -> ParseError {
    if command.subcommands.is_empty() {
        return ParseError::new(ErrorKind::UnexpectedArgument, token);
    }
    let mut error = ParseError::new(ErrorKind::UnknownCommand, token);
    if let Some(nearest) = did_you_mean(
        token,
        command
            .subcommands
            .iter()
            .flat_map(Command::invocation_names),
    ) {
        error = error.with_suggestion(nearest);
    }
    error
}

/// Parse one `--long` or `-short` token. Returns the index of the next token.
fn parse_flag(
    cli: &Cli,
    command: &Command,
    extra: &[&Arg],
    sink: &mut Sink,
    tokens: &[String],
    i: usize,
) -> Result<usize, ParseError> {
    let token = &tokens[i];
    match token.strip_prefix("--") {
        Some(body) => parse_long(cli, command, extra, sink, body, tokens, i),
        None => parse_short(command, extra, sink, token, tokens, i),
    }
}

/// Parse a `--long` token, possibly `--long=value`.
fn parse_long(
    cli: &Cli,
    command: &Command,
    extra: &[&Arg],
    sink: &mut Sink,
    body: &str,
    tokens: &[String],
    i: usize,
) -> Result<usize, ParseError> {
    let (name, inline) = match body.split_once('=') {
        Some((name, value)) => (name, Some(value)),
        None => (body, None),
    };
    let written = Written::Long(name);

    // `--no-NAME` on a negatable flag. Checked only once `NAME` itself has
    // failed to match, so an argument genuinely called `no-cache` still wins.
    if command.find_long(name, extra).is_none() {
        if let Some(positive) = name.strip_prefix("no-") {
            if let Some(arg) = command
                .find_long(positive, extra)
                .filter(|arg| arg.is_negatable())
            {
                if inline.is_some() {
                    return Err(ParseError::new(
                        ErrorKind::UnexpectedArgument,
                        crate::shim::format!("--{body}"),
                    )
                    .with_detail(crate::shim::format!("'--{name}' takes no value")));
                }
                negate(sink.target(arg), arg, ValueSource::CommandLine);
                return Ok(i + 1);
            }
        }
    }

    let Some(arg) = command.find_long(name, extra) else {
        let mut error = ParseError::new(ErrorKind::UnknownFlag, crate::shim::format!("--{body}"));
        let candidates: Vec<String> = command
            .args
            .iter()
            .chain(extra.iter().copied())
            .filter(|a| !a.hidden)
            .filter_map(Arg::long_name)
            .map(crate::shim::ToString::to_string)
            .chain(
                command
                    .args
                    .iter()
                    .chain(extra.iter().copied())
                    .filter(|a| !a.hidden && a.is_negatable())
                    .filter_map(Arg::long_name)
                    .map(|long| crate::shim::format!("no-{long}")),
            )
            .chain(["help".to_owned()])
            .chain(cli.version.map(|_| "version".to_owned()))
            .collect();
        if let Some(nearest) = did_you_mean(name, candidates.iter().map(String::as_str)) {
            error = error.with_suggestion(crate::shim::format!("--{nearest}"));
        }
        return Err(error);
    };

    match arg.kind {
        ArgKind::Flag | ArgKind::Count => {
            if inline.is_some() {
                return Err(ParseError::new(
                    ErrorKind::UnexpectedArgument,
                    crate::shim::format!("--{body}"),
                )
                .with_detail(crate::shim::format!("'--{name}' takes no value")));
            }
            let kind = arg.kind;
            let target = sink.target(arg);
            if kind == ArgKind::Count {
                bump_count(target, &arg.name);
            } else {
                set_flag(target, arg, ValueSource::CommandLine);
            }
            Ok(i + 1)
        }
        ArgKind::Option => match inline {
            Some(value) => {
                let owned = value.to_owned();
                record(
                    sink.target(arg),
                    arg,
                    owned,
                    written,
                    ValueSource::CommandLine,
                )?;
                Ok(i + 1)
            }
            None => {
                let value = tokens
                    .get(i + 1)
                    .ok_or_else(|| ParseError::new(ErrorKind::MissingValue, written.display()))?
                    .clone();
                record(
                    sink.target(arg),
                    arg,
                    value,
                    written,
                    ValueSource::CommandLine,
                )?;
                Ok(i + 2)
            }
        },
        // `find_long` never returns a positional: they have no long form.
        ArgKind::Positional => Err(ParseError::new(
            ErrorKind::UnknownFlag,
            crate::shim::format!("--{body}"),
        )),
    }
}

/// Parse a `-short` token: a single flag, bundled flags or counts (`-abc`,
/// `-vvv`), or an option with an attached or following value (`-o value`,
/// `-ovalue`).
fn parse_short(
    command: &Command,
    extra: &[&Arg],
    sink: &mut Sink,
    token: &str,
    tokens: &[String],
    i: usize,
) -> Result<usize, ParseError> {
    let chars: Vec<char> = token[1..].chars().collect();
    let mut idx = 0;

    while idx < chars.len() {
        let c = chars[idx];
        let written = Written::Short(c);
        let Some(arg) = command.find_short(c, extra) else {
            return Err(ParseError::new(
                ErrorKind::UnknownFlag,
                crate::shim::format!("-{c}"),
            ));
        };

        match arg.kind {
            ArgKind::Flag => {
                set_flag(sink.target(arg), arg, ValueSource::CommandLine);
                idx += 1;
            }
            ArgKind::Count => {
                let name = arg.name.clone();
                bump_count(sink.target(arg), &name);
                idx += 1;
            }
            ArgKind::Option => {
                let rest: String = chars[idx + 1..].iter().collect();
                if rest.is_empty() {
                    let value = tokens
                        .get(i + 1)
                        .ok_or_else(|| ParseError::new(ErrorKind::MissingValue, written.display()))?
                        .clone();
                    record(
                        sink.target(arg),
                        arg,
                        value,
                        written,
                        ValueSource::CommandLine,
                    )?;
                    return Ok(i + 2);
                }
                // `-ovalue`, and also `-o=value`, which users write often enough
                // that silently keeping the `=` would be a trap.
                let value = rest.strip_prefix('=').unwrap_or(&rest).to_owned();
                record(
                    sink.target(arg),
                    arg,
                    value,
                    written,
                    ValueSource::CommandLine,
                )?;
                return Ok(i + 1);
            }
            // `find_short` never returns a positional.
            ArgKind::Positional => {
                return Err(ParseError::new(ErrorKind::UnknownFlag, written.display()));
            }
        }
    }

    Ok(i + 1)
}

/// Fill in what the command line did not say for each of `declared`, then check
/// the relationships between them.
///
/// `subcommand_invoked` suppresses the required-argument check: once a subcommand
/// has taken over, this command's own required arguments could not have been
/// supplied, so demanding them would make every grouping command unusable.
fn finalize(
    command: &Command,
    declared: &[&Arg],
    sink: &mut Sink,
    subcommand_invoked: bool,
) -> Result<(), ParseError> {
    for arg in declared {
        if arg.expects_value() {
            if sink.target(arg).values.contains_key(&arg.name) {
                continue;
            }
            if let Some(value) = env_value(arg) {
                let written = match arg.env.as_deref() {
                    Some(name) => Written::Env(name),
                    None => Written::Name(&arg.name),
                };
                record(
                    sink.target(arg),
                    arg,
                    value,
                    written,
                    ValueSource::Environment,
                )?;
                continue;
            }
            if let Some(default) = &arg.default {
                let value = default.clone();
                record(
                    sink.target(arg),
                    arg,
                    value,
                    Written::Name(&arg.name),
                    ValueSource::Default,
                )?;
                continue;
            }
        } else {
            if sink.flag(&arg.name) || sink.negated(&arg.name) {
                continue;
            }
            if arg.is_negatable() && env_value(arg).is_some_and(|value| !truthy(&value)) {
                negate(sink.target(arg), arg, ValueSource::Environment);
                continue;
            }
            if env_value(arg).is_some_and(|value| truthy(&value)) {
                let kind = arg.kind;
                let name = arg.name.clone();
                let target = sink.target(arg);
                if kind == ArgKind::Count {
                    bump_count(target, &name);
                } else {
                    set_flag(target, arg, ValueSource::Environment);
                }
                continue;
            }
        }

        if arg.required
            && !subcommand_invoked
            && !arg.required_unless.iter().any(|other| sink.present(other))
        {
            let mut error = ParseError::new(ErrorKind::MissingRequired, &arg.name);
            if !arg.required_unless.is_empty() {
                error = error.with_detail(crate::shim::format!(
                    "provide '{}', or one of: {}",
                    arg.name,
                    arg.required_unless.join(", ")
                ));
            }
            return Err(error);
        }
    }

    check_relationships(declared, sink)?;
    check_groups(command, declared, sink, subcommand_invoked)?;
    Ok(())
}

/// Enforce each of `command`'s argument groups, and record which member
/// answered each one.
///
/// "Given" for the at-most-one rule means supplied by the user and not turned
/// off; a default does not count, or a group whose members have defaults could
/// never be satisfied without a conflict. For the at-least-one rule any value
/// counts, defaults included, because a default is an answer.
fn check_groups(
    command: &Command,
    declared: &[&Arg],
    sink: &mut Sink,
    subcommand_invoked: bool,
) -> Result<(), ParseError> {
    for group in &command.groups {
        let given: Vec<&str> = group
            .members
            .iter()
            .map(String::as_str)
            .filter(|name| {
                matches!(
                    sink.source(name),
                    Some(ValueSource::CommandLine | ValueSource::Environment)
                ) && !sink.negated(name)
            })
            .collect();

        if !group.multiple && given.len() > 1 {
            return Err(
                ParseError::new(ErrorKind::Conflict, display_member(declared, given[0]))
                    .with_detail(crate::shim::format!(
                        "'{}' and '{}' cannot be used together; '{}' accepts only one of: {}",
                        display_member(declared, given[0]),
                        display_member(declared, given[1]),
                        group.name,
                        members_list(declared, group),
                    )),
            );
        }

        let answered = given.first().copied().or_else(|| {
            group
                .members
                .iter()
                .map(String::as_str)
                .find(|name| sink.present(name))
        });

        match answered {
            Some(member) => {
                let _ = sink
                    .level
                    .groups
                    .insert(group.name.clone(), member.to_owned());
            }
            // As with a required argument, a grouping command whose subcommand
            // took over cannot have been expected to answer its own group.
            None if group.required && !subcommand_invoked => {
                return Err(
                    ParseError::new(ErrorKind::MissingRequired, &group.name).with_detail(
                        crate::shim::format!("provide one of: {}", members_list(declared, group)),
                    ),
                );
            }
            None => {}
        }
    }
    Ok(())
}

/// How to name a group member in an error: its flag form if it has one.
fn display_member(declared: &[&Arg], name: &str) -> String {
    match declared.iter().find(|arg| arg.name == name) {
        Some(arg) => match arg.long_name() {
            Some(long) => crate::shim::format!("--{long}"),
            None => arg.name.clone(),
        },
        None => name.to_owned(),
    }
}

/// Every member of `group`, named for an error.
fn members_list(declared: &[&Arg], group: &crate::group::ArgGroup) -> String {
    group
        .members
        .iter()
        .map(|name| display_member(declared, name))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The value of an argument's environment variable, if it has one and it is set.
fn env_value(arg: &Arg) -> Option<String> {
    let name = arg.env.as_deref()?;
    // A variable set to the empty string is treated as unset, which is what lets
    // a shell clear an inherited value with `VAR=`.
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

/// Whether an environment value counts as setting a flag.
fn truthy(value: &str) -> bool {
    !matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "" | "0" | "false" | "no" | "off"
    )
}

/// Check the conflicts and dependencies the arguments declare.
fn check_relationships(declared: &[&Arg], sink: &Sink) -> Result<(), ParseError> {
    for arg in declared {
        // Only an argument the user actually supplied can conflict with
        // anything: a default or an inherited environment value must not.
        if !sink.asserted(&arg.name) {
            continue;
        }
        for other in &arg.conflicts {
            if sink.asserted(other) {
                return Err(ParseError::new(ErrorKind::Conflict, &arg.name).with_detail(
                    crate::shim::format!("'{}' cannot be used with '{other}'", arg.name),
                ));
            }
        }
        for other in &arg.requires {
            if !sink.present(other) {
                return Err(ParseError::new(ErrorKind::MissingDependency, &arg.name)
                    .with_detail(crate::shim::format!("'{}' requires '{other}'", arg.name)));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    /// A command with one short flag, for the negative-number decision.
    fn with_short(short: char) -> Command {
        Command::new("calc").arg(Arg::flag("x").short(short))
    }

    #[test]
    fn test_a_lone_dash_is_a_value() {
        assert!(!is_flag_like("-", &Command::new("c"), &[]));
    }

    #[test]
    fn test_negative_numbers_are_values_unless_claimed() {
        let plain = Command::new("calc");
        for token in ["-5", "-42", "-.5", "-0"] {
            assert!(
                !is_flag_like(token, &plain, &[]),
                "{token} should be a value"
            );
        }
        // A command that really declares `-5` gets it back.
        assert!(is_flag_like("-5", &with_short('5'), &[]));
    }

    #[test]
    fn test_ordinary_flags_are_still_flags() {
        let plain = Command::new("calc");
        for token in ["-v", "--verbose", "-abc", "--jobs=4"] {
            assert!(is_flag_like(token, &plain, &[]), "{token} should be a flag");
        }
    }

    #[test]
    fn test_a_global_can_claim_a_numeric_short() {
        let one = Arg::flag("one").short('1');
        let globals = [&one];
        assert!(is_flag_like("-1", &Command::new("c"), &globals));
    }

    #[test]
    fn test_truthy_environment_values() {
        for yes in ["1", "true", "YES", "on", "anything"] {
            assert!(truthy(yes), "{yes}");
        }
        for no in ["", "0", "false", "No", "OFF", "  off  "] {
            assert!(!truthy(no), "{no:?}");
        }
    }

    #[test]
    fn test_written_forms_are_only_built_on_failure() {
        // The display form exists for error messages; the happy path never
        // builds one, which is why `Written` is carried unformatted.
        assert_eq!(Written::Long("level").display(), "--level");
        assert_eq!(Written::Short('j').display(), "-j");
        assert_eq!(Written::Name("path").display(), "path");
        assert_eq!(Written::Env("FORGE_TOKEN").display(), "$FORGE_TOKEN");
    }

    #[test]
    fn test_a_sink_routes_by_whether_the_argument_is_global() {
        let mut level = Matches::default();
        let mut globals = Matches::default();
        let mut sink = Sink {
            level: &mut level,
            globals: &mut globals,
        };

        let local = Arg::flag("local");
        let shared = Arg::flag("shared").global(true);
        set_flag(sink.target(&local), &local, ValueSource::CommandLine);
        set_flag(sink.target(&shared), &shared, ValueSource::CommandLine);

        // Either slot answers the combined questions.
        assert!(sink.flag("local"));
        assert!(sink.flag("shared"));
        assert!(sink.present("shared"));

        assert!(level.flag("local"));
        assert!(!level.flag("shared"));
        assert!(globals.flag("shared"));
        assert!(!globals.flag("local"));
    }
}
