//! The 256 Colors option against the Reference PCs at 256 colors (`vm/<pc>-256color-*`, taken
//! 2026-11-20 after each PC's restart in 256 colors; private/probes/colors256/survey.md,
//! docs/theme-spec.md "256 Colors"). All three are 800×600.
//!
//! Windows 95's and 98's captures show Windows' own values. Windows 2000's emulator showed each
//! channel at 6 bits, an odd step 3 higher (its face D4D0C8 as D7D0C8, its desktop 3A6EA5 as
//! 386FA7, the halftone 66 / 99 / CC as 67 / 98 / CF): `shown` rounds our picture the same way
//! before a 2000 case compares it, so a case fails on any other difference.
//!
//! Masked, each with its reason in the case: the PCs' own pictures of programs and files, the
//! clock's time, the pointer, and what Bevel95 leaves out.

use super::*;
use bevel_core::theme::Depth;

/// What Windows 2000's emulator showed for a color at 256 colors: each channel at 6 bits, an odd
/// step 3 higher (D4 -> D7, 3A -> 38, CC -> CF; 80 and FF as they are).
pub(super) fn r6(p: u32) -> u32 {
    let ch = |s: u32| {
        let q = (p >> s) & 0xFF;
        let q = q >> 2;
        (q * 4 + (q & 1) * 3) << s
    };
    0xFF00_0000 | ch(16) | ch(8) | ch(0)
}

/// Our picture as the PC's screen showed it: 2000's rounded (`r6`), 95's and 98's as they are.
fn shown(era: Era, mut fb: Framebuffer) -> Framebuffer {
    if era == Era::Win2000 {
        for p in &mut fb.pixels {
            // See-through pixels (0) stay see-through.
            if *p != 0 {
                *p = r6(*p);
            }
        }
    }
    fb
}

fn theme(era: Era) -> &'static Theme {
    bevel_core::theme::theme_in(era, Depth::TwoFiftySix)
}

fn fonts_of<'a>(era: Era, f95: &'a Fonts<'a>, f2k: &'a Fonts<'a>) -> &'a Fonts<'a> {
    if era == Era::Win2000 { f2k } else { f95 }
}

fn file_of(era: Era, scene: &str) -> &'static str {
    format!("vm/{}-256color-{scene}.png", era_name(era)).leak()
}

fn name_of(era: Era, what: &str) -> &'static str {
    format!("{}-256-{what}", era_name(era)).leak()
}

const SCREEN: Rect = Rect::new(0, 0, 800, 600);

/// One case found where it is on the capture (`sixteen_cases::locate`), masks relative to ours.
fn located(out: &mut Vec<Case>, era: Era, name: &'static str, file: &'static str, ours: Framebuffer, part: Option<Rect>, masks: &[Rect]) {
    let ours = shown(era, ours);
    let part = part.unwrap_or(Rect::new(0, 0, ours.width, ours.height));
    let path = refs_dir().join(file);
    if !path.is_file() {
        out.push(Case { name, file, regions: vec![], ours, masks: vec![], vga: false });
        return;
    }
    let shot = read_png(&path);
    let mut piece = Framebuffer::new(part.w, part.h, 0);
    piece.blit_part(&ours, part, 0, 0);
    let skip: Vec<Rect> = masks.iter().map(|m| Rect::new(m.x - part.x, m.y - part.y, m.w, m.h)).collect();
    let at = sixteen_cases::locate(&shot, &piece, SCREEN, &skip).map_or((0, 0), |(x, y)| (x - part.x, y - part.y));
    let masks = masks.iter().map(|m| Rect::new(at.0 + m.x, at.1 + m.y, m.w, m.h)).collect();
    let region = region((part.x, part.y), Rect::new(at.0 + part.x, at.1 + part.y, part.w, part.h));
    out.push(Case { name, file, regions: vec![region], ours, masks, vga: false });
}

