//! What the folder windows remember between runs (`folders.txt` in Bevel95's data folder): whether
//! the status bar shows, and for each folder whose window was open: its place and size, whether it
//! was maximized, Arrange Icons' order and Auto Arrange (Windows 95 kept the same for every folder
//! it had shown, in the registry). Written by the folder windows' program; deleting the file
//! makes every folder window open as new again. Explorer adds its window's place, the tree's width,
//! and (98, 2000) the folders whose windows had the tree switched on.

use std::collections::BTreeMap;

use crate::desktop_icons::{Arrange, ViewMode};
use crate::theme::Era;

/// How a folder was shown: its view, its order (reversed by a second header click) and Details'
/// column widths (empty: the look's own). Windows kept the same for each folder it had shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewState {
    pub mode: ViewMode,
    pub arrange: Arrange,
    pub reversed: bool,
    pub columns: Vec<i32>,
}

/// The view a window opens a folder in (the Reference PCs, `private/probes/views/<era>-notes.txt`):
/// the folder's own remembered view; else Windows 95 Large Icons up to 79 items and List from 80
/// (77 Large and 80 List were measured), 98 the same in a new window but always Large Icons when
/// a window goes to another folder, 2000 always Large Icons.
pub fn opening_view(era: Era, remembered: Option<ViewMode>, count: usize, new_window: bool) -> ViewMode {
    if let Some(m) = remembered {
        return m;
    }
    match era {
        Era::Win95 if count >= 80 => ViewMode::List,
        Era::Win98 if new_window && count >= 80 => ViewMode::List,
        _ => ViewMode::Large,
    }
}

/// One folder's window as it was last closed (screen pixels; the whole-number size it was drawn
/// at is part of it: a place saved at one size is used as it is at another).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Window {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub maximized: bool,
    pub arrange: Arrange,
    pub auto_arrange: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved {
    pub status_bar: bool,
    /// By folder: its path in lower case, or "::my-computer".
    pub windows: BTreeMap<String, Window>,
    /// The Explorer window (the one with the tree) as it was last closed: one place for all of
    /// them, as Windows kept one.
    pub explorer: Option<Window>,
    /// How wide the tree's pane was last made (1× pixels; `None`: the look's own width).
    pub tree_width: Option<i32>,
    /// 98 and 2000: the folders whose own windows were closed with the tree showing (2000's
    /// Folders button, 98's Explorer Bar), lower-case keys as `windows`.
    pub tree_folders: std::collections::BTreeSet<String>,
    /// How each folder was last shown (keys as `windows`).
    pub views: BTreeMap<String, ViewState>,
    /// Windows 95's and 98's Explorer had one view of its own, the one it was last closed in
    /// (`None`: List, as both started).
    pub explorer_view: Option<ViewMode>,
    /// Windows 98: the folders whose View > as Web Page was turned off (98 kept it per folder).
    pub web_off: std::collections::BTreeSet<String>,
    /// Windows 2000's Folder Options: "Use Windows classic folders" (the web view off everywhere)
    /// and "Open each folder in its own window".
    pub classic_folders: bool,
    pub own_window: bool,
    /// Windows 2000: the system folders whose "Show Files" was clicked (2000 remembered it).
    pub shown_files: std::collections::BTreeSet<String>,
    /// Where the icons the person dragged in a folder's window were left (Large and Small Icons;
    /// brief 103: only icons really moved): by folder (keys as `windows`), (x, y, item id) in the
    /// order they were moved, as the icon view's `moved_places` gives them.
    pub places: BTreeMap<String, Vec<(i32, i32, String)>>,
    /// Windows 98 with its web view: the icons that go last in a folder's order (a dragged icon
    /// counts as moved to the end), by folder, in that order (brief 103).
    pub last: BTreeMap<String, Vec<String>>,
    /// The folders of `places` and `last`, the one changed longest ago first: beyond
    /// `MOST_ARRANGED` the oldest go.
    pub arranged: Vec<String>,
}

