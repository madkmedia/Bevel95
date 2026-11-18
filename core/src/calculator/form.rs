//! Calculator's main window as a control kit form: Windows' frame, title bar and menu bar
//! (`crate::window`) around each Windows' Standard and Scientific views, measured on the
//! Reference PCs in True Color (docs/theme-spec.md, "Calculator").
//!
//! - Windows 95: 270×228 inside (Scientific 495×296), an etched line under the menu bar, bold
//!   10-point captions in darker colors (their pixels: `captions95`), buttons 36×28 on a 40×34
//!   pitch, "Back", the display text 8 from its right edge (Scientific 5); nothing is ever grayed.
//! - Windows 98 and 2000: 254×208 inside (Scientific 474×265), MS Sans Serif 8 captions (2000's
//!   Calculator did not use Tahoma), centered as Windows centers a button's text, buttons 36×29
//!   (rows 33, 32, 33 apart: dialog units rounded), "Backspace"; captions of buttons that do
//!   nothing now in gray; 2000 in its own colors, a space after its numbers, a full-width
//!   Scientific display and Qword, and without 98's lost corner of M+ (Scientific: Dat).
//! - No button ever shows a focus rectangle; a key flashes its button pushed in for a moment;
//!   the mouse holds one pushed in while it is on it.

use super::engine::{self, Angle, Engine, Key as CalcKey, Radix, Word};
use crate::dialog::{self, OPTION_OFF, OPTION_ON};
use crate::fb::{Framebuffer, Icon, Rect};
use crate::kit::{self, Button, Effect, Key, Mods, Press};
use crate::paint::{Mark, Menu, MenuEntry};
use crate::text::{Font, Fonts, split_access_key};
use crate::theme::{Era, Theme, rgb};
use crate::window::{self, ButtonLook, Frame, TitleButton, TitleLook};

pub const TITLE: &str = "Calculator";
/// The menu bar's titles.
pub const MENUS: [&str; 3] = ["&Edit", "&View", "&Help"];

const VK_BACK: u16 = 0x08;
const VK_INSERT: u16 = 0x2D;
const VK_F2: u16 = 0x71;
const VK_F3: u16 = 0x72;
const VK_F5: u16 = 0x74;
const VK_F6: u16 = 0x75;
const VK_F7: u16 = 0x76;
const VK_F8: u16 = 0x77;
const VK_F9: u16 = 0x78;
const VK_F12: u16 = 0x7B;

/// How many host ticks (50 ms) a key's button stays pushed in: the Reference PCs showed it
/// pushed in about 150 ms after the key, and up again by 250 ms.
pub const FLASH_TICKS: u8 = 4;

/// What the menus do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Copy,
    Paste,
    Standard,
    Scientific,
    DigitGrouping,
    HelpTopics,
    About,
    Radix(Radix),
    Angle(Angle),
    Word(Word),
    /// Sta: the Statistics Box opens (or comes to the front).
    Statistics,
}

