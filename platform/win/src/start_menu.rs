//! What the Start menu shows, read from Windows: the Programs folders (yours and the one
//! for all users, merged as Windows merges them), your Favorites (Windows 98), your recent
//! documents, the icons of these, and what the Settings and Find entries open.

use std::cell::Cell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::fs;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use bevel_core::fb::Icon;
use bevel_core::programs::{ACCESSORIES, APP_PREFIX, Folder, Item, OWN_PROGRAMS, Row, Shown, Ticked};
use bevel_core::start::{self, Target};
use bevel_core::theme::Era;
use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND, GetLastError};
use windows_sys::Win32::Globalization::{CP_ACP, MultiByteToWideChar};
use windows_sys::Win32::System::Environment::ExpandEnvironmentStringsW;
use windows_sys::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_MODE_BACKGROUND_BEGIN};
use windows_sys::Win32::System::WindowsProgramming::GetUserNameW;
use windows_sys::Win32::UI::Controls::ILD_TRANSPARENT;
use windows_sys::Win32::Storage::FileSystem::{FILE_ATTRIBUTE_NORMAL, GetFileAttributesW, INVALID_FILE_ATTRIBUTES};
use windows_sys::Win32::UI::Shell::Common::ITEMIDLIST;
use windows_sys::Win32::UI::Shell::{
    FOLDERID_CommonPrograms, FOLDERID_Favorites, FOLDERID_Programs, FOLDERID_Recent, FOLDERID_StartMenu,
    ILFree, SHFILEINFOW, SHGFI_DISPLAYNAME, SHGFI_PIDL, SHGFI_SMALLICON, SHGFI_SYSICONINDEX,
    SHGFI_USEFILEATTRIBUTES, SHGetFileInfoW, SHGetImageList, SHIL_SMALL, SHLoadIndirectString,
    SHParseDisplayName,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{DestroyIcon, HICON};
use windows_sys::core::{GUID, HRESULT};

use crate::font::DpiUnaware;
use crate::util::wide;
use crate::window_list::{icon_pixels, launch};

/// Hidden and system files (such as desktop.ini) never show in the Start menu.
const HIDDEN_OR_SYSTEM: u32 = 0x2 | 0x4;
/// How many recent documents Windows 95's Documents menu listed.
const RECENT_COUNT: usize = 15;

/// The Programs menu as the owner chose it to show (Bevel95 Settings, Programs page):
/// everything found on this PC, only Bevel95's own classic programs, or only the programs
/// ticked by hand.
pub fn programs(shown: Shown) -> Folder {
    match shown {
        Shown::Everything => all_programs(),
        Shown::Classic => {
            let mut root = Folder::default();
            if let Ok(exe) = std::env::current_exe() && let Some(dir) = exe.parent() {
                add_own_programs(&mut root, dir);
            }
            root.sort();
            root
        }
        Shown::Ticked => {
            let mut root = all_programs();
            // Without a list (nothing was ever ticked or unticked) every program counts as ticked.
            if let Some(ticked) = ticked_programs() {
                root.retain(&|_, place| ticked.contains(place));
            }
            root
        }
    }
}

/// The file that lists the programs ticked by hand, one place in the menu on each line.
fn ticked_file() -> PathBuf {
    crate::log::data_dir().join("programs.txt")
}

/// The programs ticked by hand, or `None` while there is no list of them yet.
pub fn ticked_programs() -> Option<Ticked> {
    // A list that cannot be read counts as none (every program ticked); it is kept as it is.
    let ticked = bevel_winkit::state_file::load(&ticked_file()).text().map(|text| Ticked::parse(&text))?;
    // A list written before Accessories got its old name back (the first day of the Programs
    // page) names that folder by Windows' label: its ticks go with the folder.
    Some(match accessories_label() {
        Some(label) if !label.eq_ignore_ascii_case(ACCESSORIES) => ticked.with_folder_renamed(&label, ACCESSORIES),
        _ => ticked,
    })
}

// The Accessories folder (`ACCESSORIES`, the core's) as Windows 95, 98 and 2000 called it.
// Today's Windows still has the folder under this name on the disk and only labels it "Windows
// Accessories" (through its desktop.ini). Bevel95 shows the old name (the owner, 2026-11-11) and
// puts its own programs into that same folder, where the old Windows had their games.

/// The label today's Windows gives the Accessories folder here, if it gives one.
fn accessories_label() -> Option<String> {
    known_folder(&FOLDERID_CommonPrograms).and_then(|dir| desktop_ini(&dir.join(ACCESSORIES)).folder)
}

/// Writes the list of the programs ticked by hand. `before` is what the menu showed until now.
/// If that was not the ticked programs, the new list was started from another choice
/// (everything, or the classic programs), and a list ticked by hand earlier would be gone
/// with one click: that earlier list is kept beside it as programs.old.txt (one copy, which
/// Bevel95 never reads; the owner can put it back by renaming it).
pub fn save_ticked_programs<'a>(places: impl IntoIterator<Item = &'a str>, before: Shown) {
    use bevel_winkit::state_file::{self, Read};
    let path = ticked_file();
    let text = bevel_core::programs::ticked_file(places);
    let old = match state_file::read(&path) {
        Read::Text(old) => Some(old),
        Read::Missing => None,
        // Kept as it is, and not replaced by a list made without it.
        Read::Unreadable(why) => {
            crate::log::log(&format!("the ticked programs were not saved: {} cannot be read ({why}); it is kept as it is", crate::log::path_kind(&path)));
            return;
        }
    };
    if before != Shown::Ticked && let Some(old) = old && old != text {
        // A whole copy, or none: the earlier list is never lost half way.
        match state_file::write_whole(&path.with_file_name("programs.old.txt"), old.as_bytes()) {
            Ok(()) => crate::log::log("programs: the earlier list of ticked programs is kept as programs.old.txt"),
            Err(e) => {
                crate::log::log(&format!("could not keep the earlier list as programs.old.txt ({e}); the new list was not saved"));
                return;
            }
        }
    }
    if let Err(e) = state_file::save(&path, &text)
        && !state_file::is_read_only(&e)
    {
        crate::log::log(&format!("could not save the ticked programs to {}: {e}", crate::log::path_kind(&path)));
    }
}

