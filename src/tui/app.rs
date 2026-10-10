use std::{
    collections::{HashMap, HashSet, VecDeque},
    io,
    time::{Duration, Instant},
};

use midi::Message;
use midir::{MidiOutput, MidiOutputPort};
use ratatui::{
    DefaultTerminal,
    crossterm::event::{self, Event, KeyCode, KeyCode::Char, KeyEvent, KeyEventKind},
    text::Line,
    widgets::Padding,
};

use crate::{
    data::{
        config::{Config, TITLE},
        effect::CommandKind,
        midi::{MIDI_CHANNEL_MAX, MidiEvent, VOLUME_MAX},
        note::{Note, NoteKind, NotePitch},
        pattern::{Pattern, PatternId},
        project::{Project, SequenceRow},
    },
    engine::{engine::PlayMode, handle::EngineHandle},
    tui::{
        framework::{
            core_extensions::Boxed, dialog::Dialog, widget::MtrakWidget,
            widgets::container::Container,
        },
        theme::Theme,
        views::main_view::MainView,
    },
};

use super::constants;

pub type Result<T> = io::Result<T>;
pub fn column_index_to_note_string_index(column_index: usize) -> Option<usize> {
    if column_index == 0 {
        return Some(0);
    }
    let note_end = constants::EMPTY_NOTE.find('|')?;
    return constants::EMPTY_NOTE
        .char_indices()
        .skip(note_end)
        .filter(|&(_, c)| c != '|')
        .nth(column_index - 1)
        .map(|(i, _)| i);
}

pub struct App {
    pub state: AppState,
    pub terminal: DefaultTerminal,
    pub fps: u16,
    pub root: Container,
    pub dialogs: Vec<Dialog>,
    held_keys: HashSet<KeyCode>,
    reports_releases: bool,
}

impl App {
    pub fn new(terminal: DefaultTerminal, fps: u16, project: Project) -> Self {
        let state = AppState::new(project);
        return Self {
            terminal,
            fps,
            root: Container::new(MainView::new(&state).boxed(), Padding::ZERO),
            dialogs: Vec::new(),
            held_keys: HashSet::new(),
            reports_releases: false,
            state,
        };
    }
    pub fn run(&mut self) -> Result<bool> {
        let frame_time = Duration::from_secs_f64(1.0 / self.fps as f64);

        let mut timeout = frame_time;
        while event::poll(timeout)? {
            let mut event = event::read()?;
            timeout = Duration::ZERO;
            if let Event::Key(key) = &mut event {
                self.mark_repeat(key);
                if key.kind == KeyEventKind::Release {
                    continue;
                }
            }
            match self.dialogs.last_mut() {
                Some(dialog) => dialog.handle_event(&mut self.state, &event),
                None => self.root.handle_event(&mut self.state, &event),
            };
            self.sync_dialogs();
        }
        self.state.update();
        self.root.update(&mut self.state);
        for dialog in &mut self.dialogs {
            dialog.update(&mut self.state);
        }
        self.sync_dialogs();
        self.state.send_changes_to_engine();
        self.terminal.draw(|f| {
            let area = f.area();
            f.buffer_mut().set_style(area, self.state.theme.desktop);
            self.root.render(&self.state, f.area(), f.buffer_mut());
            for dialog in &mut self.dialogs {
                dialog.render(&self.state, f.area(), f.buffer_mut());
            }
        })?;
        return Ok(!self.state.exit_requested);
    }

    /// Marks presses of keys that are still held as repeats.
    fn mark_repeat(&mut self, key: &mut KeyEvent) {
        match key.kind {
            KeyEventKind::Release => {
                self.reports_releases = true;
                self.held_keys.remove(&key.code);
            }
            KeyEventKind::Press if self.reports_releases => {
                if !self.held_keys.insert(key.code) {
                    key.kind = KeyEventKind::Repeat;
                }
            }
            _ => {}
        }
    }

    fn sync_dialogs(&mut self) {
        self.dialogs.retain(|d| !d.is_closed());
        self.dialogs.append(&mut self.state.pending_dialogs);
    }
}
const EVENT_HISTORY_MAX: usize = 256;

#[derive(Clone, Copy)]
pub struct ChannelHit {
    pub at: Instant,
    pub velocity: u8,
}

pub struct AppState {
    pub config: Config,
    pub project: Project,
    pub engine: EngineHandle,

    pub midi_client: Option<MidiOutput>,
    pub theme: Theme,
    pub exit_requested: bool,

