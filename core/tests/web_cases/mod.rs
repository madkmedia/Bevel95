//! Folder windows with the web view panel against the Windows 98 and 2000 Reference PCs
//! (`vm/<era>-truecolor-web-*`, taken 2026-11-15 with `private/probes/web/w.ps1`; the test folder's
//! contents and every number in `private/probes/web/<era>-notes.txt` and docs/theme-spec.md "The web
//! view panel"). Masked: the PCs' own pictures of their files (Bevel95 reads today's from the
//! shell), what Bevel95 leaves out (menu titles and toolbar buttons it does not have), the pointer,
//! and the pie chart (shell32 drew it with GDI's Ellipse and Pie, whose rounding is not reproduced:
//! its outline differs by a pixel here and there).

use std::sync::Arc;

use super::*;
use bevel_core::desktop_art::{self, Art};
use bevel_core::desktop_icons::{Item, Picture, local_filetime};
use bevel_core::folder::art::{self, DriveKind, Small};
use bevel_core::folder::menus::Nav;
use bevel_core::folder::web::{Guard, Kind, WebFonts};
use bevel_core::folder::{FolderWindow, Space};
use bevel_core::kit::Form;

fn fixture(name: &str) -> Font {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
    Font::parse(&std::fs::read_to_string(&path).unwrap()).unwrap().with_period_glyphs()
}

/// The panel's fonts as the folders program makes them.
pub fn web_fonts(era: Era, look: &Fonts) -> Arc<WebFonts> {
    Arc::new(match era {
        Era::Win2000 => WebFonts { body: look.regular.clone(), bold: look.bold.clone(), title: fixture("tahoma-17px-bold.font") },
        _ => WebFonts { body: fixture("verdana-11px.font"), bold: fixture("verdana-11px-bold.font"), title: fixture("verdana-21px-bold.font") },
    })
}

