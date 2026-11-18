//! Klondike rules and scoring shared by the three Solitaire editions.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Suit {
    Clubs,
    Diamonds,
    Hearts,
    Spades,
}

impl Suit {
    pub const ALL: [Self; 4] = [Self::Clubs, Self::Diamonds, Self::Hearts, Self::Spades];

    pub fn red(self) -> bool {
        matches!(self, Self::Diamonds | Self::Hearts)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Card {
    pub suit: Suit,
    /// Ace is 1 and king is 13.
    pub rank: u8,
}

impl Card {
    fn fits_tableau(self, upper: Self) -> bool {
        upper.rank == self.rank + 1 && self.suit.red() != upper.suit.red()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Draw {
    One,
    Three,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scoring {
    Standard,
    Vegas,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rules {
    pub draw: Draw,
    pub scoring: Scoring,
    pub timed: bool,
    pub cumulative: bool,
}

impl Default for Rules {
    fn default() -> Self {
        Self { draw: Draw::Three, scoring: Scoring::Standard, timed: true, cumulative: false }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Waste,
    Foundation(usize),
    Tableau { column: usize, index: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pile {
    Foundation(usize),
    Tableau(usize),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Column {
    /// From the head of the column down to its exposed end.
    pub cards: Vec<Card>,
    /// Number of face-down cards at the head.
    pub hidden: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Position {
    stock: Vec<Card>,
    waste: Vec<Card>,
    shown: usize,
    recycles: u32,
    tableau: [Column; 7],
    foundations: [Vec<Card>; 4],
    score: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Game {
    rules: Rules,
    position: Position,
    undo: Option<Position>,
    previous_balance: i32,
    started: bool,
    seconds: u32,
    bonus: i32,
}

fn ordered_deck() -> [Card; 52] {
    std::array::from_fn(|i| Card { suit: Suit::ALL[i % 4], rank: (i / 4 + 1) as u8 })
}

pub(super) fn random(state: &mut u32) -> u32 {
    *state = state.wrapping_mul(214013).wrapping_add(2531011);
    (*state >> 16) & 0x7fff
}

/// The same startup roll is used to select the default card back.
pub fn first_rand(seed: u64) -> u32 {
    random(&mut (seed as u16 as u32))
}

/// SOL shuffles five times, swapping each position with a card chosen from the entire deck.
fn shuffled(mut state: u32) -> [Card; 52] {
    let mut deck = ordered_deck();
    for _ in 0..5 {
        for i in 0..deck.len() {
            let j = random(&mut state) as usize % deck.len();
            deck.swap(i, j);
        }
    }
    deck.reverse();
    deck
}

impl Game {
    pub fn new(seed: u64, rules: Rules) -> Self {
        Self::with_balance(seed, rules, 0)
    }

    pub fn with_balance(seed: u64, rules: Rules, previous_balance: i32) -> Self {
        Self::dealt(shuffled(seed as u32 & 0x7fff), rules, previous_balance)
    }

    /// Takes the cards in dealing order. Cards 0 through 27 fill successive rows of the
    /// tableau, from left to right. Card 28 is the first card drawn from stock.
    pub fn from_deck(deck: [Card; 52], rules: Rules) -> Option<Self> {
        let mut seen = [false; 52];
        for card in deck {
            if !(1..=13).contains(&card.rank) {
                return None;
            }
            let index = (card.rank as usize - 1) * 4 + card.suit as usize;
            if std::mem::replace(&mut seen[index], true) {
                return None;
            }
        }
        Some(Self::dealt(deck, rules, 0))
    }

    fn dealt(deck: [Card; 52], rules: Rules, previous_balance: i32) -> Self {
        let mut cards = deck.into_iter();
        let mut tableau: [Column; 7] = Default::default();
        for row in 0..tableau.len() {
            for (index, column) in tableau.iter_mut().enumerate().skip(row) {
                column.cards.push(cards.next().unwrap());
                if index > row {
                    column.hidden += 1;
                }
            }
        }
        let score = match rules.scoring {
            Scoring::Vegas if rules.cumulative => previous_balance.saturating_sub(52),
            Scoring::Vegas => -52,
            _ => 0,
        };
        Self {
            rules,
            position: Position {
                stock: cards.rev().collect(),
                waste: Vec::new(),
                shown: 0,
                recycles: 0,
                tableau,
                foundations: Default::default(),
                score,
            },
            undo: None,
            previous_balance,
            started: false,
            seconds: 0,
            bonus: 0,
        }
    }

    pub fn deal(&mut self, seed: u64, rules: Rules) {
        let balance = if rules == self.rules { self.vegas_balance() } else { 0 };
        *self = Self::with_balance(seed, rules, balance);
    }

    pub fn rules(&self) -> Rules {
        self.rules
    }

    /// This option takes effect without the fresh deal required by Draw, Scoring and Timed.
    pub fn set_cumulative(&mut self, cumulative: bool) {
        self.rules.cumulative = cumulative;
    }

    /// Stock, waste and foundation piles have their playable card at the end.
    pub fn stock(&self) -> &[Card] {
        &self.position.stock
    }

    pub fn waste(&self) -> &[Card] {
        &self.position.waste
    }

    pub fn visible_waste(&self) -> &[Card] {
        &self.waste()[self.waste().len() - self.position.shown..]
    }

    pub fn foundations(&self) -> &[Vec<Card>; 4] {
        &self.position.foundations
    }

    pub fn tableau(&self) -> &[Column; 7] {
        &self.position.tableau
    }

    pub fn score(&self) -> i32 {
        self.position.score
    }

    pub fn bonus(&self) -> i32 {
        self.bonus
    }

    pub fn vegas_balance(&self) -> i32 {
        if self.rules.scoring == Scoring::Vegas && self.rules.cumulative { self.score() } else { self.previous_balance }
    }

    pub fn started(&self) -> bool {
        self.started
    }

    pub fn start(&mut self) {
        self.started = true;
    }

    pub fn seconds(&self) -> u32 {
        self.seconds
    }

    /// The original clock stores quarter-second ticks in a signed 16-bit counter.
    pub fn advance_time(&mut self, delta_seconds: u32) -> bool {
        if !self.started || !self.rules.timed || self.won() {
            return false;
        }
        let seconds = self.seconds.saturating_add(delta_seconds).min(8191);
        if seconds == self.seconds {
            return false;
        }
        let intervals = seconds / 10 - self.seconds / 10;
        self.adjust_score(-2 * intervals as i32, 0);
        self.seconds = seconds;
        true
    }

    fn adjust_score(&mut self, standard: i32, vegas: i32) {
        let score = &mut self.position.score;
        match self.rules.scoring {
            Scoring::Standard => *score = score.saturating_add(standard).max(0),
            Scoring::Vegas => *score = score.saturating_add(vegas),
            Scoring::None => {}
        }
    }

    pub fn won(&self) -> bool {
        self.foundations().iter().all(|pile| pile.len() == 13)
    }

    pub fn can_undo(&self) -> bool {
        self.undo.is_some() && !self.won()
    }

    pub fn undo(&mut self) -> bool {
        if !self.can_undo() {
            return false;
        }
        self.position = self.undo.take().unwrap();
        self.adjust_score(-2, 0);
        true
    }

    pub fn can_redeal(&self) -> bool {
        self.stock().is_empty() && !self.waste().is_empty() && self.recycles_allowed()
    }

    /// Determines the circle/cross on an empty stock, even when the waste is also empty.
    pub fn recycles_allowed(&self) -> bool {
        self.rules.scoring != Scoring::Vegas || (self.rules.draw == Draw::Three && self.position.recycles < 2)
    }

    pub fn draw_stock(&mut self) -> bool {
        self.draw_cards(self.rules.draw)
    }

    /// Ctrl+Alt+Shift on the stock draws a single card, even in a draw-three game.
    pub fn draw_one(&mut self) -> bool {
        self.draw_cards(Draw::One)
    }

    fn draw_cards(&mut self, draw: Draw) -> bool {
        self.start();
        if self.stock().is_empty() {
            if !self.can_redeal() {
                return false;
            }
            self.undo = Some(self.position.clone());
            let position = &mut self.position;
            position.stock.extend(position.waste.drain(..).rev());
            position.shown = 0;
            position.recycles = position.recycles.saturating_add(1);
            let penalty = match self.rules.draw {
                Draw::One => -100,
                Draw::Three if position.recycles > 3 => -20,
                _ => 0,
            };
            self.adjust_score(penalty, 0);
        } else {
            self.undo = Some(self.position.clone());
            let count = match draw {
                Draw::One => 1,
                Draw::Three => 3,
            };
            let position = &mut self.position;
            position.shown = position.stock.len().min(count);
            for _ in 0..position.shown {
                position.waste.push(position.stock.pop().unwrap());
            }
        }
        true
    }

    pub fn movable(&self, source: Source) -> &[Card] {
        if self.won() {
            return &[];
        }
        match source {
            Source::Waste => self.waste().last().map_or(&[], std::slice::from_ref),
            Source::Foundation(index) => {
                self.foundations().get(index).and_then(|cards| cards.last()).map_or(&[], std::slice::from_ref)
            }
            Source::Tableau { column, index } => {
                let Some(col) = self.tableau().get(column) else { return &[] };
                let Some(cards) = col.cards.get(index..) else { return &[] };
                if index < col.hidden || !cards.windows(2).all(|pair| pair[1].fits_tableau(pair[0])) {
                    return &[];
                }
                cards
            }
        }
    }

    pub fn can_move(&self, source: Source, destination: Pile) -> bool {
        let cards = self.movable(source);
        let Some(&card) = cards.first() else { return false };
        match destination {
            Pile::Foundation(index) => {
                if cards.len() != 1 || source == Source::Foundation(index) {
                    return false;
                }
                self.foundations().get(index).is_some_and(|pile| match pile.last() {
                    Some(top) => top.suit == card.suit && top.rank + 1 == card.rank,
                    None => card.rank == 1,
                })
            }
            Pile::Tableau(index) => {
                if matches!(source, Source::Tableau { column, .. } if column == index) {
                    return false;
                }
                self.tableau().get(index).is_some_and(|col| match col.cards.last() {
                    Some(&top) => col.hidden < col.cards.len() && card.fits_tableau(top),
                    None => card.rank == 13,
                })
            }
        }
    }

    pub fn move_cards(&mut self, source: Source, destination: Pile) -> bool {
        self.start();
        self.transfer(source, destination)
    }

    fn transfer(&mut self, source: Source, destination: Pile) -> bool {
        if !self.can_move(source, destination) {
            return false;
        }
        self.undo = Some(self.position.clone());
        let position = &mut self.position;
        let mut cards = match source {
            Source::Tableau { column, index } => position.tableau[column].cards.split_off(index),
            Source::Foundation(index) => vec![position.foundations[index].pop().unwrap()],
            Source::Waste => {
                let card = position.waste.pop().unwrap();
                position.shown = (position.shown - 1).max(usize::from(!position.waste.is_empty()));
                vec![card]
            }
        };
        match destination {
            Pile::Foundation(index) => {
                position.foundations[index].append(&mut cards);
                if !matches!(source, Source::Foundation(_)) {
                    self.adjust_score(10, 5);
                }
                if self.won() && self.rules.scoring == Scoring::Standard && self.rules.timed && self.seconds >= 30 {
                    self.bonus = 35 * (20_000 / self.seconds as i32);
                    self.adjust_score(self.bonus, 0);
                }
            }
            Pile::Tableau(index) => {
                position.tableau[index].cards.append(&mut cards);
                match source {
                    Source::Waste => self.adjust_score(5, 0),
                    Source::Foundation(_) => self.adjust_score(-15, -5),
                    _ => {}
                }
            }
        }
        true
    }

    /// Click an exposed face-down tableau card. Turning it starts a new undo history.
    pub fn turn(&mut self, column: usize) -> bool {
        self.start();
        let Some(col) = self.position.tableau.get_mut(column) else { return false };
        if col.hidden == 0 || col.hidden != col.cards.len() {
            return false;
        }
        col.hidden -= 1;
        self.undo = None;
        self.adjust_score(5, 0);
        true
    }

    pub fn auto_foundation(&mut self, source: Source) -> bool {
        self.start();
        !matches!(source, Source::Foundation(_)) && self.send_to_foundation(source)
    }

    fn send_to_foundation(&mut self, source: Source) -> bool {
        (0..4).any(|index| self.transfer(source, Pile::Foundation(index)))
    }

    /// Windows 2000's right-click/Ctrl+A command. Each card remains a separate move for Undo.
    pub fn auto_all(&mut self) -> bool {
        let mut moved = false;
        loop {
            let mut changed = self.send_to_foundation(Source::Waste);
            for column in 0..self.tableau().len() {
                if let Some(index) = self.tableau()[column].cards.len().checked_sub(1) {
                    changed |= self.send_to_foundation(Source::Tableau { column, index });
                }
            }
            if !changed {
                return moved;
            }
            moved = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Suit::*;

    fn card(suit: Suit, rank: u8) -> Card {
        Card { suit, rank }
    }

    fn source(column: usize, index: usize) -> Source {
        Source::Tableau { column, index }
    }

    fn blank() -> Game {
        let mut game = Game::new(1, Rules::default());
        game.position.tableau = Default::default();
        game.position.stock.clear();
        game
    }

    #[test]
    fn deal_faces_and_stock_follow_the_supplied_order() {
        let deck = ordered_deck();
        let game = Game::from_deck(deck, Rules::default()).unwrap();
        let exposed = [0, 7, 13, 18, 22, 25, 27];
        for (i, col) in game.tableau().iter().enumerate() {
            assert_eq!(col.hidden, i);
            assert_eq!(col.cards.len(), i + 1);
            assert_eq!(col.cards[i], deck[exposed[i]]);
        }
        assert_eq!(game.stock().iter().rev().copied().collect::<Vec<_>>(), deck[28..]);
        let mut malformed = deck;
        malformed[51] = malformed[0];
        assert!(Game::from_deck(malformed, Rules::default()).is_none());
        malformed[51].rank = 0;
        assert!(Game::from_deck(malformed, Rules::default()).is_none());
    }

    #[test]
    fn seeded_game_and_startup_back_use_separate_random_rolls() {
        assert_eq!(first_rand(7), 61);
        let game = Game::new(7, Rules::default());
        assert_eq!(game.tableau()[5].cards.last(), Some(&card(Hearts, 8)));
        assert_eq!(game.tableau()[3].cards.last(), Some(&card(Spades, 9)));
        assert_eq!(game.stock().last(), Some(&card(Hearts, 4)));
        assert!(game.can_move(source(5, 5), Pile::Tableau(3)));
        assert!(!game.can_move(source(5, 5), Pile::Tableau(1)));
    }

    #[test]
    fn stacks_move_by_rank_and_color_and_only_kings_fill_spaces() {
        let mut game = blank();
        game.position.tableau[0] =
            Column { cards: vec![card(Hearts, 13), card(Clubs, 12), card(Diamonds, 11)], hidden: 0 };
        game.position.tableau[1].cards.push(card(Spades, 13));
        game.position.tableau[2].cards.push(card(Diamonds, 13));
        assert!(!game.can_move(source(0, 1), Pile::Tableau(1)));
        assert!(!game.can_move(source(0, 1), Pile::Tableau(4)));
        assert!(!game.can_move(source(0, 0), Pile::Tableau(0)));
        assert!(game.can_move(source(0, 0), Pile::Tableau(4)));
        assert!(game.move_cards(source(0, 1), Pile::Tableau(2)));
        assert_eq!(game.tableau()[2].cards, [card(Diamonds, 13), card(Clubs, 12), card(Diamonds, 11)]);
        assert!(game.undo());
        assert_eq!(game.tableau()[0].cards.len(), 3);
        assert!(!game.undo());
        assert!(game.movable(source(usize::MAX, 0)).is_empty());
        assert!(!game.can_move(source(0, usize::MAX), Pile::Foundation(4)));
    }

    #[test]
    fn manual_turn_is_scored_and_discards_the_last_undo() {
        let mut game = blank();
        game.position.tableau[1] = Column { cards: vec![card(Clubs, 8), card(Hearts, 1)], hidden: 1 };
        assert!(game.auto_foundation(source(1, 1)));
        assert_eq!(game.score(), 10);
        assert!(game.can_undo());
        assert!(game.movable(source(1, 0)).is_empty());
        assert!(game.turn(1));
        assert_eq!(game.score(), 15);
        assert_eq!(game.movable(source(1, 0)), [card(Clubs, 8)]);
        assert!(!game.can_undo());
        assert!(!game.turn(1));
    }

    #[test]
    fn waste_fan_collapses_after_the_last_card_of_a_draw() {
        let mut game = blank();
        game.position.stock = vec![
            card(Spades, 1),
            card(Hearts, 1),
            card(Clubs, 1),
            card(Hearts, 8),
            card(Spades, 9),
            card(Diamonds, 10),
        ];
        assert!(game.draw_stock());
        let old_packet = game.visible_waste().to_vec();
        assert!(game.draw_stock());
        assert_eq!(game.visible_waste(), [card(Clubs, 1), card(Hearts, 1), card(Spades, 1)]);
        assert!(game.undo());
        assert_eq!(game.visible_waste(), old_packet);
        assert!(game.draw_stock());
        for remaining in [2, 1, 1] {
            assert!(game.auto_foundation(Source::Waste));
            assert_eq!(game.visible_waste().len(), remaining);
        }
        assert_eq!(game.visible_waste(), [card(Hearts, 8)]);
        assert!(game.draw_stock());
        assert!(game.waste().is_empty());
        assert!(game.draw_stock());
        assert_eq!(game.visible_waste(), old_packet);
    }

    #[test]
    fn stock_passes_apply_the_penalty_on_the_original_recycle() {
        for (draw, scores) in [(Draw::One, [200, 100, 0, 0, 0]), (Draw::Three, [300, 300, 300, 280, 260])] {
            let mut game = Game::new(1, Rules { draw, ..Default::default() });
            game.position.score = 300;
            let stock = game.stock().to_vec();
            for score in scores {
                while !game.stock().is_empty() {
                    assert!(game.draw_stock());
                }
                assert!(game.draw_stock());
                assert_eq!(game.stock(), stock);
                assert_eq!(game.score(), score);
            }
        }
    }

    #[test]
    fn vegas_costs_and_pass_limits_apply_without_spending_any_cards() {
        for (draw, allowed_recycles) in [(Draw::One, 0), (Draw::Three, 2)] {
            let rules = Rules { draw, scoring: Scoring::Vegas, cumulative: true, ..Default::default() };
            let mut game = Game::with_balance(1, rules, 100);
            assert_eq!(game.score(), 48);
            game.deal(2, rules);
            assert_eq!(game.score(), -4);
            for pass in 0..=allowed_recycles {
                while !game.stock().is_empty() {
                    assert!(game.draw_stock());
                }
                assert_eq!(game.can_redeal(), pass < allowed_recycles);
                assert_eq!(game.draw_stock(), pass < allowed_recycles);
            }
            game.deal(3, Rules { timed: false, ..rules });
            assert_eq!(game.score(), -52);
        }
    }

    #[test]
    fn foundation_waste_and_tableau_moves_charge_the_correct_amounts() {
        for (scoring, expected) in
            [(Scoring::Standard, [35, 45, 30, 43]), (Scoring::Vegas, [30, 35, 30, 35]), (Scoring::None, [0, 0, 0, 0])]
        {
            let mut game = blank();
            game.rules.scoring = scoring;
            game.position.score = if scoring == Scoring::None { 0 } else { 30 };
            game.position.stock = vec![card(Clubs, 1)];
            game.position.tableau[0].cards.push(card(Diamonds, 2));
            assert!(game.draw_one());
            assert!(game.move_cards(Source::Waste, Pile::Tableau(0)));
            assert_eq!(game.score(), expected[0]);
            assert!(game.auto_foundation(source(0, 1)));
            assert_eq!(game.score(), expected[1]);
            assert!(game.move_cards(Source::Foundation(0), Pile::Tableau(0)));
            assert_eq!(game.score(), expected[2]);
            assert!(game.undo());
            assert_eq!(game.score(), expected[3]);
        }
    }

    #[test]
    fn undo_restores_score_before_move_and_keeps_elapsed_time() {
        let mut game = Game::new(2, Rules::default());
        game.position.score = 25;
        assert!(!game.advance_time(10));
        assert!(game.draw_stock());
        assert!(game.advance_time(25));
        assert_eq!(game.score(), 21);
        assert!(game.undo());
        assert_eq!(game.score(), 23);
        assert_eq!(game.seconds(), 25);
        assert!(game.advance_time(u32::MAX));
        assert_eq!(game.seconds(), 8191);
        assert_eq!(game.score(), 0);
        assert!(!game.advance_time(1));
    }

    #[test]
    fn auto_all_unlocks_earlier_columns_and_leaves_the_clock_as_it_was() {
        let mut game = blank();
        game.position.tableau[0].cards.push(card(Spades, 2));
        game.position.tableau[6].cards.push(card(Spades, 1));
        assert!(game.auto_all());
        assert_eq!(game.foundations()[0], [card(Spades, 1), card(Spades, 2)]);
        assert!(!game.started());
        assert!(game.undo());
        assert_eq!(game.foundations()[0], [card(Spades, 1)]);
        assert_eq!(game.tableau()[0].cards, [card(Spades, 2)]);
    }

    #[test]
    fn aces_can_exchange_foundation_slots_without_scoring() {
        let mut game = blank();
        game.position.tableau[0].cards.push(card(Spades, 1));
        assert!(game.auto_foundation(source(0, 0)));
        assert!(!game.auto_foundation(Source::Foundation(0)));
        assert!(game.move_cards(Source::Foundation(0), Pile::Foundation(2)));
        assert_eq!(game.score(), 10);
        assert!(game.foundations()[0].is_empty());
        assert_eq!(game.foundations()[2], [card(Spades, 1)]);
    }

    #[test]
    fn final_card_wins_and_timed_bonus_uses_whole_second_division() {
        for (seconds, bonus) in [(29, 0), (30, 23310), (39, 17920)] {
            let mut game = blank();
            for (i, suit) in Suit::ALL.into_iter().enumerate() {
                game.position.foundations[i].extend((1..=13).map(|rank| card(suit, rank)));
            }
            game.position.tableau[4].cards.push(game.position.foundations[2].pop().unwrap());
            game.start();
            game.advance_time(seconds);
            assert!(game.auto_foundation(source(4, 0)));
            assert!(game.won());
            assert_eq!(game.bonus(), bonus);
            assert_eq!(game.score(), 10 + bonus);
            assert!(!game.advance_time(1));
            assert!(!game.undo());
        }
    }
}
