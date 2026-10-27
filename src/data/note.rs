use std::fmt::{self, Display};

use crate::{
    data::{
        effect::{Command, CommandKind},
        midi::{MIDI_CHANNEL_MAX, VOLUME_MAX},
        pattern::invalid_pattern,
    },
    error::Result,
    tui::constants::{self},
};

/// A saved cell starts with a flag byte: the top bit is always set, and each of the low bits
/// says that a field follows, in this order. A cell with no fields is empty.
const CELL: u8 = 0x80;
const CELL_NOTE: u8 = 0x01;
const CELL_INSTRUMENT: u8 = 0x02;
const CELL_VOLUME: u8 = 0x04;
const CELL_COMMAND: u8 = 0x08;
const CELL_VALUE: u8 = 0x10;
const CELL_FLAGS: u8 = CELL | CELL_NOTE | CELL_INSTRUMENT | CELL_VOLUME | CELL_COMMAND | CELL_VALUE;
/// The note field of a note off; other note fields are MIDI pitches.
const CELL_NOTE_OFF: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq)]
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

#[derive(Debug, Clone, Copy, PartialEq)]
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

    /// Appends the note as a saved cell: a flag byte, then the fields it names.
    pub fn write_cell(&self, bytes: &mut Vec<u8>) {
        let note = match self.kind {
            NoteKind::Empty => None,
            NoteKind::Off => Some(CELL_NOTE_OFF),
            NoteKind::On(pitch) => Some(pitch.0),
        };
        let command = self.command.kind;
        let fields = [
            (CELL_NOTE, note),
            (CELL_INSTRUMENT, self.instrument_id),
            (CELL_VOLUME, self.velocity),
            (
                CELL_COMMAND,
                (command != CommandKind::None).then(|| command.letter() as u8),
            ),
            (
                CELL_VALUE,
                (self.command.value != 0).then_some(self.command.value),
            ),
        ];
        let flags_index = bytes.len();
        bytes.push(CELL);
        for (flag, field) in fields {
            let Some(field) = field else {
                continue;
            };
            bytes[flags_index] |= flag;
            bytes.push(field);
        }
    }

    /// Reads a cell written by [`Note::write_cell`] off the front of `bytes`.
    pub fn read_cell(bytes: &mut &[u8]) -> Result<Note> {
        let flags = take_byte(bytes)?;
        if flags & CELL == 0 || flags & !CELL_FLAGS != 0 {
            return Err(invalid_pattern(format!("an unknown flag byte {flags:02X}")));
        }
        let kind = match take_field(bytes, flags, CELL_NOTE)? {
            None => NoteKind::Empty,
            Some(CELL_NOTE_OFF) => NoteKind::Off,
            Some(pitch @ NotePitch::MIN..=NotePitch::MAX) => NoteKind::On(NotePitch(pitch)),
            Some(note) => return Err(invalid_pattern(format!("note {note}"))),
        };
        let instrument_id = take_field(bytes, flags, CELL_INSTRUMENT)?;
        if let Some(instrument) = instrument_id.filter(|&i| i > MIDI_CHANNEL_MAX) {
            return Err(invalid_pattern(format!("instrument {instrument}")));
        }
        let velocity = take_field(bytes, flags, CELL_VOLUME)?;
        if let Some(volume) = velocity.filter(|&v| v > VOLUME_MAX) {
            return Err(invalid_pattern(format!("volume {volume}")));
        }
        let command = match take_field(bytes, flags, CELL_COMMAND)? {
            None => CommandKind::None,
            Some(letter) => CommandKind::from_char(letter as char)
                .map_err(|_| invalid_pattern(format!("command {:?}", letter as char)))?,
        };
        let value = take_field(bytes, flags, CELL_VALUE)?.unwrap_or(0);
        return Ok(Note::new(
            kind,
            instrument_id,
            velocity,
            Command::new(command, value),
        ));
    }
}

fn take_byte(bytes: &mut &[u8]) -> Result<u8> {
    let Some((&byte, rest)) = bytes.split_first() else {
        return Err(invalid_pattern("cells that are cut short".into()));
    };
    *bytes = rest;
    return Ok(byte);
}

/// Takes a cell's field off the front of `bytes` if its flag is set.
fn take_field(bytes: &mut &[u8], flags: u8, flag: u8) -> Result<Option<u8>> {
    if flags & flag == 0 {
        return Ok(None);
    }
    return take_byte(bytes).map(Some);
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
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NotePitch(pub u8);
impl NotePitch {
    pub fn new(pitch: u8) -> Self {
        let pitch = pitch.clamp(Self::MIN, Self::MAX);
        return Self(pitch);
    }

    /// C-0; lower pitches would need octave -1, which doesn't fit the note column.
    pub const MIN: u8 = 12;
    pub const MAX: u8 = 127;
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
    use crate::error::MTrakError;

    fn cell(note: Note) -> Vec<u8> {
        let mut bytes = Vec::new();
        note.write_cell(&mut bytes);
        return bytes;
    }

    fn read(bytes: &[u8]) -> Result<Note> {
        let mut bytes = bytes;
        let note = Note::read_cell(&mut bytes)?;
        assert!(
            bytes.is_empty(),
            "the cell left {} bytes unread",
            bytes.len()
        );
        return Ok(note);
    }

    #[test]
    fn cells_hold_only_the_fields_that_are_set() {
        assert_eq!(cell(Note::empty()), [0x80]);
        let off = Note::new(NoteKind::Off, None, None, Command::default());
        assert_eq!(cell(off), [0x81, 1]);
        let full = Note::new(
            NoteKind::On(NotePitch(60)),
            Some(3),
            Some(0x40),
            "F8C".parse().unwrap(),
        );
        assert_eq!(cell(full), [0x9F, 60, 3, 0x40, b'F', 0x8C]);
        let command_only = Note::new(NoteKind::Empty, None, None, "S10".parse().unwrap());
        assert_eq!(cell(command_only), [0x98, b'S', 0x10]);
    }

    #[test]
    fn cells_round_trip() {
        let notes = [
            Note::empty(),
            Note::new(NoteKind::Off, Some(15), None, Command::default()),
            Note::new(
                NoteKind::On(NotePitch(127)),
                None,
                Some(0),
                "I7D".parse().unwrap(),
            ),
            Note::new(
                NoteKind::On(NotePitch(12)),
                Some(0),
                Some(0x80),
                "Y00".parse().unwrap(),
            ),
            // a value without a command is kept too
            Note::new(NoteKind::Empty, None, None, "012".parse().unwrap()),
        ];
        for note in notes {
            assert_eq!(read(&cell(note)).unwrap(), note);
        }
    }

    #[test]
    fn invalid_cells_are_rejected() {
        let invalid: [&[u8]; 9] = [
            &[],
            &[0x83, 60],
            &[0x00],
            &[0xA0],
            &[0x81, 5],
            &[0x81, 128],
            &[0x82, 16],
            &[0x84, 0x81],
            &[0x88, b'Z'],
        ];
        for bytes in invalid {
            assert!(
                matches!(read(bytes), Err(MTrakError::InvalidProject(_))),
                "{bytes:02X?} should be rejected"
            );
        }
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
