# Changelog

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
