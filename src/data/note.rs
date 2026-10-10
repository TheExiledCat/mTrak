use serde::{Deserialize, Serialize};

use crate::{
    data::effect::Command,
    tui::constants::{self},
};

#[derive(Debug)]
pub enum NoteError {
    PARSE_FAILURE(String),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum NoteKind {
    Empty,
    On(NotePitch),
    Off,
}

impl NoteKind {
    pub fn from_string(string: &str) -> Result<NoteKind, NoteError> {
        return match string {
            "---" => Ok(NoteKind::Empty),
            "===" => Ok(NoteKind::Off),
            _ => NotePitch::from_string(string).map(NoteKind::On),
        };
    }
}

impl ToString for NoteKind {
    fn to_string(&self) -> String {
        return match self {
            NoteKind::Empty => String::from("---"),
            NoteKind::Off => String::from("==="),
            NoteKind::On(pitch) => pitch.to_string(),
        };
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Note {
    pub kind: NoteKind,
    pub instrument_id: Option<u8>,
    pub velocity: Option<u8>,
    pub command: Command,
}
impl Note {
    pub fn new(
        kind: NoteKind,
        instrument_id: Option<u8>,
        velocity: Option<u8>,
        command: Command,
    ) -> Self {
        return Self {
            kind,
            instrument_id,
            velocity,
            command,
        };
    }
    pub fn empty() -> Self {
        return Self::new(NoteKind::Empty, None, None, Command::default());
    }
    pub fn from_string(string: &str) -> Result<Note, NoteError> {
        let chunks = string.split("|").collect::<Vec<&str>>();
        if chunks.len() != 4 {
            return Err(NoteError::PARSE_FAILURE(string.to_owned()));
        }

        let kind = NoteKind::from_string(chunks[0])?;
        let instrument_id = match chunks[1] {
            "-" => None,
            s => Some(
                u8::from_str_radix(s, 16).map_err(|e| NoteError::PARSE_FAILURE(e.to_string()))?,
            ),
        };
        let velocity = u8::from_str_radix(chunks[2], 16).ok();
        let command = Command::from_str(chunks[3])
            .map_err(|_| NoteError::PARSE_FAILURE("Invalid Command String".into()))?;
        return Ok(Note::new(kind, instrument_id, velocity, command));
    }
    /// Clears the note and the instrument that goes with it.
    pub fn clear_note(&mut self) {
        self.kind = NoteKind::Empty;
        self.instrument_id = None;
    }
    pub fn clear_volume(&mut self) {
        self.velocity = None;
    }
    pub fn clear_command(&mut self) {
        self.command = Command::default();
    }
}

impl ToString for Note {
    fn to_string(&self) -> String {
        return format!(
            "{}|{}|{}|{}",
            self.kind.to_string(),
            self.instrument_id
                .map(|i| format!("{:1X}", i))
                .unwrap_or("-".into()),
            self.velocity
                .map(|v| format!("{:02X}", v))
                .unwrap_or("--".into()),
            self.command.to_string()
        );
    }
}

/// A MIDI pitch, displayed in scientific notation where C-4 is middle C (60).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct NotePitch(pub u8);
impl NotePitch {
    /// C-0; lower pitches would need octave -1, which doesn't fit the note column.
    pub const MIN: u8 = 12;
    pub const MAX: u8 = 127;

    pub fn new(pitch: u8) -> Self {
        let pitch = pitch.clamp(Self::MIN, Self::MAX);
        return Self(pitch);
    }
    pub fn from_string(string: &str) -> Result<NotePitch, NoteError> {
        if !string.is_char_boundary(2) {
            return Err(NoteError::PARSE_FAILURE(string.to_owned()));
        }
        let (note_name, octave) = string.split_at(2);
        let octave: u8 = octave
            .parse()
            .map_err(|_| NoteError::PARSE_FAILURE(string.to_owned()))?;
        let note_index = constants::NOTE_NAMES
            .iter()
            .position(|s| *s == note_name)
            .ok_or(NoteError::PARSE_FAILURE(string.to_owned()))?;
        let pitch = octave
            .checked_add(1)
            .and_then(|o| o.checked_mul(12))
            .and_then(|p| p.checked_add(note_index as u8))
            .filter(|p| *p <= Self::MAX)
            .ok_or(NoteError::PARSE_FAILURE(string.to_owned()))?;
        return Ok(NotePitch(pitch));
    }
}

impl ToString for NotePitch {
    fn to_string(&self) -> String {
        let note = self.0;
        let pitch_class = note as usize % 12;
        let octave = (note as i32 / 12) - 1;
        let note_name = constants::NOTE_NAMES[pitch_class];
        format!("{}{}", note_name, octave)
    }
}

#[cfg(test)]
mod tests {
    use crate::data::{effect::CommandKind, midi::MIDI_CHANNEL_MAX};

    use super::*;
    #[test]
    fn note_string_to_note() {
        let note = Note::from_string("D#5|1|F0|0FF").unwrap();
        assert_eq!(
            note,
            Note::new(
                NoteKind::On(NotePitch(75)),
                Some(1),
                Some(240),
                Command::new(CommandKind::None, 0xFF)
            )
        );

        let note = Note::from_string("---|F|00|000").unwrap();
        assert_eq!(
            note,
            Note::new(
                NoteKind::Empty,
                Some(MIDI_CHANNEL_MAX),
                Some(0),
                Command::default()
            )
        );

        let note = Note::from_string("===|-|--|000").unwrap();
        assert_eq!(
            note,
            Note::new(NoteKind::Off, None, None, Command::default())
        );
    }

    #[test]
    fn empty_and_off_round_trip() {
        assert_eq!(Note::empty().to_string(), constants::EMPTY_NOTE);
        assert_eq!(
            Note::new(NoteKind::Off, None, None, Command::default()).to_string(),
            "===|-|--|000"
        );
    }

    #[test]
    fn pitch_parses_sharps_and_octaves() {
        assert_eq!(NotePitch::from_string("C-0").unwrap(), NotePitch(12));
        assert_eq!(NotePitch::from_string("C-4").unwrap(), NotePitch(60));
        assert_eq!(NotePitch::from_string("A-4").unwrap(), NotePitch(69));
        assert_eq!(NotePitch::from_string("G-9").unwrap(), NotePitch(127));
    }

    #[test]
    fn pitch_round_trips_through_display() {
        for pitch in NotePitch::MIN..=NotePitch::MAX {
            let text = NotePitch(pitch).to_string();
            assert_eq!(text.len(), 3, "{} should fit the note column", text);
            assert_eq!(
                NotePitch::from_string(&text).unwrap(),
                NotePitch(pitch),
                "{}",
                text
            );
        }
    }

    #[test]
    fn pitch_rejects_unknown_names() {
        for text in ["", "C", "H-4", "C-x", "Cb4", "C--1", "G#9", "C-10"] {
            assert!(
                NotePitch::from_string(text).is_err(),
                "{:?} should not parse",
                text
            );
        }
    }

    #[test]
    fn pitch_new_clamps_to_displayable_range() {
        assert_eq!(NotePitch::new(200), NotePitch(127));
        assert_eq!(NotePitch::new(0), NotePitch(12));
    }

    #[test]
    fn note_rejects_wrong_column_count() {
        assert!(Note::from_string("C-4|1|40").is_err());
        assert!(Note::from_string("C-4|1|40|000|000").is_err());
    }

    #[test]
    fn note_rejects_bad_instrument_and_command() {
        assert!(Note::from_string("C-4|G|40|000").is_err());
        assert!(Note::from_string("C-4|1|40|X00").is_err());
    }

    #[test]
    fn note_round_trips_with_a_command() {
        let text = "===|3|7F|S4A";
        assert_eq!(Note::from_string(text).unwrap().to_string(), text);
    }
}
