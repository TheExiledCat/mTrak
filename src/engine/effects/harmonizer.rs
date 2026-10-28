use midi::{Channel, Message};

use crate::{
    data::{midi::VELOCITY_MAX, note::NotePitch},
    engine::{
        effects::{MidiEffect, NoteEvent, Param, ParamKind, Timing, note_event},
        notes::{Countdown, HeldNotes},
    },
};

const INTERVALS_COUNT: usize = 7;
const INVERSION_PARAM: usize = INTERVALS_COUNT;
const LEGATO_PARAM: usize = INVERSION_PARAM + 1;
const NO_INTERVAL: i8 = 0;
const INTERVAL_NAMES: [&str; INTERVALS_COUNT] = [
    "Interval 1",
    "Interval 2",
    "Interval 3",
    "Interval 4",
    "Interval 5",
    "Interval 6",
    "Interval 7",
];
const PARAMS: &[Param] = &params();

const fn params() -> [Param; LEGATO_PARAM + 1] {
    let mut params = [Param {
        name: "",
        kind: ParamKind::SignedNumber {
            min: i8::MIN,
            max: i8::MAX,
        },
        default: NO_INTERVAL as u8,
    }; LEGATO_PARAM + 1];
    let mut i = 0;
    while i < INTERVALS_COUNT {
        params[i].name = INTERVAL_NAMES[i];
        i += 1;
    }
    params[INVERSION_PARAM] = Param {
        name: "Inversion",
        kind: ParamKind::Number {
            min: 0,
            max: INTERVALS_COUNT as u8,
        },
        default: 0,
    };
    params[LEGATO_PARAM] = Param {
        name: "Legato (ticks)",
        kind: ParamKind::Number {
            min: 0,
            max: u8::MAX,
        },
        default: 0,
    };
    return params;
}
/// The pitches a held root sent, so its note off releases them even if the settings changed.
struct HeldChord {
    channel: Channel,
    root: u8,
    pitches: Vec<u8>,
}

/// A chord note waiting for its turn when legato spaces the notes out.
struct PendingNote {
    root: u8,
    note: NoteEvent,
}

pub struct Harmonizer {
    intervals: [i8; INTERVALS_COUNT],
    inversion: u8,
    legato: u8,
    chords: Vec<HeldChord>,
    pending: Countdown<PendingNote>,
    /// Chord notes due this pulse, played once the pulse's notes are in.
    due: Vec<PendingNote>,
    notes: HeldNotes,
}
impl Harmonizer {
    pub fn new() -> Self {
        return Self {
            intervals: [NO_INTERVAL; INTERVALS_COUNT],
            inversion: 0,
            legato: 0,
            chords: Vec::new(),
            pending: Countdown::new(),
            due: Vec::new(),
            notes: HeldNotes::new(),
        };
    }

    /// The root and its intervals, with the lowest raised an octave once per inversion.
    fn chord(&self, root: u8) -> Vec<u8> {
        let mut pitches = vec![root as i16];
        for interval in self.intervals {
            if interval != NO_INTERVAL {
                pitches.push(root as i16 + interval as i16);
            }
        }
        for _ in 0..self.inversion {
            let lowest = pitches.iter_mut().min().unwrap();
            *lowest += 12;
        }
        let mut pitches: Vec<u8> = pitches
            .into_iter()
            .filter(|pitch| (0..=VELOCITY_MAX as i16).contains(pitch))
            .map(|pitch| pitch as u8)
            .collect();
        pitches.sort_unstable();
        pitches.dedup();
        return pitches;
    }

