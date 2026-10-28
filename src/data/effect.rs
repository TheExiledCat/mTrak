use serde::{Deserialize, Serialize};
use strum::EnumIter;

/// Every effect an instrument can have. Saved by name, so variants can be reordered.
/// `EffectKind::iter()` lists the ones that can be added; disabled ones are WIP.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, EnumIter)]
pub enum EffectKind {
    Eq,
    Arp,
    Harmonize,
}
impl EffectKind {
    pub fn name(&self) -> &'static str {
        return match self {
            EffectKind::Eq => "EQ",
            EffectKind::Arp => "Arp",
            EffectKind::Harmonize => "Chord",
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
}
impl EffectSettings {
    pub fn new(kind: EffectKind, values: Vec<u8>) -> Self {
        return Self {
            kind,
            values,
            enabled: true,
        };
    }
}
