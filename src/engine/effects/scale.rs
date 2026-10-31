use midi::Message;
use rand::RngExt;

use crate::{
    data::note::{NotePitch, SEMITONES_PER_OCTAVE},
    engine::{
        effects::{MidiEffect, Param, ParamKind, SentPitches, Timing, ValueLabel, note_event},
        notes::{MIDI_CHANNEL_COUNT, PITCH_COUNT},
    },
};

const ROOTS: &[&str] = &[
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];
const SCALE_NAMES: &[&str] = &[
    "Major",
    "Minor",
    "Harmonic minor",
    "Melodic minor",
    "Dorian",
    "Phrygian",
    "Lydian",
    "Mixolydian",
    "Locrian",
    "Major pentatonic",
    "Minor pentatonic",
    "Blues",
    "Whole tone",
];
/// Bit n set means n semitones above the root is in the scale.
const SCALES: [u16; 13] = [
    0b1010_1011_0101,
    0b0101_1010_1101,
    0b1001_1010_1101,
    0b1010_1010_1101,
    0b0110_1010_1101,
    0b0101_1010_1011,
    0b1010_1101_0101,
    0b0110_1011_0101,
    0b0101_0110_1011,
    0b0010_1001_0101,
    0b0100_1010_1001,
    0b0100_1110_1001,
    0b0101_0101_0101,
];
const STICKS: &[&str] = &["Down", "Up", "Random", "Filter"];
const TRANSPOSE: ParamKind = ParamKind::SignedNumber {
    min: -(SEMITONES_PER_OCTAVE as i8),
    max: SEMITONES_PER_OCTAVE as i8,
};

pub const ROOT: usize = 0;
pub const SCALE: usize = 1;
pub const STICK: usize = 2;
pub const TRANSPOSE_BY: usize = 3;

const PARAMS: &[Param] = &[
    Param {
        name: "Root",
        kind: ParamKind::Choice(ROOTS),
        default: 0,
        label: ValueLabel::None,
    },
    Param {
        name: "Scale",
        kind: ParamKind::Choice(SCALE_NAMES),
        default: 0,
        label: ValueLabel::None,
    },
    Param {
        name: "Between two",
        kind: ParamKind::Choice(STICKS),
        default: 0,
        label: ValueLabel::None,
    },
    Param {
        name: "Transpose",
        kind: TRANSPOSE,
        default: 0,
        label: ValueLabel::Semitones,
    },
];

pub struct ScaleEffect {
    root: u8,
    scale: u8,
    stick: u8,
    transpose: i8,
    sent: SentPitches,
    dropped: [[bool; PITCH_COUNT]; MIDI_CHANNEL_COUNT],
}
impl ScaleEffect {
    pub fn new() -> Self {
        return Self {
            root: PARAMS[ROOT].default,
            scale: PARAMS[SCALE].default,
            stick: PARAMS[STICK].default,
            transpose: PARAMS[TRANSPOSE_BY].default as i8,
            sent: SentPitches::new(),
            dropped: [[false; PITCH_COUNT]; MIDI_CHANNEL_COUNT],
        };
    }

    fn in_scale(&self, pitch: i16) -> bool {
        let degree = (pitch - self.root as i16).rem_euclid(SEMITONES_PER_OCTAVE as i16);
        return SCALES[self.scale as usize] & (1 << degree) != 0;
    }

