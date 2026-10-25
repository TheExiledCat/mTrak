use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
};

use midir::MidiOutputPort;

use super::engine::{MtrakEngine, PlayMode};
use crate::{
    data::{
        midi::MidiEvent,
        note::NotePitch,
        pattern::{Pattern, PatternId},
        project::SequenceRow,
    },
    error::Result,
};

pub enum EngineCommand {
    SetOutputPort(Option<MidiOutputPort>),
    PlayPattern(PatternId, bool),
    PlaySong(usize, bool),
    /// Stops playback and silences and resets every channel.
    Panic,
    /// While playing a pattern, carries on in another one from the next row.
    SwitchPattern(PatternId),
    /// While playing the song, jumps to the start of a sequence position on the next row.
    JumpToSequence(usize),
    Stop,
    SetPattern(PatternId, Pattern),
    SetSequence(Vec<SequenceRow>),
    SetBpm(u8),
    SetSpeed(u8),
    Event(MidiEvent),
    PlayNote {
        track: usize,
        channel: u8,
        pitch: u8,
        velocity: u8,
    },
    ReleaseNote(usize),
    ReleasePitch {
        track: usize,
        pitch: u8,
    },

    SetGlobalVolume {
        channel: u8,
        volume: u8,
    },
    SetGlobalVelocity(u8),
    SetTranspose(i8),
    SetChannelTranspose {
        channel: u8,
        transpose: i8,
    },
    SetDefaultCC {
        channel: u8,
        control: u8,
    },
    SetMidiClock(bool),
    SetClockSource(ClockSource),
    /// A clock pulse from the clock input, with midir's timestamp in microseconds.
    ExternalClock(u64),
    /// A song position pointer from the clock input, in sixteenth notes.
    ExternalSongPosition(u16),
    ExternalStart,
    ExternalContinue,
    ExternalStop,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClockSource {
    Internal,
    External,
}

#[derive(Default)]
pub struct EngineStatus {
    play_mode: AtomicU8,
    sequence_index: AtomicUsize,
    pattern: AtomicUsize,
    row: AtomicUsize,
    output_connected: AtomicBool,
    synced_bpm: AtomicU8,
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
    /// The tempo measured from the clock input.
    pub fn synced_bpm(&self) -> u8 {
        return self.synced_bpm.load(Ordering::Relaxed);
    }

    pub fn set_play_mode(&self, mode: PlayMode) {
        self.play_mode.store(mode as u8, Ordering::Relaxed);
    }
    pub fn set_position(&self, sequence_index: usize, pattern: usize, row: usize) {
        self.sequence_index.store(sequence_index, Ordering::Relaxed);
        self.pattern.store(pattern, Ordering::Relaxed);
        self.row.store(row, Ordering::Relaxed);
    }
    pub fn set_synced_bpm(&self, bpm: u8) {
        self.synced_bpm.store(bpm, Ordering::Relaxed);
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
    pub fn spawn() -> Result<Self> {
        let (commands, receiver) = mpsc::channel();
        let (event_sender, events) = mpsc::channel();
        let status = Arc::new(EngineStatus::default());
        let engine = MtrakEngine::new(receiver, event_sender, status.clone());
        let thread = thread::Builder::new()
            .name("mtrak-engine".into())
            .spawn(move || engine.run())?;
        return Ok(Self {
            commands: Some(commands),
            events,
            status,
            thread: Some(thread),
        });
    }

    pub fn status(&self) -> &EngineStatus {
        return &self.status;
    }
    pub fn set_midi_clock(&self, enabled: bool) {
        self.send(EngineCommand::SetMidiClock(enabled));
    }
    pub fn set_clock_source(&self, source: ClockSource) {
        self.send(EngineCommand::SetClockSource(source));
    }
    /// A sender for the clock input's callback, so pulses reach the engine without going
    /// through the UI.
    pub fn clock_sender(&self) -> Option<Sender<EngineCommand>> {
        return self.commands.clone();
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
    pub fn switch_pattern(&self, id: PatternId) {
        self.send(EngineCommand::SwitchPattern(id));
    }
    pub fn jump_to_sequence(&self, sequence_index: usize) {
        self.send(EngineCommand::JumpToSequence(sequence_index));
    }
    pub fn panic(&self) {
        self.send(EngineCommand::Panic);
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
    pub fn set_bpm(&self, bpm: u8) {
        self.send(EngineCommand::SetBpm(bpm));
    }
    pub fn set_speed(&self, speed: u8) {
        self.send(EngineCommand::SetSpeed(speed));
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

    pub fn play_note(&self, track: usize, channel: u8, note: NotePitch, velocity: u8) {
        self.send(EngineCommand::PlayNote {
            track,
            channel,
            pitch: note.0,
            velocity,
        });
    }
    pub fn release_note(&self, track: usize) {
        self.send(EngineCommand::ReleaseNote(track));
    }
    pub fn release_pitch(&self, track: usize, note: NotePitch) {
        self.send(EngineCommand::ReleasePitch {
            track,
            pitch: note.0,
        });
    }

    pub fn set_global_volume(&self, channel: u8, volume: u8) {
        self.send(EngineCommand::SetGlobalVolume { channel, volume });
    }
    pub fn set_default_cc(&self, channel: u8, control: u8) {
        self.send(EngineCommand::SetDefaultCC { channel, control });
    }
    pub fn set_transpose(&self, transpose: i8) {
        self.send(EngineCommand::SetTranspose(transpose));
    }
    pub fn set_channel_transpose(&self, channel: u8, transpose: i8) {
        self.send(EngineCommand::SetChannelTranspose { channel, transpose });
    }
    pub fn set_global_velocity(&self, velocity: u8) {
        self.send(EngineCommand::SetGlobalVelocity(velocity));
    }
}

impl EngineHandle {
    pub fn shut_down(&mut self) {
        self.commands.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for EngineHandle {
    fn drop(&mut self) {
        self.shut_down();
    }
}
