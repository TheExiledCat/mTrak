pub mod game;
pub mod leaderboard;

use std::time::Instant;

use chrono::Local;
use ratatui::{
    buffer::Buffer,
    crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers},
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::Line,
    widgets::{Block, Paragraph, Widget},
};

use crate::{
    error::Result,
    tui::{
        app::AppState,
        files,
        framework::{
            keymap::{Action, HelpSection, Key, KeyActions, KeyHint, Keymap},
            widget::MtrakWidget,
            widgets::footer::Footer,
        },
        glyphs,
        views::rowfall::{
            game::{Game, Kind, Piece, Square, VISIBLE_HEIGHT, WIDTH},
            leaderboard::{Entry, Leaderboard, MAX_NAME_LENGTH},
        },
    },
};

const HIDDEN_ROWS: usize = game::HEIGHT - VISIBLE_HEIGHT;
const BOARD_WIDTH: u16 = WIDTH as u16 * 2 + 2;
const BOARD_HEIGHT: u16 = VISIBLE_HEIGHT as u16 + 2;
const PANEL_WIDTH: u16 = 18;
const BEST_SHOWN: usize = 10;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RowfallAction {
    Left,
    Right,
    SoftDrop,
    FirmDrop,
    RotateLeft,
    RotateRight,
    Hold,
    Pause,
    NewGame,
    SaveName,
    Back,
}
impl Action for RowfallAction {}

enum Phase {
    Playing,
    EnteringName(String),
    Over,
}

pub struct RowfallView {
    open: bool,
    game: Game,
    phase: Phase,
    paused: bool,
    last_tick: Instant,
    leaderboard: Leaderboard,
    /// The place of the last saved score, to show it on the leaderboard.
    saved_place: Option<usize>,
    playing_keys: Keymap<RowfallAction>,
    name_keys: Keymap<RowfallAction>,
    over_keys: Keymap<RowfallAction>,
    footer: Footer,
}

impl RowfallView {
    pub fn new() -> Self {
        use RowfallAction::*;
        let back = |keys: Keymap<RowfallAction>| keys.bind(Key::plain(KeyCode::Esc), Back, "Back");
        return Self {
            open: false,
            game: Game::new(),
            phase: Phase::Playing,
            paused: false,
            last_tick: Instant::now(),
            leaderboard: Leaderboard::new(),
            saved_place: None,
            playing_keys: back(
                Keymap::new("rowfall")
                    .bind(Key::plain(KeyCode::Left), Left, "Move")
                    .bind(Key::plain(KeyCode::Right), Right, "Move")
                    .bind(Key::char('x'), RotateRight, "Turn")
                    .bind(Key::char('z'), RotateLeft, "Turn")
                    .bind(Key::plain(KeyCode::Down), SoftDrop, "Drop")
                    .bind(Key::plain(KeyCode::Up), FirmDrop, "Floor")
                    .bind(Key::char(' '), Hold, "Hold")
                    .bind(Key::char('p'), Pause, "Pause"),
            ),
            name_keys: back(Keymap::new("rowfall").bind(
                Key::plain(KeyCode::Enter),
                SaveName,
                "Save",
            )),
            over_keys: back(Keymap::new("rowfall").bind(
                Key::plain(KeyCode::Enter),
                NewGame,
                "New game",
            )),
            footer: Footer::new(),
        };
    }

    pub fn is_open(&self) -> bool {
        return self.open;
    }

    /// The game waits while it's closed, and carries on where it was when it opens again.
    pub fn toggle(&mut self, state: &mut AppState) {
        self.open = !self.open;
        if !self.open {
            return;
        }
        self.last_tick = Instant::now();
        match files::get_leaderboard_file_path().and_then(|path| Leaderboard::load(&path)) {
            Ok(leaderboard) => self.leaderboard = leaderboard,
            Err(e) => state.report_error(e),
        }
    }

    fn new_game(&mut self) {
        self.game = Game::new();
        self.phase = Phase::Playing;
        self.paused = false;
        self.saved_place = None;
        self.last_tick = Instant::now();
    }

    fn game_over(&mut self) {
        self.phase = match self.leaderboard.qualifies(self.game.score) {
            true => Phase::EnteringName(String::new()),
            false => Phase::Over,
        };
    }

