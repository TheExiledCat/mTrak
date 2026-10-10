use std::io;

use clap::Parser;
use cli::Cli;
use data::project::Project;
use ratatui::crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
};
use tui::app::App;

pub mod cli;
pub mod data;
pub mod engine;
pub mod tui;
pub mod util;

fn main() -> Result<(), io::Error> {
    let args = Cli::parse();
    let terminal = ratatui::init();
    execute!(io::stdout(), EnableMouseCapture)?;
    let mut app = App::new(
        terminal,
        120,
        if let Some(path) = args.project_file {
            Project::new(path)
        } else {
            Project::empty()
        },
    );
    app.terminal.clear().unwrap();
    while app.run()? {}

    execute!(io::stdout(), DisableMouseCapture)?;
    ratatui::restore();

    println!("Application exit_requested");
    return Ok(());
}
