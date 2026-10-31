# Project file format

This page describes how mTrak saves a project (`.mtrak`), so other programs can read and write
it. It describes file version 5, written by mTrak v0.8.0 and later.

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

The payload is a map with these fields, in this order. `patterns` is always last, so a program
that only needs the song's settings can stop reading before it. mTrak ignores fields it
doesn't know, so other programs can add their own.

| Field                   | Type             | Contents                                                     |
| ----------------------- | ---------------- | ------------------------------------------------------------ |
| `name`                  | string or nil    | The song's name, not used yet                                |
| `bpm`                   | int              | Beats per minute                                             |
| `speed`                 | int              | Ticks per row; 6 plays 4 rows per beat                       |
| `global_velocity`       | int              | Scales every note's velocity, `0`-`255`                      |
| `transpose`             | int              | Semitones added to every note, can be negative              |
| `midi_input`            | string or nil    | The name of the MIDI input port to record from               |
| `clock_input`           | string or nil    | The name of the MIDI port to sync to                         |
| `record_input_velocity` | bool             | Whether recording keeps the velocity of incoming notes       |
| `midi_output`           | string or nil    | The name of the MIDI output port                             |
| `send_midi_clock`       | bool             | Whether mTrak sends MIDI clock to the output                 |
| `metronome`             | map              | When and how the metronome clicks, see below                 |
| `sequence`              | array of maps    | The song's order of patterns, see below                      |
| `instruments`           | array of 16 maps | One instrument per MIDI channel, see below                   |
| `patterns`              | array of maps    | Every pattern; a pattern's number is its index, see below    |

### Metronome

| Field             | Type   | Contents                                                             |
| ----------------- | ------ | -------------------------------------------------------------------- |
| `mode`            | string | `Off`, `Recording` (only while recording) or `Always`                |
| `channels`        | int    | The channels it clicks on, a bit per channel: bit 0 is channel 1, bit 15 channel 16 |
| `accent_note`     | int    | The MIDI note on the first beat of each bar                          |
| `accent_velocity` | int    | Its velocity, `0`-`127`                                              |
| `beat_note`       | int    | The MIDI note on the other beats                                     |
| `beat_velocity`   | int    | Its velocity, `0`-`127`                                              |
| `beats_per_bar`   | int    | Beats in a bar, `1`-`16`                                             |

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
| `effects`           | array of maps | The instrument's live MIDI effects, see below           |

#### Effects

The effects on an instrument's channel, in the order notes go through them. Each one is:

| Field     | Type          | Contents                                                     |
| --------- | ------------- | ------------------------------------------------------------ |
| `kind`    | string        | Which effect it is: `Eq`, `Arp`, `Harmonize`, `Lfo`, `Echo`, `Envelope`, `Swing`, `Randomize`, `Scale`, `NoteToCc` or `Probability`, see below |
| `values`  | array of ints | Each of the effect's parameters, `0`-`255`, in the order below |
| `enabled` | bool          | `false` if the effect is turned off: notes pass it unchanged, and it keeps its place and values |
| `param_ccs` | array of ints or nils | The CC each parameter follows, `0`-`127`, in the same order as `values`; nil for none. Parameters past its end follow none |

mTrak clamps a value to what its parameter takes, gives parameters without a value their
default, and ignores values past the effect's last parameter.

The `Eq` effect changes the velocity of notes by their pitch, and fades notes out above and
below a cutoff. A note whose velocity reaches 0 isn't played. Its values:

| Index | Parameter                | Values                                                         | Default |
| ----- | ------------------------ | -------------------------------------------------------------- | ------- |
| 0     | Low band up to           | A MIDI note, `0`-`127`; notes up to it are in the low band     | `42`    |
| 1     | Mid band up to           | A MIDI note; notes above the low band up to it are in the mid band | `84` |
| 2     | Low velocity             | Scales the low band's velocity, a two's complement signed byte: `0` leaves it unchanged, `0x80` (-128) silences it, `0x7F` (127) about doubles it | `0` |
| 3     | Mid velocity             | The same for the mid band                                      | `0`     |
| 4     | High velocity            | The same for notes above the mid band                          | `0`     |
| 5     | High pass                | A MIDI note; notes below it get quieter                        | `0`     |
| 6     | Low pass                 | A MIDI note; notes above it get quieter                        | `127`   |
| 7     | Filter slope             | The velocity notes lose per octave past a cutoff               | `8`     |

