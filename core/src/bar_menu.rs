//! The bar's right-click menu: what it says and what each entry does.
//!
//! The menu reads as the original Windows' plus one entry: everything of Bevel95's own sits in
//! the "Bevel95" submenu (`own_menu`, the owner's request of 2026-11-16), with the Theme and Size
//! submenus inside it.

use crate::config::MENU_SCALES;
use crate::paint::{Mark, Menu, MenuEntry};
use crate::scale::Scale;
use crate::theme::{Depth, Era};

/// What a bar menu entry does. The platform layer carries it out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarCommand {
    /// Hide this bar and bring back the Windows taskbar (same as Ctrl+Win+T).
    SwitchToWindowsTaskbar,
    /// Open the Taskbar Properties window.
    Properties,
    Restart,
    /// Exit; with Shift held, restart instead.
    Exit,
    /// Open Date/Time Properties (the clock's menu only).
    AdjustDateTime,
    /// Open the Bevel95 Settings window (everything that is Bevel95's own).
    Settings,
    /// Open Quick Search (also the Windows key, the Start menu's Find submenu, scripts).
    Find,
    // No menu entry: things Quick Search does (`quick`).
    /// The Run dialog, as Start > Run... opens it.
    Run,
    /// The Shut Down dialog, as Start > Shut Down... opens it.
    ShutDown,
    /// Bevel95's Windows Explorer (the folder windows' two panes).
    Explorer,
    /// Bevel95's My Computer window, as its desktop icon opens it.
    MyComputer,
    /// Bevel95's classic desktop on (Bevel95 Settings' Desktop page has the same switch).
    DesktopOn,
    /// And off, as the desktop menu's Switch to Windows desktop.
    DesktopOff,
    /// The About Bevel95 window (the Bevel95 submenu's last entry; Quick Search).
    About,
    /// Back from Windows' taskbar to Bevel95's bar (the first entry of Bevel95's notification
    /// icon's menu, `tray_icon`); nothing while Bevel95's bar shows.
    SwitchToBevel95,
    /// Quick Launch on if it is off, off if it is on (Toolbars > Quick Launch).
    QuickLaunch,
    /// The Quick Launch folder in a folder window (the band's Open).
    QuickLaunchOpen,
    /// Quick Launch off (the band's Close).
    QuickLaunchClose,
    /// The Quick Launch folder read again (the band's Refresh).
    QuickLaunchRefresh,
    /// The latest release page in the person's browser (`about::UPDATES_URL`; About Bevel95's
    /// Check for updates, Quick Search). Bevel95 asks nothing itself (brief 100).
    CheckForUpdates,
}

impl BarCommand {
    /// In the order of their codes: new commands join the end, so the codes that scripts and
    /// tests send to the bar stay what they were.
    const ALL: [BarCommand; 20] = [
        BarCommand::SwitchToWindowsTaskbar,
        BarCommand::Properties,
        BarCommand::Restart,
        BarCommand::Exit,
        BarCommand::AdjustDateTime,
        BarCommand::Settings,
        BarCommand::Find,
        BarCommand::Run,
        BarCommand::ShutDown,
        BarCommand::Explorer,
        BarCommand::MyComputer,
        BarCommand::DesktopOn,
        BarCommand::DesktopOff,
        BarCommand::About,
        BarCommand::SwitchToBevel95,
        BarCommand::QuickLaunch,
        BarCommand::QuickLaunchOpen,
        BarCommand::QuickLaunchClose,
        BarCommand::QuickLaunchRefresh,
        BarCommand::CheckForUpdates,
    ];

    /// A number standing for the command in a window message (`from_code` reads it back).
    pub fn code(self) -> usize {
        self as usize
    }

    pub fn from_code(code: usize) -> Option<BarCommand> {
        BarCommand::ALL.get(code).copied()
    }
}

