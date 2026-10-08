//! The terminal backend: capability detection, explicit overrides, and geometry.
//!
//! Every styled byte cli-forge emits goes through one decision made here: how
//! much colour the destination stream can render. The rest of the crate never
//! asks "are we on Windows?" or "is this a pipe?" — it asks [`level`] for a
//! [`Stream`] and renders accordingly. Isolating the platform and capability
//! logic in this one module is what lets the public API stay identical on Linux,
//! macOS, and the Windows console.
//!
//! Three properties distinguish this backend from a plain `is_terminal()` check:
//!
//! 1. **Per-stream.** Standard output and standard error are detected
//!    independently, because redirecting one does not redirect the other. A
//!    program whose data is piped into a file while its diagnostics still go to
//!    the terminal gets clean bytes in the file and colour on screen; the
//!    single-stream shortcut necessarily gets one of the two wrong.
//! 2. **Overridable.** [`set_color_choice`] and [`set_level`] let the program —
//!    or its `--color` flag — decide, which detection alone can never express.
//!    An override applies immediately and discards any cached answer.
//! 3. **Cached without freezing.** Detection is memoised per stream in a relaxed
//!    atomic, so the hot path is one integer load, yet an override can still
//!    reset it. A `OnceLock` would make the first observation permanent, which is
//!    exactly what makes a styled program untestable.
//!
//! Without the `std` feature there is nothing to detect — no streams, no
//! environment — so detection compiles out and the level is whatever an explicit
//! override says, defaulting to [`ColorLevel::None`]. That is what makes the
//! styling core usable where the caller knows the sink's capability and says so.

use core::sync::atomic::{AtomicU8, Ordering};

/// How much colour a terminal can render.
///
/// The variants are ordered by capability, so they compare: `Ansi16 < TrueColor`.
/// Rendering downgrades a colour the level cannot express to the nearest one it
/// can (see [`Color`](crate::Color)), and emits no escape sequences at all at
/// [`None`](ColorLevel::None).
///
/// # Examples
///
/// ```
/// use cli_forge::{ColorLevel, Stream, terminal};
///
/// // Detection is per-stream; a pipe on one side does not silence the other.
/// let _ = terminal::level(Stream::Stdout);
///
/// assert!(ColorLevel::Ansi16 < ColorLevel::TrueColor);
/// assert!(ColorLevel::None.is_none());
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub enum ColorLevel {
    /// No styling. Styled values render as their plain text.
    #[default]
    None,
    /// The 16 standard ANSI colours (SGR 30–37 and the bright 90–97).
    Ansi16,
    /// The 256-colour palette (SGR `38;5;n`).
    Ansi256,
    /// 24-bit "true colour" (SGR `38;2;r;g;b`).
    TrueColor,
}

impl ColorLevel {
    /// Whether styling is disabled at this level.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::ColorLevel;
    ///
    /// assert!(ColorLevel::None.is_none());
    /// assert!(!ColorLevel::Ansi16.is_none());
    /// ```
    #[inline]
    #[must_use]
    pub const fn is_none(self) -> bool {
        matches!(self, ColorLevel::None)
    }

    /// Encode as the byte stored in the capability cache. `0` is reserved for
    /// "not yet detected", so every level is stored one higher.
    #[inline]
    const fn to_cache(self) -> u8 {
        match self {
            ColorLevel::None => 1,
            ColorLevel::Ansi16 => 2,
            ColorLevel::Ansi256 => 3,
            ColorLevel::TrueColor => 4,
        }
    }

    /// Decode a cache byte, or `None` for the "not yet detected" sentinel.
    #[inline]
    const fn from_cache(byte: u8) -> Option<ColorLevel> {
        match byte {
            1 => Some(ColorLevel::None),
            2 => Some(ColorLevel::Ansi16),
            3 => Some(ColorLevel::Ansi256),
            4 => Some(ColorLevel::TrueColor),
            _ => None,
        }
    }
}

