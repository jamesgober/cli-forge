//! Themed, reusable responses: the vocabulary a CLI already speaks.
//!
//! Almost every program prints the same handful of things — something worked,
//! something failed, something is worth warning about, here is a hint — and in
//! most codebases each of those is restyled by hand at every call site. That is
//! the duplication this module exists to delete. A [`Theme`] maps each [`Level`]
//! onto a [`Style`], a glyph, and a stream, once; the free functions [`ok`],
//! [`fail`], [`warn`], [`info`], [`hint`], [`note`], [`debug`], and [`trace`]
//! print through it. Nothing at the call site names a colour:
//!
//! ```
//! # #[cfg(feature = "std")] fn main() {
//! cli_forge::ok("deployed to staging");
//! cli_forge::warn("2 tests skipped");
//! cli_forge::fail("smoke test failed");
//! # }
//! # #[cfg(not(feature = "std"))] fn main() {}
//! ```
//!
//! Replace the theme and all of it changes together:
//!
//! ```
//! # #[cfg(feature = "std")] fn main() {
//! use cli_forge::{Glyphs, Level, Style, Theme};
//!
//! Theme::new()
//!     .set(Level::Success, Style::new().bright_green().bold(), "✓")
//!     .set(Level::Error, Style::new().bright_red().bold(), "✗")
//!     .glyphs(Glyphs::Unicode)
//!     .install();
//! # }
//! # #[cfg(not(feature = "std"))] fn main() {}
//! ```
//!
//! Three things the theme handles that hand-rolled status lines usually get
//! wrong:
//!
//! - **Stream discipline.** Errors, warnings, and diagnostics go to standard
//!   error; data, hints, and notes go to standard output. A program's output
//!   therefore stays pipeable without the program arranging it.
//! - **Glyph fallback.** `✓` becomes `+` where the destination cannot render
//!   non-ASCII, and the ASCII stand-ins are all one column wide, so a column of
//!   status lines stays aligned in either mode.
//! - **Per-stream colour.** Each line is rendered at the depth detected for the
//!   stream it is going to, so colour survives on a terminal and vanishes in a
//!   pipe independently for each.

use core::fmt::Display;

use crate::shim::{Box, String, format};
use crate::style::Style;
use crate::terminal::{ColorLevel, Stream};

/// The kind of thing being reported.
///
/// These are the levels a command-line program actually speaks in, and the keys a
/// [`Theme`] assigns appearance to.
///
/// # Examples
///
/// ```
/// use cli_forge::{Level, Stream};
///
/// // Diagnostics go to standard error so data stays pipeable.
/// assert_eq!(Level::Error.stream(), Stream::Stderr);
/// assert_eq!(Level::Success.stream(), Stream::Stdout);
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Level {
    /// Something finished as intended.
    Success,
    /// Something went wrong.
    Error,
    /// Something is not wrong but deserves attention.
    Warning,
    /// Neutral progress or context.
    Info,
    /// A suggested next action.
    Hint,
    /// An aside worth keeping in the transcript.
    Note,
    /// Detail for someone diagnosing the program.
    Debug,
    /// Fine-grained detail, usually voluminous.
    Trace,
}

impl Level {
    /// Every level, in declaration order. Useful for building a theme in a loop
    /// or rendering a legend.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Level;
    ///
    /// assert_eq!(Level::ALL.len(), 8);
    /// assert_eq!(Level::ALL[0], Level::Success);
    /// ```
    pub const ALL: [Level; 8] = [
        Level::Success,
        Level::Error,
        Level::Warning,
        Level::Info,
        Level::Hint,
        Level::Note,
        Level::Debug,
        Level::Trace,
    ];

    /// This level's slot in a theme's tables.
    #[inline]
    const fn index(self) -> usize {
        match self {
            Level::Success => 0,
            Level::Error => 1,
            Level::Warning => 2,
            Level::Info => 3,
            Level::Hint => 4,
            Level::Note => 5,
            Level::Debug => 6,
            Level::Trace => 7,
        }
    }

