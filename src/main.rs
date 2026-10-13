use std::{io, process::ExitCode};

use clap::Parser;
use cli::Cli;
use data::project::Project;
use error::{MTrakError, Result};
use ratatui::{
    DefaultTerminal,
    crossterm::{
        event::{DisableMouseCapture, EnableMouseCapture},
        execute,
    },
};
use tui::{app::App, files, glyphs, theme::Theme};
use updater::UpdateOutcome;

mod cli;
mod data;
mod engine;
mod error;
mod tui;
mod updater;
mod util;

fn main() -> ExitCode {
    let args = Cli::parse();
    glyphs::set_ascii_mode(args.ascii_mode);
    updater::remove_old_exe();

    if !args.no_update_check {
        if let UpdateOutcome::Updated(version) = updater::check_for_update() {
            println!(
                "mTrak was updated to v{}. Start it again to use the new version.",
                version
            );
            return ExitCode::SUCCESS;
        }
    }
    let terminal = ratatui::init();
    let result = run(terminal, args);
    let _ = execute!(io::stdout(), DisableMouseCapture);
    ratatui::restore();

    if let Err(e) = result {
        eprintln!("mTrak stopped because of an error: {}", e);
        return ExitCode::FAILURE;
    }
    println!("Application exit_requested");
    return ExitCode::SUCCESS;
}

/// Runs the app until the user quits. Errors that leave the app usable are shown in a
/// dialog instead; only errors that stop the app are returned.
fn run(terminal: DefaultTerminal, args: Cli) -> Result<()> {
    execute!(io::stdout(), EnableMouseCapture)?;
    let mut startup_errors = Vec::new();
    if let Err(e) = files::initialize_directories() {
        startup_errors.push(e);
    }
    let (project, project_path) = match args.project_file {
        Some(path) => match Project::load(path.clone()) {
            Ok(project) => (project, Some(path)),
            Err(MTrakError::Io(e)) if e.kind() == io::ErrorKind::NotFound => {
                (Project::empty(), Some(path))
            }
            Err(e) => {
                startup_errors.push(e);
                (Project::empty(), None)
            }
        },
        None => (Project::empty(), None),
    };

    let mut app = App::new(terminal, 120, Theme::from_color_mode(args.color), project)?;
    app.state.project_path = project_path;
    for error in startup_errors {
        app.state.report_error(error);
    }
    app.terminal.clear()?;
    while app.run()? {}
    return Ok(());
}