/// "Remove Shortcuts/Folders" takes the programs at `places` out of the menu, which shows
/// `shown` now: what it shows becomes the ticked programs, without these, written to
/// programs.txt. Nothing on the disk is deleted. Returns how many programs are ticked now, and
/// how many there are.
pub fn hide_programs(shown: Shown, places: &[String]) -> (usize, usize) {
    let ticked = ticked_programs();
    let own = |item: &Item| (own_program(&item.target).is_some(), None);
    let mut rows = bevel_core::programs::rows(&all_programs(), ticked.as_ref(), own);
    bevel_core::programs::hide(&mut rows, shown, places);
    let left: Vec<&str> = bevel_core::programs::ticked_places(&rows).collect();
    let all = rows.iter().filter(|row| row.kind == bevel_core::programs::RowKind::Item).count();
    save_ticked_programs(left.iter().copied(), shown);
    (left.len(), all)
}

/// The mirror of `hide_programs`, for Taskbar Properties' Add...: the programs at `places`
/// (shortcuts the Create Shortcut wizard just made) join what the menu shows, which is `shown`
/// now. With everything, and with the ticked ones while there is no list yet (every program
/// counts as ticked), they show already: nothing is written, `None`. Otherwise what the menu
/// shows becomes the ticked programs, these with them (programs.txt, with the same safety copy
/// as Remove's), and the menu shows only the ticked ones from then on. Returns how many
/// programs are ticked now, and how many there are.
pub fn show_programs(shown: Shown, places: &[String]) -> Option<(usize, usize)> {
    let ticked = ticked_programs();
    if shown == Shown::Everything || (shown == Shown::Ticked && ticked.is_none()) {
        return None;
    }
    let own = |item: &Item| (own_program(&item.target).is_some(), None);
    let mut rows = bevel_core::programs::rows(&all_programs(), ticked.as_ref(), own);
    bevel_core::programs::unhide(&mut rows, shown, places);
    let now: Vec<&str> = bevel_core::programs::ticked_places(&rows).collect();
    let all = rows.iter().filter(|row| row.kind == bevel_core::programs::RowKind::Item).count();
    save_ticked_programs(now.iter().copied(), shown);
    Some((now.len(), all))
}

/// The places in the menu (as programs.txt names them) of the entries whose files are `files`
/// (compared without regard to case), for a shortcut found by its file.
pub fn places_of(files: &[PathBuf]) -> Vec<String> {
    fn walk(folder: &Folder, above: &mut Vec<String>, wanted: &[String], places: &mut Vec<String>) {
        for inner in &folder.folders {
            above.push(inner.name.clone());
            walk(inner, above, wanted, places);
            above.pop();
        }
        for item in &folder.items {
            if wanted.contains(&item.target.to_lowercase()) {
                let names: Vec<&str> = above.iter().map(String::as_str).collect();
                places.push(bevel_core::programs::place(&names, &item.name));
            }
        }
    }
    let wanted: Vec<String> = files.iter().map(|file| file.display().to_string().to_lowercase()).collect();
    let mut places = Vec::new();
    walk(&all_programs(), &mut Vec::new(), &wanted, &mut places);
    places
}

/// The list of Bevel95 Settings' Programs page: every program found on this PC, ticked as
/// programs.txt says (all of them while there is no such file), each with its picture.
pub fn program_rows(era: Era) -> Vec<Row> {
    let ticked = ticked_programs();
    bevel_core::programs::rows(&all_programs(), ticked.as_ref(), |item| {
        let icon = file_icon(era, &item.target);
        (own_program_icon(era, &item.target).is_some(), icon)
    })
}

