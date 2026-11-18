//! What the menu on an icon (on the classic desktop or in a folder window) does with the owner's
//! files, always through Windows' own file machinery (SHFileOperation: its progress windows, its
//! questions when a name is taken, its messages for a name it refuses, the Recycle Bin and its
//! Restore), never by a plain file call that removes or overwrites anything:
//! - Delete: to the Recycle Bin (FOF_ALLOWUNDO), after the look's own question (`confirm`), so
//!   without Windows' second one; FOF_WANTNUKEWARNING makes Windows ask again if a file cannot go
//!   to the Recycle Bin and would be deleted for good.
//! - Rename: FO_RENAME. Paste: FO_COPY or FO_MOVE into the folder shown (the owner's Desktop
//!   folder for the desktop).
//! - Cut and Copy: the files onto the clipboard as Explorer puts them there (CF_HDROP and
//!   "Preferred DropEffect"), so Explorer can paste them and the desktop pastes what Explorer
//!   copied.
//! - Create Shortcut and Paste Shortcut: a new "Shortcut to <name>.lnk" (never over a file).
//! - Properties: Windows' own Properties windows. Explore: Explorer. Empty Recycle Bin: Windows'
//!   own, after the look's question.
//!
//! Everything that can show a window of Windows' or wait for the disk runs on a thread of its
//! own, so the bar, the desktop and the folder windows never wait. Once a command has changed
//! something on the disk, the program's own hook runs (`set_hooks`: the desktop reads its folders
//! again at once, a folder window its folder).

use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};

use bevel_core::desktop::ItemKind;
use bevel_core::drag::{Effect, Planned, Recorded, Step};
use bevel_core::desktop_icons::System;
use windows_sys::Win32::Foundation::{ERROR_ALREADY_EXISTS, ERROR_FILE_EXISTS, GlobalFree, HANDLE, HGLOBAL, HWND};
use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL;
use windows_sys::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree, STGM_FAILIFTHERE,
    STGM_SHARE_EXCLUSIVE, STGM_WRITE,
};
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber, IsClipboardFormatAvailable,
    OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
};
use windows_sys::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows_sys::Win32::System::Ole::{CF_HDROP, DROPEFFECT_COPY, DROPEFFECT_NONE};
use windows_sys::Win32::UI::Shell::Common::ITEMIDLIST;
use windows_sys::Win32::UI::Shell::{
    ASSOCF_INIT_IGNOREUNKNOWN, ASSOCSTR_PROGID, AssocQueryStringW, DROPFILES, DragQueryFileW, FO_COPY, FO_DELETE, FO_MOVE, FO_RENAME, FOF_ALLOWUNDO, FOF_MULTIDESTFILES,
    FOF_NOCONFIRMATION, FOF_RENAMEONCOLLISION, FOF_WANTNUKEWARNING, HDROP, ILClone, ILFindLastID, ILFree, ILRemoveLastID,
    SEE_MASK_INVOKEIDLIST, SHCreateDataObject, SHELLEXECUTEINFOW, SHERB_NOCONFIRMATION, SHEmptyRecycleBinW,
    SHCreateStreamOnFileEx, SHFILEOPSTRUCTW, SHFileOperationW, SHMultiFileProperties, SHOP_FILEPATH, SHObjectProperties, SHParseDisplayName,
    SHQUERYRBINFO, SHQueryRecycleBinW, ShellExecuteExW, ShellExecuteW, ShellLink,
};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows_sys::core::{GUID, HRESULT, PCWSTR};

use crate::com::{method, release};
use crate::util::wide;

/// Who uses the file commands (the log's lines start with it) and what runs after a command
/// changed something on the disk.
static HOOKS: std::sync::OnceLock<(&'static str, fn())> = std::sync::OnceLock::new();

/// Sets the log's word for this program ("desktop", "folders") and the function that runs after
/// a command changed something on the disk (on the command's own thread). Once per program.
pub fn set_hooks(who: &'static str, changed: fn()) {
    let _ = HOOKS.set((who, changed));
}

fn who() -> &'static str {
    HOOKS.get().map_or("files", |h| h.0)
}

pub(crate) fn changed() {
    if let Some((_, changed)) = HOOKS.get() {
        changed();
    }
}

/// A line in the log, after the program's word.
pub(crate) fn log(line: &str) {
    crate::log::log(&format!("{}: {line}", who()));
}

/// One icon a command acts on: its id (a path, or the look's own icon's `System::id`), its name
/// as the desktop shows it, and its kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub id: String,
    pub name: String,
    pub kind: ItemKind,
}

impl Target {
    /// One of the owner's files or folders (not one of the look's own icons).
    pub fn is_file(&self) -> bool {
        matches!(self.kind, ItemKind::Folder | ItemKind::File)
    }
}

/// The shell's own name for the look's own icons (a parsing name), for Explore, Properties and
/// Create Shortcut.
fn shell_name(target: &Target) -> String {
    match target.kind {
        ItemKind::System(System::MyComputer) => String::from("::{20D04FE0-3AEA-1069-A2D8-08002B30309D}"),
        ItemKind::System(System::RecycleBin) => String::from("::{645FF040-5081-101B-9F08-00AA002F954E}"),
        ItemKind::System(System::MyDocuments) => {
            crate::shell::documents_folder().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
        }
        ItemKind::Place if target.id == CONTROL_PANEL => String::from("::{21EC2020-3AEA-1069-A2DD-08002B30309D}"),
        ItemKind::Place if target.id == PRINTERS => String::from("::{2227A280-3AEA-1069-A2DE-08002B30309D}"),
        _ => target.id.clone(),
    }
}

/// The ids of My Computer's Control Panel and Printers (opened as Windows' own).
pub const CONTROL_PANEL: &str = "::control-panel";
pub const PRINTERS: &str = "::printers";

/// What opens an item as Explorer would: its shell name (a path, or "::{CLSID}" for the shell's
/// own folders).
pub fn open_name(target: &Target) -> String {
    shell_name(target)
}

/// Runs `job` on a thread of its own with COM (the shell's windows need it). False when the
/// thread could not be started (logged). The job is counted (`jobs`) from now until its thread is
/// done, so the program does not end in the middle of it.
pub(crate) fn on_thread(name: &str, job: impl FnOnce() + Send + 'static) -> bool {
    let running = crate::jobs::Running::start();
    let spawned = std::thread::Builder::new().name(name.into()).spawn(move || {
        let _running = running;
        // SAFETY: COM for this thread.
        unsafe { CoInitializeEx(null(), COINIT_APARTMENTTHREADED as u32) };
        job();
    });
    if let Err(e) = &spawned {
        log(&format!("could not start {name}: {e}"));
    }
    spawned.is_ok()
}

/// Until it is told the job's outcome was recorded (`recorded`), a job that ends early (its
/// thread unwinds) empties the Undo list: whatever part of it was done, the list never reaches
/// past it to the step before (the safety review of 2026-11-18, A2).
struct UndoGuard {
    recorded: bool,
}

impl UndoGuard {
    fn new() -> UndoGuard {
        UndoGuard { recorded: false }
    }

    fn recorded(mut self) {
        self.recorded = true;
    }
}

impl Drop for UndoGuard {
    fn drop(&mut self) {
        if !self.recorded {
            log("a file job ended before its outcome was recorded; the Undo list is emptied");
            crate::undo::record(Recorded::Unproven);
        }
    }
}

/// A list of paths as SHFileOperation wants it: each ended by a nul, the list by another.
fn path_list<'a>(paths: impl IntoIterator<Item = &'a Path>) -> Vec<u16> {
    let mut out = Vec::new();
    for p in paths {
        out.extend(p.as_os_str().encode_wide_lossy());
        out.push(0);
    }
    out.push(0);
    out
}

trait EncodeWide {
    fn encode_wide_lossy(&self) -> Vec<u16>;
}

impl EncodeWide for std::ffi::OsStr {
    fn encode_wide_lossy(&self) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        self.encode_wide().collect()
    }
}

/// Windows' file machinery: `func` on `from` (to `to`), with `flags`. Ok(true) when it was done,
/// Ok(false) when the owner (or Windows' own question) stopped it.
fn file_operation(func: u32, from: &[&Path], to: Option<&Path>, flags: u32) -> Result<bool, i32> {
    file_operation_to(func, from, to.map(|t| vec![t]), flags)
}

