<h1 align="center">
    <img width="90px" height="auto" src="https://raw.githubusercontent.com/jamesgober/jamesgober/main/media/icons/hexagon-3.svg" alt="Triple Hexagon">
    <br><b>cli-forge · Commands &amp; Arguments</b>
</h1>

<p align="center">
  <code>v2.1.0</code> &mdash; defining commands, parsing input, validating values, reporting failure.
</p>

> If you have not read the [Guide](./GUIDE.md), start there. For a specific
> answer rather than an explanation, try [Recipes](./RECIPES.md).

<hr>
<br>

## Contents

- [The shape](#the-shape)
- [Commands](#commands)
  - [Subcommands](#subcommands)
  - [Registering from anywhere](#registering-from-anywhere)
- [Arguments](#arguments)
  - [The four kinds of argument](#the-four-kinds-of-argument)
  - [Every form a user can type](#every-form-a-user-can-type)
  - [Reading values back](#reading-values-back)
  - [Typed values](#typed-values)
  - [Validating values](#validating-values)
  - [Defaults and environment fallbacks](#defaults-and-environment-fallbacks)
  - [Where a value came from](#where-a-value-came-from)
  - [Global arguments](#global-arguments)
  - [Relationships between arguments](#relationships-between-arguments)
  - [Groups of arguments](#groups-of-arguments)
  - [Many values](#many-values)
  - [Lists in one value](#lists-in-one-value)
  - [Turning a flag off](#turning-a-flag-off)
- [Failing well](#failing-well)
- [Entry points](#entry-points)
- [Plugins: external subcommands](#plugins-external-subcommands)
- [Errors](#errors)
- [Help](#help)
- [Authorization](#authorization)
- [Testing](#testing)
- [Reading the command tree](#reading-the-command-tree)
- [Common mistakes](#common-mistakes)

---

## The shape

```text
App        the program. Holds commands and app-level arguments.
 └─ Command    one thing the program can do. Holds arguments and a handler.
     └─ Arg        one input a command accepts.
         └─ Matches    what the user typed, handed to the handler.
```

Written out:

```rust
use std::process::ExitCode;

use cli_forge::{out, App, Arg, Command};

fn main() -> ExitCode {
    let mut app = App::new("forge");                     // the program

    app.register(
        Command::new("build")                            // one thing it does
            .about("compile the project")
            .arg(Arg::flag("release").short('r'))        // one input
            .run(|m| {                                   // what to do
                out(format!("release={}", m.flag("release")));
            }),
    );

    app.run()
}
```

---

## Commands

```rust
use cli_forge::Command;

let _cmd = Command::new("remove")
    .aliases(["rm", "del"])                 // also invokable as rm / del
    .about("delete build artifacts")        // one line, shown in the listing
    .long_about("Removes ./target and any cached downloads. Never \
                 touches source files.")    // the command's own page
    .display_order(3)                       // where it sits in the listing
    .hidden(false);                         // true = works, but not advertised
```

| Method | Does |
|---|---|
| `Command::new(name)` | The invocation name. |
| `.alias(name)` / `.aliases([..])` | Extra names. They resolve to the canonical one. |
| `.about(text)` | One line, shown beside the name in the parent's listing. |
| `.long_about(text)` | Paragraphs, shown on this command's own help page. |
| `.before_help(text)` / `.after_help(text)` | Text above / below everything on that page. |
| `.usage(text)` | Replace the generated usage line. |
| `.arg(arg)` / `.args([..])` | What it accepts. |
| `.subcommand(cmd)` | Nest another command. |
| `.subcommand_required(true)` | Refuse to run bare. |
| `.hidden(true)` | Keep it invokable but out of help. |
| `.display_order(n)` | Sort position in a listing. |
| `.requires_auth(true)` | Gate it behind the auth hook. |
| `.run(f)` / `.run_status(f)` | The handler. |

An alias resolves to the canonical name, so your handler never has to know which
spelling was used:

```rust
use cli_forge::{App, Command};

let mut app = App::new("demo");
app.register(Command::new("remove").aliases(["rm", "del"]));

for spelling in ["remove", "rm", "del"] {
    let m = app.try_parse_from([spelling]).unwrap();
    assert_eq!(m.subcommand_name(), Some("remove"));
}
```

`hidden` is for a command that exists but should not be advertised — a debugging
switch, or one kept working for compatibility:

```rust
use cli_forge::{App, Command};

let mut app = App::new("demo");
app.register(Command::new("debug-dump").hidden(true).run(|_| {}));

assert!(!app.help().contains("debug-dump"));          // not advertised
assert!(app.try_run_from(["debug-dump"]).is_ok());    // still works
```

`display_order` lets the listing follow importance rather than source order:

```rust
use cli_forge::{App, Command};

let mut app = App::new("forge");
app.register(Command::new("clean").display_order(10));
app.register(Command::new("build").display_order(1));

let help = app.help();
assert!(help.find("build").unwrap() < help.find("clean").unwrap());
```

Without it, commands list in registration order.

### Subcommands

Nest to any depth:

```rust
use cli_forge::{App, Arg, Command};

let mut app = App::new("forge");
app.register(
    Command::new("remote")
        .about("manage remotes")
        .subcommand_required(true)                 // `forge remote` alone is a mistake
        .subcommand(
            Command::new("add")
                .arg(Arg::positional("name").required(true))
                .arg(Arg::positional("url").required(true))
                .run(|m| {
                    let _ = (m.value("name"), m.value("url"));
                }),
        )
        .subcommand(Command::new("list").run(|_| {})),
);

assert!(app.try_run_from(["remote", "add", "origin", "https://example.com"]).is_ok());
```

`subcommand_required(true)` is right for a command that is only a grouping —
`forge remote` on its own is a mistake, not a no-op:

```console
$ forge remote
error: 'remote' needs a subcommand

  expected one of: add, list

USAGE: forge remote [options] <command>
```

Without it, `forge remote` runs `remote`'s own handler, if it has one.

The handler reaches the deepest command the user named, and receives **that
level's** `Matches`:

```rust
use cli_forge::{App, Arg, Command};

let mut app = App::new("forge");
app.register(
    Command::new("remote").subcommand(
        Command::new("add").arg(Arg::positional("url")).run(|m| {
            // `m` is `add`'s level, so `url` is right here.
            assert_eq!(m.value("url"), Some("https://example.com"));
        }),
    ),
);
let _ = app.try_run_from(["remote", "add", "https://example.com"]);
```

### Registering from anywhere

A command does not have to be built in `main`. This is the property that lets
each command live beside the code it drives:

```rust
// src/commands/build.rs
use cli_forge::{out, App, Arg, Command};

pub fn install(app: &mut App) {
    app.register(
        Command::new("build")
            .about("compile the project")
            .arg(Arg::flag("release").short('r'))
            .run(|_| out("building...")),
    );
}
```

```rust
// src/main.rs
use std::process::ExitCode;

use cli_forge::App;

// In a real program these are `mod commands;` and separate files; written
// inline here so the example compiles as one piece.
mod commands {
    pub mod build {
        use cli_forge::{out, App, Arg, Command};

        pub fn install(app: &mut App) {
            app.register(
                Command::new("build")
                    .about("compile the project")
                    .arg(Arg::flag("release").short('r'))
                    .run(|_| out("building...")),
            );
        }
    }

    pub mod test {
        use cli_forge::{App, Command};

        pub fn install(app: &mut App) {
            app.register(Command::new("test").about("run the tests").run(|_| {}));
        }
    }
}

fn main() -> ExitCode {
    let mut app = App::new("forge");

    commands::build::install(&mut app);
    commands::test::install(&mut app);

    app.run()
}
```

A command registered this way is reachable and behaves identically to one
registered inline. It also works from a loop over a config file, or from a
plugin's setup function.

---

## Arguments

### The four kinds of argument

```rust
use cli_forge::{Arg, Command};

let _cmd = Command::new("build")
    // 1. A switch. Present or absent.
    //    forge build --release      forge build -r
    .arg(Arg::flag("release").short('r'))

    // 2. A switch you can repeat. Counted.
    //    forge build -v    -vv    -vvv    -v -v -v
    .arg(Arg::count("verbose").short('v'))

    // 3. A named value.
    //    forge build --jobs 8    --jobs=8    -j 8    -j8
    .arg(Arg::option("jobs").short('j'))

    // 4. A bare value, identified by position.
    //    forge build server
    .arg(Arg::positional("target"));
```

That is the whole list. Everything else is a refinement of one of those four.

### Every form a user can type

All of this already works. You do not opt in:

```console
$ forge build --jobs 8          # long, separate value
$ forge build --jobs=8          # long, attached value
$ forge build -j 8              # short, separate value
$ forge build -j8               # short, attached value
$ forge build -j=8              # short with an equals (people write this)
$ forge build -r                # a short flag
$ forge build -rv               # bundled flags
$ forge build -rvvv             # bundled, with a repeat
$ forge build -rj8              # bundled, ending in an option and its value
$ forge build a.rs b.rs         # positionals
$ forge build -- --not-a-flag   # everything after -- is a value
```

And two cases most parsers get wrong:

```console
$ calc add -5 3                 # a negative number is a value, not a flag
$ cat read -                    # a lone dash is a value (read stdin)
```

A leading `-` only means "flag" when it could: `-5` is a value unless your
command actually declares a short `-5`, in which case you get it back.

### Reading values back

Your handler receives a `Matches`:

```rust
use cli_forge::{Arg, Command};

let _cmd = Command::new("build")
    .arg(Arg::flag("release"))
    .arg(Arg::count("verbose"))
    .arg(Arg::option("jobs").default("1"))
    .arg(Arg::positional("targets").multiple(true))
    .run(|m| {
        let release: bool = m.flag("release");
        let verbosity: usize = m.count("verbose");
        let jobs: Option<&str> = m.value("jobs");
        let targets: Vec<&str> = m.values("targets").collect();
        let _ = (release, verbosity, jobs, targets);
    });
```

| Method | Returns | For |
|---|---|---|
| `.flag(name)` | `bool` | A flag, or a count that reached one. |
| `.count(name)` | `usize` | A counting flag. `0` if absent. |
| `.value(name)` | `Option<&str>` | One value, or its fallback. |
| `.values(name)` | iterator of `&str` | Every value. Empty if absent. |
| `.get::<T>(name)` | `Option<T>` | One value, parsed. |
| `.get_all::<T>(name)` | `Vec<T>` | Every value, parsed. |
| `.try_get::<T>(name)` | `Result<Option<T>, ParseError>` | One value, parsed, with the reason it would not. |
| `.present(name)` | `bool` | Whether it has a value at all. |
| `.source(name)` | `Option<ValueSource>` | Where it came from. |
| `.subcommand()` | `Option<(&str, &Matches)>` | The invoked subcommand. |
| `.subcommand_name()` | `Option<&str>` | Just its name. |
| `.command_path()` | `Vec<&str>` | The whole chain, outermost first. |
| `.leaf()` | `&Matches` | The deepest level — the one that will run. |

**An unknown name is never an error.** `m.flag("typo")` is `false`,
`m.value("typo")` is `None`, `m.count("typo")` is `0`. Nothing panics.

`leaf()` saves walking the chain when you dispatch yourself:

```rust
use cli_forge::{App, Arg, Command};

let mut app = App::new("forge");
app.register(
    Command::new("remote").subcommand(Command::new("add").arg(Arg::positional("url"))),
);

let m = app.try_parse_from(["remote", "add", "https://example.com"]).unwrap();
assert_eq!(m.command_path(), ["remote", "add"]);
assert_eq!(m.leaf().value("url"), Some("https://example.com"));
```

### Typed values

```rust
use cli_forge::{App, Arg, Command};

let mut app = App::new("serve");
app.register(
    Command::new("start")
        .arg(Arg::option("port").default("8080"))
        .arg(Arg::option("ratio").default("0.25")),
);

let m = app.try_parse_from(["start"]).unwrap();
let start = m.leaf();

assert_eq!(start.get::<u16>("port"), Some(8080));
assert_eq!(start.get::<f64>("ratio"), Some(0.25));
assert_eq!(start.get::<u16>("absent"), None);
```

`get::<T>` works for anything implementing `FromStr`, including your own types.
It returns `None` for an absent value *or* one that will not parse — which is
safe **because you validated it at parse time**. If you did not, use `try_get`,
which tells you why:

```rust
use cli_forge::{App, Arg, Command, ErrorKind};

let mut app = App::new("demo");
app.register(Command::new("wait").arg(Arg::option("seconds")));

let m = app.try_parse_from(["wait", "--seconds", "soon"]).unwrap();
let err = m.leaf().try_get::<u32>("seconds").unwrap_err();

assert_eq!(err.kind(), ErrorKind::InvalidValue);
assert_eq!(err.subject(), "soon");
```

### Validating values

**This is the highest-value habit in the crate.** Say what a value may be, and
the parser refuses a bad one — with a proper message — before your code runs.

Two ways. A fixed set:

```rust
use cli_forge::{App, Arg, Command};

let mut app = App::new("forge");
app.register(
    Command::new("log").arg(Arg::option("level").possible_values(["warn", "info", "debug"])),
);

assert!(app.try_parse_from(["log", "--level", "info"]).is_ok());
let err = app.try_parse_from(["log", "--level", "inof"]).unwrap_err();
assert_eq!(err.suggestion(), Some("info"));
```

```console
$ forge log --level inof
error: invalid value 'inof'

  '--level' accepts one of: warn, info, debug

  did you mean 'info'?

USAGE: forge log [options]
```

Or an arbitrary check:

```rust
use cli_forge::{App, Arg, Command};

let mut app = App::new("serve");
app.register(
    Command::new("start").arg(Arg::option("port").validate(|value| {
        match value.parse::<u16>() {
            Ok(port) if port >= 1024 => Ok(()),
            Ok(_) => Err("ports below 1024 need privileges".to_string()),
            Err(_) => Err("expected a number from 1024 to 65535".to_string()),
        }
    })),
);

assert!(app.try_parse_from(["start", "--port", "8080"]).is_ok());
let err = app.try_parse_from(["start", "--port", "80"]).unwrap_err();
assert!(err.report().contains("need privileges"));
```

The string you return becomes the error's detail, so write it as the sentence the
user should read.

Compare the alternative. Without validation you end up with this, three functions
deep, in a place where you can no longer produce a good message:

```rust,should_panic
// Don't. This example really does panic — that is the point.
fn main() {
    let raw = "70000";
    let _port: u16 = raw.parse().unwrap();    // panics, with no context
}
```

Validation also applies to positionals, and runs **before any handler**:

```rust
use cli_forge::{App, Arg, Command};

let mut app = App::new("demo");
app.register(
    Command::new("log")
        .arg(Arg::positional("level").possible_values(["warn", "info"]))
        .run(|_| -> () { panic!("must not run") }),
);
assert!(app.try_run_from(["log", "loud"]).is_err());     // the handler never ran
```

### Defaults and environment fallbacks

Precedence is **command line, then environment, then default**:

```rust
use cli_forge::{Arg, Command};

let _cmd = Command::new("push")
    .arg(
        Arg::option("token")
            .env("FORGE_TOKEN")          // checked if not on the command line
            .help("credentials for the registry"),
    )
    .arg(Arg::option("jobs").default("1"));
```

An environment variable set to the empty string counts as unset, which is what
lets a shell clear an inherited value with `VAR=`.

For a flag or a counting flag, any value other than empty, `0`, `false`, `no`, or
`off` counts as set:

```console
$ FORGE_VERBOSE=1 forge build       # the flag is set
$ FORGE_VERBOSE=off forge build     # it is not
```

> **`env` is how a secret stays out of the process list.** `--token abc123` is
> visible to every user on the machine via `ps`; `FORGE_TOKEN` is not. Prefer it
> for credentials, and consider `.hide(true)` so the flag is not advertised.

A default also satisfies `required`, because the argument always has a value:

```rust
use cli_forge::{App, Arg, Command};

let mut app = App::new("demo");
app.register(Command::new("go").arg(Arg::positional("path").required(true).default(".")));

assert_eq!(app.try_parse_from(["go"]).unwrap().leaf().value("path"), Some("."));
```

### Where a value came from

The distinction a sentinel default cannot express: did the user *choose* this, or
did we only fall back?

```rust
use cli_forge::{App, Arg, Command, ValueSource};

let mut app = App::new("demo");
app.register(Command::new("build").arg(Arg::option("jobs").default("1")));

let defaulted = app.try_parse_from(["build"]).unwrap();
assert_eq!(defaulted.leaf().source("jobs"), Some(ValueSource::Default));

let chosen = app.try_parse_from(["build", "--jobs", "8"]).unwrap();
assert_eq!(chosen.leaf().source("jobs"), Some(ValueSource::CommandLine));
```

This is what you want when merging with a config file: overwrite the file's
setting only when the user actually asked.

```rust
use cli_forge::{Matches, ValueSource};

fn resolve_jobs(m: &Matches, from_config: u16) -> u16 {
    match m.source("jobs") {
        // The user asked. Their choice wins.
        Some(ValueSource::CommandLine) | Some(ValueSource::Environment) => {
            m.get("jobs").unwrap_or(from_config)
        }
        // Nobody asked; the config file is more specific than our default.
        _ => from_config,
    }
}
```

### Global arguments

An argument the whole program accepts, on either side of the command name:

```rust
use cli_forge::{App, Arg, Command};

let mut app = App::new("forge")
    .arg(Arg::count("verbose").short('v').global(true))
    .arg(Arg::option("config").short('c').global(true));

app.register(Command::new("build").subcommand(Command::new("docs")));

// Before the command, after it, or both — all the same.
for argv in [vec!["-vv", "build"], vec!["build", "-vv"], vec!["-v", "build", "-v"]] {
    let m = app.try_parse_from(argv).unwrap();
    assert_eq!(m.count("verbose"), 2);
}

// And the value is visible at every level, so a handler never walks back up.
let deep = app.try_parse_from(["-v", "build", "docs"]).unwrap();
assert_eq!(deep.leaf().count("verbose"), 1);
```

`App::arg` without `.global(true)` makes an argument the **app level only**
accepts — before any command name:

```rust
use cli_forge::{App, Arg, Command, ErrorKind};

let mut app = App::new("forge").arg(Arg::flag("app-only"));
app.register(Command::new("build"));

assert!(app.try_parse_from(["--app-only", "build"]).is_ok());
let err = app.try_parse_from(["build", "--app-only"]).unwrap_err();
assert_eq!(err.kind(), ErrorKind::UnknownFlag);
```

Global arguments also appear in each command's help listing, so a user reading
that page knows the command accepts them.

### Relationships between arguments

```rust
use cli_forge::{App, Arg, Command, ErrorKind};

let mut app = App::new("forge");
app.register(
    Command::new("push")
        // Cannot be used together.
        .arg(Arg::flag("quiet").conflicts_with(["verbose"]))
        .arg(Arg::flag("verbose"))
        // Needs another argument.
        .arg(Arg::flag("sign").requires(["key"]))
        .arg(Arg::option("key"))
        // Required, unless something else was given.
        .arg(Arg::positional("file").required(true).required_unless(["stdin"]))
        .arg(Arg::flag("stdin")),
);

// Conflicts.
assert_eq!(
    app.try_parse_from(["push", "--quiet", "--verbose", "f"]).unwrap_err().kind(),
    ErrorKind::Conflict
);
// Dependencies.
assert_eq!(
    app.try_parse_from(["push", "--sign", "f"]).unwrap_err().kind(),
    ErrorKind::MissingDependency
);
// Either form of input.
assert!(app.try_parse_from(["push", "notes.txt"]).is_ok());
assert!(app.try_parse_from(["push", "--stdin"]).is_ok());
assert!(app.try_parse_from(["push"]).is_err());
```

A **default never triggers a conflict** — only an argument the user actually
supplied can. Otherwise an argument with a default could not conflict with
anything.

### Groups of arguments

`conflicts_with` and `requires` describe **pairs**. Some rules are about a
**set** — "exactly one output format" over three formats would be six conflict
declarations and still say nothing about requiring one. An `ArgGroup` names the
set once:

```rust
use cli_forge::{App, Arg, ArgGroup, Command, ErrorKind};

let mut app = App::new("export");
app.register(
    Command::new("dump")
        .arg(Arg::flag("json"))
        .arg(Arg::flag("yaml"))
        .arg(Arg::flag("toml"))
        .group(ArgGroup::new("format").args(["json", "yaml", "toml"]).required(true)),
);

// None: refused.
assert_eq!(app.try_parse_from(["dump"]).unwrap_err().kind(), ErrorKind::MissingRequired);
// Two: refused.
assert_eq!(
    app.try_parse_from(["dump", "--json", "--yaml"]).unwrap_err().kind(),
    ErrorKind::Conflict
);
// One: fine, and you can ask which.
let m = app.try_parse_from(["dump", "--yaml"]).unwrap();
assert_eq!(m.leaf().group("format"), Some("yaml"));
```

```console
$ export dump
error: one of --json, --yaml, --toml is required

USAGE: export dump [options] <--json|--yaml|--toml>

$ export dump --json --yaml
error: '--json' and '--yaml' cannot be used together

  choose one of: --json, --yaml, --toml

USAGE: export dump [options] <--json|--yaml|--toml>
```

Note the usage line: a required group appears there, so the user sees the choice
before they get it wrong.

The rule is set by two switches:

| `required` | `multiple` | Means |
|---|---|---|
| no | no | at most one *(default)* |
| yes | no | exactly one |
| yes | yes | at least one |
| no | yes | any number — the group only names the set |

`m.group("format")` answers which member was chosen, so dispatching on the
choice is one `match`:

```rust
use cli_forge::{App, Arg, ArgGroup, Command};

let mut app = App::new("export");
app.register(
    Command::new("dump")
        .args([Arg::flag("json"), Arg::flag("yaml")])
        .group(ArgGroup::new("format").args(["json", "yaml"]).required(true)),
);

let m = app.try_parse_from(["dump", "--json"]).unwrap();
let extension = match m.leaf().group("format") {
    Some("json") => "json",
    _ => "yml",
};
assert_eq!(extension, "json");
```

**What counts as given.** For the at-most-one rule, a member counts when the
user supplied it — on the command line or through its environment variable —
and did not turn it off with `--no-NAME`. A **default does not count**, or a
group whose members have defaults could never be satisfied without a conflict.
For the at-least-one rule, any value counts, defaults included, because a
default is an answer:

```rust
use cli_forge::{App, Arg, ArgGroup, Command};

let mut app = App::new("serve");
app.register(
    Command::new("start")
        .arg(Arg::option("port").default("8080"))
        .arg(Arg::option("socket"))
        .group(ArgGroup::new("listen").args(["port", "socket"]).required(true)),
);

// The default answers the required group...
assert_eq!(app.try_parse_from(["start"]).unwrap().leaf().group("listen"), Some("port"));
// ...and does not count against the user's own choice.
assert_eq!(
    app.try_parse_from(["start", "--socket", "/tmp/s"]).unwrap().leaf().group("listen"),
    Some("socket")
);
```

Positionals can be members too, which is how "a file, or `--stdin`" is said as a
group rather than with `required_unless`.

### Many values

```rust
use cli_forge::{App, Arg, Command};

let mut app = App::new("cc");
app.register(
    Command::new("build")
        // Repeatable: -I a -I b
        .arg(Arg::option("include").short('I').multiple(true))
        // Variadic: absorbs every remaining bare value. Put it last.
        .arg(Arg::positional("sources").multiple(true)),
);

let m = app.try_parse_from(["build", "-I", "a", "-I", "b", "x.c", "y.c"]).unwrap();
let build = m.leaf();
assert_eq!(build.values("include").collect::<Vec<_>>(), ["a", "b"]);
assert_eq!(build.values("sources").collect::<Vec<_>>(), ["x.c", "y.c"]);
```

Without `multiple(true)`, an option is **last wins**:

```rust
use cli_forge::{App, Arg, Command};

let mut app = App::new("demo");
app.register(Command::new("build").arg(Arg::option("jobs")));

let m = app.try_parse_from(["build", "--jobs", "1", "--jobs", "8"]).unwrap();
assert_eq!(m.leaf().value("jobs"), Some("8"));
```

---

### Lists in one value

Repeating a flag works (`-I a -I b`), but most tools also accept a list in one
go: `--features serde,tokio`. `value_delimiter` does that:

```rust
use cli_forge::{App, Arg, Command};

let mut app = App::new("cargo");
app.register(Command::new("build").arg(Arg::option("features").short('F').value_delimiter(',')));

let m = app.try_parse_from(["build", "--features", "serde,tokio", "-F", "log"]).unwrap();
assert_eq!(m.leaf().values("features").collect::<Vec<_>>(), ["serde", "tokio", "log"]);
```

Both styles mix freely, and every attached form splits too (`--features=a,b`,
`-Fa,b`, `-F=a,b`).

Three details:

- **Each piece is validated on its own.** `possible_values` sees `serde` and
  `tokio`, not `serde,tokio` — and a typo in one piece gets its own
  "did you mean". Every piece is checked before any is stored, so a bad one
  leaves no partial list behind.
- **Empty pieces are kept.** `a,,b` is three values, the middle one empty.
  Dropping it silently would hide a typo that a validator could catch.
- **Defaults and environment values split the same way**, so
  `.default("linux,macos")` is two values.

`value_delimiter` implies `multiple`. Help shows `[delimiter: ',']`.

### Turning a flag off

A setting with a default sometimes needs overriding in **either** direction —
`--color` and `--no-color`, `--cache` and `--no-cache`. `negatable` adds the
second spelling:

```rust
use cli_forge::{App, Arg, Command};

let mut app = App::new("forge");
app.register(Command::new("build").arg(Arg::flag("cache").negatable(true)));

let on = app.try_parse_from(["build", "--cache"]).unwrap();
let off = app.try_parse_from(["build", "--no-cache"]).unwrap();
let unsaid = app.try_parse_from(["build"]).unwrap();

assert_eq!(on.leaf().explicit_flag("cache"), Some(true));
assert_eq!(off.leaf().explicit_flag("cache"), Some(false));
assert_eq!(unsaid.leaf().explicit_flag("cache"), None);
```

**Why `explicit_flag` and not `flag`?** `flag` answers `false` for both "turned
off" and "never mentioned", which is usually what you want. When a config file
has its own opinion, those two mean different things — only an explicit choice
should override the file:

```rust
use cli_forge::Matches;

fn use_cache(m: &Matches, from_config: bool) -> bool {
    m.explicit_flag("cache").unwrap_or(from_config)
}
```

The rules:

- **The last spelling wins**, so `--no-cache --cache` leaves it on. That is what
  lets a shell alias set a default the user can still flip.
- **An explicit off never takes part in a conflict.** If `quiet` conflicts with
  `verbose`, `--quiet --no-verbose` is accepted — the user said the opposite of
  verbose.
- **An explicit off does not satisfy a `requires`** or answer a group; it holds
  no value.
- **For an environment variable**, `0`, `false`, `no`, or `off` is an explicit
  off on a negatable flag (on an ordinary flag it just means unset). The command
  line still outranks it in both directions.
- **An argument really named `no-cache` wins** over negating `cache`.

Help shows one entry: `--[no-]cache`.

## Failing well

A handler may return nothing, or a `Result`:

```rust
use cli_forge::{out, App, Command};

let mut app = App::new("forge");

// Nothing to report.
app.register(Command::new("ping").run(|_| out("pong")));

// A message. Exits 1.
app.register(Command::new("pull").run(|_| Err("not a repository")));

// `?` against any error type you already use.
app.register(Command::new("read").run(|_| -> std::io::Result<()> {
    out(std::fs::read_to_string("forge.toml")?);
    Ok(())
}));
```

`run` accepts any `Result<(), E>` where `E` can be printed — `io::Error`,
`Box<dyn Error>`, `anyhow::Error`, `String`, `&str`, your own types. The message
is reported through the theme's error level, and the process exits `1`.

For a **specific** exit code — some tools treat `1` and `2` as different answers,
like `diff` and `grep` — use `run_status`:

```rust
use cli_forge::{App, Command, CommandError};

let mut app = App::new("diffy");
app.register(Command::new("diff").run_status(|_| {
    Err(CommandError::new("files differ").with_code(2))
}));

let failure = app.try_run_from(["diff"]).unwrap().unwrap_err();
assert_eq!(failure.exit_code(), 2);
```

> **Why two methods?** `run` is generic over any printable error, which is what
> makes `?` work. That generality means it cannot also read an exit code off the
> error, so it reports every failure as `1`. `run_status` names the type exactly,
> so the code survives. Use `run` unless you need a specific status.

### Exit codes

| Outcome | Status |
|---|---|
| Success | `0` |
| `--help` or `--version` | `0`, printed to **stdout** |
| A bad command line | `2`, printed to **stderr** |
| A failed handler | `1`, or whatever `run_status` chose |

Help exiting `0` on stdout matters: it is what lets `yourtool --help | less`
work, and what keeps `yourtool --help && echo ok` from looking like a failure.

---

## Entry points

Four, because programs genuinely want different things:

| Method | Parses | Runs handlers | Prints | Exits |
|---|---|---|---|---|
| `run()` | process args | yes | yes | returns an `ExitCode` |
| `parse()` | process args | yes | yes | yes |
| `try_run_from(args)` | given args | yes | no | no |
| `try_parse_from(args)` | given args | **no** | no | no |

**Use `run()`** in `main`:

```rust
use std::process::ExitCode;

use cli_forge::{out, App, Command};

fn main() -> ExitCode {
    let mut app = App::new("forge");
    app.register(Command::new("build").run(|_| out("building...")));
    app.run()
}
```

**Use `try_parse_from`** in tests. It parses and nothing else: no output, no
handlers, no exit.

**Use `dispatch`** when you want to inspect or adjust the `Matches` in between,
or to decide not to run at all:

```rust
use cli_forge::{App, Command};

let mut app = App::new("forge");
app.register(Command::new("build").run(|_| {}));

let matches = app.try_parse_from(["build"]).unwrap();
// ...look at `matches`, maybe decide not to proceed...
assert!(app.dispatch(&matches).is_ok());
```

`parse()` exists for the shape where you want the `Matches` back and are happy
for the program to exit on a bad command line. Prefer `run()`, which leaves the
exit decision to `main`.

> **If you are coming from 1.x:** `try_parse_from` used to run handlers as a side
> effect of parsing. It no longer does. Use `try_run_from` where you relied on
> that. This is the one silent behaviour change in 2.0.

---

## Plugins: external subcommands

`cargo watch` is not part of cargo. It is a separate `cargo-watch` program that
cargo finds and runs, passing along everything after the command name. That is
how a tool grows plugins without knowing about them in advance — and how a suite
can reach tools installed independently of it.

`App::external` hands any command name the app does not define to a hook:

```rust
use cli_forge::{App, Command, External};

let app = App::new("forge")
    .external(|ext: &External<'_>| {
        let program = format!("forge-{}", ext.name());
        match std::process::Command::new(&program).args(ext.args()).status() {
            Ok(status) if status.success() => Ok(()),
            Ok(status) => Err(format!("{program} exited with {status}")),
            // No such program: say what they probably meant, as an unknown
            // command would have without the hook.
            Err(_) => Err(match ext.suggestion() {
                Some(nearest) => format!("unknown command '{}'; did you mean '{nearest}'?", ext.name()),
                None => format!("unknown command '{}'", ext.name()),
            }),
        }
    })
    .command(Command::new("build"));
# let _ = app;
```

What the hook receives:

| `External::` | Is |
|---|---|
| `name()` | The command name the user typed. |
| `args()` | Every token after it, **unparsed** — flags included, because only the plugin knows what they mean. |
| `matches()` | The app-level arguments parsed *before* the name, so a plugin can inherit `-vv` or `--color`. |
| `suggestion()` | The nearest registered command, for the hook to fall back on. |

The hook returns anything a handler can, so a failure becomes the exit status.

The boundaries:

- **Registered commands never reach the hook**, aliases included.
- **A flag is never handed off.** `forge --bogus` is still an unknown-flag
  error; only a bare name reaches the hook.
- **`forge help` and a bare `forge` keep their meaning.**
- **Parsing still has no side effects.** `try_parse_from` records the hand-off as
  `Matches::external()` without calling the hook; `dispatch`, `run`, and
  `try_run_from` call it.
- **A typo of a built-in reaches the hook too**, because only the hook knows
  whether `forge-buidl` exists. That is what `suggestion()` is for, and
  `App::suggest(name)` gives the same answer anywhere else.

## Errors

Nothing panics and nothing prints until you ask. A `ParseError` is a value:

```rust
use cli_forge::{App, Arg, Command, ErrorKind, Stream};

let mut app = App::new("forge");
app.register(
    Command::new("build")
        .arg(Arg::positional("targets").multiple(true))
        .arg(Arg::flag("release").short('r')),
);

let err = app.try_parse_from(["build", "--releaze"]).unwrap_err();

assert_eq!(err.kind(), ErrorKind::UnknownFlag);
assert_eq!(err.subject(), "--releaze");
assert_eq!(err.suggestion(), Some("--release"));
assert_eq!(err.exit_code(), 2);
assert_eq!(err.stream(), Stream::Stderr);
assert!(err.usage().unwrap().starts_with("USAGE: forge build"));
```

`report()` gives the whole thing, which is what `run()` prints:

```text
error: unknown flag '--releaze'

  did you mean '--release'?

USAGE: forge build [options] [targets]...
```

| Accessor | Gives |
|---|---|
| `.kind()` | An `ErrorKind`. Match on this. |
| `.subject()` | What it is about: the flag as typed, the argument name, the bad value. |
| `.detail()` | What would have been acceptable. |
| `.suggestion()` | The nearest valid spelling, if one is close. |
| `.usage()` | The usage line of the command being invoked. |
| `.text()` | For help and version: the rendered text. |
| `.is_request()` | Whether this is help/version rather than a mistake. |
| `.exit_code()` / `.stream()` | Where it goes and what status it means. |
| `.report()` / `.styled_report()` | The whole thing, plain or themed. |

### Help and version arrive here too

They come back through the same channel, because parsing stops either way — but
they are **successes**:

```rust
use cli_forge::{App, Command, ErrorKind, Stream};

let mut app = App::new("forge");
app.register(Command::new("build"));

let help = app.try_parse_from(["--help"]).unwrap_err();
assert_eq!(help.kind(), ErrorKind::HelpRequested);
assert!(help.is_request());
assert_eq!(help.exit_code(), 0);
assert_eq!(help.stream(), Stream::Stdout);
assert!(help.text().unwrap().contains("build"));
```

If you handle errors yourself, check `is_request()` before treating one as a
failure — otherwise your program exits non-zero for `--help`, which breaks every
script that checks the status.

### What to match on

`ErrorKind` is stable; the wording is not. Match on the kind and read
`subject()`:

```rust
use cli_forge::{App, Command, ErrorKind};

let app = App::new("forge");
let err = app.try_parse_from(["nope"]).unwrap_err();

match err.kind() {
    ErrorKind::UnknownCommand => { /* ... */ }
    ErrorKind::HelpRequested | ErrorKind::VersionRequested => { /* not a failure */ }
    _ => { /* ... */ }
}
```

`ErrorKind` is `#[non_exhaustive]`, so always include a `_` arm.

The kinds: `UnknownFlag`, `MissingValue`, `MissingRequired`, `UnknownCommand`,
`UnexpectedArgument`, `InvalidValue`, `Conflict`, `MissingDependency`,
`MissingSubcommand`, `NonUtf8`, `Unauthorized`, `HelpRequested`,
`VersionRequested`.

---

## Help

Help is generated, wrapped to the terminal, and aligned in display columns. You
mostly do not touch it — you just write good `help` and `about` text.

```rust
use cli_forge::{App, Arg, Command};

let mut app = App::new("forge")
    .version(env!("CARGO_PKG_VERSION"))
    .about("builds projects described by a forge.toml")
    .long_about("Resolves dependencies, compiles every target the manifest \
                 lists, and caches the result under ./target.")
    .help_header("forge — a project constructor")
    .help_footer("docs: https://docs.rs/cli-forge");

app.register(
    Command::new("build")
        .about("compile the project")
        .arg(Arg::option("jobs").short('j').default("1").value_name("N")
            .help("compilation units to run at once"))
        .arg(Arg::option("level").possible_values(["warn", "info"]))
        .arg(Arg::option("token").env("FORGE_TOKEN"))
        .after_help("EXAMPLES:\n  forge build --release"),
);

let help = app.command_help(["build"]).unwrap();
assert!(help.contains("-j, --jobs <N>"));
assert!(help.contains("[default: 1]"));
assert!(help.contains("[possible: warn, info]"));
assert!(help.contains("[env: FORGE_TOKEN]"));
assert!(help.contains("EXAMPLES:"));
```

Notice what is stated for you: the default, the allowed values, and the
environment variable. Those are the parts a reader most often came to find out,
and you did not have to repeat them in the `help` string.

What you control:

| Where | Method |
|---|---|
| Above every page | `App::help_header` |
| Below every page | `App::help_footer` |
| The app's description | `App::about`, `App::long_about` |
| A command's one-liner | `Command::about` |
| A command's own page | `Command::long_about`, `before_help`, `after_help` |
| The usage line | `Command::usage` |
| Listing order | `Command::display_order` |
| The value placeholder | `Arg::value_name` |
| Hiding things | `Command::hidden`, `Arg::hide` |

### Sections

Past a dozen commands or options, one list stops being readable. `category` puts
an item under its own heading:

```rust
use cli_forge::{App, Arg, Command};

let mut app = App::new("forge");
app.register(Command::new("build").about("compile"));
app.register(Command::new("test").about("run the tests"));
app.register(Command::new("publish").category("Release").about("upload a version"));
app.register(Command::new("yank").category("Release").about("withdraw a version"));

app.register(
    Command::new("fetch")
        .arg(Arg::option("output").short('o'))
        .arg(Arg::option("proxy").category("Network"))
        .arg(Arg::option("timeout").category("Network")),
);

let help = cli_forge::text::strip(&app.help()).into_owned();
assert!(help.contains("RELEASE:"));
```

```text
COMMANDS:
  build    compile
  test     run the tests
  fetch

RELEASE:
  publish  upload a version
  yank     withdraw a version
```

Uncategorised items come first, under the default heading. Named sections follow
in the order their first item appears — so `display_order` still decides what
comes first, and nothing gets alphabetised behind your back. Headings are
upper-cased with a colon, matching the built-in ones. `-h, --help` always stays
in the default `OPTIONS:` section, and all sections share one column width, so
descriptions stay in a single straight column down the page.

### Rendering help yourself

```rust
use cli_forge::{App, Command};

let mut app = App::new("forge");
app.register(Command::new("remote").subcommand(Command::new("add")));

let top = app.help();                                  // the app page
let nested = app.command_help(["remote", "add"]);      // any command's page
assert!(nested.unwrap().contains("forge remote add"));
assert!(app.command_help(["nope"]).is_none());
let _ = top;
```

### The automatic `help` command

`forge help`, `forge help build`, and `forge help remote add` all work. So does
running the program with no arguments, which prints the page instead of exiting
silently. Turn both off with `App::help_command(false)` — for a program whose
bare invocation is meaningful, or one that declares its own `help`.

> **Do not assert on help layout in tests.** Column widths, wrapping, and section
> order are presentation and may change in a minor release. Assert on content.

---

## Authorization

*(feature `auth`)*

Gate a command behind a hook you supply. cli-forge holds the seam; the login
state lives in your code:

```rust
#[cfg(feature = "auth")]
fn main() {
    use cli_forge::{App, Command, ErrorKind};

    let mut app = App::new("forge").auth(|req| {
        // `req.command()` is the command name; `req.path()` is the whole chain.
        req.command() != "publish" || session_is_valid()
    });

    app.register(Command::new("publish").requires_auth(true).run(|_| {}));

    let err = app.try_run_from(["publish"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Unauthorized);
}

/// Wherever your login state actually lives.
fn session_is_valid() -> bool {
    false
}

#[cfg(not(feature = "auth"))]
fn main() {}
```

Three things to know:

- **It fails closed.** An auth-gated command with no hook is never authorized.
  The alternative — a gate that silently opens when nobody wired it up — is not a
  gate.
- **An unauthorized command is absent from help**, so a user who cannot run
  something is not told it exists.
- **The hook also runs during help generation**, so keep it pure and cheap: check
  already-loaded session state, not I/O, and never print from it.

Without the `auth` feature, `requires_auth` is inert — the command runs and shows
normally.

---

## Testing

`try_parse_from` is the one to test with: it parses and nothing else.

Build the app in a helper so every test starts from the same definition, then
wrap each `fn main` body below in `#[test] fn whatever_it_checks()`:

```rust
use cli_forge::{App, Arg, Command, ErrorKind};

fn app() -> App {
    let mut app = App::new("forge");
    app.register(
        Command::new("build")
            .arg(Arg::option("jobs").default("1"))
            .arg(Arg::flag("release").short('r')),
    );
    app
}

fn main() {
    // #[test] fn parses_a_normal_invocation()
    let m = app().try_parse_from(["build", "-r", "--jobs", "8"]).unwrap();
    let build = m.leaf();
    assert!(build.flag("release"));
    assert_eq!(build.get::<u16>("jobs"), Some(8));

    // #[test] fn rejects_an_unknown_flag()
    let err = app().try_parse_from(["build", "--bogus"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::UnknownFlag);
}
```

To test a handler, use `try_run_from` and assert on its outcome:

```rust
use cli_forge::{App, Command};

fn main() {
    // #[test] fn a_failing_command_reports_its_message()
    let mut app = App::new("forge");
    app.register(Command::new("pull").run(|_| Err("not a repository")));

    let failure = app.try_run_from(["pull"]).unwrap().unwrap_err();
    assert_eq!(failure.message(), "not a repository");
    assert_eq!(failure.exit_code(), 1);
}
```

The nested `Result` is deliberate: the outer one is about the command line, the
inner one about the work. A bad invocation and a failed command deserve different
handling.

To make styled output deterministic, render at an explicit depth. This touches no
process state, so such tests are safe to run in parallel:

```rust
use cli_forge::{ColorLevel, Style};

fn main() {
    // #[test] fn renders_the_expected_bytes()
    let bytes = Style::new()
        .red()
        .bold()
        .paint_at("ERR", ColorLevel::Ansi16)
        .to_string();
    assert_eq!(bytes, "\u{1b}[1;31mERR\u{1b}[0m");
}
```

`markup_at` and `Theme::render_at` do the same for the other two paths. Where you
must assert on something that detects — a help page, for instance — strip the
escapes instead:

```rust
use cli_forge::{text, App, Command};

fn main() {
    // #[test] fn help_mentions_the_command()
    let mut app = App::new("forge");
    app.register(Command::new("build").about("compile"));

    // Section headings are styled, so strip before asserting on content.
    let help = text::strip(&app.help()).into_owned();
    assert!(help.contains("build"));
    assert!(help.contains("compile"));
}
```

`terminal::set_color_choice(ColorChoice::Never)` also works, but it is
process-wide: tests run in parallel by default, so one test turning colour off
affects the others. Prefer the explicit-depth form.

### Testing what the program prints

The hard part of testing a CLI is normally its *output*, because it goes to a
file descriptor — so the usual answer is to spawn the binary and read its pipes.
`capture` makes it an ordinary assertion instead:

```rust
use cli_forge::{capture, ok, out, terminal, warn, App, ColorChoice, Command, Glyphs, Theme};

fn main() {
    // #[test] fn build_reports_what_it_did()
    //
    // Pin what the terminal would otherwise decide: under the defaults `✓`
    // becomes `+` where it cannot be drawn, and colour is on wherever
    // FORCE_COLOR is set. A test must not depend on where it runs.
    terminal::set_color_choice(ColorChoice::Never);
    Theme::new().set_glyphs(Glyphs::Unicode).install();

    let mut app = App::new("forge");
    app.register(Command::new("build").run(|_| {
        out("building...");
        warn("this build is not optimised");
        ok("compiled 3 targets");
    }));

    let (outcome, log) = capture(|| app.try_run_from(["build"]));

    assert!(outcome.unwrap().is_ok());
    assert_eq!(log.lines(cli_forge::Stream::Stdout), ["building...", "✓ compiled 3 targets"]);
    assert!(log.err().contains("not optimised"));
}
```

Notice what that asserts without arranging anything: the warning went to standard
error and the success did not. That is the stream discipline a theme owns, and
nothing other than a capture can observe it.

`Captured` gives you `out()`, `err()`, `combined()` (both streams in the order
the lines were written), `lines(stream)` (without the trailing newlines, which
makes a failure far more readable), and `is_empty()`.

Two things to know:

- **It is per-thread**, so tests that capture run in parallel with tests that
  print, and a thread spawned inside the closure is not captured.
- **Pin anything that depends on the environment.** The theme's glyphs and the
  colour depth are decided by the terminal, so an assertion on `✓` passes on your
  machine and fails on a CI runner that falls back to `+` — or that sets
  `FORCE_COLOR`, which puts escape codes in the captured text. In tests, call
  `terminal::set_color_choice(ColorChoice::Never)` and install the theme with
  `set_glyphs(Glyphs::Unicode)` (or `Glyphs::Ascii`). A capture records exactly
  the bytes that would have been written, which is why both matter. The theme is also process-wide, so tests
  that install different themes should not run in parallel with each other —
  have each take a shared `Mutex`.
- **It sees only what went through this crate.** Not `println!`, not a direct
  write to `std::io::stdout`, and not a child process. And only the entry points
  that print at all print: `try_run_from` and `try_parse_from` hand failures back
  rather than reporting them, so a capture around one of those sees handler output
  and nothing else.

---

## Reading the command tree

`App`, `Command`, and `Arg` expose read-only accessors, so a tool can generate
shell completions, manual pages, or documentation from the **live** tree instead
of a second description of the same CLI that drifts.

```rust
use cli_forge::{App, Arg, Command};

let app = App::new("forge")
    .version("2.0.0")
    .arg(Arg::count("verbose").short('v').global(true))
    .command(
        Command::new("build")
            .about("compile the project")
            .arg(Arg::option("level").possible_values(["warn", "info"])),
    );

// What a completion generator walks.
for command in app.commands() {
    let _ = (command.name(), command.alias_names(), command.about_text());
    for arg in command.arguments() {
        let _ = (
            arg.name(),
            arg.short_form(),
            arg.long_form(),
            arg.help_text(),
            arg.expects_value(),     // offer a value next?
            arg.allowed_values(),    // complete the values too
            arg.is_hidden(),         // skip it
        );
    }
    for sub in command.subcommands() {
        let _ = sub.name();
    }
}
assert_eq!(app.commands()[0].arguments()[0].allowed_values(), ["warn", "info"]);
```

---

## Common mistakes

**Validating inside the handler.** Use `possible_values` or `validate`, so the
refusal happens where a good message is still possible.

**`unwrap()` on a value you did not validate.** Either validate it, or use
`try_get` and report the error.

**Treating `HelpRequested` as a failure.** Check `is_request()`.

**Asserting on help layout.** Assert on content; layout is presentation.

**Expecting `try_parse_from` to run handlers.** It does not. Use `try_run_from`.

**Spelling "exactly one of these" as pairwise conflicts.** Use an `ArgGroup`
with `required(true)`; it is one line, and it requires one as well as forbidding
two.

**Reading `flag` when a config file has an opinion.** `flag` cannot tell "turned
off" from "never mentioned"; `explicit_flag` can.

**Putting a variadic positional before another positional.** It absorbs
everything; put it last.

**Forgetting `.global(true)` on an app-level argument** you want accepted after
the command name.

**Calling `std::env::args()` yourself.** It panics on an argument that is not
valid UTF-8. `run()` and `parse()` read the raw form and report it instead.

<hr>
<br>

## See also

- **[Output & Styling](./OUTPUT.md)** — the other half of the crate.
- **[Recipes](./RECIPES.md)** — answers without the explanation.
- **[API](./API.md)** — the full surface and the stability promise.

<div align="center">
  <h2></h2>
  <sup>COPYRIGHT <small>&copy;</small> 2026 <strong>James Gober <me@jamesgober.com>.</strong></sup>
</div>
