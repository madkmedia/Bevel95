//! Bevel95's own icon in Windows' notification area: what it says, what its clicks do and its
//! right-click menu. The icon exists only while Bevel95 is switched to Windows' taskbar
//! (Ctrl+Win+T), so a person who switched sees that Bevel95 still runs and how to come back
//! (the owner's idea, 2026-11-17: "a tray icon (only in real taskbar, not ours), so that people
//! know it's running"). The platform layer adds and removes it (`own_icon.rs`).

use crate::bar_menu::BarCommand;
use crate::fb::Rect;
use crate::paint::{Menu, MenuEntry};

/// The icon's tooltip: the name and the key that switches back (PROVISIONAL wording).
pub const TOOLTIP: &str = concat!(crate::product_name!(), " (Ctrl+Win+T switches back)");

/// What Windows' taskbar tells the icon's window about the mouse and the keyboard (the
/// platform layer reads Explorer's messages into these).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// A button went down or came up (the plain messages that come before the ones below).
    Press,
    /// A left click (NIN_SELECT, sent after the button comes up).
    Select,
    /// Enter or Space on the icon (NIN_KEYSELECT).
    KeySelect,
    /// The second press of a left double-click.
    DoubleClick,
    /// A right click, or the menu key on the icon (WM_CONTEXTMENU).
    ContextMenu,
    /// Anything else (the pointer moving over it, its pop-up opening or closing).
    Other,
}

/// What Bevel95 does about an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Response {
    /// Switch back to Bevel95's bar, as Ctrl+Win+T does.
    SwitchBack,
    /// Open the icon's menu (`menu`) at the pointer.
    OpenMenu,
    Nothing,
}

/// What an event on the icon does; `windows_taskbar`: Bevel95 is still switched to Windows'
/// taskbar. A left click switches back; so does a double-click, once: its first click already
/// switched (the icon is gone then) and Explorer's late messages for it change nothing.
pub fn respond(event: Event, windows_taskbar: bool) -> Response {
    if !windows_taskbar {
        return Response::Nothing;
    }
    match event {
        Event::Select | Event::KeySelect | Event::DoubleClick => Response::SwitchBack,
        Event::ContextMenu => Response::OpenMenu,
        Event::Press | Event::Other => Response::Nothing,
    }
}

/// The menu's entries and what each does (`None`: a separator). PROVISIONAL wording.
const ENTRIES: [Option<(&str, BarCommand)>; 5] = [
    Some((concat!("Switch to ", crate::product_name!()), BarCommand::SwitchToBevel95)),
    Some(("Bevel95 Settings", BarCommand::Settings)),
    Some((concat!("About ", crate::product_name!()), BarCommand::About)),
    None,
    Some(("Exit", BarCommand::Exit)),
];

/// The icon's right-click menu: Switch to Bevel95 (the default, in bold: what a click does),
/// Bevel95 Settings, About Bevel95, a line, Exit. Drawn in the look in use, as all of
/// Bevel95's menus.
pub fn menu() -> Menu {
    let entries = ENTRIES
        .iter()
        .map(|e| match e {
            Some((label, command)) => {
                let mut entry = MenuEntry::item(label, None);
                entry.bold = *command == BarCommand::SwitchToBevel95;
                entry
            }
            None => MenuEntry::separator(),
        })
        .collect();
    Menu::new(entries)
}

/// What entry `index` of `menu()` does (`None` for a separator).
pub fn command_at(index: usize) -> Option<BarCommand> {
    ENTRIES.get(index).copied().flatten().map(|(_, command)| command)
}