/// `file_operation` with a list of destinations (with FOF_MULTIDESTFILES: one whole path for
/// each of `from`).
fn file_operation_to(func: u32, from: &[&Path], to: Option<Vec<&Path>>, flags: u32) -> Result<bool, i32> {
    let from = path_list(from.iter().copied());
    let to = to.map(path_list);
    let mut op = SHFILEOPSTRUCTW {
        hwnd: null_mut(),
        wFunc: func,
        pFrom: from.as_ptr(),
        pTo: to.as_ref().map_or(null(), |t| t.as_ptr()),
        fFlags: flags as u16,
        ..Default::default()
    };
    // SAFETY: the lists live until the call returns; no name mappings asked for.
    let r = unsafe { SHFileOperationW(&mut op) };
    if r != 0 {
        return Err(r);
    }
    Ok(op.fAnyOperationsAborted == 0)
}

/// Delete (after Yes in the look's question): the files to the Recycle Bin; Undo can bring them
/// back.
pub fn delete(targets: Vec<Target>) {
    let paths: Vec<PathBuf> = targets.iter().filter(|t| t.is_file()).map(|t| PathBuf::from(&t.id)).collect();
    delete_paths(paths, true);
}

/// The files to the Recycle Bin (`record`: as a step Undo can take back; Undo Copy's own delete
/// is not one).
pub(crate) fn delete_paths(paths: Vec<PathBuf>, record: bool) {
    if paths.is_empty() {
        return;
    }
    on_thread("desktop delete", move || delete_now(paths, record));
}

/// `delete_paths` on this thread.
pub(crate) fn delete_now(paths: Vec<PathBuf>, record: bool) {
    if paths.is_empty() {
        return;
    }
    let guard = record.then(UndoGuard::new);
    let (r, left) = delete_recorded(&paths, |from| file_operation(FO_DELETE, from, None, FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_WANTNUKEWARNING));
    match r {
        Ok(done) => log(&format!("Delete sent {} item(s) to the Recycle Bin{}", paths.len(), if done { "" } else { " (stopped)" })),
        Err(e) => log(&format!("Delete: Windows' file machinery said 0x{e:X}")),
    }
    if record {
        crate::undo::record(left);
    }
    if let Some(guard) = guard {
        guard.recorded();
    }
    changed();
}

/// Deletes `paths` with `op` and reads off the disk what it did, however it returned (an outside
/// review of 2026-11-18: after an error nothing was recorded, though Windows may have sent some
/// items to the Recycle Bin already, so Undo reached past them to the step before): the items
/// gone (Windows says nothing is there) are a step; none gone and every one still there (it can
/// be read) is nothing; else the list is emptied.
fn delete_recorded(paths: &[PathBuf], op: impl FnOnce(&[&Path]) -> Result<bool, i32>) -> (Result<bool, i32>, Recorded) {
    let from: Vec<&Path> = paths.iter().map(PathBuf::as_path).collect();
    let start = now();
    let r = op(&from);
    let as_text = |p: &PathBuf| p.to_string_lossy().into_owned();
    let gone: Vec<String> = paths.iter().map(as_text).filter(|p| free_now(p)).collect();
    let left = if !gone.is_empty() {
        Recorded::Step(Step::Delete(gone, start, now()))
    } else if paths.iter().map(as_text).all(|p| taken(&p)) {
        Recorded::Nothing
    } else {
        Recorded::Unproven
    };
    (r, left)
}

