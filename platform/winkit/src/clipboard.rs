//! The person's clipboard, plain text only (CF_UNICODETEXT), for Bevel95's text fields (brief 95).
//!
//! The clipboard is the person's own: Bevel95 writes it only when the person presses Cut or Copy,
//! and its text is never written to the log, a test's output or anywhere else; a failed call is
//! logged by its name only (and counted). Opening it can fail while another program holds it: a
//! few short tries, then it gives up quietly, never a hang and never a panic.
//!
//! **No test reaches the real clipboard**: in a build for tests (this crate's own, or any crate's
//! with the feature `test-scratch`, which every crate using the kit adds for its tests) every
//! call answers "nothing there" / "not done" without touching Windows' clipboard
//! (`tests::tests_never_reach_the_real_clipboard`). Tests use `bevel_core::edit::Memory`.

use std::ptr::null_mut;
use std::sync::atomic::AtomicU32;

use windows_sys::Win32::Foundation::{GlobalFree, HWND};
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard, SetClipboardData,
};
use windows_sys::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows_sys::Win32::System::Ole::CF_UNICODETEXT;

use crate::log::counted;

/// A build for tests never touches the real clipboard.
const IN_TESTS: bool = cfg!(any(test, feature = "test-scratch"));

/// How often, and how long apart, opening the clipboard is tried while another program holds it.
const TRIES: u32 = 5;
const PAUSE_MS: u64 = 10;

static OPEN_FAILED: AtomicU32 = AtomicU32::new(0);
static SET_FAILED: AtomicU32 = AtomicU32::new(0);

/// Opens the clipboard for `owner` (null: no window), a few short tries. False: given up (logged
/// by name, counted).
fn open(owner: HWND) -> bool {
    for i in 0..TRIES {
        // SAFETY: a plain call; closed by the caller when it succeeded.
        if unsafe { OpenClipboard(owner) } != 0 {
            return true;
        }
        if i + 1 < TRIES {
            std::thread::sleep(std::time::Duration::from_millis(PAUSE_MS));
        }
    }
    counted(&OPEN_FAILED, "OpenClipboard failed (another program held the clipboard)");
    false
}

/// The clipboard's plain text, if it holds any.
pub fn text() -> Option<String> {
    if IN_TESTS || !has_text() || !open(null_mut()) {
        return None;
    }
    // SAFETY: the clipboard is open; the locked memory is read as the null-terminated UTF-16 text
    // Windows documents for CF_UNICODETEXT, and unlocked and closed here.
    unsafe {
        let handle = GetClipboardData(u32::from(CF_UNICODETEXT));
        let text = if handle.is_null() {
            None
        } else {
            let p = GlobalLock(handle) as *const u16;
            let text = (!p.is_null()).then(|| {
                let len = (0..).take_while(|&i| *p.add(i) != 0).count();
                String::from_utf16_lossy(std::slice::from_raw_parts(p, len))
            });
            GlobalUnlock(handle);
            text
        };
        CloseClipboard();
        text
    }
}

/// Whether the clipboard holds plain text (a field's menu greys Paste when not).
pub fn has_text() -> bool {
    // SAFETY: a plain query.
    !IN_TESTS && unsafe { IsClipboardFormatAvailable(u32::from(CF_UNICODETEXT)) } != 0
}

/// Puts `text` on the clipboard for window `owner` (Cut, Copy). False: not done (logged by name).
pub fn set_text(owner: HWND, text: &str) -> bool {
    if IN_TESTS || !open(owner) {
        return false;
    }
    let units: Vec<u16> = text.encode_utf16().chain([0]).collect();
    // SAFETY: the clipboard is open; the memory handed to it is a moveable global block holding
    // the null-terminated text, which the clipboard owns once set (freed here if it was not taken).
    let done = unsafe {
        EmptyClipboard();
        let block = GlobalAlloc(GMEM_MOVEABLE, units.len() * 2);
        let mut done = false;
        if !block.is_null() {
            let p = GlobalLock(block) as *mut u16;
            if p.is_null() {
                GlobalFree(block);
            } else {
                std::ptr::copy_nonoverlapping(units.as_ptr(), p, units.len());
                GlobalUnlock(block);
                if SetClipboardData(u32::from(CF_UNICODETEXT), block).is_null() {
                    GlobalFree(block);
                } else {
                    done = true;
                }
            }
        }
        CloseClipboard();
        done
    };
    if !done {
        counted(&SET_FAILED, "SetClipboardData failed");
    }
    done
}

/// The real clipboard as `bevel_core::edit::Clipboard`, for window `owner`.
pub struct System(pub HWND);

impl bevel_core::edit::Clipboard for System {
    fn text(&mut self) -> Option<String> {
        text()
    }
    fn set_text(&mut self, text: &str) -> bool {
        set_text(self.0, text)
    }
    fn has_text(&mut self) -> bool {
        has_text()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The guard of brief 95: a build for tests answers without touching Windows' clipboard, so
    /// no test under `cargo test` can read or overwrite the person's clipboard.
    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn tests_never_reach_the_real_clipboard() {
        assert!(IN_TESTS);
        assert!(!set_text(null_mut(), "a test's words"));
        assert_eq!(text(), None);
        assert!(!has_text());
    }
}
