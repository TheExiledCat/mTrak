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
        framework::dialog::{
            Buttons, Checkbox, Dialog, DialogFields, DialogResult, Field, InfoLevel, Label, Link,
            List, ListEvent, NumberBox, Select, TextPrompt,
        },
        glyphs,
    },
    util::time::duration_to_hh_mm_ss_string,
};

/// The year mTrak was started, for the copyright line in the About dialog.
const FIRST_YEAR: i32 = 2026;

const REPOSITORY_URL: &str = "https://github.com/TheExiledCat/mTrak";

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
    let mut dialog = Dialog::new("MIDI");
    let input = dialog.add_field(
        Select::new(in_names)
            .label("MIDI input")
            .selected(current_in),
    );
    let clock = dialog.add_field(
        Select::new(clock_names)
            .label("Clock SRC")
            .selected(current_clock),
    );
    let velocity = dialog
        .add_field(Checkbox::new("Record Velocity").checked(state.project().record_input_velocity));
    let soft_thru = dialog.add_field(Checkbox::new("Soft thru").checked(state.config.soft_thru));
    let output = dialog.add_field(
        Select::new(out_names)
            .label("MIDI output")
            .selected(current_out),
    );
    let midi_clock =
        dialog.add_field(Checkbox::new("Send MIDI clock").checked(state.project().send_midi_clock));
    let dialog = dialog.on_submit(move |state, result| {
        state.set_midi_clock(*result.get(midi_clock));
        state.set_midi_velocity_record(*result.get(velocity));
        let soft_thru = state.set_soft_thru(*result.get(soft_thru));
        let output = match *result.get(output) {
            0 => None,
            output => state
                .midi_out_client
                .as_ref()
                .and_then(|client| client.ports().get(output - 1).cloned()),
        };
        state.set_midi_output(output);
        // inputs last, so a port that fails to open doesn't drop the other changes
        let clock = state.set_midi_sync(*result.get(clock));
        let port = state.input_port_for_choice(*result.get(input));
        let input = state.set_midi_input(port);
        clock?;
        soft_thru?;
        return input;
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
    let mut dialog = Dialog::new("Clear");
    let clear = dialog.add_field(Select::new(options).label("Selection"));
    let target = dialog.add_field(Select::new(targets).label("Target"));
    let instrument = dialog.add_field(
        Select::new(instruments)
            .label("Instrument")
            .selected(all_instruments),
    );
    let dialog = dialog.on_submit(move |state, result| {
        use ClearOption::*;
        use ClearTarget::*;
        let option = match *result.get(clear) {
            0 => Block,
            1 => Track,
            2 => Pattern,
            _ => unreachable!(),
        };
        let target = match *result.get(target) {
            0 => All,
            1 => Note,
            2 => Velocity,
            3 => Command,
            _ => unreachable!(),
        };
        let instrument = Some(*result.get(instrument))
            .filter(|&choice| choice != all_instruments)
            .map(|i| i as u8);
        state.request_clear(option, target, instrument);
        return Ok(());
    });

    state.open_dialog(dialog);
}

pub fn open_remap_dialog(state: &mut AppState) {
    let mut dialog = Dialog::new("Remap");
    let kind = dialog.add_field(Select::new(vec!["Instruments", "Pitches"]).label("Remap"));
    let dialog = dialog.focus_first().on_submit(move |state, result| {
        match *result.get(kind) {
            0 => open_remap_instruments_dialog(state),
            _ => open_remap_pitches_dialog(state),
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
    let name = TextPrompt::new("Name").initial(project.name.as_deref().unwrap_or_default());
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
    let mut dialog = Dialog::new("Info");
    let name = dialog.add_field(name);
    for line in lines {
        dialog = dialog.with(Label::new(line));
    }
    let dialog = dialog
        .close_on_esc()
        .focus_first()
        .on_update(move |state, fields| {
            set_project_name(state, fields.get(name).as_str());
            return Ok(());
        });
    state.open_dialog(dialog);
}

/// Changes are made right away, so Esc closes it like Enter.
pub fn open_instrument_dialog(state: &mut AppState) {
    let instrument = state.active_instrument;
    let settings = &state.project().instruments[instrument as usize];
    let mut dialog = Dialog::new(format!("Instrument {:1X}", instrument));
    let fields = InstrumentFields {
        name: dialog.add_field(TextPrompt::new("Name").initial(&settings.name)),
        volume: dialog.add_field(labelled(
            NumberBox::new("Volume", 2)
                .show_range()
                .value(settings.volume as i32),
            ValueLabel::Percent,
        )),
        bend_range: dialog.add_field(
            NumberBox::new("Bend range (semitones)", 2)
                .show_range()
                .value(settings.pitch_wheel_range as i32),
        ),
        default_cc: dialog.add_field(
            Select::new(cc_names(state))
                .label("Default CC")
                .selected(settings.default_cc as usize),
        ),
        transpose: dialog.add_field(labelled(
            NumberBox::new("Transpose (semitones)", 2)
                .show_range()
                .range(i8::MIN as i32, i8::MAX as i32)
                .value(settings.transpose as i32),
            ValueLabel::Semitones,
        )),
    };
    let effects = dialog.add_field(Buttons::new(vec!["Effects"]));
    let dialog = dialog
        .width(60)
        .hide_behind_child()
        .close_on_esc()
        .focus_first()
        .on_update(move |state, dialog| {
            fields.apply(state, instrument, dialog);
            if dialog.changed(effects) {
                open_effects_dialog(state);
            }
            return Ok(());
        })
        .on_submit(move |state, result| {
            fields.apply(state, instrument, &result);
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
    let mut dialog = Dialog::new("About");
    for line in lines {
        dialog = dialog.with(Label::new(line).centered());
    }
    let dialog = dialog
        .with(Link::new(REPOSITORY_URL))
        .level(InfoLevel::Success)
        .close_on_esc();
    state.open_dialog(dialog);
}

/// Changes are made right away, so Esc closes it like Enter.
pub fn open_global_dialog(state: &mut AppState) {
    let mut dialog = Dialog::new("Global");
    let velocity = dialog.add_field(labelled(
        NumberBox::new("Velocity", 2)
            .show_range()
            .value(state.project().global_velocity as i32),
        ValueLabel::Percent,
    ));
    let transpose = dialog.add_field(labelled(
        NumberBox::new("Transpose (semitones)", 2)
            .show_range()
            .range(i8::MIN as i32, i8::MAX as i32)
            .value(state.project().transpose as i32),
        ValueLabel::Semitones,
    ));
    let apply = move |state: &mut AppState, result: &DialogResult| {
        state.set_global_velocity(*result.get(velocity) as u8);
        state.set_transpose(*result.get(transpose) as i8);
    };
    let dialog = dialog
        .width(60)
        .close_on_esc()
        .focus_first()
        .on_update(move |state, fields| {
            apply(state, fields);
            return Ok(());
        })
        .on_submit(move |state, result| {
            apply(state, &result);
            return Ok(());
        });
    state.open_dialog(dialog);
}

/// Gives the cells that use one instrument another, e.g. after moving a synth to another MIDI
/// channel. Copying the settings gives the new instrument the old one's, and the old one the new
/// one's.
fn open_remap_instruments_dialog(state: &mut AppState) {
    let instrument = state.active_instrument as usize;
    let mut dialog = Dialog::new("Remap instruments");
    let scope = dialog.add_field(Select::new(REMAP_SCOPES.to_vec()).label("Selection"));
    let from = dialog.add_field(
        Select::new(instrument_names(state))
            .label("From")
            .selected(instrument),
    );
    let to = dialog.add_field(
        Select::new(instrument_names(state))
            .label("To")
            .selected(instrument),
    );
    let swap = dialog.add_field(Checkbox::new("Copy their settings"));
    let dialog = dialog.focus_first().on_submit(move |state, result| {
        let from = *result.get(from) as u8;
        let to = *result.get(to) as u8;
        if from != to {
            let remap = Remap::Instrument { from, to };
            state.request_remap(remap_scope(*result.get(scope)), remap, *result.get(swap));
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
    let mut dialog = Dialog::new("Remap pitches");
    let scope = dialog.add_field(Select::new(REMAP_SCOPES.to_vec()).label("Selection"));
    let from = dialog.add_field(
        Select::new(pitches.clone())
            .label("From")
            .selected(middle_c)
            .page_step(SEMITONES_PER_OCTAVE as usize),
    );
    let to = dialog.add_field(
        Select::new(pitches)
            .label("To")
            .selected(middle_c)
            .page_step(SEMITONES_PER_OCTAVE as usize),
    );
    let instrument = dialog.add_field(
        Select::new(instruments)
            .label("Instrument")
            .selected(all_instruments),
    );
    let dialog = dialog.focus_first().on_submit(move |state, result| {
        let pitch = |field| NotePitch(NotePitch::MIN + *result.get(field) as u8);
        let (from, to) = (pitch(from), pitch(to));
        if from != to {
            let instrument = Some(*result.get(instrument))
                .filter(|&choice| choice != all_instruments)
                .map(|i| i as u8);
            let remap = Remap::Pitch {
                from,
                to,
                instrument,
            };
            state.request_remap(remap_scope(*result.get(scope)), remap, false);
        }
        return Ok(());
    });
    state.open_dialog(dialog);
}

fn remap_scope(choice: usize) -> RemapScope {
    return match choice {
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

#[derive(Clone, Copy)]
struct InstrumentFields {
    name: Field<String>,
    volume: Field<i32>,
    bend_range: Field<i32>,
    default_cc: Field<usize>,
    transpose: Field<i32>,
}

impl InstrumentFields {
    fn apply(&self, state: &mut AppState, instrument: u8, result: &DialogResult) {
        state.project_mut().instruments[instrument as usize].name = result.get(self.name).clone();
        state.set_global_volume(instrument, *result.get(self.volume) as u8);
        state.set_bend_range(instrument, *result.get(self.bend_range) as u8);
        state.set_default_cc(instrument, *result.get(self.default_cc) as u8);
        state.set_instrument_transpose(instrument, *result.get(self.transpose) as i8);
    }
}

/// The active instrument's effects, in the order notes go through them. Changes are made
/// right away, so Esc closes it like Enter.
fn open_effects_dialog(state: &mut AppState) {
    let instrument = state.active_instrument;
    let list = List::new(effect_names(state, instrument))
        .items_from(move |state| effect_names(state, instrument))
        .checkboxes(effects_enabled(state, instrument))
        .checks_from(move |state| effects_enabled(state, instrument))
        .show_index(true)
        .add_button("Add effect")
        .removable(true)
        .reorderable(true);
    let mut dialog = Dialog::new(format!("Instrument {:1X} effects", instrument));
    let list = dialog.add_field(list);
    let dialog = dialog
        .hide_behind_child()
        .close_on_esc()
        .focus_first()
        .on_update(move |state, fields| {
            match fields.get(list).event() {
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
    let mut dialog = Dialog::new("Add effect");
    let kind = dialog.add_field(Select::new(kinds).label("Effect"));
    let dialog = dialog.focus_first().on_submit(move |state, result| {
        if let Some(kind) = EffectKind::iter().nth(*result.get(kind)) {
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
    let mut dialog = Dialog::new(format!("{} {:X}", settings.kind.name(), effect));
    let fields: Vec<ParamField> = params
        .iter()
        .zip(&values)
        .map(|(param, value)| param_input(state, param, *value).add_to(&mut dialog))
        .collect();
    let cc_mapping = dialog.add_field(Buttons::new(vec!["CC mapping"]));
    let dialog = dialog
        .hide_behind_child()
        .close_on_esc()
        .on_update(move |state, dialog| {
            if dialog.changed(cc_mapping) {
                open_cc_mapping_dialog(state, instrument, effect);
                return Ok(());
            }
            if let Some(index) = fields.iter().position(|field| field.changed(dialog)) {
                state.set_effect_param(instrument, effect, index, fields[index].read(dialog));
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
    let title = format!("{} {:X} CC mapping", settings.kind.name(), effect);
    let mut dialog = Dialog::new(title);
    let fields: Vec<Field<usize>> = params
        .iter()
        .enumerate()
        .map(|(index, param)| {
            let selected = settings.param_cc(index).map_or(0, |cc| cc as usize + 1);
            dialog.add_field(
                Select::new(cc_options(state))
                    .label(param.name)
                    .selected(selected)
                    .typed_from(1),
            )
        })
        .collect();
    let dialog = dialog
        .close_on_esc()
        .focus_first()
        .on_update(move |state, dialog| {
            if let Some(index) = fields.iter().position(|field| dialog.changed(*field)) {
                let cc = dialog.get(fields[index]).checked_sub(1).map(|cc| cc as u8);
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

enum ParamInput {
    Number(NumberBox),
    Toggle(Checkbox),
    Choice(Select),
}

impl ParamInput {
    fn add_to(self, dialog: &mut Dialog) -> ParamField {
        return match self {
            ParamInput::Number(number) => ParamField::Number(dialog.add_field(number)),
            ParamInput::Toggle(checkbox) => ParamField::Toggle(dialog.add_field(checkbox)),
            ParamInput::Choice(select) => ParamField::Choice(dialog.add_field(select)),
        };
    }
}

#[derive(Clone, Copy)]
enum ParamField {
    /// Signed numbers too; `as u8` wraps them like they're stored.
    Number(Field<i32>),
    Toggle(Field<bool>),
    Choice(Field<usize>),
}

impl ParamField {
    fn read(&self, result: &DialogResult) -> u8 {
        return match *self {
            ParamField::Number(field) => *result.get(field) as u8,
            ParamField::Toggle(field) => *result.get(field) as u8,
            ParamField::Choice(field) => *result.get(field) as u8,
        };
    }

    fn changed(&self, fields: &DialogFields) -> bool {
        return match *self {
            ParamField::Number(field) => fields.changed(field),
            ParamField::Toggle(field) => fields.changed(field),
            ParamField::Choice(field) => fields.changed(field),
        };
    }
}

fn param_input(state: &AppState, param: &Param, value: u8) -> ParamInput {
    return match param.kind {
        ParamKind::Number { min, max } => {
            let digits = if max > 0xF { 2 } else { 1 };
            let number = NumberBox::new(param.name, digits)
                .show_range()
                .range(min as i32, max as i32)
                .value(value as i32);
            ParamInput::Number(labelled(number, param.label))
        }
        ParamKind::SignedNumber { min, max } => {
            let digits = if min.unsigned_abs().max(max.unsigned_abs()) > 0xF {
                2
            } else {
                1
            };
            let number = NumberBox::new(param.name, digits)
                .show_range()
                .range(min as i32, max as i32)
                .value(value as i8 as i32);
            ParamInput::Number(labelled(number, param.label))
        }
        ParamKind::Toggle => ParamInput::Toggle(Checkbox::new(param.name).checked(value != 0)),
        ParamKind::Choice(names) => ParamInput::Choice(
            Select::new(names.to_vec())
                .label(param.name)
                .selected(value as usize),
        ),
        ParamKind::Cc => ParamInput::Choice(
            Select::new(cc_names(state))
                .label(param.name)
                .selected(value as usize),
        ),
        ParamKind::Pitch => {
            let pitches = (0..=PITCH_MAX).map(|pitch| NotePitch(pitch).to_string());
            ParamInput::Choice(
                Select::new(pitches.collect())
                    .label(param.name)
                    .selected(value as usize)
                    .page_step(SEMITONES_PER_OCTAVE as usize),
            )
        }
    };
}

fn labelled(number: NumberBox, label: ValueLabel) -> NumberBox {
    if label == ValueLabel::None {
        return number;
    }
    return number.value_label(move |value| label.text(value));
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
    let velocity = |label, value: u8| {
        return NumberBox::new(label, 2)
            .show_range()
            .range(0, VELOCITY_MAX as i32)
            .value(value as i32);
    };
    let mut dialog = Dialog::new("Metronome");
    let fields = MetronomeFields {
        mode: dialog.add_field(Select::new(modes).label("Play").selected(mode)),
        channels: dialog.add_field(
            Select::new(instrument_names(state))
                .label("Channels")
                .checkboxes(channels),
        ),
        accent_note: dialog.add_field(
            Select::new(pitch_names())
                .label("Accent note")
                .selected(pitch_index(metronome.accent_note))
                .page_step(SEMITONES_PER_OCTAVE as usize),
        ),
        accent_velocity: dialog.add_field(velocity("Accent velocity", metronome.accent_velocity)),
        beat_note: dialog.add_field(
            Select::new(pitch_names())
                .label("Beat note")
                .selected(pitch_index(metronome.beat_note))
                .page_step(SEMITONES_PER_OCTAVE as usize),
        ),
        beat_velocity: dialog.add_field(velocity("Beat velocity", metronome.beat_velocity)),
        beats_per_bar: dialog.add_field(
            NumberBox::new("Beats per bar", 2)
                .show_range()
                .range(1, 0x10)
                .value(metronome.beats_per_bar as i32),
        ),
    };
    let dialog = dialog
        .width(60)
        .close_on_esc()
        .on_update(move |state, dialog| {
            fields.apply(state, dialog);
            return Ok(());
        })
        .on_submit(move |state, result| {
            fields.apply(state, &result);
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

#[derive(Clone, Copy)]
struct MetronomeFields {
    mode: Field<usize>,
    channels: Field<Vec<bool>>,
    accent_note: Field<usize>,
    accent_velocity: Field<i32>,
    beat_note: Field<usize>,
    beat_velocity: Field<i32>,
    beats_per_bar: Field<i32>,
}

impl MetronomeFields {
    fn apply(&self, state: &mut AppState, result: &DialogResult) {
        let mut metronome = state.project().metronome;
        if let Some(mode) = MetronomeMode::iter().nth(*result.get(self.mode)) {
            metronome.mode = mode;
        }
        for (channel, on) in result.get(self.channels).iter().enumerate() {
            metronome.set_channel(channel as u8, *on);
        }
        let pitch = |field| NotePitch::MIN + *result.get(field) as u8;
        let number = |field| *result.get(field) as u8;
        metronome.accent_note = pitch(self.accent_note);
        metronome.beat_note = pitch(self.beat_note);
        metronome.accent_velocity = number(self.accent_velocity);
        metronome.beat_velocity = number(self.beat_velocity);
        metronome.beats_per_bar = number(self.beats_per_bar);
        if metronome != state.project().metronome {
            state.set_metronome(metronome);
        }
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
            framework::{
                dialog::{DialogComponent, DialogInput},
                widget::MtrakWidget,
            },
            theme::Theme,
        },
    };

    /// The values `param_value` reads from a field made by `param_field` as `keys` are pressed.
    fn edit(param: Param, value: u8, keys: &[KeyCode]) -> Vec<u8> {
        let mut state = AppState::new(Project::empty(), Theme::monochrome()).unwrap();
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut dialog = Dialog::new("");
        let field = param_input(&state, &param, value).add_to(&mut dialog);
        let mut dialog = dialog.focus_first().on_update({
            let seen = seen.clone();
            move |_, fields| {
                seen.borrow_mut().push(field.read(fields));
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
        let ParamInput::Choice(select) = param_input(&state, &param, 60) else {
            panic!("a pitch is a choice");
        };
        assert_eq!(select.value(), 60);
    }

    #[test]
    fn every_effect_param_has_a_field_that_reads_back_its_default() {
        let state = AppState::new(Project::empty(), Theme::monochrome()).unwrap();
        for kind in EffectKind::iter() {
            for param in effect_params(kind) {
                let mut dialog = Dialog::new("");
                let field = param_input(&state, param, param.default).add_to(&mut dialog);
                let read = field.read(&dialog.result());
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
    fn rendered(field: &mut ParamInput, hex: bool, ranges: bool) -> String {
        let mut state = AppState::new(Project::empty(), Theme::monochrome()).unwrap();
        state.config.hex_values = hex;
        state.config.show_value_ranges = ranges;
        let area = ratatui::layout::Rect::new(0, 0, 60, 1);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        let component: &mut dyn DialogComponent = match field {
            ParamInput::Number(number) => number,
            ParamInput::Toggle(checkbox) => checkbox,
            ParamInput::Choice(select) => select,
        };
        component.render(&state, area, &mut buf);
        let line: String = (0..area.width).map(|x| buf[(x, 0)].symbol()).collect();
        return line.trim_end().to_owned();
    }

    #[test]
    fn labelled_params_follow_the_hex_and_range_settings() {
        let state = AppState::new(Project::empty(), Theme::monochrome()).unwrap();
        let feedback = effect_params(EffectKind::Echo)[1];
        let mut field = param_input(&state, &feedback, 0x80);
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
        let mut field = param_input(&state, &step, -12i8 as u8);
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
        let mut field = param_input(&state, &effect_params(EffectKind::Lfo)[5], 74);
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