/// Every program found on this PC as one merged Programs tree, sorted like Windows 95 (folders
/// first, then by name).
pub fn all_programs() -> Folder {
    let mut root = Folder::default();
    for id in [&FOLDERID_Programs, &FOLDERID_CommonPrograms] {
        if let Some(dir) = known_folder(id) {
            read_into(&mut root, &dir, &mut Vec::new());
        }
    }
    // Windows' Store-style apps have no shortcut files: they come from Windows' own list of
    // apps (as last read: `apps.rs`) and stand in Programs itself, where a program without a
    // folder of its own put its shortcut. A shortcut file of the same name there comes first.
    for app in crate::apps::cached() {
        root.add(&[], Item { name: app.name, target: app.target });
    }
    if let Ok(exe) = std::env::current_exe() && let Some(dir) = exe.parent() {
        add_own_programs(&mut root, dir);
    }
    root.sort();
    root
}

// Bevel95's own programs: the table is the core's (`bevel_core::programs::OWN_PROGRAMS`), so that
// the README's picture of the Programs menu is drawn from the same list.

/// The look the Programs menu is put together for (Windows Explorer's place depends on it).
static ERA: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// The bar's look changed (or the bar started).
pub fn set_era(era: Era) {
    let n = Era::ALL.iter().position(|&e| e == era).unwrap_or(0) as u8;
    ERA.store(n, std::sync::atomic::Ordering::SeqCst);
}

pub(crate) fn era_now() -> Era {
    Era::ALL.get(ERA.load(std::sync::atomic::Ordering::SeqCst) as usize).copied().unwrap_or(Era::Win95)
}

/// The viewer and the folder windows accompany every install; the other companions also appear
/// in Start (each file once).
pub(crate) fn companions() -> impl Iterator<Item = (&'static str, &'static str)> {
    [("Bevel95 Help", "bevel-help.exe"), ("Bevel95 folder windows", "folders.exe")]
        .into_iter()
        .chain(OWN_PROGRAMS.iter().filter(|p| p.file != "folders.exe").map(|p| (p.name, p.file)))
}

/// Packaged copies live in programs/. Earlier portable copies and Cargo builds are flat.
pub(crate) fn companion_file(root: &Path, file: &str) -> Option<PathBuf> {
    [root.join("programs").join(file), root.join(file)].into_iter().find(|path| path.is_file())
}

/// The full path to a program shipped with the running bar.
fn own_program_path(file: &str) -> Option<PathBuf> {
    companion_file(std::env::current_exe().ok()?.parent()?, file)
}

/// Lists Bevel95's own programs in the Programs menu where Windows 95, 98 and 2000 kept them:
/// the games under Accessories > Games, Calculator under Accessories. Nothing is written to the
/// Start Menu folders: the entry is there while the program's file is present.
fn add_own_programs(root: &mut Folder, dir: &Path) {
    bevel_core::programs::add_own_programs(root, era_now(), |program| {
        companion_file(dir, program.file).map(|path| path.display().to_string())
    });
}

/// Which of Bevel95's own programs `path` is the file of, if any (its number in `OWN_PROGRAMS`).
fn own_program(path: &str) -> Option<usize> {
    let file = Path::new(path).file_name()?.to_str()?;
    OWN_PROGRAMS.iter().position(|program| {
        program.file.eq_ignore_ascii_case(file)
            && own_program_path(program.file).is_some_and(|own| own.display().to_string().eq_ignore_ascii_case(path))
    })
}

/// What starts one of Bevel95's own programs (`file`, as in `OWN_PROGRAMS`), as its Programs
/// menu entry does; `None` when its file is missing.
pub fn own_program_target(file: &str) -> Option<String> {
    own_program_path(file).map(|p| p.display().to_string())
}

/// Whether `path` is the file of one of Bevel95's own programs (Find puts them first among
/// rows that rank the same).
pub fn is_own_program(path: &str) -> bool {
    own_program(path).is_some()
}

/// The picture of one of Bevel95's own programs, if `path` is its file.
fn own_program_icon(era: Era, path: &str) -> Option<Icon> {
    own_program(path).map(|i| (OWN_PROGRAMS[i].icon)(era))
}

/// Your own Programs folder (where the Properties window's Add... puts new shortcuts).
pub fn programs_folder() -> Option<PathBuf> {
    known_folder(&FOLDERID_Programs)
}

/// Your own Start Menu folder (the Properties window's Advanced... opens it).
pub fn start_menu_folder() -> Option<PathBuf> {
    known_folder(&FOLDERID_StartMenu)
}

/// Your Favorites folder (links and their folders), sorted the same way.
pub fn favorites() -> Folder {
    let mut root = Folder::default();
    if let Some(dir) = known_folder(&FOLDERID_Favorites) {
        read_into(&mut root, &dir, &mut Vec::new());
    }
    root.sort();
    root
}

