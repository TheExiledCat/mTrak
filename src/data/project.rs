use std::{
    fs::File,
    io::{Error, Read, Write},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

use super::{
    config::Config,
    pattern::{Pattern, PatternStore, PatternStoreMut},
    timeline::Timeline,
};
const PROJECT_HEADER: [u8; 4] = *b"MTRK";
const PROJECT_VERSION: u16 = 1;
#[derive(Serialize, Deserialize)]
pub struct Project {
    pub version: u16,
    pub name: Option<String>,
    pub timeline: Timeline,
    pub patterns: Vec<Pattern>,
}
impl Project {
    pub fn empty() -> Self {
        return Project {
            version: PROJECT_VERSION,
            name: None,
            timeline: Timeline::new(),
            patterns: vec![Pattern::new(64, 4)],
        };
    }
    pub fn new(project_file: PathBuf) -> Self {
        if project_file.is_file() {
            // load file and create project like that
            let mut file = File::open(&project_file).unwrap();
            let mut magic = [0u8; 4];
            if file.metadata().unwrap().len() < 4 {
                panic!("Empty file");
            }
            file.read_exact(&mut magic).unwrap();
            if magic != PROJECT_HEADER {
                panic!("Not a real mtrak file");
            }

            let mut project_content: Vec<u8> = Vec::new();
            file.read_to_end(&mut project_content).unwrap();
            let mut project: Project =
                bincode::serde::decode_from_slice(&project_content, bincode::config::standard())
                    .unwrap()
                    .0;
            if project.version > PROJECT_VERSION {
                panic!(
                    "Cant open a version v{} file in version v{} of mtrak",
                    project.version, PROJECT_VERSION
                )
            }
            return project;
        }

        panic!("File not found");
    }
    pub fn pattern_store(&self) -> PatternStore {
        return PatternStore::new(&self.patterns);
    }
    pub fn pattern_store_mut(&mut self) -> PatternStoreMut {
        return PatternStoreMut::new(&mut self.patterns);
    }
    pub fn save(&mut self, config: &Config, name: Option<String>) -> Result<(), Error> {
        if let Some(name) = name.clone() {
            self.name = Some(name);
        } else {
            if let Some(name) = self.name.clone() {
                // all is good
            } else {
                // there is no name
                panic!("No name set for project");
            }
        }
        let file_path = config.project_dir.join(self.name.clone().unwrap());
        let mut file = File::create(file_path)?;
        file.write_all(&PROJECT_HEADER)?;
        let _bin =
            bincode::serde::encode_into_std_write(self, &mut file, bincode::config::standard())
                .unwrap();

        return Ok(());
    }
}
