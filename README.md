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
        cli-forge is a unified command-line framework where argument parsing and styled output speak one API. Commands register at runtime - from anywhere, not just main - and can be hidden or auth-gated; output flows through a single layer (plain, tag-parsed, or builder-styled) that extensions like tables, progress, and gradients reuse seamlessly. It targets the lightness of argh with the reach of clap, without the split between parsing in one crate and styling in five.
    </p>
    <br>
    <hr>
    <p>
        <strong>MSRV is 1.85+</strong> (Rust 2024 edition).
    </p>
    <blockquote>
        <strong>Status: pre-1.0, in active development.</strong> The public API is being designed across the 0.x series and frozen at <code>1.0.0</code>. See <a href="./CHANGELOG.md"><code>CHANGELOG.md</code></a>.
    </blockquote>
</div>

<hr>
<br>

## What's here

As of `v0.5.0` the framework is **feature-complete and the public surface is
frozen** — the output layer, command layer, help engine, and auth seam are all in
place:

- **Plain output** — `out` / `err`: one call, no parsing, no allocation for a
  string literal. The hot path stays cheap.
- **Three styling paths** — a chainable `style` builder, inline `parse` tags, and
  a named `define_tag` / `tag` registry. All three render to **identical bytes**
  for the same intent.
- **Full color** — the eight standard names plus any 24-bit color via hex or RGB,
  with graceful downgrade to 256- or 16-color terminals and clean fall-back to
  plain text on pipes, `NO_COLOR`, or the Windows console without ANSI.
- **Command tree** — a recursive `Command` tree with a full argument model
  (flags, **counting** flags `-vvv`, options, **repeatable** options, positionals,
  **variadic** positionals) and **aliases**, registered into an `App` **from
  anywhere** (not just `main`), with `.hidden()` / `.requires_auth()` flags and
  structured, non-panicking errors.
- **Help & version** — auto-generated `--help` / `-h` (top-level and per-command)
  and `--version` / `-V`, rendered through the output layer with injectable
  header/footer.
- **Auth seam** *(feature `auth`)* — gate a command behind a consumer-supplied
  authorization hook. cli-forge holds the seam; the login state lives in your code.
  Fails closed, and hides unauthorized commands from help.

The remaining `0.x` releases are docs, tests, and optimization only — this surface
is the `1.0` contract. See the [`ROADMAP`](./dev/ROADMAP.md).

<hr>
<br>

## Installation

```toml
[dependencies]
cli-forge = "0.6"
```

Color is on by default. For a build that never emits escape sequences (the API
stays complete; every styled value renders as its plain text):

```toml
[dependencies]
cli-forge = { version = "0.6", default-features = false, features = ["std"] }
```

<br>

## Quick Start

```rust
use cli_forge::{define_tag, err, out, parse, style, tag};

// Plain output — the common case.
out("building...");
err("something went wrong");

// Styling, three ways, all rendering to the same bytes for the same intent:
out(style("done").green().bold());                          // builder
parse("<c=red><b>ERROR:</b></c> <c=#ff8800>low disk</c>");  // inline tags
define_tag("error", style("").red().bold());                // named registry
out(tag("error").render_with("build failed"));
```

<br>

## The three styling paths

The same styled line, produced three ways. The choice is ergonomic, not visual —
the bytes are identical.

```rust
use cli_forge::{define_tag, out, parse, style, tag};

// 1. Builder — chain methods; the result is `Display`. Best for computed/one-off.
out(style("ERROR: build failed").red().bold());

// 2. Tags — one string with inline markup. Best when text and style live together.
parse("<c=red><b>ERROR: build failed</b></c>");

// 3. Named registry — define the look once, reuse by name across the program.
define_tag("error", style("").red().bold());
out(tag("error").render_with("ERROR: build failed"));
```

**Tag grammar:** `<b>…</b>` (bold), `<u>…</u>` (underline),
`<c=VALUE>…</c>` (color, where `VALUE` is a named color, `#rrggbb`, or `r,g,b`),
and `</>` to close the innermost tag. Tags nest; unrecognized tags print
literally, so `parse` never rejects input.

<br>

## Colors and terminals

Colors are the eight standard names, plus any 24-bit value:

```rust
use cli_forge::{out, style};

out(style("amber").hex("#ff8800"));
out(style("teal").rgb(0, 200, 120));
out(style("link").hex("#3b82f6").underline());
```

