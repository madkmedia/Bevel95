//! Bevel95's desktop: the desktop in the color each Windows first showed (the theme's
//! `Colors::desktop`), which the platform layer shows over Windows' own desktop, with its icons
//! (`desktop_icons`). This module holds its right-click menu.
//!
//! Each look's own desktop menu (the Reference PCs, `private/reference/refs/vm/<era>-truecolor-desk-menu-*`),
//! limited to what works: Arrange Icons (by Name, Type, Size, Date, and Auto Arrange, ticked
//! while it is on), Line up Icons, Refresh (98, 2000), New (Folder, Shortcut, Text Document) and
//! Properties, and Paste and Paste Shortcut (grayed while the clipboard holds no files, as in
//! Windows). Windows' entries that do nothing here are left out, not grayed: Active Desktop. Undo (after Paste Shortcut) shows only while there is something to take back.
//! After a separator, Bevel95's one entry, "Bevel95", whose submenu holds its own (`own_menu`).
//!
//! And the menu on an icon (`item_menu`), each look's own (the Reference PCs,
//! `private/reference/refs/vm/<era>-truecolor-item-menu-*`), limited to what works here: Open (bold, the
//! default), Explore for folders and the look's own icons, Empty Recycle Bin, Cut, Copy, Create
//! Shortcut, Delete, Rename, Properties. Left out, not grayed: Print, Open With, Quick View, Find
//! (Search), Manage, Map Network Drive, Sharing, Send To, Remove from Desktop, entries other
//! programs add. The look's own icons are never cut, copied, deleted or renamed.

use crate::desktop_art::Art;
use crate::desktop_icons::{Arrange, Item, Picture, System};
use crate::paint::{Mark, Menu, MenuEntry};
use crate::theme::Era;

/// What an entry of the desktop's menus does. The platform layer carries it out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DesktopCommand {
    /// Order the owner's icons again.
    Arrange(Arrange),
    /// Auto Arrange on or off.
    AutoArrange,
    /// Every icon onto the nearest cell of the grid.
    LineUp,
    /// Read the desktop's items again (98, 2000).
    Refresh,
    /// Make a new item in the owner's Desktop folder, with Windows' own default name.
    NewFolder,
    /// Windows' Create Shortcut wizard, making the shortcut in the Desktop folder.
    NewShortcut,
    NewTextDocument,
    /// Windows' own Display settings (`ms-settings:display`), where Windows 95 to XP had their
    /// Display Properties. Bevel95 Settings' Desktop page is the Bevel95 entry's `Settings`.
    Properties,
    /// Take Bevel95's desktop away, so that Windows' own shows again. It stays away until
    /// Bevel95 Settings brings it back.
    SwitchToWindowsDesktop,
    /// Open Bevel95 Settings on its Desktop page.
    Settings,
    /// The clipboard's files into the owner's Desktop folder (copied, or moved after Cut).
    Paste,
    /// A shortcut on the desktop to each file on the clipboard.
    PasteShortcut,
    // The menu on an icon: each acts on every selected icon.
    /// The default command, as a double-click.
    Open,
    /// A folder (or the look's own icon) in Explorer's two-pane window.
    Explore,
    /// Onto the clipboard, to be moved by the next Paste.
    Cut,
    Copy,
    /// "Shortcut to <name>" on the desktop.
    CreateShortcut,
    /// To the Recycle Bin, after the look's own question.
    Delete,
    /// The label becomes a box to type the new name in.
    Rename,
    /// Windows' own Properties window of the item.
    ItemProperties,
    /// Every item in the Recycle Bin deleted for good, after the look's own question.
    EmptyRecycleBin,
    /// The last move, copy, rename or delete taken back (shown only while there is one).
    Undo,
    /// Open Quick Search (the Bevel95 submenu).
    QuickSearch,
}

