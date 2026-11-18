//! The classic Start menu icons, pixel for pixel: the Start menu's own entries (32×32), the
//! folder icon of the Programs menus and the entries of Settings and Find (16×16), for
//! Windows 95, 98 and 2000.
//!
//! Cut from the Reference PCs' True Color captures or read from their Windows' own files (first
//! from GUIdebook's screenshots and the v86 emulator; found identical on 2026-11-18, except the
//! few that `art/src/start.rs` names). The Settings and Find icons come from the reference PCs
//! (True Color), each entry captured selected and not (`tools/dev/cutmenu.ps1`).
//!
//! The pictures themselves are Windows' and kept in bevel-art (`art/src/start.rs`); this
//! module chooses and draws them.

use crate::art::Look;
use crate::fb::Icon;
use crate::start::{StartEntry, Target};
use crate::theme::{Era, rgb};

/// A small picture: rows top to bottom, one character per pixel, "." see-through, other
/// characters colors from `palette` (0xRRGGBB).
#[derive(Debug)]
pub struct ClassicIcon {
    pub size: i32,
    pub palette: &'static [(u8, u32)],
    pub rows: &'static [&'static str],
}

pub(crate) const WARNING: ClassicIcon = ClassicIcon::art("message-box/warning", crate::art::ANY);

impl ClassicIcon {
    /// The picture `what` of `look`, from bevel-art (Windows' pictures).
    pub(crate) const fn art(what: &str, look: crate::art::Look) -> ClassicIcon {
        let r = crate::art::rows(what, look);
        ClassicIcon { size: r.w, palette: r.palette, rows: r.rows }
    }

    pub fn icon(&self) -> Icon {
        let color = |b: u8| self.palette.iter().find(|(c, _)| *c == b).map_or(0, |&(_, hex)| rgb(hex));
        let pixels = self.rows.iter().flat_map(|row| row.bytes()).map(color).collect();
        Icon { width: self.size, height: self.size, pixels }
    }
}

/// The Start menu's icon for `entry`, as `era` drew it.
pub fn start_icon(era: Era, entry: StartEntry) -> Option<Icon> {
    use StartEntry::*;
    let icon = match (era, entry) {
        (Era::Win95, Programs) => &WIN95_PROGRAMS,
        (Era::Win95, Documents) => &WIN95_DOCUMENTS,
        (Era::Win95, Settings) => &WIN95_SETTINGS,
        (Era::Win95, Find) => &WIN95_FIND,
        (Era::Win95, Help) => &WIN95_HELP,
        (Era::Win95, Run) => &WIN95_RUN,
        (Era::Win95, ShutDown) => &WIN95_SHUT_DOWN,
        (Era::Win95, WindowsUpdate | Favorites | LogOff) => return None,
        // Windows 2000 shows Favorites only when its Start Menu Settings say so.
        (Era::Win2000, Favorites) => &WIN2000_FAVORITES,
        (Era::Win98, WindowsUpdate) => &WIN98_WINDOWS_UPDATE,
        (Era::Win98, Programs) => &WIN98_PROGRAMS,
        (Era::Win98, Favorites) => &WIN98_FAVORITES,
        (Era::Win98, Documents) => &WIN98_DOCUMENTS,
        (Era::Win98, Settings) => &WIN98_SETTINGS,
        (Era::Win98, Find) => &WIN98_FIND,
        (Era::Win98, Help) => &WIN98_HELP,
        (Era::Win98, Run) => &WIN98_RUN,
        (Era::Win98, LogOff) => &WIN98_LOG_OFF,
        (Era::Win98, ShutDown) => &WIN98_SHUT_DOWN,
        (Era::Win2000, WindowsUpdate) => &WIN2000_WINDOWS_UPDATE,
        (Era::Win2000, Programs) => &WIN2000_PROGRAMS,
        (Era::Win2000, Documents) => &WIN2000_DOCUMENTS,
        (Era::Win2000, Settings) => &WIN2000_SETTINGS,
        (Era::Win2000, Find) => &WIN2000_SEARCH,
        (Era::Win2000, Help) => &WIN2000_HELP,
        (Era::Win2000, Run) => &WIN2000_RUN,
        (Era::Win2000, LogOff) => &WIN2000_LOG_OFF,
        (Era::Win2000, ShutDown) => &WIN2000_SHUT_DOWN,
    };
    Some(icon.icon())
}

