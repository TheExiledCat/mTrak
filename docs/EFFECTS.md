# Effects

Each instrument has a chain of live MIDI effects that change its notes on the way out: an
arpeggiator, chords, an EQ for velocities, an LFO and an echo. They work on the notes the pattern plays
and on notes you play live, and they're saved with the project.

## Adding effects

`Ctrl+R` opens the active instrument's dialog. Its `Effects` button opens the list of the
instrument's effects:

| Key                     | Does                                                      |
| ----------------------- | --------------------------------------------------------- |
| `Space`                 | Opens the selected effect's settings, or on `Add effect` adds one |
| `Del`                   | Removes the selected effect                               |
| `Shift+Up` / `Shift+Down` | Moves the selected effect up or down the chain           |
| `X`                     | Turns the selected effect off or on                       |
| `Enter`                 | Closes the dialog                                         |

These keys can be rebound like any other, see [CONFIG.md](CONFIG.md).

Changes are heard straight away, also while the song plays. The instrument, effects and
effect settings dialogs close with `Enter` only: there's nothing to cancel, as every change is
already made.

An effect that's turned off keeps its place and settings, and notes pass it unchanged.

## How notes go through

Notes go through the effects from the top of the list down, each effect getting what the one
above it sends. The order matters: with an Arp above an EQ, the EQ changes every note the arp
plays; with the EQ above, it only sees the note you play.

Effects get the notes after mutes, transposes and the global velocity, just before they're
sent.

Times are in ticks. There are 24 ticks in a beat, so at the default speed of 6 a row is 6
ticks. The Arp, Chord legato, LFO and Echo count ticks, so they only run while the song plays:
while stopped, the Arp and Echo let notes through as they are, the Chord plays all its notes
at once and the LFO stays at the start of its wave.

Numbers in the settings are hex unless `Values are hex` is turned off in the Config dialog.
Signed values go below 0, e.g. `-0C` is 12 semitones down.

## Arp

Plays a held note as a looping sequence of steps, one note at a time, for as long as the note
is held. Each held note gets its own arp.

| Setting      | Values     | Default | Does |
| ------------ | ---------- | ------- | ---- |
| Step 1-8     | `-80`-`7F` | `00`    | Semitones added to the held note. `00` plays the held note itself |
| Length       | `1`-`8`    | `4`     | How many steps play before it starts over |
| Rate (ticks) | `01`-`FF`  | `06`    | How long each step lasts |
| Gate (ticks) | `-80`-`7F` | `00`    | How notes overlap. `00`: each note ends as the next starts. Negative: each next note starts that many ticks early, at most one tick less than the rate. Positive: each note ends that many ticks after the next one starts |

A step whose note falls outside the MIDI range is a rest.

## Chord

Plays a chord for every note: the note itself and up to seven intervals above or below it.

| Setting        | Values     | Default | Does |
| -------------- | ---------- | ------- | ---- |
| Interval 1-7   | `-80`-`7F` | `00`    | Semitones from the note. `00` is unused |
| Inversion      | `0`-`7`    | `0`     | Raises the chord's lowest note an octave, once per step. A chord has one inversion fewer than it has notes, so higher values play its last one |
| Legato (ticks) | `00`-`FF`  | `00`    | Spaces the chord's notes out from low to high, this many ticks apart |

A note in the chord twice is played once, and chord notes outside the MIDI range aren't played.
When two chords share a note, it keeps playing until both are released.

An Arp below a Chord gives each chord note an arp of its own, playing side by side.

## EQ

Changes how loud notes are by their pitch, and fades out notes above or below a cutoff.

| Setting        | Values     | Default | Does |
| -------------- | ---------- | ------- | ---- |
| Low band up to | A note     | `F#2`   | Notes up to here are in the low band |
| Mid band up to | A note     | `C-6`   | Notes above the low band up to here are in the mid band; the rest are in the high band |
| Low velocity   | `-80`-`7F` | `00`    | Scales the velocity of the low band: `00` leaves it, `-80` silences it, `7F` about doubles it |
| Mid velocity   | `-80`-`7F` | `00`    | The same for the mid band |
| High velocity  | `-80`-`7F` | `00`    | The same for the high band |
| High pass      | A note     | `C--1`  | Notes below it get quieter |
| Low pass       | A note     | `G-9`   | Notes above it get quieter |
| Filter slope   | `00`-`FF`  | `08`    | How much velocity a note loses per octave past the high or low pass |

A note that ends up at velocity 0 isn't played.

## LFO

Moves a value up and down in a repeating wave: the velocity of notes, the pitch bend, or a CC.

| Setting      | Values     | Default    | Does |
| ------------ | ---------- | ---------- | ---- |
| Rate (ticks) | `01`-`FF`  | `06`       | How long one cycle of the wave lasts |
| Depth        | `-80`-`7F` | `20`       | How far the wave reaches either way, in velocity or CC steps. For pitch bend, `7F` reaches the ends of the synth's bend range. Negative turns the wave upside down |
| Shape        | A shape    | Sine       | Sine, Triangle, Saw up, Saw down, Square, or Random: a new random value every cycle |
| Retrigger    | On or off  | Off        | Starts the wave over with every note. Off, the wave follows the pattern, starting with its first row |
| Target       | A target   | Velocity   | Velocity, Pitch bend or CC |
| CC           | `00`-`7F`  | `01`       | The CC the wave moves, with the CC target |
| CC center    | `00`-`7F`  | `40`       | The CC's value in the middle of the wave |

With the Velocity target, each note's velocity is changed as it starts. Pitch bend and CC are
sent on every tick while notes are held, and just before each note, so no note starts with an
old value. When playback stops, the bend or CC is set back to the middle of the wave.

With Retrigger on and the Velocity target, every note starts at the beginning of the wave, so
they all get the same change.

## Echo

Plays every note again after a while, and again, each repeat quieter than the one before, until
it fades out. Each repeat lasts as long as the note was held.

| Setting      | Values    | Default | Does |
| ------------ | --------- | ------- | ---- |
| Rate (ticks) | `01`-`FF` | `0C`    | The time between repeats. `0C` is half a beat |
| Feedback     | `00`-`FF` | `80`    | How loud each repeat is next to the one before: `80` is half, `FF` almost the same, `00` plays no repeats |

The higher the feedback, the more repeats: at `FF` a note repeats about as many times as its
velocity, up to 127.

A repeat of a note that's still sounding ends it and plays it again, as does a new note on a
repeat that's still sounding. Every note still ends: each repeat on its own time, the note
only if nothing has played over it.

Put the Echo above an Arp or Chord to echo only the notes you play, or below to echo every note
they make.
