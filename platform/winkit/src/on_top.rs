//! The one window that must stay above every other window of its process: the bar's Monitor
//! Lines (brief 102, `platform/win/src/monitor_lines.rs`). The kit's windows, a program's menus
//! and the dragged picture call `shown` when they come up or move forward among the windows; the
//! bar sets the hook once at its start, and in every other program nothing is set and `shown`
//! does nothing.

use std::sync::OnceLock;

use windows_sys::Win32::UI::WindowsAndMessaging::{SWP_NOZORDER, WINDOWPOS};

static HOOK: OnceLock<fn()> = OnceLock::new();

/// What puts that window back on top (the bar's `monitor_lines::raise`). Set once; a second call
/// changes nothing.
pub fn set(raise: fn()) {
    let _ = HOOK.set(raise);
}

/// One of this process's windows came up or forward: the window that must stay on top goes back
/// above it at once.
pub fn shown() {
    if let Some(raise) = HOOK.get() {
        raise();
    }
}

/// A window of the process got `WM_WINDOWPOSCHANGED` (`lparam` its WINDOWPOS): `shown` if its place
/// among the windows changed (a click bringing it forward, say).
pub fn pos_changed(lparam: isize) {
    if HOOK.get().is_none() || lparam == 0 {
        return;
    }
    // SAFETY: Windows passes a WINDOWPOS with WM_WINDOWPOSCHANGED, valid during the message.
    let flags = unsafe { (*(lparam as *const WINDOWPOS)).flags };
    if flags & SWP_NOZORDER == 0 {
        shown();
    }
}
