//! The same styled line produced four ways — builder, inline markup, a named
//! style, and a theme level. All four render to identical bytes for the same
//! intent; the choice between them is ergonomic, not visual.
//!
//! ```bash
//! cargo run --example four_paths
//! ```

use cli_forge::{Level, Style, Theme, define, markup, named, out, style};

fn main() {
    out("Four ways to say the same thing:");
    out("");

    // 1. The builder. Chain methods; the result is `Display`, so it drops into
    //    `out` directly. Best when the style is computed or genuinely one-off.
    out(style("ERROR: build failed").red().bold());

    // 2. Inline markup. The whole line is one string with tags in it. Best when
    //    the text and its styling are written together, like a template — and
    //    the only path where the styling varies run by run inside a sentence.
    out(markup("<c=red><b>ERROR: build failed</b></c>"));

    // 3. A named style. Describe the look once, recall it anywhere by name.
    //    Best when the same look recurs across a program: define it in one
    //    module, use it in another, change it in one place.
    define("error", Style::new().red().bold());
    out(named("error").paint("ERROR: build failed"));

    // 4. A theme level. Best of all for the handful of things every program
    //    says — nothing here names a colour, so a different theme restyles the
    //    whole program without touching this line.
    Theme::new()
        .set(Level::Error, Style::new().red().bold(), "")
        .install();
    cli_forge::fail("ERROR: build failed");

    out("");
    out("Markup nests and mixes freely:");
    out(markup(
        "<b>summary</b>: <c=green>12 passed</c>, <c=red>1 failed</c>, <c=#888888>3 skipped</c>",
    ));

    out("");
    out("A named style carries its decoration too, so no call site rebuilds a");
    out("marker by hand — the glyph, the spacing, and the column width travel");
    out("with the name:");
    define(
        "step",
        Style::new().bright_black().prefix("  → ").pad_to(28),
    );
    let step = named("step");
    for label in ["resolve dependencies", "compile", "link"] {
        out(step.paint(label));
    }
}
