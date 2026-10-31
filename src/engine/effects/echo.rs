use midi::{Channel, Message};
use num::FromPrimitive;

use crate::{
    data::midi::scale_velocity,
    engine::{
        effects::{MidiEffect, Param, ParamKind, Timing, ValueLabel, note_event},
        notes::{Countdown, MIDI_CHANNEL_COUNT, PITCH_COUNT},
    },
};

pub const RATE_PARAM: usize = 0;
pub const FEEDBACK_PARAM: usize = 1;
pub const MIX_PARAM: usize = 2;
/// 50%: the note and its echoes both play at full velocity.
const EVEN_MIX: u8 = 0x80;

const PARAMS: &[Param] = &[
    Param {
        name: "Rate (ticks)",
        kind: ParamKind::Number {
            min: 1,
            max: u8::MAX,
        },
        default: 0x0C,
        label: ValueLabel::Ticks,
    },
    Param {
        name: "Feedback",
        kind: ParamKind::Number {
            min: 0,
            max: u8::MAX,
        },
        default: 0x80,
        label: ValueLabel::Percent,
    },
    Param {
        name: "Dry/wet",
        kind: ParamKind::Number {
            min: 0,
            max: u8::MAX,
        },
        default: EVEN_MIX,
        label: ValueLabel::Percent,
    },
];

/// A note that was played, and the echoes of it still to come. Its id is the id of the voice
/// it plays.
struct Source {
    id: u32,
    channel: Channel,
    pitch: u8,
    start: u32,
    /// How long the note was held, once it's released.
    length: Option<u32>,
    /// Echoes that haven't started yet.
    pending: u32,
}

/// An echo that started while its note was still held, so its length isn't known yet.
struct Waiting {
    source: u32,
    voice: u32,
    channel: Channel,
    pitch: u8,
    start: u32,
}

enum EchoEvent {
    On {
        source: u32,
        channel: Channel,
        pitch: u8,
        velocity: u8,
    },
    Off {
        voice: u32,
        channel: Channel,
        pitch: u8,
    },
}

/// Plays every note again every `rate` ticks, each echo as long as the note and `feedback` / 256
/// as loud as the one before, until the velocity reaches 0. While stopped there are no ticks,
/// so notes pass through.
///
/// A pitch plays one voice at a time: a note or echo of a pitch that's sounding cuts it off and
/// plays it again, and a note off only ends the voice it belongs to.
pub struct EchoEffect {
    rate: u8,
    feedback: u8,
    mix: u8,
    /// Ticks since playback started.
    now: u32,
    next_id: u32,
    /// The voice sounding on each channel and pitch.
    voices: [[Option<u32>; PITCH_COUNT]; MIDI_CHANNEL_COUNT],
    sources: Vec<Source>,
    waiting: Vec<Waiting>,
    events: Countdown<EchoEvent>,
    /// Echoes due this pulse, started once the pulse's notes are in.
    due: Vec<EchoEvent>,
}

impl EchoEffect {
    pub fn new() -> Self {
        return Self {
            rate: PARAMS[RATE_PARAM].default,
            feedback: PARAMS[FEEDBACK_PARAM].default,
            mix: PARAMS[MIX_PARAM].default,
            now: 0,
            next_id: 0,
            voices: [[None; PITCH_COUNT]; MIDI_CHANNEL_COUNT],
            sources: Vec::new(),
            waiting: Vec::new(),
            events: Countdown::new(),
            due: Vec::new(),
        };
    }

    fn dry_level(&self) -> u8 {
        if self.mix <= EVEN_MIX {
            return EVEN_MIX;
        }
        let left = (u8::MAX - self.mix) as u16;
        return (left * EVEN_MIX as u16 / (u8::MAX - EVEN_MIX) as u16) as u8;
    }

    fn voice(&mut self, channel: Channel, pitch: u8) -> &mut Option<u32> {
        return &mut self.voices[channel as usize][pitch as usize];
    }

