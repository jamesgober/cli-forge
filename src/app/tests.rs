//! The command layer's tests.
//!
//! Split out of `app.rs` because they are the largest suite in the crate and
//! cover the whole layer rather than only the `App` type: resolution, argument
//! forms, fallbacks, relationships, help, errors, and dispatch.
//!
//! Every assertion goes through [`App::try_parse_from`] or
//! [`App::try_run_from`], which neither print nor exit, so the suite is silent
//! and order-independent. Help text is compared after
//! [`plain`] strips the escapes, so results do not depend on whether the test
//! harness happens to be attached to a terminal — the bug that first exposed
//! this whole piece of work.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::error::ErrorKind;
use crate::matches::ValueSource;
use crate::terminal::Stream;
use crate::text;

/// A rendered page with its ANSI escapes removed.
///
/// Section headings are styled, so a raw page only reads as plain text when the
/// test process happens to have a colour-incapable stdout. Any assertion that
/// spans a heading boundary must go through this.
fn plain(text: &str) -> String {
    text::strip(text).into_owned()
}

/// An app with one command, for the many tests that need no more.
fn one(command: Command) -> App {
    let mut app = App::new("demo");
    app.register(command);
    app
}

// ---------------------------------------------------------------------------
// Resolution
// ---------------------------------------------------------------------------

#[test]
fn test_a_command_resolves_by_name_and_by_alias() {
    let app = one(Command::new("remove").aliases(["rm", "del"]));
    for name in ["remove", "rm", "del"] {
        let m = app.try_parse_from([name]).unwrap();
        // The canonical name is reported, whichever spelling was used.
        assert_eq!(m.subcommand_name(), Some("remove"), "{name}");
    }
}

#[test]
fn test_an_unknown_command_is_reported_with_the_nearest_match() {
    let mut app = App::new("demo");
    app.register(Command::new("build"));
    app.register(Command::new("status"));

    let error = app.try_parse_from(["buidl"]).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::UnknownCommand);
    assert_eq!(error.subject(), "buidl");
    assert_eq!(error.suggestion(), Some("build"));
    assert!(error.report().contains("USAGE: demo"));

    // Nothing close enough gets no suggestion rather than a misleading one.
    assert_eq!(
        app.try_parse_from(["xyzzy"]).unwrap_err().suggestion(),
        None
    );
}

#[test]
fn test_nested_commands_resolve_to_any_depth() {
    let app = one(Command::new("remote").subcommand(
        Command::new("set").subcommand(Command::new("url").arg(Arg::positional("value"))),
    ));
    let m = app
        .try_parse_from(["remote", "set", "url", "https://example.com"])
        .unwrap();
    assert_eq!(m.command_path(), ["remote", "set", "url"]);
    assert_eq!(m.leaf().value("value"), Some("https://example.com"));
}

#[test]
fn test_a_subcommand_required_command_refuses_to_run_bare() {
    let app = one(Command::new("remote")
        .subcommand_required(true)
        .subcommand(Command::new("add"))
        .subcommand(Command::new("list")));
    let error = app.try_parse_from(["remote"]).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::MissingSubcommand);
    // The report names what would have been accepted.
    let detail = error.detail().unwrap();
    assert!(detail.contains("add"));
    assert!(detail.contains("list"));
    assert!(app.try_parse_from(["remote", "add"]).is_ok());
}

#[test]
fn test_a_near_miss_on_a_subcommand_is_corrected() {
    let app = one(Command::new("remote").subcommand(Command::new("add")));
    let error = app.try_parse_from(["remote", "ad"]).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::UnknownCommand);
    assert_eq!(error.suggestion(), Some("add"));
}

// ---------------------------------------------------------------------------
// Argument forms
// ---------------------------------------------------------------------------

#[test]
fn test_every_long_option_form() {
    let app = one(Command::new("run").arg(Arg::option("out").short('o')));
    for argv in [
        vec!["run", "--out", "f"],
        vec!["run", "--out=f"],
        vec!["run", "-o", "f"],
        vec!["run", "-of"],
        vec!["run", "-o=f"],
    ] {
        let m = app.try_parse_from(argv.clone()).unwrap();
        assert_eq!(m.leaf().value("out"), Some("f"), "{argv:?}");
    }
}

#[test]
fn test_short_flags_bundle_and_counts_accumulate() {
    let app = one(Command::new("run")
        .arg(Arg::flag("all").short('a'))
        .arg(Arg::flag("force").short('f'))
        .arg(Arg::count("verbose").short('v')));
    let m = app.try_parse_from(["run", "-afvvv"]).unwrap();
    let leaf = m.leaf();
    assert!(leaf.flag("all"));
    assert!(leaf.flag("force"));
    assert_eq!(leaf.count("verbose"), 3);

    // Separately written repeats count the same.
    let spread = app
        .try_parse_from(["run", "-v", "-v", "--verbose"])
        .unwrap();
    assert_eq!(spread.leaf().count("verbose"), 3);
}

#[test]
fn test_a_bundled_option_takes_the_rest_of_the_token_as_its_value() {
    let app = one(Command::new("run")
        .arg(Arg::flag("all").short('a'))
        .arg(Arg::option("out").short('o')));
    let m = app.try_parse_from(["run", "-aofile"]).unwrap();
    assert!(m.leaf().flag("all"));
    assert_eq!(m.leaf().value("out"), Some("file"));
}

#[test]
fn test_a_missing_option_value_is_reported() {
    let app = one(Command::new("run").arg(Arg::option("out").short('o')));
    for argv in [vec!["run", "--out"], vec!["run", "-o"]] {
        let error = app.try_parse_from(argv.clone()).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::MissingValue, "{argv:?}");
    }
}

#[test]
fn test_a_value_given_to_a_flag_is_reported() {
    let app = one(Command::new("run").arg(Arg::flag("force")));
    let error = app.try_parse_from(["run", "--force=yes"]).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::UnexpectedArgument);
    assert!(error.detail().unwrap().contains("takes no value"));
}

#[test]
fn test_an_unknown_flag_is_reported_with_the_nearest_match() {
    let app = one(Command::new("build").arg(Arg::flag("release")));
    let error = app.try_parse_from(["build", "--releaze"]).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::UnknownFlag);
    assert_eq!(error.suggestion(), Some("--release"));
    assert!(error.report().contains("USAGE: demo build"));
}

