//! What a panic does in a Bevel95 program (the safety review of 2026-11-18, suspected 2).
//!
//! A panic on the program's main thread ends the program (inside a window procedure Rust aborts
//! the process at once; elsewhere it unwinds out of `main`). A panic on a worker thread ended
//! only that thread: the program's windows stayed, looking alive while the work they waited for
//! was gone (a folder that never fills, a Help viewer that no longer takes its caller's
//! requests), and the bar, whose hook had already given the screen back, kept running without
//! its reserved strip. So after any panic the hook logs it, runs the program's own `after` (the
//! bar gives the screen back and asks its window to close) and, for a worker thread, closes the
//! program's windows as if the person had closed them, so the program ends the ordinary way: its
//! settings saved by its usual close, and a copy or move of files still running finishing first
//! ("File jobs outlive their window"). `happened` tells the rest of the program not to undo that
//! (the bar never hides Windows' taskbar again).

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use windows_sys::Win32::Foundation::{HWND, LPARAM};
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::WindowsAndMessaging::{EnumThreadWindows, GetClassNameW, IsWindowVisible, PostMessageW, WM_CLOSE};

use crate::log::log;

/// The thread that installed the hook: the program's main (window) thread.
static MAIN: AtomicU32 = AtomicU32::new(0);
static HAPPENED: AtomicBool = AtomicBool::new(false);

/// Installs the panic hook, on the program's main thread. `after` runs on the panicking thread
/// once the panic is logged (told whether that is the main thread): it may only post messages
/// and give back what must be given back at once; it must not wait for the main thread.
pub fn install(after: fn(on_main_thread: bool)) {
    // SAFETY: a plain query.
    MAIN.store(unsafe { GetCurrentThreadId() }, Ordering::SeqCst);
    std::panic::set_hook(Box::new(move |info| {
        log(&format!("PANIC: {info}"));
        HAPPENED.store(true, Ordering::SeqCst);
        // SAFETY: a plain query.
        let on_main = unsafe { GetCurrentThreadId() } == MAIN.load(Ordering::SeqCst);
        if !on_main {
            log("the panic was on a worker thread: the program's windows are closed and the program ends");
        }
        after(on_main);
    }));
}

/// Whether a panic happened in this program (the hook has run).
pub fn happened() -> bool {
    HAPPENED.load(Ordering::SeqCst)
}

/// The usual `after` of a program: a panic on a worker thread closes the program's windows.
pub fn close_windows_after(on_main_thread: bool) {
    if !on_main_thread {
        close_windows();
    }
}

/// Asks every visible window of Bevel95's own class (`kit::KIT_CLASS`) on the main thread to
/// close, as a click on its Close button does (posted: never waits for that thread). Windows'
/// own windows the program shows (a copy's progress window, Properties) are left alone.
pub fn close_windows() {
    unsafe extern "system" fn ask(hwnd: HWND, _: LPARAM) -> i32 {
        let mut class = [0u16; 64];
        // SAFETY: a window EnumThreadWindows supplied and a buffer of the size given.
        unsafe {
            let n = GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32).max(0) as usize;
            if IsWindowVisible(hwnd) != 0 && String::from_utf16_lossy(&class[..n]) == crate::kit::KIT_CLASS {
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
            }
        }
        1
    }
    // SAFETY: the enumeration is confined to this program's own main thread.
    unsafe { EnumThreadWindows(MAIN.load(Ordering::SeqCst), Some(ask), 0) };
}
