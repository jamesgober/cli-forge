//! The markup path: styling written inline in the text.
//!
//! [`markup`] turns a string with inline tags into a styled string. It is the
//! right path when a line's styling varies run by run and spelling it out with
//! the builder would bury the sentence in method calls:
//!
//! ```
//! # #[cfg(feature = "std")] fn main() {
//! use cli_forge::{markup, out};
//!
//! out(markup("<c=red><b>ERROR:</b></c> <c=#ff8800>disk almost full</c>"));
//! # }
//! # #[cfg(not(feature = "std"))] fn main() {}
//! ```
//!
//! ## The tags
//!
//! | Tag | Effect |
//! |---|---|
//! | `<b>` | bold |
//! | `<d>` | dim |
//! | `<i>` | italic |
//! | `<u>` | underline |
//! | `<s>` | strikethrough |
//! | `<r>` | reverse video |
//! | `<c=VALUE>` | foreground colour |
//! | `<bg=VALUE>` | background colour |
//! | `<link=URL>` | hyperlink |
//! | `</x>` | close that tag |
//! | `</>` | close the innermost open tag, whatever it was |
//! | `<<` | a literal `<` |
//!
//! `VALUE` is anything [`Color::parse`] accepts: a name (`red`, `bright_red`), a
//! `#rrggbb` or `#rgb` hex string, an `r,g,b` triple, or a `0..=255` palette
//! index.
//!
//! ## It never fails
//!
//! Markup is text a program prints, so a mistake in it must not take the program
//! down or swallow the message. Anything unrecognised is emitted verbatim: a
//! stray `<`, an unknown `<tag>`, an unterminated `<b`, a mismatched close, or a
//! colour value that is not a colour all print as written, and the surrounding
//! text always survives. Nesting is capped at 64 frames so that markup arriving
//! from somewhere untrusted cannot grow the parser's state without bound; past
//! the cap, further opening tags are treated as literal text.
//!
//! The bytes this produces are identical to the equivalent [`Style`] builder
//! output for the same intent — a property the cross-path tests assert directly,
//! because it is what lets the two be mixed in one program.

use crate::color::Color;
use crate::shim::{String, Vec};
use crate::style::{StyleAttrs, write_styled};
use crate::terminal::{self, ColorLevel, Stream};

/// The deepest nesting [`markup`] will track.
///
/// Generous for hand-written markup — real styling nests two or three deep — and
/// low enough that hostile input cannot turn a print call into unbounded memory
/// growth.
pub(crate) const MAX_DEPTH: usize = 64;

/// Which kind of tag opened a stack frame, so a close pops the matching one.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TagKind {
    Color,
    Background,
    Bold,
    Dim,
    Italic,
    Underline,
    Strike,
    Reverse,
    Link,
}

/// One open tag: its kind, and the attributes in effect before it, to restore on
/// close.
#[derive(Debug)]
struct Frame {
    kind: TagKind,
    previous: StyleAttrs,
}

/// The effect of a single recognised tag.
#[derive(Debug)]
enum Action {
    /// Open a colour span. `None` is a value that did not parse, which opens a
    /// balanced span inheriting the surrounding colour.
    OpenColor(Option<Color>, bool),
    /// Open an attribute span.
    OpenAttribute(TagKind),
    /// Open a hyperlink spanning the given URL.
    OpenLink(usize, usize),
    /// Close the innermost frame if it is of this kind.
    Close(TagKind),
    /// Close the innermost frame, whatever it is.
    CloseAny,
}

impl Action {
    /// Whether this action would push a frame, and so is subject to the nesting
    /// cap.
    const fn opens(&self) -> bool {
        matches!(
            self,
            Action::OpenColor(..) | Action::OpenAttribute(_) | Action::OpenLink(..)
        )
    }
}

