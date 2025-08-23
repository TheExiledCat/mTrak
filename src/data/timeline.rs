use super::pattern::Pattern;

pub struct Timeline {
    pub patterns: Vec<Pattern>,
}
impl Timeline {
    pub fn new() -> Self {
        return Self { patterns: vec![] };
    }
}
