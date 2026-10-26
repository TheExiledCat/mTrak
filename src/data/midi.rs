use std::time::Duration;

use midi::{Channel, Message, RawMessage, utils::msb_lsb_to_u14};
use num::FromPrimitive;

pub const MIDI_CHANNEL_MAX: u8 = 0xF;
pub const PULSES_PER_QUARTER: u32 = 24;
pub const VELOCITY_MAX: u8 = 0x7F;
pub const VOLUME_MAX: u8 = 0x80;
pub const DEFAULT_BPM: u8 = 125;
pub const DEFAULT_SPEED: u8 = 6;
/// The highest speed, as in FastTracker 2; `F` values above it set the BPM instead.
pub const MAX_SPEED: u8 = 0x1F;

pub fn pulse_duration(bpm: u8) -> Duration {
    return Duration::from_secs_f64(60.0 / (bpm.max(1) as f64 * PULSES_PER_QUARTER as f64));
}
#[derive(Clone)]
pub enum MidiEvent {
    Message(Message),
    NotesOffAll,
    Aggregate(Vec<MidiEvent>),
}
impl MidiEvent {
    pub fn into_messages(self) -> Vec<Message> {
        let mut messages: Vec<Message> = vec![];

        match self {
            MidiEvent::Message(message) => messages.push(message),
            MidiEvent::NotesOffAll => messages.append(
                &mut (0..=MIDI_CHANNEL_MAX)
                    .map(|c| Message::AllSoundOff(Channel::from_u8(c).unwrap()))
                    .collect::<Vec<Message>>(),
            ),
            MidiEvent::Aggregate(midi_events) => messages.append(
                &mut midi_events
                    .into_iter()
                    .flat_map(|e| e.into_messages())
                    .collect::<Vec<Message>>(),
            ),
        }
        return messages;
    }
}
pub fn midi_to_string(event: &MidiEvent) -> String {
    match event {
        MidiEvent::Message(message) => match message {
            Message::Start => "Start".into(),
            Message::TimingClock => "Clock Pulse".into(),
            Message::Continue => "Continue".into(),
            Message::Stop => "Stop".into(),
            Message::ActiveSensing => "Active Sensing".into(),
            Message::SystemReset => "System Reset".into(),
            Message::AllSoundOff(channel) => format!("{:?} All Sound Off", channel),
            Message::ResetAllControllers(channel) => format!("{:?} Reset Controllers", channel),
            Message::LocalControlOff(channel) => format!("{:?} Local Control Off", channel),
            Message::LocalControlOn(channel) => format!("{:?} Local Control On", channel),
            Message::AllNotesOff(channel) => format!("{:?} Note Off All", channel),
            Message::NoteOff(channel, note, vel) => {
                format!("{:?} Note Off ({}, {})", channel, note, vel)
            }
            Message::ProgramChange(channel, num) => {
                format!("{:?} Program Change ({})", channel, num)
            }
            Message::ControlChange(channel, cc, val) => format!("{:?} CC{} {}", channel, cc, val),
            Message::RPN7(channel, param, val) => format!("{:?} RPN{} {}", channel, param, val),
            Message::RPN14(channel, param, val) => format!("{:?} RPN{} {}", channel, param, val),
            Message::NRPN7(channel, param, val) => format!("{:?} NRPN{} {}", channel, param, val),
            Message::NRPN14(channel, param, val) => format!("{:?} NRPN{} {}", channel, param, val),
            Message::SysEx(_, data) => format!("SysEx ({} bytes)", data.len()),
            Message::NoteOn(channel, note, vel) => {
                format!("{:?} Note On ({}, {})", channel, note, vel)
            }
            Message::PitchBend(channel, bend) => {
                format!("{:?} Bend ({})", channel, *bend as i16)
            }
            Message::PolyphonicPressure(channel, note, pressure) => {
                format!("{:?} Poly Pressure ({}, {})", channel, note, pressure)
            }
            Message::ChannelPressure(channel, pressure) => {
                format!("{:?} Pressure ({})", channel, pressure)
            }
        },
        MidiEvent::NotesOffAll => "Note Off Global".to_string(),
        MidiEvent::Aggregate(messages) => messages
            .iter()
            .map(midi_to_string)
            .collect::<Vec<String>>()
            .join("\n"),
    }
}
pub fn raw_message_to_bytes(msg: RawMessage) -> Vec<u8> {
    match msg {
        RawMessage::Status(status) => vec![status],

        RawMessage::StatusData(status, data) => {
            vec![status, data]
        }

        RawMessage::StatusDataData(status, data1, data2) => {
            vec![status, data1, data2]
        }

        RawMessage::Raw(byte) => {
            vec![byte]
        }
    }
}
pub fn bytes_to_message(bytes: &[u8]) -> Option<Message> {
    let (&status, data) = bytes.split_first()?;
    let channel = Channel::from_u8(status & 0x0F)?;
    let message = match (status & 0xF0, data) {
        (0x80, &[note, velocity, ..]) => Message::NoteOff(channel, note, velocity),
        (0x90, &[note, velocity, ..]) => Message::NoteOn(channel, note, velocity),
        (0xA0, &[note, pressure, ..]) => Message::PolyphonicPressure(channel, note, pressure),
        (0xB0, &[controller, value, ..]) => Message::ControlChange(channel, controller, value),
        (0xC0, &[program, ..]) => Message::ProgramChange(channel, program),
        (0xD0, &[pressure, ..]) => Message::ChannelPressure(channel, pressure),
        (0xE0, &[lsb, msb, ..]) => Message::PitchBend(channel, msb_lsb_to_u14(msb, lsb)),
        _ => match status {
            0xF8 => Message::TimingClock,
            0xFA => Message::Start,
            0xFB => Message::Continue,
            0xFC => Message::Stop,
            0xFE => Message::ActiveSensing,
            0xFF => Message::SystemReset,
            _ => return None,
        },
    };
    return Some(message);
}