/// How many folders' moved icons are kept at most (`places` and `last`): as many as the other
/// records of folders keep (`MOST`); each record holds only the icons really moved, so the file
/// stays small. The one changed longest ago goes first.
pub const MOST_ARRANGED: usize = MOST;
/// How many moved icons one folder keeps at most (a drag of every icon of a folder of thousands
/// keeps the last moved of them).
pub const MOST_MOVED: usize = 500;

/// When a folder window's icons are laid out again (brief 103, the survey's table).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relaid {
    /// F5, View > Refresh.
    Refresh,
    /// Gone back to in the same window (Back, Up; 95: brought forward by Backspace in a
    /// window opened from it).
    Returned,
    /// Its window closed and opened again.
    Reopened,
}

/// What becomes of the icons the person moved when the window lays them out again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kept {
    /// They stay where they were left.
    Places,
    /// Packed in order again, the moved ones last (Windows 98 with its web view).
    MovedLast,
    /// Packed in their order again, the move forgotten (2000's F5 with its web view).
    InOrder,
}

/// Each look as measured on the Reference PCs (`private/probes/folder-arrange/survey.md`, part
/// 2): 95 keeps the places for good; 98 with its web view (View > as Web Page; its default)
/// keeps My Computer's places while its window stays open (F5, Back, Up; PROVISIONAL: F5 seen
/// once) and packs an ordinary folder at every step, the moved icons last, and packs My Computer
/// so when its window is opened again; 98 without the web view keeps them for good; 2000 keeps
/// them, but its F5 with the web view on puts every icon back in its order.
pub fn kept(era: Era, web_view: bool, my_computer: bool, when: Relaid) -> Kept {
    match era {
        Era::Win95 => Kept::Places,
        Era::Win98 if !web_view => Kept::Places,
        Era::Win98 if my_computer && when != Relaid::Reopened => Kept::Places,
        Era::Win98 => Kept::MovedLast,
        Era::Win2000 if web_view && when == Relaid::Refresh => Kept::InOrder,
        Era::Win2000 => Kept::Places,
    }
}

impl Default for Saved {
    fn default() -> Saved {
        Saved {
            status_bar: true,
            windows: BTreeMap::new(),
            explorer: None,
            tree_width: None,
            tree_folders: Default::default(),
            views: BTreeMap::new(),
            explorer_view: None,
            web_off: Default::default(),
            classic_folders: false,
            own_window: false,
            shown_files: Default::default(),
            places: BTreeMap::new(),
            last: BTreeMap::new(),
            arranged: Vec::new(),
        }
    }
}

/// How many folders are remembered at most (beyond it, the first in the file's order go).
pub const MOST: usize = 200;

