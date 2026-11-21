//! Hiding the Windows taskbar while ours runs, and always putting it back.
//!
//! Hiding its window is not enough: it would still reserve its strip at the bottom of the
//! screen. So, like RetroBar (its ManagedShell library, `ExplorerHelper`), we also switch it
//! to auto-hide, which frees the strip. Auto-hide is the user's own setting, so its original
//! value is written to a file *before* it is changed, and every way out (normal exit,
//! sign-out, crash, the watchdog, `bevel95 --restore`) puts it back from that file.

use std::fs;
use std::path::{Path, PathBuf};

use bevel_winkit::state_file;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::Shell::{ABM_GETSTATE, ABM_SETSTATE, ABS_AUTOHIDE, APPBARDATA, SHAppBarMessage};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, GetPropW, HWND_TOPMOST, IsWindowVisible, SW_HIDE, SW_SHOWNA, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, SetWindowPos, ShowWindowAsync,
};

use crate::log::{Home, data_dir, log};
use crate::util::wide;

/// A window property on Bevel95's own hidden "Shell_TrayWnd" (the notification area's stand-in,
/// see tray.rs). Every Bevel95 process (the bar, the watchdog, `--restore`) uses it to tell that
/// window from Windows' taskbar, which has the same class.
pub const STAND_IN_MARK: &str = "Bevel95.StandIn";

/// The record's file name.
const STATE_FILE: &str = "windows-taskbar-state.txt";

/// Where a record of the Windows taskbar's original auto-hide state can be (its existence means
/// "Bevel95 changed the setting and has not put it back yet"): this copy's own data folder
/// (written, removed), and for a portable copy also the installed copy's folder (only read: a
/// portable copy writes nothing outside its own folder). The safety review of 2026-11-18, B2:
/// a portable copy did not see the installed copy's record and took auto-hide, switched on by
/// Bevel95, for the original.
fn places() -> Places {
    let home = crate::log::home();
    let exe = std::env::current_exe().ok();
    let exe_dir = exe.as_deref().and_then(Path::parent);
    if home != Home::NotKnown {
        // The installed folder is made here if it is missing (`data_dir`), as before.
        let _ = data_dir();
    }
    places_for(&home, exe_dir, &crate::log::installed_dir())
}

/// Where the records can be for a run whose data folder is `home` (`exe_dir`: the folder of this
/// bevel95.exe), and whether this run may change them. Known: this copy's own data folder (written
/// and removed), and for a portable copy also the installed folder (only read). **Not known** (a
/// `--restore` while the settings.txt beside the copy is held: the fifth safety review of
/// 2026-11-18, A1): both places where a record of this copy can be, beside the program and in the
/// installed folder, only read for the setting. A restore in every mode retires what it restored
/// from (the sixth review of 2026-11-18, B1: a kept record decided again later, over a setting the
/// person had made since): the record beside the program (Bevel95's own file by its name) is
/// removed, the installed copy's record is noted as used beside the program, as a portable copy
/// notes it. Nothing else is written, and no log. Without a program folder (`current_exe` failed)
/// nothing is retired: there is no place beside the program.
fn places_for(home: &Home, exe_dir: Option<&Path>, installed: &Path) -> Places {
    let (own, known, retires) = match home {
        Home::Installed => (installed.to_path_buf(), true, true),
        Home::Portable(dir) => (dir.clone(), true, true),
        Home::NotKnown => match exe_dir {
            Some(dir) => (dir.to_path_buf(), false, true),
            None => (installed.to_path_buf(), false, false),
        },
    };
    let mut list = vec![Place { path: own.join(STATE_FILE), own: true }];
    if !same_folder(&own, installed) {
        list.push(Place { path: installed.join(STATE_FILE), own: false });
    }
    Places { list, known, writes: retires }
}

/// The places of one run's records (`places_for`).
#[derive(Clone, Debug, PartialEq, Eq)]
struct Places {
    /// This copy's own place first.
    list: Vec<Place>,
    /// Whether this run's data folder is known (only then is a record written: `hide`).
    known: bool,
    /// Whether this run's restore retires the records it used (removes its own, notes the other's).
    writes: bool,
}

fn same_folder(a: &Path, b: &Path) -> bool {
    a.as_os_str().to_string_lossy().trim_end_matches('\\').eq_ignore_ascii_case(b.as_os_str().to_string_lossy().trim_end_matches('\\'))
}

/// A place where a record can be.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Place {
    path: PathBuf,
    /// This copy's own (written and removed by it); else only read.
    own: bool,
}

