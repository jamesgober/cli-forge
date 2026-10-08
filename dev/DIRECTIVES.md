# cli-forge &mdash; Engineering Directives

> Engineering standards and the definition of done for this project. Read alongside `REPS.md` (root, authoritative) and `dev/ROADMAP.md` (current phase). If anything here conflicts with `REPS.md`, `REPS.md` wins.

---

## 0. Philosophy

This library is built and maintained to a production standard and treated as a flagship piece of work. Plan the full path, then build one verified step at a time. "Good enough" is treated as a defect. cli-forge is foundational: it powers internal tooling, AVA, and database CLIs, and it is the base every higher CLI layer (tables, progress, gradients, shells) builds on. Its API is the thing developers touch every day, so the bar is not just "works" but "clean, obvious, and hard to misuse."

---

## 1. What this is

cli-forge is a unified command-line framework: argument parsing and styled output through one API, with commands that register at runtime. It owns four things and nothing else: parsing (a recursive command tree with args/flags), output (one styling layer used three ways), command registration (from anywhere, hideable, auth-gateable), and help (auto-generated, customizable). It deliberately does NOT own tables, progress bars, gradients, layouts, or shells &mdash; those are sibling crates in the cli collection that consume this crate's output API, so the core stays small and they all speak one system.

---

## 2. Engineering law (non-negotiable)

- **Simplified API (hard requirement).** The common case is one call: `out("text")`. Every feature is reachable without ceremony. If a use looks like the ugly Rust-CLI status quo, it is wrong. No required builder boilerplate for trivial output. This rule is paramount and only yields to a proven, significant cost to another primary metric.
- **No hard-coded appearance at a call site.** If a program has to name a colour, build a marker, or pad a column to print a status line, the library has failed. A `Theme` states the vocabulary once and a `Style` carries its whole appearance; call sites say *what happened*, not *what it should look like*. The test of any new output API is whether the thing it produces can be described once and reused.
- **Measure in display columns, never in bytes.** `str::len` and `{:<width$}` are wrong for anything a terminal renders. Alignment, padding, truncation, and wrapping all go through `text`, which is also the seam sibling crates depend on; a second implementation of it anywhere is a defect.
- **Performance.** The plain output path (`out`/`err`) does no tag parsing and no styling work &mdash; it is a near-direct write. Parsing/styling cost is paid only when explicitly used. Arg parsing is allocation-conscious. No "faster" claim without `criterion` numbers.
- **Flexibility.** Commands register at runtime from any module, not only `main`. No fixed source location for custom commands (the limitation that made the predecessor unusable). Commands can be hidden from help and marked auth-gated.
- **Correctness.** Tag parsing, style rendering, and arg parsing are covered by tests, including malformed tags and ambiguous args.
- **Cross-platform.** Linux/macOS/Windows first-class. ANSI vs Windows-console differences are isolated behind one terminal backend; the public API never exposes the difference.
- **Architecture.** SOLID, KISS, YAGNI. The output layer is a seam that sibling crates depend on. The auth model is a seam (flag + hook), not baked-in logic.
- **Error handling.** Parse failures return structured errors that render through the same output system; nothing panics on bad user input — including input that is not valid UTF-8, which `std::env::args` panics on and so must never be used. An error says what was wrong, what would have been right, and what to do next: the nearest valid spelling and the usage line of the command being invoked.
- **Validate at the edge.** A value is checked while it is still the user's mistake to make, so reading it back later cannot fail for a reason the user caused. A library that makes the program `unwrap` three functions deep has moved the error to where it cannot be reported well.
- **Untrusted text is never printed verbatim.** A filename, a commit message, or a server response can carry escape sequences; printing one hands the terminal to whoever wrote it. `text::sanitize` exists for this and the documentation points at it wherever text from outside the program can arrive.
- **Production-ready.** `#![forbid(unsafe_code)]` and `#![deny(missing_docs)]` from the first commit; no stray `println!`/`dbg!`; every public item has rustdoc with a runnable example.

---

## 3. Definition of done

1. Compiles clean on Linux/macOS/Windows, stable and MSRV 1.85, on **every selectable feature combination** &mdash; including the bare `no_std` build, which is a real build of the styling core and not an empty crate.
2. `fmt`, `clippy -D warnings`, `test --all-features`, `cargo doc -D warnings` clean.
3. `cargo audit` + `cargo deny check` pass.
4. No `unwrap`/`expect`/`todo!`/`dbg!` in shipping code.
5. The simplified API is real: the headline examples in the docs are short and obvious.
6. Output, parsing, and command registration covered by tests.
7. Hot-path (`out`) changes carry benchmarks; no regression over 5%. A performance claim without criterion numbers is not a claim. Measurement noise is distinguished from a regression by re-running, not by assuming.
8. Docs and `CHANGELOG.md` updated; the matching `docs/release/vX.Y.Z.md` written before the tag.

---

## 4. Project-specific invariants

- `out`/`err` never parse markup and never allocate for styling &mdash; plain text in, bytes out.
- The same logical style can be produced four ways (markup string, builder, named registry, theme level) and all four render byte-identical output for the same intent, at every colour depth. This is what makes mixing them safe, and it is asserted directly.
- Colour capability is decided **per stream**, and always overridable. A detection result that cannot be overridden makes styled output untestable, which is how the 1.x help tests came to depend on whether the harness had a terminal.
- A named style captures a whole appearance &mdash; glyph, spacing, width, colour &mdash; so no call site rebuilds a marker.
- Nothing in the public surface is write-only. `App`, `Command`, and `Arg` are readable from outside, because a sibling crate generating completions or manual pages must read the live tree rather than a second description of the same CLI.
- A command registered at runtime from a non-`main` module is reachable and behaves identically to one registered in `main`.
- A hidden command never appears in generated help but still runs if invoked.
- An auth-gated command does not appear in help and does not run unless the auth hook authorizes it (enforcement arrives with the auth seam; the flag and hook exist from the core).
- Help output is produced through the same output layer as everything else, so custom header/footer and styling apply uniformly, and it is aligned in display columns and wrapped to the terminal.
- Color works by name (`red`, `bright_red`), by palette index, and by custom value (`hex`/`rgb`), as foreground or background; custom colors degrade to the nearest the terminal can render rather than being dropped.
- Parsing has no side effects. `try_parse_from` parses and nothing else, so the testing entry point is the one without them; dispatch is a separate, explicit step.
- A documentation example is executed. Every public item carries a runnable example, and the README's examples are compiled and run as a test, because documentation that is never executed rots and a broken first impression is the most expensive kind of bug a library can have.
