use ratatui::{
    buffer::Buffer,
    crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers},
    layout::{Constraint, Layout, Rect},
    symbols::line,
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Paragraph, Widget},
};
use std::{
    cell::RefCell,
    iter::{self, repeat_n},
    ops::Deref,
};

use crate::{
    data::{
        effect::Command,
        midi::{MIDI_CHANNEL_MAX, VOLUME_MAX},
        note::{Note, NoteKind},
        pattern::PatternId,
    },
    engine::engine::PlayMode,
    tui::{
        app::{AppState, TrackCache, column_index_to_note_string_index},
        constants::{self, NOTE_OFF_KEY},
        framework::{
            keymap::{ALT_SYMBOL, Key, KeyActions, KeyHint, Keymap},
            widget::MtrakWidget,
        },
        glyphs,
    },
    util::strings::split_keep_delim,
};

const TRACK_PADDING_TOP: u16 = 1;
const ROWS_TOP_OFFSET: u16 = 1 + TRACK_PADDING_TOP;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TimelineAction {
    Save,
    RowUp,
    RowDown,
    PrevTrack,
    NextTrack,
    PrevColumn,
    NextColumn,
    ToggleEdit,
    ToggleRecord,
    Jump16Down,
    Jump16Up,
    JumpToFirstRow,
    JumpToLastRow,
    /// FT2's F9..F12: jump to one of the fixed rows in `JUMP_ROWS`.
    JumpToMarker(u8),
    NoteOff,
    /// Clears the volume when the cursor is on the volume column, otherwise the note.
    DeleteAtCursor,
    DeleteAll,
    DeleteVolumeAndEffect,
    DeleteEffect,
    /// Removes the event above the cursor, pulling the rest of the track up.
    DeletePreviousNote,
    /// Removes the row above the cursor, pulling every track up.
    DeletePreviousLine,
    /// Pushes the track down from the cursor, leaving an empty event.
    InsertNote,
    /// Pushes every track down from the cursor, leaving an empty row.
    InsertLine,
}

/// Rows F9..F12 jump to, as in FT2. Clamped to the last row for shorter patterns.
const JUMP_ROWS: [u8; 4] = [0, 16, 32, 48];
/// Alt+key jumps to the track at the key's index, as in FT2.
const TRACK_JUMP_KEYS: [char; 8] = ['q', 'w', 'e', 'r', 't', 'y', 'u', 'i'];

/// Cursor movement shared by both modes. As in FT2, left/right move between columns and
/// run over into the neighbouring track, while Tab/Shift+Tab move a whole track.
fn movement_keys(keymap: Keymap<TimelineAction>) -> Keymap<TimelineAction> {
    use TimelineAction::*;
    let keymap = keymap
        .bind(Key::plain(KeyCode::Up), RowUp, "ROW")
        .bind(Key::plain(KeyCode::Down), RowDown, "ROW")
        .bind(Key::plain(KeyCode::Left), PrevColumn, "COL")
        .bind(Key::plain(KeyCode::Right), NextColumn, "COL")
        .bind(Key::plain(KeyCode::Tab), NextTrack, "TRCK")
        .bind(
            Key::new(KeyCode::Tab, KeyModifiers::SHIFT),
            PrevTrack,
            "TRCK",
        )
        .bind(
            Key::plain(KeyCode::PageUp),
            Jump16Up,
            glyphs::pick("±16", "+-16"),
        )
        .bind(
            Key::plain(KeyCode::PageDown),
            Jump16Down,
            glyphs::pick("±16", "+-16"),
        )
        .bind(Key::plain(KeyCode::Home), JumpToFirstRow, "Top")
        .bind(Key::plain(KeyCode::End), JumpToLastRow, "End");
    return (0..JUMP_ROWS.len() as u8).fold(keymap, |keymap, i| {
        keymap.bind(Key::plain(KeyCode::F(9 + i)), JumpToMarker(i), "JMP")
    });
}

