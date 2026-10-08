//! The shortest useful tour: a real command line, and output that is themed
//! rather than restyled at every call.
//!
//! ```bash
//! cargo run --example quick_start -- build --release
//! cargo run --example quick_start -- build --jobs 0     # a refusal that explains itself
//! cargo run --example quick_start -- --help
//! ```

use std::process::ExitCode;

use cli_forge::{App, Arg, Command, hint, info, ok, out, style, warn};

fn main() -> ExitCode {
    let mut app = App::new("quick")
        .version(env!("CARGO_PKG_VERSION"))
        .about("the shortest useful cli-forge tour")
        // One app-level flag, accepted on either side of the command name.
        .arg(
            Arg::count("verbose")
                .short('v')
                .global(true)
                .help("say more"),
        );

    app.register(
        Command::new("build")
            .about("compile the project")
            .arg(Arg::flag("release").short('r').help("optimise"))
            .arg(
                Arg::option("jobs")
                    .short('j')
                    .default("1")
                    .value_name("N")
                    .help("compilation units to run at once")
                    // Checked before any of this program's code runs, so the
                    // handler below can read it without a second error path.
                    .validate(|value| match value.parse::<u16>() {
                        Ok(n) if n >= 1 => Ok(()),
                        _ => Err("expected a count of 1 or more".to_string()),
                    }),
            )
            .run(build),
    );

    app.run()
}

/// Compile — or rather, say what compiling would involve.
///
/// Nothing here names a colour or pads a column. The level says what kind of
/// thing is being reported and the theme decides how it looks, so the whole
/// program can be restyled in one place.
fn build(m: &cli_forge::Matches) -> Result<(), String> {
    // `jobs` was validated at the edge, so this cannot fail for a reason the
    // user caused.
    let jobs: u16 = m.get("jobs").unwrap_or(1);
    let profile = if m.flag("release") {
        "release"
    } else {
        "debug"
    };

    out(style(format!("building [{profile}]")).bold());
    if m.count("verbose") > 0 {
        info(format!("running {jobs} compilation unit(s) at a time"));
    }

    if profile == "debug" {
        warn("this build is not optimised");
        hint("pass --release for an optimised build");
    }

    ok("compiled 3 targets");
    Ok(())
}
