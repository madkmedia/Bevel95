//! Win95 and Win98 bring the previous Solitaire game forward. Win2000 can open several games.

use std::cell::RefCell;
use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::Path;
use std::ptr::{null, null_mut};
use std::sync::OnceLock;

use bevel_core::theme::Era;
use bevel_winkit::kit::KIT_CLASS;
use bevel_winkit::log::{data_dir, log};
use bevel_winkit::util::wide;
use windows_sys::Win32::Foundation::{HWND, WAIT_ABANDONED, WAIT_OBJECT_0};
use windows_sys::Win32::System::Threading::{
    CreateMutexW, INFINITE, OpenMutexW, ReleaseMutex, SYNCHRONIZATION_SYNCHRONIZE, WaitForSingleObject,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, GetLastActivePopup, GetWindowThreadProcessId, IsIconic, IsWindowVisible, SW_RESTORE,
    SetForegroundWindow, ShowWindowAsync,
};

static SCOPE: OnceLock<String> = OnceLock::new();
static WINDOW_MARKER: OnceLock<OwnedHandle> = OnceLock::new();
thread_local! {
    // The UI thread keeps the startup mutex from the initial check until its window exists.
    static STARTING: RefCell<Option<Startup>> = const { RefCell::new(None) };
}

/// The names of this copy's Solitaire (`copy::fingerprint` of its data folder, the one rule of
/// the fifth safety review of 2026-11-18, B1; it was a hash of the program's and the data folder's
/// paths that could differ between two builds of one copy).
fn scope(data: &Path) -> io::Result<String> {
    Ok(format!("Local\\Bevel95.Solitaire.{}", bevel_winkit::copy::fingerprint(data)))
}

fn mutex(name: &str) -> io::Result<OwnedHandle> {
    let name = wide(name);
    // SAFETY: the name is NUL-terminated. The successful call returns one owned handle.
    let raw = unsafe { CreateMutexW(null(), 0, name.as_ptr()) };
    if raw.is_null() { Err(io::Error::last_os_error()) } else { Ok(unsafe { OwnedHandle::from_raw_handle(raw) }) }
}

struct Startup(OwnedHandle);

impl Startup {
    fn acquire(scope: &str) -> io::Result<Self> {
        let handle = mutex(scope)?;
        // SAFETY: handle is a live mutex. An abandoned owner also gives us ownership.
        match unsafe { WaitForSingleObject(handle.as_raw_handle(), INFINITE) } {
            WAIT_OBJECT_0 | WAIT_ABANDONED => Ok(Self(handle)),
            _ => Err(io::Error::last_os_error()),
        }
    }
}

impl Drop for Startup {
    fn drop(&mut self) {
        // SAFETY: this guard is acquired and dropped on the UI thread.
        unsafe { ReleaseMutex(self.0.as_raw_handle()) };
    }
}

fn marker_name(scope: &str, pid: u32, window: HWND) -> String {
    format!("{scope}.{pid}.{:x}", window as usize)
}

// Each main window publishes a marker for its own process and handle. Window titles
// alone cannot distinguish another program, a test run, or a separate portable copy.
fn registered_window(scope: &str, class: &str) -> Option<HWND> {
    let class = wide(class);
    let title = wide("Solitaire");
    let mut after = null_mut();
    loop {
        // SAFETY: class and title are terminated, and the previous HWND is only used as a cursor.
        let window = unsafe { FindWindowExW(null_mut(), after, class.as_ptr(), title.as_ptr()) };
        if window.is_null() {
            return None;
        }
        after = window;
        let mut pid = 0;
        // SAFETY: we query the candidate window's owner into a local value.
        if unsafe { GetWindowThreadProcessId(window, &mut pid) } != 0 {
            let marker = wide(&marker_name(scope, pid, window));
            // SAFETY: the name is terminated. Closing our handle leaves the owner's marker alive.
            let handle = unsafe { OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, 0, marker.as_ptr()) };
            if !handle.is_null() {
                drop(unsafe { OwnedHandle::from_raw_handle(handle) });
                return Some(window);
            }
        }
    }
}

pub fn existing(era: Era) -> bool {
    let scope = scope(&data_dir());
    let scope = match scope {
        Ok(scope) => SCOPE.get_or_init(|| scope),
        Err(error) => {
            log(&format!("Solitaire: could not identify this copy: {error}"));
            return false;
        }
    };
    if era == Era::Win2000 {
        return false;
    }
    let startup = match Startup::acquire(scope) {
        Ok(guard) => guard,
        Err(error) => {
            log(&format!("Solitaire: could not coordinate launch: {error}"));
            return false;
        }
    };
    let Some(window) = registered_window(scope, KIT_CLASS) else {
        STARTING.with_borrow_mut(|slot| *slot = Some(startup));
        return false;
    };
    // SAFETY: the marker belongs to a Solitaire main window in this copy. Its last active
    // popup may be the Options or Card Back dialog the player left open.
    unsafe {
        if IsIconic(window) != 0 {
            ShowWindowAsync(window, SW_RESTORE);
        }
        let popup = GetLastActivePopup(window);
        SetForegroundWindow(if !popup.is_null() && IsWindowVisible(popup) != 0 { popup } else { window });
    }
    true
}

pub fn ready(window: HWND) {
    if let Some(scope) = SCOPE.get() {
        match mutex(&marker_name(scope, std::process::id(), window)) {
            Ok(marker) => {
                let _ = WINDOW_MARKER.set(marker);
            }
            Err(error) => log(&format!("Solitaire: could not publish its window: {error}")),
        }
    }
    STARTING.with_borrow_mut(|slot| drop(slot.take()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::{CreateWindowExW, DestroyWindow, WS_POPUP};

    #[test]
    fn a_matching_window_title_requires_the_same_copy_and_window_marker() {
        let exe = std::env::current_exe().unwrap();
        let scope = scope(exe.parent().unwrap()).unwrap();
        let other = format!("{scope}.other");
        let (class, title) = (wide("STATIC"), wide("Solitaire"));
        // SAFETY: this hidden test window is created and destroyed on the same thread.
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                title.as_ptr(),
                WS_POPUP,
                0,
                0,
                1,
                1,
                null_mut(),
                null_mut(),
                null_mut(),
                null(),
            )
        };
        assert!(!hwnd.is_null());
        assert_eq!(registered_window(&scope, "STATIC"), None);
        let marker = mutex(&marker_name(&scope, std::process::id(), hwnd)).unwrap();
        assert_eq!(registered_window(&scope, "STATIC"), Some(hwnd));
        assert_eq!(registered_window(&other, "STATIC"), None);
        drop(marker);
        assert_eq!(registered_window(&scope, "STATIC"), None);
        unsafe { DestroyWindow(hwnd) };
    }
}
