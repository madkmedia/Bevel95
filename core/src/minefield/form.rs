//! Minesweeper's main window as a control kit form: Windows' frame, title bar and menu bar
//! (`crate::window`) around the game's board (`board`), with the mouse on the field, the menu
//! bar's keyboard rules and the game's keys.
//!
//! The mouse on the field, as mine games have it: the left button opens the square it is let go
//! on (shown pushed in while held); the right button goes round flag, question mark, nothing;
//! both together, or the middle one, on a number with all its flags open the squares around it.

use crate::fb::{Framebuffer, Icon, Rect};
use crate::kit::{self, Button, Effect, Key, Mods, Press};
use crate::minefield::art;
use crate::minefield::board::{self, Board, Held, View};
use crate::minefield::game::{Cover, Field, Game, Level, State};
use crate::paint::{Mark, Menu, MenuEntry};
use crate::text::{Fonts, split_access_key};
use crate::theme::{Era, Theme};
use crate::window::{self, ButtonLook, Frame, TitleButton, TitleLook};

pub const TITLE: &str = "Minesweeper";
/// The menu bar's titles.
pub const MENUS: [&str; 2] = ["&Game", "&Help"];
/// Windows' codes of the two function keys the game uses.
const VK_F1: u16 = 0x70;
const VK_F2: u16 = 0x71;

/// What the menus (and F1, F2) do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    NewGame,
    Level(Level),
    /// The Custom Field box.
    Custom,
    /// Question marks on or off.
    Marks,
    Color,
    Sound,
    BestTimes,
    Exit,
    HowToPlay,
    SearchHelp,
    UsingHelp,
    About,
}

