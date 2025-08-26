use std::io;

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
fn main() -> Result<(), io::Error> {
    let args = Cli::parse();
    let terminal = ratatui::init();
    let mut app = App::new(terminal, 1, Project::new(args.project_file));
    let mut pitch = 0;
    let mut final_row = String::new();
    while app.draw() {
        {
            let mut state = app.state.borrow_mut();
            let mut pattern = &mut state.project.patterns[0];
            let row = &mut pattern.rows[0];
            row.dirty = true;
            row.channels[0] = NoteEvent::Note(data::note::Note {
                pitch: NotePitch::new(pitch),
                instrument_id: InstrumentId(0),
                volume: 0,
                effects: data::effect::Effects {
                    chain: [None, None, None, None],
                },
            });
        }

        let state = &mut *app.state.borrow_mut();
        let pattern = &mut state.project.patterns[0];
        state.channel_cache.update_dirty(pattern, 0, 0);
        pitch += 1;
    }

    ratatui::restore();

    println!("Application exit_requested");
    println!("{:?}", final_row);
    return Ok(());
}
