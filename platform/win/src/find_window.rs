//! Quick Search (until 2026-11-16 Find: Programs), Bevel95's own window for starting a
//! program by a few letters of its name, opened by the Windows key (`windows_key = find`, the
//! default), the Start menu's Find submenu and `BarCommand::Find`. `bevel_core::find` draws it
//! and handles its keys and mouse (`FindWindow`, a window of the control kit); this module keeps
//! the list of programs, fetches the pictures of the rows that show and starts the chosen one.
//!
//! The window appears at once: it shows the list of programs as last read, and reads it again
//! on a thread of its own each time it opens (Windows' list of apps too, as the Start menu
//! does). The first opening before any reading reads it right away (as the Start menu does each
//! time it opens).

use std::ffi::c_void;
use std::ptr::null_mut;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use bevel_core::config::QuickSearch;
use bevel_core::find::{self, FindWindow, Part, Program};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Gdi::{CreateRectRgn, SetWindowRgn};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    HWND_NOTOPMOST, PostMessageW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos,
};

use bevel_core::quick::Action;
use bevel_core::theme::{Depth, Era};

use crate::bar::{BAR_HWND, WM_BAR_COLORS, WM_BAR_COMMAND, WM_BAR_SIZE, WM_BAR_THEME, with_app};
use crate::kit::{self, Hosted};
use crate::log::{data_dir, log};
use bevel_winkit::state_file;
use crate::audio::{method, release};
use crate::util::{take_focus, wide};
use crate::{start_menu, window_list};

/// Posted to the bar when a reading of the programs (on its own thread) is done.
pub const WM_FIND_PROGRAMS_READ: u32 = windows_sys::Win32::UI::WindowsAndMessaging::WM_APP + 12;

/// The programs as last read (`None` until the first reading), and whether a reading is going on.
static PROGRAMS: Mutex<Option<Vec<Program>>> = Mutex::new(None);
static READING: AtomicBool = AtomicBool::new(false);

/// The window, while it is open.
pub struct FindHost {
    form: FindWindow,
    /// How many ticks of `TIMER` a scroll bar button has been held down.
    held: u32,
}

/// The window's timer (the host keeps 1 for the caret): it fetches the pictures of rows that
/// came into view, and keeps a held scroll bar button scrolling after a quarter of a second.
const TIMER: usize = 2;
const TICK_MS: u32 = 50;
const HOLD_TICKS: u32 = 5;

impl Hosted for FindHost {
    type F = FindWindow;
    fn form(&self) -> &FindWindow {
        &self.form
    }
    fn form_mut(&mut self) -> &mut FindWindow {
        &mut self.form
    }
    fn activate(part: Part) {
        activate(part);
    }
    fn timers() -> &'static [(usize, u32)] {
        &[(TIMER, TICK_MS)]
    }
    fn timer(&mut self, id: usize) -> bool {
        if id != TIMER {
            return false;
        }
        let pictures = fetch_icons(&mut self.form);
        let scrolled = match self.form.pressed {
            Some(Part::ScrollUp | Part::ScrollDown) => {
                self.held += 1;
                self.held > HOLD_TICKS && find::scroll_on(&mut self.form)
            }
            _ => {
                self.held = 0;
                false
            }
        };
        pictures || scrolled
    }
}

/// Fetches the pictures of the rows that show and have none yet (from the Start menu's store
/// of them, read ahead soon after the bar starts). True if any was fetched.
fn fetch_icons(form: &mut FindWindow) -> bool {
    let wanted = form.wanted_icons();
    for &i in &wanted {
        let icon = start_menu::file_icon(form.era, &form.programs[i].target);
        form.set_icon(i, icon);
    }
    !wanted.is_empty()
}

/// Reads every program the Programs menu could show (with "everything" chosen, whatever the
/// owner's choice for the menu is). Bevel95's own are marked: they come first among equals.
/// Each entry's kind of target is found out (`find::Target`): with `look_inside`, a shortcut is
/// opened and its target read (that reads the disk: only on a thread of its own); without, only
/// the entry's own file name tells (an Internet shortcut is a web page, a shortcut is unknown).
fn read_programs(look_inside: bool) -> Vec<Program> {
    let mut programs = find::programs(&start_menu::all_programs());
    for program in &mut programs {
        program.own = start_menu::is_own_program(&program.target);
        program.kind = if program.own { find::Target::Program } else { target_kind(&program.target, look_inside) };
    }
    programs
}

