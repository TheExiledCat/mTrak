pub mod arp;
pub mod eq;
pub mod harmonizer;
use midi::{Channel, Message, U7};

use crate::{
    data::{
        effect::{EffectKind, EffectSettings},
        midi::{PULSES_PER_QUARTER, VELOCITY_MAX},
        note::NotePitch,
    },
    engine::effects::eq::EqEffect,
};

/// A new effect of `settings`' kind, with its saved values. Values the effect doesn't take
/// are clamped, and parameters without a saved value keep their default.
pub fn create_effect(settings: &EffectSettings) -> Box<dyn MidiEffect> {
    let mut effect: Box<dyn MidiEffect> = match settings.kind {
        EffectKind::Eq => Box::new(EqEffect::new()),
    };
    for (index, (param, value)) in effect.params().iter().zip(&settings.values).enumerate() {
        effect.set_param(index, param.kind.clamp(*value));
    }
    return effect;
}

/// The parameters an effect of `kind` has.
pub fn effect_params(kind: EffectKind) -> &'static [Param] {
    return create_effect(&EffectSettings::new(kind, Vec::new())).params();
}

/// The settings of a new effect of `kind`, with every parameter at its default.
pub fn default_settings(kind: EffectKind) -> EffectSettings {
    let values = effect_params(kind)
        .iter()
        .map(|param| param.default)
        .collect();
    return EffectSettings::new(kind, values);
}

/// Stores a parameter's value in saved settings, clamped to what the parameter takes, and
/// returns it. Settings saved before the effect had this parameter get the defaults of the
/// ones they miss. `None` if the effect has no such parameter.
pub fn set_saved_param(settings: &mut EffectSettings, param: usize, value: u8) -> Option<u8> {
    let params = effect_params(settings.kind);
    let value = params.get(param)?.kind.clamp(value);
    if settings.values.len() <= param {
        let missing = params[settings.values.len()..=param].iter();
        settings.values.extend(missing.map(|param| param.default));
    }
    settings.values[param] = value;
    return Some(value);
}

/// An instrument's chain, built from its saved effects.
pub fn create_chain(effects: &[EffectSettings]) -> MidiEffectChain {
    let mut chain = MidiEffectChain::new();
    for settings in effects {
        chain.push(create_effect(settings));
    }
    return chain;
}

/// How a parameter's value is shown and which values it takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamKind {
    /// A number from `min` to `max`, shown in hex like the rest of the tracker.
    Number { min: u8, max: u8 },
    /// 0 for off, 1 for on.
    Toggle,
    /// An index into these names.
    Choice(&'static [&'static str]),
    /// A MIDI pitch, shown as a note name.
    Pitch,
}
impl ParamKind {
    /// The nearest value this kind takes.
    pub fn clamp(&self, value: u8) -> u8 {
        return match *self {
            ParamKind::Number { min, max } => value.clamp(min, max),
            ParamKind::Toggle => value.min(1),
            ParamKind::Choice(names) => value.min(names.len().saturating_sub(1) as u8),
            ParamKind::Pitch => value.min(VELOCITY_MAX),
        };
    }
}

/// One of an effect's settings. Every value is a byte, like the pattern's command values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Param {
    pub name: &'static str,
    pub kind: ParamKind,
    pub default: u8,
}

/// Where playback is when an effect runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timing {
    /// Pulses since the current pattern's first row, 24 per beat. They're counted as they go
    /// by, so speed changes within the pattern keep the beat in place.
    pub pulse: u32,
}
impl Timing {
    pub fn new(pulse: u32) -> Self {
        return Self { pulse };
    }

    /// How far into the beat this pulse is, 0 to 23, taking each pattern to start on a beat.
    pub fn pulse_in_beat(&self) -> u32 {
        return self.pulse % PULSES_PER_QUARTER;
    }
}

