//! The styling builder and the rendering core shared by every styling path.
//!
//! [`Style`] is the function-call path: build a color/attribute set by chaining
//! methods, then drop it straight into [`out`](crate::out) because it implements
//! [`Display`]. The same attribute set is what the tag parser and the named
//! registry produce, and all three render through one private function,
//! [`write_styled`], so identical intent yields byte-identical output.

use std::fmt::{self, Display, Write};

use crate::color::Color;
use crate::terminal::{self, ColorLevel};

/// The visual attributes of a styled run: a foreground color plus the bold and
/// underline flags. Cheap to copy, so it threads through the tag parser and the
/// registry without allocation.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) struct StyleAttrs {
    pub(crate) fg: Option<Color>,
    pub(crate) bold: bool,
    pub(crate) underline: bool,
}

impl StyleAttrs {
    /// No color and no attributes.
    pub(crate) const EMPTY: StyleAttrs = StyleAttrs {
        fg: None,
        bold: false,
        underline: false,
    };

    /// Whether this set would emit no escape sequences.
    #[inline]
    pub(crate) fn is_empty(self) -> bool {
        self.fg.is_none() && !self.bold && !self.underline
    }
}

/// Render `text` with `attrs` at `level`, writing to `w`.
///
/// This is the single rendering primitive behind the builder, the tag parser,
/// and the registry. Parameters are emitted in a fixed canonical order — bold
/// (`1`), underline (`4`), then the foreground color — so two callers expressing
/// the same intent produce the same bytes regardless of the order they set
/// things. At [`ColorLevel::None`], or when `attrs` is empty, the plain text is
/// written with no escape sequences.
pub(crate) fn write_styled<W: Write>(
    w: &mut W,
    attrs: StyleAttrs,
    text: &str,
    level: ColorLevel,
) -> fmt::Result {
    if level.is_none() || attrs.is_empty() {
        return w.write_str(text);
    }

    w.write_str("\x1b[")?;
    let mut first = true;
    if attrs.bold {
        w.write_str("1")?;
        first = false;
    }
    if attrs.underline {
        if !first {
            w.write_char(';')?;
        }
        w.write_str("4")?;
        first = false;
    }
    if let Some(color) = attrs.fg {
        color.write_fg(w, level, &mut first)?;
    }
    w.write_str("m")?;
    w.write_str(text)?;
    w.write_str("\x1b[0m")
}

/// A piece of text together with the color and attributes to render it with.
///
/// Created by [`style`]. The setter methods consume and return `self`, so they
/// chain. `Style` implements [`Display`]: passing one to [`out`](crate::out)
/// renders it, and the color depth matches whatever the terminal supports.
///
/// # Examples
///
/// ```
/// use cli_core::{out, style};
///
/// out(style("done").green().bold());
/// out(style("note").hex("#88aaff"));
/// out(style("ok").rgb(0, 200, 120));
/// ```
#[derive(Clone, Debug)]
pub struct Style {
    text: String,
    attrs: StyleAttrs,
}

/// Begin styling `text`.
///
/// The returned [`Style`] starts plain; chain color and attribute methods onto
/// it. `text` accepts anything convertible into a `String`, so both string
/// literals and owned `String`s work.
///
/// # Examples
///
/// ```
/// use cli_core::style;
///
/// let warning = style("low disk space").yellow().bold();
/// // `Style` is `Display`, so it renders when printed or formatted.
/// assert!(warning.render().contains("low disk space"));
/// ```
#[must_use]
pub fn style<S: Into<String>>(text: S) -> Style {
    Style {
        text: text.into(),
        attrs: StyleAttrs::EMPTY,
    }
}

