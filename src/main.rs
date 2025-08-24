use std::io;

use data::project::Project;
use tui::app::App;

pub mod data;
pub mod tui;
fn main() -> Result<(), io::Error> {
    let terminal = ratatui::init();
    let mut app = App::new(terminal, 30, Project::new("./sandbox/test.mtrk".into()));

    while app.draw() {}
    ratatui::restore();

    println!("Application exit_requested");
    return Ok(());
}
