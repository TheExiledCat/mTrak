use ratatui::{
    buffer::Buffer,
    crossterm::event::{Event, KeyCode, KeyModifiers},
    layout::Rect,
    text::Line,
    widgets::Widget,
};

use super::{DialogComponent, DialogValue};
use crate::{
    error::Result,
    tui::{
        app::AppState,
        framework::{
            keymap::{Action, Key, KeyActions, KeyHint, Keymap},
            widget::MtrakWidget,
        },
        glyphs,
    },
};

type ItemSource = Box<dyn Fn(&AppState) -> Vec<String>>;

/// What was last done in a list, read in `on_update` with `DialogResult::list_event`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListEvent {
    /// Enter on an item.
    Open(usize),
    /// Enter on the add row.
    Add,
    Remove(usize),
    /// An item moved one place up or down.
    Move {
        from: usize,
        to: usize,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ListAction {
    Up,
    Down,
    Open,
    Remove,
    MoveUp,
    MoveDown,
}

impl Action for ListAction {}

/// A line per item, with an optional row at the end for adding one. What the user does shows
/// up in `on_update` as a `ListEvent`. The list removes and moves its own lines; to keep it in
/// line with the app state, e.g. after a dialog opened from it changed the items, give it
/// `items_from`.
pub struct List {
    id: &'static str,
    items: Vec<String>,
    source: Option<ItemSource>,
    show_index: bool,
    add_label: Option<String>,
    selectable: bool,
    removable: bool,
    reorderable: bool,
    /// The selected row; the add row comes after the items.
    selected: usize,
    offset: usize,
    max_visible: u8,
    focused: bool,
    last: Option<ListEvent>,
    events: u32,
    keys: Keymap<ListAction>,
}

impl List {
    pub fn new(id: &'static str, items: Vec<impl Into<String>>) -> Self {
        let mut list = Self {
            id,
            items: items.into_iter().map(|item| item.into()).collect(),
            source: None,
            show_index: false,
            add_label: None,
            selectable: true,
            removable: false,
            reorderable: false,
            selected: 0,
            offset: 0,
            max_visible: 8,
            focused: false,
            last: None,
            events: 0,
            keys: Keymap::new("dialog"),
        };
        list.keys = list.bindings();
        return list;
    }

    /// Puts each item's index in hex in front of it, like the tracker's other lists.
    pub fn show_index(mut self, show: bool) -> Self {
        self.show_index = show;
        return self;
    }
    /// Adds a row after the items that sends `ListEvent::Add` when it's chosen.
    pub fn add_button(mut self, label: impl Into<String>) -> Self {
        self.add_label = Some(label.into());
        self.keys = self.bindings();
        self.clamp_selection();
        return self;
    }
    /// Whether the cursor can go onto items. A list whose items aren't selectable only shows
    /// them, and can't remove or move them.
    pub fn selectable(mut self, selectable: bool) -> Self {
        self.selectable = selectable;
        self.keys = self.bindings();
        self.clamp_selection();
        return self;
    }
    /// Lets Del remove the selected item.
    pub fn removable(mut self, removable: bool) -> Self {
        self.removable = removable;
        self.keys = self.bindings();
        return self;
    }
    /// Lets Shift+Up and Shift+Down move the selected item.
    pub fn reorderable(mut self, reorderable: bool) -> Self {
        self.reorderable = reorderable;
        self.keys = self.bindings();
        return self;
    }
    pub fn max_visible(mut self, rows: u8) -> Self {
        self.max_visible = rows.max(1);
        return self;
    }
    /// Reads the items from the app state on every update instead of keeping its own.
    pub fn items_from(mut self, source: impl Fn(&AppState) -> Vec<String> + 'static) -> Self {
        self.source = Some(Box::new(source));
        return self;
    }

    fn bindings(&self) -> Keymap<ListAction> {
        let mut keys = Keymap::new("dialog")
            .bind(Key::plain(KeyCode::Up), ListAction::Up, "Select")
            .bind(Key::plain(KeyCode::Down), ListAction::Down, "Select");
        if self.selectable || self.add_label.is_some() {
            keys = keys.bind(Key::plain(KeyCode::Enter), ListAction::Open, "Open");
        }
        if self.selectable && self.removable {
            keys = keys.bind(Key::plain(KeyCode::Delete), ListAction::Remove, "Remove");
        }
        if self.selectable && self.reorderable {
            keys = keys
                .bind(
                    Key::new(KeyCode::Up, KeyModifiers::SHIFT),
                    ListAction::MoveUp,
                    "Move",
                )
                .bind(
                    Key::new(KeyCode::Down, KeyModifiers::SHIFT),
                    ListAction::MoveDown,
                    "Move",
                );
        }
        return keys;
    }

    fn rows(&self) -> usize {
        return self.items.len() + self.add_label.is_some() as usize;
    }
    fn first_row(&self) -> usize {
        return if self.selectable { 0 } else { self.items.len() };
    }
    fn on_item(&self) -> bool {
        return self.selectable && self.selected < self.items.len();
    }
    fn clamp_selection(&mut self) {
        let last = self.rows().saturating_sub(1);
        self.selected = self.selected.clamp(self.first_row().min(last), last);
    }
    fn emit(&mut self, event: ListEvent) {
        self.last = Some(event);
        self.events += 1;
    }

    fn act(&mut self, action: ListAction) {
        match action {
            ListAction::Up if self.selected > self.first_row() => self.selected -= 1,
            ListAction::Down if self.selected + 1 < self.rows() => self.selected += 1,
            ListAction::Open if self.on_item() => self.emit(ListEvent::Open(self.selected)),
            ListAction::Open if self.add_label.is_some() && self.selected == self.items.len() => {
                self.emit(ListEvent::Add)
            }
            ListAction::Remove if self.removable && self.on_item() => {
                self.items.remove(self.selected);
                self.emit(ListEvent::Remove(self.selected));
                self.clamp_selection();
            }
            ListAction::MoveUp if self.reorderable && self.on_item() && self.selected > 0 => {
                self.move_selected(self.selected - 1);
            }
            ListAction::MoveDown
                if self.reorderable && self.on_item() && self.selected + 1 < self.items.len() =>
            {
                self.move_selected(self.selected + 1);
            }
            _ => {}
        }
    }

    fn move_selected(&mut self, to: usize) {
        let from = self.selected;
        self.items.swap(from, to);
        self.selected = to;
        self.emit(ListEvent::Move { from, to });
    }

    fn row_text(&self, row: usize) -> String {
        let Some(item) = self.items.get(row) else {
            return format!("+ {}", self.add_label.as_deref().unwrap_or_default());
        };
        if !self.show_index {
            return item.clone();
        }
        let width = format!("{:X}", self.items.len().saturating_sub(1)).len();
        return format!("{row:>width$X} {item}");
    }
}

impl KeyActions for List {
    type Action = ListAction;

    fn keymap(&self) -> &Keymap<ListAction> {
        return &self.keys;
    }

    fn perform(&mut self, action: ListAction, _state: &mut AppState) -> Result<()> {
        self.act(action);
        return Ok(());
    }

    fn describe(action: ListAction) -> &'static str {
        return match action {
            ListAction::Up => "Previous item",
            ListAction::Down => "Next item",
            ListAction::Open => "Open an item, or add one",
            ListAction::Remove => "Remove an item",
            ListAction::MoveUp => "Move an item up",
            ListAction::MoveDown => "Move an item down",
        };
    }
}

