use std::path::PathBuf;

use clap_derive::Parser;

use crate::data::config::ThemeSetting;
#[derive(Parser, Debug)]
#[command(version, about = "mTrak, the terminal midi tracker")]
pub struct Cli {
    pub project_file: Option<PathBuf>,
    /// Draw with ASCII characters only, for this session
    #[arg(long)]
    pub ascii_mode: bool,
    /// The theme for this session: auto, mono, ansi16, full or the name of a theme in the
    /// themes folder
    #[arg(long, alias = "theme")]
    pub color: Option<ThemeSetting>,
    /// Skip looking for a new version this time
    #[arg(long)]
    pub no_update_check: bool,
}
