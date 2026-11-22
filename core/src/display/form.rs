//! Display Properties, the window the classic desktop's Properties opens: each look's own
//! Background page, drawn from its Windows' dialog template and checked against the Reference PCs'
//! captures (`vm/<pc>-truecolor-display-*`; survey.md, sections 1 and 2).
//!
//! - **95**: two list boxes side by side (Pattern, Wallpaper), Edit Pattern..., Browse..., Display:
//!   Tile and Center.
//! - **98** and **2000**: one list view of wallpapers with small pictures, Browse..., Pattern...
//!   (the Pattern window, `pattern`), and a Display (2000: Picture Display) list: Center, Tile,
//!   Stretch.
//!
//! The 404x448 window with OK, Cancel and Apply (grayed until something changes). Only the
//! Background tab is on it for now (PROVISIONAL: the owner decides whether the other tabs show
//! before they work); the tab strip takes more pages without changing the window. Grayed for now,
//! each a later step: Browse... (the person's own pictures) and Edit Pattern... (the Pattern Editor).

use crate::dialog::{self, OPTION_OFF, OPTION_ON, TitleButtons};
use crate::display::{self, Choice, Show};
use crate::fb::{Framebuffer, Rect};
use crate::kit::{self, Effect, Key, Mods, Press};
use crate::properties::{group_box, tab};
use crate::scroll::ScrollBar;
use crate::text::{Font, Fonts};
use crate::theme::{Era, Theme, rgb};

/// The window's size, the same in all three.
pub const SIZE: (i32, i32) = (404, 448);
const TAB_X: i32 = 11;
const TAB_Y: i32 = 31;
const TAB_H: i32 = 18;
const TAB_PAD: i32 = 6;
/// The page's raised frame under the tabs.
const PAGE: Rect = Rect::new(9, 49, 386, 360);
const BUTTONS: [(Part, i32); 3] = [(Part::Ok, 158), (Part::Cancel, 239), (Part::Apply, 320)];
const BUTTONS_Y: i32 = 415;
/// Where the page's dialog template starts in the window.
const ORIGIN: (i32, i32) = (13, 51);
/// A list box's rows (95's lists, the Pattern window's) and a list view's (98's and 2000's).
pub(crate) const BOX_ROW: i32 = 13;
const VIEW_ROW: i32 = 17;
/// A drop-down list's rows when open.
const DROP_ROW: i32 = 13;

/// Dialog units (MS Sans Serif and Tahoma 8 at Small Fonts: 1.5 pixels across, 1.625 down,
/// rounded as Windows' MulDiv) to pixels, from `origin`.
pub(crate) fn du(origin: (i32, i32), x: i32, y: i32, w: i32, h: i32) -> Rect {
    let across = |v: i32| (v * 6 + 2) / 4;
    let down = |v: i32| (v * 13 + 4) / 8;
    Rect::new(origin.0 + across(x), origin.1 + down(y), across(w), down(h))
}

/// The window's pages, in tab order (only Background for now).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Background,
}

impl Page {
    pub fn title(self) -> &'static str {
        match self {
            Page::Background => "Background",
        }
    }
}

/// Which list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    Wallpaper,
    Pattern,
}

/// A scroll bar's parts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bar {
    Up,
    Down,
    PageUp,
    PageDown,
    Thumb,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    TitleHelp,
    TitleClose,
    Tab(usize),
    /// A list (one stop for the keyboard), a row of it ("(None)" is row 0), its scroll bar.
    List(Which),
    Row(Which, usize),
    Scroll(Which, Bar),
    /// 95's option buttons.
    Tile,
    Center,
    /// 98's Display and 2000's Picture Display list, and a row of it while open.
    ShowList,
    ShowRow(usize),
    Browse,
    EditPattern,
    /// 98's and 2000's Pattern... (opens the Pattern window).
    PatternButton,
    Ok,
    Cancel,
    Apply,
}

impl Part {
    pub fn is_push_button(self) -> bool {
        matches!(self, Part::Browse | Part::EditPattern | Part::PatternButton | Part::Ok | Part::Cancel | Part::Apply)
    }
}

/// A list's place and rows: its outer rectangle (the sunken frame included), its row height, how
/// many rows, which is selected and which is the first shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct List {
    pub rect: Rect,
    pub row_h: i32,
    pub rows: usize,
    pub sel: usize,
    pub top: usize,
}

impl List {
    pub fn inside(&self) -> Rect {
        self.rect.inset(2, 2, 2, 2)
    }
    /// The rows that show whole.
    pub fn page(&self) -> usize {
        (self.inside().h / self.row_h).max(1) as usize
    }
    pub fn scroll(&self) -> Option<ScrollBar> {
        let i = self.inside();
        (self.rows > self.page()).then_some(ScrollBar { bar: Rect::new(i.right() - 16, i.y, 16, i.h), total: self.rows, page: self.page() })
    }
    /// Where the rows are drawn (the inside without the scroll bar).
    pub fn rows_rect(&self) -> Rect {
        let i = self.inside();
        if self.scroll().is_some() { Rect::new(i.x, i.y, i.w - 16, i.h) } else { i }
    }
    pub fn row_rect(&self, i: usize) -> Rect {
        let r = self.rows_rect();
        Rect::new(r.x, r.y + (i as i32 - self.top as i32) * self.row_h, r.w, self.row_h)
    }
    fn max_top(&self) -> usize {
        self.rows.saturating_sub(self.page())
    }
    pub fn set_top(&mut self, top: usize) {
        self.top = top.min(self.max_top());
    }
    /// Scrolls so that the selected row shows whole.
    pub fn show_sel(&mut self) {
        if self.sel < self.top {
            self.top = self.sel;
        } else if self.sel >= self.top + self.page() {
            self.top = self.sel + 1 - self.page();
        }
        self.set_top(self.top);
    }
    /// Moves the selection by `delta` rows (clamped); true if it moved.
    pub fn step(&mut self, delta: i32) -> bool {
        let to = (self.sel as i32 + delta).clamp(0, self.rows as i32 - 1) as usize;
        let moved = to != self.sel;
        self.sel = to;
        self.show_sel();
        moved
    }
    /// The parts of its scroll bar.
    pub fn scroll_parts(&self, which: Which) -> Vec<(Part, Rect)> {
        let Some(bar) = self.scroll() else { return Vec::new() };
        let r = bar.rects(self.top);
        vec![
            (Part::Scroll(which, Bar::Up), r.up),
            (Part::Scroll(which, Bar::Down), r.down),
            (Part::Scroll(which, Bar::Thumb), r.thumb),
            (Part::Scroll(which, Bar::PageUp), r.page_up),
            (Part::Scroll(which, Bar::PageDown), r.page_down),
        ]
    }
    /// The rows that show, each with its rectangle cut to the list.
    pub fn shown_rows(&self) -> Vec<(usize, Rect)> {
        let rows = self.rows_rect();
        (self.top..self.rows)
            .map(|i| (i, self.row_rect(i)))
            .take_while(|(_, r)| r.y < rows.bottom())
            .map(|(i, r)| (i, Rect::new(r.x, r.y, r.w, r.h.min(rows.bottom() - r.y))))
            .collect()
    }
    /// The next row after the selected one whose name starts with `ch` (Windows' lists).
    pub fn find(&self, names: &[String], ch: char) -> Option<usize> {
        let n = names.len();
        (1..=n).map(|k| (self.sel + k) % n).find(|&i| names[i].chars().next().is_some_and(|c| c.eq_ignore_ascii_case(&ch)))
    }
}

