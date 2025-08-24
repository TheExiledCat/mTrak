use std::{cell::RefCell, rc::Rc};

use ratatui::{
    layout::{Constraint, Layout},
    style::{Style, Stylize},
    widgets::{Block, Paragraph, Widget},
};

use crate::tui::app::AppState;

pub struct SideBar {
    state: Rc<RefCell<AppState>>,
}
impl SideBar {
    pub fn new(state: Rc<RefCell<AppState>>) -> Self {
        return SideBar { state };
    }
}
impl Widget for SideBar {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let layout = Layout::vertical([Constraint::Percentage(20), Constraint::Fill(1)])
            .spacing(1)
            .margin(1)
            .split(area);
        let title = Paragraph::new("Patterns")
            .style(Style::new().red())
            .centered();
        let border = Block::bordered().border_style(Style::new().red());
        title.render(layout[0], buf);
        border.render(area, buf);
        let selected_index = {
            let state = self.state.borrow();
            state.selected_pattern_index
        };
        let mut state = self.state.borrow_mut();
        let pattern_store = state.project.pattern_store();
        let patterns = pattern_store.get_patterns();
        let list =
            Layout::vertical((0..patterns.len()).map(|i| Constraint::Length(3))).split(layout[1]);
        for (i, pattern) in patterns.iter().enumerate() {
            let style = if selected_index == i {
                Style::new().green()
            } else {
                Style::new().blue()
            };
            let text = Paragraph::new(format!("Pattern: {:02}", i + 1))
                .centered()
                .block(Block::bordered().border_style(style));

            text.render(list[i], buf);
        }
    }
}
