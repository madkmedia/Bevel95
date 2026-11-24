//! A folder window's other views: Small Icons, List and Details (Windows' list view in its
//! LVS_SMALLICON, LVS_LIST and LVS_REPORT modes), measured on the three Reference PCs
//! (`private/reference/refs/vm/<era>-truecolor-view-*`, numbers in docs/theme-spec.md "Folder views").
//!
//! - Every view shows the 16×16 picture beside a one-line label in rows 17 pixels high. The label's
//!   box (the selection's color, the focus frame) starts right after the picture and is the
//!   text's width and 4 pixels (Details: 8), the whole row high; the text starts 2 pixels in.
//! - Small Icons: free places as large icons, on a grid one row high and (95) 112 pixels wide or
//!   (98, 2000) as wide as the widest label needs; labels are never cut (95's long label runs over
//!   the next cell).
//! - List: column after column, as many rows as fit; the columns as wide as the widest label
//!   needs (95: 112 when the window opened in List).
//! - Details: one row per item under the header (`paint_header`); Name, Size, Type, Modified (My
//!   Computer: Name, Type, Total Size, Free Space); only the name is highlighted; a name too long
//!   for its column ends in "...".

use crate::desktop_icons::{Arrange, Desktop, Gesture, Host, Item, Key, Picture, ViewMode, focus_frame, intersects, shift};
use std::collections::HashMap;

use crate::fb::{Framebuffer, Icon, Rect};
use crate::text::Font;
use crate::theme::{Era, Theme};

/// One row of every small view.
pub const ROW_H: i32 = 17;
/// The small pictures' size.
pub const SMALL_PICTURE: i32 = 16;
/// Windows 95's Small Icons grid width (and List's column width in a window opened in List).
pub const SMALL_95: i32 = 112;
/// How much wider than its widest label text a 98 or 2000 Small Icons cell and a List column are
/// (98: "a0" 40 apart, the long test name 213; the picture 16, the label box 4, the gap between).
pub const COLUMN_PAD: i32 = 28;
/// Details' header height.
pub const HEADER_H: i32 = 17;
/// Details: the picture's place in the Name column, the label box's start, and its padding.
const DETAILS_PICTURE_X: i32 = 2;
const DETAILS_LABEL_X: i32 = 18;
const DETAILS_LABEL_PAD: i32 = 8;
/// How much of the Name column a cut name leaves free after the label's start.
const DETAILS_LABEL_ROOM: i32 = 4;
/// The other columns' text inset from the column's edge (left, or right for numbers).
const CELL_INSET: i32 = 6;
/// A divider takes the mouse this far left and right of the next column's first pixel.
pub const DIVIDER_LEFT: i32 = 8;
pub const DIVIDER_RIGHT: i32 = 7;

/// Which columns Details shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Columns {
    /// Name, Size, Type, Modified.
    #[default]
    Folder,
    /// My Computer: Name, Type, Total Size, Free Space.
    MyComputer,
}

impl Columns {
    pub fn titles(self) -> [&'static str; 4] {
        match self {
            Columns::Folder => ["Name", "Size", "Type", "Modified"],
            Columns::MyComputer => ["Name", "Type", "Total Size", "Free Space"],
        }
    }

    /// The columns' widths as each Windows first showed them.
    pub fn default_widths(self, era: Era) -> Vec<i32> {
        match (self, era) {
            (Columns::MyComputer, _) => vec![120, 150, 90, 90],
            (Columns::Folder, Era::Win2000) => vec![120, 96, 120, 120],
            (Columns::Folder, _) => vec![120, 60, 120, 120],
        }
    }

    /// Whether column `k`'s text stands at its right edge (sizes).
    pub fn right_aligned(self, k: usize) -> bool {
        match self {
            Columns::Folder => k == 1,
            Columns::MyComputer => k >= 2,
        }
    }

    /// What a click on column `k`'s header sorts by.
    pub fn sorts_by(self, k: usize) -> Arrange {
        match (self, k) {
            (Columns::Folder, 1) | (Columns::MyComputer, 2) => Arrange::Size,
            (Columns::Folder, 2) | (Columns::MyComputer, 1) => Arrange::Type,
            (Columns::Folder, 3) | (Columns::MyComputer, 3) => Arrange::Date,
            _ => Arrange::Name,
        }
    }

    /// The column a sort shows its arrow on (2000).
    pub fn column_of(self, by: Arrange) -> usize {
        (0..4).find(|&k| self.sorts_by(k) == by).unwrap_or(0)
    }
}

impl ViewMode {
    /// Large and Small Icons let items lie anywhere; List and Details keep them in order.
    pub fn arranged_freely(self) -> bool {
        matches!(self, ViewMode::Large | ViewMode::Small)
    }
}

