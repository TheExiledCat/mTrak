use std::{collections::HashMap, ops::Deref};

use ratatui::{
    buffer::Buffer,
    crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers},
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{
        Block, Clear, Padding, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
        StatefulWidget, Widget,
    },
};

use crate::{
    data::config::Config,
    error::Result,
    tui::{
        app::AppState,
        framework::{
            core_extensions::Boxed,
            keymap::{Action, HelpSection, Key, KeyActions, KeyHint, Keymap},
            widget::MtrakWidget,
            widgets::footer::Footer,
        },
        glyphs,
    },
};

mod color_picker;
mod list;

pub use color_picker::ColorPicker;
pub use list::{List, ListEvent};

/// The value a single input component contributes to a `DialogResult`.
#[derive(Clone, Debug, PartialEq)]
pub enum DialogValue {
    Text(String),
    Bool(bool),
    Choice(usize),
    Checks(Vec<bool>),
    Number(i32),
    Color(Color),
    /// The last button pressed in a row of buttons, and how many presses there have been.
    Press {
        button: usize,
        count: u32,
    },
    /// What was last done in a list, and how many times something has been.
    List {
        event: Option<ListEvent>,
        count: u32,
    },
}

/// The values of every input component, keyed by component id.
#[derive(PartialEq)]
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

    /// The button of a row of buttons that was pressed last, if any was.
    pub fn checks(&self, id: &str) -> Option<Vec<bool>> {
        return match self.get(id)? {
            DialogValue::Checks(checks) => Some(checks.clone()),
            _ => None,
        };
    }

    pub fn pressed(&self, id: &str) -> Option<usize> {
        return match self.values.get(id) {
            Some(DialogValue::Press { button, count }) if *count > 0 => Some(*button),
            _ => None,
        };
    }

    pub fn list_event(&self, id: &str) -> Option<ListEvent> {
        return match self.values.get(id) {
            Some(DialogValue::List { event, count }) if *count > 0 => *event,
            _ => None,
        };
    }

    pub fn color(&self, id: &str) -> Option<Color> {
        return match self.values.get(id) {
            Some(DialogValue::Color(c)) => Some(*c),
            _ => None,
        };
    }
}

/// The fields of an open dialog, handed to `on_update` after one of them changed. It reads
/// like a `DialogResult` and can also set fields, so one field can follow another.
pub struct DialogFields<'a> {
    components: &'a mut [Box<dyn DialogComponent>],
    result: DialogResult,
    changed: &'static str,
}

impl DialogFields<'_> {
    /// The id of the field whose change triggered this update.
    pub fn changed(&self) -> &'static str {
        return self.changed;
    }

    /// Sets the field with this id, if it takes that kind of value. Doesn't trigger another
    /// update.
    pub fn set(&mut self, id: &str, value: DialogValue) {
        let Some(component) = self
            .components
            .iter_mut()
            .find(|c| c.value().is_some_and(|(cid, _)| cid == id))
        else {
            return;
        };
        component.set_value(value);
        if let Some((id, value)) = component.value() {
            self.result.values.insert(id, value);
        }
    }
}

impl Deref for DialogFields<'_> {
    type Target = DialogResult;

    fn deref(&self) -> &DialogResult {
        return &self.result;
    }
}

#[allow(unused_variables)]
pub trait DialogComponent: MtrakWidget {
    fn get_height(&self) -> u8 {
        return 1;
    }
    /// How many levels the component is indented by, see `Indent::indent`.
    fn indent_level(&self) -> u8 {
        return 0;
    }
    /// Lets the component handle keys like Enter before the dialog does.
    fn takes_keys_first(&self) -> bool {
        return false;
    }
    /// Hides the dialog's own key hints, e.g. while capturing a key.
    fn takes_all_keys(&self) -> bool {
        return false;
    }
    /// Whether Tab can move focus onto this component. Static content should return false.
    fn focusable(&self) -> bool {
        return false;
    }
    fn set_focused(&mut self, focused: bool) {}
    /// How many fields Tab moves through inside this component before going on to the next
    /// one; 0 for a component that is a single field.
    fn subcomponents(&self) -> usize {
        return 0;
    }
    /// Focuses one of the fields counted by `subcomponents`, while the component has focus.
    fn focus_subcomponent(&mut self, index: usize) {}
    /// Draws on top of the whole dialog, e.g. an open dropdown list.
    fn render_overlay(&mut self, state: &AppState, bounds: Rect, buf: &mut Buffer) {}
    /// The id and current value of an input component; `None` for static content.
    fn value(&self) -> Option<(&'static str, DialogValue)> {
        return None;
    }
    /// Replaces the value of an input component; values of the wrong kind are ignored.
    fn set_value(&mut self, value: DialogValue) {}
}

type OnSubmit = Box<dyn FnOnce(&mut AppState, DialogResult) -> Result<()>>;
type OnCancel = Box<dyn FnOnce(&mut AppState)>;
type OnUpdate = Box<dyn FnMut(&mut AppState, &mut DialogFields) -> Result<()>>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DialogAction {
    Submit,
    Cancel,
    NextField,
    PrevField,
}