impl DesktopCommand {
    const ALL: [DesktopCommand; 26] = [
        DesktopCommand::Arrange(Arrange::Name),
        DesktopCommand::Arrange(Arrange::Type),
        DesktopCommand::Arrange(Arrange::Size),
        DesktopCommand::Arrange(Arrange::Date),
        DesktopCommand::AutoArrange,
        DesktopCommand::LineUp,
        DesktopCommand::Refresh,
        DesktopCommand::NewFolder,
        DesktopCommand::NewShortcut,
        DesktopCommand::NewTextDocument,
        DesktopCommand::Properties,
        DesktopCommand::SwitchToWindowsDesktop,
        DesktopCommand::Settings,
        DesktopCommand::Paste,
        DesktopCommand::PasteShortcut,
        DesktopCommand::Open,
        DesktopCommand::Explore,
        DesktopCommand::Cut,
        DesktopCommand::Copy,
        DesktopCommand::CreateShortcut,
        DesktopCommand::Delete,
        DesktopCommand::Rename,
        DesktopCommand::ItemProperties,
        DesktopCommand::EmptyRecycleBin,
        DesktopCommand::Undo,
        DesktopCommand::QuickSearch,
    ];

    /// A number standing for the command in a window message (`from_code` reads it back).
    pub fn code(self) -> usize {
        DesktopCommand::ALL.iter().position(|&c| c == self).unwrap_or(0)
    }

    pub fn from_code(code: usize) -> Option<DesktopCommand> {
        DesktopCommand::ALL.get(code).copied()
    }
}

/// An entry of the desktop's own menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    /// Opens the Arrange Icons submenu.
    ArrangeIcons,
    /// Opens the New submenu.
    New,
    /// Opens the Bevel95 submenu (`own_menu`).
    Bevel95,
    Command(DesktopCommand),
}

/// `None` is a separator.
fn entries(era: Era) -> Vec<Option<(&'static str, Entry)>> {
    use DesktopCommand::*;
    let arrange = Some(("Arrange &Icons", Entry::ArrangeIcons));
    let refresh = Some(("Re&fresh", Entry::Command(Refresh)));
    let properties = Some(("P&roperties", Entry::Command(Properties)));
    // Windows 95 wrote "Line up Icons" with its key on the e (the Reference PC's menu).
    let line_up = Some((if era == Era::Win95 { "Lin&e up Icons" } else { "Line &Up Icons" }, Entry::Command(LineUp)));
    let paste = Some(("&Paste", Entry::Command(Paste)));
    let paste_shortcut = Some(("Paste &Shortcut", Entry::Command(PasteShortcut)));
    let mut out = match era {
        // Windows 95 had no Refresh; its New has its access key on the w.
        Era::Win95 => vec![arrange, line_up, None, paste, paste_shortcut, None, Some(("Ne&w", Entry::New)), None, properties],
        // Windows 98: Arrange Icons and Line Up Icons, then Refresh after a separator; Windows
        // 2000 the three together.
        Era::Win98 => {
            vec![arrange, line_up, None, refresh, None, paste, paste_shortcut, None, Some(("&New", Entry::New)), None, properties]
        }
        Era::Win2000 => vec![arrange, line_up, refresh, None, paste, paste_shortcut, None, Some(("Ne&w", Entry::New)), None, properties],
    };
    out.extend([None, Some(("Bevel95", Entry::Bevel95))]);
    out
}

/// The Bevel95 submenu (`None` a separator), in the bar's Bevel95 submenu's order (PROVISIONAL:
/// the order and the wording).
const OWN: [Option<(&str, DesktopCommand)>; 4] = [
    Some(("Bevel95 Settings", DesktopCommand::Settings)),
    Some(("Quick Search", DesktopCommand::QuickSearch)),
    None,
    Some(("Switch to Windows desktop", DesktopCommand::SwitchToWindowsDesktop)),
];

/// The desktop menu's Bevel95 submenu, nothing highlighted; `desktop_key`: the bar registered
/// `bar_menu::DESKTOP_KEY`, which then shows beside Switch to Windows desktop.
pub fn own_menu(desktop_key: bool) -> Menu {
    Menu::new(
        OWN.iter()
            .map(|e| match e {
                Some((label, DesktopCommand::SwitchToWindowsDesktop)) if desktop_key => {
                    MenuEntry::item(&format!("{label}\t{}", crate::bar_menu::DESKTOP_KEY), None)
                }
                Some((label, _)) => MenuEntry::item(label, None),
                None => MenuEntry::separator(),
            })
            .collect(),
    )
}

/// What entry `index` of `own_menu` does.
pub fn own_at(index: usize) -> Option<DesktopCommand> {
    OWN.get(index).copied().flatten().map(|(_, c)| c)
}