/// A size as Details' Size column wrote it: whole KB rounded up, thousands with commas, "KB" right
/// after the number (95, 98: "1,600KB") or after a space (2000: "2,328 KB").
pub fn kb_text(era: Era, bytes: u64) -> String {
    let kb = bytes.div_ceil(1024);
    let digits = kb.to_string();
    let mut out = String::new();
    for (k, c) in digits.chars().enumerate() {
        if k > 0 && (digits.len() - k).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out + if era == Era::Win2000 { " KB" } else { "KB" }
}

/// A time (a FILETIME in local time: 100 ns steps since 1601) as Details wrote it: "11/14/26 2:39
/// PM" (95, 98), "11/14/2026 4:13 PM" (2000). Nothing for 0.
pub fn date_text(era: Era, local: u64) -> String {
    if local == 0 {
        return String::new();
    }
    let minutes = local / 600_000_000;
    let days = (minutes / 1440) as i64;
    let (h, m) = ((minutes % 1440) / 60, minutes % 60);
    // Days from 1601-01-01 to the civil date (Howard Hinnant's days_from_civil, backwards).
    let z = days - 134_774 + 719_468; // 1601-01-01 is 134774 days before 1970-01-01
    let era_ = z.div_euclid(146_097);
    let doe = z - era_ * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era_ * 400 + i64::from(month <= 2);
    let (h12, ampm) = match h {
        0 => (12, "AM"),
        1..=11 => (h, "AM"),
        12 => (12, "PM"),
        _ => (h - 12, "PM"),
    };
    let year = if era == Era::Win2000 { year.to_string() } else { format!("{:02}", year.rem_euclid(100)) };
    format!("{month}/{d}/{year} {h12}:{m:02} {ampm}")
}

/// A local time as a FILETIME (for tests and the comparisons: the Reference PCs' dates).
pub fn local_filetime(year: i64, month: i64, day: i64, hour: u64, minute: u64) -> u64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days_1970 = era * 146_097 + doe - 719_468;
    let days = (days_1970 + 134_774) as u64;
    ((days * 1440 + hour * 60 + minute) * 60) * 10_000_000
}

impl Desktop {
    pub fn mode(&self) -> ViewMode {
        self.mode
    }

    /// Changes the view: the items go into the new view's rows or columns in their order (as
    /// Windows arranged them anew when the view changed).
    pub fn set_mode(&mut self, mode: ViewMode) {
        if mode == self.mode {
            return;
        }
        let before = self.mode;
        let (old_w, old_h) = self.cell_size();
        self.mode = mode;
        self.gesture = None;
        self.label_sizes.borrow_mut().clear();
        if self.columns.is_empty() {
            self.columns = self.details.default_widths(self.era);
        }
        // A folder window going to List or Details keeps the icons' places for the way back, and
        // coming back puts them there: no original moved a dragged icon at that step (the
        // Reference PCs, brief 103: `*-arrange-mc-c-moved-details-back-to-large`).
        if self.host == Host::Window {
            if before.arranged_freely() && !mode.arranged_freely() {
                if let Some(places) = self.places.take() {
                    self.stash = Some((before, places));
                }
                self.relayout();
                return;
            }
            if !before.arranged_freely()
                && mode.arranged_freely()
                && let Some((was, places)) = self.stash.take()
            {
                if was == mode {
                    self.places = Some(places);
                    self.relayout();
                    return;
                }
                if self.era != Era::Win2000 {
                    let (old_w, old_h) = self.cell_size_of(was);
                    let (new_w, new_h) = self.cell_size();
                    let moved: HashMap<String, (i32, i32)> = places
                        .into_iter()
                        .map(|(id, (x, y))| (id, (x * new_w / old_w.max(1), y * new_h / old_h.max(1))))
                        .collect();
                    self.places = Some(moved);
                    self.relayout();
                    return;
                }
                self.moved.clear();
            }
        }
        // Between Large and Small Icons each item keeps its place on the grid (column and row):
        // 95's three columns of large icons became three columns of small ones (`v1-small-open`).
        // Windows 2000 put them into rows as wide as the window again (`-view-v1-small-open`).
        if before.arranged_freely() && mode.arranged_freely() && self.places.is_some() && self.era != Era::Win2000 {
            let (new_w, new_h) = self.cell_size();
            let cells: Vec<(i32, i32)> = self.cells.iter().map(|&(x, y)| (x.div_euclid(old_w.max(1)) * new_w, y.div_euclid(old_h.max(1)) * new_h)).collect();
            self.cells = cells;
            self.places = Some(HashMap::new());
            self.store_cells();
            self.changed = false;
            self.relayout();
            return;
        }
        // Put in order afresh: no icon lies where it was dragged any more.
        self.moved.clear();
        self.places = None;
        self.relayout();
        if self.host == Host::Window && mode.arranged_freely() && !self.items.is_empty() {
            self.freeze();
            self.changed = false;
        }
    }

    /// Small Icons' cell and List's column from the widest label (`font`: the window's), and (95)
    /// List's 112 when the window opened in List (`fixed_list`).
    pub fn measure(&mut self, font: &Font, fixed_list: bool) {
        let widest = self.items.iter().map(|i| font.measure(&i.name)).max().unwrap_or(0);
        let (small, list) = match self.era {
            Era::Win95 => (SMALL_95, if fixed_list { SMALL_95.max(widest + COLUMN_PAD) } else { widest + COLUMN_PAD }),
            _ => (widest + COLUMN_PAD, widest + COLUMN_PAD),
        };
        if (small, list) != (self.small_w, self.list_w) {
            self.small_w = small;
            self.list_w = list;
            if self.mode != ViewMode::Large && self.mode != ViewMode::Details {
                self.places = None;
                self.relayout();
                if self.mode == ViewMode::Small && self.host == Host::Window && !self.items.is_empty() {
                    self.freeze();
                    self.changed = false;
                }
            }
        }
    }

    pub fn list_width(&self) -> i32 {
        self.list_w
    }

    /// Details' columns' widths (Name first); fewer or none given: the defaults.
    pub fn set_columns(&mut self, widths: &[i32]) {
        let mut w = self.details.default_widths(self.era);
        for (k, &v) in widths.iter().enumerate().take(w.len()) {
            w[k] = v.max(0);
        }
        self.columns = w;
        self.label_sizes.borrow_mut().clear();
    }

    /// Which columns Details shows (My Computer has its own).
    pub fn set_details(&mut self, columns: crate::desktop_icons::Columns) {
        if self.details != columns || self.columns.is_empty() {
            self.details = columns;
            self.columns = columns.default_widths(self.era);
        }
    }

    /// How many rows a List column holds: as many as fit in the list (at least one).
    pub fn list_rows(&self) -> usize {
        (self.area.h / ROW_H).max(1) as usize
    }

