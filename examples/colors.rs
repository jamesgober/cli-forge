//! The color model: the eight standard names, plus arbitrary 24-bit colors via
//! hex or RGB. On terminals that cannot render 24-bit color the values are
//! downgraded to the nearest representable color rather than dropped.
//!
//! ```bash
//! cargo run --example colors
//! # Force plain output to see the graceful no-color fallback:
//! NO_COLOR=1 cargo run --example colors
//! ```

use cli_forge::{out, style};

fn main() {
    out(style("Standard named colors").bold());
    for (name, swatch) in [
        ("black", style("  black  ").black()),
        ("red", style("  red    ").red()),
        ("green", style("  green  ").green()),
        ("yellow", style("  yellow ").yellow()),
        ("blue", style("  blue   ").blue()),
        ("magenta", style("  magenta").magenta()),
        ("cyan", style("  cyan   ").cyan()),
        ("white", style("  white  ").white()),
    ] {
        out(format!("{swatch}  {name}"));
    }

    out("");
    out(style("24-bit color (hex and rgb)").bold());
    out(style("  #ff8800 — amber").hex("#ff8800"));
    out(style("  rgb(0, 200, 120) — teal").rgb(0, 200, 120));
    out(style("  #3b82f6 — link blue, underlined")
        .hex("#3b82f6")
        .underline());

    out("");
    out("Each of the lines above is plain text when this program's output is a");
    out("pipe, a file, or a NO_COLOR environment — the styling simply falls away.");
}