type Entry = Option<(&'static str, Command)>;

const EDIT_MENU: &[Entry] = &[Some(("&Copy\tCtrl+C", Command::Copy)), Some(("&Paste\tCtrl+V", Command::Paste))];
const VIEW_95: &[Entry] = &[Some(("&Scientific", Command::Scientific)), Some(("S&tandard", Command::Standard))];
const VIEW_98: &[Entry] = &[Some(("S&tandard", Command::Standard)), Some(("&Scientific", Command::Scientific))];
const HELP_MENU: &[Entry] = &[Some(("&Help Topics", Command::HelpTopics)), None, Some(("&About Calculator", Command::About))];
/// Windows 2000's View menu in the Scientific view: the bases, then the angles (Dec) or the
/// word sizes, as the option buttons are.
const VIEW_2000_BASES: &[Entry] = &[
    Some(("&Hex\tF5", Command::Radix(Radix::Hex))),
    Some(("&Decimal\tF6", Command::Radix(Radix::Dec))),
    Some(("&Octal\tF7", Command::Radix(Radix::Oct))),
    Some(("&Binary\tF8", Command::Radix(Radix::Bin))),
    None,
];
const VIEW_2000_ANGLES: &[Entry] = &[
    Some(("D&egrees\tF2", Command::Angle(Angle::Degrees))),
    Some(("&Radians\tF3", Command::Angle(Angle::Radians))),
    Some(("&Grads\tF4", Command::Angle(Angle::Grads))),
    None,
];
const VIEW_2000_WORDS: &[Entry] = &[
    Some(("&Qword\tF12", Command::Word(Word::Qword))),
    Some(("Dwo&rd\tF2", Command::Word(Word::Dword))),
    Some(("&Word\tF3", Command::Word(Word::Word))),
    Some(("B&yte\tF4", Command::Word(Word::Byte))),
    None,
];

/// One button: its key, its caption in 98 and 2000 (95's are pictures: `captions95`), and the
/// caption's color in 95 and in 98/2000.
struct ButtonDef {
    key: CalcKey,
    label: &'static str,
    color_95: u32,
    color: u32,
}

const BLUE: u32 = rgb(0x0000FF);
const RED: u32 = rgb(0xFF0000);
const NAVY: u32 = rgb(0x000080);
const MAROON: u32 = rgb(0x800000);
const PURPLE: u32 = rgb(0x800080);
const MAGENTA: u32 = rgb(0xFF00FF);
const GRAY: u32 = rgb(0x808080);

const fn def(key: CalcKey, label: &'static str, color_95: u32, color: u32) -> ButtonDef {
    ButtonDef { key, label, color_95, color }
}

/// The Standard view's buttons, in the order `button_rect` places them: the top row, then
/// column by column.
#[rustfmt::skip]
const STANDARD: [ButtonDef; 27] = [
    def(CalcKey::Back, "Backspace", MAROON, RED),
    def(CalcKey::ClearEntry, "CE", MAROON, RED),
    def(CalcKey::Clear, "C", MAROON, RED),
    def(CalcKey::MemoryClear, "MC", RED, RED),
    def(CalcKey::MemoryRecall, "MR", RED, RED),
    def(CalcKey::MemoryStore, "MS", RED, RED),
    def(CalcKey::MemoryAdd, "M+", RED, RED),
    def(CalcKey::Digit(7), "7", BLUE, BLUE),
    def(CalcKey::Digit(4), "4", BLUE, BLUE),
    def(CalcKey::Digit(1), "1", BLUE, BLUE),
    def(CalcKey::Digit(0), "0", BLUE, BLUE),
    def(CalcKey::Digit(8), "8", BLUE, BLUE),
    def(CalcKey::Digit(5), "5", BLUE, BLUE),
    def(CalcKey::Digit(2), "2", BLUE, BLUE),
    def(CalcKey::Negate, "+/-", BLUE, BLUE),
    def(CalcKey::Digit(9), "9", BLUE, BLUE),
    def(CalcKey::Digit(6), "6", BLUE, BLUE),
    def(CalcKey::Digit(3), "3", BLUE, BLUE),
    def(CalcKey::Point, ".", BLUE, BLUE),
    def(CalcKey::Divide, "/", RED, RED),
    def(CalcKey::Multiply, "*", RED, RED),
    def(CalcKey::Subtract, "-", RED, RED),
    def(CalcKey::Add, "+", RED, RED),
    def(CalcKey::Sqrt, "sqrt", NAVY, BLUE),
    def(CalcKey::Percent, "%", NAVY, BLUE),
    def(CalcKey::Reciprocal, "1/x", NAVY, BLUE),
    def(CalcKey::Equals, "=", RED, RED),
];

/// The Scientific view's buttons: the top row, then the eleven columns of five.
#[rustfmt::skip]
const SCIENTIFIC: [ButtonDef; 58] = [
    def(CalcKey::Back, "Backspace", MAROON, RED),
    def(CalcKey::ClearEntry, "CE", MAROON, RED),
    def(CalcKey::Clear, "C", MAROON, RED),
    def(CalcKey::Sta, "Sta", NAVY, BLUE),
    def(CalcKey::Ave, "Ave", NAVY, BLUE),
    def(CalcKey::Sum, "Sum", NAVY, BLUE),
    def(CalcKey::Dev, "s", NAVY, BLUE),
    def(CalcKey::Dat, "Dat", NAVY, BLUE),
    def(CalcKey::FixedExp, "F-E", PURPLE, MAGENTA),
    def(CalcKey::Dms, "dms", PURPLE, MAGENTA),
    def(CalcKey::Sin, "sin", PURPLE, MAGENTA),
    def(CalcKey::Cos, "cos", PURPLE, MAGENTA),
    def(CalcKey::Tan, "tan", PURPLE, MAGENTA),
    def(CalcKey::Open, "(", PURPLE, MAGENTA),
    def(CalcKey::Exp, "Exp", PURPLE, MAGENTA),
    def(CalcKey::Power, "x^y", PURPLE, MAGENTA),
    def(CalcKey::Cube, "x^3", PURPLE, MAGENTA),
    def(CalcKey::Square, "x^2", PURPLE, MAGENTA),
    def(CalcKey::Close, ")", PURPLE, MAGENTA),
    def(CalcKey::Ln, "ln", PURPLE, MAGENTA),
    def(CalcKey::Log, "log", PURPLE, MAGENTA),
    def(CalcKey::Factorial, "n!", PURPLE, MAGENTA),
    def(CalcKey::Reciprocal, "1/x", PURPLE, MAGENTA),
    def(CalcKey::MemoryClear, "MC", RED, RED),
    def(CalcKey::MemoryRecall, "MR", RED, RED),
    def(CalcKey::MemoryStore, "MS", RED, RED),
    def(CalcKey::MemoryAdd, "M+", RED, RED),
    def(CalcKey::Pi, "pi", NAVY, BLUE),
    def(CalcKey::Digit(7), "7", BLUE, BLUE),
    def(CalcKey::Digit(4), "4", BLUE, BLUE),
    def(CalcKey::Digit(1), "1", BLUE, BLUE),
    def(CalcKey::Digit(0), "0", BLUE, BLUE),
    def(CalcKey::Digit(10), "A", NAVY, BLUE),
    def(CalcKey::Digit(8), "8", BLUE, BLUE),
    def(CalcKey::Digit(5), "5", BLUE, BLUE),
    def(CalcKey::Digit(2), "2", BLUE, BLUE),
    def(CalcKey::Negate, "+/-", BLUE, BLUE),
    def(CalcKey::Digit(11), "B", NAVY, BLUE),
    def(CalcKey::Digit(9), "9", BLUE, BLUE),
    def(CalcKey::Digit(6), "6", BLUE, BLUE),
    def(CalcKey::Digit(3), "3", BLUE, BLUE),
    def(CalcKey::Point, ".", BLUE, BLUE),
    def(CalcKey::Digit(12), "C", NAVY, BLUE),
    def(CalcKey::Divide, "/", RED, RED),
    def(CalcKey::Multiply, "*", RED, RED),
    def(CalcKey::Subtract, "-", RED, RED),
    def(CalcKey::Add, "+", RED, RED),
    def(CalcKey::Digit(13), "D", NAVY, BLUE),
    def(CalcKey::Mod, "Mod", RED, RED),
    def(CalcKey::Or, "Or", RED, RED),
    def(CalcKey::Lsh, "Lsh", RED, RED),
    def(CalcKey::Equals, "=", RED, RED),
    def(CalcKey::Digit(14), "E", NAVY, BLUE),
    def(CalcKey::And, "And", RED, RED),
    def(CalcKey::Xor, "Xor", RED, RED),
    def(CalcKey::Not, "Not", RED, RED),
    def(CalcKey::Int, "Int", RED, RED),
    def(CalcKey::Digit(15), "F", NAVY, BLUE),
];

/// Where things are in one Windows' view (in the client area).
#[derive(Clone, Copy)]
struct Geometry {
    client: (i32, i32),
    display: Rect,
    /// Where the display's text ends: this far left of the display's right edge.
    text_margin: i32,
    /// The text this many rows lower than in the Standard view (95's Scientific display).
    text_dy: i32,
    memory: Rect,
    /// The "M" in the memory box: its pen, from the box's corner.
    memory_text: (i32, i32),
    columns: &'static [i32],
    rows: &'static [i32],
    button: (i32, i32),
    /// The top row: x and width of Back(space), CE, C; its y.
    top: [(i32, i32); 3],
    top_y: i32,
}

const GEOMETRY_95: Geometry = Geometry {
    client: (270, 228),
    display: Rect::new(11, 20, 249, 26),
    text_margin: 8,
    text_dy: 0,
    memory: Rect::new(11, 52, 36, 26),
    memory_text: (13, 7),
    columns: &[11, 63, 103, 143, 183, 223],
    rows: &[86, 120, 154, 188],
    button: (36, 28),
    top: [(104, 49), (157, 49), (210, 49)],
    top_y: 52,
};

const GEOMETRY_98: Geometry = Geometry {
    client: (254, 208),
    display: Rect::new(8, 0, 239, 26),
    text_margin: 2,
    text_dy: 0,
    memory: Rect::new(12, 39, 27, 26),
    memory_text: (10, 6),
    columns: &[8, 54, 93, 132, 171, 210],
    rows: &[73, 106, 138, 171],
    button: (36, 29),
    top: [(54, 63), (120, 62), (185, 62)],
    top_y: 37,
};

const SCIENTIFIC_95: Geometry = Geometry {
    client: (495, 296),
    display: Rect::new(246, 16, 237, 26),
    text_margin: 5,
    text_dy: 1,
    memory: Rect::new(195, 88, 36, 27),
    memory_text: (12, 6),
    columns: &[11, 63, 103, 143, 195, 247, 287, 327, 367, 407, 447],
    rows: &[122, 156, 190, 224, 258],
    button: (36, 28),
    top: [(328, 49), (381, 49), (434, 49)],
    top_y: 88,
};

const SCIENTIFIC_98: Geometry = Geometry {
    client: (474, 265),
    display: Rect::new(216, 0, 249, 26),
    text_margin: 2,
    text_dy: 0,
    memory: Rect::new(189, 62, 27, 26),
    memory_text: (10, 6),
    columns: &[8, 57, 96, 135, 185, 234, 273, 312, 351, 390, 429],
    rows: &[98, 130, 163, 195, 228],
    button: (36, 29),
    top: [(266, 65), (333, 65), (401, 65)],
    top_y: 62,
};

const SCIENTIFIC_2000: Geometry = Geometry { display: Rect::new(8, 0, 458, 26), ..SCIENTIFIC_98 };

/// The Scientific view's option buttons, check boxes and boxes.
struct Controls {
    groups: [Rect; 3],
    /// The parentheses box and its text's pen.
    parens: Rect,
    parens_text: (i32, i32),
    /// The option buttons' row and each one's x: the bases, the angles, the word sizes.
    radio_y: i32,
    bases: [i32; 4],
    angles: [i32; 3],
    words: &'static [i32],
    angle_labels: [&'static str; 3],
    word_labels: &'static [&'static str],
    /// The check boxes' row and x (Inv, Hyp).
    check_y: i32,
    checks: [i32; 2],
}

const CONTROLS_95: Controls = Controls {
    groups: [Rect::new(11, 55, 259, 28), Rect::new(276, 55, 207, 28), Rect::new(11, 88, 127, 27)],
    parens: Rect::new(143, 88, 36, 27),
    parens_text: (11, 6),
    radio_y: 63,
    bases: [21, 82, 144, 202],
    angles: [286, 352, 421],
    words: &[286, 352, 421],
    angle_labels: ["Deg", "Rad", "Grad"],
    word_labels: &["Dword", "Word", "Byte"],
    check_y: 95,
    checks: [21, 81],
};

const CONTROLS_98: Controls = Controls {
    groups: [Rect::new(8, 29, 242, 27), Rect::new(251, 29, 215, 27), Rect::new(8, 61, 123, 27)],
    parens: Rect::new(140, 62, 27, 26),
    parens_text: (6, 6),
    radio_y: 35,
    bases: [13, 75, 136, 195],
    angles: [258, 324, 393],
    words: &[258, 324, 393],
    angle_labels: ["Degrees", "Radians", "Gradients"],
    word_labels: &["Dword", "Word", "Byte"],
    check_y: 68,
    checks: [12, 74],
};

const CONTROLS_2000: Controls = Controls {
    groups: [Rect::new(8, 29, 212, 27), Rect::new(221, 29, 245, 27), Rect::new(8, 61, 126, 27)],
    bases: [13, 63, 112, 162],
    angles: [226, 307, 388],
    words: &[226, 286, 346, 406],
    angle_labels: ["Degrees", "Radians", "Grads"],
    word_labels: &["Qword", "Dword", "Word", "Byte"],
    ..CONTROLS_98
};

const BASE_LABELS: [&str; 4] = ["Hex", "Dec", "Oct", "Bin"];
const BASES: [Radix; 4] = [Radix::Hex, Radix::Dec, Radix::Oct, Radix::Bin];
const ANGLES: [Angle; 3] = [Angle::Degrees, Angle::Radians, Angle::Grads];

fn words(era: Era) -> &'static [Word] {
    if era == Era::Win2000 { &[Word::Qword, Word::Dword, Word::Word, Word::Byte] } else { &[Word::Dword, Word::Word, Word::Byte] }
}