/// The entries of the two menus; `None` is a separator.
type Entry = Option<(&'static str, Command)>;

const GAME_MENU: &[Entry] = &[
    Some(("&New\tF2", Command::NewGame)),
    None,
    Some(("&Beginner", Command::Level(Level::Beginner))),
    Some(("&Intermediate", Command::Level(Level::Intermediate))),
    Some(("&Expert", Command::Level(Level::Expert))),
    Some(("&Custom...", Command::Custom)),
    None,
    Some(("&Marks (?)", Command::Marks)),
    Some(("Co&lor", Command::Color)),
    Some(("&Sound", Command::Sound)),
    None,
    Some(("Best &Times...", Command::BestTimes)),
    None,
    Some(("E&xit", Command::Exit)),
];
const HELP_MENU: &[Entry] =
    &[Some(("&Help Topics", Command::HowToPlay)), None, Some(("&About Minesweeper", Command::About))];
const HELP_2000: &[Entry] = &[
    Some(("&Contents\tF1", Command::HowToPlay)),
    Some(("&Search for Help on...", Command::SearchHelp)),
    Some(("Using &Help", Command::UsingHelp)),
    None,
    Some(("&About Minesweeper...", Command::About)),
];

fn entries(era: Era, menu: usize) -> impl Iterator<Item = Entry> {
    let items = match menu {
        0 => GAME_MENU,
        1 if era == Era::Win2000 => HELP_2000,
        1 => HELP_MENU,
        _ => &[],
    };
    items.iter().copied().filter(move |e| era == Era::Win2000 || !matches!(e, Some((_, Command::Sound))))
}

/// What entry `entry` of menu `menu` does.
pub fn command_at(era: Era, menu: usize, entry: usize) -> Option<Command> {
    entries(era, menu).nth(entry).flatten().map(|(_, command)| command)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    /// The program's picture at the title bar's left end: the window menu.
    Icon,
    Minimize,
    Maximize,
    Close,
    /// A title of the menu bar.
    Menu(usize),
    /// The smiley-face button starts a new game.
    Mascot,
    /// The field of squares.
    Field,
    // The rest are not places in the window but things the form asks of its host.
    /// The window menu where the pointer is (a right click on the title bar).
    TitleMenu,
    /// The window menu by the keyboard (Alt+Space), its first entry highlighted.
    KeyMenu,
    /// A command dispatched through the same host path as a menu selection.
    Ask(Command),
    /// The first square was opened: the clock starts.
    Started,
    /// The game was won or lost: the clock stops.
    Ended,
}

/// The main window.
#[derive(Clone, Debug)]
pub struct MinefieldWindow {
    pub era: Era,
    pub level: Level,
    /// What the Custom Field box last said.
    pub custom: Field,
    /// Question marks are on.
    pub marks: bool,
    pub color: bool,
    pub sound: bool,
    pub game: Game,
    /// Picks where the next game's mines go.
    seed: u64,
    board: Board,
    titles: Vec<Rect>,
    icon: Icon,
    /// The window in front (its title bar shows it).
    active: bool,
    pressed: Option<Part>,
    /// The menu bar's title chosen with the keyboard, or whose menu is open.
    chosen: Option<usize>,
    menu_open: bool,
    /// The menu was reached with the keyboard (its first entry is highlighted, its access keys
    /// show in every look).
    by_keyboard: bool,
    /// The title under the pointer (Windows 2000 raises it).
    hot: Option<usize>,
    /// Alt was used: Windows 2000 underlines the access keys only then.
    keys: bool,
    // The mouse on the field.
    left: bool,
    right: bool,
    middle: bool,
    at: Option<(usize, usize)>,
    /// Both buttons were let go of once in this press: the button still down does nothing more.
    spent: bool,
}

impl MinefieldWindow {
    /// `seed`: anything that differs from start to start (the time).
    pub fn new(era: Era, level: Level, custom: Field, marks: bool, seed: u64) -> MinefieldWindow {
        let field = level.field(era).unwrap_or(custom);
        MinefieldWindow {
            era,
            level,
            custom,
            marks,
            color: true,
            sound: false,
            game: Game::new(field, seed),
            seed,
            board: Board { era, width: field.width, height: field.height, initial: true },
            titles: Vec::new(),
            icon: art::icon(era, 16),
            active: true,
            pressed: None,
            chosen: None,
            menu_open: false,
            by_keyboard: false,
            hot: None,
            keys: false,
            left: false,
            right: false,
            middle: false,
            at: None,
            spent: false,
        }
    }

    /// The field being played.
    pub fn field(&self) -> Field {
        self.game.field()
    }

    /// A fresh board takes the new edition's dimensions. A board the player has
    /// already opened or marked stays playable until the next game.
    pub fn set_era(&mut self, era: Era) {
        if self.era == era {
            return;
        }
        self.era = era;
        self.icon = art::icon(era, 16);
        self.board.era = era;
        self.custom = Field::custom(
            era, self.custom.width, self.custom.height, self.custom.mines, (Field::MAX_WIDTH, Field::MAX_HEIGHT),
        );
        let field = self.field();
        if self.level.field(era).unwrap_or(self.custom) != field
            && self.game.state() == State::Ready
            && (0..field.height).all(|y| (0..field.width).all(|x| self.game.square(x, y).cover == Cover::Covered))
        {
            self.new_game();
        }
        self.board.initial = true;
    }

    /// A new game on the same field.
    pub fn new_game(&mut self) {
        // Another seed each time (a step of a well-known generator; the game mixes it further).
        self.seed = self.seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        self.game = Game::new(self.level.field(self.era).unwrap_or(self.custom), self.seed);
        let field = self.game.field();
        if (field.width, field.height) != (self.board.width, self.board.height) {
            self.board.initial = false;
        }
        self.board.width = field.width;
        self.board.height = field.height;
        self.release_mouse();
    }

    /// Carries out a command that needs nothing from outside. False: it is the host's (a box to
    /// show, the window to close).
    pub fn command(&mut self, command: Command) -> bool {
        match command {
            Command::NewGame => self.new_game(),
            Command::Level(level) if level != Level::Custom => {
                self.level = level;
                self.new_game();
            }
            Command::Marks => self.marks = !self.marks,
            Command::Color => self.color = !self.color,
            Command::Sound if self.era == Era::Win2000 => self.sound = !self.sound,
            _ => return false,
        }
        true
    }

    /// The Custom Field box was answered: a new game on that field.
    pub fn play_custom(&mut self, field: Field) {
        self.custom = field;
        self.level = Level::Custom;
        self.new_game();
    }

    /// Menu `menu` as it shows now (the level in play and question marks checked).
    pub fn menu(&self, menu: usize) -> Menu {
        let entries = entries(self.era, menu)
            .map(|entry| {
                let Some((label, command)) = entry else {
                    return MenuEntry::separator();
                };
                let checked = match command {
                    Command::Level(level) => level == self.level,
                    Command::Custom => self.level == Level::Custom,
                    Command::Marks => self.marks,
                    Command::Color => self.color,
                    Command::Sound => self.sound,
                    _ => false,
                };
                MenuEntry { mark: checked.then_some(Mark::Check), ..MenuEntry::item(label, None) }
            })
            .collect();
        let mut menu = Menu::new(entries);
        menu.show_keys = self.era != Era::Win2000 || self.by_keyboard;
        if self.by_keyboard {
            menu.hover = Some(0);
        }
        menu
    }

    /// The host shows menu `menu` now (its title is drawn chosen), or none any more.
    pub fn show_menu(&mut self, menu: Option<usize>) {
        self.chosen = menu;
        self.menu_open = menu.is_some();
        if menu.is_none() {
            self.by_keyboard = false;
            self.keys = false;
        }
    }

    /// The open menu was left with Esc: its title stays chosen, the keyboard still on the bar.
    pub fn back_to_bar(&mut self, menu: usize) {
        self.chosen = Some(menu);
        self.menu_open = false;
        self.by_keyboard = true;
        self.keys = true;
    }

    /// The next menu is reached by the keyboard (the arrow keys between open menus) or by the
    /// pointer (moved onto another title).
    pub fn reached_by_keyboard(&mut self, by_keyboard: bool) {
        self.by_keyboard = by_keyboard;
    }

    /// Where a menu bar title is in the window.
    pub fn title_rect(&self, menu: usize) -> Option<Rect> {
        self.titles.get(menu).copied()
    }

    /// The squares shown pushed in now.
    pub fn held(&self) -> Held {
        let Some((x, y)) = self.at else {
            return Held::Nothing;
        };
        if self.game.over() || self.spent {
            Held::Nothing
        } else if self.middle || (self.left && self.right) {
            Held::Around(x, y)
        } else if self.left {
            Held::One(x, y)
        } else {
            Held::Nothing
        }
    }

    /// The seconds on the clock are set by the host, which has the time.
    pub fn set_seconds(&mut self, seconds: u32) -> bool {
        self.game.set_seconds(seconds)
    }

    fn release_mouse(&mut self) {
        (self.left, self.right, self.middle, self.at, self.spent) = (false, false, false, None, false);
    }

    fn leave_bar(&mut self) {
        if !self.menu_open {
            self.chosen = None;
            self.by_keyboard = false;
            self.keys = false;
        }
    }

    /// What the host must hear of after a move: the clock starts with the first opened square
    /// and stops with the game.
    fn after_move(&self, before: State) -> Vec<Effect<Part>> {
        let mut effects = Vec::new();
        if before == State::Ready && self.game.state() != State::Ready {
            effects.push(Effect::Activate(Part::Started));
        }
        if !matches!(before, State::Won | State::Lost) && self.game.over() {
            effects.push(Effect::Activate(Part::Ended));
        }
        effects
    }

    fn open_here(&mut self) -> Vec<Effect<Part>> {
        let Some((x, y)) = self.at else {
            return Vec::new();
        };
        let before = self.game.state();
        self.game.open(x, y);
        self.after_move(before)
    }

    fn open_around_here(&mut self) -> Vec<Effect<Part>> {
        let Some((x, y)) = self.at else {
            return Vec::new();
        };
        let before = self.game.state();
        self.game.open_around(x, y);
        self.after_move(before)
    }

    /// The menu whose title has `ch` as its access key.
    fn menu_for(&self, ch: char) -> Option<usize> {
        let ch = ch.to_ascii_lowercase();
        MENUS.iter().position(|label| {
            let (text, at) = split_access_key(label);
            at.and_then(|k| text.chars().nth(k)).is_some_and(|c| c.to_ascii_lowercase() == ch)
        })
    }

    fn open_by_keyboard(&mut self, menu: usize) -> Vec<Effect<Part>> {
        self.by_keyboard = true;
        self.keys = true;
        vec![Effect::Activate(Part::Menu(menu))]
    }

    fn title_buttons(&self) -> [ButtonLook; 3] {
        let width = self.board.window_size().0;
        let look = |button, part: Part| ButtonLook {
            button,
            rect: window::title_button_rect(width, button),
            pressed: self.pressed == Some(part),
            enabled: self.enabled_part(part),
        };
        [
            look(TitleButton::Minimize, Part::Minimize),
            look(TitleButton::Maximize, Part::Maximize),
            look(TitleButton::Close, Part::Close),
        ]
    }

    /// The window cannot be made bigger: its size is the field's.
    fn enabled_part(&self, part: Part) -> bool {
        part != Part::Maximize
    }
}

impl kit::Form for MinefieldWindow {
    type Part = Part;

    fn title(&self) -> &str {
        TITLE
    }
    fn size(&self) -> (i32, i32) {
        self.board.window_size()
    }
    fn layout(&mut self, fonts: &Fonts) {
        self.titles = window::menu_bar_layout(fonts, &MENUS);
        let field = self.game.field();
        self.board.width = field.width;
        self.board.height = field.height;
    }
    fn paint(&self, fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts) {
        let buttons = self.title_buttons();
        let frame =
            Frame { era: self.era, title: TITLE, icon: Some(&self.icon), active: self.active, buttons: &buttons };
        window::paint_frame(fb, theme, fonts, &frame);
        let looks: Vec<TitleLook> = (0..MENUS.len())
            .map(|i| {
                if self.chosen == Some(i) {
                    TitleLook::Chosen
                } else if self.hot == Some(i) && self.chosen.is_none() {
                    TitleLook::Hot
                } else {
                    TitleLook::Plain
                }
            })
            .collect();
        let show_keys = self.era != Era::Win2000 || self.keys;
        window::paint_menu_bar(fb, theme, fonts, self.era, &MENUS, &self.titles, &looks, show_keys, self.active);
        let view = View { game: &self.game, held: self.held(), button_down: self.pressed == Some(Part::Mascot) };
        board::paint(fb, &self.board, &view, self.color);
        if theme.depth == crate::colors16::Depth::Sixteen {
            // 95's and 98's face bitmap has one color outside the 16 (BFBF00): by the look's rule.
            let b = self.board.button();
            for y in b.y.max(0)..b.bottom().min(fb.height) {
                for x in b.x.max(0)..b.right().min(fb.width) {
                    let p = fb.get(x, y);
                    fb.set(x, y, crate::colors16::convert(self.era, p));
                }
            }
        }
    }

    fn hit(&self, x: i32, y: i32) -> Option<Part> {
        let places = [Part::Minimize, Part::Maximize, Part::Close, Part::Icon, Part::Mascot, Part::Field];
        if let Some(i) = self.titles.iter().position(|r| r.contains(x, y)) {
            return Some(Part::Menu(i));
        }
        places.into_iter().find(|&p| self.rect(p).is_some_and(|r| r.contains(x, y)))
    }
    fn rect(&self, part: Part) -> Option<Rect> {
        let width = self.board.window_size().0;
        Some(match part {
            Part::Icon => Rect::new(window::FRAME + 2, window::FRAME + 1, 16, 16),
            Part::Minimize => window::title_button_rect(width, TitleButton::Minimize),
            Part::Maximize => window::title_button_rect(width, TitleButton::Maximize),
            Part::Close => window::title_button_rect(width, TitleButton::Close),
            Part::Menu(i) => return self.title_rect(i),
            Part::Mascot => self.board.button(),
            // The squares, without the frame around them.
            Part::Field => self.board.grid(),
            Part::TitleMenu | Part::KeyMenu | Part::Ask(_) | Part::Started | Part::Ended => return None,
        })
    }
    fn enabled(&self, part: Part) -> bool {
        self.enabled_part(part)
    }
    fn focus(&self) -> Part {
        Part::Field
    }
    fn set_focus(&mut self, _part: Part) {}
    fn tab_stops(&self) -> Vec<Part> {
        Vec::new()
    }
    fn set_pressed(&mut self, part: Option<Part>) {
        self.pressed = part;
    }
    /// The menu bar keeps track of its own underlines (`keys`).
    fn show_keys(&mut self) {}
    fn enter(&self) -> Part {
        Part::Field
    }
    fn escape(&self) -> Part {
        Part::Field
    }

    /// Every key is the window's own: there are no controls to tab through.
    fn key(&mut self, key: Key, _mods: Mods) -> Option<Vec<Effect<Part>>> {
        if let Some(i) = self.chosen.filter(|_| !self.menu_open) {
            // A title is chosen with the keyboard: the arrows move along the bar, Down, Up and
            // Enter open the menu, Esc lets go.
            let n = MENUS.len();
            match key {
                Key::Left => self.chosen = Some((i + n - 1) % n),
                Key::Right => self.chosen = Some((i + 1) % n),
                Key::Down | Key::Up | Key::Enter => return Some(self.open_by_keyboard(i)),
                Key::Modifier => {}
                // A letter comes again as a typed character (`typed`), where it may open a menu.
                Key::Other(vk) if (u16::from(b'A')..=u16::from(b'Z')).contains(&vk) => {}
                _ => self.leave_bar(),
            }
            return Some(Vec::new());
        }
        // Any other key ends what Alt began (Windows 2000 hides the access keys again).
        if key != Key::Modifier {
            self.keys = false;
        }
        match key {
            Key::Other(VK_F2) => return Some(vec![Effect::Activate(Part::Ask(Command::NewGame))]),
            Key::Other(VK_F1) => return Some(vec![Effect::Activate(Part::Ask(Command::HowToPlay))]),
            _ => {}
        }
        Some(Vec::new())
    }
    /// Alt going down shows the access keys (Windows 2000 hides them until then). The key stays
    /// Windows' own: letting go of it alone comes back as `menu_key`.
    fn alt_key(&mut self, key: Key) -> bool {
        if key == Key::Modifier {
            self.keys = true;
        }
        false
    }
    fn typed(&mut self, ch: char) -> Vec<Effect<Part>> {
        if self.chosen.is_some() && !self.menu_open {
            return match self.menu_for(ch) {
                Some(menu) => self.open_by_keyboard(menu),
                None => Vec::new(),
            };
        }
        Vec::new()
    }
    /// Alt+letter opens that menu; Alt+Space the window menu.
    fn access(&mut self, ch: char) -> Vec<Effect<Part>> {
        if ch == ' ' {
            return vec![Effect::Activate(Part::KeyMenu)];
        }
        match self.menu_for(ch) {
            Some(menu) => self.open_by_keyboard(menu),
            None => Vec::new(),
        }
    }
    fn space(&mut self) -> Vec<Effect<Part>> {
        Vec::new()
    }
    fn menu_key(&mut self) -> Vec<Effect<Part>> {
        if self.chosen.is_some() {
            self.leave_bar();
        } else {
            self.chosen = Some(0);
            self.by_keyboard = true;
            self.keys = true;
        }
        Vec::new()
    }

    fn press(&mut self, part: Option<Part>, x: i32, y: i32) -> Press<Part> {
        self.leave_bar();
        match part {
            Some(p @ (Part::Icon | Part::Menu(_))) => Press::Act(p),
            Some(p @ (Part::Minimize | Part::Close | Part::Mascot)) => Press::Hold(p),
            Some(Part::Field) if !self.game.over() => {
                self.left = true;
                self.at = self.board.square_at(x, y);
                Press::Grab(Part::Field, 0)
            }
            Some(_) => Press::Done,
            None => Press::Nothing,
        }
    }
    fn release(&mut self, part: Part) -> Vec<Effect<Part>> {
        if part == Part::Mascot {
            vec![Effect::Activate(Part::Ask(Command::NewGame))]
        } else {
            vec![Effect::Activate(part)]
        }
    }
    fn drag(&mut self, _part: Part, _grab_y: i32, _value: i32, x: i32, y: i32) -> bool {
        let at = self.board.square_at(x, y);
        std::mem::replace(&mut self.at, at) != at
    }
    /// The left button came up.
    fn dropped(&mut self, _part: Part, x: i32, y: i32) -> Vec<Effect<Part>> {
        self.at = self.board.square_at(x, y);
        let was_down = std::mem::replace(&mut self.left, false);
        let effects = if !was_down || self.spent {
            Vec::new()
        } else if self.right || self.middle {
            self.spent = true;
            self.open_around_here()
        } else {
            self.open_here()
        };
        if !(self.right || self.middle) {
            self.release_mouse();
        }
        effects
    }
    fn button(&mut self, button: Button, down: bool, x: i32, y: i32) -> Vec<Effect<Part>> {
        let square = self.board.square_at(x, y);
        let mut effects = Vec::new();
        if down {
            self.leave_bar();
            if self.game.over() {
                return effects;
            }
            self.at = square;
            match button {
                Button::Right => {
                    self.right = true;
                    // Alone, it marks the square at once; with the left one it is a both-button press.
                    if !self.left
                        && !self.middle
                        && let Some((sx, sy)) = square
                    {
                        self.game.mark(sx, sy, self.marks);
                    }
                }
                Button::Middle => self.middle = true,
            }
            return effects;
        }
        let was_down = match button {
            Button::Right => std::mem::replace(&mut self.right, false),
            Button::Middle => std::mem::replace(&mut self.middle, false),
        };
        self.at = square;
        let both = button == Button::Middle || self.left;
        if was_down && both && !self.spent {
            effects = self.open_around_here();
            self.spent = true;
        }
        if !(self.left || self.right || self.middle) {
            self.release_mouse();
        }
        // A right click on the title bar: the window menu.
        if button == Button::Right && kit::TITLE_ROWS.contains(&y) && matches!(self.hit(x, y), None | Some(Part::Icon))
        {
            let width = self.board.window_size().0;
            if (window::FRAME..width - window::FRAME).contains(&x) {
                effects.push(Effect::Activate(Part::TitleMenu));
            }
        }
        effects
    }
    fn set_active(&mut self, active: bool) {
        self.active = active;
        if !active {
            self.release_mouse();
            self.leave_bar();
            self.hot = None;
        }
    }
    /// The pointer moved with the left button up: the other buttons may still hold squares, and
    /// Windows 2000 raises the menu title under the pointer.
    fn hover(&mut self, x: i32, y: i32) -> bool {
        let mut changed = false;
        if self.right || self.middle {
            let at = self.board.square_at(x, y);
            changed = std::mem::replace(&mut self.at, at) != at;
        }
        let hot = self.titles.iter().position(|r| r.contains(x, y));
        changed | (std::mem::replace(&mut self.hot, hot) != hot && self.era == Era::Win2000)
    }
    fn pointer_left(&mut self) -> bool {
        self.hot.take().is_some() && self.era == Era::Win2000
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::{Controller, Form};
    use crate::text::Font;

    fn window() -> (MinefieldWindow, Controller<Part>) {
        let regular = Font::parse(include_str!("../../tests/fixtures/ms-sans-serif-11px.font")).unwrap();
        let bold = Font::parse(include_str!("../../tests/fixtures/ms-sans-serif-11px-bold.font")).unwrap();
        let mut w =
            MinefieldWindow::new(Era::Win95, Level::Beginner, Field { width: 12, height: 10, mines: 20 }, true, 7);
        w.layout(&Fonts { regular: &regular, bold: &bold });
        (w, Controller::default())
    }

    /// The middle of square (x, y).
    fn at(w: &MinefieldWindow, x: usize, y: usize) -> (i32, i32) {
        let r = w.board.square(x, y);
        (r.x + 8, r.y + 8)
    }

    fn click(w: &mut MinefieldWindow, c: &mut Controller<Part>, x: usize, y: usize) -> Vec<Effect<Part>> {
        let (px, py) = at(w, x, y);
        c.mouse_down(w, px, py);
        c.mouse_up(w, px, py)
    }

    #[test]
    fn the_left_button_opens_where_it_is_let_go() {
        let (mut w, mut c) = window();
        assert_eq!(w.size(), (160, 239));
        let (px, py) = at(&w, 4, 4);
        assert_eq!(c.mouse_down(&mut w, px, py), vec![Effect::Capture]);
        assert_eq!(w.held(), Held::One(4, 4));
        assert_eq!(w.game.state(), State::Ready, "nothing opens while the button is down");
        // dragged to another square, it is that one that is held, then opened
        let (qx, qy) = at(&w, 5, 4);
        assert!(c.mouse_move(&mut w, qx, qy));
        assert_eq!(w.held(), Held::One(5, 4));
        let effects = c.mouse_up(&mut w, qx, qy);
        assert_eq!(effects, vec![Effect::Release, Effect::Activate(Part::Started)]);
        assert_eq!(w.game.square(5, 4).cover, Cover::Open);
        assert_eq!(w.held(), Held::Nothing);
        // let go off the field: nothing opens
        let before = w.game.clone();
        c.mouse_down(&mut w, px, py);
        c.mouse_up(&mut w, 2, 200);
        assert_eq!(format!("{:?}", w.game), format!("{before:?}"));
    }

    /// A game with known mines on 8 by 8: (3,0) and (0,2), and two more that fence off the
    /// bottom right corner, so that opening the rest does not win the game.
    fn known() -> (MinefieldWindow, Controller<Part>) {
        let (mut w, c) = window();
        w.game = Game::with_mines(8, 8, &[(3, 0), (0, 2), (7, 5), (5, 7)]);
        (w, c)
    }

    #[test]
    fn the_right_button_marks_at_once_and_goes_round() {
        let (mut w, mut c) = known();
        let (px, py) = at(&w, 3, 0);
        assert_eq!(c.button(&mut w, Button::Right, true, px, py), vec![Effect::Capture]);
        assert_eq!(w.game.square(3, 0).cover, Cover::Flagged, "as the button goes down");
        assert_eq!(c.button(&mut w, Button::Right, false, px, py), vec![Effect::Release]);
        assert_eq!(w.game.mines_left(), 3);
        c.button(&mut w, Button::Right, true, px, py);
        c.button(&mut w, Button::Right, false, px, py);
        assert_eq!(w.game.square(3, 0).cover, Cover::Marked);
        // Turning Marks off keeps existing question marks. New cycles skip them.
        assert!(w.command(Command::Marks));
        assert_eq!(w.game.square(3, 0).cover, Cover::Marked);
        c.button(&mut w, Button::Right, true, px, py);
        c.button(&mut w, Button::Right, false, px, py);
        assert_eq!(w.game.square(3, 0).cover, Cover::Covered);
        for _ in 0..2 {
            c.button(&mut w, Button::Right, true, px, py);
            c.button(&mut w, Button::Right, false, px, py);
        }
        assert_eq!(w.game.square(3, 0).cover, Cover::Covered);
    }

    #[test]
    fn both_buttons_open_around_a_number_with_its_flags() {
        let (mut w, mut c) = known();
        click(&mut w, &mut c, 2, 1); // a "1" next to the mine at (3,0)
        assert_eq!(w.game.square(2, 1).near, 1);
        let (fx, fy) = at(&w, 3, 0);
        c.button(&mut w, Button::Right, true, fx, fy);
        c.button(&mut w, Button::Right, false, fx, fy);
        // left, then right as well: the nine squares show held; letting one go opens them
        let (px, py) = at(&w, 2, 1);
        c.mouse_down(&mut w, px, py);
        c.button(&mut w, Button::Right, true, px, py);
        assert_eq!(w.held(), Held::Around(2, 1));
        assert_eq!(w.game.square(3, 0).cover, Cover::Flagged, "the right button marked nothing this time");
        let effects = c.button(&mut w, Button::Right, false, px, py);
        assert_eq!(effects, vec![], "the mouse stays captured while the left button is down");
        assert_eq!(w.game.square(1, 0).cover, Cover::Open);
        assert_eq!(w.held(), Held::Nothing, "the button still down does nothing more");
        assert_eq!(c.mouse_up(&mut w, px, py), vec![Effect::Release]);
        // the middle button alone does the same
        let (mut w, mut c) = known();
        click(&mut w, &mut c, 2, 1);
        w.game.mark(3, 0, false);
        c.button(&mut w, Button::Middle, true, px, py);
        assert_eq!(w.held(), Held::Around(2, 1));
        c.button(&mut w, Button::Middle, false, px, py);
        assert_eq!(w.game.square(1, 0).cover, Cover::Open);
    }

    #[test]
    fn the_host_hears_when_the_game_ends() {
        let (mut w, mut c) = known();
        assert_eq!(click(&mut w, &mut c, 2, 1), vec![Effect::Release, Effect::Activate(Part::Started)]);
        assert_eq!(click(&mut w, &mut c, 3, 0), vec![Effect::Release, Effect::Activate(Part::Ended)]);
        assert_eq!(w.game.state(), State::Lost);
        // nothing more can be held or opened; the mascot's button starts afresh
        let (px, py) = at(&w, 5, 5);
        assert_eq!(c.mouse_down(&mut w, px, py), vec![]);
        let button = w.board.button();
        c.mouse_down(&mut w, button.x + 3, button.y + 3);
        assert_eq!(w.pressed, Some(Part::Mascot));
        assert_eq!(
            c.mouse_up(&mut w, button.x + 3, button.y + 3),
            vec![Effect::Release, Effect::Activate(Part::Ask(Command::NewGame))]
        );
        w.command(Command::NewGame);
        assert_eq!(w.game.state(), State::Ready);
        // F2 as well
        click(&mut w, &mut c, 4, 4);
        assert_eq!(
            c.key(&mut w, Key::Other(VK_F2), Mods::default()),
            vec![Effect::Activate(Part::Ask(Command::NewGame))]
        );
        w.command(Command::NewGame);
        assert_eq!(w.game.state(), State::Ready);
        assert_eq!(
            c.key(&mut w, Key::Other(VK_F1), Mods::default()),
            vec![Effect::Activate(Part::Ask(Command::HowToPlay))]
        );
    }

    #[test]
    fn levels_change_the_field_and_the_window() {
        let (mut w, _) = window();
        assert!(w.command(Command::Level(Level::Beginner)));
        assert_eq!(w.size(), (160, 239));
        w.play_custom(Field { width: 8, height: 8, mines: 11 });
        assert_eq!(w.size(), (160, 239), "changing only the mine count keeps the startup width");
        assert!(w.command(Command::Level(Level::Expert)));
        assert_eq!((w.field(), w.size()), (Field { width: 30, height: 16, mines: 99 }, (510, 367)));
        w.command(Command::Level(Level::Beginner));
        assert_eq!(w.size(), (158, 239));
        assert!(!w.command(Command::Custom), "the box is the host's to show");
        w.play_custom(Field { width: 12, height: 10, mines: 20 });
        assert_eq!((w.level, w.field().mines, w.size().0), (Level::Custom, 20, 12 * 16 + 30));
        let checked: Vec<usize> = w.menu(0).entries.iter().enumerate().filter_map(|(i, e)| e.mark.map(|_| i)).collect();
        assert_eq!(checked, [5, 7, 8]);
        assert_eq!(command_at(w.era, 0, 12), Some(Command::Exit));
        assert_eq!(command_at(w.era, 0, 1), None);
        assert_eq!(command_at(w.era, 1, 0), Some(Command::HowToPlay));
        assert_eq!(command_at(Era::Win2000, 0, 9), Some(Command::Sound));
        assert_eq!(command_at(Era::Win2000, 1, 1), Some(Command::SearchHelp));
    }

    #[test]
    fn changing_look_keeps_played_and_marked_boards() {
        let (mut w, _) = window();
        w.set_era(Era::Win2000);
        assert_eq!(w.size(), (170, 251));
        w.game.mark(8, 8, true);
        w.set_era(Era::Win98);
        assert_eq!(w.field().width, 9);
        assert_eq!(w.game.square(8, 8).cover, Cover::Flagged);
        w.new_game();
        assert_eq!(w.field().width, 8);
        w.game.open(3, 3);
        w.set_era(Era::Win2000);
        assert_eq!(w.field().width, 8);
        assert_eq!(w.game.square(3, 3).cover, Cover::Open);
        w.new_game();
        assert_eq!(w.field().width, 9);
    }

    #[test]
    fn the_menu_bar_by_keyboard_and_by_mouse() {
        let (mut w, mut c) = window();
        // Alt alone chooses the first title; Right the next; Down opens it; Esc lets go
        assert_eq!(w.menu_key(), vec![]);
        assert_eq!(w.chosen, Some(0));
        c.key(&mut w, Key::Right, Mods::default());
        assert_eq!(w.chosen, Some(1));
        assert_eq!(c.key(&mut w, Key::Down, Mods::default()), vec![Effect::Activate(Part::Menu(1))]);
        assert_eq!(w.menu(1).hover, Some(0), "from the keyboard its first entry is highlighted");
        c.key(&mut w, Key::Escape, Mods::default());
        assert_eq!(w.chosen, None);
        // a letter on the chosen bar, and Alt+letter from anywhere
        w.menu_key();
        assert_eq!(w.typed('h'), vec![Effect::Activate(Part::Menu(1))]);
        w.show_menu(None);
        assert_eq!(c.access(&mut w, 'g'), vec![Effect::Activate(Part::Menu(0))]);
        assert_eq!(c.access(&mut w, ' '), vec![Effect::Activate(Part::KeyMenu)]);
        // a click on a title acts as the button goes down
        w.show_menu(None);
        let title = w.title_rect(0).unwrap();
        assert_eq!(c.mouse_down(&mut w, title.x + 5, title.y + 5), vec![Effect::Activate(Part::Menu(0))]);
        assert_eq!(w.menu(0).hover, None, "by the mouse nothing is highlighted");
        // the title bar: dragging moves the window, a right click asks for the window menu
        assert_eq!(c.mouse_down(&mut w, 60, 10), vec![Effect::MoveWindow]);
        c.button(&mut w, Button::Right, true, 60, 10);
        assert_eq!(
            c.button(&mut w, Button::Right, false, 60, 10),
            vec![Effect::Release, Effect::Activate(Part::TitleMenu)]
        );
        // Maximize is grayed out
        let max = w.rect(Part::Maximize).unwrap();
        assert_eq!(c.mouse_down(&mut w, max.x + 2, max.y + 2), vec![]);
        let close = w.rect(Part::Close).unwrap();
        c.mouse_down(&mut w, close.x + 2, close.y + 2);
        assert_eq!(c.mouse_up(&mut w, close.x + 2, close.y + 2), vec![Effect::Release, Effect::Activate(Part::Close)]);
    }

    /// For live tests (tools\dev\minefield.ps1 -Seed): where a seed's first Beginner game has
    /// its mines when the first square opened is (4, 4), and the squares to click to win it.
    /// SEED=7 cargo test -p bevel-core --lib prints_a_seeds_field -- --ignored --nocapture
    #[test]
    #[ignore]
    fn prints_a_seeds_field() {
        let seed = std::env::var("SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(7);
        let mut w =
            MinefieldWindow::new(Era::Win95, Level::Beginner, Field { width: 9, height: 9, mines: 10 }, true, seed);
        w.game.open(4, 4);
        let mut safe = Vec::new();
        let field = w.field();
        for y in 0..field.height {
            let row: String = (0..field.width).map(|x| if w.game.square(x, y).mine { '*' } else { '.' }).collect();
            println!("{row}");
            safe.extend(
                (0..field.width)
                    .filter(|&x| !w.game.square(x, y).mine && w.game.square(x, y).cover != Cover::Open)
                    .map(|x| format!("{x} {y}")),
            );
        }
        println!("first: 4 4\nthen: {}", safe.join("; "));
        let mines: Vec<String> = (0..field.squares())
            .map(|i| (i % field.width, i / field.width))
            .filter(|&(x, y)| w.game.square(x, y).mine)
            .map(|(x, y)| format!("{x} {y}"))
            .collect();
        println!("mines: {}", mines.join("; "));
    }

    #[test]
    fn going_behind_lets_go_of_everything() {
        let (mut w, mut c) = window();
        let (px, py) = at(&w, 4, 4);
        c.mouse_down(&mut w, px, py);
        w.menu_key();
        w.set_active(false);
        assert_eq!((w.held(), w.chosen), (Held::Nothing, None));
    }
}
