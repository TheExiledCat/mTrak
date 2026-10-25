use midi::{
    Channel,
    Message::{ChannelPressure, ControlChange, PitchBend},
};
use num::FromPrimitive;
use rand::RngExt;

use crate::{
    data::{
        effect::{Effect, MidiCommand},
        midi::{MIDI_CHANNEL_MAX, MidiEvent, VELOCITY_MAX},
        note::Note,
    },
    tui::constants::TRACK_COUNT,
};

/// What the engine found ahead of a command, for commands that depend on later rows.
#[derive(Default)]
pub struct CommandContext {
    pub slide_target: Option<SlideTarget>,
}

/// The value a slide ends on and how many ticks away its row is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SlideTarget {
    pub value: u8,
    pub ticks: u32,
}

/// What happens to the note on the same cell as an effect command.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NoteAction {
    Play,
    Skip,
    /// Play it this many ticks into the row.
    Delay(u8),
    /// Play it now, then again every `every` ticks until the row ends, changing the velocity
    /// of each repeat by `volume` (see `retrig_velocity`).
    Retrig {
        every: u8,
        volume: u8,
    },
    /// Glide into it from the track's current note, which keeps sounding this many ticks
    /// after it starts.
    Legato(u8),
    /// Play it, then release the track's note this many ticks into the row. 0 releases it
    /// straight away without playing the cell's note.
    Cut(u8),
    /// Play it, then again every this many ticks until the row ends.
    RetrigNote(u8),
}

/// The velocity of a retrig's next hit, from the previous one. Follows FT2's multi retrig
/// table, with the steps doubled because MIDI velocity goes up to 127 instead of 64.
pub fn retrig_velocity(velocity: u8, volume: u8) -> u8 {
    let velocity = velocity as i32;
    let next = match volume {
        0x1..=0x5 => velocity - (2 << (volume - 0x1)),
        0x6 => velocity * 2 / 3,
        0x7 => velocity / 2,
        0x9..=0xD => velocity + (2 << (volume - 0x9)),
        0xE => velocity * 3 / 2,
        0xF => velocity * 2,
        _ => velocity,
    };
    return next.clamp(0, VELOCITY_MAX as i32) as u8;
}

/// Command values are 00-FF but MIDI data is 7 bit, so FF maps to 127.
fn cc_value(value: u8) -> u8 {
    return value >> 1;
}

const MIDI_CHANNEL_COUNT: usize = MIDI_CHANNEL_MAX as usize + 1;
const MOD_WHEEL_CC: u8 = 1;
const BEND_CENTER: u16 = 8192;

/// Maps a 00-FF command value to a 14 bit pitch bend: 00 is full down, 80 is no bend and
/// FF is full up.
fn bend_value(value: u8) -> u16 {
    let offset = value as i32 - 0x80;
    let scaled = if offset < 0 {
        offset * 8192 / 128
    } else {
        offset * 8191 / 127
    };
    return (BEND_CENTER as i32 + scaled) as u16;
}

/// What a glide drives; combined commands drive more than one.
#[derive(Clone, Copy, Default)]
struct GlideOutputs {
    /// The CC the glide sends on every change.
    cc: Option<u8>,
    /// Whether notes on the track take the glide's value as their velocity.
    velocity: bool,
}

struct Glide {
    channel: Channel,
    outputs: GlideOutputs,
    from: u8,
    to: u8,
    total_ticks: u32,
    elapsed: u32,
    /// The glide's command value at `elapsed`, before scaling to 7 bit.
    current: u8,
    last_sent: u8,
}
impl Glide {
    /// Moves one tick along the ramp; false once the target row is reached.
    fn step(&mut self) -> bool {
        self.elapsed += 1;
        if self.elapsed >= self.total_ticks {
            return false;
        }
        let from = self.from as i64;
        let to = self.to as i64;
        self.current = (from + (to - from) * self.elapsed as i64 / self.total_ticks as i64) as u8;
        return true;
    }
}

#[derive(Default)]
struct TrackState {
    /// The CC selected by an `S`; until then the instrument's default CC is used.
    active_control: Option<u8>,
    glide: Option<Glide>,
    /// A velocity for notes on this row only: a glide's start value without a target,
    /// or the end value of a glide that just reached its target row.
    row_velocity: Option<u8>,
    /// A glide that reached its target row this row, so an EndSlide there can land it.
    landed: Option<Glide>,
}
impl TrackState {
    /// Sends the start value and begins gliding towards the context's target, if there is one.
    fn start_glide(
        &mut self,
        events: &mut Vec<MidiEvent>,
        channel: Channel,
        outputs: GlideOutputs,
        value: u8,
        context: &CommandContext,
    ) {
        if let Some(control) = outputs.cc {
            events.push(MidiEvent::Message(ControlChange(
                channel,
                control,
                cc_value(value),
            )));
        }
        if outputs.velocity {
            self.row_velocity = Some(value);
        }
        self.glide = context
            .slide_target
            .filter(|target| target.ticks > 0)
            .map(|target| Glide {
                channel,
                outputs,
                from: value,
                to: target.value,
                total_ticks: target.ticks,
                elapsed: 0,
                current: value,
                last_sent: cc_value(value),
            });
    }
}