fn controls(era: Era) -> &'static Controls {
    match era {
        Era::Win95 => &CONTROLS_95,
        Era::Win98 => &CONTROLS_98,
        Era::Win2000 => &CONTROLS_2000,
    }
}

/// Where the client area starts in the window.
const CLIENT: (i32, i32) = (window::FRAME, window::CLIENT_Y_WITH_MENU);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    /// The program's picture at the title bar's left end: the window menu.
    Icon,
    Minimize,
    Maximize,
    Close,
    /// A title of the menu bar.
    Menu(usize),
    /// A button of the Calculator (an index of the view's buttons).
    Button(usize),
    /// An option button: 0 to 3 the bases, 4 on the angles or word sizes.
    Radio(usize),
    /// Inv (0) and Hyp (1).
    Check(usize),
    // The rest are things the form asks of its host.
    /// The window menu where the pointer is (a right click on the title bar).
    TitleMenu,
    /// The window menu by the keyboard (Alt+Space).
    KeyMenu,
    /// A command of the menus by its key (Ctrl+C), or Sta.
    Ask(Command),
}

/// The main window.
#[derive(Clone, Debug)]
pub struct CalculatorWindow {
    pub era: Era,
    pub engine: Engine,
    /// The clipboard has text (Edit > Paste is not grayed): the host says so before the menu.
    pub can_paste: bool,
    titles: Vec<Rect>,
    icon: Icon,
    active: bool,
    pressed: Option<Part>,
    /// A key's button, pushed in for this many more ticks, and the key, done when it comes up.
    flash: Option<(usize, u8, CalcKey)>,
    chosen: Option<usize>,
    menu_open: bool,
    by_keyboard: bool,
    hot: Option<usize>,
    keys: bool,
    /// A key with Ctrl was the window's: the character Windows makes of it is not a key.
    swallow: bool,
    /// Windows 98: the bottom-left corner of M+ (Scientific: Dat) was painted over when the
    /// window opened, and drawn again once a button was pushed (by the mouse or a key's flash).
    corner_drawn: bool,
}

impl CalculatorWindow {
    pub fn new(era: Era) -> CalculatorWindow {
        CalculatorWindow {
            era,
            engine: Engine::new(era),
            can_paste: false,
            titles: Vec::new(),
            icon: super::art::icon(era, 16),
            active: true,
            pressed: None,
            flash: None,
            chosen: None,
            menu_open: false,
            by_keyboard: false,
            hot: None,
            keys: false,
            swallow: false,
            corner_drawn: false,
        }
    }

    /// Another look: the number shown stays.
    pub fn set_era(&mut self, era: Era) {
        if self.era != era {
            self.settle();
            self.era = era;
            self.engine = self.engine.with_era(era);
            self.icon = super::art::icon(era, 16);
            self.corner_drawn = false;
        }
    }

    pub fn grouping(&self) -> bool {
        self.engine.grouping
    }

    pub fn set_grouping(&mut self, on: bool) {
        self.engine.grouping = on;
    }

    pub fn scientific(&self) -> bool {
        self.engine.scientific()
    }

    /// The view at the start (from calculator.txt): no number has been typed yet.
    pub fn set_scientific(&mut self, on: bool) {
        self.engine.set_view(on);
        self.corner_drawn = false;
    }

    /// Carries out a command that needs nothing from outside. False: it is the host's.
    pub fn command(&mut self, command: Command) -> bool {
        self.settle();
        match command {
            Command::DigitGrouping if self.era == Era::Win2000 => self.engine.grouping = !self.engine.grouping,
            Command::Standard | Command::Scientific => {
                let scientific = command == Command::Scientific;
                if scientific != self.scientific() && self.engine.set_view(scientific) {
                    self.corner_drawn = false;
                    self.pressed = None;
                }
            }
            Command::Radix(radix) => self.engine.press(CalcKey::Radix(radix)),
            Command::Angle(angle) => self.engine.press(CalcKey::Angle(angle)),
            Command::Word(word) => self.engine.press(CalcKey::Word(word)),
            _ => return false,
        }
        true
    }