fn normal_keymap() -> Keymap<TimelineAction> {
    use TimelineAction::*;
    return movement_keys(
        Keymap::new()
            .bind(Key::char(' '), ToggleEdit, "Edit")
            .bind(Key::plain(KeyCode::Esc), ToggleRecord, "Rec")
            .bind(Key::char(NOTE_OFF_KEY), NoteOff, "Off")
            .bind(Key::ctrl('s'), Save, "Save"),
    );
}

fn edit_keymap() -> Keymap<TimelineAction> {
    use TimelineAction::*;
    return movement_keys(
        Keymap::new()
            .bind(Key::char(' '), ToggleEdit, "Edit")
            .bind(Key::plain(KeyCode::Esc), ToggleRecord, "Rec")
            .bind(Key::char(NOTE_OFF_KEY), NoteOff, "Off")
            .bind(Key::plain(KeyCode::Delete), DeleteAtCursor, "Del")
            .bind(Key::plain(KeyCode::Backspace), DeletePreviousNote, "Del")
            .bind(
                Key::new(KeyCode::Delete, KeyModifiers::SHIFT),
                DeleteAll,
                "Del",
            )
            .bind(
                Key::new(KeyCode::Backspace, KeyModifiers::ALT),
                DeletePreviousLine,
                "Del",
            )
            .bind(
                Key::new(KeyCode::Delete, KeyModifiers::CONTROL),
                DeleteVolumeAndEffect,
                "Del",
            )
            .bind(
                Key::new(KeyCode::Delete, KeyModifiers::ALT),
                DeleteEffect,
                "Del",
            )
            .bind(Key::plain(KeyCode::Insert), InsertNote, "Ins")
            .bind(
                Key::new(KeyCode::Insert, KeyModifiers::ALT),
                InsertLine,
                "Ins",
            )
            .bind(Key::ctrl('s'), Save, "Save"),
    );
}

/// The track an Alt+letter press jumps to, if it is one of `TRACK_JUMP_KEYS`.
fn track_jump(key: &KeyEvent) -> Option<u8> {
    if key.modifiers != KeyModifiers::ALT {
        return None;
    }
    let KeyCode::Char(c) = key.code else {
        return None;
    };
    let index = TRACK_JUMP_KEYS.iter().position(|&k| k == c)? as u8;
    return (index < constants::TRACK_COUNT).then_some(index);
}

fn wrap_step(index: u8, step: i32, count: u8) -> u8 {
    if count == 0 {
        return 0;
    }
    return (index as i32 + step).rem_euclid(count as i32) as u8;
}

/// Index of the line the selected row is pinned to in a view `height` rows tall.
/// For even heights this is the upper of the two middle lines, so the pattern start line
/// above it and the rows below it split the view evenly.
fn middle_line(height: u16) -> usize {
    return (height.saturating_sub(1) / 2) as usize;
}

/// Builds a paragraph scrolled so that line `selected` sits in the middle of a view `height`
/// rows tall. Blank lines are prepended so the first rows can also reach the middle,
/// which keeps the highlight fixed while the rows scroll past it.
fn scrolled_to_middle<'a>(lines: Vec<Line<'a>>, selected: u8, height: u16) -> Paragraph<'a> {
    let middle = middle_line(height);
    let padded: Vec<Line> = iter::repeat_n(Line::default(), middle)
        .chain(lines)
        .collect();
    return Paragraph::new(padded).scroll((selected as u16, 0));
}
enum TimelineColumnLocation {
    Note,
    Instrument,
    Volume { is_on_first_column: bool },
    Effect { column_offset: u8 },
}
impl TimelineColumnLocation {
    pub fn from_column_index(index: u8) -> Self {
        match index {
            0 => TimelineColumnLocation::Note,
            1 => TimelineColumnLocation::Instrument,
            2 => TimelineColumnLocation::Volume {
                is_on_first_column: true,
            },
            3 => TimelineColumnLocation::Volume {
                is_on_first_column: false,
            },
            4 | 5 | 6 => TimelineColumnLocation::Effect {
                column_offset: index - 4,
            },
            _ => panic!(),
        }
    }
}
pub struct TimeLineView {
    /// render cache for storing pattern tracks efficiently, maps pattern IDs to tracks
    pub track_cache: RefCell<TrackCache>,
    pub row_index: u8,
    pub column_index: u8,
    pub track_index: u8,
    pub is_editing: bool,
    pub row_number_lookup: String,
    normal_keys: Keymap<TimelineAction>,
    edit_keys: Keymap<TimelineAction>,
}
impl TimeLineView {
    pub fn new(state: &AppState) -> Self {
        let patterns = &state.project.patterns;
        let track_cache = TrackCache::new(patterns.get_patterns());
        return Self {
            track_cache: RefCell::new(track_cache),
            row_index: 0,
            track_index: 0,
            column_index: 0,
            is_editing: false,
            row_number_lookup: String::new(),
            normal_keys: normal_keymap(),
            edit_keys: edit_keymap(),
        };
    }
}
impl KeyActions for TimeLineView {
    type Action = TimelineAction;