/// What the Programs menu entry `entry` (a file's full path, or a Store-style app's name)
/// really opens. A shortcut (.lnk) is read with Windows' ShellLink object when `look_inside`;
/// a shortcut without a file target (a shell object, such as a Control Panel item) is
/// `Unknown`. Any other entry is told by its own name.
fn target_kind(entry: &str, look_inside: bool) -> find::Target {
    let is_link = is_shortcut(entry);
    if !is_link {
        return find::classify(entry);
    }
    if !look_inside {
        return find::Target::Unknown;
    }
    match link_target(entry) {
        // Windows Installer's "advertised" shortcut starts its program through the installer;
        // the path it names is only the program's picture.
        Some(Link::Advertised) => find::Target::Program,
        // "A folder" as the shortcut itself recorded it: the target is never asked (a dead
        // network place would stall the reading, a sleeping disk would wake).
        Some(Link::Path { folder: true, .. }) => find::Target::Folder,
        Some(Link::Path { path, .. }) => find::classify(&path),
        None => find::Target::Unknown,
    }
}

/// Whether `entry` is a shortcut's file (its name ends in ".lnk", case as it may be). Compared
/// as bytes: the fourth byte from the end may fall inside a letter of another alphabet, where
/// cutting the text would panic.
fn is_shortcut(entry: &str) -> bool {
    let bytes = entry.as_bytes();
    bytes.len() > 4 && bytes[bytes.len() - 4..].eq_ignore_ascii_case(b".lnk")
}

/// What a shortcut was found to point at.
enum Link {
    /// A file or folder, as the shortcut names it, and whether the shortcut recorded it as a
    /// folder (the target's attributes kept in the shortcut when it was made).
    Path { path: String, folder: bool },
    /// Windows Installer's advertised shortcut (SLDF_HAS_DARWINID): a program of an installed
    /// package.
    Advertised,
}

/// IID_IShellLinkW, IID_IPersistFile and IID_IShellLinkDataList (ShObjIdl.h, ObjIdl.h).
const IID_SHELL_LINK: windows_sys::core::GUID = windows_sys::core::GUID::from_u128(0x000214F9_0000_0000_C000_000000000046);
const IID_PERSIST_FILE: windows_sys::core::GUID = windows_sys::core::GUID::from_u128(0x0000010B_0000_0000_C000_000000000046);
const IID_SHELL_LINK_DATA_LIST: windows_sys::core::GUID = windows_sys::core::GUID::from_u128(0x45E2B4AE_B1C3_11D0_B92F_00A0C90312E1);
/// IShellLinkW::GetPath is the first method after IUnknown's three; IPersistFile::Load the
/// third after them (GetClassID, IsDirty, Load); IShellLinkDataList::GetFlags the fourth
/// (AddDataBlock, CopyDataBlock, RemoveDataBlock, GetFlags).
const GET_PATH: usize = 3;
const LOAD: usize = 5;
const GET_FLAGS: usize = 6;
/// SHELL_LINK_DATA_FLAGS' SLDF_HAS_DARWINID (ShlObj_core.h): an advertised shortcut.
const HAS_DARWIN_ID: u32 = 0x1000;
type QueryInterface = unsafe extern "system" fn(*mut c_void, *const windows_sys::core::GUID, *mut *mut c_void) -> windows_sys::core::HRESULT;
type Load = unsafe extern "system" fn(*mut c_void, windows_sys::core::PCWSTR, u32) -> windows_sys::core::HRESULT;
type GetPath = unsafe extern "system" fn(*mut c_void, *mut u16, i32, *mut c_void, u32) -> windows_sys::core::HRESULT;
type GetFlags = unsafe extern "system" fn(*mut c_void, *mut u32) -> windows_sys::core::HRESULT;

