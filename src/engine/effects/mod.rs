pub mod arp;
pub mod echo;
pub mod envelope;
pub mod eq;
pub mod harmonizer;
pub mod lfo;
use midi::{Channel, Message, U7};

use crate::{
    data::{
        effect::{EffectKind, EffectSettings},
        midi::{CC_MAX, PITCH_MAX, PULSES_PER_QUARTER},
        note::{NotePitch, SEMITONES_PER_OCTAVE},
    },
    engine::effects::{
        arp::ArpeggiatorEffect, echo::EchoEffect, envelope::EnvelopeEffect, eq::EqEffect,
        harmonizer::Harmonizer, lfo::LfoEffect,
    },
    tui::framework::core_extensions::Boxed,
};

/// A new effect of `settings`' kind, with its saved values. Values the effect doesn't take
/// are clamped, and parameters without a saved value keep their default.
pub fn create_effect(settings: &EffectSettings) -> Box<dyn MidiEffect> {
    let mut effect: Box<dyn MidiEffect> = match settings.kind {
        EffectKind::Eq => EqEffect::new().boxed(),
        EffectKind::Arp => ArpeggiatorEffect::new().boxed(),
        EffectKind::Harmonize => Harmonizer::new().boxed(),
        EffectKind::Lfo => LfoEffect::new().boxed(),
        EffectKind::Echo => EchoEffect::new().boxed(),
        EffectKind::Envelope => EnvelopeEffect::new().boxed(),
    };
    for (index, (param, value)) in effect.params().iter().zip(&settings.values).enumerate() {
        effect.set_param(index, param.kind.clamp(*value));
    }
    return effect;
}

pub fn effect_params(kind: EffectKind) -> &'static [Param] {
    return create_effect(&EffectSettings::new(kind, Vec::new())).params();
}

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

pub fn create_chain(effects: &[EffectSettings]) -> MidiEffectChain {
    let mut chain = MidiEffectChain::new();
    for settings in effects {
        chain.push_settings(settings);
    }
    return chain;
}

/// How a parameter's value is shown and which values it takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamKind {
    /// A number from `min` to `max`, shown in hex like the rest of the tracker.
    Number { min: u8, max: u8 },
    /// A number from `min` to `max` stored as a two's complement byte, shown in hex with a
    /// sign like transpose.
    SignedNumber { min: i8, max: i8 },
    /// 0 for off, 1 for on.
    Toggle,
    /// An index into these names.
    Choice(&'static [&'static str]),
    /// A MIDI pitch, shown as a note name.
    Pitch,
    /// A CC number, shown with its name.
    Cc,
}
impl ParamKind {
    pub fn clamp(&self, value: u8) -> u8 {
        return match *self {
            ParamKind::Number { min, max } => value.clamp(min, max),
            ParamKind::SignedNumber { min, max } => (value as i8).clamp(min, max) as u8,
            ParamKind::Toggle => value.min(1),
            ParamKind::Choice(names) => value.min(names.len().saturating_sub(1) as u8),
            ParamKind::Pitch => value.min(PITCH_MAX),
            ParamKind::Cc => value.min(CC_MAX),
        };
    }

    /// The value a CC value of 0 to 127 sets, spread over the whole range.
    pub fn cc_to_value(&self, value: u8) -> u8 {
        let value = value.min(CC_MAX) as i32;
        let spread =
            |min: i32, max: i32| min + (value * (max - min) + CC_MAX as i32 / 2) / CC_MAX as i32;
        return match *self {
            ParamKind::Number { min, max } => spread(min as i32, max as i32) as u8,
            ParamKind::SignedNumber { min, max } => spread(min as i32, max as i32) as i8 as u8,
            ParamKind::Toggle => (value > CC_MAX as i32 / 2) as u8,
            ParamKind::Choice(names) => {
                (value as usize * names.len() / (CC_MAX as usize + 1)) as u8
            }
            ParamKind::Pitch | ParamKind::Cc => value as u8,
        };
    }
}

