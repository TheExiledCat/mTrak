//! Key overrides from keybinds.toml and the keybindings screen.

use std::{
    collections::HashMap,
    fs,
    path::Path,
    sync::{LazyLock, RwLock},
};

use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use crate::{
    error::{MTrakError, Result},
    tui::{
        constants::{PIANO_KEYS_LOWER, PIANO_KEYS_UPPER, TRACK_JUMP_KEYS},
        files,
        framework::keymap::{BindingInfo, HelpSection, Key},
    },
    util::toml::{self, ParseError},
};

struct Entry {
    section: &'static str,
    description: &'static str,
    binding: BindingInfo,
}

#[derive(Default)]
struct Keybinds {
    /// By action id. An empty list unbinds the action.
    overrides: HashMap<String, Vec<Key>>,
    /// For pointing at unknown actions.
    lines: HashMap<String, usize>,
    /// Every action that can be rebound.
    entries: Vec<Entry>,
}

static KEYBINDS: LazyLock<RwLock<Keybinds>> = LazyLock::new(Default::default);

pub fn with_overrides<T>(f: impl FnOnce(&HashMap<String, Vec<Key>>) -> T) -> T {
    let keybinds = KEYBINDS.read().unwrap_or_else(|e| e.into_inner());
    return f(&keybinds.overrides);
}

/// Unknown actions are reported by `set_entries`, once every action is known.
pub fn load(path: &Path) -> Result<()> {
    let (overrides, lines) = toml::load_file(path, parse)?;
    let mut keybinds = KEYBINDS.write().unwrap_or_else(|e| e.into_inner());
    keybinds.overrides = overrides;
    keybinds.lines = lines;
    return Ok(());
}

type Parsed = (HashMap<String, Vec<Key>>, HashMap<String, usize>);

fn parse(text: &str) -> std::result::Result<Parsed, ParseError> {
    let mut overrides = HashMap::new();
    let mut lines = HashMap::new();
    for entry in toml::parse(text)? {
        let keys = entry
            .value
            .split_whitespace()
            .map(|text| {
                return Key::parse(text)
                    .ok_or_else(|| ParseError::new(entry.line, format!("{text:?} is not a key")));
            })
            .collect::<std::result::Result<Vec<Key>, ParseError>>()?;
        lines.insert(entry.key.clone(), entry.line);
        overrides.insert(entry.key, keys);
    }
    return Ok((overrides, lines));
}

/// Registers every action that can be rebound, and reports unknown ones in keybinds.toml.
pub fn set_entries(sections: &[HelpSection]) -> Result<()> {
    let mut keybinds = KEYBINDS.write().unwrap_or_else(|e| e.into_inner());
    keybinds.entries.clear();
    for section in sections {
        for entry in &section.entries {
            let Some(binding) = &entry.binding else {
                continue;
            };
            if keybinds.entries.iter().any(|e| e.binding.id == binding.id) {
                continue;
            }
            keybinds.entries.push(Entry {
                section: section.title,
                description: entry.description,
                binding: binding.clone(),
            });
        }
    }
    let mut unknown: Vec<(usize, &String)> = keybinds
        .overrides
        .keys()
        .filter(|id| !keybinds.entries.iter().any(|e| e.binding.id == **id))
        .map(|id| (keybinds.lines.get(id).copied().unwrap_or(0), id))
        .collect();
    unknown.sort();
    let Some((line, id)) = unknown.first() else {
        return Ok(());
    };
    return Err(MTrakError::InvalidConfig(format!(
        "{}:\n{}",
        files::KEYBINDS_FILE_NAME,
        ParseError::new(*line, format!("unknown action {id}"))
    )));
}

/// `None` puts back the defaults. Saves keybinds.toml.
pub fn set(id: &str, keys: Option<Vec<Key>>) -> Result<()> {
    {
        let mut keybinds = KEYBINDS.write().unwrap_or_else(|e| e.into_inner());
        let defaults = keybinds
            .entries
            .iter()
            .find(|e| e.binding.id == id)
            .map(|e| e.binding.defaults.clone());
        match keys {
            Some(keys) if Some(&keys) != defaults.as_ref() => {
                keybinds.overrides.insert(id.to_owned(), keys);
            }
            _ => {
                keybinds.overrides.remove(id);
            }
        }
    }
    return save(&files::get_keybinds_file_path()?);
}

pub fn reset_all() -> Result<()> {
    KEYBINDS
        .write()
        .unwrap_or_else(|e| e.into_inner())
        .overrides
        .clear();
    return save(&files::get_keybinds_file_path()?);
}

pub fn keys(binding: &BindingInfo) -> Vec<Key> {
    return with_overrides(|overrides| {
        return overrides
            .get(&binding.id)
            .cloned()
            .unwrap_or_else(|| binding.defaults.clone());
    });
}

