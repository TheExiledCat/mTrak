# mTrak v0.4.0

A lightweight MIDI tracker for the terminal.

mTrak is modelled on FastTracker 2: the same pattern editor layout, piano keyboard
mapping and navigation keys. Instead of playing samples it sends MIDI, so it can drive
any hardware or software synth.

![mTrak demo: typing a beat, bass and lead with retrig and CC commands, transposing a track, playing the song, collapsing the header, switching themes from the Config dialog and the commands list](docs/demo.gif)

## Features

- Windows, Mac and Linux
- Full TUI with total keyboard control for mouseless usage
- No menu diving
- Pattern editor with 8 tracks per pattern, FT2-style keyboard note entry and navigation
- Song sequence with per-pattern repeats
- MIDI output on all 16 channels, with one instrument per channel and instrument configuration
- Midi input for note data, clock sync and start/stop
  (see [docs/SYNC.md](docs/SYNC.md))
- Command column for (midi) effects, using FT2 syntax where applicable
  (see [docs/COMMANDS.md](docs/COMMANDS.md))
- Per-channel activity meters, channel muting and a MIDI event monitor
- Built-in file browser for opening and saving projects
- Context-aware key hints and a built in shortcuts screen
- FT2 color theme on truecolor terminals, a 16-color theme on Windows consoles and macOS
  terminals without truecolor, a monochrome fallback, and custom colors from a theme file
- ASCII mode for terminals or fonts without Unicode box drawing and symbols

## Installation

### From a release

