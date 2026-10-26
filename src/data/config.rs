use std::{fmt, fs, path::Path, str::FromStr};

use clap::ValueEnum;
use clap_derive::ValueEnum;

use crate::{
    error::Result,
    util::toml::{self, ParseError},
};

pub const TITLE: &str = "mTrak";
pub const COMMAND: &str = "mtrak";

/// The value of `theme` that picks a theme from what the terminal supports.
const AUTO_THEME: &str = "auto";

/// The themes built into mTrak.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeChoice {
    /// No colors, only bold, dim, underline and reverse
    Mono,
    /// The 16 standard terminal colors
    Ansi16,
    /// The FT2 theme, for truecolor terminals
    Full,
}

/// The theme to use, from `--color` or `theme` in config.toml: picked from what the terminal
/// supports, a built-in one, or a file in the themes folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThemeSetting {
    Auto,
    Builtin(ThemeChoice),
    Named(String),
}

/// The theme an edited built-in theme is saved as, unless it's given another name.
pub const CUSTOM_THEME: &str = "custom";

impl FromStr for ThemeSetting {
    type Err = String;

    fn from_str(text: &str) -> std::result::Result<Self, String> {
        if text.eq_ignore_ascii_case(AUTO_THEME) {
            return Ok(ThemeSetting::Auto);
        }
        if let Ok(choice) = ThemeChoice::from_str(text, true) {
            return Ok(ThemeSetting::Builtin(choice));
        }
        if is_theme_name(text) {
            return Ok(ThemeSetting::Named(text.to_owned()));
        }
        return Err(format!("{text:?} is not a theme name"));
    }
}

impl fmt::Display for ThemeSetting {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        return match self {
            ThemeSetting::Auto => write!(f, "{AUTO_THEME}"),
            ThemeSetting::Builtin(choice) => {
                let value = choice.to_possible_value().ok_or(fmt::Error)?;
                write!(f, "{}", value.get_name())
            }
            ThemeSetting::Named(name) => write!(f, "{name}"),
        };
    }
}

/// A name a theme file can have: letters, digits, `-` and `_`, and not the name of a built-in
/// theme.
pub fn is_theme_name(name: &str) -> bool {
    return !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && !name.eq_ignore_ascii_case(AUTO_THEME)
        && ThemeChoice::from_str(name, true).is_err();
}

/// The built-in values `theme` can take, before the themes in the themes folder.
pub fn builtin_themes() -> Vec<ThemeSetting> {
    let mut themes = vec![ThemeSetting::Auto];
    themes.extend(
        ThemeChoice::value_variants()
            .iter()
            .map(|choice| ThemeSetting::Builtin(*choice)),
    );
    return themes;
}

/// Which sides of the pattern editor show the row numbers.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineNumbers {
    Left,
    Right,
    Both,
}

impl LineNumbers {
    pub const ALL: [LineNumbers; 3] = [LineNumbers::Left, LineNumbers::Right, LineNumbers::Both];

    pub fn on_left(self) -> bool {
        return self != LineNumbers::Right;
    }
    pub fn on_right(self) -> bool {
        return self != LineNumbers::Left;
    }
    pub fn name(self) -> String {
        return self
            .to_possible_value()
            .map(|value| value.get_name().to_owned())
            .unwrap_or_default();
    }
}

/// Settings from config.toml in the config folder, with the command line options applied for
/// this session.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub theme: ThemeSetting,
    /// Draw with ASCII characters only.
    pub ascii_mode: bool,
    /// Look for a new version on startup.
    pub update_check: bool,
    /// The MIDI input port projects that don't name one connect to.
    pub midi_input: Option<String>,
    /// The MIDI output port projects that don't name one connect to.
    pub midi_output: Option<String>,
    /// Number the pattern editor's rows in hex, else in decimal from 00 to 99 and round again.
    pub hex_line_numbers: bool,
    /// Show dashes in empty instrument and volume columns.
    pub show_dashes: bool,
    /// Show the header with the track numbers above the pattern editor.
    pub track_numbers: bool,
    pub line_numbers: LineNumbers,
}

impl Default for Config {
    fn default() -> Self {
        return Self {
            theme: ThemeSetting::Auto,
            ascii_mode: false,
            update_check: true,
            midi_input: None,
            midi_output: None,
            hex_line_numbers: true,
            show_dashes: true,
            track_numbers: true,
            line_numbers: LineNumbers::Left,
        };
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        return toml::load_file(path, Self::from_toml);
    }

