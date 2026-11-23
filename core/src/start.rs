//! The Start menu's own entries, as each Windows had them, and what each one is.
//!
//! Labels mark their access key with `&` ("Sh&ut Down..." underlines the u), the way
//! Windows menus are written; the painter underlines it and the keyboard handler uses it.

use crate::config::StartOptions;
use crate::fb::Icon;
use crate::paint::{Menu, MenuEntry};
use crate::programs::Item;
use crate::text::escape_access_keys;
use crate::theme::Era;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartEntry {
    WindowsUpdate,
    Programs,
    /// Windows 98: your Internet Explorer favorites.
    Favorites,
    Documents,
    Settings,
    /// "Find" in Windows 95 and 98, "Search" in Windows 2000.
    Find,
    Help,
    Run,
    LogOff,
    ShutDown,
}

use StartEntry::*;

/// The entries top to bottom (`None` is a separator), as each version shipped them. Windows
/// 2000's Favorites and Log Off follow its Start Menu Settings (`opts`); the other looks
/// ignore them (98 always has both, 95 neither).
pub fn entries(era: Era, opts: StartOptions) -> Vec<Option<StartEntry>> {
    match era {
        Era::Win95 => {
            vec![Some(Programs), Some(Documents), Some(Settings), Some(Find), Some(Help), Some(Run), None, Some(ShutDown)]
        }
        Era::Win98 => vec![
            Some(WindowsUpdate),
            None,
            Some(Programs),
            Some(Favorites),
            Some(Documents),
            Some(Settings),
            Some(Find),
            Some(Help),
            Some(Run),
            None,
            Some(LogOff),
            Some(ShutDown),
        ],
        Era::Win2000 => {
            let mut v = vec![Some(WindowsUpdate), None, Some(Programs)];
            if opts.favorites {
                v.push(Some(Favorites));
            }
            v.extend([Some(Documents), Some(Settings), Some(Find), Some(Help), Some(Run), None]);
            if opts.log_off {
                v.push(Some(LogOff));
            }
            v.push(Some(ShutDown));
            v
        }
    }
}

/// The label of an entry (`user` appears in "Log Off <user>...").
pub fn label(entry: StartEntry, era: Era, user: &str) -> String {
    match (entry, era) {
        (WindowsUpdate, _) => "Windows Update".into(),
        (Programs, _) => "&Programs".into(),
        (Favorites, _) => "F&avorites".into(),
        (Documents, _) => "&Documents".into(),
        (Settings, _) => "&Settings".into(),
        (Find, Era::Win95 | Era::Win98) => "&Find".into(),
        (Find, Era::Win2000) => "Sear&ch".into(),
        (Help, _) => "&Help".into(),
        (Run, _) => "&Run...".into(),
        (LogOff, _) => format!("&Log Off {user}..."),
        (ShutDown, _) => "Sh&ut Down...".into(),
    }
}

/// Whether the entry opens a submenu.
pub fn has_submenu(entry: StartEntry) -> bool {
    matches!(entry, Programs | Favorites | Documents | Settings | Find)
}

/// The Start menu for `era`, with icons from `icon`.
pub fn menu(era: Era, opts: StartOptions, user: &str, mut icon: impl FnMut(StartEntry) -> Option<Icon>) -> Menu {
    let entries = entries(era, opts)
        .iter()
        .map(|entry| match *entry {
            None => MenuEntry::separator(),
            Some(e) if has_submenu(e) => MenuEntry::submenu(&label(e, era, user), icon(e)),
            Some(e) => MenuEntry::item(&label(e, era, user), icon(e)),
        })
        .collect();
    Menu::new(entries)
}

/// What entry `index` of `menu(era, opts, ..)` is.
pub fn entry_at(era: Era, opts: StartOptions, index: usize) -> Option<StartEntry> {
    entries(era, opts).get(index).copied().flatten()
}

/// What an entry of the Documents menu stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentsEntry {
    /// Windows 98 and 2000: "My Documents" at the top opens that folder.
    MyDocuments,
    /// The document at this index of the list given to `documents_menu`.
    Document(usize),
}

/// Windows 98 and 2000 start the Documents menu with "My Documents" and a line; Windows 95 had
/// no My Documents.
fn has_my_documents(era: Era) -> bool {
    era != Era::Win95
}

