use std::collections::HashMap;

use ratatui::{
    buffer::Buffer,
    crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers},
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Clear, Padding, Widget},
};

use crate::tui::{
    app::AppState,
    framework::{
        core_extensions::Boxed,
        keymap::{Key, KeyActions, KeyHint, Keymap},
        widget::MtrakWidget,
    },
};

/// The value a single input component contributes to a `DialogResult`.
#[derive(Clone, Debug, PartialEq)]
pub enum DialogValue {
    Text(String),
    Bool(bool),
    Choice(usize),
}

/// The values of every input component when the dialog was submitted, keyed by component id.
pub struct DialogResult {
    values: HashMap<&'static str, DialogValue>,
}

impl DialogResult {
    pub fn get(&self, id: &str) -> Option<&DialogValue> {
        return self.values.get(id);
    }

    pub fn text(&self, id: &str) -> Option<&str> {
        return match self.values.get(id) {
            Some(DialogValue::Text(s)) => Some(s),
            _ => None,
        };
    }

    pub fn bool(&self, id: &str) -> Option<bool> {
        return match self.values.get(id) {
            Some(DialogValue::Bool(b)) => Some(*b),
            _ => None,
        };
    }

    pub fn choice(&self, id: &str) -> Option<usize> {
        return match self.values.get(id) {
            Some(DialogValue::Choice(i)) => Some(*i),
            _ => None,
        };
    }
}

pub trait DialogComponent: MtrakWidget {
    fn get_height(&self) -> u8 {
        return 1;
    }
    /// Whether Tab can move focus onto this component. Static content should return false.
    fn focusable(&self) -> bool {
        return false;
    }
    fn set_focused(&mut self, _focused: bool) {}
    /// The id and current value of an input component; `None` for static content.
    fn value(&self) -> Option<(&'static str, DialogValue)> {
        return None;
    }
}

type OnSubmit = Box<dyn FnOnce(&mut AppState, DialogResult)>;
type OnCancel = Box<dyn FnOnce(&mut AppState)>;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DialogAction {
    Submit,
    Cancel,
    NextField,
    PrevField,
}

/// A modal popup made of components. Open one with `AppState::open_dialog`; once the user
/// submits or cancels, the matching callback runs with the app state and the dialog closes.
pub struct Dialog {
    title: String,
    width: u16,
    components: Vec<Box<dyn DialogComponent>>,
    focus: Option<usize>,
    on_submit: Option<OnSubmit>,
    on_cancel: Option<OnCancel>,
    closed: bool,
    keys: Keymap<DialogAction>,
}

impl Dialog {
    pub fn new(title: impl Into<String>, components: Vec<Box<dyn DialogComponent>>) -> Self {
        let mut dialog = Self {
            title: title.into(),
            width: 50,
            components,
            focus: None,
            on_submit: None,
            on_cancel: None,
            closed: false,
            keys: Keymap::new()
                .bind(Key::plain(KeyCode::Enter), DialogAction::Submit, "OK")
                .bind(Key::plain(KeyCode::Esc), DialogAction::Cancel, "Cancel")
                .bind(Key::plain(KeyCode::Tab), DialogAction::NextField, "Field")
                .bind(
                    Key::plain(KeyCode::BackTab),
                    DialogAction::PrevField,
                    "Field",
                ),
        };
        dialog.move_focus(true);
        return dialog;
    }
    pub fn text_field(
        title: impl Into<String>,
        placeholder: impl Into<String>,
        id: &'static str,
    ) -> Self {
        return Dialog::new(
            title,
            vec![TextPrompt::new(id, "").initial(placeholder).boxed()],
        );
    }
    pub fn width(mut self, width: u16) -> Self {
        self.width = width;
        return self;
    }

    /// Runs when the user confirms the dialog, receiving the values of its input components.
    pub fn on_submit(mut self, f: impl FnOnce(&mut AppState, DialogResult) + 'static) -> Self {
        self.on_submit = Some(Box::new(f));
        return self;
    }

    /// Runs when the user dismisses the dialog without confirming it.
    pub fn on_cancel(mut self, f: impl FnOnce(&mut AppState) + 'static) -> Self {
        self.on_cancel = Some(Box::new(f));
        return self;
    }

    pub fn is_closed(&self) -> bool {
        return self.closed;
    }

