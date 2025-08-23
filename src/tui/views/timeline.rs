use ratatui::widgets::Widget;

pub struct TimeLineView {}
impl TimeLineView {
    pub fn new() -> Self {
        return Self {};
    }
}
impl Widget for TimeLineView {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        todo!()
    }
}
