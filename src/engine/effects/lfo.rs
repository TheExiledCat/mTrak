use std::f32::consts::TAU;

use midi::{Channel, Message};
use rand::RngExt;
use strum::FromRepr;

use crate::{
    data::midi::{BEND_CENTER, CC_MAX, VELOCITY_MAX, bend_by, cc_by},
    engine::effects::{MidiEffect, Param, ParamKind, Timing, note_event},
};

pub const RATE_PARAM: usize = 0;
pub const DEPTH_PARAM: usize = 1;
pub const SHAPE_PARAM: usize = 2;
pub const RETRIGGER_PARAM: usize = 3;
pub const TARGET_PARAM: usize = 4;
pub const CC_PARAM: usize = 5;
pub const CENTER_PARAM: usize = 6;

/// In the order of `LfoShape`.
const SHAPE_NAMES: &[&str] = &["Sine", "Triangle", "Saw up", "Saw down", "Square", "Random"];
/// In the order of `LfoTarget`.
const TARGET_NAMES: &[&str] = &["Velocity", "Pitch bend", "CC"];

const PARAMS: &[Param] = &[
    Param {
        name: "Rate (ticks)",
        kind: ParamKind::Number {
            min: 1,
            max: u8::MAX,
        },
        default: 6,
    },
    Param {
        name: "Depth",
        kind: ParamKind::SignedNumber {
            min: i8::MIN,
            max: i8::MAX,
        },
        default: 0x20,
    },
    Param {
        name: "Shape",
        kind: ParamKind::Choice(SHAPE_NAMES),
        default: LfoShape::Sine as u8,
    },
    Param {
        name: "Retrigger",
        kind: ParamKind::Toggle,
        default: 0,
    },
    Param {
        name: "Target",
        kind: ParamKind::Choice(TARGET_NAMES),
        default: LfoTarget::Velocity as u8,
    },
    Param {
        name: "CC",
        kind: ParamKind::Number {
            min: 0,
            max: CC_MAX,
        },
        default: 1,
    },
    Param {
        name: "CC center",
        kind: ParamKind::Number {
            min: 0,
            max: CC_MAX,
        },
        default: 0x40,
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, FromRepr)]
#[repr(u8)]
enum LfoShape {
    Sine,
    Triangle,
    SawUp,
    SawDown,
    Square,
    /// A new random value every cycle.
    Random,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, FromRepr)]
#[repr(u8)]
enum LfoTarget {
    Velocity,
    Bend,
    CC,
}

/// Moves a value up and down in a repeating wave, `rate` ticks per cycle. Velocity changes
/// each note as it starts; bend and CC are sent on every tick while notes are held, on the
/// channel of the notes. The wave follows the pattern, or restarts with every note with
/// retrigger on. While stopped there are no ticks, so notes get the wave's start.
pub struct LfoEffect {
    rate: u8,
    /// How far the wave reaches either way, in velocity or CC steps; negative flips it. For
    /// bend, full depth reaches the whole range.
    depth: i8,
    shape: LfoShape,
    retrigger: bool,
    target: LfoTarget,
    cc: u8,
    /// The CC's value in the middle of the wave.
    center: u8,
    channel: Option<Channel>,
    /// Pitches held, so bend and CC only move while something plays.
    held: Vec<u8>,
    /// Ticks since the last note on, for retrigger.
    since_trigger: u32,
    /// The cycle the random value was picked for, and the value.
    random: Option<(u32, f32)>,
    /// The last bend or CC sent, so a value is only sent when it changes.
    sent: Option<Message>,
}

impl LfoEffect {
    pub fn new() -> Self {
        return Self {
            rate: PARAMS[RATE_PARAM].default,
            depth: PARAMS[DEPTH_PARAM].default as i8,
            shape: LfoShape::Sine,
            retrigger: false,
            target: LfoTarget::Velocity,
            cc: PARAMS[CC_PARAM].default,
            center: PARAMS[CENTER_PARAM].default,
            channel: None,
            held: Vec::new(),
            since_trigger: 0,
            random: None,
            sent: None,
        };
    }

    /// How far into the wave `timing` is, in ticks.
    fn ticks(&self, timing: Option<Timing>) -> u32 {
        if self.retrigger {
            return self.since_trigger;
        }
        return timing.map_or(0, |timing| timing.pulse);
    }

