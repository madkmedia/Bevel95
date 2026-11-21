//! Drives locked by BitLocker, on the Windows side (`bevel_core::bitlocker` decides what Windows'
//! answers mean): whether a drive is locked, and Windows' own unlock window for it.
//!
//! Every call here asks a drive, so it runs only on a thread that may wait (a folder reader, the
//! facts of a place), never on a window's thread. Nothing here changes anything: the lock state
//! is only read, and the unlock window is Windows' own program (`bdeunlock.exe`, by its whole
//! path), which asks for the password itself; Bevel95 never sees it.

use std::ptr::null_mut;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use bevel_core::bitlocker::{self, UnlockStart, UnlockStarts};
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, PROPERTYKEY, WAIT_TIMEOUT};
use windows_sys::Win32::Storage::FileSystem::{GetDriveTypeW, GetVolumeInformationW};
use windows_sys::Win32::System::Com::StructuredStorage::{PROPVARIANT, PropVariantClear};
use windows_sys::Win32::System::Threading::WaitForSingleObject;
use windows_sys::Win32::UI::Shell::PropertiesSystem::{GPS_DEFAULT, PSGetPropertyKeyFromName, SHGetPropertyStoreFromParsingName};
use windows_sys::Win32::UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows_sys::core::GUID;

use crate::log::log;
use crate::util::wide;

/// IPropertyStore's interface id (propsys.h; windows-sys has the interface's functions only).
const IID_IPROPERTY_STORE: GUID = GUID::from_u128(0x886d8eeb_8cf2_4446_8d02_cdba1dbdcf99);

/// The root of drive `letter` ("E:\").
fn root(letter: char) -> String {
    format!("{}:\\", letter.to_ascii_uppercase())
}

/// Whether drive `letter` is locked by BitLocker: its volume cannot be read and Windows says
/// why (`bevel_core::bitlocker::is_locked`). A drive that reads is never locked, and a drive of a
/// kind BitLocker does not lock (`bitlocker::may_be_locked`: a network drive, a CD or DVD drive, a
/// RAM disk) is not even read here.
pub fn drive_locked(letter: char) -> bool {
    let w = wide(&root(letter));
    // SAFETY: a null-terminated root path.
    let drive_type = unsafe { GetDriveTypeW(w.as_ptr()) };
    if !bitlocker::may_be_locked(drive_type) {
        return false;
    }
    // SAFETY: a null-terminated root; no buffers are asked for.
    let ok = unsafe { GetVolumeInformationW(w.as_ptr(), null_mut(), 0, null_mut(), null_mut(), null_mut(), null_mut(), 0) };
    if ok != 0 {
        return false;
    }
    // SAFETY: a plain query, right after the call that failed.
    let error = unsafe { GetLastError() };
    locked_after_failed_read(letter, drive_type, error)
}

/// A read of drive `letter` (of Windows' kind `drive_type`, `GetDriveTypeW`) failed with `error`
/// (taken right after the call): whether it is because the drive is locked. Asks the shell only
/// of a drive that can be locked at all, and only when the error does not already say.
pub fn locked_after_failed_read(letter: char, drive_type: u32, error: u32) -> bool {
    if !bitlocker::may_be_locked(drive_type) {
        return false;
    }
    let protection = if bitlocker::ask_shell_after(error) { protection(letter) } else { None };
    bitlocker::is_locked(Some(error), protection)
}

