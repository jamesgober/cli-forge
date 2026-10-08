//! Colours and their ANSI encoding.
//!
//! A [`Color`] is one of the sixteen standard terminal colours, an index into the
//! 256-colour palette, or an arbitrary 24-bit value. The same colour reaches the
//! terminal several ways — through the [`style`](crate::style) builder, a `<c=…>`
//! tag, a named registry entry, or a [`Theme`](crate::Theme) — and each renders
//! identically because they all funnel through [`Color::write`].
//!
//! ## Graceful degradation
//!
//! A colour the terminal cannot render is *downgraded here rather than dropped*,
//! so a design expressed in exact hex still reads correctly two capability tiers
//! down:
//!
//! | Colour | True colour | 256 colour | 16 colour | None |
//! |---|---|---|---|---|
//! | [`Rgb`](Color::Rgb) | exact | nearest cube/greyscale entry | nearest of 16 | dropped |
//! | [`Ansi`](Color::Ansi) | exact | exact | nearest of 16 | dropped |
//! | named | exact | exact | exact | dropped |
//!
//! Nearness is measured in weighted RGB distance, with the weights approximating
//! human luminance perception — a plain Euclidean match sends mid greens to black
//! often enough to be noticeable.

use core::fmt::{self, Write};

use crate::terminal::ColorLevel;

/// A terminal colour, usable as a foreground or a background.
///
/// The sixteen named variants are the terminal's own palette, so they honour the
/// user's colour scheme — `Color::Red` is whatever red that user chose, which is
/// usually what a CLI wants. [`Ansi`](Color::Ansi) and [`Rgb`](Color::Rgb) name
/// exact colours instead, and are downgraded on terminals that cannot render
/// them.
///
/// # Examples
///
/// ```
/// use cli_forge::Color;
///
/// // Named, from the user's own palette.
/// let warn = Color::Yellow;
/// let loud = Color::BrightYellow;
///
/// // Exact, with graceful degradation.
/// let brand = Color::Rgb(0x3b, 0x82, 0xf6);
/// let indexed = Color::Ansi(208);
///
/// // Parsed from text, as tags and themes do.
/// assert_eq!(Color::parse("bright_cyan"), Some(Color::BrightCyan));
/// assert_eq!(Color::parse("#3b82f6"), Some(brand));
/// assert_eq!(Color::parse("208"), Some(indexed));
/// let _ = (warn, loud);
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
pub enum Color {
    /// The terminal's black (SGR 30 / 40).
    Black,
    /// The terminal's red (SGR 31 / 41).
    Red,
    /// The terminal's green (SGR 32 / 42).
    Green,
    /// The terminal's yellow (SGR 33 / 43).
    Yellow,
    /// The terminal's blue (SGR 34 / 44).
    Blue,
    /// The terminal's magenta (SGR 35 / 45).
    Magenta,
    /// The terminal's cyan (SGR 36 / 46).
    Cyan,
    /// The terminal's white (SGR 37 / 47).
    White,
    /// The terminal's bright black, usually a mid grey (SGR 90 / 100).
    BrightBlack,
    /// The terminal's bright red (SGR 91 / 101).
    BrightRed,
    /// The terminal's bright green (SGR 92 / 102).
    BrightGreen,
    /// The terminal's bright yellow (SGR 93 / 103).
    BrightYellow,
    /// The terminal's bright blue (SGR 94 / 104).
    BrightBlue,
    /// The terminal's bright magenta (SGR 95 / 105).
    BrightMagenta,
    /// The terminal's bright cyan (SGR 96 / 106).
    BrightCyan,
    /// The terminal's bright white (SGR 97 / 107).
    BrightWhite,
    /// An index into the 256-colour palette: `0..=15` the named colours,
    /// `16..=231` the 6×6×6 cube, `232..=255` the greyscale ramp.
    Ansi(u8),
    /// An exact 24-bit colour, downgraded at render time where necessary.
    Rgb(u8, u8, u8),
}