    pub fn from_toml(text: &str) -> std::result::Result<Self, ParseError> {
        let mut config = Self::default();
        for entry in toml::parse(text)? {
            let error = |message: String| ParseError::new(entry.line, message);
            let port = |value: &str| Some(value.to_owned()).filter(|name| !name.is_empty());
            match entry.key.as_str() {
                "theme" => config.theme = entry.value.parse().map_err(error)?,
                "ascii_mode" => config.ascii_mode = parse_bool(&entry.value).map_err(error)?,
                "update_check" => config.update_check = parse_bool(&entry.value).map_err(error)?,
                "midi_input" => config.midi_input = port(&entry.value),
                "midi_output" => config.midi_output = port(&entry.value),
                "hex_line_numbers" => {
                    config.hex_line_numbers = parse_bool(&entry.value).map_err(error)?;
                }
                "show_dashes" => config.show_dashes = parse_bool(&entry.value).map_err(error)?,
                "track_numbers" => {
                    config.track_numbers = parse_bool(&entry.value).map_err(error)?
                }
                "line_numbers" => {
                    config.line_numbers = LineNumbers::from_str(&entry.value, true)
                        .map_err(|_| error("must be left, right or both".to_owned()))?;
                }
                key => return Err(error(format!("unknown setting {key}"))),
            }
        }
        return Ok(config);
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        fs::write(path, self.to_toml())?;
        return Ok(());
    }

    pub fn to_toml(&self) -> String {
        let Self {
            theme,
            ascii_mode,
            update_check,
            midi_input,
            midi_output,
            hex_line_numbers,
            show_dashes,
            track_numbers,
            line_numbers,
        } = self;
        let line_numbers = line_numbers.name();
        let midi_input = midi_input.as_deref().unwrap_or_default();
        let midi_output = midi_output.as_deref().unwrap_or_default();
        return format!(
            "# mTrak settings. Delete this file to get the defaults back.\n\
             \n\
             # The theme to open with: \"auto\" picks one from what the terminal supports, or\n\
             # \"mono\", \"ansi16\", \"full\" or the name of a file in the themes folder, without\n\
             # .toml. --color overrides this.\n\
             theme = \"{theme}\"\n\
             \n\
             # Draw with ASCII characters only, for terminals or fonts without box drawing\n\
             # characters. --ascii-mode turns this on for one session.\n\
             ascii_mode = {ascii_mode}\n\
             \n\
             # Look for a new version on startup. --no-update-check skips it for one session.\n\
             update_check = {update_check}\n\
             \n\
             # The MIDI ports a project connects to when it doesn't name its own, \"\" for none.\n\
             midi_input = \"{midi_input}\"\n\
             midi_output = \"{midi_output}\"\n\
             \n\
             # Pattern editor: row numbers in hex, or in decimal from 00 to 99 when false.\n\
             hex_line_numbers = {hex_line_numbers}\n\
             # Dashes in empty instrument and volume columns.\n\
             show_dashes = {show_dashes}\n\
             # The header with the track numbers above the tracks.\n\
             track_numbers = {track_numbers}\n\
             # Where the row numbers go: \"left\", \"right\" or \"both\".\n\
             line_numbers = \"{line_numbers}\"\n"
        );
    }
}

fn parse_bool(value: &str) -> std::result::Result<bool, String> {
    return match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!("{value:?} must be true or false")),
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_file_reads_back_as_the_default() {
        let config = Config::from_toml(&Config::default().to_toml()).unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn every_setting_reads_back() {
        let mut themes = builtin_themes();
        themes.push(ThemeSetting::Named("dracula".to_owned()));
        for theme in themes {
            let config = Config {
                theme,
                ascii_mode: true,
                update_check: false,
                midi_input: Some("Keys".to_owned()),
                midi_output: Some("USB MIDI Out".to_owned()),
                hex_line_numbers: false,
                show_dashes: false,
                track_numbers: false,
                line_numbers: LineNumbers::Both,
            };
            assert_eq!(Config::from_toml(&config.to_toml()).unwrap(), config);
        }
    }

    #[test]
    fn reads_a_theme() {
        let config = Config::from_toml("theme = \"full\"").unwrap();
        assert_eq!(config.theme, ThemeSetting::Builtin(ThemeChoice::Full));
        let config = Config::from_toml("theme = \"tokyo-night\"").unwrap();
        assert_eq!(config.theme, ThemeSetting::Named("tokyo-night".to_owned()));
        assert!(Config::from_toml("theme = \"pink/blue\"").is_err());
        assert!(Config::from_toml("volume = \"11\"").is_err());
        assert!(Config::from_toml("ascii_mode = \"yes\"").is_err());
    }

    #[test]
    fn built_in_names_are_not_theme_names() {
        assert!(is_theme_name("my_theme-2"));
        assert!(!is_theme_name("Full"));
        assert!(!is_theme_name("auto"));
        assert!(!is_theme_name(""));
        assert!(!is_theme_name("a b"));
    }
}
