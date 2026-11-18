//! A folder window's menus: the menu bar's, the menu of its empty area, and the help text its
//! status bar shows for the highlighted entry. Windows 95's words were read from its shell32's and
//! explorer.exe's string tables (`private/probes/folders/*-strings.txt`); Windows 98's and 2000's off
//! their status bars, entry by entry (`private/probes/folders/seq/<era>/*.png`).
//!
//! Limited to what works here (as the desktop's): left out, not grayed, are File's Find, Format,
//! Print, Send To, Quick View, Backup, Sharing, Search and Work Offline, Edit's Undo, Copy To Folder
//! and Move To Folder, View's Toolbar(s), Explorer Bar, as Web Page, Small Icons, List, Details,
//! Thumbnails, Customize this Folder, Line up Icons and (Folder) Options, the Go menu's web pages
//! and programs, the Favorites and Help menus, and 2000's Tools but for Folder Options. Entries Windows grayed for the selection
//! stay grayed (Delete and Rename on a drive; everything about the selection while nothing is
//! selected; Back, Forward and Up One Level where they cannot go). The menu on icons is the
//! desktop's (`desktop::item_menu`).
//!
//! Windows 98 and 2000 add the places: 98's Go menu and 2000's View > Go To (Back, Forward, Up One
//! Level, My Computer), and 98's File menu lists the folders from My Computer down to the one
//! shown, that one ticked.

use crate::desktop::{self, DesktopCommand, ItemKind};
use crate::desktop_icons::{Arrange, ViewMode};
use crate::paint::{EntryKind, Mark, Menu, MenuEntry};
use crate::theme::Era;

/// What a folder window's menu entry does (the platform carries it out).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    /// What the desktop's menus do too (Open, Explore, Cut, Copy, Paste, Paste Shortcut, Create
    /// Shortcut, Delete, Rename, Properties of the selected items, New > ..., Arrange Icons > ...,
    /// Auto Arrange, Refresh).
    Shared(DesktopCommand),
    Close,
    SelectAll,
    InvertSelection,
    /// View > Status Bar: shown or not.
    StatusBar,
    /// View > Large Icons, Small Icons, List, Details (the empty area's View and 98's and 2000's
    /// Views button too): the view chosen has the menu's dot.
    View(ViewMode),
    /// The empty area's Properties: Windows' own Properties of the folder (My Computer's: the
    /// System Properties).
    FolderProperties,
    /// Backspace, Up, Up One Level: the folder one level up.
    Up,
    /// Back and Forward (98 and 2000: Alt+Left, Alt+Right).
    Back,
    Forward,
    /// The Go menu's My Computer.
    GoMyComputer,
    /// 98's File menu: the `n`th place from My Computer down to the one shown.
    Chain(u8),
    /// A command of the menu on a tree row (or a key while the tree has the keyboard), on that
    /// row's folder.
    TreeRow(DesktopCommand),
    /// 98's View > Explorer Bar > All Folders (true) and None (false); 2000's View > Explorer
    /// Bar > Folders and the Folders button switch it (`ToggleTree`).
    TreeShown(bool),
    ToggleTree,
    /// Windows 98's View > as Web Page: the web view's panel for this folder, on or off.
    WebView,
    /// Windows 2000's Tools > Folder Options...
    FolderOptions,
    /// Edit > Undo (the empty area's Undo, Ctrl+Z): the last move, copy, rename or delete taken
    /// back.
    Undo,
}

impl Command {
    /// Whether the command acts on the selected items: then it runs only on exactly the items its
    /// menu was opened for, and not at all when one of them went meanwhile (the third safety
    /// review of 2026-11-18, A2). Every other command (the views, Refresh, Up, Close, Paste into
    /// the folder shown, Undo, ...) runs whatever happened to the selection (the fourth review,
    /// C4: they were dropped too).
    pub fn acts_on_selection(&self) -> bool {
        match self {
            Command::Shared(c) => matches!(
                c,
                DesktopCommand::Open
                    | DesktopCommand::Explore
                    | DesktopCommand::Cut
                    | DesktopCommand::Copy
                    | DesktopCommand::CreateShortcut
                    | DesktopCommand::Delete
                    | DesktopCommand::Rename
                    | DesktopCommand::ItemProperties
                    | DesktopCommand::EmptyRecycleBin
            ),
            Command::InvertSelection => true,
            _ => false,
        }
    }

