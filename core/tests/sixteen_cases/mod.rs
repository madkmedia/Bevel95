//! The 16 Colors option against the Reference PCs in 16 colors (`vm/<pc>-16color-*`, taken
//! 2026-11-16; private/probes/colors16/survey.txt, docs/theme-spec.md "16 Colors"). The emulator
//! shows the 16 colors with its own values: `read_capture` translates them to Windows' own,
//! which Bevel95 draws. Windows 2000's captures are 640×480 (its built-in VGA driver), so
//! everything is found where it is on them (`locate`) rather than at a measured place.
//!
//! Masked, each with its reason in the case: the PCs' own pictures of programs and files, the
//! clock's time, and what Bevel95 leaves out.

use super::*;

/// The emulator's picture of a 16-color screen -> Windows' 16 colors (the survey's table; the
/// other seven are shown with Windows' values).
const QEMU_16: [(u32, u32); 9] = [
    (0xA80057, 0x800000),
    (0x00A857, 0x008000),
    (0xA8A857, 0x808000),
    (0x0000A8, 0x000080),
    (0xA857A8, 0x800080),
    (0x57A8A8, 0x008080),
    (0xC0C7C8, 0xC0C0C0),
    (0x87888F, 0x808080),
    (0x000000, 0x000000),
];

/// A capture as Windows' own colors: a 16-color capture translated, any other as it is.
pub(super) fn read_capture(path: &Path) -> Framebuffer {
    let mut fb = read_png(path);
    if path.to_string_lossy().contains("-16color-") {
        for p in &mut fb.pixels {
            if let Some(&(_, windows)) = QEMU_16.iter().find(|(qemu, _)| *qemu == *p & 0xFF_FFFF) {
                *p = rgb(windows);
            }
        }
    }
    fb
}

/// Where `ours` is in `shot` (its top-left corner), searched in `area` of the shot: the place
/// where the most of a fixed spread of its pixels match, provided nearly all of them do.
pub(super) fn locate(shot: &Framebuffer, ours: &Framebuffer, area: Rect, skip: &[Rect]) -> Option<(i32, i32)> {
    // A spread of points over the picture (fixed, so the search is the same each run).
    let mut points = Vec::new();
    let mut seed = 12_345u32;
    for _ in 0..96 {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        let x = (seed >> 8) as i32 % ours.width;
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        let y = (seed >> 8) as i32 % ours.height;
        if !skip.iter().any(|m| m.contains(x, y)) {
            points.push((x, y));
        }
    }
    let mut best = None;
    let mut best_hits = 0;
    for y in area.y..=(area.bottom() - ours.height).min(shot.height - ours.height) {
        for x in area.x..=(area.right() - ours.width).min(shot.width - ours.width) {
            let mut hits = 0;
            for (i, &(px, py)) in points.iter().enumerate() {
                if shot.get(x + px, y + py) == ours.get(px, py) {
                    hits += 1;
                } else if i - hits > 12 {
                    break;
                }
            }
            if hits > best_hits {
                (best_hits, best) = (hits, Some((x, y)));
            }
        }
    }
    best.filter(|_| best_hits * 4 >= points.len() * 3)
}

/// One case found by `locate`: our picture compared where it is on the capture, the masks given
/// relative to our picture. Skipped (not built) when the capture is missing; a capture where
/// our picture is not found compares it at (0, 0), so the case fails with what differs.
fn located(out: &mut Vec<Case>, name: &'static str, file: &'static str, ours: Framebuffer, area: Rect, part: Option<Rect>, masks: &[Rect]) {
    let part = part.unwrap_or(Rect::new(0, 0, ours.width, ours.height));
    let path = refs_dir().join(file);
    if !path.is_file() {
        out.push(Case { name, file, regions: vec![], ours, masks: vec![], vga: false });
        return;
    }
    let shot = read_capture(&path);
    let mut piece = Framebuffer::new(part.w, part.h, 0);
    piece.blit_part(&ours, part, 0, 0);
    let skip: Vec<Rect> = masks.iter().map(|m| Rect::new(m.x - part.x, m.y - part.y, m.w, m.h)).collect();
    let at = locate(&shot, &piece, area, &skip).map_or((0, 0), |(x, y)| (x - part.x, y - part.y));
    let masks = masks.iter().map(|m| Rect::new(at.0 + m.x, at.1 + m.y, m.w, m.h)).collect();
    let region = region((part.x, part.y), Rect::new(at.0 + part.x, at.1 + part.y, part.w, part.h));
    out.push(Case { name, file, regions: vec![region], ours, masks, vga: false });
}

