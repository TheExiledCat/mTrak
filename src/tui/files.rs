use std::{fs, path::PathBuf};

use crate::error::{MTrakError, Result};

const FOLDER_ROOT_NAME: &str = "mtrak";

pub fn initialize_directories() -> Result<()> {
    let config = get_config_directory_root().ok_or(MTrakError::MissingDirectory("config"))?;
    fs::create_dir_all(config)?;
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