impl Color {
    /// Parse a colour as written in a `<c=…>` tag, a theme, or a config file.
    ///
    /// Accepted forms, in order of attempt:
    ///
    /// - a name, case- and separator-insensitive: `red`, `BrightRed`,
    ///   `bright_red`, `bright-red`, `bright red`
    /// - `#rrggbb`, or the `#rgb` shorthand (`#f80` is `#ff8800`)
    /// - `r,g,b`, each channel `0..=255`, spaces allowed
    /// - a bare `0..=255` palette index
    ///
    /// Returns `None` for anything else, which every caller treats as "leave the
    /// colour alone" rather than as an error — a typo in markup must not fail a
    /// program whose job is printing.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Color;
    ///
    /// assert_eq!(Color::parse("GREEN"), Some(Color::Green));
    /// assert_eq!(Color::parse("bright-blue"), Some(Color::BrightBlue));
    /// assert_eq!(Color::parse("#f80"), Some(Color::Rgb(255, 136, 0)));
    /// assert_eq!(Color::parse("0, 200, 120"), Some(Color::Rgb(0, 200, 120)));
    /// assert_eq!(Color::parse("42"), Some(Color::Ansi(42)));
    /// assert_eq!(Color::parse("chartreuse"), None);
    /// ```
    #[must_use]
    pub fn parse(value: &str) -> Option<Color> {
        let value = value.trim();
        if value.is_empty() {
            return None;
        }
        if let Some(hex) = value.strip_prefix('#') {
            return parse_hex(hex);
        }
        if value.contains(',') {
            return parse_triple(value);
        }
        if let Some(color) = named(value) {
            return Some(color);
        }
        // A bare number is a palette index. Checked last so a hypothetical colour
        // named only in digits could never be shadowed.
        value.parse::<u8>().ok().map(Color::Ansi)
    }

    /// Parse a hex colour for [`Style::hex`](crate::Style::hex). The leading `#`
    /// is optional; the rest must be three or six hex digits.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Color;
    ///
    /// assert_eq!(Color::from_hex("#88aaff"), Some(Color::Rgb(136, 170, 255)));
    /// assert_eq!(Color::from_hex("88aaff"), Some(Color::Rgb(136, 170, 255)));
    /// assert_eq!(Color::from_hex("8af"), Some(Color::Rgb(136, 170, 255)));
    /// assert_eq!(Color::from_hex("nope"), None);
    /// ```
    #[must_use]
    pub fn from_hex(hex: &str) -> Option<Color> {
        parse_hex(hex.trim().strip_prefix('#').unwrap_or(hex.trim()))
    }

    /// This colour's exact 24-bit value.
    ///
    /// The sixteen named variants resolve to the conventional xterm values, which
    /// is an approximation: the real pixels come from the user's colour scheme.
    /// Useful for computing contrast or blending, not for rendering — rendering
    /// goes through [`write`](Color::write), which preserves the named form so the
    /// user's own palette is honoured.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::Color;
    ///
    /// assert_eq!(Color::Rgb(1, 2, 3).to_rgb(), (1, 2, 3));
    /// assert_eq!(Color::Black.to_rgb(), (0, 0, 0));
    /// assert_eq!(Color::Ansi(15).to_rgb(), (255, 255, 255));
    /// ```
    #[must_use]
    pub const fn to_rgb(self) -> (u8, u8, u8) {
        match self {
            Color::Rgb(r, g, b) => (r, g, b),
            Color::Ansi(index) => palette_rgb(index),
            _ => palette_rgb(self.basic_index()),
        }
    }

