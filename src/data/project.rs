use std::{
    arch::naked_asm,
    fs::File,
    io::{Error, Read, Write},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

use super::{
    pattern::{Pattern, PatternStore},
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
    #[serde(skip)]
    project_file: PathBuf,
}
impl Project {
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
            project.project_file = project_file;
            return project;
        }
        let mut project = Self {
            version: PROJECT_VERSION,
            timeline: Timeline::new(),
            patterns: vec![],
            project_file,
            name: None,
        };
        project.pattern_store().new_pattern(32, 4, 4);
        return project;
    }
    pub fn pattern_store(&mut self) -> PatternStore {
        return PatternStore::new(&mut self.patterns);
    }
    pub fn save(&mut self, name: Option<String>) -> Result<(), Error> {
        if let Some(name) = name {
            self.name = Some(name);
        }
        let mut file = File::create(&self.project_file)?;
        file.write_all(&PROJECT_HEADER)?;
        let _bin =
            bincode::serde::encode_into_std_write(self, &mut file, bincode::config::standard())
                .unwrap();

        return Ok(());
    }
}