/// A list box as Windows drew one (95's lists, the Pattern window's): sunken, white, rows of
/// text 2 in, the selected row in the selection color across the list, the focus rectangle's dots
/// on it while the list has the keyboard.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_box(fb: &mut Framebuffer, theme: &Theme, font: &Font, list: &List, names: &[String], focused: bool, held: Option<Bar>, highlight: bool) {
    let c = &theme.colors;
    dialog::well(fb, list.rect, c);
    let rows = list.rows_rect();
    let mut area = Framebuffer::new(rows.w, rows.h, rgb(0xFFFFFF));
    for (i, name) in names.iter().enumerate() {
        let r = list.row_rect(i);
        let at = Rect::new(r.x - rows.x, r.y - rows.y, r.w, r.h);
        if at.bottom() <= 0 || at.y >= rows.h {
            continue;
        }
        let ink = if i == list.sel && highlight {
            area.fill(at, c.menu_highlight);
            c.menu_highlight_text
        } else {
            c.text
        };
        font.draw(&mut area, at.x + 2, at.y, name, ink);
        if i == list.sel && focused {
            focus_dots(&mut area, theme, at, (0, 0));
        }
    }
    fb.blit(&area, rows.x, rows.y);
    if let Some(bar) = list.scroll() {
        bar.paint_from(fb, c, list.top, held == Some(Bar::Up), held == Some(Bar::Down), (list.rect.x, list.rect.y));
    }
}

/// The focus rectangle on a selected row (Windows' DrawFocusRect over the selection color): every
/// other pixel of its edge inverted, those an odd number of pixels from `origin`, the list's inside's
/// corner (Windows lines the dots up with the list's window: the Reference PCs' lists).
pub(crate) fn focus_dots(fb: &mut Framebuffer, theme: &Theme, r: Rect, origin: (i32, i32)) {
    let reduced = theme.depth.reduced();
    for y in r.y..r.bottom() {
        for x in r.x..r.right() {
            let edge = x == r.x || y == r.y || x == r.right() - 1 || y == r.bottom() - 1;
            if !edge || x < 0 || y < 0 || x >= fb.width || y >= fb.height || (x - origin.0 + y - origin.1).rem_euclid(2) == 0 {
                continue;
            }
            let p = fb.get(x, y);
            fb.set(x, y, if reduced { crate::colors16::focus_dot(theme, p, true) } else { p ^ 0x00FF_FFFF });
        }
    }
}

/// The focus rectangle on a drop-down list's chosen entry (98's and 2000's Display lists on the
/// Reference PCs): from its corner every other pixel of its edge inverted, the others black.
fn combo_focus(fb: &mut Framebuffer, theme: &Theme, r: Rect) {
    for y in r.y..r.bottom() {
        for x in r.x..r.right() {
            let edge = x == r.x || y == r.y || x == r.right() - 1 || y == r.bottom() - 1;
            if !edge || x < 0 || y < 0 || x >= fb.width || y >= fb.height {
                continue;
            }
            let p = fb.get(x, y);
            let on = (x - r.x + y - r.y) % 2 == 0;
            let dot = if theme.depth.reduced() { crate::colors16::focus_dot(theme, p, true) } else { p ^ 0x00FF_FFFF };
            fb.set(x, y, if on { dot } else { 0xFF00_0000 });
        }
    }
}

/// The font of the pages' dialog templates ("MS Shell Dlg"): MS Sans Serif in all three, also in
/// Windows 2000, whose own windows (Taskbar Properties) use Tahoma (the Reference PC's capture).
fn dialog_font<'a>(era: Era, fonts: &'a Fonts) -> &'a Font {
    if era == Era::Win2000 { crate::text::ms_sans_serif() } else { fonts.regular }
}

/// The small pictures of 98's and 2000's wallpaper list (16x16 RGB, on the list's white).
struct Small {
    plain: &'static [u8],
    selected: &'static [u8],
}

const NONE_98: Small = Small {
    plain: crate::art::bytes("display/none", crate::art::Look::Win98),
    selected: crate::art::bytes("display/none-selected", crate::art::Look::Win98),
};
const BITMAP_98: Small = Small {
    plain: crate::art::bytes("display/bitmap", crate::art::Look::Win98),
    selected: crate::art::bytes("display/bitmap-selected", crate::art::Look::Win98),
};
const NONE_2000: Small = Small {
    plain: crate::art::bytes("display/none", crate::art::Look::Win2000),
    selected: crate::art::bytes("display/none-selected", crate::art::Look::Win2000),
};
const BITMAP_2000: Small = Small {
    plain: crate::art::bytes("display/bitmap", crate::art::Look::Win2000),
    selected: crate::art::bytes("display/bitmap-selected", crate::art::Look::Win2000),
};

fn small_picture(theme: &Theme, rgb_bytes: &[u8]) -> crate::fb::Icon {
    let pixels = rgb_bytes.chunks(3).map(|p| 0xFF00_0000 | u32::from(p[0]) << 16 | u32::from(p[1]) << 8 | u32::from(p[2])).collect();
    let icon = crate::fb::Icon { width: 16, height: 16, pixels };
    crate::colors16::shown(theme, &icon).into_owned()
}

/// Display Properties' state: the look, what the page shows and what is in effect, the lists.
#[derive(Clone, Debug)]
pub struct DisplayProperties {
    pub era: Era,
    /// What the page shows (the person's choice so far).
    pub shown: Choice,
    /// What is in effect (OK and Apply put `shown` into effect; Apply grays until they differ).
    pub applied: Choice,
    pub focus: Part,
    pub pressed: Option<Part>,
    pub show_keys: bool,
    pub page: usize,
    pub pages: Vec<Page>,
    pub(crate) wallpapers: List,
    pub(crate) patterns: Option<List>,
    /// The Display list open, with the highlighted row.
    pub drop: Option<usize>,
    /// Where the window's left and top edges are on the screen.
    pub screen: (i32, i32),
    wallpaper_names: Vec<String>,
    pattern_names: Vec<String>,
}

const NONE: &str = "(None)";