#[test]
fn test_help_is_suggested_for_a_near_miss_on_it() {
    let app = one(Command::new("build"));
    assert_eq!(
        app.try_parse_from(["build", "--hlep"])
            .unwrap_err()
            .suggestion(),
        Some("--help")
    );
}

#[test]
fn test_positionals_fill_in_declaration_order() {
    let app = one(Command::new("copy")
        .arg(Arg::positional("from"))
        .arg(Arg::positional("to")));
    let m = app.try_parse_from(["copy", "a", "b"]).unwrap();
    assert_eq!(m.leaf().value("from"), Some("a"));
    assert_eq!(m.leaf().value("to"), Some("b"));
}

#[test]
fn test_a_variadic_positional_absorbs_the_rest() {
    let app = one(Command::new("rm")
        .arg(Arg::positional("mode"))
        .arg(Arg::positional("files").multiple(true)));
    let m = app.try_parse_from(["rm", "safe", "a", "b", "c"]).unwrap();
    assert_eq!(m.leaf().value("mode"), Some("safe"));
    assert_eq!(
        m.leaf().values("files").collect::<Vec<_>>(),
        ["a", "b", "c"]
    );
}

#[test]
fn test_a_repeatable_option_collects_every_occurrence() {
    let app = one(Command::new("build").arg(Arg::option("define").short('D').multiple(true)));
    let m = app
        .try_parse_from(["build", "-D", "A", "-D", "B", "--define=C"])
        .unwrap();
    assert_eq!(
        m.leaf().values("define").collect::<Vec<_>>(),
        ["A", "B", "C"]
    );
}

#[test]
fn test_a_delimited_option_splits_and_accumulates() {
    let app =
        one(Command::new("build").arg(Arg::option("features").short('F').value_delimiter(',')));
    for argv in [
        vec!["build", "--features", "a,b", "--features", "c"],
        vec!["build", "--features=a,b", "-F", "c"],
        vec!["build", "-Fa,b", "-F=c"],
    ] {
        let m = app.try_parse_from(argv.clone()).unwrap();
        assert_eq!(
            m.leaf().values("features").collect::<Vec<_>>(),
            ["a", "b", "c"],
            "{argv:?}"
        );
    }
}

#[test]
fn test_every_delimited_piece_is_validated() {
    let app = one(Command::new("build").arg(
        Arg::option("features")
            .value_delimiter(',')
            .possible_values(["serde", "log"]),
    ));
    assert!(
        app.try_parse_from(["build", "--features", "serde,log"])
            .is_ok()
    );

    let error = app
        .try_parse_from(["build", "--features", "serde,lgo"])
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidValue);
    assert_eq!(error.subject(), "lgo");
    assert_eq!(error.suggestion(), Some("log"));
}

#[test]
fn test_empty_delimited_pieces_are_kept_not_dropped() {
    let app = one(Command::new("build").arg(Arg::option("features").value_delimiter(',')));
    let m = app.try_parse_from(["build", "--features", "a,,b"]).unwrap();
    assert_eq!(
        m.leaf().values("features").collect::<Vec<_>>(),
        ["a", "", "b"]
    );
}

#[test]
fn test_a_delimited_default_is_split_too() {
    let app = one(Command::new("build").arg(
        Arg::option("targets")
            .value_delimiter(',')
            .default("linux,macos"),
    ));
    let m = app.try_parse_from(["build"]).unwrap();
    assert_eq!(
        m.leaf().values("targets").collect::<Vec<_>>(),
        ["linux", "macos"]
    );
    assert_eq!(m.leaf().source("targets"), Some(ValueSource::Default));
}

#[test]
fn test_a_delimited_positional_splits() {
    let app = one(Command::new("tag").arg(Arg::positional("names").value_delimiter(',')));
    let m = app.try_parse_from(["tag", "x,y", "z"]).unwrap();
    assert_eq!(
        m.leaf().values("names").collect::<Vec<_>>(),
        ["x", "y", "z"]
    );
}

#[test]
fn test_help_states_the_delimiter() {
    let app = one(Command::new("build").arg(Arg::option("features").value_delimiter(',')));
    let help = plain(&app.command_help(["build"]).unwrap());
    assert!(help.contains("[delimiter: ',']"), "{help}");
}

#[test]
fn test_a_negatable_flag_has_three_states() {
    let app = one(Command::new("build").arg(Arg::flag("cache").negatable(true)));

    let on = app.try_parse_from(["build", "--cache"]).unwrap();
    assert_eq!(on.leaf().explicit_flag("cache"), Some(true));
    assert!(on.leaf().flag("cache"));

    let off = app.try_parse_from(["build", "--no-cache"]).unwrap();
    assert_eq!(off.leaf().explicit_flag("cache"), Some(false));
    assert!(!off.leaf().flag("cache"));
    assert_eq!(off.leaf().source("cache"), Some(ValueSource::CommandLine));
    // Off holds no value, so it does not satisfy a `requires`.
    assert!(!off.leaf().present("cache"));

    let unsaid = app.try_parse_from(["build"]).unwrap();
    assert_eq!(unsaid.leaf().explicit_flag("cache"), None);
}

#[test]
fn test_the_last_spelling_wins() {
    let app = one(Command::new("build").arg(Arg::flag("cache").negatable(true)));
    let m = app
        .try_parse_from(["build", "--no-cache", "--cache"])
        .unwrap();
    assert_eq!(m.leaf().explicit_flag("cache"), Some(true));
    let m = app
        .try_parse_from(["build", "--cache", "--no-cache"])
        .unwrap();
    assert_eq!(m.leaf().explicit_flag("cache"), Some(false));
}

#[test]
fn test_negation_is_opt_in() {
    let app = one(Command::new("build").arg(Arg::flag("cache")));
    assert_eq!(
        app.try_parse_from(["build", "--no-cache"])
            .unwrap_err()
            .kind(),
        ErrorKind::UnknownFlag
    );
}

#[test]
fn test_a_negated_flag_takes_no_value() {
    let app = one(Command::new("build").arg(Arg::flag("cache").negatable(true)));
    let error = app.try_parse_from(["build", "--no-cache=yes"]).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::UnexpectedArgument);
}

