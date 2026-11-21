//! Explorer's left pane: the tree of all folders (Windows 95's "All Folders", 98's "All Folders"
//! bar with its close button, 2000's "Folders" bar), and the bar between it and the icons.
//! Measured on the Reference PCs (`private/reference/refs/vm/<era>-truecolor-tree-*.png`, numbers in
//! docs/theme-spec.md, "Explorer's tree"):
//!
//! - The rows are Windows' tree view (`crate::tree`, the same measures as the Remove
//!   Shortcuts/Folders window): the Desktop at the top without lines or box, each folder one step
//!   further in, a plus/minus box at each row that has folders of its own.
//! - A row's label is cut at the pane's edge, not shortened with "...": the tree scrolls across
//!   instead (`*-tree-right`: "ICW-Internet Conne").
//! - The selection shows only while the tree has the keyboard (Tab and F6 take it to the icons
//!   and back: `*-tree-tab`); a selected folder shows the open folder's picture.
//! - Opening a folder scrolls the tree as little as needed for its last row to show, keeping the
//!   folder itself in sight (`win95-truecolor-tree-windows`).
//! - Which rows have a plus box is decided when they are read: a folder with folders inside it
//!   (`Win95` has none: no box), a drive that takes disks always (A: keeps its plus after "not
//!   accessible").
//!
//! The rows are read by the platform, off the window's thread: `wanted` says which folders' rows
//! are needed, `set_rows` takes them in when they come, `unreadable` when a folder could not be
//! read. `follow` opens the tree down to the place the icons show.

use super::art::{self, DriveKind, Small};
use super::tree_art;
use crate::fb::{Framebuffer, Icon, Rect};
use crate::text::Font;
use crate::theme::{Colors, Era, Theme, rgb};
use crate::tree::{self, TreeRow, TreeStyle, Zone};

/// What a row stands for (its picture, and what selecting it shows).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Desktop,
    MyDocuments,
    MyComputer,
    Drive(DriveKind),
    Folder,
    ControlPanel,
    Printers,
    RecycleBin,
}

impl NodeKind {
    /// Opens a window of Windows' own: nothing here shows its contents.
    pub fn opens_windows_own(self) -> bool {
        matches!(self, NodeKind::ControlPanel | NodeKind::Printers | NodeKind::RecycleBin)
    }

    /// A folder of files that file commands work on (a folder, a drive, My Documents).
    pub fn is_folder(self) -> bool {
        matches!(self, NodeKind::Folder | NodeKind::Drive(_) | NodeKind::MyDocuments)
    }
}

/// A row as the platform reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeSpec {
    /// Its folder's path, or the place's id (`::desktop`, `::my-computer`, `::my-documents`,
    /// `::control-panel`, `::printers`, `::recycle-bin`).
    pub id: String,
    pub name: String,
    pub kind: NodeKind,
    /// It has folders of its own (a plus box).
    pub has_rows: bool,
}

/// One row of the tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub id: String,
    pub name: String,
    pub kind: NodeKind,
    pub depth: usize,
    pub open: bool,
    pub has_rows: bool,
    /// Its rows have been read (they follow it in `nodes`).
    pub read: bool,
    /// Its rows were asked for and have not come yet.
    asked: bool,
}

/// The ids compared as Windows compares paths: without regard to case.
fn same(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b) || a.to_lowercase() == b.to_lowercase()
}

/// The tree: every row read so far, in the order they are drawn (a row's rows follow it).
#[derive(Clone, Debug)]
pub struct Tree {
    pub nodes: Vec<Node>,
    /// The selected row (an index into `nodes`).
    pub sel: usize,
    /// The first row shown (an index into `visible()`).
    pub top: usize,
    /// How far the rows are scrolled across, in pixels.
    pub scroll_x: i32,
    /// The ids still to open on the way down to a place (`follow`).
    follow: Vec<String>,
    /// The row a drag of files is over: drawn selected while the pointer is on it, the selection
    /// losing its fill meanwhile (98).
    pub drop_row: Option<usize>,
}

impl Tree {
    /// A tree with the Desktop alone, open, its rows not read yet.
    pub fn new() -> Tree {
        let desktop = Node {
            id: "::desktop".into(),
            name: "Desktop".into(),
            kind: NodeKind::Desktop,
            depth: 0,
            open: true,
            has_rows: true,
            read: false,
            asked: false,
        };
        Tree { nodes: vec![desktop], sel: 0, top: 0, scroll_x: 0, follow: Vec::new(), drop_row: None }
    }

    /// The row whose id is `id`.
    pub fn find(&self, id: &str) -> Option<usize> {
        self.nodes.iter().position(|n| same(&n.id, id))
    }

    /// The first index after row `i`'s rows.
    pub fn end_of(&self, i: usize) -> usize {
        let d = self.nodes[i].depth;
        (i + 1..self.nodes.len()).find(|&j| self.nodes[j].depth <= d).unwrap_or(self.nodes.len())
    }

    /// The row `i` is in.
    pub fn parent(&self, i: usize) -> Option<usize> {
        let d = self.nodes.get(i)?.depth;
        (0..i).rev().find(|&j| self.nodes[j].depth + 1 == d)
    }

    /// The rows that show (every folder they are in is open).
    pub fn visible(&self) -> Vec<usize> {
        let mut out = Vec::with_capacity(self.nodes.len());
        let mut closed_at: Option<usize> = None;
        for (i, n) in self.nodes.iter().enumerate() {
            if let Some(d) = closed_at {
                if n.depth > d {
                    continue;
                }
                closed_at = None;
            }
            out.push(i);
            if !n.open {
                closed_at = Some(n.depth);
            }
        }
        out
    }

    /// Whether row `i` is the last of its level in the folder it is in.
    fn is_last(&self, i: usize) -> bool {
        let d = self.nodes[i].depth;
        self.nodes.get(self.end_of(i)).is_none_or(|n| n.depth < d)
    }

