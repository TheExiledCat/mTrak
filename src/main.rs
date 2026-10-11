use std::io;

use clap::Parser;
use cli::Cli;
use data::project::Project;
use ratatui::crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
};
use tui::{app::App, files, glyphs, theme::Theme};

pub mod cli;
pub mod data;
pub mod engine;
pub mod tui;
pub mod util;

fn main() -> Result<(), io::Error> {
    let args = Cli::parse();
    glyphs::set_ascii_mode(args.ascii_mode);
    if let Err(e) = files::initialize_directories() {
        eprintln!("Could not create the mTrak directories: {e}");
    }
    let terminal = ratatui::init();
    execute!(io::stdout(), EnableMouseCapture)?;
    let mut app = App::new(
        terminal,
        120,
        Theme::from_color_mode(args.color),
        if let Some(path) = &args.project_file {
            Project::load(path.clone())
        } else {
            Project::empty()
        },
    );
    app.state.project_path = args.project_file;
    app.terminal.clear().unwrap();
    while app.run()? {}

    execute!(io::stdout(), DisableMouseCapture)?;
    ratatui::restore();

    println!("Application exit_requested");
    return Ok(());
}
