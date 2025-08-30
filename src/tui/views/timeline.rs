use std::{cell::RefCell, rc::Rc};

use ratatui::{
    layout::{Constraint, Layout},
    style::{Style, Stylize},
    text::Line,
    widgets::{Block, Borders, Widget},
};

use crate::tui::app::AppState;

pub struct TimeLineView {
    state: Rc<RefCell<AppState>>,
}
impl TimeLineView {
    pub fn new(state: Rc<RefCell<AppState>>) -> Self {
        Self { state }
    }
}

impl Widget for TimeLineView {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let selected_pattern_index = self.state.borrow().selected_pattern_index;
        let selected_row_index = self.state.borrow().row_index;
        let selected_channel_index = self.state.borrow().channel_index;
        let mut state = self.state.borrow();
        let pattern_store = state.project.pattern_store();
        let pattern = pattern_store
            .get_pattern_by_id(selected_pattern_index)
            .unwrap();

        let layout = Layout::horizontal(
            [Constraint::Fill(1)]
                .iter()
                .chain((0..pattern.channel_count).map(|_| &Constraint::Length(14)))
                .chain([Constraint::Fill(1)].iter()),
        )
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
                .split(layout[i + 1]);
            text.render(track_layout[0], buf);
            let rows_layout =
                Layout::vertical((0..pattern.row_count - 1).map(|_| Constraint::Fill(1)))
                    .split(track_layout[1]);

            for row in 0..pattern.row_count - 1 {
                let text = &state
                    .channel_cache
                    .get_row(selected_pattern_index, row)
                    .channels[i];
                if row == selected_row_index {
                    if i == selected_channel_index {
                        text.clone()
                            .centered()
                            .style(Style::new().black().on_white())
                            .render(rows_layout[row], buf);
                    } else {
                        text.clone()
                            .centered()
                            .style(Style::new().on_light_blue())
                            .render(rows_layout[row], buf);
                    }
                } else {
                    text.clone().centered().render(rows_layout[row], buf);
                }
            }

            Block::bordered()
                .border_style(Style::new().green())
                .render(layout[i + 1], buf);
        }
    }
}