/// What one record says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Record {
    /// The taskbar's state (ABM_GETSTATE) before Bevel95 changed it.
    Original(u32),
    /// A file is there but says nothing usable (empty or cut short by a power cut, unreadable):
    /// Bevel95 may have changed the setting, and the original is not known.
    NotUnderstood,
}

/// A record found at a place, with when it was written (the oldest is the true original).
#[derive(Clone, Debug, PartialEq, Eq)]
struct Found {
    place: Place,
    record: Record,
    written: std::time::SystemTime,
    /// What the file holds, as the note of a used record keeps it (`used_note`).
    content: String,
}

/// The records at `places` (missing ones left out).
fn find(places: &[Place]) -> Vec<Found> {
    places
        .iter()
        .filter_map(|place| {
            let (record, content) = match state_file::read(&place.path) {
                state_file::Read::Missing => return None,
                state_file::Read::Text(text) => {
                    (text.trim().parse::<u32>().map_or(Record::NotUnderstood, Record::Original), text.trim().to_string())
                }
                state_file::Read::Unreadable(_) => (Record::NotUnderstood, "(unreadable)".to_string()),
            };
            let written = fs::metadata(&place.path).and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
            Some(Found { place: place.clone(), record, written, content })
        })
        .collect()
}

/// The note a portable copy keeps of the installed copy's record it has restored from (the
/// second safety review of 2026-11-18, B2): the record's time and content. Kept in the portable
/// folder; only ever read and written by a portable copy.
const USED_NOTE: &str = "windows-taskbar-used.txt";

/// The note's text for record `f`: when it was written (100-ns steps since 1970) and what it says.
fn used_note(f: &Found) -> String {
    let at = f.written.duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() / 100);
    format!("{at}\n{}\n", f.content)
}

/// The records that count: an installed copy's record that this portable copy has restored from
/// already (its note says so: the same time and content) no longer does. A record the installed
/// copy writes later (another time) counts again.
fn counted(found: Vec<Found>, note: Option<&str>) -> Vec<Found> {
    found.into_iter().filter(|f| f.place.own || note.is_none_or(|n| n != used_note(f))).collect()
}

/// The note's place: this copy's own folder, beside its own record (`places()[0]`).
fn note_path(places: &[Place]) -> Option<PathBuf> {
    places.first().map(|own| own.path.with_file_name(USED_NOTE))
}

/// The records that count at `places` (`find`, then `counted` with the note there).
fn find_counted(places: &[Place]) -> Vec<Found> {
    let note = note_path(places).and_then(|p| state_file::read(&p).text());
    counted(find(places), note.as_deref())
}

/// The original state the records know: the oldest understood one.
fn known_original(found: &[Found]) -> Option<u32> {
    found
        .iter()
        .filter_map(|f| match f.record {
            Record::Original(v) => Some((f.written, v)),
            Record::NotUnderstood => None,
        })
        .min_by_key(|(written, _)| *written)
        .map(|(_, v)| v)
}

/// What `hide` does about the setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HidePlan {
    /// The original is recorded already: auto-hide may be switched on.
    Recorded,
    /// No record anywhere: record the current state first (in this copy's own place).
    Record,
    /// Only records that cannot be understood: the original is not known, so the setting is
    /// left alone (the Windows taskbar then still reserves its strip).
    LeaveAlone,
}

fn hide_plan(found: &[Found]) -> HidePlan {
    if known_original(found).is_some() {
        HidePlan::Recorded
    } else if found.is_empty() {
        HidePlan::Record
    } else {
        HidePlan::LeaveAlone
    }
}

/// What `restore` does.
#[derive(Clone, Debug, PartialEq, Eq)]
enum RestorePlan {
    /// No record: Bevel95 did not change the setting.
    Nothing,
    /// Put the setting back: to the recorded original, or, when only records that cannot be
    /// understood exist, to auto-hide off (Windows' own default: what cannot leave auto-hide on
    /// by Bevel95's doing); then remove this copy's own records, and note the other copy's record
    /// that was used (`used`: the note's text), which then no longer counts for this copy.
    Restore { original: Option<u32>, remove: Vec<PathBuf>, used: Option<String> },
}

/// `writes`: this run retires the records it used (removes its own, notes the other's); when not
/// (a not-known run without a program folder), the setting is still put back by the same rule, and
/// the records stay as they are.
fn restore_plan(found: &[Found], writes: bool) -> RestorePlan {
    if found.is_empty() {
        return RestorePlan::Nothing;
    }
    let remove = found.iter().filter(|f| writes && f.place.own).map(|f| f.place.path.clone()).collect();
    let used = found.iter().find(|f| writes && !f.place.own).map(used_note);
    RestorePlan::Restore { original: known_original(found), remove, used }
}