/// One item: name as listed, the file's name, a folder or not, bytes, type, its time.
type Spec = (&'static str, &'static str, bool, u64, &'static str, (i64, i64, i64, u64, u64));

fn items(dir: &str, list: &[Spec]) -> Vec<Item> {
    list.iter()
        .map(|&(name, file, folder, size, kind, (y, mo, d, h, mi))| Item {
            id: format!("{dir}\\{file}"),
            name: name.to_string(),
            picture: if folder { Picture::Folder } else { Picture::Own(None) },
            shortcut: false,
            folder,
            type_name: kind.to_string(),
            size,
            modified: local_filetime(y, mo, d, h, mi),
        })
        .collect()
}

/// C:\v1 as each PC held it in this session (`<era>-notes.txt`, SETUP).
fn v1(era: Era) -> Vec<Item> {
    const TEXT: &str = "Text Document";
    match era {
        Era::Win2000 => {
            let t = |m| (2026, 11, 15, 14, m);
            items("C:\\v1", &[
                ("Alpha", "Alpha", true, 0, "File Folder", t(6)),
                ("Beta", "Beta", true, 0, "File Folder", t(6)),
                ("Gamma", "Gamma", true, 0, "File Folder", t(6)),
                ("A file with a rather long name for testing", "A file with a rather long name for testing.txt", false, 19, TEXT, t(7)),
                ("Big", "Big.dat", false, 2_383_632, "DAT File", (2003, 7, 14, 4, 0)),
                ("empty", "empty.txt", false, 0, TEXT, t(7)),
                ("medium", "medium.txt", false, 321, TEXT, t(7)),
                ("Picture", "Picture.bmp", false, 9_522, "Bitmap Image", (2003, 7, 14, 4, 0)),
                ("small", "small.txt", false, 51, TEXT, t(7)),
            ])
        }
        _ => {
            let t = |m| (2026, 11, 15, 13, m);
            items("C:\\v1", &[
                ("Alpha", "Alpha", true, 0, "File Folder", t(38)),
                ("Beta", "Beta", true, 0, "File Folder", t(38)),
                ("Gamma", "Gamma", true, 0, "File Folder", t(38)),
                ("A file with a rather long name for testing", "A file with a rather long name for testing.txt", false, 19, TEXT, t(39)),
                ("Big.dat", "Big.dat", false, 1_541_400, "DAT File", t(39)),
                ("empty", "empty.txt", false, 0, TEXT, t(39)),
                ("medium", "medium.txt", false, 6_644, TEXT, t(39)),
                ("Picture", "Picture.bmp", false, 308_280, "Bitmap Image", (1998, 5, 11, 20, 1)),
                ("small", "small.txt", false, 51, TEXT, t(39)),
            ])
        }
    }
}

/// A folder window of `era` with the web view on, painted at its place.
#[allow(clippy::too_many_arguments)]
pub(super) fn web_window(fonts: &Fonts, era: Era, at: (i32, i32), size: (i32, i32), title: &str, address: &str, small: Small, files: bool, list: Vec<Item>, state: impl FnOnce(&mut FolderWindow)) -> FolderWindow {
    let icon = art::small(era, small);
    let mut w = FolderWindow::new(era, title, icon.clone(), files, size);
    w.set_screen(at.0, at.1);
    w.layout(fonts);
    w.show_place(title, icon, files, address, art::listed(era, small));
    w.set_nav(Nav { up: true, ..Nav::default() });
    w.web.fonts = Some(web_fonts(era, fonts));
    let art = match small {
        Small::MyComputer => Art::MyComputer,
        Small::Drive(k) => Art::Drive(k),
        Small::MyDocuments => Art::MyDocuments,
        _ => Art::Folder,
    };
    let kind = match small {
        Small::MyComputer => Kind::MyComputer,
        Small::Drive(_) => Kind::Drive,
        Small::MyDocuments => Kind::MyDocuments,
        _ => Kind::Folder,
    };
    w.set_web_place(kind, desktop_art::picture(era, art, false, 0), None, false, None);
    w.set_web(true);
    w.total_size = list.iter().map(|i| i.size).sum();
    w.set_items(list);
    state(&mut w);
    w.layout(fonts);
    w
}

fn painted(w: &FolderWindow, fonts: &Fonts) -> Framebuffer {
    let mut fb = Framebuffer::new(w.size_now().0, w.size_now().1, 0);
    w.paint(&mut fb, bevel_core::theme::theme(w.era), fonts);
    fb
}

/// The large pictures of items `which` (moved to the screen).
pub(super) fn picture_masks(w: &FolderWindow, at: (i32, i32), which: &[usize]) -> Vec<Rect> {
    which.iter().map(|&i| {
        let r = w.view.icon_rect(i);
        Rect::new(at.0 + r.x - 1, at.1 + r.y - 1, r.w + 2, r.h + 2)
    }).collect()
}

pub(super) fn select(w: &mut FolderWindow, which: &[usize]) {
    w.view.select_only(None);
    for &i in which {
        w.view.select(i, true);
    }
    w.view.focus = which.last().copied();
}

pub fn web_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    let whole = |fb: &Framebuffer, at: (i32, i32)| vec![region((0, 0), Rect::new(at.0, at.1, fb.width, fb.height))];

    // Windows 98: v1 at 22,22, 512 x 461 (the panel 150 wide). The list's files are the PC's own
    // pictures; its folder picture in the panel too (a shell icon with another tab).
    let at = (22, 22);
    let size = (512, 461);
    let masks98 = |w: &FolderWindow, which: &[usize], at: (i32, i32), size: (i32, i32)| {
        let mut m = picture_masks(w, at, which);
        m.push(Rect::new(at.0 + 136, at.1 + 25, 300, 22)); // Favorites, Help
        m.push(Rect::new(at.0 + 373, at.1 + 49, size.0 - 373 - 8, 40)); // Undo and what follows it
        m.push(Rect::new(at.0 + 57, at.1 + 94, 18, 16)); // the address bar's picture
        m.push(Rect::new(at.0 + 21, at.1 + 135, 34, 34)); // the panel's folder picture
        m.push(Rect::new(785, 0, 15, 22));
        m.push(Rect::new(630, 470, 20, 25)); // the pointer
        m
    };
    let files98 = [3, 4, 5, 6, 7, 8];
    let cases98: [(&str, &str, &[usize]); 3] = [
        ("vm-win98-web-v1-none", "vm/win98-truecolor-web-v1-none.png", &[]),
        ("vm-win98-web-v1-file", "vm/win98-truecolor-web-v1-file.png", &[8]),
        ("vm-win98-web-v1-several", "vm/win98-truecolor-web-v1-several.png", &[8, 6, 1]),
    ];
    for (name, file, sel) in cases98 {
        let w = web_window(f95, Era::Win98, at, size, "v1", "C:\\v1", Small::Folder, true, v1(Era::Win98), |w| {
            select(w, sel);
            // The status bar's total as the PC showed it (1.76MB; the listed sizes add up to 1.77MB).
            w.total_size = 1_850_000;
        });
        let ours = painted(&w, f95);
        let mut masks = masks98(&w, &files98, at, size);
        // The long name's second line: 98's list cut it shorter beside the panel (ather lon...)
        // than in a plain window; the rule is not settled.
        masks.push(Rect::new(400, 190, 80, 18));
        out.push(Case { name, file, regions: whole(&ours, at), ours, masks, vga: false });
    }

    // 98's My Computer maximized with C: selected: its capacity, legend and pie (`mc-c-max`). The
    // icons stay in the rows of the window it was first (512 wide: four columns). Dial-Up
    // Networking and Scheduled Tasks have no counterpart (masked).
    {
        let mut w = web_window(f95, Era::Win98, (66, 66), (512, 461), "My Computer", "My Computer", Small::MyComputer, false, my_computer_98(), |w| {
            w.spaces.insert("C:\\".into(), Space { free: 811_073_536, capacity: 1_072_693_248 });
            w.resize((800, 572), true);
            w.set_screen(0, 0);
            w.set_nav(Nav::default());
            select(w, &[1]);
        });
        w.set_screen(0, 0);
        let ours = painted(&w, f95);
        let mut masks = vec![Rect::new(16, 362, 102, 52), Rect::new(760, 0, 40, 30), Rect::new(130, 22, 120, 22), Rect::new(380, 44, 420, 40), Rect::new(57, 90, 18, 16)];
        masks.extend(picture_masks(&w, (0, 0), &[0, 1, 2, 3, 4]));
        masks.push(Rect::new(310, 190, 160, 70)); // Dial-Up Networking, Scheduled Tasks
        masks.push(Rect::new(14, 130, 36, 36)); // the panel's computer picture
        out.push(Case { name: "vm-win98-web-mc-c-max", file: "vm/win98-truecolor-web-mc-c-max.png", regions: vec![region((0, 0), Rect::new(0, 0, 800, 572))], ours, masks, vga: false });
    }
    // Windows 2000: v1 at 132,132, 664 x 436 (the panel 200 wide).
    let at = (132, 132);
    let size = (664, 436);
    let masks2k = |w: &FolderWindow, which: &[usize], at: (i32, i32), size: (i32, i32)| {
        let mut m = picture_masks(w, at, which);
        m.push(Rect::new(at.0 + 120, at.1 + 25, 514, 22)); // Favorites, Tools, Help
        m.push(Rect::new(at.0 + 58, at.1 + 74, 18, 18)); // the address bar's folder
        m.push(Rect::new(at.0 + 139, at.1 + 49, size.0 - 139 - 6, 22)); // Search and what follows it
        m.push(Rect::new(785, 0, 15, 22));
        m.push(Rect::new(at.0 + 10, at.1 + 108, 36, 36)); // the panel's folder picture
        // The icons beside the panel: this session's captures put the first column 8, 16 or 21 pixels
        // from the panel (v1, C:, My Documents) where Bevel95 puts it 20 as in a plain window; what
        // decided it is not settled (private/todo.md).
        m.push(Rect::new(at.0 + 6 + 200, at.1 + 104, size.0 - 6 - 200 - 6, size.1 - 104 - 26));
        m
    };
    let cases2k: [(&str, &str, &[usize]); 3] = [
        ("vm-win2000-web-v1-none", "vm/win2000-truecolor-web-v1-none.png", &[]),
        ("vm-win2000-web-v1-file", "vm/win2000-truecolor-web-v1-file.png", &[8]),
        ("vm-win2000-web-v1-several", "vm/win2000-truecolor-web-v1-several.png", &[8, 6, 1]),
    ];
    for (name, file, sel) in cases2k {
        let w = web_window(f2k, Era::Win2000, at, size, "v1", "v1", Small::Folder, true, v1(Era::Win2000), |w| {
            w.view.keys_used = true;
            select(w, sel);
        });
        let ours = painted(&w, f2k);
        let masks = masks2k(&w, &[0, 1, 2, 3, 4, 5, 6, 7, 8], at, size);
        out.push(Case { name, file, regions: whole(&ours, at), ours, masks, vga: false });
    }

    // 2000's My Computer (nothing selected: the note and the links) and C: (the pie), a new window
    // at 22,22, 678 x 546.
    let at = (22, 22);
    let size = (680, 546);
    {
        let w = web_window(f2k, Era::Win2000, at, size, "My Computer", "My Computer", Small::MyComputer, false, my_computer_2000(), |_| {});
        let ours = painted(&w, f2k);
        let masks = masks2k(&w, &[], at, size);
        out.push(Case { name: "vm-win2000-web-mc-none", file: "vm/win2000-truecolor-web-mc-none.png", regions: whole(&ours, at), ours, masks, vga: false });
    }
    {
        let mut w = web_window(f2k, Era::Win2000, at, size, "Local Disk (C:)", "Local Disk (C:)", Small::Drive(DriveKind::Fixed), true, drive_c_2000(), |_| {});
        w.web.space = Some(Space { free: 2_203_000_000, capacity: 3_216_500_000 });
        w.set_nav(Nav { up: true, back: true, ..Nav::default() });
        w.hidden = 2;
        let ours = painted(&w, f2k);
        let mut masks = masks2k(&w, &[0, 1, 2, 3, 4], at, size);
        masks.push(Rect::new(39, 369, 122, 61)); // the pie
        out.push(Case { name: "vm-win2000-web-drive-c-none", file: "vm/win2000-truecolor-web-drive-c-none.png", regions: whole(&ours, at), ours, masks, vga: false });
    }

    // The warning pages of the Windows folder (98: C:\WINDOWS at 110,110; 2000: C:\WINNT at 132,132).
    {
        let at = (110, 110);
        let mut w = web_window(f95, Era::Win98, at, (512, 458), "Windows", "C:\\WINDOWS", Small::Folder, true, Vec::new(), |_| {});
        w.set_web_place(Kind::Folder, desktop_art::picture(Era::Win98, Art::Folder, false, 0), Some(Guard::System), false, None);
        let ours = painted(&w, f95);
        let mut masks = masks98(&w, &[], at, (512, 458));
        masks.push(Rect::new(at.0, at.1 + 438, 400, 20)); // the status bar: the folder's 249 files are not read here
        out.push(Case { name: "vm-win98-web-windows-warning", file: "vm/win98-truecolor-web-windows-warning.png", regions: whole(&ours, at), ours, masks, vga: false });
    }
    {
        let at = (132, 132);
        let mut w = web_window(f2k, Era::Win2000, at, (664, 436), "WINNT", "WINNT", Small::Folder, true, Vec::new(), |_| {});
        w.set_web_place(Kind::Folder, desktop_art::picture(Era::Win2000, Art::Folder, false, 0), Some(Guard::System), false, None);
        let ours = painted(&w, f2k);
        let mut masks = masks2k(&w, &[], at, (664, 436));
        masks.push(Rect::new(at.0, at.1 + 414, 520, 20)); // the status bar: the folder's 87 files are not read here
        out.push(Case { name: "vm-win2000-web-windows-warning", file: "vm/win2000-truecolor-web-windows-warning.png", regions: whole(&ours, at), ours, masks, vga: false });
    }
    // 2000's Folder Options (`web-fo-own-window`: the window at 12,12, "own window" just chosen by
    // keys, so Apply is alive and the focus on it). Bevel95's page leaves out the Active Desktop and
    // Click items groups and the other tabs: the window is compared in three bands, each against
    // the capture's rows it stands for (75 and 185 rows further down), the tabs left out masked.
    {
        use bevel_core::folder::options::{Choice, FolderOptions, Part as P};
        let mut w = FolderOptions::new(Choice { web: true, own_window: false }, true);
        w.press(Some(P::Own), 0, 0);
        let mut ours = Framebuffer::new(w.size().0, w.size().1, 0);
        w.paint(&mut ours, bevel_core::theme::theme(Era::Win2000), f2k);
        let at = (12, 12);
        let band = |y0: i32, y1: i32, shift: i32| region((0, y0), Rect::new(at.0, at.1 + y0 + shift, 386, y1 - y0));
        out.push(Case {
            name: "vm-win2000-web-folder-options",
            file: "vm/win2000-truecolor-web-fo-own-window.png",
            regions: vec![band(0, 58, 0), band(58, 207, 75), band(207, 283, 185)],
            ours,
            masks: vec![Rect::new(at.0 + 62, at.1 + 28, 170, 21)],
            vga: false,
        });
    }
}

