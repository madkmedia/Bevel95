//! What the shell knows about the owner's files, for the classic desktop's icons and the folder
//! windows: the known folders, Explorer's own name and type name of a file, its 32×32 picture
//! (from the shell's list of large icons, without the shortcut arrow, which each look draws in its
//! own way), and one icon's whole item. Nothing here writes anything.

use std::cell::Cell;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::ptr::null_mut;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use bevel_core::desktop_icons::{Item, Picture, ShellPicture};
use bevel_core::fb::Icon;
use bevel_core::file_kinds::{self, Choice};
use bevel_core::theme::Era;
use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::UI::Shell::{
    FOLDERID_Desktop, FOLDERID_Documents, FOLDERID_PublicDesktop, KF_FLAG_DEFAULT, SHFILEINFOW, SHGFI_DISPLAYNAME, SHGFI_SYSICONINDEX,
    SHGFI_TYPENAME, SHGFI_USEFILEATTRIBUTES, SHGetFileInfoW, SHGetImageList, SHGetKnownFolderPath, SHIL_LARGE, SHIL_SMALL,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{DestroyIcon, HICON};
use windows_sys::core::{GUID, HRESULT, PWSTR};

use crate::font::DpiUnaware;
use crate::log::log;
use crate::util::wide;

/// A known folder's path (FOLDERID_Desktop and the like).
pub fn known_folder(id: &GUID) -> Option<PathBuf> {
    let mut path: PWSTR = null_mut();
    // SAFETY: Windows allocates the string; it is read up to its terminating null and then
    // freed with CoTaskMemFree, as the API requires.
    unsafe {
        let ok = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT as u32, null_mut(), &mut path) == 0 && !path.is_null();
        let result = ok.then(|| {
            let len = (0..).take_while(|&i| *path.add(i) != 0).count();
            PathBuf::from(String::from_utf16_lossy(std::slice::from_raw_parts(path, len)))
        });
        CoTaskMemFree(path as *const c_void);
        result
    }
}

/// The person's own folder (C:\Users\<name>): where Windows' Run starts programs.
pub fn profile_folder() -> Option<PathBuf> {
    known_folder(&windows_sys::Win32::UI::Shell::FOLDERID_Profile)
}

/// Every Bevel95 program makes the person's own folder its working folder when it starts (the
/// second safety review of 2026-11-18, C2): started by the desktop shortcut it was Bevel95's own
/// data folder (or System32 by the Run entry), so a relative name typed into Run or an address
/// bar, and every program started from Bevel95 without a shortcut of its own, met Bevel95's
/// files. Nothing of Bevel95 uses the working folder (every path it opens is whole). A program
/// started from a shortcut still gets the shortcut's "Start in".
pub fn start_in_profile() {
    let Some(profile) = profile_folder() else {
        log("the person's own folder was not found; the working folder stays as it was");
        return;
    };
    if let Err(e) = std::env::set_current_dir(&profile) {
        log(&format!("could not make the person's own folder the working folder ({e})"));
    }
}

/// Windows' own program or panel `name` (control.exe, ncpa.cpl, SndVol.exe, ...) by its whole path
/// in Windows' system folder: never a file of that name in the working folder (the person's own
/// folder, where any program may write) or elsewhere on the search path (the third safety review
/// of 2026-11-18, C1). What the person types into Run or an address bar keeps Windows' own rules.
pub fn system_program(name: &str) -> String {
    let mut buf = [0u16; 260];
    // SAFETY: a buffer of the length passed; Windows writes at most that many characters.
    let n = unsafe { windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW(buf.as_mut_ptr(), buf.len() as u32) } as usize;
    let dir = if n > 0 && n < buf.len() {
        PathBuf::from(String::from_utf16_lossy(&buf[..n]))
    } else {
        std::env::var_os("SystemRoot").map_or_else(|| PathBuf::from("C:\\Windows"), PathBuf::from).join("System32")
    };
    dir.join(name).to_string_lossy().into_owned()
}

/// The owner's Documents folder.
pub fn documents_folder() -> Option<PathBuf> {
    known_folder(&FOLDERID_Documents)
}

/// The owner's Desktop folder (where the desktop's New makes things).
pub fn desktop_folder() -> Option<PathBuf> {
    known_folder(&FOLDERID_Desktop)
}

