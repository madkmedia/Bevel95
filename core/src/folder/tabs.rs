//! Tabs in folder windows (brief 110, 2026-11-25): Bevel95's own optional extra, off unless
//! chosen (`folder_tabs` in settings.txt). The old Windows had no tabs in a folder window; the
//! row is the look's ordinary tabs exactly as the property sheets paint them (`properties::tab`,
//! measured against 2000's Task Manager in brief 109: 0 pixels differ), no pictures on them, the
//! line under them running on to the row's end, and a small flat X: in 98 and 2000 the tree
//! pane's own close button ("All Folders", "Folders"), in 95 its own close glyph drawn alone
//! (PROVISIONAL: 95 had no flat X).
//!
//! This module lays the row out and paints it; the folder window (`FolderWindow`) says where it
//! goes, and the folder program keeps the tabs' views.

use crate::fb::{Framebuffer, Rect};
use crate::text::Font;
use crate::theme::{Era, Theme};

/// Where the row goes (`folder_tabs`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Place {
    /// No row: the folder windows as they always were.
    #[default]
    Off,
    /// Right under the toolbar and the address bar (95: under the menu bar), over both panes.
    UnderAddress,
    /// Over the file list only; the tree's pane begins at the row's top (no tree: as
    /// `UnderAddress`).
    OverFiles,
    /// Between the menu bar and the toolbar (no toolbar, as 95's: as `UnderAddress`).
    UnderMenu,
    /// Directly under the title bar, above File, Edit and the rest.
    AboveMenu,
}

impl Place {
    pub const ALL: [Place; 5] = [Place::Off, Place::UnderAddress, Place::OverFiles, Place::UnderMenu, Place::AboveMenu];

    /// Its word in settings.txt.
    pub fn word(self) -> &'static str {
        match self {
            Place::Off => "off",
            Place::UnderAddress => "under_address",
            Place::OverFiles => "over_files",
            Place::UnderMenu => "under_menu",
            Place::AboveMenu => "above_menu",
        }
    }

    pub fn from_word(word: &str) -> Option<Place> {
        Place::ALL.into_iter().find(|p| p.word() == word)
    }

    /// Its name in Bevel95 Settings.
    pub fn label(self) -> &'static str {
        match self {
            Place::Off => "None",
            Place::UnderAddress => "Under the address bar",
            Place::OverFiles => "Over the file list",
            Place::UnderMenu => "Under the menu bar",
            Place::AboveMenu => "Above the menu bar",
        }
    }
}

/// Where the small X goes (`folder_tabs_close`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CloseAt {
    /// One X at the row's right end, closing the tab in front.
    #[default]
    End,
    /// On the tab in front, right of its name (the other tabs carry none).
    Tab,
}

impl CloseAt {
    pub const ALL: [CloseAt; 2] = [CloseAt::End, CloseAt::Tab];

    pub fn word(self) -> &'static str {
        match self {
            CloseAt::End => "end",
            CloseAt::Tab => "tab",
        }
    }

    pub fn from_word(word: &str) -> Option<CloseAt> {
        CloseAt::ALL.into_iter().find(|p| p.word() == word)
    }

    pub fn label(self) -> &'static str {
        match self {
            CloseAt::End => "At the end of the row",
            CloseAt::Tab => "On the tab in front",
        }
    }
}

/// The row's height: a gap of 2, the chosen tab's rise of 2, the tabs' 18, the page's top line
/// of 2 (brief 109's measurements).
pub const ROW_H: i32 = 24;
/// At most this many tabs in a window; a 33rd is refused.
pub const MOST: usize = 32;
const GAP: i32 = 2;
const TAB_H: i32 = 18;
/// The letters' place in a tab (the property sheets': `properties::TAB_PAD`).
const PAD: i32 = 6;
/// A tab's longest width: the taskbar's rule for a button's longest width (160); its smallest
/// when the row is too long (PROVISIONAL: the old arrows were never captured).
pub const LONGEST: i32 = 160;
pub const SMALLEST: i32 = 48;
/// The X at the row's end: the tree pane's close button's size (98, 2000).
const END_X: (i32, i32) = (20, 17);
/// The X on a tab: its glyph in a box this size, after the name.
const TAB_X: (i32, i32) = (12, 11);
const TAB_X_GAP: i32 = 3;

