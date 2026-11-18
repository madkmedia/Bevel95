//! Undo: what the last moves, copies, renames and deletes made in Bevel95 can be taken back
//! (`bevel_core::drag::UndoList`), one list for the desktop and every folder window, as the
//! shell's in Windows. It lives in `undo.txt` in the data folder (the desktop and the folder
//! windows are two programs); each reads it when it shows or carries out Undo.
//!
//! Taking a step back goes through Windows' file machinery like everything else: a move goes
//! back (FO_MOVE), a rename is renamed back (FO_RENAME), a copy goes to the Recycle Bin after the
//! look's question ("Are you sure you want to delete 'x'?", as the Reference PCs asked), and a
//! delete comes back out of the Recycle Bin by the Recycle Bin's own Restore. A step that cannot
//! be taken back (an item is gone, or its old place is taken) changes nothing: the look's message
//! says so, and the step leaves the list.
//!
//! Undo takes back only what it can prove (the outside review of 2026-11-17): a step holds only
//! what `file_ops` proved its operation made, an operation of which nothing could be recorded
//! empties the list (`Recorded::Unproven`) unless it provably did nothing (`Recorded::Nothing`:
//! the list stays), and Undo Copy sends to the Recycle Bin only copies still
//! exactly as they were made, its proofs read on a thread of their own (`undo_off_thread`: the
//! window never waits). The two programs share one lock for the list (a named mutex of the
//! session), and the file is written under another name and then put in place, so it is never
//! read half written.

use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};

use bevel_core::drag::{Made, Proof, Recorded, Step, UndoList};
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::System::Threading::{
    CreateEventW, CreateMutexW, EVENT_MODIFY_STATE, OpenEventW, OpenMutexW, ReleaseMutex, ResetEvent, SYNCHRONIZATION_SYNCHRONIZE, SetEvent,
    WaitForSingleObject,
};
use windows_sys::Win32::UI::Shell::Common::{ITEMIDLIST, STRRET};
use windows_sys::Win32::UI::Shell::{
    CMF_NORMAL, CMINVOKECOMMANDINFO, CSIDL_BITBUCKET, SHCONTF_FOLDERS, SHCONTF_INCLUDEHIDDEN, SHCONTF_NONFOLDERS, SHGDN_FORPARSING,
    SHGetDesktopFolder, SHGetFolderLocation, StrRetToBufW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{CreatePopupMenu, DestroyMenu, HMENU};
use windows_sys::core::{GUID, HRESULT};

use crate::com::{method, release};
use crate::file_ops::{self, log};

/// The lock both programs (and all their threads) take around every change of undo.txt: one per
/// copy, with the copy's fingerprint (`copy::name`), since each copy has its own undo.txt (the
/// fifth safety review of 2026-11-18, B1).
const LOCK_NAME: &str = "Local\\Bevel95 undo list";
/// How long a change waits for the lock before it gives up (and is not recorded).
const LOCK_WAIT_MS: u32 = 3000;
/// How long showing whether Undo is there (`last`, on the window's thread) waits for the lock to
/// pay an emptying the list is owed: only then, and briefly (every change holds it for
/// milliseconds).
const SETTLE_WAIT_MS: u32 = 200;

/// The shared lock, held until dropped.
struct Locked(HANDLE);

impl Drop for Locked {
    fn drop(&mut self) {
        // SAFETY: our own handle to a mutex this thread owns.
        unsafe {
            ReleaseMutex(self.0);
            CloseHandle(self.0);
        }
    }
}

/// Takes the shared lock, waiting up to `LOCK_WAIT_MS` (None: it could not be had).
fn lock() -> Option<Locked> {
    lock_within(LOCK_WAIT_MS)
}

/// Takes the shared lock, waiting up to `wait_ms`.
fn lock_within(wait_ms: u32) -> Option<Locked> {
    let name = crate::util::wide(&crate::copy::name(LOCK_NAME));
    // SAFETY: a valid name; the handle is closed by `Locked` or here.
    unsafe {
        let h = CreateMutexW(null(), 0, name.as_ptr());
        if h.is_null() {
            crate::log::log_win32("undo: the list's lock could not be made");
            return None;
        }
        match WaitForSingleObject(h, wait_ms) {
            // Abandoned: a program ended while holding it; the file is still whole (put in place
            // in one go), so it is ours now.
            WAIT_OBJECT_0 | WAIT_ABANDONED => Some(Locked(h)),
            _ => {
                CloseHandle(h);
                None
            }
        }
    }
}

fn file() -> PathBuf {
    crate::log::data_dir().join("undo.txt")
}

/// The list in `path`: empty when there is none, `None` when the file is there but cannot be
/// read (it is then never written over: `state_file`).
fn read(path: &Path) -> Option<UndoList> {
    match crate::state_file::read(path) {
        crate::state_file::Read::Text(text) => Some(UndoList::parse(&text)),
        crate::state_file::Read::Missing => Some(UndoList::default()),
        crate::state_file::Read::Unreadable(_) => None,
    }
}

/// Writes the list whole (under a name of this program's own, then put in place), with a line
/// naming this save alone (`Stamp`; the list's reader skips it, as every line it does not know).
fn write(path: &Path, list: &UndoList) -> std::io::Result<()> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SAVES: AtomicU64 = AtomicU64::new(0);
    let now = nanos(Ok(std::time::SystemTime::now()));
    let save = format!("{}.{now}.{}", std::process::id(), SAVES.fetch_add(1, Ordering::Relaxed));
    let text = format!("{}{SAVE_LINE}{save}\n", list.to_text());
    crate::state_file::write_whole(path, text.as_bytes())
}

/// The line of undo.txt that names its save (this program's process, the time, a count).
const SAVE_LINE: &str = "saved\t";

/// Whether a change of the list reached the disk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Saved {
    Yes,
    /// Nothing was changed, so nothing was written.
    Unchanged,
    /// It could not be saved: the lock could not be had, the list could not be read, or the
    /// writing failed.
    No,
}

/// Changes the list in `path` inside the shared lock: `f` changes it and says whether it
/// changed; `write` saves it. `f` runs only on a list that was read (None when the lock could
/// not be had or the file is there but cannot be read: nothing is changed then). With
/// `empty_if_not_saved`, a list that cannot be saved is removed while the lock is still held
/// (`empty`), so no other program's write can come between (an outside review of 2026-11-18: a
/// failed write was logged and the list went on as if saved).
fn change_with<R>(
    path: &Path,
    write: impl FnOnce(&Path, &UndoList) -> std::io::Result<()>,
    empty_if_not_saved: bool,
    f: impl FnOnce(&mut UndoList) -> (R, bool),
) -> (Option<R>, Saved) {
    let Some(_locked) = lock() else {
        log("undo: the list's lock could not be had in time; this change of the list was not made");
        if empty_if_not_saved {
            // The holder may never let go: removing the file is one step and destroys nothing.
            empty(path);
        }
        return (None, Saved::No);
    };
    let Some(mut list) = read(path) else {
        log("undo: undo.txt cannot be read; this change of the list was not made");
        if empty_if_not_saved {
            empty(path);
        }
        return (None, Saved::No);
    };
    let (r, changed) = f(&mut list);
    if !changed {
        return (Some(r), Saved::Unchanged);
    }
    match write(path, &list) {
        Ok(()) => (Some(r), Saved::Yes),
        Err(e) => {
            log(&format!("undo.txt could not be written: {e}"));
            if empty_if_not_saved {
                empty(path);
            }
            (Some(r), Saved::No)
        }
    }
}

/// `change_with` with the real write, the list kept as it is when it cannot be saved.
#[cfg(test)]
fn change<R>(path: &Path, f: impl FnOnce(&mut UndoList) -> (R, bool)) -> Option<R> {
    change_with(path, write, false, f).0
}

/// The list is emptied because it could not be saved: undo.txt is removed. If that fails too,
/// the list on the disk cannot be changed, and Undo is unavailable for as long as that lasts:
/// `last` asks the disk each time (`can_change`), and taking a step back saves the list first
/// and does nothing when it cannot (`take_last`). The list is then owed its emptying (`owe`),
/// which is paid as soon as it can be, before anything else is done with it (`settle`), so an
/// older step never comes back in reach once the disk lets the file be changed again.
fn empty(path: &Path) -> bool {
    match remove_list(path) {
        Ok(true) => {
            log("undo: the list could not be saved, so undo.txt was removed: Undo has nothing to take back");
            true
        }
        Ok(false) => true,
        Err(e) => {
            log(&format!(
                "undo: the list could neither be saved nor removed ({e}); Undo is unavailable while undo.txt cannot be changed, and the list is emptied as soon as it can be"
            ));
            false
        }
    }
}