    /// For row `i` and each folder it is in, from the outermost: whether it is the last of its
    /// level (where the lines down end).
    pub fn lasts(&self, i: usize) -> Vec<bool> {
        let mut out = vec![self.is_last(i)];
        let mut at = i;
        while let Some(p) = self.parent(at) {
            out.push(self.is_last(p));
            at = p;
        }
        out.reverse();
        out
    }

    /// The chain of rows from the Desktop down to row `i`.
    pub fn chain(&self, i: usize) -> Vec<usize> {
        let mut out = vec![i];
        let mut at = i;
        while let Some(p) = self.parent(at) {
            out.push(p);
            at = p;
        }
        out.reverse();
        out
    }

    /// The folders whose rows are needed now (open and not read), each asked for once.
    pub fn wanted(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        for n in &mut self.nodes {
            if n.open && !n.read && !n.asked && n.has_rows {
                n.asked = true;
                out.push(n.id.clone());
            }
        }
        out
    }

    /// Ids of every open row whose rows were read (Refresh reads them again).
    pub fn open_ids(&self) -> Vec<String> {
        self.nodes.iter().filter(|n| n.open && n.read).map(|n| n.id.clone()).collect()
    }

    /// The rows of the folder `id` were read: they replace what it held. Rows still there keep
    /// their own rows and whether they are open; new ones come closed; the selection stays on
    /// its row, or goes to the folder if its row is gone. Continues a `follow`.
    pub fn set_rows(&mut self, id: &str, rows: Vec<NodeSpec>) {
        let Some(i) = self.find(id) else { return };
        let selected = self.nodes.get(self.sel).map(|n| n.id.clone());
        let end = self.end_of(i);
        let depth = self.nodes[i].depth + 1;
        let old: Vec<Node> = self.nodes.drain(i + 1..end).collect();
        let mut fresh = Vec::new();
        for spec in rows {
            // The old row and its rows, if it is still there.
            let at = old.iter().position(|n| n.depth == depth && same(&n.id, &spec.id));
            match at {
                Some(a) => {
                    let stop = (a + 1..old.len()).find(|&j| old[j].depth <= depth).unwrap_or(old.len());
                    let mut head = old[a].clone();
                    head.name = spec.name;
                    head.kind = spec.kind;
                    head.has_rows = spec.has_rows || (head.read && stop > a + 1);
                    if !head.has_rows {
                        head.open = false;
                    }
                    fresh.push(head);
                    fresh.extend(old[a + 1..stop].iter().cloned());
                }
                None => fresh.push(Node {
                    id: spec.id,
                    name: spec.name,
                    kind: spec.kind,
                    depth,
                    open: false,
                    has_rows: spec.has_rows,
                    read: false,
                    asked: false,
                }),
            }
        }
        let n = &mut self.nodes[i];
        n.read = true;
        n.asked = false;
        n.has_rows = !fresh.is_empty() || matches!(n.kind, NodeKind::Drive(DriveKind::Floppy | DriveKind::Floppy525 | DriveKind::Removable | DriveKind::CdRom));
        if fresh.is_empty() {
            n.open = false;
        }
        self.nodes.splice(i + 1..i + 1, fresh);
        self.sel = selected.and_then(|s| self.find(&s)).unwrap_or(i);
        self.keep_selection_shown();
        self.go_on_following();
        self.clamp_top();
    }

    /// The folder `id` could not be read (a drive without a disk): it closes, its rows go; it
    /// keeps its plus box, as Windows 95 did after "not accessible".
    pub fn unreadable(&mut self, id: &str) {
        let Some(i) = self.find(id) else { return };
        let selected = self.nodes.get(self.sel).map(|n| n.id.clone());
        let end = self.end_of(i);
        self.nodes.drain(i + 1..end);
        let n = &mut self.nodes[i];
        n.open = false;
        n.read = false;
        n.asked = false;
        // The selection stays on its row by id (the rows below moved up), or goes to the folder if
        // it was inside it (the third safety review of 2026-11-18, A2: it kept its index, so a key's
        // command then acted on another folder).
        self.sel = selected.and_then(|s| self.find(&s)).unwrap_or(i);
        if self.follow.iter().any(|f| same(f, id)) {
            self.follow.clear();
        }
        self.clamp_top();
    }

    /// The selection is inside a folder that closed: it goes to that folder.
    fn keep_selection_shown(&mut self) {
        let chain = self.chain(self.sel.min(self.nodes.len().saturating_sub(1)));
        if let Some(&closed) = chain[..chain.len().saturating_sub(1)].iter().find(|&&c| !self.nodes[c].open) {
            self.sel = closed;
        }
    }

    /// Opens or closes row `i` (the plus/minus box, a double-click, + and -). True if the
    /// selection moved (it was inside the folder that closed).
    pub fn toggle(&mut self, i: usize, page: usize) -> bool {
        let Some(n) = self.nodes.get(i) else { return false };
        if n.open {
            self.close(i)
        } else {
            self.open(i, page);
            false
        }
    }

    /// Closes row `i`; the selection, if inside it, goes to it (true then).
    pub fn close(&mut self, i: usize) -> bool {
        if i >= self.nodes.len() || !self.nodes[i].open {
            return false;
        }
        self.nodes[i].open = false;
        let before = self.sel;
        if self.sel > i && self.sel < self.end_of(i) {
            self.sel = i;
        }
        self.clamp_top();
        before != self.sel
    }

    /// Opens row `i` if it has rows, and scrolls as little as needed for its last row to show,
    /// keeping it in sight (`page`: how many whole rows show).
    pub fn open(&mut self, i: usize, page: usize) {
        if i >= self.nodes.len() || !self.nodes[i].has_rows {
            return;
        }
        self.nodes[i].open = true;
        self.reveal_rows(i, page);
    }

