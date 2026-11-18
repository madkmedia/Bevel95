//! "Start with Windows": a value named "Bevel95" under
//! HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run holding the quoted path of
//! this program, the usual way a program starts when you sign in. Task Manager lists it under
//! Startup apps. (The owner chose this over a shortcut in the Startup folder. It is one of the
//! two places this program writes to the registry; the other, also owner-approved, is Bevel95
//! Setup's entry in Windows' list of installed apps, written with this module's helpers.)
//!
//! Task Manager's own Enable/Disable switch for a startup app is kept by Windows in a value of
//! the same name under ...\Explorer\StartupApproved\Run (12 bytes; an odd first byte means
//! disabled). So the switch counts as on only when our Run value points at this program and
//! Task Manager has not disabled it; turning it on clears such a "disabled" note, so our menu
//! and Task Manager always agree. Only values named "Bevel95" are ever touched.

use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_DWORD, REG_OPTION_NON_VOLATILE, REG_SZ, RRF_RT_REG_BINARY,
    RRF_RT_REG_SZ, RegCloseKey, RegCreateKeyExW, RegDeleteKeyW, RegDeleteValueW, RegGetValueW, RegOpenKeyExW,
    RegSetValueExW,
};

use crate::log::log;
use crate::util::wide;

const VALUE: &str = "Bevel95";

/// Where Windows keeps per-user program settings, or in Setup's test mode the scratch key
/// `Software\<BEVEL95_SETUP_TEST>` (`test_key`): Start with Windows and the installed-apps
/// entry are then written there and nowhere else.
pub(crate) fn base_key() -> String {
    match test_key() {
        Some(name) => format!("Software\\{name}"),
        None => "Software\\Microsoft\\Windows\\CurrentVersion".to_string(),
    }
}

fn run_key() -> String {
    base_key() + "\\Run"
}

fn approved_key() -> String {
    base_key() + if test_key().is_some() { "\\StartupApproved" } else { "\\Explorer\\StartupApproved\\Run" }
}

/// Setup's test mode: `BEVEL95_SETUP_TEST` names a scratch registry key under
/// HKEY_CURRENT_USER\Software, with "test" in its name and only letters, digits, `-`, `_` and
/// `.` (decisions.md, "Bevel95 Setup": testing Setup without touching the owner's installation).
/// Not set: `None`. Set to anything else: the program stops at once, before it writes anything.
pub(crate) fn test_key() -> Option<String> {
    let name = std::env::var("BEVEL95_SETUP_TEST").ok()?;
    let fits = name.to_ascii_lowercase().contains("test")
        && name.len() <= 64
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b));
    if !fits {
        std::process::exit(3);
    }
    Some(name)
}

/// Writes `values` into `key` (made if missing) under HKEY_CURRENT_USER: text, or a number.
pub(crate) fn write_values(key: &str, values: &[(&str, RegValue)]) -> Result<(), String> {
    let handle = open(key, true).map_err(|e| format!("cannot open the key (error {e})"))?;
    let mut result = Ok(());
    for (name, value) in values {
        let name = wide(name);
        let (kind, bytes): (u32, Vec<u8>) = match value {
            RegValue::Text(text) => (REG_SZ, wide(text).iter().flat_map(|u| u.to_le_bytes()).collect()),
            RegValue::Number(n) => (REG_DWORD, n.to_le_bytes().to_vec()),
        };
        // SAFETY: `handle` is an open key; the data is `bytes.len()` bytes long.
        let status = unsafe { RegSetValueExW(handle, name.as_ptr(), 0, kind, bytes.as_ptr(), bytes.len() as u32) };
        if status != ERROR_SUCCESS && result.is_ok() {
            result = Err(format!("RegSetValueExW error {status}"));
        }
    }
    // SAFETY: opened above.
    unsafe { RegCloseKey(handle) };
    result
}

/// A registry value for `write_values`.
pub(crate) enum RegValue {
    Text(String),
    Number(u32),
}

/// A text value of `key` under HKEY_CURRENT_USER.
pub(crate) fn read_text(key: &str, value: &str) -> Option<String> {
    let bytes = read_value(key, value, RRF_RT_REG_SZ)?;
    let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|&pair| u16::from_le_bytes(pair)).collect();
    Some(String::from_utf16_lossy(&units).trim_end_matches('\0').to_string())
}

/// Deletes `key` (with its values; it has no subkeys) under HKEY_CURRENT_USER; fine if missing.
pub(crate) fn delete_key(key: &str) -> Result<(), String> {
    let path = wide(key);
    // SAFETY: a valid string.
    match unsafe { RegDeleteKeyW(HKEY_CURRENT_USER, path.as_ptr()) } {
        ERROR_SUCCESS | ERROR_FILE_NOT_FOUND => Ok(()),
        error => Err(format!("RegDeleteKeyW error {error}")),
    }
}

/// What the Run value should hold: this program's full path, in quotes (it may have spaces).
fn command() -> Option<String> {
    Some(command_for(&std::env::current_exe().ok()?))
}

fn command_for(exe: &std::path::Path) -> String {
    format!("\"{}\"", exe.display())
}

