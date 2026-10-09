<h1 align="center">
    <img width="90px" height="auto" src="https://raw.githubusercontent.com/jamesgober/jamesgober/main/media/icons/hexagon-3.svg" alt="Triple Hexagon">
    <br><b>cli-forge · Output &amp; Styling</b>
</h1>

<p align="center">
  <code>v2.1.0</code> &mdash; colours, themes, markers, columns, markup, and measuring text.
</p>

> **This is the long one.** If you want a specific answer rather than an
> explanation, try [Recipes](./RECIPES.md). If you have not read the
> [Guide](./GUIDE.md), start there.

<hr>
<br>

## Contents

- [The mental model](#the-mental-model)
- [Plain output](#plain-output)
- [Themed responses](#themed-responses)
  - [The eight levels](#the-eight-levels)
  - [Building a theme](#building-a-theme)
  - [Glyphs and ASCII fallback](#glyphs-and-ascii-fallback)
  - [Which stream a level goes to](#which-stream-a-level-goes-to)
  - [A theme as a value](#a-theme-as-a-value)
- [Styles](#styles)
  - [One-off styling](#one-off-styling)
  - [Reusable styling](#reusable-styling)
  - [Decoration](#decoration)
  - [Combining styles](#combining-styles)
- [Named styles](#named-styles)
- [Aligned columns](#aligned-columns)
- [Markup](#markup)
- [Colour](#colour)
  - [The sixteen](#the-sixteen)
  - [Indexed and exact colour](#indexed-and-exact-colour)
  - [Backgrounds](#backgrounds)
  - [Attributes](#attributes)
  - [Graceful degradation](#graceful-degradation)
- [Hyperlinks](#hyperlinks)
- [Controlling colour](#controlling-colour)
- [Measuring text](#measuring-text)
- [Untrusted text](#untrusted-text)
- [Which path should I use?](#which-path-should-i-use)
- [Common mistakes](#common-mistakes)
- [Without the standard library](#without-the-standard-library)

---

## The mental model

There is **one** styling system here, reached four ways. They are not
alternatives that behave differently — they produce byte-identical output for the
same intent, which is checked by tests at every colour depth. The choice between
them is about how you want to *write* it.

| Path | Looks like | Use it when |
|---|---|---|
| **Theme level** | `ok("done")` | The thing being reported is a success, failure, warning, or note. **Most output.** |
| **Named style** | `named("step").paint(label)` | The same custom look recurs across the program. |
| **Builder** | `style("done").green().bold()` | A genuine one-off, or a style you compute. |
| **Markup** | `markup("<c=red>no</c> and <c=green>yes</c>")` | The styling varies run-by-run *inside* one sentence. |

The order of that table is the order you should reach for them. Most programs
need the first two and almost nothing else.

Underneath all four:

```text
                      ┌─────────────┐
  ok() / fail() ──────►│             │
  named("x").paint() ─►│  one escape │──► out() / err() ──► terminal
  style("x").red() ───►│   writer    │
  markup("<c=red>") ──►│             │
                      └──────┬──────┘
                             │ asks once, per stream
                      ┌──────▼──────┐
                      │  terminal   │  true colour? 256? 16? none?
                      └─────────────┘
```

---

## Plain output

```rust
use cli_forge::{err, out};

fn main() {
    out("building...");       // standard output
    err("that failed");       // standard error
}
```

Both take **anything printable**, so there is only ever one function to reach
for:

```rust
use cli_forge::out;

fn main() {
    out("text");
    out(42);
    out(1.5);
    out(format!("{} files", 3));
    out(std::path::Path::new("/tmp").display());
}
```

**They never parse anything.** `out("<b>not bold</b>")` prints those angle
brackets literally. That is deliberate: printing is the hot path, and making it
scan every string for markup would tax every line to serve a few. If you want
markup, parse it explicitly with [`markup`](#markup).

A failed write is ignored, because the usual cause is a closed pipe
(`yourtool | head`) and turning that into a panic would be worse than useless.
When you genuinely need to know:

```rust
use cli_forge::{write_to, Stream};

fn main() {
    if write_to(Stream::Stdout, "a line that matters").is_err() {
        // The pipe is gone; stop producing output.
    }
}
```

---

## Themed responses

### The problem this solves

Here is what most codebases look like:

```rust
// Don't do this.
use cli_forge::{out, style};

fn main() {
    out(style("✓ compiled 3 targets").green().bold());
    out(style("! 2 tests skipped").yellow().bold());
    out(style("✗ smoke test failed").red().bold());
}
```

Four problems, all of which get worse as the program grows:

1. The colour is named at every call site. Forty status lines, forty decisions.
2. The glyph is hard-coded, so a terminal that cannot draw `✓` shows mojibake.
3. All three went to standard output, so `yourtool > out.txt` swallows the
   failure.
4. Changing the look means editing every line.

### The fix

```rust
use cli_forge::{fail, ok, warn, Theme};

fn main() {
    Theme::new().install();     // once, at the top of main

    ok("compiled 3 targets");
    warn("2 tests skipped");
    fail("smoke test failed");
}
```

```text
✓ compiled 3 targets
! 2 tests skipped
✗ smoke test failed
```

The call sites now say *what happened*, not what it should look like. The theme
supplies the colour, the glyph, and the stream — and handles all four problems
above.

### The eight levels

| Function | Level | Default marker | Goes to | Use for |
|---|---|---|---|---|
| `ok` | `Success` | `✓` | stdout | Something finished as intended. |
| `fail` | `Error` | `✗` | **stderr** | Something went wrong. |
| `warn` | `Warning` | `!` | **stderr** | Not wrong, but worth attention. |
| `info` | `Info` | `i` | stdout | Neutral progress or context. |
| `hint` | `Hint` | `→` | stdout | A suggested next action. |
| `note` | `Note` | `•` | stdout | An aside worth keeping. |
| `debug` | `Debug` | `·` | **stderr** | Detail for diagnosing the program. |
| `trace` | `Trace` | `·` | **stderr** | Fine-grained detail. |

All eight take anything printable, exactly like `out`:

```rust
use cli_forge::{info, ok};

fn main() {
    ok("done");
    ok(format!("compiled {} targets", 3));
    info(42);
}
```

### Building a theme

`Theme::new()` is a sensible default. Adjust what you want and leave the rest:

```rust
use cli_forge::{Level, Style, Theme};

fn main() {
    Theme::new()
        // Change the style AND the marker.
        .set(Level::Success, Style::new().bright_green().bold(), "✓")
        .set(Level::Error, Style::new().bright_red().bold(), "✗")
        // Change only the style, keeping the automatic marker.
        .set_style(Level::Hint, Style::new().bright_black().italic())
        .install();

    cli_forge::ok("this is now bright green and bold");
}
```

The full set of methods:

| Method | Does |
|---|---|
| `Theme::new()` | Sensible defaults: the terminal's bright palette, Unicode markers where they render. |
| `Theme::plain()` | No styling and no markers. What a `--plain` flag should install. |
| `.set(level, style, glyph)` | Set both. Pass `""` as the glyph for no marker. |
| `.set_style(level, style)` | Set the style, keep the marker. |
| `.set_stream(level, stream)` | Send a level somewhere else. |
| `.set_glyphs(mode)` | Choose the marker set — see below. |
| `.install()` | Make it the process default. Call once, at startup. |
| `Theme::current()` | A copy of the process default. |
| `.style(level)` / `.stream(level)` / `.glyph(level)` | Read it back. |

> **Call `install()` once, early.** It replaces whatever was there. If you are
> writing a *library*, do not call it at all — see
> [A theme as a value](#a-theme-as-a-value).

A badge-style theme, for contrast:

```rust
use cli_forge::{Level, Style, Theme};

fn main() {
    Theme::new()
        .set(Level::Success, Style::new().black().on_green().bold(), " OK ")
        .set(Level::Error, Style::new().white().on_red().bold(), " ER ")
        .install();

    cli_forge::ok("deployed");    //  OK  deployed, on a green block
}
```

### Glyphs and ASCII fallback

```rust
use cli_forge::{Glyphs, Level, Theme};

fn main() {
    // The default: Unicode where it will render, ASCII where it will not.
    let auto = Theme::new().set_glyphs(Glyphs::Auto);

    // Or force one.
    let unicode = Theme::new().set_glyphs(Glyphs::Unicode);
    let ascii = Theme::new().set_glyphs(Glyphs::Ascii);
    let bare = Theme::new().set_glyphs(Glyphs::None);

    assert_eq!(unicode.glyph(Level::Success), "✓");
    assert_eq!(ascii.glyph(Level::Success), "+");
    assert_eq!(bare.glyph(Level::Success), "");
    assert!(matches!(auto.glyph(Level::Success), "✓" | "+"));
}
```

The ASCII stand-ins are `+ x ! i > * - .` — **every one is exactly one column
wide**, matching its Unicode counterpart. That means switching glyph sets never
shifts a column, so a table of status lines stays aligned either way. Try it:

```console
$ cargo run --example status_report
✓ resolve dependencies   0.4s
✓ compile (release)      31.7s
! run test suite         12 of 14

$ NO_UNICODE=1 cargo run --example status_report
+ resolve dependencies   0.4s
+ compile (release)      31.7s
! run test suite         12 of 14
```

`Glyphs::Auto` decides by looking at the locale on Unix and the terminal host on
Windows. It is a heuristic — there is no portable way to ask a terminal — so
setting `NO_UNICODE` forces ASCII for the cases where a terminal claims support
its font cannot back.

### Which stream a level goes to

Errors, warnings, and the two diagnostic levels go to **standard error**;
everything else goes to standard output. This is not cosmetic. It is what makes
your program composable:

```console
$ yourtool list > names.txt      # names.txt holds data, not warnings
$ yourtool build 2> errors.log   # errors.log holds complaints, not data
$ yourtool build 2>/dev/null     # quiet the complaints, keep the output
```

Override it when the convention does not suit:

```rust
use cli_forge::{Level, Stream, Theme};

fn main() {
    // A program whose warnings belong in its piped output.
    Theme::new().set_stream(Level::Warning, Stream::Stdout).install();
}
```

Each line also renders at the depth detected for **its own** stream, so a program
with `stdout` piped and `stderr` on a terminal gets clean bytes in the pipe and
colour on screen — independently, and without arranging anything.

### A theme as a value

A `Theme` is a plain value. Hold one and render through it directly when you must
not disturb the host program's theme — which is exactly the situation a library
is in:

```rust
use cli_forge::{out, Glyphs, Level, Theme};

fn main() {
    // My own theme, not the process default.
    let mine = Theme::new().set_glyphs(Glyphs::Unicode);

    out(mine.render(Level::Hint, "rendered through a local theme"));
}
```

`render` picks the depth for that level's stream; `render_at` takes an explicit
depth, which is what you want for a log file or a test.

---

## Styles

A `Style` is a **description of an appearance**. It does not hold text unless you
give it some, and it can be applied to anything.

### One-off styling

```rust
use cli_forge::{out, style};

fn main() {
    out(style("done").green().bold());
    out(style("note").hex("#88aaff"));
    out(style("count: 3").dim());
}
```

`style("text")` makes a `Style` carrying that text, and the result prints
directly.

### Reusable styling

`Style::new()` makes an empty one, meant to be applied to many values:

```rust
use cli_forge::{out, Style};

fn main() {
    let heading = Style::new().bold().underline();

    out(heading.paint("Results"));
    out(heading.paint("Summary"));
    out(heading.paint(42));          // anything printable
}
```

`paint` is the reuse path and **allocates nothing** — the escape codes are
written straight into the output as it prints. There is no intermediate string.

If you need an owned `String` — to put in a table cell, write to a file, or
measure — use `render` (for a style's own text) or `paint(...).to_string()`:

```rust
use cli_forge::{text, Style};

fn main() {
    let cell = Style::new().red().paint("value").to_string();
    assert_eq!(text::width(&cell), 5);          // 5 columns, not 14 bytes
}
```

### Decoration

This is the part that removes hand-built markers. A `Style` carries more than
colour:

```rust
use cli_forge::Style;

fn main() {
    let s = Style::new()
        .green()                 // colour
        .bold()                  // attribute
        .prefix("✓ ")            // text before, inside the styling
        .suffix(" (done)")       // text after, inside the styling
        .pad_to(20);             // pad to 20 display columns
    let _ = s;
}
```

| Method | Does |
|---|---|
| `.prefix(text)` | Put text immediately before, inside the styling. |
| `.suffix(text)` | Put text immediately after, inside the styling. |
| `.pad_to(columns)` | Pad out to that many **display columns**. |
| `.align(Align)` | `Left` (default), `Right`, or `Center` within the padding. |
| `.link(url)` | Make it a clickable hyperlink. |

Because the glyph, the colour, *and* the width travel together, a marker is
described in one place:

```rust
use cli_forge::{out, Style};

fn main() {
    let ok = Style::new().green().bold().prefix("[ok] ").pad_to(14);

    out(ok.paint("resolve"));
    out(ok.paint("compile"));
}
```

> **`pad_to` is the one method that allocates**, because it has to build the
> finished text before it can measure it. Everything else writes straight
> through. `pad_to` also never truncates: content wider than the budget is left
> alone, because silently losing a caller's text is worse than a ragged column.
> Pair it with [`text::truncate`](#measuring-text) when the budget is hard.

### Combining styles

`merge` layers one style over another. Anything the overlay sets wins; anything
it leaves alone is inherited:

```rust
use cli_forge::Style;

fn main() {
    let base = Style::new().green().prefix("✓ ");

    let loud = base.clone().merge(&Style::new().bold());     // green, ✓, bold
    let red = base.merge(&Style::new().red());               // red, ✓

    let _ = (loud, red);
}
```

`is_plain()` tells you whether a style would emit anything at all — useful for
skipping work:

```rust
use cli_forge::Style;

fn main() {
    assert!(Style::new().is_plain());
    assert!(!Style::new().red().is_plain());
}
```

---

## Named styles

The registry is for a program's own visual vocabulary: the looks that are neither
one of the eight theme levels nor a one-off, but that appear in a dozen places
and must match in all of them.

```rust
use cli_forge::{define, named, out, Style};

fn main() {
    // Describe it once — anywhere, including another module.
    define("step", Style::new().bright_black().prefix("  → ").pad_to(24));
    define("path", Style::new().cyan().underline());

    // Use it anywhere else.
    out(named("step").paint("resolve dependencies"));
    out(named("path").paint("./forge.toml"));
}
```

| Function | Does |
|---|---|
| `define(name, style)` | Store it. A later `define` of the same name replaces it. |
| `named(name)` | Look it up. An unknown name gives a plain style, so nothing breaks. |
| `defined(name)` | Whether it has been defined — fill in a default without overwriting. |

An unknown name renders the text unstyled rather than failing. A program must not
die because a theme forgot an entry:

```rust
use cli_forge::named;

fn main() {
    assert_eq!(named("never-defined").paint("text").to_string(), "text");
}
```

### Two things to know

**The store is process-global.** That is the point — a name defined in one module
resolves in another, including across crate boundaries. The cost is that names
can collide, so **library code should prefix its names** (`mycrate.step`) and
application code can use bare ones.

**Hoist the lookup out of loops.** A lookup takes a read lock and clones the
style (~176 ns against ~149 ns for the hoisted form):

```rust
use cli_forge::{define, named, out, Style};

fn main() {
    define("row", Style::new().bold());

    let row = named("row");                  // once
    for item in ["a", "b", "c"] {
        out(row.paint(item));                // many
    }
}
```

---

## Aligned columns

Columns are the thing hand-rolled output gets wrong most often, because the
obvious tool is wrong:

```rust
fn main() {
    // WRONG: {:<10} counts bytes, not columns.
    println!("{:<10}|", "日本語");      // 3 chars, 6 columns, 9 bytes
    println!("{:<10}|", "abcdef");     // 6 chars, 6 columns, 6 bytes
    // The two bars do not line up.
}
```

`pad_to` measures **display columns**, which is the number that matters:

```rust
use cli_forge::{text, Style};

fn main() {
    let col = Style::new().pad_to(10);

    for label in ["abcdef", "日本語", "éé", "emoji 🎉"] {
        let padded = col.paint(label).to_string();
        assert_eq!(text::width(&padded), 10, "{label}");
    }
}
```

It is also correct for text that is **already styled**, where byte length is
hopeless:

```rust
use cli_forge::{text, ColorLevel, Style};

fn main() {
    // `paint_at` forces a depth, so this example reads the same everywhere.
    // Plain `paint` depends on the terminal — the right default for a program,
    // and the wrong one for an assertion.
    let styled = Style::new()
        .red()
        .bold()
        .paint_at("done", ColorLevel::Ansi16)
        .to_string();

    assert_eq!(styled.len(), 15);          // bytes, including escapes
    assert_eq!(text::width(&styled), 4);   // what the user sees
}
```

### The complete status-line pattern

Everything together — and compare this with what it replaces:

```rust
use cli_forge::{define, fail, named, ok, warn, Level, Style, Theme};

enum Status { Ok, Warn, Fail }

fn main() {
    // Stated once.
    Theme::new()
        .set_style(Level::Success, Style::new().bright_green().bold())
        .set_style(Level::Warning, Style::new().bright_yellow().bold())
        .set_style(Level::Error, Style::new().bright_red().bold())
        .install();
    define("step", Style::new().pad_to(22));
    define("detail", Style::new().bright_black());

    step("resolve dependencies", Status::Ok, "0.4s");
    step("run test suite", Status::Warn, "12 of 14");
    step("smoke test", Status::Fail, "timeout");
}

/// No colour, no glyph, no padding arithmetic. The theme owns the marker, the
/// named style owns the width, and both are measured in display columns.
fn step(label: &str, status: Status, detail: &str) {
    let line = format!("{} {}", named("step").paint(label), named("detail").paint(detail));
    match status {
        Status::Ok => ok(line),
        Status::Warn => warn(line),
        Status::Fail => fail(line),
    }
}
```

```text
✓ resolve dependencies   0.4s
! run test suite         12 of 14
✗ smoke test             timeout
```

For real tables, use a sibling crate — but it will measure through
[`text::width`](#measuring-text), the same function, so the two will agree.

---

## Markup

Markup is for the case the builder handles badly: styling that changes run by run
*inside* one sentence.

```rust
use cli_forge::{markup, out};

fn main() {
    out(markup("<b>summary</b>: <c=green>12 passed</c>, <c=red>1 failed</c>"));
}
```

Written with the builder, that line would be three `format!` arguments and three
chains. As markup it reads like the sentence it is.

### The grammar

| Tag | Effect |
|---|---|
| `<b>` … `</b>` | **bold** |
| `<d>` … `</d>` | dim |
| `<i>` … `</i>` | *italic* |
| `<u>` … `</u>` | underline |
| `<s>` … `</s>` | strikethrough |
| `<r>` … `</r>` | reverse video |
| `<c=VALUE>` … `</c>` | foreground colour |
| `<bg=VALUE>` … `</bg>` | background colour |
| `<link=URL>` … `</link>` | hyperlink |
| `</>` | close the innermost open tag, whatever it was |
| `<<` | a literal `<` |

`VALUE` is anything [`Color::parse`](#colour) accepts: `red`, `bright_red`,
`#ff8800`, `#f80`, `255,136,0`, or a palette index like `208`.

```rust
use cli_forge::{markup, out};

fn main() {
    out(markup("<c=red><b>ERROR:</b></c> disk almost full"));
    out(markup("<c=#3b82f6><u>./forge.toml</u></c>"));
    out(markup("<bg=yellow><c=black> NOTE </c></bg> read the manual"));
    out(markup("<b>a<u>b</></>c"));          // </> closes u, then b
    out(markup("2 << 3 is 16"));             // a literal <
}
```

### It never fails

Markup is text a program prints, so a mistake in it must not take the program
down or swallow the message. **Anything unrecognised is printed as written:**

```rust
use cli_forge::markup;

fn main() {
    assert_eq!(markup("a <unknown> b"), "a <unknown> b");
    assert_eq!(markup("less < than"), "less < than");
    assert_eq!(markup("open <b without close"), "open <b without close");
    assert_eq!(markup("</nope>"), "</nope>");
}
```

A colour value that is not a colour opens a span that inherits the surrounding
colour, rather than failing. A mismatched close is ignored. An unclosed tag is
closed at the end of the string, so styling cannot leak into the next line.
Nesting is capped at 64 frames, so markup from an untrusted source cannot grow
the parser's state without bound.

### `markup` returns a `String`

That is what makes it composable — the result can be printed, written to a file,
measured, or put in a table cell:

```rust
use cli_forge::{markup, out, text};

fn main() {
    let line = markup("<c=green>ok</c>");
    assert_eq!(text::width(&line), 2);
    out(line);
}
```

Use `markup_at(tags, level)` for an explicit colour depth — a log file, standard
error when the two streams differ, or a test that needs deterministic bytes.

> **Markup is not a sanitiser.** An escape byte in the *input* is content and is
> passed through. See [Untrusted text](#untrusted-text).

---

## Colour

### The sixteen

```rust
use cli_forge::{out, Style};

fn main() {
    out(Style::new().red().paint("the user's red"));
    out(Style::new().bright_red().paint("the user's bright red"));
}
```

Foreground: `black` `red` `green` `yellow` `blue` `magenta` `cyan` `white`, and
`bright_black` `bright_red` `bright_green` `bright_yellow` `bright_blue`
`bright_magenta` `bright_cyan` `bright_white`.

**Prefer these over exact values.** They resolve to whatever the user chose in
their terminal's scheme, so your "red" is a red that works on their background.
A hard-coded `#cc0000` can be invisible on a light theme; `red` cannot.

`bright_black` is the conventional colour for de-emphasised text, and is what
`dim` should usually be replaced by (not every terminal implements `dim`).

### Indexed and exact colour

```rust
use cli_forge::{out, Color, Style};

fn main() {
    out(Style::new().ansi(208).paint("palette index 208"));
    out(Style::new().hex("#ff8800").paint("exact, long form"));
    out(Style::new().hex("#f80").paint("exact, shorthand"));
    out(Style::new().rgb(255, 136, 0).paint("exact, channels"));
    out(Style::new().fg(Color::Rgb(255, 136, 0)).paint("exact, explicitly"));
}
```

An invalid hex string is **ignored**, not an error — a typo in a palette must not
take a program down:

```rust
use cli_forge::Style;

fn main() {
    assert_eq!(Style::new().hex("nope").paint("x").to_string(), "x");
}
```

Parsing a colour from text (a config file, a `--color` value) uses
`Color::parse`:

```rust
use cli_forge::Color;

fn main() {
    assert_eq!(Color::parse("GREEN"), Some(Color::Green));
    assert_eq!(Color::parse("bright-blue"), Some(Color::BrightBlue));
    assert_eq!(Color::parse("bright_blue"), Some(Color::BrightBlue));
    assert_eq!(Color::parse("#f80"), Some(Color::Rgb(255, 136, 0)));
    assert_eq!(Color::parse("0, 200, 120"), Some(Color::Rgb(0, 200, 120)));
    assert_eq!(Color::parse("208"), Some(Color::Ansi(208)));
    assert_eq!(Color::parse("chartreuse"), None);
}
```

Names are case-insensitive and ignore `_`, `-`, and spaces, so every spelling a
user might write in a config file resolves. `grey`/`gray` and `purple` are
accepted as synonyms.

### Backgrounds

```rust
use cli_forge::{out, Color, Style};

fn main() {
    out(Style::new().black().on_green().bold().paint(" PASS "));
    out(Style::new().white().on_red().bold().paint(" FAIL "));
    out(Style::new().on_hex("#3b82f6").white().paint(" NEW "));
    out(Style::new().bg(Color::BrightYellow).black().paint(" HOT "));
}
```

`on_black` `on_red` `on_green` `on_yellow` `on_blue` `on_magenta` `on_cyan`
`on_white`, plus `on_hex`, `on_rgb`, `on_ansi`, and `bg(Color)` for anything
else — including the bright variants, which have no `on_bright_*` shorthand.

Backgrounds combine with `pad_to` to make a solid badge, because the padding goes
*inside* the styling:

```rust
use cli_forge::{out, Style};

fn main() {
    out(Style::new().black().on_green().pad_to(10).paint("PASS"));
    // A green block ten columns wide, not a green word in a plain box.
}
```

### Attributes

```rust
use cli_forge::{out, Style};

fn main() {
    out(Style::new().bold().paint("bold"));
    out(Style::new().dim().paint("dim"));
    out(Style::new().italic().paint("italic"));
    out(Style::new().underline().paint("underline"));
    out(Style::new().strike().paint("struck through"));
    out(Style::new().reverse().paint("reversed"));
    out(Style::new().blink().paint("blinking"));
    out(Style::new().hidden().paint("hidden"));
}
```

Three warnings:

- `dim` and `italic` are not implemented by every terminal; those that lack them
  ignore the request. For de-emphasis, `bright_black` is more reliable.
- `blink` is widely disabled and worth avoiding for anything the reader must
  actually read.
- `hidden` means "present but not drawn". It is **not a security measure** — the
  bytes are still in the stream, and a transcript or a copy-paste reveals them.

### Graceful degradation

An exact colour is never dropped when it could be approximated:

| Colour | True colour | 256 colour | 16 colour | None |
|---|---|---|---|---|
| `Rgb` / `hex` | exact | nearest cube or greyscale entry | nearest of 16 | dropped |
| `Ansi(n)` | exact | exact | nearest of 16 | dropped |
| named | exact | exact | exact | dropped |

Named colours are never downgraded, because they *are* the user's palette.

See it concretely:

```console
$ cargo run --example colors
...
Degradation
  The same exact colour at each capability tier:
    true colour  ████  22 byte(s) of escapes
    256 colour   ████  14 byte(s) of escapes
    16 colour    ████  9 byte(s) of escapes
    none         ████  0 byte(s) of escapes
```

Nearness is measured with weighted-luminance distance (green dominates, blue
barely registers), because plain RGB distance sends mid greens to black often
enough to notice.

---

## Hyperlinks

```rust
use cli_forge::{out, Style};

fn main() {
    out(Style::new()
        .cyan()
        .underline()
        .link("https://docs.rs/cli-forge")
        .paint("the documentation"));
}
```

Or in markup:

```rust
use cli_forge::{markup, out};

fn main() {
    out(markup("see <link=https://docs.rs/cli-forge>the documentation</link>"));
}
```

This uses the OSC 8 sequence. Terminals that support it make the text clickable;
terminals that do not ignore the sequence and show the text unchanged — so it is
**always safe to add**. A hyperlink adds no display columns:

```rust
use cli_forge::{text, Style};

fn main() {
    let linked = Style::new().link("https://example.com").paint("click").to_string();
    assert_eq!(text::width(&linked), 5);
}
```

It is the one way to put a long URL in output without making the user read it.

---

## Controlling colour

### What happens by default

Colour is decided **per stream**, detected once and cached. The order, highest
priority first:

1. `terminal::set_level(..)` — an exact depth you forced.
2. `terminal::set_color_choice(..)` — `Never` wins over everything below;
   `Always` enables colour even into a pipe.
3. `CLICOLOR_FORCE` or `FORCE_COLOR` set to anything but `0` — force on.
4. `NO_COLOR` set to anything non-empty, or `CLICOLOR=0` — off.
5. `TERM=dumb`, or the stream is not a terminal — off.

Then the depth comes from `COLORTERM`, Windows Terminal, ConEmu, and `TERM`,
defaulting to the sixteen standard colours.

```console
$ yourtool                 # colour, if the terminal supports it
$ yourtool | cat           # no colour: stdout is a pipe
$ yourtool | cat 2>&1      # stderr still coloured if it is a terminal
$ NO_COLOR=1 yourtool      # no colour anywhere
$ CLICOLOR_FORCE=1 yourtool | less -R    # colour into a pager
```

### Overriding it from the program

This is what a `--color` flag needs:

```rust
use cli_forge::{terminal, ColorChoice};

fn main() {
    // Parsed from `--color=never`.
    terminal::set_color_choice(ColorChoice::Never);
    // ...or forced on, for a pager that understands escapes.
    terminal::set_color_choice(ColorChoice::Always);
    // ...or back to detection.
    terminal::set_color_choice(ColorChoice::Auto);
}
```

Wired up end to end:

```rust
use std::process::ExitCode;

use cli_forge::{terminal, App, Arg, ColorChoice, Command};

fn main() -> ExitCode {
    let mut app = App::new("forge").arg(
        Arg::option("color")
            .global(true)
            .default("auto")
            .possible_values(["auto", "always", "never"])
            .help("when to colour the output"),
    );
    app.register(Command::new("build").run(|m| {
        // Honoured before anything is printed.
        terminal::set_color_choice(match m.value("color") {
            Some("always") => ColorChoice::Always,
            Some("never") => ColorChoice::Never,
            _ => ColorChoice::Auto,
        });
        cli_forge::ok("built");
    }));
    app.run()
}
```

`App::color(choice)` sets it for the whole app if you do not need a flag.

### Forcing an exact depth

For a destination whose capability you know from outside — a recorded session, a
capability-negotiated console, or a test that needs deterministic bytes:

```rust
use cli_forge::{terminal, ColorLevel, Stream, Style};

fn main() {
    terminal::set_level(ColorLevel::Ansi16);
    assert_eq!(terminal::level(Stream::Stdout), ColorLevel::Ansi16);
    assert_eq!(
        Style::new().red().paint("x").to_string(),
        "\u{1b}[31mx\u{1b}[0m"
    );
    terminal::clear_level();          // back to detection
}
```

In a test, prefer `paint_at` / `markup_at` / `render_at`, which take a depth
without touching process state:

```rust
use cli_forge::{ColorLevel, Style};

fn main() {
    let bytes = Style::new().red().paint_at("x", ColorLevel::Ansi16).to_string();
    assert_eq!(bytes, "\u{1b}[31mx\u{1b}[0m");
}
```

### Asking what the terminal can do

```rust
use cli_forge::{terminal, Stream};

fn main() {
    let _ = terminal::level(Stream::Stdout);     // ColorLevel
    let _ = terminal::size();                    // Option<(cols, rows)>
    let _ = terminal::width_or(80);              // cols, or 80
    let _ = terminal::supports_unicode();        // bool
}
```

---

## Measuring text

Once text carries escape sequences, `str::len()` and `chars().count()` both stop
answering the question anyone is asking. The `text` module is the one that does.

```rust
use cli_forge::{text, ColorLevel, Style};

fn main() {
    let styled = Style::new()
        .red()
        .bold()
        .paint_at("hello", ColorLevel::Ansi16)
        .to_string();

    assert_eq!(styled.len(), 16);               // bytes
    assert_eq!(text::width(&styled), 5);        // columns — what you want
    assert_eq!(text::strip(&styled), "hello");  // the visible text
}
```

| Function | Does |
|---|---|
| `text::width(s)` | Display columns, skipping escape sequences. |
| `text::strip(s)` | Remove escape sequences. Borrows if there are none. |
| `text::pad(s, cols, align)` | Pad to a width. Never truncates. |
| `text::truncate(s, cols, ellipsis)` | Cut to a width, closing any open styling. |
| `text::wrap(s, cols)` | Break into lines, at whitespace where possible. |
| `text::sanitize(s)` | Neutralise control characters. |

```rust
use cli_forge::text::{self, Align};

fn main() {
    assert_eq!(text::pad("ok", 6, Align::Left), "ok    ");
    assert_eq!(text::pad("ok", 6, Align::Right), "    ok");
    assert_eq!(text::pad("ok", 6, Align::Center), "  ok  ");

    assert_eq!(text::truncate("hello world", 8, "…"), "hello w…");
    assert_eq!(text::truncate("short", 8, "…"), "short");

    assert_eq!(text::wrap("the quick brown fox", 11), ["the quick", "brown fox"]);
    assert_eq!(text::wrap("one\ntwo", 80), ["one", "two"]);
}
```

All of them are safe on styled text. `truncate` never cuts an escape sequence in
half, and closes any styling still open at the cut — so a truncated cell cannot
leak its colour into whatever is printed next:

```rust
use cli_forge::{text, ColorLevel, Style};

fn main() {
    let long = Style::new()
        .red()
        .paint_at("hello world", ColorLevel::Ansi16)
        .to_string();

    let cut = text::truncate(&long, 5, "");
    assert_eq!(text::strip(&cut), "hello");
    assert!(cut.ends_with("\u{1b}[0m"));        // closed, not leaking
}
```

`wrap` is lossless: it breaks at whitespace where it can and mid-word only when a
single word cannot fit at all, so no content is ever dropped. Existing newlines
are hard breaks, which is what makes it usable on help text written as
paragraphs.

> **On width accuracy.** With the default `unicode` feature, widths come from the
> Unicode East Asian Width tables: CJK and emoji count as two columns, combining
> marks as zero. Without it, the fallback counts characters — exact for Latin,
> Greek, and Cyrillic, wrong for the rest. Keep the feature on unless your output
> genuinely cannot contain either.

---

## Untrusted text

A string that came from outside your program — a filename, a commit message, a
server response, an error echoed back from a remote system — can contain escape
sequences. Printing it verbatim hands the terminal to whoever wrote it: it can
clear the screen, reposition the cursor to overwrite what you already printed,
relabel the window, change the colours permanently, or on some terminals push
text back into the input queue.

```rust
use cli_forge::{out, text};

fn main() {
    let hostile = "report.txt\u{1b}[2J\u{1b}[1;1HALL FILES DELETED";

    out(hostile);                     // DON'T: the screen is now a lie
    out(text::sanitize(hostile));     // DO
}
```

```text
report.txt^[[2J^[[1;1HALL FILES DELETED
```

Every C0 control character except `\n` and `\t`, every C1 control character, and
`DEL` become a visible caret escape — legible, and no longer executable. Tabs and
newlines survive, because they are layout rather than control.

`sanitize` borrows when there is nothing to neutralise, so wrapping text that is
already clean costs nothing:

```rust
use cli_forge::text;

fn main() {
    assert_eq!(text::sanitize("clean text"), "clean text");
    assert_eq!(text::sanitize("a\tb\nc"), "a\tb\nc");     // layout preserved
}
```

> **Where to apply it.** Anywhere text crosses into your program from outside.
> The crate cannot do it for you, because it cannot tell which of your strings
> you wrote and which you were handed.

---

## Which path should I use?

```text
Is it a success / failure / warning / note?
├─ yes ──► a theme level:  ok("...")  fail("...")  warn("...")
└─ no
   │
   Does the same look appear in several places?
   ├─ yes ──► a named style:  define("x", ..) then named("x").paint(..)
   └─ no
      │
      Does the styling change inside one sentence?
      ├─ yes ──► markup:  markup("<c=red>no</c> and <c=green>yes</c>")
      └─ no  ──► the builder:  style("text").green().bold()
```

---

## Common mistakes

**Padding with `format!`.**

```rust
fn main() {
    let label = "日本語";
    let _wrong = format!("{label:<10}");                    // counts bytes
    let _right = cli_forge::text::pad(label, 10, cli_forge::text::Align::Left);
}
```

**Measuring styled text with `len()`.** Use `text::width`.

**Expecting `out` to parse markup.** It does not, by design. Use
`out(markup("..."))`.

**Calling `Theme::install()` from a library.** It replaces the host program's
theme. Hold a `Theme` value and use `render` instead.

**Looking a named style up inside a loop.** Hoist it; it takes a lock and clones.

**Using `hidden()` for secrets.** The bytes are still in the stream.

**Printing an untrusted string directly.** Wrap it in `text::sanitize`.

**Hard-coding a hex colour for text the user reads.** It can vanish on a light
background. Prefer the sixteen named colours, which follow the user's scheme.

---

## Without the standard library

`--no-default-features` gives you the styling core on `alloc` alone: `Color`,
`Style`, `Theme`, `markup`, and the whole `text` module. There is no `out`, no
`err`, no named-style registry (it needs a lock), and no detection — so tell it
what the destination can do:

```rust
use cli_forge::{terminal, ColorLevel, Style};

fn main() {
    terminal::set_level(ColorLevel::Ansi256);

    // Render into whatever you have.
    let line = Style::new().green().bold().paint("done").to_string();
    let _ = line;
}
```

<hr>
<br>

## See also

- **[Commands & Arguments](./COMMANDS.md)** — the other half of the crate.
- **[Recipes](./RECIPES.md)** — answers without the explanation.
- **[API](./API.md)** — the full surface and the stability promise.

<div align="center">
  <h2></h2>
  <sup>COPYRIGHT <small>&copy;</small> 2026 <strong>James Gober <me@jamesgober.com>.</strong></sup>
</div>
