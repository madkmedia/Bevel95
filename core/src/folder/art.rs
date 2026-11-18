//! The pictures of a Windows 95 folder window's own items and title bars: each kind of drive, Control
//! Panel and Printers as My Computer showed them, 32×32 and 16×16, pixel for pixel. They are the
//! 16-color icons in the Reference PC's shell32.dll (group ids 6 to 13, 36 and 38, dumped by
//! `tools/dev/icons.ps1`): Windows 95 draws those files' colors exactly (its My Computer window
//! matches them in every pixel: `win95-truecolor-folder-mycomputer`). Windows 98's and 2000's are
//! cut from their Reference PCs (`explorer_art`).
//!
//! The pictures themselves are Windows' and kept in bevel-art (`art/src/folder.rs`); this
//! module chooses and draws them.

use crate::art::Look;
use super::explorer_art;
use crate::fb::Icon;
use crate::icons::ClassicIcon;
use crate::theme::Era;
use crate::window_icons::{self, ClassicWindow};

/// What kind of drive a drive is (Windows' drive types, and a floppy's size).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DriveKind {
    /// 3½-inch floppy (A: or B:).
    Floppy,
    /// 5¼-inch floppy.
    Floppy525,
    Removable,
    Fixed,
    Network,
    CdRom,
    RamDisk,
}

/// What a folder window's title bar and task button show (16×16), and the 98 and 2000 address
/// bar and its list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Small {
    MyComputer,
    /// An ordinary folder: the open folder.
    Folder,
    Drive(DriveKind),
    Desktop,
    MyDocuments,
    RecycleBin,
    ControlPanel,
    Printers,
    /// A closed folder (the address bar's list).
    ClosedFolder,
}

/// The name of a drive's pictures in the 98 and 2000 sheets (`explorer_art`).
fn drive_word(kind: DriveKind) -> &'static str {
    match kind {
        DriveKind::Floppy => "floppy",
        DriveKind::Floppy525 => "floppy525",
        DriveKind::Removable => "removable",
        DriveKind::Fixed => "harddisk",
        DriveKind::Network => "network",
        DriveKind::CdRom => "cdrom",
        DriveKind::RamDisk => "ramdisk",
    }
}

/// A drive's 32×32 picture in `era` (`None`: no picture of the look's own).
pub fn drive(era: Era, kind: DriveKind) -> Option<Icon> {
    match era {
        Era::Win95 => Some(drive_icon(kind, false).icon()),
        _ => explorer_art::picture(era, &format!("d_{}", drive_word(kind))),
    }
}

/// A drive's 32×32 picture selected, where the Reference PC showed one (Windows 98's hard disk);
/// `None`: the plain picture averaged with the selection color.
pub fn drive_selected(era: Era, kind: DriveKind) -> Option<Icon> {
    match era {
        Era::Win95 => None,
        _ => explorer_art::picture(era, &format!("d_{}_sel", drive_word(kind))),
    }
}

/// Control Panel's and Printers' 32×32 pictures in My Computer.
pub fn control_panel(era: Era) -> Option<Icon> {
    match era {
        Era::Win95 => Some(W95_CONTROL_PANEL_32.icon()),
        _ => explorer_art::picture(era, "d_controlpanel"),
    }
}

pub fn printers(era: Era) -> Option<Icon> {
    match era {
        Era::Win95 => Some(W95_PRINTERS_32.icon()),
        _ => explorer_art::picture(era, "d_printers"),
    }
}

/// The 16×16 picture of a folder window's title bar and task button (and in 98 and 2000 of the
/// address bar and its list).
pub fn small(era: Era, what: Small) -> Icon {
    let sheet = |name: &str, or: ClassicWindow| explorer_art::picture(era, name).unwrap_or_else(|| window_icons::icon(era, or));
    match (era, what) {
        (_, Small::MyComputer) => window_icons::icon(era, ClassicWindow::MyComputer),
        (_, Small::Folder) => window_icons::icon(era, ClassicWindow::Folder),
        (_, Small::MyDocuments) => window_icons::icon(era, ClassicWindow::MyDocuments),
        (_, Small::RecycleBin) => window_icons::icon(era, ClassicWindow::RecycleBin),
        (Era::Win95, Small::Drive(kind)) => drive_icon(kind, true).icon(),
        (Era::Win95, Small::ControlPanel) => window_icons::icon(era, ClassicWindow::ControlPanel),
        (Era::Win95, _) => window_icons::icon(era, ClassicWindow::Folder),
        (_, Small::Drive(kind)) => sheet(&format!("s_{}", drive_word(kind)), ClassicWindow::Folder),
        (_, Small::Desktop) => sheet("s_desktop", ClassicWindow::Folder),
        (_, Small::ControlPanel) => sheet("s_controlpanel", ClassicWindow::ControlPanel),
        (_, Small::Printers) => sheet("s_printers", ClassicWindow::Folder),
        (_, Small::ClosedFolder) => sheet("s_folder", ClassicWindow::Folder),
    }
}