/// The program file the shortcut `link` points at (`None`: it cannot be read, is advertised or
/// points at a folder: it then never matches a window; brief 114). Read only on the Windows 7
/// store's own thread (`seven_store`: a shortcut on a network place that does not answer must
/// never hold the bar's thread). COM must be usable on the calling thread.
pub fn read_shortcut_program(link: &str) -> Option<String> {
    match link_target(link) {
        Some(Link::Path { path, folder: false }) => Some(path),
        _ => None,
    }
}

/// What the shortcut `link` points at, as it stands in the shortcut (nothing is searched for
/// or repaired: the shortcut is only read). `None` when it cannot be read or has no file
/// target. COM must be usable on this thread.
fn link_target(link: &str) -> Option<Link> {
    use windows_sys::Win32::Storage::FileSystem::{FILE_ATTRIBUTE_DIRECTORY, WIN32_FIND_DATAW};
    use windows_sys::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, STGM_READ};
    use windows_sys::Win32::UI::Shell::ShellLink;
    let mut shell_link: *mut c_void = null_mut();
    // SAFETY: CoCreateInstance with valid GUIDs and out-pointer; every method is called through
    // IShellLinkW's or IPersistFile's documented table with valid buffers, and every reference
    // obtained is released.
    unsafe {
        let hr = CoCreateInstance(&ShellLink, null_mut(), CLSCTX_INPROC_SERVER, &IID_SHELL_LINK, &mut shell_link);
        if hr < 0 || shell_link.is_null() {
            return None;
        }
        let mut persist: *mut c_void = null_mut();
        let hr = method::<QueryInterface>(shell_link, 0)(shell_link, &IID_PERSIST_FILE, &mut persist);
        let mut found = None;
        if hr >= 0 && !persist.is_null() {
            let name = wide(link);
            if method::<Load>(persist, LOAD)(persist, name.as_ptr(), STGM_READ) >= 0 {
                let mut data_list: *mut c_void = null_mut();
                let mut flags = 0u32;
                if method::<QueryInterface>(shell_link, 0)(shell_link, &IID_SHELL_LINK_DATA_LIST, &mut data_list) >= 0
                    && !data_list.is_null()
                {
                    if method::<GetFlags>(data_list, GET_FLAGS)(data_list, &mut flags) < 0 {
                        flags = 0;
                    }
                    release(data_list);
                }
                let mut buf = [0u16; 1024];
                // 0: the long path, environment names expanded. The find data is the target's
                // as the shortcut recorded it (MS-SHLLINK's header keeps its attributes); the
                // target itself is not asked (only Resolve would look for it).
                let mut data: WIN32_FIND_DATAW = std::mem::zeroed();
                let hr = method::<GetPath>(shell_link, GET_PATH)(
                    shell_link,
                    buf.as_mut_ptr(),
                    buf.len() as i32,
                    (&mut data as *mut WIN32_FIND_DATAW).cast(),
                    0,
                );
                let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
                if flags & HAS_DARWIN_ID != 0 {
                    found = Some(Link::Advertised);
                } else if hr == 0 && len > 0 {
                    let folder = data.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0;
                    found = Some(Link::Path { path: String::from_utf16_lossy(&buf[..len]), folder });
                }
            }
            release(persist);
        }
        release(shell_link);
        found
    }
}

/// The programs as Quick Search lists them now: only programs, unless the setting
/// `quick_search = everything` says otherwise (`find::listed`).
fn as_listed(programs: Vec<Program>) -> Vec<Program> {
    let everything = with_app(|app| app.quick_search == QuickSearch::Everything).unwrap_or(false);
    find::listed(programs, everything)
}

/// Reads the programs again on a thread of its own (not if a reading is going on); the bar
/// hears when it is done (`WM_FIND_PROGRAMS_READ`).
pub fn refresh() {
    if READING.swap(true, Ordering::SeqCst) {
        return;
    }
    let spawned = std::thread::Builder::new().name("find programs".into()).spawn(|| {
        use windows_sys::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
        // SAFETY: plain calls on this thread; every COM object the reading makes is released
        // before it returns, and a successful initialisation (S_OK or S_FALSE) is balanced.
        let com = unsafe { CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32) };
        let programs = read_programs(true);
        if com >= 0 {
            // SAFETY: balances the CoInitializeEx above, on the same thread.
            unsafe { CoUninitialize() };
        }
        if let Ok(mut known) = PROGRAMS.lock() {
            *known = Some(programs);
        }
        READING.store(false, Ordering::SeqCst);
        let bar = BAR_HWND.load(Ordering::SeqCst) as HWND;
        // SAFETY: posting to our own bar window (a stale handle just fails).
        unsafe { PostMessageW(bar, WM_FIND_PROGRAMS_READ, 0, 0) };
    });
    if let Err(e) = spawned {
        READING.store(false, Ordering::SeqCst);
        log(&format!("could not start reading the programs for Quick Search: {e}"));
    }
}

