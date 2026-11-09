use midi::{
    Channel,
    Message::{self, ControlChange, ProgramChange},
};
use serde::{Deserialize, Serialize};

use crate::data::effect::EffectSettings;

const BANK_SELECT_MSB: u8 = 0;
const BANK_SELECT_LSB: u8 = 32;

pub const DEFAULT_PITCH_WHEEL_RANGE: u8 = 12;

#[derive(Serialize, Deserialize, Clone)]
pub struct Instrument {
    pub name: String,
    pub volume: u8,
    pub pitch_wheel_range: u8,
    /// The CC that CC commands change on a track without an `S`.
    pub default_cc: u8,
    /// Semitones added to every note the instrument plays, on top of the song's transpose.
    pub transpose: i8,
    /// The live MIDI effects on the instrument's channel, in the order notes go through them.
    pub effects: Vec<EffectSettings>,
    pub patch: Patch,
}

/// What's `None` isn't sent.
#[derive(Serialize, Deserialize, Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Patch {
    pub bank_msb: Option<u8>,
    pub bank_lsb: Option<u8>,
    pub program: Option<u8>,
}

impl Patch {
    /// The bank first, as a synth only switches banks on the program change after it.
    pub fn messages(&self, channel: Channel) -> Vec<Message> {
        let mut messages = Vec::new();
        if let Some(msb) = self.bank_msb {
            messages.push(ControlChange(channel, BANK_SELECT_MSB, msb));
        }
        if let Some(lsb) = self.bank_lsb {
            messages.push(ControlChange(channel, BANK_SELECT_LSB, lsb));
        }
        if let Some(program) = self.program {
            messages.push(ProgramChange(channel, program));
        }
        return messages;
    }
}
impl Default for Instrument {
    fn default() -> Self {
        return Self {
            name: String::new(),
            volume: 0xFF,
            pitch_wheel_range: DEFAULT_PITCH_WHEEL_RANGE,
            default_cc: 0,
            transpose: 0,
            effects: Vec::new(),
            patch: Patch::default(),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_patch_selects_the_bank_before_the_program() {
        let patch = Patch {
            bank_msb: Some(1),
            bank_lsb: Some(3),
            program: Some(0x10),
        };
        assert_eq!(
            patch.messages(Channel::Ch1),
            vec![
                ControlChange(Channel::Ch1, 0, 1),
                ControlChange(Channel::Ch1, 32, 3),
                ProgramChange(Channel::Ch1, 0x10),
            ]
        );
        assert!(Patch::default().messages(Channel::Ch1).is_empty());
    }
}
