//! Pictures of Explorer's tree and window that the folder windows had no use for yet, from the
//! Reference PCs' own files (dumped by `tools/dev/icons.ps1`, turned into rows by
//! `tools/dev/icon2rust.ps1`): Windows 95's 16x16 closed and open folder (shell32.dll groups 4
//! and 5, drawn with their yellow-and-gray checkerboard, as the PC showed them), its Desktop
//! (explorer.exe 103) and Printers (shell32 38); and each Windows' Explorer picture (explorer.exe
//! 101: 95's 16-color image, 98's and 2000's 256-color one, the images each PC drew in its title
//! bar, matched pixel for pixel against `*-truecolor-tree-start`).
//!
//! The pictures themselves are Windows' and kept in bevel-art (`art/src/folder.rs`); this
//! module chooses and draws them.

use crate::art::Look;
use crate::fb::Icon;
use crate::icons::ClassicIcon;
use crate::theme::Era;

/// Windows 95's pictures of the tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum W95 {
    ClosedFolder,
    OpenFolder,
    Desktop,
    Printers,
}

pub fn w95(which: W95) -> Icon {
    match which {
        W95::ClosedFolder => W95_CLOSED_FOLDER_16.icon(),
        W95::OpenFolder => W95_OPEN_FOLDER_16.icon(),
        W95::Desktop => W95_DESKTOP_16.icon(),
        W95::Printers => W95_PRINTERS_16.icon(),
    }
}

/// The Explorer's own 16x16 picture in `era` (the title bar and task button of a window with
/// the tree; the Programs menu's "Windows Explorer").
pub fn explorer(era: Era) -> Icon {
    match era {
        Era::Win95 => W95_EXPLORER_16.icon(),
        Era::Win98 => W98_EXPLORER_16.icon(),
        Era::Win2000 => W2000_EXPLORER_16.icon(),
    }
}
static W95_CLOSED_FOLDER_16: ClassicIcon = ClassicIcon::art("tree/closed-folder-16", Look::Win95);

static W95_OPEN_FOLDER_16: ClassicIcon = ClassicIcon::art("tree/open-folder-16", Look::Win95);

static W95_DESKTOP_16: ClassicIcon = ClassicIcon::art("tree/desktop-16", Look::Win95);

static W95_PRINTERS_16: ClassicIcon = ClassicIcon::art("tree/printers-16", Look::Win95);

static W95_EXPLORER_16: ClassicIcon = ClassicIcon::art("tree/explorer-16", Look::Win95);

static W98_EXPLORER_16: ClassicIcon = ClassicIcon::art("tree/explorer-16", Look::Win98);

static W2000_EXPLORER_16: ClassicIcon = ClassicIcon::art("tree/explorer-16", Look::Win2000);