/// My Computer selected on the desktop (`-desk-sel-mycomputer`), its cell, as the 16-color case:
/// the navy checkerboard over the look's own 16-color drawing (2000 on its own desktop color).
/// And Windows 2000's focus rectangle on its desktop (`win2000-256color-desktop-2`: My Documents
/// focused, nothing selected): money green dots.
fn desktop_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::desktop_icons::Desktop;
    for era in Era::ALL {
        let t = theme(era);
        let fonts = fonts_of(era, f95, f2k);
        let (i, row) = if era == Era::Win2000 { (1, 1) } else { (0, 0) };
        let mut ours = Framebuffer::new(160, 600, t.colors.desktop);
        let area = Rect::new(0, (row - i as i32) * 75, 160, 572);
        let mut d = Desktop::new(era, area);
        d.active = true;
        d.select_only(Some(i));
        d.focus = Some(i);
        d.keys_used = true;
        let cell = d.cell(i);
        d.paint(&mut ours, t, fonts, cell);
        out.push(Case {
            name: name_of(era, "desk-sel-mycomputer"),
            file: file_of(era, "desk-sel-mycomputer"),
            regions: vec![region((cell.x, cell.y), cell)],
            ours: shown(era, ours),
            masks: vec![],
            vga: false,
        });
    }
    let era = Era::Win2000;
    let t = theme(era);
    let mut ours = Framebuffer::new(160, 600, t.colors.desktop);
    let mut d = Desktop::new(era, Rect::new(0, 0, 160, 572));
    d.active = true;
    d.select_only(None);
    d.focus = Some(0);
    d.keys_used = true;
    let cell = d.cell(0);
    d.paint(&mut ours, t, f2k, cell);
    out.push(Case {
        name: "win2000-256-desk-focus",
        file: "vm/win2000-256color-desktop-2.png",
        regions: vec![region((cell.x, cell.y), cell)],
        ours: shown(era, ours),
        masks: vec![],
        vga: false,
    });
}

/// The folder windows' rebar at My Computer (`-folder-mycomputer-2`, the window at the screen's
/// corner): the 16-color toolbar (black line art), the address bar with 2000's line-art Go button,
/// and the throbber as each PC drew it at 256 colors; 98's hot Cut (`win98-256color-folder-hot-
/// cut`: colored, as 16 colors' hot picture). Masked as in the True Color cases: what Bevel95
/// leaves out (98's Favorites and Help titles; 2000's buttons after Up and its Favorites, Tools
/// and Help titles), 2000's address bar picture.
fn folder_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::folder::Tool;
    use bevel_core::folder::art::Small;
    use bevel_core::folder::menus::Nav;
    REF_DEPTH.set(Depth::TwoFiftySix);
    for (name, file, era, size, hot, ptr) in [
        ("win98-256-folder-toolbar", "vm/win98-256color-folder-mycomputer-2.png", Era::Win98, (512, 461), None, None),
        ("win98-256-folder-hot-cut", "vm/win98-256color-folder-hot-cut.png", Era::Win98, (512, 461), Some(Tool::Cut), Some((259, 80))),
        ("win2000-256-folder-toolbar", "vm/win2000-256color-folder-mycomputer-2.png", Era::Win2000, (680, 458), None, None),
    ] {
        let fonts = fonts_of(era, f95, f2k);
        let nav = Nav { up: era == Era::Win2000, ..Nav::default() };
        let ours = explorer_window(fonts, era, "My Computer", Small::MyComputer, false, (0, 0), size, Vec::new(), "My Computer", nav, |w| {
            if let Some(t) = hot {
                w.bar.hot_tool = Some((t, false));
            }
        });
        let rebar = Rect::new(0, 22, size.0, if era == Era::Win98 { 94 } else { 76 });
        let mut masks = Vec::new();
        if era == Era::Win98 {
            masks.push(Rect::new(158, 25, 308, 22));
        } else {
            masks.extend([Rect::new(139, 49, size.0 - 145, 22), Rect::new(120, 25, 514, 22), Rect::new(60, 75, 18, 18)]);
        }
        if let Some((x, y)) = ptr {
            // The pointer (below Cut, at the rebar's bottom; no tooltip had come yet).
            masks.push(Rect::new(x - 1, y - 1, 14, 22));
        }
        let regions = vec![region((rebar.x, rebar.y), rebar)];
        out.push(Case { name, file, regions, ours: shown(era, ours), masks, vga: false });
    }
    REF_DEPTH.set(Depth::TrueColor);
}

