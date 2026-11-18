//! "Remove Shortcuts/Folders": the window that the Remove... button of Taskbar Properties
//! (Start Menu Programs; in Windows 2000: Advanced) opened in Windows 95, 98 and 2000. It shows
//! the Start menu's programs as a tree; the selected program or folder goes at a click on
//! Remove. Read pixel for pixel on the Reference PCs (captures
//! `vm/*-truecolor-remove-tree-*.png`; docs/theme-spec.md, "Remove Shortcuts/Folders"): the
//! three Windows show the same window, each with its own tree (`tree`).
//!
//! Windows deleted the shortcut file. Here Remove takes the program out of what Bevel95's
//! Programs menu shows (it loses its tick on Bevel95 Settings' Programs page, where it can be
//! ticked again): the platform layer does that with `selected_places`, then `remove_selected`
//! takes the row out of the tree.

use crate::dialog::{self, TitleButtons};
use crate::fb::{Framebuffer, Icon, Rect};
use crate::kit::{self, Effect, Form, Key, Mods, Press};
use crate::programs::{self, Cursor, Folder, Item, Row, RowKind};
use crate::scroll::ScrollBar;
use crate::text::Fonts;
use crate::theme::{Era, Theme, rgb};
use crate::tree::{self, TreeRow, TreeStyle, Zone};

pub const TITLE: &str = "Remove Shortcuts/Folders";
/// The window, the same in every look: 324×350; two lines of text from (18, 38); the tree in
/// its sunken frame; Remove and Close 75×23 at y 308, Close 18 from the right edge.
const SIZE: (i32, i32) = (324, 350);
const TEXT: [&str; 2] = ["&To remove an item from the Start menu, select the item and", "click Remove."];
const TEXT_AT: (i32, i32) = (18, 38);
const TREE: Rect = Rect::new(18, 74, 288, 224);
/// Rows the tree shows in full (a part of one more shows below them): the scroll bar's page.
const PAGE: usize = 13;
const REMOVE: Rect = Rect::new(150, 308, 75, 23);
const CLOSE: Rect = Rect::new(231, 308, 75, 23);
const REMOVE_LABEL: &str = "&Remove";
const CLOSE_LABEL: &str = "Close";
/// The tree's first row, the Start menu itself. It and its "Programs" are always there.
const START_MENU: &str = "Start Menu";

/// The parts of the window that the mouse and the keyboard act on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    TitleHelp,
    TitleClose,
    /// The tree (one stop for the keyboard) and a row of it (its number in `rows`).
    Tree,
    Row(usize),
    ScrollUp,
    ScrollDown,
    ScrollPageUp,
    ScrollPageDown,
    ScrollThumb,
    Remove,
    Close,
}

/// The window's tree: "Start Menu", under it "Programs" with what the Programs menu shows
/// (`tree`). Programs is open and its folders are closed, as the window opened in all three
/// Windows. `icon` gives a program's picture.
pub fn rows(tree: &Folder, mut icon: impl FnMut(&Item) -> Option<Icon>) -> Vec<Row> {
    let mut rows = programs::rows(tree, None, |item| (false, icon(item)));
    for row in &mut rows {
        row.depth += 1;
    }
    let start_menu = Row { depth: 0, name: START_MENU.to_string(), place: String::new(), ..rows[0].clone() };
    rows.insert(0, start_menu);
    programs::mark_last(&mut rows);
    rows
}

/// The window as it is now.
#[derive(Clone, Debug)]
pub struct RemoveWindow {
    /// The look it is drawn in.
    pub era: Era,
    /// The tree's rows, those in closed folders included.
    pub rows: Vec<Row>,
    /// The selected row (its number in `rows`; always one that shows).
    pub sel: usize,
    /// How many of the rows that show are scrolled out above the tree.
    pub top: usize,
    pub focus: Part,
    pub pressed: Option<Part>,
    /// Underline access keys and draw focus rectangles: Windows 95 and 98 always do; Windows
    /// 2000 once the keyboard has been used.
    pub show_keys: bool,
    /// Where the window's top-left corner is on the screen, in 1× pixels.
    pub screen: (i32, i32),
    parts: Vec<(Part, Rect)>,
}

