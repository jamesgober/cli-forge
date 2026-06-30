# cli-core &mdash; Engineering Directives

> Engineering standards and the definition of done for this project. Read alongside `REPS.md` (root, authoritative) and `dev/ROADMAP.md` (current phase). If anything here conflicts with `REPS.md`, `REPS.md` wins.

---

## 0. Philosophy

This library is built and maintained to a production standard and treated as a flagship piece of work. Plan the full path, then build one verified step at a time. "Good enough" is treated as a defect. cli-core is foundational: it powers internal tooling, AVA, and database CLIs, and it is the base every higher CLI layer (tables, progress, gradients, shells) builds on. Its API is the thing developers touch every day, so the bar is not just "works" but "clean, obvious, and hard to misuse."

---

## 1. What this is

cli-core is a unified command-line framework: argument parsing and styled output through one API, with commands that register at runtime. It owns four things and nothing else: parsing (a recursive command tree with args/flags), output (one styling layer used three ways), command registration (from anywhere, hideable, auth-gateable), and help (auto-generated, customizable). It deliberately does NOT own tables, progress bars, gradients, layouts, or shells &mdash; those are sibling crates in the cli collection that consume this crate's output API, so the core stays small and they all speak one system.

---

## 2. Engineering law (non-negotiable)

- **Simplified API (hard requirement).** The common case is one call: `out("text")`. Every feature is reachable without ceremony. If a use looks like the ugly Rust-CLI status quo, it is wrong. No required builder boilerplate for trivial output. This rule is paramount and only yields to a proven, significant cost to another primary metric.
- **Performance.** The plain output path (`out`/`err`) does no tag parsing and no styling work &mdash; it is a near-direct write. Parsing/styling cost is paid only when explicitly used. Arg parsing is allocation-conscious. No "faster" claim without `criterion` numbers.
- **Flexibility.** Commands register at runtime from any module, not only `main`. No fixed source location for custom commands (the limitation that made the predecessor unusable). Commands can be hidden from help and marked auth-gated.
- **Correctness.** Tag parsing, style rendering, and arg parsing are covered by tests, including malformed tags and ambiguous args.
- **Cross-platform.** Linux/macOS/Windows first-class. ANSI vs Windows-console differences are isolated behind one terminal backend; the public API never exposes the difference.
- **Architecture.** SOLID, KISS, YAGNI. The output layer is a seam that sibling crates depend on. The auth model is a seam (flag + hook), not baked-in logic.
- **Error handling.** Parse failures return structured errors that render through the same output system; nothing panics on bad user input.
- **Production-ready.** `#![forbid(unsafe_code)]` and `#![deny(missing_docs)]` from the first commit; no stray `println!`/`dbg!`; every public item has rustdoc with a runnable example.

---

## 3. Definition of done

1. Compiles clean on Linux/macOS/Windows, stable and MSRV 1.85.
2. `fmt`, `clippy -D warnings`, `test --all-features`, `cargo doc -D warnings` clean.
3. `cargo audit` + `cargo deny check` pass.
4. No `unwrap`/`expect`/`todo!`/`dbg!` in shipping code.
5. The simplified API is real: the headline examples in the docs are short and obvious.
6. Output, parsing, and command registration covered by tests.
7. Hot-path (`out`) changes carry benchmarks; no regression over 5%.
8. Docs and `CHANGELOG.md` updated; the matching `docs/release/vX.Y.Z.md` written before the tag.

---

## 4. Project-specific invariants

- `out`/`err` never parse tags and never allocate for styling &mdash; plain text in, bytes out.
- The same logical style can be produced three ways (tag string, builder, named registry) and all three render byte-identical output for the same intent.
- A command registered at runtime from a non-`main` module is reachable and behaves identically to one registered in `main`.
- A hidden command never appears in generated help but still runs if invoked.
- An auth-gated command does not appear in help and does not run unless the auth hook authorizes it (enforcement arrives with the auth seam; the flag and hook exist from the core).
- Help output is produced through the same output layer as everything else, so custom header/footer and styling apply uniformly.
- Color works by name (`red`) and by custom value (`hex`/`rgb`); custom colors degrade gracefully on terminals that cannot render them.
