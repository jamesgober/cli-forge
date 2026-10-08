//! The styling builder and the rendering core shared by every styling path.
//!
//! A [`Style`] is a *reusable description* of how text should look: colours,
//! attributes, and the decoration around it (a prefix glyph, a suffix, a padded
//! width, a hyperlink). It is built by chaining and then used two ways —
//! immediately, because it implements [`Display`] and so drops straight into
//! [`out`](crate::out), or repeatedly, by calling [`paint`](Style::paint) on
//! whatever text needs it next:
//!
//! ```
//! # #[cfg(feature = "std")] fn main() {
//! use cli_forge::{out, style, Style};
//!
//! // Once, inline.
//! out(style("done").green().bold());
//!
//! // Or described once and reused forever — no re-specifying at each call site.
//! let heading = Style::new().bold().underline();
//! out(heading.paint("Results"));
//! out(heading.paint("Summary"));
//! # }
//! # #[cfg(not(feature = "std"))] fn main() {}
//! ```
//!
//! The same attribute set is what the tag parser, the named registry, and themes
//! all produce, and all of them render through one private function,
//! [`write_styled`], so identical intent yields byte-identical output no matter
//! which path expressed it. That property is tested directly, because it is the
//! whole reason the paths can be mixed freely in one program.
//!
//! ## What costs what
//!
//! Rendering writes escape sequences straight into the formatter, so painting a
//! value allocates nothing. Two features are the exceptions, and both say so in
//! their own documentation: [`pad_to`](Style::pad_to) must measure the finished
//! text before it can know how much padding to add, and [`render`](Style::render)
//! returns an owned `String` by definition. A [`Style`] with no colours and no
//! attributes writes its text and nothing else, so a themed program that the user
//! has run with `NO_COLOR` pays essentially nothing for the styling it asked for.

use core::fmt::{self, Display, Write};

use crate::color::Color;
use crate::shim::{Box, String, format};
use crate::terminal::{self, ColorLevel, Stream};
use crate::text::{self, Align};

/// The SGR parameter for each attribute, in the canonical order they are emitted.
///
/// Fixing the order is what lets two callers that expressed the same intent
/// differently produce the same bytes.
/// Held as text rather than as a number, so emitting one is a string copy
/// instead of a trip through the integer formatter.
const ATTRIBUTES: [(u8, &str); 8] = [
    (BOLD, "1"),
    (DIM, "2"),
    (ITALIC, "3"),
    (UNDERLINE, "4"),
    (BLINK, "5"),
    (REVERSE, "7"),
    (HIDDEN, "8"),
    (STRIKE, "9"),
];

/// The attribute bits, one per renderable attribute.
///
/// Packed into a single byte so a [`StyleAttrs`] stays `Copy` and small enough to
/// thread through the markup parser without allocation. Shared with that parser,
/// which needs to set the same bits from its own tag names.
pub(crate) mod flags {
    /// Bold (SGR 1).
    pub(crate) const BOLD: u8 = 1 << 0;
    /// Dim (SGR 2).
    pub(crate) const DIM: u8 = 1 << 1;
    /// Italic (SGR 3).
    pub(crate) const ITALIC: u8 = 1 << 2;
    /// Underline (SGR 4).
    pub(crate) const UNDERLINE: u8 = 1 << 3;
    /// Blink (SGR 5).
    pub(crate) const BLINK: u8 = 1 << 4;
    /// Reverse video (SGR 7).
    pub(crate) const REVERSE: u8 = 1 << 5;
    /// Concealed (SGR 8).
    pub(crate) const HIDDEN: u8 = 1 << 6;
    /// Strikethrough (SGR 9).
    pub(crate) const STRIKE: u8 = 1 << 7;
}

use flags::{BLINK, BOLD, DIM, HIDDEN, ITALIC, REVERSE, STRIKE, UNDERLINE};

/// The visual attributes of a styled run: a foreground colour, a background
/// colour, and the attribute flags. Cheap to copy, so it threads through the tag
/// parser and the registry without allocation.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) struct StyleAttrs {
    pub(crate) fg: Option<Color>,
    pub(crate) bg: Option<Color>,
    pub(crate) flags: u8,
}

impl StyleAttrs {
    /// No colours and no attributes.
    pub(crate) const EMPTY: StyleAttrs = StyleAttrs {
        fg: None,
        bg: None,
        flags: 0,
    };

    /// Whether this set would emit no escape sequences.
    #[inline]
    pub(crate) const fn is_empty(self) -> bool {
        self.fg.is_none() && self.bg.is_none() && self.flags == 0
    }

    /// Overlay `other` onto `self`: anything `other` specifies wins, anything it
    /// leaves unset is inherited.
    #[inline]
    fn overlay(self, other: StyleAttrs) -> StyleAttrs {
        StyleAttrs {
            fg: other.fg.or(self.fg),
            bg: other.bg.or(self.bg),
            flags: self.flags | other.flags,
        }
    }
}