    /// The stream this level goes to by default.
    ///
    /// Errors, warnings, and the two diagnostic levels go to standard error;
    /// everything else goes to standard output. That split is what lets a
    /// program's data be piped while its complaints still reach the user. A theme
    /// can override it with [`Theme::stream`].
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{Level, Stream};
    ///
    /// assert_eq!(Level::Warning.stream(), Stream::Stderr);
    /// assert_eq!(Level::Hint.stream(), Stream::Stdout);
    /// ```
    #[must_use]
    pub const fn stream(self) -> Stream {
        match self {
            Level::Error | Level::Warning | Level::Debug | Level::Trace => Stream::Stderr,
            Level::Success | Level::Info | Level::Hint | Level::Note => Stream::Stdout,
        }
    }

    /// A lower-case name for this level, for log prefixes and config files.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Level;
    ///
    /// assert_eq!(Level::Warning.name(), "warning");
    /// ```
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Level::Success => "success",
            Level::Error => "error",
            Level::Warning => "warning",
            Level::Info => "info",
            Level::Hint => "hint",
            Level::Note => "note",
            Level::Debug => "debug",
            Level::Trace => "trace",
        }
    }
}

/// Which glyph set a theme draws its markers from.
///
/// # Examples
///
/// ```
/// use cli_forge::{Glyphs, Level, Theme};
///
/// let unicode = Theme::new().glyphs(Glyphs::Unicode);
/// assert_eq!(unicode.glyph(Level::Success), "✓");
///
/// // The ASCII stand-ins are one column wide, so columns stay aligned.
/// let ascii = Theme::new().glyphs(Glyphs::Ascii);
/// assert_eq!(ascii.glyph(Level::Success), "+");
///
/// // Or no markers at all.
/// assert_eq!(Theme::new().glyphs(Glyphs::None).glyph(Level::Success), "");
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Glyphs {
    /// Choose per destination: Unicode where it will render, ASCII where it will
    /// not. The default, and the right answer for a program that does not know
    /// where it is running.
    #[default]
    Auto,
    /// Always the Unicode markers.
    Unicode,
    /// Always the one-column ASCII stand-ins.
    Ascii,
    /// No markers; the styling alone carries the level.
    None,
}

/// The Unicode and ASCII marker for each level, in [`Level::index`] order.
///
/// Every ASCII stand-in is exactly one column wide, matching its Unicode
/// counterpart, so switching glyph sets never shifts a column.
const GLYPHS: [(&str, &str); 8] = [
    ("✓", "+"),
    ("✗", "x"),
    ("!", "!"),
    ("i", "i"),
    ("→", ">"),
    ("•", "*"),
    ("·", "-"),
    ("·", "."),
];