/// Your recently opened documents, as `era`'s Documents menu lists them (the 15 most recent,
/// in that Windows' order). Windows 11 also records things in its Recent folder that Windows
/// 95 to 2000 never put in this menu: links that start apps (Snipping Tool's screen clips,
/// Game Bar), Control Panel pages and folders. Only shortcuts to files are kept, and of those
/// only the ones whose file is not known to be gone (`known_gone`; the owner's choice: Windows
/// 95 to 2000 listed those too, but Windows 11 keeps them for every file ever opened).
pub fn recent_documents(era: Era) -> Vec<Item> {
    let windows_drive = windows_drive();
    let mut found: Vec<(std::time::SystemTime, u64, PathBuf)> = Vec::new();
    if let Some(dir) = known_folder(&FOLDERID_Recent)
        && let Ok(entries) = fs::read_dir(dir)
    {
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_file() || meta.file_attributes() & HIDDEN_OR_SYSTEM != 0 || !has_extension(&path, &["lnk"]) {
                continue;
            }
            let modified = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
            found.push((modified, meta.creation_time(), path));
        }
    }
    found.sort_by_key(|f| std::cmp::Reverse(f.0));
    let mut docs: Vec<(Item, u64)> = found
        .into_iter()
        .filter(|(_, _, path)| {
            fs::read(path).is_ok_and(|bytes| links_to_a_file(&bytes) && !known_gone(&bytes, windows_drive))
        })
        .take(RECENT_COUNT)
        .map(|(_, created, path)| {
            let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            (Item { name: display_name(&stem), target: path.display().to_string() }, created)
        })
        .collect();
    start::order_documents(era, &mut docs);
    docs.into_iter().map(|(item, _)| item).collect()
}

/// Whether a shortcut (MS-SHLLINK) points at a file: it carries a place on a disk or network
/// (LinkInfo, flag 0x2 at 0x14) and that place is no folder (attribute 0x10 at 0x18). Links
/// that start apps or open Control Panel pages carry no such place.
fn links_to_a_file(lnk: &[u8]) -> bool {
    let word = |at: usize| lnk.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    match (word(0), word(0x14), word(0x18)) {
        (Some(0x4C), Some(flags), Some(attributes)) => flags & 0x2 != 0 && attributes & 0x10 == 0,
        _ => false,
    }
}

/// Whether a shortcut's file is known to be gone: the shortcut records a path on the Windows
/// drive and nothing is there any more. Only the Windows drive is asked: it is always awake, so
/// the answer takes microseconds (the file's attributes, the file itself is not opened), where a
/// sleeping disk would hold the menu up for seconds. Paths on other drives, network places and
/// shortcuts without a recorded path count as present.
fn known_gone(lnk: &[u8], windows_drive: u8) -> bool {
    let Some(mut path) = local_path(lnk) else { return false };
    let colon_backslash = path.get(1..3) == Some(&[u16::from(b':'), u16::from(b'\\')][..]);
    let letter = path.first().and_then(|&c| u8::try_from(c).ok());
    if !colon_backslash || !letter.is_some_and(|l| l.eq_ignore_ascii_case(&windows_drive)) {
        return false;
    }
    path.push(0);
    // SAFETY: a null-terminated path; GetLastError right after the call that set it.
    unsafe {
        GetFileAttributesW(path.as_ptr()) == INVALID_FILE_ATTRIBUTES
            && matches!(GetLastError(), ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND)
    }
}

/// The letter of the drive Windows is on (from %SystemDrive%, "C:").
fn windows_drive() -> u8 {
    std::env::var("SystemDrive").ok().and_then(|d| d.bytes().next()).unwrap_or(b'C')
}

/// The path a shortcut (MS-SHLLINK) records for its file on a local disk, in UTF-16: LinkInfo's
/// local base path followed by its common path suffix (their Unicode copies when the link has
/// them, else the ANSI ones, in this PC's code page). `None` for shortcuts without LinkInfo or
/// whose LinkInfo names only a network place.
pub(crate) fn local_path(lnk: &[u8]) -> Option<Vec<u16>> {
    let u16_at = |at: usize| lnk.get(at..at + 2).map(|b| usize::from(u16::from_le_bytes([b[0], b[1]])));
    let u32_at = |at: usize| lnk.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize);
    let flags = u32_at(0x14)?;
    let mut info = 0x4C;
    if flags & 0x1 != 0 {
        info += 2 + u16_at(info)?; // the target's item list comes first
    }
    if flags & 0x2 == 0 || u32_at(info + 8)? & 0x1 == 0 {
        return None;
    }
    let mut path = Vec::new();
    if u32_at(info + 4)? >= 0x24 {
        // LocalBasePathOffsetUnicode, CommonPathSuffixOffsetUnicode
        for field in [28, 32] {
            let offset = u32_at(info + field)?;
            if offset == 0 {
                continue;
            }
            let text = lnk.get(info + offset..)?;
            path.extend(text.as_chunks::<2>().0.iter().map(|&c| u16::from_le_bytes(c)).take_while(|&c| c != 0));
        }
    } else {
        // LocalBasePathOffset, CommonPathSuffixOffset
        for field in [16, 24] {
            let text = lnk.get(info + u32_at(info + field)?..)?;
            path.extend(ansi_to_utf16(&text[..text.iter().position(|&b| b == 0)?])?);
        }
    }
    Some(path)
}