    fn save_score(&mut self, state: &mut AppState, name: String) {
        let name = match name.trim() {
            "" => "???".to_owned(),
            name => name.to_owned(),
        };
        let entry = Entry {
            name,
            score: self.game.score,
            lines: self.game.lines,
            level: self.game.level(),
            date: Local::now().format("%Y-%m-%d").to_string(),
        };
        self.saved_place = Some(self.leaderboard.add(entry));
        self.phase = Phase::Over;
        let saved =
            files::get_leaderboard_file_path().and_then(|path| self.leaderboard.save(&path));
        if let Err(e) = saved {
            state.report_error(e);
        }
    }

    fn type_name(&mut self, code: KeyCode) -> bool {
        let Phase::EnteringName(name) = &mut self.phase else {
            return false;
        };
        match code {
            KeyCode::Backspace => {
                name.pop();
            }
            KeyCode::Char(c) if !c.is_control() && name.chars().count() < MAX_NAME_LENGTH => {
                name.push(c);
            }
            _ => return false,
        }
        return true;
    }

    fn render_header(&self, state: &AppState, area: Rect, buf: &mut Buffer) {
        let back = self.playing_keys.key_text(RowfallAction::Back);
        let text =
            format!(" ROWFALL   {back}: back to mTrak, which keeps playing in the background");
        Line::raw(text)
            .style(state.theme.selection)
            .render(area, buf);
    }

    fn render_board(&self, state: &AppState, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered().border_style(state.theme.border);
        let inner = block.inner(area);
        block.render(area, buf);
        let clearing = self.game.clearing();
        let falling = !self.game.over && clearing.is_none();
        let piece = self.game.piece();
        let ghost = self.game.ghost();
        for y in 0..VISIBLE_HEIGHT {
            for x in 0..WIDTH {
                let row = y + HIDDEN_ROWS;
                let (text, style) = match self.game.cell(x, row) {
                    Some(cell) => cell_look(state, cell.kind, cell.square.map(|s| s.0)),
                    None if falling && covers(&piece, x, row) => cell_look(state, piece.kind, None),
                    None if falling && covers(&ghost, x, row) => (
                        glyphs::pick("\u{2591}\u{2591}", "::"),
                        state.theme.border_subtle,
                    ),
                    None => ("  ", Style::new()),
                };
                buf.set_string(inner.x + x as u16 * 2, inner.y + y as u16, text, style);
            }
        }
        if let Some((rows, progress)) = clearing {
            for &row in rows.iter().filter(|&&row| row >= HIDDEN_ROWS) {
                let y = inner.y + (row - HIDDEN_ROWS) as u16;
                render_wipe(state, inner.x, y, progress, buf);
            }
        }
        let message = match &self.phase {
            Phase::Playing if self.paused => Some("PAUSED".to_owned()),
            Phase::Playing => None,
            Phase::EnteringName(name) => Some(format!("NEW BEST!\n\nName: {name}_")),
            Phase::Over => Some("GAME OVER".to_owned()),
        };
        if let Some(message) = message {
            let height = message.lines().count() as u16;
            let [_, middle, _] = Layout::vertical([
                Constraint::Fill(1),
                Constraint::Length(height),
                Constraint::Fill(1),
            ])
            .areas(inner);
            Paragraph::new(message)
                .centered()
                .style(state.theme.highlight)
                .render(middle, buf);
        }
    }

    fn render_left(&self, state: &AppState, area: Rect, buf: &mut Buffer) {
        let [hold_area, stats_area] =
            Layout::vertical([Constraint::Length(6), Constraint::Fill(1)]).areas(area);
        let block = Block::bordered()
            .title("Hold")
            .border_style(state.theme.border);
        let inner = block.inner(hold_area);
        block.render(hold_area, buf);
        if let Some(kind) = self.game.hold() {
            render_shape(state, kind, inner, buf);
        }
        let stats = [
            ("Score", self.game.score),
            ("Lines", self.game.lines),
            ("Level", self.game.level()),
        ];
        let lines: Vec<Line> = stats
            .iter()
            .flat_map(|(label, value)| {
                [
                    Line::raw(*label).style(state.theme.label),
                    Line::raw(format!(" {value}")),
                    Line::default(),
                ]
            })
            .collect();
        Paragraph::new(lines).render(stats_area.inner(ratatui::layout::Margin::new(1, 1)), buf);
    }