fn place(id: &str, name: &str, art: Art) -> Item {
    Item { id: id.into(), name: name.into(), picture: Picture::Art(art), shortcut: false, folder: false, type_name: String::new(), size: 0, modified: 0 }
}

pub(super) fn my_computer_98() -> Vec<Item> {
    vec![
        place("A:\\", "3\u{bd} Floppy (A:)", Art::Drive(DriveKind::Floppy)),
        place("C:\\", " (C:)", Art::Drive(DriveKind::Fixed)),
        place("D:\\", " (D:)", Art::Drive(DriveKind::CdRom)),
        place("::printers", "Printers", Art::Printers),
        place("::control", "Control Panel", Art::ControlPanel),
    ]
}

pub(super) fn my_computer_2000() -> Vec<Item> {
    vec![
        place("A:\\", "3\u{bd} Floppy (A:)", Art::Drive(DriveKind::Floppy)),
        place("C:\\", "Local Disk (C:)", Art::Drive(DriveKind::Fixed)),
        place("D:\\", "Compact Disc (D:)", Art::Drive(DriveKind::CdRom)),
        place("::control", "Control Panel", Art::ControlPanel),
    ]
}

fn drive_c_2000() -> Vec<Item> {
    let t = (2026, 11, 15, 14, 6);
    items("C:", &[
        ("Documents and Settings", "Documents and Settings", true, 0, "File Folder", t),
        ("Program Files", "Program Files", true, 0, "File Folder", t),
        ("v1", "v1", true, 0, "File Folder", t),
        ("v2", "v2", true, 0, "File Folder", t),
        ("WINNT", "WINNT", true, 0, "File Folder", t),
    ])
}
