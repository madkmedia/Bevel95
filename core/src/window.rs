//! Windows' window frame, title bar and menu bar, as Windows 95, 98 and 2000 drew them around
//! a program: the raised frame, the caption with the program's icon, its name and the Minimize,
//! Maximize and Close buttons (or a dialog's "?" and "X"), in front or not, and the menu bar's
//! titles. The dialog windows use the frame and caption (`dialog::window_frame`); a program's
//! main window uses all of it (`paint_frame`, `paint_menu_bar`).
//!
//! Measured on the reference PCs in True Color (docs/theme-spec.md, "Program windows"): the
//! real Minesweeper on Windows 95 and 2000, Calculator on Windows 98, and dialogs of all three.
//! The geometry is the same in every look; colors, gradients and how a chosen menu title
//! shows differ.

use crate::fb::{Framebuffer, Icon, Rect};
use crate::text::Fonts;
use crate::colors16;
use crate::theme::{Colors, Era, Theme, rgb};

/// The frame around every window measured: a 2px raised edge and 1px of face color.
pub const FRAME: i32 = 3;
/// The caption: 18 rows, right inside the frame.
pub const CAPTION_H: i32 = 18;
/// The menu bar: 18 rows, one face-colored row below the caption; another such row follows it.
pub const MENU_BAR_Y: i32 = FRAME + CAPTION_H + 1;
pub const MENU_BAR_H: i32 = 18;
/// Where a window's contents start: without a menu bar, and with one.
pub const CLIENT_Y: i32 = FRAME + CAPTION_H + 1;
pub const CLIENT_Y_WITH_MENU: i32 = MENU_BAR_Y + MENU_BAR_H + 1;
/// The 16×14 title bar buttons, 5 rows below the window's top.
pub const TITLE_BUTTON: (i32, i32) = (16, 14);

/// The size of a window with a menu bar around contents of `client` size.
pub const fn outer_size(client: (i32, i32)) -> (i32, i32) {
    (client.0 + 2 * FRAME, client.1 + CLIENT_Y_WITH_MENU + FRAME)
}

/// A title bar button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TitleButton {
    Help,
    Minimize,
    Maximize,
    Close,
}

/// Where a title bar button sits in a window `width` wide: Close 21 from the right edge; "?"
/// (a dialog's) or Maximize 18 left of it; Minimize right against Maximize.
pub const fn title_button_rect(width: i32, button: TitleButton) -> Rect {
    let x = match button {
        TitleButton::Close => width - 21,
        TitleButton::Help | TitleButton::Maximize => width - 39,
        TitleButton::Minimize => width - 55,
    };
    Rect::new(x, 5, TITLE_BUTTON.0, TITLE_BUTTON.1)
}

/// One button of a title bar as it shows now.
#[derive(Clone, Copy, Debug)]
pub struct ButtonLook {
    pub button: TitleButton,
    pub rect: Rect,
    /// Held down with the mouse.
    pub pressed: bool,
    /// A fixed-size window's Maximize is grayed out.
    pub enabled: bool,
}

/// A window's frame and title bar.
#[derive(Clone, Copy, Debug)]
pub struct Frame<'a> {
    pub era: Era,
    pub title: &'a str,
    /// A program's 16×16 picture at the left end (dialogs have none in 95 and 98).
    pub icon: Option<&'a Icon>,
    /// The window in front: its caption has the strong colors.
    pub active: bool,
    /// Left to right.
    pub buttons: &'a [ButtonLook],
}

/// The caption's colors: where its gradient starts and ends (Windows 95: one color), and the
/// title's color.
fn caption_colors(era: Era, active: bool) -> ([i32; 3], [i32; 3], u32) {
    match (era, active) {
        (Era::Win95, true) => ([0x00, 0x00, 0x80], [0x00, 0x00, 0x80], rgb(0xFFFFFF)),
        (Era::Win95, false) => ([0x80, 0x80, 0x80], [0x80, 0x80, 0x80], rgb(0xC0C0C0)),
        (Era::Win98, true) => ([0x00, 0x00, 0x80], [0x10, 0x84, 0xD0], rgb(0xFFFFFF)),
        (Era::Win98, false) => ([0x80, 0x80, 0x80], [0xB5, 0xB5, 0xB5], rgb(0xC0C0C0)),
        (Era::Win2000, true) => ([0x0A, 0x24, 0x6A], [0xA6, 0xCA, 0xF0], rgb(0xFFFFFF)),
        (Era::Win2000, false) => ([0x80, 0x80, 0x80], [0xC0, 0xC0, 0xC0], rgb(0xD4D0C8)),
    }
}