    /// Item `i`'s cell in List (column after column) and Details (row after row).
    pub(super) fn list_cell(&self, i: usize) -> (i32, i32) {
        match self.mode {
            ViewMode::List => {
                let rows = self.list_rows();
                ((i / rows) as i32 * self.list_w, (i % rows) as i32 * ROW_H)
            }
            _ => (0, i as i32 * ROW_H),
        }
    }

    /// How far List and Details reach (`None` in the other views): List its columns across and the
    /// list's own height; Details its columns across and every row down.
    pub(super) fn list_extent(&self) -> Option<(i32, i32)> {
        let n = self.items.len() as i32;
        match self.mode {
            ViewMode::List => {
                let rows = self.list_rows() as i32;
                let cols = (n + rows - 1) / rows;
                Some((cols * self.list_w, if n == 0 { 0 } else { rows.min(n) * ROW_H }))
            }
            ViewMode::Details => Some((self.columns.iter().sum(), n * ROW_H)),
            _ => None,
        }
    }

    /// The small picture's place in a cell.
    pub(super) fn small_icon_rect(&self, cell: Rect) -> Rect {
        let x = if self.mode == ViewMode::Details { cell.x + DETAILS_PICTURE_X } else { cell.x };
        // Windows 2000's Details drew its pictures a row lower.
        let y = if self.mode == ViewMode::Details && self.era == Era::Win2000 { cell.y + 1 } else { cell.y };
        Rect::new(x, y, SMALL_PICTURE, SMALL_PICTURE)
    }

    /// The label's text in a small view (Details: cut to the Name column with "...").
    fn small_text(&self, font: &Font, i: usize) -> String {
        let name = &self.items[i].name;
        if self.mode == ViewMode::Details {
            let room = self.columns.first().copied().unwrap_or(0) - DETAILS_LABEL_X - DETAILS_LABEL_ROOM;
            return font.elide(name, room.max(0));
        }
        name.clone()
    }

    pub(super) fn small_label_box(&self, font: &Font, i: usize, cell: Rect) -> Rect {
        let cached = self.label_sizes.borrow().get(i).and_then(|s| s[0]);
        let w = cached.map(|c| c.0).unwrap_or_else(|| {
            let w = font.measure(&self.small_text(font, i));
            let mut sizes = self.label_sizes.borrow_mut();
            if sizes.len() < self.items.len() {
                sizes.resize(self.items.len(), [None, None]);
            }
            sizes[i][0] = Some((w, 1));
            w
        });
        let (x, pad) = if self.mode == ViewMode::Details { (cell.x + DETAILS_LABEL_X, DETAILS_LABEL_PAD) } else { (cell.x + SMALL_PICTURE, 4) };
        Rect::new(x, cell.y, w + pad, ROW_H)
    }

    pub(super) fn small_label(&self, font: &Font, i: usize, cell: Rect) -> (Vec<String>, Rect) {
        (vec![self.small_text(font, i)], self.small_label_box(font, i, cell))
    }

    /// The edit box over a label in a small view: one line, the edit control's frame and margin,
    /// over the label's box (measured: `*-view-v1-list-rename`, `-details-rename`).
    pub(super) fn small_rename_box(&self, font: &Font, i: usize, text: &[char]) -> Option<(Rect, Vec<(i32, i32)>)> {
        let label = self.small_label_box(font, i, self.cell(i));
        let s: String = text.iter().collect();
        let w = font.measure(&s);
        let pad = self.edit_pad();
        let b = Rect::new(label.x - pad + 1, label.y, (w + 2 * pad + 2).max(label.w), ROW_H);
        Some((b, vec![(b.x + 1 + pad, b.y + 2)]))
    }

    /// A renamed name's lines: wrapped as a large icon's label, one line in the small views.
    pub(super) fn edit_lines(&self, font: &Font, text: &[char]) -> Vec<(usize, usize)> {
        if self.mode.small() { vec![(0, text.len())] } else { Desktop::rename_lines(font, text) }
    }

    /// List and Details: the item under (x, y) found by arithmetic (`None`: another view). Only its
    /// picture and label take the mouse.
    pub(super) fn list_hit(&self, font: &Font, x: i32, y: i32) -> Option<Option<usize>> {
        if self.mode.arranged_freely() {
            return None;
        }
        let (cx, cy) = (x - self.area.x, y - self.area.y);
        if cx < 0 || cy < 0 {
            return Some(None);
        }
        let row = (cy / ROW_H) as usize;
        let i = match self.mode {
            ViewMode::List => (cx / self.list_w.max(1)) as usize * self.list_rows() + row.min(self.list_rows() - 1),
            _ => row,
        };
        if self.mode == ViewMode::List && row >= self.list_rows() {
            return Some(None);
        }
        Some((i < self.items.len() && self.on(font, i, x, y)).then_some(i))
    }

