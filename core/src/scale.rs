//! Enlarging the 1× picture for the screen, and the one rule for lengths and points.
//!
//! Every window is painted at the original size (1×) first. A whole size (1, 2, 3, ...) makes
//! every 1× pixel an n×n block (`upscale`), exactly as Bevel95 always did. A size between whole
//! numbers comes in quarter steps (1.25, 1.5, 1.75, 2.25, ...; `Scale`): then a 1× pixel cannot
//! cover a whole number of screen pixels, and the person chooses how the screen pixels that lie
//! across two 1× pixels are drawn (`Way`). The four ways give exactly what the definition the
//! owner chose from gives (`private/probes/scaling/Scale.cs`; `core/tests/scaling.rs` proves it).
//!
//! The geometry, one rule for the whole project (`Scale::at`, `Scale::len`, `Scale::old`):
//! - A screen pixel belongs to the 1× pixel under its centre, which is the 1× pixel that covers
//!   most of it: `old(X) = floor((X + 1/2) / f)`. Every click, drag and hover goes through it.
//! - A 1× position x is the first screen pixel that belongs to it: `at(x) = ceil(x·f - 1/2)`,
//!   so `old(at(x)) == x` always, and screen pixels `at(x) .. at(x + 1)` all belong to x.
//! - A picture w pixels wide takes `len(w) = ceil(w·f)` screen pixels: never fewer than it
//!   needs. The last screen pixel may reach past the picture; what lies past it is drawn as if
//!   the picture's last row and column went on (every way clamps at the edges).
//!
//! At a whole size n all three are the old ones (x·n, w·n, X / n), so nothing changes there.

use std::sync::OnceLock;

use crate::fb::{Framebuffer, Rect};

/// The integer scale for a monitor: max(1, round(dpi / 96)), halves rounding up
/// (120 DPI = 125% -> 1, 144 DPI = 150% -> 2, 240 DPI = 250% -> 3). The automatic size is
/// always a whole number; a quarter size is only ever the person's own choice.
pub fn scale_for_dpi(dpi: u32) -> i32 {
    ((dpi + 48) / 96).max(1) as i32
}

/// Every 1× pixel becomes a `factor`×`factor` block: the whole sizes' way, the only one used
/// at a whole size.
pub fn upscale(src: &Framebuffer, factor: i32) -> Framebuffer {
    assert!(factor >= 1, "scale factor must be at least 1");
    let (w, h) = (src.width * factor, src.height * factor);
    let mut out = Vec::with_capacity((w * h) as usize);
    for y in 0..src.height {
        let start = out.len();
        for &p in &src.pixels[(y * src.width) as usize..((y + 1) * src.width) as usize] {
            out.extend(std::iter::repeat_n(p, factor as usize));
        }
        for _ in 1..factor {
            out.extend_from_within(start..start + w as usize);
        }
    }
    Framebuffer { width: w, height: h, pixels: out }
}

/// How a size between whole numbers is drawn (`scaling` in settings.txt). At a whole size every
/// way draws the same: clean n×n blocks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Way {
    /// Every 1× pixel covers exactly f×f screen pixels; a screen pixel lying across 1× pixels
    /// gets their mix by area, the stored color values mixed (the mock-ups' B1, the default).
    #[default]
    Even,
    /// The same, mixed in linear light (B2): edges between dark and light look a little lighter.
    EvenLight,
    /// Each screen pixel takes the 1× pixel under its centre (A): sharp, but the 1× pixels come
    /// out unevenly wide.
    Nearest,
    /// Ordinary bilinear smoothing (C).
    Smooth,
}

impl Way {
    pub const ALL: [Way; 4] = [Way::Even, Way::EvenLight, Way::Nearest, Way::Smooth];

    /// The word in settings.txt.
    pub fn setting(self) -> &'static str {
        match self {
            Way::Even => "even",
            Way::EvenLight => "even-light",
            Way::Nearest => "nearest",
            Way::Smooth => "smooth",
        }
    }

    pub fn from_setting(value: &str) -> Option<Way> {
        Way::ALL.into_iter().find(|w| w.setting().eq_ignore_ascii_case(value.trim()))
    }

    /// Its name in plain words (PROVISIONAL, the brief's).
    pub fn title(self) -> &'static str {
        match self {
            Way::Even => "Even pixels",
            Way::EvenLight => "Even pixels, lighter edges",
            Way::Nearest => "Sharp, uneven pixels",
            Way::Smooth => "Smooth",
        }
    }
}

