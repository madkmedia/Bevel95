//! Windows 7 Basic's taskbar (Windows 7 without its see-through glass): its layout and its
//! painting at 1x, from the pictures in bevel-art (`seven/...`, a first pass from published
//! screenshots). Not wired into the bar yet: the platform fills a [`SevenBar`] and calls
//! [`layout`] and [`paint`].
//!
//! Measured in `private/probes/win7/survey.md` (the screenshots [A] 1920x1080 and [B] 1440x900):
//! the bar is 40 rows high, two top lines and a fill; both ends are a darker zone with a 39-column
//! fade into the body (the right zone is the notification area, so its fade moves with it); the
//! Start orb sits on the left zone; program slots are 60x40, 62 apart from x=59; a pinned program
//! that is not running is its bare 32x32 picture, a running one has a framed button (partly
//! see-through), the window in front a lighter one; the notification area has the arrow for
//! hidden icons, 16x16 icons 24 apart and a two-line clock; Show desktop is the last 15 columns.
//!
//! PROVISIONAL (not in any screenshot, painted as noted): a button under the pointer, pressed or
//! flashing (painted as running), several windows of one program combined (painted as running),
//! the orb under the pointer (normal), the hidden-icons window and tooltips (not painted), a bar
//! with no notification icons (the arrow's place is a guess), the clock's letters (plain Tahoma
//! 8 pt standing in for Windows 7's smoothed Segoe UI 9 pt), the clock's width (given by the
//! caller), the button's see-through share on its right 24 columns and under its icon, the front
//! button's see-through share.

use crate::art::Look;
use crate::fb::{Framebuffer, Icon, Rect};
use crate::text::Font;

/// The bar's height at 1x.
pub const HEIGHT: i32 = 40;
/// The orb's picture: x 0..56 (it covers the left zone).
pub const ORB_W: i32 = 56;
/// The left zone's last column; the fade follows on columns 56..=94.
pub const LEFT_ZONE_END: i32 = 55;
/// The fade's width in columns, at both ends.
pub const FADE: i32 = 39;
/// The first program slot's x, a slot's size and the step from one slot to the next.
pub const FIRST_SLOT: i32 = 59;
pub const SLOT_W: i32 = 60;
pub const SLOT_STEP: i32 = 62;
/// A program's 32x32 picture within its slot.
pub const ICON_X: i32 = 14;
pub const ICON_Y: i32 = 4;
pub const ICON: i32 = 32;
/// Show desktop: the last columns of the bar.
pub const SHOW_DESKTOP_W: i32 = 15;
/// The notification area, counted from the right zone's first column: the arrow (9x7) at +6,
/// row 18; the first icon at +27, row 13, then every 24 columns.
pub const ARROW_X: i32 = 6;
pub const ARROW_Y: i32 = 18;
pub const ARROW_W: i32 = 9;
pub const ARROW_H: i32 = 7;
pub const TRAY_ICON_X: i32 = 27;
pub const TRAY_ICON_Y: i32 = 13;
pub const TRAY_ICON_STEP: i32 = 24;
pub const TRAY_ICON: i32 = 16;
/// The clock's space starts this many columns after the last icon's right edge.
pub const CLOCK_GAP: i32 = 9;
/// The rows where the time's and the date's letters start (the top of their digits).
pub const TIME_TOP: i32 = 8;
pub const DATE_TOP: i32 = 25;
/// The clock's letters: white.
pub const CLOCK_COLOR: u32 = 0xFFFF_FFFF;

/// What the orb shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Orb {
    #[default]
    Normal,
    /// While the Start menu is open (the glow; the only other state the screenshots show).
    MenuOpen,
}

/// What a program slot holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotKind {
    /// Pinned and not running: only its picture.
    Pinned,
    /// Running, not in front (also: several windows combined, flashing, under the pointer, all
    /// PROVISIONAL).
    Running,
    /// Its window is in front.
    Front,
}

