use std::{collections::HashSet, path::PathBuf, time::Duration};

use serde::{Deserialize, Serialize};

use crate::{
    data::{
        commands::{CommandKind, PlaybackAction},
        instrument::Instrument,
        midi::{DEFAULT_BPM, DEFAULT_SPEED, MIDI_CHANNEL_MAX, PULSES_PER_QUARTER},
        migration::{self, FIRST_MESSAGEPACK_VERSION, PROJECT_VERSION},
        note::{Note, NoteKind},
        pattern::{Pattern, PatternId, PatternStore},
    },
    error::{MTrakError, Result},
};
/// A project file starts with these bytes, then the format version as a little-endian u16,
/// then the payload's length in bytes as a little-endian u32, then the payload itself: the
/// project as MessagePack, with structs written as maps of field names. Files of an older
/// version are upgraded by [`migration`] before they're read.
const PROJECT_HEADER: [u8; 4] = *b"MTRK";
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
/// Fields are saved in the order they're declared here. `patterns` stays last, so a program
/// can read everything else without going through the patterns.
#[derive(Serialize, Deserialize)]
pub struct Project {
    pub name: Option<String>,
    pub bpm: u8,
    /// Ticks per row, so how many rows play per beat: 6 is 4 rows per beat.
    pub speed: u8,
    pub global_velocity: u8,
    /// Semitones added to every note.
    pub transpose: i8,
    pub midi_input: Option<String>,
    pub clock_input: Option<String>,
    pub record_input_velocity: bool,
    pub midi_output: Option<String>,
    pub send_midi_clock: bool,
    pub sequence: Vec<SequenceRow>,
    pub instruments: [Instrument; MIDI_CHANNEL_MAX as usize + 1],
    pub patterns: PatternStore,
}
/// Numbers about a project, for the Info dialog.
pub struct ProjectStats {
    /// How big the project is saved, in bytes.
    pub file_size: usize,
    /// Different patterns the song plays.
    pub patterns_in_song: usize,
    /// Patterns the song plays that have nothing in them.
    pub empty_in_song: usize,
    /// Patterns with something in them that the song doesn't play.
    pub outside_song: usize,
    /// Rows the song plays, counting repeats.
    pub song_rows: u32,
    /// How long the song plays, following its tempo commands.
    pub song_length: Duration,
    pub notes: usize,
    pub note_offs: usize,
    pub commands: usize,
    pub instruments_used: usize,
    pub effects: usize,
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

    pub fn delete_sequence(&mut self, sequence_index: usize) -> usize {
        if self.sequence.len() <= 1 {
            return self.sequence.len();
        };
        self.sequence.remove(sequence_index);
        return self.sequence.len();
    }

    pub fn stats(&self) -> ProjectStats {
        let mut stats = ProjectStats {
            file_size: self.encode().map_or(0, |bytes| bytes.len()),
            patterns_in_song: 0,
            empty_in_song: 0,
            outside_song: 0,
            song_rows: 0,
            song_length: Duration::ZERO,
            notes: 0,
            note_offs: 0,
            commands: 0,
            instruments_used: 0,
            effects: self.instruments.iter().map(|i| i.effects.len()).sum(),
        };
        let in_song: HashSet<PatternId> = self.sequence.iter().map(|row| row.pattern_id).collect();
        stats.patterns_in_song = in_song.len();
        let mut instruments = HashSet::new();
        for (id, pattern) in self.patterns.get_patterns().iter().enumerate() {
            let mut used = false;
            for note in pattern.rows.iter().flat_map(|row| &row.tracks) {
                match note.kind {
                    NoteKind::On(_) => stats.notes += 1,
                    NoteKind::Off => stats.note_offs += 1,
                    NoteKind::Empty => {}
                }
                if note.command.kind != CommandKind::None {
                    stats.commands += 1;
                }
                if let Some(instrument) = note.instrument_id {
                    instruments.insert(instrument);
                }
                used |= *note != Note::empty();
            }
            match (in_song.contains(&PatternId(id as u8)), used) {
                (true, false) => stats.empty_in_song += 1,
                (false, true) => stats.outside_song += 1,
                _ => {}
            }
        }
        stats.instruments_used = instruments.len();
        let (rows, length) = self.play_time();
        stats.song_rows = rows;
        stats.song_length = length;
        return stats;
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
        if !(FIRST_MESSAGEPACK_VERSION..=PROJECT_VERSION).contains(&version) {
            return Err(MTrakError::UnsupportedProjectVersion {
                found: version,
                supported: PROJECT_VERSION,
            });
        }
        let length = u32::from_le_bytes(header[VERSION_END..LENGTH_END].try_into().unwrap());
        let Some(payload) = bytes[LENGTH_END..].get(..length as usize) else {
            return Err(MTrakError::InvalidProject("the file is cut short".into()));
        };
        let payload = rmpv::decode::read_value(&mut &payload[..])
            .map_err(|e| MTrakError::InvalidProject(e.to_string()))?;
        return Self::from_payload(&migration::migrate(payload, version)?);
    }
    /// Reads a migrated payload into a project. It's written back to bytes first, so it's read
    /// by the same code that writes projects.
    fn from_payload(payload: &rmpv::Value) -> Result<Self> {
        let mut bytes = Vec::new();
        rmpv::encode::write_value(&mut bytes, payload)
            .map_err(|e| MTrakError::InvalidProject(e.to_string()))?;
        return Ok(rmp_serde::from_slice(&bytes)?);
    }

