//! The classic volume control: the small window that opens when you click the speaker in the
//! notification area (Windows' "sndvol32 /t"): "Volume", a rising wedge, a slider and a Mute box.
//!
//! Measured on Windows 95, 98 and 2000 on the reference PCs (800×600 True Color; see
//! docs/theme-spec.md): 81×136 with a raised window edge; its text is in MS Sans Serif even in
//! 2000 (the old dialog font, "MS Shell Dlg", not Tahoma). The slider's thumb travels 54
//! pixels, the top being the loudest. Windows 95 and 98 show the same window in their colors.
//! Opened with the mouse, Windows 2000 shows no focus rectangle and no access-key underline
//! until a key is pressed; Windows 95 and 98 always show them.

use crate::fb::{Framebuffer, Rect};
use crate::layout::place_popup;
use crate::paint::{CHECK_ART, Menu, MenuEntry};
use crate::text::Font;
use crate::theme::{Colors, Era, Theme};

/// The window's size.
pub const SIZE: (i32, i32) = (81, 136);
/// The slider control (its focus rectangle's outline), the groove and the thumb's size.
const SLIDER: Rect = Rect::new(27, 27, 30, 81);
const CHANNEL: Rect = Rect::new(37, 35, 4, 65);
const THUMB_X: i32 = 29;
const THUMB: (i32, i32) = (21, 11);
/// How far the thumb's top moves, from the groove's top (loudest) down.
pub const TRAVEL: i32 = CHANNEL.h - THUMB.1;
/// The wedge left of the slider: its top-left corner, width and height.
const WEDGE: Rect = Rect::new(11, 34, 11, 66);
/// The Mute check box and where its label starts.
const MUTE_BOX: Rect = Rect::new(9, 117, 13, 13);
const MUTE_TEXT: (i32, i32) = (27, 117);
/// "Volume" is centered across the window, its cell this far down.
const TITLE_Y: i32 = 10;

const TITLE: &str = "Volume";
const MUTE: &str = "&Mute";

/// Which control has the keyboard focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    Slider,
    Mute,
}

/// A part of the window, for clicks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Thumb,
    /// The slider above the thumb (louder) or below it (softer).
    Above,
    Below,
    Mute,
}

/// What the window shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VolumeControl {
    /// The volume, 0.0 to 1.0.
    pub level: f32,
    pub muted: bool,
    pub focus: Focus,
    /// Show the focus rectangle and the access key (after a key press, or always in 95/98).
    pub show_keys: bool,
    /// The Mute box held down with the mouse (its inside turns face colored, as in Windows).
    pub mute_pressed: bool,
}

/// The speaker's right-click menu, the same in Windows 95, 98 and 2000 (the reference PCs):
/// "Open Volume Controls" in bold (what a double-click does), then "Adjust Audio Properties".
pub fn speaker_menu(show_keys: bool) -> Menu {
    let mut open = MenuEntry::item("&Open Volume Controls", None);
    open.bold = true;
    Menu { show_keys, ..Menu::new(vec![open, MenuEntry::item("&Adjust Audio Properties", None)]) }
}

/// Where the volume control, `w`×`h`, opens for a click with the pointer at (x, y) (any
/// units). Windows 95 and 98 put it on the screen's bottom edge, over the taskbar, as
/// TrackPopupMenu would at the pointer's x there: right of the pointer if it fits, else left of
/// it (95's opened left of it, 98's right of it on the reference PCs). Windows 2000 opens it at
/// the pointer itself, flipped up and left where it does not fit.
pub fn place(era: Era, x: i32, y: i32, w: i32, h: i32, screen: Rect) -> (i32, i32) {
    let y = if era == Era::Win2000 { y } else { screen.bottom() };
    place_popup(x, y, w, h, screen)
}

/// Where the speaker's menu opens: in Windows 95 on the screen's bottom edge like its volume
/// control; in Windows 98 and 2000 at the pointer (TrackPopupMenu's rule). The reference PCs.
pub fn place_menu(era: Era, x: i32, y: i32, w: i32, h: i32, screen: Rect) -> (i32, i32) {
    let y = if era == Era::Win95 { screen.bottom() } else { y };
    place_popup(x, y, w, h, screen)
}

/// The thumb's top edge for `level`.
pub fn thumb_y(level: f32) -> i32 {
    CHANNEL.y + ((1.0 - level.clamp(0.0, 1.0)) * TRAVEL as f32).round() as i32
}

/// The level whose thumb top is at `y` (clamped to the slider's ends).
pub fn level_at(y: i32) -> f32 {
    1.0 - ((y - CHANNEL.y).clamp(0, TRAVEL) as f32 / TRAVEL as f32)
}

/// The thumb's rectangle for `level`.
pub fn thumb(level: f32) -> Rect {
    Rect::new(THUMB_X, thumb_y(level), THUMB.0, THUMB.1)
}

/// The part of the window at (x, y), for the window as `v` shows it.
pub fn part_at(v: &VolumeControl, font: &Font, x: i32, y: i32) -> Option<Part> {
    let t = thumb(v.level);
    if t.contains(x, y) {
        return Some(Part::Thumb);
    }
    if SLIDER.contains(x, y) {
        return Some(if y < t.y { Part::Above } else { Part::Below });
    }
    let label = Rect::new(MUTE_TEXT.0 - 1, MUTE_TEXT.1, font.measure_label(MUTE) + 2, font.height);
    (MUTE_BOX.contains(x, y) || label.contains(x, y)).then_some(Part::Mute)
}

/// A raised edge (outer light and dark shadow, inner highlight and shadow), as Windows draws
/// windows and push buttons.
fn raised(fb: &mut Framebuffer, r: Rect, c: &Colors) {
    fb.frame(r, c.light, c.dark_shadow);
    fb.frame(r.inset(1, 1, 1, 1), c.highlight, c.shadow);
}

