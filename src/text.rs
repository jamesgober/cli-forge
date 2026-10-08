//! Terminal-aware text measurement and shaping.
//!
//! Once text carries escape sequences, `str::len` and `chars().count()` both stop
//! answering the question anyone is actually asking: *how many columns will this
//! occupy?* Everything that lines things up — this crate's own help engine, and
//! every sibling crate that draws tables, progress bars, or boxes — needs one
//! correct answer, so it lives here rather than being re-derived (and
//! re-mis-derived) in each of them.
//!
//! The functions fall into three groups:
//!
//! - **Measure.** [`width`] counts display columns, skipping escape sequences and
//!   accounting for wide and zero-width characters.
//! - **Shape.** [`pad`], [`truncate`], and [`wrap`] fit text to a column budget
//!   without ever cutting an escape sequence in half.
//! - **Neutralise.** [`strip`] removes styling; [`sanitize`] removes the control
//!   characters that let untrusted text take over a terminal.
//!
//! ## On width accuracy
//!
//! With the `unicode` feature, widths come from the Unicode East Asian Width
//! tables: CJK and emoji count as two columns, combining marks as zero. Without
//! it, the fallback counts characters, which is exact for the Latin, Greek, and
//! Cyrillic text most CLIs emit and wrong for CJK and emoji. The feature is
//! therefore worth enabling for any program whose output can contain either, and
//! skippable for one that cannot — which is why it is a feature and not a
//! dependency.

use crate::shim::{Cow, String, Vec};

/// Where text sits inside a padded field.
///
/// # Examples
///
/// ```
/// use cli_forge::text::{self, Align};
///
/// assert_eq!(text::pad("ok", 6, Align::Left), "ok    ");
/// assert_eq!(text::pad("ok", 6, Align::Right), "    ok");
/// assert_eq!(text::pad("ok", 6, Align::Center), "  ok  ");
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Align {
    /// Flush left, padding on the right. The default.
    #[default]
    Left,
    /// Flush right, padding on the left.
    Right,
    /// Centred, with any odd column going to the right.
    Center,
}

/// The escape byte that introduces every ANSI sequence.
const ESC: u8 = 0x1b;

/// The length in bytes of the escape sequence starting at `at`, or `None` when no
/// sequence starts there.
///
/// Three shapes are recognised, which covers everything a terminal styling layer
/// emits: a CSI sequence (`ESC [` … final byte in `0x40..=0x7e`, which is where
/// all SGR styling lives), an OSC sequence (`ESC ]` … `BEL` or `ESC \`, which is
/// where hyperlinks and window titles live), and a bare two-byte escape.
///
/// An unterminated sequence consumes the rest of the input: that is what a
/// terminal does with it, so measuring or stripping must agree.
fn escape_len(bytes: &[u8], at: usize) -> Option<usize> {
    if bytes.get(at).copied() != Some(ESC) {
        return None;
    }
    match bytes.get(at + 1).copied() {
        Some(b'[') => {
            let mut i = at + 2;
            // Parameter and intermediate bytes, then one final byte.
            while i < bytes.len() {
                if (0x40..=0x7e).contains(&bytes[i]) {
                    return Some(i + 1 - at);
                }
                i += 1;
            }
            Some(bytes.len() - at)
        }
        Some(b']') => {
            let mut i = at + 2;
            while i < bytes.len() {
                if bytes[i] == 0x07 {
                    return Some(i + 1 - at);
                }
                if bytes[i] == ESC && bytes.get(i + 1).copied() == Some(b'\\') {
                    return Some(i + 2 - at);
                }
                i += 1;
            }
            Some(bytes.len() - at)
        }
        // A second escape aborts the first and starts a sequence of its own,
        // which is what a terminal does with it, so the first stands alone.
        Some(ESC) => Some(1),
        // A two-byte escape, but only when what follows is ASCII. `ESC` before
        // a multi-byte character is not a sequence at all, and consuming two
        // bytes of one would land the scan inside a character and slice a `str`
        // off its boundary.
        Some(byte) if byte.is_ascii() => Some(2),
        // A stray escape before non-ASCII text, or a trailing lone one.
        Some(_) | None => Some(1),
    }
}

