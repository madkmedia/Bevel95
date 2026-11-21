//! The bar's windows on the control kit: the shared window host (`bevel_winkit::kit`) with what
//! the bar paints with. Each window's state lives in the host's list, not in the bar's `App`;
//! the look, the fonts and the size are copied from the bar when a window opens (`retheme`
//! after a look change), so painting never waits for the bar.

pub use bevel_winkit::kit::*;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos};

use crate::bar::with_app;

/// What the bar paints with now.
fn bar_look() -> Option<Look> {
    with_app(|app| {
        let fonts = app.fonts();
        Look { theme: app.theme, regular: fonts.regular.clone(), bold: fonts.bold.clone(), scale: app.scale }
    })
}

/// Opens a window of kind `H` with what the bar paints with; `place` gets its size in screen
/// pixels and says where it goes. An open one of the same kind is closed first.
pub fn open<H: Hosted>(hosted: H, place: impl FnOnce(i32, i32) -> (i32, i32)) -> Option<HWND> {
    open_as(Kind::Tool, hosted, bar_look()?, place)
}

/// Opens a window of kind `H` as a dialog of `owner`, one of the bar's own windows: it stays
/// above its owner, which cannot be used until the dialog is closed, and like its owner above
/// every other window.
pub fn open_dialog<H: Hosted>(owner: HWND, hosted: H, place: impl FnOnce(i32, i32) -> (i32, i32)) -> Option<HWND> {
    let hwnd = open_as(Kind::Dialog { owner }, hosted, bar_look()?, place)?;
    // SAFETY: our own window, just made; only its place among the windows changes.
    let raised = unsafe { SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) };
    if raised == 0 {
        crate::log::log("a dialog could not be put above every window (SetWindowPos failed)");
    }
    Some(hwnd)
}

/// After the bar's look or size changed: the open window of kind `H` paints with the new fonts
/// and size, laid out again; `place` says where it goes now (given its size in screen pixels).
pub fn retheme<H: Hosted>(place: impl FnOnce(i32, i32) -> (i32, i32)) {
    if let Some(look) = bar_look() {
        retheme_with::<H>(look, |w, h, _| place(w, h));
    }
}