The `Arp` effect plays a held note as a looping sequence of steps, one note at a time. It only
runs while the song plays; when stopped, notes pass it unchanged. Its values, where the signed
ones are two's complement bytes (`0xF4` is -12):

| Index | Parameter    | Values                                                                 | Default |
| ----- | ------------ | ---------------------------------------------------------------------- | ------- |
| 0-7   | Step 1-8     | Signed semitones added to the held note; `0` plays the held note itself | `0`     |
| 8     | Length       | How many steps play before it starts over, `1`-`8`                     | `4`     |
| 9     | Rate         | How long each step lasts in ticks, `1`-`255`                           | `6`     |
| 10    | Gate         | Signed ticks. Negative: each next note starts that much before the last one ends, at most one tick less than the rate. Positive: each note ends that much after the next one starts. `0`: each note ends as the next starts | `0` |

A step whose note falls outside `0`-`127` is a rest.

The `Harmonize` effect plays a chord for each note: the note itself and its intervals, from
low to high. Its values:

| Index | Parameter    | Values                                                                 | Default |
| ----- | ------------ | ---------------------------------------------------------------------- | ------- |
| 0-6   | Interval 1-7 | Signed semitones from the note, two's complement; `0` is unused        | `0`     |
| 7     | Inversion    | `0`-`7`; each one raises the chord's lowest note an octave. A chord has one inversion fewer than it has notes; higher values play its last one | `0` |
| 8     | Legato       | Ticks between each chord note and the next, from low to high, `0`-`255`; only while the song plays | `0` |

Chord notes outside `0`-`127` aren't played, and a note in the chord twice is played once.

The `Lfo` effect moves a value up and down in a repeating wave. Velocity changes each note as
it starts; pitch bend and CC are sent on every tick while notes are held, and before each
note. The wave follows the pattern, starting with its first row, or starts over with every
note with retrigger on. Its values:

| Index | Parameter | Values                                                                    | Default |
| ----- | --------- | ------------------------------------------------------------------------- | ------- |
| 0     | Rate      | How long one cycle of the wave lasts in ticks, `1`-`255`                  | `6`     |
| 1     | Depth     | How far the wave reaches either way, a two's complement signed byte: in velocity or CC steps; for pitch bend, `127` (or `-127`) reaches the ends of the bend range. Negative flips the wave | `0x20` |
| 2     | Shape     | `0` sine, `1` triangle, `2` saw up, `3` saw down, `4` square, `5` random (a new value every cycle) | `0` |
| 3     | Retrigger | `1` to start the wave over with every note, `0` to follow the pattern      | `0`     |
| 4     | Target    | `0` velocity, `1` pitch bend, `2` a CC                                     | `0`     |
| 5     | CC        | The CC the wave moves, `0`-`127`, for target `2`                          | `1`     |
| 6     | CC center | The CC's value in the middle of the wave, `0`-`127`                       | `64`    |

When playback stops, the bend or CC the wave moved last is set back to its middle.

The `Envelope` effect moves the pitch bend or a CC with an ADSR envelope, started by notes. It only runs while the song plays. Its values:

| Index | Parameter | Values                                                                    | Default |
| ----- | --------- | ------------------------------------------------------------------------- | ------- |
| 0     | Attack    | Ticks to rise from rest to the peak, `0`-`255`                            | `0`     |
| 1     | Decay     | Ticks to fall from the peak to the sustain level, `0`-`255`               | `12`    |
| 2     | Sustain   | The level held while notes are held, `0`-`255`; `255` is the peak         | `128`   |
| 3     | Release   | Ticks to fall back to rest after the last note ends, `0`-`255`            | `12`    |
| 4     | Overlap   | What a note does while the envelope is moving: `0` starts the attack from rest, `1` from where it is | `0` |
| 5     | Depth     | How far the peak is from rest, a two's complement signed byte: in CC steps; for pitch bend, `127` (or `-127`) reaches the end of the bend range | `0x7F` |
| 6     | Target    | `0` pitch bend, `1` a CC                                                  | `1`     |
| 7     | CC        | The CC it moves, `0`-`127`, for target `1`                                | `1`     |
| 8     | CC base   | The CC's value at rest, `0`-`127`                                         | `0`     |

When playback stops, the bend or CC is set back to rest.

The `Echo` effect plays each note again and again, each repeat as long as the note was held
and quieter than the one before, until its velocity reaches 0. It only runs while the song
plays; when stopped, notes pass it unchanged. Its values:

