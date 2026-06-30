# cli-core — Roadmap

> Path from scaffold to a stable 1.0. Hard parts are front-loaded; each phase has hard exit criteria.
>
> **Anti-deferral rule:** no listed hard task moves to a later phase unless this file records the move and the reason.

---

## v0.1.0 — Scaffold (DONE)

Compiles, CI green, structure correct, no domain logic.

- [x] Manifest, README, CHANGELOG, REPS, dual license, CI, deny, clippy, rustfmt.
- [x] Frozen public interface sketched in `docs/API.md`.

---

## v0.2.0 — Output tower + terminal backend (DONE)

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

## v0.3.0 — Command tree + runtime registration

The recursive `Command` tree with args/flags, and an `App` registry that accepts
commands registered FROM ANYWHERE (not main-only) — the limitation that killed
the predecessor. Commands support `.hidden()` and the `.requires_auth()` flag.

Exit criteria:
- [ ] A command registered from a non-`main` module is reachable and behaves identically (test).
- [ ] Hidden commands are absent from help but still invokable (test).
- [ ] Arg/flag parsing handles the standard cases; malformed input returns structured errors, never panics.

---

## v0.4.0 — Help engine + customization

Auto-generated help rendered through the output layer, with injectable
`help_header`/`help_footer` slots and per-command styling. Errors render through
the same system.

Exit criteria:
- [ ] Help is styleable and respects custom header/footer (snapshot test).
- [ ] Hidden/auth-gated commands honored in help generation.

---

## v0.5.0 — Auth seam, feature freeze

The auth hook that enforces `requires_auth` (login/logout state supplied by the
consumer or a sibling `cli-auth` crate — core holds the seam, not the logic).
Public surface declared frozen.

Exit criteria:
- [ ] An auth-gated command does not run unless the hook authorizes it (test).
- [ ] API surface documented as frozen in `docs/API.md`.

---

## v1.0.0 — API freeze

The parse + output + registration + help surface is stable and frozen until 2.0.
No new public API, only documentation, tests, and internal optimisation.
Sibling crates (`cli-table`, `cli-progress`, gradients, layouts, shell) build on
this frozen base.

Exit criteria:
- [ ] `docs/API.md` marked stable; SemVer promise recorded.
- [ ] Full test + benchmark suite green on all three platforms.
