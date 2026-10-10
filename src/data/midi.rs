use std::time::Duration;

use midi::{Channel, Message, RawMessage};
use num::FromPrimitive;

pub const MIDI_CHANNEL_MAX: u8 = 0xF;
pub const PULSES_PER_QUARTER: u32 = 24;
pub const LINES_PER_QUARTER: u32 = 4;
pub const PULSES_PER_LINE: u32 = PULSES_PER_QUARTER / LINES_PER_QUARTER;
pub const VELOCITY_MAX: u8 = 0x7F;
pub const VOLUME_MAX: u8 = 0x80;
pub const DEFAULT_BPM: u8 = 125;

pub fn pulse_duration(bpm: u8) -> Duration {
    return Duration::from_secs_f64(60.0 / (bpm.max(1) as f64 * PULSES_PER_QUARTER as f64));
}
pub fn tick_duration(bpm: u8, ticks_per_line: u8) -> Duration {
    return pulse_duration(bpm) * PULSES_PER_LINE / ticks_per_line.max(1) as u32;
}
#[derive(Clone)]
pub enum MidiEvent {
    Message(Message),
    NotesOffAll,
    Aggregate(Vec<MidiEvent>),
}
impl MidiEvent {
    pub fn to_messages(self) -> Vec<Message> {
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
                    .flat_map(|e| e.to_messages())
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
            Message::ActiveSensing => todo!(),
            Message::SystemReset => todo!(),
            Message::AllSoundOff(channel) => todo!(),
            Message::ResetAllControllers(channel) => todo!(),
            Message::LocalControlOff(channel) => todo!(),
            Message::LocalControlOn(channel) => todo!(),
            Message::AllNotesOff(channel) => format!("{:?} Note Off All", channel),
            Message::NoteOff(channel, note, vel) => {
                format!("{:?} Note Off ({}, {})", channel, note, vel)
            }
            Message::ProgramChange(channel, num) => {
                format!("{:?} Program Change ({})", channel, num)
            }
            Message::ControlChange(channel, cc, val) => format!("{:?} CC{} {}", channel, cc, val),
            Message::RPN7(channel, _, _) => todo!(),
            Message::RPN14(channel, _, _) => todo!(),
            Message::NRPN7(channel, _, _) => todo!(),
            Message::NRPN14(channel, _, _) => todo!(),
            Message::SysEx(manufacturer, items) => todo!(),
            Message::NoteOn(channel, note, vel) => {
                format!("{:?} Note On ({}, {})", channel, note, vel)
            }
            Message::PitchBend(channel, bend) => {
                format!("{:?} Bend ({})", channel, *bend as i16)
            }
            Message::PolyphonicPressure(channel, _, _) => todo!(),
            Message::ChannelPressure(channel, pressure) => {
                format!("{:?} Pressure ({})", channel, pressure)
            }
        },
        MidiEvent::NotesOffAll => format!("Note Off Global"),
        MidiEvent::Aggregate(messages) => messages
            .iter()
            .map(|m| midi_to_string(m))
            .collect::<Vec<String>>()
            .join("\n"),
    }
}
pub fn raw_message_to_bytes(msg: RawMessage) -> Vec<u8> {
    match msg {
        RawMessage::Status(status) => vec![status],

        RawMessage::StatusData(status, data) => {
            vec![status, u8::from(data)]
        }

        RawMessage::StatusDataData(status, data1, data2) => {
            vec![status, u8::from(data1), u8::from(data2)]
        }

        RawMessage::Raw(byte) => {
            vec![byte]
        }
    }
}

#[cfg(test)]
mod tests {
    use midi::ToRawMessages;

    use super::*;

    #[test]
    fn durations_follow_bpm() {
        assert_eq!(pulse_duration(125), Duration::from_millis(20));
        assert_eq!(tick_duration(125, 6), Duration::from_millis(20));
    }

    #[test]
    fn zero_bpm_and_ticks_do_not_divide_by_zero() {
        assert!(pulse_duration(0) > Duration::ZERO);
        assert!(tick_duration(0, 0) > Duration::ZERO);
    }

    #[test]
    fn notes_off_all_silences_every_channel() {
        let messages = MidiEvent::NotesOffAll.to_messages();
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
        assert_eq!(event.to_messages(), vec![Message::Start, Message::Stop]);
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
