use std::io;

use tui::app::App;

pub mod data;
pub mod tui;
fn main() -> Result<(), io::Error> {
    let terminal = ratatui::init();
    let mut app = App::new(terminal, 30);

    while app.draw() {}
    ratatui::restore();

    println!("Application exit_requested");
    return Ok(());
}
