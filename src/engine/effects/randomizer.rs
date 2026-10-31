use midi::Message;
use rand::RngExt;

use crate::{
    data::{
        midi::VELOCITY_MAX,
        note::{NotePitch, SEMITONES_PER_OCTAVE},
    },
    engine::effects::{MidiEffect, Param, ParamKind, SentPitches, Timing, ValueLabel, note_event},
};

const TARGETS: &[&str] = &["Velocity", "Pitch", "Octave"];
const RANGE: ParamKind = ParamKind::Number {
    min: 0,
    max: VELOCITY_MAX,
};

pub const TARGET: usize = 0;
pub const DOWN: usize = 1;
pub const UP: usize = 2;
pub const CHANCE: usize = 3;

const PARAMS: &[Param] = &[
    Param {
        name: "Randomize",
        kind: ParamKind::Choice(TARGETS),
        default: 0,
        label: ValueLabel::None,
    },
    Param {
        name: "Down by up to",
        kind: RANGE,
        default: 0x10,
        label: ValueLabel::None,
    },
    Param {
        name: "Up by up to",
        kind: RANGE,
        default: 0x10,
        label: ValueLabel::None,
    },
    Param {
        name: "Chance",
        kind: ParamKind::Number {
            min: 0,
            max: u8::MAX,
        },
        default: u8::MAX,
        label: ValueLabel::Percent,
    },
];

/// One target per instance; stack two to randomize two things.
pub struct RandomizerEffect {
    target: u8,
    down: u8,
    up: u8,
    chance: u8,
    sent: SentPitches,
}
impl RandomizerEffect {
    pub fn new() -> Self {
        return Self {
            target: PARAMS[TARGET].default,
            down: PARAMS[DOWN].default,
            up: PARAMS[UP].default,
            chance: PARAMS[CHANCE].default,
            sent: SentPitches::new(),
        };
    }

    fn offset(&self) -> i16 {
        let mut rng = rand::rng();
        if self.chance < u8::MAX && rng.random_range(0..u8::MAX) >= self.chance {
            return 0;
        }
        return rng.random_range(-(self.down as i16)..=self.up as i16);
    }
}
impl Default for RandomizerEffect {
    fn default() -> Self {
        return Self::new();
    }
}
impl MidiEffect for RandomizerEffect {
    fn params(&self) -> &'static [Param] {
        return PARAMS;
    }
    fn param(&self, index: usize) -> u8 {
        return match index {
            TARGET => self.target,
            DOWN => self.down,
            UP => self.up,
            CHANCE => self.chance,
            _ => 0,
        };
    }
    fn set_param(&mut self, index: usize, value: u8) {
        match index {
            TARGET => self.target = value,
            DOWN => self.down = value,
            UP => self.up = value,
            CHANCE => self.chance = value,
            _ => {}
        }
    }
    fn process(&mut self, message: Message, _timing: Option<Timing>, out: &mut Vec<Message>) {
        let Some(note) = note_event(&message) else {
            out.push(message);
            return;
        };
        let (channel, played) = (note.channel, note.pitch.0);
        if note.is_note_off {
            if let Some(sent) = self.sent.note_off(channel, played) {
                out.push(Message::NoteOff(channel, sent, note.velocity));
            }
            return;
        }
        let offset = self.offset();
        let pitch_range = NotePitch::MIN as i16..=NotePitch::MAX as i16;
        let (pitch, velocity) = match self.target {
            0 => (
                played,
                (note.velocity as i16 + offset).clamp(1, VELOCITY_MAX as i16) as u8,
            ),
            1 => (
                (played as i16 + offset).clamp(*pitch_range.start(), *pitch_range.end()) as u8,
                note.velocity,
            ),
            _ => {
                let octaves = offset * SEMITONES_PER_OCTAVE as i16;
                let pitch = played as i16 + octaves;
                let pitch = if pitch_range.contains(&pitch) {
                    pitch as u8
                } else {
                    played
                };
                (pitch, note.velocity)
            }
        };
        self.sent.note_on(channel, played, pitch);
        out.push(Message::NoteOn(channel, pitch, velocity));
    }
    fn reset(&mut self, out: &mut Vec<Message>) {
        self.sent.release_changed(out);
    }
}

#[cfg(test)]
mod tests {
    use midi::Channel;

    use super::*;

    fn play(randomizer: &mut RandomizerEffect, message: Message) -> Vec<Message> {
        let mut out = vec![];
        randomizer.process(message, None, &mut out);
        return out;
    }

    fn played(randomizer: &mut RandomizerEffect) -> (u8, u8) {
        let out = play(randomizer, Message::NoteOn(Channel::Ch1, 60, 100));
        let [Message::NoteOn(_, pitch, velocity)] = out[..] else {
            panic!("expected one note on, got {out:?}");
        };
        play(randomizer, Message::NoteOff(Channel::Ch1, 60, 0));
        return (pitch, velocity);
    }

    #[test]
    fn velocity_stays_within_its_ranges() {
        let mut randomizer = RandomizerEffect::new();
        randomizer.down = 20;
        randomizer.up = 5;
        for _ in 0..200 {
            let (pitch, velocity) = played(&mut randomizer);
            assert_eq!(pitch, 60);
            assert!((80..=105).contains(&velocity));
        }
    }

    #[test]
    fn octaves_move_by_whole_octaves() {
        let mut randomizer = RandomizerEffect::new();
        randomizer.target = 2;
        randomizer.down = 1;
        randomizer.up = 1;
        for _ in 0..100 {
            assert!([48, 60, 72].contains(&played(&mut randomizer).0));
        }
    }

    #[test]
    fn note_off_goes_to_the_pitch_that_was_sent() {
        let mut randomizer = RandomizerEffect::new();
        randomizer.target = 1;
        randomizer.down = 12;
        randomizer.up = 12;
        let out = play(&mut randomizer, Message::NoteOn(Channel::Ch1, 60, 100));
        let [Message::NoteOn(_, sent, _)] = out[..] else {
            panic!("expected one note on");
        };
        assert_eq!(
            play(&mut randomizer, Message::NoteOff(Channel::Ch1, 60, 0)),
            vec![Message::NoteOff(Channel::Ch1, sent, 0)]
        );
    }

    #[test]
    fn no_chance_leaves_notes_alone() {
        let mut randomizer = RandomizerEffect::new();
        randomizer.chance = 0;
        assert_eq!(played(&mut randomizer), (60, 100));
    }
}