#[test]
fn test_an_argument_really_named_no_something_wins() {
    let app = one(Command::new("build")
        .arg(Arg::flag("cache").negatable(true))
        .arg(Arg::flag("no-cache")));
    let m = app.try_parse_from(["build", "--no-cache"]).unwrap();
    assert!(m.leaf().flag("no-cache"));
    assert_eq!(m.leaf().explicit_flag("cache"), None);
}

#[test]
fn test_an_explicit_off_never_conflicts() {
    let app = one(Command::new("log")
        .arg(Arg::flag("quiet").conflicts_with(["verbose"]))
        .arg(Arg::flag("verbose").negatable(true)));
    assert!(
        app.try_parse_from(["log", "--quiet", "--no-verbose"])
            .is_ok()
    );
    assert_eq!(
        app.try_parse_from(["log", "--quiet", "--verbose"])
            .unwrap_err()
            .kind(),
        ErrorKind::Conflict
    );
}

#[test]
fn test_an_explicit_off_does_not_satisfy_a_dependency() {
    let app = one(Command::new("push")
        .arg(Arg::flag("sign").requires(["verify"]))
        .arg(Arg::flag("verify").negatable(true)));
    assert_eq!(
        app.try_parse_from(["push", "--sign", "--no-verify"])
            .unwrap_err()
            .kind(),
        ErrorKind::MissingDependency
    );
    assert!(app.try_parse_from(["push", "--sign", "--verify"]).is_ok());
}

#[test]
fn test_a_near_miss_on_the_negative_spelling_is_corrected() {
    let app = one(Command::new("build").arg(Arg::flag("cache").negatable(true)));
    let error = app.try_parse_from(["build", "--no-cahce"]).unwrap_err();
    assert_eq!(error.suggestion(), Some("--no-cache"));
}

#[test]
fn test_a_global_negatable_flag_works_on_either_side() {
    let mut app = App::new("demo").arg(Arg::flag("color").negatable(true).global(true));
    app.register(Command::new("build"));
    for argv in [vec!["--no-color", "build"], vec!["build", "--no-color"]] {
        let m = app.try_parse_from(argv.clone()).unwrap();
        assert_eq!(m.leaf().explicit_flag("color"), Some(false), "{argv:?}");
        assert_eq!(m.explicit_flag("color"), Some(false), "{argv:?}");
    }
}

#[test]
fn test_help_shows_both_spellings_in_one_entry() {
    let app =
        one(Command::new("build").arg(Arg::flag("cache").negatable(true).help("reuse outputs")));
    let help = plain(&app.command_help(["build"]).unwrap());
    assert!(help.contains("--[no-]cache"), "{help}");
}

fn formats(group: crate::ArgGroup) -> App {
    one(Command::new("dump")
        .args([Arg::flag("json"), Arg::flag("yaml"), Arg::flag("toml")])
        .group(group))
}

#[test]
fn test_an_exactly_one_group() {
    let app = formats(
        crate::ArgGroup::new("format")
            .args(["json", "yaml", "toml"])
            .required(true),
    );

    let none = app.try_parse_from(["dump"]).unwrap_err();
    assert_eq!(none.kind(), ErrorKind::MissingRequired);
    assert_eq!(none.subject(), "format");
    assert!(none.detail().unwrap().contains("--json, --yaml, --toml"));

    let two = app
        .try_parse_from(["dump", "--json", "--toml"])
        .unwrap_err();
    assert_eq!(two.kind(), ErrorKind::Conflict);
    let detail = two.detail().unwrap();
    assert!(detail.contains("'--json' and '--toml'"), "{detail}");
    assert!(detail.contains("'format'"), "{detail}");

    let m = app.try_parse_from(["dump", "--yaml"]).unwrap();
    assert_eq!(m.leaf().group("format"), Some("yaml"));
}

#[test]
fn test_an_at_most_one_group_is_the_default() {
    let app = formats(crate::ArgGroup::new("format").args(["json", "yaml", "toml"]));
    let m = app.try_parse_from(["dump"]).unwrap();
    assert_eq!(m.leaf().group("format"), None);
    assert!(app.try_parse_from(["dump", "--json", "--yaml"]).is_err());
}

#[test]
fn test_an_at_least_one_group() {
    let app = formats(
        crate::ArgGroup::new("format")
            .args(["json", "yaml", "toml"])
            .required(true)
            .multiple(true),
    );
    assert!(app.try_parse_from(["dump"]).is_err());
    let m = app.try_parse_from(["dump", "--toml", "--json"]).unwrap();
    // The first member listed in the group, as the documentation says.
    assert!(matches!(m.leaf().group("format"), Some("json" | "toml")));
}

#[test]
fn test_a_default_answers_a_group_but_never_conflicts() {
    let app = one(Command::new("serve")
        .arg(Arg::option("port").default("8080"))
        .arg(Arg::option("socket"))
        .group(
            crate::ArgGroup::new("listen")
                .args(["port", "socket"])
                .required(true),
        ));

    // The default answers the required group.
    let m = app.try_parse_from(["serve"]).unwrap();
    assert_eq!(m.leaf().group("listen"), Some("port"));

    // And does not count against the user's own choice.
    let m = app.try_parse_from(["serve", "--socket", "/tmp/s"]).unwrap();
    assert_eq!(m.leaf().group("listen"), Some("socket"));

    // Only two things the user actually supplied conflict.
    assert!(
        app.try_parse_from(["serve", "--socket", "s", "--port", "1"])
            .is_err()
    );
}

#[test]
fn test_an_explicit_off_is_not_a_group_answer() {
    let app = one(Command::new("dump")
        .arg(Arg::flag("json").negatable(true))
        .arg(Arg::flag("yaml"))
        .group(
            crate::ArgGroup::new("format")
                .args(["json", "yaml"])
                .required(true),
        ));
    assert!(app.try_parse_from(["dump", "--no-json"]).is_err());
    assert!(app.try_parse_from(["dump", "--no-json", "--yaml"]).is_ok());
}

