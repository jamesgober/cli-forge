//! Benchmarks for the paths whose cost the engineering directives make a
//! promise about.
//!
//! Three claims are measured here rather than asserted in prose:
//!
//! 1. **The plain path does no styling work.** `plain_write` reproduces exactly
//!    what [`cli_forge::out`] does with a `&str` — `writeln!` to an
//!    already-allocated writer — and must stay dramatically cheaper than
//!    anything that renders.
//! 2. **Styling costs only what it must.** The builder benchmarks separate an
//!    unstyled render from a styled one, a named colour from a 24-bit one, and a
//!    plain paint from a padded one, so a regression lands on a specific line
//!    rather than on "styling".
//! 3. **Reuse is cheaper than rebuilding.** `paint_reused` holds one style and
//!    applies it, while `registry_lookup_then_paint` pays the store lookup on
//!    every line. The gap between them is the reason the documentation tells
//!    callers to hoist the lookup out of a loop.
//!
//! The styling benchmarks force an explicit colour depth rather than relying on
//! detection, which in a benchmark process would resolve to "no terminal" and
//! quietly measure the plain-output fast path instead of the thing named.

use std::hint::black_box;
use std::io::Write;

use cli_forge::{
    App, Arg, Color, ColorLevel, Command, Level, Style, Theme, define, named, terminal,
};
use criterion::{Criterion, criterion_group, criterion_main};

/// A line of the length a real tool prints.
const LINE: &str = "deploying release artifacts to the staging environment";

fn output_benches(c: &mut Criterion) {
    // Force the depth so the render paths are actually exercised. Unlike the
    // previous approach of setting environment variables, this needs no `unsafe`
    // and cannot race with Criterion's worker threads.
    terminal::set_level(ColorLevel::TrueColor);

    // The plain path: reproduce `out`'s formatting against a reused buffer.
    let mut buffer: Vec<u8> = Vec::with_capacity(256);
    let _ = c.bench_function("plain_write", |b| {
        b.iter(|| {
            buffer.clear();
            let _ = writeln!(buffer, "{}", black_box(LINE));
            black_box(&buffer);
        });
    });

    // A style with nothing set must take the plain fast path even at full depth.
    let unstyled = Style::new();
    let _ = c.bench_function("paint_unstyled", |b| {
        b.iter(|| black_box(unstyled.paint(black_box(LINE)).to_string()));
    });

    // One named colour plus one attribute: the common case.
    let simple = Style::new().red().bold();
    let _ = c.bench_function("paint_named_colour", |b| {
        b.iter(|| black_box(simple.paint(black_box(LINE)).to_string()));
    });

    // A 24-bit foreground and background: the most expensive colour to encode.
    let exact = Style::new().rgb(0, 200, 120).on_rgb(20, 20, 20);
    let _ = c.bench_function("paint_exact_colour", |b| {
        b.iter(|| black_box(exact.paint(black_box(LINE)).to_string()));
    });

    // Downgrading is where the capability tiers do real work, so it is measured
    // separately from encoding an exact value.
    let _ = c.bench_function("paint_downgraded_to_16", |b| {
        b.iter(|| {
            black_box(
                exact
                    .paint_at(black_box(LINE), ColorLevel::Ansi16)
                    .to_string(),
            )
        });
    });

    // Padding is the one builder feature documented as costing an allocation,
    // because the finished text has to be measured first.
    let padded = Style::new().red().pad_to(72);
    let _ = c.bench_function("paint_padded", |b| {
        b.iter(|| black_box(padded.paint(black_box(LINE)).to_string()));
    });

    let _ = c.bench_function("paint_reused", |b| {
        b.iter(|| {
            // What the documentation recommends: resolve once, paint many.
            for _ in 0..8 {
                black_box(simple.paint(black_box(LINE)).to_string());
            }
        });
    });
}

