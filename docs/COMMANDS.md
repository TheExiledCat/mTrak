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
only go from 0 to 127, so command values are halved when used (pitch bends are the exception,
see `P` and `I`):

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

Selects which controller (CC number) the track's `M`, `V` and `J` commands change. The value
is the CC number in hex, e.g. `S4A` selects CC 74. Sends nothing by itself.

Until a track has an `S`, its CC commands change the instrument's default CC, set in the
instrument dialog (`Default CC`, in hex like `S`). It is `00` (CC 0) unless you change it.

```
---|1|--|M80   instrument 1's default CC = 64, no S needed
```

### `M` Change CC

Sends the track's selected CC with the given value.

```
---|1|--|S4A   select CC 74 (filter cutoff on many synths)
---|1|--|M80   send CC 74 = 64
```

### `W` Mod wheel

Sends the mod wheel (CC 1) with the given value and selects it, like an `S01` followed by an
`M` on the same row. The track's later `M`, `V` and `J` commands then change the mod wheel
too, so a `W` followed by a `V` glides it.

```
---|1|--|W00   mod wheel = 0, and selected
---|1|--|V00   slide the mod wheel from 0 ...
---|1|--|000
---|1|--|MFF   ... to 127 here
```

- A running slide can land on a `W`, like on an `M`.

### `V` CC slide

