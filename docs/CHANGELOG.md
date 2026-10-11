# Changelog

## v0.1.1 - 2026-10-11

### Changed

- Cut track is now `Shift+F6` and cut pattern is `Ctrl+F6` (they were `Shift+F3` and
  `Ctrl+F3`). Copy (`F4`) and paste (`F5`) keys are unchanged.
- Copy, cut and paste now confirm what they did (e.g. `TRCK CUT`, `PTN PASTE`,
  `BLCK COPY`) at the bottom of the Util panel in the header.

### Fixed

- Cutting a track or pattern did nothing. Most terminals send `Shift+F3` and `Ctrl+F3` in a
  form that is read as a cursor position report instead of a key press, so the keys never
  reached mTrak. They've moved to `F6` (see above).

## v0.1.0 - 2026-10-11

### Added

- Copy, cut and paste in the pattern editor, with FT2's keys for tracks and patterns. These
  work in and out of edit mode, and during playback:
  - Track: `Shift+F3` cut, `Shift+F4` copy, `Shift+F5` paste into the track under the cursor.
  - Pattern: `Ctrl+F3` cut, `Ctrl+F4` copy, `Ctrl+F5` paste over the current pattern.
  - Block: press `Alt+B` to drop a mark at the cursor, then move the cursor to stretch a
    highlighted block from the mark. `Alt+X` cuts it, `Alt+C` copies it, and `Alt+V` pastes
    it with its top-left corner at the cursor. Press `Alt+B` again to clear the mark.
- Pasting a pattern into a shorter one asks whether to lengthen the pattern to fit, cut the
  paste off at the end of the pattern, or cancel.
- A keybindings screen: press `?` to see every key in mTrak, grouped by section (pattern
  editor, edit mode, sequence, pattern, channels, instruments, file browser, dialogs). Scroll
  with `↑`/`↓`, `PgUp`/`PgDn` and `Home`/`End`; `Esc` closes it.
- MIDI clock: tick "Send MIDI clock" in the Config dialog (`Ctrl+O`) to send clock (24 pulses
  per quarter note) to your MIDI output while playing, following the song's tempo, including
  tempo commands. Clock messages aren't shown in the MIDI events panel.
- Projects remember their MIDI output. When you open a project, mTrak reconnects to the
  device it was saved with if it's connected, and leaves the output unset if it isn't.

### Changed

- The footer only shows the main keys now: `?` for the keybindings screen, edit, record,
  note off, delete and insert (in edit mode), save and quit. Everything else is on the `?`
  screen.
- Project files made by earlier versions can't be opened, because the file format changed
  (mTrak shows an error and starts an empty project instead). Until v1.0.0 the format may
  change between releases.
- Smaller download: unused dependencies were removed.

### Fixed

- mTrak no longer fails on startup with a missing music folder error on systems where
  `XDG_MUSIC_DIR` isn't set (common on WSL and minimal Linux installs). It now uses
  `~/Music/mtrak`.
- The pattern-length-down key hint read "LEN UP" instead of "LEN DN".

### Known issues

- Projects can only be opened from the command line (`mtrak song.mtrak`), not from inside
  mTrak yet.
- Long error messages are cut off in the warning dialog.

## v0.0.2 - 2026-10-11

### Added

- Saving projects. `Ctrl+S` opens a file browser the first time; after that, or when you
  started mTrak with a project file, it saves straight to the same file.
- Save As with `Alt+S`, which always opens the file browser so you can save a copy somewhere
  else. On macOS this needs your terminal's "Use Option as Meta key" setting.
- File browser: move with `↑`/`↓`, press `Space` to enter a folder or select a file, and
  `Enter` to save. With a file selected, `Enter` overwrites it; otherwise it asks for a name
  and extension and saves a new file in the current folder. `Ctrl+N` creates a folder and
  `Esc` cancels. Only folders and `.mtrak` files are listed.
- mTrak creates a projects folder (`mtrak` in your music folder, e.g. `~/Music/mtrak`) and a
  config folder on startup. See the Config section of the README for the paths on each
  platform.
- Starting mTrak with a project path that doesn't exist yet (`mtrak new-song.mtrak`) starts an
  empty project that `Ctrl+S` saves to that path.
- Errors such as a file that can't be written or a missing MIDI system are shown in a warning
  dialog instead of crashing mTrak.
- Key hints in dialogs and the file browser scroll when they don't fit, like the main footer.
- On macOS, key hints show `⌃` and `⌥` for Control and Option.

### Changed

- `--color 16` is now `--color ansi16`.
- macOS terminals without truecolor support, such as Terminal.app, now get the 16-color theme
  instead of the monochrome one.
- Opening a file that isn't an mTrak project, is damaged, or was made by a newer mTrak now
  shows an error and starts an empty project instead of crashing.

### Fixed

- The "GLB Velocity" key hint was misspelled.
- Building from source works again on Rust 1.85.

### Known issues

- Projects can only be opened from the command line (`mtrak song.mtrak`), not from inside
  mTrak yet.
- Long error messages are cut off in the warning dialog.