/// Which output stream a capability question is about.
///
/// Standard output and standard error are detected separately: a program whose
/// data is piped while its diagnostics stay on the terminal should emit clean
/// bytes into the pipe and colour onto the screen.
///
/// # Examples
///
/// ```
/// use cli_forge::{Stream, terminal};
///
/// let data = terminal::level(Stream::Stdout);
/// let diagnostics = terminal::level(Stream::Stderr);
/// // Independent: redirecting one stream does not change the other's answer.
/// let _ = (data, diagnostics);
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Stream {
    /// Standard output — the program's data.
    #[default]
    Stdout,
    /// Standard error — the program's diagnostics.
    Stderr,
}

/// What the program wants done about colour, overriding detection.
///
/// This is the type a `--color` flag parses into. [`Auto`](ColorChoice::Auto) is
/// the default and defers to detection and the environment; the other two settle
/// the question outright.
///
/// # Examples
///
/// ```
/// use cli_forge::{ColorChoice, Stream, terminal};
///
/// // What `--color=never` should do.
/// terminal::set_color_choice(ColorChoice::Never);
/// assert!(terminal::level(Stream::Stdout).is_none());
///
/// terminal::set_color_choice(ColorChoice::Auto);
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum ColorChoice {
    /// Detect from the stream and the environment. The default.
    #[default]
    Auto,
    /// Always style, even into a pipe or a file.
    Always,
    /// Never style, even on a capable terminal.
    Never,
}

impl ColorChoice {
    /// Encode for the atomic the choice is stored in.
    #[inline]
    const fn to_byte(self) -> u8 {
        match self {
            ColorChoice::Auto => 0,
            ColorChoice::Always => 1,
            ColorChoice::Never => 2,
        }
    }

    /// Decode from the atomic, treating any unexpected byte as the default.
    #[inline]
    const fn from_byte(byte: u8) -> ColorChoice {
        match byte {
            1 => ColorChoice::Always,
            2 => ColorChoice::Never,
            _ => ColorChoice::Auto,
        }
    }
}

/// The program-wide colour choice. Read on every styled render, so it is a
/// relaxed atomic load rather than a lock.
static CHOICE: AtomicU8 = AtomicU8::new(0);

/// Detected capability per stream, `0` until first detected. Indexed through
/// [`cache_of`].
static CACHE: [AtomicU8; 2] = [AtomicU8::new(0), AtomicU8::new(0)];

/// A level forced by [`set_level`], `0` when none is forced.
static FORCED: AtomicU8 = AtomicU8::new(0);

/// The cache slot for `stream`.
#[inline]
fn cache_of(stream: Stream) -> &'static AtomicU8 {
    match stream {
        Stream::Stdout => &CACHE[0],
        Stream::Stderr => &CACHE[1],
    }
}

/// Override what the program does about colour.
///
/// Takes effect immediately for every subsequent render and discards any cached
/// detection, so a `--color` flag parsed at startup governs all later output.
/// This is the escape hatch detection cannot provide: forcing colour into a pipe
/// (for a pager that understands escapes), or suppressing it on a capable
/// terminal.
///
/// # Examples
///
/// ```
/// use cli_forge::{ColorChoice, Stream, terminal};
///
/// terminal::set_color_choice(ColorChoice::Always);
/// assert!(!terminal::level(Stream::Stdout).is_none());
///
/// terminal::set_color_choice(ColorChoice::Never);
/// assert!(terminal::level(Stream::Stdout).is_none());
///
/// terminal::set_color_choice(ColorChoice::Auto);
/// ```
pub fn set_color_choice(choice: ColorChoice) {
    CHOICE.store(choice.to_byte(), Ordering::Relaxed);
    invalidate();
}

/// The current colour choice.
///
/// # Examples
///
/// ```
/// use cli_forge::{ColorChoice, terminal};
///
/// terminal::set_color_choice(ColorChoice::Never);
/// assert_eq!(terminal::color_choice(), ColorChoice::Never);
/// terminal::set_color_choice(ColorChoice::Auto);
/// ```
#[must_use]
pub fn color_choice() -> ColorChoice {
    ColorChoice::from_byte(CHOICE.load(Ordering::Relaxed))
}