/// Seconds since 1970.
fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Moves `from` into the folder `dir` (Windows' file machinery, with its questions).
pub(crate) fn move_into(from: &[&Path], dir: &Path) -> Result<bool, i32> {
    file_operation(FO_MOVE, from, Some(dir), FOF_ALLOWUNDO)
}

/// Renames the item `old` to the whole path `new`.
pub(crate) fn rename_path(old: &Path, new: &Path) -> Result<bool, i32> {
    file_operation(FO_RENAME, &[old], Some(new), FOF_ALLOWUNDO)
}

/// A log line for what Windows' file machinery said.
pub(crate) fn report(what: &str, r: Result<bool, i32>) {
    match r {
        Ok(true) => log(&format!("{what} done")),
        Ok(false) => log(&format!("{what} stopped")),
        Err(e) => log(&format!("{what}: Windows' file machinery said 0x{e:X}")),
    }
}

/// Whether anything is at `p` (also a link whose target is gone).
pub(crate) fn taken(p: &str) -> bool {
    Path::new(p).symlink_metadata().is_ok()
}

/// Whether Windows says nothing is at `p` (any other answer is a doubt, not "free").
pub(crate) fn free_now(p: &str) -> bool {
    matches!(Path::new(p).symlink_metadata(), Err(e) if e.kind() == std::io::ErrorKind::NotFound)
}

/// The name `p` has on the disk, as its folder lists it (a rename of only the case of letters
/// changes nothing else). None when it is not there or its folder cannot be read.
pub(crate) fn name_on_disk(p: &Path) -> Option<String> {
    let want = p.file_name()?.to_string_lossy().into_owned();
    std::fs::read_dir(p.parent()?)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .find(|n| n.to_lowercase() == want.to_lowercase())
}

/// Where each of `from` goes in `dir`, decided before the operation (`bevel_core::drag::plan`).
fn plan_for(from: &[&Path], dir: &Path, copy: bool) -> Vec<Planned> {
    let sources: Vec<String> = from.iter().map(|p| p.to_string_lossy().into_owned()).collect();
    let dir_s = dir.to_string_lossy().into_owned();
    bevel_core::drag::plan(
        &sources,
        &dir_s,
        copy,
        |s| Path::new(s).parent().is_some_and(|d| same_dir(d, dir)),
        |s| Path::new(s).is_dir(),
        taken,
    )
}

/// Copies `from` into `dir` and returns what it leaves for Undo (`recorded`: its step, nothing,
/// or an empty list). A copy of an item of `dir` itself gets the name Windows would give it
/// ("a - Copy.txt"), chosen here before the copy and handed to Windows as the whole destination
/// (FOF_MULTIDESTFILES), so the copy's name is known exactly; only the planned places are
/// looked at afterwards, never the rest of the folder.
fn copy_into(from: &[&Path], dir: &Path) -> (Result<bool, i32>, Recorded) {
    copy_into_with(from, dir, |plan| {
        let new_name = |p: &Planned| !Path::new(&p.to).file_name().eq(&Path::new(&p.from).file_name());
        if plan.iter().any(new_name) {
            let to: Vec<&Path> = plan.iter().map(|p| Path::new(p.to.as_str())).collect();
            // Every item a copy in its own folder: a "- Copy" name taken by something else
            // meanwhile gets Windows' next name instead of its question to replace it (the copy
            // is then not recorded: its planned place is not it). Mixed with items from
            // elsewhere, those keep Windows' question for a taken name, as in Windows.
            let collide = if plan.iter().all(new_name) { FOF_RENAMEONCOLLISION } else { 0 };
            file_operation_to(FO_COPY, from, Some(to), FOF_ALLOWUNDO | FOF_MULTIDESTFILES | collide)
        } else {
            file_operation(FO_COPY, from, Some(dir), FOF_ALLOWUNDO)
        }
    })
}

/// `copy_into` with the copy itself done by `op` (given the plan). What it made is read off the
/// disk however `op` returned: an error after a part was copied records that part.
fn copy_into_with(from: &[&Path], dir: &Path, op: impl FnOnce(&[Planned]) -> Result<bool, i32>) -> (Result<bool, i32>, Recorded) {
    let plan = plan_for(from, dir, true);
    let r = op(&plan);
    let made = bevel_core::drag::copied(&plan, crate::undo::proof_of);
    if made.len() < plan.len() {
        log(&format!("a copy: {} of {} item(s) recorded for Undo (the others merged, replaced or were not made)", made.len(), plan.len()));
    }
    (r, recorded(&plan, (!made.is_empty()).then_some(Step::Copy(made))))
}

/// Moves `from` into `dir` and returns what it leaves for Undo: each item whose place in `dir`
/// was free before and that went there (`recorded`).
fn move_recorded(from: &[&Path], dir: &Path) -> (Result<bool, i32>, Recorded) {
    move_recorded_with(from, dir, || move_into(from, dir))
}

/// `move_recorded` with the move done by `op`; what went is read off the disk however it
/// returned.
fn move_recorded_with(from: &[&Path], dir: &Path, op: impl FnOnce() -> Result<bool, i32>) -> (Result<bool, i32>, Recorded) {
    let plan = plan_for(from, dir, false);
    let r = op();
    let pairs = bevel_core::drag::moved(&plan, taken);
    if pairs.len() < plan.len() {
        log(&format!("a move: {} of {} item(s) recorded for Undo (the others merged, replaced or did not go)", pairs.len(), plan.len()));
    }
    (r, recorded(&plan, (!pairs.is_empty()).then_some(Step::Move(pairs))))
}

/// What an operation planned as `plan` leaves for Undo (`bevel_core::drag::recorded`): a place
/// counts as still free only when Windows says nothing is there, a source as still there only
/// when it can be read (any doubt empties the list).
fn recorded(plan: &[Planned], step: Option<Step>) -> Recorded {
    let r = bevel_core::drag::recorded(plan, step, free_now, taken);
    if r == Recorded::Nothing {
        log("the operation provably did nothing (every place still free, every item where it was); the Undo list is left as it was");
    }
    r
}

/// The paths a step made (the new icons a drop selects).
fn made_by(r: &Recorded) -> Vec<String> {
    match r {
        Recorded::Step(Step::Move(pairs)) => pairs.iter().map(|p| p.0.clone()).collect(),
        Recorded::Step(Step::Copy(made)) => made.iter().map(|m| m.path.clone()).collect(),
        _ => Vec::new(),
    }
}

/// A drop of dragged files: `paths` moved, copied or linked into the folder `dest`, with Windows'
/// file machinery (its progress window, its questions for a taken name or a folder dropped into
/// itself). A move or copy becomes a step Undo can take back (a shortcut made by a drag was not
/// one, on the Reference PC). `done` gets the new items' paths.
pub fn drop_files(paths: Vec<String>, dest: PathBuf, effect: Effect, done: impl FnOnce(Vec<String>) + Send + 'static) {
    if paths.is_empty() {
        return;
    }
    if effect == Effect::Link {
        let targets: Vec<Target> = paths
            .iter()
            .map(|p| {
                let name = crate::shell::display_name(p)
                    .unwrap_or_else(|| Path::new(p).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
                let kind = if Path::new(p).is_dir() { ItemKind::Folder } else { ItemKind::File };
                Target { id: p.clone(), name, kind }
            })
            .collect();
        create_shortcuts(&targets, dest, |first| done(vec![first]));
        return;
    }
    // A drop from another program that may delete its files when its drag call returns: the
    // drop answers once this job is done (or the source is told then: `dragdrop::drop_job`).
    let told = if effect == Effect::Copy { DROPEFFECT_COPY } else { DROPEFFECT_NONE };
    let finish = crate::dragdrop::drop_job(told);
    on_thread("drop", move || {
        let _finish = finish;
        let guard = UndoGuard::new();
        let from: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
        let from: Vec<&Path> = from.iter().map(PathBuf::as_path).collect();
        let left = if effect == Effect::Move {
            let (r, left) = move_recorded(&from, &dest);
            report(&format!("A drop moved {} item(s)", from.len()), r);
            left
        } else {
            let (r, left) = copy_into(&from, &dest);
            report(&format!("A drop copied {} item(s)", from.len()), r);
            left
        };
        let made = made_by(&left);
        crate::undo::record(left);
        guard.recorded();
        changed();
        done(made);
    });
}

/// A drop on the Recycle Bin: the files go there (after the look's question in 98, which the
/// caller asks), as Delete.
pub fn drop_in_recycle_bin(paths: Vec<String>) {
    delete_paths(paths.into_iter().map(PathBuf::from).collect(), true);
}

/// The new name of a renamed file: what was typed, with the old extension added when the
/// desktop did not show it (Explorer hid it, or it is a shortcut's .lnk), as Explorer renamed.
pub fn new_file_name(old_path: &Path, old_shown: &str, typed: &str, folder: bool) -> String {
    let Some(ext) = old_path.extension().filter(|_| !folder).map(|e| format!(".{}", e.to_string_lossy())) else {
        return typed.to_string();
    };
    let lower = |s: &str| s.to_lowercase();
    if lower(old_shown).ends_with(&lower(&ext)) { typed.to_string() } else { format!("{typed}{ext}") }
}

/// Whether today's Windows knows `ext` (".txt") as a file type: it has a program identifier for
/// it (the extension's own, the owner's choice or one of its "open with" types). Windows 95 to
/// 2000 warned only for an extension whose registry key named an existing type (measured on
/// 98: an empty key, or one naming a type that is not there, gave no warning); ".dat" is no type
/// on those PCs and none on Windows 11 either.
pub fn known_type(ext: &str) -> bool {
    let ext = wide(ext);
    let mut out = [0u16; 260];
    let mut len = out.len() as u32;
    // SAFETY: valid strings and a buffer of the length given.
    let r = unsafe {
        AssocQueryStringW(ASSOCF_INIT_IGNOREUNKNOWN, ASSOCSTR_PROGID, ext.as_ptr(), null(), out.as_mut_ptr(), &mut len)
    };
    r == 0
}

/// Rename: `id` gets the name `typed` (as the desktop shows names). `done(new_id)` runs when
/// Windows renamed it; Windows' own message comes for a name it refuses or one that is taken.
pub fn rename(target: Target, typed: String, done: impl FnOnce(String) + Send + 'static) {
    // Never a path or a name Windows refuses (the windows refuse it first, with the look's
    // message, and keep the box open: `bevel_core::rename_warning::bad_name`).
    if bevel_core::rename_warning::bad_name(&typed) {
        log("Rename refused: the name holds a character a file name cannot have");
        return;
    }
    on_thread("desktop rename", move || {
        let old = PathBuf::from(&target.id);
        let Some(parent) = old.parent() else { return };
        let name = new_file_name(&old, &target.name, &typed, target.kind == ItemKind::Folder);
        let new = parent.join(&name);
        let guard = UndoGuard::new();
        let (renamed, left) = rename_recorded(&old, &new, || file_operation(FO_RENAME, &[&old], Some(&new), FOF_ALLOWUNDO));
        crate::undo::record(left);
        guard.recorded();
        match renamed {
            Ok(true) => {
                log("Rename done");
                done(new.to_string_lossy().into_owned());
            }
            Ok(false) => log("Rename stopped (Windows' own message)"),
            Err(e) => log(&format!("Rename: Windows' file machinery said 0x{e:X}")),
        }
        changed();
    });
}

/// Renames `old` to `new` with `op` and reads off the disk what it did, however it returned (an
/// outside review of 2026-11-18): renamed (the new name was free before and is there now, the old
/// one gone; for a change of only the case of letters, the name on the disk is the new one) is a
/// step; provably not renamed (the old name still there, the new one as it was) is nothing; any
/// other outcome empties the list.
fn rename_recorded(old: &Path, new: &Path, op: impl FnOnce() -> Result<bool, i32>) -> (Result<bool, i32>, Recorded) {
    let (o, n) = (old.to_string_lossy().into_owned(), new.to_string_lossy().into_owned());
    let case_only = o != n && o.to_lowercase() == n.to_lowercase();
    let new_free_before = free_now(&n);
    let r = op();
    let file_name = |p: &Path| p.file_name().map(|x| x.to_string_lossy().into_owned());
    let left = if o == n {
        Recorded::Nothing
    } else if case_only {
        match name_on_disk(old) {
            Some(on_disk) if Some(&on_disk) == file_name(new).as_ref() => Recorded::Step(Step::Rename(n, o)),
            Some(on_disk) if Some(&on_disk) == file_name(old).as_ref() => Recorded::Nothing,
            _ => Recorded::Unproven,
        }
    } else if new_free_before && taken(&n) && free_now(&o) {
        Recorded::Step(Step::Rename(n, o))
    } else if taken(&o) && (if new_free_before { free_now(&n) } else { taken(&n) }) {
        Recorded::Nothing
    } else {
        Recorded::Unproven
    };
    (r, left)
}

/// "Preferred DropEffect": how Explorer marks files on the clipboard as cut (2) or copied (1).
fn drop_effect_format() -> u32 {
    let name = wide("Preferred DropEffect");
    // SAFETY: a valid null-terminated name.
    unsafe { RegisterClipboardFormatW(name.as_ptr()) }
}

/// Opens the clipboard for `owner`, trying a few times (another program may hold it a moment).
fn open_clipboard(owner: HWND) -> bool {
    for _ in 0..10 {
        // SAFETY: a plain call; closed by the caller.
        if unsafe { OpenClipboard(owner) } != 0 {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    log("the clipboard could not be opened (another program held it)");
    false
}

/// A movable global block holding `bytes` (for the clipboard), or None.
fn global(bytes: &[u8]) -> Option<HGLOBAL> {
    // SAFETY: a block of the right size, filled while locked.
    unsafe {
        let h = GlobalAlloc(GMEM_MOVEABLE, bytes.len());
        if h.is_null() {
            return None;
        }
        let p = GlobalLock(h) as *mut u8;
        if p.is_null() {
            GlobalFree(h);
            return None;
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
        GlobalUnlock(h);
        Some(h)
    }
}

/// Cut or Copy: the files onto the clipboard, as Explorer puts them there. Returns the
/// clipboard's sequence number afterwards (the desktop shows cut icons faded until it changes).
pub fn put_on_clipboard(owner: HWND, targets: &[Target], cut: bool) -> Option<u32> {
    let paths: Vec<PathBuf> = targets.iter().filter(|t| t.is_file()).map(|t| PathBuf::from(&t.id)).collect();
    if paths.is_empty() {
        return None;
    }
    let header = size_of::<DROPFILES>();
    let list = path_list(paths.iter().map(PathBuf::as_path));
    let mut bytes = vec![0u8; header];
    let files = DROPFILES { pFiles: header as u32, fWide: 1, ..Default::default() };
    // SAFETY: DROPFILES is plain data of `header` bytes.
    unsafe { std::ptr::copy_nonoverlapping((&files as *const DROPFILES).cast::<u8>(), bytes.as_mut_ptr(), header) };
    bytes.extend(list.iter().flat_map(|c| c.to_le_bytes()));
    let effect: u32 = if cut { 2 } else { 1 };
    if !open_clipboard(owner) {
        return None;
    }
    // SAFETY: the clipboard is ours until CloseClipboard; it owns the blocks once given.
    let ok = unsafe {
        EmptyClipboard();
        let a = global(&bytes).map(|h| !SetClipboardData(u32::from(CF_HDROP), h as HANDLE).is_null()).unwrap_or(false);
        let b = global(&effect.to_le_bytes())
            .map(|h| !SetClipboardData(drop_effect_format(), h as HANDLE).is_null())
            .unwrap_or(false);
        CloseClipboard();
        a && b
    };
    if !ok {
        log("Cut/Copy: the clipboard did not take the files");
        return None;
    }
    log(&format!("{} {} item(s) onto the clipboard", if cut { "Cut" } else { "Copy" }, paths.len()));
    // SAFETY: a plain query.
    Some(unsafe { GetClipboardSequenceNumber() })
}

thread_local! {
    /// A live test's command is being carried out on this thread (`for_a_test`).
    static ONLY_TEST_ITEMS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Carries out `f` as a live test's command: a Paste or Paste Shortcut then takes only a test's own
/// items from the clipboard, and Undo takes back only a step of test items; anything else is
/// refused and logged (the third safety review of 2026-11-18, C4: a test's Paste took whatever was
/// on the clipboard, the person's files too).
pub fn for_a_test<R>(f: impl FnOnce() -> R) -> R {
    struct Reset(bool);
    impl Drop for Reset {
        fn drop(&mut self) {
            ONLY_TEST_ITEMS.with(|t| t.set(self.0));
        }
    }
    let _reset = Reset(ONLY_TEST_ITEMS.with(|t| t.replace(true)));
    f()
}

/// Whether a live test's command is being carried out on this thread (`for_a_test`).
pub fn only_test_items() -> bool {
    ONLY_TEST_ITEMS.with(std::cell::Cell::get)
}

/// A live test's own item: named "Bevel95 test ...", in a folder named so.
pub fn is_test_item(path: &Path) -> bool {
    let test = |p: &Path| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("Bevel95 test"));
    test(path) && path.parent().is_some_and(test)
}

/// Whether the clipboard's `paths` may be taken now: always, but during a live test's command only
/// a test's own items.
fn clipboard_allowed(paths: &[PathBuf], what: &str) -> bool {
    if only_test_items() && !paths.iter().all(|p| is_test_item(p)) {
        log(&format!("a test's {what} was refused: the clipboard holds files that are not a test's own"));
        return false;
    }
    true
}

/// Whether the clipboard holds files (Paste and Paste Shortcut are grayed without).
pub fn clipboard_has_files() -> bool {
    // SAFETY: a plain query.
    unsafe { IsClipboardFormatAvailable(u32::from(CF_HDROP)) != 0 }
}

/// The files on the clipboard and whether they were cut (to be moved).
fn clipboard_files(owner: HWND) -> Option<(Vec<PathBuf>, bool)> {
    if !clipboard_has_files() || !open_clipboard(owner) {
        return None;
    }
    // SAFETY: the clipboard is open; its blocks are only read, while locked.
    let out = unsafe {
        let drop = GetClipboardData(u32::from(CF_HDROP)) as HDROP;
        let mut paths = Vec::new();
        if !drop.is_null() {
            let n = DragQueryFileW(drop, u32::MAX, null_mut(), 0);
            for i in 0..n {
                let len = DragQueryFileW(drop, i, null_mut(), 0) as usize;
                let mut buf = vec![0u16; len + 1];
                DragQueryFileW(drop, i, buf.as_mut_ptr(), buf.len() as u32);
                paths.push(PathBuf::from(String::from_utf16_lossy(&buf[..len])));
            }
        }
        let mut cut = false;
        let effect = GetClipboardData(drop_effect_format());
        if !effect.is_null() {
            let p = GlobalLock(effect as HGLOBAL) as *const u32;
            if !p.is_null() {
                cut = p.read_unaligned() & 2 != 0;
                GlobalUnlock(effect as HGLOBAL);
            }
        }
        CloseClipboard();
        (paths, cut)
    };
    (!out.0.is_empty()).then_some(out)
}

fn same_dir(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a.as_os_str().eq_ignore_ascii_case(b.as_os_str()),
        _ => false,
    }
}

/// Paste: the clipboard's files into the folder `dest` (the desktop's: the owner's Desktop
/// folder), copied, or moved after Cut (files cut from that folder itself stay where they are).
/// A file copied from that same folder gets Windows' own "- Copy" name; any other name that is
/// taken gets Windows' own question.
pub fn paste(owner: HWND, dest: PathBuf) {
    let Some((paths, cut)) = clipboard_files(owner) else { return };
    if !clipboard_allowed(&paths, "Paste") {
        return;
    }
    // SAFETY: a plain query.
    let seq = unsafe { GetClipboardSequenceNumber() };
    on_thread("desktop paste", move || {
        let from: Vec<&Path> = paths
            .iter()
            .map(PathBuf::as_path)
            .filter(|p| !(cut && p.parent().is_some_and(|d| same_dir(d, &dest))))
            .collect();
        if from.is_empty() {
            return;
        }
        // A step Undo can take back; nothing when it provably did nothing; else an empty list.
        let guard = UndoGuard::new();
        let (result, left) = if cut { move_recorded(&from, &dest) } else { copy_into(&from, &dest) };
        crate::undo::record(left);
        guard.recorded();
        match result {
            Ok(done) => {
                log(&format!(
                    "Paste {} {} item(s){}",
                    if cut { "moved" } else { "copied" },
                    from.len(),
                    if done { "" } else { " (stopped)" }
                ));
                // Moved: what the clipboard named is gone from there, so it is emptied, as
                // Explorer does after a cut and paste (unless something else is on it by now).
                // SAFETY: plain calls; the clipboard is closed again.
                if cut && done && unsafe { GetClipboardSequenceNumber() } == seq && open_clipboard(null_mut()) {
                    unsafe {
                        EmptyClipboard();
                        CloseClipboard();
                    }
                }
            }
            Err(e) => log(&format!("Paste: Windows' file machinery said 0x{e:X}")),
        }
        changed();
    });
}

const IID_SHELL_LINK: GUID = GUID::from_u128(0x000214F9_0000_0000_C000_000000000046);
const IID_PERSIST_STREAM: GUID = GUID::from_u128(0x00000109_0000_0000_C000_000000000046);
const IID_DATA_OBJECT: GUID = GUID::from_u128(0x0000010E_0000_0000_C000_000000000046);
/// IShellLinkW's table: SetIDList and SetPath; IPersistStream's Save (IUnknown's three, then
/// GetClassID, IsDirty, Load, Save).
const SET_ID_LIST: usize = 5;
const SET_PATH: usize = 20;
const SAVE: usize = 6;
type QueryInterface = unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT;
type SetText = unsafe extern "system" fn(*mut c_void, PCWSTR) -> HRESULT;
type SetIdList = unsafe extern "system" fn(*mut c_void, *const ITEMIDLIST) -> HRESULT;
type SaveStream = unsafe extern "system" fn(*mut c_void, *mut c_void, windows_sys::core::BOOL) -> HRESULT;

/// What SHCreateStreamOnFileEx says for a name already taken (HRESULT_FROM_WIN32 of
/// ERROR_FILE_EXISTS or ERROR_ALREADY_EXISTS).
fn hr_taken(hr: HRESULT) -> bool {
    let win32 = |e: u32| (0x8007_0000u32 | e) as HRESULT;
    hr == win32(ERROR_FILE_EXISTS) || hr == win32(ERROR_ALREADY_EXISTS)
}

/// What a new shortcut points at: a file's path, or a shell object's item list.
enum LinkTo {
    Path(String),
    Shell(String),
}

/// Writes the shortcut `link` with Windows' ShellLink object into a new file: the file is
/// created new (STGM_FAILIFTHERE: anything already under that name is left alone, `NewFile::Taken`)
/// and the shortcut written through that same stream. A file only partly written is removed again
/// (it is the one just made).
fn save_link_new(link: &Path, to: &LinkTo) -> Result<(), NewFile> {
    let mut shell_link: *mut c_void = null_mut();
    // SAFETY: CoCreateInstance with valid GUIDs; every method through the interfaces' documented
    // tables; every reference and item list released.
    unsafe {
        let hr = CoCreateInstance(&ShellLink, null_mut(), CLSCTX_INPROC_SERVER, &IID_SHELL_LINK, &mut shell_link);
        if hr < 0 || shell_link.is_null() {
            return Err(NewFile::Failed(format!("CoCreateInstance(ShellLink) 0x{hr:08X}")));
        }
        let set = match to {
            LinkTo::Path(p) => {
                let p = wide(p);
                method::<SetText>(shell_link, SET_PATH)(shell_link, p.as_ptr())
            }
            LinkTo::Shell(name) => {
                let n = wide(name);
                let mut pidl: *mut ITEMIDLIST = null_mut();
                let hr = SHParseDisplayName(n.as_ptr(), null_mut(), &mut pidl, 0, null_mut());
                if hr < 0 || pidl.is_null() {
                    hr
                } else {
                    let hr = method::<SetIdList>(shell_link, SET_ID_LIST)(shell_link, pidl);
                    ILFree(pidl);
                    hr
                }
            }
        };
        let result = if set < 0 { Err(NewFile::Failed(format!("the shortcut's target 0x{set:08X}"))) } else { persist_new(shell_link, link) };
        release(shell_link);
        result
    }
}

/// Saves the ShellLink object `shell_link` as the new file `link`, which must not exist yet: the
/// file is created new (SHCreateStreamOnFileEx with STGM_FAILIFTHERE, CREATE_NEW underneath)
/// and the shortcut written through that stream, so a file already there is never written over
/// (`NewFile::Taken`). A file only partly written is removed again (it is the one just made).
///
/// # Safety
/// `shell_link` is a live IShellLinkW of this thread's COM.
unsafe fn persist_new(shell_link: *mut c_void, link: &Path) -> Result<(), NewFile> {
    // SAFETY: through the interfaces' documented tables; every reference is released.
    unsafe {
        let mut persist: *mut c_void = null_mut();
        let hr = method::<QueryInterface>(shell_link, 0)(shell_link, &IID_PERSIST_STREAM, &mut persist);
        if hr < 0 || persist.is_null() {
            return Err(NewFile::Failed(format!("IPersistStream 0x{hr:08X}")));
        }
        let path = wide(&link.to_string_lossy());
        let mut stream: *mut c_void = null_mut();
        let mode = STGM_WRITE | STGM_SHARE_EXCLUSIVE | STGM_FAILIFTHERE;
        let hr = SHCreateStreamOnFileEx(path.as_ptr(), mode, FILE_ATTRIBUTE_NORMAL, 1, null_mut(), &mut stream);
        let result = if hr_taken(hr) {
            Err(NewFile::Taken)
        } else if hr < 0 || stream.is_null() {
            Err(NewFile::Failed(format!("the new file 0x{hr:08X}")))
        } else {
            let hr = method::<SaveStream>(persist, SAVE)(persist, stream, 1);
            release(stream);
            if hr < 0 {
                let _ = std::fs::remove_file(link);
                Err(NewFile::Failed(format!("Save 0x{hr:08X}")))
            } else {
                Ok(())
            }
        };
        release(persist);
        result
    }
}

/// `persist_new` for Setup's own desktop shortcuts: Err also when something is already under
/// that name (it is left alone).
///
/// # Safety
/// `shell_link` is a live IShellLinkW of this thread's COM.
pub unsafe fn save_shell_link_new(shell_link: *mut c_void, link: &Path) -> Result<(), String> {
    // SAFETY: as the caller promised.
    match unsafe { persist_new(shell_link, link) } {
        Ok(()) => Ok(()),
        Err(NewFile::Taken) => Err(String::from("a file is already under that name; it was left alone")),
        Err(NewFile::Failed(e)) => Err(e),
    }
}

/// "Shortcut to <name>.lnk" in `dir`, under a name not taken yet ("... (2)" and so on).
#[cfg(test)]
fn shortcut_path(dir: &Path, name: &str) -> PathBuf {
    shortcut_path_avoiding(dir, name, &[])
}

/// `shortcut_path`, also passing over the names in `tried` (found taken when they were claimed).
fn shortcut_path_avoiding(dir: &Path, name: &str, tried: &[PathBuf]) -> PathBuf {
    let clean: String = name.chars().map(|c| if "\\/:*?\"<>|".contains(c) { '_' } else { c }).collect();
    let taken = |n: &str| {
        let p = dir.join(n);
        p.symlink_metadata().is_ok() || tried.iter().any(|t| t.as_os_str().eq_ignore_ascii_case(p.as_os_str()))
    };
    dir.join(bevel_core::desktop::new_name(&format!("Shortcut to {clean}"), ".lnk", taken))
}

/// One new shortcut "Shortcut to <name>.lnk" in `dir` (a shortcut's shortcut, `copy`: a copy of
/// it), and its path. `chosen` runs between choosing the name and writing (the tests put a file
/// there, as another program could).
///
/// The name is claimed as the file is made: it is created new, failing if anything is there
/// (CREATE_NEW: `create_new`, or a stream with STGM_FAILIFTHERE), and written through that same
/// handle; a name taken in between is left alone and the next name is tried.
fn new_shortcut(dir: &Path, name: &str, to: &LinkTo, copy: bool, chosen: &dyn Fn(&Path)) -> Result<PathBuf, String> {
    // A shortcut's shortcut: its bytes, read before any name is claimed.
    let bytes = match (to, copy) {
        (LinkTo::Path(p), true) => Some(std::fs::read(p).map_err(|e| format!("the shortcut to copy could not be read: {e}"))?),
        _ => None,
    };
    let mut tried: Vec<PathBuf> = Vec::new();
    for _ in 0..100 {
        let link = shortcut_path_avoiding(dir, name, &tried);
        chosen(&link);
        let made = match &bytes {
            Some(bytes) => write_new(&link, bytes),
            None => save_link_new(&link, to),
        };
        match made {
            Ok(()) => return Ok(link),
            Err(NewFile::Taken) => tried.push(link),
            Err(NewFile::Failed(e)) => return Err(e),
        }
    }
    Err(String::from("no free name (100 names were taken while trying)"))
}

/// Why a new file was not made.
enum NewFile {
    /// Something is already under that name (it was left alone).
    Taken,
    Failed(String),
}

/// `bytes` as the new file `path`, which must not exist yet (a file of that name is never
/// touched). A file only partly written is removed again (it is the one just made).
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), NewFile> {
    use std::io::Write;
    let mut file = match std::fs::OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return Err(NewFile::Taken),
        Err(e) => return Err(NewFile::Failed(e.to_string())),
    };
    if let Err(e) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err(NewFile::Failed(e.to_string()));
    }
    Ok(())
}

/// Create Shortcut and Paste Shortcut: for each target, "Shortcut to <name>" in the folder `dir`
/// (the desktop's: the owner's Desktop folder), as Windows 95 to 2000 named it. A shortcut's
/// shortcut is a copy of it, as Windows made it. `made` gets the first new shortcut's path (the
/// desktop or the folder window selects it once it shows).
fn make_shortcuts(dir: PathBuf, items: Vec<(LinkTo, String, bool)>, made: impl FnOnce(String) + Send + 'static) {
    on_thread("desktop shortcut", move || {
        let mut first = None;
        for (to, name, copy) in items {
            match new_shortcut(&dir, &name, &to, copy, &|_| {}) {
                Ok(link) => {
                    first.get_or_insert(link.to_string_lossy().into_owned());
                }
                Err(e) => log(&format!("a shortcut could not be made ({e})")),
            }
        }
        if let Some(first) = first {
            log("shortcut made");
            made(first);
        }
        changed();
    });
}

/// Create Shortcut on the selected icons: the shortcuts go into `dir`.
pub fn create_shortcuts(targets: &[Target], dir: PathBuf, made: impl FnOnce(String) + Send + 'static) {
    let items = targets
        .iter()
        .map(|t| {
            let shortcut = t.is_file() && Path::new(&t.id).extension().is_some_and(|e| e.eq_ignore_ascii_case("lnk"));
            let to = match t.kind {
                ItemKind::System(System::MyDocuments) => LinkTo::Path(shell_name(t)),
                ItemKind::System(_) | ItemKind::Place => LinkTo::Shell(shell_name(t)),
                _ => LinkTo::Path(t.id.clone()),
            };
            (to, t.name.clone(), shortcut)
        })
        .collect();
    make_shortcuts(dir, items, made);
}

/// Paste Shortcut: a shortcut in `dir` to each file on the clipboard.
pub fn paste_shortcuts(owner: HWND, dir: PathBuf, made: impl FnOnce(String) + Send + 'static) {
    let Some((paths, _)) = clipboard_files(owner) else { return };
    if !clipboard_allowed(&paths, "Paste Shortcut") {
        return;
    }
    let items = paths
        .iter()
        .map(|p| {
            let path = p.to_string_lossy().into_owned();
            let name = crate::shell::display_name(&path)
                .unwrap_or_else(|| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
            let shortcut = p.extension().is_some_and(|e| e.eq_ignore_ascii_case("lnk"));
            (LinkTo::Path(path), name, shortcut)
        })
        .collect();
    make_shortcuts(dir, items, made);
}

/// Bevel95's folder windows' program (`programs/folders.exe` beside the running program, or the
/// running program itself).
fn folders_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    [dir.join("programs").join("folders.exe"), dir.join("folders.exe")].into_iter().find(|p| p.is_file())
}

/// What Bevel95's Explorer window opens for `target` (`None`: a place it does not show, which
/// Windows' Explorer explores).
fn explore_arg(target: &Target) -> Option<String> {
    match target.kind {
        ItemKind::System(System::MyComputer) => Some("::my-computer".into()),
        ItemKind::System(System::MyDocuments) => Some("::my-documents".into()),
        ItemKind::Folder | ItemKind::Drive => Some(target.id.clone()),
        _ => None,
    }
}

/// Explore: Bevel95's Explorer window (the tree of all folders, `folders.exe --explore`) on a
/// folder, a drive, My Computer or My Documents; Windows' Explorer for anything else, or when the
/// folder windows' program is missing.
pub fn explore(targets: &[Target]) {
    let exe = folders_exe();
    let mut names = Vec::new();
    for t in targets {
        match (&exe, explore_arg(t)) {
            // A portable copy's folder windows are told where their data is (`log::flag_for`).
            (Some(exe), Some(arg)) => match std::process::Command::new(exe).arg("--explore").arg(&arg).args(crate::log::flag_for(exe)).spawn() {
                Ok(_) => {}
                Err(e) => {
                    log(&format!("Explore: could not start the folder windows ({e}); Windows' Explorer explores"));
                    names.push(shell_name(t));
                }
            },
            _ => names.push(shell_name(t)),
        }
    }
    names.retain(|n| !n.is_empty());
    on_thread("desktop explore", move || {
        for name in names {
            // Windows' own Explorer by its whole path: never a file of that name in the working folder.
            let system = std::env::var_os("SystemRoot").map_or_else(|| PathBuf::from("C:\\Windows"), PathBuf::from);
            let (exe, args) = (wide(&system.join("explorer.exe").to_string_lossy()), wide(&format!("/e,\"{name}\"")));
            // SAFETY: valid null-terminated strings.
            let r = unsafe { ShellExecuteW(null_mut(), null(), exe.as_ptr(), args.as_ptr(), null(), SW_SHOWNORMAL) };
            if r as usize <= 32 {
                log(&format!("Explore: ShellExecute error {}", r as usize));
            }
        }
    });
}

/// Properties: Windows' own Properties window of each item; several files in one folder share
/// one window, as in Windows. The look's own icons: their shell object's Properties (My Computer
/// opens today's System page).
pub fn properties(targets: &[Target]) {
    let targets = targets.to_vec();
    on_thread("desktop properties", move || {
        let files: Vec<&Target> = targets.iter().filter(|t| t.is_file()).collect();
        let mut groups: Vec<(PathBuf, Vec<String>)> = Vec::new();
        for t in &files {
            let parent = Path::new(&t.id).parent().map(Path::to_path_buf).unwrap_or_default();
            match groups.iter_mut().find(|(p, _)| p == &parent) {
                Some((_, ids)) => ids.push(t.id.clone()),
                None => groups.push((parent, vec![t.id.clone()])),
            }
        }
        for (_, ids) in groups {
            if ids.len() == 1 || !multi_properties(&ids) {
                for id in ids {
                    let p = wide(&id);
                    // SAFETY: a valid path; Windows shows its window on a thread of its own.
                    if unsafe { SHObjectProperties(null_mut(), SHOP_FILEPATH as u32, p.as_ptr(), null()) } == 0 {
                        log("Properties: Windows showed no window");
                    }
                }
            }
        }
        for t in targets.iter().filter(|t| !t.is_file()) {
            shell_properties(&shell_name(t));
        }
        // Windows' Properties windows run on threads of the shell's, in this process; this one
        // may end.
    });
}

/// One Properties window for several files of one folder (SHMultiFileProperties). False if it
/// could not be shown.
fn multi_properties(ids: &[String]) -> bool {
    // SAFETY: item lists made and freed here; the data object released after the call (Windows
    // keeps its own reference while its window shows).
    unsafe {
        let mut pidls: Vec<*mut ITEMIDLIST> = Vec::new();
        for id in ids {
            let n = wide(id);
            let mut pidl: *mut ITEMIDLIST = null_mut();
            if SHParseDisplayName(n.as_ptr(), null_mut(), &mut pidl, 0, null_mut()) >= 0 && !pidl.is_null() {
                pidls.push(pidl);
            }
        }
        let mut ok = false;
        if let Some(&first) = pidls.first() {
            let folder = ILClone(first);
            if !folder.is_null() && ILRemoveLastID(folder) != 0 {
                let children: Vec<*const ITEMIDLIST> =
                    pidls.iter().map(|&p| ILFindLastID(p) as *const ITEMIDLIST).collect();
                let mut data: *mut c_void = null_mut();
                let hr = SHCreateDataObject(
                    folder,
                    children.len() as u32,
                    children.as_ptr(),
                    null_mut(),
                    &IID_DATA_OBJECT,
                    &mut data,
                );
                if hr >= 0 && !data.is_null() {
                    ok = SHMultiFileProperties(data, 0) >= 0;
                    release(data);
                }
            }
            if !folder.is_null() {
                ILFree(folder);
            }
        }
        for p in pidls {
            CoTaskMemFree(p as *const c_void);
        }
        ok
    }
}

/// SEE_MASK_NOASYNC: ShellExecuteEx returns once the verb has started (this thread may end).
const SEE_MASK_NOASYNC: u32 = 0x100;

/// The Properties of a shell object by its parsing name ("::{CLSID}").
fn shell_properties(name: &str) {
    let (verb, file) = (wide("properties"), wide(name));
    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_INVOKEIDLIST | SEE_MASK_NOASYNC,
        lpVerb: verb.as_ptr(),
        lpFile: file.as_ptr(),
        nShow: SW_SHOWNORMAL,
        ..Default::default()
    };
    // SAFETY: a correctly sized struct with valid strings for the call.
    if unsafe { ShellExecuteExW(&mut info) } == 0 {
        crate::log::log_win32(&format!("{}: Properties of the look's own icon", who()));
    }
}

