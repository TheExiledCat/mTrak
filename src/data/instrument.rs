use serde::{Deserialize, Serialize};

pub const DEFAULT_PITCH_WHEEL_RANGE: u8 = 12;

#[derive(Serialize, Deserialize, Clone)]
pub struct Instrument {
    pub name: String,
    pub volume: u8,
    pub pitch_wheel_range: u8,
    /// The CC that `C`, `G` and `H` commands change on a track without an `S`.
    pub default_cc: u8,
    /// Semitones added to every note the instrument plays, on top of the song's transpose.
    pub transpose: i8,
}
impl Default for Instrument {
    fn default() -> Self {
        return Self {
            name: String::new(),
            volume: 0xFF,
            pitch_wheel_range: DEFAULT_PITCH_WHEEL_RANGE,
            default_cc: 0,
            transpose: 0,
        };
    }
}