/// The smallest and largest sizes, in quarters (1× and 8×).
pub const MIN_QUARTERS: i32 = 4;
pub const MAX_QUARTERS: i32 = 32;

/// A size: a whole number or a quarter step between (1, 1.25, 1.5, ... 8), and how it is
/// drawn when it is not whole. All of a window's geometry goes through it (the module's rule).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Scale {
    /// The size in quarters: 4 = 1×, 7 = 1.75×, 8 = 2×.
    quarters: i32,
    pub way: Way,
}

/// Its label: "2", "1.75".
impl std::fmt::Display for Scale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label())
    }
}

impl Default for Scale {
    fn default() -> Scale {
        Scale::ONE
    }
}

impl Scale {
    pub const ONE: Scale = Scale { quarters: 4, way: Way::Even };

    /// The whole size `n` (1 to 8; less is 1, more is 8).
    pub const fn whole(n: i32) -> Scale {
        Scale::from_quarters(n.saturating_mul(4))
    }

    /// The size `quarters`/4 (4 to 32; less is 1×, more is 8×), drawn the default way.
    pub const fn from_quarters(quarters: i32) -> Scale {
        let q = if quarters < MIN_QUARTERS {
            MIN_QUARTERS
        } else if quarters > MAX_QUARTERS {
            MAX_QUARTERS
        } else {
            quarters
        };
        Scale { quarters: q, way: Way::Even }
    }

    pub const fn with_way(self, way: Way) -> Scale {
        Scale { way, ..self }
    }

    /// The size in quarters (4 = 1×).
    pub const fn quarters(self) -> i32 {
        self.quarters
    }

    /// The size if it is a whole number.
    pub const fn whole_number(self) -> Option<i32> {
        if self.quarters % 4 == 0 { Some(self.quarters / 4) } else { None }
    }

    pub const fn is_whole(self) -> bool {
        self.quarters % 4 == 0
    }

    /// The whole number at or above the size (a whole number for what must be one: a font's
    /// or an icon's pixel size asked of Windows).
    pub const fn ceil(self) -> i32 {
        (self.quarters + 3) / 4
    }

    /// The screen position of the 1× position `x`: the first screen pixel that belongs to it,
    /// ceil(x·f - 1/2).
    pub const fn at(self, x: i32) -> i32 {
        (x * self.quarters + 1).div_euclid(4)
    }

    /// The screen length of a picture (or a gap) `w` 1× pixels long: ceil(w·f).
    pub const fn len(self, w: i32) -> i32 {
        (w * self.quarters + 3).div_euclid(4)
    }

    /// The 1× pixel a screen pixel `x` belongs to: the one under its centre, floor((x + 1/2)/f).
    pub const fn old(self, x: i32) -> i32 {
        (4 * x + 2).div_euclid(self.quarters)
    }

    /// The fewest 1× pixels whose picture covers `screen` screen pixels: ceil(screen / f).
    pub const fn old_cover(self, screen: i32) -> i32 {
        (4 * screen + self.quarters - 1).div_euclid(self.quarters)
    }

    /// The most 1× pixels whose picture fits in `screen` screen pixels: floor(screen / f).
    pub const fn old_fit(self, screen: i32) -> i32 {
        (4 * screen).div_euclid(self.quarters)
    }

    /// The screen pixels of the 1× rectangle `r`: from `at` of its left and top edges to `at`
    /// of its right and bottom edges (the screen pixels that belong to it).
    pub const fn rect(self, r: Rect) -> Rect {
        let (x, y) = (self.at(r.x), self.at(r.y));
        Rect::new(x, y, self.at(r.x + r.w) - x, self.at(r.y + r.h) - y)
    }

    /// The screen pixels that may look different once the 1× rectangle `r` is painted again: at
    /// a whole size its own (`rect`); between whole numbers one 1× pixel more all round, since a
    /// screen pixel there mixes (or, smoothed, reaches) the 1× pixels beside it.
    pub const fn dirty(self, r: Rect) -> Rect {
        if self.is_whole() { self.rect(r) } else { self.rect(Rect::new(r.x - 1, r.y - 1, r.w + 2, r.h + 2)) }
    }

