//! Window and navigation controls shared by the classic HTML Help books.

use super::Tab;
use super::icons::{self, ToolIcon, TreeIcon};
use crate::dialog;
use crate::fb::{Framebuffer, Icon, Rect};
use crate::help::EntryIcon;
use crate::properties;
use crate::text::{Font, Fonts};
use crate::theme::{Era, Theme};
use crate::window::{self, ButtonLook, Frame, TitleButton};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    Icon,
    Minimize,
    Maximize,
    Close,
    NavToggle,
    Back,
    Forward,
    Options,
    WebHelp,
    Tab(Tab),
    Query,
    Row(usize),
    Display,
    ListTopics,
    Splitter,
    Article,
    ScrollUp,
    ScrollDown,
    ScrollTrack,
    ScrollThumb,
    NavUp,
    NavDown,
    NavTrack,
    NavThumb,
    /// The Contents tree's horizontal scroll bar (shown while a row is wider than the tree).
    NavLeft,
    NavRight,
    NavHTrack,
    NavHThumb,
}

pub(super) struct Row<'a> {
    pub label: &'a str,
    pub depth: usize,
    pub icon: EntryIcon,
    pub open: bool,
}

/// Row numbers index the entire list, including the part above the first visible row.
pub(super) struct Chrome<'a> {
    pub title: &'a str,
    pub active: bool,
    pub maximized: bool,
    pub keys: bool,
    pub back: bool,
    pub forward: bool,
    pub pressed: Option<Hit>,
    pub hover: Option<Hit>,
    pub focus: Option<Hit>,
    pub rows: &'a [Row<'a>],
    pub first: usize,
    pub selected: Option<usize>,
    pub query: &'a str,
    /// UTF-8 boundary of the insertion point.
    pub caret: usize,
    pub selection: Option<(usize, usize)>,
    pub blink: bool,
    pub scroll: i32,
    pub document_height: i32,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Layout {
    era: Era,
    size: (i32, i32),
    nav: bool,
    tab: Tab,
    page: Rect,
    pub article: Rect,
    pub scrollbar: Rect,
    pub splitter: Rect,
    pub list: Rect,
    pub query: Rect,
    pub display: Rect,
    pub list_topics: Rect,
    pub row_height: i32,
    /// The Contents tree: how many rows it shows, how far its widest row reaches (the right
    /// edge of that row's selection mark: what Windows' tree view scrolls to), and how far it
    /// is scrolled to the left. Zero on the other tabs.
    pub tree_rows: usize,
    pub tree_width: i32,
    pub tree_left: i32,
}

pub(super) fn native_size(era: Era) -> (i32, i32) {
    (if era == Era::Win98 { 525 } else { 550 }, 450)
}

/// Position of the splitter in the unscaled window.
pub(super) fn native_split(era: Era) -> i32 {
    if era == Era::Win98 { 224 } else { 254 }
}

pub(super) fn icon(era: Era) -> Icon {
    icons::caption(era)
}

pub(super) fn control_font<'a>(fonts: &Fonts<'a>, era: Era) -> &'a Font {
    match era {
        Era::Win2000 => crate::text::ms_sans_serif(),
        Era::Win98 => super::fonts::tree98(),
        _ => fonts.regular,
    }
}