/// Force an exact colour depth for every stream, bypassing detection.
///
/// Use this when the destination's capability is known from outside: a
/// capability-negotiated remote console, a recorded session, a sink in a build
/// without `std`, or a test that must produce deterministic bytes. Passing
/// [`ColorLevel::None`] is equivalent to [`ColorChoice::Never`].
///
/// # Examples
///
/// ```
/// use cli_forge::{ColorLevel, Stream, terminal};
///
/// terminal::set_level(ColorLevel::Ansi256);
/// assert_eq!(terminal::level(Stream::Stdout), ColorLevel::Ansi256);
/// assert_eq!(terminal::level(Stream::Stderr), ColorLevel::Ansi256);
///
/// terminal::clear_level();
/// ```
pub fn set_level(level: ColorLevel) {
    FORCED.store(level.to_cache(), Ordering::Relaxed);
    invalidate();
}

/// Drop a level forced by [`set_level`] and return to detection.
///
/// # Examples
///
/// ```
/// use cli_forge::{ColorLevel, terminal};
///
/// terminal::set_level(ColorLevel::TrueColor);
/// terminal::clear_level();
/// // Back to whatever the environment says.
/// let _ = terminal::level(cli_forge::Stream::Stdout);
/// ```
pub fn clear_level() {
    FORCED.store(0, Ordering::Relaxed);
    invalidate();
}

/// Discard cached detection so the next query re-reads the environment.
///
/// Rarely needed directly — every override calls it — but useful after the
/// program itself changes `NO_COLOR` or reopens a stream.
///
/// # Examples
///
/// ```
/// use cli_forge::terminal;
///
/// terminal::invalidate();
/// ```
pub fn invalidate() {
    CACHE[0].store(0, Ordering::Relaxed);
    CACHE[1].store(0, Ordering::Relaxed);
}

/// How much colour `stream` can render, honouring overrides, the environment,
/// and whether the stream is a terminal.
///
/// The answer is detected once per stream and then cached, so this is a single
/// relaxed atomic load on the hot path. Precedence, highest first:
///
/// 1. [`set_level`] — an exact forced depth.
/// 2. [`set_color_choice`] — `Never` wins over everything below; `Always`
///    enables colour even into a pipe.
/// 3. `CLICOLOR_FORCE` / `FORCE_COLOR` set to anything but `0` — force on.
/// 4. `NO_COLOR` set to a non-empty value, or `CLICOLOR=0` — off.
/// 5. `TERM=dumb`, or the stream is not a terminal — off.
///
/// The depth then comes from `COLORTERM` (`truecolor`/`24bit`), Windows Terminal
/// and ConEmu (both true colour), and `TERM` (`*256color*`, `*direct*`),
/// defaulting to the 16 standard colours.
///
/// # Examples
///
/// ```
/// use cli_forge::{Stream, terminal};
///
/// // Under `cargo test` output is captured, so this is typically `None`.
/// let level = terminal::level(Stream::Stdout);
/// if level.is_none() {
///     // Styled values render as plain text.
/// }
/// ```
#[must_use]
pub fn level(stream: Stream) -> ColorLevel {
    let slot = cache_of(stream);
    if let Some(cached) = ColorLevel::from_cache(slot.load(Ordering::Relaxed)) {
        return cached;
    }
    let resolved = resolve(stream);
    slot.store(resolved.to_cache(), Ordering::Relaxed);
    resolved
}

/// Resolve `stream`'s level from the overrides and, with `std`, the environment.
fn resolve(stream: Stream) -> ColorLevel {
    // A forced depth outranks every other input, including `Never`: a caller
    // that names a depth has already decided.
    if let Some(forced) = ColorLevel::from_cache(FORCED.load(Ordering::Relaxed)) {
        return forced;
    }
    match color_choice() {
        ColorChoice::Never => ColorLevel::None,
        ColorChoice::Always => detect_depth_or(ColorLevel::Ansi16),
        ColorChoice::Auto => detect(stream),
    }
}