    /// The size as settings.txt and the menus write it: "2", "1.25", "1.5", "1.75".
    pub fn label(self) -> String {
        let (whole, rest) = (self.quarters / 4, self.quarters % 4);
        match rest {
            0 => format!("{whole}"),
            1 => format!("{whole}.25"),
            2 => format!("{whole}.5"),
            _ => format!("{whole}.75"),
        }
    }

    /// Reads a size from settings.txt or a command line: a number from 1 to 8. Another number
    /// than a quarter step goes to the nearest one; the flag says whether it was one already.
    pub fn parse(text: &str) -> Option<(Scale, bool)> {
        let value: f64 = text.trim().parse().ok()?;
        if !value.is_finite() {
            return None;
        }
        let q = (value * 4.0).round();
        if !(f64::from(MIN_QUARTERS)..=f64::from(MAX_QUARTERS)).contains(&q) {
            return None;
        }
        Some((Scale::from_quarters(q as i32), q / 4.0 == value))
    }

    /// The factor in lowest terms, k/m (1.75 = 7/4, 1.5 = 3/2, 2 = 2/1).
    fn fraction(self) -> (i32, i32) {
        let g = gcd(self.quarters, 4);
        (self.quarters / g, 4 / g)
    }

    /// The 1× picture `src` enlarged for the screen: `len(width)` by `len(height)` pixels.
    pub fn enlarge(self, src: &Framebuffer) -> Framebuffer {
        match self.whole_number() {
            Some(n) => upscale(src, n),
            None => self.enlarge_part(src, Rect::new(0, 0, self.len(src.width), self.len(src.height))),
        }
    }

    /// The screen pixels `part` (screen coordinates of the enlarged picture) of `src` enlarged:
    /// each pixel exactly what `enlarge` gives there, so a part painted again joins the rest.
    pub fn enlarge_part(self, src: &Framebuffer, part: Rect) -> Framebuffer {
        let (pw, ph) = (part.w.max(0), part.h.max(0));
        let mut out = Framebuffer::new(pw, ph, 0);
        if src.width <= 0 || src.height <= 0 || pw == 0 || ph == 0 {
            return out;
        }
        let (k, m) = self.fraction();
        let way = if m == 1 { Way::Nearest } else { self.way };
        match way {
            Way::Nearest => nearest(src, k, m, part, &mut out),
            Way::Even => area(src, k, m, part, &mut out, false),
            Way::EvenLight => area(src, k, m, part, &mut out, true),
            Way::Smooth => bilinear(src, k, m, part, &mut out),
        }
        out
    }
}

fn gcd(mut a: i32, mut b: i32) -> i32 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a.max(1)
}

/// A: each screen pixel takes the 1× pixel under its centre, floor((X + 0.5)·m/k).
fn nearest(src: &Framebuffer, k: i32, m: i32, part: Rect, out: &mut Framebuffer) {
    let pick = |x: i32, size: i32| ((2 * x.max(0) + 1) * m / (2 * k)).min(size - 1);
    let cols: Vec<usize> = (part.x..part.right()).map(|x| pick(x, src.width) as usize).collect();
    let mut last_row = -1;
    for (oy, y) in (part.y..part.bottom()).enumerate() {
        let sy = pick(y, src.height);
        let row = oy * out.width as usize;
        if sy == last_row {
            // The same 1× row as the screen row above: a copy of it.
            out.pixels.copy_within(row - out.width as usize..row, row);
            continue;
        }
        last_row = sy;
        let line = &src.pixels[(sy * src.width) as usize..((sy + 1) * src.width) as usize];
        for (o, &sx) in out.pixels[row..row + out.width as usize].iter_mut().zip(&cols) {
            *o = line[sx];
        }
    }
}

/// Which 1× pixels the m enlarged pixels of a screen pixel come from (B): the first `n` of
/// them from `a`, the rest from `b` (a block of m ≤ k enlarged pixels meets at most two 1×
/// pixels). Past the picture's edge the last 1× pixel goes on.
#[derive(Clone, Copy)]
struct Span {
    a: usize,
    b: usize,
    n: i32,
}

fn span(x: i32, k: i32, m: i32, size: i32) -> Span {
    let old = |i: i32| (i / k).min(size - 1);
    let first = x.max(0) * m;
    let a = old(first);
    let n = (first..first + m).take_while(|&i| old(i) == a).count() as i32;
    Span { a: a as usize, b: old(first + m - 1) as usize, n }
}

