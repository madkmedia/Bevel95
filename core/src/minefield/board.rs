//! Minesweeper's client area. The three Windows releases share the panel and grid offsets.
//! Windows 95 and 98 show the entire raised outer border. Windows 2000's smaller client
//! ends before its bottom and right edges.

use crate::fb::{Framebuffer, Rect};
use crate::minefield::art::{self, Face, Tile};
use crate::minefield::game::{Cover, Game, Square, State};
use crate::theme::{Era, rgb};
use crate::window;

pub const SQUARE: i32 = 16;
const FIELD_EDGE: i32 = 3;
const FACE: u32 = rgb(0xC0C0C0);
const HIGHLIGHT: u32 = rgb(0xFFFFFF);
const SHADOW: u32 = rgb(0x808080);

/// Layout in window coordinates, including the non-client frame and menus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Board {
    pub era: Era,
    pub width: usize,
    pub height: usize,
    /// The 95/98 startup window is at least 160px wide, leaving two extra columns for
    /// an eight-column field. Selecting a different difficulty sizes it tightly.
    pub initial: bool,
}

impl Board {
    fn startup_padding(&self) -> i32 {
        if self.initial && self.width == 8 && self.era != Era::Win2000 { 2 } else { 0 }
    }

    fn client(&self) -> Rect {
        let (extra_width, extra_height) = match self.era {
            Era::Win95 | Era::Win98 => (24 + self.startup_padding(), 67),
            Era::Win2000 => (20, 63),
        };
        Rect::new(
            window::FRAME,
            window::CLIENT_Y_WITH_MENU,
            self.width as i32 * SQUARE + extra_width,
            self.height as i32 * SQUARE + extra_height,
        )
    }

    pub fn window_size(&self) -> (i32, i32) {
        let r = self.client();
        window::outer_size((r.w, r.h))
    }

    /// The largest custom field that will fit after choosing a new size.
    pub fn most_squares(era: Era, space: (i32, i32)) -> (usize, usize) {
        let (w0, h0) = Board { era, width: 0, height: 0, initial: false }.window_size();
        (((space.0 - w0) / SQUARE).max(0) as usize, ((space.1 - h0) / SQUARE).max(0) as usize)
    }

    fn panel(&self) -> Rect {
        Rect::new(window::FRAME + 9, window::CLIENT_Y_WITH_MENU + 9, self.width as i32 * SQUARE + 6, 37)
    }

    pub fn mines(&self) -> Rect {
        Rect::new(window::FRAME + 16, window::CLIENT_Y_WITH_MENU + 15, 41, 25)
    }

    pub fn clock(&self) -> Rect {
        Rect::new(window::FRAME + self.width as i32 * SQUARE - 35, window::CLIENT_Y_WITH_MENU + 15, 41, 25)
    }

    /// The face button's outline is one pixel outside its 24×24 bitmap.
    pub fn button(&self) -> Rect {
        Rect::new(window::FRAME + self.width as i32 * SQUARE / 2 - 1, window::CLIENT_Y_WITH_MENU + 15, 26, 26)
    }

    pub fn field(&self) -> Rect {
        Rect::new(
            window::FRAME + 9,
            window::CLIENT_Y_WITH_MENU + 52,
            self.width as i32 * SQUARE + 2 * FIELD_EDGE,
            self.height as i32 * SQUARE + 2 * FIELD_EDGE,
        )
    }

    pub fn grid(&self) -> Rect {
        self.field().inset(FIELD_EDGE, FIELD_EDGE, FIELD_EDGE, FIELD_EDGE)
    }

    pub fn square(&self, x: usize, y: usize) -> Rect {
        let grid = self.grid();
        Rect::new(grid.x + x as i32 * SQUARE, grid.y + y as i32 * SQUARE, SQUARE, SQUARE)
    }

    pub fn square_at(&self, px: i32, py: i32) -> Option<(usize, usize)> {
        let grid = self.grid();
        grid.contains(px, py).then(|| (((px - grid.x) / SQUARE) as usize, ((py - grid.y) / SQUARE) as usize))
    }
}

/// The squares shown pushed in while the mouse is held on the field.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Held {
    #[default]
    Nothing,
    One(usize, usize),
    /// Both buttons (or the middle one) on a square: it and the eight around it.
    Around(usize, usize),
}