    /// The wave at `ticks`, from -1 to 1.
    fn wave(&mut self, ticks: u32) -> f32 {
        let rate = self.rate.max(1) as u32;
        let phase = (ticks % rate) as f32 / rate as f32;
        return match self.shape {
            LfoShape::Sine => (phase * TAU).sin(),
            LfoShape::Triangle if phase < 0.25 => phase * 4.0,
            LfoShape::Triangle if phase < 0.75 => 2.0 - phase * 4.0,
            LfoShape::Triangle => phase * 4.0 - 4.0,
            LfoShape::SawUp => phase * 2.0 - 1.0,
            LfoShape::SawDown => 1.0 - phase * 2.0,
            LfoShape::Square if phase < 0.5 => 1.0,
            LfoShape::Square => -1.0,
            LfoShape::Random => {
                let cycle = ticks / rate;
                match self.random {
                    Some((picked, value)) if picked == cycle => value,
                    _ => {
                        let value = rand::rng().random_range(-1.0..=1.0);
                        self.random = Some((cycle, value));
                        value
                    }
                }
            }
        };
    }

    /// How far the wave moves the value at `ticks`, in steps of depth.
    fn offset(&mut self, ticks: u32) -> f32 {
        return self.wave(ticks) * self.depth as f32;
    }

    /// Sends the bend or CC for `ticks`, unless it's what was sent last.
    fn modulate(&mut self, ticks: u32, out: &mut Vec<Message>) {
        let Some(channel) = self.channel else {
            return;
        };
        let offset = self.offset(ticks);
        let message = match self.target {
            LfoTarget::Velocity => return,
            LfoTarget::Bend => bend_by(channel, offset),
            LfoTarget::CC => cc_by(channel, self.cc, self.center, offset),
        };
        if self.sent.as_ref() != Some(&message) {
            self.sent = Some(message.clone());
            out.push(message);
        }
    }
}

impl Default for LfoEffect {
    fn default() -> Self {
        return Self::new();
    }
}