/// The shell's `System.Volume.BitLockerProtection` of drive `letter` (Windows' own condition for
/// its Unlock Drive entry), if it answers.
pub fn protection(letter: char) -> Option<u32> {
    // The shell needs COM on this thread (the callers' threads have it already; harmless again).
    crate::com::init();
    let name = wide("System.Volume.BitLockerProtection");
    let mut key = PROPERTYKEY { fmtid: GUID::from_u128(0), pid: 0 };
    // SAFETY: a null-terminated name and a valid out-pointer.
    if unsafe { PSGetPropertyKeyFromName(name.as_ptr(), &mut key) } < 0 {
        return None;
    }
    let path = wide(&root(letter));
    let mut store: *mut core::ffi::c_void = null_mut();
    // SAFETY: a null-terminated path, the interface id it is asked for and a valid out-pointer.
    let hr = unsafe { SHGetPropertyStoreFromParsingName(path.as_ptr(), null_mut(), GPS_DEFAULT, &IID_IPROPERTY_STORE, &mut store) };
    if hr < 0 || store.is_null() {
        return None;
    }
    // IPropertyStore's table: IUnknown's three, then GetCount, GetAt, GetValue.
    type GetValue = unsafe extern "system" fn(*mut core::ffi::c_void, *const PROPERTYKEY, *mut PROPVARIANT) -> i32;
    const GET_VALUE: usize = 5;
    let mut value = PROPVARIANT::default();
    // SAFETY: `store` is a live IPropertyStore (S_OK above) whose table has GetValue at its sixth
    // place, as propsys.h declares it. The value is cleared and the store released exactly once.
    unsafe {
        let hr = crate::com::method::<GetValue>(store, GET_VALUE)(store, &key, &mut value);
        let v = &value.Anonymous.Anonymous;
        let answer = match (hr >= 0, v.vt) {
            // VT_I4, VT_UI4 (Windows' own list gives the values as VT_I4).
            (true, 3 | 19) => Some(v.Anonymous.ulVal),
            // VT_I2, VT_UI2.
            (true, 2 | 18) => Some(u32::from(v.Anonymous.uiVal)),
            _ => None,
        };
        PropVariantClear(&mut value);
        crate::com::release(store);
        answer
    }
}

/// Windows' own program behind its "Unlock Drive..." entry (`HKCR\Drive\shell\unlock-bde`,
/// command `%SystemRoot%\System32\bdeunlock.exe %1`), by its whole path (`shell::system_program`).
pub fn unlock_program() -> String {
    crate::shell::system_program("bdeunlock.exe")
}

/// When the unlock window was last started for each drive, in this program
/// (`bitlocker::UnlockStarts`). Only folders.exe starts it, and it is one process per copy.
static STARTS: Mutex<UnlockStarts> = Mutex::new(UnlockStarts::new());

/// Milliseconds since the first time this program asked: a clock that only goes forward.
fn now_ms() -> u64 {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    EPOCH.get_or_init(Instant::now).elapsed().as_millis() as u64
}

/// The process of the unlock window started last for each drive (A to Z), as a number (a
/// `HANDLE`, kept open until it is seen to have ended; 0: none).
static RUNNING: Mutex<[usize; 26]> = Mutex::new([0; 26]);

/// The slot of drive `letter` in `RUNNING`, if it is a letter A to Z.
fn slot(letter: char) -> Option<usize> {
    let l = letter.to_ascii_uppercase();
    l.is_ascii_uppercase().then(|| (l as u8 - b'A') as usize)
}

/// Whether the unlock window started last for `letter` is still running; one that has ended is
/// let go of.
fn still_running(letter: char) -> bool {
    let Some(i) = slot(letter) else { return false };
    let Ok(mut running) = RUNNING.lock() else { return false };
    let h = running[i];
    if h == 0 {
        return false;
    }
    // SAFETY: a process handle this program got from ShellExecuteExW and has not closed yet;
    // a wait of 0 ms only asks.
    if unsafe { WaitForSingleObject(h as HANDLE, 0) } == WAIT_TIMEOUT {
        return true;
    }
    // SAFETY: the same handle, closed exactly once (its slot is emptied with it).
    unsafe { CloseHandle(h as HANDLE) };
    running[i] = 0;
    false
}

/// The unlock window just started for `letter` runs as process `h` (kept until it ends).
fn started(letter: char, h: HANDLE) {
    if h.is_null() {
        return;
    }
    let (Some(i), Ok(mut running)) = (slot(letter), RUNNING.lock()) else {
        // SAFETY: a handle ShellExecuteExW gave and nothing else keeps.
        unsafe { CloseHandle(h) };
        return;
    };
    if running[i] != 0 {
        // SAFETY: the older handle of that slot, closed exactly once as it is replaced.
        unsafe { CloseHandle(running[i] as HANDLE) };
    }
    running[i] = h as usize;
}

/// Whether the unlock window may be started for `letter` now (`bitlocker::UnlockStarts`): not
/// while the one started last for it still runs, nor within two seconds of a start; a start left
/// out is logged once.
fn may_start(letter: char) -> bool {
    let running = still_running(letter);
    let Ok(mut starts) = STARTS.lock() else { return false };
    match starts.ask(letter, now_ms(), running) {
        UnlockStart::Start => true,
        UnlockStart::LeftOut { first } => {
            if first {
                log(&format!("bitlocker: Windows' unlock window for {}: is open or was started just now; not started again", letter.to_ascii_uppercase()));
            }
            false
        }
        UnlockStart::NotADrive => false,
    }
}

