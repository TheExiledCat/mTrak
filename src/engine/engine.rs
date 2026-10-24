use std::{
    collections::VecDeque,
    sync::{
        Arc,
        mpsc::{Receiver, RecvTimeoutError, Sender},
    },
    time::{Duration, Instant},
};

use chrono::Utc;
use midi::{Message, ToRawMessages};
use midir::{MidiOutput, MidiOutputConnection, MidiOutputPort};

use super::{
    handle::{ClockSource, EngineCommand, EngineStatus},
    voices::Voices,
};
use crate::{
    data::{
        config::TITLE,
        effect::{CommandKind, Effect, MidiCommand},
        midi::{
            DEFAULT_BPM, MIDI_CHANNEL_MAX, MidiEvent, PULSES_PER_LINE, PULSES_PER_QUARTER,
            VELOCITY_MAX, pulse_duration, raw_message_to_bytes, tick_duration,
        },
        note::{Note, NoteKind},
        pattern::{Pattern, PatternId},
        project::SequenceRow,
    },
    engine::command_handler::{
        CommandContext, EffectCommandHandler, MidiCommandHandler, NoteAction, SlideTarget,
        roll_velocity,
    },
    tui::constants::TRACK_COUNT,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum PlayMode {
    Paused,
    Song,
    SongLoop,
    PatternLoop,
    Pattern,
}
impl PlayMode {
    /// Converts the stored atomic value back into a mode.
    pub fn from_u8(value: u8) -> Self {
        return match value {
            1 => PlayMode::Song,
            2 => PlayMode::SongLoop,
            3 => PlayMode::PatternLoop,
            4 => PlayMode::Pattern,
            _ => PlayMode::Paused,
        };
    }
}

#[derive(Clone, Copy)]
struct Position {
    sequence_index: usize,
    repeat: u8,
    pattern: PatternId,
    row: usize,
}

struct Clock {
    /// Whether pulses come from the clock input rather than the engine's own timer.
    follows_input: bool,
    next_pulse: Instant,
    pulse_in_row: u32,
    row_start: Instant,
    tick: u32,
}

/// What the clock input asked for, waiting on its next pulse.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PendingTransport {
    Start,
    Continue,
}

/// Something a track does later in the current row.
#[derive(Clone, Copy)]
struct ScheduledNote {
    tick: u32,
    kind: Scheduled,
}

#[derive(Clone, Copy)]
enum Scheduled {
    /// Plays the note once, from a delay.
    Once(Note),
    /// Replays the note every `every` ticks until the row ends, changing its velocity by
    /// `volume` each time.
    Roll { note: Note, every: u32, volume: u8 },
    /// Releases the track's note.
    Cut,
}

/// A note from the next row that starts before its row, gliding from the track's current
/// note when `legato`. It keeps the mutes of the pattern it belongs to.
#[derive(Clone, Copy)]
struct EarlyNote {
    tick: u32,
    note: Note,
    legato: bool,
    mutes: [bool; 16],
}

pub struct MtrakEngine {
    commands: Receiver<EngineCommand>,
    events: Sender<MidiEvent>,
    status: Arc<EngineStatus>,

    patterns: Vec<Pattern>,
    sequence: Vec<SequenceRow>,
    bpm: u8,
    send_midi_clock: bool,
    output_connection: Option<MidiOutputConnection>,
    pending_events: Vec<MidiEvent>,
    mode: PlayMode,
    position: Position,
    clock: Option<Clock>,
    clock_source: ClockSource,
    /// Where the clock input's Continue picks up in the song, kept when playback stops.
    resume: Option<Position>,
    pending_transport: Option<PendingTransport>,
    /// Timestamps of the clock input's last beat of pulses, oldest first.
    external_pulses: VecDeque<u64>,
    /// The average time between pulses from the clock input over the last beat.
    external_period: Duration,
    voices: Voices,
    scheduled_notes: [Option<ScheduledNote>; TRACK_COUNT as usize],
    early_notes: [Option<EarlyNote>; TRACK_COUNT as usize],
    /// The next row's notes that were already started early, per track.
    started_early: [Option<Note>; TRACK_COUNT as usize],

    midi_command_handler: MidiCommandHandler,
    effect_command_handler: EffectCommandHandler,
}

impl MtrakEngine {
    pub fn new(
        commands: Receiver<EngineCommand>,
        events: Sender<MidiEvent>,
        status: Arc<EngineStatus>,
    ) -> Self {
        return Self {
            commands,
            events,
            status,
            patterns: Vec::new(),
            sequence: Vec::new(),
            bpm: DEFAULT_BPM,
            send_midi_clock: false,
            output_connection: None,
            mode: PlayMode::Paused,
            position: Position {
                sequence_index: 0,
                repeat: 0,
                pattern: PatternId(0),
                row: 0,
            },
            pending_events: vec![],
            clock: None,
            clock_source: ClockSource::Internal,
            resume: None,
            pending_transport: None,
            external_pulses: VecDeque::with_capacity(PULSES_PER_QUARTER as usize + 1),
            external_period: pulse_duration(DEFAULT_BPM),
            voices: Voices::new(),
            scheduled_notes: [None; TRACK_COUNT as usize],
            early_notes: [None; TRACK_COUNT as usize],
            started_early: [None; TRACK_COUNT as usize],

            midi_command_handler: MidiCommandHandler::new(),
            effect_command_handler: EffectCommandHandler::new(),
        };
    }

    /// Main loop: waits for commands until the next clock deadline, then advances the clock.
    pub fn run(mut self) {
        loop {
            let received = match self.next_deadline() {
                Some(deadline) => self
                    .commands
                    .recv_timeout(deadline.saturating_duration_since(Instant::now())),
                None => self
                    .commands
                    .recv()
                    .map_err(|_| RecvTimeoutError::Disconnected),
            };
            match received {
                Ok(command) => self.apply(command),
                Err(RecvTimeoutError::Timeout) => self.advance_clock(),
                Err(RecvTimeoutError::Disconnected) => break,
            }
            self.dispatch_pending();
        }
    }

    /// Handles one command sent from the UI.
    fn apply(&mut self, command: EngineCommand) {
        match command {
            EngineCommand::SetOutputPort(port) => self.connect_out(port),
            EngineCommand::PlayPattern(id, looping) => self.play_pattern(id, looping),
            EngineCommand::PlaySong(sequence_index, looping) => {
                self.play_song(sequence_index, looping)
            }
            EngineCommand::Stop => self.stop(),
            EngineCommand::SetPattern(id, pattern) => self.set_pattern(id, pattern),
            EngineCommand::SetSequence(sequence) => self.sequence = sequence,
            EngineCommand::SetBpm(bpm) => self.bpm = bpm,
            EngineCommand::Event(message) => self.pending_events.push(message),
            EngineCommand::PlayNote {
                track,
                channel,
                pitch,
                velocity,
            } => {
                let mut out = vec![];
                self.voices
                    .note_on(track, channel, pitch, velocity, &mut out);
                self.dispatch_messages(out);
            }
            EngineCommand::ReleaseNote(track) => {
                let mut out = vec![];
                self.voices.release(track, &mut out);
                self.dispatch_messages(out);
            }
            EngineCommand::ReleasePitch { track, pitch } => {
                let mut out = vec![];
                self.voices.release_pitch(track, pitch, &mut out);
                self.dispatch_messages(out);
            }
            EngineCommand::SetGlobalVolume { channel, volume } => {
                self.voices.set_global_volume(channel as usize, volume)
            }
            EngineCommand::SetGlobalVelocity(velocity) => self.voices.set_global_velocity(velocity),
            EngineCommand::SetMidiClock(enabled) => self.send_midi_clock = enabled,
            EngineCommand::SetClockSource(source) => self.set_clock_source(source),
            EngineCommand::ExternalClock(timestamp) => self.external_pulse(timestamp),
            EngineCommand::ExternalStart => self.pending_transport = Some(PendingTransport::Start),
            EngineCommand::ExternalContinue => {
                self.pending_transport = Some(PendingTransport::Continue)
            }
            EngineCommand::ExternalSongPosition(sixteenths) => self.song_position(sixteenths),
            EngineCommand::ExternalStop => {
                self.pending_transport = None;
                if self.clock.as_ref().is_some_and(|clock| clock.follows_input) {
                    self.stop();
                }
            }
        }
    }

    /// Stores the engine's copy of a pattern and cuts newly muted notes if it is playing.
    fn set_pattern(&mut self, id: PatternId, pattern: Pattern) {
        if id.0 as usize >= self.patterns.len() {
            self.patterns
                .resize_with(id.0 as usize + 1, Pattern::default);
        }
        self.patterns[id.0 as usize] = pattern;
        if self.clock.is_some() && id == self.position.pattern {
            self.release_muted();
        }
    }

    /// The pattern at the current play position.
    fn current_pattern(&self) -> Option<&Pattern> {
        return self.patterns.get(self.position.pattern.0 as usize);
    }

    /// The song's BPM, unless a tempo command has changed it.
    fn bpm(&self) -> u8 {
        return self.effect_command_handler.bpm().unwrap_or(self.bpm);
    }

    /// The pattern's ticks per line, unless a tempo command has changed the speed.
    fn ticks_per_line(&self, pattern: &Pattern) -> u8 {
        return self
            .effect_command_handler
            .speed()
            .unwrap_or(pattern.ticks_per_line)
            .max(1);
    }

    /// The time between pulses: from the BPM, or measured from the clock input when synced,
    /// which also sets the tempo of playback started locally.
    fn pulse_duration(&self) -> Duration {
        return match self.clock_source {
            ClockSource::Internal => pulse_duration(self.bpm()),
            ClockSource::External => self.external_period,
        };
    }

    fn set_clock_source(&mut self, source: ClockSource) {
        if source == self.clock_source {
            return;
        }
        if self.clock.is_some() {
            self.stop();
        }
        self.clock_source = source;
        self.external_period = pulse_duration(self.bpm);
        self.publish_synced_bpm();
        self.resume = None;
        self.pending_transport = None;
        self.external_pulses.clear();
    }

    /// Plays from `position` on the engine's own timer.
    fn play_from(&mut self, mode: PlayMode, position: Position) {
        self.position = position;
        self.start(mode, Message::Start, false);
    }

    /// Shows the measured tempo, holding the shown value until it is clearly off so jitter
    /// around a half BPM doesn't flicker between two values.
    fn publish_synced_bpm(&self) {
        let bpm = 60.0 / (self.external_period.as_secs_f64() * PULSES_PER_QUARTER as f64);
        if (bpm - self.status.synced_bpm() as f64).abs() < 0.75 {
            return;
        }
        self.status
            .set_synced_bpm(bpm.round().clamp(1.0, u8::MAX as f64) as u8);
    }

