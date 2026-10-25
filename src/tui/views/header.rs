use std::{iter::repeat_n, time::Instant};

use chrono::{Datelike, Local};
use figlet_rs::FIGlet;
use ratatui::{
    crossterm::event::{
        Event::{self},
        KeyCode::{self, Char},
        KeyModifiers,
    },
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Paragraph, Widget},
};

use crate::{
    data::{
        config::TITLE,
        midi::{MIDI_CHANNEL_MAX, MidiEvent, VELOCITY_MAX, midi_to_string},
        pattern::{ClearOption, ClearTarget},
    },
    error::Result,
    tui::{
        app::AppState,
        framework::{
            core_extensions::Boxed,
            dialog::{
                Checkbox, Dialog, DialogComponent, InfoLevel, Label, Link, NumberBox, Select,
                TextPrompt,
            },
            keymap::{ALT_SYMBOL, ASCII_ALT_SYMBOL, HelpSection, Key, KeyActions, KeyHint, Keymap},
            widget::{Collapsible, MtrakWidget, clicked_in},
        },
        glyphs,
        views::header::SequenceAction::*,
    },
    util::time::duration_to_hh_mm_ss_string,
};

/// The year mTrak was started, for the copyright line in the About dialog.
const FIRST_YEAR: i32 = 2026;
const REPOSITORY_URL: &str = "https://github.com/TheExiledCat/mTrak";
/// The sequence keybinds in two columns, plus the bottom border.
pub const COLLAPSED_HEIGHT: u16 = 4;

pub struct Header {
    top_left: HeaderTopLeft,
    bottom_left: HeaderBottomLeft,
    midi_events: MidiEventList,
    util: UtilList,
    instruments: InstrumentList,
}

impl Default for Header {
    fn default() -> Self {
        return Self::new();
    }
}

impl Header {
    pub fn new() -> Self {
        return Header {
            top_left: HeaderTopLeft::new(),
            bottom_left: HeaderBottomLeft::new(),
            midi_events: MidiEventList::new(),
            util: UtilList::new(),
            instruments: InstrumentList::new(),
        };
    }
}

