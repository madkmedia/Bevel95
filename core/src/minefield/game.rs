//! The rules of the mine game: a field of covered squares, some hiding mines; opening a square
//! shows how many mines touch it (opening its neighbours too when none do), flags mark the
//! squares believed mined, and the game is won when every square without a mine is open.
//! Pure bookkeeping, tested without a window.

use crate::theme::Era;

/// A field's size and how many mines it hides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Field {
    pub width: usize,
    pub height: usize,
    pub mines: usize,
}

/// The fields the Game menu offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Beginner,
    Intermediate,
    Expert,
    /// Whatever the Custom Field box says.
    Custom,
}

impl Level {
    /// The levels that keep a best time.
    pub const RANKED: [Level; 3] = [Level::Beginner, Level::Intermediate, Level::Expert];

    /// Windows 2000 enlarged the Beginner field by one row and column.
    pub fn field(self, era: Era) -> Option<Field> {
        match self {
            Level::Beginner => {
                let side = Field::min_side(era);
                Some(Field { width: side, height: side, mines: 10 })
            }
            Level::Intermediate => Some(Field { width: 16, height: 16, mines: 40 }),
            Level::Expert => Some(Field { width: 30, height: 16, mines: 99 }),
            Level::Custom => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Level::Beginner => "Beginner",
            Level::Intermediate => "Intermediate",
            Level::Expert => "Expert",
            Level::Custom => "Custom",
        }
    }
}

impl Field {
    pub const MAX_WIDTH: usize = 30;
    pub const MAX_HEIGHT: usize = 24;

    pub fn min_side(era: Era) -> usize {
        if era == Era::Win2000 { 9 } else { 8 }
    }

    /// The limits applied when the original Custom Field dialog accepts the three numbers.
    pub fn custom(era: Era, width: usize, height: usize, mines: usize, most: (usize, usize)) -> Field {
        let min = Field::min_side(era);
        let width = width.clamp(min, most.0.clamp(min, Field::MAX_WIDTH));
        let height = height.clamp(min, most.1.clamp(min, Field::MAX_HEIGHT));
        Field { width, height, mines: mines.clamp(10, (width - 1) * (height - 1)) }
    }

    pub fn squares(&self) -> usize {
        self.width * self.height
    }
}

/// What lies on a square that is not open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cover {
    Covered,
    /// The player says there is a mine here: it cannot be opened.
    Flagged,
    /// The player is not sure (the "?" mark, when marks are on).
    Marked,
    Open,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Square {
    pub mine: bool,
    pub cover: Cover,
    /// How many of the squares around it hide a mine.
    pub near: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// No opening click or chord has been attempted yet.
    Ready,
    Playing,
    Won,
    Lost,
}

/// The Microsoft C runtime's 15-bit rand(), used once for each coordinate.
struct Random(u32);

impl Random {
    fn below(&mut self, n: usize) -> usize {
        self.0 = self.0.wrapping_mul(214013).wrapping_add(2531011);
        ((self.0 >> 16) & 0x7fff) as usize % n
    }
}

/// One game.
#[derive(Clone, Debug)]
pub struct Game {
    field: Field,
    squares: Vec<Square>,
    state: State,
    /// The displayed time, from 001 on the first opening attempt to 999.
    seconds: u32,
    flags: usize,
    /// Squares without a mine still to open.
    left: usize,
    /// The mine that was stepped on.
    tripped: Option<usize>,
}

impl Game {
    /// A new game on `field` (made to hold at least one square without a mine); `seed` picks
    /// where the mines go.
    pub fn new(field: Field, seed: u64) -> Game {
        let mut game = Game::empty(field);
        let mut random = Random(seed as u32);
        let mut remaining = game.field.mines;
        while remaining > 0 {
            let x = random.below(game.field.width);
            let y = random.below(game.field.height);
            let i = game.index(x, y);
            if !game.squares[i].mine {
                game.squares[i].mine = true;
                remaining -= 1;
            }
        }
        game.count_neighbours();
        game
    }

    fn empty(field: Field) -> Game {
        let width = field.width.max(1);
        let height = field.height.max(1);
        let mines = field.mines.min(width * height - 1);
        let field = Field { width, height, mines };
        Game {
            field,
            squares: vec![Square { mine: false, cover: Cover::Covered, near: 0 }; width * height],
            state: State::Ready,
            seconds: 0,
            flags: 0,
            left: width * height - mines,
            tripped: None,
        }
    }

    /// A game whose mines are already laid at `mines` (x, y): for tests and pictures.
    pub fn with_mines(width: usize, height: usize, mines: &[(usize, usize)]) -> Game {
        let mut game = Game::empty(Field { width, height, mines: mines.len() });
        for &(x, y) in mines {
            let i = game.index(x, y);
            game.squares[i].mine = true;
        }
        game.count_neighbours();
        game
    }