    /// Whether a menu entry can be chosen: Paste needs text on the clipboard.
    fn entry_enabled(&self, command: Command) -> bool {
        match command {
            Command::Paste => self.can_paste,
            _ => true,
        }
    }

    /// The entries of menu `menu` as they are now.
    fn entries(&self, menu: usize) -> Vec<Entry> {
        match (menu, self.era) {
            (0, _) => EDIT_MENU.to_vec(),
            (1, Era::Win95) => VIEW_95.to_vec(),
            (1, Era::Win98) => VIEW_98.to_vec(),
            (1, Era::Win2000) => {
                let mut entries = VIEW_98.to_vec();
                entries.push(None);
                if self.scientific() {
                    entries.extend_from_slice(VIEW_2000_BASES);
                    let second = if self.engine.radix() == Radix::Dec { VIEW_2000_ANGLES } else { VIEW_2000_WORDS };
                    entries.extend_from_slice(second);
                }
                entries.push(Some(("D&igit grouping", Command::DigitGrouping)));
                entries
            }
            (2, _) => HELP_MENU.to_vec(),
            _ => Vec::new(),
        }
    }

    /// What entry `entry` of menu `menu` does.
    pub fn command_at(&self, menu: usize, entry: usize) -> Option<Command> {
        self.entries(menu).get(entry).copied().flatten().map(|(_, command)| command)
    }

    /// Menu `menu` as it shows now.
    pub fn menu(&self, menu: usize) -> Menu {
        let scientific = self.scientific();
        let bullet = |on: bool| on.then_some(Mark::Bullet);
        let items = self
            .entries(menu)
            .iter()
            .map(|entry| {
                let Some((label, command)) = *entry else {
                    return MenuEntry::separator();
                };
                let mark = match command {
                    Command::Standard | Command::Scientific if (command == Command::Scientific) == scientific => {
                        Some(if self.era == Era::Win95 { Mark::Check } else { Mark::Bullet })
                    }
                    Command::DigitGrouping if self.engine.grouping => Some(Mark::Check),
                    Command::Radix(radix) => bullet(radix == self.engine.radix()),
                    Command::Angle(angle) => bullet(angle == self.engine.angle()),
                    Command::Word(word) => bullet(word == self.engine.word()),
                    _ => None,
                };
                MenuEntry { mark, disabled: !self.entry_enabled(command), ..MenuEntry::item(label, None) }
            })
            .collect();
        let mut menu = Menu::new(items);
        menu.show_keys = self.era != Era::Win2000 || self.by_keyboard;
        menu.dotted_disabled_keys = self.era == Era::Win2000;
        if self.by_keyboard {
            menu.hover = Some(0);
        }
        menu
    }

    pub fn show_menu(&mut self, menu: Option<usize>) {
        self.chosen = menu;
        self.menu_open = menu.is_some();
        if menu.is_none() {
            self.by_keyboard = false;
            self.keys = false;
        }
    }

    pub fn back_to_bar(&mut self, menu: usize) {
        self.chosen = Some(menu);
        self.menu_open = false;
        self.by_keyboard = true;
        self.keys = true;
    }

    pub fn reached_by_keyboard(&mut self, by_keyboard: bool) {
        self.by_keyboard = by_keyboard;
    }

    pub fn title_rect(&self, menu: usize) -> Option<Rect> {
        self.titles.get(menu).copied()
    }

    fn geometry(&self) -> Geometry {
        match (self.era, self.scientific()) {
            (Era::Win95, false) => GEOMETRY_95,
            (_, false) => GEOMETRY_98,
            (Era::Win95, true) => SCIENTIFIC_95,
            (Era::Win98, true) => SCIENTIFIC_98,
            (Era::Win2000, true) => SCIENTIFIC_2000,
        }
    }

