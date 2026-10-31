use midi::Message;

use crate::{
    data::midi::CC_MAX,
    engine::{
        effects::{MidiEffect, Param, ParamKind, Timing, ValueLabel, note_event},
        notes::{MIDI_CHANNEL_COUNT, PITCH_COUNT},
    },
};

const SOURCES: &[&str] = &["Pitch", "Velocity"];
const DESTINATIONS: &[&str] = &["CC", "Pitch bend", "Aftertouch", "Program change"];

pub const SOURCE: usize = 0;
pub const DESTINATION: usize = 1;
pub const CC: usize = 2;
pub const PASS_NOTES: usize = 3;

const PARAMS: &[Param] = &[
    Param {
        name: "Value from",
        kind: ParamKind::Choice(SOURCES),
        default: 0,
        label: ValueLabel::None,
    },
    Param {
        name: "Send as",
        kind: ParamKind::Choice(DESTINATIONS),
        default: 0,
        label: ValueLabel::None,
    },
    Param {
        name: "CC",
        kind: ParamKind::Cc,
        default: 74,
        label: ValueLabel::None,
    },
    Param {
        name: "Play notes",
        kind: ParamKind::Toggle,
        default: 1,
        label: ValueLabel::None,
    },
];

pub struct NoteToCcEffect {
    source: u8,
    destination: u8,
    cc: u8,
    pass_notes: bool,
    /// Turning notes off while one is held shouldn't leave it hanging.
    passed: [[bool; PITCH_COUNT]; MIDI_CHANNEL_COUNT],
}
impl NoteToCcEffect {
    pub fn new() -> Self {
        return Self {
            source: PARAMS[SOURCE].default,
            destination: PARAMS[DESTINATION].default,
            cc: PARAMS[CC].default,
            pass_notes: PARAMS[PASS_NOTES].default == 1,
            passed: [[false; PITCH_COUNT]; MIDI_CHANNEL_COUNT],
        };
    }
}
impl Default for NoteToCcEffect {
    fn default() -> Self {
        return Self::new();
    }
}
impl MidiEffect for NoteToCcEffect {
    fn params(&self) -> &'static [Param] {
        return PARAMS;
    }
    fn param(&self, index: usize) -> u8 {
        return match index {
            SOURCE => self.source,
            DESTINATION => self.destination,
            CC => self.cc,
            PASS_NOTES => self.pass_notes as u8,
            _ => 0,
        };
    }
    fn set_param(&mut self, index: usize, value: u8) {
        match index {
            SOURCE => self.source = value,
            DESTINATION => self.destination = value,
            CC => self.cc = value,
            PASS_NOTES => self.pass_notes = value == 1,
            _ => {}
        }
    }
    fn process(&mut self, message: Message, _timing: Option<Timing>, out: &mut Vec<Message>) {
        let Some(note) = note_event(&message) else {
            out.push(message);
            return;
        };
        let passed = &mut self.passed[note.channel as usize][note.pitch.0 as usize];
        if note.is_note_off {
            if std::mem::take(passed) {
                out.push(message);
            }
            return;
        }
        let value = match self.source {
            0 => note.pitch.0,
            _ => note.velocity,
        }
        .min(CC_MAX);
        let channel = note.channel;
        out.push(match self.destination {
            0 => Message::ControlChange(channel, self.cc, value),
            1 => Message::PitchBend(channel, (value as u16) << 7),
            2 => Message::ChannelPressure(channel, value),
            _ => Message::ProgramChange(channel, value),
        });
        *passed = self.pass_notes;
        if self.pass_notes {
            out.push(message);
        }
    }
    fn reset(&mut self, _out: &mut Vec<Message>) {
        self.passed = [[false; PITCH_COUNT]; MIDI_CHANNEL_COUNT];
    }
}

#[cfg(test)]
mod tests {
    use midi::Channel;

    use super::*;
    use crate::data::midi::BEND_CENTER;

    fn play(effect: &mut NoteToCcEffect, message: Message) -> Vec<Message> {
        let mut out = vec![];
        effect.process(message, None, &mut out);
        return out;
    }

    #[test]
    fn notes_send_their_pitch_as_a_cc_before_playing() {
        let mut effect = NoteToCcEffect::new();
        let on = Message::NoteOn(Channel::Ch2, 60, 100);
        assert_eq!(
            play(&mut effect, on.clone()),
            vec![Message::ControlChange(Channel::Ch2, 74, 60), on]
        );
    }

    #[test]
    fn velocity_as_aftertouch_without_the_note() {
        let mut effect = NoteToCcEffect::new();
        effect.source = 1;
        effect.destination = 2;
        effect.pass_notes = false;
        assert_eq!(
            play(&mut effect, Message::NoteOn(Channel::Ch1, 60, 100)),
            vec![Message::ChannelPressure(Channel::Ch1, 100)]
        );
        assert!(play(&mut effect, Message::NoteOff(Channel::Ch1, 60, 0)).is_empty());
    }

    #[test]
    fn pitch_bend_spans_the_whole_range() {
        let mut effect = NoteToCcEffect::new();
        effect.destination = 1;
        effect.pass_notes = false;
        assert_eq!(
            play(&mut effect, Message::NoteOn(Channel::Ch1, 64, 100)),
            vec![Message::PitchBend(Channel::Ch1, BEND_CENTER)]
        );
    }

    #[test]
    fn a_passed_note_is_released_after_notes_are_turned_off() {
        let mut effect = NoteToCcEffect::new();
        play(&mut effect, Message::NoteOn(Channel::Ch1, 60, 100));
        effect.pass_notes = false;
        let off = Message::NoteOff(Channel::Ch1, 60, 0);
        assert_eq!(play(&mut effect, off.clone()), vec![off]);
    }
}