impl RemoveWindow {
    /// The window as it opens: Programs selected, the focus in the tree.
    pub fn new(era: Era, rows: Vec<Row>) -> RemoveWindow {
        let sel = rows.iter().rposition(|row| row.kind == RowKind::Root).unwrap_or(0);
        let mut window = RemoveWindow {
            era,
            rows,
            sel,
            top: 0,
            focus: Part::Tree,
            pressed: None,
            show_keys: era.always_shows_keys(),
            screen: (0, 0),
            parts: Vec::new(),
        };
        window.lay_out();
        window
    }

    fn cursor(&mut self) -> Cursor<'_> {
        Cursor { rows: &mut self.rows, sel: &mut self.sel, top: &mut self.top, page: PAGE }
    }

    /// Whether row `i` has rows of its own (a plus or minus box): the next row stands further in.
    fn has_rows(&self, i: usize) -> bool {
        self.rows.get(i + 1).is_some_and(|next| next.depth > self.rows[i].depth)
    }

    /// The places (as programs.txt names them) of what Remove would take away now: the
    /// selected program, or every program in the selected folder. None for "Start Menu" and
    /// "Programs", which stay.
    pub fn selected_places(&self) -> Vec<String> {
        if self.rows.get(self.sel).is_none_or(|row| row.kind == RowKind::Root) {
            return Vec::new();
        }
        let end = programs::inside(&self.rows, self.sel).end;
        let programs = self.rows[self.sel..end].iter().filter(|row| row.kind == RowKind::Item);
        programs.map(|row| row.place.clone()).collect()
    }

    /// The selected row has been removed: it leaves the tree with everything in it, and so
    /// does each folder that is left with nothing in it. The selection passes to the row that
    /// came after it (the last row, if none did).
    pub fn remove_selected(&mut self) {
        if self.selected_places().is_empty() {
            return;
        }
        let end = programs::inside(&self.rows, self.sel).end;
        // The row after what goes: a row of the same folder, or of one further out, so it shows.
        let next = self.rows.get(end).map(|row| (row.kind, row.depth, row.name.clone(), row.place.clone()));
        self.rows.drain(self.sel..end);
        // A folder with no row further in after it has nothing left ("Start Menu" and
        // "Programs" stay whatever happens).
        while let Some(empty) =
            (0..self.rows.len()).find(|&i| self.rows[i].kind == RowKind::Folder && !self.has_rows(i))
        {
            self.rows.remove(empty);
        }
        programs::mark_last(&mut self.rows);
        let shows = programs::visible(&self.rows);
        let same = |row: &Row| {
            next.as_ref().is_some_and(|n| (row.kind, row.depth, &row.name, &row.place) == (n.0, n.1, &n.2, &n.3))
        };
        self.sel = self.rows.iter().position(same).or(shows.last().copied()).unwrap_or(0);
        self.top = self.top.min(shows.len().saturating_sub(PAGE));
        let sel = self.sel;
        self.cursor().show(sel);
        if !self.enabled(self.focus) {
            self.focus = Part::Tree;
        }
        self.lay_out();
    }

    /// The Programs menu changed while the window is open (Add...'s wizard made a shortcut):
    /// the tree takes `rows` (`rows`), its folders open or closed as before, the selection on
    /// the same row if it is still there and shows.
    pub fn set_rows(&mut self, mut rows: Vec<Row>) {
        let key = |row: &Row| (row.kind, row.depth, row.place.to_lowercase());
        for row in rows.iter_mut().filter(|row| row.kind != RowKind::Item) {
            if let Some(old) = self.rows.iter().find(|old| key(old) == key(row)) {
                row.open = old.open;
            }
        }
        let selected = self.rows.get(self.sel).map(key);
        self.rows = rows;
        let shows = programs::visible(&self.rows);
        let now = selected.and_then(|was| self.rows.iter().position(|row| key(row) == was));
        let root = self.rows.iter().rposition(|row| row.kind == RowKind::Root).unwrap_or(0);
        self.sel = now.filter(|at| shows.contains(at)).unwrap_or(root);
        self.top = self.top.min(shows.len().saturating_sub(PAGE));
        if !self.enabled(self.focus) {
            self.focus = Part::Tree;
        }
        self.lay_out();
    }

    fn inside() -> Rect {
        TREE.inset(2, 2, 2, 2)
    }

    /// The tree's scroll bar: none while every row that shows fits.
    fn scroll(&self) -> Option<ScrollBar> {
        let (inside, total) = (Self::inside(), programs::visible(&self.rows).len());
        let bar = Rect::new(inside.right() - 16, inside.y, 16, inside.h);
        (total > PAGE).then_some(ScrollBar { bar, total, page: PAGE })
    }

    fn rows_width(&self) -> i32 {
        Self::inside().w - if self.scroll().is_some() { 16 } else { 0 }
    }

    fn lay_out(&mut self) {
        let (inside, w) = (Self::inside(), self.rows_width());
        let mut parts = Vec::new();
        // One more row than fit shows its upper part at the bottom of the tree.
        for (at, &row) in programs::visible(&self.rows).iter().enumerate().skip(self.top).take(PAGE + 1) {
            let y = inside.y + (at - self.top) as i32 * tree::ROW_H;
            parts.push((Part::Row(row), Rect::new(inside.x, y, w, tree::ROW_H.min(inside.bottom() - y))));
        }
        if let Some(scroll) = self.scroll() {
            let r = scroll.rects(self.top);
            parts.extend([
                (Part::ScrollUp, r.up),
                (Part::ScrollDown, r.down),
                (Part::ScrollThumb, r.thumb),
                (Part::ScrollPageUp, r.page_up),
                (Part::ScrollPageDown, r.page_down),
            ]);
        }
        let (help, close) = dialog::title_button_rects(SIZE.0);
        parts.extend([
            (Part::Tree, TREE),
            (Part::Remove, REMOVE),
            (Part::Close, CLOSE),
            (Part::TitleHelp, help),
            (Part::TitleClose, close),
        ]);
        self.parts = parts;
    }

    fn part_rect(&self, part: Part) -> Option<Rect> {
        self.parts.iter().find(|(p, _)| *p == part).map(|&(_, r)| r)
    }

    /// What of row `i` is at `x` (in the window): its plus/minus box, or the row itself
    /// (everything from its picture on selects it).
    fn zone(&self, i: usize, x: i32) -> Zone {
        let (Some(r), Some(row)) = (self.part_rect(Part::Row(i)), self.rows.get(i)) else { return Zone::Blank };
        tree::zone(TreeStyle::of(self.era), row.depth, self.has_rows(i), false, r.w, x - r.x)
    }

    fn scroll_by(&mut self, delta: i32) {
        let max = programs::visible(&self.rows).len().saturating_sub(PAGE) as i32;
        self.top = (self.top as i32 + delta).clamp(0, max) as usize;
    }
}