/// Text in this PC's ANSI code page, as UTF-16.
fn ansi_to_utf16(text: &[u8]) -> Option<Vec<u16>> {
    if text.is_empty() {
        return Some(Vec::new());
    }
    let mut out = vec![0u16; text.len()];
    // SAFETY: both buffers are valid for the lengths given (one UTF-16 unit per byte at most).
    let n = unsafe { MultiByteToWideChar(CP_ACP, 0, text.as_ptr(), text.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    out.truncate(usize::try_from(n).ok().filter(|&n| n > 0)?);
    Some(out)
}

/// A file's name as Explorer shows it ("zebra" for zebra.txt while known extensions are
/// hidden, Windows' default), from the name alone: the file may be gone.
fn display_name(file_name: &str) -> String {
    let wide_name = wide(file_name);
    let mut info = SHFILEINFOW::default();
    let flags = SHGFI_DISPLAYNAME | SHGFI_USEFILEATTRIBUTES;
    // SAFETY: valid pointers; with SHGFI_USEFILEATTRIBUTES the file itself is not touched.
    let ok = unsafe {
        SHGetFileInfoW(wide_name.as_ptr(), FILE_ATTRIBUTE_NORMAL, &mut info, size_of::<SHFILEINFOW>() as u32, flags)
    } != 0;
    let len = info.szDisplayName.iter().position(|&c| c == 0).unwrap_or(0);
    if ok && len > 0 { String::from_utf16_lossy(&info.szDisplayName[..len]) } else { file_name.to_string() }
}

/// Your Documents folder ("My Documents" at the top of the Documents menu in 98 and 2000).
pub use bevel_winkit::shell::documents_folder;
fn read_into(root: &mut Folder, dir: &Path, path: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let names_here = desktop_ini(dir);
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if meta.file_attributes() & HIDDEN_OR_SYSTEM != 0 {
            continue;
        }
        let entry_path = entry.path();
        if meta.is_dir() {
            let on_disk = entry.file_name().to_string_lossy().into_owned();
            // The folder's name as Windows shows it, but for Accessories its old one.
            let shown = if path.is_empty() && on_disk.eq_ignore_ascii_case(ACCESSORIES) {
                ACCESSORIES.to_string()
            } else {
                desktop_ini(&entry_path).folder.unwrap_or(on_disk)
            };
            path.push(shown);
            let names: Vec<&str> = path.iter().map(String::as_str).collect();
            root.folder_mut(&names); // so an empty folder still shows
            read_into(root, &entry_path, path);
            path.pop();
        } else if has_extension(&entry_path, &PROGRAM_EXTENSIONS) {
            let names: Vec<&str> = path.iter().map(String::as_str).collect();
            let mut item = item(&entry_path);
            if let Some(shown) = names_here.files.get(&entry.file_name().to_string_lossy().to_lowercase()) {
                item.name = shown.clone();
            }
            root.add(&names, item);
        }
    }
}

fn item(path: &Path) -> Item {
    let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    Item { name, target: path.display().to_string() }
}

/// The names Explorer shows instead of file names, from a folder's hidden desktop.ini: the
/// folder's own (`LocalizedResourceName`) and its files' (`[LocalizedFileNames]`, keyed by
/// lowercase file name). This is how "dfrgui.lnk" shows as "Defragment and Optimize Drives".
#[derive(Default)]
struct DisplayNames {
    folder: Option<String>,
    files: HashMap<String, String>,
}

fn desktop_ini(dir: &Path) -> DisplayNames {
    let mut names = DisplayNames::default();
    let Ok(bytes) = fs::read(dir.join("desktop.ini")) else {
        return names;
    };
    let text = match bytes.strip_prefix(&[0xFF, 0xFE]) {
        Some(utf16) => String::from_utf16_lossy(
            &utf16.as_chunks::<2>().0.iter().map(|&b| u16::from_le_bytes(b)).collect::<Vec<_>>(),
        ),
        None => String::from_utf8_lossy(&bytes).into_owned(),
    };
    let mut section = String::new();
    for line in text.lines().map(str::trim) {
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = name.to_lowercase();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        let Some(shown) = resolve(value.trim()) else { continue };
        match section.as_str() {
            ".shellclassinfo" if key.trim().eq_ignore_ascii_case("LocalizedResourceName") => names.folder = Some(shown),
            "localizedfilenames" => {
                names.files.insert(key.trim().to_lowercase(), shown);
            }
            _ => {}
        }
    }
    names
}

/// "@%SystemRoot%\system32\shell32.dll,-22035" style references become the text Windows
/// shows (in your language); plain text stays as it is.
fn resolve(value: &str) -> Option<String> {
    if !value.starts_with('@') {
        return (!value.is_empty()).then(|| value.to_string());
    }
    let source = wide(&expand_environment(value));
    let mut buf = [0u16; 512];
    // SAFETY: a null-terminated source and an output buffer whose length is passed.
    let hr = unsafe { SHLoadIndirectString(source.as_ptr(), buf.as_mut_ptr(), buf.len() as u32, null()) };
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    (hr == 0 && len > 0).then(|| String::from_utf16_lossy(&buf[..len]))
}

fn expand_environment(text: &str) -> String {
    let source = wide(text);
    let mut buf = vec![0u16; 1024];
    // SAFETY: the buffer length is passed; the result is cut at its terminating null.
    let n = unsafe { ExpandEnvironmentStringsW(source.as_ptr(), buf.as_mut_ptr(), buf.len() as u32) } as usize;
    if n == 0 || n > buf.len() {
        return text.to_string();
    }
    String::from_utf16_lossy(&buf[..n - 1])
}

/// The files that are entries of the Programs menu (other files in its folders are not).
const PROGRAM_EXTENSIONS: [&str; 5] = ["lnk", "url", "exe", "appref-ms", "pif"];

/// The files directly in `dir` that the Programs menu shows as entries (not those in its
/// folders), by their paths. For Add...: what the Create Shortcut wizard added.
pub fn program_files(dir: &Path) -> std::collections::BTreeSet<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Default::default();
    };
    entries
        .flatten()
        .filter(|entry| {
            entry.metadata().is_ok_and(|meta| meta.is_file() && meta.file_attributes() & HIDDEN_OR_SYSTEM == 0)
                && has_extension(&entry.path(), &PROGRAM_EXTENSIONS)
        })
        .map(|entry| entry.path())
        .collect()
}

