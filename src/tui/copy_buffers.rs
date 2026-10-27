use crate::{
    data::{
        note::Note,
        pattern::{Block, ClearTarget, Pattern, PatternRow},
    },
    error::Result,
};
pub struct CopyBuffers {
    track_buffer: Option<CopyBuffer>,
    pattern_buffer: Option<CopyBuffer>,
    block_buffer: Option<CopyBuffer>,
}
impl CopyBuffers {
    pub fn new() -> Self {
        return Self {
            track_buffer: None,
            pattern_buffer: None,
            block_buffer: None,
        };
    }

    pub fn copy_track(&mut self, pattern: &Pattern, track: u8) {
        if track >= pattern.track_count {
            return;
        }
        let width = 1;
        let height = pattern.row_count();
        let rows: Vec<PatternRow> = pattern
            .rows
            .iter()
            .map(|p| PatternRow::with_tracks(vec![p.tracks[track as usize]]))
            .collect();
        self.track_buffer = Some(CopyBuffer::new(width, height, rows));
    }
    pub fn copy_pattern(&mut self, pattern: &Pattern) {
        let width = pattern.track_count;
        let height = pattern.row_count();
        let rows = pattern.rows.clone();
        self.pattern_buffer = Some(CopyBuffer::new(width, height, rows))
    }
    pub fn copy_block(&mut self, pattern: &Pattern, block: Block) {
        let (rows, tracks) = (block.rows(), block.tracks());
        if rows.is_empty()
            || tracks.is_empty()
            || tracks.end > pattern.track_count as usize
            || rows.end > pattern.row_count() as usize
        {
            return;
        }
        let rows = pattern.rows[rows]
            .iter()
            .map(|r| PatternRow::with_tracks(r.tracks[tracks.clone()].to_vec()))
            .collect();
        self.block_buffer = Some(CopyBuffer::new(block.width, block.height, rows));
    }

    pub fn cut_track(&mut self, pattern: &mut Pattern, track: u8) {
        self.copy_track(pattern, track);
        pattern.clear(pattern.track_block(track), ClearTarget::All);
    }
    pub fn cut_pattern(&mut self, pattern: &mut Pattern) {
        self.copy_pattern(pattern);
        pattern.clear(pattern.block(), ClearTarget::All);
    }
    pub fn cut_block(&mut self, pattern: &mut Pattern, block: Block) {
        let previous = self.block_buffer.take();
        self.copy_block(pattern, block);
        if self.block_buffer.is_none() {
            self.block_buffer = previous;
            return;
        }
        pattern.clear(block, ClearTarget::All);
    }

    /// paste the buffered track into the pattern at a certain track offset. Truncates the rows to the pattern row length
    pub fn paste_track(&self, pattern: &mut Pattern, track: u8) {
        let Some(buf) = &self.track_buffer else {
            return;
        };
        let row_count = pattern.row_count() as usize;
        buf.rows
            .iter()
            .take(buf.height as usize)
            .take(row_count)
            .enumerate()
            .for_each(|(i, r)| {
                pattern.set_event(i, track as usize, r.tracks[0]);
            });
    }
    /// overwrite the entire pattern with the buffered pattern. Throws a PatternPasteLength error if the buffered pattern is longer or shorter than the input pattern
    pub fn paste_pattern(&self, pattern: &mut Pattern) -> Result<()> {
        let Some(buf) = &self.pattern_buffer else {
            return Ok(());
        };
        if buf.height != pattern.row_count() {
            return Err(crate::error::MTrakError::PatternPasteLength {
                copied_length: buf.height,
            });
        }

        self.paste_pattern_keeping_length(pattern);
        return Ok(());
    }
    /// overwrite the entire pattern with the buffered pattern, keeping the pattern's length: buffered rows past its end are dropped and rows past the buffer's end are cleared
    pub fn paste_pattern_keeping_length(&self, pattern: &mut Pattern) {
        let Some(buf) = &self.pattern_buffer else {
            return;
        };
        let track_count = pattern.track_count as usize;
        for row in 0..pattern.row_count() as usize {
            for track in 0..track_count {
                let event = buf
                    .rows
                    .get(row)
                    .and_then(|r| r.tracks.get(track))
                    .copied()
                    .unwrap_or_else(Note::empty);
                pattern.set_event(row, track, event);
            }
        }
    }
    /// paste the buffered block with its top left corner at the given position. Anything falling outside the pattern is truncated
    pub fn paste_block(&self, pattern: &mut Pattern, start_track: u8, start_row: u8) {
        let Some(buf) = &self.block_buffer else {
            return;
        };

        let row_count = pattern.row_count() as usize;
        let track_count = pattern.track_count as usize;
        let start_row = start_row as usize;
        let start_track = start_track as usize;
        for (r, row) in buf.rows.iter().take(buf.height as usize).enumerate() {
            let target_row = start_row + r;
            if target_row >= row_count {
                break;
            }
            for (t, event) in row.tracks.iter().take(buf.width as usize).enumerate() {
                let target_track = start_track + t;
                if target_track >= track_count {
                    break;
                }
                pattern.set_event(target_row, target_track, *event);
            }
        }
    }
}