impl kit::Form for RemoveWindow {
    type Part = Part;

    fn title(&self) -> &str {
        TITLE
    }
    fn size(&self) -> (i32, i32) {
        SIZE
    }
    fn layout(&mut self, _fonts: &Fonts) {
        self.lay_out();
    }
    fn set_screen(&mut self, x: i32, y: i32) {
        self.screen = (x, y);
    }

    fn paint(&self, fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts) {
        let (c, font, keys) = (&theme.colors, fonts.regular, self.show_keys);
        let held = |part: Part| self.pressed == Some(part);
        let buttons = TitleButtons {
            help: self.part_rect(Part::TitleHelp).map(|r| (r, held(Part::TitleHelp))),
            close: self.part_rect(Part::TitleClose).map(|r| (r, held(Part::TitleClose))),
        };
        dialog::window_frame(fb, theme, fonts, self.era, TITLE, &buttons);
        for (i, line) in TEXT.iter().enumerate() {
            font.draw_label(fb, (TEXT_AT.0, TEXT_AT.1 + font.height * i as i32), line, i32::MAX, c.text, keys);
        }

        fb.frame(TREE, c.shadow, c.highlight);
        fb.frame(TREE.inset(1, 1, 1, 1), c.dark_shadow, c.light);
        let (inside, w) = (Self::inside(), self.rows_width());
        let mut rows = Framebuffer::new(w, inside.h, rgb(0xFFFFFF));
        let style = TreeStyle::of(self.era);
        let (start_menu, folder) = (crate::icons::tree_start_menu(self.era), crate::icons::tree_group(self.era));
        let arrow = crate::icons::shortcut_arrow();
        let screen = (self.screen.0 + inside.x, self.screen.1 + inside.y);
        for (at, &i) in programs::visible(&self.rows).iter().enumerate().skip(self.top).take(PAGE + 1) {
            let (row, y) = (&self.rows[i], (at - self.top) as i32 * tree::ROW_H);
            let tree_row = TreeRow {
                depth: row.depth,
                open: self.has_rows(i).then_some(row.open),
                last: programs::lasts(&self.rows, i),
                tick: None,
                icon: match row.kind {
                    RowKind::Item => row.icon.as_ref(),
                    RowKind::Root if row.name == START_MENU => Some(&start_menu),
                    _ => Some(&folder),
                },
                label: &row.name,
                // The selection stays in sight, on gray, while a button has the focus.
                selected: i == self.sel,
                focused: self.focus == Part::Tree,
            };
            tree::paint_row(&mut rows, theme, font, style, &tree_row, y, w, screen);
            // The tree shows the Start menu's shortcut files, so each program's picture has the
            // shortcut arrow that the menu itself leaves out.
            if row.kind == RowKind::Item {
                crate::colors16::draw_picture(&mut rows, theme, &arrow, style.content_x(row.depth), y);
            }
        }
        fb.blit(&rows, inside.x, inside.y);
        if let Some(scroll) = self.scroll() {
            scroll.paint(fb, c, self.top, held(Part::ScrollUp), held(Part::ScrollDown));
        }

        let framed = self.enter();
        for (part, rect, label) in [(Part::Remove, REMOVE, REMOVE_LABEL), (Part::Close, CLOSE, CLOSE_LABEL)] {
            let focused = self.focus == part && keys;
            dialog::push_button(
                fb,
                theme,
                font,
                rect,
                label,
                framed == part,
                held(part),
                focused,
                self.enabled(part),
                keys,
            );
        }
    }

