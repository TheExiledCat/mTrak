use midi::{
    Channel,
    Message::{ChannelPressure, ControlChange, PitchBend},
};
use num::FromPrimitive;

use crate::data::{
    effect::{EffectCommand, MidiCommand},
    midi::{MIDI_CHANNEL_MAX, MidiEvent},
    note::Note,
};
use crate::tui::constants::TRACK_COUNT;

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

pub trait CommandHandler<T> {
    fn handle_command(
        &mut self,
        events: &mut Vec<MidiEvent>,
        track: usize,
        command: T,
        note: &Note,
        context: &CommandContext,
    );
    /// Advances running effects by one tick; called at every row start and effect tick.
    fn tick(&mut self, _events: &mut Vec<MidiEvent>) {}
    /// Forgets all effect state, e.g. when playback starts or stops, and returns anything
    /// it left set on the synth to its default.
    fn reset(&mut self, _events: &mut Vec<MidiEvent>) {}
}

/// Command values are 00-FF but MIDI data is 7 bit, so FF maps to 127.
fn cc_value(value: u8) -> u8 {
    return value >> 1;
}

const MIDI_CHANNEL_COUNT: usize = MIDI_CHANNEL_MAX as usize + 1;
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
    active_control: u8,
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
}
impl MidiCommandHandler {
    pub fn new() -> Self {
        return Self {
            tracks: Default::default(),
            bent: [false; MIDI_CHANNEL_COUNT],
            pressed: [false; MIDI_CHANNEL_COUNT],
        };
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
}
impl CommandHandler<MidiCommand> for MidiCommandHandler {
    fn handle_command(
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
        let control = state.active_control;
        match command {
            MidiCommand::SetCC => state.active_control = value,
            MidiCommand::ChangeCC => events.push(MidiEvent::Message(ControlChange(
                channel,
                control,
                cc_value(value),
            ))),
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

    fn tick(&mut self, events: &mut Vec<MidiEvent>) {
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

    fn reset(&mut self, events: &mut Vec<MidiEvent>) {
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
pub struct EffectCommandHandler {}
impl EffectCommandHandler {
    pub fn new() -> Self {
        Self {}
    }
}
impl CommandHandler<EffectCommand> for EffectCommandHandler {
    fn handle_command(
        &mut self,
        events: &mut Vec<MidiEvent>,
        track: usize,
        command: EffectCommand,
        note: &Note,
        context: &CommandContext,
    ) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use midi::Message;

    use crate::data::{
        effect::{Command, CommandKind},
        note::Note,
    };

    use super::*;

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

    #[test]
    fn out_of_range_track_sends_nothing() {
        let mut handler = MidiCommandHandler::new();
        let track = TRACK_COUNT as usize;
        assert!(handle(&mut handler, track, cc_note(MidiCommand::ChangeCC, 1, 0)).is_empty());
    }
}