/// One program slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slot {
    pub kind: SlotKind,
    /// Its 32x32 picture (None: nothing drawn).
    pub icon: Option<Icon>,
}

/// What the bar holds: everything the platform fills.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SevenBar {
    pub orb: Orb,
    pub slots: Vec<Slot>,
    /// The notification icons shown (16x16), left to right.
    pub tray_icons: Vec<Icon>,
    /// The clock's two lines.
    pub time: String,
    pub date: String,
    /// The clock's space in columns (from the last icon's right edge + 9 to the zone's end).
    /// Windows makes it wider than the letters (79 for "20/10/2018", 70 for "1/24/2025" in the
    /// screenshots); how it is chosen is not known: PROVISIONAL, the caller decides
    /// ([`clock_space_for`] is a stand-in).
    pub clock_space: i32,
}

/// Where everything goes on a bar `width` wide.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    pub width: i32,
    /// The right zone's first column (the notification area's left edge); the fade is the 39
    /// columns before it.
    pub zone: i32,
    pub slots: Vec<Rect>,
    pub arrow: Rect,
    pub tray_icons: Vec<Rect>,
    /// The clock's space, first and last column (inclusive).
    pub clock: (i32, i32),
    pub show_desktop: Rect,
}

/// The layout of a bar `width` wide with `slots` program slots, `icons` notification icons and a
/// clock space `clock_space` wide.
pub fn layout(width: i32, slots: usize, icons: usize, clock_space: i32) -> Layout {
    let show_desktop = Rect::new(width - SHOW_DESKTOP_W, 0, SHOW_DESKTOP_W, HEIGHT);
    let last = width - SHOW_DESKTOP_W - 1; // the zone's last column
    let clock_first = last - clock_space + 1;
    // The last icon's right edge is CLOCK_GAP columns before the clock's space.
    let icons_end = clock_first - CLOCK_GAP + 1; // one past the last icon
    let n = icons as i32;
    let zone = if n > 0 {
        icons_end - TRAY_ICON - TRAY_ICON_STEP * (n - 1) - TRAY_ICON_X
    } else {
        // PROVISIONAL: no screenshot shows a bar without notification icons.
        icons_end - TRAY_ICON_X
    };
    let tray_icons =
        (0..n).map(|i| Rect::new(zone + TRAY_ICON_X + TRAY_ICON_STEP * i, TRAY_ICON_Y, TRAY_ICON, TRAY_ICON)).collect();
    let slots = (0..slots as i32).map(|i| Rect::new(FIRST_SLOT + SLOT_STEP * i, 0, SLOT_W, HEIGHT)).collect();
    Layout {
        width,
        zone,
        slots,
        arrow: Rect::new(zone + ARROW_X, ARROW_Y, ARROW_W, ARROW_H),
        tray_icons,
        clock: (clock_first, last),
        show_desktop,
    }
}

/// The left column of a line of letters `ink_w` wide, centred in the clock's space (the odd
/// column to the right: rounded down).
pub fn clock_line_x(clock: (i32, i32), ink_w: i32) -> i32 {
    clock.0 + (clock.1 - clock.0 + 1 - ink_w).div_euclid(2)
}

/// A stand-in for Windows' choice of the clock's width: the wider line's letters and 20 columns
/// (PROVISIONAL: the screenshots had 21 and 18).
pub fn clock_space_for(font: &Font, time: &str, date: &str) -> i32 {
    ink(font, time).1.max(ink(font, date).1) + 20
}

/// Where a line's ink starts relative to the pen, and how wide it is.
pub fn ink(font: &Font, text: &str) -> (i32, i32) {
    let (mut pen, mut left, mut right) = (0, i32::MAX, i32::MIN);
    for ch in text.chars() {
        let g = font.glyph(ch);
        if g.ink_w > 0 {
            left = left.min(pen + g.ink_x);
            right = right.max(pen + g.ink_x + g.ink_w);
        }
        pen += g.advance;
    }
    if left > right { (0, 0) } else { (left, right - left) }
}