impl Saved {
    /// Reads what `text` wrote; lines it does not know are skipped.
    pub fn parse(text: &str) -> Saved {
        let mut saved = Saved::default();
        for line in text.lines() {
            let line = line.trim_end_matches('\r');
            if line.starts_with('#') || line.trim().is_empty() {
                continue;
            }
            if let Some(v) = line.strip_prefix("status_bar = ") {
                saved.status_bar = v.trim() != "no";
                continue;
            }
            if let Some(v) = line.strip_prefix("tree_width = ") {
                saved.tree_width = v.trim().parse().ok().filter(|w: &i32| *w > 0);
                continue;
            }
            if let Some(v) = line.strip_prefix("explorer_view = ") {
                saved.explorer_view = ViewMode::from_word(v.trim());
                continue;
            }
            if let Some(rest) = line.strip_prefix("view ") {
                let mut parts = rest.splitn(5, ' ');
                let (Some(mode), Some(arrange), Some(order), Some(columns), Some(key)) = (parts.next(), parts.next(), parts.next(), parts.next(), parts.next()) else { continue };
                let (Some(mode), Some(arrange)) = (ViewMode::from_word(mode), Arrange::from_word(arrange)) else { continue };
                if key.is_empty() {
                    continue;
                }
                let columns = if columns == "-" { Vec::new() } else { columns.split(',').filter_map(|c| c.parse().ok()).collect() };
                saved.views.insert(key.to_string(), ViewState { mode, arrange, reversed: order == "reversed", columns });
                continue;
            }
            if let Some(v) = line.strip_prefix("classic_folders = ") {
                saved.classic_folders = v.trim() == "yes";
                continue;
            }
            if let Some(v) = line.strip_prefix("own_window = ") {
                saved.own_window = v.trim() == "yes";
                continue;
            }
            if let Some(key) = line.strip_prefix("web_off ") {
                if !key.is_empty() {
                    saved.web_off.insert(key.to_string());
                }
                continue;
            }
            // moved<TAB>folder<TAB>x y id<TAB>x y id... (no file name holds a tab). Before brief
            // 103 a line "places" held every icon's place of a folder: it is read no more (those
            // folders open in order once), so no folder keeps more than the icons really moved.
            if let Some(rest) = line.strip_prefix("moved\t") {
                let mut fields = rest.split('\t');
                let Some(key) = fields.next().filter(|k| !k.is_empty()) else { continue };
                let list: Vec<(i32, i32, String)> = fields
                    .filter_map(|f| {
                        let mut p = f.splitn(3, ' ');
                        Some((p.next()?.parse().ok()?, p.next()?.parse().ok()?, p.next().filter(|s| !s.is_empty())?.to_string()))
                    })
                    .collect();
                if !list.is_empty() {
                    saved.places.insert(key.to_string(), list);
                    saved.touch(key);
                }
                continue;
            }
            // last<TAB>folder<TAB>id<TAB>id...
            if let Some(rest) = line.strip_prefix("last\t") {
                let mut fields = rest.split('\t');
                let Some(key) = fields.next().filter(|k| !k.is_empty()) else { continue };
                let list: Vec<String> = fields.filter(|f| !f.is_empty()).map(str::to_string).collect();
                if !list.is_empty() {
                    saved.last.insert(key.to_string(), list);
                    saved.touch(key);
                }
                continue;
            }
            if let Some(key) = line.strip_prefix("shown_files ") {
                if !key.is_empty() {
                    saved.shown_files.insert(key.to_string());
                }
                continue;
            }
            if let Some(key) = line.strip_prefix("tree ") {
                if !key.is_empty() {
                    saved.tree_folders.insert(key.to_string());
                }
                continue;
            }
            if let Some(rest) = line.strip_prefix("explorer ") {
                let n: Vec<i32> = rest.split(' ').filter_map(|p| p.parse().ok()).collect();
                if let [x, y, w, h, max] = n[..]
                    && w > 0
                    && h > 0
                {
                    saved.explorer = Some(Window { x, y, w, h, maximized: max == 1, arrange: Arrange::default(), auto_arrange: false });
                }
                continue;
            }
            let Some(rest) = line.strip_prefix("window ") else { continue };
            let mut parts = rest.splitn(8, ' ');
            let mut num = || parts.next().and_then(|p| p.parse::<i32>().ok());
            let (Some(x), Some(y), Some(w), Some(h), Some(max)) = (num(), num(), num(), num(), num()) else { continue };
            let (Some(arrange), Some(auto), Some(key)) = (parts.next(), parts.next(), parts.next()) else { continue };
            if key.is_empty() || w <= 0 || h <= 0 {
                continue;
            }
            let window = Window {
                x,
                y,
                w,
                h,
                maximized: max == 1,
                arrange: Arrange::from_word(arrange).unwrap_or_default(),
                auto_arrange: auto == "auto",
            };
            saved.windows.insert(key.to_string(), window);
        }
        saved
    }

