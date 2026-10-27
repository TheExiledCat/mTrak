# Project file format

This page describes how mTrak saves a project (`.mtrak`), so other programs can read and write
it. It describes file version 3, written by mTrak v0.6.0 and later.

A project file is a short header followed by a [MessagePack](https://msgpack.org) payload.
Everything in the payload is a map with named fields, except the notes in a pattern, which are
packed into bytes.

## Header

| Bytes | Contents                                                  |
| ----- | --------------------------------------------------------- |
| 0-3   | `MTRK` in ASCII                                           |
| 4-5   | The file version, a little-endian unsigned 16-bit integer |
| 6-9   | The payload's length in bytes, little-endian unsigned 32-bit |
| 10-   | The payload                                               |

mTrak opens files of its own version and upgrades files of older versions from 3 on, one
version at a time. Files of versions 1 and 2, from v0.5.0 and earlier, can't be opened.

## Payload

The payload is a map with these fields. mTrak ignores fields it doesn't know, so other programs
can add their own.

| Field                   | Type             | Contents                                                     |
| ----------------------- | ---------------- | ------------------------------------------------------------ |
| `name`                  | string or nil    | The song's name, not used yet                                |
| `bpm`                   | int              | Beats per minute                                             |
| `speed`                 | int              | Ticks per row; 6 plays 4 rows per beat                       |
| `global_velocity`       | int              | Scales every note's velocity, `0`-`255`                      |
| `transpose`             | int              | Semitones added to every note, can be negative              |
| `sequence`              | array of maps    | The song's order of patterns, see below                      |
| `patterns`              | array of maps    | Every pattern; a pattern's number is its index, see below    |
| `instruments`           | array of 16 maps | One instrument per MIDI channel, see below                   |
| `midi_input`            | string or nil    | The name of the MIDI input port to record from               |
| `clock_input`           | string or nil    | The name of the MIDI port to sync to                         |
| `record_input_velocity` | bool             | Whether recording keeps the velocity of incoming notes       |
| `midi_output`           | string or nil    | The name of the MIDI output port                             |
| `send_midi_clock`       | bool             | Whether mTrak sends MIDI clock to the output                 |

### Sequence

Each entry plays a pattern a number of times:

| Field        | Type | Contents                     |
| ------------ | ---- | ---------------------------- |
| `pattern_id` | int  | The pattern's number         |
| `repeats`    | int  | How many times it plays, 1-8 |

### Instruments

The first instrument plays on MIDI channel 1, the last on channel 16.

| Field               | Type   | Contents                                                       |
| ------------------- | ------ | -------------------------------------------------------------- |
| `name`              | string | The instrument's name, can be empty                            |
| `volume`            | int    | Scales the velocity of the instrument's notes, `0`-`255`       |
| `pitch_wheel_range` | int    | The synth's pitch bend range in semitones, for the `I` command |
| `default_cc`        | int    | The CC that CC commands change on a track without an `S`       |
| `transpose`         | int    | Semitones added to the instrument's notes, can be negative     |

### Patterns

| Field           | Type             | Contents                                       |
| --------------- | ---------------- | ---------------------------------------------- |
| `row_count`     | int              | The number of rows, 1-256                      |
| `track_count`   | int              | The number of tracks, always 8 for now         |
| `cells`         | binary           | Every cell, packed as described below          |
| `channel_mutes` | array of 16 bools | Which MIDI channels are muted in this pattern |

`cells` holds `row_count` × `track_count` cells one after another: row 0 track 0, row 0 track
1, and so on to the last track of the last row.

## Cells

Like FastTracker 2's XM patterns, each cell starts with a flag byte that says which fields
follow it, so an empty cell takes one byte. The top bit of the flag byte is always set, and the
two bits below it are always clear:

| Flag bit | Field that follows | Values                                                 |
| -------- | ------------------ | ------------------------------------------------------ |
| `0x01`   | Note               | `1` note off, `12`-`127` the MIDI note (60 is middle C) |
| `0x02`   | Instrument         | `0`-`15`                                               |
| `0x04`   | Volume             | `0`-`128` (`0x80`)                                     |
| `0x08`   | Command            | The command's letter in ASCII, e.g. `F` or `S`         |
| `0x10`   | Command value      | `0`-`255`                                              |

The fields follow in the order of the table. A field that's missing is empty: no note, no
instrument, no volume or no command. A missing command value is `0`. See
[COMMANDS.md](COMMANDS.md) for what each command letter does.

Some cells and their bytes:

| In mTrak       | Bytes                    |
| -------------- | ------------------------ |
| `---\|-\|--\|000` | `80`                     |
| `===\|-\|--\|000` | `81 01`                  |
| `C-4\|3\|40\|F8C` | `9F 3C 03 40 46 8C`      |
| `---\|-\|--\|S10` | `98 53 10`               |
