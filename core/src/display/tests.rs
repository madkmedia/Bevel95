use super::*;
use crate::theme::{WIN95, WIN98, WIN2000};

fn choice(wallpaper: Option<&str>, show: Show, pattern: Option<&str>) -> Choice {
    Choice { wallpaper: wallpaper.map(str::to_string), show, pattern: pattern.map(str::to_string) }
}

fn drawn(theme: &Theme, c: &Choice, w: i32, h: i32) -> Framebuffer {
    let mut fb = Framebuffer::new(w, h, 0);
    draw(&mut fb, theme, c);
    fb
}

#[test]
fn each_look_lists_its_own_wallpapers_in_windows_order() {
    let names = |era| wallpapers(era).iter().map(|w| w.name).collect::<Vec<_>>();
    // The Reference PCs' lists (survey.md, section 2), without "(None)", which comes first, and
    // without what is not built yet (98's HTML page, 2000's JPEG pictures and Sample).
    assert_eq!(
        names(Era::Win95),
        [
            "Black Thatch", "Blue Rivets", "Bubbles", "Carved Stone", "Circles", "Houndstooth", "Pinstripe", "Setup",
            "Straw Mat", "Tiles", "Triangles", "Waves"
        ]
    );
    assert_eq!(
        names(Era::Win98),
        [
            "1stboot", "Black Thatch", "Blue Rivets", "Bubbles", "Carved Stone", "Circles", "Clouds", "Forest",
            "Gold Weave", "Houndstooth", "Metal Links", "Pinstripe", "Red Blocks", "Sandstone", "Setup", "Stitches",
            "Straw Mat", "Tiles", "Triangles", "Waves"
        ]
    );
    // 2000: its BMPs and its twelve JPEGs, as its list had them but for Sample (the PC user's own).
    assert_eq!(
        names(Era::Win2000),
        [
            "Blue Lace 16", "Boiling Point", "Chateau", "Coffee Bean", "Fall Memories", "FeatherTexture", "Fly Away",
            "Gold Petals", "Gone Fishing", "Greenstone", "Ocean Wave", "Paradise", "Prairie Wind", "Purple Sponge",
            "Rhododendron", "River Sumida", "Santa Fe Stucco", "Snow Trees", "Soap Bubbles", "Solar Eclipse",
            "Water Color", "Windows 2000", "Zapotec"
        ]
    );
    let jpegs = wallpapers(Era::Win2000).iter().filter(|w| w.is_jpeg()).count();
    assert_eq!(jpegs, 12);
    // A JPEG is read on the bar's side (its file's bytes, a JPEG's own start); a BMP is decoded here.
    let chateau = choice(Some("Chateau"), Show::Center, None);
    let id = chateau.to_read(Era::Win2000).unwrap();
    assert_eq!(id, PictureId::Windows(Era::Win2000, "Chateau"));
    assert_eq!(&id.bytes().unwrap()[..3], [0xFF, 0xD8, 0xFF]);
    assert!(wallpaper(Era::Win2000, "Chateau").unwrap().picture().is_none());
    assert_eq!(choice(Some("Zapotec"), Show::Center, None).to_read(Era::Win2000), None);
    // The order rule places the entries left out where the PCs had them.
    let mut all = vec!["Zapotec", "Boiling Point", "Fall Memories", "FeatherTexture", "Windows 2000", "Blue Lace 16"];
    all.sort_by(|a, b| windows_order(a, b));
    assert_eq!(all, ["Blue Lace 16", "Boiling Point", "Fall Memories", "FeatherTexture", "Windows 2000", "Zapotec"]);
    // A look shows only its own: 95's Setup is 95's picture, 98's is 98's.
    let (s95, s98) = (wallpaper(Era::Win95, "Setup").unwrap().picture().unwrap(), wallpaper(Era::Win98, "Setup").unwrap().picture().unwrap());
    assert_eq!((s95.width, s95.height, s98.width, s98.height), (640, 480, 640, 480));
    assert_ne!(s95.pixels, s98.pixels);
    assert!(wallpaper(Era::Win95, "Clouds").is_none() && wallpaper(Era::Win2000, "Bubbles").is_none());
}