/// The X's glyph: 98's and 2000's tree pane close button's; 95 its title bar close glyph's
/// shape, alone (PROVISIONAL).
fn glyph(era: Era) -> &'static [&'static str] {
    const PANE: &[&str] = crate::art::rows("tree/close", crate::art::Look::Win98).rows;
    const CAPTION: &[&str] = crate::art::rows("window/close-glyph", crate::art::ANY).rows;
    if era == Era::Win95 { CAPTION } else { PANE }
}

fn glyph_size(era: Era) -> (i32, i32) {
    let g = glyph(era);
    (g.iter().map(|r| r.len() as i32).max().unwrap_or(0), g.len() as i32)
}

/// One tab: an id that never changes (what every command finds it by, never its place in the
/// row), its name, and its whole path (the tooltip of a cut name).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tab {
    pub id: u64,
    pub title: String,
    pub tip: String,
}

/// The row as it shows now.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Row {
    pub tabs: Vec<Tab>,
    /// The tab in front (an index into `tabs`).
    pub front: usize,
    pub close_at: CloseAt,
    /// The X: 0 plain, 1 under the pointer, 2 pressed (as the tree pane's).
    pub close_look: u8,
}

/// Where the row's parts are (window pixels).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Layout {
    /// The whole strip.
    pub strip: Rect,
    /// Each tab's rectangle (an unchosen tab's), `None` where it is out of sight (the row begins
    /// further along).
    pub tabs: Vec<Option<Rect>>,
    /// Each tab's name as shown (cut with three dots where it does not fit).
    pub labels: Vec<String>,
    /// The X.
    pub close: Option<Rect>,
    /// The line under the tabs: from, to.
    pub line: (i32, i32),
}

/// What a point of the row is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    Tab(usize),
    Close,
    Empty,
}

/// Lays the row out in `strip` (`ROW_H` high).
pub fn layout(era: Era, font: &Font, row: &Row, strip: Rect) -> Layout {
    let top = strip.y + GAP + 2;
    let n = row.tabs.len();
    let (gw, _) = glyph_size(era);
    let x0 = strip.x + 2;
    let end_close = (row.close_at == CloseAt::End).then(|| Rect::new(strip.right() - 2 - END_X.0, top + (TAB_H - END_X.1) / 2, END_X.0, END_X.1));
    let limit = end_close.map_or(strip.right() - 2, |c| c.x - 4);
    let avail = (limit - x0).max(0);
    let x_room = |i: usize| if row.close_at == CloseAt::Tab && i == row.front { TAB_X.0.max(gw + 4) + TAB_X_GAP } else { 0 };
    // As wide as the name needs, up to the longest width; then all shrink alike, not below the
    // smallest width.
    let natural: Vec<i32> = row.tabs.iter().enumerate().map(|(i, t)| (font.measure(&t.title) + 2 * PAD + x_room(i)).clamp(SMALLEST, LONGEST)).collect();
    let total = |cap: i32| natural.iter().map(|&w| w.min(cap)).sum::<i32>();
    let mut cap = LONGEST;
    while cap > SMALLEST && total(cap) > avail {
        cap -= 1;
    }
    let widths: Vec<i32> = natural.iter().map(|&w| w.min(cap)).collect();
    // Still too long: the row begins further along, so that the tab in front is whole.
    let mut first = 0;
    if n > 0 {
        let front = row.front.min(n - 1);
        while first < front && widths[first..=front].iter().sum::<i32>() > avail {
            first += 1;
        }
    }
    let mut tabs = vec![None; n];
    let mut labels = vec![String::new(); n];
    let mut x = x0;
    for i in first..n {
        if x + widths[i] > limit && i != first {
            break;
        }
        tabs[i] = Some(Rect::new(x, top, widths[i], TAB_H));
        labels[i] = font.elide(&row.tabs[i].title, widths[i] - 2 * PAD - x_room(i));
        x += widths[i];
    }
    let close = match row.close_at {
        CloseAt::End => end_close,
        CloseAt::Tab => tabs.get(row.front).copied().flatten().map(|t| {
            let w = TAB_X.0.max(gw + 4);
            Rect::new(t.right() - PAD + 2 - w, top - 2 + (TAB_H + 2 - TAB_X.1) / 2, w, TAB_X.1)
        }),
    };
    Layout { strip, tabs, labels, close, line: (strip.x, strip.right()) }
}