pub(super) fn layout(era: Era, size: (i32, i32), split: i32, nav: bool, tab: Tab) -> Layout {
    let old = era == Era::Win98;
    let (w, h) = size;
    let split = split.clamp(132, (w - 75).max(132));
    let split_w = if old { 7 } else { 4 };
    let left = if nav { split + split_w } else { 4 };
    let doc_frame = Rect::new(left, 63, w - left - 4, h - 67);
    let scrollbar = Rect::new(doc_frame.right() - 18, 65, 16, doc_frame.h - 4);
    let article = Rect::new(left + 2, 65, (scrollbar.x - left - 2).max(1), scrollbar.h);
    let splitter = Rect::new(split, 63, split_w, h - 67);
    let page = if old { Rect::new(4, 84, split - 4, h - 88) } else { Rect::new(4, 91, split - 8, h - 95) };
    let x = if old { 9 } else { 13 };
    let (top, pad) = if old { (87, 2) } else { (98, 13) };
    let list_w = split - pad - x;
    let mut list = Rect::new(x, top, list_w, h - if old { 6 } else { 13 } - top);
    let mut query = Rect::default();
    let mut display = Rect::default();
    let mut list_topics = Rect::default();
    if tab != Tab::Contents {
        let idx = tab == Tab::Index;
        let qy = if old && idx {
            87
        } else if old {
            112
        } else {
            121
        };
        query = Rect::new(x, qy, list_w, if old { 22 } else { 19 });
        let list_y = match (old, idx) {
            (true, true) => 118,
            (true, false) => 193,
            (false, true) => 150,
            (false, false) => 204,
        };
        let bottom = if old { h - if idx { 37 } else { 36 } } else { h - 52 };
        list = Rect::new(x, list_y, list_w, bottom - list_y);
        let bw = if old { 75 } else { 78 };
        display = Rect::new(x + list_w - bw + 1, h - if old { if idx { 30 } else { 33 } } else { 43 }, bw, 23);
        let (bw, yy) = if old { (96, 142) } else { (95, 149) };
        list_topics = Rect::new(x + list_w - bw + 1, yy, bw, 23);
    }
    Layout {
        era,
        size,
        nav,
        tab,
        page,
        article,
        scrollbar,
        splitter,
        list,
        query,
        display,
        list_topics,
        row_height: match tab {
            Tab::Contents => 16,
            Tab::Index => {
                if old {
                    14
                } else {
                    13
                }
            }
            Tab::Search => {
                if old {
                    15
                } else {
                    14
                }
            }
        },
        tree_rows: 0,
        tree_width: 0,
        tree_left: 0,
    }
}

/// The font of the navigation lists (98 and 2000; the 95 look has no HTML Help).
pub(super) fn tree_font(era: Era) -> &'static Font {
    if era == Era::Win98 { super::fonts::tree98() } else { crate::text::ms_sans_serif() }
}

impl Layout {
    /// Windows 98 restores the navigation pane with a six-pixel splitter and keeps the
    /// wider browser child. Its last four scrollbar columns fall outside the client area.
    pub(super) fn restored_navigation(&mut self) {
        if self.era == Era::Win98 && self.nav {
            self.splitter.w -= 1;
            self.article.x -= 1;
            self.article.w += 7;
            self.scrollbar.x += 6;
        }
    }

    fn rows_rect(&self) -> Rect {
        if self.tab == Tab::Search {
            let header = if self.era == Era::Win98 { 18 } else { 2 };
            self.list.inset(4, 2 + header, 2, 2)
        } else if self.tree_bars().1 {
            self.list.inset(2, 2, 2, 18)
        } else {
            self.list.inset(2, 2, 2, 2)
        }
    }

    /// The Contents tree's scroll bars (vertical, horizontal), as Windows' tree view decides
    /// them: a horizontal bar takes 16 rows from the height, a vertical one 16 columns.
    fn tree_bars(&self) -> (bool, bool) {
        if self.tab != Tab::Contents || self.tree_width == 0 {
            return (false, false);
        }
        let inner = self.list.inset(2, 2, 2, 2);
        let fits = |h: i32| (h / self.row_height).max(1) as usize;
        let vertical = self.tree_rows > fits(inner.h);
        let horizontal = self.tree_width > inner.w - if vertical { 16 } else { 0 };
        let vertical = vertical || (horizontal && self.tree_rows > fits(inner.h - 16));
        (vertical, horizontal || (vertical && self.tree_width > inner.w - 16))
    }

    /// How wide the tree's rows show (without a vertical scroll bar).
    pub fn tree_page(&self) -> i32 {
        self.rows_rect().w - if self.tree_bars().0 { 16 } else { 0 }
    }

    /// How far the tree can scroll to the left.
    pub fn tree_max_left(&self) -> i32 {
        if self.tree_bars().1 { (self.tree_width - self.tree_page()).max(0) } else { 0 }
    }