/// Writes the record of `original` in `place` (the safe way, through to the disk) and reads
/// it back: true only when it is there for certain.
fn record(place: &Place, original: u32) -> Result<(), String> {
    state_file::write_whole(&place.path, format!("{original}\n").as_bytes()).map_err(|e| e.to_string())?;
    match find(std::slice::from_ref(place)).first().map(|f| f.record) {
        Some(Record::Original(v)) if v == original => Ok(()),
        _ => Err("it did not read back".into()),
    }
}

/// One lock around `hide` and `restore` in every Bevel95 process: the bar's threads, the
/// watchdog's restore and `--restore` (the second safety review of 2026-11-18, suspected 1: a
/// panic hook's `restore` on another thread while the main thread was inside `hide`, past its
/// check of `panic::happened`, could remove the record and put the setting back, and then `hide`
/// switched auto-hide on with no record left; the third review, C2: the lock was one process's
/// only, so an old bar's watchdog or a `--restore` could do the same to a new bar's `hide`). A
/// named mutex, as the Undo list's lock is; with it the two take turns, and `hide` looks at
/// `panic::happened` again once it holds it.
const RECORD_LOCK: &str = "Local\\Bevel95 taskbar record";
/// How long `hide` or `restore` waits for the other one: then `restore` goes on without the lock
/// (as before the lock), `hide` does nothing this time. It can never hang the panic hook.
const RECORD_WAIT: std::time::Duration = std::time::Duration::from_secs(1);