/// The first row of the font's cell with ink in a digit (to put the digits' tops on a row).
fn digit_top(font: &Font) -> i32 {
    let g = font.glyph('0');
    (0..font.height).find(|&row| (0..g.ink_w).any(|c| g.ink[(row * g.ink_w + c) as usize])).unwrap_or(0)
}

// ---- the pictures ----

const FADE_PIC: &[u8; 40 * 40 * 4] = crate::art::exactly("seven/bar-fade", Look::Win7);
const ZONE_PIC: &[u8; 40 * 4] = crate::art::exactly("seven/bar-zone", Look::Win7);
const ORB_PIC: &[u8; 56 * 80 * 4] = crate::art::exactly("seven/orb", Look::Win7);
const RUNNING_PIC: &[u8; 60 * 40 * 4] = crate::art::exactly("seven/button-running", Look::Win7);
const FRONT_PIC: &[u8; 60 * 40 * 4] = crate::art::exactly("seven/button-front", Look::Win7);
const ARROW_PIC: &[u8; 9 * 7 * 4] = crate::art::exactly("seven/tray-arrow", Look::Win7);
const SHOW_PIC: &[u8; 15 * 40 * 4] = crate::art::exactly("seven/show-desktop", Look::Win7);

/// Pixel (x, y) of an RGBA picture `w` wide, as 0xAARRGGBB.
fn px(rgba: &[u8], w: i32, x: i32, y: i32) -> u32 {
    let i = ((y * w + x) * 4) as usize;
    u32::from_be_bytes([rgba[i + 3], rgba[i], rgba[i + 1], rgba[i + 2]])
}

/// An RGBA picture as an icon (for drawing with its see-through share).
fn picture(rgba: &[u8], w: i32, h: i32) -> Icon {
    Icon {
        width: w,
        height: h,
        pixels: (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).map(|(x, y)| px(rgba, w, x, y)).collect(),
    }
}

/// The bar's background color at column `x`, row `y` (the fill with its zones and fades, the two
/// top lines, Show desktop).
pub fn background(l: &Layout, x: i32, y: i32) -> u32 {
    if x >= l.show_desktop.x {
        return px(SHOW_PIC, SHOW_DESKTOP_W, x - l.show_desktop.x, y);
    }
    if x <= LEFT_ZONE_END || x >= l.zone {
        return px(ZONE_PIC, 1, 0, y);
    }
    if x <= LEFT_ZONE_END + FADE {
        // the left fade is the right one mirrored
        return px(FADE_PIC, 40, FADE - (x - LEFT_ZONE_END - 1), y);
    }
    if x >= l.zone - FADE {
        return px(FADE_PIC, 40, x - (l.zone - 40), y);
    }
    px(FADE_PIC, 40, 0, y)
}

