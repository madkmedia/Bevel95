//! A plain 1× pixel canvas plus the few drawing primitives the classic look needs.

/// A rectangle in pixels. `w`/`h` may be zero (empty).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect { x, y, w, h }
    }
    pub const fn right(&self) -> i32 {
        self.x + self.w
    }
    pub const fn bottom(&self) -> i32 {
        self.y + self.h
    }
    /// The rectangle shrunk by the given amount on each side.
    pub const fn inset(&self, left: i32, top: i32, right: i32, bottom: i32) -> Rect {
        Rect::new(self.x + left, self.y + top, self.w - left - right, self.h - top - bottom)
    }
    pub const fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.right() && y < self.bottom()
    }
}

/// A small image with straight (not premultiplied) alpha, e.g. a 16×16 app icon.
/// Pixels are `0xAARRGGBB`, row by row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Icon {
    pub width: i32,
    pub height: i32,
    pub pixels: Vec<u32>,
}

impl Icon {
    /// The icon shrunk to `size`×`size`: each new pixel is the average of the pixels it
    /// covers (weighted by how much of each it covers, and by their alpha). Programs hand the
    /// notification area icons made for the screen's scale (24×24 at 150%); the classic one
    /// shows 16×16. Already that size (or empty): unchanged.
    pub fn downscaled(&self, size: i32) -> Icon {
        let (w, h) = (self.width, self.height);
        if (w == size && h == size) || w <= 0 || h <= 0 || size <= 0 {
            return self.clone();
        }
        // In units of 1/size of an old pixel (and 1/w or 1/h of a new one): old pixel i spans
        // [i·size, (i+1)·size), new pixel o spans [o·w, (o+1)·w) across.
        let overlap = |a0: i32, a1: i32, b0: i32, b1: i32| (a1.min(b1) - a0.max(b0)).max(0) as u64;
        let mut pixels = Vec::with_capacity((size * size) as usize);
        for oy in 0..size {
            let (y0, y1) = (oy * h, (oy + 1) * h);
            for ox in 0..size {
                let (x0, x1) = (ox * w, (ox + 1) * w);
                let (mut sum, mut total) = ([0u64; 4], 0u64);
                for iy in y0 / size..(y1 + size - 1) / size {
                    let wy = overlap(iy * size, (iy + 1) * size, y0, y1);
                    for ix in x0 / size..(x1 + size - 1) / size {
                        let weight = wy * overlap(ix * size, (ix + 1) * size, x0, x1);
                        let p = self.pixels[(iy * w + ix) as usize];
                        let a = u64::from(p >> 24) * weight;
                        sum[0] += a;
                        for (k, shift) in [16, 8, 0].into_iter().enumerate() {
                            sum[k + 1] += u64::from((p >> shift) & 0xFF) * a;
                        }
                        total += weight;
                    }
                }
                let alpha = (sum[0] + total / 2) / total.max(1);
                let channel = |k: usize| (sum[k] + sum[0] / 2).checked_div(sum[0]).unwrap_or(0);
                pixels.push(((alpha as u32) << 24) | ((channel(1) as u32) << 16) | ((channel(2) as u32) << 8) | channel(3) as u32);
            }
        }
        Icon { width: size, height: size, pixels }
    }
}

/// Pixels are `0xAARRGGBB`. Stored little-endian that is B,G,R,A in memory, which is the
/// layout of a 32-bit Windows DIB section, so the platform layer can copy rows straight in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Framebuffer {
    pub width: i32,
    pub height: i32,
    pub pixels: Vec<u32>,
}

impl Framebuffer {
    pub fn new(width: i32, height: i32, color: u32) -> Framebuffer {
        assert!(width >= 0 && height >= 0, "negative framebuffer size");
        Framebuffer { width, height, pixels: vec![color; (width * height) as usize] }
    }

    pub fn get(&self, x: i32, y: i32) -> u32 {
        self.pixels[(y * self.width + x) as usize]
    }

    /// Sets one pixel; does nothing outside the canvas.
    pub fn set(&mut self, x: i32, y: i32, color: u32) {
        if x >= 0 && y >= 0 && x < self.width && y < self.height {
            self.pixels[(y * self.width + x) as usize] = color;
        }
    }

    /// Fills `r`, clipped to the canvas.
    pub fn fill(&mut self, r: Rect, color: u32) {
        let (x0, x1) = (r.x.max(0), r.right().min(self.width));
        let (y0, y1) = (r.y.max(0), r.bottom().min(self.height));
        if x0 >= x1 {
            return;
        }
        for y in y0..y1 {
            let row = (y * self.width) as usize;
            self.pixels[row + x0 as usize..row + x1 as usize].fill(color);
        }
    }

    pub fn hline(&mut self, x: i32, y: i32, w: i32, color: u32) {
        self.fill(Rect::new(x, y, w, 1), color);
    }

    pub fn vline(&mut self, x: i32, y: i32, h: i32, color: u32) {
        self.fill(Rect::new(x, y, 1, h), color);
    }

    /// A 1px outline in the Windows 3D style: `tl` on the top and left lines, `br` on the
    /// bottom and right lines. As in Windows (and RetroBar's nested borders, and 98.css's
    /// box-shadows), the bottom/right lines own the bottom-left and top-right corners.
    pub fn frame(&mut self, r: Rect, tl: u32, br: u32) {
        if r.w <= 0 || r.h <= 0 {
            return;
        }
        self.hline(r.x, r.y, r.w - 1, tl);
        self.vline(r.x, r.y, r.h - 1, tl);
        self.hline(r.x, r.bottom() - 1, r.w, br);
        self.vline(r.right() - 1, r.y, r.h, br);
    }