fn fonts_of<'a>(era: Era, f95: &'a Fonts<'a>, f2k: &'a Fonts<'a>) -> &'a Fonts<'a> {
    if era == Era::Win2000 { f2k } else { f95 }
}

/// My Computer selected on the desktop (`<pc>-16color-desk-sel-mycomputer`), its 75×75 cell where
/// the PC shows it, as the True Color case `vm-<pc>-desk-sel-mycomputer` does: the selected
/// picture is a checkerboard of navy over it (98's and 2000's their own 16-color drawings,
/// `colors16_art`).
fn desktop_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::desktop_icons::Desktop;
    for era in Era::ALL {
        let theme = bevel_core::theme::theme_in(era, bevel_core::theme::Depth::Sixteen);
        let fonts = fonts_of(era, f95, f2k);
        // My Computer's number among our icons and its row on the PC's desktop.
        let (i, row) = if era == Era::Win2000 { (1, 1) } else { (0, 0) };
        let mut ours = Framebuffer::new(160, 600, theme.colors.desktop);
        let area = Rect::new(0, (row - i as i32) * 75, 160, 572);
        let mut d = Desktop::new(era, area);
        d.active = true;
        d.select_only(Some(i));
        d.focus = Some(i);
        d.keys_used = true;
        let cell = d.cell(i);
        d.paint(&mut ours, theme, fonts, cell);
        let pc = era_name(era);
        let masks = vec![];
        out.push(Case {
            name: format!("{pc}-16-desk-sel-mycomputer").leak(),
            file: format!("vm/{pc}-16color-desk-sel-mycomputer.png").leak(),
            regions: vec![region((cell.x, cell.y), cell)],
            ours,
            masks,
            vga: false,
        });
    }
}

/// Windows 2000's folder window at My Computer (`win2000-16color-folder-mycomputer`, the window at
/// the screen's corner): its toolbar's first three buttons, Back and Forward grayed and Up plain,
/// in its own 16-color drawings (the rest of the row holds buttons Bevel95 leaves out, as in the
/// True Color cases), and the window's frame and title bar.
fn folder_cases(out: &mut Vec<Case>, f2k: &Fonts) {
    use bevel_core::folder::art::Small;
    use bevel_core::folder::menus::Nav;
    REF_DEPTH.set(bevel_core::theme::Depth::Sixteen);
    let nav = Nav { up: true, ..Nav::default() };
    let ours = explorer_window(f2k, Era::Win2000, "My Computer", Small::MyComputer, false, (0, 0), (550, 445), Vec::new(), "My Computer", nav, |_| {});
    REF_DEPTH.set(bevel_core::theme::Depth::TrueColor);
    let regions = vec![region((0, 0), Rect::new(0, 0, 490, 22)), region((0, 49), Rect::new(0, 49, 139, 22))];
    out.push(Case { name: "win2000-16-folder-toolbar", file: "vm/win2000-16color-folder-mycomputer.png", regions, ours, masks: vec![], vga: false });
}