impl DisplayProperties {
    /// The window as it opens, showing what is in effect for `era`.
    pub fn new(era: Era, applied: Choice) -> DisplayProperties {
        let wallpaper_names: Vec<String> =
            std::iter::once(NONE.to_string()).chain(display::wallpapers(era).iter().map(|w| w.name.to_string())).collect();
        let pattern_names: Vec<String> =
            std::iter::once(NONE.to_string()).chain(display::patterns(era).iter().map(|p| p.name.to_string())).collect();
        let o = ORIGIN;
        let wall_rect = match era {
            Era::Win95 => du(o, 135, 130, 103, 42),
            Era::Win98 => du(o, 14, 144, 166, 59),
            Era::Win2000 => du(o, 7, 141, 181, 70),
        };
        let row_h = if era == Era::Win95 { BOX_ROW } else { VIEW_ROW };
        let wallpapers = List { rect: wall_rect, row_h, rows: wallpaper_names.len(), sel: 0, top: 0 };
        let patterns =
            (era == Era::Win95).then(|| List { rect: du(o, 13, 130, 103, 42), row_h: BOX_ROW, rows: pattern_names.len(), sel: 0, top: 0 });
        let mut d = DisplayProperties {
            era,
            shown: applied.clone(),
            applied,
            // 95 opened with the keyboard in its Pattern list, 98 and 2000 in the wallpaper list.
            focus: if era == Era::Win95 { Part::List(Which::Pattern) } else { Part::List(Which::Wallpaper) },
            pressed: None,
            show_keys: false,
            page: 0,
            pages: vec![Page::Background],
            wallpapers,
            patterns,
            drop: None,
            screen: (0, 0),
            wallpaper_names,
            pattern_names,
        };
        d.sync_lists();
        d.applied = d.shown.clone();
        d
    }

