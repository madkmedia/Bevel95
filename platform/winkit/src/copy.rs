//! Programs of two copies never serve each other (the fifth safety review of 2026-11-18, B1).
//!
//! Every name by which one Bevel95 program finds another of its own kind (the folder windows'
//! hand-over, a game's "already running", the Undo list's lock) carries a short fingerprint of
//! the copy's data folder, so only programs of the same copy find each other. Before, an installed
//! copy's folder open could land in a portable copy's folder program, which then kept that
//! window's places and Undo steps in the other copy's folder (and the reverse).
//!
//! Two names stay one for the session on purpose, because there is one screen: the bar's place
//! (`Local\Bevel95.Bar`, and its window class, which `--restore` and Setup look for) and the lock
//! of Windows' taskbar record (`Local\Bevel95 taskbar record`).

use std::os::windows::io::{FromRawHandle, OwnedHandle};
use std::path::{Path, PathBuf};
use std::ptr::null;

use windows_sys::Win32::System::Threading::{CreateMutexW, OpenMutexW, SYNCHRONIZATION_SYNCHRONIZE};

use crate::log::{Home, home, installed_dir};
use crate::util::wide;

/// The fingerprint of a folder's path as written (`text`): 16 hex digits of FNV-1a (64 bits, the
/// same in every build and version) over the path in lower case, with `/` as `\`, without
/// trailing `\` and without the `\\?\` prefix. Pure: one folder by any of these spellings gives
/// one fingerprint.
pub fn fingerprint_of_text(text: &str) -> String {
    let mut t = text.replace('/', "\\");
    if let Some(rest) = t.strip_prefix("\\\\?\\UNC\\") {
        t = format!("\\\\{rest}");
    } else if let Some(rest) = t.strip_prefix("\\\\?\\") {
        t = rest.to_string();
    }
    let t = t.trim_end_matches('\\').to_lowercase();
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in t.bytes() {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// The fingerprint of folder `dir`: of its final path as Windows resolves it (the folder opened
/// and asked for its final name: `std::fs::canonicalize`, GetFinalPathNameByHandleW), so a mapped
/// drive and its network name, a `subst` drive, a junction and a short name give one fingerprint
/// (the sixth safety review of 2026-11-18, C1: the path as spelled made one copy two). When that
/// cannot be had, of the path as spelled with its short names spelled out (GetLongPathNameW), and
/// the log says so once. Every name that carries the copy comes through here (`name`, Solitaire's).
pub fn fingerprint(dir: &Path) -> String {
    let resolved = match std::fs::canonicalize(dir) {
        Ok(path) => path,
        Err(e) => {
            if !SPELLED_LOGGED.swap(true, std::sync::atomic::Ordering::SeqCst) {
                crate::log::log(&format!(
                    "copy: the final name of this copy's folder could not be had (Win32 error {}); its programs are found by \
                     the folder as spelled (logged once)",
                    e.raw_os_error().unwrap_or(0)
                ));
            }
            long_form(dir)
        }
    };
    fingerprint_of_text(&resolved.to_string_lossy())
}

/// Logged once per run: a fingerprint was of the folder as spelled.
static SPELLED_LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// `path` with its short names spelled out (GetLongPathNameW), or as it is.
fn long_form(path: &Path) -> PathBuf {
    use windows_sys::Win32::Storage::FileSystem::GetLongPathNameW;
    let name = wide(&path.to_string_lossy());
    let mut buf = vec![0u16; 1024];
    // SAFETY: a null-terminated name and a buffer of the length passed.
    let n = unsafe { GetLongPathNameW(name.as_ptr(), buf.as_mut_ptr(), buf.len() as u32) } as usize;
    if n == 0 || n >= buf.len() {
        return path.to_path_buf();
    }
    PathBuf::from(String::from_utf16_lossy(&buf[..n]))
}

/// This run's data folder (decided at its start: `log::home`).
fn data_folder() -> PathBuf {
    match home() {
        Home::Portable(dir) => dir,
        Home::Installed | Home::NotKnown => installed_dir(),
    }
}

/// `base` with this copy's fingerprint: `base.<16 hex digits>`.
pub fn name(base: &str) -> String {
    format!("{base}.{}", fingerprint(&data_folder()))
}

/// The mark "process `pid` is a program `kind` of this copy" (`Local\Bevel95.<kind>.<copy>.<pid>`).
fn mark_name(kind: &str, pid: u32) -> String {
    format!("{}.{pid}", name(&format!("Local\\Bevel95.{kind}")))
}

/// Puts up this process's mark of being a program `kind` of this copy, held until the process
/// ends (keep the handle). A program that finds another's window by its class and title asks
/// `is_of_this_copy` before it uses it.
pub fn mark(kind: &str) -> Option<OwnedHandle> {
    let name = wide(&mark_name(kind, std::process::id()));
    // SAFETY: a null-terminated name; the handle is owned by the result.
    let raw = unsafe { CreateMutexW(null(), 0, name.as_ptr()) };
    if raw.is_null() {
        crate::log::log_win32("CreateMutexW(a program's mark of its copy)");
        return None;
    }
    // SAFETY: a handle just made, owned by nothing else.
    Some(unsafe { OwnedHandle::from_raw_handle(raw) })
}

/// Whether process `pid` has put up the mark of a program `kind` of this copy.
pub fn is_of_this_copy(kind: &str, pid: u32) -> bool {
    let name = wide(&mark_name(kind, pid));
    // SAFETY: a null-terminated name; a handle opened is closed at once.
    unsafe {
        let h = OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, 0, name.as_ptr());
        if h.is_null() {
            return false;
        }
        windows_sys::Win32::Foundation::CloseHandle(h);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two folders give two names; one folder gives one name however its path is written: case, a
    /// trailing backslash, forward slashes, the `\\?\` prefix, and its short (8.3) name.
    #[test]
    fn one_copy_one_name_two_copies_two_names() {
        let a = fingerprint_of_text("C:\\Users\\x\\AppData\\Local\\Bevel95");
        for same in [
            "c:\\users\\X\\appdata\\local\\BEVEL95",
            "C:\\Users\\x\\AppData\\Local\\Bevel95\\",
            "C:/Users/x/AppData/Local/Bevel95/",
            "\\\\?\\C:\\Users\\x\\AppData\\Local\\Bevel95",
        ] {
            assert_eq!(fingerprint_of_text(same), a, "{same}");
        }
        for other in [
            "D:\\Tools\\Bevel95",
            "C:\\Users\\x\\AppData\\Local\\Bevel95\\programs",
            "C:\\Users\\someone\\AppData\\Local\\Bevel95",
        ] {
            assert_ne!(fingerprint_of_text(other), a, "{other}");
        }
        assert_eq!(a.len(), 16);
        assert_eq!(fingerprint_of_text("\\\\?\\UNC\\server\\share\\B"), fingerprint_of_text("\\\\server\\share\\b\\"));
        // A real folder by its short name, when the disk keeps short names.
        let long = std::env::temp_dir().join(format!("Bevel95 test long folder name {}", std::process::id()));
        std::fs::create_dir_all(&long).unwrap();
        let wide_long = wide(&long.to_string_lossy());
        let mut buf = vec![0u16; 1024];
        // SAFETY: a null-terminated name and a buffer of the length passed.
        let n = unsafe {
            windows_sys::Win32::Storage::FileSystem::GetShortPathNameW(
                wide_long.as_ptr(),
                buf.as_mut_ptr(),
                buf.len() as u32,
            )
        } as usize;
        let short = PathBuf::from(String::from_utf16_lossy(&buf[..n.min(buf.len())]));
        assert!(n > 0);
        assert_eq!(fingerprint(&short), fingerprint(&long), "{} and its long form", short.display());
        assert_eq!(fingerprint(&long.join("..").join(long.file_name().unwrap())).len(), 16);
        std::fs::remove_dir_all(&long).unwrap();
    }

    /// One copy reached through a junction is one copy (the sixth safety review of 2026-11-18, C1:
    /// the fingerprint was of the path as spelled, so a junction, a `subst` drive or a mapped drive
    /// and its network name made one copy two). The junction is made in a scratch folder under
    /// private\probes\review6\ and removed again (only the link: its folder stays until the scratch
    /// folder goes).
    #[test]
    fn one_copy_through_a_junction_has_one_fingerprint() {
        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("..\\..\\private\\probes\\review6").join(format!("junction-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let copy = base.join("Bevel95 copy");
        std::fs::create_dir_all(&copy).unwrap();
        let link = base.join("Bevel95 link");
        let made = crate::quiet::command(crate::shell::system_program("cmd.exe"))
            .args(["/d", "/c", "mklink", "/J"])
            .arg(&link)
            .arg(&copy)
            .stdout(std::process::Stdio::null())
            .status();
        assert!(made.is_ok_and(|s| s.success()), "mklink /J");
        let (through_link, direct) = (fingerprint(&link), fingerprint(&copy));
        assert_ne!(fingerprint_of_text(&link.to_string_lossy()), fingerprint_of_text(&copy.to_string_lossy()), "two spellings");
        // The link only (a junction is removed as a folder, its target untouched), then the rest.
        std::fs::remove_dir(&link).unwrap();
        assert!(copy.is_dir(), "the junction's folder is untouched");
        std::fs::remove_dir_all(&base).unwrap();
        assert_eq!(through_link, direct, "one copy, one fingerprint");
        // A folder that cannot be opened (gone): its spelling, as before.
        assert_eq!(fingerprint(&copy), fingerprint_of_text(&copy.to_string_lossy()));
    }

    /// A program's mark is seen for its own process and copy only.
    #[test]
    fn a_mark_is_seen_for_its_process_and_kind() {
        let kind = format!("Test{}", std::process::id());
        let me = std::process::id();
        assert!(!is_of_this_copy(&kind, me));
        let held = mark(&kind).unwrap();
        assert!(is_of_this_copy(&kind, me));
        assert!(!is_of_this_copy(&kind, me ^ 0x40), "another process");
        assert!(!is_of_this_copy("TestOther", me), "another kind");
        drop(held);
        assert!(!is_of_this_copy(&kind, me), "gone with its handle");
    }
}
