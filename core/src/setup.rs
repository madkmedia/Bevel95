//! Bevel95 Setup: the small window a copy of Bevel95 shows on a PC where it has not been set up,
//! or when it is not the installed copy (the owner's design, 2026-11-09: one file is the
//! installer, the user decides). Bevel95's own window, drawn like the dialogs of the look it is
//! set to (Shut Down's option buttons, Taskbar Properties' check boxes and group boxes, push
//! buttons), on the control kit. No classic Windows had it, so its layout is PROVISIONAL.
//!
//! Two pages. The first asks what to do with this copy. Installing goes on to a second page
//! with the things Bevel95 takes over, each with the owner's default (2026-11-17): the look,
//! the classic desktop, what the Windows key opens and Start with Windows. The window is drawn
//! in the look chosen there, as soon as it is chosen.

use crate::config::{self, WindowsKey};
use crate::dialog::{self, OPTION_OFF, OPTION_ON, TitleButtons};
use crate::fb::{Framebuffer, Rect};
use crate::kit::{self, Effect, Key, Mods, Press};
use crate::properties;
use crate::text::{Font, Fonts, split_access_key};
use crate::theme::{Era, Theme};

/// Why the window shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetupKind {
    /// Nothing set up yet on this PC.
    FirstRun,
    /// Bevel95 is installed, and this is another copy (`newer`: by its build stamp, or its
    /// file's date for copies built before the stamp). `can_run`: it can run as a bar where it
    /// is (a copy as built, its programs beside it); the one file cannot, since its programs
    /// are only inside it, so it does not offer "Just run this copy" (an outside review,
    /// 2026-11-17).
    OtherCopy { newer: bool, can_run: bool },
    /// The installed copy itself, started with `--setup` (to remove it).
    Installed,
}

/// What OK does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetupChoice {
    /// Copy to %LOCALAPPDATA%\Bevel95, shortcuts on the desktop, start it from there.
    Install,
    /// Keep the settings and the log next to the program, wherever it is.
    Portable,
    /// Put this copy in the installed one's place.
    Update,
    /// Run this copy as it is.
    RunThisCopy,
    /// Take Bevel95 off this PC: shortcuts, Start with Windows, its folder.
    Remove,
}

/// Which page shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    /// What to do with this copy.
    Choose,
    /// Installing: the things Bevel95 takes over.
    Questions,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    TitleClose,
    /// One of the first page's choices.
    Option(usize),
    /// One of the looks (`LOOKS`).
    Look(usize),
    Desktop,
    /// What the Windows key opens (`KEYS`).
    WindowsKey(usize),
    StartWithWindows,
    Back,
    /// OK, Next > or Install.
    Ok,
    Cancel,
}

/// The answers of the second page, with the owner's defaults.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Answers {
    pub era: Era,
    /// "Show the classic desktop" (`desktop` in settings.txt).
    pub desktop: bool,
    pub windows_key: WindowsKey,
    pub start_with_windows: bool,
}

impl Default for Answers {
    fn default() -> Answers {
        let defaults = config::Config::default();
        Answers { era: defaults.era, desktop: defaults.desktop, windows_key: defaults.windows_key, start_with_windows: false }
    }
}

impl Answers {
    /// `text` (a settings file) with the answers in it; everything else as it was.
    pub fn settings(&self, text: &str) -> String {
        let text = config::with_setting(text, "era", self.era.setting());
        let text = config::with_setting(&text, "desktop", if self.desktop { "yes" } else { "no" });
        config::with_setting(&text, "windows_key", self.windows_key.setting())
    }
}

/// The name from `about` (`product_name!`), as everywhere in this window.
pub const TITLE: &str = concat!(crate::product_name!(), " Setup");
/// What a copy that is not the installed one says, and does nothing else, when any of its
/// programs is missing (a download that stopped early: the one file without its end is just the
/// bar). PROVISIONAL wording.
pub const INCOMPLETE: &str = concat!(
    "This copy of ",
    crate::product_name!(),
    " is incomplete: its other programs are missing. A download that stopped early looks like this.\n\n\
     Download it again. Nothing was changed."
);
/// What the one file says, and does nothing else, when it is the very build that is installed and
/// the installed bar is already running (the owner, 2026-11-18: the download started again showed
/// nothing). PROVISIONAL wording.
pub fn already_running(version: &str) -> String {
    format!("{} {version} is already installed and running.", crate::product_name!())
}

