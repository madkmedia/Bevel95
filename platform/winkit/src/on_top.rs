//! The one window that must stay above every other window of Bevel95's own: the bar's Monitor
//! Lines (briefs 102 and 104, `platform/win/src/monitor_lines.rs`). The kit's windows, a program's
//! menus and the dragged picture call `shown` when they come up or move forward among the windows.
//! In the bar the hook it sets at its start puts the lines back (`set`). In every other program of
//! Bevel95 (folders.exe, the games, the Calculator, the Help viewer) `shown` asks the bar of its
//! own copy to do it, by a registered message, while the lines show: so those programs' menus
//! and dragged pictures open under the lines at once too. The bar answers that message only by
//! putting the lines where they belong.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use windows_sys::Win32::Foundation::{CloseHandle, HWND};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetWindowThreadProcessId, IsWindow, PostMessageW, SWP_NOZORDER, WINDOWPOS,
};

use crate::util::{Private, wide};

static HOOK: OnceLock<fn()> = OnceLock::new();

/// The bar's message: put the lines back where they belong (above Bevel95's own windows). It
/// carries nothing and does nothing else, whoever posts it.
pub static WM_RAISE_LINES: Private = Private::new("Bevel95.Bar.RaiseMonitorLines");

/// The lines' window's class and the bar's (one bar on the screen: `copy.rs`).
const LINES_CLASS: &str = "Bevel95MonitorLines";
const BAR_CLASS: &str = "Bevel95Bar";

/// What puts that window back on top (the bar's `monitor_lines::raise`). Set once; a second call
/// changes nothing.
pub fn set(raise: fn()) {
    let _ = HOOK.set(raise);
}

/// One of this process's windows came up or forward: the lines go back above it at once (in the
/// bar), or the bar of this program's own copy is asked to put them back (elsewhere).
pub fn shown() {
    match HOOK.get() {
        Some(raise) => raise(),
        None => tell_bar(),
    }
}

/// A window of the process got `WM_WINDOWPOSCHANGED` (`lparam` its WINDOWPOS): `shown` if its place
/// among the windows changed (a click bringing it forward, say).
pub fn pos_changed(lparam: isize) {
    if lparam == 0 {
        return;
    }
    // SAFETY: Windows passes a WINDOWPOS with WM_WINDOWPOSCHANGED, valid during the message.
    let flags = unsafe { (*(lparam as *const WINDOWPOS)).flags };
    if flags & SWP_NOZORDER == 0 {
        shown();
    }
}

/// The bar found last time and proven to be this program's own copy's (0: none yet).
static OUR_BAR: AtomicUsize = AtomicUsize::new(0);

/// Posts `WM_RAISE_LINES` to the bar of this program's own copy, while the lines show: nothing
/// when no lines' window exists, no bar runs, or the bar running is another copy's.
fn tell_bar() {
    let (lines_class, bar_class) = (wide(LINES_CLASS), wide(BAR_CLASS));
    // SAFETY: plain queries with null-terminated class names; the post carries no pointer.
    unsafe {
        if FindWindowW(lines_class.as_ptr(), std::ptr::null()).is_null() {
            return;
        }
        let bar = FindWindowW(bar_class.as_ptr(), std::ptr::null());
        if bar.is_null() {
            return;
        }
        if OUR_BAR.load(Ordering::SeqCst) != bar as usize || IsWindow(bar) == 0 {
            if !same_copy(bar) {
                return;
            }
            OUR_BAR.store(bar as usize, Ordering::SeqCst);
        }
        PostMessageW(bar, WM_RAISE_LINES.id(), 0, 0);
    }
}

/// Whether `bar`'s program file belongs to this program's own copy (`bevel_core::monitor_lines::
/// copy_folder`: by the files, never by a class name).
fn same_copy(bar: HWND) -> bool {
    let mut pid = 0;
    // SAFETY: a plain query.
    unsafe { GetWindowThreadProcessId(bar, &mut pid) };
    // SAFETY: a plain query.
    if pid == 0 || pid == unsafe { GetCurrentProcessId() } {
        return false;
    }
    let (Some(theirs), Ok(ours)) = (process_path(pid), std::env::current_exe()) else {
        return false;
    };
    use bevel_core::monitor_lines::copy_folder;
    copy_folder(&theirs) == copy_folder(&ours.to_string_lossy())
}

/// The full path of process `pid`'s program file.
pub fn process_path(pid: u32) -> Option<String> {
    // SAFETY: the handle is closed after use; the buffer size is passed in and out.
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return None;
        }
        let mut buf = vec![0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len);
        CloseHandle(process);
        (ok != 0).then(|| String::from_utf16_lossy(&buf[..len as usize]))
    }
}
