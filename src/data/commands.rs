use std::{fmt::Display, str::FromStr};

use MidiCommand::*;

use crate::error::MTrakError;
pub const MIDI_COMMAND_LOOKUP: [(char, MidiCommand); 10] = [
    ('S', SetCC),
    ('M', ChangeCC),
    ('W', ModWheel),
    ('V', CCSlide),
    ('J', CCVelocitySlide),
    ('U', VelocitySlide),
    ('N', EndSlide),
    ('T', AfterTouch),
    ('P', PitchBend),
    ('I', Interval),
];
pub const PLAYBACK_COMMAND_LOOKUP: [(char, PlaybackCommand); 10] = [
    ('F', PlaybackCommand::SetTempo),
    ('Y', PlaybackCommand::Probability),
    ('E', PlaybackCommand::Extended),
    ('R', PlaybackCommand::Retrig),
    ('L', PlaybackCommand::Legato),
    ('B', PlaybackCommand::PositionJump),
    ('D', PlaybackCommand::PatternBreak),
    ('G', PlaybackCommand::SetGlobalVelocity),
    ('H', PlaybackCommand::GlobalVelocitySlide),
    ('C', PlaybackCommand::SetTrackVelocity),
];
/// A way to write a command's value, e.g. `xy`, and what it does.
pub type CommandForm = (&'static str, &'static str);

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum CommandKind {
    None,
    Midi(MidiCommand),
    Playback(PlaybackCommand),
}
impl CommandKind {
    /// The letter for this command in the pattern's command column, `0` for none.
    pub fn letter(&self) -> char {
        return match self {
            CommandKind::None => '0',
            CommandKind::Midi(command) => {
                MIDI_COMMAND_LOOKUP
                    .iter()
                    .find(|c| c.1 == *command)
                    .unwrap()
                    .0
            }
            CommandKind::Playback(command) => {
                PLAYBACK_COMMAND_LOOKUP
                    .iter()
                    .find(|c| c.1 == *command)
                    .unwrap()
                    .0
            }
        };
    }
    pub fn from_char(char: char) -> Result<Self, MTrakError> {
        return Ok(match char {
            '0' => CommandKind::None,

            _ => {
                return MidiCommand::from_char(char)
                    .map(|c| c.as_command_kind())
                    .or_else(|_| PlaybackCommand::from_char(char).map(|c| c.as_command_kind()))
                    .map_err(|_| MTrakError::Parse("Command type invalid".into()));
            }
        });
    }
}

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum MidiCommand {
    SetCC,           // Sets the CC to change with the ChangeCC command
    ChangeCC,        // Changes the CC set by SetCC
    ModWheel,        // Selects CC 1 like SetCC 01 and sends it like ChangeCC
    CCSlide, // Slides the CC set by SetCC from this value to the track's next CC or slide command
    CCVelocitySlide, // CCSlide that also sets the velocity of notes played during it
    VelocitySlide, // Slides the velocity of notes played during it, without sending anything itself
    EndSlide, // Lands the track's running slide on this value and stops it, without starting a new one
    AfterTouch, // Sets Channel AfterTouch
    PitchBend, // Sets Pitchbend with low resolution as a single byte
    Interval, // Bends by whole semitones, scaled to the instrument's bend range
}
impl MidiCommand {
    /// Whether this command starts a slide towards the track's next CC or slide command.
    pub fn is_slide(&self) -> bool {
        return matches!(self, CCSlide | CCVelocitySlide | VelocitySlide);
    }
    /// Whether this command ends a slide that reaches it.
    pub fn is_slide_target(&self) -> bool {
        return self.is_slide() || matches!(self, ChangeCC | ModWheel | EndSlide);
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
                "Set CC: select the CC number (hex) that M, V and J change",
            )],
            ChangeCC => &[("xx", "Change CC: send the selected CC with this value")],
            ModWheel => &[(
                "xx",
                "Mod wheel: select CC 1 like S01 and send it with this value",
            )],
            CCSlide => &[(
                "xx",
                "CC slide: slide the selected CC from this value to the next target",
            )],
            CCVelocitySlide => &[(
                "xx",
                "CC + velocity slide: a V that also sets the velocity of notes",
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
            Interval => &[(
                "xx",
                "Interval: bend by semitones from 80 (83 up 3, 7D down 3) in the bend range",
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
        return CommandKind::Midi(*self);
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
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum PlaybackCommand {
    SetTempo, // Sets the BPM from 20 up, the speed below that, and 00 goes back to the song's own
    Probability, // Decides whether or not this note plays based on a chance from 0 to FF
    Extended, // FT2's E commands: 8x early delay, 9x retrig, Cx note cut, Dx note delay
    Retrig,   // Replays the note every y ticks, changing its velocity by x each time
    Legato, // Glides into this note: 0x starts it x ticks early, 1x keeps the previous note x ticks longer
    PositionJump, // FT2's B
    PatternBreak, // FT2's D
    SetGlobalVelocity, // FT2's G
    GlobalVelocitySlide, // FT2's H
    SetTrackVelocity, // scales every note on the track from here on
}
/// Values from 20 up set the BPM; lower values set the speed.
const TEMPO_MIN_BPM: u8 = 0x20;

/// What a playback command does, decoded from its letter and value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackAction {
    /// Goes back to the song's BPM and speed.
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
    Retrig {
        every: u8,
        volume: u8,
    },
    /// Glides into the note, starting it this many ticks before its row.
    LegatoEarly(u8),
    /// Glides into the note, keeping the previous one sounding this many ticks.
    LegatoHold(u8),
    /// Releases the track's note on this tick.
    Cut(u8),
    /// Replays the note every this many ticks within its row.
    RetrigNote(u8),
    PositionJump(u8),
    PatternBreak(u8),
    /// The row is held this many rows longer, but its notes don't play again.
    PatternDelay(u8),
    StopSong,
    SetGlobalVelocity(u8),
    GlobalVelocitySlide {
        up: u8,
        down: u8,
    },
    SetTrackVelocity(u8),
    /// A value no subcommand uses yet.
    Unused,
}

impl PlaybackCommand {
    pub fn decode(&self, value: u8) -> PlaybackAction {
        let (high, low) = (value >> 4, value & 0x0F);
        return match self {
            PlaybackCommand::SetTempo => match value {
                0 => PlaybackAction::ResetTempo,
                value if value < TEMPO_MIN_BPM => PlaybackAction::SetSpeed(value),
                value => PlaybackAction::SetBpm(value),
            },
            PlaybackCommand::Probability => PlaybackAction::Probability(value),
            PlaybackCommand::Extended => match high {
                0x8 => PlaybackAction::DelayEarly(low),
                0x9 => PlaybackAction::RetrigNote(low),
                0xC => PlaybackAction::Cut(low),
                0xD => PlaybackAction::DelayLate(low),
                0xE => PlaybackAction::PatternDelay(low),
                0xF => PlaybackAction::StopSong,
                _ => PlaybackAction::Unused,
            },
            PlaybackCommand::Retrig => PlaybackAction::Retrig {
                every: low,
                volume: high,
            },
            PlaybackCommand::Legato => match high {
                0x0 => PlaybackAction::LegatoEarly(low),
                0x1 => PlaybackAction::LegatoHold(low),
                _ => PlaybackAction::Unused,
            },
            PlaybackCommand::PositionJump => PlaybackAction::PositionJump(value),
            PlaybackCommand::PatternBreak => PlaybackAction::PatternBreak(value),
            PlaybackCommand::SetGlobalVelocity => PlaybackAction::SetGlobalVelocity(value),
            PlaybackCommand::GlobalVelocitySlide => PlaybackAction::GlobalVelocitySlide {
                up: high,
                down: low,
            },
            PlaybackCommand::SetTrackVelocity => PlaybackAction::SetTrackVelocity(value),
        };
    }

    /// The value syntax and a one line summary of each subcommand, for the commands screen.
    pub fn forms(&self) -> &'static [CommandForm] {
        return match self {
            PlaybackCommand::SetTempo => &[
                ("00", "Reset tempo: back to the song's BPM and speed"),
                ("xx", "Set tempo: 01-1F sets the speed, 20-FF the BPM"),
            ],
            PlaybackCommand::Probability => &[(
                "xx",
                "Probability: chance the note plays, 00 never, FF always",
            )],
            PlaybackCommand::Extended => &[
                ("8y", "Early delay: start the note y ticks before its row"),
                (
                    "9y",
                    "Retrig note: replay the note every y ticks in its row",
                ),
                ("Cy", "Note cut: release the note on tick y"),
                ("Dy", "Note delay: start the note y ticks into its row"),
                ("Ey", "Pattern delay: hold the row y rows longer"),
                ("F0", "Stop: stop playback after this row"),
            ],
            PlaybackCommand::Retrig => &[(
                "xy",
                "Retrig: replay the note every y ticks, x changes velocity per hit",
            )],
            PlaybackCommand::Legato => &[
                (
                    "0y",
                    "Legato: glide in, starting the note y ticks before its row",
                ),
                (
                    "1y",
                    "Legato: glide in, holding the previous note y more ticks",
                ),
            ],
            PlaybackCommand::PositionJump => {
                &[("xx", "Position jump: play sequence position xx next")]
            }
            PlaybackCommand::PatternBreak => &[(
                "xx",
                "Pattern break: carry on at row xx of the next position",
            )],
            PlaybackCommand::SetGlobalVelocity => &[(
                "xx",
                "Global velocity: scale every note, 00 silent, FF full",
            )],
            PlaybackCommand::GlobalVelocitySlide => &[
                ("x0", "Global velocity slide: up by x every tick"),
                ("0y", "Global velocity slide: down by y every tick"),
            ],
            PlaybackCommand::SetTrackVelocity => &[(
                "xx",
                "Track velocity: scale the track's notes, 00 silent, FF full",
            )],
        };
    }

    fn from_char(char: char) -> Result<Self, MTrakError> {
        return PLAYBACK_COMMAND_LOOKUP
            .iter()
            .position(|c| c.0 == char)
            .map(|p| PLAYBACK_COMMAND_LOOKUP[p].1)
            .ok_or(MTrakError::Parse("Not a playback command".into()));
    }
    fn as_command_kind(&self) -> CommandKind {
        return CommandKind::Playback(*self);
    }
}
#[derive(Debug, Clone, PartialEq, Copy)]

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
                CommandKind::Midi(midi_command) => midi_command.to_string(),
                CommandKind::Playback(playback_command) => playback_command.to_string(),
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
impl Display for PlaybackCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        return write!(
            f,
            "{}",
            PLAYBACK_COMMAND_LOOKUP
                .iter()
                .position(|c| c.1 == *self)
                .map(|p| PLAYBACK_COMMAND_LOOKUP[p].0)
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
            assert_eq!(command, Command::new(CommandKind::Midi(midi_command), 0x40));
        }
    }

    #[test]
    fn parses_each_playback_command() {
        for (char, playback_command) in PLAYBACK_COMMAND_LOOKUP {
            let command = Command::from_str(&format!("{}40", char)).unwrap();
            assert_eq!(
                command,
                Command::new(CommandKind::Playback(playback_command), 0x40)
            );
        }
    }

    #[test]
    fn tempo_decodes_reset_speed_and_bpm() {
        assert_eq!(
            PlaybackCommand::SetTempo.decode(0x00),
            PlaybackAction::ResetTempo
        );
        assert_eq!(
            PlaybackCommand::SetTempo.decode(0x1F),
            PlaybackAction::SetSpeed(0x1F)
        );
        assert_eq!(
            PlaybackCommand::SetTempo.decode(0x20),
            PlaybackAction::SetBpm(0x20)
        );
    }

    #[test]
    fn retrig_decodes_volume_and_interval() {
        assert_eq!(
            PlaybackCommand::Retrig.decode(0xA3),
            PlaybackAction::Retrig {
                every: 3,
                volume: 0xA
            }
        );
    }

    #[test]
    fn extended_decodes_its_subcommands() {
        assert_eq!(
            PlaybackCommand::Extended.decode(0x82),
            PlaybackAction::DelayEarly(2)
        );
        assert_eq!(
            PlaybackCommand::Extended.decode(0x94),
            PlaybackAction::RetrigNote(4)
        );
        assert_eq!(
            PlaybackCommand::Extended.decode(0xC5),
            PlaybackAction::Cut(5)
        );
        assert_eq!(
            PlaybackCommand::Extended.decode(0xD3),
            PlaybackAction::DelayLate(3)
        );
        assert_eq!(
            PlaybackCommand::Extended.decode(0x13),
            PlaybackAction::Unused
        );
    }

    #[test]
    fn legato_decodes_its_subcommands() {
        assert_eq!(
            PlaybackCommand::Legato.decode(0x03),
            PlaybackAction::LegatoEarly(3)
        );
        assert_eq!(
            PlaybackCommand::Legato.decode(0x12),
            PlaybackAction::LegatoHold(2)
        );
        assert_eq!(PlaybackCommand::Legato.decode(0x35), PlaybackAction::Unused);
        assert_eq!(PlaybackCommand::Legato.decode(0x20), PlaybackAction::Unused);
        assert_eq!(PlaybackCommand::Legato.decode(0x40), PlaybackAction::Unused);
    }

    #[test]
    fn display_round_trips() {
        for text in ["000", "0FF", "S4A", "M7F", "U00", "T00", "P80", "F7D"] {
            assert_eq!(Command::from_str(text).unwrap().to_string(), text);
        }
    }

    #[test]
    fn value_accepts_lowercase_hex() {
        assert_eq!(Command::from_str("Mff").unwrap().value, 0xFF);
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