fn has_extension(path: &Path, extensions: &[&str]) -> bool {
    path.extension().is_some_and(|e| extensions.iter().any(|x| e.eq_ignore_ascii_case(x)))
}

pub use bevel_winkit::shell::known_folder;

/// Icons already read, by file path (reading one asks the shell, which takes a moment). Shared
/// by all threads: `read_icons_ahead` fills it on a thread of its own.
static FILE_ICONS: LazyLock<Mutex<HashMap<String, Option<Icon>>>> = LazyLock::new(Default::default);

/// How long after the start the Start menu's icons are read ahead (the bar and the
/// notification area settle first).
const READ_AHEAD_AFTER: Duration = Duration::from_secs(3);

/// Reads the icons of every Programs and Favorites entry ahead of time, on a thread of its own
/// at background priority, so the first opening of a menu does not wait for the shell (the
/// 113 entries of the owner's Programs menu took 0.5 to 2 s the first time).
pub fn read_icons_ahead(era: Era) {
    set_era(era);
    let spawned = std::thread::Builder::new().name("start menu icons".into()).spawn(move || {
        std::thread::sleep(READ_AHEAD_AFTER);
        // SAFETY: plain calls on this thread's own handle.
        unsafe { SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN) };
        crate::audio::init();
        let started = Instant::now();
        let mut count = 0;
        for tree in [all_programs(), favorites()] {
            read_folder_icons(&tree, &mut count, era);
        }
        crate::log::log(&format!("start menu: read {count} icons ahead in {} ms", started.elapsed().as_millis()));
        // Quick Search's list too (its shortcuts' targets are read on a thread of its own), so
        // that its first opening already knows which entries are programs.
        crate::find_window::refresh();
    });
    if let Err(e) = spawned {
        crate::log::log(&format!("could not start reading the Start menu's icons ahead: {e}"));
    }
}

fn read_folder_icons(folder: &Folder, count: &mut usize, era: Era) {
    for item in &folder.items {
        file_icon(era, &item.target);
        *count += 1;
    }
    for inner in &folder.folders {
        read_folder_icons(inner, count, era);
    }
}

/// IID_IImageList, from CommonControls.h.
const IID_IMAGE_LIST: GUID = GUID::from_u128(0x46EB5926_582E_4017_9FDF_E8998DAA0950);
/// IImageList::GetIcon: the 11th method (after IUnknown's three and Add, ReplaceIcon,
/// SetOverlayImage, Replace, AddMasked, Draw, Remove).
const GET_ICON_SLOT: usize = 10;
type GetIcon = unsafe extern "system" fn(*mut c_void, i32, u32, *mut HICON) -> HRESULT;

thread_local! {
    /// The shell's list of small icons (a COM IImageList, kept for the life of the program),
    /// asked for on each thread that reads icons.
    static IMAGE_LIST: Cell<*mut c_void> = const { Cell::new(null_mut()) };
}

/// The 16×16 icon the shell shows for a file: a shortcut shows its target's icon. Taken
/// from the shell's icon list rather than asked for directly, because only that way comes
/// without the shortcut arrow, which Windows 95's Start menu did not show. Read with the
/// thread at 96 DPI so the list holds real 16×16 pictures.
pub fn file_icon(era: Era, path: &str) -> Option<Icon> {
    // The shell cache is shared between looks. Each built-in program supplies its own
    // edition's icon directly, including after a theme change.
    if let Some(own) = own_program_icon(era, path) {
        return Some(own);
    }
    if let Some(known) = FILE_ICONS.lock().ok().and_then(|cache| cache.get(path).cloned()) {
        return known;
    }
    let _dpi = DpiUnaware::enter();
    // SAFETY: the image list is the documented IImageList, called through its vtable; the icon
    // it hands out is ours to destroy.
    let icon = unsafe {
        let index = icon_index(path);
        let list = image_list();
        match index {
            Some(index) if !list.is_null() => {
                let vtable = *(list as *const *const usize);
                let get_icon: GetIcon = std::mem::transmute(*vtable.add(GET_ICON_SLOT));
                let mut handle: HICON = null_mut();
                if get_icon(list, index, ILD_TRANSPARENT, &mut handle) != 0 || handle.is_null() {
                    None
                } else {
                    let pixels = icon_pixels(handle);
                    DestroyIcon(handle);
                    pixels
                }
            }
            _ => None,
        }
    };
    if let Ok(mut cache) = FILE_ICONS.lock() {
        cache.insert(path.to_string(), icon.clone());
    }
    icon
}

