//! Reserving the bottom of the screen the official way: an "appbar" (SHAppBarMessage), so
//! maximized windows stop above the bar, as they do with the real taskbar.

use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::UI::Shell::{
    ABE_BOTTOM, ABM_NEW, ABM_QUERYPOS, ABM_REMOVE, ABM_SETPOS, ABM_WINDOWPOSCHANGED, APPBARDATA, SHAppBarMessage,
};

fn data(hwnd: HWND) -> APPBARDATA {
    APPBARDATA { cbSize: size_of::<APPBARDATA>() as u32, hWnd: hwnd, ..Default::default() }
}

/// Registers `hwnd` as an appbar; the shell will send it `callback` messages. False if it
/// was already registered.
pub fn register(hwnd: HWND, callback: u32) -> bool {
    let mut abd = data(hwnd);
    abd.uCallbackMessage = callback;
    // SAFETY: a correctly sized APPBARDATA.
    unsafe { SHAppBarMessage(ABM_NEW, &mut abd) != 0 }
}

/// Gives the reserved space back. Harmless if `hwnd` is not registered, or no longer exists
/// (that is how the watchdog cleans up after a killed bar).
pub fn remove(hwnd: HWND) {
    let mut abd = data(hwnd);
    // SAFETY: a correctly sized APPBARDATA.
    unsafe { SHAppBarMessage(ABM_REMOVE, &mut abd) };
}

/// Reserves a strip `height` pixels tall along the bottom of `monitor` and returns where the
/// window should go. The shell may move the strip up (e.g. above another appbar) but only
/// fixes its bottom edge, so the top is recomputed from it, as ManagedShell does.
pub fn place(hwnd: HWND, monitor: RECT, height: i32) -> RECT {
    let mut abd = data(hwnd);
    abd.uEdge = ABE_BOTTOM;
    abd.rc = RECT { left: monitor.left, top: monitor.bottom - height, right: monitor.right, bottom: monitor.bottom };
    // SAFETY: a correctly sized APPBARDATA.
    unsafe { SHAppBarMessage(ABM_QUERYPOS, &mut abd) };
    abd.rc.top = abd.rc.bottom - height;
    // SAFETY: as above.
    unsafe { SHAppBarMessage(ABM_SETPOS, &mut abd) };
    abd.rc
}

/// Appbars tell the shell when they have moved (WM_WINDOWPOSCHANGED), per the docs.
pub fn window_pos_changed(hwnd: HWND) {
    let mut abd = data(hwnd);
    // SAFETY: a correctly sized APPBARDATA.
    unsafe { SHAppBarMessage(ABM_WINDOWPOSCHANGED, &mut abd) };
}
