//! A realistic slice of CLI output: a deploy-style status report.
//!
//! The point of this example is what is *not* in it. No call site names a
//! colour, pads a column, or picks a glyph. The theme states the program's
//! vocabulary once; every line after that just says what happened. Swap the
//! theme at the top and the whole report changes appearance together.
//!
//! ```bash
//! cargo run --example status_report
//!
//! # The same report with no styling at all:
//! NO_COLOR=1 cargo run --example status_report
//!
//! # And with ASCII markers, as on a legacy console:
//! NO_UNICODE=1 cargo run --example status_report
//! ```

use cli_forge::{Level, Style, Theme, define, fail, markup, named, note, ok, out, style, warn};

fn main() {
    // The program's vocabulary, stated once.
    // `set_style` changes the colour and leaves the marker to the theme, so the
    // Unicode-or-ASCII decision stays automatic. `set` would pin the glyph and
    // take that choice away, which is right for a brand mark and wrong here.
    Theme::new()
        .set_style(Level::Success, Style::new().bright_green().bold())
        .set_style(Level::Warning, Style::new().bright_yellow().bold())
        .set_style(Level::Error, Style::new().bright_red().bold())
        .set(Level::Note, Style::new().bright_black(), "")
        .install();

    // Two named styles for the step line, so the column widths live in exactly
    // one place instead of in every `format!` that prints a step.
    define("step", Style::new().pad_to(22));
    define("detail", Style::new().bright_black());

    out(style("deploy: staging").bold().underline());
    out("");

    step("resolve dependencies", Status::Ok, "0.4s");
    step("compile (release)", Status::Ok, "31.7s");
    step("run test suite", Status::Warn, "12 of 14");
    step("upload artifacts", Status::Ok, "2.1s");
    step("smoke test", Status::Fail, "timeout");

    out("");
    out(markup(
        "<b>result</b>: <c=red>1 step failed</c> — see <c=#3b82f6><u>logs/smoke.txt</u></c>",
    ));
    note("finished in 48.2s");
}

/// How one deploy step turned out.
enum Status {
    Ok,
    Warn,
    Fail,
}

/// Print one status line: marker, label, and detail, in three columns.
///
/// Compare this with the hand-rolled version it replaces: there is no `format!`
/// building a padded marker, no lookup table from status to colour, and nothing
/// that has to know a glyph is one column wide. The label's width comes from the
/// named style and the marker from the theme, so either can change without this
/// function being touched — and the width is measured in display columns, so the
/// third column stays straight even when a label is not ASCII.
fn step(label: &str, status: Status, detail: &str) {
    let line = format!(
        "{} {}",
        named("step").paint(label),
        named("detail").paint(detail)
    );
    match status {
        Status::Ok => ok(line),
        Status::Warn => warn(line),
        Status::Fail => fail(line),
    }
}