    fn submit(&mut self, state: &mut AppState) {
        let values = self.components.iter().filter_map(|c| c.value()).collect();
        self.closed = true;
        if let Some(f) = self.on_submit.take() {
            f(state, DialogResult { values });
        }
    }

    fn cancel(&mut self, state: &mut AppState) {
        self.closed = true;
        if let Some(f) = self.on_cancel.take() {
            f(state);
        }
    }

    /// Moves focus to the next (or previous) focusable component, wrapping around.
    fn move_focus(&mut self, forward: bool) {
        let count = self.components.len();
        if count == 0 {
            return;
        }
        let start = self.focus.unwrap_or(if forward { count - 1 } else { 0 });
        for step in 1..=count {
            let index = if forward {
                (start + step) % count
            } else {
                (start + count - step) % count
            };
            if self.components[index].focusable() {
                if let Some(old) = self.focus {
                    self.components[old].set_focused(false);
                }
                self.components[index].set_focused(true);
                self.focus = Some(index);
                return;
            }
        }
    }

    fn content_height(&self) -> u16 {
        return self.components.iter().map(|c| c.get_height() as u16).sum();
    }
}

impl KeyActions for Dialog {
    type Action = DialogAction;

    fn keymap(&self) -> &Keymap<DialogAction> {
        return &self.keys;
    }

    fn perform(&mut self, action: DialogAction, state: &mut AppState) {
        match action {
            DialogAction::Submit => self.submit(state),
            DialogAction::Cancel => self.cancel(state),
            DialogAction::NextField => self.move_focus(true),
            DialogAction::PrevField => self.move_focus(false),
        }
    }
}

impl MtrakWidget for Dialog {
    fn update(&mut self, state: &mut AppState) {
        for component in &mut self.components {
            component.update(state);
        }
    }

    /// Always returns true: a dialog is modal, so nothing beneath it sees input.
    fn handle_event(&mut self, state: &mut AppState, event: &Event) -> bool {
        if !self.dispatch(state, event) {
            if let Some(index) = self.focus {
                self.components[index].handle_event(state, event);
            }
        }
        return true;
    }

    fn key_hints(&self, state: &AppState, hints: &mut Vec<KeyHint>) {
        if let Some(index) = self.focus {
            self.components[index].key_hints(state, hints);
        }
        self.keys.hints(hints);
        // switching fields is pointless with fewer than two of them
        if self.components.iter().filter(|c| c.focusable()).count() < 2 {
            hints.retain(|h| h.label != "Field");
        }
    }

    fn render(&mut self, state: &AppState, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .title(format!(" {} ", self.title))
            .border_style(state.theme.dialog_border)
            .padding(Padding::horizontal(1));

        // borders + content + spacer + hint line
        let width = self.width.min(area.width);
        let height = (self.content_height() + 4).min(area.height);
        let popup = Rect {
            x: area.x + (area.width - width) / 2,
            y: area.y + (area.height - height) / 2,
            width,
            height,
        };
        Clear.render(popup, buf);
        buf.set_style(popup, state.theme.desktop);
        let inner = block.inner(popup);
        block.render(popup, buf);

        let [content, _, hint_area] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(inner);

        let rows = Layout::vertical(
            self.components
                .iter()
                .map(|c| Constraint::Length(c.get_height() as u16)),
        )
        .split(content);
        for (component, row) in self.components.iter_mut().zip(rows.iter()) {
            component.render(state, *row, buf);
        }

        let mut hints = Vec::new();
        self.key_hints(state, &mut hints);
        let spans = hints.iter().flat_map(|hint| {
            [
                Span::styled(format!(" {} ", hint.key), state.theme.key_hint_key),
                Span::styled(format!(" {}  ", hint.label), state.theme.key_hint_label),
            ]
        });
        Line::from_iter(spans).render(hint_area, buf);
    }
}

/// Static text.
pub struct Label {
    text: String,
}

impl Label {
    pub fn new(text: impl Into<String>) -> Self {
        return Self { text: text.into() };
    }
}

impl MtrakWidget for Label {
    fn render(&mut self, _state: &AppState, area: Rect, buf: &mut Buffer) {
        Line::from(self.text.as_str()).render(area, buf);
    }
}

impl DialogComponent for Label {}

/// A single-line text input.
pub struct TextPrompt {
    id: &'static str,
    label: String,
    text: String,
    focused: bool,
}

