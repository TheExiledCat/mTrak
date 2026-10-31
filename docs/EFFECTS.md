# Effects

Each instrument has a chain of live MIDI effects that change its notes on the way out: an
arpeggiator, chords, an EQ for velocities, an LFO, an envelope, an echo, swing, a randomizer,
a scale, note to CC and probability. They work on the notes the pattern plays
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
| `Esc` / `Enter`         | Closes the dialog                                         |

These keys can be rebound like any other, see [CONFIG.md](CONFIG.md).

Changes are heard straight away, also while the song plays. There's nothing to cancel, as every
change is already made, so `Esc` closes the instrument, effects, effect settings and CC mapping
dialogs just like `Enter`.

An effect that's turned off keeps its place and settings, and notes pass it unchanged.

## Modulation

Any setting can follow a CC, so pattern commands can change it while the song plays. In an
effect's settings, `CC mapping` lists every setting with the CC it follows, or `Off`; each CC
is shown with its name in the MIDI spec, e.g. `01 Mod wheel`, and typing its number picks it.
A CC sent on the instrument's channel then sets the setting instead of reaching the synth:
`00` is its lowest value and `7F` its highest. Choices are split evenly over the CC's values,
and a toggle is on from `40`.

```
---|1|--|S14   select CC 20, which the Echo's Feedback follows
---|1|--|V00   slide the feedback from nothing ...
---|1|--|000
---|1|--|MFF   ... to full
```