    fn hit(&self, x: i32, y: i32) -> Option<Part> {
        self.parts.iter().find(|(_, r)| r.contains(x, y)).map(|&(p, _)| p)
    }
    fn rect(&self, part: Part) -> Option<Rect> {
        self.part_rect(part)
    }
    /// Remove can be used while a program or a folder of them is selected.
    fn enabled(&self, part: Part) -> bool {
        part != Part::Remove || self.rows.get(self.sel).is_some_and(|row| row.kind != RowKind::Root)
    }
    fn focus(&self) -> Part {
        self.focus
    }
    fn set_focus(&mut self, part: Part) {
        self.focus = part;
    }
    fn tab_stops(&self) -> Vec<Part> {
        [Part::Tree, Part::Remove, Part::Close].into_iter().filter(|&part| self.enabled(part)).collect()
    }
    fn set_pressed(&mut self, part: Option<Part>) {
        self.pressed = part;
    }
    fn show_keys(&mut self) {
        self.show_keys = true;
    }
    /// Enter presses the focused button; from the tree, Close (the window's default button).
    fn enter(&self) -> Part {
        if self.focus == Part::Remove && self.enabled(Part::Remove) { Part::Remove } else { Part::Close }
    }
    fn escape(&self) -> Part {
        Part::Close
    }

