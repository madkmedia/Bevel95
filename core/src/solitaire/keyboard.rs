//! The keyboard moves the real pointer between piles. Picking up a card uses the same
//! drag and drop as a mouse press, including when the player switches back to the mouse.

use crate::kit::{Key, Mods};
use crate::solitaire::art::{HEIGHT, WIDTH};
use crate::solitaire::board::{Board, Drag, Hit};
use crate::solitaire::game::{Game, Source};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Location {
    Stock,
    Waste,
    Foundation(usize),
    Tableau(usize),
}

use Location::{Foundation, Stock, Tableau, Waste};

const LOCATIONS: [Location; 13] = [
    Stock,
    Waste,
    Foundation(0),
    Foundation(1),
    Foundation(2),
    Foundation(3),
    Tableau(0),
    Tableau(1),
    Tableau(2),
    Tableau(3),
    Tableau(4),
    Tableau(5),
    Tableau(6),
];

impl Location {
    fn len(self, game: &Game) -> usize {
        match self {
            Stock => game.stock().len(),
            Waste => game.waste().len(),
            Foundation(i) => game.foundations()[i].len(),
            Tableau(i) => game.tableau()[i].cards.len(),
        }
    }

    fn accepts(self, carrying: bool) -> bool {
        !carrying || !matches!(self, Stock | Waste)
    }
}

#[derive(Default)]
pub(super) struct Keyboard {
    pile: usize,
    card: usize,
    carrying: bool,
}

#[derive(Default)]
pub(super) struct KeyPlay {
    pub moved: bool,
    pub pointer: Option<(i32, i32)>,
}

impl Keyboard {
    pub fn carrying(&self) -> bool {
        self.carrying
    }

    /// Mouse interaction cancels a keyboard pickup but preserves the logical cursor.
    pub fn cancel(&mut self) {
        self.carrying = false;
    }

    fn focus(&mut self, game: &Game, pile: usize, carrying: bool) {
        let location = LOCATIONS[pile];
        if location.accepts(carrying) {
            self.pile = pile;
            self.card = location.len(game).saturating_sub(1);
        }
    }

    fn advance(&mut self, game: &Game, forward: bool, by_group: bool, carrying: bool) {
        let n = LOCATIONS.len();
        let step = if forward { 1 } else { n - 1 };
        let group = std::mem::discriminant(&LOCATIONS[self.pile]);
        let mut i = self.pile;
        loop {
            i = (i + step) % n;
            if i == self.pile
                || (LOCATIONS[i].accepts(carrying) && (!by_group || std::mem::discriminant(&LOCATIONS[i]) != group))
            {
                self.focus(game, i, carrying);
                break;
            }
        }
    }

    fn vertical(&mut self, game: &Game, step: isize) {
        let location = LOCATIONS[self.pile];
        let last = location.len(game).saturating_sub(1);
        let first = match location {
            Tableau(column) => game.tableau()[column].hidden.min(last),
            Foundation(_) => 0,
            _ => last,
        };
        self.card = self.card.saturating_add_signed(step).clamp(first, last);
    }

    fn point(&self, game: &Game, board: Board, carrying: bool) -> (i32, i32) {
        let location = LOCATIONS[self.pile];
        let (rect, step) = match location {
            Stock => {
                let mut r = board.stock();
                let edge = (game.stock().len().saturating_sub(1) / 10) as i32;
                r.x += edge * 2;
                r.y += edge;
                (r, 0)
            }
            Waste => (board.source(game, Source::Waste), 1),
            Foundation(i) => (board.foundation(i, self.card.min(location.len(game).saturating_sub(1))), 1),
            Tableau(column) => {
                let index = self.card.min(location.len(game).saturating_sub(1));
                (board.source(game, Source::Tableau { column, index }), HEIGHT * 4 / 25)
            }
        };
        let offset = if carrying && location.len(game) > 0 { step } else { 0 };
        (rect.x + WIDTH / 2, rect.y + offset)
    }

    pub fn handle(
        &mut self,
        key: Key,
        mods: Mods,
        board: Board,
        game: &mut Game,
        drag: &mut Option<Drag>,
    ) -> Option<KeyPlay> {
        let held = drag.is_some();
        let mut play = KeyPlay::default();
        match key {
            Key::Tab => self.advance(game, !mods.shift, true, held),
            Key::Left | Key::Right => self.advance(game, key == Key::Right, mods.shift, held),
            Key::Home => self.focus(game, 0, held),
            Key::End => self.focus(game, LOCATIONS.len() - 1, held),
            Key::Up => self.vertical(game, -1),
            Key::Down => self.vertical(game, 1),
            Key::Escape => {
                *drag = None;
                self.cancel();
                return Some(play);
            }
            Key::Space | Key::Enter => {
                if let Some(previous) = drag.take() {
                    self.cancel();
                    play.moved =
                        board.destination(game, previous).is_some_and(|dest| game.move_cards(previous.source, dest));
                    self.focus(game, self.pile, false);
                    return Some(play);
                }
                self.vertical(game, 0);
                let (x, y) = self.point(game, board, false);
                game.start();
                match board.hit_cards(game, x, y) {
                    Some(Hit::Stock) => {
                        play.moved = game.draw_stock();
                        if !play.moved {
                            return Some(play);
                        }
                    }
                    Some(Hit::Turn(column)) => {
                        play.moved = game.turn(column);
                        return Some(play);
                    }
                    Some(Hit::Cards(source)) if !game.movable(source).is_empty() => {
                        *drag = Some(Drag::new(source, board.source(game, source), x, y));
                        self.carrying = true;
                    }
                    _ => return Some(play),
                }
                self.focus(game, self.pile, drag.is_some());
            }
            _ => return None,
        }
        let (x, y) = self.point(game, board, drag.is_some());
        play.pointer = Some((x, y));
        if let Some(drag) = drag {
            drag.update(x, y);
        }
        Some(play)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fb::Rect;

    #[test]
    fn group_navigation_skips_the_stock_and_waste_while_carrying() {
        let mut game = Game::new(1, Default::default());
        let board = Board { bounds: Rect::new(0, 0, 600, 400) };
        let mut keyboard = Keyboard::default();
        let mut drag = None;
        let mut press = |key, shift| {
            keyboard.handle(key, Mods { shift, ctrl: false }, board, &mut game, &mut drag).unwrap();
            (LOCATIONS[keyboard.pile], keyboard.carrying())
        };
        assert_eq!(press(Key::Left, false), (Tableau(6), false));
        assert_eq!(press(Key::Tab, true), (Foundation(3), false));
        assert_eq!(press(Key::Right, true), (Tableau(0), false));
        assert_eq!(press(Key::Space, false), (Tableau(0), true));
        assert_eq!(press(Key::Home, false), (Tableau(0), true));
        assert_eq!(press(Key::End, false), (Tableau(6), true));
        assert_eq!(press(Key::Right, false), (Foundation(0), true));
        assert_eq!(press(Key::Escape, false), (Foundation(0), false));
        assert_eq!(press(Key::Left, false), (Waste, false));
        assert!(drag.is_none());
        assert_eq!(game.stock().len(), 24);
    }
}