/// Detect from the live environment and the stream itself.
#[cfg(feature = "std")]
fn detect(stream: Stream) -> ColorLevel {
    use std::io::IsTerminal;

    if env_forces_color() {
        return detect_depth_or(ColorLevel::Ansi16);
    }
    if env_forbids_color() {
        return ColorLevel::None;
    }
    let is_tty = match stream {
        Stream::Stdout => std::io::stdout().is_terminal(),
        Stream::Stderr => std::io::stderr().is_terminal(),
    };
    if !is_tty {
        return ColorLevel::None;
    }
    // The console must interpret ANSI before we emit any; on Windows that means
    // turning virtual-terminal mode on, and failing that we emit nothing rather
    // than visible garbage.
    if !enable_vt() {
        return ColorLevel::None;
    }
    detect_depth_or(ColorLevel::Ansi16)
}

/// Without `std` there is no stream and no environment: the caller must say what
/// the sink can do via [`set_level`] or [`set_color_choice`].
#[cfg(not(feature = "std"))]
fn detect(_stream: Stream) -> ColorLevel {
    ColorLevel::None
}

/// Whether the environment demands colour regardless of the stream.
///
/// `CLICOLOR_FORCE` is the long-standing convention and `FORCE_COLOR` the one the
/// JavaScript ecosystem popularised; honouring both means a CI job that sets
/// either one gets coloured logs.
#[cfg(feature = "std")]
fn env_forces_color() -> bool {
    truthy("CLICOLOR_FORCE") || truthy("FORCE_COLOR")
}

/// Whether the environment forbids colour: `NO_COLOR` set to anything non-empty
/// (per the <https://no-color.org> convention), `CLICOLOR=0`, or a terminal that
/// declares itself incapable.
#[cfg(feature = "std")]
fn env_forbids_color() -> bool {
    if std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()) {
        return true;
    }
    if std::env::var_os("CLICOLOR").is_some_and(|v| v == "0") {
        return true;
    }
    std::env::var_os("TERM").is_some_and(|v| v == "dumb")
}

/// Whether `name` is set to anything other than empty or `0`.
#[cfg(feature = "std")]
fn truthy(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|v| !v.is_empty() && v != "0")
}

/// The depth the environment advertises, falling back to `floor` when it says
/// nothing specific.
#[cfg(feature = "std")]
fn detect_depth_or(floor: ColorLevel) -> ColorLevel {
    if let Some(colorterm) = std::env::var_os("COLORTERM") {
        let exact = colorterm.to_str().is_some_and(|value| {
            value.eq_ignore_ascii_case("truecolor") || value.eq_ignore_ascii_case("24bit")
        });
        if exact {
            return ColorLevel::TrueColor;
        }
    }
    // Windows Terminal and ConEmu both render 24-bit colour but set no
    // `COLORTERM`, so without these two checks they would be treated as 16-colour
    // consoles and lose every exact `hex`/`rgb` value.
    if std::env::var_os("WT_SESSION").is_some() {
        return ColorLevel::TrueColor;
    }
    if std::env::var_os("ConEmuANSI").is_some_and(|value| value == "ON") {
        return ColorLevel::TrueColor;
    }
    if let Some(term) = std::env::var_os("TERM") {
        if let Some(term) = term.to_str() {
            if term.contains("truecolor") || term.contains("direct") {
                return ColorLevel::TrueColor;
            }
            if term.contains("256color") {
                return ColorLevel::Ansi256;
            }
        }
    }
    floor
}

/// Without `std` the environment cannot be read, so a forced choice gets the
/// floor it asked for.
#[cfg(not(feature = "std"))]
fn detect_depth_or(floor: ColorLevel) -> ColorLevel {
    floor
}

