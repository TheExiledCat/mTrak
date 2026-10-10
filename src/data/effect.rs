use std::{fmt::Display, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::data::note::NoteError;
pub const COMMAND_VALUE_MAX: u8 = u8::MAX;
use MidiCommand::*;
pub const MIDI_COMMAND_LOOKUP: [(char, MidiCommand); 8] = [
    ('S', SetCC),
    ('C', ChangeCC),
    ('G', CCSlide),
    ('H', CCVelocitySlide),
    ('V', VelocitySlide),
    ('E', EndSlide),
    ('A', AfterTouch),
    ('W', PitchBend),
];
pub const EFFECT_COMMAND_LOOKUP: [(char, EffectCommand); 1] = [('F', EffectCommand::SetTempo)];
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Copy)]
pub enum CommandKind {
    None,
    MidiCommand(MidiCommand),
    Effect(EffectCommand),
}
impl CommandKind {
    fn from_char(char: char) -> Result<Self, NoteError> {
        return Ok(match char {
            '0' => CommandKind::None,

            _ => {
                return MidiCommand::from_char(char)
                    .map(|c| c.as_command_kind())
                    .or_else(|_| EffectCommand::from_char(char).map(|c| c.as_command_kind()))
                    .map_err(|_| NoteError::ParseFailure("Command type invalid".into()));
            }
        });
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Copy)]
pub enum MidiCommand {
    SetCC,           // Sets the CC to change with the ChangeCC command
    ChangeCC,        // Changes the CC set by SetCC
    CCSlide, // Slides the CC set by SetCC from this value to the track's next CC or slide command
    CCVelocitySlide, // CCSlide that also sets the velocity of notes played during it
    VelocitySlide, // Slides the velocity of notes played during it, without sending anything itself
    EndSlide, // Lands the track's running slide on this value and stops it, without starting a new one
    AfterTouch, // Sets Channel AfterTouch
    PitchBend, // Sets Pitchbend with low resolution as a single byte
}
impl MidiCommand {
    /// Whether this command starts a slide towards the track's next CC or slide command.
    pub fn is_slide(&self) -> bool {
        return matches!(self, CCSlide | CCVelocitySlide | VelocitySlide);
    }
    /// Whether this command ends a slide that reaches it.
    pub fn is_slide_target(&self) -> bool {
        return self.is_slide() || matches!(self, ChangeCC | EndSlide);
    }
    /// Whether the slide this command starts sends the selected CC.
    pub fn slides_cc(&self) -> bool {
        return matches!(self, CCSlide | CCVelocitySlide);
    }

    fn from_char(char: char) -> Result<Self, NoteError> {
        return MIDI_COMMAND_LOOKUP
            .iter()
            .position(|c| c.0 == char)
            .map(|p| MIDI_COMMAND_LOOKUP[p].1)
            .ok_or(NoteError::ParseFailure("Not a midi command".into()));
    }
    fn as_command_kind(&self) -> CommandKind {
        return CommandKind::MidiCommand(*self);
    }
}
impl Display for MidiCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        return write!(
            f,
            "{}",
            MIDI_COMMAND_LOOKUP
                .iter()
                .position(|c| c.1 == *self)
                .map(|p| MIDI_COMMAND_LOOKUP[p].0)
                .unwrap()
        );
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Copy)]
pub enum EffectCommand {
    SetTempo, // Sets the BPM from 20 up, the speed below that, and 00 goes back to the song's own
}
impl EffectCommand {
    fn from_char(char: char) -> Result<Self, NoteError> {
        return EFFECT_COMMAND_LOOKUP
            .iter()
            .position(|c| c.0 == char)
            .map(|p| EFFECT_COMMAND_LOOKUP[p].1)
            .ok_or(NoteError::ParseFailure("Not an effect command".into()));
    }
    fn as_command_kind(&self) -> CommandKind {
        return CommandKind::Effect(*self);
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Copy)]

pub struct Command {
    pub kind: CommandKind,
    pub value: u8,
}

impl Command {
    pub fn new(kind: CommandKind, value: u8) -> Self {
        return Self { kind, value };
    }
}

impl FromStr for Command {
    type Err = NoteError;

    fn from_str(text: &str) -> Result<Self, NoteError> {
        let chars = text.chars();
        let chars: Vec<char> = chars.collect();
        let err = NoteError::ParseFailure("Command should be 3 digits".into());
        if chars.len() != 3 {
            return Err(err);
        }

        let kind = CommandKind::from_char(chars[0])?;
        let value = u8::from_str_radix(&chars[1..=2].iter().collect::<String>(), 16)
            .map_err(|_| NoteError::ParseFailure("Invalid value".into()))?;

        return Ok(Command::new(kind, value));
    }
}
impl Display for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        return write!(
            f,
            "{}{:02X}",
            match self.kind {
                CommandKind::None => "0".into(),
                CommandKind::MidiCommand(midi_command) => midi_command.to_string(),
                CommandKind::Effect(effect_command) => effect_command.to_string(),
            },
            self.value
        );
    }
}
impl Default for Command {
    fn default() -> Self {
        return Self::new(CommandKind::None, 0);
    }
}
impl Display for EffectCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        return write!(
            f,
            "{}",
            EFFECT_COMMAND_LOOKUP
                .iter()
                .position(|c| c.1 == *self)
                .map(|p| EFFECT_COMMAND_LOOKUP[p].0)
                .unwrap()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_each_midi_command() {
        for (char, midi_command) in MIDI_COMMAND_LOOKUP {
            let command = Command::from_str(&format!("{}40", char)).unwrap();
            assert_eq!(
                command,
                Command::new(CommandKind::MidiCommand(midi_command), 0x40)
            );
        }
    }

    #[test]
    fn parses_each_effect_command() {
        for (char, effect_command) in EFFECT_COMMAND_LOOKUP {
            let command = Command::from_str(&format!("{}40", char)).unwrap();
            assert_eq!(
                command,
                Command::new(CommandKind::Effect(effect_command), 0x40)
            );
        }
    }

    #[test]
    fn display_round_trips() {
        for text in ["000", "0FF", "S4A", "C7F", "A00", "W80", "F7D"] {
            assert_eq!(Command::from_str(text).unwrap().to_string(), text);
        }
    }

    #[test]
    fn value_accepts_lowercase_hex() {
        assert_eq!(Command::from_str("Cff").unwrap().value, 0xFF);
    }

    #[test]
    fn rejects_malformed_commands() {
        for text in ["", "00", "0000", "X00", "0G0", "s00"] {
            assert!(
                Command::from_str(text).is_err(),
                "{:?} should not parse",
                text
            );
        }
    }

    #[test]
    fn default_is_no_command() {
        assert_eq!(Command::default(), Command::from_str("000").unwrap());
    }
}