    /// The item a key moves to in the small views (`None`: not handled here; `Some(None)`: stays).
    pub(super) fn view_key(&self, key: Key, from: Option<usize>) -> Option<Option<usize>> {
        let n = self.items.len();
        if n == 0 {
            return None;
        }
        let page_rows = ((self.viewport.h / ROW_H).max(1)) as usize;
        match self.mode {
            ViewMode::Large => None,
            ViewMode::Small => match (key, from) {
                (Key::PageDown | Key::PageUp, Some(f)) => {
                    // The furthest item down (up) the same column within a page, else the next one.
                    let (fx, fy) = self.cells[f];
                    let down = key == Key::PageDown;
                    let reach = self.viewport.h - ROW_H;
                    let column = |i: usize| self.cells[i].0 == fx && i != f;
                    let beyond: Vec<usize> = (0..n).filter(|&i| column(i) && if down { self.cells[i].1 > fy } else { self.cells[i].1 < fy }).collect();
                    let within = beyond.iter().copied().filter(|&i| (self.cells[i].1 - fy).abs() <= reach);
                    let pick = if down { within.max_by_key(|&i| self.cells[i].1) } else { within.min_by_key(|&i| self.cells[i].1) };
                    let pick = pick.or_else(|| if down { beyond.iter().copied().min_by_key(|&i| self.cells[i].1) } else { beyond.iter().copied().max_by_key(|&i| self.cells[i].1) });
                    Some(pick.or(Some(f)))
                }
                (Key::PageDown | Key::PageUp, None) => Some(Some(0)),
                _ => None,
            },
            ViewMode::List => {
                let rows = self.list_rows();
                let Some(f) = from else {
                    return matches!(key, Key::Up | Key::Down | Key::Left | Key::Right | Key::PageUp | Key::PageDown).then_some(Some(0));
                };
                let (col, row) = (f / rows, f % rows);
                let last_col = (n - 1) / rows;
                let at = |c: usize| {
                    // The same row of column `c`, or the last item of the column before when it is short.
                    let i = c * rows + row;
                    if i < n { i } else { (c * rows).saturating_sub(1).max(n - 1).min(n - 1) }
                };
                let shown_cols = ((self.viewport.w / self.list_w.max(1)).max(1)) as usize;
                Some(match key {
                    Key::Up => f.checked_sub(1),
                    Key::Down => (f + 1 < n).then_some(f + 1),
                    Key::Left => (col > 0).then(|| at(col - 1)),
                    Key::Right => (col < last_col && (col + 1) * rows + row < n).then(|| (col + 1) * rows + row),
                    Key::PageDown => {
                        let c = (col + shown_cols).min(last_col);
                        let mut i = c * rows + row;
                        if i >= n {
                            i = (c - 1) * rows + row;
                        }
                        Some(at(i / rows).min(i))
                    }
                    Key::PageUp => Some(col.saturating_sub(shown_cols) * rows + row),
                    _ => return None,
                })
            }
            ViewMode::Details => {
                let Some(f) = from else {
                    return matches!(key, Key::Up | Key::Down | Key::PageUp | Key::PageDown).then_some(Some(0));
                };
                // The rows that show whole.
                let top = ((self.viewport.y - self.area.y).max(0) + ROW_H - 1) / ROW_H;
                let bottom = ((self.viewport.bottom() - self.area.y) / ROW_H - 1).max(top);
                let (top, bottom) = (top as usize, (bottom as usize).min(n - 1));
                Some(match key {
                    Key::Up => f.checked_sub(1),
                    Key::Down => (f + 1 < n).then_some(f + 1),
                    Key::Left | Key::Right => None,
                    Key::PageDown if f < bottom => Some(bottom),
                    Key::PageDown => Some((f + page_rows - 1).min(n - 1)),
                    Key::PageUp if f > top => Some(top),
                    Key::PageUp => Some(f.saturating_sub(page_rows - 1)),
                    _ => return None,
                })
            }
        }
    }

    /// The items whose rows show in the viewport (List and Details by arithmetic, the others by
    /// their cells), for painting and for reading the pictures that show first.
    pub fn visible_items(&self, region: Rect) -> Vec<usize> {
        let n = self.items.len();
        match self.mode {
            ViewMode::Details => {
                let first = ((region.y - self.area.y).max(0) / ROW_H) as usize;
                let last = (((region.bottom() - self.area.y) + ROW_H - 1).max(0) / ROW_H) as usize;
                (first.min(n)..last.min(n)).collect()
            }
            ViewMode::List => {
                let rows = self.list_rows();
                let w = self.list_w.max(1);
                let c0 = ((region.x - self.area.x).max(0) / w) as usize;
                let c1 = (((region.right() - self.area.x) + w - 1).max(0) / w) as usize;
                (c0 * rows).min(n)..(c1 * rows).min(n)
            }
            .collect(),
            _ => (0..n)
                .filter(|&i| {
                    let c = self.cell(i);
                    let reach = if self.mode == ViewMode::Large { Rect::new(c.x, c.y, c.w, c.h + 4 * 13) } else { Rect::new(c.x, c.y, c.w.max(300), c.h) };
                    intersects(reach, region)
                })
                .collect(),
        }
    }

    /// Adds item i to the selection or takes it away (as Ctrl+click, without moving the focus).
    pub fn select(&mut self, i: usize, on: bool) {
        if let Some(s) = self.selected.get_mut(i) {
            *s = on;
        }
    }

    /// The order the items are in: Arrange Icons' (or a header's) and whether reversed.
    pub fn sorted(&self) -> (Arrange, bool) {
        (self.arrange, self.reversed)
    }

    /// Sorts the items (a header click, Arrange Icons): `reversed` turns the whole order round,
    /// folders last (95 to 2000 alike).
    pub fn sort(&mut self, by: Arrange, reversed: bool) {
        self.reversed = reversed;
        self.keep_order = false;
        // Every icon back in that order: none goes last any more (brief 103).
        self.last.clear();
        self.set_arrange(by);
    }

    /// Puts the owner's items in order: Arrange Icons' rules; in a folder window Windows 2000's
    /// dates oldest first (its Modified header; 95 and 98 newest first); reversed when asked.
    pub(super) fn sort_files(&self, files: &mut [Item], by: Arrange) {
        if self.host == Host::Window && self.details == Columns::MyComputer {
            self.sort_my_computer(files, by);
            if self.reversed {
                files.reverse();
            }
            return;
        }
        crate::desktop_icons::arrange(files, by);
        if self.host == Host::Window && self.era == Era::Win2000 && by == Arrange::Date {
            let folders = files.iter().filter(|i| i.folder).count();
            files[..folders].reverse();
            files[folders..].reverse();
            // Ties stay by name (the reversal turned them round).
            let name = |i: &Item| i.name.to_lowercase();
            files[..folders].sort_by(|a, b| a.modified.cmp(&b.modified).then_with(|| name(a).cmp(&name(b))));
            files[folders..].sort_by(|a, b| a.modified.cmp(&b.modified).then_with(|| name(a).cmp(&name(b))));
        }
        if self.reversed {
            files.reverse();
        }
    }