/// Render `text` with `attrs` at `level`, writing to `w`.
///
/// This is the single rendering primitive behind the builder, the tag parser, the
/// registry, and themes. Parameters are emitted in a fixed canonical order —
/// attributes low-to-high, then foreground, then background — so two callers
/// expressing the same intent produce the same bytes regardless of the order they
/// set things in. At [`ColorLevel::None`], or when `attrs` is empty, the plain
/// text is written with no escape sequences at all.
pub(crate) fn write_styled<W: Write>(
    w: &mut W,
    attrs: StyleAttrs,
    text: &str,
    level: ColorLevel,
) -> fmt::Result {
    if level.is_none() || attrs.is_empty() {
        return w.write_str(text);
    }
    open(w, attrs, level)?;
    w.write_str(text)?;
    w.write_str(RESET)
}

/// The sequence that returns the terminal to its default appearance.
const RESET: &str = "\x1b[0m";

/// Write the opening escape sequence for `attrs`.
fn open<W: Write>(w: &mut W, attrs: StyleAttrs, level: ColorLevel) -> fmt::Result {
    w.write_str("\x1b[")?;
    let mut first = true;
    for &(flag, parameter) in &ATTRIBUTES {
        if attrs.flags & flag != 0 {
            if !first {
                w.write_char(';')?;
            }
            w.write_str(parameter)?;
            first = false;
        }
    }
    if let Some(fg) = attrs.fg {
        fg.write(w, level, false, &mut first)?;
    }
    if let Some(bg) = attrs.bg {
        bg.write(w, level, true, &mut first)?;
    }
    w.write_char('m')
}

