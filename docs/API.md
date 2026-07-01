<h1 align="center">
    <img width="99" alt="Rust logo" src="https://raw.githubusercontent.com/jamesgober/rust-collection/72baabd71f00e14aa9184efcb16fa3deddda3a0a/assets/rust-logo.svg">
    <br><b>cli-forge</b><br>
    <sub><sup>API REFERENCE</sup></sub>
</h1>
<div align="center">
    <sup>
        <a href="../README.md" title="Project Home"><b>HOME</b></a>
        <span>&nbsp;│&nbsp;</span>
        <span>API</span>
        <span>&nbsp;│&nbsp;</span>
        <a href="../CHANGELOG.md" title="Changelog"><b>CHANGELOG</b></a>
        <span>&nbsp;│&nbsp;</span>
        <a href="../dev/ROADMAP.md" title="Roadmap"><b>ROADMAP</b></a>
    </sup>
</div>
<br>

> Complete reference for every public item in `cli-forge`, with examples.
>
> **Status: the public surface is FROZEN as of v0.5.0.** The output layer, the
> command layer, the help engine, and the auth seam are all implemented. The
> remaining 0.x releases add tests, docs, and internal optimization only — the
> surface documented here becomes the 1.0 contract. See
> [Stability](#stability) and [`dev/ROADMAP.md`](../dev/ROADMAP.md).

## Table of Contents

- [Overview](#overview)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Output: `out` / `err`](#output)
- [Styling path 1 — builder: `style`](#builder)
- [Styling path 2 — tags: `parse`](#tags)
- [Styling path 3 — named registry: `define_tag` / `tag`](#registry)
- [Colors and terminal behavior](#colors)
- [Commands: `App` / `Command`](#commands)
- [Arguments: `Arg`](#arguments)
- [Parsed results: `Matches`](#matches-section)
- [Errors: `ParseError`](#errors)
- [Auth seam](#auth)
- [Feature flags](#feature-flags)
- [Stability](#stability)
- [Performance notes](#performance)

---

## Overview

cli-forge unifies argument parsing and styled output under one API, with commands
that register at runtime. The design goal is the lightness of argh with the reach
of clap, and — unlike either — output styling lives in the *same* system as
parsing, so extensions (tables, progress, gradients) all speak one layer.

It owns parsing, output, command registration, and help. It does NOT own tables,
progress bars, gradients, layouts, or shells — those are sibling crates in the
cli collection that build on this crate's output API.

As of v0.5.0 the framework is feature-complete: a near-direct plain path
(`out`/`err`) and three ways to add color that all render to identical bytes, over
one cross-platform terminal backend; a recursive command tree with runtime
registration, aliases, arg/flag parsing, and structured non-panicking errors;
auto-generated `--help` / `--version`; and an auth seam that gates commands behind
a consumer-supplied hook.

---

## Installation

```toml
[dependencies]
cli-forge = "0.5"
```

Color is on by default. For a build that never emits escape sequences (the API
stays complete; every styled value renders as its plain text):

```toml
[dependencies]
cli-forge = { version = "0.5", default-features = false, features = ["std"] }
```

For the auth seam, enable the `auth` feature:

```toml
[dependencies]
cli-forge = { version = "0.5", features = ["auth"] }
```

---

## Quick Start

```rust
use cli_forge::{define_tag, err, out, parse, style, tag};

// Plain output — the common case, one call, no allocation for a literal.
out("building...");
err("something went wrong");

// Styling, three ways, all rendering to the same bytes for the same intent:
out(style("done").green().bold());                  // builder
parse("<c=red><b>ERROR:</b></c> <c=#ff8800>low disk</c>"); // inline tags
define_tag("error", style("").red().bold());        // named registry
out(tag("error").render_with("build failed"));
```

---

<h2 id="output">Output: <code>out</code> / <code>err</code></h2>

The plain path. No tag parsing, no styling work — the value is formatted straight
to the stream and followed by a newline. This is the hot path and stays cheap: a
string literal is a near-direct write with no heap allocation.

```rust
pub fn out<T: std::fmt::Display>(value: T);
pub fn err<T: std::fmt::Display>(value: T);
```

**Parameters**

- `value` — anything that implements [`Display`]. A `&str` is written directly; a
  [`Style`](#builder) renders on the way out via its own `Display`; a `String`
  from [`parse`](#tags) or [`tag`](#registry) is written verbatim. `out` writes to
  standard output, `err` to standard error.

**Behavior**

- A trailing newline is always appended (these are line-oriented, like
  `println!`/`eprintln!`).
- A failed write — a closed pipe, for instance — is silently ignored. A
  fire-and-forget print must not panic or abort the program.

**Examples**

Plain lines and formatted values:

```rust
use cli_forge::out;

out("compiling 12 crates");
out(format!("compiled {} of {} targets", 12, 12));

let path = "config.toml";
out(format!("wrote {path}"));
```

Errors and diagnostics on standard error:

```rust
use cli_forge::{err, style};

err("error: missing required argument `--input`");
err(style("error:").red().bold()); // styled marker, plain text follows
```

Mixing plain and styled in the same stream:

```rust
use cli_forge::{out, style};

out("Summary");
out(style("  3 passed").green());
out(style("  1 failed").red().bold());
```

---

<h2 id="builder">Styling path 1 — builder: <code>style</code></h2>

Function-call styling. Chain color and attribute methods onto a value, then drop
the result into [`out`](#output) — [`Style`] implements [`Display`]. Best when the
style is computed or one-off.

```rust
pub fn style<S: Into<String>>(text: S) -> Style;

impl Style {
    // The eight standard named colors:
    pub fn black(self) -> Style;
    pub fn red(self) -> Style;
    pub fn green(self) -> Style;
    pub fn yellow(self) -> Style;
    pub fn blue(self) -> Style;
    pub fn magenta(self) -> Style;
    pub fn cyan(self) -> Style;
    pub fn white(self) -> Style;
    // 24-bit color:
    pub fn hex(self, hex: &str) -> Style;       // "#rrggbb" or "rrggbb"
    pub fn rgb(self, r: u8, g: u8, b: u8) -> Style;
    // Attributes:
    pub fn bold(self) -> Style;
    pub fn underline(self) -> Style;
    // Render to an owned String:
    pub fn render(&self) -> String;
}
// Style: Display, Clone, Debug
```

**`style(text)`**

- `text` — anything convertible into a `String`, so both string literals and
  owned `String`s work. The returned `Style` starts plain.

**The named-color methods** (`black` … `white`) each set the foreground to one of
the eight standard ANSI colors and return `self`, so they chain. Setting a color
twice keeps the last one.

**`hex(hex)`**

- `hex` — a hex color string. A leading `#` is optional; the rest must be exactly
  six hex digits. An invalid string leaves the current color unchanged, so the
  builder never fails or panics.

**`rgb(r, g, b)`**

- `r`, `g`, `b` — the red, green, and blue channels, each `0..=255`. Equivalent to
  the matching `hex` string. On terminals without 24-bit support the color is
  downgraded at render time (see [Colors](#colors)).

**`bold()` / `underline()`** set the respective attribute and return `self`.

**`render(&self) -> String`** renders to an owned `String`, the same bytes
producing the value via `Display`. The color depth matches the terminal detected
for standard output, so on a pipe or under `NO_COLOR` the result is the plain
text.

> Method call order does not affect the output. Parameters are always emitted in
> a fixed canonical order — bold, underline, then color — so the same intent
> yields the same bytes no matter how you chain.

**Examples**

Named colors and attributes:

```rust
use cli_forge::{out, style};

out(style("PASS").green().bold());
out(style("note").cyan());
out(style("deprecated").yellow().underline());
```

24-bit color via hex and rgb:

```rust
use cli_forge::{out, style};

out(style("amber warning").hex("#ff8800"));
out(style("teal ok").rgb(0, 200, 120));
out(style("https://example.com").hex("#3b82f6").underline());
```

Rendering to a string instead of printing — for logging, tables, or further
composition:

```rust
use cli_forge::style;

let label = style("ERROR").red().bold().render();
let line = format!("{label}: {}", "build failed");
assert!(line.contains("ERROR"));
```

Invalid hex is ignored rather than panicking:

```rust
use cli_forge::style;

let s = style("x").hex("not-a-color"); // color left unset
assert_eq!(s.render(), "x");
```

---

<h2 id="tags">Styling path 2 — tags: <code>parse</code></h2>

Inline markup. The whole line is one string with tags; parsing cost is paid only
here, never in [`out`](#output). Best when the text and its styling are written
together, like a template.

```rust
pub fn parse<S: AsRef<str>>(tags: S);
```

**Parameters**

- `tags` — anything that is `AsRef<str>` (a `&str` or `String`). The styled result
  is printed to standard output, followed by a newline.

**Tag grammar**

| Tag | Effect |
|-----|--------|
| `<b>…</b>` | bold |
| `<u>…</u>` | underline |
| `<c=VALUE>…</c>` | foreground color; `VALUE` is a named color, `#rrggbb`, or `r,g,b` |
| `</>` | close the most recently opened tag, whatever it was |

Tags nest. Anything that is not a recognized tag is emitted verbatim, so `parse`
never rejects input — a stray `<` or an unknown `<tag>` simply prints as written.
A `<c=…>` with an unparseable value opens a balanced span that inherits the
surrounding color rather than failing.

**Examples**

A diagnostic line with a colored, bold marker:

```rust
use cli_forge::parse;

parse("<c=red><b>ERROR:</b></c> <c=#ff8800>disk almost full</c>");
```

Nested and mixed styling in one template:

```rust
use cli_forge::parse;

parse("<b>tests</b>: <c=green>12 passed</c>, <c=red>1 failed</c>, <c=128,128,128>3 skipped</c>");
```

Plain text and stray delimiters pass through unharmed:

```rust
use cli_forge::parse;

parse("use a < b to compare; <unknown> tags print literally");
```

The rendered bytes are identical to the equivalent [builder](#builder) output for
the same intent — `parse("<c=red><b>X</b></c>")` matches
`style("X").red().bold()`.

---

<h2 id="registry">Styling path 3 — named registry: <code>define_tag</code> / <code>tag</code></h2>

Define a style once by name, recall it anywhere. The DRY path: describe the look
in one place — even one module — and reuse it by name across the program. Best
when the same style recurs.

```rust
pub fn define_tag<S: Into<String>>(name: S, style: Style);
pub fn tag(name: &str) -> Tag;

impl Tag {
    pub fn render_with(&self, text: &str) -> String;
}
// Tag: Clone, Copy, Debug
```

**`define_tag(name, style)`**

- `name` — the lookup key, anything convertible into a `String`.
- `style` — a [`Style`] whose color and attributes are captured; its *text* is
  ignored, so the idiom is to define from an empty `style("")`. Defining the same
  name again replaces the previous definition.

**`tag(name) -> Tag`**

- `name` — the key passed to `define_tag`. An unknown name yields a `Tag` that
  renders its text plain, so missing definitions degrade gracefully rather than
  erroring.

**`Tag::render_with(text) -> String`**

- `text` — the text to render with the captured style. Returns an owned `String`;
  color depth matches the terminal detected for standard output.

**Examples**

Define a small palette up front, reuse it everywhere:

```rust
use cli_forge::{define_tag, out, style, tag};

define_tag("ok", style("").green().bold());
define_tag("warn", style("").yellow().bold());
define_tag("fail", style("").red().bold());

out(tag("ok").render_with("[ok]   resolve dependencies"));
out(tag("warn").render_with("[warn] no lockfile found"));
out(tag("fail").render_with("[fail] smoke test"));
```

Reuse across modules — a name defined anywhere resolves everywhere:

```rust
use cli_forge::{define_tag, style};

mod theme {
    use cli_forge::{define_tag, style};
    pub fn install() {
        define_tag("heading", style("").bold().underline());
    }
}

theme::install();
// ...elsewhere:
use cli_forge::{out, tag};
out(tag("heading").render_with("Results"));
```

Unknown names render plain instead of failing:

```rust
use cli_forge::tag;

assert_eq!(tag("never-defined").render_with("text"), "text");
```

---

<h2 id="colors">Colors and terminal behavior</h2>

**Color depth and graceful degradation.** The terminal's capability is detected
once, from standard output, and applied to all styled rendering. A 24-bit color is
emitted exactly on a true-color terminal; on a 256-color terminal it is downgraded
to the nearest cube entry; on a 16-color terminal it is downgraded to the nearest
of the eight standard colors. Named colors always map to their standard code and
never need downgrading.

**When color is dropped entirely** (styled values render as plain text):

- standard output is not a terminal (a pipe or a file);
- the `NO_COLOR` environment variable is set (and non-empty);
- `TERM=dumb`;
- the crate is built without the `color` feature.

`CLICOLOR_FORCE` (set and not `0`) forces color on, overriding a non-terminal
stream and `NO_COLOR`. Depth then comes from `COLORTERM`
(`truecolor`/`24bit` ⇒ 24-bit) and `TERM` (`*256color*` ⇒ 256-color), defaulting
to the 16 standard colors.

**Windows.** The Windows console is driven through the same ANSI backend as Unix
terminals; virtual-terminal processing is enabled automatically the first time
color is used. If it cannot be enabled, output falls back to plain text rather
than printing visible escape sequences.

---

<h2 id="commands">Commands: <code>App</code> / <code>Command</code></h2>

A recursive command tree, registered into an [`App`] **from anywhere** — not just
`main`. [`App::parse`](#app-parse) resolves the invocation, parses its arguments,
and runs the selected command's handler.

```rust
use cli_forge::{App, Arg, Command, out};

let mut app = App::new("forge")
    .help_header("forge — project constructor")
    .help_footer("docs: https://github.com/jamesgober/cli-forge");

app.register(
    Command::new("init")
        .about("bootstrap a new project")
        .arg(Arg::positional("name").required(true))
        .run(|m| out(format!("init {}", m.value("name").unwrap_or("?")))),
);
app.register(Command::new("secret").hidden(true));
app.register(Command::new("publish").requires_auth(true));

// `try_parse_from` is the non-exiting form used here for illustration.
let matches = app.try_parse_from(["init", "demo"]).unwrap();
assert_eq!(matches.subcommand().unwrap().0, "init");
```

### `Command`

A node in the tree. Build with `Command::new`, refine by chaining, attach a
handler with `run`.

| Method | Description |
|--------|-------------|
| `Command::new(name)` | Create a command with the given invocation name. |
| `.alias(name)` / `.aliases(iter)` | Alternative invocation names. Resolve to the canonical command; shown in help. |
| `.about(text)` | One-line description, shown in help. |
| `.arg(arg)` | Accept an [`Arg`](#arguments). Positionals fill in declaration order. |
| `.subcommand(cmd)` | Nest a child command; composes to any depth. |
| `.hidden(yes)` | Hide from generated help while staying invokable. |
| `.requires_auth(yes)` | Mark as auth-gated. With the `auth` feature, runs and shows in help only when the [auth hook](#auth) authorizes it; inert without the feature. |
| `.run(handler)` | `Fn(&Matches) + 'static` run when this command is selected. |

`name`, `text` accept anything `Into<String>`. An alias resolves to the canonical
command — `matches.subcommand()` reports the canonical name regardless of which
alias was typed.

**Help and version are automatic.** `-h` / `--help` at any level renders that
level's help (top-level or a specific command); `-V` / `--version` prints
`App::version(...)` if set. A command can override the built-ins by declaring its
own `help` / `h` argument. Hidden and auth-gated commands are omitted from help
listings.

### `App`

The registry and entry point.

| Method | Description |
|--------|-------------|
| `App::new(name)` | Create an application with the program name. |
| `.version(text)` | Set the version reported by `-V` / `--version`. Without it, those flags are ordinary unknown flags. |
| `.help_header(text)` / `.help_footer(text)` | Header/footer wrapping every generated help page. |
| `.register(&mut self, cmd)` | Add a top-level command. Callable from any module, any time before parsing. |
| `.help() -> String` | Render the top-level help on demand (e.g. a no-command fallback). |
| `.parse() -> Matches` | <a id="app-parse"></a>Parse `std::env::args()`, run the handler, return matches. `-h`/`--help` and `-V`/`--version` print to stdout and **exit 0**; malformed input prints a structured error to stderr and **exits 2** — never panics. |
| `.try_parse_from(args) -> Result<Matches, ParseError>` | Non-exiting twin: takes an explicit arg list (excluding the program name), runs the handler, returns the matches or a structured error (including the `HelpRequested`/`VersionRequested` signals). Ideal for embedding and tests. |

**Registration from anywhere.** `register` takes `&mut App`, so a command built
in any module — a plugin, a feature module, a config loop — is reachable and
behaves identically to one built in `main`:

```rust
use cli_forge::{App, Command};

mod plugin {
    use cli_forge::{App, Command};
    pub fn install(app: &mut App) {
        app.register(Command::new("sync").about("synchronize state"));
    }
}

let mut app = App::new("demo");
plugin::install(&mut app); // registered outside `main`
let matches = app.try_parse_from(["sync"]).unwrap();
assert_eq!(matches.subcommand().unwrap().0, "sync");
```

**Nested subcommands** dispatch to the deepest selected command:

```rust
use cli_forge::{App, Arg, Command, out};

let mut app = App::new("demo");
app.register(
    Command::new("remote").subcommand(
        Command::new("add")
            .arg(Arg::positional("url").required(true))
            .run(|m| out(format!("added {}", m.value("url").unwrap_or("?")))),
    ),
);
let _ = app.try_parse_from(["remote", "add", "https://example.com"]).unwrap();
```

---

<h2 id="arguments">Arguments: <code>Arg</code></h2>

An argument a command accepts. Three kinds, each with a constructor; the builder
methods refine and chain.

| Constructor | Form | Example input |
|-------------|------|---------------|
| `Arg::flag(name)` | boolean switch | `--verbose`, `-v` |
| `Arg::option(name)` | named value | `--output f`, `--output=f`, `-o f`, `-of` |
| `Arg::positional(name)` | value by position | `path/to/file` |

| Method | Description |
|--------|-------------|
| `.short(c)` | One-letter form `-c` (flag/option). |
| `.long(s)` | Override the `--long` form (defaults to the name). |
| `.help(s)` | Help text (surfaced by the help engine, v0.4.0). |
| `.required(b)` | Fail with `MissingRequired` if absent and no default. |
| `.default(s)` | Value used when an option/positional is omitted. |

The `name` is the key used to read the value back out of a [`Matches`](#matches-section).

```rust
use cli_forge::{App, Arg, Command};

let mut app = App::new("demo");
app.register(
    Command::new("build")
        .arg(Arg::flag("release").short('r'))
        .arg(Arg::option("jobs").short('j').default("1"))
        .arg(Arg::positional("target").default("all")),
);

let m = app.try_parse_from(["build", "-r", "-j", "8", "lib"]).unwrap();
let (_, build) = m.subcommand().unwrap();
assert!(build.flag("release"));
assert_eq!(build.value("jobs"), Some("8"));
assert_eq!(build.value("target"), Some("lib"));
```

**Parsing forms handled:** `--long`, `--long value`, `--long=value`, `-s`,
`-s value`, `-svalue`, bundled short flags `-abc`, positionals, and the `--`
end-of-options marker (everything after it is positional). A token like `-5` is
read as a short flag; put it after `--` to pass a negative-number positional.

---

<h2 id="matches-section">Parsed results: <code>Matches</code></h2>

What the parser produces for one command level, and what a `run` handler receives.

| Method | Description |
|--------|-------------|
| `.flag(name) -> bool` | Whether the flag was set (`false` for unknown names). |
| `.value(name) -> Option<&str>` | An option/positional value, or its default; `None` if absent and undefaulted. |
| `.subcommand() -> Option<(&str, &Matches)>` | The invoked subcommand's name and its own matches. |

```rust
use cli_forge::{App, Arg, Command};

let mut app = App::new("git-like");
app.register(
    Command::new("commit")
        .arg(Arg::flag("amend"))
        .arg(Arg::option("message").short('m')),
);

let top = app.try_parse_from(["commit", "--amend", "-m", "fix"]).unwrap();
let (name, commit) = top.subcommand().unwrap();
assert_eq!(name, "commit");
assert!(commit.flag("amend"));
assert_eq!(commit.value("message"), Some("fix"));
```

---

<h2 id="errors">Errors: <code>ParseError</code></h2>

Every malformed input maps to a `ParseError` variant — never a panic. Returned by
[`try_parse_from`](#app-parse); [`parse`](#app-parse) renders it through the output
layer and exits. The enum is `#[non_exhaustive]`.

| Variant | Cause |
|---------|-------|
| `UnknownFlag { flag }` | A `-x` / `--name` no argument declares. |
| `MissingValue { option }` | An option given without its value. |
| `MissingRequired { arg }` | A required argument omitted (and no default). |
| `UnknownCommand { name }` | A token where a registered subcommand was expected. |
| `UnexpectedArgument { value }` | A surplus value with nowhere to go. |
| `HelpRequested(String)` | Not an error: `-h`/`--help` was requested. Carries the rendered help. |
| `VersionRequested(String)` | Not an error: `-V`/`--version` was requested. Carries the version. |
| `Unauthorized { command }` | An auth-gated command was invoked without authorization (feature `auth`). |

`ParseError` implements `Display` and `std::error::Error`. The last two variants
are control signals, not failures: `parse` prints them to standard output and
exits `0`; `try_parse_from` callers should do the same.

```rust
use cli_forge::{App, Arg, Command, ParseError};

let mut app = App::new("demo");
app.register(Command::new("build").arg(Arg::option("jobs").short('j')));

match app.try_parse_from(["build", "-j"]) {
    Err(ParseError::MissingValue { option }) => assert_eq!(option, "jobs"),
    other => panic!("expected MissingValue, got {other:?}"),
}
```

---

<h2 id="auth">Auth seam (feature <code>auth</code>)</h2>

cli-forge holds the *seam*, not the logic. A command marked `.requires_auth(true)`
runs — and appears in help — only when the app's authorization hook allows it. The
hook, supplied by the consumer (or a sibling `cli-auth` crate), is where
login/logout state actually lives; the core just asks.

| Item | Description |
|------|-------------|
| `App::auth(hook)` | Set the hook: `Fn(&AuthRequest) -> bool + 'static`. |
| `AuthRequest::command() -> &str` | The name of the command being authorized. |
| `AuthRequest::path() -> &[&str]` | The full command-name chain (e.g. `["remote", "add"]`). |
| `ParseError::Unauthorized { command }` | Returned when an auth-gated command is refused. |

- **Fail closed.** With no hook set, auth-gated commands are never authorized —
  they neither run nor appear in help.
- **Enforcement point.** The hook is consulted after a successful parse, before
  the command's handler runs. A refusal returns `Unauthorized` (handler skipped);
  under [`parse`](#app-parse) that prints to standard error and exits `2`.
- **Help.** An unauthorized auth-gated command is omitted from help listings. The
  hook is consulted during help generation too, so it should be **pure and
  cheap** — check already-loaded session state, don't do I/O.
- **Without the feature.** `requires_auth` is inert: the command runs and shows
  normally. `App::auth` and `AuthRequest` are absent.

```rust
# #[cfg(feature = "auth")]
# {
use cli_forge::{App, Command, ParseError};

let mut app = App::new("demo").auth(|req| {
    // Authorize everything except `publish` (until the session is valid).
    req.command() != "publish"
});
app.register(Command::new("status").run(|_| { /* open */ }));
app.register(Command::new("publish").requires_auth(true).run(|_| { /* gated */ }));

// `status` runs; `publish` is refused.
assert!(app.try_parse_from(["status"]).is_ok());
let err = app.try_parse_from(["publish"]).unwrap_err();
assert!(matches!(err, ParseError::Unauthorized { .. }));
# }
```

---

<h2 id="feature-flags">Feature flags</h2>

| Feature | Default | Description |
|---------|---------|-------------|
| `std` | yes | Standard library: terminal detection, the stdout/stderr writers, and the command layer. |
| `color` | yes | ANSI / styled output. Implies `std`. Disable for plain output (still complete). |
| `auth` | no | The [auth seam](#auth): `App::auth`, `AuthRequest`, and enforcement of `requires_auth`. Implies `std`. |

cli-forge's core has no heavy mandatory dependencies. The only platform-specific
piece is enabling the Windows console's ANSI mode, pulled in by `color` on Windows
targets alone. The `auth` feature adds no dependencies.

---

<h2 id="stability">Stability</h2>

The public surface documented above is **frozen as of v0.5.0**. It is
feature-complete: the remaining 0.x releases add tests, documentation, and
internal optimization, plus strictly-additive conveniences — they do not change or
remove existing API. This surface becomes the 1.0 contract, after which it follows
SemVer strictly (a breaking change requires a MAJOR bump).

Frozen public items: `out`, `err`, `parse`, `style` / `Style`, `define_tag` /
`tag` / `Tag`, `App`, `Command`, `Arg`, `Matches`, `ParseError`, and — behind the
`auth` feature — `App::auth` / `AuthRequest`.

---

<h2 id="performance">Performance notes</h2>

The plain path is the hot path and is allocation-free for a string literal: `out`
formats the value straight to the stream with no intermediate buffer. This is
proven by a counting-allocator test, not asserted — see `tests/allocation.rs`.

Local Criterion means (Windows x86_64, Rust stable, release build):

| Operation | ns/op |
|-----------|------:|
| `out` plain write (`&str`) | ~10 |
| builder render, named color + bold | ~50 |
| builder render, 24-bit color | ~75 |
| named-registry render | ~43 |

The styling paths cost more than the plain path because they build an owned
`String` and encode escape sequences; that cost is paid only when you opt into
color. Reproduce with `cargo bench --bench bench`.

---

<sub>Copyright &copy; 2026 <strong>James Gober</strong>.</sub>
