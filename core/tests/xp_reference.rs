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

#[test]
#[ignore]
fn the_xp_bar_against_the_screenshots() {
    let mut report = Vec::new();
    compare("GUIdebook running-taskbar", "guidebookgallery/running-taskbar.png", 0, 3, Some(2), 1, &mut report);
    compare("GUIdebook desktop-full", "guidebookgallery/desktop-full.png", 570, 4, Some(3), 1, &mut report);
    compare("Toasty Tech wxpcontrolpanel", "toastytech/wxpcontrolpanel.png", 570, 1, Some(0), 1, &mut report);
    println!("{}", report.join("\n"));
}