impl MtrakWidget for List {
    fn update(&mut self, state: &mut AppState) {
        if let Some(source) = &self.source {
            self.items = source(state);
            self.clamp_selection();
        }
    }

    fn handle_event(&mut self, state: &mut AppState, event: &Event) -> bool {
        return self.dispatch(state, event);
    }

    fn key_hints(&self, _state: &AppState, hints: &mut Vec<KeyHint>) {
        self.keys.hints(hints);
    }

    fn render(&mut self, state: &AppState, area: Rect, buf: &mut Buffer) {
        let rows = self.rows();
        if rows == 0 {
            Line::styled("(empty)", state.theme.text_disabled).render(area, buf);
            return;
        }
        // scroll just enough to keep the selection in view
        let visible = area.height as usize;
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + visible {
            self.offset = self.selected + 1 - visible;
        }
        for (line, row) in (self.offset..rows).take(visible).enumerate() {
            let line_area = Rect {
                y: area.y + line as u16,
                height: 1,
                ..area
            };
            if row == self.selected && row >= self.first_row() {
                let style = if self.focused {
                    state.theme.selection
                } else {
                    state.theme.selection_subtle
                };
                buf.set_style(line_area, style);
            }
            Line::from(self.row_text(row)).render(line_area, buf);
        }
        let arrow_x = area.right().saturating_sub(1);
        if self.offset > 0 {
            buf.set_string(arrow_x, area.y, glyphs::pick("▲", "^"), state.theme.border);
        }
        if self.offset + visible < rows {
            let y = area.bottom().saturating_sub(1);
            buf.set_string(arrow_x, y, glyphs::pick("▼", "v"), state.theme.border);
        }
    }
}

