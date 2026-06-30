<h1 align="center">
    <img width="90px" height="auto" src="https://raw.githubusercontent.com/jamesgober/jamesgober/main/media/icons/hexagon-3.svg" alt="Triple Hexagon">
    <br><b>CHANGELOG</b>
</h1>
<p>
  All notable changes to <code>cli-core</code> will be documented in this file. The format is based on <a href="https://keepachangelog.com/en/1.1.0/">Keep a Changelog</a>,
  and this project adheres to <a href="https://semver.org/spec/v2.0.0.html/">Semantic Versioning</a>.
</p>

---

## [Unreleased]

### Added

### Changed

### Fixed

### Security

---

## [0.2.0] - 2026-06-30

The output layer — the load-bearing piece every sibling crate depends on. Three
styling paths over one cross-platform terminal backend, with the plain path proven
allocation-free by test rather than by claim.

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
- `deny.toml` header comment corrected (`rate-net` &rarr; `cli-core`).
- `Cargo.lock` is now committed (removed from `.gitignore`) for reproducible
  builds, as REPS requires.

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

[Unreleased]: https://github.com/jamesgober/cli-core/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/jamesgober/cli-core/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/jamesgober/cli-core/releases/tag/v0.1.0
