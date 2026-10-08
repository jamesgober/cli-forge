//! Named styles: describe once, recall anywhere.
//!
//! [`define`] stores a [`Style`] under a name and [`named`] looks it back up.
//! This is the path for a program's own visual vocabulary — the styles that are
//! neither one of the [`Level`](crate::Level) severities a
//! [`Theme`](crate::Theme) covers nor a one-off, but that appear in a dozen
//! places and must look the same in all of them.
//!
//! Because a [`Style`] carries its decoration as well as its colours, a name
//! captures the *whole* appearance — glyph, spacing, and column width included —
//! so a call site never rebuilds a marker by hand:
//!
//! ```
//! use cli_forge::{define, named, out, Style};
//!
//! define("step", Style::new().bright_black().prefix("  → ").pad_to(24));
//!
//! out(named("step").paint("resolve dependencies"));
//! out(named("step").paint("compile"));
//! ```
//!
//! The store is process-global because that is the point: a name defined in one
//! module must resolve in another, including across crate boundaries in a plugin
//! that was compiled separately. Two consequences follow, and both are deliberate:
//! a later [`define`] of the same name replaces the earlier one (last writer
//! wins, which is what lets a program re-theme a library's output), and names are
//! worth prefixing in library code (`mycrate.step`) so two libraries cannot
//! quietly collide.
//!
//! ## Cost
//!
//! A lookup takes a read lock on a small map and clones the [`Style`] it finds,
//! so the idiom for a loop is to look up once and reuse:
//!
//! ```
//! # use cli_forge::{define, named, out, Style};
//! # define("row", Style::new().bold());
//! let row = named("row");
//! for item in ["a", "b", "c"] {
//!     out(row.paint(item));
//! }
//! ```
//!
//! The store degrades rather than panics: if its lock has been poisoned by a
//! panic elsewhere, [`define`] skips the definition and [`named`] returns a plain
//! style. Styling is never important enough to take a program down over.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

use crate::style::Style;

/// The global name → style map, created on first use.
fn store() -> &'static RwLock<HashMap<String, Style>> {
    static STORE: OnceLock<RwLock<HashMap<String, Style>>> = OnceLock::new();
    STORE.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Define a reusable named style.
///
/// Defining the same name again replaces the previous definition. The style's own
/// text, if it has any, is irrelevant — [`named`] hands back the style for
/// painting other text — so the idiom is to build from [`Style::new`].
///
/// # Examples
///
/// ```
/// use cli_forge::{define, named, out, Style};
///
/// define("error", Style::new().red().bold().prefix("✗ "));
/// define("hint", Style::new().cyan());
///
/// out(named("error").paint("build failed"));
/// out(named("hint").paint("try `--release`"));
/// ```
pub fn define(name: impl Into<String>, style: Style) {
    if let Ok(mut map) = store().write() {
        // Replacing any previous definition for this name is intended.
        let _ = map.insert(name.into(), style);
    }
    // A poisoned lock means another thread panicked mid-write. Skipping the
    // definition keeps this fire-and-forget call non-panicking.
}

/// Look up a style defined by [`define`].
///
/// An unknown name yields a plain [`Style`], so a missing definition prints its
/// text unstyled instead of erroring — a program must not die because a theme
/// forgot a name. Use [`defined`] when the difference matters.
///
/// # Examples
///
/// ```
/// use cli_forge::{define, named, Style};
///
/// define("ok", Style::new().green());
/// assert!(named("ok").paint("passed").to_string().contains("passed"));
///
/// // An undefined name still renders the text, just without styling.
/// assert_eq!(named("never-defined").paint("text").to_string(), "text");
/// ```
#[must_use]
pub fn named(name: &str) -> Style {
    store()
        .read()
        .ok()
        .and_then(|map| map.get(name).cloned())
        .unwrap_or_default()
}

/// Whether `name` has been defined.
///
/// Lets a program fill in a default only when a theme has not already supplied
/// one, rather than overwriting it.
///
/// # Examples
///
/// ```
/// use cli_forge::{define, defined, Style};
///
/// assert!(!defined("registry-doc-example"));
/// define("registry-doc-example", Style::new().bold());
/// assert!(defined("registry-doc-example"));
/// ```
#[must_use]
pub fn defined(name: &str) -> bool {
    store().read().is_ok_and(|map| map.contains_key(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::ColorLevel;
    use crate::text;

    #[test]
    fn test_define_and_recall_applies_the_whole_appearance() {
        define("reg-error", Style::new().red().bold().prefix("✗ "));
        let style = named("reg-error");
        assert_eq!(
            style.paint_at("failed", ColorLevel::Ansi16).to_string(),
            "\x1b[1;31m✗ failed\x1b[0m"
        );
    }

    #[test]
    fn test_redefining_replaces() {
        define("reg-x", Style::new().red());
        define("reg-x", Style::new().green());
        assert_eq!(
            named("reg-x").paint_at("v", ColorLevel::Ansi16).to_string(),
            "\x1b[32mv\x1b[0m"
        );
    }

    #[test]
    fn test_unknown_name_is_plain_at_every_depth() {
        for level in [ColorLevel::None, ColorLevel::Ansi16, ColorLevel::TrueColor] {
            assert_eq!(
                named("reg-undefined").paint_at("text", level).to_string(),
                "text"
            );
        }
        assert!(named("reg-undefined").is_plain());
    }

    #[test]
    fn test_defined_reports_presence() {
        assert!(!defined("reg-absent"));
        define("reg-present", Style::new().bold());
        assert!(defined("reg-present"));
    }

    #[test]
    fn test_a_name_captures_padding_so_columns_stay_straight() {
        // The duplication this module exists to remove: the width travels with
        // the name instead of being re-specified at each call site.
        define("reg-step", Style::new().prefix("→ ").pad_to(12));
        let step = named("reg-step");
        for label in ["a", "resolve", "ten chars!"] {
            let line = step.paint_at(label, ColorLevel::None).to_string();
            assert_eq!(text::width(&line), 12, "{label}");
        }
        // Content past the budget is kept whole rather than cut.
        let long = step
            .paint_at("far longer than the field", ColorLevel::None)
            .to_string();
        assert!(text::width(&long) > 12);
        assert!(long.contains("far longer than the field"));
    }

    #[test]
    fn test_lookup_can_be_hoisted_out_of_a_loop() {
        define("reg-row", Style::new().bold());
        let row = named("reg-row");
        // One lookup, many paints — the documented idiom, and it must keep
        // working without re-borrowing the store.
        let rendered: Vec<String> = ["a", "b"]
            .iter()
            .map(|item| row.paint_at(item, ColorLevel::None).to_string())
            .collect();
        assert_eq!(rendered, ["a", "b"]);
    }
}
