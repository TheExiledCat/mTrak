use ratatui::{
    buffer::Buffer,
    crossterm::event::{Event, MouseButton, MouseEvent, MouseEventKind},
    layout::{Position, Rect},
};

use crate::tui::{app::AppState, framework::keymap::KeyHint};

pub trait MtrakWidget {
    fn update(&mut self, _state: &mut AppState) {}
    fn handle_event(&mut self, _state: &mut AppState, _event: &Event) -> bool {
        return false;
    }
    /// Adds the keys this widget currently responds to, for display in a footer or help view.
    /// Widgets with children should forward this to them.
    fn key_hints(&self, _state: &AppState, _hints: &mut Vec<KeyHint>) {}
    fn render(&mut self, state: &AppState, area: Rect, buf: &mut Buffer);
}

/// Returns the click position relative to `area` if `event` is a left click inside it.
pub fn clicked_in(event: &Event, area: Rect) -> Option<Position> {
    let Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        ..
    }) = event
    else {
        return None;
    };
    let pos = Position::new(*column, *row);
    if !area.contains(pos) {
        return None;
    }
    return Some(Position::new(pos.x - area.x, pos.y - area.y));
}