| Index | Parameter | Values                                                                    | Default |
| ----- | --------- | ------------------------------------------------------------------------- | ------- |
| 0     | Rate      | Ticks between the repeats, `1`-`255`                                      | `12`    |
| 1     | Feedback  | Each repeat's velocity is the one before times this / 256, rounded down, `0`-`255`; `0` plays no repeats | `128` |
| 2     | Dry/wet   | `0`-`255`. Below `128` the repeats get quieter, down to none at `0`; above it the note itself does, down to silent at `255`. At `128` both play at full velocity | `128` |

The `Swing` effect delays the notes on every second line of a grid, and can make them louder or
quieter than the notes on the beat. Notes off the grid aren't moved. It only runs while the song
plays. Its values:

| Index | Parameter | Values                                                                    | Default |
| ----- | --------- | ------------------------------------------------------------------------- | ------- |
| 0     | Grid      | `0` 1/4 (24 ticks), `1` 1/8 (12), `2` 1/16 (6), `3` 1/32 (3)               | `2`     |
| 1     | Swing     | `128`-`252`: where the swung line lands in a pair of grid steps, as a share of 256. `128` doesn't move it; the delay is the pair's ticks times this / 256, rounded, minus one step | `170` |
| 2     | Accent    | `0`-`255`. Below `128` the swung notes' velocity is scaled by this / 128; above it the notes on the beat are scaled by (256 - this) / 128 | `128` |

A delayed note's note off is delayed just as much, so it keeps its length.

The `Randomize` effect moves each note's velocity, pitch or octave by a random amount. Its values:

| Index | Parameter     | Values                                                                 | Default |
| ----- | ------------- | ---------------------------------------------------------------------- | ------- |
| 0     | Randomize     | `0` velocity, `1` pitch, `2` octave                                    | `0`     |
| 1     | Down by up to | `0`-`127`: the furthest it moves down, in velocity steps, semitones or octaves | `16` |
| 2     | Up by up to   | `0`-`127`: the furthest it moves up                                    | `16`    |
| 3     | Chance        | `0`-`255`: how often a note is moved; `255` always, otherwise this many times out of 255 | `255` |

Velocities stay within `1`-`127` and pitches within `12`-`127`; a note an octave would move
outside that isn't moved.

The `Scale` effect moves notes outside a scale to the closest note in it. Its values:

| Index | Parameter   | Values                                                                 | Default |
| ----- | ----------- | ---------------------------------------------------------------------- | ------- |
| 0     | Root        | `0`-`11`, C to B                                                       | `0`     |
| 1     | Scale       | `0` major, `1` minor, `2` harmonic minor, `3` melodic minor, `4` dorian, `5` phrygian, `6` lydian, `7` mixolydian, `8` locrian, `9` major pentatonic, `10` minor pentatonic, `11` blues, `12` whole tone | `0` |
| 2     | Between two | A note as close to the scale note below as to the one above goes `0` down, `1` up, `2` either at random; `3` drops every note outside the scale | `0` |
| 3     | Transpose   | Signed semitones added before the scale, `-12`-`12`, two's complement  | `0`     |

The `NoteToCc` effect sends a message for every note, made from the note's pitch or velocity.
Its values:

| Index | Parameter  | Values                                                                  | Default |
| ----- | ---------- | ----------------------------------------------------------------------- | ------- |
| 0     | Value from | `0` the note's pitch, `1` its velocity                                  | `0`     |
| 1     | Send as    | `0` a CC, `1` pitch bend (the value times 128), `2` channel aftertouch, `3` program change | `0` |
| 2     | CC         | The CC it sends, `0`-`127`, for `Send as` `0`                           | `74`    |
| 3     | Play notes | `1` to play the note too, `0` to only send the message                  | `1`     |

The `Probability` effect lets each note through with a chance. Its values:

| Index | Parameter   | Values                                                                 | Default |
| ----- | ----------- | ---------------------------------------------------------------------- | ------- |
| 0     | Chance      | `0`-`255`: a note plays this many times out of 255; `255` always       | `192`   |
| 1     | Chance from | `0` the chance above, `1` the note's velocity, scaled so `127` always plays | `0` |
| 2     | Roll for    | `0` each note on its own, `1` each chord: the notes starting on the same tick play or drop together | `0` |

A dropped note's note off is dropped too.

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

## Older versions

- [File version 4](archive/FILE_FORMAT_v4.md), written by mTrak v0.7.0 to v0.7.1.
- [File version 3](archive/FILE_FORMAT_v3.md), written by mTrak v0.6.0.
