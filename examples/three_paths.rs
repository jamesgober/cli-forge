//! The same styled line produced three ways — builder, inline tags, and a named
//! registry entry. All three render to identical bytes for the same intent; the
//! choice between them is ergonomic, not visual.
//!
//! ```bash
//! cargo run --example three_paths
//! ```

use cli_core::{define_tag, out, parse, style, tag};

fn main() {
    out("Three ways to say the same thing:");
    out("");

    // 1. The builder. Chain methods; the result is `Display`, so it drops into
    //    `out` directly. Best when the style is computed or one-off.
    out(style("ERROR: build failed").red().bold());

    // 2. Inline tags. The whole line is one string with markup. Best when the
    //    text and its styling are written together, like a template.
    parse("<c=red><b>ERROR: build failed</b></c>");

    // 3. A named style. Define the look once, reuse it everywhere by name. Best
    //    when the same style recurs across a program — define it in one module,
    //    recall it in another.
    define_tag("error", style("").red().bold());
    out(tag("error").render_with("ERROR: build failed"));

    out("");
    out("Tags nest and mix freely:");
    parse("<b>summary</b>: <c=green>12 passed</c>, <c=red>1 failed</c>, <c=#888888>3 skipped</c>");
}