/// How each [`Level`] looks when it is printed.
///
/// Build one with [`Theme::new`] (sensible defaults) or [`Theme::plain`] (no
/// styling at all), adjust it, and [`install`](Theme::install) it as the process
/// default. A theme is a plain value: it can also be held and used directly,
/// which is what a library that must not disturb the host program's theme should
/// do.
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "std")] fn main() {
/// use cli_forge::{Level, Style, Theme};
///
/// // Adjust two levels and leave the rest alone.
/// let theme = Theme::new()
///     .set(Level::Success, Style::new().green().bold(), "✓")
///     .set(Level::Hint, Style::new().bright_black().italic(), "→");
///
/// assert_eq!(theme.glyph(Level::Success), "✓");
/// theme.install();
/// # }
/// # #[cfg(not(feature = "std"))] fn main() {}
/// ```
#[derive(Clone, Debug)]
pub struct Theme {
    styles: [Style; 8],
    /// A per-level glyph override; `None` means use [`GLYPHS`].
    overrides: [Option<Box<str>>; 8],
    streams: [Stream; 8],
    glyphs: Glyphs,
}

impl Default for Theme {
    fn default() -> Theme {
        Theme::new()
    }
}

impl Theme {
    /// The default theme: conventional colours, Unicode markers where they will
    /// render, and the standard stream split.
    ///
    /// The colours are the terminal's own bright palette rather than exact
    /// values, so they follow whatever scheme the user has chosen — which is
    /// nearly always what a CLI wants, and what a hard-coded hex triple gets
    /// wrong on a light background.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "std")] fn main() {
    /// use cli_forge::Theme;
    ///
    /// Theme::new().install();
    /// cli_forge::ok("ready");
    /// # }
    /// # #[cfg(not(feature = "std"))] fn main() {}
    /// ```
    #[must_use]
    pub fn new() -> Theme {
        Theme {
            styles: [
                Style::new().bright_green().bold(),
                Style::new().bright_red().bold(),
                Style::new().bright_yellow().bold(),
                Style::new().bright_blue(),
                Style::new().cyan(),
                Style::new().bright_black(),
                Style::new().bright_black(),
                Style::new().bright_black().dim(),
            ],
            overrides: [const { None }; 8],
            streams: [
                Level::Success.stream(),
                Level::Error.stream(),
                Level::Warning.stream(),
                Level::Info.stream(),
                Level::Hint.stream(),
                Level::Note.stream(),
                Level::Debug.stream(),
                Level::Trace.stream(),
            ],
            glyphs: Glyphs::Auto,
        }
    }

    /// A theme with no styling and no markers: every level prints its text and
    /// nothing else.
    ///
    /// The stream split is kept, because that is about correctness rather than
    /// appearance. Useful for a program with a `--plain` mode, and for tests that
    /// want to assert on text.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{Level, Theme};
    ///
    /// let plain = Theme::plain();
    /// assert_eq!(plain.render(Level::Success, "done"), "done");
    /// ```
    #[must_use]
    pub fn plain() -> Theme {
        Theme {
            styles: [const { Style::new() }; 8],
            overrides: [const { None }; 8],
            streams: Theme::new().streams,
            glyphs: Glyphs::None,
        }
    }

    /// Set a level's style and marker.
    ///
    /// Pass an empty `glyph` to leave the level unmarked. To keep the default
    /// marker and change only the style, use [`style_for`](Theme::style_for).
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{Level, Style, Theme};
    ///
    /// let theme = Theme::new().set(Level::Error, Style::new().red().bold(), "FAIL");
    /// assert_eq!(theme.glyph(Level::Error), "FAIL");
    /// ```
    #[must_use]
    pub fn set(mut self, level: Level, style: Style, glyph: impl Into<String>) -> Theme {
        self.styles[level.index()] = style;
        self.overrides[level.index()] = Some(glyph.into().into_boxed_str());
        self
    }

    /// Set a level's style, keeping its marker.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{Level, Style, Theme};
    ///
    /// let theme = Theme::new().style_for(Level::Info, Style::new().magenta());
    /// // The default marker is untouched.
    /// assert_eq!(theme.glyph(Level::Info), "i");
    /// ```
    #[must_use]
    pub fn style_for(mut self, level: Level, style: Style) -> Theme {
        self.styles[level.index()] = style;
        self
    }

    /// Send a level to a different stream than its default.
    ///
    /// The one case that comes up often: a program whose warnings belong in its
    /// piped output rather than beside it.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{Level, Stream, Theme};
    ///
    /// let theme = Theme::new().stream(Level::Warning, Stream::Stdout);
    /// assert_eq!(theme.stream_for(Level::Warning), Stream::Stdout);
    /// ```
    #[must_use]
    pub fn stream(mut self, level: Level, stream: Stream) -> Theme {
        self.streams[level.index()] = stream;
        self
    }

    /// Choose which glyph set the markers come from.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{Glyphs, Level, Theme};
    ///
    /// assert_eq!(Theme::new().glyphs(Glyphs::Ascii).glyph(Level::Error), "x");
    /// ```
    #[must_use]
    pub fn glyphs(mut self, glyphs: Glyphs) -> Theme {
        self.glyphs = glyphs;
        self
    }

    /// The style this theme gives `level`.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{Level, Theme};
    ///
    /// let theme = Theme::new();
    /// assert!(!theme.style(Level::Error).is_plain());
    /// assert!(cli_forge::Theme::plain().style(Level::Error).is_plain());
    /// ```
    #[must_use]
    pub fn style(&self, level: Level) -> &Style {
        &self.styles[level.index()]
    }

    /// The stream this theme sends `level` to.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{Level, Stream, Theme};
    ///
    /// assert_eq!(Theme::new().stream_for(Level::Error), Stream::Stderr);
    /// ```
    #[must_use]
    pub fn stream_for(&self, level: Level) -> Stream {
        self.streams[level.index()]
    }

    /// The marker this theme puts before `level`'s text, or `""` for none.
    ///
    /// Under [`Glyphs::Auto`] the answer depends on whether the destination can
    /// render non-ASCII, so it is resolved here rather than stored.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{Glyphs, Level, Theme};
    ///
    /// assert_eq!(Theme::new().glyphs(Glyphs::Unicode).glyph(Level::Success), "✓");
    /// assert_eq!(Theme::new().glyphs(Glyphs::None).glyph(Level::Success), "");
    /// ```
    #[must_use]
    pub fn glyph(&self, level: Level) -> &str {
        if let Some(custom) = &self.overrides[level.index()] {
            return custom;
        }
        let (unicode, ascii) = GLYPHS[level.index()];
        match self.glyphs {
            Glyphs::Unicode => unicode,
            Glyphs::Ascii => ascii,
            Glyphs::None => "",
            Glyphs::Auto => {
                if unicode_destination() {
                    unicode
                } else {
                    ascii
                }
            }
        }
    }

    /// Render one line for `level` at the colour depth detected for its stream.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{Level, Theme};
    ///
    /// let line = Theme::plain().render(Level::Error, "disk full");
    /// assert_eq!(line, "disk full");
    /// ```
    #[must_use]
    pub fn render<T: Display>(&self, level: Level, value: T) -> String {
        self.render_at(level, value, crate::terminal::level(self.stream_for(level)))
    }

    /// Render one line for `level` at an explicit colour depth.
    ///
    /// The marker and the text share a single styled span, so they read as one
    /// unit and a background colour covers both.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{ColorLevel, Level, Theme};
    ///
    /// let theme = Theme::new().glyphs(cli_forge::Glyphs::Unicode);
    /// let line = theme.render_at(Level::Success, "done", ColorLevel::None);
    /// assert_eq!(line, "✓ done");
    /// ```
    #[must_use]
    pub fn render_at<T: Display>(&self, level: Level, value: T, depth: ColorLevel) -> String {
        let glyph = self.glyph(level);
        let content = if glyph.is_empty() {
            format!("{value}")
        } else {
            format!("{glyph} {value}")
        };
        format!("{}", self.style(level).paint_at(&content, depth))
    }

    /// Install this theme as the process default, replacing whatever was there.
    ///
    /// Call it once at startup. Every later [`ok`], [`fail`], and sibling call
    /// renders through it.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "std")] fn main() {
    /// use cli_forge::{Glyphs, Theme};
    ///
    /// Theme::new().glyphs(Glyphs::Ascii).install();
    /// cli_forge::ok("installed");
    /// # Theme::new().install();
    /// # }
    /// # #[cfg(not(feature = "std"))] fn main() {}
    /// ```
    #[cfg(feature = "std")]
    pub fn install(self) {
        if let Ok(mut active) = active().write() {
            *active = self;
        }
        // A poisoned lock means another thread panicked mid-write. Keeping the
        // previous theme is strictly better than panicking in a styling call.
    }

    /// A clone of the process default theme.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "std")] fn main() {
    /// use cli_forge::{Level, Theme};
    ///
    /// let theme = Theme::current();
    /// assert_eq!(theme.stream_for(Level::Error), cli_forge::Stream::Stderr);
    /// # }
    /// # #[cfg(not(feature = "std"))] fn main() {}
    /// ```
    #[cfg(feature = "std")]
    #[must_use]
    pub fn current() -> Theme {
        active()
            .read()
            .map(|theme| theme.clone())
            .unwrap_or_else(|_| Theme::new())
    }
}

/// Whether the destination can be expected to render the Unicode markers.
#[cfg(feature = "std")]
fn unicode_destination() -> bool {
    crate::terminal::supports_unicode()
}

/// Without `std` there is no environment to ask, so the portable answer wins.
#[cfg(not(feature = "std"))]
fn unicode_destination() -> bool {
    false
}

/// The process default theme, created on first use.
#[cfg(feature = "std")]
fn active() -> &'static std::sync::RwLock<Theme> {
    use std::sync::{OnceLock, RwLock};

    static ACTIVE: OnceLock<RwLock<Theme>> = OnceLock::new();
    ACTIVE.get_or_init(|| RwLock::new(Theme::new()))
}

