use std::{iter::repeat_n, time::Instant};

use ratatui::{
    crossterm::event::{
        Event::{self},
        KeyCode::Char,
        KeyModifiers,
    },
    layout::{Constraint, Layout, Rect},
    text::Line,
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::{
    data::midi::{MIDI_CHANNEL_MAX, VELOCITY_MAX},
    tui::{
        app::AppState,
        framework::{
            keymap::{ALT_SYMBOL, ASCII_ALT_SYMBOL, KeyHint},
            widget::{Collapsible, MtrakWidget},
        },
        glyphs,
    },
};

const VU_SECONDS_MAX: f32 = 1.0;

pub struct ChannelGrid {
    upper: bool,
}

impl ChannelGrid {
    pub fn new() -> Self {
        return Self { upper: false };
    }
}

impl MtrakWidget for ChannelGrid {
    fn handle_event(&mut self, state: &mut AppState, event: &Event) -> bool {
        let Event::Key(k) = event else {
            return false;
        };
        if k.modifiers != KeyModifiers::ALT {
            return false;
        }
        let Char(c) = k.code else {
            return false;
        };
        let Some(n) = c.to_digit(10) else {
            return false;
        };
        let n = n as u8;
        let boundary = MIDI_CHANNEL_MAX.div_ceil(2);
        if n == boundary + 1 {
            self.upper = !self.upper;
            return true;
        }
        if n < 1 || n > boundary {
            return false;
        }
        let channel = if self.upper { n + boundary } else { n };
        let index = channel as usize - 1;
        let mutes = &mut state.active_pattern_mut().channel_mutes;
        mutes[index] = !mutes[index];

        return true;
    }
    fn key_hints(
        &self,
        _state: &AppState,
        hints: &mut Vec<crate::tui::framework::keymap::KeyHint>,
    ) {
        hints.push(KeyHint {
            key: format!("{}{}", glyphs::pick(ALT_SYMBOL, ASCII_ALT_SYMBOL), "1-8"),
            label: "Mute",
            enabled: true,
        });
        hints.push(KeyHint {
            key: format!("{}{}", glyphs::pick(ALT_SYMBOL, ASCII_ALT_SYMBOL), "9"),
            label: if self.upper { "Ch 1-8" } else { "Ch 9-16" },
            enabled: true,
        });
    }
    fn render(&mut self, state: &AppState, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        self.render_full(state, area, buf);
    }
}

impl Collapsible for ChannelGrid {
    fn render_full(
        &mut self,
        _state: &AppState,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
    ) {
        let grid_block = Block::bordered()
            .borders(Borders::ALL)
            .title_top("Channels")
            .title_style(_state.theme.label);
        let grid_rect = grid_block.inner(area);
        grid_block.render(area, buf);
        let grid = {
            let vertical_layout =
                Layout::vertical([Constraint::Fill(1), Constraint::Fill(1)]).split(grid_rect);
            let layout = Layout::horizontal(repeat_n(
                Constraint::Fill(1),
                (MIDI_CHANNEL_MAX + 1) as usize / 2,
            ));
            let top = layout.split(vertical_layout[0]).to_vec();
            let bottom = layout.split(vertical_layout[1]).to_vec();
            top.into_iter().chain(bottom).collect::<Vec<Rect>>()
        };
        let now = Instant::now();
        grid.iter().enumerate().for_each(|c| {
            let rect = *c.1;
            let channel = c.0;

            let mut borders = Borders::NONE;
            if channel % ((MIDI_CHANNEL_MAX + 1) as usize / 2) != 0 {
                borders |= Borders::LEFT;
            }
            let in_upper = channel >= (MIDI_CHANNEL_MAX + 1) as usize / 2;
            if !in_upper {
                borders |= Borders::BOTTOM;
            }
            let style = if in_upper == self.upper {
                _state.theme.heading
            } else {
                _state.theme.text_disabled
            };

            let block = Block::bordered().borders(borders);
            let inner = block.inner(rect);
            Paragraph::new(
                [
                    Line::raw(format!("{:1X}", channel)),
                    if _state.active_pattern().channel_mutes[channel] {
                        Line::raw("M")
                    } else {
                        Line::raw("")
                    },
                ]
                .to_vec(),
            )
            .block(block)
            .centered()
            .style(style)
            .render(rect, buf);
            let Some(hit) = _state.channel_hits[channel] else {
                return;
            };
            let elapsed = now.duration_since(hit.at).as_secs_f32() / VU_SECONDS_MAX;
            let level = (1.0 - elapsed).clamp(0.0, 1.0) * hit.velocity as f32 / VELOCITY_MAX as f32;
            let height = (inner.height as f32 * level).round() as u16;
            let bar = Rect {
                y: inner.bottom() - height,
                height,
                ..inner
            };
            buf.set_style(bar, _state.theme.label);
        });
    }
}
