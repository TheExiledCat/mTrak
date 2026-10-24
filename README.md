# mTrak v0.3.0

A lightweight MIDI tracker for the terminal.

mTrak is modelled on FastTracker 2: the same pattern editor layout, piano keyboard
mapping and navigation keys. Instead of playing samples it sends MIDI, so it can drive
any hardware or software synth.

![mTrak demo: playing a song, editing patterns, block copy/paste and transpose, and editing during playback with row lock](docs/demo.gif)

## Features

- Windows, Mac and Linux
- Full TUI with total keyboard control
- Pattern editor with 8 tracks per pattern, FT2-style keyboard note entry and navigation
- Song sequence with per-pattern repeats
- MIDI output on all 16 channels, with one instrument per channel and a per-instrument volume
- Command column for (midi) effects, CC, CC slides, velocity slides, aftertouch and pitch bend
  (see [docs/COMMANDS.md](docs/COMMANDS.md))
- Pattern and song playback, looping or once through for recording
- Per-channel activity meters, channel muting and a MIDI event monitor
- Built-in file browser for opening and saving projects, with Save As and folder creation
- Context-aware key hints in the footer, dialogs and file browser
- FT2 color theme on truecolor terminals, a 16-color theme on Windows consoles and macOS
  terminals without truecolor, and a monochrome fallback
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

| Option                         | Description                                                    |
| ------------------------------ | -------------------------------------------------------------- |
| `--color <mono\|ansi16\|full>` | Force a color theme instead of detecting one from the terminal |
| `--ascii-mode`                 | Draw with ASCII characters only                                |

Setting `NO_COLOR` or `TERM=dumb` also selects the monochrome theme.

## Config

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
    - [ ] Change sequence/pattern using Program Select In
- [X] **MIDI clock out**: send clock and start/stop to sync external sequencers, drum machines and arpeggiators
- [ ] **Multiple output ports**: map instruments to different MIDI devices, not just channels on one port
- [ ] **Program change and bank select per instrument**, so loading a project restores the right patches
- [ ] **Panic button**: all notes off and reset controllers, for stuck notes
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
    - [X] Retrig
    - [X] Retrig with volume change
    - [x] Tempo (BPM) and speed change (`F`)
    - [X] Note delay
    - [X] Note probability
    - [X] Note Cut
    - [X] Portamento (overlap notes)
    - [ ] Shorthand commands for common CCs (mod wheel, filter cutoff, etc.)
    - [ ] Song modification
        - [ ] Pattern break
        - [ ] Jump song position
        - [ ] Pattern delay
        - [ ] Set global velocity
        - [ ] Set track velocity
        - [ ] Glide global velocity
- [ ] **Autosave and crash recovery**
- [ ] **Custom themes**
- [ ] **Keyboard remapping**, with default layouts per OS/shell
- [ ] **Proper CLI argument parsing** and a global app config
- [ ] ...and more

Have an idea? Feel free to open an issue.

## Contributing

Bug reports, feature requests and pull requests are welcome. See
[docs/CONTRIBUTING.md](docs/CONTRIBUTING.md) for how to get started and
[docs/CONVENTIONS.md](docs/CONVENTIONS.md) for the code style. Run `./fmt.sh` before
committing to format the code and apply lint fixes.
