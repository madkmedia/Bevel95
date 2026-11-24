//! The old Windows' pictures for kinds of file against the three Reference PCs (`vm/<pc>-<depth>-
//! fileicons-*`, taken 2026-11-23 for brief 81: `private/probes/file-icons/survey.md`). Each PC's test
//! folder C:\ICONS held the same 56 files (`floppy/<pc>/MAKE.BAT` there), shown maximized with
//! every ending shown, arranged by name, web view off. Bevel95 draws the same folder: each file
//! with what `file_kinds::decide` gives it; the list (pictures and labels) is compared, from its
//! first row to the status bar.
//!
//! Masked, each with its reason: the pictures that are the file's own or today's (icons, cursors,
//! shortcuts, the screen saver, the program with an icon, and every kind the PC did not know:
//! Bevel95 reads those from today's shell). Only the list is compared (98's tooltip over its title
//! bar lies outside it). The selection's tint is not in these captures (nothing selected; 95's
//! and 98's focus frame on the first label is). The shortcut arrows are compared, 32×32 and
//! 16×16.

use super::*;
use bevel_core::desktop_icons::{Item, Picture, ViewMode};
use bevel_core::file_kinds::{self, Choice, Kind};
use bevel_core::folder::FolderWindow;
use bevel_core::folder::art::{self, Small};
use bevel_core::kit::Form;
use bevel_core::theme::Depth;

/// The 56 files of C:\ICONS in the order the PCs arranged them by name, as they were named (the
/// shortcuts' endings never show).
const FILES: [&str; 56] = [
    "386.386", "ani.ani", "avi.avi", "bak.bak", "bat.bat", "bmp.bmp", "cab.cab", "chm.chm", "cmd.cmd", "cnt.cnt", "console.exe", "cpl.cpl",
    "cur.cur", "dat.dat", "dll.dll", "doc.doc", "dosprog.exe", "drv.drv", "fon.fon", "gif.gif", "hlp.hlp", "htm.htm", "html.html", "ico.ico",
    "inf.inf", "ini.ini", "jpg.jpg", "js.js", "lnk.lnk", "log.log", "mid.mid", "more.com", "mp3.mp3", "msi.msi", "noext", "noicon.exe",
    "ocx.ocx", "pcx.pcx", "pif.pif", "qqq.qqq", "reg.reg", "rtf.rtf", "scr.scr", "sys.sys", "theme.theme", "tif.tif", "ttf.ttf", "txt.txt",
    "url.url", "vbs.vbs", "vxd.vxd", "wav.wav", "win16.exe", "withicon.exe", "wri.wri", "zip.zip",
];

/// The programs of the folder that have no icon of their own (the PCs' REGSVR32, a console program,
/// MEM, a 16-bit program): the look's program picture.
const NO_ICON: [&str; 4] = ["console.exe", "dosprog.exe", "noicon.exe", "win16.exe"];

/// The items as Bevel95 reads them in `era`, and the indexes of those whose picture is not the
/// look's (masked).
fn items(era: Era) -> (Vec<Item>, Vec<usize>) {
    let mut masked = Vec::new();
    let items = FILES
        .iter()
        .enumerate()
        .map(|(i, &file)| {
            let d = file_kinds::decide(era, file);
            let picture = match d.picture {
                Choice::Look(k) => Picture::Kind(k),
                Choice::OwnOrProgram if NO_ICON.contains(&file) => Picture::Kind(Kind::Program),
                // The Reference PCs' url.url names no picture of its own: the kind's (brief 100).
                Choice::OwnOrLook => Picture::Kind(Kind::Ending),
                _ => {
                    masked.push(i);
                    Picture::Own(None)
                }
            };
            // .lnk, .pif and .url never showed their ending.
            let shown = ["lnk.lnk", "pif.pif", "url.url"].contains(&file);
            Item {
                id: format!("C:\\ICONS\\{file}"),
                name: if shown { file.split('.').next().unwrap().to_string() } else { file.to_string() },
                picture,
                shortcut: matches!(file, "lnk.lnk" | "pif.pif" | "url.url"),
                folder: false,
                type_name: String::new(),
                size: 0,
                modified: 0,
            }
        })
        .collect();
    (items, masked)
}

/// The maximized folder window of `era` at `depth` in `mode`, as the PC showed C:\ICONS.
fn window(fonts: &Fonts, era: Era, depth: Depth, mode: ViewMode, keys: bool) -> (FolderWindow, Framebuffer, Vec<usize>) {
    let icon = art::small(era, Small::Folder);
    let mut w = FolderWindow::new(era, "icons", icon.clone(), true, (800, 572));
    w.resize((800, 572), true);
    w.set_screen(0, 0);
    w.layout(fonts);
    if era != Era::Win95 {
        w.show_place("icons", icon, true, if era == Era::Win98 { "C:\\icons" } else { "icons" }, art::listed(era, Small::Folder));
    }
    w.set_view_mode(mode);
    let (list, masked) = items(era);
    w.set_items(list);
    // The first item has the focus, nothing is selected (its label's dotted frame in 95 and 98).
    w.view.focus = Some(0);
    w.view.keys_used = keys;
    w.layout(fonts);
    let mut fb = Framebuffer::new(800, 572, 0);
    w.paint(&mut fb, bevel_core::theme::theme_in(era, depth), fonts);
    (w, fb, masked)
}

