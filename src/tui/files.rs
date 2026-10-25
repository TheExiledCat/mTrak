use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    data::config::Config,
    error::{MTrakError, Result},
    tui::theme::Palette,
};

const FOLDER_ROOT_NAME: &str = "mtrak";
const CONFIG_FILE_NAME: &str = "config.toml";
const THEME_FILE_NAME: &str = "theme.toml";

/// Creates mTrak's folders, and the config files with their defaults when they don't exist yet.
pub fn initialize() -> Result<()> {
    let config = get_config_directory_root().ok_or(MTrakError::MissingDirectory("config"))?;
    fs::create_dir_all(config)?;
    create_if_missing(&get_config_file_path()?, || Config::default().to_toml())?;
    create_if_missing(&get_theme_file_path()?, Palette::default_toml)?;
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
pub fn get_theme_file_path() -> Result<PathBuf> {
    return config_file(THEME_FILE_NAME);
}
fn config_file(name: &str) -> Result<PathBuf> {
    let root = get_config_directory_root().ok_or(MTrakError::MissingDirectory("config"))?;
    return Ok(root.join(name));
}

fn create_if_missing(path: &Path, default: impl FnOnce() -> String) -> Result<()> {
    if !path.exists() {
        fs::write(path, default())?;
    }
    return Ok(());
}
