# mTrak

A lightweight MIDI tracker for the terminal.

mTrak is modelled on FastTracker 2: the same pattern editor layout, piano keyboard
mapping and navigation keys. Instead of playing samples it sends MIDI, so it can drive
any hardware or software synth.

![mTrak playing a song](docs/screenshot.png)

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
- Context-aware key hints in the footer
- FT2 color theme on truecolor terminals, with a monochrome fallback

## Installation

### From a release

Download the binary for your platform from the
[releases page](https://github.com/TheExiledCat/mTrak/releases) and put it somewhere on
your `PATH`.

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

## Usage

```sh
mtrak                  # start with an empty project
mtrak song.mtrak       # open a project
```

Pick a MIDI output port with `Ctrl+O`. mTrak makes no sound on its own, so connect it to a physical (or virtual) midi device to make music.

## Roadmap

Planned features, in no particular order:

- [ ] **Undo system**
- [ ] **MIDI file import/export**
- [ ] **MIDI input**
- [ ] **MIDI clock out**: send clock and start/stop to sync external sequencers, drum machines and arpeggiators
- [ ] **Multiple output ports**: map instruments to different MIDI devices, not just channels on one port
- [ ] **Program change and bank select per instrument**, so loading a project restores the right patches
- [ ] **Panic button**: all notes off and reset controllers, for stuck notes
- [ ] **Live record**: play notes in while the song is playing, for beatmaking
- [ ] **Row lock edit**: option to stop the cursor following the playback row, so you can edit a pattern while it plays (handy for loops)
- [ ] **Block operations**: select, copy, paste and transpose across rows and tracks
- [ ] **Swing/groove**, per pattern or global
- [ ] **More commands**
  - [ ] Roll
  - [ ] Roll with volume glide
  - [ ] Tempo (BPM) change
  - [ ] Note delay
  - [ ] Note probability
  - [ ] Shorthand commands for common CCs (mod wheel, filter cutoff, etc.)
- [ ] **Autosave and crash recovery**
- [ ] **Custom themes**
- [ ] **Keyboard remapping**, with default layouts per OS/shell
- [ ] **Proper CLI argument parsing** and a global app config
- [ ] ...and more

Have an idea? Feel free to open an issue.

## Contributing

Bug reports, feature requests and pull requests are welcome. See
[CONTRIBUTING.md](CONTRIBUTING.md).