impl MtrakWidget for Header {
    fn handle_event(&mut self, state: &mut AppState, event: &Event) -> bool {
        return self.top_left.handle_event(state, event)
            || self.bottom_left.handle_event(state, event)
            || self.instruments.handle_event(state, event)
            || self.util.handle_event(state, event);
    }
    fn update(&mut self, _state: &mut AppState) {
        self.top_left.update(_state);
        self.bottom_left.update(_state);
        self.instruments.update(_state);
        self.midi_events.update(_state);
        self.util.update(_state);
    }
    fn render(&mut self, state: &AppState, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        if state.header_collapsed {
            self.render_collapsed(state, area, buf);
        } else {
            self.render_full(state, area, buf);
        }
    }
    fn help(&self, sections: &mut Vec<HelpSection>) {
        let alt = glyphs::pick(ALT_SYMBOL, ASCII_ALT_SYMBOL);
        sections.push(self.top_left.sequence_list.help_section("Sequence"));
        sections.push(self.top_left.pattern_info.help_section("Pattern"));
        sections.push(
            HelpSection::new("Channels")
                .entry(format!("{alt}1-8"), "Mute or unmute a channel")
                .entry(format!("{alt}9"), "Switch between channels 1-8 and 9-16"),
        );
        sections.push(self.instruments.help_section("Instruments"));
        sections.push(self.util.help_section("Util"));
    }
    fn key_hints(&self, state: &AppState, hints: &mut Vec<KeyHint>) {
        self.bottom_left.key_hints(state, hints);
        self.instruments.key_hints(state, hints);
    }
}
impl Collapsible for Header {
    fn render_full(
        &mut self,
        state: &AppState,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
    ) {
        let border = Block::new()
            .borders(Borders::BOTTOM)
            .border_style(state.theme.text);
        border.render(area, buf);
        let layout = Layout::horizontal([
            Constraint::Percentage(50),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
            Constraint::Percentage(20),
        ])
        .split(Block::new().inner(area));

        let left_layout = Layout::vertical([Constraint::Max(8), Constraint::Fill(1)])
            .split(*layout.first().unwrap());
        let util_block = Block::bordered()
            .borders(Borders::BOTTOM | Borders::RIGHT)
            .title_bottom("Util")
            .title_style(state.theme.label);
        let util_inner = util_block.inner(layout[1]);
        util_block.render(layout[1], buf);
        self.top_left.render_full(state, left_layout[0], buf);
        self.bottom_left.render_full(state, left_layout[1], buf);

        self.util.render_full(state, util_inner, buf);
        self.midi_events
            .render_full(state, layout[layout.len() - 2], buf);
        self.instruments
            .render_full(state, *layout.last().unwrap(), buf)
    }
    fn render_collapsed(
        &mut self,
        state: &AppState,
        area: Rect,
        buf: &mut ratatui::prelude::Buffer,
    ) {
        let label_style = state.theme.label;
        let label = |text: String| Line::styled(format!(" {text} "), label_style);
        let mut border = Block::new()
            .borders(Borders::BOTTOM)
            .border_style(state.theme.text)
            .title_bottom(label(self.util.extend_hint()).left_aligned())
            .title_bottom(label(global_info(state)).left_aligned());
        if let Some(notification) = &state.notification {
            border = border.title_bottom(label(notification.clone()).right_aligned());
        }
        let border =
            border.title_bottom(label(duration_to_hh_mm_ss_string(&state.time())).right_aligned());
        let [top_left_area, instruments_area] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Percentage(30)])
                .areas(border.inner(area));
        border.render(area, buf);
        self.top_left.render_collapsed(state, top_left_area, buf);
        self.instruments
            .render_collapsed(state, instruments_area, buf);
    }
}
struct HeaderTopLeft {
    sequence_list: SequenceList,
    pattern_info: PatternInfo,
}
impl HeaderTopLeft {
    fn new() -> Self {
        return Self {
            sequence_list: SequenceList::new(),
            pattern_info: PatternInfo::new(),
        };
    }
}
impl MtrakWidget for HeaderTopLeft {
    fn handle_event(&mut self, state: &mut AppState, event: &Event) -> bool {
        if self.sequence_list.handle_event(state, event) {
            return true;
        }
        if self.pattern_info.handle_event(state, event) {
            return true;
        }
        return false;
    }
    fn render(&mut self, state: &AppState, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        self.render_full(state, area, buf);
    }
}
impl Collapsible for HeaderTopLeft {
    fn render_full(
        &mut self,
        state: &AppState,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
    ) {
        let block = Block::bordered().borders(Borders::LEFT | Borders::RIGHT);
        let sequence_block = Block::bordered()
            .title_top("Sequence")
            .title_style(state.theme.label)
            .borders(Borders::RIGHT);
        let sequence_width = self.sequence_list.width(state) + 1;
        let layout = Layout::horizontal([Constraint::Length(sequence_width), Constraint::Fill(1)])
            .split(block.inner(area));
        block.render(area, buf);
        self.sequence_list
            .render_full(state, sequence_block.inner(layout[0]), buf);
        sequence_block.render(layout[0], buf);
        self.pattern_info.render_full(state, layout[1], buf);
    }
    fn render_collapsed(
        &mut self,
        state: &AppState,
        area: Rect,
        buf: &mut ratatui::prelude::Buffer,
    ) {
        let block = Block::bordered().borders(Borders::LEFT | Borders::RIGHT);
        let sequence_block = Block::bordered().borders(Borders::RIGHT);
        let pattern_width = self.pattern_info.collapsed_width(state);
        let layout = Layout::horizontal([Constraint::Fill(1), Constraint::Length(pattern_width)])
            .split(block.inner(area));
        block.render(area, buf);
        self.sequence_list
            .render_collapsed(state, sequence_block.inner(layout[0]), buf);
        sequence_block.render(layout[0], buf);
        self.pattern_info.render_collapsed(state, layout[1], buf);
    }
}
const VU_SECONDS_MAX: f32 = 1.0;
struct HeaderBottomLeft {
    upper: bool,
}
impl HeaderBottomLeft {
    fn new() -> Self {
        return Self { upper: false };
    }
}