fn pixel(c: [i32; 3]) -> u32 {
    rgb(((c[0] << 16) | (c[1] << 8) | c[2]) as u32)
}

/// Fills the caption `r`. Windows 95: one color. Windows 98 and 2000: the start color up to
/// `left` (under the icon), a gradient from there to `right`, the end color from `right` on
/// (under and between the buttons).
fn caption_fill(fb: &mut Framebuffer, theme: &Theme, era: Era, r: Rect, left: i32, right: i32, active: bool) {
    let (from, to, _) = caption_colors(era, active);
    if theme.depth.reduced() {
        // A 16-color screen has no gradient: 98 and 2000 fill the caption with its start
        // color, navy or gray (Windows 2000's 0A246A by the nearest of the 16, navy).
        fb.fill(r, colors16::convert(era, pixel(from)));
        return;
    }
    let visible = (right - left).max(1);
    match era {
        Era::Win95 => fb.fill(r, pixel(from)),
        Era::Win98 => {
            // Windows 98 works the gradient out for a width rounded up to a multiple of 16,
            // centered on the part that shows, with one step per pixel in 8.8 fixed point; each
            // pair of columns, counted from that wider span's left end, shares its first
            // column's color. So it depends only on the window, not on where the window is.
            // Exact on six windows of the reference PC at known places (dialogs with "?" and
            // "X" and with "X" alone, a program's window in front and not) and on GUIdebook's
            // Run, Shut Down and Log Off.
            let width = (visible + 15) / 16 * 16;
            let origin = left - (width - visible) / 2;
            for x in r.x..r.right() {
                let color = if x < left {
                    from
                } else if x >= right {
                    to
                } else {
                    let at = (x - origin).div_euclid(2) * 2;
                    std::array::from_fn(|k| from[k] + ((at * (((to[k] - from[k]) << 8) / width)) >> 8))
                };
                fb.vline(x, r.y, r.h, pixel(color));
            }
        }
        Era::Win2000 => {
            // Keep the fractional color step until the final channel conversion. A 16-bit
            // fraction loses one green level in the 393-column ShellAbout gradient.
            for x in r.x..r.right() {
                let i = i64::from(x - left);
                let w = i64::from(visible);
                let color = if i < 0 {
                    from
                } else if i >= w {
                    to
                } else {
                    std::array::from_fn(|k| {
                        let (f, t) = (i64::from(from[k]), i64::from(to[k]));
                        (((f << 32) + i * (((t - f) << 32) / w)) >> 32) as i32
                    })
                };
                fb.vline(x, r.y, r.h, pixel(color));
            }
        }
    }
}

const HELP_GLYPH: &[&str] = crate::art::rows("window/help-glyph", crate::art::ANY).rows;
const CLOSE_GLYPH: &[&str] = crate::art::rows("window/close-glyph", crate::art::ANY).rows;
const MINIMIZE_GLYPH: &[&str] = crate::art::rows("window/minimize-glyph", crate::art::ANY).rows;
const MAXIMIZE_GLYPH: &[&str] = crate::art::rows("window/maximize-glyph", crate::art::ANY).rows;

fn glyph(fb: &mut Framebuffer, rows: &[&str], x: i32, y: i32, color: u32) {
    for (dy, row) in rows.iter().enumerate() {
        for (dx, b) in row.bytes().enumerate() {
            if b == b'#' {
                fb.set(x + dx as i32, y + dy as i32, color);
            }
        }
    }
}

/// A title bar button: raised (pushed in while held), its glyph black, or etched when grayed
/// out (gray over a white copy 1px down-right).
fn title_button(fb: &mut Framebuffer, c: &Colors, b: &ButtonLook) {
    let r = b.rect;
    fb.fill(r, c.face);
    if b.pressed {
        fb.frame(r, c.dark_shadow, c.highlight);
        fb.frame(r.inset(1, 1, 1, 1), c.shadow, c.light);
    } else {
        fb.frame(r, c.highlight, c.dark_shadow);
        fb.frame(r.inset(1, 1, 1, 1), c.light, c.shadow);
    }
    let rows: &[&str] = match b.button {
        TitleButton::Help => HELP_GLYPH,
        TitleButton::Close => CLOSE_GLYPH,
        TitleButton::Minimize => MINIMIZE_GLYPH,
        TitleButton::Maximize => MAXIMIZE_GLYPH,
    };
    let s = i32::from(b.pressed);
    let (x, y) = (r.x + 2 + s, r.y + 2 + s);
    if b.enabled {
        glyph(fb, rows, x, y, c.text);
    } else {
        glyph(fb, rows, x + 1, y + 1, c.highlight);
        glyph(fb, rows, x, y, c.shadow);
    }
}

