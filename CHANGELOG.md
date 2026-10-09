<h1 align="center">
    <img width="90px" height="auto" src="https://raw.githubusercontent.com/jamesgober/jamesgober/main/media/icons/hexagon-3.svg" alt="Triple Hexagon">
    <br><b>CHANGELOG</b>
</h1>
<p>
  All notable changes to <code>cli-forge</code> will be documented in this file. The format is based on <a href="https://keepachangelog.com/en/1.1.0/">Keep a Changelog</a>,
  and this project adheres to <a href="https://semver.org/spec/v2.0.0.html/">Semantic Versioning</a>.
</p>

---

## [Unreleased]

### Added

### Changed

### Fixed

### Security

---

## [2.1.0] - 2026-10-09

Strictly additive. One new capability, and one performance fix.

### Added

**Output capture, so a program's printing can be tested.** Testing what a CLI
prints normally means spawning the binary and reading its pipes, which is slow,
awkward, and blind to anything a library printed on the program's behalf.
`capture` redirects this crate's output paths into a buffer for the duration of a
closure instead:

```rust
let (outcome, log) = capture(|| app.try_run_from(["build"]));

assert_eq!(log.lines(Stream::Stdout), ["building...", "✓ compiled 3 targets"]);
assert!(log.err().contains("not optimised"));
```

That asserts the stream discipline a `Theme` is responsible for — the warning went
to standard error and the success did not — which nothing else can observe.

- `capture(body) -> (T, Captured)`, returning whatever the body returned.
- `Captured::{out, err, combined, lines, is_empty}`. `combined` interleaves both
  streams in the order the lines were written; `lines` drops the trailing
  newlines, which makes a failing assertion far more readable.
- Per-thread, so tests that capture run in parallel with tests that print.
  Nesting works: the inner capture takes over and the outer resumes.
  Panic-safe: a panic inside the closure leaves the thread un-redirected.
- Captures `out`, `err`, `write_to`, the themed printers, and the reports
  `App::run` / `App::parse` print. Not `println!`, not a direct
  `std::io::stdout` write, and not a child process.

### Changed

While no capture is active anywhere in the process, the output path checks one
relaxed atomic and proceeds exactly as before. Measured through a capture, `out`
costs ~41 ns per line and a themed line ~210 ns; the real path locks standard
output and flushes a `LineWriter` on the newline, so the added load is not
measurable against it.

### Fixed

**`Cli::inherited` deep-copied every global argument on every command level.**
`parse_command` recurses, so a three-level invocation paid for three full copies
of every argument marked `global`, each carrying six strings and three vectors.
It now borrows, and the set is built once per parse.

Two smaller allocations on the happy path went with it: every long and short flag
formatted its own display form (`"--jobs"`) up front purely so a validation error
could name it, and `bump_count` reached straight for `entry` — which needs an
owned key — so `-vvv` allocated the argument name once per repeat.

| Benchmark | 2.0.0 | 2.1.0 | |
|---|---|---|---|
| `parse_simple` | 1.34 µs | 1.22 µs | −8.9% |
| `parse_rich` | 2.91 µs | 2.70 µs | −6.6% |

The remaining gap against 1.x is hash-map traffic rather than allocation: roughly
34 short-string hashes per parse, because `finalize` and the relationship check
probe both the command's own slot and the global one for every declared
argument. Closing it would mean ordered vectors and linear search, which is
faster at the sizes a command actually has — but parsing happens once per
process, three orders of magnitude below process startup, so it is not worth the
churn.

---

## [2.0.0] - 2026-10-08

The output layer becomes **themed and reusable**, the command layer gains the
argument model a real CLI needs, and the styling core becomes a seam sibling
crates can build on. Four confirmed 1.x defects are fixed, each of which made a
legitimate command line unparseable.