/// Turns starting with Windows on for another copy of the program (Bevel95 Setup installing
/// one). Logs what happened.
pub fn set_on_for(exe: &std::path::Path) {
    match switch_on_with(command_for(exe)) {
        Ok(()) => log(&format!("start with Windows: on, for {}", crate::log::path_kind(exe))),
        Err(e) => log(&format!("start with Windows: could not switch it on: {e}")),
    }
}

/// Reads our value of `key` (under HKEY_CURRENT_USER) as raw bytes, if it exists with that type.
fn read(key: &str, flags: u32) -> Option<Vec<u8>> {
    read_value(key, VALUE, flags)
}

/// Reads `value` of `key` (under HKEY_CURRENT_USER) as raw bytes, if it exists with that type.
fn read_value(key: &str, value: &str, flags: u32) -> Option<Vec<u8>> {
    let (key, value) = (wide(key), wide(value));
    let mut size = 0u32;
    // SAFETY: valid strings; the first call only asks for the size, the second fills a buffer
    // of exactly that size.
    unsafe {
        let status =
            RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), value.as_ptr(), flags, null_mut(), null_mut(), &mut size);
        if status != ERROR_SUCCESS {
            return None;
        }
        let mut data = vec![0u8; size as usize];
        let status = RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            value.as_ptr(),
            flags,
            null_mut(),
            data.as_mut_ptr().cast(),
            &mut size,
        );
        (status == ERROR_SUCCESS).then(|| {
            data.truncate(size as usize);
            data
        })
    }
}

/// Whether Bevel95 starts when you sign in.
pub fn is_on() -> bool {
    std::env::current_exe().is_ok_and(|exe| points_to(&exe))
        && !read(&approved_key(), RRF_RT_REG_BINARY).is_some_and(|b| b.first().is_some_and(|&flags| flags & 1 == 1))
}

/// A disabled Run entry still belongs to its registered copy during removal.
pub(crate) fn points_to(exe: &std::path::Path) -> bool {
    let Some(bytes) = read(&run_key(), RRF_RT_REG_SZ) else {
        return false;
    };
    let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|&pair| u16::from_le_bytes(pair)).collect();
    let have = String::from_utf16_lossy(&units);
    have.trim_end_matches('\0').trim().eq_ignore_ascii_case(&command_for(exe))
}

/// Turns starting with Windows on or off. Logs what happened (and why, if it failed).
pub fn set(on: bool) {
    let result = if on { switch_on() } else { switch_off() };
    match result {
        Ok(()) => log(&format!("start with Windows: {}", if on { "on" } else { "off" })),
        Err(e) => log(&format!("start with Windows: could not switch it {}: {e}", if on { "on" } else { "off" })),
    }
}

fn switch_on() -> Result<(), String> {
    switch_on_with(command().ok_or("the program's own path is unknown")?)
}

fn switch_on_with(command: String) -> Result<(), String> {
    let key = open(&run_key(), true).map_err(|e| format!("cannot open the Run key (error {e})"))?;
    let data = wide(&command);
    let name = wide(VALUE);
    // SAFETY: `key` is an open key; the data is a null-terminated UTF-16 string whose size in
    // bytes (including the null) is passed.
    let status = unsafe {
        let status = RegSetValueExW(key, name.as_ptr(), 0, REG_SZ, data.as_ptr().cast(), (data.len() * 2) as u32);
        RegCloseKey(key);
        status
    };
    if status != ERROR_SUCCESS {
        return Err(format!("RegSetValueExW error {status}"));
    }
    delete(&approved_key()) // a "disabled" note from Task Manager would keep it off
}

fn switch_off() -> Result<(), String> {
    delete(&run_key())?;
    delete(&approved_key())
}

/// Opens `key` under HKEY_CURRENT_USER for writing values (creating it if `create`); the
/// error is Windows' error code.
fn open(key: &str, create: bool) -> Result<HKEY, u32> {
    let path = wide(key);
    let mut handle: HKEY = null_mut();
    // SAFETY: valid strings and out-pointers.
    let status = unsafe {
        if create {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                path.as_ptr(),
                0,
                null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                null(),
                &mut handle,
                null_mut(),
            )
        } else {
            RegOpenKeyExW(HKEY_CURRENT_USER, path.as_ptr(), 0, KEY_SET_VALUE, &mut handle)
        }
    };
    if status == ERROR_SUCCESS { Ok(handle) } else { Err(status) }
}

/// Deletes our value from `key`; fine if the key or the value is not there.
fn delete(key: &str) -> Result<(), String> {
    let handle = match open(key, false) {
        Ok(handle) => handle,
        Err(ERROR_FILE_NOT_FOUND) => return Ok(()), // no such key: nothing to delete
        Err(e) => return Err(format!("cannot open a key (error {e})")),
    };
    let name = wide(VALUE);
    // SAFETY: `handle` is an open key; closed right after.
    let status = unsafe {
        let status = RegDeleteValueW(handle, name.as_ptr());
        RegCloseKey(handle);
        status
    };
    match status {
        ERROR_SUCCESS | ERROR_FILE_NOT_FOUND => Ok(()),
        error => Err(format!("RegDeleteValueW error {error}")),
    }
}