/// Everything else `key` does, e.g. "Pattern editor: Toggle edit mode".
pub fn uses(key: Key, except: &str) -> Vec<String> {
    let keybinds = KEYBINDS.read().unwrap_or_else(|e| e.into_inner());
    let mut uses: Vec<String> = keybinds
        .entries
        .iter()
        .filter(|e| e.binding.id != except)
        .filter(|e| {
            let keys = keybinds
                .overrides
                .get(&e.binding.id)
                .unwrap_or(&e.binding.defaults);
            return keys.contains(&key);
        })
        .map(|e| format!("{}: {}", e.section, e.description))
        .collect();
    uses.extend(
        fixed_keys()
            .into_iter()
            .filter(|(k, _)| *k == key)
            .map(|(_, description)| description.to_owned()),
    );
    return uses;
}

/// Keys handled outside the keymaps, which can't be rebound.
fn fixed_keys() -> Vec<(Key, &'static str)> {
    let mut keys = Vec::new();
    for c in PIANO_KEYS_LOWER.iter().chain(PIANO_KEYS_UPPER.iter()) {
        keys.push((Key::char(*c), "Pattern editor: Play or enter a note"));
    }
    for c in TRACK_JUMP_KEYS {
        keys.push((Key::alt(c), "Pattern editor: Jump to a track"));
    }
    for n in 1..=8 {
        keys.push((Key::plain(KeyCode::F(n)), "Pattern editor: Set the octave"));
    }
    for c in "0123456789abcdef".chars() {
        keys.push((
            Key::char(c),
            "Pattern editor (edit mode): Enter a hex digit",
        ));
    }
    for c in '1'..='9' {
        keys.push((
            Key::new(KeyCode::Char(c), KeyModifiers::ALT),
            "Channels: Mute a channel or switch channels",
        ));
    }
    return keys;
}

pub fn save(path: &Path) -> Result<()> {
    fs::write(path, to_toml())?;
    return Ok(());
}

fn to_toml() -> String {
    let keybinds = KEYBINDS.read().unwrap_or_else(|e| e.into_inner());
    let mut text = String::from(
        "# mTrak keybindings. Every action is listed with its default keys; uncomment a line\n\
         # and change it to rebind the action, or rebind it on the keybindings screen (?).\n\
         #\n\
         # Keys: letters and symbols as typed (a capital means Shift), Spc, Up, Dn, Lt, Rt,\n\
         # PgUp, PgDn, Home, End, Ins, Del, Bksp, Enter, Esc, Tab and F1-F12, after any of\n\
         # ^ (Ctrl), A- (Alt) and S- (Shift), e.g. \"^S\", \"A-O\" or \"S-Tab\". Separate several\n\
         # keys with spaces, and use \"\" to unbind an action.\n",
    );
    let mut section = "";
    for entry in &keybinds.entries {
        if entry.section != section {
            section = entry.section;
            text.push_str(&format!("\n# {section}\n"));
        }
        let id = &entry.binding.id;
        let keys_text = |keys: &[Key]| {
            return keys
                .iter()
                .map(|key| key.to_text())
                .collect::<Vec<_>>()
                .join(" ");
        };
        let defaults = keys_text(&entry.binding.defaults);
        text.push_str(&format!("# {}\n", entry.description));
        let defaults = toml::quote(&defaults);
        match keybinds.overrides.get(id) {
            Some(keys) => text.push_str(&format!(
                "{id} = {} # default {defaults}\n",
                toml::quote(&keys_text(keys))
            )),
            None => text.push_str(&format!("# {id} = {defaults}\n")),
        }
    }
    return text;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backslashes_and_quotes_read_back() {
        let line = format!("pattern.toggle_edit = {}", toml::quote("\\ \" ^\\"));
        let (overrides, _) = parse(&line).unwrap();
        assert_eq!(
            overrides["pattern.toggle_edit"],
            vec![
                Key::char('\\'),
                Key::char('"'),
                Key::new(KeyCode::Char('\\'), KeyModifiers::CONTROL),
            ]
        );
    }

    #[test]
    fn reads_keys_and_unbound_actions() {
        let (overrides, lines) =
            parse("# a comment\nglobal.save = \"^W ^S\"\npattern.note_off = \"\"\n").unwrap();
        assert_eq!(
            overrides["global.save"],
            vec![Key::ctrl('w'), Key::ctrl('s')]
        );
        assert!(overrides["pattern.note_off"].is_empty());
        assert_eq!(lines["global.save"], 2);
        assert_eq!(parse("a.b = \"^\"").unwrap().0["a.b"], vec![Key::char('^')]);
        assert_eq!(parse("x = \"Fx\"").err().unwrap().line, 1);
    }
}
