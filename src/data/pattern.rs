use std::ops::Range;

use serde::{Deserialize, Serialize};

use super::note::Note;
use crate::{
    data::{midi::MIDI_CHANNEL_MAX, note::NoteKind},
    tui::constants,
};
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PatternId(pub u8);
pub const MAX_ROWS: u16 = 0x100;

pub fn same_cells(a: &[PatternRow], b: &[PatternRow]) -> bool {
    return a.iter().map(|r| &r.tracks).eq(b.iter().map(|r| &r.tracks));
}

#[derive(Clone, Copy)]
pub struct Block {
    pub track: u8,
    pub width: u8,
    pub row: u8,
    pub height: u16,
}
impl Block {
    pub fn tracks(&self) -> Range<usize> {
        return self.track as usize..self.track as usize + self.width as usize;
    }
    pub fn rows(&self) -> Range<usize> {
        return self.row as usize..self.row as usize + self.height as usize;
    }
}
#[derive(Clone, Copy)]
pub enum ClearOption {
    Block,
    Track,
    Pattern,
}
#[derive(Clone, Copy)]
pub enum ClearTarget {
    Note,
    Velocity,
    Command,
    All,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct Pattern {
    pub row_count: u16,
    pub track_count: u8,
    pub rows: Vec<PatternRow>,
    pub channel_mutes: [bool; 16],
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PatternRow {
    pub tracks: Vec<Note>,
    /// Unused, kept so saved projects keep their layout.
    dirty: bool,
}
impl Default for PatternRow {
    fn default() -> Self {
        return Self::new();
    }
}

impl PatternRow {
    pub fn new() -> Self {
        return Self {
            tracks: std::iter::repeat_n(Note::empty(), constants::TRACK_COUNT as usize).collect(),
            dirty: false,
        };
    }
    pub fn with_tracks(track_cells: Vec<Note>) -> Self {
        return Self {
            tracks: track_cells,
            dirty: false,
        };
    }
}

impl Pattern {
    pub fn new(row_count: u16) -> Self {
        let mut pattern = Pattern {
            row_count,
            track_count: constants::TRACK_COUNT,
            rows: Vec::new(),
            channel_mutes: [false; MIDI_CHANNEL_MAX as usize + 1],
        };
        pattern.initialize_rows();
        return pattern;
    }

    pub fn last_row(&self) -> u8 {
        return self.row_count.saturating_sub(1) as u8;
    }

    fn initialize_rows(&mut self) {
        for _r in 0..self.row_count {
            self.rows.push(PatternRow::new());
        }
    }
    pub fn get_event(&self, row: usize, track: usize) -> Option<&Note> {
        return self.rows.get(row)?.tracks.get(track);
    }

    pub fn set_event(&mut self, row: usize, track: usize, event: Note) {
        if let Some(row_data) = self.rows.get_mut(row) {
            if track < row_data.tracks.len() {
                row_data.tracks[track] = event;
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
        }
        self.set_event(row, track, Note::empty());
    }
    pub fn block(&self) -> Block {
        return Block {
            track: 0,
            width: self.track_count,
            row: 0,
            height: self.row_count,
        };
    }
    pub fn track_block(&self, track: u8) -> Block {
        return Block {
            track,
            width: 1,
            ..self.block()
        };
    }
    pub fn read_block(&self, block: Block) -> Vec<PatternRow> {
        return self
            .rows
            .iter()
            .skip(block.row as usize)
            .take(block.height as usize)
            .map(|r| {
                PatternRow::with_tracks(
                    r.tracks
                        .iter()
                        .skip(block.track as usize)
                        .take(block.width as usize)
                        .copied()
                        .collect(),
                )
            })
            .collect();
    }
    /// writes pattern rows into the pattern, filling only the area defined in the block
    pub fn write_block(&mut self, block: Block, rows: &[PatternRow]) {
        for (row, cells) in block.rows().zip(rows) {
            for (track, cell) in block.tracks().zip(&cells.tracks) {
                self.set_event(row, track, *cell);
            }
        }
    }

    /// checks if 2 patterns are equal, that is, they have the same length and notes
    pub fn same_content(&self, other: &Pattern) -> bool {
        return self.row_count == other.row_count && same_cells(&self.rows, &other.rows);
    }

    // swaps this pattern's length and notes with another pattern's
    pub fn swap_content(&mut self, other: &mut Pattern) {
        std::mem::swap(&mut self.row_count, &mut other.row_count);
        std::mem::swap(&mut self.rows, &mut other.rows);
    }
    pub fn swap_block(&mut self, block: Block, rows: &mut Vec<PatternRow>) {
        let current = self.read_block(block);
        self.write_block(block, rows);
        *rows = current;
    }
    pub fn transpose(&mut self, block: Block, instrument: Option<u8>, delta: i8) {
        for row in block.rows() {
            for track in block.tracks() {
                let Some(&note) = self.get_event(row, track) else {
                    continue;
                };
                if !matches!(note.kind, NoteKind::On(_))
                    || instrument.is_some_and(|i| note.instrument_id != Some(i))
                {
                    continue;
                }
                self.set_event(row, track, note.transpose(delta));
            }
        }
    }
    pub fn clear(&mut self, block: Block, target: ClearTarget) {
        self.clear_instrument(block, target, None);
    }
    /// Like `clear`, but only cells using `instrument`, if given.
    pub fn clear_instrument(&mut self, block: Block, target: ClearTarget, instrument: Option<u8>) {
        for row in block.rows() {
            for track in block.tracks() {
                let Some(&current) = self.get_event(row, track) else {
                    continue;
                };
                if instrument.is_some_and(|i| current.instrument_id != Some(i)) {
                    continue;
                }
                let mut note = current;
                match target {
                    ClearTarget::Note => note.clear_note(),
                    ClearTarget::Velocity => note.clear_volume(),
                    ClearTarget::Command => note.clear_command(),
                    ClearTarget::All => note = Note::empty(),
                }
                if note != current {
                    self.set_event(row, track, note);
                }
            }
        }
    }
    /// Shifts every track down one row from `row`, leaving an empty row at `row`.
    /// The last row falls off the end of the pattern.
    pub fn insert_line(&mut self, row: usize) {
        if row >= self.rows.len() {
            return;
        }
        self.rows.pop();
        self.rows.insert(row, PatternRow::new());
    }

    /// Removes the event at `row` in `track`, pulling the rest of the track up one row.
    /// The track's last row is left empty.
    pub fn delete_event(&mut self, row: usize, track: usize) {
        if row >= self.rows.len() || track >= self.track_count as usize {
            return;
        }
        for r in row..self.rows.len() - 1 {
            self.rows[r].tracks[track] = self.rows[r + 1].tracks[track];
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
    }

    pub fn increase_length(&mut self) {
        self.row_count = (self.row_count + 1).min(MAX_ROWS);
        self.rows
            .resize_with(self.row_count as usize, PatternRow::new);
    }

    pub fn set_length(&mut self, row_count: u16) {
        self.row_count = row_count.clamp(1, MAX_ROWS);
        self.rows
            .resize_with(self.row_count as usize, PatternRow::new);
    }

    pub fn decrease_length(&mut self) -> Vec<PatternRow> {
        self.row_count = self.row_count.saturating_sub(1).max(1);
        return self.rows.split_off(self.row_count as usize);
    }

    pub fn shrink(&mut self) -> bool {
        if self.row_count < 2 {
            return false;
        }
        let mut i = 0;
        self.rows.retain(|_| {
            let keep = i % 2 == 0;
            i += 1;
            keep
        });
        self.set_length(self.rows.len() as u16);
        return true;
    }

    pub fn expand(&mut self) -> bool {
        if self.row_count > 0x80 {
            return false;
        }
        let row_count = self.row_count * 2;
        let mut rows = Vec::with_capacity(row_count as usize);
        for row in self.rows.drain(..) {
            rows.push(row);
            rows.push(PatternRow::new());
        }
        self.rows = rows;
        self.row_count = row_count;
        return true;
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
    pub fn get_patterns(&self) -> &[Pattern] {
        return &self.patterns;
    }
    pub fn get_pattern_by_id_mut(&mut self, id: PatternId) -> Option<&mut Pattern> {
        return self.patterns.get_mut(id.0 as usize);
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
    use super::*;
    use crate::data::{
        effect::{Command, CommandKind},
        note::{NoteKind, NotePitch},
    };

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
    fn set_event_writes_the_cell() {
        let mut pattern = Pattern::new(4);
        pattern.set_event(2, 1, note());
        assert_eq!(pattern.get_event(2, 1), Some(&note()));
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
    fn clearing_an_instrument_skips_other_cells() {
        let mut pattern = Pattern::new(3);
        let mut first = note();
        first.instrument_id = Some(1);
        let mut second = note();
        second.instrument_id = Some(2);
        pattern.set_event(0, 0, first);
        pattern.set_event(1, 0, second);
        pattern.clear_instrument(pattern.block(), ClearTarget::All, Some(2));
        assert_eq!(pattern.get_event(0, 0), Some(&first));
        assert_eq!(pattern.get_event(1, 0), Some(&Note::empty()));
    }

    #[test]
    fn clear_only_touches_target_inside_block() {
        let mut pattern = Pattern::new(3);
        let mut full = note();
        full.velocity = Some(0x40);
        full.command = Command::new(CommandKind::None, 0x10);
        for row in 0..3 {
            pattern.set_event(row, 0, full);
            pattern.set_event(row, 1, full);
        }
        let block = Block {
            track: 0,
            width: 1,
            row: 1,
            height: 2,
        };
        pattern.clear(block, ClearTarget::Velocity);
        assert_eq!(pattern.get_event(0, 0), Some(&full));
        assert_eq!(pattern.get_event(1, 0).unwrap().velocity, None);
        assert_eq!(pattern.get_event(1, 0).unwrap().kind, full.kind);
        assert_eq!(pattern.get_event(1, 1), Some(&full));
        pattern.clear(block, ClearTarget::All);
        assert_eq!(pattern.get_event(2, 0), Some(&Note::empty()));
    }

    #[test]
    fn insert_event_shifts_only_its_track() {
        let mut pattern = Pattern::new(3);
        pattern.set_event(0, 0, note());
        pattern.set_event(2, 0, note());
        pattern.set_event(0, 1, note());
        pattern.insert_event(0, 0);
        assert_eq!(pattern.get_event(0, 0), Some(&Note::empty()));
        assert_eq!(pattern.get_event(1, 0), Some(&note()));
        // the event on the last row is pushed out
        assert_eq!(pattern.get_event(2, 0), Some(&Note::empty()));
        assert_eq!(pattern.get_event(0, 1), Some(&note()));
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
        pattern.delete_event(0, 0);
        assert_eq!(pattern.get_event(0, 0), Some(&note()));
        assert_eq!(pattern.get_event(1, 0), Some(&note()));
        assert_eq!(pattern.get_event(2, 0), Some(&Note::empty()));
        assert_eq!(pattern.get_event(2, 1), Some(&note()));
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
    fn store_creates_missing_patterns_up_to_id() {
        let mut store = PatternStore::new(vec![]);
        store.create_pattern_id_if_not_exists(PatternId(3));
        assert_eq!(store.get_patterns().len(), 4);
        store.create_pattern_id_if_not_exists(PatternId(1));
        assert_eq!(store.get_patterns().len(), 4);
    }

    #[test]
    fn expand_doubles_up_to_max_rows() {
        let mut pattern = Pattern::new(0x80);
        pattern.set_event(0x7F, 0, note());
        assert!(pattern.expand());
        assert_eq!(pattern.row_count, MAX_ROWS);
        assert_eq!(pattern.rows.len(), MAX_ROWS as usize);
        assert_eq!(pattern.get_event(0xFE, 0), Some(&note()));
        assert_eq!(pattern.last_row(), 0xFF);
        assert!(!pattern.expand());
    }

    #[test]
    fn shrink_keeps_even_rows() {
        let mut pattern = Pattern::new(MAX_ROWS);
        pattern.set_event(0xFE, 0, note());
        assert!(pattern.shrink());
        assert_eq!(pattern.row_count, 0x80);
        assert_eq!(pattern.rows.len(), 0x80);
        assert_eq!(pattern.get_event(0x7F, 0), Some(&note()));
        let mut single = Pattern::new(1);
        assert!(!single.shrink());
    }

    #[test]
    fn length_is_capped_at_max_rows() {
        let mut pattern = Pattern::new(MAX_ROWS);
        pattern.increase_length();
        assert_eq!(pattern.row_count, MAX_ROWS);
        pattern.set_length(0x200);
        assert_eq!(pattern.rows.len(), MAX_ROWS as usize);
    }

    fn instrument_note(pitch: u8, instrument: u8) -> Note {
        let mut note = Note::empty();
        note.kind = NoteKind::On(NotePitch(pitch));
        note.instrument_id = Some(instrument);
        return note;
    }

    #[test]
    fn transpose_filters_by_instrument() {
        let mut pattern = Pattern::new(2);
        pattern.set_event(0, 0, instrument_note(60, 1));
        pattern.set_event(1, 0, instrument_note(60, 2));
        pattern.transpose(pattern.block(), Some(1), 1);
        assert_eq!(pattern.get_event(0, 0), Some(&instrument_note(61, 1)));
        assert_eq!(pattern.get_event(1, 0), Some(&instrument_note(60, 2)));
        pattern.transpose(pattern.block(), None, -1);
        assert_eq!(pattern.get_event(0, 0), Some(&instrument_note(60, 1)));
        assert_eq!(pattern.get_event(1, 0), Some(&instrument_note(59, 2)));
    }

    #[test]
    fn transpose_stays_inside_the_block() {
        let mut pattern = Pattern::new(0x40);
        pattern.set_event(0x30, 1, instrument_note(60, 0));
        pattern.set_event(0x3F, 1, instrument_note(60, 0));
        pattern.set_event(0x30, 2, instrument_note(60, 0));
        let block = Block {
            track: 1,
            width: 1,
            row: 0x30,
            height: 0x20,
        };
        pattern.transpose(block, None, 1);
        assert_eq!(pattern.get_event(0x30, 1), Some(&instrument_note(61, 0)));
        assert_eq!(pattern.get_event(0x3F, 1), Some(&instrument_note(61, 0)));
        assert_eq!(pattern.get_event(0x30, 2), Some(&instrument_note(60, 0)));
    }

    #[test]
    fn transpose_clamps_to_the_note_range() {
        let mut pattern = Pattern::new(2);
        pattern.set_event(0, 0, instrument_note(NotePitch::MAX, 0));
        pattern.set_event(1, 0, instrument_note(NotePitch::MIN, 0));
        pattern.transpose(pattern.track_block(0), None, 1);
        pattern.transpose(pattern.track_block(0), None, -1);
        assert_eq!(
            pattern.get_event(0, 0),
            Some(&instrument_note(NotePitch::MAX - 1, 0))
        );
        pattern.transpose(pattern.track_block(0), None, -1);
        assert_eq!(
            pattern.get_event(1, 0),
            Some(&instrument_note(NotePitch::MIN, 0))
        );
    }

    #[test]
    fn block_read_then_write_round_trips_and_truncates() {
        let mut pattern = Pattern::new(4);
        pattern.set_event(2, 1, note());
        pattern.set_event(3, 2, note());
        let block = Block {
            track: 1,
            width: 2,
            row: 2,
            height: 4,
        };
        let rows = pattern.read_block(block);
        assert_eq!(rows.len(), 2);
        pattern.write_block(
            block,
            &vec![PatternRow::with_tracks(vec![Note::empty(); 2]); 2],
        );
        assert_eq!(pattern.get_event(2, 1), Some(&Note::empty()));
        pattern.write_block(block, &rows);
        assert_eq!(pattern.get_event(2, 1), Some(&note()));
        assert_eq!(pattern.get_event(3, 2), Some(&note()));
    }

    #[test]
    fn swap_content_keeps_mutes() {
        let mut pattern = Pattern::new(4);
        pattern.channel_mutes[0] = true;
        let mut other = Pattern::new(8);
        other.set_event(5, 0, note());
        let expected = other.clone();
        pattern.swap_content(&mut other);
        assert!(pattern.same_content(&expected));
        assert_eq!(other.row_count, 4);
        assert!(pattern.channel_mutes[0]);
        assert!(!other.channel_mutes[0]);
    }

    #[test]
    fn swap_block_round_trips() {
        let mut pattern = Pattern::new(4);
        let block = Block {
            track: 1,
            width: 2,
            row: 1,
            height: 2,
        };
        let mut rows = vec![PatternRow::with_tracks(vec![note(); 2]); 2];
        pattern.swap_block(block, &mut rows);
        assert_eq!(pattern.get_event(1, 1), Some(&note()));
        assert_eq!(pattern.get_event(2, 2), Some(&note()));
        assert_eq!(rows[0].tracks, vec![Note::empty(); 2]);
        pattern.swap_block(block, &mut rows);
        assert_eq!(pattern.get_event(1, 1), Some(&Note::empty()));
        assert_eq!(rows[1].tracks, vec![note(); 2]);
    }
}