    pub fn visible_rows(&self) -> usize {
        (self.rows_rect().h / self.row_height).max(1) as usize
    }

    pub fn rect(&self, hit: Hit, first: usize, count: usize, scroll: i32, document_height: i32) -> Option<Rect> {
        let (w, _) = self.size;
        let r = match hit {
            Hit::Icon => Rect::new(6, 5, 16, 16),
            Hit::Close | Hit::Minimize | Hit::Maximize => {
                let x = match hit {
                    Hit::Close => w - 22,
                    Hit::Minimize => w - 56,
                    _ => w - 40,
                };
                Rect::new(x, 6, 16, 14)
            }
            Hit::NavToggle | Hit::Back | Hit::Forward | Hit::Options | Hit::WebHelp => {
                let i = match hit {
                    Hit::NavToggle => 0,
                    Hit::Back => 1,
                    Hit::Forward => 2,
                    Hit::Options => 3,
                    _ => 4,
                };
                Rect::new(4 + i * 55, 23, 55, if self.era == Era::Win98 { 36 } else { 40 })
            }
            Hit::Splitter if self.nav => self.splitter,
            Hit::Article => self.article,
            Hit::ScrollUp => Rect::new(self.scrollbar.x, self.scrollbar.y, 16, 16),
            Hit::ScrollDown => Rect::new(self.scrollbar.x, self.scrollbar.bottom() - 16, 16, 16),
            Hit::ScrollTrack => self.scrollbar.inset(0, 16, 0, 16),
            Hit::ScrollThumb if document_height > self.article.h => {
                let track = self.scrollbar.inset(0, 16, 0, 16);
                if track.h < 8 {
                    return None;
                }
                let h = (track.h as i64 * self.article.h as i64 / document_height as i64) as i32;
                let h = h.clamp(8, track.h);
                let max = document_height - self.article.h;
                let y = track.y + (i64::from(track.h - h) * i64::from(scroll.clamp(0, max)) / i64::from(max)) as i32;
                Rect::new(track.x, y, track.w, h)
            }
            _ if !self.nav => return None,
            Hit::Tab(t) => self.tab_rect(t),
            Hit::Query if self.tab != Tab::Contents => self.query,
            Hit::Display if self.tab != Tab::Contents => self.display,
            Hit::ListTopics if self.tab == Tab::Search => self.list_topics,
            Hit::Row(i) if i >= first && i < count => {
                let mut inner = self.rows_rect();
                if self.rect(Hit::NavTrack, first, count, scroll, document_height).is_some() {
                    inner.w -= 16;
                }
                let dy = (i - first).checked_mul(self.row_height as usize)?;
                if dy >= inner.h.max(0) as usize {
                    return None;
                }
                let y = inner.y + dy as i32;
                Rect::new(inner.x, y, inner.w, self.row_height.min(inner.bottom() - y))
            }
            Hit::NavUp | Hit::NavDown | Hit::NavTrack | Hit::NavThumb if count > self.visible_rows() => {
                let inner = self.rows_rect();
                let track = Rect::new(inner.right() - 16, inner.y + 16, 16, inner.h - 32);
                if track.h < 8 {
                    return None;
                }
                match hit {
                    Hit::NavUp => Rect::new(track.x, inner.y, 16, 16),
                    Hit::NavDown => Rect::new(track.x, inner.bottom() - 16, 16, 16),
                    Hit::NavTrack => track,
                    _ => {
                        let visible = self.visible_rows();
                        let h = ((track.h as usize * visible + count / 2) / count).max(8) as i32;
                        let max = count - visible;
                        let y = track.y + (((track.h - h) as usize * first.min(max) + max / 2) / max) as i32;
                        Rect::new(track.x, y, 16, h)
                    }
                }
            }
            Hit::NavLeft | Hit::NavRight | Hit::NavHTrack | Hit::NavHThumb if self.tree_bars().1 => {
                let inner = self.rows_rect();
                let bar = Rect::new(inner.x, inner.bottom(), self.tree_page(), 16);
                let track = bar.inset(16, 0, 16, 0);
                if track.w < 8 {
                    return None;
                }
                match hit {
                    Hit::NavLeft => Rect::new(bar.x, bar.y, 16, 16),
                    Hit::NavRight => Rect::new(bar.right() - 16, bar.y, 16, 16),
                    Hit::NavHTrack => track,
                    _ => {
                        // Windows' scroll bar: the thumb is the page's share of the track,
                        // rounded (MulDiv), at least 8 long.
                        let (page, extent) = (i64::from(bar.w), i64::from(self.tree_width));
                        let w = ((i64::from(track.w) * page + extent / 2) / extent) as i32;
                        let w = w.clamp(8, track.w);
                        let max = self.tree_max_left().max(1);
                        let x = track.x + (track.w - w) * self.tree_left.clamp(0, max) / max;
                        Rect::new(x, track.y, w, 16)
                    }
                }
            }
            _ => return None,
        };
        Some(r)
    }