fn drive_icon(kind: DriveKind, small: bool) -> &'static ClassicIcon {
    match (kind, small) {
        (DriveKind::Floppy, false) => &W95_FLOPPY_32,
        (DriveKind::Floppy, true) => &W95_FLOPPY_16,
        (DriveKind::Floppy525, false) => &W95_FLOPPY_525_32,
        (DriveKind::Floppy525, true) => &W95_FLOPPY_525_16,
        (DriveKind::Removable, false) => &W95_REMOVABLE_32,
        (DriveKind::Removable, true) => &W95_REMOVABLE_16,
        (DriveKind::Fixed, false) => &W95_HARD_DISK_32,
        (DriveKind::Fixed, true) => &W95_HARD_DISK_16,
        (DriveKind::Network, false) => &W95_NET_DRIVE_32,
        (DriveKind::Network, true) => &W95_NET_DRIVE_16,
        (DriveKind::CdRom, false) => &W95_CD_ROM_32,
        (DriveKind::CdRom, true) => &W95_CD_ROM_16,
        (DriveKind::RamDisk, false) => &W95_RAM_DRIVE_32,
        (DriveKind::RamDisk, true) => &W95_RAM_DRIVE_16,
    }
}
static W95_FLOPPY_525_32: ClassicIcon = ClassicIcon::art("folder/floppy-525-32", Look::Win95);

static W95_FLOPPY_525_16: ClassicIcon = ClassicIcon::art("folder/floppy-525-16", Look::Win95);

static W95_FLOPPY_32: ClassicIcon = ClassicIcon::art("folder/floppy-32", Look::Win95);

static W95_FLOPPY_16: ClassicIcon = ClassicIcon::art("folder/floppy-16", Look::Win95);

static W95_REMOVABLE_32: ClassicIcon = ClassicIcon::art("folder/removable-32", Look::Win95);

static W95_REMOVABLE_16: ClassicIcon = ClassicIcon::art("folder/removable-16", Look::Win95);

static W95_HARD_DISK_32: ClassicIcon = ClassicIcon::art("folder/hard-disk-32", Look::Win95);

static W95_HARD_DISK_16: ClassicIcon = ClassicIcon::art("folder/hard-disk-16", Look::Win95);

static W95_NET_DRIVE_32: ClassicIcon = ClassicIcon::art("folder/net-drive-32", Look::Win95);

static W95_NET_DRIVE_16: ClassicIcon = ClassicIcon::art("folder/net-drive-16", Look::Win95);

static W95_CD_ROM_32: ClassicIcon = ClassicIcon::art("folder/cd-rom-32", Look::Win95);

static W95_CD_ROM_16: ClassicIcon = ClassicIcon::art("folder/cd-rom-16", Look::Win95);

static W95_RAM_DRIVE_32: ClassicIcon = ClassicIcon::art("folder/ram-drive-32", Look::Win95);

static W95_RAM_DRIVE_16: ClassicIcon = ClassicIcon::art("folder/ram-drive-16", Look::Win95);

static W95_CONTROL_PANEL_32: ClassicIcon = ClassicIcon::art("folder/control-panel-32", Look::Win95);


static W95_PRINTERS_32: ClassicIcon = ClassicIcon::art("folder/printers-32", Look::Win95);


/// The 16×16 picture of a place in Windows 98's and 2000's address bar and its list: drawn
/// through the shell's image list, whose colors differ slightly from the files' (the title bar's,
/// `small`). Windows 95 has no address bar: its title bar's pictures.
pub fn listed(era: Era, what: Small) -> Icon {
    let name = match what {
        Small::MyComputer => "s_mycomputer".to_string(),
        Small::Folder | Small::ClosedFolder => "s_folder".to_string(),
        Small::Drive(kind) => format!("s_{}", drive_word(kind)),
        Small::Desktop => "s_desktop".to_string(),
        Small::MyDocuments => "s_mydocuments".to_string(),
        Small::RecycleBin => "s_recycle".to_string(),
        Small::ControlPanel => "s_controlpanel".to_string(),
        Small::Printers => "s_printers".to_string(),
    };
    explorer_art::picture(era, &name).unwrap_or_else(|| small(era, what))
}