    fn snap(&self, pitch: u8) -> Option<u8> {
        let pitch = pitch as i16 + self.transpose as i16;
        let in_range = |p: i16| p.clamp(NotePitch::MIN as i16, NotePitch::MAX as i16) as u8;
        if self.in_scale(pitch) {
            return Some(in_range(pitch));
        }
        if STICKS[self.stick as usize] == "Filter" {
            return None;
        }
        for distance in 1..SEMITONES_PER_OCTAVE as i16 {
            let (down, up) = (
                self.in_scale(pitch - distance),
                self.in_scale(pitch + distance),
            );
            let go_up = match (down, up) {
                (false, false) => continue,
                (true, false) => false,
                (false, true) => true,
                (true, true) => match STICKS[self.stick as usize] {
                    "Up" => true,
                    "Random" => rand::rng().random_bool(0.5),
                    _ => false,
                },
            };
            return Some(in_range(if go_up {
                pitch + distance
            } else {
                pitch - distance
            }));
        }
        return Some(in_range(pitch));
    }
}
impl Default for ScaleEffect {
    fn default() -> Self {
        return Self::new();
    }
}
impl MidiEffect for ScaleEffect {
    fn params(&self) -> &'static [Param] {
        return PARAMS;
    }
    fn param(&self, index: usize) -> u8 {
        return match index {
            ROOT => self.root,
            SCALE => self.scale,
            STICK => self.stick,
            TRANSPOSE_BY => self.transpose as u8,
            _ => 0,
        };
    }
    fn set_param(&mut self, index: usize, value: u8) {
        match index {
            ROOT => self.root = value,
            SCALE => self.scale = value,
            STICK => self.stick = value,
            TRANSPOSE_BY => self.transpose = value as i8,
            _ => {}
        }
    }
    fn process(&mut self, message: Message, _timing: Option<Timing>, out: &mut Vec<Message>) {
        let Some(note) = note_event(&message) else {
            out.push(message);
            return;
        };
        let (channel, played) = (note.channel, note.pitch.0);
        let dropped = &mut self.dropped[channel as usize][played as usize];
        if note.is_note_off {
            if std::mem::take(dropped) {
                return;
            }
            let sent = self.sent.note_off(channel, played);
            out.push(Message::NoteOff(channel, sent, note.velocity));
            return;
        }
        let snapped = self.snap(played);
        self.dropped[channel as usize][played as usize] = snapped.is_none();
        let Some(pitch) = snapped else {
            return;
        };
        self.sent.note_on(channel, played, pitch);
        out.push(Message::NoteOn(channel, pitch, note.velocity));
    }
    fn reset(&mut self, out: &mut Vec<Message>) {
        self.sent.release_changed(out);
        self.dropped = [[false; PITCH_COUNT]; MIDI_CHANNEL_COUNT];
    }
}

#[cfg(test)]
mod tests {
    use midi::Channel;

    use super::*;

    fn snapped(scale: &mut ScaleEffect, pitch: u8) -> Option<u8> {
        let mut out = vec![];
        scale.process(Message::NoteOn(Channel::Ch1, pitch, 100), None, &mut out);
        scale.process(Message::NoteOff(Channel::Ch1, pitch, 0), None, &mut out);
        return match out[..] {
            [] => None,
            [Message::NoteOn(_, on, _), Message::NoteOff(_, off, _)] if on == off => Some(on),
            _ => panic!("expected a note on and off on one pitch, got {out:?}"),
        };
    }

    #[test]
    fn c_major_keeps_white_keys_and_sends_halfway_notes_down() {
        let mut scale = ScaleEffect::new();
        assert_eq!(snapped(&mut scale, 60), Some(60));
        assert_eq!(snapped(&mut scale, 61), Some(60));
        assert_eq!(snapped(&mut scale, 66), Some(65));
    }

    #[test]
    fn halfway_notes_can_go_up_or_be_filtered() {
        let mut scale = ScaleEffect::new();
        scale.stick = 1;
        assert_eq!(snapped(&mut scale, 61), Some(62));
        scale.stick = 3;
        assert_eq!(snapped(&mut scale, 61), None);
        assert_eq!(snapped(&mut scale, 62), Some(62));
    }

    #[test]
    fn the_closest_note_wins_over_the_direction() {
        let mut scale = ScaleEffect::new();
        scale.root = 9;
        scale.scale = 10;
        scale.stick = 1;
        // A minor pentatonic: A C D E G, so A# is closer to A than to C
        assert_eq!(snapped(&mut scale, 70), Some(69));
    }

    #[test]
    fn transpose_comes_before_the_scale() {
        let mut scale = ScaleEffect::new();
        scale.transpose = 1;
        assert_eq!(snapped(&mut scale, 63), Some(64));
    }

    #[test]
    fn every_scale_starts_on_its_root() {
        assert!(SCALES.iter().all(|scale| scale & 1 == 1));
        assert_eq!(SCALES.len(), SCALE_NAMES.len());
    }
}