/// The Start menu as it opens (`<pc>-16color-start`), found where it is: 95's whole; 2000's from
/// Windows Update down (its PC had "Set Program Access and Defaults" above it and Log Off off).
/// (98's capture caught the menu while it slid open: not compared.)
fn start_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::config::StartOptions;
    for era in [Era::Win95, Era::Win2000] {
        let theme = bevel_core::theme::theme_in(era, bevel_core::theme::Depth::Sixteen);
        let fonts = fonts_of(era, f95, f2k);
        let opts = StartOptions { favorites: false, log_off: era != Era::Win2000 };
        let mut menu = bevel_core::start::menu(era, opts, "Admin", |e| bevel_core::icons::start_icon(era, e));
        // Both opened from the keyboard: 2000 shows the access keys too.
        menu.show_keys = true;
        let layout = paint::layout_menu(theme, fonts, &menu, MenuStyle::Start, 600);
        let mut fb = Framebuffer::new(layout.width, layout.height, 0);
        paint::paint_menu(&mut fb, theme, fonts, &menu, MenuStyle::Start, &layout);
        let pc = era_name(era);
        let file: &'static str = format!("vm/{pc}-16color-start.png").leak();
        let screen = if era == Era::Win2000 { Rect::new(0, 0, 640, 480) } else { Rect::new(0, 0, 800, 600) };
        let sep = menu.entries.iter().position(|e| e.kind == EntryKind::Separator).map_or(fb.height, |i| layout.rects[i].y);
        let (part, masks) = if era == Era::Win95 {
            // 95: down to the line (its PC had Suspend below it). Masked: the banner, laid from the
            // bottom of that one entry taller menu.
            (Rect::new(0, 0, fb.width, sep), vec![Rect::new(0, 0, theme.start_menu.frame + theme.start_menu.banner_w, sep)])
        } else {
            // 2000: the banner, pictures and labels from Windows Update down (its PC's menu is wider
            // and has another entry above).
            (Rect::new(0, 3, 150, fb.height - 6), vec![])
        };
        located(out, format!("{pc}-16-start-menu").leak(), file, fb, screen, Some(part), &masks);
    }
}

/// Run as it opens (`<pc>-16color-run-inactive-title`), found where it is.
fn run_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    for era in Era::ALL {
        let theme = bevel_core::theme::theme_in(era, bevel_core::theme::Depth::Sixteen);
        let fonts = fonts_of(era, f95, f2k);
        let mut d = Dialog::new(DialogKind::Run, era);
        if era == Era::Win98 {
            // 98's PC had a command in the line, so OK is alive (the text is masked).
            d.text = "explorer".into();
        }
        if era == Era::Win2000 {
            // Opened from the keyboard: 2000 shows the access keys (and its grayed OK stays etched).
            d.show_keys = true;
        }
        let l = dialog::layout(&d);
        let mut fb = Framebuffer::new(l.width, l.height, 0);
        dialog::paint_dialog(&mut fb, theme, fonts, &d, &l);
        let pc = era_name(era);
        let file: &'static str = format!("vm/{pc}-16color-run-inactive-title.png").leak();
        let screen = if era == Era::Win2000 { Rect::new(0, 0, 640, 480) } else { Rect::new(0, 0, 800, 600) };
        // Masked: inside the command line (each PC had its last command in it, and the caret).
        let masks: Vec<Rect> = l.rect(dialog::Part::Command).map(|r| r.inset(2, 2, 2, 2)).into_iter().collect();
        located(out, format!("{pc}-16-run").leak(), file, fb, screen, None, &masks);
    }
}

