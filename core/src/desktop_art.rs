//! The pictures of Bevel95's desktop icons, pixel for pixel: My Computer, My Documents (98 and
//! 2000), the Recycle Bin empty and full, the closed folder and the shortcut arrow of each look,
//! 32×32, plain and (98, 2000) selected.
//!
//! Which pixels are see-through comes from the icons in the Reference PCs' own files
//! (explorer.exe for My Computer, mydocs.dll, shell32.dll), dumped by `tools/dev/icons.ps1`.
//! Windows 95 draws those files' colors exactly (its captures match them pixel for pixel), so
//! its pictures are the files' 16-color icons. Windows 98 and 2000 draw a file's icon through
//! the shell's own image list, which changes nearly every color slightly, and they blend a
//! selected icon with the selection color in a way that is not a plain average either; so their
//! pictures, plain and selected, are cut from the Reference PCs' desktops
//! (`private/reference/refs/vm/<era>-truecolor-desk-*.png`) with `tools/dev/cutdesk.ps1`, which takes the
//! colors from the capture and the see-through pixels from the file and checks that they agree.
//! Windows 95's selected icon is the plain average with its selection color, per channel,
//! rounded down (`selected`), as its captures show for every pixel.
//!
//! The pictures themselves are Windows' and kept in bevel-art (`art/src/desktop.rs`); this
//! module chooses and draws them.

use crate::art::Look;
use crate::fb::Icon;
use crate::folder::art::{self as folder_art, DriveKind};
use crate::icons::ClassicIcon;
use crate::theme::Era;

/// A desktop picture of the look's own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Art {
    MyComputer,
    /// Windows 98's and 2000's only (Windows 95 had no My Documents).
    MyDocuments,
    RecycleEmpty,
    RecycleFull,
    /// A closed folder (the owner's folders on the desktop).
    Folder,
    /// A drive, Control Panel and Printers (My Computer's own items in a folder window: Windows 95
    /// only so far, crate::folder::art).
    Drive(DriveKind),
    ControlPanel,
    Printers,
}

/// `art` as `era` drew it on the desktop, plain or selected (`highlight`: the selection color,
/// for the looks whose selected picture is computed). `None` for My Documents in Windows 95.
pub fn picture(era: Era, art: Art, selected: bool, highlight: u32) -> Option<Icon> {
    use Art::*;
    // My Computer's own pictures: 98 and 2000 drew a selected one through the image list's blend,
    // cut where the Reference PC showed one, else the plain average (the rule for the owner's own).
    let own = |icon: Option<Icon>| icon.map(|i| if selected { self::selected(&i, highlight) } else { i });
    let plain = match (era, art) {
        (Era::Win98 | Era::Win2000, Drive(kind)) if selected => {
            return folder_art::drive_selected(era, kind).or_else(|| own(folder_art::drive(era, kind)));
        }
        (_, Drive(kind)) => return own(folder_art::drive(era, kind)),
        (_, ControlPanel) => return own(folder_art::control_panel(era)),
        (_, Printers) => return own(folder_art::printers(era)),
        (Era::Win95, MyDocuments) => return None,
        (Era::Win95, MyComputer) => &W95_MY_COMPUTER,
        (Era::Win95, RecycleEmpty) => &W95_RECYCLE_EMPTY,
        (Era::Win95, RecycleFull) => &W95_RECYCLE_FULL,
        (Era::Win95, Folder) => &W95_FOLDER,
        (Era::Win98, MyComputer) if selected => &W98_MY_COMPUTER_SEL,
        (Era::Win98, MyComputer) => &W98_MY_COMPUTER,
        (Era::Win98, MyDocuments) if selected => &W98_MY_DOCUMENTS_SEL,
        (Era::Win98, MyDocuments) => &W98_MY_DOCUMENTS,
        (Era::Win98, RecycleEmpty) if selected => &W98_RECYCLE_EMPTY_SEL,
        (Era::Win98, RecycleEmpty) => &W98_RECYCLE_EMPTY,
        (Era::Win98, RecycleFull) if selected => &W98_RECYCLE_FULL_SEL,
        (Era::Win98, RecycleFull) => &W98_RECYCLE_FULL,
        (Era::Win98, Folder) if selected => &W98_FOLDER_SEL,
        (Era::Win98, Folder) => &W98_FOLDER,
        (Era::Win2000, MyComputer) if selected => &W2K_MY_COMPUTER_SEL,
        (Era::Win2000, MyComputer) => &W2K_MY_COMPUTER,
        (Era::Win2000, MyDocuments) if selected => &W2K_MY_DOCUMENTS_SEL,
        (Era::Win2000, MyDocuments) => &W2K_MY_DOCUMENTS,
        (Era::Win2000, RecycleEmpty) if selected => &W2K_RECYCLE_EMPTY_SEL,
        (Era::Win2000, RecycleEmpty) => &W2K_RECYCLE_EMPTY,
        (Era::Win2000, RecycleFull) if selected => &W2K_RECYCLE_FULL_SEL,
        (Era::Win2000, RecycleFull) => &W2K_RECYCLE_FULL,
        (Era::Win2000, Folder) if selected => &W2K_FOLDER_SEL,
        (Era::Win2000, Folder) => &W2K_FOLDER,
    };
    let icon = plain.icon();
    Some(if selected && era == Era::Win95 { self::selected(&icon, highlight) } else { icon })
}

/// The arrow over the lower left corner of a shortcut's 32×32 picture (shell32's overlay; Windows
/// 2000's gray is the 848484 of its image list).
pub fn shortcut_arrow(era: Era) -> Icon {
    match era {
        Era::Win95 | Era::Win98 => W95_W98_ARROW.icon(),
        Era::Win2000 => W2K_ARROW.icon(),
    }
}

