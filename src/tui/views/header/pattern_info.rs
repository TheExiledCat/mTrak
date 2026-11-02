use figlet_rs::FIGlet;
use ratatui::{
    crossterm::event::{
        Event::{self},
        KeyCode::{self},
        KeyModifiers,
    },
    layout::{Constraint, Layout, Rect},
    text::Line,
    widgets::{Block, Borders, Padding, Paragraph, Widget},
};

use crate::{
    data::config::TITLE,
    error::Result,
    tui::{
        app::AppState,
        framework::{
            keymap::{Action, Key, KeyActions, Keymap},
            widget::{Collapsible, MtrakWidget},
        },
    },
};

pub struct PatternInfo {
    keymap: Keymap<PatternInfoAction>,
    /// The FIGlet title, rendered once since parsing the font is costly.
    title: String,
}

impl PatternInfo {
    pub fn new() -> Self {
        use PatternInfoAction::*;
        let keymap = Keymap::new("pattern_info")
            .bind(Key::char('='), BpmUp, "BPM UP")
            .bind(Key::char('-'), BpmDown, "BPM DN")
            .bind(Key::char('+'), SpeedUp, "SPD UP")
            .bind(Key::char('_'), SpeedDown, "SPD DN")
            .bind(Key::char(']'), LineAddUp, "ADD UP")
            .bind(Key::char('['), LineAddDown, "ADD DN")
            .bind(Key::alt('-'), PatternShrink, "SRNK")
            .bind(Key::alt('='), PatternExpand, "EXPD")
            .bind(
                Key::new(KeyCode::Left, KeyModifiers::CONTROL),
                PatternDown,
                "PTN DN",
            )
            .bind(
                Key::new(KeyCode::Right, KeyModifiers::CONTROL),
                PatternUp,
                "PTN UP",
            )
            .bind(
                Key::new(KeyCode::Up, KeyModifiers::CONTROL),
                PatternLengthUp,
                "LEN UP",
            )
            .bind(
                Key::new(KeyCode::Down, KeyModifiers::CONTROL),
                PatternLengthDown,
                "LEN DN",
            );
        let title = FIGlet::small()
            .ok()
            .and_then(|font| font.convert(TITLE).map(|figure| figure.to_string()))
            .unwrap_or_else(|| TITLE.to_owned());
        return Self { keymap, title };
    }

    /// The pattern lines plus their left padding.
    pub fn collapsed_width(&self, state: &AppState) -> u16 {
        let width = self.collapsed_lines(state).iter().map(Line::width).max();
        return width.unwrap_or(0) as u16 + 1;
    }

    fn keys(&self, down: PatternInfoAction, up: PatternInfoAction) -> (String, String) {
        return (self.keymap.key_text(down), self.keymap.key_text(up));
    }

    fn pattern_rows(&self, state: &AppState) -> Vec<InfoRow> {
        use PatternInfoAction::*;
        return vec![
            InfoRow {
                label: "PTN",
                value: format!("{:02X}", state.active_pattern.0),
                keys: self.keys(PatternDown, PatternUp),
            },
            InfoRow {
                label: "LEN",
                value: format!("{:02X}", state.active_pattern().row_count()),
                keys: self.keys(PatternLengthDown, PatternLengthUp),
            },
        ];
    }

    fn line_add_row(&self, state: &AppState) -> InfoRow {
        use PatternInfoAction::*;
        return InfoRow {
            label: "ADD",
            value: state.line_add.to_string(),
            keys: self.keys(LineAddDown, LineAddUp),
        };
    }

    fn pattern_lines(&self, state: &AppState) -> Vec<Line<'static>> {
        return aligned(&self.pattern_rows(state));
    }

    fn collapsed_lines(&self, state: &AppState) -> Vec<Line<'static>> {
        let mut rows = self.pattern_rows(state);
        rows.push(self.line_add_row(state));
        return aligned(&rows);
    }

    fn shrink_pattern(&self, state: &mut AppState) {
        if !state.edit_pattern(|_, pattern| pattern.shrink()) {
            state.notify("PTN TOO SHORT");
        }
    }
}

impl MtrakWidget for PatternInfo {
    fn handle_event(&mut self, state: &mut AppState, event: &Event) -> bool {
        return self.dispatch(state, event);
    }
    fn render(&mut self, state: &AppState, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        self.render_full(state, area, buf);
    }
}

