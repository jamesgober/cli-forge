<h1 align="center">
    <img width="99" alt="Rust logo" src="https://raw.githubusercontent.com/jamesgober/rust-collection/72baabd71f00e14aa9184efcb16fa3deddda3a0a/assets/rust-logo.svg">
    <br>
    <b>cli-forge</b>
    <br>
    <sub><sup>UNIFIED CLI FRAMEWORK</sup></sub>
</h1>

<div align="center">
    <a href="https://crates.io/crates/cli-forge"><img alt="Crates.io" src="https://img.shields.io/crates/v/cli-forge"></a>
    <a href="https://crates.io/crates/cli-forge"><img alt="Downloads" src="https://img.shields.io/crates/d/cli-forge?color=%230099ff"></a>
    <a href="https://docs.rs/cli-forge"><img alt="docs.rs" src="https://img.shields.io/docsrs/cli-forge"></a>
    <a href="https://github.com/jamesgober/cli-forge/actions"><img alt="CI" src="https://github.com/jamesgober/cli-forge/actions/workflows/ci.yml/badge.svg"></a>
    <a href="https://github.com/rust-lang/rfcs/blob/master/text/2495-min-rust-version.md"><img alt="MSRV" src="https://img.shields.io/badge/MSRV-1.85%2B-blue"></a>
</div>

<br>

<div align="left">
    <p>
        cli-forge is a unified command-line framework where argument parsing and styled output speak one API. Output is <em>themed</em> rather than restyled at every call site: a theme states what success, failure, and warning look like once, and nothing after that names a colour. Commands register at runtime - from anywhere, not just main - and can be hidden or auth-gated. The styling and measurement layers are seams that sibling crates (tables, progress, gradients) build on, so everything a program prints speaks one system. It targets the lightness of argh with the reach of clap, without the split between parsing in one crate and styling in five.
    </p>
    <br>
    <hr>
    <p>
        <strong>MSRV is 1.85+</strong> (Rust 2024 edition).
    </p>
    <blockquote>
        <strong>Status: 2.0 &mdash; stable.</strong> The public API is frozen under <a href="https://semver.org/">Semantic Versioning</a>: no breaking changes before <code>3.0</code>. Migrating from <code>1.x</code> is a handful of renames &mdash; see the <a href="./CHANGELOG.md#migrating-from-1x">migration table</a>.
    </blockquote>
</div>

<hr>
<br>

## What's here

`v2.1.0`. Two things set cli-forge apart from the alternatives: **output is
themed and reusable** rather than restyled at every call site, and the **styling
layer is a seam** sibling crates build on, so everything a program prints speaks
one system.

- **Themed responses** — `ok`, `fail`, `warn`, `info`, `hint`, `note`, `debug`,
  `trace`. A `Theme` maps each level onto a style, a glyph, and a stream, once.
  No call site names a colour; replace the theme and the whole program changes
  together. Glyphs fall back to ASCII where the destination cannot render them,
  and diagnostics go to standard error so piped output stays clean.
- **Reusable styles** — a `Style` carries its whole appearance: colours,
  attributes, a prefix glyph, a suffix, a display-column width, an alignment, a
  hyperlink. Describe a marker once, apply it to anything with `paint`.
- **Plain output** — `out` / `err`: one call, no parsing, no allocation for a
  string literal, ~9.5 ns. The hot path stays cheap.
- **Testable output** — `capture` records what a program printed, per thread, so
  asserting on it is an ordinary test rather than a subprocess and a pipe.
- **Four styling paths** — the `style` builder, inline `markup`, a named
  `define` / `named` registry, and theme levels. All render to **identical
  bytes** for the same intent, which is what makes mixing them safe.
- **Full colour** — all sixteen terminal colours including the bright half, the
  256-colour palette, any 24-bit value, backgrounds throughout, and eight
  attributes. Exact colours degrade to the nearest the terminal can render
  rather than being dropped. Capability is detected **per stream**, and
  `ColorChoice` is what a `--color` flag drives.