/// Generate a consuming builder method that sets the foreground to a named color.
macro_rules! named_color_method {
    ($(#[$meta:meta])* $name:ident => $variant:ident) => {
        $(#[$meta])*
        #[must_use]
        pub fn $name(mut self) -> Style {
            self.attrs.fg = Some(Color::$variant);
            self
        }
    };
}

impl Style {
    named_color_method!(/// Set the foreground to the standard black.
        black => Black);
    named_color_method!(/// Set the foreground to the standard red.
        red => Red);
    named_color_method!(/// Set the foreground to the standard green.
        green => Green);
    named_color_method!(/// Set the foreground to the standard yellow.
        yellow => Yellow);
    named_color_method!(/// Set the foreground to the standard blue.
        blue => Blue);
    named_color_method!(/// Set the foreground to the standard magenta.
        magenta => Magenta);
    named_color_method!(/// Set the foreground to the standard cyan.
        cyan => Cyan);
    named_color_method!(/// Set the foreground to the standard white.
        white => White);

    /// Set the foreground to a 24-bit hex color, e.g. `"#ff8800"`.
    ///
    /// The leading `#` is optional; the rest must be exactly six hex digits. An
    /// invalid string leaves the current color unchanged, so the builder never
    /// fails. On terminals without 24-bit support the color is downgraded to the
    /// nearest representable value at render time.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_core::style;
    ///
    /// let link = style("https://example.com").hex("#3b82f6").underline();
    /// assert!(link.render().contains("https://example.com"));
    ///
    /// // An invalid hex string is ignored rather than panicking.
    /// let plain = style("x").hex("nope");
    /// assert_eq!(plain.render(), "x");
    /// ```
    #[must_use]
    pub fn hex(mut self, hex: &str) -> Style {
        if let Some(color) = Color::from_hex(hex) {
            self.attrs.fg = Some(color);
        }
        self
    }

    /// Set the foreground to a 24-bit RGB color.
    ///
    /// On terminals without 24-bit support the color is downgraded to the
    /// nearest representable value at render time.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_core::style;
    ///
    /// let teal = style("ok").rgb(0, 200, 120);
    /// assert!(teal.render().contains("ok"));
    /// ```
    #[must_use]
    pub fn rgb(mut self, r: u8, g: u8, b: u8) -> Style {
        self.attrs.fg = Some(Color::Rgb(r, g, b));
        self
    }

    /// Render the text in bold.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_core::style;
    ///
    /// let heading = style("Summary").bold();
    /// assert!(heading.render().contains("Summary"));
    /// ```
    #[must_use]
    pub fn bold(mut self) -> Style {
        self.attrs.bold = true;
        self
    }

    /// Underline the text.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_core::style;
    ///
    /// let link = style("docs").underline();
    /// assert!(link.render().contains("docs"));
    /// ```
    #[must_use]
    pub fn underline(mut self) -> Style {
        self.attrs.underline = true;
        self
    }

    /// Render to an owned `String`, ready to print or store.
    ///
    /// Equivalent to formatting the `Style` via its [`Display`] implementation.
    /// The color depth matches the terminal detected for standard output, so on
    /// a pipe or a `NO_COLOR` environment the result is the plain text.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_core::style;
    ///
    /// let s = style("ready").green().render();
    /// assert!(s.contains("ready"));
    /// ```
    #[must_use]
    pub fn render(&self) -> String {
        let mut buf = String::with_capacity(self.text.len() + STYLE_OVERHEAD);
        // Writing to a `String` is infallible; the `fmt::Result` cannot be `Err`.
        let _ = write_styled(&mut buf, self.attrs, &self.text, terminal::color_level());
        buf
    }

    /// The attribute set, for the registry to capture a reusable style.
    pub(crate) fn attrs(&self) -> StyleAttrs {
        self.attrs
    }
}

/// A generous upper bound on the escape-sequence bytes wrapping one styled run,
/// used to size the render buffer so the common case needs no reallocation.
const STYLE_OVERHEAD: usize = 24;

impl Display for Style {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_styled(f, self.attrs, &self.text, terminal::color_level())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    /// Render an attribute set at an explicit level, bypassing terminal
    /// detection so the bytes are deterministic in the test harness.
    fn render_at(attrs: StyleAttrs, text: &str, level: ColorLevel) -> String {
        let mut s = String::new();
        write_styled(&mut s, attrs, text, level).unwrap();
        s
    }

    #[test]
    fn test_empty_style_is_plain_even_with_color() {
        let attrs = StyleAttrs::EMPTY;
        assert_eq!(render_at(attrs, "hello", ColorLevel::TrueColor), "hello");
    }

    #[test]
    fn test_none_level_strips_all_styling() {
        let attrs = StyleAttrs {
            fg: Some(Color::Red),
            bold: true,
            underline: true,
        };
        assert_eq!(render_at(attrs, "x", ColorLevel::None), "x");
    }

    #[test]
    fn test_canonical_parameter_order_is_bold_underline_color() {
        let attrs = StyleAttrs {
            fg: Some(Color::Red),
            bold: true,
            underline: true,
        };
        assert_eq!(
            render_at(attrs, "ERR", ColorLevel::Ansi16),
            "\x1b[1;4;31mERR\x1b[0m"
        );
    }

    #[test]
    fn test_builder_order_does_not_change_bytes() {
        // Setting attributes in different orders yields the same canonical bytes.
        let a = style("ERR").red().bold().underline();
        let b = style("ERR").underline().bold().red();
        assert_eq!(
            render_at(a.attrs(), "ERR", ColorLevel::Ansi16),
            render_at(b.attrs(), "ERR", ColorLevel::Ansi16)
        );
    }

    #[test]
    fn test_single_attribute_has_no_stray_separator() {
        let bold = StyleAttrs {
            fg: None,
            bold: true,
            underline: false,
        };
        assert_eq!(render_at(bold, "x", ColorLevel::Ansi16), "\x1b[1mx\x1b[0m");
        let red = StyleAttrs {
            fg: Some(Color::Red),
            bold: false,
            underline: false,
        };
        assert_eq!(render_at(red, "x", ColorLevel::Ansi16), "\x1b[31mx\x1b[0m");
    }

    #[test]
    fn test_invalid_hex_leaves_color_unset() {
        assert_eq!(style("x").hex("zzzzzz").attrs().fg, None);
        assert_eq!(
            style("x").hex("#abcdef").attrs().fg,
            Some(Color::Rgb(171, 205, 239))
        );
    }
}
