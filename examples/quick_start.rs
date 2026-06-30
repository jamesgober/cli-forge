//! The shortest useful tour: plain output, then one styled line.
//!
//! ```bash
//! cargo run --example quick_start
//! ```

use cli_core::{err, out, style};

fn main() {
    // The common case is one call with no ceremony.
    out("building project...");

    // Any `Display` value works, so formatting drops right in.
    let targets = 3;
    out(format!("compiled {targets} targets"));

    // Opt into color by passing a styled value; on a pipe or under NO_COLOR it
    // prints as plain text instead.
    out(style("done").green().bold());

    // Errors go to standard error through the same system.
    err(style("warning:").yellow().bold());
    err("  no tests were run");
}