/// The programs as they open (`<pc>-16color-calc-std`, `-mine-new`), found where they are, the
/// whole window compared.
fn program_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::calculator::form::CalculatorWindow;
    use bevel_core::kit::Form;
    use bevel_core::minefield::form::MinefieldWindow;
    use bevel_core::minefield::game::{Field, Level};
    for era in Era::ALL {
        let theme = bevel_core::theme::theme_in(era, bevel_core::theme::Depth::Sixteen);
        let fonts = fonts_of(era, f95, f2k);
        let pc = era_name(era);
        let screen = if era == Era::Win2000 { Rect::new(0, 0, 640, 480) } else { Rect::new(0, 0, 800, 600) };
        let mut w = CalculatorWindow::new(era);
        w.layout(fonts);
        let (cw, ch) = w.size();
        let mut fb = Framebuffer::new(cw, ch, 0);
        w.paint(&mut fb, theme, fonts);
        let file: &'static str = format!("vm/{pc}-16color-calc-std.png").leak();
        located(out, format!("{pc}-16-calculator").leak(), file, fb, screen, None, &[]);
        if era == Era::Win95 {
            continue; // no games on 95's PC
        }
        let custom = Field { width: 20, height: 12, mines: 40 };
        let mut m = MinefieldWindow::new(era, Level::Beginner, custom, true, 1);
        m.layout(fonts);
        let (mw, mh) = m.size();
        let mut fb = Framebuffer::new(mw, mh, 0);
        m.paint(&mut fb, theme, fonts);
        let file: &'static str = format!("vm/{pc}-16color-mine-new.png").leak();
        located(out, format!("{pc}-16-minesweeper").leak(), file, fb, screen, None, &[]);
    }
}

/// Date/Time Properties (`<pc>-16color-datetime`), found where it is by its title bar, tabs and
/// page frame (the top 60 rows; the calendar, time and hands below hold the PC's moment). Not
/// 2000: its capture shows Explorer instead (a new capture needed).
fn datetime_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::datetime::{self, DateTime, Moment, Part};
    for era in [Era::Win95, Era::Win98] {
        let theme = bevel_core::theme::theme_in(era, bevel_core::theme::Depth::Sixteen);
        let fonts = fonts_of(era, f95, f2k);
        let shown = Moment { year: 2026, month: 11, day: 16, hour: 14, minute: 0, second: 0 };
        let mut d = DateTime::new(era, shown, "Pacific Standard Time".to_string());
        d.focus = Part::Month;
        d.show_keys = true;
        let l = datetime::layout(&d, fonts);
        let mut fb = Framebuffer::new(l.width, l.height, 0);
        datetime::paint(&mut fb, theme, fonts, &d, &l);
        let pc = era_name(era);
        let file: &'static str = format!("vm/{pc}-16color-datetime.png").leak();
        let screen = if era == Era::Win2000 { Rect::new(0, 0, 640, 480) } else { Rect::new(0, 0, 800, 600) };
        located(out, format!("{pc}-16-datetime").leak(), file, fb, screen, Some(Rect::new(0, 0, l.width, 60)), &[]);
    }
}

/// The bar (`win98-16color-taskbar-props-1`: two task buttons after Quick Launch, three tray icons,
/// 2:04 PM): the Start button, the bar's top edge and the clock's part of the well; the task
/// buttons sit elsewhere there (Quick Launch, which Bevel95 has not) and the tray's pictures are
/// the PC's programs'.
fn bar_cases(out: &mut Vec<Case>, f95: &Fonts) {
    let theme = bevel_core::theme::theme_in(Era::Win98, bevel_core::theme::Depth::Sixteen);
    let (fb, layout) = bar(theme, f95, 800, false, vec![task("My Computer", TaskState::Normal)], 3, "2:04 PM");
    let top = 600 - fb.height;
    let s = layout.start;
    let c = layout.clock;
    let regions = vec![
        region((0, 0), Rect::new(0, top, 800, 2)),
        region((0, 0), Rect::new(0, top, s.right() + 2, fb.height)),
        region((c.x, 0), Rect::new(c.x, top, c.w, fb.height)),
    ];
    out.push(Case { name: "win98-16-bar", file: "vm/win98-16color-taskbar-props-1.png", regions, ours: fb, masks: vec![], vga: false });
}