    fn render_right(&self, state: &AppState, area: Rect, buf: &mut Buffer) {
        let previews = game::PREVIEWS as u16;
        let [next_area, best_area] =
            Layout::vertical([Constraint::Length(previews * 3 + 2), Constraint::Fill(1)])
                .areas(area);
        let block = Block::bordered()
            .title("Next")
            .border_style(state.theme.border);
        let inner = block.inner(next_area);
        block.render(next_area, buf);
        for (index, kind) in self.game.previews().enumerate() {
            let slot = Rect::new(inner.x, inner.y + index as u16 * 3, inner.width, 2);
            render_shape(state, kind, slot, buf);
        }
        let block = Block::bordered()
            .title("Best")
            .border_style(state.theme.border);
        let inner = block.inner(best_area);
        block.render(best_area, buf);
        let lines: Vec<Line> = self
            .leaderboard
            .entries
            .iter()
            .take(BEST_SHOWN)
            .enumerate()
            .map(|(place, entry)| {
                let name: String = entry.name.chars().take(8).collect();
                let line = Line::raw(format!("{name:<8}{:>7}", entry.score));
                match Some(place) == self.saved_place {
                    true => line.style(state.theme.highlight),
                    false => line,
                }
            })
            .collect();
        Paragraph::new(lines).render(inner, buf);
    }
}

/// Two carets that start in the middle of a full row and run to its ends, wiping the row as
/// they go.
fn render_wipe(state: &AppState, x: u16, y: u16, progress: f32, buf: &mut Buffer) {
    let half = WIDTH as u16;
    let reach = (progress * half as f32).round() as u16;
    let wiped = Rect::new(x + half - reach, y, reach * 2, 1);
    buf.set_string(wiped.x, y, " ".repeat(wiped.width as usize), Style::new());
    let style = state.theme.heading;
    if reach < half {
        buf.set_string(x + half - reach - 1, y, "<", style);
        buf.set_string(x + half + reach, y, ">", style);
    }
}

fn covers(piece: &Piece, x: usize, y: usize) -> bool {
    return piece
        .blocks()
        .any(|(bx, by)| bx as usize == x && by as usize == y);
}

fn piece_style(state: &AppState, kind: Kind) -> Style {
    let index = match kind {
        Kind::I => 0,
        Kind::O => 1,
        Kind::T => 2,
        Kind::S => 3,
        Kind::Z => 4,
        Kind::J => 5,
        Kind::L => 6,
    };
    return state.theme.pieces[index];
}

fn cell_look(state: &AppState, kind: Kind, square: Option<Square>) -> (&'static str, Style) {
    return match square {
        Some(Square::Gold) => (
            glyphs::pick("\u{2588}\u{2588}", "##"),
            piece_style(state, Kind::O),
        ),
        Some(Square::Silver) => (glyphs::pick("\u{2593}\u{2593}", "%%"), state.theme.text),
        None => (
            glyphs::pick("\u{2588}\u{2588}", "##"),
            piece_style(state, kind),
        ),
    };
}

fn render_shape(state: &AppState, kind: Kind, area: Rect, buf: &mut Buffer) {
    let style = piece_style(state, kind);
    let cells = kind.cells(0);
    let left = cells.iter().map(|c| c.0).min().unwrap_or(0);
    let top = cells.iter().map(|c| c.1).min().unwrap_or(0);
    let width = (cells.iter().map(|c| c.0).max().unwrap_or(0) - left + 1) as u16 * 2;
    let x0 = area.x + area.width.saturating_sub(width) / 2;
    for (x, y) in cells {
        let x = x0 + (x - left) as u16 * 2;
        let y = area.y + (y - top) as u16;
        if y < area.bottom() {
            buf.set_string(x, y, glyphs::pick("\u{2588}\u{2588}", "##"), style);
        }
    }
}

impl Default for RowfallView {
    fn default() -> Self {
        return Self::new();
    }
}

impl KeyActions for RowfallView {
    type Action = RowfallAction;