- **Text measurement** — `text::width`, `strip`, `pad`, `truncate`, `wrap`,
  `sanitize`, all in display columns rather than bytes. This is the seam a table
  or progress crate needs; `sanitize` neutralises terminal escape injection from
  untrusted input.
- **Command tree** — a recursive `Command` tree registered into an `App` **from
  anywhere** (not just `main`), with aliases, `hidden`, `requires_auth`,
  `subcommand_required`, and `display_order`.
- **A real argument model** — flags, counting flags (`-vvv`), negatable flags
  (`--no-cache`), options, comma-separated lists, positionals, variadic
  positionals, app-level and **global** arguments, environment fallbacks,
  defaults, `possible_values`, arbitrary validators, conflicts, dependencies,
  `required_unless`, and **argument groups** ("exactly one of these"). Values are
  checked at the edge, so reading them back cannot fail.
- **Plugins** — `App::external` hands unknown command names to a hook, the way
  `cargo watch` runs a separate `cargo-watch`.
- **Errors that answer "what now?"** — a `ParseError` carries the subject, what
  would have been valid, the nearest spelling to what was typed, and the usage
  line. Help and version exit `0`; a bad command line exits `2`.
- **Help** — auto-generated, wrapped to the terminal, aligned in display
  columns, with `long_about`, `before_help`, `after_help`, a usage override, an
  automatic `help [command]` subcommand, and each argument's fallbacks stated.
- **Introspection** — `App`, `Command`, and `Arg` expose read-only accessors, so
  a completions or manual-page crate reads the live command tree instead of a
  second hand-maintained description of it.
- **Auth seam** *(feature `auth`)* — gate a command behind a consumer-supplied
  hook. cli-forge holds the seam; the login state lives in your code. Fails
  closed, and hides unauthorized commands from help.
- **`no_std`** — `--no-default-features` is a real build of the styling core on
  `alloc` alone.