/// The Documents menu, as read on the reference PCs: the recent documents in `era`'s order
/// (`order_documents`), below "My Documents" and a line in 98 and 2000. With none, Windows 95
/// and 2000 show a grayed "(Empty)"; 98 ends at its line.
pub fn documents_menu(
    era: Era,
    docs: &[Item],
    my_documents: Option<Icon>,
    mut icon: impl FnMut(&Item) -> Option<Icon>,
) -> Menu {
    let mut entries = Vec::new();
    if has_my_documents(era) {
        entries.extend([MenuEntry::item("My Documents", my_documents), MenuEntry::separator()]);
    }
    entries.extend(docs.iter().map(|d| MenuEntry::item(&escape_access_keys(&d.name), icon(d))));
    if docs.is_empty() && era != Era::Win98 {
        let mut empty = MenuEntry::item("(Empty)", None);
        empty.disabled = true;
        entries.push(empty);
    }
    // Windows 2000 leaves one more row under this menu's last entry than under its others'.
    Menu { extra_bottom: i32::from(era == Era::Win2000), ..Menu::new(entries) }
}

/// What entry `index` of `documents_menu(era, ..)` with `count` documents stands for.
pub fn documents_entry(era: Era, count: usize, index: usize) -> Option<DocumentsEntry> {
    let first = if has_my_documents(era) { 2 } else { 0 };
    match index {
        0 if has_my_documents(era) => Some(DocumentsEntry::MyDocuments),
        i if i >= first && i - first < count => Some(DocumentsEntry::Document(i - first)),
        _ => None,
    }
}

/// Puts the recent documents in the order `era`'s Documents menu lists them: Windows 95 and
/// 2000 by name, ignoring case; Windows 98 in the order they were first opened, oldest first.
/// Each comes with when it was first opened (any increasing number).
pub fn order_documents(era: Era, docs: &mut [(Item, u64)]) {
    if era == Era::Win98 {
        docs.sort_by_key(|(_, first_opened)| *first_opened);
    } else {
        docs.sort_by_key(|(item, _)| item.name.to_lowercase());
    }
}

/// The fixed submenus (Settings, Find) as (label, what to open) pairs, `None` a separator,
/// as the reference PCs show them (real Windows 95, 98 and 2000 without a network, so Find
/// has no "Computer..."). The targets are today's Windows equivalents of the old ones.
pub fn fixed_submenu(entry: StartEntry, era: Era) -> &'static [Option<(&'static str, Target)>] {
    match (entry, era) {
        (Settings, Era::Win95) => &[
            Some(("&Control Panel", Target::ControlPanel)),
            Some(("&Printers", Target::Printers)),
            Some(("&Taskbar...", Target::BarSettings)),
        ],
        // Windows 98 also had Active Desktop above the separator (a submenu with no
        // counterpart today).
        (Settings, Era::Win98) => &[
            Some(("&Control Panel", Target::ControlPanel)),
            Some(("&Printers", Target::Printers)),
            Some(("&Taskbar && Start Menu...", Target::BarSettings)),
            Some(("&Folder Options...", Target::FolderOptions)),
            None,
            // No access key here.
            Some(("Windows Update...", Target::WindowsUpdate)),
        ],
        (Settings, Era::Win2000) => &[
            Some(("&Control Panel", Target::ControlPanel)),
            None,
            Some(("&Network and Dial-up Connections", Target::Network)),
            Some(("&Printers", Target::Printers)),
            Some(("&Taskbar && Start Menu...", Target::BarSettings)),
        ],
        // Each look's Find (Search) ends with Bevel95's own entry, Quick Search (until 2026-11-16
        // "Programs...").
        (Find, Era::Win95) => {
            &[Some(("&Files or Folders...", Target::FindFiles)), Some(("&Quick Search...", Target::FindPrograms))]
        }
        // Windows 98 and 2000 also had People... (their address book search).
        (Find, Era::Win98) => {
            &[
                Some(("&Files or Folders...", Target::FindFiles)),
                Some(("On the &Internet...", Target::Internet)),
                Some(("&Quick Search...", Target::FindPrograms)),
            ]
        }
        (Find, Era::Win2000) => {
            &[
                Some(("For &Files or Folders...", Target::FindFiles)),
                Some(("On the &Internet...", Target::Internet)),
                Some(("&Quick Search...", Target::FindPrograms)),
            ]
        }
        _ => &[],
    }
}