    /// The rows the song plays and how long it takes, following its tempo commands like the
    /// engine does: a command changes the tempo from its own row on, the last track's winning,
    /// and the change carries over into the patterns after it.
    fn play_time(&self) -> (u32, Duration) {
        let (mut bpm, mut speed) = (self.bpm, self.speed);
        let (mut rows, mut seconds) = (0, 0.0);
        for position in &self.sequence {
            let Some(pattern) = self.patterns.get_pattern_by_id(position.pattern_id) else {
                continue;
            };
            for _ in 0..position.repeats {
                for row in &pattern.rows {
                    for note in &row.tracks {
                        let CommandKind::Playback(command) = note.command.kind else {
                            continue;
                        };
                        match command.decode(note.command.value) {
                            PlaybackAction::ResetTempo => (bpm, speed) = (self.bpm, self.speed),
                            PlaybackAction::SetSpeed(new) => speed = new,
                            PlaybackAction::SetBpm(new) => bpm = new,
                            _ => {}
                        }
                    }
                    rows += 1;
                    let beats = speed.max(1) as f64 / PULSES_PER_QUARTER as f64;
                    seconds += beats * 60.0 / bpm.max(1) as f64;
                }
            }
        }
        return (rows, Duration::from_secs_f64(seconds));
    }

    fn encode(&self) -> Result<Vec<u8>> {
        let payload = rmp_serde::to_vec_named(self)?;
        let mut bytes = Vec::with_capacity(LENGTH_END + payload.len());
        bytes.extend_from_slice(&PROJECT_HEADER);
        bytes.extend_from_slice(&PROJECT_VERSION.to_le_bytes());
        // a project is a few MB at most, far below u32::MAX
        bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&payload);
        return Ok(bytes);
    }
}

#[cfg(test)]
mod tests {
    use rmpv::Value;

    use super::*;
    use crate::data::{
        effect::{EffectKind, EffectSettings},
        note::{Note, NoteKind, NotePitch},
    };

    fn temp_file(name: &str, contents: &[u8]) -> PathBuf {
        let path = std::env::temp_dir().join(format!("mtrak-test-{}-{}", std::process::id(), name));
        std::fs::write(&path, contents).unwrap();
        return path;
    }

    /// Where the saved test projects live, one per file version: `v3.mtrak` and so on.
    fn test_project_path(version: u16) -> PathBuf {
        return PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src/data/testdata")
            .join(format!("v{version}.mtrak"));
    }

    fn full_note() -> Note {
        return Note::new(
            NoteKind::On(NotePitch(60)),
            Some(3),
            Some(0x40),
            "F8C".parse().unwrap(),
        );
    }