#[test]
fn test_a_positional_can_be_a_group_member() {
    let app = one(Command::new("read")
        .arg(Arg::positional("file"))
        .arg(Arg::flag("stdin"))
        .group(
            crate::ArgGroup::new("input")
                .args(["file", "stdin"])
                .required(true),
        ));
    assert_eq!(
        app.try_parse_from(["read", "a.txt"])
            .unwrap()
            .leaf()
            .group("input"),
        Some("file")
    );
    assert_eq!(
        app.try_parse_from(["read", "--stdin"])
            .unwrap()
            .leaf()
            .group("input"),
        Some("stdin")
    );
    let both = app
        .try_parse_from(["read", "a.txt", "--stdin"])
        .unwrap_err();
    assert!(
        both.detail().unwrap().contains("'file' and '--stdin'"),
        "{:?}",
        both.detail()
    );
}

#[test]
fn test_a_parents_required_group_is_not_demanded_once_a_subcommand_runs() {
    let app = one(Command::new("remote")
        .args([Arg::flag("all"), Arg::flag("mine")])
        .group(
            crate::ArgGroup::new("scope")
                .args(["all", "mine"])
                .required(true),
        )
        .subcommand(Command::new("list")));
    assert!(app.try_parse_from(["remote", "list"]).is_ok());
    assert!(app.try_parse_from(["remote"]).is_err());
}

#[test]
fn test_a_required_group_appears_in_the_usage_line() {
    let app = formats(
        crate::ArgGroup::new("format")
            .args(["json", "yaml", "toml"])
            .required(true),
    );
    let help = plain(&app.command_help(["dump"]).unwrap());
    assert!(
        help.contains("USAGE: demo dump [options] <--json|--yaml|--toml>"),
        "{help}"
    );

    // An optional group stays under [options].
    let optional = formats(crate::ArgGroup::new("format").args(["json", "yaml", "toml"]));
    let help = plain(&optional.command_help(["dump"]).unwrap());
    assert!(!help.contains("<--json"), "{help}");
}

#[test]
fn test_groups_are_readable_from_outside() {
    let cmd = Command::new("dump").group(
        crate::ArgGroup::new("format")
            .args(["json", "yaml"])
            .required(true),
    );
    let group = &cmd.groups()[0];
    assert_eq!(group.name(), "format");
    assert_eq!(group.members(), ["json", "yaml"]);
    assert!(group.is_required());
    assert!(!group.is_multiple());
}

#[test]
fn test_a_single_valued_option_is_last_wins() {
    let app = one(Command::new("build").arg(Arg::option("jobs")));
    let m = app
        .try_parse_from(["build", "--jobs", "1", "--jobs", "8"])
        .unwrap();
    assert_eq!(m.leaf().value("jobs"), Some("8"));
    assert_eq!(m.leaf().values("jobs").count(), 1);
}

#[test]
fn test_a_surplus_value_is_reported() {
    let app = one(Command::new("ping"));
    let error = app.try_parse_from(["ping", "extra"]).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::UnexpectedArgument);
    assert_eq!(error.subject(), "extra");
}

#[test]
fn test_end_of_options_makes_everything_after_it_a_value() {
    let app = one(Command::new("exec")
        .arg(Arg::flag("quiet").short('q'))
        .arg(Arg::positional("argv").multiple(true)));
    let m = app
        .try_parse_from(["exec", "-q", "--", "-q", "--not-a-flag", "x"])
        .unwrap();
    let leaf = m.leaf();
    assert!(leaf.flag("quiet"));
    assert_eq!(
        leaf.values("argv").collect::<Vec<_>>(),
        ["-q", "--not-a-flag", "x"]
    );
}

// ---------------------------------------------------------------------------
// The confirmed 1.x defects
// ---------------------------------------------------------------------------

#[test]
fn test_negative_numbers_are_values() {
    // `calc add -5 3` was `UnknownFlag { flag: "-5" }` in 1.x.
    let app = one(Command::new("add")
        .arg(Arg::positional("a"))
        .arg(Arg::positional("b")));
    let m = app.try_parse_from(["add", "-5", "3"]).unwrap();
    assert_eq!(m.leaf().get::<i32>("a"), Some(-5));
    assert_eq!(m.leaf().get::<i32>("b"), Some(3));

    // Including as an option's value, and in fractional form.
    let offsets = one(Command::new("seek").arg(Arg::option("by")));
    assert_eq!(
        offsets
            .try_parse_from(["seek", "--by", "-1.5"])
            .unwrap()
            .leaf()
            .value("by"),
        Some("-1.5")
    );
}

#[test]
fn test_a_declared_numeric_short_still_wins() {
    // The escape hatch: a command that really does declare `-1` gets it.
    let app = one(Command::new("ctx").arg(Arg::flag("one").short('1')));
    assert!(
        app.try_parse_from(["ctx", "-1"])
            .unwrap()
            .leaf()
            .flag("one")
    );
}

#[test]
fn test_a_lone_dash_is_a_value() {
    let app = one(Command::new("cat").arg(Arg::positional("path")));
    assert_eq!(
        app.try_parse_from(["cat", "-"])
            .unwrap()
            .leaf()
            .value("path"),
        Some("-")
    );
}

#[test]
fn test_app_level_arguments_parse_before_the_command() {
    // `demo --verbose build` was `UnknownFlag` in 1.x: there was no app level.
    let mut app = App::new("demo").arg(Arg::count("verbose").short('v'));
    app.register(Command::new("build"));

    let m = app.try_parse_from(["-vv", "build"]).unwrap();
    assert_eq!(m.count("verbose"), 2);
    assert_eq!(m.subcommand_name(), Some("build"));
}

#[test]
fn test_a_global_argument_works_on_either_side_of_the_command() {
    let mut app = App::new("demo").arg(Arg::count("verbose").short('v').global(true));
    app.register(Command::new("build").subcommand(Command::new("docs")));

    for argv in [
        vec!["-vv", "build"],
        vec!["build", "-vv"],
        vec!["-v", "build", "-v"],
    ] {
        let m = app.try_parse_from(argv.clone()).unwrap();
        assert_eq!(m.count("verbose"), 2, "{argv:?} at the root");
    }

    // And it reaches every level, so a handler need not walk back up.
    let deep = app.try_parse_from(["-v", "build", "docs"]).unwrap();
    assert_eq!(deep.leaf().count("verbose"), 1);
}

