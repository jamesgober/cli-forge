//! Colors and their ANSI encoding.
//!
//! A [`Color`] is either one of the eight standard named colors or an arbitrary
//! 24-bit RGB value. The same value reaches the terminal three ways — through
//! the [`style`](crate::style) builder, a `<c=…>` tag, or a named registry
//! entry — and each renders it identically because they all funnel through
//! [`Color::write_fg`].
//!
//! When the terminal cannot render a 24-bit color, the value is downgraded here
//! rather than dropped: to the nearest 256-color cube entry, or to the nearest
//! of the 16 standard colors. Named colors always map to their standard SGR code
//! and never need downgrading.

use std::fmt::{self, Write};

use crate::terminal::ColorLevel;

/// A foreground color.
///
/// Construct named colors directly, or an RGB color via [`Color::from_hex`] or
/// the [`Color::Rgb`] variant. Parsing of tag/registry color values goes through
/// [`Color::parse`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Color {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    /// A 24-bit color. Downgraded at render time on terminals that cannot
    /// display it.
    Rgb(u8, u8, u8),
}

impl Color {
    /// Parse a color value as written in a `<c=…>` tag or passed to
    /// [`define_tag`](crate::define_tag): a named color (case-insensitive), a
    /// `#rrggbb` hex string, or an `r,g,b` triple. Returns `None` if the value
    /// matches none of those forms.
    pub(crate) fn parse(value: &str) -> Option<Color> {
        let value = value.trim();
        if let Some(hex) = value.strip_prefix('#') {
            return parse_hex(hex);
        }
        if value.contains(',') {
            return parse_triple(value);
        }
        named(value)
    }

    /// Parse a hex color for the builder's [`Style::hex`](crate::Style::hex). A
    /// leading `#` is optional; the rest must be exactly six hex digits.
    pub(crate) fn from_hex(hex: &str) -> Option<Color> {
        parse_hex(hex.strip_prefix('#').unwrap_or(hex))
    }

    /// Write the SGR foreground parameters for this color at `level`, without a
    /// leading or trailing `;`. `first` tracks whether an earlier parameter (a
    /// bold/underline flag) has already been written, so separators land in the
    /// right place. Writes nothing at [`ColorLevel::None`].
    pub(crate) fn write_fg<W: Write>(
        self,
        w: &mut W,
        level: ColorLevel,
        first: &mut bool,
    ) -> fmt::Result {
        if level.is_none() {
            return Ok(());
        }
        match self {
            Color::Black => write_param(w, first, "30"),
            Color::Red => write_param(w, first, "31"),
            Color::Green => write_param(w, first, "32"),
            Color::Yellow => write_param(w, first, "33"),
            Color::Blue => write_param(w, first, "34"),
            Color::Magenta => write_param(w, first, "35"),
            Color::Cyan => write_param(w, first, "36"),
            Color::White => write_param(w, first, "37"),
            Color::Rgb(r, g, b) => match level {
                ColorLevel::TrueColor => {
                    separator(w, first)?;
                    write!(w, "38;2;{r};{g};{b}")
                }
                ColorLevel::Ansi256 => {
                    separator(w, first)?;
                    write!(w, "38;5;{}", rgb_to_256(r, g, b))
                }
                ColorLevel::Ansi16 => {
                    separator(w, first)?;
                    write!(w, "{}", rgb_to_basic(r, g, b))
                }
                ColorLevel::None => Ok(()),
            },
        }
    }
}

/// Write a `;` separator before all but the first SGR parameter.
fn separator<W: Write>(w: &mut W, first: &mut bool) -> fmt::Result {
    if *first {
        *first = false;
        Ok(())
    } else {
        w.write_char(';')
    }
}

/// Write a fixed SGR parameter, preceded by a separator when needed.
fn write_param<W: Write>(w: &mut W, first: &mut bool, param: &str) -> fmt::Result {
    separator(w, first)?;
    w.write_str(param)
}

/// Match one of the eight standard color names, case-insensitively.
fn named(value: &str) -> Option<Color> {
    const NAMES: [(&str, Color); 8] = [
        ("black", Color::Black),
        ("red", Color::Red),
        ("green", Color::Green),
        ("yellow", Color::Yellow),
        ("blue", Color::Blue),
        ("magenta", Color::Magenta),
        ("cyan", Color::Cyan),
        ("white", Color::White),
    ];
    NAMES
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(value))
        .map(|&(_, color)| color)
}

