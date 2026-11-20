//! The Start menu's Programs tree: folders and shortcuts, merged from several root folders
//! (Windows has one Programs folder per user and one for all users, and shows them as one)
//! and sorted the way Windows 95 sorts them: folders first, then shortcuts, each by name.
//! Also the owner's choice of what the menu shows of it (`Shown`), with the list of programs
//! ticked by hand (`Ticked`, kept in programs.txt) and that list as Bevel95 Settings shows it
//! (`rows`).

use std::collections::BTreeSet;

use crate::fb::Icon;
use crate::paint::{Menu, MenuEntry};
use crate::text::escape_access_keys;
use crate::theme::Era;

/// A shortcut or program in the tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// The name shown (the file name without its extension).
    pub name: String,
    /// What to open (on Windows: the shortcut file's full path).
    pub target: String,
}

/// What starts a Store-style app: this, then the app's ID in Windows' list of apps (its
/// "AppsFolder"). The shell opens such a name like a file.
pub const APP_PREFIX: &str = "shell:AppsFolder\\";

/// Whether an entry of Windows' list of apps is a Store-style (packaged) app, by its ID. Such
/// an ID is "<package family>!<app>", and a package family ends in "_" and the 13 letters and
/// digits that stand for its publisher: "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App". The
/// list's other entries are classic programs: they have a shortcut file in the Start Menu
/// folders, where the Programs menu finds them, folders and all.
pub fn is_store_app(id: &str) -> bool {
    let Some((family, app)) = id.split_once('!') else { return false };
    let Some((name, publisher)) = family.rsplit_once('_') else { return false };
    !name.is_empty()
        && !app.is_empty()
        && !app.contains('!')
        && publisher.len() == 13
        && publisher.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// What the Programs menu shows: the owner's three choices (Bevel95 Settings, Programs page;
/// `programs` in settings.txt).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shown {
    /// Every program found on this PC: the Start Menu folders' shortcuts, Windows' Store-style
    /// apps, Bevel95's own programs.
    #[default]
    Everything,
    /// Only the classic programs that come with Bevel95.
    Classic,
    /// Only the programs ticked on the Programs page (kept in programs.txt).
    Ticked,
}

impl Shown {
    pub const ALL: [Shown; 3] = [Shown::Everything, Shown::Classic, Shown::Ticked];

    /// The name in settings.txt.
    pub fn setting(self) -> &'static str {
        match self {
            Shown::Everything => "everything",
            Shown::Classic => "classic",
            Shown::Ticked => "ticked",
        }
    }

    pub fn from_setting(value: &str) -> Option<Shown> {
        Shown::ALL.into_iter().find(|shown| shown.setting().eq_ignore_ascii_case(value.trim()))
    }
}

/// The Accessories folder as Windows 95, 98 and 2000 called it (today's Windows labels it
/// "Windows Accessories"; the platform layer explains how that label is handled).
pub const ACCESSORIES: &str = "Accessories";

/// One of Bevel95's own programs (`programs/` in the source): its name in the menu, its file
/// in the installed programs folder, the folders it stands in (where Windows kept it), and its
/// menu picture for the selected Windows edition.
pub struct OwnProgram {
    pub name: &'static str,
    pub file: &'static str,
    pub folder: fn(Era) -> &'static [&'static str],
    pub icon: fn(Era) -> Icon,
}

/// Bevel95's own programs as the Programs menu lists them (the Help viewer is not one: it is
/// opened from Help).
pub const OWN_PROGRAMS: [OwnProgram; 4] = [
    OwnProgram {
        name: crate::minefield::form::TITLE,
        file: "minefield.exe",
        folder: |_| &[ACCESSORIES, "Games"],
        icon: |era| crate::minefield::art::icon(era, 16),
    },
    OwnProgram {
        name: "Solitaire",
        file: "solitaire.exe",
        folder: |_| &[ACCESSORIES, "Games"],
        icon: |era| crate::solitaire::art::icon(era, 16),
    },
    OwnProgram {
        name: crate::calculator::form::TITLE,
        file: "calculator.exe",
        folder: |_| &[ACCESSORIES],
        icon: |era| crate::calculator::art::icon(era, 16),
    },
    // The folder windows' program started without a folder is Windows Explorer: in Programs
    // itself in 95 and 98, under Accessories in 2000 (`*-truecolor-explorer-programs-menu`).
    OwnProgram {
        name: "Windows Explorer",
        file: "folders.exe",
        folder: |era| if era == Era::Win2000 { &[ACCESSORIES] } else { &[] },
        icon: crate::folder::tree_art::explorer,
    },
];

/// Lists Bevel95's own programs in the Programs tree `root` where `era`'s Windows kept them:
/// the games under Accessories > Games, Calculator under Accessories, Windows Explorer where
/// that Windows had it. `target` gives what starts each one, or `None` when its file is
/// missing (then it is left out).
pub fn add_own_programs(root: &mut Folder, era: Era, mut target: impl FnMut(&OwnProgram) -> Option<String>) {
    for program in &OWN_PROGRAMS {
        if let Some(target) = target(program) {
            root.add((program.folder)(era), Item { name: program.name.to_string(), target });
        }
    }
}

