//! Windows XP's bar style (brief 112) against the published screenshots its pictures were cut
//! from (`private/probes/xp/shots`, found through `BEVEL95_PRIVATE`, default the checkout's
//! `private`): the bar painted by Bevel95 for the same buttons, pixel for pixel outside the boxes
//! of letters and icons (the rebuild's masks, `private/probes/xp/tools/rebuild.ps1`), counted by
//! part: pixels that differ by more than 2 levels in a channel. Ignored by default, like
//! `reference.rs`: the screenshots are not part of the repository.
//!
//! `cargo test -p bevel-core --test xp_reference -- --ignored --nocapture`

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use bevel_core::fb::{Framebuffer, Icon, Rect};
use bevel_core::paint::{self, Bar, Task, TaskState};
use bevel_core::text::{Font, Fonts, fixture_name};
use bevel_core::theme::{Theme, WINXP};

fn private_dir() -> PathBuf {
    std::env::var_os("BEVEL95_PRIVATE").map(PathBuf::from).unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../private"))
}

fn read_png(path: &Path) -> Framebuffer {
    let file = BufReader::new(File::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())));
    let mut decoder = png::Decoder::new(file);
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    let channels = match reader.output_color_type().0 {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        _ => 4,
    };
    let mut fb = Framebuffer::new(info.width as i32, info.height as i32, 0);
    for (i, px) in buf[..info.buffer_size()].chunks(channels).enumerate() {
        let (r, g, b) = if channels < 3 { (px[0], px[0], px[0]) } else { (px[0], px[1], px[2]) };
        fb.pixels[i] = 0xFF00_0000 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b);
    }
    fb
}

fn font(theme: &Theme, bold: bool) -> Font {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(fixture_name(theme.font_face, theme.font_px, bold));
    Font::parse(&std::fs::read_to_string(path).unwrap()).unwrap().with_period_glyphs()
}

fn big(a: u32, b: u32) -> bool {
    [16, 8, 0].iter().any(|&s| (((a >> s) & 255) as i32 - ((b >> s) & 255) as i32).abs() > 2)
}

/// Pixels of `part` (in the bar) that differ from the shot's bar at row `oy`, outside `masks`:
/// (compared, differing).
fn count(ours: &Framebuffer, shot: &Framebuffer, oy: i32, part: Rect, masks: &[Rect]) -> (usize, usize) {
    let (mut n, mut d) = (0, 0);
    for y in part.y..part.bottom() {
        for x in part.x..part.right() {
            if masks.iter().any(|m| m.contains(x, y)) {
                continue;
            }
            n += 1;
            if big(ours.get(x, y), shot.get(x, oy + y)) {
                d += 1;
            }
        }
    }
    (n, d)
}

fn task(state: TaskState) -> Task {
    Task { title: "Window".into(), icon: Some(Icon { width: 16, height: 16, pixels: vec![0xFF00_0000; 256] }), state }
}

/// One shot: its file, the bar's row in it, the buttons (the last `front` in front), the
/// notification icons.
fn compare(name: &str, file: &str, oy: i32, buttons: usize, front: Option<usize>, icons: usize, report: &mut Vec<String>) {
    let shot = read_png(&private_dir().join("probes/xp/shots").join(file));
    let (regular, bold) = (font(&WINXP, false), font(&WINXP, true));
    let fonts = Fonts { regular: &regular, bold: &bold };
    let tasks = (0..buttons).map(|i| task(if Some(i) == front { TaskState::Active } else { TaskState::Normal })).collect();
    let tray = (0..icons).map(|_| Some(Icon { width: 16, height: 16, pixels: vec![0xFF00_0000; 256] })).collect();
    let bar = Bar { tasks, tray, clock: "10:35 AM".into(), clock_widest: "12:00 AM".into(), ..Bar::default() };
    let mut ours = Framebuffer::new(800, 30, 0);
    let layout = paint::paint_bar(&mut ours, &WINXP, &fonts, &bar);
    let tray_x = layout.well.x;
    let mut masks: Vec<Rect> = Vec::new();
    for r in &layout.tasks {
        masks.push(Rect::new(r.x + 9, 6, 18, 18)); // the icon
        masks.push(Rect::new(r.x + 29, 8, r.w - 29, 14)); // the label
    }
    masks.push(Rect::new(tray_x + 9, 3, 18 * icons as i32 + 2, 22)); // the icons
    let clock_x = tray_x + 9 + 18 * icons as i32;
    masks.push(Rect::new(clock_x, 8, 800 - clock_x, 13)); // the clock
    let first = layout.tasks.first().map_or(tray_x, |r| r.x);
    let last = layout.tasks.last().map_or(first, |r| r.right());
    let mut parts = vec![("Start button".to_string(), Rect::new(0, 0, 98, 30)), ("gap after Start".to_string(), Rect::new(98, 0, first - 98, 30))];
    for (i, r) in layout.tasks.iter().enumerate() {
        let what = if Some(i) == front { "button in front" } else { "plain button" };
        parts.push((format!("{what} {}", i + 1), Rect::new(r.x, 0, r.w, 30)));
    }
    parts.push(("empty bar".into(), Rect::new(last, 0, tray_x - last, 30)));
    parts.push(("notification area".into(), Rect::new(tray_x, 0, 800 - tray_x, 30)));
    for (what, part) in parts {
        let (n, d) = count(&ours, &shot, oy, part, &masks);
        report.push(format!("{name:<28} {what:<22} {n:>6} compared {d:>5} differ"));
    }
}

