use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::Line,
    widgets::{Block, Padding, Paragraph, Widget},
};

use crate::{
    data::midi::{MidiEvent, midi_to_string},
    tui::{
        app::AppState,
        framework::widget::{Collapsible, MtrakWidget},
    },
};

pub struct MidiEventList {
    pending_lines: Vec<String>,
    max_lines_to_show: u16,
}

impl MidiEventList {
    pub fn new() -> Self {
        return Self {
            pending_lines: vec![],
            max_lines_to_show: u16::MAX,
        };
    }

    fn create_lines_from_event(event: &MidiEvent) -> Vec<String> {
        return match event {
            MidiEvent::Aggregate(midi_events) => midi_events
                .iter()
                .flat_map(Self::create_lines_from_event)
                .collect(),
            _ => vec![midi_to_string(event)],
        };
    }
}

impl MtrakWidget for MidiEventList {
    fn update(&mut self, _state: &mut AppState) {
        let mut pending = _state
            .event_history
            .drain(..)
            .flat_map(|e| Self::create_lines_from_event(&e))
            .collect::<Vec<String>>();

        if !pending.is_empty() {
            self.pending_lines.append(&mut pending);
        }
        let overflow = self
            .pending_lines
            .len()
            .saturating_sub(self.max_lines_to_show as usize) as u16;
        if self.pending_lines.len() > self.max_lines_to_show as usize {
            self.pending_lines.drain(0..overflow as usize);
        }
    }
    fn render(&mut self, state: &AppState, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        self.render_full(state, area, buf);
    }
}

impl Collapsible for MidiEventList {
    fn render_full(&mut self, state: &AppState, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        Block::new()
            .title_bottom("MIDI")
            .title_style(state.theme.label)
            .render(area, buf);
        let area = Block::new().padding(Padding::bottom(1)).inner(area);
        self.max_lines_to_show = area.height;
        let height = self.pending_lines.len().min(area.height as usize);
        let layout =
            Layout::vertical([Constraint::Min(0), Constraint::Length(height as u16)]).split(area);
        let overflow = (self
            .pending_lines
            .len()
            .saturating_sub(area.height as usize)) as u16;
        Paragraph::new(
            self.pending_lines
                .iter()
                .map(|l| Line::raw(l.as_str()))
                .collect::<Vec<Line>>(),
        )
        .scroll((overflow, 0))
        .render(layout[1], buf);
    }
}