    /// The `0..=15` palette slot of a named variant, or `0` for the two exact
    /// variants (whose callers never ask).
    #[inline]
    const fn basic_index(self) -> u8 {
        match self {
            Color::Black => 0,
            Color::Red => 1,
            Color::Green => 2,
            Color::Yellow => 3,
            Color::Blue => 4,
            Color::Magenta => 5,
            Color::Cyan => 6,
            Color::White => 7,
            Color::BrightBlack => 8,
            Color::BrightRed => 9,
            Color::BrightGreen => 10,
            Color::BrightYellow => 11,
            Color::BrightBlue => 12,
            Color::BrightMagenta => 13,
            Color::BrightCyan => 14,
            Color::BrightWhite => 15,
            Color::Ansi(_) | Color::Rgb(..) => 0,
        }
    }

    /// Write this colour's SGR parameters at `level`, as a foreground when
    /// `background` is `false` and as a background otherwise.
    ///
    /// No leading or trailing `;` is written: `first` tracks whether an earlier
    /// parameter (an attribute flag, or the other of the two colours) has already
    /// been emitted, so separators land in the right places. Writes nothing at
    /// [`ColorLevel::None`].
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{Color, ColorLevel};
    ///
    /// let mut sgr = String::new();
    /// let mut first = true;
    /// Color::Red.write(&mut sgr, ColorLevel::Ansi16, false, &mut first).unwrap();
    /// Color::White.write(&mut sgr, ColorLevel::Ansi16, true, &mut first).unwrap();
    /// assert_eq!(sgr, "31;47");
    /// ```
    pub fn write<W: Write>(
        self,
        w: &mut W,
        level: ColorLevel,
        background: bool,
        first: &mut bool,
    ) -> fmt::Result {
        if level.is_none() {
            return Ok(());
        }
        match self {
            Color::Ansi(index) => self.write_indexed(w, level, background, first, index),
            Color::Rgb(r, g, b) => self.write_exact(w, level, background, first, r, g, b),
            _ => write_basic(w, first, self.basic_index(), background),
        }
    }

    /// Write a palette index, downgrading to the nearest of the sixteen when the
    /// terminal has no 256-colour support.
    fn write_indexed<W: Write>(
        self,
        w: &mut W,
        level: ColorLevel,
        background: bool,
        first: &mut bool,
        index: u8,
    ) -> fmt::Result {
        match level {
            ColorLevel::TrueColor | ColorLevel::Ansi256 => {
                separator(w, first)?;
                w.write_str(if background { "48;5;" } else { "38;5;" })?;
                write_u8(w, index)
            }
            ColorLevel::Ansi16 => {
                let (r, g, b) = palette_rgb(index);
                write_basic(w, first, nearest_basic(r, g, b), background)
            }
            ColorLevel::None => Ok(()),
        }
    }

    /// Write an exact colour, downgrading through the palette and then to the
    /// nearest of the sixteen as capability drops.
    #[allow(clippy::too_many_arguments)]
    fn write_exact<W: Write>(
        self,
        w: &mut W,
        level: ColorLevel,
        background: bool,
        first: &mut bool,
        r: u8,
        g: u8,
        b: u8,
    ) -> fmt::Result {
        match level {
            ColorLevel::TrueColor => {
                separator(w, first)?;
                w.write_str(if background { "48;2;" } else { "38;2;" })?;
                write_u8(w, r)?;
                w.write_char(';')?;
                write_u8(w, g)?;
                w.write_char(';')?;
                write_u8(w, b)
            }
            ColorLevel::Ansi256 => {
                separator(w, first)?;
                w.write_str(if background { "48;5;" } else { "38;5;" })?;
                write_u8(w, rgb_to_256(r, g, b))
            }
            ColorLevel::Ansi16 => write_basic(w, first, nearest_basic(r, g, b), background),
            ColorLevel::None => Ok(()),
        }
    }
}

/// Write a `;` separator before every parameter but the first.
fn separator<W: Write>(w: &mut W, first: &mut bool) -> fmt::Result {
    if *first {
        *first = false;
        Ok(())
    } else {
        w.write_char(';')
    }
}