/// How many items the Recycle Bin holds (of every drive).
pub fn recycle_bin_count() -> i64 {
    let mut info = SHQUERYRBINFO { cbSize: size_of::<SHQUERYRBINFO>() as u32, ..Default::default() };
    // SAFETY: a correctly sized out-struct; a null root means every drive.
    if unsafe { SHQueryRecycleBinW(null(), &mut info) } == 0 { info.i64NumItems } else { 0 }
}

/// The name of the one item in the Recycle Bin, for the question before emptying it: read from
/// the Recycle Bin's own record of it ($I file: its original path), on each drive's
/// $Recycle.Bin, in the folders this user may read. Only read, never changed.
pub fn recycle_bin_single_name() -> Option<String> {
    let mut names = Vec::new();
    for letter in b'A'..=b'Z' {
        let root = PathBuf::from(format!("{}:\\$Recycle.Bin", letter as char));
        let Ok(users) = std::fs::read_dir(&root) else { continue };
        for user in users.flatten() {
            let Ok(entries) = std::fs::read_dir(user.path()) else { continue };
            for e in entries.flatten() {
                let file = e.file_name().to_string_lossy().into_owned();
                if !file.starts_with("$I") {
                    continue;
                }
                if let Ok(bytes) = std::fs::read(e.path()) {
                    names.extend(original_name(&bytes));
                }
            }
        }
    }
    (names.len() == 1).then(|| names.remove(0))
}