#[test]
fn every_wallpaper_has_the_size_of_its_file() {
    // survey.md, section 5: (look, name, width, height, colors the file uses).
    let sizes = [
        (Era::Win95, "Black Thatch", 31, 30, 2),
        (Era::Win95, "Houndstooth", 27, 25, 4),
        (Era::Win95, "Straw Mat", 30, 32, 6),
        (Era::Win95, "Setup", 640, 480, 2),
        (Era::Win98, "1stboot", 50, 50, 2),
        (Era::Win98, "Clouds", 640, 480, 65),
        (Era::Win98, "Forest", 256, 256, 139),
        (Era::Win98, "Metal Links", 200, 180, 32),
        (Era::Win98, "Red Blocks", 54, 96, 3),
        (Era::Win98, "Setup", 640, 480, 52),
        (Era::Win2000, "Gone Fishing", 128, 128, 222),
        (Era::Win2000, "Greenstone", 160, 160, 232),
        (Era::Win2000, "River Sumida", 160, 160, 26),
        (Era::Win2000, "Santa Fe Stucco", 256, 256, 57),
        (Era::Win2000, "Zapotec", 96, 96, 63),
    ];
    for (era, name, w, h, colors) in sizes {
        let p = wallpaper(era, name).unwrap().picture().unwrap();
        let mut used = p.pixels.clone();
        used.sort_unstable();
        used.dedup();
        assert_eq!((p.width, p.height, used.len()), (w, h, colors), "{name}");
        assert!(p.pixels.iter().all(|&px| px >> 24 == 0xFF), "{name}: opaque");
    }
    for era in Era::ALL {
        for w in wallpapers(era).into_iter().filter(|w| !w.is_jpeg()) {
            let p = w.picture().unwrap();
            assert_eq!(p.pixels.len(), (p.width * p.height) as usize, "{}", w.name);
        }
    }
}

#[test]
fn each_look_has_its_own_patterns() {
    let names = |era| patterns(era).iter().map(|p| p.name).collect::<Vec<_>>();
    assert_eq!(names(Era::Win95), names(Era::Win98));
    assert_eq!(names(Era::Win95).len(), 19);
    assert_eq!(names(Era::Win95)[0], "Bricks");
    assert_eq!(names(Era::Win95)[18], "Waffle's Revenge");
    assert_eq!(
        names(Era::Win2000),
        ["50% Gray", "Boxes", "Critters", "Diamonds", "Paisley", "Pattern", "Quilt", "Scottie", "Spinner", "Thatches", "Tulip", "Waffle", "Weave"]
    );
    assert_eq!(pattern(Era::Win95, "Bricks").unwrap().rows, [187, 95, 174, 93, 186, 117, 234, 245]);
    assert_eq!(pattern(Era::Win2000, "Weave").unwrap().rows, [136, 84, 34, 69, 136, 21, 34, 81]);
    // Only Thatches is in both, with the same numbers.
    assert_eq!(pattern(Era::Win95, "Thatches").unwrap().rows, pattern(Era::Win2000, "Thatches").unwrap().rows);
    assert!(pattern(Era::Win2000, "Bricks").is_none() && pattern(Era::Win98, "Scottie").is_none());
}

#[test]
fn a_patterns_bits_start_at_the_screens_corner() {
    let c = choice(None, Show::Center, Some("Bricks"));
    let fb = drawn(&WIN98, &c, 20, 20);
    let rows = [187u8, 95, 174, 93, 186, 117, 234, 245];
    for y in 0..20 {
        for x in 0..20 {
            let set = (rows[(y % 8) as usize] >> (7 - x % 8)) & 1 != 0;
            assert_eq!(fb.get(x, y), if set { 0xFF00_0000 } else { WIN98.colors.desktop }, "{x},{y}");
        }
    }
    // Bit 7 of row 0 is the top-left pixel: 187 = 10111011.
    assert_eq!((fb.get(0, 0), fb.get(1, 0)), (0xFF00_0000, WIN98.colors.desktop));
    // Windows 2000's own desktop color between the black bits.
    let fb = drawn(&WIN2000, &choice(None, Show::Center, Some("Scottie")), 8, 8);
    assert_eq!((fb.get(1, 0), fb.get(0, 0)), (0xFF00_0000, WIN2000.colors.desktop));
}