Migrating from `1.x` is a handful of renames; see the
[migration table](./CHANGELOG.md#migrating-from-1x).

<hr>
<br>

## Installation

```toml
[dependencies]
cli-forge = "2.1"
```

The defaults are `std`, `color`, `unicode`, and `termsize`. For a build with no
dependencies at all (correct for Latin text; CJK and emoji widths become
approximate, and help wraps to `COLUMNS` or 80):

```toml
[dependencies]
cli-forge = { version = "2.1", default-features = false, features = ["std", "color"] }
```

<br>

## Quick Start

```rust
use std::process::ExitCode;

use cli_forge::{ok, out, warn, App, Arg, Command};

fn main() -> ExitCode {
    let mut app = App::new("forge")
        .version(env!("CARGO_PKG_VERSION"))
        .about("a project constructor")
        .arg(Arg::count("verbose").short('v').global(true));

    app.register(
        Command::new("build")
            .about("compile the project")
            .arg(Arg::flag("release").short('r'))
            .arg(Arg::option("jobs").short('j').default("1"))
            .run(|m| {
                let jobs: u16 = m.get("jobs").unwrap_or(1);
                out(format!("building with {jobs} job(s)"));
                if !m.flag("release") {
                    warn("this build is not optimised");
                }
                ok("compiled 3 targets");
            }),
    );

    app.run()
}
```

<br>

## Themed output

The problem this solves: in most codebases every status line restyles itself by
hand, so changing how a program looks means touching every call site. A theme
states the vocabulary once.

```rust
use cli_forge::{fail, hint, ok, warn, Glyphs, Level, Style, Theme};

Theme::new()
    .set(Level::Success, Style::new().bright_green().bold(), "✓")
    .set(Level::Error, Style::new().bright_red().bold(), "✗")
    .set_glyphs(Glyphs::Auto)   // Unicode where it renders, ASCII where it does not
    .install();

ok("deployed to staging");
warn("2 tests skipped");
fail("smoke test failed");
hint("try `--release` for an optimised build");
```

Nothing after `install` names a colour. `Theme::plain()` is what a `--plain`
flag should install; `Theme::current()` hands back a copy; a theme is a plain
value, so a library can render through its own without disturbing the host
program's.

<br>

## Reusable styles

A `Style` carries its decoration, which is what removes hand-built markers:

```rust
use cli_forge::{define, named, out, Style};

// Described once — the glyph, the colour, and the column width travel together.
define("step", Style::new().bright_black().prefix("  → ").pad_to(24));

let step = named("step");           // one lookup
for label in ["resolve dependencies", "compile", "link"] {
    out(step.paint(label));         // many uses, no allocation beyond the line
}
```

`pad_to` measures **display columns**, so the column stays straight whether the
content is ASCII, accented, CJK, or already styled — which `format!("{:<24}")`
cannot do.

<br>

## The four styling paths

The same styled line, four ways. The choice is ergonomic, not visual: the bytes
are identical, and the cross-path tests assert that at every colour depth.

```rust
use cli_forge::{define, markup, named, out, style, Level, Style, Theme};

// 1. Builder — chain methods; the result is `Display`. Best for one-offs.
out(style("ERROR: build failed").red().bold());

// 2. Markup — one string with inline tags. Best when the styling varies run by
//    run inside a sentence.
out(markup("<c=red><b>ERROR: build failed</b></c>"));

// 3. Named registry — describe the look once, recall it anywhere by name.
define("error", Style::new().red().bold());
out(named("error").paint("ERROR: build failed"));

// 4. Theme level — best for the handful of things every program says.
cli_forge::fail("build failed");
```

**Markup grammar:** `<b>` bold, `<d>` dim, `<i>` italic, `<u>` underline,
`<s>` strike, `<r>` reverse, `<c=VALUE>` foreground, `<bg=VALUE>` background,
`<link=URL>` hyperlink, `</x>` to close that tag, `</>` to close the innermost,
and `<<` for a literal `<`. `VALUE` is a colour name, `#rrggbb`, `#rgb`,
`r,g,b`, or a palette index. Tags nest; anything unrecognised prints literally,
so markup never rejects input or swallows a message.

<br>

## Colours and terminals

```rust
use cli_forge::{out, style, Color};

out(style("amber").hex("#ff8800"));
out(style("teal").rgb(0, 200, 120));
out(style("indexed").ansi(208));
out(style(" PASS ").black().on_green().bold());
out(style("the docs").cyan().underline().link("https://docs.rs/cli-forge"));
```

Capability is detected **once per stream** and cached, so a program whose data is
piped while its diagnostics stay on the terminal gets clean bytes in the pipe and
colour on screen. An exact colour renders precisely on a true-colour terminal,
degrades to the nearest palette or standard colour where that is all the terminal
supports, and falls away to plain text on a pipe, under `NO_COLOR`, or without the
`color` feature. `NO_COLOR`, `CLICOLOR`, `CLICOLOR_FORCE`, and `FORCE_COLOR` are
all honoured, and Windows Terminal and ConEmu are recognised as true colour. The
Windows console is handled behind the same API, with virtual-terminal mode
enabled automatically and a plain-text fall-back if it cannot be.

Detection can always be overridden — which is what a `--color` flag needs, and
what makes styled output testable:

```rust
use cli_forge::{terminal, ColorChoice};

terminal::set_color_choice(ColorChoice::Never);
```

<br>

## Commands and arguments

```rust
use std::process::ExitCode;

use cli_forge::{out, App, Arg, Command};

fn main() -> ExitCode {
    let mut app = App::new("forge")
        .version(env!("CARGO_PKG_VERSION"))
        // An app-level argument, usable on either side of the command name.
        .arg(Arg::count("verbose").short('v').global(true));

    app.register(
        Command::new("build")
            .about("compile the project")
            .arg(Arg::positional("targets").multiple(true))
            .arg(Arg::flag("release").short('r'))
            .arg(Arg::option("jobs").short('j').default("1").value_name("N")
                .validate(|v| v.parse::<u16>().map(|_| ())
                    .map_err(|_| "expected a count".to_string())))
            .arg(Arg::option("level").possible_values(["warn", "info", "debug"]))
            .arg(Arg::option("token").env("FORGE_TOKEN"))
            // `?` works inside a handler against any error type.
            .run(|m| -> std::io::Result<()> {
                out(std::fs::read_to_string("forge.toml")?);
                Ok(())
            }),
    );

    app.register(
        Command::new("remote")
            .subcommand_required(true)
            .subcommand(Command::new("add").arg(Arg::positional("url").required(true))),
    );

    app.run()
}
```

Commands register **from anywhere** — a command built in a non-`main` module is
reachable and behaves identically, which is what lets each command live beside
the code it drives. All the standard argument forms parse (`--long`,
`--long=value`, `-s`, `-svalue`, `-s=value`, bundled `-abc`, `-vvv`, `--`), and
so do the two cases most parsers get wrong: a negative number (`-5`) and a lone
`-`.

Values are validated **at the edge**, so a bad one is refused with a proper
command-line error naming the flag, the value, and what would have been accepted
— before any of your code runs. Reading them back then cannot fail:

```rust
let jobs: u16 = m.get("jobs").unwrap_or(1);
```

`Matches::source` says whether a value came from the command line, the
environment, or a default — the distinction a sentinel default cannot express.

**Entry points.** Four, because programs want different things:

| Method | Parses | Runs handlers | Prints | Exits |
|---|---|---|---|---|
| `run` | process args | yes | yes | returns a code |
| `parse` | process args | yes | yes | yes |
| `try_run_from` | given args | yes | no | no |
| `try_parse_from` | given args | no | no | no |

`try_parse_from` parses and nothing else, which is what makes it the one to test
with.

<br>

## Errors

```text
error: unknown flag '--releaze'

  did you mean '--release'?

USAGE: forge build [options] [targets]...
```

A `ParseError` is a value: `kind()`, `subject()`, `detail()`, `suggestion()`,
`usage()`, `exit_code()`, `stream()`, and `report()` for the whole thing. Nothing
panics, and nothing prints until you ask. Help and version come back through the
same channel as `ErrorKind::HelpRequested` / `VersionRequested`, and are
successes — `exit_code()` is `0` and `stream()` is standard output, so `--help`
can be piped.

<br>

## Untrusted text

A string from outside the program — a filename, a commit message, a server
response — can contain escape sequences, and printing it verbatim hands the
terminal to whoever wrote it:

```rust
use cli_forge::{out, text};

out(text::sanitize(&untrusted_filename));
```

<br>

## For sibling crates

cli-forge owns parsing, output, command registration, and help. It deliberately
does not own tables, progress bars, gradients, layouts, or shells — those are
sibling crates that build on these seams:

- `text::width` / `strip` / `pad` / `truncate` / `wrap` — the one correct answer
  to "how many columns is this?", which nothing that draws a box can do without.
- `Style` / `Color` / `Theme` — so a table's borders and a progress bar's fill
  honour the host program's theme rather than inventing their own.
- `terminal::level` / `size` / `supports_unicode` — one capability decision for
  the whole program.
- `App::commands` / `Command::arguments` / `Arg::allowed_values` and the rest of
  the read-only accessors — so a completions or manual-page generator reads the
  live tree.

<br>

## Feature flags

| Feature | Default | Description |
|---------|---------|-------------|
| `std` | yes | Terminal detection, the stdout/stderr writers, and the command layer. Without it, the styling core still works on `alloc`. |
| `color` | yes | ANSI / styled output. Disable for plain output; the API stays complete and every styled value renders as its plain text. |
| `unicode` | yes | Correct display widths for CJK, emoji, and combining marks, from the Unicode East Asian Width tables. Without it widths are counted in characters — exact for Latin, Greek, and Cyrillic, wrong for the rest. |
| `termsize` | yes | Query the real terminal for its size, so help wraps to the window. Without it only `COLUMNS`/`LINES` are consulted. |
| `auth` | no | The auth seam: `App::auth`, `AuthRequest`, and enforcement of `requires_auth`. Adds no dependencies. |

<br>

## Documentation

| Document | Read this when |
|---|---|
| **[Guide](./docs/GUIDE.md)** | Start here. Install it, write a working program, understand the shape. ~15 minutes. |
| **[Output & Styling](./docs/OUTPUT.md)** | Colours, themes, markers, aligned columns, markup, measuring text. |
| **[Commands & Arguments](./docs/COMMANDS.md)** | Commands, the argument model, validation, errors, help, testing. |
| **[Recipes](./docs/RECIPES.md)** | The answer to a specific task, without the explanation. |
| **[API](./docs/API.md)** | The surface map and the exact stability promise. |
| **[docs.rs](https://docs.rs/cli-forge)** | Signatures and a runnable example for every item. |

The full index is at [`docs/`](./docs/README.md). Every code block in those
guides is compiled and run by `cargo test`, so none of it can go stale.

<br>

## Examples

```bash
cargo run --example quick_start     # a real command line, themed output
cargo run --example theme           # the eight levels, and restyling them
cargo run --example four_paths      # the same line, four ways
cargo run --example colors          # named, bright, indexed, hex, and rgb
cargo run --example status_report   # a realistic deploy-style status report
cargo run --example commands -- build --release -j 8
cargo run --example arguments -- build -vv -D A -D B a.rs b.rs
```

See the theme and its fall-backs under different terminals:

```bash
NO_COLOR=1       cargo run --example status_report   # no styling
NO_UNICODE=1     cargo run --example status_report   # ASCII markers
CLICOLOR_FORCE=1 cargo run --example status_report   # colour into a pipe
```

<br>

## Performance

The plain path is allocation-free for a string literal — proven by a
counting-allocator test (`tests/allocation.rs`), not asserted. Criterion means
(Windows x86_64, release):

| Operation | ns/op |
|-----------|------:|
| `out` plain write (`&str`) | ~9.5 |
| `paint`, no styling | ~37 |
| `paint`, named colour + bold | ~103 |
| `paint`, 24-bit foreground and background | ~184 |
| `markup`, four styled runs | ~234 |
| themed line | ~167 |
| `text::width`, styled | ~59 |
| command parse, rich invocation | ~2700 |
| `out`, through a capture | ~41 |

Styling costs more than the plain path because it builds an owned `String` and
encodes escape sequences — a cost paid only when colour is asked for. Reproduce
with `cargo bench --bench bench`.

<hr>
<br>

## Status

`v2.0.0`. The public surface is guaranteed under Semantic Versioning: no
breaking changes before `3.0`. See the SemVer promise in
[`docs/API.md`](./docs/API.md#stability), the
[migration table](./CHANGELOG.md#migrating-from-1x), and the
[`ROADMAP`](./dev/ROADMAP.md).

<hr>
<br>

## Contributing

See <a href="./dev/DIRECTIVES.md"><code>dev/DIRECTIVES.md</code></a> for engineering standards and the definition of done. Before a PR: `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features` must be clean.

<br>

<div id="license">
    <h2>License</h2>
    <p>Licensed under either of</p>
    <ul>
        <li><b>Apache License, Version 2.0</b> &mdash; <a href="./LICENSE-APACHE">LICENSE-APACHE</a></li>
        <li><b>MIT License</b> &mdash; <a href="./LICENSE-MIT">LICENSE-MIT</a></li>
    </ul>
    <p>at your option.</p>
</div>

<div align="center">
  <h2></h2>
  <sup>COPYRIGHT <small>&copy;</small> 2026 <strong>James Gober <me@jamesgober.com>.</strong></sup>
</div>