    /// Averages the time between pulses over the last beat. A gap long enough to be a stopped
    /// clock starts the measurement over, and so does a timestamp going backwards, since each
    /// new input connection counts from zero again.
    fn measure_external_pulse(&mut self, timestamp: u64) {
        if self
            .external_pulses
            .back()
            .is_some_and(|&last| timestamp < last || timestamp - last >= 250_000)
        {
            self.external_pulses.clear();
        }
        if self.external_pulses.len() > PULSES_PER_QUARTER as usize {
            self.external_pulses.pop_front();
        }
        self.external_pulses.push_back(timestamp);
        let (Some(&first), Some(&last)) =
            (self.external_pulses.front(), self.external_pulses.back())
        else {
            return;
        };
        let intervals = self.external_pulses.len() as u32 - 1;
        if intervals == 0 {
            return;
        }
        self.external_period = Duration::from_micros(last - first) / intervals;
        self.publish_synced_bpm();
    }

    /// One pulse from the clock input. A pending Start or Continue begins on it, otherwise it
    /// moves playback along like an internal pulse.
    fn external_pulse(&mut self, timestamp: u64) {
        self.measure_external_pulse(timestamp);
        if self.clock_source != ClockSource::External {
            return;
        }
        match self.pending_transport.take() {
            Some(PendingTransport::Start) => return self.start_cued(),
            Some(PendingTransport::Continue) => return self.continue_cued(),
            None => {}
        }
        if !self.clock.as_ref().is_some_and(|clock| clock.follows_input) {
            return;
        }
        self.advance_clock();
        if self
            .clock
            .as_ref()
            .is_some_and(|clock| clock.pulse_in_row + 1 >= PULSES_PER_LINE)
        {
            self.finish_row_ticks();
        }
        if let Some(clock) = &mut self.clock {
            clock.next_pulse = Instant::now();
        }
        self.pulse();
    }

    /// The first row of the song.
    fn song_start(&self) -> Position {
        return Position {
            sequence_index: 0,
            repeat: 0,
            pattern: self
                .sequence
                .first()
                .map_or(PatternId(0), |row| row.pattern_id),
            row: 0,
        };
    }

    /// Following the clock input always plays the song on loop, since the master decides
    /// when it ends.
    fn start_cued(&mut self) {
        self.position = self.song_start();
        self.start(PlayMode::SongLoop, Message::Start, true);
    }

    /// Picks up where the clock input stopped, or where its song position pointed.
    fn continue_cued(&mut self) {
        let Some(position) = self.resume.take() else {
            return self.start_cued();
        };
        self.position = position;
        self.start(PlayMode::SongLoop, Message::Continue, true);
    }

    /// Moves where the clock input's Continue picks up to `sixteenths` sixteenth notes into
    /// the song.
    fn song_position(&mut self, sixteenths: u16) {
        let rows =
            sixteenths as usize * (PULSES_PER_QUARTER / 4) as usize / PULSES_PER_LINE as usize;
        self.resume = self.song_position_at(rows);
    }

    /// The position `rows` rows from the start of the song, wrapping around at its end.
    fn song_position_at(&self, rows: usize) -> Option<Position> {
        let row_count = |id: PatternId| {
            self.patterns
                .get(id.0 as usize)
                .map_or(0, |p| p.row_count as usize)
        };
        let total: usize = self
            .sequence
            .iter()
            .map(|row| row_count(row.pattern_id) * row.repeats as usize)
            .sum();
        if total == 0 {
            return None;
        }
        let mut remaining = rows % total;
        for (sequence_index, sequence_row) in self.sequence.iter().enumerate() {
            let count = row_count(sequence_row.pattern_id);
            for repeat in 0..sequence_row.repeats {
                if remaining < count {
                    return Some(Position {
                        sequence_index,
                        repeat,
                        pattern: sequence_row.pattern_id,
                        row: remaining,
                    });
                }
                remaining -= count;
            }
        }
        return None;
    }

    /// Runs the effect ticks left in the row, so a clock input that speeds up never skips any.
    fn finish_row_ticks(&mut self) {
        loop {
            let Some(pattern) = self.current_pattern() else {
                return;
            };
            let ticks_per_line = self.ticks_per_line(pattern) as u32;
            let Some(clock) = &mut self.clock else {
                return;
            };
            if clock.tick + 1 >= ticks_per_line {
                return;
            }
            clock.tick += 1;
            self.effect_tick();
        }
    }

    /// Plays a single pattern from row 0, once or on repeat.
    fn play_pattern(&mut self, id: PatternId, looping: bool) {
        let position = Position {
            sequence_index: 0,
            repeat: 0,
            pattern: id,
            row: 0,
        };
        let mode = if looping {
            PlayMode::PatternLoop
        } else {
            PlayMode::Pattern
        };
        self.play_from(mode, position);
    }

    /// Plays the sequence starting at the given index.
    fn play_song(&mut self, sequence_index: usize, looping: bool) {
        let Some(sequence_row) = self.sequence.get(sequence_index) else {
            return;
        };
        let position = Position {
            sequence_index,
            repeat: 0,
            pattern: sequence_row.pattern_id,
            row: 0,
        };
        let mode = if looping {
            PlayMode::SongLoop
        } else {
            PlayMode::Song
        };
        self.play_from(mode, position);
    }

    /// Resets voices and the clock, sends `transport`, then plays the row at the position.
    fn start(&mut self, mode: PlayMode, transport: Message, follows_input: bool) {
        if self.current_pattern().is_none() {
            self.stop();
            return;
        }
        let mut out = vec![];
        self.voices.release_all(&mut out);
        out.extend(self.reset_commands());
        self.dispatch_messages(out);
        let now = Instant::now();
        self.clock = Some(Clock {
            follows_input,
            next_pulse: now,
            pulse_in_row: 0,
            row_start: now,
            tick: 0,
        });
        self.mode = mode;
        self.status.set_play_mode(mode);
        self.publish_position();
        self.dispatch(MidiEvent::Message(transport));
        self.send_clock();
        self.play_row();
        self.time_next_pulse(now);
    }

    /// Schedules the first pulse after a row starts, so a tempo command on the row applies
    /// straight away.
    fn time_next_pulse(&mut self, row_start: Instant) {
        let pulse_duration = self.pulse_duration();
        if let Some(clock) = &mut self.clock {
            clock.next_pulse = row_start + pulse_duration;
        }
    }

    /// Stops playback and releases every sounding note.
    fn stop(&mut self) {
        if self.clock.is_some() {
            self.resume = match self.mode {
                PlayMode::Song | PlayMode::SongLoop => self.next_position(&self.position),
                _ => None,
            };
        }
        self.clock = None;
        self.mode = PlayMode::Paused;
        self.status.set_play_mode(PlayMode::Paused);
        let mut out = vec![Message::Stop];
        self.voices.release_all(&mut out);
        out.extend(self.reset_commands());
        let mut events: Vec<MidiEvent> = out.into_iter().map(MidiEvent::Message).collect();
        events.push(MidiEvent::NotesOffAll);
        self.dispatch(MidiEvent::Aggregate(events));
    }

    /// When the engine next needs to wake up, either for a pulse or an effect tick.
    fn next_deadline(&self) -> Option<Instant> {
        let clock = self.clock.as_ref()?;
        if clock.follows_input {
            return self.next_effect_tick();
        }
        let next_pulse = clock.next_pulse;
        return Some(match self.next_effect_tick() {
            Some(tick) => tick.min(next_pulse),
            None => next_pulse,
        });
    }

    /// When the next effect tick within the current row is due, if any are left.
    fn next_effect_tick(&self) -> Option<Instant> {
        let clock = self.clock.as_ref()?;
        let pattern = self.current_pattern()?;
        let ticks_per_line = self.ticks_per_line(pattern);
        let next = clock.tick + 1;
        if next >= ticks_per_line as u32 {
            return None;
        }
        return Some(clock.row_start + tick_duration(self.pulse_duration(), ticks_per_line) * next);
    }

    /// Catches up on every pulse and effect tick that is due by now.
    fn advance_clock(&mut self) {
        let now = Instant::now();
        loop {
            let Some(clock) = &self.clock else {
                return;
            };
            let next_pulse = clock.next_pulse;
            let follows_input = clock.follows_input;
            match self.next_effect_tick() {
                Some(tick) if tick <= next_pulse && tick <= now => {
                    if let Some(clock) = &mut self.clock {
                        clock.tick += 1;
                    }
                    self.effect_tick();
                }
                _ if next_pulse <= now && !follows_input => self.pulse(),
                _ => return,
            }
        }
    }

    /// One MIDI clock pulse; moves to the next row every PULSES_PER_LINE pulses.
    fn pulse(&mut self) {
        if self.current_pattern().is_none() {
            self.stop();
            return;
        }
        let pulse_duration = self.pulse_duration();
        let Some(clock) = &mut self.clock else {
            return;
        };
        let pulse_time = clock.next_pulse;
        clock.next_pulse += pulse_duration;
        clock.pulse_in_row += 1;
        let row_done = clock.pulse_in_row >= PULSES_PER_LINE;
        self.send_clock();
        if !row_done {
            return;
        }
        let Some(clock) = &mut self.clock else {
            return;
        };
        clock.pulse_in_row = 0;
        clock.row_start = pulse_time;
        clock.tick = 0;

        self.advance_row();
        if self.clock.is_some() {
            self.play_row();
            self.time_next_pulse(pulse_time);
        }
    }

    fn send_clock(&mut self) {
        if self.send_midi_clock {
            self.dispatch(MidiEvent::Message(Message::TimingClock));
        }
    }

    /// Moves the position forward a row, stopping when the song ends.
    fn advance_row(&mut self) {
        match self.next_position(&self.position) {
            Some(position) => {
                self.position = position;
                self.publish_position();
            }
            None => self.stop(),
        }
    }

    /// The row played after `position`, handling pattern repeats and the sequence;
    /// `None` when the song ends.
    fn next_position(&self, position: &Position) -> Option<Position> {
        let mut next = *position;
        next.row += 1;
        let row_count = self
            .patterns
            .get(next.pattern.0 as usize)
            .map_or(0, |p| p.row_count as usize);
        if next.row < row_count {
            return Some(next);
        }
        next.row = 0;
        match self.mode {
            PlayMode::PatternLoop => return Some(next),
            PlayMode::Pattern => return None,
            _ => {}
        }

        next.repeat += 1;
        let repeats = self
            .sequence
            .get(next.sequence_index)
            .map_or(0, |row| row.repeats);
        if next.repeat < repeats {
            return Some(next);
        }
        next.repeat = 0;
        next.sequence_index += 1;
        if next.sequence_index >= self.sequence.len() {
            if self.mode == PlayMode::Song {
                return None;
            }
            next.sequence_index = 0;
        }
        next.pattern = self.sequence.get(next.sequence_index)?.pattern_id;
        return Some(next);
    }

    /// How many rows one pass through the current play mode covers, to bound lookahead.
    fn rows_per_pass(&self) -> usize {
        let rows = |id: PatternId| {
            self.patterns
                .get(id.0 as usize)
                .map_or(0, |p| p.row_count as usize)
        };
        if matches!(self.mode, PlayMode::PatternLoop | PlayMode::Pattern) {
            return rows(self.position.pattern);
        }
        return self
            .sequence
            .iter()
            .map(|row| rows(row.pattern_id) * row.repeats as usize)
            .sum();
    }

