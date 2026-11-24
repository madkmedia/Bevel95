//! Settings and state files (settings.txt, desktop.txt, folders.txt, the games' files, ...):
//! written whole, and never replaced because they could not be read (the safety review of
//! 2026-11-18, B1).
//!
//! - **Written whole** (`write_whole`): under a temporary name beside the file, flushed to the
//!   disk, then put in place in one step (MoveFileExW with MOVEFILE_WRITE_THROUGH). A reader, or
//!   the next start after a power cut, finds the old file or the new one, never a part of one.
//! - **Read as text** (`read`): a file that is not there is `Missing` (the program runs on its
//!   defaults and may create it). A file that is there but cannot be read (in use for a moment,
//!   not UTF-8, UTF-16, holding NUL bytes) is `Unreadable`: it is kept as it is, never converted.
//!   A UTF-8 byte-order mark (Notepad's "UTF-8 with BOM") is read as if it were not there.
//! - **Loaded by a program** (`load`): an unreadable file is logged once; the program runs on its
//!   defaults for it, and every later `save` of that file is skipped (logged) for as long as the
//!   file is there, since the program's picture of it is the defaults, not the file.
//! - **Changed in place** (`change`): read now, changed, written whole; skipped (logged) while the
//!   file cannot be read, so one bad read can never turn a file into "the defaults plus one
//!   setting".

use std::ffi::OsStr;
use std::io::{self, Write};
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

use windows_sys::Win32::Storage::FileSystem::{MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW};

use crate::log::{log, path_kind};

/// What reading a state file found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Read {
    /// No such file: the defaults (and the file may be created).
    Missing,
    /// The file's text (without a UTF-8 byte-order mark).
    Text(String),
    /// The file is there but cannot be used as text; the reason, for the log.
    Unreadable(String),
}

impl Read {
    /// The text, or `None` for a missing or unreadable file (the defaults).
    pub fn text(self) -> Option<String> {
        match self {
            Read::Text(text) => Some(text),
            _ => None,
        }
    }
}

/// Reads `path` as a state file's text (see the module's comment). Logs nothing.
pub fn read(path: &Path) -> Read {
    match std::fs::read(path) {
        Ok(bytes) => decode(bytes),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Read::Missing,
        Err(e) => Read::Unreadable(format!("it could not be opened ({e})")),
    }
}

/// A state file's bytes as text: UTF-8, with or without a byte-order mark.
pub(crate) fn decode(mut bytes: Vec<u8>) -> Read {
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        return Read::Unreadable("it is saved as UTF-16 (Unicode); Bevel95 reads UTF-8".into());
    }
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        bytes.drain(..3);
    }
    if bytes.contains(&0) {
        return Read::Unreadable("it holds NUL bytes (UTF-16 without a mark, or not text)".into());
    }
    match String::from_utf8(bytes) {
        Ok(text) => Read::Text(text),
        Err(_) => Read::Unreadable("it is not UTF-8 text (an ANSI file with an accented letter?)".into()),
    }
}

/// The files this program could not read when it loaded them: their saves are skipped while
/// they are there.
static UNREAD: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

fn unread(path: &Path) -> bool {
    UNREAD.lock().is_ok_and(|list| list.iter().any(|p| p == path))
}

fn set_unread(path: &Path, on: bool) {
    if let Ok(mut list) = UNREAD.lock() {
        list.retain(|p| p != path);
        if on {
            list.push(path.to_path_buf());
        }
    }
}

/// Reads a state file for a program that keeps its own picture of it (and later saves that
/// picture whole with `save`). An unreadable file is logged and marked: the program runs on its
/// defaults, and its saves are skipped while the file is there.
pub fn load(path: &Path) -> Read {
    let read = read(path);
    match &read {
        Read::Unreadable(why) => {
            if !unread(path) {
                log(&format!(
                    "{} cannot be read: {why}; it is kept as it is, this program runs on its defaults for it and does not save it",
                    path_kind(path)
                ));
            }
            set_unread(path, true);
        }
        _ => set_unread(path, false),
    }
    read
}