#[test]
fn a_wallpaper_tiles_from_the_top_left_corner() {
    let c = choice(Some("Black Thatch"), Show::Tile, Some("Bricks"));
    let p = wallpaper(Era::Win95, "Black Thatch").unwrap().picture().unwrap();
    let fb = drawn(&WIN95, &c, 100, 70);
    for y in 0..70 {
        for x in 0..100 {
            assert_eq!(fb.get(x, y), p.get(x % 31, y % 30), "{x},{y}");
        }
    }
    // Under a tiled (or stretched) wallpaper no pattern shows.
    assert_eq!(c.pattern_shown(Era::Win95), None);
    assert_eq!(choice(Some("Zapotec"), Show::Stretch, Some("Tulip")).pattern_shown(Era::Win2000), None);
}

#[test]
fn a_centered_wallpaper_lies_on_the_pattern_or_the_color() {
    // The Reference PCs' places (survey.md, section 4): half the difference on the whole screen.
    assert_eq!(centered(800, 600, 64, 64), (368, 268));
    assert_eq!(centered(800, 600, 54, 96), (373, 252));
    assert_eq!(centered(800, 600, 96, 96), (352, 252));
    // Larger than the screen: cut equally (98's 1024x768 JPEG: 112 left and right, 84 top and bottom).
    assert_eq!(centered(800, 600, 1024, 768), (-112, -84));
    // Odd differences are cut toward zero, as C's division.
    assert_eq!(centered(801, 601, 54, 96), (373, 252));
    assert_eq!(centered(800, 600, 1025, 769), (-112, -84));
    // Smaller: the pattern around it.
    let c = choice(Some("Bubbles"), Show::Center, Some("Plaid"));
    let p = wallpaper(Era::Win95, "Bubbles").unwrap().picture().unwrap();
    let fb = drawn(&WIN95, &c, 100, 80);
    let (x0, y0) = centered(100, 80, 64, 64);
    assert_eq!((x0, y0), (18, 8));
    let plaid = pattern(Era::Win95, "Plaid").unwrap();
    for y in 0..80 {
        for x in 0..100 {
            let inside = (x0..x0 + 64).contains(&x) && (y0..y0 + 64).contains(&y);
            let want = if inside {
                p.get(x - x0, y - y0)
            } else if plaid.set(x, y) {
                0xFF00_0000
            } else {
                WIN95.colors.desktop
            };
            assert_eq!(fb.get(x, y), want, "{x},{y}");
        }
    }
    // Without a pattern, the desktop color around it.
    let fb = drawn(&WIN95, &choice(Some("Bubbles"), Show::Center, None), 100, 80);
    assert_eq!(fb.get(0, 0), WIN95.colors.desktop);
    // Larger: cut equally on all sides (95's Setup, 640x480, on a 600x400 screen).
    let setup = wallpaper(Era::Win95, "Setup").unwrap().picture().unwrap();
    let fb = drawn(&WIN95, &choice(Some("Setup"), Show::Center, Some("Plaid")), 600, 400);
    assert_eq!(fb.get(0, 0), setup.get(20, 40));
    assert_eq!(fb.get(599, 399), setup.get(619, 439));
}

#[test]
fn stretch_steps_as_windows_did() {
    // 98's Clouds, 640x480 to 800x600 (survey.md, section 4): the doubled columns are 1, 5, 9, ...
    // and the doubled rows 2, 6, 10, ... (counted from the top; from the bottom they are 1, 5, ...).
    let count = |map: &[i32], s: i32| map.iter().filter(|&&m| m == s).count();
    let cols = steps(Era::Win98, 640, 800, false);
    for s in 0..640 {
        assert_eq!(count(&cols, s), if s % 4 == 1 { 2 } else { 1 }, "column {s}");
    }
    let rows = steps(Era::Win98, 480, 600, true);
    for s in 0..480 {
        assert_eq!(count(&rows, s), if s % 4 == 2 { 2 } else { 1 }, "row {s}");
    }
    assert_eq!((rows[0], rows[599]), (0, 479));
    // 2000's Zapotec, 96x96 to 800x600: each source pixel covers 8 or 9 columns and 6 or 7 rows.
    let cols = steps(Era::Win2000, 96, 800, false);
    let rows = steps(Era::Win2000, 96, 600, true);
    for s in 0..96 {
        assert!((8..=9).contains(&count(&cols, s)), "column {s}");
        assert!((6..=7).contains(&count(&rows, s)), "row {s}");
    }
    // Pixels repeated, never mixed: a stretched picture has no color the picture has not.
    let c = choice(Some("Clouds"), Show::Stretch, Some("Daisies"));
    let fb = drawn(&WIN98, &c, 800, 600);
    let p = wallpaper(Era::Win98, "Clouds").unwrap().picture().unwrap();
    let mut colors = p.pixels.clone();
    colors.sort_unstable();
    assert!(fb.pixels.iter().all(|px| colors.binary_search(px).is_ok()));
    assert_eq!(fb.get(0, 0), p.get(0, 0));
    assert_eq!(fb.get(799, 599), p.get(639, 479));
    // Windows 95 had no Stretch: its look does not offer it.
    assert_eq!(Show::offered(Era::Win95), [Show::Tile, Show::Center]);
    assert_eq!(Show::offered(Era::Win2000), Show::ALL);
}

