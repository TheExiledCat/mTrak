# Changelog

## v0.4.0 - 2026-10-25

### Added

- **MIDI clock sync:** pick a port as `Clock SRC` in the Config dialog (`Ctrl+O`) to follow
  another device's clock. mTrak plays one tick per clock pulse and follows its Start, Stop,
  Continue and song position, and the header shows `SYN` with the measured tempo instead of
  `BPM`. If the clock goes quiet for two beats, playback stops. Your own play and stop keys
  still work while synced, at the last measured tempo. Notes and clock can come in on the same
  port or on two different ones. Details in [docs/SYNC.md](docs/SYNC.md).
- **Panic button (`~`):** stops playback and sends all notes off, all sound off and reset
  controllers on every channel, at any time.
- **Transpose:** a global transpose in the Global dialog (`Ctrl+E`) and a transpose per
  instrument in the Instrument dialog, from `-80` to `+7F` semitones. The status line shows
  the global one as `GLB TRSP`. Notes transposed outside MIDI's range don't play.
- **Default CC per instrument:** set in the Instrument dialog. CC commands on a track without
  an `S` change the instrument's default CC instead of CC 0.
- **New commands** (see [docs/COMMANDS.md](docs/COMMANDS.md)):
  - `Wxx` Mod wheel: sends CC 1 and selects it, so a following `V` glides the mod wheel.
  - `E9x` Retrig note: replays the note every `x` ticks within its row, like FT2's `E9x`.
- **Jumping during playback, as in FT2:** while a pattern plays, changing the pattern carries
  playback on in the new one from the next row. While the song plays, moving through the
  sequence list jumps playback to that position.
- **Number fields** in dialogs (velocity, volume, transpose, default CC, bend range): type hex
  digits over the value or step it with Up and Down. Arrows next to it show which way it can
  still move.
- **About dialog** (`Ctrl+T`).
- Typing a volume on a note replays the note at the new velocity, so you can hear it.
- Pasting a pattern that's shorter than the current one asks whether to trim the pattern to the
  copy's length or keep its length.

### Changed

- **Projects saved with v0.3.0 can't be opened in v0.4.0.**
- **Command letters follow FastTracker 2** where FT2 has the same command, and move off
  letters FT2 uses for something else:

  | Command               | v0.3.0       | v0.4.0       |
  |-----------------------|--------------|--------------|
  | Change CC             | `C`          | `M`          |
  | CC slide              | `G`          | `V`          |
  | CC and velocity slide | `H`          | `J`          |
  | Velocity slide        | `V`          | `U`          |
  | End slide             | `E`          | `N`          |
  | Aftertouch            | `A`          | `T`          |
  | Pitch bend            | `W`          | `P`          |
  | Note delay            | `D1x`        | `EDx`        |
  | Early note delay      | `D0x`        | `E8x`        |
  | Note cut              | `33x`        | `ECx`        |
  | Legato                | `30x`, `31x` | `L0x`, `L1x` |

- **`R` is now Retrig and works like FT2's multi retrig:** the interval carries on across
  rows, an `x` or `y` of `0` reuses the track's last value, a volume on the cell plays every
  repeat at that volume, and without a note it repeats the track's last note.
- **The speed sets how many rows play per beat**, as in FT2: a tick is one MIDI clock pulse,
  so `F03` or SPD 3 plays twice as fast. The speed is now one setting for the whole song (`SPD`
  in the header) instead of per pattern; use `Fxx` to change it partway through.
- Moving through the sequence list switches the editor to that position's pattern, and adding
  a position (`Ctrl+N`) copies the selected position's pattern.
- Dialogs open with nothing focused; Tab goes through the fields and back to none. Text fields
  and the Keybindings and Commands screens still open ready to use.
- Dropdowns in dialogs show only the chosen option until focused, then open their list on top.
  The focused field's label is highlighted.
- Clear and Shrink also work during playback.
- The GLB Velocity dialog is now called Global.

## v0.3.0 - 2026-10-19

### Added

Five new effect commands. Several of them work in ticks: each row is split into as many ticks
as the speed (6 by default). Full details and examples are in
[docs/COMMANDS.md](docs/COMMANDS.md).

- **`Y` Probability:** the note plays `xx` times out of 256, so `Y80` plays about half the
  time. `Y00` never plays and `YFF` always does. A note off with `Y` is also only sent when
  the roll succeeds.
- **`D` Note delay:** moves the note off its row.
  - `D1y` starts it `y` ticks into its row, e.g. `C-4|1|--|D13` starts halfway through the
    row at speed 6.
  - `D0y` starts it `y` ticks *before* its row, during the row before.
  - Note offs can be moved too.
- **`R` Roll:** `Rxy` plays the note, then again every `y` ticks until the row ends. `x`
  changes the velocity of each repeat, using FastTracker 2's multi retrig table (e.g. `R92`
  gets louder by 2 per hit, `R52` quieter by 32).
- **`3` Legato:** glides into the note from the track's previous note. The new note starts
  before the old one is released, so a synth in mono/legato mode with portamento switched on
  glides between them.
  - `30y` starts the note `y` ticks before its row.
  - `31y` starts it on its row and keeps the previous note sounding `y` more ticks.
- **`33y` Note cut:** releases the track's note on tick `y` of the row. `330` releases it
  straight away. On a cell with no note, it cuts the note that's already playing.

### Changed

- The commands screen (`/`) lists each form of a command on its own row, e.g. `F00` and `Fxx`,
  or `30y`, `31y` and `33y`.
- Projects saved with v0.2.0 open in v0.3.0; the project file format didn't change.

