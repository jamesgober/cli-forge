//! Named styles: define once, reuse anywhere.
//!
//! [`define_tag`] stores a [`Style`]'s attributes under a name; [`tag`] looks
//! that name back up and applies it to fresh text. This is the DRY styling path
//! — a style is described in one place and recalled from anywhere in the program
//! without repeating its color and attributes at every call site.
//!
//! The store is process-global because that is the point: a name defined in one
//! module must resolve in another. It is a small read-mostly map behind an
//! [`RwLock`], guarded so a poisoned lock degrades to plain output instead of
//! taking down the program — styling is never critical enough to panic over.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

use crate::Style;
use crate::style::{StyleAttrs, write_styled};
use crate::terminal;

/// The global name → attributes map, created on first use.
fn store() -> &'static RwLock<HashMap<String, StyleAttrs>> {
    static STORE: OnceLock<RwLock<HashMap<String, StyleAttrs>>> = OnceLock::new();
    STORE.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Define a reusable named style.
///
/// Only the color and attributes of `style` are stored; its text is ignored, so
/// the idiom is to define from an empty [`style`](crate::style). Defining the
/// same name again replaces the previous definition.
///
/// # Examples
///
/// ```
/// use cli_core::{define_tag, out, style, tag};
///
/// define_tag("error", style("").red().bold());
/// define_tag("hint", style("").cyan());
///
/// out(tag("error").render_with("build failed"));
/// out(tag("hint").render_with("try `--release`"));
/// ```
pub fn define_tag<S: Into<String>>(name: S, style: Style) {
    let attrs = style.attrs();
    if let Ok(mut map) = store().write() {
        // Replacing any previous definition for this name is intended.
        let _ = map.insert(name.into(), attrs);
    }
    // A poisoned lock means another thread panicked mid-write. Skipping the
    // definition keeps this fire-and-forget call non-panicking.
}

/// Look up a named style defined by [`define_tag`].
///
/// An unknown name yields a [`Tag`] that renders its text plain, so missing
/// definitions degrade gracefully rather than erroring.
///
/// # Examples
///
/// ```
/// use cli_core::{define_tag, style, tag};
///
/// define_tag("ok", style("").green());
/// assert!(tag("ok").render_with("passed").contains("passed"));
///
/// // Undefined names still render the text, just without styling.
/// assert_eq!(tag("never-defined").render_with("text"), "text");
/// ```
#[must_use]
pub fn tag(name: &str) -> Tag {
    let attrs = store().read().ok().and_then(|map| map.get(name).copied());
    Tag { attrs }
}

/// A resolved named style, returned by [`tag`].
///
/// Holds a snapshot of the named style's attributes (or none, for an unknown
/// name), so it can render text without holding the registry lock.
#[derive(Clone, Copy, Debug)]
pub struct Tag {
    attrs: Option<StyleAttrs>,
}

impl Tag {
    /// Render `text` with this named style, returning an owned `String`.
    ///
    /// Color depth matches the terminal detected for standard output. For an
    /// unknown name the text is returned unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_core::{define_tag, style, tag};
    ///
    /// define_tag("warn", style("").yellow().bold());
    /// let line = tag("warn").render_with("disk almost full");
    /// assert!(line.contains("disk almost full"));
    /// ```
    #[must_use]
    pub fn render_with(&self, text: &str) -> String {
        let mut buf = String::with_capacity(text.len() + 24);
        // Writing to a `String` is infallible.
        let _ = write_styled(
            &mut buf,
            self.attrs_or_empty(),
            text,
            terminal::color_level(),
        );
        buf
    }

    /// The captured attributes, or an empty set for an unknown name. Used by the
    /// cross-path equality tests to render at an explicit color level.
    #[cfg(test)]
    pub(crate) fn attrs_or_empty(&self) -> StyleAttrs {
        self.attrs.unwrap_or(StyleAttrs::EMPTY)
    }

    #[cfg(not(test))]
    fn attrs_or_empty(&self) -> StyleAttrs {
        self.attrs.unwrap_or(StyleAttrs::EMPTY)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::style::style;
    use crate::terminal::ColorLevel;

    /// Render a resolved tag's captured attributes at an explicit level.
    fn render_resolved(resolved: &Tag, text: &str, level: ColorLevel) -> String {
        let attrs = resolved.attrs.unwrap_or(StyleAttrs::EMPTY);
        let mut s = String::new();
        write_styled(&mut s, attrs, text, level).unwrap();
        s
    }

    #[test]
    fn test_define_and_recall_applies_attributes() {
        define_tag("reg-error", style("").red().bold());
        let resolved = tag("reg-error");
        assert_eq!(
            render_resolved(&resolved, "failed", ColorLevel::Ansi16),
            "\x1b[1;31mfailed\x1b[0m"
        );
    }

    #[test]
    fn test_redefining_replaces() {
        define_tag("reg-x", style("").red());
        define_tag("reg-x", style("").green());
        let resolved = tag("reg-x");
        assert_eq!(
            render_resolved(&resolved, "v", ColorLevel::Ansi16),
            "\x1b[32mv\x1b[0m"
        );
    }

    #[test]
    fn test_unknown_tag_is_plain() {
        assert_eq!(tag("reg-undefined").render_with("text"), "text");
        let resolved = tag("reg-undefined");
        assert_eq!(
            render_resolved(&resolved, "text", ColorLevel::TrueColor),
            "text"
        );
    }
}
