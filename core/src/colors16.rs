//! 16 Colors: Windows 95, 98 and 2000 as they looked on a 16-color screen (a PC without its
//! proper display driver), beside True Color, the way everything else in Bevel95 is drawn.
//!
//! Not a filter over the finished picture. Each look's colors, patterns and picture sets
//! change at their source, as Windows did it (`theme::theme_in` gives a look's 16-color
//! theme); only a picture that had no 16-color drawing (today's programs' icons, and the
//! classic pictures Windows had only in more colors) is converted here, pixel by pixel, by
//! that Windows' own rule. Measured on the three Reference PCs in 16 colors
//! (private/probes/colors16/survey.txt; docs/theme-spec.md, "16 Colors"):
//!
//! - Rule 1 (all three): a picture is never dotted. Each pixel becomes one of the 16.
//! - Rule 2 (Windows 2000): that color is the plain nearest one (RGB distance): all 32,768
//!   colors tested at 5 bits per channel agreed.
//! - Rule 3 (Windows 95 and 98, identical): not the nearest one (only 58% agree). The measured
//!   table of every color at 6 bits per channel (`assets/vga-9x.txt`) is used as it is.
//! - Rule 4 (95 and 98 only): a plain area (a brush) in a color outside the 16 is a 4×4 pattern
//!   of two of them (the tooltip's FFFFE1: white with two yellow dots in every 4×4). Lines and
//!   3D edges take one color. Windows 2000 never dots an area: it takes the nearest color.
//! - Rule 5 (all three): a selected icon is a checkerboard of navy over the picture, and
//!   inverting a color (the dotted focus rectangle) works on its number among the 16: i -> 15 - i.

use std::borrow::Cow;

use crate::fb::{Framebuffer, Icon, Rect};
use crate::theme::{Era, Theme, rgb};

/// How many colors the screen shows (`colors` in settings.txt): True Color, 256 colors or 16
/// colors. High Color looked the same as True Color on all three Reference PCs (no choice).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Depth {
    /// As everything was drawn before this option: the three Windows in True Color.
    #[default]
    TrueColor,
    /// As each Windows looked on a 256-color screen after its restart: its 16-color look in the
    /// shell (the 16-color drawings, the navy checkerboard, plain title bars), with what that
    /// Windows showed richer there (`colors256`).
    TwoFiftySix,
    /// As each Windows looked on a 16-color screen.
    Sixteen,
}

impl Depth {
    /// In the order of Windows' own Display Properties list (fewest colors first).
    pub const ALL: [Depth; 3] = [Depth::Sixteen, Depth::TwoFiftySix, Depth::TrueColor];

    /// The word in settings.txt (`colors = true`, `colors = 256` or `colors = 16`).
    pub const fn setting(self) -> &'static str {
        match self {
            Depth::TrueColor => "true",
            Depth::TwoFiftySix => "256",
            Depth::Sixteen => "16",
        }
    }

    /// Reads the word in settings.txt (a few spellings people might type are accepted).
    pub fn from_setting(value: &str) -> Option<Depth> {
        match value.trim().to_ascii_lowercase().replace([' ', '_', '-'], "").as_str() {
            "true" | "truecolor" | "truecolour" | "24" | "32" => Some(Depth::TrueColor),
            "256" | "256colors" | "256colours" | "256color" | "256colour" | "8" => Some(Depth::TwoFiftySix),
            "16" | "16colors" | "16colours" | "16color" | "16colour" | "4" => Some(Depth::Sixteen),
            _ => None,
        }
    }

    /// The name, as Windows' own Display Properties had it.
    pub const fn title(self) -> &'static str {
        match self {
            Depth::TrueColor => "True Color",
            Depth::TwoFiftySix => "256 Colors",
            Depth::Sixteen => "16 Colors",
        }
    }

    /// True for 16 and 256 colors: the screens on which the shell drew its 16-color look (the
    /// 16-color drawings, the navy checkerboard, plain title bars). 256 colors differs from it
    /// only where `colors256` says so.
    pub const fn reduced(self) -> bool {
        !matches!(self, Depth::TrueColor)
    }
}

