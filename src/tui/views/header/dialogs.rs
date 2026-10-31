use chrono::{Datelike, Local};
use figlet_rs::FIGlet;
use strum::IntoEnumIterator;

use crate::{
    data::{
        config::TITLE,
        effect::EffectKind,
        metronome::MetronomeMode,
        midi::{CC_MAX, PITCH_MAX, VELOCITY_MAX, cc_name},
        note::{NotePitch, SEMITONES_PER_OCTAVE},
        pattern::{ClearOption, ClearTarget, Remap, RemapScope},
        project::ProjectStats,
    },
    engine::effects::{Param, ParamKind, ValueLabel, effect_params},
    tui::{
        app::AppState,
        framework::{
            core_extensions::Boxed,
            dialog::{
                Buttons, Checkbox, Dialog, DialogComponent, DialogResult, InfoLevel, Label, Link,
                List, ListEvent, NumberBox, Select, TextPrompt,
            },
        },
        glyphs,
    },
    util::time::duration_to_hh_mm_ss_string,
};

/// The year mTrak was started, for the copyright line in the About dialog.
const FIRST_YEAR: i32 = 2026;

const REPOSITORY_URL: &str = "https://github.com/TheExiledCat/mTrak";

const CC_MAPPING_ID: &str = "cc_mapping";
const CC_NAME_WIDTH: usize = 16;
/// In the order of `remap_scope`.
const REMAP_SCOPES: [&str; 4] = ["Block", "Track", "Pattern", "Song"];
/// C-4, where the pitch remap starts.
const MIDDLE_C: u8 = 60;

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
            Checkbox::new("soft_thru", "Soft thru")
                .checked(state.config.soft_thru)
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
        let soft_thru = result
            .bool("soft_thru")
            .map(|enabled| state.set_soft_thru(enabled));
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
        soft_thru.unwrap_or(Ok(()))?;
        return input.unwrap_or(Ok(()));
    });
    state.open_dialog(dialog);
}

