use midi::{Channel, Message};
use strum::FromRepr;

use crate::{
    data::midi::{BEND_CENTER, CC_MAX, bend_by, cc_by},
    engine::effects::{MidiEffect, Param, ParamKind, Timing, ValueLabel, note_event},
};

pub const ATTACK_PARAM: usize = 0;
pub const DECAY_PARAM: usize = 1;
pub const SUSTAIN_PARAM: usize = 2;
pub const RELEASE_PARAM: usize = 3;
pub const OVERLAP_PARAM: usize = 4;
pub const DEPTH_PARAM: usize = 5;
pub const TARGET_PARAM: usize = 6;
pub const CC_PARAM: usize = 7;
pub const BASE_PARAM: usize = 8;

/// In the order of `EnvelopeTarget`.
const TARGET_NAMES: &[&str] = &["Pitch bend", "CC"];
/// In the order of `Overlap`.
const OVERLAP_NAMES: &[&str] = &["Restart", "Continue"];

const fn ticks(name: &'static str, default: u8) -> Param {
    return Param {
        name,
        kind: ParamKind::Number {
            min: 0,
            max: u8::MAX,
        },
        default,
        label: ValueLabel::Ticks,
    };
}

const PARAMS: &[Param] = &[
    ticks("Attack (ticks)", 0),
    ticks("Decay (ticks)", 0x0C),
    Param {
        name: "Sustain",
        kind: ParamKind::Number {
            min: 0,
            max: u8::MAX,
        },
        default: 0x80,
        label: ValueLabel::Percent,
    },
    ticks("Release (ticks)", 0x0C),
    Param {
        name: "Overlap",
        kind: ParamKind::Choice(OVERLAP_NAMES),
        default: Overlap::Restart as u8,
        label: ValueLabel::None,
    },
    Param {
        name: "Depth",
        kind: ParamKind::SignedNumber {
            min: i8::MIN,
            max: i8::MAX,
        },
        default: 0x7F,
        label: ValueLabel::None,
    },
    Param {
        name: "Target",
        kind: ParamKind::Choice(TARGET_NAMES),
        default: EnvelopeTarget::CC as u8,
        label: ValueLabel::None,
    },
    Param {
        name: "CC",
        kind: ParamKind::Cc,
        default: 1,
        label: ValueLabel::None,
    },
    Param {
        name: "CC base",
        kind: ParamKind::Number {
            min: 0,
            max: CC_MAX,
        },
        default: 0,
        label: ValueLabel::None,
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, FromRepr)]
#[repr(u8)]
enum EnvelopeTarget {
    Bend,
    CC,
}

/// What a note on does while the envelope is still moving.
#[derive(Clone, Copy, Debug, PartialEq, Eq, FromRepr)]
#[repr(u8)]
enum Overlap {
    /// Starts the attack from rest.
    Restart,
    /// Starts the attack from where the envelope is.
    Continue,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Stage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release { from: f32 },
}

/// An ADSR that moves the pitch bend or a CC: every note on starts the attack from wherever
/// the envelope is, and the release starts once no notes are held. It runs on ticks, so only
/// while playing; while stopped, notes pass and nothing is sent.
pub struct EnvelopeEffect {
    attack: u8,
    decay: u8,
    sustain: u8,
    release: u8,
    overlap: Overlap,
    depth: i8,
    target: EnvelopeTarget,
    cc: u8,
    base: u8,
    stage: Stage,
    /// From 0 at rest to 1 at the peak.
    level: f32,
    channel: Option<Channel>,
    held: Vec<u8>,
    /// So a value is only sent when it changes.
    sent: Option<Message>,
}

impl EnvelopeEffect {
    pub fn new() -> Self {
        return Self {
            attack: PARAMS[ATTACK_PARAM].default,
            decay: PARAMS[DECAY_PARAM].default,
            sustain: PARAMS[SUSTAIN_PARAM].default,
            release: PARAMS[RELEASE_PARAM].default,
            overlap: Overlap::Restart,
            depth: PARAMS[DEPTH_PARAM].default as i8,
            target: EnvelopeTarget::CC,
            cc: PARAMS[CC_PARAM].default,
            base: PARAMS[BASE_PARAM].default,
            stage: Stage::Idle,
            level: 0.0,
            channel: None,
            held: Vec::new(),
            sent: None,
        };
    }

    fn sustain_level(&self) -> f32 {
        return self.sustain as f32 / u8::MAX as f32;
    }

