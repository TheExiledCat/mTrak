use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Style, Stylize},
    widgets::{Block, Borders, Padding, Paragraph, Widget},
};

pub struct Header {}
impl Header {
    pub fn new() -> Self {
        return Header {};
    }
}

impl Widget for Header {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Ratio(1, 4)])
            .split(area.clone());
        let border = Block::new()
            .borders(Borders::BOTTOM)
            .border_style(Style::new().blue());
        let title = Paragraph::new("Mini Tracker")
            .style(Style::new().bold().red())
            .centered()
            .block(Block::new().padding(Padding::vertical(1)));
        title.render(layout[0], buf);
        border.render(area, buf);
    }
}
