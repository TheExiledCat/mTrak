# Code conventions

How mTrak's code is written. Run `./fmt.sh` to format, group imports and apply clippy's
fixes; this document covers the house style on top of that. When in doubt, match the
surrounding code.

## Returns

Functions always end with an explicit `return`, never with a tail expression.

```rust
pub fn is_open(&self) -> bool {
    return self.is_open;
}

fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    return write!(f, "{}{}", note_name, octave);
}
```

This includes constructors (`return Self { .. };`), `Display` impls and one-line getters.
Clippy's `needless_return` lint is disabled in `Cargo.toml` for this reason.

This applies to named functions and methods only. Closures can use `return` or end with a
plain expression, whichever reads better, and `match` arms stay plain expressions.

```rust
let lines = bindings.iter().map(|b| Line::raw(format!("{}: {}", b.key, b.label)));

return match self {
    NoteKind::Empty => write!(f, "---"),
    NoteKind::Off => write!(f, "==="),
    NoteKind::On(pitch) => write!(f, "{}", pitch),
};
```

## Control flow

Bail out early instead of nesting. Use `let ... else` with a `return` when the value has to be
unpacked, and `?` when the failure value can be passed straight up.

```rust
let Char(c) = key.code else {
    return None;
};
let note = self.key_to_note_pitch(c)?;
```

## Visibility

Items are either `pub` or private. Don't use `pub(crate)` or `pub(super)`.

In an `impl` block, associated consts and types come first, then the public methods, then the
private ones. Trait impls follow the trait's order.

## Naming

- Types, traits and enum variants are `UpperCamelCase`, including error variants
  (`NoteError::ParseFailure`).
- Constants are `SCREAMING_SNAKE_CASE` and live at the top of the file that uses them, or in
  `tui/constants.rs` when they're shared.
- Constructors are `new`. Implement `Default` when `new` takes no arguments.
- Parsing and formatting use the standard traits: `FromStr` (called with `.parse()`) and
  `Display`. Don't implement `ToString` directly or add inherent `from_str`/`default` methods
  that shadow the traits. Other conversions from text are `from_string`/`from_char`.
- Keymap actions are an enum named after the widget: `TimelineAction`, `MainAction`,
  `FileBrowserAction`.
- Unused parameters required by a trait are prefixed with `_` (`_state: &AppState`). Unused
  fields in a pattern are `_`.

## Imports

Imports come in three groups separated by a blank line: `std`, external crates, then this
crate. Nest paths in a single `use` per crate, and import from `crate::` rather than climbing
with `super::` (tests are the exception, see below). `./fmt.sh` does the grouping and merging.

```rust
use std::time::{Duration, Instant};

use ratatui::{buffer::Buffer, layout::Rect};

use crate::tui::{app::AppState, framework::keymap::KeyHint};
```

Inside a function that builds a keymap or matches many variants, `use SomeEnum::*;` is fine to
keep the lines short.

## Comments

- Use `///` doc comments on public items, struct fields and constants whose purpose isn't
  obvious from the name. Explain why or what it means, not what the code does line by line.
- Mention FT2 when a behavior copies FastTracker 2, so it isn't "fixed" by accident.
- Keep `//` comments rare. Code that needs one is usually better restructured or named better.

## UI

- Take every color and style from `Theme` (`state.theme.*`), never hard-code them, so the
  monochrome and 16-color themes keep working.
- Bind keys through a `Keymap` and implement `KeyActions` so the footer hints stay in sync.
  Put the most used bindings first, since that's the order the hints are shown in.
- Any non-ASCII glyph in text goes through `glyphs::pick(unicode, ascii)` and has a mapping in
  `glyphs::to_ascii`, so ASCII mode stays readable.
- Widgets implement `MtrakWidget`. Widgets with children forward `key_hints` to them.

## Platforms

Platform differences use `#[cfg(...)]` on the item or `let` binding, with the other platforms
covered by a matching `#[cfg(not(...))]`, so every target compiles the same names. Use `cfg!()`
in plain `if` conditions. Detect capabilities from the environment (for example `COLORTERM`)
rather than hard-coding terminal or program names.

```rust
#[cfg(target_os = "macos")]
pub const ALT_SYMBOL: &str = "⌥";
#[cfg(not(target_os = "macos"))]
pub const ALT_SYMBOL: &str = "A-";
```

## Tests

Unit tests sit at the bottom of the file they test, in a `#[cfg(test)] mod tests` that starts
with `use super::*;`. Add tests for engine and data changes where you can.

## Docs

If you add or change a pattern command, update [COMMANDS.md](COMMANDS.md).