/// What choosing a top-level entry does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarEntry {
    Command(BarCommand),
    /// Opens the Theme submenu (`theme_menu`).
    Theme,
    /// Opens the Colors submenu (`colors_menu`).
    Colors,
    /// Opens the Size submenu (`size_menu`).
    Size,
    /// Opens the Bevel95 submenu (`own_menu`).
    Bevel95,
    /// Opens the Toolbars submenu (`toolbars_menu`; 98 and 2000).
    Toolbars,
}

type Entry = Option<(&'static str, BarEntry)>;

/// `None` is a separator. Bevel95's one entry stands where its settings stood, right above
/// Windows' own Properties, which stays the last entry, as it was in Windows' taskbar menu.
const ENTRIES: [Entry; 2] = [Some(("Bevel95", BarEntry::Bevel95)), Some(("Properties", BarEntry::Command(BarCommand::Properties)))];

/// The key that switches Bevel95's classic desktop on and off, as Ctrl+Win+T switches the bar
/// (the bar registers it; `DESKTOP_KEY_LETTER` is its letter). Chosen 2026-11-17 among the
/// Ctrl+Win+letters that Windows 11 left free: W, as T is for the taskbar.
pub const DESKTOP_KEY: &str = "Ctrl+Win+W";
pub const DESKTOP_KEY_LETTER: u8 = b'W';

/// What the Bevel95 submenu shows that changes: whether the classic desktop is on (its entry
/// then switches it off) and whether `DESKTOP_KEY` could be registered (then it shows beside
/// the entry, as Ctrl+Win+T does).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Own {
    pub desktop: bool,
    pub desktop_key: bool,
}

/// The desktop's entry, right under Switch back to Windows: what it switches to.
fn desktop_entry(own: Own) -> Entry {
    let label = match (own.desktop, own.desktop_key) {
        (true, true) => "Switch to Windows desktop\tCtrl+Win+W",
        (true, false) => "Switch to Windows desktop",
        (false, true) => "Switch to classic desktop\tCtrl+Win+W",
        (false, false) => "Switch to classic desktop",
    };
    let command = if own.desktop { BarCommand::DesktopOff } else { BarCommand::DesktopOn };
    Some((label, BarEntry::Command(command)))
}

/// The Bevel95 submenu: everything of Bevel95's own that the menu held before it, nothing lost
/// (PROVISIONAL: the order and the wording).
fn own_entries(own: Own) -> [Entry; 13] {
    [
        Some(("Bevel95 Settings", BarEntry::Command(BarCommand::Settings))),
        Some(("Theme", BarEntry::Theme)),
        Some(("Colors", BarEntry::Colors)),
        Some(("Size", BarEntry::Size)),
        Some(("Quick Search", BarEntry::Command(BarCommand::Find))),
        None,
        Some(("Switch back to Windows\tCtrl+Win+T", BarEntry::Command(BarCommand::SwitchToWindowsTaskbar))),
        desktop_entry(own),
        None,
        Some(("Restart", BarEntry::Command(BarCommand::Restart))),
        Some(("Exit", BarEntry::Command(BarCommand::Exit))),
        None,
        Some((concat!("About ", crate::product_name!()), BarEntry::Command(BarCommand::About))),
    ]
}

/// Menu entries for `entries`, a submenu arrow on those that open one.
fn menu_of<'a>(entries: impl Iterator<Item = &'a Entry>) -> Menu {
    let entries = entries
        .map(|e| match e {
            Some((label, BarEntry::Theme | BarEntry::Colors | BarEntry::Size | BarEntry::Bevel95 | BarEntry::Toolbars)) => {
                MenuEntry::submenu(label, None)
            }
            Some((label, _)) => MenuEntry::item(label, None),
            None => MenuEntry::separator(),
        })
        .collect();
    Menu::new(entries)
}

/// The Bevel95 submenu, nothing highlighted.
pub fn own_menu(own: Own) -> Menu {
    menu_of(own_entries(own).iter())
}

/// What entry `index` of `own_menu(own)` does (`None` for a separator).
pub fn own_entry(own: Own, index: usize) -> Option<BarEntry> {
    own_entries(own).get(index).copied().flatten().map(|(_, entry)| entry)
}

