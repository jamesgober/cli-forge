//! Argument groups: constraints over a set of arguments.
//!
//! [`Arg::conflicts_with`](crate::Arg::conflicts_with) and
//! [`Arg::requires`](crate::Arg::requires) describe relationships between pairs.
//! Some rules are about a *set*, and spelling them pairwise is both tedious and
//! wrong: "exactly one output format" over four formats is twelve conflict
//! declarations and still says nothing about requiring one.
//!
//! An [`ArgGroup`] names the set once and states the rule:
//!
//! ```
//! use cli_forge::{App, Arg, ArgGroup, Command};
//!
//! let mut app = App::new("export");
//! app.register(
//!     Command::new("dump")
//!         .arg(Arg::flag("json"))
//!         .arg(Arg::flag("yaml"))
//!         .arg(Arg::flag("toml"))
//!         .group(ArgGroup::new("format").args(["json", "yaml", "toml"]).required(true)),
//! );
//!
//! // Exactly one: none is refused, two is refused, one is fine.
//! assert!(app.try_parse_from(["dump"]).is_err());
//! assert!(app.try_parse_from(["dump", "--json", "--yaml"]).is_err());
//!
//! let m = app.try_parse_from(["dump", "--yaml"]).unwrap();
//! assert_eq!(m.leaf().group("format"), Some("yaml"));
//! ```
//!
//! [`Matches::group`](crate::Matches::group) then answers *which* member was
//! chosen, so dispatching on the choice is one `match` rather than a chain of
//! `if m.flag(..)`.
//!
//! ## What counts as "given"
//!
//! For the one-at-most rule, a member counts when the user supplied it — on the
//! command line or through its environment variable — and did not turn it off
//! with `--no-NAME`. A default does **not** count, because otherwise a group
//! whose members have defaults could never be satisfied without a conflict. For
//! the at-least-one rule, a member with any value counts, defaults included: a
//! default is an answer.

use crate::shim::{String, Vec};

/// A named set of arguments with a rule about how many may be given.
///
/// By default at most one member may be given. [`required`](ArgGroup::required)
/// demands at least one, so the two together mean exactly one;
/// [`multiple`](ArgGroup::multiple) lifts the at-most-one limit, so a required
/// multiple group means "at least one".
///
/// | `required` | `multiple` | Rule |
/// |---|---|---|
/// | no | no | at most one *(default)* |
/// | yes | no | exactly one |
/// | yes | yes | at least one |
/// | no | yes | any number — the group only names the set |
///
/// # Examples
///
/// ```
/// use cli_forge::ArgGroup;
///
/// // Exactly one output format.
/// let format = ArgGroup::new("format").args(["json", "yaml"]).required(true);
///
/// // At least one source, as many as you like.
/// let source = ArgGroup::new("source").args(["file", "url", "stdin"]).required(true).multiple(true);
/// # let _ = (format, source);
/// ```
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ArgGroup {
    pub(crate) name: String,
    pub(crate) members: Vec<String>,
    pub(crate) required: bool,
    pub(crate) multiple: bool,
}

impl ArgGroup {
    /// A group with no members yet, allowing at most one of them.
    ///
    /// The name is what [`Matches::group`](crate::Matches::group) is asked with,
    /// and what an error names when the rule is broken.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::ArgGroup;
    /// let group = ArgGroup::new("format");
    /// assert_eq!(group.name(), "format");
    /// ```
    #[must_use]
    pub fn new(name: impl Into<String>) -> ArgGroup {
        ArgGroup {
            name: name.into(),
            members: Vec::new(),
            required: false,
            multiple: false,
        }
    }

    /// Add one argument, by its name.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::ArgGroup;
    /// let group = ArgGroup::new("format").arg("json").arg("yaml");
    /// assert_eq!(group.members(), ["json", "yaml"]);
    /// ```
    #[must_use]
    pub fn arg(mut self, name: impl Into<String>) -> ArgGroup {
        self.members.push(name.into());
        self
    }

    /// Add several arguments, by name.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::ArgGroup;
    /// let group = ArgGroup::new("format").args(["json", "yaml", "toml"]);
    /// assert_eq!(group.members().len(), 3);
    /// ```
    #[must_use]
    pub fn args<I, S>(mut self, names: I) -> ArgGroup
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.members.extend(names.into_iter().map(Into::into));
        self
    }

    /// Require at least one member.
    ///
    /// Combined with the default at-most-one rule, this is "exactly one".
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg, ArgGroup, Command, ErrorKind};
    ///
    /// let mut app = App::new("demo");
    /// app.register(
    ///     Command::new("dump")
    ///         .args([Arg::flag("json"), Arg::flag("yaml")])
    ///         .group(ArgGroup::new("format").args(["json", "yaml"]).required(true)),
    /// );
    ///
    /// let error = app.try_parse_from(["dump"]).unwrap_err();
    /// assert_eq!(error.kind(), ErrorKind::MissingRequired);
    /// assert_eq!(error.subject(), "format");
    /// ```
    #[must_use]
    pub fn required(mut self, required: bool) -> ArgGroup {
        self.required = required;
        self
    }

    /// Allow more than one member to be given at once.
    ///
    /// # Examples
    ///
    /// ```
    /// use cli_forge::{App, Arg, ArgGroup, Command};
    ///
    /// let mut app = App::new("demo");
    /// app.register(
    ///     Command::new("fetch")
    ///         .args([Arg::option("file"), Arg::option("url")])
    ///         .group(ArgGroup::new("source").args(["file", "url"]).required(true).multiple(true)),
    /// );
    ///
    /// assert!(app.try_parse_from(["fetch", "--file", "a", "--url", "b"]).is_ok());
    /// assert!(app.try_parse_from(["fetch"]).is_err());
    /// ```
    #[must_use]
    pub fn multiple(mut self, multiple: bool) -> ArgGroup {
        self.multiple = multiple;
        self
    }

    /// The group's name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The names of the member arguments, in the order they were added.
    #[must_use]
    pub fn members(&self) -> &[String] {
        &self.members
    }

    /// Whether at least one member is required.
    #[must_use]
    pub const fn is_required(&self) -> bool {
        self.required
    }

    /// Whether more than one member may be given.
    #[must_use]
    pub const fn is_multiple(&self) -> bool {
        self.multiple
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults_are_at_most_one() {
        let group = ArgGroup::new("g");
        assert!(!group.is_required());
        assert!(!group.is_multiple());
        assert!(group.members().is_empty());
    }

    #[test]
    fn test_members_accumulate_in_order() {
        let group = ArgGroup::new("g").arg("a").args(["b", "c"]).arg("d");
        assert_eq!(group.members(), ["a", "b", "c", "d"]);
    }
}
