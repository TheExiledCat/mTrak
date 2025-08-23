use ratatui::{
    layout::{Constraint, Layout},
    style::{Style, Stylize},
    widgets::{Block, StatefulWidget, Widget},
};

pub struct SideBar {}
impl SideBar {
    pub fn new() -> Self {
        return SideBar {};
    }
}
impl Widget for SideBar {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let layout = Layout::vertical([Constraint::Percentage(100)]).split(area);
        let border = Block::bordered()
            .border_style(Style::new().red())
            .render(area, buf);
    }
}
