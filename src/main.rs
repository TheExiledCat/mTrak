use std::io;

use clap::Parser;
use cli::Cli;
use data::project::Project;
use tui::app::App;

pub mod cli;
pub mod data;
pub mod tui;
fn main() -> Result<(), io::Error> {
    let args = Cli::parse();
    let terminal = ratatui::init();
    let mut app = App::new(terminal, 30, Project::new(args.project_file));
    app.state.project.save()?;

    while app.draw() {}
    ratatui::restore();

    println!("Application exit_requested");
    return Ok(());
}