impl DialogComponent for List {
    fn get_height(&self) -> u8 {
        return self.rows().clamp(1, self.max_visible as usize) as u8;
    }
    /// Enter opens the selected item instead of submitting the dialog.
    fn takes_keys_first(&self) -> bool {
        return true;
    }
    fn focusable(&self) -> bool {
        return self.rows() > self.first_row();
    }
    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
        self.clamp_selection();
    }
    fn value(&self) -> Option<(&'static str, DialogValue)> {
        return Some((
            self.id,
            DialogValue::List {
                event: self.last,
                count: self.events,
            },
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(items: &[&str]) -> List {
        return List::new("list", items.to_vec());
    }

    #[test]
    fn up_and_down_stay_in_the_list() {
        let mut list = list(&["a", "b"]);
        list.act(ListAction::Up);
        assert_eq!(list.selected, 0);
        list.act(ListAction::Down);
        list.act(ListAction::Down);
        assert_eq!(list.selected, 1);
    }

    #[test]
    fn enter_opens_the_selected_item() {
        let mut list = list(&["a", "b"]);
        list.act(ListAction::Down);
        list.act(ListAction::Open);
        assert_eq!(list.last, Some(ListEvent::Open(1)));
    }

    #[test]
    fn the_add_row_comes_after_the_items() {
        let mut list = list(&["a"]).add_button("Add");
        assert_eq!(list.get_height(), 2);
        list.act(ListAction::Down);
        assert_eq!(list.row_text(1), "+ Add");
        list.act(ListAction::Open);
        assert_eq!(list.last, Some(ListEvent::Add));
    }

    #[test]
    fn unselectable_items_are_skipped() {
        let mut list = list(&["a", "b"]).selectable(false).add_button("Add");
        list.set_focused(true);
        assert_eq!(list.selected, 2);
        list.act(ListAction::Up);
        assert_eq!(list.selected, 2);
        assert!(!self::list(&["a"]).selectable(false).focusable());
    }

    #[test]
    fn remove_only_works_when_removable() {
        let mut list = list(&["a", "b"]);
        list.act(ListAction::Remove);
        assert_eq!(list.items.len(), 2);
        let mut list = list.removable(true);
        list.act(ListAction::Down);
        list.act(ListAction::Remove);
        assert_eq!(list.items, vec!["a"]);
        assert_eq!(list.last, Some(ListEvent::Remove(1)));
        assert_eq!(list.selected, 0);
    }

    #[test]
    fn moving_swaps_items_and_follows_the_moved_one() {
        let mut list = list(&["a", "b", "c"]).reorderable(true);
        list.act(ListAction::MoveDown);
        assert_eq!(list.items, vec!["b", "a", "c"]);
        assert_eq!(list.selected, 1);
        assert_eq!(list.last, Some(ListEvent::Move { from: 0, to: 1 }));
        list.act(ListAction::MoveUp);
        list.act(ListAction::MoveUp);
        assert_eq!(list.items, vec!["a", "b", "c"]);
        assert_eq!(list.events, 2);
    }

    #[test]
    fn the_add_row_never_moves() {
        let mut list = list(&["a"]).reorderable(true).add_button("Add");
        list.act(ListAction::MoveDown);
        list.act(ListAction::Down);
        list.act(ListAction::MoveUp);
        assert_eq!(list.items, vec!["a"]);
        assert_eq!(list.events, 0);
    }

    #[test]
    fn every_event_changes_the_value() {
        let mut list = list(&["a"]);
        let before = list.value();
        list.act(ListAction::Open);
        let once = list.value();
        list.act(ListAction::Open);
        assert_ne!(before, once);
        assert_ne!(once, list.value());
    }

    #[test]
    fn indices_are_hex_and_padded() {
        let items: Vec<String> = (0..17).map(|i| i.to_string()).collect();
        let list = List::new("list", items).show_index(true);
        assert_eq!(list.row_text(0), " 0 0");
        assert_eq!(list.row_text(16), "10 16");
    }

    #[test]
    fn height_is_capped_at_max_visible() {
        let items: Vec<String> = (0..20).map(|i| i.to_string()).collect();
        assert_eq!(List::new("list", items).max_visible(5).get_height(), 5);
        assert_eq!(List::new("list", Vec::<String>::new()).get_height(), 1);
    }
}