    /// Scrolls so that row `i`'s last row shows, without taking `i` out of sight.
    fn reveal_rows(&mut self, i: usize, page: usize) {
        let vis = self.visible();
        let (Some(at), page) = (vis.iter().position(|&v| v == i), page.max(1)) else { return };
        let end = self.end_of(i);
        let last = vis.iter().rposition(|&v| v > i && v < end).unwrap_or(at);
        if last >= self.top + page {
            self.top = (last + 1 - page).min(at);
        }
        self.clamp_top();
    }

    /// Opens row `i` and every folder in it that has been read (the "*" key: Windows read all of
    /// them; here the unread ones are read as they open).
    pub fn open_all(&mut self, i: usize, page: usize) {
        let end = self.end_of(i);
        for node in self.nodes.get_mut(i..end).into_iter().flatten() {
            if node.has_rows {
                node.open = true;
            }
        }
        self.reveal_rows(i, page);
    }

    /// Keeps `top` in range for `rows_shown` whole rows.
    pub fn clamp_top_for(&mut self, page: usize) {
        let n = self.visible().len();
        self.top = self.top.min(n.saturating_sub(page.max(1)));
    }

    fn clamp_top(&mut self) {
        let n = self.visible().len();
        self.top = self.top.min(n.saturating_sub(1));
    }

    /// Scrolls so that the selected row shows.
    pub fn show_selection(&mut self, page: usize) {
        let vis = self.visible();
        if let Some(at) = vis.iter().position(|&v| v == self.sel) {
            let page = page.max(1);
            if at < self.top {
                self.top = at;
            } else if at >= self.top + page {
                self.top = at + 1 - page;
            }
        }
    }

    /// Selects row `i` (it shows: the folders it is in open).
    pub fn select(&mut self, i: usize, page: usize) {
        if i >= self.nodes.len() {
            return;
        }
        for c in self.chain(i) {
            if c != i {
                self.nodes[c].open = true;
            }
        }
        self.sel = i;
        self.show_selection(page);
    }

    /// The way down to a place: the ids from the Desktop's rows down to the place itself. Each
    /// folder on the way opens (read first where needed) and the place's row is selected once it
    /// is there. A row that does not come leaves the selection where the way stopped.
    pub fn follow(&mut self, chain: Vec<String>, page: usize) {
        self.follow = chain;
        self.go_on_following_page(page);
    }

    fn go_on_following(&mut self) {
        self.go_on_following_page(usize::MAX / 2);
    }

    fn go_on_following_page(&mut self, page: usize) {
        while let Some(next) = self.follow.first().cloned() {
            let Some(i) = self.find(&next) else {
                // Not read yet: wait for its folder's rows; or the way is gone.
                let parent_waiting = self.nodes.iter().any(|n| n.open && !n.read && n.has_rows);
                if !parent_waiting {
                    self.follow.clear();
                }
                return;
            };
            self.follow.remove(0);
            if self.follow.is_empty() {
                // The place itself opens too, as Explorer opened the folder it started in
                // (`*-tree-start`: C: open, its folders showing).
                if self.nodes[i].has_rows {
                    self.nodes[i].open = true;
                }
                self.select(i, if page > 10_000 { 1 } else { page });
                return;
            }
            // A folder on the way: open (its rows are read if they are not).
            if !self.nodes[i].has_rows {
                self.nodes[i].has_rows = true;
            }
            self.nodes[i].open = true;
            if !self.nodes[i].read {
                return;
            }
        }
    }

    /// Whether a `follow` is still on its way.
    pub fn following(&self) -> bool {
        !self.follow.is_empty()
    }

    /// The selected row.
    pub fn selected(&self) -> Option<&Node> {
        self.nodes.get(self.sel)
    }
}

impl Default for Tree {
    fn default() -> Self {
        Tree::new()
    }
}

/// What a key in the tree did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Moved {
    /// Nothing that matters outside the tree.
    No,
    /// Another row is selected: the icons follow it (after a pause, as Windows' did).
    Selection,
}

/// The tree's keys (Windows' tree view): the arrows, Home, End, Page Up and Down, + and - and *
/// of the number pad, Backspace (to the folder above), a typed letter (the next row starting
/// with it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeKey {
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Plus,
    Minus,
    Star,
    Backspace,
    Char(char),
}

impl Tree {
    /// Carries out a key; `page` is how many whole rows show.
    pub fn key(&mut self, key: TreeKey, page: usize) -> Moved {
        let vis = self.visible();
        let Some(at) = vis.iter().position(|&v| v == self.sel) else {
            self.sel = 0;
            return Moved::Selection;
        };
        let before = self.sel;
        let page = page.max(1);
        let go = |t: &mut Tree, to: usize| {
            t.sel = vis[to.min(vis.len() - 1)];
            t.show_selection(page);
        };
        match key {
            TreeKey::Up => go(self, at.saturating_sub(1)),
            TreeKey::Down => go(self, at + 1),
            TreeKey::Home => go(self, 0),
            TreeKey::End => go(self, vis.len() - 1),
            TreeKey::PageUp => go(self, at.saturating_sub(page.saturating_sub(1).max(1))),
            TreeKey::PageDown => go(self, at + page.saturating_sub(1).max(1)),
            TreeKey::Left => {
                let i = self.sel;
                if self.nodes[i].open {
                    self.close(i);
                } else if let Some(p) = self.parent(i) {
                    go(self, vis.iter().position(|&v| v == p).unwrap_or(0));
                }
            }
            TreeKey::Backspace => {
                if let Some(p) = self.parent(self.sel) {
                    go(self, vis.iter().position(|&v| v == p).unwrap_or(0));
                }
            }
            TreeKey::Right => {
                let i = self.sel;
                if self.nodes[i].open {
                    // Into its first row.
                    if vis.get(at + 1).is_some_and(|&v| self.nodes[v].depth > self.nodes[i].depth) {
                        go(self, at + 1);
                    }
                } else {
                    self.open(i, page);
                }
            }
            TreeKey::Plus => self.open(self.sel, page),
            TreeKey::Minus => {
                self.close(self.sel);
            }
            TreeKey::Star => self.open_all(self.sel, page),
            TreeKey::Char(c) => {
                let lower = c.to_lowercase().next().unwrap_or(c);
                let n = vis.len();
                let found = (1..=n).map(|k| (at + k) % n).find(|&k| {
                    self.nodes[vis[k]].name.trim_start().chars().next().and_then(|f| f.to_lowercase().next()) == Some(lower)
                });
                if let Some(k) = found {
                    go(self, k);
                }
            }
        }
        if before != self.sel { Moved::Selection } else { Moved::No }
    }
}

