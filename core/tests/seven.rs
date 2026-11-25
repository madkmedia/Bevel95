//! Windows 7 Basic's taskbar (`bevel_core::seven`): its golden pictures (new files
//! `tests/golden/seven-*.png`, written only by the ignored `write_seven_golden`), and the
//! comparison with the two screenshots it was measured from (ignored: the screenshots are
//! private, found through BEVEL95_PRIVATE or the `private` folder above this checkout).

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use bevel_core::fb::{Framebuffer, Icon, Rect};
use bevel_core::seven::{self, Orb, SevenBar, Slot, SlotKind};
use bevel_core::text::{self, Font};

/// The stand-in for Windows 7's Segoe UI 9 pt (PROVISIONAL): the nearest stored face.
fn clock_font() -> Font {
    text::stored("Tahoma", 11, false).expect("Tahoma 11 is stored")
}

fn read_png(path: &Path) -> Framebuffer {
    let file = BufReader::new(File::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())));
    let mut reader = png::Decoder::new(file).read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    let step = match info.color_type {
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        other => panic!("{}: {other:?}", path.display()),
    };
    assert_eq!(info.bit_depth, png::BitDepth::Eight);
    let pixels = buf[..info.line_size * info.height as usize]
        .chunks(step)
        .map(|c| 0xFF00_0000 | (u32::from(c[0]) << 16) | (u32::from(c[1]) << 8) | u32::from(c[2]))
        .collect();
    Framebuffer { width: info.width as i32, height: info.height as i32, pixels }
}