impl Collapsible for PatternInfo {
    fn render_full(
        &mut self,
        state: &AppState,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
    ) {
        use PatternInfoAction::*;
        let vertical_layout =
            Layout::vertical([Constraint::Fill(1), Constraint::Max(3)]).split(area);

        Paragraph::new(self.title.as_str())
            .centered()
            .style(state.theme.heading)
            .block(
                Block::bordered()
                    .borders(Borders::BOTTOM)
                    .title_style(state.theme.label)
                    .title_bottom("Pattern"),
            )
            .render(vertical_layout[0], buf);

        let (tempo_label, tempo) = match state.active_clock_port {
            Some(_) => ("SYN", state.engine.status().synced_bpm()),
            None => ("BPM", state.project().bpm),
        };
        let mut width: u16 = 0;
        let bindings = &self.keymap;
        let tempo_rows = [
            InfoRow {
                label: tempo_label,
                value: format!("{:03}", tempo),
                keys: self.keys(BpmDown, BpmUp),
            },
            InfoRow {
                label: "SPD",
                value: state.project().speed.to_string(),
                keys: self.keys(SpeedDown, SpeedUp),
            },
            self.line_add_row(state),
        ];
        let left_content = Paragraph::new(
            aligned(&tempo_rows)
            .into_iter()
            .map(|line| {
                width = width.max(line.width() as u16) + 1; //border
                line
            })
            .collect::<Vec<Line>>(),
        )
        .block(Block::bordered().borders(Borders::RIGHT));
        let horizontal_layout = Layout::horizontal([Constraint::Max(width), Constraint::Fill(1)])
            .split(vertical_layout[1]);
        let right_layout = Layout::vertical([Constraint::Length(2), Constraint::Length(1)])
            .split(horizontal_layout[1]);

        let right_content_top = Paragraph::new(self.pattern_lines(state));
        let buttons_layout = Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .split(right_layout[1]);

        let buttons_block = Block::new().style(state.theme.label);
        let buttons_style = state.theme.label;
        Paragraph::new(bindings.labelled_key(PatternExpand))
            .style(buttons_style)
            .block(buttons_block.clone())
            .render(buttons_layout[2], buf);

        Paragraph::new(bindings.labelled_key(PatternShrink))
            .style(buttons_style)
            .block(buttons_block)
            .render(buttons_layout[0], buf);
        left_content.render(horizontal_layout[0], buf);
        right_content_top.render(right_layout[0], buf);
    }
    fn render_collapsed(
        &mut self,
        state: &AppState,
        area: Rect,
        buf: &mut ratatui::prelude::Buffer,
    ) {
        Paragraph::new(self.collapsed_lines(state))
            .block(Block::new().padding(Padding::left(1)))
            .render(area, buf);
    }
}

struct InfoRow {
    label: &'static str,
    value: String,
    keys: (String, String),
}

/// `LABEL: value down/up DN/UP`, padded so the values and the key slashes line up.
fn aligned(rows: &[InfoRow]) -> Vec<Line<'static>> {
    let width = |f: fn(&InfoRow) -> &str| {
        rows.iter().map(|r| f(r).chars().count()).max().unwrap_or(0)
    };
    let value_width = width(|r| &r.value);
    let down_width = width(|r| &r.keys.0);
    let up_width = width(|r| &r.keys.1);
    return rows
        .iter()
        .map(|r| {
            Line::raw(format!(
                "{}: {:>value_width$} {:>down_width$}/{:<up_width$} DN/UP",
                r.label, r.value, r.keys.0, r.keys.1
            ))
        })
        .collect();
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum PatternInfoAction {
    BpmUp,
    BpmDown,
    SpeedUp,
    SpeedDown,
    LineAddUp,
    LineAddDown,

    PatternUp,
    PatternDown,
    PatternLengthUp,
    PatternLengthDown,

    PatternShrink,
    PatternExpand,
}

impl Action for PatternInfoAction {}

impl KeyActions for PatternInfo {
    type Action = PatternInfoAction;

    fn keymap(&self) -> &Keymap<Self::Action> {
        return &self.keymap;
    }

    fn perform(&mut self, action: Self::Action, state: &mut AppState) -> Result<()> {
        match action {
            PatternInfoAction::BpmUp => state.increase_bpm(),
            PatternInfoAction::BpmDown => state.decrease_bpm(),
            PatternInfoAction::SpeedUp => state.change_speed(true),
            PatternInfoAction::SpeedDown => state.change_speed(false),
            PatternInfoAction::LineAddUp => state.increase_line_add(),
            PatternInfoAction::LineAddDown => state.decrease_line_add(),
            PatternInfoAction::PatternUp => state.next_pattern(),
            PatternInfoAction::PatternDown => state.prev_pattern(),
            PatternInfoAction::PatternLengthUp => state.change_length(true),
            PatternInfoAction::PatternLengthDown => state.change_length(false),
            PatternInfoAction::PatternShrink => self.shrink_pattern(state),
            PatternInfoAction::PatternExpand => {
                if !state.edit_pattern(|_, pattern| pattern.expand()) {
                    state.notify("PTN TOO LONG");
                }
            }
        }
        return Ok(());
    }

    fn describe(action: PatternInfoAction) -> &'static str {
        return match action {
            PatternInfoAction::BpmUp => "Increase BPM",
            PatternInfoAction::BpmDown => "Decrease BPM",
            PatternInfoAction::SpeedUp => "Increase the song speed (ticks per row)",
            PatternInfoAction::SpeedDown => "Decrease the song speed (ticks per row)",
            PatternInfoAction::LineAddUp => "Increase rows skipped after entering a note",
            PatternInfoAction::LineAddDown => "Decrease rows skipped after entering a note",
            PatternInfoAction::PatternUp => "Next pattern",
            PatternInfoAction::PatternDown => "Previous pattern",
            PatternInfoAction::PatternLengthUp => "Make the pattern a row longer",
            PatternInfoAction::PatternLengthDown => "Make the pattern a row shorter",
            PatternInfoAction::PatternShrink => "Halve the pattern by removing every odd row",
            PatternInfoAction::PatternExpand => {
                "Double the pattern by inserting a blank row after each row"
            }
        };
    }
}
