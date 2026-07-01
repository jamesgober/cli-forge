//! The authentication seam.
//!
//! cli-forge holds the *seam*, not the logic. A command marked
//! [`requires_auth`](crate::Command::requires_auth) runs only if the app's auth
//! hook authorizes it; the hook — supplied by the consumer or a sibling
//! `cli-auth` crate — is where login/logout state actually lives. The core just
//! asks. This module is compiled only with the `auth` feature; without it,
//! `requires_auth` is inert.
//!
//! The hook is `Fn(&`[`AuthRequest`]`) -> bool`. It is consulted before an
//! auth-gated command's handler runs (returning `false` refuses the command with
//! [`ParseError::Unauthorized`](crate::ParseError::Unauthorized)) and again when
//! generating help (an unauthorized command is omitted from the listing). If no
//! hook is set, auth-gated commands are never authorized — the seam fails closed.
//!
//! Because it also runs during help generation, the hook should be pure and
//! cheap: check already-loaded session state rather than doing I/O or printing.

/// The boxed authorization hook stored on an [`App`](crate::App).
pub(crate) type AuthHook = Box<dyn Fn(&AuthRequest<'_>) -> bool>;

/// The context passed to the auth hook: which command is being authorized.
///
/// Marked `#[non_exhaustive]` so future context (roles, the parsed arguments,
/// …) can be added without a breaking change.
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "auth")]
/// # {
/// use cli_forge::{App, Command};
///
/// let mut app = App::new("demo").auth(|req| {
///     // Only authorize `publish` when some session check passes.
///     req.command() != "publish" || session_is_valid()
/// });
/// app.register(Command::new("publish").requires_auth(true).run(|_| { /* ... */ }));
///
/// fn session_is_valid() -> bool { true }
/// # let _ = app.try_parse_from(["publish"]);
/// # }
/// ```
#[non_exhaustive]
pub struct AuthRequest<'a> {
    path: &'a [&'a str],
}

impl<'a> AuthRequest<'a> {
    /// Build a request for the command reached by `path` (the command-name chain
    /// from the app root to the command being authorized).
    pub(crate) fn new(path: &'a [&'a str]) -> AuthRequest<'a> {
        AuthRequest { path }
    }

    /// The name of the command being authorized (the last element of the path).
    #[must_use]
    pub fn command(&self) -> &str {
        self.path.last().copied().unwrap_or("")
    }

    /// The full command-name chain from the app root to this command, e.g.
    /// `["remote", "add"]`.
    #[must_use]
    pub fn path(&self) -> &[&str] {
        self.path
    }
}
