use serde::{Deserialize, Serialize};

/// Every effect an instrument can have. Saved by name, so variants can be reordered.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectKind {
    Eq,
}
impl EffectKind {
    pub const ALL: [EffectKind; 1] = [EffectKind::Eq];

    pub fn name(&self) -> &'static str {
        return match self {
            EffectKind::Eq => "EQ",
        };
    }
}

/// An effect in an instrument's chain as it's saved: its kind and the value of each of its
/// parameters, in the order the effect lists them.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct EffectSettings {
    pub kind: EffectKind,
    pub values: Vec<u8>,
}
impl EffectSettings {
    pub fn new(kind: EffectKind, values: Vec<u8>) -> Self {
        return Self { kind, values };
    }
}
