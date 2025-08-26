use serde::{Deserialize, Serialize};

use crate::tui::constants::MAX_TRACK_EFFECTS;

pub trait TEffect {}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Effect {
    SetVolume { new_vol: u8 },
    CutNote,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Effects {
    pub chain: [Option<Effect>; MAX_TRACK_EFFECTS],
}
impl Effects {}
impl ToString for Effects {
    fn to_string(&self) -> String {
        // TODO make real effects system
        return String::from("00");
    }
}