/// What Portable says, and does nothing else, in a folder that already holds another program's
/// settings.txt (the safety review of 2026-11-18, A3). PROVISIONAL wording.
pub const FOREIGN_SETTINGS: &str = concat!(
    "This folder already holds another program's settings.txt, so ",
    crate::product_name!(),
    " cannot keep its own settings here.\n\nPut ",
    crate::product_name!(),
    " into a folder of its own and start it there, or choose Install. Nothing was changed."
);
/// What Portable says, and does nothing else, in a folder whose settings.txt another program holds
/// open, so that it cannot be read to tell whose it is (the third safety review of 2026-11-18,
/// A1). PROVISIONAL wording.
pub const SETTINGS_IN_USE: &str = concat!(
    "This folder holds a settings.txt that another program has open just now, so ",
    crate::product_name!(),
    " cannot tell whether it is its own.\n\nClose that program and start ",
    crate::product_name!(),
    " again, put it into a folder of its own, or choose Install. Nothing was changed."
);
/// What Portable says, and does nothing else, in a folder whose settings.txt cannot be read for a
/// lasting reason (access denied, a damaged file), so that it cannot tell whose it is (the fifth
/// safety review of 2026-11-18, B2: it asked the person to close a program that did not exist).
/// PROVISIONAL wording.
pub const SETTINGS_UNREADABLE: &str = concat!(
    "This folder holds a settings.txt that cannot be read, so ",
    crate::product_name!(),
    " cannot tell whether it is its own and cannot keep its settings here.\n\nPut ",
    crate::product_name!(),
    " into a folder of its own and start it there, or choose Install. Nothing was changed."
);
/// What every program of a copy says, and does nothing else, when it is started by the person
/// while the settings.txt beside the copy is held open by another program, so that it cannot be
/// read to tell whether the copy is a portable one (the fourth safety review of 2026-11-18, B1
/// and C2): it then ends before Setup compares anything, writing nothing. `SETTINGS_IN_USE`
/// without its "or choose Install" (no Setup window is shown here). PROVISIONAL wording.
pub const SETTINGS_IN_USE_AT_START: &str = concat!(
    "This folder holds a settings.txt that another program has open just now, so ",
    crate::product_name!(),
    " cannot tell whether it is its own.\n\nClose that program and start ",
    crate::product_name!(),
    " again. Nothing was changed."
);
const START_LABEL: &str = concat!("&Start ", crate::product_name!(), " when Windows starts");
const DESKTOP_LABEL: &str = "Show the classic &desktop";
const QUESTIONS: &str = concat!(
    "Choose how ",
    crate::product_name!(),
    " looks and what it does. Bevel95 Settings can change all of this later."
);
const LOOK_GROUP: &str = "Look";
const KEY_GROUP: &str = "The Windows key opens";
/// The looks, in the Theme menu's order.
pub const LOOKS: [(Era, &str); 3] = [(Era::Win95, "Windows 9&5"), (Era::Win98, "Windows 9&8"), (Era::Win2000, "Windows &2000")];
/// What the Windows key can open.
pub const KEYS: [(WindowsKey, &str); 2] =
    [(WindowsKey::Find, "&Quick Search, to find programs by typing"), (WindowsKey::Start, "The Start &menu")];
const WIDTH: i32 = 344;
const TEXT_X: i32 = 14;
const TEXT_Y: i32 = 32;
const OPTION_X: i32 = 18;
const ROW: i32 = 20;
const CHECK_X: i32 = 20;
const GROUP_X: i32 = 10;
/// The step between the looks' option buttons, side by side in their group.
const LOOK_STEP: i32 = 100;
const BUTTON: (i32, i32) = (75, 23);

/// The window's state.
#[derive(Clone, Debug)]
pub struct Setup {
    pub kind: SetupKind,
    pub page: Page,
    pub choice: usize,
    /// The second page's answers; `answers.era` is also the look the window is drawn in.
    pub answers: Answers,
    pub focus: Part,
    pub pressed: Option<Part>,
    /// Access keys underlined and focus shown (95 and 98 always; 2000 after a key).
    pub keys: bool,
}

impl Setup {
    /// In the Windows 95 look (a first run: nothing is set yet).
    pub fn new(kind: SetupKind) -> Setup {
        Setup::in_look(kind, Answers::default().era)
    }

