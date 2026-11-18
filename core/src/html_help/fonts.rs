//! Non-antialiased article and navigation fonts used by HTML Help. Each resource
//! holds just the advance, ink position and one-bit mask of each printable CP-1252
//! character at one display size.

use std::sync::LazyLock;

use crate::text::{Font, Glyph};

pub(super) fn body() -> &'static Font {
    static FONT: LazyLock<Font> =
        LazyLock::new(|| load(const { crate::art::bytes("html-help/font-verdana-11", crate::art::Look::Win98) }, "Verdana", false));
    &FONT
}

pub(super) fn bold() -> &'static Font {
    static FONT: LazyLock<Font> =
        LazyLock::new(|| load(const { crate::art::bytes("html-help/font-verdana-11-bold", crate::art::Look::Win98) }, "Verdana", true));
    &FONT
}

pub(super) fn welcome_heading() -> &'static Font {
    static FONT: LazyLock<Font> =
        LazyLock::new(|| load(const { crate::art::bytes("html-help/font-verdana-13-bold", crate::art::Look::Win98) }, "Verdana", true));
    &FONT
}

pub(super) fn heading() -> &'static Font {
    static FONT: LazyLock<Font> =
        LazyLock::new(|| load(const { crate::art::bytes("html-help/font-verdana-14-bold", crate::art::Look::Win98) }, "Verdana", true));
    &FONT
}

pub(super) fn tree98() -> &'static Font {
    static FONT: LazyLock<Font> =
        LazyLock::new(|| load(const { crate::art::bytes("html-help/font-arial-11", crate::art::Look::Win98) }, "Arial", false));
    &FONT
}

pub(super) fn web_heading() -> &'static Font {
    static FONT: LazyLock<Font> =
        LazyLock::new(|| load(const { crate::art::bytes("html-help/font-verdana-11-bold-italic", crate::art::Look::Win98) }, "Verdana", true));
    &FONT
}

fn load(bytes: &[u8], face: &str, bold: bool) -> Font {
    let height = i32::from(bytes[0]);
    let ascent = i32::from(bytes[1]);
    let mut remaining = &bytes[2..];
    let mut glyphs = Vec::new();
    while !remaining.is_empty() {
        let (header, data) = remaining.split_at(5);
        let code = u16::from_le_bytes([header[0], header[1]]);
        let ink_w = i32::from(header[4]);
        let pixels = (height * ink_w) as usize;
        let bitmap;
        (bitmap, remaining) = data.split_at(pixels.div_ceil(8));
        glyphs.push(Glyph {
            ch: char::from_u32(u32::from(code)).expect("valid article glyph"),
            advance: i32::from(header[2]),
            ink_x: i32::from(header[3] as i8),
            ink_w,
            ink: (0..pixels).map(|bit| bitmap[bit / 8] & (0x80 >> (bit % 8)) != 0).collect(),
        });
    }
    Font::new(face.into(), bold, height, ascent, glyphs).expect("valid Help font atlas")
}