impl Layout {
    /// What is at (x, y); `None` outside the strip.
    pub fn hit(&self, row: &Row, x: i32, y: i32) -> Option<Hit> {
        if !self.strip.contains(x, y) {
            return None;
        }
        if self.close.is_some_and(|c| c.contains(x, y)) {
            return Some(Hit::Close);
        }
        // The tab in front stands 2 higher and 2 wider each side.
        if let Some(Some(t)) = self.tabs.get(row.front)
            && Rect::new(t.x - 2, t.y - 2, t.w + 4, t.h + 2).contains(x, y)
        {
            return Some(Hit::Tab(row.front));
        }
        for (i, t) in self.tabs.iter().enumerate() {
            if t.is_some_and(|t| t.contains(x, y)) {
                return Some(Hit::Tab(i));
            }
        }
        Some(Hit::Empty)
    }

    /// Whether tab `i`'s name is cut (its tooltip then shows the whole path).
    pub fn cut(&self, row: &Row, i: usize) -> bool {
        self.labels.get(i).zip(row.tabs.get(i)).is_some_and(|(l, t)| *l != t.title)
    }
}

/// A tab, as `properties::tab` paints it.
fn tab(fb: &mut Framebuffer, theme: &Theme, r: Rect) {
    crate::properties::tab(fb, &theme.colors, r);
}

/// The small flat X in `b`: plain, a raised one-pixel edge under the pointer, a sunken one when
/// pressed (the tree pane's close button; 95 the same, PROVISIONAL).
fn paint_x(fb: &mut Framebuffer, theme: &Theme, era: Era, b: Rect, look: u8) {
    let c = &theme.colors;
    match look {
        1 => fb.frame(b, c.highlight, c.shadow),
        2 => fb.frame(b, c.shadow, c.highlight),
        _ => {}
    }
    let (gw, gh) = glyph_size(era);
    let s = i32::from(look == 2);
    let (ox, oy) = ((b.w - gw) / 2, (b.h - gh) / 2);
    for (r, line) in glyph(era).iter().enumerate() {
        for (k, ch) in line.bytes().enumerate() {
            if ch == b'#' {
                fb.set(b.x + ox + k as i32 + s, b.y + oy + r as i32 + s, c.text);
            }
        }
    }
}