    /// Drawn in `era` (the installed copy's look, for an update or Remove).
    pub fn in_look(kind: SetupKind, era: Era) -> Setup {
        Setup {
            kind,
            page: Page::Choose,
            choice: 0,
            answers: Answers { era, ..Answers::default() },
            focus: Part::Option(0),
            pressed: None,
            keys: era.always_shows_keys(),
        }
    }

    /// The look the window is drawn in.
    pub fn era(&self) -> Era {
        self.answers.era
    }

    /// The choices offered, in order, with their labels.
    pub fn choices(&self) -> Vec<(SetupChoice, &'static str)> {
        match self.kind {
            SetupKind::FirstRun => vec![
                (SetupChoice::Install, "&Install it on this computer, with shortcuts on the desktop"),
                (SetupChoice::Portable, "Keep it &portable: settings and log next to this file"),
            ],
            SetupKind::OtherCopy { newer, can_run } => {
                let update = if newer {
                    concat!("&Update the installed ", crate::product_name!(), " with this newer copy")
                } else {
                    concat!("&Replace the installed ", crate::product_name!(), " with this older copy")
                };
                let mut choices = vec![(SetupChoice::Update, update)];
                if can_run {
                    choices.push((SetupChoice::RunThisCopy, "&Just run this copy"));
                }
                choices.push((SetupChoice::Remove, concat!("Re&move ", crate::product_name!(), " from this computer")));
                choices
            }
            SetupKind::Installed => vec![(SetupChoice::Remove, concat!("Re&move ", crate::product_name!(), " from this computer"))],
        }
    }

    pub fn chosen(&self) -> SetupChoice {
        let choices = self.choices();
        choices[self.choice.min(choices.len() - 1)].0
    }

    fn text(&self) -> &'static str {
        match (self.page, self.kind) {
            (Page::Questions, _) => QUESTIONS,
            (_, SetupKind::FirstRun) => concat!(crate::product_name!(), " is not set up on this computer yet. How do you want to use it?"),
            (_, SetupKind::OtherCopy { .. }) => {
                concat!(crate::product_name!(), " is already installed on this computer. What do you want to do with this copy?")
            }
            (_, SetupKind::Installed) => concat!(crate::product_name!(), " is installed on this computer."),
        }
    }

    /// Whether OK goes on to the questions (installing) rather than finishing.
    pub fn leads_on(&self) -> bool {
        self.page == Page::Choose && self.chosen() == SetupChoice::Install
    }

    /// The label of the button that goes forward.
    pub fn ok_label(&self) -> &'static str {
        match self.page {
            Page::Questions => "&Install",
            Page::Choose if self.leads_on() => "&Next >",
            Page::Choose => "OK",
        }
    }

    /// OK, Next > or Install: true when Setup is answered (the window closes), false when it
    /// went on to the questions.
    pub fn ok(&mut self) -> bool {
        if !self.leads_on() {
            return true;
        }
        self.page = Page::Questions;
        self.focus = Part::Look(self.look_index());
        false
    }

    /// < Back: the first page again, as it was left.
    pub fn back(&mut self) {
        self.page = Page::Choose;
        self.focus = Part::Option(self.choice);
    }

    /// The chosen look's place in `LOOKS`.
    pub fn look_index(&self) -> usize {
        LOOKS.iter().position(|&(era, _)| era == self.answers.era).unwrap_or(0)
    }

    fn key_index(&self) -> usize {
        KEYS.iter().position(|&(key, _)| key == self.answers.windows_key).unwrap_or(0)
    }

    pub fn enabled(&self, _part: Part) -> bool {
        true
    }
}

/// Where everything goes, at 1×.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetupLayout {
    pub height: i32,
    pub lines: Vec<String>,
    pub parts: Vec<(Part, Rect)>,
    /// The second page's two group boxes.
    pub groups: Vec<(&'static str, Rect)>,
}

impl SetupLayout {
    pub fn rect(&self, part: Part) -> Option<Rect> {
        self.parts.iter().find(|(p, _)| *p == part).map(|&(_, r)| r)
    }
}

/// An option button's place: the circle and its label (`label` with its access key).
fn option_rect(font: &Font, x: i32, y: i32, label: &str) -> Rect {
    Rect::new(x, y, 17 + font.measure_label(label) + 2, 16)
}

/// A check box's place: the box and its label.
fn check_rect(font: &Font, x: i32, y: i32, label: &str) -> Rect {
    Rect::new(x, y, 18 + font.measure_label(label) + 2, 15)
}

