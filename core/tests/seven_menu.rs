//! Windows 7 Basic's Start menu (`bevel_core::seven::menu`): its golden picture (a new file
//! `tests/golden/seven-start-menu.png`, written only by the ignored `write_seven_menu_golden`) and
//! the comparison with the screenshot it was measured from (ignored: the screenshot is private,
//! found through BEVEL95_PRIVATE or the `private` folder above this checkout).

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use bevel_core::fb::{Framebuffer, Icon, Rect};
use bevel_core::seven::menu::{self, ProgramRow, RightEntry, SevenMenu};
use bevel_core::text::{self, Font};

/// The stand-in for Windows 7's Segoe UI 9 pt (PROVISIONAL), as for the bar's clock.
fn font() -> Font {
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

/// A 32x32 stand-in for a program's picture (not Windows'): a colored square.
fn square(color: u32) -> Icon {
    let pixels = (0..32 * 32)
        .map(|i| {
            let (x, y) = (i % 32, i / 32);
            if (3..29).contains(&x) && (3..29).contains(&y) { 0xFF00_0000 | color } else { 0 }
        })
        .collect();
    Icon { width: 32, height: 32, pixels }
}

fn draw(m: &SevenMenu) -> Framebuffer {
    let l = menu::layout(m, 860);
    let mut fb = Framebuffer::new(menu::W, m.height, 0xFF00_0000);
    menu::paint(&mut fb, m, &l, &font());
    fb
}

fn golden_menu() -> SevenMenu {
    let colors = [0x2D6FD0, 0x40A040, 0xE87020, 0xE8C040, 0x8050C0, 0xC03030, 0x30A0A0, 0x909090];
    let names = [
        "Getting Started",
        "Media Center",
        "Calculator",
        "Sticky Notes",
        "Snipping Tool",
        "Paint",
        "Remote Desktop",
        "Magnifier",
    ];
    let item = |s: &str| RightEntry::Item(s.into());
    SevenMenu {
        programs: names
            .iter()
            .zip(colors)
            .enumerate()
            .map(|(k, (n, c))| ProgramRow { name: (*n).into(), icon: Some(square(c)), submenu: k == 0 })
            .collect(),
        user: "User".into(),
        right: vec![
            item("Documents"),
            item("Pictures"),
            item("Music"),
            RightEntry::Separator,
            item("Games"),
            item("Computer"),
            RightEntry::Separator,
            item("Control Panel"),
            item("Devices and Printers"),
            item("Default Programs"),
            item("Help and Support"),
        ],
        ..Default::default()
    }
}

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("golden").join("seven-start-menu.png")
}

#[test]
fn seven_menu_golden_image_matches() {
    let path = golden_path();
    assert!(
        path.exists(),
        "missing; `cargo test -p bevel-core --test seven_menu -- --ignored write_seven_menu_golden` writes it"
    );
    let expected = read_png(&path);
    let image = draw(&golden_menu());
    let differ = expected.pixels.iter().zip(&image.pixels).filter(|(a, b)| (*a ^ *b) & 0xFF_FFFF != 0).count();
    if (expected.width, expected.height) != (image.width, image.height) || differ > 0 {
        write_png(&path.with_extension("actual.png"), &image);
        panic!("seven-start-menu: {differ} pixels differ");
    }
}

#[test]
#[ignore = "writes tests/golden/seven-start-menu.png (only this); run it on request"]
fn write_seven_menu_golden() {
    write_png(&golden_path(), &draw(&golden_menu()));
    println!("wrote {}", golden_path().display());
}

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

