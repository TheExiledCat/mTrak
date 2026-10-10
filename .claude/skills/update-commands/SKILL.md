---
name: update-commands
description: Update docs/COMMANDS.md, the user guide to the pattern command column, so it matches the code. Use after adding, removing or changing a command (MIDI or effect), changing command syntax or value scaling, or changing how commands behave during playback (ticks, muting, resets), or when asked to "update commands" or refresh the commands docs.
---

# Update commands

`docs/COMMANDS.md` documents the pattern command column for users of mTrak. Bring it in line
with the code as it is now. The code is the source of truth; never document behavior you
haven't confirmed in it.

## 1. Read the sources

Read these before touching the doc:

- `src/data/effect.rs`: the command letters (`MIDI_COMMAND_LOOKUP`, plus any effect command
  lookup), the `MidiCommand` and `EffectCommand` enums and their comments, and
  `Command::from_str` / `Display` for the syntax (letter + two hex digits, `0` for none).
- `src/engine/command_handler.rs`: what each command does in `handle_command`, what runs per
  tick in `tick`, what `reset` clears, and value scaling (e.g. `cc_value`).
- `src/engine/engine.rs`: how commands are fed from the pattern: `play_row` (order, muting,
  volume `00` rows), `command_context` / lookahead such as `find_slide_target`,
  `effect_tick`, and where handlers are reset (`start` / `stop`).
- `src/tui/constants.rs` (`EMPTY_NOTE`) for the cell layout, and `enter_command` in
  `src/tui/app.rs` for what entering a command does to the cell.
- `docs/COMMANDS.md` itself.

Use `git diff` / `git log` on those files to see what changed recently, but still check the whole
doc against the code: earlier changes may have been missed.

## 2. Compare and update

For every command letter in the code, check the doc has a section with the right letter, name,
value meaning and behavior, and for every command in the doc, check it still exists. Also check
the shared sections: cell layout, value scaling table, muting, playback reset.

- A command whose handler arm is `todo!()` or otherwise unimplemented must be listed as not
  implemented and say what happens if used (a `todo!()` in the engine stops playback).
- When a command becomes implemented, replace that warning with a real section.
- Keep each command's section in the order of the lookup table, grouped under its kind
  (MIDI commands, effect commands).

## 3. Style

Match the existing doc:

- Written for musicians using the tracker, not for developers. Describe what you hear and see,
  not internal names like `CcSlide` or `TrackState`.
- Each command gets a `### \`X\` Name` heading, one or two sentences on what it does, and a
  short pattern example in cell syntax (`---|1|--|S4A` with a comment) when it helps.
- Value examples use the same hex conventions as the editor (uppercase, two digits) and state
  what is actually sent after scaling.
- Short. Bullets for edge cases, no restating the same rule twice.

## 4. Verify

- Every example cell must parse: letter in the lookup table, two hex digits.
- Every number in a table or example (scaled values, tick counts, CC numbers) must match the
  code; recompute them, don't copy old ones.
- Run `cargo test` if the change touched code, and mention any failures rather than editing
  the doc around them.

Finish with a short summary of what changed in the doc and anything in the code that looked
inconsistent or unfinished (e.g. a command that parses but isn't handled).