pub fn layout(s: &Setup, font: &Font) -> SetupLayout {
    let lines = dialog::wrap(font, s.text(), WIDTH - 2 * TEXT_X);
    let mut parts = vec![(Part::TitleClose, dialog::title_button_rects(WIDTH).1)];
    let mut groups = Vec::new();
    let mut y = TEXT_Y + lines.len() as i32 * font.height + 12;
    match s.page {
        Page::Choose => {
            for (i, (_, label)) in s.choices().iter().enumerate() {
                parts.push((Part::Option(i), option_rect(font, OPTION_X, y, label)));
                y += ROW;
            }
        }
        Page::Questions => {
            y += 2;
            let group_w = WIDTH - 2 * GROUP_X;
            groups.push((LOOK_GROUP, Rect::new(GROUP_X, y, group_w, 40)));
            for (i, (_, label)) in LOOKS.iter().enumerate() {
                parts.push((Part::Look(i), option_rect(font, GROUP_X + 10 + i as i32 * LOOK_STEP, y + 16, label)));
            }
            y += 40 + 8;
            parts.push((Part::Desktop, check_rect(font, CHECK_X, y, DESKTOP_LABEL)));
            y += ROW + 10;
            groups.push((KEY_GROUP, Rect::new(GROUP_X, y, group_w, 16 + KEYS.len() as i32 * ROW + 2)));
            for (i, (_, label)) in KEYS.iter().enumerate() {
                parts.push((Part::WindowsKey(i), option_rect(font, GROUP_X + 10, y + 16 + i as i32 * ROW, label)));
            }
            y += 16 + KEYS.len() as i32 * ROW + 2 + 8;
            parts.push((Part::StartWithWindows, check_rect(font, CHECK_X, y, START_LABEL)));
            y += ROW;
        }
    }
    let buttons_y = y + 12;
    let ok = Rect::new(WIDTH - 10 - BUTTON.0 - 6 - BUTTON.0, buttons_y, BUTTON.0, BUTTON.1);
    if s.page == Page::Questions {
        // A wizard's < Back and Next > (here Install) touch, as in Windows' own wizards.
        parts.push((Part::Back, Rect::new(ok.x - BUTTON.0, buttons_y, BUTTON.0, BUTTON.1)));
    }
    parts.push((Part::Ok, ok));
    parts.push((Part::Cancel, Rect::new(WIDTH - 10 - BUTTON.0, buttons_y, BUTTON.0, BUTTON.1)));
    SetupLayout { height: buttons_y + BUTTON.1 + 12, lines, parts, groups }
}

/// An option button with its label, and its focus dots.
#[allow(clippy::too_many_arguments)]
fn paint_option(fb: &mut Framebuffer, theme: &Theme, font: &Font, r: Rect, label: &str, on: bool, focused: bool, keys: bool) {
    let c = &theme.colors;
    let rows = if on { &OPTION_ON } else { &OPTION_OFF };
    dialog::art(fb, rows, r.x, r.y + 2, |b| match b {
        b'g' => Some(c.shadow),
        b'#' => Some(c.dark_shadow),
        b'W' => Some(c.highlight),
        b'l' => Some(c.light),
        _ => None,
    });
    let (tx, ty) = (r.x + 17, r.y + 2);
    font.draw_label(fb, (tx, ty), label, i32::MAX, c.text, keys);
    if focused && keys {
        let w = font.measure_label(label);
        dialog::focus_dots(fb, Rect::new(tx - 1, ty - 1, w + 2, font.height + 3), (r.x, r.y), c.text);
    }
}

/// A check box with its label, and its focus dots.
#[allow(clippy::too_many_arguments)]
fn paint_check(fb: &mut Framebuffer, theme: &Theme, font: &Font, r: Rect, label: &str, on: bool, pressed: bool, focused: bool, keys: bool) {
    let c = &theme.colors;
    properties::check_box(fb, c, r.x, r.y, on, pressed, true);
    let tx = r.x + 18;
    font.draw_label(fb, (tx, r.y), label, i32::MAX, c.text, keys);
    if focused && keys {
        let w = font.measure_label(label);
        dialog::focus_dots(fb, Rect::new(tx - 1, r.y - 1, w + 2, font.height + 3), (r.x, r.y - 1), c.text);
    }
}