/// An entry's place in the Programs menu, as programs.txt names it: the folders above it and
/// its own name, with "\" between them ("Accessories\Games\Minesweeper"). No file or folder
/// name can hold a "\".
pub fn place(folders: &[&str], name: &str) -> String {
    folders.iter().chain([&name]).copied().collect::<Vec<_>>().join("\\")
}

/// The programs ticked on the Programs page, by their places in the menu. The case of the
/// letters does not matter, as it does not in the menu's folders; nor do spaces before or after
/// a place (a line of the file loses them when it is read, and some programs' names end in
/// one: the owner's first list had such a program, which then went missing from the menu).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ticked(BTreeSet<String>);

impl Ticked {
    /// Reads programs.txt: one place on each line. Empty lines and lines starting with "#"
    /// are skipped; a place that itself starts with "#" is written with a "\" before it.
    pub fn parse(text: &str) -> Ticked {
        let places = text.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with('#'));
        Ticked(places.map(|line| Ticked::key(line.strip_prefix('\\').unwrap_or(line))).collect())
    }

    /// A place as the list keeps it.
    fn key(place: &str) -> String {
        place.trim().to_lowercase()
    }

    pub fn contains(&self, place: &str) -> bool {
        self.0.contains(&Ticked::key(place))
    }

    /// The list with every place in the outermost folder `from` moved to the folder `to`: a
    /// folder that Bevel95 used to show under another name (today's Windows labels the
    /// Accessories folder "Windows Accessories") keeps the ticks given under that name.
    pub fn with_folder_renamed(self, from: &str, to: &str) -> Ticked {
        let (from, to) = (Ticked::key(from) + "\\", Ticked::key(to) + "\\");
        let moved = |place: String| match place.strip_prefix(&from) {
            Some(rest) => format!("{to}{rest}"),
            None => place,
        };
        Ticked(self.0.into_iter().map(moved).collect())
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// What programs.txt contains for the ticked `places`.
pub fn ticked_file<'a>(places: impl IntoIterator<Item = &'a str>) -> String {
    let mut text = String::from(
        "# The programs ticked in Bevel95 Settings, on its Programs page: with \"Only the programs\n\
         # ticked here\" chosen, the Programs menu shows these and no others. One on each line, by\n\
         # its place in the menu. Bevel95 Settings writes this file anew each time.\n",
    );
    for place in places {
        if place.starts_with(['#', '\\']) {
            text.push('\\');
        }
        text.push_str(place);
        text.push('\n');
    }
    text
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Folder {
    pub name: String,
    pub folders: Vec<Folder>,
    pub items: Vec<Item>,
}

/// What a row of the Programs page's list is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKind {
    /// The first row, "Programs": the menu itself, with everything in it.
    Root,
    Folder,
    Item,
}

/// A row of the Programs page's list: the Programs menu laid out as a list, each folder
/// followed by what is in it, one step further in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// How far in it stands: 0 for "Programs" itself, 1 for what is in it, and so on.
    pub depth: usize,
    pub kind: RowKind,
    pub name: String,
    /// Its place in the menu: an item's is what programs.txt lists, a folder's tells the folder
    /// apart from others of its name. "Programs" itself has none.
    pub place: String,
    /// An item is ticked in the list. Folders show what their items add up to (`tick`).
    pub ticked: bool,
    /// An item is one of Bevel95's own programs (what `Shown::Classic` shows).
    pub own: bool,
    /// An item's picture (folders get the look's own).
    pub icon: Option<Icon>,
    /// A folder's rows show (its box in the tree has a minus). "Programs" starts out open, the
    /// folders closed, so the list begins as short as the Programs menu's first column.
    pub open: bool,
    /// No further row of its level follows in its folder: the tree's line down ends at it.
    pub last: bool,
}

/// What a row's box shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tick {
    Off,
    On,
    /// A folder with some of its programs ticked and some not.
    Mixed,
}

/// The Programs page's list for the whole `tree`: "Programs" first, then its folders and
/// items in the menu's own order (a folder with no program anywhere in it is left out). An
/// item is ticked if `ticked` lists its place; without a list (no programs.txt yet) every item
/// is. `detail` says of an item whether it is one of Bevel95's own, and gives its picture.
pub fn rows(tree: &Folder, ticked: Option<&Ticked>, mut detail: impl FnMut(&Item) -> (bool, Option<Icon>)) -> Vec<Row> {
    let root = Row {
        depth: 0,
        kind: RowKind::Root,
        name: "Programs".to_string(),
        place: String::new(),
        ticked: false,
        own: false,
        icon: None,
        open: true,
        last: true,
    };
    let mut rows = vec![root];
    add_rows(&mut rows, tree, &mut Vec::new(), ticked, &mut detail);
    mark_last(&mut rows);
    rows
}