/// Paints the bar into `fb` (at least `l.width` x 40) at 1x. `font` draws the clock.
pub fn paint(fb: &mut Framebuffer, bar: &SevenBar, l: &Layout, font: &Font) {
    for y in 0..HEIGHT {
        for x in 0..l.width {
            fb.set(x, y, background(l, x, y));
        }
    }
    let orb_row = if bar.orb == Orb::MenuOpen { HEIGHT } else { 0 };
    for y in 0..HEIGHT {
        for x in 0..ORB_W {
            fb.set(x, y, px(ORB_PIC, ORB_W, x, orb_row + y));
        }
    }
    let running = picture(RUNNING_PIC, SLOT_W, HEIGHT);
    let front = picture(FRONT_PIC, SLOT_W, HEIGHT);
    for (slot, r) in bar.slots.iter().zip(&l.slots) {
        match slot.kind {
            SlotKind::Pinned => {}
            SlotKind::Running => fb.draw_icon(&running, r.x, r.y),
            SlotKind::Front => fb.draw_icon(&front, r.x, r.y),
        }
        if let Some(icon) = &slot.icon {
            fb.draw_icon(icon, r.x + ICON_X, r.y + ICON_Y);
        }
    }
    for y in 0..ARROW_H {
        for x in 0..ARROW_W {
            fb.set(l.arrow.x + x, l.arrow.y + y, px(ARROW_PIC, ARROW_W, x, y));
        }
    }
    for (icon, r) in bar.tray_icons.iter().zip(&l.tray_icons) {
        fb.draw_icon(icon, r.x, r.y);
    }
    let top = digit_top(font);
    for (text, row) in [(&bar.time, TIME_TOP), (&bar.date, DATE_TOP)] {
        let (ink_x, ink_w) = ink(font, text);
        let x = clock_line_x(l.clock, ink_w);
        font.draw(fb, x - ink_x, row - top, text, CLOCK_COLOR);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_are_60_wide_62_apart_from_59() {
        let l = layout(1920, 6, 4, 79);
        let xs: Vec<i32> = l.slots.iter().map(|r| r.x).collect();
        assert_eq!(xs, [59, 121, 183, 245, 307, 369]);
        assert!(l.slots.iter().all(|r| (r.w, r.h, r.y) == (60, 40, 0)));
    }

    #[test]
    fn the_notification_area_and_its_fade_follow_the_screenshots() {
        // [A]: 1920 wide, four icons, clock space 79: the zone starts at 1703, its fade 1664..1702
        let a = layout(1920, 6, 4, 79);
        assert_eq!(a.zone, 1703);
        assert_eq!(a.arrow, Rect::new(1709, 18, 9, 7));
        assert_eq!(a.tray_icons.iter().map(|r| r.x).collect::<Vec<_>>(), [1730, 1754, 1778, 1802]);
        assert_eq!(a.clock, (1826, 1904));
        assert_eq!(a.show_desktop, Rect::new(1905, 0, 15, 40));
        // [B]: 1440 wide, two icons, clock space 70: the zone starts at 1280
        let b = layout(1440, 4, 2, 70);
        assert_eq!(b.zone, 1280);
        assert_eq!(b.tray_icons.iter().map(|r| r.x).collect::<Vec<_>>(), [1307, 1331]);
        assert_eq!(b.clock, (1355, 1424));
        // the fade's first column moves with the zone
        let fade_start =
            |l: &Layout| (0..l.width).find(|&x| x > 94 && background(l, x, 20) != background(l, 500, 20)).unwrap();
        assert_eq!(fade_start(&a), 1703 - 39);
        assert_eq!(fade_start(&b), 1280 - 39);
    }

    #[test]
    fn the_clock_centres_each_line_rounding_down() {
        // even remainder: [B]'s date, 52 wide in 1355..1424 (70): 9 on each side
        assert_eq!(clock_line_x((1355, 1424), 52), 1364);
        // odd remainder: [A]'s date, 58 wide in 1826..1904 (79): 10 left, 11 right
        assert_eq!(clock_line_x((1826, 1904), 58), 1836);
        // the times: [A] 27 wide, [B] 43 wide
        assert_eq!(clock_line_x((1826, 1904), 27), 1852);
        assert_eq!(clock_line_x((1355, 1424), 43), 1368);
    }

    #[test]
    fn the_ends_are_darker_and_the_middle_is_the_body() {
        let l = layout(1024, 3, 2, 70);
        let body = background(&l, 500, 20);
        assert_eq!(body & 0xFF_FFFF, 0xA7C0DC);
        assert_eq!(background(&l, 10, 20) & 0xFF_FFFF, 0x8194AA);
        assert_eq!(background(&l, l.zone, 20) & 0xFF_FFFF, 0x8194AA);
        assert_eq!(background(&l, 500, 0) & 0xFF_FFFF, 0x434D58);
        assert_eq!(background(&l, 500, 1) & 0xFF_FFFF, 0xCAD9EA);
        // the left fade mirrors the right one
        for i in 0..FADE {
            assert_eq!(background(&l, LEFT_ZONE_END + 1 + i, 20), background(&l, l.zone - 1 - i, 20));
        }
    }
}
