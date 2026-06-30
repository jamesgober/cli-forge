//! The load-bearing invariant of the output layer: the builder, the tag parser,
//! and the named registry render byte-identical output for the same intent.
//!
//! These tests drive the three paths at explicit color levels so the assertion
//! does not depend on the terminal the test happens to run under. Every sibling
//! crate in the cli collection relies on this property — a color produced one
//! way is the same on the wire as the same color produced another way.

#![allow(clippy::unwrap_used)]

use crate::registry::{define_tag, tag};
use crate::style::{StyleAttrs, style, write_styled};
use crate::tags::render;
use crate::terminal::ColorLevel;

/// Render an attribute set at an explicit level.
fn render_attrs(attrs: StyleAttrs, text: &str, level: ColorLevel) -> String {
    let mut s = String::new();
    write_styled(&mut s, attrs, text, level).unwrap();
    s
}

const LEVELS: [ColorLevel; 4] = [
    ColorLevel::None,
    ColorLevel::Ansi16,
    ColorLevel::Ansi256,
    ColorLevel::TrueColor,
];

#[test]
fn test_three_paths_identical_named_color_and_bold() {
    let text = "ERROR:";
    define_tag("xp-error", style("").red().bold());
    for level in LEVELS {
        let builder = render_attrs(style(text).red().bold().attrs(), text, level);
        let tags = render("<c=red><b>ERROR:</b></c>", level);
        let registry = render_attrs(tag("xp-error").attrs_or_empty(), text, level);
        assert_eq!(builder, tags, "builder vs tags at {level:?}");
        assert_eq!(builder, registry, "builder vs registry at {level:?}");
    }
}

#[test]
fn test_three_paths_identical_rgb() {
    let text = "ok";
    define_tag("xp-ok", style("").rgb(0, 200, 120));
    for level in LEVELS {
        let builder = render_attrs(style(text).rgb(0, 200, 120).attrs(), text, level);
        let tags = render("<c=0,200,120>ok</c>", level);
        let registry = render_attrs(tag("xp-ok").attrs_or_empty(), text, level);
        assert_eq!(builder, tags, "builder vs tags at {level:?}");
        assert_eq!(builder, registry, "builder vs registry at {level:?}");
    }
}

#[test]
fn test_three_paths_identical_hex_and_underline() {
    let text = "link";
    define_tag("xp-link", style("").hex("#88aaff").underline());
    for level in LEVELS {
        let builder = render_attrs(style(text).hex("#88aaff").underline().attrs(), text, level);
        let tags = render("<c=#88aaff><u>link</u></c>", level);
        let registry = render_attrs(tag("xp-link").attrs_or_empty(), text, level);
        assert_eq!(builder, tags, "builder vs tags at {level:?}");
        assert_eq!(builder, registry, "builder vs registry at {level:?}");
    }
}

#[test]
fn test_hex_and_rgb_are_the_same_intent() {
    // `#88aaff` and `rgb(136, 170, 255)` are the same color, so they render
    // identically at every level.
    let text = "x";
    for level in LEVELS {
        let from_hex = render_attrs(style(text).hex("#88aaff").attrs(), text, level);
        let from_rgb = render_attrs(style(text).rgb(136, 170, 255).attrs(), text, level);
        assert_eq!(from_hex, from_rgb, "hex vs rgb at {level:?}");
    }
}

#[test]
fn test_graceful_degradation_lowers_color_depth() {
    // A 24-bit color emits true-color bytes at TrueColor, a 256-cube index at
    // Ansi256, a basic code at Ansi16, and nothing at None.
    let text = "x";
    let attrs = style(text).rgb(255, 136, 0).attrs();

    assert_eq!(
        render_attrs(attrs, text, ColorLevel::TrueColor),
        "\x1b[38;2;255;136;0mx\x1b[0m"
    );
    assert!(render_attrs(attrs, text, ColorLevel::Ansi256).contains("\x1b[38;5;"));
    let basic = render_attrs(attrs, text, ColorLevel::Ansi16);
    assert!(basic.starts_with("\x1b[3") && basic.ends_with("mx\x1b[0m"));
    assert_eq!(render_attrs(attrs, text, ColorLevel::None), "x");
}