    /// The window's title.
    pub fn title(&self) -> &'static str {
        "Display Properties"
    }

    /// Selects in the lists what `shown` says (a name the look does not have selects "(None)").
    fn sync_lists(&mut self) {
        let era = self.era;
        let w = self.shown.wallpaper_in(era).map(|w| w.name);
        self.wallpapers.sel = w.and_then(|n| self.wallpaper_names.iter().position(|x| x == n)).unwrap_or(0);
        self.wallpapers.show_sel();
        let p = self.shown.pattern_in(era).map(|p| p.name);
        let psel = p.and_then(|n| self.pattern_names.iter().position(|x| x == n)).unwrap_or(0);
        if let Some(list) = self.patterns.as_mut() {
            list.sel = psel;
            list.show_sel();
        }
        self.shown.wallpaper = w.map(str::to_string);
        self.shown.pattern = p.map(str::to_string);
        if era == Era::Win95 && self.shown.show == Show::Stretch {
            self.shown.show = Show::Center;
        }
    }

    /// Apply is usable once the page shows something else than what is in effect.
    pub fn changed(&self) -> bool {
        self.shown != self.applied
    }

    /// OK or Apply: what the page shows is now in effect; returned for the host to save and show.
    pub fn apply(&mut self) -> Choice {
        self.applied = self.shown.clone();
        self.applied.clone()
    }

    /// Shows `choice` on the page as if chosen there (tests, the owner's sheet).
    pub fn set_shown(&mut self, choice: Choice) {
        self.shown = choice;
        self.sync_lists();
    }

    /// The Pattern window's OK: the pattern chosen there (`None`: none).
    pub fn set_pattern(&mut self, name: Option<String>) {
        self.shown.pattern = name;
        self.sync_lists();
    }

    fn list(&self, which: Which) -> Option<&List> {
        match which {
            Which::Wallpaper => Some(&self.wallpapers),
            Which::Pattern => self.patterns.as_ref(),
        }
    }

    fn list_mut(&mut self, which: Which) -> Option<&mut List> {
        match which {
            Which::Wallpaper => Some(&mut self.wallpapers),
            Which::Pattern => self.patterns.as_mut(),
        }
    }

    fn names(&self, which: Which) -> &[String] {
        match which {
            Which::Wallpaper => &self.wallpaper_names,
            Which::Pattern => &self.pattern_names,
        }
    }

    /// The row selected in a list.
    pub fn selected(&self, which: Which) -> Option<usize> {
        self.list(which).map(|l| l.sel)
    }

    /// The first row shown of a list.
    pub fn top(&self, which: Which) -> Option<usize> {
        self.list(which).map(|l| l.top)
    }

    /// The selection of a list changed: the page's choice follows at once (Windows' monitor
    /// shows it before Apply).
    fn take_selection(&mut self, which: Which) {
        let Some(list) = self.list(which) else { return };
        let name = (list.sel > 0).then(|| self.names(which)[list.sel].clone());
        match which {
            Which::Wallpaper => self.shown.wallpaper = name,
            Which::Pattern => self.shown.pattern = name,
        }
    }

    pub fn enabled(&self, part: Part) -> bool {
        match part {
            // Later steps: the person's own pictures and the Pattern Editor.
            Part::Browse | Part::EditPattern => false,
            Part::Apply => self.changed(),
            // 98 and 2000 gray Pattern... whenever a wallpaper is tiled or stretched.
            Part::PatternButton => self.shown.wallpaper.is_none() || self.shown.show == Show::Center,
            // Grayed (showing Center) while no wallpaper is chosen.
            Part::ShowList | Part::ShowRow(_) => self.shown.wallpaper.is_some(),
            _ => true,
        }
    }

    /// The Display list's entries (98, 2000).
    pub fn show_choices(&self) -> &'static [Show] {
        Show::offered(self.era)
    }

    /// What the Display list shows: the way chosen, Center while no wallpaper is chosen.
    fn show_text(&self) -> &'static str {
        if self.shown.wallpaper.is_none() { Show::Center.label() } else { self.shown.show.label() }
    }

    pub fn tab_stops(&self) -> Vec<Part> {
        let mut stops = vec![Part::Tab(self.page)];
        match self.era {
            Era::Win95 => {
                stops.extend([Part::List(Which::Pattern), Part::EditPattern, Part::List(Which::Wallpaper), Part::Browse]);
                stops.push(if self.shown.show == Show::Tile { Part::Tile } else { Part::Center });
            }
            Era::Win98 => stops.extend([Part::List(Which::Wallpaper), Part::Browse, Part::PatternButton, Part::ShowList]),
            Era::Win2000 => stops.extend([Part::List(Which::Wallpaper), Part::Browse, Part::ShowList, Part::PatternButton]),
        }
        stops.extend([Part::Ok, Part::Cancel, Part::Apply]);
        stops.retain(|&p| self.enabled(p));
        stops
    }

    /// The button Enter carries out: the focused push button, else OK.
    pub fn default_button(&self) -> Part {
        if self.focus.is_push_button() { self.focus } else { Part::Ok }
    }

    /// The part an access key reaches (Alt+letter, or a letter outside the lists).
    pub fn access_key(&self, ch: char) -> Option<Part> {
        let ch = ch.to_ascii_lowercase();
        let on = if self.shown.show == Show::Tile { Part::Tile } else { Part::Center };
        let part = match (self.era, ch) {
            (Era::Win95, 'p') => Part::List(Which::Pattern),
            (Era::Win95, 'e') => Part::EditPattern,
            (Era::Win95, 'w') => Part::List(Which::Wallpaper),
            (Era::Win95, 'd') => on,
            (Era::Win95, 't') => Part::Tile,
            (Era::Win95, 'c') => Part::Center,
            (_, 'b') => Part::Browse,
            (Era::Win98 | Era::Win2000, 's') => Part::List(Which::Wallpaper),
            (Era::Win98 | Era::Win2000, 'p') => Part::PatternButton,
            (Era::Win98 | Era::Win2000, 'd') => Part::ShowList,
            _ => return None,
        };
        self.enabled(part).then_some(part)
    }

    /// Carries out a choice on the page; true if what it shows changed.
    pub fn choose(&mut self, part: Part) -> bool {
        match part {
            Part::Tile | Part::Center => {
                let show = if part == Part::Tile { Show::Tile } else { Show::Center };
                let changed = self.shown.show != show;
                self.shown.show = show;
                changed
            }
            Part::ShowRow(i) => {
                if !self.enabled(Part::ShowList) {
                    return false;
                }
                let Some(&show) = self.show_choices().get(i) else { return false };
                let changed = self.shown.show != show;
                self.shown.show = show;
                changed
            }
            Part::Row(which, i) => {
                let Some(list) = self.list_mut(which) else { return false };
                if i >= list.rows {
                    return false;
                }
                list.sel = i;
                list.show_sel();
                self.take_selection(which);
                true
            }
            _ => false,
        }
    }

    /// Moves a list's selection (keys); the choice follows.
    pub fn step(&mut self, which: Which, delta: i32) {
        if let Some(list) = self.list_mut(which)
            && list.step(delta)
        {
            self.take_selection(which);
        }
    }

    /// Scrolls a list by `delta` rows without moving its selection.
    pub fn scroll(&mut self, which: Which, delta: i32) {
        if let Some(list) = self.list_mut(which) {
            let top = (list.top as i32 + delta).max(0) as usize;
            list.set_top(top);
        }
    }

    pub fn show_page(&mut self, i: usize) {
        if i < self.pages.len() {
            self.page = i;
        }
    }

    /// The Display list's field (closed).
    fn show_field(&self) -> Rect {
        let o = ORIGIN;
        let r = if self.era == Era::Win2000 { du(o, 195, 174, 50, 40) } else { du(o, 187, 191, 50, 40) };
        Rect::new(r.x, r.y, r.w, 21)
    }

    fn drop_frame(&self) -> Option<Rect> {
        self.drop?;
        let f = self.show_field();
        Some(Rect::new(f.x, f.bottom(), f.w, self.show_choices().len() as i32 * DROP_ROW + 2))
    }

    fn monitor_at(&self) -> (i32, i32) {
        let o = ORIGIN;
        let r = if self.era == Era::Win2000 { du(o, 63, 7, 125, 107) } else { du(o, 63, 4, 125, 107) };
        (r.x + 2, r.y + 2)
    }

    fn tabs(&self, font: &Font) -> Vec<Rect> {
        let mut x = TAB_X;
        self.pages
            .iter()
            .map(|p| {
                let w = font.measure(p.title()) + 2 * TAB_PAD;
                let r = Rect::new(x, TAB_Y, w, TAB_H);
                x += w;
                r
            })
            .collect()
    }

    /// Every part with its rectangle, the topmost first.
    pub fn parts(&self, fonts: &Fonts) -> Vec<(Part, Rect)> {
        let o = ORIGIN;
        let font = dialog_font(self.era, fonts);
        let mut parts: Vec<(Part, Rect)> = Vec::new();
        if let Some(frame) = self.drop_frame() {
            for i in 0..self.show_choices().len() {
                parts.push((Part::ShowRow(i), Rect::new(frame.x + 1, frame.y + 1 + i as i32 * DROP_ROW, frame.w - 2, DROP_ROW)));
            }
        }
        let (help, close) = dialog::title_button_rects(SIZE.0);
        parts.extend([(Part::TitleHelp, help), (Part::TitleClose, close)]);
        for (i, t) in self.tabs(font).into_iter().enumerate() {
            let r = if i == self.page { Rect::new(t.x - 2, t.y - 2, t.w + 4, t.h + 2) } else { t };
            parts.push((Part::Tab(i), r));
        }
        for which in [Which::Wallpaper, Which::Pattern] {
            if let Some(list) = self.list(which) {
                parts.extend(list.scroll_parts(which));
                parts.extend(list.shown_rows().into_iter().map(|(i, r)| (Part::Row(which, i), r)));
                parts.push((Part::List(which), list.rect));
            }
        }
        match self.era {
            Era::Win95 => {
                parts.push((Part::EditPattern, du(o, 55, 175, 61, 14)));
                parts.push((Part::Browse, du(o, 177, 175, 61, 14)));
                for (part, r, label) in [(Part::Tile, du(o, 170, 196, 27, 8), "&Tile"), (Part::Center, du(o, 205, 196, 34, 8), "&Center")] {
                    parts.push((part, Rect::new(r.x, r.y, r.w.max(16 + font.measure_label(label) + 1), r.h)));
                }
            }
            Era::Win98 => {
                parts.push((Part::Browse, du(o, 187, 144, 50, 14)));
                parts.push((Part::PatternButton, du(o, 187, 162, 50, 14)));
                parts.push((Part::ShowList, self.show_field()));
            }
            Era::Win2000 => {
                parts.push((Part::Browse, du(o, 195, 142, 50, 14)));
                parts.push((Part::ShowList, self.show_field()));
                parts.push((Part::PatternButton, du(o, 195, 195, 50, 14)));
            }
        }
        for (part, x) in BUTTONS {
            parts.push((part, Rect::new(x, BUTTONS_Y, 75, 23)));
        }
        parts
    }

    /// Paints the whole window (404x448).
    pub fn paint(&self, fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts) {
        let c = &theme.colors;
        let font = dialog_font(self.era, fonts);
        let keys = self.show_keys;
        let parts = self.parts(fonts);
        let rect = |part: Part| parts.iter().find(|(p, _)| *p == part).map(|&(_, r)| r);
        let held = |part: Part| self.pressed == Some(part);
        let buttons = TitleButtons {
            help: rect(Part::TitleHelp).map(|r| (r, held(Part::TitleHelp))),
            close: rect(Part::TitleClose).map(|r| (r, held(Part::TitleClose))),
        };
        dialog::window_frame(fb, theme, fonts, self.era, self.title(), &buttons);

        // The tabs not chosen, the page's frame, then the chosen tab over the frame's top line.
        let tabs = self.tabs(font);
        for (i, &t) in tabs.iter().enumerate() {
            if i != self.page {
                tab(fb, c, t);
                font.draw(fb, t.x + TAB_PAD, t.y + 3, self.pages[i].title(), c.text);
            }
        }
        fb.frame(PAGE, c.highlight, c.dark_shadow);
        fb.frame(PAGE.inset(1, 1, 1, 1), c.light, c.shadow);
        if let Some(&t) = tabs.get(self.page) {
            let drawn = Rect::new(t.x - 2, t.y - 2, t.w + 4, t.h + 3);
            tab(fb, c, drawn);
            // Under the chosen tab the page's light inner line is not drawn (the Reference PCs).
            fb.fill(Rect::new(drawn.x + 2, PAGE.y + 1, drawn.w - 2, 1), c.face);
            font.draw(fb, t.x + TAB_PAD, drawn.y + 3, self.pages[self.page].title(), c.text);
            if self.focus == Part::Tab(self.page) && keys {
                dialog::focus_dots(fb, t.inset(1, 1, 1, 1), (0, 0), c.text);
            }
        }

        let (mx, my) = self.monitor_at();
        display::paint_monitor(fb, theme, mx, my, &self.shown);

        let o = ORIGIN;
        let framed = self.default_button();
        let button = |fb: &mut Framebuffer, part: Part, label: &str| {
            if let Some(r) = rect(part) {
                let focused = self.focus == part && keys;
                dialog::push_button(fb, theme, font, r, label, framed == part, held(part), focused, self.enabled(part), keys);
            }
        };
        let text = |fb: &mut Framebuffer, r: Rect, label: &str, enabled: bool| {
            if enabled {
                font.draw_label(fb, (r.x, r.y), label, i32::MAX, c.text, keys);
            } else {
                // Etched, cut at the control's own height (2000's grayed Picture Display:).
                font.draw_label(fb, (r.x + 1, r.y + 1), label, i32::MAX, c.etch, keys);
                fb.fill(Rect::new(r.x, r.bottom(), font.measure_label(label) + 2, 1), c.face);
                font.draw_label(fb, (r.x, r.y), label, i32::MAX, c.shadow, keys);
            }
        };
        // A group box's control rectangle starts at its label's top; the frame runs through the
        // label's middle.
        let group = |fb: &mut Framebuffer, r: Rect, label: &str| {
            let frame = Rect::new(r.x, r.y + 6, r.w, r.h - 6);
            group_box(fb, c, font, frame, label, keys);
        };
        let scroll_held = |which: Which| match self.pressed {
            Some(Part::Scroll(w, b)) if w == which => Some(b),
            _ => None,
        };
        match self.era {
            Era::Win95 => {
                group(fb, du(o, 7, 118, 115, 92), "&Pattern");
                group(fb, du(o, 129, 118, 115, 92), "&Wallpaper");
                if let Some(list) = &self.patterns {
                    let focused = self.focus == Part::List(Which::Pattern);
                    paint_box(fb, theme, font, list, &self.pattern_names, focused && keys, scroll_held(Which::Pattern), true);
                }
                let focused = self.focus == Part::List(Which::Wallpaper);
                paint_box(fb, theme, font, &self.wallpapers, &self.wallpaper_names, focused && keys, scroll_held(Which::Wallpaper), true);
                button(fb, Part::EditPattern, "&Edit Pattern...");
                button(fb, Part::Browse, "&Browse...");
                text(fb, du(o, 135, 196, 26, 8), "&Display:", true);
                for (part, label, on) in [(Part::Tile, "&Tile", self.shown.show == Show::Tile), (Part::Center, "&Center", self.shown.show != Show::Tile)] {
                    let Some(r) = rect(part) else { continue };
                    let rows = if on { OPTION_ON } else { OPTION_OFF };
                    dialog::art(fb, rows, r.x + 1, r.y, |b| match b {
                        b'g' => Some(c.shadow),
                        b'#' => Some(c.dark_shadow),
                        b'W' => Some(if held(part) { c.face } else { c.highlight }),
                        b'l' => Some(c.light),
                        _ => None,
                    });
                    // The words 18 in (the Reference PC).
                    let (tx, ty) = (r.x + 18, r.y);
                    font.draw_label(fb, (tx, ty), label, i32::MAX, c.text, keys);
                    if self.focus == part && keys {
                        let w = font.measure_label(label);
                        // Drawn over the words as Windows' focus rectangle is: where a dot meets the access
                        // key's line it shows the face (the Reference PC).
                        let f = Rect::new(tx - 1, ty, w + 2, font.height);
                        for y in f.y..f.bottom() {
                            for x in f.x..f.right() {
                                let edge = x == f.x || y == f.y || x == f.right() - 1 || y == f.bottom() - 1;
                                if edge && (x - r.x + y - r.y).rem_euclid(2) == 1 && x >= 0 && y >= 0 && x < fb.width && y < fb.height {
                                    let p = fb.get(x, y);
                                    fb.set(x, y, if p == c.text { c.face } else { c.text });
                                }
                            }
                        }
                    }
                }
            }
            era => {
                if era == Era::Win98 {
                    group(fb, du(o, 7, 118, 238, 93), "Wallpaper");
                    text(fb, du(o, 13, 132, 162, 10), "&Select an HTML Document or a picture:", true);
                    text(fb, du(o, 187, 181, 50, 10), "&Display:", true);
                } else {
                    text(fb, du(o, 7, 131, 238, 8), "&Select a background picture or HTML document as Wallpaper:", true);
                    // 2000 grays its label with the list, 98 does not (the Reference PCs).
                    text(fb, du(o, 195, 164, 50, 8), "Picture &Display:", self.enabled(Part::ShowList));
                }
                self.paint_view(fb, theme, font, scroll_held(Which::Wallpaper));
                button(fb, Part::Browse, "&Browse...");
                button(fb, Part::PatternButton, "&Pattern...");
                self.paint_show_field(fb, theme, font);
            }
        }
        for (part, _) in BUTTONS {
            let label = match part {
                Part::Ok => "OK",
                Part::Cancel => "Cancel",
                _ => "&Apply",
            };
            button(fb, part, label);
        }
        // The open Display list, over everything.
        if let (Some(hover), Some(frame)) = (self.drop, self.drop_frame()) {
            fb.fill(frame, rgb(0xFFFFFF));
            fb.frame(frame, c.text, c.text);
            for (i, show) in self.show_choices().iter().enumerate() {
                let row = Rect::new(frame.x + 1, frame.y + 1 + DROP_ROW * i as i32, frame.w - 2, DROP_ROW);
                let ink = if i == hover {
                    fb.fill(row, c.menu_highlight);
                    c.menu_highlight_text
                } else {
                    c.text
                };
                font.draw(fb, row.x + 2, row.y, show.label(), ink);
                if i == hover {
                    focus_dots(fb, theme, row, (row.x, row.y));
                }
            }
        }
    }

    /// 98's and 2000's list view: each wallpaper's small picture and its name, the selected one's
    /// name in the selection color (the face color while the list does not have the keyboard) and
    /// its picture as drawn selected.
    fn paint_view(&self, fb: &mut Framebuffer, theme: &Theme, font: &Font, held: Option<Bar>) {
        let c = &theme.colors;
        let list = &self.wallpapers;
        dialog::well(fb, list.rect, c);
        let rows = list.rows_rect();
        let mut area = Framebuffer::new(rows.w, rows.h, rgb(0xFFFFFF));
        let focused = self.focus == Part::List(Which::Wallpaper);
        let (none, bitmap) = if self.era == Era::Win2000 { (&NONE_2000, &BITMAP_2000) } else { (&NONE_98, &BITMAP_98) };
        for (i, name) in self.wallpaper_names.iter().enumerate() {
            let r = list.row_rect(i);
            let y = r.y - rows.y;
            if y + r.h <= 0 || y >= rows.h {
                continue;
            }
            let selected = i == list.sel;
            let small = if i == 0 { none } else { bitmap };
            // The picture shows selected while the list has the keyboard (98), always (2000).
            let as_selected = selected && (focused || self.era == Era::Win2000);
            let picture = small_picture(theme, if as_selected { small.selected } else { small.plain });
            area.draw_icon(&picture, 2, y);
            let w = font.measure(name);
            // The label's box: the text 2 in, 6 more after it (the Reference PCs).
            let label = Rect::new(18, y, w + 8, r.h);
            let ink = if selected && focused {
                area.fill(label, c.menu_highlight);
                c.menu_highlight_text
            } else if selected {
                area.fill(label, c.face);
                c.text
            } else {
                c.text
            };
            font.draw(&mut area, label.x + 2, y + 2, name, ink);
            if selected && focused && self.show_keys {
                focus_dots(&mut area, theme, label, (0, 0));
            }
        }
        fb.blit(&area, rows.x, rows.y);
        if let Some(bar) = list.scroll() {
            bar.paint(fb, c, list.top, held == Some(Bar::Up), held == Some(Bar::Down));
        }
    }

    /// The Display list's field: what is chosen, highlighted while it has the keyboard; grayed
    /// (the face inside, the words etched) while no wallpaper is chosen.
    fn paint_show_field(&self, fb: &mut Framebuffer, theme: &Theme, font: &Font) {
        let c = &theme.colors;
        let r = self.show_field();
        let field = dialog::well(fb, r, c);
        let item = Rect::new(field.x + 1, field.y + 1, field.w - 2 - 16, field.h - 2);
        let text = self.show_text();
        let enabled = self.enabled(Part::ShowList);
        if !enabled {
            // Grayed (the Reference PCs): the face inside, the words plain gray.
            fb.fill(field, c.face);
            font.draw(fb, item.x + 1, item.y + 1, text, c.shadow);
        } else {
            let focused = self.focus == Part::ShowList && self.drop.is_none();
            let ink = if focused {
                fb.fill(item, c.menu_highlight);
                c.menu_highlight_text
            } else {
                c.text
            };
            font.draw(fb, item.x + 1, item.y + 1, text, ink);
            if focused && self.show_keys {
                combo_focus(fb, theme, item);
            }
        }
        let button = Rect::new(field.right() - 16, field.y, 16, field.h);
        dialog::drop_down(fb, button, c, self.pressed == Some(Part::ShowList));
        if !enabled {
            // Its arrow grayed: gray over a white copy one pixel right and down.
            for row in 0..4 {
                fb.hline(button.x + 4 + row, button.y + 6 + row, 7 - 2 * row, c.face);
            }
            for row in 0..4 {
                fb.hline(button.x + 5 + row, button.y + 7 + row, 7 - 2 * row, c.highlight);
            }
            for row in 0..4 {
                fb.hline(button.x + 4 + row, button.y + 6 + row, 7 - 2 * row, c.shadow);
            }
        }
    }
}