This is a breaking release. `docs/API.md` froze the 1.x surface "until 2.0";
this is that. See [Migrating from 1.x](#migrating-from-1x) below — most programs
need a handful of renames.

### Added

**Themed responses.** The headline: a `Theme` maps each of the eight `Level`s a
CLI speaks in (`Success`, `Error`, `Warning`, `Info`, `Hint`, `Note`, `Debug`,
`Trace`) onto a style, a glyph, and a stream. The free functions `ok`, `fail`,
`warn`, `info`, `hint`, `note`, `debug`, and `trace` print through it, so no
call site names a colour and the whole program restyles in one place.

- `Theme::new`, `Theme::plain`, `Theme::set`, `Theme::set_style`,
  `Theme::set_stream`, `Theme::set_glyphs`, `Theme::install`, `Theme::current`,
  `Theme::render`, `Theme::render_at`.
- `Glyphs::{Auto, Unicode, Ascii, None}`. Under `Auto`, `✓` becomes `+` where
  the destination cannot render it; every ASCII stand-in is one column wide, so
  a column of status lines stays aligned either way.
- Diagnostics go to standard error and data to standard output by default, so a
  program's output stays pipeable without the program arranging it.

**Styles carry their decoration.** A `Style` is now a reusable value describing a
whole appearance, not just colours — which is what removes the hand-built
markers 1.x left to each call site.

- `Style::new` (text-less, for reuse) and `Style::paint` / `Style::paint_at`,
  which apply one style to any number of values with no allocation.
- `Style::prefix`, `Style::suffix`, `Style::pad_to`, `Style::align`,
  `Style::link` (OSC 8 hyperlinks), `Style::merge`, `Style::is_plain`.
- `Painted<T>`, the printable result of `paint`.
- Padding is measured in **display columns**, so a column stays straight whether
  the content is ASCII, accented, CJK, or already styled.

**The full colour model.** `Color` is now public and complete.

- All sixteen terminal colours including the bright half (`bright_red`, …), the
  256-colour palette (`Style::ansi`, `Color::Ansi`), and exact 24-bit values.
- Backgrounds throughout: `on_red`, `on_hex`, `on_rgb`, `on_ansi`, `Style::bg`.
- Eight attributes: `bold`, `dim`, `italic`, `underline`, `blink`, `reverse`,
  `hidden`, `strike`.
- `Color::parse` accepts names (case- and separator-insensitive, so `BrightRed`,
  `bright_red`, and `bright red` all work), `#rrggbb`, the `#rgb` shorthand,
  `r,g,b`, and a bare palette index. `Color::to_rgb`, `Color::from_hex`.
- Exact colours degrade through the palette (now including the greyscale ramp)
  to the nearest of the sixteen, using weighted-luminance distance rather than
  plain Euclidean — which stopped mid greens resolving to black.

**A `text` module: the measurement seam.** Everything that lines things up needs
one correct answer to "how many columns is this?", and a table or progress crate
cannot get it from `str::len`.

- `text::width`, `text::strip`, `text::pad`, `text::truncate`, `text::wrap`,
  `text::sanitize`, `text::Align`.
- `text::sanitize` neutralises terminal escape injection from untrusted input —
  a filename, a commit message, a server response — which 1.x wrote verbatim.

**Terminal control.**

- `ColorChoice::{Auto, Always, Never}` with `terminal::set_color_choice`, which
  is what a `--color` flag drives; `terminal::set_level` / `clear_level` for a
  destination whose capability is known from outside.
- `Stream::{Stdout, Stderr}`, detected **independently**.
- `terminal::size`, `terminal::width_or`, `terminal::supports_unicode`.
- `ColorLevel` is public and ordered by capability.

**The argument model.**

- `App::arg` / `App::args` for app-level arguments, and `Arg::global` to make one
  usable on either side of the command name, recorded once and visible at every
  level.
- `Arg::value_name`, `Arg::env`, `Arg::possible_values`, `Arg::validate`,
  `Arg::conflicts_with`, `Arg::requires`, `Arg::required_unless`, `Arg::hide`.
- `Matches::get::<T>`, `try_get::<T>`, `get_all::<T>`, `present`, `source`,
  `subcommand_name`, `command_path`, `leaf`.
- `ValueSource::{CommandLine, Environment, Default}`, so a program can tell "the
  user chose this" from "nobody said".

**Errors that answer "what now?"** A `ParseError` carries the subject, what
would have been valid, the nearest spelling to what was typed, and the usage
line of the command being invoked.

- `ErrorKind` (fifteen variants), `ParseError::{kind, subject, detail,
  suggestion, usage, text, is_request, exit_code, stream, report,
  styled_report}`.
- Did-you-mean suggestions for commands, subcommands, flags, and the members of
  a `possible_values` set.

**Dispatch and exit codes.**

- `App::run` returns an `ExitCode` for `fn main() -> ExitCode`; `App::try_run_from`
  and `App::dispatch` for tests and for programs that dispatch themselves.
- Handlers may return any printable `Result`, so `?` works inside a command.
  `Command::run_status` carries an exact exit status.
- `CommandError`, `Outcome`.
- Help and version exit `0`; a bad command line exits `2`; a failed command
  exits `1` or its own status.

**Help.**

- `App::about`, `App::long_about`, `App::command`, `App::help_command`,
  `App::theme`, `App::color`, `App::command_help`.
- `Command::long_about`, `before_help`, `after_help`, `usage`, `display_order`,
  `subcommand_required`, `args`.
- An automatic `help [command]` subcommand; a bare invocation shows the page
  rather than exiting silently; descriptions wrap to the terminal width; an
  argument's default, environment variable, and allowed values are stated.

**Introspection, for the sibling crates.** `App::{name, version_text, commands,
global_arguments}`, `Command::{name, alias_names, about_text, arguments,
subcommands, is_hidden, is_auth_gated}`, and `Arg::{name, short_form, long_form,
help_text, default_value, env_var, allowed_values, is_required, is_multiple,
is_hidden, is_global, expects_value, is_positional}`. A completions or
manual-page generator can now read the live command tree instead of being handed
a second description of the same CLI.

**Markup.** `<d>` dim, `<i>` italic, `<s>` strike, `<r>` reverse, `<bg=…>`
backgrounds, `<link=…>` hyperlinks, and `<<` for a literal `<`. `markup_at` for
an explicit depth.

**Prose documentation**, indexed at [`docs/`](./docs/README.md): a
[Guide](./docs/GUIDE.md), [Output & Styling](./docs/OUTPUT.md),
[Commands & Arguments](./docs/COMMANDS.md), and [Recipes](./docs/RECIPES.md).
Every code block in them — and in this README — is compiled and run by
`cargo test`, so none of it can go stale.

### Changed

- **`--no-default-features` is now a real `no_std` build** of the styling core
  (colour, style, text, markup, themes) on `alloc` alone, rather than an empty
  crate whose own test suite would not compile.
- New default features `unicode` and `termsize`, for correct display widths and
  terminal-aware wrapping. Both can be turned off; the fallbacks are documented.
- `ParseError` is a struct with an `ErrorKind` rather than an enum with payloads,
  and is boxed internally so `Result<Matches, ParseError>` stays small.
- `panic = "abort"` removed from the release profile: in a library's own profile
  it only affected this crate as a root, while breaking `cargo bench` and any
  consumer's use of `catch_unwind`.

### Fixed

- **Negative numbers were unparseable.** `calc add -5 3` reported
  `UnknownFlag { flag: "-5" }`. A leading `-` is now only treated as a flag when
  it could be one, so negative numbers — and `-`, the read-standard-input
  convention — are values. A command that really declares `-5` still gets it.
- **There were no app-level arguments at all.** `demo --verbose build` reported
  `UnknownFlag`.
- **A parent's required positionals were demanded even when a subcommand took
  over.** `remote list` reported `MissingRequired { arg: "name" }`, which made
  every grouping command with its own positionals unusable.
- **`App::parse()` panicked on an argument that was not valid UTF-8**, because
  `std::env::args` does. Process arguments are now read as `OsString` and
  reported, never panicked on.
- **Help columns were aligned by byte length**, so a command named `ünïcödé` or
  `日本語` skewed every row. Alignment is now measured in display columns.
- **Standard error's styling was decided by standard output's capability**, so a
  program with one stream redirected got one of the two wrong.
- The colour level was cached in a `OnceLock`, making the first observation
  permanent — which is also what made styled output untestable.
- Markup nesting was unbounded on hostile input; it is now capped, with tags past
  the cap kept as the literal text they look like rather than dropped.
- A stray escape byte corrupted the sequence after it during measurement and
  truncation.

### Security

- `text::sanitize` neutralises the control characters that let untrusted text
  take over a terminal: clearing the screen, repositioning the cursor to
  overwrite earlier output, relabelling the window, or changing colours
  permanently. 1.x wrote such text verbatim.
- Markup nesting is capped at 64 frames, so markup from an untrusted source
  cannot grow the parser's state without bound.
- `Arg::env` lets a secret stay out of the process list, where `--token` is
  visible to every user on the machine.
- The `auth` seam still fails closed: an auth-gated command with no hook is never
  authorized, and never appears in help.

### Performance

Measured with `criterion`; the styling optimisation replaced `core::fmt`'s
integer formatter with tables for the fixed set of SGR codes.

| Benchmark | 1.x | 2.0 | |
|---|---|---|---|
| `paint_named_colour` | 160 ns | 103 ns | −36% |
| `paint_exact_colour` | 316 ns | 184 ns | −40% |
| `paint_downgraded_to_16` | 201 ns | 138 ns | −31% |
| `markup_rich` | 409 ns | 234 ns | −42% |
| `theme_render` | 246 ns | 167 ns | −30% |

The plain path (`out` / `err`) is byte-for-byte unchanged at ~9.5 ns and remains
allocation-free, which `tests/allocation.rs` asserts by measurement.

### Migrating from 1.x

| 1.x | 2.0 | Why |
|---|---|---|
| `parse("<c=red>x</c>")` | `out(markup("<c=red>x</c>"))` | `markup` returns a `String`, so it composes — into a file, a table cell, or `text::width`. The old name also collided with argument parsing. |
| `define_tag("e", style("").red())` | `define("e", Style::new().red())` | "tag" meant both a markup tag and a named style. |
| `tag("e").render_with("msg")` | `named("e").paint("msg")` | Returns a `Style`, so decoration travels with the name and reuse costs no allocation. |
| `ParseError::UnknownFlag { flag }` | `err.kind() == ErrorKind::UnknownFlag`, `err.subject()` | The error now also carries a suggestion and a usage line. |
| `app.try_parse_from(..)` ran handlers | `app.try_run_from(..)` | `try_parse_from` now only parses. **This is the one silent behaviour change**: a program relying on parsing to dispatch will stop running handlers. `App::parse` and `App::run` still dispatch. |
| `Command::run(\|m\| { .. })` | unchanged | Handlers may now also return a `Result`. |
| `App::parse() -> Matches` | `App::run() -> ExitCode` | `parse` still works; `run` hands the exit decision to `main`. |

Two behaviours changed without a rename, both deliberate: a bare invocation now
shows the help instead of doing nothing (`App::help_command(false)` restores the
old behaviour), and `prog help [command]` is accepted.

---

## [1.0.0] - 2026-07-01

The stable API freeze. No new public API — the surface built across the 0.x series
is now guaranteed under Semantic Versioning: no breaking change before 2.0.

### Added

- Command-parse benchmarks (`parse_simple`, `parse_rich`) rounding out the
  benchmark suite alongside the output-layer benches (internal; not public API).

### Changed

- Public surface **declared stable**. `docs/API.md` records the SemVer promise
  (breaking changes require a MAJOR bump; additions are minor; fixes/optimization
  are patch; the 1.85 MSRV rises only in minors).
- README and API.md status updated from "pre-1.0 / frozen" to "1.0 — stable".
- Doc comments that referenced upcoming milestones (`reserved for v0.5.0`, "surface
  once the auth seam lands") corrected to describe the shipped behavior.

---

## [0.6.0] - 2026-07-01

Two small, common argument conveniences — counting flags and multiple values —
added within the frozen surface (strictly additive; nothing existing changed).

### Added

- `Arg::count(name)`: a repeatable flag whose occurrences are tallied
  (`-v`/`-vv`/`-vvv`, `-v -v -v`, `--verbose --verbose`). Read with
  `Matches::count(name) -> usize`.
- `Arg::multiple(bool)`: collect every occurrence of an option into a list, or
  make a positional variadic (absorbing the remaining bare values). Read with
  `Matches::values(name) -> impl Iterator<Item = &str>`.
- `Matches::count` and `Matches::values` accessors. `Matches::flag` now also
  reports `true` for a counting flag once its count reaches one.
- `examples/arguments.rs`: every argument kind in one command.

### Changed

- Counts saturate rather than overflowing on pathological repeated input.
- `docs/API.md` documents the two new argument kinds with parameter tables and
  examples; the Stability section notes the additive step.

---

## [0.5.0] - 2026-07-01

The auth seam, and the public surface declared frozen ahead of 1.0.

### Added

- `App::auth(hook)` (feature `auth`): the authorization seam. The hook —
  `Fn(&AuthRequest) -> bool`, supplied by the consumer — decides whether an
  auth-gated command may run. cli-forge holds the seam; the login/logout logic
  lives in the consumer or a sibling crate.
- `AuthRequest` (feature `auth`): the `#[non_exhaustive]` context passed to the
  hook, naming the command being authorized (`command()`, `path()`).
- `ParseError::Unauthorized { command }`: returned when an auth-gated command is
  invoked without authorization. The handler does not run.
- `examples/auth.rs`: an auth-gated command that runs only when authorized.

### Changed

- `Command::requires_auth` is now enforced with the `auth` feature: an auth-gated
  command runs — and appears in help — only when the hook authorizes it, and
  fails closed when no hook is set. Without the `auth` feature the flag is inert
  (the command runs and shows normally).
- The `auth` feature now enables the seam (was a reserved no-op) and implies `std`.
- `docs/API.md` documents the auth seam and **declares the public surface frozen**:
  the remaining 0.x releases add tests, docs, and optimization only, and 0.5.0's
  surface becomes the 1.0 contract.

### Security

- Auth-gated commands fail closed: with the `auth` feature and no hook set, they
  are never authorized (neither run nor listed in help).

---

## [0.4.0] - 2026-06-30

The help engine, plus the small conveniences a base CLI is expected to have:
command aliases, `--help`/`-h`, and `--version`/`-V`.

### Added

- Auto-generated help rendered through the output layer: styled section headers,
  aligned columns, a usage line, and command/argument/option listings. The
  injectable `App::help_header` / `App::help_footer` wrap every page.
- `App::help() -> String` renders the top-level help on demand (for a no-command
  fallback, a `help` command, etc.).
- `App::version(...)` and the `-V` / `--version` flags, printed to standard output
  with exit `0`.
- `-h` / `--help` at any command level renders that level's help (top-level or a
  specific command), to standard output with exit `0`. A command may override the
  built-in by declaring its own `help` / `h` argument.
- `Command::alias(...)` / `Command::aliases(...)`: alternative invocation names.
  Aliases resolve to the canonical command (the parsed subcommand name stays
  canonical) and are shown alongside the name in help.
- `ParseError::HelpRequested(String)` and `ParseError::VersionRequested(String)`
  control signals (carrying the rendered text) so the exiting `parse` and the
  non-exiting `try_parse_from` share one path.

### Changed

- Hidden and auth-gated commands are omitted from generated help listings
  (auth-gated commands surface once the auth seam lands in v0.5.0).
- `docs/API.md` documents the help engine, aliases, version, and the new
  `ParseError` signals.

---

## [0.3.0] - 2026-06-30

The command layer: a recursive command tree, runtime registration from anywhere,
and arg/flag parsing with structured, non-panicking errors.

### Added

- `App`: the command registry and entry point — `new`, `register` (callable from
  any module), `help_header` / `help_footer` (stored for the v0.4.0 help engine),
  `parse` (env args; prints a structured error and exits `2` on malformed input),
  and `try_parse_from` (non-exiting, testable/embeddable).
- `Command`: the recursive tree node — `new`, `about`, `arg`, `subcommand`,
  `hidden`, `requires_auth` (flag stored; enforced with the auth seam in v0.5.0),
  and `run` for the handler.
- `Arg`: `flag` / `option` / `positional` constructors with `short`, `long`,
  `help`, `required`, and `default`.
- `Matches`: `flag`, `value`, and `subcommand` accessors, passed to handlers.
- `ParseError`: a `#[non_exhaustive]` structured error
  (`UnknownFlag`, `MissingValue`, `MissingRequired`, `UnknownCommand`,
  `UnexpectedArgument`) implementing `Display` and `std::error::Error`.
- Parser handling the standard forms: `--long`, `--long=value`, `--long value`,
  `-s`, `-s value`, `-svalue`, bundled short flags `-abc`, positionals, and the
  `--` end-of-options marker. Selected command's handler dispatched on parse.
- `tests/registration.rs`: a command registered from a non-`main` module is
  reachable and behaves identically (the predecessor's limitation, now tested).
- `examples/commands.rs`: a subcommand CLI with flags, options, positionals, and
  the structured-error exit path.
- `proptest` fuzzing of the parser (arbitrary argument vectors never panic).

### Changed

- `docs/API.md` now documents the implemented command surface (`App`, `Command`,
  `Arg`, `Matches`, `ParseError`) with parameter tables and examples.

---

## [0.2.5] - 2026-06-30

The output layer — the load-bearing piece every sibling crate depends on. Three
styling paths over one cross-platform terminal backend, with the plain path proven
allocation-free by test rather than by claim. This is the first substantive release
under the `cli-forge` name, following the 0.2.0 name claim.

### Added

- `out` / `err`: the plain output path. Line-oriented, no tag parsing, and
  allocation-free for a string literal &mdash; proven by a counting-allocator test
  (`tests/allocation.rs`).
- `style` builder: chainable styling (`Style`) with the eight standard named
  colors, 24-bit `hex` / `rgb`, `bold`, `underline`, `render`, and `Display`.
- `parse`: inline tag styling &mdash; `<b>`, `<u>`, `<c=VALUE>` (named / `#rrggbb`
  / `r,g,b`), and `</>`. Nesting, graceful pass-through of unrecognized markup.
- `define_tag` / `tag` / `Tag`: a named-style registry &mdash; define a style once,
  recall it anywhere by name.
- A single terminal backend resolving color depth once (true-color / 256 / 16 /
  none) from `NO_COLOR`, `CLICOLOR_FORCE`, `TERM`, `COLORTERM`, and TTY detection,
  with automatic Windows virtual-terminal enablement and a plain-text fall-back.
- 24-bit colors degrade to the nearest 256- or 16-color value on terminals that
  cannot render them.
- Cross-path byte-identical rendering: the builder, tags, and registry produce the
  same bytes for the same intent (verified across all color levels).
- Runnable examples: `quick_start`, `three_paths`, `colors`, `status_report`.
- Criterion benchmarks for the plain and styled render paths; property tests
  (`proptest`) over the parser and color downgrades.
- `docs/API.md` rewritten to document the implemented surface with parameters and
  multiple examples per item.

### Changed

- `Cargo.toml` features now match the documented surface: `std`, `color` (default,
  implies `std`), and a reserved `auth`. The undocumented `serde` feature/dependency
  was removed (YAGNI).
- Added `rust-toolchain.toml` pinning the development channel; the CI matrix
  overrides it per-job via `RUSTUP_TOOLCHAIN` so the 1.85 MSRV is still exercised.

### Fixed

- `clippy.toml` MSRV corrected from `1.87` to the crate's `1.85`.
- `deny.toml` header comment corrected (`rate-net` &rarr; `cli-forge`).
- `Cargo.lock` is now committed (removed from `.gitignore`) for reproducible
  builds, as REPS requires.

---

## [0.2.0] - 2026-06-30

Name claim. The crate's original name was unavailable on crates.io, so the project
was renamed to `cli-forge` and this version was published to secure the name. It
carries the 0.1.0 structure forward under the new name; the output layer ships in
0.2.5.

### Changed

- Crate renamed to `cli-forge` (crate name, library path `cli_forge`, repository
  and documentation links).

---

## [0.1.0] - 2026-06-30

Initial scaffold and repository bootstrap. No domain logic yet &mdash; this release establishes the structure, tooling, and quality gates the implementation will be built on.

### Added

- `Cargo.toml` with crate metadata, Rust 2024 edition, MSRV 1.85.
- Dual `Apache-2.0 OR MIT` license files.
- `README.md`, `CHANGELOG.md`, and a documentation skeleton.
- `REPS.md` compliance baseline.
- `.github/workflows/ci.yml` CI matrix; `deny.toml`, `clippy.toml`, `rustfmt.toml`.
- `dev/DIRECTIVES.md` and `dev/ROADMAP.md` (committed engineering standards + plan).

[Unreleased]: https://github.com/jamesgober/cli-forge/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/jamesgober/cli-forge/compare/v0.6.0...v1.0.0
[0.6.0]: https://github.com/jamesgober/cli-forge/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/jamesgober/cli-forge/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/jamesgober/cli-forge/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/jamesgober/cli-forge/compare/v0.2.5...v0.3.0
[0.2.5]: https://github.com/jamesgober/cli-forge/compare/v0.2.0...v0.2.5
[0.2.0]: https://github.com/jamesgober/cli-forge/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/jamesgober/cli-forge/releases/tag/v0.1.0