/// Whether the escape sequence of length `len` starting at `at` is a lone,
/// meaningless escape byte rather than a real sequence.
///
/// A stray `ESC` carries no styling and occupies no columns, and copying one into
/// shaped output is actively harmful: placed next to a following sequence it
/// swallows that sequence's introducer and turns it into visible text.
#[inline]
fn is_stray_escape(bytes: &[u8], at: usize, len: usize) -> bool {
    len == 1 && bytes.get(at).copied() == Some(ESC)
}

/// The display width of one character, in terminal columns.
#[inline]
fn char_width(ch: char) -> usize {
    #[cfg(feature = "unicode")]
    {
        unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0)
    }
    #[cfg(not(feature = "unicode"))]
    {
        // Control characters occupy no columns; everything else is assumed to be
        // one. Exact for Latin, Greek, and Cyrillic; wrong for CJK and emoji,
        // which is what the `unicode` feature exists to fix.
        if ch.is_control() { 0 } else { 1 }
    }
}

/// The number of terminal columns `text` occupies.
///
/// Escape sequences are skipped (they are invisible), and character widths follow
/// the rules described in the [module documentation](self#on-width-accuracy).
///
/// # Examples
///
/// ```
/// use cli_forge::{style, text};
///
/// assert_eq!(text::width("hello"), 5);
///
/// // Styling is invisible, so it adds no columns — this is the measurement
/// // `str::len` gets wrong.
/// let styled = style("hello").red().bold().render();
/// assert_eq!(text::width(&styled), 5);
///
/// // Control characters take no space.
/// assert_eq!(text::width("a\u{7}b"), 2);
/// ```
#[must_use]
pub fn width(text: &str) -> usize {
    let bytes = text.as_bytes();
    let mut total = 0;
    let mut i = 0;
    while i < bytes.len() {
        if let Some(len) = escape_len(bytes, i) {
            i += len;
            continue;
        }
        // `i` is at a character boundary: escape sequences are pure ASCII, so
        // skipping one never lands inside a multi-byte character.
        let ch = text[i..].chars().next().unwrap_or('\0');
        total += char_width(ch);
        i += ch.len_utf8();
    }
    total
}

/// Remove every escape sequence from `text`, leaving the visible characters.
///
/// Borrows when there is nothing to remove, so measuring or logging already-plain
/// text costs no allocation.
///
/// # Examples
///
/// ```
/// use cli_forge::{style, text};
///
/// let styled = style("done").green().bold().render();
/// assert_eq!(text::strip(&styled), "done");
///
/// // Plain text is returned untouched, without allocating.
/// assert!(matches!(text::strip("plain"), std::borrow::Cow::Borrowed("plain")));
/// ```
#[must_use]
pub fn strip(text: &str) -> Cow<'_, str> {
    let bytes = text.as_bytes();
    let Some(first) = bytes.iter().position(|&b| b == ESC) else {
        return Cow::Borrowed(text);
    };

    let mut out = String::with_capacity(text.len());
    out.push_str(&text[..first]);
    let mut i = first;
    while i < bytes.len() {
        if let Some(len) = escape_len(bytes, i) {
            i += len;
            continue;
        }
        let ch = text[i..].chars().next().unwrap_or('\0');
        out.push(ch);
        i += ch.len_utf8();
    }
    Cow::Owned(out)
}

/// Neutralise control characters in text from an untrusted source.
///
/// Printing an attacker-controlled string straight to a terminal hands it the
/// terminal: escape sequences can clear the screen, reposition the cursor to
/// overwrite earlier output, relabel the window, change the colours permanently,
/// or — on some terminals — stuff text back into the input queue. A filename, a
/// commit message, a server response, or an error echoed back from a remote
/// system is exactly the kind of text this happens with.
///
/// Every C0 control character except `\n` and `\t`, every C1 control character,
/// and `DEL` are replaced with a visible caret escape (`^[`, `^G`, …), so the
/// output is still legible and no longer executable. Tabs and newlines survive
/// because they are layout, not control, and callers rely on them.
///
/// Borrows when there is nothing to neutralise.
///
/// # Examples
///
/// ```
/// use cli_forge::text;
///
/// // A hostile "filename" that would otherwise clear the screen and lie.
/// let hostile = "report.txt\x1b[2J\x1b[1;1HALL FILES DELETED";
/// let safe = text::sanitize(hostile);
/// assert!(!safe.contains('\u{1b}'));
/// assert!(safe.starts_with("report.txt^["));
///
/// // Layout whitespace is preserved.
/// assert_eq!(text::sanitize("a\tb\nc"), "a\tb\nc");
/// ```
#[must_use]
pub fn sanitize(text: &str) -> Cow<'_, str> {
    if !text.chars().any(needs_sanitizing) {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len() + 8);
    for ch in text.chars() {
        if needs_sanitizing(ch) {
            push_caret(&mut out, ch);
        } else {
            out.push(ch);
        }
    }
    Cow::Owned(out)
}

