use std::iter::repeat;

use super::note::NoteEvent;

pub struct Pattern {
    row_count: usize,
    channel_count: usize,
    ///defines how long each row of a pattern lasts in a note division, e.g. 64 means every row lasts a 64th note long.
    note_length: u32,
    rows: Vec<PatternRow>,
}
pub struct PatternRow {
    pub channels: Vec<NoteEvent>,
}
impl PatternRow {
    pub fn new(channel_count: usize) -> Self {
        return Self {
            channels: repeat(NoteEvent::Empty).take(channel_count).collect(),
        };
    }
}
impl Pattern {
    pub fn new(row_count: usize, channel_count: usize, note_length: u32) -> Self {
        let mut pattern = Pattern {
            row_count,
            channel_count,
            note_length,
            rows: Vec::new(),
        };
        pattern.initialize_rows();
        return pattern;
    }

    fn initialize_rows(&mut self) {
        for _r in 0..self.row_count {
            self.rows.push(PatternRow::new(self.channel_count));
        }
    }
    /// Calculate the duration of a row in milliseconds for a given BPM
    pub fn row_duration_ms(&self, bpm: u32) -> f32 {
        if self.note_length == 0 {
            // Prevent division by zero, fallback to quarter note
            let notes_per_beat = 4.0;
            60_000.0 / (bpm as f32 * notes_per_beat)
        } else {
            // notes_per_beat = note_length_denominator / 4 (since BPM is quarter notes)
            let notes_per_beat = self.note_length as f32 / 4.0;
            60_000.0 / (bpm as f32 * notes_per_beat)
        }
    }
    pub fn get_event(&self, row: usize, channel: usize) -> Option<&NoteEvent> {
        self.rows.get(row)?.channels.get(channel)
    }

    pub fn set_event(&mut self, row: usize, channel: usize, event: NoteEvent) {
        if let Some(row_data) = self.rows.get_mut(row) {
            if channel < row_data.channels.len() {
                row_data.channels[channel] = event;
            }
        }
    }
}

pub struct PatternStore {
    patterns: &[Pattern],
}
