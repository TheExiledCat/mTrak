//! Upgrades project payloads saved by older mTrak versions so they can still be opened.
//!
//! A payload is MessagePack with every struct written as a map of field names, so it's read
//! as a generic [`Value`] first. Each file version has a migration that takes a payload of
//! that version and returns it as the next version (fields renamed, added or converted). The
//! payload is passed through every migration from the file's version on, and what comes out
//! of the last one is read into a `Project`. Fields the current version doesn't know are
//! ignored when reading.
//!
//! To change the project format, raise [`PROJECT_VERSION`] and add a migration to
//! [`MIGRATIONS`] for the version before it, turning its payload into the new one. Every
//! version from [`FIRST_MESSAGEPACK_VERSION`] up to the current one must have one. A migration
//! is named after the versions it goes between, e.g. `v3_to_v4`.

use rmpv::Value;

use crate::error::{MTrakError, Result};

pub type Migration = fn(Value) -> Result<Value>;

/// Each file version before the current one, with the migration that turns its payload into
/// the next version's.
const MIGRATIONS: &[(u16, Migration)] = &[(3, v3_to_v4)];

/// The first file version written as MessagePack. Older files used bincode, which can't be
/// migrated as it doesn't store field names.
pub const FIRST_MESSAGEPACK_VERSION: u16 = 3;
pub const PROJECT_VERSION: u16 = 4;

/// Passes a payload of `version` through every migration up to [`PROJECT_VERSION`].
pub fn migrate(payload: Value, version: u16) -> Result<Value> {
    return migrate_with(payload, version, PROJECT_VERSION, MIGRATIONS);
}

/// Version 4 gave instruments a chain of effects, which is empty for older projects.
fn v3_to_v4(mut payload: Value) -> Result<Value> {
    let Some(Value::Array(instruments)) = map_field(&mut payload, "instruments") else {
        return Err(MTrakError::InvalidProject(
            "the project has no instruments".into(),
        ));
    };
    for instrument in instruments {
        let Value::Map(fields) = instrument else {
            return Err(MTrakError::InvalidProject(
                "an instrument isn't a map".into(),
            ));
        };
        fields.push((Value::from("effects"), Value::Array(Vec::new())));
    }
    return Ok(payload);
}

fn map_field<'a>(payload: &'a mut Value, name: &str) -> Option<&'a mut Value> {
    let Value::Map(fields) = payload else {
        return None;
    };
    return fields
        .iter_mut()
        .find(|(key, _)| key.as_str() == Some(name))
        .map(|(_, value)| value);
}

fn migrate_with(
    mut payload: Value,
    version: u16,
    current: u16,
    migrations: &[(u16, Migration)],
) -> Result<Value> {
    for version in version..current {
        let Some((_, migration)) = migrations.iter().find(|(from, _)| *from == version) else {
            return Err(MTrakError::InvalidProject(format!(
                "there's no way to upgrade a version {version} project"
            )));
        };
        payload = migration(payload)?;
    }
    return Ok(payload);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rename_tempo_to_bpm(payload: Value) -> Result<Value> {
        let Value::Map(fields) = payload else {
            return Err(MTrakError::InvalidProject("the project isn't a map".into()));
        };
        let fields = fields
            .into_iter()
            .map(|(key, value)| match key.as_str() {
                Some("tempo") => (Value::from("bpm"), value),
                _ => (key, value),
            })
            .collect();
        return Ok(Value::Map(fields));
    }

    fn add_speed(payload: Value) -> Result<Value> {
        let Value::Map(mut fields) = payload else {
            return Err(MTrakError::InvalidProject("the project isn't a map".into()));
        };
        fields.push((Value::from("speed"), Value::from(6)));
        return Ok(Value::Map(fields));
    }

    fn payload(fields: Vec<(&str, Value)>) -> Value {
        return Value::Map(
            fields
                .into_iter()
                .map(|(key, value)| (Value::from(key), value))
                .collect(),
        );
    }

    fn field<'a>(payload: &'a Value, name: &str) -> Option<&'a Value> {
        return payload
            .as_map()?
            .iter()
            .find(|(key, _)| key.as_str() == Some(name))
            .map(|(_, value)| value);
    }

    const V: u16 = FIRST_MESSAGEPACK_VERSION;
    const TEST_MIGRATIONS: &[(u16, Migration)] = &[(V, rename_tempo_to_bpm), (V + 1, add_speed)];

    #[test]
    fn version_3_instruments_get_an_empty_effect_chain() {
        let instrument = payload(vec![("name", Value::from("Bass"))]);
        let old = payload(vec![("instruments", Value::Array(vec![instrument; 2]))]);
        let migrated = v3_to_v4(old).unwrap();
        let Some(Value::Array(instruments)) = field(&migrated, "instruments") else {
            panic!("the instruments are gone");
        };
        for instrument in instruments {
            assert_eq!(field(instrument, "name"), Some(&Value::from("Bass")));
            assert_eq!(field(instrument, "effects"), Some(&Value::Array(vec![])));
        }
    }

    #[test]
    fn version_3_without_instruments_is_an_error() {
        assert!(matches!(
            v3_to_v4(payload(vec![])),
            Err(MTrakError::InvalidProject(_))
        ));
    }

    #[test]
    fn every_version_before_the_current_one_has_a_migration() {
        for version in FIRST_MESSAGEPACK_VERSION..PROJECT_VERSION {
            let count = MIGRATIONS
                .iter()
                .filter(|(from, _)| *from == version)
                .count();
            assert_eq!(count, 1, "version {version} needs exactly one migration");
        }
        assert!(
            MIGRATIONS
                .iter()
                .all(|(from, _)| (FIRST_MESSAGEPACK_VERSION..PROJECT_VERSION).contains(from)),
            "a migration is listed for a version that isn't migrated"
        );
    }

    #[test]
    fn passes_the_payload_through_every_migration_from_its_version_on() {
        let old = payload(vec![("tempo", Value::from(140))]);
        let migrated = migrate_with(old, V, V + 2, TEST_MIGRATIONS).unwrap();
        assert_eq!(field(&migrated, "bpm"), Some(&Value::from(140)));
        assert_eq!(field(&migrated, "speed"), Some(&Value::from(6)));

        // a file saved after the rename only goes through the later migration
        let newer = payload(vec![("bpm", Value::from(140))]);
        let migrated = migrate_with(newer, V + 1, V + 2, TEST_MIGRATIONS).unwrap();
        assert_eq!(field(&migrated, "bpm"), Some(&Value::from(140)));
        assert_eq!(field(&migrated, "speed"), Some(&Value::from(6)));
    }

    #[test]
    fn a_current_payload_is_left_alone() {
        let current = payload(vec![("tempo", Value::from(140))]);
        let migrated = migrate_with(current.clone(), V + 2, V + 2, TEST_MIGRATIONS).unwrap();
        assert_eq!(migrated, current);
    }

    #[test]
    fn a_failing_migration_stops_the_chain() {
        assert!(matches!(
            migrate_with(Value::from(1), V, V + 2, TEST_MIGRATIONS),
            Err(MTrakError::InvalidProject(_))
        ));
    }

    #[test]
    fn a_missing_migration_is_an_error() {
        let current = payload(vec![("bpm", Value::from(140))]);
        assert!(matches!(
            migrate_with(current, V, V + 1, &[]),
            Err(MTrakError::InvalidProject(_))
        ));
    }
}