    /// The file's text.
    pub fn text(&self) -> String {
        let mut out = String::from(
            "# Bevel95's folder windows: whether the status bar shows, and where each folder's window\n\
             # was (x y width height maximized arrange auto-arrange folder). Written by Bevel95.\n",
        );
        out += &format!("status_bar = {}\n", if self.status_bar { "yes" } else { "no" });
        if let Some(w) = self.tree_width {
            out += &format!("tree_width = {w}\n");
        }
        if let Some(e) = &self.explorer {
            out += &format!("explorer {} {} {} {} {}\n", e.x, e.y, e.w, e.h, i32::from(e.maximized));
        }
        for key in &self.tree_folders {
            out += &format!("tree {key}\n");
        }
        if let Some(m) = self.explorer_view {
            out += &format!("explorer_view = {}\n", m.word());
        }
        if self.classic_folders {
            out += "classic_folders = yes\n";
        }
        if self.own_window {
            out += "own_window = yes\n";
        }
        for key in &self.web_off {
            out += &format!("web_off {key}\n");
        }
        for key in &self.shown_files {
            out += &format!("shown_files {key}\n");
        }
        // The folders' moved icons, the one changed longest ago first.
        let mut keys: Vec<&String> = self.arranged.iter().filter(|k| self.places.contains_key(*k) || self.last.contains_key(*k)).collect();
        for k in self.places.keys().chain(self.last.keys()) {
            if !keys.contains(&k) {
                keys.push(k);
            }
        }
        for key in keys {
            if let Some(list) = self.places.get(key) {
                out += &format!("moved\t{key}");
                for (x, y, id) in list {
                    out += &format!("\t{x} {y} {id}");
                }
                out.push('\n');
            }
            if let Some(list) = self.last.get(key) {
                out += &format!("last\t{key}");
                for id in list {
                    out += &format!("\t{id}");
                }
                out.push('\n');
            }
        }
        for (key, v) in &self.views {
            let columns = if v.columns.is_empty() { "-".to_string() } else { v.columns.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(",") };
            out += &format!("view {} {} {} {columns} {key}\n", v.mode.word(), v.arrange.word(), if v.reversed { "reversed" } else { "forward" });
        }
        for (key, w) in &self.windows {
            out += &format!(
                "window {} {} {} {} {} {} {} {key}\n",
                w.x,
                w.y,
                w.w,
                w.h,
                i32::from(w.maximized),
                w.arrange.word(),
                if w.auto_arrange { "auto" } else { "free" }
            );
        }
        out
    }

    /// Remembers how a folder was shown (at most `MOST` folders).
    pub fn remember_view(&mut self, key: &str, view: ViewState) {
        self.views.insert(key.to_string(), view);
        while self.views.len() > MOST {
            let first = self.views.keys().next().cloned();
            if let Some(k) = first {
                self.views.remove(&k);
            }
        }
    }

    /// Remembers where the icons the person moved in a folder lie (at most `MOST_MOVED`, the last
    /// moved; at most `MOST_ARRANGED` folders, the one changed longest ago going); an empty list
    /// forgets.
    pub fn remember_places(&mut self, key: &str, mut list: Vec<(i32, i32, String)>) {
        if list.is_empty() {
            self.places.remove(key);
            self.untouch(key);
            return;
        }
        if list.len() > MOST_MOVED {
            list.drain(..list.len() - MOST_MOVED);
        }
        self.places.insert(key.to_string(), list);
        self.touch(key);
        self.prune();
    }

    /// Remembers the icons that go last in a folder's order (Windows 98 with its web view), as
    /// `remember_places` does; an empty list forgets.
    pub fn remember_last(&mut self, key: &str, mut list: Vec<String>) {
        if list.is_empty() {
            self.last.remove(key);
            self.untouch(key);
            return;
        }
        if list.len() > MOST_MOVED {
            list.drain(..list.len() - MOST_MOVED);
        }
        self.last.insert(key.to_string(), list);
        self.touch(key);
        self.prune();
    }