    /// Whether the command chosen from a menu is carried out: `selection_still_there` tells
    /// whether every item the menu was opened for is still there (and selected again).
    pub fn runs_after_menu(&self, selection_still_there: impl FnOnce() -> bool) -> bool {
        !self.acts_on_selection() || selection_still_there()
    }
}

/// A submenu of the folder window's menus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sub {
    New,
    Arrange,
    /// Windows 2000's View > Go To.
    GoTo,
    /// 98's and 2000's View > Explorer Bar.
    ExplorerBar,
    /// The empty area's View (and 98's and 2000's Views button's menu): the four views.
    Views,
}

/// One row of a menu: what it shows and does, and the status bar's text while it is highlighted.
#[derive(Clone, Debug)]
pub struct Row {
    pub entry: MenuEntry,
    pub command: Option<Command>,
    pub sub: Option<Sub>,
    pub help: String,
}

impl Row {
    fn item(label: &str, command: Command, help: &str) -> Row {
        Row { entry: MenuEntry::item(label, None), command: Some(command), sub: None, help: help.to_string() }
    }
    fn sub(label: &str, sub: Sub, help: &str) -> Row {
        Row { entry: MenuEntry::submenu(label, None), command: None, sub: Some(sub), help: help.to_string() }
    }
    fn separator() -> Row {
        Row { entry: MenuEntry::separator(), command: None, sub: None, help: String::new() }
    }
    fn grayed(mut self, grayed: bool) -> Row {
        self.entry.disabled = grayed;
        self
    }
    fn ticked(mut self, ticked: bool) -> Row {
        self.entry.mark = ticked.then_some(Mark::Check);
        self
    }
}

/// Where the window can go (98 and 2000).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Nav {
    pub back: bool,
    pub forward: bool,
    pub up: bool,
    /// The places' names from My Computer down to the one shown (98's File menu).
    pub chain: Vec<String>,
    pub at_my_computer: bool,
}

/// What the menus depend on.
#[derive(Clone, Debug)]
pub struct State {
    pub era: Era,
    /// The selected icons' kinds (empty: nothing selected).
    pub kinds: Vec<ItemKind>,
    /// A folder of files (My Computer is not: it has no New, and nothing can be pasted into it).
    pub files: bool,
    /// The clipboard holds files.
    pub can_paste: bool,
    pub status_bar: bool,
    pub auto_arrange: bool,
    pub nav: Nav,
    /// 98 and 2000: whether the tree shows (`None` in 95, whose Explorer was a window of its own
    /// kind).
    pub tree: Option<bool>,
    /// The view shown.
    pub mode: ViewMode,
    /// Windows 98: whether View > as Web Page is ticked (None in 95 and 2000, which had no such
    /// entry).
    pub web: Option<bool>,
    /// What Undo would take back: its entry and the status bar's words (None: no Undo entry).
    pub undo: Option<(String, String)>,
}

/// The menu bar's titles (Windows 95's Help, 98's Favorites and Help, 2000's Favorites, Tools and
/// Help are left out: nothing in them works here yet).
pub fn titles(era: Era) -> &'static [&'static str] {
    match era {
        Era::Win98 => &["&File", "&Edit", "&View", "&Go"],
        // 2000's Tools for its Folder Options (Map Network Drive and Synchronize left out).
        Era::Win2000 => &["&File", "&Edit", "&View", "&Tools"],
        _ => &["&File", "&Edit", "&View"],
    }
}

/// The status bar's text while a menu bar title is selected (95: explorer.exe's strings 800 to
/// 928; 98 and 2000: their status bars).
pub fn title_help(era: Era, title: usize) -> &'static str {
    let help: &[&str] = match era {
        Era::Win95 => &[
            "Contains commands for working with the selected items.",
            "Contains commands for selecting and transferring items.",
            "Contains commands for customizing this window.",
        ],
        Era::Win98 => &[
            "Contains commands for working with the selected items.",
            "Contains Edit commands",
            "Contains commands for manipulating the view",
            "Contains commands for browsing to various pages.",
        ],
        Era::Win2000 => &[
            "Contains commands for working with the selected items.",
            "Contains edit commands.",
            "Contains commands for manipulating the view.",
        ],
    };
    help.get(title).copied().unwrap_or("")
}