/// Something the platform opens for a fixed menu entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    ControlPanel,
    Printers,
    Network,
    /// The bar's Taskbar Properties window.
    BarSettings,
    FolderOptions,
    WindowsUpdate,
    FindFiles,
    Internet,
    /// Bevel95's own Quick Search (BarCommand::Find).
    FindPrograms,
}

/// A fixed submenu as a menu, with each entry's classic icon.
pub fn fixed_menu(entry: StartEntry, era: Era) -> Menu {
    let entries = fixed_submenu(entry, era)
        .iter()
        .map(|e| match *e {
            Some((label, target)) => MenuEntry::item(label, crate::icons::target_icon(era, target)),
            None => MenuEntry::separator(),
        })
        .collect();
    Menu::new(entries)
}

/// What entry `index` of `fixed_menu(entry, era)` opens (`None` for a separator).
pub fn fixed_target(entry: StartEntry, era: Era, index: usize) -> Option<Target> {
    fixed_submenu(entry, era).get(index).copied().flatten().map(|(_, target)| target)
}

/// Which of the Start menu's long menus a menu is, for its look's rule (`long_rule`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LongKind {
    /// The Programs menu and its folders.
    Programs,
    /// Favorites and its folders.
    Favorites,
    /// Any other menu of the Start menu's family: the Start menu itself, Documents, Settings,
    /// Find, a task button's list of windows, Windows 2000's Quick Launch list.
    Other,
}

/// How a menu longer than the screen goes on in `era` (brief 91; the Reference PCs, survey
/// part 1): Windows 95 in columns; Windows 98 scrolls every long menu; Windows 2000 in columns,
/// its Programs menu scrolling with "Scroll the Programs menu" on and its Favorites always.
pub fn long_rule(era: Era, kind: LongKind, scroll_programs: bool) -> crate::paint::Long {
    use crate::paint::Long;
    match era {
        Era::Win95 => Long::Columns { own_widths: true },
        Era::Win98 => Long::Scroll { flat: false },
        Era::Win2000 => match kind {
            LongKind::Favorites => Long::Scroll { flat: true },
            LongKind::Programs if scroll_programs => Long::Scroll { flat: true },
            _ => Long::Columns { own_widths: false },
        },
    }
}

/// What a look's menu does when its columns would not fit the screen: it scrolls (never an entry
/// off the screen: the owner's decision for Windows 95, which ran off; applied to Windows 2000 too,
/// PROVISIONAL), Windows 95 with strips drawn as 98's (it had none), 2000 with its own flat ones.
pub fn scroll_instead(era: Era) -> crate::paint::Long {
    crate::paint::Long::Scroll { flat: era == Era::Win2000 }
}

/// Where a menu of columns `w` wide stands across a screen `screen_x..screen_x + screen_w`: right of
/// its parent (`right`: its left edge there) if it fits there whole; else left of its parent
/// (`left`: its left edge there), as every Windows menu goes and as Bevel95 did before brief 91;
/// only where it fits on neither side the look's last way: Windows 95 moves it to the screen's
/// left edge (measured for the Programs menu beside the Start menu, whose left has no room),
/// Windows 2000 never moves it. `None`: it scrolls instead (`scroll_instead`). The left side is
/// PROVISIONAL (brief 92): only the Programs menu beside the Start menu was measured.
pub fn columns_x(era: Era, right: i32, left: i32, w: i32, screen_x: i32, screen_w: i32) -> Option<i32> {
    if right + w <= screen_x + screen_w {
        return Some(right);
    }
    if left >= screen_x {
        return Some(left);
    }
    (era == Era::Win95 && w <= screen_w).then_some(screen_x)
}

