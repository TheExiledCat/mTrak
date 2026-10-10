use serde::{Deserialize, Serialize};

use super::pattern::Pattern;
#[derive(Serialize, Deserialize)]
pub struct Timeline {
    pub patterns: Vec<Pattern>,
}
impl Default for Timeline {
    fn default() -> Self {
        return Self::new();
    }
}

impl Timeline {
    pub fn new() -> Self {
        return Self { patterns: vec![] };
    }
}