/// Keeps MIDI command state per track, so tracks sharing an instrument don't clash.
pub struct MidiCommandHandler {
    tracks: [TrackState; TRACK_COUNT as usize],
    /// Channels that were sent a pitch bend or pressure, so stopping can undo it.
    bent: [bool; MIDI_CHANNEL_COUNT],
    pressed: [bool; MIDI_CHANNEL_COUNT],
    /// Each instrument's default CC, by channel.
    default_controls: [u8; MIDI_CHANNEL_COUNT],
}
impl Default for MidiCommandHandler {
    fn default() -> Self {
        return Self::new();
    }
}

impl MidiCommandHandler {
    pub fn new() -> Self {
        return Self {
            tracks: Default::default(),
            bent: [false; MIDI_CHANNEL_COUNT],
            pressed: [false; MIDI_CHANNEL_COUNT],
            default_controls: [0; MIDI_CHANNEL_COUNT],
        };
    }

    pub fn set_default_control(&mut self, channel: usize, control: u8) {
        if let Some(default) = self.default_controls.get_mut(channel) {
            *default = control;
        }
    }

    /// The 7 bit velocity a velocity glide gives notes on this track's current row, if any.
    pub fn velocity(&self, track: usize) -> Option<u8> {
        let state = self.tracks.get(track)?;
        let glide = state
            .glide
            .as_ref()
            .filter(|glide| glide.outputs.velocity)
            .map(|glide| glide.current);
        return glide.or(state.row_velocity).map(cc_value);
    }

    pub fn handle_command(
        &mut self,
        events: &mut Vec<MidiEvent>,
        track: usize,
        command: MidiCommand,
        note: &Note,
        context: &CommandContext,
    ) {
        let Some(state) = self.tracks.get_mut(track) else {
            return;
        };
        // usually the row start tick already finished the glide EndSlide is the target of
        let ending = match command {
            MidiCommand::EndSlide => state.glide.take().or_else(|| state.landed.take()),
            _ => None,
        };
        // a glide ends on its target row; SetCC only ends glides that send the CC it replaces
        let ends_glide = match command {
            MidiCommand::SetCC => state
                .glide
                .as_ref()
                .is_some_and(|glide| glide.outputs.cc.is_some()),
            _ => command.is_slide_target(),
        };
        if ends_glide {
            state.glide = None;
        }
        let value = note.command.value;
        let channel = Channel::from_u8(note.instrument_id.unwrap_or(0)).unwrap();
        let control = state
            .active_control
            .unwrap_or(self.default_controls[channel as usize]);
        match command {
            MidiCommand::SetCC => state.active_control = Some(value),
            MidiCommand::ChangeCC => events.push(MidiEvent::Message(ControlChange(
                channel,
                control,
                cc_value(value),
            ))),
            MidiCommand::ModWheel => {
                state.active_control = Some(MOD_WHEEL_CC);
                events.push(MidiEvent::Message(ControlChange(
                    channel,
                    MOD_WHEEL_CC,
                    cc_value(value),
                )));
            }
            MidiCommand::CCSlide | MidiCommand::CCVelocitySlide | MidiCommand::VelocitySlide => {
                let outputs = GlideOutputs {
                    cc: command.slides_cc().then_some(control),
                    velocity: command != MidiCommand::CCSlide,
                };
                state.start_glide(events, channel, outputs, value, context);
            }
            MidiCommand::EndSlide => {
                let Some(glide) = ending else {
                    return;
                };
                if let Some(control) = glide.outputs.cc {
                    events.push(MidiEvent::Message(ControlChange(
                        glide.channel,
                        control,
                        cc_value(value),
                    )));
                }
                if glide.outputs.velocity {
                    state.row_velocity = Some(value);
                }
            }
            MidiCommand::AfterTouch => {
                self.pressed[channel as usize] = true;
                events.push(MidiEvent::Message(ChannelPressure(
                    channel,
                    cc_value(value),
                )));
            }
            MidiCommand::PitchBend => {
                self.bent[channel as usize] = true;
                events.push(MidiEvent::Message(PitchBend(channel, bend_value(value))));
            }
        }
    }