pub fn open_clear_dialog(state: &mut AppState) {
    let options = vec!["Block", "Track", "Pattern"];
    let targets = vec!["All", "Note", "Vel", "Cmd"];
    // All goes last so the digits match the instrument numbers
    let mut instruments = instrument_names(state);
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

pub fn open_remap_dialog(state: &mut AppState) {
    let dialog = Dialog::new(
        "Remap",
        vec![
            Select::new("kind", vec!["Instruments", "Pitches"])
                .label("Remap")
                .boxed(),
        ],
    )
    .focus_first()
    .on_submit(|state, result| {
        match result.choice("kind") {
            Some(0) => open_remap_instruments_dialog(state),
            Some(_) => open_remap_pitches_dialog(state),
            None => {}
        }
        return Ok(());
    });
    state.open_dialog(dialog);
}

/// The name is saved as it's typed, so Esc closes it like Enter.
pub fn open_info_dialog(state: &mut AppState) {
    let project = state.project();
    let stats = project.stats();
    let file = state
        .project_path
        .as_ref()
        .and_then(|path| path.file_name())
        .map_or("Not saved yet".to_owned(), |name| {
            name.to_string_lossy().into_owned()
        });
    let name = TextPrompt::new("name", "Name").initial(project.name.as_deref().unwrap_or_default());
    let lines = [
        format!("File: {file}"),
        format!("Size: {}", file_size(stats.file_size)),
        format!("Tempo: {} BPM, speed {}", project.bpm, project.speed),
        format!(
            "Song: {}, {} rows, {}",
            count(project.sequence.len(), "position"),
            stats.song_rows,
            duration_to_hh_mm_ss_string(&stats.song_length)
        ),
        format!("Patterns: {}", pattern_counts(&stats)),
        format!("Notes: {}, note offs: {}", stats.notes, stats.note_offs),
        format!("Commands: {}", stats.commands),
        format!(
            "Instruments: {} of {} used",
            stats.instruments_used,
            project.instruments.len()
        ),
        format!("Effects: {}", stats.effects),
    ];
    let mut components = vec![name.boxed() as Box<dyn DialogComponent>];
    components.extend(
        lines
            .into_iter()
            .map(|line| Label::new(line).boxed() as Box<dyn DialogComponent>),
    );
    let dialog = Dialog::new("Info", components)
        .close_on_esc()
        .focus_first()
        .on_update(|state, fields| {
            if let Some(name) = fields.text("name") {
                set_project_name(state, name);
            }
            return Ok(());
        });
    state.open_dialog(dialog);
}

/// Changes are made right away, so Esc closes it like Enter.
pub fn open_instrument_dialog(state: &mut AppState) {
    let instrument = state.active_instrument;
    let settings = &state.project().instruments[instrument as usize];
    let dialog = Dialog::new(
        format!("Instrument {:1X}", instrument),
        vec![
            TextPrompt::new("name", "Name")
                .initial(&settings.name)
                .boxed(),
            labelled(
                NumberBox::new("volume", "Volume", 2)
                    .show_range()
                    .value(settings.volume as i32),
                ValueLabel::Percent,
            )
            .boxed(),
            NumberBox::new("pitch_wheel_range", "Bend range (semitones)", 2)
                .show_range()
                .value(settings.pitch_wheel_range as i32)
                .boxed(),
            Select::new("default_cc", cc_names(state))
                .label("Default CC")
                .selected(settings.default_cc as usize)
                .boxed(),
            labelled(
                NumberBox::new("transpose", "Transpose (semitones)", 2)
                    .show_range()
                    .range(i8::MIN as i32, i8::MAX as i32)
                    .value(settings.transpose as i32),
                ValueLabel::Semitones,
            )
            .boxed(),
            Buttons::new("effects", vec!["Effects"]).boxed(),
        ],
    )
    .width(60)
    .hide_behind_child()
    .close_on_esc()
    .focus_first()
    .on_update(move |state, fields| {
        apply_instrument(state, instrument, fields);
        if fields.changed() == "effects" {
            open_effects_dialog(state);
        }
        return Ok(());
    })
    .on_submit(move |state, result| {
        apply_instrument(state, instrument, &result);
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
        .close_on_esc();
    state.open_dialog(dialog);
}

/// Changes are made right away, so Esc closes it like Enter.
pub fn open_global_dialog(state: &mut AppState) {
    let dialog = Dialog::new(
        "Global",
        vec![
            labelled(
                NumberBox::new("velocity", "Velocity", 2)
                    .show_range()
                    .value(state.project().global_velocity as i32),
                ValueLabel::Percent,
            )
            .boxed(),
            labelled(
                NumberBox::new("transpose", "Transpose (semitones)", 2)
                    .show_range()
                    .range(i8::MIN as i32, i8::MAX as i32)
                    .value(state.project().transpose as i32),
                ValueLabel::Semitones,
            )
            .boxed(),
        ],
    )
    .width(60)
    .close_on_esc()
    .focus_first()
    .on_update(|state, fields| {
        apply_globals(state, fields);
        return Ok(());
    })
    .on_submit(|state, result| {
        apply_globals(state, &result);
        return Ok(());
    });
    state.open_dialog(dialog);
}

/// Gives the cells that use one instrument another, e.g. after moving a synth to another MIDI
/// channel. Copying the settings gives the new instrument the old one's, and the old one the new
/// one's.
fn open_remap_instruments_dialog(state: &mut AppState) {
    let instrument = state.active_instrument as usize;
    let dialog = Dialog::new(
        "Remap instruments",
        vec![
            Select::new("scope", REMAP_SCOPES.to_vec())
                .label("Selection")
                .boxed(),
            Select::new("from", instrument_names(state))
                .label("From")
                .selected(instrument)
                .boxed(),
            Select::new("to", instrument_names(state))
                .label("To")
                .selected(instrument)
                .boxed(),
            Checkbox::new("swap", "Copy their settings").boxed(),
        ],
    )
    .focus_first()
    .on_submit(|state, result| {
        let from = result.choice("from").unwrap_or(0) as u8;
        let to = result.choice("to").unwrap_or(0) as u8;
        if from != to {
            let remap = Remap::Instrument { from, to };
            let swap = result.bool("swap").unwrap_or(false);
            state.request_remap(remap_scope(&result), remap, swap);
        }
        return Ok(());
    });
    state.open_dialog(dialog);
}

/// Plays the notes of one pitch at another, e.g. to move a drum to another key.
fn open_remap_pitches_dialog(state: &mut AppState) {
    let pitches = pitch_names();
    let middle_c = pitch_index(MIDDLE_C);
    let mut instruments = instrument_names(state);
    let all_instruments = instruments.len();
    instruments.push("All".to_owned());
    let dialog = Dialog::new(
        "Remap pitches",
        vec![
            Select::new("scope", REMAP_SCOPES.to_vec())
                .label("Selection")
                .boxed(),
            Select::new("from", pitches.clone())
                .label("From")
                .selected(middle_c)
                .page_step(SEMITONES_PER_OCTAVE as usize)
                .boxed(),
            Select::new("to", pitches)
                .label("To")
                .selected(middle_c)
                .page_step(SEMITONES_PER_OCTAVE as usize)
                .boxed(),
            Select::new("instrument", instruments)
                .label("Instrument")
                .selected(all_instruments)
                .boxed(),
        ],
    )
    .focus_first()
    .on_submit(move |state, result| {
        let pitch = |id| NotePitch(NotePitch::MIN + result.choice(id).unwrap_or(0) as u8);
        let (from, to) = (pitch("from"), pitch("to"));
        if from != to {
            let instrument = result
                .choice("instrument")
                .filter(|&choice| choice != all_instruments)
                .map(|i| i as u8);
            let remap = Remap::Pitch {
                from,
                to,
                instrument,
            };
            state.request_remap(remap_scope(&result), remap, false);
        }
        return Ok(());
    });
    state.open_dialog(dialog);
}

fn remap_scope(result: &DialogResult) -> RemapScope {
    return match result.choice("scope").unwrap_or(0) {
        0 => RemapScope::Block,
        1 => RemapScope::Track,
        2 => RemapScope::Pattern,
        _ => RemapScope::Song,
    };
}

/// An empty name leaves the project untitled.
fn set_project_name(state: &mut AppState, name: &str) {
    let name = Some(name.trim().to_owned()).filter(|name| !name.is_empty());
    if state.project().name != name {
        state.project_mut().name = name;
    }
}

fn instrument_names(state: &AppState) -> Vec<String> {
    return state
        .project()
        .instruments
        .iter()
        .enumerate()
        .map(|(i, instrument)| format!("{i:X} {}", instrument.name).trim_end().to_owned())
        .collect();
}

/// The patterns the song plays, how many of them are empty, and the ones with something in
/// them that it doesn't play.
fn pattern_counts(stats: &ProjectStats) -> String {
    let mut text = format!("{} in the song", stats.patterns_in_song);
    if stats.empty_in_song > 0 {
        text += &format!(", {} empty", stats.empty_in_song);
    }
    if stats.outside_song > 0 {
        text += &format!(", {} more not in it", stats.outside_song);
    }
    return text;
}

fn count(n: usize, thing: &str) -> String {
    return match n {
        1 => format!("1 {thing}"),
        n => format!("{n} {thing}s"),
    };
}

fn file_size(bytes: usize) -> String {
    return match bytes {
        0..1024 => format!("{bytes} bytes"),
        _ => format!("{:.1} KB ({bytes} bytes)", bytes as f64 / 1024.0),
    };
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
    if let Some(control) = result.choice("default_cc") {
        state.set_default_cc(instrument, control as u8);
    }
    if let Some(transpose) = result.number("transpose") {
        state.set_instrument_transpose(instrument, transpose as i8);
    }
}
/// The active instrument's effects, in the order notes go through them. Changes are made
/// right away, so Esc closes it like Enter.
fn open_effects_dialog(state: &mut AppState) {
    let instrument = state.active_instrument;
    let list = List::new("effects", effect_names(state, instrument))
        .items_from(move |state| effect_names(state, instrument))
        .checkboxes(effects_enabled(state, instrument))
        .checks_from(move |state| effects_enabled(state, instrument))
        .show_index(true)
        .add_button("Add effect")
        .removable(true)
        .reorderable(true);
    let dialog = Dialog::new(
        format!("Instrument {:1X} effects", instrument),
        vec![list.boxed()],
    )
    .hide_behind_child()
    .close_on_esc()
    .focus_first()
    .on_update(move |state, fields| {
        match fields.list_event("effects") {
            Some(ListEvent::Open(effect)) => open_effect_dialog(state, instrument, effect),
            Some(ListEvent::Add) => open_add_effect_dialog(state, instrument),
            Some(ListEvent::Remove(effect)) => state.remove_effect(instrument, effect),
            Some(ListEvent::Move { from, to }) => state.move_effect(instrument, from, to),
            Some(ListEvent::Check { item, checked }) => {
                state.set_effect_enabled(instrument, item, checked)
            }
            None => {}
        }
        return Ok(());
    });
    state.open_dialog(dialog);
}

fn effect_names(state: &AppState, instrument: u8) -> Vec<String> {
    return state.project().instruments[instrument as usize]
        .effects
        .iter()
        .map(|settings| settings.kind.name().to_owned())
        .collect();
}

fn effects_enabled(state: &AppState, instrument: u8) -> Vec<bool> {
    return state.project().instruments[instrument as usize]
        .effects
        .iter()
        .map(|settings| settings.enabled)
        .collect();
}

fn open_add_effect_dialog(state: &mut AppState, instrument: u8) {
    let kinds = EffectKind::iter().map(|kind| kind.name()).collect();
    let dialog = Dialog::new(
        "Add effect",
        vec![Select::new("kind", kinds).label("Effect").boxed()],
    )
    .focus_first()
    .on_submit(move |state, result| {
        if let Some(kind) = result
            .choice("kind")
            .and_then(|i| EffectKind::iter().nth(i))
        {
            state.add_effect(instrument, kind);
        }
        return Ok(());
    });
    state.open_dialog(dialog);
}

/// One field per parameter of an effect. Changes are made right away, so Esc closes it like
/// Enter.
fn open_effect_dialog(state: &mut AppState, instrument: u8, effect: usize) {
    let Some(settings) = state.project().instruments[instrument as usize]
        .effects
        .get(effect)
        .cloned()
    else {
        return;
    };
    let params = effect_params(settings.kind);
    let values: Vec<u8> = params
        .iter()
        .enumerate()
        .map(|(index, param)| settings.values.get(index).copied().unwrap_or(param.default))
        .collect();
    let mut fields: Vec<Box<dyn DialogComponent>> = params
        .iter()
        .zip(&values)
        .map(|(param, value)| param_field(state, param, *value))
        .collect();
    fields.push(Buttons::new(CC_MAPPING_ID, vec!["CC mapping"]).boxed());
    let dialog = Dialog::new(format!("{} {:X}", settings.kind.name(), effect), fields)
        .hide_behind_child()
        .close_on_esc()
        .on_update(move |state, fields| {
            if fields.changed() == CC_MAPPING_ID {
                open_cc_mapping_dialog(state, instrument, effect);
                return Ok(());
            }
            let changed = params
                .iter()
                .position(|param| param.name == fields.changed());
            if let Some(index) = changed
                && let Some(value) = param_value(&params[index], fields)
            {
                state.set_effect_param(instrument, effect, index, value);
            }
            return Ok(());
        });
    state.open_dialog(dialog);
}

/// Which CC each parameter of an effect follows, so pattern commands can change it. Changes are
/// made right away, so Esc closes it like Enter.
fn open_cc_mapping_dialog(state: &mut AppState, instrument: u8, effect: usize) {
    let Some(settings) = state.project().instruments[instrument as usize]
        .effects
        .get(effect)
        .cloned()
    else {
        return;
    };
    let params = effect_params(settings.kind);
    let fields = params
        .iter()
        .enumerate()
        .map(|(index, param)| {
            let selected = settings.param_cc(index).map_or(0, |cc| cc as usize + 1);
            Select::new(param.name, cc_options(state))
                .label(param.name)
                .selected(selected)
                .typed_from(1)
                .boxed() as Box<dyn DialogComponent>
        })
        .collect();
    let title = format!("{} {:X} CC mapping", settings.kind.name(), effect);
    let dialog = Dialog::new(title, fields)
        .close_on_esc()
        .focus_first()
        .on_update(move |state, fields| {
            let changed = params
                .iter()
                .position(|param| param.name == fields.changed());
            if let Some(index) = changed
                && let Some(choice) = fields.choice(params[index].name)
            {
                let cc = choice.checked_sub(1).map(|cc| cc as u8);
                state.set_effect_param_cc(instrument, effect, index, cc);
            }
            return Ok(());
        });
    state.open_dialog(dialog);
}

/// Every CC number with its name, in hex or decimal as the config says.
fn cc_names(state: &AppState) -> Vec<String> {
    return (0..=CC_MAX)
        .map(|cc| {
            let number = match state.config.hex_values {
                true => format!("{cc:02X}"),
                false => format!("{cc:<3}"),
            };
            match cc_name(cc) {
                Some(name) => format!("{number} {}", truncate(&name, CC_NAME_WIDTH)),
                None => number.trim_end().to_owned(),
            }
        })
        .collect();
}

fn cc_options(state: &AppState) -> Vec<String> {
    return std::iter::once("Off".to_owned())
        .chain(cc_names(state))
        .collect();
}

fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_owned();
    }
    let kept: String = text.chars().take(width - 1).collect();
    return format!("{kept}{}", glyphs::pick("\u{2026}", "."));
}

fn param_field(state: &AppState, param: &Param, value: u8) -> Box<dyn DialogComponent> {
    return match param.kind {
        ParamKind::Number { min, max } => {
            let digits = if max > 0xF { 2 } else { 1 };
            let number = NumberBox::new(param.name, param.name, digits)
                .show_range()
                .range(min as i32, max as i32)
                .value(value as i32);
            labelled(number, param.label).boxed()
        }
        ParamKind::SignedNumber { min, max } => {
            let digits = if min.unsigned_abs().max(max.unsigned_abs()) > 0xF {
                2
            } else {
                1
            };
            let number = NumberBox::new(param.name, param.name, digits)
                .show_range()
                .range(min as i32, max as i32)
                .value(value as i8 as i32);
            labelled(number, param.label).boxed()
        }
        ParamKind::Toggle => Checkbox::new(param.name, param.name)
            .checked(value != 0)
            .boxed(),
        ParamKind::Choice(names) => Select::new(param.name, names.to_vec())
            .label(param.name)
            .selected(value as usize)
            .boxed(),
        ParamKind::Cc => Select::new(param.name, cc_names(state))
            .label(param.name)
            .selected(value as usize)
            .boxed(),
        ParamKind::Pitch => {
            let pitches = (0..=PITCH_MAX).map(|pitch| NotePitch(pitch).to_string());
            Select::new(param.name, pitches.collect())
                .label(param.name)
                .selected(value as usize)
                .page_step(SEMITONES_PER_OCTAVE as usize)
                .boxed()
        }
    };
}

fn labelled(number: NumberBox, label: ValueLabel) -> NumberBox {
    if label == ValueLabel::None {
        return number;
    }
    return number.value_label(move |value| label.text(value));
}

fn param_value(param: &Param, result: &DialogResult) -> Option<u8> {
    return match param.kind {
        ParamKind::Number { .. } => result.number(param.name).map(|n| n as u8),
        ParamKind::SignedNumber { .. } => result.number(param.name).map(|n| n as i8 as u8),
        ParamKind::Toggle => result.bool(param.name).map(|on| on as u8),
        ParamKind::Choice(_) | ParamKind::Pitch | ParamKind::Cc => {
            result.choice(param.name).map(|i| i as u8)
        }
    };
}

pub fn open_metronome_dialog(state: &mut AppState) {
    let metronome = state.project().metronome;
    let modes: Vec<&str> = MetronomeMode::iter().map(|mode| mode.name()).collect();
    let mode = MetronomeMode::iter()
        .position(|mode| mode == metronome.mode)
        .unwrap_or(0);
    let channels = (0..instrument_names(state).len() as u8)
        .map(|channel| metronome.sends_to(channel))
        .collect();
    let velocity = |id, label, value: u8| {
        return NumberBox::new(id, label, 2)
            .show_range()
            .range(0, VELOCITY_MAX as i32)
            .value(value as i32)
            .boxed();
    };
    let dialog = Dialog::new(
        "Metronome",
        vec![
            Select::new("mode", modes)
                .label("Play")
                .selected(mode)
                .boxed(),
            Select::new("channels", instrument_names(state))
                .label("Channels")
                .checkboxes(channels)
                .boxed(),
            Select::new("accent_note", pitch_names())
                .label("Accent note")
                .selected(pitch_index(metronome.accent_note))
                .page_step(SEMITONES_PER_OCTAVE as usize)
                .boxed(),
            velocity(
                "accent_velocity",
                "Accent velocity",
                metronome.accent_velocity,
            ),
            Select::new("beat_note", pitch_names())
                .label("Beat note")
                .selected(pitch_index(metronome.beat_note))
                .page_step(SEMITONES_PER_OCTAVE as usize)
                .boxed(),
            velocity("beat_velocity", "Beat velocity", metronome.beat_velocity),
            NumberBox::new("beats_per_bar", "Beats per bar", 2)
                .show_range()
                .range(1, 0x10)
                .value(metronome.beats_per_bar as i32)
                .boxed(),
        ],
    )
    .width(60)
    .close_on_esc()
    .on_update(|state, fields| {
        apply_metronome(state, fields);
        return Ok(());
    })
    .on_submit(|state, result| {
        apply_metronome(state, &result);
        return Ok(());
    });
    state.open_dialog(dialog);
}

fn pitch_names() -> Vec<String> {
    return (NotePitch::MIN..=NotePitch::MAX)
        .map(|pitch| NotePitch(pitch).to_string())
        .collect();
}

fn pitch_index(pitch: u8) -> usize {
    return pitch.clamp(NotePitch::MIN, NotePitch::MAX) as usize - NotePitch::MIN as usize;
}

fn apply_metronome(state: &mut AppState, result: &DialogResult) {
    let mut metronome = state.project().metronome;
    if let Some(mode) = result
        .choice("mode")
        .and_then(|i| MetronomeMode::iter().nth(i))
    {
        metronome.mode = mode;
    }
    if let Some(checks) = result.checks("channels") {
        for (channel, on) in checks.into_iter().enumerate() {
            metronome.set_channel(channel as u8, on);
        }
    }
    let pitch = |id| result.choice(id).map(|i| NotePitch::MIN + i as u8);
    let number = |id| result.number(id).map(|n| n as u8);
    metronome.accent_note = pitch("accent_note").unwrap_or(metronome.accent_note);
    metronome.beat_note = pitch("beat_note").unwrap_or(metronome.beat_note);
    metronome.accent_velocity = number("accent_velocity").unwrap_or(metronome.accent_velocity);
    metronome.beat_velocity = number("beat_velocity").unwrap_or(metronome.beat_velocity);
    metronome.beats_per_bar = number("beats_per_bar").unwrap_or(metronome.beats_per_bar);
    if metronome != state.project().metronome {
        state.set_metronome(metronome);
    }
}

fn apply_globals(state: &mut AppState, result: &DialogResult) {
    if let Some(velocity) = result.number("velocity") {
        state.set_global_velocity(velocity as u8);
    }
    if let Some(transpose) = result.number("transpose") {
        state.set_transpose(transpose as i8);
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

    use super::*;
    use crate::{
        data::project::Project,
        tui::{
            framework::{dialog::DialogValue, widget::MtrakWidget},
            theme::Theme,
        },
    };

    /// The values `param_value` reads from a field made by `param_field` as `keys` are pressed.
    fn edit(param: Param, value: u8, keys: &[KeyCode]) -> Vec<u8> {
        let mut state = AppState::new(Project::empty(), Theme::monochrome()).unwrap();
        let seen = Rc::new(RefCell::new(Vec::new()));
        let field = param_field(&state, &param, value);
        let mut dialog = Dialog::new("", vec![field]).focus_first().on_update({
            let seen = seen.clone();
            move |_, fields| {
                seen.borrow_mut().extend(param_value(&param, fields));
                return Ok(());
            }
        });
        for key in keys {
            let event = Event::Key(KeyEvent::new(*key, KeyModifiers::NONE));
            dialog.handle_event(&mut state, &event);
        }
        return seen.take();
    }

    #[test]
    fn signed_params_are_edited_as_signed_numbers() {
        let param = Param {
            name: "Step",
            kind: ParamKind::SignedNumber { min: -12, max: 12 },
            default: 0,
            label: ValueLabel::None,
        };
        let keys = [KeyCode::Down, KeyCode::Up, KeyCode::Up];
        assert_eq!(edit(param, -1i8 as u8, &keys), vec![0xFE, 0xFF, 0x00]);
        assert!(edit(param, -12i8 as u8, &[KeyCode::Down]).is_empty());
    }

    #[test]
    fn number_params_stay_in_their_range() {
        let param = Param {
            name: "Rate",
            kind: ParamKind::Number { min: 1, max: 0xFF },
            default: 6,
            label: ValueLabel::None,
        };
        let keys = [KeyCode::Down, KeyCode::Down];
        assert_eq!(edit(param, 2, &keys), vec![1]);
        assert!(edit(param, 0xFF, &[KeyCode::Up]).is_empty());
    }

    #[test]
    fn pitch_params_are_picked_by_note() {
        let param = Param {
            name: "Pitch",
            kind: ParamKind::Pitch,
            default: 60,
            label: ValueLabel::None,
        };
        let state = AppState::new(Project::empty(), Theme::monochrome()).unwrap();
        let field = param_field(&state, &param, 60);
        let Some((_, DialogValue::Choice(choice))) = field.value() else {
            panic!("a pitch is a choice");
        };
        assert_eq!(choice, 60);
    }

    #[test]
    fn every_effect_param_has_a_field_that_reads_back_its_default() {
        let state = AppState::new(Project::empty(), Theme::monochrome()).unwrap();
        for kind in EffectKind::iter() {
            for param in effect_params(kind) {
                let field = param_field(&state, param, param.default);
                let (id, value) = field.value().unwrap();
                assert_eq!(id, param.name);
                let read = match (param.kind, value) {
                    (ParamKind::SignedNumber { .. }, DialogValue::Number(n)) => n as i8 as u8,
                    (_, DialogValue::Number(n)) => n as u8,
                    (_, DialogValue::Choice(i)) => i as u8,
                    (_, DialogValue::Bool(b)) => b as u8,
                    (_, value) => panic!("{} has an unexpected field: {value:?}", param.name),
                };
                assert_eq!(read, param.default, "{} {}", kind.name(), param.name);
            }
        }
    }

    #[test]
    fn cc_options_name_each_cc_and_cut_long_names() {
        let mut state = AppState::new(Project::empty(), Theme::monochrome()).unwrap();
        let options = cc_options(&state);
        assert_eq!(options.len(), CC_MAX as usize + 2);
        assert_eq!(options[0], "Off");
        assert_eq!(options[2], "01 Mod wheel");
        assert_eq!(options[4], "03");
        assert_eq!(
            options[0x55],
            format!("54 Portamento cont{}", glyphs::pick("\u{2026}", "."))
        );
        state.config.hex_values = false;
        let options = cc_options(&state);
        assert_eq!(options[2], "1   Mod wheel");
        assert_eq!(options[75], "74  Cutoff");
        assert_eq!(options[4], "3");
    }

    /// How a field draws with values in hex or decimal and ranges on or off.
    fn rendered(field: &mut Box<dyn DialogComponent>, hex: bool, ranges: bool) -> String {
        let mut state = AppState::new(Project::empty(), Theme::monochrome()).unwrap();
        state.config.hex_values = hex;
        state.config.show_value_ranges = ranges;
        let area = ratatui::layout::Rect::new(0, 0, 60, 1);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        field.render(&state, area, &mut buf);
        let line: String = (0..area.width).map(|x| buf[(x, 0)].symbol()).collect();
        return line.trim_end().to_owned();
    }

    #[test]
    fn labelled_params_follow_the_hex_and_range_settings() {
        let state = AppState::new(Project::empty(), Theme::monochrome()).unwrap();
        let feedback = effect_params(EffectKind::Echo)[1];
        let mut field = param_field(&state, &feedback, 0x80);
        let all = [
            (true, true, "80", "(00-FF)"),
            (true, false, "80", ""),
            (false, true, "128", "(0-255)"),
            (false, false, "128", ""),
        ];
        for (hex, ranges, value, range) in all {
            let line = rendered(&mut field, hex, ranges);
            assert!(line.contains(value) && line.ends_with("50%"), "{line:?}");
            assert_eq!(
                line.contains(range) && !range.is_empty(),
                ranges,
                "{line:?}"
            );
        }
        let step = effect_params(EffectKind::Arp)[0];
        let mut field = param_field(&state, &step, -12i8 as u8);
        for (hex, ranges, value) in [(true, true, "-0C"), (false, false, "-012")] {
            let line = rendered(&mut field, hex, ranges);
            assert!(line.contains(value) && line.ends_with("-oct"), "{line:?}");
        }
    }

    #[test]
    fn cc_params_are_dropdowns_of_named_ccs() {
        let param = Param {
            name: "CC",
            kind: ParamKind::Cc,
            default: 1,
            label: ValueLabel::None,
        };
        assert_eq!(
            edit(param, 0x7E, &[KeyCode::Down, KeyCode::Down]),
            vec![0x7F]
        );
        let state = AppState::new(Project::empty(), Theme::monochrome()).unwrap();
        let mut field = param_field(&state, &effect_params(EffectKind::Lfo)[5], 74);
        assert!(rendered(&mut field, true, true).contains("4A Cutoff"));
    }

    #[test]
    fn cc_mapping_sets_and_clears_a_params_cc() {
        let mut state = AppState::new(Project::empty(), Theme::monochrome()).unwrap();
        let instrument = state.active_instrument;
        state.add_effect(instrument, EffectKind::Echo);
        open_cc_mapping_dialog(&mut state, instrument, 0);
        let mut dialog = state.take_pending_dialogs().remove(0);
        let press = |code| Event::Key(KeyEvent::new(code, KeyModifiers::NONE));
        let mut type_keys = |dialog: &mut Dialog, codes: &[KeyCode]| {
            for code in codes {
                dialog.handle_event(&mut state, &press(*code));
            }
            return state.project().instruments[instrument as usize].effects[0].param_cc(0);
        };
        assert_eq!(type_keys(&mut dialog, &[KeyCode::Down]), Some(0));
        assert_eq!(type_keys(&mut dialog, &[KeyCode::Down]), Some(1));
        assert_eq!(type_keys(&mut dialog, &[KeyCode::Up, KeyCode::Up]), None);
    }

    #[test]
    fn remap_asks_for_the_instruments_picked() {
        let mut state = AppState::new(Project::empty(), Theme::monochrome()).unwrap();
        state.active_instrument = 1;
        open_remap_dialog(&mut state);
        let mut chooser = state.take_pending_dialogs().remove(0);
        let press = |code| Event::Key(KeyEvent::new(code, KeyModifiers::NONE));
        chooser.handle_event(&mut state, &press(KeyCode::Enter));
        let mut dialog = state.take_pending_dialogs().remove(0);
        let keys = [
            KeyCode::Down,
            KeyCode::Down,
            KeyCode::Down,
            KeyCode::Tab,
            KeyCode::Tab,
            KeyCode::Down,
            KeyCode::Tab,
            KeyCode::Char(' '),
            KeyCode::Enter,
        ];
        for code in keys {
            dialog.handle_event(&mut state, &press(code));
        }
        let remap = Remap::Instrument { from: 1, to: 2 };
        assert_eq!(
            state.take_remap_request(),
            Some((RemapScope::Song, remap, true))
        );
    }

    #[test]
    fn remap_asks_for_the_pitches_picked() {
        let mut state = AppState::new(Project::empty(), Theme::monochrome()).unwrap();
        open_remap_dialog(&mut state);
        let mut chooser = state.take_pending_dialogs().remove(0);
        let press = |code| Event::Key(KeyEvent::new(code, KeyModifiers::NONE));
        chooser.handle_event(&mut state, &press(KeyCode::Down));
        chooser.handle_event(&mut state, &press(KeyCode::Enter));
        let mut dialog = state.take_pending_dialogs().remove(0);
        for code in [KeyCode::Tab, KeyCode::Tab, KeyCode::Up, KeyCode::Enter] {
            dialog.handle_event(&mut state, &press(code));
        }
        let remap = Remap::Pitch {
            from: NotePitch(MIDDLE_C),
            to: NotePitch(MIDDLE_C - 1),
            instrument: None,
        };
        assert_eq!(
            state.take_remap_request(),
            Some((RemapScope::Block, remap, false))
        );
    }

    #[test]
    fn remapping_an_instrument_to_itself_does_nothing() {
        let mut state = AppState::new(Project::empty(), Theme::monochrome()).unwrap();
        open_remap_instruments_dialog(&mut state);
        let mut dialog = state.take_pending_dialogs().remove(0);
        let enter = Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        dialog.handle_event(&mut state, &enter);
        assert_eq!(state.take_remap_request(), None);
    }

    #[test]
    fn the_name_in_info_is_saved_as_it_is_typed() {
        let mut state = AppState::new(Project::empty(), Theme::monochrome()).unwrap();
        state.project_mut().name = Some("Demo".to_owned());
        open_info_dialog(&mut state);
        let mut dialog = state.take_pending_dialogs().remove(0);
        let press = |code| Event::Key(KeyEvent::new(code, KeyModifiers::NONE));
        dialog.handle_event(&mut state, &press(KeyCode::Char('s')));
        assert_eq!(state.project().name.as_deref(), Some("Demos"));
        for _ in 0..5 {
            dialog.handle_event(&mut state, &press(KeyCode::Backspace));
        }
        assert_eq!(state.project().name, None);
        dialog.handle_event(&mut state, &press(KeyCode::Esc));
        assert!(dialog.is_closed());
    }

    #[test]
    fn info_shows_the_projects_numbers() {
        let mut state = AppState::new(Project::empty(), Theme::monochrome()).unwrap();
        state.project_mut().name = Some("Demo".to_owned());
        open_info_dialog(&mut state);
        let mut dialog = state.take_pending_dialogs().remove(0);
        let area = ratatui::layout::Rect::new(0, 0, 80, 20);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        dialog.render(&state, area, &mut buf);
        let text: String = (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
                    + "\n"
            })
            .collect();
        for expected in [
            "Name: Demo",
            "File: Not saved yet",
            "Tempo: 125 BPM, speed 6",
            "Song: 1 position, 64 rows, 00:00:07",
            "Instruments: 0 of 16 used",
            "Patterns: 1 in the song, 1 empty",
        ] {
            assert!(text.contains(expected), "{expected}\n{text}");
        }
        assert_eq!(file_size(512), "512 bytes");
        assert_eq!(file_size(2048), "2.0 KB (2048 bytes)");
    }

    #[test]
    fn dialogs_with_live_changes_close_with_esc_or_enter() {
        let mut state = AppState::new(Project::empty(), Theme::monochrome()).unwrap();
        for open in [open_instrument_dialog, open_global_dialog] {
            open(&mut state);
        }
        let instrument = state.active_instrument;
        open_effects_dialog(&mut state);
        state.add_effect(instrument, EffectKind::Arp);
        open_effect_dialog(&mut state, instrument, 0);
        open_cc_mapping_dialog(&mut state, instrument, 0);
        let press = |code| Event::Key(KeyEvent::new(code, KeyModifiers::NONE));
        let dialogs = state.take_pending_dialogs();
        assert_eq!(dialogs.len(), 5);
        for (code, mut dialog) in [KeyCode::Esc, KeyCode::Enter]
            .into_iter()
            .cycle()
            .zip(dialogs)
        {
            dialog.handle_event(&mut state, &press(code));
            assert!(dialog.is_closed(), "{code:?}");
        }
    }
}
