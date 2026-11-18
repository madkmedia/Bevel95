//! The pictures of Windows 98's and 2000's folder windows (used by `core::folder::explorer_art`),
//! cut from the Reference PCs' captures (`private/reference/refs/vm/<era>-truecolor-folder-*.png`, by
//! `private/probes/folders2/cut.ps1` from the specs `spec98.txt` and `spec2000.txt` there): the toolbar's
//! pictures plain (gray), under the pointer (in color) and grayed, the throbber at rest, the 16×16
//! pictures of the address bar and its list, and the 32×32 pictures of My Computer's drives. Their
//! colors are the captures' (both Windows drew through the shell's image lists, which change colors
//! slightly); which pixels are see-through comes from the icons in the PCs' own files. Pictures of
//! kinds the PCs had no example of (a 5¼" floppy, a removable, network or RAM drive) are the files'
//! own.
//!
//! The sheets (`assets/win98-explorer.rgba`, `assets/win2000-explorer.rgba`) hold each picture's
//! pixels one after the other, 4 bytes each (red, green, blue, alpha); the tables say where each
//! begins and how big it is.

use crate::{Art, Depth, Entry, Look, Place};
#[rustfmt::skip]
const TABLE_98: &[Place] = &[
    ("back_cold", 0, 21, 21),
    ("back_gray", 1764, 21, 21),
    ("back_hot", 3528, 21, 21),
    ("forward_cold", 5292, 21, 21),
    ("forward_gray", 7056, 21, 21),
    ("forward_hot", 8820, 21, 21),
    ("up_cold", 10584, 21, 21),
    ("up_gray", 12348, 21, 21),
    ("up_hot", 14112, 21, 21),
    ("cut_cold", 15876, 21, 21),
    ("cut_hot", 17640, 21, 21),
    ("copy_cold", 19404, 21, 21),
    ("copy_hot", 21168, 21, 21),
    ("paste_cold", 22932, 21, 21),
    ("paste_hot", 24696, 21, 21),
    ("delete_cold", 26460, 21, 21),
    ("delete_hot", 28224, 21, 21),
    ("properties_cold", 29988, 21, 21),
    ("properties_hot", 31752, 21, 21),
    ("throbber", 33516, 38, 22),
    ("s_desktop", 36860, 16, 16),
    ("s_mycomputer", 37884, 16, 16),
    ("s_floppy", 38908, 16, 16),
    ("s_harddisk", 39932, 16, 16),
    ("s_cdrom", 40956, 16, 16),
    ("s_printers", 41980, 16, 16),
    ("s_controlpanel", 43004, 16, 16),
    ("s_mydocuments", 44028, 16, 16),
    ("s_recycle", 45052, 16, 16),
    ("s_folder", 46076, 16, 16),
    ("s_floppy525", 47100, 16, 16),
    ("s_removable", 48124, 16, 16),
    ("s_network", 49148, 16, 16),
    ("s_ramdisk", 50172, 16, 16),
    ("d_floppy", 51196, 32, 32),
    ("d_harddisk", 55292, 32, 32),
    ("d_harddisk_sel", 59388, 32, 32),
    ("d_cdrom", 63484, 32, 32),
    ("d_printers", 67580, 32, 32),
    ("d_controlpanel", 71676, 32, 32),
    ("d_floppy525", 75772, 32, 32),
    ("d_removable", 79868, 32, 32),
    ("d_network", 83964, 32, 32),
    ("d_ramdisk", 88060, 32, 32),
    ("z_mycomputer", 92156, 16, 16),
    ("views_cold", 93180, 21, 21),
    ("views_hot", 94944, 21, 21),
    // The Undo button (`win98-truecolor-folder-mycomputer`, `-folder-hot-undo`; cut by
    // `private/probes/desktop-drag/cutundo.ps1`).
    ("undo_cold", 96708, 21, 21),
    ("undo_hot", 98472, 21, 21),
];

#[rustfmt::skip]
const TABLE_2000: &[Place] = &[
    ("back_cold", 0, 17, 17),
    ("back_gray", 1156, 17, 17),
    ("back_hot", 2312, 17, 17),
    ("forward_cold", 3468, 17, 17),
    ("forward_gray", 4624, 17, 17),
    ("forward_hot", 5780, 17, 17),
    ("up_cold", 6936, 17, 17),
    ("up_gray", 8092, 17, 17),
    ("up_hot", 9248, 17, 17),
    ("delete_cold", 10404, 17, 17),
    ("delete_hot", 11560, 17, 17),
    ("go_cold", 12716, 18, 17),
    ("go_hot", 13940, 18, 17),
    ("throbber", 15164, 38, 22),
    ("s_desktop", 18508, 16, 16),
    ("s_mydocuments", 19532, 16, 16),
    ("s_mycomputer", 20556, 16, 16),
    ("s_floppy", 21580, 16, 16),
    ("s_harddisk", 22604, 16, 16),
    ("s_cdrom", 23628, 16, 16),
    ("s_controlpanel", 24652, 16, 16),
    ("s_recycle", 25676, 16, 16),
    ("s_folder", 26700, 16, 16),
    ("s_printers", 27724, 16, 16),
    ("s_floppy525", 28748, 16, 16),
    ("s_removable", 29772, 16, 16),
    ("s_network", 30796, 16, 16),
    ("s_ramdisk", 31820, 16, 16),
    ("d_floppy", 32844, 32, 32),
    ("d_harddisk", 36940, 32, 32),
    ("d_cdrom", 41036, 32, 32),
    ("d_controlpanel", 45132, 32, 32),
    ("d_printers", 49228, 32, 32),
    ("d_floppy525", 53324, 32, 32),
    ("d_removable", 57420, 32, 32),
    ("d_network", 61516, 32, 32),
    ("d_ramdisk", 65612, 32, 32),
    ("z_mycomputer", 69708, 16, 16),
    // The Folders button (`win2000-truecolor-tree-folders-off-cold`, `-hot-off`).
    ("folders_cold", 70732, 17, 17),
    ("folders_hot", 71888, 17, 17),
    // The Views button (`win2000-truecolor-view-v1-small-one`, `-view-views-hot`).
    ("views_cold", 73044, 17, 17),
    ("views_hot", 74200, 17, 17),
    // The Undo button: plain from `win2000-truecolor-folder-mycomputer`; under the pointer from
    // 2000's own hot picture strip (browseui.dll bitmap 279), as no True Color capture shows it
    // (the same way gives hot Delete exactly as `-folder-hot-delete`; `cutundo.ps1`).
    ("undo_cold", 75356, 17, 17),
    ("undo_hot", 76512, 17, 17),
];

#[rustfmt::skip]
pub(crate) const ENTRIES: &[Entry] = &[
    Entry { what: "explorer/sheet", looks: &[Look::Win98], depth: Depth::TrueColor, art: Art::Bytes(include_bytes!("../assets/win98-explorer.rgba")) },
    Entry { what: "explorer/sheet", looks: &[Look::Win2000], depth: Depth::TrueColor, art: Art::Bytes(include_bytes!("../assets/win2000-explorer.rgba")) },
    Entry { what: "explorer/places", looks: &[Look::Win98], depth: Depth::TrueColor, art: Art::Places(TABLE_98) },
    Entry { what: "explorer/places", looks: &[Look::Win2000], depth: Depth::TrueColor, art: Art::Places(TABLE_2000) },
];