    /// My Computer's Arrange Icons (brief 103; the Reference PCs' submenu: by Drive Letter, by
    /// Type, by Size, by Free Space; `Arrange::Name` is by Drive Letter, `Date` by Free Space, as
    /// the Details headers sort): the drives by their letter, type, size or free space (the
    /// facts the window already has for Details, `spaces`; a drive without them counts as 0, so
    /// an empty floppy comes first, as `win2000-...-mc-after-arrange-by-free-space` shows), the
    /// other places (Control Panel, Printers, ...) after the drives in the order they came.
    fn sort_my_computer(&self, files: &mut [Item], by: Arrange) {
        let given = |i: &Item| self.given.get(&i.id).copied().unwrap_or(usize::MAX);
        let drive = |i: &Item| i.id.len() == 3 && i.id.ends_with(":\\");
        let space = |i: &Item| self.spaces.get(&i.id).copied().unwrap_or((0, 0));
        files.sort_by(|a, b| {
            drive(b).cmp(&drive(a)).then_with(|| {
                if !drive(a) {
                    return given(a).cmp(&given(b));
                }
                let key = match by {
                    Arrange::Name => std::cmp::Ordering::Equal,
                    Arrange::Type => a.type_name.to_lowercase().cmp(&b.type_name.to_lowercase()),
                    Arrange::Size => space(a).1.cmp(&space(b).1),
                    Arrange::Date => space(a).0.cmp(&space(b).0),
                };
                key.then_with(|| a.id.to_ascii_uppercase().cmp(&b.id.to_ascii_uppercase()))
            })
        });
    }

    /// Replaces items' pictures (read after their names showed), by id; nothing moves.
    pub fn set_pictures(&mut self, pictures: Vec<(String, Picture)>) {
        if pictures.is_empty() {
            return;
        }
        let index: std::collections::HashMap<&str, usize> = self.items.iter().enumerate().map(|(k, i)| (i.id.as_str(), k)).collect();
        let found: Vec<(usize, Picture)> = pictures.into_iter().filter_map(|(id, p)| index.get(id.as_str()).map(|&k| (k, p))).collect();
        for (k, p) in found {
            self.items[k].picture = p;
        }
    }

    /// The small picture of item `i` as it shows (selected: tinted as each Windows did).
    fn small_picture(&self, theme: &Theme, i: usize, selected: bool) -> Option<Icon> {
        use crate::folder::pane::{self, NodeKind};
        use crate::desktop_art::Art;
        let item = &self.items[i];
        let icon = match &item.picture {
            Picture::Folder => pane::picture(self.era, NodeKind::Folder, false),
            Picture::Art(Art::Drive(k)) => pane::picture(self.era, NodeKind::Drive(*k), false),
            Picture::Art(Art::ControlPanel) => pane::picture(self.era, NodeKind::ControlPanel, false),
            Picture::Art(Art::Printers) => pane::picture(self.era, NodeKind::Printers, false),
            Picture::Art(_) | Picture::System(_) => return None,
            Picture::Own(icon) => icon.as_ref()?.downscaled(SMALL_PICTURE),
            Picture::Shell(p) => match (&p.small, &p.large) {
                (Some(s), _) => s.clone(),
                (None, Some(l)) => l.downscaled(SMALL_PICTURE),
                _ => return None,
            },
            Picture::Kind(k) => crate::file_kinds::picture(self.era, theme.depth, *k, &item.id, true)?,
        };
        let mut icon = icon;
        if item.shortcut {
            // Shell32's own 16×16 arrow, as the Reference PCs drew it in the small views (the
            // `-fileicons-` captures' Internet shortcut, 2026-11-23); Windows 2000's through its
            // image list's 5 bits per channel (its gray 808080 shows as 848484, as its 32×32 one).
            let mut arrow = crate::icons::SHORTCUT_ARROW.icon();
            if self.era == Era::Win2000 {
                let five = |v: u32| (v & 0xF8) | (v >> 5);
                for p in arrow.pixels.iter_mut().filter(|p| **p >> 24 != 0) {
                    *p = (*p & 0xFF00_0000) | five((*p >> 16) & 0xFF) << 16 | five((*p >> 8) & 0xFF) << 8 | five(*p & 0xFF);
                }
            }
            for (p, a) in icon.pixels.iter_mut().zip(&arrow.pixels) {
                if a >> 24 != 0 {
                    *p = *a;
                }
            }
        }
        // Selected: each pixel half the selection color (95, 98), in 5-bit steps in 2000 (the
        // Reference PCs: FFFFFF over 000080 7F7FBF; over 0A246A 7B8CAD).
        Some(if selected { crate::desktop_icons::cut_picture(self.era, &icon, theme.colors.menu_highlight) } else { icon })
    }

