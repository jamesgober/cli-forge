//! A small CLI built from the command tree: top-level commands, a nested
//! subcommand, flags, options, and positionals — each with a handler that runs
//! when its command is selected. Styling flows through the same output layer.
//!
//! ```bash
//! cargo run --example commands -- build --release -j 8
//! cargo run --example commands -- greet Ada
//! cargo run --example commands -- remote add origin
//! cargo run --example commands -- --bogus        # structured error, exit 2
//! ```

use cli_forge::{App, Arg, Command, out, style};

fn main() {
    let mut app = App::new("demo")
        .help_header("demo — a cli-forge example")
        .help_footer("see https://github.com/jamesgober/cli-forge");

    app.register(
        Command::new("build")
            .about("compile the project")
            .arg(Arg::flag("release").short('r').help("optimized build"))
            .arg(
                Arg::option("jobs")
                    .short('j')
                    .default("1")
                    .help("parallel jobs"),
            )
            .run(|m| {
                let profile = if m.flag("release") {
                    "release"
                } else {
                    "debug"
                };
                let jobs = m.value("jobs").unwrap_or("1");
                out(style(format!("building [{profile}] with {jobs} job(s)"))
                    .cyan()
                    .bold());
            }),
    );

    app.register(
        Command::new("greet")
            .about("print a greeting")
            .arg(Arg::positional("name").default("world"))
            .run(|m| {
                let name = m.value("name").unwrap_or("world");
                out(format!("hello, {name}"));
            }),
    );

    app.register(
        Command::new("remote")
            .about("manage remotes")
            .subcommand(
                Command::new("add")
                    .about("add a remote")
                    .arg(Arg::positional("name").required(true))
                    .run(|m| out(format!("added remote {}", m.value("name").unwrap_or("?")))),
            )
            .subcommand(
                Command::new("remove")
                    .about("remove a remote")
                    .arg(Arg::positional("name").required(true))
                    .run(|m| out(format!("removed remote {}", m.value("name").unwrap_or("?")))),
            ),
    );

    // Parses the process arguments, runs the selected command's handler, and on
    // malformed input prints a structured error and exits with status 2.
    let _matches = app.parse();
}