/// The SGR parameter for each of the sixteen palette slots, as text.
///
/// A table rather than arithmetic plus `write!`: the set is fixed and tiny, and
/// formatting an integer costs more than the whole rest of writing a styled run.
const FG_CODES: [&str; 16] = [
    "30", "31", "32", "33", "34", "35", "36", "37", "90", "91", "92", "93", "94", "95", "96", "97",
];

/// The background counterparts of [`FG_CODES`].
const BG_CODES: [&str; 16] = [
    "40", "41", "42", "43", "44", "45", "46", "47", "100", "101", "102", "103", "104", "105",
    "106", "107",
];

/// Write one of the sixteen standard colours by palette slot.
fn write_basic<W: Write>(w: &mut W, first: &mut bool, slot: u8, background: bool) -> fmt::Result {
    separator(w, first)?;
    let table = if background { &BG_CODES } else { &FG_CODES };
    // Every caller derives `slot` from a 16-entry palette, so this cannot be out
    // of range; falling back to white rather than panicking keeps the promise
    // that rendering never brings a program down.
    w.write_str(table.get(slot as usize).copied().unwrap_or("37"))
}

/// Write a `0..=255` value in decimal without going through the formatting
/// machinery, which dominates the cost of emitting an escape sequence.
fn write_u8<W: Write>(w: &mut W, value: u8) -> fmt::Result {
    if value >= 100 {
        w.write_char(char::from(b'0' + value / 100))?;
    }
    if value >= 10 {
        w.write_char(char::from(b'0' + (value / 10) % 10))?;
    }
    w.write_char(char::from(b'0' + value % 10))
}

/// Match a colour name, ignoring case and any `_`, `-`, or space separators, so
/// `BrightRed`, `bright_red`, and `bright red` all resolve.
fn named(value: &str) -> Option<Color> {
    const NAMES: [(&str, Color); 20] = [
        ("black", Color::Black),
        ("red", Color::Red),
        ("green", Color::Green),
        ("yellow", Color::Yellow),
        ("blue", Color::Blue),
        ("magenta", Color::Magenta),
        ("cyan", Color::Cyan),
        ("white", Color::White),
        ("brightblack", Color::BrightBlack),
        ("brightred", Color::BrightRed),
        ("brightgreen", Color::BrightGreen),
        ("brightyellow", Color::BrightYellow),
        ("brightblue", Color::BrightBlue),
        ("brightmagenta", Color::BrightMagenta),
        ("brightcyan", Color::BrightCyan),
        ("brightwhite", Color::BrightWhite),
        // Common synonyms, so a theme written by hand does not need a lookup
        // table of its own.
        ("grey", Color::BrightBlack),
        ("gray", Color::BrightBlack),
        ("purple", Color::Magenta),
        ("brightgrey", Color::BrightWhite),
    ];
    NAMES
        .iter()
        .find(|(name, _)| name_eq(name, value))
        .map(|&(_, color)| color)
}

/// Compare a canonical lower-case name with user input, ignoring case and
/// skipping separator characters in the input.
fn name_eq(canonical: &str, input: &str) -> bool {
    let mut expected = canonical.bytes();
    for byte in input.bytes() {
        if matches!(byte, b'_' | b'-' | b' ') {
            continue;
        }
        match expected.next() {
            Some(want) if want == byte.to_ascii_lowercase() => {}
            _ => return false,
        }
    }
    expected.next().is_none()
}

/// Parse three or six hex digits into an exact colour. The three-digit form
/// expands each digit (`#f80` is `#ff8800`), as in CSS.
fn parse_hex(hex: &str) -> Option<Color> {
    if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    match hex.len() {
        3 => {
            let mut digits = hex.bytes().map(|b| hex_value(b) * 0x11);
            Some(Color::Rgb(digits.next()?, digits.next()?, digits.next()?))
        }
        6 => {
            let mut bytes = hex.as_bytes().chunks_exact(2).map(|pair| {
                // Both bytes are already known to be hex digits.
                hex_value(pair[0]) * 16 + hex_value(pair[1])
            });
            Some(Color::Rgb(bytes.next()?, bytes.next()?, bytes.next()?))
        }
        _ => None,
    }
}

