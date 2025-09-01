use std::iter::repeat;

use serde::{Deserialize, Serialize};

use crate::tui::constants;

use super::note::{NoteError, NoteEvent};
#[derive(Serialize, Deserialize, Clone)]
pub struct Pattern {
    pub row_count: usize,
    pub channel_count: usize,
    ///defines how long each row of a pattern lasts in a note division, e.g. 64 means every row lasts a 64th note long.
    pub note_length: u32,
    pub rows: Vec<PatternRow>,
    pub name: Option<String>,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PatternRow {
    pub channels: Vec<NoteEvent>,
    pub dirty: bool,
}
impl PatternRow {
    pub fn new() -> Self {
        return Self {
            channels: repeat(NoteEvent::Empty)
                .take(constants::CHANNEL_COUNT)
                .collect(),
            dirty: true,
        };
    }
}
impl Pattern {
    pub fn new(row_count: usize, note_length: u32) -> Self {
        let mut pattern = Pattern {
            row_count,
            channel_count: constants::CHANNEL_COUNT,
            note_length,
            rows: Vec::new(),
            name: None,
        };
        pattern.initialize_rows();
        return pattern;
    }

    fn initialize_rows(&mut self) {
        for _r in 0..self.row_count {
            self.rows.push(PatternRow::new());
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

pub struct PatternStore<'a> {
    patterns: &'a Vec<Pattern>,
}
pub struct PatternStoreMut<'a> {
    patterns: &'a mut Vec<Pattern>,
}
impl<'a> PatternStore<'a> {
    pub fn new(patterns: &'a Vec<Pattern>) -> Self {
        return PatternStore { patterns };
    }
    pub fn get_pattern_by_id(&'a self, id: usize) -> Option<&'a Pattern> {
        return self.patterns.get(id);
    }
    pub fn get_pattern_by_name(&'a self, name: &str) -> Option<&'a Pattern> {
        for pattern in self.patterns.iter() {
            let pattern_name = match &pattern.name {
                Some(n) => n,
                None => continue,
            };
            if pattern_name == name {
                return Some(&pattern);
            }
        }
        return None;
    }
    pub fn get_patterns(&'a self) -> &'a [Pattern] {
        return &self.patterns;
    }
}
impl<'a> PatternStoreMut<'a> {
    pub fn new(patterns: &'a mut Vec<Pattern>) -> Self {
        return PatternStoreMut { patterns };
    }
    pub fn get_pattern_by_id(&'a mut self, id: usize) -> Option<&'a mut Pattern> {
        return self.patterns.get_mut(id);
    }
    pub fn get_pattern_by_name(&'a mut self, name: &str) -> Option<&'a mut Pattern> {
        for pattern in self.patterns.iter_mut() {
            let pattern_name = match &pattern.name {
                Some(n) => n,
                None => continue,
            };
            if pattern_name == name {
                return Some(pattern);
            }
        }
        return None;
    }
    pub fn get_patterns(&'a mut self) -> &'a mut [Pattern] {
        return self.patterns;
    }
    pub fn new_pattern(&'a mut self, row_count: usize, note_length: u32) -> &'a Pattern {
        self.patterns.push(Pattern::new(row_count, note_length));
        return self.patterns.last().unwrap();
    }
    pub fn update_row(
        &'a mut self,
        pattern_index: usize,
        row_index: usize,
        channel_index: usize,
        note_string: &str,
    ) -> Result<(), NoteError> {
        self.patterns[pattern_index].rows[row_index].channels[channel_index] =
            NoteEvent::from_string(note_string)?;
        self.patterns[pattern_index].rows[row_index].dirty = true;
        return Ok(());
    }
}