/// Where the shell keeps the small icon of `path` in its list of icons: a file's, or that of a
/// Store-style app (`APP_PREFIX` and its ID). An app has no file, so the shell is asked by the
/// app's place among its own things (an item list) instead of by a path.
fn icon_index(path: &str) -> Option<i32> {
    let wide_path = wide(path);
    let mut info = SHFILEINFOW::default();
    let (size, flags) = (size_of::<SHFILEINFOW>() as u32, SHGFI_SYSICONINDEX | SHGFI_SMALLICON);
    // SAFETY: a null-terminated name and a correctly sized out-struct; with SYSICONINDEX
    // nothing of the result needs freeing. The item list the shell makes is freed again.
    unsafe {
        if !path.starts_with(APP_PREFIX) {
            return (SHGetFileInfoW(wide_path.as_ptr(), 0, &mut info, size, flags) != 0).then_some(info.iIcon);
        }
        let mut id: *mut ITEMIDLIST = null_mut();
        if SHParseDisplayName(wide_path.as_ptr(), null_mut(), &mut id, 0, null_mut()) != 0 || id.is_null() {
            return None;
        }
        let found = SHGetFileInfoW(id as *const u16, 0, &mut info, size, flags | SHGFI_PIDL) != 0;
        ILFree(id);
        found.then_some(info.iIcon)
    }
}


fn image_list() -> *mut c_void {
    IMAGE_LIST.with(|cell| {
        if cell.get().is_null() {
            let mut list: *mut c_void = null_mut();
            // SAFETY: a valid IID and out-pointer; the reference is kept for good.
            if unsafe { SHGetImageList(SHIL_SMALL as i32, &IID_IMAGE_LIST, &mut list) } == 0 {
                cell.set(list);
            } else {
                crate::log::log("SHGetImageList failed: Start menu items get no icons");
            }
        }
        cell.get()
    })
}

/// File Explorer's Home (Windows 11), by its folder's CLSID.
const FILE_EXPLORER_HOME: &str = "shell:::{f874310e-b6b7-47dc-bc84-b9e6b38f5903}";

/// Opens what a Settings or Find entry stands for, in today's Windows.
pub fn open_target(target: Target) {
    match target {
        // Windows' own programs by their whole path (the third safety review of 2026-11-18, C1).
        Target::ControlPanel => launch(&bevel_winkit::shell::system_program("control.exe")),
        Target::Printers => launch("ms-settings:printers"),
        Target::Network => launch(&bevel_winkit::shell::system_program("ncpa.cpl")),
        Target::BarSettings => {
            // Opened by the bar once the menu is closed.
            let bar = crate::bar::BAR_HWND.load(std::sync::atomic::Ordering::SeqCst) as windows_sys::Win32::Foundation::HWND;
            // SAFETY: posting to our own bar window.
            unsafe { windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(bar, crate::bar::WM_OPEN_PROPERTIES.id(), 0, 0) };
        }
        // File Explorer Options, today's Folder Options.
        Target::FolderOptions => launch("shell:::{6DFD7C5C-2451-11d3-A299-00C04F8EF6AF}"),
        Target::WindowsUpdate => launch("ms-settings:windowsupdate"),
        // Today's counterpart of "Find: All Files": File Explorer's Home, its search box ready
        // ("Search Home"). "search-ms:" alone, which this used to open, opens nothing on
        // Windows 11 while ShellExecute reports success; a search-ms: address opens a window
        // only with a query, and an empty one lists every file under its place (decisions.md,
        // "Settings and Find pictures").
        Target::FindFiles => {
            crate::log::log("Find > Files or Folders: opening File Explorer's Home");
            launch(FILE_EXPLORER_HOME);
        }
        Target::Internet => launch("https://www.bing.com/"),
        // Opened by the bar once the menu is closed.
        Target::FindPrograms => {
            let bar = crate::bar::BAR_HWND.load(std::sync::atomic::Ordering::SeqCst) as windows_sys::Win32::Foundation::HWND;
            let code = bevel_core::bar_menu::BarCommand::Find.code();
            // SAFETY: posting to our own bar window.
            unsafe { windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(bar, crate::bar::WM_BAR_COMMAND.id(), code, 0) };
        }
    }
}

