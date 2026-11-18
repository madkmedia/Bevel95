//! The vertical banners down the left side of the Start menu: "Windows95", "Windows98" and
//! "Windows 2000 Professional", pixel for pixel.
//!
//! Cut from the Reference PCs' Start menus (first from GUIdebook's screenshots; found identical
//! on 2026-11-18, see `art/src/banner.rs`). Windows draws the picture at the bottom of the
//! banner and fills the rest with its top color, so a taller menu just gets more of that color
//! above it.
//!
//! The pictures themselves are Windows' and kept in bevel-art (`art/src/banner.rs`); this
//! module chooses and draws them.

use crate::art::{self, Look};
use crate::fb::{Framebuffer, Rect};
use crate::theme::rgb;

/// A banner picture: rows top to bottom, one character per pixel, colors from palette.
#[derive(Debug)]
pub struct Banner {
    pub width: i32,
    /// The color above the picture.
    pub fill: u32,
    pub palette: &'static [(u8, u32)],
    pub rows: &'static [&'static str],
}

impl Banner {
    /// The banner of `look`, from bevel-art (Windows' pictures).
    const fn art(look: Look) -> Banner {
        let r = art::rows("start/banner", look);
        Banner { width: r.w, fill: rgb(art::colors("start/banner-top", look)[0]), palette: r.palette, rows: r.rows }
    }

    /// Paints the banner into `r`: the picture against the bottom edge (its top cut off if
    /// `r` is shorter), `fill` above it.
    pub fn paint(&self, fb: &mut Framebuffer, r: Rect) {
        fb.fill(r, self.fill);
        let top = r.bottom() - self.rows.len() as i32;
        for (i, row) in self.rows.iter().enumerate() {
            let y = top + i as i32;
            if y < r.y {
                continue;
            }
            for (x, ch) in row.bytes().enumerate().take(r.w.max(0) as usize) {
                let color = self.palette.iter().find(|(c, _)| *c == ch).map_or(self.fill, |&(_, color)| rgb(color));
                fb.set(r.x + x as i32, y, color);
            }
        }
    }
}

/// Windows 95: light gray letters with white edges on dark gray.
pub static WIN95_BANNER: Banner = Banner::art(Look::Win95);

/// Windows 2000 Professional: white letters on a dithered black-to-blue gradient.
pub static WIN2000_BANNER: Banner = Banner::art(Look::Win2000);

/// Windows 98: white letters on Windows 98's blue, which runs dark at the top and lighter
/// toward the bottom (as dithered in the original 16-color picture).
pub static WIN98_BANNER: Banner = Banner::art(Look::Win98);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_are_complete_and_use_known_colors() {
        for banner in [&WIN95_BANNER, &WIN98_BANNER, &WIN2000_BANNER] {
            for row in banner.rows {
                assert_eq!(row.len() as i32, banner.width);
                assert!(row.bytes().all(|b| banner.palette.iter().any(|(c, _)| *c == b)), "{row}");
            }
        }
    }

    #[test]
    fn picture_sits_at_the_bottom_and_is_cut_at_the_top() {
        let mut fb = Framebuffer::new(21, 300, 0);
        WIN95_BANNER.paint(&mut fb, Rect::new(0, 0, 21, 300));
        assert_eq!(fb.get(0, 0), rgb(0x808080));
        let last = WIN95_BANNER.rows.last().unwrap().as_bytes();
        assert_eq!(fb.get(0, 299), if last[0] == b'g' { rgb(0x808080) } else { fb.get(0, 299) });
        let mut short = Framebuffer::new(21, 10, 0);
        WIN2000_BANNER.paint(&mut short, Rect::new(0, 0, 21, 10));
        assert!(short.pixels.iter().all(|&p| p != 0), "every pixel painted");
    }
}
