---
name: update-file-format
description: Update docs/FILE_FORMAT.md, the description of the .mtrak project file for other programs, so it matches the code, and archive the description of the previous file version when the version goes up. Use after changing anything that's saved in a project (fields of Project, Instrument, SequenceRow, the saved pattern or the cell bytes), after raising PROJECT_VERSION or adding a migration, or when asked to "update the file format" docs.
---

# Update file format

`docs/FILE_FORMAT.md` describes the `.mtrak` project file for people writing other programs
that read or write it. Bring it in line with the code as it is now. The code is the source of
truth; never document a field or value you haven't confirmed in it.

Each file version has its own description. When the version goes up, the description of the
old version is kept in `docs/archive/` so files of that version can still be read by others.

## 1. Read the sources

Read these before touching the doc:

- `src/data/project.rs`: the header (`PROJECT_HEADER`, `VERSION_END`, `LENGTH_END`), `encode`
  and `decode`, and the `Project` and `SequenceRow` structs. Every field without
  `#[serde(skip)]` is saved, under its Rust name unless a `#[serde(rename)]` says otherwise.
- `src/data/migration.rs`: `PROJECT_VERSION`, `FIRST_MESSAGEPACK_VERSION` and `MIGRATIONS`,
  for the current version and which older ones can be opened.
- `src/data/pattern.rs`: `SavedPattern` (how a pattern is saved), its `TryFrom` checks (what
  loading accepts), and `PatternStore`.
- `src/data/note.rs`: the `CELL_*` constants, `write_cell` and `read_cell` for the cell bytes,
  and `NotePitch::MIN` / `MAX`.
- `src/data/instrument.rs`, and `src/data/effect.rs` for the saved effects (`EffectKind`,
  `EffectSettings`). Each effect's parameters, defaults and value ranges are in its file in
  `src/engine/effects/` (e.g. `PARAMS` in `eq.rs`).
- `src/data/commands.rs` for the command letters and `CommandKind::letter`.
- Ranges the doc gives: check them in the code that enforces them (e.g. `VOLUME_MAX` and
  `MIDI_CHANNEL_MAX` in `src/data/midi.rs`, `MAX_ROWS`, `TRACK_COUNT`).
- `docs/FILE_FORMAT.md` itself, and the archived versions in `docs/archive/`.

Use `git diff` / `git log` on those files to see what changed recently, but still check the
whole doc against the code: earlier changes may have been missed.

## 2. Archive the previous version if the version went up

The doc's intro says which file version it describes ("It describes file version N"). If
`PROJECT_VERSION` is higher than that N:

1. Copy `docs/FILE_FORMAT.md` as it is to `docs/archive/FILE_FORMAT_vN.md`, before changing
   anything. Don't edit its description of the format: it documents files that exist.
2. At the top of the copy, under the heading, add one line saying it's archived and which
   mTrak releases wrote it, e.g. `Archived: file version 3, written by mTrak v0.6.0 to
   v0.7.2. The current format is described in [FILE_FORMAT.md](../FILE_FORMAT.md).` Find the
   last release that wrote it with `git tag` and the history of `PROJECT_VERSION`.
3. Fix the copy's relative links for its new folder (`COMMANDS.md` becomes
   `../COMMANDS.md`).
4. In `docs/FILE_FORMAT.md`, list the archived version under `## Older versions` at the end,
   newest first, with a link and the releases that wrote it.

If an archive for that version already exists, don't overwrite it; ask what to do. Versions
1 and 2 (bincode, v0.5.0 and earlier) were never documented; don't backfill them.

## 3. Compare and update

Check every part of the doc against the code, in both directions: every saved field is in
the doc, and everything in the doc is still saved.

- **Intro:** the file version it describes, and the first mTrak release that writes it. For
  a version that isn't released yet, that's the next release: the version in `Cargo.toml` if
  no `vX.Y.Z` tag exists for it yet, otherwise ask.
- **Header:** byte offsets, sizes and endianness, and which file versions mTrak can open.
- **Payload, sequence, instruments, patterns:** one table row per field, with its saved name,
  MessagePack type (int, string, bool, nil, array, map, binary) and what it holds, in the
  order the struct declares them, which is the order they're saved in. Say when a value can be
  nil or negative. `patterns` must stay the last field of the payload (the doc promises it and
  `patterns_are_the_last_field` tests it); if a new field was declared after it, point that
  out.
- **Cells:** the flag bits and the field order, the values each field can hold and what a
  missing field means. Recompute every example's bytes from `write_cell`; the tests in
  `src/data/note.rs` show some cells and their bytes.

A change to the saved fields of an already released file version breaks files that exist.
Compare the saved structs with the last release tag (`git diff vX.Y.Z -- src/data`); if they
differ while `PROJECT_VERSION` is the same as in that tag, tell the user it needs a version
bump and a migration. Don't make that change yourself, and don't document the new shape as
the old version.

## 4. Style

Match the existing doc:

- Written for developers of other programs: byte layouts, types and ranges, not mTrak's
  internal type names.
- Tables for fields and bytes, hex for byte values (`0x80`, `9F 3C`), and a short example
  where it helps.
- Short. No restating the same rule twice.

## 5. Verify

- Every field name in the doc must be the exact name that's saved.
- Every number (offsets, ranges, flag bits, example bytes) must match the code; recompute
  them, don't copy old ones.
- Every link must resolve, including those in archived copies.
- Run `cargo test` if the change touched code, and mention any failures rather than editing
  the doc around them. A format change also needs the test projects updated: run the
  `update-test-projects` skill.

Finish with a short summary of what changed in the doc, whether a version was archived, and
anything in the code that looked inconsistent, such as a saved field changed without a
version bump.
