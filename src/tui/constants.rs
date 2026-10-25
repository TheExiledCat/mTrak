use ratatui::crossterm::event::KeyCode;
pub const EXIT_KEY: KeyCode = KeyCode::Char('q');
pub const TRACK_COUNT: u8 = 8;
pub const EMPTY_NOTE: &str = "---|-|--|000";
pub const NOTE_OFF_KEY: char = '`';
/// A track in the pattern editor: its left border and the note string, the right border being
/// shared with the next track.
pub const TRACK_WIDTH: u8 = 14;
pub const TRACK_COLUMN_COUNT: u8 = 7;

pub const NOTE_NAMES: &[&str] = &[
    "C-", "C#", "D-", "D#", "E-", "F-", "F#", "G-", "G#", "A-", "A#", "B-",
];
