use midi::Message;

use crate::{
    data::midi::scale_velocity,
    engine::{
        effects::{MidiEffect, Param, ParamKind, Timing, ValueLabel, note_event},
        notes::{Countdown, MIDI_CHANNEL_COUNT, PITCH_COUNT},
    },
};

const GRIDS: &[&str] = &["1/4", "1/8", "1/16", "1/32"];
const GRID_PULSES: [u32; 4] = [24, 12, 6, 3];
/// 50%, where nothing moves.
const STRAIGHT: u8 = 0x80;

pub const GRID: usize = 0;
pub const AMOUNT: usize = 1;
pub const ACCENT: usize = 2;

const PARAMS: &[Param] = &[
    Param {
        name: "Grid",
        kind: ParamKind::Choice(GRIDS),
        default: 2,
        label: ValueLabel::None,
    },
    Param {
        name: "Swing",
        kind: ParamKind::Number {
            min: STRAIGHT,
            max: 0xFC,
        },
        default: 0xAA,
        label: ValueLabel::Percent,
    },
    Param {
        name: "Accent",
        kind: ParamKind::Number {
            min: 0,
            max: u8::MAX,
        },
        default: STRAIGHT,
        label: ValueLabel::Percent,
    },
];

/// 50% is straight. Accent below 50% quietens the swung notes, above it the ones on the beat.
pub struct SwingEffect {
    grid: u8,
    amount: u8,
    accent: u8,
    waiting: Countdown<Message>,
    due: Vec<Message>,
    /// So a late note also ends late and keeps its length.
    delays: [[u32; PITCH_COUNT]; MIDI_CHANNEL_COUNT],
}
impl SwingEffect {
    pub fn new() -> Self {
        return Self {
            grid: PARAMS[GRID].default,
            amount: PARAMS[AMOUNT].default,
            accent: PARAMS[ACCENT].default,
            waiting: Countdown::new(),
            due: Vec::new(),
            delays: [[0; PITCH_COUNT]; MIDI_CHANNEL_COUNT],
        };
    }

    fn step(&self) -> u32 {
        return GRID_PULSES[self.grid as usize];
    }

    fn delay(&self) -> u32 {
        let pair = self.step() * 2;
        return (pair * self.amount as u32 + 0x80)
            .div_euclid(u8::MAX as u32 + 1)
            .saturating_sub(self.step());
    }

    fn accented(&self, velocity: u8, swung: bool) -> u8 {
        let scale = match swung {
            true => self.accent.min(STRAIGHT),
            false => (u8::MAX as u16 + 1 - self.accent as u16).min(STRAIGHT as u16) as u8,
        };
        return scale_velocity(velocity, scale, STRAIGHT).max(1);
    }
}
impl Default for SwingEffect {
    fn default() -> Self {
        return Self::new();
    }
}
impl MidiEffect for SwingEffect {
    fn params(&self) -> &'static [Param] {
        return PARAMS;
    }
    fn param(&self, index: usize) -> u8 {
        return match index {
            GRID => self.grid,
            AMOUNT => self.amount,
            ACCENT => self.accent,
            _ => 0,
        };
    }
    fn set_param(&mut self, index: usize, value: u8) {
        match index {
            GRID => self.grid = value,
            AMOUNT => self.amount = value,
            ACCENT => self.accent = value,
            _ => {}
        }
    }
    fn process(&mut self, message: Message, timing: Option<Timing>, out: &mut Vec<Message>) {
        let (Some(mut note), Some(timing)) = (note_event(&message), timing) else {
            out.push(message);
            return;
        };
        let step = self.step();
        let on_grid = timing.pulse % step == 0;
        let swung = on_grid && timing.pulse % (step * 2) == step;
        let late = if swung { self.delay() } else { 0 };
        let delay = &mut self.delays[note.channel as usize][note.pitch.0 as usize];
        let wait = match note.is_note_off {
            true => std::mem::take(delay),
            false => {
                *delay = late;
                if on_grid {
                    note.velocity = self.accented(note.velocity, swung);
                }
                late
            }
        };
        match wait {
            0 => out.push(note.to_message()),
            wait => self.waiting.push(note.to_message(), wait),
        }
    }
    fn tick(&mut self, _timing: Timing, out: &mut Vec<Message>) {
        for message in self.waiting.tick() {
            match note_event(&message).is_some_and(|note| note.is_note_off) {
                true => out.push(message),
                false => self.due.push(message),
            }
        }
    }
    fn after_pulse(&mut self, _timing: Timing, out: &mut Vec<Message>) {
        out.append(&mut self.due);
    }
    fn reset(&mut self, out: &mut Vec<Message>) {
        let is_note_off = |m: &Message| note_event(m).is_some_and(|note| note.is_note_off);
        out.extend(self.waiting.remove(is_note_off));
        self.waiting.clear();
        self.due.clear();
        self.delays = [[0; PITCH_COUNT]; MIDI_CHANNEL_COUNT];
    }
}

#[cfg(test)]
mod tests {
    use midi::Channel;

    use super::*;

    fn note_on(pitch: u8, velocity: u8) -> Message {
        return Message::NoteOn(Channel::Ch1, pitch, velocity);
    }
    fn note_off(pitch: u8) -> Message {
        return Message::NoteOff(Channel::Ch1, pitch, 0);
    }

    fn run(swing: &mut SwingEffect, messages: &[(u32, Message)], last: u32) -> Vec<(u32, Message)> {
        let mut sent = vec![];
        for pulse in 0..=last {
            let timing = Timing::new(pulse);
            let mut out = vec![];
            swing.tick(timing, &mut out);
            for (_, message) in messages.iter().filter(|(at, _)| *at == pulse) {
                swing.process(message.clone(), Some(timing), &mut out);
            }
            swing.after_pulse(timing, &mut out);
            sent.extend(out.into_iter().map(|m| (pulse, m)));
        }
        return sent;
    }

    #[test]
    fn triplet_swing_moves_the_off_beat_two_ticks_and_keeps_its_length() {
        let mut swing = SwingEffect::new();
        let messages = [
            (0, note_on(60, 100)),
            (3, note_off(60)),
            (6, note_on(62, 100)),
            (9, note_off(62)),
        ];
        assert_eq!(
            run(&mut swing, &messages, 12),
            vec![
                (0, note_on(60, 100)),
                (3, note_off(60)),
                (8, note_on(62, 100)),
                (11, note_off(62)),
            ]
        );
    }

    #[test]
    fn fifty_percent_is_straight() {
        let mut swing = SwingEffect::new();
        swing.amount = STRAIGHT;
        assert_eq!(swing.delay(), 0);
    }

    #[test]
    fn accent_below_half_quietens_the_swung_notes() {
        let mut swing = SwingEffect::new();
        swing.accent = 0x40;
        let sent = run(
            &mut swing,
            &[(0, note_on(60, 100)), (6, note_on(62, 100))],
            12,
        );
        assert_eq!(sent, vec![(0, note_on(60, 100)), (8, note_on(62, 50))]);
    }

    #[test]
    fn notes_pass_unchanged_while_stopped() {
        let mut swing = SwingEffect::new();
        let mut out = vec![];
        swing.process(note_on(60, 100), None, &mut out);
        assert_eq!(out, vec![note_on(60, 100)]);
    }

    #[test]
    fn reset_releases_notes_whose_note_off_is_waiting() {
        let mut swing = SwingEffect::new();
        run(&mut swing, &[(6, note_on(62, 100)), (10, note_off(62))], 10);
        let mut out = vec![];
        swing.reset(&mut out);
        assert_eq!(out, vec![note_off(62)]);
    }
}