/// One of an effect's settings. Every value is a byte, like the pattern's command values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Param {
    pub name: &'static str,
    pub kind: ParamKind,
    pub default: u8,
    pub label: ValueLabel,
}

/// What a number stands for, shown after it in dialogs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueLabel {
    None,
    /// A share of `FF`.
    Percent,
    /// A signed scale where 0 leaves a value as it is.
    Scale,
    /// A length in ticks, as a note length where it is one.
    Ticks,
    /// Semitones, as an interval.
    Semitones,
}
impl ValueLabel {
    pub fn text(&self, value: i32) -> String {
        return match self {
            ValueLabel::None => String::new(),
            ValueLabel::Percent => percent(value, u8::MAX as i32),
            ValueLabel::Scale => percent(0x80 + value, 0x80),
            ValueLabel::Ticks => note_length(value),
            ValueLabel::Semitones => interval(value),
        };
    }
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
    /// Called on every clock pulse while playing, after the pulse's own messages. Notes due on
    /// this pulse are best started here: a note on the same pulse may replace them, and
    /// starting them in `tick` would have them cut off right away.
    fn after_pulse(&mut self, timing: Timing, out: &mut Vec<Message>) {}
    /// Forgets anything held, e.g. on stop, panic or when playback starts, writing the messages
    /// that release notes it still has sounding.
    fn reset(&mut self, out: &mut Vec<Message>) {}
}

/// An instrument's effects, each one feeding the next.
///
/// A parameter can follow a CC: when that CC reaches its effect, it sets the parameter instead
/// of going through the effect, so effects above it, like an LFO, can modulate it too. A CC
/// any parameter follows is never sent out. Modulated parameters go back to their own values
/// on reset.
#[derive(Default)]
pub struct MidiEffectChain {
    chain: Vec<ChainedEffect>,
    /// Every CC a parameter in the chain follows.
    mapped: Vec<u8>,
}

struct ChainedEffect {
    effect: Box<dyn MidiEffect>,
    /// A disabled effect keeps its place and settings, but lets every message through.
    enabled: bool,
    ccs: Vec<Option<u8>>,
    /// Each parameter's own value, to go back to after modulation.
    values: Vec<u8>,
}
impl ChainedEffect {
    /// Runs the messages through the effect, setting the parameters a CC among them maps to.
    fn process_all(&mut self, messages: Vec<Message>, timing: Option<Timing>) -> Vec<Message> {
        let mut out = vec![];
        for message in messages {
            if let Message::ControlChange(_, control, value) = message
                && self.ccs.contains(&Some(control))
            {
                self.modulate(control, value);
                out.push(message);
                continue;
            }
            match self.enabled {
                true => self.effect.process(message, timing, &mut out),
                false => out.push(message),
            }
        }
        return out;
    }

    fn modulate(&mut self, control: u8, value: u8) {
        let params = self.effect.params();
        for (index, cc) in self.ccs.iter().enumerate() {
            if *cc == Some(control)
                && let Some(param) = params.get(index)
            {
                self.effect.set_param(index, param.kind.cc_to_value(value));
            }
        }
    }

    fn restore(&mut self) {
        for index in 0..self.values.len() {
            self.restore_param(index);
        }
    }

    fn restore_param(&mut self, index: usize) {
        let value = self.values[index];
        if self.effect.param(index) != value {
            self.effect.set_param(index, value);
        }
    }
}

impl MidiEffectChain {
    pub fn new() -> Self {
        return Self {
            chain: Vec::new(),
            mapped: Vec::new(),
        };
    }

    #[cfg(test)]
    pub fn push(&mut self, effect: Box<dyn MidiEffect>) {
        self.add(effect, Vec::new(), true);
    }

    /// Turns an effect on or off in place, so the effects around it keep what they're
    /// playing. A disabled effect releases what it has sounding through the effects after it.
    pub fn push_settings(&mut self, settings: &EffectSettings) {
        let effect = create_effect(settings);
        self.add(effect, settings.param_ccs.clone(), settings.enabled);
    }

