//! Folder windows in Small Icons, List and Details against the three Reference PCs
//! (`vm/<era>-truecolor-view-*`, taken 2026-11-14 with `private/probes/views/v.ps1`; the test folders'
//! contents and every number in `private/probes/views/<era>-notes.txt` and docs/theme-spec.md "Folder
//! views"). Masked: the PCs' own pictures of their files (Bevel95 reads today's from the shell),
//! folders the virtual PC drew gray, the pointer, and what Bevel95 leaves out (menu titles and
//! toolbar buttons it does not have).

use super::*;
use bevel_core::desktop_icons::{Item, Picture, ViewMode, local_filetime};
use bevel_core::folder::FolderWindow;
use bevel_core::folder::art::{self, Small};
use bevel_core::folder::menus::Nav;
use bevel_core::kit::Form;

/// A case: its name, the capture, the window's state, and what else it needs.
type StateCase<'a, T> = (&'static str, &'static str, &'a dyn Fn(&mut FolderWindow), T);

/// One item of a test folder: name, a folder or not, bytes, type, its time (y, m, d, h, min).
type Spec = (&'static str, bool, u64, &'static str, (i64, i64, i64, u64, u64));

fn items(list: &[Spec]) -> Vec<Item> {
    list.iter()
        .map(|&(name, folder, size, kind, (y, mo, d, h, mi))| Item {
            id: format!("C:\\v1\\{name}"),
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

/// The few-items folder `C:\v1` of each PC.
fn v1(era: Era) -> Vec<Item> {
    const TEXT: &str = "Text Document";
    match era {
        Era::Win95 => {
            let t = |m| (2026, 11, 14, 14, m);
            items(&[
                ("Alpha", true, 0, "File Folder", t(39)),
                ("Beta", true, 0, "File Folder", t(39)),
                ("Gamma", true, 0, "File Folder", t(39)),
                ("A file with a rather long name for testing", false, 22, TEXT, t(40)),
                ("Big.dat", false, 1_638_400, "DAT File", t(39)),
                ("empty", false, 0, TEXT, t(39)),
                ("medium", false, 3_446, TEXT, t(40)),
                ("Picture", false, 38_462, "Bitmap Image", (1996, 8, 24, 11, 11)),
                ("small", false, 174, TEXT, t(39)),
            ])
        }
        Era::Win98 => {
            let t = |m| (2026, 11, 14, 15, m);
            items(&[
                ("Alpha", true, 0, "File Folder", t(35)),
                ("Beta", true, 0, "File Folder", t(35)),
                ("Gamma", true, 0, "File Folder", t(35)),
                ("A file with a rather long name for testing", false, 22, TEXT, t(36)),
                ("Big.dat", false, 2_801_664, "DAT File", t(36)),
                ("empty", false, 0, TEXT, t(35)),
                ("medium", false, 6_037, TEXT, t(35)),
                ("Picture", false, 308_280, "Bitmap Image", (1998, 5, 11, 20, 1)),
                ("small", false, 289, TEXT, t(35)),
            ])
        }
        Era::Win2000 => {
            let t = |h, m| (2026, 11, 14, h, m);
            items(&[
                ("Alpha", true, 0, "File Folder", t(16, 13)),
                ("Beta", true, 0, "File Folder", t(16, 13)),
                ("Gamma", true, 0, "File Folder", t(16, 13)),
                ("A file with a rather long name for testing", false, 22, TEXT, t(16, 14)),
                ("Big", false, 2_383_632, "DAT File", (2003, 7, 14, 4, 0)),
                ("empty", false, 0, TEXT, t(16, 13)),
                ("medium", false, 1_529, TEXT, t(16, 14)),
                ("Picture", false, 9_522, "Bitmap Image", (2003, 7, 14, 4, 0)),
                ("small", false, 195, TEXT, t(16, 14)),
            ])
        }
    }
}

/// A folder window of `era` as the Reference PC showed one, in `mode`, painted at its place.
fn view_window(fonts: &Fonts, era: Era, at: (i32, i32), size: (i32, i32), list: Vec<Item>, mode: ViewMode, state: impl FnOnce(&mut FolderWindow)) -> FolderWindow {
    let icon = art::small(era, Small::Folder);
    let mut w = FolderWindow::new(era, "v1", icon.clone(), true, size);
    w.set_screen(at.0, at.1);
    w.layout(fonts);
    if era != Era::Win95 {
        w.show_place("v1", icon, true, if era == Era::Win98 { "C:\\v1" } else { "v1" }, art::listed(era, Small::Folder));
        w.set_nav(Nav { up: true, ..Nav::default() });
    }
    w.set_view_mode(mode);
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

/// The small pictures of items `which` (window pixels at 1×, moved to the screen).
fn picture_masks(w: &FolderWindow, at: (i32, i32), which: &[usize]) -> Vec<Rect> {
    which.iter().map(|&i| {
        let r = w.view.icon_rect(i);
        // A row more: 2000's own pictures of text files reach a pixel lower than its folders.
        // (and an application's picture a pixel or two higher).
        Rect::new(at.0 + r.x, at.1 + r.y - 2, r.w, r.h + 3)
    }).collect()
}

pub fn view_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    let whole = |fb: &Framebuffer, at: (i32, i32)| vec![region((0, 0), Rect::new(at.0, at.1, fb.width, fb.height))];
    // Windows 95 (`win95-notes.txt`): v1 at 66, 66, 634 x 411.
    let at = (66, 66);
    let size = (634, 411);
    // Files' pictures are the PC's own; Beta's folder the virtual PC drew gray.
    let masked = [1, 3, 4, 5, 6, 7, 8];
    let help = bevel_core::window::menu_bar_layout(f95, &["&File", "&Edit", "&View", "&Help"])[3];
    let help_mask = Rect::new(at.0 + help.x + 1, at.1 + help.y + 1, help.w, help.h);
    for (name, file, mode, pick) in [
        ("vm-win95-view-list-one", "vm/win95-truecolor-view-v1-list-one.png", ViewMode::List, 6),
        ("vm-win95-view-small-one", "vm/win95-truecolor-view-v1-small-one.png", ViewMode::Small, 5),
        ("vm-win95-view-details-one", "vm/win95-truecolor-view-v1-details-one.png", ViewMode::Details, 6),
    ] {
        // The window opened at 265 x 294 in Large Icons (three columns) and was then made larger.
        let w = view_window(f95, Era::Win95, at, (265, 294), v1(Era::Win95), ViewMode::Large, |w| {
            w.resize(size, false);
            w.set_view_mode(mode);
            w.view.select_only(Some(pick));
        });
        let ours = painted(&w, f95);
        let mut masks = picture_masks(&w, at, &masked);
        masks.push(help_mask);
        masks.push(Rect::new(785, 0, 15, 22));
        out.push(Case { name, file, regions: whole(&ours, at), ours, masks, vga: false });
    }

    // 95's Details in more states: another window in front (no selection), Size's button held,
    // the divider dragged 30 to the right (the line, the columns not moved yet), its release,
    // and a double-click on it (Name fitted to the long name: 209).
    let base95 = |state: &dyn Fn(&mut FolderWindow)| {
        view_window(f95, Era::Win95, at, (265, 294), v1(Era::Win95), ViewMode::Large, |w| {
            w.resize(size, false);
            w.set_view_mode(ViewMode::Details);
            state(w);
        })
    };
    let pointer = |x: i32, y: i32| Rect::new(x, y, 16, 22);
    let cases95: [StateCase<Vec<Rect>>; 5] = [
        ("vm-win95-view-details-inactive", "vm/win95-truecolor-view-v1-details-inactive.png", &|w| {
            // Selected before (-several): medium, Beta, Picture.
            w.view.select_only(Some(7));
            w.view.select(6, true);
            w.view.select(1, true);
            w.set_active(false);
        }, vec![Rect::new(0, 0, 66, 600)]),
        ("vm-win95-view-hdr-size-pressed", "vm/win95-truecolor-view-v1-hdr-size-pressed.png", &|w| {
            w.view.select_only(Some(8));
            w.set_pressed(Some(bevel_core::folder::Part::Header(1)));
        }, vec![pointer(225, 118)]),
        ("vm-win95-view-hdr-divider-drag", "vm/win95-truecolor-view-v1-hdr-divider-drag.png", &|w| {
            w.view.select_only(Some(8));
            let edge = 6 + 120;
            w.press(Some(bevel_core::folder::Part::Divider(0)), edge, 50);
            w.drag(bevel_core::folder::Part::Divider(0), 0, 0, edge + 30, 50);
        }, vec![Rect::new(208, 104, 26, 26)]),
        ("vm-win95-view-hdr-divider-released", "vm/win95-truecolor-view-v1-hdr-divider-released.png", &|w| {
            w.view.select_only(Some(8));
            let edge = 6 + 120;
            w.press(Some(bevel_core::folder::Part::Divider(0)), edge, 50);
            w.drag(bevel_core::folder::Part::Divider(0), 0, 0, edge + 30, 50);
            w.dropped(bevel_core::folder::Part::Divider(0), edge + 30, 50);
        }, vec![Rect::new(208, 104, 26, 26)]),
        ("vm-win95-view-hdr-divider-dblclick", "vm/win95-truecolor-view-v1-hdr-divider-dblclick.png", &|w| {
            w.view.select_only(Some(8));
            let mut c = w.view.columns.clone();
            c[0] = 150;
            w.view.set_columns(&c);
            w.double_click(6 + 150, 50);
        }, vec![Rect::new(200, 110, 60, 18)]),
    ];
    for (name, file, state, extra) in cases95 {
        let w = base95(state);
        let ours = painted(&w, f95);
        let mut masks = picture_masks(&w, at, &masked);
        masks.push(help_mask);
        masks.push(Rect::new(785, 0, 15, 22));
        masks.extend(extra);
        out.push(Case { name, file, regions: whole(&ours, at), ours, masks, vga: false });
    }

    // Windows 98: v1 at 22, 22, 728 x 461; it opened 512 wide with the web view's panel (four
    // columns of large icons), then the panel went and the window was made wider.
    let at = (22, 22);
    let size = (728, 461);
    let masks98 = |w: &FolderWindow, which: &[usize]| {
        let mut m = picture_masks(w, at, which);
        m.push(Rect::new(at.0 + 136, at.1 + 25, 300, 22)); // Favorites, Help
        m.push(Rect::new(80, 114, 18, 16)); // the address bar's picture: none on the PC for a typed place
        m.push(Rect::new(at.0 + 373, at.1 + 49, size.0 - 373 - 8, 40)); // Undo and what follows it
        m.push(Rect::new(785, 0, 15, 22));
        m
    };
    for (name, file, mode, pick, masked) in [
        ("vm-win98-view-list-one", "vm/win98-truecolor-view-v1-list-one.png", ViewMode::List, 6, &[3, 4, 5, 6, 7, 8][..]),
        ("vm-win98-view-small-one", "vm/win98-truecolor-view-v1-small-one.png", ViewMode::Small, 5, &[3, 4, 5, 6, 7, 8][..]),
        ("vm-win98-view-details-one", "vm/win98-truecolor-view-v1-details-one.png", ViewMode::Details, 6, &[3, 4, 5, 6, 7, 8][..]),
    ] {
        let w = view_window(f95, Era::Win98, at, (340, 461), v1(Era::Win98), ViewMode::Large, |w| {
            w.resize(size, false);
            w.set_view_mode(mode);
            w.view.select_only(Some(pick));
        });
        let ours = painted(&w, f95);
        let masks = masks98(&w, masked);
        out.push(Case { name, file, regions: whole(&ours, at), ours, masks, vga: false });
    }

    // 98's Views button under the pointer: the button and its arrow raised, the picture in color
    // (Windows had Undo before it, so only the button itself is compared, where each draws it).
    {
        let mut w = view_window(f95, Era::Win98, at, size, v1(Era::Win98), ViewMode::Large, |_| {});
        let r = w.rect(bevel_core::folder::Part::Tool(bevel_core::folder::Tool::Views)).expect("the Views button");
        w.hover(r.x + 10, r.y + 10);
        let ours = painted(&w, f95);
        out.push(Case {
            name: "vm-win98-view-views-hot",
            file: "vm/win98-truecolor-view-views-hot.png",
            regions: vec![region((r.x, r.y), Rect::new(581, 71, 67, 40))],
            ours,
            // The pointer; one pixel at the arrow's lower left corner differs (not settled).
            masks: vec![Rect::new(605, 90, 16, 20), Rect::new(634, 109, 1, 2)],
            vga: false,
        });
    }

    // Windows 2000: v1 at 22, 22, 680 x 546, as it opened.
    let size = (680, 546);
    let masks2k = |w: &FolderWindow, which: &[usize]| {
        let mut m = picture_masks(w, at, which);
        m.push(Rect::new(at.0 + 120, at.1 + 25, 514, 22)); // Favorites, Tools, Help
        m.push(Rect::new(at.0 + 58, at.1 + 74, 18, 18)); // the address bar's folder (as the list's)
        m.push(Rect::new(at.0 + 139, at.1 + 49, size.0 - 139 - 6, 22)); // Search and what follows it
        m.push(Rect::new(785, 0, 15, 22));
        m
    };
    for (name, file, mode, pick) in [
        ("vm-win2000-view-list-one", "vm/win2000-truecolor-view-v1-list-one.png", ViewMode::List, 6),
        ("vm-win2000-view-small-one", "vm/win2000-truecolor-view-v1-small-one.png", ViewMode::Small, 5),
        ("vm-win2000-view-details-one", "vm/win2000-truecolor-view-v1-details-one.png", ViewMode::Details, 6),
    ] {
        let w = view_window(f2k, Era::Win2000, at, size, v1(Era::Win2000), ViewMode::Large, |w| {
            // Its menus had been opened by keys: 2000 shows focus frames from then on.
            w.view.keys_used = true;
            w.set_view_mode(mode);
            w.view.select_only(Some(pick));
        });
        let ours = painted(&w, f2k);
        // The folders too: this session's PC drew 2000's own smooth folder, Bevel95 the image list's.
        let masks = masks2k(&w, &[0, 1, 2, 3, 4, 5, 6, 7, 8]);
        out.push(Case { name, file, regions: whole(&ours, at), ours, masks, vga: false });
    }
    {
        let mut w = view_window(f2k, Era::Win2000, at, size, v1(Era::Win2000), ViewMode::Large, |_| {});
        let r = w.rect(bevel_core::folder::Part::Tool(bevel_core::folder::Tool::Views)).expect("the Views button");
        w.hover(r.x + 5, r.y + 5);
        let ours = painted(&w, f2k);
        out.push(Case {
            name: "vm-win2000-view-views-hot",
            file: "vm/win2000-truecolor-view-views-hot.png",
            regions: vec![region((r.x, r.y), Rect::new(470, 71, 31, 22))],
            ours,
            masks: vec![Rect::new(478, 84, 23, 9)],
            vga: false,
        });
    }
    // 2000's Details: another window in front keeps the selection in the face color; Size clicked
    // twice (Name fitted to 218 before): biggest first, folders last, the down arrow, the focus on
    // "small" and no selection.
    let cases2k: [StateCase<()>; 2] = [
        ("vm-win2000-view-details-inactive", "vm/win2000-truecolor-view-v1-details-inactive.png", &|w| {
            w.view.select_only(Some(7));
            w.view.select(6, true);
            w.view.select(1, true);
            w.set_active(false);
        }, ()),
        ("vm-win2000-view-sort-size2", "vm/win2000-truecolor-view-v1-sort-size2.png", &|w| {
            let mut c = w.view.columns.clone();
            c[0] = 218;
            w.view.set_columns(&c);
            w.total_size = 2_383_632 + 9_522 + 1_529 + 195 + 22;
            w.view.select_only(Some(8));
            w.header_click(1);
            w.header_click(1);
        }, ()),
    ];
    for (name, file, state, ()) in cases2k {
        let w = view_window(f2k, Era::Win2000, at, size, v1(Era::Win2000), ViewMode::Large, |w| {
            w.view.keys_used = true;
            w.set_view_mode(ViewMode::Details);
            state(w);
        });
        let ours = painted(&w, f2k);
        let mut masks = masks2k(&w, &[0, 1, 2, 3, 4, 5, 6, 7, 8]);
        masks.push(Rect::new(0, 0, 22, 600));
        // 2000 grays the menu titles of a window without the keyboard (not built).
        masks.push(Rect::new(at.0 + 20, at.1 + 28, 100, 14));
        out.push(Case { name, file, regions: whole(&ours, at), ours, masks, vga: false });
    }
}