/// The icon of a folder in the Programs menus ("program group").
pub fn program_group(era: Era) -> Icon {
    match era {
        Era::Win95 => WIN95_PROGRAM_GROUP.icon(),
        Era::Win98 => WIN98_PROGRAM_GROUP.icon(),
        Era::Win2000 => WIN2000_PROGRAM_GROUP.icon(),
    }
}

/// The icon of a folder of the Programs menu in a tree (the "Remove Shortcuts/Folders" window
/// of Taskbar Properties): Windows 95's is its menu's; Windows 98's and 2000's trees both show
/// a 16-color one that their menus do not.
pub fn tree_group(era: Era) -> Icon {
    match era {
        Era::Win95 => WIN95_PROGRAM_GROUP.icon(),
        Era::Win98 | Era::Win2000 => WIN98_TREE_GROUP.icon(),
    }
}

/// The icon of the "Start Menu" row at the top of Windows' tree of the Start menu: Windows
/// 95's yellow folder, Windows 98's and 2000's gray one.
pub fn tree_start_menu(era: Era) -> Icon {
    match era {
        Era::Win95 => WIN95_TREE_START_MENU.icon(),
        Era::Win98 | Era::Win2000 => WIN98_TREE_START_MENU.icon(),
    }
}

/// The arrow over the lower left corner of a shortcut's picture (the rest is see-through).
pub fn shortcut_arrow() -> Icon {
    SHORTCUT_ARROW.icon()
}

/// The icon of a folder in the Favorites menu (Windows 98's plain yellow folder).
pub fn favorites_folder() -> Icon {
    WIN98_FOLDER.icon()
}

/// The icon of "My Documents" at the top of the Documents menu (Windows 98 and 2000; cut from
/// the reference PCs' menus, which draw it from the shell's own icon list: a few colors
/// differ from the window's picture in `window_icons`).
pub fn my_documents(era: Era) -> Option<Icon> {
    match era {
        Era::Win95 => None,
        Era::Win98 => Some(WIN98_MY_DOCUMENTS_MENU.icon()),
        Era::Win2000 => Some(WIN2000_MY_DOCUMENTS_MENU.icon()),
    }
}

/// The icon of a Settings or Find entry, as `era` drew it (cut from the reference PCs' menus;
/// Control Panel's is its window's picture there too). `None` for an entry `era` did not have.
pub fn target_icon(era: Era, target: Target) -> Option<Icon> {
    use Target::*;
    let icon = match (era, target) {
        (_, ControlPanel) => return Some(crate::window_icons::icon(era, crate::window_icons::ClassicWindow::ControlPanel)),
        (Era::Win95, Printers) => &WIN95_PRINTERS,
        (Era::Win98, Printers) => &WIN98_PRINTERS,
        (Era::Win2000, Printers) => &WIN2000_PRINTERS,
        (Era::Win95 | Era::Win98, BarSettings) => &WIN95_TASKBAR,
        (Era::Win2000, BarSettings) => &WIN2000_TASKBAR,
        (Era::Win98, FolderOptions) => &WIN98_FOLDER_OPTIONS,
        (Era::Win98, WindowsUpdate) => &WIN98_WINDOWS_UPDATE_MENU,
        (Era::Win2000, Network) => &WIN2000_NETWORK,
        (Era::Win95 | Era::Win98, FindFiles) => &WIN95_FIND_FILES,
        (Era::Win2000, FindFiles) => &WIN2000_FIND_FILES,
        (Era::Win98, Internet) => &WIN98_FIND_INTERNET,
        (Era::Win2000, Internet) => &WIN2000_FIND_INTERNET,
        // Bevel95's own entry: the look's small Programs picture.
        (_, FindPrograms) => return start_icon_small(era, StartEntry::Programs),
        _ => return None,
    };
    Some(icon.icon())
}

/// The volume control's speaker in the notification area, with the red "no" sign when muted.
/// Windows 2000's, from the reference PC (identical to GUIdebook's screenshot S11); the
/// reference PCs' Windows 95 (normal and muted) and 98 (normal) show the very same picture.
pub fn speaker(muted: bool) -> Icon {
    if muted { WIN2000_SPEAKER_MUTED.icon() } else { WIN2000_SPEAKER.icon() }
}

static WIN2000_SPEAKER: ClassicIcon = ClassicIcon::art("tray/speaker", Look::Win2000);

static WIN2000_SPEAKER_MUTED: ClassicIcon = ClassicIcon::art("tray/speaker-muted", Look::Win2000);
pub static WIN95_PROGRAMS: ClassicIcon = ClassicIcon::art("start/programs", Look::Win95);

