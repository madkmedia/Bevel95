//! The letters every program draws with: stored as pixels in the program (`bevel_core::text::stored`,
//! brief 89, 0.1.5), so that making a look never asks Windows for a font and cannot fail for want of
//! one. How they were made, and the check that they equal what Windows draws on this PC, are the
//! tests' (`gdi`).

use bevel_core::text::Font;
use windows_sys::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT, DPI_AWARENESS_CONTEXT_UNAWARE, SetThreadDpiAwarenessContext};

#[cfg(test)]
mod gdi;

/// The stored letters of `face` at `px` pixels per em, regular or bold (every alphabet Bevel95 has
/// letters for). Nothing of the PC's is asked, so nothing can fail: every face, size and weight
/// the programs ask for is stored (`gdi`'s test `the_stored_letters_are_what_windows_draws` fails
/// otherwise), and one that were not, a mistake in the program, would get MS Sans Serif 8pt of
/// the same weight rather than no window.
pub fn letters(face: &str, px: i32, bold: bool) -> Font {
    bevel_core::text::stored(face, px, bold)
        .or_else(|| bevel_core::text::stored("MS Sans Serif", 11, bold))
        .expect("the stored MS Sans Serif letters (proved by the tests)")
}

/// While alive, GDI on this thread behaves as if the screen were 96 DPI, so font
/// selection picks the 96-DPI raster font file (SSERIFE.FON) even on a high-DPI monitor.
pub struct DpiUnaware(DPI_AWARENESS_CONTEXT);

impl DpiUnaware {
    pub fn enter() -> DpiUnaware {
        // SAFETY: plain Win32 call with a predefined context constant.
        DpiUnaware(unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_UNAWARE) })
    }
}

impl Drop for DpiUnaware {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: restores the context returned by the earlier call.
            unsafe { SetThreadDpiAwarenessContext(self.0) };
        }
    }
}

