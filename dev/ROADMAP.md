# cli-forge — Roadmap

> Path from scaffold to a stable 1.0. Hard parts are front-loaded; each phase has hard exit criteria.
>
> **Anti-deferral rule:** no listed hard task moves to a later phase unless this file records the move and the reason.

---

## v0.1.0 — Scaffold (DONE)

Compiles, CI green, structure correct, no domain logic.

- [x] Manifest, README, CHANGELOG, REPS, dual license, CI, deny, clippy, rustfmt.
- [x] Frozen public interface sketched in `docs/API.md`.

---

## v0.2.0 — Output tower + terminal backend (DONE, shipped in 0.2.5)

The output layer is the load-bearing piece every sibling crate depends on, so it
is built and proven first. Deliver the three styling paths over ONE system:
`out`/`err` (plain, no parsing, near-direct write), `parse` (tag strings with
named AND hex/rgb colors), and the `style(..)` builder. Plus the named-tag
registry (`define_tag`/`tag`) so a style is defined once and reused. All of it
sits on a single cross-platform terminal backend that isolates ANSI vs
Windows-console so the public API never exposes the difference.

Exit criteria:
- [x] `out` proven allocation-free / no-parse by benchmark, not by claim. (`tests/allocation.rs` + `benches/bench.rs`.)
- [x] The three styling paths render byte-identical output for the same intent (test). (`src/crosspath_tests.rs`.)
- [x] Named + hex + rgb colors work; graceful degradation on limited terminals (test).
- [x] Verified on Linux, macOS, and Windows console. (Linux via WSL2 + Windows directly; macOS shares the identical non-Windows code path.)
- [x] Every public item has rustdoc + a runnable example.

---

## v0.3.0 — Command tree + runtime registration (DONE)

The recursive `Command` tree with args/flags, and an `App` registry that accepts
commands registered FROM ANYWHERE (not main-only) — the limitation that killed
the predecessor. Commands support `.hidden()` and the `.requires_auth()` flag.

Exit criteria:
- [x] A command registered from a non-`main` module is reachable and behaves identically (test). (`tests/registration.rs`.)
- [x] Hidden commands are absent from help but still invokable (test). (Excluded from the visible-command listing; still dispatch.)
- [x] Arg/flag parsing handles the standard cases; malformed input returns structured errors, never panics. (`ParseError`; parser proptest-fuzzed.)

---

## v0.4.0 — Help engine + customization (DONE)

Auto-generated help rendered through the output layer, with injectable
`help_header`/`help_footer` slots and per-command styling. Errors render through
the same system. Also folded in the small conveniences a base CLI expects:
command aliases, `--help`/`-h`, and `--version`/`-V`.

Exit criteria:
- [x] Help is styleable and respects custom header/footer (test). (Section headers styled through the output layer; `App::help()` + header/footer asserted.)
- [x] Hidden/auth-gated commands honored in help generation. (Both excluded from listings; tested.)

---

## v0.5.0 — Auth seam, feature freeze (DONE)

The auth hook that enforces `requires_auth` (login/logout state supplied by the
consumer or a sibling `cli-auth` crate — core holds the seam, not the logic).
Public surface declared frozen.

Exit criteria:
- [x] An auth-gated command does not run unless the hook authorizes it (test). (`App::auth` hook; `enforce_auth` fails closed; tested both feature sets.)
- [x] API surface documented as frozen in `docs/API.md`. (See the Stability section.)

---

## v0.6.0 — Argument conveniences (DONE)

Strictly-additive polish within the frozen surface — the small, common argument
kinds a base CLI is expected to have, added without changing anything existing.

- [x] Counting flags: `Arg::count` + `Matches::count` (`-vvv` verbosity).
- [x] Multiple values: `Arg::multiple` (repeatable options + variadic positionals)
      + `Matches::values`.
- [x] Comprehensive edge-case tests on both feature sets; docs and an example.

---

## v1.0.0 — API freeze (DONE)

The parse + output + registration + help surface is stable and frozen until 2.0.
No new public API, only documentation, tests, and internal optimisation.
Sibling crates (`cli-table`, `cli-progress`, gradients, layouts, shell) build on
this frozen base.

Exit criteria:
- [x] `docs/API.md` marked stable; SemVer promise recorded. (See the Stability section.)
- [x] Full test + benchmark suite green on all three platforms. (Windows + Linux verified directly; macOS via the CI matrix on the shared non-Windows path. Output + command-parse benchmarks green.)
---

## v2.0.0 — Themed output, the argument model, the suite seams (DONE)

The 1.0 freeze held until it was in the way. Three things forced a major:

1. **Output was not reusable.** A named style carried colour only, so every
   status line in a program rebuilt its own marker by hand — the duplication the
   1.x `status_report` example demonstrated rather than solved.
2. **The argument model was too thin for a real CLI**, and four confirmed defects
   each made a legitimate command line unparseable.
3. **The crate could not serve as the suite's base.** There was no way to measure
   styled text, no way to override colour, and no public accessor on `App`,
   `Command`, or `Arg` — so a completions or manual-page crate could not read the
   command tree at all.

Exit criteria:

- [x] A theme states a program's vocabulary once; no call site names a colour.
      (`Theme`, `Level`, `Glyphs`, and the eight printers. `examples/theme.rs`
      restyles a whole report by replacing the theme.)
- [x] A style carries its whole appearance, so a marker is described once.
      (`prefix`/`suffix`/`pad_to`/`align`/`link`/`merge` + `paint`;
      `examples/status_report.rs` rewritten to show the difference.)
- [x] All four styling paths render byte-identical output for the same intent,
      at every colour depth (`src/crosspath_tests.rs`).
- [x] Each of the four parser defects reproduced, then fixed, with a regression
      test naming the 1.x behaviour (`src/app/tests.rs`).
- [x] Values validated at the edge, so reading them back cannot fail for a reason
      the user caused (`Arg::validate`/`possible_values`, `Matches::get`).
- [x] Errors carry a suggestion and a usage line; help and version exit `0` on
      standard output, a bad command line exits `2`.
- [x] Parsing and dispatch separated; `App::run` returns an `ExitCode`.
- [x] Help aligned in display columns and wrapped to the terminal.
- [x] The measurement seam exists and is public (`text`), and the command tree is
      readable from outside (the read-only accessors).
- [x] `--no-default-features` is a real `no_std` build with its own passing suite,
      not an empty crate.
- [x] Untrusted text can be printed safely (`text::sanitize`).
- [x] Eight feature combinations green in CI; MSRV 1.85 verified; `fmt`, `clippy
      -D warnings`, `cargo doc -D warnings` clean on each.
- [x] Styling measurably faster, with criterion numbers, and no regression on the
      plain path.
- [x] The README's examples compiled and run as a test, so they cannot rot.

---

## Next

The core is the base; the extensions are separate crates that drop onto these
seams. Nothing below belongs in this crate.

- `cli-table` — tables and grids, measuring through `text::width`.
- `cli-progress` — bars and spinners, styled through `Theme` and sized through
  `terminal::size`.
- `cli-complete` — shell completions, generated from the read-only command tree.
- `cli-man` — manual pages, from the same tree.
- `cli-prompt` — interactive input: confirm, select, password.
- `cli-gradient` — gradient and multi-stop colour over `Color`.
- `cli-suite` — the umbrella crate combining them behind feature flags.

The one thing to hold to: each of those consumes this crate's public seams and
adds nothing to it. If an extension needs something the core does not expose, the
core gets a minor, additive release — not a copy of the seam.
