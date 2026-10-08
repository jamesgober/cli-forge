<h1 align="center">
    <img width="90px" height="auto" src="https://raw.githubusercontent.com/jamesgober/jamesgober/main/media/icons/hexagon-3.svg" alt="Triple Hexagon">
    <br><b>cli-forge · Documentation</b>
</h1>

<p align="center">
  <code>v2.0.0</code> &mdash; the documentation index.
</p>

<div align="center">
    <a href="https://crates.io/crates/cli-forge"><img alt="Crates.io" src="https://img.shields.io/crates/v/cli-forge"></a>
    <a href="https://docs.rs/cli-forge"><img alt="docs.rs" src="https://img.shields.io/docsrs/cli-forge"></a>
    <a href="https://github.com/rust-lang/rfcs/blob/master/text/2495-min-rust-version.md"><img alt="MSRV" src="https://img.shields.io/badge/MSRV-1.85%2B-blue"></a>
</div>

<br>

<div align="left">
    <p>
        Everything here is written to be read in order, by someone who has not used this crate before. Every example is complete and runnable &mdash; nothing is a fragment that needs imports guessed at.
    </p>
</div>

<hr>
<br>

## Start here

| | Read this when |
|---|---|
| **[Guide](./GUIDE.md)** | You are new. Install it, write a working program, understand the shape of the thing. ~15 minutes. |
| **[Output & Styling](./OUTPUT.md)** | You want to make output look right: colours, themes, markers, columns, markup, measuring text. |
| **[Commands & Arguments](./COMMANDS.md)** | You want to define commands, parse arguments, validate values, and report errors well. |
| **[Recipes](./RECIPES.md)** | You have a specific task and want the answer, not the explanation. |
| **[API](./API.md)** | You want the surface map and the exact stability promise. |

If you only read one thing, read the [Guide](./GUIDE.md).

<br>

## By task

**Output and appearance**

- [Print something](./GUIDE.md#your-first-output) &middot; [`out` / `err`](./OUTPUT.md#plain-output)
- [Report success, failure, a warning](./OUTPUT.md#themed-responses) &mdash; the thing most programs want
- [Change how the whole program looks, in one place](./OUTPUT.md#building-a-theme)
- [Build a status line with an aligned column](./OUTPUT.md#named-styles) &middot; [Recipe](./RECIPES.md#a-status-line-with-aligned-columns)
- [Colour text](./OUTPUT.md#colour) &middot; [backgrounds](./OUTPUT.md#backgrounds) &middot; [bold, italic, the rest](./OUTPUT.md#attributes)
- [Style part of a sentence](./OUTPUT.md#markup) &mdash; inline markup
- [Add a hyperlink](./OUTPUT.md#hyperlinks)
- [Measure, pad, truncate, or wrap text](./OUTPUT.md#measuring-text)
- [Support `--color=never`](./OUTPUT.md#controlling-colour) &middot; [Recipe](./RECIPES.md#wire-up-a---color-flag)
- [Print a filename you do not trust](./OUTPUT.md#untrusted-text)

**Commands and input**

- [Define a command](./COMMANDS.md#commands) &middot; [nest subcommands](./COMMANDS.md#subcommands)
- [Accept a flag, an option, a positional](./COMMANDS.md#the-four-kinds-of-argument)
- [Read a value back as a number](./COMMANDS.md#typed-values)
- [Reject a bad value properly](./COMMANDS.md#validating-values)
- [Read a secret from the environment](./COMMANDS.md#defaults-and-environment-fallbacks)
- [Accept `--verbose` anywhere](./COMMANDS.md#global-arguments)
- [Make two flags mutually exclusive](./COMMANDS.md#relationships-between-arguments)
- [Return a failure, and an exit code](./COMMANDS.md#failing-well)
- [Customise the help page](./COMMANDS.md#help)
- [Test a CLI](./COMMANDS.md#testing) &middot; [Recipe](./RECIPES.md#test-a-command-without-running-it)

<br>

## Reference

| Document | Contents |
|---|---|
| [`API.md`](./API.md) | Every public item, grouped by job; the SemVer promise; what is *not* guaranteed; the performance table. |
| [docs.rs/cli-forge](https://docs.rs/cli-forge) | Signatures and a runnable example for every item, generated from the source. |
| [`CHANGELOG.md`](../CHANGELOG.md) | What changed, and the [1.x migration table](../CHANGELOG.md#migrating-from-1x). |
| [`release/`](./release) | Release notes, one file per version. [v2.0.0](./release/v2.0.0.md) explains the reasoning behind the current design. |

<br>

## For contributors

| Document | Contents |
|---|---|
| [`dev/DIRECTIVES.md`](../dev/DIRECTIVES.md) | Engineering standards and the definition of done. |
| [`dev/ROADMAP.md`](../dev/ROADMAP.md) | How the crate got here, and what belongs in a sibling crate instead. |
| [`dev/check-api-docs.py`](../dev/check-api-docs.py) | Verifies every public item is mentioned in `API.md`. Needs nightly for rustdoc's JSON output. |

<br>

## A one-minute orientation

Four things exist, and they are worth telling apart before reading anything else:

```rust
use cli_forge::{App, Command, Style, Theme, out, ok};

// 1. `out` and `err` print. That is all they do, and they are cheap.
out("building...");

// 2. A `Style` describes an appearance. It can be used once, or reused forever.
out(Style::new().green().bold().paint("done"));

// 3. A `Theme` says what success, failure, and warning look like — once.
//    After that, nothing in the program names a colour.
Theme::new().install();
ok("compiled 3 targets");

// 4. An `App` holds commands, parses the command line, and runs the right one.
let mut app = App::new("forge");
app.register(Command::new("build").run(|_| out("building...")));
```

Most programs need `out`, a `Theme`, and an `App`. The rest of the surface exists
for when you need it.

<hr>
<br>

<div id="license">
    <h2>License</h2>
    <p>Licensed under either of</p>
    <ul>
        <li><b>Apache License, Version 2.0</b> &mdash; <a href="../LICENSE-APACHE">LICENSE-APACHE</a></li>
        <li><b>MIT License</b> &mdash; <a href="../LICENSE-MIT">LICENSE-MIT</a></li>
    </ul>
    <p>at your option.</p>
</div>

<div align="center">
  <h2></h2>
  <sup>COPYRIGHT <small>&copy;</small> 2026 <strong>James Gober <me@jamesgober.com>.</strong></sup>
</div>
