use std::{iter::repeat, time::Duration};

use serde::{Deserialize, Serialize};

use crate::tui::constants;

use super::{
    midi::{PULSES_PER_LINE, PULSES_PER_QUARTER},
    note::Note,
};
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PatternId(pub u8);
#[derive(Serialize, Deserialize, Clone)]
pub struct Pattern {
    pub bpm: u8,
    pub ticks_per_line: u8,
    pub row_count: u8,
    pub track_count: u8,
    pub rows: Vec<PatternRow>,
    pub name: Option<String>,
    pub channel_mutes: [bool; 16],
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PatternRow {
    pub tracks: Vec<Note>,
    pub dirty: bool,
}
impl PatternRow {
    pub fn new() -> Self {
        return Self {
            tracks: repeat(Note::empty())
                .take(constants::TRACK_COUNT as usize)
                .collect(),
            dirty: true,
        };
    }
}

impl Pattern {
    pub fn new(row_count: u8) -> Self {
        let mut pattern = Pattern {
            row_count,
            track_count: constants::TRACK_COUNT,
            rows: Vec::new(),
            name: None,

            bpm: 125,
            ticks_per_line: 6,
            channel_mutes: [false; _],
        };
        pattern.initialize_rows();
        return pattern;
    }

    fn initialize_rows(&mut self) {
        for _r in 0..self.row_count {
            self.rows.push(PatternRow::new());
        }
    }
    pub fn pulse_duration(&self) -> Duration {
        return Duration::from_secs_f64(
            60.0 / (self.bpm.max(1) as f64 * PULSES_PER_QUARTER as f64),
        );
    }
    pub fn tick_duration(&self) -> Duration {
        return self.pulse_duration() * PULSES_PER_LINE / self.ticks_per_line.max(1) as u32;
    }
    pub fn get_event(&self, row: usize, track: usize) -> Option<&Note> {
        self.rows.get(row)?.tracks.get(track)
    }

    pub fn set_event(&mut self, row: usize, track: usize, event: Note) {
        if let Some(row_data) = self.rows.get_mut(row) {
            if track < row_data.tracks.len() {
                row_data.tracks[track] = event;
                row_data.dirty = true;
            }
        }
    }

    /// Shifts `track` down one row from `row`, leaving an empty event at `row`.
    /// The track's last event falls off the end of the pattern.
    pub fn insert_event(&mut self, row: usize, track: usize) {
        if row >= self.rows.len() || track >= self.track_count as usize {
            return;
        }
        for r in (row + 1..self.rows.len()).rev() {
            self.rows[r].tracks[track] = self.rows[r - 1].tracks[track];
            self.rows[r].dirty = true;
        }
        self.set_event(row, track, Note::empty());
    }

    /// Shifts every track down one row from `row`, leaving an empty row at `row`.
    /// The last row falls off the end of the pattern.
    pub fn insert_line(&mut self, row: usize) {
        if row >= self.rows.len() {
            return;
        }
        self.rows.pop();
        self.rows.insert(row, PatternRow::new());
        for r in &mut self.rows[row..] {
            r.dirty = true;
        }
    }

    /// Removes the event at `row` in `track`, pulling the rest of the track up one row.
    /// The track's last row is left empty.
    pub fn delete_event(&mut self, row: usize, track: usize) {
        if row >= self.rows.len() || track >= self.track_count as usize {
            return;
        }
        for r in row..self.rows.len() - 1 {
            self.rows[r].tracks[track] = self.rows[r + 1].tracks[track];
            self.rows[r].dirty = true;
        }
        self.set_event(self.rows.len() - 1, track, Note::empty());
    }

    /// Removes the row at `row`, pulling every track up one row.
    /// The last row is left empty.
    pub fn delete_line(&mut self, row: usize) {
        if row >= self.rows.len() {
            return;
        }
        self.rows.remove(row);
        self.rows.push(PatternRow::new());
        for r in &mut self.rows[row..] {
            r.dirty = true;
        }
    }

    pub fn increase_bpm(&mut self) {
        self.bpm = self.bpm.saturating_add(1);
    }

