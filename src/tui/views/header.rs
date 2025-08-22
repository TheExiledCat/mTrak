use ratatui::{layout::{Constraint, Direction, Layout}, widgets::Widget};

pub struct Header {}

impl Widget for Header {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let layout = Layout::default().direction(Direction::Horizontal).constraints([Constraint::Ratio(1, 3) ])
    }
}