impl Action for DialogAction {}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum OnEsc {
    Cancel,
    Nothing,
    /// Closes it like Enter, for a dialog whose changes are made as they happen.
    Close,
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
/// `on_update` runs whenever a value changes while it's open, e.g. to preview a choice.
pub struct Dialog {
    title: String,
    width: u16,
    level: InfoLevel,
    on_esc: OnEsc,
    components: Vec<Box<dyn DialogComponent>>,
    focus: Option<usize>,
    /// The focused field inside the focused component, for one with subcomponents.
    sub_focus: usize,
    on_submit: Option<OnSubmit>,
    on_cancel: Option<OnCancel>,
    on_update: Option<OnUpdate>,
    closed: bool,
    /// Draws only the frame and title while another dialog is open on top.
    hides_behind_child: bool,
    keys: Keymap<DialogAction>,
    footer: Footer,
}

impl Dialog {
    pub fn new(title: impl Into<String>, components: Vec<Box<dyn DialogComponent>>) -> Self {
        let dialog = Self {
            title: title.into(),
            width: 50,
            level: InfoLevel::Info,
            on_esc: OnEsc::Cancel,
            components,
            focus: None,
            sub_focus: 0,
            on_submit: None,
            on_cancel: None,
            on_update: None,
            closed: false,
            hides_behind_child: false,
            footer: Footer::new(),
            keys: Keymap::new("dialog")
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
        dialog.keys = Keymap::new("dialog")
            .bind(Key::plain(KeyCode::Enter), DialogAction::Submit, "Confirm")
            .bind(Key::plain(KeyCode::Esc), DialogAction::Cancel, "Cancel");
        dialog.level = level;
        return dialog;
    }
    pub fn popup(level: InfoLevel, title: impl Into<String>, text: impl Into<String>) -> Self {
        let mut dialog = Dialog::new(title, vec![Label::new(text).boxed()]).cancellable(false);
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
    /// While a dialog opened from this one is on top, draws only this one's frame and title,
    /// so the two don't run into each other.
    pub fn hide_behind_child(mut self) -> Self {
        self.hides_behind_child = true;
        return self;
    }

    /// Draws the dialog while another one is open on top of it.
    pub fn render_covered(&mut self, state: &AppState, area: Rect, buf: &mut Buffer) {
        if self.hides_behind_child {
            self.render_frame(state, area, buf);
        } else {
            self.render(state, area, buf);
        }
    }

    /// Opens with the first focusable component focused.
    pub fn focus_first(mut self) -> Self {
        if self.focus.is_none() {
            self.move_focus(true);
        }
        return self;
    }

    pub fn cancellable(mut self, is_cancellable: bool) -> Self {
        self.on_esc = match is_cancellable {
            true => OnEsc::Cancel,
            false => OnEsc::Nothing,
        };
        return self;
    }

    /// Has Esc close the dialog like Enter, running `on_submit`, for a dialog whose changes
    /// are made as they happen so there's nothing to cancel.
    pub fn close_on_esc(mut self) -> Self {
        self.on_esc = OnEsc::Close;
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

    /// Runs every time the value of an input component changes, receiving the fields to read
    /// or set.
    pub fn on_update(
        mut self,
        f: impl FnMut(&mut AppState, &mut DialogFields) -> Result<()> + 'static,
    ) -> Self {
        self.on_update = Some(Box::new(f));
        return self;
    }

    pub fn help(sections: &mut Vec<HelpSection>) {
        let dialog = Dialog::new("", Vec::new());
        let select = Select::new("", Vec::<String>::new());
        let checkbox = Checkbox::new("", "");
        let buttons = Buttons::new("", Vec::<String>::new());
        let number = NumberBox::new("", "", 2).steps();
        sections.push(
            dialog
                .help_section("Dialogs")
                .entries(select.keys.help_entries(Select::describe))
                .entries(open_list_keys(Keymap::new("dialog")).help_entries(Select::describe))
                .entries(toggle_option_key(Keymap::new("dialog")).help_entries(Select::describe))
                .entries(checkbox.keys.help_entries(Checkbox::describe))
                .entries(buttons.keys.help_entries(Buttons::describe))
                .entries(number.keys.help_entries(NumberBox::describe))
                .entries(List::help_entries())
                .entry(
                    "0-9 / A-F",
                    "Pick the dropdown option at that number, from 0",
                )
                .entry("Bksp", "Delete the last character of a text field"),
        );
    }

    /// Whether the focused field wants every key, like one waiting for a key to bind.
    pub fn takes_all_keys(&self) -> bool {
        return self
            .focus
            .is_some_and(|index| self.components[index].takes_all_keys());
    }

    pub fn is_closed(&self) -> bool {
        return self.closed;
    }

    fn result(&self) -> DialogResult {
        let values = self.components.iter().filter_map(|c| c.value()).collect();
        return DialogResult { values };
    }

    fn submit(&mut self, state: &mut AppState) -> Result<()> {
        self.closed = true;
        let Some(f) = self.on_submit.take() else {
            return Ok(());
        };
        return f(state, self.result());
    }

    fn cancel(&mut self, state: &mut AppState) {
        self.closed = true;
        if let Some(f) = self.on_cancel.take() {
            f(state);
        }
    }

    /// Moves focus to the next or previous field, going through the subcomponents of a
    /// component before leaving it, and wrapping round through none.
    fn move_focus(&mut self, forward: bool) {
        if let Some(focus) = self.focus {
            let count = self.components[focus].subcomponents();
            let next = if forward {
                Some(self.sub_focus + 1).filter(|&sub| sub < count)
            } else {
                self.sub_focus.checked_sub(1).filter(|_| count > 0)
            };
            if let Some(sub) = next {
                self.sub_focus = sub;
                self.components[focus].focus_subcomponent(sub);
                return;
            }
        }
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
            let count = self.components[new].subcomponents();
            self.sub_focus = if forward { 0 } else { count.saturating_sub(1) };
            if count > 0 {
                self.components[new].focus_subcomponent(self.sub_focus);
            }
        }
        self.focus = next;
    }

    /// The lines in front of an indented component, as in a folder tree: a branch on each of
    /// its rows, ending in a corner unless another component follows at its level, and the
    /// lines of outer levels passing by.
    fn render_tree_lines(&self, state: &AppState, index: usize, area: Rect, buf: &mut Buffer) {
        let level = self.components[index].indent_level();
        for depth in 1..=level {
            let x = area.x + (depth as u16 - 1) * INDENT_WIDTH;
            if x + INDENT_WIDTH > area.right() {
                return;
            }
            let continues = self.has_later_sibling(index, depth);
            for y in area.top()..area.bottom() {
                let last_row = y + 1 == area.bottom();
                let symbol = match (depth == level, continues || !last_row) {
                    (true, true) => glyphs::pick("\u{251C}\u{2500}", "|-"),
                    (true, false) => glyphs::pick("\u{2514}\u{2500}", "`-"),
                    (false, true) => glyphs::pick("\u{2502}", "|"),
                    (false, false) => " ",
                };
                buf.set_string(x, y, symbol, state.theme.border);
            }
        }
    }

    /// Whether a component at `depth` comes after `index` before the tree steps back out of it.
    fn has_later_sibling(&self, index: usize, depth: u8) -> bool {
        for component in &self.components[index + 1..] {
            let level = component.indent_level();
            if level < depth {
                return false;
            }
            if level == depth {
                return true;
            }
        }
        return false;
    }

    fn content_height(&self) -> u16 {
        return self.components.iter().map(|c| c.get_height() as u16).sum();
    }

    /// Clears the dialog's area and draws its border and title, returning the area inside.
    fn render_frame(&self, state: &AppState, area: Rect, buf: &mut Buffer) -> Rect {
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
        return inner;
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
            DialogAction::Cancel => match self.on_esc {
                OnEsc::Cancel => self.cancel(state),
                OnEsc::Nothing => {}
                OnEsc::Close => self.submit(state)?,
            },
            DialogAction::NextField => self.move_focus(true),
            DialogAction::PrevField => self.move_focus(false),
        }
        return Ok(());
    }

    fn describe(action: DialogAction) -> &'static str {
        return match action {
            DialogAction::Submit => "Confirm",
            DialogAction::Cancel => "Cancel, or close a dialog whose changes are already made",
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
        if let Some(index) = self.focus
            && self.components[index].takes_keys_first()
        {
            if !self.components[index].handle_event(state, event) {
                self.dispatch(state, event);
            }
            return true;
        }
        if self.dispatch(state, event) {
            return true;
        }
        let Some(index) = self.focus else {
            return true;
        };
        let before = self.components[index].value();
        self.components[index].handle_event(state, event);
        let after = self.components[index].value();
        if after == before {
            return true;
        }
        let Some((changed, _)) = after else {
            return true;
        };
        let result = self.result();
        if let Some(on_update) = self.on_update.as_mut() {
            let mut fields = DialogFields {
                components: &mut self.components,
                result,
                changed,
            };
            if let Err(e) = on_update(state, &mut fields) {
                state.report_error(e);
            }
        }
        return true;
    }

    fn key_hints(&self, state: &AppState, hints: &mut Vec<KeyHint>) {
        if let Some(index) = self.focus {
            self.components[index].key_hints(state, hints);
        }
        if self
            .focus
            .is_some_and(|index| self.components[index].takes_all_keys())
        {
            return;
        }
        let component_hints = hints.len();
        self.keys.hints(hints);
        if let Some(index) = self.focus
            && self.components[index].takes_keys_first()
        {
            let taken: Vec<String> = hints[..component_hints]
                .iter()
                .map(|h| h.key.clone())
                .collect();
            let mut position = 0;
            hints.retain(|h| {
                position += 1;
                return position <= component_hints || !taken.contains(&h.key);
            });
        }
        match self.on_esc {
            OnEsc::Cancel => {}
            OnEsc::Nothing => hints.retain(|h| h.label != "Cancel"),
            OnEsc::Close => {
                hints.retain(|h| h.label != "OK");
                for hint in hints.iter_mut().filter(|h| h.label == "Cancel") {
                    hint.label = "Close";
                }
            }
        }
        if !self.components.iter().any(|c| c.focusable()) {
            hints.retain(|h| h.label != "Field");
        }
    }

    fn render(&mut self, state: &AppState, area: Rect, buf: &mut Buffer) {
        let inner = self.render_frame(state, area, buf);

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
        for (index, row) in rows.iter().enumerate() {
            self.render_tree_lines(state, index, *row, buf);
        }
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

/// How many cells one level of `Indent::indent` moves a component in, room for a tree line
/// like `├─ `.
const INDENT_WIDTH: u16 = 3;

/// Draws any component further in, e.g. fields that belong to the one above them.
pub trait Indent: DialogComponent + Sized {
    fn indent(self, levels: u8) -> Indented<Self> {
        return Indented {
            inner: self,
            levels,
        };
    }
}

impl<T: DialogComponent> Indent for T {}

pub struct Indented<T> {
    inner: T,
    levels: u8,
}

impl<T: DialogComponent> MtrakWidget for Indented<T> {
    fn update(&mut self, state: &mut AppState) {
        self.inner.update(state);
    }
    fn handle_event(&mut self, state: &mut AppState, event: &Event) -> bool {
        return self.inner.handle_event(state, event);
    }
    fn key_hints(&self, state: &AppState, hints: &mut Vec<KeyHint>) {
        self.inner.key_hints(state, hints);
    }
    fn help(&self, sections: &mut Vec<HelpSection>) {
        self.inner.help(sections);
    }
    fn render(&mut self, state: &AppState, area: Rect, buf: &mut Buffer) {
        let indent = (self.levels as u16 * INDENT_WIDTH).min(area.width);
        let area = Rect {
            x: area.x + indent,
            width: area.width - indent,
            ..area
        };
        self.inner.render(state, area, buf);
    }
}

impl<T: DialogComponent> DialogComponent for Indented<T> {
    fn get_height(&self) -> u8 {
        return self.inner.get_height();
    }
    fn indent_level(&self) -> u8 {
        return self.levels;
    }
    fn focusable(&self) -> bool {
        return self.inner.focusable();
    }
    fn set_focused(&mut self, focused: bool) {
        self.inner.set_focused(focused);
    }
    fn subcomponents(&self) -> usize {
        return self.inner.subcomponents();
    }
    fn focus_subcomponent(&mut self, index: usize) {
        self.inner.focus_subcomponent(index);
    }
    fn render_overlay(&mut self, state: &AppState, bounds: Rect, buf: &mut Buffer) {
        self.inner.render_overlay(state, bounds, buf);
    }
    fn value(&self) -> Option<(&'static str, DialogValue)> {
        return self.inner.value();
    }
    fn set_value(&mut self, value: DialogValue) {
        self.inner.set_value(value);
    }
}

/// A component's label followed by its separator, marked while the component has focus.
fn label_spans<'a>(state: &AppState, label: &'a str, focused: bool) -> [Span<'a>; 2] {
    if label.is_empty() {
        return [Span::raw(""), Span::raw("")];
    }
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
    fn render(&mut self, state: &AppState, area: Rect, buf: &mut Buffer) {
        let chars: Vec<char> = self.url.chars().collect();
        let width = (chars.len() as u16).min(area.width);
        let x = area.x + (area.width - width) / 2;
        // two characters per cell, as in ratatui's hyperlink example
        for (i, pair) in chars[..width as usize].chunks(2).enumerate() {
            let text: String = pair.iter().collect();
            let cell = &mut buf[(x + i as u16 * 2, area.y)];
            cell.set_symbol(&format!("\x1b]8;;{}\x07{}\x1b]8;;\x07", self.url, text));
            cell.set_style(state.theme.link);
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
    allowed: fn(char) -> bool,
}

impl TextPrompt {
    pub fn new(id: &'static str, label: impl Into<String>) -> Self {
        return Self {
            id,
            label: label.into(),
            text: String::new(),
            focused: false,
            allowed: |_| true,
        };
    }

    pub fn initial(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        return self;
    }

    /// Only takes the characters `allowed` accepts.
    pub fn allowed(mut self, allowed: fn(char) -> bool) -> Self {
        self.allowed = allowed;
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
                if (self.allowed)(c)
                    && !key
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
    fn set_value(&mut self, value: DialogValue) {
        if let DialogValue::Text(text) = value {
            self.text = text;
        }
    }
}

/// A number input that Up and Down step within its range. It's in hex or decimal as the
/// config's `hex_values` says, switching along with it.
pub struct NumberBox {
    id: &'static str,
    label: String,
    /// How many digits the value has in hex.
    digits: usize,
    min: i32,
    max: i32,
    negative: bool,
    /// The digits of the value without its sign, in hex or decimal as `hex` says.
    text: String,
    hex: bool,
    show_range: bool,
    value_label: Option<Box<dyn Fn(i32) -> String>>,
    focused: bool,
    keys: Keymap<NumberBoxAction>,
}

/// How far Left and Right move a number box with `steps`, in hex and in decimal.
const BIG_STEP_HEX: i32 = 0x10;
const BIG_STEP_DECIMAL: i32 = 10;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NumberBoxAction {
    Increment,
    Decrement,
    StepUp,
    StepDown,
}

impl Action for NumberBoxAction {}

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
            hex: true,
            show_range: false,
            value_label: None,
            focused: false,
            keys: Keymap::new("dialog")
                .bind(Key::plain(KeyCode::Up), NumberBoxAction::Increment, "+/-")
                .bind(Key::plain(KeyCode::Down), NumberBoxAction::Decrement, "+/-")
                .bind(Key::plain(KeyCode::PageUp), NumberBoxAction::StepUp, "Step")
                .bind(
                    Key::plain(KeyCode::PageDown),
                    NumberBoxAction::StepDown,
                    "Step",
                ),
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

    /// Shows the range after the label, e.g. `Volume (00-FF)`, in hex or decimal like the
    /// value, while the config's `show_value_ranges` is on.
    pub fn show_range(mut self) -> Self {
        self.show_range = true;
        return self;
    }

    /// Shows what the value stands for after it, e.g. a CC's name.
    pub fn value_label(mut self, label: impl Fn(i32) -> String + 'static) -> Self {
        self.value_label = Some(Box::new(label));
        return self;
    }

    /// Lets Left and Right move the value in bigger steps too, like PgUp and PgDn.
    pub fn steps(mut self) -> Self {
        self.keys = self
            .keys
            .bind(Key::plain(KeyCode::Right), NumberBoxAction::StepUp, "Step")
            .bind(Key::plain(KeyCode::Left), NumberBoxAction::StepDown, "Step");
        return self;
    }

    /// The typed value, kept within the range.
    fn number(&self) -> i32 {
        let magnitude = i32::from_str_radix(&self.text, self.radix()).unwrap_or(0);
        let value = if self.negative { -magnitude } else { magnitude };
        return value.clamp(self.min, self.max);
    }

    fn set(&mut self, value: i32) {
        let value = value.clamp(self.min, self.max);
        let width = self.width();
        self.negative = value < 0;
        self.text = match self.hex {
            true => format!("{:0width$X}", value.unsigned_abs()),
            false => format!("{:0width$}", value.unsigned_abs()),
        };
    }

    fn radix(&self) -> u32 {
        return if self.hex { 16 } else { 10 };
    }

    /// How many digits the value is shown with: as asked for in hex, and as many as the
    /// range needs in decimal.
    fn width(&self) -> usize {
        if self.hex {
            return self.digits;
        }
        let largest = self.min.unsigned_abs().max(self.max.unsigned_abs());
        return largest.to_string().len();
    }

    /// Switches to hex or decimal if the config changed, keeping the value.
    fn follow_config(&mut self, state: &AppState) {
        if self.hex != state.config.hex_values {
            let value = self.number();
            self.hex = state.config.hex_values;
            self.set(value);
        }
    }

    /// `value` as the box shows it: signed if the range goes below 0, and in hex padded to
    /// the box's digits or in decimal.
    fn format(&self, value: i32) -> String {
        let sign = match (value < 0, self.min < 0) {
            (true, _) => "-",
            (false, true) => "+",
            (false, false) => "",
        };
        let magnitude = value.unsigned_abs();
        return match self.hex {
            true => format!("{sign}{magnitude:0width$X}", width = self.digits),
            false => format!("{sign}{magnitude}"),
        };
    }

    /// The label, with the range when `show_range` and the config's `show_value_ranges` are
    /// on. A label that ends in brackets gets the range inside them.
    fn label(&self, config: &Config) -> String {
        if !self.show_range || !config.show_value_ranges {
            return self.label.clone();
        }
        let separator = if self.min < 0 { " to " } else { "-" };
        let range = format!(
            "{}{separator}{}",
            self.format(self.min),
            self.format(self.max)
        );
        return match self.label.strip_suffix(')') {
            Some(label) => format!("{label}, {range})"),
            None => format!("{} ({range})", self.label),
        };
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
            KeyCode::Char(c) if c.is_digit(self.radix()) => {
                if self.text.len() >= self.width() {
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
        let big_step = if self.hex {
            BIG_STEP_HEX
        } else {
            BIG_STEP_DECIMAL
        };
        self.set(match action {
            NumberBoxAction::Increment => value.saturating_add(1),
            NumberBoxAction::Decrement => value.saturating_sub(1),
            NumberBoxAction::StepUp => value.saturating_add(big_step),
            NumberBoxAction::StepDown => value.saturating_sub(big_step),
        });
        return Ok(());
    }

    fn describe(action: NumberBoxAction) -> &'static str {
        return match action {
            NumberBoxAction::Increment => "Increase a number field",
            NumberBoxAction::Decrement => "Decrease a number field",
            NumberBoxAction::StepUp => "Increase a number field in bigger steps",
            NumberBoxAction::StepDown => "Decrease a number field in bigger steps",
        };
    }
}

impl MtrakWidget for NumberBox {
    fn handle_event(&mut self, state: &mut AppState, event: &Event) -> bool {
        self.follow_config(state);
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
        self.follow_config(state);
        let label = self.label(&state.config);
        let mut spans = label_spans(state, &label, self.focused).to_vec();
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
        if let Some(label) = &self.value_label {
            spans.push(Span::raw(format!(" {}", label(self.number()))));
        }
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
    fn set_value(&mut self, value: DialogValue) {
        if let DialogValue::Number(number) = value {
            self.set(number);
        }
    }
}

/// A row of buttons that Tab moves through. A press doesn't close the dialog; it shows up
/// in `on_update`, where `DialogResult::pressed` tells which button it was.
pub struct Buttons {
    id: &'static str,
    labels: Vec<String>,
    focused: Option<usize>,
    presses: u32,
    last: usize,
    keys: Keymap<ButtonsAction>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonsAction {
    Press,
}

impl Action for ButtonsAction {}

impl Buttons {
    pub fn new(id: &'static str, labels: Vec<impl Into<String>>) -> Self {
        return Self {
            id,
            labels: labels.into_iter().map(|l| l.into()).collect(),
            focused: None,
            presses: 0,
            last: 0,
            keys: Keymap::new("dialog").bind(Key::char(' '), ButtonsAction::Press, "Press"),
        };
    }
}

impl KeyActions for Buttons {
    type Action = ButtonsAction;

    fn keymap(&self) -> &Keymap<ButtonsAction> {
        return &self.keys;
    }

    fn perform(&mut self, action: ButtonsAction, _state: &mut AppState) -> Result<()> {
        match action {
            ButtonsAction::Press => {
                if let Some(button) = self.focused {
                    self.last = button;
                    self.presses += 1;
                }
            }
        }
        return Ok(());
    }

    fn describe(action: ButtonsAction) -> &'static str {
        return match action {
            ButtonsAction::Press => "Press a button",
        };
    }
}

impl MtrakWidget for Buttons {
    fn handle_event(&mut self, state: &mut AppState, event: &Event) -> bool {
        return self.dispatch(state, event);
    }

    fn key_hints(&self, _state: &AppState, hints: &mut Vec<KeyHint>) {
        self.keys.hints(hints);
    }

    fn render(&mut self, state: &AppState, area: Rect, buf: &mut Buffer) {
        let mut spans = Vec::new();
        for (index, label) in self.labels.iter().enumerate() {
            if index > 0 {
                spans.push(Span::raw("  "));
            }
            let style = if self.focused == Some(index) {
                state.theme.selection
            } else {
                state.theme.highlight
            };
            spans.push(Span::styled(format!("[ {label} ]"), style));
        }
        Line::from(spans).render(area, buf);
    }
}

impl DialogComponent for Buttons {
    fn focusable(&self) -> bool {
        return !self.labels.is_empty();
    }
    fn set_focused(&mut self, focused: bool) {
        self.focused = focused.then_some(0);
    }
    fn subcomponents(&self) -> usize {
        return self.labels.len();
    }
    fn focus_subcomponent(&mut self, index: usize) {
        self.focused = Some(index.min(self.labels.len().saturating_sub(1)));
    }
    fn value(&self) -> Option<(&'static str, DialogValue)> {
        return Some((
            self.id,
            DialogValue::Press {
                button: self.last,
                count: self.presses,
            },
        ));
    }
}

pub struct Checkbox {
    id: &'static str,
    label: String,
    checked: bool,
    focused: bool,
    keys: Keymap<CheckboxAction>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CheckboxAction {
    Toggle,
}

impl Action for CheckboxAction {}

impl Checkbox {
    pub fn new(id: &'static str, label: impl Into<String>) -> Self {
        return Self {
            id,
            label: label.into(),
            checked: false,
            focused: false,
            keys: Keymap::new("dialog").bind(Key::char(' '), CheckboxAction::Toggle, "Toggle"),
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
    fn set_value(&mut self, value: DialogValue) {
        if let DialogValue::Bool(checked) = value {
            self.checked = checked;
        }
    }
}

/// A dropdown. Up and Down change it in place; Space opens the whole list on top of the dialog.
pub struct Select {
    id: &'static str,
    label: Option<String>,
    options: Vec<String>,
    selected: usize,
    offset: usize,
    max_visible: u8,
    /// Defaults to the list's height.
    page_step: Option<usize>,
    focused: bool,
    checks: Option<Vec<bool>>,
    /// What was picked and ticked when the list opened, which Esc goes back to.
    open: Option<(usize, Option<Vec<bool>>)>,
    area: Rect,
    /// The digits typed so far, to pick options past the first digit.
    typed: String,
    typed_from: usize,
    keys: Keymap<SelectAction>,
    open_keys: Keymap<SelectAction>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SelectAction {
    Up,
    Down,
    PageUp,
    PageDown,
    OpenList,
    PickOption,
    CloseList,
    ToggleOption,
}

impl Action for SelectAction {}

impl Select {
    pub fn new(id: &'static str, options: Vec<impl Into<String>>) -> Self {
        return Self {
            id,
            label: None,
            options: options.into_iter().map(|s| s.into()).collect(),
            selected: 0,
            offset: 0,
            max_visible: 8,
            page_step: None,
            focused: false,
            checks: None,
            open: None,
            area: Rect::default(),
            typed: String::new(),
            typed_from: 0,
            keys: closed_list_keys(),
            open_keys: open_list_keys(select_keys()),
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

    pub fn page_step(mut self, options: usize) -> Self {
        self.page_step = Some(options.max(1));
        return self;
    }

    pub fn checkboxes(mut self, checks: Vec<bool>) -> Self {
        let mut checks = checks;
        checks.resize(self.options.len(), false);
        self.checks = Some(checks);
        self.keys = toggle_option_key(closed_list_keys());
        self.open_keys = toggle_option_key(open_list_keys(select_keys()));
        return self;
    }

    /// Typing a number picks the option this many places further down, for lists that start
    /// with options that aren't numbered, like an `Off`.
    pub fn typed_from(mut self, index: usize) -> Self {
        self.typed_from = index;
        return self;
    }

    fn is_open(&self) -> bool {
        return self.open.is_some();
    }

    fn option_text(&self, index: usize) -> String {
        let option = &self.options[index];
        return match &self.checks {
            Some(checks) if checks[index] => format!("[x] {option}"),
            Some(_) => format!("[ ] {option}"),
            None => option.clone(),
        };
    }

    /// The first word of every ticked option, e.g. `9, A`.
    fn ticked_summary(&self) -> Option<String> {
        let checks = self.checks.as_ref()?;
        let ticked: Vec<&str> = self
            .options
            .iter()
            .zip(checks)
            .filter(|(_, checked)| **checked)
            .map(|(option, _)| option.split_whitespace().next().unwrap_or(option))
            .collect();
        return Some(match ticked.is_empty() {
            true => "none on".to_owned(),
            false => format!("on: {}", ticked.join(", ")),
        });
    }

    fn arrows(&self) -> &'static str {
        let last = self.options.len().saturating_sub(1);
        return match (self.selected > 0, self.selected < last) {
            (true, true) => glyphs::pick("\u{2195}", "^v"),
            (false, true) => glyphs::pick("\u{25BC}", " v"),
            (true, false) => glyphs::pick("\u{25B2}", "^ "),
            (false, false) => glyphs::pick(" ", "  "),
        };
    }

    /// Picks the option for the digits typed so far, in hex or decimal as the config says. A
    /// number past the last option starts over from the digit just typed.
    fn type_digit(&mut self, c: char, hex: bool) -> bool {
        let radix = if hex { 16 } else { 10 };
        if !c.is_digit(radix) || self.options.len() <= self.typed_from {
            return false;
        }
        let last = self.options.len() - 1 - self.typed_from;
        let max_digits = if hex {
            format!("{last:X}").len()
        } else {
            last.to_string().len()
        };
        self.typed.push(c);
        let skip = self.typed.chars().count().saturating_sub(max_digits);
        self.typed = self.typed.chars().skip(skip).collect();
        while !self.typed.is_empty() {
            if let Ok(number) = usize::from_str_radix(&self.typed, radix)
                && number <= last
            {
                self.selected = number + self.typed_from;
                return true;
            }
            self.typed.remove(0);
        }
        return false;
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

fn closed_list_keys() -> Keymap<SelectAction> {
    return select_keys().bind(Key::char(' '), SelectAction::OpenList, "Open");
}

fn toggle_option_key(keys: Keymap<SelectAction>) -> Keymap<SelectAction> {
    return keys.bind(Key::char('x'), SelectAction::ToggleOption, "Toggle");
}

fn open_list_keys(keys: Keymap<SelectAction>) -> Keymap<SelectAction> {
    return keys
        .bind(Key::char(' '), SelectAction::PickOption, "Pick")
        .bind(Key::plain(KeyCode::Enter), SelectAction::PickOption, "Pick")
        .bind(Key::plain(KeyCode::Esc), SelectAction::CloseList, "Back");
}

fn select_keys() -> Keymap<SelectAction> {
    return Keymap::new("dialog")
        .bind(Key::plain(KeyCode::Up), SelectAction::Up, "Choose")
        .bind(Key::plain(KeyCode::Down), SelectAction::Down, "Choose")
        .bind(Key::plain(KeyCode::PageUp), SelectAction::PageUp, "Page")
        .bind(
            Key::plain(KeyCode::PageDown),
            SelectAction::PageDown,
            "Page",
        );
}

impl KeyActions for Select {
    type Action = SelectAction;

    fn keymap(&self) -> &Keymap<SelectAction> {
        if self.is_open() {
            return &self.open_keys;
        }
        return &self.keys;
    }

    fn perform(&mut self, action: SelectAction, _state: &mut AppState) -> Result<()> {
        self.typed.clear();
        let last = self.options.len().saturating_sub(1);
        let page = self.page_step.unwrap_or(self.max_visible as usize);
        match action {
            SelectAction::Up => self.selected = self.selected.saturating_sub(1),
            SelectAction::Down => self.selected = (self.selected + 1).min(last),
            SelectAction::PageUp => self.selected = self.selected.saturating_sub(page),
            SelectAction::PageDown => self.selected = (self.selected + page).min(last),
            SelectAction::OpenList => self.open = Some((self.selected, self.checks.clone())),
            SelectAction::PickOption => self.open = None,
            SelectAction::CloseList => {
                if let Some((selected, checks)) = self.open.take() {
                    self.selected = selected;
                    self.checks = checks;
                }
            }
            SelectAction::ToggleOption => {
                if let Some(checked) = self.checks.as_mut().and_then(|c| c.get_mut(self.selected)) {
                    *checked = !*checked;
                }
            }
        }
        return Ok(());
    }

    fn describe(action: SelectAction) -> &'static str {
        return match action {
            SelectAction::Up => "Previous option",
            SelectAction::Down => "Next option",
            SelectAction::PageUp => "Move up a page of options, or an octave of notes",
            SelectAction::PageDown => "Move down a page of options, or an octave of notes",
            SelectAction::OpenList => "Open a dropdown's list",
            SelectAction::PickOption => "Pick the highlighted option and close the list",
            SelectAction::CloseList => "Close the list without changing the option",
            SelectAction::ToggleOption => "Tick or untick the option in a dropdown with checkboxes",
        };
    }
}

impl MtrakWidget for Select {
    /// A hex digit picks the option at that index.
    fn handle_event(&mut self, state: &mut AppState, event: &Event) -> bool {
        if self.dispatch(state, event) {
            return true;
        }
        let Event::Key(key) = event else {
            return false;
        };
        let KeyCode::Char(c) = key.code else {
            return false;
        };
        if key.kind == KeyEventKind::Release
            || key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            return false;
        }
        return self.type_digit(c, state.config.hex_values);
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
        let text = match self.selected < self.options.len() {
            true => self.option_text(self.selected),
            false => "(none)".to_owned(),
        };
        let summary = self.ticked_summary().unwrap_or_default();
        let [text_area, summary_area, arrow_area] = Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Length(summary.chars().count() as u16 + 1),
            Constraint::Length(3),
        ])
        .areas(area);
        buf.set_style(area, style);
        Line::from(format!(" {text}")).render(text_area, buf);
        Line::from(summary).render(summary_area, buf);
        Line::from(self.arrows()).render(arrow_area, buf);
    }
}

impl DialogComponent for Select {
    fn render_overlay(&mut self, state: &AppState, bounds: Rect, buf: &mut Buffer) {
        if !self.focused || !self.is_open() || self.options.is_empty() {
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
        let overflows = self.options.len() > visible;
        let text_width = list_area.width - overflows as u16;
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
                width: text_width,
                height: 1,
                ..list_area
            };
            buf.set_style(line_area, style);
            Line::from(format!(" {}", self.option_text(index))).render(line_area, buf);
        }
        if overflows {
            let last_offset = self.options.len() - visible;
            let mut scroll = ScrollbarState::new(last_offset + 1).position(self.offset);
            Scrollbar::new(ScrollbarOrientation::VerticalRight).render(list_area, buf, &mut scroll);
        }
    }
    fn takes_keys_first(&self) -> bool {
        return self.is_open();
    }
    fn focusable(&self) -> bool {
        return !self.options.is_empty();
    }
    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
        self.open = None;
        self.typed.clear();
    }
    fn value(&self) -> Option<(&'static str, DialogValue)> {
        if self.options.is_empty() {
            return None;
        }
        if let Some(checks) = &self.checks {
            return Some((self.id, DialogValue::Checks(checks.clone())));
        }
        return Some((self.id, DialogValue::Choice(self.selected)));
    }
    fn set_value(&mut self, value: DialogValue) {
        match value {
            DialogValue::Choice(index) => {
                self.selected = index.min(self.options.len().saturating_sub(1));
            }
            DialogValue::Checks(mut checks) if self.checks.is_some() => {
                checks.resize(self.options.len(), false);
                self.checks = Some(checks);
            }
            _ => {}
        }
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

    #[test]
    fn esc_closes_a_close_on_esc_dialog_like_enter() {
        use std::{cell::Cell, rc::Rc};

        use ratatui::crossterm::event::KeyEvent;

        use crate::{data::project::Project, tui::theme::Theme};

        let mut state = AppState::new(Project::empty(), Theme::monochrome()).unwrap();
        let submitted = Rc::new(Cell::new(false));
        let cancelled = Rc::new(Cell::new(false));
        let mut dialog = Dialog::new("", vec![Checkbox::new("c", "C").boxed()])
            .close_on_esc()
            .on_submit({
                let submitted = submitted.clone();
                move |_, _| {
                    submitted.set(true);
                    return Ok(());
                }
            })
            .on_cancel({
                let cancelled = cancelled.clone();
                move |_| cancelled.set(true)
            });
        let mut hints = vec![];
        dialog.key_hints(&state, &mut hints);
        let labels: Vec<&str> = hints.iter().map(|hint| hint.label).collect();
        assert!(labels.contains(&"Close"), "{labels:?}");
        assert!(
            !labels.contains(&"OK") && !labels.contains(&"Cancel"),
            "{labels:?}"
        );
        let esc = Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        dialog.handle_event(&mut state, &esc);
        assert!(dialog.is_closed());
        assert!(submitted.get());
        assert!(!cancelled.get());
    }

    fn focused(dialog: &Dialog) -> Option<usize> {
        return dialog.focus;
    }

    #[test]
    fn typed_digits_add_up_to_pick_options_past_the_first_digit() {
        let mut select = Select::new("s", (0..0x81).map(|i| i.to_string()).collect());
        select.type_digit('4', true);
        select.type_digit('a', true);
        assert_eq!(select.selected, 0x4A);
        select.type_digit('8', true);
        select.type_digit('1', true);
        assert_eq!(
            select.selected, 0x01,
            "81 is past the end, so only the 1 counts"
        );
        select.type_digit('7', false);
        select.type_digit('4', false);
        assert_eq!(select.selected, 74);
        assert!(!select.type_digit('a', false));
    }

    #[test]
    fn typed_numbers_can_skip_the_first_options() {
        let mut select = Select::new("s", (0..0x81).map(|i| i.to_string()).collect()).typed_from(1);
        select.type_digit('0', true);
        assert_eq!(select.selected, 1);
        select.type_digit('7', true);
        select.type_digit('f', true);
        assert_eq!(select.selected, 0x80);
    }

    #[test]
    fn number_boxes_show_what_their_value_stands_for() {
        use crate::{data::project::Project, tui::theme::Theme};

        let state = AppState::new(Project::empty(), Theme::monochrome()).unwrap();
        let mut number = NumberBox::new("n", "N", 2)
            .value(3)
            .value_label(|value| format!("is {value}"));
        let area = Rect::new(0, 0, 30, 1);
        let mut buf = Buffer::empty(area);
        number.render(&state, area, &mut buf);
        let line: String = (0..area.width).map(|x| buf[(x, 0)].symbol()).collect();
        assert!(line.contains("03") && line.contains("is 3"), "{line:?}");
    }

    #[test]
    fn a_hex_digit_picks_that_option() {
        use ratatui::crossterm::event::KeyEvent;

        use crate::{data::project::Project, tui::theme::Theme};

        let mut state = AppState::new(Project::empty(), Theme::monochrome()).unwrap();
        let mut select = Select::new("s", (0..12).map(|i| i.to_string()).collect());
        let mut press = |select: &mut Select, c| {
            let event = Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
            return select.handle_event(&mut state, &event);
        };
        assert!(press(&mut select, '5'));
        assert_eq!(select.selected, 5);
        assert!(press(&mut select, 'b'));
        assert_eq!(select.selected, 11);
        assert!(!press(&mut select, 'c'));
        assert_eq!(select.selected, 11);
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
    fn tree_lines_end_where_a_level_does() {
        let dialog = Dialog::new(
            "",
            vec![
                Label::new("").boxed(),
                Label::new("").indent(1).boxed(),
                Label::new("").indent(2).boxed(),
                Label::new("").indent(1).boxed(),
                Label::new("").boxed(),
            ],
        );
        assert!(dialog.has_later_sibling(1, 1));
        assert!(!dialog.has_later_sibling(2, 2));
        assert!(!dialog.has_later_sibling(3, 1));
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

    fn state_with_hex_values(hex: bool) -> AppState {
        use crate::{data::project::Project, tui::theme::Theme};

        let mut state = AppState::new(Project::empty(), Theme::monochrome()).unwrap();
        state.config.hex_values = hex;
        return state;
    }

    #[test]
    fn number_box_follows_the_config_keeping_its_value() {
        let mut number = number_box(2).range(-0x80, 0x7F).value(-0x1A);
        number.follow_config(&state_with_hex_values(false));
        assert_eq!((number.text.as_str(), number.negative), ("026", true));
        assert_eq!(number.number(), -26);
        number.follow_config(&state_with_hex_values(true));
        assert_eq!(number.text, "1A");
    }

    #[test]
    fn decimal_number_boxes_take_decimal_digits_as_wide_as_the_range() {
        let mut number = number_box(2).value(0x10);
        number.follow_config(&state_with_hex_values(false));
        assert_eq!(number.text, "016");
        type_keys(&mut number, "2a");
        assert_eq!(number.number(), 162);
        type_keys(&mut number, "99");
        assert_eq!(number.number(), 255, "kept in range");
        let mut small = number_box(1).range(0, 7);
        small.follow_config(&state_with_hex_values(false));
        type_keys(&mut small, "5");
        assert_eq!(small.text, "5");
    }

    #[test]
    fn number_box_big_steps_are_round_in_either_base() {
        let mut state = state_with_hex_values(true);
        let mut number = number_box(2).steps().value(0);
        number.follow_config(&state);
        number.perform(NumberBoxAction::StepUp, &mut state).unwrap();
        assert_eq!(number.number(), 0x10);
        let mut state = state_with_hex_values(false);
        number.follow_config(&state);
        number.perform(NumberBoxAction::StepUp, &mut state).unwrap();
        assert_eq!(number.number(), 26);
    }

    fn ranges_shown() -> Config {
        return Config {
            show_value_ranges: true,
            ..Config::default()
        };
    }

    #[test]
    fn number_box_labels_show_the_range_when_asked() {
        let config = ranges_shown();
        let volume = NumberBox::new("v", "Volume", 2);
        assert_eq!(volume.label(&config), "Volume");
        let mut volume = volume.show_range();
        assert_eq!(volume.label(&config), "Volume (00-FF)");
        let mut state = state_with_hex_values(false);
        state.config.show_value_ranges = true;
        volume.follow_config(&state);
        assert_eq!(volume.label(&state.config), "Volume (0-255)");
        let cc = NumberBox::new("c", "Default CC", 2)
            .range(0, 0x7F)
            .show_range();
        assert_eq!(cc.label(&config), "Default CC (00-7F)");
    }

    #[test]
    fn value_ranges_are_off_unless_the_config_turns_them_on() {
        let volume = NumberBox::new("v", "Volume", 2).show_range();
        assert_eq!(volume.label(&Config::default()), "Volume");
    }

    #[test]
    fn signed_ranges_go_inside_the_labels_own_brackets() {
        let mut transpose = NumberBox::new("t", "Transpose (semitones)", 2)
            .range(-0x80, 0x7F)
            .show_range();
        assert_eq!(
            transpose.label(&ranges_shown()),
            "Transpose (semitones, -80 to +7F)"
        );
        let mut state = state_with_hex_values(false);
        state.config.show_value_ranges = true;
        transpose.follow_config(&state);
        assert_eq!(
            transpose.label(&state.config),
            "Transpose (semitones, -128 to +127)"
        );
    }

    #[test]
    fn every_dialog_key_is_listed_and_rebinds_on_its_own() {
        let mut sections = vec![];
        Dialog::help(&mut sections);
        let ids: Vec<String> = sections[0]
            .entries
            .iter()
            .filter_map(|entry| entry.binding.as_ref().map(|b| b.id.clone()))
            .collect();
        for action in [
            "prev_item",
            "next_item",
            "open",
            "remove",
            "move_up",
            "move_down",
            "check",
        ] {
            assert!(ids.contains(&format!("dialog.{action}")), "dialog.{action}");
        }
        let mut unique = ids.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), ids.len(), "{ids:?}");
    }

    #[test]
    fn number_box_arrows_show_which_ways_it_can_move() {
        let number = number_box(2).range(0, 2);
        assert_eq!(number.clone_with(0).arrows(), "\u{25B2}");
        assert_eq!(number.clone_with(1).arrows(), "\u{2195}");
        assert_eq!(number.clone_with(2).arrows(), "\u{25BC}");
    }

    #[test]
    fn on_update_runs_only_when_a_value_changes() {
        use std::{cell::RefCell, rc::Rc};

        use ratatui::crossterm::event::KeyEvent;

        use crate::{data::project::Project, tui::theme::Theme};

        let mut state = AppState::new(Project::empty(), Theme::monochrome()).unwrap();
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut dialog = Dialog::new("", vec![Select::new("s", vec!["a", "b"]).boxed()])
            .focus_first()
            .on_update({
                let seen = seen.clone();
                move |_, result| {
                    seen.borrow_mut().extend(result.choice("s"));
                    return Ok(());
                }
            });
        let press = |code| Event::Key(KeyEvent::new(code, KeyModifiers::NONE));
        dialog.handle_event(&mut state, &press(KeyCode::Down));
        dialog.handle_event(&mut state, &press(KeyCode::Down));
        dialog.handle_event(&mut state, &press(KeyCode::Tab));
        assert_eq!(*seen.borrow(), vec![1]);
    }

    #[test]
    fn on_update_can_set_other_fields() {
        use ratatui::crossterm::event::KeyEvent;

        use crate::{data::project::Project, tui::theme::Theme};

        let mut state = AppState::new(Project::empty(), Theme::monochrome()).unwrap();
        let mut dialog = Dialog::new(
            "",
            vec![
                Select::new("color", vec!["red", "green"]).boxed(),
                TextPrompt::new("hex", "Hex").initial("#ff0000").boxed(),
            ],
        )
        .focus_first()
        .on_update(|_, fields| {
            if fields.changed() == "color" {
                let hex = ["#ff0000", "#00ff00"][fields.choice("color").unwrap()];
                fields.set("hex", DialogValue::Text(hex.to_owned()));
            }
            assert_eq!(fields.text("hex"), Some("#00ff00"));
            return Ok(());
        });
        dialog.handle_event(&mut state, &Event::Key(KeyEvent::from(KeyCode::Down)));
        assert_eq!(dialog.result().text("hex"), Some("#00ff00"));
    }

    #[test]
    fn space_opens_a_list_item_and_enter_still_submits() {
        use std::{cell::RefCell, rc::Rc};

        use ratatui::crossterm::event::KeyEvent;

        use crate::{data::project::Project, tui::theme::Theme};

        let mut state = AppState::new(Project::empty(), Theme::monochrome()).unwrap();
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut dialog = Dialog::new("", vec![List::new("l", vec!["a", "b"]).boxed()])
            .focus_first()
            .on_update({
                let seen = seen.clone();
                move |_, fields| {
                    seen.borrow_mut().extend(fields.list_event("l"));
                    return Ok(());
                }
            });
        let press = |code| Event::Key(KeyEvent::new(code, KeyModifiers::NONE));
        dialog.handle_event(&mut state, &press(KeyCode::Down));
        dialog.handle_event(&mut state, &press(KeyCode::Char(' ')));
        assert_eq!(*seen.borrow(), vec![ListEvent::Open(1)]);
        assert!(!dialog.closed);
        dialog.handle_event(&mut state, &press(KeyCode::Enter));
        assert!(dialog.closed);
    }

    impl NumberBox {
        fn clone_with(&self, value: i32) -> NumberBox {
            return NumberBox::new(self.id, "", self.digits)
                .range(self.min, self.max)
                .value(value);
        }
    }

    fn dropdown_dialog(select: Select) -> (AppState, Dialog) {
        use crate::{data::project::Project, tui::theme::Theme};

        let state = AppState::new(Project::empty(), Theme::monochrome()).unwrap();
        let dialog = Dialog::new("", vec![select.boxed()]).focus_first();
        return (state, dialog);
    }

    fn press(dialog: &mut Dialog, state: &mut AppState, code: KeyCode) {
        use ratatui::crossterm::event::KeyEvent;

        dialog.handle_event(state, &Event::Key(KeyEvent::new(code, KeyModifiers::NONE)));
    }

    fn choice(dialog: &Dialog) -> Option<usize> {
        return dialog.result().choice("s");
    }

    #[test]
    fn up_and_down_change_a_dropdown_without_opening_it() {
        let (mut state, mut dialog) = dropdown_dialog(Select::new("s", vec!["a", "b", "c"]));
        press(&mut dialog, &mut state, KeyCode::Down);
        press(&mut dialog, &mut state, KeyCode::Down);
        assert_eq!(choice(&dialog), Some(2));
        press(&mut dialog, &mut state, KeyCode::Enter);
        assert!(dialog.is_closed());
    }

    #[test]
    fn space_opens_the_list_and_esc_puts_the_option_back() {
        let (mut state, mut dialog) = dropdown_dialog(Select::new("s", vec!["a", "b", "c"]));
        press(&mut dialog, &mut state, KeyCode::Char(' '));
        press(&mut dialog, &mut state, KeyCode::Down);
        press(&mut dialog, &mut state, KeyCode::Esc);
        assert!(!dialog.is_closed());
        assert_eq!(choice(&dialog), Some(0));
        press(&mut dialog, &mut state, KeyCode::Char(' '));
        press(&mut dialog, &mut state, KeyCode::Down);
        press(&mut dialog, &mut state, KeyCode::Enter);
        assert!(!dialog.is_closed());
        assert_eq!(choice(&dialog), Some(1));
    }

    #[test]
    fn page_keys_move_by_the_page_step() {
        let notes: Vec<String> = (0..40).map(|i| i.to_string()).collect();
        let (mut state, mut dialog) = dropdown_dialog(Select::new("s", notes).page_step(12));
        press(&mut dialog, &mut state, KeyCode::PageDown);
        press(&mut dialog, &mut state, KeyCode::PageDown);
        assert_eq!(choice(&dialog), Some(24));
        press(&mut dialog, &mut state, KeyCode::PageUp);
        assert_eq!(choice(&dialog), Some(12));
    }

    #[test]
    fn x_ticks_options_and_esc_in_the_list_undoes_it() {
        let select = Select::new("s", vec!["a", "b", "c"]).checkboxes(vec![false, true, false]);
        let (mut state, mut dialog) = dropdown_dialog(select);
        let checks = |dialog: &Dialog| dialog.result().checks("s").unwrap();
        press(&mut dialog, &mut state, KeyCode::Char('x'));
        assert_eq!(checks(&dialog), vec![true, true, false]);
        press(&mut dialog, &mut state, KeyCode::Char(' '));
        press(&mut dialog, &mut state, KeyCode::Down);
        press(&mut dialog, &mut state, KeyCode::Char('x'));
        assert_eq!(checks(&dialog), vec![true, false, false]);
        press(&mut dialog, &mut state, KeyCode::Esc);
        assert_eq!(checks(&dialog), vec![true, true, false]);
        assert!(!dialog.is_closed());
    }
}