impl MidiEffect for LfoEffect {
    fn params(&self) -> &'static [Param] {
        return PARAMS;
    }

    fn param(&self, index: usize) -> u8 {
        return match index {
            RATE_PARAM => self.rate,
            DEPTH_PARAM => self.depth as u8,
            SHAPE_PARAM => self.shape as u8,
            RETRIGGER_PARAM => self.retrigger as u8,
            TARGET_PARAM => self.target as u8,
            CC_PARAM => self.cc,
            CENTER_PARAM => self.center,
            _ => 0,
        };
    }

    fn set_param(&mut self, index: usize, value: u8) {
        match index {
            RATE_PARAM => self.rate = value,
            DEPTH_PARAM => self.depth = value as i8,
            SHAPE_PARAM => self.shape = LfoShape::from_repr(value).unwrap_or(self.shape),
            RETRIGGER_PARAM => self.retrigger = value != 0,
            TARGET_PARAM => self.target = LfoTarget::from_repr(value).unwrap_or(self.target),
            CC_PARAM => self.cc = value,
            CENTER_PARAM => self.center = value,
            _ => {}
        }
    }

    /// Bend and CC are sent before each note on, so no note starts with an old value.
    fn process(&mut self, message: Message, timing: Option<Timing>, out: &mut Vec<Message>) {
        let Some(mut note) = note_event(&message) else {
            out.push(message);
            return;
        };
        self.channel = Some(note.channel);
        let pitch = note.pitch.0;
        if note.is_note_off {
            if let Some(at) = self.held.iter().position(|held| *held == pitch) {
                self.held.remove(at);
            }
            out.push(message);
            return;
        }
        self.held.push(pitch);
        if self.retrigger {
            self.since_trigger = 0;
        }
        let ticks = self.ticks(timing);
        if self.target != LfoTarget::Velocity {
            self.modulate(ticks, out);
            out.push(message);
            return;
        }
        let velocity = note.velocity as i32 + self.offset(ticks).round() as i32;
        note.velocity = velocity.clamp(1, VELOCITY_MAX as i32) as u8;
        out.push(note.to_message());
    }

    fn tick(&mut self, timing: Timing, out: &mut Vec<Message>) {
        self.since_trigger = self.since_trigger.saturating_add(1);
        if !self.held.is_empty() {
            self.modulate(self.ticks(Some(timing)), out);
        }
    }

    /// Puts back the middle of the wave for what was last moved.
    fn reset(&mut self, out: &mut Vec<Message>) {
        match self.sent.take() {
            Some(Message::PitchBend(channel, _)) => {
                out.push(Message::PitchBend(channel, BEND_CENTER));
            }
            Some(Message::ControlChange(channel, cc, _)) => {
                out.push(Message::ControlChange(channel, cc, self.center));
            }
            _ => {}
        }
        self.held.clear();
        self.since_trigger = 0;
        self.random = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lfo(target: LfoTarget, shape: LfoShape, rate: u8, depth: i8) -> LfoEffect {
        let mut lfo = LfoEffect::new();
        lfo.set_param(TARGET_PARAM, target as u8);
        lfo.set_param(SHAPE_PARAM, shape as u8);
        lfo.set_param(RATE_PARAM, rate);
        lfo.set_param(DEPTH_PARAM, depth as u8);
        return lfo;
    }

    fn on(pitch: u8, velocity: u8) -> Message {
        return Message::NoteOn(Channel::Ch1, pitch, velocity);
    }

    fn off(pitch: u8) -> Message {
        return Message::NoteOff(Channel::Ch1, pitch, 0);
    }

    fn cc(value: u8) -> Message {
        return Message::ControlChange(Channel::Ch1, 1, value);
    }

    /// The velocity a note gets at each pulse.
    fn velocities(lfo: &mut LfoEffect, pulses: u32) -> Vec<u8> {
        return (0..pulses)
            .map(|pulse| {
                let mut out = vec![];
                lfo.process(on(60, 64), Some(Timing::new(pulse)), &mut out);
                lfo.process(off(60), Some(Timing::new(pulse)), &mut out);
                let Message::NoteOn(_, _, velocity) = out[0] else {
                    panic!("no note on");
                };
                velocity
            })
            .collect();
    }

    #[test]
    fn every_shape_and_target_has_a_name() {
        assert_eq!(
            LfoShape::from_repr(SHAPE_NAMES.len() as u8 - 1),
            Some(LfoShape::Random)
        );
        assert_eq!(LfoShape::from_repr(SHAPE_NAMES.len() as u8), None);
        assert_eq!(
            LfoTarget::from_repr(TARGET_NAMES.len() as u8 - 1),
            Some(LfoTarget::CC)
        );
        assert_eq!(LfoTarget::from_repr(TARGET_NAMES.len() as u8), None);
    }

    #[test]
    fn shapes_go_from_minus_one_to_one() {
        let mut lfo = lfo(LfoTarget::Velocity, LfoShape::Triangle, 8, 0);
        let triangle: Vec<f32> = (0..8).map(|tick| lfo.wave(tick)).collect();
        assert_eq!(triangle, vec![0.0, 0.5, 1.0, 0.5, 0.0, -0.5, -1.0, -0.5]);
        lfo.shape = LfoShape::SawUp;
        assert_eq!(lfo.wave(0), -1.0);
        assert_eq!(lfo.wave(4), 0.0);
        lfo.shape = LfoShape::SawDown;
        assert_eq!(lfo.wave(0), 1.0);
        lfo.shape = LfoShape::Square;
        assert_eq!((lfo.wave(3), lfo.wave(4)), (1.0, -1.0));
        lfo.shape = LfoShape::Sine;
        assert_eq!(lfo.wave(2), 1.0);
        assert_eq!(lfo.wave(8), 0.0);
    }

    #[test]
    fn random_keeps_its_value_for_a_cycle() {
        let mut lfo = lfo(LfoTarget::Velocity, LfoShape::Random, 4, 0);
        let first = lfo.wave(0);
        assert!((-1.0..=1.0).contains(&first));
        assert_eq!(lfo.wave(3), first);
        lfo.wave(4);
        assert_eq!(lfo.random.unwrap().0, 1);
    }

    #[test]
    fn velocity_follows_the_wave_through_the_pattern() {
        let mut lfo = lfo(LfoTarget::Velocity, LfoShape::Triangle, 4, 0x20);
        assert_eq!(velocities(&mut lfo, 5), vec![64, 96, 64, 32, 64]);
    }

    #[test]
    fn velocity_stays_a_note_on() {
        let mut lfo = lfo(LfoTarget::Velocity, LfoShape::Square, 2, i8::MIN);
        assert_eq!(velocities(&mut lfo, 2), vec![1, 127]);
    }

    #[test]
    fn a_negative_depth_flips_the_wave() {
        let mut lfo = lfo(LfoTarget::Velocity, LfoShape::Triangle, 4, -0x20);
        assert_eq!(velocities(&mut lfo, 2), vec![64, 32]);
    }

    #[test]
    fn cc_moves_round_its_center_while_notes_are_held() {
        let mut lfo = lfo(LfoTarget::CC, LfoShape::Square, 2, 0x10);
        let mut out = vec![];
        lfo.tick(Timing::new(0), &mut out);
        assert!(out.is_empty(), "nothing is held yet");
        lfo.process(on(60, 100), Some(Timing::new(0)), &mut out);
        assert_eq!(out, vec![cc(0x50), on(60, 100)]);
        out.clear();
        for pulse in 1..=3 {
            lfo.tick(Timing::new(pulse), &mut out);
        }
        assert_eq!(out, vec![cc(0x30), cc(0x50), cc(0x30)]);
        out.clear();
        lfo.process(off(60), Some(Timing::new(3)), &mut out);
        lfo.tick(Timing::new(4), &mut out);
        assert_eq!(out, vec![off(60)]);
    }

    #[test]
    fn unchanged_values_are_not_sent_again() {
        let mut lfo = lfo(LfoTarget::CC, LfoShape::Square, 8, 0x10);
        let mut out = vec![];
        lfo.process(on(60, 100), Some(Timing::new(0)), &mut out);
        for pulse in 1..4 {
            lfo.tick(Timing::new(pulse), &mut out);
        }
        assert_eq!(out, vec![cc(0x50), on(60, 100)]);
    }

    /// The bends a square wave of `depth` sends.
    fn square_bends(depth: i8) -> Vec<Message> {
        let mut lfo = lfo(LfoTarget::Bend, LfoShape::Square, 2, depth);
        let mut out = vec![];
        lfo.process(on(60, 100), Some(Timing::new(0)), &mut out);
        lfo.tick(Timing::new(1), &mut out);
        out.retain(|message| matches!(message, Message::PitchBend(..)));
        return out;
    }

    #[test]
    fn full_depth_bends_the_whole_range() {
        let bend = |value| Message::PitchBend(Channel::Ch1, value);
        assert_eq!(square_bends(i8::MAX), vec![bend(0x3FFF), bend(0)]);
        assert_eq!(square_bends(i8::MIN), vec![bend(0), bend(0x3FFF)]);
        assert_eq!(square_bends(-i8::MAX), vec![bend(0), bend(0x3FFF)]);
    }

    #[test]
    fn half_depth_bends_half_the_range() {
        let bend = |value| Message::PitchBend(Channel::Ch1, value);
        assert_eq!(square_bends(0x40), vec![bend(0x3020), bend(0x0FE0)]);
    }

    #[test]
    fn retrigger_starts_the_wave_over_with_each_note() {
        let mut lfo = lfo(LfoTarget::CC, LfoShape::SawUp, 4, 0x10);
        lfo.set_param(RETRIGGER_PARAM, 1);
        let mut out = vec![];
        for pulse in 0..3 {
            lfo.tick(Timing::new(pulse), &mut out);
        }
        lfo.process(on(60, 100), Some(Timing::new(3)), &mut out);
        lfo.tick(Timing::new(4), &mut out);
        assert_eq!(out, vec![cc(0x30), on(60, 100), cc(0x38)]);
    }

    #[test]
    fn reset_puts_back_the_center_of_what_was_moved() {
        let mut lfo = lfo(LfoTarget::Bend, LfoShape::Square, 2, 0x10);
        let mut out = vec![];
        lfo.process(on(60, 100), Some(Timing::new(0)), &mut out);
        lfo.set_param(TARGET_PARAM, LfoTarget::CC as u8);
        out.clear();
        lfo.reset(&mut out);
        assert_eq!(out, vec![Message::PitchBend(Channel::Ch1, 0x2000)]);
        out.clear();
        lfo.reset(&mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn other_messages_pass() {
        let mut lfo = lfo(LfoTarget::CC, LfoShape::Sine, 4, 0x10);
        let mut out = vec![];
        lfo.process(cc(5), None, &mut out);
        assert_eq!(out, vec![cc(5)]);
    }
}