/// The desktop's right-click menu in `era`, nothing highlighted. Paste and Paste Shortcut are
/// grayed unless `can_paste` (the clipboard holds files).
pub fn menu(era: Era, can_paste: bool) -> Menu {
    let entries = entries(era)
        .into_iter()
        .map(|e| match e {
            Some((label, Entry::ArrangeIcons | Entry::New | Entry::Bevel95)) => MenuEntry::submenu(label, None),
            Some((label, Entry::Command(c))) => {
                let mut entry = MenuEntry::item(label, None);
                entry.disabled = matches!(c, DesktopCommand::Paste | DesktopCommand::PasteShortcut) && !can_paste;
                entry
            }
            None => MenuEntry::separator(),
        })
        .collect();
    Menu::new(entries)
}

/// What kind of icon a menu is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind {
    System(System),
    Folder,
    /// A file or a shortcut (their menus were the same here: Print, Open With and Quick View,
    /// which shortcuts showed for their targets, are left out).
    File,
    /// A drive in My Computer's window (Find and Format are left out).
    Drive,
    /// Control Panel or Printers in My Computer's window.
    Place,
}

impl ItemKind {
    pub fn of(item: &Item) -> ItemKind {
        match item.picture {
            Picture::System(s) => ItemKind::System(s),
            Picture::Art(Art::Drive(_)) => ItemKind::Drive,
            Picture::Art(_) => ItemKind::Place,
            _ if item.folder => ItemKind::Folder,
            _ => ItemKind::File,
        }
    }

    /// The commands its menu holds (in any order; `item_menu` puts them in Windows' order).
    pub fn commands(self) -> &'static [DesktopCommand] {
        use DesktopCommand::*;
        match self {
            ItemKind::System(System::RecycleBin) => &[Open, Explore, EmptyRecycleBin, CreateShortcut, ItemProperties],
            ItemKind::System(_) => &[Open, Explore, CreateShortcut, ItemProperties],
            ItemKind::Folder => &[Open, Explore, Cut, Copy, CreateShortcut, Delete, Rename, ItemProperties],
            ItemKind::File => &[Open, Cut, Copy, CreateShortcut, Delete, Rename, ItemProperties],
            ItemKind::Drive => &[Open, Explore, CreateShortcut, ItemProperties],
            ItemKind::Place => &[Open, Explore, CreateShortcut],
        }
    }
}

/// Windows' order of an icon's entries, in groups between separators (the Reference PCs): Open,
/// Explore and Empty Recycle Bin; Cut and Copy; Create Shortcut, Delete and Rename; Properties.
const ITEM_GROUPS: [&[DesktopCommand]; 4] = [
    &[DesktopCommand::Open, DesktopCommand::Explore, DesktopCommand::EmptyRecycleBin],
    &[DesktopCommand::Cut, DesktopCommand::Copy],
    &[DesktopCommand::CreateShortcut, DesktopCommand::Delete, DesktopCommand::Rename],
    &[DesktopCommand::ItemProperties],
];

/// An entry's label in `era`, as the Reference PCs showed it (Windows 2000 moved Explore's
/// access key to the x).
fn item_label(era: Era, command: DesktopCommand) -> &'static str {
    use DesktopCommand::*;
    match command {
        Open => "&Open",
        Explore if era == Era::Win2000 => "E&xplore",
        Explore => "&Explore",
        EmptyRecycleBin => "Empty Recycle &Bin",
        Cut => "Cu&t",
        Copy => "&Copy",
        CreateShortcut => "Create &Shortcut",
        Delete => "&Delete",
        Rename => "Rena&me",
        _ => "P&roperties",
    }
}

/// The commands of the menu on the selected icons, `None` for a separator: the entries every
/// selected icon has (Windows showed only what applies to all of them), without Rename when
/// several are selected.
pub fn item_entries(kinds: &[ItemKind]) -> Vec<Option<DesktopCommand>> {
    let has = |c: DesktopCommand| {
        !kinds.is_empty() && kinds.iter().all(|k| k.commands().contains(&c)) && !(c == DesktopCommand::Rename && kinds.len() > 1)
    };
    let mut out: Vec<Option<DesktopCommand>> = Vec::new();
    for group in ITEM_GROUPS {
        let present: Vec<DesktopCommand> = group.iter().copied().filter(|&c| has(c)).collect();
        if present.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push(None);
        }
        out.extend(present.into_iter().map(Some));
    }
    out
}