/// Saves a program's whole picture of a state file. Skipped (an error saying so) while the file
/// is there and this program could not load it, or cannot read it now: an unreadable file is
/// never replaced.
pub fn save(path: &Path, text: &str) -> io::Result<()> {
    let present = std::fs::symlink_metadata(path).is_ok();
    if present && unread(path) {
        return Err(kept("it could not be read when this program loaded it"));
    }
    if present && let Read::Unreadable(why) = read(path) {
        return Err(kept(&why));
    }
    write_whole(path, text.as_bytes())?;
    set_unread(path, false);
    Ok(())
}

/// Changes a state file that is read again for every change (settings.txt, run-history.txt):
/// `f` gets its text now, or `default` when there is none, and the result is written whole.
/// Skipped (an error saying so) while the file is there but cannot be read.
pub fn change(path: &Path, default: &str, f: impl FnOnce(&str) -> String) -> io::Result<()> {
    let text = match read(path) {
        Read::Text(text) => text,
        Read::Missing => default.to_string(),
        Read::Unreadable(why) => return Err(kept(&why)),
    };
    write_whole(path, f(&text).as_bytes())
}

/// Changes a settings.txt (the bar's settings, the clock's, Taskbar Properties', Setup's answers):
/// as `change`, with Bevel95's defaults for a missing file, and **never a settings.txt that is not
/// provably Bevel95's own**: a file that is there and holds no `log::SETTINGS_MARK` is not written,
/// by any path (the third safety review of 2026-11-18, A1: Bevel95 wrote its keys into another
/// program's settings.txt). Refused, it is logged once per file and run (`already_logged` tells
/// the error); the setting changed then holds for this run only.
pub fn change_settings(path: &Path, f: impl FnOnce(&str) -> String) -> io::Result<()> {
    change_settings_in(path, &crate::log::installed_dir(), f)
}

/// `change_settings` with `installed` the installed folder. There a settings.txt is always
/// Bevel95's own (only Bevel95 writes that folder): it is saved whatever its first line is, and the
/// mark is put back as its first line when the person removed it (the fourth safety review of
/// 2026-11-18, C1: every change was refused there, silently). The rule stays for every other
/// folder, which may hold another program's file.
fn change_settings_in(path: &Path, installed: &Path, f: impl FnOnce(&str) -> String) -> io::Result<()> {
    let own_folder = path.parent().is_some_and(|dir| {
        dir.to_string_lossy().trim_end_matches('\\').eq_ignore_ascii_case(installed.to_string_lossy().trim_end_matches('\\'))
    });
    let text = match read(path) {
        Read::Text(text) if crate::log::has_settings_mark(&text) => text,
        Read::Text(text) if own_folder => {
            let mark = bevel_core::config::DEFAULT_FILE.lines().next().unwrap_or(crate::log::SETTINGS_MARK);
            log_once(&MARK_BACK_LOGGED, path, &format!(
                "{} had lost its first line \"{}\"; it is Bevel95's own folder, so the line is put back and the settings are \
                 saved (logged once)",
                path_kind(path),
                crate::log::SETTINGS_MARK
            ));
            format!("{mark}\n{text}")
        }
        Read::Text(_) => {
            log_once(&NOT_OURS_LOGGED, path, &format!(
                "{} holds no \"{}\" line, so it is not Bevel95's own: it is kept as it is and never written; a setting changed \
                 holds for this run only (logged once)",
                path_kind(path),
                crate::log::SETTINGS_MARK
            ));
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, NotOurs));
        }
        Read::Missing => bevel_core::config::DEFAULT_FILE.to_string(),
        Read::Unreadable(why) => return Err(kept(&why)),
    };
    write_whole(path, f(&text).as_bytes())
}