use DesktopCommand as D;

/// Whether every selected icon's menu holds `c` (as the desktop's menu on icons decides).
fn offered(s: &State, c: DesktopCommand) -> bool {
    desktop::item_entries(&s.kinds).contains(&Some(c))
}

/// The rows of menu bar title `title` (`titles`).
pub fn bar_rows(s: &State, title: usize) -> Vec<Row> {
    match title {
        0 => file_rows(s),
        1 => edit_rows(s),
        2 => view_rows(s),
        _ if s.era == Era::Win2000 => vec![Row::item("Folder &Options...", Command::FolderOptions, "")],
        _ => go_rows(s),
    }
}

fn file_rows(s: &State) -> Vec<Row> {
    let old = s.era == Era::Win95;
    let mut rows = Vec::new();
    // The selection's own commands first, Open in bold (the default).
    if offered(s, D::Open) {
        let mut open = Row::item("&Open", Command::Shared(D::Open), if old { "Open the selected items." } else { "Open" });
        open.entry.bold = true;
        rows.push(open);
    }
    if offered(s, D::Explore) {
        rows.push(Row::item("&Explore", Command::Shared(D::Explore), if old { "" } else { "Explore" }));
    }
    if !rows.is_empty() {
        rows.push(Row::separator());
    }
    if s.files {
        let new_help = if s.era == Era::Win2000 { "Creates a new, empty folder." } else { "Contains commands for creating new items." };
        rows.push(Row::sub(if old { "Ne&w" } else { "&New" }, Sub::New, new_help));
        rows.push(Row::separator());
    }
    rows.push(Row::item("Create &Shortcut", Command::Shared(D::CreateShortcut), "Creates shortcuts to the selected items.").grayed(!offered(s, D::CreateShortcut)));
    rows.push(Row::item("&Delete", Command::Shared(D::Delete), "Deletes the selected items.").grayed(!offered(s, D::Delete)));
    rows.push(Row::item("Rena&me", Command::Shared(D::Rename), "Renames the selected item.").grayed(!offered(s, D::Rename)));
    rows.push(
        Row::item("P&roperties", Command::Shared(D::ItemProperties), "Displays the properties of the selected items.")
            .grayed(!offered(s, D::ItemProperties)),
    );
    rows.push(Row::separator());
    if s.era == Era::Win98 && !s.nav.chain.is_empty() {
        let last = s.nav.chain.len() - 1;
        for (i, name) in s.nav.chain.iter().enumerate().take(u8::MAX as usize) {
            rows.push(Row::item(&crate::text::escape_access_keys(name), Command::Chain(i as u8), "").ticked(i == last));
        }
        rows.push(Row::separator());
    }
    rows.push(Row::item("&Close", Command::Close, "Closes the window."));
    rows
}

fn edit_rows(s: &State) -> Vec<Row> {
    let paste = s.files && s.can_paste;
    let copy_help = if s.era == Era::Win95 {
        "Copies the selected items to the Clipboard. Use Paste to put them in the new location."
    } else {
        "Copies the selected items to the Clipboard. To put them in the new location, use the Paste command."
    };
    // Undo first, then a line (the Reference PCs): grayed while there is nothing to take back.
    let undo = match &s.undo {
        Some((label, help)) => Row::item(label, Command::Undo, help),
        None => Row::item(crate::drag::empty_undo_label(s.era), Command::Undo, "Undoes the previous file operation.").grayed(true),
    };
    vec![
        undo,
        Row::separator(),
        Row::item("Cu&t\tCtrl+X",Command::Shared(D::Cut), "Removes the selected items and copies them onto the Clipboard.").grayed(!offered(s, D::Cut)),
        Row::item("&Copy\tCtrl+C", Command::Shared(D::Copy), copy_help).grayed(!offered(s, D::Copy)),
        Row::item("&Paste\tCtrl+V", Command::Shared(D::Paste), "Inserts the items you have copied or cut into the selected location.").grayed(!paste),
        Row::item("Paste &Shortcut", Command::Shared(D::PasteShortcut), "Creates shortcuts to the items you have copied or cut into the selected location.")
            .grayed(!paste),
        Row::separator(),
        Row::item("Select &All\tCtrl+A", Command::SelectAll, "Selects all items in the window."),
        Row::item("&Invert Selection", Command::InvertSelection, "Reverses which items are selected and which are not."),
    ]
}

