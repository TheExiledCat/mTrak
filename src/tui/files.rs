use std::{
    fs::{self},
    path::PathBuf,
};

const FOLDER_ROOT_NAME: &str = "mtrak";
type Result<T> = std::io::Result<T>;
pub fn initialize_directories() -> Result<()> {
    fn create_error() -> std::io::Error {
        return std::io::Error::other("failed to create default directory");
    }

    fs::create_dir_all(get_config_directory_root().ok_or_else(create_error)?)?;
    fs::create_dir_all(get_projects_directory_root().ok_or_else(create_error)?)?;
    return Ok(());
}

pub fn get_projects_directory_root() -> Option<PathBuf> {
    return dirs::audio_dir().map(|d| d.join(FOLDER_ROOT_NAME));
}
pub fn get_config_directory_root() -> Option<PathBuf> {
    return dirs::config_local_dir().map(|d| d.join(FOLDER_ROOT_NAME));
}
