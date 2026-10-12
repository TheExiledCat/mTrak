use std::path::PathBuf;

use clap_derive::{Parser, ValueEnum};
#[derive(Parser, Debug)]
#[command(version, about = "mTrak, the terminal midi tracker")]
pub struct Cli {
    pub project_file: Option<PathBuf>,
    #[arg(long)]
    pub ascii_mode: bool,
    #[arg(long, value_enum)]
    pub color: Option<ColorMode>,
    #[arg(long)]
    pub no_update_check: bool,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum ColorMode {
    Mono,
    Ansi16,
    Full,
}