fn view_rows(s: &State) -> Vec<Row> {
    let status_label = if s.era == Era::Win98 { "&Status Bar" } else { "Status &Bar" };
    let mut status = Row::item(status_label, Command::StatusBar, "Shows or hides the status bar.");
    status.entry.mark = s.status_bar.then_some(Mark::Check);
    let mut rows = vec![status];
    if s.tree.is_some() {
        rows.push(Row::sub("E&xplorer Bar", Sub::ExplorerBar, ""));
    }
    // 98: as Web Page between the bars and the views (win98-truecolor-web-menu-view).
    if let Some(on) = s.web {
        rows.push(Row::separator());
        rows.push(Row::item("as &Web Page", Command::WebView, "Displays items in Web View").ticked(on));
    }
    rows.push(Row::separator());
    rows.extend(view_choices(s));
    rows.push(Row::separator());
    if s.files {
        rows.push(Row::sub("Arrange &Icons", Sub::Arrange, "Contains commands for arranging items in the window."));
        rows.push(Row::separator());
    }
    let refresh = match s.era {
        Era::Win95 => "Refresh item information.",
        Era::Win98 => "Refresh the contents of the current page.",
        Era::Win2000 => "Refreshes the contents of the current page.",
    };
    if s.era == Era::Win2000 {
        rows.push(Row::sub("G&o To", Sub::GoTo, "Contains commands for browsing to various pages."));
    }
    rows.push(Row::item("&Refresh", Command::Shared(D::Refresh), refresh));
    rows
}

/// The four views with the dot on the one shown (the View menu, the empty area's View, the Views
/// button's menu; their help texts read off each Reference PC's status bar: `seq/<era>/*-view-menu`).
pub fn view_choices(s: &State) -> Vec<Row> {
    [
        ("Lar&ge Icons", ViewMode::Large, "Displays items by using large icons."),
        ("S&mall Icons", ViewMode::Small, "Displays items by using small icons."),
        ("&List", ViewMode::List, "Displays items in a list."),
        ("&Details", ViewMode::Details, "Displays information about each item in the window."),
    ]
    .into_iter()
    .map(|(label, mode, help)| {
        let mut r = Row::item(label, Command::View(mode), help);
        r.entry.mark = (s.mode == mode).then_some(Mark::Bullet);
        r
    })
    .collect()
}

/// Windows 98's Go menu and 2000's View > Go To: Back, Forward, Up One Level, My Computer.
fn go_rows(s: &State) -> Vec<Row> {
    let (back, forward) = match s.era {
        Era::Win2000 => ("Goes to the previous page.", "Goes to the next page."),
        _ => ("Goes back one step.", "Goes forward one step."),
    };
    vec![
        Row::item("&Back\tAlt+Left Arrow", Command::Back, back).grayed(!s.nav.back),
        Row::item("&Forward\tAlt+Right Arrow", Command::Forward, forward).grayed(!s.nav.forward),
        Row::item("&Up One Level", Command::Up, "Goes up one level.").grayed(!s.nav.up),
        Row::separator(),
        Row::item("My &Computer", Command::GoMyComputer, "").ticked(s.era == Era::Win2000 && s.nav.at_my_computer),
    ]
}

