use std::{fs, path::PathBuf};

pub const TITLE: &str = "mTrak";
#[derive(Clone)]
pub struct Config {
    pub project_dir: PathBuf,
}
impl Default for Config {
    fn default() -> Self {
        return Self {
            project_dir: dirs::data_dir().unwrap().join("mTrak/Projects"),
        };
    }
}
impl Config {
    pub fn ensure_created(&self) {
        fs::create_dir(&self.project_dir).unwrap();
    }
}
