use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
};

use midir::MidiOutputPort;

use crate::data::{
    midi::MidiEvent,
    note::NotePitch,
    pattern::{Pattern, PatternId},
    project::SequenceRow,
};

use super::engine::{MtrakEngine, PlayMode};

pub enum EngineCommand {
    SetOutputPort(Option<MidiOutputPort>),
    PlayPattern(PatternId, bool),
    PlaySong(usize, bool),
    Stop,
    SetPattern(PatternId, Pattern),
    SetSequence(Vec<SequenceRow>),
    Event(MidiEvent),
    PlayNote {
        track: usize,
        channel: u8,
        pitch: u8,
    },
    ReleaseNote(usize),

    SetGlobalVolume {
        channel: u8,
        volume: u8,
    },
}

#[derive(Default)]
pub struct EngineStatus {
    play_mode: AtomicU8,
    sequence_index: AtomicUsize,
    pattern: AtomicUsize,
    row: AtomicUsize,
    output_connected: AtomicBool,
}
impl EngineStatus {
    pub fn play_mode(&self) -> PlayMode {
        return PlayMode::from_u8(self.play_mode.load(Ordering::Relaxed));
    }
    pub fn sequence_index(&self) -> usize {
        return self.sequence_index.load(Ordering::Relaxed);
    }
    pub fn pattern(&self) -> usize {
        return self.pattern.load(Ordering::Relaxed);
    }
    pub fn row(&self) -> usize {
        return self.row.load(Ordering::Relaxed);
    }
    pub fn output_connected(&self) -> bool {
        return self.output_connected.load(Ordering::Relaxed);
    }

    pub fn set_play_mode(&self, mode: PlayMode) {
        self.play_mode.store(mode as u8, Ordering::Relaxed);
    }
    pub fn set_position(&self, sequence_index: usize, pattern: usize, row: usize) {
        self.sequence_index.store(sequence_index, Ordering::Relaxed);
        self.pattern.store(pattern, Ordering::Relaxed);
        self.row.store(row, Ordering::Relaxed);
    }
    pub fn set_output_connected(&self, connected: bool) {
        self.output_connected.store(connected, Ordering::Relaxed);
    }
}

pub struct EngineHandle {
    commands: Option<Sender<EngineCommand>>,
    events: Receiver<MidiEvent>,
    status: Arc<EngineStatus>,
    thread: Option<JoinHandle<()>>,
}

impl EngineHandle {
    pub fn spawn() -> Self {
        let (commands, receiver) = mpsc::channel();
        let (event_sender, events) = mpsc::channel();
        let status = Arc::new(EngineStatus::default());
        let engine = MtrakEngine::new(receiver, event_sender, status.clone());
        let thread = thread::Builder::new()
            .name("mtrak-engine".into())
            .spawn(move || engine.run())
            .expect("failed to spawn the engine thread");
        return Self {
            commands: Some(commands),
            events,
            status,
            thread: Some(thread),
        };
    }

    pub fn status(&self) -> &EngineStatus {
        return &self.status;
    }
    pub fn set_output_port(&self, port: Option<MidiOutputPort>) {
        self.send(EngineCommand::SetOutputPort(port));
    }
    pub fn play_pattern(&self, id: PatternId, looping: bool) {
        self.send(EngineCommand::PlayPattern(id, looping));
    }
    pub fn play_song(&self, sequence_index: usize, looping: bool) {
        self.send(EngineCommand::PlaySong(sequence_index, looping));
    }
    pub fn stop(&self) {
        self.send(EngineCommand::Stop);
    }
    pub fn set_pattern(&self, id: PatternId, pattern: Pattern) {
        self.send(EngineCommand::SetPattern(id, pattern));
    }
    pub fn set_sequence(&self, sequence: Vec<SequenceRow>) {
        self.send(EngineCommand::SetSequence(sequence));
    }
    pub fn send_event(&self, message: MidiEvent) {
        self.send(EngineCommand::Event(message));
    }
    pub fn drain_events(&self) -> impl Iterator<Item = MidiEvent> + '_ {
        return self.events.try_iter();
    }

    fn send(&self, command: EngineCommand) {
        if let Some(commands) = &self.commands {
            let _ = commands.send(command);
        }
    }

    pub fn play_note(&self, track: usize, channel: u8, note: NotePitch) {
        self.send(EngineCommand::PlayNote {
            track,
            channel,
            pitch: note.0,
        });
    }
    pub fn release_note(&self, track: usize) {
        self.send(EngineCommand::ReleaseNote(track));
    }

    pub fn set_global_volume(&self, channel: u8, volume: u8) {
        self.send(EngineCommand::SetGlobalVolume { channel, volume });
    }
}

impl Drop for EngineHandle {
    fn drop(&mut self) {
        self.commands.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