/// The desktop folder every user shares (its items show on the desktop too).
pub fn public_desktop_folder() -> Option<PathBuf> {
    known_folder(&FOLDERID_PublicDesktop)
}

/// Explorer's display name and type name of a file.
pub fn names(path: &str) -> Option<(String, String)> {
    names_asking(path, true)
}

/// `names`, the type name asked for only when `type_name` (else it comes back empty).
fn names_asking(path: &str, type_name: bool) -> Option<(String, String)> {
    let p = wide(path);
    let mut info = SHFILEINFOW::default();
    let flags = if type_name { SHGFI_DISPLAYNAME | SHGFI_TYPENAME } else { SHGFI_DISPLAYNAME };
    // SAFETY: a null-terminated path and a correctly sized out-struct; nothing to free.
    let ok = unsafe { SHGetFileInfoW(p.as_ptr(), 0, &mut info, size_of::<SHFILEINFOW>() as u32, flags) };
    if ok == 0 {
        return None;
    }
    let text = |buf: &[u16]| String::from_utf16_lossy(&buf[..buf.iter().position(|&c| c == 0).unwrap_or(buf.len())]);
    Some((text(&info.szDisplayName), text(&info.szTypeName)))
}

/// Explorer's name for a file (as the desktop and the folder windows show it).
pub fn display_name(path: &str) -> Option<String> {
    names(path).map(|(name, _)| name)
}

/// IID_IImageList, from CommonControls.h.
const IID_IMAGE_LIST: GUID = GUID::from_u128(0x46EB5926_582E_4017_9FDF_E8998DAA0950);
/// IImageList::GetIcon: the 11th method (after IUnknown's three and Add, ReplaceIcon,
/// SetOverlayImage, Replace, AddMasked, Draw, Remove).
const GET_ICON_SLOT: usize = 10;
type GetIcon = unsafe extern "system" fn(*mut c_void, i32, u32, *mut HICON) -> HRESULT;
/// ILD_TRANSPARENT.
const ILD_TRANSPARENT: u32 = 1;

thread_local! {
    /// The shell's list of large (32×32) icons (a COM IImageList, kept for the life of the
    /// thread), asked for on each thread that reads pictures.
    static LARGE_IMAGE_LIST: Cell<*mut c_void> = const { Cell::new(null_mut()) };
}

/// Where the shell keeps the picture of `path` in its list of icons.
fn icon_index(path: &str) -> Option<i32> {
    let p = wide(path);
    let mut info = SHFILEINFOW::default();
    // SAFETY: a null-terminated name and a correctly sized out-struct; with SYSICONINDEX nothing
    // of the result needs freeing.
    let found = unsafe { SHGetFileInfoW(p.as_ptr(), 0, &mut info, size_of::<SHFILEINFOW>() as u32, SHGFI_SYSICONINDEX) } != 0;
    found.then_some(info.iIcon)
}

/// The 32×32 icon the shell shows for a file, without the shortcut arrow (the looks draw their
/// own), read at 96 DPI from the shell's list of large icons. `None` when the shell has none. Not
/// cached: the callers keep what they read. The thread needs COM (`com::init`).
pub fn large_file_icon(path: &str) -> Option<Icon> {
    let _dpi = DpiUnaware::enter();
    let list = LARGE_IMAGE_LIST.with(|cell| {
        if cell.get().is_null() {
            let mut list: *mut c_void = null_mut();
            // SAFETY: a valid IID and out-pointer; the reference is kept for good.
            if unsafe { SHGetImageList(SHIL_LARGE as i32, &IID_IMAGE_LIST, &mut list) } == 0 {
                cell.set(list);
            } else {
                log("SHGetImageList (large) failed: the owner's items get no pictures");
            }
        }
        cell.get()
    });
    let index = icon_index(path)?;
    if list.is_null() {
        return None;
    }
    // SAFETY: the documented IImageList, called through its vtable; the icon is ours to destroy.
    unsafe {
        let get_icon: GetIcon = crate::com::method(list, GET_ICON_SLOT);
        let mut handle: HICON = null_mut();
        if get_icon(list, index, ILD_TRANSPARENT, &mut handle) != 0 || handle.is_null() {
            return None;
        }
        let pixels = crate::icon::icon_pixels_at(handle, 32);
        DestroyIcon(handle);
        pixels
    }
}

