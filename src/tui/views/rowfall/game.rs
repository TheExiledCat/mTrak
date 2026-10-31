//! A 63 piece bag, three previews, a hold piece, and squares made of four whole pieces that
//! score extra when their rows clear.
//!
//! The rules, scoring and kicks follow https://tetris.wiki/The_New_Tetris

use std::{collections::VecDeque, time::Duration};

use rand::seq::SliceRandom;

/// The field you play in, 10 columns by 20 rows.
pub const WIDTH: usize = 10;
pub const VISIBLE_HEIGHT: usize = 20;
/// The field plus the two hidden rows above it where new pieces appear.
pub const HEIGHT: usize = VISIBLE_HEIGHT + 2;
/// How many of the upcoming pieces are shown.
pub const PREVIEWS: usize = 3;
/// Each bag holds this many of every piece, 63 in all, in a random order. A new bag is
/// shuffled when one runs out, so no piece can go missing for long.
const COPIES_PER_BAG: usize = 9;
/// How long a piece can sit on something before it locks. Sliding doesn't restart it, only
/// falling to a row lower than it's been.
const LOCK_DELAY: Duration = Duration::from_millis(500);
/// The level goes up every this many cleared lines, which makes pieces fall faster.
const LINES_PER_LEVEL: u32 = 10;
/// Past this level the speed stays the same.
const MAX_LEVEL: u32 = 20;
/// A square is four whole pieces filling a 4x4 block.
const SQUARE_SIZE: usize = 4;
/// How long full lines stay on screen while they're wiped away, before the rows above drop.
const CLEAR_TIME: Duration = Duration::from_millis(250);
/// Extra points for each row of a square a cleared line goes through, on top of the line's own.
const SILVER_STRIP_POINTS: u32 = 5;
const GOLD_STRIP_POINTS: u32 = 10;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    I,
    O,
    T,
    S,
    Z,
    J,
    L,
}
const KINDS: [Kind; 7] = [
    Kind::I,
    Kind::O,
    Kind::T,
    Kind::S,
    Kind::Z,
    Kind::J,
    Kind::L,
];

impl Kind {
    pub fn cells(self, rotation: u8) -> [(i8, i8); 4] {
        let (mut cells, size) = self.spawn_cells();
        if self == Kind::O {
            return cells;
        }
        for _ in 0..rotation % 4 {
            for cell in &mut cells {
                *cell = (size - 1 - cell.1, cell.0);
            }
        }
        return cells;
    }

