use std::{cell::RefCell, rc::Rc};

use ratatui::{
    layout::{Constraint, Layout},
    style::{Style, Stylize},
    text::Line,
    widgets::{Block, Borders, Padding, Paragraph, Widget},
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
        let borrowed_state = self.state.borrow();
        let selected_pattern_index = borrowed_state.selected_pattern_index;
        let selected_row_index = borrowed_state.row_index;
        let selected_channel_index = borrowed_state.channel_index;
        let row_number_string = borrowed_state.row_number_lookup.clone();
        let editing = borrowed_state.is_editing;
        let mut state = self.state.borrow();
        let pattern_store = state.project.pattern_store();
        let pattern = pattern_store
            .get_pattern_by_id(selected_pattern_index)
            .unwrap();
        let top_bottom = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).split(area);
        let editing_text = Line::raw(if editing { "-- EDIT MODE --" } else { "" }).centered();
        editing_text.render(top_bottom[0], buf);
        let layout = Layout::horizontal(
            [Constraint::Fill(1)]
                .iter()
                .chain((0..pattern.channel_count).map(|_| &Constraint::Length(14)))
                .chain([Constraint::Fill(1)].iter()),
        )
        .margin(1)
        .split(top_bottom[1]);

        Block::new()
            .borders(Borders::BOTTOM)
            .border_style(Style::new().yellow())
            .render(area, buf);
        let line_numbers = Paragraph::new(
            (0..pattern.row_count)
                .map(|i| {
                    let line = Line::raw(format!("{:02X}", i));
                    let mut style = Style::new().black().on_light_green();
                    if i % 4 == 0 {
                        style = style.on_red();
                    }
                    if i == selected_row_index {
                        style = style.on_light_blue();
                    }
                    line.style(style)
                })
                .collect::<Vec<Line>>(),
        )
        .block(Block::new().padding(Padding::new(0, 1, 4, 0)))
        .right_aligned()
        .scroll((selected_row_index.saturating_sub(16) as u16, 0));
        line_numbers.render(layout[0], buf);
        let no_lines = [
            Line::raw("No."),
            Line::raw(format!(
                "{}{}",
                if row_number_string.chars().count() == 0 {
                    ""
                } else {
                    "SEARCH: "
                },
                row_number_string
            ))
            .style(Style::new().black().on_yellow()),
        ];
        Paragraph::new(Vec::from(no_lines))
            .right_aligned()
            .block(Block::new().padding(Padding::new(0, 1, 1, 0)))
            .render(layout[0], buf);
        for i in 0..pattern.channel_count {
            let text = Line::raw(format!("Track {:02}", i + 1)).centered();

            let track_layout = Layout::vertical([Constraint::Length(3), Constraint::Fill(1)])
                .margin(1)
                .split(layout[i + 1]);
            text.render(track_layout[0], buf);
            let mut rows = state
                .channel_cache
                .get_all_rows_for_channel(selected_pattern_index, i);
            let row = rows[selected_row_index].clone();
            if i == selected_channel_index {
                let new_row = row.centered().style(Style::new().black().on_white());
                rows[selected_row_index] = new_row;
            } else {
                let new_row = row.centered().style(Style::new().on_light_blue());
                rows[selected_row_index] = new_row;
            }
            let mut row_paragraph =
                Paragraph::new(rows).scroll((selected_row_index.saturating_sub(16) as u16, 0));
            row_paragraph.render(track_layout[1], buf);

            Block::bordered()
                .border_style(Style::new().green())
                .render(layout[i + 1], buf);
        }
    }
}