impl Default for CopyBuffers {
    fn default() -> Self {
        return Self::new();
    }
}

pub struct CopyBuffer {
    width: u8,
    height: u16,
    rows: Vec<PatternRow>,
}

impl CopyBuffer {
    fn new(width: u8, height: u16, rows: Vec<PatternRow>) -> Self {
        return Self {
            width,
            height,
            rows,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::note::{NoteKind, NotePitch};

    fn note(pitch: u8) -> Note {
        let mut note = Note::empty();
        note.kind = NoteKind::On(NotePitch(pitch));
        return note;
    }

    #[test]
    fn block_round_trips_and_truncates_at_edges() {
        let mut pattern = Pattern::new(4);
        pattern.set_event(0, 0, note(60));
        pattern.set_event(1, 1, note(61));
        let mut buffers = CopyBuffers::new();
        buffers.copy_block(
            &pattern,
            Block {
                track: 0,
                width: 2,
                row: 0,
                height: 2,
            },
        );
        let last_track = pattern.track_count - 1;
        buffers.paste_block(&mut pattern, last_track, 3);
        assert_eq!(pattern.get_event(3, last_track as usize), Some(&note(60)));
        assert_eq!(pattern.rows.len(), 4);
    }

    #[test]
    fn block_reaching_the_last_row_and_track_can_be_copied() {
        let pattern = Pattern::new(4);
        let mut buffers = CopyBuffers::new();
        buffers.copy_block(
            &pattern,
            Block {
                track: pattern.track_count - 1,
                width: 1,
                row: 3,
                height: 1,
            },
        );
        assert!(buffers.block_buffer.is_some());
    }

    #[test]
    fn invalid_cut_keeps_previous_block() {
        let mut pattern = Pattern::new(4);
        pattern.set_event(0, 0, note(60));
        let mut buffers = CopyBuffers::new();
        buffers.copy_block(
            &pattern,
            Block {
                track: 0,
                width: 1,
                row: 0,
                height: 1,
            },
        );
        buffers.cut_block(
            &mut pattern,
            Block {
                track: 0,
                width: 1,
                row: 3,
                height: 2,
            },
        );
        buffers.paste_block(&mut pattern, 1, 0);
        assert_eq!(pattern.get_event(0, 1), Some(&note(60)));
    }

    #[test]
    fn cut_track_clears_only_that_track() {
        let mut pattern = Pattern::new(2);
        pattern.set_event(0, 0, note(60));
        pattern.set_event(0, 1, note(61));
        let mut buffers = CopyBuffers::new();
        buffers.cut_track(&mut pattern, 0);
        assert_eq!(pattern.get_event(0, 0), Some(&Note::empty()));
        assert_eq!(pattern.get_event(0, 1), Some(&note(61)));
        buffers.paste_track(&mut pattern, 2);
        assert_eq!(pattern.get_event(0, 2), Some(&note(60)));
    }

    #[test]
    fn longer_pattern_does_not_paste_into_shorter_one() {
        let long = Pattern::new(8);
        let mut short = Pattern::new(4);
        let mut buffers = CopyBuffers::new();
        buffers.copy_pattern(&long);
        assert!(buffers.paste_pattern(&mut short).is_err());
    }

    #[test]
    fn shorter_pattern_does_not_paste_into_longer_one() {
        let short = Pattern::new(4);
        let mut long = Pattern::new(8);
        let mut buffers = CopyBuffers::new();
        buffers.copy_pattern(&short);
        assert!(matches!(
            buffers.paste_pattern(&mut long),
            Err(crate::error::MTrakError::PatternPasteLength { copied_length: 4 })
        ));
    }

    #[test]
    fn shorter_pattern_paste_keeping_length_clears_the_rest() {
        let mut short = Pattern::new(4);
        short.set_event(3, 0, note(60));
        let mut long = Pattern::new(8);
        long.set_event(6, 0, note(61));
        let mut buffers = CopyBuffers::new();
        buffers.copy_pattern(&short);
        buffers.paste_pattern_keeping_length(&mut long);
        assert_eq!(long.rows.len(), 8);
        assert_eq!(long.get_event(3, 0), Some(&note(60)));
        assert_eq!(long.get_event(6, 0), Some(&Note::empty()));
    }

    #[test]
    fn truncated_pattern_paste_fills_the_shorter_pattern() {
        let mut long = Pattern::new(8);
        long.set_event(3, 0, note(60));
        long.set_event(7, 0, note(61));
        let mut short = Pattern::new(4);
        let mut buffers = CopyBuffers::new();
        buffers.copy_pattern(&long);
        buffers.paste_pattern_keeping_length(&mut short);
        assert_eq!(short.rows.len(), 4);
        assert_eq!(short.get_event(3, 0), Some(&note(60)));
    }
}
