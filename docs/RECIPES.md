<h1 align="center">
    <img width="90px" height="auto" src="https://raw.githubusercontent.com/jamesgober/jamesgober/main/media/icons/hexagon-3.svg" alt="Triple Hexagon">
    <br><b>cli-forge · Recipes</b>
</h1>

<p align="center">
  <code>v2.0.0</code> &mdash; the answer, without the explanation.
</p>

> Every recipe is complete and compiles. For *why* any of it works, see
> [Output &amp; Styling](./OUTPUT.md) or [Commands &amp; Arguments](./COMMANDS.md).

<hr>
<br>

## Contents

**Output**

- [Print a success, a warning, a failure](#print-a-success-a-warning-a-failure)
- [Restyle the whole program in one place](#restyle-the-whole-program-in-one-place)
- [Add a `--plain` mode](#add-a---plain-mode)
- [A status line with aligned columns](#a-status-line-with-aligned-columns)
- [A badge](#a-badge)
- [Style part of a sentence](#style-part-of-a-sentence)
- [A clickable link](#a-clickable-link)
- [Print a progress-style counter](#print-a-progress-style-counter)
- [Print a two-column list](#print-a-two-column-list)
- [Print a filename you do not trust](#print-a-filename-you-do-not-trust)
- [Truncate a long value to fit](#truncate-a-long-value-to-fit)
- [Wrap a paragraph to the terminal](#wrap-a-paragraph-to-the-terminal)

**Command line**

- [The smallest complete program](#the-smallest-complete-program)
- [Wire up a `--color` flag](#wire-up-a---color-flag)
- [Wire up `--verbose` / `--quiet`](#wire-up---verbose----quiet)
- [Read a number, safely](#read-a-number-safely)
- [Accept one value from a fixed set](#accept-one-value-from-a-fixed-set)
- [Read a secret from the environment](#read-a-secret-from-the-environment)
- [Accept a file, or standard input](#accept-a-file-or-standard-input)
- [Make two flags mutually exclusive](#make-two-flags-mutually-exclusive)
- [Accept a list of files](#accept-a-list-of-files)
- [Group commands under one name](#group-commands-under-one-name)
- [Put each command in its own module](#put-each-command-in-its-own-module)
- [Return a specific exit code](#return-a-specific-exit-code)
- [Override a config file only when asked](#override-a-config-file-only-when-asked)
- [Add examples to a help page](#add-examples-to-a-help-page)
- [Hide a debugging flag](#hide-a-debugging-flag)
- [Test a command without running it](#test-a-command-without-running-it)
- [Make styled output deterministic in a test](#make-styled-output-deterministic-in-a-test)
- [Generate something from the command tree](#generate-something-from-the-command-tree)

---

## Print a success, a warning, a failure

```rust
use cli_forge::{fail, ok, warn, Theme};

fn main() {
    Theme::new().install();     // once, at the top of main

    ok("compiled 3 targets");
    warn("2 tests were skipped");
    fail("smoke test failed");
}
```

`warn` and `fail` go to standard error automatically. The other levels are
`info`, `hint`, `note`, `debug`, and `trace`.

---

## Restyle the whole program in one place

```rust
use cli_forge::{Level, Style, Theme};

fn main() {
    Theme::new()
        .set(Level::Success, Style::new().bright_green().bold(), "✓")
        .set(Level::Error, Style::new().bright_red().bold(), "✗")
        .set_style(Level::Hint, Style::new().bright_black().italic())
        .install();

    cli_forge::ok("nothing below this line names a colour");
}
```

---

## Add a `--plain` mode

```rust
use cli_forge::{App, Arg, Command, Theme};

fn main() {
    let mut app = App::new("forge").arg(Arg::flag("plain").global(true));
    app.register(Command::new("build").run(|m| {
        if m.flag("plain") {
            Theme::plain().install();
        }
        cli_forge::ok("built");
    }));
    let _ = app;
}
```

`Theme::plain()` keeps the stream split and drops all styling and markers.

---

## A status line with aligned columns

```rust
use cli_forge::{define, fail, named, ok, warn, Style, Theme};

enum Status { Ok, Warn, Fail }

fn main() {
    Theme::new().install();
    define("step", Style::new().pad_to(22));
    define("detail", Style::new().bright_black());

    step("resolve dependencies", Status::Ok, "0.4s");
    step("run test suite", Status::Warn, "12 of 14");
    step("smoke test", Status::Fail, "timeout");
}

fn step(label: &str, status: Status, detail: &str) {
    let line = format!("{} {}", named("step").paint(label), named("detail").paint(detail));
    match status {
        Status::Ok => ok(line),
        Status::Warn => warn(line),
        Status::Fail => fail(line),
    }
}
```

```text
✓ resolve dependencies   0.4s
! run test suite         12 of 14
✗ smoke test             timeout
```

The width lives in one place, and it is measured in display columns — so a
non-ASCII label does not skew the second column.

---

## A badge

```rust
use cli_forge::{out, Style};

fn main() {
    out(Style::new().black().on_green().bold().pad_to(8).paint("PASS"));
    out(Style::new().white().on_red().bold().pad_to(8).paint("FAIL"));
}
```

The padding is inside the styling, so the background fills the whole field.

---

## Style part of a sentence

```rust
use cli_forge::{markup, out};

fn main() {
    out(markup("<b>summary</b>: <c=green>12 passed</c>, <c=red>1 failed</c>"));
    out(markup("see <c=#3b82f6><u>./forge.toml</u></c>"));
}
```

---

## A clickable link

```rust
use cli_forge::{out, Style};

fn main() {
    out(Style::new()
        .cyan()
        .underline()
        .link("https://docs.rs/cli-forge")
        .paint("the documentation"));
}
```

Terminals that do not support it show the text unchanged, so it is always safe.

---

## Print a progress-style counter

```rust
use cli_forge::{out, text::Align, Style};

fn main() {
    let total = 42;
    let index = Style::new().bright_black().pad_to(7).align(Align::Right);

    for n in 1..=3 {
        out(format!("{} compiling unit {n}", index.paint(format!("[{n}/{total}]"))));
    }
}
```

```text
 [1/42] compiling unit 1
 [2/42] compiling unit 2
 [3/42] compiling unit 3
```

---

## Print a two-column list

```rust
use cli_forge::{out, text, Style};

fn main() {
    let rows = [("build", "compile the project"), ("日本語", "a wide name"), ("rm", "delete")];

    // Width the first column to the widest entry, in display columns.
    let width = rows.iter().map(|(name, _)| text::width(name)).max().unwrap_or(0);
    let name = Style::new().bold().pad_to(width as u16);

    for (n, description) in rows {
        out(format!("  {}  {description}", name.paint(n)));
    }
}
```

---

## Print a filename you do not trust

```rust
use cli_forge::{out, text};

fn main() {
    let from_outside = "report.txt\u{1b}[2JALL FILES DELETED";
    out(text::sanitize(from_outside));
}
```

Apply it to anything that came from a filesystem, a network, or a user.

---

## Truncate a long value to fit

```rust
use cli_forge::{out, text, terminal};

fn main() {
    let long = "a/very/long/path/that/will/not/fit/on/one/line/file.rs";
    let budget = usize::from(terminal::width_or(80)).saturating_sub(4);

    out(format!("  {}", text::truncate(long, budget, "…")));
}
```

Safe on styled text: it never cuts an escape sequence, and closes any styling
left open at the cut.

---

## Wrap a paragraph to the terminal

```rust
use cli_forge::{out, terminal, text};

fn main() {
    let note = "This operation rewrites history. Anyone who has already pulled \
                this branch will need to reset to it.";

    for line in text::wrap(note, usize::from(terminal::width_or(80))) {
        out(line);
    }
}
```

---

## The smallest complete program

```rust
use std::process::ExitCode;

use cli_forge::{out, App, Command};

fn main() -> ExitCode {
    let mut app = App::new("hello").version(env!("CARGO_PKG_VERSION"));
    app.register(Command::new("greet").run(|_| out("hello, world")));
    app.run()
}
```

`--help`, `--version`, `help greet`, typo correction, and exit codes all work.

---

## Wire up a `--color` flag

```rust
use std::process::ExitCode;

use cli_forge::{terminal, App, Arg, ColorChoice, Command};

fn main() -> ExitCode {
    let mut app = App::new("forge").arg(
        Arg::option("color")
            .global(true)
            .default("auto")
            .possible_values(["auto", "always", "never"])
            .help("when to colour the output"),
    );

    app.register(Command::new("build").run(|m| {
        terminal::set_color_choice(match m.value("color") {
            Some("always") => ColorChoice::Always,
            Some("never") => ColorChoice::Never,
            _ => ColorChoice::Auto,
        });
        cli_forge::ok("built");
    }));

    app.run()
}
```

`possible_values` means `--color=purple` is refused with the valid set, and a
near miss is corrected.

---

## Wire up `--verbose` / `--quiet`

```rust
use std::process::ExitCode;

use cli_forge::{debug, info, App, Arg, Command, Level, Matches, Stream, Theme};

fn main() -> ExitCode {
    let mut app = App::new("forge")
        .arg(Arg::count("verbose").short('v').global(true).help("say more; repeat for more"))
        .arg(Arg::flag("quiet").short('q').global(true).conflicts_with(["verbose"]));

    app.register(Command::new("build").run(build));
    app.run()
}

fn build(m: &Matches) {
    // Quiet mode: send the chatty levels to stderr so stdout stays data-only.
    if m.flag("quiet") {
        Theme::new()
            .set_stream(Level::Info, Stream::Stderr)
            .set_stream(Level::Note, Stream::Stderr)
            .install();
    }

    if !m.flag("quiet") {
        info("resolving dependencies");
    }
    if m.count("verbose") >= 1 {
        debug("41 packages, 12ms");
    }
    if m.count("verbose") >= 2 {
        debug("cache hit: libcore-1.0.0");
    }

    cli_forge::ok("built");
}
```

---

## Read a number, safely

```rust
use cli_forge::{Arg, Command};

let cmd = Command::new("serve")
    .arg(
        Arg::option("port")
            .default("8080")
            .validate(|v| match v.parse::<u16>() {
                Ok(p) if p >= 1024 => Ok(()),
                Ok(_) => Err("ports below 1024 need privileges".to_string()),
                Err(_) => Err("expected a number from 1024 to 65535".to_string()),
            }),
    )
    .run(|m| {
        // Validated at parse time, so this cannot fail for a user's reason.
        let port: u16 = m.get("port").unwrap_or(8080);
        let _ = port;
    });
# let _ = cmd;
```

---

## Accept one value from a fixed set

```rust
use cli_forge::{Arg, Command};

let cmd = Command::new("log")
    .arg(Arg::option("level").possible_values(["warn", "info", "debug"]).default("info"))
    .run(|m| {
        match m.value("level") {
            Some("warn") => {}
            Some("debug") => {}
            _ => {}
        }
    });
# let _ = cmd;
```

The set appears in help, is enforced, and powers the correction for a typo.

---

## Read a secret from the environment

```rust
use cli_forge::{Arg, Command};

let cmd = Command::new("publish")
    .arg(
        Arg::option("token")
            .env("FORGE_TOKEN")
            .hide(true)                  // works, but not advertised
            .help("registry credentials"),
    )
    .run(|m| {
        let Some(token) = m.value("token") else {
            return Err("set FORGE_TOKEN or pass --token");
        };
        let _ = token;
        Ok(())
    });
# let _ = cmd;
```

`FORGE_TOKEN` is not visible in `ps`; `--token abc123` is.

---

## Accept a file, or standard input

```rust
use cli_forge::{App, Arg, Command};

let mut app = App::new("forge");
app.register(
    Command::new("read")
        .arg(Arg::positional("file").required(true).required_unless(["stdin"]))
        .arg(Arg::flag("stdin").help("read from standard input instead")),
);

assert!(app.try_parse_from(["read", "notes.txt"]).is_ok());
assert!(app.try_parse_from(["read", "--stdin"]).is_ok());
assert!(app.try_parse_from(["read"]).is_err());
```

A lone `-` also parses as a value, if you prefer that convention.

---

## Make two flags mutually exclusive

```rust
use cli_forge::{App, Arg, Command, ErrorKind};

let mut app = App::new("forge");
app.register(
    Command::new("log")
        .arg(Arg::flag("quiet").conflicts_with(["verbose"]))
        .arg(Arg::flag("verbose")),
);

assert_eq!(
    app.try_parse_from(["log", "--quiet", "--verbose"]).unwrap_err().kind(),
    ErrorKind::Conflict
);
```

---

## Accept a list of files

```rust
use cli_forge::{App, Arg, Command};

let mut app = App::new("forge");
app.register(
    Command::new("build")
        // Repeatable option: -I a -I b
        .arg(Arg::option("include").short('I').multiple(true))
        // Variadic positional: absorbs the rest. Put it last.
        .arg(Arg::positional("sources").multiple(true)),
);

let m = app.try_parse_from(["build", "-I", "a", "x.c", "y.c"]).unwrap();
let build = m.leaf();
assert_eq!(build.values("include").collect::<Vec<_>>(), ["a"]);
assert_eq!(build.values("sources").collect::<Vec<_>>(), ["x.c", "y.c"]);
```

---

## Group commands under one name

```rust
use cli_forge::{App, Arg, Command};

let mut app = App::new("forge");
app.register(
    Command::new("remote")
        .about("manage remotes")
        .subcommand_required(true)
        .subcommand(
            Command::new("add")
                .arg(Arg::positional("name").required(true))
                .arg(Arg::positional("url").required(true))
                .run(|_| {}),
        )
        .subcommand(Command::new("list").run(|_| {}))
        .subcommand(Command::new("remove").alias("rm").run(|_| {})),
);

assert!(app.try_parse_from(["remote"]).is_err());          // needs a subcommand
assert!(app.try_run_from(["remote", "list"]).is_ok());
```

---

## Put each command in its own module

```rust
// src/commands/build.rs
use cli_forge::{out, App, Arg, Command};

pub fn install(app: &mut App) {
    app.register(
        Command::new("build")
            .about("compile the project")
            .arg(Arg::flag("release").short('r'))
            .run(|_| out("building...")),
    );
}
```

```rust
// src/main.rs
# mod commands { pub mod build { pub fn install(a: &mut cli_forge::App) { let _ = a; } }
#                pub mod test { pub fn install(a: &mut cli_forge::App) { let _ = a; } } }
use std::process::ExitCode;

use cli_forge::App;

fn main() -> ExitCode {
    let mut app = App::new("forge");
    commands::build::install(&mut app);
    commands::test::install(&mut app);
    app.run()
}
```

---

## Return a specific exit code

```rust
use cli_forge::{App, Command, CommandError};

let mut app = App::new("diffy");
app.register(Command::new("diff").run_status(|_| {
    Err(CommandError::new("files differ").with_code(1))
}));

let failure = app.try_run_from(["diff"]).unwrap().unwrap_err();
assert_eq!(failure.exit_code(), 1);
```

Plain `run` reports every failure as `1`; `run_status` carries the exact status.

---

## Override a config file only when asked

```rust
use cli_forge::{Matches, ValueSource};

fn jobs(m: &Matches, from_config: u16) -> u16 {
    match m.source("jobs") {
        // The user asked for it. Their choice wins.
        Some(ValueSource::CommandLine | ValueSource::Environment) => {
            m.get("jobs").unwrap_or(from_config)
        }
        // Nobody asked; the config file is more specific than our default.
        _ => from_config,
    }
}
```

---

## Add examples to a help page

```rust
use cli_forge::{App, Command};

let mut app = App::new("forge");
app.register(
    Command::new("build")
        .about("compile the project")
        .long_about("Compiles every target the manifest lists.")
        .after_help("EXAMPLES:\n  forge build --release\n  forge build -j8 server"),
);

assert!(app.command_help(["build"]).unwrap().contains("EXAMPLES:"));
```

Examples are the part of a help page people actually read.

---

## Hide a debugging flag

```rust
use cli_forge::{App, Arg, Command};

let mut app = App::new("forge");
app.register(Command::new("build").arg(Arg::flag("dump-ast").hide(true)));

assert!(!app.command_help(["build"]).unwrap().contains("dump-ast"));
assert!(app.try_parse_from(["build", "--dump-ast"]).is_ok());
```

`Command::hidden(true)` does the same for a whole command.

---

## Test a command without running it

```rust
# fn main() {}
use cli_forge::{App, Arg, Command, ErrorKind};

fn app() -> App {
    let mut app = App::new("forge");
    app.register(
        Command::new("build")
            .arg(Arg::option("jobs").default("1"))
            .arg(Arg::flag("release").short('r'))
            .run(|_| -> () { panic!("must not run") }),
    );
    app
}

#[test]
fn parses_without_dispatching() {
    let m = app().try_parse_from(["build", "-r", "--jobs", "8"]).unwrap();
    let build = m.leaf();
    assert!(build.flag("release"));
    assert_eq!(build.get::<u16>("jobs"), Some(8));
}

#[test]
fn rejects_an_unknown_flag() {
    let err = app().try_parse_from(["build", "--bogus"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::UnknownFlag);
    assert_eq!(err.suggestion(), None);
}
```

To run the handler, use `try_run_from` and assert on the outcome.

---

## Make styled output deterministic in a test

Best: render at an explicit depth, which touches no process state and is safe in
parallel.

```rust
# fn main() {}
use cli_forge::{ColorLevel, Style};

#[test]
fn renders_the_expected_bytes() {
    let bytes = Style::new().red().bold().paint_at("ERR", ColorLevel::Ansi16).to_string();
    assert_eq!(bytes, "\u{1b}[1;31mERR\u{1b}[0m");
}
```

If you must assert on something that detects, strip instead:

```rust
# fn main() {}
use cli_forge::{text, App, Command};

#[test]
fn help_mentions_the_command() {
    let mut app = App::new("forge");
    app.register(Command::new("build").about("compile"));

    // Headings are styled, so strip before asserting on content.
    let help = text::strip(&app.help()).into_owned();
    assert!(help.contains("build"));
    assert!(help.contains("compile"));
}
```

---

## Generate something from the command tree

```rust
use cli_forge::{App, Arg, Command};

/// A sketch of what a completion generator does.
fn describe(app: &App) -> String {
    let mut out = String::new();
    for command in app.commands() {
        if command.is_hidden() {
            continue;
        }
        out.push_str(command.name());
        for arg in command.arguments() {
            if arg.is_hidden() {
                continue;
            }
            if let Some(long) = arg.long_form() {
                out.push_str(&format!(" --{long}"));
                if arg.expects_value() {
                    out.push_str(if arg.allowed_values().is_empty() { " <v>" } else { " <set>" });
                }
            }
        }
        out.push('\n');
    }
    out
}

fn main() {
    let app = App::new("forge").command(
        Command::new("build").arg(Arg::option("level").possible_values(["warn", "info"])),
    );
    assert_eq!(describe(&app), "build --level <set>\n");
}
```

<hr>
<br>

## See also

- **[Guide](./GUIDE.md)** — start here if any of the above was unclear.
- **[Output & Styling](./OUTPUT.md)** · **[Commands & Arguments](./COMMANDS.md)**
- **[API](./API.md)** — the full surface and the stability promise.

<div align="center">
  <h2></h2>
  <sup>COPYRIGHT <small>&copy;</small> 2026 <strong>James Gober <me@jamesgober.com>.</strong></sup>
</div>