pub fn paint(fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts, s: &Setup, l: &SetupLayout) {
    let c = &theme.colors;
    let font = fonts.regular;
    let keys = s.keys;
    let held = |part: Part| s.pressed == Some(part);
    let buttons = TitleButtons { help: None, close: l.rect(Part::TitleClose).map(|r| (r, held(Part::TitleClose))) };
    dialog::window_frame(fb, theme, fonts, s.era(), TITLE, &buttons);
    for (i, line) in l.lines.iter().enumerate() {
        font.draw(fb, TEXT_X, TEXT_Y + i as i32 * font.height, line, c.text);
    }
    for &(label, r) in &l.groups {
        properties::group_box(fb, c, font, r, label, keys);
    }
    for &(part, r) in &l.parts {
        let focused = s.focus == part;
        match part {
            Part::Option(i) => paint_option(fb, theme, font, r, s.choices()[i].1, s.choice == i, focused, keys),
            Part::Look(i) => paint_option(fb, theme, font, r, LOOKS[i].1, s.look_index() == i, focused, keys),
            Part::WindowsKey(i) => paint_option(fb, theme, font, r, KEYS[i].1, s.key_index() == i, focused, keys),
            Part::Desktop => paint_check(fb, theme, font, r, DESKTOP_LABEL, s.answers.desktop, held(part), focused, keys),
            Part::StartWithWindows => {
                paint_check(fb, theme, font, r, START_LABEL, s.answers.start_with_windows, held(part), focused, keys);
            }
            Part::Back | Part::Ok | Part::Cancel | Part::TitleClose => {}
        }
    }
    // The button that goes forward is the default one: framed unless another button has the
    // focus.
    let framed = match s.focus {
        f @ (Part::Cancel | Part::Back) => f,
        _ => Part::Ok,
    };
    for (part, label) in [(Part::Back, "< &Back"), (Part::Ok, s.ok_label()), (Part::Cancel, "Cancel")] {
        if let Some(r) = l.rect(part) {
            dialog::push_button(fb, theme, font, r, label, framed == part, held(part), s.focus == part && keys, true, keys);
        }
    }
}

/// The setup window as a window of the control kit.
#[derive(Clone, Debug)]
pub struct SetupWindow {
    pub setup: Setup,
    pub layout: SetupLayout,
}

impl SetupWindow {
    pub fn new(setup: Setup, font: &Font) -> SetupWindow {
        let layout = layout(&setup, font);
        SetupWindow { setup, layout }
    }

    fn choose(&mut self, i: usize) {
        self.setup.choice = i.min(self.setup.choices().len() - 1);
        self.setup.focus = Part::Option(self.setup.choice);
    }

    /// Picks a look: the host then draws the window in it (`Effect::Activate(Part::Look)`).
    fn choose_look(&mut self, i: usize) -> Vec<Effect<Part>> {
        let i = i.min(LOOKS.len() - 1);
        self.setup.focus = Part::Look(i);
        if self.setup.look_index() == i {
            return Vec::new();
        }
        self.setup.answers.era = LOOKS[i].0;
        vec![Effect::Activate(Part::Look(i))]
    }

    fn choose_key(&mut self, i: usize) {
        let i = i.min(KEYS.len() - 1);
        self.setup.answers.windows_key = KEYS[i].0;
        self.setup.focus = Part::WindowsKey(i);
    }

    fn toggle(&mut self, part: Part) {
        match part {
            Part::Desktop => self.setup.answers.desktop = !self.setup.answers.desktop,
            Part::StartWithWindows => self.setup.answers.start_with_windows = !self.setup.answers.start_with_windows,
            _ => {}
        }
    }
}

/// Whether `label`'s access key is `ch`.
fn has_key(label: &str, ch: char) -> bool {
    let (text, at) = split_access_key(label);
    at.and_then(|k| text.chars().nth(k)).is_some_and(|c| c.eq_ignore_ascii_case(&ch))
}

impl kit::Form for SetupWindow {
    type Part = Part;