    fn buttons(&self) -> &'static [ButtonDef] {
        if self.scientific() { &SCIENTIFIC } else { &STANDARD }
    }

    /// Button `i`'s rectangle in the window.
    fn button_rect(&self, i: usize) -> Rect {
        let g = self.geometry();
        let r = if i < 3 {
            let (x, w) = g.top[i];
            Rect::new(x, g.top_y, w, g.button.1)
        } else {
            let n = g.rows.len();
            let (col, row) = ((i - 3) / n, (i - 3) % n);
            Rect::new(g.columns[col], g.rows[row], g.button.0, g.button.1)
        };
        Rect::new(CLIENT.0 + r.x, CLIENT.1 + r.y, r.w, r.h)
    }

    fn button_of(&self, key: CalcKey) -> Option<usize> {
        self.buttons().iter().position(|b| b.key == key)
    }

    /// The option buttons now: (part, circle's corner in the window, label, chosen).
    fn radios(&self) -> Vec<(Part, (i32, i32), &'static str, bool)> {
        let c = controls(self.era);
        let at = |x: i32| (CLIENT.0 + x, CLIENT.1 + c.radio_y);
        let mut out: Vec<_> = (0..4)
            .map(|i| (Part::Radio(i), at(c.bases[i]), BASE_LABELS[i], BASES[i] == self.engine.radix()))
            .collect();
        if self.engine.radix() == Radix::Dec {
            out.extend((0..3).map(|i| (Part::Radio(4 + i), at(c.angles[i]), c.angle_labels[i], ANGLES[i] == self.engine.angle())));
        } else {
            let words = words(self.era);
            out.extend((0..words.len()).map(|i| (Part::Radio(4 + i), at(c.words[i]), c.word_labels[i], words[i] == self.engine.word())));
        }
        out
    }

    /// The check boxes: (part, box's corner in the window, label, ticked).
    fn checks(&self) -> [(Part, (i32, i32), &'static str, bool); 2] {
        let c = controls(self.era);
        let at = |x: i32| (CLIENT.0 + x, CLIENT.1 + c.check_y);
        [
            (Part::Check(0), at(c.checks[0]), "Inv", self.engine.inv_ticked()),
            (Part::Check(1), at(c.checks[1]), "Hyp", self.engine.hyp_ticked()),
        ]
    }

    /// A key typed: its button flashes, and the Calculator does it as the button comes up
    /// again (the Reference PCs still showed the number before while the button was down).
    pub fn press_key(&mut self, key: CalcKey) -> Vec<Effect<Part>> {
        self.settle();
        match self.button_of(key) {
            Some(i) => {
                self.flash = Some((i, FLASH_TICKS, key));
                self.corner_drawn = true;
            }
            None => self.engine.press(key),
        }
        if key == CalcKey::Sta { vec![Effect::Activate(Part::Ask(Command::Statistics))] } else { Vec::new() }
    }

    /// The caption of the button a key has pushed in, while it is (98's and 2000's captions;
    /// 95's pictures say Back and PI).
    pub fn flashing(&self) -> Option<&'static str> {
        self.flash.map(|(i, _, _)| self.buttons()[i].label)
    }

    /// The captions of the view's buttons (98's and 2000's words).
    pub fn captions(&self) -> Vec<&'static str> {
        self.buttons().iter().map(|b| b.label).collect()
    }

    /// The captions of the option buttons and check boxes now (the bases, the angles or word
    /// sizes, Inv and Hyp); empty in the Standard view.
    pub fn option_captions(&self) -> Vec<&'static str> {
        if !self.scientific() {
            return Vec::new();
        }
        let c = controls(self.era);
        let mut all: Vec<&'static str> = BASE_LABELS.to_vec();
        all.extend(c.angle_labels);
        all.extend(c.word_labels.iter().copied());
        all.extend(["Inv", "Hyp"]);
        all
    }

    /// A key whose button is still flashing is done now (another key, a click, the menus).
    pub fn settle(&mut self) {
        if let Some((_, _, key)) = self.flash.take() {
            self.engine.press(key);
        }
    }

    /// The host's 50 ms tick: a flashing button comes up again. True if that changed anything.
    pub fn tick(&mut self) -> bool {
        match self.flash {
            Some((_, 0 | 1, _)) => {
                self.settle();
                true
            }
            Some((i, n, key)) => {
                self.flash = Some((i, n - 1, key));
                false
            }
            None => false,
        }
    }

    /// Pasted text, typed key by key (`engine::paste_keys`).
    pub fn paste_text(&mut self, text: &str) {
        self.settle();
        for key in engine::paste_keys_in(text, self.scientific()) {
            self.engine.press(key);
        }
    }

    /// What Edit > Copy copies.
    pub fn copy_text(&mut self) -> String {
        self.settle();
        self.engine.copy_text()
    }

    fn leave_bar(&mut self) {
        if !self.menu_open {
            self.chosen = None;
            self.by_keyboard = false;
            self.keys = false;
        }
    }

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

    fn size_now(&self) -> (i32, i32) {
        window::outer_size(self.geometry().client)
    }

    fn title_buttons(&self) -> [ButtonLook; 3] {
        let width = self.size_now().0;
        let look = |button, part: Part| ButtonLook {
            button,
            rect: window::title_button_rect(width, button),
            pressed: self.pressed == Some(part),
            enabled: part != Part::Maximize,
        };
        [
            look(TitleButton::Minimize, Part::Minimize),
            look(TitleButton::Maximize, Part::Maximize),
            look(TitleButton::Close, Part::Close),
        ]
    }

    /// Whether button `b` does nothing now: 98 and 2000 gray its caption (95 grays nothing).
    fn grayed(&self, b: &ButtonDef) -> bool {
        if self.era == Era::Win95 || !self.scientific() {
            return false;
        }
        let dec = self.engine.radix() == Radix::Dec;
        match b.key {
            CalcKey::Digit(d) => u32::from(d) >= self.engine.radix().base(),
            CalcKey::Ave | CalcKey::Sum | CalcKey::Dev | CalcKey::Dat => !self.engine.stats_open(),
            CalcKey::FixedExp | CalcKey::Dms | CalcKey::Exp | CalcKey::Sin | CalcKey::Cos | CalcKey::Tan | CalcKey::Pi => !dec,
            _ => false,
        }
    }

    fn paint_client(&self, fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts) {
        let c = &theme.colors;
        let g = self.geometry();
        let scientific = self.scientific();
        let at = |r: Rect| Rect::new(CLIENT.0 + r.x, CLIENT.1 + r.y, r.w, r.h);
        // The client's own font: MS Sans Serif in every look (bold captions on 95).
        let regular: &Font = if self.era == Era::Win2000 { crate::text::ms_sans_serif() } else { fonts.regular };
        if self.era == Era::Win95 {
            let w = g.client.0 - 2;
            fb.hline(CLIENT.0 + 1, CLIENT.1 + 5, w, c.shadow);
            fb.hline(CLIENT.0 + 1, CLIENT.1 + 6, w, c.highlight);
        }
        // The display: sunken, white, the text right-aligned and cut off at its inside edges.
        let display = at(g.display);
        sunken(fb, theme, display);
        let inside = display.inset(2, 2, 2, 2);
        fb.fill(inside, rgb(0xFFFFFF));
        // Windows 2000 ends a number (not an error's text) with a space: 3 pixels further left.
        let mut text = self.engine.display();
        if self.era == Era::Win2000 && self.engine.error().is_none() && !text.starts_with('(') {
            text.push(' ');
        }
        let mut strip = Framebuffer::new(inside.w, inside.h, rgb(0xFFFFFF));
        let x = display.right() - g.text_margin - regular.measure(&text) - inside.x;
        regular.draw(&mut strip, x, (display.h - regular.height) / 2 - 2 + g.text_dy, &text, c.text);
        fb.blit(&strip, inside.x, inside.y);
        // The memory box, with "M" while the memory holds a number.
        let memory = at(g.memory);
        sunken(fb, theme, memory);
        if self.engine.memory_shown() {
            regular.draw(fb, memory.x + g.memory_text.0, memory.y + g.memory_text.1, "M", c.text);
        }
        if scientific {
            self.paint_controls(fb, theme, regular);
        }
        for (i, b) in self.buttons().iter().enumerate() {
            let r = self.button_rect(i);
            let down = self.pressed == Some(Part::Button(i)) || self.flash.is_some_and(|(f, ..)| f == i);
            push_button(fb, theme, self.era, r, down);
            if self.era == Era::Win95 {
                // Its own buttons moved the caption 1 right and 2 down while pushed in.
                let (dx, dy, rows) =
                    if scientific { super::captions95::CAPTIONS_SCIENTIFIC[i] } else { super::captions95::CAPTIONS[i] };
                let (x, y) = if down { (r.x + dx + 1, r.y + dy + 2) } else { (r.x + dx, r.y + dy) };
                dialog::art(fb, rows, x, y, |ch| (ch == b'#').then_some(b.color_95));
            } else {
                let w = regular.measure(b.label);
                let color = if self.grayed(b) { GRAY } else { b.color };
                regular.draw(fb, r.x + (r.w - w) / 2, r.y + (r.h - regular.height) / 2, b.label, color);
            }
            // Windows 98 paints over the bottom-left corner of M+ (Scientific: Dat).
            let corner = if scientific { CalcKey::Dat } else { CalcKey::MemoryAdd };
            if self.era == Era::Win98 && b.key == corner && !self.corner_drawn {
                fb.fill(Rect::new(r.x, r.bottom() - 5, 4, 5), c.face);
            }
        }
    }

    /// The Scientific view's groups, option buttons, check boxes and parentheses box.
    fn paint_controls(&self, fb: &mut Framebuffer, theme: &Theme, font: &Font) {
        let c = &theme.colors;
        let k = controls(self.era);
        let at = |r: Rect| Rect::new(CLIENT.0 + r.x, CLIENT.1 + r.y, r.w, r.h);
        for group in k.groups {
            let r = at(group);
            fb.frame(r, c.shadow, c.highlight);
            fb.frame(r.inset(1, 1, 1, 1), c.highlight, c.shadow);
        }
        // While an error shows, 95 and 98 switch the option buttons and check boxes off.
        let enabled = self.engine.error().is_none() || self.era == Era::Win2000;
        let label = |fb: &mut Framebuffer, x: i32, y: i32, text: &str| {
            if enabled {
                font.draw(fb, x, y, text, c.text);
            } else {
                font.draw(fb, x + 1, y + 1, text, c.etch);
                font.draw(fb, x, y, text, c.shadow);
            }
        };
        for (part, (x, y), text, on) in self.radios() {
            let held = self.pressed == Some(part) || !enabled;
            let rows = if on { &OPTION_ON } else { &OPTION_OFF };
            dialog::art(fb, rows, x, y, |b| match b {
                b'g' => Some(c.shadow),
                b'#' => Some(c.dark_shadow),
                b'W' => Some(c.highlight),
                b'l' => Some(c.light),
                _ => None,
            });
            if held {
                // the inside (white before an edge further right) goes gray; the outer edge stays
                for (dy, row) in rows.iter().enumerate() {
                    let row = row.as_bytes();
                    for (dx, &b) in row.iter().enumerate() {
                        if dy <= 9 && b == b'W' && row[dx + 1..].iter().any(|&r| r == b'l' || r == b'#') {
                            fb.set(x + dx as i32, y + dy as i32, c.face);
                        }
                    }
                }
            }
            if on {
                // the dot
                let dot = if enabled { c.text } else { c.shadow };
                fb.fill(Rect::new(x + 4, y + 5, 4, 2), dot);
                fb.fill(Rect::new(x + 5, y + 4, 2, 4), dot);
            }
            label(fb, x + 17, y, text);
        }
        for (part, (x, y), text, on) in self.checks() {
            crate::properties::check_box(fb, c, x, y, on, self.pressed == Some(part), enabled);
            label(fb, x + 18, y, text);
        }
        let parens = at(k.parens);
        sunken(fb, theme, parens);
        let open = self.engine.open_parens();
        if open > 0 {
            // centered, as a static text control centers it
            let text = format!("(={open}");
            let x = parens.x + (parens.w - font.measure(&text)) / 2;
            font.draw(fb, x, parens.y + k.parens_text.1, &text, c.text);
        }
    }
}