/// One file or folder as it was seen on the disk: its place, whether it is a folder, its size and
/// the time of its last change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seen {
    pub path: PathBuf,
    pub folder: bool,
    pub size: u64,
    pub modified: u64,
    /// The file's attributes (a read-only or system folder may have a name of its own:
    /// desktop.ini's).
    pub attributes: u32,
}

const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;

impl Seen {
    /// A hidden item shown (Windows shows hidden items): its picture is drawn faded.
    pub fn hidden(&self) -> bool {
        bevel_core::hidden::faded(self.attributes)
    }
}

/// What `list_folder` found besides the items shown: how many it left out (Windows 95's status
/// bar counted them: "plus N hidden") and the size of all files, hidden ones too (its status bar
/// added those up).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Hidden {
    pub count: usize,
    pub total_size: u64,
}

/// Windows' own two switches for hidden files, read through the shell's own call
/// (`SHGetSetSettings`, reading only: Bevel95 changes nothing there): File Explorer's "Hidden
/// items" and Folder Options' "Hide protected operating system files". If the call says nothing
/// both stay off, so nothing hidden shows.
pub fn explorer_switches() -> bevel_core::hidden::Switches {
    use windows_sys::Win32::UI::Shell::{SHELLSTATEA, SHGetSetSettings, SSF_SHOWALLOBJECTS, SSF_SHOWSUPERHIDDEN};
    let mut state = SHELLSTATEA::default();
    // SAFETY: a valid out-struct; 0 = read, never set.
    unsafe { SHGetSetSettings(&mut state, SSF_SHOWALLOBJECTS | SSF_SHOWSUPERHIDDEN, 0) };
    // SHELLSTATE's first bit field: fShowAllObjects is its bit 0, fShowSuperHidden its bit 15.
    let bits = state._bitfield1 as u32;
    bevel_core::hidden::Switches { show_hidden: bits & 1 != 0, show_protected: bits & (1 << 15) != 0 }
}

/// Everything in `dir` that Windows' two switches show (`bevel_core::hidden::shown`, the switches
/// read now), in no particular order, and what it leaves out. `None` if the folder could not be
/// read. Called off every window's thread, as the listings are.
pub fn list_folder(dir: &Path) -> Option<(Vec<Seen>, Hidden)> {
    list_folder_with(dir, explorer_switches())
}

/// `list_folder` with the switches given (the tests give their own, never the PC's).
pub fn list_folder_with(dir: &Path, switches: bevel_core::hidden::Switches) -> Option<(Vec<Seen>, Hidden)> {
    use std::os::windows::fs::MetadataExt;
    let entries = std::fs::read_dir(dir).ok()?;
    let mut out = Vec::new();
    let mut hidden = Hidden::default();
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_dir() {
            hidden.total_size += meta.len();
        }
        // Only what is left out is counted ("plus N hidden").
        if !bevel_core::hidden::shown(meta.file_attributes(), switches) {
            hidden.count += 1;
            continue;
        }
        out.push(Seen { path: entry.path(), folder: meta.is_dir(), size: meta.len(), modified: meta.last_write_time(), attributes: meta.file_attributes() });
    }
    Some((out, hidden))
}

/// The icon of a file or folder in `era`: Explorer's name for it (without ".lnk", and without the
/// extension where Explorer hides it), its type, and its picture: the look's own folder for a
/// folder; for a file what `file_kinds::decide` says (the look's picture and Type name for a kind
/// the look knew, a program's own picture or else the look's program picture, today's otherwise).
/// Off every window's thread: the shell may read the file (never run it).
pub fn read_item(s: &Seen, era: Era) -> Item {
    let path = s.path.to_string_lossy().into_owned();
    let file_name = s.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let decision = if s.folder { None } else { Some(file_kinds::decide(era, &file_name)) };
    // Today's Type name only where the look does not give its own.
    let ask_type = decision.and_then(|d| d.type_name).is_none();
    let (name, today_type) = names_asking(&path, ask_type).unwrap_or_else(|| (file_name.clone(), String::new()));
    let extension = ending_of(&s.path, s.folder);
    let shortcut = !s.folder && (extension == "lnk" || extension == "url");
    let picture = match decision.map(|d| d.picture) {
        None => Picture::Folder,
        Some(Choice::Look(kind)) => Picture::Kind(kind),
        Some(Choice::OwnOrProgram) => match own_icon_of_program(&path, &extension) {
            Some(own) => Picture::Own(Some(own)),
            None => Picture::Kind(file_kinds::Kind::Program),
        },
        Some(Choice::Today) => Picture::Own(large_file_icon(&path)),
    };
    let type_name = decision.and_then(|d| d.type_name).map_or(today_type, str::to_string);
    Item { id: path, name, picture, shortcut, folder: s.folder, type_name, size: s.size, modified: s.modified }
}

