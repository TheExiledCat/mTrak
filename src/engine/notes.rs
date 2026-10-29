use midi::{Channel, Message};
use num::FromPrimitive;

use crate::data::midi::{MIDI_CHANNEL_MAX, PITCH_MAX};

pub const PITCH_COUNT: usize = PITCH_MAX as usize + 1;
pub const MIDI_CHANNEL_COUNT: usize = MIDI_CHANNEL_MAX as usize + 1;

/// How many times each pitch is held on each channel, so a pitch played twice is only
/// released once both let go of it.
pub struct HeldNotes {
    counts: [[u8; PITCH_COUNT]; MIDI_CHANNEL_COUNT],
}

impl Default for HeldNotes {
    fn default() -> Self {
        return Self::new();
    }
}

impl HeldNotes {
    pub fn new() -> Self {
        return Self {
            counts: [[0; PITCH_COUNT]; MIDI_CHANNEL_COUNT],
        };
    }

    /// Always sends the note on, so a pitch that's already held is played again.
    pub fn note_on(&mut self, channel: Channel, pitch: u8, velocity: u8, out: &mut Vec<Message>) {
        let Some(count) = self.count(channel, pitch) else {
            return;
        };
        *count = count.saturating_add(1);
        out.push(Message::NoteOn(channel, pitch, velocity));
    }

    /// The note off is only sent once nothing else holds the pitch.
    pub fn note_off(&mut self, channel: Channel, pitch: u8, velocity: u8, out: &mut Vec<Message>) {
        let Some(count) = self.count(channel, pitch) else {
            return;
        };
        if *count == 0 {
            return;
        }
        *count -= 1;
        if *count == 0 {
            out.push(Message::NoteOff(channel, pitch, velocity));
        }
    }

    /// Releases every held pitch, however many times it's held.
    pub fn release_all(&mut self, out: &mut Vec<Message>) {
        for (channel, counts) in self.counts.iter_mut().enumerate() {
            for (pitch, count) in counts.iter_mut().enumerate() {
                if *count > 0 {
                    *count = 0;
                    let channel = Channel::from_usize(channel).unwrap();
                    out.push(Message::NoteOff(channel, pitch as u8, 0));
                }
            }
        }
    }

    fn count(&mut self, channel: Channel, pitch: u8) -> Option<&mut u8> {
        return self.counts[channel as usize].get_mut(pitch as usize);
    }
}

/// Things waiting a number of ticks, handed back by `tick` once their time is up.
pub struct Countdown<T> {
    waiting: Vec<(T, u32)>,
}

impl<T> Default for Countdown<T> {
    fn default() -> Self {
        return Self::new();
    }
}

impl<T> Countdown<T> {
    pub fn new() -> Self {
        return Self {
            waiting: Vec::new(),
        };
    }

    /// Waits `ticks` ticks; 0 counts as 1, as nothing is handed back before the next tick.
    pub fn push(&mut self, item: T, ticks: u32) {
        self.waiting.push((item, ticks.max(1)));
    }

    /// Counts down a tick and returns what's due, in the order it was pushed.
    pub fn tick(&mut self) -> Vec<T> {
        for (_, ticks) in &mut self.waiting {
            *ticks -= 1;
        }
        return self.take(|_| true, |ticks| ticks == 0);
    }

    /// Stops waiting for the items `matches` picks and returns them.
    pub fn remove(&mut self, matches: impl Fn(&T) -> bool) -> Vec<T> {
        return self.take(matches, |_| true);
    }

    pub fn clear(&mut self) {
        self.waiting.clear();
    }

    fn take(&mut self, matches: impl Fn(&T) -> bool, due: impl Fn(u32) -> bool) -> Vec<T> {
        let (taken, waiting) = std::mem::take(&mut self.waiting)
            .into_iter()
            .partition(|(item, ticks)| matches(item) && due(*ticks));
        self.waiting = waiting;
        return taken.into_iter().map(|(item, _)| item).collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pitch_held_twice_is_released_once_both_let_go() {
        let mut held = HeldNotes::new();
        let mut out = vec![];
        held.note_on(Channel::Ch1, 60, 100, &mut out);
        held.note_on(Channel::Ch1, 60, 90, &mut out);
        assert_eq!(out.len(), 2);
        out.clear();
        held.note_off(Channel::Ch1, 60, 0, &mut out);
        assert!(out.is_empty());
        held.note_off(Channel::Ch1, 60, 0, &mut out);
        assert_eq!(out, vec![Message::NoteOff(Channel::Ch1, 60, 0)]);
    }

    #[test]
    fn releasing_what_is_not_held_sends_nothing() {
        let mut held = HeldNotes::new();
        let mut out = vec![];
        held.note_off(Channel::Ch1, 60, 0, &mut out);
        held.note_on(Channel::Ch1, 200, 100, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn release_all_lets_go_of_every_pitch_once() {
        let mut held = HeldNotes::new();
        let mut out = vec![];
        held.note_on(Channel::Ch2, 64, 100, &mut out);
        held.note_on(Channel::Ch1, 60, 100, &mut out);
        held.note_on(Channel::Ch1, 60, 100, &mut out);
        out.clear();
        held.release_all(&mut out);
        assert_eq!(
            out,
            vec![
                Message::NoteOff(Channel::Ch1, 60, 0),
                Message::NoteOff(Channel::Ch2, 64, 0),
            ]
        );
        out.clear();
        held.note_off(Channel::Ch1, 60, 0, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn countdown_hands_items_back_when_due() {
        let mut countdown = Countdown::new();
        countdown.push('a', 2);
        countdown.push('b', 1);
        countdown.push('c', 0);
        assert_eq!(countdown.tick(), vec!['b', 'c']);
        assert_eq!(countdown.tick(), vec!['a']);
        assert!(countdown.tick().is_empty());
    }

    #[test]
    fn removed_items_are_never_due() {
        let mut countdown = Countdown::new();
        countdown.push(1, 1);
        countdown.push(2, 1);
        assert_eq!(countdown.remove(|item| *item == 1), vec![1]);
        assert_eq!(countdown.tick(), vec![2]);
    }
}