/// Render markup to a styled string at the depth detected for standard output.
///
/// Returns an owned `String`, which is what makes this composable: the result can
/// be printed, written to a file, measured with
/// [`text::width`](crate::text::width), or dropped into a table cell. On a pipe,
/// under `NO_COLOR`, or in a build without the `color` feature, the tags are
/// stripped and only their text remains.
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "std")] fn main() {
/// use cli_forge::{markup, out, text};
///
/// out(markup("<c=red><b>ERROR:</b></c> disk almost full"));
///
/// // Whatever the terminal, the visible text is the same.
/// let line = markup("<c=green>ok</c>");
/// assert_eq!(text::strip(&line), "ok");
///
/// // Unrecognised markup prints as written rather than failing.
/// assert_eq!(markup("a <unknown> b"), "a <unknown> b");
/// assert_eq!(markup("2 << 3"), "2 < 3");
/// # }
/// # #[cfg(not(feature = "std"))] fn main() {}
/// ```
#[must_use]
pub fn markup<S: AsRef<str>>(tags: S) -> String {
    render(tags.as_ref(), terminal::level(Stream::Stdout))
}

/// Render markup at an explicit colour depth.
///
/// Use this for a destination whose capability is known independently of the
/// detected terminal — standard error when the two streams differ, a log file, or
/// a test that needs deterministic bytes.
///
/// # Examples
///
/// ```
/// use cli_forge::{markup_at, ColorLevel};
///
/// assert_eq!(
///     markup_at("<c=red>x</c>", ColorLevel::Ansi16),
///     "\x1b[31mx\x1b[0m"
/// );
/// assert_eq!(markup_at("<c=red>x</c>", ColorLevel::None), "x");
/// ```
#[must_use]
pub fn markup_at<S: AsRef<str>>(tags: S, level: ColorLevel) -> String {
    render(tags.as_ref(), level)
}

/// Render markup at `level`. The core both public entry points drive.
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

        // `<<` is the escape for a literal `<`, so markup can describe markup.
        if bytes.get(i + 1).copied() == Some(b'<') {
            flush(&mut out, current, &input[run_start..i], level);
            // Styled like the run it sits in, not emitted bare: it is content.
            flush(&mut out, current, "<", level);
            i += 2;
            run_start = i;
            continue;
        }

        if let Some(relative) = input[i + 1..].find('>') {
            let inner_start = i + 1;
            let inner_end = inner_start + relative;
            let after = inner_end + 1;
            if let Some(action) = parse_tag(input, inner_start, inner_end) {
                // Past the nesting cap, an opening tag is not tracked — so it is
                // left as the literal text it looks like, which keeps the content
                // intact instead of silently dropping it.
                if action.opens() && stack.len() >= MAX_DEPTH {
                    i += 1;
                    continue;
                }
                // Flush the pending text run with the attributes in effect so
                // far, then apply the tag.
                flush(&mut out, current, &input[run_start..i], level);
                apply(action, input, &mut out, &mut current, &mut stack, level);
                i = after;
                run_start = after;
                continue;
            }
        }

        // Not a recognised tag: leave the `<` as literal text and move on.
        i += 1;
    }

    flush(&mut out, current, &input[run_start..], level);
    // An unbalanced open tag must not leak styling into whatever is printed
    // next, so anything still open is closed here.
    close_remaining(&mut out, &stack);
    out
}

/// Write one text run, skipping empty ones so that adjacent tags do not emit
/// empty escape pairs. Writing to a `String` is infallible.
fn flush(out: &mut String, attrs: StyleAttrs, run: &str, level: ColorLevel) {
    if !run.is_empty() {
        let _ = write_styled(out, attrs, run, level);
    }
}

/// Close any hyperlinks left open at the end of the input.
///
/// Styling needs no closing here because every run is written as a self-contained
/// styled span, but a hyperlink's opening sequence stays in effect until its
/// closing one, so an unbalanced `<link=…>` would make the rest of the terminal's
/// output clickable.
fn close_remaining(out: &mut String, stack: &[Frame]) {
    for frame in stack.iter().rev() {
        if frame.kind == TagKind::Link {
            out.push_str(LINK_CLOSE);
        }
    }
}

/// The sequence that ends an OSC 8 hyperlink.
const LINK_CLOSE: &str = "\x1b]8;;\x07";