    fn title(&self) -> &str {
        TITLE
    }
    fn size(&self) -> (i32, i32) {
        (WIDTH, self.layout.height)
    }
    fn layout(&mut self, fonts: &Fonts) {
        self.layout = layout(&self.setup, fonts.regular);
    }
    fn paint(&self, fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts) {
        paint(fb, theme, fonts, &self.setup, &self.layout);
    }
    fn hit(&self, x: i32, y: i32) -> Option<Part> {
        self.layout.parts.iter().find(|(_, r)| r.contains(x, y)).map(|&(p, _)| p)
    }
    fn rect(&self, part: Part) -> Option<Rect> {
        self.layout.rect(part)
    }
    fn enabled(&self, part: Part) -> bool {
        self.setup.enabled(part)
    }
    fn focus(&self) -> Part {
        self.setup.focus
    }
    fn set_focus(&mut self, part: Part) {
        self.setup.focus = part;
    }
    fn tab_stops(&self) -> Vec<Part> {
        match self.setup.page {
            Page::Choose => vec![Part::Option(self.setup.choice), Part::Ok, Part::Cancel],
            Page::Questions => vec![
                Part::Look(self.setup.look_index()),
                Part::Desktop,
                Part::WindowsKey(self.setup.key_index()),
                Part::StartWithWindows,
                Part::Back,
                Part::Ok,
                Part::Cancel,
            ],
        }
    }
    fn set_pressed(&mut self, part: Option<Part>) {
        self.setup.pressed = part;
    }
    fn show_keys(&mut self) {
        self.setup.keys = true;
    }
    fn enter(&self) -> Part {
        match self.setup.focus {
            f @ (Part::Cancel | Part::Back) => f,
            _ => Part::Ok,
        }
    }
    fn escape(&self) -> Part {
        Part::Cancel
    }
    fn key(&mut self, key: Key, _mods: Mods) -> Option<Vec<Effect<Part>>> {
        let (up, down) = (matches!(key, Key::Up | Key::Left), matches!(key, Key::Down | Key::Right));
        if !up && !down {
            return None;
        }
        let step = |i: usize| if down { i + 1 } else { i.saturating_sub(1) };
        match self.setup.focus {
            Part::Option(i) => self.choose(step(i)),
            Part::Look(i) => return Some(self.choose_look(step(i))),
            Part::WindowsKey(i) => self.choose_key(step(i)),
            _ => return None,
        }
        Some(Vec::new())
    }
    /// No text fields: a letter or digit is an access key, with or without Alt.
    fn typed(&mut self, ch: char) -> Vec<Effect<Part>> {
        if ch.is_alphanumeric() { self.access(ch) } else { Vec::new() }
    }
    fn access(&mut self, ch: char) -> Vec<Effect<Part>> {
        match self.setup.page {
            Page::Choose => {
                if let Some(i) = self.setup.choices().iter().position(|(_, label)| has_key(label, ch)) {
                    self.choose(i);
                } else if has_key(self.setup.ok_label(), ch) {
                    return vec![Effect::Activate(Part::Ok)];
                }
            }
            Page::Questions => {
                if let Some(i) = LOOKS.iter().position(|(_, label)| has_key(label, ch)) {
                    return self.choose_look(i);
                } else if let Some(i) = KEYS.iter().position(|(_, label)| has_key(label, ch)) {
                    self.choose_key(i);
                } else if has_key(DESKTOP_LABEL, ch) {
                    self.setup.focus = Part::Desktop;
                    self.toggle(Part::Desktop);
                } else if has_key(START_LABEL, ch) {
                    self.setup.focus = Part::StartWithWindows;
                    self.toggle(Part::StartWithWindows);
                } else if has_key("< &Back", ch) {
                    return vec![Effect::Activate(Part::Back)];
                } else if has_key(self.setup.ok_label(), ch) {
                    return vec![Effect::Activate(Part::Ok)];
                }
            }
        }
        Vec::new()
    }
    fn space(&mut self) -> Vec<Effect<Part>> {
        match self.setup.focus {
            f @ (Part::Desktop | Part::StartWithWindows) => self.toggle(f),
            f @ (Part::Ok | Part::Cancel | Part::Back) => return vec![Effect::Activate(f)],
            _ => {}
        }
        Vec::new()
    }
    fn press(&mut self, part: Option<Part>, _x: i32, _y: i32) -> Press<Part> {
        match part {
            Some(Part::Option(i)) => {
                self.choose(i);
                Press::Done
            }
            Some(Part::Look(i)) => match self.choose_look(i).first() {
                Some(&Effect::Activate(p)) => Press::Act(p),
                _ => Press::Done,
            },
            Some(Part::WindowsKey(i)) => {
                self.choose_key(i);
                Press::Done
            }
            Some(p @ (Part::Desktop | Part::StartWithWindows | Part::Back | Part::Ok | Part::Cancel)) => {
                self.setup.focus = p;
                Press::Hold(p)
            }
            Some(Part::TitleClose) => Press::Hold(Part::TitleClose),
            None => Press::Nothing,
        }
    }
    fn release(&mut self, part: Part) -> Vec<Effect<Part>> {
        match part {
            Part::Desktop | Part::StartWithWindows => {
                self.toggle(part);
                Vec::new()
            }
            _ => vec![Effect::Activate(part)],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::{Controller, Form};

    fn font() -> Font {
        Font::parse(include_str!("../tests/fixtures/ms-sans-serif-11px.font")).unwrap()
    }

    /// What the host does with OK and < Back (the Windows side's `SetupHost::activate`).
    fn carry_out(w: &mut SetupWindow, effects: Vec<Effect<Part>>) -> Option<bool> {
        let mut done = None;
        for effect in effects {
            match effect {
                Effect::Activate(Part::Ok) => done = Some(w.setup.ok()),
                Effect::Activate(Part::Back) => w.setup.back(),
                _ => {}
            }
        }
        w.layout = layout(&w.setup, &font());
        done
    }

    #[test]
    fn first_run_offers_install_or_portable() {
        let mut w = SetupWindow::new(Setup::new(SetupKind::FirstRun), &font());
        let mut c = Controller::default();
        assert_eq!(w.setup.chosen(), SetupChoice::Install);
        assert_eq!(w.setup.ok_label(), "&Next >", "installing goes on to the questions");
        assert!(w.layout.rect(Part::StartWithWindows).is_none() && w.layout.rect(Part::Back).is_none());
        c.key(&mut w, Key::Tab, Mods::default());
        assert_eq!(w.setup.focus, Part::Ok);
        // portable: OK finishes at once
        w.access('p');
        assert_eq!(w.setup.chosen(), SetupChoice::Portable);
        assert_eq!(w.setup.ok_label(), "OK");
        assert!(!w.setup.leads_on());
        assert_eq!(c.key(&mut w, Key::Enter, Mods::default()), vec![Effect::Activate(Part::Ok)]);
        assert_eq!(c.key(&mut w, Key::Escape, Mods::default()), vec![Effect::Activate(Part::Cancel)]);
        assert!(w.setup.ok());
    }

    #[test]
    fn installing_asks_the_questions_with_the_owners_defaults() {
        let mut w = SetupWindow::new(Setup::new(SetupKind::FirstRun), &font());
        let mut c = Controller::default();
        let enter = c.key(&mut w, Key::Enter, Mods::default());
        assert_eq!(carry_out(&mut w, enter), Some(false), "Next > shows the second page");
        assert_eq!(w.setup.page, Page::Questions);
        assert_eq!(w.setup.ok_label(), "&Install");
        assert_eq!(
            w.setup.answers,
            Answers { era: Era::Win95, desktop: true, windows_key: WindowsKey::Find, start_with_windows: false }
        );
        assert_eq!(w.setup.focus, Part::Look(0));
        // Tab goes through the groups and boxes in order.
        let mut order = vec![w.setup.focus];
        for _ in 0..6 {
            c.key(&mut w, Key::Tab, Mods::default());
            order.push(w.setup.focus);
        }
        assert_eq!(
            order,
            [Part::Look(0), Part::Desktop, Part::WindowsKey(0), Part::StartWithWindows, Part::Back, Part::Ok, Part::Cancel]
        );
        // A look by its key: the window is told to draw itself in it.
        assert_eq!(c.access(&mut w, '8'), vec![Effect::Activate(Part::Look(1))]);
        assert_eq!(w.setup.era(), Era::Win98);
        assert_eq!(c.access(&mut w, '8'), vec![], "the same look again changes nothing");
        // The arrows move within the focused group.
        assert_eq!(c.key(&mut w, Key::Right, Mods::default()), vec![Effect::Activate(Part::Look(2))]);
        assert_eq!(w.setup.era(), Era::Win2000);
        c.access(&mut w, 'm');
        assert_eq!(w.setup.answers.windows_key, WindowsKey::Start);
        c.key(&mut w, Key::Up, Mods::default());
        assert_eq!(w.setup.answers.windows_key, WindowsKey::Find);
        c.access(&mut w, 'd');
        c.access(&mut w, 's');
        assert!(!w.setup.answers.desktop && w.setup.answers.start_with_windows);
        // A click on a look acts at once.
        let r = w.layout.rect(Part::Look(0)).unwrap();
        assert_eq!(c.mouse_down(&mut w, r.x + 2, r.y + 4), vec![Effect::Activate(Part::Look(0))]);
        // < Back keeps the answers; Next > comes back to them.
        let back = c.access(&mut w, 'b');
        carry_out(&mut w, back);
        assert_eq!((w.setup.page, w.setup.focus), (Page::Choose, Part::Option(0)));
        assert_eq!(w.setup.era(), Era::Win95, "the first page is drawn in the chosen look");
        let next = c.access(&mut w, 'n');
        assert_eq!(carry_out(&mut w, next), Some(false));
        assert!(!w.setup.answers.desktop);
        let install = c.access(&mut w, 'i');
        assert_eq!(carry_out(&mut w, install), Some(true), "Install finishes");
    }

    #[test]
    fn the_answers_go_into_the_settings() {
        let answers = Answers { era: Era::Win2000, desktop: false, windows_key: WindowsKey::Start, start_with_windows: true };
        let text = answers.settings(config::DEFAULT_FILE);
        let (settings, warnings) = config::parse(&text);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!((settings.era, settings.desktop, settings.windows_key), (Era::Win2000, false, WindowsKey::Start));
        // Nothing else changed, comments included.
        let defaults = Answers::default().settings(config::DEFAULT_FILE);
        assert_eq!(defaults, config::DEFAULT_FILE);
        assert_eq!(config::parse(&text).0.scale, config::Config::default().scale);
    }

    #[test]
    fn another_copy_can_update_run_or_remove() {
        let mut w = SetupWindow::new(Setup::in_look(SetupKind::OtherCopy { newer: true, can_run: true }, Era::Win2000), &font());
        let mut c = Controller::default();
        assert_eq!(w.setup.chosen(), SetupChoice::Update);
        assert_eq!(w.setup.era(), Era::Win2000, "drawn in the installed copy's look");
        assert!(!w.setup.keys, "2000 hides the underlines until a key is used");
        assert_eq!(w.setup.ok_label(), "OK", "an update asks nothing");
        c.key(&mut w, Key::Down, Mods::default());
        c.key(&mut w, Key::Down, Mods::default());
        c.key(&mut w, Key::Down, Mods::default());
        assert!(w.setup.keys);
        assert_eq!(w.setup.chosen(), SetupChoice::Remove, "the arrows stop at the last choice");
        assert!(w.layout.rect(Part::StartWithWindows).is_none());
        assert!(w.setup.ok());
        let older = Setup::new(SetupKind::OtherCopy { newer: false, can_run: true });
        assert!(older.choices()[0].1.contains("older"));
    }

    /// The one file over an installed copy: Update (or the older copy's wording) and Remove, no
    /// "Just run this copy" (its programs are only inside the file, so a bar run from it would
    /// have none).
    #[test]
    fn the_one_file_offers_update_or_remove_only() {
        for newer in [true, false] {
            let mut w = SetupWindow::new(Setup::in_look(SetupKind::OtherCopy { newer, can_run: false }, Era::Win95), &font());
            let chosen: Vec<SetupChoice> = w.setup.choices().iter().map(|&(c, _)| c).collect();
            assert_eq!(chosen, [SetupChoice::Update, SetupChoice::Remove], "newer: {newer}");
            assert!(w.layout.rect(Part::Option(1)).is_some() && w.layout.rect(Part::Option(2)).is_none());
            // The arrows and the access keys reach only these two.
            let mut c = Controller::default();
            c.key(&mut w, Key::Down, Mods::default());
            c.key(&mut w, Key::Down, Mods::default());
            assert_eq!(w.setup.chosen(), SetupChoice::Remove);
            c.access(&mut w, 'j');
            assert_eq!(w.setup.chosen(), SetupChoice::Remove, "no 'Just run this copy' to choose");
            c.access(&mut w, 'u');
            assert_eq!(w.setup.chosen(), if newer { SetupChoice::Update } else { SetupChoice::Remove });
        }
        // The page is shorter by one row than a copy as built's.
        let one = SetupWindow::new(Setup::new(SetupKind::OtherCopy { newer: true, can_run: false }), &font());
        let built = SetupWindow::new(Setup::new(SetupKind::OtherCopy { newer: true, can_run: true }), &font());
        assert_eq!(built.layout.height - one.layout.height, ROW);
        // The message of an incomplete copy names Bevel95 from the one name.
        assert!(INCOMPLETE.starts_with(concat!("This copy of ", crate::product_name!(), " is incomplete")));
        assert!(INCOMPLETE.ends_with("Nothing was changed."));
    }
}
