use std::path::PathBuf;

use clap_derive::Parser;
#[derive(Parser, Debug)]
#[command(version, about = "M-Trak, the mini tracker for your terminal")]
pub struct Cli {
    pub project_file: Option<PathBuf>,
    #[arg(long)]
    pub ascii_mode: bool,
}
