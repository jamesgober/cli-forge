//! A realistic slice of CLI output: a deploy-style status report that defines a
//! small palette of named styles up front and reuses them, mixing plain lines,
//! the builder, and inline tags the way a real tool would.
//!
//! ```bash
//! cargo run --example status_report
//! ```

use cli_core::{define_tag, out, parse, style, tag};

fn main() {
    // Define the program's vocabulary of styles once.
    define_tag("ok", style("").green().bold());
    define_tag("warn", style("").yellow().bold());
    define_tag("fail", style("").red().bold());
    define_tag("muted", style("").rgb(136, 136, 136));

    out(style("deploy: staging").bold().underline());
    out("");

    step("resolve dependencies", Status::Ok);
    step("compile (release)", Status::Ok);
    step("run test suite", Status::Warn);
    step("upload artifacts", Status::Ok);
    step("smoke test", Status::Fail);

    out("");
    parse("<b>result</b>: <c=red>1 step failed</c> — see <c=#3b82f6><u>logs/smoke.txt</u></c>");
    out(tag("muted").render_with("finished in 48.2s"));
}

enum Status {
    Ok,
    Warn,
    Fail,
}

/// Print one status line, reusing the named styles defined in `main`.
fn step(label: &str, status: Status) {
    let name = match status {
        Status::Ok => "ok",
        Status::Warn => "warn",
        Status::Fail => "fail",
    };
    // Pad the text to a fixed width *before* styling it, so the visible columns
    // line up whether or not color (and its zero-width escape bytes) is applied.
    let marker = tag(name).render_with(&format!("{:<6}", format!("[{name}]")));
    out(format!("  {marker} {label}"));
}