    pub active_sequence_index: usize,
    pub active_pattern: PatternId,
    pub active_port: Option<MidiOutputPort>,
    pub line_add: u8,
    pub active_instrument: u8,
    pub event_history: VecDeque<MidiEvent>,
    pub channel_hits: [Option<ChannelHit>; MIDI_CHANNEL_MAX as usize + 1],
    pub current_octave: u8,
    pub recording: bool,
    pending_dialogs: Vec<Dialog>,
    changed_patterns: HashSet<PatternId>,
    sequence_changed: bool,
}
impl AppState {
    pub fn new(project: Project) -> Self {
        let changed_patterns = (0..project.patterns.get_patterns().len() as u8)
            .map(PatternId)
            .collect();
        let state = Self {
            config: Config::default(),
            project,
            engine: EngineHandle::spawn(),
            theme: Theme::detect(),
            exit_requested: false,
            active_sequence_index: 0,
            active_port: None,
            midi_client: MidiOutput::new(TITLE).ok(),
            active_instrument: 0,
            active_pattern: PatternId(0),
            current_octave: 4,
            recording: false,
            line_add: 1,
            pending_dialogs: Vec::new(),
            changed_patterns,
            event_history: VecDeque::new(),
            channel_hits: [None; _],
            sequence_changed: true,
        };
        for (channel, volume) in state.project.global_instrument_volumes.iter().enumerate() {
            state.engine.set_global_volume(channel as u8, *volume);
        }
        return state;
    }
    pub fn update(&mut self) {
        let now = Instant::now();
        let events: Vec<MidiEvent> = self.engine.drain_events().collect();
        for event in events {
            self.record_hits(&event, now);
            if self.event_history.len() == EVENT_HISTORY_MAX {
                self.event_history.pop_front();
            }
            self.event_history.push_back(event);
        }

        let status = self.engine.status();
        if status.play_mode() == PlayMode::Paused {
            return;
        }

        if status.pattern() as u8 != self.active_pattern.0 {
            self.active_pattern.0 = status.pattern() as u8;
        }
    }
    fn record_hits(&mut self, event: &MidiEvent, at: Instant) {
        match event {
            MidiEvent::Message(Message::NoteOn(channel, _, velocity)) if *velocity > 0 => {
                self.channel_hits[*channel as usize] = Some(ChannelHit {
                    at,
                    velocity: *velocity,
                });
            }
            MidiEvent::Aggregate(events) => {
                for event in events {
                    self.record_hits(event, at);
                }
            }
            _ => {}
        }
    }
    pub fn get_midi_port_names(&self) -> Vec<String> {
        let Some(client) = &self.midi_client else {
            return vec![];
        };
        return client
            .ports()
            .iter()
            .map(|p| client.port_name(p).unwrap_or(p.id()))
            .collect::<Vec<String>>();
    }

    pub fn active_pattern(&self) -> &Pattern {
        self.project
            .patterns
            .get_pattern_by_id(self.active_pattern)
            .unwrap()
    }
    pub fn active_pattern_mut(&mut self) -> &mut Pattern {
        self.changed_patterns.insert(self.active_pattern);
        self.project
            .patterns
            .get_pattern_by_id_mut(self.active_pattern)
            .unwrap()
    }
    pub fn open_dialog(&mut self, dialog: Dialog) {
        self.pending_dialogs.push(dialog);
    }
    pub fn request_exit(&mut self) {
        self.exit_requested = true;
    }

    pub fn next_instrument(&mut self) -> u8 {
        self.active_instrument = self
            .active_instrument
            .saturating_add(1)
            .clamp(0, MIDI_CHANNEL_MAX);
        return self.active_instrument;
    }
    pub fn prev_instrument(&mut self) -> u8 {
        self.active_instrument = self.active_instrument.saturating_sub(1);
        return self.active_instrument;
    }
    pub fn increase_line_add(&mut self) {
        self.line_add = self.line_add.saturating_add(1);
    }

    pub fn decrease_line_add(&mut self) {
        self.line_add = self.line_add.saturating_sub(1);
    }

    pub fn next_pattern(&mut self) {
        self.active_pattern.0 = self.active_pattern.0.saturating_add(1);
        self.ensure_pattern(self.active_pattern);
    }

    pub fn prev_pattern(&mut self) {
        self.active_pattern.0 = self.active_pattern.0.saturating_sub(1);
        self.ensure_pattern(self.active_pattern);
    }

    fn ensure_pattern(&mut self, id: PatternId) {
        self.project.patterns.create_pattern_id_if_not_exists(id);
        self.changed_patterns.insert(id);
    }

    pub fn change_sequence_pattern(&mut self, index: usize, increase: bool) {
        let row = self.sequence_row_mut(index);
        let id = if increase {
            row.increate_pattern()
        } else {
            row.decrease_pattern()
        };
        self.ensure_pattern(id);
    }