/// Display Properties as a window of the control kit.
#[derive(Clone, Debug)]
pub struct DisplayWindow {
    pub props: DisplayProperties,
    parts: Vec<(Part, Rect)>,
}

impl DisplayWindow {
    pub fn new(props: DisplayProperties, fonts: &Fonts) -> DisplayWindow {
        let parts = props.parts(fonts);
        DisplayWindow { props, parts }
    }

    fn open_or_close_drop(&mut self) {
        let p = &mut self.props;
        p.drop = match p.drop {
            Some(_) => None,
            None => Some(p.show_choices().iter().position(|&s| s == p.shown.show).unwrap_or(0)),
        };
    }

    fn focused_list(&self) -> Option<Which> {
        match self.props.focus {
            Part::List(w) => Some(w),
            _ => None,
        }
    }
}

impl kit::Form for DisplayWindow {
    type Part = Part;

    fn title(&self) -> &str {
        self.props.title()
    }
    fn size(&self) -> (i32, i32) {
        SIZE
    }
    fn layout(&mut self, fonts: &Fonts) {
        self.parts = self.props.parts(fonts);
    }
    fn paint(&self, fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts) {
        self.props.paint(fb, theme, fonts);
    }
    fn set_screen(&mut self, x: i32, y: i32) {
        self.props.screen = (x, y);
    }
    fn hit(&self, x: i32, y: i32) -> Option<Part> {
        self.parts.iter().find(|(_, r)| r.contains(x, y)).map(|&(p, _)| p)
    }
    fn rect(&self, part: Part) -> Option<Rect> {
        self.parts.iter().find(|(p, _)| *p == part).map(|&(_, r)| r)
    }
    fn enabled(&self, part: Part) -> bool {
        self.props.enabled(part)
    }
    fn focus(&self) -> Part {
        self.props.focus
    }
    fn set_focus(&mut self, part: Part) {
        self.props.focus = part;
        self.props.drop = None;
    }
    fn tab_stops(&self) -> Vec<Part> {
        self.props.tab_stops()
    }
    fn set_pressed(&mut self, part: Option<Part>) {
        self.props.pressed = part;
    }
    fn show_keys(&mut self) {
        self.props.show_keys = true;
    }
    fn enter(&self) -> Part {
        self.props.default_button()
    }
    fn escape(&self) -> Part {
        Part::Cancel
    }