/// Whether `ch` is a control character that must not reach a terminal verbatim.
#[inline]
fn needs_sanitizing(ch: char) -> bool {
    match ch {
        '\n' | '\t' => false,
        '\0'..='\u{1f}' | '\u{7f}' => true,
        '\u{80}'..='\u{9f}' => true,
        _ => false,
    }
}

/// Write `ch` in caret notation: `ESC` becomes `^[`, `BEL` becomes `^G`.
fn push_caret(out: &mut String, ch: char) {
    let code = ch as u32;
    out.push('^');
    // C0 maps to `@`..`_` by adding 0x40; DEL conventionally shows as `?`. C1
    // has no caret form, so its low nibble is shown with a leading `[`.
    match code {
        0x00..=0x1f => out.push(char::from(b'@' + code as u8)),
        0x7f => out.push('?'),
        _ => {
            out.push('[');
            // Two hex digits, which is unambiguous and never itself a control.
            let byte = (code & 0xff) as u8;
            for shift in [4, 0] {
                let nibble = (byte >> shift) & 0xf;
                out.push(char::from(if nibble < 10 {
                    b'0' + nibble
                } else {
                    b'a' + nibble - 10
                }));
            }
        }
    }
}

/// Pad `text` to `columns` display columns, aligned as `align` says.
///
/// Text already at or over the budget is returned untouched — padding never
/// truncates, because silently losing a caller's content is worse than a ragged
/// column. Pair with [`truncate`] when the budget is hard.
///
/// Because the measurement is display width rather than byte length, this is the
/// one that keeps columns straight when the content is styled or non-Latin, which
/// `format!("{:<width$}")` cannot do.
///
/// # Examples
///
/// ```
/// use cli_forge::text::{self, Align};
///
/// assert_eq!(text::pad("ok", 5, Align::Left), "ok   ");
/// assert_eq!(text::pad("ok", 5, Align::Right), "   ok");
/// assert_eq!(text::pad("ok", 5, Align::Center), " ok  ");
///
/// // Over budget: returned as-is.
/// assert_eq!(text::pad("too long", 3, Align::Left), "too long");
/// ```
#[must_use]
pub fn pad(text: &str, columns: usize, align: Align) -> Cow<'_, str> {
    let current = width(text);
    if current >= columns {
        return Cow::Borrowed(text);
    }
    let deficit = columns - current;
    let (before, after) = match align {
        Align::Left => (0, deficit),
        Align::Right => (deficit, 0),
        // An odd column goes to the right, so a column of centred labels has a
        // consistent left edge.
        Align::Center => (deficit / 2, deficit - deficit / 2),
    };
    let mut out = String::with_capacity(text.len() + deficit);
    for _ in 0..before {
        out.push(' ');
    }
    out.push_str(text);
    for _ in 0..after {
        out.push(' ');
    }
    Cow::Owned(out)
}

/// Cut `text` down to `columns` display columns, ending with `ellipsis` when
/// anything was removed.
///
/// Escape sequences are copied through rather than counted, and are never cut in
/// half, so the result is always a valid sequence of styled runs. Any styling
/// still open at the cut is closed with a reset, so a truncated cell cannot leak
/// its colour into whatever is printed next.
///
/// # Examples
///
/// ```
/// use cli_forge::{style, text};
///
/// assert_eq!(text::truncate("hello world", 8, "…"), "hello w…");
/// // Nothing removed, nothing appended.
/// assert_eq!(text::truncate("short", 8, "…"), "short");
///
/// // Styling survives the cut and is closed properly.
/// let styled = style("hello world").red().render();
/// let cut = text::truncate(&styled, 5, "");
/// assert_eq!(text::strip(&cut), "hello");
/// ```
#[must_use]
pub fn truncate<'a>(text: &'a str, columns: usize, ellipsis: &str) -> Cow<'a, str> {
    if width(text) <= columns {
        return Cow::Borrowed(text);
    }
    let budget = columns.saturating_sub(width(ellipsis));

    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len().min(columns * 4 + 16));
    let mut used = 0;
    let mut i = 0;
    let mut styled = false;

    while i < bytes.len() {
        if let Some(len) = escape_len(bytes, i) {
            // A stray escape is dropped rather than copied: keeping it would
            // corrupt whichever sequence ended up after it.
            if !is_stray_escape(bytes, i, len) {
                let sequence = &text[i..i + len];
                // A reset closes styling; anything else opens or changes it.
                styled = !matches!(sequence, "\x1b[0m" | "\x1b[m");
                out.push_str(sequence);
            }
            i += len;
            continue;
        }
        let ch = text[i..].chars().next().unwrap_or('\0');
        let w = char_width(ch);
        if used + w > budget {
            break;
        }
        out.push(ch);
        used += w;
        i += ch.len_utf8();
    }

    out.push_str(ellipsis);
    if styled {
        out.push_str("\x1b[0m");
    }
    Cow::Owned(out)
}