pub static WIN95_DOCUMENTS: ClassicIcon = ClassicIcon::art("start/documents", Look::Win95);

pub static WIN95_SETTINGS: ClassicIcon = ClassicIcon::art("start/settings", Look::Win95);

pub static WIN95_FIND: ClassicIcon = ClassicIcon::art("start/find", Look::Win95);

pub static WIN95_HELP: ClassicIcon = ClassicIcon::art("start/help", Look::Win95);

pub static WIN95_RUN: ClassicIcon = ClassicIcon::art("start/run", Look::Win95);

pub static WIN95_SHUT_DOWN: ClassicIcon = ClassicIcon::art("start/shut-down", Look::Win95);

pub static WIN98_TREE_START_MENU: ClassicIcon = ClassicIcon::art("start/tree-start-menu", Look::Win98);

pub static SHORTCUT_ARROW: ClassicIcon = ClassicIcon::art("start/shortcut-arrow", crate::art::ANY);

pub static WIN95_TREE_START_MENU: ClassicIcon = ClassicIcon::art("start/tree-start-menu", Look::Win95);

pub static WIN98_TREE_GROUP: ClassicIcon = ClassicIcon::art("start/tree-group", Look::Win98);

pub static WIN95_PROGRAM_GROUP: ClassicIcon = ClassicIcon::art("start/program-group", Look::Win95);

pub static WIN2000_WINDOWS_UPDATE: ClassicIcon = ClassicIcon::art("start/windows-update", Look::Win2000);

pub static WIN2000_PROGRAMS: ClassicIcon = ClassicIcon::art("start/programs", Look::Win2000);

pub static WIN2000_DOCUMENTS: ClassicIcon = ClassicIcon::art("start/documents", Look::Win2000);

pub static WIN2000_SETTINGS: ClassicIcon = ClassicIcon::art("start/settings", Look::Win2000);

pub static WIN2000_SEARCH: ClassicIcon = ClassicIcon::art("start/search", Look::Win2000);

pub static WIN2000_HELP: ClassicIcon = ClassicIcon::art("start/help", Look::Win2000);

pub static WIN2000_RUN: ClassicIcon = ClassicIcon::art("start/run", Look::Win2000);

pub static WIN2000_LOG_OFF: ClassicIcon = ClassicIcon::art("start/log-off", Look::Win2000);

pub static WIN2000_FAVORITES: ClassicIcon = ClassicIcon::art("start/favorites", Look::Win2000);

pub static WIN98_WINDOWS_UPDATE: ClassicIcon = ClassicIcon::art("start/windows-update", Look::Win98);

pub static WIN2000_SHUT_DOWN: ClassicIcon = ClassicIcon::art("start/shut-down", Look::Win2000);

pub static WIN2000_PROGRAM_GROUP: ClassicIcon = ClassicIcon::art("start/program-group", Look::Win2000);

pub static WIN95_RUN_DIALOG: ClassicIcon = ClassicIcon::art("dialog/run", Look::Win95);

pub static WIN2000_RUN_DIALOG: ClassicIcon = ClassicIcon::art("dialog/run", Look::Win2000);

pub static WIN95_SHUT_DOWN_DIALOG: ClassicIcon = ClassicIcon::art("dialog/shut-down", Look::Win95);

pub static WIN2000_SHUT_DOWN_DIALOG: ClassicIcon = ClassicIcon::art("dialog/shut-down", Look::Win2000);

pub static WIN2000_LOG_OFF_DIALOG: ClassicIcon = ClassicIcon::art("dialog/log-off", Look::Win2000);

pub static WIN98_PROGRAMS: ClassicIcon = ClassicIcon::art("start/programs", Look::Win98);

pub static WIN98_FAVORITES: ClassicIcon = ClassicIcon::art("start/favorites", Look::Win98);

pub static WIN98_DOCUMENTS: ClassicIcon = ClassicIcon::art("start/documents", Look::Win98);

pub static WIN98_SETTINGS: ClassicIcon = ClassicIcon::art("start/settings", Look::Win98);

pub static WIN98_FIND: ClassicIcon = ClassicIcon::art("start/find", Look::Win98);

pub static WIN98_HELP: ClassicIcon = ClassicIcon::art("start/help", Look::Win98);

pub static WIN98_RUN: ClassicIcon = ClassicIcon::art("start/run", Look::Win98);

pub static WIN98_LOG_OFF: ClassicIcon = ClassicIcon::art("start/log-off", Look::Win98);

