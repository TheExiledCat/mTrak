# Changelog

## v0.1.4 - 2026-10-12

### Changed

- The download is about 3.5 MB smaller: the Linux binary is now around 1.6 MB instead of
  5.2 MB. The updater was rewritten to use far fewer libraries and the system's own TLS
  (OpenSSL 3 on Linux, the built-in TLS on macOS and Windows).
- The update download now shows a simple `Downloading...  42%` line.

### Fixed

- On macOS, updating could fail or write the new binary to the wrong place when mTrak was
  started through a relative symlink. The updater now always replaces the real binary behind
  any symlinks and leaves the links alone.

## v0.1.3 - 2026-10-12

### Added

- Automatic updates. When mTrak starts it checks GitHub for a newer release and, if there is
  one for your platform, asks `A new version of mTrak is available: v0.1.3 -> vX.Y.Z. Update
  now? [y/n]`. Answering `y` downloads the new version and replaces the installed `mtrak`;
  start mTrak again afterwards to use it. Answering `n` starts mTrak as usual.
  - If you're offline or the check fails, mTrak just starts.
  - The check is skipped when mTrak isn't run from a terminal.
  - Start with `--no-update-check` to skip it altogether.
  - This release is the first with the updater, so updating to it from an earlier version
    still has to be done by hand.

### Changed

- A sequence entry can now repeat its pattern at most 8 times.

## v0.1.2 - 2026-10-12

### Added

- Shrink and expand patterns, as in FT2. `Alt+-` (SRNK) halves the current pattern by
  removing every odd row, and `Alt+=` (EXPD) doubles it by inserting an empty row after every
  row. Both are shown as buttons in the pattern panel of the header.
  - Shrinking can't be undone, so it asks for confirmation first, and it only works while
    playback is stopped.
  - Expanding works on patterns up to `80` rows long; a `80`-row pattern expands to the full
    `100` rows.
- Transpose notes up or down a semitone, for the current instrument or for all instruments:

  | Area    | Current instrument (down / up) | All instruments (down / up)   |
  |---------|--------------------------------|-------------------------------|
  | Track   | `Alt+G` / `Alt+H`              | `Alt+Shift+G` / `Alt+Shift+H` |
  | Pattern | `Alt+J` / `Alt+K`              | `Alt+Shift+J` / `Alt+Shift+K` |
  | Block   | `Alt+L` / `Alt+;`              | `Alt+Shift+L` / `Alt+:`       |

  With no block marked, the block keys transpose just the note under the cursor and play
  it, so you can tune a single note by ear. Notes stop at `C-0` and `G-9`.
- A play timer under the sequence list shows how long playback has been running
  (`hh:mm:ss`), as in FT2.
- Unsaved work protection: the Save key hint shows `*Save` while the project has unsaved
  changes, and quitting with unsaved changes asks for confirmation first.

### Changed

- Patterns can now be up to `100` (256) rows long, numbered `00`–`FF` like in FT2. Before,
  the maximum was `FF` (255) rows.
- Notifications at the bottom of the Util panel (like `PTN COPY` or `SAVED`) now disappear
  after 5 seconds.
- The instrument list only scrolls when it doesn't fit in the window, and then follows the
  selected instrument. On small terminals it used to show only the first instruments.

### Known issues

- Projects saved with an earlier version that contain a pattern of `FB`–`FF` (251–255) rows
  can't be opened. Projects with shorter patterns open normally.

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
