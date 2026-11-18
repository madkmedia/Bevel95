//! The pictures of Windows 98's and 2000's folder windows, cut from the Reference PCs' captures
//! (`private/reference/refs/vm/<era>-truecolor-folder-*.png`, by `private/probes/folders2/cut.ps1` from the specs
//! `spec98.txt` and `spec2000.txt` there): the toolbar's pictures plain (gray), under the pointer
//! (in color) and grayed, the throbber at rest, the 16×16 pictures of the address bar and its
//! list, and the 32×32 pictures of My Computer's drives. Their colors are the captures' (both
//! Windows drew through the shell's image lists, which change colors slightly); which pixels are
//! see-through comes from the icons in the PCs' own files. Pictures of kinds the PCs had no
//! example of (a 5¼" floppy, a removable, network or RAM drive) are the files' own.
//!
//! The sheets hold each picture's pixels one after the other, 4 bytes each (red, green, blue,
//! alpha); the tables say where each begins and how big it is.
//!
//! The pictures themselves are Windows' and kept in bevel-art (`art/src/explorer.rs`); this
//! module chooses and draws them.

use crate::art::{self, Look};
use crate::fb::Icon;
use crate::theme::Era;

const SHEET_98: &[u8] = art::bytes("explorer/sheet", Look::Win98);
const SHEET_2000: &[u8] = art::bytes("explorer/sheet", Look::Win2000);

const TABLE_98: &[(&str, usize, i32, i32)] = art::places("explorer/places", Look::Win98);
const TABLE_2000: &[(&str, usize, i32, i32)] = art::places("explorer/places", Look::Win2000);

/// The picture called `name` in `era`'s sheet (`None`: the look has none of that name; Windows 95
/// has no sheet).
pub fn picture(era: Era, name: &str) -> Option<Icon> {
    let (sheet, table) = match era {
        Era::Win95 => return None,
        Era::Win98 => (SHEET_98, TABLE_98),
        Era::Win2000 => (SHEET_2000, TABLE_2000),
    };
    let &(_, at, w, h) = table.iter().find(|(n, ..)| *n == name)?;
    let bytes = sheet.get(at..at + (w * h * 4) as usize)?;
    let pixels = bytes.chunks(4).map(|p| u32::from(p[3]) << 24 | u32::from(p[0]) << 16 | u32::from(p[1]) << 8 | u32::from(p[2])).collect();
    Some(Icon { width: w, height: h, pixels })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_picture_lies_inside_its_sheet() {
        for (sheet, table) in [(SHEET_98, TABLE_98), (SHEET_2000, TABLE_2000)] {
            for &(name, at, w, h) in table {
                assert!(at + (w * h * 4) as usize <= sheet.len(), "{name}");
            }
        }
        assert_eq!(picture(Era::Win98, "throbber").map(|i| (i.width, i.height)), Some((38, 22)));
        assert!(picture(Era::Win2000, "cut_cold").is_none(), "2000's toolbar has no Cut");
    }
}