/// A file's ending by Windows' rule (`file_kinds::ending`: ".exe" alone is a program, "a.b c" has
/// none), lower case; empty for a folder or a name without one. Every place here takes it so.
fn ending_of(path: &Path, folder: bool) -> String {
    if folder {
        return String::new();
    }
    path.file_name().and_then(|n| file_kinds::ending(&n.to_string_lossy())).unwrap_or_default()
}

/// A program's own 32×32 picture, if it has one: the shell's picture of the file is not the one it
/// gives every program of its kind (`x.exe` without a file: today's picture of a program without an
/// icon). `None` also when the shell cannot read the file (locked, on a drive that is gone): the
/// look's program picture then.
fn own_icon_of_program(path: &str, extension: &str) -> Option<Icon> {
    let own = icon_index(path)?;
    (Some(own) != kind_of(extension, false).index).then(|| large_file_icon(path)).flatten()
}

// ---------------------------------------------------------------------------------------------
// A folder of thousands of files: names at once, pictures afterwards, each kind's picture once.

const FILE_ATTRIBUTE_READONLY: u32 = 0x1;
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;

/// What the shell says about a kind of file without opening any (SHGetFileInfo with
/// SHGFI_USEFILEATTRIBUTES on a made-up name): whether Explorer shows its extension, its type's
/// name, and where its usual picture is in the shell's list of icons.
#[derive(Clone, Debug)]
pub struct Kind {
    pub shows_extension: bool,
    pub type_name: String,
    pub index: Option<i32>,
}

/// The kind of files ending in `.extension` (lower case; empty: none), or of folders.
pub fn kind_of(extension: &str, folder: bool) -> Kind {
    let probe = if folder || extension.is_empty() { "x".to_string() } else { format!("x.{extension}") };
    let p = wide(&probe);
    let mut info = SHFILEINFOW::default();
    let attributes = if folder { FILE_ATTRIBUTE_DIRECTORY } else { FILE_ATTRIBUTE_NORMAL };
    let flags = SHGFI_USEFILEATTRIBUTES | SHGFI_DISPLAYNAME | SHGFI_TYPENAME | SHGFI_SYSICONINDEX;
    // SAFETY: a null-terminated name and a correctly sized out-struct; nothing to free.
    let ok = unsafe { SHGetFileInfoW(p.as_ptr(), attributes, &mut info, size_of::<SHFILEINFOW>() as u32, flags) } != 0;
    let text = |buf: &[u16]| String::from_utf16_lossy(&buf[..buf.iter().position(|&c| c == 0).unwrap_or(buf.len())]);
    if !ok {
        return Kind { shows_extension: true, type_name: String::new(), index: None };
    }
    Kind { shows_extension: text(&info.szDisplayName) == probe, type_name: text(&info.szTypeName), index: Some(info.iIcon) }
}

/// The shell's pictures, read once for each place in its list of icons and shared by every item
/// that shows it. At most `STORE_MOST` are kept; the least used go first (items that show one keep
/// their own reference until they go).
pub const STORE_MOST: usize = 2048;

struct Store {
    pictures: HashMap<i32, (Arc<ShellPicture>, u64)>,
    clock: u64,
}

static STORE: Mutex<Option<Store>> = Mutex::new(None);

/// How many pictures the store holds now (for the log and the leak checks).
pub fn store_size() -> usize {
    STORE.lock().ok().and_then(|s| s.as_ref().map(|s| s.pictures.len())).unwrap_or(0)
}

thread_local! {
    /// The shell's list of small (16×16) icons, as `LARGE_IMAGE_LIST`.
    static SMALL_IMAGE_LIST: Cell<*mut c_void> = const { Cell::new(null_mut()) };
}

