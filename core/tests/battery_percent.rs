//! The battery's percentage beside its icon (Bevel95's own option, off unless switched on):
//! where its part goes in the notification area, what it moves and what it does not, and that
//! the bar without it is drawn as before. Drawn with the real fonts of each look.

use std::path::PathBuf;

use bevel_core::fb::{Framebuffer, Icon};
use bevel_core::icons;
use bevel_core::layout::{self, BarLayout};
use bevel_core::paint::{self, Bar};
use bevel_core::status::{self, Network, Power};
use bevel_core::text::{Font, Fonts, fixture_name};
use bevel_core::theme::{Era, Theme, WIN95, WIN98, WIN2000, theme_in};

const WIDTH: i32 = 640;

fn load_font(theme: &Theme, bold: bool) -> Font {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(fixture_name(
        theme.font_face,
        theme.font_px,
        bold,
    ));
    Font::parse(&std::fs::read_to_string(path).unwrap()).unwrap().with_period_glyphs()
}

fn on_battery(percent: u8) -> Power {
    Power { battery: true, on_ac: Some(false), percent: Some(percent), ..Power::default() }
}

/// The area as the bar fills it: the connection, the battery (index 1), then two programs'
/// icons (the speaker twice will do).
fn icons_for(era: Era, power: &Power) -> Vec<Option<Icon>> {
    let net = Network { connected: true, ..Network::default() };
    vec![
        Some(status::network_icon(era, &net)),
        Some(status::power_icon(era, power)),
        Some(icons::speaker(false)),
        Some(icons::speaker(true)),
    ]
}

fn bar(era: Era, power: &Power, number: Option<String>) -> Bar {
    Bar {
        tray: icons_for(era, power),
        tray_number: number.map(|text| (1, text)),
        clock: "3:15 PM".into(),
        clock_widest: "12:00 AM".into(),
        ..Bar::default()
    }
}

fn paint(theme: &Theme, fonts: &Fonts, state: &Bar) -> (Framebuffer, BarLayout) {
    let mut fb = Framebuffer::new(WIDTH, theme.bar.height, 0);
    let layout = paint::paint_bar(&mut fb, theme, fonts, state);
    (fb, layout)
}

fn each_look(f: impl Fn(Era, &Theme, &Fonts)) {
    for (era, theme) in [(Era::Win95, &WIN95), (Era::Win98, &WIN98), (Era::Win2000, &WIN2000)] {
        let (regular, bold) = (load_font(theme, false), load_font(theme, true));
        let fonts = Fonts { regular: &regular, bold: &bold };
        f(era, theme, &fonts);
    }
}

#[test]
fn switched_off_nothing_changes() {
    each_look(|era, theme, fonts| {
        let power = on_battery(87);
        let (today, today_layout) = paint(theme, fonts, &bar(era, &power, None));
        assert_eq!(today_layout.number, None);
        // An empty text (Windows does not know the percentage) is the same as none.
        let (empty, empty_layout) = paint(theme, fonts, &bar(era, &power, Some(String::new())));
        assert_eq!(empty_layout, today_layout, "{era:?}");
        assert!(empty.pixels == today.pixels, "{era:?}");
        // The layout is the one `layout::bar` gives without a number.
        let plain = layout::bar(theme, fonts, WIDTH, 0, 4, None, "3:15 PM", "12:00 AM", 0);
        assert_eq!(plain, today_layout, "{era:?}");
    });
}