    pub fn hit(&self, x: i32, y: i32, first: usize, count: usize, scroll: i32, document_height: i32) -> Option<Hit> {
        if x < 4 || x >= self.size.0 - 4 {
            return None;
        }
        if self.nav && self.rows_rect().contains(x, y) {
            let index = first + (y - self.rows_rect().y) as usize / self.row_height as usize;
            if index < count
                && self.rect(Hit::Row(index), first, count, scroll, document_height).is_some_and(|r| r.contains(x, y))
            {
                return Some(Hit::Row(index));
            }
        }
        let parts = [
            Hit::Close,
            Hit::Minimize,
            Hit::Maximize,
            Hit::Icon,
            Hit::NavToggle,
            Hit::Back,
            Hit::Forward,
            Hit::Options,
            Hit::WebHelp,
            Hit::Tab(self.tab),
            Hit::Tab(Tab::Contents),
            Hit::Tab(Tab::Index),
            Hit::Tab(Tab::Search),
            Hit::Query,
            Hit::Display,
            Hit::ListTopics,
            Hit::NavUp,
            Hit::NavDown,
            Hit::NavThumb,
            Hit::NavTrack,
            Hit::NavLeft,
            Hit::NavRight,
            Hit::NavHThumb,
            Hit::NavHTrack,
            Hit::Splitter,
            Hit::ScrollUp,
            Hit::ScrollDown,
            Hit::ScrollThumb,
            Hit::ScrollTrack,
            Hit::Article,
        ];
        parts
            .into_iter()
            .find(|&p| self.rect(p, first, count, scroll, document_height).is_some_and(|r| r.contains(x, y)))
    }

    fn tab_rect(&self, t: Tab) -> Rect {
        let old = self.era == Era::Win98;
        let (x, w) = match (old, t) {
            (true, Tab::Contents) => (6, 55),
            (true, Tab::Index) => (61, 42),
            (true, Tab::Search) => (103, 47),
            (false, Tab::Contents) => (6, 54),
            (false, Tab::Index) => (60, 42),
            (false, Tab::Search) => (102, 46),
        };
        let y = if old { 65 } else { 73 };
        Rect::new(x, y, w, self.page.y - y)
    }
}

