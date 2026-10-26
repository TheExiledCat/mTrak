use std::{cell::RefCell, rc::Rc};

use ratatui::style::Color;

use crate::{
    data::config::{self, CUSTOM_THEME, Config, LineNumbers, ThemeSetting},
    error::{MTrakError, Result},
    tui::{
        app::AppState,
        files,
        framework::{
            core_extensions::Boxed,
            dialog::{
                Buttons, Checkbox, ColorPicker, Dialog, DialogResult, DialogValue, Indent, Label,
                Select, TextPrompt,
            },
        },
        theme::{Palette, Theme},
    },
};

const RESET_COLOR: usize = 0;
const RESET_ALL: usize = 1;
const NO_PORT: &str = "(none)";

/// Settings that aren't part of the project, written to config.toml straight away unless
/// they're only meant for this session. Every change previews straight away, and cancelling
/// puts back what was there.
///
/// The colors of the chosen theme can be edited, previewing as they change. Confirming writes
/// them to the themes folder under the name given, else back to the theme's own file. Editing
/// a built-in theme fills in "custom" as the name, since it has no file of its own.
pub fn open_config_dialog(state: &mut AppState) {
    let mut themes = config::builtin_themes();
    themes.extend(files::list_themes().into_iter().map(ThemeSetting::Named));
    let names: Vec<String> = themes.iter().map(|theme| theme.to_string()).collect();
    let current = themes
        .iter()
        .position(|theme| *theme == state.config.theme)
        .unwrap_or(0);
    let edit = PaletteEdit::new(current, &themes[current]);
    let mut palette = edit.palette.clone();
    let color_names: Vec<&str> = palette.entries().iter().map(|(key, _, _)| *key).collect();
    let first_color = *palette.entries()[0].2;
    let (in_names, in_ports, in_selected) =
        port_options(state.get_midi_in_port_names(), &state.config.midi_input);
    let (out_names, out_ports, out_selected) =
        port_options(state.get_midi_out_port_names(), &state.config.midi_output);

    let line_numbers = LineNumbers::ALL;
    let line_number_names: Vec<String> = line_numbers.iter().map(|l| l.name()).collect();
    let current_line_numbers = line_numbers
        .iter()
        .position(|l| *l == state.config.line_numbers)
        .unwrap_or(0);
    let ports = Rc::new(Ports {
        input: in_ports,
        output: out_ports,
    });

    let edit = Rc::new(RefCell::new(edit));
    let themes = Rc::new(themes);
    let original = Original {
        theme: state.theme.clone(),
        config: state.config.clone(),
    };
    let dialog = Dialog::new(
        "Config",
        vec![
            Select::new("theme", names)
                .label("Theme")
                .selected(current)
                .boxed(),
            Select::new("color", color_names)
                .label("Color")
                .max_visible(11)
                .boxed(),
            ColorPicker::new("rgb").color(first_color).indent(1).boxed(),
            Buttons::new("reset", vec!["Reset color", "Reset all"])
                .indent(1)
                .boxed(),
            TextPrompt::new("name", "Save theme as")
                .allowed(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                .boxed(),
            Label::new("").boxed(),
            Select::new("midi_in", in_names)
                .label("Default MIDI in")
                .selected(in_selected)
                .boxed(),
            Select::new("midi_out", out_names)
                .label("Default MIDI out")
                .selected(out_selected)
                .boxed(),
            Checkbox::new("ascii", "ASCII mode")
                .checked(state.config.ascii_mode)
                .boxed(),
            Checkbox::new("update", "Check for updates")
                .checked(state.config.update_check)
                .boxed(),
            Label::new("Pattern editor").boxed(),
            Select::new("line_numbers", line_number_names)
                .label("Line numbers")
                .selected(current_line_numbers)
                .indent(1)
                .boxed(),
            Checkbox::new("hex", "Hex line numbers")
                .checked(state.config.hex_line_numbers)
                .indent(1)
                .boxed(),
            Checkbox::new("dashes", "Show dashes")
                .checked(state.config.show_dashes)
                .indent(1)
                .boxed(),
            Checkbox::new("track_numbers", "Track numbers")
                .checked(state.config.track_numbers)
                .indent(1)
                .boxed(),
            Label::new("").boxed(),
            Buttons::new("open_folder", vec!["Open config folder"]).boxed(),
            Checkbox::new("save", "Save to config.toml")
                .checked(true)
                .boxed(),
        ],
    )
    .width(64)
    .on_update({
        let themes = themes.clone();
        let edit = edit.clone();
        let original = original.clone();
        let ports = ports.clone();
        move |state, fields| {
            if fields.changed() == "open_folder" {
                let folder = files::get_config_directory_root()
                    .ok_or(MTrakError::MissingDirectory("config"))?;
                return files::open_in_file_manager(&folder);
            }
            let config = settings(fields, state.config.theme.clone(), &ports);
            state.apply_config(config);
            let mut edit = edit.borrow_mut();
            let color = fields.choice("color").unwrap_or(0);
            match (fields.changed(), fields.pressed("reset")) {
                ("theme", _) => {
                    let index = fields.choice("theme").unwrap_or(0);
                    *edit = PaletteEdit::new(index, &themes[index]);
                }
                ("color", _) => {}
                ("rgb", _) => {
                    if let Some(rgb) = fields.color("rgb") {
                        *edit.palette.entries()[color].2 = rgb;
                    }
                }
                ("reset", Some(RESET_COLOR)) => {
                    let base = *edit.base.entries()[color].2;
                    *edit.palette.entries()[color].2 = base;
                }
                ("reset", Some(RESET_ALL)) => edit.palette = edit.base.clone(),
                _ => return Ok(()),
            }
            fields.set("rgb", DialogValue::Color(edit.color(color)));
            let is_builtin = !matches!(themes[edit.source], ThemeSetting::Named(_));
            let name = fields.text("name").unwrap_or_default().to_owned();
            if is_builtin && edit.edited() && name.is_empty() {
                fields.set("name", DialogValue::Text(CUSTOM_THEME.to_owned()));
            } else if !edit.edited() && name == CUSTOM_THEME {
                fields.set("name", DialogValue::Text(String::new()));
            }
            state.theme = if edit.edited() {
                Theme::from_palette(&edit.palette)
            } else {
                Theme::from_setting(&themes[edit.source]).unwrap_or_else(|_| original.theme.clone())
            };
            return Ok(());
        }
    })
    .on_submit({
        let original = original.clone();
        move |state, result| {
            let saved = save_settings(state, &result, &themes, &edit.borrow(), &ports);
            if saved.is_err() {
                original.restore(state);
            }
            return saved;
        }
    })
    .on_cancel(move |state| original.restore(state));
    state.open_dialog(dialog);
}

/// Saves the edited theme if there is one, then switches to the settings in the dialog and
/// writes them to config.toml if that's ticked.
fn save_settings(
    state: &mut AppState,
    result: &DialogResult,
    themes: &[ThemeSetting],
    edit: &PaletteEdit,
    ports: &Ports,
) -> Result<()> {
    let mut theme = themes[result.choice("theme").unwrap_or(0)].clone();
    let name = result.text("name").unwrap_or_default().trim();
    if !name.is_empty() {
        if !config::is_theme_name(name) {
            return Err(MTrakError::InvalidConfig(format!(
                "{name} is the name of a built-in theme, so the theme can't be saved \
                     under it"
            )));
        }
        edit.palette.save(&files::get_named_theme_path(name)?)?;
        theme = ThemeSetting::Named(name.to_owned());
    } else if edit.edited() {
        let name = match &theme {
            ThemeSetting::Named(name) => name.clone(),
            _ => CUSTOM_THEME.to_owned(),
        };
        edit.palette.save(&files::get_named_theme_path(&name)?)?;
        theme = ThemeSetting::Named(name);
    }
    let config = settings(result, theme, ports);
    return apply_config(state, config, result.bool("save").unwrap_or(false));
}

/// The ports the default port lists stand for.
struct Ports {
    input: Vec<Option<String>>,
    output: Vec<Option<String>>,
}

/// What the dialog changes while it's open, to put back when it's cancelled.
#[derive(Clone)]
struct Original {
    theme: Theme,
    config: Config,
}

impl Original {
    fn restore(&self, state: &mut AppState) {
        state.theme = self.theme.clone();
        state.apply_config(self.config.clone());
    }
}

/// The settings in the dialog, with `theme`.
fn settings(result: &DialogResult, theme: ThemeSetting, ports: &Ports) -> Config {
    let defaults = Config::default();
    let line_numbers = result
        .choice("line_numbers")
        .and_then(|i| LineNumbers::ALL.get(i).copied());
    return Config {
        theme,
        ascii_mode: result.bool("ascii").unwrap_or(defaults.ascii_mode),
        update_check: result.bool("update").unwrap_or(defaults.update_check),
        midi_input: result
            .choice("midi_in")
            .and_then(|i| ports.input[i].clone()),
        midi_output: result
            .choice("midi_out")
            .and_then(|i| ports.output[i].clone()),
        hex_line_numbers: result.bool("hex").unwrap_or(defaults.hex_line_numbers),
        show_dashes: result.bool("dashes").unwrap_or(defaults.show_dashes),
        track_numbers: result
            .bool("track_numbers")
            .unwrap_or(defaults.track_numbers),
        line_numbers: line_numbers.unwrap_or(defaults.line_numbers),
    };
}

/// The palette being edited, and the theme it started from.
struct PaletteEdit {
    /// The index of the theme in the theme list.
    source: usize,
    base: Palette,
    palette: Palette,
}

impl PaletteEdit {
    fn new(source: usize, theme: &ThemeSetting) -> Self {
        let base = Palette::for_setting(theme);
        return Self {
            source,
            palette: base.clone(),
            base,
        };
    }

    fn edited(&self) -> bool {
        return self.palette != self.base;
    }

    fn color(&mut self, index: usize) -> Color {
        return *self.palette.entries()[index].2;
    }
}

/// The names to list for picking a default port, with the port each one stands for and the
/// one to start on. A configured port that isn't connected stays in the list, so confirming
/// the dialog doesn't drop it.
fn port_options(
    available: Vec<String>,
    configured: &Option<String>,
) -> (Vec<String>, Vec<Option<String>>, usize) {
    let mut ports: Vec<Option<String>> = vec![None];
    ports.extend(available.iter().cloned().map(Some));
    let mut names = vec![NO_PORT.to_owned()];
    names.extend(available);
    if let Some(name) = configured
        && !ports.contains(configured)
    {
        names.push(format!("{name} (not connected)"));
        ports.push(configured.clone());
    }
    let selected = ports.iter().position(|p| p == configured).unwrap_or(0);
    return (names, ports, selected);
}

/// Switches to `config` for this session, and writes it to config.toml when `save` is set.
/// Nothing changes when its theme can't be built, e.g. a theme file with an error.
fn apply_config(state: &mut AppState, config: Config, save: bool) -> Result<()> {
    state.theme = Theme::from_setting(&config.theme)?;
    if save {
        config.save(&files::get_config_file_path()?)?;
    }
    state.apply_config(config);
    return Ok(());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{config::ThemeChoice, project::Project};

    #[test]
    fn a_session_only_config_switches_the_theme() {
        let mut state = AppState::new(Project::empty(), Theme::monochrome()).unwrap();
        let theme = ThemeSetting::Builtin(ThemeChoice::Ansi16);
        let config = Config {
            theme: theme.clone(),
            ..Config::default()
        };
        apply_config(&mut state, config, false).unwrap();
        assert_eq!(state.config.theme, theme);
        assert_eq!(state.theme.background, Theme::ansi16().background);
    }

    #[test]
    fn a_missing_default_port_stays_listed() {
        let (names, ports, selected) =
            port_options(vec!["Synth".to_owned()], &Some("Keys".to_owned()));
        assert_eq!(names, ["(none)", "Synth", "Keys (not connected)"]);
        assert_eq!(ports[selected].as_deref(), Some("Keys"));
        let (_, ports, selected) = port_options(vec!["Synth".to_owned()], &None);
        assert_eq!(ports[selected], None);
    }
}