/// Enable ANSI processing on the Windows console. Reports whether colour is
/// usable afterwards.
///
/// Delegated to a crate because the crate root is `#![forbid(unsafe_code)]` and
/// the underlying `SetConsoleMode` call requires FFI. The answer is remembered
/// because it is a syscall and cannot change.
#[cfg(all(feature = "std", feature = "color", windows))]
fn enable_vt() -> bool {
    use std::sync::OnceLock;

    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| enable_ansi_support::enable_ansi_support().is_ok())
}

/// Unix terminals interpret ANSI natively; nothing to enable. Also the path for a
/// Windows build without the `color` feature, which emits no escapes at all.
#[cfg(all(feature = "std", not(all(feature = "color", windows))))]
fn enable_vt() -> bool {
    true
}

/// The terminal's size as `(columns, rows)`, or `None` when it cannot be
/// determined.
///
/// `COLUMNS` and `LINES` are honoured first, because a user who exports them
/// means them — and because they are the only answer available when output is a
/// pipe. With the `termsize` feature the real terminal is then queried. Without
/// it, and with nothing in the environment, the answer is `None`: the caller
/// decides what to assume, which [`width_or`] makes convenient.
///
/// # Examples
///
/// ```
/// use cli_forge::terminal;
///
/// match terminal::size() {
///     Some((cols, rows)) => assert!(cols > 0 && rows > 0),
///     None => { /* not a terminal, or no way to ask */ }
/// }
/// ```
#[cfg(feature = "std")]
#[must_use]
pub fn size() -> Option<(u16, u16)> {
    let cols = env_dimension("COLUMNS");
    let rows = env_dimension("LINES");
    if let (Some(cols), Some(rows)) = (cols, rows) {
        return Some((cols, rows));
    }
    #[cfg(feature = "termsize")]
    if let Some((terminal_size::Width(w), terminal_size::Height(h))) =
        terminal_size::terminal_size()
    {
        // An exported dimension still wins over the queried one.
        return Some((cols.unwrap_or(w), rows.unwrap_or(h)));
    }
    None
}

/// Read a positive dimension from an environment variable.
#[cfg(feature = "std")]
fn env_dimension(name: &str) -> Option<u16> {
    std::env::var(name)
        .ok()?
        .trim()
        .parse::<u16>()
        .ok()
        .filter(|&n| n > 0)
}

/// The terminal's width in columns, or `fallback` when it cannot be determined.
///
/// The help engine wraps to this, with 80 as the conventional fallback.
///
/// # Examples
///
/// ```
/// use cli_forge::terminal;
///
/// let width = terminal::width_or(80);
/// assert!(width >= 1);
/// ```
#[cfg(feature = "std")]
#[must_use]
pub fn width_or(fallback: u16) -> u16 {
    size().map_or(fallback, |(cols, _)| cols)
}

/// Whether the destination can be expected to render non-ASCII text.
///
/// Themes consult this to choose between Unicode glyphs (`✓`) and their ASCII
/// stand-ins (`OK`), because a legacy Windows console or a C-locale terminal
/// turns the former into mojibake. There is no portable way to ask a terminal, so
/// this is a deliberately conservative heuristic: a Unicode locale on Unix, or a
/// modern terminal host on Windows. Setting `NO_UNICODE` forces it off, for
/// terminals that claim support their font cannot back.
///
/// Note that this is about the *destination*, not about Rust: a `&str` is always
/// UTF-8 either way.
///
/// # Examples
///
/// ```
/// use cli_forge::terminal;
///
/// let glyph = if terminal::supports_unicode() { "✓" } else { "OK" };
/// assert!(!glyph.is_empty());
/// ```
#[cfg(feature = "std")]
#[must_use]
pub fn supports_unicode() -> bool {
    use std::sync::OnceLock;

    static SUPPORTED: OnceLock<bool> = OnceLock::new();
    *SUPPORTED.get_or_init(|| {
        if std::env::var_os("NO_UNICODE").is_some_and(|value| !value.is_empty()) {
            return false;
        }
        detect_unicode()
    })
}