/// Removes undo.txt (Ok(true): removed, Ok(false): there was none); the list then holds nothing,
/// so an emptying owed to it is paid. When it cannot be removed, the emptying is owed.
fn remove_list(path: &Path) -> std::io::Result<bool> {
    match std::fs::remove_file(path) {
        Ok(()) => {
            paid(path);
            Ok(true)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            paid(path);
            Ok(false)
        }
        Err(e) => {
            owe(path);
            Err(e)
        }
    }
}

// ---------------------------------------------------------------------------------------------
// An emptying owed to the list.
//
// A list that could be neither saved nor removed still holds older steps that the operation
// after them made unreachable; while the disk does not let the file be changed Undo is grayed
// (`can_change`), and once it does, those steps would be in reach again. So the program that
// could not empty it remembers that for its run, and whoever does anything with the list next
// pays it first: before showing whether Undo is there (`last`), before an Undo (`take_last`),
// before the next record (`record_with`), before putting Undo Copy's step back (`put_back`).
// The debt is a named event of the session per list (unsignaled while it is owed, set once paid),
// so the other program (the desktop's or the folder windows') sees it and pays it too; this
// program holds the event open for its run. An event that could not be made is remembered in
// this program's memory only (it then pays it itself, at the latest).

struct Debt {
    name: String,
    /// The event, held open so the debt lasts as long as this program; None when it could not be
    /// made (then only this program knows).
    _event: Option<std::os::windows::io::OwnedHandle>,
    known_only_here: bool,
}

static DEBTS: std::sync::Mutex<Vec<Debt>> = std::sync::Mutex::new(Vec::new());

fn debts() -> std::sync::MutexGuard<'static, Vec<Debt>> {
    DEBTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The debt's name for the list in `path`: one per list (its folder as Windows resolves it, so
/// both programs' spellings meet), in this session.
fn debt_name(path: &Path) -> String {
    let dir = path.parent().and_then(|d| std::fs::canonicalize(d).ok()).unwrap_or_else(|| path.parent().unwrap_or(path).to_path_buf());
    let text = dir.join(path.file_name().unwrap_or_default()).to_string_lossy().to_lowercase();
    // FNV-1a: a short, stable name.
    let hash = text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3));
    format!("Local\\Bevel95 undo owed {hash:016x}")
}

/// The list in `path` is owed an emptying (it could be neither saved nor removed).
fn owe(path: &Path) {
    let name = debt_name(path);
    let w = crate::util::wide(&name);
    // SAFETY: a null-terminated name; the handle is owned by the debt or never made.
    let event = unsafe {
        let h = CreateEventW(null(), 1, 0, w.as_ptr());
        if h.is_null() {
            crate::log::log_win32("CreateEventW(the Undo list's debt); only this program remembers it");
            None
        } else {
            if GetLastError() == ERROR_ALREADY_EXISTS {
                ResetEvent(h);
            }
            use std::os::windows::io::FromRawHandle;
            Some(std::os::windows::io::OwnedHandle::from_raw_handle(h))
        }
    };
    let mut debts = debts();
    debts.retain(|d| d.name != name);
    let known_only_here = event.is_none();
    debts.push(Debt { name, _event: event, known_only_here });
}

/// Whether the list in `path` is owed an emptying, by this program or the other.
fn owed(path: &Path) -> bool {
    let name = debt_name(path);
    if debts().iter().any(|d| d.name == name && d.known_only_here) {
        return true;
    }
    let w = crate::util::wide(&name);
    // SAFETY: a null-terminated name; a handle opened is closed at once.
    let owed = unsafe {
        let h = OpenEventW(SYNCHRONIZATION_SYNCHRONIZE, 0, w.as_ptr());
        if h.is_null() {
            return false;
        }
        let owed = WaitForSingleObject(h, 0) == WAIT_TIMEOUT;
        CloseHandle(h);
        owed
    };
    if !owed {
        // Paid (by the other program, perhaps): this program's hold on it goes.
        debts().retain(|d| d.name != name);
    }
    owed
}

/// The list in `path` holds nothing older now: its debt, if any, is paid (for both programs).
fn paid(path: &Path) {
    let name = debt_name(path);
    let w = crate::util::wide(&name);
    // SAFETY: a null-terminated name; a handle opened is closed at once.
    unsafe {
        let h = OpenEventW(EVENT_MODIFY_STATE, 0, w.as_ptr());
        if !h.is_null() {
            SetEvent(h);
            CloseHandle(h);
        }
    }
    debts().retain(|d| d.name != name);
}

/// Pays the emptying the list in `path` is owed, if it is: undo.txt is removed inside the lock
/// (waiting up to `wait_ms` for it). True when nothing is owed now; false when the debt stands
/// (the file still cannot be removed, or the lock could not be had): then nothing may be taken
/// from the list.
fn settle(path: &Path, wait_ms: u32) -> bool {
    if !owed(path) {
        return true;
    }
    let Some(_locked) = lock_within(wait_ms) else {
        log("undo: the list is owed an emptying, and its lock could not be had now; Undo is unavailable");
        return false;
    };
    if !owed(path) {
        return true;
    }
    match remove_list(path) {
        Ok(_) => {
            log("undo: the emptying the list was owed is paid: undo.txt was removed");
            true
        }
        Err(_) => false,
    }
}

/// Whether the list in `path` can be changed now, asked of the disk: not when undo.txt is
/// read-only or cannot be opened for writing (nothing is written by asking). A list that cannot
/// be changed is never taken back from: its step could not leave it first.
fn can_change(path: &Path) -> bool {
    match path.symlink_metadata() {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return true,
        Err(_) => return false,
        Ok(m) if m.permissions().readonly() => return false,
        Ok(_) => {}
    }
    std::fs::OpenOptions::new().write(true).open(path).is_ok()
}

/// The list as it is now (always a whole file: it is put in place in one go).
pub fn load() -> UndoList {
    read(&file()).unwrap_or_default()
}

/// What an operation leaves for Undo (`bevel_core::drag::Recorded`): its step; nothing, when it
/// provably did nothing (the list stays as it was); or an empty list, when something may have
/// happened that cannot be taken back, so that Undo never reaches past it to an older step. A
/// list that cannot be saved with it is emptied (`empty`).
pub fn record(r: Recorded) {
    record_with(&file(), r, write);
}

fn record_with(path: &Path, r: Recorded, write: impl FnOnce(&Path, &UndoList) -> std::io::Result<()>) {
    // An emptying the list is owed is paid first; if it still cannot be, the record empties the
    // list itself before its step (the older steps never come back in reach).
    let owed = !settle(path, LOCK_WAIT_MS);
    if r == Recorded::Nothing && !owed {
        return;
    }
    if r == Recorded::Unproven {
        log("undo: nothing of the operation could be recorded; the Undo list is emptied");
    }
    let (_, saved) = change_with(path, write, true, |list| {
        let mut changed = false;
        if owed && !list.steps.is_empty() {
            list.steps.clear();
            changed = true;
        }
        // An emptying is written even onto an empty list that is there, so that a step waiting
        // to be put back (Undo Copy's question, `put_back`) sees that something happened.
        changed |= match &r {
            Recorded::Step(_) => true,
            Recorded::Unproven => !list.steps.is_empty() || path.exists(),
            Recorded::Nothing => false,
        };
        list.leave(r);
        ((), changed)
    });
    if owed && saved != Saved::No {
        // The list on the disk holds nothing older than this record: the debt is paid.
        paid(path);
        log("undo: the emptying the list was owed is paid with this record");
    }
}

/// A new step at the end of the list.
pub fn push(step: Step) {
    record(Recorded::Step(step));
}

/// The step Undo would take back now; None while Undo is unavailable: an Undo is being carried
/// out (`running`, in either program) or the list cannot be changed (`can_change`).
pub fn last() -> Option<Step> {
    last_at(&file(), &running_name())
}

