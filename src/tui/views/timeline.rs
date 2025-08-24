use std::{cell::RefCell, rc::Rc};

use ratatui::{
    layout::{Constraint, Layout},
    style::{Style, Stylize},
    text::Line,
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::tui::app::AppState;

pub struct TimeLineView {
    state: Rc<RefCell<AppState>>,
    channel_caches: Vec<ChannelCache>,
}
impl TimeLineView {
    pub fn new(state: Rc<RefCell<AppState>>) -> Self {
        let selected_pattern_index = state.borrow().selected_pattern_index;
        let row_count = state
            .borrow_mut()
            .project
            .pattern_store()
            .get_pattern_by_id(selected_pattern_index)
            .map(|p| p.row_count)
            .unwrap_or(0);

        let channel_count = state
            .borrow_mut()
            .project
            .pattern_store()
            .get_pattern_by_id(selected_pattern_index)
            .map(|p| p.channel_count)
            .unwrap_or(0);

        let channel_caches = (0..channel_count)
            .map(|_| ChannelCache::new(row_count))
            .collect();

        Self {
            state,
            channel_caches,
        }
    }
}
struct CachedRow {
    text: Line<'static>,
}

struct ChannelCache {
    rows: Vec<CachedRow>,
}

impl ChannelCache {
    fn new(row_count: usize) -> Self {
        let rows = (0..row_count)
            .map(|_| CachedRow {
                text: Line::from("A-1|00|00|00"),
            })
            .collect();
        Self { rows }
    }

    fn update_row(&mut self, index: usize, value: &str) {
        self.rows[index].text = Line::from(value.to_string());
    }
}
impl Widget for TimeLineView {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let selected_pattern_index = self.state.borrow().selected_pattern_index;
        let mut state = self.state.borrow_mut();
        let pattern_store = state.project.pattern_store();
        let pattern = pattern_store
            .get_pattern_by_id(selected_pattern_index)
            .unwrap();
        let layout = Layout::horizontal((0..pattern.channel_count).map(|_| Constraint::Length(14)))
            .margin(1)
            .split(area);

        Block::new()
            .borders(Borders::BOTTOM)
            .border_style(Style::new().yellow())
            .render(area, buf);

        for i in 0..pattern.channel_count {
            let text = Line::raw(format!("Track {:02}", i)).centered();

            let track_layout = Layout::vertical([Constraint::Length(3), Constraint::Fill(1)])
                .margin(1)
                .split(layout[i]);
            text.render(track_layout[0], buf);
            let rows_layout =
                Layout::vertical((0..pattern.row_count - 1).map(|_| Constraint::Fill(1)))
                    .split(track_layout[1]);

            for row in 0..pattern.row_count - 1 {
                let text = Line::raw(format!("A-1|00|00|00")).centered();
                text.render(rows_layout[row], buf);
            }

            Block::bordered()
                .border_style(Style::new().green())
                .render(layout[i], buf);
        }
    }
}