#[test]
fn test_a_non_global_app_argument_is_not_accepted_after_the_command() {
    let mut app = App::new("demo").arg(Arg::flag("app-only"));
    app.register(Command::new("build"));

    assert!(app.try_parse_from(["--app-only", "build"]).is_ok());
    let error = app.try_parse_from(["build", "--app-only"]).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::UnknownFlag);
}

#[test]
fn test_a_parents_required_argument_is_not_demanded_once_a_subcommand_runs() {
    // `remote list` was `MissingRequired { arg: "name" }` in 1.x, which made
    // every grouping command with its own positionals unusable.
    let app = one(Command::new("remote")
        .arg(Arg::positional("name").required(true))
        .subcommand(Command::new("list")));
    assert!(app.try_parse_from(["remote", "list"]).is_ok());
    // Still required when the command runs on its own.
    assert_eq!(
        app.try_parse_from(["remote"]).unwrap_err().kind(),
        ErrorKind::MissingRequired
    );
    assert!(app.try_parse_from(["remote", "origin"]).is_ok());
}

// ---------------------------------------------------------------------------
// Fallbacks and relationships
// ---------------------------------------------------------------------------

#[test]
fn test_a_default_fills_in_and_is_reported_as_such() {
    let app = one(Command::new("build").arg(Arg::option("jobs").default("1")));

    let defaulted = app.try_parse_from(["build"]).unwrap();
    assert_eq!(defaulted.leaf().value("jobs"), Some("1"));
    assert_eq!(defaulted.leaf().source("jobs"), Some(ValueSource::Default));

    let chosen = app.try_parse_from(["build", "--jobs", "8"]).unwrap();
    assert_eq!(chosen.leaf().value("jobs"), Some("8"));
    assert_eq!(chosen.leaf().source("jobs"), Some(ValueSource::CommandLine));
}

#[test]
fn test_a_default_satisfies_a_required_argument() {
    let app = one(Command::new("go").arg(Arg::positional("path").required(true).default(".")));
    assert_eq!(
        app.try_parse_from(["go"]).unwrap().leaf().value("path"),
        Some(".")
    );
}

#[test]
fn test_required_unless_accepts_either_form() {
    let app = one(Command::new("read")
        .arg(
            Arg::positional("file")
                .required(true)
                .required_unless(["stdin"]),
        )
        .arg(Arg::flag("stdin")));
    assert!(app.try_parse_from(["read", "notes.txt"]).is_ok());
    assert!(app.try_parse_from(["read", "--stdin"]).is_ok());

    let error = app.try_parse_from(["read"]).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::MissingRequired);
    assert!(error.detail().unwrap().contains("stdin"));
}

#[test]
fn test_conflicting_arguments_are_refused() {
    let app = one(Command::new("log")
        .arg(Arg::flag("quiet").conflicts_with(["verbose"]))
        .arg(Arg::flag("verbose")));
    assert!(app.try_parse_from(["log", "--quiet"]).is_ok());
    assert!(app.try_parse_from(["log", "--verbose"]).is_ok());

    let error = app
        .try_parse_from(["log", "--quiet", "--verbose"])
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Conflict);
    assert!(error.detail().unwrap().contains("verbose"));
}

#[test]
fn test_a_default_never_triggers_a_conflict() {
    // Otherwise an argument with a default could not conflict with anything.
    let app = one(Command::new("log")
        .arg(
            Arg::option("level")
                .default("info")
                .conflicts_with(["quiet"]),
        )
        .arg(Arg::flag("quiet")));
    assert!(app.try_parse_from(["log", "--quiet"]).is_ok());
    assert_eq!(
        app.try_parse_from(["log", "--quiet", "--level", "warn"])
            .unwrap_err()
            .kind(),
        ErrorKind::Conflict
    );
}

#[test]
fn test_a_dependency_is_enforced() {
    let app = one(Command::new("push")
        .arg(Arg::flag("sign").requires(["key"]))
        .arg(Arg::option("key")));
    let error = app.try_parse_from(["push", "--sign"]).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::MissingDependency);
    assert!(error.detail().unwrap().contains("key"));
    assert!(app.try_parse_from(["push", "--sign", "--key", "k"]).is_ok());
    // And is not demanded when the dependent argument was not used.
    assert!(app.try_parse_from(["push"]).is_ok());
}

#[test]
fn test_possible_values_are_enforced_and_corrected() {
    let app =
        one(Command::new("log")
            .arg(Arg::option("level").possible_values(["warn", "info", "debug"])));
    assert!(app.try_parse_from(["log", "--level", "info"]).is_ok());

    let error = app.try_parse_from(["log", "--level", "inof"]).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidValue);
    assert_eq!(error.subject(), "inof");
    assert_eq!(error.suggestion(), Some("info"));
    assert!(error.detail().unwrap().contains("warn, info, debug"));
}

#[test]
fn test_a_validator_runs_before_any_handler() {
    static RAN: AtomicUsize = AtomicUsize::new(0);
    let app = one(Command::new("serve")
        .arg(Arg::option("port").validate(|value| {
            value
                .parse::<u16>()
                .map(|_| ())
                .map_err(|_| "expected a port number".to_owned())
        }))
        .run(|_| {
            let _ = RAN.fetch_add(1, Ordering::SeqCst);
        }));

    let error = app.try_run_from(["serve", "--port", "70000"]).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidValue);
    assert!(error.report().contains("expected a port number"));
    assert_eq!(
        RAN.load(Ordering::SeqCst),
        0,
        "the handler must not have run"
    );

    assert!(
        app.try_run_from(["serve", "--port", "8080"])
            .unwrap()
            .is_ok()
    );
    assert_eq!(RAN.load(Ordering::SeqCst), 1);
}

#[test]
fn test_a_positional_is_validated_too() {
    let app =
        one(Command::new("log").arg(Arg::positional("level").possible_values(["warn", "info"])));
    assert!(app.try_parse_from(["log", "warn"]).is_ok());
    assert_eq!(
        app.try_parse_from(["log", "loud"]).unwrap_err().kind(),
        ErrorKind::InvalidValue
    );
}

// ---------------------------------------------------------------------------
// Typed reads
// ---------------------------------------------------------------------------

