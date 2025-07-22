pub trait TEffect {}
#[derive(Debug, Clone)]
pub enum Effect {
    SetVolume { new_vol: u8 },
    CutNote,
}
