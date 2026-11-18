//! Tooltips: the small pale yellow boxes that name what is under the pointer (a notification
//! icon, the clock's date). Windows' tooltip control draws them the same way in 95, 98 and
//! 2000: a 1px Window Frame border, Info Background inside, Info Text in the menu font, the
//! text 2px in from the border at the sides and 1px at the top and bottom (one line is 17
//! high). Measured on screenshots S10 (2000) and S13 (98), whose wrapped lines count their
//! trailing space, as DrawText does.

use crate::fb::{Framebuffer, Rect};
use crate::text::Font;
use crate::theme::Theme;

/// Space between the border and the text, at the sides and at the top and bottom.
const PAD_X: i32 = 2;
const PAD_Y: i32 = 1;

/// The lines of a tooltip: a program's text may hold line breaks.
fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.split('\n').map(|line| line.trim_end_matches('\r'))
}

/// The tooltip's size for `text`, border included.
pub fn size(font: &Font, text: &str) -> (i32, i32) {
    let widest = lines(text).map(|line| font.measure(line)).max().unwrap_or(0);
    let count = lines(text).count() as i32;
    (1 + PAD_X + widest + PAD_X + 1, 1 + PAD_Y + count * font.height + PAD_Y + 1)
}

/// Where a tooltip `w`×`h` goes (its top-left corner) for the pointer at `pointer`, the way
/// Windows' tooltip control places it: at the pointer's x and `below` under it; when that runs
/// past `work`'s bottom (the bar is there), on top of the thing it describes, its bottom edge on
/// `tool`'s top edge; then moved left to end `margin` inside the screen's right edge. Windows 98
/// on the reference PC: the clock's tooltip stays put whichever part of the clock the pointer
/// is on, and ends 1px before the right edge. Works in any units (the platform uses screen
/// pixels).
#[allow(clippy::too_many_arguments)]
pub fn place(pointer: (i32, i32), w: i32, h: i32, tool: Rect, screen: Rect, work: Rect, below: i32, margin: i32) -> (i32, i32) {
    let mut y = pointer.1 + below;
    if y + h > work.bottom() {
        y = tool.y - h;
    }
    let x = pointer.0.min(screen.right() - margin - w).max(screen.x);
    (x, y.max(screen.y))
}

/// Paints a tooltip for `text` filling the framebuffer (made `size` big).
pub fn paint(fb: &mut Framebuffer, theme: &Theme, font: &Font, text: &str) {
    let c = &theme.colors;
    let all = Rect::new(0, 0, fb.width, fb.height);
    crate::colors16::fill(fb, theme, all, c.tip_face);
    fb.frame(all, c.tip_border, c.tip_border);
    for (i, line) in lines(text).enumerate() {
        font.draw(fb, 1 + PAD_X, 1 + PAD_Y + i as i32 * font.height, line, c.tip_text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tooltips_sit_on_top_of_the_tray_and_inside_the_screen() {
        // Windows 98 on an 800×600 screen, the bar from y = 572: the clock's tooltip (142×17)
        // for the pointer at (759, 589) or (759, 583) is at (657, 561), on top of the clock
        // (from y = 578); the first icon's (123×17, the icon from y = 579) at (676, 562).
        let screen = Rect::new(0, 0, 800, 600);
        let work = Rect::new(0, 0, 800, 572);
        let clock = Rect::new(744, 578, 52, 18);
        assert_eq!(place((759, 589), 142, 17, clock, screen, work, 20, 1), (657, 561));
        assert_eq!(place((759, 583), 142, 17, clock, screen, work, 20, 1), (657, 561));
        assert_eq!(place((705, 589), 123, 17, Rect::new(703, 579, 16, 16), screen, work, 20, 1), (676, 562));
        // With room below the pointer: at the pointer, below it.
        assert_eq!(place((100, 100), 50, 17, Rect::new(90, 95, 16, 16), screen, work, 20, 1), (100, 120));
    }

    #[test]
    fn one_line_is_17_high_and_6_wider_than_its_text() {
        let font = Font::new(
            "t".into(),
            false,
            13,
            11,
            vec![
                crate::text::Glyph { ch: '?', advance: 5, ink_x: 0, ink_w: 0, ink: Vec::new() },
                crate::text::Glyph { ch: 'a', advance: 6, ink_x: 0, ink_w: 0, ink: Vec::new() },
            ],
        )
        .expect("a font");
        assert_eq!(size(&font, "aa"), (18, 17));
        assert_eq!(size(&font, "a\naaa\r\n"), (24, 43));
    }
}