/// Windows' 16 colors in its own (VGA) order, as `0xRRGGBB`: black, maroon, green, olive,
/// navy, purple, teal, silver, gray, red, lime, yellow, blue, fuchsia, aqua, white.
pub const VGA: [u32; 16] = [
    0x000000, 0x800000, 0x008000, 0x808000, 0x000080, 0x800080, 0x008080, 0xC0C0C0, 0x808080, 0xFF0000, 0x00FF00,
    0xFFFF00, 0x0000FF, 0xFF00FF, 0x00FFFF, 0xFFFFFF,
];

/// One of the 16 as an opaque framebuffer pixel.
pub const fn vga(index: usize) -> u32 {
    rgb(VGA[index])
}

pub const BLACK: u32 = vga(0);
pub const NAVY: u32 = vga(4);
pub const TEAL: u32 = vga(6);
pub const SILVER: u32 = vga(7);
pub const GRAY: u32 = vga(8);
pub const YELLOW: u32 = vga(11);
pub const WHITE: u32 = vga(15);

/// Which of the 16 a pixel is (its alpha ignored), if it is one of them.
pub fn index(pixel: u32) -> Option<usize> {
    VGA.iter().position(|&v| v == pixel & 0x00FF_FFFF)
}

/// A pixel inverted the way a 16-color screen inverts it (the dotted focus rectangle): its
/// number among the 16 turned round, i -> 15 - i (teal becomes red, silver gray, navy yellow).
/// A pixel outside the 16 is taken to its color by `rule` first.
pub fn invert(era: Era, pixel: u32) -> u32 {
    let i = index(pixel).unwrap_or_else(|| index(convert(era, pixel)).unwrap_or(0));
    vga(15 - i)
}

/// One pixel of the dotted focus rectangle (DrawFocusRect) on a 16-color screen: those it
/// inverts take the number turned round (`invert`; on the selection's navy, yellow: the selected
/// desktop labels of 95 and 2000 on the Reference PCs); the others stay.
pub fn focus_dot(theme: &Theme, pixel: u32, inverted: bool) -> u32 {
    if !inverted {
        return pixel;
    }
    // Windows 2000 at 256 colors: its own desktop and face invert to their own palette numbers.
    crate::colors256::invert(theme, pixel).unwrap_or_else(|| invert(theme.era, pixel))
}

fn channels(pixel: u32) -> [i32; 3] {
    [((pixel >> 16) & 0xFF) as i32, ((pixel >> 8) & 0xFF) as i32, (pixel & 0xFF) as i32]
}

/// Rule 2, Windows 2000's: the nearest of the 16 (plain RGB distance; on a tie the first in
/// Windows' order).
pub fn nearest(pixel: u32) -> u32 {
    let p = channels(pixel);
    let distance = |v: u32| channels(v).iter().zip(&p).map(|(a, b)| (a - b) * (a - b)).sum::<i32>();
    rgb(*VGA.iter().min_by_key(|&&v| distance(v)).expect("16 colors"))
}

/// The measured table of Windows 95 and 98: 4,096 lines (blue 0..63 outer, green 0..63 inner)
/// of 64 hexadecimal digits (red 0..63), each digit a color's number among the 16. A channel
/// value v at 6 bits stands for round(v × 255 / 63).
const TABLE_9X: &str = include_str!("../assets/vga-9x.txt");

/// A channel (0..255) at 6 bits, as the table counts: the nearest of its 64 steps.
const fn six_bits(c: i32) -> usize {
    ((c * 63 + 127) / 255) as usize
}

/// Rule 3, Windows 95's and 98's: the measured table. It is exact at its own 64 steps per
/// channel; between them a color within one step of a border between two of the 16 may go
/// either way (four of the survey's patches, measured apart from the table, went the other
/// way: 3A6EA5 silver, 4080C0 blue, C08040 red, 40C080 lime, where the table's nearest step
/// says navy, silver, silver, silver).
pub fn table_9x(pixel: u32) -> u32 {
    let [r, g, b] = channels(pixel).map(six_bits);
    let digit = TABLE_9X.as_bytes()[(b * 64 + g) * 65 + r];
    vga(char::from(digit).to_digit(16).expect("a hex digit") as usize)
}

/// One pixel of a picture without a 16-color drawing, by `era`'s rule (opaque).
pub fn convert(era: Era, pixel: u32) -> u32 {
    if index(pixel).is_some() {
        return 0xFF00_0000 | pixel;
    }
    match era {
        Era::Win2000 => nearest(pixel),
        Era::Win95 | Era::Win98 => table_9x(pixel),
    }
}

