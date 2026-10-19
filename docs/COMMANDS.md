# Commands

Every cell in a pattern has a command column next to the note, instrument and volume:

```
C-4|1|7F|S4A
 │  │  │  └─ command
 │  │  └──── volume
 │  └─────── instrument
 └────────── note
```

A command is one letter followed by a two digit hex value, e.g. `S4A` is command `S` with value `4A`.
`000` means no command.

Commands send on the MIDI channel of the cell's instrument column. When you enter a command,
the cell's instrument is set to the active instrument.

Each track keeps its own command state, so two tracks playing the same instrument don't
interfere with each other.

A row's command runs before its note starts, so a CC on the same row as a note reaches the
synth just before the note does.

## Values

Command values go from `00` to `FF`, but MIDI controller values, velocities and aftertouch
only go from 0 to 127, so command values are halved when used (pitch bend is the exception,
see `W`):

| Value | Sent as |
|-------|---------|
| `00`  | 0       |
| `40`  | 32      |
| `80`  | 64      |
| `FF`  | 127     |

The volume column isn't halved: its value is the note's velocity as it is, so `40` plays at
velocity 64, and `7F` and `80` both play at 127.

## MIDI commands

### `S` Set CC

Selects which controller (CC number) the track's `C`, `G` and `H` commands change. The value
is the CC number in hex, e.g. `S4A` selects CC 74. Sends nothing by itself.

Until a track has an `S`, its CC commands change CC 0.

### `C` Change CC

Sends the track's selected CC with the given value.

```
---|1|--|S4A   select CC 74 (filter cutoff on many synths)
---|1|--|C80   send CC 74 = 64
```

### `G` CC slide