    /// Plays a new voice, cutting off the one sounding the pitch.
    fn start(&mut self, channel: Channel, pitch: u8, velocity: u8, out: &mut Vec<Message>) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        if self.voice(channel, pitch).replace(id).is_some() {
            out.push(Message::NoteOff(channel, pitch, 0));
        }
        out.push(Message::NoteOn(channel, pitch, velocity));
        return id;
    }

    /// Ends a voice, unless something else took over its pitch.
    fn stop(
        &mut self,
        voice: u32,
        channel: Channel,
        pitch: u8,
        velocity: u8,
        out: &mut Vec<Message>,
    ) {
        let sounding = self.voice(channel, pitch);
        if *sounding == Some(voice) {
            *sounding = None;
            out.push(Message::NoteOff(channel, pitch, velocity));
        }
    }

    fn play_note(
        &mut self,
        channel: Channel,
        pitch: u8,
        velocity: u8,
        playing: bool,
        out: &mut Vec<Message>,
    ) {
        let (dry, wet) = match playing {
            true => (self.dry_level(), self.mix.min(EVEN_MIX)),
            false => (EVEN_MIX, 0),
        };
        let id = match scale_velocity(velocity, dry, EVEN_MIX) {
            0 => {
                let id = self.next_id;
                self.next_id = self.next_id.wrapping_add(1);
                id
            }
            dry => self.start(channel, pitch, dry, out),
        };
        let feedback = if playing { self.feedback as u32 } else { 0 };
        let mut velocity = scale_velocity(velocity, wet, EVEN_MIX) as u32;
        let mut pending = 0;
        loop {
            velocity = velocity * feedback / 0x100;
            if velocity == 0 {
                break;
            }
            pending += 1;
            let event = EchoEvent::On {
                source: id,
                channel,
                pitch,
                velocity: velocity as u8,
            };
            self.events.push(event, pending * self.rate as u32);
        }
        self.sources.push(Source {
            id,
            channel,
            pitch,
            start: self.now,
            length: None,
            pending,
        });
    }

    /// Ends the oldest held note of the pitch. Its length is known now, so the echoes it already
    /// started can end too.
    fn release(&mut self, channel: Channel, pitch: u8, velocity: u8, out: &mut Vec<Message>) {
        let Some(source) = self
            .sources
            .iter_mut()
            .find(|s| s.channel == channel && s.pitch == pitch && s.length.is_none())
        else {
            if self.voice(channel, pitch).is_none() {
                out.push(Message::NoteOff(channel, pitch, velocity));
            }
            return;
        };
        let length = (self.now - source.start).max(1);
        source.length = Some(length);
        let id = source.id;
        self.stop(id, channel, pitch, velocity, out);
        let (started, waiting): (Vec<Waiting>, _) = std::mem::take(&mut self.waiting)
            .into_iter()
            .partition(|echo| echo.source == id);
        self.waiting = waiting;
        for echo in started {
            let off = EchoEvent::Off {
                voice: echo.voice,
                channel: echo.channel,
                pitch: echo.pitch,
            };
            self.events.push(off, echo.start + length - self.now);
        }
        self.forget_finished();
    }

    fn forget_finished(&mut self) {
        self.sources
            .retain(|source| source.length.is_none() || source.pending > 0);
    }

    fn play_echo(&mut self, event: EchoEvent, out: &mut Vec<Message>) {
        let EchoEvent::On {
            source,
            channel,
            pitch,
            velocity,
        } = event
        else {
            return;
        };
        let Some(index) = self.sources.iter().position(|s| s.id == source) else {
            return;
        };
        let voice = self.start(channel, pitch, velocity, out);
        let source = &mut self.sources[index];
        source.pending -= 1;
        match source.length {
            Some(length) => {
                let off = EchoEvent::Off {
                    voice,
                    channel,
                    pitch,
                };
                self.events.push(off, length);
            }
            None => self.waiting.push(Waiting {
                source: source.id,
                voice,
                channel,
                pitch,
                start: self.now,
            }),
        }
        self.forget_finished();
    }
}

