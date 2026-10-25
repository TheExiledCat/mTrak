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
            keymap::{Key, KeyActions, Keymap},
            widget::{Collapsible, MtrakWidget},
        },
    },
};

pub struct PatternInfo {
    keymap: Keymap<PatternInfoAction>,
}

impl PatternInfo {
    pub fn new() -> Self {
        use PatternInfoAction::*;
        let keymap = Keymap::new()
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
        return Self { keymap };
    }

    /// The pattern lines plus their left padding.
    pub fn collapsed_width(&self, state: &AppState) -> u16 {
        let width = self.pattern_lines(state).iter().map(Line::width).max();
        return width.unwrap_or(0) as u16 + 1;
    }

    fn pattern_lines(&self, state: &AppState) -> Vec<Line<'static>> {
        use PatternInfoAction::*;
        let bindings = &self.keymap;
        return [
            format!(
                "PTN: {:02X} {}/{} DN/UP",
                state.active_pattern.0,
                bindings.key_for(PatternDown).unwrap(),
                bindings.key_for(PatternUp).unwrap(),
            ),
            format!(
                "LEN: {:02X} {}/{} DN/UP",
                state.active_pattern().row_count,
                bindings.key_for(PatternLengthDown).unwrap(),
                bindings.key_for(PatternLengthUp).unwrap()
            ),
        ]
        .into_iter()
        .map(Line::raw)
        .collect();
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

        let title = FIGlet::small()
            .ok()
            .and_then(|font| font.convert(TITLE).map(|figure| figure.to_string()))
            .unwrap_or_else(|| TITLE.to_owned());
        Paragraph::new(title)
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
        let left_content = Paragraph::new(
            [
                format!(
                    "{} {:03} {}/{}: DN/UP",
                    tempo_label,
                    tempo,
                    bindings.key_for(BpmDown).unwrap(),
                    bindings.key_for(BpmUp).unwrap()
                ),
                format!(
                    "SPD {:>3} {}/{}: DN/UP",
                    state.project().speed,
                    bindings.key_for(SpeedDown).unwrap(),
                    bindings.key_for(SpeedUp).unwrap()
                ),
                format!(
                    "ADD {:>3} {}/{}: DN/UP",
                    state.line_add,
                    bindings.key_for(LineAddDown).unwrap(),
                    bindings.key_for(LineAddUp).unwrap()
                ),
            ]
            .into_iter()
            .map(|s| {
                width = width.max(s.len() as u16) + 1; //border
                Line::raw(s)
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
        let hint = bindings.hint_for(PatternExpand).unwrap();
        Paragraph::new(format!("{}: {}", hint.key, hint.label))
            .style(buttons_style)
            .block(buttons_block.clone())
            .render(buttons_layout[2], buf);

        let hint = bindings.hint_for(PatternShrink).unwrap();
        Paragraph::new(format!("{}: {}", hint.key, hint.label))
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
        Paragraph::new(self.pattern_lines(state))
            .block(Block::new().padding(Padding::left(1)))
            .render(area, buf);
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
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