/// Paints a window's frame and title bar over the whole framebuffer (the rest is left in the
/// face color for the caller's contents).
pub fn paint_frame(fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts, f: &Frame) {
    let c = &theme.colors;
    let outer = Rect::new(0, 0, fb.width, fb.height);
    fb.fill(outer, c.face);
    fb.frame(outer, c.light, c.dark_shadow);
    fb.frame(outer.inset(1, 1, 1, 1), c.highlight, c.shadow);
    let caption = Rect::new(FRAME, FRAME, fb.width - 2 * FRAME, CAPTION_H);
    // The icon's part (2 + 16) keeps the start color; the title starts 2 after it.
    let left = caption.x + if f.icon.is_some() { 18 } else { 0 };
    // The gradient ends at a dialog's "?", or 2px before Minimize or a lone "X".
    // Message boxes on 98 and 2000 extend the gradient right up to their disabled "X".
    let right = match f.buttons.first() {
        Some(b) if b.button == TitleButton::Help => b.rect.x,
        Some(b) if b.button == TitleButton::Close && !b.enabled => b.rect.x,
        Some(b) => b.rect.x - 2,
        None => caption.right(),
    };
    caption_fill(fb, theme, f.era, caption, left, right, f.active);
    if let Some(icon) = f.icon {
        colors16::draw_picture(fb, theme, icon, caption.x + 2, caption.y + 1);
    }
    fonts.bold.draw(fb, left + 2, caption.y + 2, f.title, colors16::line(theme, caption_colors(f.era, f.active).2));
    for b in f.buttons {
        title_button(fb, c, b);
    }
}

/// The frame of a window that can be resized: 4 pixels (2 of them face color) where a dialog's
/// is 3; none while it is maximized (Windows puts the frame past the screen's edges).
pub const fn sizable_border(maximized: bool) -> i32 {
    if maximized { 0 } else { 4 }
}

/// A sizable window's title bar button: where a fixed window's would be, moved in by the wider
/// frame (or out to the screen's edge while maximized).
pub fn sizable_button_rect(width: i32, maximized: bool, button: TitleButton) -> Rect {
    let delta = sizable_border(maximized) - FRAME;
    let r = title_button_rect(width - 2 * delta, button);
    Rect::new(r.x + delta, r.y + delta, r.w, r.h)
}

/// A resizable program's frame and title bar (Minimize, Maximize or Restore, Close) over the
/// whole framebuffer, the rest left in the face color (Solitaire's and the folder windows').
#[allow(clippy::too_many_arguments)]
pub fn paint_sizable_frame(
    fb: &mut Framebuffer,
    theme: &Theme,
    fonts: &Fonts,
    era: Era,
    title: &str,
    icon: &Icon,
    active: bool,
    pressed: Option<TitleButton>,
    maximized: bool,
) {
    let c = &theme.colors;
    let outer = Rect::new(0, 0, fb.width, fb.height);
    fb.fill(outer, c.face);
    if !maximized {
        fb.frame(outer, c.light, c.dark_shadow);
        fb.frame(outer.inset(1, 1, 1, 1), c.highlight, c.shadow);
    }
    let frame = sizable_border(maximized);
    let width = fb.width - 2 * frame;
    if width <= 0 {
        return;
    }
    let mut strip = Framebuffer::new(width + 2 * FRAME, 24, c.face);
    let buttons = [TitleButton::Minimize, TitleButton::Maximize, TitleButton::Close].map(|button| ButtonLook {
        button,
        rect: title_button_rect(strip.width, button),
        pressed: pressed == Some(button),
        enabled: true,
    });
    paint_frame(&mut strip, theme, fonts, &Frame { era, title, icon: Some(icon), active, buttons: &buttons });
    // The frame painter places this caption inside a 3px fixed window. Copy the caption into the
    // four-pixel sizing frame (or directly to the screen edge).
    for y in 0..CAPTION_H.min(fb.height - frame) {
        let src = ((y + FRAME) * strip.width + FRAME) as usize;
        let dst = ((y + frame) * fb.width + frame) as usize;
        fb.pixels[dst..dst + width as usize].copy_from_slice(&strip.pixels[src..src + width as usize]);
    }
    if maximized {
        let r = sizable_button_rect(fb.width, true, TitleButton::Maximize);
        fb.fill(r.inset(2, 2, 2, 2), c.face);
        let offset = i32::from(pressed == Some(TitleButton::Maximize));
        for (dx, dy) in [(5, 2), (3, 5)] {
            let b = Rect::new(r.x + dx + offset, r.y + dy + offset, 6, 6);
            fb.fill(b, c.face);
            fb.frame(b, c.text, c.text);
            fb.hline(b.x, b.y + 1, b.w, c.text);
        }
    }
}

