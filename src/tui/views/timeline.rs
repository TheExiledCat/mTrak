use ratatui::{
    style::{Style, Stylize},
    widgets::{Block, Borders, Widget},
};

use crate::tui::app::AppState;

pub struct TimeLineView<'a> {
    state: &'a AppState,
}
impl<'a> TimeLineView<'a> {
    pub fn new(state: &'a AppState) -> Self {
        return Self { state };
    }
}
impl<'a> Widget for TimeLineView<'a> {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        Block::new()
            .borders(Borders::BOTTOM)
            .border_style(Style::new().yellow())
            .render(area, buf);
    }
}
