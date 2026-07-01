//! The auth seam: an auth-gated command that runs only when authorized. Requires
//! the `auth` feature.
//!
//! cli-forge holds the seam; the consumer supplies the session state. Here that
//! state is a single environment variable standing in for "logged in".
//!
//! ```bash
//! # Unauthorized: `publish` is refused (exit 2) and hidden from --help.
//! cargo run --features auth --example auth -- publish
//! cargo run --features auth --example auth -- --help
//!
//! # Authorized: `publish` runs and shows in --help.
//! DEMO_LOGGED_IN=1 cargo run --features auth --example auth -- publish
//! DEMO_LOGGED_IN=1 cargo run --features auth --example auth -- --help
//!
//! # Open commands are unaffected either way.
//! cargo run --features auth --example auth -- status
//! ```

use cli_forge::{App, Command, out};

fn main() {
    // The consumer decides what "authorized" means; cli-forge only asks. Keep the
    // hook pure — it is also consulted while rendering help.
    let logged_in = std::env::var_os("DEMO_LOGGED_IN").is_some();

    let mut app = App::new("demo")
        .version(env!("CARGO_PKG_VERSION"))
        .help_header("demo — auth seam example")
        .auth(move |_req| logged_in);

    app.register(
        Command::new("status")
            .about("show status (open to everyone)")
            .run(|_| out("status: ok")),
    );
    app.register(
        Command::new("publish")
            .about("publish a release (auth-gated)")
            .requires_auth(true)
            .run(|_| out("published!")),
    );

    // `publish` runs only when the hook authorizes it; otherwise `parse` prints
    // "error: not authorized to run: publish" and exits 2.
    let _ = app.parse();
}