/// A 2-px sunken field (the display, the memory box).
fn sunken(fb: &mut Framebuffer, theme: &Theme, r: Rect) {
    let c = &theme.colors;
    fb.frame(r, c.shadow, c.highlight);
    fb.frame(r.inset(1, 1, 1, 1), c.dark_shadow, c.light);
}

/// A Calculator button, up or pushed in, as each Windows drew it.
fn push_button(fb: &mut Framebuffer, theme: &Theme, era: Era, r: Rect, down: bool) {
    let c = &theme.colors;
    fb.fill(r, c.face);
    let inner = r.inset(1, 1, 1, 1);
    match (era, down) {
        (Era::Win95, false) => {
            fb.frame(r, c.highlight, c.dark_shadow);
            fb.frame(inner, c.light, c.shadow);
        }
        (Era::Win95, true) => {
            fb.frame(r, c.dark_shadow, c.highlight);
            fb.frame(inner, c.shadow, c.light);
        }
        (_, false) => {
            fb.frame(r, c.light, c.dark_shadow);
            fb.frame(inner, c.highlight, c.shadow);
        }
        (_, true) => {
            fb.frame(r, c.shadow, c.highlight);
            fb.frame(inner, c.dark_shadow, c.light);
        }
    }
}

impl kit::Form for CalculatorWindow {
    type Part = Part;

    fn title(&self) -> &str {
        TITLE
    }
    fn size(&self) -> (i32, i32) {
        self.size_now()
    }
    fn layout(&mut self, fonts: &Fonts) {
        self.titles = window::menu_bar_layout(fonts, &MENUS);
    }
    fn paint(&self, fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts) {
        let buttons = self.title_buttons();
        let frame = Frame { era: self.era, title: TITLE, icon: Some(&self.icon), active: self.active, buttons: &buttons };
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
        self.paint_client(fb, theme, fonts);
    }

    fn hit(&self, x: i32, y: i32) -> Option<Part> {
        if let Some(i) = self.titles.iter().position(|r| r.contains(x, y)) {
            return Some(Part::Menu(i));
        }
        if let Some(i) = (0..self.buttons().len()).find(|&i| self.button_rect(i).contains(x, y)) {
            return Some(Part::Button(i));
        }
        if self.scientific()
            && let Some(part) = self.radios().iter().map(|r| r.0).chain(self.checks().iter().map(|c| c.0)).find(|&p| {
                self.rect(p).is_some_and(|r| r.contains(x, y))
            })
        {
            return Some(part);
        }
        [Part::Minimize, Part::Maximize, Part::Close, Part::Icon]
            .into_iter()
            .find(|&p| self.rect(p).is_some_and(|r| r.contains(x, y)))
    }
    fn rect(&self, part: Part) -> Option<Rect> {
        let width = self.size_now().0;
        let regular = crate::text::ms_sans_serif();
        Some(match part {
            Part::Icon => Rect::new(window::FRAME + 2, window::FRAME + 1, 16, 16),
            Part::Minimize => window::title_button_rect(width, TitleButton::Minimize),
            Part::Maximize => window::title_button_rect(width, TitleButton::Maximize),
            Part::Close => window::title_button_rect(width, TitleButton::Close),
            Part::Menu(i) => return self.title_rect(i),
            Part::Button(i) => self.button_rect(i),
            Part::Radio(_) => {
                let (_, (x, y), label, _) = self.radios().into_iter().find(|r| r.0 == part)?;
                Rect::new(x, y - 1, 17 + regular.measure(label) + 2, 14)
            }
            Part::Check(_) => {
                let (_, (x, y), label, _) = self.checks().into_iter().find(|r| r.0 == part)?;
                Rect::new(x, y, 18 + regular.measure(label) + 2, 13)
            }
            Part::TitleMenu | Part::KeyMenu | Part::Ask(_) => return None,
        })
    }
    fn enabled(&self, part: Part) -> bool {
        part != Part::Maximize
    }
    fn focus(&self) -> Part {
        Part::Button(0)
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
        Part::Button(0)
    }
    fn escape(&self) -> Part {
        Part::Button(0)
    }