/// Break `text` into lines of at most `columns` display columns.
///
/// Breaks at whitespace where it can and mid-word only when a single word cannot
/// fit at all, so no content is ever dropped. Existing newlines are honoured as
/// hard breaks, which is what makes this usable for help text written as
/// paragraphs. A `columns` of `0` is treated as `1`, so the function always makes
/// progress.
///
/// The returned lines borrow from `text`: wrapping allocates one vector and no
/// strings.
///
/// # Examples
///
/// ```
/// use cli_forge::text;
///
/// let lines = text::wrap("the quick brown fox jumps", 11);
/// assert_eq!(lines, ["the quick", "brown fox", "jumps"]);
///
/// // Hard newlines are respected.
/// assert_eq!(text::wrap("one\ntwo", 80), ["one", "two"]);
///
/// // A word longer than the budget is split rather than lost.
/// assert_eq!(text::wrap("antidisestablishmentarianism", 10).len(), 3);
/// ```
#[must_use]
pub fn wrap(text: &str, columns: usize) -> Vec<&str> {
    let columns = columns.max(1);
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        wrap_paragraph(paragraph, columns, &mut lines);
    }
    lines
}

/// Wrap one newline-free paragraph into `lines`.
///
/// Each pass takes the longest prefix of what is left that fits the budget,
/// preferring to end it at whitespace. The whitespace a break lands on is
/// consumed rather than carried onto the next line, so wrapped paragraphs do not
/// acquire a creeping indent. Progress is guaranteed because every pass pushes at
/// least one character, which is what keeps a budget of one column — or a single
/// character wider than the whole budget — from looping.
fn wrap_paragraph<'a>(paragraph: &'a str, columns: usize, lines: &mut Vec<&'a str>) {
    if paragraph.is_empty() {
        lines.push(paragraph);
        return;
    }

    let mut start = 0;
    while start < paragraph.len() {
        let rest = &paragraph[start..];
        if width(rest) <= columns {
            lines.push(rest);
            return;
        }

        // Walk until the budget is spent, remembering the last place a word
        // ended. `cut` is where the budget ran out.
        let mut used = 0;
        let mut last_space: Option<usize> = None;
        let mut cut = rest.len();
        for (offset, ch) in rest.char_indices() {
            if used + char_width(ch) > columns {
                cut = offset;
                break;
            }
            if ch.is_whitespace() && offset > 0 {
                last_space = Some(offset);
            }
            used += char_width(ch);
        }

        // If the budget ran out exactly at a space, that space is the break —
        // and a better one than any earlier space, because it yields the longest
        // line that fits. Checked unconditionally for that reason: preferring an
        // earlier space would wrap `"a b c"` at 3 columns into `["a", "b c"]`
        // when `["a b", "c"]` fits.
        if rest[cut..].starts_with(char::is_whitespace) {
            last_space = Some(cut);
        }

        let (end, next) = match last_space {
            // Break at whitespace and swallow the whole run of it.
            Some(space) => (space, space + whitespace_len(&rest[space..])),
            // No break opportunity: one word is wider than the budget, so split
            // it rather than drop any of it. At least one character must go, or
            // the loop would not advance.
            None => {
                let least = rest.chars().next().map_or(1, char::len_utf8);
                let at = if cut == 0 { least } else { cut };
                (at, at)
            }
        };
        lines.push(&rest[..end]);
        start += next;
    }
}

