//! Every argument kind in one command: a flag, a counting flag, an option with a
//! default, a repeatable option, and a required variadic positional.
//!
//! ```bash
//! cargo run --example arguments -- build -vv --release -D A=1 -D B=2 src/a.rs src/b.rs
//! cargo run --example arguments -- build --help
//! ```

use cli_forge::{App, Arg, Command, out};

fn main() {
    let mut app = App::new("cc").version(env!("CARGO_PKG_VERSION"));

    app.register(
        Command::new("build")
            .about("compile — demonstrates every argument kind")
            .arg(Arg::flag("release").short('r').help("optimized build"))
            .arg(
                Arg::count("verbose")
                    .short('v')
                    .help("increase verbosity (repeat: -vvv)"),
            )
            .arg(
                Arg::option("jobs")
                    .short('j')
                    .default("1")
                    .help("parallel jobs"),
            )
            .arg(
                Arg::option("define")
                    .short('D')
                    .multiple(true)
                    .help("preprocessor define (repeatable)"),
            )
            .arg(
                Arg::positional("sources")
                    .multiple(true)
                    .required(true)
                    .help("source files"),
            )
            .run(|m| {
                out(format!("release  : {}", m.flag("release")));
                out(format!("verbosity: {}", m.count("verbose")));
                out(format!("jobs     : {}", m.value("jobs").unwrap_or("1")));
                out(format!(
                    "defines  : {:?}",
                    m.values("define").collect::<Vec<_>>()
                ));
                out(format!(
                    "sources  : {:?}",
                    m.values("sources").collect::<Vec<_>>()
                ));
            }),
    );

    let _ = app.parse();
}