/// A menu of the Start menu's family laid out and placed beside entry `entry` of its parent
/// `parent` (screen pixels at `scale`, on `screen`), as the bar opens a submenu: the look's rule for
/// a menu longer than the screen (`long`, scrolled to `offset`); beside the entry's own column when
/// the parent stands in columns (`parent_in_columns`: never for Windows' own popup menus); a block
/// of columns where `columns_x` puts it, scrolling where it fits nowhere; anything else where
/// `layout::place_submenu` puts it. One piece of code for the bar and the golden pictures (brief
/// 92). Returns the layout (1×) and the menu's left and top on the screen.
#[allow(clippy::too_many_arguments)]
pub fn place_cascade(
    theme: &crate::theme::Theme,
    fonts: &crate::text::Fonts,
    era: Era,
    menu: &Menu,
    style: crate::paint::MenuStyle,
    long: crate::paint::Long,
    offset: i32,
    scale: crate::scale::Scale,
    screen: crate::fb::Rect,
    parent: crate::fb::Rect,
    entry: crate::fb::Rect,
    parent_in_columns: bool,
) -> (crate::paint::MenuLayout, i32, i32) {
    use crate::fb::Rect;
    use crate::paint::{self, MenuStyle};
    let (rule, frame) = match style {
        MenuStyle::Popup => (theme.popup.submenu, theme.popup.frame),
        _ => (theme.start_menu.submenu, theme.start_menu.frame),
    };
    let rule = crate::theme::SubmenuPlacement { overlap: scale.len(rule.overlap), margin_bottom: scale.len(rule.margin_bottom), ..rule };
    let frame = scale.len(frame);
    let parent = if parent_in_columns { Rect::new(entry.x - frame, parent.y, entry.w + 2 * frame, parent.h) } else { parent };
    let screen_h = scale.old_fit(screen.h);
    let mut layout = match style {
        MenuStyle::Popup => paint::layout_menu(theme, fonts, menu, style, screen_h),
        _ => paint::layout_long_menu(theme, fonts, menu, style, screen_h, long, offset),
    };
    if style != MenuStyle::Popup && layout.columns.len() > 1 {
        let w = scale.len(layout.width);
        match columns_x(era, parent.right() - rule.overlap, parent.x + rule.overlap - w, w, screen.x, screen.w) {
            Some(x) => {
                let (_, y) = crate::layout::place_submenu(parent, entry, w, scale.len(layout.height), frame, rule, screen);
                return (layout, x, y);
            }
            None => layout = paint::layout_long_menu(theme, fonts, menu, style, screen_h, scroll_instead(era), offset),
        }
    }
    let (x, y) = crate::layout::place_submenu(parent, entry, scale.len(layout.width), scale.len(layout.height), frame, rule, screen);
    (layout, x, y)
}