pub(super) fn sixteen_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    bar_cases(out, f95);
    datetime_cases(out, f95, f2k);
    program_cases(out, f95, f2k);
    run_cases(out, f95, f2k);
    desktop_cases(out, f95, f2k);
    folder_cases(out, f2k);
    start_cases(out, f95, f2k);
    for era in Era::ALL {
        let theme = bevel_core::theme::theme_in(era, bevel_core::theme::Depth::Sixteen);
        let fonts = fonts_of(era, f95, f2k);
        let pc = era_name(era);
        let screen = |file: &str| -> Rect {
            let shot = refs_dir().join(file);
            if shot.is_file() {
                let fb = read_png(&shot);
                Rect::new(0, 0, fb.width, fb.height)
            } else {
                Rect::new(0, 0, 800, 600)
            }
        };

        // Shut Down Windows, as it opens (the screen behind it dimmed is left out: only the box).
        let mut d = Dialog::new(DialogKind::ShutDown, era);
        if era == Era::Win2000 {
            // Its PC had chosen Restart last (2000 shows the last choice), and the keyboard was used
            // (so the focus rectangle shows).
            d.choice = 2;
            d.show_keys = true;
        }
        let l = dialog::layout(&d);
        let mut fb = Framebuffer::new(l.width, l.height, 0);
        dialog::paint_dialog(&mut fb, theme, fonts, &d, &l);
        let file: &'static str = format!("vm/{pc}-16color-shutdown.png").leak();
        // 95: its PC had no network, so its box had no "Close all programs and log on as a
        // different user" at the bottom: the title bar, the question and the first three choices are
        // compared. 98: its PC had no Stand by at the top: the title bar and the question.
        let part = (era != Era::Win2000).then(|| Rect::new(0, 0, fb.width, if era == Era::Win95 { 128 } else { 62 }));
        // 2000: masked, the question's underlined W: Windows 2000 shows the focus once a key was used
        // but the underlines only after Alt; Bevel95 shows both at once.
        let masks: &[Rect] = if era == Era::Win2000 { &[Rect::new(68, 120, 12, 3)] } else { &[] };
        located(out, format!("{pc}-16-shut-down").leak(), file, fb, screen(file), part, masks);

        // Taskbar Properties' first page, as it opens from the Start menu (the focus on Always
        // on top). Not on 2000: its capture shows the Start menu instead.
        if era != Era::Win2000 {
            use bevel_core::config::{StartOptions, TaskbarOptions};
            use bevel_core::properties::{self, Check, Part, Properties, Sheet, Values};
            let values = Values {
                options: TaskbarOptions::default(),
                start: StartOptions { favorites: false, log_off: era != Era::Win2000 },
                era,
                colors: bevel_core::theme::Depth::Sixteen,
                scale: 2,
                start_with_windows: false,
                clock_offset: 0,
                desktop: true,
                programs: bevel_core::programs::Shown::Everything,
                windows_key: bevel_core::config::WindowsKey::Find,
                quick_search_all: false,
                battery_percent: false,
                quick_launch: false,
                group: true,
            };
            let mut p = Properties::new(Sheet::Taskbar, era, values);
            p.focus = Part::Check(Check::AlwaysOnTop);
            p.show_keys = true;
            let l = properties::layout(&p, fonts);
            let mut fb = Framebuffer::new(l.width, l.height, 0);
            properties::paint(&mut fb, theme, fonts, &p, &l);
            let file: &'static str = format!("vm/{pc}-16color-taskbar-props-1.png").leak();
            // 95: masked, its "Show clock" (plain 95: "Show Clock"; Bevel95's 95 look has Internet
            // Explorer 4's words). Its preview is plain Windows 95's since 2026-11-18 and compared.
            let masks: &[Rect] = if era == Era::Win95 { &[Rect::new(52, 316, 70, 18)] } else { &[] };
            located(out, format!("{pc}-16-properties").leak(), file, fb, screen(file), None, masks);
        }
    }
}
