//! Solitaire's native card pictures. The three editions have identical pixels in all 74
//! card and animation bitmaps and both application icons. Windows 95 and 98 also embed the
//! deck in SOL.EXE. Windows 2000 draws from CARDS.DLL.
//!
//! The pictures themselves are Windows' and kept in bevel-art (`art/assets/solitaire`); this
//! module chooses and draws them.

use crate::fb::{Framebuffer, Icon, Rect};
use crate::theme::{Era, rgb};

use super::game::{Card, Suit};

pub const WIDTH: i32 = 71;
pub const HEIGHT: i32 = 96;
pub const BACK_COUNT: usize = 12;

// Ace through ten have just white plus ink. Each row occupies nine bytes, high bit
// first. Court cards, backs, stock markers and animation patches use four bits per pixel,
// top down, high nibble first, without row padding.
const CARD_BYTES: usize = WIDTH as usize * HEIGHT as usize / 2;
const SPOT_BYTES: usize = 9 * HEIGHT as usize;
const SPOTS: &[u8; 40 * SPOT_BYTES] = crate::art::exactly("solitaire/spots", crate::art::Look::Win95);
const COURT: &[u8; 12 * CARD_BYTES] = crate::art::exactly("solitaire/court", crate::art::Look::Win95);
const BACKS: &[u8; BACK_COUNT * CARD_BYTES] = crate::art::exactly("solitaire/backs", crate::art::Look::Win95);
const SLOTS: &[u8; 3 * CARD_BYTES] = crate::art::exactly("solitaire/slots", crate::art::Look::Win95);
const ANIMATIONS: &[u8; 1196] = crate::art::exactly("solitaire/animations", crate::art::Look::Win95);

#[rustfmt::skip]
const COLORS: [u32; 16] = [
    rgb(0x000000), rgb(0x800000), rgb(0x008000), rgb(0x808000),
    rgb(0x000080), rgb(0x800080), rgb(0x008080), rgb(0xC0C0C0),
    rgb(0x808080), rgb(0xFF0000), rgb(0x00FF00), rgb(0xFFFF00),
    rgb(0x0000FF), rgb(0xFF00FF), rgb(0x00FFFF), rgb(0xFFFFFF),
];

fn indexed(data: &[u8], pixel: usize) -> u32 {
    let pair = data[pixel / 2];
    COLORS[usize::from(if pixel.is_multiple_of(2) { pair >> 4 } else { pair & 15 })]
}

/// At native size cdtDraw saves and restores three destination pixels in each corner.
/// A ghost pile also fills the four corners with the table color.
fn draw_card(fb: &mut Framebuffer, x: i32, y: i32, rounded: bool, pixel: impl Fn(usize, usize) -> u32) {
    for dy in 0..HEIGHT {
        let inset = if rounded { (2 - dy.min(HEIGHT - 1 - dy)).max(0) } else { 0 };
        for dx in inset..WIDTH - inset {
            fb.set(x + dx, y + dy, pixel(dx as usize, dy as usize));
        }
    }
}

/// Paint a face at its original size. Highlighting uses CARDS.DLL's inverted-face mode.
pub fn face(fb: &mut Framebuffer, x: i32, y: i32, _era: Era, card: Card, selected: bool) {
    let suit = match card.suit {
        Suit::Clubs => 0,
        Suit::Diamonds => 1,
        Suit::Hearts => 2,
        Suit::Spades => 3,
    };
    let rank = usize::from(card.rank);
    if (11..=13).contains(&rank) {
        let start = (suit * 3 + rank - 11) * CARD_BYTES;
        let invert = if selected { 0x00FF_FFFF } else { 0 };
        draw_card(fb, x, y, true, |dx, dy| indexed(&COURT[start..], dy * WIDTH as usize + dx) ^ invert);
    } else if (1..=10).contains(&rank) {
        let start = (suit * 10 + rank - 1) * SPOT_BYTES;
        let red = card.suit.red();
        // The black pip cards are monochrome GDI bitmaps. During an inverted blit their
        // white bits pick up the DC background (table green) before being inverted.
        let (ink, paper) = match (red, selected) {
            (false, false) => (COLORS[0], COLORS[15]),
            (true, false) => (COLORS[9], COLORS[15]),
            (false, true) => (COLORS[15], rgb(0xFF7FFF)),
            (true, true) => (COLORS[14], COLORS[0]),
        };
        draw_card(fb, x, y, true, |dx, dy| {
            if SPOTS[start + dy * 9 + dx / 8] & (0x80 >> (dx % 8)) == 0 {
                return paper;
            }
            // Red spot-card resources contain a red outline. Normal cdtDraw repaints it black.
            let edge_x = dx.min(WIDTH as usize - 1 - dx);
            let edge_y = dy.min(HEIGHT as usize - 1 - dy);
            if red && !selected && (edge_x == 0 || edge_y == 0 || (edge_x == 1 && edge_y == 1)) {
                COLORS[0]
            } else {
                ink
            }
        });
    }
}