#[test]
fn the_plain_desktop_has_no_picture() {
    for era in Era::ALL {
        let theme = crate::theme::theme_in(era, Depth::TrueColor);
        assert!(background(theme, &Choice::plain(), 800, 600).is_none());
        // A name the look does not have is no choice either.
        assert!(background(theme, &choice(Some("Nothing"), Show::Tile, Some("Nothing")), 800, 600).is_none());
    }
    assert!(background(&WIN98, &choice(Some("Clouds"), Show::Center, None), 800, 600).is_some());
}

#[test]
fn the_choice_is_kept_per_look_by_name() {
    let text = with_choice("", Era::Win98, &choice(Some("Clouds"), Show::Stretch, Some("Daisies")));
    let text = with_choice(&text, Era::Win2000, &choice(Some("Zapotec"), Show::Tile, None));
    let [c95, c98, c2000] = parse(&text);
    assert_eq!(c95, Choice::plain(), "a look never chosen has the plain desktop");
    assert_eq!(c98, choice(Some("Clouds"), Show::Stretch, Some("Daisies")));
    assert_eq!(c2000, choice(Some("Zapotec"), Show::Tile, None));
    assert!(text.contains("win98 wallpaper = Clouds\n") && text.contains("win2000 pattern = (None)\n"), "{text}");
    // Changing one look changes only its own lines; comments and the other looks stay.
    let again = with_choice(&format!("# mine\n{text}"), Era::Win98, &Choice::plain());
    assert!(again.starts_with("# mine\n"));
    let [_, c98, c2000] = parse(&again);
    assert_eq!((c98, c2000), (Choice::plain(), choice(Some("Zapotec"), Show::Tile, None)));
    assert_eq!(again.matches("win98 wallpaper").count(), 1);
}

/// The review of 2026-11-21, finding 2: reading takes a key's last line, so writing must leave
/// one line per key, or the person's choice is lost at the next start.
#[test]
fn a_key_written_twice_is_left_once() {
    let text = "# mine\nwin98 wallpaper = Forest\nwin95 wallpaper = Bubbles\nwin98 wallpaper = Clouds\nWIN98 Wallpaper = Setup\n";
    assert_eq!(parse(text)[1].wallpaper.as_deref(), Some("Setup"), "the last line wins when read");
    let written = with_choice(text, Era::Win98, &choice(Some("Metal Links"), Show::Tile, None));
    assert_eq!(written.matches("win98 wallpaper").count() + written.matches("WIN98 Wallpaper").count(), 1, "{written}");
    assert_eq!(parse(&written)[1], choice(Some("Metal Links"), Show::Tile, None), "{written}");
    // The first line took the value where it stood; the other look and the note stay.
    assert!(written.starts_with("# mine\nwin98 wallpaper = Metal Links\nwin95 wallpaper = Bubbles\n"), "{written}");
    assert_eq!(parse(&written)[0].wallpaper.as_deref(), Some("Bubbles"));
    // A note on a line of its own is kept; a new file says so in its header.
    assert!(with_choice("", Era::Win95, &Choice::plain()).contains("A note needs a line of its own"));
}

