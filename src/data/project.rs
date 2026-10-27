use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{
    instrument::Instrument,
    pattern::{Pattern, PatternStore},
};
use crate::{
    data::{
        midi::{DEFAULT_BPM, DEFAULT_SPEED, MIDI_CHANNEL_MAX},
        pattern::PatternId,
    },
    error::{MTrakError, Result},
};
/// A project file starts with these bytes, then the format version as a little-endian u16,
/// then the payload's length in bytes as a little-endian u32, then the payload itself.
/// Reading the version before the payload lets a later version pick how to decode it.
const PROJECT_HEADER: [u8; 4] = *b"MTRK";
const PROJECT_VERSION: u16 = 2;
const VERSION_END: usize = PROJECT_HEADER.len() + 2;
const LENGTH_END: usize = VERSION_END + 4;
#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct SequenceRow {
    pub pattern_id: PatternId,
    pub repeats: u8,
}
impl SequenceRow {
    pub fn increase_repeats(&mut self) {
        self.repeats = self.repeats.saturating_add(1).min(8);
    }
    pub fn decrease_repeats(&mut self) {
        self.repeats = self.repeats.saturating_sub(1).max(1);
    }

    pub fn increase_pattern(&mut self) -> PatternId {
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
    pub name: Option<String>,
    pub bpm: u8,
    /// Ticks per row, so how many rows play per beat: 6 is 4 rows per beat.
    pub speed: u8,
    pub global_velocity: u8,
    /// Semitones added to every note.
    pub transpose: i8,
    pub sequence: Vec<SequenceRow>,
    pub patterns: PatternStore,
    pub instruments: [Instrument; MIDI_CHANNEL_MAX as usize + 1],
    pub midi_input: Option<String>,
    pub clock_input: Option<String>,
    pub record_input_velocity: bool,
    pub midi_output: Option<String>,
    pub send_midi_clock: bool,
}
impl Project {
    pub fn empty() -> Self {
        return Project {
            name: None,
            bpm: DEFAULT_BPM,
            speed: DEFAULT_SPEED,
            global_velocity: 0xFF,
            transpose: 0,
            sequence: vec![SequenceRow {
                pattern_id: PatternId(0),
                repeats: 1,
            }],
            instruments: std::array::from_fn(|_| Instrument::default()),
            patterns: PatternStore::new(vec![Pattern::default()]),
            midi_input: None,
            clock_input: None,
            record_input_velocity: false,
            midi_output: None,
            send_midi_clock: false,
        };
    }
    pub fn load(project_file_path: PathBuf) -> Result<Self> {
        return Self::decode(&std::fs::read(project_file_path)?);
    }
    fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.is_empty() {
            return Err(MTrakError::InvalidProject("the file is empty".into()));
        }
        if !bytes.starts_with(&PROJECT_HEADER) {
            return Err(MTrakError::InvalidProject(
                "the file has no mTrak header".into(),
            ));
        }
        let Some(header) = bytes.get(..LENGTH_END) else {
            return Err(MTrakError::InvalidProject("the file is cut short".into()));
        };
        let version = u16::from_le_bytes([header[4], header[5]]);
        if version != PROJECT_VERSION {
            return Err(MTrakError::UnsupportedProjectVersion {
                found: version,
                supported: PROJECT_VERSION,
            });
        }
        let length = u32::from_le_bytes(header[VERSION_END..LENGTH_END].try_into().unwrap());
        let Some(payload) = bytes[LENGTH_END..].get(..length as usize) else {
            return Err(MTrakError::InvalidProject("the file is cut short".into()));
        };
        let (project, _): (Project, usize) =
            bincode::serde::decode_from_slice(payload, bincode::config::standard())?;
        return Ok(project);
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
    pub fn save(&mut self, file_path: PathBuf) -> Result<()> {
        std::fs::write(file_path, self.encode()?)?;
        return Ok(());
    }
    fn encode(&self) -> Result<Vec<u8>> {
        let payload = bincode::serde::encode_to_vec(self, bincode::config::standard())?;
        let mut bytes = Vec::with_capacity(LENGTH_END + payload.len());
        bytes.extend_from_slice(&PROJECT_HEADER);
        bytes.extend_from_slice(&PROJECT_VERSION.to_le_bytes());
        // a project is a few MB at most, far below u32::MAX
        bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&payload);
        return Ok(bytes);
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

    fn temp_file(name: &str, contents: &[u8]) -> PathBuf {
        let path = std::env::temp_dir().join(format!("mtrak-test-{}-{}", std::process::id(), name));
        std::fs::write(&path, contents).unwrap();
        return path;
    }

    #[test]
    fn save_then_load_round_trips() {
        let path = temp_file("round-trip.mtrak", b"");
        let mut project = Project::empty();
        project.bpm = 140;
        project.save(path.clone()).unwrap();
        let loaded = Project::load(path.clone()).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(loaded.bpm, 140);
    }

    #[test]
    fn midi_output_round_trips() {
        let path = temp_file("midi-output.mtrak", b"");
        let mut project = Project::empty();
        project.midi_output = Some("Synth".into());
        project.save(path.clone()).unwrap();
        let loaded = Project::load(path.clone()).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(loaded.midi_output.as_deref(), Some("Synth"));
    }

    #[test]
    fn load_rejects_files_that_are_not_projects() {
        let empty = temp_file("empty.mtrak", b"");
        let wrong_header = temp_file("wrong-header.mtrak", b"RIFF1234");
        let truncated = temp_file("truncated.mtrak", b"MTRK\x01");
        let results = [
            Project::load(empty.clone()),
            Project::load(wrong_header.clone()),
            Project::load(truncated.clone()),
        ];
        for path in [empty, wrong_header, truncated] {
            std::fs::remove_file(path).unwrap();
        }
        assert!(matches!(results[0], Err(MTrakError::InvalidProject(_))));
        assert!(matches!(results[1], Err(MTrakError::InvalidProject(_))));
        assert!(matches!(results[2], Err(MTrakError::InvalidProject(_))));
    }

    #[test]
    fn file_starts_with_header_version_and_payload_length() {
        let bytes = Project::empty().encode().unwrap();
        assert_eq!(&bytes[..4], b"MTRK");
        assert_eq!(u16::from_le_bytes([bytes[4], bytes[5]]), PROJECT_VERSION);
        let length = u32::from_le_bytes(bytes[6..10].try_into().unwrap());
        assert_eq!(length as usize, bytes.len() - LENGTH_END);
    }

    #[test]
    fn decode_rejects_other_versions() {
        for version in [PROJECT_VERSION - 1, PROJECT_VERSION + 1] {
            let mut bytes = Project::empty().encode().unwrap();
            bytes[4..6].copy_from_slice(&version.to_le_bytes());
            assert!(matches!(
                Project::decode(&bytes),
                Err(MTrakError::UnsupportedProjectVersion { found, .. }) if found == version
            ));
        }
    }

    #[test]
    fn decode_rejects_a_payload_shorter_than_its_length() {
        let mut bytes = Project::empty().encode().unwrap();
        bytes.pop();
        assert!(matches!(
            Project::decode(&bytes),
            Err(MTrakError::InvalidProject(_))
        ));
    }

    #[test]
    fn load_reports_missing_files_as_io_errors() {
        let path = std::env::temp_dir().join("mtrak-test-does-not-exist.mtrak");
        assert!(matches!(Project::load(path), Err(MTrakError::Io(_))));
    }

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
        assert_eq!(row.increase_pattern().0, u8::MAX);
    }
}