    pub fn field(&self) -> Field {
        self.field
    }
    pub fn state(&self) -> State {
        self.state
    }
    pub fn seconds(&self) -> u32 {
        self.seconds
    }
    pub fn square(&self, x: usize, y: usize) -> Square {
        self.squares[self.index(x, y)]
    }
    /// The mine that ended the game.
    pub fn tripped(&self) -> Option<(usize, usize)> {
        self.tripped.map(|i| (i % self.field.width, i / self.field.width))
    }
    /// Mines not flagged yet (below zero with more flags than mines).
    pub fn mines_left(&self) -> i32 {
        self.field.mines as i32 - self.flags as i32
    }
    pub fn over(&self) -> bool {
        matches!(self.state, State::Won | State::Lost)
    }

    fn index(&self, x: usize, y: usize) -> usize {
        y * self.field.width + x
    }

    /// The squares around (x, y), up to eight.
    fn around(&self, x: usize, y: usize) -> impl Iterator<Item = (usize, usize)> + use<> {
        let (w, h) = (self.field.width as i32, self.field.height as i32);
        (-1..=1)
            .flat_map(|dy| (-1..=1).map(move |dx| (dx, dy)))
            .filter(|&d| d != (0, 0))
            .map(move |(dx, dy)| (x as i32 + dx, y as i32 + dy))
            .filter(move |&(nx, ny)| nx >= 0 && ny >= 0 && nx < w && ny < h)
            .map(|(nx, ny)| (nx as usize, ny as usize))
    }

    fn count_neighbours(&mut self) {
        for y in 0..self.field.height {
            for x in 0..self.field.width {
                let near = self.around(x, y).filter(|&(nx, ny)| self.squares[self.index(nx, ny)].mine).count();
                let i = self.index(x, y);
                self.squares[i].near = near as u8;
            }
        }
    }

    /// Opens the square at (x, y) (a click). A mine ends the game; a square that no mine
    /// touches opens the squares around it, and so on. True if anything changed.
    pub fn open(&mut self, x: usize, y: usize) -> bool {
        if self.over() || x >= self.field.width || y >= self.field.height {
            return false;
        }
        self.start_clock();
        if !matches!(self.square(x, y).cover, Cover::Covered | Cover::Marked) {
            return false;
        }
        // On the first click only, move a mine to the first free square, scanning from
        // the upper left. The original search stops before the last row and column.
        if self.left == self.field.squares() - self.field.mines && self.square(x, y).mine {
            let dest = (0..self.field.height - 1)
                .flat_map(|row| (0..self.field.width - 1).map(move |col| (col, row)))
                .find(|&(col, row)| !self.square(col, row).mine)
                .map(|(col, row)| self.index(col, row))
                // At maximum mine density the whole interior can be mined. An edge square
                // is still free, and the opening click must be safe even on this layout.
                .or_else(|| self.squares.iter().position(|square| !square.mine));
            let Some(dest) = dest else {
                return false;
            };
            let from = self.index(x, y);
            self.squares[from].mine = false;
            self.squares[dest].mine = true;
            self.count_neighbours();
        }
        self.uncover(x, y);
        self.settle();
        true
    }

    /// On an open square with as many flags around it as its number: opens the other squares
    /// around it (both buttons, or the middle one). True if anything changed.
    pub fn open_around(&mut self, x: usize, y: usize) -> bool {
        if self.over() || x >= self.field.width || y >= self.field.height {
            return false;
        }
        self.start_clock();
        let square = self.square(x, y);
        let flags = self.around(x, y).filter(|&(nx, ny)| self.square(nx, ny).cover == Cover::Flagged).count();
        if square.cover != Cover::Open || flags != usize::from(square.near) {
            return false;
        }
        let covered: Vec<(usize, usize)> = self
            .around(x, y)
            .filter(|&(nx, ny)| matches!(self.square(nx, ny).cover, Cover::Covered | Cover::Marked))
            .collect();
        if covered.is_empty() {
            return false;
        }
        for (nx, ny) in covered {
            self.uncover(nx, ny);
        }
        self.settle();
        true
    }

    /// Opens one square and, from a square no mine touches, everything connected to it.
    fn uncover(&mut self, x: usize, y: usize) {
        let mut todo = vec![(x, y)];
        while let Some((x, y)) = todo.pop() {
            let i = self.index(x, y);
            if !matches!(self.squares[i].cover, Cover::Covered | Cover::Marked) {
                continue;
            }
            self.squares[i].cover = Cover::Open;
            if self.squares[i].mine {
                self.tripped.get_or_insert(i);
                continue;
            }
            self.left -= 1;
            if self.squares[i].near == 0 {
                todo.extend(self.around(x, y));
            }
        }
    }

