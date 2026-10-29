use midi::{Channel, Message};

use crate::{
    data::midi::PITCH_MAX,
    engine::{
        effects::{MidiEffect, Param, ParamKind, Timing, ValueLabel, note_event},
        notes::{Countdown, HeldNotes},
    },
};

/// The steps are the first params, so step `n` is param `n`.
pub const STEP_COUNT: usize = 8;
pub const LENGTH_PARAM: usize = STEP_COUNT;
pub const RATE_PARAM: usize = LENGTH_PARAM + 1;
pub const GATE_PARAM: usize = RATE_PARAM + 1;
const STEP_NAMES: [&str; STEP_COUNT] = [
    "Step 1", "Step 2", "Step 3", "Step 4", "Step 5", "Step 6", "Step 7", "Step 8",
];
const PARAMS: &[Param] = &params();

const fn params() -> [Param; GATE_PARAM + 1] {
    let mut params = [Param {
        name: "",
        kind: ParamKind::SignedNumber {
            min: i8::MIN,
            max: i8::MAX,
        },
        default: 0,
        label: ValueLabel::Semitones,
    }; GATE_PARAM + 1];
    let mut i = 0;
    while i < STEP_COUNT {
        params[i].name = STEP_NAMES[i];
        i += 1;
    }
    params[LENGTH_PARAM] = Param {
        name: "Length",
        kind: ParamKind::Number {
            min: 1,
            max: STEP_COUNT as u8,
        },
        default: 4,
        label: ValueLabel::None,
    };
    params[RATE_PARAM] = Param {
        name: "Rate (ticks)",
        kind: ParamKind::Number {
            min: 1,
            max: u8::MAX,
        },
        default: 6,
        label: ValueLabel::Ticks,
    };
    params[GATE_PARAM].name = "Gate (ticks)";
    params[GATE_PARAM].label = ValueLabel::Ticks;
    return params;
}

struct HeldArp {
    channel: Channel,
    root: u8,
    velocity: u8,
    /// How many steps have played, so the first can be told apart.
    played: u32,
    sounding: Vec<u8>,
}

enum ArpEvent {
    Step {
        channel: Channel,
        root: u8,
    },
    Off {
        channel: Channel,
        root: u8,
        pitch: u8,
    },
}
impl ArpEvent {
    fn is_for(&self, for_channel: Channel, for_root: u8) -> bool {
        let (Self::Step { channel, root } | Self::Off { channel, root, .. }) = *self;
        return channel == for_channel && root == for_root;
    }
}

/// Plays the steps one at a time, each `rate` ticks long, and loops while the root is held.
/// A negative gate starts each next note that many ticks before the last one's note off, a
/// positive gate holds each note off that many ticks past the next note on. While stopped
/// there are no ticks, so notes pass through.
pub struct ArpeggiatorEffect {
    steps: [i8; STEP_COUNT],
    length: u8,
    rate: u8,
    gate: i8,
    arps: Vec<HeldArp>,
    events: Countdown<ArpEvent>,
    /// The roots whose next step is due this pulse, played once the pulse's notes are in.
    due: Vec<(Channel, u8)>,
    notes: HeldNotes,
}

impl ArpeggiatorEffect {
    pub fn new() -> Self {
        return Self {
            steps: [0; STEP_COUNT],
            length: PARAMS[LENGTH_PARAM].default,
            rate: PARAMS[RATE_PARAM].default,
            gate: 0,
            arps: Vec::new(),
            events: Countdown::new(),
            due: Vec::new(),
            notes: HeldNotes::new(),
        };
    }

    /// How many ticks before the end of the step the next note starts; less than a step, so
    /// the notes keep their order.
    fn early(&self) -> u32 {
        return (self.gate.min(0).unsigned_abs() as u32).min(self.rate as u32 - 1);
    }
    /// How many ticks past the end of the step the note off comes.
    fn late(&self) -> u32 {
        return self.gate.max(0) as u32;
    }