Download the binary for your platform from the
[releases page](https://github.com/TheExiledCat/mTrak/releases) and put it somewhere on
your `PATH` or run it directly.

### From source

You need a recent stable Rust toolchain (install it with [rustup](https://rustup.rs)).
On Linux you also need the ALSA development headers, e.g. `libasound2-dev` on
Debian/Ubuntu or `alsa-lib-devel` on Fedora.

```sh
cargo install --git https://github.com/TheExiledCat/mTrak
```

Or from a local clone:

```sh
git clone https://github.com/TheExiledCat/mTrak
cd mTrak
cargo install --path .
```

**Note**: v0.1.3 to v0.1.5 could be flagged by some antivirus software (especially Windows
Defender) as a false positive because of how the self updater replaced the executable. v0.1.6
changes how updates are installed on Windows to avoid this, but the release binaries are not
signed yet, so warnings can still show up. Once the project gets enough adoption, I will apply
for signing credentials. Until then, building from source with `cargo install` avoids the
issue.

## Usage

```sh
mtrak                  # start with an empty project
mtrak song.mtrak       # open a project
```

Pick a MIDI output port with `Ctrl+O`. mTrak makes no sound on its own, so connect it to a physical (or virtual) midi device to make music.
You can also open projects from the built-in file browser with `Alt+O`.

### Keybindings

Press `?` to see every key in mTrak, grouped by section. The footer only shows the most
common ones.

Press `/` to see every pattern command with a short description. The full guide is
[docs/COMMANDS.md](docs/COMMANDS.md), which is also included in every release zip.

### Options

| Option                                | Description                                                         |
| ------------------------------------- | ------------------------------------------------------------------- |
| `--color <mono\|ansi16\|full\|custom>` | Use this theme instead of the one in `config.toml` or the detected one |
| `--ascii-mode`                        | Draw with ASCII characters only                                     |

Setting `NO_COLOR` or `TERM=dumb` also selects the monochrome theme.

## Configuration

### Folders

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

The config folder holds two files, `config.toml` and `theme.toml`. Both are written with
their defaults when they're missing, so deleting one resets it. They use a small part of TOML:
one `key = "value"` per line, with `#` starting a comment. An error in either file is shown
when mTrak starts, with the line it's on, and the defaults are used instead.

### config.toml

Settings that aren't part of a project.

| Setting | Values                                         | Default  | Description          |
| ------- | ---------------------------------------------- | -------- | -------------------- |
| `theme` | `auto`, `mono`, `ansi16`, `full` or `custom`   | `auto`   | The theme to open with |

The themes:

| Theme    | Description                                                                      |
| -------- | -------------------------------------------------------------------------------- |
| `auto`   | Picks one from what the terminal supports: `full`, `ansi16` or `mono`            |
| `mono`   | No colors, only bold, dim, underline and reverse                                 |
| `ansi16` | The 16 standard terminal colors, so it follows your terminal's color scheme      |
| `full`   | The FT2 theme, for terminals with 24-bit color                                   |
| `custom` | The colors in `theme.toml`                                                       |

`Ctrl+P` opens the Config dialog, which changes the theme straight away. Tick
`Save to config.toml` to keep it for next time, or untick it to try a theme for this session
only. `--color` overrides `theme` for one session, and `NO_COLOR` or `TERM=dumb` select `mono`
when `theme` is `auto`.

### theme.toml

The colors of the `custom` theme. Every part of mTrak is drawn with these ten colors:

| Color          | Used for                                                                    |
| -------------- | --------------------------------------------------------------------------- |
| `backdrop`     | The background around the panels, the cursor row bar and subtle borders     |
| `muted`        | Toned-down text and borders, and the fill behind the selection              |
| `disabled`     | Text and key fills for something unavailable, e.g. undo with nothing to undo |
| `surface`      | The pattern editor's background, and the text on filled labels             |
| `surface_text` | Text in the pattern editor                                                  |
| `text`         | Text on the backdrop, and the fill under the cursor                         |
| `danger`       | Errors, edit mode and recording                                             |
| `warning`      | Warnings, and the playing row while the row is locked                       |
| `success`      | Success, e.g. the About dialog's border                                     |
| `mark`         | The fill behind a marked block                                              |

Colors are hex (`"#49688C"`) or ANSI color names (`"Red"`, `"DarkGray"`, `"LightBlue"`, ...).
Colors you leave out come from the palette named by `base`, `"ft2"` (the default) or
`"ansi16"`, so a theme can change just a few:

```toml
base = "ansi16"
danger = "#FF5555"
mark = "#44475A"
```

The [example_themes](example_themes) folder has ports of Catppuccin, Dracula, Gruvbox, Nord,
One Dark, Solarized and Tokyo Night. Copy one to `theme.toml` and pick `custom`.

### Project files

**Note:** the project file format is still changing between releases. Until v1.0.0, mTrak
can only open projects saved with the same version of mTrak; opening a project from another
version shows an error and starts an empty project instead. From v1.0.0 the format will be
stable and newer versions will open older projects.

## Roadmap

Planned features, in no particular order:

- [x] **File browser and global saving**
- [X] **Undo system**
- [ ] **MIDI file import/export**
- [ ] **MIDI input**:
    - [X] play notes into the pattern using a connected midi controller
    - [X] trigger mtrak to start and stop playback, sync to midi controller
    - [X] follow an external MIDI clock, with start, stop, continue and song position
    - [X] separate ports for notes and clock
    - [ ] Change sequence/pattern using Program Select In
- [X] **MIDI clock out**: send clock and start/stop to sync external sequencers, drum machines and arpeggiators
- [ ] **Multiple output ports**: map instruments to different MIDI devices, not just channels on one port
- [ ] **Program change and bank select per instrument**, so loading a project restores the right patches
- [X] **Panic button** (`~`): all notes off and reset controllers, for stuck notes
- [ ] **Live record**:
    - [ ] play notes in while the song is playing, for beatmaking
    - [ ] send metronome to channel
- [ ] **Live edit**:
    - [X] row lock edit: option to stop the cursor following the playback row, so you can edit a pattern while it plays (handy for loops)
    - [ ] block loop: loop the currently marked block
- [X] **Block operations**:
    - [X] select, copy, paste 
    - [X] transpose across rows and tracks
    - [X] clearing blocks
- [ ] **Swing/groove**, per pattern or global
- [x] **Prebuilt binaries** for Linux, Windows and macOS on every release
- [ ] **More commands**
    - [X] Retrig (`R`, FT2's multi retrig) and retrig note (`E9`)
    - [x] BPM and SPD change (`F`)
    - [X] Note delay (`ED`) and early note delay (`E8`)
    - [X] Note probability (`Y`)
    - [X] Note Cut (`EC`)
    - [X] Legato (overlap notes, `L`)
    - [ ] Shorthand commands for common CCs
        - [X] Mod wheel (`W`)
        - [ ] filter cutoff, etc.
    - [ ] FT2-style per-tick slides: velocity (`A`), pitch up/down (`1`/`2`) and their fine versions
    - [ ] Tone portamento (`3`), gliding the pitch to the next note
    - [ ] Song modification
        - [ ] Pattern break
        - [ ] Jump song position
        - [ ] Pattern delay
        - [ ] Set global velocity
        - [ ] Set track velocity
        - [ ] Glide global velocity
- [ ] **Autosave and crash recovery**
- [x] **Custom themes**
- [ ] **Keyboard remapping**, with default layouts per OS/shell
- [ ] **Proper CLI argument parsing** and a global app config
- [ ] ...and more

Have an idea? Feel free to open an issue.

## Contributing

Bug reports, feature requests and pull requests are welcome. See
[docs/CONTRIBUTING.md](docs/CONTRIBUTING.md) for how to get started and
[docs/CONVENTIONS.md](docs/CONVENTIONS.md) for the code style. Run `./fmt.sh` before
committing to format the code and apply lint fixes.