/// The error of a write refused because the settings.txt is not Bevel95's (`change_settings`).
#[derive(Debug)]
struct NotOurs;

impl std::fmt::Display for NotOurs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the file is not Bevel95's own settings.txt; it is kept as it is")
    }
}

impl std::error::Error for NotOurs {}

/// The settings files whose refusal was logged in this program's run.
static NOT_OURS_LOGGED: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
/// The installed folder's settings files whose mark was put back in this program's run.
static MARK_BACK_LOGGED: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// Whether `e` is a save of settings.txt refused because the file is not provably Bevel95's own
/// (a portable folder's file without the mark): the bar says so on screen, once per run.
pub fn is_not_ours(e: &io::Error) -> bool {
    e.get_ref().is_some_and(|inner| inner.is::<NotOurs>())
}

fn log_once(logged: &Mutex<Vec<PathBuf>>, path: &Path, line: &str) {
    if let Ok(mut logged) = logged.lock()
        && !logged.iter().any(|p| p == path)
    {
        logged.push(path.to_path_buf());
        log(line);
    }
}

/// Whether `e` was logged once already where it came from (a read-only file, a settings.txt that
/// is not Bevel95's): callers leave it out of their own "could not save" lines.
pub fn already_logged(e: &io::Error) -> bool {
    is_read_only(e) || e.get_ref().is_some_and(|inner| inner.is::<NotOurs>())
}

/// The error of a skipped save.
fn kept(why: &str) -> io::Error {
    io::Error::other(format!("not saved, the file is kept as it is: {why}"))
}

/// The error of a save to a read-only file (`is_read_only` tells it from others). Its line is
/// logged once per file by `write_whole`; callers leave it out of their own "could not save" lines.
#[derive(Debug)]
struct ReadOnly;

impl std::fmt::Display for ReadOnly {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the file is read-only; it is kept as it is")
    }
}

impl std::error::Error for ReadOnly {}

/// Whether `e` is the error of a save to a read-only file (logged once already by `write_whole`).
pub fn is_read_only(e: &io::Error) -> bool {
    e.get_ref().is_some_and(|inner| inner.is::<ReadOnly>())
}

/// The read-only files whose refusal was logged in this program's run.
static READ_ONLY_LOGGED: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// What `write_whole` writes to for `path`: the file itself; for a symbolic link, the file it
/// points at (the link stays as it is); for a file with more than one hard link, the file in place.
#[derive(Debug, PartialEq, Eq)]
enum Destination {
    /// Under a temporary name beside it, then put in place in one step.
    Whole(PathBuf),
    /// Written in place (truncated first), as before the safe write existed: a rename would part
    /// it from its other names.
    InPlace(PathBuf),
}

fn destination(path: &Path) -> io::Result<Destination> {
    let target = match std::fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() => std::fs::canonicalize(path)?,
        _ => path.to_path_buf(),
    };
    if hard_links(&target).is_some_and(|n| n > 1) {
        return Ok(Destination::InPlace(target));
    }
    Ok(Destination::Whole(target))
}

/// How many names (hard links) the file at `path` has; None when it cannot be asked (missing).
fn hard_links(path: &Path) -> Option<u32> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle};
    let file = std::fs::File::open(path).ok()?;
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: an open file's handle and a correctly sized out-struct.
    let ok = unsafe { GetFileInformationByHandle(file.as_raw_handle() as _, &mut info) } != 0;
    ok.then_some(info.nNumberOfLinks)
}

/// Writes `bytes` as the whole of `path`: under a temporary name of this program's own beside
/// it, flushed to the disk, then put in place in one step, also through to the disk. On a
/// failure the temporary file goes and `path` is as it was. A read-only file is refused at once
/// (logged once; `is_read_only`), a symbolic link is written through to the file it points at, and
/// a file with several hard links is written in place (the second safety review of 2026-11-18, C1).
pub fn write_whole(path: &Path, bytes: &[u8]) -> io::Result<()> {
    // A run whose data folder is not known writes nothing (it is about to end: `log::decide_at_start`).
    if crate::log::not_known() {
        return Err(io::Error::other("this run's data folder is not known; nothing is written"));
    }
    write_whole_even_if_not_known(path, bytes)
}

