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

/// Piano keys from C, one semitone apart.
pub const PIANO_KEYS_LOWER: [char; 13] = [
    'z', 's', 'x', 'd', 'c', 'v', 'g', 'b', 'h', 'n', 'j', 'm', ',',
];
pub const PIANO_KEYS_UPPER: [char; 13] = [
    'q', '2', 'w', '3', 'e', 'r', '5', 't', '6', 'y', '7', 'u', 'i',
];
/// The letters that, with Alt, jump to tracks 1 to 8, as in FT2.
pub const TRACK_JUMP_KEYS: [char; 8] = ['q', 'w', 'e', 'r', 't', 'y', 'u', 'i'];
