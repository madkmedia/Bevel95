//! Small helpers shared by the Win32 modules.

use std::ptr::null_mut;

use bevel_core::fb::Framebuffer;
use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::Graphics::Gdi::{BI_RGB, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, HDC, SetDIBitsToDevice};
use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow};

use crate::log::log;

/// Gives one of our windows the keyboard focus. Windows refuses that to a program that did
/// not get the last input (the Windows key went to the app in front), so if it says no,
/// our input is joined to that app's for a moment, which Windows allows.
pub fn take_focus(hwnd: HWND) {
    if bring_forward(hwnd) == Forward::Refused {
        log("SetForegroundWindow failed: keys and clicking elsewhere may not work in this window");
    }
}

/// How `bring_forward` got a window to the front.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Forward {
    /// SetForegroundWindow did it at once.
    Direct,
    /// Only with our input joined for a moment to that of the window in front.
    Joined,
    /// Windows refused both ways.
    Refused,
}

/// Brings any window to the front: SetForegroundWindow, and if Windows refuses it (it does
/// when another program got the last input, or a Windows panel holds the front), the same
/// again with our input joined to the front window's for one call, which Windows allows.
pub fn bring_forward(hwnd: HWND) -> Forward {
    // SAFETY: plain Win32 calls; the input queues are joined only around one call.
    unsafe {
        let previous = GetForegroundWindow();
        if SetForegroundWindow(hwnd) != 0 {
            return Forward::Direct;
        }
        let theirs = GetWindowThreadProcessId(previous, null_mut());
        let ours = GetCurrentThreadId();
        if theirs != 0 && theirs != ours && AttachThreadInput(ours, theirs, 1) != 0 {
            let ok = SetForegroundWindow(hwnd) != 0;
            AttachThreadInput(ours, theirs, 0);
            if ok {
                return Forward::Joined;
            }
        }
        Forward::Refused
    }
}

/// A null-terminated UTF-16 copy of `s`, for Win32 "W" functions.
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

/// A private message between Bevel95's own threads and programs, registered by its name
/// (RegisterWindowMessageW) the first time it is used: a number Windows gives every program of the
/// session that asks by the same name, and no other. A fixed WM_APP number could be hit by any
/// program's stray broadcast in that range (the third safety review of 2026-11-18, C4: the
/// classic desktop pasted the clipboard's files or arranged its icons at `WM_APP + 11` and
/// `WM_APP + 5`). The names are listed in decisions.md ("Desktop icons"), for the live tests.
pub struct Private {
    name: &'static str,
    id: std::sync::atomic::AtomicU32,
}

/// What `Private::id` gives when Windows refused to register the name: no window message has this
/// number (they end at 0xFFFF), so nothing matches it and posting it fails.
pub const NO_MESSAGE: u32 = u32::MAX;

impl Private {
    pub const fn new(name: &'static str) -> Private {
        Private { name, id: std::sync::atomic::AtomicU32::new(0) }
    }

    /// The message's number in this session. A name Windows refused to register is remembered as
    /// failed (`NO_MESSAGE`) and logged once; the guards do not ask again at every message (the
    /// fourth safety review of 2026-11-18, C5: each message to the desktop, the bar or the folder
    /// windows asked again and wrote a log line per guard).
    pub fn id(&self) -> u32 {
        self.id_by(
            || {
                let name = wide(self.name);
                // SAFETY: a null-terminated name.
                unsafe { windows_sys::Win32::UI::WindowsAndMessaging::RegisterWindowMessageW(name.as_ptr()) }
            },
            || {
                let message = self.name;
                crate::log::log_win32(&format!("RegisterWindowMessageW({message}); its messages are ignored for this run"));
            },
        )
    }

    /// `id` with the registration and the failure's log line given.
    fn id_by(&self, register: impl FnOnce() -> u32, failed: impl FnOnce()) -> u32 {
        use std::sync::atomic::Ordering;
        let known = self.id.load(Ordering::Relaxed);
        if known != 0 {
            return known;
        }
        let id = match register() {
            0 => NO_MESSAGE,
            id => id,
        };
        // Only the first to store it logs a failure (two threads may ask at once).
        if self.id.compare_exchange(0, id, Ordering::Relaxed, Ordering::Relaxed).is_ok() && id == NO_MESSAGE {
            failed();
        }
        self.id.load(Ordering::Relaxed)
    }

    /// The name it is registered by.
    pub fn name(&self) -> &'static str {
        self.name
    }
}

/// The (x, y) packed into a mouse message's lParam (signed: it can be off the window).
pub fn point(lparam: isize) -> (i32, i32) {
    (i32::from(lparam as u16 as i16), i32::from((lparam >> 16) as u16 as i16))
}

pub fn rect_w(r: &RECT) -> i32 {
    r.right - r.left
}

pub fn rect_h(r: &RECT) -> i32 {
    r.bottom - r.top
}

/// Copies `fb` 1:1 to the top-left of `hdc` (no scaling: the caller already upscaled it
/// with whole-number nearest-neighbour). Pixels beyond `width` are cut off.
pub fn blit(hdc: HDC, fb: &Framebuffer, width: i32) {
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: fb.width,
            biHeight: -fb.height, // negative = rows top to bottom, as in the framebuffer
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..Default::default()
        },
        ..Default::default()
    };
    // SAFETY: `fb.pixels` holds width*height 32-bit pixels matching `info`, and stays alive
    // for the duration of the call.
    unsafe {
        SetDIBitsToDevice(
            hdc,
            0,
            0,
            width.min(fb.width) as u32,
            fb.height as u32,
            0,
            0,
            0,
            fb.height as u32,
            fb.pixels.as_ptr().cast(),
            &info,
            DIB_RGB_COLORS,
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A name Windows refuses is asked for once, logged once, and then stays failed (the fourth
    /// safety review of 2026-11-18, C5: every guard asked again and logged again).
    #[test]
    fn a_name_that_cannot_be_registered_is_remembered_as_failed() {
        let message = Private::new("Bevel95.Test.Refused");
        let (asked, logged) = (std::cell::Cell::new(0), std::cell::Cell::new(0));
        for _ in 0..20 {
            let id = message.id_by(
                || {
                    asked.set(asked.get() + 1);
                    0
                },
                || logged.set(logged.get() + 1),
            );
            assert_eq!(id, NO_MESSAGE);
        }
        assert_eq!((asked.get(), logged.get()), (1, 1));
        // A name Windows registers is asked for once too, and never logged.
        let fine = Private::new("Bevel95.Test.Fine");
        for _ in 0..3 {
            assert_eq!(fine.id_by(|| 0xC123, || panic!("logged")), 0xC123);
        }
    }
}
