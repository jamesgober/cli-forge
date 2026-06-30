//! The property that defines the crate: a command registered from a non-`main`
//! module is reachable and behaves identically to one registered inline. This is
//! the limitation that made the predecessor unusable, so it gets a dedicated
//! cross-crate test against the public API only.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use cli_forge::{App, Command};

/// A command defined and registered entirely inside a separate module — standing
/// in for a plugin, a feature module, or a config-driven setup function that is
/// nowhere near `main`.
mod plugin {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use cli_forge::{App, Command};

    pub static GREET_HITS: AtomicUsize = AtomicUsize::new(0);

    pub fn install(app: &mut App) {
        app.register(Command::new("greet").run(|_| {
            GREET_HITS.fetch_add(1, Ordering::SeqCst);
        }));
    }
}

#[test]
fn command_registered_outside_main_is_reachable_and_runs() {
    let mut app = App::new("demo");
    // Registration happens in `plugin`, not here.
    plugin::install(&mut app);

    let matches = app.try_parse_from(["greet"]).expect("parse should succeed");
    assert_eq!(matches.subcommand().map(|(name, _)| name), Some("greet"));
    assert_eq!(plugin::GREET_HITS.load(Ordering::SeqCst), 1);
}

#[test]
fn command_built_inline_behaves_identically() {
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&hits);

    let mut app = App::new("demo");
    app.register(Command::new("greet").run(move |_| {
        counter.fetch_add(1, Ordering::SeqCst);
    }));

    let matches = app.try_parse_from(["greet"]).expect("parse should succeed");
    assert_eq!(matches.subcommand().map(|(name, _)| name), Some("greet"));
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}