/// The signed-in user's name ("Log Off <name>..." in Windows 98 and 2000).
pub fn user_name() -> String {
    let mut buf = [0u16; 257];
    let mut len = buf.len() as u32;
    // SAFETY: the buffer length is passed; on success `len` counts the terminating null.
    if unsafe { GetUserNameW(buf.as_mut_ptr(), &mut len) } == 0 || len == 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buf[..len as usize - 1])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn companion_lookup_prefers_installed_programs_and_accepts_flat_portable_copies() {
        let root = std::env::temp_dir().join(format!("bevel95-companion-test-{}", std::process::id()));
        let file = "minefield.exe";
        fs::create_dir_all(root.join("programs")).unwrap();
        assert_eq!(companion_file(&root, file), None);
        let legacy = root.join(file);
        fs::write(&legacy, []).unwrap();
        assert_eq!(companion_file(&root, file), Some(legacy));
        let installed = root.join("programs").join(file);
        fs::write(&installed, []).unwrap();
        assert_eq!(companion_file(&root, file), Some(installed));
        let solitaire = root.join("programs/solitaire.exe");
        fs::write(&solitaire, []).unwrap();
        fs::write(root.join("programs/bevel-help.exe"), []).unwrap();
        let mut programs = Folder::default();
        add_own_programs(&mut programs, &root);
        let games = &programs.folders[0].folders[0];
        assert_eq!(games.name, "Games");
        assert_eq!(games.items.iter().map(|item| item.name.as_str()).collect::<Vec<_>>(), ["Minesweeper", "Solitaire"]);
        assert_eq!(Path::new(&games.items[1].target), solitaire);
        fs::remove_dir_all(root).unwrap();
    }

    /// A shortcut file (MS-SHLLINK) whose LinkInfo records `base` + `suffix` on a local disk:
    /// ANSI copies (`ansi`, `ansi_suffix`; ASCII here), Unicode copies when `unicode`, and an
    /// item list in front when `item_list`.
    fn shortcut(ansi: &str, ansi_suffix: &str, unicode: Option<(&str, &str)>, item_list: bool) -> Vec<u8> {
        let mut lnk = vec![0u8; 0x4C];
        lnk[0] = 0x4C;
        let flags: u32 = 0x2 | 0x80 | u32::from(item_list);
        lnk[0x14..0x18].copy_from_slice(&flags.to_le_bytes());
        lnk[0x18..0x1C].copy_from_slice(&0x20u32.to_le_bytes());
        if item_list {
            // one item of 4 bytes, then the terminating zero
            lnk.extend_from_slice(&[6, 0, 4, 0, 0x1F, 0x50, 0, 0]);
        }
        let header = if unicode.is_some() { 0x24 } else { 0x1C };
        let mut info = vec![0u8; header];
        let put = |info: &mut Vec<u8>, at: usize, value: usize| {
            info[at..at + 4].copy_from_slice(&(value as u32).to_le_bytes());
        };
        put(&mut info, 4, header);
        put(&mut info, 8, 1);
        put(&mut info, 12, header);
        // VolumeID: size, drive type 3 (fixed), serial number, label offset, empty label
        info.extend_from_slice(&[0x11, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0x10, 0, 0, 0, 0]);
        for (field, text) in [(16, ansi), (24, ansi_suffix)] {
            let at = info.len();
            put(&mut info, field, at);
            info.extend(text.bytes().chain([0]));
        }
        if let Some((base, suffix)) = unicode {
            for (field, text) in [(28, base), (32, suffix)] {
                let at = info.len();
                put(&mut info, field, at);
                info.extend(text.encode_utf16().chain([0]).flat_map(u16::to_le_bytes));
            }
        }
        let size = info.len();
        put(&mut info, 0, size);
        lnk.extend(info);
        lnk
    }

    fn text(path: Option<Vec<u16>>) -> Option<String> {
        path.map(|p| String::from_utf16_lossy(&p))
    }

    #[test]
    fn shortcut_paths_are_read_like_windows() {
        let full = shortcut(r"C:\Temp\notes.txt", "", None, false);
        assert_eq!(text(local_path(&full)).as_deref(), Some(r"C:\Temp\notes.txt"));
        // the base path and the suffix are joined
        let split = shortcut(r"C:\Temp\", "notes.txt", None, true);
        assert_eq!(text(local_path(&split)).as_deref(), Some(r"C:\Temp\notes.txt"));
        // the Unicode copy wins over the ANSI one (which may have lost letters)
        let unicode = shortcut(r"C:\Temp\caf?.txt", "", Some((r"C:\Temp\café.txt", "")), true);
        assert_eq!(text(local_path(&unicode)).as_deref(), Some(r"C:\Temp\café.txt"));
        // a network place only (LinkInfo flag 0x2 without 0x1)
        let mut network = full.clone();
        network[0x4C + 8] = 2;
        assert_eq!(local_path(&network), None);
        // no LinkInfo at all
        let mut no_info = full.clone();
        no_info[0x14] = 0x80;
        assert_eq!(local_path(&no_info), None);
        // cut short anywhere: no path, no panic
        for len in 0..full.len() {
            let _ = local_path(&full[..len]);
        }
    }

    #[test]
    fn only_files_known_to_be_gone_are_left_out() {
        let drive = windows_drive();
        let windows = std::env::var("SystemRoot").unwrap();
        assert!(!known_gone(&shortcut(&windows, "", None, false), drive));
        let missing = format!(r"{}:\bevel95-test-nothing-here\notes.txt", char::from(drive));
        assert!(known_gone(&shortcut(&missing, "", None, false), drive));
        // other drives are not asked (a sleeping disk would hold the menu up)
        let other = if drive.eq_ignore_ascii_case(&b'Q') { 'R' } else { 'Q' };
        assert!(!known_gone(&shortcut(&format!(r"{other}:\nothing-here.txt"), "", None, false), drive));
        assert!(!known_gone(&shortcut(r"\\server\share\notes.txt", "", None, false), drive));
    }
}