/// `write_whole` also while this run's data folder is not known: only for the one file a
/// not-known `--restore` writes, the note of the installed copy's record it used, beside the
/// program (the sixth safety review of 2026-11-18, B1). Nothing else may use it.
pub fn write_whole_even_if_not_known(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let path = &match destination(path)? {
        Destination::Whole(path) => path,
        Destination::InPlace(path) => return write_in_place(&path, bytes),
    };
    if std::fs::metadata(path).is_ok_and(|m| m.permissions().readonly()) {
        if let Ok(mut logged) = READ_ONLY_LOGGED.lock()
            && !logged.iter().any(|p| p == path)
        {
            logged.push(path.clone());
            log(&format!("{} is read-only: Bevel95 keeps it as it is and does not save it (logged once)", path_kind(path)));
        }
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, ReadOnly));
    }
    static SERIAL: AtomicU32 = AtomicU32::new(0);
    let name = path.file_name().unwrap_or(OsStr::new("state")).to_string_lossy().into_owned();
    let (temp, mut file) = loop {
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let temp = path.with_file_name(format!("{name}.{}.{serial}.tmp", std::process::id()));
        match std::fs::File::create_new(&temp) {
            Ok(file) => break (temp, file),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    };
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    let result = written.and_then(|()| replace(&temp, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}

/// A file with several names written in place: truncated, written and flushed (a power cut in
/// that moment can leave it short, as every file was before the safe write).
fn write_in_place(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if std::fs::metadata(path).is_ok_and(|m| m.permissions().readonly()) {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, ReadOnly));
    }
    let mut file = std::fs::OpenOptions::new().write(true).truncate(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// How often, and how far apart, `replace` tries again while the file is open just now: 25 times
/// 20 ms, so a save on a window's thread waits at most about half a second (it was a second).
const REPLACE_TRIES: u32 = 25;
const REPLACE_PAUSE_MS: u64 = 20;

/// Puts `from` in place of `to` in one step, through to the disk. A reader that has the file
/// open at that moment (another Bevel95 program, a backup or antivirus program) makes Windows
/// refuse for a moment: tried again for up to about half a second. A folder where the file should be
/// fails at once (a read-only file never gets here).
fn replace(from: &Path, to: &Path) -> io::Result<()> {
    let wide = |p: &Path| p.as_os_str().encode_wide().chain([0]).collect::<Vec<u16>>();
    let (wide_from, wide_to) = (wide(from), wide(to));
    let mut tries = 0;
    loop {
        // SAFETY: two NUL-terminated paths that live through the call.
        if unsafe { MoveFileExW(wide_from.as_ptr(), wide_to.as_ptr(), MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH) } != 0 {
            return Ok(());
        }
        let e = io::Error::last_os_error();
        // ERROR_ACCESS_DENIED (5) and ERROR_SHARING_VIOLATION (32): the file is open just now.
        tries += 1;
        if !matches!(e.raw_os_error(), Some(5 | 32)) || to.is_dir() || tries >= REPLACE_TRIES {
            return Err(e);
        }
        std::thread::sleep(std::time::Duration::from_millis(REPLACE_PAUSE_MS));
    }
}

/// Clears the safe write's own leftover temporary files in `dir` (`<file>.txt.<pid>.<n>.tmp`,
/// as `write_whole` names them) older than `age`: a program ended between writing one and putting
/// it in place left it for good (the second safety review of 2026-11-18, C1). Run when a program
/// starts (`log::started`) and by Setup's Remove. Returns how many went.
pub fn clear_leftovers(dir: &Path, age: std::time::Duration) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    let mut cleared = 0;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !is_leftover_name(&name) {
            continue;
        }
        let Ok(meta) = entry.path().symlink_metadata() else { continue };
        let old = meta.modified().ok().and_then(|t| t.elapsed().ok()).is_some_and(|e| e >= age);
        if meta.is_file() && old && std::fs::remove_file(entry.path()).is_ok() {
            cleared += 1;
        }
    }
    if cleared > 0 {
        log(&format!("cleared {cleared} leftover temporary file(s) of a save in {}", path_kind(dir)));
    }
    cleared
}