// ---------------------------------------------------------------------------------------------
// The pane's place in the window.

/// The look's default width of the pane (95: the header and the tree's box; 98 and 2000: the
/// bar's frame), and of the bar between the panes. The same at every window size on the PCs
/// (600 x 411 and maximized at 800 x 600).
pub fn default_width(era: Era) -> i32 {
    match era {
        Era::Win95 => 192,
        Era::Win98 => 196,
        Era::Win2000 => 200,
    }
}

pub fn split_width(era: Era) -> i32 {
    if era == Era::Win95 { 3 } else { 4 }
}

/// The narrowest the pane gets when the bar between the panes is dragged, and how much room the
/// icons keep (Bevel95's limits; Windows' were not measured).
const MIN_WIDTH: i32 = 24;
const MIN_RIGHT: i32 = 24;

/// Where the parts of the panes are, in window pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Layout {
    /// 95: "All Folders" (a sunken box of 19 rows); 98 and 2000: the bar's caption.
    pub header: Rect,
    /// 98 and 2000: the bar's outer frame.
    pub frame: Option<Rect>,
    /// 98 and 2000: the caption's close button.
    pub close: Option<Rect>,
    /// The tree's box: 95 with its sunken edge of 2, 98 and 2000 without an edge.
    pub tree_box: Rect,
    /// The rows' area inside it (scroll bars included).
    pub tree: Rect,
    /// The bar between the panes.
    pub split: Rect,
    /// 95: "Contents of '...'" above the icons.
    pub contents: Option<Rect>,
    /// The icons' list, with its sunken edge.
    pub list: Rect,
}

/// Lays the panes out in `area` (from the list's top-left to the status bar), the pane `width`
/// wide.
pub fn layout(era: Era, area: Rect, width: i32) -> Layout {
    let split = split_width(era);
    let w = width.clamp(MIN_WIDTH.min(area.w), (area.w - split - MIN_RIGHT).max(MIN_WIDTH.min(area.w)));
    let right_x = area.x + w + split;
    let right_w = (area.right() - right_x).max(0);
    if era == Era::Win95 {
        let header = Rect::new(area.x, area.y, w, 19);
        let tree_box = Rect::new(area.x, area.y + 20, w, (area.h - 20).max(0));
        return Layout {
            header,
            frame: None,
            close: None,
            tree_box,
            tree: tree_box.inset(2, 2, 2, 2),
            split: Rect::new(area.x + w, area.y, split, area.h),
            contents: Some(Rect::new(right_x, area.y, right_w, 19)),
            list: Rect::new(right_x, area.y + 20, right_w, (area.h - 20).max(0)),
        };
    }
    let frame = Rect::new(area.x, area.y, w, area.h);
    let header = Rect::new(area.x + 2, area.y + 2, (w - 4).max(0), 20);
    let close = if era == Era::Win98 { Rect::new(frame.x + w - 25, frame.y + 4, 20, 17) } else { Rect::new(frame.x + w - 23, frame.y + 3, 20, 17) };
    let tree_box = Rect::new(area.x + 2, area.y + 24, (w - 4).max(0), (area.h - 26).max(0));
    Layout {
        header,
        frame: Some(frame),
        close: Some(close),
        tree_box,
        tree: tree_box,
        split: Rect::new(area.x + w, area.y, split, area.h),
        contents: None,
        list: Rect::new(right_x, area.y, right_w, area.h),
    }
}

/// The pane's caption: 95 "All Folders", 98 "All Folders", 2000 "Folders".
pub fn caption(era: Era) -> &'static str {
    if era == Era::Win2000 { "Folders" } else { "All Folders" }
}

/// 95's caption over the icons.
pub fn contents_caption(title: &str) -> String {
    format!("Contents of '{title}'")
}

// ---------------------------------------------------------------------------------------------
// The rows' scroll bars.

/// A scroll bar's width.
pub const BAR: i32 = 16;

/// Which scroll bars the rows need in `tree` (down, across), and the rows' area without them.
pub fn bars(tree: Rect, rows: usize, content_w: i32) -> (bool, bool, Rect) {
    let fits = |h: i32| (h / tree::ROW_H).max(0) as usize >= rows;
    let mut v = !fits(tree.h);
    let mut h = content_w > tree.w - if v { BAR } else { 0 };
    v = !fits(tree.h - if h { BAR } else { 0 });
    h = content_w > tree.w - if v { BAR } else { 0 };
    let shown = Rect::new(tree.x, tree.y, (tree.w - if v { BAR } else { 0 }).max(0), (tree.h - if h { BAR } else { 0 }).max(0));
    (v, h, shown)
}

/// How many whole rows show in `h` pixels.
pub fn page(h: i32) -> usize {
    (h / tree::ROW_H).max(1) as usize
}

// ---------------------------------------------------------------------------------------------
// Pictures and painting.