/// Print `value` through the process theme at `level`, to that level's stream.
///
/// Also the route the command layer takes to report a failure, so a program's
/// errors look like the rest of its output rather than like a different tool.
#[cfg(feature = "std")]
pub(crate) fn emit<T: Display>(level: Level, value: T) {
    // The theme is read under a lock, so the line is built here and written
    // afterwards: no I/O happens while the lock is held.
    let (line, stream) = match active().read() {
        Ok(theme) => (theme.render(level, value), theme.stream_for(level)),
        // A poisoned lock must not swallow the message; print it plainly.
        Err(_) => (format!("{value}"), level.stream()),
    };
    crate::output::write_line(stream, &line);
}

/// Generate one themed printer.
macro_rules! printer {
    ($(#[$meta:meta])* $name:ident => $level:ident) => {
        $(#[$meta])*
        #[cfg(feature = "std")]
        pub fn $name<T: Display>(value: T) {
            emit(Level::$level, value);
        }
    };
}

printer!(
    /// Report success, through the process theme, to standard output.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "std")] fn main() {
    /// cli_forge::ok("deployed to staging");
    /// # }
    /// # #[cfg(not(feature = "std"))] fn main() {}
    /// ```
    ok => Success
);
printer!(
    /// Report a failure, through the process theme, to standard error.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "std")] fn main() {
    /// cli_forge::fail("smoke test failed");
    /// # }
    /// # #[cfg(not(feature = "std"))] fn main() {}
    /// ```
    fail => Error
);
printer!(
    /// Report something worth attention, through the process theme, to standard
    /// error.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "std")] fn main() {
    /// cli_forge::warn("2 tests skipped");
    /// # }
    /// # #[cfg(not(feature = "std"))] fn main() {}
    /// ```
    warn => Warning
);
printer!(
    /// Report neutral progress or context, through the process theme, to
    /// standard output.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "std")] fn main() {
    /// cli_forge::info("using cached dependencies");
    /// # }
    /// # #[cfg(not(feature = "std"))] fn main() {}
    /// ```
    info => Info
);
printer!(
    /// Suggest a next action, through the process theme, to standard output.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "std")] fn main() {
    /// cli_forge::hint("try `--release` for an optimised build");
    /// # }
    /// # #[cfg(not(feature = "std"))] fn main() {}
    /// ```
    hint => Hint
);
printer!(
    /// Record an aside, through the process theme, to standard output.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "std")] fn main() {
    /// cli_forge::note("config was read from ./forge.toml");
    /// # }
    /// # #[cfg(not(feature = "std"))] fn main() {}
    /// ```
    note => Note
);
printer!(
    /// Report diagnostic detail, through the process theme, to standard error.
    ///
    /// # Examples
    ///
    /// ```
    /// cli_forge::debug("resolved 41 packages in 12ms");
    /// ```
    debug => Debug
);
printer!(
    /// Report fine-grained detail, through the process theme, to standard error.
    ///
    /// # Examples
    ///
    /// ```
    /// cli_forge::trace("cache hit: libcore-1.0.0");
    /// ```
    trace => Trace
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_every_level_has_a_distinct_slot() {
        let mut seen = [false; 8];
        for level in Level::ALL {
            assert!(!seen[level.index()], "duplicate slot for {level:?}");
            seen[level.index()] = true;
        }
        assert!(seen.iter().all(|&s| s));
    }

    #[test]
    fn test_diagnostics_go_to_standard_error() {
        for level in [Level::Error, Level::Warning, Level::Debug, Level::Trace] {
            assert_eq!(level.stream(), Stream::Stderr, "{level:?}");
        }
        for level in [Level::Success, Level::Info, Level::Hint, Level::Note] {
            assert_eq!(level.stream(), Stream::Stdout, "{level:?}");
        }
    }

    #[test]
    fn test_ascii_glyphs_match_unicode_widths() {
        // Switching glyph sets must never shift a column, so the two sets have
        // to measure the same.
        for (unicode, ascii) in GLYPHS {
            assert_eq!(
                crate::text::width(unicode),
                crate::text::width(ascii),
                "{unicode} and {ascii} differ in width"
            );
        }
    }

    #[test]
    fn test_glyph_modes() {
        let theme = Theme::new();
        assert_eq!(
            theme.clone().glyphs(Glyphs::Unicode).glyph(Level::Success),
            "✓"
        );
        assert_eq!(
            theme.clone().glyphs(Glyphs::Ascii).glyph(Level::Success),
            "+"
        );
        assert_eq!(theme.clone().glyphs(Glyphs::None).glyph(Level::Success), "");
        // `Auto` picks one of the two, never something else.
        let auto = theme.glyphs(Glyphs::Auto);
        assert!(matches!(auto.glyph(Level::Success), "✓" | "+"));
    }

    #[test]
    fn test_custom_glyph_overrides_every_mode() {
        let theme = Theme::new().set(Level::Error, Style::new().red(), "FAIL");
        for mode in [Glyphs::Auto, Glyphs::Unicode, Glyphs::Ascii, Glyphs::None] {
            assert_eq!(theme.clone().glyphs(mode).glyph(Level::Error), "FAIL");
        }
    }

    #[test]
    fn test_empty_custom_glyph_leaves_the_line_unmarked() {
        let theme = Theme::new().set(Level::Info, Style::new(), "");
        assert_eq!(
            theme.render_at(Level::Info, "text", ColorLevel::None),
            "text"
        );
    }

    #[test]
    fn test_render_puts_glyph_and_text_in_one_span() {
        let theme = Theme::new()
            .glyphs(Glyphs::Unicode)
            .style_for(Level::Success, Style::new().green());
        assert_eq!(
            theme.render_at(Level::Success, "done", ColorLevel::Ansi16),
            "\x1b[32m✓ done\x1b[0m"
        );
        assert_eq!(
            theme.render_at(Level::Success, "done", ColorLevel::None),
            "✓ done"
        );
    }

    #[test]
    fn test_plain_theme_renders_text_only() {
        let plain = Theme::plain();
        for level in Level::ALL {
            assert_eq!(plain.render_at(level, "msg", ColorLevel::TrueColor), "msg");
            assert!(plain.style(level).is_plain());
        }
        // The stream split survives, because it is correctness, not appearance.
        assert_eq!(plain.stream_for(Level::Error), Stream::Stderr);
    }

    #[test]
    fn test_stream_override() {
        let theme = Theme::new().stream(Level::Warning, Stream::Stdout);
        assert_eq!(theme.stream_for(Level::Warning), Stream::Stdout);
        // Other levels are untouched.
        assert_eq!(theme.stream_for(Level::Error), Stream::Stderr);
    }

    #[test]
    fn test_render_accepts_any_display_value() {
        let theme = Theme::plain();
        assert_eq!(theme.render_at(Level::Info, 42, ColorLevel::None), "42");
        assert_eq!(theme.render_at(Level::Info, 1.5, ColorLevel::None), "1.5");
        assert_eq!(
            theme.render_at(Level::Info, format_args!("{}+{}", 1, 2), ColorLevel::None),
            "1+2"
        );
    }

    #[test]
    fn test_level_names_are_unique_and_lowercase() {
        let mut names = Level::ALL.map(Level::name).to_vec();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count, "duplicate level name");
        assert!(
            Level::ALL
                .iter()
                .all(|l| l.name().chars().all(char::is_lowercase))
        );
    }

    /// The process default theme lives behind a lock, which needs `std`.
    #[cfg(feature = "std")]
    #[test]
    fn test_install_and_current_round_trip() {
        // Leave the process theme as it was found, so test order cannot matter.
        let saved = Theme::current();
        Theme::new().glyphs(Glyphs::Ascii).install();
        assert_eq!(Theme::current().glyph(Level::Success), "+");
        saved.install();
    }
}