pub static WIN98_SHUT_DOWN: ClassicIcon = ClassicIcon::art("start/shut-down", Look::Win98);

pub static WIN98_PROGRAM_GROUP: ClassicIcon = ClassicIcon::art("start/program-group", Look::Win98);

pub static WIN98_FOLDER: ClassicIcon = ClassicIcon::art("start/folder", Look::Win98);

pub static WIN98_SHUT_DOWN_DIALOG: ClassicIcon = ClassicIcon::art("dialog/shut-down", Look::Win98);

/// The Start menu's 16×16 icon for entry ("Show small icons in Start menu"), as era drew
/// it: first cut from the v86 emulator's Start menus with small icons; since 2026-11-18 every one
/// is the Reference PCs' (`art/src/start.rs` names each one's source): 95's and 2000's cut from
/// their Start menus with small icons, 98's the 16-color images of the same icons in its files
/// (the emulator ran it in 16 colors; in True Color 98 draws richer ones, not done yet).
pub fn start_icon_small(era: Era, entry: StartEntry) -> Option<Icon> {
    use StartEntry::*;
    let icon = match (era, entry) {
        (Era::Win95, Programs) => &WIN95_SMALL_PROGRAMS,
        (Era::Win95, Documents) => &WIN95_SMALL_DOCUMENTS,
        (Era::Win95, Settings) => &WIN95_SMALL_SETTINGS,
        (Era::Win95, Find) => &WIN95_SMALL_FIND,
        (Era::Win95, Help) => &WIN95_SMALL_HELP,
        (Era::Win95, Run) => &WIN95_SMALL_RUN,
        (Era::Win95, ShutDown) => &WIN95_SMALL_SHUT_DOWN,
        (Era::Win95, WindowsUpdate | Favorites | LogOff) => return None,
        (Era::Win2000, WindowsUpdate) => &WIN2000_SMALL_WINDOWS_UPDATE,
        (Era::Win2000, Programs) => &WIN2000_SMALL_PROGRAMS,
        (Era::Win2000, Favorites) => &WIN2000_SMALL_FAVORITES,
        (Era::Win2000, Documents) => &WIN2000_SMALL_DOCUMENTS,
        (Era::Win2000, Settings) => &WIN2000_SMALL_SETTINGS,
        (Era::Win2000, Find) => &WIN2000_SMALL_SEARCH,
        (Era::Win2000, Help) => &WIN2000_SMALL_HELP,
        (Era::Win2000, Run) => &WIN2000_SMALL_RUN,
        (Era::Win2000, LogOff) => &WIN2000_SMALL_LOG_OFF,
        (Era::Win2000, ShutDown) => &WIN2000_SMALL_SHUT_DOWN,
        (_, WindowsUpdate) => &WIN98_SMALL_WINDOWS_UPDATE,
        (_, Programs) => &WIN98_SMALL_PROGRAMS,
        (_, Favorites) => &WIN98_SMALL_FAVORITES,
        (_, Documents) => &WIN98_SMALL_DOCUMENTS,
        (_, Settings) => &WIN98_SMALL_SETTINGS,
        (_, Find) => &WIN98_SMALL_FIND,
        (_, Help) => &WIN98_SMALL_HELP,
        (_, Run) => &WIN98_SMALL_RUN,
        (_, LogOff) => &WIN98_SMALL_LOG_OFF,
        (_, ShutDown) => &WIN98_SMALL_SHUT_DOWN,
    };
    Some(icon.icon())
}

pub static WIN95_SMALL_PROGRAMS: ClassicIcon = ClassicIcon::art("start/small-programs", Look::Win95);

pub static WIN95_SMALL_DOCUMENTS: ClassicIcon = ClassicIcon::art("start/small-documents", Look::Win95);

pub static WIN95_SMALL_SETTINGS: ClassicIcon = ClassicIcon::art("start/small-settings", Look::Win95);

pub static WIN95_SMALL_FIND: ClassicIcon = ClassicIcon::art("start/small-find", Look::Win95);

pub static WIN95_SMALL_HELP: ClassicIcon = ClassicIcon::art("start/small-help", Look::Win95);

pub static WIN95_SMALL_RUN: ClassicIcon = ClassicIcon::art("start/small-run", Look::Win95);

pub static WIN95_SMALL_SHUT_DOWN: ClassicIcon = ClassicIcon::art("start/small-shut-down", Look::Win95);