/// A live MIDI effect on an instrument's channel. Effects see the channel's messages after the
/// engine has applied mutes, transposes and the global velocity, just before they're sent.
#[allow(unused_variables)]
pub trait MidiEffect: Send {
    /// The effect's settings; their indices are what `param` and `set_param` take.
    fn params(&self) -> &'static [Param];
    fn param(&self, index: usize) -> u8;
    /// Only gets values `ParamKind::clamp` allows, for indices `params` has.
    fn set_param(&mut self, index: usize, value: u8);
    /// Handles one message, writing what it turns into to `out`. Write nothing to drop it.
    /// `timing` is `None` while stopped, e.g. for notes played live.
    fn process(&mut self, message: Message, timing: Option<Timing>, out: &mut Vec<Message>);
    /// Called once on every clock pulse while playing, 24 per beat, for effects that make
    /// messages of their own over time. It runs before the pulse's own messages are processed.
    fn tick(&mut self, timing: Timing, out: &mut Vec<Message>) {}
    /// Forgets anything held, e.g. on stop, panic or when playback starts, writing the messages
    /// that release notes it still has sounding.
    fn reset(&mut self, out: &mut Vec<Message>) {}
}

/// An instrument's effects, each one feeding the next.
#[derive(Default)]
pub struct MidiEffectChain {
    chain: Vec<Box<dyn MidiEffect>>,
}
impl MidiEffectChain {
    pub fn new() -> Self {
        return Self { chain: Vec::new() };
    }

    pub fn push(&mut self, effect: Box<dyn MidiEffect>) {
        self.chain.push(effect);
    }
    pub fn is_empty(&self) -> bool {
        return self.chain.is_empty();
    }
    pub fn len(&self) -> usize {
        return self.chain.len();
    }
    pub fn get(&self, index: usize) -> Option<&dyn MidiEffect> {
        return self.chain.get(index).map(|effect| effect.as_ref());
    }
    /// Sets a parameter of the effect at `effect`, clamped to what the parameter takes.
    /// Indices out of range are ignored.
    pub fn set_param(&mut self, effect: usize, param: usize, value: u8) {
        let Some(effect) = self.chain.get_mut(effect) else {
            return;
        };
        let Some(kind) = effect.params().get(param).map(|p| p.kind) else {
            return;
        };
        effect.set_param(param, kind.clamp(value));
    }

    /// Runs a message through every effect in turn.
    pub fn process(&mut self, message: Message, timing: Option<Timing>, out: &mut Vec<Message>) {
        let mut messages = vec![message];
        for effect in &mut self.chain {
            messages = process_all(effect.as_mut(), messages, timing);
        }
        out.extend(messages);
    }

    /// What an effect makes on a tick goes through the effects after it.
    pub fn tick(&mut self, timing: Timing, out: &mut Vec<Message>) {
        let mut messages = vec![];
        for effect in &mut self.chain {
            messages = process_all(effect.as_mut(), messages, Some(timing));
            effect.tick(timing, &mut messages);
        }
        out.extend(messages);
    }

    /// The releases an effect writes go through the effects after it before those reset too.
    pub fn reset(&mut self, out: &mut Vec<Message>) {
        let mut messages = vec![];
        for effect in &mut self.chain {
            messages = process_all(effect.as_mut(), messages, None);
            effect.reset(&mut messages);
        }
        out.extend(messages);
    }
}

fn process_all(
    effect: &mut dyn MidiEffect,
    messages: Vec<Message>,
    timing: Option<Timing>,
) -> Vec<Message> {
    let mut out = vec![];
    for message in messages {
        effect.process(message, timing, &mut out);
    }
    return out;
}

