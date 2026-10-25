use chrono::{Datelike, Local};
use figlet_rs::FIGlet;

use crate::{
    data::{
        config::{self, TITLE, ThemeChoice},
        pattern::{ClearOption, ClearTarget},
    },
    error::Result,
    tui::{
        app::AppState,
        files,
        framework::{
            core_extensions::Boxed,
            dialog::{
                Checkbox, Dialog, DialogComponent, InfoLevel, Label, Link, NumberBox, Select,
                TextPrompt,
            },
        },
        glyphs,
        theme::Theme,
    },
};

/// The year mTrak was started, for the copyright line in the About dialog.
const FIRST_YEAR: i32 = 2026;

const REPOSITORY_URL: &str = "https://github.com/TheExiledCat/mTrak";

/// Settings that aren't part of the project, so they're written to config.toml straight away
/// unless they're only meant for this session.
pub fn open_config_dialog(state: &mut AppState) {
    let themes = config::theme_options();
    let names: Vec<String> = themes.iter().map(|t| config::theme_name(*t)).collect();
    let current = themes
        .iter()
        .position(|t| *t == state.theme_choice)
        .unwrap_or(0);
    let dialog = Dialog::new(
        "Config",
        vec![
            Select::new("theme", names)
                .label("Theme")
                .selected(current)
                .boxed(),
            Checkbox::new("save", "Save to config.toml")
                .checked(true)
                .boxed(),
        ],
    )
    .on_submit(move |state, result| {
        let theme = result
            .choice("theme")
            .map_or(state.theme_choice, |choice| themes[choice]);
        return apply_config(state, theme, result.bool("save").unwrap_or(false));
    });
    state.open_dialog(dialog);
}

/// Switches to `theme`, and makes it the theme to open with when `save` is set. Nothing
/// changes when the theme can't be built, e.g. a custom theme with a broken theme.toml.
fn apply_config(state: &mut AppState, theme: Option<ThemeChoice>, save: bool) -> Result<()> {
    state.theme = Theme::from_choice(theme)?;
    state.theme_choice = theme;
    if save {
        let mut config = state.config.clone();
        config.theme = theme;
        config.save(&files::get_config_file_path()?)?;
        state.config = config;
    }
    return Ok(());
}

pub fn open_midi_dialog(state: &mut AppState) {
    let mut in_names = state.get_midi_in_port_names();
    in_names.insert(0, "(none)".into());
    let mut out_names = state.get_midi_out_port_names();
    out_names.insert(0, "(none)".into());

    let mut clock_names = state.get_midi_in_port_names();
    clock_names.insert(0, "(internal)".into());
    let current_in = state
        .active_in_port
        .as_ref()
        .zip(state.midi_in_client.as_ref())
        .and_then(|(p, client)| client.port_name(p).ok())
        .and_then(|name| in_names.iter().position(|n| *n == name))
        .unwrap_or(0);

    let current_clock = state
        .active_clock_name()
        .and_then(|name| clock_names.iter().position(|n| *n == name))
        .unwrap_or(0);

    let current_out = state
        .active_out_port
        .as_ref()
        .zip(state.midi_out_client.as_ref())
        .and_then(|(p, client)| client.port_name(p).ok())
        .and_then(|name| out_names.iter().position(|n| *n == name))
        .unwrap_or(0);
    let dialog = Dialog::new(
        "MIDI",
        vec![
            Select::new("input", in_names)
                .label("MIDI input")
                .selected(current_in)
                .boxed(),
            Select::new("clock", clock_names)
                .label("Clock SRC")
                .selected(current_clock)
                .boxed(),
            Checkbox::new("velocity", "Record Velocity")
                .checked(state.project().record_input_velocity)
                .boxed(),
            Select::new("output", out_names)
                .label("MIDI output")
                .selected(current_out)
                .boxed(),
            Checkbox::new("midi_clock", "Send MIDI clock")
                .checked(state.project().send_midi_clock)
                .boxed(),
        ],
    )
    .on_submit(|state, result| {
        if let Some(midi_clock) = result.bool("midi_clock") {
            state.set_midi_clock(midi_clock);
        }
        if let Some(velocity) = result.bool("velocity") {
            state.set_midi_velocity_record(velocity);
        }
        if let Some(output) = result.choice("output") {
            let output = if output == 0 {
                None
            } else {
                state
                    .midi_out_client
                    .as_ref()
                    .and_then(|client| client.ports().get(output - 1).cloned())
            };
            state.set_midi_output(output);
        }
        // inputs last, so a port that fails to open doesn't drop the other changes
        let clock = result
            .choice("clock")
            .map(|clock| state.set_midi_sync(clock));
        let input = result.choice("input").map(|input| {
            let port = state.input_port_for_choice(input);
            state.set_midi_input(port)
        });
        clock.unwrap_or(Ok(()))?;
        return input.unwrap_or(Ok(()));
    });
    state.open_dialog(dialog);
}