    fn advance(&mut self) {
        match self.stage {
            Stage::Idle => {}
            Stage::Attack => {
                self.level += 1.0 / self.attack.max(1) as f32;
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = Stage::Decay;
                }
            }
            Stage::Decay => {
                let sustain = self.sustain_level();
                self.level -= (1.0 - sustain) / self.decay.max(1) as f32;
                if self.level <= sustain {
                    self.level = sustain;
                    self.stage = Stage::Sustain;
                }
            }
            Stage::Sustain => self.level = self.sustain_level(),
            Stage::Release { from } => {
                self.level -= from / self.release.max(1) as f32;
                if self.level <= 0.0 {
                    self.level = 0.0;
                    self.stage = Stage::Idle;
                }
            }
        }
        self.skip_empty_stages();
    }

    /// Stages 0 ticks long end straight away.
    fn skip_empty_stages(&mut self) {
        loop {
            match self.stage {
                Stage::Attack if self.attack == 0 => {
                    self.level = 1.0;
                    self.stage = Stage::Decay;
                }
                Stage::Decay if self.decay == 0 => {
                    self.level = self.sustain_level();
                    self.stage = Stage::Sustain;
                }
                Stage::Release { .. } if self.release == 0 => {
                    self.level = 0.0;
                    self.stage = Stage::Idle;
                }
                _ => return,
            }
        }
    }

    fn rest_message(&self, channel: Channel) -> Message {
        return match self.target {
            EnvelopeTarget::Bend => Message::PitchBend(channel, BEND_CENTER),
            EnvelopeTarget::CC => Message::ControlChange(channel, self.cc, self.base),
        };
    }

    /// Sends the bend or CC for the envelope's level, unless it's what was sent last.
    fn modulate(&mut self, out: &mut Vec<Message>) {
        let Some(channel) = self.channel else {
            return;
        };
        let offset = self.level * self.depth as f32;
        let message = match self.target {
            EnvelopeTarget::Bend => bend_by(channel, offset),
            EnvelopeTarget::CC => cc_by(channel, self.cc, self.base, offset),
        };
        if self.sent.as_ref() != Some(&message) {
            self.sent = Some(message.clone());
            out.push(message);
        }
    }
}

