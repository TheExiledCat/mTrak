use std::{iter::repeat_n, time::Instant};

use figlet_rs::FIGlet;
use ratatui::{
    crossterm::{
        event::{
            Event::{self},
            KeyCode::{self, Char},
            KeyModifiers,
        },
        terminal,
    },
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Paragraph, Widget},
};

use crate::{
    data::{
        config::TITLE,
        midi::{MIDI_CHANNEL_MAX, MidiEvent, VELOCITY_MAX, midi_to_string},
    },
    tui::{
        app::AppState,
        framework::{
            core_extensions::Boxed,
            dialog::{Dialog, Select},
            keymap::{ALT_SYMBOL, Key, KeyActions, KeyHint, Keymap},
            widget::{MtrakWidget, clicked_in},
        },
        views::header::SequenceAction::*,
    },
};

pub struct Header {
    top_left: HeaderTopLeft,
    bottom_left: HeaderBottomLeft,
    midi_events: MidiEventList,
    instruments: InstrumentList,
}

impl Header {
    pub fn new() -> Self {
        return Header {
            top_left: HeaderTopLeft::new(),
            bottom_left: HeaderBottomLeft::new(),
            midi_events: MidiEventList::new(),
            instruments: InstrumentList::new(),
        };
    }
}

impl MtrakWidget for Header {
    fn handle_event(&mut self, state: &mut AppState, event: &Event) -> bool {
        return self.top_left.handle_event(state, event)
            || self.bottom_left.handle_event(state, event)
            || self.instruments.handle_event(state, event);
    }
    fn update(&mut self, _state: &mut AppState) {
        self.top_left.update(_state);
        self.bottom_left.update(_state);
        self.instruments.update(_state);
        self.midi_events.update(_state);
    }
    fn render(
        &mut self,
        state: &AppState,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
    ) where
        Self: Sized,
    {
        let border = Block::new()
            .borders(Borders::BOTTOM)
            .border_style(state.theme.header_border);
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
        Block::bordered()
            .borders(Borders::BOTTOM | Borders::RIGHT)
            .render(layout[1], buf);
        self.top_left.render(state, left_layout[0], buf);
        self.bottom_left.render(state, left_layout[1], buf);
        self.midi_events
            .render(state, layout[layout.len() - 2], buf);
        self.instruments.render(state, *layout.last().unwrap(), buf)
    }
    fn key_hints(&self, state: &AppState, hints: &mut Vec<KeyHint>) {
        self.bottom_left.key_hints(state, hints);
        self.instruments.key_hints(state, hints);
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
    fn render(
        &mut self,
        state: &AppState,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
    ) {
        let block = Block::bordered().borders(Borders::LEFT | Borders::RIGHT);
        let sequence_block = Block::bordered()
            .title_top("Sequence")
            .title_style(Style::new().reversed())
            .borders(Borders::RIGHT);
        let sequence_width = self.sequence_list.width(state) + 1;
        let layout = Layout::horizontal([Constraint::Length(sequence_width), Constraint::Fill(1)])
            .split(block.inner(area));
        block.render(area, buf);
        self.sequence_list
            .render(state, sequence_block.inner(layout[0]), buf);
        sequence_block.render(layout[0], buf);
        self.pattern_info.render(state, layout[1], buf);
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
        let boundary = (MIDI_CHANNEL_MAX + 1) / 2;
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
            key: format!("{}{}", ALT_SYMBOL, "1-8"),
            label: "Mute",
        });
        hints.push(KeyHint {
            key: format!("{}{}", ALT_SYMBOL, "9"),
            label: if self.upper { "Ch 1-8" } else { "Ch 9-16" },
        });
    }
    fn render(
        &mut self,
        _state: &AppState,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
    ) {
        let grid_block = Block::bordered()
            .borders(Borders::ALL)
            .title_top("Channels")
            .title_style(Style::new().reversed());
        let grid_rect = grid_block.inner(area.clone());
        grid_block.render(area, buf);
        let grid = {
            let vertical_layout =
                Layout::vertical([Constraint::Fill(1), Constraint::Fill(1)]).split(grid_rect);
            let layout = Layout::horizontal(repeat_n(
                Constraint::Fill(1),
                (MIDI_CHANNEL_MAX + 1) as usize / 2,
            ));
            let top = layout.split(vertical_layout[0].clone()).to_vec();
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
                Style::new().bold()
            } else {
                Style::new().bold().dim()
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
            buf.set_style(bar, _state.theme.header_border.reversed());
        });
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
            ("LEN".to_string(), state.project.song_length().to_string()),
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

    fn perform(&mut self, action: SequenceAction, state: &mut AppState) {
        let sequence_length = state.project.sequence.len();
        match action {
            NextPattern => {
                state.active_sequence_index = state
                    .active_sequence_index
                    .saturating_add(1)
                    .clamp(0, sequence_length - 1)
            }
            PrevPattern => {
                state.active_sequence_index = state
                    .active_sequence_index
                    .saturating_sub(1)
                    .clamp(0, sequence_length - 1)
            }
            Insert => state.insert_sequence(state.active_sequence_index + 1, state.active_pattern),
            Delete => {
                state.active_sequence_index = state
                    .active_sequence_index
                    .clamp(0, state.delete_sequence(state.active_sequence_index) - 1)
            }
            IncRepeats => state
                .sequence_row_mut(state.active_sequence_index)
                .increase_repeats(),
            DecRepeats => state
                .sequence_row_mut(state.active_sequence_index)
                .decrease_repeats(),
            IncPattern => state.change_sequence_pattern(state.active_sequence_index, true),
            DecPattern => state.change_sequence_pattern(state.active_sequence_index, false),
        }
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
    fn render(
        &mut self,
        state: &AppState,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
    ) {
        let layout =
            Layout::horizontal([Constraint::Length(SEQUENCE_LIST_WIDTH), Constraint::Fill(1)])
                .split(area);
        let lines = (0..state.project.sequence.len())
            .map(|i| {
                let row = &state.project.sequence[i];
                let mut line = Line::raw(format!(
                    "{:02X} {:02X} {}",
                    i, row.pattern_id.0, row.repeats
                ));
                if i == state.active_sequence_index {
                    line = line.reversed();
                }
                line
            })
            .collect::<Vec<Line>>();
        let visible_rows = layout[0].height.saturating_sub(1) | 1;
        let [list_area] = Layout::vertical([Constraint::Length(visible_rows)]).areas(layout[0]);
        let middle = (list_area.height.saturating_sub(1) / 2) as usize;
        let padded: Vec<Line> = repeat_n(Line::default(), middle).chain(lines).collect();
        let list = Paragraph::new(padded)
            .scroll((state.active_sequence_index as u16, 0))
            .centered();
        list.render(list_area, buf);
        let rows = self.keybind_rows(state);
        let (key_width, label_width) = Self::keybind_widths(&rows);
        let [keys_area, colon_area, labels_area] = Layout::horizontal([
            Constraint::Length(key_width),
            Constraint::Length(1),
            Constraint::Length(label_width),
        ])
        .areas(layout[1]);
        let (keys, labels): (Vec<Line>, Vec<Line>) = rows
            .into_iter()
            .map(|(key, label)| (Line::raw(key), Line::raw(format!(" {label}"))))
            .unzip();
        let colons = repeat_n(Line::raw(":"), keys.len()).collect::<Vec<Line>>();
        Paragraph::new(keys).right_aligned().render(keys_area, buf);
        Paragraph::new(colons).render(colon_area, buf);
        Paragraph::new(labels).render(labels_area, buf);
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
                "LEN UP",
            );
        return Self { keymap };
    }
}
impl MtrakWidget for PatternInfo {
    fn handle_event(&mut self, state: &mut AppState, event: &Event) -> bool {
        return self.dispatch(state, event);
    }
    fn render(
        &mut self,
        state: &AppState,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
    ) {
        use PatternInfoActions::*;
        let vertical_layout =
            Layout::vertical([Constraint::Fill(1), Constraint::Max(3)]).split(area);

        let font = FIGlet::small().unwrap();
        let figlet = font.convert(TITLE).take().unwrap();
        let _terminal_size = terminal::size().unwrap();
        let _title = Paragraph::new(figlet.as_str())
            .centered()
            .bold()
            .block(
                Block::bordered()
                    .borders(Borders::BOTTOM)
                    .title_style(Style::new().reversed())
                    .title_bottom("Pattern"),
            )
            .render(vertical_layout[0], buf);

        let active_pattern = state
            .project
            .patterns
            .get_pattern_by_id(state.active_pattern)
            .unwrap();
        let mut width: u16 = 0;
        let bindings = &self.keymap;
        let left_content = Paragraph::new(
            [
                format!(
                    "BPM {:03} {}/{}: DN/UP",
                    active_pattern.bpm,
                    bindings.key_for(BpmDown).unwrap(),
                    bindings.key_for(BpmUp).unwrap()
                ),
                format!(
                    "SPD {:>3} {}/{}: DN/UP",
                    active_pattern.ticks_per_line,
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
        let right_content = Paragraph::new(
            [
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
            .map(|l| Line::raw(l))
            .collect::<Vec<Line>>(),
        );
        let horizontal_layout = Layout::horizontal([Constraint::Max(width), Constraint::Fill(1)])
            .split(vertical_layout[1]);
        left_content.render(horizontal_layout[0], buf);
        right_content.render(horizontal_layout[1], buf);
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
}
impl KeyActions for PatternInfo {
    type Action = PatternInfoActions;

    fn keymap(&self) -> &Keymap<Self::Action> {
        &self.keymap
    }

    fn perform(&mut self, action: Self::Action, state: &mut AppState) {
        let pattern = state.active_pattern_mut();
        match action {
            PatternInfoActions::BpmUp => pattern.increase_bpm(),
            PatternInfoActions::BpmDown => pattern.decrease_bpm(),
            PatternInfoActions::SpeedUp => pattern.increase_ticks_per_line(),
            PatternInfoActions::SpeedDown => pattern.decrease_ticks_per_line(),
            PatternInfoActions::LineAddUp => state.increase_line_add(),
            PatternInfoActions::LineAddDown => state.decrease_line_add(),
            PatternInfoActions::PatternUp => state.next_pattern(),
            PatternInfoActions::PatternDown => state.prev_pattern(),
            PatternInfoActions::PatternLengthUp => state.active_pattern_mut().increase_length(),
            PatternInfoActions::PatternLengthDown => state.active_pattern_mut().decrease_length(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InstrumentListAction {
    SelectMidiPort,
    RenameInstrument,
    SetVolume,
    PrevInstrument,
    NextInstrument,
}

struct InstrumentList {
    keymap: Keymap<InstrumentListAction>,
    /// Where the instrument rows were last drawn, for mouse hit-testing.
    rows_area: Rect,
}
impl InstrumentList {
    fn new() -> Self {
        return Self {
            rows_area: Rect::default(),
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
                )
                .bind(Key::ctrl('l'), InstrumentListAction::SetVolume, "INST VOL")
                .bind(
                    Key::ctrl('r'),
                    InstrumentListAction::RenameInstrument,
                    "RNM INST",
                )
                .bind(Key::ctrl('o'), InstrumentListAction::SelectMidiPort, "MIDI"),
        };
    }
}
impl KeyActions for InstrumentList {
    type Action = InstrumentListAction;

    fn keymap(&self) -> &Keymap<InstrumentListAction> {
        return &self.keymap;
    }

    fn perform(&mut self, action: InstrumentListAction, state: &mut AppState) {
        match action {
            InstrumentListAction::SelectMidiPort => {
                open_midi_port_dialog(state);
            }
            InstrumentListAction::RenameInstrument => {
                open_rename_dialog(state);
            }
            InstrumentListAction::PrevInstrument => {
                state.prev_instrument();
            }
            InstrumentListAction::NextInstrument => {
                state.next_instrument();
            }
            InstrumentListAction::SetVolume => open_volume_dialog(state),
        };
    }
}
fn open_midi_port_dialog(state: &mut AppState) {
    let names = state.get_midi_port_names();
    let current = state
        .active_port
        .as_ref()
        .and_then(|p| names.iter().position(|n| *n == p.id()))
        .unwrap_or(0);
    let dialog = Dialog::new(
        "MIDI output",
        vec![Select::new("port", names).selected(current).boxed()],
    )
    .on_submit(|state, result| {
        let Some(index) = result.choice("port") else {
            return;
        };
        let port = state
            .midi_client
            .as_ref()
            .and_then(|client| client.ports().get(index).cloned());
        state.engine.set_output_port(port.clone());
        state.active_port = port;
    });
    state.open_dialog(dialog);
}
fn open_rename_dialog(state: &mut AppState) {
    let current = &state.project.instruments[state.active_instrument as usize];
    let dialog = Dialog::text_field(
        format!("Instrument {:1X} name", state.active_instrument),
        current,
        "name",
    )
    .on_submit(|state, result| {
        let Some(name) = result.text("name") else {
            return;
        };

        state.project.instruments[state.active_instrument as usize] = name.into();
    });
    state.open_dialog(dialog);
}
fn open_volume_dialog(state: &mut AppState) {
    let current = state.project.global_instrument_volumes[state.active_instrument as usize];
    let dialog = Dialog::text_field(
        format!("Instrument {:1X} volume", state.active_instrument),
        format!("{:02X}", current),
        "volume",
    )
    .on_submit(move |state, result| {
        let Some(vol) = result.text("volume") else {
            return;
        };
        let vol = u8::from_str_radix(vol, 16).unwrap_or(current);
        state.set_global_volume(state.active_instrument, vol);
    });
    state.open_dialog(dialog);
}
impl MtrakWidget for InstrumentList {
    fn handle_event(&mut self, state: &mut AppState, event: &Event) -> bool {
        if let Some(pos) = clicked_in(event, self.rows_area) {
            let index = pos.y as usize;
            if index < state.project.instruments.len() {
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
        let block = Block::bordered().borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM);

        let layout = Layout::vertical([
            Constraint::Length(2),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .split(block.inner(area));

        block.render(area, buf);
        let port = match (&state.midi_client, &state.active_port) {
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
        Paragraph::new(port)
            .left_aligned()
            .reversed()
            .bold()
            .block(
                Block::bordered()
                    .reset()
                    .borders(Borders::BOTTOM)
                    .title_bottom("Instruments")
                    .title_style(Style::new().reversed()),
            )
            .render(layout[0], buf);
        let channel_names = &state.project.instruments;
        let channel_volumes = &state.project.global_instrument_volumes;
        let selected_instrument = state.active_instrument;
        let lines = channel_names
            .iter()
            .enumerate()
            .map(|e| {
                Line::from(vec![
                    Span::raw(format!("{:1X} {:02X}", e.0, channel_volumes[e.0])).style(
                        if selected_instrument == e.0 as u8 {
                            Style::reset()
                        } else {
                            Style::new().reversed()
                        },
                    ),
                    Span::raw(format!("| {}", e.1.as_str())),
                ])
            })
            .collect::<Vec<Line>>();
        self.rows_area = layout[1];
        Paragraph::new(lines).render(layout[1], buf);

        Paragraph::new(format!("OCT: {}", state.current_octave)).render(layout[2], buf);
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
                .flat_map(|e| Self::create_lines_from_event(e))
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

        if pending.len() > 0 {
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
}
