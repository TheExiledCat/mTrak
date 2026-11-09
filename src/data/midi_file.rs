//! Standard MIDI files, format 1: a track with the song's name and tempo, then a track per
//! channel.

use midi::{Message, ToRawMessages};

use crate::data::midi::{PULSES_PER_QUARTER, raw_message_to_bytes};

const MICROSECONDS_PER_MINUTE: u32 = 60_000_000;

/// A channel's messages, in pulses from the start, and the name its track gets.
pub struct ChannelTrack {
    pub name: String,
    pub messages: Vec<(u32, Message)>,
}

/// Pulses are MIDI clock pulses, so the file counts `PULSES_PER_QUARTER` ticks to a beat.
pub fn write(name: &str, tempos: &[(u32, u8)], channels: &[ChannelTrack]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"MThd");
    bytes.extend_from_slice(&6u32.to_be_bytes());
    bytes.extend_from_slice(&1u16.to_be_bytes());
    bytes.extend_from_slice(&(channels.len() as u16 + 1).to_be_bytes());
    bytes.extend_from_slice(&(PULSES_PER_QUARTER as u16).to_be_bytes());

    let mut events = vec![(0, track_name(name))];
    for &(pulse, bpm) in tempos {
        let tempo = MICROSECONDS_PER_MINUTE / bpm.max(1) as u32;
        let mut event = vec![0xFF, 0x51, 0x03];
        event.extend_from_slice(&tempo.to_be_bytes()[1..]);
        events.push((pulse, event));
    }
    write_track(&mut bytes, events);
    for channel in channels {
        let mut events = vec![(0, track_name(&channel.name))];
        events.extend(channel.messages.iter().map(|(pulse, message)| {
            let raw = message.to_raw_messages().into_iter();
            (*pulse, raw.flat_map(raw_message_to_bytes).collect())
        }));
        write_track(&mut bytes, events);
    }
    return bytes;
}

fn track_name(name: &str) -> Vec<u8> {
    let mut event = vec![0xFF, 0x03];
    write_variable(&mut event, name.len() as u32);
    event.extend_from_slice(name.as_bytes());
    return event;
}

/// `events` must be in order of time.
fn write_track(bytes: &mut Vec<u8>, events: Vec<(u32, Vec<u8>)>) {
    let mut track = Vec::new();
    let mut time = 0;
    for (pulse, event) in events {
        write_variable(&mut track, pulse - time);
        track.extend_from_slice(&event);
        time = pulse;
    }
    write_variable(&mut track, 0);
    track.extend_from_slice(&[0xFF, 0x2F, 0x00]);
    bytes.extend_from_slice(b"MTrk");
    bytes.extend_from_slice(&(track.len() as u32).to_be_bytes());
    bytes.extend_from_slice(&track);
}

/// Seven bits a byte, highest first, with the top bit set on all but the last.
fn write_variable(bytes: &mut Vec<u8>, value: u32) {
    let mut groups = vec![(value & 0x7F) as u8];
    let mut rest = value >> 7;
    while rest > 0 {
        groups.push((rest & 0x7F) as u8 | 0x80);
        rest >>= 7;
    }
    bytes.extend(groups.into_iter().rev());
}

#[cfg(test)]
mod tests {
    use midi::Channel;

    use super::*;

    #[test]
    fn variable_lengths_match_the_midi_file_spec() {
        let encoded = |value| {
            let mut bytes = vec![];
            write_variable(&mut bytes, value);
            return bytes;
        };
        assert_eq!(encoded(0), [0x00]);
        assert_eq!(encoded(0x7F), [0x7F]);
        assert_eq!(encoded(0x80), [0x81, 0x00]);
        assert_eq!(encoded(0x3FFF), [0xFF, 0x7F]);
        assert_eq!(encoded(0x0FFF_FFFF), [0xFF, 0xFF, 0xFF, 0x7F]);
    }

    #[test]
    fn a_file_has_a_tempo_track_and_a_track_per_channel() {
        let channel = ChannelTrack {
            name: "Bass".into(),
            messages: vec![
                (0, Message::NoteOn(Channel::Ch2, 60, 100)),
                (24, Message::NoteOff(Channel::Ch2, 60, 0)),
            ],
        };
        let bytes = write("Song", &[(0, 120)], &[channel]);
        assert_eq!(&bytes[..14], b"MThd\0\0\0\x06\0\x01\0\x02\0\x18");
        let tempo = &bytes[14..];
        assert_eq!(&tempo[..4], b"MTrk");
        let tempo_track = [
            0x00, 0xFF, 0x03, 4, b'S', b'o', b'n', b'g', // name
            0x00, 0xFF, 0x51, 0x03, 0x07, 0xA1, 0x20, // 500000 us a beat, 120 BPM
            0x00, 0xFF, 0x2F, 0x00,
        ];
        assert_eq!(tempo[4..8], (tempo_track.len() as u32).to_be_bytes());
        assert_eq!(tempo[8..8 + tempo_track.len()], tempo_track);
        let notes = &tempo[8 + tempo_track.len()..];
        let notes_track = [
            0x00, 0xFF, 0x03, 4, b'B', b'a', b's', b's', //
            0x00, 0x91, 60, 100, //
            0x18, 0x81, 60, 0, //
            0x00, 0xFF, 0x2F, 0x00,
        ];
        assert_eq!(&notes[..4], b"MTrk");
        assert_eq!(notes[8..], notes_track);
    }
}