/// A picture without a 16-color drawing as `era` showed it in 16 colors: each pixel one of the
/// 16 by that Windows' rule, and see-through or not (a 16-color icon has no half-transparent
/// pixels: Windows used its mask, which is set where the picture is mostly see-through).
pub fn picture(era: Era, icon: &Icon) -> Icon {
    // The look's own 16-color drawing of this picture, where that Windows had one.
    if let Some(own) = crate::colors16_art::own(era, icon) {
        return own;
    }
    let pixels = icon.pixels.iter().map(|&p| if p >> 24 < 0x80 { 0 } else { convert(era, p) }).collect();
    Icon { width: icon.width, height: icon.height, pixels }
}

/// The picture as `theme` shows it: unchanged in True Color, converted in 16 colors. (A
/// picture already in the 16 colors comes out the same, apart from half-see-through pixels.)
pub fn shown<'a>(theme: &Theme, icon: &'a Icon) -> Cow<'a, Icon> {
    match theme.depth {
        Depth::TrueColor => Cow::Borrowed(icon),
        Depth::Sixteen => Cow::Owned(picture(theme.era, icon)),
        Depth::TwoFiftySix => {
            if let Some(own) = crate::colors256::own(theme, icon) {
                return Cow::Owned(own);
            }
            let mut out = picture(theme.era, icon);
            // Windows 2000 at 256 colors kept its face: where the True Color picture shows the
            // face (D4D0C8, the toolbar pieces cut with the button's face, 2000's Go button), it
            // stays the face (the 16-color drawing has silver there).
            if theme.era == Era::Win2000 {
                for (o, &p) in out.pixels.iter_mut().zip(&icon.pixels) {
                    if p >> 24 >= 0x80 && crate::colors256::kept(theme, p) && *o == SILVER {
                        *o = p | 0xFF00_0000;
                    }
                }
            }
            Cow::Owned(out)
        }
    }
}

/// Draws a picture as `theme` shows it (see `shown`).
pub fn draw_picture(fb: &mut Framebuffer, theme: &Theme, icon: &Icon, x: i32, y: i32) {
    fb.draw_icon(&shown(theme, icon), x, y);
}

/// How Windows 95 and 98 fill an area in a color outside the 16: `base` with `dots` pixels of
/// `dot` in every 4×4 block, in the order of a 4×4 ordered (Bayer) pattern.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dotted {
    pub base: u32,
    pub dot: u32,
    pub dots: u32,
}

/// The areas measured (Rule 4): the tooltip FFFFE1, white with 2 of 16 pixels yellow (98's
/// clock tooltip on the Reference PC). Any other color outside the 16 is filled plain with
/// its table color (not measured as an area).
fn dotted_9x(color: u32) -> Option<Dotted> {
    match color & 0x00FF_FFFF {
        0xFFFFE1 => Some(Dotted { base: WHITE, dot: YELLOW, dots: 2 }),
        _ => None,
    }
}