fn last_at(path: &Path, mark: &str) -> Option<Step> {
    // An emptying owed is paid first; the menus ask on the window's thread, so only while a debt
    // is owed (rare) the lock is waited for, and briefly (not had: Undo shows unavailable).
    if running(mark) || !settle(path, SETTLE_WAIT_MS) {
        return None;
    }
    let step = read(path)?.last().cloned()?;
    can_change(path).then_some(step)
}

/// Which save of undo.txt is on the disk: the line naming it (`write`), different for every
/// save, so a list saved meanwhile shows, also one with the same steps. (The file's times were
/// tried first: two saves within the system clock's step, about 15 ms, have the same.)
type Stamp = String;

fn stamp_of(path: &Path) -> Option<Stamp> {
    let text = std::fs::read_to_string(path).ok()?;
    text.lines().find_map(|l| l.strip_prefix(SAVE_LINE)).map(str::to_string)
}

/// The list as an Undo left it, saved without its step: what putting the step back needs.
#[derive(Debug, PartialEq, Eq)]
struct Before {
    rest: UndoList,
    stamp: Option<Stamp>,
}

/// What taking the last step off the list did.
#[derive(Debug, PartialEq, Eq)]
enum Taken {
    /// Taken off and saved: it is carried out now (the list as saved without it).
    Step(Step, Before),
    /// Nothing to take (or a test's Undo refused).
    Nothing,
    /// The list could not be saved without it: nothing is carried out, the list is as it was
    /// (the step, when it is known, for the look's "cannot be done" message).
    NotSaved(Option<Step>),
}

/// Takes the last step off the list in `path` and saves the list first (an outside review of
/// 2026-11-18: a step was carried out though the list still held it, so a later Undo did it
/// again).
fn take_last(path: &Path, only_test: bool) -> Taken {
    take_last_with(path, only_test, write)
}

fn take_last_with(path: &Path, only_test: bool, write: impl FnOnce(&Path, &UndoList) -> std::io::Result<()>) -> Taken {
    // An emptying owed is paid first; while it cannot be, nothing is taken (the steps on the disk
    // are older than an operation that could not be recorded).
    if !settle(path, LOCK_WAIT_MS) {
        log("Undo: the list is owed an emptying that cannot be made yet; nothing was taken back");
        return Taken::Nothing;
    }
    // The file's stamp right after the save, inside the lock.
    let mut stamp = None;
    let saving = |p: &Path, l: &UndoList| {
        let r = write(p, l);
        if r.is_ok() {
            stamp = stamp_of(p);
        }
        r
    };
    let (step, saved) = change_with(path, saving, false, |list| {
        // A live test's Undo takes back only a step of test items (the third safety review of
        // 2026-11-18, C4): the last step may be the person's own.
        if only_test && !list.last().is_some_and(Step::test_only) {
            log("Undo: a test's Undo was refused (the last step is not of test items)");
            return (None, false);
        }
        let step = list.pop();
        let changed = step.is_some();
        (step.map(|s| (s, list.clone())), changed)
    });
    match (step.flatten(), saved) {
        (Some((step, rest)), Saved::Yes) => Taken::Step(step, Before { rest, stamp }),
        (step, Saved::No) => {
            log("Undo: the list could not be saved without its last step; nothing was taken back");
            Taken::NotSaved(step.map(|(s, _)| s).or_else(|| read(path).and_then(|l| l.last().cloned())))
        }
        _ => Taken::Nothing,
    }
}

// ---------------------------------------------------------------------------------------------
// One Undo at a time.

/// The name of the mark "an Undo is being carried out" of this copy (both programs).
fn running_name() -> String {
    crate::copy::name("Local\\Bevel95 undo running")
}

/// The mark that an Undo is being carried out, from taking its step until its file operation has
/// returned (Undo Copy's question included): a named object of the session that exists while a
/// handle to it is open, so it is shared by the desktop's and the folder windows' programs and
/// goes with its process at the latest (an outside review of 2026-11-18: a second Undo could
/// take the next step before the first had put its items back, find them not in place and lose
/// that step for good).
pub struct UndoMark {
    _held: std::os::windows::io::OwnedHandle,
}

impl UndoMark {
    /// The mark, if no Undo is being carried out now (None: one is, or the mark could not be
    /// made: then no Undo either).
    fn take(name: &str) -> Option<UndoMark> {
        use std::os::windows::io::FromRawHandle;
        let w = crate::util::wide(name);
        // SAFETY: a null-terminated name; the handle is owned by the result or closed here.
        unsafe {
            let h = CreateMutexW(null(), 0, w.as_ptr());
            if h.is_null() {
                crate::log::log_win32("CreateMutexW(the Undo mark); no Undo is carried out");
                return None;
            }
            if GetLastError() == ERROR_ALREADY_EXISTS {
                CloseHandle(h);
                return None;
            }
            Some(UndoMark { _held: std::os::windows::io::OwnedHandle::from_raw_handle(h) })
        }
    }
}

/// Whether an Undo is being carried out now (by either program).
fn running(name: &str) -> bool {
    let w = crate::util::wide(name);
    // SAFETY: a null-terminated name; a handle opened is closed at once.
    unsafe {
        let h = OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, 0, w.as_ptr());
        if h.is_null() {
            return false;
        }
        CloseHandle(h);
        true
    }
}

fn exists(p: &str) -> bool {
    Path::new(p).symlink_metadata().is_ok()
}

/// Nanoseconds since 1970 of a time of change.
fn nanos(t: std::io::Result<std::time::SystemTime>) -> u64 {
    t.ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos() as u64)
}

/// What the item at `path` is like now (`bevel_core::drag::Proof`): a file's size and time of
/// change; a folder's number of items, total size and newest change, read through the whole
/// folder (links inside it are counted, not followed). None when nothing is there or it cannot
/// be read through.
pub fn proof_of(path: &str) -> Option<Proof> {
    let meta = Path::new(path).symlink_metadata().ok()?;
    if !meta.is_dir() {
        return Some(Proof { folder: false, items: 1, bytes: meta.len(), newest: nanos(meta.modified()) });
    }
    let mut proof = Proof { folder: true, items: 0, bytes: 0, newest: nanos(meta.modified()) };
    let mut dirs = vec![PathBuf::from(path)];
    while let Some(dir) = dirs.pop() {
        for e in std::fs::read_dir(&dir).ok()? {
            let e = e.ok()?;
            let m = e.path().symlink_metadata().ok()?;
            proof.items += 1;
            proof.newest = proof.newest.max(nanos(m.modified()));
            if m.is_dir() {
                dirs.push(e.path());
            } else {
                proof.bytes += m.len();
            }
        }
    }
    Some(proof)
}

/// Whether `step` can be taken back as things are on the disk now (Delete is checked against
/// the Recycle Bin when it is carried out).
pub fn possible(step: &Step) -> bool {
    match step {
        Step::Move(pairs) => !pairs.is_empty() && pairs.iter().all(|(now, was)| exists(now) && !exists(was)),
        Step::Rename(now, was) => exists(now) && (!exists(was) || now.eq_ignore_ascii_case(was)),
        Step::Copy(made) => made.iter().any(|m| exists(&m.path)),
        Step::Delete(was, ..) => !was.is_empty() && was.iter().all(|p| !exists(p)),
    }
}

/// What Undo does for a step that can be taken back: the work, on a thread of its own.
pub enum Plan {
    /// Carried out at once.
    Done,
    /// Undo Copy: `take` goes to the Recycle Bin once the look's question is answered Yes (each
    /// checked again first); `left` were changed since they were made and stay (the look's
    /// message says so). The Undo is still being carried out while the question is open
    /// (`asking` holds its mark).
    AskDelete { take: Vec<Made>, left: Vec<String>, asking: Asking },
    /// It cannot be done: (title, text) of the look's message.
    Failed(Step),
}

/// Undo Copy's question is open: the Undo's mark is held until it is answered. Answered Yes, the
/// mark goes on to the deletion (`delete_copies`). Answered No, or closed any other way without
/// Yes, nothing happened, so the step goes back where it was (`put_back`: the list as it was
/// before the Undo, when nothing changed it meanwhile and it can be saved; else it is emptied, as
/// it always was before 2026-11-18), then the mark goes. No question at all (every copy changed
/// since: only the message, `without_question`): the copies stay and the list is emptied.
pub struct Asking {
    mark: Option<UndoMark>,
    path: PathBuf,
    /// The step and the list as the Undo left it; None: the list is emptied.
    back: Option<(Step, Before)>,
}