    /// A 2×2 checkerboard filling `r`: `odd` where (x + y) is odd, counted from `origin`,
    /// `even` elsewhere. (Windows lines a pattern up with the window it paints, not with each
    /// shape in it: all pressed task buttons share one checkerboard.)
    pub fn checker(&mut self, r: Rect, origin: (i32, i32), even: u32, odd: u32) {
        for y in r.y..r.bottom() {
            for x in r.x..r.right() {
                let c = if ((x - origin.0) + (y - origin.1)).rem_euclid(2) == 1 { odd } else { even };
                self.set(x, y, c);
            }
        }
    }

    /// Draws `icon` with its top-left at (x, y), blending by its alpha.
    pub fn draw_icon(&mut self, icon: &Icon, x: i32, y: i32) {
        for iy in 0..icon.height {
            for ix in 0..icon.width {
                let (px, py) = (x + ix, y + iy);
                if px >= 0 && py >= 0 && px < self.width && py < self.height {
                    let src = icon.pixels[(iy * icon.width + ix) as usize];
                    let i = (py * self.width + px) as usize;
                    self.pixels[i] = blend(self.pixels[i], src);
                }
            }
        }
    }

    /// Copies `src` with its top-left at (x, y), clipped.
    pub fn blit(&mut self, src: &Framebuffer, x: i32, y: i32) {
        for sy in 0..src.height {
            for sx in 0..src.width {
                self.set(x + sx, y + sy, src.get(sx, sy));
            }
        }
    }

    /// Copies the part `part` of `src` with its top-left at (x, y), clipped.
    pub fn blit_part(&mut self, src: &Framebuffer, part: Rect, x: i32, y: i32) {
        for sy in 0..part.h {
            for sx in 0..part.w {
                self.set(x + sx, y + sy, src.get(part.x + sx, part.y + sy));
            }
        }
    }
}

/// `src` (straight alpha) over `dst` (opaque). Integer math, so results are exact.
fn blend(dst: u32, src: u32) -> u32 {
    let a = src >> 24;
    match a {
        0 => dst,
        255 => src,
        _ => {
            let channel = |shift: u32| {
                let s = (src >> shift) & 0xFF;
                let d = (dst >> shift) & 0xFF;
                (s * a + d * (255 - a) + 127) / 255
            };
            0xFF00_0000 | (channel(16) << 16) | (channel(8) << 8) | channel(0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_gives_bottom_right_the_corners() {
        let mut fb = Framebuffer::new(4, 3, 0);
        fb.frame(Rect::new(0, 0, 4, 3), 1, 2);
        #[rustfmt::skip]
        let want = [
            1, 1, 1, 2,
            1, 0, 0, 2,
            2, 2, 2, 2,
        ];
        assert_eq!(fb.pixels, want);
    }

    #[test]
    fn checker_starts_even_at_its_origin() {
        let mut fb = Framebuffer::new(3, 2, 0);
        fb.checker(Rect::new(0, 0, 3, 2), (0, 0), 5, 6);
        assert_eq!(fb.pixels, [5, 6, 5, 6, 5, 6]);
        // Counted from a point one to the left, the pattern is the other way round.
        fb.checker(Rect::new(0, 0, 3, 2), (-1, 0), 5, 6);
        assert_eq!(fb.pixels, [6, 5, 6, 5, 6, 5]);
    }

    #[test]
    fn fill_clips_and_ignores_empty() {
        let mut fb = Framebuffer::new(2, 2, 0);
        fb.fill(Rect::new(-5, -5, 6, 6), 7);
        fb.fill(Rect::new(1, 1, 0, 5), 9);
        assert_eq!(fb.pixels, [7, 0, 0, 0]);
    }

    #[test]
    fn downscaling_averages_what_each_pixel_covers() {
        // 4×4 → 2×2: each new pixel is one 2×2 block. Top left: two opaque white pixels and
        // two transparent ones give half-transparent white (the transparent pixels' color
        // does not count).
        #[rustfmt::skip]
        let pixels = vec![
            0xFFFFFFFF, 0x00000000, 0xFF000000, 0xFF000000,
            0x00123456, 0xFFFFFFFF, 0xFF000000, 0xFF000000,
            0xFFFF0000, 0xFFFF0000, 0xFF0000FF, 0xFFFF0000,
            0xFFFF0000, 0xFFFF0000, 0xFFFF0000, 0xFF0000FF,
        ];
        let small = Icon { width: 4, height: 4, pixels }.downscaled(2);
        assert_eq!(small.pixels, [0x80FFFFFF, 0xFF000000, 0xFFFF0000, 0xFF800080]);
        // 3 → 2: the middle old pixel is split between the two new ones.
        let row = Icon { width: 3, height: 1, pixels: vec![0xFF000000, 0xFFFFFFFF, 0xFF000000] };
        let small = Icon { width: 3, height: 3, pixels: [row.pixels.clone(), row.pixels.clone(), row.pixels].concat() };
        assert_eq!(small.downscaled(2).pixels, [0xFF555555; 4]);
        // The right size already: unchanged.
        let same = Icon { width: 2, height: 2, pixels: vec![1, 2, 3, 4] };
        assert_eq!(same.downscaled(2), same);
    }

    #[test]
    fn blend_is_exact_at_the_ends() {
        assert_eq!(blend(0xFF123456, 0x00FFFFFF), 0xFF123456);
        assert_eq!(blend(0xFF123456, 0xFFABCDEF), 0xFFABCDEF);
        assert_eq!(blend(0xFF000000, 0x80FFFFFF), 0xFF808080);
    }
}