fn markup_benches(c: &mut Criterion) {
    terminal::set_level(ColorLevel::TrueColor);

    // Text with no tags at all must not pay for the scan beyond one pass.
    let _ = c.bench_function("markup_tagless", |b| {
        b.iter(|| black_box(cli_forge::markup(black_box(LINE))));
    });

    let rich = "<b>summary</b>: <c=green>12 passed</c>, <c=red>1 failed</c>, \
                <c=#888888>3 skipped</c>";
    let _ = c.bench_function("markup_rich", |b| {
        b.iter(|| black_box(cli_forge::markup(black_box(rich))));
    });

    let nested = "<c=red><b><u>deeply styled run of text</u></b></c>";
    let _ = c.bench_function("markup_nested", |b| {
        b.iter(|| black_box(cli_forge::markup(black_box(nested))));
    });
}

fn reuse_benches(c: &mut Criterion) {
    terminal::set_level(ColorLevel::TrueColor);
    define("bench-error", Style::new().red().bold());

    // The cost the registry documentation warns about: a lock and a clone per
    // line, versus hoisting the lookup out of the loop.
    let _ = c.bench_function("registry_lookup_then_paint", |b| {
        b.iter(|| {
            black_box(
                named(black_box("bench-error"))
                    .paint(black_box(LINE))
                    .to_string(),
            )
        });
    });

    let hoisted = named("bench-error");
    let _ = c.bench_function("registry_hoisted_paint", |b| {
        b.iter(|| black_box(hoisted.paint(black_box(LINE)).to_string()));
    });

    // A themed line: glyph resolution plus the same render.
    let theme = Theme::new();
    let _ = c.bench_function("theme_render", |b| {
        b.iter(|| {
            black_box(theme.render_at(Level::Success, black_box(LINE), ColorLevel::TrueColor))
        });
    });
}

fn text_benches(c: &mut Criterion) {
    let styled = Style::new()
        .red()
        .bold()
        .paint_at(LINE, ColorLevel::TrueColor)
        .to_string();

    // Measurement is what every sibling crate calls on every cell, so it is the
    // hottest function in the text module by a wide margin.
    let _ = c.bench_function("width_plain", |b| {
        b.iter(|| black_box(cli_forge::text::width(black_box(LINE))));
    });
    let _ = c.bench_function("width_styled", |b| {
        b.iter(|| black_box(cli_forge::text::width(black_box(&styled))));
    });
    let _ = c.bench_function("strip_styled", |b| {
        b.iter(|| black_box(cli_forge::text::strip(black_box(&styled))));
    });
    // The borrowing fast path: clean text must not be copied.
    let _ = c.bench_function("strip_plain", |b| {
        b.iter(|| black_box(cli_forge::text::strip(black_box(LINE))));
    });
    let _ = c.bench_function("sanitize_clean", |b| {
        b.iter(|| black_box(cli_forge::text::sanitize(black_box(LINE))));
    });
    let _ = c.bench_function("wrap", |b| {
        b.iter(|| black_box(cli_forge::text::wrap(black_box(LINE), 24)));
    });
}

fn color_benches(c: &mut Criterion) {
    let _ = c.bench_function("color_parse_name", |b| {
        b.iter(|| black_box(Color::parse(black_box("bright_magenta"))));
    });
    let _ = c.bench_function("color_parse_hex", |b| {
        b.iter(|| black_box(Color::parse(black_box("#3b82f6"))));
    });
}

/// Command-layer benchmarks: resolving and parsing an invocation. The app is
/// built once; only `try_parse_from` — parse plus dispatch of an empty handler —
/// is measured.
fn parse_benches(c: &mut Criterion) {
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
    let _ = c.bench_function("parse_simple", |b| {
        b.iter(|| black_box(app.try_parse_from(black_box(["build", "-r"]))));
    });

    // A realistic invocation exercising counts, repeated options, and variadics.
    let _ = c.bench_function("parse_rich", |b| {
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

    // Help rendering: the path a `--help` invocation takes, including the
    // width-aware column measurement.
    let _ = c.bench_function("render_help", |b| {
        b.iter(|| black_box(app.help()));
    });
}

criterion_group!(
    benches,
    output_benches,
    markup_benches,
    reuse_benches,
    text_benches,
    color_benches,
    parse_benches
);
criterion_main!(benches);
