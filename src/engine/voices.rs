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

/// A note replaced by a legato note that keeps sounding for a few more ticks.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Lingering {
    voice: Voice,
    ticks: u8,
}

/// Tracks which note each track is playing, so note offs reach the right channel and pitch.
pub struct Voices {
    tracks: [Option<Voice>; TRACK_COUNT as usize],
    lingering: [Option<Lingering>; TRACK_COUNT as usize],
    held: [[u8; PITCH_COUNT]; MIDI_CHANNEL_COUNT],
    channel_volumes: [u8; MIDI_CHANNEL_COUNT],
    velocity: u8,
}

impl Default for Voices {
    fn default() -> Self {
        return Self::new();
    }
}

impl Voices {
    pub fn new() -> Self {
        return Self {
            tracks: [None; TRACK_COUNT as usize],
            lingering: [None; TRACK_COUNT as usize],
            held: [[0; PITCH_COUNT]; MIDI_CHANNEL_COUNT],
            channel_volumes: [0xFF; MIDI_CHANNEL_COUNT],
            velocity: 0xFF,
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
        self.release(track, out);
        self.start(track, channel, pitch, velocity, out);
    }

    /// Starts the new note before releasing the track's current one, so a synth in legato
    /// mode glides between them. The old note keeps sounding for `overlap` more ticks.
    pub fn legato_on(
        &mut self,
        track: usize,
        channel: u8,
        pitch: u8,
        velocity: u8,
        overlap: u8,
        out: &mut Vec<Message>,
    ) {
        if track >= self.tracks.len() {
            return;
        }
        let previous = self.tracks[track].take();
        self.start(track, channel, pitch, velocity, out);
        let Some(previous) = previous else {
            return;
        };
        if overlap == 0 {
            self.note_off(previous, out);
            return;
        }
        if let Some(lingering) = self.lingering[track].take() {
            self.note_off(lingering.voice, out);
        }
        self.lingering[track] = Some(Lingering {
            voice: previous,
            ticks: overlap,
        });
    }

    fn start(
        &mut self,
        track: usize,
        channel: u8,
        pitch: u8,
        velocity: u8,
        out: &mut Vec<Message>,
    ) {
        let channel = channel.min(MIDI_CHANNEL_MAX);
        let pitch = pitch.min(PITCH_COUNT as u8 - 1);
        let velocity = (velocity as f32
            * (self.channel_volumes[channel as usize] as f32 / u8::MAX as f32)
            * (self.velocity as f32 / u8::MAX as f32))
            .ceil() as u8; // global volume and velocity work as velocity multipliers.
        let held = &mut self.held[channel as usize][pitch as usize];
        *held = held.saturating_add(1);
        self.tracks[track] = Some(Voice { channel, pitch });
        out.push(Message::NoteOn(to_channel(channel), pitch, velocity));
    }

    /// Stops the track's note; the note off is only sent once no other track holds that pitch.
    pub fn release(&mut self, track: usize, out: &mut Vec<Message>) {
        if let Some(voice) = self.tracks.get_mut(track).and_then(Option::take) {
            self.note_off(voice, out);
        }
    }

    /// Stops the track's note and any note still ringing after a legato note, straight away.
    pub fn cut(&mut self, track: usize, out: &mut Vec<Message>) {
        if let Some(voice) = self.tracks.get_mut(track).and_then(Option::take) {
            self.note_off(voice, out);
        }
        if let Some(lingering) = self.lingering.get_mut(track).and_then(Option::take) {
            self.note_off(lingering.voice, out);
        }
    }

    /// The note off is only sent once no other track holds that pitch.
    fn note_off(&mut self, voice: Voice, out: &mut Vec<Message>) {
        let held = &mut self.held[voice.channel as usize][voice.pitch as usize];
        *held = held.saturating_sub(1);
        if *held == 0 {
            out.push(Message::NoteOff(to_channel(voice.channel), voice.pitch, 0));
        }
    }

    /// Counts down notes ringing after a legato note and releases the ones that are done.
    pub fn tick(&mut self, out: &mut Vec<Message>) {
        for track in 0..self.lingering.len() {
            let Some(lingering) = &mut self.lingering[track] else {
                continue;
            };
            lingering.ticks = lingering.ticks.saturating_sub(1);
            if lingering.ticks == 0 {
                let voice = lingering.voice;
                self.lingering[track] = None;
                self.note_off(voice, out);
            }
        }
    }

    /// Stops the track's note only if it is still playing `pitch`.
    pub fn release_pitch(&mut self, track: usize, pitch: u8, out: &mut Vec<Message>) {
        if self
            .tracks
            .get(track)
            .copied()
            .flatten()
            .is_some_and(|voice| voice.pitch == pitch)
        {
            self.release(track, out);
        }
    }

    /// Cuts every note playing on a muted channel, overlapping ones included.
    pub fn release_channels(&mut self, muted: &[bool], out: &mut Vec<Message>) {
        let is_muted = |voice: &Voice| muted.get(voice.channel as usize).copied().unwrap_or(false);
        for track in 0..self.tracks.len() {
            if let Some(voice) = self.tracks[track]
                && is_muted(&voice)
            {
                self.tracks[track] = None;
                self.note_off(voice, out);
            }
            if let Some(lingering) = self.lingering[track]
                && is_muted(&lingering.voice)
            {
                self.lingering[track] = None;
                self.note_off(lingering.voice, out);
            }
        }
    }

    /// Cuts every note, overlapping ones included.
    pub fn release_all(&mut self, out: &mut Vec<Message>) {
        for track in 0..self.tracks.len() {
            self.cut(track, out);
        }
    }

    pub fn set_global_volume(&mut self, channel: usize, volume: u8) {
        self.channel_volumes[channel.min(MIDI_CHANNEL_MAX as usize)] = volume;
    }

    pub fn set_global_velocity(&mut self, velocity: u8) {
        self.velocity = velocity;
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
    fn legato_starts_the_new_note_before_releasing_the_old_one() {
        let mut voices = Voices::new();
        let mut out = vec![];
        voices.note_on(0, 0, 60, 100, &mut out);
        out.clear();
        voices.legato_on(0, 0, 62, 100, 0, &mut out);
        assert_eq!(
            out,
            vec![
                Message::NoteOn(Channel::Ch1, 62, 100),
                Message::NoteOff(Channel::Ch1, 60, 0),
            ]
        );
    }

    #[test]
    fn legato_overlap_keeps_the_old_note_ringing() {
        let mut voices = Voices::new();
        let mut out = vec![];
        voices.note_on(0, 0, 60, 100, &mut out);
        out.clear();
        voices.legato_on(0, 0, 62, 100, 2, &mut out);
        assert_eq!(out, vec![Message::NoteOn(Channel::Ch1, 62, 100)]);
        out.clear();
        voices.tick(&mut out);
        assert!(out.is_empty());
        voices.tick(&mut out);
        assert_eq!(out, vec![Message::NoteOff(Channel::Ch1, 60, 0)]);
    }

    #[test]
    fn legato_on_the_same_pitch_is_released_once_both_are_done() {
        let mut voices = Voices::new();
        let mut out = vec![];
        voices.note_on(0, 0, 60, 100, &mut out);
        voices.legato_on(0, 0, 60, 100, 1, &mut out);
        out.clear();
        voices.tick(&mut out);
        assert!(out.is_empty());
        voices.release(0, &mut out);
        assert_eq!(out, vec![Message::NoteOff(Channel::Ch1, 60, 0)]);
    }

    #[test]
    fn release_all_cuts_ringing_notes() {
        let mut voices = Voices::new();
        let mut out = vec![];
        voices.note_on(0, 0, 60, 100, &mut out);
        voices.legato_on(0, 0, 62, 100, 4, &mut out);
        out.clear();
        voices.release_all(&mut out);
        assert_eq!(note_offs(&out), 2);
        out.clear();
        voices.tick(&mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn muting_cuts_ringing_notes() {
        let mut voices = Voices::new();
        let mut out = vec![];
        voices.note_on(0, 0, 60, 100, &mut out);
        voices.legato_on(0, 0, 62, 100, 4, &mut out);
        out.clear();
        let mut muted = [false; 16];
        muted[0] = true;
        voices.release_channels(&muted, &mut out);
        assert_eq!(note_offs(&out), 2);
    }

    #[test]
    fn release_pitch_ignores_a_replaced_note() {
        let mut voices = Voices::new();
        let mut out = vec![];
        voices.note_on(0, 0, 60, 100, &mut out);
        voices.note_on(0, 0, 62, 100, &mut out);
        out.clear();
        voices.release_pitch(0, 60, &mut out);
        assert!(out.is_empty());
        voices.release_pitch(0, 62, &mut out);
        assert_eq!(out, vec![Message::NoteOff(Channel::Ch1, 62, 0)]);
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
    fn global_velocity_scales_with_channel_volume() {
        let mut voices = Voices::new();
        let mut out = vec![];
        voices.set_global_volume(0, 0x80);
        voices.set_global_velocity(0x80);
        voices.note_on(0, 0, 60, 100, &mut out);
        assert_eq!(out, vec![Message::NoteOn(Channel::Ch1, 60, 26)]);
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
