use serde::{Deserialize, Serialize};

pub trait TEffect {}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Effect {
    SetVolume { new_vol: u8 },
    CutNote,
}
