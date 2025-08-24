use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{
    pattern::{Pattern, PatternStore},
    timeline::Timeline,
};
const PROJECT_HEADER: [u8; 4] = *b"MTRK";
#[derive(Serialize, Deserialize)]
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
            timeline: Timeline::new(),
            patterns: vec![],
        };
    }
    pub fn pattern_store(&mut self) -> PatternStore {
        return PatternStore::new(&mut self.patterns);
    }
}