/// Where `entry` is in `own_menu(own)`.
pub fn own_index_of(own: Own, entry: BarEntry) -> Option<usize> {
    own_entries(own).iter().position(|e| e.is_some_and(|(_, e)| e == entry))
}

/// Right-clicked on the clock, Windows 95, 98 and 2000 put this first, then a separator.
const CLOCK_ENTRIES: [Entry; 2] = [Some(("Adjust Date/Time", BarEntry::Command(BarCommand::AdjustDateTime))), None];

/// Windows 98's and 2000's menus begin with Toolbars and a line (the Reference PCs, mouse and
/// keyboard alike); a right-click on the clock puts Adjust Date/Time after them. Windows 95 had no
/// toolbars on its bar.
const TOOLBARS_ENTRIES: [Entry; 2] = [Some(("&Toolbars", BarEntry::Toolbars)), None];

/// The entries of `era`'s menu, for a right-click on the clock (`clock`) or elsewhere on the bar.
fn entries(era: Era, clock: bool) -> impl Iterator<Item = &'static Entry> {
    let toolbars: &'static [Entry] = if era == Era::Win95 { &[] } else { &TOOLBARS_ENTRIES };
    let first: &'static [Entry] = if clock { &CLOCK_ENTRIES } else { &[] };
    toolbars.iter().chain(first.iter()).chain(ENTRIES.iter())
}

/// The right-click menu of the bar in `era` (`clock`: right-clicked on the clock), nothing
/// highlighted.
pub fn menu(era: Era, clock: bool) -> Menu {
    menu_of(entries(era, clock))
}

/// What entry `index` of `menu(era, clock)` does (`None` for a separator).
pub fn entry(era: Era, clock: bool, index: usize) -> Option<BarEntry> {
    entries(era, clock).nth(index).copied().flatten().map(|(_, entry)| entry)
}

/// Where `entry` is in `menu(era, clock)`.
pub fn index_of(era: Era, clock: bool, entry: BarEntry) -> Option<usize> {
    entries(era, clock).position(|e| e.is_some_and(|(_, e)| e == entry))
}

/// The Toolbars submenu (the Reference PCs' 98 and 2000 alike): Address, Links, Desktop, Quick
/// Launch with its check mark while it shows, a line, New Toolbar... Only Quick Launch is built:
/// the others are grayed.
pub fn toolbars_menu(quick_launch: bool) -> Menu {
    let gray = |label: &str| MenuEntry { disabled: true, ..MenuEntry::item(label, None) };
    let mut ql = MenuEntry::item("&Quick Launch", None);
    if quick_launch {
        ql.mark = Some(Mark::Check);
    }
    Menu::new(vec![gray("&Address"), gray("&Links"), gray("&Desktop"), ql, MenuEntry::separator(), gray("&New Toolbar...")])
}

/// What entry `index` of `toolbars_menu` does: only Quick Launch does anything.
pub fn toolbars_entry(index: usize) -> Option<BarCommand> {
    (index == TOOLBARS_QUICK_LAUNCH).then_some(BarCommand::QuickLaunch)
}

/// Where Quick Launch is in `toolbars_menu`.
pub const TOOLBARS_QUICK_LAUNCH: usize = 3;

/// A submenu of options with a bullet on the one in use.
fn choices(labels: impl Iterator<Item = (String, bool)>) -> Menu {
    let entries = labels
        .map(|(label, current)| {
            let mut entry = MenuEntry::item(&label, None);
            if current {
                entry.mark = Some(Mark::Bullet);
            }
            entry
        })
        .collect();
    Menu::new(entries)
}

/// The Theme submenu: "Windows 95", "Windows 98", "Windows 2000", a bullet on the look in use.
pub fn theme_menu(current: Era) -> Menu {
    choices(Era::ALL.iter().map(|&era| (era.title().to_string(), era == current)))
}

/// The look chosen by entry `index` of `theme_menu`.
pub fn theme_at(index: usize) -> Option<Era> {
    Era::ALL.get(index).copied()
}