/// sRGB stored value to linear light, as the definition computes it.
fn to_linear(v: u32) -> f64 {
    let c = f64::from(v) / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

/// Linear light back to a stored value, rounded half to even (C#'s Math.Round).
fn from_linear(l: f64) -> u32 {
    let c = if l <= 0.0031308 { l * 12.92 } else { 1.055 * l.powf(1.0 / 2.4) - 0.055 };
    (c * 255.0).round_ties_even().clamp(0.0, 255.0) as u32
}

/// The tables of the linear way: every stored value in linear light, and for m = 2 and 4 what
/// a screen pixel lying wholly on one 1× pixel of each value comes out as (the m·m sums in the
/// definition's own order: the same additions, the same result).
struct Light {
    linear: [f64; 256],
    /// `same[0]` for m = 2, `same[1]` for m = 4.
    same: [[u32; 256]; 2],
}

fn light() -> &'static Light {
    static LIGHT: OnceLock<Light> = OnceLock::new();
    LIGHT.get_or_init(|| {
        let linear: [f64; 256] = std::array::from_fn(|v| to_linear(v as u32));
        let same = [2, 4].map(|m: i32| {
            std::array::from_fn(|v| {
                let mut sum = 0.0;
                for _ in 0..m * m {
                    sum += linear[v];
                }
                from_linear(sum / f64::from(m * m))
            })
        });
        Light { linear, same }
    })
}

/// Half to even, as C#'s Math.Round of `sum / n` (n a power of two: the division is exact).
fn round_even(sum: u32, n: u32) -> u32 {
    let (q, r) = (sum / n, sum % n);
    if 2 * r > n || (2 * r == n && q & 1 == 1) { q + 1 } else { q }
}

/// B1 and B2: the picture enlarged k times without smoothing, then every m×m block averaged.
fn area(src: &Framebuffer, k: i32, m: i32, part: Rect, out: &mut Framebuffer, linear: bool) {
    let cols: Vec<Span> = (part.x..part.right()).map(|x| span(x, k, m, src.width)).collect();
    let n = (m * m) as u32;
    let light = linear.then(light);
    let which = if m == 2 { 0 } else { 1 };
    let w = src.width as usize;
    for (oy, y) in (part.y..part.bottom()).enumerate() {
        let r = span(y, k, m, src.height);
        let (top, bottom) = (&src.pixels[r.a * w..(r.a + 1) * w], &src.pixels[r.b * w..(r.b + 1) * w]);
        let row = oy * out.width as usize;
        for (o, c) in out.pixels[row..row + out.width as usize].iter_mut().zip(&cols) {
            let (p00, p10, p01, p11) = (top[c.a], top[c.b], bottom[c.a], bottom[c.b]);
            if p00 == p10 && p00 == p01 && p00 == p11 {
                *o = match light {
                    None => p00,
                    Some(l) => {
                        let s = &l.same[which];
                        (p00 & 0xFF00_0000) | (s[(p00 >> 16 & 0xFF) as usize] << 16) | (s[(p00 >> 8 & 0xFF) as usize] << 8) | s[(p00 & 0xFF) as usize]
                    }
                };
                continue;
            }
            let (wa, wb) = (c.n as u32, (m - c.n) as u32);
            let (ha, hb) = (r.n as u32, (m - r.n) as u32);
            let channel = |shift: u32| {
                let v = |p: u32| p >> shift & 0xFF;
                round_even(v(p00) * wa * ha + v(p10) * wb * ha + v(p01) * wa * hb + v(p11) * wb * hb, n)
            };
            let alpha = channel(24) << 24;
            *o = match light {
                None => alpha | (channel(16) << 16) | (channel(8) << 8) | channel(0),
                Some(l) => {
                    // The definition's own order of additions: row by row of the m×m block,
                    // each enlarged pixel once.
                    let mut sum = [0.0f64; 3];
                    for j in 0..m {
                        let line = if j < r.n { (p00, p10) } else { (p01, p11) };
                        for i in 0..m {
                            let p = if i < c.n { line.0 } else { line.1 };
                            sum[0] += l.linear[(p >> 16 & 0xFF) as usize];
                            sum[1] += l.linear[(p >> 8 & 0xFF) as usize];
                            sum[2] += l.linear[(p & 0xFF) as usize];
                        }
                    }
                    let nf = f64::from(n);
                    alpha | (from_linear(sum[0] / nf) << 16) | (from_linear(sum[1] / nf) << 8) | from_linear(sum[2] / nf)
                }
            };
        }
    }
}