    pub fn insert_sequence(&mut self, index: usize, pattern_id: PatternId) {
        self.sequence_changed = true;
        self.project.insert_sequence(index, pattern_id);
    }
    pub fn delete_sequence(&mut self, index: usize) -> usize {
        self.sequence_changed = true;
        return self.project.delete_sequence(index);
    }
    pub fn sequence_row_mut(&mut self, index: usize) -> &mut SequenceRow {
        self.sequence_changed = true;
        return &mut self.project.sequence[index];
    }

    pub fn play_song(&mut self, sequence_index: usize) {
        self.send_all_to_engine();
        self.engine.play_song(sequence_index, !self.recording);
    }

    pub fn play_pattern(&mut self, id: PatternId) {
        self.send_all_to_engine();
        self.engine.play_pattern(id, !self.recording);
    }
    pub fn set_octave(&mut self, octave: u8) {
        self.current_octave = octave.clamp(0, 7);
    }
    fn key_to_note_pitch(&self, key: char) -> Option<NotePitch> {
        let bottom_octave = [
            'z', 's', 'x', 'd', 'c', 'v', 'g', 'b', 'h', 'n', 'j', 'm', ',',
        ];
        let top_octave = [
            'q', '2', 'w', '3', 'e', 'r', '5', 't', '6', 'y', '7', 'u', 'i',
        ];
        let note = bottom_octave
            .iter()
            .position(|p| *p == key)
            .or_else(|| top_octave.iter().position(|p| *p == key).map(|n| n + 12))?
            as u8;
        return Some(NotePitch::new(note + 12 * (self.current_octave + 1)));
    }
    fn send_all_to_engine(&mut self) {
        self.changed_patterns.clear();
        self.sequence_changed = false;
        for (i, pattern) in self.project.patterns.get_patterns().iter().enumerate() {
            self.engine.set_pattern(PatternId(i as u8), pattern.clone());
        }
        self.engine.set_sequence(self.project.sequence.clone());
    }

    fn send_changes_to_engine(&mut self) {
        for id in self.changed_patterns.drain() {
            if let Some(pattern) = self.project.patterns.get_pattern_by_id(id) {
                self.engine.set_pattern(id, pattern.clone());
            }
        }
        if self.sequence_changed {
            self.sequence_changed = false;
            self.engine.set_sequence(self.project.sequence.clone());
        }
    }
    pub fn enter_key(
        &mut self,
        track: usize,
        row: usize,
        key: event::KeyEvent,
    ) -> Option<NotePitch> {
        let pitch = self.play_key(track, key)?;
        let instrument = self.active_instrument;
        let pattern = self.active_pattern_mut();
        let Some(mut note) = pattern.get_event(row, track).copied() else {
            return None;
        };
        note.kind = NoteKind::On(pitch);
        note.instrument_id = note.instrument_id.or(Some(instrument));
        pattern.set_event(row, track, note);
        return Some(pitch);
    }
    pub fn play_key(&self, track: usize, key: event::KeyEvent) -> Option<NotePitch> {
        if !key.modifiers.is_empty() {
            return None;
        }
        let Char(c) = key.code else {
            return None;
        };
        let Some(note) = self.key_to_note_pitch(c) else {
            return None;
        };

        if key.kind != KeyEventKind::Repeat {
            self.engine
                .play_note(track, self.active_instrument, note.clone());
        }
        return Some(note);
    }

    /// Applies `edit` to the event at `row` in `track` of the active pattern.
    pub fn edit_event(&mut self, row: usize, track: usize, edit: impl FnOnce(&mut Note)) {
        let pattern = self.active_pattern_mut();
        let Some(mut note) = pattern.get_event(row, track).copied() else {
            return;
        };
        edit(&mut note);
        pattern.set_event(row, track, note);
    }

    pub fn enter_instrument(&mut self, track_index: usize, row_index: usize, instrument: u8) {
        if instrument > MIDI_CHANNEL_MAX {
            return;
        };
        let mut current_event = *self
            .active_pattern()
            .get_event(row_index, track_index)
            .unwrap();
        current_event.instrument_id = Some(instrument);
        self.active_pattern_mut()
            .set_event(row_index, track_index, current_event);
    }

    pub fn enter_volume(&mut self, track_index: usize, row_index: usize, volume: u8) {
        if volume > VOLUME_MAX {
            return;
        };
        let mut current_event = *self
            .active_pattern()
            .get_event(row_index, track_index)
            .unwrap();
        current_event.velocity = Some(volume);
        self.active_pattern_mut()
            .set_event(row_index, track_index, current_event);
    }