/// The byte length of the whitespace run at the start of `rest`, so a line break
/// consumes the space it broke on rather than indenting the next line with it.
fn whitespace_len(rest: &str) -> usize {
    rest.chars()
        .take_while(|c| c.is_whitespace())
        .map(char::len_utf8)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style;
    use crate::terminal::ColorLevel;

    /// A styled string at a known level, independent of the test environment's
    /// terminal.
    fn styled(text: &str) -> String {
        style(text).red().bold().render_at(ColorLevel::Ansi16)
    }

    #[test]
    fn test_width_ignores_escape_sequences() {
        assert_eq!(width("hello"), 5);
        assert_eq!(width(&styled("hello")), 5);
        assert_eq!(width(""), 0);
    }

    #[test]
    fn test_width_ignores_osc_hyperlinks() {
        // An OSC 8 hyperlink wraps visible text in two invisible sequences.
        let link = "\x1b]8;;https://example.com\x07click\x1b]8;;\x07";
        assert_eq!(width(link), 5);
        assert_eq!(strip(link), "click");
    }

    #[test]
    fn test_width_of_unterminated_escape_is_zero() {
        // A truncated sequence swallows the rest, exactly as a terminal would.
        assert_eq!(width("a\x1b[31"), 1);
        assert_eq!(width("\x1b"), 0);
    }

    #[test]
    fn test_strip_borrows_plain_text() {
        assert!(matches!(strip("plain"), Cow::Borrowed(_)));
        assert!(matches!(strip(&styled("x")), Cow::Owned(_)));
        assert_eq!(strip(&styled("done")), "done");
    }

    #[test]
    fn test_sanitize_neutralises_escape_injection() {
        let hostile = "report.txt\x1b[2J\x1b[1;1H\x1b[31mSYSTEM COMPROMISED";
        let safe = sanitize(hostile);
        assert!(!safe.contains('\u{1b}'), "escape survived: {safe:?}");
        assert!(safe.starts_with("report.txt^["));
        // The visible text is kept; only the control byte is defanged.
        assert!(safe.contains("SYSTEM COMPROMISED"));
    }

    #[test]
    fn test_sanitize_preserves_layout_whitespace_and_borrows() {
        assert!(matches!(sanitize("clean text"), Cow::Borrowed(_)));
        assert_eq!(sanitize("a\tb\nc"), "a\tb\nc");
        // A bell and a DEL both get caret forms.
        assert_eq!(sanitize("a\u{7}b"), "a^Gb");
        assert_eq!(sanitize("a\u{7f}b"), "a^?b");
        // C1 controls are escapes in UTF-8 terminals too.
        assert_eq!(sanitize("a\u{9b}b"), "a^[9bb");
    }

    #[test]
    fn test_pad_measures_display_columns_not_bytes() {
        assert_eq!(pad("ok", 5, Align::Left), "ok   ");
        assert_eq!(pad("ok", 5, Align::Right), "   ok");
        assert_eq!(pad("ok", 5, Align::Center), " ok  ");
        // Accented text is 2 columns but 4 bytes; `format!` would under-pad it.
        assert_eq!(width(&pad("éé", 5, Align::Left)), 5);
        // Styled text pads by what is visible.
        assert_eq!(width(&pad(&styled("ok"), 5, Align::Left)), 5);
    }

    #[test]
    fn test_pad_never_truncates() {
        assert_eq!(pad("too long", 3, Align::Left), "too long");
        assert!(matches!(pad("exact", 5, Align::Left), Cow::Borrowed(_)));
    }

    #[test]
    fn test_truncate_respects_the_budget_and_closes_styling() {
        assert_eq!(truncate("hello world", 8, "…"), "hello w…");
        assert_eq!(truncate("short", 8, "…"), "short");
        assert!(matches!(truncate("short", 8, "…"), Cow::Borrowed(_)));

        let long = styled("hello world");
        let cut = truncate(&long, 5, "");
        assert_eq!(strip(&cut), "hello");
        assert!(cut.ends_with("\x1b[0m"), "styling leaked: {cut:?}");
        assert_eq!(width(&cut), 5);
    }

    #[test]
    fn test_truncate_with_a_budget_under_the_ellipsis() {
        // Nothing fits, but the result is still valid and never panics.
        let cut = truncate("hello", 1, "...");
        assert_eq!(cut, "...");
    }

    #[test]
    fn test_wrap_uses_the_last_space_that_fits() {
        // The budget runs out exactly at a space, which is the longest line that
        // fits; breaking at the earlier space would waste a column.
        assert_eq!(wrap("a b c", 3), ["a b", "c"]);
        assert_eq!(wrap("ab cd ef", 5), ["ab cd", "ef"]);
        assert_eq!(wrap("a bb ccc", 6), ["a bb", "ccc"]);
    }

    #[test]
    fn test_wrap_breaks_on_whitespace() {
        assert_eq!(
            wrap("the quick brown fox jumps", 11),
            ["the quick", "brown fox", "jumps"]
        );
        assert_eq!(wrap("short", 80), ["short"]);
    }

    #[test]
    fn test_wrap_honours_hard_newlines() {
        assert_eq!(wrap("one\ntwo", 80), ["one", "two"]);
        assert_eq!(wrap("a\n\nb", 80), ["a", "", "b"]);
    }

    #[test]
    fn test_wrap_splits_an_unbreakable_word() {
        let lines = wrap("antidisestablishmentarianism", 10);
        assert_eq!(lines.len(), 3);
        // Every character is preserved across the split.
        assert_eq!(lines.concat(), "antidisestablishmentarianism");
        assert!(lines.iter().all(|line| width(line) <= 10));
    }

    #[test]
    fn test_wrap_tolerates_a_zero_budget() {
        let lines = wrap("ab", 0);
        assert_eq!(lines.concat(), "ab");
    }

    #[test]
    fn test_escape_len_recognises_each_shape() {
        assert_eq!(escape_len(b"\x1b[31m", 0), Some(5));
        assert_eq!(escape_len(b"\x1b]8;;x\x07", 0), Some(7));
        assert_eq!(escape_len(b"\x1b]8;;x\x1b\\", 0), Some(8));
        assert_eq!(escape_len(b"\x1bM", 0), Some(2));
        assert_eq!(escape_len(b"\x1b", 0), Some(1));
        assert_eq!(escape_len(b"plain", 0), None);
    }
}

