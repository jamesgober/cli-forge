//! The terminal backend.
//!
//! Every styled byte cli-forge emits goes through one decision made here: how
//! much color the current terminal can render. The rest of the crate never asks
//! "are we on Windows?" or "is this a pipe?" — it asks [`color_level`] once and
//! renders accordingly. Isolating the platform and capability logic in this one
//! module is what lets the public API stay identical on Linux, macOS, and the
//! Windows console.
//!
//! The level is resolved a single time, lazily, from standard output and then
//! cached for the life of the process. Standard output is the reference stream
//! because it is the one a user most often redirects; when it is not a terminal
//! (a pipe or a file) no escape sequences are emitted, so redirected output
//! stays clean. The same resolved level drives styling sent to standard error.

/// How much color a terminal can render.
///
/// The variants are ordered by capability. Rendering downgrades a 24-bit color
/// to the nearest representable value when the level is below [`TrueColor`], and
/// emits no escape sequences at all at [`None`].
///
/// [`TrueColor`]: ColorLevel::TrueColor
/// [`None`]: ColorLevel::None
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(not(feature = "color"), allow(dead_code))]
pub(crate) enum ColorLevel {
    /// No styling. Styled values render as their plain text.
    None,
    /// The 16 standard ANSI colors (SGR 30–37).
    Ansi16,
    /// The 256-color palette (SGR `38;5;n`).
    Ansi256,
    /// 24-bit "true color" (SGR `38;2;r;g;b`).
    TrueColor,
}

impl ColorLevel {
    /// Whether styling is disabled for this level.
    #[inline]
    pub(crate) fn is_none(self) -> bool {
        matches!(self, ColorLevel::None)
    }
}

/// The color capability of the current process, detected once and cached.
///
/// With the `color` feature disabled this is a compile-time constant
/// [`ColorLevel::None`], so the styling code paths fold away to plain writes.
#[cfg(feature = "color")]
pub(crate) fn color_level() -> ColorLevel {
    use std::sync::OnceLock;

    static LEVEL: OnceLock<ColorLevel> = OnceLock::new();
    *LEVEL.get_or_init(detect)
}

/// Plain-output build: styling is unconditionally disabled.
#[cfg(not(feature = "color"))]
#[inline]
pub(crate) fn color_level() -> ColorLevel {
    ColorLevel::None
}

/// Detect the capability from the live environment and, on Windows, enable the
/// console's virtual-terminal processing so escape sequences are interpreted
/// rather than printed.
#[cfg(feature = "color")]
fn detect() -> ColorLevel {
    use std::io::IsTerminal;

    let is_tty = std::io::stdout().is_terminal();
    let no_color = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
    let force = std::env::var_os("CLICOLOR_FORCE").is_some_and(|v| v != "0");
    let term = std::env::var("TERM").ok();
    let colorterm = std::env::var("COLORTERM").ok();

    let level = resolve(
        is_tty,
        no_color,
        force,
        term.as_deref(),
        colorterm.as_deref(),
    );
    if level.is_none() {
        return ColorLevel::None;
    }

    // On Windows the console interprets ANSI only once virtual-terminal mode is
    // turned on; without it the sequences would print literally. If it cannot be
    // enabled we fall back to plain output rather than emit visible garbage.
    if enable_vt() { level } else { ColorLevel::None }
}

/// Pure capability resolution, separated from environment access so the decision
/// table can be unit-tested with explicit inputs.
///
/// Precedence: `CLICOLOR_FORCE` (when truthy) forces color on and overrides both
/// `NO_COLOR` and a non-terminal stream; otherwise `NO_COLOR`, a non-terminal,
/// or `TERM=dumb` each disable color. The level itself comes from `COLORTERM`
/// (`truecolor`/`24bit` ⇒ 24-bit) and `TERM` (`*256color*` ⇒ 256-color),
/// defaulting to the 16 standard colors.
#[cfg(feature = "color")]
fn resolve(
    is_tty: bool,
    no_color: bool,
    force: bool,
    term: Option<&str>,
    colorterm: Option<&str>,
) -> ColorLevel {
    if !force && (no_color || !is_tty || matches!(term, Some("dumb"))) {
        return ColorLevel::None;
    }

    if let Some(ct) = colorterm {
        if ct.eq_ignore_ascii_case("truecolor") || ct.eq_ignore_ascii_case("24bit") {
            return ColorLevel::TrueColor;
        }
    }
    if term.is_some_and(|t| t.contains("256color")) {
        return ColorLevel::Ansi256;
    }
    ColorLevel::Ansi16
}

/// Enable ANSI processing on the Windows console. Returns whether color is
/// usable afterwards.
#[cfg(all(feature = "color", windows))]
fn enable_vt() -> bool {
    enable_ansi_support::enable_ansi_support().is_ok()
}

/// Unix terminals interpret ANSI natively; nothing to enable.
#[cfg(all(feature = "color", not(windows)))]
fn enable_vt() -> bool {
    true
}

#[cfg(all(test, feature = "color"))]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_non_terminal_disables_color() {
        assert_eq!(
            resolve(
                false,
                false,
                false,
                Some("xterm-256color"),
                Some("truecolor")
            ),
            ColorLevel::None
        );
    }

    #[test]
    fn test_resolve_no_color_disables_even_on_tty() {
        assert_eq!(
            resolve(true, true, false, Some("xterm-256color"), None),
            ColorLevel::None
        );
    }

    #[test]
    fn test_resolve_clicolor_force_overrides_no_color_and_pipe() {
        // Forced on despite NO_COLOR and a non-terminal stream.
        assert_eq!(
            resolve(false, true, true, Some("xterm-256color"), None),
            ColorLevel::Ansi256
        );
    }

    #[test]
    fn test_resolve_dumb_terminal_disables_color() {
        assert_eq!(
            resolve(true, false, false, Some("dumb"), Some("truecolor")),
            ColorLevel::None
        );
    }

    #[test]
    fn test_resolve_truecolor_from_colorterm() {
        assert_eq!(
            resolve(true, false, false, Some("xterm"), Some("24bit")),
            ColorLevel::TrueColor
        );
        assert_eq!(
            resolve(true, false, false, Some("xterm"), Some("TrueColor")),
            ColorLevel::TrueColor
        );
    }

    #[test]
    fn test_resolve_256_from_term() {
        assert_eq!(
            resolve(true, false, false, Some("screen-256color"), None),
            ColorLevel::Ansi256
        );
    }

    #[test]
    fn test_resolve_defaults_to_ansi16_on_plain_tty() {
        assert_eq!(
            resolve(true, false, false, Some("xterm"), None),
            ColorLevel::Ansi16
        );
        assert_eq!(resolve(true, false, false, None, None), ColorLevel::Ansi16);
    }
}
