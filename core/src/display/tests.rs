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
    assert_eq!(
        names(Era::Win2000),
        [
            "Blue Lace 16", "Coffee Bean", "FeatherTexture", "Gone Fishing", "Greenstone", "Prairie Wind",
            "Rhododendron", "River Sumida", "Santa Fe Stucco", "Soap Bubbles", "Zapotec"
        ]
    );
    // The order rule places the entries left out where the PCs had them.
    let mut all = vec!["Zapotec", "Boiling Point", "Fall Memories", "FeatherTexture", "Windows 2000", "Blue Lace 16"];
    all.sort_by(|a, b| windows_order(a, b));
    assert_eq!(all, ["Blue Lace 16", "Boiling Point", "Fall Memories", "FeatherTexture", "Windows 2000", "Zapotec"]);
    // A look shows only its own: 95's Setup is 95's picture, 98's is 98's.
    let (s95, s98) = (wallpaper(Era::Win95, "Setup").unwrap().picture(), wallpaper(Era::Win98, "Setup").unwrap().picture());
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
        let p = wallpaper(era, name).unwrap().picture();
        let mut used = p.pixels.clone();
        used.sort_unstable();
        used.dedup();
        assert_eq!((p.width, p.height, used.len()), (w, h, colors), "{name}");
        assert!(p.pixels.iter().all(|&px| px >> 24 == 0xFF), "{name}: opaque");
    }
    for era in Era::ALL {
        for w in wallpapers(era) {
            let p = w.picture();
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
    let p = wallpaper(Era::Win95, "Black Thatch").unwrap().picture();
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
    let p = wallpaper(Era::Win95, "Bubbles").unwrap().picture();
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
    let setup = wallpaper(Era::Win95, "Setup").unwrap().picture();
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
    let p = wallpaper(Era::Win98, "Clouds").unwrap().picture();
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
    paint_monitor(&mut fb, &WIN2000, 0, 0, &Choice::plain());
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
