//! Calculator's picture: the 16×16 and 32×32 16-color icons of CALC.EXE, identical in
//! Windows 95, 98 and 2000 (the same bytes in all three files).
//!
//! The pictures themselves are Windows' and kept in bevel-art (`art/assets/calculator`); this
//! module chooses and draws them.

use crate::fb::Icon;
use crate::theme::Era;

/// The title bar and taskbar (16) or program (32) icon.
pub fn icon(_era: Era, size: i32) -> Icon {
    const SMALL_BYTES: usize = 16 * 16 * 4;
    const DATA: &[u8; SMALL_BYTES + 32 * 32 * 4] = crate::art::exactly("calculator/icons", crate::art::Look::Win95);
    let data = DATA;
    let (width, rgba) = if size == 32 { (32, &data[SMALL_BYTES..]) } else { (16, &data[..SMALL_BYTES]) };
    let pixels = rgba.as_chunks::<4>().0.iter().map(|&[r, g, b, a]| u32::from_be_bytes([a, r, g, b])).collect();
    Icon { width, height: width, pixels }
}
