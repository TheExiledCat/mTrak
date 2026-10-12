use std::collections::HashMap;

use ratatui::{
    buffer::Buffer,
    crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers},
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Padding, Widget},
};

use crate::{
    error::Result,
    tui::{
        app::AppState,
        framework::{
            core_extensions::Boxed,
            keymap::{HelpSection, Key, KeyActions, KeyHint, Keymap},
            widget::MtrakWidget,
            widgets::footer::Footer,
        },
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

type OnSubmit = Box<dyn FnOnce(&mut AppState, DialogResult) -> Result<()>>;
type OnCancel = Box<dyn FnOnce(&mut AppState)>;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DialogAction {
    Submit,
    Cancel,
    NextField,
    PrevField,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PopupLevel {
    Info,
    Warning,
    Error,
}

/// A modal popup made of components. Open one with `AppState::open_dialog`; once the user
/// submits or cancels, the matching callback runs with the app state and the dialog closes.
pub struct Dialog {
    title: String,
    width: u16,
    level: PopupLevel,
    is_cancellable: bool,
    components: Vec<Box<dyn DialogComponent>>,
    focus: Option<usize>,
    on_submit: Option<OnSubmit>,
    on_cancel: Option<OnCancel>,
    closed: bool,
    keys: Keymap<DialogAction>,
    footer: Footer,
}

impl Dialog {
    pub fn new(title: impl Into<String>, components: Vec<Box<dyn DialogComponent>>) -> Self {
        let mut dialog = Self {
            title: title.into(),
            width: 50,
            level: PopupLevel::Info,
            is_cancellable: true,
            components,
            focus: None,
            on_submit: None,
            on_cancel: None,
            closed: false,
            footer: Footer::new(),
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
    pub fn confirm(level: PopupLevel, title: impl Into<String>, text: impl Into<String>) -> Self {
        let mut dialog = Self::new(title, vec![Label::new(text).boxed()]);
        dialog.keys = Keymap::new()
            .bind(Key::plain(KeyCode::Enter), DialogAction::Submit, "Confirm")
            .bind(Key::plain(KeyCode::Esc), DialogAction::Cancel, "Cancel");
        dialog.level = level;
        return dialog;
    }
    pub fn popup(level: PopupLevel, title: impl Into<String>, text: impl Into<String>) -> Self {
        let mut dialog = Dialog::new(title, vec![Label::new(text).boxed()]).is_cancellable(false);
        dialog.level = level;
        return dialog;
    }
    pub fn width(mut self, width: u16) -> Self {
        self.width = width;
        return self;
    }

    pub fn is_cancellable(mut self, is_cancellable: bool) -> Self {
        self.is_cancellable = is_cancellable;
        return self;
    }

    /// Runs when the user confirms the dialog, receiving the values of its input components.
    pub fn on_submit(
        mut self,
        f: impl FnOnce(&mut AppState, DialogResult) -> Result<()> + 'static,
    ) -> Self {
        self.on_submit = Some(Box::new(f));
        return self;
    }

    /// Runs when the user dismisses the dialog without confirming it.
    pub fn on_cancel(mut self, f: impl FnOnce(&mut AppState) + 'static) -> Self {
        self.on_cancel = Some(Box::new(f));
        return self;
    }

    pub fn help(sections: &mut Vec<HelpSection>) {
        let dialog = Dialog::new("", Vec::new());
        let select = Select::new("", Vec::new());
        let checkbox = Checkbox::new("", "");
        sections.push(
            dialog
                .help_section("Dialogs")
                .entries(select.keys.help_entries(Select::describe))
                .entries(checkbox.keys.help_entries(Checkbox::describe))
                .entry("Bksp", "Delete the last character of a text field"),
        );
    }

    pub fn is_closed(&self) -> bool {
        return self.closed;
    }

    fn submit(&mut self, state: &mut AppState) -> Result<()> {
        let values = self.components.iter().filter_map(|c| c.value()).collect();
        self.closed = true;
        let Some(f) = self.on_submit.take() else {
            return Ok(());
        };
        return f(state, DialogResult { values });
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

    fn perform(&mut self, action: DialogAction, state: &mut AppState) -> Result<()> {
        match action {
            DialogAction::Submit => self.submit(state)?,
            DialogAction::Cancel if self.is_cancellable => self.cancel(state),
            DialogAction::Cancel => {}
            DialogAction::NextField => self.move_focus(true),
            DialogAction::PrevField => self.move_focus(false),
        }
        return Ok(());
    }

    fn describe(action: DialogAction) -> &'static str {
        return match action {
            DialogAction::Submit => "Confirm",
            DialogAction::Cancel => "Cancel",
            DialogAction::NextField => "Next field",
            DialogAction::PrevField => "Previous field",
        };
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
        if !self.is_cancellable {
            hints.retain(|h| h.label != "Cancel");
        }
        // switching fields is pointless with fewer than two of them
        if self.components.iter().filter(|c| c.focusable()).count() < 2 {
            hints.retain(|h| h.label != "Field");
        }
    }

    fn render(&mut self, state: &AppState, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .title(format!(" {} ", self.title))
            .border_style(match self.level {
                PopupLevel::Info => state.theme.dialog_border,
                PopupLevel::Warning => state.theme.dialog_warning_border,
                PopupLevel::Error => state.theme.dialog_error_border,
            })
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
        self.footer.set_hints(hints);
        self.footer.render(state, hint_area, buf);
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

pub struct Checkbox {
    id: &'static str,
    label: String,
    checked: bool,
    focused: bool,
    keys: Keymap<CheckboxAction>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CheckboxAction {
    Toggle,
}

impl Checkbox {
    pub fn new(id: &'static str, label: impl Into<String>) -> Self {
        return Self {
            id,
            label: label.into(),
            checked: false,
            focused: false,
            keys: Keymap::new().bind(Key::char(' '), CheckboxAction::Toggle, "Toggle"),
        };
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        return self;
    }
}

impl KeyActions for Checkbox {
    type Action = CheckboxAction;

    fn keymap(&self) -> &Keymap<CheckboxAction> {
        return &self.keys;
    }

    fn perform(&mut self, action: CheckboxAction, _state: &mut AppState) -> Result<()> {
        match action {
            CheckboxAction::Toggle => self.checked = !self.checked,
        }
        return Ok(());
    }

    fn describe(action: CheckboxAction) -> &'static str {
        return match action {
            CheckboxAction::Toggle => "Check or uncheck a checkbox",
        };
    }
}

impl MtrakWidget for Checkbox {
    fn handle_event(&mut self, state: &mut AppState, event: &Event) -> bool {
        return self.dispatch(state, event);
    }

    fn key_hints(&self, _state: &AppState, hints: &mut Vec<KeyHint>) {
        self.keys.hints(hints);
    }

    fn render(&mut self, state: &AppState, area: Rect, buf: &mut Buffer) {
        let mark = if self.checked { "X" } else { " " };
        let style = if self.focused {
            state.theme.selected_cell
        } else {
            Style::new()
        };
        Line::from(vec![
            Span::raw(format!("{}: ", self.label)),
            Span::styled(format!("[{mark}]"), style),
        ])
        .render(area, buf);
    }
}

impl DialogComponent for Checkbox {
    fn focusable(&self) -> bool {
        return true;
    }
    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
    fn value(&self) -> Option<(&'static str, DialogValue)> {
        return Some((self.id, DialogValue::Bool(self.checked)));
    }
}

/// A vertical list to pick one option from. Contributes no value when it has no options.
pub struct Select {
    id: &'static str,
    label: Option<String>,
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
            label: None,
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

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        return self;
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

    fn perform(&mut self, action: SelectAction, _state: &mut AppState) -> Result<()> {
        let last = self.options.len().saturating_sub(1);
        self.selected = match action {
            SelectAction::Up => self.selected.saturating_sub(1),
            SelectAction::Down => (self.selected + 1).min(last),
        };
        return Ok(());
    }

    fn describe(action: SelectAction) -> &'static str {
        return match action {
            SelectAction::Up => "Previous option",
            SelectAction::Down => "Next option",
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
        let area = match &self.label {
            Some(label) => {
                let [label_area, rest] =
                    Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(area);
                Line::from(format!("{label}:")).render(label_area, buf);
                rest
            }
            None => area,
        };
        let block = Block::new()
            .borders(Borders::LEFT)
            .border_style(state.theme.dialog_border);
        let list_area = block.inner(area);
        block.render(area, buf);
        let area = list_area;
        if self.options.is_empty() {
            Line::from(" (none)").render(area, buf);
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
            let marker = if index == self.selected { "▶" } else { " " };
            Line::styled(format!("{marker} {}", self.options[index]), style).render(line_area, buf);
        }
    }
}

impl DialogComponent for Select {
    fn get_height(&self) -> u8 {
        let label = self.label.is_some() as u8;
        return self.options.len().clamp(1, self.max_visible as usize) as u8 + label;
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
