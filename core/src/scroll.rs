//! A list's vertical scroll bar: where its thumb is, what a click or a drag does, how it is
//! drawn. Measured in Windows 2000's Start Menu Settings list (Taskbar Properties, Advanced)
//! and shared by Bevel95 Settings' tree of programs and the Remove Shortcuts/Folders window.

use crate::fb::{Framebuffer, Rect};
use crate::properties::scroll_arrow;
use crate::theme::Colors;

/// A scroll bar: its place (16 wide, a 16px button at each end, the track between them), and
/// the list's rows: `total`, of which `page` show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ScrollBar {
    pub bar: Rect,
    pub total: usize,
    pub page: usize,
}

/// What of a scroll bar takes clicks: the two buttons, the thumb, and the track above and
/// below it (a click there turns a page).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ScrollRects {
    pub up: Rect,
    pub down: Rect,
    pub thumb: Rect,
    pub page_up: Rect,
    pub page_down: Rect,
}

impl ScrollBar {
    fn track(self) -> Rect {
        Rect::new(self.bar.x, self.bar.y + 16, 16, self.bar.h - 32)
    }

    /// The thumb: its length is the track's share of the rows shown, rounded (as Windows'
    /// MulDiv rounds); its place follows `top`, the first row shown.
    pub fn thumb(self, top: usize) -> Rect {
        let track = self.track();
        let (total, page) = (self.total.max(1) as i32, self.page as i32);
        let len = ((track.h * page + total / 2) / total).max(8);
        let range = (total - page).max(1);
        let pos = ((track.h - len) * top as i32 + range / 2) / range;
        Rect::new(track.x, track.y + pos, 16, len)
    }

    /// The first row shown once the thumb, grabbed while `grab_top` was the first, has been
    /// dragged `dy` pixels: one row per (track - thumb) / (rows - page), rounded.
    pub fn dragged(self, grab_top: i32, dy: i32) -> usize {
        let span = (self.track().h - self.thumb(0).h).max(1);
        let range = self.total.saturating_sub(self.page) as i32;
        let delta = (dy * range + dy.signum() * span / 2) / span;
        (grab_top + delta).clamp(0, range) as usize
    }

    /// What takes clicks while `top` is the first row shown.
    pub fn rects(self, top: usize) -> ScrollRects {
        let (bar, thumb) = (self.bar, self.thumb(top));
        ScrollRects {
            up: Rect::new(bar.x, bar.y, 16, 16),
            down: Rect::new(bar.x, bar.bottom() - 16, 16, 16),
            thumb,
            page_up: Rect::new(bar.x, bar.y + 16, 16, thumb.y - bar.y - 16),
            page_down: Rect::new(bar.x, thumb.bottom(), 16, bar.bottom() - 16 - thumb.bottom()),
        }
    }

    /// Paints it: the track's checkerboard, the buttons (each pushed in while held), the thumb.
    pub fn paint(self, fb: &mut Framebuffer, c: &Colors, top: usize, up_held: bool, down_held: bool) {
        self.paint_from(fb, c, top, up_held, down_held, (0, 0));
    }

    /// paint with the track's checkerboard counted from origin (Windows lines it up with the
    /// list's own window: Display Properties' lists, measured on the Reference PCs).
    pub fn paint_from(self, fb: &mut Framebuffer, c: &Colors, top: usize, up_held: bool, down_held: bool, origin: (i32, i32)) {
        let bar = self.bar;
        for y in bar.y + 16..bar.bottom() - 16 {
            for x in bar.x..bar.right() {
                fb.set(x, y, if (x - origin.0 + y - origin.1).rem_euclid(2) == 1 { c.highlight } else { c.face });
            }
        }
        scroll_arrow(fb, c, Rect::new(bar.x, bar.y, 16, 16), true, up_held);
        scroll_arrow(fb, c, Rect::new(bar.x, bar.bottom() - 16, 16, 16), false, down_held);
        let thumb = self.thumb(top);
        fb.fill(thumb, c.face);
        fb.frame(thumb, c.light, c.dark_shadow);
        fb.frame(thumb.inset(1, 1, 1, 1), c.highlight, c.shadow);
    }
}