/// The review of 2026-11-21, finding 5 (PROVISIONAL, nothing measured): with several screens the
/// pattern and the tiles start at the main screen's corner and run over every screen; Center and
/// Stretch are done for each screen within its own.
#[test]
fn several_screens_each_center_and_stretch_their_own() {
    let bubbles = wallpaper(Era::Win95, "Bubbles").unwrap().picture().unwrap();
    let at = |fb: &Framebuffer, x: i32, y: i32| fb.get(x, y);
    // Two screens side by side, of different heights: the main 200x150 at the left, a 100x80 right.
    let (main, second) = (Rect::new(0, 0, 200, 150), Rect::new(200, 0, 100, 80));
    let c = choice(Some("Bubbles"), Show::Center, Some("Plaid"));
    let mut fb = Framebuffer::new(300, 150, 0);
    draw_on(&mut fb, &WIN95, &c, None, &[main, second]);
    let plaid = pattern(Era::Win95, "Plaid").unwrap();
    let ground = |x: i32, y: i32, origin: (i32, i32)| if plaid.set(x - origin.0, y - origin.1) { 0xFF00_0000 } else { WIN95.colors.desktop };
    for (s, origin) in [(main, (0, 0)), (second, (0, 0))] {
        let (x0, y0) = centered(s.w, s.h, 64, 64);
        for y in 0..s.h {
            for x in 0..s.w {
                let inside = (x0..x0 + 64).contains(&x) && (y0..y0 + 64).contains(&y);
                let want = if inside { bubbles.get(x - x0, y - y0) } else { ground(s.x + x, s.y + y, origin) };
                assert_eq!(at(&fb, s.x + x, s.y + y), want, "{s:?} {x},{y}");
            }
        }
    }
    // A picture larger than a screen is cut to it: the 100x80 screen shows only its own part, and
    // the main screen's picture does not reach into it.
    let setup = wallpaper(Era::Win95, "Setup").unwrap().picture().unwrap();
    let mut fb = Framebuffer::new(300, 150, 0);
    draw_on(&mut fb, &WIN95, &choice(Some("Setup"), Show::Center, None), None, &[main, second]);
    let (mx, my) = centered(200, 150, 640, 480);
    let (sx, sy) = centered(100, 80, 640, 480);
    assert_eq!(at(&fb, 199, 10), setup.get(199 - mx, 10 - my));
    assert_eq!(at(&fb, 200, 10), setup.get(-sx, 10 - sy));
    assert_eq!(at(&fb, 250, 100), WIN95.colors.desktop, "below the shorter screen: no screen, the ground");
    // Stretch: each screen gets the whole picture.
    let clouds = wallpaper(Era::Win98, "Clouds").unwrap().picture().unwrap();
    let mut fb = Framebuffer::new(300, 150, 0);
    draw_on(&mut fb, &WIN98, &choice(Some("Clouds"), Show::Stretch, None), None, &[main, second]);
    for (s, small) in [(main, stretched(Era::Win98, &clouds, 200, 150)), (second, stretched(Era::Win98, &clouds, 100, 80))] {
        for y in 0..s.h {
            for x in 0..s.w {
                assert_eq!(at(&fb, s.x + x, s.y + y), small.get(x, y), "{s:?} {x},{y}");
            }
        }
    }
    // A second screen left of and above the main one: the desktop's corner is the second screen's,
    // the pattern and the tiles line up with the main screen's corner.
    let (main, second) = (Rect::new(100, 40, 200, 150), Rect::new(0, 0, 100, 80));
    let mut fb = Framebuffer::new(300, 190, 0);
    draw_on(&mut fb, &WIN95, &choice(None, Show::Center, Some("Plaid")), None, &[main, second]);
    for (x, y) in [(100, 40), (0, 0), (37, 13), (299, 189), (5, 70)] {
        assert_eq!(at(&fb, x, y), ground(x, y, (100, 40)), "{x},{y}");
    }
    let thatch = wallpaper(Era::Win95, "Black Thatch").unwrap().picture().unwrap();
    let mut fb = Framebuffer::new(300, 190, 0);
    draw_on(&mut fb, &WIN95, &choice(Some("Black Thatch"), Show::Tile, None), None, &[main, second]);
    for y in 0..190 {
        for x in 0..300 {
            assert_eq!(at(&fb, x, y), thatch.get((x - 100).rem_euclid(31), (y - 40).rem_euclid(30)), "{x},{y}");
        }
    }
    // Centered on the second screen within it, on the main within the main.
    let mut fb = Framebuffer::new(300, 190, 0);
    draw_on(&mut fb, &WIN95, &c, None, &[main, second]);
    let (x0, y0) = centered(100, 80, 64, 64);
    assert_eq!(at(&fb, x0, y0), bubbles.get(0, 0));
    let (x0, y0) = centered(200, 150, 64, 64);
    assert_eq!(at(&fb, 100 + x0 + 63, 40 + y0 + 63), bubbles.get(63, 63));
    // One screen over the whole area is the one-screen drawing.
    for c in [c.clone(), choice(Some("Black Thatch"), Show::Tile, Some("Bricks")), choice(Some("Setup"), Show::Center, None)] {
        let mut one = Framebuffer::new(300, 190, 0);
        draw_on(&mut one, &WIN95, &c, None, &[Rect::new(0, 0, 300, 190)]);
        assert_eq!(one.pixels, drawn(&WIN95, &c, 300, 190).pixels);
    }
}