/// The web view panel at 256 colors: 98's My Computer with (C:) selected (`win98-256color-folder-
/// mc-sel-drive`: the dithered watercolor, True Color's line, the legend and pie in silver), and
/// 2000's with nothing selected (`win2000-256color-folder-mycomputer-2`: its dithered corner picture,
/// True Color's line, the infotip dotted white and yellow). Only the panel is compared (the rest of
/// the window is the toolbar case's and the icons the PCs' drives).
fn web_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::folder::Space;
    use bevel_core::folder::art::Small;
    use bevel_core::kit::Form;
    for (era, file, size) in [
        (Era::Win98, "vm/win98-256color-folder-mc-sel-drive.png", (512, 461)),
        (Era::Win2000, "vm/win2000-256color-folder-mycomputer-2.png", (680, 458)),
    ] {
        let fonts = fonts_of(era, f95, f2k);
        let list = if era == Era::Win98 { web_cases::my_computer_98() } else { web_cases::my_computer_2000() };
        let w = web_cases::web_window(fonts, era, (0, 0), size, "My Computer", "My Computer", Small::MyComputer, false, list, |w| {
            if era == Era::Win98 {
                // 98's PC: (C:) 0.99 GB, 247 MB used.
                w.spaces.insert("C:\\".into(), Space { free: 813_694_976, capacity: 1_072_693_248 });
                web_cases::select(w, &[1]);
            }
        });
        let mut ours = Framebuffer::new(w.size_now().0, w.size_now().1, 0);
        w.paint(&mut ours, theme(era), fonts);
        // The panel where both PCs had it (98: 150 wide with its scroll bars; 2000: 200 wide).
        let panel = if era == Era::Win98 { Rect::new(6, 120, 150, 314) } else { Rect::new(6, 102, 200, 330) };
        let mut masks = vec![Rect::new(panel.x + if era == Era::Win98 { 15 } else { 12 }, panel.y + if era == Era::Win98 { 15 } else { 11 }, 32, 32)];
        if era == Era::Win98 {
            // The pie (GDI's Ellipse and Pie, as in the True Color cases) and the capacity's words
            // (its PC's 0.99 GB, Bevel95's rounding of the same bytes is not settled here).
            masks.push(Rect::new(panel.x + 10, panel.y + 270, 130, 40));
            // The panel's bottom: 98 gave it a horizontal scroll bar here (the pie wider than the
            // panel) and shortened its vertical one; Bevel95's panel has no horizontal scroll bar
            // (a layout matter, the same in True Color; not a matter of the colors).
            masks.push(Rect::new(panel.x, panel.y + 275, panel.w, 40));
            masks.push(Rect::new(panel.right() - 16, panel.y + 200, 16, 75)); // the thumb's end, likewise
        }
        out.push(Case { name: name_of(era, "web-panel"), file, regions: vec![region((panel.x, panel.y), panel)], ours: shown(era, ours), masks, vga: false });
    }
}

/// The Start menu as it opens (`-start`): 95's down to the line, 2000's from Windows Update down
/// (as the 16-color cases; 98's was caught sliding open).
fn start_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::config::StartOptions;
    for era in [Era::Win95, Era::Win2000] {
        let t = theme(era);
        let fonts = fonts_of(era, f95, f2k);
        let opts = StartOptions { favorites: false, log_off: era != Era::Win2000 };
        let mut menu = bevel_core::start::menu(era, opts, "Admin", |e| bevel_core::icons::start_icon(era, e));
        menu.show_keys = true;
        let layout = paint::layout_menu(t, fonts, &menu, MenuStyle::Start, 600);
        let mut fb = Framebuffer::new(layout.width, layout.height, 0);
        paint::paint_menu(&mut fb, t, fonts, &menu, MenuStyle::Start, &layout);
        let sep = menu.entries.iter().position(|e| e.kind == EntryKind::Separator).map_or(fb.height, |i| layout.rects[i].y);
        let (part, masks) = if era == Era::Win95 {
            (Rect::new(0, 0, fb.width, sep), vec![Rect::new(0, 0, t.start_menu.frame + t.start_menu.banner_w, sep)])
        } else {
            (Rect::new(0, 3, 150, fb.height - 6), vec![])
        };
        located(out, era, name_of(era, "start-menu"), file_of(era, "start"), fb, Some(part), &masks);
    }
}

/// Run as it opens (`-run-inactive-title`), found where it is.
fn run_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    for era in Era::ALL {
        let t = theme(era);
        let fonts = fonts_of(era, f95, f2k);
        // Each PC's command line was empty at 256 colors (OK grayed).
        let mut d = Dialog::new(DialogKind::Run, era);
        if era == Era::Win2000 {
            d.show_keys = true;
        }
        let l = dialog::layout(&d);
        let mut fb = Framebuffer::new(l.width, l.height, 0);
        dialog::paint_dialog(&mut fb, t, fonts, &d, &l);
        // Masked: inside the command line (each PC had its last command in it, and the caret).
        let masks: Vec<Rect> = l.rect(dialog::Part::Command).map(|r| r.inset(2, 2, 2, 2)).into_iter().collect();
        located(out, era, name_of(era, "run"), file_of(era, "run-inactive-title"), fb, None, &masks);
    }
}