/// The Colors submenu: "16 Colors", "True Color" (the order of Windows' own Display Properties
/// list), a bullet on the colors in use.
pub fn colors_menu(current: Depth) -> Menu {
    choices(Depth::ALL.iter().map(|&d| (d.title().to_string(), d == current)))
}

/// The colors chosen by entry `index` of `colors_menu`.
pub fn colors_at(index: usize) -> Option<Depth> {
    Depth::ALL.get(index).copied()
}

/// The Size submenu: "1×" to "4×", with a bullet on the size in use; the whole sizes in bold
/// (the menu's own bold, a default entry's), the sizes between them plain.
pub fn size_menu(current: Scale) -> Menu {
    let mut menu = choices(MENU_SCALES.iter().map(|&s| (format!("{s}\u{d7}"), s.quarters() == current.quarters())));
    for (entry, s) in menu.entries.iter_mut().zip(MENU_SCALES) {
        entry.bold = s.is_whole();
    }
    menu
}

/// The Size menu's and Quick Search's sizes between whole numbers: put on at once, saved only
/// after Yes to the look's question (the owner, 2026-11-21). The bar keeps one of these; its
/// size setting (None: automatic) goes through it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SizeAsking {
    /// The open question: its ticket, the size it asks about and the setting before the first
    /// of these questions.
    open: Option<(usize, Scale, Option<Scale>)>,
    tickets: usize,
}

/// What a size chosen in the Size menu or Quick Search does (`SizeAsking::choose`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SizeChosen {
    /// A whole size: on and saved at once, no question (an open one is closed: `forget`).
    Save(Scale),
    /// A size between whole numbers: on at once, not saved; the question asks, its answer naming
    /// this ticket.
    Ask(Scale, usize),
}

/// What the question's answer does (`SizeAsking::answer`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SizeAnswer {
    /// Yes: this size is saved.
    Keep(Scale),
    /// Anything else: back to this setting (None: automatic); nothing saved.
    Back(Option<Scale>),
}

impl SizeAsking {
    /// `scale` chosen while the size setting is `setting`.
    pub fn choose(&mut self, setting: Option<Scale>, scale: Scale) -> SizeChosen {
        let scale = Scale::from_quarters(scale.quarters());
        if scale.is_whole() {
            self.forget();
            return SizeChosen::Save(scale);
        }
        // A question already open: the setting before is still the one before it.
        let before = self.open.map_or(setting, |(_, _, before)| before);
        self.tickets += 1;
        self.open = Some((self.tickets, scale, before));
        SizeChosen::Ask(scale, self.tickets)
    }

    /// The answer of question `ticket`: `keep` for Yes. None for an older question's answer, or
    /// once forgotten.
    pub fn answer(&mut self, ticket: usize, keep: bool) -> Option<SizeAnswer> {
        let (_, asked, before) = self.open.filter(|&(t, _, _)| t == ticket)?;
        self.open = None;
        Some(if keep { SizeAnswer::Keep(asked) } else { SizeAnswer::Back(before) })
    }

    /// A size set and saved another way (Bevel95 Settings): the open question means nothing now.
    /// True if one was open.
    pub fn forget(&mut self) -> bool {
        self.open.take().is_some()
    }
}

