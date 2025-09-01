use std::{cell::RefCell, iter::repeat_n, rc::Rc};

use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Style, Stylize},
    widgets::{Block, Borders, Padding, Paragraph, Widget},
};

use crate::tui::app::AppState;

pub struct Header {
    state: Rc<RefCell<AppState>>,
}

impl Header {}
impl Header {
    pub fn new(state: Rc<RefCell<AppState>>) -> Self {
        return Header { state };
    }
}

impl Widget for Header {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(repeat_n(Constraint::Fill(1), 4))
            .split(area.clone());
        let border = Block::new()
            .borders(Borders::BOTTOM)
            .border_style(Style::new().blue());
        let project_name;
        if let Some(name) = &self.state.borrow().project.name {
            project_name = name.clone();
        } else {
            project_name = "Unsaved".into();
        }
        let title = Paragraph::new(format!("Mini Tracker: {}", project_name))
            .style(Style::new().bold().red())
            .centered()
            .block(
                Block::new()
                    .padding(Padding::vertical(1))
                    .borders(Borders::TOP)
                    .border_style(Style::new().red())
                    .title("Ctrl+Q to exit")
                    .title_alignment(ratatui::layout::Alignment::Left),
            );
        title.render(layout[0], buf);

        border.render(area, buf);
    }
}
