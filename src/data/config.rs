use std::{fs, iter::once, path::Path};

use clap::ValueEnum;

use crate::{
    cli::ColorMode,
    error::{MTrakError, Result},
    util::toml::{self, ParseError},
};

pub const TITLE: &str = "mTrak";
pub const COMMAND: &str = "mtrak";

/// The value of `theme` that picks a theme from what the terminal supports.
const AUTO_THEME: &str = "auto";

/// Settings from config.toml in the config folder.
#[derive(Clone)]
pub struct Config {
    /// The theme to open with, or `None` to pick one from what the terminal supports.
    pub theme: Option<ColorMode>,
}
impl Default for Config {
    fn default() -> Self {
        return Self { theme: None };
    }
}
impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)?;
        return Self::from_toml(&text).map_err(|e| {
            MTrakError::InvalidConfig(format!(
                "{}:\n{}",
                path.file_name().unwrap_or_default().display(),
                e
            ))
        });
    }

    pub fn from_toml(text: &str) -> std::result::Result<Self, ParseError> {
        let mut config = Self::default();
        for entry in toml::parse(text)? {
            match entry.key.as_str() {
                "theme" => {
                    config.theme = if entry.value == AUTO_THEME {
                        None
                    } else {
                        let mode = ColorMode::from_str(&entry.value, true).map_err(|_| {
                            ParseError::new(entry.line, format!("unknown theme {:?}", entry.value))
                        })?;
                        Some(mode)
                    };
                }
                key => {
                    return Err(ParseError::new(
                        entry.line,
                        format!("unknown setting {key}"),
                    ));
                }
            }
        }
        return Ok(config);
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        fs::write(path, self.to_toml())?;
        return Ok(());
    }

    pub fn to_toml(&self) -> String {
        let theme = theme_name(self.theme);
        return format!(
            "# mTrak settings. Delete this file to get the defaults back.\n\
             \n\
             # The theme to open with: \"auto\" picks one from what the terminal supports, or\n\
             # \"mono\", \"ansi16\", \"full\" or \"custom\" (the colors in theme.toml).\n\
             # --color overrides this.\n\
             theme = \"{theme}\"\n"
        );
    }
}

/// Every value `theme` can take, `None` being "auto".
pub fn theme_options() -> Vec<Option<ColorMode>> {
    return once(None)
        .chain(ColorMode::value_variants().iter().copied().map(Some))
        .collect();
}

/// How a `theme` value is written in config.toml.
pub fn theme_name(theme: Option<ColorMode>) -> String {
    return theme
        .and_then(|mode| mode.to_possible_value())
        .map(|value| value.get_name().to_owned())
        .unwrap_or_else(|| AUTO_THEME.to_owned());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_file_reads_back_as_the_default() {
        let config = Config::from_toml(&Config::default().to_toml()).unwrap();
        assert!(config.theme.is_none());
    }

    #[test]
    fn every_theme_option_reads_back() {
        for theme in theme_options() {
            let config = Config::from_toml(&Config { theme }.to_toml()).unwrap();
            assert_eq!(config.theme, theme);
        }
    }

    #[test]
    fn reads_a_theme() {
        let config = Config::from_toml("theme = \"custom\"").unwrap();
        assert!(matches!(config.theme, Some(ColorMode::Custom)));
        assert!(Config::from_toml("theme = \"pink\"").is_err());
        assert!(Config::from_toml("volume = \"11\"").is_err());
    }
}