/// The 4×4 ordered pattern: a pixel is a dot when its number here is below the count of dots.
/// Its phase is the one measured: the clock tooltip's two yellow dots at (0, 2) and (2, 0) of
/// every block, counted from the window's corner.
const BAYER: [[u32; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

fn is_dot(x: i32, y: i32, dots: u32) -> bool {
    BAYER[y.rem_euclid(4) as usize][(x + 2).rem_euclid(4) as usize] < dots
}

/// Fills `r` with a plain area of `color` as `theme` shows it: in True Color plainly; in 16
/// colors with one of the 16, or (95, 98: Rule 4) a pattern of two of them, lined up with the
/// framebuffer's corner.
pub fn fill(fb: &mut Framebuffer, theme: &Theme, r: Rect, color: u32) {
    if theme.depth == Depth::TrueColor || index(color).is_some() || crate::colors256::kept(theme, color) {
        fb.fill(r, color);
        return;
    }
    // 256 colors' own dotted areas (`colors256::dotted`), counted from the area's corner.
    if let Some(d) = crate::colors256::dotted(theme, color) {
        for y in r.y.max(0)..r.bottom().min(fb.height) {
            for x in r.x.max(0)..r.right().min(fb.width) {
                fb.set(x, y, d.at(x - r.x, y - r.y));
            }
        }
        return;
    }
    match theme.era {
        Era::Win2000 => fb.fill(r, nearest(color)),
        Era::Win95 | Era::Win98 => match dotted_9x(color) {
            None => fb.fill(r, table_9x(color)),
            Some(d) => {
                fb.fill(r, d.base);
                for y in r.y.max(0)..r.bottom().min(fb.height) {
                    for x in r.x.max(0)..r.right().min(fb.width) {
                        if is_dot(x, y, d.dots) {
                            fb.set(x, y, d.dot);
                        }
                    }
                }
            }
        },
    }
}

/// A line or a single pixel's color as `theme` shows it (lines and 3D edges are never
/// dotted): unchanged in True Color, else by the look's rule.
pub fn line(theme: &Theme, color: u32) -> u32 {
    if crate::colors256::kept(theme, color) {
        return color;
    }
    match theme.depth {
        Depth::TrueColor => color,
        Depth::TwoFiftySix | Depth::Sixteen => convert(theme.era, color),
    }
}

/// A selected icon in 16 colors (Rule 5): every other pixel of the picture navy, the others
/// its own, the pattern one step on from row to row; its see-through pixels stay as they
/// are. `origin` is where the checkerboard is counted from. (True Color blends the picture
/// half with the selection color instead.)
pub fn selected(icon: &Icon, origin: (i32, i32)) -> Icon {
    let mut out = icon.clone();
    for y in 0..icon.height {
        for x in 0..icon.width {
            let i = (y * icon.width + x) as usize;
            if icon.pixels[i] >> 24 >= 0x80 && (x + y + origin.0 + origin.1).rem_euclid(2) == 0 {
                out.pixels[i] = NAVY;
            }
        }
    }
    out
}

/// True when every pixel of `fb` is one of the 16 (for the tests of the 16-color looks).
pub fn all_sixteen(fb: &Framebuffer) -> bool {
    fb.pixels.iter().all(|&p| index(p).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_setting_reads_and_writes() {
        assert_eq!(Depth::default(), Depth::TrueColor, "a file without the line is as before");
        for d in Depth::ALL {
            assert_eq!(Depth::from_setting(d.setting()), Some(d));
        }
        assert_eq!(Depth::from_setting(" 16 Colors "), Some(Depth::Sixteen));
        assert_eq!(Depth::from_setting("True Color"), Some(Depth::TrueColor));
        assert_eq!(Depth::from_setting("256"), Some(Depth::TwoFiftySix));
        assert_eq!(Depth::from_setting("256 Colors"), Some(Depth::TwoFiftySix));
        assert_eq!(Depth::from_setting("65536"), None);
        assert_eq!(Depth::TwoFiftySix.title(), "256 Colors");
        assert_eq!(Depth::Sixteen.title(), "16 Colors");
        assert!(Depth::Sixteen.reduced() && Depth::TwoFiftySix.reduced() && !Depth::TrueColor.reduced());
        assert_eq!(Depth::TrueColor.title(), "True Color");
    }

    #[test]
    fn the_16_stay_themselves_by_every_rule() {
        for (i, &v) in VGA.iter().enumerate() {
            assert_eq!(nearest(rgb(v)), rgb(v));
            assert_eq!(table_9x(rgb(v)), rgb(v), "{v:06X}");
            assert_eq!(index(rgb(v)), Some(i));
            for era in crate::theme::Era::ALL {
                assert_eq!(convert(era, rgb(v)), rgb(v));
            }
        }
    }

    #[test]
    fn inverting_turns_the_number_round() {
        assert_eq!(invert(Era::Win95, TEAL), vga(9), "teal becomes red");
        assert_eq!(invert(Era::Win98, SILVER), GRAY);
        assert_eq!(invert(Era::Win2000, NAVY), YELLOW);
        assert_eq!(invert(Era::Win95, BLACK), WHITE);
    }

    #[test]
    fn windows_2000_takes_the_nearest_color() {
        // Rows of the Reference PC's 5-bit table (private/probes/colors16/win2000/map-16color-5bit.txt):
        // (blue, green) = (0, 0), (15, 20) and (31, 31), red 0..31.
        let rows = [
            (0, 0, "00000000111111111111111199999999"),
            (15, 20, "66666666888888888888888877777777"),
            (31, 31, "EEEEEEEEEEEEEEE77777FFFFFFFFFFFF"),
        ];
        for (b, g, row) in rows {
            for (r, digit) in row.chars().enumerate() {
                let c = |v: i32| ((v * 255 + 15) / 31) as u32;
                let pixel = rgb((c(r as i32) << 16) | (c(g) << 8) | c(b));
                assert_eq!(nearest(pixel), vga(digit.to_digit(16).unwrap() as usize), "r{r} g{g} b{b}");
            }
        }
        // The survey's plain areas: the desktop, the face, the dark shadow, the selection, the
        // tooltip.
        assert_eq!(nearest(rgb(0x3A6EA5)), TEAL);
        assert_eq!(nearest(rgb(0xD4D0C8)), SILVER);
        assert_eq!(nearest(rgb(0x404040)), BLACK);
        assert_eq!(nearest(rgb(0x0A246A)), NAVY);
        assert_eq!(nearest(rgb(0xFFFFE1)), WHITE);
    }

    #[test]
    fn windows_95_and_98_take_the_measured_table() {
        // The survey's examples measured apart from the table (the TESTTC picture's patches)
        // that are not within a step of a border (see `table_9x` for the four that are), and
        // its example of where the nearest color would say maroon.
        let examples = [
            (0xD4D0C8, 0xC0C0C0),
            (0x0A246A, 0x000080),
            (0xA6CAF0, 0xFFFFFF),
            (0x1084D0, 0x00FFFF),
            (0x800040, 0x800000),
            (0xFFFFE1, 0xFFFFFF),
            (0x492800, 0x808000),
        ];
        for (from, to) in examples {
            assert_eq!(table_9x(rgb(from)), rgb(to), "{from:06X}");
        }
        // Its first line (blue 0, green 0), as measured.
        let first = "0000000000000000111111111111111111111111111111119999999999999999";
        for (r, digit) in first.chars().enumerate() {
            let v = ((r * 255 + 31) / 63) as u32;
            assert_eq!(table_9x(rgb(v << 16)), vga(digit.to_digit(16).unwrap() as usize), "r{r}");
        }
        assert_eq!(TABLE_9X.len(), 4096 * 65);
    }

    #[test]
    fn pictures_take_one_of_the_16_and_a_hard_edge() {
        let icon = Icon { width: 3, height: 1, pixels: vec![0x40FF_FFFF, 0xC0D4_D0C8, 0xFF3A_6EA5] };
        assert_eq!(picture(Era::Win2000, &icon).pixels, [0, SILVER, TEAL]);
        assert_eq!(picture(Era::Win95, &icon).pixels, [0, SILVER, NAVY]);
    }

    #[test]
    fn the_tooltip_is_white_with_two_yellow_dots_in_every_four_by_four() {
        let theme = crate::theme::theme_in(Era::Win98, Depth::Sixteen);
        let mut fb = Framebuffer::new(8, 8, 0);
        fill(&mut fb, theme, Rect::new(0, 0, 8, 8), rgb(0xFFFFE1));
        let dots: Vec<(i32, i32)> =
            (0..8).flat_map(|y| (0..8).map(move |x| (x, y))).filter(|&(x, y)| fb.get(x, y) == YELLOW).collect();
        assert_eq!(dots, [(2, 0), (6, 0), (0, 2), (4, 2), (2, 4), (6, 4), (0, 6), (4, 6)]);
        assert!(fb.pixels.iter().all(|&p| p == WHITE || p == YELLOW));
        // Windows 2000 never dots an area; True Color is untouched.
        fill(&mut fb, crate::theme::theme_in(Era::Win2000, Depth::Sixteen), Rect::new(0, 0, 8, 8), rgb(0xFFFFE1));
        assert!(fb.pixels.iter().all(|&p| p == WHITE));
        fill(&mut fb, crate::theme::theme(Era::Win98), Rect::new(0, 0, 8, 8), rgb(0xFFFFE1));
        assert!(fb.pixels.iter().all(|&p| p == rgb(0xFFFFE1)));
    }

    #[test]
    fn a_selected_icon_is_a_navy_checkerboard() {
        let icon = Icon { width: 2, height: 2, pixels: vec![rgb(0xFFFF00), rgb(0xFFFF00), 0, rgb(0xFFFF00)] };
        // Counted from the picture's own corner: its corner navy (My Computer on the three PCs).
        assert_eq!(selected(&icon, (0, 0)).pixels, [NAVY, rgb(0xFFFF00), 0, NAVY]);
        assert_eq!(selected(&icon, (1, 0)).pixels, [rgb(0xFFFF00), NAVY, 0, rgb(0xFFFF00)]);
    }
}