/// Windows' own unlock window for drive `letter`, started as Windows' own entry starts it (its
/// program by its whole path, the drive's root as the argument), on a thread of its own; not
/// while the one started last for that drive still runs, and at most once for one drive within
/// `bitlocker::UNLOCK_AGAIN_AFTER_MS`, whoever asks.
pub fn start_unlock(letter: char) {
    if !may_start(letter) {
        return;
    }
    let letter = letter.to_ascii_uppercase();
    let spawned = std::thread::Builder::new().name("unlock drive".into()).spawn(move || {
        crate::com::init();
        let (exe, arg) = (wide(&unlock_program()), wide(&root(letter)));
        let mut info = SHELLEXECUTEINFOW {
            cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
            // Its process is kept, to know while it runs.
            fMask: SEE_MASK_NOCLOSEPROCESS,
            lpFile: exe.as_ptr(),
            lpParameters: arg.as_ptr(),
            nShow: SW_SHOWNORMAL,
            ..Default::default()
        };
        // SAFETY: a filled structure of the size given, with null-terminated program and
        // argument that live until the call returns; a null verb is its default command.
        if unsafe { ShellExecuteExW(&mut info) } == 0 {
            // SAFETY: a plain query, right after the call that failed.
            log(&format!("bitlocker: Windows' unlock window could not start for {letter}: (error {})", unsafe { GetLastError() }));
        } else {
            started(letter, info.hProcess);
            log(&format!("bitlocker: Windows' unlock window started for {letter}:"));
        }
    });
    if let Err(e) = spawned {
        log(&format!("bitlocker: could not start the unlock window: {e}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::Storage::FileSystem::GetLogicalDrives;
    use windows_sys::Win32::System::Registry::{HKEY_CLASSES_ROOT, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ, RegGetValueW};

    /// The unlock program exists on this PC, and it is the one Windows' own Unlock Drive entry
    /// starts (read from the registry; nothing is started).
    #[test]
    fn windows_own_unlock_program_is_the_one_started() {
        let program = unlock_program();
        assert!(std::path::Path::new(&program).is_file(), "{program}");
        let (key, value) = (wide("Drive\\shell\\unlock-bde\\command"), wide(""));
        let mut buf = vec![0u16; 1024];
        let mut size = (buf.len() * 2) as u32;
        // SAFETY: null-terminated names, a buffer of the size given.
        let r = unsafe {
            RegGetValueW(HKEY_CLASSES_ROOT, key.as_ptr(), value.as_ptr(), RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ, null_mut(), buf.as_mut_ptr().cast(), &mut size)
        };
        if r != 0 {
            // A Windows without BitLocker (Home) registers no such entry: nothing to compare.
            eprintln!("no unlock-bde entry on this Windows ({r})");
            return;
        }
        let n = buf.iter().position(|&c| c == 0).unwrap_or(0);
        let command = String::from_utf16_lossy(&buf[..n]).to_lowercase();
        let file = std::path::Path::new(&program).file_name().unwrap().to_string_lossy().to_lowercase();
        assert!(command.contains(&file) && command.contains("%1"), "{command}");
        assert!(command.contains("system32\\bdeunlock.exe"), "{command}");
    }

    /// Every drive of this PC is found as the shell says: locked only where Windows' own
    /// condition for Unlock Drive holds (on a PC without a locked drive: none).
    #[test]
    fn each_drive_is_found_as_windows_says() {
        // SAFETY: a plain query.
        let drives = unsafe { GetLogicalDrives() };
        for n in 0..26u8 {
            if drives & (1 << n) == 0 {
                continue;
            }
            let letter = (b'A' + n) as char;
            if letter <= 'B' {
                continue;
            }
            let (locked, shell) = (drive_locked(letter), protection(letter));
            eprintln!("{letter}: locked {locked}, shell's BitLockerProtection {shell:?}");
            assert_eq!(locked, shell == Some(bitlocker::PROTECTION_LOCKED), "{letter}:");
        }
    }
}