/// The menu on the selected icons (`item_entries`) in `era`: Open in bold, Empty Recycle Bin
/// grayed while the Recycle Bin is empty.
pub fn item_menu(era: Era, kinds: &[ItemKind], recycle_full: bool) -> Menu {
    let entries = item_entries(kinds)
        .into_iter()
        .map(|e| match e {
            Some(c) => {
                let mut entry = MenuEntry::item(item_label(era, c), None);
                entry.bold = c == DesktopCommand::Open;
                entry.disabled = c == DesktopCommand::EmptyRecycleBin && !recycle_full;
                entry
            }
            None => MenuEntry::separator(),
        })
        .collect();
    // Windows 2000 underlined a grayed entry's access key with every other pixel (Empty Recycle Bin).
    Menu { explorer_bold: true, dotted_disabled_keys: era == Era::Win2000, ..Menu::new(entries) }
}

/// What entry `index` of `item_menu` does.
pub fn item_at(kinds: &[ItemKind], index: usize) -> Option<DesktopCommand> {
    item_entries(kinds).get(index).copied().flatten()
}

/// A key the desktop knows besides moving the selection (`desktop_icons::Key`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortcutKey {
    Delete,
    F2,
    F10,
    /// The menu key (VK_APPS).
    Menu,
    Enter,
    Letter(char),
}

/// What a key does on the desktop, as on Windows' own desktop: Del deletes, F2 renames, Ctrl+X,
/// Ctrl+C and Ctrl+V cut, copy and paste, Alt+Enter shows the Properties, Shift+F10 and the menu
/// key open the menu (`Some(None)`). Shift+Del deleted for good in Windows; here it goes to the
/// Recycle Bin like Del (Bevel95 never deletes the owner's files for good).
pub fn shortcut(key: ShortcutKey, ctrl: bool, shift: bool, alt: bool) -> Option<Option<DesktopCommand>> {
    use DesktopCommand::*;
    match (key, ctrl, shift, alt) {
        (ShortcutKey::Delete, false, _, false) => Some(Some(Delete)),
        (ShortcutKey::F2, false, false, false) => Some(Some(Rename)),
        (ShortcutKey::Letter('X'), true, false, false) => Some(Some(Cut)),
        (ShortcutKey::Letter('C'), true, false, false) => Some(Some(Copy)),
        (ShortcutKey::Letter('V'), true, false, false) => Some(Some(Paste)),
        (ShortcutKey::Letter('Z'), true, false, false) => Some(Some(Undo)),
        (ShortcutKey::Enter, false, false, true) => Some(Some(ItemProperties)),
        (ShortcutKey::F10, false, true, false) | (ShortcutKey::Menu, false, false, false) => Some(None),
        _ => None,
    }
}

/// What entry `index` of `menu(era)` is (`None` for a separator).
pub fn entry_at(era: Era, index: usize) -> Option<Entry> {
    entries(era).get(index).copied().flatten().map(|(_, e)| e)
}

/// Where Undo goes in the desktop's menu: right after Paste Shortcut (the Reference PCs,
/// `<era>-truecolor-drag-desktop-menu-undo`: "Undo Delete" above New's line).
fn undo_index(era: Era) -> usize {
    entries(era).iter().position(|e| matches!(e, Some((_, Entry::Command(DesktopCommand::PasteShortcut))))).map_or(0, |i| i + 1)
}

/// The desktop's menu with Undo for `undo` (what it would take back) when there is one.
pub fn menu_with_undo(era: Era, can_paste: bool, undo: Option<&crate::drag::Step>) -> Menu {
    let mut m = menu(era, can_paste);
    if let Some(step) = undo {
        m.entries.insert(undo_index(era), MenuEntry::item(&step.label(era), None));
    }
    m
}

/// What entry `index` of `menu_with_undo` is (`undo`: it held Undo).
pub fn entry_at_with_undo(era: Era, undo: bool, index: usize) -> Option<Entry> {
    let at = undo_index(era);
    match index {
        i if undo && i == at => Some(Entry::Command(DesktopCommand::Undo)),
        i if undo && i > at => entry_at(era, i - 1),
        i => entry_at(era, i),
    }
}