/// A picture as a selected desktop icon shows it: every visible pixel the average of its color
/// and the selection color, per channel, rounded down (Windows 95, every pixel of its captures;
/// for the owner's own pictures in every look, see the module's note).
pub fn selected(icon: &Icon, highlight: u32) -> Icon {
    let half = |a: u32, b: u32, shift: u32| ((((a >> shift) & 0xFF) + ((b >> shift) & 0xFF)) / 2) << shift;
    let pixels = icon
        .pixels
        .iter()
        .map(|&p| if p >> 24 == 0 { p } else { (p & 0xFF00_0000) | half(p, highlight, 16) | half(p, highlight, 8) | half(p, highlight, 0) })
        .collect();
    Icon { width: icon.width, height: icon.height, pixels }
}

static W95_MY_COMPUTER: ClassicIcon = ClassicIcon::art("desktop/my-computer", Look::Win95);

static W95_RECYCLE_EMPTY: ClassicIcon = ClassicIcon::art("desktop/recycle-empty", Look::Win95);

static W95_RECYCLE_FULL: ClassicIcon = ClassicIcon::art("desktop/recycle-full", Look::Win95);

static W95_FOLDER: ClassicIcon = ClassicIcon::art("desktop/folder", Look::Win95);

static W95_W98_ARROW: ClassicIcon = ClassicIcon::art("desktop/arrow", Look::Win95);

static W98_MY_COMPUTER: ClassicIcon = ClassicIcon::art("desktop/my-computer", Look::Win98);

static W98_MY_DOCUMENTS: ClassicIcon = ClassicIcon::art("desktop/my-documents", Look::Win98);

static W98_RECYCLE_EMPTY: ClassicIcon = ClassicIcon::art("desktop/recycle-empty", Look::Win98);

static W98_RECYCLE_FULL: ClassicIcon = ClassicIcon::art("desktop/recycle-full", Look::Win98);

static W98_FOLDER: ClassicIcon = ClassicIcon::art("desktop/folder", Look::Win98);

static W98_MY_COMPUTER_SEL: ClassicIcon = ClassicIcon::art("desktop/my-computer-sel", Look::Win98);

static W98_MY_DOCUMENTS_SEL: ClassicIcon = ClassicIcon::art("desktop/my-documents-sel", Look::Win98);

static W98_RECYCLE_EMPTY_SEL: ClassicIcon = ClassicIcon::art("desktop/recycle-empty-sel", Look::Win98);

static W98_RECYCLE_FULL_SEL: ClassicIcon = ClassicIcon::art("desktop/recycle-full-sel", Look::Win98);

static W98_FOLDER_SEL: ClassicIcon = ClassicIcon::art("desktop/folder-sel", Look::Win98);

static W2K_MY_DOCUMENTS: ClassicIcon = ClassicIcon::art("desktop/my-documents", Look::Win2000);

static W2K_MY_COMPUTER: ClassicIcon = ClassicIcon::art("desktop/my-computer", Look::Win2000);

static W2K_RECYCLE_EMPTY: ClassicIcon = ClassicIcon::art("desktop/recycle-empty", Look::Win2000);

static W2K_RECYCLE_FULL: ClassicIcon = ClassicIcon::art("desktop/recycle-full", Look::Win2000);

static W2K_FOLDER: ClassicIcon = ClassicIcon::art("desktop/folder", Look::Win2000);

static W2K_ARROW: ClassicIcon = ClassicIcon::art("desktop/arrow", Look::Win2000);

static W2K_MY_DOCUMENTS_SEL: ClassicIcon = ClassicIcon::art("desktop/my-documents-sel", Look::Win2000);

static W2K_MY_COMPUTER_SEL: ClassicIcon = ClassicIcon::art("desktop/my-computer-sel", Look::Win2000);

static W2K_RECYCLE_EMPTY_SEL: ClassicIcon = ClassicIcon::art("desktop/recycle-empty-sel", Look::Win2000);

static W2K_RECYCLE_FULL_SEL: ClassicIcon = ClassicIcon::art("desktop/recycle-full-sel", Look::Win2000);

static W2K_FOLDER_SEL: ClassicIcon = ClassicIcon::art("desktop/folder-sel", Look::Win2000);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_look_has_its_pictures() {
        for era in Era::ALL {
            for art in [Art::MyComputer, Art::RecycleEmpty, Art::RecycleFull, Art::Folder] {
                for selected in [false, true] {
                    let icon = picture(era, art, selected, 0xFF00_0080).unwrap();
                    assert_eq!((icon.width, icon.height), (32, 32), "{era:?} {art:?}");
                    assert!(icon.pixels.iter().any(|p| p >> 24 == 0), "{era:?} {art:?} has see-through pixels");
                }
            }
            assert_eq!(picture(era, Art::MyDocuments, false, 0).is_some(), era != Era::Win95, "Windows 95 had no My Documents");
            assert_eq!(shortcut_arrow(era).width, 32);
        }
    }

    #[test]
    fn windows_95_selects_by_averaging_with_the_selection_color() {
        // Read off the Reference PC (Recycle Bin, plain and selected): white, gray, black, green.
        let icon = Icon { width: 4, height: 1, pixels: vec![0xFFFF_FFFF, 0xFF80_8080, 0xFF00_0000, 0xFF00_8000] };
        let s = selected(&icon, 0xFF00_0080);
        assert_eq!(s.pixels, [0xFF7F_7FBF, 0xFF40_4080, 0xFF00_0040, 0xFF00_4040]);
    }
}