/// The programs as they open: Calculator (`-calc-std`; 98's capture shows the Scientific view, its
/// Calculator kept it) and Minesweeper (`-mine-new`, 98 and 2000).
fn program_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::calculator::form::CalculatorWindow;
    use bevel_core::kit::Form;
    use bevel_core::minefield::form::MinefieldWindow;
    use bevel_core::minefield::game::{Field, Level};
    for era in Era::ALL {
        let t = theme(era);
        let fonts = fonts_of(era, f95, f2k);
        let mut w = CalculatorWindow::new(era);
        if era == Era::Win98 {
            w.set_scientific(true);
        }
        w.layout(fonts);
        let (cw, ch) = w.size();
        let mut fb = Framebuffer::new(cw, ch, 0);
        w.paint(&mut fb, t, fonts);
        let scene = if era == Era::Win98 { "calc-sci" } else { "calc-std" };
        located(out, era, name_of(era, "calculator"), file_of(era, scene), fb, None, &[]);
        if era == Era::Win95 {
            continue; // no games on 95's PC
        }
        let custom = Field { width: 20, height: 12, mines: 40 };
        let mut m = MinefieldWindow::new(era, Level::Beginner, custom, true, 1);
        m.layout(fonts);
        let (mw, mh) = m.size();
        let mut fb = Framebuffer::new(mw, mh, 0);
        m.paint(&mut fb, t, fonts);
        located(out, era, name_of(era, "minesweeper"), file_of(era, "mine-new"), fb, None, &[]);
    }
}

/// Date/Time Properties (`-datetime`), found by its title bar, tabs and page frame (the top 60
/// rows; below them the PC's moment).
fn datetime_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::datetime::{self, DateTime, Moment, Part};
    for era in Era::ALL {
        let t = theme(era);
        let fonts = fonts_of(era, f95, f2k);
        let shown_at = Moment { year: 2026, month: 11, day: 20, hour: 16, minute: 0, second: 0 };
        let mut d = DateTime::new(era, shown_at, "Pacific Standard Time".to_string());
        d.focus = Part::Month;
        d.show_keys = true;
        let l = datetime::layout(&d, fonts);
        let mut fb = Framebuffer::new(l.width, l.height, 0);
        datetime::paint(&mut fb, t, fonts, &d, &l);
        located(out, era, name_of(era, "datetime"), file_of(era, "datetime"), fb, Some(Rect::new(0, 0, l.width, 60)), &[]);
    }
}

/// The bar (`-taskbar-props-1`): the Start button, the bar's top edge and the clock's part of the
/// well (the task buttons sit after Quick Launch there, and the tray's pictures are the PC's).
fn bar_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    for (era, tray, clock) in [(Era::Win98, 3, "4:00 PM"), (Era::Win2000, 1, "4:27 PM")] {
        let t = theme(era);
        let fonts = fonts_of(era, f95, f2k);
        let (fb, layout) = bar(t, fonts, 800, false, vec![task("My Computer", TaskState::Normal)], tray, clock);
        let top = 600 - fb.height;
        let s = layout.start;
        let c = layout.clock;
        let regions = vec![
            region((0, 0), Rect::new(0, top, 800, 2)),
            region((0, 0), Rect::new(0, top, s.right() + 2, fb.height)),
            region((c.x, 0), Rect::new(c.x, top, c.w, fb.height)),
        ];
        // The clock's time (each capture's own moment).
        let masks = vec![Rect::new(c.x + 2, top + 4, c.w - 4, fb.height - 8)];
        out.push(Case { name: name_of(era, "bar"), file: file_of(era, "taskbar-props-1"), regions, ours: shown(era, fb), masks, vga: false });
    }
}