impl Asking {
    /// No question is asked (every copy was changed since it was made): the copies stay, Undo
    /// could not take the step back, and the list is emptied (Undo never reaches past copies
    /// still there to an older step).
    pub fn without_question(mut self) {
        self.back = None;
    }
}

impl Drop for Asking {
    fn drop(&mut self) {
        let Some(mark) = self.mark.take() else { return };
        let path = self.path.clone();
        let back = self.back.take();
        file_ops::on_thread("undo kept", move || {
            let _mark = mark;
            match back {
                Some((step, before)) => {
                    log("Undo Copy was not carried out (No, or its question closed); its step goes back on the list");
                    put_back(&path, step, before, write);
                }
                None => {
                    log("Undo Copy was not carried out (every copy changed since); the Undo list is emptied");
                    record_with(&path, Recorded::Unproven, write);
                }
            }
        });
    }
}

/// Undo Copy's step goes back where it was, after No: only when the list is still exactly as the
/// Undo left it (the same steps in the same file, nothing owed: nothing was recorded, emptied or
/// owed meanwhile, in either program) and it can be saved. Otherwise the list is emptied as
/// before (a step put back after a newer operation could reach past it; a list that cannot be
/// saved is removed, or owed its emptying). True when it went back.
fn put_back(path: &Path, step: Step, before: Before, write: impl FnOnce(&Path, &UndoList) -> std::io::Result<()>) -> bool {
    if owed(path) {
        log("Undo Copy: the list was owed an emptying meanwhile; its step does not go back");
        record_with(path, Recorded::Unproven, write);
        return false;
    }
    let mut back = false;
    let (_, saved) = change_with(path, write, true, |list| {
        if *list == before.rest && before.stamp.is_some() && stamp_of(path) == before.stamp {
            list.steps.push(step);
            back = true;
            ((), true)
        } else {
            log("Undo Copy: the list changed while its question was open; its step does not go back, the list is emptied");
            let changed = !list.steps.is_empty();
            list.steps.clear();
            ((), changed)
        }
    });
    let back = back && saved == Saved::Yes;
    if back {
        log("Undo Copy: its step is back on the list, as before the Undo");
    }
    back
}

/// Takes the last step back (Edit > Undo, Ctrl+Z, the toolbar's Undo, the desktop's menu), on a
/// thread of its own: the list's lock and Undo Copy's proofs (a copied folder is read through,
/// which can take long for many thousands of files) never hold the window. `known` hears what
/// Undo does once that is known, on that thread (the caller takes it back to its window's thread
/// for the look's question). False when the thread could not be started (nothing was done).
pub fn undo_off_thread(known: impl FnOnce(Option<Plan>) + Send + 'static) -> bool {
    undo_at_off_thread(file(), running_name(), proof_of, known)
}

/// `undo_off_thread` on the list in `path` with the mark `mark`, with `proof` reading what an
/// item is like now.
fn undo_at_off_thread(
    path: PathBuf,
    mark: String,
    proof: impl Fn(&str) -> Option<Proof> + Send + 'static,
    known: impl FnOnce(Option<Plan>) + Send + 'static,
) -> bool {
    // A live test's command (`file_ops::for_a_test`) on the caller's thread: test items only.
    let only_test = file_ops::only_test_items();
    file_ops::on_thread("undo check", move || known(undo_at(&path, &mark, proof, only_test)))
}

/// Takes the last step of the list in `path` back, on this thread (`only_test`: only a step of a
/// live test's own items). Only one Undo at a time (the mark `mark`): another Undo meanwhile, in
/// either program, does nothing and takes nothing off the list.
fn undo_at(path: &Path, mark: &str, proof: impl Fn(&str) -> Option<Proof>, only_test: bool) -> Option<Plan> {
    let Some(mark) = UndoMark::take(mark) else {
        log("Undo: another Undo is being carried out; this one did nothing");
        return None;
    };
    let (step, before) = match take_last(path, only_test) {
        Taken::Step(step, before) => (step, before),
        Taken::Nothing => return None,
        // The list could not be saved without the step: it is not carried out.
        Taken::NotSaved(step) => return step.map(Plan::Failed),
    };
    if !possible(&step) {
        log(&format!("Undo {}: cannot be done (an item is gone or its old place is taken)", step.word()));
        return Some(Plan::Failed(step));
    }
    match step {
        Step::Copy(made) => {
            let (take, left) = bevel_core::drag::undo_copy(&made, proof);
            if !left.is_empty() {
                log(&format!("Undo Copy: {} copy(ies) changed since they were made are left", left.len()));
            }
            let back = Some((Step::Copy(made), before));
            Some(Plan::AskDelete { take, left, asking: Asking { mark: Some(mark), path: path.to_path_buf(), back } })
        }
        step => {
            let path = path.to_path_buf();
            file_ops::on_thread("undo", move || {
                let _mark = mark;
                carry_out(&path, step);
            });
            Some(Plan::Done)
        }
    }
}

/// Undo Copy after Yes: the copies still exactly as they were made go to the Recycle Bin (checked
/// once more now: the question may have waited). `any_left`: some were changed since they were
/// made and stay. Unless every copy of the step is gone afterwards, the list is emptied.
pub(crate) fn delete_copies(take: Vec<Made>, any_left: bool, mut asking: Asking) {
    let Some(mark) = asking.mark.take() else { return };
    let path = asking.path.clone();
    file_ops::on_thread("undo copy", move || {
        let _mark = mark;
        let (still, changed) = bevel_core::drag::undo_copy(&take, proof_of);
        if !changed.is_empty() {
            log(&format!("Undo Copy: {} copy(ies) changed while the question waited are left", changed.len()));
        }
        file_ops::delete_now(still.into_iter().map(|m| PathBuf::from(m.path)).collect(), false);
        let step = Step::Copy(take);
        if any_left || !taken_back(&step) {
            log("Undo Copy: not every copy went (changed, an error, or Windows' question); the Undo list is emptied");
            record_with(&path, Recorded::Unproven, write);
        }
        file_ops::changed();
    });
}

/// Whether `step` is wholly taken back, as the disk says now (an outside review of 2026-11-18:
/// after Undo's own operation, however it returned, what it did is read off the disk). A place
/// counts as free only when Windows says nothing is there, an item as there only when it can be
/// read: any doubt is "not wholly".
fn taken_back(step: &Step) -> bool {
    let (free, there) = (file_ops::free_now, file_ops::taken);
    match step {
        Step::Move(pairs) => pairs.iter().all(|(now, was)| free(now) && there(was)),
        Step::Rename(now, was) if now.eq_ignore_ascii_case(was) => file_ops::name_on_disk(Path::new(was)).as_deref() == Path::new(was).file_name().and_then(|n| n.to_str()),
        Step::Rename(now, was) => free(now) && there(was),
        Step::Delete(was, ..) => was.iter().all(|p| there(p)),
        Step::Copy(made) => made.iter().all(|m| free(&m.path)),
    }
}

/// What is left for Undo after Undo's own operation on `step`: wholly taken back, the list stays
/// as it is (its older steps are right again); anything else (Windows' error, its question
/// answered Cancel, a part done, nothing done) empties it, so Undo never reaches past a step that
/// is still partly in effect. (A step put back on the list could land after a newer one.)
fn after_undo(step: &Step) -> Recorded {
    if taken_back(step) { Recorded::Nothing } else { Recorded::Unproven }
}

fn parent_of(p: &str) -> Option<PathBuf> {
    Path::new(p).parent().map(Path::to_path_buf)
}