Every pattern command for CCs works, so `V` and `J` slide a setting smoothly. A CC also comes
from an LFO with the CC target, which then modulates the effects below it in the list, but not
the ones above. With [soft thru](SYNC.md#soft-thru) on, a knob on a MIDI controller does it
too. Settings can share a CC, to change together.

A CC any setting follows is never sent to the synth on that instrument's channel, so pick
ones the synth doesn't use; CCs 102 to 119 are free in MIDI. Changes from a CC only last while
the song plays: stopping or starting it puts every setting back to what the dialog shows.

## How notes go through

Notes go through the effects from the top of the list down, each effect getting what the one
above it sends. The order matters: with an Arp above an EQ, the EQ changes every note the arp
plays; with the EQ above, it only sees the note you play.

Effects get the notes after mutes, transposes and the global velocity, just before they're
sent.

Times are in ticks. There are 24 ticks in a beat, so at the default speed of 6 a row is 6
ticks. The Arp, Chord legato, LFO, Envelope and Echo count ticks, so they only run while the song plays:
while stopped, the Arp and Echo let notes through as they are, the Chord plays all its notes
at once, the LFO stays at the start of its wave and the Envelope sends nothing.

Numbers in the settings are hex unless `Values are hex` is turned off in the Config dialog.
Signed values go below 0, e.g. `-0C` is 12 semitones down.
Some settings show what their number means after it: a percentage, a note length for ticks
(`06` is `1/16`, `08` is `1/8T`) or an interval for semitones (`07` is `5th`, `-0C` is `-oct`).

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

## Envelope

Moves the pitch bend or a CC with an ADSR envelope each time a note starts: up to its peak in
the attack, down to the sustain level in the decay, held there while notes are held, and back
to rest in the release once the last note ends.

| Setting         | Values     | Default | Does |
| --------------- | ---------- | ------- | ---- |
| Attack (ticks)  | `00`-`FF`  | `00`    | How long it takes to rise to the peak. `00` starts the note at the peak |
| Decay (ticks)   | `00`-`FF`  | `0C`    | How long it takes to fall from the peak to the sustain level |
| Sustain         | `00`-`FF`  | `80`    | The level held while notes are held: `FF` is the peak, `80` halfway, `00` back at rest |
| Release (ticks) | `00`-`FF`  | `0C`    | How long it takes to fall back to rest after the last note ends |
| Overlap         | A choice   | Restart  | What a note does while the envelope is still moving, see below |
| Depth           | `-80`-`7F` | `7F`    | How far the peak is from rest, in CC steps. For pitch bend, `7F` reaches the top of the synth's bend range. Negative moves it down instead |
| Target          | A target   | CC      | Pitch bend or CC |
| CC              | `00`-`7F`  | `01`    | The CC it moves, with the CC target |
| CC base         | `00`-`7F`  | `00`    | The CC's value at rest |

A note that starts while the envelope is still moving, during another note or its release,
does what `Overlap` says:

- `Restart`: jumps back to rest and starts the attack over, like a new note on its own.
- `Continue`: starts the attack from wherever the envelope got to, so it never jumps.

The value for the start of the attack is sent just before the note. When playback stops, the bend or CC goes back to rest.

## Echo

Plays every note again after a while, and again, each repeat quieter than the one before, until
it fades out. Each repeat lasts as long as the note was held.

| Setting      | Values    | Default | Does |
| ------------ | --------- | ------- | ---- |
| Rate (ticks) | `01`-`FF` | `0C`    | The time between repeats. `0C` is half a beat |
| Feedback     | `00`-`FF` | `80`    | How loud each repeat is next to the one before: `80` is half, `FF` almost the same, `00` plays no repeats |
| Dry/wet      | `00`-`FF` | `80`    | At `80` the note and its repeats play at full velocity. Lower fades the repeats out, down to none at `00`; higher fades the note itself out, down to only the repeats at `FF` |

The higher the feedback, the more repeats: at `FF` a note repeats about as many times as its
velocity, up to 127.

A repeat of a note that's still sounding ends it and plays it again, as does a new note on a
repeat that's still sounding. Every note still ends: each repeat on its own time, the note
only if nothing has played over it.

Put the Echo above an Arp or Chord to echo only the notes you play, or below to echo every note
they make.

## Swing

Pushes every second line of a grid later, for a shuffled feel. Notes on the grid's other
lines and notes off the grid stay where they are.

| Setting | Values      | Default | Does |
| ------- | ----------- | ------- | ---- |
| Grid    | `1/4`-`1/32` | `1/16` | Which lines swing: with `1/16`, the second, fourth, sixth... 16th of each beat |
| Swing   | `80`-`FC`   | `AA`    | How far the swung line moves, as a share of two grid steps: `80` (50%) is straight, `AA` (67%) a triplet shuffle, higher pushes it towards the next line |
| Accent  | `00`-`FF`   | `80`    | Below `80`, the swung notes get quieter; above it, the notes on the beat do |

A swung note keeps its length: its note off moves just as much. Swing only works while the song
plays, as it needs the ticks; notes you play while stopped pass through.

## Randomizer

Moves each note by a random amount. It changes one thing; put two Randomizers on an
instrument to change two.

| Setting       | Values    | Default    | Does |
| ------------- | --------- | ---------- | ---- |
| Randomize     | Velocity, Pitch, Octave | Velocity | What it changes |
| Down by up to | `00`-`7F` | `10`       | The furthest down a note moves: in velocity steps, semitones or octaves |
| Up by up to   | `00`-`7F` | `10`       | The furthest up |
| Chance        | `00`-`FF` | `FF`       | How often a note is moved at all; `FF` is every time |

A note moved to another pitch ends on that pitch, so nothing hangs.

## Scale

Keeps notes in a scale: a note outside it moves to the closest note in it.

| Setting     | Values    | Default | Does |
| ----------- | --------- | ------- | ---- |
| Root        | C-B       | C       | The scale's first note |
| Scale       | Major, Minor, Harmonic minor, Melodic minor, Dorian, Phrygian, Lydian, Mixolydian, Locrian, Major pentatonic, Minor pentatonic, Blues, Whole tone | Major | |
| Between two | Down, Up, Random, Filter | Down | Where a note goes when it's as close to the scale note below as to the one above. Filter drops every note outside the scale instead |
| Transpose   | `-0C`-`0C` | `00`   | Semitones added before the scale is applied |

## Note to CC

Sends a message for each note, made from its pitch or velocity, e.g. to open a filter more for
harder hits.

| Setting    | Values    | Default | Does |
| ---------- | --------- | ------- | ---- |
| Value from | Pitch, Velocity | Pitch | What the value is |
| Send as    | CC, Pitch bend, Aftertouch, Program change | CC | What it sends. Pitch bend uses the whole range, so pitch `40` is the centre |
| CC         | `00`-`7F` | `4A`    | The CC it sends, for `Send as` CC |
| Play notes | Off, On   | On      | Off sends only the message, without the note |

The message goes out just before the note, so the synth has it when the note starts.

## Probability

Lets each note through with a chance, like a `Y` command on every note of the instrument.

| Setting     | Values    | Default | Does |
| ----------- | --------- | ------- | ---- |
| Chance      | `00`-`FF` | `C0`    | How often a note plays: `FF` always, `80` half the time, `00` never |
| Chance from | Chance, Velocity | Chance | Velocity uses each note's own velocity as its chance instead, so soft notes are rare and notes at `7F` always play |
| Roll for    | Each note, Each chord | Each note | Each chord rolls once for the notes that start together, so a chord plays or drops as a whole |

A dropped note's note off is dropped too.