    /// Paints the small views' items that reach into `region` (its pixels are `tmp`'s).
    pub(super) fn paint_small(&self, tmp: &mut Framebuffer, theme: &Theme, font: &Font, region: Rect, origin: (i32, i32), active: bool) {
        let c = &theme.colors;
        let renaming = self.renaming();
        let mut order = self.visible_items(region);
        // Small Icons: the later items first, so that a long label lies over the next cell's item
        // (95's "A file with a rather long name..." hid Big.dat: `*-view-v1-small-one`).
        if self.mode == ViewMode::Small {
            order.reverse();
        }
        // The focus item last (its label may run over the next cell in Small Icons).
        if let Some(f) = self.focus
            && let Some(k) = order.iter().position(|&i| i == f)
        {
            order.remove(k);
            order.push(f);
        }
        // Windows 2000 kept the selection of a window without the keyboard, in the face color.
        let gray = !active && self.era == Era::Win2000 && self.host == Host::Window;
        for i in order {
            let cell = self.cell(i);
            let hover = self.drop_hover == Some(i);
            let selected = (self.is_selected(i) && (active || gray) || hover) && renaming != Some(i);
            let lit = active || hover;
            let r = self.small_icon_rect(cell);
            let sixteen = theme.depth.reduced();
            if let Some(icon) = self.small_picture(theme, i, selected && lit && !sixteen) {
                let icon = if self.is_faded(i) { crate::desktop_icons::cut_picture(self.era, &icon, crate::desktop_icons::WINDOW) } else { icon };
                let icon = if sixteen { crate::desktop_icons::sixteen_picture(theme, &icon, selected && lit, (0, 0)) } else { icon };
                tmp.draw_icon(&icon, r.x - region.x, r.y - region.y);
            }
            if self.mode == ViewMode::Details {
                self.paint_cells(tmp, font, i, cell, region);
            }
            if renaming == Some(i) {
                continue;
            }
            let (lines, label) = self.small_label(font, i, cell);
            let at = shift(label, (-region.x, -region.y));
            let ink = if selected && lit {
                tmp.fill(at, c.menu_highlight);
                c.menu_highlight_text
            } else if selected {
                tmp.fill(at, c.face);
                c.text
            } else {
                tmp.fill(at, crate::desktop_icons::WINDOW);
                crate::desktop_icons::WINDOW_TEXT
            };
            // Small Icons' text sits 2 pixels higher in its row than List's and Details' (the Reference PCs).
            let ty = self.text_y();
            font.draw(tmp, at.x + 2, at.y + ty, &lines[0], ink);
            if active && self.focus == Some(i) && (self.era != Era::Win2000 || self.keys_used) {
                focus_frame(tmp, at, origin, theme);
            }
        }
    }

    /// Details: an item's other columns (size, type, date; a drive's type, size and free space).
    fn paint_cells(&self, tmp: &mut Framebuffer, font: &Font, i: usize, cell: Rect, region: Rect) {
        let mut x = cell.x + self.columns.first().copied().unwrap_or(0);
        for k in 1..self.columns.len().min(4) {
            let w = self.columns[k];
            let text = self.cell_text(i, k);
            if w > 0 && !text.is_empty() && x < region.right() && x + w > region.x {
                let room = (w - 2 * CELL_INSET).max(0);
                let shown = font.elide(&text, room);
                let tw = font.measure(&shown);
                let tx = if self.details.right_aligned(k) { x + w - CELL_INSET - tw } else { x + CELL_INSET };
                // Clipped to its column.
                let mut part = Framebuffer::new(w, ROW_H, crate::desktop_icons::WINDOW);
                font.draw(&mut part, tx - x, self.text_y(), &shown, crate::desktop_icons::WINDOW_TEXT);
                tmp.blit(&part, x - region.x, cell.y - region.y);
            }
            x += w;
        }
    }

    /// Where a row's text starts below the row's top: Small Icons 0, List and Details 2.
    fn text_y(&self) -> i32 {
        match (self.mode, self.era) {
            (ViewMode::Small, _) => 0,
            _ => 2,
        }
    }

    /// The text of item `i`'s column `k` in Details.
    pub fn cell_text(&self, i: usize, k: usize) -> String {
        let item = &self.items[i];
        match (self.details, k) {
            (_, 0) => item.name.clone(),
            (crate::desktop_icons::Columns::Folder, 1) => {
                if item.folder {
                    String::new()
                } else {
                    kb_text(self.era, item.size)
                }
            }
            (crate::desktop_icons::Columns::Folder, 2) | (crate::desktop_icons::Columns::MyComputer, 1) => item.type_name.clone(),
            (crate::desktop_icons::Columns::Folder, 3) => date_text(self.era, item.modified),
            (crate::desktop_icons::Columns::MyComputer, 2) => self.spaces.get(&item.id).map(|s| crate::folder::status::size_in(self.era, s.1)).unwrap_or_default(),
            (crate::desktop_icons::Columns::MyComputer, 3) => self.spaces.get(&item.id).map(|s| crate::folder::status::size_in(self.era, s.0)).unwrap_or_default(),
            _ => String::new(),
        }
    }

    /// The width a double-click on column `k`'s divider gives it: its widest entry.
    pub fn fitted_width(&self, font: &Font, k: usize) -> i32 {
        if k == 0 {
            let widest = self.items.iter().map(|i| font.measure(&i.name)).max().unwrap_or(0);
            // 95's and 98's long test name: 209 = the text, the picture's 18 and 6 (`-hdr-divider-dblclick`).
            return widest + DETAILS_LABEL_X + 6;
        }
        let widest = (0..self.items.len()).map(|i| font.measure(&self.cell_text(i, k))).max().unwrap_or(0);
        widest + 2 * CELL_INSET
    }

    /// Whether a band drawn from (x, y) is a selection rectangle (anywhere off an item's picture
    /// and label: in Details also right of a name, where it selects nothing it crosses only there).
    pub fn band_active(&self) -> bool {
        matches!(self.gesture, Some(Gesture::Band { .. }))
    }
}

/// The darkest a dragged picture may be: each look's way of laying it over what is under it.
const HALF: u32 = 0x80;

