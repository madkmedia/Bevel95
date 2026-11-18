//! The open list of a drop-down list (Windows' combo box), as Windows 95, 98 and 2000 drew it:
//! a 1px black frame, white rows of 13px with each entry 2px in, the chosen one highlighted, and
//! a 16px scroll bar on the right. Measured on the Time Zone page's list and on the Run box's
//! list of earlier commands (docs/theme-spec.md, "Time Zone page" and "Run's list").
//!
//! `DropList` is where the open list is; `Shown` is what it shows (its first row and the
//! highlighted entry) and the rules the Run box's list follows: opened on the entry that starts
//! with what the field holds, kept in view, scrolled by its scroll bar and the wheel.

use crate::datetime::{draw_clipped, invert_dots};
use crate::fb::{Framebuffer, Rect};
use crate::properties::scroll_arrow;
use crate::scroll::ScrollBar;
use crate::text::Font;
use crate::theme::{Colors, rgb};

/// The height of one row.
pub const ROW_H: i32 = 13;
/// The scroll bar's width.
const BAR_W: i32 = 16;

/// An open list: its frame (in the window's coordinates; it may reach past the window's edges)
/// and how many rows it shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DropList {
    pub frame: Rect,
    pub rows: usize,
}

/// What of an open list the pointer is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListPart {
    /// The entry shown in a row.
    Row(usize),
    Up,
    Down,
    PageUp,
    PageDown,
    Thumb,
    /// Inside the frame, on nothing that acts (a row without an entry, a grayed scroll bar).
    Frame,
}

impl DropList {
    /// A list `rows` rows high whose frame starts at (x, y) and is `w` wide.
    pub fn new(x: i32, y: i32, w: i32, rows: usize) -> DropList {
        DropList { frame: Rect::new(x, y, w, rows as i32 * ROW_H + 2), rows }
    }

    /// The rows, without the frame and the scroll bar.
    pub fn rows_rect(&self) -> Rect {
        let f = self.frame;
        Rect::new(f.x + 1, f.y + 1, f.w - 2 - BAR_W, f.h - 2)
    }

    /// The `row`th row shown.
    pub fn row_rect(&self, row: usize) -> Rect {
        let r = self.rows_rect();
        Rect::new(r.x, r.y + row as i32 * ROW_H, r.w, ROW_H)
    }

    /// The scroll bar for `total` entries.
    pub(crate) fn scroll_bar(&self, total: usize) -> ScrollBar {
        let f = self.frame;
        ScrollBar { bar: Rect::new(f.right() - 1 - BAR_W, f.y + 1, BAR_W, f.h - 2), total, page: self.rows }
    }

    /// Whether the scroll bar can be used: more entries than rows (otherwise it is shown grayed,
    /// with no thumb, as the Run box's list shows it).
    pub fn scrolls(&self, total: usize) -> bool {
        total > self.rows
    }

    /// The first row shown at most.
    pub fn max_top(&self, total: usize) -> usize {
        total.saturating_sub(self.rows)
    }

    /// What is under (x, y) while `top` is the first entry shown, if it is inside the frame.
    pub fn hit(&self, total: usize, top: usize, x: i32, y: i32) -> Option<ListPart> {
        if !self.frame.contains(x, y) {
            return None;
        }
        let rows = self.rows_rect();
        if rows.contains(x, y) {
            let i = top + ((y - rows.y) / ROW_H) as usize;
            return Some(if i < total { ListPart::Row(i) } else { ListPart::Frame });
        }
        if !self.scrolls(total) {
            return Some(ListPart::Frame);
        }
        let r = self.scroll_bar(total).rects(top);
        let found = [
            (r.up, ListPart::Up),
            (r.down, ListPart::Down),
            (r.thumb, ListPart::Thumb),
            (r.page_up, ListPart::PageUp),
            (r.page_down, ListPart::PageDown),
        ];
        Some(found.iter().find(|(rect, _)| rect.contains(x, y)).map_or(ListPart::Frame, |&(_, p)| p))
    }