/// The Arrange Icons submenu (Windows 98's access keys; Windows 95's and 2000's are the same
/// letters as far as the captures show), then a separator and Auto Arrange.
const ARRANGE: [(&str, Arrange); 4] =
    [("by &Name", Arrange::Name), ("by &Type", Arrange::Type), ("by Si&ze", Arrange::Size), ("by &Date", Arrange::Date)];

/// The Arrange Icons submenu; Auto Arrange has a tick while it is on (all three Windows drew a
/// check mark there: `*-truecolor-move-menu-auto-on`).
pub fn arrange_menu(auto_arrange: bool) -> Menu {
    let mut entries: Vec<MenuEntry> = ARRANGE.iter().map(|(label, _)| MenuEntry::item(label, None)).collect();
    entries.push(MenuEntry::separator());
    let mut auto = MenuEntry::item("&Auto Arrange", None);
    auto.mark = auto_arrange.then_some(Mark::Check);
    entries.push(auto);
    Menu::new(entries)
}

pub fn arrange_at(index: usize) -> Option<DesktopCommand> {
    match index {
        4 => None,
        5 => Some(DesktopCommand::AutoArrange),
        _ => ARRANGE.get(index).map(|&(_, by)| DesktopCommand::Arrange(by)),
    }
}

/// The New submenu: Folder and Shortcut, a separator, then the kinds of file Windows offers
/// (here only Text Document, which has no access key).
const NEW: [Option<(&str, DesktopCommand)>; 4] = [
    Some(("&Folder", DesktopCommand::NewFolder)),
    Some(("&Shortcut", DesktopCommand::NewShortcut)),
    None,
    Some(("Text Document", DesktopCommand::NewTextDocument)),
];

pub fn new_menu() -> Menu {
    Menu::new(
        NEW.iter()
            .map(|e| match e {
                Some((label, _)) => MenuEntry::item(label, None),
                None => MenuEntry::separator(),
            })
            .collect(),
    )
}

pub fn new_at(index: usize) -> Option<DesktopCommand> {
    NEW.get(index).copied().flatten().map(|(_, c)| c)
}