    fn keymap(&self) -> &Keymap<TimelineAction> {
        if self.is_editing {
            return &self.edit_keys;
        }
        return &self.normal_keys;
    }

    fn perform(&mut self, action: TimelineAction, state: &mut AppState) {
        match action {
            TimelineAction::Save => {
                let project_name = state.project.name.clone();
                let config = state.config.clone();

                if let Some(_) = &project_name {
                    state.project.save(&config, project_name).unwrap();
                } else {
                    // ask for project name
                }
            }
            TimelineAction::RowUp => {
                let row_count = state.active_pattern().row_count;
                self.row_index = wrap_step(self.row_index, -1, row_count);
            }
            TimelineAction::RowDown => {
                let row_count = state.active_pattern().row_count;
                self.row_index = wrap_step(self.row_index, 1, row_count);
            }
            TimelineAction::PrevTrack => {
                self.track_index = wrap_step(self.track_index, -1, constants::TRACK_COUNT);
                self.column_index = 0;
            }
            TimelineAction::NextTrack => {
                self.track_index = wrap_step(self.track_index, 1, constants::TRACK_COUNT);
                self.column_index = 0;
            }
            TimelineAction::PrevColumn => {
                if self.column_index > 0 {
                    self.column_index -= 1;
                } else {
                    self.track_index = wrap_step(self.track_index, -1, constants::TRACK_COUNT);
                    self.column_index = constants::TRACK_COLUMN_COUNT - 1;
                }
            }
            TimelineAction::NextColumn => {
                if self.column_index + 1 < constants::TRACK_COLUMN_COUNT {
                    self.column_index += 1;
                } else {
                    self.track_index = wrap_step(self.track_index, 1, constants::TRACK_COUNT);
                    self.column_index = 0;
                }
            }
            TimelineAction::ToggleEdit => {
                if state.engine.status().play_mode() == PlayMode::Paused {
                    self.is_editing = !self.is_editing;
                } else {
                    state.engine.stop();
                }
            }
            TimelineAction::ToggleRecord => {
                if state.engine.status().play_mode() == PlayMode::Paused {
                    state.recording = !state.recording;
                }
            }
            TimelineAction::Jump16Down => {
                let last_row = state.active_pattern().row_count.saturating_sub(1);
                self.row_index = self.row_index.saturating_add(16).min(last_row);
            }
            TimelineAction::Jump16Up => {
                self.row_index = self.row_index.saturating_sub(16);
            }
            TimelineAction::JumpToFirstRow => {
                self.row_index = 0;
            }
            TimelineAction::JumpToLastRow => {
                self.row_index = state.active_pattern().row_count.saturating_sub(1);
            }
            TimelineAction::JumpToMarker(i) => {
                let last_row = state.active_pattern().row_count.saturating_sub(1);
                self.row_index = JUMP_ROWS[i as usize].min(last_row);
            }
            TimelineAction::NoteOff => {
                if !self.is_editing {
                    state.engine.release_note(self.track_index as usize);
                    return;
                }
                let row = self.row_index as usize;
                let track = self.track_index as usize;
                let pattern = state.active_pattern_mut();
                let Some(mut note) = pattern.get_event(row, track).copied() else {
                    return;
                };
                note.kind = NoteKind::Off;
                pattern.set_event(row, track, note);
                self.refresh_row_and_advance(state, row);
            }
            TimelineAction::DeleteAtCursor => {
                let on_volume = matches!(
                    TimelineColumnLocation::from_column_index(self.column_index),
                    TimelineColumnLocation::Volume { .. }
                );
                self.edit_and_advance(state, |note| {
                    if on_volume {
                        note.clear_volume();
                    } else {
                        note.clear_note();
                    }
                });
            }
            TimelineAction::DeleteAll => {
                self.edit_and_advance(state, |note| *note = Note::empty());
            }
            TimelineAction::DeleteVolumeAndEffect => {
                self.edit_and_advance(state, |note| {
                    note.clear_volume();
                    note.clear_command();
                });
            }
            TimelineAction::DeleteEffect => {
                self.edit_and_advance(state, Note::clear_command);
            }
            TimelineAction::DeletePreviousNote => {
                let Some(row) = self.row_index.checked_sub(1) else {
                    return;
                };
                let track = self.track_index as usize;
                state.active_pattern_mut().delete_event(row as usize, track);
                self.refresh_rows_from(state, row as usize);
                self.row_index = row;
            }
            TimelineAction::DeletePreviousLine => {
                let Some(row) = self.row_index.checked_sub(1) else {
                    return;
                };
                state.active_pattern_mut().delete_line(row as usize);
                self.refresh_rows_from(state, row as usize);
                self.row_index = row;
            }
            TimelineAction::InsertNote => {
                let (row, track) = (self.row_index as usize, self.track_index as usize);
                state.active_pattern_mut().insert_event(row, track);
                self.refresh_rows_from(state, row);
            }
            TimelineAction::InsertLine => {
                let row = self.row_index as usize;
                state.active_pattern_mut().insert_line(row);
                self.refresh_rows_from(state, row);
            }
        }
    }
}
impl TimeLineView {
    fn refresh_row_and_advance(&mut self, state: &mut AppState, row: usize) {
        self.refresh_row(state, row);
        let row_count = state.active_pattern().row_count;
        self.row_index = wrap_step(self.row_index, state.line_add as i32, row_count);
    }
    /// Edits the event under the cursor, then moves down by the line add like entering a note.
    fn edit_and_advance(&mut self, state: &mut AppState, edit: impl FnOnce(&mut Note)) {
        let row = self.row_index as usize;
        state.edit_event(row, self.track_index as usize, edit);
        self.refresh_row_and_advance(state, row);
    }
    fn refresh_rows_from(&mut self, state: &mut AppState, row: usize) {
        for row in row..state.active_pattern().row_count as usize {
            self.refresh_row(state, row);
        }
    }
    fn refresh_row(&mut self, state: &mut AppState, row: usize) {
        self.track_cache
            .borrow_mut()
            .update_dirty(&mut state.project, state.active_pattern, row);
    }