/// The twelve deck backs in resource order (original bitmap IDs 54 through 65).
pub fn back(fb: &mut Framebuffer, x: i32, y: i32, _era: Era, index: usize) {
    if index < BACK_COUNT {
        let data = &BACKS[index * CARD_BYTES..];
        draw_card(fb, x, y, true, |dx, dy| indexed(data, dy * WIDTH as usize + dx));
    }
}

/// The Windows 95 and 98 stretchers group the same source pixels, rounding half an
/// interval up horizontally and down vertically. Windows 98 takes the last sample.
/// Windows 95 and 2000 AND all the samples. NT uses different interval boundaries.
fn samples(era: Era, d: i32, source: i32, dest: i32, horizontal: bool) -> std::ops::Range<i32> {
    if dest >= source {
        let pos = (2 * d + 1) * source / (2 * dest);
        return pos..pos + 1;
    }
    match era {
        Era::Win95 | Era::Win98 => {
            let bias = (dest + i32::from(horizontal)) / 2;
            let start = (d * source + bias) / dest;
            let end = ((d + 1) * source + bias) / dest;
            if era == Era::Win98 { end - 1..end } else { start..end }
        }
        Era::Win2000 => {
            let boundary = |j: i32| (((2 * j - 1) * source).div_euclid(2 * dest) + 1).clamp(0, source);
            boundary(d)..boundary(d + 1)
        }
    }
}

/// The scaled card inside a Deck dialog button, including the bitmap's own corner pixels.
pub fn back_scaled(fb: &mut Framebuffer, rect: Rect, era: Era, index: usize) {
    if (rect.w, rect.h) == (WIDTH, HEIGHT) {
        back(fb, rect.x, rect.y, era, index);
    } else if index < BACK_COUNT {
        let data = &BACKS[index * CARD_BYTES..];
        for y in 0..rect.h {
            let rows = samples(era, y, HEIGHT, rect.h, false);
            for x in 0..rect.w {
                let cols = samples(era, x, WIDTH, rect.w, true);
                let mut color = COLORS[15];
                for sy in rows.clone() {
                    for sx in cols.clone() {
                        color &= indexed(data, (sy * WIDTH + sx) as usize);
                    }
                }
                fb.set(rect.x + x, rect.y + y, color);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Empty,
    Recycle,
    Stop,
}

pub fn slot(fb: &mut Framebuffer, x: i32, y: i32, _era: Era, slot: Slot) {
    let frame = match slot {
        Slot::Empty => 0,
        Slot::Recycle => 1,
        Slot::Stop => 2,
    };
    let data = &SLOTS[frame * CARD_BYTES..];
    draw_card(fb, x, y, slot != Slot::Empty, |dx, dy| {
        let color = indexed(data, dy * WIDTH as usize + dx);
        if slot == Slot::Empty && color == COLORS[15] { COLORS[2] } else { color }
    });
}

/// Animate the top card of the stock after painting its back. One tick is 250 ms.
/// The robot and castle cycle continuously. The beach and sleeve pause between cycles.
pub fn animate_back(fb: &mut Framebuffer, x: i32, y: i32, index: usize, ticks: u32) {
    // Patches are packed in original resource order, 678 through 684.
    let (start, width, height, dx, dy) = match index {
        2 => match ticks % 4 {
            0 | 2 => (1028, 24, 7, 24, 40),
            1 => (1112, 24, 7, 24, 40),
            _ => return,
        },
        9 if ticks.is_multiple_of(2) => (704, 26, 12, 42, 12),
        10 => match (ticks % 200 + 4) % 200 {
            0 | 2 => (860, 14, 12, 47, 1),
            1 => (944, 14, 12, 47, 1),
            _ => return,
        },
        11 => match (ticks % 60 + 4) % 60 {
            0 | 2 => (0, 32, 22, 32, 32),
            1 => (352, 32, 22, 32, 32),
            _ => return,
        },
        _ => return,
    };
    for py in 0..height {
        for px in 0..width {
            fb.set(x + dx + px, y + dy + py, indexed(&ANIMATIONS[start..], (py * width + px) as usize));
        }
    }
}

/// Native 16- and 32-pixel Solitaire icons, with transparency from their original AND masks.
pub fn icon(_era: Era, size: i32) -> Icon {
    const SMALL: usize = 16 * 16 * 4;
    const ICONS: &[u8; SMALL + 32 * 32 * 4] = crate::art::exactly("solitaire/icons", crate::art::Look::Win95);
    let (width, rgba) = if size == 32 { (32, &ICONS[SMALL..]) } else { (16, &ICONS[..SMALL]) };
    let pixels = rgba.as_chunks::<4>().0.iter().map(|&[r, g, b, a]| u32::from_be_bytes([a, r, g, b])).collect();
    Icon { width, height: width, pixels }
}