impl MidiEffect for EchoEffect {
    fn params(&self) -> &'static [Param] {
        return PARAMS;
    }

    fn param(&self, index: usize) -> u8 {
        return match index {
            RATE_PARAM => self.rate,
            FEEDBACK_PARAM => self.feedback,
            MIX_PARAM => self.mix,
            _ => 0,
        };
    }

    fn set_param(&mut self, index: usize, value: u8) {
        match index {
            RATE_PARAM => self.rate = value,
            FEEDBACK_PARAM => self.feedback = value,
            MIX_PARAM => self.mix = value,
            _ => {}
        }
    }

    fn process(&mut self, message: Message, timing: Option<Timing>, out: &mut Vec<Message>) {
        let Some(note) = note_event(&message) else {
            out.push(message);
            return;
        };
        let (channel, pitch) = (note.channel, note.pitch.0);
        match note.is_note_off {
            true => self.release(channel, pitch, note.velocity, out),
            false => self.play_note(channel, pitch, note.velocity, timing.is_some(), out),
        }
    }

    fn tick(&mut self, _timing: Timing, out: &mut Vec<Message>) {
        self.now += 1;
        for event in self.events.tick() {
            match event {
                EchoEvent::Off {
                    voice,
                    channel,
                    pitch,
                } => self.stop(voice, channel, pitch, 0, out),
                on => self.due.push(on),
            }
        }
    }

    fn after_pulse(&mut self, _timing: Timing, out: &mut Vec<Message>) {
        for event in std::mem::take(&mut self.due) {
            self.play_echo(event, out);
        }
    }

    fn reset(&mut self, out: &mut Vec<Message>) {
        self.now = 0;
        self.sources.clear();
        self.waiting.clear();
        self.events.clear();
        self.due.clear();
        for (channel, voices) in self.voices.iter_mut().enumerate() {
            for (pitch, voice) in voices.iter_mut().enumerate() {
                if voice.take().is_some() {
                    let channel = Channel::from_usize(channel).unwrap();
                    out.push(Message::NoteOff(channel, pitch as u8, 0));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn echo(rate: u8, feedback: u8) -> EchoEffect {
        let mut echo = EchoEffect::new();
        echo.set_param(RATE_PARAM, rate);
        echo.set_param(FEEDBACK_PARAM, feedback);
        return echo;
    }

    fn on(pitch: u8, velocity: u8) -> Message {
        return Message::NoteOn(Channel::Ch1, pitch, velocity);
    }

    fn off(pitch: u8) -> Message {
        return Message::NoteOff(Channel::Ch1, pitch, 0);
    }

    /// Plays `pulses`, sending each message on the pulse it's paired with, and returns what
    /// was sent on each pulse.
    fn run(
        echo: &mut EchoEffect,
        pulses: std::ops::RangeInclusive<u32>,
        messages: &[(u32, Message)],
    ) -> Vec<Vec<Message>> {
        let mut sent = vec![];
        for pulse in pulses {
            let timing = Timing::new(pulse);
            let mut out = vec![];
            echo.tick(timing, &mut out);
            for (at, message) in messages {
                if *at == pulse {
                    echo.process(message.clone(), Some(timing), &mut out);
                }
            }
            echo.after_pulse(timing, &mut out);
            sent.push(out);
        }
        return sent;
    }

    #[test]
    fn echoes_repeat_at_the_rate_and_fade() {
        let mut echo = echo(4, 0x80);
        let sent = run(&mut echo, 0..=12, &[(0, on(60, 100)), (2, off(60))]);
        assert_eq!(sent[0], vec![on(60, 100)]);
        assert_eq!(sent[2], vec![off(60)]);
        assert_eq!(sent[4], vec![on(60, 50)]);
        assert_eq!(sent[6], vec![off(60)]);
        assert_eq!(sent[8], vec![on(60, 25)]);
        assert_eq!(sent[10], vec![off(60)]);
        assert_eq!(sent[12], vec![on(60, 12)]);
    }

    #[test]
    fn echoes_stop_once_the_velocity_reaches_0() {
        let mut echo = echo(1, 0x80);
        let sent = run(&mut echo, 0..=20, &[(0, on(60, 8))]);
        let ons: Vec<&Message> = sent
            .iter()
            .flatten()
            .filter(|m| matches!(m, Message::NoteOn(..)))
            .collect();
        assert_eq!(ons, vec![&on(60, 8), &on(60, 4), &on(60, 2), &on(60, 1)]);
        assert!(echo.events.tick().is_empty());
    }

    #[test]
    fn full_feedback_still_ends() {
        let mut echo = echo(1, u8::MAX);
        run(&mut echo, 0..=200, &[(0, on(60, 100)), (0, off(60))]);
        assert!(echo.sources.is_empty());
        assert!(echo.events.tick().is_empty());
    }

    #[test]
    fn no_feedback_means_no_echoes() {
        let mut echo = echo(1, 0);
        let sent = run(&mut echo, 0..=4, &[(0, on(60, 100)), (1, off(60))]);
        assert_eq!(sent.concat(), vec![on(60, 100), off(60)]);
        assert!(echo.sources.is_empty());
    }

    #[test]
    fn an_echo_that_starts_while_the_note_is_held_lasts_as_long() {
        let mut echo = echo(4, 3);
        let sent = run(&mut echo, 0..=12, &[(0, on(60, 127)), (6, off(60))]);
        assert_eq!(sent[4], vec![off(60), on(60, 1)]);
        assert!(sent[6].is_empty());
        assert_eq!(sent[10], vec![off(60)]);
        assert_eq!(sent.concat().len(), 4);
    }

    #[test]
    fn the_note_off_of_a_cut_off_echo_leaves_the_new_note_alone() {
        let mut echo = echo(4, 0x80);
        let sent = run(
            &mut echo,
            0..=7,
            &[(0, on(60, 100)), (3, off(60)), (5, on(60, 100))],
        );
        assert_eq!(sent[4], vec![on(60, 50)]);
        assert_eq!(sent[5], vec![off(60), on(60, 100)]);
        assert!(sent[7].is_empty());
    }

    #[test]
    fn every_note_on_gets_its_note_off() {
        let mut echo = echo(3, 0xF0);
        let mut messages = vec![];
        for start in (8..64).step_by(8) {
            messages.push((start, off(60)));
        }
        for start in (0..64).step_by(8) {
            messages.push((start, on(60, 100)));
        }
        let mut sent = run(&mut echo, 0..=64, &messages).concat();
        echo.reset(&mut sent);
        let mut sounding = false;
        for message in &sent {
            match message {
                Message::NoteOn(..) => assert!(!sounding, "{sent:?}"),
                Message::NoteOff(..) => assert!(sounding, "{sent:?}"),
                _ => {}
            }
            sounding = matches!(message, Message::NoteOn(..));
        }
        assert!(!sounding);
        assert!(sent.len() > 32);
    }

    #[test]
    fn notes_pass_through_while_stopped() {
        let mut echo = echo(1, 0x80);
        let mut out = vec![];
        echo.process(on(60, 100), None, &mut out);
        echo.process(off(60), None, &mut out);
        assert_eq!(out, vec![on(60, 100), off(60)]);
        assert!(echo.events.tick().is_empty());
    }

    #[test]
    fn a_note_played_while_stopped_is_released_while_playing() {
        let mut echo = echo(1, 0x80);
        let mut out = vec![];
        echo.process(on(60, 100), None, &mut out);
        let sent = run(&mut echo, 0..=0, &[(0, off(60))]);
        assert_eq!(sent[0], vec![off(60)]);
    }

    #[test]
    fn other_messages_pass_through() {
        let mut echo = echo(1, 0x80);
        let cc = Message::ControlChange(Channel::Ch1, 1, 64);
        let sent = run(&mut echo, 0..=2, &[(0, cc.clone())]);
        assert_eq!(sent.concat(), vec![cc]);
    }

    #[test]
    fn reset_releases_the_echoes_sounding() {
        let mut echo = echo(1, 0x80);
        run(&mut echo, 0..=1, &[(0, on(60, 100))]);
        let mut out = vec![];
        echo.reset(&mut out);
        assert_eq!(out, vec![off(60)]);
        assert!(run(&mut echo, 0..=10, &[]).concat().is_empty());
    }

    #[test]
    fn dry_wet_fades_out_the_echoes_or_the_note() {
        let first_echo = |mix: u8| {
            let mut echo = echo(1, 0x80);
            echo.mix = mix;
            let mut out = vec![];
            echo.process(
                Message::NoteOn(Channel::Ch1, 60, 100),
                Some(Timing::new(0)),
                &mut out,
            );
            echo.tick(Timing::new(1), &mut out);
            echo.after_pulse(Timing::new(1), &mut out);
            return out
                .into_iter()
                .filter_map(|m| match m {
                    Message::NoteOn(_, _, velocity) => Some(velocity),
                    _ => None,
                })
                .collect::<Vec<u8>>();
        };
        assert_eq!(first_echo(EVEN_MIX), vec![100, 50]);
        assert_eq!(first_echo(0), vec![100]);
        assert_eq!(first_echo(u8::MAX), vec![50]);
        assert_eq!(first_echo(0x40), vec![100, 25]);
    }
}