    /// After squares were opened: lost if a mine is among them, won if only mines are left.
    fn settle(&mut self) {
        if self.tripped.is_some() {
            self.state = State::Lost;
        } else if self.left == 0 {
            self.state = State::Won;
            // The mines left unflagged get their flags, and the counter reads zero.
            for square in &mut self.squares {
                if square.mine {
                    square.cover = Cover::Flagged;
                }
            }
            self.flags = self.field.mines;
        }
    }

    fn start_clock(&mut self) {
        if self.state == State::Ready {
            self.state = State::Playing;
            self.seconds = 1;
        }
    }

    /// The right button on a covered square: a flag, then (with `marks`) a question mark, then
    /// nothing again. True if anything changed.
    pub fn mark(&mut self, x: usize, y: usize, marks: bool) -> bool {
        if self.over() || x >= self.field.width || y >= self.field.height {
            return false;
        }
        let i = self.index(x, y);
        let next = match self.squares[i].cover {
            Cover::Covered => Cover::Flagged,
            Cover::Flagged if marks => Cover::Marked,
            Cover::Flagged | Cover::Marked => Cover::Covered,
            Cover::Open => return false,
        };
        match (self.squares[i].cover, next) {
            (_, Cover::Flagged) => self.flags += 1,
            (Cover::Flagged, _) => self.flags -= 1,
            _ => {}
        }
        self.squares[i].cover = next;
        true
    }

    /// Update the displayed clock while the game is running.
    pub fn set_seconds(&mut self, seconds: u32) -> bool {
        let seconds = seconds.clamp(1, 999);
        if self.state != State::Playing || seconds == self.seconds {
            return false;
        }
        self.seconds = seconds;
        true
    }