impl TextPrompt {
    pub fn new(id: &'static str, label: impl Into<String>) -> Self {
        return Self {
            id,
            label: label.into(),
            text: String::new(),
            focused: false,
        };
    }

    pub fn initial(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        return self;
    }
}

impl MtrakWidget for TextPrompt {
    fn handle_event(&mut self, _state: &mut AppState, event: &Event) -> bool {
        let Event::Key(key) = event else {
            return false;
        };
        if key.kind == KeyEventKind::Release {
            return false;
        }
        match key.code {
            KeyCode::Char(c)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                self.text.push(c);
            }
            KeyCode::Backspace => {
                self.text.pop();
            }
            _ => return false,
        }
        return true;
    }

    fn render(&mut self, state: &AppState, area: Rect, buf: &mut Buffer) {
        let mut spans = vec![
            Span::raw(format!("{}: ", self.label)),
            Span::raw(self.text.as_str()),
        ];
        if self.focused {
            spans.push(Span::styled(" ", state.theme.selected_cell));
        }
        Line::from(spans).render(area, buf);
    }
}

impl DialogComponent for TextPrompt {
    fn focusable(&self) -> bool {
        return true;
    }
    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
    fn value(&self) -> Option<(&'static str, DialogValue)> {
        return Some((self.id, DialogValue::Text(self.text.clone())));
    }
}

/// A vertical list to pick one option from. Contributes no value when it has no options.
pub struct Select {
    id: &'static str,
    options: Vec<String>,
    selected: usize,
    offset: usize,
    max_visible: u8,
    focused: bool,
    keys: Keymap<SelectAction>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SelectAction {
    Up,
    Down,
}

impl Select {
    pub fn new(id: &'static str, options: Vec<String>) -> Self {
        return Self {
            id,
            options,
            selected: 0,
            offset: 0,
            max_visible: 8,
            focused: false,
            keys: Keymap::new()
                .bind(Key::plain(KeyCode::Up), SelectAction::Up, "Choose")
                .bind(Key::plain(KeyCode::Down), SelectAction::Down, "Choose"),
        };
    }

    pub fn selected(mut self, index: usize) -> Self {
        self.selected = index.min(self.options.len().saturating_sub(1));
        return self;
    }

    pub fn max_visible(mut self, rows: u8) -> Self {
        self.max_visible = rows.max(1);
        return self;
    }
}

impl KeyActions for Select {
    type Action = SelectAction;

    fn keymap(&self) -> &Keymap<SelectAction> {
        return &self.keys;
    }

    fn perform(&mut self, action: SelectAction, _state: &mut AppState) {
        let last = self.options.len().saturating_sub(1);
        self.selected = match action {
            SelectAction::Up => self.selected.saturating_sub(1),
            SelectAction::Down => (self.selected + 1).min(last),
        };
    }
}

impl MtrakWidget for Select {
    fn handle_event(&mut self, state: &mut AppState, event: &Event) -> bool {
        return self.dispatch(state, event);
    }

    fn key_hints(&self, _state: &AppState, hints: &mut Vec<KeyHint>) {
        self.keys.hints(hints);
    }

    fn render(&mut self, state: &AppState, area: Rect, buf: &mut Buffer) {
        if self.options.is_empty() {
            Line::from("(none)").render(area, buf);
            return;
        }
        // scroll just enough to keep the selection in view
        let visible = area.height as usize;
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + visible {
            self.offset = self.selected + 1 - visible;
        }
        for (row, index) in (self.offset..self.options.len()).take(visible).enumerate() {
            let style = match (index == self.selected, self.focused) {
                (true, true) => state.theme.selected_cell,
                (true, false) => state.theme.selected_row,
                _ => Style::new(),
            };
            let line_area = Rect {
                y: area.y + row as u16,
                height: 1,
                ..area
            };
            Line::styled(self.options[index].as_str(), style).render(line_area, buf);
        }
    }
}

impl DialogComponent for Select {
    fn get_height(&self) -> u8 {
        return self.options.len().clamp(1, self.max_visible as usize) as u8;
    }
    fn focusable(&self) -> bool {
        return !self.options.is_empty();
    }
    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
    fn value(&self) -> Option<(&'static str, DialogValue)> {
        if self.options.is_empty() {
            return None;
        }
        return Some((self.id, DialogValue::Choice(self.selected)));
    }
}