    /// Advances running slides by one tick; called at every row start and effect tick.
    pub fn tick(&mut self, events: &mut Vec<MidiEvent>) {
        for state in self.tracks.iter_mut() {
            state.row_velocity = None;
            state.landed = None;
            let Some(glide) = &mut state.glide else {
                continue;
            };
            // the target row's note gets the end velocity; its command sends the final CC
            if !glide.step() {
                if glide.outputs.velocity {
                    state.row_velocity = Some(glide.to);
                }
                state.landed = state.glide.take();
                continue;
            }
            let Some(control) = glide.outputs.cc else {
                continue;
            };
            let value = cc_value(glide.current);
            if value != glide.last_sent {
                glide.last_sent = value;
                events.push(MidiEvent::Message(ControlChange(
                    glide.channel,
                    control,
                    value,
                )));
            }
        }
    }

    /// Forgets all command state, e.g. when playback starts or stops, and returns anything
    /// it left set on the synth to its default.
    pub fn reset(&mut self, events: &mut Vec<MidiEvent>) {
        self.tracks = Default::default();
        for index in 0..MIDI_CHANNEL_COUNT {
            let channel = Channel::from_u8(index as u8).unwrap();
            if self.bent[index] {
                events.push(MidiEvent::Message(PitchBend(channel, BEND_CENTER)));
            }
            if self.pressed[index] {
                events.push(MidiEvent::Message(ChannelPressure(channel, 0)));
            }
        }
        self.bent = [false; MIDI_CHANNEL_COUNT];
        self.pressed = [false; MIDI_CHANNEL_COUNT];
    }
}
/// Keeps the tempo set by effect commands, which carries over pattern boundaries until it is
/// reset by 00 or by playback starting or stopping.
pub struct EffectCommandHandler {
    bpm: Option<u8>,
    speed: Option<u8>,
}
impl Default for EffectCommandHandler {
    fn default() -> Self {
        return Self::new();
    }
}

impl EffectCommandHandler {
    pub fn new() -> Self {
        return Self {
            bpm: None,
            speed: None,
        };
    }

    /// The BPM set by a tempo command, overriding the song's.
    pub fn bpm(&self) -> Option<u8> {
        return self.bpm;
    }
    /// The ticks per line set by a tempo command, overriding the pattern's.
    pub fn speed(&self) -> Option<u8> {
        return self.speed;
    }

    /// Runs an effect and says what happens to the note on its cell.
    pub fn apply(&mut self, effect: Effect) -> NoteAction {
        return match effect {
            Effect::ResetTempo => {
                self.bpm = None;
                self.speed = None;
                NoteAction::Play
            }
            Effect::SetSpeed(speed) => {
                self.speed = Some(speed);
                NoteAction::Play
            }
            Effect::SetBpm(bpm) => {
                self.bpm = Some(bpm);
                NoteAction::Play
            }
            Effect::Probability(0) => NoteAction::Skip,
            Effect::Probability(0xFF) => NoteAction::Play,
            Effect::Probability(chance) if rand::rng().random_range(0..=0xFF) < chance => {
                NoteAction::Play
            }
            Effect::Probability(_) => NoteAction::Skip,
            Effect::DelayEarly(_) => NoteAction::Play,
            Effect::DelayLate(0) => NoteAction::Play,
            Effect::DelayLate(ticks) => NoteAction::Delay(ticks),
            Effect::Retrig { every, volume } => NoteAction::Retrig { every, volume },
            Effect::LegatoEarly(_) => NoteAction::Legato(0),
            Effect::LegatoHold(overlap) => NoteAction::Legato(overlap),
            Effect::Cut(tick) => NoteAction::Cut(tick),
            Effect::RetrigNote(every) => NoteAction::RetrigNote(every),
            Effect::Unused => NoteAction::Play,
        };
    }

    /// Forgets the tempo, e.g. when playback starts or stops.
    pub fn reset(&mut self) {
        self.bpm = None;
        self.speed = None;
    }
}

#[cfg(test)]
mod tests {
    use midi::Message;

    use super::*;
    use crate::data::{
        effect::{Command, CommandKind, EffectCommand},
        note::Note,
    };

    fn cc_note(command: MidiCommand, value: u8, instrument: u8) -> Note {
        let mut note = Note::empty();
        note.instrument_id = Some(instrument);
        note.command = Command::new(CommandKind::MidiCommand(command), value);
        return note;
    }