fn carry_out(path: &Path, step: Step) {
    match &step {
        Step::Move(pairs) => {
            // Back into each one's old folder, folder by folder.
            let mut groups: Vec<(PathBuf, Vec<PathBuf>)> = Vec::new();
            for (now, was) in pairs {
                let Some(dir) = parent_of(was) else { continue };
                match groups.iter_mut().find(|(d, _)| d == &dir) {
                    Some((_, list)) => list.push(PathBuf::from(now)),
                    None => groups.push((dir, vec![PathBuf::from(now)])),
                }
            }
            for (dir, items) in groups {
                let from: Vec<&Path> = items.iter().map(PathBuf::as_path).collect();
                file_ops::report("Undo Move", file_ops::move_into(&from, &dir));
            }
        }
        Step::Rename(now, was) => file_ops::report("Undo Rename", file_ops::rename_path(Path::new(now), Path::new(was))),
        Step::Delete(was, from, until) => restore(was, *from, *until),
        Step::Copy(_) => {}
    }
    // The Recycle Bin's Restore may finish a moment after its call returns: looked at for up to
    // 3 s before the list is emptied.
    let mut left = after_undo(&step);
    for _ in 0..30 {
        if left == Recorded::Nothing || !matches!(step, Step::Delete(..)) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
        left = after_undo(&step);
    }
    if left != Recorded::Nothing {
        log(&format!("Undo {}: not wholly taken back (Windows' error or question, or a part of it); the Undo list is emptied", step.word()));
        record_with(path, left, write);
    }
    file_ops::changed();
}

// ---------------------------------------------------------------------------------------------
// Taking deleted items out of the Recycle Bin with its own Restore.

const IID_SHELL_FOLDER: GUID = GUID::from_u128(0x000214E6_0000_0000_C000_000000000046);
const IID_CONTEXT_MENU: GUID = GUID::from_u128(0x000214E4_0000_0000_C000_000000000046);
/// IShellFolder's EnumObjects, BindToObject, GetUIObjectOf, GetDisplayNameOf; IEnumIDList's
/// Next; IContextMenu's QueryContextMenu and InvokeCommand.
const ENUM_OBJECTS: usize = 4;
const BIND_TO_OBJECT: usize = 5;
const GET_UI_OBJECT_OF: usize = 10;
const GET_DISPLAY_NAME_OF: usize = 11;
const NEXT: usize = 3;
const QUERY_CONTEXT_MENU: usize = 3;
const INVOKE_COMMAND: usize = 4;
type EnumObjects = unsafe extern "system" fn(*mut c_void, *mut c_void, u32, *mut *mut c_void) -> HRESULT;
type BindToObject = unsafe extern "system" fn(*mut c_void, *const ITEMIDLIST, *mut c_void, *const GUID, *mut *mut c_void) -> HRESULT;
type GetUiObjectOf =
    unsafe extern "system" fn(*mut c_void, *mut c_void, u32, *const *const ITEMIDLIST, *const GUID, *mut u32, *mut *mut c_void) -> HRESULT;
type GetDisplayNameOf = unsafe extern "system" fn(*mut c_void, *const ITEMIDLIST, u32, *mut STRRET) -> HRESULT;
type Next = unsafe extern "system" fn(*mut c_void, u32, *mut *mut ITEMIDLIST, *mut u32) -> HRESULT;
type QueryContextMenu = unsafe extern "system" fn(*mut c_void, HMENU, u32, u32, u32, u32) -> HRESULT;
type InvokeCommand = unsafe extern "system" fn(*mut c_void, *const CMINVOKECOMMANDINFO) -> HRESULT;

/// A Recycle Bin record ($I file): the item's original path and when it was deleted (seconds
/// since 1970). Version 1 has the path at byte 24 (260 characters), version 2 a length at 24 and
/// the path from 28; the deletion time is at 16 in both.
pub fn bin_record(bytes: &[u8]) -> Option<(String, u64)> {
    let version = u64::from_le_bytes(bytes.get(0..8)?.try_into().ok()?);
    let filetime = u64::from_le_bytes(bytes.get(16..24)?.try_into().ok()?);
    let chars = |b: &[u8]| -> String {
        let v: Vec<u16> = b.as_chunks::<2>().0.iter().map(|&c| u16::from_le_bytes(c)).take_while(|&c| c != 0).collect();
        String::from_utf16_lossy(&v)
    };
    let path = match version {
        1 => chars(bytes.get(24..)?),
        2 => {
            let len = u32::from_le_bytes(bytes.get(24..28)?.try_into().ok()?) as usize;
            chars(bytes.get(28..28 + len * 2)?)
        }
        _ => return None,
    };
    let unix = (filetime / 10_000_000).saturating_sub(11_644_473_600);
    Some((path, unix))
}

/// Whether a Recycle Bin record (its original path, deleted at `at`) is one of `was` deleted by
/// the operation that ran from `from` to `until` (two seconds' margin: the record's time and
/// ours are read by different clocks' rounding). `taken`: the paths already matched, so each
/// path is restored once even if the same path was deleted again later.
fn wanted(orig: &str, at: u64, was: &[String], from: u64, until: u64, taken: &[String]) -> bool {
    at + 2 >= from
        && at <= until + 2
        && was.iter().any(|w| w.eq_ignore_ascii_case(orig))
        && !taken.iter().any(|t| t.eq_ignore_ascii_case(orig))
}

/// The Recycle Bin's items that are `was` deleted between `from` and `until`, by their own
/// records (one each), and Restore on them.
fn restore(was: &[String], from: u64, until: u64) {
    // SAFETY: COM objects through their documented tables; every reference, item list and menu
    // made here is let go.
    unsafe {
        let mut desktop: *mut c_void = null_mut();
        if SHGetDesktopFolder(&mut desktop) < 0 || desktop.is_null() {
            log("Undo Delete: the desktop's shell folder could not be had");
            return;
        }
        let mut bin_pidl: *mut ITEMIDLIST = null_mut();
        let mut bin: *mut c_void = null_mut();
        if SHGetFolderLocation(null_mut(), CSIDL_BITBUCKET as i32, null_mut(), 0, &mut bin_pidl) >= 0 && !bin_pidl.is_null() {
            method::<BindToObject>(desktop, BIND_TO_OBJECT)(desktop, bin_pidl, null_mut(), &IID_SHELL_FOLDER, &mut bin);
            CoTaskMemFree(bin_pidl as *const c_void);
        }
        release(desktop);
        if bin.is_null() {
            log("Undo Delete: the Recycle Bin could not be opened");
            return;
        }
        let mut found: Vec<*mut ITEMIDLIST> = Vec::new();
        let mut taken: Vec<String> = Vec::new();
        let mut others: Vec<*mut ITEMIDLIST> = Vec::new();
        let mut list: *mut c_void = null_mut();
        let flags = (SHCONTF_FOLDERS | SHCONTF_NONFOLDERS | SHCONTF_INCLUDEHIDDEN) as u32;
        if method::<EnumObjects>(bin, ENUM_OBJECTS)(bin, null_mut(), flags, &mut list) >= 0 && !list.is_null() {
            loop {
                let mut child: *mut ITEMIDLIST = null_mut();
                let mut got = 0u32;
                if method::<Next>(list, NEXT)(list, 1, &mut child, &mut got) != 0 || got == 0 || child.is_null() {
                    break;
                }
                let mut name = STRRET::default();
                let mut buf = [0u16; 1024];
                let parsed = method::<GetDisplayNameOf>(bin, GET_DISPLAY_NAME_OF)(bin, child, SHGDN_FORPARSING, &mut name) >= 0
                    && StrRetToBufW(&mut name, child, buf.as_mut_ptr(), buf.len() as u32) >= 0;
                let path = if parsed {
                    let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
                    String::from_utf16_lossy(&buf[..n])
                } else {
                    String::new()
                };
                match record_of(&path).filter(|(orig, at)| wanted(orig, *at, was, from, until, &taken)) {
                    Some((orig, _)) => {
                        taken.push(orig);
                        found.push(child);
                    }
                    None => others.push(child),
                }
            }
            release(list);
        }
        for p in others {
            CoTaskMemFree(p as *const c_void);
        }
        if found.len() < was.len() {
            log(&format!("Undo Delete: {} of {} item(s) found in the Recycle Bin", found.len(), was.len()));
        }
        if !found.is_empty() {
            let children: Vec<*const ITEMIDLIST> = found.iter().map(|&p| p as *const ITEMIDLIST).collect();
            let mut menu: *mut c_void = null_mut();
            let hr = method::<GetUiObjectOf>(bin, GET_UI_OBJECT_OF)(
                bin,
                null_mut(),
                children.len() as u32,
                children.as_ptr(),
                &IID_CONTEXT_MENU,
                null_mut(),
                &mut menu,
            );
            if hr >= 0 && !menu.is_null() {
                let popup = CreatePopupMenu();
                method::<QueryContextMenu>(menu, QUERY_CONTEXT_MENU)(menu, popup, 0, 1, 0x7FFF, CMF_NORMAL);
                let verb = b"undelete\0";
                let info = CMINVOKECOMMANDINFO {
                    cbSize: size_of::<CMINVOKECOMMANDINFO>() as u32,
                    lpVerb: verb.as_ptr(),
                    nShow: 1,
                    ..Default::default()
                };
                let hr = method::<InvokeCommand>(menu, INVOKE_COMMAND)(menu, &info);
                log(&format!("Undo Delete: Restore of {} item(s): 0x{hr:08X}", found.len()));
                if !popup.is_null() {
                    DestroyMenu(popup);
                }
                release(menu);
            } else {
                log(&format!("Undo Delete: the Recycle Bin gave no menu (0x{hr:08X})"));
            }
        }
        for p in found {
            CoTaskMemFree(p as *const c_void);
        }
        release(bin);
    }
}

