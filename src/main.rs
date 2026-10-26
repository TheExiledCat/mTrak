use std::{io, process::ExitCode};

use clap::Parser;
use cli::Cli;
use data::{
    config::{Config, ThemeSetting},
    project::Project,
};
use error::{MTrakError, Result};
use ratatui::{
    DefaultTerminal,
    crossterm::{
        event::{DisableMouseCapture, EnableMouseCapture},
        execute,
    },
};
use tui::{app::App, files, glyphs, keybinds, theme::Theme};
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
    let mut startup_errors = Vec::new();
    let initialized = match files::initialize() {
        Ok(()) => true,
        Err(e) => {
            startup_errors.push(e);
            false
        }
    };
    // without a config folder there's nothing to load, and its error is already reported
    let mut config = if initialized {
        files::get_config_file_path()
            .and_then(|path| Config::load(&path))
            .unwrap_or_else(|e| {
                startup_errors.push(e);
                Config::default()
            })
    } else {
        Config::default()
    };
    config.ascii_mode |= args.ascii_mode;
    glyphs::set_ascii_mode(config.ascii_mode);
    updater::remove_old_exe();

    if !args.no_update_check && config.update_check {
        if let UpdateOutcome::Updated(version) = updater::check_for_update() {
            println!(
                "mTrak was updated to v{}. Start it again to use the new version.",
                version
            );
            return ExitCode::SUCCESS;
        }
    }
    let terminal = ratatui::init();
    let result = run(terminal, args, config, initialized, startup_errors);
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
fn run(
    terminal: DefaultTerminal,
    args: Cli,
    mut config: Config,
    initialized: bool,
    mut startup_errors: Vec<MTrakError>,
) -> Result<()> {
    execute!(io::stdout(), EnableMouseCapture)?;
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

    if let Some(theme) = args.color {
        config.theme = theme;
    }
    if !initialized && matches!(config.theme, ThemeSetting::Named(_)) {
        config.theme = ThemeSetting::Auto;
    }
    let theme = Theme::from_setting(&config.theme).unwrap_or_else(|e| {
        startup_errors.push(e);
        config.theme = ThemeSetting::Auto;
        Theme::detect()
    });

    if initialized {
        let loaded = files::get_keybinds_file_path().and_then(|path| {
            return if path.exists() {
                keybinds::load(&path)
            } else {
                Ok(())
            };
        });
        if let Err(e) = loaded {
            startup_errors.push(e);
        }
    }

    let mut app = App::new(terminal, 120, theme, project)?;
    // the keymaps exist once the app does, so every action is known now
    if let Err(e) = keybinds::set_entries(&app.help_sections()) {
        startup_errors.push(e);
    }
    if initialized {
        let created = files::get_keybinds_file_path().and_then(|path| {
            return if path.exists() {
                Ok(())
            } else {
                keybinds::save(&path)
            };
        });
        if let Err(e) = created {
            startup_errors.push(e);
        }
    }
    app.state.project_path = project_path;
    app.state.apply_config(config);
    for error in startup_errors {
        app.state.report_error(error);
    }
    app.terminal.clear()?;
    while app.run()? {}
    return Ok(());
}
