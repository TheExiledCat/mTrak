use ratatui::{
    crossterm::event::{
        Event::{self},
        KeyCode::{self},
        KeyModifiers,
    },
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::{
    error::Result,
    tui::{
        app::AppState,
        framework::{
            keymap::{Action, Key, KeyActions, KeyHint, Keymap},
            widget::{Collapsible, MtrakWidget, clicked_in},
        },
        views::header::octave_and_globals_text,
    },
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InstrumentListAction {
    PrevInstrument,
    NextInstrument,
}

impl Action for InstrumentListAction {}

pub struct InstrumentList {
    keymap: Keymap<InstrumentListAction>,
    /// Where the instrument rows were last drawn, for mouse hit-testing.
    rows_area: Rect,
    /// How many columns the rows were last drawn in, filled top to bottom.
    columns: u16,
    scroll: usize,
}

impl InstrumentList {
    pub fn new() -> Self {
        return Self {
            rows_area: Rect::default(),
            columns: 1,
            scroll: 0,
            keymap: Keymap::new("instruments")
                .bind(
                    Key::new(KeyCode::Up, KeyModifiers::SHIFT),
                    InstrumentListAction::PrevInstrument,
                    "INST",
                )
                .bind(
                    Key::new(KeyCode::Down, KeyModifiers::SHIFT),
                    InstrumentListAction::NextInstrument,
                    "INST",
                ),
        };
    }

    fn instrument_lines(state: &AppState) -> Vec<Line<'_>> {
        let selected_instrument = state.active_instrument;
        return state
            .project()
            .instruments
            .iter()
            .enumerate()
            .map(|e| {
                Line::from(vec![
                    Span::raw(format!("{:1X} {:02X}", e.0, e.1.volume)).style(
                        if selected_instrument == e.0 as u8 {
                            state.theme.surface
                        } else {
                            state.theme.label
                        },
                    ),
                    Span::raw(format!("| {}", e.1.name.as_str())),
                ])
            })
            .collect();
    }

    /// Draws the instruments in two columns, a page at a time like FT2's instrument switcher.
    fn render_grid(&mut self, state: &AppState, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        let height = area.height as usize;
        let page_size = (height * 2).max(1);
        self.scroll = state.active_instrument as usize / page_size * page_size;
        self.rows_area = area;
        self.columns = 2;
        let lines = Self::instrument_lines(state);
        let column_areas = Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)])
            .spacing(1)
            .split(area);
        for (column, column_area) in column_areas.iter().enumerate() {
            let column_lines = lines
                .iter()
                .skip(self.scroll + column * height)
                .take(height)
                .cloned()
                .collect::<Vec<Line>>();
            Paragraph::new(column_lines).render(*column_area, buf);
        }
    }

    fn render_rows(&mut self, state: &AppState, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        let lines = Self::instrument_lines(state);
        let height = area.height as usize;
        let selected = state.active_instrument as usize;
        let max_scroll = lines.len().saturating_sub(height);
        self.scroll = self
            .scroll
            .clamp(selected.saturating_sub(height.saturating_sub(1)), selected)
            .min(max_scroll);
        self.rows_area = area;
        self.columns = 1;
        Paragraph::new(lines)
            .scroll((self.scroll as u16, 0))
            .render(area, buf);
    }
}

impl KeyActions for InstrumentList {
    type Action = InstrumentListAction;

    fn keymap(&self) -> &Keymap<InstrumentListAction> {
        return &self.keymap;
    }

    fn perform(&mut self, action: InstrumentListAction, state: &mut AppState) -> Result<()> {
        match action {
            InstrumentListAction::PrevInstrument => {
                state.prev_instrument();
            }
            InstrumentListAction::NextInstrument => {
                state.next_instrument();
            }
        };
        return Ok(());
    }

    fn describe(action: InstrumentListAction) -> &'static str {
        return match action {
            InstrumentListAction::PrevInstrument => "Previous instrument",
            InstrumentListAction::NextInstrument => "Next instrument",
        };
    }
}

impl MtrakWidget for InstrumentList {
    fn handle_event(&mut self, state: &mut AppState, event: &Event) -> bool {
        if let Some(pos) = clicked_in(event, self.rows_area) {
            let column_width = (self.rows_area.width / self.columns).max(1);
            let column = (pos.x / column_width).min(self.columns - 1);
            let index =
                self.scroll + column as usize * self.rows_area.height as usize + pos.y as usize;
            if index < state.project().instruments.len() {
                state.active_instrument = index as u8;
            }
            return true;
        }
        return self.dispatch(state, event);
    }
    fn key_hints(&self, _state: &AppState, hints: &mut Vec<KeyHint>) {
        self.keymap.hints(hints);
    }
    fn render(&mut self, state: &AppState, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        self.render_full(state, area, buf);
    }
}

impl Collapsible for InstrumentList {
    fn render_full(&mut self, state: &AppState, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        let block = Block::bordered().borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM);

        let layout = Layout::vertical([
            Constraint::Length(2),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .split(block.inner(area));

        block.render(area, buf);
        let port = match (&state.midi_out_client, &state.active_out_port) {
            (None, _) => "no MIDI backend".into(),
            (Some(c), Some(p)) => {
                let name = c.port_name(p).unwrap_or(p.id());
                if state.engine.status().output_connected() {
                    name
                } else {
                    format!("{name} (not connected)")
                }
            }
            (Some(_), None) => "".into(),
        };
        let port_block = Block::bordered()
            .borders(Borders::BOTTOM)
            .title_bottom("Instruments")
            .title_style(state.theme.label);
        Paragraph::new(port)
            .left_aligned()
            .style(state.theme.label)
            .render(port_block.inner(layout[0]), buf);
        port_block.render(layout[0], buf);
        self.render_rows(state, layout[1], buf);

        Paragraph::new(octave_and_globals_text(state)).render(layout[2], buf);
    }
    fn render_collapsed(
        &mut self,
        state: &AppState,
        area: Rect,
        buf: &mut ratatui::prelude::Buffer,
    ) {
        let block = Block::bordered().borders(Borders::RIGHT);
        let rows_area = block.inner(area);
        block.render(area, buf);
        self.render_grid(state, rows_area, buf);
    }
}