    fn key(&mut self, key: Key, mods: Mods) -> Option<Vec<Effect<Part>>> {
        if let Some(i) = self.chosen.filter(|_| !self.menu_open) {
            let n = MENUS.len();
            match key {
                Key::Left => self.chosen = Some((i + n - 1) % n),
                Key::Right => self.chosen = Some((i + 1) % n),
                Key::Down | Key::Up | Key::Enter => return Some(self.open_by_keyboard(i)),
                Key::Modifier => {}
                Key::Other(vk) if (u16::from(b'A')..=u16::from(b'Z')).contains(&vk) => {}
                _ => self.leave_bar(),
            }
            return Some(Vec::new());
        }
        if key != Key::Modifier {
            self.keys = false;
        }
        let scientific = self.scientific();
        if mods.ctrl {
            let calc = match key {
                Key::Other(0x4C) => Some(CalcKey::MemoryClear), // Ctrl+L
                Key::Other(0x52) => Some(CalcKey::MemoryRecall), // Ctrl+R
                Key::Other(0x4D) => Some(CalcKey::MemoryStore), // Ctrl+M
                Key::Other(0x50) => Some(CalcKey::MemoryAdd), // Ctrl+P
                Key::Other(0x53) if scientific => Some(CalcKey::Sta), // Ctrl+S
                Key::Other(0x41) if scientific => Some(CalcKey::Ave), // Ctrl+A
                Key::Other(0x54) if scientific => Some(CalcKey::Sum), // Ctrl+T
                Key::Other(0x44) if scientific => Some(CalcKey::Dev), // Ctrl+D
                _ => None,
            };
            if let Some(calc) = calc {
                self.swallow = true;
                return Some(self.press_key(calc));
            }
            match key {
                Key::Other(0x43) => return Some(vec![Effect::Activate(Part::Ask(Command::Copy))]),
                Key::Other(0x56) => return Some(vec![Effect::Paste]),
                _ => {}
            }
        }
        let dec = self.engine.radix() == Radix::Dec;
        let calc = match key {
            Key::Enter => CalcKey::Equals,
            Key::Escape => CalcKey::Clear,
            Key::Delete => CalcKey::ClearEntry,
            Key::Other(VK_BACK) => CalcKey::Back,
            Key::Other(VK_F9) => CalcKey::Negate,
            Key::Other(VK_INSERT) if scientific => CalcKey::Dat,
            Key::Other(VK_F5) if scientific => CalcKey::Radix(Radix::Hex),
            Key::Other(VK_F6) if scientific => CalcKey::Radix(Radix::Dec),
            Key::Other(VK_F7) if scientific => CalcKey::Radix(Radix::Oct),
            Key::Other(VK_F8) if scientific => CalcKey::Radix(Radix::Bin),
            Key::Other(VK_F2) if scientific && dec => CalcKey::Angle(Angle::Degrees),
            Key::Other(VK_F3) if scientific && dec => CalcKey::Angle(Angle::Radians),
            Key::F4 if scientific && dec => CalcKey::Angle(Angle::Grads),
            Key::Other(VK_F2) if scientific => CalcKey::Word(Word::Dword),
            Key::Other(VK_F3) if scientific => CalcKey::Word(Word::Word),
            Key::F4 if scientific => CalcKey::Word(Word::Byte),
            Key::Other(VK_F12) if scientific => CalcKey::Word(Word::Qword),
            _ => return Some(Vec::new()),
        };
        Some(self.press_key(calc))
    }
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
        // Control characters (Enter, Esc, Backspace, Ctrl+letter) came as keys already.
        if std::mem::take(&mut self.swallow) || ch < ' ' {
            return Vec::new();
        }
        let key = if self.scientific() { engine::key_for_char_scientific(ch) } else { engine::key_for_char(ch) };
        match key {
            Some(key) => self.press_key(key),
            None => Vec::new(),
        }
    }
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
    fn press(&mut self, part: Option<Part>, _x: i32, _y: i32) -> Press<Part> {
        self.leave_bar();
        match part {
            Some(p @ (Part::Icon | Part::Menu(_))) => Press::Act(p),
            Some(p @ (Part::Button(_) | Part::Radio(_) | Part::Check(_))) => {
                self.corner_drawn |= matches!(p, Part::Button(_));
                Press::Hold(p)
            }
            Some(p @ (Part::Minimize | Part::Close)) => Press::Hold(p),
            Some(_) => Press::Done,
            None => Press::Nothing,
        }
    }
    fn release(&mut self, part: Part) -> Vec<Effect<Part>> {
        match part {
            Part::Button(i) => {
                self.settle();
                let key = self.buttons()[i].key;
                self.engine.press(key);
                if key == CalcKey::Sta {
                    return vec![Effect::Activate(Part::Ask(Command::Statistics))];
                }
                Vec::new()
            }
            Part::Radio(i) => {
                self.settle();
                let key = match i {
                    0..=3 => CalcKey::Radix(BASES[i]),
                    _ if self.engine.radix() == Radix::Dec => CalcKey::Angle(ANGLES[i - 4]),
                    _ => CalcKey::Word(words(self.era)[i - 4]),
                };
                self.engine.press(key);
                Vec::new()
            }
            Part::Check(i) => {
                self.settle();
                self.engine.press(if i == 0 { CalcKey::Inv } else { CalcKey::Hyp });
                Vec::new()
            }
            _ => vec![Effect::Activate(part)],
        }
    }
    fn button(&mut self, button: Button, down: bool, x: i32, y: i32) -> Vec<Effect<Part>> {
        if down {
            self.leave_bar();
            return Vec::new();
        }
        let width = self.size_now().0;
        if button == Button::Right
            && kit::TITLE_ROWS.contains(&y)
            && matches!(self.hit(x, y), None | Some(Part::Icon))
            && (window::FRAME..width - window::FRAME).contains(&x)
        {
            return vec![Effect::Activate(Part::TitleMenu)];
        }
        Vec::new()
    }
    fn set_active(&mut self, active: bool) {
        self.active = active;
        if !active {
            self.leave_bar();
            self.hot = None;
        }
    }
    fn hover(&mut self, x: i32, y: i32) -> bool {
        let hot = self.titles.iter().position(|r| r.contains(x, y));
        std::mem::replace(&mut self.hot, hot) != hot && self.era == Era::Win2000
    }
    fn pointer_left(&mut self) -> bool {
        self.hot.take().is_some() && self.era == Era::Win2000
    }
    fn paste(&mut self, text: &str) {
        self.paste_text(text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::{Controller, Form};

    fn window(era: Era) -> (CalculatorWindow, Controller<Part>) {
        let regular = Font::parse(include_str!("../../tests/fixtures/ms-sans-serif-11px.font")).unwrap();
        let bold = Font::parse(include_str!("../../tests/fixtures/ms-sans-serif-11px-bold.font")).unwrap();
        let mut w = CalculatorWindow::new(era);
        w.layout(&Fonts { regular: &regular, bold: &bold });
        (w, Controller::default())
    }

    fn click(w: &mut CalculatorWindow, c: &mut Controller<Part>, key: CalcKey) {
        let r = w.button_rect(w.button_of(key).unwrap());
        c.mouse_down(w, r.x + 5, r.y + 5);
        c.mouse_up(w, r.x + 5, r.y + 5);
    }

    #[test]
    fn sizes_as_measured() {
        assert_eq!(window(Era::Win95).0.size(), (276, 272));
        assert_eq!(window(Era::Win98).0.size(), (260, 252));
        assert_eq!(window(Era::Win2000).0.size(), (260, 252));
        // Windows 98's M+: client (8, 171) in a window whose client starts at (3, 41).
        let (mut w, _) = window(Era::Win98);
        assert_eq!(w.button_rect(6), Rect::new(11, 212, 36, 29));
        w.command(Command::Scientific);
        assert_eq!(w.size(), (480, 309));
        // Dat: client (8, 228)
        assert_eq!(w.button_rect(7), Rect::new(11, 269, 36, 29));
        let (mut w95, _) = window(Era::Win95);
        w95.command(Command::Scientific);
        assert_eq!(w95.size(), (501, 340));
    }

    #[test]
    fn buttons_by_mouse_and_keys_by_keyboard() {
        let (mut w, mut c) = window(Era::Win98);
        click(&mut w, &mut c, CalcKey::Digit(2));
        click(&mut w, &mut c, CalcKey::Add);
        // held, then let go off the button: nothing typed
        let r = w.button_rect(w.button_of(CalcKey::Digit(3)).unwrap());
        assert_eq!(c.mouse_down(&mut w, r.x + 3, r.y + 3), vec![Effect::Capture]);
        assert_eq!(w.pressed, Some(Part::Button(17)));
        assert!(c.mouse_move(&mut w, r.x - 30, r.y));
        assert_eq!(w.pressed, None, "dragged off it comes up");
        c.mouse_up(&mut w, r.x - 30, r.y);
        assert_eq!(w.engine.display(), "2.");
        w.typed('3');
        assert_eq!(w.flash, Some((17, FLASH_TICKS, CalcKey::Digit(3))), "a key flashes its button");
        assert_eq!(w.engine.display(), "2.", "the digit comes as the button comes up");
        c.key(&mut w, Key::Enter, Mods::default());
        assert_eq!(w.engine.display(), "3.", "another key: the one before is done");
        assert!(!w.tick() && !w.tick() && !w.tick());
        assert!(w.tick(), "up again after the flash");
        assert_eq!((w.flash, w.engine.display().as_str()), (None, "5."));
        // Ctrl+M stores; the character Windows makes of it (a carriage return) is no "="
        let ctrl = Mods { ctrl: true, shift: false };
        c.key(&mut w, Key::Other(0x4D), ctrl);
        w.typed('\r');
        w.settle();
        assert!(w.engine.memory_shown());
        c.key(&mut w, Key::Escape, Mods::default());
        w.settle();
        assert_eq!(w.engine.display(), "0.");
        c.key(&mut w, Key::Other(0x52), ctrl);
        w.settle();
        assert_eq!(w.engine.display(), "5.");
        assert_eq!(c.key(&mut w, Key::Other(0x43), ctrl), vec![Effect::Activate(Part::Ask(Command::Copy))]);
        assert_eq!(c.key(&mut w, Key::Other(0x56), ctrl), vec![Effect::Paste]);
        w.paste("*2=");
        assert_eq!(w.engine.display(), "10.");
        c.key(&mut w, Key::Other(VK_F9), Mods::default());
        w.settle();
        assert_eq!(w.engine.display(), "-10.");
    }

    #[test]
    fn the_scientific_view_by_keys_and_clicks() {
        let (mut w, mut c) = window(Era::Win2000);
        assert!(w.command(Command::Scientific));
        assert!(w.scientific());
        // 2 + 3 * 4 = with precedence, typed
        for ch in "2+3*4=".chars() {
            w.typed(ch);
        }
        w.settle();
        assert_eq!(w.engine.display(), "14.");
        // F5: Hex, by the key and the option button
        c.key(&mut w, Key::Other(VK_F5), Mods::default());
        w.settle();
        assert_eq!(w.engine.display(), "E");
        let r = w.rect(Part::Radio(1)).unwrap();
        c.mouse_down(&mut w, r.x + 3, r.y + 5);
        c.mouse_up(&mut w, r.x + 3, r.y + 5);
        assert_eq!(w.engine.radix(), Radix::Dec);
        // Inv by its check box, then sin: arc sine of 1
        let r = w.rect(Part::Check(0)).unwrap();
        c.mouse_down(&mut w, r.x + 3, r.y + 3);
        c.mouse_up(&mut w, r.x + 3, r.y + 3);
        assert!(w.engine.inv_ticked());
        w.typed('1');
        w.typed('s');
        w.settle();
        assert_eq!(w.engine.display(), "90.");
        assert!(!w.engine.inv_ticked());
        // Sta asks the host for the Statistics Box; Ave was grayed before
        assert!(w.grayed(&SCIENTIFIC[4]));
        let ctrl = Mods { ctrl: true, shift: false };
        assert_eq!(c.key(&mut w, Key::Other(0x53), ctrl), vec![Effect::Activate(Part::Ask(Command::Statistics))]);
        w.settle();
        assert!(!w.grayed(&SCIENTIFIC[4]));
        // the View menu shows the bases and angles; Standard goes back
        assert_eq!(w.command_at(1, 3), Some(Command::Radix(Radix::Hex)));
        assert_eq!(w.command_at(1, 8), Some(Command::Angle(Angle::Degrees)));
        assert_eq!(w.menu(1).entries[1].mark, Some(Mark::Bullet));
        assert!(w.command(Command::Standard));
        assert!(!w.scientific());
        assert!(!w.engine.stats_open(), "the Statistics Box goes with the view");
    }

    #[test]
    fn menus_per_look() {
        let (mut w, mut c) = window(Era::Win2000);
        let (w95, _) = window(Era::Win95);
        let (w98, _) = window(Era::Win98);
        assert_eq!(w95.command_at(1, 0), Some(Command::Scientific));
        assert_eq!(w98.command_at(1, 0), Some(Command::Standard));
        assert_eq!(w.command_at(1, 3), Some(Command::DigitGrouping));
        assert_eq!(w98.command_at(1, 3), None);
        assert_eq!(w98.command_at(2, 2), Some(Command::About));
        let view = w.menu(1);
        assert_eq!(view.entries[0].mark, Some(Mark::Bullet));
        assert!(!view.entries[1].disabled, "Scientific is there");
        assert!(!w.menu(2).entries[0].disabled, "Help Topics opens the book");
        assert_eq!(w.command_at(2, 0), Some(Command::HelpTopics));
        assert!(w.menu(0).entries[1].disabled, "nothing to paste");
        w.can_paste = true;
        assert!(!w.menu(0).entries[1].disabled);
        assert!(w.command(Command::DigitGrouping));
        assert_eq!(w.menu(1).entries[3].mark, Some(Mark::Check));
        w.typed('1');
        w.typed('2');
        w.typed('3');
        w.typed('4');
        w.settle();
        assert_eq!(w.engine.display(), "1,234.");
        // Alt+V opens View, Alt+E Edit
        assert_eq!(c.access(&mut w, 'v'), vec![Effect::Activate(Part::Menu(1))]);
        w.show_menu(None);
        assert_eq!(c.access(&mut w, 'e'), vec![Effect::Activate(Part::Menu(0))]);
        assert_eq!(w95.menu(1).entries[1].mark, Some(Mark::Check));
    }
}
