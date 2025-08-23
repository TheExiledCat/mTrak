use std::path::PathBuf;

use super::{
    pattern::{self, Pattern},
    timeline::Timeline,
};

const PROJECT_HEADER: [u8; 4] = *b"MTRK";
pub struct Project {
    pub magic: [u8; 4],
    pub version: u16,

    pub timeline: Timeline,
    pub patterns: Vec<Pattern>,
}
impl Project {
    pub fn new(file: PathBuf) -> Self {
        return Self {
            magic: PROJECT_HEADER,
            version: 1,
            timeline: todo!(),
            patterns: vec![],
        };
    }
    pub fn pattern_store(&self) -> PatternStore {
        return PatternStore::new(&self.patterns);
    }
}
