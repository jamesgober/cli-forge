//! Cross-path equality: the same intent renders to the same bytes.
//!
//! cli-forge offers four ways to say "red and bold" — the [`Style`] builder,
//! inline [`markup`](crate::markup), a [`named`](crate::named) style from the
//! registry, and a [`Theme`] level. That is only safe if they are genuinely one
//! system underneath: a program that mixes them, or a sibling crate that renders
//! through one while the host program uses another, must not produce two
//! different-looking reds. These tests pin that invariant down at every colour
//! depth, which is also where a divergence would most likely hide — a path that
//! forgot to downgrade, or that emitted its SGR parameters in a different order.
//!
//! Every assertion renders at an explicit [`ColorLevel`] rather than a detected
//! one, so the results do not depend on whether the test harness happens to be
//! attached to a terminal.

use crate::shim::{String, ToString};
use crate::style::Style;
use crate::tags::render;
use crate::terminal::ColorLevel;
use crate::theme::{Glyphs, Level, Theme};

/// A builder method under test, named by the markup tag that must match it.
type Case = (&'static str, fn(Style) -> Style);

/// Every depth that emits escape sequences, plus the one that emits none.
const LEVELS: [ColorLevel; 4] = [
    ColorLevel::None,
    ColorLevel::Ansi16,
    ColorLevel::Ansi256,
    ColorLevel::TrueColor,
];

/// Store `style` under `name` and render `text` back through the registry.
///
/// Reports `None` in a build without the registry, which needs a process-global
/// store and so needs `std`. The remaining three paths are still compared, so
/// this file keeps its value in a `no_std` build rather than being switched off
/// wholesale.
fn via_registry(name: &str, style: &Style, text: &str, level: ColorLevel) -> Option<String> {
    #[cfg(feature = "std")]
    {
        crate::registry::define(name, style.clone());
        Some(
            crate::registry::named(name)
                .paint_at(text, level)
                .to_string(),
        )
    }
    #[cfg(not(feature = "std"))]
    {
        let _ = (name, style, text, level);
        None
    }
}

#[test]
fn test_named_colour_renders_identically_on_every_path() {
    let text = "ALERT";
    let style = Style::new().red().bold();

    for level in LEVELS {
        let builder = style.paint_at(text, level).to_string();
        let markup = render("<c=red><b>ALERT</b></c>", level);
        assert_eq!(builder, markup, "builder vs markup at {level:?}");

        if let Some(registry) = via_registry("xpath-alert", &style, text, level) {
            assert_eq!(builder, registry, "builder vs registry at {level:?}");
        }
    }
}

#[test]
fn test_exact_colour_renders_identically_on_every_path() {
    let text = "ok";
    let style = Style::new().rgb(0, 200, 120);

    for level in LEVELS {
        let builder = style.paint_at(text, level).to_string();
        let markup = render("<c=0,200,120>ok</c>", level);
        let hex = Style::new()
            .hex("#00c878")
            .paint_at(text, level)
            .to_string();

        assert_eq!(builder, markup, "builder vs markup at {level:?}");
        // The two ways of naming the same 24-bit value must agree, including
        // after being downgraded.
        assert_eq!(builder, hex, "rgb vs hex at {level:?}");

        if let Some(registry) = via_registry("xpath-ok", &style, text, level) {
            assert_eq!(builder, registry, "builder vs registry at {level:?}");
        }
    }
}

#[test]
fn test_palette_index_renders_identically_on_every_path() {
    let text = "warn";
    let style = Style::new().ansi(208);

    for level in LEVELS {
        let builder = style.paint_at(text, level).to_string();
        let markup = render("<c=208>warn</c>", level);
        assert_eq!(builder, markup, "builder vs markup at {level:?}");

        if let Some(registry) = via_registry("xpath-warn", &style, text, level) {
            assert_eq!(builder, registry, "builder vs registry at {level:?}");
        }
    }
}

#[test]
fn test_background_and_attributes_render_identically() {
    let text = "LINK";

    for level in LEVELS {
        let builder = Style::new()
            .on_hex("#3b82f6")
            .underline()
            .paint_at(text, level)
            .to_string();
        let markup = render("<bg=#3b82f6><u>LINK</u></bg>", level);
        assert_eq!(builder, markup, "at {level:?}");
    }
}

#[test]
fn test_every_attribute_agrees_between_builder_and_markup() {
    // One tag per attribute, paired with the builder method that must match it.
    let cases: [Case; 6] = [
        ("<b>x</b>", Style::bold),
        ("<d>x</d>", Style::dim),
        ("<i>x</i>", Style::italic),
        ("<u>x</u>", Style::underline),
        ("<s>x</s>", Style::strike),
        ("<r>x</r>", Style::reverse),
    ];
    for (tags, method) in cases {
        for level in LEVELS {
            assert_eq!(
                method(Style::new()).paint_at("x", level).to_string(),
                render(tags, level),
                "{tags} at {level:?}"
            );
        }
    }
}

#[test]
fn test_every_colour_name_agrees_between_builder_and_markup() {
    // The full sixteen, so a bright variant cannot drift from its tag spelling.
    let cases: [Case; 16] = [
        ("black", Style::black),
        ("red", Style::red),
        ("green", Style::green),
        ("yellow", Style::yellow),
        ("blue", Style::blue),
        ("magenta", Style::magenta),
        ("cyan", Style::cyan),
        ("white", Style::white),
        ("bright_black", Style::bright_black),
        ("bright_red", Style::bright_red),
        ("bright_green", Style::bright_green),
        ("bright_yellow", Style::bright_yellow),
        ("bright_blue", Style::bright_blue),
        ("bright_magenta", Style::bright_magenta),
        ("bright_cyan", Style::bright_cyan),
        ("bright_white", Style::bright_white),
    ];
    for (name, method) in cases {
        let tags = crate::shim::format!("<c={name}>x</c>");
        for level in LEVELS {
            assert_eq!(
                method(Style::new()).paint_at("x", level).to_string(),
                render(&tags, level),
                "{name} at {level:?}"
            );
        }
    }
}

#[test]
fn test_theme_level_renders_through_the_same_core() {
    // A theme is not a fourth renderer: it composes a glyph with a style and
    // hands the result to the same primitive the builder uses.
    let style = Style::new().bright_green().bold();
    let theme = Theme::new()
        .set_glyphs(Glyphs::Unicode)
        .set(Level::Success, style.clone(), "✓");

    for level in LEVELS {
        let via_theme = theme.render_at(Level::Success, "done", level);
        let via_builder = style.paint_at("✓ done", level).to_string();
        assert_eq!(via_theme, via_builder, "at {level:?}");
    }
}

#[test]
fn test_decoration_is_part_of_the_style_not_the_call_site() {
    // A named style carrying a prefix and a width must render exactly as the
    // same style spelled out inline — the property that makes the registry a
    // real substitute for hand-built markers.
    let inline = Style::new().green().prefix("✓ ").pad_to(12);

    for level in LEVELS {
        if let Some(registry) = via_registry("xpath-step", &inline, "compile", level) {
            assert_eq!(
                registry,
                inline.paint_at("compile", level).to_string(),
                "at {level:?}"
            );
        }
    }
}

#[test]
fn test_no_colour_yields_the_plain_text_on_every_path() {
    let style = Style::new().red().bold();

    assert_eq!(style.paint_at("text", ColorLevel::None).to_string(), "text");
    assert_eq!(render("<c=red><b>text</b></c>", ColorLevel::None), "text");
    assert_eq!(
        Theme::plain().render_at(Level::Error, "text", ColorLevel::None),
        "text"
    );
    if let Some(registry) = via_registry("xpath-plain", &style, "text", ColorLevel::None) {
        assert_eq!(registry, "text");
    }
}

#[test]
fn test_downgrade_is_consistent_across_paths() {
    // An exact colour that has to be approximated must be approximated the same
    // way whichever path asked for it, or a program that mixes paths would show
    // two different "brand" colours on a 16-colour terminal.
    for (r, g, b) in [(255, 136, 0), (59, 130, 246), (0, 0, 0), (118, 118, 118)] {
        let tags = crate::shim::format!("<c={r},{g},{b}>x</c>");
        for level in [ColorLevel::Ansi16, ColorLevel::Ansi256] {
            assert_eq!(
                Style::new().rgb(r, g, b).paint_at("x", level).to_string(),
                render(&tags, level),
                "rgb({r},{g},{b}) at {level:?}"
            );
        }
    }
}