    /// A folder's moved icons' places (`remember_places`).
    pub fn places_of(&self, key: &str) -> Vec<(i32, i32, String)> {
        self.places.get(key).cloned().unwrap_or_default()
    }

    /// The icons that go last in a folder's order (`remember_last`).
    pub fn last_of(&self, key: &str) -> Vec<String> {
        self.last.get(key).cloned().unwrap_or_default()
    }

    /// `key` was changed just now: it goes to the end of `arranged`.
    fn touch(&mut self, key: &str) {
        self.arranged.retain(|k| k != key);
        self.arranged.push(key.to_string());
    }

    /// `key` holds nothing any more (neither places nor an order): it leaves `arranged`.
    fn untouch(&mut self, key: &str) {
        if !self.places.contains_key(key) && !self.last.contains_key(key) {
            self.arranged.retain(|k| k != key);
        }
    }

    /// Beyond `MOST_ARRANGED` folders the one changed longest ago goes.
    fn prune(&mut self) {
        while self.arranged.len() > MOST_ARRANGED {
            let oldest = self.arranged.remove(0);
            self.places.remove(&oldest);
            self.last.remove(&oldest);
        }
    }

    /// Remembers a folder's window (at most `MOST` folders).
    pub fn remember(&mut self, key: &str, window: Window) {
        self.windows.insert(key.to_string(), window);
        while self.windows.len() > MOST {
            let first = self.windows.keys().next().cloned();
            if let Some(k) = first {
                self.windows.remove(&k);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_written_reads_back() {
        let mut s = Saved { status_bar: false, tree_width: Some(230), ..Saved::default() };
        s.explorer = Some(Window { x: 0, y: 0, w: 600, h: 411, maximized: false, arrange: Arrange::default(), auto_arrange: false });
        s.tree_folders.insert("c:\\with tree".into());
        let w = Window { x: 44, y: -3, w: 265, h: 219, maximized: false, arrange: Arrange::Date, auto_arrange: true };
        s.remember("c:\\users\\x\\letters with spaces", w.clone());
        s.remember("::my-computer", Window { maximized: true, arrange: Arrange::Name, auto_arrange: false, ..w.clone() });
        s.explorer_view = Some(ViewMode::Details);
        s.web_off.insert("c:\\no web".into());
        s.shown_files.insert("c:\\windows".into());
        s.classic_folders = true;
        s.own_window = true;
        s.remember_view("c:\\a b", ViewState { mode: ViewMode::List, arrange: Arrange::Size, reversed: true, columns: vec![209, 60, 120, 120] });
        s.remember_view("::my-computer", ViewState { mode: ViewMode::Small, arrange: Arrange::Name, reversed: false, columns: Vec::new() });
        let back = Saved::parse(&s.text());
        assert_eq!(back, s);
        assert_eq!(back.windows["c:\\users\\x\\letters with spaces"], w);
        assert_eq!(Saved::parse("rubbish\nwindow 1 2 x\n"), Saved::default());
    }

    #[test]
    fn each_windows_opens_a_folder_in_its_own_view() {
        use Era::*;
        assert_eq!(opening_view(Win95, None, 77, true), ViewMode::Large);
        assert_eq!(opening_view(Win95, None, 80, true), ViewMode::List);
        assert_eq!(opening_view(Win95, None, 120, true), ViewMode::List);
        assert_eq!(opening_view(Win98, None, 120, true), ViewMode::List, "a new window (rule-v2-120-run-newwindow)");
        assert_eq!(opening_view(Win98, None, 120, false), ViewMode::Large, "the same window (rule-v3-120-from-large)");
        assert_eq!(opening_view(Win2000, None, 120, true), ViewMode::Large);
        assert_eq!(opening_view(Win2000, Some(ViewMode::Details), 3, false), ViewMode::Details, "remembered");
    }
}