    fn enter_note(&mut self, state: &mut AppState, key: KeyEvent) -> bool {
        let row = self.row_index as usize;
        if state
            .enter_key(self.track_index as usize, row, key)
            .is_none()
        {
            return false;
        }
        self.refresh_row_and_advance(state, row);
        return true;
    }
    fn enter_instrument(&mut self, state: &mut AppState, instrument: u8) -> bool {
        if instrument > MIDI_CHANNEL_MAX {
            return false;
        }
        state.enter_instrument(
            self.track_index as usize,
            self.row_index as usize,
            instrument,
        );
        self.refresh_row_and_advance(state, self.row_index as usize);
        return true;
    }
    fn enter_volume(&mut self, state: &mut AppState, digits: [char; 2]) -> bool {
        let digits: String = digits
            .iter()
            .map(|&c| if c == '-' { '0' } else { c })
            .collect();
        let Ok(volume) = u8::from_str_radix(&digits, 16) else {
            return false;
        };
        state.enter_volume(
            self.track_index as usize,
            self.row_index as usize,
            volume.min(VOLUME_MAX),
        );
        self.refresh_row_and_advance(state, self.row_index as usize);
        return true;
    }
    fn enter_command(&mut self, state: &mut AppState, digits: [char; 3]) -> bool {
        let Ok(command) = Command::from_str(&digits.iter().collect::<String>()) else {
            return false;
        };

        state.enter_command(self.track_index as usize, self.row_index as usize, command);
        self.refresh_row_and_advance(state, self.row_index as usize);
        return true;
    }
    /// In edit mode, a hex digit replaces the column under the cursor in the selected cell.
    fn enter_note_digit(&mut self, state: &mut AppState, digit: char) -> bool {
        let line = &self
            .track_cache
            .borrow()
            .get_row(state.active_pattern, self.row_index as usize)
            .tracks[self.track_index as usize]
            .spans[0]
            .content
            .as_ref()
            .to_owned();
        let digit = digit.to_ascii_uppercase();
        let char_index = column_index_to_note_string_index(self.column_index as usize);
        let mut chars: Vec<char> = line.chars().map(|c| c.to_ascii_uppercase()).collect();
        chars[char_index.unwrap()] = digit;

        let location = TimelineColumnLocation::from_column_index(self.column_index);
        return match location {
            TimelineColumnLocation::Note => self.enter_note(
                state,
                KeyEvent::new(KeyCode::Char(digit), KeyModifiers::NONE),
            ),
            TimelineColumnLocation::Instrument => {
                let instrument: u8 = if let Ok(i) = u8::from_str_radix(&digit.to_string(), 16) {
                    i
                } else {
                    return false;
                };
                self.enter_instrument(state, instrument)
            }
            TimelineColumnLocation::Volume { .. } => {
                let high = column_index_to_note_string_index(2).unwrap();
                let low = column_index_to_note_string_index(3).unwrap();
                self.enter_volume(state, [chars[high], chars[low]])
            }
            TimelineColumnLocation::Effect { .. } => {
                let kind = column_index_to_note_string_index(4).unwrap();
                let val_high = column_index_to_note_string_index(5).unwrap();
                let val_low = column_index_to_note_string_index(6).unwrap();
                self.enter_command(state, [chars[kind], chars[val_high], chars[val_low]])
            }
        };
    }