pub(super) fn paint(fb: &mut Framebuffer, t: &Theme, fonts: &Fonts, l: &Layout, v: &Chrome) {
    let c = &t.colors;
    let (w, h) = l.size;
    let outer = Rect::new(0, 0, w, h);
    fb.fill(outer, c.face);
    fb.frame(outer, c.light, c.dark_shadow);
    fb.frame(outer.inset(1, 1, 1, 1), c.highlight, c.shadow);
    // Resizable windows inset the system caption one extra pixel.
    let mut strip = Framebuffer::new(w - 2, 26, c.face);
    let buttons = [
        (TitleButton::Minimize, Hit::Minimize),
        (TitleButton::Maximize, Hit::Maximize),
        (TitleButton::Close, Hit::Close),
    ]
    .map(|(button, hit)| ButtonLook {
        button,
        rect: window::title_button_rect(w - 2, button),
        pressed: v.pressed == Some(hit),
        enabled: true,
    });
    window::paint_frame(
        &mut strip,
        t,
        fonts,
        &Frame { era: l.era, title: v.title, icon: Some(&icon(l.era)), active: v.active, buttons: &buttons },
    );
    for y in 3..21 {
        let start = (y * strip.width + 3) as usize;
        let dest = ((y + 1) * w + 4) as usize;
        let len = (w - 8) as usize;
        fb.pixels[dest..dest + len].copy_from_slice(&strip.pixels[start..start + len]);
    }
    if v.maximized {
        let r = l.rect(Hit::Maximize, 0, 0, 0, 0).unwrap();
        let s = i32::from(v.pressed == Some(Hit::Maximize));
        fb.fill(r.inset(2, 2, 2, 2), c.face);
        for (dx, dy) in [(6, 2), (3, 5)] {
            let b = Rect::new(r.x + dx + s, r.y + dy + s, 7, 6);
            fb.fill(b, c.face);
            fb.frame(b, c.text, c.text);
            fb.hline(b.x, b.y + 1, b.w, c.text);
        }
    }
    toolbar(fb, t, fonts, l, v);
    if l.nav {
        nav_panel(fb, t, fonts, l, v);
    }
    let body = Rect::new(l.article.x - 2, l.article.y - 2, l.scrollbar.right() - l.article.x + 4, l.article.h + 4);
    dialog::well(fb, body, c);
    scroll_bar(fb, t, l, v);
    if body.right() > w - 4 {
        for (i, color) in [c.face, c.face, c.shadow, c.dark_shadow].into_iter().enumerate() {
            fb.vline(w - 4 + i as i32, body.y, body.h, color);
        }
    }
}

fn toolbar(fb: &mut Framebuffer, t: &Theme, fonts: &Fonts, l: &Layout, v: &Chrome) {
    let c = &t.colors;
    let old = l.era == Era::Win98;
    let toggle = if l.nav { (ToolIcon::Hide, "Hide") } else { (ToolIcon::Show, "Show") };
    let buttons = [
        (Hit::NavToggle, toggle, true),
        (Hit::Back, (ToolIcon::Back, "Back"), v.back || old),
        (Hit::Forward, (ToolIcon::Forward, "Forward"), v.forward || old),
        (Hit::Options, (ToolIcon::Options, "&Options"), true),
        (Hit::WebHelp, (ToolIcon::WebHelp, "Web Help"), true),
    ];
    for (i, (p, (tool, label), enabled)) in buttons.into_iter().enumerate() {
        let r = l.rect(p, 0, 0, 0, 0).unwrap();
        let pressed = v.pressed == Some(p);
        let hot = enabled && (pressed || v.hover == Some(p));
        if hot {
            let (tl, br) = if pressed { (c.shadow, c.highlight) } else { (c.highlight, c.shadow) };
            fb.frame(r, tl, br);
        }
        let s = i32::from(pressed);
        let image = icons::toolbar(l.era, tool, enabled, hot);
        crate::colors16::draw_picture(fb, t, &image, (if old { 22 } else { 21 }) + 55 * i as i32 + s, 26 + s);
        let font = if old { fonts.regular } else { crate::text::ms_sans_serif() };
        let x = r.x + (r.w - font.measure_label(label)) / 2 + s;
        let y = if old { 43 } else { 47 };
        if !enabled {
            font.draw_label(fb, (x + 1, y + s + 1), label, r.w, c.etch, v.keys);
        }
        font.draw_label(fb, (x, y + s), label, r.w, if enabled { c.text } else { c.shadow }, v.keys);
    }
}