fn write_png(path: &Path, fb: &Framebuffer) {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, fb.width as u32, fb.height as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    let data: Vec<u8> = fb.pixels.iter().flat_map(|&p| [(p >> 16) as u8, (p >> 8) as u8, p as u8]).collect();
    writer.write_image_data(&data).unwrap();
    writer.finish().unwrap();
    std::fs::write(path, bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
}

// ---- test pictures (made up here, not Windows'): plain shapes standing in for program icons ----

/// A 32x32 stand-in for a program's picture: a colored square with a darker edge and corners
/// left see-through.
fn program_icon(color: u32) -> Icon {
    let dark = 0xFF00_0000 | ((color & 0xFE_FEFE) >> 1);
    let pixels = (0..32 * 32)
        .map(|i| {
            let (x, y) = (i % 32, i / 32);
            let edge = x == 2 || y == 2 || x == 29 || y == 29;
            let inside = (2..=29).contains(&x) && (2..=29).contains(&y);
            if !inside || ((x == 2 || x == 29) && (y == 2 || y == 29)) {
                0
            } else if edge {
                dark
            } else {
                0xFF00_0000 | color
            }
        })
        .collect();
    Icon { width: 32, height: 32, pixels }
}

/// A 16x16 stand-in for a notification icon.
fn tray_icon(color: u32) -> Icon {
    let pixels = (0..16 * 16)
        .map(|i| {
            let (x, y) = (i % 16, i / 16);
            if (2..14).contains(&x) && (3..13).contains(&y) { 0xFF00_0000 | color } else { 0 }
        })
        .collect();
    Icon { width: 16, height: 16, pixels }
}

const BLUE: u32 = 0x2D6FD0;
const YELLOW: u32 = 0xE8C040;
const ORANGE: u32 = 0xE87020;
const GREEN: u32 = 0x40A040;
const GRAY: u32 = 0x909090;

fn pinned_three() -> Vec<Slot> {
    vec![
        Slot { kind: SlotKind::Pinned, icon: Some(program_icon(BLUE)) },
        Slot { kind: SlotKind::Pinned, icon: Some(program_icon(YELLOW)) },
        Slot { kind: SlotKind::Pinned, icon: Some(program_icon(ORANGE)) },
    ]
}

fn draw(bar: &SevenBar, width: i32) -> Framebuffer {
    let l = seven::layout(width, bar.slots.len(), bar.tray_icons.len(), bar.clock_space);
    let mut fb = Framebuffer::new(width, seven::HEIGHT, 0xFF00_0000);
    seven::paint(&mut fb, bar, &l, &clock_font());
    fb
}

/// The golden scenes: the bar 1024 wide.
fn scenes() -> Vec<(&'static str, Framebuffer)> {
    let font = clock_font();
    let clock = |time: &str, date: &str, icons: Vec<Icon>| SevenBar {
        orb: Orb::Normal,
        slots: Vec::new(),
        tray_icons: icons,
        time: time.into(),
        date: date.into(),
        clock_space: seven::clock_space_for(&font, time, date),
    };
    // 1: empty, the three programs Windows pins
    let mut empty = clock("10:05 AM", "11/25/2026", vec![tray_icon(GRAY), tray_icon(GREEN)]);
    empty.slots = pinned_three();
    // 2: two running and one in front (the second pinned program is in front)
    let mut busy = clock("10:05 AM", "11/25/2026", vec![tray_icon(GRAY), tray_icon(GREEN)]);
    busy.slots = pinned_three();
    busy.slots[0].kind = SlotKind::Running;
    busy.slots[1].kind = SlotKind::Front;
    busy.slots.push(Slot { kind: SlotKind::Running, icon: Some(program_icon(GREEN)) });
    // 3: a wide notification area (seven icons) and the Start menu open
    let icons = [GRAY, GREEN, BLUE, ORANGE, YELLOW, GRAY, GREEN].iter().map(|&c| tray_icon(c)).collect();
    let mut wide = clock("12:59 PM", "12/31/2026", icons);
    wide.slots = pinned_three();
    wide.orb = Orb::MenuOpen;
    vec![
        ("seven-bar-pinned", draw(&empty, 1024)),
        ("seven-bar-running", draw(&busy, 1024)),
        ("seven-bar-wide-tray", draw(&wide, 1024)),
    ]
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("golden")
}

#[test]
fn seven_golden_images_match() {
    let mut failures = Vec::new();
    for (name, image) in scenes() {
        let path = golden_dir().join(format!("{name}.png"));
        if !path.exists() {
            failures.push(format!(
                "{name}: missing; `cargo test -p bevel-core --test seven -- --ignored write_seven_golden` writes it"
            ));
            continue;
        }
        let expected = read_png(&path);
        let differ = expected.pixels.iter().zip(&image.pixels).filter(|(a, b)| (*a ^ *b) & 0xFF_FFFF != 0).count();
        if (expected.width, expected.height) != (image.width, image.height) || differ > 0 {
            write_png(&path.with_extension("actual.png"), &image);
            failures.push(format!("{name}: {differ} pixels differ"));
        }
    }
    assert!(failures.is_empty(), "\n{}\n", failures.join("\n"));
}

#[test]
#[ignore = "writes tests/golden/seven-*.png (only these); run it on request"]
fn write_seven_golden() {
    for (name, image) in scenes() {
        let path = golden_dir().join(format!("{name}.png"));
        write_png(&path, &image);
        println!("wrote {}", path.display());
    }
}

// ---- the comparison with the screenshots ----

/// The survey's folder: BEVEL95_PRIVATE/probes/win7, or `private/probes/win7` in this folder or
/// any folder above it (a worktree inside the main checkout's `private`).
fn survey() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("BEVEL95_PRIVATE") {
        let d = PathBuf::from(p).join("probes").join("win7");
        if d.join("shots").exists() {
            return Some(d);
        }
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .map(|a| a.join("private").join("probes").join("win7"))
        .find(|d| d.join("shots").exists())
}

/// Paints the bar of a screenshot and counts, by part, the pixels that differ from it outside the
/// letter and icon boxes. Returns (part, equal, differ, masked) rows.
fn compare(shot: &Framebuffer, top: i32, bar: &SevenBar, letters: Rect) -> Vec<(String, usize, usize, usize)> {
    let w = shot.width;
    let l = seven::layout(w, bar.slots.len(), bar.tray_icons.len(), bar.clock_space);
    let mut fb = Framebuffer::new(w, seven::HEIGHT, 0xFF00_0000);
    seven::paint(&mut fb, bar, &l, &clock_font());
    let mut rows: Vec<(String, usize, usize, usize)> = Vec::new();
    for y in 0..seven::HEIGHT {
        for x in 0..w {
            let in_slot = l.slots.iter().position(|r| r.contains(x, y));
            let icon_box = in_slot.is_some_and(|i| {
                Rect::new(l.slots[i].x + seven::ICON_X, seven::ICON_Y, seven::ICON, seven::ICON).contains(x, y)
            }) || l.tray_icons.iter().any(|r| r.contains(x, y));
            let masked = icon_box || letters.contains(x, y);
            let part = if x < seven::ORB_W {
                "orb".to_string()
            } else if let Some(i) = in_slot {
                let over_fade = l.slots[i].x <= seven::LEFT_ZONE_END + seven::FADE;
                match bar.slots[i].kind {
                    SlotKind::Pinned => "pinned slot (the bar behind)".to_string(),
                    SlotKind::Front => "button in front".to_string(),
                    SlotKind::Running if over_fade => "running button over the fade".to_string(),
                    SlotKind::Running => "running button".to_string(),
                }
            } else if l.arrow.contains(x, y) {
                "tray arrow".to_string()
            } else if x >= l.show_desktop.x {
                "show desktop".to_string()
            } else if y < 2 {
                "top lines".to_string()
            } else if x <= seven::LEFT_ZONE_END + seven::FADE {
                "left fade".to_string()
            } else if x >= l.zone {
                "notification zone (clock included)".to_string()
            } else if x >= l.zone - seven::FADE {
                "right fade".to_string()
            } else {
                "body".to_string()
            };
            let idx = match rows.iter().position(|r| r.0 == part) {
                Some(i) => i,
                None => {
                    rows.push((part, 0, 0, 0));
                    rows.len() - 1
                }
            };
            if masked {
                rows[idx].3 += 1;
            } else if (fb.get(x, y) ^ shot.get(x, top + y)) & 0xFF_FFFF == 0 {
                rows[idx].1 += 1;
            } else {
                rows[idx].2 += 1;
            }
        }
    }
    rows.sort();
    rows
}

#[test]
#[ignore = "reads the private screenshots of the Windows 7 survey (BEVEL95_PRIVATE or private/probes/win7)"]
fn seven_bar_matches_the_screenshots() {
    let dir = survey().expect("the survey's folder (private/probes/win7) is not found");
    let none = |kind| Slot { kind, icon: None };
    // [A] 7600, 1920x1080, bar at y=1040: IE running, Explorer in front, Media Player pinned,
    // three running; four notification icons; the clock's letters at x 1836..1893, rows 8..34.
    let a = SevenBar {
        orb: Orb::Normal,
        slots: vec![
            none(SlotKind::Running),
            none(SlotKind::Front),
            none(SlotKind::Pinned),
            none(SlotKind::Running),
            none(SlotKind::Running),
            none(SlotKind::Running),
        ],
        tray_icons: vec![Icon { width: 16, height: 16, pixels: vec![0; 256] }; 4],
        time: String::new(),
        date: String::new(),
        clock_space: 79,
    };
    // [B] 7601, 1440x900, bar at y=860, Start menu open: three pinned, Control Panel running; two
    // notification icons; the clock's letters at x 1364..1415, rows 8..34.
    let b = SevenBar {
        orb: Orb::MenuOpen,
        slots: vec![none(SlotKind::Pinned), none(SlotKind::Pinned), none(SlotKind::Pinned), none(SlotKind::Running)],
        tray_icons: vec![Icon { width: 16, height: 16, pixels: vec![0; 256] }; 2],
        time: String::new(),
        date: String::new(),
        clock_space: 70,
    };
    let shots = [
        ("[A]", "Windows_7_basic_theme.png", 1040, a, Rect::new(1836, 8, 58, 27)),
        ("[B]", "7601-ThemeBasic.png", 860, b, Rect::new(1364, 8, 52, 27)),
    ];
    let mut wrong = Vec::new();
    for (name, file, top, bar, letters) in shots {
        let shot = read_png(&dir.join("shots").join("betawiki").join(file));
        println!("{name} {file}: part | equal | differ | masked");
        for (part, eq, differ, masked) in compare(&shot, top, &bar, letters) {
            println!("  {part:<40} {eq:>7} {differ:>7} {masked:>7}");
            if differ > 0 {
                wrong.push(format!("{name} {part}: {differ} differ"));
            }
        }
    }
    assert!(wrong.is_empty(), "\n{}\n", wrong.join("\n"));
}

/// The stand-in letters against the screenshots' clock (not a pass/fail: printed for the notes).
#[test]
#[ignore = "reads the private screenshots of the Windows 7 survey"]
fn seven_clock_letters_against_the_screenshots() {
    let dir = survey().expect("the survey's folder");
    let font = clock_font();
    for (file, top, w, time, date, space, icons) in [
        ("Windows_7_basic_theme.png", 1040, 1920, "19:29", "20/10/2018", 79, 4usize),
        ("7601-ThemeBasic.png", 860, 1440, "6:52 AM", "1/24/2025", 70, 2usize),
    ] {
        let shot = read_png(&dir.join("shots").join("betawiki").join(file));
        let l = seven::layout(w, 0, icons, space);
        let mut fb = Framebuffer::new(w, seven::HEIGHT, 0xFF00_0000);
        let bar = SevenBar {
            tray_icons: Vec::new(),
            time: time.into(),
            date: date.into(),
            clock_space: space,
            ..Default::default()
        };
        seven::paint(&mut fb, &bar, &l, &font);
        let (mut differ, mut total) = (0, 0);
        for y in 6..36 {
            for x in l.clock.0..=l.clock.1 {
                total += 1;
                if (fb.get(x, y) ^ shot.get(x, top + y)) & 0xFF_FFFF != 0 {
                    differ += 1;
                }
            }
        }
        let (_, tw) = seven::ink(&font, time);
        let (_, dw) = seven::ink(&font, date);
        println!(
            "{file}: stand-in time {tw} wide, date {dw} wide; {differ} of {total} pixels of the clock's space differ"
        );
    }
}