    /// The screen row just above row 00, or `None` when it has scrolled out of view.
    fn pattern_start_y(&self, rows_top: u16, rows_height: u16) -> Option<u16> {
        let middle = middle_line(rows_height);
        let offset = middle.checked_sub(self.row_index as usize + 1)?;
        return Some(rows_top + offset as u16);
    }

    /// The screen row just below the last row, or `None` when it has scrolled out of view.
    fn pattern_end_y(&self, row_count: u8, rows_top: u16, rows_height: u16) -> Option<u16> {
        let middle = middle_line(rows_height);
        let offset = middle + (row_count - self.row_index) as usize;
        return (offset < rows_height as usize).then(|| rows_top + offset as u16);
    }

    /// Draws a horizontal line across the line numbers and tracks at `y`, joining the
    /// track borders, to mark where the pattern begins or ends.
    fn render_pattern_boundary_line(&self, state: &AppState, width: u16, y: u16, buf: &mut Buffer) {
        let style = state.theme.pattern_start;
        Line::raw(repeat_n(line::HORIZONTAL, width as usize).collect::<String>())
            .style(style)
            .render(Rect::new(0, y, width, 1), buf);
    }

    fn render_mode_indicator(&self, state: &AppState, area: Rect, buf: &mut Buffer) {
        let recording = state.theme.recording;
        let spans = match state.engine.status().play_mode() {
            PlayMode::Song => vec![Span::styled("-- REC SONG --", recording)],
            PlayMode::SongLoop => vec![Span::raw("-- LOOP SONG --")],
            PlayMode::Pattern => vec![Span::styled("-- REC PTN --", recording)],
            PlayMode::PatternLoop => vec![Span::raw("-- LOOP PTN --")],
            PlayMode::Paused => {
                let mut spans = Vec::new();
                if self.is_editing {
                    spans.push(Span::raw("-- EDIT MODE --"));
                }
                if self.is_editing && state.recording {
                    spans.push(Span::raw(" "));
                }
                if state.recording {
                    spans.push(Span::styled("-- REC MODE --", recording));
                }
                spans
            }
        };
        Line::from(spans).centered().render(area, buf);
    }