/// A row's picture in `era` (`open`: a selected folder shows the open folder).
pub fn picture(era: Era, kind: NodeKind, open: bool) -> Icon {
    match (era, kind) {
        (Era::Win95, NodeKind::Folder) => tree_art::w95(if open { tree_art::W95::OpenFolder } else { tree_art::W95::ClosedFolder }),
        (Era::Win95, NodeKind::Desktop) => tree_art::w95(tree_art::W95::Desktop),
        (Era::Win95, NodeKind::Printers) => tree_art::w95(tree_art::W95::Printers),
        (_, NodeKind::Folder) => art::small(era, if open { Small::Folder } else { Small::ClosedFolder }),
        (_, NodeKind::Desktop) => art::small(era, Small::Desktop),
        (_, NodeKind::MyDocuments) => art::small(era, Small::MyDocuments),
        (_, NodeKind::MyComputer) => art::small(era, Small::MyComputer),
        (_, NodeKind::Drive(k)) => art::small(era, Small::Drive(k)),
        (_, NodeKind::ControlPanel) => art::small(era, Small::ControlPanel),
        (_, NodeKind::Printers) => art::small(era, Small::Printers),
        (_, NodeKind::RecycleBin) => art::small(era, Small::RecycleBin),
    }
}

/// How wide the rows reach (the scroll bar across follows it): the widest row's label box.
pub fn content_width(t: &Tree, font: &Font) -> i32 {
    let style = TreeStyle::of(Era::Win95);
    t.visible()
        .iter()
        .map(|&i| {
            let n = &t.nodes[i];
            let (x, _) = tree::label_room(style, n.depth, false, 0);
            x + font.measure(&n.name) + 2
        })
        .max()
        .unwrap_or(0)
}

/// Paints the rows into `area` (the rows' part of the tree, scroll bars excluded), the tree
/// scrolled by `t.top` rows and `t.scroll_x` pixels. `focused`: the tree has the keyboard, so
/// the selection shows. `screen`: where the window's corner is on the screen (the focus
/// rectangle's dots); `cues`: the focus rectangle shows (2000: once a key was pressed).
#[allow(clippy::too_many_arguments)]
pub fn paint_rows(fb: &mut Framebuffer, theme: &Theme, font: &Font, era: Era, t: &Tree, area: Rect, focused: bool, cues: bool, screen: (i32, i32)) {
    let c = &theme.colors;
    if area.w <= 0 || area.h <= 0 {
        return;
    }
    let content_w = content_width(t, font).max(area.w + t.scroll_x);
    let mut rows = Framebuffer::new(content_w, area.h, rgb(0xFFFFFF));
    let style = TreeStyle::of(era);
    let vis = t.visible();
    let shown = (area.h + tree::ROW_H - 1) / tree::ROW_H;
    for (k, &i) in vis.iter().enumerate().skip(t.top).take(shown.max(0) as usize) {
        let n = &t.nodes[i];
        let y = (k - t.top) as i32 * tree::ROW_H;
        let selected = match t.drop_row {
            Some(d) => i == d,
            None => i == t.sel,
        };
        let lit = if t.drop_row.is_some() { selected } else { selected && focused };
        let icon = picture(era, n.kind, selected && n.kind == NodeKind::Folder);
        let row = TreeRow {
            depth: n.depth,
            open: n.has_rows.then_some(n.open),
            last: t.lasts(i),
            tick: None,
            icon: Some(&icon),
            label: &n.name,
            selected: lit,
            focused: focused || t.drop_row.is_some(),
        };
        tree::paint_row(&mut rows, theme, font, style, &row, y, content_w, (screen.0 + area.x - t.scroll_x + 1, screen.1 + area.y));
        // Windows 2000 hid the focus rectangle until a key was pressed (`win2000-truecolor-tree-start`).
        if selected && focused && !cues {
            let (text_x, _) = tree::label_room(style, n.depth, false, content_w);
            let r = Rect::new(text_x - 2, y, font.measure(&n.name) + 4, tree::ROW_H);
            rows.frame(r, c.menu_highlight, c.menu_highlight);
        }
    }
    fb.blit_part(&rows, Rect::new(t.scroll_x, 0, area.w, area.h), area.x, area.y);
}

/// What of the tree is at (x, y) in `area`: a row's plus/minus box or the row (`None`: no row).
pub fn row_at(t: &Tree, font: &Font, area: Rect, x: i32, y: i32) -> Option<(usize, Zone)> {
    if !area.contains(x, y) {
        return None;
    }
    let k = t.top + ((y - area.y) / tree::ROW_H) as usize;
    let &i = t.visible().get(k)?;
    let n = &t.nodes[i];
    let zone = tree::zone(TreeStyle::of(Era::Win95), n.depth, n.has_rows, false, font.measure(&n.name), x - area.x + t.scroll_x);
    Some((i, zone))
}

/// The close button's "X": 8 by 7, as both bars drew it.
const CLOSE_X: &[&str] = crate::art::rows("tree/close", crate::art::Look::Win98).rows;