#[cfg(test)]
mod tests {
    use midi::ToRawMessages;

    use super::*;

    #[test]
    fn durations_follow_bpm() {
        assert_eq!(pulse_duration(125), Duration::from_millis(20));
    }

    #[test]
    fn zero_bpm_does_not_divide_by_zero() {
        assert!(pulse_duration(0) > Duration::ZERO);
    }

    #[test]
    fn notes_off_all_silences_every_channel() {
        let messages = MidiEvent::NotesOffAll.into_messages();
        assert_eq!(messages.len(), MIDI_CHANNEL_MAX as usize + 1);
        assert_eq!(messages[0], Message::AllSoundOff(Channel::Ch1));
        assert_eq!(messages[15], Message::AllSoundOff(Channel::Ch16));
    }

    #[test]
    fn aggregate_flattens_in_order() {
        let event = MidiEvent::Aggregate(vec![
            MidiEvent::Message(Message::Start),
            MidiEvent::Aggregate(vec![MidiEvent::Message(Message::Stop)]),
        ]);
        assert_eq!(event.into_messages(), vec![Message::Start, Message::Stop]);
    }

    #[test]
    fn raw_bytes_match_midi_wire_format() {
        let bytes: Vec<u8> = Message::NoteOn(Channel::Ch2, 60, 100)
            .to_raw_messages()
            .into_iter()
            .flat_map(raw_message_to_bytes)
            .collect();
        assert_eq!(bytes, vec![0x91, 60, 100]);
        assert_eq!(raw_message_to_bytes(RawMessage::Status(0xF8)), vec![0xF8]);
    }

    #[test]
    fn bytes_round_trip_through_messages() {
        let messages = [
            Message::NoteOn(Channel::Ch2, 60, 100),
            Message::NoteOff(Channel::Ch1, 60, 0),
            Message::ControlChange(Channel::Ch16, 74, 64),
            Message::ProgramChange(Channel::Ch3, 5),
            Message::ChannelPressure(Channel::Ch4, 90),
            Message::PitchBend(Channel::Ch1, 12345),
            Message::TimingClock,
        ];
        for message in messages {
            let bytes: Vec<u8> = message
                .to_raw_messages()
                .into_iter()
                .flat_map(raw_message_to_bytes)
                .collect();
            assert_eq!(bytes_to_message(&bytes), Some(message));
        }
        assert_eq!(bytes_to_message(&[0x90, 60]), None);
        assert_eq!(bytes_to_message(&[]), None);
    }

    #[test]
    fn describes_control_changes() {
        let event = MidiEvent::Message(Message::ControlChange(Channel::Ch1, 74, 64));
        assert_eq!(midi_to_string(&event), "Ch1 CC74 64");
    }

    #[test]
    fn describes_channel_pressure() {
        let event = MidiEvent::Message(Message::ChannelPressure(Channel::Ch2, 100));
        assert_eq!(midi_to_string(&event), "Ch2 Pressure (100)");
    }
}