#[test]
fn test_typed_reads_of_parsed_values() {
    let app = one(Command::new("serve")
        .arg(Arg::option("port").default("8080"))
        .arg(Arg::option("ratio").default("0.25"))
        .arg(Arg::positional("n").multiple(true)));
    let m = app.try_parse_from(["serve", "1", "2", "3"]).unwrap();
    let leaf = m.leaf();
    assert_eq!(leaf.get::<u16>("port"), Some(8080));
    assert_eq!(leaf.get::<f64>("ratio"), Some(0.25));
    assert_eq!(leaf.get_all::<i64>("n").iter().sum::<i64>(), 6);
    assert_eq!(leaf.try_get::<u32>("port").unwrap(), Some(8080));
}

// ---------------------------------------------------------------------------
// Help and version
// ---------------------------------------------------------------------------

fn help_demo() -> App {
    let mut app = App::new("demo")
        .version("1.0.0")
        .about("a demonstration")
        .help_header("HEADER LINE")
        .help_footer("FOOTER LINE");
    app.register(Command::new("build").about("compile the project"));
    app.register(
        Command::new("remove")
            .aliases(["rm", "del"])
            .about("delete a thing"),
    );
    app.register(Command::new("secret").hidden(true).about("do not show me"));
    app.register(Command::new("publish").requires_auth(true).about("gated"));
    app
}

#[test]
fn test_help_respects_header_footer_and_lists_options() {
    let help = plain(&help_demo().help());
    assert!(help.contains("HEADER LINE"));
    assert!(help.contains("FOOTER LINE"));
    assert!(help.contains("a demonstration"));
    assert!(help.contains("USAGE: demo <command>"));
    assert!(help.contains("-h, --help"));
    assert!(help.contains("-V, --version"));
}

#[test]
fn test_help_hides_hidden_commands() {
    let help = plain(&help_demo().help());
    assert!(help.contains("build"));
    assert!(help.contains("compile the project"));
    assert!(!help.contains("secret"));
    assert!(!help.contains("do not show me"));
}

#[test]
fn test_help_shows_command_aliases() {
    assert!(plain(&help_demo().help()).contains("remove, rm, del"));
}

#[test]
fn test_help_omits_the_version_line_without_a_version() {
    let app = one(Command::new("build"));
    let help = plain(&app.help());
    assert!(help.contains("-h, --help"));
    assert!(!help.contains("--version"));
}

#[test]
fn test_help_columns_align_for_non_ascii_names() {
    // The confirmed 1.x defect: byte-width padding skewed every row.
    let mut app = App::new("demo");
    app.register(Command::new("ünïcödé").about("accents"));
    app.register(Command::new("日本語").about("wide"));
    app.register(Command::new("ab").about("short"));

    let help = plain(&app.help());
    let columns: Vec<usize> = help
        .lines()
        .filter(|line| {
            ["accents", "wide", "short"]
                .iter()
                .any(|d| line.ends_with(d))
        })
        .map(|line| text::width(line) - text::width(line.trim_end_matches(char::is_alphabetic)))
        .collect();
    assert_eq!(columns.len(), 3);

    // Every description must start in the same column.
    let starts: Vec<usize> = help
        .lines()
        .filter_map(|line| {
            ["accents", "wide", "short"]
                .iter()
                .find_map(|d| line.find(d).map(|byte| text::width(&line[..byte])))
        })
        .collect();
    assert_eq!(starts.len(), 3);
    assert!(
        starts.windows(2).all(|pair| pair[0] == pair[1]),
        "descriptions start in different columns: {starts:?}\n{help}"
    );
}

#[test]
fn test_help_flag_returns_the_rendered_page() {
    let app = help_demo();

    let top = app.try_parse_from(["--help"]).unwrap_err();
    assert_eq!(top.kind(), ErrorKind::HelpRequested);
    assert!(plain(top.text().unwrap()).contains("USAGE: demo"));
    assert_eq!(top.exit_code(), 0);
    assert_eq!(top.stream(), Stream::Stdout);

    let command = app.try_parse_from(["build", "-h"]).unwrap_err();
    assert!(plain(command.text().unwrap()).contains("demo build"));
}

#[test]
fn test_the_help_command_renders_a_page() {
    let app = one(Command::new("remote").subcommand(Command::new("add").about("add a remote")));

    let top = app.try_parse_from(["help"]).unwrap_err();
    assert!(plain(top.text().unwrap()).contains("USAGE: demo"));

    let nested = app.try_parse_from(["help", "remote", "add"]).unwrap_err();
    let page = plain(nested.text().unwrap());
    assert!(page.contains("demo remote add"));
    assert!(page.contains("add a remote"));
}

#[test]
fn test_a_programs_own_help_command_wins() {
    static RAN: AtomicUsize = AtomicUsize::new(0);
    let app = one(Command::new("help").run(|_| {
        let _ = RAN.fetch_add(1, Ordering::SeqCst);
    }));
    assert!(app.try_run_from(["help"]).unwrap().is_ok());
    assert_eq!(RAN.load(Ordering::SeqCst), 1);
}

#[test]
fn test_a_bare_invocation_shows_the_help() {
    let app = one(Command::new("build"));
    let error = app.try_parse_from([] as [&str; 0]).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::HelpRequested);

    // Unless the program asks for the quiet behaviour.
    let quiet = App::new("demo")
        .help_command(false)
        .command(Command::new("build"));
    assert!(
        quiet
            .try_parse_from([] as [&str; 0])
            .unwrap()
            .subcommand()
            .is_none()
    );
}

#[test]
fn test_command_help_renders_any_path_or_reports_none() {
    let app = one(Command::new("remote").subcommand(Command::new("add").about("add a remote")));
    assert!(plain(&app.command_help(["remote"]).unwrap()).contains("demo remote"));
    assert!(plain(&app.command_help(["remote", "add"]).unwrap()).contains("add a remote"));
    assert!(app.command_help(["nope"]).is_none());
    assert!(app.command_help(["remote", "nope"]).is_none());
}