impl Desktop {
    /// The dragged picture of `items` for the window that follows the pointer during a drag of
    /// files, in each look's way (the Reference PCs, `*-drag-image-*`): Windows 95 the picture's
    /// every other pixel (a checkerboard) and the name in black; 98 picture and black name half
    /// over what is under them; 2000 each item as it shows selected, see-through; several items
    /// in 95 and 98 dotted outlines only (Large Icons: a rectangle round each picture and a line
    /// for each name; the small views: a 16 × 16 square and a line). Returns the picture
    /// (0xAARRGGBB, straight alpha) and its top left corner in the view's pixels; only what shows
    /// in the view is drawn.
    pub fn drag_picture(&self, theme: &Theme, font: &Font, items: &[usize], ground: u32) -> Option<(Framebuffer, i32, i32)> {
        let shown = if self.host == Host::Window && self.viewport.w > 0 { self.viewport } else { self.area };
        let items: Vec<usize> = items.iter().copied().filter(|&i| i < self.items.len() && intersects(self.bounds(font, i), shown)).collect();
        let whole = items.iter().map(|&i| self.bounds(font, i)).reduce(crate::desktop_icons::union)?;
        // Cut to what shows (a Select All of thousands draws only the visible ones).
        let x0 = whole.x.max(shown.x);
        let y0 = whole.y.max(shown.y);
        let x1 = whole.right().min(shown.right());
        let y1 = whole.bottom().min(shown.bottom());
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        let mut pic = Framebuffer::new(x1 - x0, y1 - y0, 0);
        let several = items.len() > 1 && self.era != Era::Win2000;
        // The dotted outlines: the inverse of the view's ground, as Windows' inverted dots showed.
        let dots = 0xFF00_0000 | (!ground & 0x00FF_FFFF);
        let dotted = |pic: &mut Framebuffer, r: Rect| {
            let r = shift(r, (-x0, -y0));
            for x in (r.x..r.right()).step_by(2) {
                pic.set(x, r.y, dots);
                pic.set(x, r.bottom() - 1, dots);
            }
            for y in (r.y..r.bottom()).step_by(2) {
                pic.set(r.x, y, dots);
                pic.set(r.right() - 1, y, dots);
            }
        };
        let alpha = match self.era {
            Era::Win95 => 0xFF,
            Era::Win98 => HALF,
            Era::Win2000 => super::DRAG_ALPHA_2000,
        };
        let with = |p: u32| (p & 0x00FF_FFFF) | (alpha << 24);
        for &i in &items {
            let cell = self.cell(i);
            let (icon_rect, label) = if self.mode.small() {
                (self.small_icon_rect(cell), self.small_label_box(font, i, cell))
            } else {
                (self.icon_rect(i), self.label_at(font, i, cell, self.expanded(i)).1)
            };
            if several {
                if self.mode.small() {
                    dotted(&mut pic, icon_rect);
                } else {
                    dotted(&mut pic, Rect::new(cell.x + super::OUTLINE_X, cell.y, super::OUTLINE_W, super::OUTLINE_H));
                }
                dotted(&mut pic, Rect::new(label.x + 2, label.y + label.h / 2, (label.w - 4).max(2), 2));
                continue;
            }
            let selected = self.era == Era::Win2000;
            let picture = if self.mode.small() { self.small_picture(theme, i, selected) } else { self.picture(theme, i, selected) };
            if let Some(icon) = picture {
                for (k, &p) in icon.pixels.iter().enumerate() {
                    let (x, y) = (icon_rect.x + k as i32 % icon.width - x0, icon_rect.y + k as i32 / icon.width - y0);
                    // 95: every other pixel, following the screen's pixels.
                    if p >> 24 < 0x80 || (self.era == Era::Win95 && (x + x0 + y + y0).rem_euclid(2) != 0) {
                        continue;
                    }
                    pic.set(x, y, with(p));
                }
            }
            let at = shift(label, (-x0, -y0));
            let ink = if selected {
                pic.fill(at, with(theme.colors.menu_highlight));
                with(theme.colors.menu_highlight_text)
            } else {
                with(0xFF00_0000)
            };
            if self.mode.small() {
                let text = self.small_label(font, i, cell).0;
                font.draw(&mut pic, at.x + 2, at.y + self.text_y(), &text[0], ink);
            } else {
                let lines = self.label_at(font, i, cell, self.expanded(i)).0;
                super::draw_lines(&mut pic, font, &lines, at, ink);
            }
        }
        Some((pic, x0, y0))
    }
}

/// Details' header: one raised button per column and a last one to the list's right edge (`r`:
/// the header's whole width at its place, `x0` where the first column starts after scrolling).
/// `pressed`: the button held down; `sorted`: 2000's arrow (column, descending).
#[allow(clippy::too_many_arguments)]
pub fn paint_header(fb: &mut Framebuffer, theme: &Theme, font: &Font, era: Era, columns: Columns, widths: &[i32], r: Rect, x0: i32, pressed: Option<usize>, sorted: Option<(usize, bool)>) {
    let c = &theme.colors;
    let mut bar = Framebuffer::new(r.w.max(0), HEADER_H, c.face);
    let titles = columns.titles();
    let mut x = x0 - r.x;
    let mut buttons: Vec<(Rect, Option<usize>)> = Vec::new();
    for (k, &w) in widths.iter().enumerate() {
        buttons.push((Rect::new(x, 0, w, HEADER_H), Some(k)));
        x += w;
    }
    if x < r.w {
        buttons.push((Rect::new(x, 0, r.w - x + 2, HEADER_H), None));
    }
    for (b, k) in buttons {
        if b.w <= 0 {
            continue;
        }
        let down = k.is_some() && k == pressed;
        if down {
            bar.frame(b, c.shadow, c.shadow);
        } else {
            bar.frame(b, c.highlight, c.dark_shadow);
            bar.frame(Rect::new(b.x + 1, b.y + 1, b.w - 2, b.h - 2), c.light, c.shadow);
        }
        let Some(k) = k else { continue };
        let d = i32::from(down);
        let arrow = sorted.filter(|s| s.0 == k && era == Era::Win2000);
        let arrow_w = if arrow.is_some() { ARROW_GAP + 8 } else { 0 };
        let room = (b.w - 2 * CELL_INSET - arrow_w).max(0);
        let text = font.elide(titles[k], room);
        let tw = font.measure(&text);
        let tx = if columns.right_aligned(k) { b.right() - CELL_INSET - tw - if arrow.is_some() { ARROW_RIGHT } else { 0 } } else { b.x + CELL_INSET };
        let mut part = Framebuffer::new((b.w - 4).max(0), HEADER_H - 4, c.face);
        font.draw(&mut part, tx - (b.x + 2) + d, d, &text, c.text);
        if let Some((_, descending)) = arrow {
            paint_sort_arrow(&mut part, theme, tx - (b.x + 2) + tw + ARROW_GAP + d, 3 + d, descending);
        }
        bar.blit(&part, b.x + 2, 2);
    }
    fb.blit(&bar, r.x, r.y);
}

