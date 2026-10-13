use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
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

use super::constants;
use crate::{
    data::{
        config::{Config, TITLE},
        effect::CommandKind,
        midi::{MIDI_CHANNEL_MAX, MidiEvent, VOLUME_MAX},
        note::{Note, NoteKind, NotePitch},
        pattern::{Block, Pattern, PatternId, PatternRow},
        project::{Project, SequenceRow},
    },
    engine::{engine::PlayMode, handle::EngineHandle},
    error::{MTrakError, Result},
    tui::{
        copy_buffers::CopyBuffers,
        framework::{
            core_extensions::Boxed,
            dialog::{Dialog, PopupLevel},
            widget::MtrakWidget,
            widgets::container::Container,
        },
        glyphs,
        theme::Theme,
        undo::{UndoAction, UndoStack},
        views::{
            file_browser::{FileBrowser, Submit},
            keybindings,
            main_view::MainView,
        },
    },
};

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
    pub file_browser: FileBrowser,

    held_keys: HashSet<KeyCode>,
    reports_releases: bool,
}

impl App {
    pub fn new(
        terminal: DefaultTerminal,
        fps: u16,
        theme: Theme,
        project: Project,
    ) -> Result<Self> {
        let state = AppState::new(project, theme)?;
        return Ok(Self {
            terminal,
            fps,
            root: Container::new(MainView::new(&state).boxed(), Padding::ZERO),
            dialogs: Vec::new(),
            file_browser: FileBrowser::new(),
            held_keys: HashSet::new(),
            reports_releases: false,
            state,
        });
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
                None => {
                    self.file_browser.handle_event(&mut self.state, &event)
                        || self.root.handle_event(&mut self.state, &event)
                }
            };
            self.sync_dialogs();
        }
        self.state.update();
        if self.file_browser.is_open() {
            self.file_browser.update(&mut self.state);
        } else {
            self.root.update(&mut self.state);
        }
        for dialog in &mut self.dialogs {
            dialog.update(&mut self.state);
        }
        self.sync_dialogs();
        self.state.send_changes_to_engine();
        self.terminal.draw(|f| {
            let area = f.area();
            let buf = f.buffer_mut();
            buf.set_style(area, self.state.theme.background);
            let file_browser = &mut self.file_browser;
            if file_browser.is_open() {
                file_browser.render(&self.state, area, buf);
            } else {
                self.root.render(&self.state, area, buf);
            }
            for dialog in &mut self.dialogs {
                dialog.render(&self.state, area, buf);
            }
            if glyphs::ascii_mode() {
                glyphs::asciify(f.buffer_mut());
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
        for error in self.state.errors.drain(..) {
            self.dialogs.push(Dialog::popup(
                PopupLevel::Warning,
                error.title(),
                error.to_string(),
            ));
        }
        if std::mem::take(&mut self.state.help_requested) {
            let mut sections = Vec::new();
            self.root.help(&mut sections);
            self.file_browser.help(&mut sections);
            Dialog::help(&mut sections);
            let terminal_height = self.terminal.size().map(|s| s.height).unwrap_or(24);
            let height = (terminal_height * 3 / 5)
                .saturating_sub(4)
                .min(u8::MAX as u16);
            self.dialogs
                .push(keybindings::keybindings_dialog(sections, height as u8));
        }
        if let Some(on_submit) = self.state.save_file_request.take() {
            self.file_browser
                .save_file(on_submit.0, on_submit.1, on_submit.2, on_submit.3);
        }
    }
}
const EVENT_HISTORY_MAX: usize = 256;
const NOTIFICATION_DURATION: Duration = Duration::from_secs(5);

#[derive(Clone, Copy)]
pub struct ChannelHit {
    pub at: Instant,
    pub velocity: u8,
}

enum AppStateAction {
    EditCell {
        pattern: PatternId,
        row: usize,
        track: usize,
        previous_cell: Note,
        new_cell: Note,
    },
    EditBlock {
        pattern: PatternId,
        block: Block,
        previous_cells: Vec<PatternRow>,
        new_cells: Vec<PatternRow>,
    },
    EditPattern {
        pattern: PatternId,
        previous: Pattern,
        new: Pattern,
    },
    EditSequence {
        previous: Vec<SequenceRow>,
        new: Vec<SequenceRow>,
    },
    SetSpeed {
        pattern: PatternId,
        previous: u8,
        new: u8,
    },
    ChangeLength {
        pattern: PatternId,
        previous: u16,
        new: u16,
        removed_rows: Vec<PatternRow>,
    },
    SetBpm {
        previous: u8,
        new: u8,
    },
}
impl AppStateAction {
    fn apply(&self, state: &mut AppState, undo: bool) {
        match self {
            AppStateAction::EditCell {
                pattern,
                row,
                track,
                previous_cell,
                new_cell,
            } => {
                let cell = if undo { previous_cell } else { new_cell };
                state.active_pattern = *pattern;
                state.pattern_mut(*pattern).set_event(*row, *track, *cell);
            }
            AppStateAction::EditBlock {
                pattern,
                block,
                previous_cells,
                new_cells,
            } => {
                let cells = if undo { previous_cells } else { new_cells };
                state.active_pattern = *pattern;
                state.pattern_mut(*pattern).write_block(*block, cells);
            }
            AppStateAction::EditPattern {
                pattern,
                previous,
                new,
            } => {
                let content = if undo { previous } else { new };
                state.active_pattern = *pattern;
                state.pattern_mut(*pattern).copy_content_from(content);
            }
            AppStateAction::EditSequence { previous, new } => {
                let sequence = if undo { previous } else { new };
                state.sequence_changed = true;
                state.project_mut().sequence = sequence.clone();
                state.active_sequence_index = state.active_sequence_index.min(sequence.len() - 1);
            }
            AppStateAction::SetSpeed {
                pattern,
                previous,
                new,
            } => {
                state.active_pattern = *pattern;
                state.pattern_mut(*pattern).ticks_per_line = if undo { *previous } else { *new };
            }
            AppStateAction::ChangeLength {
                pattern,
                previous,
                new,
                removed_rows,
            } => {
                state.active_pattern = *pattern;
                let target = state.pattern_mut(*pattern);
                if !undo {
                    target.set_length(*new);
                    return;
                }
                target.set_length(*previous);
                for (row, removed) in target.rows.iter_mut().skip(*new as usize).zip(removed_rows) {
                    row.tracks = removed.tracks.clone();
                }
            }
            AppStateAction::SetBpm { previous, new } => {
                let bpm = if undo { *previous } else { *new };
                state.project_mut().bpm = bpm;
                state.engine.set_bpm(bpm);
            }
        }
    }
}
impl UndoAction for AppStateAction {
    type State = AppState;

    fn do_action(&mut self, state: &mut Self::State) {
        self.apply(state, false);
    }

    fn undo_action(&mut self, state: &mut Self::State) {
        self.apply(state, true);
    }
}
pub struct AppState {
    pub config: Config,
    project: Project,
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
    /// Where the project was last loaded from or saved to, so saving again skips the browser.
    pub project_path: Option<PathBuf>,
    pub copy_buffers: CopyBuffers,
    pub notification: Option<String>,
    undo_stack: UndoStack<AppStateAction>,
    has_changes: bool,
    notified_at: Instant,
    time: Duration,
    pending_dialogs: Vec<Dialog>,
    help_requested: bool,
    save_file_request: Option<(Box<Submit>, PathBuf, String, Vec<String>)>, //starting directory, title, extension filters
    changed_patterns: HashSet<PatternId>,
    sequence_changed: bool,
    errors: Vec<MTrakError>,
    playback_start_time: Option<Instant>,
}
impl AppState {
    pub fn new(project: Project, theme: Theme) -> Result<Self> {
        let mut errors = Vec::new();
        let midi_client = match MidiOutput::new(TITLE) {
            Ok(client) => Some(client),
            Err(e) => {
                errors.push(e.into());
                None
            }
        };
        let changed_patterns = (0..project.patterns.get_patterns().len() as u8)
            .map(PatternId)
            .collect();
        let mut state = Self {
            config: Config::default(),
            project,
            engine: EngineHandle::spawn()?,
            notification: None,
            notified_at: Instant::now(),
            theme,
            has_changes: false,
            exit_requested: false,
            active_sequence_index: 0,
            active_port: None,
            midi_client,
            time: Duration::ZERO,
            playback_start_time: None,
            active_instrument: 0,
            active_pattern: PatternId(0),
            current_octave: 4,
            recording: false,
            project_path: None,
            line_add: 1,
            pending_dialogs: Vec::new(),
            changed_patterns,
            event_history: VecDeque::new(),
            channel_hits: [None; MIDI_CHANNEL_MAX as usize + 1],
            sequence_changed: true,
            save_file_request: None,
            errors,
            copy_buffers: CopyBuffers::new(),
            undo_stack: UndoStack::new(),
            help_requested: false,
        };
        for (channel, instrument) in state.project.instruments.iter().enumerate() {
            state
                .engine
                .set_global_volume(channel as u8, instrument.volume);
        }
        state.engine.set_bpm(state.project.bpm);
        state
            .engine
            .set_global_velocity(state.project.global_velocity);
        state.engine.set_midi_clock(state.project.send_midi_clock);
        state.restore_midi_output();
        return Ok(state);
    }
    pub fn update(&mut self) {
        let now = Instant::now();
        if let Some(p) = self.playback_start_time
            && self.engine.status().play_mode() != PlayMode::Paused
        {
            let time = p.elapsed();
            self.time = time;
        }
        if self.notified_at.elapsed() >= NOTIFICATION_DURATION {
            self.notification = None;
        }
        let events: Vec<MidiEvent> = self.engine.drain_events().collect();
        for event in events {
            //ignore midi clock messages to avoid UI clutter
            if matches!(event, MidiEvent::Message(Message::TimingClock)) {
                continue;
            }
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
    fn restore_midi_output(&mut self) {
        let Some(name) = self.project.midi_output.clone() else {
            return;
        };
        let port = self.midi_client.as_ref().and_then(|client| {
            client
                .ports()
                .into_iter()
                .find(|p| client.port_name(p).is_ok_and(|n| n == name))
        });
        if port.is_none() {
            self.project.midi_output = None;
        }
        self.set_midi_output(port);
    }
    pub fn set_midi_output(&mut self, port: Option<MidiOutputPort>) {
        let name = self
            .midi_client
            .as_ref()
            .map(|client| port.as_ref().and_then(|p| client.port_name(p).ok()));
        if let Some(name) = name
            && name != self.project.midi_output
        {
            self.project_mut().midi_output = name;
        }
        self.engine.set_output_port(port.clone());
        self.active_port = port;
    }
    pub fn set_midi_clock(&mut self, enabled: bool) {
        self.project_mut().send_midi_clock = enabled;
        self.engine.set_midi_clock(enabled);
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

    pub fn project(&self) -> &Project {
        return &self.project;
    }
    pub fn project_mut(&mut self) -> &mut Project {
        self.has_changes = true;
        return &mut self.project;
    }
    pub fn refresh_track_cache_row(&mut self, cache: &mut TrackCache, row: usize) {
        cache.update_dirty(&mut self.project, self.active_pattern, row);
    }
    pub fn active_pattern(&self) -> &Pattern {
        return self
            .project
            .patterns
            .get_pattern_by_id(self.active_pattern)
            .unwrap();
    }
    pub fn active_pattern_mut(&mut self) -> &mut Pattern {
        return self.pattern_mut(self.active_pattern);
    }
    fn pattern_mut(&mut self, id: PatternId) -> &mut Pattern {
        self.changed_patterns.insert(id);
        return self
            .project_mut()
            .patterns
            .get_pattern_by_id_mut(id)
            .unwrap();
    }
    pub fn copy_buffers_and_active_pattern(&mut self) -> (&mut CopyBuffers, &Pattern) {
        let pattern = self
            .project
            .patterns
            .get_pattern_by_id(self.active_pattern)
            .unwrap();
        return (&mut self.copy_buffers, pattern);
    }
    fn copy_buffers_and_active_pattern_mut(&mut self) -> (&mut CopyBuffers, &mut Pattern) {
        self.changed_patterns.insert(self.active_pattern);
        let pattern = self
            .project
            .patterns
            .get_pattern_by_id_mut(self.active_pattern)
            .unwrap();
        return (&mut self.copy_buffers, pattern);
    }
    fn track_and_do(&mut self, action: AppStateAction) {
        let mut undo_stack = std::mem::take(&mut self.undo_stack);
        undo_stack.track_and_do(self, action);
        self.undo_stack = undo_stack;
    }
    fn track(&mut self, action: AppStateAction) {
        self.has_changes = true;
        self.undo_stack.track(action);
    }
    pub fn undo(&mut self) -> bool {
        let mut undo_stack = std::mem::take(&mut self.undo_stack);
        let undone = undo_stack.undo(self);
        self.undo_stack = undo_stack;
        return undone;
    }
    pub fn redo(&mut self) -> bool {
        let mut undo_stack = std::mem::take(&mut self.undo_stack);
        let redone = undo_stack.redo(self);
        self.undo_stack = undo_stack;
        return redone;
    }
    /// `block` has to cover every cell `edit` changes, or undo won't restore the rest.
    pub fn edit_block<R>(
        &mut self,
        block: Block,
        edit: impl FnOnce(&mut CopyBuffers, &mut Pattern) -> R,
    ) -> R {
        let (buffers, pattern) = self.copy_buffers_and_active_pattern_mut();
        let previous_cells = pattern.read_block(block);
        let result = edit(buffers, pattern);
        let new_cells = pattern.read_block(block);
        if !previous_cells
            .iter()
            .map(|r| &r.tracks)
            .eq(new_cells.iter().map(|r| &r.tracks))
        {
            self.track(AppStateAction::EditBlock {
                pattern: self.active_pattern,
                block,
                previous_cells,
                new_cells,
            });
        }
        return result;
    }
    pub fn edit_pattern<R>(&mut self, edit: impl FnOnce(&mut CopyBuffers, &mut Pattern) -> R) -> R {
        let (buffers, pattern) = self.copy_buffers_and_active_pattern_mut();
        let previous = pattern.clone();
        let result = edit(buffers, pattern);
        if !previous.same_content(pattern) {
            let new = pattern.clone();
            self.track(AppStateAction::EditPattern {
                pattern: self.active_pattern,
                previous,
                new,
            });
        }
        return result;
    }
    pub fn change_speed(&mut self, increase: bool) {
        let (_, pattern) = self.copy_buffers_and_active_pattern_mut();
        let previous = pattern.ticks_per_line;
        if increase {
            pattern.increase_ticks_per_line();
        } else {
            pattern.decrease_ticks_per_line();
        }
        let new = pattern.ticks_per_line;
        if new != previous {
            self.track(AppStateAction::SetSpeed {
                pattern: self.active_pattern,
                previous,
                new,
            });
        }
    }
    pub fn change_length(&mut self, increase: bool) {
        let (_, pattern) = self.copy_buffers_and_active_pattern_mut();
        let previous = pattern.row_count;
        let removed_rows = if increase {
            pattern.increase_length();
            Vec::new()
        } else {
            pattern.decrease_length()
        };
        let new = pattern.row_count;
        if new != previous {
            self.track(AppStateAction::ChangeLength {
                pattern: self.active_pattern,
                previous,
                new,
                removed_rows,
            });
        }
    }
    fn edit_sequence<R>(&mut self, edit: impl FnOnce(&mut Project) -> R) -> R {
        let previous = self.project.sequence.clone();
        let result = edit(&mut self.project);
        if self.project.sequence != previous {
            self.sequence_changed = true;
            let new = self.project.sequence.clone();
            self.track(AppStateAction::EditSequence { previous, new });
        }
        return result;
    }
    pub fn open_dialog(&mut self, dialog: Dialog) {
        self.pending_dialogs.push(dialog);
    }
    pub fn notify(&mut self, notification: impl Into<String>) {
        self.notification = Some(notification.into());
        self.notified_at = Instant::now();
    }
    pub fn time(&self) -> Duration {
        return self.time;
    }
    pub fn request_save_file(
        &mut self,
        on_submit: impl FnOnce(&mut AppState, PathBuf) -> Result<()> + 'static,
        start_directory: PathBuf,
        title: impl Into<String>,
        extensions: Vec<String>,
    ) {
        self.save_file_request = Some((
            Box::new(on_submit),
            start_directory,
            title.into(),
            extensions,
        ));
    }
    pub fn save_project(&mut self, path: PathBuf) -> Result<()> {
        self.project.save(path.clone())?;
        self.has_changes = false;
        self.project_path = Some(path);
        self.notify("SAVED");
        return Ok(());
    }
    /// Hands an error to the app, which shows it in a warning dialog.
    pub fn report_error(&mut self, error: MTrakError) {
        self.errors.push(error);
    }
    pub fn request_help(&mut self) {
        self.help_requested = true;
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
        self.project_mut()
            .patterns
            .create_pattern_id_if_not_exists(id);
        self.changed_patterns.insert(id);
    }

    pub fn change_sequence_pattern(&mut self, index: usize, increase: bool) {
        let id = self.edit_sequence(|project| {
            let row = &mut project.sequence[index];
            if increase {
                row.increate_pattern()
            } else {
                row.decrease_pattern()
            }
        });
        self.ensure_pattern(id);
    }
    pub fn change_sequence_repeats(&mut self, index: usize, increase: bool) {
        self.edit_sequence(|project| {
            let row = &mut project.sequence[index];
            if increase {
                row.increase_repeats();
            } else {
                row.decrease_repeats();
            }
        });
    }

    pub fn insert_sequence(&mut self, index: usize, pattern_id: PatternId) {
        self.edit_sequence(|project| project.insert_sequence(index, pattern_id));
    }
    pub fn delete_sequence(&mut self, index: usize) -> usize {
        return self.edit_sequence(|project| project.delete_sequence(index));
    }

    pub fn play_song(&mut self, sequence_index: usize) {
        self.send_all_to_engine();
        self.engine.play_song(sequence_index, !self.recording);
        self.playback_start_time = Some(Instant::now());
    }

    pub fn play_pattern(&mut self, id: PatternId) {
        self.send_all_to_engine();
        self.engine.play_pattern(id, !self.recording);
        self.playback_start_time = Some(Instant::now());
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
        self.edit_event(row, track, |note| {
            note.kind = NoteKind::On(pitch);
            note.instrument_id = note.instrument_id.or(Some(instrument));
        });
        return Some(pitch);
    }
    pub fn play_key(&self, track: usize, key: event::KeyEvent) -> Option<NotePitch> {
        if !key.modifiers.is_empty() {
            return None;
        }
        let Char(c) = key.code else {
            return None;
        };
        let note = self.key_to_note_pitch(c)?;

        if key.kind != KeyEventKind::Repeat {
            self.engine.play_note(track, self.active_instrument, note);
        }
        return Some(note);
    }

    /// Applies `edit` to the event at `row` in `track` of the active pattern.
    pub fn edit_event(&mut self, row: usize, track: usize, edit: impl FnOnce(&mut Note)) {
        let Some(&previous_cell) = self.active_pattern().get_event(row, track) else {
            return;
        };
        let mut new_cell = previous_cell;
        edit(&mut new_cell);
        if new_cell == previous_cell {
            return;
        }
        self.track_and_do(AppStateAction::EditCell {
            pattern: self.active_pattern,
            row,
            track,
            previous_cell,
            new_cell,
        });
    }

    pub fn enter_instrument(&mut self, track_index: usize, row_index: usize, instrument: u8) {
        if instrument > MIDI_CHANNEL_MAX {
            return;
        };
        self.edit_event(row_index, track_index, |note| {
            note.instrument_id = Some(instrument)
        });
    }

    pub fn enter_volume(&mut self, track_index: usize, row_index: usize, volume: u8) {
        if volume > VOLUME_MAX {
            return;
        };
        self.edit_event(row_index, track_index, |note| note.velocity = Some(volume));
    }

    pub fn enter_command(
        &mut self,
        track_index: usize,
        row_index: usize,
        command: crate::data::effect::Command,
    ) {
        let instrument = self.active_instrument;
        self.edit_event(row_index, track_index, |note| {
            note.command = command;
            if note.command.kind != CommandKind::None {
                note.instrument_id = Some(instrument);
            }
        });
    }

    pub fn increase_bpm(&mut self) {
        self.set_bpm(self.project.bpm.saturating_add(1));
    }

    pub fn decrease_bpm(&mut self) {
        self.set_bpm(self.project.bpm.saturating_sub(1).max(1));
    }

    fn set_bpm(&mut self, bpm: u8) {
        if bpm == self.project.bpm {
            return;
        }
        self.track_and_do(AppStateAction::SetBpm {
            previous: self.project.bpm,
            new: bpm,
        });
    }

    pub fn set_global_velocity(&mut self, velocity: u8) {
        self.project_mut().global_velocity = velocity;
        self.engine.set_global_velocity(velocity);
    }

    pub fn set_global_volume(&mut self, channel: u8, vol: u8) {
        self.project_mut().instruments[channel as usize].volume = vol;
        self.engine.set_global_volume(channel, vol);
    }
    pub fn is_saved(&self) -> bool {
        return !self.has_changes;
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
            .is_none_or(|rows| rows.len() != pattern.rows.len());
        if stale {
            self.patterns
                .insert(pattern_index, Self::build_rows(pattern));
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
