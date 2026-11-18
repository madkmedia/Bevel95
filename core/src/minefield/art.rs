//! Minesweeper's artwork at native resolution. The indexed strips are the pixels of
//! WINMINE's bitmap resources 410/411 (field), 420/421 (counters) and 430/431 (face).
//! Windows 95 and 98 have identical strips. Windows 2000 draws new color faces and tiles,
//! but uses the same digits and monochrome artwork as the earlier versions.
//!
//! The pictures themselves are Windows' and kept in bevel-art (`art/assets/minefield`); this
//! module chooses and draws them.

use crate::art::{self, Look};
use crate::fb::{Framebuffer, Icon};
use crate::theme::{Era, rgb};

// Top-down rows, one palette index per pixel. Within each strip the frames retain the
// original bitmap's top-to-bottom order.
const TILES: &[u8; 3 * 16 * 16 * 16] = art::exactly("minesweeper/tiles", Look::Win95);
const FACES: &[u8; 3 * 5 * 24 * 24] = art::exactly("minesweeper/faces", Look::Win95);
const DIGITS: &[u8; 2 * 12 * 13 * 23] = art::exactly("minesweeper/digits", Look::Win95);

#[rustfmt::skip]
const PALETTE: [u32; 17] = [
    rgb(0x000000), rgb(0x800000), rgb(0x008000), rgb(0x808000),
    rgb(0x000080), rgb(0x800080), rgb(0x008080), rgb(0xC0C0C0),
    rgb(0x808080), rgb(0xFF0000), rgb(0x00FF00), rgb(0xFFFF00),
    rgb(0x0000FF), rgb(0xFF00FF), rgb(0x00FFFF), rgb(0xFFFFFF),
    rgb(0xBFBF00),
];

fn variant(era: Era, color: bool) -> usize {
    match (era, color) {
        (_, false) => 2,
        (Era::Win2000, true) => 1,
        (_, true) => 0,
    }
}

fn draw(fb: &mut Framebuffer, x: i32, y: i32, strip: &[u8], index: usize, width: usize, height: usize) {
    let start = index * width * height;
    let frame = &strip[start..start + width * height];
    for (dy, row) in frame.chunks_exact(width).enumerate() {
        for (dx, &color) in row.iter().enumerate() {
            fb.set(x + dx as i32, y + dy as i32, PALETTE[usize::from(color)]);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tile {
    Covered,
    Flagged,
    Marked,
    Tripped,
    WrongFlag,
    Mine,
    MarkedDown,
    Open(u8),
}

pub fn tile(fb: &mut Framebuffer, x: i32, y: i32, era: Era, color: bool, tile: Tile) {
    let frame = match tile {
        Tile::Covered => 0,
        Tile::Flagged => 1,
        Tile::Marked => 2,
        Tile::Tripped => 3,
        Tile::WrongFlag => 4,
        Tile::Mine => 5,
        Tile::MarkedDown => 6,
        Tile::Open(neighbors) => 15 - usize::from(neighbors.min(8)),
    };
    draw(fb, x, y, TILES, variant(era, color) * 16 + frame, 16, 16);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    Down,
    Won,
    Lost,
    Worried,
    Waiting,
}

pub fn face(fb: &mut Framebuffer, x: i32, y: i32, era: Era, color: bool, face: Face) {
    draw(fb, x, y, FACES, variant(era, color) * 5 + face as usize, 24, 24);
}

pub const MINUS: u8 = 10;

/// One 13×23 counter digit, including its background and the unlit segments.
pub fn digit(fb: &mut Framebuffer, x: i32, y: i32, color: bool, value: u8) {
    let frame = match value {
        0..=9 => 11 - usize::from(value),
        MINUS => 0,
        _ => 1, // blank
    };
    draw(fb, x, y, DIGITS, usize::from(!color) * 12 + frame, 13, 23);
}

/// The native title/taskbar (16) or program (32) icon. Windows 2000 supplies just a
/// 32-pixel color icon. Its 16-pixel drawing was cut from the reference PC's title bar,
/// using the inactive caption to identify transparent pixels.
pub fn icon(era: Era, size: i32) -> Icon {
    const SMALL_BYTES: usize = 16 * 16 * 4;
    let data: &[u8; SMALL_BYTES + 32 * 32 * 4] = match era {
        Era::Win95 | Era::Win98 => const { art::exactly("minesweeper/icons", Look::Win95) },
        Era::Win2000 => const { art::exactly("minesweeper/icons", Look::Win2000) },
    };
    let (width, rgba) = if size == 32 { (32, &data[SMALL_BYTES..]) } else { (16, &data[..SMALL_BYTES]) };
    let pixels = rgba.as_chunks::<4>().0.iter().map(|&[r, g, b, a]| u32::from_be_bytes([a, r, g, b])).collect();
    Icon { width, height: width, pixels }
}
