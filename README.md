# mTrak v0.8.0

A lightweight MIDI tracker for the terminal.

mTrak is modelled on FastTracker 2: the same pattern editor layout, piano keyboard
mapping and navigation keys. Instead of playing samples it sends MIDI, so it can drive
any hardware or software synth.

![mTrak demo: typing a beat, bass and lead with retrig and CC commands, transposing a track, playing the song, giving the lead an arpeggiator while it plays, naming the song in the Info dialog, a tour of the example themes, collapsing the header and the commands list](docs/demo.gif)

## Features

- Windows, Mac and Linux
- Full TUI with total keyboard control for mouseless usage
- No menu diving
- Pattern editor with 8 tracks per pattern, FT2-style keyboard note entry and navigation
- Song sequence with per-pattern repeats
- Song, pattern and block loop playback, live recording into the playing pattern, and a
  metronome
- MIDI output on all 16 channels, with one instrument per channel and instrument configuration
- Midi input for note data, clock sync and start/stop
  (see [docs/SYNC.md](docs/SYNC.md))
- Command column for MIDI and playback commands, using FT2 syntax where applicable
  (see [docs/COMMANDS.md](docs/COMMANDS.md))
- Live MIDI effects on each instrument: arpeggiator, chords, a velocity EQ, an echo, an LFO,
  an envelope, swing, a randomizer, a scale, note to CC and probability, with every setting
  changeable from pattern CC commands
  (see [docs/EFFECTS.md](docs/EFFECTS.md))
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
common ones. Keys can be rebound on that screen or in `keybinds.toml`, see
[docs/CONFIG.md](docs/CONFIG.md#keybindings).

Press `/` to see every pattern command with a short description. The full guide is
[docs/COMMANDS.md](docs/COMMANDS.md), which is also included in every release zip.

### Playing and recording

`Enter` plays the song from the current sequence position, `Ctrl+Space` the current pattern,
and `Ctrl+B` loops the rows of the marked block; press it again to let the pattern play on
past them. Playback always loops; `Space` stops it, and an `EF0` command in the pattern stops
it when it's reached.

`Esc` turns record mode on or off. In record mode, notes you play while the song plays, on the
keyboard or a MIDI controller, are written into the cursor's track on the row closest to when
you played them.

`Ctrl+G` sets up the metronome: when it plays, the channels it clicks on, and the notes and
velocities of the accent and the other beats.

### Effects

Each instrument can have a chain of live MIDI effects. Press `Ctrl+R` for the active
instrument's dialog and choose `Effects` to add them. Each effect's `CC mapping` lets pattern
commands change its settings while the song plays. The guide is
[docs/EFFECTS.md](docs/EFFECTS.md), which is also included in every release zip.

### Options

| Option              | Description                                                                         |
| ------------------- | ----------------------------------------------------------------------------------- |
| `--color <theme>`   | Use this theme instead of the one in `config.toml`: `auto`, `mono`, `ansi16`, `full` or the name of a theme in the `themes` folder. `--theme` works too |
| `--ascii-mode`      | Draw with ASCII characters only                                                     |
| `--no-update-check` | Don't look for a new version this time                                              |

Each overrides its setting in [config.toml](docs/CONFIG.md#configtoml) for one session.
Setting `NO_COLOR` or `TERM=dumb` selects the monochrome theme when the theme is `auto`.

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

Settings and themes are explained in [docs/CONFIG.md](docs/CONFIG.md), which is also
included in every release zip. Press `Ctrl+P` to change them in the Config dialog.

### Project files

Projects saved with v0.6.0 or later open in every later version of mTrak: each project file
records its format version, and when the format changes, newer versions upgrade older projects
as they open them.

Projects saved with v0.5.0 or earlier can't be opened. mTrak shows an error instead; from the
file browser the current project stays open, and from the command line mTrak starts with an
empty project.

To read or write projects from other programs, see [docs/FILE_FORMAT.md](docs/FILE_FORMAT.md).

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
- [X] **Live record**:
    - [X] play notes in while the song is playing, for beatmaking
    - [X] send metronome to channel
- [X] **Live edit**:
    - [X] row lock edit: option to stop the cursor following the playback row, so you can edit a pattern while it plays (handy for loops)
    - [X] block loop: loop the currently marked block
- [X] **Block operations**:
    - [X] select, copy, paste 
    - [X] transpose across rows and tracks
    - [X] clearing blocks
- [x] **Prebuilt binaries** for Linux, Windows and macOS on every release
- [ ] Selectable track counts other than 8
- [ ] Polyphonic note entering (arming multiple tracks so chords can be entered)
- [ ] **More commands**
    - [X] Retrig (`R`, FT2's multi retrig) and retrig note (`E9`)
    - [x] BPM and SPD change (`F`)
    - [X] Note delay (`ED`) and early note delay (`E8`)
    - [X] Note probability (`Y`)
    - [X] Note Cut (`EC`)
    - [X] Legato (overlap notes, `L`)
    - [X] Pitch bend in semitones, using the instrument's bend range (`I`)
    - [ ] Shorthand commands for common CCs
        - [X] Mod wheel (`W`)
        - [ ] filter cutoff, etc.
    - [ ] FT2-style per-tick slides: velocity (`A`), pitch up/down (`1`/`2`) and their fine versions
    - [ ] Tone portamento (`3`), gliding the pitch to the next note
    - [X] Song modification
        - [X] Pattern break (`D`)
        - [X] Jump song position (`B`)
        - [X] Pattern delay (`EE`)
        - [X] Stop playback (`EF`)
        - [X] Set global velocity (`G`)
        - [X] Set track velocity (`C`)
        - [X] Glide global velocity (`H`)
- [ ] **Autosave and crash recovery**
- [x] **Custom themes**
    - [x] theme files, with ports of popular color schemes
    - [x] editing themes in the Config dialog, with a live preview
- [ ] **Keyboard remapping**
    - [x] rebinding keys on the keybindings screen and in `keybinds.toml`
    - [ ] piano key layouts per keyboard (QWERTZ, AZERTY, ...)
- [x] **Proper CLI argument parsing** and a global app config
- [ ] **Instrument Live Midi Effects**
    - [x] Auto arp
    - [x] Midi EQ
    - [x] Chord mode / Harmonizer
    - [x] Delay (Echo)
    - [X] Swing
    - [X] Randomizer
    - [X] Scale mode
    - [X] Note to CC
    - [X] Global Probability
    - [x] CC LFO
    - [ ] Potential custom plugin format scripting method (no VSTs though)
- [ ] ...and more

Have an idea? Feel free to open an issue.

## Contributing

Bug reports, feature requests and pull requests are welcome. See
[docs/CONTRIBUTING.md](docs/CONTRIBUTING.md) for how to get started and
[docs/CONVENTIONS.md](docs/CONVENTIONS.md) for the code style. Run `./fmt.sh` before
committing to format the code and apply lint fixes.

Note that most of the code was very quickly written so there might be some lines that might be confusing or dirty.
I will make a big refactoring PR in the future when my primary roadmap targets have been finished.