    /// Plays the arp's next step and schedules its note off and the step after it. A step
    /// whose pitch is outside MIDI's range is a rest.
    fn play_step(&mut self, index: usize, out: &mut Vec<Message>) {
        let rate = self.rate as u32;
        let (early, late) = (self.early(), self.late());
        let arp = &mut self.arps[index];
        let step = self.steps[arp.played as usize % self.length as usize];
        let first = arp.played == 0;
        arp.played += 1;
        let (channel, root) = (arp.channel, arp.root);
        let pitch = root as i16 + step as i16;
        if (0..=PITCH_MAX as i16).contains(&pitch) {
            let pitch = pitch as u8;
            arp.sounding.push(pitch);
            self.notes.note_on(channel, pitch, arp.velocity, out);
            let length = if first {
                rate + late
            } else {
                rate + early + late
            };
            self.events.push(
                ArpEvent::Off {
                    channel,
                    root,
                    pitch,
                },
                length,
            );
        }
        let next = if first { rate - early } else { rate };
        self.events.push(ArpEvent::Step { channel, root }, next);
    }

    fn find(&self, channel: Channel, root: u8) -> Option<usize> {
        return self
            .arps
            .iter()
            .position(|arp| arp.channel == channel && arp.root == root);
    }

    /// Stops a root's arp, releasing what it has sounding. False if it had none.
    fn release(&mut self, channel: Channel, root: u8, out: &mut Vec<Message>) -> bool {
        self.events.remove(|event| event.is_for(channel, root));
        self.due.retain(|due| *due != (channel, root));
        let Some(index) = self.find(channel, root) else {
            return false;
        };
        for pitch in self.arps.remove(index).sounding {
            self.notes.note_off(channel, pitch, 0, out);
        }
        return true;
    }
}

