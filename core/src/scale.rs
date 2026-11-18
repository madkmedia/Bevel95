//! Integer nearest-neighbour upscaling: every 1× pixel becomes a `factor`×`factor` block.
//! No smoothing anywhere, so the result is exactly as crisp as the 1× image.

use crate::fb::Framebuffer;

/// The integer scale for a monitor: max(1, round(dpi / 96)), halves rounding up
/// (120 DPI = 125% -> 1, 144 DPI = 150% -> 2, 240 DPI = 250% -> 3).
pub fn scale_for_dpi(dpi: u32) -> i32 {
    ((dpi + 48) / 96).max(1) as i32
}

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
}
