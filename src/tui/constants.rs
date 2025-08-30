use ratatui::crossterm::event::KeyCode;
pub const EXIT_KEY: KeyCode = KeyCode::Char('q');
pub const CHANNEL_COUNT: usize = 8;
pub const EMPTY_NOTE: &'static str = "---|00|00|00";
pub const MAX_TRACK_EFFECTS: usize = 4;

pub const NOTE_NAMES: &[&'static str] = &[
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];