    fn key(&mut self, key: Key, _mods: Mods) -> Option<Vec<Effect<Part>>> {
        if self.focus != Part::Tree {
            return None;
        }
        let all = self.rows.len() as i32;
        let mut cursor = self.cursor();
        match key {
            Key::Up | Key::Down => cursor.move_by(if key == Key::Down { 1 } else { -1 }),
            Key::Left | Key::Right => cursor.step(key == Key::Right),
            Key::PageUp | Key::PageDown => {
                cursor.move_by(if key == Key::PageDown { PAGE as i32 - 1 } else { 1 - PAGE as i32 })
            }
            Key::Home | Key::End => cursor.move_by(if key == Key::End { all } else { -all }),
            _ => return None,
        }
        Some(Vec::new())
    }
    /// In the tree a letter goes to the next row of that letter, and "+", "-" and "*" open and
    /// close folders; on a button a letter is an access key, with or without Alt.
    fn typed(&mut self, ch: char) -> Vec<Effect<Part>> {
        if self.focus != Part::Tree {
            return if ch.is_alphanumeric() { self.access(ch) } else { Vec::new() };
        }
        let sel = self.sel;
        let mut cursor = self.cursor();
        match ch {
            '+' => cursor.open(sel, true),
            '-' => cursor.open(sel, false),
            '*' => cursor.open_all(),
            letter if letter.is_alphanumeric() => {
                cursor.find(letter);
            }
            _ => {}
        }
        Vec::new()
    }
    fn access(&mut self, ch: char) -> Vec<Effect<Part>> {
        self.show_keys = true;
        match ch.to_ascii_lowercase() {
            't' => self.focus = Part::Tree,
            'r' if self.enabled(Part::Remove) => {
                self.focus = Part::Remove;
                return vec![Effect::Activate(Part::Remove)];
            }
            _ => {}
        }
        Vec::new()
    }
    fn space(&mut self) -> Vec<Effect<Part>> {
        match self.focus {
            part @ (Part::Remove | Part::Close) => kit::activate(self, part),
            _ => Vec::new(),
        }
    }
    fn press(&mut self, part: Option<Part>, x: i32, _y: i32) -> Press<Part> {
        match part {
            Some(Part::Row(i)) => {
                self.focus = Part::Tree;
                match self.zone(i, x) {
                    Zone::Button => {
                        let open = self.rows[i].open;
                        self.cursor().open(i, !open);
                    }
                    Zone::Item | Zone::Tick => self.sel = i,
                    Zone::Blank => {}
                }
            }
            Some(part @ (Part::ScrollUp | Part::ScrollDown)) => {
                self.scroll_by(if part == Part::ScrollUp { -1 } else { 1 });
                return Press::Hold(part);
            }
            Some(part @ (Part::ScrollPageUp | Part::ScrollPageDown)) => {
                self.scroll_by(if part == Part::ScrollPageUp { -(PAGE as i32) } else { PAGE as i32 });
            }
            Some(Part::ScrollThumb) => return Press::Grab(Part::ScrollThumb, self.top as i32),
            Some(Part::Tree) => self.focus = Part::Tree,
            Some(part) if self.enabled(part) => {
                if matches!(part, Part::Remove | Part::Close) {
                    self.focus = part;
                }
                return Press::Hold(part);
            }
            Some(_) => {}
            None => return Press::Nothing,
        }
        Press::Done
    }
    /// The second press of a double-click on a folder's picture or name: its rows show or are
    /// put away.
    fn double_click(&mut self, x: i32, y: i32) -> Option<Vec<Effect<Part>>> {
        let Some(Part::Row(i)) = self.hit(x, y) else { return None };
        if self.zone(i, x) != Zone::Item || !self.has_rows(i) {
            return None;
        }
        let open = self.rows[i].open;
        (self.focus, self.sel) = (Part::Tree, i);
        self.cursor().open(i, !open);
        Some(Vec::new())
    }
    fn release(&mut self, part: Part) -> Vec<Effect<Part>> {
        match part {
            Part::Remove | Part::Close | Part::TitleHelp | Part::TitleClose => vec![Effect::Activate(part)],
            _ => Vec::new(),
        }
    }
    fn drag(&mut self, _part: Part, grab_y: i32, grab_top: i32, _x: i32, y: i32) -> bool {
        let Some(scroll) = self.scroll() else { return false };
        let top = scroll.dragged(grab_top, y - grab_y);
        let changed = top != self.top;
        self.top = top;
        changed
    }
    fn wheel(&mut self, notches: i32) {
        self.scroll_by(-3 * notches);
    }
}