    fn handle(handler: &mut MidiCommandHandler, track: usize, note: Note) -> Vec<Message> {
        let mut events = vec![];
        let CommandKind::MidiCommand(command) = note.command.kind else {
            unreachable!();
        };
        handler.handle_command(
            &mut events,
            track,
            command,
            &note,
            &CommandContext::default(),
        );
        return events.into_iter().flat_map(|e| e.to_messages()).collect();
    }

    #[test]
    fn change_cc_uses_the_tracks_selected_control() {
        let mut handler = MidiCommandHandler::new();
        assert!(handle(&mut handler, 0, cc_note(MidiCommand::SetCC, 74, 2)).is_empty());
        assert_eq!(
            handle(&mut handler, 0, cc_note(MidiCommand::ChangeCC, 100, 2)),
            vec![Message::ControlChange(Channel::Ch3, 74, 50)]
        );
    }

    #[test]
    fn mod_wheel_sends_cc_1_and_selects_it() {
        let mut handler = MidiCommandHandler::new();
        handler.set_default_control(0, 74);
        assert_eq!(
            handle(&mut handler, 0, cc_note(MidiCommand::ModWheel, 0x80, 0)),
            vec![Message::ControlChange(Channel::Ch1, 1, 64)]
        );
        assert_eq!(
            handle(&mut handler, 0, cc_note(MidiCommand::ChangeCC, 0x40, 0)),
            vec![Message::ControlChange(Channel::Ch1, 1, 32)]
        );
    }

    #[test]
    fn change_cc_without_set_cc_uses_the_instruments_default() {
        let mut handler = MidiCommandHandler::new();
        handler.set_default_control(2, 74);
        assert_eq!(
            handle(&mut handler, 0, cc_note(MidiCommand::ChangeCC, 100, 2)),
            vec![Message::ControlChange(Channel::Ch3, 74, 50)]
        );
        assert_eq!(
            handle(&mut handler, 0, cc_note(MidiCommand::ChangeCC, 100, 1)),
            vec![Message::ControlChange(Channel::Ch2, 0, 50)]
        );
    }

    #[test]
    fn set_cc_overrides_the_instruments_default() {
        let mut handler = MidiCommandHandler::new();
        handler.set_default_control(0, 74);
        handle(&mut handler, 0, cc_note(MidiCommand::SetCC, 71, 0));
        assert_eq!(
            handle(&mut handler, 0, cc_note(MidiCommand::ChangeCC, 100, 0)),
            vec![Message::ControlChange(Channel::Ch1, 71, 50)]
        );
    }

    #[test]
    fn reset_goes_back_to_the_instruments_default() {
        let mut handler = MidiCommandHandler::new();
        handler.set_default_control(0, 74);
        handle(&mut handler, 0, cc_note(MidiCommand::SetCC, 71, 0));
        handler.reset(&mut vec![]);
        assert_eq!(
            handle(&mut handler, 0, cc_note(MidiCommand::ChangeCC, 100, 0)),
            vec![Message::ControlChange(Channel::Ch1, 74, 50)]
        );
    }

    #[test]
    fn tracks_on_the_same_channel_keep_separate_controls() {
        let mut handler = MidiCommandHandler::new();
        handle(&mut handler, 0, cc_note(MidiCommand::SetCC, 74, 0));
        handle(&mut handler, 1, cc_note(MidiCommand::SetCC, 71, 0));
        assert_eq!(
            handle(&mut handler, 0, cc_note(MidiCommand::ChangeCC, 10, 0)),
            vec![Message::ControlChange(Channel::Ch1, 74, 5)]
        );
        assert_eq!(
            handle(&mut handler, 1, cc_note(MidiCommand::ChangeCC, 20, 0)),
            vec![Message::ControlChange(Channel::Ch1, 71, 10)]
        );
    }

    #[test]
    fn bend_value_keeps_80_as_the_center() {
        assert_eq!(bend_value(0x00), 0);
        assert_eq!(bend_value(0x40), 4096);
        assert_eq!(bend_value(0x80), BEND_CENTER);
        assert_eq!(bend_value(0xC0), 12319);
        assert_eq!(bend_value(0xFF), 16383);
    }

    #[test]
    fn aftertouch_and_pitch_bend_send_scaled_values() {
        let mut handler = MidiCommandHandler::new();
        assert_eq!(
            handle(&mut handler, 0, cc_note(MidiCommand::AfterTouch, 0xFF, 1)),
            vec![Message::ChannelPressure(Channel::Ch2, 127)]
        );
        assert_eq!(
            handle(&mut handler, 0, cc_note(MidiCommand::PitchBend, 0x80, 1)),
            vec![Message::PitchBend(Channel::Ch2, 8192)]
        );
    }