    fn key(&mut self, key: Key, _mods: Mods) -> Option<Vec<Effect<Part>>> {
        if self.props.focus == Part::ShowList && self.props.enabled(Part::ShowList) {
            let p = &mut self.props;
            let n = p.show_choices().len();
            match (key, p.drop) {
                (Key::Enter, Some(hover)) => {
                    p.drop = None;
                    p.choose(Part::ShowRow(hover));
                    return Some(Vec::new());
                }
                (Key::Escape, Some(_)) => {
                    p.drop = None;
                    return Some(Vec::new());
                }
                (Key::F4, _) => {
                    self.open_or_close_drop();
                    return Some(Vec::new());
                }
                (Key::Up | Key::Down | Key::Left | Key::Right | Key::Home | Key::End, drop) => {
                    let at = drop.unwrap_or_else(|| p.show_choices().iter().position(|&s| s == p.shown.show).unwrap_or(0));
                    let to = match key {
                        Key::Home => 0,
                        Key::End => n - 1,
                        Key::Up | Key::Left => at.saturating_sub(1),
                        _ => (at + 1).min(n - 1),
                    };
                    if drop.is_some() {
                        p.drop = Some(to);
                    } else {
                        p.choose(Part::ShowRow(to));
                    }
                    return Some(Vec::new());
                }
                _ => {}
            }
        }
        if let Some(which) = self.focused_list() {
            let page = self.props.list(which).map_or(1, |l| l.page() as i32);
            let delta = match key {
                Key::Up | Key::Left => -1,
                Key::Down | Key::Right => 1,
                Key::PageUp => -(page - 1).max(1),
                Key::PageDown => (page - 1).max(1),
                Key::Home => -10_000,
                Key::End => 10_000,
                _ => return None,
            };
            self.props.step(which, delta);
            return Some(Vec::new());
        }
        let p = &mut self.props;
        match key {
            Key::Up | Key::Down | Key::Left | Key::Right if matches!(p.focus, Part::Tile | Part::Center) => {
                let next = if p.focus == Part::Tile { Part::Center } else { Part::Tile };
                p.focus = next;
                p.choose(next);
                Some(Vec::new())
            }
            Key::Left | Key::Right if matches!(p.focus, Part::Tab(_)) => Some(Vec::new()),
            _ => None,
        }
    }