/// A reusable description of how text should look.
///
/// Created by [`style`] (carrying text) or [`Style::new`] (empty, to be reused).
/// Setter methods consume and return `self`, so they chain. A `Style` is
/// [`Display`], so one carrying text drops straight into [`out`](crate::out);
/// [`paint`](Style::paint) applies a text-less one to any value.
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "std")] fn main() {
/// use cli_forge::{out, style, Style};
///
/// // Inline, for a one-off.
/// out(style("done").green().bold());
/// out(style("note").hex("#88aaff"));
///
/// // Described once, reused — including the glyph and the column width, so no
/// // call site hand-builds a marker again.
/// let ok = Style::new().green().bold().prefix("[ok] ").pad_to(10);
/// out(ok.paint("resolve dependencies"));
/// out(ok.paint("compile"));
/// # }
/// # #[cfg(not(feature = "std"))] fn main() {}
/// ```
#[derive(Clone, Debug)]
pub struct Style {
    text: String,
    attrs: StyleAttrs,
    prefix: Option<Box<str>>,
    suffix: Option<Box<str>>,
    pad: Option<(u16, Align)>,
    link: Option<Box<str>>,
}

/// Begin styling `text`.
///
/// The returned [`Style`] starts plain; chain colour and attribute methods onto
/// it. `text` accepts anything convertible into a `String`, so both literals and
/// owned values work. For a style to be *reused* rather than printed once, build
/// it with [`Style::new`] instead and apply it with [`Style::paint`].
///
/// # Examples
///
/// ```
/// use cli_forge::style;
///
/// let warning = style("low disk space").yellow().bold();
/// // `Style` is `Display`, so it renders when printed or formatted.
/// assert!(warning.render().contains("low disk space"));
/// ```
#[must_use]
pub fn style<S: Into<String>>(text: S) -> Style {
    Style {
        text: text.into(),
        ..Style::new()
    }
}

impl Default for Style {
    /// An empty style, identical to [`Style::new`].
    fn default() -> Style {
        Style::new()
    }
}

/// Generate a consuming builder method that sets a colour.
macro_rules! color_method {
    ($(#[$meta:meta])* $name:ident => $field:ident, $variant:ident) => {
        $(#[$meta])*
        #[must_use]
        pub fn $name(mut self) -> Style {
            self.attrs.$field = Some(Color::$variant);
            self
        }
    };
}

/// Generate a consuming builder method that sets an attribute flag.
macro_rules! attribute_method {
    ($(#[$meta:meta])* $name:ident => $flag:ident) => {
        $(#[$meta])*
        #[must_use]
        pub fn $name(mut self) -> Style {
            self.attrs.flags |= $flag;
            self
        }
    };
}

impl Style {
    /// An empty style, carrying no text: the starting point for a style meant to
    /// be defined once and reused.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Style;
    ///
    /// let emphasis = Style::new().bold();
    /// assert_eq!(emphasis.paint("x").to_string().contains("x"), true);
    /// ```
    #[must_use]
    pub const fn new() -> Style {
        Style {
            text: String::new(),
            attrs: StyleAttrs::EMPTY,
            prefix: None,
            suffix: None,
            pad: None,
            link: None,
        }
    }

    color_method!(/// Set the foreground to the terminal's black.
        black => fg, Black);
    color_method!(/// Set the foreground to the terminal's red.
        red => fg, Red);
    color_method!(/// Set the foreground to the terminal's green.
        green => fg, Green);
    color_method!(/// Set the foreground to the terminal's yellow.
        yellow => fg, Yellow);
    color_method!(/// Set the foreground to the terminal's blue.
        blue => fg, Blue);
    color_method!(/// Set the foreground to the terminal's magenta.
        magenta => fg, Magenta);
    color_method!(/// Set the foreground to the terminal's cyan.
        cyan => fg, Cyan);
    color_method!(/// Set the foreground to the terminal's white.
        white => fg, White);
    color_method!(/// Set the foreground to the terminal's bright black — the
        /// conventional colour for de-emphasised text.
        bright_black => fg, BrightBlack);
    color_method!(/// Set the foreground to the terminal's bright red.
        bright_red => fg, BrightRed);
    color_method!(/// Set the foreground to the terminal's bright green.
        bright_green => fg, BrightGreen);
    color_method!(/// Set the foreground to the terminal's bright yellow.
        bright_yellow => fg, BrightYellow);
    color_method!(/// Set the foreground to the terminal's bright blue.
        bright_blue => fg, BrightBlue);
    color_method!(/// Set the foreground to the terminal's bright magenta.
        bright_magenta => fg, BrightMagenta);
    color_method!(/// Set the foreground to the terminal's bright cyan.
        bright_cyan => fg, BrightCyan);
    color_method!(/// Set the foreground to the terminal's bright white.
        bright_white => fg, BrightWhite);

    color_method!(/// Set the background to the terminal's black.
        on_black => bg, Black);
    color_method!(/// Set the background to the terminal's red.
        on_red => bg, Red);
    color_method!(/// Set the background to the terminal's green.
        on_green => bg, Green);
    color_method!(/// Set the background to the terminal's yellow.
        on_yellow => bg, Yellow);
    color_method!(/// Set the background to the terminal's blue.
        on_blue => bg, Blue);
    color_method!(/// Set the background to the terminal's magenta.
        on_magenta => bg, Magenta);
    color_method!(/// Set the background to the terminal's cyan.
        on_cyan => bg, Cyan);
    color_method!(/// Set the background to the terminal's white.
        on_white => bg, White);

    attribute_method!(/// Render the text in bold.
        bold => BOLD);
    attribute_method!(/// Render the text dimmed. Not every terminal implements
        /// this; those that do not simply ignore it.
        dim => DIM);
    attribute_method!(/// Render the text in italics, where the terminal supports
        /// it.
        italic => ITALIC);
    attribute_method!(/// Underline the text.
        underline => UNDERLINE);
    attribute_method!(/// Blink the text. Widely disabled, and worth avoiding for
        /// anything the reader must actually read.
        blink => BLINK);
    attribute_method!(/// Swap the foreground and background colours.
        reverse => REVERSE);
    attribute_method!(/// Conceal the text — present and selectable, but not
        /// drawn. Intended for secrets echoed into a transcript, and not a
        /// security measure: the bytes are still in the stream.
        hidden => HIDDEN);
    attribute_method!(/// Strike the text through.
        strike => STRIKE);

    /// Set the foreground to any [`Color`], including a palette index or an exact
    /// 24-bit value.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{Color, style};
    ///
    /// let brand = style("forge").fg(Color::Rgb(0x3b, 0x82, 0xf6));
    /// assert!(brand.render().contains("forge"));
    /// ```
    #[must_use]
    pub fn fg(mut self, color: Color) -> Style {
        self.attrs.fg = Some(color);
        self
    }

    /// Set the background to any [`Color`].
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{Color, style};
    ///
    /// let badge = style(" PASS ").black().bg(Color::Green).bold();
    /// assert!(badge.render().contains("PASS"));
    /// ```
    #[must_use]
    pub fn bg(mut self, color: Color) -> Style {
        self.attrs.bg = Some(color);
        self
    }

    /// Set the foreground to a 24-bit hex colour, e.g. `"#ff8800"` or `"#f80"`.
    ///
    /// The leading `#` is optional. An invalid string leaves the colour
    /// unchanged, so the builder never fails — a typo in a palette must not take
    /// a program down. On terminals without 24-bit support the colour is
    /// downgraded at render time to the nearest value they can show.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::style;
    ///
    /// let link = style("https://example.com").hex("#3b82f6").underline();
    /// assert!(link.render().contains("https://example.com"));
    ///
    /// // An invalid hex string is ignored rather than panicking.
    /// assert_eq!(style("x").hex("nope").render(), "x");
    /// ```
    #[must_use]
    pub fn hex(mut self, hex: &str) -> Style {
        if let Some(color) = Color::from_hex(hex) {
            self.attrs.fg = Some(color);
        }
        self
    }

    /// Set the background to a 24-bit hex colour. Invalid strings are ignored,
    /// as in [`hex`](Style::hex).
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::style;
    ///
    /// let badge = style(" NEW ").on_hex("#3b82f6").white().bold();
    /// assert!(badge.render().contains("NEW"));
    /// ```
    #[must_use]
    pub fn on_hex(mut self, hex: &str) -> Style {
        if let Some(color) = Color::from_hex(hex) {
            self.attrs.bg = Some(color);
        }
        self
    }

    /// Set the foreground to a 24-bit RGB colour.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::style;
    ///
    /// let teal = style("ok").rgb(0, 200, 120);
    /// assert!(teal.render().contains("ok"));
    /// ```
    #[must_use]
    pub fn rgb(mut self, r: u8, g: u8, b: u8) -> Style {
        self.attrs.fg = Some(Color::Rgb(r, g, b));
        self
    }

    /// Set the background to a 24-bit RGB colour.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::style;
    ///
    /// let bar = style("  ").on_rgb(0, 200, 120);
    /// assert_eq!(bar.render().contains("  "), true);
    /// ```
    #[must_use]
    pub fn on_rgb(mut self, r: u8, g: u8, b: u8) -> Style {
        self.attrs.bg = Some(Color::Rgb(r, g, b));
        self
    }

    /// Set the foreground to a 256-colour palette index.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::style;
    ///
    /// let orange = style("warn").ansi(208);
    /// assert!(orange.render().contains("warn"));
    /// ```
    #[must_use]
    pub fn ansi(mut self, index: u8) -> Style {
        self.attrs.fg = Some(Color::Ansi(index));
        self
    }

    /// Set the background to a 256-colour palette index.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::style;
    ///
    /// let cell = style(" ").on_ansi(237);
    /// assert_eq!(cell.render().contains(' '), true);
    /// ```
    #[must_use]
    pub fn on_ansi(mut self, index: u8) -> Style {
        self.attrs.bg = Some(Color::Ansi(index));
        self
    }

    /// Put `prefix` immediately before the text, inside the styling.
    ///
    /// This is what lets a marker be described once instead of rebuilt at every
    /// call site: the glyph, its colour, and its spacing travel together as one
    /// value.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Style;
    ///
    /// let ok = Style::new().green().prefix("✓ ");
    /// assert_eq!(cli_forge::text::strip(&ok.paint("done").to_string()), "✓ done");
    /// ```
    #[must_use]
    pub fn prefix(mut self, prefix: impl Into<String>) -> Style {
        self.prefix = Some(prefix.into().into_boxed_str());
        self
    }

    /// Put `suffix` immediately after the text, inside the styling.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Style;
    ///
    /// let quoted = Style::new().prefix("\"").suffix("\"");
    /// assert_eq!(quoted.paint("x").to_string(), "\"x\"");
    /// ```
    #[must_use]
    pub fn suffix(mut self, suffix: impl Into<String>) -> Style {
        self.suffix = Some(suffix.into().into_boxed_str());
        self
    }

    /// Pad the styled content out to `columns` display columns.
    ///
    /// The width is measured in columns, not bytes, so a column of labels stays
    /// straight whether the content is ASCII, accented, CJK, or already styled —
    /// which is the thing `format!("{:<width$}")` cannot do. Prefix and suffix
    /// count toward the width, and content wider than `columns` is left alone
    /// rather than cut (use [`text::truncate`](crate::text::truncate) when the
    /// budget is hard).
    ///
    /// Unlike the rest of the builder, this costs an allocation per render: the
    /// finished text has to be measured before the padding can be known.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{text, Style};
    ///
    /// let label = Style::new().bold().pad_to(8);
    /// assert_eq!(text::width(&label.paint("name").to_string()), 8);
    /// ```
    #[must_use]
    pub fn pad_to(mut self, columns: u16) -> Style {
        let align = self.pad.map_or(Align::Left, |(_, align)| align);
        self.pad = Some((columns, align));
        self
    }

    /// Set how padded content sits in its field. Without [`pad_to`](Style::pad_to)
    /// this has no effect.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{text::Align, Style};
    ///
    /// let number = Style::new().pad_to(6).align(Align::Right);
    /// assert_eq!(number.paint("42").to_string(), "    42");
    /// ```
    #[must_use]
    pub fn align(mut self, align: Align) -> Style {
        let columns = self.pad.map_or(0, |(columns, _)| columns);
        self.pad = Some((columns, align));
        self
    }

    /// Make the text a hyperlink to `url`, using the OSC 8 sequence.
    ///
    /// Terminals that support it render the text as clickable; terminals that do
    /// not ignore the sequence and show the text unchanged, so this is always
    /// safe to add. It is the one way to put a long URL in output without making
    /// the user read it.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{text, Style};
    ///
    /// let docs = Style::new().cyan().underline().link("https://docs.rs/cli-forge");
    /// let rendered = docs.paint("the documentation").to_string();
    /// // Invisible either way: the link adds no columns.
    /// assert_eq!(text::width(&rendered), "the documentation".len());
    /// ```
    #[must_use]
    pub fn link(mut self, url: impl Into<String>) -> Style {
        self.link = Some(url.into().into_boxed_str());
        self
    }

    /// Overlay `other` onto this style: anything `other` sets wins, anything it
    /// leaves unset is kept.
    ///
    /// This is how a theme is adjusted without being rewritten — take the base
    /// style and layer one change over it.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Style;
    ///
    /// let base = Style::new().green().prefix("✓ ");
    /// // Keep the glyph and the colour, add emphasis.
    /// let loud = base.merge(&Style::new().bold());
    /// assert!(loud.paint("done").to_string().contains("done"));
    /// ```
    #[must_use]
    pub fn merge(mut self, other: &Style) -> Style {
        self.attrs = self.attrs.overlay(other.attrs);
        if other.prefix.is_some() {
            self.prefix = other.prefix.clone();
        }
        if other.suffix.is_some() {
            self.suffix = other.suffix.clone();
        }
        if other.pad.is_some() {
            self.pad = other.pad;
        }
        if other.link.is_some() {
            self.link = other.link.clone();
        }
        if !other.text.is_empty() {
            self.text = other.text.clone();
        }
        self
    }

    /// Whether this style would emit no escape sequences and no decoration — in
    /// which case rendering is a plain copy of the text.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Style;
    ///
    /// assert!(Style::new().is_plain());
    /// assert!(!Style::new().red().is_plain());
    /// ```
    #[must_use]
    pub fn is_plain(&self) -> bool {
        self.attrs.is_empty()
            && self.prefix.is_none()
            && self.suffix.is_none()
            && self.pad.is_none()
            && self.link.is_none()
    }

    /// Apply this style to `value`, producing something printable.
    ///
    /// This is the reuse path: one style, any number of values, no allocation
    /// unless [`pad_to`](Style::pad_to) is set. The style's own text is ignored.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "std")] fn main() {
    /// use cli_forge::{out, Style};
    ///
    /// let field = Style::new().bold().pad_to(12);
    /// out(field.paint("name"));
    /// out(field.paint("description"));
    /// out(field.paint(42));
    /// # }
    /// # #[cfg(not(feature = "std"))] fn main() {}
    /// ```
    #[must_use]
    pub fn paint<T: Display>(&self, value: T) -> Painted<'_, T> {
        Painted {
            style: self,
            value,
            level: None,
        }
    }

    /// Apply this style to `value` at an explicit colour depth, bypassing
    /// detection.
    ///
    /// Useful for writing to a destination whose capability is known
    /// independently, and for tests that need deterministic bytes.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{ColorLevel, Style};
    ///
    /// let s = Style::new().red();
    /// assert_eq!(s.paint_at("x", ColorLevel::Ansi16).to_string(), "\x1b[31mx\x1b[0m");
    /// assert_eq!(s.paint_at("x", ColorLevel::None).to_string(), "x");
    /// ```
    #[must_use]
    pub fn paint_at<T: Display>(&self, value: T, level: ColorLevel) -> Painted<'_, T> {
        Painted {
            style: self,
            value,
            level: Some(level),
        }
    }

    /// Render this style's own text to an owned `String`.
    ///
    /// The colour depth matches the terminal detected for standard output, so on
    /// a pipe or under `NO_COLOR` the result is the plain text. Prefer
    /// [`paint`](Style::paint) or passing the `Style` itself to
    /// [`out`](crate::out), both of which avoid the allocation.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::style;
    ///
    /// assert!(style("ready").green().render().contains("ready"));
    /// ```
    #[must_use]
    pub fn render(&self) -> String {
        format!("{self}")
    }

    /// Render this style's own text at an explicit colour depth.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{style, ColorLevel};
    ///
    /// let bytes = style("ERR").red().bold().render_at(ColorLevel::Ansi16);
    /// assert_eq!(bytes, "\x1b[1;31mERR\x1b[0m");
    /// ```
    #[must_use]
    pub fn render_at(&self, level: ColorLevel) -> String {
        format!("{}", self.paint_at(&self.text, level))
    }

    /// Render this style's own text at the depth detected for `stream`.
    ///
    /// [`Display`] cannot know which stream it is being written to, so it assumes
    /// standard output. Use this when writing diagnostics to standard error and
    /// the two streams may have different capabilities — one redirected, the
    /// other not.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "std")] fn main() {
    /// use cli_forge::{err, style, Stream};
    ///
    /// err(style("failed").red().bold().render_for(Stream::Stderr));
    /// # }
    /// # #[cfg(not(feature = "std"))] fn main() {}
    /// ```
    #[must_use]
    pub fn render_for(&self, stream: Stream) -> String {
        self.render_at(terminal::level(stream))
    }

    /// Write `value` through this style at `level`.
    ///
    /// The one place decoration order is decided: the hyperlink wraps everything
    /// visible, the styling wraps prefix-text-suffix as a unit, and padding is
    /// applied inside the styling so a background colour fills the whole field
    /// (which is what makes a padded badge a solid block rather than a coloured
    /// word in a plain box).
    fn write_value<W: Write>(
        &self,
        w: &mut W,
        value: &dyn Display,
        level: ColorLevel,
    ) -> fmt::Result {
        if let Some(url) = &self.link {
            write!(w, "\x1b]8;;{url}\x07")?;
        }

        match self.pad {
            None => {
                // The common path: no measuring, so nothing is buffered.
                if level.is_none() || self.attrs.is_empty() {
                    self.write_content(w, value)?;
                } else {
                    open(w, self.attrs, level)?;
                    self.write_content(w, value)?;
                    w.write_str(RESET)?;
                }
            }
            Some((columns, align)) => {
                // Padding needs the finished width, so the content is built once
                // and then measured.
                let mut content = String::new();
                self.write_content(&mut content, value)?;
                let padded = text::pad(&content, usize::from(columns), align);
                write_styled(w, self.attrs, &padded, level)?;
            }
        }

        if self.link.is_some() {
            w.write_str("\x1b]8;;\x07")?;
        }
        Ok(())
    }

    /// Write prefix, value, and suffix with no styling of their own.
    fn write_content<W: Write>(&self, w: &mut W, value: &dyn Display) -> fmt::Result {
        if let Some(prefix) = &self.prefix {
            w.write_str(prefix)?;
        }
        write!(w, "{value}")?;
        if let Some(suffix) = &self.suffix {
            w.write_str(suffix)?;
        }
        Ok(())
    }
}

impl Display for Style {
    /// Renders the style's own text at the depth detected for standard output.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write_value(f, &self.text, terminal::level(Stream::Stdout))
    }
}

/// A value with a [`Style`] applied, ready to print.
///
/// Returned by [`Style::paint`]. Holds the style by reference and the value by
/// move, and renders on the way out, so no intermediate string exists.
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "std")] fn main() {
/// use cli_forge::{out, Style};
///
/// let heading = Style::new().bold().underline();
/// out(heading.paint("Summary"));
/// # }
/// # #[cfg(not(feature = "std"))] fn main() {}
/// ```
#[derive(Clone, Copy, Debug)]
pub struct Painted<'a, T> {
    style: &'a Style,
    value: T,
    /// An explicit depth from [`Style::paint_at`]; `None` means detect.
    level: Option<ColorLevel>,
}