Sends the given value, then slides the selected CC smoothly to the target value
(see [Slides](#slides)).

```
---|1|--|S02   select CC 2
---|1|--|V00   start at 0 ...
---|1|--|000
---|1|--|MFF   ... arrive at 127 here
```

### `J` CC and velocity slide

A `V` and a `U` in one: the same slide moves the selected CC and sets the velocity of the
notes played during it.

```
---|1|--|S07   select CC 7 (volume)
C-4|1|--|J40   CC 7 = 32, note at velocity 32
C-4|1|--|000   CC 7 still rising, note at velocity 79
---|1|--|MFF   CC 7 arrives at 127
```

### `U` Velocity slide

Slides the velocity of the track's notes, like typing a ramp of values into the volume
column. It sends nothing by itself; it only changes notes that play while it runs.

```
C-4|1|--|U20   velocity 16
C-4|1|--|000   velocity 32
C-4|1|--|000   velocity 48
C-4|1|--|000   velocity 64
C-4|1|--|NA0   velocity 80, slide ends
C-4|1|--|000   velocity 127 (volume column empty)
```

- A note with its own volume value plays at that volume; the slide carries on underneath.
- After the slide reaches its target row, notes use their volume column again (full velocity
  when it's empty).
- A velocity of 0 (`U00` or `U01`) makes notes silent, like volume `00`.
- End a velocity slide with `N`, not `M`: an `M` would also send a CC, which on a track
  without an `S` is the instrument's default CC (CC 0, bank select on many synths, unless
  you changed it).

### `N` End slide

Lands the track's running slide on the given value and stops it there, without starting a new
one. The value goes wherever the slide was going: its CC, the velocity of the note on the `N`
row, or both. Without a running slide, `N` does nothing.

```
---|1|--|S02   select CC 2
---|1|--|V00   start at 0 ...
---|1|--|000
---|1|--|NFF   ... arrive at 127 and stop
---|1|--|V40   a new, separate slide
```

### `T` Aftertouch

Sends channel aftertouch (pressure) with the given value, scaled like a CC: `T00` is no
pressure, `TFF` is full pressure (127).

### `P` Pitch bend

Bends the pitch of the whole channel. `80` is no bend, `00` is fully down and `FF` is fully
up. How far "fully" is depends on the bend range set on your synth.

```
C-4|1|--|PFF   note starts fully bent up
---|1|--|P80   back to normal pitch
```

- The bend stays until the next `P`, so remember to bring it back to `P80`.
- Pitch bend affects every note on the channel, including notes from other tracks playing
  the same instrument.
- `P` has 256 steps, so on large bend ranges you may hear the steps between values.

### `I` Interval

Bends the pitch of the whole channel by whole semitones: `80` is no bend, `83` is 3 semitones
up and `7D` 3 down. Useful for repitching chopped samples, e.g. drum hits, without retyping
their notes.

```
C-4|1|--|I83   note plays 3 semitones higher (D#4)
C-4|1|--|I7B   5 semitones lower (G-3)
---|1|--|I80   back to normal pitch
```

- It works out the bend from the instrument's bend range, set in the instrument dialog
  (`Ctrl+R`, `Bend range`, in hex: `0C` is 12 semitones, the default). Set it to the same
  range as your synth, or the notes land out of tune.
- Bends past the bend range stop at a full bend: with a range of 12, `I8F` bends 12 up, like
  `I8C`. With a range of `00`, `I` sends no bend.
- Like `P`, the bend affects every note on the channel and stays until the next `P` or `I`.

## Playback commands

Playback commands change playback, or how the cell's note plays, instead of sending MIDI. They
work on any track and on muted channels.

Several of them work in ticks. A row lasts as many ticks as the speed, 6 by default: the
song's `SPD` in the header, or an `F` below `20` to change it from a row onwards. Tick 0 is
the start of the row.

As in FastTracker 2, a tick is one MIDI clock pulse: there are 24 per beat, so at 125 BPM a
tick lasts 20 ms. The speed therefore sets how fast rows go by: at speed 6 a beat is 4 rows,
at speed 3 it is 8 rows (double speed), and at speed 12 it is 2 rows (half speed).

### `F` Set tempo

Changes the BPM or the speed, starting on its own row:

| Value     | Does                                                        |
|-----------|-------------------------------------------------------------|
| `00`      | Back to the song's BPM and speed                            |
| `01`-`1F` | Sets the speed (ticks per row) to that value                |
| `20`-`FF` | Sets the BPM to that value in hex, e.g. `F7D` is 125 BPM    |

```
---|1|--|F8C   140 BPM from here on
---|1|--|F03   speed 3, still at 140 BPM
---|1|--|F00   back to the song's BPM and speed
```

- The change keeps going into the next patterns until an `F00` or until playback stops.
- BPM and speed are separate: setting one leaves the other as it is.
- Both change how fast the song plays: the BPM sets how long a tick is, the speed how many
  ticks a row lasts. `F03` plays the rows below it twice as fast.
- While following an external MIDI clock, BPM values have no effect, since the clock sets the
  tempo; speed values still apply.

### `Y` Probability

Gives the cell's note a chance to play: `00` never plays, `FF` always plays, and in between
the note plays `xx` times out of 256, so `Y80` plays half the time and `YC0` three times out
of four. Each time the row plays, the chance is rolled again.

```
C-4|1|--|Y80   plays about every other time round
```

- A note off with `Y` is also only sent when the roll succeeds.

### `E` Extended

FastTracker 2's `E` commands: the first digit picks the command, the second is its value.

| Value     | Does                                                                  |
|-----------|-----------------------------------------------------------------------|
| `80`-`8F` | Early delay: starts the note that many ticks before its row           |
| `90`-`9F` | Retrig note: replays the note every that many ticks in its row        |
| `C0`-`CF` | Note cut: releases the track's note on that tick of the row           |
| `D0`-`DF` | Note delay: starts the note that many ticks into its row              |
| `E0`-`EF` | Pattern delay: holds the row that many rows longer                    |
| `F0`-`FF` | Stop: stops playback once the row has played                          |

```
C-4|1|--|ED3   at speed 6, the note starts halfway through its row
D-4|1|--|E82   at speed 6, the note starts on tick 4 of the row above
C-4|1|--|E92   at speed 6, hits on ticks 0, 2 and 4
---|1|--|EC3   the note playing is released on tick 3
---|1|--|EE3   the row lasts 4 rows
---|1|--|EF0   playback stops at the end of this row
```

**Note delay (`ED`) and early delay (`E8`):**

- The track's previous note keeps sounding until the moved one starts.
- A note off can be moved the same way.
- `ED0` and `E80` play the note normally. A delay as long as the row or longer (e.g. `ED6`
  at speed 6) means the note doesn't play.
- `E8` isn't in FastTracker 2, which has no early delay; it uses an `E` slot FT2 leaves free.
- Early notes work like early `L` notes: the earliest start is tick 1 of the row before, they
  follow the play order across patterns, and they use the mutes of their own pattern. When
  one starts, anything still planned on that track in the row before (more retrig hits, a note
  cut, an `ED` that hasn't started yet) is dropped.
- The note's velocity is worked out when it starts, so a running velocity slide gives it the
  velocity of that tick.

**Retrig note (`E9`):**

- Like FT2's `E9x`: hits on ticks `x`, `2x` and so on within the row, at the same velocity,
  starting over every row. For velocity changes and intervals that carry across rows, use
  `R`.
- Without a note it repeats the track's last note; `E90` without a note replays it straight
  away.

**Note cut (`EC`):**

- `EC0` releases the note straight away and the cell's own note doesn't play. On a cell
  without a note, a cut releases the note that's playing. A cut also stops a previous note
  still sounding from an `L1y`. A cut past the end of the row doesn't happen.

**Pattern delay (`EE`):**

- Like FT2's `EEx`: the row lasts `x` more rows. Its notes don't play again, but slides,
  retrigs and effects keep going. With several on one row, the longest wins.

**Stop (`EF`):**

- Playing always loops, so this is how a song ends: playback stops once the row has played,
  and the notes still sounding are released. The second digit doesn't matter.
- `EF` isn't in FastTracker 2; it uses an `E` slot FT2 leaves free.

Other `E` values do nothing yet; the note plays normally.

### `R` Retrig

`Rxy` replays the note every `y` ticks, like FastTracker 2's multi retrig. `x` changes the
velocity of each repeat:

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
C-4|1|--|R13   at speed 6: hits on ticks 0, 2 and 5 at velocity 127, 125 and 123
---|1|--|RA6   no note: repeats the C-4 above every 6 ticks, 4 louder each time
C-4|1|40|R92   with a volume: hits on ticks 0, 2 and 4, all at velocity 64
```

It works like FT2's:

- **Counting:** each track counts ticks since its last note or repeat, and repeats the note
  when the count reaches `y`. The count carries on across rows, so a `y` as long as the row
  or longer still repeats on later rows. Only rows with an `R` count, and the row start only
  counts when the cell has no volume. A new note starts the count over.
- **No note:** `R` repeats the last note the track played, carrying on from its velocity.
  If the track hasn't played a note since playback started, nothing happens.
- **Volume column:** with a volume on the `R` cell, every repeat plays at that volume and
  `x` has no effect.
- **Memory:** an `x` or `y` of `0` keeps the track's previous one, so `R00` carries on the
  last retrig. With no previous `y`, a `y` of `0` repeats on every tick.
- The `x` table is FastTracker 2's, with the steps doubled since MIDI velocity goes up to
  127. Velocity stays between 0 and 127; once it reaches 0 the repeats are silent.
- With `x` at `0` or `8`, a running velocity slide still sets the velocity of each hit.
- Starting or stopping playback forgets the counts, the last notes and the memory.

### `L` Legato

Glides into the cell's note from the track's previous note. The new note starts before the
old one is released, which makes a synth in mono or legato mode glide between them instead
of starting the note fresh. The glide itself comes from the synth: turn on its portamento
(on many synths CC 65 switches it on, `S41` then `M7F`, and CC 5 sets the glide time).

| Value     | Does                                                                                 |
|-----------|--------------------------------------------------------------------------------------|
| `00`      | Glides in on its own row                                                             |
| `01`-`0F` | Starts the note that many ticks before its row, during the row before                |
| `10`-`1F` | Starts the note on its row and keeps the previous note sounding that many more ticks |

```
C-4|1|--|000
D-4|1|--|L02   at speed 6, D-4 starts on tick 4 of the row above and glides from C-4
E-4|1|--|L12   E-4 starts on its row, D-4 keeps sounding 2 more ticks
---|1|--|EC3   E-4 is released on tick 3
```

- The earliest an early note can start is tick 1 of the row before, since tick 0 is that
  row's own note. At speed 1 it starts on its own row.
- Early notes follow the play order, so they work across patterns and when a pattern loops back
  to its top.
- When an early note starts, anything still planned on that track in the row before (more
  retrig hits, a note cut, an `ED` that hasn't started yet) is dropped, as with an `E8`.
- To cut the note, use `EC` (see [`E` Extended](#e-extended)).
- FastTracker 2 has no legato command; its `L` sets a sample's envelope position, which has
  no meaning for MIDI, so mTrak uses the letter for legato.
- `20` and up do nothing yet; the note plays normally.

### `B` Position jump

Like FT2's `Bxx`: after this row, playback goes to sequence position `xx` and starts its
pattern from the top. A position past the end of the song goes to the first one.

```
---|1|--|B03   carry on at sequence position 03
```

- Only works while the song plays: playing a pattern or a block ignores it.
- With a `D` on the same row, the song goes to that row of position `xx` instead of its top.

### `D` Pattern break

Like FT2's `Dxx`: after this row, the song moves on to the next sequence position as if the
pattern had ended, starting at row `xx`. A row past the end of that pattern starts it from
the top.

```
---|1|--|D10   skip the rest of this pattern, carry on at row 10 of the next
```

- Rows are hex like everywhere else in mTrak, so `D10` is row `10`. FT2 reads the value as
  decimal.
- A pattern with repeats in the sequence counts as ending, so a break moves on to its next
  repeat.
- While playing a single pattern, the break jumps to row `xx` of the same pattern. A block loop
  ignores it.

### `G` Global velocity

Like FT2's `Gxx`: scales the velocity of every note, like `Velocity` in the Global dialog,
from `00` (silent) to `FF` (as they are). It lasts until playback starts or stops, and then
goes back to the song's own.

```
---|1|--|G80   every note at about half its velocity from here on
```

### `H` Global velocity slide

Like FT2's `Hxy`: slides the global velocity up by `x` or down by `y` steps on every tick of
the row but its first. A step is 4 out of the global velocity's `FF`, since FT2's global
volume only goes up to 64.

```
---|1|--|H02   at speed 6, 5 ticks of 8 down: 28 lower by the end of the row
---|1|--|H00   the same again
```

- If both are set, `x` wins.
- `H00` repeats the last slide.
- It only runs on rows that have it, so put it on every row the slide should last.

### `C` Track velocity

Scales the velocity of every note on the track from this row on, from `00` (silent) to `FF`
(as they are). It lasts until another `C` on the track, or until playback starts or stops.

```
---|1|--|C80   this track at about half velocity from here on
```

- FT2's `C` sets the volume of the note on its row; here it keeps scaling the track, on top
  of each note's own volume and the global velocity.

## Slides

A slide moves from its own value to a target, updating once per tick. With a speed (`SPD`, or
an `F` below `20`) of 6 that is 6 updates per row. Since ticks always last the same time at a
given BPM, a slide over the same amount of time is equally smooth at any speed.

- **Target:** the track's next `M`, `W`, `N`, or command that starts a slide. The slide reaches
  that value on that row.
- **Chaining:** a command that starts a slide also counts as the target of a running one, so
  the running slide lands there and the new one carries straight on. That lets you draw a
  shape out of several points: `V00` → `V80` → `M00` slides up to 64 and back down to 0.
  To land a slide and stop, use `N`.
- **Across patterns:** a slide follows the play order, so its target can be in the next
  pattern of the song, or back at the top of the pattern when looping it.
- **No target:** if there is no later target, the slide only uses its own value on its own row.
- **`S` during a slide:** an `S` before the target stops slides that change a CC, since they'd
  be changing a different one. Velocity-only slides carry on.

`H` is the exception: it moves by the same step on every tick of its own row, the
FastTracker 2 way, instead of towards a target.

## Muting

A muted channel sends no MIDI, but its commands keep running in the background. If you mute
a channel in the middle of a slide and unmute it later, the slide carries on from where it
would have been had you never muted it.

Commands on rows with volume `00` still run, even though the note doesn't play.

Delayed notes and retrigs check the mutes when they play, so muting a channel mid-row silences
them too. Early `E8` and `L` notes use the mutes of the pattern they belong to, even when they
start during the pattern before. Muting also cuts notes still sounding from an `L1y`.

## Playback

Starting or stopping playback resets all command state: selected CCs go back to each
instrument's default CC, running slides end, delayed notes, retrigs and cuts that haven't
happened yet are dropped, notes still sounding from an `L1y` are cut, the tempo goes back
to the song's BPM and speed, and the global and track velocities go back to the song's. Channels that were sent a pitch bend or aftertouch get them
reset to no bend and no pressure, even when muted, so your synth is never left out of tune.
