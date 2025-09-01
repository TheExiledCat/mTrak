use serde::{Deserialize, Serialize};

use crate::tui::constants::{self};

use super::effect::Effects;

#[derive(Debug)]
pub enum NoteError {
    PARSE_FAILURE(String),
}

/// Type safe wrapper for a handle on an instrument
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HexTwoDigitNumber(pub u8);
impl HexTwoDigitNumber {
    pub fn from_string(string: &str) -> Result<Self, NoteError> {
        if string.chars().count() != 2 {
            return Err(NoteError::PARSE_FAILURE(string.to_owned()));
        }

        let num = u8::from_str_radix(string, 16)
            .map_err(|_| NoteError::PARSE_FAILURE(string.to_owned()))?;

        return Ok(Self(num));
    }
}
impl ToString for HexTwoDigitNumber {
    fn to_string(&self) -> String {
        return format!("{:02X}", self.0).to_owned();
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Note {
    pub pitch: Option<NotePitch>,
    pub instrument_id: HexTwoDigitNumber,
    pub volume: HexTwoDigitNumber,
    pub effects: Effects,
}
impl Note {
    pub fn new(
        pitch: Option<NotePitch>,
        instrument_id: HexTwoDigitNumber,
        volume: HexTwoDigitNumber,
        effects: Effects,
    ) -> Self {
        return Self {
            pitch,
            instrument_id,
            volume,
            effects,
        };
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NotePitch(pub u8);
impl NotePitch {
    pub fn new(pitch: u8) -> Self {
        let pitch = pitch.clamp(0, 127);
        return Self(pitch);
    }
    pub fn from_string(string: &str) -> Result<Option<NotePitch>, NoteError> {
        if (string == "---") {
            return Ok(None);
        }
        let (note_name, octave) = string.split_at(2);
        let octave: u8 = octave
            .parse()
            .map_err(|_| NoteError::PARSE_FAILURE(string.to_owned()))?;
        let note_index = constants::NOTE_NAMES
            .iter()
            .position(|s| *s == note_name)
            .ok_or(NoteError::PARSE_FAILURE(string.to_owned()))?;
        let pitch: u8 = (octave + 1) * 12 + note_index as u8;
        return Ok(Some(NotePitch(pitch)));
    }
}

impl ToString for NotePitch {
    fn to_string(&self) -> String {
        let note = self.0;
        let pitch_class = note as usize % 12;
        let octave = (note as i32 / 12) + 1; // instead of -1
        let note_name = constants::NOTE_NAMES[pitch_class];
        let sharp = note_name.ends_with('#');
        format!("{}{}{}", note_name, if sharp { "" } else { "-" }, octave)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NoteEvent {
    Empty,
    Note(Note),
}

impl ToString for NoteEvent {
    fn to_string(&self) -> String {
        match self {
            NoteEvent::Empty => String::from(constants::EMPTY_NOTE),
            NoteEvent::Note(note) => String::from(format!(
                "{}|{}|{}|{}",
                note.pitch
                    .clone()
                    .map(|p| p.to_string())
                    .unwrap_or("---".to_owned()),
                note.instrument_id.to_string(),
                note.volume.to_string(),
                note.effects.to_string()
            )),
        }
    }
}
impl NoteEvent {
    pub fn from_string(string: &str) -> Result<NoteEvent, NoteError> {
        if string == constants::EMPTY_NOTE {
            return Ok(NoteEvent::Empty);
        }
        let chunks = string
            .split("|")
            .map(|s| s.to_owned())
            .collect::<Vec<String>>();
        if chunks.len() != 4 {
            return Err(NoteError::PARSE_FAILURE(string.to_owned()));
        }

        let pitch = NotePitch::from_string(&chunks[0])?;
        let instrument_id = HexTwoDigitNumber::from_string(&chunks[1])?;
        let volume = HexTwoDigitNumber::from_string(&chunks[2])?;
        let effects = Effects {
            chain: [None, None, None, None],
        };

        let note = Note::new(pitch, instrument_id, volume, effects);
        return Ok(NoteEvent::Note(note));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn note_string_to_note_event() {
        let input = "D#5|01|F0|0FF";

        let event = NoteEvent::from_string(input).unwrap();
        let effects = Effects {
            chain: [None, None, None, None],
        };
        assert_eq!(
            event,
            NoteEvent::Note(Note {
                pitch: Some(NotePitch(75)),
                instrument_id: HexTwoDigitNumber(1),
                volume: HexTwoDigitNumber(240),
                effects: effects.clone()
            })
        );

        let input = "---|0F|00|000";

        let event = NoteEvent::from_string(input).unwrap();
        assert_eq!(
            event,
            NoteEvent::Note(Note {
                pitch: None,
                instrument_id: HexTwoDigitNumber(15),
                volume: HexTwoDigitNumber(0),
                effects: effects.clone()
            })
        );
    }
}