impl MidiEffect for EnvelopeEffect {
    fn params(&self) -> &'static [Param] {
        return PARAMS;
    }

    fn param(&self, index: usize) -> u8 {
        return match index {
            ATTACK_PARAM => self.attack,
            DECAY_PARAM => self.decay,
            SUSTAIN_PARAM => self.sustain,
            RELEASE_PARAM => self.release,
            OVERLAP_PARAM => self.overlap as u8,
            DEPTH_PARAM => self.depth as u8,
            TARGET_PARAM => self.target as u8,
            CC_PARAM => self.cc,
            BASE_PARAM => self.base,
            _ => 0,
        };
    }

    fn set_param(&mut self, index: usize, value: u8) {
        match index {
            ATTACK_PARAM => self.attack = value,
            DECAY_PARAM => self.decay = value,
            SUSTAIN_PARAM => self.sustain = value,
            RELEASE_PARAM => self.release = value,
            OVERLAP_PARAM => self.overlap = Overlap::from_repr(value).unwrap_or(self.overlap),
            DEPTH_PARAM => self.depth = value as i8,
            TARGET_PARAM => self.target = EnvelopeTarget::from_repr(value).unwrap_or(self.target),
            CC_PARAM => self.cc = value,
            BASE_PARAM => self.base = value,
            _ => {}
        }
    }

    /// The value for the start of the attack is sent before the note on, so the note never
    /// starts with an old one.
    fn process(&mut self, message: Message, timing: Option<Timing>, out: &mut Vec<Message>) {
        let Some(note) = note_event(&message) else {
            out.push(message);
            return;
        };
        let pitch = note.pitch.0;
        if note.is_note_off {
            if let Some(at) = self.held.iter().position(|held| *held == pitch) {
                self.held.remove(at);
            }
            out.push(message);
            if self.held.is_empty() && self.stage != Stage::Idle {
                self.stage = Stage::Release { from: self.level };
                self.skip_empty_stages();
                self.modulate(out);
            }
            return;
        }
        self.held.push(pitch);
        if timing.is_some() {
            self.channel = Some(note.channel);
            if self.overlap == Overlap::Restart {
                self.level = 0.0;
            }
            self.stage = Stage::Attack;
            self.skip_empty_stages();
            self.modulate(out);
        }
        out.push(message);
    }

    fn tick(&mut self, _timing: Timing, out: &mut Vec<Message>) {
        if self.stage != Stage::Idle {
            self.advance();
            self.modulate(out);
        }
    }

    /// Puts back the bend or CC's resting value.
    fn reset(&mut self, out: &mut Vec<Message>) {
        if self.sent.take().is_some()
            && let Some(channel) = self.channel
        {
            out.push(self.rest_message(channel));
        }
        self.stage = Stage::Idle;
        self.level = 0.0;
        self.held.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope(attack: u8, decay: u8, sustain: u8, release: u8) -> EnvelopeEffect {
        let mut envelope = EnvelopeEffect::new();
        envelope.set_param(ATTACK_PARAM, attack);
        envelope.set_param(DECAY_PARAM, decay);
        envelope.set_param(SUSTAIN_PARAM, sustain);
        envelope.set_param(RELEASE_PARAM, release);
        return envelope;
    }

    fn on() -> Message {
        return Message::NoteOn(Channel::Ch1, 60, 100);
    }

    fn off() -> Message {
        return Message::NoteOff(Channel::Ch1, 60, 0);
    }

    fn cc(value: u8) -> Message {
        return Message::ControlChange(Channel::Ch1, 1, value);
    }

    /// The CC values sent on each pulse, with `messages` sent on the pulses they're paired with.
    fn run(
        envelope: &mut EnvelopeEffect,
        pulses: u32,
        messages: &[(u32, Message)],
    ) -> Vec<Vec<u8>> {
        let mut sent = vec![];
        for pulse in 0..=pulses {
            let timing = Timing::new(pulse);
            let mut out = vec![];
            envelope.tick(timing, &mut out);
            for (at, message) in messages {
                if *at == pulse {
                    envelope.process(message.clone(), Some(timing), &mut out);
                }
            }
            sent.push(
                out.into_iter()
                    .filter_map(|message| match message {
                        Message::ControlChange(_, _, value) => Some(value),
                        _ => None,
                    })
                    .collect(),
            );
        }
        return sent;
    }

    #[test]
    fn attack_decay_sustain_and_release_in_turn() {
        let mut envelope = envelope(2, 2, 0x80, 2);
        let sent = run(&mut envelope, 8, &[(0, on()), (5, off())]);
        assert_eq!(
            sent,
            vec![
                vec![0],
                vec![0x40],
                vec![0x7F],
                vec![0x5F],
                vec![0x40],
                vec![],
                vec![0x20],
                vec![0],
                vec![],
            ]
        );
    }

    #[test]
    fn the_peak_is_sent_before_the_note_without_an_attack() {
        let mut envelope = envelope(0, 0, 0xFF, 0);
        let mut out = vec![];
        envelope.process(on(), Some(Timing::new(0)), &mut out);
        assert_eq!(out, vec![cc(0x7F), on()]);
        out.clear();
        envelope.process(off(), Some(Timing::new(1)), &mut out);
        assert_eq!(out, vec![off(), cc(0)]);
    }

    #[test]
    fn the_release_waits_for_the_last_held_note() {
        let mut envelope = envelope(0, 0, 0xFF, 0);
        let other = Message::NoteOn(Channel::Ch1, 64, 100);
        let sent = run(&mut envelope, 2, &[(0, on()), (0, other), (1, off())]);
        assert_eq!(sent, vec![vec![0x7F], vec![], vec![]]);
    }

    #[test]
    fn continue_starts_the_attack_from_where_the_release_got_to() {
        let mut envelope = envelope(4, 0, 0xFF, 4);
        envelope.set_param(OVERLAP_PARAM, Overlap::Continue as u8);
        let sent = run(&mut envelope, 8, &[(0, on()), (4, off()), (6, on())]);
        assert_eq!(sent[4], vec![0x7F]);
        assert_eq!(sent[6], vec![0x40]);
        assert_eq!(sent[7], vec![0x5F]);
    }

    #[test]
    fn restart_starts_the_attack_from_rest() {
        let mut envelope = envelope(4, 0, 0xFF, 4);
        let sent = run(&mut envelope, 7, &[(0, on()), (4, off()), (6, on())]);
        assert_eq!(sent[6], vec![0x40, 0]);
        assert_eq!(sent[7], vec![0x20]);
    }

    #[test]
    fn negative_depth_and_the_base_move_the_cc_down() {
        let mut envelope = envelope(0, 0, 0xFF, 0);
        envelope.set_param(DEPTH_PARAM, -0x40i8 as u8);
        envelope.set_param(BASE_PARAM, 0x60);
        let mut out = vec![];
        envelope.process(on(), Some(Timing::new(0)), &mut out);
        assert_eq!(out[0], cc(0x20));
    }

    #[test]
    fn full_depth_bends_to_the_top_of_the_range() {
        let mut envelope = envelope(0, 0, 0xFF, 0);
        envelope.set_param(TARGET_PARAM, EnvelopeTarget::Bend as u8);
        let mut out = vec![];
        envelope.process(on(), Some(Timing::new(0)), &mut out);
        assert_eq!(out[0], Message::PitchBend(Channel::Ch1, 0x3FFF));
        envelope.reset(&mut out);
        assert_eq!(out[2], Message::PitchBend(Channel::Ch1, BEND_CENTER));
    }

    #[test]
    fn notes_pass_without_an_envelope_while_stopped() {
        let mut envelope = envelope(0, 0, 0xFF, 0);
        let mut out = vec![];
        envelope.process(on(), None, &mut out);
        envelope.process(off(), None, &mut out);
        assert_eq!(out, vec![on(), off()]);
    }

    #[test]
    fn reset_puts_the_cc_back_at_its_base() {
        let mut envelope = envelope(0, 0, 0xFF, 0);
        envelope.set_param(BASE_PARAM, 0x10);
        let mut out = vec![];
        envelope.process(on(), Some(Timing::new(0)), &mut out);
        out.clear();
        envelope.reset(&mut out);
        assert_eq!(out, vec![cc(0x10)]);
        out.clear();
        envelope.reset(&mut out);
        assert!(out.is_empty());
    }
}