impl MtrakWidget for HeaderBottomLeft {
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
impl Collapsible for HeaderBottomLeft {
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
    fn render_collapsed(
        &mut self,
        _state: &AppState,
        _area: Rect,
        _buf: &mut ratatui::prelude::Buffer,
    ) {
    }
}
#[derive(Clone, Copy, PartialEq)]
enum SequenceAction {
    NextPattern,
    PrevPattern,
    Insert,
    Delete,
    IncPattern,
    DecPattern,
    IncRepeats,
    DecRepeats,
}
const SEQUENCE_LIST_WIDTH: u16 = 9;
struct SequenceList {
    keymap: Keymap<SequenceAction>,
}
impl SequenceList {
    fn new() -> Self {
        let keymap = Keymap::new()
            .bind(
                Key::new(KeyCode::Left, KeyModifiers::SHIFT),
                PrevPattern,
                "UP",
            )
            .bind(
                Key::new(KeyCode::Right, KeyModifiers::SHIFT),
                NextPattern,
                "DOWN",
            )
            .bind(Key::ctrl('n'), Insert, "ADD")
            .bind(Key::ctrl('d'), Delete, "DEL")
            .bind(
                Key::new(KeyCode::Char('.'), KeyModifiers::ALT),
                IncPattern,
                "+PTN",
            )
            .bind(
                Key::new(KeyCode::Char(','), KeyModifiers::ALT),
                DecPattern,
                "-PTN",
            )
            .bind(Key::char('>'), IncRepeats, "+REP")
            .bind(Key::char('<'), DecRepeats, "-REP");
        return Self { keymap };
    }

    fn keybind_rows(&self, state: &AppState) -> [(String, String); 6] {
        let key = |action| self.keymap.key_for(action).unwrap();
        return [
            (
                format!("{}/{}", key(NextPattern), key(PrevPattern)),
                "DN/UP".to_string(),
            ),
            (key(Insert).to_string(), "ADD".to_string()),
            (key(Delete).to_string(), "DEL".to_string()),
            (
                format!("{}/{}", key(DecPattern), key(IncPattern)),
                "PTN".to_string(),
            ),
            (
                format!("{}/{}", key(DecRepeats), key(IncRepeats)),
                "REP".to_string(),
            ),
            ("LEN".to_string(), state.project().song_length().to_string()),
        ];
    }

    fn keybind_widths(rows: &[(String, String)]) -> (u16, u16) {
        return rows.iter().fold((0, 0), |(keys, labels), (key, label)| {
            (
                keys.max(Line::raw(key.as_str()).width() as u16),
                labels.max(Line::raw(label.as_str()).width() as u16 + 1),
            )
        });
    }

    fn render_list(&self, state: &AppState, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        let lines = (0..state.project().sequence.len())
            .map(|i| {
                let row = &state.project().sequence[i];
                let mut line = Line::raw(format!(
                    "{:02X} {:02X} {}",
                    i, row.pattern_id.0, row.repeats
                ));
                if i == state.active_sequence_index {
                    line = line.style(state.theme.selection);
                }
                line
            })
            .collect::<Vec<Line>>();
        let visible_rows = area.height.saturating_sub(1) | 1;
        let [list_area] = Layout::vertical([Constraint::Length(visible_rows)]).areas(area);
        let middle = (list_area.height.saturating_sub(1) / 2) as usize;
        let padded: Vec<Line> = repeat_n(Line::default(), middle).chain(lines).collect();
        let list = Paragraph::new(padded)
            .scroll((state.active_sequence_index as u16, 0))
            .centered();
        list.render(list_area, buf);
    }

    fn render_keybinds(&self, state: &AppState, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        Self::render_keybind_column(self.keybind_rows(state).to_vec(), area, buf);
    }

    fn render_keybind_column(
        rows: Vec<(String, String)>,
        area: Rect,
        buf: &mut ratatui::prelude::Buffer,
    ) {
        let (key_width, label_width) = Self::keybind_widths(&rows);
        let [keys_area, colon_area, labels_area] = Layout::horizontal([
            Constraint::Length(key_width),
            Constraint::Length(1),
            Constraint::Length(label_width),
        ])
        .areas(area);
        let (keys, labels): (Vec<Line>, Vec<Line>) = rows
            .into_iter()
            .map(|(key, label)| (Line::raw(key), Line::raw(format!(" {label}"))))
            .unzip();
        let colons = repeat_n(Line::raw(":"), keys.len()).collect::<Vec<Line>>();
        Paragraph::new(keys).right_aligned().render(keys_area, buf);
        Paragraph::new(colons).render(colon_area, buf);
        Paragraph::new(labels).render(labels_area, buf);
    }

