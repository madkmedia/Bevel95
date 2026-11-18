//! Fullscreen apps (a video, a game, a slide show): the bar steps behind them, like the real
//! taskbar. The rules are RetroBar's (ManagedShell's FullScreenHelper): a window counts if
//! Windows reported it as fullscreen, or if it fills the main screen (for a window with a
//! frame, its inside must), and it is not one of Windows' own see-through or full-screen
//! helpers. One rule is Bevel95's own: a maximized window with a title bar never counts. With
//! Auto hide the bar reserves no space, so maximized windows cover the whole screen, and
//! browsers that draw their own title bar fill it with their inside too.

use std::collections::HashSet;
use std::ptr::null_mut;

use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTOPRIMARY, MapWindowPoints, MonitorFromPoint, MonitorFromWindow,
};
use windows_sys::Win32::System::Threading::GetCurrentProcessId;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GWL_STYLE, GetClientRect, GetForegroundWindow, GetPropW, GetWindowLongPtrW, GetWindowRect,
    GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible, IsZoomed, WS_BORDER, WS_CAPTION, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_EX_WINDOWEDGE, WS_POPUP, WS_THICKFRAME,
};

use crate::util::wide;
use crate::window_list::{class_name, is_cloaked};

/// Shell hook codes Windows sends when a window enters or leaves fullscreen (also when a
/// program calls ITaskbarList2::MarkFullscreenWindow). Not in windows-sys or WinUser.h; the
/// values are ManagedShell's.
pub const HSHELL_FULLSCREENENTER: u32 = 53;
pub const HSHELL_FULLSCREENEXIT: u32 = 54;

#[derive(Default)]
pub struct Fullscreen {
    /// The app the bar stepped behind.
    behind: Option<usize>,
    /// Windows that Windows reported as fullscreen and that have not left it yet.
    reported: HashSet<usize>,
}

impl Fullscreen {
    /// Whether the bar is behind a fullscreen app now.
    pub fn is_behind(&self) -> bool {
        self.behind.is_some()
    }

    /// The shell hook said `window` entered fullscreen (`entered`) or left it.
    pub fn reported(&mut self, window: HWND, entered: bool) {
        if entered {
            self.reported.insert(window as usize);
        } else {
            self.reported.remove(&(window as usize));
        }
    }

    /// Works out which app, if any, the bar should be behind now (`screen` is the main
    /// screen's rectangle); true if that changed.
    pub fn update(&mut self, screen: &RECT) -> bool {
        // SAFETY: read-only queries; all accept any window handle, even one that has gone.
        let now = unsafe {
            self.reported.retain(|&w| IsWindow(w as HWND) != 0);
            let front = GetForegroundWindow();
            if self.counts(front, screen) {
                Some(front as usize)
            } else {
                // A fullscreen app stays so while the window in front is on another screen, or
                // stays above every window (a volume pop-up, an overlay).
                let elsewhere = front.is_null()
                    || !on_main_screen(front)
                    || GetWindowLongPtrW(front, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST != 0;
                self.behind.filter(|&w| elsewhere && self.counts(w as HWND, screen))
            }
        };
        let changed = now != self.behind;
        self.behind = now;
        changed
    }

    /// Whether `window` is a fullscreen app on the main screen.
    ///
    /// # Safety
    /// Only read-only queries; any window handle.
    unsafe fn counts(&self, window: HWND, screen: &RECT) -> bool {
        // SAFETY: as above.
        unsafe {
            can_be_fullscreen(window)
                && on_main_screen(window)
                && (self.reported.contains(&(window as usize)) || fills(window, screen))
        }
    }
}

/// # Safety
/// Only read-only queries; any window handle.
unsafe fn on_main_screen(window: HWND) -> bool {
    // SAFETY: as above.
    unsafe {
        MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST)
            == MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY)
    }
}

/// Not hidden, minimized, maximized with a title bar, or ours, and not the desktop, a taskbar,
/// or one of Windows' own full-screen helpers (Start, search) or see-through overlays (game
/// overlays, recorders).
///
/// # Safety
/// Only read-only queries; any window handle.
unsafe fn can_be_fullscreen(window: HWND) -> bool {
    // SAFETY: as above; the property name is null-terminated.
    unsafe {
        if window.is_null() || IsWindowVisible(window) == 0 || IsIconic(window) != 0 {
            return false;
        }
        let mut process = 0;
        GetWindowThreadProcessId(window, &mut process);
        // The bar, its menus and its windows; the desktop marks itself (Windows 8 and later).
        if process == GetCurrentProcessId() || !GetPropW(window, wide("NonRudeHWND").as_ptr()).is_null() {
            return false;
        }
        // A maximized window with a title bar never counts, even when Windows reports it.
        let style = GetWindowLongPtrW(window, GWL_STYLE) as u32;
        if is_cloaked(window) || (IsZoomed(window) != 0 && style & WS_CAPTION == WS_CAPTION) {
            return false;
        }
        let ex = GetWindowLongPtrW(window, GWL_EXSTYLE) as u32;
        let helper = match class_name(window).as_str() {
            "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd" => true,
            // An app's frame has the edge style; Windows' own panels do not.
            "ApplicationFrameWindow"
            | "Windows.UI.Core.CoreWindow"
            | "StartMenuSizingFrame"
            | "Shell_LightDismissOverlay" => ex & WS_EX_WINDOWEDGE == 0,
            _ => false,
        };
        let overlay = ex & WS_EX_LAYERED != 0 && ex & (WS_EX_TRANSPARENT | WS_EX_NOACTIVATE) != 0;
        !helper && !overlay
    }
}

/// Whether `window` covers the whole of `screen`: a window with a frame by its inside (so a
/// maximized window's title bar keeps it from counting), any other by its outline.
///
/// # Safety
/// Only read-only queries; any window handle.
unsafe fn fills(window: HWND, screen: &RECT) -> bool {
    // SAFETY: as above; `r` is a RECT, which is two POINTs.
    unsafe {
        let style = GetWindowLongPtrW(window, GWL_STYLE) as u32;
        let has = |bits: u32| style & bits == bits;
        let mut r = RECT::default();
        if has(WS_CAPTION | WS_THICKFRAME) || has(WS_POPUP | WS_THICKFRAME) || has(WS_POPUP | WS_BORDER) {
            GetClientRect(window, &mut r);
            MapWindowPoints(window, null_mut(), (&mut r as *mut RECT).cast::<POINT>(), 2);
        } else {
            GetWindowRect(window, &mut r);
        }
        r.left <= screen.left && r.top <= screen.top && r.right >= screen.right && r.bottom >= screen.bottom
    }
}
