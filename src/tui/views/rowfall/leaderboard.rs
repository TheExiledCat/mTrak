use std::{fs, path::Path};

use crate::{
    error::Result,
    util::toml::{self, ParseError},
};

pub const MAX_ENTRIES: usize = 10;
pub const MAX_NAME_LENGTH: usize = 12;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub score: u32,
    pub lines: u32,
    pub level: u32,
    pub date: String,
}

/// The best scores, highest first, kept in leaderboard.toml as numbered keys like
/// `1.name = "AMANO"`.
#[derive(Default)]
pub struct Leaderboard {
    pub entries: Vec<Entry>,
}

impl Leaderboard {
    pub fn new() -> Self {
        return Self {
            entries: Vec::new(),
        };
    }

    /// A missing file is an empty leaderboard.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::new());
        }
        return toml::load_file(path, parse);
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(folder) = path.parent() {
            fs::create_dir_all(folder)?;
        }
        fs::write(path, self.to_text())?;
        return Ok(());
    }

    pub fn qualifies(&self, score: u32) -> bool {
        return score > 0
            && (self.entries.len() < MAX_ENTRIES
                || self.entries.last().is_some_and(|last| score > last.score));
    }

    /// Adds the entry in its place and returns that place, from 0.
    pub fn add(&mut self, entry: Entry) -> usize {
        let place = self
            .entries
            .iter()
            .position(|other| entry.score > other.score)
            .unwrap_or(self.entries.len());
        self.entries.insert(place, entry);
        self.entries.truncate(MAX_ENTRIES);
        return place;
    }

    fn to_text(&self) -> String {
        let mut text = String::from("# mTrak Rowfall leaderboard\n");
        for (index, entry) in self.entries.iter().enumerate() {
            let place = index + 1;
            text.push('\n');
            text.push_str(&format!("{place}.name = {}\n", toml::quote(&entry.name)));
            text.push_str(&format!("{place}.score = \"{}\"\n", entry.score));
            text.push_str(&format!("{place}.lines = \"{}\"\n", entry.lines));
            text.push_str(&format!("{place}.level = \"{}\"\n", entry.level));
            text.push_str(&format!("{place}.date = {}\n", toml::quote(&entry.date)));
        }
        return text;
    }
}

fn parse(text: &str) -> std::result::Result<Leaderboard, ParseError> {
    let mut entries: Vec<(usize, Entry)> = Vec::new();
    for item in toml::parse(text)? {
        let Some((place, field)) = item
            .key
            .split_once('.')
            .and_then(|(place, field)| Some((place.parse::<usize>().ok()?, field)))
        else {
            return Err(ParseError::new(
                item.line,
                format!("unknown key {}", item.key),
            ));
        };
        let index = match entries.iter().position(|(p, _)| *p == place) {
            Some(index) => index,
            None => {
                entries.push((
                    place,
                    Entry {
                        name: String::new(),
                        score: 0,
                        lines: 0,
                        level: 1,
                        date: String::new(),
                    },
                ));
                entries.len() - 1
            }
        };
        let entry = &mut entries[index].1;
        let number = || {
            return item
                .value
                .parse::<u32>()
                .map_err(|_| ParseError::new(item.line, format!("{} isn't a number", item.key)));
        };
        match field {
            "name" => entry.name = item.value.chars().take(MAX_NAME_LENGTH).collect(),
            "score" => entry.score = number()?,
            "lines" => entry.lines = number()?,
            "level" => entry.level = number()?,
            "date" => entry.date = item.value.clone(),
            _ => {
                return Err(ParseError::new(
                    item.line,
                    format!("unknown key {}", item.key),
                ));
            }
        }
    }
    let mut entries: Vec<Entry> = entries.into_iter().map(|(_, entry)| entry).collect();
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.score));
    entries.truncate(MAX_ENTRIES);
    return Ok(Leaderboard { entries });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, score: u32) -> Entry {
        return Entry {
            name: name.into(),
            score,
            lines: score / 2,
            level: 2,
            date: "2026-10-31".into(),
        };
    }

    #[test]
    fn saved_text_reads_back_the_same() {
        let mut board = Leaderboard::new();
        board.add(entry("AMANO", 45));
        board.add(entry("Quote \"me\"", 120));
        let read = parse(&board.to_text()).unwrap();
        assert_eq!(read.entries, board.entries);
        assert_eq!(read.entries[0].name, "Quote \"me\"");
    }

    #[test]
    fn only_the_best_ten_are_kept() {
        let mut board = Leaderboard::new();
        for score in 1..=12 {
            board.add(entry("A", score));
        }
        assert_eq!(board.entries.len(), MAX_ENTRIES);
        assert_eq!(board.entries[0].score, 12);
        assert!(!board.qualifies(3));
        assert!(board.qualifies(4));
        assert!(!Leaderboard::new().qualifies(0));
    }

    #[test]
    fn a_new_score_goes_below_an_equal_one() {
        let mut board = Leaderboard::new();
        board.add(entry("FIRST", 10));
        assert_eq!(board.add(entry("SECOND", 10)), 1);
    }

    #[test]
    fn unknown_keys_are_errors() {
        assert!(parse("1.colour = \"red\"").is_err());
        assert!(parse("best = \"1\"").is_err());
    }
}
