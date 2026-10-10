use ratatui::crossterm::event::KeyCode;
pub const EXIT_KEY: KeyCode = KeyCode::Char('q');
pub const TRACK_COUNT: u8 = 8;
pub const EMPTY_NOTE: &'static str = "---|-|--|000";
pub const NOTE_OFF_KEY: char = '`';
pub const NOTE_STRING_LENGTH: u8 = 15;
pub const MAX_TRACK_EFFECTS: u8 = 4;
pub const TRACK_COLUMN_COUNT: u8 = 7;

pub const NOTE_NAMES: &[&'static str] = &[
    "C-", "C#", "D-", "D#", "E-", "F-", "F#", "G-", "G#", "A-", "A#", "B-",
];
