use ratatui::widgets::{Block, Padding};

use crate::tui::framework::{keymap::KeyHint, widget::MtrakWidget};

pub struct Container {
    padding: Padding,
    child: Box<dyn MtrakWidget>,
}
impl Container {
    pub fn new(root: Box<dyn MtrakWidget>, padding: Padding) -> Self {
        return Self {
            child: root,
            padding,
        };
    }
}
impl MtrakWidget for Container {
    fn update(&mut self, state: &mut crate::tui::app::AppState) {
        self.child.update(state);
    }

    fn handle_event(
        &mut self,
        state: &mut crate::tui::app::AppState,
        event: &ratatui::crossterm::event::Event,
    ) -> bool {
        return self.child.handle_event(state, event);
    }

    fn key_hints(&self, state: &crate::tui::app::AppState, hints: &mut Vec<KeyHint>) {
        self.child.key_hints(state, hints);
    }

    fn render(
        &mut self,
        state: &crate::tui::app::AppState,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
    ) {
        let inner = Block::new().padding(self.padding).inner(area);
        self.child.render(state, inner, buf);
    }
}
