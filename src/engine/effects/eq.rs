use midi::Message;

use crate::{
    data::{
        midi::scale_velocity,
        note::{NotePitch, SEMITONES_PER_OCTAVE},
    },
    engine::effects::{MidiEffect, Param, ParamKind, Timing, ValueLabel, note_event},
};

/// Band velocities are signed: 0 leaves notes unchanged, -128 silences them and 127 about
/// doubles them. This is added to get the scale for `scale_velocity`.
const BAND_UNITY: u8 = 0x80;
const BAND: ParamKind = ParamKind::SignedNumber {
    min: i8::MIN,
    max: i8::MAX,
};
const BYTE: ParamKind = ParamKind::Number {
    min: 0,
    max: u8::MAX,
};

pub const LOW_FREQUENCY: usize = 0;
pub const HIGH_FREQUENCY: usize = 1;
pub const LOW_VELOCITY: usize = 2;
pub const MID_VELOCITY: usize = 3;
pub const HIGH_VELOCITY: usize = 4;
pub const HIGH_PASS_CUTOFF: usize = 5;
pub const LOW_PASS_CUTOFF: usize = 6;
pub const FILTER_VELOCITY_PER_OCTAVE: usize = 7;

const PARAMS: &[Param] = &[
    Param {
        name: "Low band up to",
        kind: ParamKind::Pitch,
        default: NotePitch::MAX / 3,
        label: ValueLabel::None,
    },
    Param {
        name: "Mid band up to",
        kind: ParamKind::Pitch,
        default: NotePitch::MAX / 3 * 2,
        label: ValueLabel::None,
    },
    Param {
        name: "Low velocity",
        kind: BAND,
        default: 0,
        label: ValueLabel::Scale,
    },
    Param {
        name: "Mid velocity",
        kind: BAND,
        default: 0,
        label: ValueLabel::Scale,
    },
    Param {
        name: "High velocity",
        kind: BAND,
        default: 0,
        label: ValueLabel::Scale,
    },
    Param {
        name: "High pass",
        kind: ParamKind::Pitch,
        default: 0,
        label: ValueLabel::None,
    },
    Param {
        name: "Low pass",
        kind: ParamKind::Pitch,
        default: NotePitch::MAX,
        label: ValueLabel::None,
    },
    Param {
        name: "Filter slope",
        kind: BYTE,
        default: 8,
        label: ValueLabel::None,
    },
];

pub struct EqEffect {
    low_frequency: u8,
    high_frequency: u8,
    /// Scales the velocity of notes in each band, 0 leaves them unchanged.
    low_velocity: i8,
    mid_velocity: i8,
    high_velocity: i8,
    /// Notes below this pitch lose `filter_velocity_per_octave` for each octave below it.
    high_pass_cutoff: u8,
    /// Notes above this pitch lose `filter_velocity_per_octave` for each octave above it.
    low_pass_cutoff: u8,
    filter_velocity_per_octave: u8,
}
impl EqEffect {
    pub fn new() -> Self {
        return Self {
            low_frequency: PARAMS[LOW_FREQUENCY].default,
            high_frequency: PARAMS[HIGH_FREQUENCY].default,
            low_velocity: PARAMS[LOW_VELOCITY].default as i8,
            mid_velocity: PARAMS[MID_VELOCITY].default as i8,
            high_velocity: PARAMS[HIGH_VELOCITY].default as i8,
            high_pass_cutoff: PARAMS[HIGH_PASS_CUTOFF].default,
            low_pass_cutoff: PARAMS[LOW_PASS_CUTOFF].default,
            filter_velocity_per_octave: PARAMS[FILTER_VELOCITY_PER_OCTAVE].default,
        };
    }

    /// The scale for `scale_velocity` of a pitch's band, out of `BAND_UNITY`.
    fn band_scale(&self, pitch: u8) -> u8 {
        let velocity = match pitch {
            pitch if pitch <= self.low_frequency => self.low_velocity,
            pitch if pitch <= self.high_frequency => self.mid_velocity,
            _ => self.high_velocity,
        };
        return BAND_UNITY.wrapping_add_signed(velocity);
    }

