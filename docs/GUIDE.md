# Using mTrak

How to get going, play and record, and where mTrak keeps its files. The pattern commands,
the instrument effects and the settings each have a guide of their own: [COMMANDS.md](COMMANDS.md),
[EFFECTS.md](EFFECTS.md) and [CONFIG.md](CONFIG.md).

## Starting

```sh
mtrak                  # start with an empty project
mtrak song.mtrak       # open a project
```

Pick a MIDI output port with `Ctrl+O`. mTrak makes no sound of its own, so connect it to a
hardware or software synth to hear anything. `Ctrl+S` saves the project, `Alt+S` saves it under
a new name, and `Alt+O` opens one from the built-in file browser.

## Keys

Press `?` to see every key in mTrak, grouped by section. The footer only shows the most
common ones. Keys can be rebound on that screen or in `keybinds.toml`, see
[CONFIG.md](CONFIG.md#keybindings). If your terminal keeps some keys for itself, the
[keybinds](https://github.com/TheExiledCat/mTrak/tree/main/keybinds) folder has premade
overrides for Windows Terminal, GNOME Terminal, Konsole, macOS Terminal, tmux and VS Code.

Press `/` to see every pattern command with a short description. The full guide is
[COMMANDS.md](COMMANDS.md), which is also included in every release zip.

## Editing

`Space` turns edit mode on and off. In edit mode the piano keys enter notes on the track under
the cursor, and hex digits type into the instrument, volume and command columns. `Alt+B` marks
a block to copy (`Alt+C`), cut (`Alt+X`), paste (`Alt+V`) or transpose, and `Alt+F` fills the
volumes, or the command values on the command column, in a straight line between the ones
above and below the cursor. `Ctrl+Z` undoes and `Ctrl+Y` redoes.

## Playing and recording

`Enter` plays the song from the current sequence position, `Ctrl+Space` the current pattern,
and `Ctrl+B` loops the rows of the marked block; press it again to let the pattern play on
past them. Playback always loops; `Space` stops it, and an `EF0` command in the pattern stops
it when it's reached.

`Esc` turns record mode on or off. In record mode, notes you play while the song plays, on the
keyboard or a MIDI controller, are written into the cursor's track on the row closest to when
you played them.

`Ctrl+G` sets up the metronome: when it plays, the channels it clicks on, and the notes and
velocities of the accent and the other beats.

## Instruments

`Ctrl+R` opens the active instrument's dialog: its name, volume, bend range, default CC and
transpose, and the sound it picks on the synth. `Program`, `Bank MSB` and `Bank LSB` are sent
as a program change and bank select (CC 0 and 32) when they change, when the MIDI output
connects and every time playback starts, so a project always plays on the right patches.
Leave one at `Off` to not send it.

## Effects

Each instrument can have a chain of live MIDI effects. Press `Ctrl+R` for the active
instrument's dialog and choose `Effects` to add them. Each effect's `CC mapping` lets pattern
commands change its settings while the song plays. The guide is
[EFFECTS.md](EFFECTS.md), which is also included in every release zip.

## Exporting to MIDI

`Alt+M` exports the song as a MIDI file (`.mid`), to take it into a DAW. It's the song as it
plays from its first sequence position, with every command, effect, transpose and volume
applied, and ends where the song length in the Info dialog (`Ctrl+F`) does. The file has a
track per instrument that plays something, starting with its bank and program, and the
song's tempo changes.

## Command-line options

| Option              | Description                                                                         |
| ------------------- | ----------------------------------------------------------------------------------- |
| `--color <theme>`   | Use this theme instead of the one in `config.toml`: `auto`, `mono`, `ansi16`, `full` or the name of a theme in the `themes` folder. `--theme` works too |
| `--ascii-mode`      | Draw with ASCII characters only                                                     |
| `--no-update-check` | Don't look for a new version this time                                              |

Each overrides its setting in [config.toml](CONFIG.md#configtoml) for one session.
Setting `NO_COLOR` or `TERM=dumb` selects the monochrome theme when the theme is `auto`.

## Folders

Projects are saved to an `mtrak` folder in your music folder by default, but you can save
and open them from any folder. Global settings live in an `mtrak` folder in your config
folder. Both are created on startup if they don't exist yet.

| Platform | Projects                    | Config                                |
| -------- | --------------------------- | ------------------------------------- |
| Linux    | `~/Music/mtrak`             | `~/.config/mtrak`                     |
| macOS    | `~/Music/mtrak`             | `~/Library/Application Support/mtrak` |
| Windows  | `%USERPROFILE%\Music\mtrak` | `%LOCALAPPDATA%\mtrak`                |

On Linux the music folder follows `XDG_MUSIC_DIR` and the config folder follows
`XDG_CONFIG_HOME`, if they're set.

Settings and themes are explained in [CONFIG.md](CONFIG.md), which is also
included in every release zip. Press `Ctrl+P` to change them in the Config dialog.

## Project files

Projects saved with v0.6.0 or later open in every later version of mTrak: each project file
records its format version, and when the format changes, newer versions upgrade older projects
as they open them.

Projects saved with v0.5.0 or earlier can't be opened. mTrak shows an error instead; from the
file browser the current project stays open, and from the command line mTrak starts with an
empty project.

To read or write projects from other programs, see [FILE_FORMAT.md](FILE_FORMAT.md).