impl<T: Display> Display for Painted<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let level = self
            .level
            .unwrap_or_else(|| terminal::level(Stream::Stdout));
        self.style.write_value(f, &self.value, level)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::shim::ToString;
    use crate::text;

    /// Render an attribute set at an explicit level, bypassing detection so the
    /// bytes are deterministic in the test harness.
    fn render_at(attrs: StyleAttrs, text: &str, level: ColorLevel) -> String {
        let mut s = String::new();
        write_styled(&mut s, attrs, text, level).unwrap();
        s
    }

    #[test]
    fn test_empty_style_is_plain_even_with_colour_available() {
        assert_eq!(
            render_at(StyleAttrs::EMPTY, "hello", ColorLevel::TrueColor),
            "hello"
        );
        assert_eq!(
            Style::new()
                .paint_at("x", ColorLevel::TrueColor)
                .to_string(),
            "x"
        );
    }

    #[test]
    fn test_no_colour_strips_all_styling() {
        let loud = Style::new().red().on_white().bold().italic().strike();
        assert_eq!(loud.paint_at("x", ColorLevel::None).to_string(), "x");
    }

    #[test]
    fn test_canonical_parameter_order() {
        // Attributes ascending, then foreground, then background — regardless of
        // the order the builder set them in.
        let s = Style::new().on_white().red().strike().bold();
        assert_eq!(
            s.paint_at("X", ColorLevel::Ansi16).to_string(),
            "\x1b[1;9;31;47mX\x1b[0m"
        );
    }

    #[test]
    fn test_builder_order_does_not_change_bytes() {
        let a = Style::new().red().bold().underline();
        let b = Style::new().underline().bold().red();
        assert_eq!(
            a.paint_at("ERR", ColorLevel::Ansi16).to_string(),
            b.paint_at("ERR", ColorLevel::Ansi16).to_string()
        );
    }

    #[test]
    fn test_every_attribute_has_its_sgr_parameter() {
        let cases = [
            (Style::new().bold(), "1"),
            (Style::new().dim(), "2"),
            (Style::new().italic(), "3"),
            (Style::new().underline(), "4"),
            (Style::new().blink(), "5"),
            (Style::new().reverse(), "7"),
            (Style::new().hidden(), "8"),
            (Style::new().strike(), "9"),
        ];
        for (style, parameter) in cases {
            assert_eq!(
                style.paint_at("x", ColorLevel::Ansi16).to_string(),
                format!("\x1b[{parameter}mx\x1b[0m")
            );
        }
    }

    #[test]
    fn test_single_attribute_has_no_stray_separator() {
        assert_eq!(
            Style::new()
                .bold()
                .paint_at("x", ColorLevel::Ansi16)
                .to_string(),
            "\x1b[1mx\x1b[0m"
        );
        assert_eq!(
            Style::new()
                .red()
                .paint_at("x", ColorLevel::Ansi16)
                .to_string(),
            "\x1b[31mx\x1b[0m"
        );
    }

    #[test]
    fn test_invalid_hex_leaves_colour_unset() {
        assert_eq!(Style::new().hex("zzzzzz").attrs.fg, None);
        assert_eq!(Style::new().on_hex("nope").attrs.bg, None);
        assert_eq!(
            Style::new().hex("#abcdef").attrs.fg,
            Some(Color::Rgb(171, 205, 239))
        );
    }

    #[test]
    fn test_prefix_and_suffix_live_inside_the_styling() {
        let s = Style::new().green().prefix("[").suffix("]");
        assert_eq!(
            s.paint_at("ok", ColorLevel::Ansi16).to_string(),
            "\x1b[32m[ok]\x1b[0m"
        );
    }

    #[test]
    fn test_padding_measures_columns_and_fills_the_background() {
        let s = Style::new().on_green().pad_to(6);
        let rendered = s.paint_at("ok", ColorLevel::Ansi16).to_string();
        // The whole field is inside one styled span, so a background fills it.
        assert_eq!(rendered, "\x1b[42mok    \x1b[0m");
        assert_eq!(text::width(&rendered), 6);
    }

    #[test]
    fn test_padding_counts_the_prefix() {
        let s = Style::new().prefix("✓ ").pad_to(8);
        let rendered = s.paint_at("ok", ColorLevel::None).to_string();
        assert_eq!(text::width(&rendered), 8);
        assert!(rendered.starts_with("✓ ok"));
    }

    #[test]
    fn test_padding_alignment() {
        assert_eq!(
            Style::new()
                .pad_to(6)
                .align(text::Align::Right)
                .paint_at("42", ColorLevel::None)
                .to_string(),
            "    42"
        );
        assert_eq!(
            Style::new()
                .pad_to(6)
                .align(text::Align::Center)
                .paint_at("42", ColorLevel::None)
                .to_string(),
            "  42  "
        );
        // Order does not matter: `align` before `pad_to` keeps the alignment.
        assert_eq!(
            Style::new()
                .align(text::Align::Right)
                .pad_to(6)
                .paint_at("42", ColorLevel::None)
                .to_string(),
            "    42"
        );
    }

    #[test]
    fn test_padding_never_truncates() {
        let s = Style::new().pad_to(2);
        assert_eq!(
            s.paint_at("far too long", ColorLevel::None).to_string(),
            "far too long"
        );
    }

    #[test]
    fn test_hyperlink_wraps_the_text_and_adds_no_columns() {
        let s = Style::new().link("https://example.com");
        let rendered = s.paint_at("click", ColorLevel::Ansi16).to_string();
        assert_eq!(rendered, "\x1b]8;;https://example.com\x07click\x1b]8;;\x07");
        assert_eq!(text::width(&rendered), 5);
        assert_eq!(text::strip(&rendered), "click");
    }

    #[test]
    fn test_paint_reuses_one_style_for_many_values() {
        let field = Style::new().bold().pad_to(6);
        assert_eq!(field.paint_at("a", ColorLevel::None).to_string(), "a     ");
        assert_eq!(field.paint_at(42, ColorLevel::None).to_string(), "42    ");
        assert_eq!(field.paint_at(1.5, ColorLevel::None).to_string(), "1.5   ");
    }

    #[test]
    fn test_merge_overlays_only_what_is_set() {
        let base = Style::new().green().prefix("✓ ");
        let merged = base.clone().merge(&Style::new().bold());
        // Colour and prefix survive; bold is added.
        assert_eq!(
            merged.paint_at("x", ColorLevel::Ansi16).to_string(),
            "\x1b[1;32m✓ x\x1b[0m"
        );
        // An explicit colour in the overlay wins.
        let recoloured = base.merge(&Style::new().red());
        assert_eq!(
            recoloured.paint_at("x", ColorLevel::Ansi16).to_string(),
            "\x1b[31m✓ x\x1b[0m"
        );
    }

    #[test]
    fn test_is_plain_tracks_every_kind_of_decoration() {
        assert!(Style::new().is_plain());
        assert!(style("text").is_plain());
        assert!(!Style::new().red().is_plain());
        assert!(!Style::new().bold().is_plain());
        assert!(!Style::new().prefix("x").is_plain());
        assert!(!Style::new().suffix("x").is_plain());
        assert!(!Style::new().pad_to(2).is_plain());
        assert!(!Style::new().link("x").is_plain());
    }

    #[test]
    fn test_render_at_uses_the_styles_own_text() {
        assert_eq!(
            style("ERR").red().bold().render_at(ColorLevel::Ansi16),
            "\x1b[1;31mERR\x1b[0m"
        );
        assert_eq!(style("ERR").red().render_at(ColorLevel::None), "ERR");
    }

    #[test]
    fn test_attrs_overlay_keeps_inherited_values() {
        let base = StyleAttrs {
            fg: Some(Color::Red),
            bg: None,
            flags: BOLD,
        };
        let over = StyleAttrs {
            fg: None,
            bg: Some(Color::White),
            flags: UNDERLINE,
        };
        let merged = base.overlay(over);
        assert_eq!(merged.fg, Some(Color::Red));
        assert_eq!(merged.bg, Some(Color::White));
        assert_eq!(merged.flags, BOLD | UNDERLINE);
    }

    #[test]
    fn test_attribute_flags_occupy_distinct_bits() {
        let mut seen = 0u8;
        for &(flag, _) in &ATTRIBUTES {
            assert_eq!(seen & flag, 0, "duplicate attribute bit {flag}");
            seen |= flag;
        }
        // All eight bits of the flag byte are accounted for.
        assert_eq!(seen, u8::MAX);
    }

    #[test]
    fn test_attribute_parameters_are_the_sgr_codes() {
        // The table replaced integer formatting, so a typo in it would now be a
        // silently wrong escape sequence rather than a compile error.
        let expected = ["1", "2", "3", "4", "5", "7", "8", "9"];
        let actual: crate::shim::Vec<&str> = ATTRIBUTES.iter().map(|&(_, p)| p).collect();
        assert_eq!(actual, expected);
    }
}

