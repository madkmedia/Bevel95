//! Windows 98 HTML Help's non-repeating welcome-page background.

use std::sync::OnceLock;

use crate::fb::Framebuffer;
use crate::theme::rgb;

const WIDTH: usize = 478;
const HEIGHT: usize = 322;
const BACKDROP: &[u8] = crate::art::bytes("html-help/welcome", crate::art::Look::Win98);

// The first 768 bytes are the RGB palette. Each packet holds 1–128 palette
// indices, either a literal sequence or a repeated index (high bit set).
fn backdrop() -> &'static [u32] {
    static PIXELS: OnceLock<Vec<u32>> = OnceLock::new();
    PIXELS.get_or_init(|| {
        let (colors, mut packets) = BACKDROP.split_at(256 * 3);
        let (entries, _) = colors.as_chunks::<3>();
        let palette: Vec<_> = entries.iter().map(|&[r, g, b]| rgb(u32::from_be_bytes([0, r, g, b]))).collect();
        let mut pixels = Vec::with_capacity(WIDTH * HEIGHT);
        while let Some((&tag, rest)) = packets.split_first() {
            let len = usize::from(tag & 0x7f) + 1;
            if tag & 0x80 != 0 {
                pixels.extend(std::iter::repeat_n(palette[usize::from(rest[0])], len));
                packets = &rest[1..];
            } else {
                pixels.extend(rest[..len].iter().map(|&index| palette[usize::from(index)]));
                packets = &rest[len..];
            }
        }
        assert_eq!(pixels.len(), WIDTH * HEIGHT);
        pixels
    })
}

pub(super) fn paint(fb: &mut Framebuffer, scroll_y: i32) {
    fb.pixels.fill(rgb(0xffffff));
    let first = scroll_y.max(0) as usize;
    let stride = fb.width as usize;
    let width = WIDTH.min(stride);
    let rows = HEIGHT.saturating_sub(first).min(fb.height as usize);
    if width == 0 || rows == 0 {
        return;
    }
    let pixels = backdrop();
    for y in 0..rows {
        let src = (first + y) * WIDTH;
        let dst = y * stride;
        fb.pixels[dst..dst + width].copy_from_slice(&pixels[src..src + width]);
    }
}