    /// The rectangle of a part (for drawing a held scroll arrow pushed in).
    pub fn part_rect(&self, total: usize, top: usize, part: ListPart) -> Option<Rect> {
        let r = self.scroll_bar(total).rects(top);
        match part {
            ListPart::Row(i) => (i >= top && i < top + self.rows).then(|| self.row_rect(i - top)),
            ListPart::Up => Some(r.up),
            ListPart::Down => Some(r.down),
            ListPart::PageUp => Some(r.page_up),
            ListPart::PageDown => Some(r.page_down),
            ListPart::Thumb => Some(r.thumb),
            ListPart::Frame => Some(self.frame),
        }
    }

    /// Paints the list: `labels` from `top`, `selected` highlighted (with the dotted rectangle
    /// over it when `dots` gives the screen position it lines up with), and the scroll bar, its
    /// arrows pushed in while `held` says so (-1 the upper, 1 the lower).
    #[allow(clippy::too_many_arguments)]
    pub fn paint(
        &self,
        fb: &mut Framebuffer,
        c: &Colors,
        font: &Font,
        labels: &[&str],
        (top, selected): (usize, Option<usize>),
        dots: Option<(i32, i32)>,
        held: impl Fn(i32) -> bool,
    ) {
        fb.frame(self.frame, rgb(0x000000), rgb(0x000000));
        let rows = self.rows_rect();
        fb.fill(rows, rgb(0xFFFFFF));
        for (row, (i, label)) in labels.iter().enumerate().skip(top).take(self.rows).enumerate() {
            let r = self.row_rect(row);
            let chosen = selected == Some(i);
            let ink = if chosen {
                fb.fill(r, c.menu_highlight);
                c.menu_highlight_text
            } else {
                c.text
            };
            draw_clipped(fb, font, (r.x + 2, r.y), label, ink, r);
            if let (true, Some(screen)) = (chosen, dots) {
                invert_dots(fb, r, screen);
            }
        }
        let bar = self.scroll_bar(labels.len());
        if self.scrolls(labels.len()) {
            bar.paint(fb, c, top, held(-1), held(1));
        } else {
            paint_grayed_bar(fb, c, bar.bar);
        }
    }
}

/// A scroll bar that cannot be used (all entries fit): the track's checkerboard and both
/// arrows grayed out (gray with a white copy 1px down and right), no thumb. Each arrow is 16px
/// high, or half the bar when it is shorter than two (PROVISIONAL: not seen).
fn paint_grayed_bar(fb: &mut Framebuffer, c: &Colors, bar: Rect) {
    for y in bar.y..bar.bottom() {
        for x in bar.x..bar.right() {
            fb.set(x, y, if (x + y) % 2 == 1 { c.highlight } else { c.face });
        }
    }
    let h = BAR_W.min(bar.h / 2);
    for (r, up) in [(Rect::new(bar.x, bar.y, BAR_W, h), true), (Rect::new(bar.x, bar.bottom() - h, BAR_W, h), false)] {
        let mut button = Framebuffer::new(BAR_W, BAR_W, 0);
        scroll_arrow(&mut button, c, Rect::new(0, 0, BAR_W, BAR_W), up, false);
        // The arrow (where `scroll_arrow` puts it) becomes the grayed one.
        let arrow: Vec<(i32, i32)> = (0..4)
            .flat_map(|row| {
                let w = if up { 2 * row + 1 } else { 7 - 2 * row };
                (0..w).map(move |i| (7 - w / 2 + i, 6 + row))
            })
            .collect();
        for &(x, y) in &arrow {
            button.set(x, y, c.face);
        }
        for &(x, y) in &arrow {
            button.set(x + 1, y + 1, c.highlight);
        }
        for &(x, y) in &arrow {
            button.set(x, y, c.shadow);
        }
        for y in 0..h {
            for x in 0..BAR_W {
                // A short button keeps its frame: its lower rows come from the full one's.
                let from = if y < h / 2 || h == BAR_W { y } else { BAR_W - (h - y) };
                fb.set(r.x + x, r.y + y, button.get(x, from));
            }
        }
    }
}

/// What an open list shows: its first row and the highlighted entry (none when nothing in the
/// list starts with the field's text).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shown {
    pub top: usize,
    pub selected: Option<usize>,
}

impl Shown {
    /// Opens on `selected`, which becomes the first row as far as the list allows (otherwise the
    /// list starts at its first entry).
    pub fn open(list: &DropList, total: usize, selected: Option<usize>) -> Shown {
        Shown { top: selected.unwrap_or(0).min(list.max_top(total)), selected }
    }

