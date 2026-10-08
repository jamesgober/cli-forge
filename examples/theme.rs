//! Themed responses: the eight levels a CLI speaks in, and what changing the
//! theme does to all of them at once.
//!
//! ```bash
//! cargo run --example theme
//!
//! # Diagnostics go to standard error, so data stays pipeable:
//! cargo run --example theme 2>/dev/null   # only the stdout levels
//! cargo run --example theme 1>/dev/null   # only the stderr levels
//! ```

use cli_forge::{
    Glyphs, Level, Stream, Style, Theme, debug, fail, hint, info, note, ok, out, style, trace, warn,
};

fn main() {
    out(style("The default theme").bold().underline());
    out("");
    every_level();

    out("");
    out(style("The same program, restyled in one place")
        .bold()
        .underline());
    out("");
    // Nothing below `install` changes — only the theme does.
    Theme::new()
        .set(
            Level::Success,
            Style::new().black().on_green().bold(),
            " OK ",
        )
        .set(Level::Error, Style::new().white().on_red().bold(), " ER ")
        .set(Level::Warning, Style::new().black().on_yellow(), " WARN ")
        .style_for(Level::Info, Style::new().bright_blue().italic())
        .install();
    every_level();

    out("");
    out(
        style("ASCII markers, for a console that cannot render the rest")
            .bold()
            .underline(),
    );
    out("");
    Theme::new().glyphs(Glyphs::Ascii).install();
    every_level();

    out("");
    out(style("No styling at all — what `--plain` should do")
        .bold()
        .underline());
    out("");
    Theme::plain().install();
    every_level();

    // A theme is a value, so a library can render through its own without
    // disturbing the host program's.
    out("");
    let mine = Theme::new().glyphs(Glyphs::Unicode);
    out(mine.render(Level::Hint, "rendered through a theme held locally"));

    // And a level can be redirected when the convention does not suit.
    let warnings_in_band = Theme::new().stream(Level::Warning, Stream::Stdout);
    out(warnings_in_band.render(Level::Warning, "this one would go to stdout"));
}

/// Print one line at every level, so a theme change can be seen whole.
fn every_level() {
    ok("deployed to staging");
    fail("smoke test failed");
    warn("2 tests skipped");
    info("using cached dependencies");
    hint("try `--release` for an optimised build");
    note("config was read from ./forge.toml");
    debug("resolved 41 packages in 12ms");
    trace("cache hit: libcore-1.0.0");
}
