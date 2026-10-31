use ratatui::{
    crossterm::event::Event::{self},
    layout::{Constraint, Layout, Rect},
    text::Line,
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::{
    error::Result,
    tui::{
        app::AppState,
        framework::{
            keymap::{Action, Key, KeyActions, Keymap},
            widget::{Collapsible, MtrakWidget},
        },
        views::header::{
            config_dialog::open_config_dialog,
            dialogs::{
                open_about_dialog, open_clear_dialog, open_global_dialog, open_info_dialog,
                open_instrument_dialog, open_metronome_dialog, open_midi_dialog, open_remap_dialog,
            },
        },
    },
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UtilListAction {
    Instruments,
    Global,
    Metronome,
    Midi,
    Config,
    Clear,
    Remap,
    Info,
    About,
    Extend,
}

impl Action for UtilListAction {}

pub struct UtilList {
    keymap: Keymap<UtilListAction>,
}

impl UtilList {
    pub fn new() -> Self {
        use UtilListAction::*;
        let keymap = Keymap::new("util")
            .bind(Key::ctrl('r'), Instruments, "Instrument")
            .bind(Key::ctrl('e'), Global, "Global")
            .bind(Key::ctrl('g'), Metronome, "Metronome")
            .bind(Key::alt('z'), Clear, "Clear")
            .bind(Key::ctrl('k'), Remap, "Remap")
            .bind(Key::ctrl('o'), Midi, "Midi")
            .bind(Key::ctrl('p'), Config, "Config")
            .bind(Key::ctrl('x'), Extend, "Extend")
            .bind(Key::ctrl('f'), Info, "Info")
            .bind(Key::ctrl('t'), About, "About");
        return Self { keymap };
    }

    pub fn extend_hint(&self) -> String {
        return self.keymap.labelled_key(UtilListAction::Extend);
    }

    fn render_notification(
        &self,
        state: &AppState,
        area: Rect,
        buf: &mut ratatui::prelude::Buffer,
    ) {
        if let Some(notification) = &state.notification {
            Paragraph::new(notification.as_str())
                .centered()
                .block(Block::bordered().borders(Borders::TOP))
                .render(area, buf);
        }
    }
}

impl MtrakWidget for UtilList {
    fn handle_event(&mut self, state: &mut AppState, event: &Event) -> bool {
        return self.dispatch(state, event);
    }
    fn render(&mut self, state: &AppState, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        self.render_full(state, area, buf);
    }
}

impl Collapsible for UtilList {
    fn render_full(&mut self, state: &AppState, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        let bindings = &self.keymap.bindings();
        let layout = Layout::vertical([
            Constraint::Length(bindings.len() as u16),
            Constraint::Fill(1),
            Constraint::Length(2),
        ])
        .split(area);
        let keys: Vec<String> = bindings.iter().map(|b| b.key.to_string()).collect();
        let key_width = keys.iter().map(|k| k.chars().count()).max().unwrap_or(0);
        let lines: Vec<Line> = bindings
            .iter()
            .zip(&keys)
            .map(|(b, key)| {
                Line::raw(format!("{key:<key_width$}: {}", b.label)).style(state.theme.label)
            })
            .collect();
        Paragraph::new(lines)
            .block(Block::new().style(state.theme.label))
            .render(layout[0], buf);
        self.render_notification(state, *layout.last().unwrap(), buf);
    }
}

impl KeyActions for UtilList {
    type Action = UtilListAction;

    fn keymap(&self) -> &Keymap<Self::Action> {
        return &self.keymap;
    }

    fn perform(&mut self, action: Self::Action, state: &mut AppState) -> Result<()> {
        match action {
            UtilListAction::Instruments => open_instrument_dialog(state),
            UtilListAction::Global => open_global_dialog(state),
            UtilListAction::Metronome => open_metronome_dialog(state),
            UtilListAction::Midi => open_midi_dialog(state),
            UtilListAction::Config => open_config_dialog(state),
            UtilListAction::About => open_about_dialog(state),
            UtilListAction::Clear => open_clear_dialog(state),
            UtilListAction::Remap => open_remap_dialog(state),
            UtilListAction::Info => open_info_dialog(state),
            UtilListAction::Extend => state.header_collapsed = !state.header_collapsed,
        }
        return Ok(());
    }

    fn describe(action: UtilListAction) -> &'static str {
        return match action {
            UtilListAction::Instruments => "Edit the current instrument",
            UtilListAction::Global => "Set the global velocity and transpose",
            UtilListAction::Metronome => "Set when and how the metronome clicks",
            UtilListAction::Midi => "Set the MIDI ports and clock",
            UtilListAction::Config => "Edit the settings in config.toml",
            UtilListAction::About => "About mTrak",
            UtilListAction::Clear => "Clear part of the current pattern",
            UtilListAction::Remap => "Move notes to another instrument or pitch",
            UtilListAction::Info => "Show the project's size and how much is in it",
            UtilListAction::Extend => "Collapse or expand the header to show more of the pattern",
        };
    }
}