/// Paints the row: its strip in the look's face, the tabs not in front, the line running on to
/// the row's end, the tab in front over it (opening into what is below), the X.
pub fn paint(fb: &mut Framebuffer, theme: &Theme, font: &Font, era: Era, row: &Row, l: &Layout) {
    let c = &theme.colors;
    fb.fill(l.strip, c.face);
    let text = |fb: &mut Framebuffer, i: usize, top: i32| {
        if let Some(Some(t)) = l.tabs.get(i) {
            font.draw(fb, t.x + PAD, top + 3, &l.labels[i], c.text);
        }
    };
    for (i, t) in l.tabs.iter().enumerate() {
        if let Some(t) = t
            && i != row.front
        {
            tab(fb, theme, *t);
            text(fb, i, t.y);
        }
    }
    let ly = l.strip.y + GAP + 2 + TAB_H;
    fb.hline(l.line.0, ly, l.line.1 - l.line.0, c.highlight);
    fb.hline(l.line.0, ly + 1, l.line.1 - l.line.0, c.light);
    if let Some(Some(t)) = l.tabs.get(row.front) {
        let drawn = Rect::new(t.x - 2, t.y - 2, t.w + 4, t.h + 3);
        tab(fb, theme, drawn);
        text(fb, row.front, drawn.y);
    }
    if let Some(b) = l.close {
        paint_x(fb, theme, era, b, row.close_look);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn font() -> Font {
        Font::parse(include_str!("../../tests/fixtures/ms-sans-serif-11px.font")).unwrap()
    }

    fn row(names: &[&str], front: usize, close_at: CloseAt) -> Row {
        Row {
            tabs: names.iter().enumerate().map(|(i, n)| Tab { id: i as u64 + 1, title: n.to_string(), tip: format!("C:\\{n}") }).collect(),
            front,
            close_at,
            close_look: 0,
        }
    }

    #[test]
    fn the_words_go_both_ways() {
        for p in Place::ALL {
            assert_eq!(Place::from_word(p.word()), Some(p));
        }
        for c in CloseAt::ALL {
            assert_eq!(CloseAt::from_word(c.word()), Some(c));
        }
        assert_eq!(Place::default(), Place::Off);
        assert_eq!(CloseAt::default(), CloseAt::End);
        assert_eq!(Place::from_word("bottom"), None);
    }

    #[test]
    fn a_tab_is_as_wide_as_its_name_up_to_the_longest() {
        let f = font();
        let r = row(&["Work", "My Computer", "A very long folder name that goes on and on"], 0, CloseAt::End);
        let l = layout(Era::Win98, &f, &r, Rect::new(4, 100, 552, ROW_H));
        let t: Vec<Rect> = l.tabs.iter().map(|t| t.unwrap()).collect();
        assert_eq!(t[0].w, (f.measure("Work") + 12).max(SMALLEST));
        assert_eq!(t[1].w, f.measure("My Computer") + 12);
        assert_eq!(t[2].w, LONGEST);
        assert!(l.labels[2].ends_with("..."), "{}", l.labels[2]);
        assert!(l.cut(&r, 2) && !l.cut(&r, 1));
        assert_eq!(t[1].x, t[0].right());
        // The X at the end, the tabs' tops 4 below the strip's.
        assert_eq!(l.close, Some(Rect::new(4 + 552 - 2 - 20, 104, 20, 17)));
        assert_eq!(t[0].y, 104);
    }

    #[test]
    fn a_long_row_shrinks_alike_then_begins_further_along() {
        let f = font();
        let names: Vec<String> = (0..12).map(|i| format!("Folder number {i}")).collect();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        // Shrinks alike: every tab the same width, all in view.
        let r = row(&refs[..6], 0, CloseAt::End);
        let l = layout(Era::Win98, &f, &r, Rect::new(0, 0, 400, ROW_H));
        let w: Vec<i32> = l.tabs.iter().map(|t| t.unwrap().w).collect();
        assert!(w.iter().all(|&x| x == w[0]) && w[0] < f.measure("Folder number 0") + 12, "{w:?}");
        assert!(l.tabs.iter().all(|t| t.unwrap().right() <= l.close.unwrap().x));
        // Too many even at the smallest width: the row begins so that the tab in front is whole.
        let r = row(&refs, 11, CloseAt::End);
        let l = layout(Era::Win98, &f, &r, Rect::new(0, 0, 400, ROW_H));
        assert!(l.tabs[0].is_none(), "the first ones out of sight");
        let front = l.tabs[11].unwrap();
        assert_eq!(front.w, SMALLEST);
        assert!(front.right() <= l.close.unwrap().x, "whole, the X in view");
        // The front at the start: the row from the first.
        let r = row(&refs, 0, CloseAt::End);
        let l = layout(Era::Win98, &f, &r, Rect::new(0, 0, 400, ROW_H));
        assert!(l.tabs[0].is_some() && l.tabs[11].is_none());
    }

    /// The folder window with a row: its parts are found by the tab's id, a press brings a tab
    /// forward at once, the X acts at its release, Ctrl+T/W/Tab and a double-click on the row's
    /// empty part ask for a new tab, closing, the next and the previous; with `Off` none of it.
    #[test]
    fn the_window_with_a_row() {
        use crate::folder::{FolderWindow, Part};
        use crate::kit::{Effect, Form, Key, Mods, Press};
        let (r, b) = (font(), Font::parse(include_str!("../../tests/fixtures/ms-sans-serif-11px-bold.font")).unwrap());
        let fonts = crate::text::Fonts { regular: &r, bold: &b };
        let icon = crate::fb::Icon { width: 16, height: 16, pixels: vec![0; 256] };
        for era in [Era::Win95, Era::Win98, Era::Win2000] {
            for place in [Place::UnderAddress, Place::OverFiles, Place::UnderMenu, Place::AboveMenu] {
                let mut w = FolderWindow::new(era, "Work", icon.clone(), true, (560, 380));
                w.layout(&fonts);
                let row = Row { tabs: vec![Tab { id: 7, title: "Work".into(), tip: "C:\\Work".into() }, Tab { id: 9, title: "Maps".into(), tip: "C:\\Maps".into() }], front: 0, close_at: CloseAt::End, close_look: 0 };
                w.set_tabs(place, row);
                let l = w.tab_layout().expect("a row");
                let maps = l.tabs[1].unwrap();
                assert_eq!(w.hit(maps.x + 4, maps.y + 8), Some(Part::Tab(9)), "{era:?} {place:?}");
                assert_eq!(w.press(Some(Part::Tab(9)), maps.x + 4, maps.y + 8), Press::Act(Part::Tab(9)));
                let x = l.close.unwrap();
                assert_eq!(w.hit(x.x + 3, x.y + 3), Some(Part::TabClose));
                assert_eq!(w.press(Some(Part::TabClose), x.x + 3, x.y + 3), Press::Hold(Part::TabClose));
                let ctrl = Mods { ctrl: true, shift: false };
                assert_eq!(w.key(Key::Other(0x54), ctrl), Some(vec![Effect::Activate(Part::TabNew)]));
                assert_eq!(w.key(Key::Other(0x57), ctrl), Some(vec![Effect::Activate(Part::TabClose)]));
                assert_eq!(w.key(Key::Tab, ctrl), Some(vec![Effect::Activate(Part::TabNext)]));
                assert_eq!(w.key(Key::Tab, Mods { ctrl: true, shift: true }), Some(vec![Effect::Activate(Part::TabPrev)]));
                let empty = (l.strip.right() - 40, l.strip.y + 6);
                assert_eq!(w.hit(empty.0, empty.1), Some(Part::TabRow));
                assert_eq!(w.double_click(empty.0, empty.1), Some(vec![Effect::Activate(Part::TabNew)]));
                // Another tab's window takes the window's own state (its size, the row).
                let mut other = FolderWindow::new(era, "Maps", icon.clone(), true, (300, 200));
                other.take_window_from(&w);
                assert_eq!(other.size_now(), (560, 380));
                assert_eq!(other.tab_place(), place);
                // Off: no row, no keys.
                w.set_tabs(Place::Off, Row::default());
                assert!(w.tab_layout().is_none());
                assert_ne!(w.key(Key::Other(0x54), ctrl), Some(vec![Effect::Activate(Part::TabNew)]));
            }
        }
    }

    #[test]
    fn the_x_on_the_tab_in_front() {
        let f = font();
        let r = row(&["Work", "Maps"], 1, CloseAt::Tab);
        let l = layout(Era::Win2000, &f, &r, Rect::new(0, 0, 500, ROW_H));
        let (a, b) = (l.tabs[0].unwrap(), l.tabs[1].unwrap());
        let x = l.close.unwrap();
        assert!(x.x > b.x + 6 + f.measure("Maps") && x.right() <= b.right(), "right of its name, inside the tab");
        assert!(b.w > a.w, "room for the X on the tab in front only");
        assert_eq!(l.hit(&r, x.x + 2, x.y + 2), Some(Hit::Close));
        assert_eq!(l.hit(&r, a.x + 3, a.y + 5), Some(Hit::Tab(0)));
        assert_eq!(l.hit(&r, 490, 10), Some(Hit::Empty));
        assert_eq!(l.hit(&r, 10, 30), None);
    }
}
