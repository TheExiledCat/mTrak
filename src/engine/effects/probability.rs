use midi::Message;
use rand::RngExt;

use crate::{
    data::midi::VELOCITY_MAX,
    engine::{
        effects::{MidiEffect, Param, ParamKind, Timing, ValueLabel, note_event},
        notes::{MIDI_CHANNEL_COUNT, PITCH_COUNT},
    },
};

const CHANCE_FROM: &[&str] = &["Chance", "Velocity"];
const ROLLS: &[&str] = &["Each note", "Each chord"];

pub const CHANCE: usize = 0;
pub const CHANCE_SOURCE: usize = 1;
pub const ROLL: usize = 2;

const PARAMS: &[Param] = &[
    Param {
        name: "Chance",
        kind: ParamKind::Number {
            min: 0,
            max: u8::MAX,
        },
        default: 0xC0,
        label: ValueLabel::Percent,
    },
    Param {
        name: "Chance from",
        kind: ParamKind::Choice(CHANCE_FROM),
        default: 0,
        label: ValueLabel::None,
    },
    Param {
        name: "Roll for",
        kind: ParamKind::Choice(ROLLS),
        default: 0,
        label: ValueLabel::None,
    },
];

/// Like a Y on every note of the instrument.
pub struct ProbabilityEffect {
    chance: u8,
    chance_source: u8,
    roll: u8,
    /// Each channel's roll for the notes starting on this pulse, so a chord plays or drops
    /// as one. Forgotten on the next pulse.
    chord_rolls: [Option<bool>; MIDI_CHANNEL_COUNT],
    dropped: [[bool; PITCH_COUNT]; MIDI_CHANNEL_COUNT],
}
impl ProbabilityEffect {
    pub fn new() -> Self {
        return Self {
            chance: PARAMS[CHANCE].default,
            chance_source: PARAMS[CHANCE_SOURCE].default,
            roll: PARAMS[ROLL].default,
            chord_rolls: [None; MIDI_CHANNEL_COUNT],
            dropped: [[false; PITCH_COUNT]; MIDI_CHANNEL_COUNT],
        };
    }

    fn plays(&mut self, velocity: u8, channel: u8, timing: Option<Timing>) -> bool {
        let chord = self.roll == 1 && timing.is_some();
        if chord && let Some(plays) = self.chord_rolls[channel as usize] {
            return plays;
        }
        let chance = match self.chance_source {
            0 => self.chance as u32,
            _ => velocity as u32 * u8::MAX as u32 / VELOCITY_MAX as u32,
        };
        let plays = match chance {
            0 => false,
            chance if chance >= u8::MAX as u32 => true,
            chance => rand::rng().random_range(0..u8::MAX as u32) < chance,
        };
        if chord {
            self.chord_rolls[channel as usize] = Some(plays);
        }
        return plays;
    }
}
impl Default for ProbabilityEffect {
    fn default() -> Self {
        return Self::new();
    }
}
impl MidiEffect for ProbabilityEffect {
    fn params(&self) -> &'static [Param] {
        return PARAMS;
    }
    fn param(&self, index: usize) -> u8 {
        return match index {
            CHANCE => self.chance,
            CHANCE_SOURCE => self.chance_source,
            ROLL => self.roll,
            _ => 0,
        };
    }
    fn set_param(&mut self, index: usize, value: u8) {
        match index {
            CHANCE => self.chance = value,
            CHANCE_SOURCE => self.chance_source = value,
            ROLL => self.roll = value,
            _ => {}
        }
    }
    fn process(&mut self, message: Message, timing: Option<Timing>, out: &mut Vec<Message>) {
        let Some(note) = note_event(&message) else {
            out.push(message);
            return;
        };
        let (channel, pitch) = (note.channel as usize, note.pitch.0 as usize);
        if note.is_note_off {
            if !std::mem::take(&mut self.dropped[channel][pitch]) {
                out.push(message);
            }
            return;
        }
        let plays = self.plays(note.velocity, channel as u8, timing);
        self.dropped[channel][pitch] = !plays;
        if plays {
            out.push(message);
        }
    }
    fn tick(&mut self, _timing: Timing, _out: &mut Vec<Message>) {
        self.chord_rolls = [None; MIDI_CHANNEL_COUNT];
    }
    fn reset(&mut self, _out: &mut Vec<Message>) {
        self.chord_rolls = [None; MIDI_CHANNEL_COUNT];
        self.dropped = [[false; PITCH_COUNT]; MIDI_CHANNEL_COUNT];
    }
}

#[cfg(test)]
mod tests {
    use midi::Channel;

    use super::*;

    fn play(effect: &mut ProbabilityEffect, message: Message, timing: Option<Timing>) -> bool {
        let mut out = vec![];
        effect.process(message, timing, &mut out);
        return !out.is_empty();
    }

    #[test]
    fn full_and_no_chance() {
        let mut effect = ProbabilityEffect::new();
        let on = Message::NoteOn(Channel::Ch1, 60, 100);
        let off = Message::NoteOff(Channel::Ch1, 60, 0);
        effect.chance = u8::MAX;
        assert!(play(&mut effect, on.clone(), None));
        assert!(play(&mut effect, off.clone(), None));
        effect.chance = 0;
        assert!(!play(&mut effect, on, None));
        assert!(!play(&mut effect, off, None));
    }

    #[test]
    fn about_half_the_notes_play_at_half_chance() {
        let mut effect = ProbabilityEffect::new();
        effect.chance = 0x80;
        let played = (0..1000)
            .filter(|_| {
                let played = play(&mut effect, Message::NoteOn(Channel::Ch1, 60, 100), None);
                play(&mut effect, Message::NoteOff(Channel::Ch1, 60, 0), None);
                played
            })
            .count();
        assert!((350..650).contains(&played), "{played} of 1000 played");
    }

    #[test]
    fn a_chord_plays_or_drops_as_one() {
        let mut effect = ProbabilityEffect::new();
        effect.chance = 0x80;
        effect.roll = 1;
        for pulse in 0..100 {
            let timing = Some(Timing::new(pulse));
            let chord: Vec<bool> = [60, 64, 67]
                .into_iter()
                .map(|pitch| {
                    play(
                        &mut effect,
                        Message::NoteOn(Channel::Ch1, pitch, 100),
                        timing,
                    )
                })
                .collect();
            assert!(chord.iter().all(|&p| p == chord[0]));
            effect.tick(Timing::new(pulse), &mut vec![]);
        }
    }

    #[test]
    fn full_velocity_always_plays_when_chance_comes_from_velocity() {
        let mut effect = ProbabilityEffect::new();
        effect.chance_source = 1;
        assert!(play(
            &mut effect,
            Message::NoteOn(Channel::Ch1, 60, 127),
            None
        ));
    }

    #[test]
    fn a_chord_on_the_same_pulse_of_a_later_loop_rolls_again() {
        let mut effect = ProbabilityEffect::new();
        effect.chance = 0x80;
        effect.roll = 1;
        let timing = Some(Timing::new(0));
        let rolls: Vec<bool> = (0..200)
            .map(|_| {
                effect.tick(Timing::new(0), &mut vec![]);
                return play(&mut effect, Message::NoteOn(Channel::Ch1, 60, 100), timing);
            })
            .collect();
        assert!(rolls.contains(&true) && rolls.contains(&false));
    }
}