pub fn open_clear_dialog(state: &mut AppState) {
    let options = vec!["Block", "Track", "Pattern"];
    let targets = vec!["All", "Note", "Vel", "Cmd"];
    let dialog = Dialog::new(
        "Clear",
        vec![
            Select::new("clear", options).label("Selection").boxed(),
            Select::new("target", targets).label("Target").boxed(),
        ],
    )
    .on_submit(|state, result| {
        use ClearOption::*;
        use ClearTarget::*;
        let option = match result.choice("clear").unwrap_or(0) {
            0 => Block,
            1 => Track,
            2 => Pattern,
            _ => unreachable!(),
        };
        let target = match result.choice("target").unwrap_or(0) {
            0 => All,
            1 => Note,
            2 => Velocity,
            3 => Command,
            _ => unreachable!(),
        };
        state.request_clear(option, target);
        return Ok(());
    });

    state.open_dialog(dialog);
}

pub fn open_instrument_dialog(state: &mut AppState) {
    let instrument = state.active_instrument;
    let settings = &state.project().instruments[instrument as usize];
    let dialog = Dialog::new(
        format!("Instrument {:1X}", instrument),
        vec![
            TextPrompt::new("name", "Name")
                .initial(&settings.name)
                .boxed(),
            NumberBox::new("volume", "Volume", 2)
                .value(settings.volume as i32)
                .boxed(),
            NumberBox::new("pitch_wheel_range", "Bend range (semitones)", 2)
                .value(settings.pitch_wheel_range as i32)
                .boxed(),
            NumberBox::new("default_cc", "Default CC", 2)
                .range(0, 0x7F)
                .value(settings.default_cc as i32)
                .boxed(),
            NumberBox::new("transpose", "Transpose (semitones)", 2)
                .range(i8::MIN as i32, i8::MAX as i32)
                .value(settings.transpose as i32)
                .boxed(),
        ],
    )
    .focus_first()
    .on_submit(move |state, result| {
        if let Some(name) = result.text("name") {
            state.project_mut().instruments[instrument as usize].name = name.into();
        }
        if let Some(volume) = result.number("volume") {
            state.set_global_volume(instrument, volume as u8);
        }
        if let Some(range) = result.number("pitch_wheel_range") {
            state.project_mut().instruments[instrument as usize].pitch_wheel_range = range as u8;
        }
        if let Some(control) = result.number("default_cc") {
            state.set_default_cc(instrument, control as u8);
        }
        if let Some(transpose) = result.number("transpose") {
            state.set_instrument_transpose(instrument, transpose as i8);
        }
        return Ok(());
    });
    state.open_dialog(dialog);
}

pub fn open_about_dialog(state: &mut AppState) {
    let logo = FIGlet::small()
        .ok()
        .and_then(|font| font.convert(TITLE).map(|figure| figure.to_string()))
        .unwrap_or_else(|| TITLE.to_owned());
    let year = Local::now().year();
    let years = if year > FIRST_YEAR {
        format!("{FIRST_YEAR}-{year}")
    } else {
        FIRST_YEAR.to_string()
    };
    let build = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    // pad the logo's lines to one width so centering keeps them lined up
    let width = logo
        .lines()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0);
    let logo: Vec<String> = logo
        .trim_end()
        .lines()
        .map(|line| format!("{line:<width$}"))
        .collect();
    let lines = [
        logo.join("\n"),
        String::new(),
        format!("v{}", env!("CARGO_PKG_VERSION")),
        format!(
            "{build} build, {} {}",
            std::env::consts::OS,
            std::env::consts::ARCH
        ),
        String::new(),
        "A MIDI tracker for the terminal".to_owned(),
        "Created by TheExiledCat".to_owned(),
        format!(
            "{} {years} TheExiledCat, MIT License",
            glyphs::pick("\u{a9}", "(c)")
        ),
    ];
    let mut components: Vec<Box<dyn DialogComponent>> = lines
        .into_iter()
        .map(|line| Label::new(line).centered().boxed() as Box<dyn DialogComponent>)
        .collect();
    components.push(Link::new(REPOSITORY_URL).boxed());
    let dialog = Dialog::new("About", components)
        .level(InfoLevel::Success)
        .is_cancellable(false);
    state.open_dialog(dialog);
}

pub fn open_global_dialog(state: &mut AppState) {
    let dialog = Dialog::new(
        "Global",
        vec![
            NumberBox::new("velocity", "Velocity", 2)
                .value(state.project().global_velocity as i32)
                .boxed(),
            NumberBox::new("transpose", "Transpose (semitones)", 2)
                .range(i8::MIN as i32, i8::MAX as i32)
                .value(state.project().transpose as i32)
                .boxed(),
        ],
    )
    .focus_first()
    .on_submit(move |state, result| {
        if let Some(velocity) = result.number("velocity") {
            state.set_global_velocity(velocity as u8);
        }
        if let Some(transpose) = result.number("transpose") {
            state.set_transpose(transpose as i8);
        }
        return Ok(());
    });
    state.open_dialog(dialog);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::project::Project;

    #[test]
    fn a_session_only_config_switches_the_theme() {
        let mut state = AppState::new(Project::empty(), Theme::monochrome()).unwrap();
        apply_config(&mut state, Some(ThemeChoice::Ansi16), false).unwrap();
        assert_eq!(state.theme_choice, Some(ThemeChoice::Ansi16));
        assert_eq!(state.config.theme, None);
        assert_eq!(state.theme.background, Theme::ansi16().background);
    }
}