/// Windows' own name for a new item in a folder that already holds `taken` (Windows 11's,
/// since the item is made on today's desktop: "New folder", then "New folder (2)" and so on).
pub fn new_name(base: &str, extension: &str, taken: impl Fn(&str) -> bool) -> String {
    let mut name = format!("{base}{extension}");
    let mut n = 2;
    while taken(&name) {
        name = format!("{base} ({n}){extension}");
        n += 1;
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::EntryKind;

    #[test]
    fn entries_and_commands_line_up() {
        for era in Era::ALL {
            let m = menu(era, false);
            let n = m.entries.len();
            for (i, e) in m.entries.iter().enumerate() {
                assert_eq!(e.kind == EntryKind::Separator, entry_at(era, i).is_none(), "{era:?} entry {i}");
                let submenu = matches!(entry_at(era, i), Some(Entry::ArrangeIcons | Entry::New | Entry::Bevel95));
                assert_eq!(e.kind == EntryKind::Submenu, submenu, "{era:?} entry {i}");
            }
            assert_eq!(m.entries[0].label, "Arrange &Icons");
            // The look's own entries end with Properties; then Bevel95's one entry.
            assert_eq!(entry_at(era, n - 3), Some(Entry::Command(DesktopCommand::Properties)));
            assert_eq!(m.entries[n - 2].kind, EntryKind::Separator);
            assert_eq!((m.entries[n - 1].label.as_str(), entry_at(era, n - 1)), ("Bevel95", Some(Entry::Bevel95)));
            let ours = [DesktopCommand::SwitchToWindowsDesktop, DesktopCommand::Settings, DesktopCommand::QuickSearch];
            assert!((0..n).all(|i| !matches!(entry_at(era, i), Some(Entry::Command(c)) if ours.contains(&c))));
            let refresh = m.entries.iter().any(|e| e.label == "Re&fresh");
            assert_eq!(refresh, era != Era::Win95, "Refresh came with Windows 98");
            assert_eq!(entry_at(era, 99), None);
        }
        assert_eq!(arrange_menu(false).entries.len(), 6);
        assert_eq!(arrange_menu(false).entries[5].mark, None);
        assert_eq!(arrange_menu(true).entries[5].mark, Some(Mark::Check));
        assert_eq!(arrange_at(2), Some(DesktopCommand::Arrange(Arrange::Size)));
        assert_eq!(arrange_at(4), None);
        assert_eq!(arrange_at(5), Some(DesktopCommand::AutoArrange));
        for era in Era::ALL {
            let line_up = (0..12).find(|&i| entry_at(era, i) == Some(Entry::Command(DesktopCommand::LineUp)));
            assert_eq!(line_up, Some(1), "{era:?}: right below Arrange Icons");
        }
        assert_eq!(menu(Era::Win95, false).entries[1].label, "Lin&e up Icons");
        assert_eq!(new_at(2), None);
        assert_eq!(new_at(3), Some(DesktopCommand::NewTextDocument));
    }

    #[test]
    fn the_bevel95_submenu_holds_our_own() {
        let m = own_menu(false);
        let labels: Vec<&str> = m.entries.iter().map(|e| e.label.as_str()).collect();
        assert_eq!(labels, ["Bevel95 Settings", "Quick Search", "", "Switch to Windows desktop"]);
        // With the bar's key registered, it shows beside the entry, as in the bar's submenu.
        assert_eq!(own_menu(true).entries[3].label, "Switch to Windows desktop\tCtrl+Win+W");
        assert_eq!(m.entries[2].kind, EntryKind::Separator);
        assert_eq!(own_at(0), Some(DesktopCommand::Settings));
        assert_eq!(own_at(1), Some(DesktopCommand::QuickSearch));
        assert_eq!(own_at(2), None);
        assert_eq!(own_at(3), Some(DesktopCommand::SwitchToWindowsDesktop));
        assert_eq!(own_at(4), None);
        // With Undo in the menu, Bevel95's entry is still the last.
        for era in Era::ALL {
            let n = menu(era, false).entries.len() + 1;
            assert_eq!(entry_at_with_undo(era, true, n - 1), Some(Entry::Bevel95));
        }
        // Codes the bar reads back stay what they were; Quick Search's is new at the end.
        assert_eq!(DesktopCommand::Undo.code(), 24);
        assert_eq!(DesktopCommand::from_code(25), Some(DesktopCommand::QuickSearch));
    }

    #[test]
    fn paste_is_grayed_until_the_clipboard_holds_files() {
        for era in Era::ALL {
            for can in [false, true] {
                let m = menu(era, can);
                let paste: Vec<usize> = (0..m.entries.len())
                    .filter(|&i| matches!(entry_at(era, i), Some(Entry::Command(DesktopCommand::Paste | DesktopCommand::PasteShortcut))))
                    .collect();
                assert_eq!(paste.len(), 2, "{era:?}");
                assert_eq!(paste[1], paste[0] + 1, "Paste Shortcut right below Paste");
                for i in paste {
                    assert_eq!(m.entries[i].disabled, !can);
                }
                // Between separators, right above New (the Reference PCs' menus).
                let new = (0..m.entries.len()).find(|&i| entry_at(era, i) == Some(Entry::New)).unwrap();
                assert_eq!(entry_at(era, new - 1), None);
                assert!(matches!(entry_at(era, new - 2), Some(Entry::Command(DesktopCommand::PasteShortcut))));
            }
        }
    }

    fn labels(era: Era, kinds: &[ItemKind]) -> Vec<String> {
        item_menu(era, kinds, true).entries.iter().map(|e| if e.label.is_empty() { "-".to_string() } else { e.label.clone() }).collect()
    }

    #[test]
    fn each_kind_of_icon_has_windows_entries_that_work_here() {
        let file = labels(Era::Win98, &[ItemKind::File]);
        assert_eq!(file, ["&Open", "-", "Cu&t", "&Copy", "-", "Create &Shortcut", "&Delete", "Rena&me", "-", "P&roperties"]);
        let folder = labels(Era::Win98, &[ItemKind::Folder]);
        assert_eq!(folder, ["&Open", "&Explore", "-", "Cu&t", "&Copy", "-", "Create &Shortcut", "&Delete", "Rena&me", "-", "P&roperties"]);
        assert_eq!(labels(Era::Win2000, &[ItemKind::Folder])[1], "E&xplore");
        // The look's own icons: never cut, copied, deleted or renamed.
        for era in Era::ALL {
            for s in [System::MyComputer, System::MyDocuments] {
                assert_eq!(labels(era, &[ItemKind::System(s)]).len(), 6);
                assert_eq!(item_entries(&[ItemKind::System(s)]), [
                    Some(DesktopCommand::Open),
                    Some(DesktopCommand::Explore),
                    None,
                    Some(DesktopCommand::CreateShortcut),
                    None,
                    Some(DesktopCommand::ItemProperties)
                ]);
            }
        }
        let bin = [ItemKind::System(System::RecycleBin)];
        assert_eq!(labels(Era::Win95, &bin), ["&Open", "&Explore", "Empty Recycle &Bin", "-", "Create &Shortcut", "-", "P&roperties"]);
        assert!(item_menu(Era::Win95, &bin, false).entries[2].disabled, "grayed while the Recycle Bin is empty");
        assert!(!item_menu(Era::Win95, &bin, true).entries[2].disabled);
        assert!(item_menu(Era::Win2000, &bin, false).entries[0].bold, "Open is the default");
        assert_eq!(item_at(&bin, 2), Some(DesktopCommand::EmptyRecycleBin));
        assert_eq!(item_at(&bin, 3), None);
    }

    #[test]
    fn several_icons_get_the_entries_they_all_have() {
        let two_files = item_entries(&[ItemKind::File, ItemKind::File]);
        assert!(!two_files.contains(&Some(DesktopCommand::Rename)), "Windows renames one icon at a time");
        assert!(two_files.contains(&Some(DesktopCommand::Delete)));
        let mixed = item_entries(&[ItemKind::File, ItemKind::System(System::MyComputer)]);
        assert_eq!(mixed, [Some(DesktopCommand::Open), None, Some(DesktopCommand::CreateShortcut), None, Some(DesktopCommand::ItemProperties)]);
        assert!(item_entries(&[]).is_empty());
    }

    #[test]
    fn keys_do_what_they_did_on_the_desktop() {
        use DesktopCommand::*;
        assert_eq!(shortcut(ShortcutKey::Delete, false, false, false), Some(Some(Delete)));
        assert_eq!(shortcut(ShortcutKey::Delete, false, true, false), Some(Some(Delete)), "Shift+Del still to the Recycle Bin");
        assert_eq!(shortcut(ShortcutKey::F2, false, false, false), Some(Some(Rename)));
        assert_eq!(shortcut(ShortcutKey::Letter('X'), true, false, false), Some(Some(Cut)));
        assert_eq!(shortcut(ShortcutKey::Letter('C'), true, false, false), Some(Some(Copy)));
        assert_eq!(shortcut(ShortcutKey::Letter('V'), true, false, false), Some(Some(Paste)));
        assert_eq!(shortcut(ShortcutKey::Enter, false, false, true), Some(Some(ItemProperties)));
        assert_eq!(shortcut(ShortcutKey::F10, false, true, false), Some(None));
        assert_eq!(shortcut(ShortcutKey::Menu, false, false, false), Some(None));
        assert_eq!(shortcut(ShortcutKey::Letter('X'), false, false, false), None, "a plain letter goes to an icon's name");
        assert_eq!(shortcut(ShortcutKey::Enter, false, false, false), None, "Enter opens (desktop_icons)");
    }

    #[test]
    fn commands_survive_a_window_message() {
        for c in DesktopCommand::ALL {
            assert_eq!(DesktopCommand::from_code(c.code()), Some(c));
        }
        assert_eq!(DesktopCommand::from_code(99), None);
    }

    #[test]
    fn new_items_get_windows_own_names() {
        let taken = ["New folder", "New folder (2)", "New Text Document.txt"];
        let is_taken = |n: &str| taken.contains(&n);
        assert_eq!(new_name("New folder", "", is_taken), "New folder (3)");
        assert_eq!(new_name("New Text Document", ".txt", is_taken), "New Text Document (2).txt");
        assert_eq!(new_name("New Text Document", ".txt", |_| false), "New Text Document.txt");
    }
}
