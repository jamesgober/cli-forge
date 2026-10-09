//! The README's code examples, compiled and run.
//!
//! Documentation that is never executed rots, and a broken first impression is
//! the most expensive kind of bug a library can have. Every Rust block in
//! `README.md` appears here, adapted only where a snippet shows a `main` (which
//! becomes a function taking an argument list, so the behaviour can be asserted
//! rather than merely compiled).
//!
//! If a change to the public API breaks one of these, the README is wrong and
//! this test is how that gets noticed.

use cli_forge::{
    App, Arg, Color, ColorChoice, Command, Glyphs, Level, Matches, Style, Theme, define, fail,
    hint, markup, named, ok, out, style, terminal, text, warn,
};

/// Hold a lock for the whole test and put the process-wide presentation state
/// back afterwards.
///
/// The theme and the colour choice are global, and these tests run on parallel
/// threads: without the lock, one test's reset to the default theme could land
/// between another's `install` and its assertion. That race was invisible while
/// the default glyph was also `✓`, and failed 7 runs in 10 with Unicode
/// detection off.
struct Restore {
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl Restore {
    fn new() -> Restore {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        Restore {
            _lock: LOCK
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        }
    }
}

impl Drop for Restore {
    fn drop(&mut self) {
        terminal::set_color_choice(ColorChoice::Auto);
        Theme::new().install();
    }
}

// --- Quick Start -----------------------------------------------------------

/// The quick-start command, with its `main` turned inside out so the parse can
/// be asserted.
fn quick_start_app() -> App {
    let mut app = App::new("forge")
        .version(env!("CARGO_PKG_VERSION"))
        .about("a project constructor")
        .arg(Arg::count("verbose").short('v').global(true));

    app.register(
        Command::new("build")
            .about("compile the project")
            .arg(Arg::flag("release").short('r'))
            .arg(Arg::option("jobs").short('j').default("1"))
            .run(|m| {
                let jobs: u16 = m.get("jobs").unwrap_or(1);
                out(format!("building with {jobs} job(s)"));
                if !m.flag("release") {
                    warn("this build is not optimised");
                }
                ok("compiled 3 targets");
            }),
    );
    app
}

#[test]
fn readme_quick_start_runs() {
    let app = quick_start_app();
    assert!(
        app.try_run_from(["build", "-r", "-j", "8"])
            .unwrap()
            .is_ok()
    );
    // The global flag works on either side of the command name, as claimed.
    assert_eq!(
        app.try_parse_from(["-vv", "build"])
            .unwrap()
            .count("verbose"),
        2
    );
    assert_eq!(
        app.try_parse_from(["build", "-vv"])
            .unwrap()
            .count("verbose"),
        2
    );
}

// --- Themed output ---------------------------------------------------------

#[test]
fn readme_themed_output() {
    let _restore = Restore::new();

    Theme::new()
        .set(Level::Success, Style::new().bright_green().bold(), "✓")
        .set(Level::Error, Style::new().bright_red().bold(), "✗")
        .set_glyphs(Glyphs::Auto)
        .install();

    ok("deployed to staging");
    warn("2 tests skipped");
    fail("smoke test failed");
    hint("try `--release` for an optimised build");

    // The claims made around the snippet.
    assert_eq!(Theme::current().glyph(Level::Success), "✓");
    assert!(Theme::plain().style(Level::Error).is_plain());
}

// --- Reusable styles -------------------------------------------------------

#[test]
fn readme_reusable_styles() {
    define(
        "step",
        Style::new().bright_black().prefix("  → ").pad_to(24),
    );

    let step = named("step");
    for label in ["resolve dependencies", "compile", "link"] {
        out(step.paint(label));
    }

    // The claim that matters: the column is measured in display columns, so
    // every row is the same width whatever the content.
    for label in ["compile", "日本語", "éé"] {
        let line = step.paint(label).to_string();
        assert_eq!(text::width(&line), 24, "{label}");
    }
}

// --- The four styling paths ------------------------------------------------

#[test]
fn readme_four_paths() {
    let _restore = Restore::new();

    out(style("ERROR: build failed").red().bold());
    out(markup("<c=red><b>ERROR: build failed</b></c>"));

    define("error", Style::new().red().bold());
    out(named("error").paint("ERROR: build failed"));

    fail("build failed");

    // The claim that makes mixing them safe: identical bytes for identical
    // intent, at every depth.
    terminal::set_color_choice(ColorChoice::Always);
    for level in [
        cli_forge::ColorLevel::Ansi16,
        cli_forge::ColorLevel::Ansi256,
        cli_forge::ColorLevel::TrueColor,
    ] {
        let builder = Style::new().red().bold().paint_at("X", level).to_string();
        assert_eq!(builder, cli_forge::markup_at("<c=red><b>X</b></c>", level));
        assert_eq!(builder, named("error").paint_at("X", level).to_string());
    }
}

#[test]
fn readme_markup_grammar() {
    // Every tag the grammar paragraph lists.
    for snippet in [
        "<b>x</b>",
        "<d>x</d>",
        "<i>x</i>",
        "<u>x</u>",
        "<s>x</s>",
        "<r>x</r>",
        "<c=red>x</c>",
        "<bg=blue>x</bg>",
        "<link=https://example.com>x</link>",
        "<b>x</>",
    ] {
        let rendered = markup(snippet);
        assert_eq!(text::strip(&rendered), "x", "{snippet}");
    }
    // And the two robustness claims.
    assert_eq!(markup("2 << 3"), "2 < 3");
    assert_eq!(markup("a <unknown> b"), "a <unknown> b");

    // Every colour value form the paragraph names.
    for value in ["red", "bright_red", "#ff8800", "#f80", "255,136,0", "208"] {
        assert!(Color::parse(value).is_some(), "{value}");
    }
}

// --- Colours and terminals -------------------------------------------------

#[test]
fn readme_colours() {
    let _restore = Restore::new();

    out(style("amber").hex("#ff8800"));
    out(style("teal").rgb(0, 200, 120));
    out(style("indexed").ansi(208));
    out(style(" PASS ").black().on_green().bold());
    out(style("the docs")
        .cyan()
        .underline()
        .link("https://docs.rs/cli-forge"));

    terminal::set_color_choice(ColorChoice::Never);
    assert!(terminal::level(cli_forge::Stream::Stdout).is_none());
    // A hyperlink adds no columns, as claimed.
    terminal::set_color_choice(ColorChoice::Always);
    let linked = Style::new()
        .link("https://docs.rs/cli-forge")
        .paint("docs")
        .to_string();
    assert_eq!(text::width(&linked), 4);
}

// --- Commands and arguments ------------------------------------------------

/// The command-layer example, with its `main` turned inside out.
fn commands_app() -> App {
    let mut app = App::new("forge")
        .version(env!("CARGO_PKG_VERSION"))
        .arg(Arg::count("verbose").short('v').global(true));

    app.register(
        Command::new("build")
            .about("compile the project")
            .arg(Arg::positional("targets").multiple(true))
            .arg(Arg::flag("release").short('r'))
            .arg(
                Arg::option("jobs")
                    .short('j')
                    .default("1")
                    .value_name("N")
                    .validate(|v| {
                        v.parse::<u16>()
                            .map(|_| ())
                            .map_err(|_| "expected a count".to_string())
                    }),
            )
            .arg(Arg::option("level").possible_values(["warn", "info", "debug"]))
            .arg(Arg::option("token").env("FORGE_TOKEN"))
            .run(|_| -> std::io::Result<()> { Ok(()) }),
    );

    app.register(
        Command::new("remote")
            .subcommand_required(true)
            .subcommand(Command::new("add").arg(Arg::positional("url").required(true))),
    );
    app
}

#[test]
fn readme_commands_parse_every_claimed_form() {
    let app = commands_app();

    // All the standard forms the paragraph lists.
    for argv in [
        vec!["build", "--jobs", "8"],
        vec!["build", "--jobs=8"],
        vec!["build", "-j", "8"],
        vec!["build", "-j8"],
        vec!["build", "-j=8"],
        vec!["build", "-rv"],
        vec!["build", "-vvv"],
        vec!["build", "--", "-r"],
    ] {
        assert!(app.try_parse_from(argv.clone()).is_ok(), "{argv:?}");
    }

    // And the two cases most parsers get wrong.
    let negatives = App::new("calc").command(
        Command::new("add")
            .arg(Arg::positional("a"))
            .arg(Arg::positional("b")),
    );
    let m = negatives.try_parse_from(["add", "-5", "3"]).unwrap();
    assert_eq!(m.leaf().get::<i32>("a"), Some(-5));

    let dash = App::new("cat").command(Command::new("read").arg(Arg::positional("path")));
    assert_eq!(
        dash.try_parse_from(["read", "-"])
            .unwrap()
            .leaf()
            .value("path"),
        Some("-")
    );
}

#[test]
fn readme_validation_happens_at_the_edge() {
    let app = commands_app();

    // Refused with a proper error, naming the flag, the value, and what was
    // acceptable — before any handler runs.
    let error = app.try_run_from(["build", "--jobs", "nope"]).unwrap_err();
    assert_eq!(error.kind(), cli_forge::ErrorKind::InvalidValue);
    assert!(error.report().contains("expected a count"));

    let near_miss = app
        .try_parse_from(["build", "--level", "inof"])
        .unwrap_err();
    assert_eq!(near_miss.suggestion(), Some("info"));

    // So reading it back cannot fail.
    let m = app.try_parse_from(["build"]).unwrap();
    let jobs: u16 = m.leaf().get("jobs").unwrap_or(1);
    assert_eq!(jobs, 1);
    assert_eq!(
        m.leaf().source("jobs"),
        Some(cli_forge::ValueSource::Default)
    );
}

#[test]
fn readme_subcommand_required() {
    let app = commands_app();
    assert_eq!(
        app.try_parse_from(["remote"]).unwrap_err().kind(),
        cli_forge::ErrorKind::MissingSubcommand
    );
    assert!(
        app.try_parse_from(["remote", "add", "https://example.com"])
            .is_ok()
    );
}

#[test]
fn readme_entry_point_table_is_accurate() {
    let app = commands_app();
    // `try_parse_from` parses and nothing else.
    let matches = app.try_parse_from(["build"]).unwrap();
    // `try_run_from` parses and dispatches.
    assert!(app.try_run_from(["build"]).unwrap().is_ok());
    // `dispatch` runs a handler against matches parsed earlier.
    assert!(app.dispatch(&matches).is_ok());
}

// --- Errors ----------------------------------------------------------------

#[test]
fn readme_error_report_matches_the_shown_output() {
    let mut app = App::new("forge");
    app.register(
        Command::new("build")
            .arg(Arg::positional("targets").multiple(true))
            .arg(Arg::flag("release").short('r')),
    );

    let error = app.try_parse_from(["build", "--releaze"]).unwrap_err();
    let report = error.report();

    // The exact three parts the README shows.
    assert!(
        report.contains("error: unknown flag '--releaze'"),
        "{report}"
    );
    assert!(report.contains("did you mean '--release'?"), "{report}");
    assert!(
        report.contains("USAGE: forge build [options] [targets]..."),
        "{report}"
    );

    // And the accessors it lists.
    assert_eq!(error.kind(), cli_forge::ErrorKind::UnknownFlag);
    assert_eq!(error.subject(), "--releaze");
    assert_eq!(error.exit_code(), 2);
    assert_eq!(error.stream(), cli_forge::Stream::Stderr);

    // Help and version are successes on standard output, so `--help` can be
    // piped — the claim at the end of the section.
    let help = app.try_parse_from(["--help"]).unwrap_err();
    assert_eq!(help.kind(), cli_forge::ErrorKind::HelpRequested);
    assert_eq!(help.exit_code(), 0);
    assert_eq!(help.stream(), cli_forge::Stream::Stdout);
}

// --- Untrusted text --------------------------------------------------------

#[test]
fn readme_sanitize() {
    let untrusted_filename = "report.txt\u{1b}[2J\u{1b}[1;1HALL FILES DELETED";
    out(text::sanitize(untrusted_filename));
    assert!(!text::sanitize(untrusted_filename).contains('\u{1b}'));
}

// --- For sibling crates ----------------------------------------------------

#[test]
fn readme_sibling_crate_seams_exist() {
    // Each seam the section promises, exercised the way a sibling crate would.
    let styled = Style::new().red().bold().paint("cell").to_string();
    assert_eq!(text::width(&styled), 4);
    assert_eq!(text::strip(&styled), "cell");
    assert_eq!(text::width(&text::pad("x", 6, text::Align::Right)), 6);
    assert_eq!(text::truncate("abcdef", 4, "…"), "abc…");
    assert_eq!(text::wrap("a b c", 3), ["a b", "c"]);

    let _ = terminal::level(cli_forge::Stream::Stdout);
    let _ = terminal::width_or(80);
    let _ = terminal::supports_unicode();

    // The read-only view of a live command tree.
    let app = commands_app();
    let build = app
        .commands()
        .iter()
        .find(|c| c.name() == "build")
        .expect("build");
    let level = build
        .arguments()
        .iter()
        .find(|a| a.name() == "level")
        .expect("level");
    assert_eq!(level.allowed_values(), ["warn", "info", "debug"]);
    assert!(level.expects_value());
    assert!(!app.global_arguments().is_empty());
    assert!(!build.subcommands().iter().any(|c| c.is_hidden()));
}

// --- Performance claims ----------------------------------------------------

#[test]
fn readme_plain_path_stays_plain() {
    // Not a timing assertion — those belong in the benchmarks — but the
    // structural claim behind the table: an unstyled value gains nothing.
    assert_eq!(Style::new().paint("x").to_string(), "x");
    assert!(Style::new().is_plain());
}

/// A handler signature the README shows, kept compiling.
#[test]
fn readme_handler_signatures_compile() {
    fn typed(m: &Matches) -> Result<(), String> {
        let _: u16 = m.get("jobs").unwrap_or(1);
        Ok(())
    }
    let app = App::new("demo").command(Command::new("x").run(typed));
    assert!(app.try_run_from(["x"]).unwrap().is_ok());
}