    /// Take the final displayed clock reading.
    pub fn stop_clock(&mut self, seconds: u32) {
        if self.over() {
            self.seconds = seconds.clamp(1, 999);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 4×3 field:  . . 1 *      (mines at (3,0) and (0,2))
    ///               1 1 1 1
    ///               * 1 . .
    fn small() -> Game {
        Game::with_mines(4, 3, &[(3, 0), (0, 2)])
    }

    #[test]
    fn numbers_count_the_mines_around() {
        let g = small();
        let near: Vec<u8> =
            (0..3).flat_map(|y| (0..4).map(move |x| (x, y))).map(|(x, y)| g.square(x, y).near).collect();
        assert_eq!(near, [0, 0, 1, 0, 1, 1, 1, 1, 0, 1, 0, 0]);
        assert_eq!((g.state(), g.mines_left(), g.seconds()), (State::Ready, 2, 0));
    }

    #[test]
    fn an_empty_square_opens_its_neighbourhood() {
        let mut g = small();
        assert!(g.open(0, 0));
        // (0,0) and (1,0) touch no mine: they open everything up to the numbers around them.
        let open: Vec<(usize, usize)> = (0..3)
            .flat_map(|y| (0..4).map(move |x| (x, y)))
            .filter(|&(x, y)| g.square(x, y).cover == Cover::Open)
            .collect();
        assert_eq!(open, [(0, 0), (1, 0), (2, 0), (0, 1), (1, 1), (2, 1)]);
        assert_eq!((g.state(), g.seconds()), (State::Playing, 1));
        assert!(!g.open(0, 0), "already open");
    }

    #[test]
    fn a_mine_ends_the_game() {
        let mut g = small();
        g.open(1, 1);
        assert!(g.open(3, 0));
        assert_eq!((g.state(), g.tripped()), (State::Lost, Some((3, 0))));

        assert!(!g.open(0, 0) && !g.mark(0, 0, true) && !g.set_seconds(5), "nothing moves after the end");
    }

    #[test]
    fn opening_every_safe_square_wins_and_flags_the_mines() {
        let mut g = small();
        for (x, y) in [(0, 0), (3, 1), (1, 2), (2, 2)] {
            g.open(x, y);
        }
        assert_eq!((g.state(), g.mines_left()), (State::Won, 0));
        assert_eq!((g.square(3, 0).cover, g.square(0, 2).cover), (Cover::Flagged, Cover::Flagged));
    }

    #[test]
    fn the_right_button_goes_round_flag_mark_nothing() {
        let mut g = small();
        g.mark(3, 0, true);
        assert_eq!((g.square(3, 0).cover, g.mines_left()), (Cover::Flagged, 1));
        assert!(!g.open(3, 0), "a flagged square does not open");
        g.mark(3, 0, true);
        assert_eq!((g.square(3, 0).cover, g.mines_left()), (Cover::Marked, 2));
        g.mark(3, 0, true);
        assert_eq!(g.square(3, 0).cover, Cover::Covered);
        // without marks: flag, then nothing
        g.mark(3, 0, false);
        g.mark(3, 0, false);
        assert_eq!(g.square(3, 0).cover, Cover::Covered);
        // more flags than mines: the counter goes below zero
        for x in 0..3 {
            g.mark(x, 0, false);
        }
        assert_eq!(g.mines_left(), -1);
        g.mark(1, 1, true);
        g.mark(1, 1, true);
        g.mark(1, 1, false);
        assert_eq!(g.square(1, 1).cover, Cover::Covered);
    }

    #[test]
    fn both_buttons_open_around_a_satisfied_number() {
        let mut g = small();
        g.open(2, 1); // a "1": the mine at (3,0) touches it
        assert!(!g.open_around(2, 1), "no flag yet");
        g.mark(3, 0, false);
        assert!(g.open_around(2, 1));
        assert_eq!(g.square(3, 2).cover, Cover::Open);
        assert_eq!(g.state(), State::Won, "every square without a mine is open now");
        // a wrong flag makes it step on the mine
        let mut g = small();
        g.open(2, 1);
        g.mark(3, 1, false);
        assert!(g.open_around(2, 1));
        assert_eq!((g.state(), g.tripped()), (State::Lost, Some((3, 0))));

        // A flag can hold a safe neighbour of a blank closed during the flood.
        let mut g = small();
        g.mark(1, 1, false);
        g.open(0, 0);
        g.mark(1, 1, false);
        assert!(g.open_around(0, 0));
        assert_eq!(g.square(1, 1).cover, Cover::Open);
    }

    #[test]
    fn the_first_mine_moves_to_the_first_free_square() {
        let mut g = Game::with_mines(4, 4, &[(0, 0), (2, 1), (3, 2)]);
        g.mark(1, 0, true);
        // A click on a flag starts the timer but leaves first-reveal protection intact.
        assert!(!g.open(1, 0));
        assert_eq!((g.state(), g.seconds()), (State::Playing, 1));
        assert!(g.open(2, 1));
        assert_eq!(g.state(), State::Playing);
        assert_eq!(g.square(2, 1), Square { mine: false, cover: Cover::Open, near: 2 });
        assert!(g.square(1, 0).mine, "a flag does not change the relocation order");
        assert_eq!(g.mines_left(), 2);
        // The next mine ends the game.
        g.open(3, 2);
        assert_eq!(g.state(), State::Lost);

        let mines: Vec<_> = (0..7).flat_map(|y| (0..7).map(move |x| (x, y))).collect();
        let mut g = Game::with_mines(8, 8, &mines);
        assert!(g.open(1, 1));
        assert_eq!(g.square(1, 1), Square { mine: false, cover: Cover::Open, near: 8 });
        assert!(g.square(7, 0).mine);

        // different seeds, different fields
        let field = |seed| {
            let g = Game::new(Level::Expert.field(Era::Win95).unwrap(), seed);
            (0..16).flat_map(|y| (0..30).map(move |x| (x, y))).map(|(x, y)| g.square(x, y).mine).collect::<Vec<_>>()
        };
        assert_ne!(field(1), field(2));
        assert_eq!(field(7), field(7));
        assert_eq!(field(7).iter().filter(|&&mine| mine).count(), 99);
    }

    #[test]
    fn the_clock_runs_from_the_first_square_to_999() {
        let mut g = small();
        assert!(!g.set_seconds(3), "not before the first square");
        g.open(1, 1);
        assert_eq!(g.seconds(), 1);
        assert!(!g.set_seconds(1) && g.set_seconds(2));
        g.set_seconds(5000);
        assert_eq!(g.seconds(), 999);
        // the move that ends the game takes the clock's last reading
        let mut g = small();
        g.open(1, 1);
        g.open(3, 0);
        g.stop_clock(12);
        assert_eq!((g.state(), g.seconds()), (State::Lost, 12));
    }

    #[test]
    fn levels_and_custom_fields_keep_to_the_limits() {
        let roomy = (Field::MAX_WIDTH, Field::MAX_HEIGHT);
        for (era, min) in [(Era::Win95, 8), (Era::Win98, 8), (Era::Win2000, 9)] {
            assert_eq!(Level::Beginner.field(era), Some(Field { width: min, height: min, mines: 10 }));
            assert_eq!(Level::Expert.field(era), Some(Field { width: 30, height: 16, mines: 99 }));
            assert_eq!(Level::Custom.field(era), None);
            assert_eq!(Field::custom(era, 1, 99, 0, roomy), Field { width: min, height: 24, mines: 10 });
            assert_eq!(Field::custom(era, 99, 99, 5000, roomy), Field { width: 30, height: 24, mines: 667 });
            assert_eq!(Field::custom(era, 99, 99, 20, (25, 3)), Field { width: 25, height: min, mines: 20 });
        }
    }
}