    /// The removed effect releases what it has sounding through the effects after it.
    pub fn remove(&mut self, effect: usize, out: &mut Vec<Message>) {
        if effect >= self.chain.len() {
            return;
        }
        let mut removed = self.chain.remove(effect);
        let mut messages = vec![];
        removed.effect.reset(&mut messages);
        for chained in &mut self.chain[effect..] {
            messages = chained.process_all(messages, None);
        }
        self.map_ccs();
        self.send(messages, out);
    }

    /// Nothing is reset, so every effect keeps what it's playing.
    pub fn move_effect(&mut self, from: usize, to: usize) {
        if from >= self.chain.len() || to >= self.chain.len() {
            return;
        }
        let chained = self.chain.remove(from);
        self.chain.insert(to, chained);
    }

    pub fn set_enabled(&mut self, effect: usize, enabled: bool, out: &mut Vec<Message>) {
        let Some(chained) = self.chain.get_mut(effect) else {
            return;
        };
        if chained.enabled == enabled {
            return;
        }
        chained.enabled = enabled;
        if enabled {
            return;
        }
        let mut messages = vec![];
        chained.effect.reset(&mut messages);
        for chained in &mut self.chain[effect + 1..] {
            messages = chained.process_all(messages, None);
        }
        self.send(messages, out);
    }

    /// Has a parameter follow a CC, or none. A parameter that stops following one goes back to
    /// its own value.
    pub fn set_param_cc(&mut self, effect: usize, param: usize, cc: Option<u8>) {
        let Some(chained) = self.chain.get_mut(effect) else {
            return;
        };
        if param >= chained.values.len() {
            return;
        }
        if chained.ccs.len() <= param {
            chained.ccs.resize(param + 1, None);
        }
        chained.ccs[param] = cc;
        chained.restore_param(param);
        self.map_ccs();
    }

    pub fn is_empty(&self) -> bool {
        return self.chain.is_empty();
    }

    pub fn len(&self) -> usize {
        return self.chain.len();
    }

    pub fn get(&self, index: usize) -> Option<&dyn MidiEffect> {
        return self.chain.get(index).map(|chained| chained.effect.as_ref());
    }

    /// Sets a parameter of the effect at `effect`, clamped to what the parameter takes. This
    /// is its own value, which modulation goes back to. Indices out of range are ignored.
    pub fn set_param(&mut self, effect: usize, param: usize, value: u8) {
        let Some(chained) = self.chain.get_mut(effect) else {
            return;
        };
        let Some(kind) = chained.effect.params().get(param).map(|p| p.kind) else {
            return;
        };
        let value = kind.clamp(value);
        chained.effect.set_param(param, value);
        chained.values[param] = value;
    }

    pub fn process(&mut self, message: Message, timing: Option<Timing>, out: &mut Vec<Message>) {
        let mut messages = vec![message];
        for chained in &mut self.chain {
            messages = chained.process_all(messages, timing);
        }
        self.send(messages, out);
    }

    /// What an effect makes on a tick goes through the effects after it.
    pub fn tick(&mut self, timing: Timing, out: &mut Vec<Message>) {
        let mut messages = vec![];
        for chained in &mut self.chain {
            messages = chained.process_all(messages, Some(timing));
            if chained.enabled {
                chained.effect.tick(timing, &mut messages);
            }
        }
        self.send(messages, out);
    }

    /// What an effect makes after a pulse goes through the effects after it.
    pub fn after_pulse(&mut self, timing: Timing, out: &mut Vec<Message>) {
        let mut messages = vec![];
        for chained in &mut self.chain {
            messages = chained.process_all(messages, Some(timing));
            if chained.enabled {
                chained.effect.after_pulse(timing, &mut messages);
            }
        }
        self.send(messages, out);
    }

    /// The releases an effect writes go through the effects after it before those reset too.
    /// Modulated parameters go back to their own values.
    pub fn reset(&mut self, out: &mut Vec<Message>) {
        let mut messages = vec![];
        for chained in &mut self.chain {
            messages = chained.process_all(messages, None);
            chained.effect.reset(&mut messages);
        }
        for chained in &mut self.chain {
            chained.restore();
        }
        self.send(messages, out);
    }

