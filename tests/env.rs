//! Environment fallbacks, against real environment variables.
//!
//! Until this file existed, `Arg::env` had only ever been tested with the
//! variable unset — every unit test can see is the "nothing in the environment"
//! branch, because the crate is `#![forbid(unsafe_code)]` and setting a variable
//! is `unsafe` in the 2024 edition.
//!
//! Everything lives in one test, run sequentially, because the environment is
//! process-wide: two tests setting variables on parallel threads would race, and
//! that race is the reason `set_var` became `unsafe` in the first place. Each
//! variable name is unique to this file so nothing else in the process can be
//! reading it.

use cli_forge::{App, Arg, Command, ErrorKind, ValueSource};

/// Set a variable for the duration of a test.
fn set(name: &str, value: &str) {
    // SAFETY: only this test, running on one thread, touches these variables,
    // and nothing in the process reads them through libc directly.
    unsafe { std::env::set_var(name, value) }
}

/// Remove a variable.
fn unset(name: &str) {
    // SAFETY: as for `set`.
    unsafe { std::env::remove_var(name) }
}

fn app() -> App {
    let mut app = App::new("forge");
    app.register(
        Command::new("push")
            .arg(Arg::option("token").env("CLI_FORGE_TEST_TOKEN"))
            .arg(Arg::option("jobs").env("CLI_FORGE_TEST_JOBS").default("1"))
            .arg(Arg::flag("force").env("CLI_FORGE_TEST_FORCE"))
            .arg(
                Arg::flag("cache")
                    .negatable(true)
                    .env("CLI_FORGE_TEST_CACHE"),
            )
            .arg(
                Arg::option("targets")
                    .value_delimiter(',')
                    .env("CLI_FORGE_TEST_TARGETS"),
            )
            .arg(
                Arg::option("level")
                    .possible_values(["warn", "info"])
                    .env("CLI_FORGE_TEST_LEVEL"),
            ),
    );
    app
}

#[test]
fn environment_fallbacks_behave_as_documented() {
    let app = app();

    // An option falls back to its variable, and says so.
    set("CLI_FORGE_TEST_TOKEN", "from-env");
    let m = app.try_parse_from(["push"]).unwrap();
    assert_eq!(m.leaf().value("token"), Some("from-env"));
    assert_eq!(m.leaf().source("token"), Some(ValueSource::Environment));

    // The command line outranks the environment.
    let m = app
        .try_parse_from(["push", "--token", "from-flag"])
        .unwrap();
    assert_eq!(m.leaf().value("token"), Some("from-flag"));
    assert_eq!(m.leaf().source("token"), Some(ValueSource::CommandLine));
    unset("CLI_FORGE_TEST_TOKEN");

    // The environment outranks the default.
    set("CLI_FORGE_TEST_JOBS", "8");
    let m = app.try_parse_from(["push"]).unwrap();
    assert_eq!(m.leaf().get::<u16>("jobs"), Some(8));
    assert_eq!(m.leaf().source("jobs"), Some(ValueSource::Environment));

    // An empty variable counts as unset, which is how a shell clears one.
    set("CLI_FORGE_TEST_JOBS", "");
    let m = app.try_parse_from(["push"]).unwrap();
    assert_eq!(m.leaf().value("jobs"), Some("1"));
    assert_eq!(m.leaf().source("jobs"), Some(ValueSource::Default));
    unset("CLI_FORGE_TEST_JOBS");

    // A truthy value sets a flag; a falsy one leaves an ordinary flag alone.
    set("CLI_FORGE_TEST_FORCE", "1");
    assert!(app.try_parse_from(["push"]).unwrap().leaf().flag("force"));
    set("CLI_FORGE_TEST_FORCE", "off");
    let m = app.try_parse_from(["push"]).unwrap();
    assert!(!m.leaf().flag("force"));
    assert_eq!(m.leaf().explicit_flag("force"), None);
    unset("CLI_FORGE_TEST_FORCE");

    // On a negatable flag, a falsy value is an explicit off...
    set("CLI_FORGE_TEST_CACHE", "false");
    let m = app.try_parse_from(["push"]).unwrap();
    assert_eq!(m.leaf().explicit_flag("cache"), Some(false));
    assert_eq!(m.leaf().source("cache"), Some(ValueSource::Environment));

    // ...and the command line still wins in either direction.
    let m = app.try_parse_from(["push", "--cache"]).unwrap();
    assert_eq!(m.leaf().explicit_flag("cache"), Some(true));
    set("CLI_FORGE_TEST_CACHE", "yes");
    let m = app.try_parse_from(["push", "--no-cache"]).unwrap();
    assert_eq!(m.leaf().explicit_flag("cache"), Some(false));
    unset("CLI_FORGE_TEST_CACHE");

    // A delimited value from the environment splits like one from the flag.
    set("CLI_FORGE_TEST_TARGETS", "linux,macos");
    let m = app.try_parse_from(["push"]).unwrap();
    assert_eq!(
        m.leaf().values("targets").collect::<Vec<_>>(),
        ["linux", "macos"]
    );
    unset("CLI_FORGE_TEST_TARGETS");

    // A value from the environment is validated, and the error names the
    // variable so the user knows where to look.
    set("CLI_FORGE_TEST_LEVEL", "trace");
    let error = app.try_parse_from(["push"]).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidValue);
    assert!(
        error.detail().unwrap().contains("$CLI_FORGE_TEST_LEVEL"),
        "{:?}",
        error.detail()
    );
    unset("CLI_FORGE_TEST_LEVEL");

    // A member supplied through its variable answers a group, and counts as
    // given for the at-most-one rule.
    let grouped = App::new("forge").command(
        Command::new("dump")
            .arg(Arg::flag("json").env("CLI_FORGE_TEST_JSON"))
            .arg(Arg::flag("yaml"))
            .group(
                cli_forge::ArgGroup::new("format")
                    .args(["json", "yaml"])
                    .required(true),
            ),
    );
    set("CLI_FORGE_TEST_JSON", "1");
    assert_eq!(
        grouped
            .try_parse_from(["dump"])
            .unwrap()
            .leaf()
            .group("format"),
        Some("json")
    );
    assert_eq!(
        grouped
            .try_parse_from(["dump", "--yaml"])
            .unwrap_err()
            .kind(),
        ErrorKind::Conflict
    );
    unset("CLI_FORGE_TEST_JSON");

    // With everything unset again, nothing is left over.
    let m = app.try_parse_from(["push"]).unwrap();
    assert_eq!(m.leaf().value("token"), None);
    assert_eq!(m.leaf().explicit_flag("cache"), None);
}
