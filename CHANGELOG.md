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

[Unreleased]: https://github.com/jamesgober/cli-forge/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/jamesgober/cli-forge/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/jamesgober/cli-forge/compare/v0.2.5...v0.3.0
[0.2.5]: https://github.com/jamesgober/cli-forge/compare/v0.2.0...v0.2.5
[0.2.0]: https://github.com/jamesgober/cli-forge/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/jamesgober/cli-forge/releases/tag/v0.1.0