    fn add(&mut self, effect: Box<dyn MidiEffect>, ccs: Vec<Option<u8>>, enabled: bool) {
        let values = (0..effect.params().len())
            .map(|i| effect.param(i))
            .collect();
        self.chain.push(ChainedEffect {
            effect,
            enabled,
            ccs,
            values,
        });
        self.map_ccs();
    }

    fn map_ccs(&mut self) {
        self.mapped.clear();
        for chained in &self.chain {
            for cc in chained.ccs.iter().flatten() {
                if !self.mapped.contains(cc) {
                    self.mapped.push(*cc);
                }
            }
        }
    }

    /// Sends what came out of the last effect, keeping back the CCs parameters follow.
    fn send(&self, messages: Vec<Message>, out: &mut Vec<Message>) {
        out.extend(messages.into_iter().filter(|message| {
            !matches!(message, Message::ControlChange(_, control, _) if self.mapped.contains(control))
        }));
    }
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

fn percent(value: i32, full: i32) -> String {
    return format!("{}%", (value * 100 + full / 2) / full);
}

/// Ticks as a note length, 24 to a beat, or nothing if they aren't a common one.
fn note_length(ticks: i32) -> String {
    let length = match ticks.abs() {
        2 => "1/32T",
        3 => "1/32",
        4 => "1/16T",
        6 => "1/16",
        8 => "1/8T",
        9 => "1/16.",
        12 => "1/8",
        16 => "1/4T",
        18 => "1/8.",
        24 => "1/4",
        32 => "1/2T",
        36 => "1/4.",
        48 => "1/2",
        64 => "1/1T",
        72 => "1/2.",
        96 => "1 bar",
        144 => "1/1.",
        192 => "2 bars",
        _ => return String::new(),
    };
    let sign = if ticks < 0 { "-" } else { "" };
    return format!("{sign}{length}");
}

fn interval(semitones: i32) -> String {
    const NAMES: [&str; SEMITONES_PER_OCTAVE as usize] = [
        "", "m2", "M2", "m3", "M3", "4th", "tritone", "5th", "m6", "M6", "m7", "M7",
    ];
    let octave = SEMITONES_PER_OCTAVE as u32;
    let (octaves, name) = (
        semitones.unsigned_abs() / octave,
        NAMES[(semitones.unsigned_abs() % octave) as usize],
    );
    let octaves = match octaves {
        0 => String::new(),
        1 => "oct".to_owned(),
        n => format!("{n} oct"),
    };
    let text = match (octaves.is_empty(), name.is_empty()) {
        (true, _) => name.to_owned(),
        (false, true) => octaves,
        (false, false) => format!("{octaves}+{name}"),
    };
    let sign = if semitones < 0 { "-" } else { "" };
    return format!("{sign}{text}");
}

#[cfg(test)]
mod tests {
    use strum::IntoEnumIterator;

    use super::*;
    use crate::engine::effects::eq::{FILTER_VELOCITY_PER_OCTAVE, LOW_FREQUENCY, MID_VELOCITY};

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
        let signed = ParamKind::SignedNumber { min: -12, max: 12 };
        assert_eq!(signed.clamp(0xF0), 0xF4);
        assert_eq!(signed.clamp(0x20), 12);
        assert_eq!(signed.clamp(0xFE), 0xFE);
        assert_eq!(ParamKind::Toggle.clamp(5), 1);
        assert_eq!(ParamKind::Choice(&["a", "b", "c"]).clamp(7), 2);
        assert_eq!(ParamKind::Pitch.clamp(0xFF), PITCH_MAX);
    }