// The Win98 HTML Help component uses an Arial TrueType underline at the baseline + 1.
fn nav_label(fb: &mut Framebuffer, font: &Font, (x, y): (i32, i32), label: &str, color: u32, keys: bool, old: bool) {
    font.draw_label(fb, (x, y), label, i32::MAX, color, keys && !old);
    if old && keys {
        let (plain, key) = crate::text::split_access_key(label);
        if let Some(index) = key.and_then(|k| plain.char_indices().nth(k)) {
            fb.hline(x + font.measure(&plain[..index.0]), y + font.height - 2, font.glyph(index.1).advance, color);
        }
    }
}

fn nav_panel(fb: &mut Framebuffer, t: &Theme, fonts: &Fonts, l: &Layout, v: &Chrome) {
    let c = &t.colors;
    let old = l.era == Era::Win98;
    let split = l.splitter;
    fb.hline(split.x, split.y, split.w, c.shadow);
    fb.hline(split.x + 1, split.y + 1, split.w - 1, c.highlight);
    fb.vline(split.x, split.y, split.h, c.shadow);
    fb.vline(split.x + 1, split.y + 1, split.h - 1, c.highlight);
    if !old {
        fb.hline(4, 63, split.x - 4, c.shadow);
        fb.hline(4, 64, split.x - 4, c.highlight);
        fb.hline(split.x, split.bottom() - 2, split.w, c.shadow);
        fb.hline(split.x, split.bottom() - 1, split.w, c.highlight);
    }
    let font = control_font(fonts, l.era);
    let name = |tab: Tab| match tab {
        Tab::Contents => "&Contents",
        Tab::Index if old => "&Index",
        Tab::Index => "I&ndex",
        Tab::Search => "&Search",
    };
    for tab in [Tab::Contents, Tab::Index, Tab::Search] {
        if tab == l.tab {
            continue;
        }
        let r = l.tab_rect(tab);
        properties::tab(fb, c, r);
        nav_label(fb, font, (r.x + 6, r.y + if old { 1 } else { 3 }), name(tab), c.text, v.keys, old);
    }
    if old {
        fb.hline(l.page.x, l.page.y, l.page.w, c.highlight);
        fb.hline(l.page.x + 1, l.page.y + 1, l.page.w - 1, c.light);
        fb.vline(l.page.x, l.page.y, l.page.h, c.highlight);
        fb.vline(l.page.x + 1, l.page.y + 1, l.page.h - 1, c.light);
    } else {
        dialog::raised(fb, l.page, c);
    }
    let r = l.tab_rect(l.tab);
    let active = r.inset(-2, -2, -2, -2);
    properties::tab(fb, c, active);
    let x = active.x.max(l.page.x + 2);
    fb.hline(x, l.page.y + 1, active.right() - x, c.face);
    nav_label(fb, font, (r.x + 6, active.y + if old { 4 } else { 3 }), name(l.tab), c.text, v.keys, old);
    if v.focus == Some(Hit::Tab(l.tab)) {
        dialog::focus_dots(fb, r.inset(1, 1, 1, 1), (0, 0), c.text);
    }
    if l.tab != Tab::Contents {
        if !old || l.tab == Tab::Search {
            nav_label(
                fb,
                font,
                (l.query.x - 1, l.query.y - 24),
                if old { "Type in the &keyword to find:" } else { "Type in the key&word to find:" },
                c.text,
                v.keys,
                old,
            );
        }
        let qr = dialog::well(fb, l.query, c);
        let mut input = Framebuffer::new(qr.w, qr.h, c.highlight);
        let before = v.query.get(..v.caret).unwrap_or(v.query);
        let caret = font.measure(before);
        let x = (if old { 3 } else { 1 }).min(input.width - 1 - caret);
        font.draw(&mut input, x, 1, v.query, c.text);
        if let Some((a, b)) = v.selection.filter(|(a, b)| a != b) {
            let (a, b) = (a.min(b), a.max(b));
            if let (Some(prefix), Some(selected)) = (v.query.get(..a), v.query.get(a..b)) {
                let sx = x + font.measure(prefix);
                input.fill(Rect::new(sx, 1, font.measure(selected), font.height), c.menu_highlight);
                font.draw(&mut input, sx, 1, selected, c.menu_highlight_text);
            }
        } else if v.blink && v.focus == Some(Hit::Query) {
            input.vline(x + caret, 1, font.height, c.text);
        }
        fb.blit(&input, qr.x, qr.y);
        let button = |fb: &mut Framebuffer, r: Rect, label: &str, p, enabled| {
            let pressed = v.pressed == Some(p);
            dialog::push_button(fb, t, font, r, if old { "" } else { label }, false, pressed, false, enabled, v.keys);
            if v.keys && v.focus == Some(p) {
                dialog::focus_dots(fb, r.inset(3, 3, 3, 3), (r.x, r.y), c.text);
            }
            if old {
                let shift = i32::from(pressed);
                let x = r.x + (r.w - font.measure_label(label)) / 2 + shift;
                let y = r.y + (r.h - font.height) / 2 + shift;
                nav_label(fb, font, (x, y), label, c.text, v.keys, true);
            }
        };
        button(fb, l.display, "&Display", Hit::Display, old || v.selected.is_some());
        if l.tab == Tab::Search {
            button(fb, l.list_topics, "&List Topics", Hit::ListTopics, old || !v.query.is_empty());
            nav_label(
                fb,
                font,
                (l.list.x - 1, l.list.y - if old { 26 } else { 24 }),
                if old { "Select To&pic to display:" } else { "Select &Topic to display:" },
                c.text,
                v.keys,
                old,
            );
        }
    }
    nav_list(fb, t, fonts, l, v);
}