/// The picture at `index` of one of the shell's lists of icons, at `size`.
fn list_icon(small: bool, index: i32) -> Option<Icon> {
    let _dpi = DpiUnaware::enter();
    let cell = if small { &SMALL_IMAGE_LIST } else { &LARGE_IMAGE_LIST };
    let list = cell.with(|cell| {
        if cell.get().is_null() {
            let mut list: *mut c_void = null_mut();
            let which = if small { SHIL_SMALL } else { SHIL_LARGE };
            // SAFETY: a valid IID and out-pointer; the reference is kept for good.
            if unsafe { SHGetImageList(which as i32, &IID_IMAGE_LIST, &mut list) } == 0 {
                cell.set(list);
            } else {
                log("SHGetImageList failed: items get no pictures");
            }
        }
        cell.get()
    });
    if list.is_null() {
        return None;
    }
    // SAFETY: the documented IImageList, called through its vtable; the icon is ours to destroy.
    unsafe {
        let get_icon: GetIcon = crate::com::method(list, GET_ICON_SLOT);
        let mut handle: HICON = null_mut();
        if get_icon(list, index, ILD_TRANSPARENT, &mut handle) != 0 || handle.is_null() {
            return None;
        }
        let pixels = crate::icon::icon_pixels_at(handle, if small { 16 } else { 32 });
        DestroyIcon(handle);
        pixels
    }
}

/// The pictures at `index` of the shell's lists (large and small), from the store or read now.
pub fn pictures_at(index: i32) -> Arc<ShellPicture> {
    if let Ok(mut store) = STORE.lock() {
        let s = store.get_or_insert_with(|| Store { pictures: HashMap::new(), clock: 0 });
        s.clock += 1;
        let now = s.clock;
        if let Some((p, used)) = s.pictures.get_mut(&index) {
            *used = now;
            return p.clone();
        }
    }
    let picture = Arc::new(ShellPicture { large: list_icon(false, index), small: list_icon(true, index) });
    if let Ok(mut store) = STORE.lock() {
        let s = store.get_or_insert_with(|| Store { pictures: HashMap::new(), clock: 0 });
        s.clock += 1;
        let now = s.clock;
        s.pictures.insert(index, (picture.clone(), now));
        if s.pictures.len() > STORE_MOST {
            // The least used quarter goes.
            let mut uses: Vec<(u64, i32)> = s.pictures.iter().map(|(&k, &(_, u))| (u, k)).collect();
            uses.sort_unstable();
            for (_, k) in uses.into_iter().take(STORE_MOST / 4) {
                s.pictures.remove(&k);
            }
        }
    }
    picture
}

/// An item as soon as its folder is listed, without opening the file: its name as Explorer shows
/// it (its extension hidden where Explorer hides that kind's), its kind's type name and picture
/// (`kinds` keeps each kind once). Folders get the look's own picture; a read-only or system
/// folder is asked for its own name (desktop.ini can give one). A file gets what
/// `file_kinds::decide` says for `era`: a kind the look knew its picture and Type name at once (its
/// own picture is never read: `reads_own_picture`), a program the look's program picture until its
/// own is read, every other file today's kind's picture and name.
pub fn quick_item(s: &Seen, kinds: &mut HashMap<(String, bool), Kind>, era: Era) -> Item {
    let path = s.path.to_string_lossy().into_owned();
    let file_name = s.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let extension = ending_of(&s.path, s.folder);
    let kind = kinds.entry((extension.clone(), s.folder)).or_insert_with(|| kind_of(&extension, s.folder)).clone();
    // The name without its extension is the path's own stem, never a cut by the lowercase
    // extension's length: lowercasing can change a letter's length in bytes (Kelvin sign, İ),
    // and a cut inside a letter would panic.
    let stem = s.path.file_stem().map(|n| n.to_string_lossy().into_owned()).filter(|stem| !stem.is_empty() && stem.len() < file_name.len());
    let mut name = if let Some(stem) = stem.filter(|_| !s.folder && !kind.shows_extension && !extension.is_empty()) {
        stem
    } else {
        file_name.clone()
    };
    if s.folder && s.attributes & (FILE_ATTRIBUTE_READONLY | FILE_ATTRIBUTE_SYSTEM) != 0
        && let Some(own) = display_name(&path)
    {
        name = own;
    }
    let shortcut = !s.folder && (extension == "lnk" || extension == "url");
    let decision = if s.folder { None } else { Some(file_kinds::decide(era, &file_name)) };
    let picture = match decision.map(|d| d.picture) {
        None => Picture::Folder,
        Some(Choice::Look(k)) => Picture::Kind(k),
        Some(Choice::OwnOrProgram) => Picture::Kind(file_kinds::Kind::Program),
        Some(Choice::Today) => kind.index.map_or(Picture::Own(None), |i| Picture::Shell(pictures_at(i))),
    };
    let type_name = decision.and_then(|d| d.type_name).map_or(kind.type_name, str::to_string);
    Item { id: path, name, picture, shortcut, folder: s.folder, type_name, size: s.size, modified: local_time(s.modified) }
}