/// A reading is done: an open window shows the programs as read now (its text, its pictures
/// and its selected program stay).
pub fn programs_read() {
    if kit::hwnd::<FindHost>().is_none() {
        return;
    }
    let Some(programs) = PROGRAMS.lock().ok().and_then(|known| known.clone()) else {
        return;
    };
    let programs = as_listed(programs);
    let with_commands = find::with_commands(programs.clone());
    let same = kit::with::<FindHost, _>(|h| h.form.programs == with_commands).unwrap_or(true);
    if !same {
        log(&format!("Quick Search: the list of programs was read again ({} programs)", programs.len()));
        kit::update::<FindHost>(|h| h.form.set_programs(programs));
    }
}

/// Whether the window is open and the one in front.
pub fn in_front() -> bool {
    kit::hwnd::<FindHost>().is_some_and(|hwnd| window_list::foreground() == hwnd)
}

/// Opens the window, empty, where the Run dialog opens; if it is open already, it comes
/// forward as it is.
pub fn open() {
    if let Some(hwnd) = kit::hwnd::<FindHost>() {
        take_focus(hwnd);
        return;
    }
    let Some((era, monitor_left, bar_top, scale)) =
        with_app(|app| (app.era, app.monitor.left, app.bar_top_on_screen(), app.scale))
    else {
        return;
    };
    let started = Instant::now();
    let known = PROGRAMS.lock().ok().and_then(|known| known.clone());
    let programs = match known {
        Some(programs) => programs,
        // Not read yet (the bar reads it soon after it starts): read now, but without opening
        // the shortcuts, which would read the disk on this window's thread. The reading on a
        // thread of its own, started below, then tells their targets.
        None => read_programs(false),
    };
    let programs = as_listed(programs);
    let count = programs.len();
    let mut form = FindWindow::new(era, programs, true);
    if let Some(width) = state_file::load(&state_path()).text().and_then(|text| find::read_state(&text)) {
        form.name_w = width;
    }
    fetch_icons(&mut form);
    // Where Run opens: in the lower left, above the Start button.
    let opened = kit::open(FindHost { form, held: 0 }, |_, h| (monitor_left + scale.len(12), bar_top - h - scale.len(12)));
    if let Some(hwnd) = opened {
        // Not above every window, as the bar's other windows are: another program brought
        // forward covers it, and the Windows key brings it back.
        // SAFETY: our own window; only its place among the windows changes.
        if unsafe { SetWindowPos(hwnd, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) } == 0 {
            log("Quick Search: SetWindowPos(not topmost) failed");
        }
        log(&format!("opened Quick Search: {count} programs (ready in {} ms)", started.elapsed().as_millis()));
    }
    crate::apps::refresh();
    refresh();
}

/// Closes the window.
pub fn close() {
    kit::close::<FindHost>();
}

/// Closes the window if it stopped being the one in front (a click outside it, Alt+Tab, another
/// window coming forward: the owner, "I think quick search should go away on any click outside
/// the window ... otherwise it's stuck a lot when I change my mind", brief 90). Its own menus and
/// a drag of its column line keep it in front, and a program it starts closes it anyway. On the
/// bar's tick.
pub fn close_if_turned_away() {
    if kit::with::<FindHost, _>(|h| h.form.turned_away()).unwrap_or(false) {
        log("Quick Search closed: another window came to the front");
        close();
    }
}

/// A press on the bar or the classic desktop (which never take the front from it themselves):
/// an open Quick Search closes, and the press goes on to do what it does there.
pub fn close_for_a_press() {
    if kit::hwnd::<FindHost>().is_some() {
        log("Quick Search closed: a press on the bar or the desktop");
        close();
    }
}

