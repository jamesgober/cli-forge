//! Output-layer benchmarks.
//!
//! The headline number the roadmap asks for is that the plain path does no
//! styling work: `bench_plain_write` reproduces exactly what
//! [`cli_forge::out`](cli_forge::out) does with a `&str` — `writeln!` to an
//! already-allocated writer — and it is dramatically cheaper than rendering a
//! styled value. The styling benchmarks force a true-color level so they measure
//! real escape-sequence generation rather than the plain-output fast path the
//! non-terminal benchmark process would otherwise resolve to.

use std::hint::black_box;
use std::io::Write;

use cli_forge::{define_tag, style, tag};
use criterion::{Criterion, criterion_group, criterion_main};

/// Force the terminal backend to resolve a true-color level so the styling
/// benchmarks exercise the full render path. Must run before the first styled
/// render (which caches the detected level) and before Criterion spawns threads.
fn force_truecolor() {
    // SAFETY: called once at the top of the benchmark group, single-threaded,
    // before any color level is resolved and before any worker thread starts.
    unsafe {
        std::env::set_var("CLICOLOR_FORCE", "1");
        std::env::set_var("COLORTERM", "truecolor");
    }
}

fn output_benches(c: &mut Criterion) {
    force_truecolor();
    define_tag("bench-error", style("").red().bold());

    // The plain path: reproduce `out`'s formatting against a reused buffer.
    let mut buffer: Vec<u8> = Vec::with_capacity(256);
    c.bench_function("plain_write", |b| {
        b.iter(|| {
            buffer.clear();
            let _ = writeln!(buffer, "{}", black_box("deploying release artifacts"));
            black_box(&buffer);
        });
    });

    // Building a styled value with no attributes still hits the render fast path.
    c.bench_function("builder_render_unstyled", |b| {
        b.iter(|| black_box(style(black_box("deploying release artifacts")).render()));
    });

    // The full styling cost: named color plus bold, rendered to true color.
    c.bench_function("builder_render_styled", |b| {
        b.iter(|| {
            black_box(
                style(black_box("deploying release artifacts"))
                    .red()
                    .bold()
                    .render(),
            )
        });
    });

    // A 24-bit color, the most expensive foreground to encode.
    c.bench_function("builder_render_rgb", |b| {
        b.iter(|| black_box(style(black_box("status: ok")).rgb(0, 200, 120).render()));
    });

    // The named-registry path: a lock-guarded lookup plus a styled render.
    c.bench_function("registry_render", |b| {
        b.iter(|| black_box(tag(black_box("bench-error")).render_with(black_box("build failed"))));
    });
}

/// Command-layer benchmarks: resolving and parsing an invocation (the app is
/// built once; only `try_parse_from` — parse plus dispatch of an empty handler —
/// is measured).
fn parse_benches(c: &mut Criterion) {
    use cli_forge::{App, Arg, Command};

    let mut app = App::new("bench").version("1.0.0");
    app.register(
        Command::new("build")
            .arg(Arg::flag("release").short('r'))
            .arg(Arg::count("verbose").short('v'))
            .arg(Arg::option("jobs").short('j').default("1"))
            .arg(Arg::option("define").short('D').multiple(true))
            .arg(Arg::positional("targets").multiple(true))
            .run(|_| {}),
    );

    // A minimal invocation: one command, one flag.
    c.bench_function("parse_simple", |b| {
        b.iter(|| black_box(app.try_parse_from(black_box(["build", "-r"]))));
    });

    // A realistic invocation exercising counts, repeated options, and variadics.
    c.bench_function("parse_rich", |b| {
        b.iter(|| {
            black_box(app.try_parse_from(black_box([
                "build",
                "-vvv",
                "--release",
                "-D",
                "A",
                "-D",
                "B",
                "-j",
                "8",
                "a.rs",
                "b.rs",
            ])))
        });
    });
}

criterion_group!(benches, output_benches, parse_benches);
criterion_main!(benches);