/// Classify the text between `<` and `>`, given as byte offsets into `input`.
///
/// Returns `None` for anything that is not a known tag, which the caller then
/// treats as literal text. Offsets rather than a slice are returned for the link
/// case so the URL can be read back out without another allocation.
fn parse_tag(input: &str, start: usize, end: usize) -> Option<Action> {
    let inner = input[start..end].trim();
    match inner {
        "b" => return Some(Action::OpenAttribute(TagKind::Bold)),
        "d" => return Some(Action::OpenAttribute(TagKind::Dim)),
        "i" => return Some(Action::OpenAttribute(TagKind::Italic)),
        "u" => return Some(Action::OpenAttribute(TagKind::Underline)),
        "s" => return Some(Action::OpenAttribute(TagKind::Strike)),
        "r" => return Some(Action::OpenAttribute(TagKind::Reverse)),
        "/b" => return Some(Action::Close(TagKind::Bold)),
        "/d" => return Some(Action::Close(TagKind::Dim)),
        "/i" => return Some(Action::Close(TagKind::Italic)),
        "/u" => return Some(Action::Close(TagKind::Underline)),
        "/s" => return Some(Action::Close(TagKind::Strike)),
        "/r" => return Some(Action::Close(TagKind::Reverse)),
        "/c" => return Some(Action::Close(TagKind::Color)),
        "/bg" => return Some(Action::Close(TagKind::Background)),
        "/link" => return Some(Action::Close(TagKind::Link)),
        "/" => return Some(Action::CloseAny),
        _ => {}
    }
    if let Some(value) = inner.strip_prefix("c=") {
        return Some(Action::OpenColor(Color::parse(value), false));
    }
    if let Some(value) = inner.strip_prefix("bg=") {
        return Some(Action::OpenColor(Color::parse(value), true));
    }
    if inner.starts_with("link=") {
        // The URL is kept as offsets into the untrimmed inner text so that no
        // copy is needed; leading whitespace inside the tag is already excluded
        // because `trim` only shrinks the range.
        let offset = input[start..end].find("link=")? + "link=".len();
        return Some(Action::OpenLink(start + offset, end));
    }
    None
}

/// Apply a tag's effect to the running attribute state, the open-tag stack, and —
/// for hyperlinks, which are not SGR styling — the output directly.
fn apply(
    action: Action,
    input: &str,
    out: &mut String,
    current: &mut StyleAttrs,
    stack: &mut Vec<Frame>,
    level: ColorLevel,
) {
    match action {
        Action::OpenColor(color, background) => {
            push(stack, TagKind::Color, *current, background);
            // A value that did not parse opens a balanced span inheriting the
            // surrounding colour, rather than failing the render.
            if let Some(color) = color {
                if background {
                    current.bg = Some(color);
                } else {
                    current.fg = Some(color);
                }
            }
        }
        Action::OpenAttribute(kind) => {
            push(stack, kind, *current, false);
            current.flags |= flag_of(kind);
        }
        Action::OpenLink(start, end) => {
            push(stack, TagKind::Link, *current, false);
            // A hyperlink is an OSC sequence, not an SGR parameter, so it is
            // written straight through rather than folded into the attributes.
            // At `None` it is dropped with the rest of the styling.
            if !level.is_none() {
                out.push_str("\x1b]8;;");
                out.push_str(&input[start..end]);
                out.push('\x07');
            }
        }
        Action::Close(kind) => {
            // Only unwind when the innermost open tag matches; a mismatched or
            // unbalanced close is ignored.
            let matches = stack.last().is_some_and(|frame| {
                frame.kind == kind
                    // A `</c>` closes a foreground span and `</bg>` a background
                    // one; both are recorded as `Color` frames, so the kind alone
                    // cannot tell them apart. Treating them as interchangeable is
                    // the forgiving choice, and matches `</>`.
                    || (kind == TagKind::Background && frame.kind == TagKind::Color)
                    || (kind == TagKind::Color && frame.kind == TagKind::Background)
            });
            if matches {
                pop(stack, out, current, level);
            }
        }
        Action::CloseAny => pop(stack, out, current, level),
    }
}

/// Push a frame recording the attributes to restore when it closes.
///
/// The caller has already refused anything past [`MAX_DEPTH`], so this always
/// succeeds and the stack cannot grow without bound.
fn push(stack: &mut Vec<Frame>, kind: TagKind, previous: StyleAttrs, background: bool) {
    let kind = if background && kind == TagKind::Color {
        TagKind::Background
    } else {
        kind
    };
    stack.push(Frame { kind, previous });
}

/// Pop the innermost frame, restoring the attributes it saved and closing a
/// hyperlink if that is what it was.
fn pop(stack: &mut Vec<Frame>, out: &mut String, current: &mut StyleAttrs, level: ColorLevel) {
    if let Some(frame) = stack.pop() {
        *current = frame.previous;
        if frame.kind == TagKind::Link && !level.is_none() {
            out.push_str(LINK_CLOSE);
        }
    }
}