    /// Finds the next CC or slide command on the track after the current row, and how many
    /// ticks away it is; for slides that send a CC, a SetCC first means there is nothing to reach.
    fn find_slide_target(&self, track: usize, stop_at_set_cc: bool) -> Option<SlideTarget> {
        let mut position = self.position;
        let mut ticks = 0;
        for _ in 0..self.rows_per_pass() {
            let pattern = self.patterns.get(position.pattern.0 as usize)?;
            ticks += self.ticks_per_line(pattern) as u32;
            position = self.next_position(&position)?;
            let note = self
                .patterns
                .get(position.pattern.0 as usize)?
                .get_event(position.row, track)?;
            match note.command.kind {
                CommandKind::MidiCommand(command) if command.is_slide_target() => {
                    return Some(SlideTarget {
                        value: note.command.value,
                        ticks,
                    });
                }
                CommandKind::MidiCommand(MidiCommand::SetCC) if stop_at_set_cc => return None,
                _ => {}
            }
        }
        return None;
    }

    /// Looks ahead in the song for commands that need it.
    fn command_context(&self, track: usize, command: MidiCommand) -> CommandContext {
        if !command.is_slide() {
            return CommandContext::default();
        }
        return CommandContext {
            slide_target: self.find_slide_target(track, command.slides_cc()),
        };
    }

    /// Clears effect state so nothing carries over between playbacks, and returns the
    /// messages that undo what commands left set on the synth. These go out even on muted
    /// channels, like the all sound off on stop, so a synth is never left bent.
    fn reset_commands(&mut self) -> Vec<Message> {
        self.scheduled_notes = [None; TRACK_COUNT as usize];
        self.early_notes = [None; TRACK_COUNT as usize];
        self.started_early = [None; TRACK_COUNT as usize];
        let mut events = vec![];
        self.midi_command_handler.reset(&mut events);
        self.effect_command_handler.reset();
        return events
            .into_iter()
            .flat_map(MidiEvent::to_messages)
            .collect();
    }

    /// Advances running effects by one tick and returns their messages.
    fn tick_commands(&mut self) -> Vec<MidiEvent> {
        let mut events = vec![];
        self.midi_command_handler.tick(&mut events);
        return events;
    }

    /// The channel mutes of the pattern being played.
    fn current_mutes(&self) -> [bool; 16] {
        return self
            .current_pattern()
            .map_or([false; 16], |pattern| pattern.channel_mutes);
    }

    /// Shares the play position with the UI.
    fn publish_position(&self) {
        self.status.set_position(
            self.position.sequence_index,
            self.position.pattern.0 as usize,
            self.position.row,
        );
    }

    /// Runs each track's command for the row at the current position, then sends its note.
    /// Commands come first so a CC lands before the note starts and a velocity slide can set
    /// the note's velocity. They run on muted channels too so their effects stay in step;
    /// only their output is dropped.
    fn play_row(&mut self) {
        let row_tick = self.tick_commands();
        let mutes = self.current_mutes();
        let Some(pattern) = self.patterns.get(self.position.pattern.0 as usize) else {
            return;
        };
        let Some(row) = pattern.rows.get(self.position.row) else {
            return;
        };
        let tracks = row.tracks.clone();
        let mut out = vec![];
        self.voices.release_channels(&mutes, &mut out);
        self.voices.tick(&mut out);
        out.extend(audible(row_tick, &mutes));
        // notes still waiting were scheduled past the end of the row, so they never play
        self.scheduled_notes = [None; TRACK_COUNT as usize];
        self.early_notes = [None; TRACK_COUNT as usize];
        let started_early = std::mem::take(&mut self.started_early);
        for (track, note) in tracks.iter().enumerate() {
            let action = match note.command.kind {
                CommandKind::None => NoteAction::Play,
                CommandKind::MidiCommand(command) => {
                    let context = self.command_context(track, command);
                    let mut events = vec![];
                    self.midi_command_handler.handle_command(
                        &mut events,
                        track,
                        command,
                        note,
                        &context,
                    );
                    out.extend(audible(events, &mutes));
                    NoteAction::Play
                }
                CommandKind::Effect(command) => self
                    .effect_command_handler
                    .apply(command.decode(note.command.value)),
            };
            match action {
                // the row before already started this note
                _ if started_early[track] == Some(*note) => {}
                NoteAction::Play => self.play_note(track, note, None, &mutes, &mut out),
                NoteAction::Legato(overlap) => {
                    self.play_note(track, note, Some(overlap), &mutes, &mut out)
                }
                NoteAction::Skip => {}
                NoteAction::Cut(0) => self.voices.cut(track, &mut out),
                NoteAction::Cut(tick) => {
                    self.play_note(track, note, None, &mutes, &mut out);
                    self.scheduled_notes[track] = Some(ScheduledNote {
                        tick: tick as u32,
                        kind: Scheduled::Cut,
                    });
                }
                NoteAction::Delay(tick) => {
                    self.scheduled_notes[track] = Some(ScheduledNote {
                        tick: tick as u32,
                        kind: Scheduled::Once(*note),
                    });
                }
                NoteAction::Roll { every, volume } => {
                    self.play_note(track, note, None, &mutes, &mut out);
                    self.scheduled_notes[track] = Some(ScheduledNote {
                        tick: every as u32,
                        kind: Scheduled::Roll {
                            note: *note,
                            every: every as u32,
                            volume,
                        },
                    });
                }
            }
        }
        self.schedule_early_notes();
        self.dispatch_messages(out);
    }

    /// Sends a cell's note, or releases the track's note for a note off. With `legato`, the
    /// note glides from the track's current one, which keeps sounding that many more ticks.
    fn play_note(
        &mut self,
        track: usize,
        note: &Note,
        legato: Option<u8>,
        mutes: &[bool; 16],
        out: &mut Vec<Message>,
    ) {
        match note.kind {
            NoteKind::Empty => {}
            NoteKind::Off => self.voices.release(track, out),
            NoteKind::On(pitch) => {
                let channel = note.instrument_id.unwrap_or(0).min(MIDI_CHANNEL_MAX);
                match velocity(note.velocity, self.midi_command_handler.velocity(track)) {
                    Some(velocity) if !mutes[channel as usize] => match legato {
                        Some(overlap) => self
                            .voices
                            .legato_on(track, channel, pitch.0, velocity, overlap, out),
                        None => self.voices.note_on(track, channel, pitch.0, velocity, out),
                    },
                    _ => self.voices.release(track, out),
                }
            }
        }
    }

    /// The velocity the cell's note plays at right now, 0 when it's silent.
    fn note_velocity(&self, track: usize, note: &Note) -> u8 {
        return velocity(note.velocity, self.midi_command_handler.velocity(track)).unwrap_or(0);
    }

    /// Looks at the next row for notes that start early, and schedules them into the end of
    /// this row.
    fn schedule_early_notes(&mut self) {
        let Some(ticks) = self
            .current_pattern()
            .map(|p| self.ticks_per_line(p) as u32)
        else {
            return;
        };
        let Some(next) = self.next_position(&self.position) else {
            return;
        };
        let Some(pattern) = self.patterns.get(next.pattern.0 as usize) else {
            return;
        };
        let Some(row) = pattern.rows.get(next.row) else {
            return;
        };
        for (track, note) in row.tracks.iter().enumerate() {
            let CommandKind::Effect(command) = note.command.kind else {
                continue;
            };
            let (early, legato) = match command.decode(note.command.value) {
                Effect::LegatoEarly(early) => (early, true),
                Effect::DelayEarly(early) => (early, false),
                _ => continue,
            };
            // tick 0 is this row's own note, so the earliest start is tick 1
            let tick = ticks.saturating_sub(early as u32).max(1);
            if early == 0 || tick >= ticks {
                continue;
            }
            self.early_notes[track] = Some(EarlyNote {
                tick,
                note: *note,
                legato,
                mutes: pattern.channel_mutes,
            });
        }
    }

    /// Plays the scheduled and early notes that are due on the current tick.
    fn play_scheduled_notes(&mut self, mutes: &[bool; 16], out: &mut Vec<Message>) {
        let Some(tick) = self.clock.as_ref().map(|clock| clock.tick) else {
            return;
        };
        for track in 0..self.scheduled_notes.len() {
            let Some(early) = self.early_notes[track].filter(|early| early.tick == tick) else {
                self.play_scheduled_note(track, tick, mutes, out);
                continue;
            };
            // the next row's note takes over the track, so the rest of this row's plans go
            self.early_notes[track] = None;
            self.scheduled_notes[track] = None;
            self.started_early[track] = Some(early.note);
            let legato = early.legato.then_some(0);
            self.play_note(track, &early.note, legato, &early.mutes, out);
        }
    }

    /// Runs the track's delayed note, roll hit or cut if it is due on `tick`.
    fn play_scheduled_note(
        &mut self,
        track: usize,
        tick: u32,
        mutes: &[bool; 16],
        out: &mut Vec<Message>,
    ) {
        let Some(scheduled) = self.scheduled_notes[track] else {
            return;
        };
        if scheduled.tick != tick {
            return;
        }
        self.scheduled_notes[track] = None;
        match scheduled.kind {
            Scheduled::Once(note) => self.play_note(track, &note, None, mutes, out),
            Scheduled::Roll {
                mut note,
                every,
                volume,
            } => {
                // 0 and 8 leave the velocity alone, so a velocity slide still sets it per hit
                if !matches!(volume, 0x0 | 0x8) {
                    let previous = self.note_velocity(track, &note);
                    note.velocity = Some(roll_velocity(previous, volume));
                }
                self.scheduled_notes[track] = Some(ScheduledNote {
                    tick: tick + every,
                    kind: Scheduled::Roll {
                        note,
                        every,
                        volume,
                    },
                });
                self.play_note(track, &note, None, mutes, out);
            }
            Scheduled::Cut => self.voices.cut(track, out),
        }
    }

    /// Cuts notes on channels muted in the current pattern.
    fn release_muted(&mut self) {
        let Some(pattern) = self.patterns.get(self.position.pattern.0 as usize) else {
            return;
        };
        let mut out = vec![];
        self.voices
            .release_channels(&pattern.channel_mutes, &mut out);
        self.dispatch_messages(out);
    }

    /// Runs effects for the ticks between rows.
    fn effect_tick(&mut self) {
        let events = self.tick_commands();
        let mutes = self.current_mutes();
        let mut out = audible(events, &mutes);
        self.voices.tick(&mut out);
        self.play_scheduled_notes(&mutes, &mut out);
        self.dispatch_messages(out);
    }

    /// Opens the MIDI output port, closing the previous one.
    fn connect_out(&mut self, port: Option<MidiOutputPort>) {
        if let Some(connection) = self.output_connection.take() {
            connection.close();
        }
        self.output_connection = port.and_then(|port| {
            MidiOutput::new(TITLE)
                .ok()?
                .connect(&port, &Utc::now().to_string())
                .ok()
        });
        self.status
            .set_output_connected(self.output_connection.is_some());
    }
    /// Sends an event to the MIDI output and reports it to the UI.
    fn dispatch(&mut self, event: MidiEvent) {
        let _ = self.events.send(event.clone());
        let Some(conn) = &mut self.output_connection else {
            return;
        };
        for message in event.to_messages() {
            let bytes: Vec<u8> = message
                .to_raw_messages()
                .into_iter()
                .flat_map(raw_message_to_bytes)
                .collect();
            let _ = conn.send(&bytes);
        }
    }
    /// Dispatches a batch of messages as one event.
    fn dispatch_messages(&mut self, messages: Vec<Message>) {
        if messages.is_empty() {
            return;
        }
        self.dispatch(MidiEvent::Aggregate(
            messages.into_iter().map(MidiEvent::Message).collect(),
        ));
    }
    /// Sends the events queued by the UI.
    fn dispatch_pending(&mut self) {
        let pending: Vec<MidiEvent> = std::mem::take(&mut self.pending_events);
        for event in pending {
            self.dispatch(event);
        }
    }
}