pub static WIN98_SMALL_WINDOWS_UPDATE: ClassicIcon = ClassicIcon::art("start/small-windows-update", Look::Win98);

pub static WIN98_SMALL_PROGRAMS: ClassicIcon = ClassicIcon::art("start/small-programs", Look::Win98);

pub static WIN98_SMALL_FAVORITES: ClassicIcon = ClassicIcon::art("start/small-favorites", Look::Win98);

pub static WIN98_SMALL_DOCUMENTS: ClassicIcon = ClassicIcon::art("start/small-documents", Look::Win98);

pub static WIN98_SMALL_SETTINGS: ClassicIcon = ClassicIcon::art("start/small-settings", Look::Win98);

pub static WIN98_SMALL_FIND: ClassicIcon = ClassicIcon::art("start/small-find", Look::Win98);

pub static WIN98_SMALL_HELP: ClassicIcon = ClassicIcon::art("start/small-help", Look::Win98);

pub static WIN98_SMALL_RUN: ClassicIcon = ClassicIcon::art("start/small-run", Look::Win98);

pub static WIN98_SMALL_LOG_OFF: ClassicIcon = ClassicIcon::art("start/small-log-off", Look::Win98);

pub static WIN98_SMALL_SHUT_DOWN: ClassicIcon = ClassicIcon::art("start/small-shut-down", Look::Win98);


pub static WIN2000_SMALL_WINDOWS_UPDATE: ClassicIcon = ClassicIcon::art("start/small-windows-update", Look::Win2000);

pub static WIN2000_SMALL_PROGRAMS: ClassicIcon = ClassicIcon::art("start/small-programs", Look::Win2000);

pub static WIN2000_SMALL_FAVORITES: ClassicIcon = ClassicIcon::art("start/small-favorites", Look::Win2000);

pub static WIN2000_SMALL_DOCUMENTS: ClassicIcon = ClassicIcon::art("start/small-documents", Look::Win2000);

pub static WIN2000_SMALL_LOG_OFF: ClassicIcon = ClassicIcon::art("start/small-log-off", Look::Win2000);

pub static WIN2000_SMALL_SETTINGS: ClassicIcon = ClassicIcon::art("start/small-settings", Look::Win2000);

pub static WIN2000_SMALL_SEARCH: ClassicIcon = ClassicIcon::art("start/small-search", Look::Win2000);

pub static WIN2000_SMALL_HELP: ClassicIcon = ClassicIcon::art("start/small-help", Look::Win2000);

pub static WIN2000_SMALL_RUN: ClassicIcon = ClassicIcon::art("start/small-run", Look::Win2000);

pub static WIN2000_SMALL_SHUT_DOWN: ClassicIcon = ClassicIcon::art("start/small-shut-down", Look::Win2000);

static WIN95_PRINTERS: ClassicIcon = ClassicIcon::art("start/printers", Look::Win95);

static WIN95_TASKBAR: ClassicIcon = ClassicIcon::art("start/taskbar", Look::Win95);

static WIN95_FIND_FILES: ClassicIcon = ClassicIcon::art("start/find-files", Look::Win95);

static WIN98_PRINTERS: ClassicIcon = ClassicIcon::art("start/printers", Look::Win98);

static WIN98_FOLDER_OPTIONS: ClassicIcon = ClassicIcon::art("start/folder-options", Look::Win98);

static WIN98_WINDOWS_UPDATE_MENU: ClassicIcon = ClassicIcon::art("start/windows-update-menu", Look::Win98);

static WIN98_FIND_INTERNET: ClassicIcon = ClassicIcon::art("start/find-internet", Look::Win98);

static WIN2000_NETWORK: ClassicIcon = ClassicIcon::art("start/network", Look::Win2000);

static WIN2000_PRINTERS: ClassicIcon = ClassicIcon::art("start/printers", Look::Win2000);

static WIN2000_TASKBAR: ClassicIcon = ClassicIcon::art("start/taskbar", Look::Win2000);

static WIN2000_FIND_FILES: ClassicIcon = ClassicIcon::art("start/find-files", Look::Win2000);

static WIN2000_FIND_INTERNET: ClassicIcon = ClassicIcon::art("start/find-internet", Look::Win2000);