    fn render_line_numbers(
        &self,
        state: &AppState,
        row_count: u8,
        rows_height: u16,
        area: Rect,
        buf: &mut Buffer,
    ) {
        let lines = (0..row_count)
            .map(|i| {
                let mut style = state.theme.row_number;
                if i % 4 == 0 {
                    style = style.patch(state.theme.beat_row_number);
                }
                if i == self.row_index {
                    style = style.patch(state.theme.selected_row_number);
                }
                Line::raw(format!("{:02X}", i)).style(style)
            })
            .collect::<Vec<Line>>();
        scrolled_to_middle(lines, self.row_index, rows_height)
            .block(Block::new().padding(Padding::top(ROWS_TOP_OFFSET)))
            .right_aligned()
            .render(area, buf);
    }

    fn render_line_number_header(&self, state: &AppState, area: Rect, buf: &mut Buffer) {
        let lines = vec![Line::raw("No.")];
        Paragraph::new(lines).right_aligned().render(area, buf);
    }

    fn render_track(
        &self,
        state: &AppState,
        track_index: u8,
        rows_height: u16,
        area: Rect,
        buf: &mut Buffer,
    ) {
        // no bottom margin, so the rows run to the bottom of the area like the line numbers
        let track_layout = Layout::vertical([
            Constraint::Length(1 + TRACK_PADDING_TOP),
            Constraint::Fill(1),
        ])
        .horizontal_margin(1)
        .split(area);
        Line::raw(format!("Track {:02}", track_index + 1))
            .centered()
            .render(track_layout[0], buf);

        let cache = self.track_cache.borrow();
        let mut rows = cache.get_all_rows_for_track(state.active_pattern, track_index as usize);
        let selected_row = rows[self.row_index as usize].clone();
        rows[self.row_index as usize] =
            self.highlight_selected_row(state, track_index, selected_row);
        scrolled_to_middle(rows, self.row_index, rows_height).render(track_layout[1], buf);

        Block::bordered()
            .borders(Borders::LEFT | Borders::RIGHT)
            .border_style(state.theme.track_border)
            .render(area, buf);
    }