    /// A project that sets every saved field away from its default, saved as the test project
    /// of each file version. Changing it changes the current version's test project.
    fn test_project() -> Project {
        let mut project = Project::empty();
        project.name = Some("Test song".into());
        project.bpm = 140;
        project.speed = 4;
        project.global_velocity = 0xC0;
        project.transpose = -2;
        project.midi_input = Some("Test Keys".into());
        project.clock_input = Some("Test Clock".into());
        project.record_input_velocity = true;
        project.midi_output = Some("Test Synth".into());
        project.send_midi_clock = true;
        project.sequence[0].repeats = 2;
        project.insert_sequence(1, PatternId(1));

        let bass = &mut project.instruments[3];
        bass.name = "Bass".into();
        bass.volume = 0xA0;
        bass.pitch_wheel_range = 2;
        bass.default_cc = 74;
        bass.transpose = 12;
        bass.effects = vec![EffectSettings::new(
            EffectKind::Eq,
            vec![30, 90, 0xC0, 0x00, 0x40, 24, 100, 16],
        )];
        bass.effects[0].enabled = false;
        bass.effects[0].param_ccs = vec![None, Some(0x4A)];

        let first = project
            .patterns
            .get_pattern_by_id_mut(PatternId(0))
            .unwrap();
        first.set_event(0, 0, full_note());
        first.set_event(
            1,
            0,
            Note::new(NoteKind::Off, None, None, Default::default()),
        );
        first.set_event(
            2,
            5,
            Note::new(NoteKind::Empty, None, None, "S10".parse().unwrap()),
        );
        first.channel_mutes[2] = true;
        project
            .patterns
            .create_pattern_id_if_not_exists(PatternId(1));
        let second = project
            .patterns
            .get_pattern_by_id_mut(PatternId(1))
            .unwrap();
        second.set_length(32);
        second.set_event(31, 7, full_note());
        return project;
    }

    /// Checks a loaded test project holds what [`test_project`] put in it. Every file version
    /// has to load into this, so keep it to what all of them save.
    fn assert_test_project(project: &Project) {
        assert_eq!(project.name.as_deref(), Some("Test song"));
        assert_eq!(project.bpm, 140);
        assert_eq!(project.speed, 4);
        assert_eq!(project.global_velocity, 0xC0);
        assert_eq!(project.transpose, -2);
        assert_eq!(project.midi_input.as_deref(), Some("Test Keys"));
        assert_eq!(project.clock_input.as_deref(), Some("Test Clock"));
        assert!(project.record_input_velocity);
        assert_eq!(project.midi_output.as_deref(), Some("Test Synth"));
        assert!(project.send_midi_clock);
        assert!(project.sequence == test_project().sequence);

        let bass = &project.instruments[3];
        assert_eq!(bass.name, "Bass");
        assert_eq!(bass.volume, 0xA0);
        assert_eq!(bass.pitch_wheel_range, 2);
        assert_eq!(bass.default_cc, 74);
        assert_eq!(bass.transpose, 12);

        let expected = test_project();
        assert_eq!(project.patterns.get_patterns().len(), 2);
        for id in [PatternId(0), PatternId(1)] {
            let pattern = project.patterns.get_pattern_by_id(id).unwrap();
            let wanted = expected.patterns.get_pattern_by_id(id).unwrap();
            assert!(pattern.same_content(wanted), "pattern {} differs", id.0);
            assert_eq!(pattern.channel_mutes, wanted.channel_mutes);
        }
    }

    fn with_header(payload: &[u8]) -> Vec<u8> {
        let mut bytes = PROJECT_HEADER.to_vec();
        bytes.extend_from_slice(&PROJECT_VERSION.to_le_bytes());
        bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        bytes.extend_from_slice(payload);
        return bytes;
    }

    fn test_project_payload() -> Value {
        let bytes = test_project().encode().unwrap();
        return rmpv::decode::read_value(&mut &bytes[LENGTH_END..]).unwrap();
    }

    fn encode_value(value: &Value) -> Vec<u8> {
        let mut bytes = Vec::new();
        rmpv::encode::write_value(&mut bytes, value).unwrap();
        return bytes;
    }