/// A root menu of the Start menu's family (the Start menu, a task button's list, 2000's Quick
/// Launch list) laid out for `screen`: the look's rule, and columns wider than the whole screen
/// scroll instead. Windows' own popup menus as before.
#[allow(clippy::too_many_arguments)]
pub fn layout_root(
    theme: &crate::theme::Theme,
    fonts: &crate::text::Fonts,
    era: Era,
    menu: &Menu,
    style: crate::paint::MenuStyle,
    long: crate::paint::Long,
    offset: i32,
    scale: crate::scale::Scale,
    screen: crate::fb::Rect,
) -> crate::paint::MenuLayout {
    use crate::paint::{self, MenuStyle};
    let screen_h = scale.old_fit(screen.h);
    if style == MenuStyle::Popup {
        return paint::layout_menu(theme, fonts, menu, style, screen_h);
    }
    let layout = paint::layout_long_menu(theme, fonts, menu, style, screen_h, long, offset);
    if layout.columns.len() > 1 && scale.len(layout.width) > screen.w {
        return paint::layout_long_menu(theme, fonts, menu, style, screen_h, scroll_instead(era), offset);
    }
    layout
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each look's own way with a menu longer than the screen (survey part 1).
    #[test]
    fn each_look_has_its_own_long_menus() {
        use crate::paint::Long;
        for kind in [LongKind::Programs, LongKind::Favorites, LongKind::Other] {
            assert_eq!(long_rule(Era::Win95, kind, true), Long::Columns { own_widths: true });
            assert_eq!(long_rule(Era::Win98, kind, false), Long::Scroll { flat: false });
        }
        assert_eq!(long_rule(Era::Win2000, LongKind::Programs, false), Long::Columns { own_widths: false }, "a fresh installation");
        assert_eq!(long_rule(Era::Win2000, LongKind::Programs, true), Long::Scroll { flat: true }, "Scroll the Programs menu");
        assert_eq!(long_rule(Era::Win2000, LongKind::Favorites, false), Long::Scroll { flat: true }, "Favorites always");
        assert_eq!(long_rule(Era::Win2000, LongKind::Other, true), Long::Columns { own_widths: false });
        assert_eq!(scroll_instead(Era::Win95), Long::Scroll { flat: false });
        assert_eq!(scroll_instead(Era::Win2000), Long::Scroll { flat: true });
    }

    /// Columns stand right of their parent if they fit there, else left of it (brief 92), else the
    /// look's last way: Windows 95 the screen's left edge (measured: 80 entries at 640x480 from x 0,
    /// the Programs menu beside the Start menu, which has no room on its left), Windows 2000 never
    /// moved (`None`: it scrolls).
    #[test]
    fn columns_stand_right_then_left_then_the_looks_way() {
        // Beside the Start menu (no room on its left: the left edge would be negative).
        assert_eq!(columns_x(Era::Win95, 160, 3 - 400, 400, 0, 640), Some(160), "fits beside");
        assert_eq!(columns_x(Era::Win95, 160, 3 - 494, 494, 0, 640), Some(0), "moved to the left edge");
        assert_eq!(columns_x(Era::Win95, 160, 3 - 640, 640, 0, 640), Some(0), "just fits the width");
        assert_eq!(columns_x(Era::Win95, 160, 3 - 641, 641, 0, 640), None, "one pixel too wide: scrolls");
        assert_eq!(columns_x(Era::Win2000, 250, 3 - 398, 398, 0, 800), Some(250));
        assert_eq!(columns_x(Era::Win2000, 250, 3 - 551, 551, 0, 800), None, "would run off: scrolls");
        assert_eq!(columns_x(Era::Win95, 1920 + 160, 1920 + 3 - 494, 494, 1920, 640), Some(1920), "a second screen's left edge");
        // A long folder far to the right: left of its parent, as before brief 91, in every look.
        for era in [Era::Win95, Era::Win2000] {
            assert_eq!(columns_x(era, 600, 420 - 300, 300, 0, 800), Some(120), "{era:?}: left of the parent");
        }
    }

    /// The bar's placement of a long submenu, in one place (brief 92): a 95 Programs menu of 80
    /// entries at 640x480 beside the Start menu moves to the left edge; at Size 2 the same at twice
    /// the pixels; Windows' own popup menus keep their old place.
    #[test]
    fn a_long_submenu_is_placed_by_one_rule() {
        use crate::fb::Rect;
        use crate::paint::{Menu, MenuEntry, MenuStyle};
        use crate::scale::Scale;
        let r = crate::text::Font::parse(include_str!("../tests/fixtures/ms-sans-serif-11px.font")).unwrap();
        let b = crate::text::Font::parse(include_str!("../tests/fixtures/ms-sans-serif-11px-bold.font")).unwrap();
        let fonts = crate::text::Fonts { regular: &r, bold: &b };
        let theme = &crate::theme::WIN95;
        let mut entries: Vec<MenuEntry> = (0..100).map(|i| MenuEntry::item(&format!("Entry {i:03}"), None)).collect();
        entries.insert(0, MenuEntry::submenu("Accessories", None));
        let menu = Menu::new(entries);
        let long = long_rule(Era::Win95, LongKind::Programs, false);
        for k in [1, 2] {
            let s = Scale::whole(k);
            let screen = Rect::new(0, 0, 640 * k, 480 * k);
            let parent = Rect::new(0, 220 * k, 163 * k, 238 * k);
            let entry = Rect::new(3 * k, 223 * k, 157 * k, 32 * k);
            let (l, x, _) = place_cascade(theme, &fonts, Era::Win95, &menu, MenuStyle::Cascade, long, 0, s, screen, parent, entry, false);
            assert!(l.columns.len() > 1 && l.scroll.is_none(), "columns");
            assert_eq!(x, 0, "Size {k}: moved to the screen's left edge");
        }
        // Too many to fit even at the left edge: it scrolls, at its usual place.
        let mut more = menu.clone();
        more.entries.extend((100..240).map(|i| MenuEntry::item(&format!("Entry {i:03}"), None)));
        let screen = Rect::new(0, 0, 640, 480);
        let (l, x, y) =
            place_cascade(theme, &fonts, Era::Win95, &more, MenuStyle::Cascade, long, 0, Scale::whole(1), screen, Rect::new(0, 220, 163, 238), Rect::new(3, 223, 157, 32), false);
        assert!(l.scroll.is_some());
        assert_eq!((x, y, l.height), (163 - theme.start_menu.submenu.overlap, 0, 480), "beside the Start menu, the screen's height");
    }

    /// As read on the reference PCs after opening zebra, apple and mango (in that order).
    #[test]
    fn documents_menu_as_each_windows_listed_it() {
        let doc = |name: &str, opened: u64| (Item { name: name.into(), target: format!("{name}.lnk") }, opened);
        let opened = || vec![doc("zebra", 1), doc("apple", 2), doc("mango", 3), doc("Readme", 0)];
        let labels = |era: Era| {
            let mut docs = opened();
            order_documents(era, &mut docs);
            let docs: Vec<Item> = docs.into_iter().map(|(i, _)| i).collect();
            documents_menu(era, &docs, None, |_| None).entries.iter().map(|e| e.label.clone()).collect::<Vec<_>>()
        };
        assert_eq!(labels(Era::Win95), ["apple", "mango", "Readme", "zebra"]);
        assert_eq!(labels(Era::Win98), ["My Documents", "", "Readme", "zebra", "apple", "mango"]);
        assert_eq!(labels(Era::Win2000), ["My Documents", "", "apple", "mango", "Readme", "zebra"]);
        let empty = |era: Era| documents_menu(era, &[], None, |_| None).entries.iter().map(|e| e.label.clone()).collect::<Vec<_>>();
        assert_eq!(empty(Era::Win95), ["(Empty)"]);
        assert_eq!(empty(Era::Win98), ["My Documents", ""]);
        assert_eq!(empty(Era::Win2000), ["My Documents", "", "(Empty)"]);
        assert_eq!(documents_entry(Era::Win98, 3, 0), Some(DocumentsEntry::MyDocuments));
        assert_eq!(documents_entry(Era::Win98, 3, 1), None, "the line");
        assert_eq!(documents_entry(Era::Win98, 3, 2), Some(DocumentsEntry::Document(0)));
        assert_eq!(documents_entry(Era::Win2000, 0, 2), None, "(Empty)");
        assert_eq!(documents_entry(Era::Win95, 3, 0), Some(DocumentsEntry::Document(0)));
    }

    #[test]
    fn windows_95_start_menu() {
        let m = menu(Era::Win95, StartOptions::default(), "someone", |_| None);
        let labels: Vec<&str> = m.entries.iter().map(|e| e.label.as_str()).collect();
        assert_eq!(labels, ["&Programs", "&Documents", "&Settings", "&Find", "&Help", "&Run...", "", "Sh&ut Down..."]);
        assert_eq!(entry_at(Era::Win95, StartOptions::default(), 7), Some(ShutDown));
        assert_eq!(entry_at(Era::Win95, StartOptions::default(), 6), None);
    }

    #[test]
    fn windows_98_adds_favorites_and_log_off() {
        let m = menu(Era::Win98, StartOptions::default(), "Midnite", |_| None);
        let labels: Vec<&str> = m.entries.iter().map(|e| e.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "Windows Update",
                "",
                "&Programs",
                "F&avorites",
                "&Documents",
                "&Settings",
                "&Find",
                "&Help",
                "&Run...",
                "",
                "&Log Off Midnite...",
                "Sh&ut Down..."
            ]
        );
        assert_eq!(entry_at(Era::Win98, StartOptions::default(), 3), Some(Favorites));
        assert!(has_submenu(Favorites));
        assert_eq!(fixed_menu(Find, Era::Win98).entries.len(), 3);
        assert_eq!(fixed_target(Find, Era::Win98, 2), Some(Target::FindPrograms), "Bevel95's own entry comes last");
        // Windows Update sits under a separator.
        assert_eq!(fixed_target(Settings, Era::Win98, 4), None);
        assert_eq!(fixed_target(Settings, Era::Win98, 5), Some(Target::WindowsUpdate));
    }

    #[test]
    fn windows_2000_has_its_own_entries() {
        let m = menu(Era::Win2000, StartOptions::default(), "someone", |_| None);
        assert_eq!(m.entries[0].label, "Windows Update");
        assert!(m.entries.iter().any(|e| e.label == "&Log Off someone..."));
        assert!(m.entries.iter().any(|e| e.label == "Sear&ch"));
        assert_eq!(fixed_menu(Settings, Era::Win2000).entries.len(), 5);
        assert_eq!(fixed_target(Settings, Era::Win2000, 2), Some(Target::Network));
    }

    #[test]
    fn every_settings_and_find_entry_has_its_classic_icon() {
        for era in Era::ALL {
            for entry in [Settings, Find] {
                let menu = fixed_menu(entry, era);
                assert!(!menu.entries.is_empty());
                for e in menu.entries.iter().filter(|e| e.kind != crate::paint::EntryKind::Separator) {
                    assert!(e.icon.is_some(), "{era:?} {entry:?} {}", e.label);
                }
            }
        }
    }
}