/// On Windows, only a modern terminal host can be trusted with non-ASCII: the
/// legacy console's code page is usually a single-byte one, and asking which
/// would require FFI.
#[cfg(all(feature = "std", windows))]
fn detect_unicode() -> bool {
    std::env::var_os("WT_SESSION").is_some()
        || std::env::var_os("TERM_PROGRAM").is_some()
        || std::env::var_os("TERM").is_some()
}

/// On Unix the locale decides, and `C`/`POSIX` means ASCII only.
#[cfg(all(feature = "std", not(windows)))]
fn detect_unicode() -> bool {
    for name in ["LC_ALL", "LC_CTYPE", "LANG"] {
        if let Some(value) = std::env::var_os(name) {
            if let Some(value) = value.to_str() {
                if value.is_empty() {
                    continue;
                }
                return value.to_ascii_uppercase().contains("UTF");
            }
        }
    }
    // No locale set at all: assume UTF-8, which modern Unix terminals are.
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Restore the global colour state after a test that perturbs it, so test
    /// order cannot matter even though the state is process-wide.
    struct Restore;

    impl Drop for Restore {
        fn drop(&mut self) {
            clear_level();
            set_color_choice(ColorChoice::Auto);
        }
    }

    #[test]
    fn test_forced_level_applies_to_every_stream() {
        let _restore = Restore;
        set_level(ColorLevel::Ansi256);
        assert_eq!(level(Stream::Stdout), ColorLevel::Ansi256);
        assert_eq!(level(Stream::Stderr), ColorLevel::Ansi256);
    }

    #[test]
    fn test_never_silences_every_stream() {
        let _restore = Restore;
        set_color_choice(ColorChoice::Never);
        assert!(level(Stream::Stdout).is_none());
        assert!(level(Stream::Stderr).is_none());
    }

    #[test]
    fn test_always_enables_colour_without_a_terminal() {
        let _restore = Restore;
        set_color_choice(ColorChoice::Always);
        assert!(!level(Stream::Stdout).is_none());
    }

    #[test]
    fn test_forced_level_outranks_never() {
        let _restore = Restore;
        set_color_choice(ColorChoice::Never);
        set_level(ColorLevel::TrueColor);
        assert_eq!(level(Stream::Stdout), ColorLevel::TrueColor);
    }

    #[test]
    fn test_override_invalidates_a_cached_answer() {
        let _restore = Restore;
        // Resolve and cache one answer, then change the choice and observe that
        // the cache did not freeze it.
        set_color_choice(ColorChoice::Never);
        assert!(level(Stream::Stdout).is_none());
        set_color_choice(ColorChoice::Always);
        assert!(!level(Stream::Stdout).is_none());
    }

    #[test]
    fn test_level_cache_round_trips_every_variant() {
        for expected in [
            ColorLevel::None,
            ColorLevel::Ansi16,
            ColorLevel::Ansi256,
            ColorLevel::TrueColor,
        ] {
            assert_eq!(ColorLevel::from_cache(expected.to_cache()), Some(expected));
        }
        // `0` is the "not yet detected" sentinel and must never decode.
        assert_eq!(ColorLevel::from_cache(0), None);
    }

    #[test]
    fn test_choice_byte_round_trips() {
        for expected in [ColorChoice::Auto, ColorChoice::Always, ColorChoice::Never] {
            assert_eq!(ColorChoice::from_byte(expected.to_byte()), expected);
        }
        // An unexpected byte degrades to the default rather than misbehaving.
        assert_eq!(ColorChoice::from_byte(200), ColorChoice::Auto);
    }

    #[test]
    fn test_levels_are_ordered_by_capability() {
        assert!(ColorLevel::None < ColorLevel::Ansi16);
        assert!(ColorLevel::Ansi16 < ColorLevel::Ansi256);
        assert!(ColorLevel::Ansi256 < ColorLevel::TrueColor);
    }

    /// Geometry needs an operating system to ask, so this only applies to a
    /// build that has one.
    #[cfg(feature = "std")]
    #[test]
    fn test_width_or_returns_a_usable_column_count() {
        assert!(width_or(80) >= 1);
    }
}
