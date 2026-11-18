//! The game shares Bevel95's installed or portable data directory with the other programs.

use std::fs;
use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::Path;

use bevel_core::solitaire::settings::Settings;
use bevel_winkit::log::data_dir;
use bevel_winkit::state_file::{self, Read};
use windows_sys::Win32::Foundation::{WAIT_ABANDONED, WAIT_OBJECT_0};
use windows_sys::Win32::System::Threading::{CreateMutexW, INFINITE, ReleaseMutex, WaitForSingleObject};

const FILE: &str = "solitaire.txt";

/// The options as saved (an unreadable file is logged and kept: the defaults; `update` reads the
/// file again for every save and leaves an unreadable one alone).
pub fn load(first_rand: u32) -> Settings {
    let text = state_file::load(&data_dir().join(FILE)).text().unwrap_or_default();
    Settings::parse_with_random(&text, first_rand)
}

pub fn save_options(settings: &Settings) -> io::Result<()> {
    update(&data_dir(), "Options", settings.options_value())
}

pub fn save_back(settings: &Settings) -> io::Result<()> {
    update(&data_dir(), "Back", settings.deck + 1)
}

/// The mutex stays independent of the file being replaced and disappears when no writer
/// has a handle to it. A second instance therefore reads the first instance's completed edit.
struct Writer(OwnedHandle);

impl Writer {
    fn lock(dir: &Path) -> io::Result<Self> {
        let dir = fs::canonicalize(dir)?;
        // A fixed FNV-1a hash gives all program versions the same legal, short object name.
        // Aliases (including relative paths and directory links) resolve before hashing.
        let mut id = 0xcbf29ce484222325u64;
        for byte in dir.to_string_lossy().to_lowercase().bytes() {
            id = (id ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
        let name: Vec<_> = format!("Global\\Bevel95.Solitaire.Settings.{id:016x}").encode_utf16().chain([0]).collect();
        // SAFETY: the name is NUL-terminated. The new handle is not inherited and has one owner.
        let raw = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
        if raw.is_null() {
            return Err(io::Error::last_os_error());
        }
        let handle = unsafe { OwnedHandle::from_raw_handle(raw) };
        // SAFETY: the handle stays alive throughout the wait. An abandoned mutex also grants
        // ownership. The live settings file still contains a complete version after a crash.
        match unsafe { WaitForSingleObject(handle.as_raw_handle(), INFINITE) } {
            WAIT_OBJECT_0 | WAIT_ABANDONED => Ok(Self(handle)),
            _ => Err(io::Error::last_os_error()),
        }
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        // SAFETY: the thread calling update acquired this mutex and still owns its handle.
        unsafe { ReleaseMutex(self.0.as_raw_handle()) };
    }
}

fn update(dir: &Path, key: &str, value: u8) -> io::Result<()> {
    let _writer = Writer::lock(dir)?;
    let existing = match state_file::read(&dir.join(FILE)) {
        Read::Text(text) => text,
        Read::Missing => String::new(),
        Read::Unreadable(why) => return Err(io::Error::other(format!("not saved, the file is kept as it is: {why}"))),
    };
    let line = format!("{key}={value}\r\n");
    let mut next = String::new();
    let mut replaced = false;
    for previous in existing.lines() {
        if previous.split_once('=').is_some_and(|(name, _)| name.trim().eq_ignore_ascii_case(key)) {
            if !replaced {
                next.push_str(&line);
                replaced = true;
            }
        } else {
            next.push_str(previous);
            next.push_str("\r\n");
        }
    }
    if next.is_empty() {
        next.push_str("[Solitaire]\r\n");
    }
    if !replaced {
        next.push_str(&line);
    }
    write_settings(dir, &next)
}

/// Writes the file whole (Bevel95's shared way: a private file on the same volume, put in
/// place). Readers see either the old settings or the new, complete file, including on a
/// failed or concurrent save.
fn write_settings(dir: &Path, text: &str) -> io::Result<()> {
    state_file::write_whole(&dir.join(FILE), text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simultaneous_windows_keep_each_others_options_and_card_back() {
        let dir =
            std::env::current_exe().unwrap().with_file_name(format!("solitaire-merge-test-{}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        let file = dir.join(FILE);
        update(&dir, "Back", 9).unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "[Solitaire]\r\nBack=9\r\n");
        fs::write(&file, "[Solitaire]\r\nOptions=11\r\nBack=9\r\nback=1\r\n# player note\r\n").unwrap();
        let start = std::sync::Barrier::new(2);
        std::thread::scope(|scope| {
            for (key, value) in [("Options", 85), ("Back", 12)] {
                let (dir, start) = (&dir, &start);
                scope.spawn(move || {
                    start.wait();
                    update(dir, key, value).unwrap();
                });
            }
        });
        assert_eq!(fs::read_to_string(&file).unwrap(), "[Solitaire]\r\nOptions=85\r\nBack=12\r\n# player note\r\n");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        fs::remove_file(file).unwrap();
        fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn a_save_replaces_an_existing_file_and_cleans_up_after_failed_replacement() {
        let test_binary = std::env::current_exe().unwrap();
        let dir = test_binary.with_file_name(format!("solitaire-settings-test-{}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        let file = dir.join(FILE);
        fs::write(&file, "previous settings").unwrap();
        write_settings(&dir, "new settings").unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "new settings");

        fs::remove_file(&file).unwrap();
        fs::create_dir(&file).unwrap();
        assert!(write_settings(&dir, "another set").is_err());
        assert!(file.is_dir());
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        fs::remove_dir(file).unwrap();
        fs::remove_dir(dir).unwrap();
    }
}