    fn typed(&mut self, ch: char) -> Vec<Effect<Part>> {
        if !ch.is_alphanumeric() {
            return Vec::new();
        }
        self.props.show_keys = true;
        // In a list a letter goes to the next entry that starts with it.
        if let Some(which) = self.focused_list() {
            let names = self.props.names(which).to_vec();
            if let Some(i) = self.props.list(which).and_then(|l| l.find(&names, ch)) {
                self.props.choose(Part::Row(which, i));
            }
            return Vec::new();
        }
        if self.props.focus == Part::ShowList && self.props.enabled(Part::ShowList) {
            let choices = self.props.show_choices();
            if let Some(i) = choices.iter().position(|s| s.label().starts_with(ch.to_ascii_uppercase())) {
                match self.props.drop.as_mut() {
                    Some(h) => *h = i,
                    None => {
                        self.props.choose(Part::ShowRow(i));
                    }
                }
            }
            return Vec::new();
        }
        self.access(ch)
    }

    fn access(&mut self, ch: char) -> Vec<Effect<Part>> {
        let p = &mut self.props;
        p.drop = None;
        match p.access_key(ch) {
            Some(part @ (Part::Tile | Part::Center)) => {
                p.focus = part;
                p.choose(part);
            }
            Some(part @ (Part::List(_) | Part::ShowList)) => p.focus = part,
            Some(part) => {
                p.focus = part;
                return kit::activate(self, part);
            }
            None => {}
        }
        Vec::new()
    }

    fn space(&mut self) -> Vec<Effect<Part>> {
        let f = self.props.focus;
        match f {
            Part::Tile | Part::Center => {
                self.props.choose(f);
            }
            f if f.is_push_button() => return kit::activate(self, f),
            _ => {}
        }
        Vec::new()
    }

    fn press(&mut self, part: Option<Part>, _x: i32, _y: i32) -> Press<Part> {
        // While the Display list is open a press takes its entry, or closes it.
        if self.props.drop.take().is_some() {
            if let Some(Part::ShowRow(i)) = part {
                self.props.choose(Part::ShowRow(i));
            }
            return Press::Done;
        }
        match part {
            Some(Part::ShowList) if self.props.enabled(Part::ShowList) => {
                self.props.focus = Part::ShowList;
                self.open_or_close_drop();
            }
            Some(Part::Tab(i)) => {
                self.props.show_page(i);
                self.props.focus = Part::Tab(i);
            }
            Some(Part::Row(which, i)) => {
                self.props.focus = Part::List(which);
                self.props.choose(Part::Row(which, i));
            }
            Some(Part::List(which)) => self.props.focus = Part::List(which),
            Some(part @ Part::Scroll(which, bar)) => {
                let p = &mut self.props;
                let page = p.list(which).map_or(1, |l| l.page() as i32);
                match bar {
                    Bar::Up | Bar::Down => {
                        p.scroll(which, if bar == Bar::Up { -1 } else { 1 });
                        return Press::Hold(part);
                    }
                    Bar::PageUp => p.scroll(which, -page),
                    Bar::PageDown => p.scroll(which, page),
                    Bar::Thumb => return Press::Grab(part, p.list(which).map_or(0, |l| l.top as i32)),
                }
            }
            Some(part) if self.props.enabled(part) => {
                if !matches!(part, Part::TitleHelp | Part::TitleClose) {
                    self.props.focus = part;
                }
                return Press::Hold(part);
            }
            Some(_) => {}
            None => return Press::Nothing,
        }
        Press::Done
    }

    fn release(&mut self, part: Part) -> Vec<Effect<Part>> {
        match part {
            Part::Tile | Part::Center => {
                self.props.choose(part);
                Vec::new()
            }
            p if p.is_push_button() || matches!(p, Part::TitleHelp | Part::TitleClose) => vec![Effect::Activate(p)],
            _ => Vec::new(),
        }
    }

    fn drag(&mut self, part: Part, grab_y: i32, grab_top: i32, _x: i32, y: i32) -> bool {
        let Part::Scroll(which, Bar::Thumb) = part else { return false };
        let Some(list) = self.props.list_mut(which) else { return false };
        let Some(bar) = list.scroll() else { return false };
        let top = bar.dragged(grab_top, y - grab_y);
        let changed = top != list.top;
        list.set_top(top);
        changed
    }

    fn alt_key(&mut self, key: Key) -> bool {
        if self.props.focus != Part::ShowList || !matches!(key, Key::Up | Key::Down) || !self.props.enabled(Part::ShowList) {
            return false;
        }
        match self.props.drop.take() {
            Some(hover) => {
                self.props.choose(Part::ShowRow(hover));
            }
            None => self.open_or_close_drop(),
        }
        true
    }

    fn hover(&mut self, x: i32, y: i32) -> bool {
        let Some(hover) = self.props.drop else { return false };
        match kit::Form::hit(self, x, y) {
            Some(Part::ShowRow(i)) if i != hover => {
                self.props.drop = Some(i);
                true
            }
            _ => false,
        }
    }

    fn set_active(&mut self, active: bool) {
        if !active {
            self.props.drop = None;
        }
    }