fn add_rows<'a>(
    rows: &mut Vec<Row>,
    folder: &'a Folder,
    above: &mut Vec<&'a str>,
    ticked: Option<&Ticked>,
    detail: &mut impl FnMut(&Item) -> (bool, Option<Icon>),
) {
    let depth = above.len() + 1;
    let blank = Row {
        depth,
        kind: RowKind::Folder,
        name: String::new(),
        place: String::new(),
        ticked: false,
        own: false,
        icon: None,
        open: false,
        last: false,
    };
    for inner in &folder.folders {
        rows.push(Row { name: inner.name.clone(), place: place(above, &inner.name), ..blank.clone() });
        let before = rows.len();
        above.push(&inner.name);
        add_rows(rows, inner, above, ticked, detail);
        above.pop();
        if rows.len() == before {
            // A folder with no program anywhere in it has nothing to tick: it gets no row.
            rows.pop();
        }
    }
    for item in &folder.items {
        let place = place(above, &item.name);
        let (own, icon) = detail(item);
        let is_ticked = ticked.is_none_or(|list| list.contains(&place));
        let (kind, name) = (RowKind::Item, item.name.clone());
        rows.push(Row { kind, name, place, ticked: is_ticked, own, icon, ..blank.clone() });
    }
}

/// The rows that show (their numbers in `rows`): those whose folders, all the way out, are
/// open.
pub fn visible(rows: &[Row]) -> Vec<usize> {
    let mut shown = Vec::new();
    // The depth of the closed folder whose rows are being passed over.
    let mut closed: Option<usize> = None;
    for (i, row) in rows.iter().enumerate() {
        if closed.is_some_and(|depth| row.depth > depth) {
            continue;
        }
        shown.push(i);
        closed = (row.kind != RowKind::Item && !row.open).then_some(row.depth);
    }
    shown
}

/// Marks each row that is the last of its level in its folder (`Row::last`), going through the
/// rows from the end: a row is the last, unless one of its level was met since the last row
/// that stands further out.
pub fn mark_last(rows: &mut [Row]) {
    let mut met = vec![false; rows.iter().map(|row| row.depth).max().unwrap_or(0) + 2];
    for row in rows.iter_mut().rev() {
        row.last = !met[row.depth];
        met[row.depth] = true;
        met[row.depth + 1..].fill(false);
    }
}

/// A tree of rows in a list that shows `page` of them at a time: which row is selected (its
/// number in `rows`) and how many of the rows that show are scrolled out above. What keys and
/// mouse do with it, as in Windows' trees; shared by Bevel95 Settings' list of programs and
/// the Remove Shortcuts/Folders window.
pub struct Cursor<'a> {
    pub rows: &'a mut Vec<Row>,
    pub sel: &'a mut usize,
    pub top: &'a mut usize,
    pub page: usize,
}

impl Cursor<'_> {
    /// Opens or closes folder `i`. A selection inside a folder that closes goes to the folder.
    pub fn open(&mut self, i: usize, open: bool) {
        let Some(row) = self.rows.get_mut(i).filter(|row| row.kind != RowKind::Item) else { return };
        row.open = open;
        let shows = visible(self.rows);
        if !shows.contains(self.sel) {
            *self.sel = i;
        }
        *self.top = (*self.top).min(shows.len().saturating_sub(self.page));
    }

    /// Scrolls so that row `i` shows (it must be in open folders).
    pub fn show(&mut self, i: usize) {
        let Some(at) = visible(self.rows).iter().position(|&row| row == i) else { return };
        if at < *self.top {
            *self.top = at;
        } else if at >= *self.top + self.page {
            *self.top = at + 1 - self.page;
        }
    }

    /// Moves the selection by `delta` of the rows that show, scrolling to keep it in view.
    pub fn move_by(&mut self, delta: i32) {
        let shows = visible(self.rows);
        let at = shows.iter().position(|row| row == self.sel).unwrap_or(0) as i32;
        let last = shows.len().saturating_sub(1) as i32;
        if let Some(&row) = shows.get((at + delta).clamp(0, last) as usize) {
            *self.sel = row;
            self.show(row);
        }
    }

    /// A letter typed: the selection goes to the next row that shows whose name starts with
    /// it, around the end of the list (as Windows' lists do). False if none does.
    pub fn find(&mut self, ch: char) -> bool {
        let shows = visible(self.rows);
        let starts = |row: &Row| row.name.chars().next().is_some_and(|c| c.to_lowercase().eq(ch.to_lowercase()));
        let at = shows.iter().position(|row| row == self.sel).unwrap_or(0);
        let mut around = shows.iter().cycle().skip(at + 1).take(shows.len());
        let Some(&found) = around.find(|&&row| starts(&self.rows[row])) else { return false };
        *self.sel = found;
        self.show(found);
        true
    }

    /// The Right key: a closed folder opens; an open one passes the selection to its first row.
    /// The Left key: an open folder closes; any other row passes the selection to the folder
    /// it is in.
    pub fn step(&mut self, right: bool) {
        let sel = *self.sel;
        let Some(row) = self.rows.get(sel) else { return };
        match (right, row.kind != RowKind::Item, row.open) {
            (true, true, false) => self.open(sel, true),
            (true, true, true) => self.move_by(1),
            (false, true, true) => self.open(sel, false),
            (false, ..) => {
                if let Some(above) = parent(self.rows, sel) {
                    *self.sel = above;
                    self.show(above);
                }
            }
            (true, false, _) => {}
        }
    }

    /// The "*" key: the selected folder opens with every folder in it.
    pub fn open_all(&mut self) {
        let sel = *self.sel;
        let Some(depth) = self.rows.get(sel).filter(|row| row.kind != RowKind::Item).map(|row| row.depth) else {
            return;
        };
        self.rows[sel].open = true;
        for row in self.rows[sel + 1..].iter_mut().take_while(|row| row.depth > depth) {
            row.open = true;
        }
    }
}