Sends the given value, then slides the selected CC smoothly to the target value
(see [Slides](#slides)).

```
---|1|--|S02   select CC 2
---|1|--|G00   start at 0 ...
---|1|--|000
---|1|--|CFF   ... arrive at 127 here
```

### `H` CC and velocity slide

A `G` and a `V` in one: the same slide moves the selected CC and sets the velocity of the
notes played during it.

```
---|1|--|S07   select CC 7 (volume)
C-4|1|--|H40   CC 7 = 32, note at velocity 32
C-4|1|--|000   CC 7 still rising, note at velocity 79
---|1|--|CFF   CC 7 arrives at 127
```

### `V` Velocity slide

Slides the velocity of the track's notes, like typing a ramp of values into the volume
column. It sends nothing by itself; it only changes notes that play while it runs.

```
C-4|1|--|V20   velocity 16
C-4|1|--|000   velocity 32
C-4|1|--|000   velocity 48
C-4|1|--|000   velocity 64
C-4|1|--|EA0   velocity 80, slide ends
C-4|1|--|000   velocity 127 (volume column empty)
```

- A note with its own volume value plays at that volume; the slide carries on underneath.
- After the slide reaches its target row, notes use their volume column again (full velocity
  when it's empty).
- A velocity of 0 (`V00` or `V01`) makes notes silent, like volume `00`.
- End a velocity slide with `E`, not `C`: a `C` would also send a CC, which on a track
  without an `S` is CC 0 (bank select on many synths).

### `E` End slide

Lands the track's running slide on the given value and stops it there, without starting a new
one. The value goes wherever the slide was going: its CC, the velocity of the note on the `E`
row, or both. Without a running slide, `E` does nothing.

```
---|1|--|S02   select CC 2
---|1|--|G00   start at 0 ...
---|1|--|000
---|1|--|EFF   ... arrive at 127 and stop
---|1|--|G40   a new, separate slide
```

### `A` Aftertouch

Sends channel aftertouch (pressure) with the given value, scaled like a CC: `A00` is no
pressure, `AFF` is full pressure (127).

### `W` Pitch bend

Bends the pitch of the whole channel. `80` is no bend, `00` is fully down and `FF` is fully
up. How far "fully" is depends on the bend range set on your synth.

```
C-4|1|--|WFF   note starts fully bent up
---|1|--|W80   back to normal pitch
```

- The bend stays until the next `W`, so remember to bring it back to `W80`.
- Pitch bend affects every note on the channel, including notes from other tracks playing
  the same instrument.
- `W` has 256 steps, so on large bend ranges you may hear the steps between values.

## Effect commands

Effect commands change playback, or how the cell's note plays, instead of sending MIDI. They
work on any track and on muted channels.

Several of them work in ticks: each row is split into as many ticks as the speed (`SPD`, or an
`F` below `20`), 6 by default. Tick 0 is the start of the row.

### `F` Set tempo

Changes the BPM or the speed, starting on its own row:

| Value     | Does                                                        |
|-----------|-------------------------------------------------------------|
| `00`      | Back to the song's BPM and the pattern's speed              |
| `01`-`1F` | Sets the speed (ticks per row) to that value                |
| `20`-`FF` | Sets the BPM to that value in hex, e.g. `F7D` is 125 BPM    |

```
---|1|--|F8C   140 BPM from here on
---|1|--|F03   speed 3, still at 140 BPM
---|1|--|F00   back to the song's BPM and the pattern's speed
```

- The change keeps going into the next patterns until an `F00` or until playback stops.
- BPM and speed are separate: setting one leaves the other as it is.
- The speed only sets how many ticks a row has, for slides and for `D`, `R` and `3`; rows
  last the same time at any speed, so only the BPM changes how fast the song plays.

### `Y` Probability

Gives the cell's note a chance to play: `00` never plays, `FF` always plays, and in between
the note plays `xx` times out of 256, so `Y80` plays half the time and `YC0` three times out
of four. Each time the row plays, the chance is rolled again.

```
C-4|1|--|Y80   plays about every other time round
```

- A note off with `Y` is also only sent when the roll succeeds.

### `D` Note delay

Moves the cell's note earlier or later than its row:

| Value     | Does                                                                  |
|-----------|-----------------------------------------------------------------------|
| `00`-`0F` | Starts the note that many ticks before its row, during the row before |
| `10`-`1F` | Starts the note that many ticks into its row                          |

```
C-4|1|--|D13   at speed 6, the note starts halfway through its row
D-4|1|--|D02   at speed 6, the note starts on tick 4 of the row above
```

- The track's previous note keeps sounding until the moved one starts.
- A note off can be moved the same way.
- `D00` and `D10` play the note normally. A late delay as long as the row or longer (e.g.
  `D16` at speed 6) means the note doesn't play.
- Early notes work like early `3` notes: the earliest start is tick 1 of the row before, they
  follow the play order across patterns, and they use the mutes of their own pattern. When
  one starts, anything still planned on that track in the row before (more roll hits, a note
  cut, a late `D` that hasn't started yet) is dropped.
- The note's velocity is worked out when it starts, so a running velocity slide gives it the
  velocity of that tick.
- `20` and up do nothing yet; the note plays normally.

### `R` Roll

`Rxy` plays the note at the start of the row and then again every `y` ticks until the row
ends. `x` changes the velocity of each repeat:

| `x` | Each repeat | `x` | Each repeat |
|-----|-------------|-----|-------------|
| `0` | same        | `8` | same        |
| `1` | -2          | `9` | +2          |
| `2` | -4          | `A` | +4          |
| `3` | -8          | `B` | +8          |
| `4` | -16         | `C` | +16         |
| `5` | -32         | `D` | +32         |
| `6` | ×2/3        | `E` | ×3/2        |
| `7` | ×1/2        | `F` | ×2          |

```
C-4|1|40|R92   at speed 6: hits on ticks 0, 2 and 4, at velocity 64, 66 and 68
```

- This is FastTracker 2's multi retrig table, with the steps doubled since MIDI velocity goes
  up to 127.
- Velocity stays between 0 and 127; once it reaches 0 the repeats are silent.
- With `x` at `0` or `8`, a running velocity slide still sets the velocity of each hit.
- `y` of `0` (e.g. `R00` or `R30`) just plays the note once.

### `3` Legato

Glides into the cell's note from the track's previous note. The new note starts before the
old one is released, which makes a synth in mono or legato mode glide between them instead
of starting the note fresh. The glide itself comes from the synth: turn on its portamento
(on many synths CC 65 switches it on, `S41` then `C7F`, and CC 5 sets the glide time).

| Value     | Does                                                                                 |
|-----------|--------------------------------------------------------------------------------------|
| `00`      | Glides in on its own row                                                             |
| `01`-`0F` | Starts the note that many ticks before its row, during the row before                |
| `10`-`1F` | Starts the note on its row and keeps the previous note sounding that many more ticks |
| `30`-`3F` | Note cut: releases the track's note on that tick of the row                          |

```
C-4|1|--|000
D-4|1|--|302   at speed 6, D-4 starts on tick 4 of the row above and glides from C-4
E-4|1|--|312   E-4 starts on its row, D-4 keeps sounding 2 more ticks
---|1|--|333   E-4 is released on tick 3
```

- The earliest an early note can start is tick 1 of the row before, since tick 0 is that
  row's own note. At speed 1 it starts on its own row.
- Early notes follow the play order, so they work across patterns and when a pattern loops back
  to its top.
- When an early note starts, anything still planned on that track in the row before (more
  roll hits, a note cut, a late `D` that hasn't started yet) is dropped, as with an early `D`.
- `330` releases the note straight away and the cell's own note doesn't play. On a cell
  without a note, a cut releases the note that's playing. A cut also stops a previous note
  still sounding from a `31y`. A cut past the end of the row
  doesn't happen.
- `20`-`2F` and `40` up do nothing yet; the note plays normally.

## Slides

A slide moves from its own value to a target, updating once per tick. With a speed (`SPD`, or
an `F` below `20`) of 6 that is 6 updates per row, so a higher speed gives a smoother slide.

- **Target:** the track's next `C`, `E`, or command that starts a slide. The slide reaches
  that value on that row.
- **Chaining:** a command that starts a slide also counts as the target of a running one, so
  the running slide lands there and the new one carries straight on. That lets you draw a
  shape out of several points: `G00` → `G80` → `C00` slides up to 64 and back down to 0.
  To land a slide and stop, use `E`.
- **Across patterns:** a slide follows the play order, so its target can be in the next
  pattern of the song, or back at the top of the pattern when looping it.
- **No target:** if there is no later target, the slide only uses its own value on its own row.
- **`S` during a slide:** an `S` before the target stops slides that change a CC, since they'd
  be changing a different one. Velocity-only slides carry on.

## Muting

A muted channel sends no MIDI, but its commands keep running in the background. If you mute
a channel in the middle of a slide and unmute it later, the slide carries on from where it
would have been had you never muted it.

Commands on rows with volume `00` still run, even though the note doesn't play.

Delayed notes and rolls check the mutes when they play, so muting a channel mid-row silences
them too. Early `D` and `3` notes use the mutes of the pattern they belong to, even when they
start during the pattern before. Muting also cuts notes still sounding from a `31y`.

## Playback

Starting or stopping playback resets all command state: selected CCs go back to 0, running
slides end, delayed notes, rolls and cuts that haven't happened yet are dropped, notes still
sounding from a `31y` are cut, and the tempo goes back to the song's BPM and the pattern's
speed. Channels that were sent a pitch bend or aftertouch get them reset to no bend and no
pressure, even when muted, so your synth is never left out of tune.