    pub fn decrease_bpm(&mut self) {
        self.bpm = self.bpm.saturating_sub(1);
    }

    pub fn increase_ticks_per_line(&mut self) {
        self.ticks_per_line = self.ticks_per_line.saturating_add(1);
    }

    pub fn decrease_ticks_per_line(&mut self) {
        self.ticks_per_line = self.ticks_per_line.saturating_sub(1);
    }

    pub fn increase_length(&mut self) {
        self.row_count = self.row_count.saturating_add(1);
        self.rows
            .resize_with(self.row_count as usize, PatternRow::new);
    }

    pub fn decrease_length(&mut self) {
        self.row_count = self.row_count.saturating_sub(1).clamp(1, u8::MAX);
        self.rows.truncate(self.row_count as usize);
    }
}

impl Default for Pattern {
    fn default() -> Self {
        return Self::new(64);
    }
}
#[derive(Serialize, Deserialize)]
pub struct PatternStore {
    patterns: Vec<Pattern>,
}
impl PatternStore {
    pub fn new(patterns: Vec<Pattern>) -> Self {
        return PatternStore { patterns };
    }
    pub fn get_pattern_by_id(&self, id: PatternId) -> Option<&Pattern> {
        return self.patterns.get(id.0 as usize);
    }
    pub fn get_pattern_by_name(&self, name: &str) -> Option<&Pattern> {
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
    pub fn get_patterns(&self) -> &[Pattern] {
        return &self.patterns;
    }
    pub fn get_pattern_by_id_mut(&mut self, id: PatternId) -> Option<&mut Pattern> {
        return self.patterns.get_mut(id.0 as usize);
    }
    pub fn get_pattern_by_name_mut(&mut self, name: &str) -> Option<&mut Pattern> {
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
    pub fn get_patterns_mut(&mut self) -> &mut [Pattern] {
        return &mut self.patterns;
    }
    pub fn copy_pattern(&mut self, source: &Pattern) {
        self.patterns.push(source.clone());
    }
    pub fn create_pattern(&mut self) {
        self.patterns.push(Pattern::default());
    }
    pub fn create_pattern_id_if_not_exists(&mut self, pattern: PatternId) {
        let pattern = pattern.0 as usize;
        if pattern >= self.patterns.len() {
            self.patterns.resize_with(pattern + 1, Pattern::default);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::data::note::{NoteKind, NotePitch};

    use super::*;

    fn note() -> Note {
        let mut note = Note::empty();
        note.kind = NoteKind::On(NotePitch(60));
        return note;
    }

    #[test]
    fn new_pattern_is_empty() {
        let pattern = Pattern::new(16);
        assert_eq!(pattern.rows.len(), 16);
        assert!(pattern.rows.iter().all(|row| {
            row.tracks.len() == constants::TRACK_COUNT as usize
                && row.tracks.iter().all(|n| *n == Note::empty())
        }));
    }

    #[test]
    fn set_event_marks_row_dirty() {
        let mut pattern = Pattern::new(4);
        pattern.rows[2].dirty = false;
        pattern.set_event(2, 1, note());
        assert_eq!(pattern.get_event(2, 1), Some(&note()));
        assert!(pattern.rows[2].dirty);
    }

    #[test]
    fn out_of_range_events_are_ignored() {
        let mut pattern = Pattern::new(4);
        pattern.set_event(4, 0, note());
        pattern.set_event(0, constants::TRACK_COUNT as usize, note());
        assert_eq!(pattern.get_event(4, 0), None);
        assert_eq!(pattern.get_event(0, constants::TRACK_COUNT as usize), None);
    }

    #[test]
    fn insert_event_shifts_only_its_track() {
        let mut pattern = Pattern::new(3);
        pattern.set_event(0, 0, note());
        pattern.set_event(2, 0, note());
        pattern.set_event(0, 1, note());
        for row in &mut pattern.rows {
            row.dirty = false;
        }
        pattern.insert_event(0, 0);
        assert_eq!(pattern.get_event(0, 0), Some(&Note::empty()));
        assert_eq!(pattern.get_event(1, 0), Some(&note()));
        // the event on the last row is pushed out
        assert_eq!(pattern.get_event(2, 0), Some(&Note::empty()));
        assert_eq!(pattern.get_event(0, 1), Some(&note()));
        assert!(pattern.rows.iter().all(|row| row.dirty));
    }

    #[test]
    fn insert_line_shifts_every_track() {
        let mut pattern = Pattern::new(3);
        pattern.set_event(1, 0, note());
        pattern.set_event(1, 1, note());
        pattern.set_event(2, 2, note());
        pattern.insert_line(1);
        assert_eq!(pattern.rows.len(), 3);
        assert_eq!(pattern.get_event(1, 0), Some(&Note::empty()));
        assert_eq!(pattern.get_event(2, 0), Some(&note()));
        assert_eq!(pattern.get_event(2, 1), Some(&note()));
        assert_eq!(pattern.get_event(2, 2), Some(&Note::empty()));
    }

    #[test]
    fn delete_event_pulls_only_its_track_up() {
        let mut pattern = Pattern::new(3);
        pattern.set_event(1, 0, note());
        pattern.set_event(2, 0, note());
        pattern.set_event(2, 1, note());
        for row in &mut pattern.rows {
            row.dirty = false;
        }
        pattern.delete_event(0, 0);
        assert_eq!(pattern.get_event(0, 0), Some(&note()));
        assert_eq!(pattern.get_event(1, 0), Some(&note()));
        assert_eq!(pattern.get_event(2, 0), Some(&Note::empty()));
        assert_eq!(pattern.get_event(2, 1), Some(&note()));
        assert!(pattern.rows.iter().all(|row| row.dirty));
    }

    #[test]
    fn delete_line_pulls_every_track_up() {
        let mut pattern = Pattern::new(3);
        pattern.set_event(1, 0, note());
        pattern.set_event(2, 1, note());
        pattern.delete_line(0);
        assert_eq!(pattern.rows.len(), 3);
        assert_eq!(pattern.get_event(0, 0), Some(&note()));
        assert_eq!(pattern.get_event(1, 1), Some(&note()));
        assert_eq!(pattern.get_event(2, 1), Some(&Note::empty()));
    }

    #[test]
    fn length_changes_keep_rows_in_sync() {
        let mut pattern = Pattern::new(2);
        pattern.set_event(1, 0, note());
        pattern.increase_length();
        assert_eq!(pattern.rows.len(), 3);
        assert_eq!(pattern.get_event(2, 0), Some(&Note::empty()));
        pattern.decrease_length();
        pattern.decrease_length();
        pattern.decrease_length();
        assert_eq!(pattern.row_count, 1);
        assert_eq!(pattern.rows.len(), 1);
    }

    #[test]
    fn durations_follow_bpm() {
        let mut pattern = Pattern::new(1);
        pattern.bpm = 125;
        pattern.ticks_per_line = 6;
        assert_eq!(pattern.pulse_duration(), Duration::from_millis(20));
        assert_eq!(pattern.tick_duration(), Duration::from_millis(20));
    }

    #[test]
    fn zero_bpm_and_ticks_do_not_divide_by_zero() {
        let mut pattern = Pattern::new(1);
        pattern.bpm = 0;
        pattern.ticks_per_line = 0;
        assert!(pattern.pulse_duration() > Duration::ZERO);
        assert!(pattern.tick_duration() > Duration::ZERO);
    }

    #[test]
    fn store_creates_missing_patterns_up_to_id() {
        let mut store = PatternStore::new(vec![]);
        store.create_pattern_id_if_not_exists(PatternId(3));
        assert_eq!(store.get_patterns().len(), 4);
        store.create_pattern_id_if_not_exists(PatternId(1));
        assert_eq!(store.get_patterns().len(), 4);
    }

    #[test]
    fn store_finds_patterns_by_name() {
        let mut named = Pattern::new(1);
        named.name = Some("verse".into());
        let store = PatternStore::new(vec![Pattern::new(1), named]);
        assert_eq!(store.get_pattern_by_name("verse").unwrap().row_count, 1);
        assert!(store.get_pattern_by_name("chorus").is_none());
    }
}
