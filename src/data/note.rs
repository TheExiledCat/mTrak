use serde::{Deserialize, Serialize};

use crate::tui::constants::{self};

use super::effect::Effects;

/// Type safe wrapper for a handle on an instrument
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstrumentId(pub u8);
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub pitch: NotePitch,
    pub instrument_id: InstrumentId,
    pub volume: u8,
    pub effects: Effects,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotePitch(pub u8);
impl NotePitch {
    pub fn new(pitch: u8) -> Self {
        let pitch = pitch.clamp(0, 127);
        return Self(pitch);
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NoteEvent {
    Empty,
    Note(Note),
}

impl ToString for NoteEvent {
    fn to_string(&self) -> String {
        match self {
            NoteEvent::Empty => String::from(constants::EMPTY_NOTE),
            NoteEvent::Note(note) => String::from(format!(
                "{}|{:02}|{:02}|{}",
                note.pitch.to_string(),
                note.instrument_id.0,
                note.volume,
                note.effects.to_string()
            )),
        }
    }
}