    pub fn enter_command(
        &mut self,
        track_index: usize,
        row_index: usize,
        command: crate::data::effect::Command,
    ) {
        let mut current_event = *self
            .active_pattern()
            .get_event(row_index, track_index)
            .unwrap();
        current_event.command = command;
        if current_event.command.kind != CommandKind::None {
            current_event.instrument_id = Some(self.active_instrument);
        }
        self.active_pattern_mut()
            .set_event(row_index, track_index, current_event);
    }

    pub fn set_global_volume(&mut self, channel: u8, vol: u8) {
        self.project.global_instrument_volumes[channel as usize] = vol;
        self.engine.set_global_volume(channel, vol);
    }
}
pub struct TrackCache {
    patterns: HashMap<usize, Vec<TrackCacheRow>>,
}

pub struct TrackCacheRow {
    pub tracks: [Line<'static>; constants::TRACK_COUNT as usize],
}
impl<'a> TrackCache {
    pub fn new(patterns: &[Pattern]) -> Self {
        let mut cache = TrackCache {
            patterns: HashMap::new(),
        };
        for (i, pattern) in patterns.iter().enumerate() {
            cache.sync_pattern(PatternId(i as u8), pattern);
        }
        return cache;
    }
    fn build_rows(pattern: &Pattern) -> Vec<TrackCacheRow> {
        return pattern
            .rows
            .iter()
            .map(|row| TrackCacheRow {
                tracks: row
                    .tracks
                    .iter()
                    .map(|c| Line::from(c.to_string()))
                    .collect::<Vec<Line<'static>>>()
                    .try_into()
                    .unwrap(),
            })
            .collect();
    }
    pub fn sync_pattern(&mut self, pattern_index: PatternId, pattern: &Pattern) {
        let pattern_index = pattern_index.0 as usize;
        let stale = self
            .patterns
            .get(&pattern_index)
            .map_or(true, |rows| rows.len() != pattern.rows.len());
        if stale {
            self.patterns
                .insert(pattern_index, Self::build_rows(pattern));
        }
    }
    pub fn update_all_dirty(&mut self, patterns: &[Pattern]) {
        for (i, pattern) in patterns.iter().enumerate() {
            for row_index in 0..pattern.row_count as usize {
                let row = &pattern.rows[row_index];
                if !row.dirty {
                    continue;
                }
                self.patterns.get_mut(&i).unwrap()[row_index].tracks = row
                    .tracks
                    .iter()
                    .map(|c| Line::from(c.to_string()))
                    .collect::<Vec<Line<'static>>>()
                    .try_into()
                    .unwrap();
            }
        }
    }
    pub fn update_dirty(
        &mut self,
        project: &mut Project,
        pattern_index: PatternId,
        row_index: usize,
    ) {
        let pattern = project
            .patterns
            .get_pattern_by_id_mut(pattern_index)
            .unwrap();
        let pattern_index = pattern_index.0 as usize;
        let row = pattern.rows.get_mut(row_index).unwrap();

        if !row.dirty {
            return;
        }
        row.dirty = false;
        self.patterns.get_mut(&pattern_index).unwrap()[row_index].tracks = row
            .tracks
            .iter()
            .map(|c| Line::from(c.to_string()))
            .collect::<Vec<Line<'static>>>()
            .try_into()
            .unwrap();
    }
    pub fn get_row(&'a self, pattern_index: PatternId, row_index: usize) -> &'a TrackCacheRow {
        let pattern_index = pattern_index.0 as usize;
        return &self.patterns.get(&pattern_index).unwrap()[row_index];
    }
    pub fn get_all_rows_for_track(
        &'a self,
        pattern_index: PatternId,
        track_index: usize,
    ) -> Vec<Line<'a>> {
        let pattern_index = pattern_index.0 as usize;
        return self
            .patterns
            .get(&pattern_index)
            .unwrap()
            .iter()
            .map(|r| r.tracks[track_index].clone())
            .collect();
    }
    pub fn get_row_mut(
        &'a mut self,
        pattern_index: usize,
        row_index: usize,
    ) -> &'a mut TrackCacheRow {
        return self
            .patterns
            .get_mut(&pattern_index)
            .unwrap()
            .get_mut(row_index)
            .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_column_maps_into_the_note_string() {
        for column in 0..constants::TRACK_COLUMN_COUNT as usize {
            assert!(column_index_to_note_string_index(column).is_some());
        }
        assert_eq!(
            column_index_to_note_string_index(constants::TRACK_COLUMN_COUNT as usize),
            None
        );
    }

    #[test]
    fn columns_skip_separators() {
        let indices: Vec<usize> = (0..constants::TRACK_COLUMN_COUNT as usize)
            .filter_map(column_index_to_note_string_index)
            .collect();
        assert_eq!(indices, vec![0, 4, 6, 7, 9, 10, 11]);
    }
}