    #[test]
    fn reset_undoes_bend_and_pressure_on_used_channels_only() {
        let mut handler = MidiCommandHandler::new();
        handle(&mut handler, 0, cc_note(MidiCommand::PitchBend, 0xFF, 2));
        handle(&mut handler, 1, cc_note(MidiCommand::AfterTouch, 0x40, 5));
        let mut events = vec![];
        handler.reset(&mut events);
        let messages: Vec<Message> = events.into_iter().flat_map(|e| e.to_messages()).collect();
        assert_eq!(
            messages,
            vec![
                Message::PitchBend(Channel::Ch3, 8192),
                Message::ChannelPressure(Channel::Ch6, 0),
            ]
        );
        let mut events = vec![];
        handler.reset(&mut events);
        assert!(events.is_empty());
    }

    fn tempo(handler: &mut EffectCommandHandler, value: u8) {
        assert_eq!(
            handler.apply(EffectCommand::SetTempo.decode(value)),
            NoteAction::Play
        );
    }

    #[test]
    fn tempo_sets_bpm_from_20_and_speed_below() {
        let mut handler = EffectCommandHandler::new();
        tempo(&mut handler, 0x1F);
        assert_eq!((handler.bpm(), handler.speed()), (None, Some(0x1F)));
        tempo(&mut handler, 0x20);
        assert_eq!((handler.bpm(), handler.speed()), (Some(0x20), Some(0x1F)));
        tempo(&mut handler, 0x00);
        assert_eq!((handler.bpm(), handler.speed()), (None, None));
    }

    #[test]
    fn reset_clears_the_tempo() {
        let mut handler = EffectCommandHandler::new();
        tempo(&mut handler, 0x90);
        tempo(&mut handler, 0x03);
        handler.reset();
        assert_eq!((handler.bpm(), handler.speed()), (None, None));
    }

    #[test]
    fn probability_00_never_plays_and_ff_always_plays() {
        let mut handler = EffectCommandHandler::new();
        for _ in 0..100 {
            assert_eq!(
                handler.apply(EffectCommand::Probability.decode(0x00)),
                NoteAction::Skip
            );
            assert_eq!(
                handler.apply(EffectCommand::Probability.decode(0xFF)),
                NoteAction::Play
            );
        }
    }

    #[test]
    fn late_delay_holds_the_note_back_unless_it_is_10() {
        let mut handler = EffectCommandHandler::new();
        assert_eq!(
            handler.apply(EffectCommand::Extended.decode(0xD0)),
            NoteAction::Play
        );
        assert_eq!(
            handler.apply(EffectCommand::Extended.decode(0xD3)),
            NoteAction::Delay(3)
        );
    }

    #[test]
    fn retrig_with_zeros_is_still_a_retrig() {
        let mut handler = EffectCommandHandler::new();
        assert_eq!(
            handler.apply(EffectCommand::Retrig.decode(0x00)),
            NoteAction::Retrig {
                every: 0,
                volume: 0
            }
        );
    }

    #[test]
    fn retrig_splits_volume_and_interval() {
        let mut handler = EffectCommandHandler::new();
        assert_eq!(
            handler.apply(EffectCommand::Retrig.decode(0x02)),
            NoteAction::Retrig {
                every: 2,
                volume: 0
            }
        );
        assert_eq!(
            handler.apply(EffectCommand::Retrig.decode(0xA3)),
            NoteAction::Retrig {
                every: 3,
                volume: 0xA
            }
        );
    }

    #[test]
    fn retrig_velocity_follows_the_ft2_table() {
        let steps: Vec<u8> = (0..=0xF)
            .map(|volume| retrig_velocity(60, volume))
            .collect();
        assert_eq!(
            steps,
            vec![
                60, 58, 56, 52, 44, 28, 40, 30, 60, 62, 64, 68, 76, 92, 90, 120
            ]
        );
    }

    #[test]
    fn retrig_velocity_stays_in_midi_range() {
        assert_eq!(retrig_velocity(10, 0x5), 0);
        assert_eq!(retrig_velocity(100, 0xF), VELOCITY_MAX);
        assert_eq!(retrig_velocity(120, 0xD), VELOCITY_MAX);
    }

    #[test]
    fn out_of_range_track_sends_nothing() {
        let mut handler = MidiCommandHandler::new();
        let track = TRACK_COUNT as usize;
        assert!(handle(&mut handler, track, cc_note(MidiCommand::ChangeCC, 1, 0)).is_empty());
    }
}