#[test]
fn the_part_keeps_its_width_and_the_clock_its_place() {
    each_look(|era, theme, fonts| {
        let (_, off) = paint(theme, fonts, &bar(era, &on_battery(87), None));
        let t = &theme.tray;
        let part_w = fonts.regular.measure("100%") + 10;
        let mut wells = Vec::new();
        for percent in [9, 87, 100] {
            let power = on_battery(percent);
            let (_, on) = paint(theme, fonts, &bar(era, &power, status::percent_text(&power)));
            let (icon, part) = on.number.expect("a part for the number");
            assert_eq!((icon, part.w), (1, part_w), "{era:?} {percent}%");
            assert_eq!(on.clock, off.clock, "{era:?}: the clock never moves");
            assert_eq!(on.well.right(), off.well.right(), "{era:?}");
            // The area grows to the left by the icons' gap and the part.
            let grown = t.icons_gap + part_w;
            assert_eq!(on.well.x, off.well.x - grown, "{era:?}");
            // The icons left of the battery and the battery itself move left with it; those
            // right of the number keep their places.
            for i in 0..=1 {
                assert_eq!(on.tray[i].x, off.tray[i].x - grown, "{era:?} icon {i}");
            }
            for i in 2..4 {
                assert_eq!(on.tray[i], off.tray[i], "{era:?} icon {i}");
            }
            // The part comes the icons' gap after the battery's step; the next icon at its end.
            assert_eq!(part.x, on.tray[1].x + t.icon_step + t.icons_gap, "{era:?}");
            assert_eq!(on.tray[2].x, part.right(), "{era:?}");
            assert_eq!((part.y, part.h), (on.clock.y, on.clock.h), "{era:?}");
            wells.push(on.well);
        }
        assert!(wells.windows(2).all(|w| w[0] == w[1]), "{era:?}: the area does not jump");
    });
}

#[test]
fn a_point_on_the_number_is_on_the_battery() {
    each_look(|era, theme, fonts| {
        let power = on_battery(45);
        let (_, on) = paint(theme, fonts, &bar(era, &power, status::percent_text(&power)));
        let (_, part) = on.number.unwrap();
        let y = part.y + part.h / 2;
        for x in [part.x, part.x + part.w / 2, part.right() - 1] {
            assert_eq!(on.tray_icon_at(theme, x, y), Some(1), "{era:?} at {x}");
        }
        // The gap between the battery's step and the part too; the next icon is its own.
        assert_eq!(on.tray_icon_at(theme, part.x - 1, y), Some(1), "{era:?}");
        assert_eq!(on.tray_icon_at(theme, part.right(), y), Some(2), "{era:?}");
        assert_eq!(on.tray_icon_at(theme, on.tray[0].x, y), Some(0), "{era:?}");
    });
}

#[test]
fn the_number_is_written_as_the_clock() {
    each_look(|era, theme, fonts| {
        let power = on_battery(87);
        let (fb, on) = paint(theme, fonts, &bar(era, &power, status::percent_text(&power)));
        let (_, part) = on.number.unwrap();
        // Written 5 into its part, in the clock's color, on the clock's baseline: the same
        // text drawn by hand where the clock's would go gives the same pixels.
        let mut expect = Framebuffer::new(WIDTH, theme.bar.height, 0);
        expect.fill(part, theme.colors.face);
        let text_y = on.well.y + 1 + theme.tray.pad + theme.text.clock_baseline - fonts.regular.ascent;
        fonts.regular.draw(&mut expect, part.x + 5, text_y, "87%", theme.colors.clock_text);
        for y in part.y..part.bottom() {
            for x in part.x..part.right() {
                assert_eq!(fb.get(x, y), expect.get(x, y), "{era:?} at ({x}, {y})");
            }
        }
        assert!((part.x..part.right()).any(|x| fb.get(x, text_y + 3) == theme.colors.clock_text), "{era:?}: ink");
    });
}

#[test]
fn in_16_colors_as_the_clock() {
    use bevel_core::theme::Depth;
    for era in Era::ALL {
        let theme = theme_in(era, Depth::Sixteen);
        let (regular, bold) = (load_font(theme, false), load_font(theme, true));
        let fonts = Fonts { regular: &regular, bold: &bold };
        let power = on_battery(87);
        let (fb, on) = paint(theme, &fonts, &bar(era, &power, status::percent_text(&power)));
        let (_, part) = on.number.unwrap();
        let inked: Vec<u32> = (part.y..part.bottom())
            .flat_map(|y| (part.x..part.right()).map(move |x| (x, y)))
            .map(|(x, y)| fb.get(x, y))
            .collect();
        assert!(inked.iter().all(|&p| p == theme.colors.face || p == theme.colors.clock_text), "{era:?}");
    }
}

#[test]
fn an_icon_index_past_the_icons_is_ignored() {
    each_look(|_, theme, fonts| {
        let plain = layout::bar(theme, fonts, WIDTH, 0, 2, None, "3:15 PM", "12:00 AM", 0);
        assert_eq!(layout::bar(theme, fonts, WIDTH, 0, 2, Some(2), "3:15 PM", "12:00 AM", 0), plain);
    });
}