/// The Start menu's frame against Toasty Tech's default desktop (the menu at 0,92): the band
/// (the user's picture and name masked), the lines, the body (the left list and the right
/// column's icons and labels masked: they are the stranger's programs and letters), All Programs,
/// the bottom band.
fn compare_menu(report: &mut Vec<String>) {
    let shot = read_png(&private_dir().join("probes/xp/shots/toastytech/wxpdefaultdesk.png"));
    let (regular, bold) = (font(&WINXP, false), font(&WINXP, true));
    let fonts = Fonts { regular: &regular, bold: &bold };
    let menu = bevel_core::xp_start::menu("", &[("A".into(), None), ("B".into(), None)], &(0..6).map(|i| (format!("{i}"), None)).collect::<Vec<_>>());
    let layout = bevel_core::xp_start::layout(&menu);
    let mut ours = Framebuffer::new(layout.width, layout.height, 0);
    paint::paint_menu(&mut ours, &WINXP, &fonts, &menu, paint::MenuStyle::XpStart, &layout);
    let mut masks = vec![
        Rect::new(4, 3, 60, 61),             // the user's picture and its frame (firstrun-2's chess)
        Rect::new(64, 10, 300, 40),           // the user's name
        Rect::new(2, 67 + 4, 188, 329),       // the left list (the stranger's programs)
    ];
    // The rounded corners' outside (the wallpaper in the shot; see-through in the pictures, square
    // in this version: PROVISIONAL): each see-through pixel of the corners and the band's ends.
    for (name, x0) in [("menu_corner_left", 0), ("menu_corner_right", 368), ("menu_band_left", 0), ("menu_band_right", 345)] {
        let p = bevel_core::xp::picture(name).unwrap();
        for y in 0..p.height {
            for x in 0..p.width {
                if p.pixels[(y * p.width + x) as usize] >> 24 == 0 {
                    masks.push(Rect::new(x0 + x, y, 1, 1));
                }
            }
        }
    }
    for r in layout.rects.iter().skip(11).take(12) {
        if r.h == 32 {
            masks.push(Rect::new(r.x + 6, r.y + 3, 26, 26)); // the icon
            masks.push(Rect::new(r.x + 33, r.y + 10, r.w - 33, 14)); // the label and arrow
        }
    }
    let parts = [
        ("top band", Rect::new(0, 0, 380, 64)),
        ("dark + orange lines", Rect::new(0, 64, 380, 3)),
        ("columns, divider, shade", Rect::new(0, 67, 380, 333)),
        ("All Programs", Rect::new(0, 400, 380, 37)),
        ("bottom band", Rect::new(0, 437, 380, 41)),
    ];
    // Where the band's differing pixels are: by row.
    let rows: Vec<String> = (0..64)
        .filter_map(|y| {
            let d = (0..380).filter(|&x| !masks.iter().any(|m| m.contains(x, y)) && big(ours.get(x, y), shot.get(x, 92 + y))).count();
            (d > 0).then(|| format!("{y}:{d}"))
        })
        .collect();
    report.push(format!("top band's differing pixels by row: {}", rows.join(" ")));
    for (what, part) in parts {
        let (n, d) = count_at(&ours, &shot, (0, 92), part, &masks);
        report.push(format!("{:<28} {what:<22} {n:>6} compared {d:>5} differ", "Toasty Tech wxpdefaultdesk"));
    }
}

/// `count` for a picture that stands at `at` in the shot.
fn count_at(ours: &Framebuffer, shot: &Framebuffer, at: (i32, i32), part: Rect, masks: &[Rect]) -> (usize, usize) {
    let (mut n, mut d) = (0, 0);
    for y in part.y..part.bottom() {
        for x in part.x..part.right() {
            if masks.iter().any(|m| m.contains(x, y)) {
                continue;
            }
            n += 1;
            if big(ours.get(x, y), shot.get(at.0 + x, at.1 + y)) {
                d += 1;
            }
        }
    }
    (n, d)
}