/// The folder (or "Programs") that row `i` is in.
pub fn parent(rows: &[Row], i: usize) -> Option<usize> {
    let depth = rows[i].depth;
    (0..i).rev().find(|&above| rows[above].depth < depth)
}

/// For the tree's lines: whether each folder that row `i` is in, from "Programs" on, and at
/// the end row `i` itself, is the last row of its level.
pub fn lasts(rows: &[Row], i: usize) -> Vec<bool> {
    let mut lasts = vec![rows[i].last];
    let mut at = i;
    while let Some(above) = parent(rows, at) {
        lasts.push(rows[above].last);
        at = above;
    }
    lasts.reverse();
    lasts
}

/// Opens folders so that what `shown` shows can be seen at once. With the classic programs:
/// the folders that hold Bevel95's own, and no others. With the ticked ones: the folders in
/// which some programs are ticked and some are not (a folder's own box says the rest); the
/// others stay as they are. With everything, nothing changes.
pub fn reveal(rows: &mut [Row], shown: Shown) {
    for i in 0..rows.len() {
        if rows[i].kind == RowKind::Item {
            continue;
        }
        let state = tick(rows, i, shown);
        match shown {
            Shown::Everything => {}
            Shown::Classic => rows[i].open = rows[i].kind == RowKind::Root || state != Tick::Off,
            Shown::Ticked => rows[i].open |= state == Tick::Mixed,
        }
    }
}

/// The programs that `shown` shows become the ticked ones: a box flipped while another choice
/// is made starts from what that choice shows (the classic programs as a base to add to).
pub fn adopt(rows: &mut [Row], shown: Shown) {
    for row in rows.iter_mut().filter(|row| row.kind == RowKind::Item) {
        row.ticked = match shown {
            Shown::Everything => true,
            Shown::Classic => row.own,
            Shown::Ticked => row.ticked,
        };
    }
}

/// "Remove Shortcuts/Folders" takes the programs at `places` out of the menu: what `shown`
/// shows becomes the ticked ones (`adopt`), without these. They stay in the list, unticked,
/// and can be ticked again.
pub fn hide(rows: &mut [Row], shown: Shown, places: &[String]) {
    adopt(rows, shown);
    for row in rows.iter_mut().filter(|row| row.kind == RowKind::Item) {
        if places.contains(&row.place) {
            row.ticked = false;
        }
    }
}

/// The mirror of `hide`, for Taskbar Properties' Add...: a shortcut made with Windows' Create
/// Shortcut wizard joins the menu whatever it shows. What `shown` shows becomes the ticked
/// ones (`adopt`), with the programs at `places` ticked as well.
pub fn unhide(rows: &mut [Row], shown: Shown, places: &[String]) {
    adopt(rows, shown);
    for row in rows.iter_mut().filter(|row| row.kind == RowKind::Item) {
        if places.contains(&row.place) {
            row.ticked = true;
        }
    }
}

/// The rows inside row `i` (a folder's, or all the others for "Programs"): those after it
/// that stand further in, up to the next one that does not.
pub fn inside(rows: &[Row], i: usize) -> std::ops::Range<usize> {
    let depth = rows[i].depth;
    i + 1..i + 1 + rows[i + 1..].iter().take_while(|row| row.depth > depth).count()
}

/// What row `i`'s box shows while the menu shows `shown`: with everything, every program is
/// ticked; with the classic programs, Bevel95's own; else what was ticked by hand. A folder
/// (and "Programs") shows what its programs add up to.
pub fn tick(rows: &[Row], i: usize, shown: Shown) -> Tick {
    let on = |row: &Row| match shown {
        Shown::Everything => true,
        Shown::Classic => row.own,
        Shown::Ticked => row.ticked,
    };
    if rows[i].kind == RowKind::Item {
        return if on(&rows[i]) { Tick::On } else { Tick::Off };
    }
    let (mut any, mut all) = (false, true);
    for row in rows[inside(rows, i)].iter().filter(|row| row.kind == RowKind::Item) {
        any |= on(row);
        all &= on(row);
    }
    match (any, all) {
        (true, true) => Tick::On,
        (true, false) => Tick::Mixed,
        _ => Tick::Off,
    }
}

