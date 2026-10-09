<h1 align="center">
    <img width="90px" height="auto" src="https://raw.githubusercontent.com/jamesgober/jamesgober/main/media/icons/hexagon-3.svg" alt="Triple Hexagon">
    <br><b>cli-forge · API</b>
</h1>

<p align="center">
  <code>v2.1.0</code> &mdash; the public surface and the SemVer promise.
</p>

> **Signatures and examples live in the rustdoc**, at
> [docs.rs/cli-forge](https://docs.rs/cli-forge), which is generated from the
> source and cannot drift from it. Every public item carries a runnable example.
>
> This document is the **map and the contract**: what exists, grouped by job, and
> exactly what is guaranteed not to change.

---

## Contents

- [What this crate owns](#what-this-crate-owns)
- [The surface](#the-surface)
  - [Output](#output)
  - [Styling](#styling)
  - [Themes](#themes)
  - [Colour](#colour)
  - [Text measurement](#text-measurement)
  - [Terminal](#terminal)
  - [Commands](#commands)
  - [Arguments](#arguments)
  - [Results](#results)
  - [Errors](#errors)
  - [Auth](#auth)
  - [Introspection](#introspection)
- [Feature flags](#feature-flags)
- [Stability](#stability)
- [Performance notes](#performance-notes)

---

## What this crate owns

Four things, and nothing else:

1. **Output** — one styling system, reached four ways, over one cross-platform
   terminal backend.
2. **Parsing** — a recursive command tree with a full argument model.
3. **Command registration** — from anywhere, hideable, auth-gateable.
4. **Help** — auto-generated and customisable.

It deliberately does **not** own tables, progress bars, gradients, layouts, or
shells. Those are sibling crates that consume the seams listed under
[Introspection](#introspection) and [Text measurement](#text-measurement), so the
core stays small and everything speaks one system.

---

## The surface

### Output

| Item | Job |
|---|---|
| `out(value)` | Print any `Display` to standard output, with a newline. Allocation-free for a `&str`. |
| `err(value)` | The same, to standard error. |
| `write_to(stream, value)` | The accountable form: reports whether the write succeeded. |
| `capture(body)` | Record everything printed while `body` runs, so printing can be tested. |
| `Captured` | `out`, `err`, `combined`, `lines(stream)`, `is_empty`. |

`out` and `err` never parse markup and never allocate for styling. A failed write
is ignored, because the usual cause is a closed pipe (`yourtool | head`) and a
print helper has no way to report failure that the caller could act on.

`capture` is per-thread, so tests that capture run in parallel with tests that
print. While nothing in the process is capturing, the output path checks one
relaxed atomic and proceeds exactly as before. It sees only what went through
this crate — not `println!`, not a direct `std::io::stdout` write, and not a
child process.

### Styling

| Item | Job |
|---|---|
| `Style` | A reusable description of an appearance. |
| `style(text)` | A `Style` carrying text, for a one-off. |
| `Style::new()` | An empty `Style`, for reuse. |
| `Painted<T>` | A value with a `Style` applied, ready to print. |

**Colours** — foreground: `black`, `red`, `green`, `yellow`, `blue`, `magenta`,
`cyan`, `white`, `bright_black`, `bright_red`, `bright_green`, `bright_yellow`,
`bright_blue`, `bright_magenta`, `bright_cyan`, `bright_white`, plus `fg(Color)`,
`hex`, `rgb`, `ansi`.

Background: `on_black`, `on_red`, `on_green`, `on_yellow`, `on_blue`,
`on_magenta`, `on_cyan`, `on_white`, plus `bg(Color)`, `on_hex`, `on_rgb`,
`on_ansi`.

(There are no `on_bright_*` methods; use `bg(Color::BrightRed)`, which is the
same thing without sixteen more names.)

**Attributes** — `bold`, `dim`, `italic`, `underline`, `blink`, `reverse`,
`hidden`, `strike`.

**Decoration** — `prefix`, `suffix`, `pad_to`, `align`, `link`.

**Composition** — `merge`, `is_plain`.

**Rendering** — `paint`, `paint_at`, `render`, `render_at`, `render_for`.

`paint` is the reuse path and allocates nothing; `pad_to` and `render` are the
two documented exceptions, since one must measure the finished text and the other
returns an owned `String` by definition.

**Markup** — `markup(tags)` and `markup_at(tags, level)` render a string with
inline tags to a styled `String`. Grammar: `<b>` `<d>` `<i>` `<u>` `<s>` `<r>`,
`<c=VALUE>`, `<bg=VALUE>`, `<link=URL>`, `</x>`, `</>`, and `<<` for a literal
`<`. Unrecognised markup prints literally; nesting is capped at 64 frames so that
markup from an untrusted source cannot grow the parser's state without bound.

**Named styles** — `define(name, style)`, `named(name) -> Style`,
`defined(name) -> bool`. The store is process-global, so a name defined in one
module resolves in another; a later `define` of the same name replaces the
earlier one, and library code should prefix its names.

### Themes

| Item | Job |
|---|---|
| `Level` | `Success`, `Error`, `Warning`, `Info`, `Hint`, `Note`, `Debug`, `Trace`, plus `ALL`, `stream()`, `name()`. |
| `Glyphs` | `Auto`, `Unicode`, `Ascii`, `None`. |
| `Theme` | `new`, `plain`, `set`, `set_style`, `set_stream`, `set_glyphs`, `style`, `stream`, `glyph`, `render`, `render_at`, `install`, `current`. |
| `ok` `fail` `warn` `info` `hint` `note` `debug` `trace` | Print through the process theme at that level, to that level's stream. |

Errors, warnings, and the two diagnostic levels go to standard error by default;
everything else to standard output, so a program's data stays pipeable without
the program arranging it. Under `Glyphs::Auto`, markers fall back to one-column
ASCII where the destination cannot render Unicode, so a column of status lines
stays aligned either way.

### Colour

`Color` — the sixteen terminal colours (`Black` … `BrightWhite`), `Ansi(u8)` for a
256-palette index, and `Rgb(u8, u8, u8)` for an exact value. `parse`, `from_hex`,
`to_rgb`. Marked `#[non_exhaustive]`.

`parse` accepts a name (case- and separator-insensitive, so `BrightRed`,
`bright_red`, and `bright red` all resolve), `#rrggbb`, the `#rgb` shorthand,
`r,g,b`, and a bare `0..=255` index.

Degradation is by capability tier, and a colour is never dropped where it could be
approximated:

| Colour | True colour | 256 colour | 16 colour | None |
|---|---|---|---|---|
| `Rgb` | exact | nearest cube or greyscale entry | nearest of 16 | dropped |
| `Ansi` | exact | exact | nearest of 16 | dropped |
| named | exact | exact | exact | dropped |

### Text measurement

`text::width`, `strip`, `pad`, `truncate`, `wrap`, `sanitize`, `Align`.

All measured in **display columns**, not bytes, skipping escape sequences. This is
the seam every sibling crate that aligns anything depends on; a second
implementation of it is a defect. `sanitize` neutralises the control characters
that let untrusted text take over a terminal.

### Terminal

`ColorLevel` (`None` < `Ansi16` < `Ansi256` < `TrueColor`, plus `is_none()`),
`Stream` (`Stdout`, `Stderr`), `ColorChoice` (`Auto`, `Always`, `Never`).

`terminal::level`, `set_color_choice`, `color_choice`, `set_level`, `clear_level`,
`invalidate`, `size`, `width_or`, `supports_unicode`.

Capability is detected **per stream** and cached in a relaxed atomic, so the hot
path is one integer load and an override still takes effect. Precedence, highest
first: `set_level`; `set_color_choice`; `CLICOLOR_FORCE` / `FORCE_COLOR`;
`NO_COLOR` / `CLICOLOR=0`; `TERM=dumb` or not a terminal.

### Commands

`App` — `new`, `version`, `about`, `long_about`, `help_header`, `help_footer`,
`arg`, `args`, `command`, `register`, `help_command`, `theme`, `color`,
`external`, `suggest`, `auth`, `help`, `command_help`, `try_parse_from`,
`try_run_from`, `dispatch`, `run`, `parse`.

`Command` — `new`, `alias`, `aliases`, `about`, `long_about`, `before_help`,
`after_help`, `usage`, `arg`, `args`, `group`, `subcommand`,
`subcommand_required`, `hidden`, `display_order`, `category`, `requires_auth`,
`run`, `run_status`.

`External` *(2.1)* — what the hook set with `App::external` receives for a
command the app does not define: `name`, `args` (the untouched tokens after it),
`matches` (the app-level arguments parsed before it), and `suggestion` (the
nearest registered command, for a hook that finds no matching program). Marked
`#[non_exhaustive]`. A flag is never handed off; only a bare name is.

**Entry points.** Four, because programs want different things:

| Method | Parses | Runs handlers | Prints | Exits |
|---|---|---|---|---|
| `run` | process args | yes | yes | returns an `ExitCode` |
| `parse` | process args | yes | yes | yes |
| `try_run_from` | given args | yes | no | no |
| `try_parse_from` | given args | no | no | no |

`try_parse_from` has no side effects, which is what makes it the one to test with.

### Arguments

`Arg` — constructors `flag`, `count`, `option`, `positional`; then `short`,
`long`, `help`, `value_name`, `required`, `required_unless`, `conflicts_with`,
`requires`, `multiple`, `value_delimiter`, `negatable`, `default`, `env`,
`possible_values`, `validate`, `hide`, `global`, `category`.

`ArgGroup` *(2.1)* — a rule over a set of arguments: `new`, `arg`, `args`,
`required`, `multiple`, plus `name`, `members`, `is_required`, `is_multiple`.
At most one member by default; `required` makes it exactly one; `required` +
`multiple` is at least one. A default never counts against the at-most-one
rule, and an explicit `--no-NAME` never counts as an answer.

`value_delimiter(',')` *(2.1)* splits each value, so `--features a,b` is two
values; each piece is validated on its own and empty pieces are kept.
`negatable(true)` *(2.1)* accepts `--no-NAME` as an explicit off; the last
spelling wins, and an explicit off never takes part in a conflict.

Parsed forms: `--long`, `--long=value`, `--long value`, `-s`, `-s value`,
`-svalue`, `-s=value`, bundled `-abc`, counting `-vvv`, repeatable options,
variadic positionals, and `--`. A negative number (`-5`) and a lone `-` are
values, not flags.

Precedence for a value: command line, then environment, then default.

### Results

`Matches` — `flag`, `explicit_flag`, `count`, `present`, `value`, `values`,
`source`, `get`, `try_get`, `get_all`, `group`, `external`, `subcommand`,
`subcommand_name`, `command_path`, `leaf`.

`explicit_flag` answers `Some(true)`, `Some(false)` (a negatable flag turned
off), or `None` (never mentioned) — the distinction `flag` collapses. `group`
names the member that answered an `ArgGroup`. `external` is the name and raw
arguments of an external subcommand, set only when `App::external` is.

`ValueSource` — `CommandLine`, `Environment`, `Default`.

Because values are validated at parse time, `get::<T>` reads them back
infallibly; `try_get` reports why an unvalidated value would not parse.

### Errors

`ParseError` — `kind`, `subject`, `detail`, `suggestion`, `usage`, `text`,
`is_request`, `exit_code`, `stream`, `report`, `styled_report`.

`ErrorKind` — `UnknownFlag`, `MissingValue`, `MissingRequired`, `UnknownCommand`,
`UnexpectedArgument`, `InvalidValue`, `Conflict`, `MissingDependency`,
`MissingSubcommand`, `NonUtf8`, `Unauthorized`, `HelpRequested`,
`VersionRequested`, plus `is_request()`. Marked `#[non_exhaustive]`.

`CommandError` — `new`, `with_code`, `message`, `exit_code`.

A broken `ArgGroup` reports through the existing kinds: `MissingRequired` with
the group's name as the subject when nothing answered it, `Conflict` when two
members were given to a group that allows one.

`Outcome` — what a `run` handler may return: `()`, or any `Result<(), E>` where
`E: Display`.

Help and version arrive through the same channel as errors but are **successes**:
`exit_code()` is `0` and `stream()` is standard output, so `--help` can be piped.
A bad command line exits `2`; a failed command exits `1` or its own status.

### Auth

*(feature `auth`)* `App::auth(hook)` and `AuthRequest` (`command`, `path`).
Marked `#[non_exhaustive]`.

The seam **fails closed**: an auth-gated command with no hook is never
authorized, and never appears in help. The hook also runs during help generation,
so it must be pure and cheap — check already-loaded session state, not I/O.

### Introspection

Read-only accessors, so a sibling crate can generate shell completions, manual
pages, or documentation from the **live** command tree instead of a second
hand-maintained description of the same CLI.

| Type | Accessors |
|---|---|
| `App` | `name`, `version_text`, `commands`, `global_arguments` |
| `Command` | `name`, `alias_names`, `about_text`, `arguments`, `groups`, `subcommands`, `category_name`, `is_hidden`, `is_auth_gated` |
| `Arg` | `name`, `short_form`, `long_form`, `help_text`, `default_value`, `env_var`, `allowed_values`, `delimiter`, `category_name`, `is_required`, `is_multiple`, `is_negatable`, `is_hidden`, `is_global`, `expects_value`, `is_positional` |
| `ArgGroup` | `name`, `members`, `is_required`, `is_multiple` |

---

## Feature flags

| Feature | Default | Description |
|---------|---------|-------------|
| `std` | yes | Terminal detection, the stdout/stderr writers, and the command layer. Without it the styling core still works on `alloc`. |
| `color` | yes | ANSI / styled output. Disable for plain output; the API stays complete and every styled value renders as its plain text. |
| `unicode` | yes | Correct display widths for CJK, emoji, and combining marks. Without it widths are counted in characters. |
| `termsize` | yes | Query the terminal for its size. Without it only `COLUMNS`/`LINES` are consulted. |
| `auth` | no | The auth seam. Adds no dependencies. |

`--no-default-features` is a real `no_std` build of the styling core — colour,
style, text, markup, themes — on `alloc` alone, for a sink whose capability the
caller declares with `terminal::set_level`.

**Dependencies.** With the defaults, two direct ones, both narrow:
`unicode-width` (data tables, no unsafe, no dependencies of its own) and
`terminal_size` (which needs FFI, and is isolated there because this crate's root
is `#![forbid(unsafe_code)]`). On Windows the `color` feature adds
`enable-ansi-support`, for the same reason. Turning `unicode` and `termsize` off
leaves a crate with **no dependencies at all**:

```
cli-forge                      # --no-default-features --features std,color
├── terminal_size              # feature: termsize
│   └── rustix                 #   (windows-sys on Windows)
├── unicode-width              # feature: unicode
└── enable-ansi-support        # feature: color, Windows only
    └── windows-sys
```

---

## Stability

**The 2.0 surface is stable.** Everything listed above is guaranteed under
[Semantic Versioning](https://semver.org/):

- **No breaking change before 3.0.** Every public item's signature and documented
  behaviour is guaranteed, including the feature flags, the `auth` seam, and the
  read-only accessors.
- **Additions are minor.** New public API arrives only in `2.x` releases and is
  strictly additive. `Color`, `ErrorKind`, and `AuthRequest` are
  `#[non_exhaustive]`, so new variants and new context can be added without a
  major bump — which means **do not match them exhaustively**.
- **Fixes and internal optimisation are patch.** This includes making something
  faster, provided the observable behaviour is unchanged.
- **The MSRV (1.85) rises only in a minor release**, never in a patch.

What is explicitly *not* guaranteed:

- **The exact bytes of help output.** Column widths, wrapping, and section order
  are presentation and may change in a minor release. Assert on content, not on
  layout.
- **The exact wording of an error message.** Match on `ErrorKind` and read
  `subject()`; those are stable, and the prose is not.
- **Which suggestion is offered** for a near miss, or whether one is offered at
  all. The threshold may be tuned.
- **The approximation chosen when a colour is downgraded.** The distance metric
  may improve; that a colour degrades rather than being dropped is the guarantee.

Migrating from `1.x` is a handful of renames; see the
[migration table](../CHANGELOG.md#migrating-from-1x).

---

## Performance notes

Criterion means, Windows x86_64, release. A range means the figure moved that
much between runs of the same code on the same machine; treat anything inside it
as the same number:

| Operation | ns/op |
|-----------|------:|
| `out` plain write (`&str`) | ~9.5 |
| `paint`, no styling | ~37 |
| `paint`, named colour + bold | ~103 |
| `paint`, 24-bit foreground and background | ~184 |
| `paint`, 24-bit downgraded to 16 colours | ~138 |
| `markup`, four styled runs | ~234 |
| themed line | ~167 |
| named-style lookup then paint | ~176 |
| the same with the lookup hoisted | ~149 |
| `text::width`, styled | ~59 |
| `text::strip`, plain (borrows) | ~12 |
| `out`, through a capture | ~25–40 |
| a themed line, through a capture | ~160–210 |
| command parse, minimal invocation | ~730–970 |
| command parse, rich invocation | ~2100–2500 |
| help render | ~3599 |

The invariants behind those numbers:

- **The plain path does no styling work.** `out` with a `&str` is a near-direct
  write and allocates nothing, which `tests/allocation.rs` asserts with a
  counting global allocator rather than by claim.
- **Styling is paid for only when asked for.** A `Style` with nothing set takes
  the plain path even at full colour depth, so a program its user ran with
  `NO_COLOR` pays essentially nothing for the styling it asked for.
- **A capability question is one atomic load** after the first, so rendering never
  re-reads the environment.
- **Reuse is cheaper than rebuilding.** Resolve a named style once and paint many
  times; the benchmark pair above is the difference.

Reproduce with `cargo bench --bench bench`.
