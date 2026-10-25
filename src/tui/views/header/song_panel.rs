use ratatui::{
    crossterm::event::Event::{self},
    layout::{Constraint, Layout, Rect},
    widgets::{Block, Borders, Widget},
};

use crate::tui::{
    app::AppState,
    framework::widget::{Collapsible, MtrakWidget},
    views::header::{pattern_info::PatternInfo, sequence::SequenceList},
};

pub struct SongPanel {
    pub sequence_list: SequenceList,
    pub pattern_info: PatternInfo,
}

impl SongPanel {
    pub fn new() -> Self {
        return Self {
            sequence_list: SequenceList::new(),
            pattern_info: PatternInfo::new(),
        };
    }
}

impl MtrakWidget for SongPanel {
    fn handle_event(&mut self, state: &mut AppState, event: &Event) -> bool {
        if self.sequence_list.handle_event(state, event) {
            return true;
        }
        if self.pattern_info.handle_event(state, event) {
            return true;
        }
        return false;
    }
    fn render(&mut self, state: &AppState, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        self.render_full(state, area, buf);
    }
}

impl Collapsible for SongPanel {
    fn render_full(
        &mut self,
        state: &AppState,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
    ) {
        let block = Block::bordered().borders(Borders::LEFT | Borders::RIGHT);
        let sequence_block = Block::bordered()
            .title_top("Sequence")
            .title_style(state.theme.label)
            .borders(Borders::RIGHT);
        let sequence_width = self.sequence_list.width(state) + 1;
        let layout = Layout::horizontal([Constraint::Length(sequence_width), Constraint::Fill(1)])
            .split(block.inner(area));
        block.render(area, buf);
        self.sequence_list
            .render_full(state, sequence_block.inner(layout[0]), buf);
        sequence_block.render(layout[0], buf);
        self.pattern_info.render_full(state, layout[1], buf);
    }
    fn render_collapsed(
        &mut self,
        state: &AppState,
        area: Rect,
        buf: &mut ratatui::prelude::Buffer,
    ) {
        let block = Block::bordered().borders(Borders::LEFT | Borders::RIGHT);
        let sequence_block = Block::bordered().borders(Borders::RIGHT);
        let pattern_width = self.pattern_info.collapsed_width(state);
        let layout = Layout::horizontal([Constraint::Fill(1), Constraint::Length(pattern_width)])
            .split(block.inner(area));
        block.render(area, buf);
        self.sequence_list
            .render_collapsed(state, sequence_block.inner(layout[0]), buf);
        sequence_block.render(layout[0], buf);
        self.pattern_info.render_collapsed(state, layout[1], buf);
    }
}