/// The bar's look or size changed: an open window closes (its pictures belong to the old look).
pub fn look_changed() {
    if kit::hwnd::<FindHost>().is_some() {
        close();
    }
}

/// Carries out a command found (`bevel_core::quick`): posted to the bar as the menu entry that
/// does the same would post it, so it happens once this window is gone.
fn run_command(action: Action) {
    let bar = BAR_HWND.load(Ordering::SeqCst) as HWND;
    log(&format!("Quick Search: {action:?}"));
    let (msg, wparam, lparam) = match action {
        Action::Bar(command) => (WM_BAR_COMMAND.id(), command.code(), 0),
        Action::Look(era) => (WM_BAR_THEME.id(), Era::ALL.iter().position(|&e| e == era).unwrap_or(0), 0),
        Action::Size(scale) => {
            let (wparam, lparam) = crate::bar::size_message(scale);
            (WM_BAR_SIZE.id(), wparam, lparam)
        }
        Action::Colors(depth) => (WM_BAR_COLORS.id(), Depth::ALL.iter().position(|&d| d == depth).unwrap_or(Depth::ALL.len() - 1), 0),
        Action::MonitorLines(strength) => (crate::bar::WM_BAR_MONITOR_LINES.id(), strength.index(), 0),
    };
    // SAFETY: posting to our own bar window.
    unsafe { PostMessageW(bar, msg, wparam, lparam) };
}

/// The file that keeps Quick Search's Name column width between runs (`find::state_text`).
fn state_path() -> std::path::PathBuf {
    data_dir().join("quick-search.txt")
}

/// The line between the columns was moved or fitted: the width is remembered. A save that
/// fails is logged once in a run, not at every drag.
fn save_name_width() {
    static SAID: AtomicBool = AtomicBool::new(false);
    let Some(width) = kit::with::<FindHost, _>(|h| h.form.name_width()) else { return };
    if let Err(e) = state_file::save(&state_path(), &find::state_text(width))
        && !state_file::is_read_only(&e)
        && !SAID.swap(true, Ordering::SeqCst)
    {
        log(&format!("could not save Quick Search's column width: {e} (said once)"));
    }
}

