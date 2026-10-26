//! A parser for the small subset of TOML that mTrak's config files use: `key = "value"` pairs,
//! bare `true` and `false`, and `#` comments.

use std::{
    fmt::{self, Display},
    fs,
    path::Path,
};

use crate::error::{MTrakError, Result as MTrakResult};

pub struct Entry {
    pub line: usize,
    pub key: String,
    pub value: String,
}

#[derive(Debug)]
pub struct ParseError {
    pub line: usize,
    pub message: String,
}

impl ParseError {
    pub fn new(line: usize, message: impl Into<String>) -> Self {
        return Self {
            line,
            message: message.into(),
        };
    }
}

impl Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        return write!(f, "line {}: {}", self.line, self.message);
    }
}

/// Reads and parses a config file, naming the file in any error.
pub fn load_file<T>(
    path: &Path,
    parse: impl FnOnce(&str) -> Result<T, ParseError>,
) -> MTrakResult<T> {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let text =
        fs::read_to_string(path).map_err(|e| MTrakError::InvalidConfig(format!("{name}:\n{e}")))?;
    return parse(&text).map_err(|e| MTrakError::InvalidConfig(format!("{name}:\n{e}")));
}

pub fn parse(text: &str) -> Result<Vec<Entry>, ParseError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut entries: Vec<Entry> = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let content = raw.trim();
        if is_comment_or_empty(content) {
            continue;
        }
        let (key, value) = content
            .split_once('=')
            .ok_or_else(|| ParseError::new(line, "expected key = \"value\""))?;
        let key = key.trim();
        if !is_bare_key(key) {
            return Err(ParseError::new(line, format!("invalid key {key:?}")));
        }
        let value = parse_value(value.trim()).map_err(|message| ParseError::new(line, message))?;
        let key = key.to_owned();
        if entries.iter().any(|e| e.key == key) {
            return Err(ParseError::new(line, format!("{key} is set twice")));
        }
        entries.push(Entry { line, key, value });
    }
    return Ok(entries);
}

fn is_bare_key(key: &str) -> bool {
    return !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
}

fn is_comment_or_empty(text: &str) -> bool {
    let text = text.trim();
    return text.is_empty() || text.starts_with('#');
}

/// A string, or a bare `true` or `false`, which read as the strings "true" and "false".
fn parse_value(text: &str) -> Result<String, &'static str> {
    for word in ["true", "false"] {
        if let Some(after) = text.strip_prefix(word)
            && is_comment_or_empty(after)
        {
            return Ok(word.to_owned());
        }
    }
    return parse_string(text);
}

/// A double-quoted string without escapes, optionally followed by a comment.
fn parse_string(text: &str) -> Result<String, &'static str> {
    let rest = text
        .strip_prefix('"')
        .ok_or("values must be in double quotes")?;
    let (value, after) = rest.split_once('"').ok_or("missing closing quote")?;
    if value.contains('\\') {
        return Err("escape sequences are not supported");
    }
    if !is_comment_or_empty(after) {
        return Err("unexpected text after the value");
    }
    return Ok(value.to_owned());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(text: &str) -> Vec<(String, String)> {
        return parse(text)
            .unwrap()
            .into_iter()
            .map(|e| (e.key, e.value))
            .collect();
    }

    fn error_line(text: &str) -> usize {
        return parse(text).err().unwrap().line;
    }

    #[test]
    fn reads_pairs_and_comments() {
        let text = "# theme\nbase = \"ft2\"  # start here\n\ndanger = \"#ff0000\"\n";
        assert_eq!(
            pairs(text),
            vec![
                ("base".to_owned(), "ft2".to_owned()),
                ("danger".to_owned(), "#ff0000".to_owned()),
            ]
        );
    }

    #[test]
    fn skips_a_byte_order_mark() {
        assert_eq!(
            pairs("\u{feff}a = \"1\""),
            vec![("a".to_owned(), "1".to_owned())]
        );
    }

    #[test]
    fn reads_bare_booleans() {
        assert_eq!(
            pairs("a = true # on\nb = false"),
            vec![
                ("a".to_owned(), "true".to_owned()),
                ("b".to_owned(), "false".to_owned()),
            ]
        );
        assert_eq!(error_line("a = truest"), 1);
    }

    #[test]
    fn keeps_hashes_inside_values() {
        assert_eq!(
            pairs("text = \"#fff\""),
            vec![("text".to_owned(), "#fff".to_owned())]
        );
    }

    #[test]
    fn reports_the_line_of_an_error() {
        assert_eq!(error_line("a = \"1\"\nb = 2"), 2);
        assert_eq!(error_line("a = \"1\"\n\na = \"2\""), 3);
        assert_eq!(error_line("[colors]"), 1);
        assert_eq!(error_line("a = \"1\" b"), 1);
        assert_eq!(error_line("bad key = \"1\""), 1);
    }
}