    /// The velocity the filters take off a pitch, per semitone past the cutoff and rounded up
    /// so a note just past it is already quieter.
    fn filter_cut(&self, pitch: u8) -> u8 {
        let below = self.high_pass_cutoff.saturating_sub(pitch);
        let above = pitch.saturating_sub(self.low_pass_cutoff);
        let semitones = below.max(above) as u16;
        return (semitones * self.filter_velocity_per_octave as u16)
            .div_ceil(SEMITONES_PER_OCTAVE as u16)
            .min(u8::MAX as u16) as u8;
    }
}
impl Default for EqEffect {
    fn default() -> Self {
        return Self::new();
    }
}
impl MidiEffect for EqEffect {
    fn params(&self) -> &'static [Param] {
        return PARAMS;
    }
    fn param(&self, index: usize) -> u8 {
        return match index {
            LOW_FREQUENCY => self.low_frequency,
            HIGH_FREQUENCY => self.high_frequency,
            LOW_VELOCITY => self.low_velocity as u8,
            MID_VELOCITY => self.mid_velocity as u8,
            HIGH_VELOCITY => self.high_velocity as u8,
            HIGH_PASS_CUTOFF => self.high_pass_cutoff,
            LOW_PASS_CUTOFF => self.low_pass_cutoff,
            FILTER_VELOCITY_PER_OCTAVE => self.filter_velocity_per_octave,
            _ => 0,
        };
    }
    fn set_param(&mut self, index: usize, value: u8) {
        match index {
            LOW_FREQUENCY => self.low_frequency = value,
            HIGH_FREQUENCY => self.high_frequency = value,
            LOW_VELOCITY => self.low_velocity = value as i8,
            MID_VELOCITY => self.mid_velocity = value as i8,
            HIGH_VELOCITY => self.high_velocity = value as i8,
            HIGH_PASS_CUTOFF => self.high_pass_cutoff = value,
            LOW_PASS_CUTOFF => self.low_pass_cutoff = value,
            FILTER_VELOCITY_PER_OCTAVE => self.filter_velocity_per_octave = value,
            _ => {}
        }
    }
    fn process(&mut self, message: Message, _timing: Option<Timing>, out: &mut Vec<Message>) {
        let Some(mut note) = note_event(&message) else {
            out.push(message);
            return;
        };
        // note offs always pass, so changing the settings while a note plays can't hang it
        if note.is_note_off {
            out.push(message);
            return;
        }
        let pitch = note.pitch.0;
        let velocity = scale_velocity(note.velocity, self.band_scale(pitch), BAND_UNITY);
        note.velocity = velocity.saturating_sub(self.filter_cut(pitch));
        if note.velocity == 0 {
            return;
        }
        out.push(note.to_message());
    }
}

#[cfg(test)]
mod tests {
    use midi::Channel;

    use super::*;

    fn play(eq: &mut EqEffect, message: Message) -> Vec<Message> {
        let mut out = vec![];
        eq.process(message, None, &mut out);
        return out;
    }

    fn note_on(pitch: u8, velocity: u8) -> Message {
        return Message::NoteOn(Channel::Ch1, pitch, velocity);
    }

    #[test]
    fn default_eq_leaves_notes_unchanged() {
        let mut eq = EqEffect::new();
        for pitch in [0, 40, 60, 100, 127] {
            assert_eq!(
                play(&mut eq, note_on(pitch, 100)),
                vec![note_on(pitch, 100)]
            );
        }
    }

    #[test]
    fn bands_scale_their_notes() {
        let mut eq = EqEffect::new();
        eq.low_velocity = -0x40;
        eq.high_velocity = 0x40;
        assert_eq!(play(&mut eq, note_on(30, 100)), vec![note_on(30, 50)]);
        assert_eq!(play(&mut eq, note_on(60, 100)), vec![note_on(60, 100)]);
        assert_eq!(play(&mut eq, note_on(100, 80)), vec![note_on(100, 120)]);
    }

    #[test]
    fn band_velocities_reach_silence_and_about_double() {
        let mut eq = EqEffect::new();
        eq.low_velocity = i8::MIN;
        eq.high_velocity = i8::MAX;
        assert!(play(&mut eq, note_on(30, 100)).is_empty());
        assert_eq!(play(&mut eq, note_on(100, 50)), vec![note_on(100, 100)]);
    }

    #[test]
    fn filters_cut_velocity_per_octave_past_the_cutoff() {
        let mut eq = EqEffect::new();
        eq.high_pass_cutoff = 48;
        eq.low_pass_cutoff = 72;
        eq.filter_velocity_per_octave = 24;
        assert_eq!(play(&mut eq, note_on(48, 100)), vec![note_on(48, 100)]);
        assert_eq!(play(&mut eq, note_on(36, 100)), vec![note_on(36, 76)]);
        assert_eq!(play(&mut eq, note_on(24, 100)), vec![note_on(24, 52)]);
        assert_eq!(play(&mut eq, note_on(78, 100)), vec![note_on(78, 88)]);
        assert_eq!(play(&mut eq, note_on(73, 100)), vec![note_on(73, 98)]);
    }

    #[test]
    fn fully_filtered_notes_are_dropped() {
        let mut eq = EqEffect::new();
        eq.high_pass_cutoff = 60;
        eq.filter_velocity_per_octave = 120;
        assert!(play(&mut eq, note_on(48, 100)).is_empty());
    }

    #[test]
    fn note_offs_always_pass() {
        let mut eq = EqEffect::new();
        eq.high_pass_cutoff = 60;
        eq.filter_velocity_per_octave = 120;
        eq.low_velocity = i8::MIN;
        let off = Message::NoteOff(Channel::Ch1, 30, 0);
        assert_eq!(play(&mut eq, off.clone()), vec![off]);
    }
}