/// Where a screen pixel's centre falls for ordinary bilinear smoothing: the two 1× pixels and
/// the share of the second (edges clamped).
#[derive(Clone, Copy)]
struct Between {
    a: usize,
    b: usize,
    t: f64,
}

fn between(x: i32, k: i32, m: i32, size: i32) -> Between {
    let mut f = (f64::from(x) + 0.5) * f64::from(m) / f64::from(k) - 0.5;
    if f < 0.0 {
        f = 0.0;
    }
    if f > f64::from(size - 1) {
        f = f64::from(size - 1);
    }
    let a = f.floor() as i32;
    Between { a: a as usize, b: (a + 1).min(size - 1) as usize, t: f - f64::from(a) }
}

/// C: ordinary bilinear smoothing between pixel centres, the stored values mixed.
fn bilinear(src: &Framebuffer, k: i32, m: i32, part: Rect, out: &mut Framebuffer) {
    let cols: Vec<Between> = (part.x..part.right()).map(|x| between(x, k, m, src.width)).collect();
    let w = src.width as usize;
    for (oy, y) in (part.y..part.bottom()).enumerate() {
        let r = between(y, k, m, src.height);
        let (top, bottom) = (&src.pixels[r.a * w..(r.a + 1) * w], &src.pixels[r.b * w..(r.b + 1) * w]);
        let (ty, uy) = (r.t, 1.0 - r.t);
        let row = oy * out.width as usize;
        for (o, c) in out.pixels[row..row + out.width as usize].iter_mut().zip(&cols) {
            let (p00, p10, p01, p11) = (top[c.a], top[c.b], bottom[c.a], bottom[c.b]);
            if p00 == p10 && p00 == p01 && p00 == p11 {
                *o = p00;
                continue;
            }
            let (tx, ux) = (c.t, 1.0 - c.t);
            let channel = |shift: u32| {
                let v = |p: u32| f64::from(p >> shift & 0xFF);
                let mixed = uy * (ux * v(p00) + tx * v(p10)) + ty * (ux * v(p01) + tx * v(p11));
                (mixed.round_ties_even() as u32).min(255)
            };
            *o = (channel(24) << 24) | (channel(16) << 16) | (channel(8) << 8) | channel(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpi_to_scale() {
        let cases = [(96, 1), (120, 1), (144, 2), (168, 2), (192, 2), (240, 3), (288, 3), (48, 1)];
        for (dpi, want) in cases {
            assert_eq!(scale_for_dpi(dpi), want, "dpi {dpi}");
        }
    }

    #[test]
    fn upscale_repeats_pixels() {
        let src = Framebuffer { width: 2, height: 1, pixels: vec![1, 2] };
        let out = upscale(&src, 2);
        assert_eq!((out.width, out.height), (4, 2));
        assert_eq!(out.pixels, [1, 1, 2, 2, 1, 1, 2, 2]);
        assert_eq!(upscale(&src, 1), src);
    }

    #[test]
    fn whole_sizes_keep_the_old_geometry() {
        for n in 1..=8 {
            let s = Scale::whole(n);
            assert_eq!((s.whole_number(), s.is_whole(), s.ceil()), (Some(n), true, n));
            for x in -50..300 {
                assert_eq!(s.at(x), x * n, "at {x} size {n}");
                assert_eq!(s.old(x), x.div_euclid(n), "old {x} size {n}");
                if x >= 0 {
                    assert_eq!(s.len(x), x * n);
                    assert_eq!(s.old_cover(x), (x + n - 1) / n);
                    assert_eq!(s.old_fit(x), x / n);
                }
            }
        }
    }

    #[test]
    fn every_quarter_size_maps_points_both_ways() {
        for q in MIN_QUARTERS..=MAX_QUARTERS {
            let s = Scale::from_quarters(q);
            let f = f64::from(q) / 4.0;
            for x in -60..400 {
                // Back and forth: a 1× position's first screen pixel belongs to it.
                assert_eq!(s.old(s.at(x)), x, "q {q} x {x}");
                // The screen pixels between two 1× positions all belong to the first.
                for screen in s.at(x)..s.at(x + 1) {
                    assert_eq!(s.old(screen), x, "q {q} x {x} screen {screen}");
                }
                // The centre rule, said with real numbers.
                assert_eq!(s.old(x), ((f64::from(x) + 0.5) / f).floor() as i32, "q {q} screen {x}");
                if x >= 0 {
                    // A picture never gets fewer screen pixels than it needs, nor a whole one more.
                    let need = f64::from(x) * f;
                    assert!(f64::from(s.len(x)) >= need && f64::from(s.len(x)) < need + 1.0, "q {q} len {x}");
                    assert!(s.at(x) <= s.len(x));
                    // The 1× length for a screen length: covering it, or fitting in it.
                    let cover = f64::from(s.old_cover(x));
                    assert!(s.len(s.old_cover(x)) >= x && cover * f >= f64::from(x) && (cover - 1.0) * f < f64::from(x));
                    assert!(s.len(s.old_fit(x)) <= x + 1 && f64::from(s.old_fit(x)) * f <= f64::from(x));
                }
            }
        }
    }

    #[test]
    fn sizes_read_and_written() {
        assert_eq!(Scale::parse("1.75"), Some((Scale::from_quarters(7), true)));
        assert_eq!(Scale::parse(" 3 "), Some((Scale::whole(3), true)));
        assert_eq!(Scale::parse("2.2"), Some((Scale::from_quarters(9), false)));
        assert_eq!(Scale::parse("8"), Some((Scale::whole(8), true)));
        for bad in ["0.5", "9", "8.2", "", "two", "NaN", "inf", "-1"] {
            assert_eq!(Scale::parse(bad), None, "{bad}");
        }
        for q in MIN_QUARTERS..=MAX_QUARTERS {
            let s = Scale::from_quarters(q);
            assert_eq!(Scale::parse(&s.label()), Some((s, true)));
        }
        assert_eq!(Scale::from_quarters(7).label(), "1.75");
        assert_eq!(Scale::from_quarters(10).label(), "2.5");
        assert_eq!(Scale::whole(2).label(), "2");
        assert_eq!((Scale::from_quarters(1), Scale::from_quarters(99)), (Scale::ONE, Scale::whole(8)));
        for way in Way::ALL {
            assert_eq!(Way::from_setting(way.setting()), Some(way));
        }
        assert_eq!(Way::default(), Way::Even);
    }

    #[test]
    fn whole_sizes_enlarge_as_before_in_every_way() {
        let src = Framebuffer { width: 3, height: 2, pixels: vec![1, 2, 3, 0xFF00_00FF, 0xFFFF_FFFF, 0] };
        for n in 1..=4 {
            for way in Way::ALL {
                let s = Scale::whole(n).with_way(way);
                assert_eq!(s.enlarge(&src), upscale(&src, n));
                let part = s.enlarge_part(&src, Rect::new(1, 1, 3 * n - 1, 2 * n - 1));
                let whole = upscale(&src, n);
                for y in 0..part.height {
                    for x in 0..part.width {
                        assert_eq!(part.get(x, y), whole.get(x + 1, y + 1));
                    }
                }
            }
        }
    }

    #[test]
    fn a_part_is_the_same_as_the_whole_there() {
        let pixels = (0..7 * 5).map(|i: u32| 0xFF00_0000 | i.wrapping_mul(0x9E37_79B9) >> 8).collect();
        let src = Framebuffer { width: 7, height: 5, pixels };
        for q in [5, 6, 7, 9, 10, 11, 13, 14, 15] {
            for way in Way::ALL {
                let s = Scale::from_quarters(q).with_way(way);
                let whole = s.enlarge(&src);
                assert_eq!((whole.width, whole.height), (s.len(7), s.len(5)));
                let r = Rect::new(2, 1, whole.width - 3, whole.height - 2);
                let part = s.enlarge_part(&src, r);
                for y in 0..r.h {
                    for x in 0..r.w {
                        assert_eq!(part.get(x, y), whole.get(x + r.x, y + r.y), "q {q} {way:?} {x},{y}");
                    }
                }
            }
        }
    }

    #[test]
    fn one_color_stays_that_color() {
        let src = Framebuffer::new(5, 4, 0xFFC0_C0C0);
        for q in [5, 6, 7, 9, 31] {
            for way in Way::ALL {
                let big = Scale::from_quarters(q).with_way(way).enlarge(&src);
                assert!(big.pixels.iter().all(|&p| p == 0xFFC0_C0C0), "q {q} {way:?}");
            }
        }
    }
}