fn nav_list(fb: &mut Framebuffer, t: &Theme, fonts: &Fonts, l: &Layout, v: &Chrome) {
    let c = &t.colors;
    let font = control_font(fonts, l.era);
    dialog::well(fb, l.list, c);
    let mut inner = l.rows_rect();
    let nav_scroll = l.rect(Hit::NavTrack, v.first, v.rows.len(), 0, 0).is_some();
    if nav_scroll {
        inner.w -= 16;
    }
    if l.tab == Tab::Search && l.era == Era::Win98 {
        let r = Rect::new(l.list.x + 2, inner.y - 18, l.list.w - 3, 18);
        fb.fill(r, c.face);
        dialog::raised(fb, r, c);
        fb.vline(r.right() - 1, r.y, r.h, c.light);
        font.draw(fb, r.x + 6, r.y + 2, "Topic", c.text);
    }
    let mut canvas = Framebuffer::new(inner.w.max(0), inner.h.max(0), c.highlight);
    let tree = l.tab == Tab::Contents;
    for (i, row) in v.rows.iter().enumerate().skip(v.first) {
        let y = (i - v.first) as i32 * l.row_height;
        if y >= inner.h {
            break;
        }
        if tree && row.depth > (canvas.width / 19) as usize {
            continue;
        }
        let x = if tree { 19 + 19 * row.depth as i32 - l.tree_left } else { 0 };
        if tree {
            let tree_icon = match row.icon {
                EntryIcon::Book if row.open => TreeIcon::OpenBook,
                EntryIcon::Book => TreeIcon::ClosedBook,
                EntryIcon::Overview => TreeIcon::Overview,
                EntryIcon::Page => TreeIcon::Topic,
            };
            crate::colors16::draw_picture(&mut canvas, t, &icons::tree(l.era, tree_icon), x - 19, y);
        }
        let selected = v.selected == Some(i);
        let row_focus = v.active && v.focus == Some(Hit::Row(i));
        let inactive_selection = l.tab != Tab::Index && !row_focus;
        if selected {
            let w = if tree { font.measure(row.label) + 4 } else { inner.w };
            let mark = Rect::new(x, y, w, l.row_height);
            canvas.fill(mark, if inactive_selection { c.face } else { c.menu_highlight });
            if tree && row_focus {
                dialog::focus_dots(&mut canvas, mark, (0, 0), c.text);
                dialog::focus_dots(&mut canvas, mark, (1, 0), crate::theme::rgb(!c.menu_highlight));
            }
        }
        let white = selected && !inactive_selection;
        font.draw(
            &mut canvas,
            x + 2,
            y + if tree { 1 } else { 0 },
            row.label,
            if white { c.menu_highlight_text } else { c.text },
        );
    }
    fb.blit(&canvas, inner.x, inner.y);
    if nav_scroll {
        let rect = |p| l.rect(p, v.first, v.rows.len(), 0, 0).unwrap();
        scroll_track(fb, t, rect(Hit::NavTrack), Some(rect(Hit::NavThumb)));
        for p in [Hit::NavUp, Hit::NavDown] {
            properties::scroll_arrow(fb, c, rect(p), p == Hit::NavUp, v.pressed == Some(p));
        }
    }
    let rect = |p| l.rect(p, v.first, v.rows.len(), 0, 0);
    if let (Some(track), Some(left), Some(right)) = (rect(Hit::NavHTrack), rect(Hit::NavLeft), rect(Hit::NavRight)) {
        // The pattern starts one pixel later than the vertical bars' (the PCs' captures).
        fb.checker(track, (track.x + 1, track.y), c.face, c.highlight);
        if let Some(thumb) = rect(Hit::NavHThumb) {
            fb.fill(thumb, c.face);
            fb.frame(thumb, c.light, c.dark_shadow);
            fb.frame(thumb.inset(1, 1, 1, 1), c.highlight, c.shadow);
        }
        for (r, toward_left, p) in [(left, true, Hit::NavLeft), (right, false, Hit::NavRight)] {
            side_arrow(fb, c, r, toward_left, v.pressed == Some(p));
        }
        if nav_scroll {
            // The square where the two bars meet.
            fb.fill(Rect::new(right.right(), right.y, 16, 16), c.face);
        }
    }
}