/// Where a capture's items stood when the PC put them elsewhere than Bevel95 does (its layout, not
/// a picture: not this job's): each item is compared cell by cell where the PC drew it.
#[derive(Clone, Copy)]
enum Placed {
    /// Where Bevel95 puts them.
    Same,
    /// 2000's True Color window (that session's own arrangement): every column 6 pixels to the
    /// left of where Bevel95 and 2000's 256-color capture put it (its first picture at x 17).
    Left(i32),
    /// 95's Small Icons arranged after Large Icons: eight columns 89 apart (the widest label + 28;
    /// the survey's "one difference found"), where Bevel95 keeps theme-spec's 112 (seven columns).
    Columns(i32),
}

pub fn fileicons_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use Placed::*;
    for (name, file, era, depth, mode, list_top, placed) in [
        ("vm-win95-fileicons-large", "vm/win95-truecolor-fileicons-large.png", Era::Win95, Depth::TrueColor, ViewMode::Large, 40, Same),
        ("vm-win95-fileicons-small", "vm/win95-truecolor-fileicons-small.png", Era::Win95, Depth::TrueColor, ViewMode::Small, 40, Columns(89)),
        ("vm-win95-fileicons-256-large", "vm/win95-256color-fileicons-large.png", Era::Win95, Depth::TwoFiftySix, ViewMode::Large, 40, Same),
        ("vm-win95-fileicons-16-large", "vm/win95-16color-fileicons-large.png", Era::Win95, Depth::Sixteen, ViewMode::Large, 40, Same),
        ("vm-win98-fileicons-large", "vm/win98-truecolor-fileicons-large.png", Era::Win98, Depth::TrueColor, ViewMode::Large, 116, Same),
        ("vm-win98-fileicons-small", "vm/win98-truecolor-fileicons-small.png", Era::Win98, Depth::TrueColor, ViewMode::Small, 116, Same),
        ("vm-win98-fileicons-256-large", "vm/win98-256color-fileicons-large.png", Era::Win98, Depth::TwoFiftySix, ViewMode::Large, 116, Same),
        ("vm-win98-fileicons-16-large", "vm/win98-16color-fileicons-large.png", Era::Win98, Depth::Sixteen, ViewMode::Large, 116, Same),
        ("vm-win2000-fileicons-large", "vm/win2000-truecolor-fileicons-large.png", Era::Win2000, Depth::TrueColor, ViewMode::Large, 98, Left(6)),
        ("vm-win2000-fileicons-small", "vm/win2000-truecolor-fileicons-small.png", Era::Win2000, Depth::TrueColor, ViewMode::Small, 98, Same),
        ("vm-win2000-fileicons-256-large", "vm/win2000-256color-fileicons-large.png", Era::Win2000, Depth::TwoFiftySix, ViewMode::Large, 98, Same),
    ] {
        let fonts = if era == Era::Win2000 { f2k } else { f95 };
        // 2000's 256-color session had used the keyboard: its focus frame shows (2000 shows it
        // only then).
        let keys = era == Era::Win2000 && depth == Depth::TwoFiftySix;
        let (w, ours, masked) = window(fonts, era, depth, mode, keys);
        let first = w.view.cell(0);
        // Where item i's cell is on the capture, moved from where Bevel95 drew it.
        let moved = |i: usize, r: Rect| -> Rect {
            match placed {
                Same => r,
                Left(d) => Rect::new(r.x - d, r.y, r.w, r.h),
                // The PC's eight columns, row by row, against Bevel95's seven.
                Columns(step) => {
                    let c = w.view.cell(i);
                    let n = 8;
                    let (x, y) = (first.x + (i as i32 % n) * step, first.y + (i as i32 / n) * first.h);
                    Rect::new(r.x - c.x + x, r.y - c.y + y, r.w, r.h)
                }
            }
        };
        let masks: Vec<Rect> = masked.iter().map(|&i| moved(i, w.view.icon_rect(i))).collect();
        let regions = match placed {
            Same => {
                let list = Rect::new(2, list_top, 796, 550 - list_top);
                vec![region((list.x, list.y), list)]
            }
            // Cell by cell (a cell as wide as the PC's columns, so that none overlaps).
            Left(_) | Columns(_) => (0..FILES.len())
                .map(|i| {
                    let c = w.view.cell(i);
                    // Left: the cell's first pixels would fall on the PC's window frame.
                    let ours_cell = match placed {
                        Columns(step) => Rect::new(c.x, c.y, step, c.h),
                        Left(d) => Rect::new(c.x + d, c.y, c.w - d, c.h),
                        Same => c,
                    };
                    let theirs = moved(i, ours_cell);
                    region((ours_cell.x, ours_cell.y), theirs)
                })
                .collect(),
        };
        out.push(Case { name, file, regions, ours, masks, vga: false });
    }
}
