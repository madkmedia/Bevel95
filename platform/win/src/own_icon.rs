//! Bevel95's own icon in Windows' notification area, shown only while Bevel95 is switched to
//! Windows' taskbar (what it says and does: `bevel_core::tray_icon`).
//!
//! While Bevel95's bar shows, its stand-in "Shell_TrayWnd" (`tray.rs`) is the window
//! Shell_NotifyIcon finds, and the icon would land in Bevel95's own notification area. So the
//! icon is added only after that window is gone (`tray::stop`), when Shell_NotifyIcon finds
//! Windows' taskbar, and it is taken away before the stand-in comes back (`tray::start`): it is
//! never in Bevel95's area, not even for a moment at the switch.

use std::ptr::null;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use bevel_core::tray_icon::{Event, TOOLTIP};
use windows_sys::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::GetSystemMetricsForDpi;
use windows_sys::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NIM_SETVERSION, NIN_SELECT,
    NOTIFYICON_VERSION_4, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DestroyIcon, HICON, IMAGE_ICON, LR_DEFAULTCOLOR, LoadImageW, SM_CXSMICON, SM_CYSMICON, WM_CONTEXTMENU,
    WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_RBUTTONDOWN, WM_RBUTTONUP,
};

use crate::log::log;

/// The message Windows' taskbar sends the bar about its icon (version 4: the pointer in wParam,
/// the event and the icon's number in lParam).
/// Registered by name (Bevel95.Bar.OwnIcon, the fourth safety review of 2026-11-18, C3: a stray
/// broadcast of WM_APP + 41 could open the icon's menu).
pub static WM_OWN_ICON: crate::util::Private = crate::util::Private::new("Bevel95.Bar.OwnIcon");
/// The icon's number among the bar window's icons (it has only this one).
const UID: u32 = 1;
/// Enter or Space on the icon: NIN_SELECT | NINF_KEY (shellapi.h; not in windows-sys).
const NIN_KEYSELECT: u32 = NIN_SELECT | 1;
/// The program's icon group in bevel95.exe (`tools/build/program_icon.rs`).
const ICON_RESOURCE: usize = 1;

/// The icon belongs in Windows' notification area now (from `show` to `hide`, also while
/// Windows' taskbar did not take it: a new Explorer is asked again).
static WANTED: AtomicBool = AtomicBool::new(false);
/// The picture handed to Windows' taskbar (0: none loaded).
static PICTURE: AtomicUsize = AtomicUsize::new(0);

/// The icon's description for Shell_NotifyIcon (`picture`: with NIF_ICON).
fn data(bar: HWND, picture: HICON) -> NOTIFYICONDATAW {
    let mut tip = [0u16; 128];
    for (slot, unit) in tip.iter_mut().zip(TOOLTIP.encode_utf16().take(127)) {
        *slot = unit;
    }
    let mut d = NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: bar,
        uID: UID,
        uFlags: NIF_MESSAGE | NIF_TIP | NIF_SHOWTIP,
        uCallbackMessage: WM_OWN_ICON.id(),
        szTip: tip,
        ..Default::default()
    };
    if !picture.is_null() {
        d.uFlags |= NIF_ICON;
        d.hIcon = picture;
    }
    d
}

/// Bevel95's program icon at Windows' small icon size for `dpi` (loaded once per showing).
fn picture(dpi: u32) -> HICON {
    let loaded = PICTURE.load(Ordering::SeqCst);
    if loaded != 0 {
        return loaded as HICON;
    }
    // SAFETY: plain Win32 calls; the resource is a numeric id in our own executable.
    let icon = unsafe {
        let (w, h) = (GetSystemMetricsForDpi(SM_CXSMICON, dpi), GetSystemMetricsForDpi(SM_CYSMICON, dpi));
        LoadImageW(GetModuleHandleW(null()), ICON_RESOURCE as *const u16, IMAGE_ICON, w.max(16), h.max(16), LR_DEFAULTCOLOR)
    };
    if icon.is_null() {
        crate::log::log_win32("LoadImageW (Bevel95's notification icon: shown without a picture)");
        return std::ptr::null_mut();
    }
    PICTURE.store(icon as usize, Ordering::SeqCst);
    icon as HICON
}

/// Shows the icon in Windows' notification area (`dpi`: the main screen's). Called once
/// Bevel95's stand-in window is gone; also again when Explorer restarts ("TaskbarCreated"):
/// then it adds the icon anew, or, if Windows' taskbar still has it, brings it up to date.
pub fn show(bar: HWND, dpi: u32) {
    WANTED.store(true, Ordering::SeqCst);
    let d = data(bar, picture(dpi));
    // SAFETY: a fully initialized NOTIFYICONDATAW for our own window.
    let added = unsafe { Shell_NotifyIconW(NIM_ADD, &d) } != 0;
    // SAFETY: as above.
    let ok = added || unsafe { Shell_NotifyIconW(NIM_MODIFY, &d) } != 0;
    if !ok {
        log("own icon: Windows' taskbar did not take Bevel95's notification icon (Shell_NotifyIcon add and modify failed)");
        return;
    }
    let mut v = data(bar, std::ptr::null_mut());
    v.Anonymous.uVersion = NOTIFYICON_VERSION_4;
    // SAFETY: as above.
    if unsafe { Shell_NotifyIconW(NIM_SETVERSION, &v) } == 0 {
        log("own icon: Shell_NotifyIcon(NIM_SETVERSION 4) failed; its clicks may arrive in the old form");
    }
    log(if added { "own icon: in Windows' notification area" } else { "own icon: was in Windows' notification area already; brought up to date" });
}

/// Takes the icon away (before Bevel95's bar and its stand-in come back, and when it ends).
pub fn hide(bar: HWND) {
    if WANTED.swap(false, Ordering::SeqCst) {
        let d = data(bar, std::ptr::null_mut());
        // SAFETY: as in `show`.
        if unsafe { Shell_NotifyIconW(NIM_DELETE, &d) } == 0 {
            log("own icon: Shell_NotifyIcon(NIM_DELETE) failed (it was not there, or Windows' taskbar did not answer)");
        } else {
            log("own icon: taken away");
        }
    }
    let picture = PICTURE.swap(0, Ordering::SeqCst);
    // SAFETY: an icon we loaded; Windows' taskbar keeps its own copy.
    if picture != 0 && unsafe { DestroyIcon(picture as HICON) } == 0 {
        crate::log::log_win32("DestroyIcon (Bevel95's notification icon)");
    }
}

/// Whether the icon belongs in Windows' notification area (Bevel95 is switched to it).
pub fn wanted() -> bool {
    WANTED.load(Ordering::SeqCst)
}

/// What Windows' taskbar reported about the icon (WM_OWN_ICON's wParam and lParam), with the
/// pointer's place (screen pixels) for a menu.
pub fn event_of(wparam: WPARAM, lparam: LPARAM) -> (Event, i32, i32) {
    let event = match (lparam as u32) & 0xFFFF {
        WM_LBUTTONDOWN | WM_LBUTTONUP | WM_RBUTTONDOWN | WM_RBUTTONUP => Event::Press,
        NIN_SELECT => Event::Select,
        NIN_KEYSELECT => Event::KeySelect,
        WM_LBUTTONDBLCLK => Event::DoubleClick,
        WM_CONTEXTMENU => Event::ContextMenu,
        _ => Event::Other,
    };
    let (x, y) = ((wparam & 0xFFFF) as u16 as i16 as i32, ((wparam >> 16) & 0xFFFF) as u16 as i16 as i32);
    (event, x, y)
}