/// The record beside a Recycle Bin item ("...\$Rxxxx.ext" has "...\$Ixxxx.ext"), read only.
fn record_of(item: &str) -> Option<(String, u64)> {
    let p = Path::new(item);
    let name = p.file_name()?.to_string_lossy().into_owned();
    let rest = name.strip_prefix("$R")?;
    let bytes = std::fs::read(p.with_file_name(format!("$I{rest}"))).ok()?;
    bin_record(&bytes)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn recycle_bin_records_give_the_path_and_the_time() {
        let mut v2 = 2u64.to_le_bytes().to_vec();
        v2.extend(5u64.to_le_bytes());
        // 2026-01-01 00:00:00 UTC as a FILETIME.
        let unix = 1_767_225_600u64;
        v2.extend(((unix + 11_644_473_600) * 10_000_000).to_le_bytes());
        let path: Vec<u16> = "C:\\t\\Bevel95 test.txt\0".encode_utf16().collect();
        v2.extend((path.len() as u32).to_le_bytes());
        v2.extend(path.iter().flat_map(|c| c.to_le_bytes()));
        assert_eq!(bin_record(&v2), Some(("C:\\t\\Bevel95 test.txt".to_string(), unix)));
        assert_eq!(bin_record(&[0; 4]), None);
    }

    #[test]
    fn steps_that_cannot_be_taken_back() {
        let gone = "C:\\no such folder\\Bevel95 test gone.txt".to_string();
        assert!(!possible(&Step::Move(vec![(gone.clone(), "C:\\x.txt".into())])));
        assert!(!possible(&Step::Copy(vec![Made { path: gone.clone(), proof: Proof::default() }])));
        assert!(!possible(&Step::Rename(gone.clone(), "C:\\y.txt".into())));
        let here = std::env::current_exe().unwrap().to_string_lossy().into_owned();
        assert!(!possible(&Step::Delete(vec![here], 0, 0)), "something is at the old place");
    }

    #[test]
    fn a_delete_is_restored_once_and_only_from_its_own_time() {
        let was = vec!["C:\\t\\Bevel95 test a.txt".to_string()];
        assert!(wanted("c:\\t\\bevel95 test a.txt", 101, &was, 100, 103, &[]));
        assert!(!wanted("C:\\t\\Bevel95 test a.txt", 90, &was, 100, 103, &[]), "an older delete of the same name");
        assert!(!wanted("C:\\t\\Bevel95 test a.txt", 200, &was, 100, 103, &[]), "a later delete of the same name");
        assert!(!wanted("C:\\t\\Bevel95 test a.txt", 101, &was, 100, 103, &was), "each path once");
        assert!(!wanted("C:\\t\\other.txt", 101, &was, 100, 103, &[]));
    }

    /// A scratch folder of the tests' own under private\probes\undo-safety (only "Bevel95 test" items).
    pub(crate) fn scratch(name: &str) -> PathBuf {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().and_then(Path::parent).unwrap();
        let dir = root.join("private\\probes\\undo-safety\\unit").join(format!("Bevel95 test {name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A scratch folder of this job's own under private\probes\codex2\unit (only "Bevel95 test" items).
    pub(crate) fn scratch2(name: &str) -> PathBuf {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().and_then(Path::parent).unwrap();
        let dir = root.join("private\\probes\\codex2\\unit").join(format!("Bevel95 test {name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A scratch folder of this job's own under private\probes\quiet-tests\unit (only "Bevel95 test"
    /// items).
    pub(crate) fn scratch3(name: &str) -> PathBuf {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().and_then(Path::parent).unwrap();
        let dir = root.join("private\\probes\\quiet-tests\\unit").join(format!("Bevel95 test {name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn steps(path: &Path) -> Vec<Step> {
        read(path).map(|l| l.steps).unwrap_or_default()
    }

    fn rename(n: &str) -> Step {
        Step::Rename(format!("C:\\t\\Bevel95 test {n} b"), format!("C:\\t\\Bevel95 test {n} a"))
    }

    /// An outside review of 2026-11-18: a failed save of the list was logged and the list went on
    /// as if saved, so an older step stayed in reach past the operation, and an Undo whose step
    /// could not leave the list carried it out anyway. Now a record that cannot be saved empties
    /// the list; one that can neither be saved nor removed leaves Undo unavailable, decided from
    /// the disk; and Undo saves the list without its step first, or does nothing.
    #[test]
    fn a_list_that_cannot_be_saved_never_leaves_an_older_step_in_reach() {
        let dir = scratch2("cannot be saved");
        let path = dir.join("undo.txt");
        let mark = format!("Local\\Bevel95 test undo running saved {}", std::process::id());
        let put = |list: &[Step]| {
            let _ = std::fs::remove_file(&path);
            for step in list {
                change(&path, |l| {
                    l.steps.push(step.clone());
                    ((), true)
                })
                .unwrap();
            }
        };
        let failing = |_: &Path, _: &UndoList| Err(std::io::Error::other("a test's failing writer"));
        // A failing writer: the operation's step cannot be saved, so the list is emptied.
        put(&[rename("older")]);
        record_with(&path, Recorded::Step(rename("new")), failing);
        assert!(!path.exists(), "emptied: Undo never reaches the older step");
        // Undo with a failing writer: nothing taken, nothing carried out, the list as it was.
        put(&[rename("older"), rename("last")]);
        assert_eq!(take_last_with(&path, false, failing), Taken::NotSaved(Some(rename("last"))));
        assert_eq!(steps(&path), [rename("older"), rename("last")]);
        // Read-only: the record cannot save; the file is removed if Windows lets it be, and else
        // Undo is unavailable (greyed) and takes nothing, as the disk says.
        put(&[rename("older")]);
        let mut p = std::fs::metadata(&path).unwrap().permissions();
        p.set_readonly(true);
        std::fs::set_permissions(&path, p).unwrap();
        record_with(&path, Recorded::Step(rename("new")), write);
        assert_eq!(last_at(&path, &mark), None, "Undo unavailable");
        assert!(matches!(take_last(&path, false), Taken::Nothing | Taken::NotSaved(_)));
        if path.exists() {
            assert_eq!(steps(&path), [rename("older")], "kept as it was, never carried out");
            let mut p = std::fs::metadata(&path).unwrap().permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            p.set_readonly(false);
            std::fs::set_permissions(&path, p).unwrap();
        }
        // A list another program holds open (neither written nor removed): Undo is unavailable
        // while that lasts; once the disk lets it be changed, the emptying it is owed is paid
        // first (2026-11-18: before, the older step was in reach again then).
        put(&[rename("older")]);
        {
            use std::os::windows::fs::OpenOptionsExt;
            let _held = std::fs::OpenOptions::new().read(true).share_mode(1).open(&path).unwrap();
            record_with(&path, Recorded::Step(rename("new")), write);
            assert_eq!(steps(&path), [rename("older")], "neither saved nor removed");
            assert_eq!(last_at(&path, &mark), None, "Undo unavailable while it cannot be changed");
            assert_eq!(take_last(&path, false), Taken::Nothing, "and Undo takes nothing");
            assert_eq!(steps(&path), [rename("older")]);
        }
        assert_eq!(last_at(&path, &mark), None, "the disk can be changed again: the list is emptied first");
        assert!(!path.exists() && !owed(&path));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// An outside review of 2026-11-18: one Undo at a time. While an Undo is being carried out
    /// (here: Undo Copy's question is open), another Undo, on this thread or another, takes
    /// nothing off the list and Undo shows as unavailable; once the first is over, Undo can run
    /// again.
    #[test]
    fn a_second_undo_meanwhile_takes_nothing() {
        let dir = scratch2("one at a time");
        let path = dir.join("undo.txt");
        let mark = format!("Local\\Bevel95 test undo running one {}", std::process::id());
        let made = |n: &str| {
            let p = dir.join(n);
            std::fs::write(&p, n.as_bytes()).unwrap();
            let p = p.to_string_lossy().into_owned();
            Made { proof: proof_of(&p).unwrap(), path: p }
        };
        let (older, newer) = (Step::Copy(vec![made("Bevel95 test a - Copy.txt")]), Step::Copy(vec![made("Bevel95 test b - Copy.txt")]));
        for step in [older.clone(), newer.clone()] {
            change(&path, |l| {
                l.steps.push(step);
                ((), true)
            })
            .unwrap();
        }
        let Some(Plan::AskDelete { asking, .. }) = undo_at(&path, &mark, proof_of, false) else { panic!("Undo Copy's question") };
        assert!(running(&mark));
        assert_eq!(last_at(&path, &mark), None, "Undo shows as unavailable meanwhile");
        assert!(undo_at(&path, &mark, proof_of, false).is_none(), "a second Undo does nothing");
        let (p2, m2) = (path.clone(), mark.clone());
        assert!(std::thread::spawn(move || undo_at(&p2, &m2, proof_of, false).is_none()).join().unwrap(), "nor one on another thread");
        assert_eq!(steps(&path), std::slice::from_ref(&older), "and takes nothing off the list");
        // The first is answered No: the copy stays, its step goes back, the mark goes.
        drop(asking);
        wait_for_end(&mark);
        assert!(!running(&mark), "the mark went with the Undo");
        assert_eq!(steps(&path), [older, newer], "No: the list as it was before the Undo");
        assert!(UndoMark::take(&mark).is_some(), "the next Undo can run");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Waits until the Undo of `mark` is over (its work runs on a thread of its own).
    fn wait_for_end(mark: &str) {
        let t = std::time::Instant::now();
        while running(mark) && t.elapsed() < std::time::Duration::from_secs(10) {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    /// Undo Copy's question answered No (2026-11-18, the main session's brief): nothing happened,
    /// so the step goes back where it was, the list as before Ctrl+Z, but only when nothing
    /// changed the list meanwhile and it can be saved; else it is emptied as before. Before, No
    /// always left the list emptied.
    #[test]
    fn no_to_undo_copy_puts_its_step_back() {
        let dir = scratch3("no puts it back");
        let path = dir.join("undo.txt");
        let mark = format!("Local\\Bevel95 test undo running no {}", std::process::id());
        let copy = {
            let p = dir.join("Bevel95 test a - Copy.txt");
            std::fs::write(&p, b"a").unwrap();
            let p = p.to_string_lossy().into_owned();
            Step::Copy(vec![Made { proof: proof_of(&p).unwrap(), path: p }])
        };
        let put = |list: &[Step]| {
            let _ = std::fs::remove_file(&path);
            change(&path, |l| {
                l.steps = list.to_vec();
                ((), true)
            })
            .unwrap();
        };
        let ask = || match undo_at(&path, &mark, proof_of, false) {
            Some(Plan::AskDelete { asking, .. }) => asking,
            _ => panic!("Undo Copy's question"),
        };
        // No: the list as it was before the Undo, its order kept.
        put(&[rename("older"), copy.clone()]);
        drop(ask());
        wait_for_end(&mark);
        assert_eq!(steps(&path), [rename("older"), copy.clone()], "No: as before Ctrl+Z");
        // A newer operation recorded while the question was open: the step does not go back
        // after it; the list is emptied, as before.
        let asking = ask();
        record_with(&path, Recorded::Step(rename("meanwhile")), write);
        drop(asking);
        wait_for_end(&mark);
        assert!(steps(&path).is_empty(), "changed meanwhile: emptied");
        // An operation that could not be recorded meanwhile, on a list already empty without
        // the step (the same steps, but a new save): emptied too.
        put(std::slice::from_ref(&copy));
        let asking = ask();
        record_with(&path, Recorded::Unproven, write);
        drop(asking);
        wait_for_end(&mark);
        assert!(steps(&path).is_empty(), "an emptying meanwhile: the step does not come back");
        // The list cannot be saved with the step back: emptied, as before.
        put(&[rename("older"), copy.clone()]);
        let Taken::Step(step, before) = take_last(&path, false) else { panic!("the step") };
        let failing = |_: &Path, _: &UndoList| Err(std::io::Error::other("a test's failing writer"));
        assert!(!put_back(&path, step, before, failing));
        assert!(!path.exists(), "not saved: emptied");
        // No question at all (every copy changed since it was made): emptied, as before.
        put(&[rename("older"), copy.clone()]);
        ask().without_question();
        wait_for_end(&mark);
        assert!(steps(&path).is_empty(), "no question: emptied");
        // The copy itself was never touched.
        assert_eq!(std::fs::read(dir.join("Bevel95 test a - Copy.txt")).unwrap(), b"a");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A list that could be neither saved nor removed is emptied as soon as it can be (2026-11-18,
    /// the main session's brief): the program remembers it owes the emptying and pays it before
    /// anything else it does with the list. Before, once the file could be changed again the
    /// older step was back in reach. A writer that fails and then works (the file held open by
    /// "another program", so it can neither be written nor removed, then let go).
    #[test]
    fn a_list_that_could_not_be_emptied_is_emptied_as_soon_as_it_can_be() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = scratch3("owed emptying");
        let path = dir.join("undo.txt");
        let mark = format!("Local\\Bevel95 test undo running owed {}", std::process::id());
        let put = |list: &[Step]| {
            let _ = std::fs::remove_file(&path);
            change(&path, |l| {
                l.steps = list.to_vec();
                ((), true)
            })
            .unwrap();
        };
        let hold = || std::fs::OpenOptions::new().read(true).share_mode(1).open(&path).unwrap();
        let failing = |_: &Path, _: &UndoList| Err(std::io::Error::other("a test's failing writer"));
        // The record fails and the list cannot be removed: owed.
        put(&[rename("older")]);
        let held = hold();
        record_with(&path, Recorded::Step(rename("new")), failing);
        assert!(owed(&path), "the emptying is owed");
        assert_eq!(steps(&path), [rename("older")], "the file as it was");
        // While it still cannot be changed: Undo unavailable, and an Undo takes nothing.
        assert_eq!(last_at(&path, &mark), None);
        assert_eq!(take_last(&path, false), Taken::Nothing);
        assert_eq!(steps(&path), [rename("older")]);
        // It can be changed again: before showing whether Undo is there, the list is emptied.
        drop(held);
        assert_eq!(last_at(&path, &mark), None, "the older step is not in reach");
        assert!(!path.exists() && !owed(&path), "emptied, paid");
        // Paid before an Undo.
        put(&[rename("older")]);
        let held = hold();
        record_with(&path, Recorded::Step(rename("new")), failing);
        drop(held);
        assert_eq!(take_last(&path, false), Taken::Nothing, "the Undo finds the list emptied");
        assert!(!path.exists() && !owed(&path));
        // Paid before the next record: the list holds only the new step.
        put(&[rename("older")]);
        let held = hold();
        record_with(&path, Recorded::Step(rename("new")), failing);
        drop(held);
        record_with(&path, Recorded::Step(rename("next")), write);
        assert_eq!(steps(&path), [rename("next")], "nothing older than the next step");
        assert!(!owed(&path));
        // The debt is seen through its named event, as the other program sees it: once paid by
        // someone, it is not paid again (the list's new steps stay).
        put(&[rename("older")]);
        let held = hold();
        record_with(&path, Recorded::Step(rename("new")), failing);
        drop(held);
        let name = crate::util::wide(&debt_name(&path));
        // SAFETY: a null-terminated name; the handle is closed at once.
        unsafe {
            let h = OpenEventW(SYNCHRONIZATION_SYNCHRONIZE, 0, name.as_ptr());
            assert!(!h.is_null(), "the debt is a named event of the session");
            assert_eq!(WaitForSingleObject(h, 0), WAIT_TIMEOUT, "unsignaled while owed");
            CloseHandle(h);
        }
        assert!(settle(&path, LOCK_WAIT_MS));
        put(&[rename("after")]);
        assert_eq!(last_at(&path, &mark), Some(rename("after")), "paid once: a later list is kept");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// An outside review of 2026-11-18: after Undo's own operation, however it returned, what it
    /// did is read off the disk: wholly taken back leaves the list (its older steps are right
    /// again); a part (an error or Windows' question after one of two items) empties it.
    #[test]
    fn undo_reads_off_the_disk_what_its_operation_did() {
        let dir = scratch2("undo part way");
        let s = |p: &Path| p.to_string_lossy().into_owned();
        let into = dir.join("Bevel95 test into");
        std::fs::create_dir_all(&into).unwrap();
        let (a_now, b_now) = (into.join("Bevel95 test a.txt"), into.join("Bevel95 test b.txt"));
        std::fs::write(&a_now, b"a").unwrap();
        std::fs::write(&b_now, b"b").unwrap();
        let (a_was, b_was) = (dir.join("Bevel95 test a.txt"), dir.join("Bevel95 test b.txt"));
        let step = Step::Move(vec![(s(&a_now), s(&a_was)), (s(&b_now), s(&b_was))]);
        assert_eq!(after_undo(&step), Recorded::Unproven, "nothing went back");
        std::fs::rename(&a_now, &a_was).unwrap();
        assert_eq!(after_undo(&step), Recorded::Unproven, "one of two went back");
        std::fs::rename(&b_now, &b_was).unwrap();
        assert_eq!(after_undo(&step), Recorded::Nothing, "both back: the list stays");
        let copies = Step::Copy(vec![
            Made { path: s(&a_was), proof: Proof::default() },
            Made { path: s(&dir.join("Bevel95 test gone.txt")), proof: Proof::default() },
        ]);
        assert_eq!(after_undo(&copies), Recorded::Unproven, "a copy still there");
        let rename = Step::Rename(s(&b_was), s(&dir.join("Bevel95 test c.txt")));
        assert_eq!(after_undo(&rename), Recorded::Unproven, "not renamed back");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn two_writers_without_a_shared_in_process_lock_lose_no_step() {
        let dir = scratch("two writers");
        let path = dir.join("undo.txt");
        let writer = |who: &'static str| {
            let path = path.clone();
            std::thread::spawn(move || {
                for i in 0..20 {
                    let step = Step::Rename(format!("C:\\t\\{who} {i} b"), format!("C:\\t\\{who} {i} a"));
                    change(&path, |list| {
                        list.steps.push(step);
                        ((), true)
                    })
                    .expect("the lock");
                }
            })
        };
        let (a, b) = (writer("a"), writer("b"));
        a.join().unwrap();
        b.join().unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("bevel95 undo 2\n"), "a whole file");
        let list = UndoList::parse(&text);
        assert_eq!(list.steps.len(), 40, "all 40 steps are in the list");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "no temporary file is left");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A live test's Undo takes back only a step of test items; a person's last step stays in the
    /// list, untouched (the third safety review of 2026-11-18, C4: a test's Undo took back
    /// whatever came last).
    #[test]
    fn a_tests_undo_takes_back_only_a_step_of_test_items() {
        let dir = std::env::temp_dir().join(format!("bevel95-undo-test-only-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("undo.txt");
        let theirs = Step::Rename("C:\\Users\\someone\\Taxes 2026".into(), "C:\\Users\\someone\\Taxes".into());
        let test = Step::Rename("C:\\t\\Bevel95 test b".into(), "C:\\t\\Bevel95 test a".into());
        for step in [test.clone(), theirs.clone()] {
            change(&path, |list| {
                list.steps.push(step);
                ((), true)
            })
            .unwrap();
        }
        assert_eq!(take_last(&path, true), Taken::Nothing, "the person's step is last: refused");
        assert_eq!(UndoList::parse(&std::fs::read_to_string(&path).unwrap()).steps.len(), 2, "and kept");
        assert!(matches!(take_last(&path, false), Taken::Step(s, _) if s == theirs), "the person's own Undo takes it");
        assert!(matches!(take_last(&path, true), Taken::Step(s, _) if s == test), "a test's step is a test's");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unreadable_list_is_kept_as_it_is() {
        let dir = scratch("unreadable list");
        let path = dir.join("undo.txt");
        let utf16: Vec<u8> = [0xFEFFu16].into_iter().chain("bevel95 undo 2\n".encode_utf16()).flat_map(u16::to_le_bytes).collect();
        std::fs::write(&path, &utf16).unwrap();
        let seen = change(&path, |list| {
            let seen = list.steps.len();
            list.steps.push(Step::Rename("C:\\t\\b".into(), "C:\\t\\a".into()));
            (seen, true)
        });
        assert_eq!(seen, None, "the change was not made");
        assert_eq!(std::fs::read(&path).unwrap(), utf16, "and the file is as it was");
        assert_eq!(take_last(&path, false), Taken::NotSaved(None), "Undo takes nothing from it");
        assert_eq!(std::fs::read(&path).unwrap(), utf16, "and leaves it as it was");
        // An operation's step cannot be saved into it: the list is emptied (an outside review of
        // 2026-11-18), so the older steps it may hold are never reached past the operation.
        record_with(&path, Recorded::Step(Step::Rename("C:\\t\\d".into(), "C:\\t\\c".into())), write);
        assert!(!path.exists(), "removed");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Undo Copy reads its proofs (a copied folder read through) off the caller's thread (the
    /// window's), and the answer comes when it is known.
    #[test]
    fn undo_copy_reads_its_proof_off_the_callers_thread() {
        let dir = scratch("undo off thread");
        let copy = dir.join("Bevel95 test F - Copy");
        std::fs::create_dir_all(&copy).unwrap();
        std::fs::write(copy.join("Bevel95 test one.txt"), b"12").unwrap();
        let path = dir.join("undo.txt");
        let made = Made { path: copy.to_string_lossy().into_owned(), proof: proof_of(&copy.to_string_lossy()).unwrap() };
        change(&path, |list| {
            list.record(Some(Step::Copy(vec![made.clone()])));
            ((), true)
        })
        .unwrap();
        let caller = std::thread::current().id();
        let read_on = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen = read_on.clone();
        let proof = move |p: &str| {
            seen.lock().unwrap().push(std::thread::current().id());
            proof_of(p)
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let mark = format!("Local\\Bevel95 test undo running off thread {}", std::process::id());
        assert!(undo_at_off_thread(path.clone(), mark, proof, move |plan| tx.send(plan).unwrap()));
        let plan = rx.recv_timeout(std::time::Duration::from_secs(10)).expect("the answer comes");
        let threads = read_on.lock().unwrap().clone();
        assert!(!threads.is_empty() && threads.iter().all(|t| *t != caller), "the proof is read off the caller's thread");
        match plan {
            Some(Plan::AskDelete { take, left, mut asking }) => {
                asking.mark = None;
                assert_eq!(take, [made]);
                assert!(left.is_empty());
            }
            _ => panic!("Undo Copy's question"),
        }
        assert!(UndoList::parse(&std::fs::read_to_string(&path).unwrap()).steps.is_empty(), "the step left the list");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_proof_reads_a_file_and_a_whole_folder() {
        let dir = scratch("proof");
        std::fs::write(dir.join("Bevel95 test a.txt"), b"hello").unwrap();
        std::fs::create_dir_all(dir.join("Bevel95 test F\\sub")).unwrap();
        std::fs::write(dir.join("Bevel95 test F\\one.txt"), b"12").unwrap();
        std::fs::write(dir.join("Bevel95 test F\\sub\\two.txt"), b"345").unwrap();
        let file = proof_of(&dir.join("Bevel95 test a.txt").to_string_lossy()).unwrap();
        assert_eq!((file.folder, file.items, file.bytes), (false, 1, 5));
        let folder_path = dir.join("Bevel95 test F").to_string_lossy().into_owned();
        let folder = proof_of(&folder_path).unwrap();
        assert_eq!((folder.folder, folder.items, folder.bytes), (true, 3, 5));
        assert_eq!(proof_of(&folder_path), Some(folder), "the same while nothing changes");
        std::fs::write(dir.join("Bevel95 test F\\sub\\three.txt"), b"").unwrap();
        assert_ne!(proof_of(&folder_path), Some(folder), "an item added deep inside shows");
        assert_eq!(proof_of(&dir.join("Bevel95 test none").to_string_lossy()), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