impl MidiEffect for ArpeggiatorEffect {
    fn params(&self) -> &'static [Param] {
        return PARAMS;
    }

    fn param(&self, index: usize) -> u8 {
        return match index {
            i if i < STEP_COUNT => self.steps[i] as u8,
            LENGTH_PARAM => self.length,
            RATE_PARAM => self.rate,
            GATE_PARAM => self.gate as u8,
            _ => 0,
        };
    }

    fn set_param(&mut self, index: usize, value: u8) {
        match index {
            i if i < STEP_COUNT => self.steps[i] = value as i8,
            LENGTH_PARAM => self.length = value,
            RATE_PARAM => self.rate = value,
            GATE_PARAM => self.gate = value as i8,
            _ => {}
        }
    }

    fn process(&mut self, message: Message, timing: Option<Timing>, out: &mut Vec<Message>) {
        let Some(note) = note_event(&message) else {
            out.push(message);
            return;
        };
        let root = note.pitch.0;
        let released = self.release(note.channel, root, out);
        if note.is_note_off {
            if !released {
                out.push(message);
            }
            return;
        }
        if timing.is_none() {
            out.push(message);
            return;
        }
        self.arps.push(HeldArp {
            channel: note.channel,
            root,
            velocity: note.velocity,
            played: 0,
            sounding: Vec::new(),
        });
        self.play_step(self.arps.len() - 1, out);
    }

    fn tick(&mut self, _timing: Timing, out: &mut Vec<Message>) {
        for event in self.events.tick() {
            match event {
                ArpEvent::Step { channel, root } => self.due.push((channel, root)),
                ArpEvent::Off {
                    channel,
                    root,
                    pitch,
                } => {
                    let Some(index) = self.find(channel, root) else {
                        continue;
                    };
                    let sounding = &mut self.arps[index].sounding;
                    if let Some(at) = sounding.iter().position(|p| *p == pitch) {
                        sounding.remove(at);
                        self.notes.note_off(channel, pitch, 0, out);
                    }
                }
            }
        }
    }

    fn after_pulse(&mut self, _timing: Timing, out: &mut Vec<Message>) {
        for (channel, root) in std::mem::take(&mut self.due) {
            if let Some(index) = self.find(channel, root) {
                self.play_step(index, out);
            }
        }
    }

    fn reset(&mut self, out: &mut Vec<Message>) {
        self.events.clear();
        self.due.clear();
        self.arps.clear();
        self.notes.release_all(out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pulse as the engine runs it: the tick, then what waits for the pulse's notes.
    fn run_pulse(effect: &mut dyn MidiEffect, pulse: u32, out: &mut Vec<Message>) {
        effect.tick(Timing::new(pulse), out);
        effect.after_pulse(Timing::new(pulse), out);
    }

    fn arp(steps: &[i8], length: u8, rate: u8, gate: i8) -> ArpeggiatorEffect {
        let mut arp = ArpeggiatorEffect::new();
        for (index, step) in steps.iter().enumerate() {
            arp.set_param(index, *step as u8);
        }
        arp.set_param(LENGTH_PARAM, length);
        arp.set_param(RATE_PARAM, rate);
        arp.set_param(GATE_PARAM, gate as u8);
        return arp;
    }

    fn on(pitch: u8) -> Message {
        return Message::NoteOn(Channel::Ch1, pitch, 100);
    }

    fn off(pitch: u8) -> Message {
        return Message::NoteOff(Channel::Ch1, pitch, 0);
    }

    /// What the arp sends on each tick after a key goes down, the key's own tick first.
    fn run(arp: &mut ArpeggiatorEffect, ticks: u32) -> Vec<Vec<Message>> {
        let mut out = vec![];
        arp.process(on(60), Some(Timing::new(0)), &mut out);
        let mut sent = vec![out];
        for pulse in 1..=ticks {
            let mut out = vec![];
            run_pulse(arp, pulse, &mut out);
            sent.push(out);
        }
        return sent;
    }

    #[test]
    fn steps_play_in_order_and_repeat_after_the_length() {
        let mut arp = arp(&[0, 7, 3, 12], 3, 2, 0);
        let sent = run(&mut arp, 6);
        assert_eq!(
            sent,
            vec![
                vec![on(60)],
                vec![],
                vec![off(60), on(67)],
                vec![],
                vec![off(67), on(63)],
                vec![],
                vec![off(63), on(60)],
            ]
        );
    }

    #[test]
    fn a_negative_gate_starts_the_next_note_early() {
        let mut arp = arp(&[0, 7], 2, 4, -1);
        let sent = run(&mut arp, 8);
        assert_eq!(sent[3], vec![on(67)]);
        assert_eq!(sent[4], vec![off(60)]);
        assert_eq!(sent[7], vec![on(60)]);
        assert_eq!(sent[8], vec![off(67)]);
    }

    #[test]
    fn a_positive_gate_holds_the_note_off_late() {
        let mut arp = arp(&[0, 7], 2, 4, 2);
        let sent = run(&mut arp, 6);
        assert_eq!(sent[4], vec![on(67)]);
        assert_eq!(sent[6], vec![off(60)]);
    }

    #[test]
    fn a_negative_gate_never_reaches_the_note_before() {
        let mut arp = arp(&[0, 7], 2, 2, -100);
        let sent = run(&mut arp, 2);
        assert_eq!(sent[1], vec![on(67)]);
        assert_eq!(sent[2], vec![off(60)]);
    }

    #[test]
    fn releasing_the_root_stops_the_arp() {
        let mut arp = arp(&[0, 7], 2, 2, 0);
        run(&mut arp, 2);
        let mut out = vec![];
        arp.process(off(60), Some(Timing::new(2)), &mut out);
        assert_eq!(out, vec![off(67)]);
        for pulse in 3..10 {
            run_pulse(&mut arp, pulse, &mut out);
        }
        assert_eq!(out, vec![off(67)]);
    }

    #[test]
    fn a_step_due_as_the_root_is_played_again_is_not_heard() {
        let mut arp = arp(&[0, 7], 2, 2, 0);
        run(&mut arp, 1);
        let timing = Timing::new(2);
        let mut out = vec![];
        arp.tick(timing, &mut out);
        arp.process(off(60), Some(timing), &mut out);
        arp.process(on(60), Some(timing), &mut out);
        arp.after_pulse(timing, &mut out);
        assert_eq!(out, vec![off(60), on(60)]);
    }

    #[test]
    fn notes_pass_through_while_stopped() {
        let mut arp = arp(&[0, 7], 2, 2, 0);
        let mut out = vec![];
        arp.process(on(60), None, &mut out);
        arp.process(off(60), None, &mut out);
        assert_eq!(out, vec![on(60), off(60)]);
    }

    #[test]
    fn steps_out_of_range_are_rests() {
        let mut arp = arp(&[0, 100], 2, 1, 0);
        let sent = run(&mut arp, 2);
        assert_eq!(sent[1], vec![off(60)]);
        assert_eq!(sent[2], vec![on(60)]);
    }

    #[test]
    fn a_shorter_length_takes_effect_on_the_next_step() {
        let mut arp = arp(&[0, 7, 3], 3, 1, 0);
        let mut sent = run(&mut arp, 1);
        arp.set_param(LENGTH_PARAM, 1);
        for pulse in 2..=3 {
            let mut out = vec![];
            run_pulse(&mut arp, pulse, &mut out);
            sent.push(out);
        }
        assert_eq!(
            sent[1..],
            [
                vec![off(60), on(67)],
                vec![off(67), on(60)],
                vec![off(60), on(60)],
            ]
        );
    }

    #[test]
    fn a_new_rate_starts_after_the_step_already_waiting() {
        let mut arp = arp(&[0, 7], 2, 4, 0);
        let mut sent = run(&mut arp, 0);
        arp.set_param(RATE_PARAM, 1);
        for pulse in 1..=5 {
            let mut out = vec![];
            run_pulse(&mut arp, pulse, &mut out);
            sent.push(out);
        }
        assert!(sent[1..4].iter().all(Vec::is_empty));
        assert_eq!(sent[4], vec![off(60), on(67)]);
        assert_eq!(sent[5], vec![off(67), on(60)]);
    }

    #[test]
    fn a_pitch_two_arps_share_is_released_by_the_last_one() {
        let mut arp = arp(&[0, 7], 2, 2, 0);
        run(&mut arp, 2);
        let timing = Some(Timing::new(2));
        let mut out = vec![];
        arp.process(on(67), timing, &mut out);
        assert_eq!(out, vec![on(67)]);
        out.clear();
        arp.process(off(67), timing, &mut out);
        assert!(out.is_empty());
        arp.process(off(60), timing, &mut out);
        assert_eq!(out, vec![off(67)]);
    }

    #[test]
    fn a_note_on_without_velocity_stops_the_arp() {
        let mut arp = arp(&[0, 7], 2, 2, 0);
        run(&mut arp, 2);
        let mut out = vec![];
        let release = Message::NoteOn(Channel::Ch1, 60, 0);
        arp.process(release, Some(Timing::new(2)), &mut out);
        assert_eq!(out, vec![off(67)]);
        assert!(arp.arps.is_empty());
    }

    #[test]
    fn roots_on_other_channels_have_their_own_arps() {
        let mut arp = arp(&[0, 7], 2, 2, 0);
        run(&mut arp, 0);
        let mut out = vec![];
        let other = Message::NoteOn(Channel::Ch2, 60, 100);
        arp.process(other, Some(Timing::new(0)), &mut out);
        out.clear();
        arp.process(off(60), Some(Timing::new(0)), &mut out);
        assert_eq!(out, vec![off(60)]);
        out.clear();
        run_pulse(&mut arp, 1, &mut out);
        run_pulse(&mut arp, 2, &mut out);
        assert_eq!(
            out,
            vec![
                Message::NoteOff(Channel::Ch2, 60, 0),
                Message::NoteOn(Channel::Ch2, 67, 100),
            ]
        );
    }

    #[test]
    fn reset_releases_what_is_sounding() {
        let mut arp = arp(&[0, 7], 2, 4, 2);
        run(&mut arp, 4);
        let mut out = vec![];
        arp.reset(&mut out);
        assert_eq!(out, vec![off(60), off(67)]);
    }
}
