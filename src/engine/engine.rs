use std::{
    sync::{
        Arc,
        mpsc::{Receiver, RecvTimeoutError, Sender},
    },
    time::Instant,
};

use chrono::Utc;
use midi::{Message, ToRawMessages};
use midir::{MidiOutput, MidiOutputConnection, MidiOutputPort};

use crate::{
    data::{
        config::TITLE,
        effect::{CommandKind, MidiCommand},
        midi::{MIDI_CHANNEL_MAX, MidiEvent, PULSES_PER_LINE, VELOCITY_MAX, raw_message_to_bytes},
        note::{Note, NoteKind},
        pattern::{Pattern, PatternId},
        project::SequenceRow,
    },
    engine::command_handler::{
        CommandContext, CommandHandler, EffectCommandHandler, MidiCommandHandler, SlideTarget,
    },
};

use super::{
    handle::{EngineCommand, EngineStatus},
    voices::Voices,
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
    next_pulse: Instant,
    pulse_in_row: u32,
    row_start: Instant,
    tick: u32,
}

pub struct MtrakEngine {
    commands: Receiver<EngineCommand>,
    events: Sender<MidiEvent>,
    status: Arc<EngineStatus>,

    patterns: Vec<Pattern>,
    sequence: Vec<SequenceRow>,
    output_connection: Option<MidiOutputConnection>,
    pending_events: Vec<MidiEvent>,
    mode: PlayMode,
    position: Position,
    clock: Option<Clock>,
    voices: Voices,

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
            voices: Voices::new(),

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
            EngineCommand::Event(message) => self.pending_events.push(message),
            EngineCommand::PlayNote {
                track,
                channel,
                pitch,
            } => {
                let mut out = vec![];
                self.voices
                    .note_on(track, channel, pitch, VELOCITY_MAX, &mut out);
                self.dispatch_messages(out);
            }
            EngineCommand::ReleaseNote(track) => {
                let mut out = vec![];
                self.voices.release(track, &mut out);
                self.dispatch_messages(out);
            }
            EngineCommand::SetGlobalVolume { channel, volume } => {
                self.voices.set_global_volume(channel as usize, volume)
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

    /// Plays a single pattern from row 0, once or on repeat.
    fn play_pattern(&mut self, id: PatternId, looping: bool) {
        self.position = Position {
            sequence_index: 0,
            repeat: 0,
            pattern: id,
            row: 0,
        };
        self.start(if looping {
            PlayMode::PatternLoop
        } else {
            PlayMode::Pattern
        });
    }

    /// Plays the sequence starting at the given index.
    fn play_song(&mut self, sequence_index: usize, looping: bool) {
        let Some(sequence_row) = self.sequence.get(sequence_index) else {
            return;
        };
        self.position = Position {
            sequence_index,
            repeat: 0,
            pattern: sequence_row.pattern_id,
            row: 0,
        };
        self.start(if looping {
            PlayMode::SongLoop
        } else {
            PlayMode::Song
        });
    }

    /// Resets voices and the clock, then plays the first row.
    fn start(&mut self, mode: PlayMode) {
        let Some(pulse_duration) = self.current_pattern().map(Pattern::pulse_duration) else {
            self.stop();
            return;
        };
        let mut out = vec![];
        self.voices.release_all(&mut out);
        out.extend(self.reset_commands());
        self.dispatch_messages(out);
        let now = Instant::now();
        self.clock = Some(Clock {
            next_pulse: now + pulse_duration,
            pulse_in_row: 0,
            row_start: now,
            tick: 0,
        });
        self.mode = mode;
        self.status.set_play_mode(mode);
        self.publish_position();
        self.dispatch(MidiEvent::Message(Message::Start));
        self.play_row();
    }

    /// Stops playback and releases every sounding note.
    fn stop(&mut self) {
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
        let next_pulse = self.clock.as_ref()?.next_pulse;
        return Some(match self.next_effect_tick() {
            Some(tick) => tick.min(next_pulse),
            None => next_pulse,
        });
    }

    /// When the next effect tick within the current row is due, if any are left.
    fn next_effect_tick(&self) -> Option<Instant> {
        let clock = self.clock.as_ref()?;
        let pattern = self.current_pattern()?;
        let next = clock.tick + 1;
        if next >= pattern.ticks_per_line.max(1) as u32 {
            return None;
        }
        return Some(clock.row_start + pattern.tick_duration() * next);
    }

    /// Catches up on every pulse and effect tick that is due by now.
    fn advance_clock(&mut self) {
        let now = Instant::now();
        loop {
            let Some(clock) = &self.clock else {
                return;
            };
            let next_pulse = clock.next_pulse;
            match self.next_effect_tick() {
                Some(tick) if tick <= next_pulse && tick <= now => {
                    if let Some(clock) = &mut self.clock {
                        clock.tick += 1;
                    }
                    self.effect_tick();
                }
                _ if next_pulse <= now => self.pulse(),
                _ => return,
            }
        }
    }

    /// One MIDI clock pulse; moves to the next row every PULSES_PER_LINE pulses.
    fn pulse(&mut self) {
        let Some(pulse_duration) = self.current_pattern().map(Pattern::pulse_duration) else {
            self.stop();
            return;
        };
        let Some(clock) = &mut self.clock else {
            return;
        };
        let pulse_time = clock.next_pulse;
        clock.next_pulse += pulse_duration;
        clock.pulse_in_row += 1;
        if clock.pulse_in_row < PULSES_PER_LINE {
            return;
        }
        clock.pulse_in_row = 0;
        clock.row_start = pulse_time;
        clock.tick = 0;

        self.advance_row();
        if self.clock.is_some() {
            self.play_row();
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
            ticks += pattern.ticks_per_line.max(1) as u32;
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
    fn command_context(&self, track: usize, note: &Note) -> CommandContext {
        let CommandKind::MidiCommand(command) = note.command.kind else {
            return CommandContext::default();
        };
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
        let mut events = vec![];
        self.midi_command_handler.reset(&mut events);
        self.effect_command_handler.reset(&mut events);
        return events
            .into_iter()
            .flat_map(MidiEvent::to_messages)
            .collect();
    }

    /// Advances running effects by one tick and returns their messages.
    fn tick_commands(&mut self) -> Vec<MidiEvent> {
        let mut events = vec![];
        self.midi_command_handler.tick(&mut events);
        self.effect_command_handler.tick(&mut events);
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
        let mut out = vec![];
        self.voices.release_channels(&mutes, &mut out);
        out.extend(audible(row_tick, &mutes));
        for (track, note) in row.tracks.iter().enumerate() {
            if note.command.kind != CommandKind::None {
                let context = self.command_context(track, note);
                let events = Self::handle_command(
                    &mut self.midi_command_handler,
                    &mut self.effect_command_handler,
                    track,
                    note,
                    &context,
                );
                out.extend(audible(events, &mutes));
            }
            match note.kind {
                NoteKind::Empty => {}
                NoteKind::Off => self.voices.release(track, &mut out),
                NoteKind::On(pitch) => {
                    let channel = note.instrument_id.unwrap_or(0).min(MIDI_CHANNEL_MAX);
                    let glide = self.midi_command_handler.velocity(track);
                    match velocity(note.velocity, glide) {
                        Some(velocity) if !mutes[channel as usize] => self
                            .voices
                            .note_on(track, channel, pitch.0, velocity, &mut out),
                        _ => self.voices.release(track, &mut out),
                    }
                }
            }
        }
        self.dispatch_messages(out);
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
        let out = audible(events, &self.current_mutes());
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
        let pending: Vec<MidiEvent> = self.pending_events.drain(..).collect();
        for event in pending {
            self.dispatch(event);
        }
    }

    fn handle_command(
        midi_command_handler: &mut MidiCommandHandler,
        effect_command_handler: &mut EffectCommandHandler,
        track: usize,
        note: &Note,
        context: &CommandContext,
    ) -> Vec<MidiEvent> {
        let command = note.command;
        let mut midi_events = vec![];
        match command.kind {
            CommandKind::None => return midi_events,
            CommandKind::MidiCommand(midi_command) => {
                midi_command_handler.handle_command(
                    &mut midi_events,
                    track,
                    midi_command,
                    note,
                    context,
                );
            }
            CommandKind::Effect(effect_command) => {
                effect_command_handler.handle_command(
                    &mut midi_events,
                    track,
                    effect_command,
                    note,
                    context,
                );
            }
        }
        return midi_events;
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
fn velocity(volume: Option<u8>, glide: Option<u8>) -> Option<u8> {
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

    use crate::data::{
        effect::{Command, MidiCommand},
        note::NotePitch,
    };

    use super::*;

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

        /// Runs the rest of the row's effect ticks, then pulses the clock until the next row has played.
        fn next_row(&mut self) {
            let ticks = self.engine.current_pattern().unwrap().ticks_per_line.max(1);
            for _ in 1..ticks {
                self.engine.effect_tick();
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
            pitch => NoteKind::On(NotePitch::from_string(pitch).unwrap()),
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
    fn pattern(rows: u8, cells: &[(usize, usize, Note)]) -> Pattern {
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
}