/// The dotted focus rectangle, as Windows' DrawFocusRect draws it: every other pixel of the
/// outline is XORed with `with`. The slider's starts one pixel after the corner and inverts
/// (white); the Mute box's starts on the corner and is XORed with the face color, so its dots
/// are black on the face and turn the access key's black underline face-colored where they
/// cross it (the reference PC).
/// On a 16-color screen the XOR works on the colors' numbers among the 16.
fn focus_rect(fb: &mut Framebuffer, theme: &Theme, r: Rect, phase: i32, with: u32) {
    let sixteen = theme.depth == crate::colors16::Depth::Sixteen;
    let mut dot = |x: i32, y: i32| {
        if ((x - r.x) + (y - r.y)) % 2 == phase && x >= 0 && y >= 0 && x < fb.width && y < fb.height {
            let p = fb.get(x, y);
            let xored = match (sixteen, crate::colors16::index(p), crate::colors16::index(with)) {
                (true, Some(a), Some(b)) => crate::colors16::vga(a ^ b),
                _ => p ^ (with & 0x00FF_FFFF),
            };
            fb.set(x, y, xored);
        }
    };
    for x in r.x..r.right() {
        dot(x, r.y);
        dot(x, r.bottom() - 1);
    }
    for y in r.y + 1..r.bottom() - 1 {
        dot(r.x, y);
        dot(r.right() - 1, y);
    }
}

/// Paints the window filling the framebuffer (made `SIZE`); `font` is MS Sans Serif in every
/// look.
pub fn paint(fb: &mut Framebuffer, theme: &Theme, font: &Font, v: &VolumeControl) {
    let c = &theme.colors;
    let all = Rect::new(0, 0, fb.width, fb.height);
    fb.fill(all, c.face);
    raised(fb, all, c);

    let title_x = (SIZE.0 - font.measure(TITLE)) / 2;
    font.draw(fb, title_x, TITLE_Y, TITLE, c.text);

    // The wedge: a highlighted top edge and slope, a shadowed right edge; the slope moves one
    // column right every six rows.
    let w = WEDGE;
    for row in 0..w.h {
        fb.set(w.x + row / 6, w.y + row, c.highlight);
    }
    fb.hline(w.x, w.y, w.w - 1, c.highlight);
    fb.vline(w.right() - 1, w.y, w.h - 1, c.shadow);

    // The slider: a sunken groove and a raised thumb.
    let g = CHANNEL;
    fb.frame(g, c.shadow, c.highlight);
    fb.frame(g.inset(1, 1, 1, 1), c.dark_shadow, c.light);
    let t = thumb(v.level);
    fb.fill(t, c.face);
    fb.frame(t, c.highlight, c.dark_shadow);
    fb.frame(t.inset(1, 1, 1, 1), c.light, c.shadow);
    if v.show_keys && v.focus == Focus::Slider {
        focus_rect(fb, theme, SLIDER, 1, 0x00FF_FFFF);
    }

    // The Mute box: a sunken 13×13 square, white inside (face colored while held down).
    let b = MUTE_BOX;
    fb.frame(b, c.shadow, c.highlight);
    fb.frame(b.inset(1, 1, 1, 1), c.dark_shadow, c.light);
    fb.fill(b.inset(2, 2, 2, 2), if v.mute_pressed { c.face } else { c.highlight });
    if v.muted {
        for (y, row) in CHECK_ART.iter().enumerate() {
            for (x, px) in row.bytes().enumerate() {
                if px == b'#' {
                    fb.set(b.x + 3 + x as i32, b.y + 3 + y as i32, c.text);
                }
            }
        }
    }
    font.draw_label(fb, MUTE_TEXT, MUTE, i32::MAX, c.text, v.show_keys);
    if v.show_keys && v.focus == Focus::Mute {
        let r = Rect::new(MUTE_TEXT.0 - 1, MUTE_TEXT.1, font.measure_label(MUTE) + 2, font.height);
        focus_rect(fb, theme, r, 0, c.face);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_thumb_travels_from_the_top_loudest_to_the_bottom() {
        // Windows 2000 on the reference PC: the thumb's top at y 35 (Home), 62 (half), 89 (End).
        assert_eq!((thumb_y(1.0), thumb_y(0.5), thumb_y(0.0)), (35, 62, 89));
        assert_eq!(TRAVEL, 54);
        assert_eq!((level_at(35), level_at(89), level_at(0), level_at(200)), (1.0, 0.0, 1.0, 0.0));
        assert!((level_at(62) - 0.5).abs() < 0.01);
    }

    #[test]
    fn it_opens_where_windows_opens_it() {
        let screen = Rect::new(0, 0, 800, 600);
        // Windows 95 (the reference PC), the pointer's tip at (733, 589) on the speaker: the
        // volume control at (652, 464), the speaker's menu (174×40) at (559, 560).
        assert_eq!(place(Era::Win95, 733, 589, SIZE.0, SIZE.1, screen), (652, 464));
        assert_eq!(place_menu(Era::Win95, 733, 589, 174, 40, screen), (559, 560));
        // Windows 98, the pointer's tip at (710, 586): the volume control at (710, 464), the
        // menu at (536, 546).
        assert_eq!(place(Era::Win98, 710, 586, SIZE.0, SIZE.1, screen), (710, 464));
        assert_eq!(place_menu(Era::Win98, 710, 586, 174, 40, screen), (536, 546));
        // Windows 2000, the pointer's tip at (796, 587): the volume control at (715, 451).
        assert_eq!(place(Era::Win2000, 796, 587, SIZE.0, SIZE.1, screen), (715, 451));
    }
}