    fn keymap(&self) -> &Keymap<RowfallAction> {
        return match self.phase {
            Phase::Playing => &self.playing_keys,
            Phase::EnteringName(_) => &self.name_keys,
            Phase::Over => &self.over_keys,
        };
    }

    fn perform(&mut self, action: RowfallAction, state: &mut AppState) -> Result<()> {
        use RowfallAction::*;
        let playing = matches!(self.phase, Phase::Playing) && !self.paused;
        match action {
            Back => self.toggle(state),
            Pause => self.paused = !self.paused,
            NewGame => self.new_game(),
            SaveName => {
                if let Phase::EnteringName(name) = std::mem::replace(&mut self.phase, Phase::Over) {
                    self.save_score(state, name);
                }
            }
            _ if !playing => {}
            Left => self.game.shift(-1),
            Right => self.game.shift(1),
            RotateLeft => self.game.rotate(false),
            RotateRight => self.game.rotate(true),
            SoftDrop => self.game.soft_drop(),
            FirmDrop => self.game.firm_drop(),
            Hold => self.game.hold_piece(),
        }
        return Ok(());
    }

    fn describe(action: RowfallAction) -> &'static str {
        use RowfallAction::*;
        return match action {
            Left => "Move the piece left",
            Right => "Move the piece right",
            SoftDrop => "Move the piece down a row",
            FirmDrop => "Drop the piece to the floor, where it can still slide",
            RotateLeft => "Turn the piece anticlockwise",
            RotateRight => "Turn the piece clockwise",
            Hold => "Hold the piece, or swap it with the held one",
            Pause => "Pause or carry on",
            NewGame => "Start a new game",
            SaveName => "Save your score under this name",
            Back => "Go back to mTrak; the game waits",
        };
    }
}

impl MtrakWidget for RowfallView {
    fn update(&mut self, _state: &mut AppState) {
        let now = Instant::now();
        let elapsed = now - self.last_tick;
        self.last_tick = now;
        if !self.open || self.paused || !matches!(self.phase, Phase::Playing) {
            return;
        }
        self.game.tick(elapsed);
        if self.game.over {
            self.game_over();
        }
    }

    fn handle_event(&mut self, state: &mut AppState, event: &Event) -> bool {
        if !self.open {
            return false;
        }
        if self.dispatch(state, event) {
            return true;
        }
        if let Event::Key(key) = event
            && key.kind != KeyEventKind::Release
            && !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            self.type_name(key.code);
        }
        return true;
    }

    fn key_hints(&self, _state: &AppState, hints: &mut Vec<KeyHint>) {
        self.keymap().hints(hints);
    }

    fn help(&self, sections: &mut Vec<HelpSection>) {
        let mut section = self.help_section("Rowfall");
        let back = Self::describe(RowfallAction::Back);
        for keys in [&self.name_keys, &self.over_keys] {
            let entries = keys.help_entries(Self::describe);
            section = section.entries(
                entries
                    .into_iter()
                    .filter(|e| e.description != back)
                    .collect(),
            );
        }
        sections.push(section);
    }

    fn render(&mut self, state: &AppState, area: Rect, buf: &mut Buffer) {
        buf.set_style(area, state.theme.surface);
        let [header, body, footer] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(area);
        self.render_header(state, header, buf);
        let mut hints = vec![];
        self.key_hints(state, &mut hints);
        self.footer.set_hints(hints);
        self.footer.render(state, footer, buf);
        let width = BOARD_WIDTH + PANEL_WIDTH * 2 + 2;
        if body.width < width || body.height < BOARD_HEIGHT {
            Paragraph::new("Make the window bigger to play")
                .centered()
                .render(body, buf);
            return;
        }
        let [_, middle, _] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(BOARD_HEIGHT),
            Constraint::Fill(1),
        ])
        .areas(body);
        let [_, left, _, board, _, right, _] = Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Length(PANEL_WIDTH),
            Constraint::Length(1),
            Constraint::Length(BOARD_WIDTH),
            Constraint::Length(1),
            Constraint::Length(PANEL_WIDTH),
            Constraint::Fill(1),
        ])
        .areas(middle);
        self.render_left(state, left, buf);
        self.render_board(state, board, buf);
        self.render_right(state, right, buf);
    }
}