    /// Releases what a root sounded and drops its notes still waiting. False if it had none.
    fn release(&mut self, note: NoteEvent, out: &mut Vec<Message>) -> bool {
        let root = note.pitch.0;
        let other_root =
            |pending: &PendingNote| pending.note.channel != note.channel || pending.root != root;
        self.pending.remove(|pending| !other_root(pending));
        self.due.retain(other_root);
        let Some(index) = self
            .chords
            .iter()
            .position(|chord| chord.channel == note.channel && chord.root == root)
        else {
            return false;
        };
        for pitch in self.chords.remove(index).pitches {
            self.notes.note_off(note.channel, pitch, note.velocity, out);
        }
        return true;
    }
}
impl MidiEffect for Harmonizer {
    /// While stopped there are no ticks to wait for, so legato is ignored.
    fn process(&mut self, message: Message, timing: Option<Timing>, out: &mut Vec<Message>) {
        let Some(note) = note_event(&message) else {
            out.push(message);
            return;
        };
        let released = self.release(note, out);
        if note.is_note_off {
            if !released {
                out.push(message);
            }
            return;
        }
        let root = note.pitch.0;
        let spacing = match timing {
            Some(_) => self.legato as u32,
            None => 0,
        };
        let mut sounded = Vec::new();
        for (index, pitch) in self.chord(root).into_iter().enumerate() {
            let delay = index as u32 * spacing;
            if delay == 0 {
                self.notes.note_on(note.channel, pitch, note.velocity, out);
                sounded.push(pitch);
            } else {
                let note = NoteEvent {
                    pitch: NotePitch(pitch),
                    ..note
                };
                self.pending.push(PendingNote { root, note }, delay);
            }
        }
        self.chords.push(HeldChord {
            channel: note.channel,
            root,
            pitches: sounded,
        });
    }

    fn tick(&mut self, _timing: Timing, _out: &mut Vec<Message>) {
        self.due.extend(self.pending.tick());
    }

    fn after_pulse(&mut self, _timing: Timing, out: &mut Vec<Message>) {
        for PendingNote { root, note } in std::mem::take(&mut self.due) {
            let chord = self
                .chords
                .iter_mut()
                .find(|chord| chord.channel == note.channel && chord.root == root);
            if let Some(chord) = chord {
                chord.pitches.push(note.pitch.0);
                self.notes
                    .note_on(note.channel, note.pitch.0, note.velocity, out);
            }
        }
    }

    fn reset(&mut self, out: &mut Vec<Message>) {
        self.pending.clear();
        self.due.clear();
        self.chords.clear();
        self.notes.release_all(out);
    }