    /// Scrolls `delta` rows (the scroll bar, the wheel), the highlight staying where it is.
    pub fn scroll(&mut self, list: &DropList, total: usize, delta: i32) {
        self.top = (self.top as i32 + delta).clamp(0, list.max_top(total) as i32) as usize;
    }

    /// Highlights entry `i` and scrolls it into view.
    pub fn select(&mut self, list: &DropList, i: usize) {
        self.selected = Some(i);
        if i < self.top {
            self.top = i;
        } else if i >= self.top + list.rows {
            self.top = i + 1 - list.rows;
        }
    }

    /// Moves the highlight `delta` entries (an arrow key; nothing highlighted yet: the first
    /// entry), stopping at the ends. The entry it lands on.
    pub fn step(&mut self, list: &DropList, total: usize, delta: i32) -> Option<usize> {
        if total == 0 {
            return None;
        }
        let i = match self.selected {
            None => 0,
            Some(i) => (i as i32 + delta).clamp(0, total as i32 - 1) as usize,
        };
        self.select(list, i);
        Some(i)
    }
}

/// The first entry that starts with `text` (letters of either case alike), as Windows finds the
/// entry to highlight when the list opens. Nothing for an empty text.
pub fn find_prefix(entries: &[String], text: &str) -> Option<usize> {
    if text.is_empty() {
        return None;
    }
    let text = text.to_lowercase();
    entries.iter().position(|e| e.to_lowercase().starts_with(&text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_is_measured_in_rows() {
        let l = DropList::new(57, 103, 275, 10);
        assert_eq!(l.frame, Rect::new(57, 103, 275, 132));
        assert_eq!(l.rows_rect(), Rect::new(58, 104, 257, 130));
        assert_eq!(l.scroll_bar(15).bar, Rect::new(315, 104, 16, 130));
        // Windows 98, 15 entries in 10 rows: the thumb 65 long at the top of the track.
        assert_eq!(l.scroll_bar(15).thumb(0), Rect::new(315, 120, 16, 65));
        assert_eq!(l.hit(15, 2, 60, 105), Some(ListPart::Row(2)));
        assert_eq!(l.hit(15, 2, 60, 104 + 13 * 9), Some(ListPart::Row(11)));
        assert_eq!(l.hit(15, 2, 320, 110), Some(ListPart::Up));
        assert_eq!(l.hit(7, 0, 320, 110), Some(ListPart::Frame), "a grayed scroll bar does nothing");
        assert_eq!(l.hit(15, 2, 56, 110), None);
    }

    #[test]
    fn it_opens_on_the_entry_and_keeps_it_in_view() {
        let l = DropList::new(0, 0, 100, 10);
        assert_eq!(Shown::open(&l, 15, Some(2)), Shown { top: 2, selected: Some(2) });
        assert_eq!(Shown::open(&l, 15, Some(6)), Shown { top: 5, selected: Some(6) }, "as far as the list allows");
        assert_eq!(Shown::open(&l, 15, None), Shown { top: 0, selected: None });
        let mut s = Shown::open(&l, 15, None);
        assert_eq!(s.step(&l, 15, 1), Some(0), "the first entry when none is highlighted");
        for _ in 0..12 {
            s.step(&l, 15, 1);
        }
        assert_eq!(s, Shown { top: 3, selected: Some(12) });
        s.scroll(&l, 15, -3);
        assert_eq!(s, Shown { top: 0, selected: Some(12) }, "scrolling leaves the highlight");
        s.scroll(&l, 15, 30);
        assert_eq!(s.top, 5);
    }

    #[test]
    fn it_finds_the_first_entry_starting_with_the_text() {
        let entries: Vec<String> = ["freecell", "sol", "c:\\windows", "c:\\", "Calc"].iter().map(|s| s.to_string()).collect();
        assert_eq!(find_prefix(&entries, "c:\\"), Some(2), "not the exact entry: the first that starts so");
        assert_eq!(find_prefix(&entries, "CA"), Some(4));
        assert_eq!(find_prefix(&entries, "zz"), None);
        assert_eq!(find_prefix(&entries, ""), None);
    }
}