    #[test]
    fn stats_tell_empty_patterns_in_the_song_from_full_ones_outside_it() {
        let mut project = Project::empty();
        project
            .patterns
            .create_pattern_id_if_not_exists(PatternId(3));
        let note = Note::new(NoteKind::On(NotePitch(60)), None, None, Default::default());
        let pattern = project
            .patterns
            .get_pattern_by_id_mut(PatternId(3))
            .unwrap();
        pattern.set_event(0, 0, note);
        let stats = project.stats();
        assert_eq!(stats.patterns_in_song, 1);
        assert_eq!(stats.empty_in_song, 1);
        assert_eq!(
            stats.outside_song, 1,
            "pattern 3; patterns 1 and 2 are empty"
        );
    }

    #[test]
    fn song_length_follows_tempo_commands_across_patterns() {
        let mut project = Project::empty();
        project.bpm = 120;
        project.speed = 6;
        project.insert_sequence(1, PatternId(1));
        let tempo = |value: &str| {
            Note::new(
                NoteKind::Empty,
                None,
                None,
                format!("F{value}").parse().unwrap(),
            )
        };
        project
            .patterns
            .create_pattern_id_if_not_exists(PatternId(1));
        let first = project
            .patterns
            .get_pattern_by_id_mut(PatternId(0))
            .unwrap();
        first.set_length(4);
        first.set_event(2, 1, tempo("03"));
        let second = project
            .patterns
            .get_pattern_by_id_mut(PatternId(1))
            .unwrap();
        second.set_length(4);
        second.set_event(1, 0, tempo("00"));
        second.set_event(3, 0, tempo("3C"));
        second.set_event(3, 1, tempo("F0"));
        // a row at speed 6 and 120 BPM is 1/8 s; the speed 3 rows are half that, and so is
        // the last row at 240 BPM, the last track's command
        let stats = project.stats();
        assert_eq!(stats.song_rows, 8);
        let expected = 2.0 * 0.125 + 3.0 * 0.0625 + 2.0 * 0.125 + 0.0625;
        assert!(
            (stats.song_length.as_secs_f64() - expected).abs() < 1e-6,
            "{:?}",
            stats.song_length
        );
    }

    #[test]
    fn stats_count_what_the_project_holds() {
        let mut project = Project::empty();
        project.bpm = 120;
        project.speed = 6;
        let pattern = project
            .patterns
            .get_pattern_by_id_mut(PatternId(0))
            .unwrap();
        pattern.set_length(8);
        let note = Note::new(
            NoteKind::On(NotePitch(60)),
            Some(2),
            None,
            Default::default(),
        );
        pattern.set_event(0, 0, note);
        pattern.set_event(
            1,
            0,
            Note::new(NoteKind::Off, None, None, Default::default()),
        );
        pattern.set_event(
            2,
            1,
            Note::new(NoteKind::Empty, Some(4), None, "S10".parse().unwrap()),
        );
        project.sequence[0].repeats = 2;
        project.instruments[2].effects = vec![EffectSettings::new(EffectKind::Arp, vec![])];
        let stats = project.stats();
        assert_eq!((stats.notes, stats.note_offs, stats.commands), (1, 1, 1));
        assert_eq!((stats.instruments_used, stats.effects), (2, 1));
        assert_eq!(
            (stats.patterns_in_song, stats.empty_in_song, stats.song_rows),
            (1, 0, 16)
        );
        assert_eq!(stats.song_length, Duration::from_secs(2));
        assert_eq!(stats.file_size, project.encode().unwrap().len());
    }

