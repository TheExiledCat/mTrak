use ratatui::{
    buffer::Buffer,
    crossterm::event::{Event, KeyCode, KeyModifiers},
    layout::Rect,
    text::Line,
    widgets::Widget,
};

use crate::{
    error::Result,
    tui::{
        app::AppState,
        framework::{
            dialog::{DialogComponent, DialogValue},
            keymap::{Action, HelpEntry, Key, KeyActions, KeyHint, Keymap},
            widget::MtrakWidget,
        },
        glyphs,
    },
};

type ItemSource = Box<dyn Fn(&AppState) -> Vec<String>>;
type CheckSource = Box<dyn Fn(&AppState) -> Vec<bool>>;

/// What was last done in a list, read in `on_update` with `DialogResult::list_event`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListEvent {
    /// Space on an item.
    Open(usize),
    /// Space on the add row.
    Add,
    Remove(usize),
    /// An item moved one place up or down.
    Move {
        from: usize,
        to: usize,
    },
    /// An item's checkbox was ticked or cleared.
    Check {
        item: usize,
        checked: bool,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ListAction {
    /// Not `Up` and `Down`, so they rebind apart from the dropdown's.
    PrevItem,
    NextItem,
    Open,
    Remove,
    MoveUp,
    MoveDown,
    Check,
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
    checks: Option<Vec<bool>>,
    check_source: Option<CheckSource>,
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
            checks: None,
            check_source: None,
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
    /// Puts a checkbox in front of each item, which X ticks or clears.
    pub fn checkboxes(mut self, checks: Vec<bool>) -> Self {
        self.checks = Some(checks);
        self.keys = self.bindings();
        return self;
    }
    /// Reads the checkboxes from the app state on every update, like `items_from`.
    pub fn checks_from(mut self, source: impl Fn(&AppState) -> Vec<bool> + 'static) -> Self {
        self.check_source = Some(Box::new(source));
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

    /// Every key a list can have, for the shortcuts screen and keybinds.toml.
    pub fn help_entries() -> Vec<HelpEntry> {
        let list = List::new("", Vec::<String>::new())
            .add_button("")
            .removable(true)
            .reorderable(true)
            .checkboxes(Vec::new());
        return list.keys.help_entries(List::describe);
    }

    fn bindings(&self) -> Keymap<ListAction> {
        let mut keys = Keymap::new("dialog")
            .bind(Key::plain(KeyCode::Up), ListAction::PrevItem, "Select")
            .bind(Key::plain(KeyCode::Down), ListAction::NextItem, "Select");
        if self.selectable || self.add_label.is_some() {
            keys = keys.bind(Key::char(' '), ListAction::Open, "Open");
        }
        if self.selectable && self.removable {
            keys = keys.bind(Key::plain(KeyCode::Delete), ListAction::Remove, "Remove");
        }
        if self.selectable && self.checks.is_some() {
            keys = keys.bind(Key::char('x'), ListAction::Check, "On/off");
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
            ListAction::PrevItem if self.selected > self.first_row() => self.selected -= 1,
            ListAction::NextItem if self.selected + 1 < self.rows() => self.selected += 1,
            ListAction::Open if self.on_item() => self.emit(ListEvent::Open(self.selected)),
            ListAction::Open if self.add_label.is_some() && self.selected == self.items.len() => {
                self.emit(ListEvent::Add)
            }
            ListAction::Remove if self.removable && self.on_item() => {
                self.items.remove(self.selected);
                if let Some(checks) = &mut self.checks
                    && self.selected < checks.len()
                {
                    checks.remove(self.selected);
                }
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
            ListAction::Check if self.on_item() => {
                let item = self.selected;
                if let Some(check) = self.checks.as_mut().and_then(|checks| checks.get_mut(item)) {
                    *check = !*check;
                    let checked = *check;
                    self.emit(ListEvent::Check { item, checked });
                }
            }
            _ => {}
        }
    }

    fn move_selected(&mut self, to: usize) {
        let from = self.selected;
        self.items.swap(from, to);
        if let Some(checks) = &mut self.checks
            && from.max(to) < checks.len()
        {
            checks.swap(from, to);
        }
        self.selected = to;
        self.emit(ListEvent::Move { from, to });
    }

    fn row_text(&self, row: usize) -> String {
        let Some(item) = self.items.get(row) else {
            return format!("+ {}", self.add_label.as_deref().unwrap_or_default());
        };
        let item = match &self.checks {
            Some(checks) => {
                let mark = if checks.get(row).copied().unwrap_or(false) {
                    "X"
                } else {
                    " "
                };
                format!("[{mark}] {item}")
            }
            None => item.clone(),
        };
        if !self.show_index {
            return item;
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
            ListAction::PrevItem => "Previous item",
            ListAction::NextItem => "Next item",
            ListAction::Open => "Open an item, or add one",
            ListAction::Remove => "Remove an item",
            ListAction::MoveUp => "Move an item up",
            ListAction::MoveDown => "Move an item down",
            ListAction::Check => "Tick or clear an item's checkbox",
        };
    }
}

impl MtrakWidget for List {
    fn update(&mut self, state: &mut AppState) {
        if let Some(source) = &self.source {
            self.items = source(state);
            self.clamp_selection();
        }
        if let Some(source) = &self.check_source {
            self.checks = Some(source(state));
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
        list.act(ListAction::PrevItem);
        assert_eq!(list.selected, 0);
        list.act(ListAction::NextItem);
        list.act(ListAction::NextItem);
        assert_eq!(list.selected, 1);
    }

    #[test]
    fn space_opens_the_selected_item() {
        let mut list = list(&["a", "b"]);
        list.act(ListAction::NextItem);
        list.act(ListAction::Open);
        assert_eq!(list.last, Some(ListEvent::Open(1)));
    }

    #[test]
    fn the_add_row_comes_after_the_items() {
        let mut list = list(&["a"]).add_button("Add");
        assert_eq!(list.get_height(), 2);
        list.act(ListAction::NextItem);
        assert_eq!(list.row_text(1), "+ Add");
        list.act(ListAction::Open);
        assert_eq!(list.last, Some(ListEvent::Add));
    }

    #[test]
    fn unselectable_items_are_skipped() {
        let mut list = list(&["a", "b"]).selectable(false).add_button("Add");
        list.set_focused(true);
        assert_eq!(list.selected, 2);
        list.act(ListAction::PrevItem);
        assert_eq!(list.selected, 2);
        assert!(!self::list(&["a"]).selectable(false).focusable());
    }

    #[test]
    fn remove_only_works_when_removable() {
        let mut list = list(&["a", "b"]);
        list.act(ListAction::Remove);
        assert_eq!(list.items.len(), 2);
        let mut list = list.removable(true);
        list.act(ListAction::NextItem);
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
        list.act(ListAction::NextItem);
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
    fn x_ticks_checkboxes_which_follow_moves_and_removes() {
        let mut list = list(&["a", "b", "c"])
            .checkboxes(vec![true, false, true])
            .reorderable(true)
            .removable(true)
            .show_index(true);
        assert_eq!(list.row_text(1), "1 [ ] b");
        list.act(ListAction::NextItem);
        list.act(ListAction::Check);
        assert_eq!(
            list.last,
            Some(ListEvent::Check {
                item: 1,
                checked: true
            })
        );
        assert_eq!(list.row_text(1), "1 [X] b");
        list.act(ListAction::Remove);
        assert_eq!(list.checks, Some(vec![true, true]));
        list.act(ListAction::MoveUp);
        list.act(ListAction::Check);
        assert_eq!(list.checks, Some(vec![false, true]));
        assert_eq!(list.items, vec!["c", "a"]);
    }

    #[test]
    fn x_does_nothing_without_checkboxes() {
        let mut list = list(&["a"]);
        list.act(ListAction::Check);
        assert_eq!(list.events, 0);
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