#[test]
fn a_nonsense_value_means_the_plain_desktop_for_that_look() {
    let text = "win95 wallpaper = Clouds\nwin95 display = tile\nwin98 wallpaper = Clouds\nwin98 display = sideways\n\
                win2000 wallpaper = Zapotec\nwin2000 display = tile\nwin2000 pattern = Bricks\n";
    assert_eq!(parse(text), [Choice::plain(), Choice::plain(), Choice::plain()]);
    // 95 had no Stretch.
    let [c95, ..] = parse("win95 wallpaper = Bubbles\nwin95 display = stretch\n");
    assert_eq!(c95, Choice::plain());
    // A good look beside a bad one keeps its choice.
    let [c95, c98, _] = parse("win95 wallpaper = Bubbles\nwin95 display = tile\nwin98 pattern = Nothing\n");
    assert_eq!((c95, c98), (choice(Some("Bubbles"), Show::Tile, None), Choice::plain()));
    // Nothing at all: every look plain.
    assert_eq!(parse(""), [Choice::plain(), Choice::plain(), Choice::plain()]);
    assert_eq!(parse("just words\n=\n"), [Choice::plain(), Choice::plain(), Choice::plain()]);
}

#[test]
fn the_monitor_scales_a_wallpaper_as_the_screen_is() {
    // survey.md, section 2: 152/800 across and 112/600 down, each side rounded.
    assert_eq!(in_monitor(640, 480), (122, 90));
    assert_eq!(in_monitor(256, 256), (49, 48));
    assert_eq!(in_monitor(200, 180), (38, 34));
    assert_eq!(in_monitor(50, 50), (10, 9));
    assert_eq!(in_monitor(96, 96), (18, 18));
    assert_eq!(in_monitor(64, 64), (12, 12));
    assert_eq!(in_monitor(1, 1), (1, 1));
    let monitor = decode(MONITOR);
    assert_eq!((monitor.width, monitor.height), MONITOR_SIZE);
    // See-through around the monitor, its gray C0C0C0 inside, its screen inside the picture.
    assert_eq!(monitor.get(0, 0), 0);
    assert_eq!(monitor.get(MONITOR_SCREEN.x - 4, MONITOR_SCREEN.y + 20), 0xFFC0_C0C0);
    let mut fb = Framebuffer::new(184, 170, 0xFF11_2233);
    paint_monitor(&mut fb, &WIN2000, 0, 0, &Choice::plain(), None);
    assert_eq!(fb.get(0, 0), 0xFF11_2233, "the page shows around it");
    assert_eq!(fb.get(MONITOR_SCREEN.x + 5, MONITOR_SCREEN.y + 5), WIN2000.colors.desktop);
}

#[test]
fn wallpapers_take_the_looks_rule_in_16_and_256_colors() {
    let theme = crate::theme::theme_in(Era::Win98, Depth::Sixteen);
    let fb = background(theme, &choice(Some("Forest"), Show::Tile, None), 64, 64).unwrap();
    assert!(fb.pixels.iter().all(|&p| crate::colors16::index(p & 0x00FF_FFFF).is_some()));
    // 2000 at 256 colors keeps its desktop color around a picture.
    let theme = crate::theme::theme_in(Era::Win2000, Depth::TwoFiftySix);
    let fb = background(theme, &choice(Some("Zapotec"), Show::Center, None), 300, 300).unwrap();
    assert_eq!(fb.get(0, 0), WIN2000.colors.desktop);
}

/// A small picture whose every pixel says where it is (x in red, y in green).
fn where_picture(w: i32, h: i32) -> Picture {
    let pixels = (0..h).flat_map(|y| (0..w).map(move |x| 0xFF00_0000 | (x as u32) << 16 | (y as u32) << 8)).collect();
    Picture { width: w, height: h, pixels }
}