    /// Writes the current version's test project. Run it with
    /// `cargo test write_test_project -- --ignored`, see the update-test-projects skill.
    #[test]
    #[ignore = "writes src/data/testdata, see the update-test-projects skill"]
    fn write_test_project() {
        let path = test_project_path(PROJECT_VERSION);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, test_project().encode().unwrap()).unwrap();
    }

    #[test]
    fn the_test_project_of_every_version_loads() {
        for version in FIRST_MESSAGEPACK_VERSION..=PROJECT_VERSION {
            let path = test_project_path(version);
            let Ok(bytes) = std::fs::read(&path) else {
                panic!(
                    "{} is missing, see the update-test-projects skill",
                    path.display()
                );
            };
            match Project::decode(&bytes) {
                Ok(project) => assert_test_project(&project),
                Err(e) => panic!("the version {version} test project doesn't load: {e}"),
            }
        }
    }

    #[test]
    fn saving_writes_the_current_versions_test_project_byte_for_byte() {
        let saved = std::fs::read(test_project_path(PROJECT_VERSION)).unwrap();
        assert!(
            test_project().encode().unwrap() == saved,
            "projects are saved differently than v{PROJECT_VERSION}.mtrak, see the \
             update-test-projects skill"
        );
    }

    #[test]
    fn every_field_survives_saving_and_loading() {
        let saved = test_project().encode().unwrap();
        let loaded = Project::decode(&saved).unwrap();
        assert_test_project(&loaded);
        assert!(loaded.encode().unwrap() == saved);
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let mut payload = test_project_payload();
        let Value::Map(fields) = &mut payload else {
            panic!("the payload isn't a map");
        };
        fields.insert(0, (Value::from("from_another_program"), Value::from(1)));
        let instruments = fields
            .iter_mut()
            .find(|(key, _)| key.as_str() == Some("instruments"))
            .map(|(_, value)| value);
        let Some(Value::Array(instruments)) = instruments else {
            panic!("the payload has no instruments");
        };
        let Value::Map(bass) = &mut instruments[3] else {
            panic!("an instrument isn't a map");
        };
        bass.push((Value::from("colour"), Value::from("red")));
        let project = Project::decode(&with_header(&encode_value(&payload))).unwrap();
        assert_test_project(&project);
    }

    #[test]
    fn a_missing_field_is_an_error() {
        let mut payload = test_project_payload();
        let Value::Map(fields) = &mut payload else {
            panic!("the payload isn't a map");
        };
        fields.retain(|(key, _)| key.as_str() != Some("bpm"));
        assert!(matches!(
            Project::decode(&with_header(&encode_value(&payload))),
            Err(MTrakError::Decode(_))
        ));
    }

    #[test]
    fn a_payload_that_isnt_messagepack_is_an_error() {
        // 0xC1 is the one byte MessagePack never uses, 0xDE starts a map that's cut short, and
        // a bare number isn't a project
        let garbage: [&[u8]; 4] = [&[0xC1], &[], &[0xDE, 0xFF], &[0x05]];
        for payload in garbage {
            assert!(
                matches!(
                    Project::decode(&with_header(payload)),
                    Err(MTrakError::InvalidProject(_) | MTrakError::Decode(_))
                ),
                "{payload:02X?} should be rejected"
            );
        }
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
    fn notes_and_commands_round_trip() {
        use crate::data::note::{Note, NoteKind, NotePitch};
        let note = Note::new(
            NoteKind::On(NotePitch(60)),
            Some(3),
            Some(0x40),
            "F8C".parse().unwrap(),
        );
        let off = Note::new(NoteKind::Off, None, None, Default::default());
        let mut project = Project::empty();
        let pattern = project
            .patterns
            .get_pattern_by_id_mut(PatternId(0))
            .unwrap();
        pattern.set_event(0, 0, note);
        pattern.set_event(1, 7, off);
        pattern.channel_mutes[2] = true;
        project.instruments[3].name = "Bass".into();

        let loaded = Project::decode(&project.encode().unwrap()).unwrap();
        let pattern = loaded.patterns.get_pattern_by_id(PatternId(0)).unwrap();
        assert_eq!(pattern.get_event(0, 0), Some(&note));
        assert_eq!(pattern.get_event(1, 7), Some(&off));
        assert!(pattern.channel_mutes[2]);
        assert_eq!(loaded.instruments[3].name, "Bass");
    }

    #[test]
    fn patterns_are_the_last_field() {
        let bytes = Project::empty().encode().unwrap();
        let payload = rmpv::decode::read_value(&mut &bytes[LENGTH_END..]).unwrap();
        let keys: Vec<&str> = payload
            .as_map()
            .unwrap()
            .iter()
            .map(|(key, _)| key.as_str().unwrap())
            .collect();
        assert_eq!(keys.last(), Some(&"patterns"));
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
    fn decode_rejects_bincode_and_newer_versions() {
        let bincode_versions = 1..FIRST_MESSAGEPACK_VERSION;
        for version in bincode_versions.chain([PROJECT_VERSION + 1]) {
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
