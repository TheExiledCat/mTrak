use midi::{Channel, Message};
use num::FromPrimitive;

use crate::{data::midi::MIDI_CHANNEL_MAX, tui::constants::TRACK_COUNT};

const PITCH_COUNT: usize = 128;
const MIDI_CHANNEL_COUNT: usize = MIDI_CHANNEL_MAX as usize + 1;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Voice {
    channel: u8,
    pitch: u8,
}

/// Tracks which note each track is playing, so note offs reach the right channel and pitch.
pub struct Voices {
    tracks: [Option<Voice>; TRACK_COUNT as usize],
    held: [[u8; PITCH_COUNT]; MIDI_CHANNEL_COUNT],
    channel_volumes: [u8; MIDI_CHANNEL_COUNT],
}

impl Voices {
    pub fn new() -> Self {
        return Self {
            tracks: [None; TRACK_COUNT as usize],
            held: [[0; PITCH_COUNT]; MIDI_CHANNEL_COUNT],
            channel_volumes: [0xFF; _],
        };
    }

    /// Releases the track's current note, then starts the new one.
    pub fn note_on(
        &mut self,
        track: usize,
        channel: u8,
        pitch: u8,
        velocity: u8,
        out: &mut Vec<Message>,
    ) {
        if track >= self.tracks.len() {
            return;
        }
        let channel = channel.min(MIDI_CHANNEL_MAX);
        let pitch = pitch.min(PITCH_COUNT as u8 - 1);
        let velocity = (velocity as f32
            * (self.channel_volumes[channel as usize] as f32 / u8::MAX as f32))
            .ceil() as u8; // global volume works as velocity multiplier.
        self.release(track, out);
        let held = &mut self.held[channel as usize][pitch as usize];
        *held = held.saturating_add(1);
        self.tracks[track] = Some(Voice { channel, pitch });
        out.push(Message::NoteOn(to_channel(channel), pitch, velocity));
    }

    /// Stops the track's note; the note off is only sent once no other track holds that pitch.
    pub fn release(&mut self, track: usize, out: &mut Vec<Message>) {
        let Some(voice) = self.tracks.get_mut(track).and_then(Option::take) else {
            return;
        };
        let held = &mut self.held[voice.channel as usize][voice.pitch as usize];
        *held = held.saturating_sub(1);
        if *held == 0 {
            out.push(Message::NoteOff(to_channel(voice.channel), voice.pitch, 0));
        }
    }

    /// Releases every track playing on a muted channel.
    pub fn release_channels(&mut self, muted: &[bool], out: &mut Vec<Message>) {
        for track in 0..self.tracks.len() {
            let Some(voice) = self.tracks[track] else {
                continue;
            };
            if muted.get(voice.channel as usize).copied().unwrap_or(false) {
                self.release(track, out);
            }
        }
    }

    /// Releases every track.
    pub fn release_all(&mut self, out: &mut Vec<Message>) {
        for track in 0..self.tracks.len() {
            self.release(track, out);
        }
    }

    pub fn set_global_volume(&mut self, channel: usize, volume: u8) {
        self.channel_volumes[channel.min(MIDI_CHANNEL_MAX as usize)] = volume;
    }
}

fn to_channel(channel: u8) -> Channel {
    return Channel::from_u8(channel).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note_offs(messages: &[Message]) -> usize {
        return messages
            .iter()
            .filter(|m| matches!(m, Message::NoteOff(..)))
            .count();
    }

    #[test]
    fn new_note_releases_previous_note_first() {
        let mut voices = Voices::new();
        let mut out = vec![];
        voices.note_on(0, 0, 60, 100, &mut out);
        out.clear();
        voices.note_on(0, 0, 62, 100, &mut out);
        assert_eq!(
            out,
            vec![
                Message::NoteOff(Channel::Ch1, 60, 0),
                Message::NoteOn(Channel::Ch1, 62, 100),
            ]
        );
    }

    #[test]
    fn retriggering_same_pitch_sends_off_before_on() {
        let mut voices = Voices::new();
        let mut out = vec![];
        voices.note_on(0, 0, 60, 100, &mut out);
        out.clear();
        voices.note_on(0, 0, 60, 100, &mut out);
        assert_eq!(
            out,
            vec![
                Message::NoteOff(Channel::Ch1, 60, 0),
                Message::NoteOn(Channel::Ch1, 60, 100),
            ]
        );
    }

    #[test]
    fn shared_pitch_is_only_released_by_the_last_track() {
        let mut voices = Voices::new();
        let mut out = vec![];
        voices.note_on(0, 3, 60, 100, &mut out);
        voices.note_on(1, 3, 60, 100, &mut out);
        out.clear();
        voices.release(0, &mut out);
        assert_eq!(note_offs(&out), 0);
        voices.release(1, &mut out);
        assert_eq!(out, vec![Message::NoteOff(Channel::Ch4, 60, 0)]);
    }

    #[test]
    fn release_uses_the_channel_the_note_was_played_on() {
        let mut voices = Voices::new();
        let mut out = vec![];
        voices.note_on(2, 5, 48, 100, &mut out);
        out.clear();
        voices.note_on(2, 9, 48, 100, &mut out);
        assert_eq!(out[0], Message::NoteOff(Channel::Ch6, 48, 0));
    }

    #[test]
    fn release_channels_only_cuts_muted_channels() {
        let mut voices = Voices::new();
        let mut out = vec![];
        voices.note_on(0, 0, 60, 100, &mut out);
        voices.note_on(1, 1, 60, 100, &mut out);
        out.clear();
        let mut muted = [false; MIDI_CHANNEL_COUNT];
        muted[1] = true;
        voices.release_channels(&muted, &mut out);
        assert_eq!(out, vec![Message::NoteOff(Channel::Ch2, 60, 0)]);
    }

    #[test]
    fn releasing_an_idle_track_sends_nothing() {
        let mut voices = Voices::new();
        let mut out = vec![];
        voices.release(4, &mut out);
        voices.release_all(&mut out);
        assert!(out.is_empty());
    }
}
