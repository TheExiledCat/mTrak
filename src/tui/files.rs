use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    data::config::{self, Config},
    error::{MTrakError, Result},
};

const FOLDER_ROOT_NAME: &str = "mtrak";
const CONFIG_FILE_NAME: &str = "config.toml";
const THEMES_FOLDER_NAME: &str = "themes";
/// Where the custom theme was kept before themes got their own folder.
const OLD_THEME_FILE_NAME: &str = "theme.toml";
const THEME_EXTENSION: &str = "toml";

/// Kept in the themes folder, so there's always something to pick from.
const EXAMPLE_THEMES: [(&str, &str); 8] = [
    (
        "catppuccin-latte",
        include_str!("../../example_themes/catppuccin-latte.toml"),
    ),
    (
        "catppuccin-mocha",
        include_str!("../../example_themes/catppuccin-mocha.toml"),
    ),
    ("dracula", include_str!("../../example_themes/dracula.toml")),
    (
        "gruvbox-dark",
        include_str!("../../example_themes/gruvbox-dark.toml"),
    ),
    ("nord", include_str!("../../example_themes/nord.toml")),
    (
        "one-dark",
        include_str!("../../example_themes/one-dark.toml"),
    ),
    (
        "solarized-dark",
        include_str!("../../example_themes/solarized-dark.toml"),
    ),
    (
        "tokyo-night",
        include_str!("../../example_themes/tokyo-night.toml"),
    ),
];

/// Creates mTrak's folders, and the config files with their defaults when they don't exist yet.
pub fn initialize() -> Result<()> {
    let config = get_config_directory_root().ok_or(MTrakError::MissingDirectory("config"))?;
    fs::create_dir_all(config)?;
    create_if_missing(&get_config_file_path()?, || Config::default().to_toml())?;
    fs::create_dir_all(get_themes_directory()?)?;
    for (name, text) in EXAMPLE_THEMES {
        create_if_missing(&get_named_theme_path(name)?, || text.to_owned())?;
    }
    move_old_theme_file()?;
    let projects = get_projects_directory_root().ok_or(MTrakError::MissingDirectory("music"))?;
    fs::create_dir_all(projects)?;
    return Ok(());
}

pub fn get_projects_directory_root() -> Option<PathBuf> {
    return dirs::audio_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join("Music")))
        .map(|d| d.join(FOLDER_ROOT_NAME));
}
pub fn get_config_directory_root() -> Option<PathBuf> {
    return dirs::config_local_dir().map(|d| d.join(FOLDER_ROOT_NAME));
}
pub fn get_config_file_path() -> Result<PathBuf> {
    return config_file(CONFIG_FILE_NAME);
}
pub fn get_themes_directory() -> Result<PathBuf> {
    return config_file(THEMES_FOLDER_NAME);
}
pub fn get_named_theme_path(name: &str) -> Result<PathBuf> {
    return Ok(get_themes_directory()?
        .join(name)
        .with_extension(THEME_EXTENSION));
}

/// The names of the themes in the themes folder, sorted, skipping files whose name can't be
/// used as a theme name.
pub fn list_themes() -> Vec<String> {
    let Ok(entries) = get_themes_directory().and_then(|dir| Ok(fs::read_dir(dir)?)) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(|entry| Some(entry.ok()?.path()))
        .filter(|path| path.extension().is_some_and(|e| e == THEME_EXTENSION))
        .filter_map(|path| Some(path.file_stem()?.to_str()?.to_owned()))
        .filter(|name| config::is_theme_name(name))
        .collect();
    names.sort();
    return names;
}

fn config_file(name: &str) -> Result<PathBuf> {
    let root = get_config_directory_root().ok_or(MTrakError::MissingDirectory("config"))?;
    return Ok(root.join(name));
}

/// Moves theme.toml from older versions into the themes folder as the custom theme, which
/// `theme = "custom"` in config.toml now names. A custom theme already there is left alone.
fn move_old_theme_file() -> Result<()> {
    let old = config_file(OLD_THEME_FILE_NAME)?;
    let new = get_named_theme_path(config::CUSTOM_THEME)?;
    if old.exists() && !new.exists() {
        fs::rename(old, new)?;
    }
    return Ok(());
}

fn create_if_missing(path: &Path, default: impl FnOnce() -> String) -> Result<()> {
    if !path.exists() {
        fs::write(path, default())?;
    }
    return Ok(());
}