/// A scroll bar's left or right button (`properties::scroll_arrow` turned on its side).
fn side_arrow(fb: &mut Framebuffer, c: &crate::theme::Colors, r: Rect, left: bool, pressed: bool) {
    fb.fill(r, c.face);
    if pressed {
        fb.frame(r, c.shadow, c.shadow);
    } else {
        fb.frame(r, c.light, c.dark_shadow);
        fb.frame(r.inset(1, 1, 1, 1), c.highlight, c.shadow);
    }
    let s = i32::from(pressed);
    for col in 0..4 {
        let h = if left { 2 * col + 1 } else { 7 - 2 * col };
        // Measured: the left arrow's tip at x 5, the right arrow's back at x 6.
        fb.vline(r.x + if left { 5 } else { 6 } + col + s, r.y + 7 - h / 2 + s, h, c.text);
    }
}

fn scroll_track(fb: &mut Framebuffer, t: &Theme, track: Rect, thumb: Option<Rect>) {
    let c = &t.colors;
    fb.checker(track, (track.x, track.y), c.face, c.highlight);
    if let Some(thumb) = thumb {
        fb.fill(thumb, c.face);
        fb.frame(thumb, c.light, c.dark_shadow);
        fb.frame(thumb.inset(1, 1, 1, 1), c.highlight, c.shadow);
    }
}

fn scroll_bar(fb: &mut Framebuffer, t: &Theme, l: &Layout, v: &Chrome) {
    let c = &t.colors;
    let enabled = v.document_height > l.article.h;
    scroll_track(fb, t, l.scrollbar.inset(0, 16, 0, 16), l.rect(Hit::ScrollThumb, 0, 0, v.scroll, v.document_height));
    for (p, up) in [(Hit::ScrollUp, true), (Hit::ScrollDown, false)] {
        let r = l.rect(p, 0, 0, 0, 0).unwrap();
        properties::scroll_arrow(fb, c, r, up, enabled && v.pressed == Some(p));
        if !enabled {
            for shift in [1, 0] {
                for row in 0..4 {
                    let width = if up { 2 * row + 1 } else { 7 - 2 * row };
                    fb.hline(
                        r.x + 7 - width / 2 + shift,
                        r.y + 6 + row + shift,
                        width,
                        if shift == 1 { c.highlight } else { c.shadow },
                    );
                }
            }
        }
    }
}
