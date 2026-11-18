//! Only a running file job keeps a Bevel95 program alive (the second safety review of
//! 2026-11-18, B1): a dialog of Windows' left open in the program (a Properties window, a message)
//! does not. Before, every visible window of the classes `#32770` and `OperationStatusWindow` of
//! the program counted, so a Properties window left open kept the bar's process, and its place,
//! for good. Tried in this test's own process, where no file job runs.

use std::ptr::null;

use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, IsWindowVisible, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP, WS_VISIBLE,
};

#[test]
fn a_dialog_of_windows_left_open_does_not_keep_the_program() {
    let mut windows = Vec::new();
    for class in ["#32770", "OperationStatusWindow"] {
        let name: Vec<u16> = class.encode_utf16().chain([0]).collect();
        // SAFETY: a system class (the dialog class) or an unregistered one (then null, skipped);
        // a 1x1 tool window far off the screen, never activated.
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                name.as_ptr(),
                null(),
                WS_POPUP | WS_VISIBLE,
                -32000,
                -32000,
                1,
                1,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                GetModuleHandleW(null()),
                null(),
            )
        };
        if !hwnd.is_null() {
            windows.push(hwnd);
        }
    }
    assert!(!windows.is_empty(), "the dialog class always exists");
    // SAFETY: IsWindowVisible takes any handle.
    assert!(windows.iter().all(|&w| unsafe { IsWindowVisible(w) } != 0));
    assert_eq!(bevel_winkit::jobs::running(), 0);
    assert!(!bevel_winkit::jobs::busy(), "a dialog of Windows' does not keep the program");
    for w in windows {
        // SAFETY: this thread's own window.
        unsafe { DestroyWindow(w) };
    }
}