/// The attribute bit a tag kind sets. Colour and link frames set none.
fn flag_of(kind: TagKind) -> u8 {
    match kind {
        TagKind::Bold => crate::style::flags::BOLD,
        TagKind::Dim => crate::style::flags::DIM,
        TagKind::Italic => crate::style::flags::ITALIC,
        TagKind::Underline => crate::style::flags::UNDERLINE,
        TagKind::Strike => crate::style::flags::STRIKE,
        TagKind::Reverse => crate::style::flags::REVERSE,
        TagKind::Color | TagKind::Background | TagKind::Link => 0,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::shim::ToString;
    use crate::style::Style;
    use crate::text;

    #[test]
    fn test_nested_tags_render_runs_in_order() {
        let got = render(
            "<c=red><b>ERROR:</b></c> <c=#ff8800>disk almost full</c>",
            ColorLevel::TrueColor,
        );
        // Two styled spans with a plain space between them.
        assert_eq!(
            got,
            "\x1b[1;31mERROR:\x1b[0m \x1b[38;2;255;136;0mdisk almost full\x1b[0m"
        );
    }

    #[test]
    fn test_plain_input_passes_through() {
        assert_eq!(render("just text", ColorLevel::TrueColor), "just text");
        assert_eq!(render("", ColorLevel::TrueColor), "");
    }

    #[test]
    fn test_unrecognised_markup_is_literal() {
        for input in [
            "a <unknown> b",
            "less < than",
            "open <b without close",
            "<>",
            "</nope>",
            "<c red>x",
            "<link>x",
        ] {
            let rendered = render(input, ColorLevel::Ansi16);
            assert_eq!(text::strip(&rendered), input, "{input:?} changed");
        }
    }

    #[test]
    fn test_double_angle_escapes_a_literal_bracket() {
        assert_eq!(render("2 << 3", ColorLevel::Ansi16), "2 < 3");
        assert_eq!(render("<<b>", ColorLevel::Ansi16), "<b>");
        // And it still works inside a styled span.
        assert_eq!(
            render("<b><<b></b>", ColorLevel::Ansi16),
            "\x1b[1m<\x1b[0m\x1b[1mb>\x1b[0m"
        );
    }

    #[test]
    fn test_every_attribute_tag_has_an_effect() {
        let cases = [
            ("<b>x</b>", "1"),
            ("<d>x</d>", "2"),
            ("<i>x</i>", "3"),
            ("<u>x</u>", "4"),
            ("<s>x</s>", "9"),
            ("<r>x</r>", "7"),
        ];
        for (input, parameter) in cases {
            assert_eq!(
                render(input, ColorLevel::Ansi16),
                crate::shim::format!("\x1b[{parameter}mx\x1b[0m"),
                "{input}"
            );
        }
    }

    #[test]
    fn test_background_tag() {
        assert_eq!(
            render("<bg=white>x</bg>", ColorLevel::Ansi16),
            "\x1b[47mx\x1b[0m"
        );
        assert_eq!(
            render("<c=red><bg=white>x</bg></c>", ColorLevel::Ansi16),
            "\x1b[31;47mx\x1b[0m"
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
        // `</u>` does not match the open `<b>`, so it is dropped and bold stays
        // active; the close still ends one run and starts the next.
        assert_eq!(
            render("<b>x</u>y", ColorLevel::Ansi16),
            "\x1b[1mx\x1b[0m\x1b[1my\x1b[0m"
        );
    }

    #[test]
    fn test_unparseable_colour_inherits_the_surrounding_one() {
        assert_eq!(
            render("<c=red>a<c=bogus>b</c>c</c>", ColorLevel::Ansi16),
            "\x1b[31ma\x1b[0m\x1b[31mb\x1b[0m\x1b[31mc\x1b[0m"
        );
        // An empty value is just another value that is not a colour, so it opens
        // a balanced span the same way rather than becoming literal text.
        assert_eq!(
            render("<c=red>a<c=>b</c>c</c>", ColorLevel::Ansi16),
            "\x1b[31ma\x1b[0m\x1b[31mb\x1b[0m\x1b[31mc\x1b[0m"
        );
    }

    #[test]
    fn test_colour_value_forms() {
        assert_eq!(
            render("<c=0,200,120>ok</c>", ColorLevel::TrueColor),
            "\x1b[38;2;0;200;120mok\x1b[0m"
        );
        assert_eq!(
            render("<c=208>ok</c>", ColorLevel::Ansi256),
            "\x1b[38;5;208mok\x1b[0m"
        );
        assert_eq!(
            render("<c=bright_red>ok</c>", ColorLevel::Ansi16),
            "\x1b[91mok\x1b[0m"
        );
        assert_eq!(
            render("<c=#f80>ok</c>", ColorLevel::TrueColor),
            "\x1b[38;2;255;136;0mok\x1b[0m"
        );
    }

    #[test]
    fn test_hyperlink_wraps_its_text() {
        let rendered = render("<link=https://example.com>docs</link>", ColorLevel::Ansi16);
        assert_eq!(rendered, "\x1b]8;;https://example.com\x07docs\x1b]8;;\x07");
        assert_eq!(text::width(&rendered), 4);
    }

    #[test]
    fn test_hyperlink_is_dropped_without_colour() {
        assert_eq!(
            render("<link=https://example.com>docs</link>", ColorLevel::None),
            "docs"
        );
    }

    #[test]
    fn test_unclosed_hyperlink_is_closed_at_the_end() {
        // Otherwise every later line of terminal output would stay clickable.
        let rendered = render("<link=https://x>docs", ColorLevel::Ansi16);
        assert!(rendered.ends_with(LINK_CLOSE), "link leaked: {rendered:?}");
    }

    #[test]
    fn test_nesting_is_capped_and_still_lossless() {
        let input = "<b>".repeat(MAX_DEPTH + 50);
        let rendered = render(&input, ColorLevel::Ansi16);
        // Tags past the cap become literal text, so nothing is silently lost.
        assert!(text::strip(&rendered).contains("<b>"));
    }

    #[test]
    fn test_markup_matches_the_builder_for_the_same_intent() {
        // The property that lets the two paths be mixed: same intent, same bytes.
        for level in [
            ColorLevel::Ansi16,
            ColorLevel::Ansi256,
            ColorLevel::TrueColor,
        ] {
            assert_eq!(
                render("<c=red><b>ALERT</b></c>", level),
                Style::new()
                    .red()
                    .bold()
                    .paint_at("ALERT", level)
                    .to_string(),
                "{level:?}"
            );
            assert_eq!(
                render("<bg=#3b82f6><u>LINK</u></bg>", level),
                Style::new()
                    .on_hex("#3b82f6")
                    .underline()
                    .paint_at("LINK", level)
                    .to_string(),
                "{level:?}"
            );
        }
    }

    #[test]
    fn test_no_colour_strips_every_tag() {
        let input = "<c=red><b>a</b></c><bg=blue>b</bg><link=https://x>c</link>";
        assert_eq!(render(input, ColorLevel::None), "abc");
    }

    #[test]
    fn test_a_raw_escape_in_the_input_is_passed_through() {
        // Markup is a styling language, not a sanitiser: an escape byte the
        // caller supplied is content, and rewriting it would corrupt text that
        // legitimately contains one. A caller printing text from an untrusted
        // source is the one who must decide, with `text::sanitize`.
        assert_eq!(render("\x1b[2J", ColorLevel::None), "\x1b[2J");
        assert_eq!(
            render(&text::sanitize("\x1b[2J"), ColorLevel::None),
            "^[[2J"
        );
    }

    #[test]
    fn test_multibyte_text_survives_scanning() {
        // The byte scan for `<` must not land inside a multi-byte character.
        assert_eq!(render("日本語", ColorLevel::TrueColor), "日本語");
        assert_eq!(
            text::strip(&render("<b>日本語</b>é", ColorLevel::Ansi16)),
            "日本語é"
        );
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
            for level in [ColorLevel::None, ColorLevel::Ansi16, ColorLevel::TrueColor] {
                let _ = render(&input, level);
            }
        }

        /// Text without a `<` carries no tags, so it passes through
        /// byte-for-byte at every depth.
        #[test]
        fn test_tagless_text_is_unchanged(input in "[^<]*") {
            prop_assert_eq!(render(&input, ColorLevel::TrueColor), input.clone());
            prop_assert_eq!(render(&input, ColorLevel::None), input);
        }

        /// Rendering with colour off yields exactly the visible text of
        /// rendering with colour on: the two paths agree on what is content.
        ///
        /// Escape bytes are excluded from the input because markup is not a
        /// sanitiser: one the caller supplied is literal text and is passed
        /// through, so stripping would remove something rendering kept. See
        /// `test_a_raw_escape_in_the_input_is_passed_through`.
        #[test]
        fn test_stripping_matches_rendering_without_colour(input in "[^\\x1b]*") {
            let plain = render(&input, ColorLevel::None);
            let styled = render(&input, ColorLevel::TrueColor);
            let stripped = crate::text::strip(&styled);
            prop_assert_eq!(stripped.as_ref(), plain.as_str());
        }

        /// Styling never leaks past the end of the output.
        #[test]
        fn test_no_styling_is_left_open(input in "(<b>|</b>|<c=red>|</c>|<link=u>|</link>|x){0,40}") {
            let rendered = render(&input, ColorLevel::TrueColor);
            let opens = rendered.matches("\x1b]8;;").count();
            // Every hyperlink opening sequence has a closing one.
            prop_assert_eq!(opens % 2, 0, "unbalanced hyperlink in {:?}", rendered);
        }
    }
}