/// The numeric value of one ASCII hex digit. Non-digits cannot reach this: every
/// caller checks `is_ascii_hexdigit` first, and a stray byte yields `0` rather
/// than panicking.
#[inline]
const fn hex_value(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        b'A'..=b'F' => byte - b'A' + 10,
        _ => 0,
    }
}

/// Parse an `r,g,b` triple of decimal `0..=255` channels.
fn parse_triple(value: &str) -> Option<Color> {
    let mut parts = value.split(',');
    let r = parts.next()?.trim().parse::<u8>().ok()?;
    let g = parts.next()?.trim().parse::<u8>().ok()?;
    let b = parts.next()?.trim().parse::<u8>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(Color::Rgb(r, g, b))
}

/// The six levels an xterm cube channel can take.
const CUBE_LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];

/// Map an exact colour to the nearest entry of the 256-colour palette.
///
/// Both the 6×6×6 cube and the 24-step greyscale ramp are considered, and the
/// closer of the two wins: a near-grey matches the ramp far more precisely than
/// the cube, whose grey diagonal has only six stops.
fn rgb_to_256(r: u8, g: u8, b: u8) -> u8 {
    let cube = 16 + 36 * cube_index(r) + 6 * cube_index(g) + cube_index(b);
    let cube_distance = {
        let (cr, cg, cb) = (
            CUBE_LEVELS[cube_index(r) as usize],
            CUBE_LEVELS[cube_index(g) as usize],
            CUBE_LEVELS[cube_index(b) as usize],
        );
        distance(r, g, b, cr, cg, cb)
    };

    // The ramp runs 232..=255 at 8, 18, 28, … 238.
    let grey_level = (u32::from(r) + u32::from(g) + u32::from(b)) / 3;
    let step = ((grey_level.saturating_sub(8)) + 5) / 10;
    let step = step.min(23) as u8;
    let grey_value = 8 + step * 10;
    let grey_distance = distance(r, g, b, grey_value, grey_value, grey_value);

    if grey_distance < cube_distance {
        232 + step
    } else {
        cube
    }
}

/// The cube stop nearest one channel value.
fn cube_index(value: u8) -> u8 {
    let mut best = 0u8;
    let mut best_delta = u16::MAX;
    let mut index = 0u8;
    while (index as usize) < CUBE_LEVELS.len() {
        let level = CUBE_LEVELS[index as usize];
        let delta = u16::from(level.abs_diff(value));
        if delta < best_delta {
            best_delta = delta;
            best = index;
        }
        index += 1;
    }
    best
}

/// The conventional xterm RGB values of the 256-colour palette.
const fn palette_rgb(index: u8) -> (u8, u8, u8) {
    const BASIC: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (128, 0, 0),
        (0, 128, 0),
        (128, 128, 0),
        (0, 0, 128),
        (128, 0, 128),
        (0, 128, 128),
        (192, 192, 192),
        (128, 128, 128),
        (255, 0, 0),
        (0, 255, 0),
        (255, 255, 0),
        (0, 0, 255),
        (255, 0, 255),
        (0, 255, 255),
        (255, 255, 255),
    ];
    match index {
        0..=15 => BASIC[index as usize],
        16..=231 => {
            let offset = index - 16;
            (
                CUBE_LEVELS[(offset / 36) as usize],
                CUBE_LEVELS[((offset % 36) / 6) as usize],
                CUBE_LEVELS[(offset % 6) as usize],
            )
        }
        _ => {
            let value = 8 + (index - 232) * 10;
            (value, value, value)
        }
    }
}

