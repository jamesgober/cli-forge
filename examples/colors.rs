//! The whole colour and attribute model, as a swatch sheet.
//!
//! Every line here is also a graceful-degradation test: run it on a true-colour
//! terminal, then on a 256- or 16-colour one, then with colour off, and the
//! layout does not move — only the fidelity changes.
//!
//! ```bash
//! cargo run --example colors
//!
//! # Plain output, to see the no-colour fall-back:
//! NO_COLOR=1 cargo run --example colors
//!
//! # Or force a depth, to see what a less capable terminal gets:
//! cargo run --example colors            # whatever this terminal supports
//! ```

use cli_forge::{ColorLevel, Style, out, style, terminal, text};

fn main() {
    heading("The sixteen terminal colours");
    out("These follow the user's own scheme, which is usually what a CLI wants:");
    out("");
    for (name, plain, bright) in [
        ("black", Style::new().black(), Style::new().bright_black()),
        ("red", Style::new().red(), Style::new().bright_red()),
        ("green", Style::new().green(), Style::new().bright_green()),
        (
            "yellow",
            Style::new().yellow(),
            Style::new().bright_yellow(),
        ),
        ("blue", Style::new().blue(), Style::new().bright_blue()),
        (
            "magenta",
            Style::new().magenta(),
            Style::new().bright_magenta(),
        ),
        ("cyan", Style::new().cyan(), Style::new().bright_cyan()),
        ("white", Style::new().white(), Style::new().bright_white()),
    ] {
        out(format!(
            "  {}  {}  {}",
            plain.paint("████"),
            bright.paint("████"),
            label(name),
        ));
    }

    heading("Backgrounds");
    out(format!(
        "  {}  {}  {}",
        Style::new().black().on_green().bold().paint(" PASS "),
        Style::new().white().on_red().bold().paint(" FAIL "),
        Style::new().black().on_yellow().paint(" SKIP "),
    ));

    heading("The 256-colour palette");
    out("  the 6x6x6 cube:");
    for row in 0..6u16 {
        let mut line = String::from("    ");
        for cell in 0..36u16 {
            let index = 16 + row * 36 + cell;
            // The index is always within the cube, so the cast cannot lose data.
            line.push_str(&Style::new().on_ansi(index as u8).paint(" ").to_string());
        }
        out(line);
    }
    out("  the greyscale ramp:");
    let mut ramp = String::from("    ");
    for index in 232..=255u8 {
        ramp.push_str(&Style::new().on_ansi(index).paint("  ").to_string());
    }
    out(ramp);

    heading("Exact 24-bit colour");
    out(style("  #ff8800 — amber").hex("#ff8800"));
    out(style("  #f80 — the same, in shorthand").hex("#f80"));
    out(style("  rgb(0, 200, 120) — teal").rgb(0, 200, 120));
    out(style("  #3b82f6 — a brand blue, underlined")
        .hex("#3b82f6")
        .underline());

    heading("Attributes");
    for (name, styled) in [
        ("bold", Style::new().bold()),
        ("dim", Style::new().dim()),
        ("italic", Style::new().italic()),
        ("underline", Style::new().underline()),
        ("reverse", Style::new().reverse()),
        ("strike", Style::new().strike()),
        ("blink", Style::new().blink()),
    ] {
        out(format!(
            "  {}  {}",
            styled.paint("sample text  "),
            label(name)
        ));
    }
    out(format!(
        "  {}  {}",
        Style::new().hidden().paint("sample text  "),
        label("hidden — present but not drawn; not a security measure"),
    ));

    heading("Hyperlinks");
    out(format!(
        "  {}",
        Style::new()
            .cyan()
            .underline()
            .link("https://docs.rs/cli-forge")
            .paint("the documentation")
    ));
    out("  Terminals that support OSC 8 make that clickable; the rest show the");
    out("  text unchanged, so it is always safe to add.");

    heading("Degradation");
    let brand = Style::new().hex("#3b82f6");
    out("  The same exact colour at each capability tier:");
    for (name, level) in [
        ("true colour", ColorLevel::TrueColor),
        ("256 colour ", ColorLevel::Ansi256),
        ("16 colour  ", ColorLevel::Ansi16),
        ("none       ", ColorLevel::None),
    ] {
        let swatch = brand.paint_at("████", level).to_string();
        out(format!(
            "    {name}  {swatch}  {} byte(s) of escapes",
            swatch.len() - text::strip(&swatch).len(),
        ));
    }

    out("");
    out(format!(
        "This terminal was detected as: {:?} (stdout), {:?} (stderr).",
        terminal::level(cli_forge::Stream::Stdout),
        terminal::level(cli_forge::Stream::Stderr),
    ));
    out("Every line above is plain text when output is a pipe, a file, or under");
    out("NO_COLOR — the styling simply falls away, and nothing else moves.");
}

/// A section heading, with the blank line before it.
fn heading(text: &str) {
    out("");
    out(style(text).bold().underline());
    out("");
}

/// A de-emphasised label.
fn label(text: &str) -> String {
    Style::new().bright_black().paint(text).to_string()
}