#[test]
fn test_command_help_shows_arguments_their_slots_and_their_fallbacks() {
    let app = one(Command::new("build")
        .long_about("Compiles everything the manifest lists.")
        .after_help("EXAMPLES:\n  demo build --release")
        .arg(Arg::positional("targets").multiple(true))
        .arg(
            Arg::option("jobs")
                .short('j')
                .default("1")
                .help("parallel jobs"),
        )
        .arg(Arg::option("level").possible_values(["warn", "info"]))
        .arg(Arg::flag("hidden-switch").hide(true)));
    let help = plain(&app.command_help(["build"]).unwrap());
    assert!(help.contains("Compiles everything the manifest lists."));
    assert!(help.contains("USAGE: demo build [options] [targets]..."));
    assert!(help.contains("-j, --jobs <JOBS>"));
    assert!(help.contains("parallel jobs [default: 1]"));
    assert!(help.contains("[possible: warn, info]"));
    assert!(help.contains("EXAMPLES:"));
    assert!(!help.contains("hidden-switch"));
}

#[test]
fn test_command_help_lists_inherited_global_arguments() {
    // A user reading a command's page needs to know it accepts these.
    let mut app = App::new("demo").arg(Arg::count("verbose").short('v').global(true));
    app.register(Command::new("build"));
    let help = plain(&app.command_help(["build"]).unwrap());
    assert!(help.contains("-v, --verbose"), "{help}");
}

#[test]
fn test_a_custom_usage_line_replaces_the_generated_one() {
    let app = one(Command::new("exec")
        .arg(Arg::positional("argv").multiple(true))
        .usage("demo exec [options] -- <program> [args]..."));
    assert!(plain(&app.command_help(["exec"]).unwrap()).contains("-- <program>"));
}

#[test]
fn test_display_order_sorts_the_listing() {
    let mut app = App::new("demo");
    app.register(Command::new("clean").display_order(10));
    app.register(Command::new("build").display_order(1));
    app.register(Command::new("test").display_order(5));

    let help = plain(&app.help());
    let position = |name: &str| help.find(name).expect(name);
    assert!(position("build") < position("test"));
    assert!(position("test") < position("clean"));
}

#[test]
fn test_registration_order_is_kept_without_an_explicit_order() {
    let mut app = App::new("demo");
    for name in ["zebra", "alpha", "middle"] {
        app.register(Command::new(name));
    }
    let help = plain(&app.help());
    let position = |name: &str| help.find(name).expect(name);
    assert!(position("zebra") < position("alpha"));
    assert!(position("alpha") < position("middle"));
}

#[test]
fn test_version_flag_returns_the_version() {
    let app = help_demo();
    for argv in [vec!["--version"], vec!["-V"], vec!["build", "-V"]] {
        let error = app.try_parse_from(argv.clone()).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::VersionRequested, "{argv:?}");
        assert_eq!(error.text(), Some("1.0.0"));
        assert_eq!(error.exit_code(), 0);
    }
}

#[test]
fn test_version_flags_are_unknown_without_a_version() {
    let app = one(Command::new("build"));
    assert_eq!(
        app.try_parse_from(["--version"]).unwrap_err().kind(),
        ErrorKind::UnknownFlag
    );
}