/// The upright rule: what a camera writes into a photo (EXIF orientation 1 to 8) turns the stored
/// picture so that it stands as it was taken.
#[test]
fn a_photo_stands_upright() {
    let p = where_picture(3, 2);
    let at = |q: &Picture, x: i32, y: i32| ((q.get(x, y) >> 16) & 0xFF, (q.get(x, y) >> 8) & 0xFF);
    // (orientation, size after, the stored pixel shown at the upright picture's top-left and at its
    // top-right).
    let cases = [
        (1, (3, 2), (0, 0), (2, 0)),
        (2, (3, 2), (2, 0), (0, 0)),
        (3, (3, 2), (2, 1), (0, 1)),
        (4, (3, 2), (0, 1), (2, 1)),
        (5, (2, 3), (0, 0), (0, 1)),
        (6, (2, 3), (0, 1), (0, 0)),
        (7, (2, 3), (2, 1), (2, 0)),
        (8, (2, 3), (2, 0), (2, 1)),
        (0, (3, 2), (0, 0), (2, 0)),
        (9, (3, 2), (0, 0), (2, 0)),
    ];
    for (o, size, top_left, top_right) in cases {
        let q = p.clone().upright(o);
        assert_eq!((q.width, q.height), size, "{o}");
        assert_eq!(at(&q, 0, 0), (top_left.0 as u32, top_left.1 as u32), "{o}: top-left");
        assert_eq!(at(&q, q.width - 1, 0), (top_right.0 as u32, top_right.1 as u32), "{o}: top-right");
        let mut a = q.pixels.clone();
        let mut b = p.pixels.clone();
        a.sort_unstable();
        b.sort_unstable();
        assert_eq!(a, b, "{o}: every pixel once");
    }
}

/// A person's own picture is told from Windows' names by being a file's full path; its name in
/// the list is the file's name without the ending, cut at characters.
#[test]
fn a_persons_own_picture_is_a_path() {
    for path in ["C:\\Pictures\\Me.jpg", "d:\\x", "\\\\server\\share\\a b#=.c.png", "C:\\Фото\\日本 #1=2.v1.final.JPG"] {
        assert!(is_own(path), "{path}");
    }
    for value in ["Forest", "(None)", "C:", "C:relative.jpg", "\\single", "\\\\", "C:\\a\u{7}b.jpg", ""] {
        assert!(!is_own(value), "{value}");
    }
    assert_eq!(own_name("C:\\Фото\\日本 #1=2.v1.final.JPG"), "日本 #1=2.v1.final");
    assert_eq!(own_name("C:\\Pictures\\Me.jpg"), "Me");
    assert_eq!(own_name("C:\\Pictures\\NoEnding"), "NoEnding");
    assert_eq!(own_name("C:\\Pictures\\.hidden"), ".hidden");
    let c = choice(Some("C:\\Pictures\\Me.jpg"), Show::Tile, Some("Bricks"));
    for era in Era::ALL {
        assert_eq!(c.to_read(era), Some(PictureId::Own("C:\\Pictures\\Me.jpg".into())));
        assert!(c.has_wallpaper(era) && !c.is_plain(era));
        assert_eq!(c.pattern_shown(era), None, "tiled: no pattern");
    }
    assert_eq!(PictureId::Own("C:\\x.jpg".into()).bytes(), None);
    // The limits: a side of 16,384 at most, 50 million pixels at most.
    assert!(size_taken(16_384, 3_000) && size_taken(1, 1) && size_taken(8_160, 6_120));
    assert!(!size_taken(16_385, 1) && !size_taken(0, 5) && !size_taken(10_000, 5_001) && !size_taken(-1, 1));
}

