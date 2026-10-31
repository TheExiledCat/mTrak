use serde::{Deserialize, Serialize};
use strum::EnumIter;

use crate::data::midi::MIDI_CHANNEL_MAX;

/// Saved by name, so variants can be reordered.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, EnumIter)]
pub enum MetronomeMode {
    Off,
    Recording,
    Always,
}
impl MetronomeMode {
    pub fn name(&self) -> &'static str {
        return match self {
            MetronomeMode::Off => "Off",
            MetronomeMode::Recording => "While recording",
            MetronomeMode::Always => "Always",
        };
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Metronome {
    pub mode: MetronomeMode,
    /// A bit per channel, from bit 0 for channel 1.
    pub channels: u16,
    pub accent_note: u8,
    pub accent_velocity: u8,
    pub beat_note: u8,
    pub beat_velocity: u8,
    pub beats_per_bar: u8,
}
impl Metronome {
    pub fn new() -> Self {
        return Self {
            mode: MetronomeMode::Off,
            channels: 1 << DRUM_CHANNEL,
            accent_note: 76,
            accent_velocity: 0x7F,
            beat_note: 77,
            beat_velocity: 0x60,
            beats_per_bar: 4,
        };
    }

    pub fn sends_to(&self, channel: u8) -> bool {
        return channel <= MIDI_CHANNEL_MAX && self.channels & (1 << channel) != 0;
    }
    pub fn set_channel(&mut self, channel: u8, on: bool) {
        if channel > MIDI_CHANNEL_MAX {
            return;
        }
        match on {
            true => self.channels |= 1 << channel,
            false => self.channels &= !(1 << channel),
        }
    }
}
impl Default for Metronome {
    fn default() -> Self {
        return Self::new();
    }
}

/// Channel 10, the usual drum channel. The default notes are wood blocks there.
const DRUM_CHANNEL: u8 = 9;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_metronome_clicks_on_the_drum_channel_only() {
        let metronome = Metronome::new();
        assert!(metronome.sends_to(DRUM_CHANNEL));
        assert!(
            (0..=MIDI_CHANNEL_MAX)
                .filter(|&c| metronome.sends_to(c))
                .count()
                == 1
        );
    }

    #[test]
    fn channels_turn_on_and_off() {
        let mut metronome = Metronome::new();
        metronome.set_channel(0, true);
        metronome.set_channel(DRUM_CHANNEL, false);
        assert!(metronome.sends_to(0));
        assert!(!metronome.sends_to(DRUM_CHANNEL));
        metronome.set_channel(16, true);
        assert!(!metronome.sends_to(16));
    }
}