/// Whether a file's own picture is to be read after its name (`own_picture`, off the window's
/// thread): not for a kind whose picture is the look's own (`file_kinds::Choice::Look`).
pub fn reads_own_picture(path: &Path, era: Era) -> bool {
    let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    !matches!(file_kinds::decide(era, &file_name).picture, Choice::Look(_))
}

/// A file's own picture (an application's, a shortcut's: read from the file), when it is not its
/// kind's (`None`: the kind's picture is right, or the shell has none; for a program, the look's
/// program picture stays).
pub fn own_picture(path: &str, kind_index: Option<i32>) -> Option<Arc<ShellPicture>> {
    let index = icon_index(path)?;
    (Some(index) != kind_index).then(|| pictures_at(index))
}

/// The place in the shell's list of icons a kind of file has (for `own_picture`).
pub fn kind_index(path: &Path, kinds: &mut HashMap<(String, bool), Kind>) -> Option<i32> {
    let extension = ending_of(path, false);
    kinds.entry((extension.clone(), false)).or_insert_with(|| kind_of(&extension, false)).index
}

/// A file time in local time (as Explorer's Details wrote it).
pub fn local_time(utc: u64) -> u64 {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::Storage::FileSystem::FileTimeToLocalFileTime;
    let ft = FILETIME { dwLowDateTime: utc as u32, dwHighDateTime: (utc >> 32) as u32 };
    let mut local = FILETIME::default();
    // SAFETY: two valid FILETIME structs.
    if unsafe { FileTimeToLocalFileTime(&ft, &mut local) } == 0 {
        return utc;
    }
    (u64::from(local.dwHighDateTime) << 32) | u64::from(local.dwLowDateTime)
}
#[cfg(test)]
mod kind_tests {
    use super::*;
    use bevel_core::file_kinds::Kind as LookKind;

