# cli-core &mdash; API Reference

> Complete reference for every public item in `cli-core`, with examples.
> **Status: pre-1.0 — the surface below is the FROZEN planned design, built across the 0.x series.** Items marked _(planned, vX.Y.Z)_ are not yet implemented; see [`dev/ROADMAP.md`](../dev/ROADMAP.md). The signatures here are the contract sibling crates and tools build against.

## Table of Contents

- [Overview](#overview)
- [Installation](#installation)
- [Output: `out` / `err`](#output)
- [Styling path 1 — tags: `parse`](#tags)
- [Styling path 2 — builder: `style`](#builder)
- [Styling path 3 — named registry: `define_tag` / `tag`](#registry)
- [Commands: `Command` / `App`](#commands)
- [Feature flags](#feature-flags)

---

## Overview

cli-core unifies argument parsing and styled output under one API, with commands
that register at runtime. The design goal is the lightness of argh with the reach
of clap, and — unlike either — output styling lives in the *same* system as
parsing, so extensions (tables, progress, gradients) all speak one layer.

It owns parsing, output, command registration, and help. It does NOT own tables,
progress bars, gradients, layouts, or shells — those are sibling crates in the
cli collection that build on this crate's output API.

---

## Installation

```toml
[dependencies]
cli-core = "0.1"
```

---

## Output

_(planned, v0.2.0)_ The plain path. No tag parsing, no styling work — a
near-direct write. This is the hot path and stays cheap.

```rust,ignore
use cli_core::{out, err};

out("building...");          // plain stdout line
err("something went wrong"); // plain stderr line
```

## Tags

_(planned, v0.2.0)_ Opt into styling with an inline tag string. Named colors and
custom `hex`/`rgb`. Parsing cost is paid only here, never in `out`.

```rust,ignore
use cli_core::parse;

parse("<c=red><b>ERROR:</b></c> <c=#ff8800>disk almost full</c>");
```

## Builder

_(planned, v0.2.0)_ Function-call styling, chainable, `Display` so it drops
straight into `out`.

```rust,ignore
use cli_core::{out, style};

out(style("ERROR:").red().bold());
out(style("note").hex("#88aaff"));
out(style("ok").rgb(0, 200, 120));
```

```rust,ignore
pub fn style(s: impl Into<String>) -> Style;
impl Style {
    pub fn red(self) -> Style;            // + the standard named colors
    pub fn hex(self, hex: &str) -> Style;
    pub fn rgb(self, r: u8, g: u8, b: u8) -> Style;
    pub fn bold(self) -> Style;
    pub fn underline(self) -> Style;
    pub fn render(&self) -> String;
}
```

## Registry

_(planned, v0.2.0)_ Define a style once by name, recall it anywhere — DRY styling,
so colors/extensions reuse the write layer instead of redefining it.

```rust,ignore
use cli_core::{define_tag, tag, out, style};

define_tag("error", style("").red().bold());
// ...elsewhere, no re-specifying the style:
out(tag("error").render_with("ERROR: build failed"));
```

## Commands

_(planned, v0.3.0)_ A recursive command tree; an `App` registry that accepts
commands registered **from anywhere**, not just `main`. Commands can be hidden
from help or marked auth-gated.

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

## Feature flags

| Feature | Default | Description |
|---------|---------|-------------|
| `std` | yes | Standard library + terminal detection. |
| `color` | yes | ANSI / styled output. Disable for plain output (still complete). |
| `auth` | no | Enables enforcement of `requires_auth` via the auth hook. |

cli-core's core has no heavy mandatory dependencies; the terminal backend is the
only platform-specific piece.

---

<sub>Copyright &copy; 2026 <strong>James Gober</strong>.</sub>
