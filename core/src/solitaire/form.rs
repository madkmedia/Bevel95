//! Solitaire's main form. The same hit testing drives play in the Windows host and in
//! the reference scenes, including the original grip point when dragging a packet.

use crate::fb::{Framebuffer, Icon, Rect};
use crate::kit::{self, Button, Effect, Form, Key, Mods, Press};
use crate::paint::Menu;
use crate::text::Fonts;
use crate::theme::{Era, Theme};
use crate::window::TitleButton;

use super::art;
use super::board::{Board, Drag, Exposures, Hit, View};
use super::chrome::{self, Chrome, Score, Status};
use super::game::{Card, Game, Rules, Scoring, Suit};
pub use super::menu::{Command, MENUS, command_at};
use super::menu::{MenuAction, MenuBar};
use super::victory::Victory;
#[path = "keyboard.rs"]
mod keyboard;
use keyboard::Keyboard;

pub const MIN_SIZE: (i32, i32) = (112, 27);
pub const fn default_size(era: Era) -> (i32, i32) {
    match era {
        Era::Win95 | Era::Win98 => (597, 434),
        Era::Win2000 => (593, 430),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Icon,
    Minimize,
    Maximize,
    Close,
    Menu(usize),
    Board,
    TitleMenu,
    KeyMenu,
    Ask(Command),
    Won,
    MovePointer(i32, i32),
}

pub struct SolitaireWindow {
    pub era: Era,
    pub game: Game,
    pub back: usize,
    pub status: bool,
    pub outline: bool,
    pub menu_status: Option<&'static str>,
    menus: MenuBar,
    size: (i32, i32),
    maximized: bool,
    active: bool,
    pressed: Option<Part>,
    drag: Option<Drag>,
    keyboard: Keyboard,
    back_ticks: u32,
    displayed_back_ticks: u32,
    stock_edge: bool,
    exposures: Exposures,
    icon: Icon,
    victory: Option<Victory>,
    victory_done: bool,
    keyboard_stop: bool,
    animation_seed: u64,
}

impl SolitaireWindow {
    pub fn new(era: Era, game: Game, back: usize, status: bool, outline: bool) -> Self {
        let seed = game
            .stock()
            .iter()
            .fold(0u64, |sum, card| sum.wrapping_mul(53).wrapping_add(u64::from(card.rank) * 4 + card.suit as u64));
        let stock_edge = game.stock().len() == 24 && !game.started();
        Self {
            era,
            game,
            back,
            status,
            outline,
            menu_status: None,
            menus: MenuBar::default(),
            size: default_size(era),
            maximized: false,
            active: true,
            pressed: None,
            drag: None,
            keyboard: Keyboard::default(),
            back_ticks: 0,
            displayed_back_ticks: 0,
            stock_edge,
            exposures: Exposures::default(),
            icon: art::icon(era, 16),
            victory: None,
            victory_done: false,
            keyboard_stop: false,
            animation_seed: seed,
        }
    }

    fn board(&self) -> Board {
        Board { bounds: chrome::board_rect(self.era, self.size, self.status, self.maximized) }
    }

    /// Repainting the whole client erases any border pixel left by a partial pile redraw.
    pub fn repaint_board(&mut self) {
        self.stock_edge = false;
        self.exposures = Exposures::default();
    }

    fn stock_changed(&mut self, before: usize) {
        let after = self.game.stock().len();
        if after == 0 || after > before {
            self.stock_edge = false;
        } else if (after - 1) / 10 < (before - 1) / 10 {
            self.stock_edge = true;
        }
    }

    pub fn set_size(&mut self, size: (i32, i32), maximized: bool) {
        if self.size != size || self.maximized != maximized {
            self.repaint_board();
            self.drag = None;
            self.keyboard.cancel();
            self.stop_win();
            self.size = size;
            self.maximized = maximized;
        }
    }

    pub fn set_era(&mut self, era: Era) {
        if self.era != era {
            if !self.maximized && self.size == default_size(self.era) {
                self.size = default_size(era);
            }
            self.era = era;
            self.repaint_board();
            self.icon = art::icon(era, 16);
            self.drag = None;
            self.keyboard.cancel();
            self.menus.leave();
            self.menu_status = None;
            self.stop_win();
        }
    }

    pub fn new_deal(&mut self, seed: u64, rules: Rules) {
        self.game.deal(seed, rules);
        self.exposures = Exposures::default();
        self.stock_edge = true;
        self.animation_seed = seed;
        self.victory = None;
        self.victory_done = false;
        self.drag = None;
        self.keyboard = Keyboard::default();
        self.back_ticks = 0;
        self.displayed_back_ticks = 0;
    }

    pub fn set_back(&mut self, index: usize) {
        self.repaint_board();
        self.back = index;
        self.back_ticks = 0;
        self.displayed_back_ticks = 0;
    }

    pub fn set_display(&mut self, status: bool, outline: bool) {
        self.status = status;
        self.outline = outline;
    }

    pub fn tick(&mut self, seconds: u32) -> bool {
        !self.victory_done && self.victory.is_none() && self.game.advance_time(seconds)
    }

    pub fn tick_back(&mut self) -> bool {
        if !matches!(self.back, 2 | 9 | 10 | 11)
            || self.game.stock().is_empty()
            || self.victory_done
            || self.victory.is_some()
        {
            return false;
        }
        self.back_ticks = self.back_ticks.wrapping_add(1);
        if self.drag.is_some() {
            return false;
        }
        self.displayed_back_ticks = self.back_ticks;
        true
    }

    pub fn menu(&self, index: usize) -> Menu {
        self.menus.menu(self.era, index, self.game.can_undo() && !self.victory_done)
    }

    pub fn title_rect(&self, index: usize) -> Option<Rect> {
        self.menus.title_rect(index)
    }
    pub fn show_menu(&mut self, menu: Option<usize>) {
        if menu.is_none() {
            self.repaint_board();
        }
        self.menus.show_menu(menu);
    }
    pub fn back_to_bar(&mut self, index: usize) {
        self.repaint_board();
        self.menus.back_to_bar(index);
    }
    pub fn reached_by_keyboard(&mut self, keyboard: bool) {
        self.menus.reached_by_keyboard(keyboard);
    }

    pub fn start_win(&mut self) {
        let board = self.board();
        let origins = std::array::from_fn(|i| {
            let r = board.foundation_origin(i);
            (r.x, r.y)
        });
        let kings = std::array::from_fn(|i| Card {
            suit: self.game.foundations()[i].last().map_or(Suit::ALL[i], |c| c.suit),
            rank: 13,
        });
        self.drag = None;
        self.keyboard.cancel();
        self.victory_done = false;
        self.keyboard_stop = false;
        self.victory = Some(Victory::new(self.era, board.bounds, origins, kings, self.animation_seed));
    }

    fn stop_win(&mut self) -> bool {
        if self.victory.take().is_some() {
            self.victory_done = true;
            true
        } else {
            false
        }
    }

    pub fn animate_win(&mut self) -> bool {
        let Some(victory) = &mut self.victory else { return false };
        for _ in 0..4 {
            if !victory.advance() {
                self.stop_win();
                return false;
            }
        }
        true
    }

    pub fn win_finished(&self) -> bool {
        self.victory_done
    }
    pub fn win_stopped_by_keyboard(&self) -> bool {
        self.keyboard_stop
    }

    fn won_effect(&self, moved: bool) -> Vec<Effect<Part>> {
        if moved && self.game.won() { vec![Effect::Activate(Part::Won)] } else { Vec::new() }
    }
}

fn button_for(part: Part) -> Option<TitleButton> {
    match part {
        Part::Minimize => Some(TitleButton::Minimize),
        Part::Maximize => Some(TitleButton::Maximize),
        Part::Close => Some(TitleButton::Close),
        _ => None,
    }
}

fn menu_effect(action: Option<MenuAction>) -> Vec<Effect<Part>> {
    match action {
        Some(MenuAction::Open(index)) => vec![Effect::Activate(Part::Menu(index))],
        Some(MenuAction::Run(command)) => vec![Effect::Activate(Part::Ask(command))],
        Some(MenuAction::SystemMenu) => vec![Effect::Activate(Part::KeyMenu)],
        _ => Vec::new(),
    }
}

impl Form for SolitaireWindow {
    type Part = Part;

    fn title(&self) -> &str {
        "Solitaire"
    }
    fn size(&self) -> (i32, i32) {
        self.size
    }

    fn layout(&mut self, fonts: &Fonts) {
        let (dx, dy) = chrome::menu_offset(self.maximized);
        self.menus.layout(fonts, dx, dy);
    }

    fn paint(&self, fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts) {
        let score = match self.game.rules().scoring {
            Scoring::None => None,
            Scoring::Standard => Some(Score::Standard(self.game.score())),
            Scoring::Vegas => Some(Score::Vegas(self.game.score())),
        };
        let status = Status {
            score,
            seconds: self.game.rules().timed.then_some(self.game.seconds()),
            message: if self.victory.is_some() {
                Some(if self.era == Era::Win2000 {
                    "Press Esc or a mouse button to stop..."
                } else {
                    "Click or press Esc to stop..."
                })
            } else {
                self.menu_status
            },
            bonus: self.victory.is_some().then_some(self.game.bonus()),
        };
        let bounds = chrome::draw_chrome(
            fb,
            theme,
            fonts,
            &Chrome {
                era: self.era,
                icon: &self.icon,
                active: self.active,
                maximized: self.maximized,
                pressed: self.pressed.and_then(button_for),
                titles: self.menus.looks(),
                show_keys: self.menus.show_keys(self.era),
                status: self.status.then_some(&status),
            },
        );
        if self.victory_done {
            fb.fill(bounds, 0xff008000);
            return;
        }
        Board { bounds }.paint(
            fb,
            &self.game,
            &View {
                era: self.era,
                back: self.back,
                ticks: self.displayed_back_ticks,
                drag: self.drag,
                outline: self.outline,
                stock_edge: self.stock_edge,
            },
            &self.exposures,
        );
        if let Some(victory) = &self.victory {
            victory.paint(fb);
        }
    }

    fn hit(&self, x: i32, y: i32) -> Option<Part> {
        if let Some(i) = self.menus.hit(x, y) {
            return Some(Part::Menu(i));
        }
        [Part::Icon, Part::Minimize, Part::Maximize, Part::Close, Part::Board]
            .into_iter()
            .find(|&part| self.rect(part).is_some_and(|r| r.contains(x, y)))
    }

    fn rect(&self, part: Part) -> Option<Rect> {
        if let Some(button) = button_for(part) {
            return Some(chrome::title_button_rect(self.size.0, self.maximized, button));
        }
        match part {
            Part::Icon => Some(chrome::icon_rect(self.maximized)),
            Part::Menu(i) => self.title_rect(i),
            Part::Board => Some(self.board().bounds),
            _ => None,
        }
    }

    fn enabled(&self, _part: Part) -> bool {
        true
    }
    fn focus(&self) -> Part {
        Part::Board
    }
    fn set_focus(&mut self, _part: Part) {}
    fn tab_stops(&self) -> Vec<Part> {
        Vec::new()
    }
    fn set_pressed(&mut self, part: Option<Part>) {
        self.pressed = part;
    }
    fn show_keys(&mut self) {}
    fn enter(&self) -> Part {
        Part::Board
    }
    fn escape(&self) -> Part {
        Part::Board
    }

    fn key(&mut self, key: Key, mods: Mods) -> Option<Vec<Effect<Part>>> {
        if key == Key::Escape && self.stop_win() {
            self.keyboard_stop = true;
            return Some(Vec::new());
        }
        if let Some(action) = self.menus.key(self.era, key, mods) {
            return Some(menu_effect(Some(action)));
        }
        if self.era == Era::Win2000
            && mods.ctrl
            && key == Key::Other(u16::from(b'A'))
            && self.victory.is_none()
            && !self.victory_done
        {
            let previous = self.game.tableau().clone();
            let moved = self.game.auto_all();
            self.exposures.record(&previous, &self.game, self.era, true);
            return Some(self.won_effect(moved));
        }
        if self.victory.is_none() && !self.victory_done {
            let board = self.board();
            let before = self.game.stock().len();
            let in_place = self.outline || self.drag.is_none();
            let previous = matches!(key, Key::Space | Key::Enter).then(|| self.game.tableau().clone());
            if let Some(play) = self.keyboard.handle(key, mods, board, &mut self.game, &mut self.drag) {
                self.stock_changed(before);
                if let Some(previous) = previous {
                    self.exposures.record(&previous, &self.game, self.era, in_place);
                }
                let mut effects = Vec::new();
                if let Some((x, y)) = play.pointer {
                    effects.push(Effect::Activate(Part::MovePointer(x, y)));
                }
                effects.extend(self.won_effect(play.moved));
                return Some(effects);
            }
        }
        Some(Vec::new())
    }

    fn alt_key(&mut self, key: Key) -> bool {
        self.menus.alt_key(key)
    }
    fn alt_key_with_mods(&mut self, key: Key, mods: Mods) -> Option<Vec<Effect<Part>>> {
        if self.era == Era::Win2000 && mods.shift && key == Key::Other(u16::from(b'2')) && self.victory.is_none() {
            return Some(vec![Effect::Activate(Part::Won)]);
        }
        self.alt_key(key).then(Vec::new)
    }
    fn access(&mut self, ch: char) -> Vec<Effect<Part>> {
        menu_effect(self.menus.access(ch))
    }
    fn typed(&mut self, ch: char) -> Vec<Effect<Part>> {
        menu_effect(self.menus.typed(ch))
    }
    fn space(&mut self) -> Vec<Effect<Part>> {
        Vec::new()
    }
    fn menu_key(&mut self) -> Vec<Effect<Part>> {
        self.menus.menu_key();
        Vec::new()
    }

    fn press(&mut self, part: Option<Part>, x: i32, y: i32) -> Press<Part> {
        if self.keyboard.carrying() && part == Some(Part::Board) {
            self.keyboard.cancel();
            if let Some(drag) = &mut self.drag {
                drag.update(x, y);
            }
            return Press::Grab(Part::Board, 0);
        }
        self.keyboard.cancel();
        self.drag = None;
        if self.stop_win() {
            return Press::Done;
        }
        self.menus.leave();
        match part {
            Some(Part::Board) if !self.victory_done => {
                self.game.start();
                let board = self.board();
                match board.hit(&self.game, x, y) {
                    Some(Hit::Stock) => {
                        let before = self.game.stock().len();
                        self.game.draw_stock();
                        self.stock_changed(before);
                    }
                    Some(Hit::Turn(column)) => {
                        self.game.turn(column);
                    }
                    Some(Hit::Cards(source)) if !self.game.movable(source).is_empty() => {
                        self.drag = Some(Drag::new(source, board.source(&self.game, source), x, y));
                        return Press::Grab(Part::Board, 0);
                    }
                    _ => {}
                }
                Press::Done
            }
            Some(p @ (Part::Minimize | Part::Maximize | Part::Close)) => Press::Hold(p),
            Some(p @ (Part::Icon | Part::Menu(_))) => Press::Act(p),
            None => Press::Nothing,
            _ => Press::Done,
        }
    }

    fn double_click(&mut self, x: i32, y: i32) -> Option<Vec<Effect<Part>>> {
        if self.stop_win() {
            return Some(Vec::new());
        }
        if self.hit(x, y) == Some(Part::Icon) {
            return Some(vec![Effect::Close]);
        }
        if self.hit(x, y).is_none() && kit::TITLE_ROWS.contains(&y) {
            return Some(vec![Effect::Activate(Part::Maximize)]);
        }
        if self.victory_done {
            return None;
        }
        match self.board().hit(&self.game, x, y) {
            Some(Hit::Cards(source)) => {
                self.drag = None;
                let previous = self.game.tableau().clone();
                let moved = self.game.auto_foundation(source);
                self.exposures.record(&previous, &self.game, self.era, true);
                Some(self.won_effect(moved))
            }
            _ => None,
        }
    }

    fn drag(&mut self, _part: Part, _grab_y: i32, _value: i32, x: i32, y: i32) -> bool {
        self.drag.as_mut().is_some_and(|drag| drag.update(x, y))
    }

    fn dropped(&mut self, _part: Part, x: i32, y: i32) -> Vec<Effect<Part>> {
        let Some(mut drag) = self.drag.take() else { return Vec::new() };
        drag.update(x, y);
        let previous = self.game.tableau().clone();
        let moved =
            self.board().destination(&self.game, drag).is_some_and(|dest| self.game.move_cards(drag.source, dest));
        self.exposures.record(&previous, &self.game, self.era, self.outline);
        self.won_effect(moved)
    }

    fn button(&mut self, button: Button, down: bool, x: i32, y: i32) -> Vec<Effect<Part>> {
        if down && self.stop_win() {
            return Vec::new();
        }
        if button != Button::Right {
            return Vec::new();
        }
        if !down && kit::TITLE_ROWS.contains(&y) && matches!(self.hit(x, y), None | Some(Part::Icon)) {
            return vec![Effect::Activate(Part::TitleMenu)];
        }
        if down
            && self.era == Era::Win2000
            && self.board().bounds.contains(x, y)
            && !self.victory_done
            && self.drag.is_none()
        {
            let previous = self.game.tableau().clone();
            let moved = self.game.auto_all();
            self.exposures.record(&previous, &self.game, self.era, true);
            return self.won_effect(moved);
        }
        Vec::new()
    }

    fn set_active(&mut self, active: bool) {
        self.active = active;
        if !active {
            self.drag = None;
            self.keyboard.cancel();
            self.menus.leave();
            self.menus.pointer_left(self.era);
            self.menu_status = None;
        }
    }
    fn hover(&mut self, x: i32, y: i32) -> bool {
        if self.keyboard.carrying() {
            return self.drag.as_mut().is_some_and(|drag| drag.update(x, y));
        }
        self.drag.take().is_some() | self.menus.hover(x, y, self.era)
    }
    fn pointer_left(&mut self) -> bool {
        self.menus.pointer_left(self.era)
    }
}

#[cfg(test)]
mod tests {
    use super::super::game::{Source, Suit};
    use super::*;
    use crate::kit::Controller;

    fn ace_deal() -> SolitaireWindow {
        let deck = std::array::from_fn(|i| Card { suit: Suit::ALL[i / 13], rank: (i % 13 + 1) as u8 });
        SolitaireWindow::new(Era::Win98, Game::from_deck(deck, Rules::default()).unwrap(), 0, true, false)
    }

    #[test]
    fn drag_preserves_grip_and_snaps_the_ace_to_its_foundation() {
        let mut form = ace_deal();
        let board = form.board();
        let mut input = Controller::default();
        let from = board.source(&form.game, Source::Tableau { column: 0, index: 0 });
        let dest = board.foundation_origin(1);
        assert_eq!(from, Rect::new(17, 151, 71, 96));
        assert_eq!(input.mouse_down(&mut form, from.x + 67, from.y + 80), [Effect::Capture]);
        input.mouse_move(&mut form, dest.x + 67, dest.y + 80);
        input.mouse_up(&mut form, dest.x + 67, dest.y + 80);
        assert_eq!(form.game.foundations()[1], [Card { suit: Suit::Clubs, rank: 1 }]);
        assert!(form.game.tableau()[0].cards.is_empty());
        assert_eq!(form.game.score(), 10);
        assert!(form.game.undo());
        assert!(form.double_click(from.x + 4, from.y + 4).is_some());
        assert_eq!(form.game.foundations()[0], [Card { suit: Suit::Clubs, rank: 1 }]);
    }

    #[test]
    fn cancelled_drag_cannot_move_a_card_on_later_mouse_up() {
        let mut form = ace_deal();
        let mut input = Controller::default();
        input.mouse_down(&mut form, 27, 161);
        input.mouse_move(&mut form, 280, 61);
        input.key(&mut form, Key::Escape, Mods::default());
        input.mouse_up(&mut form, 280, 61);
        assert!(form.game.foundations()[0].is_empty());
        assert_eq!(form.game.tableau()[0].cards.len(), 1);
    }
}