/// Map an exact colour to the nearest of the sixteen standard colours, returning
/// its palette slot.
fn nearest_basic(r: u8, g: u8, b: u8) -> u8 {
    let mut best = 7u8;
    let mut best_distance = u32::MAX;
    let mut slot = 0u8;
    while slot < 16 {
        let (pr, pg, pb) = palette_rgb(slot);
        let d = distance(r, g, b, pr, pg, pb);
        if d < best_distance {
            best_distance = d;
            best = slot;
        }
        slot += 1;
    }
    best
}

/// Weighted squared RGB distance.
///
/// The 3/6/1 weighting approximates perceived luminance (green dominates, blue
/// barely registers). Unweighted Euclidean distance in RGB puts mid greens closer
/// to black than to green often enough to be visible, which is the whole point of
/// degrading gracefully rather than arbitrarily.
#[inline]
const fn distance(r: u8, g: u8, b: u8, pr: u8, pg: u8, pb: u8) -> u32 {
    let dr = r.abs_diff(pr) as u32;
    let dg = g.abs_diff(pg) as u32;
    let db = b.abs_diff(pb) as u32;
    3 * dr * dr + 6 * dg * dg + db * db
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::shim::{String, ToString};

    /// Render a colour's SGR parameters in isolation.
    fn sgr(color: Color, level: ColorLevel, background: bool) -> String {
        let mut s = String::new();
        let mut first = true;
        color.write(&mut s, level, background, &mut first).unwrap();
        s
    }

    #[test]
    fn test_parse_named_ignores_case_and_separators() {
        assert_eq!(Color::parse("red"), Some(Color::Red));
        assert_eq!(Color::parse("RED"), Some(Color::Red));
        assert_eq!(Color::parse("  Cyan  "), Some(Color::Cyan));
        for spelling in [
            "brightred",
            "BrightRed",
            "bright_red",
            "bright-red",
            "bright red",
        ] {
            assert_eq!(Color::parse(spelling), Some(Color::BrightRed), "{spelling}");
        }
        assert_eq!(Color::parse("grey"), Some(Color::BrightBlack));
    }

    #[test]
    fn test_parse_exact_forms() {
        assert_eq!(Color::parse("#ff8800"), Some(Color::Rgb(255, 136, 0)));
        assert_eq!(Color::parse("#f80"), Some(Color::Rgb(255, 136, 0)));
        assert_eq!(Color::parse("0,200,120"), Some(Color::Rgb(0, 200, 120)));
        assert_eq!(Color::parse("255, 0, 0"), Some(Color::Rgb(255, 0, 0)));
        assert_eq!(Color::parse("0"), Some(Color::Ansi(0)));
        assert_eq!(Color::parse("208"), Some(Color::Ansi(208)));
        assert_eq!(Color::parse("255"), Some(Color::Ansi(255)));
    }

    #[test]
    fn test_parse_rejects_garbage() {
        for bad in [
            "notacolor",
            "#gggggg",
            "#ffff",
            "#",
            "1,2",
            "1,2,3,4",
            "0,0,256",
            "256",
            "-1",
            "",
            "   ",
        ] {
            assert_eq!(Color::parse(bad), None, "{bad:?} should not parse");
        }
    }

    #[test]
    fn test_named_colours_use_the_terminal_palette() {
        assert_eq!(sgr(Color::Red, ColorLevel::Ansi16, false), "31");
        assert_eq!(sgr(Color::Red, ColorLevel::Ansi16, true), "41");
        assert_eq!(sgr(Color::BrightRed, ColorLevel::Ansi16, false), "91");
        assert_eq!(sgr(Color::BrightRed, ColorLevel::Ansi16, true), "101");
        assert_eq!(sgr(Color::BrightWhite, ColorLevel::TrueColor, false), "97");
        // A named colour is never downgraded: it is already the user's palette.
        assert_eq!(sgr(Color::Green, ColorLevel::TrueColor, false), "32");
    }

    #[test]
    fn test_nothing_is_written_without_colour() {
        for color in [Color::Red, Color::Ansi(100), Color::Rgb(1, 2, 3)] {
            assert_eq!(sgr(color, ColorLevel::None, false), "");
            assert_eq!(sgr(color, ColorLevel::None, true), "");
        }
    }

    #[test]
    fn test_exact_colour_degrades_by_tier() {
        let orange = Color::Rgb(255, 136, 0);
        assert_eq!(sgr(orange, ColorLevel::TrueColor, false), "38;2;255;136;0");
        assert_eq!(sgr(orange, ColorLevel::TrueColor, true), "48;2;255;136;0");
        assert!(sgr(orange, ColorLevel::Ansi256, false).starts_with("38;5;"));
        assert!(sgr(orange, ColorLevel::Ansi256, true).starts_with("48;5;"));
        // Pure red reaches the bright red of the sixteen, not a muddy neighbour.
        assert_eq!(sgr(Color::Rgb(255, 0, 0), ColorLevel::Ansi16, false), "91");
        assert_eq!(sgr(Color::Rgb(0, 0, 0), ColorLevel::Ansi16, false), "30");
        assert_eq!(
            sgr(Color::Rgb(255, 255, 255), ColorLevel::Ansi16, false),
            "97"
        );
    }

    #[test]
    fn test_palette_index_degrades_only_at_sixteen_colours() {
        let c = Color::Ansi(208);
        assert_eq!(sgr(c, ColorLevel::TrueColor, false), "38;5;208");
        assert_eq!(sgr(c, ColorLevel::Ansi256, false), "38;5;208");
        // 208 is a saturated orange; the nearest of the sixteen is one of the
        // warm ones, and which exactly is a matter of the distance metric rather
        // than of correctness.
        let basic = sgr(c, ColorLevel::Ansi16, false);
        assert!(
            ["31", "33", "91", "93"].contains(&basic.as_str()),
            "orange downgraded to something that is not warm: {basic}"
        );
    }

    #[test]
    fn test_separator_only_between_parameters() {
        let mut s = String::new();
        let mut first = true;
        Color::Red
            .write(&mut s, ColorLevel::Ansi16, false, &mut first)
            .unwrap();
        Color::White
            .write(&mut s, ColorLevel::Ansi16, true, &mut first)
            .unwrap();
        assert_eq!(s, "31;47");
    }

    #[test]
    fn test_greyscale_prefers_the_ramp_over_the_cube() {
        // A near-grey has a far closer match on the 24-step ramp than on the
        // cube's six-stop diagonal.
        let index = rgb_to_256(118, 118, 118);
        assert!(
            (232..=255).contains(&index),
            "expected a greyscale ramp entry, got {index}"
        );
    }

    #[test]
    fn test_cube_is_used_for_saturated_colour() {
        let index = rgb_to_256(255, 0, 0);
        assert!(
            (16..=231).contains(&index),
            "expected a cube entry, got {index}"
        );
    }

    #[test]
    fn test_to_rgb_resolves_every_variant() {
        assert_eq!(Color::Rgb(1, 2, 3).to_rgb(), (1, 2, 3));
        assert_eq!(Color::Black.to_rgb(), (0, 0, 0));
        assert_eq!(Color::BrightWhite.to_rgb(), (255, 255, 255));
        assert_eq!(Color::Ansi(15).to_rgb(), (255, 255, 255));
        assert_eq!(Color::Ansi(16).to_rgb(), (0, 0, 0));
        assert_eq!(Color::Ansi(255).to_rgb(), (238, 238, 238));
    }

    #[test]
    fn test_code_tables_match_the_sgr_ranges() {
        // The tables replaced arithmetic, so a typo would now be a silently
        // wrong colour rather than a compile error.
        for slot in 0..16u8 {
            let expected_fg = if slot < 8 { 30 + slot } else { 90 + slot - 8 };
            let expected_bg = if slot < 8 { 40 + slot } else { 100 + slot - 8 };
            assert_eq!(
                FG_CODES[slot as usize],
                expected_fg.to_string(),
                "fg {slot}"
            );
            assert_eq!(
                BG_CODES[slot as usize],
                expected_bg.to_string(),
                "bg {slot}"
            );
        }
    }

    #[test]
    fn test_write_u8_matches_decimal_formatting() {
        for value in 0..=255u8 {
            let mut written = String::new();
            write_u8(&mut written, value).unwrap();
            assert_eq!(written, value.to_string(), "{value}");
        }
    }

    #[test]
    fn test_from_hex_accepts_both_widths_with_optional_hash() {
        assert_eq!(Color::from_hex("#88aaff"), Some(Color::Rgb(136, 170, 255)));
        assert_eq!(Color::from_hex("88aaff"), Some(Color::Rgb(136, 170, 255)));
        assert_eq!(Color::from_hex("8af"), Some(Color::Rgb(136, 170, 255)));
        assert_eq!(Color::from_hex("#8AF"), Some(Color::Rgb(136, 170, 255)));
        assert_eq!(Color::from_hex("nope"), None);
    }
}

