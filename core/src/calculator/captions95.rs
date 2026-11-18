//! Windows 95 Calculator's Standard captions, cut from the Reference PC's True Color capture
//! (win95-truecolor-calc-std-fresh): its bold 10-point face is not today's MS Sans Serif (C and E
//! are narrower), so the captions are its pixels. In `form::BUTTONS` order: where the ink starts
//! in the button (up), and the ink, "#" in the caption's color.
//! The captions are Windows' pixels, kept in bevel-art.

use crate::art::{self, Look};

/// (x, y, rows) of each caption.
pub(super) const CAPTIONS: &[(i32, i32, &[&str])] = art::captions("calculator/captions-standard", Look::Win95);
pub(super) const CAPTIONS_SCIENTIFIC: &[(i32, i32, &[&str])] = art::captions("calculator/captions-scientific", Look::Win95);