/// The menu of the window's empty area: Arrange Icons (a folder of files), (98 and 2000: Refresh,)
/// Paste and Paste Shortcut (grayed in My Computer, as Windows had them), New (a folder of files),
/// Properties.
/// The status bar's words for an entry of the menu on the selected icons, while it is lit (the
/// Reference PCs, `private/probes/measure17/<era>-notes.txt`, G: the shell's own help strings; Open
/// and Explore show the verb's name).
pub fn item_help(era: Era, c: D) -> &'static str {
    match c {
        D::Open => "Open",
        D::Explore => "Explore",
        D::Cut => "Removes the selected items and copies them onto the Clipboard.",
        D::Copy if era == Era::Win95 => "Copies the selected items to the Clipboard. Use Paste to put them in the new location.",
        D::Copy => "Copies the selected items to the Clipboard. To put them in the new location, use the Paste command.",
        D::Paste => "Inserts the items you have copied or cut into the selected location.",
        D::CreateShortcut => "Creates shortcuts to the selected items.",
        D::Delete => "Deletes the selected items.",
        D::Rename => "Renames the selected item.",
        D::ItemProperties => "Displays the properties of the selected items.",
        _ => "",
    }
}

pub fn background_rows(s: &State) -> Vec<Row> {
    let paste = s.files && s.can_paste;
    // The View submenu first, a line under it (`*-view-v1-details-rmenu`, `-folder-mc-rmenu-empty`).
    // The status bar's words (measured, `private/probes/measure17`, G): 95 names what each submenu
    // holds; 98 and 2000 show nothing for the submenus and Refresh.
    let sub_help = |words: &'static str| if s.era == Era::Win95 { words } else { "" };
    let mut rows = vec![Row::sub("&View", Sub::Views, sub_help("Contains commands for customizing this window.")), Row::separator()];
    if s.files {
        rows.push(Row::sub("Arrange &Icons", Sub::Arrange, sub_help("Contains commands for arranging items in the window.")));
        if s.era == Era::Win98 {
            rows.push(Row::separator());
        }
    }
    if s.era != Era::Win95 {
        rows.push(Row::item("R&efresh", Command::Shared(D::Refresh), ""));
    }
    if rows.last().is_some_and(|r| r.entry.kind != EntryKind::Separator) {
        rows.push(Row::separator());
    }
    rows.push(Row::item("&Paste", Command::Shared(D::Paste), "Inserts the items you have copied or cut into the selected location.").grayed(!paste));
    rows.push(
        Row::item("Paste &Shortcut", Command::Shared(D::PasteShortcut), "Creates shortcuts to the items you have copied or cut into the selected location.")
            .grayed(!paste),
    );
    // What Undo would take back, after Paste Shortcut (the Reference PCs; only while there is one).
    if let Some((label, help)) = &s.undo {
        rows.push(Row::item(label, Command::Undo, help));
    }
    rows.push(Row::separator());
    if s.files {
        rows.push(Row::sub("Ne&w", Sub::New, sub_help("Contains commands for creating new items.")));
        rows.push(Row::separator());
    }
    rows.push(Row::item("P&roperties", Command::FolderProperties, "Displays properties for this folder."));
    rows
}

/// A submenu's rows: New (Folder, Shortcut, Text Document: the desktop's), Arrange Icons (by Name,
/// Type, Size, Date, Auto Arrange: the desktop's, ticked while it is on), 2000's Go To.
pub fn sub_rows(s: &State, sub: Sub) -> Vec<Row> {
    if sub == Sub::GoTo {
        return go_rows(s);
    }
    if sub == Sub::ExplorerBar {
        return explorer_bar_rows(s);
    }
    if sub == Sub::Views {
        // 98's Views button and the empty area's View begin with as Web Page (-views-arrow-menu).
        let mut rows = Vec::new();
        if let Some(on) = s.web {
            rows.push(Row::item("as &Web Page", Command::WebView, "Displays items in Web View").ticked(on));
            rows.push(Row::separator());
        }
        rows.extend(view_choices(s));
        return rows;
    }
    let menu = match sub {
        Sub::New => desktop::new_menu(),
        _ => desktop::arrange_menu(s.auto_arrange),
    };
    menu.entries
        .into_iter()
        .enumerate()
        .map(|(i, entry)| {
            let command = match sub {
                Sub::New => desktop::new_at(i),
                _ => desktop::arrange_at(i),
            };
            let help = match command {
                Some(D::NewFolder) => "Creates a new, empty folder.",
                Some(D::NewShortcut) => "Creates a new, empty shortcut.",
                Some(D::Arrange(Arrange::Name)) => "Sorts items alphabetically by name.",
                Some(D::Arrange(Arrange::Type)) => "Sorts items by type.",
                Some(D::Arrange(Arrange::Size)) => "Sorts items by size, from smallest to largest.",
                Some(D::Arrange(Arrange::Date)) => "Sorts items by date, from oldest to most recent.",
                Some(D::AutoArrange) => "Arranges the icons automatically.",
                _ => "",
            };
            Row { entry, command: command.map(Command::Shared), sub: None, help: help.to_string() }
        })
        .collect()
}