/// Shut Down Windows and Taskbar Properties' first page, as the 16-color cases compare them.
fn dialog_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    for era in Era::ALL {
        let t = theme(era);
        let fonts = fonts_of(era, f95, f2k);
        let mut d = Dialog::new(DialogKind::ShutDown, era);
        if era == Era::Win2000 {
            d.show_keys = true;
        }
        let l = dialog::layout(&d);
        let mut fb = Framebuffer::new(l.width, l.height, 0);
        dialog::paint_dialog(&mut fb, t, fonts, &d, &l);
        // 95: no network on its PC (no last choice); 98: no Stand by at the top: as the 16-color
        // cases, the title bar and what agrees.
        let part = (era != Era::Win2000).then(|| Rect::new(0, 0, fb.width, if era == Era::Win95 { 128 } else { 62 }));
        // 2000: the question's underlined W (Bevel95 shows the underlines with the focus).
        let masks: &[Rect] = if era == Era::Win2000 { &[Rect::new(68, 120, 12, 3)] } else { &[] };
        located(out, era, name_of(era, "shut-down"), file_of(era, "shutdown"), fb, part, masks);

        use bevel_core::config::{StartOptions, TaskbarOptions};
        use bevel_core::properties::{self, Check, Part, Properties, Sheet, Values};
        let values = Values {
            options: TaskbarOptions::default(),
            start: StartOptions { favorites: false, log_off: era != Era::Win2000 },
            era,
            colors: Depth::TwoFiftySix,
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
        properties::paint(&mut fb, t, fonts, &p, &l);
        // 95: its "Show clock" (as the 16-color case). 2000: "Use Personalized Menus", ticked and
        // alive on its PC, always grayed in Bevel95 (it has no personalized menus).
        let masks: Vec<Rect> = match era {
            Era::Win95 => vec![Rect::new(52, 316, 70, 18)],
            Era::Win2000 => l.rect(Part::Check(Check::PersonalizedMenus)).map(|r| r.inset(-2, -2, -2, -2)).into_iter().collect(),
            Era::Win98 => vec![],
        };
        located(out, era, name_of(era, "properties"), file_of(era, "taskbar-props-1"), fb, None, &masks);
    }
}

/// 98's clock tooltip (`win98-256color-tooltip-clock`: white dotted with yellow, as 16 colors),
/// where the True Color case puts it; and 95's Help topic (`win95-256color-calc-help-topic`: its
/// pale yellow paper dotted white and yellow), the window at (510, 59) as the True Color case's.
fn tooltip_and_help_cases(out: &mut Vec<Case>, f95: &Fonts) {
    let text = "Friday, November 20, 2026";
    let (w, h) = tooltip::size(f95.regular, text);
    let mut ours = Framebuffer::new(w, h, 0);
    tooltip::paint(&mut ours, theme(Era::Win98), f95.regular, text);
    // Found near the clock (the pointer below it is not in the tooltip).
    let file = "vm/win98-256color-tooltip-clock.png";
    let path = refs_dir().join(file);
    let at = if path.is_file() { sixteen_cases::locate(&read_png(&path), &ours, Rect::new(500, 500, 300, 100), &[]).unwrap_or((0, 0)) } else { (0, 0) };
    out.push(Case { name: "win98-256-tooltip-clock", file, regions: vec![whole(&ours, at)], ours, masks: vec![], vga: false });

    use bevel_core::calculator::help::catalog;
    use bevel_core::help::winhelp::HelpTopics;
    use bevel_core::kit::Form;
    let book = catalog(Era::Win95);
    let mut viewer = HelpTopics::article(Era::Win95, book, 0).unwrap();
    let at = (510, 59);
    viewer.set_article_bounds(281, 508);
    viewer.set_screen(at.0, at.1);
    viewer.layout(f95);
    let (w, h) = viewer.size();
    let mut ours = Framebuffer::new(w, h, 0);
    viewer.paint(&mut ours, theme(Era::Win95), f95);
    let _ = h;
    // The window down to the five steps: below them the shared viewer's own wording reflows (its
    // Tips lines one row apart from CALC.HLP's). Masked: "operators", a glossary link in CALC.HLP
    // (green, dotted), plain text in the shared book.
    let regions = vec![region((0, 0), Rect::new(at.0, at.1, w, 190))];
    // And step 5's "=." (four pixels: CALC.HLP's text there is not the shared book's; not a matter
    // of the colors).
    let masks = vec![Rect::new(638, 206, 52, 14), Rect::new(574, 226, 8, 9)];
    out.push(Case { name: "win95-256-help-topic", file: "vm/win95-256color-calc-help-topic.png", regions, ours, masks, vga: false });
}

pub(super) fn twofiftysix_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    desktop_cases(out, f95, f2k);
    folder_cases(out, f95, f2k);
    web_cases(out, f95, f2k);
    start_cases(out, f95, f2k);
    run_cases(out, f95, f2k);
    program_cases(out, f95, f2k);
    datetime_cases(out, f95, f2k);
    bar_cases(out, f95, f2k);
    dialog_cases(out, f95, f2k);
    tooltip_and_help_cases(out, f95);
}
