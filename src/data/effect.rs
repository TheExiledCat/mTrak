use serde::{Deserialize, Serialize};
use strum::EnumIter;

/// Every effect an instrument can have. Saved by name, so variants can be reordered.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, EnumIter)]
pub enum EffectKind {
    Eq,
    Arp,
    Harmonize,
    Lfo,
    Echo,
    Envelope,
}
impl EffectKind {
    pub fn name(&self) -> &'static str {
        return match self {
            EffectKind::Eq => "EQ",
            EffectKind::Arp => "Arp",
            EffectKind::Harmonize => "Chord",
            EffectKind::Lfo => "LFO",
            EffectKind::Echo => "Echo",
            EffectKind::Envelope => "Envelope",
        };
    }
}

/// An effect in an instrument's chain as it's saved: its kind and the value of each of its
/// parameters, in the order the effect lists them.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct EffectSettings {
    pub kind: EffectKind,
    pub values: Vec<u8>,
    pub enabled: bool,
    /// The CC that modulates each parameter, in the same order as `values`. Parameters past
    /// the end have none.
    pub param_ccs: Vec<Option<u8>>,
}
impl EffectSettings {
    pub fn new(kind: EffectKind, values: Vec<u8>) -> Self {
        return Self {
            kind,
            values,
            enabled: true,
            param_ccs: Vec::new(),
        };
    }

    pub fn param_cc(&self, param: usize) -> Option<u8> {
        return self.param_ccs.get(param).copied().flatten();
    }
}