/// A held scroll bar button scrolls on (the window's host calls this at each tick of its
/// timer once the button has been held a moment). True if the tree moved.
pub fn scroll_on(window: &mut RemoveWindow) -> bool {
    let before = window.top;
    match window.pressed {
        Some(Part::ScrollUp) => window.scroll_by(-1),
        Some(Part::ScrollDown) => window.scroll_by(1),
        _ => {}
    }
    window.top != before
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small Programs menu: Accessories (Games with two programs, and Notepad), StartUp with
    /// one program, and two programs in Programs itself.
    fn menu() -> Folder {
        let item = |name: &str| Item { name: name.into(), target: format!("C:\\{name}.lnk") };
        let mut tree = Folder::default();
        tree.add(&["Accessories", "Games"], item("Minesweeper"));
        tree.add(&["Accessories", "Games"], item("Solitaire"));
        tree.add(&["Accessories"], item("Notepad"));
        tree.add(&["StartUp"], item("Sync"));
        tree.add(&[], item("MS-DOS Prompt"));
        tree.add(&[], item("Windows Explorer"));
        tree.sort();
        tree
    }

    fn names(window: &RemoveWindow) -> Vec<(usize, &str)> {
        programs::visible(&window.rows).iter().map(|&i| (window.rows[i].depth, window.rows[i].name.as_str())).collect()
    }

    #[test]
    fn every_look_begins_its_tree_as_windows_did() {
        // "Start Menu", under it Programs (open, selected), its folders closed.
        for era in Era::ALL {
            let window = RemoveWindow::new(era, rows(&menu(), |_| None));
            let start = [(0, "Start Menu"), (1, "Programs"), (2, "Accessories"), (2, "StartUp")];
            assert_eq!(names(&window)[..4], start);
            assert_eq!(names(&window)[4..], [(2, "MS-DOS Prompt"), (2, "Windows Explorer")]);
            assert_eq!((window.sel, window.focus), (1, Part::Tree));
            assert!(!window.enabled(Part::Remove), "Programs itself stays");
            assert_eq!(window.tab_stops(), [Part::Tree, Part::Close]);
            assert_eq!(window.enter(), Part::Close);
        }
    }

    #[test]
    fn remove_takes_the_selected_program_or_folder_away() {
        let mut w = RemoveWindow::new(Era::Win2000, rows(&menu(), |_| None));
        let plain = Mods::default();
        // Down to Accessories, Right opens it, Right again goes in (Games), and once more into
        // Games: Minesweeper.
        for key in [Key::Down, Key::Right, Key::Right, Key::Right, Key::Right] {
            w.key(key, plain);
        }
        assert_eq!(w.rows[w.sel].name, "Minesweeper");
        assert!(w.enabled(Part::Remove));
        assert_eq!(w.selected_places(), ["Accessories\\Games\\Minesweeper"]);
        assert_eq!(w.access('r'), vec![Effect::Activate(Part::Remove)]);
        assert_eq!(w.focus, Part::Remove, "Alt+R goes to the button, as in Windows");
        w.remove_selected();
        assert_eq!(w.rows[w.sel].name, "Solitaire", "the next row comes into its place");
        // The last program of a folder: the folder goes with it, and the selection to the row
        // that came after it.
        w.remove_selected();
        assert_eq!(w.rows[w.sel].name, "Notepad");
        assert!(!w.rows.iter().any(|row| row.name == "Games"), "nothing was left in Games");
        // A folder: every program in it.
        w.set_focus(Part::Tree);
        w.key(Key::Left, plain);
        assert_eq!(
            (w.rows[w.sel].name.as_str(), w.selected_places()),
            ("Accessories", vec!["Accessories\\Notepad".to_string()])
        );
        w.remove_selected();
        let left = [(0, "Start Menu"), (1, "Programs"), (2, "StartUp"), (2, "MS-DOS Prompt"), (2, "Windows Explorer")];
        assert_eq!(names(&w), left);
        assert_eq!(w.rows[w.sel].name, "StartUp");
        // The last row of all: the selection goes to the one above.
        w.key(Key::End, plain);
        w.remove_selected();
        assert_eq!(w.rows[w.sel].name, "MS-DOS Prompt");
        // "Start Menu" and "Programs" stay, whatever is asked.
        w.key(Key::Home, plain);
        assert!(w.selected_places().is_empty() && !w.enabled(Part::Remove));
        w.remove_selected();
        assert_eq!(w.rows.len(), 5, "Start Menu, Programs, StartUp with its program, MS-DOS Prompt");
    }

    #[test]
    fn a_new_program_joins_the_open_tree() {
        let mut w = RemoveWindow::new(Era::Win98, rows(&menu(), |_| None));
        let plain = Mods::default();
        // Down to Accessories, Right opens it, Right goes in (Games, closed), Down: Notepad.
        for key in [Key::Down, Key::Right, Key::Right, Key::Down] {
            w.key(key, plain);
        }
        assert_eq!(w.rows[w.sel].name, "Notepad");
        // Add...'s wizard made a shortcut in Programs itself while the window was open.
        let mut tree = menu();
        tree.add(&[], Item { name: "Calculator".into(), target: "C:\\Calculator.lnk".into() });
        tree.sort();
        w.set_rows(rows(&tree, |_| None));
        assert_eq!(w.rows[w.sel].name, "Notepad", "the selection stays on its row");
        assert!(w.rows[2].open && w.rows[2].name == "Accessories", "an open folder stays open");
        assert!(names(&w).contains(&(2, "Calculator")));
        // A selected row that is gone: the selection goes to Programs.
        w.set_rows(rows(&Folder::default(), |_| None));
        assert_eq!((w.sel, w.focus), (1, Part::Tree));
    }

    #[test]
    fn the_tree_takes_clicks_and_keys() {
        let mut w = RemoveWindow::new(Era::Win98, rows(&menu(), |_| None));
        // Row 2 of the tree, Accessories, two steps in: its plus box is under Programs' picture
        // (19 + 4 to 19 + 12 from the tree's inside), its picture from 38.
        let r = w.rect(Part::Row(2)).unwrap();
        assert_eq!(r, Rect::new(20, 76 + 2 * 16, 284, 16));
        w.press(Some(Part::Row(2)), r.x + 27, r.y + 8);
        assert!(w.rows[2].open && w.sel == 1, "the box opens the folder; the selection stays");
        w.press(Some(Part::Row(2)), r.x + 27, r.y + 8);
        assert!(!w.rows[2].open);
        w.press(Some(Part::Row(2)), r.x + 60, r.y + 8);
        assert_eq!(w.sel, 2);
        assert_eq!(w.double_click(r.x + 60, r.y + 8), Some(vec![]));
        assert!(w.rows[2].open);
        w.layout(&Fonts { regular: &font(), bold: &font() });
        // Letters go to rows; "-" closes, "+" opens; Tab goes to Remove (a folder is
        // selected), where a letter is an access key again.
        w.typed('w');
        assert_eq!(w.rows[w.sel].name, "Windows Explorer");
        w.typed('a');
        w.typed('-');
        assert!(!w.rows[2].open);
        assert_eq!(w.tab_stops(), [Part::Tree, Part::Remove, Part::Close]);
        w.set_focus(Part::Remove);
        assert_eq!(w.enter(), Part::Remove);
        assert_eq!(w.typed('r'), vec![Effect::Activate(Part::Remove)]);
        assert_eq!(w.typed('t'), vec![]);
        assert_eq!(w.focus, Part::Tree);
        assert_eq!(w.escape(), Part::Close);
    }

    #[test]
    fn a_long_tree_scrolls() {
        let item = |name: String| Item { target: format!("C:\\{name}.lnk"), name };
        let mut tree = Folder::default();
        for i in 0..30 {
            tree.add(&[], item(format!("Program {i:02}")));
        }
        let mut w = RemoveWindow::new(Era::Win95, rows(&tree, |_| None));
        // 32 rows, 13 in full: the scroll bar beside 268 of the tree's 284.
        assert_eq!(w.rect(Part::Row(1)).unwrap().w, 268);
        assert!(w.rect(Part::Row(13)).is_some_and(|r| r.h == 12), "the 14th row shows its upper part");
        assert_eq!(w.press(Some(Part::ScrollDown), 0, 0), Press::Hold(Part::ScrollDown));
        w.set_pressed(Some(Part::ScrollDown));
        assert!(scroll_on(&mut w) && w.top == 2);
        w.wheel(-1);
        assert_eq!(w.top, 5);
        w.key(Key::End, Mods::default());
        assert_eq!((w.sel, w.top), (31, 19));
        w.wheel(100);
        assert_eq!(w.top, 0);
    }

    fn font() -> crate::text::Font {
        let glyph = crate::text::Glyph { ch: '?', advance: 6, ink_x: 0, ink_w: 0, ink: Vec::new() };
        crate::text::Font::new("test".into(), false, 13, 11, vec![glyph]).unwrap()
    }
}