## v0.2.0 - 2026-10-19

### Added

- Commands screen: press `/` to see every pattern command with a one line description of
  what it does. `/` is shown in the footer next to `?`. For examples and the details of
  slides, see [docs/COMMANDS.md](docs/COMMANDS.md).
- Clear part of a pattern: press `Alt+Z` to open the Clear dialog. Pick what to clear:
  - **Selection:** `Block` (the marked block, or the cell under the cursor if nothing is
    marked), `Track` (the current track) or `Pattern` (the whole pattern).
  - **Target:** `All`, `Note` (the note and its instrument), `Vel` (the volume column) or
    `Cmd` (the command column).

  Clearing can be undone with `Ctrl+Z`. It only works while playback is stopped; during
  playback mTrak shows "CANT CLR".
- Release downloads now include `COMMANDS.md` next to the mTrak program.

### Changed

- Changing the pattern at a song position (`Alt+,` / `Alt+.`) now also shows that pattern
  in the pattern editor.
- The keys in the list at the top right are lined up in one column.
- Projects saved with v0.1.9 open in v0.2.0; the project file format didn't change.

## v0.1.9 - 2026-10-15

### Added

- Open projects from inside mTrak: press `Alt+O` to open the file browser on your projects
  folder (or the folder of the current project), pick a `.mtrak` file and press `Enter`.
  You no longer have to start mTrak from the command line to open a project.
  - `Enter` opens the highlighted file, or the file you selected with `Space`.
  - If the current project has unsaved changes, mTrak asks before discarding them.
  - Playback stops and the project opens fresh, the same as starting mTrak with it: the
    cursor goes back to the start of the song, undo history is cleared, and the octave, line
    add, active instrument and copy/paste clipboard go back to their defaults.
  - The MIDI input and output saved in the project are reconnected.
  - If the file can't be opened (for example a project saved with another version of
    mTrak), an error is shown and the current project stays open.

### Changed

- Projects saved with v0.1.8 open in v0.1.9; the project file format didn't change.

## v0.1.8 - 2026-10-14

### Added

- MIDI input: connect a MIDI keyboard or controller and play notes with it, like with the
  computer keyboard. Pick the device under **MIDI input** in the Config dialog; mTrak
  remembers it in the project and reconnects when you open the project again.
  - In edit mode, each note you play is written on the cursor row with the active
    instrument, and the cursor moves down by the line add. Outside edit mode, notes are
    just played.
  - Notes keep sounding for as long as you hold the key and stop when you let go.
  - **Record Velocity** (in the Config dialog) writes how hard you played each note into the
    volume column. With it off, the volume column is left as it is and notes play at full
    velocity.
  - Starting and stopping playback from a controller isn't supported yet.
- The MIDI input and MIDI output lists in the Config dialog both start with a **(none)**
  entry, so you can disconnect a port without unplugging anything.

### Changed

- **Projects saved with v0.1.7 or earlier can't be opened.** The project file format changed
  to store the MIDI input settings; until v1.0.0 mTrak only opens projects saved with the
  same version.
- Transposing a single note now plays it back at its own volume instead of always at full
  velocity. A note with volume `00` stays silent.

## v0.1.7 - 2026-10-13

### Added

- Undo and redo: `Ctrl+Z` undoes the last edit and `Ctrl+Y` redoes it. This covers everything
  you do in the pattern editor (entering notes, instruments, volumes and effects, note offs,
  inserting and deleting rows or events, cutting and pasting tracks, patterns and blocks,
  transposing), changing a pattern's speed or length, shrinking and expanding patterns,
  editing the sequence and changing the BPM. Up to 256 steps are kept.
  - Undo and redo take you to what changed: the pattern editor switches to the edited
    pattern and moves the cursor to the edited cell. After a block edit, the cursor is only
    moved if it's outside the edited area. While the editor follows playback, the edit is
    still undone but the cursor stays with the playing row.
  - Undo and Redo show in the key hints at the bottom of the screen, greyed out when there's
    nothing to undo or redo.
  - Undoing back to the point where you last saved counts as saved again, so quitting won't
    warn about unsaved progress.
- Row lock: `Ctrl+L` stops the cursor following the playing row, so you can edit a pattern
  while it plays, which is handy for loops. Turning it on also turns on edit mode, and the
  top of the pattern editor shows `-- LOCK --`. When the playing pattern is the one on screen,
  the playing row is highlighted separately from the cursor row. During row lock, `Space`
  toggles edit mode instead of stopping playback, and `Esc` stops playback.

### Changed

- **Projects saved with v0.1.6 or earlier can't be opened.** The project file format changed;
  until v1.0.0 mTrak only opens projects saved with the same version.
- Shrinking a pattern no longer asks for confirmation, since it can now be undone.
- If you installed mTrak with `cargo install`, the package is now called `mtrak` instead of
  `m_trak`. Run `cargo uninstall m_trak` before installing this version, otherwise cargo
  refuses to install because the `mtrak` binary already exists.

## v0.1.6 - 2026-10-13

### Changed

- Updating on Windows works differently, to make antivirus software less likely to flag
  mTrak as a trojan. The new version is now written straight into the folder mTrak lives in:
  the old `mtrak.exe` is renamed to `mtrak.old.exe` and removed the next time you start mTrak.
  Nothing is written to the temp folder and no helper process is started anymore.
- mTrak is now released under the MIT license.

## v0.1.5 - 2026-10-12

### Added

- The bottom-right corner of the screen shows which version of mTrak you're running, e.g.
  `mTrak v0.1.5`, next to the key hints.

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