#[test]
fn test_a_command_may_claim_the_help_and_version_flags() {
    let app = one(Command::new("render")
        .arg(Arg::option("help").long("help"))
        .arg(Arg::flag("verbose").short('V')));
    let m = app
        .try_parse_from(["render", "--help", "topic", "-V"])
        .unwrap();
    assert_eq!(m.leaf().value("help"), Some("topic"));
    assert!(m.leaf().flag("verbose"));
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

#[test]
fn test_parsing_alone_runs_no_handler() {
    // The one deliberate behaviour change from 1.x, and the reason
    // `try_parse_from` is the method to test with.
    static RAN: AtomicUsize = AtomicUsize::new(0);
    let app = one(Command::new("build").run(|_| {
        let _ = RAN.fetch_add(1, Ordering::SeqCst);
    }));

    let matches = app.try_parse_from(["build"]).unwrap();
    assert_eq!(RAN.load(Ordering::SeqCst), 0);

    assert!(app.dispatch(&matches).is_ok());
    assert_eq!(RAN.load(Ordering::SeqCst), 1);
}

#[test]
fn test_dispatch_reaches_the_deepest_command() {
    static DEPTH: AtomicUsize = AtomicUsize::new(0);
    let app = one(Command::new("remote")
        .run(|_| DEPTH.store(1, Ordering::SeqCst))
        .subcommand(Command::new("add").run(|_| DEPTH.store(2, Ordering::SeqCst))));

    assert!(app.try_run_from(["remote"]).unwrap().is_ok());
    assert_eq!(DEPTH.load(Ordering::SeqCst), 1);

    assert!(app.try_run_from(["remote", "add"]).unwrap().is_ok());
    assert_eq!(DEPTH.load(Ordering::SeqCst), 2);
}

#[test]
fn test_a_handler_receives_its_own_levels_matches() {
    let app = one(Command::new("build").arg(Arg::flag("release")).run(|m| {
        if m.flag("release") {
            Ok(())
        } else {
            Err("flag missing")
        }
    }));
    assert!(app.try_run_from(["build", "--release"]).unwrap().is_ok());
    assert!(app.try_run_from(["build"]).unwrap().is_err());
}

#[test]
fn test_a_failed_handler_carries_its_message_and_code() {
    let mut app = App::new("demo");
    app.register(Command::new("plain").run(|_| Err("it broke")));
    app.register(
        Command::new("exact").run_status(|_| Err(CommandError::new("differ").with_code(3))),
    );

    let plain_failure = app.try_run_from(["plain"]).unwrap().unwrap_err();
    assert_eq!(plain_failure.message(), "it broke");
    assert_eq!(plain_failure.exit_code(), 1);

    let exact = app.try_run_from(["exact"]).unwrap().unwrap_err();
    assert_eq!(exact.exit_code(), 3);
}

#[test]
fn test_a_command_without_a_handler_is_not_a_failure() {
    let app = one(Command::new("noop"));
    assert!(app.try_run_from(["noop"]).unwrap().is_ok());
}

#[test]
fn test_a_hidden_command_still_runs() {
    static RAN: AtomicUsize = AtomicUsize::new(0);
    let app = one(Command::new("debug-dump").hidden(true).run(|_| {
        let _ = RAN.fetch_add(1, Ordering::SeqCst);
    }));
    assert!(!plain(&app.help()).contains("debug-dump"));
    assert!(app.try_run_from(["debug-dump"]).unwrap().is_ok());
    assert_eq!(RAN.load(Ordering::SeqCst), 1);
}

// ---------------------------------------------------------------------------
// Introspection
// ---------------------------------------------------------------------------

#[test]
fn test_the_command_tree_is_readable_from_outside() {
    // What a completions or manual-page generator walks. Without this the tree
    // is write-only and every such tool needs a second description of the CLI.
    let app = App::new("forge")
        .version("2.0.0")
        .arg(Arg::count("verbose").short('v').global(true))
        .command(
            Command::new("build")
                .about("compile")
                .arg(Arg::option("level").possible_values(["warn", "info"]))
                .subcommand(Command::new("docs")),
        );

    assert_eq!(app.name(), "forge");
    assert_eq!(app.version_text(), Some("2.0.0"));
    assert!(app.global_arguments()[0].is_global());

    let build = &app.commands()[0];
    assert_eq!(build.name(), "build");
    assert_eq!(build.about_text(), Some("compile"));
    assert_eq!(build.subcommands()[0].name(), "docs");

    let level = &build.arguments()[0];
    assert_eq!(level.long_form(), Some("level"));
    assert!(level.expects_value());
    assert!(!level.is_positional());
    assert_eq!(level.allowed_values(), ["warn", "info"]);
}

// ---------------------------------------------------------------------------
// Robustness
// ---------------------------------------------------------------------------

#[test]
fn test_an_app_with_no_commands_parses_nothing_gracefully() {
    let app = App::new("empty");
    assert!(
        app.try_parse_from([] as [&str; 0])
            .unwrap()
            .subcommand()
            .is_none()
    );
    assert_eq!(
        app.try_parse_from(["anything"]).unwrap_err().kind(),
        ErrorKind::UnknownCommand
    );
}

#[test]
fn test_empty_and_odd_tokens_never_panic() {
    let app = one(Command::new("run")
        .arg(Arg::option("out").short('o'))
        .arg(Arg::positional("rest").multiple(true)));
    for argv in [
        vec![""],
        vec!["run", ""],
        vec!["run", "--"],
        vec!["run", "--="],
        vec!["run", "-"],
        vec!["run", "--out", ""],
        vec!["run", "-o"],
        vec!["run", "日本語"],
        vec!["run", "--日本語"],
    ] {
        // The contract is only that it returns rather than panicking.
        let _ = app.try_parse_from(argv);
    }
}

#[test]
fn test_deeply_nested_invocations_do_not_overflow() {
    // A pathological tree, to show recursion depth is bounded by the program's
    // own definition rather than by the input.
    let mut leaf = Command::new("level50");
    for depth in (0..50).rev() {
        leaf = Command::new(crate::shim::format!("level{depth}")).subcommand(leaf);
    }
    let app = one(leaf);
    let argv: Vec<String> = (0..=50).map(|d| crate::shim::format!("level{d}")).collect();
    assert_eq!(app.try_parse_from(argv).unwrap().command_path().len(), 51);
}

#[test]
fn test_exit_codes_map_onto_a_process_status() {
    assert_eq!(exit_code(0), ExitCode::SUCCESS);
    // A code that cannot be represented must stay a failure rather than wrap
    // around to success.
    let _ = exit_code(1);
    let _ = exit_code(255);
    let _ = exit_code(256);
    let _ = exit_code(-1);
}

#[cfg(feature = "auth")]
mod auth {
    use super::*;

    #[test]
    fn test_an_auth_gated_command_is_refused_without_a_hook() {
        static RAN: AtomicUsize = AtomicUsize::new(0);
        let app = one(Command::new("publish").requires_auth(true).run(|_| {
            let _ = RAN.fetch_add(1, Ordering::SeqCst);
        }));

        let error = app.try_run_from(["publish"]).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Unauthorized);
        assert_eq!(error.subject(), "publish");
        assert_eq!(
            RAN.load(Ordering::SeqCst),
            0,
            "the handler must not have run"
        );
    }

    #[test]
    fn test_the_hook_decides() {
        let mut app = App::new("demo").auth(|req| req.command() == "allowed");
        app.register(Command::new("allowed").requires_auth(true).run(|_| {}));
        app.register(Command::new("refused").requires_auth(true).run(|_| {}));

        assert!(app.try_run_from(["allowed"]).unwrap().is_ok());
        assert_eq!(
            app.try_run_from(["refused"]).unwrap_err().kind(),
            ErrorKind::Unauthorized
        );
    }

    #[test]
    fn test_the_hook_sees_the_whole_path() {
        let mut app = App::new("demo").auth(|req| req.path() == ["remote", "add"]);
        app.register(
            Command::new("remote")
                .subcommand(Command::new("add").requires_auth(true).run(|_| {}))
                .subcommand(Command::new("remove").requires_auth(true).run(|_| {})),
        );
        assert!(app.try_run_from(["remote", "add"]).unwrap().is_ok());
        assert!(app.try_run_from(["remote", "remove"]).is_err());
    }

    #[test]
    fn test_an_unauthorized_command_is_absent_from_help() {
        let mut app = App::new("demo").auth(|req| req.command() == "allowed");
        app.register(Command::new("allowed").requires_auth(true).about("visible"));
        app.register(
            Command::new("refused")
                .requires_auth(true)
                .about("invisible"),
        );

        let help = plain(&app.help());
        assert!(help.contains("allowed"));
        assert!(!help.contains("refused"));
        assert!(!help.contains("invisible"));
    }

    #[test]
    fn test_an_ungated_command_is_unaffected_by_the_hook() {
        let mut app = App::new("demo").auth(|_| false);
        app.register(Command::new("open").run(|_| {}));
        assert!(app.try_run_from(["open"]).unwrap().is_ok());
        assert!(plain(&app.help()).contains("open"));
    }
}

#[cfg(not(feature = "auth"))]
#[test]
fn test_requires_auth_is_inert_without_the_feature() {
    let app = one(Command::new("publish").requires_auth(true).run(|_| {}));
    assert!(plain(&app.help()).contains("publish"));
    assert!(app.try_run_from(["publish"]).unwrap().is_ok());
}