    fn width(&self, state: &AppState) -> u16 {
        let (key_width, label_width) = Self::keybind_widths(&self.keybind_rows(state));
        return SEQUENCE_LIST_WIDTH + key_width + 1 + label_width;
    }
}
impl KeyActions for SequenceList {
    type Action = SequenceAction;

    fn keymap(&self) -> &Keymap<SequenceAction> {
        return &self.keymap;
    }

    fn perform(&mut self, action: SequenceAction, state: &mut AppState) -> Result<()> {
        let sequence_length = state.project().sequence.len();
        match action {
            NextPattern => {
                state.select_sequence((state.active_sequence_index + 1).min(sequence_length - 1))
            }
            PrevPattern => state.select_sequence(state.active_sequence_index.saturating_sub(1)),
            Insert => {
                let index = state.active_sequence_index;
                let pattern = state.project().sequence[index].pattern_id;
                state.insert_sequence(index + 1, pattern);
            }
            Delete => {
                let remaining = state.delete_sequence(state.active_sequence_index);
                state.show_sequence(state.active_sequence_index.min(remaining - 1))
            }
            IncRepeats => state.change_sequence_repeats(state.active_sequence_index, true),
            DecRepeats => state.change_sequence_repeats(state.active_sequence_index, false),
            IncPattern => state.change_sequence_pattern(state.active_sequence_index, true),
            DecPattern => state.change_sequence_pattern(state.active_sequence_index, false),
        }
        return Ok(());
    }

    fn describe(action: SequenceAction) -> &'static str {
        return match action {
            NextPattern => "Next sequence position",
            PrevPattern => "Previous sequence position",
            Insert => "Insert this position's pattern after it",
            Delete => "Delete this sequence position",
            IncPattern => "Next pattern at this position",
            DecPattern => "Previous pattern at this position",
            IncRepeats => "Repeat this position more",
            DecRepeats => "Repeat this position less",
        };
    }
}
impl MtrakWidget for SequenceList {
    fn handle_event(
        &mut self,
        state: &mut AppState,
        event: &ratatui::crossterm::event::Event,
    ) -> bool {
        return self.dispatch(state, event);
    }
    fn render(&mut self, state: &AppState, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        self.render_full(state, area, buf);
    }
}
impl Collapsible for SequenceList {
    fn render_full(
        &mut self,
        state: &AppState,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
    ) {
        let layout =
            Layout::horizontal([Constraint::Length(SEQUENCE_LIST_WIDTH), Constraint::Fill(1)])
                .split(area);
        self.render_list(state, layout[0], buf);
        let time_layout =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).split(layout[1]);
        self.render_keybinds(state, time_layout[0], buf);

        let time = state.time();

