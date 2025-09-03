use std::{fs, path::PathBuf};

#[derive(Clone)]
pub struct Config {
    pub project_dir: PathBuf,
}
impl Config {
    pub fn default() -> Self {
        return Self {
            project_dir: dirs::data_dir().unwrap().join("mTrak/Projects"),
        };
    }
    pub fn ensure_created(&self) {
        fs::create_dir(&self.project_dir).unwrap();
    }
}
