use std::collections::HashMap;

use ratatui::{
    buffer::Buffer,
    crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers},
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Padding, Paragraph, Widget},
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
        glyphs,
    },
};

/// The value a single input component contributes to a `DialogResult`.
#[derive(Clone, Debug, PartialEq)]
pub enum DialogValue {
    Text(String),
    Bool(bool),
    Choice(usize),
    Number(i32),
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

    pub fn number(&self, id: &str) -> Option<i32> {
        return match self.values.get(id) {
            Some(DialogValue::Number(n)) => Some(*n),
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
    /// Draws on top of the whole dialog, e.g. an open dropdown list.
    fn render_overlay(&mut self, _state: &AppState, _bounds: Rect, _buf: &mut Buffer) {}
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
pub enum InfoLevel {
    Info,
    Success,
    Warning,
    Error,
}

/// A modal popup made of components. Open one with `AppState::open_dialog`; once the user
/// submits or cancels, the matching callback runs with the app state and the dialog closes.
pub struct Dialog {
    title: String,
    width: u16,
    level: InfoLevel,
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
        let dialog = Self {
            title: title.into(),
            width: 50,
            level: InfoLevel::Info,
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
        )
        .focus_first();
    }
    pub fn confirm(level: InfoLevel, title: impl Into<String>, text: impl Into<String>) -> Self {
        let mut dialog = Self::new(title, vec![Label::new(text).boxed()]);
        dialog.keys = Keymap::new()
            .bind(Key::plain(KeyCode::Enter), DialogAction::Submit, "Confirm")
            .bind(Key::plain(KeyCode::Esc), DialogAction::Cancel, "Cancel");
        dialog.level = level;
        return dialog;
    }
    pub fn popup(level: InfoLevel, title: impl Into<String>, text: impl Into<String>) -> Self {
        let mut dialog = Dialog::new(title, vec![Label::new(text).boxed()]).is_cancellable(false);
        dialog.level = level;
        return dialog;
    }
    pub fn level(mut self, level: InfoLevel) -> Self {
        self.level = level;
        return self;
    }
    pub fn width(mut self, width: u16) -> Self {
        self.width = width;
        return self;
    }

    /// Opens with the first focusable component focused.
    pub fn focus_first(mut self) -> Self {
        if self.focus.is_none() {
            self.move_focus(true);
        }
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
        let select = Select::new("", Vec::<String>::new());
        let checkbox = Checkbox::new("", "");
        let number = NumberBox::new("", "", 2);
        sections.push(
            dialog
                .help_section("Dialogs")
                .entries(select.keys.help_entries(Select::describe))
                .entries(checkbox.keys.help_entries(Checkbox::describe))
                .entries(number.keys.help_entries(NumberBox::describe))
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

    /// Moves focus to the next or previous field, wrapping round through none.
    fn move_focus(&mut self, forward: bool) {
        let focusable: Vec<usize> = (0..self.components.len())
            .filter(|&i| self.components[i].focusable())
            .collect();
        let current = self
            .focus
            .and_then(|focus| focusable.iter().position(|&i| i == focus));
        let next = match (current, forward) {
            (None, true) => focusable.first().copied(),
            (None, false) => focusable.last().copied(),
            (Some(at), true) => focusable.get(at + 1).copied(),
            (Some(at), false) => at.checked_sub(1).map(|at| focusable[at]),
        };
        if let Some(old) = self.focus {
            self.components[old].set_focused(false);
        }
        if let Some(new) = next {
            self.components[new].set_focused(true);
        }
        self.focus = next;
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
        if !self.components.iter().any(|c| c.focusable()) {
            hints.retain(|h| h.label != "Field");
        }
    }

    fn render(&mut self, state: &AppState, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .title(format!(" {} ", self.title))
            .border_style(match self.level {
                InfoLevel::Info => state.theme.border,
                InfoLevel::Success => state.theme.border_success,
                InfoLevel::Warning => state.theme.border_warning,
                InfoLevel::Error => state.theme.border_danger,
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
        buf.set_style(popup, state.theme.background);
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

        if let Some(index) = self.focus {
            self.components[index].render_overlay(state, area, buf);
        }
    }
}

/// A component's label followed by its separator, marked while the component has focus.
fn label_spans<'a>(state: &AppState, label: &'a str, focused: bool) -> [Span<'a>; 2] {
    let style = if focused {
        state.theme.selection_subtle
    } else {
        Style::new()
    };
    return [Span::styled(format!("{label}:"), style), Span::raw(" ")];
}

/// Static text.
pub struct Label {
    text: String,
    centered: bool,
}

impl Label {
    pub fn new(text: impl Into<String>) -> Self {
        return Self {
            text: text.into(),
            centered: false,
        };
    }

    pub fn centered(mut self) -> Self {
        self.centered = true;
        return self;
    }
}

impl MtrakWidget for Label {
    fn render(&mut self, _state: &AppState, area: Rect, buf: &mut Buffer) {
        let mut paragraph = Paragraph::new(self.text.as_str());
        if self.centered {
            paragraph = paragraph.centered();
        }
        paragraph.render(area, buf);
    }
}

impl DialogComponent for Label {
    fn get_height(&self) -> u8 {
        return self.text.lines().count().clamp(1, u8::MAX as usize) as u8;
    }
}

/// A centered web link, clickable in terminals that support OSC 8 hyperlinks.
pub struct Link {
    url: String,
}

impl Link {
    pub fn new(url: impl Into<String>) -> Self {
        return Self { url: url.into() };
    }
}

impl MtrakWidget for Link {
    fn render(&mut self, _state: &AppState, area: Rect, buf: &mut Buffer) {
        let chars: Vec<char> = self.url.chars().collect();
        let width = (chars.len() as u16).min(area.width);
        let x = area.x + (area.width - width) / 2;
        // two characters per cell, as in ratatui's hyperlink example
        for (i, pair) in chars[..width as usize].chunks(2).enumerate() {
            let text: String = pair.iter().collect();
            let cell = &mut buf[(x + i as u16 * 2, area.y)];
            cell.set_symbol(&format!("\x1b]8;;{}\x07{}\x1b]8;;\x07", self.url, text));
            cell.set_style(Style::new().add_modifier(Modifier::UNDERLINED));
        }
    }
}

impl DialogComponent for Link {}

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
        let mut spans = label_spans(state, &self.label, self.focused).to_vec();
        spans.push(Span::raw(self.text.as_str()));
        if self.focused {
            spans.push(Span::styled(" ", state.theme.selection));
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

/// A hex number input that Up and Down step within its range.
pub struct NumberBox {
    id: &'static str,
    label: String,
    digits: usize,
    min: i32,
    max: i32,
    negative: bool,
    text: String,
    focused: bool,
    keys: Keymap<NumberBoxAction>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NumberBoxAction {
    Increment,
    Decrement,
}

impl NumberBox {
    pub fn new(id: &'static str, label: impl Into<String>, digits: usize) -> Self {
        let digits = digits.clamp(1, 7);
        let mut number = Self {
            id,
            label: label.into(),
            digits,
            min: 0,
            max: (1 << (4 * digits)) - 1,
            negative: false,
            text: String::new(),
            focused: false,
            keys: Keymap::new()
                .bind(Key::plain(KeyCode::Up), NumberBoxAction::Increment, "+/-")
                .bind(Key::plain(KeyCode::Down), NumberBoxAction::Decrement, "+/-"),
        };
        number.set(0);
        return number;
    }

    pub fn range(mut self, min: i32, max: i32) -> Self {
        self.min = min;
        self.max = max.max(min);
        let value = self.number();
        self.set(value);
        return self;
    }

    pub fn value(mut self, value: i32) -> Self {
        self.set(value);
        return self;
    }

    /// The typed value, kept within the range.
    fn number(&self) -> i32 {
        let magnitude = i32::from_str_radix(&self.text, 16).unwrap_or(0);
        let value = if self.negative { -magnitude } else { magnitude };
        return value.clamp(self.min, self.max);
    }

    fn set(&mut self, value: i32) {
        let value = value.clamp(self.min, self.max);
        self.negative = value < 0;
        self.text = format!("{:0width$X}", value.unsigned_abs(), width = self.digits);
    }

    /// Arrows showing which ways Up and Down can still move the value.
    fn arrows(&self) -> &'static str {
        let value = self.number();
        return match (value < self.max, value > self.min) {
            (true, true) => glyphs::pick("\u{2195}", "^v"),
            (true, false) => glyphs::pick("\u{25B2}", "^ "),
            (false, true) => glyphs::pick("\u{25BC}", " v"),
            (false, false) => glyphs::pick(" ", "  "),
        };
    }

    /// Applies a typed key, returning whether it was one the box takes.
    fn type_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char(c) if c.is_ascii_hexdigit() => {
                if self.text.len() >= self.digits {
                    self.text.remove(0);
                }
                self.text.push(c.to_ascii_uppercase());
            }
            KeyCode::Char('-') if self.min < 0 => self.negative = !self.negative,
            KeyCode::Backspace => {
                self.text.pop();
            }
            _ => return false,
        }
        return true;
    }
}

impl KeyActions for NumberBox {
    type Action = NumberBoxAction;

    fn keymap(&self) -> &Keymap<NumberBoxAction> {
        return &self.keys;
    }

    fn perform(&mut self, action: NumberBoxAction, _state: &mut AppState) -> Result<()> {
        let value = self.number();
        self.set(match action {
            NumberBoxAction::Increment => value.saturating_add(1),
            NumberBoxAction::Decrement => value.saturating_sub(1),
        });
        return Ok(());
    }

    fn describe(action: NumberBoxAction) -> &'static str {
        return match action {
            NumberBoxAction::Increment => "Increase a number field",
            NumberBoxAction::Decrement => "Decrease a number field",
        };
    }
}

impl MtrakWidget for NumberBox {
    fn handle_event(&mut self, state: &mut AppState, event: &Event) -> bool {
        if self.dispatch(state, event) {
            return true;
        }
        let Event::Key(key) = event else {
            return false;
        };
        if key.kind == KeyEventKind::Release
            || key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            return false;
        }
        return self.type_key(key.code);
    }

    fn key_hints(&self, _state: &AppState, hints: &mut Vec<KeyHint>) {
        self.keys.hints(hints);
    }

    fn render(&mut self, state: &AppState, area: Rect, buf: &mut Buffer) {
        let mut spans = label_spans(state, &self.label, self.focused).to_vec();
        let sign = match (self.negative, self.min < 0) {
            (true, _) => "-",
            (false, true) => "+",
            (false, false) => "",
        };
        let style = if self.focused {
            state.theme.selection
        } else {
            Style::new()
        };
        spans.push(Span::styled(format!("{sign}{}", self.text), style));
        spans.push(Span::raw(format!(" {}", self.arrows())));
        Line::from(spans).render(area, buf);
    }
}

impl DialogComponent for NumberBox {
    fn focusable(&self) -> bool {
        return true;
    }
    fn set_focused(&mut self, focused: bool) {
        if !focused {
            let value = self.number();
            self.set(value);
        }
        self.focused = focused;
    }
    fn value(&self) -> Option<(&'static str, DialogValue)> {
        return Some((self.id, DialogValue::Number(self.number())));
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
            state.theme.selection
        } else {
            Style::new()
        };
        let [label, separator] = label_spans(state, &self.label, self.focused);
        Line::from(vec![
            label,
            separator,
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

/// A dropdown that lists its options on top of everything else when focused.
pub struct Select {
    id: &'static str,
    label: Option<String>,
    options: Vec<String>,
    selected: usize,
    offset: usize,
    max_visible: u8,
    focused: bool,
    area: Rect,
    keys: Keymap<SelectAction>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SelectAction {
    Up,
    Down,
}

impl Select {
    pub fn new(id: &'static str, options: Vec<impl Into<String>>) -> Self {
        return Self {
            id,
            label: None,
            options: options.into_iter().map(|s| s.into()).collect(),
            selected: 0,
            offset: 0,
            max_visible: 8,
            focused: false,
            area: Rect::default(),
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

    /// Where the open list goes: below the box, else above, shrinking to fit.
    fn list_area(&self, bounds: Rect) -> Rect {
        let rows = self.options.len().min(self.max_visible as usize) as u16;
        let wanted = rows + 2;
        let below = self.area.y + 1;
        let room_below = bounds.bottom().saturating_sub(below);
        let room_above = self.area.y.saturating_sub(bounds.y);
        let (y, height) = if wanted <= room_below || room_below >= room_above {
            (below, wanted.min(room_below))
        } else {
            let height = wanted.min(room_above);
            (self.area.y - height, height)
        };
        let x = self.area.x.saturating_sub(1).max(bounds.x);
        let width = (self.area.width + 2).min(bounds.right() - x);
        return Rect {
            x,
            y,
            width,
            height,
        };
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
                let [label_area, rest] = Layout::horizontal([
                    Constraint::Length(label.chars().count() as u16 + 2),
                    Constraint::Fill(1),
                ])
                .areas(area);
                Line::from(label_spans(state, label, self.focused).to_vec())
                    .render(label_area, buf);
                rest
            }
            None => area,
        };
        self.area = area;
        let style = if self.focused {
            state.theme.selection
        } else {
            state.theme.highlight
        };
        let text = self
            .options
            .get(self.selected)
            .map_or("(none)", |s| s.as_str());
        let [text_area, arrow_area] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Length(2)]).areas(area);
        buf.set_style(area, style);
        Line::from(format!(" {text}")).render(text_area, buf);
        let arrow = if self.focused { "▴" } else { "▾" };
        Line::from(arrow).render(arrow_area, buf);
    }
}

impl DialogComponent for Select {
    fn render_overlay(&mut self, state: &AppState, bounds: Rect, buf: &mut Buffer) {
        if !self.focused || self.options.is_empty() {
            return;
        }
        let popup = self.list_area(bounds);
        // borders and at least one option
        if popup.height < 3 || popup.width < 3 {
            return;
        }
        let block = Block::bordered().border_style(state.theme.border);
        let list_area = block.inner(popup);
        Clear.render(popup, buf);
        buf.set_style(popup, state.theme.background);
        block.render(popup, buf);

        // scroll just enough to keep the selection in view
        let visible = list_area.height as usize;
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + visible {
            self.offset = self.selected + 1 - visible;
        }
        for (row, index) in (self.offset..self.options.len()).take(visible).enumerate() {
            let style = if index == self.selected {
                state.theme.selection
            } else {
                Style::new()
            };
            let line_area = Rect {
                y: list_area.y + row as u16,
                height: 1,
                ..list_area
            };
            buf.set_style(line_area, style);
            Line::from(format!(" {}", self.options[index])).render(line_area, buf);
        }
        if self.offset > 0 {
            buf.set_string(popup.right() - 2, popup.y, "▴", state.theme.border);
        }
        if self.offset + visible < self.options.len() {
            buf.set_string(
                popup.right() - 2,
                popup.bottom() - 1,
                "▾",
                state.theme.border,
            );
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn select_at(y: u16, options: usize) -> Select {
        let mut select = Select::new("", (0..options).map(|i| i.to_string()).collect());
        select.area = Rect::new(10, y, 20, 1);
        return select;
    }

    #[test]
    fn list_opens_below_with_up_to_max_visible_options() {
        let list = select_at(5, 20).list_area(Rect::new(0, 0, 80, 40));
        assert_eq!((list.y, list.height), (6, 10));
    }

    #[test]
    fn list_opens_above_when_it_only_fits_there() {
        let list = select_at(30, 20).list_area(Rect::new(0, 0, 80, 35));
        assert_eq!((list.y, list.height), (20, 10));
    }

    #[test]
    fn list_shrinks_on_the_roomier_side_of_a_small_screen() {
        let below = select_at(3, 20).list_area(Rect::new(0, 0, 80, 12));
        assert_eq!((below.y, below.height), (4, 8));
        let above = select_at(8, 20).list_area(Rect::new(0, 0, 80, 12));
        assert_eq!((above.y, above.height), (0, 8));
    }

    fn focused(dialog: &Dialog) -> Option<usize> {
        return dialog.focus;
    }

    #[test]
    fn tab_cycles_through_fields_and_back_to_none() {
        let mut dialog = Dialog::new(
            "",
            vec![
                Label::new("").boxed(),
                Checkbox::new("a", "A").boxed(),
                Checkbox::new("b", "B").boxed(),
            ],
        );
        assert_eq!(focused(&dialog), None);
        dialog.move_focus(true);
        assert_eq!(focused(&dialog), Some(1));
        dialog.move_focus(true);
        assert_eq!(focused(&dialog), Some(2));
        dialog.move_focus(true);
        assert_eq!(focused(&dialog), None);
        dialog.move_focus(false);
        assert_eq!(focused(&dialog), Some(2));
    }

    #[test]
    fn focus_first_starts_on_the_first_field() {
        let dialog = Dialog::text_field("", "", "name");
        assert_eq!(focused(&dialog), Some(0));
    }

    fn number_box(digits: usize) -> NumberBox {
        let mut number = NumberBox::new("n", "", digits);
        number.focused = true;
        return number;
    }

    fn type_keys(number: &mut NumberBox, keys: &str) {
        for c in keys.chars() {
            number.type_key(KeyCode::Char(c));
        }
    }

    #[test]
    fn number_box_typing_shifts_in_hex_digits() {
        let mut number = number_box(2).value(0xFF);
        type_keys(&mut number, "4a");
        assert_eq!(number.number(), 0x4A);
        type_keys(&mut number, "g7");
        assert_eq!(number.number(), 0xA7);
    }

    #[test]
    fn number_box_keeps_the_value_in_range() {
        let number = number_box(2).range(0, 0x7F).value(0xFF);
        assert_eq!(number.number(), 0x7F);
        let mut number = number_box(2).range(-0x80, 0x7F).value(-3);
        assert_eq!(number.text, "03");
        type_keys(&mut number, "-");
        assert_eq!(number.number(), 3);
        type_keys(&mut number, "-9F");
        assert_eq!(number.number(), -0x80);
    }

    #[test]
    fn number_box_steps_within_its_range() {
        let mut number = number_box(2).range(-2, 2).value(1);
        number.set(number.number() + 1);
        number.set(number.number() + 1);
        assert_eq!(number.number(), 2);
        number.set(-5);
        assert_eq!(number.number(), -2);
        assert_eq!(number.text, "02");
        assert!(number.negative);
    }

    #[test]
    fn number_box_arrows_show_which_ways_it_can_move() {
        let number = number_box(2).range(0, 2);
        assert_eq!(number.clone_with(0).arrows(), "\u{25B2}");
        assert_eq!(number.clone_with(1).arrows(), "\u{2195}");
        assert_eq!(number.clone_with(2).arrows(), "\u{25BC}");
    }

    impl NumberBox {
        fn clone_with(&self, value: i32) -> NumberBox {
            return NumberBox::new(self.id, "", self.digits)
                .range(self.min, self.max)
                .value(value);
        }
    }
}
