use std::{
    io,
    time::{Duration, Instant},
};

use clap::Parser;
use cli::Cli;
use data::{
    note::{InstrumentId, NoteEvent, NotePitch},
    pattern::PatternRow,
    project::Project,
};
use ratatui::layout::Rows;
use tui::app::App;

pub mod cli;
pub mod data;
pub mod tui;
pub mod util;
use util::timers::{self, set_timeout};
fn main() -> Result<(), io::Error> {
    let args = Cli::parse();
    let terminal = ratatui::init();
    let mut app = App::new(terminal, 60, Project::new(args.project_file));
    while app.draw() {}

    ratatui::restore();

    println!("Application exit_requested");
    return Ok(());
}
