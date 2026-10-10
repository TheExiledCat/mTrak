use std::{
    fs::File,
    io::{Error, Read, Write},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

use crate::data::{midi::MIDI_CHANNEL_MAX, pattern::PatternId};

use super::{
    config::Config,
    pattern::{Pattern, PatternStore},
};
const PROJECT_HEADER: [u8; 4] = *b"MTRK";
const PROJECT_VERSION: u16 = 1;
#[derive(Serialize, Deserialize, Clone)]
pub struct SequenceRow {
    pub pattern_id: PatternId,
    pub repeats: u8,
}
impl SequenceRow {
    pub fn increase_repeats(&mut self) {
        self.repeats = self.repeats.saturating_add(1);
    }
    pub fn decrease_repeats(&mut self) {
        self.repeats = self.repeats.saturating_sub(1).clamp(1, u8::MAX);
    }

    pub fn increate_pattern(&mut self) -> PatternId {
        self.pattern_id.0 = self.pattern_id.0.saturating_add(1);
        return self.pattern_id;
    }

    pub fn decrease_pattern(&mut self) -> PatternId {
        self.pattern_id.0 = self.pattern_id.0.saturating_sub(1);
        return self.pattern_id;
    }
}
#[derive(Serialize, Deserialize)]
pub struct Project {
    pub version: u16,
    pub name: Option<String>,
    pub sequence: Vec<SequenceRow>,
    pub patterns: PatternStore,
    pub instruments: [String; MIDI_CHANNEL_MAX as usize + 1],
    pub global_instrument_volumes: [u8; MIDI_CHANNEL_MAX as usize + 1],
}
impl Project {
    pub fn empty() -> Self {
        return Project {
            version: PROJECT_VERSION,
            name: None,
            sequence: vec![SequenceRow {
                pattern_id: PatternId(0),
                repeats: 1,
            }],
            instruments: std::array::from_fn(|_| String::new()),
            patterns: PatternStore::new(vec![Pattern::default()]),
            global_instrument_volumes: [0xFF; _],
        };
    }
    pub fn new(project_file_path: PathBuf) -> Self {
        if project_file_path.is_file() {
            // load file and create project like that
            let mut file = File::open(&project_file_path).unwrap();
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
            let project: Project =
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
    pub fn song_length(&self) -> usize {
        return self.sequence.iter().map(|s| s.repeats as usize).sum();
    }
    pub fn insert_sequence(&mut self, index: usize, pattern_id: PatternId) {
        self.sequence.insert(
            index,
            SequenceRow {
                pattern_id,
                repeats: 1,
            },
        );
    }
    pub fn save(&mut self, config: &Config, name: Option<String>) -> Result<(), Error> {
        if let Some(name) = name.clone() {
            self.name = Some(name);
        } else {
            if let Some(_name) = self.name.clone() {
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

    pub fn delete_sequence(&mut self, sequence_index: usize) -> usize {
        if self.sequence.len() <= 1 {
            return self.sequence.len();
        };
        self.sequence.remove(sequence_index);
        return self.sequence.len();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn song_length_sums_repeats() {
        let mut project = Project::empty();
        project.sequence[0].repeats = 2;
        project.insert_sequence(1, PatternId(0));
        assert_eq!(project.song_length(), 3);
    }

    #[test]
    fn song_length_does_not_overflow_u8() {
        let mut project = Project::empty();
        project.sequence[0].repeats = 200;
        project.insert_sequence(1, PatternId(0));
        project.sequence[1].repeats = 200;
        assert_eq!(project.song_length(), 400);
    }

    #[test]
    fn insert_sequence_places_row_at_index() {
        let mut project = Project::empty();
        project.insert_sequence(0, PatternId(5));
        assert_eq!(project.sequence[0].pattern_id.0, 5);
        assert_eq!(project.sequence[1].pattern_id.0, 0);
    }

    #[test]
    fn last_sequence_row_cannot_be_deleted() {
        let mut project = Project::empty();
        assert_eq!(project.delete_sequence(0), 1);
        project.insert_sequence(1, PatternId(2));
        assert_eq!(project.delete_sequence(0), 1);
        assert_eq!(project.sequence[0].pattern_id.0, 2);
    }

    #[test]
    fn repeats_never_drop_below_one() {
        let mut row = SequenceRow {
            pattern_id: PatternId(0),
            repeats: 1,
        };
        row.decrease_repeats();
        assert_eq!(row.repeats, 1);
        row.increase_repeats();
        assert_eq!(row.repeats, 2);
    }

    #[test]
    fn pattern_id_saturates() {
        let mut row = SequenceRow {
            pattern_id: PatternId(0),
            repeats: 1,
        };
        assert_eq!(row.decrease_pattern().0, 0);
        row.pattern_id = PatternId(u8::MAX);
        assert_eq!(row.increate_pattern().0, u8::MAX);
    }
}