static WIN98_MY_DOCUMENTS_MENU: ClassicIcon = ClassicIcon::art("start/my-documents-menu", Look::Win98);
static WIN2000_MY_DOCUMENTS_MENU: ClassicIcon = ClassicIcon::art("start/my-documents-menu", Look::Win2000);

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [&ClassicIcon; 76] = [
        &WIN2000_SMALL_FAVORITES,
        &WIN2000_SMALL_LOG_OFF,
        &WIN2000_FAVORITES,
        &WIN98_WINDOWS_UPDATE,
        &WIN95_PRINTERS,
        &WIN95_TASKBAR,
        &WIN95_FIND_FILES,
        &WIN98_PRINTERS,
        &WIN98_FOLDER_OPTIONS,
        &WIN98_WINDOWS_UPDATE_MENU,
        &WIN98_FIND_INTERNET,
        &WIN2000_NETWORK,
        &WIN2000_PRINTERS,
        &WIN2000_TASKBAR,
        &WIN2000_FIND_FILES,
        &WIN2000_FIND_INTERNET,
        &WIN2000_SMALL_WINDOWS_UPDATE,
        &WIN2000_SMALL_PROGRAMS,
        &WIN2000_SMALL_DOCUMENTS,
        &WIN2000_SMALL_SETTINGS,
        &WIN2000_SMALL_SEARCH,
        &WIN2000_SMALL_HELP,
        &WIN2000_SMALL_RUN,
        &WIN2000_SMALL_SHUT_DOWN,
        &WIN95_SMALL_PROGRAMS,
        &WIN95_SMALL_DOCUMENTS,
        &WIN95_SMALL_SETTINGS,
        &WIN95_SMALL_FIND,
        &WIN95_SMALL_HELP,
        &WIN95_SMALL_RUN,
        &WIN95_SMALL_SHUT_DOWN,
        &WIN98_SMALL_WINDOWS_UPDATE,
        &WIN98_SMALL_PROGRAMS,
        &WIN98_SMALL_FAVORITES,
        &WIN98_SMALL_DOCUMENTS,
        &WIN98_SMALL_SETTINGS,
        &WIN98_SMALL_FIND,
        &WIN98_SMALL_HELP,
        &WIN98_SMALL_RUN,
        &WIN98_SMALL_LOG_OFF,
        &WIN98_SMALL_SHUT_DOWN,
        &WIN95_PROGRAMS,
        &WIN95_DOCUMENTS,
        &WIN95_SETTINGS,
        &WIN95_FIND,
        &WIN95_HELP,
        &WIN95_RUN,
        &WIN95_SHUT_DOWN,
        &WIN95_PROGRAM_GROUP,
        &WIN98_PROGRAMS,
        &WIN98_FAVORITES,
        &WIN98_DOCUMENTS,
        &WIN98_SETTINGS,
        &WIN98_FIND,
        &WIN98_HELP,
        &WIN98_RUN,
        &WIN98_LOG_OFF,
        &WIN98_SHUT_DOWN,
        &WIN98_PROGRAM_GROUP,
        &WIN98_FOLDER,
        &WIN2000_WINDOWS_UPDATE,
        &WIN2000_PROGRAMS,
        &WIN2000_DOCUMENTS,
        &WIN2000_SETTINGS,
        &WIN2000_SEARCH,
        &WIN2000_HELP,
        &WIN2000_RUN,
        &WIN2000_LOG_OFF,
        &WIN2000_SHUT_DOWN,
        &WIN2000_PROGRAM_GROUP,
        &WIN95_RUN_DIALOG,
        &WIN95_SHUT_DOWN_DIALOG,
        &WIN98_SHUT_DOWN_DIALOG,
        &WIN2000_RUN_DIALOG,
        &WIN2000_SHUT_DOWN_DIALOG,
        &WIN2000_LOG_OFF_DIALOG,
    ];

    #[test]
    fn icons_are_square_and_use_their_palette() {
        for icon in ALL {
            assert_eq!(icon.rows.len() as i32, icon.size);
            for row in icon.rows {
                assert_eq!(row.len() as i32, icon.size);
                assert!(row.bytes().all(|b| b == b'.' || icon.palette.iter().any(|(c, _)| *c == b)), "{row}");
            }
        }
    }

    #[test]
    fn every_start_entry_has_an_icon_in_its_era() {
        use StartEntry::*;
        for era in Era::ALL {
            for entry in crate::start::entries(era, crate::config::StartOptions { favorites: true, log_off: true }).iter().flatten() {
                assert!(start_icon(era, *entry).is_some(), "{era:?} {entry:?}");
                assert!(start_icon_small(era, *entry).is_some(), "small {era:?} {entry:?}");
            }
        }
        assert!(start_icon(Era::Win95, LogOff).is_none());

    }
}
