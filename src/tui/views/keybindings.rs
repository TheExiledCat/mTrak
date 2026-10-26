use ratatui::{
    buffer::Buffer,
    crossterm::event::{Event, KeyCode, KeyEventKind},
    layout::Rect,
    text::{Line, Span},
    widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget, Widget},
};

use crate::{
    data::effect::{CommandForm, EFFECT_COMMAND_LOOKUP, MIDI_COMMAND_LOOKUP},
    error::Result,
    tui::{
        app::AppState,
        framework::{
            core_extensions::Boxed,
            dialog::{Buttons, Dialog, DialogComponent, InfoLevel, Label},
            keymap::{
                Action, BindingInfo, HelpSection, Key, KeyActions, KeyHint, Keymap, UNBOUND,
                join_keys,
            },
            widget::MtrakWidget,
        },
        keybinds,
    },
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyTableAction {
    Up,
    Down,
    PageUp,
    PageDown,
    Top,
    Bottom,
    Rebind,
    Reset,
}

impl Action for KeyTableAction {}

/// Shown in place of the keys while waiting for the key to bind.
const WAITING: &str = "...";

enum Row {
    Title(&'static str),
    Entry(String, &'static str, Option<BindingInfo>),
    Blank,
}

/// A scrolling table of keys or commands. On the keybindings screen it's also where keys are
/// rebound: Space waits for the key to bind to the selected action, Del puts back its default.
struct KeyTable {
    rows: Vec<Row>,
    height: u8,
    visible: usize,
    offset: usize,
    /// The selected row, on the keybindings screen.
    selected: Option<usize>,
    /// Waiting for the key to bind to the selected action.
    capturing: bool,
    /// The selection only shows while the table has focus.
    focused: bool,
    keys: Keymap<KeyTableAction>,
}

impl KeyTable {
    fn new(sections: Vec<HelpSection>, height: u8, editable: bool) -> Self {
        let mut rows = Vec::new();
        for section in sections {
            if section.entries.is_empty() {
                continue;
            }
            if !rows.is_empty() {
                rows.push(Row::Blank);
            }
            rows.push(Row::Title(section.title));
            for entry in section.entries {
                rows.push(Row::Entry(entry.key, entry.description, entry.binding));
            }
        }
        let selected = editable
            .then(|| rows.iter().position(|r| matches!(r, Row::Entry(..))))
            .flatten();
        return Self {
            rows,
            height,
            visible: height as usize,
            offset: 0,
            selected,
            capturing: false,
            focused: false,
            keys: table_keymap(editable),
        };
    }

    fn max_offset(&self) -> usize {
        return self.rows.len().saturating_sub(self.visible);
    }

    /// The keys a row shows, as they apply now.
    fn key_text(&self, index: usize) -> String {
        let Row::Entry(key, _, binding) = &self.rows[index] else {
            return String::new();
        };
        if self.capturing && self.selected == Some(index) {
            return WAITING.to_owned();
        }
        let Some(binding) = binding else {
            return key.clone();
        };
        let keys = keybinds::keys(binding);
        return if keys.is_empty() {
            UNBOUND.to_owned()
        } else {
            join_keys(&keys)
        };
    }

    fn selected_binding(&self) -> Option<&BindingInfo> {
        return match self.rows.get(self.selected?)? {
            Row::Entry(_, _, binding) => binding.as_ref(),
            _ => None,
        };
    }

    /// Moves the selection `steps` entries up or down, stopping at the first and last.
    fn move_selection(&mut self, steps: isize) {
        let Some(mut selected) = self.selected else {
            return;
        };
        let entries: Vec<usize> = (0..self.rows.len())
            .filter(|&i| matches!(self.rows[i], Row::Entry(..)))
            .collect();
        let at = entries.iter().position(|&i| i == selected).unwrap_or(0) as isize;
        let target = (at + steps).clamp(0, entries.len() as isize - 1);
        selected = entries[target as usize];
        self.selected = Some(selected);
    }

    /// Binds the key just pressed to the selected action, after a warning when the key does
    /// something else too.
    fn capture(&mut self, state: &mut AppState, key: Key) {
        self.capturing = false;
        let Some(binding) = self.selected_binding() else {
            return;
        };
        let id = binding.id.clone();
        let uses = keybinds::uses(key, &id);
        if uses.is_empty() {
            if let Err(e) = keybinds::set(&id, Some(vec![key])) {
                state.report_error(e);
            }
            return;
        }
        let text = format!(
            "{} is also used for:\n{}\n\nBind it anyway?",
            key,
            uses.iter()
                .map(|u| format!("- {u}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
        let width = text.lines().map(|l| l.chars().count()).max().unwrap_or(0) as u16 + 4;
        let dialog = Dialog::confirm(InfoLevel::Warning, "Key in use", text)
            .width(width.max(40))
            .on_submit(move |_, _| keybinds::set(&id, Some(vec![key])));
        state.open_dialog(dialog);
    }
}

fn table_keymap(editable: bool) -> Keymap<KeyTableAction> {
    use KeyTableAction::*;
    let label = if editable { "Select" } else { "Scroll" };
    let keymap = Keymap::new("keybindings")
        .bind(Key::plain(KeyCode::Up), Up, label)
        .bind(Key::plain(KeyCode::Down), Down, label)
        .bind(Key::plain(KeyCode::PageUp), PageUp, "Page")
        .bind(Key::plain(KeyCode::PageDown), PageDown, "Page")
        .bind(Key::plain(KeyCode::Home), Top, "Top")
        .bind(Key::plain(KeyCode::End), Bottom, "End");
    if !editable {
        return keymap;
    }
    return keymap.bind(Key::char(' '), Rebind, "Rebind").bind(
        Key::plain(KeyCode::Delete),
        Reset,
        "Default",
    );
}

impl KeyActions for KeyTable {
    type Action = KeyTableAction;

    fn keymap(&self) -> &Keymap<KeyTableAction> {
        return &self.keys;
    }

    fn perform(&mut self, action: KeyTableAction, _state: &mut AppState) -> Result<()> {
        let page = self.visible.max(1);
        if self.selected.is_some() {
            match action {
                KeyTableAction::Up => self.move_selection(-1),
                KeyTableAction::Down => self.move_selection(1),
                KeyTableAction::PageUp => self.move_selection(-(page as isize)),
                KeyTableAction::PageDown => self.move_selection(page as isize),
                KeyTableAction::Top => self.move_selection(-(self.rows.len() as isize)),
                KeyTableAction::Bottom => self.move_selection(self.rows.len() as isize),
                KeyTableAction::Rebind => self.capturing = self.selected_binding().is_some(),
                KeyTableAction::Reset => {
                    if let Some(binding) = self.selected_binding() {
                        keybinds::set(&binding.id.clone(), None)?;
                    }
                }
            }
            return Ok(());
        }
        self.offset = match action {
            KeyTableAction::Up => self.offset.saturating_sub(1),
            KeyTableAction::Down => self.offset + 1,
            KeyTableAction::PageUp => self.offset.saturating_sub(page),
            KeyTableAction::PageDown => self.offset + page,
            KeyTableAction::Top => 0,
            KeyTableAction::Bottom => self.max_offset(),
            KeyTableAction::Rebind | KeyTableAction::Reset => self.offset,
        }
        .min(self.max_offset());
        return Ok(());
    }

    fn describe(action: KeyTableAction) -> &'static str {
        return match action {
            KeyTableAction::Up => "Move up",
            KeyTableAction::Down => "Move down",
            KeyTableAction::PageUp => "Move a page up",
            KeyTableAction::PageDown => "Move a page down",
            KeyTableAction::Top => "Move to the top",
            KeyTableAction::Bottom => "Move to the bottom",
            KeyTableAction::Rebind => "Rebind: press the key to bind, Esc to keep the old one",
            KeyTableAction::Reset => "Put back the default keys",
        };
    }
}

impl MtrakWidget for KeyTable {
    fn handle_event(&mut self, state: &mut AppState, event: &Event) -> bool {
        if !self.capturing {
            return self.dispatch(state, event);
        }
        let Event::Key(key) = event else {
            return true;
        };
        match key.code {
            _ if key.kind == KeyEventKind::Release => {}
            KeyCode::Modifier(_) => {}
            KeyCode::Esc => self.capturing = false,
            code => self.capture(state, Key::new(code, key.modifiers)),
        }
        return true;
    }

    fn key_hints(&self, _state: &AppState, hints: &mut Vec<KeyHint>) {
        if self.capturing {
            hints.push(KeyHint {
                key: "Any".to_owned(),
                label: "Bind",
                enabled: true,
            });
            hints.push(KeyHint {
                key: Key::plain(KeyCode::Esc).to_string(),
                label: "Keep",
                enabled: true,
            });
            return;
        }
        self.keys.hints(hints);
    }

    fn render(&mut self, state: &AppState, area: Rect, buf: &mut Buffer) {
        self.visible = area.height as usize;
        if let Some(selected) = self.selected {
            // scroll just enough to keep the selection in view, along with its title
            let first = selected.saturating_sub(1);
            if first < self.offset {
                self.offset = first;
            } else if selected >= self.offset + self.visible {
                self.offset = selected + 1 - self.visible;
            }
        }
        self.offset = self.offset.min(self.max_offset());
        let overflows = self.rows.len() > self.visible;
        let text_width = area.width.saturating_sub(overflows as u16 * 2);
        let keys: Vec<String> = (0..self.rows.len()).map(|i| self.key_text(i)).collect();
        let key_width = keys.iter().map(|k| k.chars().count()).max().unwrap_or(0);
        for (y, index) in (self.offset..self.rows.len())
            .take(self.visible)
            .enumerate()
        {
            let key = &keys[index];
            let line = match &self.rows[index] {
                Row::Title(title) => Line::styled(*title, state.theme.heading),
                Row::Entry(_, description, _) => {
                    let selected = self.focused && self.selected == Some(index);
                    let (key_style, row_style) = match (selected, self.capturing) {
                        (true, true) => (state.theme.danger, state.theme.selection),
                        (true, false) => (state.theme.cursor, state.theme.selection),
                        (false, _) => (state.theme.selection, state.theme.text),
                    };
                    let padding = " ".repeat(key_width - key.chars().count() + 2);
                    let line = Line::from(vec![
                        Span::styled(format!(" {key} "), key_style),
                        Span::styled(padding, row_style),
                        Span::styled(*description, row_style),
                    ]);
                    if selected {
                        line.style(row_style)
                    } else {
                        line
                    }
                }
                Row::Blank => Line::default(),
            };
            line.render(Rect::new(area.x, area.y + y as u16, text_width, 1), buf);
        }
        if overflows {
            let mut scroll = ScrollbarState::new(self.max_offset() + 1).position(self.offset);
            Scrollbar::new(ScrollbarOrientation::VerticalRight).render(area, buf, &mut scroll);
        }
    }
}

impl DialogComponent for KeyTable {
    fn get_height(&self) -> u8 {
        return self.height;
    }
    fn focusable(&self) -> bool {
        return true;
    }
    fn takes_keys_first(&self) -> bool {
        return true;
    }
    fn takes_all_keys(&self) -> bool {
        return self.capturing;
    }
    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
        self.capturing &= focused;
    }
}

/// The keybindings screen's own keys, for the keybindings screen and keybinds.toml.
pub fn help_section() -> HelpSection {
    return HelpSection::new("Keybindings screen")
        .entries(table_keymap(true).help_entries(KeyTable::describe));
}

fn table_dialog(title: &str, table: KeyTable) -> Dialog {
    return Dialog::new(title, vec![table.boxed()])
        .width(u16::MAX)
        .focus_first();
}

/// Every key, from `App::help_sections`, where keys can be rebound too, with a button below
/// that puts back every default.
pub fn keybindings_dialog(sections: Vec<HelpSection>, height: u8) -> Dialog {
    // the button and the gap above it come out of the table's height
    let table = KeyTable::new(sections, height.saturating_sub(2).max(1), true);
    return Dialog::new(
        "Keybindings",
        vec![
            table.boxed(),
            Label::new("").boxed(),
            Buttons::new("reset_all", vec!["Reset all keys"]).boxed(),
        ],
    )
    .width(u16::MAX)
    .focus_first()
    .on_update(|state, fields| {
        if fields.changed() == "reset_all" && fields.pressed("reset_all").is_some() {
            let dialog = Dialog::confirm(
                InfoLevel::Warning,
                "Reset all keys",
                "Put back the default keys of every action?",
            )
            .on_submit(|_, _| keybinds::reset_all());
            state.open_dialog(dialog);
        }
        return Ok(());
    });
}

pub fn commands_dialog(height: u8) -> Dialog {
    let midi = commands_section(
        "MIDI commands",
        MIDI_COMMAND_LOOKUP
            .iter()
            .map(|(c, command)| (*c, command.forms())),
    );
    let effects = commands_section(
        "Effect commands",
        EFFECT_COMMAND_LOOKUP
            .iter()
            .map(|(c, command)| (*c, command.forms())),
    );
    let general = HelpSection::new("Values").entry("000", "No command");
    let sections = vec![general, midi, effects];
    return table_dialog("Commands", KeyTable::new(sections, height.max(1), false));
}

/// One row per way to write each command, e.g. `L0y` and `L1y` for the legato command.
fn commands_section(
    title: &'static str,
    commands: impl Iterator<Item = (char, &'static [CommandForm])>,
) -> HelpSection {
    return commands
        .flat_map(|(c, forms)| forms.iter().map(move |form| (c, form)))
        .fold(
            HelpSection::new(title),
            |section, (c, (syntax, description))| {
                section.entry(format!("{c}{syntax}"), description)
            },
        );
}
