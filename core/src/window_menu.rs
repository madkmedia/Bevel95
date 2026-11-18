//! The menu a task button opens on a right-click: for one window, that window's own menu
//! (Restore, Move, Size, Minimize, Maximize, Close), as Windows 95, 98 and 2000 show it; for
//! a button holding several windows, the group menu of Windows XP (which grouped buttons
//! this way).

use crate::paint::{Menu, MenuEntry, MenuGlyph};
use crate::theme::Era;

/// What a window menu entry does. The platform layer carries it out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowCommand {
    Restore,
    Move,
    Size,
    Minimize,
    Maximize,
    Close,
}

/// What a window allows right now, which decides the grayed-out entries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WindowState {
    pub minimized: bool,
    pub maximized: bool,
    /// It has a sizing border.
    pub resizable: bool,
    pub has_minimize_box: bool,
    pub has_maximize_box: bool,
    /// Its class does not forbid closing (CS_NOCLOSE).
    pub closable: bool,
}

/// `None` is a separator. Labels and access keys as in Windows' own window menu (the
/// emulated Windows 95 and 98 show these underlined letters).
const ENTRIES: [Option<(&str, WindowCommand)>; 7] = [
    Some(("&Restore", WindowCommand::Restore)),
    Some(("&Move", WindowCommand::Move)),
    Some(("&Size", WindowCommand::Size)),
    Some(("Mi&nimize", WindowCommand::Minimize)),
    Some(("Ma&ximize", WindowCommand::Maximize)),
    None,
    Some(("&Close\tAlt+F4", WindowCommand::Close)),
];

/// The window menu for a window in `state`. Windows 98 and 2000 add little pictures, Windows
/// 95 has none. Which entries are grayed out follows Windows' own rule for the menu: Restore
/// only for a minimized or maximized window, Move not when maximized, Size only for a normal
/// resizable window, Minimize and Maximize when the window has that box and is not already
/// so. The default entry (bold) is Restore for a minimized window, else Close.
pub fn menu(era: Era, state: WindowState) -> Menu {
    let entries = ENTRIES
        .iter()
        .map(|e| {
            let Some((label, command)) = *e else {
                return MenuEntry::separator();
            };
            let mut entry = MenuEntry::item(label, None);
            entry.disabled = !allowed(command, state);
            entry.bold = command == default_command(state);
            if era != Era::Win95 {
                entry.glyph = match command {
                    WindowCommand::Restore => Some(MenuGlyph::Restore),
                    WindowCommand::Minimize => Some(MenuGlyph::Minimize),
                    WindowCommand::Maximize => Some(MenuGlyph::Maximize),
                    WindowCommand::Close => Some(MenuGlyph::Close),
                    WindowCommand::Move | WindowCommand::Size => None,
                };
            }
            entry
        })
        .collect();
    Menu::new(entries)
}

fn allowed(command: WindowCommand, s: WindowState) -> bool {
    let normal = !s.minimized && !s.maximized;
    match command {
        WindowCommand::Restore => !normal,
        WindowCommand::Move => !s.maximized,
        WindowCommand::Size => normal && s.resizable,
        WindowCommand::Minimize => s.has_minimize_box && !s.minimized,
        WindowCommand::Maximize => s.has_maximize_box && !s.maximized,
        WindowCommand::Close => s.closable,
    }
}

/// The system menu opened from a program's caption. Windows 2000 gives rows
/// containing its system bitmaps three more pixels than ordinary menu rows.
pub fn program_menu(era: Era, state: WindowState) -> Menu {
    Menu { system_bitmaps: era == Era::Win2000, ..menu(era, state) }
}

fn default_command(s: WindowState) -> WindowCommand {
    if s.minimized { WindowCommand::Restore } else { WindowCommand::Close }
}

/// What entry `index` of `menu()` does (`None` for a separator).
pub fn command_at(index: usize) -> Option<WindowCommand> {
    ENTRIES.get(index).copied().flatten().map(|(_, command)| command)
}

/// What a group menu entry does to all the windows of a button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupCommand {
    Cascade,
    TileHorizontally,
    TileVertically,
    Minimize,
    Close,
}

const GROUP_ENTRIES: [Option<(&str, GroupCommand)>; 7] = [
    Some(("&Cascade", GroupCommand::Cascade)),
    Some(("Tile &Horizontally", GroupCommand::TileHorizontally)),
    Some(("Tile &Vertically", GroupCommand::TileVertically)),
    None,
    Some(("Mi&nimize Group", GroupCommand::Minimize)),
    None,
    Some(("&Close Group", GroupCommand::Close)),
];

/// The menu of a button that holds several windows.
pub fn group_menu() -> Menu {
    let entries = GROUP_ENTRIES
        .iter()
        .map(|e| match e {
            Some((label, _)) => MenuEntry::item(label, None),
            None => MenuEntry::separator(),
        })
        .collect();
    Menu::new(entries)
}

/// What entry `index` of `group_menu()` does (`None` for a separator).
pub fn group_command_at(index: usize) -> Option<GroupCommand> {
    GROUP_ENTRIES.get(index).copied().flatten().map(|(_, command)| command)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::EntryKind;

    const NORMAL: WindowState = WindowState {
        minimized: false,
        maximized: false,
        resizable: true,
        has_minimize_box: true,
        has_maximize_box: true,
        closable: true,
    };

    fn grayed(m: &Menu) -> Vec<bool> {
        m.entries.iter().filter(|e| e.kind != EntryKind::Separator).map(|e| e.disabled).collect()
    }

    #[test]
    fn a_normal_window_cannot_be_restored() {
        let m = menu(Era::Win95, NORMAL);
        // Restore, Move, Size, Minimize, Maximize, Close
        assert_eq!(grayed(&m), [true, false, false, false, false, false]);
        assert!(m.entries[6].bold && !m.entries[0].bold);
        assert_eq!(m.entries[6].parts(), ("&Close", Some("Alt+F4")));
    }

    #[test]
    fn a_maximized_window_cannot_move_size_or_maximize() {
        let m = menu(Era::Win98, WindowState { maximized: true, ..NORMAL });
        assert_eq!(grayed(&m), [false, true, true, false, true, false]);
        assert!(m.entries[6].bold);
    }

    #[test]
    fn a_minimized_window_restores_by_default() {
        let m = menu(Era::Win2000, WindowState { minimized: true, ..NORMAL });
        assert_eq!(grayed(&m), [false, false, true, true, false, false]);
        assert!(m.entries[0].bold && !m.entries[6].bold);
    }

    #[test]
    fn only_windows_98_and_2000_have_pictures() {
        assert!(menu(Era::Win95, NORMAL).entries.iter().all(|e| e.glyph.is_none()));
        let glyphs: Vec<_> = menu(Era::Win98, NORMAL).entries.iter().map(|e| e.glyph).collect();
        assert_eq!(glyphs[0], Some(MenuGlyph::Restore));
        assert_eq!(glyphs[1], None);
        assert_eq!(glyphs[6], Some(MenuGlyph::Close));
    }

    #[test]
    fn entries_and_commands_line_up() {
        assert_eq!(command_at(0), Some(WindowCommand::Restore));
        assert_eq!(command_at(5), None);
        assert_eq!(command_at(6), Some(WindowCommand::Close));
        assert_eq!(group_command_at(4), Some(GroupCommand::Minimize));
        assert_eq!(group_command_at(3), None);
        assert_eq!(group_menu().entries.len(), GROUP_ENTRIES.len());
    }
}
