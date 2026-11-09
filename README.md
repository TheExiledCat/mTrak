# mTrak v0.9.0

A lightweight MIDI tracker for the terminal.

mTrak is modelled on FastTracker 2: the same pattern editor layout, piano keyboard
mapping and navigation keys. Instead of playing samples it sends MIDI, so it can drive
any hardware or software synth.

Join us on the TrackerCatz Discord server.
[![Discord](https://img.shields.io/badge/TrackerCatz_Discord-Join%20Server-5865F2?logo=discord&logoColor=white)](https://discord.gg/cjAQR9xB5q)

![mTrak demo: typing a beat, bass and lead with retrig and CC commands, transposing a track, playing the song, giving the lead an arpeggiator while it plays, naming the song in the Info dialog, a tour of the example themes, collapsing the header and the commands list](docs/demo.gif)

## Features

- An FT2-style pattern editor in the terminal, run entirely from the keyboard, on Windows, macOS
  and Linux
- MIDI out on all 16 channels, one instrument each, with its own patch, transpose and bend range
- Pattern commands for CCs, slides, pitch bends, chords, retrigs, delays and song flow, in FT2's
  syntax where it has one
- Live MIDI effects on every instrument, like an arpeggiator, chords, echo and an LFO
- Live recording, MIDI input and clock sync, and a metronome
- MIDI file export for taking a song into a DAW
- Rebindable keys, themes, and an ASCII mode for any terminal

## Quick start

Download mTrak for your platform from the
[releases page](https://github.com/TheExiledCat/mTrak/releases), or build it with
`cargo install --git https://github.com/TheExiledCat/mTrak` (see
[docs/INSTALL.md](docs/INSTALL.md)). Then:

```sh
mtrak                  # start with an empty project
mtrak song.mtrak       # open a project
```

Pick a MIDI output with `Ctrl+O`, press `?` for every key and `/` for every pattern command.

## Documentation

| Guide                                      | What's in it                                                        |
| ------------------------------------------ | ------------------------------------------------------------------- |
| [Installing](docs/INSTALL.md)              | Releases, building from source                                      |
| [Using mTrak](docs/GUIDE.md)               | Keys, playing and recording, instruments, export, files and options |
| [Commands](docs/COMMANDS.md)               | Every pattern command                                               |
| [Effects](docs/EFFECTS.md)                 | The live MIDI effects on instruments                                |
| [Configuration](docs/CONFIG.md)            | Settings, themes and keybindings                                    |
| [Sync](docs/SYNC.md)                       | MIDI input, clock in and out                                        |
| [Project file format](docs/FILE_FORMAT.md) | Reading and writing `.mtrak` files from other programs              |
| [Roadmap](docs/ROADMAP.md)                 | What's done and what's planned                                      |
| [Changelog](docs/CHANGELOG.md)             | What changed in each release                                        |

## Contributing

Bug reports, feature requests and pull requests are welcome. See
[docs/CONTRIBUTING.md](docs/CONTRIBUTING.md) for how to get started and
[docs/CONVENTIONS.md](docs/CONVENTIONS.md) for the code style. Run `./fmt.sh` before
committing to format the code and apply lint fixes.

Note that most of the code was very quickly written so there might be some lines that might be confusing or dirty.
I will make a big refactoring PR in the future when my primary roadmap targets have been finished.