/// Bevel95's own state files, the ones that go through the safe write: only their temporary names
/// are ever cleared (the third safety review of 2026-11-18, C3: any `<anything>.txt.<n>.<n>.tmp`
/// went, which in a portable copy's folder could be a file of the person's). Setup's Remove list
/// (`install::DATA_FILES`) holds each of them (a test checks it).
pub const OWN_FILES: [&str; 16] = [
    "settings.txt",
    "desktop.txt",
    "background.txt",
    "folders.txt",
    "undo.txt",
    "task-order.txt",
    "quick-launch.txt",
    "quick-search.txt",
    "programs.txt",
    "programs.old.txt",
    "minefield.txt",
    "solitaire.txt",
    "calculator.txt",
    "run-history.txt",
    "windows-taskbar-state.txt",
    "windows-taskbar-used.txt",
];

/// Whether `name` is one of `write_whole`'s temporary names for one of Bevel95's own state files:
/// `<own file>.<digits>.<digits>.tmp`.
fn is_leftover_name(name: &str) -> bool {
    let Some(rest) = name.strip_suffix(".tmp") else { return false };
    let mut parts = rest.rsplitn(3, '.');
    let (Some(serial), Some(pid), Some(file)) = (parts.next(), parts.next(), parts.next()) else { return false };
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    digits(serial) && digits(pid) && OWN_FILES.iter().any(|own| own.eq_ignore_ascii_case(file))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("bevel95-state-file-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_reader_never_sees_a_part_of_a_file() {
        let dir = scratch("whole");
        let path = dir.join("desktop.txt");
        let (a, b) = (vec![b'a'; 256 << 10], vec![b'b'; 256 << 10]);
        write_whole(&path, &a).unwrap();
        let done = std::sync::atomic::AtomicBool::new(false);
        let (mut reads, mut partial) = (0, 0);
        let mut failed = 0;
        std::thread::scope(|s| {
            let writer = s.spawn(|| {
                let failed = (0..30).filter(|i| write_whole(&path, if i % 2 == 0 { &b } else { &a }).is_err()).count();
                done.store(true, Ordering::SeqCst);
                failed
            });
            while !done.load(Ordering::SeqCst) {
                // A read may meet the file at the moment it is replaced (an error): never a part.
                if let Ok(bytes) = fs::read(&path) {
                    reads += 1;
                    if bytes != a && bytes != b {
                        partial += 1;
                    }
                }
                // A reader that lets go now and then, as a real one does (one reading without a
                // pause would keep the file for longer than the tries last).
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            failed = writer.join().unwrap();
        });
        assert!(reads > 0);
        assert_eq!(failed, 0, "every write got through a reader's open file");
        assert_eq!(partial, 0, "{partial} of {reads} reads saw a part of the file");
        let left: Vec<_> = fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(left, ["desktop.txt"], "no temporary file is left");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_failed_write_leaves_the_file_and_no_temporary_file() {
        let dir = scratch("failed");
        let path = dir.join("folders.txt");
        fs::create_dir(&path).unwrap(); // a folder where the file should be: the rename fails
        assert!(write_whole(&path, b"new").is_err());
        assert!(path.is_dir());
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        fs::remove_dir_all(dir).unwrap();
    }

    /// A read-only file is refused at once, not after a second of tries (every icon moved on the
    /// classic desktop stopped its thread for about a second: the second safety review of
    /// 2026-11-18, C1), and its refusal is logged once.
    #[test]
    #[allow(clippy::permissions_set_readonly_false)] // Windows: only the read-only attribute
    fn a_read_only_file_is_refused_at_once() {
        let dir = scratch("readonly");
        let path = dir.join("desktop.txt");
        fs::write(&path, b"kept\n").unwrap();
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&path, perms.clone()).unwrap();
        for _ in 0..3 {
            let started = std::time::Instant::now();
            let e = write_whole(&path, b"new\n").unwrap_err();
            // On the clock only on request (BEVEL95_TIMED=1): a busy PC must not fail the default
            // run (brief 104).
            if std::env::var_os("BEVEL95_TIMED").is_some() {
                assert!(started.elapsed() < std::time::Duration::from_millis(100), "{:?}", started.elapsed());
            }
            assert!(is_read_only(&e), "{e}");
        }
        assert_eq!(READ_ONLY_LOGGED.lock().unwrap().iter().filter(|p| **p == path).count(), 1, "logged once");
        assert_eq!(fs::read(&path).unwrap(), b"kept\n");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1, "no temporary file");
        perms.set_readonly(false);
        fs::set_permissions(&path, perms).unwrap();
        write_whole(&path, b"new\n").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"new\n");
        fs::remove_dir_all(dir).unwrap();
    }

    /// A file with a second name (a hard link) keeps it: written in place. A symbolic link stays a
    /// link, and the file it points at gets the text. Before, the rename put a plain file in place
    /// of the name written, and the other name (or the link's file) kept the old text.
    #[test]
    fn links_stay_links() {
        let dir = scratch("links");
        let (path, other) = (dir.join("settings.txt"), dir.join("settings copy.txt"));
        fs::write(&path, b"old\n").unwrap();
        fs::hard_link(&path, &other).unwrap();
        write_whole(&path, b"new\n").unwrap();
        assert_eq!(fs::read(&other).unwrap(), b"new\n", "the other name sees the new text");
        assert_eq!(hard_links(&path), Some(2));
        // A symbolic link needs Windows' Developer Mode or an administrator: tried when allowed.
        let (target, link) = (dir.join("kept elsewhere.txt"), dir.join("desktop.txt"));
        fs::write(&target, b"old\n").unwrap();
        if std::os::windows::fs::symlink_file(&target, &link).is_ok() {
            write_whole(&link, b"new\n").unwrap();
            assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink(), "still a link");
            assert_eq!(fs::read(&target).unwrap(), b"new\n");
        } else {
            eprintln!("links_stay_links: no symbolic link could be made here; that part was not tried");
        }
        let names: Vec<String> = fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert!(names.iter().all(|n| !n.ends_with(".tmp")), "{names:?}");
        fs::remove_dir_all(dir).unwrap();
    }

    /// The safe write's own leftovers a minute old go; younger ones, and every other name, stay.
    /// Before, nothing cleared them, so Remove left the folder behind.
    #[test]
    fn leftovers_of_a_save_are_cleared() {
        let dir = scratch("leftovers");
        let old = std::time::SystemTime::now() - std::time::Duration::from_secs(120);
        let make = |name: &str, aged: bool| {
            let p = dir.join(name);
            fs::write(&p, b"x").unwrap();
            if aged {
                fs::File::options().write(true).open(&p).unwrap().set_modified(old).unwrap();
            }
        };
        make("settings.txt.1234.0.tmp", true);
        make("desktop.txt.99.12.tmp", true);
        make("Windows-Taskbar-State.txt.7.3.tmp", true);
        make("folders.txt.1234.1.tmp", false);
        // A person's own files of the same shape stay (they went before: the third safety review
        // of 2026-11-18, C3).
        let theirs = ["notes.txt.2024.1.tmp", "Report draft.txt.3.4.tmp", "desktop.txt.bak.1.2.tmp"];
        for other in ["settings.txt", "notes.tmp", "settings.txt.12a.0.tmp", "bevel95.exe.1234.tmp", "my.doc.1.2.tmp", ".txt.1.2.tmp"].iter().chain(&theirs) {
            make(other, true);
        }
        assert_eq!(clear_leftovers(&dir, std::time::Duration::from_secs(60)), 3);
        let mut left: Vec<String> = fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        left.sort();
        let mut want = vec![".txt.1.2.tmp", "Report draft.txt.3.4.tmp", "bevel95.exe.1234.tmp", "desktop.txt.bak.1.2.tmp", "folders.txt.1234.1.tmp", "my.doc.1.2.tmp", "notes.tmp", "notes.txt.2024.1.tmp", "settings.txt", "settings.txt.12a.0.tmp"];
        want.sort();
        assert_eq!(left, want);
        fs::remove_dir_all(dir).unwrap();
    }

    /// A settings.txt that is there without Bevel95's mark is never written by `change_settings`
    /// (the third safety review of 2026-11-18, A1: `change` wrote Bevel95's keys into it once it
    /// could be read). Bevel95's own is changed; a missing one gets the defaults.
    #[test]
    fn a_settings_file_without_the_mark_is_never_written() {
        let dir = scratch("settings-mark");
        let path = dir.join("settings.txt");
        let theirs: &[u8] = b"[General]\r\nscale = 2\r\n";
        fs::write(&path, theirs).unwrap();
        for _ in 0..2 {
            let e = change_settings(&path, |t| bevel_core::config::with_setting(t, "scale", "3")).unwrap_err();
            assert!(already_logged(&e), "{e}");
        }
        assert_eq!(fs::read(&path).unwrap(), theirs, "their file byte for byte");
        assert_eq!(NOT_OURS_LOGGED.lock().unwrap().iter().filter(|p| **p == path).count(), 1, "logged once");
        // Bevel95's own: changed, everything else kept.
        fs::write(&path, bevel_core::config::DEFAULT_FILE).unwrap();
        change_settings(&path, |t| bevel_core::config::with_setting(t, "scale", "3")).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(crate::log::has_settings_mark(&text) && bevel_core::config::parse(&text).0.scale == Some(bevel_core::scale::Scale::whole(3)));
        // Missing: the defaults with the change.
        fs::remove_file(&path).unwrap();
        change_settings(&path, |t| bevel_core::config::with_setting(t, "era", "win98")).unwrap();
        assert!(crate::log::has_settings_mark(&fs::read_to_string(&path).unwrap()));
        // Unreadable: kept, as `change` does.
        let ansi: &[u8] = b"# Bevel95 settings caf\xE9\n";
        fs::write(&path, ansi).unwrap();
        assert!(change_settings(&path, |t| t.to_string()).is_err());
        assert_eq!(fs::read(&path).unwrap(), ansi);
        fs::remove_dir_all(dir).unwrap();
    }

    /// In the installed folder settings.txt is always Bevel95's own: one whose first line the
    /// person removed is saved, with the mark put back first (the fourth safety review of
    /// 2026-11-18, C1: every change was refused there for good, without a word). Elsewhere the
    /// same file is still refused, and the refusal can be told (`is_not_ours`) for the bar's notice.
    #[test]
    fn the_installed_folders_settings_are_saved_without_the_mark() {
        let installed = scratch("settings-installed");
        let path = installed.join("settings.txt");
        let tidied: &[u8] = b"era = win98\nscale = 2\n";
        fs::write(&path, tidied).unwrap();
        change_settings_in(&path, &installed, |t| bevel_core::config::with_setting(t, "scale", "3")).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.starts_with(crate::log::SETTINGS_MARK), "the mark is the first line again: {text:?}");
        let (config, _) = bevel_core::config::parse(&text);
        assert_eq!((config.scale, config.era), (Some(bevel_core::scale::Scale::whole(3)), bevel_core::theme::Era::Win98), "the change saved, the rest kept");
        // Another folder (a portable copy's): refused, the file kept, and told apart.
        let portable = scratch("settings-portable");
        let other = portable.join("settings.txt");
        fs::write(&other, tidied).unwrap();
        let e = change_settings_in(&other, &installed, |t| bevel_core::config::with_setting(t, "scale", "3")).unwrap_err();
        assert!(is_not_ours(&e) && already_logged(&e));
        assert_eq!(fs::read(&other).unwrap(), tidied);
        fs::remove_dir_all(installed).unwrap();
        fs::remove_dir_all(portable).unwrap();
    }

    #[test]
    fn what_reading_finds() {
        let dir = scratch("read");
        let path = dir.join("settings.txt");
        assert_eq!(read(&path), Read::Missing);
        fs::write(&path, b"era = win98\n").unwrap();
        assert_eq!(read(&path), Read::Text("era = win98\n".into()));
        // Notepad's "UTF-8 with BOM" and PowerShell's Set-Content -Encoding utf8.
        fs::write(&path, b"\xEF\xBB\xBFera = win98\n").unwrap();
        assert_eq!(read(&path), Read::Text("era = win98\n".into()));
        let utf16: Vec<u8> = [0xFEFFu16].into_iter().chain("era = win98\n".encode_utf16()).flat_map(u16::to_le_bytes).collect();
        fs::write(&path, &utf16).unwrap();
        assert!(matches!(read(&path), Read::Unreadable(_)));
        let no_mark: Vec<u8> = "era = win98\n".encode_utf16().flat_map(u16::to_le_bytes).collect();
        fs::write(&path, &no_mark).unwrap();
        assert!(matches!(read(&path), Read::Unreadable(_)), "UTF-16 without a mark");
        fs::write(&path, b"# caf\xE9\nera = win98\n").unwrap(); // ANSI
        assert!(matches!(read(&path), Read::Unreadable(_)));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn an_unreadable_file_is_kept_byte_for_byte() {
        let dir = scratch("kept");
        let ansi: &[u8] = b"# my notes: caf\xE9\r\nera = win2000\r\n";
        // Saved whole by a program that loaded it: skipped while it is there.
        let path = dir.join("desktop.txt");
        fs::write(&path, ansi).unwrap();
        assert!(load(&path).text().is_none(), "the defaults");
        assert!(save(&path, "places\n").is_err());
        assert_eq!(fs::read(&path).unwrap(), ansi);
        // Even once it reads again (the program's picture is still the defaults).
        fs::write(&path, b"era = win2000\n").unwrap();
        assert!(save(&path, "places\n").is_err());
        assert_eq!(fs::read(&path).unwrap(), b"era = win2000\n");
        // Gone: saved again.
        fs::remove_file(&path).unwrap();
        save(&path, "places\n").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"places\n");
        // A file that became unreadable after it was loaded is not saved over either.
        let path = dir.join("minefield.txt");
        fs::write(&path, b"[Minesweeper]\n").unwrap();
        assert!(load(&path).text().is_some());
        fs::write(&path, ansi).unwrap();
        assert!(save(&path, "x").is_err());
        assert_eq!(fs::read(&path).unwrap(), ansi);
        // Changed in place: skipped while unreadable, never "the defaults plus one setting".
        let path = dir.join("settings.txt");
        fs::write(&path, ansi).unwrap();
        assert!(change(&path, "era = win95\n", |t| format!("{t}scale = 3\n")).is_err());
        assert_eq!(fs::read(&path).unwrap(), ansi);
        fs::write(&path, b"\xEF\xBB\xBFera = win2000\n").unwrap();
        change(&path, "era = win95\n", |t| format!("{t}scale = 3\n")).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"era = win2000\nscale = 3\n");
        fs::remove_file(&path).unwrap();
        change(&path, "era = win95\n", |t| format!("{t}scale = 3\n")).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"era = win95\nscale = 3\n");
        fs::remove_dir_all(dir).unwrap();
    }
}