/// Paints the pane's chrome: 95's two captions; 98's and 2000's frame, caption and close button
/// (`close`: 0 plain, 1 under the pointer, 2 pressed). The rows and scroll bars are painted
/// separately.
pub fn paint_chrome(fb: &mut Framebuffer, c: &Colors, font: &Font, era: Era, l: &Layout, contents: Option<&str>, close: u8) {
    fb.fill(l.split, c.face);
    if era == Era::Win95 {
        for (r, text) in [(Some(l.header), Some(caption(era).to_string())), (l.contents, contents.map(contents_caption))] {
            let (Some(r), Some(text)) = (r, text) else { continue };
            if r.w <= 0 {
                continue;
            }
            fb.fill(r, c.face);
            fb.frame(r, c.shadow, c.highlight);
            let mut inner = Framebuffer::new((r.w - 2).max(0), 17, c.face);
            font.draw(&mut inner, 3, 2, &text, c.text);
            fb.blit(&inner, r.x + 1, r.y + 1);
        }
        // The face row between the captions and the boxes.
        fb.fill(Rect::new(l.header.x, l.header.bottom(), l.header.w, 1), c.face);
        if let Some(r) = l.contents {
            fb.fill(Rect::new(r.x, r.bottom(), r.w, 1), c.face);
        }
        let b = l.tree_box;
        if b.w > 4 && b.h > 4 {
            fb.frame(b, c.shadow, c.highlight);
            fb.frame(b.inset(1, 1, 1, 1), c.dark_shadow, c.light);
        }
        return;
    }
    let Some(f) = l.frame else { return };
    if f.w <= 4 || f.h <= 4 {
        return;
    }
    fb.fill(f, c.face);
    fb.frame(f, c.shadow, c.highlight);
    fb.frame(f.inset(1, 1, 1, 1), c.highlight, c.shadow);
    // The caption's etched line under it.
    fb.hline(f.x + 2, f.y + 22, f.w - 3, c.shadow);
    fb.hline(f.x + 1, f.y + 23, f.w - 3, c.highlight);
    let h = l.header;
    let mut text = Framebuffer::new((h.w - 24).max(0), 18, c.face);
    let (tx, ty) = if era == Era::Win98 { (11, 4) } else { (6, 3) };
    font.draw(&mut text, tx, ty, caption(era), c.text);
    fb.blit(&text, h.x, h.y);
    if let Some(b) = l.close {
        match close {
            1 => fb.frame(b, c.highlight, c.shadow),
            2 => fb.frame(b, c.shadow, c.highlight),
            _ => {}
        }
        let s = i32::from(close == 2);
        for (r, line) in CLOSE_X.iter().enumerate() {
            for (k, ch) in line.bytes().enumerate() {
                if ch == b'#' {
                    fb.set(b.x + 6 + k as i32 + s, b.y + 5 + r as i32 + s, c.text);
                }
            }
        }
    }
}