/// Parse exactly six hex digits into an RGB color.
fn parse_hex(hex: &str) -> Option<Color> {
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
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

/// Map a 24-bit color to the nearest entry in the xterm 256-color cube.
///
/// The 6×6×6 cube uses the standard non-linear level breakpoints; the result is
/// always in `16..=231`, a valid `38;5;n` index.
fn rgb_to_256(r: u8, g: u8, b: u8) -> u8 {
    16 + 36 * cube_index(r) + 6 * cube_index(g) + cube_index(b)
}

/// Nearest of the six cube levels (`0, 95, 135, 175, 215, 255`) for one channel.
fn cube_index(v: u8) -> u8 {
    const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    LEVELS
        .iter()
        .enumerate()
        .min_by_key(|&(_, &level)| (i16::from(level) - i16::from(v)).unsigned_abs())
        .map_or(0, |(i, _)| i as u8)
}

/// Map a 24-bit color to the nearest of the 16 standard colors, returning its
/// SGR foreground code (`30..=37`).
fn rgb_to_basic(r: u8, g: u8, b: u8) -> u8 {
    // Reference points for the eight standard colors, paired with their codes.
    const PALETTE: [(u8, u8, u8, u8); 8] = [
        (0, 0, 0, 30),       // black
        (128, 0, 0, 31),     // red
        (0, 128, 0, 32),     // green
        (128, 128, 0, 33),   // yellow
        (0, 0, 128, 34),     // blue
        (128, 0, 128, 35),   // magenta
        (0, 128, 128, 36),   // cyan
        (192, 192, 192, 37), // white
    ];
    PALETTE
        .iter()
        .min_by_key(|&&(pr, pg, pb, _)| {
            let dr = (i32::from(pr) - i32::from(r)).pow(2);
            let dg = (i32::from(pg) - i32::from(g)).pow(2);
            let db = (i32::from(pb) - i32::from(b)).pow(2);
            (dr + dg + db) as u32
        })
        .map_or(37, |&(_, _, _, code)| code)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn fg(color: Color, level: ColorLevel) -> String {
        let mut s = String::new();
        let mut first = true;
        color.write_fg(&mut s, level, &mut first).unwrap();
        s
    }

    #[test]
    fn test_parse_named_is_case_insensitive() {
        assert_eq!(Color::parse("red"), Some(Color::Red));
        assert_eq!(Color::parse("RED"), Some(Color::Red));
        assert_eq!(Color::parse("  Cyan  "), Some(Color::Cyan));
    }

    #[test]
    fn test_parse_hex_and_triple() {
        assert_eq!(Color::parse("#ff8800"), Some(Color::Rgb(255, 136, 0)));
        assert_eq!(Color::parse("0,200,120"), Some(Color::Rgb(0, 200, 120)));
        assert_eq!(Color::parse("255, 0, 0"), Some(Color::Rgb(255, 0, 0)));
    }

    #[test]
    fn test_parse_rejects_garbage() {
        assert_eq!(Color::parse("notacolor"), None);
        assert_eq!(Color::parse("#fff"), None); // 3-digit shorthand unsupported
        assert_eq!(Color::parse("#gggggg"), None);
        assert_eq!(Color::parse("1,2"), None);
        assert_eq!(Color::parse("1,2,3,4"), None);
        assert_eq!(Color::parse("0,0,256"), None); // out of u8 range
    }

    #[test]
    fn test_from_hex_optional_hash() {
        assert_eq!(Color::from_hex("#88aaff"), Some(Color::Rgb(136, 170, 255)));
        assert_eq!(Color::from_hex("88aaff"), Some(Color::Rgb(136, 170, 255)));
        assert_eq!(Color::from_hex("nope"), None);
    }

    #[test]
    fn test_named_fg_codes() {
        assert_eq!(fg(Color::Red, ColorLevel::Ansi16), "31");
        assert_eq!(fg(Color::White, ColorLevel::TrueColor), "37");
        assert_eq!(fg(Color::Black, ColorLevel::None), "");
    }

    #[test]
    fn test_rgb_fg_per_level() {
        let c = Color::Rgb(255, 136, 0);
        assert_eq!(fg(c, ColorLevel::TrueColor), "38;2;255;136;0");
        assert!(fg(c, ColorLevel::Ansi256).starts_with("38;5;"));
        assert_eq!(fg(c, ColorLevel::None), "");
        // Pure red downgrades to the standard red (code 31) at 16-color depth.
        assert_eq!(fg(Color::Rgb(255, 0, 0), ColorLevel::Ansi16), "31");
    }

    #[test]
    fn test_rgb_to_256_is_always_in_cube_range() {
        for &(r, g, b) in &[(0, 0, 0), (255, 255, 255), (12, 200, 77), (250, 5, 130)] {
            let idx = rgb_to_256(r, g, b);
            assert!((16..=231).contains(&idx), "index {idx} out of cube range");
        }
    }

    #[test]
    fn test_separator_inserts_between_params() {
        let mut s = String::new();
        let mut first = true;
        write_param(&mut s, &mut first, "1").unwrap();
        write_param(&mut s, &mut first, "4").unwrap();
        assert_eq!(s, "1;4");
    }
}

#[cfg(test)]
mod proptests {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        /// Parsing arbitrary color strings never panics.
        #[test]
        fn test_parse_never_panics(value in ".*") {
            let _ = Color::parse(&value);
            let _ = Color::from_hex(&value);
        }

        /// Every 24-bit color downgrades to a valid 256-cube index (`16..=231`).
        #[test]
        fn test_rgb_to_256_in_cube_range(r in 0u8..=255, g in 0u8..=255, b in 0u8..=255) {
            prop_assert!((16..=231).contains(&rgb_to_256(r, g, b)));
        }

        /// Every 24-bit color downgrades to a valid basic SGR code (`30..=37`).
        #[test]
        fn test_rgb_to_basic_in_range(r in 0u8..=255, g in 0u8..=255, b in 0u8..=255) {
            prop_assert!((30..=37).contains(&rgb_to_basic(r, g, b)));
        }

        /// A `#rrggbb` string round-trips through parsing to the exact channels.
        #[test]
        fn test_hex_round_trip(r in 0u8..=255, g in 0u8..=255, b in 0u8..=255) {
            let hex = format!("#{r:02x}{g:02x}{b:02x}");
            prop_assert_eq!(Color::parse(&hex), Some(Color::Rgb(r, g, b)));
        }
    }
}
