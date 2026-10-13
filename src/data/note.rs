use std::fmt::{self, Display};

use serde::{Deserialize, Serialize};

use crate::{
    data::effect::Command,
    tui::constants::{self},
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum NoteKind {
    Empty,
    On(NotePitch),
    Off,
}

impl Display for NoteKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        return match self {
            NoteKind::Empty => write!(f, "---"),
            NoteKind::Off => write!(f, "==="),
            NoteKind::On(pitch) => write!(f, "{}", pitch),
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

    pub fn transpose(mut self, delta: i8) -> Note {
        if let NoteKind::On(p) = &mut self.kind {
            *p = NotePitch::new(p.0.saturating_add_signed(delta));
        }
        return self;
    }
}

impl Display for Note {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        return write!(
            f,
            "{}|{}|{}|{}",
            self.kind,
            self.instrument_id
                .map(|i| format!("{:1X}", i))
                .unwrap_or("-".into()),
            self.velocity
                .map(|v| format!("{:02X}", v))
                .unwrap_or("--".into()),
            self.command
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
}

impl Display for NotePitch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let note = self.0;
        let pitch_class = note as usize % 12;
        let octave = (note as i32 / 12) - 1;
        let note_name = constants::NOTE_NAMES[pitch_class];
        return write!(f, "{}{}", note_name, octave);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_and_off_round_trip() {
        assert_eq!(Note::empty().to_string(), constants::EMPTY_NOTE);
        assert_eq!(
            Note::new(NoteKind::Off, None, None, Command::default()).to_string(),
            "===|-|--|000"
        );
    }

    #[test]
    fn pitch_display_fits_note_column() {
        for pitch in NotePitch::MIN..=NotePitch::MAX {
            let text = NotePitch(pitch).to_string();
            assert_eq!(text.len(), 3, "{} should fit the note column", text);
        }
    }

    #[test]
    fn pitch_new_clamps_to_displayable_range() {
        assert_eq!(NotePitch::new(200), NotePitch(127));
        assert_eq!(NotePitch::new(0), NotePitch(12));
    }
}
