<h1 align="center">
    <img width="90px" height="auto" src="https://raw.githubusercontent.com/jamesgober/jamesgober/main/media/icons/hexagon-3.svg" alt="Triple Hexagon">
    <br><b>cli-forge · Guide</b>
</h1>

<p align="center">
  <code>v2.1.0</code> &mdash; start here. Install it, write something that works, understand the shape.
</p>

> **No prior knowledge assumed.** Every example below is complete: copy it, run
> it, and it works. Where something is explained twice, that is deliberate.

<hr>
<br>

## Contents

- [Install](#install)
- [Your first output](#your-first-output)
- [Your first command](#your-first-command)
- [The shape of the thing](#the-shape-of-the-thing)
- [Making it look right](#making-it-look-right)
- [Accepting input](#accepting-input)
- [Failing well](#failing-well)
- [A complete program](#a-complete-program)
- [What to read next](#what-to-read-next)

---

## Install

```toml
[dependencies]
cli-forge = "2.1"
```

That is it. The defaults give you colour, correct text widths, and help that
wraps to the terminal. If you want zero dependencies, see
[Feature flags](./API.md#feature-flags) — but start with the defaults.

---

## Your first output

```rust
use cli_forge::{err, out};

fn main() {
    out("building...");
    err("something went wrong");
}
```

```text
building...
something went wrong
```

`out` prints to standard output; `err` prints to standard error. Both add a
newline. Both take **anything printable**, which means you never need to reach
for a different function:

```rust
use cli_forge::out;

fn main() {
    out("a string literal");
    out(42);
    out(3.14);
    out(format!("{} of {}", 3, 7));
    out(vec![1, 2, 3].len());
}
```

### Why not just `println!`?

For plain text, `println!` is fine and `out` is doing the same thing at the same
speed (~9.5 ns, no heap allocation). The reason to use `out` is that **everything
else in this crate flows through it** — a styled value, a themed status line, a
measured column — so you never have two ways of printing that behave differently.
There is also no `unwrap` to forget: a closed pipe (`yourtool | head`) cannot
panic your program.

> **The one rule worth knowing now:** `out` never parses anything. It takes a
> value and writes it. If you want colour, you make the *value* coloured. That is
> why there is no `out_colored` or `out_red`.

---

## Your first command

```rust
use std::process::ExitCode;

use cli_forge::{out, App, Command};

fn main() -> ExitCode {
    let mut app = App::new("hello");

    app.register(Command::new("greet").run(|_| out("hello, world")));

    app.run()
}
```

```console
$ hello greet
hello, world

$ hello --help
USAGE: hello <command>

COMMANDS:
  greet

OPTIONS:
  -h, --help  show this help
```

Three things happened without you asking:

1. **`--help` works**, and so does `hello help greet`.
2. **Running `hello` with no arguments printed the help** instead of doing
   nothing silently.
3. **A typo is corrected**: `hello gret` says `did you mean 'greet'?` and exits
   `2`.

### Why `fn main() -> ExitCode`?

Because `app.run()` returns the exit status your program should have, and
returning it from `main` is how you give it to the shell. If you forget, the
compiler tells you — `run` is `#[must_use]`.

A shell script doing `yourtool build || echo failed` depends on that status being
right. `run` handles it: `0` for success and for `--help`, `2` for a bad command
line, `1` (or whatever you choose) for a command that failed.

---

## The shape of the thing

Four types carry almost everything. Learning what each one is *for* makes the
rest obvious.

```text
App        the program. Holds commands, parses the command line, runs one.
 └─ Command    one thing the program can do. Holds arguments and a handler.
     └─ Arg        one input a command accepts.
         └─ Matches    what the user actually typed, handed to your handler.
```

And two more, for appearance:

```text
Style      how text should look. Reusable.
Theme      what success / failure / warning look like. Stated once.
```

Written out:

```rust
use cli_forge::{out, App, Arg, Command};

let mut app = App::new("forge");                    // the program

app.register(
    Command::new("build")                           // one thing it can do
        .arg(Arg::flag("release").short('r'))       // one input it accepts
        .run(|matches| {                            // what to do
            out(format!("release={}", matches.flag("release")));
        }),
);
```

Everything else in the crate is a refinement of one of those six.

---

## Making it look right

### One-off colour

```rust
use cli_forge::{out, style};

fn main() {
    out(style("done").green().bold());
    out(style("warning").yellow());
    out(style("a brand colour").hex("#3b82f6"));
}
```

`style("text")` starts a description; the methods chain; the result prints.

### The way you actually want

Here is the problem with the above: if a program has forty status lines, it names
a colour forty times. Change your mind about what "success" looks like and you
edit forty places.

So instead, say it once:

```rust
use cli_forge::{fail, ok, warn, Theme};

fn main() {
    Theme::new().install();     // once, at the top of main

    ok("compiled 3 targets");
    warn("2 tests were skipped");
    fail("smoke test failed");
}
```

```text
✓ compiled 3 targets
! 2 tests were skipped
✗ smoke test failed
```

**Nothing in those three calls names a colour.** They say *what happened*. The
theme decides what that looks like, which means:

- You can restyle the entire program by changing the theme.
- `✓` becomes `+` automatically on a terminal that cannot draw it.
- `warn` and `fail` went to **standard error**, so `yourtool > out.txt` captures
  the data and leaves the complaints on screen.
- On a pipe or under `NO_COLOR`, the colour falls away and nothing else moves.

That last point matters more than it sounds. Try it:

```console
$ yourtool | cat          # no escape codes in the pipe
$ NO_COLOR=1 yourtool     # no escape codes at all
```

The eight levels are `ok`, `fail`, `warn`, `info`, `hint`, `note`, `debug`, and
`trace`. Use the one that describes what happened.

> **Rule of thumb:** if you are writing `style(...)` more than once for the same
> *kind* of thing, you want a theme or a named style instead. See
> [Output & Styling](./OUTPUT.md).

---

## Accepting input

Four kinds of argument, and that is the whole list:

```rust
use cli_forge::{Arg, Command};

let _cmd = Command::new("build")
    // --release / -r        a switch: on or off
    .arg(Arg::flag("release").short('r'))
    // -vvv                  a switch you can repeat, counted
    .arg(Arg::count("verbose").short('v'))
    // --jobs 8 / -j8        a named value
    .arg(Arg::option("jobs").short('j').default("1"))
    // forge build server    a bare value, by position
    .arg(Arg::positional("target"));
```

Reading them back:

```rust
use cli_forge::{out, Arg, Command};

let _cmd = Command::new("build")
    .arg(Arg::flag("release").short('r'))
    .arg(Arg::count("verbose").short('v'))
    .arg(Arg::option("jobs").short('j').default("1"))
    .arg(Arg::positional("target"))
    .run(|m| {
        let release: bool = m.flag("release");
        let verbosity: usize = m.count("verbose");
        let jobs: u16 = m.get("jobs").unwrap_or(1);        // parsed for you
        let target: &str = m.value("target").unwrap_or("all");

        out(format!("{target}: release={release} jobs={jobs} v={verbosity}"));
    });
```

Every form a user might type already works, without you doing anything:

```console
$ forge build --jobs 8
$ forge build --jobs=8
$ forge build -j 8
$ forge build -j8
$ forge build -rvvv            # bundled: -r -v -v -v
$ forge build -- --not-a-flag  # everything after -- is a value
```

### Reject bad input properly

Do not check values inside your handler. Say what is acceptable, and let the
parser refuse it before your code runs:

```rust
use cli_forge::{Arg, Command};

let _cmd = Command::new("log")
    .arg(Arg::option("level").possible_values(["warn", "info", "debug"]))
    .run(|m| {
        // By the time this runs, `level` is one of those three. Guaranteed.
        let _ = m.value("level");
    });
```

```console
$ forge log --level inof
error: invalid value 'inof'

  '--level' accepts one of: warn, info, debug

  did you mean 'info'?

USAGE: forge log [options]
```

You wrote one line. The user got the valid set, a correction, the usage line, and
exit code `2`. **This is the single highest-value habit in the whole crate**: say
what a value may be, and you never write a validation `if` again.

---

## Failing well

A handler can just return nothing, like the examples so far. Or it can fail:

```rust
use cli_forge::{App, Command};

let mut app = App::new("forge");

// A message. Exits 1.
app.register(Command::new("pull").run(|_| Err("not a repository")));

// Or `?` against any error type you already use.
app.register(Command::new("read").run(|_| -> std::io::Result<()> {
    let text = std::fs::read_to_string("forge.toml")?;
    cli_forge::out(text);
    Ok(())
}));
```

```console
$ forge pull
error: not a repository
$ echo $?
1
```

`?` works because `run` accepts any `Result` whose error can be printed — which
includes `io::Error`, `Box<dyn Error>`, `anyhow::Error`, and your own types.

If you need a *specific* exit code (some tools treat `1` and `2` as different
answers), use `run_status`:

```rust
use cli_forge::{Command, CommandError};

let _cmd = Command::new("diff")
    .run_status(|_| Err(CommandError::new("files differ").with_code(1)));
```

---

## A complete program

Everything above, in one file that actually runs.

```rust
use std::process::ExitCode;

use cli_forge::{hint, info, ok, out, style, warn, App, Arg, Command, Matches, Theme};

fn main() -> ExitCode {
    // 1. Say what the program looks like. Once.
    Theme::new().install();

    // 2. Describe the program.
    let mut app = App::new("forge")
        .version(env!("CARGO_PKG_VERSION"))
        .about("builds projects described by a forge.toml")
        // An argument the whole program accepts, on either side of the command.
        .arg(Arg::count("verbose").short('v').global(true).help("say more"));

    // 3. Describe what it can do.
    app.register(
        Command::new("build")
            .about("compile the project")
            .arg(Arg::positional("targets").multiple(true).help("defaults to all"))
            .arg(Arg::flag("release").short('r').help("optimise"))
            .arg(
                Arg::option("jobs")
                    .short('j')
                    .default("1")
                    .value_name("N")
                    .help("compilation units at a time")
                    .validate(|v| match v.parse::<u16>() {
                        Ok(n) if n >= 1 => Ok(()),
                        _ => Err("expected a count of 1 or more".to_string()),
                    }),
            )
            .run(build),
    );

    app.register(Command::new("clean").about("delete build artifacts").run(clean));

    // 4. Run it, and give the shell the right exit status.
    app.run()
}

/// Compile. Returns an error, so `?` works and failure becomes the exit status.
fn build(m: &Matches) -> Result<(), String> {
    // Validated at parse time, so this cannot fail for a reason the user caused.
    let jobs: u16 = m.get("jobs").unwrap_or(1);
    let targets: Vec<&str> = m.values("targets").collect();
    let profile = if m.flag("release") { "release" } else { "debug" };

    out(style(format!("building [{profile}]")).bold());

    if m.count("verbose") > 0 {
        info(format!("{jobs} job(s), {} target(s)", targets.len().max(1)));
    }
    if profile == "debug" {
        warn("this build is not optimised");
        hint("pass --release for an optimised build");
    }

    ok("compiled 3 targets");
    Ok(())
}

fn clean(_: &Matches) -> Result<(), String> {
    ok("removed ./target");
    Ok(())
}
```

Try it:

```console
$ forge
# prints the help, because nothing was asked for

$ forge build -v
building [debug]
i 1 job(s), 1 target(s)
! this build is not optimised
→ pass --release for an optimised build
✓ compiled 3 targets

$ forge build --jobs 0
error: invalid value '0'

  --jobs: expected a count of 1 or more

USAGE: forge build [options] [targets]...

$ forge buidl
error: unknown command 'buidl'

  did you mean 'build'?

USAGE: forge [options] <command>
```

Note what is **not** in that program: no colour at a call site, no validation
`if`, no manual `--help`, no exit-code arithmetic, no `unwrap`.

---

## What to read next

- **[Output & Styling](./OUTPUT.md)** — themes in depth, named styles, aligned
  columns, markup, the full colour model, measuring text. Read this if your
  output needs to look like a real tool.
- **[Commands & Arguments](./COMMANDS.md)** — subcommands, global arguments,
  environment fallbacks, relationships between arguments, customising help,
  testing. Read this if your CLI is more than a couple of commands.
- **[Recipes](./RECIPES.md)** — the answer, without the explanation.
- **[API](./API.md)** — the full surface and what is guaranteed.

Runnable versions of everything here live in
[`examples/`](../examples):

```bash
cargo run --example quick_start     # roughly the program above
cargo run --example theme           # the eight levels, and restyling them
cargo run --example commands -- build --release -j 8
```

<hr>
<br>

<div align="center">
  <sup>COPYRIGHT <small>&copy;</small> 2026 <strong>James Gober <me@jamesgober.com>.</strong></sup>
</div>
