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

Values go from `00` to `FF`, but MIDI controller values, velocities and aftertouch only go
from 0 to 127, so values are halved when used (pitch bend is the exception, see `W`):

| Value | Sent as |
|-------|---------|
| `00`  | 0       |
| `40`  | 32      |
| `80`  | 64      |
| `FF`  | 127     |

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

## Slides

A slide moves from its own value to a target, updating once per tick. With a speed (`SPD`) of
6 that is 6 updates per row, so a higher speed gives a smoother slide.

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

## Playback

Starting or stopping playback resets all command state: selected CCs go back to 0 and running
slides end. Channels that were sent a pitch bend or aftertouch get them reset to no bend and
no pressure, even when muted, so your synth is never left out of tune.