/// A picture to read (a JPEG, a person's own) is drawn once it is there; until then the ground
/// shows (the pattern around a centered one), or nothing at all (the plain desktop).
#[test]
fn a_picture_read_later_is_drawn_when_it_comes() {
    let p = where_picture(10, 6);
    let c = choice(Some("C:\\Pictures\\Me.png"), Show::Center, None);
    assert!(background_on(&WIN98, &c, None, &[], 40, 30).is_none(), "not there yet: the plain desktop");
    let fb = background_on(&WIN98, &c, Some(&p), &[], 40, 30).unwrap();
    assert_eq!((fb.get(15, 12), fb.get(24, 17), fb.get(0, 0)), (p.get(0, 0), p.get(9, 5), WIN98.colors.desktop));
    let with_pattern = choice(Some("C:\\Pictures\\Me.png"), Show::Center, Some("Bricks"));
    let fb = background_on(&WIN98, &with_pattern, None, &[], 16, 16).unwrap();
    assert_eq!(fb.pixels, drawn(&WIN98, &choice(None, Show::Center, Some("Bricks")), 16, 16).pixels);
    // Tiled and stretched by the rules the BMPs have.
    let fb = background_on(&WIN2000, &choice(Some("Chateau"), Show::Tile, None), Some(&p), &[], 25, 13).unwrap();
    assert_eq!((fb.get(0, 0), fb.get(12, 7), fb.get(24, 12)), (p.get(0, 0), p.get(2, 1), p.get(4, 0)));
    let fb = background_on(&WIN2000, &choice(Some("Chateau"), Show::Stretch, None), Some(&p), &[], 20, 12).unwrap();
    assert_eq!(fb.pixels, stretched(Era::Win2000, &p, 20, 12).pixels);
    // A picture read for a BMP's choice is not taken: the BMP is the look's own.
    let fb = background_on(&WIN2000, &choice(Some("Zapotec"), Show::Tile, None), Some(&p), &[], 96, 96).unwrap();
    assert_eq!(fb.pixels, background(&WIN2000, &choice(Some("Zapotec"), Show::Tile, None), 96, 96).unwrap().pixels);
    // A partly see-through pixel is mixed with what lies under it.
    let half = Picture { width: 1, height: 1, pixels: vec![0x80FF_FFFF] };
    let fb = background_on(&WIN98, &c, Some(&half), &[], 1, 1).unwrap();
    let d = WIN98.colors.desktop;
    let mix = |s: u32| (0xFF * 0x80 + ((d >> s) & 0xFF) * 0x7F + 127) / 255;
    assert_eq!(fb.get(0, 0), 0xFF00_0000 | mix(16) << 16 | mix(8) << 8 | mix(0));
    // The monitor shows it by the BMPs' rule too.
    let mut a = Framebuffer::new(184, 170, 0);
    paint_monitor(&mut a, &WIN98, 0, 0, &c, Some(&p));
    let mut b = Framebuffer::new(184, 170, 0);
    paint_monitor(&mut b, &WIN98, 0, 0, &c, None);
    assert_ne!(a.pixels, b.pixels);
}

/// What the desktop's picture costs on a large screen (the owner's 2560x1440 at Size 2, and at
/// Size 1): how long making the picture takes, what it holds, and a full repaint (the icons painted
/// over the whole desktop, then enlarged as the desktop window does) with and without it. Run on
/// request: `cargo test -p bevel-core --release --lib -- --ignored --nocapture how_much_a_wallpaper_costs`.
#[test]
#[ignore = "measures time; run on request"]
fn how_much_a_wallpaper_costs() {
    use crate::desktop_icons::Desktop;
    use crate::text::{Font, Fonts};
    use std::time::Instant;
    let font = Font::parse(include_str!("../../tests/fixtures/ms-sans-serif-11px.font")).unwrap();
    let fonts = Fonts { regular: &font, bold: &font };
    let theme = &WIN98;
    let choices = [
        ("plain", Choice::plain()),
        ("Forest tiled", choice(Some("Forest"), Show::Tile, None)),
        ("Clouds stretched", choice(Some("Clouds"), Show::Stretch, None)),
        ("Red Blocks on Waffle's Revenge", choice(Some("Red Blocks"), Show::Center, Some("Waffle's Revenge"))),
    ];
    for scale in [2, 1] {
        let (w, h) = (2560 / scale, 1440 / scale);
        for (name, c) in &choices {
            let runs = 10;
            let t = Instant::now();
            let mut bg = None;
            for _ in 0..runs {
                bg = background(theme, c, w, h);
            }
            let make = t.elapsed().as_secs_f64() * 1000.0 / f64::from(runs);
            let bytes = bg.as_ref().map_or(0, |b| b.pixels.len() * 4);
            let mut d = Desktop::new(Era::Win98, Rect::new(0, 0, w, h - 28));
            d.set_background(bg.map(std::sync::Arc::new));
            let mut fb = Framebuffer::new(w, h, theme.colors.desktop);
            let t = Instant::now();
            for _ in 0..runs {
                d.paint(&mut fb, theme, &fonts, Rect::new(0, 0, w, h));
                let big = crate::scale::upscale(&fb, scale);
                std::hint::black_box(&big);
            }
            let repaint = t.elapsed().as_secs_f64() * 1000.0 / f64::from(runs);
            println!("size {scale}, {w}x{h}: {name}: picture made in {make:.1} ms, holds {bytes} bytes; full repaint {repaint:.1} ms");
        }
    }
}