/// Where the menu (`w`×`h`) opens for a click at (x, y): upward from the pointer, its bottom
/// edge there (as programs open their notification icons' menus on Windows' taskbar), to the
/// right of it unless that runs off `screen` (then to the left), and always on `screen`.
pub fn place_menu(x: i32, y: i32, w: i32, h: i32, screen: Rect) -> (i32, i32) {
    let left = if x + w > screen.right() { x - w } else { x };
    let top = y - h;
    (left.min(screen.right() - w).max(screen.x), top.min(screen.bottom() - h).max(screen.y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::EntryKind;

    #[test]
    fn the_tooltip_names_bevel95_and_the_key() {
        assert_eq!(TOOLTIP, "Bevel95 (Ctrl+Win+T switches back)");
        assert!(TOOLTIP.starts_with(crate::about::NAME));
        // Windows cuts a notification icon's tooltip at 127 characters.
        assert!(TOOLTIP.encode_utf16().count() < 128);
    }

    #[test]
    fn a_click_or_a_double_click_switches_back_once() {
        use Event::*;
        assert_eq!(respond(Select, true), Response::SwitchBack);
        assert_eq!(respond(KeySelect, true), Response::SwitchBack);
        assert_eq!(respond(ContextMenu, true), Response::OpenMenu);
        assert_eq!(respond(Press, true), Response::Nothing, "the button's own messages do nothing by themselves");
        assert_eq!(respond(Other, true), Response::Nothing);
        // A double-click: the first click switches; what follows arrives with Bevel95's bar
        // back and changes nothing (not there and back again).
        let mut windows_taskbar = true;
        let mut switches = 0;
        for event in [Press, Press, Select, Press, DoubleClick, Press] {
            if respond(event, windows_taskbar) == Response::SwitchBack {
                switches += 1;
                windows_taskbar = false;
            }
        }
        assert_eq!(switches, 1);
        // A double-click alone (Explorer did not send the first click's Select) switches too.
        assert_eq!(respond(DoubleClick, true), Response::SwitchBack);
        for event in [Press, Select, KeySelect, DoubleClick, ContextMenu, Other] {
            assert_eq!(respond(event, false), Response::Nothing, "{event:?} with Bevel95's bar showing");
        }
    }

    #[test]
    fn the_menu_has_switch_settings_about_and_exit() {
        let m = menu();
        let labels: Vec<String> = m
            .entries
            .iter()
            .map(|e| if e.kind == EntryKind::Separator { "-".into() } else { e.label.clone() })
            .collect();
        assert_eq!(labels, ["Switch to Bevel95", "Bevel95 Settings", "About Bevel95", "-", "Exit"]);
        let bold: Vec<bool> = m.entries.iter().map(|e| e.bold).collect();
        assert_eq!(bold, [true, false, false, false, false], "the default entry is bold");
        assert!(m.entries.iter().all(|e| e.mark.is_none() && !e.disabled && e.kind != EntryKind::Submenu));
        use BarCommand::*;
        let commands: Vec<Option<BarCommand>> = (0..6).map(command_at).collect();
        assert_eq!(commands, [Some(SwitchToBevel95), Some(Settings), Some(About), None, Some(Exit), None]);
        // The same entries as the bar's Bevel95 submenu, worded the same.
        let own = crate::bar_menu::own_menu(crate::bar_menu::Own::default());
        for label in ["Bevel95 Settings", "About Bevel95", "Exit"] {
            assert!(own.entries.iter().any(|e| e.label == label), "{label}");
        }
    }

    #[test]
    fn the_menu_opens_upward_from_the_pointer_and_stays_on_the_screen() {
        let screen = Rect::new(0, 0, 1920, 1080);
        // At the pointer on Windows' taskbar: above it, to its right.
        assert_eq!(place_menu(1700, 1060, 150, 90, screen), (1700, 970));
        // Near the right edge: to the left of the pointer.
        assert_eq!(place_menu(1850, 1060, 150, 90, screen), (1700, 970));
        // Near the top (a taskbar at the top of the screen): kept on the screen.
        assert_eq!(place_menu(100, 30, 150, 90, screen), (100, 0));
        // A second screen to the left of the main one.
        let left = Rect::new(-1280, 0, 1280, 1024);
        assert_eq!(place_menu(-1300, 1000, 150, 90, left), (-1280, 910));
    }
}
