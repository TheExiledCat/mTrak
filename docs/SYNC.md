# MIDI setup and sync

mTrak can take notes from a MIDI keyboard, follow the clock of another device, and send its
own clock out. All of it is set up in the Config dialog
(`Ctrl+O`).

## The Config dialog

| Field             | What it does                                                            |
|-------------------|-------------------------------------------------------------------------|
| `MIDI input`      | The port notes come in on, for playing and entering notes               |
| `Clock SRC`       | `(internal)` to use mTrak's own tempo, or the port to follow clock from |
| `Record Velocity` | Notes entered from the MIDI input keep their velocity                   |
| `MIDI output`     | The port mTrak plays on                                                 |
| `Send MIDI clock` | Sends clock pulses to the output, so other devices can follow mTrak     |

The ports are saved with the project. When a project opens and a saved port isn't there,
that setting goes back to none (or `(internal)` for the clock).

The note input and the clock input can be the same port or two different ones. Clock and
transport messages never reach the pattern editor, so a keyboard that also sends clock
doesn't get in the way.

## Following an external clock

Pick the device's port as `Clock SRC`, then start playback on that device. While a clock
input is set, the header shows `SYN` and the tempo measured from the clock instead of `BPM`.
The measured tempo is averaged over the last beat, so it stays steady.

mTrak plays one tick per clock pulse, so the device sets the tempo and the song's speed sets
how many rows play per beat, as always. `F` tempo commands have no effect while following a
clock; `F` speed commands still change the rows per beat.

What mTrak does with the device's transport messages:

| Message                  | What mTrak does                                                         |
|--------------------------|-------------------------------------------------------------------------|
| Start                    | Plays the song from the top, looping, from the next clock pulse         |
| Stop                     | Stops playback that follows the clock                                   |
| Continue                 | Picks up where playback stopped, from the next clock pulse              |
| Song position            | Moves where the next Continue picks up                                  |

- Following the clock always plays the whole song on loop, since the device decides when it
  ends. Start ignores whatever you last played in mTrak.
- Song position counts from the top of the song, using the song's speed. It doesn't follow
  `F` speed changes, so a song that changes speed can pick up a little off after the device
  jumps. Start, Stop and Continue on their own stay exact.
- If the clock stops without a Stop message, playback stops after two beats without pulses
  (at least half a second), as if it had received Stop. A Continue then picks up from there.

### Playing on your own while synced

The play and stop keys still work while a clock input is set. Playback you start yourself
runs on mTrak's own timer at the last measured tempo (the song's BPM if no clock has arrived
yet), and the device's pulses and its Stop don't affect it. The device's next Start or
Continue takes over again.

## Sending clock

mTrak always sends Start, Continue and Stop to the output when playback starts and stops.
Turn on `Send MIDI clock` to also send 24 clock pulses per beat, so other devices can follow
mTrak's tempo.

While following an external clock, mTrak passes each incoming pulse on, so a device on
mTrak's output follows the same clock.
