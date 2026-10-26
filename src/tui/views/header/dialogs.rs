use chrono::{Datelike, Local};
use figlet_rs::FIGlet;

use crate::{
    data::{
        config::TITLE,
        pattern::{ClearOption, ClearTarget},
    },
    tui::{
        app::AppState,
        framework::{
            core_extensions::Boxed,
            dialog::{
                Checkbox, Dialog, DialogComponent, DialogResult, InfoLevel, Label, Link, NumberBox,
                Select, TextPrompt,
            },
        },
        glyphs,
    },
};

/// The year mTrak was started, for the copyright line in the About dialog.
const FIRST_YEAR: i32 = 2026;

const REPOSITORY_URL: &str = "https://github.com/TheExiledCat/mTrak";

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
    // All goes last so the digits match the instrument numbers
    let mut instruments: Vec<String> = state
        .project()
        .instruments
        .iter()
        .enumerate()
        .map(|(i, instrument)| format!("{i:X} {}", instrument.name).trim_end().to_owned())
        .collect();
    let all_instruments = instruments.len();
    instruments.push("All".to_owned());
    let dialog = Dialog::new(
        "Clear",
        vec![
            Select::new("clear", options).label("Selection").boxed(),
            Select::new("target", targets).label("Target").boxed(),
            Select::new("instrument", instruments)
                .label("Instrument")
                .selected(all_instruments)
                .boxed(),
        ],
    )
    .on_submit(move |state, result| {
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
        let instrument = result
            .choice("instrument")
            .filter(|&choice| choice != all_instruments)
            .map(|i| i as u8);
        state.request_clear(option, target, instrument);
        return Ok(());
    });

    state.open_dialog(dialog);
}

/// Previews changes live; cancelling restores the old values.
pub fn open_instrument_dialog(state: &mut AppState) {
    let instrument = state.active_instrument;
    let original = state.project().instruments[instrument as usize].clone();
    let had_changes = state.has_changes();
    let settings = &original;
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
    .on_update(move |state, fields| {
        apply_instrument(state, instrument, fields);
        return Ok(());
    })
    .on_submit(move |state, result| {
        apply_instrument(state, instrument, &result);
        return Ok(());
    })
    .on_cancel(move |state| {
        state.project_mut().instruments[instrument as usize].name = original.name;
        state.set_global_volume(instrument, original.volume);
        state.set_bend_range(instrument, original.pitch_wheel_range);
        state.set_default_cc(instrument, original.default_cc);
        state.set_instrument_transpose(instrument, original.transpose);
        state.restore_has_changes(had_changes);
    });
    state.open_dialog(dialog);
}

fn apply_instrument(state: &mut AppState, instrument: u8, result: &DialogResult) {
    if let Some(name) = result.text("name") {
        state.project_mut().instruments[instrument as usize].name = name.into();
    }
    if let Some(volume) = result.number("volume") {
        state.set_global_volume(instrument, volume as u8);
    }
    if let Some(range) = result.number("pitch_wheel_range") {
        state.set_bend_range(instrument, range as u8);
    }
    if let Some(control) = result.number("default_cc") {
        state.set_default_cc(instrument, control as u8);
    }
    if let Some(transpose) = result.number("transpose") {
        state.set_instrument_transpose(instrument, transpose as i8);
    }
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
        .cancellable(false);
    state.open_dialog(dialog);
}

/// Previews changes live; cancelling restores the old values.
pub fn open_global_dialog(state: &mut AppState) {
    let velocity = state.project().global_velocity;
    let transpose = state.project().transpose;
    let had_changes = state.has_changes();
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
    .on_update(|state, fields| {
        apply_globals(state, fields);
        return Ok(());
    })
    .on_submit(|state, result| {
        apply_globals(state, &result);
        return Ok(());
    })
    .on_cancel(move |state| {
        state.set_global_velocity(velocity);
        state.set_transpose(transpose);
        state.restore_has_changes(had_changes);
    });
    state.open_dialog(dialog);
}

fn apply_globals(state: &mut AppState, result: &DialogResult) {
    if let Some(velocity) = result.number("velocity") {
        state.set_global_velocity(velocity as u8);
    }
    if let Some(transpose) = result.number("transpose") {
        state.set_transpose(transpose as i8);
    }
}