/// The scale chosen by entry `index` of `size_menu`.
pub fn size_at(index: usize) -> Option<Scale> {
    MENU_SCALES.get(index).copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::EntryKind;

    fn labels(m: &Menu) -> Vec<String> {
        m.entries
            .iter()
            .map(|e| match e.kind {
                EntryKind::Separator => "-".to_string(),
                EntryKind::Submenu => format!("{}>", e.label),
                _ => e.label.clone(),
            })
            .collect()
    }

    #[test]
    fn the_menu_is_windows_own_plus_one_entry() {
        // Windows 95, 98 and 2000 each had Properties last (the entries they had that Bevel95
        // does not do are left out), and Bevel95's one entry; 98 and 2000 begin with Toolbars.
        assert_eq!(labels(&menu(Era::Win95, false)), ["Bevel95>", "Properties"]);
        assert_eq!(labels(&menu(Era::Win95, true)), ["Adjust Date/Time", "-", "Bevel95>", "Properties"]);
        for era in [Era::Win98, Era::Win2000] {
            assert_eq!(labels(&menu(era, false)), ["&Toolbars>", "-", "Bevel95>", "Properties"]);
            assert_eq!(labels(&menu(era, true)), ["&Toolbars>", "-", "Adjust Date/Time", "-", "Bevel95>", "Properties"]);
            assert_eq!(entry(era, false, 0), Some(BarEntry::Toolbars));
        }
        for era in Era::ALL {
            for clock in [false, true] {
                let m = menu(era, clock);
                for (i, e) in m.entries.iter().enumerate() {
                    assert_eq!(e.kind == EntryKind::Separator, entry(era, clock, i).is_none(), "entry {i}");
                }
                let own = index_of(era, clock, BarEntry::Bevel95).unwrap();
                assert_eq!(entry(era, clock, own), Some(BarEntry::Bevel95));
                assert_eq!(own + 1, index_of(era, clock, BarEntry::Command(BarCommand::Properties)).unwrap());
                assert_eq!(entry(era, clock, 99), None);
                assert!(m.entries.iter().all(|e| e.mark.is_none()));
                // Bevel95's own entries are only in its submenu.
                for own in [BarEntry::Theme, BarEntry::Colors, BarEntry::Size, BarEntry::Command(BarCommand::Settings), BarEntry::Command(BarCommand::Exit)] {
                    assert_eq!(index_of(era, clock, own), None);
                }
            }
            assert_eq!(index_of(era, false, BarEntry::Command(BarCommand::AdjustDateTime)), None);
        }
        assert_eq!(entry(Era::Win95, true, 0), Some(BarEntry::Command(BarCommand::AdjustDateTime)));
        assert_eq!(entry(Era::Win95, true, 1), None, "then a separator");
        assert_eq!(index_of(Era::Win95, false, BarEntry::Toolbars), None, "95's bar had no toolbars");
    }

    #[test]
    fn the_toolbars_submenu_has_quick_launch_working() {
        let on = toolbars_menu(true);
        assert_eq!(labels(&on), ["&Address", "&Links", "&Desktop", "&Quick Launch", "-", "&New Toolbar..."]);
        let working: Vec<bool> = on.entries.iter().map(|e| e.choosable()).collect();
        assert_eq!(working, [false, false, false, true, false, false]);
        assert_eq!(on.entries[TOOLBARS_QUICK_LAUNCH].mark, Some(Mark::Check));
        assert_eq!(toolbars_menu(false).entries[TOOLBARS_QUICK_LAUNCH].mark, None);
        assert_eq!(toolbars_entry(TOOLBARS_QUICK_LAUNCH), Some(BarCommand::QuickLaunch));
        assert_eq!(toolbars_entry(0), None);
    }

    #[test]
    fn the_bevel95_submenu_holds_everything_of_ours() {
        let on = Own { desktop: true, desktop_key: true };
        let m = own_menu(on);
        assert_eq!(
            labels(&m),
            [
                "Bevel95 Settings",
                "Theme>",
                "Colors>",
                "Size>",
                "Quick Search",
                "-",
                "Switch back to Windows\tCtrl+Win+T",
                "Switch to Windows desktop\tCtrl+Win+W",
                "-",
                "Restart",
                "Exit",
                "-",
                "About Bevel95"
            ]
        );
        for (i, e) in m.entries.iter().enumerate() {
            assert_eq!(e.kind == EntryKind::Separator, own_entry(on, i).is_none(), "entry {i}");
        }
        for submenu in [BarEntry::Theme, BarEntry::Colors, BarEntry::Size] {
            let i = own_index_of(on, submenu).unwrap();
            assert_eq!(m.entries[i].kind, EntryKind::Submenu);
            assert_eq!(own_entry(on, i), Some(submenu));
        }
        // Every command the menu held before, and Quick Search.
        use BarCommand::*;
        for c in [Settings, Find, SwitchToWindowsTaskbar, Restart, Exit, About] {
            assert!(own_index_of(on, BarEntry::Command(c)).is_some(), "{c:?}");
        }
        assert_eq!(own_index_of(on, BarEntry::Command(Properties)), None, "Windows' own stays in the menu");
        assert_eq!(own_entry(on, 99), None);
    }

    #[test]
    fn the_desktop_entry_says_what_it_switches_to() {
        // Right after Switch back to Windows (the taskbar); the key only when the bar
        // could register it.
        let taskbar = own_index_of(Own::default(), BarEntry::Command(BarCommand::SwitchToWindowsTaskbar)).unwrap();
        for (own, label, command) in [
            (Own { desktop: true, desktop_key: true }, "Switch to Windows desktop\tCtrl+Win+W", BarCommand::DesktopOff),
            (Own { desktop: true, desktop_key: false }, "Switch to Windows desktop", BarCommand::DesktopOff),
            (Own { desktop: false, desktop_key: true }, "Switch to classic desktop\tCtrl+Win+W", BarCommand::DesktopOn),
            (Own { desktop: false, desktop_key: false }, "Switch to classic desktop", BarCommand::DesktopOn),
        ] {
            let m = own_menu(own);
            assert_eq!(m.entries[taskbar + 1].label, label);
            assert_eq!(own_entry(own, taskbar + 1), Some(BarEntry::Command(command)));
            assert_eq!(own_index_of(own, BarEntry::Command(command)), Some(taskbar + 1));
            assert_eq!(m.entries.len(), own_menu(Own::default()).entries.len(), "the entries stay where they are");
        }
        assert!(format!("\t{DESKTOP_KEY}").ends_with(&format!("+{}", DESKTOP_KEY_LETTER as char)));
        assert!(own_menu(Own { desktop: true, desktop_key: true }).entries.iter().any(|e| e.label.ends_with(DESKTOP_KEY)));
    }

    #[test]
    fn commands_survive_a_window_message() {
        for c in BarCommand::ALL {
            assert_eq!(BarCommand::from_code(c.code()), Some(c));
        }
        assert_eq!(BarCommand::from_code(99), None);
        // The codes scripts send (tools/dev/dialogcaps.ps1) stay what they were.
        assert_eq!((BarCommand::AdjustDateTime.code(), BarCommand::Settings.code()), (4, 5));
        assert_eq!(BarCommand::Find.code(), 6);
        assert_eq!((BarCommand::About.code(), BarCommand::SwitchToBevel95.code()), (13, 14));
    }

    #[test]
    fn size_menu_marks_the_current_size() {
        let m = size_menu(Scale::whole(2).with_way(crate::scale::Way::Nearest));
        let labels: Vec<&str> = m.entries.iter().map(|e| e.label.as_str()).collect();
        assert_eq!(
            labels,
            ["1", "1.25", "1.5", "1.75", "2", "2.25", "2.5", "2.75", "3", "3.25", "3.5", "3.75", "4"].map(|s| format!("{s}\u{d7}"))
        );
        let marked: Vec<usize> = (0..m.entries.len()).filter(|&i| m.entries[i].mark.is_some()).collect();
        assert_eq!(marked, [4]);
        assert_eq!(size_at(12), Some(Scale::whole(4)));
        assert_eq!(size_at(3), Some(Scale::from_quarters(7)));
        assert_eq!(size_at(13), None);
        // A quarter size in use has its bullet too.
        let m = size_menu(Scale::from_quarters(7));
        assert_eq!((0..m.entries.len()).filter(|&i| m.entries[i].mark.is_some()).collect::<Vec<_>>(), [3]);
        // The whole sizes in bold, the quarter sizes plain.
        let bold: Vec<usize> = (0..m.entries.len()).filter(|&i| m.entries[i].bold).collect();
        assert_eq!(bold, [0, 4, 8, 12]);
    }

    #[test]
    fn a_quarter_size_from_the_menu_is_kept_only_after_yes() {
        let (two, q7, q9) = (Scale::whole(2), Scale::from_quarters(7), Scale::from_quarters(9));
        // A whole size: saved at once, no question.
        let mut a = SizeAsking::default();
        assert_eq!(a.choose(Some(two), Scale::whole(3)), SizeChosen::Save(Scale::whole(3)));
        // Yes keeps it (and only then is it saved).
        let SizeChosen::Ask(s, t) = a.choose(Some(two), q7) else { panic!("a quarter size asks") };
        assert_eq!(s, q7);
        assert_eq!(a.answer(t, true), Some(SizeAnswer::Keep(q7)));
        assert_eq!(a.answer(t, false), None, "answered once");
        // No (or a closed question, which answers the same) goes back; nothing to save.
        let SizeChosen::Ask(_, t) = a.choose(Some(two), q7) else { panic!() };
        assert_eq!(a.answer(t, false), Some(SizeAnswer::Back(Some(two))));
        // From the automatic size: back to automatic.
        let SizeChosen::Ask(_, t) = a.choose(None, q7) else { panic!() };
        assert_eq!(a.answer(t, false), Some(SizeAnswer::Back(None)));
        // Asked again from another quarter size while open: the size before stays the first one,
        // and the first question's late answer does nothing.
        let SizeChosen::Ask(_, first) = a.choose(Some(two), q7) else { panic!() };
        let SizeChosen::Ask(s, second) = a.choose(Some(q7), q9) else { panic!("it asks every time") };
        assert_eq!(s, q9);
        assert_eq!(a.answer(first, false), None);
        assert_eq!(a.answer(second, false), Some(SizeAnswer::Back(Some(two))));
        // A kept quarter size, then another: asks again, back goes to the kept one.
        let SizeChosen::Ask(_, t) = a.choose(Some(q7), q9) else { panic!() };
        assert_eq!(a.answer(t, false), Some(SizeAnswer::Back(Some(q7))));
        // A size set another way while open (Bevel95 Settings, a whole size): the question is moot.
        let SizeChosen::Ask(_, t) = a.choose(Some(two), q7) else { panic!() };
        assert!(a.forget());
        assert_eq!(a.answer(t, true), None);
        let SizeChosen::Ask(_, t) = a.choose(Some(two), q7) else { panic!() };
        assert_eq!(a.choose(Some(q7), Scale::whole(1)), SizeChosen::Save(Scale::ONE));
        assert_eq!(a.answer(t, false), None);
    }

    #[test]
    fn colors_menu_marks_the_colors_in_use() {
        let m = colors_menu(Depth::TwoFiftySix);
        let labels: Vec<&str> = m.entries.iter().map(|e| e.label.as_str()).collect();
        assert_eq!(labels, ["16 Colors", "256 Colors", "True Color"]);
        let marked: Vec<bool> = m.entries.iter().map(|e| e.mark.is_some()).collect();
        assert_eq!(marked, [false, true, false]);
        assert_eq!(colors_at(0), Some(Depth::Sixteen));
        assert_eq!(colors_at(1), Some(Depth::TwoFiftySix));
        assert_eq!(colors_at(2), Some(Depth::TrueColor));
        assert_eq!(colors_at(3), None);
    }

    #[test]
    fn theme_menu_marks_the_current_look() {
        let m = theme_menu(Era::Win98);
        let labels: Vec<&str> = m.entries.iter().map(|e| e.label.as_str()).collect();
        assert_eq!(labels, ["Windows 95", "Windows 98", "Windows 2000"]);
        let marked: Vec<bool> = m.entries.iter().map(|e| e.mark.is_some()).collect();
        assert_eq!(marked, [false, true, false]);
        assert_eq!(theme_at(0), Some(Era::Win95));
        assert_eq!(theme_at(2), Some(Era::Win2000));
        assert_eq!(theme_at(3), None);
    }
}
