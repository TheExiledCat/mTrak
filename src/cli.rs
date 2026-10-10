use std::path::PathBuf;

use clap_derive::{Parser, ValueEnum};
#[derive(Parser, Debug)]
#[command(version, about = "M-Trak, the mini tracker for your terminal")]
pub struct Cli {
    pub project_file: Option<PathBuf>,
    #[arg(long)]
    pub ascii_mode: bool,
    #[arg(long, value_enum)]
    pub color: Option<ColorMode>,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum ColorMode {
    Mono,
    #[value(name = "16")]
    Ansi16,
    Full,
}
