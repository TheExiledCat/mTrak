use std::{fmt::Display, str::FromStr};

use MidiCommand::*;
use serde::{Deserialize, Serialize};

use crate::error::MTrakError;
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
pub const EFFECT_COMMAND_LOOKUP: [(char, EffectCommand); 5] = [
    ('F', EffectCommand::SetTempo),
    ('Y', EffectCommand::Probability),
    ('D', EffectCommand::Delay),
    ('R', EffectCommand::Roll),
    ('3', EffectCommand::Legato),
];
/// A way to write a command's value, e.g. `xy`, and what it does.
pub type CommandForm = (&'static str, &'static str);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Copy)]
pub enum CommandKind {
    None,
    MidiCommand(MidiCommand),
    Effect(EffectCommand),
}
impl CommandKind {
    fn from_char(char: char) -> Result<Self, MTrakError> {
        return Ok(match char {
            '0' => CommandKind::None,

            _ => {
                return MidiCommand::from_char(char)
                    .map(|c| c.as_command_kind())
                    .or_else(|_| EffectCommand::from_char(char).map(|c| c.as_command_kind()))
                    .map_err(|_| MTrakError::Parse("Command type invalid".into()));
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

    /// The command's value syntax and a one line summary, for the commands screen.
    pub fn forms(&self) -> &'static [CommandForm] {
        return match self {
            SetCC => &[(
                "xx",
                "Set CC: select the CC number (hex) that C, G and H change",
            )],
            ChangeCC => &[("xx", "Change CC: send the selected CC with this value")],
            CCSlide => &[(
                "xx",
                "CC slide: slide the selected CC from this value to the next target",
            )],
            CCVelocitySlide => &[(
                "xx",
                "CC + velocity slide: a G that also sets the velocity of notes",
            )],
            VelocitySlide => &[(
                "xx",
                "Velocity slide: slide the velocity of notes played during it",
            )],
            EndSlide => &[(
                "xx",
                "End slide: land the running slide on this value and stop it",
            )],
            AfterTouch => &[(
                "xx",
                "Aftertouch: send channel pressure, 00 none to FF full",
            )],
            PitchBend => &[(
                "xx",
                "Pitch bend: bend the channel, 00 down, 80 centre, FF up",
            )],
        };
    }

    fn from_char(char: char) -> Result<Self, MTrakError> {
        return MIDI_COMMAND_LOOKUP
            .iter()
            .position(|c| c.0 == char)
            .map(|p| MIDI_COMMAND_LOOKUP[p].1)
            .ok_or(MTrakError::Parse("Not a midi command".into()));
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
    Probability, // Decides whether or not this note plays based on a chance from 0 to FF
    Delay,    // Moves the note: 0x starts it x ticks before its row, 1x x ticks into it
    Roll,     // Replays the note every y ticks, changing its velocity by x each time
    Legato, // Glides into this note: 0x starts it x ticks early, 1x keeps the previous note x ticks longer, 3x cuts the note on tick x
}
/// Values from 20 up set the BPM; lower values set the speed.
const TEMPO_MIN_BPM: u8 = 0x20;

/// What an effect command does, decoded from its letter and value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// Goes back to the song's BPM and the pattern's speed.
    ResetTempo,
    SetSpeed(u8),
    SetBpm(u8),
    /// The chance out of 256 that the note plays; 00 never, FF always.
    Probability(u8),
    /// Starts the note this many ticks before its row.
    DelayEarly(u8),
    /// Starts the note this many ticks into its row.
    DelayLate(u8),
    /// Replays the note every `every` ticks, changing its velocity by `volume` each time.
    Roll {
        every: u8,
        volume: u8,
    },
    /// Glides into the note, starting it this many ticks before its row.
    LegatoEarly(u8),
    /// Glides into the note, keeping the previous one sounding this many ticks.
    LegatoHold(u8),
    /// Releases the track's note on this tick.
    Cut(u8),
    /// A value no subcommand uses yet.
    Unused,
}

impl EffectCommand {
    pub fn decode(&self, value: u8) -> Effect {
        let (high, low) = (value >> 4, value & 0x0F);
        return match self {
            EffectCommand::SetTempo => match value {
                0 => Effect::ResetTempo,
                value if value < TEMPO_MIN_BPM => Effect::SetSpeed(value),
                value => Effect::SetBpm(value),
            },
            EffectCommand::Probability => Effect::Probability(value),
            EffectCommand::Delay => match high {
                0x0 => Effect::DelayEarly(low),
                0x1 => Effect::DelayLate(low),
                _ => Effect::Unused,
            },
            EffectCommand::Roll => Effect::Roll {
                every: low,
                volume: high,
            },
            EffectCommand::Legato => match high {
                0x0 => Effect::LegatoEarly(low),
                0x1 => Effect::LegatoHold(low),
                0x3 => Effect::Cut(low),
                _ => Effect::Unused,
            },
        };
    }

    /// The value syntax and a one line summary of each subcommand, for the commands screen.
    pub fn forms(&self) -> &'static [CommandForm] {
        return match self {
            EffectCommand::SetTempo => &[
                (
                    "00",
                    "Reset tempo: back to the song's BPM and the pattern's speed",
                ),
                ("xx", "Set tempo: 01-1F sets the speed, 20-FF the BPM"),
            ],
            EffectCommand::Probability => &[(
                "xx",
                "Probability: chance the note plays, 00 never, FF always",
            )],
            EffectCommand::Delay => &[
                ("0y", "Delay: start the note y ticks before its row"),
                ("1y", "Delay: start the note y ticks into its row"),
            ],
            EffectCommand::Roll => &[(
                "xy",
                "Roll: replay the note every y ticks, x changes velocity per hit",
            )],
            EffectCommand::Legato => &[
                (
                    "0y",
                    "Legato: glide in, starting the note y ticks before its row",
                ),
                (
                    "1y",
                    "Legato: glide in, holding the previous note y more ticks",
                ),
                ("3y", "Cut: release the note on tick y"),
            ],
        };
    }

    fn from_char(char: char) -> Result<Self, MTrakError> {
        return EFFECT_COMMAND_LOOKUP
            .iter()
            .position(|c| c.0 == char)
            .map(|p| EFFECT_COMMAND_LOOKUP[p].1)
            .ok_or(MTrakError::Parse("Not an effect command".into()));
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
    type Err = MTrakError;

    fn from_str(text: &str) -> Result<Self, MTrakError> {
        let chars = text.chars();
        let chars: Vec<char> = chars.collect();
        let err = MTrakError::Parse("Command should be 3 digits".into());
        if chars.len() != 3 {
            return Err(err);
        }

        let kind = CommandKind::from_char(chars[0])?;
        let value = u8::from_str_radix(&chars[1..=2].iter().collect::<String>(), 16)
            .map_err(|_| MTrakError::Parse("Invalid value".into()))?;

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
    fn tempo_decodes_reset_speed_and_bpm() {
        assert_eq!(EffectCommand::SetTempo.decode(0x00), Effect::ResetTempo);
        assert_eq!(EffectCommand::SetTempo.decode(0x1F), Effect::SetSpeed(0x1F));
        assert_eq!(EffectCommand::SetTempo.decode(0x20), Effect::SetBpm(0x20));
    }

    #[test]
    fn roll_decodes_volume_and_interval() {
        assert_eq!(
            EffectCommand::Roll.decode(0xA3),
            Effect::Roll {
                every: 3,
                volume: 0xA
            }
        );
    }

    #[test]
    fn delay_decodes_early_and_late() {
        assert_eq!(EffectCommand::Delay.decode(0x02), Effect::DelayEarly(2));
        assert_eq!(EffectCommand::Delay.decode(0x13), Effect::DelayLate(3));
        assert_eq!(EffectCommand::Delay.decode(0x23), Effect::Unused);
    }

    #[test]
    fn legato_decodes_its_subcommands() {
        assert_eq!(EffectCommand::Legato.decode(0x03), Effect::LegatoEarly(3));
        assert_eq!(EffectCommand::Legato.decode(0x12), Effect::LegatoHold(2));
        assert_eq!(EffectCommand::Legato.decode(0x35), Effect::Cut(5));
        assert_eq!(EffectCommand::Legato.decode(0x20), Effect::Unused);
        assert_eq!(EffectCommand::Legato.decode(0x40), Effect::Unused);
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
