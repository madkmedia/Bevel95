//! Drives locked by BitLocker, on the Windows side (`bevel_core::bitlocker` decides what Windows'
//! answers mean): whether a drive is locked, and Windows' own unlock window for it.
//!
//! Every call here asks a drive, so it runs only on a thread that may wait (a folder reader, the
//! facts of a place), never on a window's thread. Nothing here changes anything: the lock state
//! is only read, and the unlock window is Windows' own program (`bdeunlock.exe`, by its whole
//! path), which asks for the password itself; Bevel95 never sees it.

use std::ptr::{null, null_mut};

use bevel_core::bitlocker;
use windows_sys::Win32::Foundation::{GetLastError, PROPERTYKEY};
use windows_sys::Win32::Storage::FileSystem::GetVolumeInformationW;
use windows_sys::Win32::System::Com::StructuredStorage::{PROPVARIANT, PropVariantClear};
use windows_sys::Win32::UI::Shell::PropertiesSystem::{GPS_DEFAULT, PSGetPropertyKeyFromName, SHGetPropertyStoreFromParsingName};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
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
/// why (`bevel_core::bitlocker::is_locked`). A drive that reads is never locked.
pub fn drive_locked(letter: char) -> bool {
    let w = wide(&root(letter));
    // SAFETY: a null-terminated root; no buffers are asked for.
    let ok = unsafe { GetVolumeInformationW(w.as_ptr(), null_mut(), 0, null_mut(), null_mut(), null_mut(), null_mut(), 0) };
    if ok != 0 {
        return false;
    }
    // SAFETY: a plain query, right after the call that failed.
    let error = unsafe { GetLastError() };
    locked_after_failed_read(letter, error)
}

/// A read of drive `letter` failed with `error` (taken right after the call): whether it is
/// because the drive is locked. Asks the shell only when the error does not already say.
pub fn locked_after_failed_read(letter: char, error: u32) -> bool {
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

/// Windows' own unlock window for drive `letter`, started as Windows' own entry starts it (its
/// program by its whole path, the drive's root as the argument), on a thread of its own.
pub fn start_unlock(letter: char) {
    let letter = letter.to_ascii_uppercase();
    let spawned = std::thread::Builder::new().name("unlock drive".into()).spawn(move || {
        crate::com::init();
        let (exe, arg) = (wide(&unlock_program()), wide(&root(letter)));
        // SAFETY: null-terminated program and argument; a null verb is its default command.
        let r = unsafe { ShellExecuteW(null_mut(), null(), exe.as_ptr(), arg.as_ptr(), null(), SW_SHOWNORMAL) };
        if r as usize <= 32 {
            log(&format!("bitlocker: Windows' unlock window could not start for {letter}: (ShellExecute error {})", r as usize));
        } else {
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
