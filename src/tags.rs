//! The tag-string styling path.
//!
//! [`parse`] takes a string with inline markup and prints the styled result. The
//! markup is deliberately small:
//!
//! - `<b>…</b>` — bold
//! - `<u>…</u>` — underline
//! - `<c=VALUE>…</c>` — foreground color, where `VALUE` is a named color, a
//!   `#rrggbb` hex string, or an `r,g,b` triple
//! - `</>` — close the most recently opened tag, whatever it was
//!
//! Tags nest. Anything that does not match a known tag is emitted verbatim, so
//! `parse` never rejects input — a stray `<` or an unknown `<tag>` simply prints
//! as written. The rendered bytes are identical to the equivalent
//! [`style`](crate::style) builder output for the same intent.

use crate::style::{StyleAttrs, write_styled};
use crate::terminal::{self, ColorLevel};

/// Which kind of tag opened a stack frame, so a close pops the matching one.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TagKind {
    Color,
    Bold,
    Underline,
}

/// One open tag: the kind, and the attributes that were in effect before it, to
/// restore on close.
struct Frame {
    kind: TagKind,
    previous: StyleAttrs,
}

/// The effect of a single recognized tag.
enum Action {
    OpenColor(Option<crate::color::Color>),
    OpenBold,
    OpenUnderline,
    Close(TagKind),
    CloseAny,
}

/// Parse a tag string and print the styled result, followed by a newline.
///
/// Color depth matches the terminal detected for standard output; on a pipe, a
/// `NO_COLOR` environment, or a build without the `color` feature, the tags are
/// stripped and only their text is printed.
///
/// # Examples
///
/// ```
/// use cli_core::parse;
///
/// parse("<c=red><b>ERROR:</b></c> <c=#ff8800>disk almost full</c>");
/// parse("plain text with a stray < prints fine");
/// ```
pub fn parse<S: AsRef<str>>(tags: S) {
    crate::out(render(tags.as_ref(), terminal::color_level()));
}

/// Render a tag string to a styled `String` at an explicit color level. This is
/// the core both [`parse`] and the cross-path equality tests drive.
pub(crate) fn render(input: &str, level: ColorLevel) -> String {
    let mut out = String::with_capacity(input.len());
    let mut current = StyleAttrs::EMPTY;
    let mut stack: Vec<Frame> = Vec::new();

    let bytes = input.as_bytes();
    let mut i = 0;
    let mut run_start = 0;

    while i < bytes.len() {
        // `<` is ASCII, so it never appears inside a multi-byte UTF-8 sequence;
        // scanning byte-by-byte for it stays on character boundaries.
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }

        if let Some(rel) = input[i + 1..].find('>') {
            let inner = &input[i + 1..i + 1 + rel];
            let after = i + 1 + rel + 1;
            if let Some(action) = parse_tag(inner) {
                // Flush the pending text run with the attributes in effect so
                // far. Skip empty runs so adjacent tags do not emit empty escape
                // pairs. Writing to a `String` is infallible.
                if run_start < i {
                    let _ = write_styled(&mut out, current, &input[run_start..i], level);
                }
                apply(action, &mut current, &mut stack);
                i = after;
                run_start = after;
                continue;
            }
        }

        // Not a recognized tag: leave the `<` as literal text and move on.
        i += 1;
    }

    if run_start < input.len() {
        let _ = write_styled(&mut out, current, &input[run_start..], level);
    }
    out
}

/// Classify the text between `<` and `>`. Returns `None` for anything that is
/// not a known tag, which the caller then treats as literal text.
fn parse_tag(inner: &str) -> Option<Action> {
    match inner.trim() {
        "b" => Some(Action::OpenBold),
        "u" => Some(Action::OpenUnderline),
        "/b" => Some(Action::Close(TagKind::Bold)),
        "/u" => Some(Action::Close(TagKind::Underline)),
        "/c" => Some(Action::Close(TagKind::Color)),
        "/" => Some(Action::CloseAny),
        other => other
            .strip_prefix("c=")
            .map(|value| Action::OpenColor(crate::color::Color::parse(value))),
    }
}

