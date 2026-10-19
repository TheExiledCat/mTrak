use ratatui::{
    buffer::Buffer,
    crossterm::event::{Event, KeyCode},
    layout::Rect,
    text::{Line, Span},
    widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget, Widget},
};

use crate::{
    data::effect::{EFFECT_COMMAND_LOOKUP, MIDI_COMMAND_LOOKUP},
    error::Result,
    tui::{
        app::AppState,
        framework::{
            core_extensions::Boxed,
            dialog::{Dialog, DialogComponent},
            keymap::{HelpSection, Key, KeyActions, KeyHint, Keymap},
            widget::MtrakWidget,
        },
    },
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum KeyTableAction {
    Up,
    Down,
    PageUp,
    PageDown,
    Top,
    Bottom,
}

enum Row {
    Title(&'static str),
    Entry(String, &'static str),
    Blank,
}

struct KeyTable {
    rows: Vec<Row>,
    key_width: usize,
    height: u8,
    visible: usize,
    offset: usize,
    keys: Keymap<KeyTableAction>,
}

impl KeyTable {
    fn new(sections: Vec<HelpSection>, height: u8) -> Self {
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
                rows.push(Row::Entry(entry.key, entry.description));
            }
        }
        let key_width = rows
            .iter()
            .filter_map(|r| match r {
                Row::Entry(key, _) => Some(key.chars().count()),
                _ => None,
            })
            .max()
            .unwrap_or(0);
        return Self {
            rows,
            key_width,
            height,
            visible: height as usize,
            offset: 0,
            keys: table_keymap(),
        };
    }

    fn max_offset(&self) -> usize {
        return self.rows.len().saturating_sub(self.visible);
    }
}

fn table_keymap() -> Keymap<KeyTableAction> {
    use KeyTableAction::*;
    return Keymap::new()
        .bind(Key::plain(KeyCode::Up), Up, "Scroll")
        .bind(Key::plain(KeyCode::Down), Down, "Scroll")
        .bind(Key::plain(KeyCode::PageUp), PageUp, "Page")
        .bind(Key::plain(KeyCode::PageDown), PageDown, "Page")
        .bind(Key::plain(KeyCode::Home), Top, "Top")
        .bind(Key::plain(KeyCode::End), Bottom, "End");
}

impl KeyActions for KeyTable {
    type Action = KeyTableAction;

    fn keymap(&self) -> &Keymap<KeyTableAction> {
        return &self.keys;
    }

    fn perform(&mut self, action: KeyTableAction, _state: &mut AppState) -> Result<()> {
        let page = self.visible.max(1);
        self.offset = match action {
            KeyTableAction::Up => self.offset.saturating_sub(1),
            KeyTableAction::Down => self.offset + 1,
            KeyTableAction::PageUp => self.offset.saturating_sub(page),
            KeyTableAction::PageDown => self.offset + page,
            KeyTableAction::Top => 0,
            KeyTableAction::Bottom => self.max_offset(),
        }
        .min(self.max_offset());
        return Ok(());
    }

    fn describe(action: KeyTableAction) -> &'static str {
        return match action {
            KeyTableAction::Up | KeyTableAction::Down => "Scroll",
            KeyTableAction::PageUp | KeyTableAction::PageDown => "Scroll a page",
            KeyTableAction::Top => "Scroll to the top",
            KeyTableAction::Bottom => "Scroll to the bottom",
        };
    }
}

impl MtrakWidget for KeyTable {
    fn handle_event(&mut self, state: &mut AppState, event: &Event) -> bool {
        return self.dispatch(state, event);
    }

    fn key_hints(&self, _state: &AppState, hints: &mut Vec<KeyHint>) {
        self.keys.hints(hints);
    }

    fn render(&mut self, state: &AppState, area: Rect, buf: &mut Buffer) {
        self.visible = area.height as usize;
        self.offset = self.offset.min(self.max_offset());
        let overflows = self.rows.len() > self.visible;
        let text_width = area.width.saturating_sub(overflows as u16 * 2);
        for (y, row) in self
            .rows
            .iter()
            .skip(self.offset)
            .take(self.visible)
            .enumerate()
        {
            let line = match row {
                Row::Title(title) => Line::styled(*title, state.theme.heading),
                Row::Entry(key, description) => Line::from(vec![
                    Span::styled(format!(" {key} "), state.theme.badge),
                    Span::raw(" ".repeat(self.key_width - key.chars().count() + 2)),
                    Span::styled(*description, state.theme.text),
                ]),
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
}

fn table_dialog(title: &str, sections: Vec<HelpSection>, height: u8) -> Dialog {
    return Dialog::new(title, vec![KeyTable::new(sections, height.max(1)).boxed()])
        .width(u16::MAX);
}

pub fn keybindings_dialog(mut sections: Vec<HelpSection>, height: u8) -> Dialog {
    sections.push(
        HelpSection::new("Keybindings screen")
            .entries(table_keymap().help_entries(KeyTable::describe)),
    );
    return table_dialog("Keybindings", sections, height);
}

pub fn commands_dialog(height: u8) -> Dialog {
    let midi = MIDI_COMMAND_LOOKUP.iter().fold(
        HelpSection::new("MIDI commands"),
        |section, (c, command)| section.entry(format!("{c}xx"), command.description()),
    );
    let effects = EFFECT_COMMAND_LOOKUP.iter().fold(
        HelpSection::new("Effect commands"),
        |section, (c, command)| section.entry(format!("{c}xx"), command.description()),
    );
    let general = HelpSection::new("Values").entry("000", "No command");
    return table_dialog("Commands", vec![general, midi, effects], height);
}