    fn params(&self) -> &'static [Param] {
        return PARAMS;
    }

    fn param(&self, index: usize) -> u8 {
        return match index {
            i if i < INTERVALS_COUNT => self.intervals[i] as u8,
            INVERSION_PARAM => self.inversion,
            LEGATO_PARAM => self.legato,
            _ => 0,
        };
    }

    fn set_param(&mut self, index: usize, value: u8) {
        match index {
            i if i < INTERVALS_COUNT => self.intervals[i] = value as i8,
            INVERSION_PARAM => self.inversion = value,
            LEGATO_PARAM => self.legato = value,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pulse as the engine runs it: the tick, then what waits for the pulse's notes.
    fn run_pulse(effect: &mut dyn MidiEffect, pulse: u32, out: &mut Vec<Message>) {
        effect.tick(Timing::new(pulse), out);
        effect.after_pulse(Timing::new(pulse), out);
    }

    fn harmonizer(intervals: &[i8], inversion: u8) -> Harmonizer {
        let mut harmonizer = Harmonizer::new();
        for (index, interval) in intervals.iter().enumerate() {
            harmonizer.set_param(index, *interval as u8);
        }
        harmonizer.set_param(INVERSION_PARAM, inversion);
        return harmonizer;
    }

    fn pitches(messages: &[Message]) -> Vec<u8> {
        return messages
            .iter()
            .filter_map(|message| note_event(message).map(|note| note.pitch.0))
            .collect();
    }

    #[test]
    fn inversions_raise_the_lowest_notes_an_octave() {
        assert_eq!(harmonizer(&[4, 7], 0).chord(60), vec![60, 64, 67]);
        assert_eq!(harmonizer(&[4, 7], 1).chord(60), vec![64, 67, 72]);
        assert_eq!(harmonizer(&[4, 7], 2).chord(60), vec![67, 72, 76]);
        assert_eq!(harmonizer(&[4, 7], 3).chord(60), vec![72, 76, 79]);
    }

    #[test]
    fn chords_drop_pitches_out_of_range_and_repeats() {
        assert_eq!(harmonizer(&[12, -12, 12], 0).chord(120), vec![108, 120]);
    }

    #[test]
    fn note_offs_release_what_was_played_after_settings_change() {
        let mut harmonizer = harmonizer(&[4, 7], 0);
        let mut out = vec![];
        harmonizer.process(Message::NoteOn(Channel::Ch1, 60, 100), None, &mut out);
        harmonizer.set_param(0, 3);
        out.clear();
        harmonizer.process(Message::NoteOff(Channel::Ch1, 60, 0), None, &mut out);
        assert_eq!(pitches(&out), vec![60, 64, 67]);
        assert!(harmonizer.chords.is_empty());
    }

    fn on(pitch: u8) -> Message {
        return Message::NoteOn(Channel::Ch1, pitch, 100);
    }

    #[test]
    fn legato_spaces_the_chord_out_low_to_high() {
        let mut harmonizer = harmonizer(&[4, 7], 0);
        harmonizer.set_param(LEGATO_PARAM, 2);
        let timing = Some(Timing::new(0));
        let mut out = vec![];
        harmonizer.process(on(60), timing, &mut out);
        assert_eq!(out, vec![on(60)]);
        let mut played = vec![];
        for pulse in 1..=4 {
            out.clear();
            run_pulse(&mut harmonizer, pulse, &mut out);
            played.push(out.clone());
        }
        assert_eq!(played, vec![vec![], vec![on(64)], vec![], vec![on(67)]]);
    }

    #[test]
    fn legato_is_ignored_while_stopped() {
        let mut harmonizer = harmonizer(&[4, 7], 0);
        harmonizer.set_param(LEGATO_PARAM, 2);
        let mut out = vec![];
        harmonizer.process(on(60), None, &mut out);
        assert_eq!(out, vec![on(60), on(64), on(67)]);
    }

    #[test]
    fn a_note_off_drops_notes_still_waiting() {
        let mut harmonizer = harmonizer(&[4, 7], 0);
        harmonizer.set_param(LEGATO_PARAM, 2);
        let mut out = vec![];
        harmonizer.process(on(60), Some(Timing::new(0)), &mut out);
        run_pulse(&mut harmonizer, 1, &mut out);
        run_pulse(&mut harmonizer, 2, &mut out);
        out.clear();
        harmonizer.process(
            Message::NoteOff(Channel::Ch1, 60, 0),
            Some(Timing::new(2)),
            &mut out,
        );
        assert_eq!(pitches(&out), vec![60, 64]);
        for pulse in 3..6 {
            run_pulse(&mut harmonizer, pulse, &mut out);
        }
        assert_eq!(pitches(&out), vec![60, 64]);
    }

    #[test]
    fn a_pitch_two_chords_share_is_released_by_the_last_one() {
        let mut harmonizer = harmonizer(&[7], 0);
        let mut out = vec![];
        harmonizer.process(on(60), None, &mut out);
        harmonizer.process(on(67), None, &mut out);
        out.clear();
        harmonizer.process(Message::NoteOff(Channel::Ch1, 60, 0), None, &mut out);
        assert_eq!(pitches(&out), vec![60]);
        out.clear();
        harmonizer.process(Message::NoteOff(Channel::Ch1, 67, 0), None, &mut out);
        assert_eq!(pitches(&out), vec![67, 74]);
    }

    #[test]
    fn every_param_has_its_own_name() {
        let mut names: Vec<&str> = PARAMS.iter().map(|param| param.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), PARAMS.len());
    }

    #[test]
    fn reset_releases_held_chords_and_other_messages_pass() {
        let mut harmonizer = harmonizer(&[7], 0);
        let mut out = vec![];
        let bend = Message::PitchBend(Channel::Ch1, 0x2000);
        harmonizer.process(bend.clone(), None, &mut out);
        assert_eq!(out, vec![bend]);
        harmonizer.process(Message::NoteOn(Channel::Ch1, 60, 100), None, &mut out);
        out.clear();
        harmonizer.reset(&mut out);
        assert_eq!(
            out,
            vec![
                Message::NoteOff(Channel::Ch1, 60, 0),
                Message::NoteOff(Channel::Ch1, 67, 0),
            ]
        );
    }
}