    /// A scratch folder of the test's own (never anyone's), made empty.
    fn scratch(name: &str) -> PathBuf {
        let dir = crate::log::installed_dir().join(name);
        assert!(!dir.to_string_lossy().contains("AppData\\Local\\Bevel95\\"), "a scratch folder");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn seen(path: &Path) -> Seen {
        use std::os::windows::fs::MetadataExt;
        let meta = std::fs::metadata(path).unwrap();
        Seen { path: path.to_path_buf(), folder: meta.is_dir(), size: meta.len(), modified: meta.last_write_time(), attributes: meta.file_attributes() }
    }

    /// Files made here: a text file, a program without an icon of its own (this test's own
    /// program, copied: a Rust test program has none), one with icons (Windows' command prompt,
    /// copied), a PDF (no kind the old Windows knew), in each look.
    #[test]
    fn a_file_shows_what_the_look_decides() {
        crate::com::init();
        let dir = scratch("file-kinds");
        std::fs::write(dir.join("Notes.TXT"), b"x").unwrap();
        std::fs::write(dir.join("paper.pdf"), b"%PDF-1.4\n").unwrap();
        std::fs::copy(std::env::current_exe().unwrap(), dir.join("noicon.exe")).unwrap();
        std::fs::copy(system_program("cmd.exe"), dir.join("withicon.exe")).unwrap();
        for era in Era::ALL {
            let mut kinds = HashMap::new();
            let quick = |name: &str, kinds: &mut HashMap<(String, bool), Kind>| quick_item(&seen(&dir.join(name)), kinds, era);
            let text = quick("Notes.TXT", &mut kinds);
            assert!(matches!(text.picture, Picture::Kind(LookKind::Ending)), "{era:?}: {:?}", text.picture);
            assert_eq!(text.type_name, "Text Document");
            assert!(!reads_own_picture(&dir.join("Notes.TXT"), era), "a known kind's picture is never read");
            let pdf = quick("paper.pdf", &mut kinds);
            assert!(matches!(pdf.picture, Picture::Shell(_) | Picture::Own(None)), "today's picture: {:?}", pdf.picture);
            assert_eq!(pdf.type_name, kind_of("pdf", false).type_name, "today's name");
            assert!(reads_own_picture(&dir.join("paper.pdf"), era));
            for (name, own) in [("noicon.exe", false), ("withicon.exe", true)] {
                let path = dir.join(name);
                let item = quick(name, &mut kinds);
                assert!(matches!(item.picture, Picture::Kind(LookKind::Program)), "the program picture first");
                assert_eq!(item.type_name, "Application");
                assert!(reads_own_picture(&path, era));
                let found = own_picture(&path.to_string_lossy(), kind_index(&path, &mut kinds));
                assert_eq!(found.is_some(), own, "{name}: its own picture read afterwards");
                // The desktop's way: the same in one go.
                let item = read_item(&seen(&path), era);
                match (own, &item.picture) {
                    (false, Picture::Kind(LookKind::Program)) | (true, Picture::Own(Some(_))) => {}
                    other => panic!("{name}: {other:?}"),
                }
                assert_eq!(item.type_name, "Application");
            }
            let text = read_item(&seen(&dir.join("Notes.TXT")), era);
            assert!(matches!(text.picture, Picture::Kind(LookKind::Ending)) && text.type_name == "Text Document");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Every place takes a file's ending by Windows' one rule (`file_kinds::ending`): a program
    /// named only ".exe" is a program everywhere (its own picture compared with the kind's), and
    /// "a.b c" has no ending (finding 3 of the review of 2026-11-23).
    #[test]
    fn every_place_takes_the_ending_by_one_rule() {
        crate::com::init();
        assert_eq!(ending_of(Path::new("C:\\x\\.exe"), false), "exe");
        assert_eq!(ending_of(Path::new("C:\\x\\Notes.TXT"), false), "txt");
        assert_eq!(ending_of(Path::new("C:\\x\\a.b c"), false), "");
        assert_eq!(ending_of(Path::new("C:\\x\\a.txt"), true), "", "a folder has none");
        let dir = scratch("file-kinds-dot-exe");
        std::fs::copy(std::env::current_exe().unwrap(), dir.join(".exe")).unwrap();
        let path = dir.join(".exe");
        let mut kinds = HashMap::new();
        let item = quick_item(&seen(&path), &mut kinds, Era::Win98);
        assert!(matches!(item.picture, Picture::Kind(LookKind::Program)), "{:?}", item.picture);
        assert_eq!(item.type_name, "Application");
        // Its own picture is compared with the programs' kind ("exe"), not with "no ending". (What
        // the shell answers is not checked here: while other tests ask it at the same time, its
        // answers about this file came back different or empty now and then.)
        assert!(kinds.contains_key(&("exe".to_string(), false)) && !kinds.contains_key(&(String::new(), false)));
        let _ = kind_index(&path, &mut kinds);
        assert_eq!(kinds.len(), 1, "kind_index asked about the same kind");
        assert_eq!(read_item(&seen(&path), Era::Win98).type_name, "Application");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A program that cannot be read (held open by the test with no sharing) or is gone costs
    /// nothing but its own picture: the look's program picture.
    #[test]
    fn a_file_that_cannot_be_read_gets_the_looks_picture() {
        use std::os::windows::fs::OpenOptionsExt;
        crate::com::init();
        let dir = scratch("file-kinds-locked");
        let path = dir.join("locked.exe");
        std::fs::copy(system_program("cmd.exe"), &path).unwrap();
        let s = seen(&path);
        let held = std::fs::OpenOptions::new().read(true).share_mode(0).open(&path).unwrap();
        let item = read_item(&s, Era::Win98);
        assert!(matches!(item.picture, Picture::Kind(LookKind::Program)), "{:?}", item.picture);
        let mut kinds = HashMap::new();
        assert!(own_picture(&path.to_string_lossy(), kind_index(&path, &mut kinds)).is_none());
        drop(held);
        let _ = std::fs::remove_dir_all(&dir);
        let gone = read_item(&s, Era::Win2000);
        assert!(matches!(gone.picture, Picture::Kind(LookKind::Program)), "{:?}", gone.picture);
    }

    /// What a folder of 5,000 text files costs on its reader's thread, before (each file's own
    /// picture asked of the shell after its name) and now (none asked). Prints the times; run it
    /// with `cargo test -p bevel-winkit --lib -- --ignored --nocapture five_thousand`.
    #[test]
    #[ignore = "makes 5,000 files and prints times"]
    fn five_thousand_text_files() {
        crate::com::init();
        let dir = scratch("file-kinds-5000");
        for n in 0..5000 {
            std::fs::write(dir.join(format!("note {n:04}.txt")), b"x").unwrap();
        }
        let (listed, _) = list_folder_with(&dir, bevel_core::hidden::Switches::NONE).unwrap();
        // Before: today's kind's picture for every item, then every file's own picture asked for.
        let started = std::time::Instant::now();
        let mut kinds = HashMap::new();
        for s in &listed {
            let ext = s.path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            let kind = kinds.entry((ext.clone(), false)).or_insert_with(|| kind_of(&ext, false)).clone();
            let _ = kind.index.map(pictures_at);
        }
        let named = started.elapsed();
        for s in &listed {
            let _ = own_picture(&s.path.to_string_lossy(), kind_index(&s.path, &mut kinds));
        }
        let before = started.elapsed();
        // Now: the names and the look's pictures at once, nothing read afterwards.
        let started = std::time::Instant::now();
        let mut kinds = HashMap::new();
        let mut read = 0;
        for s in &listed {
            let _ = quick_item(s, &mut kinds, Era::Win98);
            if reads_own_picture(&s.path, Era::Win98) {
                read += 1;
            }
        }
        let now = started.elapsed();
        println!("5,000 text files: before {} ms named, {} ms with every picture; now {} ms with every picture ({read} read afterwards)", named.as_millis(), before.as_millis(), now.as_millis());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod hidden_tests {
    use super::*;
    use bevel_core::hidden::Switches;

    fn set_attributes(path: &Path, attributes: u32) {
        let w = wide(&path.to_string_lossy());
        // SAFETY: a null-terminated path of the test's own scratch file.
        assert!(unsafe { windows_sys::Win32::Storage::FileSystem::SetFileAttributesW(w.as_ptr(), attributes) } != 0);
    }

    /// A scratch folder with a plain file, a hidden one, a hidden system one and a hidden folder
    /// (made here, never anyone's): what each pair of switches lists and counts as left out.
    #[test]
    fn a_listing_follows_the_two_switches() {
        let dir = crate::log::installed_dir().join("hidden-listing");
        assert!(!dir.to_string_lossy().contains("AppData\\Local\\Bevel95\\"), "a scratch folder");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Secret folder")).unwrap();
        for name in ["plain.dat", "hidden.dat", "desktop.ini"] {
            std::fs::write(dir.join(name), b"x").unwrap();
        }
        set_attributes(&dir.join("hidden.dat"), 0x2);
        set_attributes(&dir.join("desktop.ini"), 0x2 | 0x4);
        set_attributes(&dir.join("Secret folder"), 0x2 | 0x10);
        let names = |s: Switches| {
            let (seen, hidden) = list_folder_with(&dir, s).unwrap();
            let mut n: Vec<(String, bool)> = seen.iter().map(|s| (s.path.file_name().unwrap().to_string_lossy().into_owned(), s.hidden())).collect();
            n.sort();
            (n, hidden.count)
        };
        let item = |n: &str, faded: bool| (n.to_string(), faded);
        assert_eq!(names(Switches::NONE), (vec![item("plain.dat", false)], 3));
        assert_eq!(
            names(Switches { show_hidden: true, show_protected: false }),
            (vec![item("Secret folder", true), item("hidden.dat", true), item("plain.dat", false)], 1)
        );
        assert_eq!(
            names(Switches { show_hidden: true, show_protected: true }),
            (vec![item("Secret folder", true), item("desktop.ini", true), item("hidden.dat", true), item("plain.dat", false)], 0)
        );
        assert_eq!(names(Switches { show_hidden: false, show_protected: true }), (vec![item("plain.dat", false)], 3));
        for name in ["hidden.dat", "desktop.ini", "Secret folder"] {
            set_attributes(&dir.join(name), 0x80);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}