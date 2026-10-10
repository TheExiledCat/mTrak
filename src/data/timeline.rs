use serde::{Deserialize, Serialize};

use super::pattern::Pattern;
#[derive(Serialize, Deserialize)]
pub struct Timeline {
    pub patterns: Vec<Pattern>,
}
impl Timeline {
    pub fn new() -> Self {
        return Self { patterns: vec![] };
    }
}