/// The sort arrow's place after the header's text (from the text's end), and how much further
/// from the column's right edge a number's text stands with it.
const ARROW_GAP: i32 = 9;
const ARROW_RIGHT: i32 = 14;

/// Windows 2000's sort arrow (measured pixel by pixel: `*-view-v1-details-one`, `-sort-size2`): an
/// etched triangle 8 wide and 7 high, its left edges gray and right edges white; pointing up with
/// a white base while ascending, down with a gray base while descending.
fn paint_sort_arrow(fb: &mut Framebuffer, theme: &Theme, x: i32, y: i32, descending: bool) {
    let c = &theme.colors;
    // Rows from the point: gray columns, white columns.
    const EDGES: [(&[i32], &[i32]); 6] = [(&[3], &[4]), (&[2, 3], &[4, 5]), (&[2], &[5]), (&[1, 2], &[5, 6]), (&[1], &[6]), (&[0, 1], &[6, 7])];
    for (k, (gray, white)) in EDGES.iter().enumerate() {
        let row = if descending { y + 6 - k as i32 } else { y + k as i32 };
        for &g in *gray {
            fb.set(x + g, row, c.shadow);
        }
        for &w in *white {
            fb.set(x + w, row, c.highlight);
        }
    }
    let base = if descending { y } else { y + 6 };
    fb.hline(x, base, 8, if descending { c.shadow } else { c.highlight });
}

/// Which header part is at window x `px` (`x0`: where the first column starts): a divider (the
/// column to its left, for resizing) or a button.
pub fn header_hit(widths: &[i32], x0: i32, px: i32) -> Option<HeaderPart> {
    let mut x = x0;
    for (k, &w) in widths.iter().enumerate() {
        let edge = x + w;
        // 16 pixels: 8 left of the next column's first pixel to 7 right of it, in all three
        // (measured on the Reference PCs, `private/probes/measure17`).
        if (edge - DIVIDER_LEFT..=edge + DIVIDER_RIGHT).contains(&px) {
            return Some(HeaderPart::Divider(k));
        }
        if px >= x && px < edge {
            return Some(HeaderPart::Button(k));
        }
        x = edge;
    }
    (px >= x).then_some(HeaderPart::Filler)
}

/// A part of Details' header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeaderPart {
    Button(usize),
    Divider(usize),
    Filler,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_and_dates_read_as_each_windows_wrote_them() {
        assert_eq!(kb_text(Era::Win95, 0), "0KB");
        assert_eq!(kb_text(Era::Win95, 22), "1KB");
        assert_eq!(kb_text(Era::Win95, 3_446), "4KB");
        assert_eq!(kb_text(Era::Win95, 1_638_400), "1,600KB");
        assert_eq!(kb_text(Era::Win98, 2_801_664), "2,736KB");
        assert_eq!(kb_text(Era::Win2000, 2_383_632), "2,328 KB");
        assert_eq!(date_text(Era::Win95, local_filetime(2026, 11, 14, 14, 39)), "11/14/26 2:39 PM");
        assert_eq!(date_text(Era::Win95, local_filetime(1996, 8, 24, 11, 11)), "8/24/96 11:11 AM");
        assert_eq!(date_text(Era::Win2000, local_filetime(2003, 7, 14, 4, 0)), "7/14/2003 4:00 AM");
        assert_eq!(date_text(Era::Win2000, local_filetime(2026, 1, 1, 0, 5)), "1/1/2026 12:05 AM");
        assert_eq!(date_text(Era::Win98, local_filetime(2000, 2, 29, 12, 0)), "2/29/00 12:00 PM");
        assert_eq!(date_text(Era::Win98, 0), "");
    }

    #[test]
    fn header_parts_and_dividers() {
        let w = [120, 60, 120, 120];
        assert_eq!(header_hit(&w, 72, 80), Some(HeaderPart::Button(0)));
        assert_eq!(header_hit(&w, 72, 192), Some(HeaderPart::Divider(0)));
        // The Reference PCs: 8 left to 7 right of the next column's first pixel (192).
        assert_eq!(header_hit(&w, 72, 184), Some(HeaderPart::Divider(0)));
        assert_eq!(header_hit(&w, 72, 183), Some(HeaderPart::Button(0)));
        assert_eq!(header_hit(&w, 72, 199), Some(HeaderPart::Divider(0)));
        assert_eq!(header_hit(&w, 72, 200), Some(HeaderPart::Button(1)));
        assert_eq!(header_hit(&w, 72, 225), Some(HeaderPart::Button(1)));
        assert_eq!(header_hit(&w, 72, 600), Some(HeaderPart::Filler));
    }
}