#[cfg(test)]
mod proptests {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        /// No input, however adversarial, may panic any of the shapers.
        #[test]
        fn test_never_panics(input in ".*", columns in 0usize..40) {
            let _ = width(&input);
            let _ = strip(&input);
            let _ = sanitize(&input);
            let _ = pad(&input, columns, Align::Center);
            let _ = truncate(&input, columns, "…");
            let _ = wrap(&input, columns);
        }

        /// Padding reaches the requested width whenever the text fits in it.
        #[test]
        fn test_pad_reaches_the_budget(input in "[a-z ]{0,20}", columns in 0usize..40) {
            let padded = pad(&input, columns, Align::Left);
            prop_assert_eq!(width(&padded), width(&input).max(columns));
        }

        /// Truncation never exceeds the budget, and never alters text that fits.
        #[test]
        fn test_truncate_respects_the_budget(input in ".{0,40}", columns in 1usize..40) {
            let cut = truncate(&input, columns, "");
            prop_assert!(width(&cut) <= columns);
            if width(&input) <= columns {
                prop_assert_eq!(cut.as_ref(), input.as_str());
            }
        }

        /// Wrapping is lossless: the lines rejoin to the original, and none
        /// exceeds the budget unless it holds a single oversized character.
        #[test]
        fn test_wrap_is_lossless(input in "[a-z ]{0,60}", columns in 1usize..20) {
            let lines = wrap(&input, columns);
            let rejoined: String = lines.join("");
            // Only the whitespace a break consumed may be missing.
            prop_assert_eq!(
                rejoined.chars().filter(|c| !c.is_whitespace()).count(),
                input.chars().filter(|c| !c.is_whitespace()).count()
            );
            for line in &lines {
                prop_assert!(width(line) <= columns, "line over budget: {line:?}");
            }
        }

        /// Sanitising always removes every escape byte and never adds one.
        #[test]
        fn test_sanitize_leaves_no_controls(input in ".*") {
            let safe = sanitize(&input);
            prop_assert!(!safe.chars().any(needs_sanitizing));
        }

        /// Stripping leaves nothing an escape scanner would recognise.
        #[test]
        fn test_strip_removes_every_sequence(input in ".*") {
            let plain = strip(&input);
            prop_assert!(!plain.as_bytes().contains(&ESC));
        }

        /// Stripping cannot change what the text measures: both skip exactly the
        /// same bytes.
        #[test]
        fn test_strip_preserves_width(input in ".*") {
            prop_assert_eq!(width(&strip(&input)), width(&input));
        }
    }
}