/// Apply a tag's effect to the running attribute state and the open-tag stack.
fn apply(action: Action, current: &mut StyleAttrs, stack: &mut Vec<Frame>) {
    match action {
        Action::OpenColor(color) => {
            stack.push(Frame {
                kind: TagKind::Color,
                previous: *current,
            });
            // An unparseable color value opens a balanced span that inherits the
            // surrounding color rather than failing the parse.
            if let Some(color) = color {
                current.fg = Some(color);
            }
        }
        Action::OpenBold => {
            stack.push(Frame {
                kind: TagKind::Bold,
                previous: *current,
            });
            current.bold = true;
        }
        Action::OpenUnderline => {
            stack.push(Frame {
                kind: TagKind::Underline,
                previous: *current,
            });
            current.underline = true;
        }
        Action::Close(kind) => {
            // Only unwind when the innermost open tag matches; mismatched or
            // unbalanced closes are ignored.
            if stack.last().is_some_and(|frame| frame.kind == kind) {
                if let Some(frame) = stack.pop() {
                    *current = frame.previous;
                }
            }
        }
        Action::CloseAny => {
            if let Some(frame) = stack.pop() {
                *current = frame.previous;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::color::Color;
    use crate::style::style;

    #[test]
    fn test_nested_tags_render_runs_in_order() {
        let got = render(
            "<c=red><b>ERROR:</b></c> <c=#ff8800>disk almost full</c>",
            ColorLevel::TrueColor,
        );
        // Two styled spans with a plain space between them.
        let expected = "\x1b[1;31mERROR:\x1b[0m \x1b[38;2;255;136;0mdisk almost full\x1b[0m";
        assert_eq!(got, expected);
    }

    #[test]
    fn test_plain_input_passes_through() {
        assert_eq!(render("just text", ColorLevel::TrueColor), "just text");
    }

    #[test]
    fn test_unknown_tag_is_literal() {
        assert_eq!(render("a <unknown> b", ColorLevel::Ansi16), "a <unknown> b");
        assert_eq!(render("less < than", ColorLevel::Ansi16), "less < than");
        assert_eq!(
            render("open <b without close", ColorLevel::Ansi16),
            "open <b without close"
        );
    }

    #[test]
    fn test_generic_close_pops_innermost() {
        // `</>` closes underline, then `</>` closes bold, so `c` renders plain.
        assert_eq!(
            render("<b>a<u>b</></>c", ColorLevel::Ansi16),
            "\x1b[1ma\x1b[0m\x1b[1;4mb\x1b[0mc"
        );
    }

    #[test]
    fn test_mismatched_close_is_ignored() {
        // `</c>` does not match the open `<b>`, so it is dropped and bold stays
        // active; the close still ends one run and starts the next.
        assert_eq!(
            render("<b>x</c>y", ColorLevel::Ansi16),
            "\x1b[1mx\x1b[0m\x1b[1my\x1b[0m"
        );
    }

    #[test]
    fn test_invalid_color_inherits_surrounding() {
        // The inner `<c=bogus>` keeps the outer red rather than dropping color.
        assert_eq!(
            render("<c=red>a<c=bogus>b</c>c</c>", ColorLevel::Ansi16),
            "\x1b[31ma\x1b[0m\x1b[31mb\x1b[0m\x1b[31mc\x1b[0m"
        );
    }

    #[test]
    fn test_tag_matches_builder_for_same_intent() {
        // The named-color path: a `<c=red><b>` run equals the builder's red+bold.
        let via_tags = render("<c=red><b>ALERT</b></c>", ColorLevel::Ansi256);
        let mut via_builder = String::new();
        write_styled(
            &mut via_builder,
            style("ALERT").red().bold().attrs(),
            "ALERT",
            ColorLevel::Ansi256,
        )
        .unwrap();
        assert_eq!(via_tags, via_builder);
    }

    #[test]
    fn test_color_triple_in_tag() {
        assert_eq!(
            render("<c=0,200,120>ok</c>", ColorLevel::TrueColor),
            "\x1b[38;2;0;200;120mok\x1b[0m"
        );
    }

    #[test]
    fn test_color_round_trips_to_expected_variant() {
        // Sanity check that the tag parser and the builder agree on the value.
        assert_eq!(Color::parse("red"), Some(Color::Red));
    }
}

#[cfg(test)]
mod proptests {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        /// No input — however adversarial its tags — may panic the parser.
        #[test]
        fn test_render_never_panics(input in ".*") {
            let _ = render(&input, ColorLevel::TrueColor);
            let _ = render(&input, ColorLevel::None);
        }

        /// Text without a `<` carries no tags, so it passes through byte-for-byte
        /// at every level.
        #[test]
        fn test_tagless_text_is_unchanged(input in "[^<]*") {
            prop_assert_eq!(render(&input, ColorLevel::TrueColor), input.clone());
            prop_assert_eq!(render(&input, ColorLevel::None), input);
        }
    }
}