#[derive(Clone, Copy)]
pub struct NoteEvent {
    pub pitch: NotePitch,
    pub velocity: U7,
    pub channel: Channel,
    pub is_note_off: bool,
}
impl NoteEvent {
    pub fn to_message(self) -> Message {
        let channel = self.channel;
        let pitch = self.pitch.0;
        let velocity = self.velocity;
        return match self.is_note_off {
            true => Message::NoteOff(channel, pitch, velocity),
            false => Message::NoteOn(channel, pitch, velocity),
        };
    }
}
/// The note in a note on or off. A note on with velocity 0 is a note off in MIDI, so it comes
/// back as one.
pub fn note_event(message: &Message) -> Option<NoteEvent> {
    return Some(match *message {
        Message::NoteOn(channel, pitch, velocity) => NoteEvent {
            channel,
            pitch: NotePitch(pitch),
            velocity,
            is_note_off: velocity == 0,
        },
        Message::NoteOff(channel, pitch, velocity) => NoteEvent {
            channel,
            pitch: NotePitch(pitch),
            velocity,
            is_note_off: true,
        },
        _ => return None,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Moves notes up by `semitones` and plays each one again `delay` ticks later.
    struct Echo {
        semitones: u8,
        delay: u8,
        pending: Vec<(u8, Message)>,
    }
    impl MidiEffect for Echo {
        fn params(&self) -> &'static [Param] {
            return &[];
        }
        fn param(&self, _index: usize) -> u8 {
            return 0;
        }
        fn set_param(&mut self, _index: usize, _value: u8) {}
        fn process(&mut self, message: Message, _timing: Option<Timing>, out: &mut Vec<Message>) {
            let message = match message {
                Message::NoteOn(c, p, v) => Message::NoteOn(c, p + self.semitones, v),
                Message::NoteOff(c, p, v) => Message::NoteOff(c, p + self.semitones, v),
                message => message,
            };
            self.pending.push((self.delay, message.clone()));
            out.push(message);
        }
        fn tick(&mut self, _timing: Timing, out: &mut Vec<Message>) {
            for (ticks, message) in &mut self.pending {
                *ticks -= 1;
                if *ticks == 0 {
                    out.push(message.clone());
                }
            }
            self.pending.retain(|(ticks, _)| *ticks > 0);
        }
        fn reset(&mut self, out: &mut Vec<Message>) {
            for (_, message) in self.pending.drain(..) {
                if let Message::NoteOn(c, p, _) = message {
                    out.push(Message::NoteOff(c, p, 0));
                }
            }
        }
    }

    fn echo(semitones: u8, delay: u8) -> Box<dyn MidiEffect> {
        return Box::new(Echo {
            semitones,
            delay,
            pending: vec![],
        });
    }

    fn note_on(pitch: u8) -> Message {
        return Message::NoteOn(Channel::Ch1, pitch, 100);
    }

    #[test]
    fn empty_chain_passes_messages_through() {
        let mut chain = MidiEffectChain::new();
        let mut out = vec![];
        chain.process(note_on(60), None, &mut out);
        assert_eq!(out, vec![note_on(60)]);
    }

    #[test]
    fn each_effect_feeds_the_next() {
        let mut chain = MidiEffectChain::new();
        chain.push(echo(1, 10));
        chain.push(echo(2, 10));
        let mut out = vec![];
        chain.process(note_on(60), None, &mut out);
        assert_eq!(out, vec![note_on(63)]);
    }

    #[test]
    fn ticked_messages_go_through_later_effects() {
        let mut chain = MidiEffectChain::new();
        chain.push(echo(0, 1));
        chain.push(echo(12, 5));
        let mut out = vec![];
        chain.process(note_on(60), None, &mut out);
        out.clear();
        chain.tick(Timing::new(1), &mut out);
        assert_eq!(out, vec![note_on(72)]);
    }

    #[test]
    fn reset_releases_go_through_later_effects() {
        let mut chain = MidiEffectChain::new();
        chain.push(echo(0, 5));
        chain.push(echo(12, 5));
        let mut out = vec![];
        chain.process(note_on(60), None, &mut out);
        out.clear();
        chain.reset(&mut out);
        assert_eq!(
            out,
            vec![
                Message::NoteOff(Channel::Ch1, 72, 0),
                Message::NoteOff(Channel::Ch1, 72, 0),
            ]
        );
    }

    #[test]
    fn param_values_are_clamped_to_their_kind() {
        let number = ParamKind::Number { min: 2, max: 10 };
        assert_eq!(number.clamp(0), 2);
        assert_eq!(number.clamp(20), 10);
        assert_eq!(ParamKind::Toggle.clamp(5), 1);
        assert_eq!(ParamKind::Choice(&["a", "b", "c"]).clamp(7), 2);
        assert_eq!(ParamKind::Pitch.clamp(0xFF), 127);
    }

    #[test]
    fn chain_sets_clamped_params_and_ignores_bad_indices() {
        let mut chain = MidiEffectChain::new();
        chain.push(create_effect(&default_settings(EffectKind::Eq)));
        let filter = chain.get(0).unwrap().params().len() - 1;
        chain.set_param(0, 0, 0xFF);
        assert_eq!(chain.get(0).unwrap().param(0), 127);
        chain.set_param(0, 99, 1);
        chain.set_param(5, 0, 1);
        assert_eq!(chain.get(0).unwrap().param(filter), 8);
    }

    #[test]
    fn every_effect_starts_at_its_defaults_and_keeps_what_is_set() {
        for kind in EffectKind::ALL {
            let mut effect = create_effect(&EffectSettings::new(kind, Vec::new()));
            for (index, param) in effect.params().iter().enumerate() {
                assert_eq!(
                    effect.param(index),
                    param.default,
                    "{} {}",
                    kind.name(),
                    param.name
                );
                assert_eq!(param.kind.clamp(param.default), param.default);
            }
            for (index, param) in effect.params().iter().enumerate() {
                let value = param.kind.clamp(param.default.wrapping_add(1));
                effect.set_param(index, value);
                assert_eq!(effect.param(index), value, "{} {}", kind.name(), param.name);
            }
        }
    }

    #[test]
    fn created_effects_take_their_saved_values() {
        let saved = EffectSettings::new(EffectKind::Eq, vec![30, 0xFF, 0x40]);
        let effect = create_effect(&saved);
        let defaults = default_settings(EffectKind::Eq).values;
        assert_eq!(effect.param(0), 30);
        assert_eq!(effect.param(1), 127, "a pitch above 127 is clamped");
        assert_eq!(effect.param(2), 0x40);
        assert_eq!(
            effect.param(3),
            defaults[3],
            "a value that wasn't saved is the default"
        );
    }

    #[test]
    fn values_past_an_effects_params_are_ignored() {
        let mut values = default_settings(EffectKind::Eq).values;
        values.push(0x12);
        let effect = create_effect(&EffectSettings::new(EffectKind::Eq, values.clone()));
        let read: Vec<u8> = (0..effect.params().len())
            .map(|i| effect.param(i))
            .collect();
        assert_eq!(read, values[..values.len() - 1]);
    }

    #[test]
    fn saved_params_are_clamped() {
        let mut settings = default_settings(EffectKind::Eq);
        assert_eq!(set_saved_param(&mut settings, 0, 0xFF), Some(127));
        assert_eq!(settings.values[0], 127);
    }

    #[test]
    fn saved_params_fill_in_the_defaults_of_missing_values() {
        let mut settings = EffectSettings::new(EffectKind::Eq, vec![30]);
        assert_eq!(set_saved_param(&mut settings, 3, 0x40), Some(0x40));
        let defaults = default_settings(EffectKind::Eq).values;
        assert_eq!(settings.values, vec![30, defaults[1], defaults[2], 0x40]);
    }

    #[test]
    fn saved_params_ignore_params_the_effect_does_not_have() {
        let mut settings = default_settings(EffectKind::Eq);
        let before = settings.clone();
        assert_eq!(set_saved_param(&mut settings, 99, 1), None);
        assert_eq!(settings, before);
    }

    #[test]
    fn chains_are_built_in_the_saved_order() {
        let mut quiet = default_settings(EffectKind::Eq);
        quiet.values[3] = 0x40;
        let chain = create_chain(&[default_settings(EffectKind::Eq), quiet]);
        assert_eq!(chain.len(), 2);
        assert_eq!(chain.get(0).unwrap().param(3), 0x80);
        assert_eq!(chain.get(1).unwrap().param(3), 0x40);
    }

    #[test]
    fn pulse_in_beat_wraps_every_beat() {
        assert_eq!(Timing::new(5).pulse_in_beat(), 5);
        assert_eq!(Timing::new(PULSES_PER_QUARTER * 3 + 2).pulse_in_beat(), 2);
    }

    #[test]
    fn zero_velocity_note_on_is_a_note_off() {
        let note = note_event(&Message::NoteOn(Channel::Ch1, 60, 0)).unwrap();
        assert!(note.is_note_off);
    }

    #[test]
    fn note_event_keeps_pitches_below_the_tracker_range() {
        let note = note_event(&Message::NoteOn(Channel::Ch1, 0, 100)).unwrap();
        assert_eq!(note.pitch.0, 0);
    }
}