thread_local! {
    /// This thread holds `RECORD_LOCK` (a panic inside `hide` runs the hook's `restore` here).
    static HOLDING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The lock held, until dropped.
struct Held(windows_sys::Win32::Foundation::HANDLE);

impl Drop for Held {
    fn drop(&mut self) {
        HOLDING.with(|h| h.set(false));
        // SAFETY: our own handle to a mutex this thread owns.
        unsafe {
            windows_sys::Win32::System::Threading::ReleaseMutex(self.0);
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

/// Takes the named lock `name`, waiting at most `limit`. None when this thread holds it already (a
/// panic in the middle of `hide`: no wait at all), the limit ran out or the lock could not be
/// made. A lock left by a program that ended holding it (abandoned) is taken all the same: the
/// record is written whole, so it is never half there.
fn hold(name: &str, limit: std::time::Duration) -> Option<Held> {
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_ABANDONED, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{CreateMutexW, WaitForSingleObject};
    if HOLDING.with(std::cell::Cell::get) {
        return None;
    }
    let wide_name = wide(name);
    // SAFETY: a null-terminated name; the handle is closed by `Held` or here.
    unsafe {
        let h = CreateMutexW(null(), 0, wide_name.as_ptr());
        if h.is_null() {
            crate::log::log_win32("CreateMutexW(the taskbar record's lock)");
            return None;
        }
        match WaitForSingleObject(h, limit.as_millis().min(u128::from(u32::MAX - 1)) as u32) {
            WAIT_OBJECT_0 | WAIT_ABANDONED => {
                HOLDING.with(|c| c.set(true));
                Some(Held(h))
            }
            _ => {
                CloseHandle(h);
                None
            }
        }
    }
}

/// Logged once per program run: the records could not be understood.
static LEFT_ALONE_LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The main Windows taskbar window: the first "Shell_TrayWnd" that is not our stand-in.
pub fn main_tray() -> HWND {
    let class = wide("Shell_TrayWnd");
    let mark = wide(STAND_IN_MARK);
    let mut after: HWND = null_mut();
    loop {
        // SAFETY: valid null-terminated strings; `after` is null or a window this loop just found.
        unsafe {
            after = FindWindowExW(null_mut(), after, class.as_ptr(), null());
            if after.is_null() || GetPropW(after, mark.as_ptr()).is_null() {
                return after;
            }
        }
    }
}

/// Explorer's own taskbar window: the first "Shell_TrayWnd" that is not one of Bevel95's stand-ins
/// and whose process is Windows' own explorer.exe (the file in Windows' folder); another taskbar
/// program's window of that class is never taken. Nothing is sent to any window here (the windows
/// are found by class, their process asked of Windows), so a hung Explorer cannot hold it up.
/// Null when there is none.
pub fn explorer_tray() -> HWND {
    let class = wide("Shell_TrayWnd");
    let mark = wide(STAND_IN_MARK);
    let explorer = windows_explorer_exe();
    let mut after: HWND = null_mut();
    loop {
        // SAFETY: valid null-terminated strings; `after` is null or a window this loop just found.
        let (found, marked) = unsafe {
            after = FindWindowExW(null_mut(), after, class.as_ptr(), null());
            (after, !after.is_null() && !GetPropW(after, mark.as_ptr()).is_null())
        };
        if found.is_null() {
            return null_mut();
        }
        if !marked && Path::new(&crate::window_list::program_path(found).to_lowercase()) == explorer.as_path() {
            return found;
        }
    }
}

/// Windows' own explorer.exe, in Windows' folder (the system folder's parent). Compared without
/// regard to case, as Windows names files.
fn windows_explorer_exe() -> PathBuf {
    let system = PathBuf::from(bevel_winkit::shell::system_program("explorer.exe"));
    let windows = system.parent().and_then(Path::parent).map_or_else(|| PathBuf::from("C:\\Windows"), Path::to_path_buf);
    PathBuf::from(windows.join("explorer.exe").to_string_lossy().to_lowercase())
}

/// All Windows taskbar windows: the main one plus one per extra monitor.
fn trays() -> Vec<HWND> {
    let mut out = Vec::new();
    let main = main_tray();
    if !main.is_null() {
        out.push(main);
    }
    let class = wide("Shell_SecondaryTrayWnd");
    let mut after: HWND = null_mut();
    loop {
        // SAFETY: valid class name; `after` is null or a window this loop just found.
        after = unsafe { FindWindowExW(null_mut(), after, class.as_ptr(), null()) };
        if after.is_null() {
            return out;
        }
        out.push(after);
    }
}

fn appbar_data(hwnd: HWND) -> APPBARDATA {
    APPBARDATA { cbSize: size_of::<APPBARDATA>() as u32, hWnd: hwnd, ..Default::default() }
}

/// The taskbar's auto-hide/always-on-top bits (ABM_GETSTATE).
fn state(tray: HWND) -> u32 {
    let mut abd = appbar_data(tray);
    // SAFETY: a correctly sized APPBARDATA.
    unsafe { SHAppBarMessage(ABM_GETSTATE, &mut abd) as u32 }
}

fn set_state(tray: HWND, state: u32) {
    let mut abd = appbar_data(tray);
    abd.lParam = state as isize;
    // SAFETY: a correctly sized APPBARDATA.
    unsafe { SHAppBarMessage(ABM_SETSTATE, &mut abd) };
}

/// True if any Windows taskbar window is showing.
pub fn any_visible() -> bool {
    // SAFETY: IsWindowVisible accepts any handle, even a stale one.
    trays().into_iter().any(|h| unsafe { IsWindowVisible(h) } != 0)
}

/// Frees the Windows taskbar's strip (auto-hide) and hides its windows. Safe to repeat:
/// the original setting is recorded only once, before the first change.
pub fn hide() {
    // After a panic the screen has been given back and the bar is ending: it stays given back.
    if bevel_winkit::panic::happened() {
        return;
    }
    let Some(_held) = hold(RECORD_LOCK, RECORD_WAIT) else {
        log("hide: the restore of the Windows taskbar did not finish within a second; not hidden this time");
        return;
    };
    // Again, now that a restore cannot run beside this (a panic hook's restore went first).
    if bevel_winkit::panic::happened() {
        return;
    }
    let tray = main_tray();
    if tray.is_null() {
        log("hide: the Windows taskbar window was not found (is Explorer running?)");
        return;
    }
    let Places { list: places, known, .. } = places();
    if !known {
        // Never: the bar runs only once its data folder is known. Nothing is hidden then.
        return;
    }
    let found = find_counted(&places);
    let recorded = match hide_plan(&found) {
        HidePlan::Recorded => true,
        HidePlan::Record => {
            let original = state(tray);
            match record(&places[0], original) {
                Ok(()) => {
                    log(&format!("saved the Windows taskbar's original state ({original})"));
                    true
                }
                // Unrecorded, the setting could not be put back: it is left alone (the Windows
                // taskbar then still reserves its strip of the screen).
                Err(e) => {
                    log(&format!(
                        "could not save the Windows taskbar's state to {}: {e}; its auto-hide setting is left alone",
                        crate::log::path_kind(&places[0].path)
                    ));
                    false
                }
            }
        }
        HidePlan::LeaveAlone => {
            if !LEFT_ALONE_LOGGED.swap(true, std::sync::atomic::Ordering::SeqCst) {
                log("hide: the record of the Windows taskbar's original state cannot be understood; its auto-hide setting is \
                     left alone this time (the restore at the end switches auto-hide off and removes the record)");
            }
            false
        }
    };
    if recorded && state(tray) & ABS_AUTOHIDE == 0 {
        set_state(tray, ABS_AUTOHIDE);
    }
    for h in trays() {
        // SAFETY: hiding another process's window; harmless if it has gone away. Posted, so a
        // hung Explorer cannot hang us.
        unsafe { ShowWindowAsync(h, SW_HIDE) };
    }
}

/// Puts the Windows taskbar back: its original auto-hide setting (if we changed it) and its
/// windows. Never panics; safe to call any number of times, from any of our processes.
pub fn restore() {
    // Waits for a `hide` running on another thread (at most `RECORD_WAIT`, then goes on as
    // before); inside `hide` on this very thread (its panic) it goes on at once.
    let _held = hold(RECORD_LOCK, RECORD_WAIT);
    let tray = main_tray();
    let Places { list: places, writes, .. } = places();
    match restore_plan(&find_counted(&places), writes) {
        RestorePlan::Nothing => {}
        // Keep the records: whoever restores next (e.g. `--restore` later) can still use them.
        RestorePlan::Restore { .. } if tray.is_null() => {
            log("restore: the Windows taskbar window was not found; setting not restored yet");
        }
        RestorePlan::Restore { original, remove, used } => {
            match original {
                Some(original) => {
                    set_state(tray, original);
                    log(&format!("restored the Windows taskbar's original state ({original})"));
                }
                None => {
                    set_state(tray, state(tray) & !ABS_AUTOHIDE);
                    log("restore: the record of the Windows taskbar's original state could not be understood; its auto-hide \
                         setting is switched off (Windows' default), and the record is removed");
                }
            }
            for path in remove {
                let _ = fs::remove_file(path);
            }
            // The installed copy's record (which a portable copy may not remove) decides once:
            // noted, it no longer counts for this copy (the second safety review of 2026-11-18,
            // B2: it was used at the end of every portable run, over a setting made later).
            // Also when this run's data folder is not known (the sixth review, B1): beside the
            // program, the one file a not-known restore writes.
            if let (Some(note), Some(path)) = (used, note_path(&places)) {
                match state_file::write_whole_even_if_not_known(&path, note.as_bytes()) {
                    Ok(()) => log("restore: the installed copy's record was used; noted, so it no longer counts for this portable copy"),
                    Err(e) => log(&format!("restore: could not note the installed copy's record as used ({e})")),
                }
            }
        }
    }
    for h in trays() {
        // SAFETY: showing another process's window without activating it. It goes back above
        // every window, as the Windows taskbar always is (our stand-in for its notification
        // area was kept above it while ours ran). Both are posted, so a hung Explorer cannot
        // hang us (this runs in the panic hook and the watchdog too).
        unsafe {
            ShowWindowAsync(h, SW_SHOWNA);
            SetWindowPos(
                h,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A portable copy's two places in a scratch folder: its own, and the installed copy's.
    fn scratch(name: &str) -> (PathBuf, [Place; 2]) {
        let dir = std::env::temp_dir().join(format!("bevel95-taskbar-record-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("portable")).unwrap();
        fs::create_dir_all(dir.join("installed")).unwrap();
        let own = Place { path: dir.join("portable").join(STATE_FILE), own: true };
        let other = Place { path: dir.join("installed").join(STATE_FILE), own: false };
        (dir, [own, other])
    }

    #[test]
    fn the_record_is_written_whole_and_read_back() {
        let (dir, places) = scratch("write");
        assert_eq!(hide_plan(&find(&places)), HidePlan::Record, "no record anywhere");
        record(&places[0], 2).unwrap();
        assert_eq!(fs::read_to_string(&places[0].path).unwrap(), "2\n");
        assert_eq!(hide_plan(&find(&places)), HidePlan::Recorded);
        assert_eq!(
            restore_plan(&find(&places), true),
            RestorePlan::Restore { original: Some(2), remove: vec![places[0].path.clone()], used: None }
        );
        let left: Vec<_> = fs::read_dir(dir.join("portable")).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(left, [STATE_FILE], "no temporary file");
        // A record that cannot be written: nothing is recorded, so the setting is left alone.
        fs::remove_file(&places[0].path).unwrap();
        fs::create_dir(&places[0].path).unwrap();
        assert!(record(&places[0], 2).is_err());
        fs::remove_dir_all(dir).unwrap();
    }

    /// An empty or cut record (a power cut before it reached the disk) is not "recorded": `hide`
    /// leaves the setting alone and `restore` switches auto-hide off and removes it. Before, `hide`
    /// took it as recorded (and switched auto-hide on), and `restore` removed it without putting
    /// anything back: auto-hide stayed on for good.
    #[test]
    fn a_record_that_cannot_be_understood_never_leaves_auto_hide_on() {
        let (dir, places) = scratch("cut");
        let utf16: Vec<u8> = [0xFEFFu16].into_iter().chain("2\n".encode_utf16()).flat_map(u16::to_le_bytes).collect();
        for bad in [&b""[..], b"\n", b"2x", b"\0\0\0", &utf16] {
            fs::write(&places[0].path, bad).unwrap();
            let found = find(&places);
            assert_eq!(found.len(), 1);
            assert_eq!(found[0].record, Record::NotUnderstood, "{bad:?}");
            assert_eq!(hide_plan(&found), HidePlan::LeaveAlone, "{bad:?}");
            assert_eq!(restore_plan(&found, true), RestorePlan::Restore { original: None, remove: vec![places[0].path.clone()], used: None });
        }
        fs::remove_dir_all(dir).unwrap();
    }

    /// The installed copy's record is seen by a portable copy (and only read), so neither loses
    /// the other's original; the oldest record is the original.
    #[test]
    fn two_copies_keep_each_others_record() {
        let (dir, places) = scratch("two");
        // The installed copy ended hard with auto-hide switched on: its record says off (0).
        fs::write(&places[1].path, "0\n").unwrap();
        let found = find(&places);
        assert_eq!(hide_plan(&found), HidePlan::Recorded, "the portable copy records nothing new");
        let used = Some(used_note(&found[0]));
        assert_eq!(
            restore_plan(&found, true),
            RestorePlan::Restore { original: Some(0), remove: vec![], used: used.clone() },
            "and removes nothing of the other's"
        );
        // Its own record cut short: the installed copy's original still counts.
        fs::write(&places[0].path, "").unwrap();
        let found = find(&places);
        assert_eq!(hide_plan(&found), HidePlan::Recorded);
        assert_eq!(restore_plan(&found, true), RestorePlan::Restore { original: Some(0), remove: vec![places[0].path.clone()], used });
        // Two understood records (written by older builds): the older one is the original.
        fs::write(&places[0].path, "1\n").unwrap();
        let old = fs::File::options().write(true).open(&places[1].path).unwrap();
        old.set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(3600)).unwrap();
        drop(old);
        assert_eq!(known_original(&find(&places)), Some(0));
        assert_eq!(fs::read_to_string(&places[1].path).unwrap(), "0\n", "the other's record is only read");
        fs::remove_dir_all(dir).unwrap();
    }

    /// The installed copy's record decides once for a portable copy (the second safety review of
    /// 2026-11-18, B2): once the portable copy has restored from it (and noted it), it no longer
    /// counts, so the next portable run records the state the person has now and puts that back;
    /// a record the installed copy writes later counts again. Before, the old record was used at
    /// the end of every portable run, switching off an auto-hide the person had switched on since.
    #[test]
    fn the_installed_copys_record_decides_once_for_a_portable_copy() {
        let (dir, places) = scratch("once");
        let note = places[0].path.with_file_name(USED_NOTE);
        // The installed copy ended hard: its record says auto-hide was off (0).
        fs::write(&places[1].path, "0\n").unwrap();
        let old = fs::File::options().write(true).open(&places[1].path).unwrap();
        old.set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(7200)).unwrap();
        drop(old);
        // The first portable run: hides by it, restores from it, notes it.
        let found = counted(find(&places), None);
        assert_eq!(hide_plan(&found), HidePlan::Recorded);
        let RestorePlan::Restore { original: Some(0), used: Some(used), .. } = restore_plan(&found, true) else { panic!("restores 0") };
        fs::write(&note, &used).unwrap();
        // The person switches auto-hide on. The next portable run: the old record no longer
        // counts, so it records the state now (in its own place) and restores that.
        let note_text = fs::read_to_string(&note).unwrap();
        let found = counted(find(&places), Some(&note_text));
        assert!(found.is_empty(), "the used record no longer counts");
        assert_eq!(hide_plan(&found), HidePlan::Record);
        assert_eq!(restore_plan(&found, true), RestorePlan::Nothing);
        assert_eq!(fs::read_to_string(&places[1].path).unwrap(), "0\n", "the installed copy's record is only read");
        record(&places[0], ABS_AUTOHIDE).unwrap();
        let found = counted(find(&places), Some(&note_text));
        assert_eq!(restore_plan(&found, true), RestorePlan::Restore { original: Some(ABS_AUTOHIDE), remove: vec![places[0].path.clone()], used: None });
        fs::remove_file(&places[0].path).unwrap();
        // The installed copy writes a new record later (another time): it counts again.
        fs::write(&places[1].path, "0\n").unwrap();
        let fresh = fs::File::options().write(true).open(&places[1].path).unwrap();
        fresh.set_modified(std::time::SystemTime::now()).unwrap();
        drop(fresh);
        let found = counted(find(&places), Some(&note_text));
        assert_eq!(found.len(), 1, "a later record counts again");
        assert_eq!(hide_plan(&found), HidePlan::Recorded);
        // The same time but another content counts too (not the record that was used).
        let old = fs::File::options().write(true).open(&places[1].path).unwrap();
        old.set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(7200)).unwrap();
        drop(old);
        let found_now = find(&places);
        let note_of_other = format!("{}\n2\n", used_note(&found_now[0]).lines().next().unwrap());
        assert_eq!(counted(found_now, Some(&note_of_other)).len(), 1);
        fs::remove_dir_all(dir).unwrap();
    }

    /// `hide` and `restore` take turns (the second safety review of 2026-11-18, suspected 1): a
    /// restore asked for on another thread (the panic hook's) while a hide runs waits until the
    /// hide is done, so it can never be in its middle; the wait has a limit, so a hide that never
    /// ends cannot hang the panic hook; and on the thread inside the hide (a panic there) the
    /// restore goes on at once. Before, nothing made them take turns.
    #[test]
    fn hide_and_restore_take_turns_and_never_hang_the_panic_hook() {
        use std::sync::{Arc, Mutex as M};
        use std::time::{Duration, Instant};
        // A name of this test's own: the owner's bar takes the real one.
        let name = format!("Local\\Bevel95 test record lock {}", std::process::id());
        let lock: &'static str = Box::leak(name.into_boxed_str());
        let order = Arc::new(M::new(Vec::new()));
        let (o, (ready, wait)) = (order.clone(), std::sync::mpsc::channel());
        let hiding = std::thread::spawn(move || {
            let _held = hold(lock, Duration::from_secs(1)).expect("free");
            ready.send(()).unwrap();
            o.lock().unwrap().push("hide begins");
            std::thread::sleep(Duration::from_millis(300));
            o.lock().unwrap().push("hide ends");
        });
        wait.recv().unwrap();
        let started = Instant::now();
        let held = hold(lock, Duration::from_secs(1));
        order.lock().unwrap().push("restore");
        assert!(held.is_some(), "the restore got the lock once the hide was done");
        assert!(started.elapsed() >= Duration::from_millis(200), "it waited for the hide");
        hiding.join().unwrap();
        assert_eq!(*order.lock().unwrap(), ["hide begins", "hide ends", "restore"]);
        // On the same thread (a panic inside the hide): at once, without the lock.
        let started = Instant::now();
        assert!(hold(lock, Duration::from_secs(1)).is_none());
        assert!(started.elapsed() < Duration::from_millis(100));
        drop(held);
        // A hide that does not end: the restore stops waiting at the limit and goes on.
        let (ready, wait) = std::sync::mpsc::channel();
        let (stop, stopped) = std::sync::mpsc::channel::<()>();
        let stuck = std::thread::spawn(move || {
            let _held = hold(lock, Duration::from_secs(1)).expect("free");
            ready.send(()).unwrap();
            let _ = stopped.recv();
        });
        wait.recv().unwrap();
        let started = Instant::now();
        assert!(hold(lock, Duration::from_millis(300)).is_none(), "the limit ran out");
        let took = started.elapsed();
        assert!(took >= Duration::from_millis(250) && took < Duration::from_secs(2), "{took:?}");
        stop.send(()).unwrap();
        stuck.join().unwrap();
        // A lock left by a panic (poisoned) is taken all the same.
        let _ = std::thread::spawn(move || {
            let _held = hold(lock, Duration::from_secs(1));
            panic!("a test panic while holding the lock");
        })
        .join();
        assert!(hold(lock, Duration::from_millis(100)).is_some());
    }

    /// The lock is one for every Bevel95 process (the third safety review of 2026-11-18, C2: it
    /// held inside one process only, so an old bar's watchdog or a `--restore` could restore in the
    /// middle of a new bar's `hide`). Another process holding the lock by its name (Windows
    /// PowerShell, standing for the watchdog) makes this one wait, at most its limit, and get it
    /// once that process lets go. Before, the lock was a `std::sync::Mutex` of this process only.
    #[test]
    fn the_record_lock_is_shared_with_other_processes() {
        use std::time::{Duration, Instant};
        let name = format!("Local\\Bevel95 test record lock other {}", std::process::id());
        let shell = std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()))
            .join("System32\\WindowsPowerShell\\v1.0\\powershell.exe");
        let script = format!(
            "$m = [System.Threading.Mutex]::new($true, '{name}'); [Console]::Out.WriteLine('held'); [Console]::Out.Flush(); \
             Start-Sleep -Milliseconds 1500; $m.ReleaseMutex(); $m.Dispose()"
        );
        let mut child = bevel_winkit::quiet::command(shell)
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("Windows PowerShell starts");
        let mut line = String::new();
        std::io::BufRead::read_line(&mut std::io::BufReader::new(child.stdout.take().unwrap()), &mut line).unwrap();
        assert_eq!(line.trim(), "held");
        let started = Instant::now();
        assert!(hold(&name, Duration::from_millis(300)).is_none(), "the other process holds it: the limit ran out");
        assert!(started.elapsed() >= Duration::from_millis(250));
        let held = hold(&name, Duration::from_secs(10));
        assert!(held.is_some(), "it came once the other process let go");
        drop(held);
        assert!(child.wait().unwrap().success());
    }

    /// What `--restore` (and every restore) does with the record, for each state of the data
    /// folder (the fifth safety review of 2026-11-18, A1: a `--restore` that could not know its
    /// folder only asked a running bar to close). Known: its own place written and removed, the
    /// installed copy's only read. Not known: both places where a record of this copy can be are
    /// read, the setting is put back by the same rules (the oldest understood original, or
    /// auto-hide off), and what was used is retired (the sixth review of 2026-11-18, B1: it was
    /// kept, so a later run took it for a live original and switched the person's auto-hide back):
    /// the record beside the program removed, the installed copy's noted beside the program.
    #[test]
    fn a_restore_puts_the_setting_back_whatever_is_known_of_the_data_folder() {
        let (dir, _) = scratch("states");
        let (copy, installed) = (dir.join("portable"), dir.join("installed"));
        let record = |folder: &Path| folder.join(STATE_FILE);
        // The installed copy: one place, its own.
        let p = places_for(&Home::Installed, Some(&copy), &installed);
        assert_eq!(p.list, [Place { path: record(&installed), own: true }]);
        assert!(p.writes && p.known);
        // A portable copy: its own place, and the installed one only read.
        let p = places_for(&Home::Portable(copy.clone()), Some(&copy), &installed);
        assert_eq!(p.list, [Place { path: record(&copy), own: true }, Place { path: record(&installed), own: false }]);
        assert!(p.writes && p.known);
        // Not known: beside the program and in the installed folder; nothing is recorded (`hide`),
        // and what a restore used is retired.
        let p = places_for(&Home::NotKnown, Some(&copy), &installed);
        assert_eq!(p.list, [Place { path: record(&copy), own: true }, Place { path: record(&installed), own: false }]);
        assert!(p.writes && !p.known);
        assert_eq!(restore_plan(&find(&p.list), p.writes), RestorePlan::Nothing, "no record: nothing to put back");
        fs::write(record(&copy), "0\n").unwrap();
        assert_eq!(
            restore_plan(&find(&p.list), p.writes),
            RestorePlan::Restore { original: Some(0), remove: vec![record(&copy)], used: None },
            "the record beside the program is used, and removed"
        );
        fs::remove_file(record(&copy)).unwrap();
        fs::write(record(&installed), "1\n").unwrap();
        let found = find(&p.list);
        let note = used_note(&found[0]);
        assert_eq!(
            restore_plan(&found, p.writes),
            RestorePlan::Restore { original: Some(1), remove: vec![], used: Some(note.clone()) },
            "the installed copy's record is noted as used, beside the program"
        );
        assert!(counted(find(&p.list), Some(&note)).is_empty(), "noted, it no longer counts for this copy");
        fs::write(record(&copy), "").unwrap();
        fs::write(record(&installed), "").unwrap();
        let RestorePlan::Restore { original: None, remove, used: Some(_) } = restore_plan(&find(&p.list), p.writes) else {
            panic!("records that cannot be understood: auto-hide off, retired")
        };
        assert_eq!(remove, [record(&copy)]);
        // Not known and no program folder (current_exe failed): the installed folder, only read,
        // nothing retired (there is no place beside the program).
        let p = places_for(&Home::NotKnown, None, &installed);
        assert_eq!((p.list.clone(), p.writes, p.known), (vec![Place { path: record(&installed), own: true }], false, false));
        assert_eq!(restore_plan(&find(&p.list), p.writes), RestorePlan::Restore { original: None, remove: vec![], used: None });
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn an_installed_copy_has_one_place() {
        assert!(same_folder(Path::new("C:\\Users\\x\\AppData\\Local\\Bevel95"), Path::new("c:\\users\\x\\appdata\\local\\bevel95\\")));
        assert!(!same_folder(Path::new("D:\\Tools\\Bevel95"), Path::new("C:\\Users\\x\\AppData\\Local\\Bevel95")));
    }
}