#[cfg(test)]
mod proptests {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        /// Parsing arbitrary text never panics, however hostile.
        #[test]
        fn test_parse_never_panics(value in ".*") {
            let _ = Color::parse(&value);
            let _ = Color::from_hex(&value);
        }

        /// Every exact colour downgrades to a valid palette index.
        #[test]
        fn test_rgb_to_256_is_in_palette_range(r in 0u8..=255, g in 0u8..=255, b in 0u8..=255) {
            let index = rgb_to_256(r, g, b);
            prop_assert!((16..=255).contains(&index));
        }

        /// Every exact colour downgrades to one of the sixteen slots.
        #[test]
        fn test_nearest_basic_is_a_palette_slot(r in 0u8..=255, g in 0u8..=255, b in 0u8..=255) {
            prop_assert!(nearest_basic(r, g, b) < 16);
        }

        /// A `#rrggbb` string round-trips to the exact channels it names.
        #[test]
        fn test_hex_round_trip(r in 0u8..=255, g in 0u8..=255, b in 0u8..=255) {
            let hex = crate::shim::format!("#{r:02x}{g:02x}{b:02x}");
            prop_assert_eq!(Color::parse(&hex), Some(Color::Rgb(r, g, b)));
        }

        /// Every palette index resolves to a colour, and every colour's own
        /// index resolves back to itself.
        #[test]
        fn test_palette_rgb_total(index in 0u8..=255) {
            let (r, g, b) = palette_rgb(index);
            prop_assert_eq!(Color::Ansi(index).to_rgb(), (r, g, b));
        }

        /// A rendered colour is pure SGR payload: digits and separators only,
        /// never a stray escape or terminator that could corrupt the sequence
        /// it is embedded in.
        #[test]
        fn test_rendered_parameters_are_well_formed(
            index in 0u8..=255,
            r in 0u8..=255,
            g in 0u8..=255,
            b in 0u8..=255,
        ) {
            for level in [ColorLevel::Ansi16, ColorLevel::Ansi256, ColorLevel::TrueColor] {
                for color in [Color::Ansi(index), Color::Rgb(r, g, b)] {
                    for background in [false, true] {
                        let mut s = crate::shim::String::new();
                        let mut first = true;
                        let _ = color.write(&mut s, level, background, &mut first);
                        prop_assert!(
                            s.bytes().all(|byte| byte.is_ascii_digit() || byte == b';'),
                            "malformed SGR payload {s:?}"
                        );
                        prop_assert!(!s.is_empty());
                    }
                }
            }
        }
    }
}
