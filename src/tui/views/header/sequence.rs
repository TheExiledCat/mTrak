use std::iter::repeat_n;

use ratatui::{
    crossterm::event::{
        KeyCode::{self},
        KeyModifiers,
    },
    layout::{Constraint, Layout, Rect},
    text::Line,
    widgets::{Paragraph, Widget},
};

use crate::{
    error::Result,
    tui::{
        app::AppState,
        framework::{
            keymap::{Action, Key, KeyActions, Keymap},
            widget::{Collapsible, MtrakWidget},
        },
        views::header::sequence::SequenceAction::*,
    },
    util::time::duration_to_hh_mm_ss_string,
};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SequenceAction {
    NextPattern,
    PrevPattern,
    Insert,
    Delete,
    IncPattern,
    DecPattern,
    IncRepeats,
    DecRepeats,
}

impl Action for SequenceAction {}

const SEQUENCE_LIST_WIDTH: u16 = 9;

pub struct SequenceList {
    keymap: Keymap<SequenceAction>,
}

impl SequenceList {
    pub fn new() -> Self {
        let keymap = Keymap::new("sequence")
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
        let key = |action| self.keymap.key_text(action);
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
                    line = line.style(state.theme.label);
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

    pub fn width(&self, state: &AppState) -> u16 {
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