    fn wheel(&mut self, notches: i32) {
        let which = self.focused_list().unwrap_or(Which::Wallpaper);
        self.props.scroll(which, -3 * notches);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::{Controller, Form};
    use crate::text::Font;

    fn fonts() -> (Font, Font) {
        let f = Font::parse(include_str!("../../tests/fixtures/ms-sans-serif-11px.font")).unwrap();
        (f.clone(), f)
    }

    fn window(era: Era, applied: Choice) -> DisplayWindow {
        let (r, b) = fonts();
        DisplayWindow::new(DisplayProperties::new(era, applied), &Fonts { regular: &r, bold: &b })
    }

    fn relayout(w: &mut DisplayWindow) {
        let (r, b) = fonts();
        w.layout(&Fonts { regular: &r, bold: &b });
    }

    fn centre(w: &DisplayWindow, part: Part) -> (i32, i32) {
        let r = w.rect(part).unwrap_or_else(|| panic!("{part:?} has a place"));
        (r.x + r.w / 2, r.y + r.h / 2)
    }

    #[test]
    fn each_look_lists_none_first_then_its_own_names() {
        let w = window(Era::Win98, Choice::plain());
        assert_eq!(w.props.wallpaper_names.len(), 21);
        assert_eq!((w.props.wallpaper_names[0].as_str(), w.props.wallpaper_names[1].as_str()), ("(None)", "1stboot"));
        assert!(w.props.patterns.is_none(), "98's patterns are in the Pattern window");
        let w = window(Era::Win95, Choice::plain());
        assert_eq!(w.props.pattern_names.len(), 20);
        assert_eq!(w.props.patterns.unwrap().rows, 20);
        assert_eq!(w.props.wallpaper_names[8], "Setup");
        let w = window(Era::Win2000, Choice::plain());
        assert_eq!(w.props.wallpaper_names.last().unwrap(), "Zapotec");
        assert_eq!(w.props.pattern_names.len(), 14);
    }

    #[test]
    fn it_opens_on_what_is_in_effect_with_apply_grayed() {
        let applied = Choice { wallpaper: Some("Forest".into()), show: Show::Tile, pattern: Some("Daisies".into()) };
        let w = window(Era::Win98, applied.clone());
        assert_eq!(w.props.shown, applied);
        assert_eq!(w.props.selected(Which::Wallpaper), Some(8));
        assert!(!w.props.enabled(Part::Apply));
        assert_eq!(w.props.focus, Part::List(Which::Wallpaper));
        // A name the look does not have opens as "(None)".
        let w = window(Era::Win98, Choice { wallpaper: Some("Zapotec".into()), ..Choice::plain() });
        assert_eq!((w.props.selected(Which::Wallpaper), w.props.shown.wallpaper.clone()), (Some(0), None));
        assert!(!w.props.enabled(Part::Apply));
        // 95 opens with the keyboard in its Pattern list.
        assert_eq!(window(Era::Win95, Choice::plain()).props.focus, Part::List(Which::Pattern));
    }

    #[test]
    fn a_choice_turns_apply_on_and_apply_puts_it_in_effect() {
        let mut w = window(Era::Win98, Choice::plain());
        let mut c = Controller::default();
        c.key(&mut w, Key::Down, Mods::default());
        assert_eq!(w.props.shown.wallpaper.as_deref(), Some("1stboot"));
        assert!(w.props.enabled(Part::Apply));
        // The monitor shows it at once; Apply puts it in effect and grays again.
        let applied = w.props.apply();
        assert_eq!(applied.wallpaper.as_deref(), Some("1stboot"));
        assert!(!w.props.enabled(Part::Apply));
        // Another change after Apply: Cancel (the host closing without OK) leaves what Apply did.
        c.key(&mut w, Key::Down, Mods::default());
        assert_eq!(w.props.applied.wallpaper.as_deref(), Some("1stboot"));
        assert_eq!(w.props.shown.wallpaper.as_deref(), Some("Black Thatch"));
        // Back to what is in effect: nothing to apply.
        c.key(&mut w, Key::Up, Mods::default());
        assert!(!w.props.enabled(Part::Apply));
    }

    #[test]
    fn the_grayed_controls_do_not_act() {
        let mut w = window(Era::Win98, Choice::plain());
        let mut c = Controller::default();
        // Browse... is grayed: a click, Alt+B and Tab never reach it.
        let (x, y) = centre(&w, Part::Browse);
        assert!(c.mouse_down(&mut w, x, y).is_empty());
        assert!(c.mouse_up(&mut w, x, y).is_empty());
        assert!(c.access(&mut w, 'b').is_empty());
        assert!(!w.tab_stops().contains(&Part::Browse));
        // The Display list is grayed while no wallpaper is chosen.
        assert!(!w.props.enabled(Part::ShowList));
        assert!(!w.props.choose(Part::ShowRow(1)));
        assert_eq!(w.props.shown.show, Show::Center);
        // Tiled, Pattern... grays (patterns show only with Center or no wallpaper).
        w.props.choose(Part::Row(Which::Wallpaper, 3));
        assert!(w.props.enabled(Part::PatternButton));
        w.props.choose(Part::ShowRow(1));
        assert_eq!(w.props.shown.show, Show::Tile);
        assert!(!w.props.enabled(Part::PatternButton));
        assert!(c.access(&mut w, 'p').is_empty());
        // 95: Edit Pattern... is grayed for now, Browse... too.
        let mut w = window(Era::Win95, Choice::plain());
        relayout(&mut w);
        let (x, y) = centre(&w, Part::EditPattern);
        assert!(c.mouse_down(&mut w, x, y).is_empty());
        assert!(c.access(&mut w, 'e').is_empty());
    }

    #[test]
    fn ok_apply_cancel_and_pattern_reach_the_host() {
        let mut w = window(Era::Win2000, Choice::plain());
        let mut c = Controller::default();
        assert!(c.key(&mut w, Key::Enter, Mods::default()) == vec![Effect::Activate(Part::Ok)]);
        assert!(c.key(&mut w, Key::Escape, Mods::default()) == vec![Effect::Activate(Part::Cancel)]);
        assert!(c.access(&mut w, 'p') == vec![Effect::Activate(Part::PatternButton)]);
        // Apply only once something changed.
        assert!(c.access(&mut w, 'a').is_empty());
        w.props.choose(Part::Row(Which::Wallpaper, 1));
        relayout(&mut w);
        let (x, y) = centre(&w, Part::Apply);
        assert_eq!(c.mouse_down(&mut w, x, y), vec![Effect::Capture]);
        assert_eq!(c.mouse_up(&mut w, x, y), vec![Effect::Release, Effect::Activate(Part::Apply)]);
    }

    #[test]
    fn the_95_page_has_tile_and_center_and_two_lists() {
        let mut w = window(Era::Win95, Choice::plain());
        let mut c = Controller::default();
        c.key(&mut w, Key::Down, Mods::default());
        assert_eq!(w.props.shown.pattern.as_deref(), Some("Bricks"));
        c.access(&mut w, 'w');
        w.typed('s');
        assert_eq!(w.props.shown.wallpaper.as_deref(), Some("Setup"));
        c.access(&mut w, 't');
        assert_eq!(w.props.shown.show, Show::Tile);
        c.key(&mut w, Key::Right, Mods::default());
        assert_eq!((w.props.focus, w.props.shown.show), (Part::Center, Show::Center));
        // 95 never shows Stretch, even when told so.
        let w = window(Era::Win95, Choice { wallpaper: Some("Waves".into()), show: Show::Stretch, pattern: None });
        assert_eq!(w.props.shown.show, Show::Center);
    }

    #[test]
    fn the_display_list_opens_and_takes_an_entry() {
        let mut w = window(Era::Win98, Choice { wallpaper: Some("Clouds".into()), ..Choice::plain() });
        let mut c = Controller::default();
        c.access(&mut w, 'd');
        assert_eq!(w.props.focus, Part::ShowList);
        assert!(c.alt_key(&mut w, Key::Down).0);
        assert_eq!(w.props.drop, Some(0));
        c.key(&mut w, Key::Down, Mods::default());
        c.key(&mut w, Key::Down, Mods::default());
        assert_eq!(w.props.shown.show, Show::Center, "nothing taken while it is open");
        c.key(&mut w, Key::Enter, Mods::default());
        assert_eq!((w.props.drop, w.props.shown.show), (None, Show::Stretch));
        // Esc closes it as it was.
        c.key(&mut w, Key::F4, Mods::default());
        c.key(&mut w, Key::Up, Mods::default());
        c.key(&mut w, Key::Escape, Mods::default());
        assert_eq!((w.props.drop, w.props.shown.show), (None, Show::Stretch));
    }

    #[test]
    fn the_pattern_window_hands_back_its_choice() {
        use crate::display::pattern::PatternWindow;
        let mut p = PatternWindow::new(Era::Win98, None);
        assert_eq!(p.chosen(), None);
        let mut c = Controller::default();
        c.key(&mut p, Key::Down, Mods::default());
        assert_eq!(p.chosen().as_deref(), Some("Bricks"));
        p.typed('w');
        assert_eq!(p.chosen().as_deref(), Some("Waffle's Revenge"));
        assert!(c.key(&mut p, Key::Enter, Mods::default()) == vec![Effect::Activate(crate::display::pattern::Part::Ok)]);
        let p = PatternWindow::new(Era::Win2000, Some("Weave"));
        assert_eq!(p.chosen().as_deref(), Some("Weave"));
        // The page takes it: the pattern shows on the monitor and Apply turns on.
        let mut w = window(Era::Win2000, Choice::plain());
        w.props.set_pattern(p.chosen());
        assert_eq!(w.props.shown.pattern.as_deref(), Some("Weave"));
        assert!(w.props.enabled(Part::Apply));
    }
}