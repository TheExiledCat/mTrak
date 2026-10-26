mod channel_grid;
mod config_dialog;
mod dialogs;
mod instruments;
mod midi_events;
mod pattern_info;
mod sequence;
mod song_panel;
mod util;

use ratatui::{
    crossterm::event::Event::{self},
    layout::{Constraint, Layout, Rect},
    text::Line,
    widgets::{Block, Borders, Widget},
};

use crate::{
    tui::{
        app::AppState,
        framework::{
            keymap::{ALT_SYMBOL, ASCII_ALT_SYMBOL, HelpSection, KeyActions, KeyHint},
            widget::{Collapsible, MtrakWidget},
        },
        glyphs,
        views::header::{
            channel_grid::ChannelGrid, instruments::InstrumentList, midi_events::MidiEventList,
            song_panel::SongPanel, util::UtilList,
        },
    },
    util::time::duration_to_hh_mm_ss_string,
};

/// The sequence keybinds in two columns, plus the bottom border.
pub const COLLAPSED_HEIGHT: u16 = 4;

pub struct Header {
    song_panel: SongPanel,
    channel_grid: ChannelGrid,
    midi_events: MidiEventList,
    util: UtilList,
    instruments: InstrumentList,
}

impl Default for Header {
    fn default() -> Self {
        return Self::new();
    }
}

impl Header {
    pub fn new() -> Self {
        return Header {
            song_panel: SongPanel::new(),
            channel_grid: ChannelGrid::new(),
            midi_events: MidiEventList::new(),
            util: UtilList::new(),
            instruments: InstrumentList::new(),
        };
    }
}

impl MtrakWidget for Header {
    fn handle_event(&mut self, state: &mut AppState, event: &Event) -> bool {
        return self.song_panel.handle_event(state, event)
            || self.channel_grid.handle_event(state, event)
            || self.instruments.handle_event(state, event)
            || self.util.handle_event(state, event);
    }
    fn update(&mut self, _state: &mut AppState) {
        self.song_panel.update(_state);
        self.channel_grid.update(_state);
        self.instruments.update(_state);
        self.midi_events.update(_state);
        self.util.update(_state);
    }
    fn render(&mut self, state: &AppState, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        if state.header_collapsed {
            self.render_collapsed(state, area, buf);
        } else {
            self.render_full(state, area, buf);
        }
    }
    fn help(&self, sections: &mut Vec<HelpSection>) {
        let alt = glyphs::pick(ALT_SYMBOL, ASCII_ALT_SYMBOL);
        sections.push(self.song_panel.sequence_list.help_section("Sequence"));
        sections.push(self.song_panel.pattern_info.help_section("Pattern"));
        sections.push(
            HelpSection::new("Channels")
                .entry(format!("{alt}1-8"), "Mute or unmute a channel")
                .entry(format!("{alt}9"), "Switch between channels 1-8 and 9-16"),
        );
        sections.push(self.instruments.help_section("Instruments"));
        sections.push(self.util.help_section("Util"));
    }
    fn key_hints(&self, state: &AppState, hints: &mut Vec<KeyHint>) {
        self.channel_grid.key_hints(state, hints);
        self.instruments.key_hints(state, hints);
    }
}

impl Collapsible for Header {
    fn render_full(
        &mut self,
        state: &AppState,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
    ) {
        let border = Block::new()
            .borders(Borders::BOTTOM)
            .border_style(state.theme.text);
        border.render(area, buf);
        let layout = Layout::horizontal([
            Constraint::Percentage(50),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
            Constraint::Percentage(20),
        ])
        .split(Block::new().inner(area));

        let left_layout = Layout::vertical([Constraint::Max(8), Constraint::Fill(1)])
            .split(*layout.first().unwrap());
        let util_block = Block::bordered()
            .borders(Borders::BOTTOM | Borders::RIGHT)
            .title_bottom("Util")
            .title_style(state.theme.label);
        let util_inner = util_block.inner(layout[1]);
        util_block.render(layout[1], buf);
        self.song_panel.render_full(state, left_layout[0], buf);
        self.channel_grid.render_full(state, left_layout[1], buf);

        self.util.render_full(state, util_inner, buf);
        self.midi_events
            .render_full(state, layout[layout.len() - 2], buf);
        self.instruments
            .render_full(state, *layout.last().unwrap(), buf)
    }
    fn render_collapsed(
        &mut self,
        state: &AppState,
        area: Rect,
        buf: &mut ratatui::prelude::Buffer,
    ) {
        let label_style = state.theme.label;
        let label = |text: String| Line::styled(format!(" {text} "), label_style);
        let mut border = Block::new()
            .borders(Borders::BOTTOM)
            .border_style(state.theme.text)
            .title_bottom(label(self.util.extend_hint()).left_aligned())
            .title_bottom(label(octave_and_globals_text(state)).left_aligned());
        if let Some(notification) = &state.notification {
            border = border.title_bottom(label(notification.clone()).right_aligned());
        }
        let border =
            border.title_bottom(label(duration_to_hh_mm_ss_string(&state.time())).right_aligned());
        let [song_panel_area, instruments_area] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Percentage(30)])
                .areas(border.inner(area));
        border.render(area, buf);
        self.song_panel
            .render_collapsed(state, song_panel_area, buf);
        self.instruments
            .render_collapsed(state, instruments_area, buf);
    }
}

pub fn octave_and_globals_text(state: &AppState) -> String {
    return format!(
        "OCT: {}, GLB VEL: {:02X}, GLB TRSP: {}{:02X}",
        state.current_octave,
        state.project().global_velocity,
        if state.project().transpose < 0 {
            "-"
        } else {
            "+"
        },
        state.project().transpose.unsigned_abs()
    );
}