    fn highlight_selected_row<'a>(
        &self,
        state: &AppState,
        track_index: u8,
        row: Line<'a>,
    ) -> Line<'a> {
        if track_index != self.track_index {
            return row.centered().style(state.theme.selected_row);
        }
        let cell_style = if self.is_editing {
            state.theme.editing_cell
        } else {
            state.theme.selected_cell
        };
        let cell = row.clone().centered().style(cell_style);
        return cell.spans(self.cursor_spans(state, &row));
    }

    /// Splits a note string into one span per editable column so the column under the
    /// cursor can be styled on its own.
    fn cursor_spans(&self, state: &AppState, row: &Line) -> Vec<Span<'static>> {
        let note_string = String::from(row.spans[0].content.clone());
        let mut columns: Vec<String> = Vec::new();
        for (i, part) in split_keep_delim(&note_string, "|").iter().enumerate() {
            if i == 0 {
                columns.push(part.deref().to_owned());
                continue;
            }
            for char in part.chars() {
                columns.push(char.to_string());
            }
        }

        let mut column_number = 0;
        return columns
            .into_iter()
            .map(|s| {
                if s != "|" {
                    column_number += 1;
                }
                if column_number - 1 == self.column_index && s != "|" {
                    return Span::styled(s, state.theme.cursor);
                }
                return Span::from(s);
            })
            .collect();
    }
}
impl MtrakWidget for TimeLineView {
    fn update(&mut self, state: &mut AppState) {
        let status = state.engine.status();
        let mode = status.play_mode();
        if mode == PlayMode::Paused {
            return;
        }
        self.is_editing = false;
        let (sequence_index, pattern, row) =
            (status.sequence_index(), status.pattern(), status.row());
        state.active_pattern = PatternId(pattern as u8);
        if !matches!(mode, PlayMode::PatternLoop | PlayMode::Pattern) {
            state.active_sequence_index = sequence_index;
        }
        self.row_index = row as u8;
    }
    fn handle_event(
        &mut self,
        state: &mut AppState,
        event: &ratatui::crossterm::event::Event,
    ) -> bool {
        if self.dispatch(state, event) {
            return true;
        }
        let Event::Key(key) = event else {
            return false;
        };
        if let Some(track) = track_jump(key) {
            self.track_index = track;
            return true;
        }
        match key.code {
            KeyCode::F(mut f) => {
                f = f.clamp(0, 8);
                state.set_octave(f - 1);
            }
            _ => (),
        };
        if !self.is_editing {
            return state.play_key(self.track_index as usize, *key).is_some();
        }
        if self.column_index == 0 {
            return self.enter_note(state, *key);
        }

        // enter_note_digit validates per column; '+' is rejected because from_str_radix accepts it as a sign
        if let KeyCode::Char(digit) = key.code
            && digit != '+'
            && !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            return self.enter_note_digit(state, digit);
        }
        return false;
    }

    fn key_hints(&self, _state: &AppState, hints: &mut Vec<KeyHint>) {
        self.keymap().hints(hints);
        hints.push(KeyHint {
            key: format!("{}Q-I", ALT_SYMBOL),
            label: "TRCK",
        });
    }

    fn render(
        &mut self,
        state: &AppState,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
    ) {
        let pattern_store = &state.project.patterns;
        let pattern = pattern_store
            .get_pattern_by_id(state.active_pattern)
            .unwrap();
        self.track_cache
            .borrow_mut()
            .sync_pattern(state.active_pattern, pattern);
        self.row_index = self.row_index.min(pattern.row_count.saturating_sub(1));
        buf.set_style(area, state.theme.pattern_area);
        let top_bottom = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).split(area);
        self.render_mode_indicator(state, top_bottom[0], buf);

        let layout = Layout::horizontal(
            [Constraint::Min(0), Constraint::Length(2)]
                .iter()
                .chain(
                    (0..pattern.track_count)
                        .map(|_| &Constraint::Length(constants::NOTE_STRING_LENGTH as u16)),
                )
                .chain([Constraint::Min(0)].iter()),
        )
        .horizontal_margin(1)
        .split(top_bottom[1]);
        let rows_top = layout[0].y + ROWS_TOP_OFFSET;
        let rows_height = layout[0].height.saturating_sub(ROWS_TOP_OFFSET);

        self.render_line_numbers(state, pattern.row_count, rows_height, layout[1], buf);
        self.render_line_number_header(state, layout[1], buf);
        for track_index in 0..pattern.track_count as usize {
            self.render_track(
                state,
                track_index as u8,
                rows_height,
                layout[track_index + 2],
                buf,
            );
        }
        if let Some(y) = self.pattern_start_y(rows_top, rows_height) {
            self.render_pattern_boundary_line(state, area.width, y, buf);
        }
        if let Some(y) = self.pattern_end_y(pattern.row_count, rows_top, rows_height) {
            self.render_pattern_boundary_line(state, area.width, y, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alt_letters_jump_to_tracks() {
        let alt = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::ALT);
        assert_eq!(track_jump(&alt('q')), Some(0));
        assert_eq!(track_jump(&alt('i')), Some(7));
        assert_eq!(track_jump(&alt('a')), None);
        assert_eq!(track_jump(&alt('z')), None);
        assert_eq!(
            track_jump(&KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
            None
        );
    }
}