/// The second witness (brief 112's review round): GUIdebook's firstrun-2, another installation and
/// another user (the chess pieces, whose frame and picture the menu now takes from this shot): the
/// band outside the name, the frame with its picture, the lines and the columns outside the lists.
/// And the name's letters: the rows its white ink spans, against Tahoma 17 bold.
fn compare_menu_second(report: &mut Vec<String>) {
    let shot = read_png(&private_dir().join("probes/xp/shots/guidebookgallery/firstrun-2.png"));
    let (regular, bold) = (font(&WINXP, false), font(&WINXP, true));
    let fonts = Fonts { regular: &regular, bold: &bold };
    let menu = bevel_core::xp_start::menu("", &[("A".into(), None), ("B".into(), None)], &(0..6).map(|i| (format!("{i}"), None)).collect::<Vec<_>>());
    let layout = bevel_core::xp_start::layout(&menu);
    let mut ours = Framebuffer::new(layout.width, layout.height, 0);
    paint::paint_menu(&mut ours, &WINXP, &fonts, &menu, paint::MenuStyle::XpStart, &layout);
    let mut masks = vec![Rect::new(64, 10, 300, 40), Rect::new(2, 67 + 4, 188, 329)];
    for r in layout.rects.iter().skip(11).take(12) {
        if r.h == 32 {
            masks.push(Rect::new(r.x + 6, r.y + 3, 26, 26));
            masks.push(Rect::new(r.x + 33, r.y + 10, r.w - 33, 14));
        }
    }
    for (name, x0) in [("menu_corner_left", 0), ("menu_corner_right", 368), ("menu_band_left", 0), ("menu_band_right", 345)] {
        let p = bevel_core::xp::picture(name).unwrap();
        for y in 0..p.height {
            for x in 0..p.width {
                if p.pixels[(y * p.width + x) as usize] >> 24 == 0 {
                    masks.push(Rect::new(x0 + x, y, 1, 1));
                }
            }
        }
    }
    let frame = Rect::new(4, 3, 60, 61);
    let band_masks: Vec<Rect> = masks.iter().copied().chain([frame]).collect();
    for (what, part, m) in [
        ("top band (no name)", Rect::new(0, 0, 380, 64), &band_masks),
        ("user frame and picture", frame, &masks),
        ("dark + orange lines", Rect::new(0, 64, 380, 3), &masks),
        ("columns, divider, shade", Rect::new(0, 67, 380, 333), &masks),
    ] {
        let (n, d) = count_at(&ours, &shot, (0, 92), part, m);
        report.push(format!("{:<28} {what:<22} {n:>6} compared {d:>5} differ", "GUIdebook firstrun-2"));
    }
    let ink: Vec<(i32, i32)> =
        (92 + 10..92 + 50).flat_map(|y| (64..360).map(move |x| (x, y))).filter(|&(x, y)| shot.get(x, y) & 0xFF_FFFF >= 0xF0_F0F0).collect();
    let (top, bottom) = (ink.iter().map(|p| p.1).min().unwrap_or(0), ink.iter().map(|p| p.1).max().unwrap_or(0));
    let (left, right) = (ink.iter().map(|p| p.0).min().unwrap_or(0), ink.iter().map(|p| p.0).max().unwrap_or(0));
    let t17 = bevel_core::text::stored("Tahoma", 17, true).unwrap();
    let mut ours17 = Framebuffer::new(200, 40, 0);
    t17.draw(&mut ours17, 0, 0, "Jim Midnite", 0xFFFF_FFFF);
    let our_ink: Vec<(i32, i32)> = (0..40).flat_map(|y| (0..200).map(move |x| (x, y))).filter(|&(x, y)| ours17.get(x, y) & 0xFF_FFFF == 0xFF_FFFF).collect();
    let our_h = our_ink.iter().map(|p| p.1).max().unwrap_or(0) - our_ink.iter().map(|p| p.1).min().unwrap_or(0) + 1;
    let our_w = our_ink.iter().map(|p| p.0).max().unwrap_or(0) - our_ink.iter().map(|p| p.0).min().unwrap_or(0) + 1;
    report.push(format!(
        "GUIdebook firstrun-2 name \"Jim Midnite\": white ink rows {}..{} ({} high), x {left}..{right} ({} wide); Tahoma 17 bold: {our_h} high, {our_w} wide, its ink {} rows below where it is drawn",
        top - 92,
        bottom - 92,
        bottom - top + 1,
        right - left + 1,
        our_ink.iter().map(|p| p.1).min().unwrap_or(0)
    ));
}

#[test]
#[ignore]
fn the_xp_start_menu_against_the_screenshot() {
    let mut report = Vec::new();
    compare_menu(&mut report);
    compare_menu_second(&mut report);
    println!("{}", report.join("\n"));
}

#[test]
#[ignore]
fn the_xp_bar_against_the_screenshots() {
    let mut report = Vec::new();
    compare("GUIdebook running-taskbar", "guidebookgallery/running-taskbar.png", 0, 3, Some(2), 1, &mut report);
    compare("GUIdebook desktop-full", "guidebookgallery/desktop-full.png", 570, 4, Some(3), 1, &mut report);
    compare("Toasty Tech wxpcontrolpanel", "toastytech/wxpcontrolpanel.png", 570, 1, Some(0), 1, &mut report);
    println!("{}", report.join("\n"));
}