/// Drops channel messages for muted channels.
fn audible(events: Vec<MidiEvent>, mutes: &[bool; 16]) -> Vec<Message> {
    return events
        .into_iter()
        .flat_map(MidiEvent::to_messages)
        .filter(|message| !message_channel(message).is_some_and(|c| mutes[c as usize]))
        .collect();
}

/// The channel a channel voice message is sent on; `None` for system messages.
fn message_channel(message: &Message) -> Option<u8> {
    let status = *message
        .to_raw_messages()
        .into_iter()
        .flat_map(raw_message_to_bytes)
        .collect::<Vec<u8>>()
        .first()?;
    return (0x80..=0xEF).contains(&status).then_some(status & 0x0F);
}

/// A note's velocity: the volume column (00-80) wins, then a velocity slide's value, then full
/// velocity. `None` means the note is silent.
pub fn velocity(volume: Option<u8>, glide: Option<u8>) -> Option<u8> {
    let velocity = match volume {
        Some(volume) => volume.min(VELOCITY_MAX),
        None => glide.unwrap_or(VELOCITY_MAX),
    };
    return (velocity > 0).then_some(velocity);
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use midi::Channel;

    use super::*;
    use crate::data::{
        effect::{Command, EffectCommand, MidiCommand},
        note::NotePitch,
    };

    /// Drives an engine without a MIDI port and collects every message it would send.
    struct Harness {
        engine: MtrakEngine,
        events: Receiver<MidiEvent>,
        _commands: Sender<EngineCommand>,
    }

    impl Harness {
        fn new(patterns: Vec<Pattern>) -> Self {
            let (commands, receiver) = mpsc::channel();
            let (event_sender, events) = mpsc::channel();
            let mut engine =
                MtrakEngine::new(receiver, event_sender, Arc::new(EngineStatus::default()));
            for (id, pattern) in patterns.into_iter().enumerate() {
                engine.apply(EngineCommand::SetPattern(PatternId(id as u8), pattern));
            }
            return Self {
                engine,
                events,
                _commands: commands,
            };
        }

        /// Everything sent since the last call, flattened into plain messages.
        fn messages(&self) -> Vec<Message> {
            return self
                .events
                .try_iter()
                .flat_map(MidiEvent::to_messages)
                .collect();
        }

        /// Moves to the next tick in the row and runs it, like the clock does.
        fn effect_tick(&mut self) {
            self.engine.clock.as_mut().unwrap().tick += 1;
            self.engine.effect_tick();
        }

        /// Runs the rest of the row's effect ticks, then pulses the clock until the next row has played.
        fn next_row(&mut self) {
            let ticks = self
                .engine
                .ticks_per_line(self.engine.current_pattern().unwrap());
            for _ in 1..ticks {
                self.effect_tick();
            }
            for _ in 0..PULSES_PER_LINE {
                self.engine.pulse();
            }
        }
    }

    fn note(text: &str) -> Note {
        let mut parts = text.split(' ');
        let kind = match parts.next().unwrap() {
            "===" => NoteKind::Off,
            pitch => NoteKind::On(
                (NotePitch::MIN..=NotePitch::MAX)
                    .map(NotePitch)
                    .find(|p| p.to_string() == pitch)
                    .unwrap(),
            ),
        };
        let instrument = parts.next().map(|i| u8::from_str_radix(i, 16).unwrap());
        let volume = parts
            .next()
            .filter(|v| *v != "--")
            .map(|v| u8::from_str_radix(v, 16).unwrap());
        return Note::new(kind, instrument, volume, Command::default());
    }

    fn command(kind: MidiCommand, value: u8, instrument: u8) -> Note {
        let mut note = Note::empty();
        note.instrument_id = Some(instrument);
        note.command = Command::new(CommandKind::MidiCommand(kind), value);
        return note;
    }

    /// A pattern built from (row, track, note) cells.
    fn pattern(rows: u16, cells: &[(usize, usize, Note)]) -> Pattern {
        let mut pattern = Pattern::new(rows);
        for (row, track, note) in cells {
            pattern.set_event(*row, *track, *note);
        }
        return pattern;
    }

    #[test]
    fn first_row_plays_on_start() {
        let mut harness = Harness::new(vec![pattern(4, &[(0, 0, note("C-4 2 40"))])]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        assert_eq!(
            harness.messages(),
            vec![Message::Start, Message::NoteOn(Channel::Ch3, 60, 0x40)]
        );
    }

    #[test]
    fn midi_clock_is_off_by_default() {
        let mut harness = Harness::new(vec![pattern(2, &[])]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.next_row();
        assert!(!harness.messages().contains(&Message::TimingClock));
    }

    #[test]
    fn midi_clock_sends_a_pulse_on_start_and_every_pulse() {
        let mut harness = Harness::new(vec![pattern(2, &[])]);
        harness.engine.apply(EngineCommand::SetMidiClock(true));
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        assert_eq!(
            harness.messages(),
            vec![Message::Start, Message::TimingClock]
        );
        harness.next_row();
        let clocks = harness
            .messages()
            .into_iter()
            .filter(|m| *m == Message::TimingClock)
            .count();
        assert_eq!(clocks, PULSES_PER_LINE as usize);
    }

    #[test]
    fn empty_rows_send_nothing() {
        let mut harness = Harness::new(vec![pattern(4, &[])]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.messages();
        harness.next_row();
        assert!(harness.messages().is_empty());
    }

    #[test]
    fn empty_volume_plays_full_velocity() {
        let mut harness = Harness::new(vec![pattern(4, &[(0, 0, note("C-4 0 --"))])]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        assert!(
            harness
                .messages()
                .contains(&Message::NoteOn(Channel::Ch1, 60, VELOCITY_MAX))
        );
    }

    #[test]
    fn new_note_on_a_track_releases_the_previous_one() {
        let mut harness = Harness::new(vec![pattern(
            4,
            &[(0, 0, note("C-4 0 7F")), (1, 0, note("E-4 0 7F"))],
        )]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.messages();
        harness.next_row();
        assert_eq!(
            harness.messages(),
            vec![
                Message::NoteOff(Channel::Ch1, 60, 0),
                Message::NoteOn(Channel::Ch1, 64, 0x7F),
            ]
        );
    }

    #[test]
    fn note_off_releases_the_track() {
        let mut harness = Harness::new(vec![pattern(
            4,
            &[(0, 1, note("C-4 5 7F")), (1, 1, note("==="))],
        )]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.messages();
        harness.next_row();
        assert_eq!(
            harness.messages(),
            vec![Message::NoteOff(Channel::Ch6, 60, 0)]
        );
    }

    #[test]
    fn zero_volume_releases_without_a_new_note() {
        let mut harness = Harness::new(vec![pattern(
            4,
            &[(0, 0, note("C-4 0 7F")), (1, 0, note("E-4 0 00"))],
        )]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.messages();
        harness.next_row();
        assert_eq!(
            harness.messages(),
            vec![Message::NoteOff(Channel::Ch1, 60, 0)]
        );
    }

    #[test]
    fn tracks_play_in_track_order_on_their_own_channels() {
        let mut harness = Harness::new(vec![pattern(
            4,
            &[(0, 2, note("G-4 1 50")), (0, 0, note("C-4 0 40"))],
        )]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        assert_eq!(
            harness.messages(),
            vec![
                Message::Start,
                Message::NoteOn(Channel::Ch1, 60, 0x40),
                Message::NoteOn(Channel::Ch2, 67, 0x50),
            ]
        );
    }

    #[test]
    fn muted_channels_do_not_play() {
        let mut muted = pattern(4, &[(0, 0, note("C-4 3 7F")), (0, 1, note("D-4 4 7F"))]);
        muted.channel_mutes[3] = true;
        let mut harness = Harness::new(vec![muted]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        assert_eq!(
            harness.messages(),
            vec![Message::Start, Message::NoteOn(Channel::Ch5, 62, 0x7F)]
        );
    }

    #[test]
    fn set_cc_then_change_cc_sends_control_change() {
        let mut harness = Harness::new(vec![pattern(
            4,
            &[
                (0, 0, command(MidiCommand::SetCC, 74, 1)),
                (1, 0, command(MidiCommand::ChangeCC, 0x20, 1)),
            ],
        )]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        assert_eq!(harness.messages(), vec![Message::Start]);
        harness.next_row();
        assert_eq!(
            harness.messages(),
            vec![Message::ControlChange(Channel::Ch2, 74, 0x10)]
        );
    }

    #[test]
    fn command_on_a_note_row_comes_before_the_note_on() {
        let mut cell = note("C-4 0 7F");
        cell.command = Command::new(CommandKind::MidiCommand(MidiCommand::ChangeCC), 0x10);
        let mut harness = Harness::new(vec![pattern(4, &[(0, 0, cell)])]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        assert_eq!(
            harness.messages(),
            vec![
                Message::Start,
                Message::ControlChange(Channel::Ch1, 0, 0x08),
                Message::NoteOn(Channel::Ch1, 60, 0x7F),
            ]
        );
    }

    #[test]
    fn tracks_sharing_a_channel_change_their_own_cc() {
        let mut harness = Harness::new(vec![pattern(
            4,
            &[
                (0, 0, command(MidiCommand::SetCC, 74, 0)),
                (0, 1, command(MidiCommand::SetCC, 71, 0)),
                (1, 0, command(MidiCommand::ChangeCC, 2, 0)),
                (1, 1, command(MidiCommand::ChangeCC, 4, 0)),
            ],
        )]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.messages();
        harness.next_row();
        assert_eq!(
            harness.messages(),
            vec![
                Message::ControlChange(Channel::Ch1, 74, 1),
                Message::ControlChange(Channel::Ch1, 71, 2),
            ]
        );
    }

    /// The values of every CC message, in order.
    fn cc_values(messages: &[Message]) -> Vec<u8> {
        return messages
            .iter()
            .filter_map(|m| match m {
                Message::ControlChange(_, _, value) => Some(*value),
                _ => None,
            })
            .collect();
    }

    /// S02 on row 0, a slide from G00 on row 1 to CFF on row 3, all on track 0 and channel 1.
    fn slide_pattern() -> Pattern {
        return pattern(
            8,
            &[
                (0, 0, command(MidiCommand::SetCC, 2, 0)),
                (1, 0, command(MidiCommand::CCSlide, 0x00, 0)),
                (3, 0, command(MidiCommand::ChangeCC, 0xFF, 0)),
            ],
        );
    }

    #[test]
    fn change_cc_scales_values_to_seven_bits() {
        let mut harness = Harness::new(vec![pattern(
            4,
            &[
                (0, 0, command(MidiCommand::ChangeCC, 0xFF, 0)),
                (1, 0, command(MidiCommand::ChangeCC, 0x80, 0)),
            ],
        )]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.next_row();
        assert_eq!(cc_values(&harness.messages()), vec![127, 64]);
    }

    #[test]
    fn cc_slide_ramps_once_per_tick_to_the_next_cc() {
        let mut harness = Harness::new(vec![slide_pattern()]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.messages();
        harness.next_row();
        harness.next_row();
        harness.next_row();
        let messages = harness.messages();
        assert!(
            messages
                .iter()
                .all(|m| matches!(m, Message::ControlChange(Channel::Ch1, 2, _)))
        );
        // two rows of six ticks: the start value, eleven steps, then the target row's value
        assert_eq!(
            cc_values(&messages),
            vec![0, 10, 21, 31, 42, 53, 63, 74, 85, 95, 106, 116, 127]
        );
    }

    #[test]
    fn cc_slide_stops_at_its_target_row() {
        let mut harness = Harness::new(vec![slide_pattern()]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        for _ in 0..3 {
            harness.next_row();
        }
        harness.messages();
        harness.next_row();
        assert!(harness.messages().is_empty());
    }

    #[test]
    fn cc_slides_chain_into_each_other() {
        let mut harness = Harness::new(vec![pattern(
            4,
            &[
                (0, 0, command(MidiCommand::CCSlide, 0x00, 0)),
                (1, 0, command(MidiCommand::CCSlide, 0x0C, 0)),
                (2, 0, command(MidiCommand::ChangeCC, 0x00, 0)),
            ],
        )]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.next_row();
        harness.next_row();
        // up to 0C (6 in 7 bit) over a row, then back down over the next
        assert_eq!(
            cc_values(&harness.messages()),
            vec![0, 1, 2, 3, 4, 5, 6, 5, 4, 3, 2, 1, 0]
        );
    }

    #[test]
    fn cc_slide_crosses_into_the_next_pattern() {
        let mut harness = Harness::new(vec![
            pattern(2, &[(1, 0, command(MidiCommand::CCSlide, 0x00, 0))]),
            pattern(2, &[(0, 0, command(MidiCommand::ChangeCC, 0x18, 0))]),
        ]);
        harness.engine.apply(EngineCommand::SetSequence(vec![
            SequenceRow {
                pattern_id: PatternId(0),
                repeats: 1,
            },
            SequenceRow {
                pattern_id: PatternId(1),
                repeats: 1,
            },
        ]));
        harness.engine.apply(EngineCommand::PlaySong(0, true));
        harness.next_row();
        harness.next_row();
        assert_eq!(cc_values(&harness.messages()), vec![0, 2, 4, 6, 8, 10, 12]);
    }

    #[test]
    fn cc_slide_without_a_target_only_sends_its_value() {
        let mut harness = Harness::new(vec![pattern(
            4,
            &[
                (0, 0, command(MidiCommand::CCSlide, 0x40, 0)),
                (2, 0, command(MidiCommand::SetCC, 7, 0)),
                (3, 0, command(MidiCommand::ChangeCC, 0xFF, 0)),
            ],
        )]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.next_row();
        harness.next_row();
        assert_eq!(cc_values(&harness.messages()), vec![0x20]);
    }

    #[test]
    fn stopping_ends_a_running_slide() {
        let mut harness = Harness::new(vec![slide_pattern()]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.next_row();
        harness.engine.apply(EngineCommand::Stop);
        harness.messages();
        harness.engine.effect_tick();
        assert!(harness.messages().is_empty());
    }

    #[test]
    fn muted_slide_keeps_going_and_resumes_in_step() {
        let mut muted = slide_pattern();
        muted.channel_mutes[0] = true;
        let mut control = Harness::new(vec![slide_pattern()]);
        let mut harness = Harness::new(vec![slide_pattern()]);
        for h in [&mut control, &mut harness] {
            h.engine
                .apply(EngineCommand::PlayPattern(PatternId(0), true));
            h.next_row();
            h.messages();
        }
        harness
            .engine
            .apply(EngineCommand::SetPattern(PatternId(0), muted));
        control.next_row();
        harness.next_row();
        assert!(!control.messages().is_empty());
        assert!(harness.messages().is_empty());

        harness
            .engine
            .apply(EngineCommand::SetPattern(PatternId(0), slide_pattern()));
        control.next_row();
        harness.next_row();
        assert_eq!(harness.messages(), control.messages());
    }

    #[test]
    fn commands_on_muted_channels_still_update_state() {
        let mut muted = pattern(
            4,
            &[
                (0, 0, command(MidiCommand::SetCC, 74, 0)),
                (1, 0, command(MidiCommand::ChangeCC, 0x40, 0)),
            ],
        );
        muted.channel_mutes[0] = true;
        let mut harness = Harness::new(vec![muted]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        assert_eq!(harness.messages(), vec![Message::Start]);
        harness.engine.apply(EngineCommand::SetPattern(
            PatternId(0),
            pattern(4, &[(1, 0, command(MidiCommand::ChangeCC, 0x40, 0))]),
        ));
        harness.next_row();
        assert_eq!(
            harness.messages(),
            vec![Message::ControlChange(Channel::Ch1, 74, 0x20)]
        );
    }

    #[test]
    fn zero_volume_rows_still_run_their_command() {
        let mut cell = note("C-4 0 00");
        cell.command = Command::new(CommandKind::MidiCommand(MidiCommand::ChangeCC), 0x40);
        let mut harness = Harness::new(vec![pattern(4, &[(0, 0, cell)])]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        assert_eq!(
            harness.messages(),
            vec![
                Message::Start,
                Message::ControlChange(Channel::Ch1, 0, 0x20)
            ]
        );
    }

    /// A note cell that also has a MIDI command.
    fn with_command(mut cell: Note, kind: MidiCommand, value: u8) -> Note {
        cell.command = Command::new(CommandKind::MidiCommand(kind), value);
        return cell;
    }

    /// The velocity of every note on, in order.
    fn note_velocities(messages: &[Message]) -> Vec<u8> {
        return messages
            .iter()
            .filter_map(|m| match m {
                Message::NoteOn(_, _, velocity) => Some(*velocity),
                _ => None,
            })
            .collect();
    }

    #[test]
    fn velocity_slide_ramps_the_velocity_of_notes() {
        let mut harness = Harness::new(vec![pattern(
            8,
            &[
                (
                    0,
                    0,
                    with_command(note("C-4 0 --"), MidiCommand::VelocitySlide, 0x20),
                ),
                (1, 0, note("C-4 0 --")),
                (2, 0, note("C-4 0 --")),
                (3, 0, note("C-4 0 --")),
                (
                    4,
                    0,
                    with_command(note("C-4 0 --"), MidiCommand::ChangeCC, 0xA0),
                ),
                (5, 0, note("C-4 0 --")),
            ],
        )]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        for _ in 0..5 {
            harness.next_row();
        }
        let messages = harness.messages();
        // 20 -> A0 over four rows, then back to the empty volume column's full velocity
        assert_eq!(note_velocities(&messages), vec![16, 32, 48, 64, 80, 127]);
        // only the target row's own CC; the velocity slide sends nothing
        assert_eq!(cc_values(&messages), vec![80]);
    }

    #[test]
    fn velocity_slide_sends_nothing_by_itself() {
        let mut harness = Harness::new(vec![pattern(
            4,
            &[
                (0, 0, command(MidiCommand::VelocitySlide, 0x00, 0)),
                (2, 0, command(MidiCommand::VelocitySlide, 0xFF, 0)),
            ],
        )]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        assert_eq!(harness.messages(), vec![Message::Start]);
        harness.next_row();
        harness.next_row();
        harness.next_row();
        assert!(harness.messages().is_empty());
    }

    #[test]
    fn velocity_slide_lands_on_a_cc_target() {
        let mut harness = Harness::new(vec![pattern(
            4,
            &[
                (0, 0, command(MidiCommand::VelocitySlide, 0x20, 0)),
                (
                    2,
                    0,
                    with_command(note("C-4 0 --"), MidiCommand::ChangeCC, 0xA0),
                ),
            ],
        )]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.next_row();
        harness.messages();
        harness.next_row();
        assert_eq!(
            harness.messages(),
            vec![
                Message::ControlChange(Channel::Ch1, 0, 80),
                Message::NoteOn(Channel::Ch1, 60, 80),
            ]
        );
    }

    #[test]
    fn volume_column_wins_during_a_velocity_slide() {
        let mut harness = Harness::new(vec![pattern(
            8,
            &[
                (0, 0, command(MidiCommand::VelocitySlide, 0x20, 0)),
                (1, 0, note("C-4 0 10")),
                (2, 0, note("C-4 0 --")),
                (4, 0, command(MidiCommand::VelocitySlide, 0xA0, 0)),
            ],
        )]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.next_row();
        harness.next_row();
        assert_eq!(note_velocities(&harness.messages()), vec![0x10, 48]);
    }

    #[test]
    fn set_cc_does_not_end_a_velocity_slide() {
        let mut harness = Harness::new(vec![pattern(
            8,
            &[
                (0, 0, command(MidiCommand::VelocitySlide, 0x20, 0)),
                (1, 0, command(MidiCommand::SetCC, 5, 0)),
                (2, 0, note("C-4 0 --")),
                (4, 0, command(MidiCommand::VelocitySlide, 0xA0, 0)),
            ],
        )]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.next_row();
        harness.next_row();
        assert_eq!(note_velocities(&harness.messages()), vec![48]);
    }

    #[test]
    fn cc_velocity_slide_drives_the_cc_and_note_velocity() {
        let mut harness = Harness::new(vec![pattern(
            8,
            &[
                (0, 0, command(MidiCommand::SetCC, 7, 0)),
                (
                    1,
                    0,
                    with_command(note("C-4 0 --"), MidiCommand::CCVelocitySlide, 0x40),
                ),
                (2, 0, note("C-4 0 --")),
                (3, 0, command(MidiCommand::ChangeCC, 0xFF, 0)),
            ],
        )]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.messages();
        harness.next_row();
        let first_row = harness.messages();
        assert_eq!(
            first_row[..2],
            [
                Message::ControlChange(Channel::Ch1, 7, 32),
                Message::NoteOn(Channel::Ch1, 60, 32),
            ]
        );
        harness.next_row();
        harness.next_row();
        let messages = [first_row, harness.messages()].concat();
        assert_eq!(note_velocities(&messages), vec![32, 79]);
        let values = cc_values(&messages);
        assert_eq!(values.first(), Some(&32));
        assert_eq!(values.last(), Some(&127));
        assert!(values.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(
            messages
                .iter()
                .all(|m| !matches!(m, Message::ControlChange(_, control, _) if *control != 7))
        );
    }

    #[test]
    fn stop_recenters_pitch_bend_and_clears_pressure() {
        let mut harness = Harness::new(vec![pattern(
            4,
            &[
                (0, 0, command(MidiCommand::PitchBend, 0xFF, 3)),
                (0, 1, command(MidiCommand::AfterTouch, 0x80, 3)),
            ],
        )]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        assert_eq!(
            harness.messages(),
            vec![
                Message::Start,
                Message::PitchBend(Channel::Ch4, 16383),
                Message::ChannelPressure(Channel::Ch4, 64),
            ]
        );
        harness.engine.apply(EngineCommand::Stop);
        let messages = harness.messages();
        assert_eq!(
            messages[..3],
            [
                Message::Stop,
                Message::PitchBend(Channel::Ch4, 8192),
                Message::ChannelPressure(Channel::Ch4, 0),
            ]
        );
    }

    #[test]
    fn muted_bend_is_not_sent_but_still_reset_on_stop() {
        let mut muted = pattern(4, &[(0, 0, command(MidiCommand::PitchBend, 0x00, 2))]);
        muted.channel_mutes[2] = true;
        let mut harness = Harness::new(vec![muted]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        assert_eq!(harness.messages(), vec![Message::Start]);
        harness.engine.apply(EngineCommand::Stop);
        assert_eq!(
            harness.messages()[1],
            Message::PitchBend(Channel::Ch3, 8192)
        );
    }

    #[test]
    fn end_slide_lands_a_velocity_slide_and_stops() {
        let mut harness = Harness::new(vec![pattern(
            8,
            &[
                (
                    0,
                    0,
                    with_command(note("C-4 0 --"), MidiCommand::VelocitySlide, 0x20),
                ),
                (1, 0, note("C-4 0 --")),
                (
                    2,
                    0,
                    with_command(note("C-4 0 --"), MidiCommand::EndSlide, 0xA0),
                ),
                (3, 0, note("C-4 0 --")),
                (
                    4,
                    0,
                    with_command(note("C-4 0 --"), MidiCommand::VelocitySlide, 0x40),
                ),
            ],
        )]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        for _ in 0..4 {
            harness.next_row();
        }
        let messages = harness.messages();
        // the later V40 is a separate slide: the E row lands on A0 and row 3 is back to full
        assert_eq!(note_velocities(&messages), vec![16, 48, 80, 127, 32]);
        assert!(cc_values(&messages).is_empty());
    }

    #[test]
    fn end_slide_lands_a_cc_slide() {
        let mut ended = slide_pattern();
        ended.set_event(3, 0, command(MidiCommand::EndSlide, 0xFF, 0));
        let mut harness = Harness::new(vec![ended]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.messages();
        for _ in 0..3 {
            harness.next_row();
        }
        assert_eq!(
            cc_values(&harness.messages()),
            vec![0, 10, 21, 31, 42, 53, 63, 74, 85, 95, 106, 116, 127]
        );
        harness.next_row();
        assert!(harness.messages().is_empty());
    }

    #[test]
    fn end_slide_lands_both_outputs_of_a_cc_velocity_slide() {
        let mut harness = Harness::new(vec![pattern(
            8,
            &[
                (0, 0, command(MidiCommand::SetCC, 7, 0)),
                (1, 0, command(MidiCommand::CCVelocitySlide, 0x40, 0)),
                (
                    2,
                    0,
                    with_command(note("C-4 0 --"), MidiCommand::EndSlide, 0xC0),
                ),
            ],
        )]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.next_row();
        harness.messages();
        harness.next_row();
        let messages = harness.messages();
        assert_eq!(
            messages[messages.len() - 2..],
            [
                Message::ControlChange(Channel::Ch1, 7, 0x60),
                Message::NoteOn(Channel::Ch1, 60, 0x60),
            ]
        );
    }

    #[test]
    fn end_slide_without_a_running_slide_does_nothing() {
        let mut harness = Harness::new(vec![pattern(
            4,
            &[
                (0, 0, command(MidiCommand::SetCC, 7, 0)),
                (
                    1,
                    0,
                    with_command(note("C-4 0 --"), MidiCommand::EndSlide, 0x40),
                ),
            ],
        )]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.messages();
        harness.next_row();
        assert_eq!(
            harness.messages(),
            vec![Message::NoteOn(Channel::Ch1, 60, VELOCITY_MAX)]
        );
    }

    #[test]
    fn pattern_loop_replays_row_zero() {
        let mut harness = Harness::new(vec![pattern(2, &[(0, 0, note("C-4 0 7F"))])]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.messages();
        harness.next_row();
        harness.next_row();
        assert_eq!(
            harness.messages(),
            vec![
                Message::NoteOff(Channel::Ch1, 60, 0),
                Message::NoteOn(Channel::Ch1, 60, 0x7F),
            ]
        );
    }

    #[test]
    fn one_shot_pattern_stops_after_the_last_row() {
        let mut harness = Harness::new(vec![pattern(2, &[(0, 0, note("C-4 0 7F"))])]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), false));
        harness.messages();
        harness.next_row();
        harness.next_row();
        assert!(harness.messages().contains(&Message::Stop));
        assert_eq!(harness.engine.mode, PlayMode::Paused);
    }

    #[test]
    fn song_moves_to_the_next_sequence_row() {
        let mut harness = Harness::new(vec![
            pattern(1, &[(0, 0, note("C-4 0 7F"))]),
            pattern(1, &[(0, 0, note("D-4 0 7F"))]),
        ]);
        harness.engine.apply(EngineCommand::SetSequence(vec![
            SequenceRow {
                pattern_id: PatternId(0),
                repeats: 2,
            },
            SequenceRow {
                pattern_id: PatternId(1),
                repeats: 1,
            },
        ]));
        harness.engine.apply(EngineCommand::PlaySong(0, true));
        harness.messages();
        harness.next_row();
        assert!(
            harness
                .messages()
                .contains(&Message::NoteOn(Channel::Ch1, 60, 0x7F))
        );
        harness.next_row();
        assert_eq!(
            harness.messages(),
            vec![
                Message::NoteOff(Channel::Ch1, 60, 0),
                Message::NoteOn(Channel::Ch1, 62, 0x7F),
            ]
        );
    }

    #[test]
    fn stop_releases_held_notes_then_silences_everything() {
        let mut harness = Harness::new(vec![pattern(4, &[(0, 0, note("C-4 0 7F"))])]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.messages();
        harness.engine.apply(EngineCommand::Stop);
        let messages = harness.messages();
        assert_eq!(
            messages[..2],
            [Message::Stop, Message::NoteOff(Channel::Ch1, 60, 0)]
        );
        assert_eq!(messages.len(), 2 + MIDI_CHANNEL_MAX as usize + 1);
        assert!(
            messages[2..]
                .iter()
                .all(|m| matches!(m, Message::AllSoundOff(_)))
        );
    }

    #[test]
    fn playing_a_missing_pattern_stops() {
        let mut harness = Harness::new(vec![]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(3), true));
        assert_eq!(harness.messages()[0], Message::Stop);
        assert!(harness.engine.clock.is_none());
    }

    fn tempo(value: u8) -> Note {
        let mut note = Note::empty();
        note.command = Command::new(CommandKind::Effect(EffectCommand::SetTempo), value);
        return note;
    }

    fn delayed(mut cell: Note, ticks: u8) -> Note {
        cell.command = Command::new(CommandKind::Effect(EffectCommand::Delay), ticks);
        return cell;
    }

    #[test]
    fn delayed_note_plays_on_its_tick() {
        let mut harness =
            Harness::new(vec![pattern(2, &[(0, 0, delayed(note("C-4 0 40"), 0x12))])]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        assert_eq!(harness.messages(), vec![Message::Start]);
        harness.effect_tick();
        assert!(harness.messages().is_empty());
        harness.effect_tick();
        assert_eq!(
            harness.messages(),
            vec![Message::NoteOn(Channel::Ch1, 60, 0x40)]
        );
    }

    #[test]
    fn delayed_note_keeps_the_previous_note_until_it_plays() {
        let mut harness = Harness::new(vec![pattern(
            2,
            &[
                (0, 0, note("C-4 0 40")),
                (1, 0, delayed(note("D-4 0 40"), 0x13)),
            ],
        )]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.next_row();
        harness.messages();
        harness.effect_tick();
        harness.effect_tick();
        assert!(harness.messages().is_empty());
        harness.effect_tick();
        assert_eq!(
            harness.messages(),
            vec![
                Message::NoteOff(Channel::Ch1, 60, 0),
                Message::NoteOn(Channel::Ch1, 62, 0x40)
            ]
        );
    }

    #[test]
    fn delayed_note_off_releases_on_its_tick() {
        let mut harness = Harness::new(vec![pattern(
            2,
            &[(0, 0, note("C-4 0 40")), (1, 0, delayed(note("==="), 0x11))],
        )]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.next_row();
        harness.messages();
        harness.effect_tick();
        assert_eq!(
            harness.messages(),
            vec![Message::NoteOff(Channel::Ch1, 60, 0)]
        );
    }

    #[test]
    fn delay_longer_than_the_row_never_plays() {
        let mut harness =
            Harness::new(vec![pattern(2, &[(0, 0, delayed(note("C-4 0 40"), 0x16))])]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.next_row();
        harness.next_row();
        assert!(note_velocities(&harness.messages()).is_empty());
    }

    #[test]
    fn late_delay_10_plays_straight_away() {
        let mut harness =
            Harness::new(vec![pattern(2, &[(0, 0, delayed(note("C-4 0 40"), 0x10))])]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        assert_eq!(
            harness.messages(),
            vec![Message::Start, Message::NoteOn(Channel::Ch1, 60, 0x40)]
        );
    }

    fn rolled(mut cell: Note, every: u8) -> Note {
        cell.command = Command::new(CommandKind::Effect(EffectCommand::Roll), every);
        return cell;
    }

    /// The ticks of the current row that send a note on, starting with the row itself.
    fn note_on_ticks(harness: &mut Harness) -> Vec<u32> {
        let ticks = harness
            .engine
            .ticks_per_line(harness.engine.current_pattern().unwrap());
        let mut played = vec![];
        for tick in 0..ticks as u32 {
            if tick > 0 {
                harness.effect_tick();
            }
            if !note_velocities(&harness.messages()).is_empty() {
                played.push(tick);
            }
        }
        return played;
    }

    #[test]
    fn roll_plays_on_the_row_then_every_interval() {
        for (every, expected) in [
            (1, vec![0, 1, 2, 3, 4, 5]),
            (2, vec![0, 2, 4]),
            (3, vec![0, 3]),
            (6, vec![0]),
        ] {
            let mut harness =
                Harness::new(vec![pattern(2, &[(0, 0, rolled(note("C-4 0 40"), every))])]);
            harness
                .engine
                .apply(EngineCommand::PlayPattern(PatternId(0), true));
            assert_eq!(note_on_ticks(&mut harness), expected, "R{every:02X}");
        }
    }

    #[test]
    fn roll_changes_the_velocity_of_each_repeat() {
        let mut harness = Harness::new(vec![pattern(2, &[(0, 0, rolled(note("C-4 0 40"), 0x92))])]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.next_row();
        assert_eq!(note_velocities(&harness.messages()), vec![0x40, 0x42, 0x44]);
    }

    #[test]
    fn roll_down_to_silence_stops_sending_notes() {
        let mut harness = Harness::new(vec![pattern(2, &[(0, 0, rolled(note("C-4 0 04"), 0x52))])]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.next_row();
        assert_eq!(note_velocities(&harness.messages()), vec![0x04]);
    }

    #[test]
    fn roll_stops_at_the_next_row() {
        let mut harness = Harness::new(vec![pattern(2, &[(0, 0, rolled(note("C-4 0 40"), 2))])]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.next_row();
        harness.messages();
        assert!(note_on_ticks(&mut harness).is_empty());
    }

    #[test]
    fn roll_00_plays_once() {
        let mut harness = Harness::new(vec![pattern(2, &[(0, 0, rolled(note("C-4 0 40"), 0))])]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        assert_eq!(note_on_ticks(&mut harness), vec![0]);
    }

    fn legato(mut cell: Note, value: u8) -> Note {
        cell.command = Command::new(CommandKind::Effect(EffectCommand::Legato), value);
        return cell;
    }

    #[test]
    fn legato_on_the_row_starts_the_new_note_before_releasing_the_old() {
        let mut harness = Harness::new(vec![pattern(
            2,
            &[
                (0, 0, note("C-4 0 40")),
                (1, 0, legato(note("D-4 0 40"), 0x00)),
            ],
        )]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.messages();
        harness.next_row();
        assert_eq!(
            harness.messages(),
            vec![
                Message::NoteOn(Channel::Ch1, 62, 0x40),
                Message::NoteOff(Channel::Ch1, 60, 0),
            ]
        );
    }

    #[test]
    fn legato_1x_keeps_the_previous_note_for_x_ticks() {
        let mut harness = Harness::new(vec![pattern(
            2,
            &[
                (0, 0, note("C-4 0 40")),
                (1, 0, legato(note("D-4 0 40"), 0x12)),
            ],
        )]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.messages();
        harness.next_row();
        assert_eq!(
            harness.messages(),
            vec![Message::NoteOn(Channel::Ch1, 62, 0x40)]
        );
        harness.effect_tick();
        assert!(harness.messages().is_empty());
        harness.effect_tick();
        assert_eq!(
            harness.messages(),
            vec![Message::NoteOff(Channel::Ch1, 60, 0)]
        );
    }

    #[test]
    fn legato_0x_starts_the_note_x_ticks_before_its_row() {
        let mut harness = Harness::new(vec![pattern(
            2,
            &[
                (0, 0, note("C-4 0 40")),
                (1, 0, legato(note("D-4 0 40"), 0x02)),
            ],
        )]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.messages();
        for _ in 0..3 {
            harness.effect_tick();
        }
        assert!(harness.messages().is_empty());
        harness.effect_tick();
        assert_eq!(
            harness.messages(),
            vec![
                Message::NoteOn(Channel::Ch1, 62, 0x40),
                Message::NoteOff(Channel::Ch1, 60, 0),
            ]
        );
        harness.effect_tick();
        for _ in 0..PULSES_PER_LINE {
            harness.engine.pulse();
        }
        assert!(note_velocities(&harness.messages()).is_empty());
    }

    #[test]
    fn early_legato_looks_ahead_into_the_next_loop() {
        let mut harness = Harness::new(vec![pattern(
            2,
            &[
                (0, 0, legato(note("D-4 0 40"), 0x01)),
                (1, 0, note("C-4 0 40")),
            ],
        )]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.next_row();
        harness.messages();
        for _ in 0..5 {
            harness.effect_tick();
        }
        assert_eq!(
            harness.messages(),
            vec![
                Message::NoteOn(Channel::Ch1, 62, 0x40),
                Message::NoteOff(Channel::Ch1, 60, 0),
            ]
        );
    }

    #[test]
    fn cut_releases_the_note_on_its_tick() {
        let mut harness = Harness::new(vec![pattern(2, &[(0, 0, legato(note("C-4 0 40"), 0x33))])]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        assert_eq!(
            harness.messages(),
            vec![Message::Start, Message::NoteOn(Channel::Ch1, 60, 0x40)]
        );
        harness.effect_tick();
        harness.effect_tick();
        assert!(harness.messages().is_empty());
        harness.effect_tick();
        assert_eq!(
            harness.messages(),
            vec![Message::NoteOff(Channel::Ch1, 60, 0)]
        );
    }

    #[test]
    fn cut_without_a_note_cuts_the_playing_note() {
        let cut = legato(Note::empty(), 0x31);
        let mut harness = Harness::new(vec![pattern(2, &[(0, 0, note("C-4 0 40")), (1, 0, cut)])]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.next_row();
        harness.messages();
        harness.effect_tick();
        assert_eq!(
            harness.messages(),
            vec![Message::NoteOff(Channel::Ch1, 60, 0)]
        );
    }

    #[test]
    fn cut_on_tick_0_releases_without_playing_the_cell() {
        let mut harness = Harness::new(vec![pattern(
            2,
            &[
                (0, 0, note("C-4 0 40")),
                (1, 0, legato(note("D-4 0 40"), 0x30)),
            ],
        )]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.messages();
        harness.next_row();
        assert_eq!(
            harness.messages(),
            vec![Message::NoteOff(Channel::Ch1, 60, 0)]
        );
    }

    #[test]
    fn cut_past_the_end_of_the_row_never_happens() {
        let mut harness = Harness::new(vec![pattern(2, &[(0, 0, legato(note("C-4 0 40"), 0x36))])]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.messages();
        for _ in 1..6 {
            harness.effect_tick();
        }
        assert!(harness.messages().is_empty());
    }

    #[test]
    fn early_delay_starts_the_note_before_its_row() {
        let mut harness = Harness::new(vec![pattern(
            2,
            &[
                (0, 0, note("C-4 0 40")),
                (1, 0, delayed(note("D-4 0 40"), 0x02)),
            ],
        )]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.messages();
        for _ in 0..3 {
            harness.effect_tick();
        }
        assert!(harness.messages().is_empty());
        harness.effect_tick();
        assert_eq!(
            harness.messages(),
            vec![
                Message::NoteOff(Channel::Ch1, 60, 0),
                Message::NoteOn(Channel::Ch1, 62, 0x40),
            ]
        );
        harness.effect_tick();
        for _ in 0..PULSES_PER_LINE {
            harness.engine.pulse();
        }
        assert!(note_velocities(&harness.messages()).is_empty());
    }

    #[test]
    fn cut_also_stops_a_held_over_note() {
        let mut harness = Harness::new(vec![pattern(
            3,
            &[
                (0, 0, note("C-4 0 40")),
                (1, 0, legato(note("D-4 0 40"), 0x1F)),
                (2, 0, legato(Note::empty(), 0x30)),
            ],
        )]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.next_row();
        harness.messages();
        harness.next_row();
        let messages = harness.messages();
        assert!(messages.contains(&Message::NoteOff(Channel::Ch1, 60, 0)));
        assert!(messages.contains(&Message::NoteOff(Channel::Ch1, 62, 0)));
    }

    #[test]
    fn early_note_keeps_a_late_delay_on_the_row_before() {
        let mut harness = Harness::new(vec![pattern(
            2,
            &[
                (0, 0, delayed(note("C-4 0 40"), 0x13)),
                (1, 0, legato(note("D-4 0 40"), 0x02)),
            ],
        )]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.messages();
        for _ in 0..4 {
            harness.effect_tick();
        }
        assert_eq!(
            harness.messages(),
            vec![
                Message::NoteOn(Channel::Ch1, 60, 0x40),
                Message::NoteOn(Channel::Ch1, 62, 0x40),
                Message::NoteOff(Channel::Ch1, 60, 0),
            ]
        );
    }

    #[test]
    fn early_note_drops_the_rest_of_a_roll() {
        let mut harness = Harness::new(vec![pattern(
            2,
            &[
                (0, 0, rolled(note("C-4 0 40"), 0x01)),
                (1, 0, legato(note("D-4 0 40"), 0x02)),
            ],
        )]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.messages();
        for _ in 0..5 {
            harness.effect_tick();
        }
        let pitches: Vec<u8> = harness
            .messages()
            .iter()
            .filter_map(|m| match m {
                Message::NoteOn(_, pitch, _) => Some(*pitch),
                _ => None,
            })
            .collect();
        assert_eq!(pitches, vec![60, 60, 60, 62]);
    }

    #[test]
    fn early_note_uses_the_mutes_of_its_own_pattern() {
        let mut muted = pattern(1, &[(0, 0, delayed(note("D-4 0 40"), 0x02))]);
        muted.channel_mutes[0] = true;
        let mut harness = Harness::new(vec![pattern(1, &[(0, 0, note("C-4 0 40"))]), muted]);
        harness.engine.apply(EngineCommand::SetSequence(vec![
            SequenceRow {
                pattern_id: PatternId(0),
                repeats: 1,
            },
            SequenceRow {
                pattern_id: PatternId(1),
                repeats: 1,
            },
        ]));
        harness.engine.apply(EngineCommand::PlaySong(0, true));
        harness.messages();
        for _ in 0..5 {
            harness.effect_tick();
        }
        assert!(note_velocities(&harness.messages()).is_empty());
    }

    #[test]
    fn a_row_changed_after_its_note_started_early_still_plays() {
        let mut harness = Harness::new(vec![pattern(
            2,
            &[
                (0, 0, note("C-4 0 40")),
                (1, 0, delayed(note("D-4 0 40"), 0x02)),
            ],
        )]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        for _ in 0..5 {
            harness.effect_tick();
        }
        harness.engine.apply(EngineCommand::SetPattern(
            PatternId(0),
            pattern(2, &[(0, 0, note("C-4 0 40")), (1, 0, note("E-4 0 40"))]),
        ));
        harness.messages();
        for _ in 0..PULSES_PER_LINE {
            harness.engine.pulse();
        }
        assert!(
            harness
                .messages()
                .contains(&Message::NoteOn(Channel::Ch1, 64, 0x40))
        );
    }

    #[test]
    fn stopping_drops_a_pending_delayed_note() {
        let mut harness =
            Harness::new(vec![pattern(2, &[(0, 0, delayed(note("C-4 0 40"), 0x12))])]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.engine.apply(EngineCommand::Stop);
        assert!(harness.engine.scheduled_notes.iter().all(Option::is_none));
    }

    #[test]
    fn tempo_carries_over_into_the_next_pattern() {
        let mut harness = Harness::new(vec![
            pattern(1, &[(0, 0, tempo(0x90)), (0, 1, tempo(0x03))]),
            pattern(2, &[(1, 0, tempo(0x00))]),
        ]);
        harness.engine.apply(EngineCommand::SetBpm(0x60));
        harness.engine.apply(EngineCommand::SetSequence(vec![
            SequenceRow {
                pattern_id: PatternId(0),
                repeats: 1,
            },
            SequenceRow {
                pattern_id: PatternId(1),
                repeats: 1,
            },
        ]));
        harness.engine.apply(EngineCommand::PlaySong(0, true));
        assert_eq!(harness.engine.bpm(), 0x90);
        harness.next_row();
        assert_eq!(harness.engine.position.pattern.0, 1);
        assert_eq!(harness.engine.bpm(), 0x90);
        assert_eq!(harness.engine.ticks_per_line(&Pattern::new(1)), 3);
        harness.next_row();
        assert_eq!(harness.engine.bpm(), 0x60);
        assert_eq!(harness.engine.ticks_per_line(&Pattern::new(1)), 6);
    }

    #[test]
    fn tempo_applies_from_its_own_row() {
        let mut harness = Harness::new(vec![pattern(4, &[(0, 0, tempo(0xFA))])]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        let clock = harness.engine.clock.as_ref().unwrap();
        assert_eq!(clock.next_pulse - clock.row_start, pulse_duration(0xFA));
    }

    #[test]
    fn stopping_goes_back_to_the_song_tempo() {
        let mut harness = Harness::new(vec![pattern(4, &[(0, 0, tempo(0x90))])]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        harness.engine.apply(EngineCommand::Stop);
        assert_eq!(harness.engine.bpm(), DEFAULT_BPM);
    }

    #[test]
    fn velocity_maps_volume_column() {
        assert_eq!(velocity(None, None), Some(VELOCITY_MAX));
        assert_eq!(velocity(Some(0), None), None);
        assert_eq!(velocity(Some(0x40), None), Some(0x40));
        assert_eq!(velocity(Some(0x80), None), Some(VELOCITY_MAX));
    }

    #[test]
    fn volume_column_wins_over_a_velocity_slide() {
        assert_eq!(velocity(Some(0x10), Some(0x60)), Some(0x10));
        assert_eq!(velocity(None, Some(0x60)), Some(0x60));
        assert_eq!(velocity(None, Some(0)), None);
    }
    /// A harness following the clock input, with a song playing each pattern once.
    fn synced(patterns: Vec<Pattern>) -> Harness {
        let sequence = (0..patterns.len())
            .map(|id| SequenceRow {
                pattern_id: PatternId(id as u8),
                repeats: 1,
            })
            .collect();
        let mut harness = Harness::new(patterns);
        harness.engine.apply(EngineCommand::SetSequence(sequence));
        harness
            .engine
            .apply(EngineCommand::SetClockSource(ClockSource::External));
        return harness;
    }

    /// Sends `count` clock input pulses, 20ms apart, after `from` microseconds.
    fn external_pulses(harness: &mut Harness, from: u64, count: u64) {
        for i in 0..count {
            harness
                .engine
                .apply(EngineCommand::ExternalClock(from + i * 20_000));
        }
    }

    #[test]
    fn synced_start_waits_for_its_first_pulse() {
        let mut harness = synced(vec![pattern(2, &[(0, 0, note("C-4 0 40"))])]);
        harness.engine.apply(EngineCommand::ExternalStart);
        assert!(harness.messages().is_empty());
        external_pulses(&mut harness, 0, 1);
        assert_eq!(
            harness.messages(),
            vec![Message::Start, Message::NoteOn(Channel::Ch1, 60, 0x40)]
        );
    }

    #[test]
    fn synced_rows_follow_the_clock_input() {
        let mut harness = synced(vec![pattern(2, &[(1, 0, note("D-4 0 40"))])]);
        harness.engine.apply(EngineCommand::ExternalStart);
        external_pulses(&mut harness, 0, PULSES_PER_LINE as u64);
        harness.messages();
        assert_eq!(harness.engine.position.row, 0);
        external_pulses(&mut harness, 120_000, 1);
        assert_eq!(harness.engine.position.row, 1);
        assert_eq!(
            harness.messages(),
            vec![Message::NoteOn(Channel::Ch1, 62, 0x40)]
        );
    }

    #[test]
    fn synced_row_runs_its_effect_ticks_before_the_next_row() {
        let mut harness = synced(vec![pattern(2, &[(0, 0, delayed(note("C-4 0 40"), 0x12))])]);
        harness.engine.apply(EngineCommand::ExternalStart);
        external_pulses(&mut harness, 0, PULSES_PER_LINE as u64 + 1);
        assert!(
            harness
                .messages()
                .contains(&Message::NoteOn(Channel::Ch1, 60, 0x40))
        );
    }

    #[test]
    fn synced_continue_picks_up_on_the_next_row() {
        let mut harness = synced(vec![pattern(4, &[(2, 0, note("E-4 0 40"))])]);
        harness.engine.apply(EngineCommand::ExternalStart);
        external_pulses(&mut harness, 0, PULSES_PER_LINE as u64 + 1);
        harness.engine.apply(EngineCommand::ExternalStop);
        assert_eq!(harness.engine.mode, PlayMode::Paused);
        harness.messages();
        harness.engine.apply(EngineCommand::ExternalContinue);
        external_pulses(&mut harness, 1_000_000, 1);
        let messages = harness.messages();
        assert_eq!(messages[0], Message::Continue);
        assert!(messages.contains(&Message::NoteOn(Channel::Ch1, 64, 0x40)));
        assert_eq!(harness.engine.position.row, 2);
    }

    #[test]
    fn synced_start_plays_the_song_after_a_local_pattern_play() {
        let mut harness = synced(vec![pattern(1, &[]), pattern(1, &[])]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(1), true));
        harness.engine.apply(EngineCommand::Stop);
        harness.engine.apply(EngineCommand::ExternalStart);
        external_pulses(&mut harness, 0, 1);
        assert_eq!(harness.engine.mode, PlayMode::SongLoop);
        assert!(harness.engine.position.pattern == PatternId(0));
        external_pulses(&mut harness, 20_000, PULSES_PER_LINE as u64);
        assert!(harness.engine.position.pattern == PatternId(1));
    }

    #[test]
    fn synced_continue_after_a_local_pattern_play_starts_the_song() {
        let mut harness = synced(vec![pattern(4, &[]), pattern(4, &[])]);
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(1), true));
        harness.engine.apply(EngineCommand::Stop);
        harness.engine.apply(EngineCommand::ExternalContinue);
        external_pulses(&mut harness, 0, 1);
        assert_eq!(harness.engine.mode, PlayMode::SongLoop);
        assert!(harness.engine.position.pattern == PatternId(0));
        assert_eq!(harness.engine.position.row, 0);
    }

    #[test]
    fn synced_pulse_period_follows_the_clock_input() {
        let mut harness = synced(vec![pattern(2, &[])]);
        for i in 0..100 {
            harness
                .engine
                .apply(EngineCommand::ExternalClock(i * 10_000));
        }
        let period = harness.engine.pulse_duration();
        assert!(period > Duration::from_micros(9_900) && period < Duration::from_micros(10_100));
    }

    #[test]
    fn internal_clock_ignores_the_clock_input() {
        let mut harness = Harness::new(vec![pattern(2, &[(0, 0, note("C-4 0 40"))])]);
        harness.engine.apply(EngineCommand::ExternalStart);
        external_pulses(&mut harness, 0, 1);
        assert!(harness.messages().is_empty());
        assert_eq!(harness.engine.mode, PlayMode::Paused);
    }

    #[test]
    fn local_play_while_synced_keeps_the_measured_tempo() {
        let mut harness = synced(vec![pattern(4, &[])]);
        for i in 0..100 {
            harness
                .engine
                .apply(EngineCommand::ExternalClock(i * 10_000));
        }
        harness
            .engine
            .apply(EngineCommand::PlayPattern(PatternId(0), true));
        assert_eq!(harness.engine.mode, PlayMode::PatternLoop);
        assert!(!harness.engine.clock.as_ref().unwrap().follows_input);
        let period = harness.engine.pulse_duration();
        assert!(period > Duration::from_micros(9_900) && period < Duration::from_micros(10_100));
        external_pulses(&mut harness, 2_000_000, PULSES_PER_LINE as u64);
        assert_eq!(harness.engine.position.row, 0);
        harness.engine.apply(EngineCommand::ExternalStop);
        assert_eq!(harness.engine.mode, PlayMode::PatternLoop);
    }

    #[test]
    fn switching_to_sync_starts_from_the_song_tempo() {
        let mut harness = Harness::new(vec![pattern(2, &[])]);
        harness.engine.apply(EngineCommand::SetBpm(150));
        harness
            .engine
            .apply(EngineCommand::SetClockSource(ClockSource::External));
        assert_eq!(harness.engine.pulse_duration(), pulse_duration(150));
        assert_eq!(harness.engine.status.synced_bpm(), 150);
    }

    #[test]
    fn synced_bpm_is_measured_from_the_clock_input() {
        let mut harness = synced(vec![pattern(2, &[])]);
        for i in 0..100 {
            harness
                .engine
                .apply(EngineCommand::ExternalClock(i * 20_000));
        }
        assert_eq!(harness.engine.status.synced_bpm(), 125);
    }

    #[test]
    fn song_position_sets_where_continue_plays() {
        let mut harness = Harness::new(vec![
            pattern(4, &[]),
            pattern(4, &[(1, 0, note("G-4 0 40"))]),
        ]);
        harness.engine.apply(EngineCommand::SetSequence(vec![
            SequenceRow {
                pattern_id: PatternId(0),
                repeats: 2,
            },
            SequenceRow {
                pattern_id: PatternId(1),
                repeats: 1,
            },
        ]));
        harness
            .engine
            .apply(EngineCommand::SetClockSource(ClockSource::External));
        harness.engine.apply(EngineCommand::ExternalSongPosition(9));
        harness.engine.apply(EngineCommand::ExternalContinue);
        external_pulses(&mut harness, 0, 1);
        assert_eq!(harness.engine.position.sequence_index, 1);
        assert_eq!(harness.engine.position.row, 1);
        assert!(
            harness
                .messages()
                .contains(&Message::NoteOn(Channel::Ch1, 67, 0x40))
        );
    }

    #[test]
    fn song_position_wraps_around_the_song() {
        let mut harness = synced(vec![pattern(4, &[]), pattern(2, &[])]);
        harness.engine.apply(EngineCommand::ExternalSongPosition(8));
        harness.engine.apply(EngineCommand::ExternalContinue);
        external_pulses(&mut harness, 0, 1);
        assert_eq!(harness.engine.mode, PlayMode::SongLoop);
        assert!(harness.engine.position.pattern == PatternId(0));
        assert_eq!(harness.engine.position.row, 2);
    }

    #[test]
    fn synced_bpm_holds_steady_through_jitter() {
        let mut harness = synced(vec![pattern(2, &[])]);
        let mut shown = Vec::new();
        for i in 0..200u64 {
            let jitter = [0, 1_000, 0, 0, 1_000, 0, 0, 0][i as usize % 8];
            harness
                .engine
                .apply(EngineCommand::ExternalClock(i * 20_833 + jitter));
            if i > PULSES_PER_QUARTER as u64 {
                shown.push(harness.engine.status.synced_bpm());
            }
        }
        assert!(shown.iter().all(|&bpm| bpm == 120));
    }

    #[test]
    fn reconnecting_the_clock_input_restarts_the_measurement() {
        let mut harness = synced(vec![pattern(2, &[])]);
        external_pulses(&mut harness, 5_000_000, 30);
        external_pulses(&mut harness, 0, 30);
        assert_eq!(
            harness.engine.pulse_duration(),
            Duration::from_micros(20_000)
        );
    }
}
