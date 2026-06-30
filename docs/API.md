<h1 align="center">
    <img width="99" alt="Rust logo" src="https://raw.githubusercontent.com/jamesgober/rust-collection/72baabd71f00e14aa9184efcb16fa3deddda3a0a/assets/rust-logo.svg">
    <br><b>cli-core</b><br>
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

> Complete reference for every public item in `cli-core`, with examples.
>
> **Status:** the output layer below — `out`/`err`, the three styling paths, and
> the color model — is implemented and stable as of **v0.2.0**. The
> command/registration surface (`Command`/`App`) is the FROZEN planned design and
> is marked _(planned, v0.3.0)_; its signatures are the contract sibling crates
> build against and will not drift. See [`dev/ROADMAP.md`](../dev/ROADMAP.md).

## Table of Contents

- [Overview](#overview)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Output: `out` / `err`](#output)
- [Styling path 1 — builder: `style`](#builder)
- [Styling path 2 — tags: `parse`](#tags)
- [Styling path 3 — named registry: `define_tag` / `tag`](#registry)
- [Colors and terminal behavior](#colors)
- [Commands: `Command` / `App`](#commands)
- [Feature flags](#feature-flags)
- [Performance notes](#performance)

---

## Overview

cli-core unifies argument parsing and styled output under one API, with commands
that register at runtime. The design goal is the lightness of argh with the reach
of clap, and — unlike either — output styling lives in the *same* system as
parsing, so extensions (tables, progress, gradients) all speak one layer.

It owns parsing, output, command registration, and help. It does NOT own tables,
progress bars, gradients, layouts, or shells — those are sibling crates in the
cli collection that build on this crate's output API.

This release delivers the output layer: a near-direct plain path (`out`/`err`)
and three ways to add color and attributes that all render to identical bytes for
the same intent, over one cross-platform terminal backend.

---

## Installation

```toml
[dependencies]
cli-core = "0.2"
```

Color is on by default. For a build that never emits escape sequences (the API
stays complete; every styled value renders as its plain text):

```toml
[dependencies]
cli-core = { version = "0.2", default-features = false, features = ["std"] }
```

---

## Quick Start

```rust
use cli_core::{define_tag, err, out, parse, style, tag};

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
use cli_core::out;

out("compiling 12 crates");
out(format!("compiled {} of {} targets", 12, 12));

let path = "config.toml";
out(format!("wrote {path}"));
```

Errors and diagnostics on standard error:

```rust
use cli_core::{err, style};

err("error: missing required argument `--input`");
err(style("error:").red().bold()); // styled marker, plain text follows
```

Mixing plain and styled in the same stream:

```rust
use cli_core::{out, style};

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
use cli_core::{out, style};

out(style("PASS").green().bold());
out(style("note").cyan());
out(style("deprecated").yellow().underline());
```

24-bit color via hex and rgb:

```rust
use cli_core::{out, style};

out(style("amber warning").hex("#ff8800"));
out(style("teal ok").rgb(0, 200, 120));
out(style("https://example.com").hex("#3b82f6").underline());
```

Rendering to a string instead of printing — for logging, tables, or further
composition:

```rust
use cli_core::style;

let label = style("ERROR").red().bold().render();
let line = format!("{label}: {}", "build failed");
assert!(line.contains("ERROR"));
```

Invalid hex is ignored rather than panicking:

```rust
use cli_core::style;

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
use cli_core::parse;

parse("<c=red><b>ERROR:</b></c> <c=#ff8800>disk almost full</c>");
```

Nested and mixed styling in one template:

```rust
use cli_core::parse;

parse("<b>tests</b>: <c=green>12 passed</c>, <c=red>1 failed</c>, <c=128,128,128>3 skipped</c>");
```

Plain text and stray delimiters pass through unharmed:

```rust
use cli_core::parse;

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
use cli_core::{define_tag, out, style, tag};

define_tag("ok", style("").green().bold());
define_tag("warn", style("").yellow().bold());
define_tag("fail", style("").red().bold());

out(tag("ok").render_with("[ok]   resolve dependencies"));
out(tag("warn").render_with("[warn] no lockfile found"));
out(tag("fail").render_with("[fail] smoke test"));
```

Reuse across modules — a name defined anywhere resolves everywhere:

```rust
use cli_core::{define_tag, style};

mod theme {
    use cli_core::{define_tag, style};
    pub fn install() {
        define_tag("heading", style("").bold().underline());
    }
}

theme::install();
// ...elsewhere:
use cli_core::{out, tag};
out(tag("heading").render_with("Results"));
```

Unknown names render plain instead of failing:

```rust
use cli_core::tag;

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

<h2 id="commands">Commands: <code>Command</code> / <code>App</code></h2>

_(planned, v0.3.0)_ A recursive command tree; an `App` registry that accepts
commands registered **from anywhere**, not just `main`. Commands can be hidden
from help or marked auth-gated. The signatures below are frozen.

```rust,ignore
use cli_core::{App, Command};

let mut app = App::new("forge")
    .help_header("forge — project constructor")
    .help_footer("docs: https://github.com/jamesgober/cli-core");

app.register(
    Command::new("init")
        .about("bootstrap a planned lib")
        .run(|m| { /* ... */ })
);
app.register(Command::new("secret").hidden(true).run(|_| {}));
app.register(Command::new("publish").requires_auth(true).run(|_| {}));

let matches = app.parse();
```

```rust,ignore
impl Command {
    pub fn new(name: &str) -> Command;
    pub fn about(self, text: &str) -> Command;
    pub fn arg(self, arg: Arg) -> Command;
    pub fn subcommand(self, cmd: Command) -> Command;
    pub fn hidden(self, yes: bool) -> Command;
    pub fn requires_auth(self, yes: bool) -> Command;
    pub fn run(self, handler: impl Fn(&Matches)) -> Command;
}
impl App {
    pub fn new(name: &str) -> App;
    pub fn register(&mut self, cmd: Command);
    pub fn help_header(self, text: &str) -> App;
    pub fn help_footer(self, text: &str) -> App;
    pub fn parse(&self) -> Matches;
}
```

---

<h2 id="feature-flags">Feature flags</h2>

| Feature | Default | Description |
|---------|---------|-------------|
| `std` | yes | Standard library: terminal detection and the stdout/stderr writers. |
| `color` | yes | ANSI / styled output. Implies `std`. Disable for plain output (still complete). |
| `auth` | no | Reserved for enforcement of the `requires_auth` command flag (v0.5.0); no effect yet. |

cli-core's core has no heavy mandatory dependencies. The only platform-specific
piece is enabling the Windows console's ANSI mode, pulled in by `color` on Windows
targets alone.

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
