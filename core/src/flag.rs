//! The Start button flag, as an embedded 1× pixel image.
//!
//! Windows' own Start button flag (16×16, each pixel drawn or see-through), the same in Windows 95,
//! 98 and 2000. First taken from another taskbar's copy of it; since 2026-11-18 cut from the
//! Reference PCs' Start buttons and found identical, pixel for pixel, in every True Color capture
//! of each (`vm/<pc>-truecolor-calc-about.png` and 387 to 399 more; docs/decisions.md, "Every
//! picture from the owner's own Windows").
//!
//! The pictures themselves are Windows' and kept in bevel-art (`art/src/start.rs`); this
//! module chooses and draws them.

use crate::fb::Icon;
use crate::theme::rgb;

const ART: &[&str] = crate::art::rows("start-button/flag", crate::art::ANY).rows;

pub fn start_flag() -> Icon {
    let pixels = ART
        .iter()
        .flat_map(|row| row.bytes())
        .map(|b| match b {
            b'K' => rgb(0x000000),
            b'R' => rgb(0xFF0000),
            b'G' => rgb(0x00FF00),
            b'B' => rgb(0x0000FF),
            b'Y' => rgb(0xFFFF00),
            _ => 0x0000_0000,
        })
        .collect();
    Icon { width: 16, height: 16, pixels }
}

#[cfg(test)]
mod tests {
    #[test]
    fn flag_is_16_by_16() {
        let f = super::start_flag();
        assert_eq!(f.pixels.len(), 16 * 16);
        assert!(super::ART.iter().all(|r| r.len() == 16));
    }
}
