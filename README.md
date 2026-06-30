<h1 align="center">
    <img width="99" alt="Rust logo" src="https://raw.githubusercontent.com/jamesgober/rust-collection/72baabd71f00e14aa9184efcb16fa3deddda3a0a/assets/rust-logo.svg">
    <br>
    <b>cli-core</b>
    <br>
    <sub><sup>UNIFIED CLI FRAMEWORK</sup></sub>
</h1>

<div align="center">
    <a href="https://crates.io/crates/cli-core"><img alt="Crates.io" src="https://img.shields.io/crates/v/cli-core"></a>
    <a href="https://crates.io/crates/cli-core"><img alt="Downloads" src="https://img.shields.io/crates/d/cli-core?color=%230099ff"></a>
    <a href="https://docs.rs/cli-core"><img alt="docs.rs" src="https://img.shields.io/docsrs/cli-core"></a>
    <a href="https://github.com/jamesgober/cli-core/actions"><img alt="CI" src="https://github.com/jamesgober/cli-core/actions/workflows/ci.yml/badge.svg"></a>
    <a href="https://github.com/rust-lang/rfcs/blob/master/text/2495-min-rust-version.md"><img alt="MSRV" src="https://img.shields.io/badge/MSRV-1.85%2B-blue"></a>
</div>

<br>

<div align="left">
    <p>
        cli-core is a unified command-line framework where argument parsing and styled output speak one API. Commands register at runtime - from anywhere, not just main - and can be hidden or auth-gated; output flows through a single layer (plain, tag-parsed, or builder-styled) that extensions like tables, progress, and gradients reuse seamlessly. It targets the lightness of argh with the reach of clap, without the split between parsing in one crate and styling in five.
    </p>
    <br>
    <hr>
    <p>
        <strong>MSRV is 1.85+</strong> (Rust 2024 edition).
    </p>
    <blockquote>
        <strong>Status: pre-1.0, in active development.</strong> The public API is being designed across the 0.x series and frozen at <code>1.0.0</code>. See <a href="./CHANGELOG.md"><code>CHANGELOG.md</code></a>.
    </blockquote>
</div>

<hr>
<br>

## Installation

```toml
[dependencies]
cli-core = "0.1"
```

<br>

## Status

This is the <code>v0.1.0</code> scaffold: structure, tooling, and quality gates are in place; the implementation lands across the 0.x series per the <a href="./dev/ROADMAP.md"><code>ROADMAP</code></a> and <a href="./docs/API.md"><code>docs/API.md</code></a>.

<hr>
<br>

## Contributing

See <a href="./dev/DIRECTIVES.md"><code>dev/DIRECTIVES.md</code></a> for engineering standards and the definition of done. Before a PR: `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features` must be clean.

<br>

<div id="license">
    <h2>License</h2>
    <p>Licensed under either of</p>
    <ul>
        <li><b>Apache License, Version 2.0</b> &mdash; <a href="./LICENSE-APACHE">LICENSE-APACHE</a></li>
        <li><b>MIT License</b> &mdash; <a href="./LICENSE-MIT">LICENSE-MIT</a></li>
    </ul>
    <p>at your option.</p>
</div>

<div align="center">
  <h2></h2>
  <sup>COPYRIGHT <small>&copy;</small> 2026 <strong>James Gober <me@jamesgober.com>.</strong></sup>
</div>