/// The bar between the panes while it is dragged: a 4-pixel band of every other pixel inverted
/// at `x`, from the panes' top to their bottom (Windows 95's drag line, `*-tree-split-drag`).
pub fn paint_drag_line(fb: &mut Framebuffer, x: i32, top: i32, h: i32) {
    for y in top..top + h {
        for dx in 0..4 {
            let px = x + dx;
            if (px + y) % 2 == 0 {
                let v = fb.get(px, y);
                fb.set(px, y, (v & 0xFF00_0000) | (!v & 0x00FF_FFFF));
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The menu on a row.

use crate::desktop::DesktopCommand as D;
use crate::paint::{Menu, MenuEntry};

/// The commands of the menu on a tree row, in Windows' order (`None`: a separator): Explore (the
/// default, in bold) and Open; Cut, Copy and Paste for a folder of files; Create Shortcut and
/// Delete; Properties. Left out for now (todo): Find..., Send To, Format..., Rename, Sharing.
/// `locked`: the row is a drive found locked by BitLocker; Windows' own Unlock Drive... follows
/// Open, as on the drive's icon (`desktop::ItemKind::LockedDrive`), and Paste is left out: it
/// would put files into a drive that cannot be written until it is unlocked (the review before
/// 0.1.3, B5). Its other entries do not need the drive's contents: a shortcut to the drive, and
/// Windows' own Properties.
pub fn row_entries(kind: NodeKind, can_paste: bool, locked: bool) -> Vec<Option<D>> {
    match kind {
        NodeKind::Drive(_) if locked => {
            vec![Some(D::Explore), Some(D::Open), Some(D::UnlockDrive), None, Some(D::CreateShortcut), None, Some(D::ItemProperties)]
        }
        NodeKind::Desktop => Vec::new(),
        NodeKind::Folder => {
            let mut out = vec![Some(D::Explore), Some(D::Open), None, Some(D::Cut), Some(D::Copy)];
            if can_paste {
                out.push(Some(D::Paste));
            }
            out.extend([None, Some(D::CreateShortcut), Some(D::Delete), None, Some(D::ItemProperties)]);
            out
        }
        NodeKind::Drive(_) | NodeKind::MyDocuments => {
            let mut out = vec![Some(D::Explore), Some(D::Open)];
            if can_paste {
                out.extend([None, Some(D::Paste)]);
            }
            out.extend([None, Some(D::CreateShortcut), None, Some(D::ItemProperties)]);
            out
        }
        NodeKind::MyComputer => vec![Some(D::Explore), Some(D::Open), None, Some(D::CreateShortcut), None, Some(D::ItemProperties)],
        NodeKind::ControlPanel | NodeKind::Printers => vec![Some(D::Explore), Some(D::Open), None, Some(D::CreateShortcut)],
        NodeKind::RecycleBin => vec![Some(D::Explore), Some(D::Open), None, Some(D::CreateShortcut), None, Some(D::ItemProperties)],
    }
}

/// The menu on a tree row in `era` (`row_entries`), Explore in bold.
pub fn row_menu(era: Era, kind: NodeKind, can_paste: bool, locked: bool) -> Menu {
    let entries = row_entries(kind, can_paste, locked)
        .into_iter()
        .map(|e| match e {
            Some(c) => {
                let label = match c {
                    D::Explore if era == Era::Win2000 => "E&xplore",
                    D::Explore => "&Explore",
                    D::Open => "&Open",
                    D::Cut => "Cu&t",
                    D::Copy => "&Copy",
                    D::Paste => "&Paste",
                    D::CreateShortcut => "Create &Shortcut",
                    D::Delete => "&Delete",
                    D::UnlockDrive => crate::bitlocker::UNLOCK_LABEL,
                    _ => "P&roperties",
                };
                let mut entry = MenuEntry::item(label, None);
                entry.bold = c == D::Explore;
                entry
            }
            None => MenuEntry::separator(),
        })
        .collect();
    Menu { explorer_bold: true, show_keys: era.always_shows_keys(), ..Menu::new(entries) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(id: &str, kind: NodeKind, has_rows: bool) -> NodeSpec {
        NodeSpec { id: id.into(), name: id.trim_end_matches('\\').rsplit('\\').next().unwrap_or(id).into(), kind, has_rows }
    }

    /// The Reference PC's tree as Windows 95 opened it: the Desktop, My Computer with A:, C: and
    /// its three folders, Control Panel and Printers, the Recycle Bin, a folder on the desktop.
    fn reference() -> Tree {
        let mut t = Tree::new();
        assert_eq!(t.wanted(), vec!["::desktop".to_string()]);
        t.set_rows(
            "::desktop",
            vec![spec("::my-computer", NodeKind::MyComputer, true), spec("::recycle-bin", NodeKind::RecycleBin, false), spec("D\\Online Services", NodeKind::Folder, false)],
        );
        t.follow(vec!["::my-computer".into(), "C:\\".into()], 20);
        assert_eq!(t.wanted(), vec!["::my-computer".to_string()]);
        t.set_rows(
            "::my-computer",
            vec![
                spec("A:\\", NodeKind::Drive(DriveKind::Floppy), true),
                spec("C:\\", NodeKind::Drive(DriveKind::Fixed), true),
                spec("::control-panel", NodeKind::ControlPanel, false),
                spec("::printers", NodeKind::Printers, false),
            ],
        );
        t
    }

    fn names(t: &Tree) -> Vec<String> {
        t.visible().iter().map(|&i| format!("{}{}", " ".repeat(t.nodes[i].depth), t.nodes[i].name)).collect()
    }

    /// A folder that could not be read loses its rows; the selection below it stays on its own row
    /// (by id), not on the index that now names another row (the third safety review of
    /// 2026-11-18, A2).
    #[test]
    fn the_selection_stays_on_its_row_when_a_folder_above_cannot_be_read() {
        let mut t = Tree::new();
        t.wanted();
        t.set_rows("::desktop", vec![spec("A:\\", NodeKind::Drive(DriveKind::Floppy), true), spec("C:\\Bevel95 test", NodeKind::Folder, false), spec("C:\\Other", NodeKind::Folder, false), spec("C:\\Third", NodeKind::Folder, false)]);
        t.toggle(t.find("A:\\").unwrap(), 20);
        t.set_rows("A:\\", vec![spec("A:\\one", NodeKind::Folder, false), spec("A:\\two", NodeKind::Folder, false)]);
        t.sel = t.find("C:\\Bevel95 test").unwrap();
        t.unreadable("A:\\");
        assert_eq!(t.selected().unwrap().id, "C:\\Bevel95 test", "its index named C:\\Third before");
        // Inside the folder that went: the folder itself.
        t.toggle(t.find("A:\\").unwrap(), 20);
        t.set_rows("A:\\", vec![spec("A:\\one", NodeKind::Folder, false)]);
        t.sel = t.find("A:\\one").unwrap();
        t.unreadable("A:\\");
        assert_eq!(t.selected().unwrap().id, "A:\\");
    }

    #[test]
    fn the_tree_opens_down_to_the_place_shown() {
        let mut t = reference();
        assert_eq!(t.selected().unwrap().id, "C:\\", "C: selected");
        // The place itself opens, its rows are read next (`win95-truecolor-tree-start`).
        assert!(t.nodes[t.sel].open);
        assert_eq!(t.wanted(), vec!["C:\\".to_string()]);
        t.set_rows("C:\\", vec![spec("C:\\Program Files", NodeKind::Folder, true), spec("C:\\Win95", NodeKind::Folder, false), spec("C:\\Windows", NodeKind::Folder, true)]);
        assert_eq!(
            names(&t),
            ["Desktop", " ::my-computer", "  A:", "  C:", "   Program Files", "   Win95", "   Windows", "  ::control-panel", "  ::printers", " ::recycle-bin", " Online Services"]
                .map(String::from)
        );
        // Which rows have boxes, and where the lines end.
        assert!(!t.nodes[t.find("C:\\Win95").unwrap()].has_rows);
        assert_eq!(t.lasts(t.find("C:\\Windows").unwrap()), [true, false, false, true]);
        assert_eq!(t.lasts(t.find("D\\Online Services").unwrap()), [true, true]);
        // A way to a folder not read yet: each level is read in turn, then selected.
        t.follow(vec!["::my-computer".into(), "C:\\".into(), "C:\\Windows".into(), "C:\\Windows\\System".into()], 20);
        assert_eq!(t.wanted(), vec!["C:\\Windows".to_string()]);
        t.set_rows("C:\\Windows", vec![spec("C:\\Windows\\System", NodeKind::Folder, false)]);
        assert_eq!(t.selected().unwrap().id, "C:\\Windows\\System");
        assert!(!t.following());
    }

    #[test]
    fn keys_move_open_and_close_as_windows_trees_did() {
        let mut t = reference();
        let c = t.sel;
        t.set_rows("C:\\", vec![spec("C:\\Program Files", NodeKind::Folder, true), spec("C:\\Windows", NodeKind::Folder, true)]);
        assert!(t.nodes[c].open);
        assert_eq!(t.key(TreeKey::Down, 20), Moved::Selection);
        assert_eq!(t.selected().unwrap().name, "Program Files");
        // Right opens, Right again goes in once its rows are there.
        assert_eq!(t.key(TreeKey::Right, 20), Moved::No);
        assert_eq!(t.wanted(), vec!["C:\\Program Files".to_string()]);
        t.set_rows("C:\\Program Files", vec![spec("C:\\Program Files\\Accessories", NodeKind::Folder, false)]);
        assert_eq!(t.key(TreeKey::Right, 20), Moved::Selection);
        assert_eq!(t.selected().unwrap().name, "Accessories");
        // Backspace: the folder above; Left on an open row closes it.
        assert_eq!(t.key(TreeKey::Backspace, 20), Moved::Selection);
        assert_eq!(t.selected().unwrap().name, "Program Files");
        assert_eq!(t.key(TreeKey::Left, 20), Moved::No);
        assert!(!t.nodes[t.sel].open);
        // A letter: the next row starting with it, round to the top (`win95-w1..w4`).
        assert_eq!(t.key(TreeKey::Char('w'), 20), Moved::Selection);
        assert_eq!(t.selected().unwrap().name, "Windows");
        t.key(TreeKey::Char('d'), 20);
        assert_eq!(t.selected().unwrap().name, "Desktop");
        // Closing a folder that holds the selection selects the folder.
        let pf = t.find("C:\\Program Files").unwrap();
        t.open(pf, 20);
        t.sel = t.find("C:\\Program Files\\Accessories").unwrap();
        assert!(t.toggle(pf, 20));
        assert_eq!(t.sel, pf);
    }

    #[test]
    fn opening_scrolls_as_little_as_needed() {
        let mut t = reference();
        t.set_rows("C:\\", vec![spec("C:\\Windows", NodeKind::Folder, true)]);
        let w = t.find("C:\\Windows").unwrap();
        t.sel = w;
        t.open(w, 18);
        let many: Vec<NodeSpec> = (0..16).map(|k| spec(&format!("C:\\Windows\\F{k:02}"), NodeKind::Folder, false)).collect();
        t.set_rows("C:\\Windows", many);
        t.top = 0;
        t.open(w, 18);
        // Desktop, My Computer, A:, C:, Windows (row 4) and its 16 rows: the last of them shows
        // at the bottom of 18 whole rows, Windows still in sight.
        let vis = t.visible();
        let last = vis.iter().position(|&v| v == t.end_of(w) - 1).unwrap();
        assert_eq!(t.top + 18, last + 1);
        assert!(vis.iter().position(|&v| v == w).unwrap() >= t.top);
    }
    #[test]
    fn rows_read_again_keep_what_was_open() {
        let mut t = reference();
        t.set_rows("C:\\", vec![spec("C:\\A", NodeKind::Folder, true), spec("C:\\B", NodeKind::Folder, false)]);
        let a = t.find("C:\\A").unwrap();
        t.open(a, 20);
        t.set_rows("C:\\A", vec![spec("C:\\A\\x", NodeKind::Folder, false)]);
        t.sel = t.find("C:\\B").unwrap();
        // B is gone, a new folder came: A keeps its rows and stays open, the selection goes to C:.
        t.set_rows("C:\\", vec![spec("C:\\A", NodeKind::Folder, true), spec("C:\\New", NodeKind::Folder, false)]);
        assert!(t.nodes[t.find("C:\\A").unwrap()].open);
        assert!(t.find("C:\\A\\x").is_some());
        assert_eq!(t.selected().unwrap().id, "C:\\");
        // A drive that cannot be read closes and keeps its box.
        let a_drive = t.find("A:\\").unwrap();
        t.open(a_drive, 20);
        assert_eq!(t.wanted(), vec!["A:\\".to_string()]);
        t.unreadable("A:\\");
        let n = &t.nodes[t.find("A:\\").unwrap()];
        assert!(!n.open && n.has_rows);
    }

    #[test]
    fn each_look_lays_its_pane_out() {
        // 95 (`win95-truecolor-tree-start`, 600 x 411 at 44,44): the panes from (4, 42).
        let l = layout(Era::Win95, Rect::new(4, 42, 592, 346), 192);
        assert_eq!(l.header, Rect::new(4, 42, 192, 19));
        assert_eq!(l.tree_box, Rect::new(4, 62, 192, 326));
        assert_eq!(l.split, Rect::new(196, 42, 3, 346));
        assert_eq!(l.list.x, 199);
        // 98 (`win98-truecolor-tree-large`): the bar's frame from (4, 118), 196 wide.
        let l = layout(Era::Win98, Rect::new(4, 118, 592, 289), 196);
        assert_eq!(l.close, Some(Rect::new(175, 122, 20, 17)));
        assert_eq!(l.list.x, 204);
        let l = layout(Era::Win2000, Rect::new(114, 210, 672, 334), 200);
        assert_eq!(l.close, Some(Rect::new(291, 213, 20, 17)));
        assert_eq!(l.list.x, 318);
    }

    #[test]
    fn the_menu_on_a_row_holds_what_works() {
        assert_eq!(row_entries(NodeKind::Desktop, true, false), Vec::new());
        let folder = row_entries(NodeKind::Folder, false, false);
        assert_eq!(folder[0], Some(D::Explore));
        assert!(!folder.contains(&Some(D::Paste)));
        assert!(row_entries(NodeKind::Folder, true, false).contains(&Some(D::Paste)));
        let m = row_menu(Era::Win95, NodeKind::Drive(DriveKind::Fixed), false, false);
        assert!(m.entries[0].bold);
    }

    #[test]
    fn a_locked_drive_row_has_unlock_drive_after_open() {
        let drive = NodeKind::Drive(DriveKind::Removable);
        for era in Era::ALL {
            let entries = row_entries(drive, false, true);
            assert_eq!(entries[..4], [Some(D::Explore), Some(D::Open), Some(D::UnlockDrive), None]);
            let m = row_menu(era, drive, false, true);
            assert_eq!(m.entries[2].label, crate::bitlocker::UNLOCK_LABEL);
            assert!(m.entries[0].bold && !m.entries[2].bold);
        }
        // No Paste on a locked drive, whatever the Clipboard holds (the review before 0.1.3, B5);
        // a drive that reads keeps it.
        assert!(!row_entries(drive, true, true).contains(&Some(D::Paste)));
        assert_eq!(row_entries(drive, true, true), row_entries(drive, false, true));
        assert!(row_entries(drive, true, false).contains(&Some(D::Paste)));
        // A drive that reads, and any other row: no such entry, whatever `locked` says.
        assert!(!row_entries(drive, true, false).contains(&Some(D::UnlockDrive)));
        for kind in [NodeKind::Folder, NodeKind::MyComputer, NodeKind::MyDocuments, NodeKind::RecycleBin, NodeKind::ControlPanel] {
            assert!(!row_entries(kind, true, true).contains(&Some(D::UnlockDrive)), "{kind:?}");
        }
    }
}