#[cfg(test)]
mod proptests {
    use proptest::prelude::*;

    use super::*;
    use crate::shim::ToString;

    proptest! {
        /// A styled run always measures exactly its visible content, whatever
        /// the text and whatever the depth.
        #[test]
        fn test_styled_width_equals_plain_width(
            input in "[^\\x00-\\x1f]{0,30}",
            bold in any::<bool>(),
            fg in 0u8..=255,
            bg in 0u8..=255,
        ) {
            let mut s = Style::new().ansi(fg).on_ansi(bg);
            if bold {
                s = s.bold();
            }
            for level in [ColorLevel::None, ColorLevel::Ansi16, ColorLevel::Ansi256, ColorLevel::TrueColor] {
                let rendered = s.paint_at(&input, level).to_string();
                prop_assert_eq!(crate::text::width(&rendered), crate::text::width(&input));
                prop_assert_eq!(crate::text::strip(&rendered), input.as_str());
            }
        }

        /// A padded field reaches its budget for any content that fits.
        #[test]
        fn test_padded_width_is_exact(input in "[a-z]{0,8}", columns in 0u16..24) {
            let s = Style::new().red().pad_to(columns);
            let rendered = s.paint_at(&input, ColorLevel::TrueColor).to_string();
            prop_assert_eq!(
                crate::text::width(&rendered),
                crate::text::width(&input).max(usize::from(columns))
            );
        }

        /// Rendering never panics and always terminates its escape sequences,
        /// so a style can never leak into the next line of output.
        #[test]
        fn test_rendering_is_balanced(input in ".{0,30}", flags in 0u8..=255) {
            let mut s = Style::new().red();
            for (i, &(flag, _)) in ATTRIBUTES.iter().enumerate() {
                if flags & (1 << i) != 0 {
                    s.attrs.flags |= flag;
                }
            }
            let rendered = s.paint_at(&input, ColorLevel::TrueColor).to_string();
            prop_assert!(rendered.starts_with("\x1b["));
            prop_assert!(rendered.ends_with("\x1b[0m"));
        }
    }
}