The terminal's capability is detected once. A 24-bit color renders exactly on a
true-color terminal, downgrades to the nearest 256- or 16-color value where that
is all the terminal supports, and falls away to plain text when output is a pipe,
`NO_COLOR` is set, or the crate is built without `color`. `CLICOLOR_FORCE`
overrides detection and forces color on. The Windows console is handled behind the
same API — virtual-terminal mode is enabled automatically, with a plain-text
fall-back if it cannot be.

<br>

## Commands

Build a recursive command tree, register commands into an `App` from anywhere, and
let `parse` resolve the invocation, parse arguments, and run the selected
command's handler:

```rust
use cli_forge::{App, Arg, Command, out};

let mut app = App::new("forge");

app.register(
    Command::new("build")
        .about("compile the project")
        .arg(Arg::flag("release").short('r'))
        .arg(Arg::option("jobs").short('j').default("1"))
        .run(|m| out(format!(
            "release={} jobs={}",
            m.flag("release"),
            m.value("jobs").unwrap_or("?"),
        ))),
);

app.register(
    Command::new("remote").subcommand(
        Command::new("add")
            .arg(Arg::positional("url").required(true))
            .run(|m| out(format!("added {}", m.value("url").unwrap_or("?")))),
    ),
);

let _ = app.parse();
```

Commands register **from anywhere** — a command built in a non-`main` module is
reachable and behaves identically, the limitation that made the predecessor
unusable. Give a command extra names with `.alias("rm")` / `.aliases(["rm", "del"])`;
aliases resolve to the canonical command. Arguments cover flags, counting flags
(`Arg::count`, read with `count()`), options, repeatable options and variadic
positionals (`.multiple(true)`, read with `values()`), and positionals — parsed
from all the standard forms (`--long`, `--long=value`, `-s`, `-svalue`, bundled
`-abc`, `-vvv`, `--`). Malformed input becomes a structured `ParseError`: `parse`
prints it and exits `2`, never panicking; `try_parse_from` returns it instead.
`.hidden()` keeps a command invokable but out of help; `.requires_auth()` gates it
behind the auth hook (feature `auth`).

**Help and version come for free.** `-h` / `--help` renders styled help for the
app or any command (with your `help_header` / `help_footer`); `-V` / `--version`
prints `App::version(...)`. `App::help()` renders the top-level help as a string
whenever you want it. Both exit `0` under `parse`; `try_parse_from` returns them
as `ParseError::HelpRequested` / `VersionRequested` control signals.

<br>

## Feature flags

| Feature | Default | Description |
|---------|---------|-------------|
| `std` | yes | Standard library: terminal detection, the stdout/stderr writers, and the command layer. |
| `color` | yes | ANSI / styled output. Implies `std`. Disable for plain output (still complete). |
| `auth` | no | The auth seam: `App::auth`, `AuthRequest`, and enforcement of `requires_auth`. Implies `std`; adds no dependencies. |

<br>

## Examples

Runnable examples live in [`examples/`](./examples):

```bash
cargo run --example quick_start     # plain output and one styled line
cargo run --example three_paths     # the same line, three ways
cargo run --example colors          # named, hex, and rgb color
cargo run --example status_report   # a realistic deploy-style status report
cargo run --example commands -- build --release -j 8   # the command tree
cargo run --example arguments -- build -vv -D A -D B a.rs b.rs   # every arg kind
```

Force color when output is captured, or disable it, to see both paths:

```bash
CLICOLOR_FORCE=1 cargo run --example status_report
NO_COLOR=1       cargo run --example status_report
```

<br>

## Performance

The plain path is allocation-free for a string literal — proven by a
counting-allocator test (`tests/allocation.rs`), not asserted. Local Criterion
means (Windows x86_64, release build):

| Operation | ns/op |
|-----------|------:|
| `out` plain write (`&str`) | ~10 |
| builder render, named color + bold | ~50 |
| builder render, 24-bit color | ~75 |
| named-registry render | ~43 |

Styling costs more than the plain path because it builds an owned `String` and
encodes escape sequences — a cost paid only when you opt into color. Reproduce
with `cargo bench --bench bench`.

<hr>
<br>

## Status

The public surface is **frozen** (feature-complete at `v0.5.0`); `v0.6.0` added the
strictly-additive `count` / `multiple` argument conveniences the freeze permits.
Per the [`ROADMAP`](./dev/ROADMAP.md) and [`docs/API.md`](./docs/API.md), the
remaining `0.x` releases add tests, docs, and optimization only; `1.0.0` is the
formal freeze.

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