/// The original file name in a Recycle Bin record ($I file): version 1 has the path at byte 24
/// (260 characters), version 2 a length at 24 and the path from 28.
pub fn original_name(bytes: &[u8]) -> Option<String> {
    let version = u64::from_le_bytes(bytes.get(0..8)?.try_into().ok()?);
    let utf16: Vec<u16> = match version {
        1 => {
            bytes.get(24..)?.as_chunks::<2>().0.iter().map(|&c| u16::from_le_bytes(c)).take_while(|&c| c != 0).collect()
        }
        2 => {
            let len = u32::from_le_bytes(bytes.get(24..28)?.try_into().ok()?) as usize;
            bytes
                .get(28..28 + len * 2)?
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&c| u16::from_le_bytes(c))
                .take_while(|&c| c != 0)
                .collect()
        }
        _ => return None,
    };
    let path = String::from_utf16_lossy(&utf16);
    Path::new(&path).file_name().map(|n| n.to_string_lossy().into_owned())
}

/// Empty Recycle Bin (after Yes in the look's question): Windows' own, without its second
/// question (its progress window and sound stay).
pub fn empty_recycle_bin() {
    on_thread("desktop empty bin", || {
        // SAFETY: a null root empties every drive's Recycle Bin.
        let hr = unsafe { SHEmptyRecycleBinW(null_mut(), null(), SHERB_NOCONFIRMATION) };
        log(&format!("Empty Recycle Bin: 0x{hr:08X}"));
        changed();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A live test's command takes from the clipboard only a test's own items (the third safety
    /// review of 2026-11-18, C4: a test's Paste took whatever was there); outside a test, anything.
    #[test]
    fn a_tests_paste_takes_only_test_items() {
        let test = [PathBuf::from("C:\\t\\Bevel95 test src\\Bevel95 test a.txt")];
        let mixed = [test[0].clone(), PathBuf::from("C:\\Users\\someone\\Documents\\Letter.docx")];
        let loose = [PathBuf::from("C:\\Users\\someone\\Desktop\\Bevel95 test a.txt")];
        assert!(clipboard_allowed(&mixed, "Paste"), "the person's own Paste");
        assert!(!only_test_items());
        for_a_test(|| {
            assert!(only_test_items());
            assert!(clipboard_allowed(&test, "Paste"));
            assert!(!clipboard_allowed(&mixed, "Paste"), "a person's file among them: refused");
            assert!(!clipboard_allowed(&loose, "Paste"), "a test's name outside a test folder: refused");
            for_a_test(|| assert!(only_test_items()));
            assert!(only_test_items(), "still a test after a nested one");
        });
        assert!(!only_test_items(), "over once the test's command is");
        let _ = std::panic::catch_unwind(|| for_a_test(|| panic!("a test panic inside a test's command")));
        assert!(!only_test_items(), "over after a panic too");
    }

    #[test]
    fn renamed_files_keep_the_extension_the_desktop_did_not_show() {
        let p = Path::new("C:\\Desktop\\Test text.rtf");
        assert_eq!(new_file_name(p, "Test text", "Letter", false), "Letter.rtf", "Explorer hid .rtf");
        assert_eq!(new_file_name(p, "Test text.rtf", "Letter.doc", false), "Letter.doc", "shown: what was typed");
        assert_eq!(new_file_name(Path::new("C:\\Desktop\\Notepad.lnk"), "Notepad", "Editor", false), "Editor.lnk");
        assert_eq!(
            new_file_name(Path::new("C:\\Desktop\\v1.2"), "v1.2", "v1.3", true),
            "v1.3",
            "a folder's dot is no extension"
        );
        assert_eq!(new_file_name(Path::new("C:\\Desktop\\Folder"), "Folder", "New", true), "New");
    }

    #[test]
    fn known_types_are_windows_own() {
        assert!(known_type(".txt"), "every Windows knows text files");
        assert!(known_type(".lnk"));
        assert!(!known_type(".bevel95-no-such-type"));
        // On the real names: a known extension changed asks, a hidden one kept does not.
        let p = Path::new("C:\\Desktop\\Bevel95 test.txt");
        let new = new_file_name(p, "Bevel95 test", "Bevel95 test.dat", false);
        assert!(!bevel_core::rename_warning::warns("Bevel95 test.txt", &new, false, known_type));
        let new = new_file_name(p, "Bevel95 test.txt", "Bevel95 test.dat", false);
        assert!(bevel_core::rename_warning::warns("Bevel95 test.txt", &new, false, known_type));
    }

    #[test]
    fn shortcuts_get_windows_old_names() {
        let dir = std::env::temp_dir();
        let p = shortcut_path(&dir, "My Computer");
        assert!(p.file_name().unwrap().to_string_lossy().starts_with("Shortcut to My Computer"));
        // Characters a file name cannot hold become "_".
        assert!(shortcut_path(&dir, "a:b").file_name().unwrap().to_string_lossy().starts_with("Shortcut to a_b"));
    }

    /// A file that takes a shortcut's chosen name before the shortcut is written (as another
    /// program could) is never written over: the shortcut takes the next name.
    #[test]
    fn a_shortcut_never_writes_over_a_file_that_took_its_name() {
        // SAFETY: COM for this test's thread (the ShellLink object).
        unsafe { CoInitializeEx(null(), COINIT_APARTMENTTHREADED as u32) };
        let dir = crate::undo::tests::scratch("shortcut race");
        let target = dir.join("Bevel95 test target.txt");
        std::fs::write(&target, b"target").unwrap();
        let old_link = dir.join("Bevel95 test old.lnk");
        let theirs: &[u8] = b"not Bevel95's";
        for (to, name, copy) in [
            (LinkTo::Path(target.to_string_lossy().into_owned()), "Bevel95 test target.txt", false),
            (LinkTo::Path(old_link.to_string_lossy().into_owned()), "Bevel95 test old", true),
        ] {
            if copy {
                // A shortcut to copy: the one the first round made.
                std::fs::copy(dir.join("Shortcut to Bevel95 test target.txt (2).lnk"), &old_link).unwrap();
            }
            let first = std::cell::Cell::new(true);
            let appear = |p: &Path| {
                if first.replace(false) {
                    std::fs::write(p, theirs).unwrap();
                }
            };
            let made = new_shortcut(&dir, name, &to, copy, &appear).expect("a shortcut");
            assert!(std::fs::read(dir.join(format!("Shortcut to {name}.lnk"))).unwrap() == theirs, "the other file is untouched");
            assert_eq!(made, dir.join(format!("Shortcut to {name} (2).lnk")), "the next name");
            let bytes = std::fs::read(&made).unwrap();
            assert!(bytes.starts_with(&[0x4C, 0, 0, 0]), "a shortcut was written there");
            if copy {
                assert_eq!(bytes, std::fs::read(&old_link).unwrap(), "a shortcut's shortcut is a copy of it");
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// On the disk, in a scratch folder of "Bevel95 test" items: a copy into its own folder gets
    /// the name chosen before (Windows' own "- Copy" rule) and is recorded with its proof; the
    /// folder's other items are never taken for copies; a merged folder is not recorded.
    #[test]
    fn copies_and_moves_record_only_what_they_made() {
        let dir = crate::undo::tests::scratch("copies");
        std::fs::write(dir.join("Bevel95 test a.txt"), b"hello").unwrap();
        std::fs::create_dir_all(dir.join("Bevel95 test F")).unwrap();
        std::fs::write(dir.join("Bevel95 test F\\one.txt"), b"12").unwrap();
        std::fs::write(dir.join("Bevel95 test download.txt"), b"not ours").unwrap();
        let a = dir.join("Bevel95 test a.txt");
        let f = dir.join("Bevel95 test F");
        let (r, step) = copy_into(&[&a, &f], &dir);
        assert_eq!(r, Ok(true));
        let Recorded::Step(Step::Copy(made)) = step else { panic!("a copy step") };
        let paths: Vec<String> = made.iter().map(|m| m.path.clone()).collect();
        let want = |n: &str| dir.join(n).to_string_lossy().into_owned();
        assert_eq!(paths, [want("Bevel95 test a - Copy.txt"), want("Bevel95 test F - Copy")]);
        assert!(made.iter().all(|m| crate::undo::proof_of(&m.path) == Some(m.proof)));
        // Again: the names Windows would give next.
        let (_, step) = copy_into(&[&a], &dir);
        let Recorded::Step(Step::Copy(made)) = step else { panic!("a copy step") };
        assert_eq!(made[0].path, want("Bevel95 test a - Copy (2).txt"));
        let mut names: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        assert_eq!(names.len(), 6, "{names:?}");
        // A folder moved onto a folder of the same name merges: nothing of it is recorded.
        let other = dir.join("Bevel95 test other");
        std::fs::create_dir_all(other.join("Bevel95 test F")).unwrap();
        std::fs::write(other.join("Bevel95 test F\\always there.txt"), b"x").unwrap();
        let plan = plan_for(&[&f], &other, false);
        assert!(!plan[0].free);
        let r = file_operation(FO_MOVE, &[&f], Some(&other), FOF_NOCONFIRMATION);
        assert_eq!(r, Ok(true));
        assert!(!f.exists() && other.join("Bevel95 test F\\one.txt").exists(), "merged");
        assert!(bevel_core::drag::moved(&plan, taken).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// An outside review of 2026-11-18: what an operation did is read off the disk however it
    /// returned. Each operation here fails (an error code) after doing part of its work, as
    /// Windows' file machinery can: the part done is a step; an operation that failed before
    /// doing anything leaves the list alone. Delete (and the Recycle Bin's drop) recorded nothing
    /// after an error before, so Undo reached past the items already in the Recycle Bin.
    #[test]
    fn an_operation_that_fails_part_way_records_what_it_did() {
        let dir = crate::undo::tests::scratch2("fails part way");
        let file = |n: &str| {
            let p = dir.join(n);
            std::fs::write(&p, n.as_bytes()).unwrap();
            p
        };
        let s = |p: &Path| p.to_string_lossy().into_owned();
        const FAILED: i32 = 0x78;
        // Delete (Delete, the Recycle Bin's drop): one of two gone, then the error.
        let (a, b) = (file("Bevel95 test del a.txt"), file("Bevel95 test del b.txt"));
        let (r, left) = delete_recorded(&[a.clone(), b.clone()], |_| {
            std::fs::remove_file(&a).unwrap();
            Err(FAILED)
        });
        assert_eq!(r, Err(FAILED));
        assert!(matches!(&left, Recorded::Step(Step::Delete(gone, ..)) if *gone == [s(&a)]), "{left:?}");
        let (_, left) = delete_recorded(std::slice::from_ref(&b), |_| Err(FAILED));
        assert_eq!(left, Recorded::Nothing, "failed before anything: the list stays");
        // Rename: renamed, then an error.
        let (old, new) = (file("Bevel95 test ren a.txt"), dir.join("Bevel95 test ren b.txt"));
        let (_, left) = rename_recorded(&old, &new, || {
            std::fs::rename(&old, &new).unwrap();
            Err(FAILED)
        });
        assert_eq!(left, Recorded::Step(Step::Rename(s(&new), s(&old))));
        let (_, left) = rename_recorded(&new, &old, || Err(FAILED));
        assert_eq!(left, Recorded::Nothing);
        // Only the case of letters, then an error.
        let lower = dir.join("bevel95 test ren b.txt");
        let (_, left) = rename_recorded(&new, &lower, || {
            std::fs::rename(&new, &lower).unwrap();
            Err(FAILED)
        });
        assert_eq!(left, Recorded::Step(Step::Rename(s(&lower), s(&new))));
        // Move (Paste after Cut, a drop's move): one of two went, then the error.
        let into = dir.join("Bevel95 test into");
        std::fs::create_dir_all(&into).unwrap();
        let (m1, m2) = (file("Bevel95 test mv a.txt"), file("Bevel95 test mv b.txt"));
        let (_, left) = move_recorded_with(&[&m1, &m2], &into, || {
            std::fs::rename(&m1, into.join("Bevel95 test mv a.txt")).unwrap();
            Err(FAILED)
        });
        assert_eq!(left, Recorded::Step(Step::Move(vec![(s(&into.join("Bevel95 test mv a.txt")), s(&m1))])));
        // Copy (Paste after Copy, a drop's copy): one of two made, then the error.
        let (c1, c2) = (file("Bevel95 test cp a.txt"), file("Bevel95 test cp b.txt"));
        let (_, left) = copy_into_with(&[&c1, &c2], &into, |_| {
            std::fs::copy(&c1, into.join("Bevel95 test cp a.txt")).unwrap();
            Err(FAILED)
        });
        let Recorded::Step(Step::Copy(made)) = left else { panic!("a copy step: {left:?}") };
        assert_eq!(made.iter().map(|m| m.path.clone()).collect::<Vec<_>>(), [s(&into.join("Bevel95 test cp a.txt"))]);
        let (_, left) = copy_into_with(&[&c2], &into, |_| Err(FAILED));
        assert_eq!(left, Recorded::Nothing);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A job on a thread of its own counts as running from the moment it is asked for until it
    /// is done, so the program waits for it (the safety review of 2026-11-18, A2: nothing kept
    /// the program for these threads).
    #[test]
    fn a_file_job_counts_until_it_is_done() {
        let (go, wait) = std::sync::mpsc::channel::<()>();
        let (done_tx, done) = std::sync::mpsc::channel::<()>();
        assert!(on_thread("test job", move || {
            wait.recv().unwrap();
            done_tx.send(()).unwrap();
        }));
        assert!(crate::jobs::running() >= 1, "the job counts while it waits");
        assert!(crate::jobs::busy());
        go.send(()).unwrap();
        done.recv().unwrap();
    }

    #[test]
    fn recycle_bin_records_give_the_original_name() {
        let mut v2 = 2u64.to_le_bytes().to_vec();
        v2.extend(5u64.to_le_bytes());
        v2.extend(0u64.to_le_bytes());
        let path: Vec<u16> = "C:\\Users\\x\\Desktop\\Test text.rtf\0".encode_utf16().collect();
        v2.extend((path.len() as u32).to_le_bytes());
        v2.extend(path.iter().flat_map(|c| c.to_le_bytes()));
        assert_eq!(original_name(&v2).as_deref(), Some("Test text.rtf"));
        let mut v1 = 1u64.to_le_bytes().to_vec();
        v1.extend([0u8; 16]);
        let mut name: Vec<u16> = "D:\\a.rtf".encode_utf16().collect();
        name.resize(260, 0);
        v1.extend(name.iter().flat_map(|c| c.to_le_bytes()));
        assert_eq!(original_name(&v1).as_deref(), Some("a.rtf"));
        assert_eq!(original_name(&[1, 2, 3]), None);
    }
}