/// Flips row `i`'s box: an item's own tick; for a folder (or "Programs") every program in it,
/// which all get a tick unless all of them had one.
pub fn toggle(rows: &mut [Row], i: usize) {
    if rows[i].kind == RowKind::Item {
        rows[i].ticked = !rows[i].ticked;
        return;
    }
    let to = tick(rows, i, Shown::Ticked) != Tick::On;
    let range = inside(rows, i);
    for row in rows[range].iter_mut().filter(|row| row.kind == RowKind::Item) {
        row.ticked = to;
    }
}

/// The places of the ticked programs, for programs.txt.
pub fn ticked_places(rows: &[Row]) -> impl Iterator<Item = &str> {
    rows.iter().filter(|row| row.kind == RowKind::Item && row.ticked).map(|row| row.place.as_str())
}

/// What an entry of a folder's menu stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry<'a> {
    Folder(&'a Folder),
    Item(&'a Item),
}

impl Folder {
    /// The folder at `path` (names, from this one down), created if missing. Names match
    /// regardless of case, so "Accessories" from two roots becomes one folder.
    pub fn folder_mut(&mut self, path: &[&str]) -> &mut Folder {
        let Some((first, rest)) = path.split_first() else {
            return self;
        };
        let index = match self.folders.iter().position(|f| crate::text::same_name(&f.name, first)) {
            Some(i) => i,
            None => {
                self.folders.push(Folder { name: first.to_string(), ..Folder::default() });
                self.folders.len() - 1
            }
        };
        self.folders[index].folder_mut(rest)
    }

    /// Adds `item` to the folder at `path`. A second item with the same name in the same
    /// folder (the same shortcut in both roots) is left out, as Windows shows it once.
    pub fn add(&mut self, path: &[&str], item: Item) {
        let folder = self.folder_mut(path);
        if !folder.items.iter().any(|i| crate::text::same_name(&i.name, &item.name)) {
            folder.items.push(item);
        }
    }

    /// Sorts this folder and everything below it: folders, then items, by name ignoring case.
    pub fn sort(&mut self) {
        self.folders.sort_by_key(|f| f.name.to_lowercase());
        self.items.sort_by_key(|i| i.name.to_lowercase());
        for folder in &mut self.folders {
            folder.sort();
        }
    }

    /// Keeps only the items `keep` says yes to (asked with each item and its `place` in the
    /// menu) and drops every folder that is left with nothing in it.
    pub fn retain(&mut self, keep: &impl Fn(&Item, &str) -> bool) {
        self.retain_below(&mut Vec::new(), keep);
    }

    fn retain_below(&mut self, above: &mut Vec<String>, keep: &impl Fn(&Item, &str) -> bool) {
        let names: Vec<&str> = above.iter().map(String::as_str).collect();
        self.items.retain(|item| keep(item, &place(&names, &item.name)));
        for folder in &mut self.folders {
            above.push(folder.name.clone());
            folder.retain_below(above, keep);
            above.pop();
        }
        self.folders.retain(|folder| !folder.folders.is_empty() || !folder.items.is_empty());
    }

    /// The folder reached by following entry indices of successive `menu()`s.
    pub fn folder_at(&self, path: &[usize]) -> Option<&Folder> {
        match path.split_first() {
            None => Some(self),
            Some((&first, rest)) => match self.entry(first)? {
                Entry::Folder(folder) => folder.folder_at(rest),
                Entry::Item(_) => None,
            },
        }
    }

    /// What entry `index` of `menu()` stands for.
    pub fn entry(&self, index: usize) -> Option<Entry<'_>> {
        match index.checked_sub(self.folders.len()) {
            None => self.folders.get(index).map(Entry::Folder),
            Some(i) => self.items.get(i).map(Entry::Item),
        }
    }

    /// This folder as a menu: subfolders (opening further menus), then items. An empty folder
    /// shows a single "(Empty)" entry, as Windows does.
    pub fn menu(&self, folder_icon: Option<&Icon>, mut item_icon: impl FnMut(&Item) -> Option<Icon>) -> Menu {
        let mut entries: Vec<MenuEntry> = self
            .folders
            .iter()
            .map(|f| MenuEntry::submenu(&escape_access_keys(&f.name), folder_icon.cloned()))
            .collect();
        entries.extend(self.items.iter().map(|i| MenuEntry::item(&escape_access_keys(&i.name), item_icon(i))));
        if entries.is_empty() {
            let mut empty = MenuEntry::item("(Empty)", None);
            empty.disabled = true;
            entries.push(empty);
        }
        Menu::new(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::EntryKind;

    fn item(name: &str) -> Item {
        Item { name: name.into(), target: format!("C:\\{name}.lnk") }
    }

    #[test]
    fn two_roots_merge_into_one_sorted_tree() {
        let mut root = Folder::default();
        // "Yours"
        root.add(&["Accessories"], item("Notepad"));
        root.add(&[], item("Zebra"));
        root.add(&["Startup"], item("Sync"));
        // "All users": same folder name in another case, a duplicate shortcut, a new folder
        root.add(&["accessories"], item("Calculator"));
        root.add(&["Accessories"], item("notepad"));
        root.add(&["Games"], item("Solitaire"));
        root.add(&[], item("Alpha"));
        root.sort();
        let folders: Vec<&str> = root.folders.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(folders, ["Accessories", "Games", "Startup"]);
        let accessories: Vec<&str> = root.folders[0].items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(accessories, ["Calculator", "Notepad"]);
        let items: Vec<&str> = root.items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(items, ["Alpha", "Zebra"]);
    }

    #[test]
    fn menus_list_folders_first_and_entries_map_back() {
        let mut root = Folder::default();
        root.add(&["Games"], item("Solitaire"));
        root.add(&[], item("Paint"));
        let menu = root.menu(None, |_| None);
        let kinds: Vec<EntryKind> = menu.entries.iter().map(|e| e.kind).collect();
        assert_eq!(kinds, [EntryKind::Submenu, EntryKind::Item]);
        assert!(matches!(root.entry(0), Some(Entry::Folder(f)) if f.name == "Games"));
        assert!(matches!(root.entry(1), Some(Entry::Item(i)) if i.name == "Paint"));
        assert_eq!(root.entry(2), None);
        assert_eq!(root.folder_at(&[0]).map(|f| f.items.len()), Some(1));
        assert_eq!(root.folder_at(&[1]), None);
    }

    #[test]
    fn store_style_apps_are_told_by_their_ids() {
        for id in [
            "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App",
            "Microsoft.WindowsNotepad_8wekyb3d8bbwe!App",
            "windows.immersivecontrolpanel_cw5n1h2txyewy!microsoft.windows.immersivecontrolpanel",
            "1234ABCD.ExampleChat_a1b2c3d4e5f6g!App",
        ] {
            assert!(is_store_app(id), "{id}");
        }
        for id in [
            // classic programs: a path below a known folder, a name of the program's own
            r"{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\notepad.exe",
            "Chrome",
            "Microsoft.Windows.Explorer",
            "Microsoft.AutoGenerated.{C1C6F8AC-40A3-0F5C-146F-65A9DC70BBB4}",
            // not quite the shape: no app, no publisher, a publisher of another length
            "Microsoft.WindowsCalculator_8wekyb3d8bbwe!",
            "Microsoft.WindowsCalculator!App",
            "Microsoft.WindowsCalculator_8wekyb3d8bbw!App",
            "_8wekyb3d8bbwe!App",
            "",
        ] {
            assert!(!is_store_app(id), "{id}");
        }
    }

    #[test]
    fn the_choice_of_what_shows_has_its_names() {
        assert_eq!(Shown::default(), Shown::Everything);
        for shown in Shown::ALL {
            assert_eq!(Shown::from_setting(shown.setting()), Some(shown));
        }
        assert_eq!(Shown::from_setting(" Ticked "), Some(Shown::Ticked));
        assert_eq!(Shown::from_setting("all"), None);
    }

    /// A small Programs menu: two folders with programs (one of them with a folder of its
    /// own), a folder with nothing in it, two programs in Programs itself.
    fn sample() -> Folder {
        let mut root = Folder::default();
        root.add(&["Accessories", "Games"], item("Minesweeper"));
        root.add(&["Accessories", "Games"], item("Solitaire"));
        root.add(&["Accessories"], item("Notepad"));
        root.add(&["Startup"], item("Sync"));
        root.folder_mut(&["Empty"]);
        root.add(&[], item("Calculator"));
        root.add(&[], item("#hash"));
        root.sort();
        root
    }

    #[test]
    fn ticked_programs_round_trip_through_their_file() {
        let text = ticked_file(["Accessories\\Games\\Minesweeper", "Calculator", "#hash"]);
        assert!(text.starts_with("# "));
        assert!(text.ends_with("Accessories\\Games\\Minesweeper\nCalculator\n\\#hash\n"), "{text}");
        let ticked = Ticked::parse(&text);
        assert_eq!(ticked.len(), 3);
        assert!(ticked.contains("calculator") && ticked.contains("#hash"));
        assert!(ticked.contains("ACCESSORIES\\games\\Minesweeper") && !ticked.contains("Notepad"));
        assert!(Ticked::parse("# nothing ticked\n\n").is_empty());
        // Written by hand, with Windows' line ends and spaces around.
        assert!(Ticked::parse("  Calculator \r\n").contains("Calculator"));
        // A program whose name ends in a space (its shortcut file is "Sync .lnk"): the line of
        // the file loses the space, and the program is still found.
        let spaced = Ticked::parse(&ticked_file(["Startup\\Sync ", " Odd"]));
        assert!(spaced.contains("Startup\\Sync ") && spaced.contains("Startup\\Sync"));
        assert!(spaced.contains(" Odd") && spaced.len() == 2);
        let mut tree = Folder::default();
        tree.add(&["Startup"], item("Sync "));
        tree.add(&["Startup"], item("Other"));
        tree.retain(&|_, place| spaced.contains(place));
        assert_eq!(tree.folders[0].items, [item("Sync ")]);
        let list = rows(&tree, Some(&spaced), |_| (false, None));
        assert!(list[2].ticked, "and its box is ticked when the page lists it again");
    }

    #[test]
    fn only_the_wanted_items_stay_and_empty_folders_go() {
        let ticked = Ticked::parse("Accessories\\Games\\Solitaire\nCalculator\n");
        let mut tree = sample();
        tree.retain(&|_, place| ticked.contains(place));
        assert_eq!(tree.folders.len(), 1, "Startup and the folder with nothing in it are gone");
        assert!(tree.folders[0].items.is_empty(), "Notepad too");
        assert_eq!(tree.folders[0].folders[0].items, [item("Solitaire")]);
        assert_eq!(tree.items, [item("Calculator")]);
        let mut none = sample();
        none.retain(&|_, _| false);
        assert_eq!(none, Folder::default());
        let mut all = sample();
        all.retain(&|_, _| true);
        assert_eq!(all.folders.len(), 2, "only the folder with nothing in it goes");
    }

    #[test]
    fn the_list_lays_the_menu_out_and_adds_folders_up() {
        let tree = sample();
        let own = |i: &Item| (i.name == "Minesweeper" || i.name == "Solitaire", None);
        // Without a list of ticked programs, every program is ticked.
        let mut list = rows(&tree, None, own);
        let names: Vec<(usize, &str)> = list.iter().map(|r| (r.depth, r.name.as_str())).collect();
        assert_eq!(
            names,
            [
                (0, "Programs"),
                (1, "Accessories"),
                (2, "Games"),
                (3, "Minesweeper"),
                (3, "Solitaire"),
                (2, "Notepad"),
                (1, "Startup"),
                (2, "Sync"),
                (1, "#hash"),
                (1, "Calculator"),
            ],
            "the folder with nothing in it has nothing to tick: no row"
        );
        assert_eq!(list[3].place, "Accessories\\Games\\Minesweeper");
        assert_eq!((list[1].kind, list[3].kind), (RowKind::Folder, RowKind::Item));
        assert!((0..list.len()).all(|i| tick(&list, i, Shown::Ticked) == Tick::On));
        // What each choice shows: everything; Bevel95's own; the ticked ones.
        assert_eq!(tick(&list, 5, Shown::Everything), Tick::On);
        assert_eq!(
            [0, 1, 2, 3, 5, 6].map(|i| tick(&list, i, Shown::Classic)),
            [Tick::Mixed, Tick::Mixed, Tick::On, Tick::On, Tick::Off, Tick::Off]
        );
        // Unticking one program leaves its folders half ticked; a folder's box flips all in it.
        toggle(&mut list, 5);
        assert_eq!(
            [0, 1, 2, 5].map(|i| tick(&list, i, Shown::Ticked)),
            [Tick::Mixed, Tick::Mixed, Tick::On, Tick::Off]
        );
        toggle(&mut list, 1);
        assert_eq!(tick(&list, 1, Shown::Ticked), Tick::On, "half ticked becomes all ticked");
        toggle(&mut list, 1);
        assert_eq!([1, 2, 3, 5].map(|i| tick(&list, i, Shown::Ticked)), [Tick::Off; 4]);
        assert_eq!(ticked_places(&list).collect::<Vec<_>>(), ["Startup\\Sync", "#hash", "Calculator"]);
        toggle(&mut list, 0);
        assert_eq!(ticked_places(&list).count(), 6, "Programs' own box ticks everything");
        toggle(&mut list, 0);
        assert_eq!(ticked_places(&list).count(), 0);
        // With a list, only what it names is ticked.
        let ticked = Ticked::parse("calculator\n");
        let list = rows(&tree, Some(&ticked), own);
        assert_eq!(ticked_places(&list).collect::<Vec<_>>(), ["Calculator"]);
    }

    #[test]
    fn the_list_is_a_tree_whose_folders_open_and_close() {
        let own = |i: &Item| (i.name == "Minesweeper" || i.name == "Solitaire", None);
        // Rows: 0 Programs, 1 Accessories, 2 Games, 3 Minesweeper, 4 Solitaire, 5 Notepad,
        // 6 Startup, 7 Sync, 8 #hash, 9 Calculator.
        let mut list = rows(&sample(), None, own);
        assert_eq!(list[2].place, "Accessories\\Games", "a folder's place tells it apart");
        // It starts as short as the menu's first column: Programs open, its folders closed.
        assert_eq!(visible(&list), [0, 1, 6, 8, 9]);
        list[1].open = true;
        assert_eq!(visible(&list), [0, 1, 2, 5, 6, 8, 9]);
        list[2].open = true;
        assert_eq!(visible(&list), [0, 1, 2, 3, 4, 5, 6, 8, 9]);
        list[1].open = false;
        assert_eq!(visible(&list), [0, 1, 6, 8, 9], "a closed folder hides its open ones too");
        assert_eq!([0, 1, 2, 3].map(|i| parent(&list, i)), [None, Some(0), Some(1), Some(2)]);
        // Where each level's line down ends: at the last row of that level.
        let last: Vec<bool> = list.iter().map(|row| row.last).collect();
        assert_eq!(last, [true, false, false, false, true, true, false, true, false, true]);
        assert_eq!(lasts(&list, 4), [true, false, false, true], "Programs, Accessories, Games, Solitaire");
        // The classic choice opens the folders that hold Bevel95's own programs, and only them.
        list[6].open = true;
        reveal(&mut list, Shown::Classic);
        assert_eq!(visible(&list), [0, 1, 2, 3, 4, 5, 6, 8, 9]);
        assert!(!list[6].open, "Startup holds none of them");
        // The ticked choice opens the folders with some ticked and some not: a folder with
        // all or none of its programs ticked says so with its own box.
        for row in &mut list {
            row.open = row.kind == RowKind::Root;
        }
        toggle(&mut list, 5);
        reveal(&mut list, Shown::Ticked);
        assert_eq!(visible(&list), [0, 1, 2, 5, 6, 8, 9], "Accessories opens, Games (all ticked) stays shut");
        reveal(&mut list, Shown::Everything);
        assert_eq!(visible(&list), [0, 1, 2, 5, 6, 8, 9], "everything: nothing to look for");
        // A box flipped while the classic programs show starts from them.
        adopt(&mut list, Shown::Classic);
        let games = ["Accessories\\Games\\Minesweeper", "Accessories\\Games\\Solitaire"];
        assert_eq!(ticked_places(&list).collect::<Vec<_>>(), games);
        adopt(&mut list, Shown::Everything);
        assert_eq!(ticked_places(&list).count(), 6);
        // Remove Shortcuts/Folders: what the menu shows now stays ticked, but for what goes.
        hide(&mut list, Shown::Classic, &[games[0].to_string()]);
        assert_eq!(ticked_places(&list).collect::<Vec<_>>(), games[1..]);
        hide(&mut list, Shown::Ticked, &["Calculator".to_string()]);
        assert_eq!(ticked_places(&list).collect::<Vec<_>>(), games[1..], "it was not showing anyway");
        hide(&mut list, Shown::Everything, &["Calculator".to_string(), "Accessories\\Notepad".to_string()]);
        assert_eq!(ticked_places(&list).count(), 4);
        // Add...'s new shortcut, the mirror: what the menu shows now stays ticked, and the new
        // program joins it.
        unhide(&mut list, Shown::Classic, &["Calculator".to_string()]);
        let classic_and_new = [games[0], games[1], "Calculator"];
        let mut now: Vec<&str> = ticked_places(&list).collect();
        now.sort();
        assert_eq!(now, classic_and_new, "the classic set plus the new program");
        unhide(&mut list, Shown::Ticked, &["Accessories\\Notepad".to_string()]);
        assert_eq!(ticked_places(&list).count(), 4, "a hand-ticked list keeps its ticks and gains one");
        assert!(ticked_places(&list).any(|place| place == "Accessories\\Notepad"));
        unhide(&mut list, Shown::Ticked, &["Calculator".to_string(), "Nowhere\\Gone".to_string()]);
        assert_eq!(ticked_places(&list).count(), 4, "already ticked, and a place not in the list");
        unhide(&mut list, Shown::Everything, &[]);
        assert_eq!(ticked_places(&list).count(), 6);
    }

    #[test]
    fn ticks_follow_a_folder_shown_under_a_new_name() {
        let old = "Windows Accessories\\Games\\Minesweeper\nCalculator\nWindows Accessories Extra\\Tool\n";
        let ticked = Ticked::parse(old).with_folder_renamed("Windows Accessories", "Accessories");
        assert!(ticked.contains("Accessories\\Games\\Minesweeper") && ticked.contains("Calculator"));
        assert!(!ticked.contains("Windows Accessories\\Games\\Minesweeper"));
        assert!(ticked.contains("Windows Accessories Extra\\Tool"), "another folder that only begins the same");
        assert_eq!(ticked.len(), 3);
    }

    #[test]
    fn empty_folders_say_so() {
        let menu = Folder::default().menu(None, |_| None);
        assert_eq!(menu.entries.len(), 1);
        assert_eq!(menu.entries[0].label, "(Empty)");
        assert!(menu.entries[0].disabled);
    }
}