    #[test]
    fn chain_sets_clamped_params_and_ignores_bad_indices() {
        let mut chain = MidiEffectChain::new();
        chain.push(create_effect(&default_settings(EffectKind::Eq)));
        chain.set_param(0, LOW_FREQUENCY, 0xFF);
        assert_eq!(chain.get(0).unwrap().param(LOW_FREQUENCY), PITCH_MAX);
        chain.set_param(0, 99, 1);
        chain.set_param(5, FILTER_VELOCITY_PER_OCTAVE, 1);
        assert_eq!(chain.get(0).unwrap().param(FILTER_VELOCITY_PER_OCTAVE), 8);
    }

    #[test]
    fn every_effect_starts_at_its_defaults_and_keeps_what_is_set() {
        for kind in EffectKind::iter() {
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
            effect.param(MID_VELOCITY),
            defaults[MID_VELOCITY],
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
        let saved = set_saved_param(&mut settings, LOW_FREQUENCY, 0xFF);
        assert_eq!(saved, Some(PITCH_MAX));
        assert_eq!(settings.values[LOW_FREQUENCY], PITCH_MAX);
    }

    #[test]
    fn saved_params_fill_in_the_defaults_of_missing_values() {
        let mut settings = EffectSettings::new(EffectKind::Eq, vec![30]);
        assert_eq!(
            set_saved_param(&mut settings, MID_VELOCITY, 0x40),
            Some(0x40)
        );
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
        quiet.values[MID_VELOCITY] = -0x40i8 as u8;
        let chain = create_chain(&[default_settings(EffectKind::Eq), quiet]);
        assert_eq!(chain.len(), 2);
        assert_eq!(chain.get(0).unwrap().param(MID_VELOCITY), 0);
        assert_eq!(chain.get(1).unwrap().param(MID_VELOCITY), -0x40i8 as u8);
    }

    #[test]
    fn disabled_effects_keep_their_place_and_let_messages_through() {
        let mut quiet = default_settings(EffectKind::Eq);
        quiet.values[MID_VELOCITY] = i8::MIN as u8;
        quiet.enabled = false;
        let mut chain = create_chain(&[quiet]);
        assert_eq!(chain.len(), 1);
        chain.set_param(0, MID_VELOCITY, 0x40);
        assert_eq!(chain.get(0).unwrap().param(MID_VELOCITY), 0x40);
        let mut out = vec![];
        chain.process(note_on(60), None, &mut out);
        assert_eq!(out, vec![note_on(60)]);
    }

    #[test]
    fn turning_off_an_effect_keeps_the_arp_above_it_playing() {
        let chord = settings(EffectKind::Harmonize, &[(0, 4)]);
        let mut chain = create_chain(&[fifth_arp(), chord]);
        let mut out = vec![];
        chain.process(note_on(60), Some(Timing::new(0)), &mut out);
        assert_eq!(out, vec![note_on(60), note_on(64)]);
        out.clear();
        chain.set_enabled(1, false, &mut out);
        assert_eq!(
            out,
            vec![
                Message::NoteOff(Channel::Ch1, 60, 0),
                Message::NoteOff(Channel::Ch1, 64, 0),
            ]
        );
        out.clear();
        out.extend(run_pulse(&mut chain, 1));
        out.extend(run_pulse(&mut chain, 2));
        assert_eq!(
            out,
            vec![Message::NoteOff(Channel::Ch1, 60, 0), note_on(67)]
        );
        out.clear();
        chain.set_enabled(1, true, &mut out);
        assert!(out.is_empty());
        out.extend(run_pulse(&mut chain, 3));
        out.extend(run_pulse(&mut chain, 4));
        assert_eq!(
            out,
            vec![
                Message::NoteOff(Channel::Ch1, 67, 0),
                note_on(60),
                note_on(64)
            ]
        );
    }

    /// A held arp above a chord of a major third, one pulse in.
    fn arp_into_chord() -> MidiEffectChain {
        let chord = settings(EffectKind::Harmonize, &[(0, 4)]);
        let mut chain = create_chain(&[fifth_arp(), chord]);
        let mut out = vec![];
        chain.process(note_on(60), Some(Timing::new(0)), &mut out);
        assert_eq!(out, vec![note_on(60), note_on(64)]);
        return chain;
    }

    #[test]
    fn removing_an_effect_releases_its_notes_and_keeps_the_arp_playing() {
        let mut chain = arp_into_chord();
        let mut out = vec![];
        chain.remove(1, &mut out);
        assert_eq!(
            out,
            vec![
                Message::NoteOff(Channel::Ch1, 60, 0),
                Message::NoteOff(Channel::Ch1, 64, 0),
            ]
        );
        assert_eq!(chain.len(), 1);
        out.clear();
        out.extend(run_pulse(&mut chain, 1));
        out.extend(run_pulse(&mut chain, 2));
        assert_eq!(
            out,
            vec![Message::NoteOff(Channel::Ch1, 60, 0), note_on(67)]
        );
    }

    #[test]
    fn an_added_effect_changes_the_next_step_of_a_playing_arp() {
        let mut chain = create_chain(&[fifth_arp()]);
        let mut out = vec![];
        chain.process(note_on(60), Some(Timing::new(0)), &mut out);
        chain.push_settings(&settings(EffectKind::Harmonize, &[(0, 4)]));
        out.clear();
        out.extend(run_pulse(&mut chain, 1));
        out.extend(run_pulse(&mut chain, 2));
        assert_eq!(
            out,
            vec![
                Message::NoteOff(Channel::Ch1, 60, 0),
                note_on(67),
                note_on(71)
            ]
        );
    }

    #[test]
    fn moving_an_effect_keeps_everything_playing() {
        let mut chain = arp_into_chord();
        chain.move_effect(0, 1);
        let mut out = vec![];
        out.extend(run_pulse(&mut chain, 1));
        out.extend(run_pulse(&mut chain, 2));
        assert_eq!(
            out,
            vec![Message::NoteOff(Channel::Ch1, 60, 0), note_on(67)]
        );
        chain.move_effect(5, 0);
        assert_eq!(chain.len(), 2);
    }

    #[test]
    fn a_param_that_stops_following_a_cc_goes_back_to_its_value() {
        let mut chain = echo_on_cc_20();
        let mut out = vec![];
        chain.process(cc(20, 0), None, &mut out);
        chain.set_param_cc(0, echo::FEEDBACK_PARAM, None);
        assert_eq!(chain.get(0).unwrap().param(echo::FEEDBACK_PARAM), 0x80);
        chain.process(cc(20, 0), None, &mut out);
        assert_eq!(out, vec![cc(20, 0)]);
        chain.set_param_cc(0, echo::RATE_PARAM, Some(21));
        chain.process(cc(21, 0), None, &mut out);
        assert_eq!(chain.get(0).unwrap().param(echo::RATE_PARAM), 1);
        assert_eq!(out, vec![cc(20, 0)]);
    }

    fn settings(kind: EffectKind, values: &[(usize, u8)]) -> EffectSettings {
        let mut settings = default_settings(kind);
        for (param, value) in values {
            set_saved_param(&mut settings, *param, *value);
        }
        return settings;
    }

    /// An arp of steps 0 and 7, a step every 2 ticks.
    fn fifth_arp() -> EffectSettings {
        return settings(
            EffectKind::Arp,
            &[(1, 7), (arp::LENGTH_PARAM, 2), (arp::RATE_PARAM, 2)],
        );
    }

    /// A pulse as the engine runs it.
    fn run_pulse(chain: &mut MidiEffectChain, pulse: u32) -> Vec<Message> {
        let mut out = vec![];
        chain.tick(Timing::new(pulse), &mut out);
        chain.after_pulse(Timing::new(pulse), &mut out);
        return out;
    }

    #[test]
    fn steps_an_arp_plays_later_go_through_the_effects_after_it() {
        let quiet = settings(EffectKind::Eq, &[(MID_VELOCITY, -0x40i8 as u8)]);
        let mut chain = create_chain(&[fifth_arp(), quiet]);
        let mut out = vec![];
        chain.process(note_on(60), Some(Timing::new(0)), &mut out);
        assert_eq!(out, vec![Message::NoteOn(Channel::Ch1, 60, 50)]);
        run_pulse(&mut chain, 1);
        assert_eq!(
            run_pulse(&mut chain, 2),
            vec![
                Message::NoteOff(Channel::Ch1, 60, 0),
                Message::NoteOn(Channel::Ch1, 67, 50),
            ]
        );
    }

    #[test]
    fn a_chord_into_an_arp_gives_each_chord_note_its_own_arp() {
        let chord = settings(EffectKind::Harmonize, &[(0, 4)]);
        let mut chain = create_chain(&[chord, fifth_arp()]);
        let mut out = vec![];
        chain.process(note_on(60), Some(Timing::new(0)), &mut out);
        assert_eq!(out, vec![note_on(60), note_on(64)]);
        run_pulse(&mut chain, 1);
        let stepped = run_pulse(&mut chain, 2);
        assert!(stepped.contains(&note_on(67)));
        assert!(stepped.contains(&note_on(71)));
        out.clear();
        chain.process(Message::NoteOff(Channel::Ch1, 60, 0), None, &mut out);
        out.sort_by_key(|message| note_event(message).unwrap().pitch.0);
        assert_eq!(
            out,
            vec![
                Message::NoteOff(Channel::Ch1, 67, 0),
                Message::NoteOff(Channel::Ch1, 71, 0),
            ]
        );
    }

    #[test]
    fn a_disabled_arp_lets_notes_through_while_playing() {
        let mut arp = fifth_arp();
        arp.enabled = false;
        let mut chain = create_chain(&[arp]);
        let mut out = vec![];
        chain.process(note_on(60), Some(Timing::new(0)), &mut out);
        for pulse in 1..=4 {
            out.extend(run_pulse(&mut chain, pulse));
        }
        chain.process(
            Message::NoteOff(Channel::Ch1, 60, 0),
            Some(Timing::new(4)),
            &mut out,
        );
        assert_eq!(
            out,
            vec![note_on(60), Message::NoteOff(Channel::Ch1, 60, 0)]
        );
    }

    fn cc(control: u8, value: u8) -> Message {
        return Message::ControlChange(Channel::Ch1, control, value);
    }

    /// An echo whose feedback follows CC 20.
    fn echo_on_cc_20() -> MidiEffectChain {
        let mut settings = default_settings(EffectKind::Echo);
        settings.param_ccs = vec![None, Some(20)];
        return create_chain(&[settings]);
    }

    #[test]
    fn a_mapped_cc_sets_its_param_and_is_not_sent() {
        let mut chain = echo_on_cc_20();
        let mut out = vec![];
        chain.process(cc(20, 0x7F), None, &mut out);
        chain.process(cc(21, 5), None, &mut out);
        assert_eq!(out, vec![cc(21, 5)]);
        assert_eq!(chain.get(0).unwrap().param(echo::FEEDBACK_PARAM), 0xFF);
        chain.process(cc(20, 0), None, &mut out);
        assert_eq!(chain.get(0).unwrap().param(echo::FEEDBACK_PARAM), 0);
    }

    #[test]
    fn modulation_is_undone_on_reset_but_set_params_stay() {
        let mut chain = echo_on_cc_20();
        let mut out = vec![];
        chain.process(cc(20, 0), None, &mut out);
        chain.reset(&mut out);
        assert_eq!(chain.get(0).unwrap().param(echo::FEEDBACK_PARAM), 0x80);
        chain.set_param(0, echo::FEEDBACK_PARAM, 0x10);
        chain.reset(&mut out);
        assert_eq!(chain.get(0).unwrap().param(echo::FEEDBACK_PARAM), 0x10);
    }

    #[test]
    fn an_lfo_above_modulates_the_effects_below_it() {
        let lfo = settings(
            EffectKind::Lfo,
            &[
                (lfo::TARGET_PARAM, 2),
                (lfo::CC_PARAM, 20),
                (lfo::SHAPE_PARAM, 4),
                (lfo::DEPTH_PARAM, 0x7F),
            ],
        );
        let mut echo = default_settings(EffectKind::Echo);
        echo.param_ccs = vec![None, Some(20)];
        let mut chain = create_chain(&[lfo, echo]);
        let mut out = vec![];
        chain.process(
            Message::NoteOn(Channel::Ch1, 60, 100),
            Some(Timing::new(0)),
            &mut out,
        );
        out.extend(run_pulse(&mut chain, 1));
        assert!(
            out.iter()
                .all(|message| !matches!(message, Message::ControlChange(..))),
            "{out:?}"
        );
        assert_ne!(chain.get(1).unwrap().param(echo::FEEDBACK_PARAM), 0x80);
    }

    #[test]
    fn a_cc_reaches_only_the_effects_below_whatever_sends_it() {
        let mut echo = default_settings(EffectKind::Echo);
        echo.param_ccs = vec![None, Some(20)];
        let lfo = settings(
            EffectKind::Lfo,
            &[
                (lfo::TARGET_PARAM, 2),
                (lfo::CC_PARAM, 20),
                (lfo::SHAPE_PARAM, 4),
            ],
        );
        let mut chain = create_chain(&[echo, lfo]);
        let mut out = vec![];
        chain.process(
            Message::NoteOn(Channel::Ch1, 60, 100),
            Some(Timing::new(0)),
            &mut out,
        );
        out.extend(run_pulse(&mut chain, 1));
        assert_eq!(chain.get(0).unwrap().param(echo::FEEDBACK_PARAM), 0x80);
        assert!(
            out.iter()
                .all(|message| !matches!(message, Message::ControlChange(..))),
            "{out:?}"
        );
    }

    #[test]
    fn cc_values_spread_over_each_params_range() {
        let rate = ParamKind::Number { min: 1, max: 0xFF };
        assert_eq!(
            (
                rate.cc_to_value(0),
                rate.cc_to_value(0x40),
                rate.cc_to_value(0x7F)
            ),
            (1, 0x81, 0xFF)
        );
        let signed = ParamKind::SignedNumber {
            min: i8::MIN,
            max: i8::MAX,
        };
        assert_eq!(signed.cc_to_value(0) as i8, i8::MIN);
        assert_eq!(signed.cc_to_value(0x7F) as i8, i8::MAX);
        assert_eq!(
            (
                ParamKind::Toggle.cc_to_value(0x3F),
                ParamKind::Toggle.cc_to_value(0x40)
            ),
            (0, 1)
        );
        let choice = ParamKind::Choice(&["a", "b", "c"]);
        assert_eq!(
            (
                choice.cc_to_value(0),
                choice.cc_to_value(0x40),
                choice.cc_to_value(0x7F)
            ),
            (0, 1, 2)
        );
        assert_eq!(ParamKind::Pitch.cc_to_value(0xFF), PITCH_MAX);
    }

    #[test]
    fn value_labels_say_what_numbers_stand_for() {
        assert_eq!(ValueLabel::Percent.text(0x80), "50%");
        assert_eq!(ValueLabel::Percent.text(0xFF), "100%");
        assert_eq!(ValueLabel::Scale.text(0), "100%");
        assert_eq!(ValueLabel::Scale.text(-0x80), "0%");
        assert_eq!(ValueLabel::Scale.text(0x7F), "199%");
        assert_eq!(ValueLabel::Ticks.text(6), "1/16");
        assert_eq!(ValueLabel::Ticks.text(8), "1/8T");
        assert_eq!(ValueLabel::Ticks.text(36), "1/4.");
        assert_eq!(ValueLabel::Ticks.text(-12), "-1/8");
        assert_eq!(ValueLabel::Ticks.text(7), "");
        assert_eq!(ValueLabel::Semitones.text(7), "5th");
        assert_eq!(ValueLabel::Semitones.text(-12), "-oct");
        assert_eq!(ValueLabel::Semitones.text(13), "oct+m2");
        assert_eq!(ValueLabel::Semitones.text(28), "2 oct+M3");
        assert_eq!(ValueLabel::Semitones.text(0), "");
        assert_eq!(ValueLabel::None.text(5), "");
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