        Paragraph::new(duration_to_hh_mm_ss_string(&time))
            .centered()
            .render(time_layout[1], buf);
    }
    fn render_collapsed(
        &mut self,
        state: &AppState,
        area: Rect,
        buf: &mut ratatui::prelude::Buffer,
    ) {
        let layout =
            Layout::horizontal([Constraint::Length(SEQUENCE_LIST_WIDTH), Constraint::Fill(1)])
                .split(area);
        self.render_list(state, layout[0], buf);
        let rows = self.keybind_rows(state);
        let column_areas =
            Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).split(layout[1]);
        for (column, area) in rows.chunks(rows.len().div_ceil(2)).zip(column_areas.iter()) {
            Self::render_keybind_column(column.to_vec(), *area, buf);
        }
    }
}
struct PatternInfo {
    keymap: Keymap<PatternInfoActions>,
}
impl PatternInfo {
    fn new() -> Self {
        use PatternInfoActions::*;
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
    fn collapsed_width(&self, state: &AppState) -> u16 {
        let width = self.pattern_lines(state).iter().map(Line::width).max();
        return width.unwrap_or(0) as u16 + 1;
    }

    fn pattern_lines(&self, state: &AppState) -> Vec<Line<'static>> {
        use PatternInfoActions::*;
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
        use PatternInfoActions::*;
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
enum PatternInfoActions {
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
    type Action = PatternInfoActions;

    fn keymap(&self) -> &Keymap<Self::Action> {
        return &self.keymap;
    }

    fn perform(&mut self, action: Self::Action, state: &mut AppState) -> Result<()> {
        match action {
            PatternInfoActions::BpmUp => state.increase_bpm(),
            PatternInfoActions::BpmDown => state.decrease_bpm(),
            PatternInfoActions::SpeedUp => state.change_speed(true),
            PatternInfoActions::SpeedDown => state.change_speed(false),
            PatternInfoActions::LineAddUp => state.increase_line_add(),
            PatternInfoActions::LineAddDown => state.decrease_line_add(),
            PatternInfoActions::PatternUp => state.next_pattern(),
            PatternInfoActions::PatternDown => state.prev_pattern(),
            PatternInfoActions::PatternLengthUp => state.change_length(true),
            PatternInfoActions::PatternLengthDown => state.change_length(false),
            PatternInfoActions::PatternShrink => self.shrink_pattern(state),
            PatternInfoActions::PatternExpand => {
                if !state.edit_pattern(|_, pattern| pattern.expand()) {
                    state.notify("PTN TOO LONG");
                }
            }
        }
        return Ok(());
    }

    fn describe(action: PatternInfoActions) -> &'static str {
        return match action {
            PatternInfoActions::BpmUp => "Increase BPM",
            PatternInfoActions::BpmDown => "Decrease BPM",
            PatternInfoActions::SpeedUp => "Increase the song speed (ticks per row)",
            PatternInfoActions::SpeedDown => "Decrease the song speed (ticks per row)",
            PatternInfoActions::LineAddUp => "Increase rows skipped after entering a note",
            PatternInfoActions::LineAddDown => "Decrease rows skipped after entering a note",
            PatternInfoActions::PatternUp => "Next pattern",
            PatternInfoActions::PatternDown => "Previous pattern",
            PatternInfoActions::PatternLengthUp => "Make the pattern a row longer",
            PatternInfoActions::PatternLengthDown => "Make the pattern a row shorter",
            PatternInfoActions::PatternShrink => "Halve the pattern by removing every odd row",
            PatternInfoActions::PatternExpand => {
                "Double the pattern by inserting a blank row after each row"
            }
        };
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InstrumentListAction {
    PrevInstrument,
    NextInstrument,
}

struct InstrumentList {
    keymap: Keymap<InstrumentListAction>,
    /// Where the instrument rows were last drawn, for mouse hit-testing.
    rows_area: Rect,
    /// How many columns the rows were last drawn in, filled top to bottom.
    columns: u16,
    scroll: usize,
}
impl InstrumentList {
    fn new() -> Self {
        return Self {
            rows_area: Rect::default(),
            columns: 1,
            scroll: 0,
            keymap: Keymap::new()
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
}
impl InstrumentList {
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
fn global_info(state: &AppState) -> String {
    return format!(
        "OCT: {}, GLB VEL: {:02X}, GLB TRSP: {}{:02X}",
        state.current_octave,
        state.project().global_velocity,
        if state.project().transpose < 0 {
            "-"
        } else {
            "+"
        },
        state.project().transpose.unsigned_abs()
    );
}
fn open_midi_dialog(state: &mut AppState) {
    let mut in_names = state.get_midi_in_port_names();
    in_names.insert(0, "(none)".into());
    let mut out_names = state.get_midi_out_port_names();
    out_names.insert(0, "(none)".into());

    let mut clock_names = state.get_midi_in_port_names();
    clock_names.insert(0, "(internal)".into());
    let current_in = state
        .active_in_port
        .as_ref()
        .zip(state.midi_in_client.as_ref())
        .and_then(|(p, client)| client.port_name(p).ok())
        .and_then(|name| in_names.iter().position(|n| *n == name))
        .unwrap_or(0);

    let current_clock = state
        .active_clock_name()
        .and_then(|name| clock_names.iter().position(|n| *n == name))
        .unwrap_or(0);

    let current_out = state
        .active_out_port
        .as_ref()
        .zip(state.midi_out_client.as_ref())
        .and_then(|(p, client)| client.port_name(p).ok())
        .and_then(|name| out_names.iter().position(|n| *n == name))
        .unwrap_or(0);
    let dialog = Dialog::new(
        "MIDI",
        vec![
            Select::new("input", in_names)
                .label("MIDI input")
                .selected(current_in)
                .boxed(),
            Select::new("clock", clock_names)
                .label("Clock SRC")
                .selected(current_clock)
                .boxed(),
            Checkbox::new("velocity", "Record Velocity")
                .checked(state.project().record_input_velocity)
                .boxed(),
            Select::new("output", out_names)
                .label("MIDI output")
                .selected(current_out)
                .boxed(),
            Checkbox::new("midi_clock", "Send MIDI clock")
                .checked(state.project().send_midi_clock)
                .boxed(),
        ],
    )
    .on_submit(|state, result| {
        if let Some(midi_clock) = result.bool("midi_clock") {
            state.set_midi_clock(midi_clock);
        }
        if let Some(velocity) = result.bool("velocity") {
            state.set_midi_velocity_record(velocity);
        }
        if let Some(output) = result.choice("output") {
            let output = if output == 0 {
                None
            } else {
                state
                    .midi_out_client
                    .as_ref()
                    .and_then(|client| client.ports().get(output - 1).cloned())
            };
            state.set_midi_output(output);
        }
        // inputs last, so a port that fails to open doesn't drop the other changes
        let clock = result
            .choice("clock")
            .map(|clock| state.set_midi_sync(clock));
        let input = result.choice("input").map(|input| {
            let port = state.input_port_for_choice(input);
            state.set_midi_input(port)
        });
        clock.unwrap_or(Ok(()))?;
        return input.unwrap_or(Ok(()));
    });
    state.open_dialog(dialog);
}
fn open_clear_dialog(state: &mut AppState) {
    let options = vec!["Block", "Track", "Pattern"];
    let targets = vec!["All", "Note", "Vel", "Cmd"];
    let dialog = Dialog::new(
        "Clear",
        vec![
            Select::new("clear", options).label("Selection").boxed(),
            Select::new("target", targets).label("Target").boxed(),
        ],
    )
    .on_submit(|state, result| {
        use ClearOption::*;
        use ClearTarget::*;
        let option = match result.choice("clear").unwrap_or(0) {
            0 => Block,
            1 => Track,
            2 => Pattern,
            _ => unreachable!(),
        };
        let target = match result.choice("target").unwrap_or(0) {
            0 => All,
            1 => Note,
            2 => Velocity,
            3 => Command,
            _ => unreachable!(),
        };
        state.request_clear(option, target);
        return Ok(());
    });

    state.open_dialog(dialog);
}
fn open_instrument_dialog(state: &mut AppState) {
    let instrument = state.active_instrument;
    let settings = &state.project().instruments[instrument as usize];
    let dialog = Dialog::new(
        format!("Instrument {:1X}", instrument),
        vec![
            TextPrompt::new("name", "Name")
                .initial(&settings.name)
                .boxed(),
            NumberBox::new("volume", "Volume", 2)
                .value(settings.volume as i32)
                .boxed(),
            NumberBox::new("pitch_wheel_range", "Bend range (semitones)", 2)
                .value(settings.pitch_wheel_range as i32)
                .boxed(),
            NumberBox::new("default_cc", "Default CC", 2)
                .range(0, 0x7F)
                .value(settings.default_cc as i32)
                .boxed(),
            NumberBox::new("transpose", "Transpose (semitones)", 2)
                .range(i8::MIN as i32, i8::MAX as i32)
                .value(settings.transpose as i32)
                .boxed(),
        ],
    )
    .focus_first()
    .on_submit(move |state, result| {
        if let Some(name) = result.text("name") {
            state.project_mut().instruments[instrument as usize].name = name.into();
        }
        if let Some(volume) = result.number("volume") {
            state.set_global_volume(instrument, volume as u8);
        }
        if let Some(range) = result.number("pitch_wheel_range") {
            state.project_mut().instruments[instrument as usize].pitch_wheel_range = range as u8;
        }
        if let Some(control) = result.number("default_cc") {
            state.set_default_cc(instrument, control as u8);
        }
        if let Some(transpose) = result.number("transpose") {
            state.set_instrument_transpose(instrument, transpose as i8);
        }
        return Ok(());
    });
    state.open_dialog(dialog);
}
fn open_about_dialog(state: &mut AppState) {
    let logo = FIGlet::small()
        .ok()
        .and_then(|font| font.convert(TITLE).map(|figure| figure.to_string()))
        .unwrap_or_else(|| TITLE.to_owned());
    let year = Local::now().year();
    let years = if year > FIRST_YEAR {
        format!("{FIRST_YEAR}-{year}")
    } else {
        FIRST_YEAR.to_string()
    };
    let build = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    // pad the logo's lines to one width so centering keeps them lined up
    let width = logo
        .lines()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0);
    let logo: Vec<String> = logo
        .trim_end()
        .lines()
        .map(|line| format!("{line:<width$}"))
        .collect();
    let lines = [
        logo.join("\n"),
        String::new(),
        format!("v{}", env!("CARGO_PKG_VERSION")),
        format!(
            "{build} build, {} {}",
            std::env::consts::OS,
            std::env::consts::ARCH
        ),
        String::new(),
        "A MIDI tracker for the terminal".to_owned(),
        "Created by TheExiledCat".to_owned(),
        format!(
            "{} {years} TheExiledCat, MIT License",
            glyphs::pick("\u{a9}", "(c)")
        ),
    ];
    let mut components: Vec<Box<dyn DialogComponent>> = lines
        .into_iter()
        .map(|line| Label::new(line).centered().boxed() as Box<dyn DialogComponent>)
        .collect();
    components.push(Link::new(REPOSITORY_URL).boxed());
    let dialog = Dialog::new("About", components)
        .level(InfoLevel::Success)
        .is_cancellable(false);
    state.open_dialog(dialog);
}
fn open_global_dialog(state: &mut AppState) {
    let dialog = Dialog::new(
        "Global",
        vec![
            NumberBox::new("velocity", "Velocity", 2)
                .value(state.project().global_velocity as i32)
                .boxed(),
            NumberBox::new("transpose", "Transpose (semitones)", 2)
                .range(i8::MIN as i32, i8::MAX as i32)
                .value(state.project().transpose as i32)
                .boxed(),
        ],
    )
    .focus_first()
    .on_submit(move |state, result| {
        if let Some(velocity) = result.number("velocity") {
            state.set_global_velocity(velocity as u8);
        }
        if let Some(transpose) = result.number("transpose") {
            state.set_transpose(transpose as i8);
        }
        return Ok(());
    });
    state.open_dialog(dialog);
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

        Paragraph::new(global_info(state)).render(layout[2], buf);
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

struct MidiEventList {
    pending_lines: Vec<String>,
    max_lines_to_show: u16,
}

impl MidiEventList {
    fn new() -> Self {
        return Self {
            pending_lines: vec![],
            max_lines_to_show: u16::MAX,
        };
    }

    fn create_lines_from_event(event: &MidiEvent) -> Vec<String> {
        return match event {
            MidiEvent::Aggregate(midi_events) => midi_events
                .iter()
                .flat_map(Self::create_lines_from_event)
                .collect(),
            _ => vec![midi_to_string(event)],
        };
    }
}
impl MtrakWidget for MidiEventList {
    fn update(&mut self, _state: &mut AppState) {
        let mut pending = _state
            .event_history
            .drain(..)
            .flat_map(|e| Self::create_lines_from_event(&e))
            .collect::<Vec<String>>();

        if !pending.is_empty() {
            self.pending_lines.append(&mut pending);
        }
        let overflow = self
            .pending_lines
            .len()
            .saturating_sub(self.max_lines_to_show as usize) as u16;
        if self.pending_lines.len() > self.max_lines_to_show as usize {
            self.pending_lines.drain(0..overflow as usize);
        }
    }
    fn render(&mut self, state: &AppState, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        self.render_full(state, area, buf);
    }
}
impl Collapsible for MidiEventList {
    fn render_full(&mut self, state: &AppState, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        Block::new()
            .title_bottom("MIDI")
            .title_style(state.theme.label)
            .render(area, buf);
        let area = Block::new().padding(Padding::bottom(1)).inner(area);
        self.max_lines_to_show = area.height;
        let height = self.pending_lines.len().min(area.height as usize);
        let layout =
            Layout::vertical([Constraint::Min(0), Constraint::Length(height as u16)]).split(area);
        let overflow = (self
            .pending_lines
            .len()
            .saturating_sub(area.height as usize)) as u16;
        Paragraph::new(
            self.pending_lines
                .iter()
                .map(|l| Line::raw(l.as_str()))
                .collect::<Vec<Line>>(),
        )
        .scroll((overflow, 0))
        .render(layout[1], buf);
    }
    fn render_collapsed(
        &mut self,
        _state: &AppState,
        _area: Rect,
        _buf: &mut ratatui::prelude::Buffer,
    ) {
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum UtilListAction {
    Instruments,
    Global,
    Midi,
    Clear,
    About,
    Extend,
}
struct UtilList {
    keymap: Keymap<UtilListAction>,
}
impl UtilList {
    fn new() -> Self {
        use UtilListAction::*;
        let keymap = Keymap::new()
            .bind(Key::ctrl('r'), Instruments, "Instrument")
            .bind(Key::ctrl('e'), Global, "Global")
            .bind(Key::alt('z'), Clear, "Clear")
            .bind(Key::ctrl('o'), Midi, "Midi")
            .bind(Key::ctrl('x'), Extend, "Extend")
            .bind(Key::ctrl('t'), About, "About");
        return Self { keymap };
    }
}
impl MtrakWidget for UtilList {
    fn handle_event(&mut self, state: &mut AppState, event: &Event) -> bool {
        return self.dispatch(state, event);
    }
    fn render(&mut self, state: &AppState, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        self.render_full(state, area, buf);
    }
}
impl Collapsible for UtilList {
    fn render_full(&mut self, state: &AppState, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        let bindings = &self.keymap.bindings();
        let layout = Layout::vertical([
            Constraint::Length(bindings.len() as u16),
            Constraint::Fill(1),
            Constraint::Length(2),
        ])
        .split(area);
        let keys: Vec<String> = bindings.iter().map(|b| b.key.to_string()).collect();
        let key_width = keys.iter().map(|k| k.chars().count()).max().unwrap_or(0);
        let lines: Vec<Line> = bindings
            .iter()
            .zip(&keys)
            .map(|(b, key)| {
                Line::raw(format!("{key:<key_width$}: {}", b.label)).style(state.theme.label)
            })
            .collect();
        Paragraph::new(lines)
            .block(Block::new().style(state.theme.label))
            .render(layout[0], buf);
        self.render_notification(state, *layout.last().unwrap(), buf);
    }
    fn render_collapsed(
        &mut self,
        _state: &AppState,
        _area: Rect,
        _buf: &mut ratatui::prelude::Buffer,
    ) {
    }
}
impl UtilList {
    fn extend_hint(&self) -> String {
        let hint = self.keymap.hint_for(UtilListAction::Extend).unwrap();
        return format!("{}: {}", hint.key, hint.label);
    }

    fn render_notification(
        &self,
        state: &AppState,
        area: Rect,
        buf: &mut ratatui::prelude::Buffer,
    ) {
        if let Some(notification) = &state.notification {
            Paragraph::new(notification.as_str())
                .centered()
                .block(Block::bordered().borders(Borders::TOP))
                .render(area, buf);
        }
    }
}
impl KeyActions for UtilList {
    type Action = UtilListAction;

    fn keymap(&self) -> &Keymap<Self::Action> {
        return &self.keymap;
    }

    fn perform(&mut self, action: Self::Action, state: &mut AppState) -> Result<()> {
        match action {
            UtilListAction::Instruments => open_instrument_dialog(state),
            UtilListAction::Global => open_global_dialog(state),
            UtilListAction::Midi => open_midi_dialog(state),
            UtilListAction::About => open_about_dialog(state),
            UtilListAction::Clear => open_clear_dialog(state),
            UtilListAction::Extend => state.header_collapsed = !state.header_collapsed,
        }
        return Ok(());
    }

    fn describe(action: UtilListAction) -> &'static str {
        return match action {
            UtilListAction::Instruments => "Edit the current instrument",
            UtilListAction::Global => "Set the global velocity and transpose",
            UtilListAction::Midi => "Set the MIDI ports and clock",
            UtilListAction::About => "About mTrak",
            UtilListAction::Clear => "Clear part of the current pattern",
            UtilListAction::Extend => "Collapse or expand the header to show more of the pattern",
        };
    }
}