impl Held {
    fn covers(self, x: usize, y: usize) -> bool {
        match self {
            Held::Nothing => false,
            Held::One(hx, hy) => (hx, hy) == (x, y),
            Held::Around(hx, hy) => hx.abs_diff(x) <= 1 && hy.abs_diff(y) <= 1,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct View<'a> {
    pub game: &'a Game,
    pub held: Held,
    pub button_down: bool,
}

impl View<'_> {
    fn face(&self) -> Face {
        if self.button_down {
            return Face::Down;
        }
        match self.game.state() {
            State::Won => Face::Won,
            State::Lost => Face::Lost,
            _ if self.held != Held::Nothing => Face::Worried,
            _ => Face::Waiting,
        }
    }
}

/// Each bevel layer leaves its top-right and bottom-left pixels the background color.
/// This also applies to the one-pixel outline around the face.
fn bevel(fb: &mut Framebuffer, mut r: Rect, depth: i32, top_left: u32, bottom_right: u32) {
    for _ in 0..depth {
        fb.hline(r.x, r.y, r.w - 1, top_left);
        fb.vline(r.x, r.y, r.h - 1, top_left);
        fb.hline(r.x + 1, r.bottom() - 1, r.w - 1, bottom_right);
        fb.vline(r.right() - 1, r.y + 1, r.h - 1, bottom_right);
        r = r.inset(1, 1, 1, 1);
    }
}

fn digits(value: i32) -> [u8; 3] {
    // A negative mine count uses the first digit for the sign and displays the last
    // two digits of the magnitude, even when more than 99 excess flags are placed.
    let n = value.min(999).unsigned_abs();
    [if value < 0 { art::MINUS } else { (n / 100) as u8 }, ((n / 10) % 10) as u8, (n % 10) as u8]
}

fn counter(fb: &mut Framebuffer, r: Rect, color: bool, shadow: u32, value: i32) {
    bevel(fb, r, 1, shadow, HIGHLIGHT);
    for (i, digit) in digits(value).into_iter().enumerate() {
        art::digit(fb, r.x + 1 + 13 * i as i32, r.y + 1, color, digit);
    }
}

fn tile(square: Square, lost: bool, held: bool) -> Tile {
    match square.cover {
        Cover::Flagged if lost && !square.mine => Tile::WrongFlag,
        Cover::Flagged => Tile::Flagged,
        // A failed chord can reveal several mines in the same move. Each has a red tile.
        Cover::Open if square.mine => Tile::Tripped,
        _ if lost && square.mine => Tile::Mine,
        Cover::Open => Tile::Open(square.near),
        Cover::Marked if held => Tile::MarkedDown,
        Cover::Marked => Tile::Marked,
        Cover::Covered if held => Tile::Open(0),
        Cover::Covered => Tile::Covered,
    }
}

pub fn paint(fb: &mut Framebuffer, board: &Board, view: &View, color: bool) {
    let client = board.client();
    let shadow = if color { SHADOW } else { rgb(0x000000) };
    fb.fill(client, FACE);
    match board.era {
        Era::Win95 | Era::Win98 => {
            let surround = client.inset(0, 0, board.startup_padding(), 0);
            bevel(fb, surround, 3, HIGHLIGHT, shadow);
        }
        Era::Win2000 => {
            fb.fill(Rect::new(client.x, client.y, client.w, 3), HIGHLIGHT);
            fb.fill(Rect::new(client.x, client.y, 3, client.h), HIGHLIGHT);
        }
    }
    bevel(fb, board.panel(), 2, shadow, HIGHLIGHT);
    counter(fb, board.mines(), color, shadow, view.game.mines_left());
    counter(fb, board.clock(), color, shadow, view.game.seconds() as i32);
    let button = board.button();
    bevel(fb, button, 1, shadow, shadow);
    art::face(fb, button.x + 1, button.y + 1, board.era, color, view.face());

    bevel(fb, board.field(), FIELD_EDGE, shadow, HIGHLIGHT);
    let game = view.game;
    let held = if game.over() { Held::Nothing } else { view.held };
    for y in 0..board.height {
        for x in 0..board.width {
            let t = tile(game.square(x, y), game.state() == State::Lost, held.covers(x, y));
            let r = board.square(x, y);
            art::tile(fb, r.x, r.y, board.era, color, t);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_sizes_match_the_reference_pcs() {
        for era in [Era::Win95, Era::Win98] {
            let b = Board { era, width: 8, height: 8, initial: true };
            assert_eq!(b.window_size(), (160, 239));
            assert_eq!(Board { initial: false, ..b }.window_size(), (158, 239));
            assert_eq!(Board { width: 16, height: 16, initial: false, ..b }.window_size(), (286, 367));
            assert_eq!(Board { width: 16, height: 16, ..b }.window_size(), (286, 367));
            assert_eq!(Board { width: 30, height: 16, initial: false, ..b }.window_size(), (510, 367));
        }
        let b = Board { era: Era::Win2000, width: 9, height: 9, initial: true };
        assert_eq!(b.window_size(), (170, 251));
        assert_eq!(Board { width: 30, height: 16, ..b }.window_size(), (506, 363));
        assert_eq!(Board::most_squares(b.era, (170, 251)), (9, 9));
        assert_eq!(Board::most_squares(b.era, (169, 250)), (8, 8));
        assert_eq!(Board::most_squares(b.era, (10, 10)), (0, 0));
    }

    #[test]
    fn a_point_finds_its_square() {
        let b = Board { era: Era::Win2000, width: 9, height: 9, initial: true };
        let first = b.square(0, 0);
        assert_eq!(first, Rect::new(15, 96, 16, 16));
        assert_eq!(b.square_at(first.x, first.y), Some((0, 0)));
        assert_eq!(b.square_at(first.x + 15, first.y + 16), Some((0, 1)));
        assert_eq!(b.square_at(first.x - 1, first.y), None);
        assert_eq!(b.square_at(first.x + 9 * 16, first.y), None);
        assert_eq!(b.square_at(first.x + 9 * 16 - 1, first.y + 9 * 16 - 1), Some((8, 8)));
    }

    #[test]
    fn counters_show_three_places() {
        assert_eq!(digits(10), [0, 1, 0]);
        assert_eq!(digits(999), [9, 9, 9]);
        assert_eq!(digits(-7), [art::MINUS, 0, 7]);
        assert_eq!(digits(-100), [art::MINUS, 0, 0]);
        assert_eq!(digits(-123), [art::MINUS, 2, 3]);
    }

    #[test]
    fn a_failed_chord_highlights_every_mine_it_opened() {
        let mut game = Game::with_mines(4, 3, &[(0, 0), (2, 0), (3, 2)]);
        game.open(1, 1);
        game.mark(0, 2, false);
        game.mark(2, 2, false);
        game.open_around(1, 1);
        assert_eq!(game.state(), State::Lost);
        let shown = |x, y| tile(game.square(x, y), true, false);
        assert_eq!(shown(0, 0), Tile::Tripped);
        assert_eq!(shown(2, 0), Tile::Tripped);
        assert_eq!(shown(3, 2), Tile::Mine);
        assert_eq!(shown(0, 2), Tile::WrongFlag);
    }
}