/// View > Explorer Bar, with only what works here: 98's All Folders and None (the one in effect
/// with a dot, `win98-truecolor-tree-explorer-bar-menu`), 2000's Folders (Search, Favorites,
/// History, Channels and Tip of the Day are left out).
fn explorer_bar_rows(s: &State) -> Vec<Row> {
    let shown = s.tree.unwrap_or(false);
    let dot = |r: Row, on: bool| {
        let mut r = r;
        r.entry.mark = on.then_some(Mark::Bullet);
        r
    };
    if s.era == Era::Win2000 {
        return vec![Row::item("F&olders", Command::ToggleTree, "").ticked(shown)];
    }
    vec![
        dot(Row::item("&All Folders", Command::TreeShown(true), ""), shown),
        Row::separator(),
        dot(Row::item("&None", Command::TreeShown(false), ""), !shown),
    ]
}

/// The rows as a menu to show (nothing highlighted; Windows 95 always underlined access keys,
/// 98's Explorer too; 2000 only after a key).
pub fn menu(rows: &[Row]) -> Menu {
    let mut menu = Menu::new(rows.iter().map(|r| r.entry.clone()).collect());
    menu.explorer_bold = true;
    menu.show_keys = true;
    menu
}

/// Whether row `i` opens a submenu.
pub fn opens(rows: &[Row], i: usize) -> Option<Sub> {
    rows.get(i).filter(|r| r.entry.kind == EntryKind::Submenu && !r.entry.disabled).and_then(|r| r.sub)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A command that does not use the selection runs although a selected item went while its
    /// menu was open (the fourth safety review of 2026-11-18, C4: Details, Refresh, Up, Close and
    /// About were dropped); one that acts on the selection still does not.
    #[test]
    fn only_a_command_on_the_selection_needs_the_selection_it_was_opened_on() {
        let gone = || false;
        let others = [
            Command::View(ViewMode::Details),
            Command::Shared(DesktopCommand::Refresh),
            Command::Shared(DesktopCommand::Paste),
            Command::Shared(DesktopCommand::NewFolder),
            Command::Shared(DesktopCommand::Arrange(Arrange::Name)),
            Command::Up,
            Command::Back,
            Command::Close,
            Command::SelectAll,
            Command::StatusBar,
            Command::FolderProperties,
            Command::Undo,
            Command::GoMyComputer,
        ];
        for c in others {
            assert!(c.runs_after_menu(gone), "{c:?}");
        }
        for c in [DesktopCommand::Open, DesktopCommand::Delete, DesktopCommand::Rename, DesktopCommand::Cut, DesktopCommand::ItemProperties] {
            assert!(!Command::Shared(c).runs_after_menu(gone), "{c:?}");
            assert!(Command::Shared(c).runs_after_menu(|| true), "{c:?}");
        }
        // Every row of every menu of every look is one or the other, and none of the menu bar's
        // View, Go or Help rows waits for the selection.
        for era in Era::ALL {
            let state = State { era, ..state(&[ItemKind::File], true) };
            for title in 0..titles(era).len() {
                for row in bar_rows(&state, title) {
                    if let Some(c) = row.command
                        && matches!(c, Command::View(_) | Command::Up | Command::Back | Command::Forward | Command::GoMyComputer)
                    {
                        assert!(!c.acts_on_selection(), "{era:?} {c:?}");
                    }
                }
            }
        }
    }

    fn labels(rows: &[Row]) -> Vec<String> {
        rows.iter().map(|r| if r.entry.kind == EntryKind::Separator { "-".into() } else { r.entry.label.clone() }).collect()
    }

    fn state(kinds: &[ItemKind], files: bool) -> State {
        State { era: Era::Win95, kinds: kinds.to_vec(), files, can_paste: false, status_bar: true, auto_arrange: false, nav: Nav::default(), tree: None, mode: ViewMode::Large, web: None, undo: None }
    }

    #[test]
    fn the_file_menu_follows_the_selection_as_on_the_reference_pc() {
        // My Computer, nothing selected (-mc-menu-file): the selection's entries grayed, Close.
        let rows = file_rows(&state(&[], false));
        assert_eq!(labels(&rows), ["Create &Shortcut", "&Delete", "Rena&me", "P&roperties", "-", "&Close"]);
        assert!(rows[..4].iter().all(|r| r.entry.disabled));
        // A drive selected (-mc-menu-file-drive): Open (bold), Explore; Delete and Rename grayed.
        let rows = file_rows(&state(&[ItemKind::Drive], false));
        assert_eq!(labels(&rows), ["&Open", "&Explore", "-", "Create &Shortcut", "&Delete", "Rena&me", "P&roperties", "-", "&Close"]);
        assert!(rows[0].entry.bold);
        assert!(rows[4].entry.disabled && rows[5].entry.disabled && !rows[6].entry.disabled);
        // A folder in a folder of files (-c-menu-file-folder): New after the selection's entries.
        let rows = file_rows(&state(&[ItemKind::Folder], true));
        assert_eq!(labels(&rows)[..5], ["&Open", "&Explore", "-", "Ne&w", "-"]);
        // A file (-c-menu-file-file): no Explore.
        let rows = file_rows(&state(&[ItemKind::File], true));
        assert_eq!(labels(&rows)[..4], ["&Open", "-", "Ne&w", "-"]);
        // Nothing selected in a folder of files (-c-menu-file-none): New first.
        assert_eq!(labels(&file_rows(&state(&[], true)))[0], "Ne&w");
        assert_eq!(rows.last().unwrap().help, "Closes the window.");
    }

    #[test]
    fn edit_view_and_the_empty_area_hold_what_works() {
        let s = State { can_paste: true, ..state(&[ItemKind::File], true) };
        let edit = edit_rows(&s);
        assert_eq!(
            labels(&edit),
            ["&Undo\tCtrl+Z", "-", "Cu&t\tCtrl+X", "&Copy\tCtrl+C", "&Paste\tCtrl+V", "Paste &Shortcut", "-", "Select &All\tCtrl+A", "&Invert Selection"]
        );
        assert!(edit[0].entry.disabled, "nothing to undo: grayed");
        assert!(edit[2..6].iter().all(|r| !r.entry.disabled));
        // My Computer: nothing to cut, nothing pasted into it.
        let edit = edit_rows(&State { can_paste: true, ..state(&[ItemKind::Drive], false) });
        assert!(edit[2..6].iter().all(|r| r.entry.disabled));
        // Something to take back: its word, enabled, its status bar text; also on the empty area.
        let step = crate::drag::Step::Move(vec![("C:\\B\\a3".into(), "C:\\A\\a3".into())]);
        let s2 = State { era: Era::Win2000, undo: Some((step.label(Era::Win2000), step.help())), ..s.clone() };
        let edit = edit_rows(&s2);
        assert_eq!(edit[0].entry.label, "&Undo Move\tCtrl+Z");
        assert!(!edit[0].entry.disabled);
        assert_eq!(edit[0].help, "Undo Move of 'a3'");
        assert!(background_rows(&s2).iter().any(|r| r.command == Some(Command::Undo)));
        assert!(!background_rows(&s).iter().any(|r| r.command == Some(Command::Undo)));
        let view = view_rows(&s);
        const VIEWS: [&str; 4] = ["Lar&ge Icons", "S&mall Icons", "&List", "&Details"];
        assert_eq!(labels(&view), [&["Status &Bar", "-"][..], &VIEWS, &["-", "Arrange &Icons", "-", "&Refresh"]].concat());
        assert_eq!(view[0].entry.mark, Some(Mark::Check));
        assert_eq!(view[2].entry.mark, Some(Mark::Bullet));
        assert_eq!(view[3].entry.mark, None);
        assert_eq!(view[4].help, "Displays items in a list.");
        let details = view_rows(&State { mode: ViewMode::Details, ..s.clone() });
        assert_eq!(details[5].entry.mark, Some(Mark::Bullet), "the dot on the view shown");
        assert_eq!(details[5].command, Some(Command::View(ViewMode::Details)));
        assert_eq!(labels(&view_rows(&state(&[], false))), [&["Status &Bar", "-"][..], &VIEWS, &["-", "&Refresh"]].concat());
        // The empty area's menu: View first (95: `*-view-v1-details-rmenu`).
        let back = background_rows(&state(&[], true));
        assert_eq!(labels(&back), ["&View", "-", "Arrange &Icons", "-", "&Paste", "Paste &Shortcut", "-", "Ne&w", "-", "P&roperties"]);
        assert!(back[4].entry.disabled, "nothing on the clipboard");
        assert_eq!(labels(&sub_rows(&s, Sub::Views)), VIEWS);
        let back = background_rows(&state(&[], false));
        assert_eq!(labels(&back), ["&View", "-", "&Paste", "Paste &Shortcut", "-", "P&roperties"], "My Computer (-folder-mc-rmenu-empty)");
        assert_eq!(opens(&background_rows(&state(&[], true)), 0), Some(Sub::Views));
        assert_eq!(opens(&background_rows(&state(&[], true)), 2), Some(Sub::Arrange));
        let new = sub_rows(&s, Sub::New);
        assert_eq!(new[0].command, Some(Command::Shared(D::NewFolder)));
        assert_eq!(new[0].help, "Creates a new, empty folder.");
        assert_eq!(sub_rows(&s, Sub::Arrange)[5].command, Some(Command::Shared(D::AutoArrange)));
    }

    #[test]
    fn windows_98_has_a_go_menu_and_lists_the_way_down() {
        let nav = Nav { back: true, forward: false, up: true, chain: vec!["My Computer".into(), "(C:)".into(), "Windows".into()], at_my_computer: false };
        let s = State { era: Era::Win98, nav, ..state(&[], true) };
        assert_eq!(titles(Era::Win98), ["&File", "&Edit", "&View", "&Go"]);
        assert_eq!(title_help(Era::Win98, 3), "Contains commands for browsing to various pages.");
        // `windows-menu-file-none`: New, the selection's entries, the places (Windows ticked), Close.
        let file = file_rows(&s);
        assert_eq!(
            labels(&file),
            ["&New", "-", "Create &Shortcut", "&Delete", "Rena&me", "P&roperties", "-", "My Computer", "(C:)", "Windows", "-", "&Close"]
        );
        assert_eq!(file[9].entry.mark, Some(Mark::Check));
        assert_eq!(file[8].command, Some(Command::Chain(1)));
        let go = go_rows(&s);
        assert_eq!(labels(&go), ["&Back\tAlt+Left Arrow", "&Forward\tAlt+Right Arrow", "&Up One Level", "-", "My &Computer"]);
        assert!(!go[0].entry.disabled && go[1].entry.disabled);
        assert_eq!(labels(&background_rows(&s)), ["&View", "-", "Arrange &Icons", "-", "R&efresh", "-", "&Paste", "Paste &Shortcut", "-", "Ne&w", "-", "P&roperties"]);
    }

    #[test]
    fn windows_2000_keeps_go_to_under_view() {
        let s = State { era: Era::Win2000, nav: Nav { at_my_computer: true, ..Nav::default() }, ..state(&[], false) };
        assert_eq!(titles(Era::Win2000), ["&File", "&Edit", "&View", "&Tools"]);
        assert_eq!(bar_rows(&s, 3)[0].command, Some(Command::FolderOptions));
        let view = view_rows(&s);
        assert_eq!(labels(&view), ["Status &Bar", "-", "Lar&ge Icons", "S&mall Icons", "&List", "&Details", "-", "G&o To", "&Refresh"]);
        assert_eq!(opens(&view, 7), Some(Sub::GoTo));
        let go = sub_rows(&s, Sub::GoTo);
        assert_eq!(go[4].entry.mark, Some(Mark::Check), "My Computer ticked while shown");
        assert!(go[0].entry.disabled && go[2].entry.disabled);
        assert_eq!(labels(&file_rows(&s)), ["Create &Shortcut", "&Delete", "Rena&me", "P&roperties", "-", "&Close"], "no list of places");
    }
}