    /// The cells in spawn orientation, and the size of the box they turn in.
    fn spawn_cells(self) -> ([(i8, i8); 4], i8) {
        return match self {
            Kind::I => ([(0, 1), (1, 1), (2, 1), (3, 1)], 4),
            Kind::O => ([(1, 0), (2, 0), (1, 1), (2, 1)], 4),
            Kind::T => ([(1, 0), (0, 1), (1, 1), (2, 1)], 3),
            Kind::S => ([(1, 0), (2, 0), (0, 1), (1, 1)], 3),
            Kind::Z => ([(0, 0), (1, 0), (1, 1), (2, 1)], 3),
            Kind::J => ([(0, 0), (0, 1), (1, 1), (2, 1)], 3),
            Kind::L => ([(2, 0), (0, 1), (1, 1), (2, 1)], 3),
        };
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Square {
    Silver,
    Gold,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cell {
    pub kind: Kind,
    pub square: Option<(Square, u32)>,
    piece: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Piece {
    pub kind: Kind,
    pub rotation: u8,
    pub x: i8,
    pub y: i8,
}
impl Piece {
    pub fn blocks(&self) -> impl Iterator<Item = (i8, i8)> + '_ {
        return self
            .kind
            .cells(self.rotation)
            .into_iter()
            .map(|(x, y)| (self.x + x, self.y + y));
    }

    fn spawn(kind: Kind) -> Self {
        // centred, rounded to the right like the original
        let x = if kind == Kind::I || kind == Kind::O {
            3
        } else {
            4
        };
        return Self {
            kind,
            rotation: 0,
            x,
            y: 0,
        };
    }
}

pub struct Game {
    board: [[Option<Cell>; WIDTH]; HEIGHT],
    piece: Piece,
    hold: Option<Kind>,
    held_this_piece: bool,
    bag: Vec<Kind>,
    next: VecDeque<Kind>,
    pub score: u32,
    pub lines: u32,
    pub over: bool,
    next_piece_id: u32,
    next_square_id: u32,
    /// How many cells of each locked piece are still on the board.
    piece_cells: Vec<u8>,
    fall: Duration,
    lock: Option<Duration>,
    lowest_y: i8,
    /// The full rows being wiped, and how long they've been at it. The game waits meanwhile.
    clearing: Option<(Vec<usize>, Duration)>,
}

impl Game {
    pub fn new() -> Self {
        let mut game = Self {
            board: [[None; WIDTH]; HEIGHT],
            piece: Piece::spawn(Kind::I),
            hold: None,
            held_this_piece: false,
            bag: Vec::new(),
            next: VecDeque::new(),
            score: 0,
            lines: 0,
            over: false,
            next_piece_id: 0,
            next_square_id: 0,
            piece_cells: Vec::new(),
            fall: Duration::ZERO,
            lock: None,
            lowest_y: 0,
            clearing: None,
        };
        game.fill_previews();
        let first = game.take_next();
        game.spawn(first);
        return game;
    }

    pub fn cell(&self, x: usize, y: usize) -> Option<Cell> {
        return self.board[y][x];
    }
    pub fn piece(&self) -> Piece {
        return self.piece;
    }
    pub fn hold(&self) -> Option<Kind> {
        return self.hold;
    }
    pub fn previews(&self) -> impl Iterator<Item = Kind> + '_ {
        return self.next.iter().copied();
    }

    pub fn level(&self) -> u32 {
        return (1 + self.lines / LINES_PER_LEVEL).min(MAX_LEVEL);
    }

    /// How long a row takes to fall at the current level.
    pub fn gravity(&self) -> Duration {
        let level = self.level() as f64 - 1.0;
        let seconds = (0.8 - level * 0.007).powf(level);
        return Duration::from_secs_f64(seconds.max(0.01));
    }

    /// The rows being wiped and how far along it is, from 0 to 1.
    pub fn clearing(&self) -> Option<(&[usize], f32)> {
        let (rows, elapsed) = self.clearing.as_ref()?;
        let progress = elapsed.as_secs_f32() / CLEAR_TIME.as_secs_f32();
        return Some((rows, progress.min(1.0)));
    }

    /// Where the piece would land, for the ghost.
    pub fn ghost(&self) -> Piece {
        let mut ghost = self.piece;
        while self.fits(&Piece {
            y: ghost.y + 1,
            ..ghost
        }) {
            ghost.y += 1;
        }
        return ghost;
    }

    pub fn tick(&mut self, elapsed: Duration) {
        if let Some((_, time)) = &mut self.clearing {
            *time += elapsed;
            if *time >= CLEAR_TIME {
                self.finish_clearing();
            }
            return;
        }
        if self.over {
            return;
        }
        self.fall += elapsed;
        let gravity = self.gravity();
        while self.fall >= gravity {
            self.fall -= gravity;
            if !self.step_down() {
                self.fall = Duration::ZERO;
                break;
            }
        }
        if self.resting() {
            let lock = self.lock.unwrap_or_default() + elapsed;
            if lock >= LOCK_DELAY {
                self.lock_piece();
            } else {
                self.lock = Some(lock);
            }
        } else {
            self.lock = None;
        }
    }

    pub fn shift(&mut self, dx: i8) {
        let moved = Piece {
            x: self.piece.x + dx,
            ..self.piece
        };
        if !self.waiting() && self.fits(&moved) {
            self.piece = moved;
        }
    }

    /// The original's kicks: in place, then one down, one to each side and one up.
    pub fn rotate(&mut self, clockwise: bool) {
        if self.waiting() {
            return;
        }
        let rotation = match clockwise {
            true => (self.piece.rotation + 1) % 4,
            false => (self.piece.rotation + 3) % 4,
        };
        let side = if clockwise { 1 } else { -1 };
        for (dx, dy) in [(0, 0), (0, 1), (side, 0), (-side, 0), (0, -1)] {
            let turned = Piece {
                rotation,
                x: self.piece.x + dx,
                y: self.piece.y + dy,
                ..self.piece
            };
            if self.fits(&turned) {
                self.piece = turned;
                return;
            }
        }
    }

    pub fn soft_drop(&mut self) {
        if !self.waiting() && self.step_down() {
            self.fall = Duration::ZERO;
        }
    }

    /// Drops the piece to the floor without locking it, so it can still slide.
    pub fn firm_drop(&mut self) {
        if self.waiting() {
            return;
        }
        while self.step_down() {}
        self.fall = Duration::ZERO;
    }

    /// Once per piece. The held piece comes back in its spawn orientation.
    pub fn hold_piece(&mut self) {
        if self.waiting() || self.held_this_piece {
            return;
        }
        let current = self.piece.kind;
        let next = match self.hold.replace(current) {
            Some(held) => held,
            None => self.take_next(),
        };
        self.spawn(next);
        self.held_this_piece = true;
    }

    fn waiting(&self) -> bool {
        return self.over || self.clearing.is_some();
    }

    fn step_down(&mut self) -> bool {
        let lower = Piece {
            y: self.piece.y + 1,
            ..self.piece
        };
        if !self.fits(&lower) {
            return false;
        }
        self.piece = lower;
        // step reset: only reaching a new low row restarts the lock delay
        if self.piece.y > self.lowest_y {
            self.lowest_y = self.piece.y;
            self.lock = None;
        }
        return true;
    }

    fn resting(&self) -> bool {
        return !self.fits(&Piece {
            y: self.piece.y + 1,
            ..self.piece
        });
    }

    fn fits(&self, piece: &Piece) -> bool {
        return piece.blocks().all(|(x, y)| {
            (0..WIDTH as i8).contains(&x)
                && (0..HEIGHT as i8).contains(&y)
                && self.board[y as usize][x as usize].is_none()
        });
    }

    fn fill_previews(&mut self) {
        while self.next.len() < PREVIEWS {
            if self.bag.is_empty() {
                self.bag = KINDS
                    .iter()
                    .flat_map(|&kind| std::iter::repeat_n(kind, COPIES_PER_BAG))
                    .collect();
                self.bag.shuffle(&mut rand::rng());
            }
            let kind = self.bag.pop().unwrap();
            self.next.push_back(kind);
        }
    }

    fn take_next(&mut self) -> Kind {
        let kind = self.next.pop_front().unwrap();
        self.fill_previews();
        return kind;
    }

    fn spawn(&mut self, kind: Kind) {
        self.piece = Piece::spawn(kind);
        self.lowest_y = self.piece.y;
        self.fall = Duration::ZERO;
        self.lock = None;
        if !self.fits(&self.piece) {
            self.over = true;
        }
    }

    fn lock_piece(&mut self) {
        let id = self.next_piece_id;
        self.next_piece_id += 1;
        self.piece_cells.push(4);
        let blocks: Vec<(i8, i8)> = self.piece.blocks().collect();
        for &(x, y) in &blocks {
            self.board[y as usize][x as usize] = Some(Cell {
                kind: self.piece.kind,
                square: None,
                piece: id,
            });
        }
        // a block left in the rows above the field tops out, like the original
        if blocks
            .iter()
            .any(|&(_, y)| (y as usize) < HEIGHT - VISIBLE_HEIGHT)
        {
            self.over = true;
            return;
        }
        self.form_squares();
        let full: Vec<usize> = (0..HEIGHT)
            .filter(|&y| self.board[y].iter().all(Option::is_some))
            .collect();
        if full.is_empty() {
            self.next_piece();
        } else {
            self.clearing = Some((full, Duration::ZERO));
        }
    }

    fn finish_clearing(&mut self) {
        if let Some((full, _)) = self.clearing.take() {
            self.clear_lines(&full);
        }
        self.next_piece();
    }

    fn next_piece(&mut self) {
        self.held_this_piece = false;
        let next = self.take_next();
        self.spawn(next);
    }

    /// Four whole pieces filling a 4x4 area become a square: gold if they're all the same
    /// piece, silver otherwise.
    fn form_squares(&mut self) {
        for top in 0..=HEIGHT - SQUARE_SIZE {
            for left in 0..=WIDTH - SQUARE_SIZE {
                let mut pieces: Vec<(u32, Kind, u8)> = Vec::new();
                let mut whole = true;
                'area: for row in &self.board[top..top + SQUARE_SIZE] {
                    for cell in &row[left..left + SQUARE_SIZE] {
                        let Some(cell) = cell.filter(|cell| cell.square.is_none()) else {
                            whole = false;
                            break 'area;
                        };
                        match pieces.iter_mut().find(|(id, _, _)| *id == cell.piece) {
                            Some((_, _, count)) => *count += 1,
                            None => pieces.push((cell.piece, cell.kind, 1)),
                        }
                    }
                }
                let complete = |&(id, _, count): &(u32, Kind, u8)| {
                    return count == 4 && self.piece_cells.get(id as usize) == Some(&4);
                };
                if !whole || pieces.len() != 4 || !pieces.iter().all(complete) {
                    continue;
                }
                let gold = pieces.iter().all(|(_, kind, _)| *kind == pieces[0].1);
                let square = (
                    if gold { Square::Gold } else { Square::Silver },
                    self.next_square_id,
                );
                self.next_square_id += 1;
                for row in &mut self.board[top..top + SQUARE_SIZE] {
                    for cell in row[left..left + SQUARE_SIZE].iter_mut().flatten() {
                        cell.square = Some(square);
                    }
                }
            }
        }
    }

    /// A point a line and one more for four, plus 5 for each row of a silver square and 10
    /// for each row of a gold one that goes with them.
    fn clear_lines(&mut self, full: &[usize]) {
        let mut points = full.len() as u32;
        if full.len() == 4 {
            points += 1;
        }
        for &y in full {
            let mut squares: Vec<(Square, u32)> = Vec::new();
            for cell in self.board[y].iter().flatten() {
                if let Some(square) = cell.square
                    && !squares.contains(&square)
                {
                    squares.push(square);
                }
                if let Some(count) = self.piece_cells.get_mut(cell.piece as usize) {
                    *count -= 1;
                }
            }
            points += squares
                .iter()
                .map(|(square, _)| match square {
                    Square::Silver => SILVER_STRIP_POINTS,
                    Square::Gold => GOLD_STRIP_POINTS,
                })
                .sum::<u32>();
        }
        let kept: Vec<[Option<Cell>; WIDTH]> = (0..HEIGHT)
            .filter(|y| !full.contains(y))
            .map(|y| self.board[y])
            .collect();
        let mut board = [[None; WIDTH]; HEIGHT];
        board[full.len()..].copy_from_slice(&kept);
        self.board = board;
        self.lines += full.len() as u32;
        self.score += points;
    }
}

impl Default for Game {
    fn default() -> Self {
        return Self::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A game with this piece falling and an empty board.
    fn game_with(kind: Kind) -> Game {
        let mut game = Game::new();
        game.spawn(kind);
        return game;
    }

    fn drop_at(game: &mut Game, kind: Kind, rotation: u8, x: i8) {
        game.spawn(kind);
        game.piece.rotation = rotation;
        game.piece.x = x;
        game.firm_drop();
        game.lock_piece();
        if game.clearing.is_some() {
            game.finish_clearing();
        }
    }

    fn fill_row(game: &mut Game, y: usize, except: &[usize]) {
        for x in (0..WIDTH).filter(|x| !except.contains(x)) {
            game.board[y][x] = Some(Cell {
                kind: Kind::I,
                square: None,
                piece: u32::MAX,
            });
        }
    }

    #[test]
    fn a_bag_holds_nine_of_each_piece() {
        let mut game = Game::new();
        game.bag.clear();
        game.next.clear();
        game.fill_previews();
        let mut drawn: Vec<Kind> = game.next.iter().copied().collect();
        drawn.extend(game.bag.iter().copied());
        assert_eq!(drawn.len(), 63);
        for kind in KINDS {
            assert_eq!(drawn.iter().filter(|&&k| k == kind).count(), 9);
        }
    }

    #[test]
    fn four_turns_bring_a_piece_back() {
        for kind in KINDS {
            let mut cells = kind.cells(0);
            let mut turned = kind.cells(4);
            cells.sort();
            turned.sort();
            assert_eq!(cells, turned, "{kind:?}");
        }
    }

    #[test]
    fn firm_drop_lands_without_locking() {
        let mut game = game_with(Kind::O);
        game.firm_drop();
        assert_eq!(game.piece.y as usize, HEIGHT - 2);
        assert!(game.cell(4, HEIGHT - 1).is_none());
        game.shift(1);
        assert_eq!(game.piece.x, 4);
    }

    #[test]
    fn a_resting_piece_locks_after_the_delay() {
        let mut game = game_with(Kind::O);
        game.firm_drop();
        game.tick(Duration::from_millis(400));
        assert!(game.cell(4, HEIGHT - 1).is_none());
        game.tick(Duration::from_millis(150));
        assert!(game.cell(4, HEIGHT - 1).is_some());
    }

    #[test]
    fn rotating_against_a_wall_kicks_away_from_it() {
        let mut game = game_with(Kind::I);
        game.rotate(true);
        game.piece.x = -1;
        assert!(game.fits(&game.piece));
        game.rotate(true);
        assert_eq!((game.piece.rotation, game.piece.x), (2, 0));
    }

    #[test]
    fn hold_swaps_once_per_piece() {
        let mut game = game_with(Kind::T);
        let next = game.next[0];
        game.hold_piece();
        assert_eq!(game.hold, Some(Kind::T));
        assert_eq!(game.piece.kind, next);
        game.hold_piece();
        assert_eq!(game.piece.kind, next);
        assert_eq!(game.hold, Some(Kind::T));
    }

    #[test]
    fn lines_score_a_point_each_and_four_at_once_one_more() {
        let mut game = game_with(Kind::O);
        fill_row(&mut game, HEIGHT - 1, &[0]);
        drop_at(&mut game, Kind::I, 1, -2);
        assert_eq!((game.lines, game.score), (1, 1));
        let mut game = game_with(Kind::O);
        for y in HEIGHT - 4..HEIGHT {
            fill_row(&mut game, y, &[0]);
        }
        drop_at(&mut game, Kind::I, 1, -2);
        assert_eq!((game.lines, game.score), (4, 5));
    }

    #[test]
    fn four_os_make_a_gold_square_worth_45_when_its_four_rows_clear() {
        let mut game = game_with(Kind::O);
        for y in HEIGHT - 4..HEIGHT {
            fill_row(&mut game, y, &[0, 1, 2, 3, 4]);
        }
        for x in [-1, 1, -1, 1] {
            drop_at(&mut game, Kind::O, 0, x);
        }
        let square = game.cell(0, HEIGHT - 1).unwrap().square.unwrap().0;
        assert_eq!((square, game.score), (Square::Gold, 0));
        drop_at(&mut game, Kind::I, 1, 2);
        assert_eq!((game.lines, game.score), (4, 45));
    }

    #[test]
    fn mixed_pieces_make_a_silver_square() {
        let mut game = game_with(Kind::O);
        drop_at(&mut game, Kind::O, 0, -1);
        drop_at(&mut game, Kind::O, 0, 1);
        drop_at(&mut game, Kind::I, 0, 0);
        drop_at(&mut game, Kind::I, 0, 0);
        let square = game.cell(0, HEIGHT - 1).unwrap().square.unwrap().0;
        assert_eq!(square, Square::Silver);
    }

    #[test]
    fn levels_go_up_every_ten_lines_and_speed_up() {
        let mut game = Game::new();
        let slow = game.gravity();
        game.lines = 25;
        assert_eq!(game.level(), 3);
        assert!(game.gravity() < slow);
    }

    #[test]
    fn locking_in_the_rows_above_the_field_ends_the_game() {
        let mut game = game_with(Kind::O);
        for y in 2..HEIGHT {
            fill_row(&mut game, y, &[]);
        }
        game.lock_piece();
        assert!(game.over);
    }

    #[test]
    fn full_lines_wait_to_be_wiped_before_they_clear() {
        let mut game = game_with(Kind::O);
        fill_row(&mut game, HEIGHT - 1, &[0]);
        game.piece = Piece {
            kind: Kind::I,
            rotation: 1,
            x: -2,
            y: 0,
        };
        game.firm_drop();
        game.lock_piece();
        assert_eq!(
            game.clearing().map(|(rows, _)| rows.to_vec()),
            Some(vec![HEIGHT - 1])
        );
        game.shift(1);
        game.tick(Duration::from_millis(100));
        assert_eq!(game.lines, 0);
        game.tick(Duration::from_millis(200));
        assert!(game.clearing().is_none());
        assert_eq!(game.lines, 1);
    }
}