/// Carries out Enter, a double-click, the title bar's "X" and a new column width.
fn activate(part: Part) {
    match part {
        Part::TitleClose => close(),
        Part::Divider => save_name_width(),
        Part::Open => {
            let Some((target, command, name)) =
                kit::with::<FindHost, _>(|h| h.form.selected().map(|p| (p.target.clone(), p.command, p.name.clone()))).flatten()
            else {
                return;
            };
            // A program started: counted for XP's Start menu (brief 112).
            if command.is_none() {
                crate::often::count(&name, &target);
            }
            // A thing to do: the window closes, then the bar does it as its menus would.
            if let Some(action) = command {
                close();
                run_command(action);
                return;
            }
            // As the Programs menu starts its entries: the window goes off the screen at once
            // but keeps the focus while the program starts (so the program may come to the
            // front), then closes without taking the focus back.
            if let Some(hwnd) = kit::hwnd::<FindHost>() {
                // SAFETY: our own window; the system owns the region from here on.
                unsafe { SetWindowRgn(hwnd, CreateRectRgn(0, 0, 0, 0), 1) };
            }
            let started = Instant::now();
            window_list::launch(&target);
            let took = started.elapsed().as_millis();
            if took > 250 {
                log(&format!("starting a program from Quick Search took {took} ms (its window was already gone)"));
            }
            close();
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Names in other alphabets, with the fourth byte from the end inside a letter, are told
    /// (not cut inside the letter, which would panic).
    #[test]
    fn a_shortcut_is_told_by_its_bytes() {
        for link in ["C:\\Меню\\Блокнот.lnk", "C:\\Μενού\\Σημειωματάριο.LNK", "C:\\菜单\\记事本.Lnk"] {
            assert!(is_shortcut(link), "{link}");
        }
        // The fourth byte from the end inside a letter: Cyrillic, Greek, two kinds of Chinese.
        let inside = ["C:\\Меню\\Блокнотabc", "C:\\Μενού\\Σημειωματάριοx", "C:\\菜单\\记事本", "C:\\菜单\\记事本ab"];
        for other in inside {
            assert!(!other.is_char_boundary(other.len() - 4), "the test's own case: {other}");
            assert!(!is_shortcut(other), "{other}");
            assert_eq!(target_kind(other, false), find::Target::Unknown, "{other}");
        }
        for other in [".lnk", "lnk", ""] {
            assert!(!is_shortcut(other), "{other}");
        }
        assert_eq!(target_kind("C:\\Меню\\Блокнот.lnk", false), find::Target::Unknown);
    }

    /// This PC's Programs menu as Quick Search sees it, in counts only (never a name): every
    /// entry, how many each rule hides, and how many still listed look like documentation by
    /// their name. Reads the Start Menu folders and their shortcuts, writes nothing.
    /// `cargo test -p bevel95 quick_search_counts -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn quick_search_counts() {
        crate::audio::init();
        crate::apps::read_now();
        let programs = read_programs(true);
        let others: Vec<&Program> = programs.iter().filter(|p| !p.own).collect();
        let by = |rule: find::Hidden| others.iter().filter(|p| find::hidden(&p.name, p.kind) == Some(rule)).count();
        let shown: Vec<&&Program> = others.iter().filter(|p| find::hidden(&p.name, p.kind).is_none()).collect();
        let words = ["help", "manual", "documentation", "readme", "website", "support", "license", "release notes"];
        let doc_like = shown.iter().filter(|p| words.iter().any(|w| p.name.to_lowercase().contains(w))).count();
        let kinds = [find::Target::Unknown, find::Target::Program, find::Target::Web, find::Target::Document, find::Target::Folder]
            .map(|k| (k, others.iter().filter(|p| p.kind == k).count()));
        let by_name = others.iter().filter(|p| find::hidden(&p.name, p.kind) == Some(find::Hidden::ByName)).count();
        let as_kind = |k: find::Target| others.iter().filter(|p| p.kind == k).count();
        println!(
            "in all {}, hidden as web {}, as document {}, as folder {}, by name {}, still listed {}",
            programs.len(),
            as_kind(find::Target::Web),
            as_kind(find::Target::Document),
            as_kind(find::Target::Folder),
            by_name,
            programs.len() - others.len() + shown.len()
        );
        println!(
            "entries {} (Bevel95's own {}), hidden by target {}, hidden by name {}, still listed {}, of which named like documentation {}; kinds {kinds:?}",
            programs.len(),
            programs.len() - others.len(),
            by(find::Hidden::ByTarget),
            by(find::Hidden::ByName),
            shown.len(),
            doc_like
        );
        // The documents' extensions (an extension is no name), to see that no program hides.
        let mut exts = std::collections::BTreeMap::new();
        for p in others.iter().filter(|p| p.kind == find::Target::Document) {
            let path = target_of(&p.target);
            let ext = path.rsplit_once('.').map(|(_, e)| e.to_lowercase()).filter(|e| e.len() <= 8 && !e.contains('\\')).unwrap_or_default();
            *exts.entry(ext).or_insert(0) += 1;
        }
        println!("documents by extension: {exts:?}");
    }

    /// What `link_target` says of the shortcuts named in `BEVEL95_TEST_LINKS` (paths separated
    /// by `|`), and how long each took: made to show that "a folder" comes from what the
    /// shortcut recorded (a shortcut to a folder since removed still says folder) and that a
    /// target on a network place that does not answer is not waited for.
    /// `cargo test -p bevel95 recorded_folder -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn recorded_folder() {
        crate::audio::init();
        let links = std::env::var("BEVEL95_TEST_LINKS").unwrap_or_default();
        for (n, link) in links.split('|').filter(|l| !l.is_empty()).enumerate() {
            let started = Instant::now();
            let found = match link_target(link) {
                Some(Link::Path { folder, .. }) => format!("path, folder {folder}"),
                Some(Link::Advertised) => "advertised".into(),
                None => "none".into(),
            };
            println!("link {n}: {found}, {} ms", started.elapsed().as_millis());
        }
    }

    fn target_of(entry: &str) -> String {
        match link_target(entry) {
            Some(Link::Path { path, .. }) => path,
            _ => entry.to_string(),
        }
    }
}