/// How a menu bar's title shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TitleLook {
    Plain,
    /// The pointer is on it (Windows 2000 raises it; 95 and 98 show nothing).
    Hot,
    /// Chosen with the keyboard, or its menu is open.
    Chosen,
}

/// Where a menu bar's titles go in a window: side by side from the frame's inside edge, each
/// as wide as its label plus 6 on both sides.
pub fn menu_bar_layout(fonts: &Fonts, titles: &[&str]) -> Vec<Rect> {
    let mut x = FRAME;
    titles
        .iter()
        .map(|label| {
            let w = fonts.regular.measure_label(label) + 12;
            let r = Rect::new(x, MENU_BAR_Y, w, MENU_BAR_H);
            x += w;
            r
        })
        .collect()
}

/// Paints a menu bar's titles (over the face color `paint_frame` left). `looks`: how each
/// shows; `show_keys`: underline the access keys (Windows 2000 only after Alt was used);
/// `active`: the window is in front (Windows 98 and 2000 gray the titles of one that is not).
#[allow(clippy::too_many_arguments)]
pub fn paint_menu_bar(
    fb: &mut Framebuffer,
    theme: &Theme,
    fonts: &Fonts,
    era: Era,
    titles: &[&str],
    rects: &[Rect],
    looks: &[TitleLook],
    show_keys: bool,
    active: bool,
) {
    let c = &theme.colors;
    for ((label, &r), &look) in titles.iter().zip(rects).zip(looks) {
        let (mut color, mut shift) = (if active || era == Era::Win95 { c.text } else { c.shadow }, 0);
        match (era, look) {
            (_, TitleLook::Plain) | (Era::Win95 | Era::Win98, TitleLook::Hot) => {}
            (Era::Win95, TitleLook::Chosen) => {
                fb.fill(r, c.menu_highlight);
                color = c.menu_highlight_text;
            }
            (Era::Win98 | Era::Win2000, TitleLook::Chosen) => {
                fb.frame(r, c.shadow, c.highlight);
                shift = 1;
            }
            (Era::Win2000, TitleLook::Hot) => fb.frame(r, c.highlight, c.shadow),
        }
        fonts.regular.draw_label(fb, (r.x + 6 + shift, r.y + 2 + shift), label, i32::MAX, color, show_keys);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::Font;

    #[test]
    fn buttons_sit_where_windows_put_them() {
        // The real Minesweeper on Windows 2000, 170 wide: Minimize 115, Maximize 131, Close 149.
        assert_eq!(title_button_rect(170, TitleButton::Minimize), Rect::new(115, 5, 16, 14));
        assert_eq!(title_button_rect(170, TitleButton::Maximize), Rect::new(131, 5, 16, 14));
        assert_eq!(title_button_rect(170, TitleButton::Close), Rect::new(149, 5, 16, 14));
        // A dialog's "?" is where Maximize would be.
        assert_eq!(title_button_rect(347, TitleButton::Help), Rect::new(308, 5, 16, 14));
        // Its client area: 164 x 207 inside a 170 x 251 window.
        assert_eq!(outer_size((164, 207)), (170, 251));
    }

    #[test]
    fn menu_titles_are_their_labels_plus_twelve() {
        let regular = Font::parse(include_str!("../tests/fixtures/ms-sans-serif-11px.font")).unwrap();
        let bold = Font::parse(include_str!("../tests/fixtures/ms-sans-serif-11px-bold.font")).unwrap();
        let fonts = Fonts { regular: &regular, bold: &bold };
        // Windows 95's Minesweeper: "Game" 79..118 and "Help" from 119, in a window at 76.
        let rects = menu_bar_layout(&fonts, &["&Game", "&Help"]);
        assert_eq!(rects[0], Rect::new(3, 22, 40, 18));
        assert_eq!(rects[1].x, 43);
    }
}