#[test]
#[ignore = "reads the private screenshot of the Windows 7 survey (BEVEL95_PRIVATE or private/probes/win7)"]
fn seven_menu_matches_the_screenshot() {
    let dir = survey().expect("the survey's folder (private/probes/win7) is not found");
    let shot = read_png(&dir.join("shots").join("betawiki").join("7601-ThemeBasic.png"));
    // [B]: the menu at x 0..410, y 375..859; nine programs, the first with a submenu; the user's
    // name and ten entries with two separators. The names are masked, so any names serve.
    let item = |s: &str| RightEntry::Item(s.into());
    let m = SevenMenu {
        programs: (0..9).map(|k| ProgramRow { name: format!("Program {k}"), icon: None, submenu: k == 0 }).collect(),
        user: "User".into(),
        right: vec![
            item("a"),
            item("b"),
            item("c"),
            RightEntry::Separator,
            item("d"),
            item("e"),
            RightEntry::Separator,
            item("f"),
            item("g"),
            item("h"),
            item("i"),
        ],
        ..Default::default()
    };
    let l = menu::layout(&m, 860);
    assert_eq!(l.place, Rect::new(0, 375, 411, 485));
    let mut fb = Framebuffer::new(menu::W, m.height, 0xFF00_0000);
    menu::paint(&mut fb, &m, &l, &font());
    let h = m.height;
    // letter and icon boxes (the letters are the stand-in's, the icons the programs')
    let mut masks: Vec<Rect> = Vec::new();
    for r in &l.program_icons {
        masks.push(*r);
        masks.push(Rect::new(48, r.y + 10, 193, 15));
    }
    masks.push(Rect::new(45, h - 76, 196, 16)); // All Programs' name
    masks.push(Rect::new(22, h - 41, 204, 20)); // the hint and the text cursor
    masks.push(Rect::new(272, h - 39, 60, 16)); // Shut down's name
    for &t in &l.entry_tops {
        masks.push(Rect::new(270, t - 1, 131, 14));
    }
    let part = |x: i32, y: i32| -> &'static str {
        let up = h - y;
        if l.tile.contains(x, y) {
            "user picture tile (cut from it)"
        } else if l.shut_down.contains(x, y) {
            "Shut down button (cut from it, letters taken out)"
        } else if y == h - 1 {
            "bottom border"
        } else if y < menu::TOP_H {
            "top rows (cut from it)"
        } else if x < menu::PANE_X {
            "left frame (cut from it)"
        } else if x < menu::EDGE_X {
            if up <= menu::BOTTOM_UP {
                "pane bottom rows"
            } else if up <= menu::BAND_UP {
                "search band (cut from it, box emptied)"
            } else if y == l.line_row {
                "line above All Programs"
            } else if Rect::new(menu::SUBMENU_ARROW_X, 0, 4, h).contains(x, y)
                || Rect::new(menu::ALL_ARROW_X, h - menu::ALL_ARROW_UP, 7, 11).contains(x, y)
            {
                "arrows in the pane"
            } else {
                "left pane white"
            }
        } else if x < menu::RIGHT_X {
            "columns between the panes"
        } else if x < menu::RIGHT_EDGE_X {
            if l.separators.iter().any(|&s| y == s || y == s + 1) && (276..400).contains(&x) {
                "right separators (see-through, solved)"
            } else {
                "right column"
            }
        } else {
            "right edge"
        }
    };
    let mut rows: Vec<(&str, usize, usize, usize)> = Vec::new();
    for y in 0..h {
        for x in 0..menu::W {
            let p = part(x, y);
            let i = rows.iter().position(|r| r.0 == p).unwrap_or_else(|| {
                rows.push((p, 0, 0, 0));
                rows.len() - 1
            });
            if masks.iter().any(|r| r.contains(x, y)) {
                rows[i].3 += 1;
            } else if (fb.get(x, y) ^ shot.get(x, 375 + y)) & 0xFF_FFFF == 0 {
                rows[i].1 += 1;
            } else {
                rows[i].2 += 1;
            }
        }
    }
    rows.sort();
    println!("[B] the Start menu: part | equal | differ | masked");
    let mut wrong = Vec::new();
    for (p, eq, differ, masked) in rows {
        println!("  {p:<52} {eq:>7} {differ:>7} {masked:>7}");
        if differ > 0 {
            wrong.push(format!("{p}: {differ} differ"));
        }
    }
    assert!(wrong.is_empty(), "\n{}\n", wrong.join("\n"));
}
