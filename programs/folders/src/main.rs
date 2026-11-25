//! Windows 95's folder windows: My Computer and every ordinary folder, one window per folder, as
//! Explorer's folder windows were in the Windows 95 look. The window itself is
//! `bevel_core::folder` (drawn by Bevel95); Windows does the work underneath: the folders are read
//! from the disk, files open with their own programs, and every file command goes through Windows'
//! own file machinery (`bevel_winkit::file_ops`, shared with the classic desktop).
//!
//! One process holds every folder window. Started again (the desktop opens a folder by starting
//! `folders.exe <folder>`), it hands the folder to the one running and ends: that one opens a new
//! window, or brings the folder's window forward if it is open. It ends when its last window
//! closes. A folder window that fails can never take the bar down: they are separate programs.
//!
//! It keeps `folders.txt` beside Bevel95's settings (each folder's window place, `core::folder::saved`),
//! writes its window resources into the log every ten minutes as the bar does, and gives back every
//! window, icon and timer a folder window made when that window closes.
#![windows_subsystem = "windows"]

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bevel_core::desktop::{DesktopCommand as D, ItemKind};
use bevel_core::desktop_art::Art;
use bevel_core::desktop_icons::{Item, Picture, RenameEnd};
use bevel_core::folder::art::{self, DriveKind, Small};
use bevel_core::folder::menus::{self, Command};
use bevel_core::folder::saved::{Held, Kept, Relaid, Saved, ViewState, Window, full_id, kept, short_id};
use bevel_core::folder::menus::Nav;
use bevel_core::disk_box::{DiskBox, Part as DiskPart};
use bevel_core::folder::pane::{self, NodeKind, NodeSpec};
use bevel_core::folder::options::{Choice, FolderOptions, Part as OptionsPart};
use bevel_core::folder::web::{Guard, Kind as WebKind, Link, WebFonts};
use bevel_core::folder::{FolderWindow, History, ListRow, Part, Place, Space, Step, Tool, address, status, tabs, tree_art};
use bevel_core::kit::Form;
use bevel_core::paint::{MenuEntry, MenuStyle};
use bevel_core::text::escape_access_keys;
use bevel_core::theme::Era;
use bevel_winkit::confirm::{self, Place as ConfirmPlace};
use bevel_winkit::dragdrop::Dest;
use bevel_winkit::file_ops::{self, Target};
use bevel_winkit::kit::{self, Hosted, Kind, Look};
use bevel_winkit::log::{counted, data_dir, log, log_win32};
use bevel_winkit::menu::{self, Outcome, Request};
use bevel_winkit::shell::{self, Seen};
use bevel_winkit::util::{Private, point, rect_h, rect_w, wide};
use bevel_winkit::{font, look, shortcut_wizard};
use windows_sys::Win32::Foundation::{GetLastError, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromRect, MonitorFromWindow,
};
use windows_sys::Win32::Storage::FileSystem::{
    GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW,
};
use windows_sys::Win32::System::DataExchange::{
    AddClipboardFormatListener, COPYDATASTRUCT, GetClipboardSequenceNumber, RemoveClipboardFormatListener,
};
use windows_sys::Win32::System::Diagnostics::Debug::{SEM_FAILCRITICALERRORS, SetErrorMode};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, FindWindowExW, SetWindowTextW, GetWindowPlacement, GetWindowRect, HWND_MESSAGE,
    IsIconic, IsZoomed, KillTimer, PostMessageW, RegisterClassExW, SW_MAXIMIZE, SW_MINIMIZE,
    SW_RESTORE, SW_SHOWNORMAL, SendMessageTimeoutW, SetForegroundWindow, SetTimer, ShowWindow,
    SMTO_ABORTIFHUNG, WINDOWPLACEMENT, WM_CLIPBOARDUPDATE, WM_COPYDATA, WM_TIMER, WNDCLASSEXW,
};

/// The program's hidden window: the next start hands it the folder to open (WM_COPYDATA), the
/// folder readers tell it when something changed, and its timer watches the window resources.
const HUB_CLASS: &str = "Bevel95Folders";
/// The id of the folder handed over in WM_COPYDATA.
const OPEN_FOLDER: usize = 0xB95F;
/// Sent by a live test in WM_COPYDATA: a drop "from another program" into a folder window
/// (`dragdrop::test_drop`): the text is the offered DROPEFFECT bits, the window's id and the
/// files, separated by tabs. Only into a test folder's window, only test items; answered with
/// what the source would be told. The answer comes when the drop handler returns, as a real
/// source's DoDragDrop does.
const TEST_DROP: usize = 0xB95D;
/// A folder reader has new items for window `wparam` (its id).
static WM_ITEMS: Private = Private::new("Bevel95.Folders.Items");
/// The extension warning was answered (`wparam` 1: Yes).
static WM_RENAME_ANSWER: Private = Private::new("Bevel95.Folders.RenameAnswer");
/// Sent by a live test to the hidden window: which icon is at (x, y) = lParam (x + y * 65536, window
/// pixels) of folder window wParam? Answered with a number made from its id (as the desktop's
/// `WM_ICON_AT`, FNV-1a) or 0. It only answers.
static WM_ICON_AT: Private = Private::new("Bevel95.Folders.IconAt");
/// Sent by a live test: where is the icon whose number is lParam in folder window wParam? Its
/// picture's middle in window pixels (x + y * 65536), or 0 if there is no such icon.
static WM_ICON_PLACE: Private = Private::new("Bevel95.Folders.IconPlace");
/// Sent (posted) by a live test to the hidden window: in folder window wParam, select the icon
/// whose number (`id_code`) is lParam's high 32 bits (0: none), then carry out lParam's low 16 bits:
/// a `DesktopCommand::code` (Copy, Paste, Delete, Rename...), `TEST_UP` (Backspace), or open
/// `TEST_MENU + title` (the menu bar's), `TEST_ITEM_MENU`, `TEST_BACK_MENU`. Only on test items
/// (named "Bevel95 test ...") and, for commands on the folder itself, in a test folder's window
/// (its name "Bevel95 test ..."); anything else is refused and logged.
static WM_TEST_COMMAND: Private = Private::new("Bevel95.Folders.TestCommand");
const TEST_UP: usize = 100;
/// 98 and 2000: into the first folder named "Bevel95 test ..." of a test folder's window, Back,
/// Forward (same window), the address bar's list, Back's list of places.
const TEST_INTO: usize = 101;
const TEST_BACK: usize = 102;
const TEST_FORWARD: usize = 103;
const TEST_LIST: usize = 212;
const TEST_PLACES: usize = 213;
const TEST_MENU: usize = 200;
const TEST_ITEM_MENU: usize = 210;
const TEST_BACK_MENU: usize = 211;
/// Explorer's tree: open, close, move down and up, the menu on the selected row, show or hide
/// the tree, and open an Explorer window on the first test folder (only in a test folder's
/// window).
const TEST_TREE_TOGGLE: usize = 220;
const TEST_TREE_DOWN: usize = 221;
const TEST_TREE_UP: usize = 222;
const TEST_TREE_MENU: usize = 223;
const TEST_TREE_SHOW: usize = 224;
const TEST_EXPLORE: usize = 225;
/// The views: 230 + Large, Small, List, Details; a click on Details' header 240 + column (only in
/// a test folder's window).
const TEST_VIEW: usize = 230;
const TEST_HEADER: usize = 240;
/// The web view: only select the item, 98's View > as Web Page, Show Files on the warning page,
/// Select All (only in a test folder's window).
const TEST_SELECT: usize = 250;
const TEST_WEB_TOGGLE: usize = 251;
const TEST_SHOW_FILES: usize = 252;
const TEST_SELECT_ALL: usize = 253;
/// Opens Windows 2000's Folder Options over a test folder's window.
const TEST_FOLDER_OPTIONS: usize = 254;
/// From now on in this program, the reading of a test folder's place ("Bevel95 test ...") comes
/// `SLOW_READING_MS` late, past the patience (a slow drive, for the live try of a late reading;
/// only from a test folder's window).
const TEST_SLOW_READING: usize = 255;
/// Brief 103: the selected test item moved within its window by `TEST_MOVE_BY` (the survey's drag
/// of C: on 98), as a drag dropped in the same window moves it (`move_ids`, then kept as the look
/// keeps it). Only on a test item in a test folder's window.
const TEST_MOVE: usize = 256;
/// Brief 110: the selected test folder opened in a new tab; the next tab to the front; the tab in
/// front closed. Only in a test folder's window (the new tab: on a test folder).
const TEST_NEW_TAB: usize = 257;
const TEST_TAB_NEXT: usize = 258;
const TEST_TAB_CLOSE: usize = 259;
const TEST_MOVE_BY: (i32, i32) = (171, 173);
const SLOW_READING_MS: u64 = 1500;
static SLOW_READING: AtomicBool = AtomicBool::new(false);
/// A tree reader has rows for a window's tree.
static WM_TREE_ROWS: Private = Private::new("Bevel95.Folders.TreeRows");
/// A drive a window went to could not be read (wparam: the window's id; lparam: the letter, with
/// `LOCKED_FLAG` when it is locked by BitLocker).
static WM_UNREADABLE: Private = Private::new("Bevel95.Folders.Unreadable");
const LOCKED_FLAG: isize = 0x1_0000;
/// A reader has files' own pictures for window `wparam` (`PICTURES`).
static WM_PICTURES: Private = Private::new("Bevel95.Folders.Pictures");
/// The resource watch's timer.
const WATCH_TIMER: usize = 1;
/// The tree's timer (100 ms): asks for rows the tree needs, lets the icons follow a selection
/// moved by keys once the keys have rested, reads a changed folder's rows again.
const TREE_TIMER: usize = 2;
/// How long the tree's selection rests before the icons follow it (keys only; Windows' Explorer
/// waited too, `win95-down0`: the icons still showed C: right after Down).
const SETTLE: Duration = Duration::from_millis(500);
/// How often a folder is looked at again.
const READ_EVERY: Duration = Duration::from_millis(1000);
/// My Computer's id (as the desktop's).
const MY_COMPUTER: &str = "::my-computer";

/// What a window shows.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Shows {
    /// The desktop as a folder (98 and 2000: the address bar's list and Up from My Computer).
    Desktop,
    MyComputer,
    Folder(PathBuf),
}

/// The Desktop's id (as the place a window shows).
const DESKTOP: &str = "::desktop";

impl Shows {
    /// The key it is remembered and found by: the path in lower case, or My Computer's id.
    fn key(&self) -> String {
        match self {
            Shows::Desktop => DESKTOP.to_string(),
            Shows::MyComputer => MY_COMPUTER.to_string(),
            Shows::Folder(p) => p.to_string_lossy().to_lowercase(),
        }
    }

    /// The core's place for it (history, address bar).
    fn place(&self) -> Place {
        match self {
            Shows::Desktop => Place::Desktop,
            Shows::MyComputer => Place::MyComputer,
            Shows::Folder(p) => Place::Folder(p.to_string_lossy().into_owned()),
        }
    }

    fn of_place(place: &Place) -> Shows {
        match place {
            Place::Desktop => Shows::Desktop,
            Place::MyComputer => Shows::MyComputer,
            Place::Folder(p) => Shows::Folder(PathBuf::from(p)),
        }
    }
}

/// The hidden items among those listed (by id), whose pictures are drawn faded
/// (`bevel_core::hidden::faded`).
fn hidden_of(attributes: &HashMap<String, u32>) -> Vec<String> {
    attributes.iter().filter(|(_, a)| bevel_core::hidden::faded(**a)).map(|(id, _)| id.clone()).collect()
}

/// What a folder reader found.
struct Snapshot {
    items: Vec<Item>,
    hidden: usize,
    total_size: u64,
    spaces: HashMap<String, Space>,
    /// The free space of the drive the folder is on (Explorer's status bar).
    free: Option<u64>,
    /// Each item's file attributes, by id (2000's web view says them).
    attributes: HashMap<String, u32>,
}

/// What readers found, by window id and the reader's generation (`Generations`).
static SNAPSHOTS: Mutex<Vec<(usize, u64, Snapshot)>> = Mutex::new(Vec::new());
/// Files' own pictures read after their names showed, by window id and reader generation: (item
/// id, picture).
static PICTURES: Mutex<Vec<(usize, u64, PictureUpdates)>> = Mutex::new(Vec::new());

/// Each window's readers by generation: a window whose reader is started again (the look changed,
/// Refresh) keeps its id, so what an earlier reader still delivers is told apart by its
/// generation and dropped, at the reader (before it posts) and at the window (when it takes it).
#[derive(Default)]
struct Generations {
    next: u64,
    current: HashMap<usize, u64>,
}

impl Generations {
    /// A new reader of window `id`: its generation, from now on the window's current one.
    fn start(&mut self, id: usize) -> u64 {
        self.next += 1;
        self.current.insert(id, self.next);
        self.next
    }

    fn is_current(&self, id: usize, generation: u64) -> bool {
        self.current.get(&id) == Some(&generation)
    }

    fn forget(&mut self, id: usize) {
        self.current.remove(&id);
    }
}

static GENERATIONS: Mutex<Option<Generations>> = Mutex::new(None);

/// Whether reader `generation` is window `id`'s current one.
fn is_current(id: usize, generation: u64) -> bool {
    GENERATIONS.lock().ok().is_some_and(|g| g.as_ref().is_some_and(|g| g.is_current(id, generation)))
}

/// Takes window `id`'s entries out of `all`, giving back the current reader's (the last) and
/// dropping any earlier reader's.
fn take_current<T>(all: &mut Vec<(usize, u64, T)>, id: usize, current: impl Fn(u64) -> bool) -> Vec<T> {
    let mut out = Vec::new();
    let mut k = 0;
    while k < all.len() {
        if all[k].0 == id {
            let (_, g, t) = all.remove(k);
            if current(g) {
                out.push(t);
            }
        } else {
            k += 1;
        }
    }
    out
}

/// Who started a reader: the person (opening, navigating, Refresh) or the look's change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StartedBy {
    Person,
    LookChange,
}

/// Whether a reader tells that its drive cannot be read: once, and only when the person's own
/// doing started it (finding 1 of the review of 2026-11-23: a look change asked again).
struct Unreadable {
    told: bool,
}

impl Unreadable {
    fn new(by: StartedBy) -> Unreadable {
        Unreadable { told: by == StartedBy::LookChange }
    }

    /// True the first time only (and never for a reader the look's change started).
    fn tell(&mut self) -> bool {
        !std::mem::replace(&mut self.told, true)
    }
}
type PictureUpdates = Vec<(String, Picture)>;
/// The items each window shows on its screen now (their pictures are read first).
static WANTED: Mutex<Option<HashMap<usize, Vec<String>>>> = Mutex::new(None);

/// One folder window.
struct FolderHost {
    form: FolderWindow,
    id: usize,
    shows: Shows,
    /// Its reader stops when this is set.
    stop: Arc<AtomicBool>,
    /// A new item (New, Create Shortcut, Paste) is selected once it shows: its id.
    select: Option<String>,
    /// 98 and 2000: the places it has shown (Back, Forward), and the address bar's list's entries
    /// while it is open.
    history: History,
    /// How the window came to the place it shows (`None`: it was opened there), for a drive found
    /// locked once the window was there (`locked_found`: that step is undone).
    came: Option<Step>,
    entries: Vec<address::Entry>,
    /// Explorer: when the icons follow the tree's selection (after keys); rows to read again
    /// at a moment (after a file command on a tree row).
    settle: Option<std::time::Instant>,
    reread: Vec<(std::time::Instant, String)>,
    /// When the place shown was asked for (the log says how long its names took).
    asked: Option<std::time::Instant>,
    /// What is known of the place shown (`read_facts`, off the window's thread).
    facts: Facts,
    /// The places of the icons the person moved in each folder this window has shown, while it
    /// stays open (Windows 98 with its web view keeps My Computer's so: brief 103), by folder key,
    /// with the view they were made in (brief 105, 1).
    session: HashMap<String, bevel_core::folder::saved::MovedRecord>,
    /// Tabs (brief 110): the tab in front (`tab`, an id that never changes), the row's order by
    /// id, and the tabs behind with all they keep (`TabSlot`). One tab: `back` is empty.
    tab: u64,
    order: Vec<u64>,
    back: Vec<TabSlot>,
}

/// A tab behind the one in front: its own view and everything the window keeps per place. It has
/// no reader: the reader stops when a tab goes behind, and a new one (a new window id) reads the
/// place again when it comes to the front, so a late reading can only reach the tab that asked.
struct TabSlot {
    tab: u64,
    form: FolderWindow,
    shows: Shows,
    select: Option<String>,
    history: History,
    came: Option<Step>,
    entries: Vec<address::Entry>,
    settle: Option<std::time::Instant>,
    reread: Vec<(std::time::Instant, String)>,
    asked: Option<std::time::Instant>,
    facts: Facts,
    session: HashMap<String, bevel_core::folder::saved::MovedRecord>,
}

thread_local! {
    /// Tabs in folder windows (brief 110): where the row goes and where its X goes (settings.txt,
    /// read at the start and at every change of the look).
    static TAB_SETTINGS: Cell<(tabs::Place, tabs::CloseAt)> = const { Cell::new((tabs::Place::Off, tabs::CloseAt::End)) };
    static NEXT_TAB: Cell<u64> = const { Cell::new(1) };
    /// A new tab is being given its place: the window goes there itself, also in 95.
    static IN_NEW_TAB: Cell<bool> = const { Cell::new(false) };
}

thread_local! {
    static HUB: Cell<HWND> = const { Cell::new(null_mut()) };
    static NEXT_ID: Cell<usize> = const { Cell::new(1) };
    /// Each window's id, for the readers' messages.
    static WINDOWS: RefCell<Vec<(usize, HWND, String)>> = const { RefCell::new(Vec::new()) };
    static LAST_CASCADE: Cell<Option<i32>> = const { Cell::new(None) };
    static SAVED: RefCell<Saved> = RefCell::new(Saved::default());
    /// The window whose rename box waits for the extension warning's answer.
    static ASKING: Cell<HWND> = const { Cell::new(null_mut()) };
    /// The ids cut onto the clipboard, and the clipboard's number right after.
    static CUT: RefCell<(Vec<String>, u32)> = const { RefCell::new((Vec::new(), 0)) };
    static ERA_LOOK: RefCell<Option<(Era, Look)>> = const { RefCell::new(None) };
    /// The web view's fonts, made once for the look in use.
    static WEB_FONTS: RefCell<Option<(Era, Arc<WebFonts>)>> = const { RefCell::new(None) };
}

fn saved_path() -> PathBuf {
    data_dir().join("folders.txt")
}

/// folders.txt as it would be written now (`save_if_changed`).
fn saved_text() -> String {
    SAVED.with(|s| s.borrow().text())
}

/// Saves folders.txt only if it differs from `before` (`saved_text` before a change): nothing
/// is written without a change (brief 106, 7).
fn save_if_changed(before: &str) {
    if saved_text() != before {
        save();
    }
}

fn save() {
    // Only the program that holds the hub's place saves (a second one's picture would overwrite
    // the first's: the third safety review of 2026-11-18, C5).
    if !HUB_HOLDER.load(Ordering::SeqCst) {
        static SAID: AtomicBool = AtomicBool::new(false);
        if !SAID.swap(true, Ordering::SeqCst) {
            log("folders: a second folder windows' program does not save folders.txt (the first one does)");
        }
        return;
    }
    let text = SAVED.with(|s| s.borrow().text());
    if let Err(e) = bevel_winkit::state_file::save(&saved_path(), &text)
        && !bevel_winkit::state_file::is_read_only(&e)
    {
        log(&format!("folders: could not save folders.txt: {e}"));
    }
}

fn the_look() -> Option<(Era, Look)> {
    ERA_LOOK.with(|l| l.borrow().clone())
}

/// The work area (screen pixels) of the screen `hwnd` is on, or of the main screen.
fn work_area(hwnd: Option<HWND>) -> RECT {
    match hwnd {
        Some(h) => {
            let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
            // SAFETY: a valid out-pointer; any window handle.
            unsafe { GetMonitorInfoW(MonitorFromWindow(h, MONITOR_DEFAULTTONEAREST), &mut info) };
            info.rcWork
        }
        None => look::main_screen().0,
    }
}

/// Where a window `w` by `h` that wants to be at `at` goes: there, moved onto the nearest screen if
/// part of it would be off it.
fn on_screen(at: (i32, i32), w: i32, h: i32) -> (i32, i32) {
    let wanted = RECT { left: at.0, top: at.1, right: at.0 + w, bottom: at.1 + h };
    let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
    // SAFETY: a valid rectangle and out-pointer.
    unsafe { GetMonitorInfoW(MonitorFromRect(&wanted, MONITOR_DEFAULTTONEAREST), &mut info) };
    let work = info.rcWork;
    (at.0.min(work.right - w).max(work.left), at.1.min(work.bottom - h).max(work.top))
}

// ---------------------------------------------------------------------------------------------
// Reading folders and drives.

/// A drive's name as the look's My Computer showed it: a floppy's "3½ Floppy (A:)", else its
/// label, if it has one, and its letter; without a label 95 wrote "(C:)", 98 " (C:)" (with a space
/// in front, `win98-truecolor-folder-address-list`), 2000 the kind ("Local Disk (C:)", "Compact
/// Disc (D:)").
fn drive_name(era: Era, letter: char, kind: DriveKind, label: &str) -> String {
    match kind {
        DriveKind::Floppy => format!("3\u{bd} Floppy ({letter}:)"),
        _ if !label.is_empty() => format!("{label} ({letter}:)"),
        _ => match era {
            Era::Win95 => format!("({letter}:)"),
            Era::Win98 => format!(" ({letter}:)"),
            Era::Win2000 => {
                let kind_name = match kind {
                    DriveKind::CdRom => "Compact Disc",
                    DriveKind::Removable | DriveKind::Floppy | DriveKind::Floppy525 => "Removable Disk",
                    DriveKind::Network => "Network Drive",
                    DriveKind::RamDisk => "RAM Disk",
                    DriveKind::Fixed => "Local Disk",
                };
                format!("{kind_name} ({letter}:)")
            }
        },
    }
}

/// The look's own items of the desktop (in its desktop's order), then the owner's Desktop folder
/// and the public one: what a window showing the Desktop holds.
fn read_desktop(era: Era) -> Option<(Vec<Seen>, usize, u64)> {
    let mut seen = Vec::new();
    let (mut hidden, mut total) = (0, 0);
    for dir in [shell::desktop_folder(), shell::public_desktop_folder()].into_iter().flatten() {
        if let Some((s, h)) = shell::list_folder(&dir) {
            seen.extend(s);
            hidden += h.count;
            total += h.total_size;
        }
    }
    let _ = era;
    Some((seen, hidden, total))
}

/// This PC's drives as My Computer shows them, with their free space, then (in each look's order)
/// Control Panel and Printers (Windows 2000 had no Printers there). `ask_lock`: a drive that cannot
/// be read is asked whether it is locked by BitLocker (on a reading thread only: false on a
/// window's thread, which then uses what the reading threads found, `known_locked`).
fn read_my_computer(era: Era, ask_lock: bool) -> Snapshot {
    let mut items = Vec::new();
    let mut spaces = HashMap::new();
    // SAFETY: plain queries; a drive without a disk answers with an error, not a message box.
    let drives = unsafe { GetLogicalDrives() };
    for n in 0..26 {
        if drives & (1 << n) == 0 {
            continue;
        }
        let letter = (b'A' + n as u8) as char;
        let root = format!("{letter}:\\");
        let w = wide(&root);
        // SAFETY: a null-terminated root path.
        let drive_type = unsafe { GetDriveTypeW(w.as_ptr()) };
        let kind = match drive_type {
            2 if letter <= 'B' => DriveKind::Floppy,
            2 => DriveKind::Removable,
            3 => DriveKind::Fixed,
            4 => DriveKind::Network,
            5 => DriveKind::CdRom,
            6 => DriveKind::RamDisk,
            _ => continue,
        };
        let mut label = [0u16; 261];
        let mut label_text = String::new();
        if kind != DriveKind::Floppy {
            // SAFETY: a null-terminated root and a buffer of the length given.
            let ok = unsafe {
                GetVolumeInformationW(w.as_ptr(), label.as_mut_ptr(), label.len() as u32, null_mut(), null_mut(), null_mut(), null_mut(), 0)
            };
            if ok != 0 {
                let len = label.iter().position(|&c| c == 0).unwrap_or(0);
                label_text = String::from_utf16_lossy(&label[..len]);
                note_lock(letter, false);
            } else if ask_lock {
                // SAFETY: a plain query, right after the call that failed.
                lock_after_failed_read(letter, drive_type, unsafe { GetLastError() });
            }
            let (mut free, mut total) = (0u64, 0u64);
            // SAFETY: a null-terminated root and valid out-pointers.
            if unsafe { GetDiskFreeSpaceExW(w.as_ptr(), &mut free, &mut total, null_mut()) } != 0 {
                spaces.insert(root.clone(), Space { free, capacity: total });
            }
        }
        items.push(Item {
            id: root,
            name: drive_name(era, letter, kind, &label_text),
            picture: Picture::Art(Art::Drive(kind)),
            shortcut: false,
            folder: false,
            type_name: String::new(),
            size: 0,
            modified: 0,
        });
    }
    let control_panel = (file_ops::CONTROL_PANEL, "Control Panel", Art::ControlPanel);
    let printers = (file_ops::PRINTERS, "Printers", Art::Printers);
    let places = match era {
        Era::Win95 => vec![control_panel, printers],
        Era::Win98 => vec![printers, control_panel],
        Era::Win2000 => vec![control_panel],
    };
    for (id, name, art) in places {
        items.push(Item {
            id: id.to_string(),
            name: name.to_string(),
            picture: Picture::Art(art),
            shortcut: false,
            folder: false,
            type_name: String::new(),
            size: 0,
            modified: 0,
        });
    }
    Snapshot { items, hidden: 0, total_size: 0, spaces, free: None, attributes: HashMap::new() }
}

/// Reads a folder (or My Computer, or the Desktop) again and again on a thread of its own until
/// `stop`, handing what changed to the program's window (`WM_ITEMS`). Pictures are read only for
/// new or changed items.
///
/// `by`: whether the person's own doing started it (opening, navigating, Refresh) or the look's
/// change: only the person's doing tells that a drive cannot be read (the look's message, or
/// Windows' unlock window for a locked drive). Whatever an earlier reader of the same window still
/// delivers after this one started is dropped (`Generations`).
fn start_reader(id: usize, shows: Shows, era: Era, stop: Arc<AtomicBool>, hub: HWND, by: StartedBy) {
    let hub = hub as usize;
    let generation = GENERATIONS.lock().map(|mut g| g.get_or_insert_with(Generations::default).start(id)).unwrap_or(0);
    let spawned = std::thread::Builder::new().name("folder reader".into()).spawn(move || {
        bevel_winkit::com::init();
        let mut last: Option<(Vec<Seen>, usize, u64)> = None;
        let mut last_drives: Option<Vec<(String, String)>> = None;
        let mut known: HashMap<(PathBuf, u64), Item> = HashMap::new();
        let mut unreadable = Unreadable::new(by);
        // Each kind of file asked about once; the files whose own picture is still to be read.
        let mut kinds: HashMap<(String, bool), shell::Kind> = HashMap::new();
        let mut pending: Vec<(PathBuf, u64)> = Vec::new();
        let mut checked: std::collections::HashSet<(PathBuf, u64)> = std::collections::HashSet::new();
        let started = std::time::Instant::now();
        let mut first_screen_told = false;
        while !stop.load(Ordering::SeqCst) {
            let listed = match &shows {
                Shows::MyComputer => None,
                Shows::Desktop => read_desktop(era),
                Shows::Folder(dir) => shell::list_folder(dir).map(|(s, h)| (s, h.count, h.total_size)),
            };
            // A drive's root that was read is not locked (any more) (B2 of the review before 0.1.3).
            if listed.is_some()
                && let Some(l) = drive_letter_of(&shows)
            {
                note_lock(l, false);
            }
            let snapshot = match &shows {
                Shows::MyComputer => {
                    let s = read_my_computer(era, true);
                    let names: Vec<(String, String)> = s.items.iter().map(|i| (i.id.clone(), i.name.clone())).collect();
                    if last_drives.as_ref() == Some(&names) {
                        None
                    } else {
                        last_drives = Some(names);
                        Some(s)
                    }
                }
                _ => match listed {
                    None => {
                        // A drive without a disk: the window says so once (`disk_box`).
                        if let Shows::Folder(dir) = &shows
                            && dir.parent().is_none()
                            && last.is_none()
                            && unreadable.tell()
                        {
                            let letter = dir.to_string_lossy().chars().next().unwrap_or('A');
                            // Locked by BitLocker (asked here, on the reader's thread): Windows'
                            // unlock window instead of the look's message (`LOCKED_FLAG`).
                            let locked = root_letter(&dir.to_string_lossy()).is_some_and(|l| {
                                let locked = bevel_winkit::bitlocker::drive_locked(l);
                                note_lock(l, locked);
                                locked
                            });
                            if locked {
                                tell_locked(id, letter);
                            }
                            let flag = if locked { LOCKED_FLAG } else { 0 };
                            // SAFETY: posting to the program's own window.
                            unsafe { PostMessageW(hub as HWND, WM_UNREADABLE.id(), id, letter as isize | flag) };
                        }
                        None
                    }
                    Some((mut seen, hidden, total)) => {
                        seen.sort_by(|a, b| a.path.cmp(&b.path));
                        let now = (seen.clone(), hidden, total);
                        if last.as_ref() == Some(&now) {
                            None
                        } else {
                            // The Desktop: the look's own icons first, in its desktop's order.
                            let mut items: Vec<Item> = if shows == Shows::Desktop {
                                bevel_core::desktop_icons::system_icons(era).iter().map(|&s| Item::system(s)).collect()
                            } else {
                                Vec::new()
                            };
                            // Names and each kind's picture at once; files' own pictures follow.
                            let mut fresh = HashMap::new();
                            pending.clear();
                            for s in &seen {
                                let key = (s.path.clone(), s.modified);
                                let item = known.remove(&key).unwrap_or_else(|| shell::quick_item(s, &mut kinds, era));
                                // A kind with the look's own picture is never read again.
                                if !s.folder && !checked.contains(&key) && shell::reads_own_picture(&s.path, era) {
                                    pending.push(key.clone());
                                }
                                items.push(item.clone());
                                fresh.insert(key, item);
                            }
                            checked.retain(|k| fresh.contains_key(k));
                            known = fresh;
                            if last.is_none() {
                                log(&format!("folders: {} items listed and named in {} ms", seen.len(), started.elapsed().as_millis()));
                            }
                            last = Some(now);
                            let free = match &shows {
                                Shows::Folder(dir) => free_space(dir),
                                _ => None,
                            };
                            let attributes = seen.iter().map(|s| (s.path.to_string_lossy().into_owned(), s.attributes)).collect();
                            Some(Snapshot { items, hidden, total_size: total, spaces: HashMap::new(), free, attributes })
                        }
                    }
                },
            };
            if let Some(s) = snapshot {
                if let Ok(mut all) = SNAPSHOTS.lock() {
                    // A newer reader of this window took over: this one's reading goes nowhere
                    // (and never replaces the newer one's).
                    if !is_current(id, generation) {
                        break;
                    }
                    all.retain(|(i, ..)| *i != id);
                    all.push((id, generation, s));
                }
                // SAFETY: posting to the program's own window (a stale handle just fails).
                unsafe { PostMessageW(hub as HWND, WM_ITEMS.id(), id, 0) };
            }
            // A big folder is looked at less often (3000 items read every second cost a tenth
            // of a processor): once a second up to 300 items, then a second more per 300.
            let items = last.as_ref().map_or(0, |(seen, ..)| seen.len());
            let every = READ_EVERY * (1 + items as u32 / 300).min(10);
            let mut waited = Duration::ZERO;
            let mut since = std::time::Instant::now();
            while waited < every && !stop.load(Ordering::SeqCst) {
                if pending.is_empty() {
                    std::thread::sleep(Duration::from_millis(50));
                    waited += Duration::from_millis(50);
                    continue;
                }
                // The files' own pictures, those on the screen first (`WANTED`), a few at a time.
                let wanted: std::collections::HashSet<String> =
                    WANTED.lock().ok().and_then(|w| w.as_ref().and_then(|m| m.get(&id).cloned())).unwrap_or_default().into_iter().collect();
                let mut batch: Vec<(PathBuf, u64)> = Vec::new();
                pending.retain(|k| {
                    if batch.len() < 24 && wanted.contains(k.0.to_string_lossy().as_ref()) {
                        batch.push(k.clone());
                        false
                    } else {
                        true
                    }
                });
                let on_screen = !batch.is_empty();
                while batch.len() < 24 && !pending.is_empty() && !on_screen {
                    batch.push(pending.remove(0));
                }
                let mut updates = Vec::new();
                for key in batch {
                    let path = key.0.to_string_lossy().into_owned();
                    let kind = shell::kind_index(&key.0, &mut kinds);
                    if let Some(own) = shell::own_picture(&path, kind) {
                        let picture = Picture::Shell(own);
                        if let Some(item) = known.get_mut(&key) {
                            item.picture = picture.clone();
                        }
                        updates.push((path, picture));
                    }
                    checked.insert(key);
                }
                if !updates.is_empty() {
                    if let Ok(mut all) = PICTURES.lock() {
                        if !is_current(id, generation) {
                            break;
                        }
                        all.push((id, generation, updates));
                    }
                    // SAFETY: posting to the program's own window.
                    unsafe { PostMessageW(hub as HWND, WM_PICTURES.id(), id, 0) };
                }
                if !first_screen_told && (!on_screen || pending.is_empty()) {
                    first_screen_told = true;
                    log(&format!("folders: the first screen's pictures read {} ms after the start", started.elapsed().as_millis()));
                }
                if pending.is_empty() {
                    log(&format!("folders: every picture read {} ms after the start ({} kept)", started.elapsed().as_millis(), shell::store_size()));
                }
                let now = std::time::Instant::now();
                waited += now - since;
                since = now;
            }
        }
    });
    if let Err(e) = spawned {
        log(&format!("folders: could not start reading a folder: {e}"));
    }
}

// ---------------------------------------------------------------------------------------------
// Drives locked by BitLocker (`bevel_core::bitlocker`, `bevel_winkit::bitlocker`).

/// What each drive's last reading found: locked or not, and when. Written by the threads that read
/// drives (My Computer's reader, a place's facts, the tree's reader, a folder reader), read by the
/// window's thread, which never asks a drive itself.
static LOCKED: Mutex<Option<HashMap<char, (bool, std::time::Instant)>>> = Mutex::new(None);

/// How long a reading that found a drive locked is believed, for Open: My Computer's reader reads
/// its drives every second, so a drive unlocked meanwhile is known within that time.
const LOCK_BELIEVED: Duration = Duration::from_secs(3);

fn note_lock(letter: char, locked: bool) {
    if let Ok(mut all) = LOCKED.lock() {
        all.get_or_insert_with(HashMap::new).insert(letter.to_ascii_uppercase(), (locked, std::time::Instant::now()));
    }
}

/// A drive's lock as its last reading found it (`fresh`: only if that reading is recent).
fn known_locked(letter: char, fresh: bool) -> bool {
    LOCKED.lock().ok().and_then(|all| all.as_ref()?.get(&letter.to_ascii_uppercase()).copied()).is_some_and(|(locked, at)| locked && (!fresh || at.elapsed() < LOCK_BELIEVED))
}

/// The windows (by id) whose reader found their drive locked and is telling the program so
/// (`WM_UNREADABLE` with `LOCKED_FLAG`): the message is acted on only if its window and letter are
/// here, and each is taken out as it is acted on, so that a message from another program, which
/// anyone can send, does nothing.
static LOCKED_TOLD: Mutex<Vec<(usize, char)>> = Mutex::new(Vec::new());

/// A reader of window `id` found its drive `letter` locked and is about to tell the program.
fn tell_locked(id: usize, letter: char) {
    if let Ok(mut told) = LOCKED_TOLD.lock() {
        // At most one per window: a window has one reader, which tells it once.
        told.retain(|&(w, _)| w != id);
        told.push((id, letter.to_ascii_uppercase()));
    }
}

/// Whether window `id`'s reader told that drive `letter` is locked (taken out: once only).
fn take_locked_told(id: usize, letter: char) -> bool {
    let letter = letter.to_ascii_uppercase();
    if !letter.is_ascii_uppercase() {
        return false;
    }
    let Ok(mut told) = LOCKED_TOLD.lock() else { return false };
    match told.iter().position(|&t| t == (id, letter)) {
        Some(i) => {
            told.swap_remove(i);
            true
        }
        None => false,
    }
}

/// The drive letter of a drive's root ("E:\"), if `id` is one.
fn root_letter(id: &str) -> Option<char> {
    let b = id.as_bytes();
    (b.len() == 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\').then(|| (b[0] as char).to_ascii_uppercase())
}

/// The drive a place is, if it is a drive's root.
fn drive_letter_of(shows: &Shows) -> Option<char> {
    match shows {
        Shows::Folder(p) => root_letter(&p.to_string_lossy()),
        _ => None,
    }
}

/// An item's kind for its menu and commands: a drive whose last reading found it locked is a
/// `LockedDrive` (its menu has Unlock Drive..., its Open starts Windows' unlock window).
fn kind_of(item: &Item) -> ItemKind {
    match ItemKind::of(item) {
        ItemKind::Drive if root_letter(&item.id).is_some_and(|l| known_locked(l, false)) => ItemKind::LockedDrive,
        kind => kind,
    }
}

/// The window's menus' state, its selection's kinds as the menu on the icons has them
/// (`kind_of`: a drive found locked has Unlock Drive... in 98's and 2000's File menu too).
fn menu_state(h: &FolderHost, can_paste: bool) -> menus::State {
    let mut state = h.form.menu_state(can_paste);
    state.kinds = h.form.view.selection().into_iter().map(|i| kind_of(&h.form.view.items[i])).collect();
    state
}

/// Reads drive `letter`'s lock after a call on it failed (`error`: `GetLastError` right after it;
/// `drive_type`: its `GetDriveTypeW`) and keeps what was found. On a reading thread only.
fn lock_after_failed_read(letter: char, drive_type: u32, error: u32) -> bool {
    let locked = bevel_winkit::bitlocker::locked_after_failed_read(letter, drive_type, error);
    note_lock(letter, locked);
    locked
}

/// A start's or a hand-over's argument naming a drive's root that cannot be read because it is
/// locked by BitLocker: the place it names (on a reading thread only: it asks the drive).
fn locked_root(arg: &str) -> Option<Shows> {
    let shows = named_place(arg)?;
    let letter = drive_letter_of(&shows)?;
    let locked = bevel_winkit::bitlocker::drive_locked(letter);
    note_lock(letter, locked);
    locked.then_some(shows)
}

/// Open or Explore on a place that is a drive found locked just now: Windows' own unlock window
/// for it instead, and nothing else (true). The window's thread asks no drive here.
fn unlock_instead(shows: &Shows) -> bool {
    let Some(letter) = drive_letter_of(shows).filter(|&l| known_locked(l, true)) else { return false };
    log(&format!("folders: {letter}: is locked by BitLocker; Windows' unlock window instead"));
    bevel_winkit::bitlocker::start_unlock(letter);
    true
}

/// The free space of the drive `dir` is on.
fn free_space(dir: &Path) -> Option<u64> {
    let w = wide(&dir.to_string_lossy());
    let mut free = 0u64;
    // SAFETY: a null-terminated path and a valid out-pointer.
    (unsafe { GetDiskFreeSpaceExW(w.as_ptr(), &mut free, null_mut(), null_mut()) } != 0).then_some(free)
}

/// The window of folder window `id`.
fn window_of(id: usize) -> Option<HWND> {
    WINDOWS.with(|w| w.borrow().iter().find(|(i, _, _)| *i == id).map(|&(_, h, _)| h))
}

/// Files' own pictures reach their windows (nothing moves; the rows on screen were read first).
fn pictures_ready() {
    let all = PICTURES.lock().map(|mut p| std::mem::take(&mut *p)).unwrap_or_default();
    // An earlier reader's pictures (the window's reader started again since) are dropped.
    for (id, _, updates) in all.into_iter().filter(|&(id, g, _)| is_current(id, g)) {
        let Some(hwnd) = window_of(id) else { continue };
        kit::scope::<FolderHost, _>(hwnd, || {
            kit::update::<FolderHost>(|h| {
                if h.id == id {
                    h.form.view.set_pictures(updates);
                }
            });
        });
    }
}

/// Tells the readers which items each window shows on its screen now.
fn publish_wanted() {
    let mut wanted = HashMap::new();
    for hwnd in kit::windows::<FolderHost>() {
        if let Some((id, ids)) = kit::scope::<FolderHost, _>(hwnd, || kit::with::<FolderHost, _>(|h| (h.id, h.form.visible_ids()))) {
            wanted.insert(id, ids);
        }
    }
    if let Ok(mut w) = WANTED.lock() {
        *w = Some(wanted);
    }
}

/// A reader's new items reach their window.
fn items_ready(id: usize) {
    let Some(snapshot) = SNAPSHOTS.lock().ok().and_then(|mut all| take_current(&mut all, id, |g| is_current(id, g)).pop()) else {
        return;
    };
    let Some(hwnd) = window_of(id) else { return };
    let cut = CUT.with(|c| c.borrow().0.clone());
    kit::scope::<FolderHost, _>(hwnd, || {
        kit::update::<FolderHost>(|h| {
            if h.id != id {
                return;
            }
            h.form.hidden = snapshot.hidden;
            h.form.total_size = snapshot.total_size;
            h.form.spaces = snapshot.spaces;
            h.form.disk_free = snapshot.free;
            // The hidden items shown (Windows shows hidden items) are drawn faded.
            let hidden_ids = hidden_of(&snapshot.attributes);
            h.form.web.attributes = snapshot.attributes;
            // The tree's rows of the folder shown follow its folders (one made, renamed, gone).
            if h.form.tree_shown {
                let key = h.shows.key();
                if let Some(i) = h.form.tree.nodes.iter().position(|n| n.read && node_key(&n.id) == key) {
                    let end = h.form.tree.end_of(i);
                    let depth = h.form.tree.nodes[i].depth + 1;
                    let mut had: Vec<String> = h.form.tree.nodes[i + 1..end].iter().filter(|n| n.depth == depth).map(|n| n.id.to_lowercase()).collect();
                    let mut now: Vec<String> = snapshot.items.iter().filter(|it| it.folder).map(|it| it.id.to_lowercase()).collect();
                    had.sort();
                    now.sort();
                    if had != now {
                        let id = h.form.tree.nodes[i].id.clone();
                        h.reread.push((std::time::Instant::now(), id));
                    }
                }
            }
            let n = snapshot.items.len();
            h.form.set_items(snapshot.items);
            h.form.view.set_cut(cut);
            h.form.view.set_hidden(hidden_ids);
            if let Some(asked) = h.asked.take() {
                log(&format!("folders: {n} items shown {} ms after the window asked", asked.elapsed().as_millis()));
            }
            if let Some(i) = h.select.as_ref().and_then(|id| h.form.view.index_of(id)) {
                h.form.view.select_only(Some(i));
                h.select = None;
            }
        });
    });
}

// ---------------------------------------------------------------------------------------------
// Opening windows.

// ---------------------------------------------------------------------------------------------
// What a window needs to know of its place, read off the window's thread.

/// A place's own name as Windows gives it: a drive's kind and label, a folder's display name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Named {
    drive: Option<(DriveKind, String)>,
    display: Option<String>,
}

/// What a window needs to know of its place to show it: the names of the place and of every place
/// above it, a drive's space (the web view), Windows' known folders (My Documents, the Desktop
/// folders: the title, the tree's way down) and how many items it holds (the first size and
/// view). All of it is read on a thread of its own (`read_facts`), never on the program's only
/// thread, which every window and every hand-over shares (the fourth safety review of 2026-11-18,
/// B3: a folder on a drive that did not answer held that thread, and every later folder open
/// waited a minute). The titles are made from it for each look (`title_from`).
#[derive(Clone, Debug, Default)]
struct Facts {
    /// By `Shows::key`: the place and each place above it.
    names: HashMap<String, Named>,
    /// A drive root's free space and capacity.
    space: Option<Space>,
    documents: Option<PathBuf>,
    desktops: Vec<PathBuf>,
    /// The items: a folder's or the Desktop's (with the look's three icons), My Computer's drives.
    items: usize,
    my_computer: bool,
    /// The place is a drive found locked by BitLocker by this reading (its letter).
    locked: Option<char>,
}

impl Facts {
    /// How many items the window holds in `era` (My Computer adds its look's places).
    fn count(&self, era: Era) -> usize {
        match (self.my_computer, era) {
            (false, _) => self.items,
            (true, Era::Win2000) => self.items + 1,
            (true, _) => self.items + 2,
        }
    }
}

/// Windows' known folders and the names read so far (a window going to a place shows its title
/// from them at once; the place's own reading follows).
static KNOWN: Mutex<Option<(Option<PathBuf>, Vec<PathBuf>)>> = Mutex::new(None);
static NAMES: Mutex<Option<HashMap<String, Named>>> = Mutex::new(None);

/// What is known of `shows` without asking Windows anything: the known folders and names read
/// before, no space, no count (the window's thread uses only this).
fn known_facts(shows: &Shows) -> Facts {
    let (documents, desktops) = KNOWN.lock().ok().and_then(|k| k.clone()).unwrap_or_default();
    let names = NAMES.lock().ok().and_then(|n| n.clone()).unwrap_or_default();
    let names = shows.place().chain().iter().map(|p| Shows::of_place(p).key()).filter_map(|k| names.get(&k).map(|n| (k, n.clone()))).collect();
    Facts { names, documents, desktops, ..Facts::default() }
}

/// A drive's kind and label, as My Computer reads them (`read_my_computer`). `ask_lock`: a drive
/// that cannot be read is asked whether it is locked by BitLocker (on a reading thread only:
/// false on a window's thread, the review before 0.1.3, B4).
fn drive_of(root: &Path, ask_lock: bool) -> Option<(DriveKind, String)> {
    let letter = root.to_string_lossy().chars().next()?.to_ascii_uppercase();
    let w = wide(&format!("{letter}:\\"));
    // SAFETY: a null-terminated root path.
    let drive_type = unsafe { GetDriveTypeW(w.as_ptr()) };
    let kind = match drive_type {
        2 if letter <= 'B' => DriveKind::Floppy,
        2 => DriveKind::Removable,
        3 => DriveKind::Fixed,
        4 => DriveKind::Network,
        5 => DriveKind::CdRom,
        6 => DriveKind::RamDisk,
        _ => return None,
    };
    let mut label = [0u16; 261];
    let mut text = String::new();
    if kind != DriveKind::Floppy {
        // SAFETY: a null-terminated root and a buffer of the length given.
        if unsafe { GetVolumeInformationW(w.as_ptr(), label.as_mut_ptr(), label.len() as u32, null_mut(), null_mut(), null_mut(), null_mut(), 0) } != 0 {
            let len = label.iter().position(|&c| c == 0).unwrap_or(0);
            text = String::from_utf16_lossy(&label[..len]);
            note_lock(letter, false);
        } else if ask_lock {
            // SAFETY: a plain query, right after the call that failed.
            lock_after_failed_read(letter, drive_type, unsafe { GetLastError() });
        }
    }
    Some((kind, text))
}

/// Reads what a window needs to know of `shows` (on a thread of its own: it asks drives, lists
/// the folder and resolves names), and keeps the known folders and names for `known_facts`.
fn read_facts(shows: &Shows) -> Facts {
    if let Shows::Folder(p) = shows
        && test_named(p)
        && SLOW_READING.load(Ordering::SeqCst)
    {
        // A live test's slow drive (`TEST_SLOW_READING`): only test folders.
        std::thread::sleep(Duration::from_millis(SLOW_READING_MS));
    }
    let documents = shell::documents_folder();
    let desktops: Vec<PathBuf> = [shell::desktop_folder(), shell::public_desktop_folder()].into_iter().flatten().collect();
    if let Ok(mut k) = KNOWN.lock() {
        *k = Some((documents.clone(), desktops.clone()));
    }
    let mut names = HashMap::new();
    for place in shows.place().chain() {
        if let Place::Folder(p) = &place {
            let path = PathBuf::from(p);
            let named = if path.parent().is_none() {
                Named { drive: drive_of(&path, true), display: None }
            } else {
                Named { drive: None, display: shell::display_name(p) }
            };
            names.insert(Shows::Folder(path).key(), named);
        }
    }
    if let Ok(mut all) = NAMES.lock() {
        all.get_or_insert_with(HashMap::new).extend(names.iter().map(|(k, v)| (k.clone(), v.clone())));
    }
    let space = match shows {
        Shows::Folder(p) if p.parent().is_none() => {
            let w = wide(&p.to_string_lossy());
            let (mut free, mut total) = (0u64, 0u64);
            // SAFETY: a null-terminated path and valid out-pointers.
            (unsafe { GetDiskFreeSpaceExW(w.as_ptr(), &mut free, &mut total, null_mut()) } != 0).then_some(Space { free, capacity: total })
        }
        _ => None,
    };
    let (items, my_computer) = match shows {
        Shows::MyComputer => (read_my_computer(Era::Win95, true).items.iter().filter(|i| matches!(i.picture, Picture::Art(Art::Drive(_)))).count(), true),
        Shows::Desktop => (read_desktop(Era::Win95).map_or(0, |(seen, ..)| seen.len() + 3), false),
        Shows::Folder(p) => (shell::list_folder(p).map_or(0, |(seen, _)| seen.len()), false),
    };
    let locked = drive_letter_of(shows).filter(|&l| known_locked(l, true));
    Facts { names, space, documents, desktops, items, my_computer, locked }
}

/// What a folder window is called in `era`, from what is known of its place: My Computer, the
/// Desktop, a drive's name ("Win95 (C:)"), My Documents (98, 2000), or the folder's name as
/// Explorer shows it (while it is not read yet: its own name, a drive's letter).
fn title_from(era: Era, shows: &Shows, facts: &Facts) -> (String, Small) {
    match shows {
        Shows::Desktop => ("Desktop".into(), Small::Desktop),
        Shows::MyComputer => ("My Computer".into(), Small::MyComputer),
        Shows::Folder(p) => {
            let named = facts.names.get(&shows.key());
            if p.parent().is_none() {
                // A drive's root: its name as My Computer shows it, its picture.
                let root = p.to_string_lossy().to_uppercase();
                if let Some((kind, label)) = named.and_then(|n| n.drive.as_ref()) {
                    let letter = root.chars().next().unwrap_or('C');
                    return (drive_name(era, letter, *kind, label), Small::Drive(*kind));
                }
                return (root, Small::Drive(DriveKind::Fixed));
            }
            if facts.documents.as_ref().is_some_and(|d| d == p) && era != Era::Win95 {
                return ("My Documents".into(), Small::MyDocuments);
            }
            let name = named
                .and_then(|n| n.display.clone())
                .unwrap_or_else(|| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
            (name, Small::Folder)
        }
    }
}

/// `title_from` with the place read now, on the calling thread (the places menu and the address
/// list, which a person opens in a window already shown).
fn title_of(era: Era, shows: &Shows) -> (String, Small) {
    title_from(era, shows, &read_facts_names(shows))
}

/// Only the names `title_from` needs (no listing, no space). On a window's thread: no drive is
/// asked whether it is locked (`drive_of(.., false)`).
fn read_facts_names(shows: &Shows) -> Facts {
    let mut facts = known_facts(shows);
    if let Shows::Folder(p) = shows {
        let named = if p.parent().is_none() { Named { drive: drive_of(p, false), display: None } } else { Named { drive: None, display: shell::display_name(&p.to_string_lossy()) } };
        facts.names.insert(shows.key(), named);
        if facts.documents.is_none() {
            facts.documents = shell::documents_folder();
        }
    }
    facts
}

/// What the program was asked to open: My Computer's id, or a folder (a drive's root ends in "\").
fn shows_of(arg: &str) -> Option<Shows> {
    named_place(arg).filter(|s| match s {
        Shows::Folder(path) => path.is_dir(),
        _ => true,
    })
}

/// `shows_of` without asking the drive whether it is a folder (None: no place at all, an empty
/// argument).
fn named_place(arg: &str) -> Option<Shows> {
    if arg == MY_COMPUTER || arg.eq_ignore_ascii_case("::{20D04FE0-3AEA-1069-A2D8-08002B30309D}") {
        return Some(Shows::MyComputer);
    }
    if arg == DESKTOP {
        return Some(Shows::Desktop);
    }
    let mut path = PathBuf::from(arg.trim_matches('"'));
    if path.as_os_str().len() == 2 && arg.ends_with(':') {
        path = PathBuf::from(format!("{arg}\\"));
    }
    (!path.as_os_str().is_empty()).then_some(Shows::Folder(path))
}

/// Shows a new place in the window (98 and 2000; 95's Explorer): a new window, a navigation, a
/// change of look (whose items are put back by its caller). The view starts afresh (no items, no
/// selection, scrolled to the top) and the reader brings the place's items. Its title and
/// pictures, the address, where Back, Forward and Up can go, the places 98's File menu lists.
fn show_new_place(h: &mut FolderHost, era: Era) {
    let (title, small) = title_from(era, &h.shows, &h.facts);
    let address = bevel_core::folder::address::wording(era, &h.shows.place(), &title);
    let files = matches!(h.shows, Shows::Folder(_)) || h.shows == Shows::Desktop;
    h.form.show_place(&title, art::small(era, small), files, &address, art::listed(era, small));
    place_known(h, era, &title);
    apply_web(h, era);
    show_row(h);
}

/// More is known of the place the window shows (its late reading, `apply_facts`): only what was
/// read changes (the title, the window's and the address bar's pictures, the address text, where
/// Back, Forward and Up can go, the web view's facts). The items, the selection, the scroll, the
/// view's kind and order, a rename box and an open menu stay: the reader tells only of changes,
/// so items thrown away here would stay away until Refresh (an outside review of 2026-11-18).
fn show_what_is_known(h: &mut FolderHost, era: Era) {
    let (title, small) = title_from(era, &h.shows, &h.facts);
    let address = bevel_core::folder::address::wording(era, &h.shows.place(), &title);
    h.form.update_place(&title, art::small(era, small), &address, art::listed(era, small));
    place_known(h, era, &title);
    show_row(h);
    if era != Era::Win95 {
        let (kind, art, space) = web_place(era, &h.shows, &h.facts);
        h.form.set_web_known(kind, art, space);
    }
}

/// What `show_new_place` and `show_what_is_known` both set from the place: Back, Forward, Up and
/// 98's chain of places, Explorer's title and picture.
fn place_known(h: &mut FolderHost, era: Era, title: &str) {
    let place = h.shows.place();
    let chain = place
        .chain()
        .iter()
        .map(|p| match p {
            Place::Folder(_) => title_from(era, &Shows::of_place(p), &h.facts).0.trim().to_string(),
            Place::MyComputer => "My Computer".into(),
            Place::Desktop => "Desktop".into(),
        })
        .collect();
    h.form.set_nav(Nav {
        back: h.history.can_back(),
        forward: h.history.can_forward(),
        up: place.parent(era).is_some(),
        chain: if place == Place::Desktop { vec!["Desktop".into()] } else { chain },
        at_my_computer: place == Place::MyComputer,
    });
    // Explorer: 95 and 98 titled the window "Exploring - <place>"; the Explorer's picture in 95's
    // and 98's Explorer windows, in 2000's while the tree shows.
    if h.form.exploring && era != Era::Win2000 {
        h.form.title = format!("Exploring - {title}");
    }
    if (h.form.exploring && era != Era::Win2000) || (h.form.tree_shown && era == Era::Win2000) {
        h.form.icon = tree_art::explorer(era);
    }
}

/// The web view panel's fonts for `era` (98: Verdana; 2000: the look's Tahoma and its 13pt bold),
/// taken from the stored letters once per look; `None` in 95 (no web view there).
fn web_fonts(era: Era) -> Option<Arc<WebFonts>> {
    if era == Era::Win95 {
        return None;
    }
    if let Some(f) = WEB_FONTS.with(|w| w.borrow().as_ref().filter(|(e, _)| *e == era).map(|(_, f)| f.clone())) {
        return Some(f);
    }
    let (_, look) = the_look()?;
    let made = match era {
        Era::Win98 => WebFonts { body: font::letters("Verdana", 11, false), bold: font::letters("Verdana", 11, true), title: font::letters("Verdana", 21, true) },
        _ => WebFonts { body: look.regular.clone(), bold: look.bold.clone(), title: font::letters("Tahoma", 17, true) },
    };
    let f = Arc::new(made);
    WEB_FONTS.with(|w| *w.borrow_mut() = Some((era, f.clone())));
    Some(f)
}

/// The system folders whose web view warned before showing their files: the Windows folder and
/// its System and System32 (98; 2000 only system32), and Program Files (also the x86 one).
fn guard_of(era: Era, shows: &Shows) -> Option<Guard> {
    let Shows::Folder(p) = shows else { return None };
    // A live test's stand-in for a system folder (the tests open no real one).
    if p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("Bevel95 test guard")) {
        return Some(Guard::System);
    }
    let path = p.to_string_lossy().trim_end_matches('\\').to_lowercase();
    let env = |name: &str| std::env::var(name).ok().map(|v| v.trim_end_matches('\\').to_lowercase());
    if let Some(windows) = env("SystemRoot") {
        let mut system = vec![windows.clone(), format!("{windows}\\system32")];
        if era == Era::Win98 {
            system.push(format!("{windows}\\system"));
        }
        if system.contains(&path) {
            return Some(Guard::System);
        }
    }
    let program_files = [env("ProgramFiles"), env("ProgramFiles(x86)")];
    program_files.iter().flatten().any(|pf| *pf == path).then_some(Guard::ProgramFiles)
}

/// What the panel shows for the window's place: its kind, its 32×32 picture, a drive's space.
fn web_place(era: Era, shows: &Shows, facts: &Facts) -> (WebKind, Option<bevel_core::fb::Icon>, Option<Space>) {
    let (kind, picture, space) = match shows {
        Shows::MyComputer => (WebKind::MyComputer, Art::MyComputer, None),
        Shows::Desktop => (WebKind::Folder, Art::Folder, None),
        Shows::Folder(p) if p.parent().is_none() => {
            let (_, small) = title_from(era, shows, facts);
            let drive = match small {
                Small::Drive(k) => k,
                _ => DriveKind::Fixed,
            };
            (WebKind::Drive, Art::Drive(drive), facts.space)
        }
        Shows::Folder(p) if era != Era::Win95 && facts.documents.as_ref().is_some_and(|d| d == p) => (WebKind::MyDocuments, Art::MyDocuments, None),
        Shows::Folder(_) => (WebKind::Folder, Art::Folder, None),
    };
    (kind, bevel_core::desktop_art::picture(era, picture, false, 0), space)
}

/// Whether the web view is on for the folder key (98: as Web Page; 2000: Folder Options).
fn web_on(era: Era, key: &str) -> bool {
    SAVED.with(|s| {
        let s = s.borrow();
        match era {
            Era::Win98 => !s.web_off.contains(key),
            Era::Win2000 => !s.classic_folders,
            Era::Win95 => false,
        }
    })
}

/// Gives the window's web view its place and switches it as the look keeps it (98: View > as Web
/// Page, per folder; 2000: Folder Options, for every folder).
fn apply_web(h: &mut FolderHost, era: Era) {
    if era == Era::Win95 {
        h.form.set_web(false);
        return;
    }
    let key = h.shows.key();
    let (on, shown) = SAVED.with(|s| {
        let s = s.borrow();
        let on = match era {
            Era::Win98 => !s.web_off.contains(&key),
            _ => !s.classic_folders,
        };
        (on, era == Era::Win2000 && s.shown_files.contains(&key))
    });
    h.form.web.fonts = web_fonts(era);
    let (kind, art, space) = web_place(era, &h.shows, &h.facts);
    h.form.set_web_place(kind, art, guard_of(era, &h.shows), shown, space);
    h.form.set_web(on && h.form.web.fonts.is_some());
}

/// Opens Windows' own window for `target` (a shell: place or a Control Panel item) on a thread of
/// its own.
fn shell_open(target: String) {
    let spawned = std::thread::Builder::new().name("folder open".into()).spawn(move || {
        bevel_winkit::com::init();
        let file = wide(&target);
        // SAFETY: a null-terminated name; a null verb is its default command.
        let r = unsafe { ShellExecuteW(null_mut(), null(), file.as_ptr(), null(), null(), SW_SHOWNORMAL) };
        if r as usize <= 32 {
            log(&format!("folders: could not open an item (ShellExecute error {})", r as usize));
        }
    });
    if let Err(e) = spawned {
        log(&format!("folders: could not start opening an item: {e}"));
    }
}

/// A link of the web view was clicked.
fn web_link(hwnd: HWND, link: Link) {
    match link {
        Link::ShowFiles => {
            // 2000 remembered it for the folder (98 showed the warning again each time).
            let remembered = kit::with::<FolderHost, _>(|h| (h.form.era == Era::Win2000).then(|| h.shows.key())).flatten();
            if let Some(key) = remembered {
                SAVED.with(|s| s.borrow_mut().shown_files.insert(key));
                save();
            }
        }
        Link::MyComputer => navigate(hwnd, Some(Shows::MyComputer), Go::New),
        Link::MyDocuments => {
            if let Some(d) = shell::documents_folder() {
                navigate(hwnd, Some(Shows::Folder(d)), Go::New);
            }
        }
        Link::NetworkPlaces => shell_open("shell:NetworkPlacesFolder".into()),
        Link::NetworkConnections => shell_open("shell:ConnectionsFolder".into()),
        // Windows' own, by its whole path (the third safety review of 2026-11-18, C1).
        Link::AddRemove => shell_open(shell::system_program("appwiz.cpl")),
    }
}

/// Opens the window of `shows`, or brings its window forward if it is open (Windows 95: a
/// folder's window comes forward as it was; 98 and 2000 the same for a window showing it).
fn open(shows: Shows) {
    open_window(shows, false);
}

/// Opens a new Explorer window (the tree of all folders beside the icons) on `shows`: Start >
/// Programs > Windows Explorer, Explore on an item's menu. Each is a new window, as in Windows.
fn open_explorer(shows: Shows) {
    open_window(shows, true);
}

/// The key a window is found by in `WINDOWS`: an Explorer window's never matches a folder's own.
fn window_key(shows: &Shows, exploring: bool) -> String {
    if exploring { format!("explore:{}", shows.key()) } else { shows.key() }
}

/// The window of `shows` if one is open (an Explorer window is never reused): brought forward.
fn bring_forward(shows: &Shows, exploring: bool) -> bool {
    let key = window_key(shows, exploring);
    let Some(hwnd) = WINDOWS.with(|w| w.borrow().iter().find(|(_, _, k)| *k == key && !exploring).map(|&(_, h, _)| h)) else {
        return false;
    };
    // SAFETY: our own window.
    unsafe {
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }
        SetForegroundWindow(hwnd);
    }
    bevel_winkit::util::take_focus(hwnd);
    true
}

/// Opens the window of `shows` (or brings it forward): its place is read on a thread of its own
/// first (`ask`), and the window opens as soon as that is known, or after `PATIENCE` with what is
/// known without asking (its title then follows).
fn open_window(shows: Shows, exploring: bool) {
    if unlock_instead(&shows) {
        return;
    }
    if !bring_forward(&shows, exploring) {
        ask(Ask::Shows(shows, exploring));
    }
}

/// What a window is asked for: a start's or a hand-over's argument (read off the window's thread
/// to tell whether it is a folder), or a place already known to be one.
#[derive(Clone, Debug)]
enum Ask {
    Arg(String),
    Shows(Shows, bool),
}

/// A window asked for whose place is being read.
struct Asked {
    serial: usize,
    what: Ask,
    /// When the window opens with what is known if the reading has not come (a start's or a
    /// hand-over's argument too, since the sixth safety review of 2026-11-18, C3: an argument whose
    /// reading never came kept the program for ever, without a window: `known_of`).
    deadline: std::time::Instant,
    /// It opened (or an open one came forward) before its reading came: the window, if any, and
    /// its id then (the place it showed when it opened: `on_reading`).
    opened: Option<Option<(HWND, usize)>>,
}

/// What a place's reading does when it comes for a window asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
enum OnReading {
    /// The window opened before it came: it takes the reading, only while it still shows the
    /// place it opened with (`id`, checked by `apply_facts`).
    Apply { hwnd: HWND, id: usize },
    /// No window yet: it opens now (or an open one of that place comes forward).
    Open(Shows, bool),
    /// Nothing: it was not a folder, or the early window has gone or was one that came forward.
    Nothing,
}

/// What the reading `found` for an asking does, by what happened while it was read (`opened`)
/// and whether that window still exists (`alive`). The fifth safety review of 2026-11-18, C2: a
/// window that opened early took the reading even after it had gone elsewhere (the wrong drive's
/// space and a lost title), since it was applied without the window's id.
fn on_reading(opened: Option<Option<(HWND, usize)>>, found: Option<(Shows, bool)>, alive: impl Fn(HWND) -> bool) -> OnReading {
    match (opened, found) {
        (Some(Some((hwnd, id))), _) if alive(hwnd) => OnReading::Apply { hwnd, id },
        (Some(_), _) | (None, None) => OnReading::Nothing,
        (None, Some((shows, exploring))) => OnReading::Open(shows, exploring),
    }
}

/// How long a new window waits for its place's reading before it opens with what is known.
const PATIENCE: Duration = Duration::from_millis(500);

thread_local! {
    static ASKED: RefCell<Vec<Asked>> = const { RefCell::new(Vec::new()) };
    static NEXT_ASK: Cell<usize> = const { Cell::new(1) };
}

/// Places read for windows asked for: (the asking's number, what it was found to be (None: not a
/// folder), its facts).
type Prepared = (usize, Option<(Shows, bool)>, Facts);
static PREPARED: Mutex<Vec<Prepared>> = Mutex::new(Vec::new());
/// Places read again for windows that went there (`navigate`): (the window's id, its facts).
static REREAD: Mutex<Vec<(usize, Facts)>> = Mutex::new(Vec::new());

/// What an argument opens (on the reading thread: it asks the drive whether it is a folder).
fn resolve(what: &Ask, era: Era) -> Option<(Shows, bool)> {
    match what {
        Ask::Shows(s, exploring) => Some((s.clone(), *exploring)),
        Ask::Arg(arg) if arg == EXPLORER => Some((explorer_start(era), true)),
        Ask::Arg(arg) => {
            if let Some(place) = arg.strip_prefix(EXPLORE) {
                let place = if place == "::my-documents" { shell::documents_folder().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default() } else { place.to_string() };
                let found = shows_of(&place).or_else(|| locked_root(&place)).map(|s| (s, true));
                if found.is_none() {
                    log("folders: asked to explore something that is not a folder");
                }
                return found;
            }
            let found = shows_of(arg).or_else(|| locked_root(arg)).map(|s| (s, false));
            if found.is_none() {
                log("folders: asked to open something that is not a folder");
            }
            found
        }
    }
}

/// What is known of the place of `what` without asking its drive (the window's thread): a start's
/// or a hand-over's argument taken as the place it names. None: it names no place at all.
fn known_of(what: &Ask, era: Era) -> Option<(Shows, bool)> {
    match what {
        Ask::Shows(s, exploring) => Some((s.clone(), *exploring)),
        Ask::Arg(arg) if arg == EXPLORER => Some((explorer_start(era), true)),
        Ask::Arg(arg) => match arg.strip_prefix(EXPLORE) {
            Some("::my-documents") => shell::documents_folder().map(|d| (Shows::Folder(d), true)),
            Some(place) => named_place(place).map(|s| (s, true)),
            None => named_place(arg).map(|s| (s, false)),
        },
    }
}

/// Places whose reading thread ended without an answer (a panic in it): "not read" at once.
static NOT_READ: Mutex<Vec<usize>> = Mutex::new(Vec::new());

/// The answer of a place's reading thread, handed over when the thread ends in any way: its
/// reading (`PREPARED`), or, ended without one, "not read" (`NOT_READ`; the sixth safety review
/// of 2026-11-18, C3: a reading thread that panicked left its asking for ever).
struct Answer {
    serial: usize,
    hub: usize,
    reading: Option<(Option<(Shows, bool)>, Facts)>,
}

impl Drop for Answer {
    fn drop(&mut self) {
        match self.reading.take() {
            Some((found, facts)) => PREPARED.lock().map(|mut p| p.push((self.serial, found, facts))).ok(),
            None => NOT_READ.lock().map(|mut n| n.push(self.serial)).ok(),
        };
        if self.hub != 0 {
            // SAFETY: posting to the program's own hidden window.
            unsafe { PostMessageW(self.hub as HWND, WM_ITEMS.id(), 0, 6) };
        }
    }
}

/// Reads the place of `what` on a thread of its own; the window opens when it is known.
fn ask(what: Ask) {
    let Some((era, _)) = the_look() else { return };
    let serial = NEXT_ASK.with(|n| n.replace(n.get() + 1));
    ASKED.with(|a| a.borrow_mut().push(Asked { serial, what: what.clone(), deadline: std::time::Instant::now() + PATIENCE, opened: None }));
    let hub = HUB_FOR_THREADS.load(Ordering::SeqCst);
    let spawned = std::thread::Builder::new().name("folder place".into()).spawn(move || {
        let mut answer = Answer { serial, hub, reading: None };
        bevel_winkit::com::init();
        let found = resolve(&what, era);
        let facts = found.as_ref().map(|(s, _)| read_facts(s)).unwrap_or_default();
        answer.reading = Some((found, facts));
    });
    if let Err(e) = spawned {
        log(&format!("folders: could not start reading a place: {e}"));
        ASKED.with(|a| a.borrow_mut().retain(|x| x.serial != serial));
    }
}

/// The places read reach their windows: a window opens, or the one opened meanwhile gets its title.
fn prepared_ready() {
    let all = PREPARED.lock().map(|mut p| std::mem::take(&mut *p)).unwrap_or_default();
    for (serial, found, facts) in all {
        let Some(asked) = ASKED.with(|a| {
            let mut a = a.borrow_mut();
            let at = a.iter().position(|x| x.serial == serial)?;
            Some(a.remove(at))
        }) else {
            continue;
        };
        let open = kit::windows::<FolderHost>();
        match on_reading(asked.opened, found, |hwnd| open.contains(&hwnd)) {
            OnReading::Apply { hwnd, id } => apply_facts(hwnd, id, facts),
            OnReading::Nothing => {}
            OnReading::Open(shows, exploring) => {
                if let Some(letter) = facts.locked {
                    // A drive locked by BitLocker: Windows' own unlock window, and no window of ours.
                    log(&format!("folders: {letter}: is locked by BitLocker; Windows' unlock window instead"));
                    bevel_winkit::bitlocker::start_unlock(letter);
                } else if !bring_forward(&shows, exploring) {
                    create_window(shows, exploring, facts);
                }
            }
        }
    }
    let again = REREAD.lock().map(|mut p| std::mem::take(&mut *p)).unwrap_or_default();
    for (id, facts) in again {
        if let Some(hwnd) = window_of(id) {
            apply_facts(hwnd, id, facts);
        }
    }
    // Readings that ended without an answer: "not read" at once.
    let unread = NOT_READ.lock().map(|mut n| std::mem::take(&mut *n)).unwrap_or_default();
    if !unread.is_empty() {
        log("folders: a place's reading ended without an answer; counted as not read");
        ASKED.with(|a| not_read(&mut a.borrow_mut(), &unread, std::time::Instant::now()));
        patience();
    }
}

/// The askings `unread` whose reading ended without an answer: one whose window has opened waits
/// for nothing more (it goes); one still to open is due at once (`patience` opens it with what is
/// known).
fn not_read(asked: &mut Vec<Asked>, unread: &[usize], now: std::time::Instant) {
    asked.retain(|x| !(unread.contains(&x.serial) && x.opened.is_some()));
    for x in asked.iter_mut().filter(|x| unread.contains(&x.serial)) {
        x.deadline = now;
    }
}

/// The askings whose window opens now with what is known: not opened yet and past their patience,
/// a place known to be one and a start's or a hand-over's argument alike (the sixth safety review
/// of 2026-11-18, C3: an argument waited for its reading without limit).
fn due(asked: &[Asked], now: std::time::Instant) -> Vec<(usize, Ask)> {
    asked.iter().filter(|x| x.opened.is_none() && x.deadline <= now).map(|x| (x.serial, x.what.clone())).collect()
}

/// Whether an asking's window is still to open (one that opened early only waits for its
/// reading's title and keeps nothing alive).
fn still_to_open(asked: &[Asked]) -> bool {
    asked.iter().any(|x| x.opened.is_none())
}

/// A place whose reading has not come within `PATIENCE`: its window opens with what is known now
/// (the hidden window's timer). After a panic nothing new opens (the panic hook has closed the
/// windows so that the program ends): such an asking goes.
fn patience() {
    let Some((era, _)) = the_look() else { return };
    let due: Vec<(usize, Ask)> = ASKED.with(|a| due(&a.borrow(), std::time::Instant::now()));
    for (serial, what) in due {
        let known = if bevel_winkit::panic::happened() { None } else { known_of(&what, era) };
        let Some((shows, exploring)) = known else {
            // Nothing to show: no window opens, and nothing keeps the program for it.
            log("folders: a place asked for could not be shown; no window opens for it");
            ASKED.with(|a| a.borrow_mut().retain(|x| x.serial != serial));
            continue;
        };
        let opened = if bring_forward(&shows, exploring) {
            None
        } else {
            log("folders: a place is slow to read; its window opens with what is known, its title follows");
            // With the window's id now: its reading is taken only while it still shows this place.
            create_window(shows.clone(), exploring, known_facts(&shows))
                .and_then(|hwnd| kit::scope::<FolderHost, _>(hwnd, || kit::with::<FolderHost, _>(|h| h.id)).map(|id| (hwnd, id)))
        };
        ASKED.with(|a| {
            if let Some(x) = a.borrow_mut().iter_mut().find(|x| x.serial == serial) {
                x.opened = Some(opened);
            }
        });
    }
}

/// A window's place has been read: its title, pictures, address and web view follow, only while
/// the window still shows the place it was read for (its id `id`, which every navigation changes).
fn apply_facts(hwnd: HWND, id: usize, facts: Facts) {
    let Some((era, _)) = the_look() else { return };
    let changed = kit::scope::<FolderHost, _>(hwnd, || {
        let mut changed = None;
        kit::update::<FolderHost>(|h| {
            if id != h.id {
                return;
            }
            h.facts = facts;
            let before = h.form.view.items.len();
            let (title, small) = title_from(era, &h.shows, &h.facts);
            h.form.title = title;
            h.form.icon = art::small(era, small);
            if era != Era::Win95 || h.form.tree_shown {
                show_what_is_known(h, era);
            }
            if before > 0 {
                log(&format!("folders: a place's reading came after its items: {before} items shown, {} after it", h.form.view.items.len()));
            }
            changed = Some((wide(&h.form.title), h.form.icon.clone()));
        });
        changed
    });
    if let Some((title, icon)) = changed {
        // SAFETY: our own window and a null-terminated title (the task button shows it).
        unsafe { SetWindowTextW(hwnd, title.as_ptr()) };
        kit::scope::<FolderHost, _>(hwnd, || kit::set_program_icons::<FolderHost>(&icon, &icon));
    }
}

/// Carries out the program's messages while no window is open and one asked for is still being
/// read (`ask`): it opens once its place is known, or after `PATIENCE`.
fn wait_for_asked() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{DispatchMessageW, GetMessageW, MSG, TranslateMessage};
    while kit::windows::<FolderHost>().is_empty() && ASKED.with(|a| still_to_open(&a.borrow())) {
        let mut msg = MSG::default();
        // SAFETY: this thread's own message queue and a valid out-pointer.
        let got = unsafe { GetMessageW(&mut msg, null_mut(), 0, 0) };
        if got <= 0 {
            // WM_QUIT (or an error): the program ends; nothing asked for opens any more.
            ASKED.with(|a| a.borrow_mut().clear());
            return;
        }
        // SAFETY: a message this thread just took.
        unsafe {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// Makes the window of `shows` with what is known of its place.
fn create_window(shows: Shows, exploring: bool, facts: Facts) -> Option<HWND> {
    let key = window_key(&shows, exploring);
    let (era, look) = the_look()?;
    let scale = look.scale;
    let (title, small) = title_from(era, &shows, &facts);
    // How many items, for the first size (only counted; the reader reads them).
    let count = facts.count(era);
    let remembered = SAVED.with(|s| if exploring { s.borrow().explorer.clone() } else { s.borrow().windows.get(&key).cloned() });
    let work = work_area(None);
    let (ww, wh) = (scale.old_fit(rect_w(&work)).max(1), scale.old_fit(rect_h(&work)).max(1));
    let size = match &remembered {
        Some(w) => (scale.old_fit(w.w).max(120), scale.old_fit(w.h).max(60)),
        None if exploring => status::explorer_size(era, (ww, wh)),
        // 98 opened a folder with its web view at its largest first size (web-v1-none: 512 x 461).
        None if era == Era::Win98 && web_on(era, &key) => status::first_size_in(era, 10_000, (ww, wh)),
        None => status::first_size_in(era, count, (ww, wh)),
    };
    let files = matches!(shows, Shows::Folder(_)) || shows == Shows::Desktop;
    let mut form = FolderWindow::new(era, &title, art::small(era, small), files, size);
    form.status_bar = SAVED.with(|s| s.borrow().status_bar);
    // The tree: in an Explorer window; in 98's and 2000's own windows of folders closed with it.
    let with_tree = exploring || (era != Era::Win95 && SAVED.with(|s| s.borrow().tree_folders.contains(&key)));
    if with_tree {
        form.exploring = exploring;
        if let Some(w) = SAVED.with(|s| s.borrow().tree_width) {
            form.tree_width = w;
        }
        form.show_tree(true);
        form.tree_focus = era != Era::Win95;
    }
    let id = NEXT_ID.with(|n| n.replace(n.get() + 1));
    let stop = Arc::new(AtomicBool::new(false));
    let mut host = FolderHost {
        form,
        id,
        shows: shows.clone(),
        stop: stop.clone(),
        select: None,
        history: History::new(shows.place()),
        came: None,
        entries: Vec::new(),
        settle: None,
        reread: Vec::new(),
        asked: Some(std::time::Instant::now()),
        facts,
        session: HashMap::new(),
        tab: NEXT_TAB.with(|n| n.replace(n.get() + 1)),
        order: Vec::new(),
        back: Vec::new(),
    };
    host.order.push(host.tab);
    if era != Era::Win95 || with_tree {
        show_new_place(&mut host, era);
    }
    show_row(&mut host);
    if with_tree {
        let chain = tree_chain(era, &shows, false, &host.facts);
        host.form.tree.follow(chain, 20);
    }
    if let Some(w) = &remembered {
        host.form.view.set_arrange(w.arrange);
        host.form.view.set_auto_arrange(w.auto_arrange);
    }
    // The view it opens in: Explorer's own (95, 98), the folder's own, or each Windows' rule.
    let mut state = opening_state(era, &shows, exploring, count, true);
    if state.arrange == bevel_core::desktop_icons::Arrange::Name
        && !state.reversed
        && let Some(w) = &remembered
    {
        state.arrange = w.arrange;
    }
    host.form.apply_view_state(&state);
    if load_places(&mut host, Relaid::Reopened) {
        save();
    }
    host.form.list_fixed = era == Era::Win95 && state.mode == bevel_core::desktop_icons::ViewMode::List;
    let place = |w: i32, h: i32| match &remembered {
        Some(r) => on_screen((r.x, r.y), w, h),
        None => {
            let at = LAST_CASCADE.with(|c| {
                let next = status::next_cascade(c.get(), wh);
                c.set(Some(next));
                next
            });
            // 2000's windows were cut to the work area where they opened (`mydocs-new`).
            on_screen((work.left + scale.at(at), work.top + scale.at(at)), w.min(rect_w(&work)), h.min(rect_h(&work)))
        }
    };
    let kind = Kind::ResizableProgram { small: Some(host.form.icon.clone()), big: None };
    let Some(hwnd) = kit::open_another(kind, host, look, place) else {
        log("folders: a folder window could not be made");
        return None;
    };
    WINDOWS.with(|w| w.borrow_mut().push((id, hwnd, key)));
    // It takes dragged files (from its own icons, the desktop, Windows' Explorer, any program).
    bevel_winkit::dragdrop::register(hwnd, folder_drop);
    if remembered.as_ref().is_some_and(|r| r.maximized) {
        // SAFETY: our own window.
        unsafe { ShowWindow(hwnd, SW_MAXIMIZE) };
    }
    log(&format!("folders: opened a window{} ({} open)", if exploring { " with the tree" } else { "" }, WINDOWS.with(|w| w.borrow().len())));
    start_reader(id, shows, era, stop, HUB.with(Cell::get), StartedBy::Person);
    if with_tree {
        let wanted = kit::scope::<FolderHost, _>(hwnd, || kit::with::<FolderHost, _>(|h| h.form.tree.wanted()).unwrap_or_default());
        read_tree_rows(hwnd, era, wanted, OpenedBy::Person);
    }
    Some(hwnd)
}

/// How a window goes to another place (98 and 2000).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Go {
    /// A new place: after the one shown, Forward's list forgotten.
    New,
    /// `n` places back or forward.
    Back(usize),
    Forward(usize),
    /// The place shown cannot be shown (a drive found locked): the step that came there undone
    /// (`History::undo`).
    Undo,
}

/// The current window goes to another place in the same window (Windows 98 and 2000): its reader
/// stops, a new one reads the new place, the title, pictures, address and buttons follow. Windows
/// 95 opens a window of its own instead.
fn navigate(hwnd: HWND, to: Option<Shows>, go: Go) {
    let Some((era, _)) = the_look() else { return };
    if to.as_ref().is_some_and(unlock_instead) {
        return;
    }
    // Windows 95 opened a window per folder; its Explorer went to the folder in the same window.
    let with_tree = kit::with::<FolderHost, _>(|h| h.form.tree_shown || h.form.exploring).unwrap_or(false);
    // A new tab goes to its place in its own window, in 95 too (brief 110).
    if era == Era::Win95 && !with_tree && !IN_NEW_TAB.with(Cell::get) {
        if let Some(s) = to {
            open(s);
        }
        return;
    }
    let from_tree = FROM_TREE.with(Cell::get);
    let done = kit::with::<FolderHost, _>(|h| {
        let place = match go {
            Go::New => {
                let s = to?;
                if s == h.shows {
                    return None;
                }
                h.history.go(s.place());
                h.came = Some(Step::New);
                s.place()
            }
            Go::Back(n) => {
                let p = h.history.back(n)?;
                h.came = Some(Step::Back(n));
                p
            }
            Go::Forward(n) => {
                let p = h.history.forward(n)?;
                h.came = Some(Step::Forward(n));
                p
            }
            Go::Undo => {
                let p = h.history.undo(h.came)?;
                // The place gone back to is one the window showed before; how it first came
                // there is not known any more.
                h.came = None;
                p
            }
        };
        let shows = Shows::of_place(&place);
        h.stop.store(true, Ordering::SeqCst);
        h.stop = Arc::new(AtomicBool::new(false));
        let id = NEXT_ID.with(|n| n.replace(n.get() + 1));
        // The folder left keeps its view for the next time.
        remember_view(h);
        let mode_before = h.form.view_mode();
        // The id left behind has no reader any more: its entry goes (the review before 0.1.5, C2).
        if let Ok(mut g) = GENERATIONS.lock()
            && let Some(g) = g.as_mut()
        {
            g.forget(h.id);
        }
        h.id = id;
        h.shows = shows.clone();
        h.select = None;
        h.asked = Some(std::time::Instant::now());
        let in_documents = h.form.tree.selected().is_some_and(|n| n.kind == NodeKind::MyDocuments)
            || h.form.tree.chain(h.form.tree.sel).iter().any(|&c| h.form.tree.nodes[c].kind == NodeKind::MyDocuments);
        // What is known of the place at once (no drive is asked here); its reading follows.
        h.facts = known_facts(&shows);
        show_new_place(h, era);
        let mut state = opening_state(era, &shows, h.form.exploring, 0, false);
        if h.form.exploring && era != Era::Win2000 {
            // 95's and 98's Explorer kept its own view from folder to folder.
            state.mode = mode_before;
        }
        h.form.apply_view_state(&state);
        if load_places(h, Relaid::Returned) {
            save();
        }
        // 2000 showed the focus frame in a window that went to another place.
        h.form.view.keys_used = true;
        h.settle = None;
        if h.form.tree_shown && !from_tree {
            let chain = tree_chain(era, &shows, in_documents, &h.facts);
            let page = h.form.tree_page();
            h.form.tree.follow(chain, page);
            h.form.tree_selection_shown();
        }
        Some((id, shows, h.stop.clone(), h.form.title.clone(), h.form.icon.clone(), h.form.exploring))
    })
    .flatten();
    let Some((id, shows, stop, title, icon, exploring)) = done else { return };
    let key = window_key(&shows, exploring);
    WINDOWS.with(|w| {
        if let Some(entry) = w.borrow_mut().iter_mut().find(|(_, h, _)| *h == hwnd) {
            *entry = (id, hwnd, key);
        }
    });
    let text = wide(&title);
    // SAFETY: our own window and a null-terminated title (the task button shows it).
    unsafe { SetWindowTextW(hwnd, text.as_ptr()) };
    kit::set_program_icons::<FolderHost>(&icon, &icon);
    kit::update::<FolderHost>(|_| {});
    start_reader(id, shows.clone(), era, stop, HUB.with(Cell::get), StartedBy::Person);
    // The place's names, space and known folders, read off this thread; they reach the window
    // while it still shows that place (`prepared_ready`).
    let hub = HUB_FOR_THREADS.load(Ordering::SeqCst);
    let spawned = std::thread::Builder::new().name("folder place".into()).spawn(move || {
        bevel_winkit::com::init();
        if let Shows::Folder(p) = &shows
            && !p.is_dir()
        {
            log("folders: a place of the history is gone");
        }
        let facts = read_facts(&shows);
        REREAD.lock().map(|mut r| r.push((id, facts))).ok();
        // SAFETY: posting to the program's own hidden window.
        unsafe { PostMessageW(hub as HWND, WM_ITEMS.id(), 0, 6) };
    });
    if let Err(e) = spawned {
        log(&format!("folders: could not start reading a place: {e}"));
    }
}

// ---------------------------------------------------------------------------------------------
// Explorer's tree.

/// What a tree row stands for, as a window's place (`None`: Control Panel, Printers, the
/// Recycle Bin, which open Windows' own).
fn node_shows(id: &str) -> Option<Shows> {
    match id {
        DESKTOP => Some(Shows::Desktop),
        MY_COMPUTER => Some(Shows::MyComputer),
        "::my-documents" => shell::documents_folder().map(Shows::Folder),
        _ if id.starts_with("::") => None,
        _ => Some(Shows::Folder(PathBuf::from(id))),
    }
}

/// A tree row's id as a window's key (`Shows::key`).
fn node_key(id: &str) -> String {
    node_shows(id).map(|s| s.key()).unwrap_or_else(|| id.to_string())
}

/// The ids of the tree's rows from the Desktop's rows down to `shows` (`pane::Tree::follow`):
/// through My Documents (98, 2000) or a folder on the desktop when the place is inside them,
/// else through My Computer and the drive.
/// The known folders come from `facts` (read off the window's thread).
fn tree_chain(era: Era, shows: &Shows, from_documents: bool, facts: &Facts) -> Vec<String> {
    let path = match shows {
        Shows::Desktop => return vec![DESKTOP.to_string()],
        Shows::MyComputer => return vec![MY_COMPUTER.to_string()],
        Shows::Folder(p) => p.clone(),
    };
    let lower = |p: &Path| p.to_string_lossy().to_lowercase();
    let below = |base: &Path| {
        let (b, p) = (lower(base), lower(&path));
        p == b || p.starts_with(&format!("{}\\", b.trim_end_matches('\\')))
    };
    let walk = |base: &Path, first: Vec<String>| {
        let mut out = first;
        let rest = path.strip_prefix(base).map(Path::to_path_buf).unwrap_or_default();
        let mut at = base.to_path_buf();
        for part in rest.components() {
            at.push(part);
            out.push(at.to_string_lossy().into_owned());
        }
        out
    };
    if era != Era::Win95
        && let Some(docs) = facts.documents.clone()
        && below(&docs)
        && (from_documents || lower(&docs) == lower(&path))
    {
        return walk(&docs, vec!["::my-documents".to_string()]);
    }
    for desk in &facts.desktops {
        if below(desk) && lower(desk) != lower(&path) {
            let rest = path.strip_prefix(desk).ok().and_then(|r| r.components().next()).map(|c| desk.join(c));
            if let Some(first) = rest {
                return walk(&first, vec![first.to_string_lossy().into_owned()]);
            }
        }
    }
    let root: PathBuf = path.components().take(2).collect();
    let root = if root.as_os_str().is_empty() { path.clone() } else { root };
    walk(&root, vec![MY_COMPUTER.to_string(), root.to_string_lossy().into_owned()])
}

/// Whether `dir` holds a folder that shows by Windows' two switches (a plus box in the tree;
/// `bevel_core::hidden::shown`). Reads only until the first one.
fn has_folders(dir: &Path, switches: bevel_core::hidden::Switches) -> bool {
    use std::os::windows::fs::MetadataExt;
    let Ok(entries) = std::fs::read_dir(dir) else { return false };
    entries.flatten().any(|e| e.metadata().is_ok_and(|m| m.is_dir() && bevel_core::hidden::shown(m.file_attributes(), switches)))
}

/// The folders in `dir` as tree rows, by name (`None`: it could not be read).
fn folder_rows(dir: &Path) -> Option<Vec<NodeSpec>> {
    let switches = shell::explorer_switches();
    let (seen, _) = shell::list_folder_with(dir, switches)?;
    let mut rows: Vec<NodeSpec> = seen
        .into_iter()
        .filter(|s| s.folder)
        .map(|s| {
            let id = s.path.to_string_lossy().into_owned();
            let name = shell::display_name(&id).unwrap_or_else(|| s.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
            NodeSpec { has_rows: has_folders(&s.path, switches), hidden: s.hidden(), id, name, kind: NodeKind::Folder }
        })
        .collect();
    rows.sort_by_key(|r| r.name.to_lowercase());
    Some(rows)
}

/// The rows of tree row `id` in `era` (`None`: it could not be read).
fn tree_rows(era: Era, id: &str) -> Option<Vec<NodeSpec>> {
    let spec = |id: &str, name: &str, kind: NodeKind, has_rows: bool| NodeSpec { id: id.into(), name: name.into(), kind, has_rows, hidden: false };
    let switches = shell::explorer_switches();
    match id {
        DESKTOP => {
            let computer = spec(MY_COMPUTER, "My Computer", NodeKind::MyComputer, true);
            let documents = shell::documents_folder().map(|d| spec("::my-documents", "My Documents", NodeKind::MyDocuments, has_folders(&d, switches)));
            let recycle = spec("::recycle-bin", "Recycle Bin", NodeKind::RecycleBin, false);
            // Each look's order of the desktop's own items (95 had no My Documents).
            let mut rows: Vec<NodeSpec> = match era {
                Era::Win95 => vec![computer, recycle],
                Era::Win98 => [Some(computer), documents, Some(recycle)].into_iter().flatten().collect(),
                Era::Win2000 => [documents, Some(computer), Some(recycle)].into_iter().flatten().collect(),
            };
            let mut folders: Vec<NodeSpec> =
                desktop_folders_with(switches)
                    .into_iter()
                    .map(|(path, name, hidden)| NodeSpec { has_rows: has_folders(Path::new(&path), switches), id: path, name, kind: NodeKind::Folder, hidden })
                    .collect();
            folders.sort_by_key(|r| r.name.to_lowercase());
            rows.extend(folders);
            Some(rows)
        }
        MY_COMPUTER => Some(
            read_my_computer(era, true)
                .items
                .into_iter()
                .map(|i| match i.picture {
                    Picture::Art(Art::Drive(k)) => {
                        // A drive that takes disks always has its plus (Windows asked it only when opened).
                        let rows = matches!(k, DriveKind::Floppy | DriveKind::Floppy525 | DriveKind::Removable | DriveKind::CdRom) || has_folders(Path::new(&i.id), switches);
                        NodeSpec { id: i.id, name: i.name, kind: NodeKind::Drive(k), has_rows: rows, hidden: false }
                    }
                    Picture::Art(Art::Printers) => NodeSpec { id: i.id, name: i.name, kind: NodeKind::Printers, has_rows: false, hidden: false },
                    _ => NodeSpec { id: i.id, name: i.name, kind: NodeKind::ControlPanel, has_rows: false, hidden: false },
                })
                .collect(),
        ),
        "::my-documents" => folder_rows(&shell::documents_folder()?),
        _ if id.starts_with("::") => Some(Vec::new()),
        _ => folder_rows(Path::new(id)),
    }
}

/// Why a tree's rows are read: the person opened them (a click, a key, the window's start), or a
/// drag resting on a row opened it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OpenedBy {
    Person,
    Drag,
}

/// Rows read for a window's tree: (the window, the row, its rows or `None`, whether the row is a
/// drive that could not be read because it is locked by BitLocker, and why it was read).
type TreeRows = (usize, String, Option<Vec<NodeSpec>>, bool, OpenedBy);
static TREE_ROWS: Mutex<Vec<TreeRows>> = Mutex::new(Vec::new());

/// What a tree row that could not be read leads to, once it is marked as not readable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UnreadableRow {
    /// Nothing more (not a drive; or any row a drag opened: neither Windows' unlock window nor
    /// the look's message box comes up in the middle of a drag, the review before 0.1.3, B6).
    Nothing,
    /// The look's "not accessible" message for the drive (Retry opens it again).
    DriveMessage,
    /// Windows' own unlock window for the drive locked by BitLocker.
    Unlock,
}

fn after_unreadable_row(id: &str, locked: bool, by: OpenedBy) -> UnreadableRow {
    match (root_letter(id).is_some() && locked, by) {
        (_, OpenedBy::Drag) => UnreadableRow::Nothing,
        (true, OpenedBy::Person) => UnreadableRow::Unlock,
        (false, OpenedBy::Person) if id.len() == 3 && id.ends_with(":\\") => UnreadableRow::DriveMessage,
        _ => UnreadableRow::Nothing,
    }
}

/// Reads the rows of `ids` for window `hwnd`'s tree on a thread of its own (a slow drive never
/// holds the window up), then tells the program (`WM_TREE_ROWS`).
fn read_tree_rows(hwnd: HWND, era: Era, ids: Vec<String>, by: OpenedBy) {
    if ids.is_empty() {
        return;
    }
    let (w, hub) = (hwnd as usize, HUB_FOR_THREADS.load(Ordering::SeqCst));
    let spawned = std::thread::Builder::new().name("tree reader".into()).spawn(move || {
        bevel_winkit::com::init();
        for id in ids {
            let rows = tree_rows(era, &id);
            // A drive whose rows were read is not locked (any more): its menu loses Unlock
            // Drive... and Open opens it (the review before 0.1.3, B2).
            if rows.is_some()
                && let Some(l) = root_letter(&id)
            {
                note_lock(l, false);
            }
            // A drive that could not be read: is it locked? (asked here, off the window's thread)
            let locked = rows.is_none()
                && root_letter(&id).is_some_and(|l| {
                    let locked = bevel_winkit::bitlocker::drive_locked(l);
                    note_lock(l, locked);
                    locked
                });
            TREE_ROWS.lock().map(|mut r| r.push((w, id, rows, locked, by))).ok();
            // SAFETY: posting to the program's own window.
            unsafe { PostMessageW(hub as HWND, WM_TREE_ROWS.id(), 0, 0) };
        }
    });
    if let Err(e) = spawned {
        log(&format!("folders: could not start reading the tree: {e}"));
    }
}

/// Rows a tree reader brought: into their window's tree (a folder that could not be read closes,
/// as Windows' did after its message).
fn tree_rows_ready() {
    let all = TREE_ROWS.lock().map(|mut r| std::mem::take(&mut *r)).unwrap_or_default();
    let mut unreadable_drive: Vec<(HWND, char, Option<String>)> = Vec::new();
    let mut locked_drive: Vec<char> = Vec::new();
    for (w, id, rows, locked, by) in all {
        let hwnd = w as HWND;
        if !kit::windows::<FolderHost>().contains(&hwnd) {
            continue;
        }
        kit::scope::<FolderHost, _>(hwnd, || {
            kit::update::<FolderHost>(|h| {
                match rows {
                    Some(rows) => h.form.tree.set_rows(&id, rows),
                    None => {
                        log("folders: a folder of the tree could not be read");
                        h.form.tree.unreadable(&id);
                        if by == OpenedBy::Drag {
                            drag_found_unreadable(hwnd, &id);
                        }
                        // A drive opened in the tree: the look's message (Retry opens it again);
                        // one locked by BitLocker: Windows' own unlock window instead, but only
                        // for a row the person opened, never for one a drag opened.
                        match after_unreadable_row(&id, locked, by) {
                            UnreadableRow::Unlock => locked_drive.extend(root_letter(&id)),
                            UnreadableRow::DriveMessage => unreadable_drive.push((hwnd, id.chars().next().unwrap_or('A'), Some(id.clone()))),
                            UnreadableRow::Nothing => {}
                        }
                    }
                }
                h.form.tree_selection_shown();
            });
        });
    }
    for (hwnd, letter, node) in unreadable_drive {
        ask_drive(hwnd, letter, node);
    }
    for letter in locked_drive {
        log(&format!("folders: {letter}: is locked by BitLocker; Windows' unlock window instead"));
        bevel_winkit::bitlocker::start_unlock(letter);
    }
}

// ---------------------------------------------------------------------------------------------
// Windows 2000's Folder Options window.

/// Tools > Folder Options... (Windows 2000), owned by the folder window it was opened from.
struct FolderOptionsHost {
    form: FolderOptions,
}

impl Hosted for FolderOptionsHost {
    type F = FolderOptions;
    fn form(&self) -> &FolderOptions {
        &self.form
    }
    fn form_mut(&mut self) -> &mut FolderOptions {
        &mut self.form
    }
    fn activate(part: OptionsPart) {
        match part {
            OptionsPart::Restore => {
                kit::update::<FolderOptionsHost>(|h| bevel_core::folder::options::restore(&mut h.form));
            }
            OptionsPart::Apply | OptionsPart::Ok => {
                let choice = kit::with::<FolderOptionsHost, _>(|h| h.form.choice);
                if let Some(choice) = choice {
                    apply_folder_options(choice);
                    kit::update::<FolderOptionsHost>(|h| h.form.applied_now());
                }
                if part == OptionsPart::Ok {
                    kit::close::<FolderOptionsHost>();
                }
            }
            OptionsPart::Cancel | OptionsPart::Close => kit::close::<FolderOptionsHost>(),
            _ => {}
        }
    }
}

/// Opens Folder Options over the folder window `owner` (one at a time; an open one comes forward).
fn folder_options(owner: HWND, by_keyboard: bool) {
    if let Some(open) = kit::windows::<FolderOptionsHost>().first().copied() {
        // SAFETY: our own window.
        unsafe { SetForegroundWindow(open) };
        return;
    }
    let Some((_, look)) = the_look() else { return };
    let now = SAVED.with(|s| {
        let s = s.borrow();
        Choice { web: !s.classic_folders, own_window: s.own_window }
    });
    let form = FolderOptions::new(now, by_keyboard);
    let mut r = RECT::default();
    // SAFETY: our own window and a valid out-pointer.
    unsafe { GetWindowRect(owner, &mut r) };
    let scale = look.scale;
    let opened = kit::open_as(Kind::Owned { owner }, FolderOptionsHost { form }, look, |w, h| on_screen((r.left + scale.at(12), r.top + scale.at(12)), w, h));
    if opened.is_none() {
        log("folders: the Folder Options window could not open");
    }
}

/// OK or Apply in Folder Options: kept in folders.txt, and every open window follows at once (2000
/// switched its web view in open windows too).
fn apply_folder_options(choice: Choice) {
    SAVED.with(|s| {
        let mut s = s.borrow_mut();
        s.classic_folders = !choice.web;
        s.own_window = choice.own_window;
    });
    save();
    for hwnd in kit::windows::<FolderHost>() {
        kit::scope::<FolderHost, _>(hwnd, || {
            kit::update::<FolderHost>(|h| {
                if h.form.era == Era::Win2000 {
                    h.form.set_web(choice.web && h.form.web.fonts.is_some());
                }
            });
        });
    }
}

// ---------------------------------------------------------------------------------------------
// Dragging files (decisions.md, "Dragging files"): out of a window with Windows' drag and drop
// (`dragdrop::run`), into one through its drop target (`folder_drop`).

/// A drag that started in a folder window: the window, the icons (by id: a refresh during the
/// drag reorders the items, the third safety review of 2026-11-18, suspected 3) and where the
/// press was.
struct OwnOut {
    hwnd: HWND,
    items: Vec<String>,
    from: (i32, i32),
}

/// A drag over a window now: the tree's row it rests on (by id) and since when (2000 opens a
/// closed row after about a second; 98 at a move after a short rest), the pointer, the last edge
/// scroll.
struct Hover {
    row: Option<String>,
    since: std::time::Instant,
    at: (i32, i32),
    scrolled: std::time::Instant,
    /// The rows this drag opened that could not be read: it does not open them again (no
    /// reader, no reading of the drive, no log line at every movement; the review before 0.1.3,
    /// B6). Forgotten with the drag.
    unreadable: Vec<String>,
}

/// Whether a drag resting on a closed row of the tree opens it now: 2000 after `ROW_OPENS_2000`
/// without moving, 95 and 98 at a move after `ROW_OPENS_98`; never a row this drag found
/// unreadable.
fn drop_row_opens(era: Era, moved: bool, rested: Duration, unreadable: bool) -> bool {
    !unreadable
        && match era {
            Era::Win2000 => rested >= ROW_OPENS_2000,
            _ => moved && rested >= ROW_OPENS_98,
        }
}

/// A row a drag opened on window `hwnd` could not be read: that drag leaves it closed.
fn drag_found_unreadable(hwnd: HWND, id: &str) {
    HOVER.with(|hv| {
        if let Some(e) = hv.borrow_mut().get_mut(&(hwnd as isize))
            && !e.unreadable.iter().any(|u| u == id)
        {
            e.unreadable.push(id.to_string());
        }
    });
}

thread_local! {
    static OUT: RefCell<Option<OwnOut>> = const { RefCell::new(None) };
    static HOVER: RefCell<HashMap<isize, Hover>> = RefCell::new(HashMap::new());
}

/// A closed row of the tree opens under a resting drag (2000: about 1 s without moving, the
/// Reference PC 0.7-1.7 s; 98 at the first move after a short rest; 95 not measured: 98's).
const ROW_OPENS_2000: Duration = Duration::from_millis(1000);
const ROW_OPENS_98: Duration = Duration::from_millis(400);
/// How often a drag resting at an edge scrolls a row (Bevel95's, PROVISIONAL).
const EDGE_SCROLL: Duration = Duration::from_millis(150);

/// A drag of icons has begun in the current window: Windows' drag and drop carries it.
fn drag_out(hwnd: HWND) {
    let Some((era, look)) = the_look() else { return };
    let taken = kit::with::<FolderHost, _>(|h| {
        let (items, from, right) = h.form.view.take_drag()?;
        let mut paths = Vec::new();
        let mut others = Vec::new();
        for &i in &items {
            let item = &h.form.view.items[i];
            // Only files and folders leave the window (a drive or a place never goes anywhere by a
            // drag here: Windows made shortcuts of them, which is not built).
            match ItemKind::of(item) {
                ItemKind::Folder | ItemKind::File => paths.push(item.id.clone()),
                _ => others.push(item.id.clone()),
            }
        }
        let picture = h.form.view.drag_picture(look.theme, &look.regular, &items, bevel_core::desktop_icons::WINDOW).map(|(p, x, y)| (p, x - from.0, y - from.1));
        Some((h.form.view.ids_of(&items), from, right, paths, others, picture))
    })
    .flatten();
    let Some((items, from, right, paths, others, picture)) = taken else { return };
    kit::update::<FolderHost>(|_| {});
    OUT.with(|o| *o.borrow_mut() = Some(OwnOut { hwnd, items, from }));
    log(&format!("folders: a drag of {} item(s){} began", paths.len() + others.len(), if right { " with the right button" } else { "" }));
    let done = bevel_winkit::dragdrop::run(bevel_winkit::dragdrop::Out { hwnd, paths, others, right, era, picture, scale: look.scale });
    OUT.with(|o| o.borrow_mut().take());
    log(&format!("folders: the drag ended ({})", match done {
        Some(e) => format!("{e:?}"),
        None => String::from("no file operation reported to the source"),
    }));
}

/// The folder a tree row or an icon stands for, as a drop place.
fn dest_of_item(item: &Item) -> Dest {
    match ItemKind::of(item) {
        ItemKind::Folder | ItemKind::Drive | ItemKind::LockedDrive => Dest::Folder(PathBuf::from(&item.id)),
        ItemKind::System(bevel_core::desktop_icons::System::RecycleBin) => Dest::RecycleBin,
        ItemKind::System(bevel_core::desktop_icons::System::MyDocuments) => shell::documents_folder().map_or(Dest::Refuse, Dest::Folder),
        _ => Dest::Refuse,
    }
}

fn dest_of_node(kind: NodeKind, id: &str) -> Dest {
    match kind {
        NodeKind::Folder | NodeKind::Drive(_) => Dest::Folder(PathBuf::from(id)),
        NodeKind::MyDocuments => shell::documents_folder().map_or(Dest::Refuse, Dest::Folder),
        NodeKind::Desktop => shell::desktop_folder().map_or(Dest::Refuse, Dest::Folder),
        NodeKind::RecycleBin => Dest::RecycleBin,
        _ => Dest::Refuse,
    }
}

/// A drag over a folder window (its drop target, `dragdrop::register`).
fn folder_drop(hwnd: HWND, phase: bevel_winkit::dragdrop::Phase, over: &bevel_winkit::dragdrop::Over) -> Option<bevel_core::drag::Effect> {
    use bevel_core::drag::Effect;
    use bevel_core::folder::DropAt;
    use bevel_winkit::dragdrop::Phase;
    if !kit::windows::<FolderHost>().contains(&hwnd) {
        return None;
    }
    kit::scope::<FolderHost, _>(hwnd, || {
        let scale = kit::look_of::<FolderHost>().map_or(bevel_core::scale::Scale::ONE, |l| l.scale);
        let (x, y) = (scale.old(over.x), scale.old(over.y));
        if phase == Phase::Leave {
            HOVER.with(|h| h.borrow_mut().remove(&(hwnd as isize)));
            kit::update::<FolderHost>(|h| {
                h.form.show_drop(DropAt::Nowhere);
            });
            return None;
        }
        // A drag from this very window: the icons it carries.
        let own = OUT.with(|o| o.borrow().as_ref().filter(|o| o.hwnd == hwnd).map(|o| (o.items.clone(), o.from)));
        let mut dragged = over.files.clone();
        if let Some(own) = &over.own {
            dragged.extend(own.others.iter().cloned());
        }
        let (at, era, here, small_free) = kit::with::<FolderHost, _>(|h| {
            let at = h.form.drop_at(x, y, &dragged);
            let here = match &h.shows {
                Shows::Folder(p) => Dest::Folder(p.clone()),
                Shows::Desktop => shell::desktop_folder().map_or(Dest::Refuse, Dest::Folder),
                Shows::MyComputer => Dest::Refuse,
            };
            (at, h.form.era, here, h.form.view.mode().arranged_freely())
        })?;
        let dest = kit::with::<FolderHost, _>(|h| match at {
            DropAt::Item(i) => dest_of_item(&h.form.view.items[i]),
            DropAt::Row(i) => dest_of_node(h.form.tree.nodes[i].kind, &h.form.tree.nodes[i].id),
            DropAt::Here => here.clone(),
            DropAt::Nowhere => Dest::Refuse,
        })?;
        // Dropped in its own window, in its own folder: the icons only move (Large and Small
        // Icons, as on the Reference PCs); in List and Details nothing happens.
        let reposition = own.is_some()
            && at == DropAt::Here
            && (over.files.is_empty() || matches!(&dest, Dest::Folder(d) if bevel_core::drag::all_in(&over.files, &d.to_string_lossy())));
        // What is done is what the pointer shows: the look's wish within what the source offers,
        // decided here once, before anything is done.
        let effect = if reposition {
            bevel_core::drag::narrow(small_free.then_some(Effect::Move), over.offers(), false)
        } else {
            bevel_winkit::dragdrop::drop_effect(era, &dest, over)
        };
        if phase != Phase::Drop {
            // The place drawn selected; the tree opens a row and the views scroll under a resting drag.
            let now = std::time::Instant::now();
            let row = match at {
                DropAt::Row(i) => kit::with::<FolderHost, _>(|h| h.form.tree.nodes.get(i).map(|n| (i, n.id.clone()))).flatten(),
                _ => None,
            };
            let (open, scroll) = HOVER.with(|hv| {
                let mut hv = hv.borrow_mut();
                let id = row.as_ref().map(|(_, id)| id.clone());
                let e = hv.entry(hwnd as isize).or_insert(Hover { row: id.clone(), since: now, at: (x, y), scrolled: now, unreadable: Vec::new() });
                let moved = e.at != (x, y);
                if e.row != id {
                    e.row = id;
                    e.since = now;
                }
                e.at = (x, y);
                let rested = now.duration_since(e.since);
                let open = row
                    .as_ref()
                    .filter(|(_, id)| drop_row_opens(era, moved, rested, e.unreadable.iter().any(|u| u == id)))
                    .map(|&(i, _)| i);
                let scroll = now.duration_since(e.scrolled) >= EDGE_SCROLL;
                if scroll {
                    e.scrolled = now;
                }
                (open, scroll)
            });
            let mut wanted = None;
            kit::update::<FolderHost>(|h| {
                h.form.show_drop(at);
                if let Some(i) = open
                    && h.form.open_drop_row(i)
                {
                    wanted = Some(h.form.tree.wanted());
                }
                if scroll {
                    h.form.drop_scroll(x, y);
                }
            });
            if let Some(wanted) = wanted {
                read_tree_rows(hwnd, era, wanted, OpenedBy::Drag);
            }
            return effect;
        }
        // The drop.
        HOVER.with(|h| h.borrow_mut().remove(&(hwnd as isize)));
        kit::update::<FolderHost>(|h| {
            h.form.show_drop(DropAt::Nowhere);
        });
        let mut effect = effect?;
        if over.right {
            let offered = bevel_winkit::dragdrop::menu_effects(&dest, reposition, over);
            effect = right_drop_menu(hwnd, era, effect, &offered)?;
        }
        if reposition {
            if let Some((items, from)) = own {
                let mut changed = false;
                kit::update::<FolderHost>(|h| {
                    h.form.view.move_ids(&items, (x - from.0, y - from.1));
                    changed = remember_places(h);
                });
                if changed {
                    save();
                }
            }
            return Some(Effect::Move);
        }
        let files = over.files.clone();
        match dest {
            Dest::RecycleBin if era == Era::Win98 => {
                if let Some(place) = confirm_place(hwnd) {
                    confirm::ask_recycle(place, files);
                }
            }
            Dest::RecycleBin => file_ops::drop_in_recycle_bin(files),
            Dest::Folder(dir) => {
                // Into the window's own folder: each new icon where the pointer let go (the
                // Reference PCs), the next ones beside it.
                if at == DropAt::Here && small_free {
                    kit::with::<FolderHost, _>(|h| {
                        for (k, f) in files.iter().enumerate() {
                            if let Some(name) = Path::new(f).file_name() {
                                let id = dir.join(name).to_string_lossy().into_owned();
                                h.form.view.place_new(&id, x - 16 + 75 * k as i32, y - 16);
                            }
                        }
                    });
                }
                file_ops::drop_files(files, dir, effect, |_| {});
            }
            Dest::Refuse => return None,
        }
        Some(effect)
    })
}

/// The right-button drag's menu at the pointer (`bevel_core::drag::drop_menu`): what is chosen,
/// or `None` for Cancel, Esc or a click elsewhere.
fn right_drop_menu(hwnd: HWND, era: Era, default: bevel_core::drag::Effect, offered: &[bevel_core::drag::Effect]) -> Option<bevel_core::drag::Effect> {
    let (_, look) = the_look()?;
    let (menu, does) = bevel_core::drag::drop_menu(era, default, offered);
    let mut p = windows_sys::Win32::Foundation::POINT::default();
    // SAFETY: a valid out-pointer.
    unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut p) };
    let ended = menu::track(&look, Request { owner: hwnd, menu, style: MenuStyle::Popup, at: (p.x, p.y), above: None, titles: &[] });
    match ended.outcome {
        Outcome::Chosen(i) => does.get(i).copied().flatten(),
        _ => None,
    }
}

/// The look's message for a drive that cannot be read (`bevel_core::disk_box`), owned by the
/// window: Retry reads it again (the tree's row, or the place shown), Cancel goes back.
struct DiskBoxHost {
    form: DiskBox,
    owner: HWND,
    node: Option<String>,
}

thread_local! {
    static DISK_ANSWER: RefCell<Option<(HWND, Option<String>, bool)>> = const { RefCell::new(None) };
}

impl Hosted for DiskBoxHost {
    type F = DiskBox;
    fn form(&self) -> &DiskBox {
        &self.form
    }
    fn form_mut(&mut self) -> &mut DiskBox {
        &mut self.form
    }
    fn activate(part: DiskPart) {
        if part == DiskPart::Ok {
            // The address bar's message: the typed address stays, selected, with the keyboard.
            let owner = kit::with::<DiskBoxHost, _>(|h| h.owner);
            kit::close::<DiskBoxHost>();
            if let Some(owner) = owner.filter(|o| kit::windows::<FolderHost>().contains(o)) {
                kit::scope::<FolderHost, _>(owner, || {
                    kit::update::<FolderHost>(|h| {
                        h.form.field.focus();
                        h.form.field.key(address::EditKey::SelectAll, false);
                    })
                });
            }
            return;
        }
        let retry = part == DiskPart::Retry;
        if let Some((owner, node)) = kit::with::<DiskBoxHost, _>(|h| (h.owner, h.node.clone())) {
            DISK_ANSWER.with(|a| *a.borrow_mut() = Some((owner, node, retry)));
        }
        kit::close::<DiskBoxHost>();
        // Carried out once the box is gone (from the program's own message).
        let hub = HUB.with(Cell::get);
        // SAFETY: posting to our own window.
        unsafe { PostMessageW(hub, WM_ITEMS.id(), 0, 3) };
    }
}

/// A window went to a drive that its reading found locked by BitLocker (the lock was not known
/// before the window went there): Windows' own unlock window for it, and the window returns to
/// where it came from by undoing the step that brought it there (a new place, Back or Forward:
/// `History::undo`, the review before 0.1.3, B3); it closes only if it has no other place to show
/// at all.
fn locked_found(owner: HWND, letter: char) {
    log(&format!("folders: {letter}: is locked by BitLocker; Windows' unlock window instead"));
    bevel_winkit::bitlocker::start_unlock(letter);
    if !kit::windows::<FolderHost>().contains(&owner) {
        return;
    }
    kit::scope::<FolderHost, _>(owner, || {
        let still_there = kit::with::<FolderHost, _>(|h| drive_letter_of(&h.shows) == Some(letter.to_ascii_uppercase())).unwrap_or(false);
        if !still_there {
            return;
        }
        // Whether any place is left once the step is undone (a copy of the history: `navigate`
        // undoes it on the real one).
        let elsewhere = kit::with::<FolderHost, _>(|h| h.history.clone().undo(h.came).is_some()).unwrap_or(false);
        if elsewhere {
            navigate(owner, None, Go::Undo);
        } else {
            kit::close::<FolderHost>();
        }
    });
}

fn ask_drive(owner: HWND, letter: char, node: Option<String>) {
    if !kit::windows::<DiskBoxHost>().is_empty() || !kit::windows::<FolderHost>().contains(&owner) {
        return;
    }
    let Some((era, look)) = the_look() else { return };
    let title = kit::scope::<FolderHost, _>(owner, || kit::with::<FolderHost, _>(|h| h.form.title.clone())).unwrap_or_default();
    let kind = read_my_computer(era, false).items.iter().find_map(|i| match i.picture {
        Picture::Art(Art::Drive(k)) if i.id.starts_with(letter) => Some(k),
        _ => None,
    });
    let picture = art::drive(era, kind.unwrap_or(DriveKind::Floppy));
    let form = DiskBox::new(era, &title, letter, picture, true, |l| look.regular.measure(l));
    let screen = work_area(Some(owner));
    let scale = look.scale;
    let opened = kit::open_as(Kind::Owned { owner }, DiskBoxHost { form, owner, node }, look, |w, h| {
        let (x, y) = bevel_core::rename_warning::place((rect_w(&screen), rect_h(&screen)), (w, h));
        (screen.left + x + (scale.len(7) - 7) / 2, screen.top + y + (scale.len(17) - 17) / 2)
    });
    if opened.is_none() {
        log("folders: the message for a drive without a disk could not open");
    }
}

thread_local! {
    /// Windows whose slow second click is waiting, and since when.
    static SLOW_CLICKS: RefCell<HashMap<isize, std::time::Instant>> = RefCell::new(HashMap::new());
}

/// A slow second click on a selected name: once Windows' double-click time has gone by without a
/// double-click or a drag, the name becomes a box to type in (as F2 does).
fn slow_renames() {
    // SAFETY: a plain query.
    let wait = std::time::Duration::from_millis(u64::from(unsafe { windows_sys::Win32::UI::Input::KeyboardAndMouse::GetDoubleClickTime() }));
    for hwnd in kit::windows::<FolderHost>() {
        let waiting = kit::scope::<FolderHost, _>(hwnd, || kit::with::<FolderHost, _>(|h| h.form.slow_rename_waiting())).unwrap_or(false);
        let key = hwnd as isize;
        if !waiting {
            SLOW_CLICKS.with(|s| s.borrow_mut().remove(&key));
            continue;
        }
        let since = SLOW_CLICKS.with(|s| *s.borrow_mut().entry(key).or_insert_with(std::time::Instant::now));
        // The tick comes every 100 ms: the click was up to one tick before it was first seen.
        if since.elapsed() + std::time::Duration::from_millis(50) < wait {
            continue;
        }
        SLOW_CLICKS.with(|s| s.borrow_mut().remove(&key));
        let due = kit::scope::<FolderHost, _>(hwnd, || kit::with::<FolderHost, _>(|h| h.form.slow_rename_due())).flatten();
        if due.is_some() {
            static SLOW: AtomicU32 = AtomicU32::new(0);
            counted(&SLOW, "folders: a slow second click opened the rename box");
            kit::scope::<FolderHost, _>(hwnd, || run(hwnd, Command::Shared(D::Rename), false));
        }
    }
}

thread_local! {
    /// The window last found drawn inactive while in front (logged once per time it happens).
    static INACTIVE_FRONT: Cell<isize> = const { Cell::new(0) };
}

/// A window once stayed drawn inactive although it was in front, until another window took the
/// front and gave it back (the real-mouse pass of 2026-11-15, seen once). Looked at every 100
/// ms: if it happens, the log says so (with what the window last heard) and it is drawn active.
fn check_front() {
    // SAFETY: a plain query.
    let front = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow() };
    if front.is_null() || !kit::windows::<FolderHost>().contains(&front) {
        INACTIVE_FRONT.with(|c| c.set(0));
        return;
    }
    let inactive = kit::scope::<FolderHost, _>(front, || kit::with::<FolderHost, _>(|h| !h.form.is_active())).unwrap_or(false);
    if !inactive {
        INACTIVE_FRONT.with(|c| c.set(0));
        return;
    }
    if INACTIVE_FRONT.with(|c| c.replace(front as isize)) != front as isize {
        log("folders: a window in front was drawn inactive (it never heard it was activated); drawn active now");
        kit::scope::<FolderHost, _>(front, || kit::update::<FolderHost>(|h| h.form.set_active(true)));
    }
}

/// What was typed into a window's address bar and could not be opened (from the thread that
/// tried), shown as the look's message on the program's thread.
static ADDRESS_FAILED: std::sync::Mutex<Vec<(isize, String)>> = std::sync::Mutex::new(Vec::new());

/// The look's message for an address that is no folder and cannot be opened (98 and 2000
/// showed it; the typed text stays in the field, selected, once it is answered).
fn address_failed() {
    let failed = ADDRESS_FAILED.lock().map(|mut f| std::mem::take(&mut *f)).unwrap_or_default();
    for (owner, typed) in failed {
        let owner = owner as HWND;
        if !kit::windows::<DiskBoxHost>().is_empty() || !kit::windows::<FolderHost>().contains(&owner) {
            continue;
        }
        let Some((era, look)) = the_look() else { return };
        let scale = look.scale;
        let screen = work_area(Some(owner));
        let form = DiskBox::address(era, &typed, scale.old_fit(rect_w(&screen)), &look.regular);
        let opened = kit::open_as(Kind::Owned { owner }, DiskBoxHost { form, owner, node: None }, look, |w, h| {
            let (x, y) = bevel_core::rename_warning::place((rect_w(&screen), rect_h(&screen)), (w, h));
            (screen.left + x + (scale.len(7) - 7) / 2, screen.top + y + (scale.len(17) - 17) / 2)
        });
        if opened.is_none() {
            log("folders: the message for an address that cannot be opened could not open");
        }
    }
}

/// The drive box's answer: Retry reads again, Cancel goes back to where the window was.
fn disk_answered() {
    let Some((owner, node, retry)) = DISK_ANSWER.with(|a| a.borrow_mut().take()) else { return };
    if !kit::windows::<FolderHost>().contains(&owner) {
        return;
    }
    kit::scope::<FolderHost, _>(owner, || match (retry, node) {
        (true, Some(id)) => kit::update::<FolderHost>(|h| {
            if let Some(i) = h.form.tree.find(&id) {
                let page = h.form.tree_page();
                h.form.tree.open(i, page);
            }
        }),
        (true, None) => run(owner, Command::Shared(D::Refresh), false),
        (false, None) => {
            if kit::with::<FolderHost, _>(|h| h.history.can_back()).unwrap_or(false) {
                navigate(owner, None, Go::Back(1));
            }
        }
        (false, Some(_)) => {}
    });
}

/// The tree's timer: rows the trees need are read, a selection moved by keys is followed once it
/// rests, folders changed by a command are read again.
fn tree_tick() {
    let Some((era, _)) = the_look() else { return };
    let now = std::time::Instant::now();
    for hwnd in kit::windows::<FolderHost>() {
        let (wanted, settled) = kit::scope::<FolderHost, _>(hwnd, || {
            kit::with::<FolderHost, _>(|h| {
                if !h.form.tree_shown {
                    return (Vec::new(), false);
                }
                let mut wanted = h.form.tree.wanted();
                let due: Vec<String> = h.reread.iter().filter(|(at, _)| *at <= now).map(|(_, id)| id.clone()).collect();
                h.reread.retain(|(at, _)| *at > now);
                wanted.extend(due);
                let settled = h.settle.is_some_and(|at| at <= now);
                if settled {
                    h.settle = None;
                }
                (wanted, settled)
            })
            .unwrap_or_default()
        });
        read_tree_rows(hwnd, era, wanted, OpenedBy::Person);
        if settled {
            kit::scope::<FolderHost, _>(hwnd, || tree_go(hwnd, false));
        }
    }
}

thread_local! {
    /// The window is going to a place chosen in its tree (the tree is not opened down again).
    static FROM_TREE: Cell<bool> = const { Cell::new(false) };
}

/// The current window's icons show the place selected in its tree. Control Panel, Printers and
/// the Recycle Bin open Windows' own (only when chosen by the mouse or Enter, `explicit`), and
/// the tree goes back to the place shown.
fn tree_go(hwnd: HWND, explicit: bool) {
    let Some(node) = kit::with::<FolderHost, _>(|h| h.form.tree.selected().cloned()).flatten() else { return };
    match node_shows(&node.id) {
        Some(shows) => {
            FROM_TREE.with(|f| f.set(true));
            navigate(hwnd, Some(shows), Go::New);
            FROM_TREE.with(|f| f.set(false));
        }
        None => {
            if explicit && let Some(t) = node_target(&node) {
                open_target(t);
            }
            kit::update::<FolderHost>(|h| {
                let chain = tree_chain(h.form.era, &h.shows, false, &h.facts);
                let page = h.form.tree_page();
                h.form.tree.follow(chain, page);
                h.form.tree_selection_shown();
            });
        }
    }
}

/// A tree row as the file commands' target.
fn node_target(n: &pane::Node) -> Option<Target> {
    use bevel_core::desktop_icons::System;
    let kind = match n.kind {
        NodeKind::Desktop => return None,
        NodeKind::Folder => ItemKind::Folder,
        NodeKind::Drive(_) if node_locked(n) => ItemKind::LockedDrive,
        NodeKind::Drive(_) => ItemKind::Drive,
        NodeKind::MyDocuments => ItemKind::System(System::MyDocuments),
        NodeKind::MyComputer => ItemKind::System(System::MyComputer),
        NodeKind::RecycleBin => ItemKind::System(System::RecycleBin),
        NodeKind::ControlPanel | NodeKind::Printers => ItemKind::Place,
    };
    let id = match n.kind {
        NodeKind::ControlPanel => file_ops::CONTROL_PANEL.to_string(),
        NodeKind::Printers => file_ops::PRINTERS.to_string(),
        _ => n.id.clone(),
    };
    Some(Target { id, name: n.name.clone(), kind })
}

/// The folder a tree row stands for (a folder, a drive, My Documents' folder).
fn node_folder(n: &pane::Node) -> Option<PathBuf> {
    match n.kind {
        NodeKind::Folder | NodeKind::Drive(_) => Some(PathBuf::from(&n.id)),
        NodeKind::MyDocuments => shell::documents_folder(),
        _ => None,
    }
}

/// Carries out a command on a tree row (its menu's, or a key's on the selected row).
fn tree_command(hwnd: HWND, c: D, by_keyboard: bool) {
    // The menu's row by its id (rows read while the menu was open moved it); gone meanwhile: no
    // command at all (the third safety review of 2026-11-18, A2). A key: the selected row.
    let found = kit::with::<FolderHost, _>(|h| {
        let node = h.form.menu_node().cloned();
        let by_menu = h.form.menu_row.take().is_some();
        (node, by_menu)
    });
    let Some((found, by_menu)) = found else { return };
    let Some(node) = found else {
        if by_menu {
            log("folders: the tree row the menu was opened for is gone; its command is not carried out");
        }
        return;
    };
    let Some(target) = node_target(&node) else { return };
    let folder = node_folder(&node);
    let parent = kit::with::<FolderHost, _>(|h| h.form.tree.find(&node.id).and_then(|i| h.form.tree.parent(i)).map(|p| h.form.tree.nodes[p].id.clone())).flatten();
    let reread_soon = |id: Option<String>| {
        if let Some(id) = id {
            let now = std::time::Instant::now();
            kit::with::<FolderHost, _>(|h| {
                h.reread.push((now + Duration::from_millis(1500), id.clone()));
                h.reread.push((now + Duration::from_millis(4000), id));
            });
        }
    };
    match c {
        // A drive shown locked: Windows' unlock window at once (`unlock_shown_locked`).
        D::Explore | D::Open if unlock_shown_locked(&target) => {}
        D::Explore => match node_shows(&node.id) {
            Some(s) => open_explorer(s),
            None => open_target(target),
        },
        D::Open => match node_shows(&node.id) {
            Some(s) => open(s),
            None => open_target(target),
        },
        D::Cut | D::Copy if node.kind == NodeKind::Folder => {
            if let Some(seq) = file_ops::put_on_clipboard(hwnd, std::slice::from_ref(&target), c == D::Cut) {
                let ids: Vec<String> = if c == D::Cut { vec![target.id.clone()] } else { Vec::new() };
                CUT.with(|cut| *cut.borrow_mut() = (ids.clone(), seq));
                show_cut(ids);
            }
        }
        // Not into a drive shown locked (its menu has no Paste; a key on its row does nothing).
        D::Paste if !node_locked(&node) => {
            if let Some(dir) = folder {
                file_ops::paste(hwnd, dir);
                reread_soon(Some(node.id.clone()));
            }
        }
        D::CreateShortcut => {
            // Next to the folder; a drive's or a place's on the desktop, as Windows offered.
            let into = match node.kind {
                NodeKind::Folder => Path::new(&node.id).parent().map(Path::to_path_buf),
                _ => shell::desktop_folder(),
            };
            if let Some(into) = into {
                file_ops::create_shortcuts(std::slice::from_ref(&target), into, |_| {});
                reread_soon(parent);
            }
        }
        D::Delete if node.kind == NodeKind::Folder => {
            if let Some(place) = confirm_place(hwnd) {
                confirm::ask_delete(place, vec![target], by_keyboard);
                reread_soon(parent);
            }
        }
        D::ItemProperties if !matches!(node.kind, NodeKind::ControlPanel | NodeKind::Printers) => file_ops::properties(&[target]),
        // Windows' own unlock window; it asks for the password itself.
        D::UnlockDrive if node_locked(&node) => {
            if let Some(letter) = root_letter(&node.id) {
                log(&format!("folders: Unlock Drive... on {letter}: (the tree)"));
                bevel_winkit::bitlocker::start_unlock(letter);
            }
        }
        _ => {}
    }
}

/// Whether a tree row is a drive whose last reading found it locked by BitLocker (as `kind_of`
/// decides for a drive's icon).
fn node_locked(n: &pane::Node) -> bool {
    matches!(n.kind, NodeKind::Drive(_)) && root_letter(&n.id).is_some_and(|l| known_locked(l, false))
}

/// The menu on a tree row (`menu_row`), at `menu_at`.
fn tree_menu(hwnd: HWND) {
    let Some(scale) = kit::look_of::<FolderHost>().map(|l| l.scale) else { return };
    let Some((kind, locked, at, era)) =
        kit::with::<FolderHost, _>(|h| h.form.menu_node().map(|n| (n.kind, node_locked(n), h.form.menu_at, h.form.era))).flatten()
    else {
        kit::with::<FolderHost, _>(|h| h.form.menu_row = None);
        return;
    };
    let can_paste = file_ops::clipboard_has_files();
    let entries = pane::row_entries(kind, can_paste, locked);
    if entries.is_empty() {
        kit::with::<FolderHost, _>(|h| h.form.menu_row = None);
        return;
    }
    // (The look is set before any window opens and never taken away, so this cannot fail; if it
    // did, the row is forgotten like every other way out: the fourth review's suspicion 2.)
    let Some((_, look)) = the_look() else {
        kit::with::<FolderHost, _>(|h| h.form.menu_row = None);
        return;
    };
    let mut menu = pane::row_menu(era, kind, can_paste, locked);
    // Tabs (brief 110): a folder's or a drive's row: "Open in New Tab" right under the entries
    // that open it.
    let new_tab_at = (TAB_SETTINGS.with(Cell::get).0 != tabs::Place::Off && matches!(kind, NodeKind::Folder | NodeKind::Drive(_)) && !locked)
        .then(|| menu.entries.iter().position(|e| e.kind == bevel_core::paint::EntryKind::Separator).unwrap_or(menu.entries.len()));
    let node_id = kit::with::<FolderHost, _>(|h| h.form.menu_node().map(|n| n.id.clone())).flatten();
    if let Some(k) = new_tab_at {
        menu.entries.insert(k, bevel_core::paint::MenuEntry::item("Open in New &Tab", None));
    }
    let entries: Vec<_> = match new_tab_at {
        Some(k) => entries[..k].iter().copied().chain(std::iter::once(None)).chain(entries[k..].iter().copied()).collect(),
        None => entries,
    };
    let at = to_screen(hwnd, at, scale);
    // 95 and 98 blanked the status bar while an entry of this menu was lit (no words for any);
    // 2000 left it as it was (`private/probes/measure17`, G).
    let ended = menu::track_with_selection(&look, Request { owner: hwnd, menu, style: MenuStyle::Popup, at, above: None, titles: &[] }, |row| {
        if era != Era::Win2000 && row.is_some() {
            kit::update::<FolderHost>(|h| h.form.help = Some(String::new()));
        }
    });
    kit::update::<FolderHost>(|h| h.form.help = None);
    match ended.outcome {
        Outcome::Chosen(i) if Some(i) == new_tab_at => {
            kit::with::<FolderHost, _>(|h| h.form.menu_row = None);
            if let Some(id) = node_id {
                new_tab(hwnd, Shows::Folder(PathBuf::from(id)));
            }
        }
        Outcome::Chosen(i) => match entries.get(i).copied().flatten() {
            Some(c) => tree_command(hwnd, c, ended.keyboard),
            None => {
                kit::with::<FolderHost, _>(|h| h.form.menu_row = None);
            }
        },
        _ => {
            kit::with::<FolderHost, _>(|h| h.form.menu_row = None);
            if let Some(msg) = ended.pending {
                menu::dispatch(&msg);
            }
        }
    }
}

/// Shows or hides the current window's tree (98's Explorer Bar, 2000's Folders, the bar's close
/// button). Windows 2000 showed the Explorer's picture while the tree showed.
fn set_tree(hwnd: HWND, shown: bool) {
    let Some(icon) = kit::with::<FolderHost, _>(|h| {
        h.form.show_tree(shown);
        if shown {
            let chain = tree_chain(h.form.era, &h.shows, false, &h.facts);
            let page = h.form.tree_page();
            h.form.tree.follow(chain, page);
        }
        if h.form.era == Era::Win2000 {
            h.form.icon = if shown { tree_art::explorer(h.form.era) } else { title_from(h.form.era, &h.shows, &h.facts).1.icon(h.form.era) };
        }
        h.form.icon.clone()
    }) else {
        return;
    };
    kit::set_program_icons::<FolderHost>(&icon, &icon);
    kit::update::<FolderHost>(|_| {});
    let _ = hwnd;
}

trait SmallIcon {
    fn icon(self, era: Era) -> bevel_core::fb::Icon;
}

impl SmallIcon for Small {
    fn icon(self, era: Era) -> bevel_core::fb::Icon {
        art::small(era, self)
    }
}

/// The place Windows Explorer opened in: 95's and 98's the Windows drive, 2000's My Documents
/// (`*-truecolor-tree-start`).
fn explorer_start(era: Era) -> Shows {
    if era == Era::Win2000
        && let Some(d) = shell::documents_folder()
    {
        return Shows::Folder(d);
    }
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    Shows::Folder(PathBuf::from(format!("{}\\", drive.trim_end_matches('\\'))))
}
// ---------------------------------------------------------------------------------------------
// The window's own behaviour.

impl Hosted for FolderHost {
    type F = FolderWindow;

    fn form(&self) -> &FolderWindow {
        &self.form
    }
    fn form_mut(&mut self) -> &mut FolderWindow {
        &mut self.form
    }
    fn activate(part: Part) {
        let Some(hwnd) = kit::hwnd::<FolderHost>() else { return };
        // The menus show what Undo would take back (the list the desktop shares).
        if matches!(part, Part::Menu(_) | Part::BackgroundMenu) {
            let undo = bevel_winkit::undo::last();
            kit::with::<FolderHost, _>(|h| h.form.undo = undo);
        }
        match part {
            // SAFETY (each): our own window.
            Part::Minimize => unsafe {
                ShowWindow(hwnd, SW_MINIMIZE);
            },
            Part::Maximize => kit::toggle_maximize::<FolderHost>(),
            Part::Close => kit::close::<FolderHost>(),
            Part::Icon => menu::track_window::<FolderHost>(Part::Icon, |f| f.era, false, false),
            Part::TitleMenu => menu::track_window::<FolderHost>(Part::Icon, |f| f.era, true, false),
            Part::KeyMenu => menu::track_window::<FolderHost>(Part::Icon, |f| f.era, false, true),
            Part::Menu(first) => menu_bar(hwnd, first),
            // Tabs (brief 110).
            Part::Tab(tab) => switch_tab(hwnd, tab),
            Part::TabClose => {
                if let Some(tab) = kit::with::<FolderHost, _>(|h| h.tab) {
                    close_tab(hwnd, tab);
                }
            }
            Part::TabRow => {}
            Part::TabNew => new_tab(hwnd, Shows::MyComputer),
            Part::TabNext => step_tab(hwnd, true),
            Part::TabPrev => step_tab(hwnd, false),
            Part::TabCloseOf(tab) => close_tab(hwnd, tab),
            Part::TabMenu(tab) => tab_menu(hwnd, tab),
            Part::Open => open_selected(hwnd),
            Part::ItemMenu => item_menu(hwnd),
            Part::BackgroundMenu => background_menu(hwnd),
            Part::Command(c) => run(hwnd, c, true),
            Part::RenameEnd(by_keyboard) => {
                finish_rename(hwnd, by_keyboard);
                // A press outside the box: once the name is taken, it is a click there (selects;
                // a double-click opens), as on the Reference PCs (brief 90).
                if !by_keyboard {
                    // A press elsewhere (the address field, a toolbar button, the tree...) goes on
                    // too once the name is taken without the warning (brief 99).
                    let (went_on, carried) = kit::with::<FolderHost, _>(|h| (h.form.click_after_rename(), h.form.carry_on_after_rename()))
                        .unwrap_or((false, Vec::new()));
                    if went_on || !carried.is_empty() {
                        kit::update::<FolderHost>(|_| {});
                        for effect in carried {
                            if let bevel_core::kit::Effect::Activate(p) = effect {
                                FolderHost::activate(p);
                            }
                        }
                    } else if kit::with::<FolderHost, _>(|h| h.form.view.renaming().is_none()).unwrap_or(false) {
                        // The press did its work as it was made again (the address field took the
                        // keyboard, the tree's selection moved): only a repaint.
                        kit::update::<FolderHost>(|_| {});
                    } else {
                        static STOPPED: AtomicU32 = AtomicU32::new(0);
                        counted(&STOPPED, "folders: a click ended a rename and went no further (a question came up)");
                    }
                }
            }
            Part::List | Part::VThumb | Part::HThumb | Part::Scroll(_) | Part::Address => {}
            Part::Tool(t) => tool(hwnd, t),
            Part::ToolArrow(t) => places_menu(hwnd, t),
            Part::AddressButton => toggle_list(),
            Part::ListRow(i) => list_chosen(hwnd, i),
            Part::Go => go_typed(hwnd),
            Part::TreeChosen => {
                kit::with::<FolderHost, _>(|h| h.settle = None);
                tree_go(hwnd, true);
            }
            Part::TreeMoved => {
                kit::with::<FolderHost, _>(|h| h.settle = Some(std::time::Instant::now() + SETTLE));
            }
            Part::TreeMenu => tree_menu(hwnd),
            Part::TreeClose => set_tree(hwnd, false),
            Part::Split => {
                // The pane's new width is kept for the next Explorer window.
                if let Some(w) = kit::with::<FolderHost, _>(|h| h.form.tree_width) {
                    SAVED.with(|s| s.borrow_mut().tree_width = Some(w));
                    save();
                }
            }
            Part::TreeRow(_) | Part::TreeBox(_) | Part::TreeBlank | Part::TreeVThumb | Part::TreeHThumb | Part::TreeScroll(_) => {}
            // Details: a header click sorts; a column's new width; both remembered.
            Part::Header(k) => {
                kit::update::<FolderHost>(|h| {
                    h.form.header_click(k);
                    remember_view(h);
                });
                save();
            }
            Part::Divider(_) => {
                kit::with::<FolderHost, _>(|h| remember_view(h));
                save();
            }
            Part::WebLink(l) => web_link(hwnd, l),
            Part::WebScroll(_) => {}
            Part::DragOut => drag_out(hwnd),
        }
    }
    /// The place, the size and the arrangement are remembered; the reader stops.
    fn closing(&mut self, hwnd: HWND) {
        bevel_winkit::dragdrop::revoke(hwnd);
        self.stop.store(true, Ordering::SeqCst);
        let mut placement = WINDOWPLACEMENT { length: size_of::<WINDOWPLACEMENT>() as u32, ..Default::default() };
        // SAFETY: our own window; a valid out-pointer.
        let (ok, maximized) = unsafe { (GetWindowPlacement(hwnd, &mut placement) != 0, IsZoomed(hwnd) != 0) };
        if ok {
            let r = placement.rcNormalPosition;
            let window = Window {
                x: r.left,
                y: r.top,
                w: rect_w(&r),
                h: rect_h(&r),
                maximized,
                arrange: self.form.view.arrangement(),
                auto_arrange: self.form.view.auto_arrange(),
            };
            remember_view(self);
            let key = self.shows.key();
            let (exploring, tree) = (self.form.exploring, self.form.tree_shown);
            SAVED.with(|s| {
                let mut s = s.borrow_mut();
                if exploring {
                    s.explorer = Some(window);
                } else {
                    // 98 and 2000: a folder closed with its tree showing opens with it again.
                    if tree {
                        s.tree_folders.insert(key.clone());
                    } else {
                        s.tree_folders.remove(&key);
                    }
                    s.remember(&key, window);
                }

            });
            save();
        }
    }
    fn closed(&mut self) {
        let id = self.id;
        WINDOWS.with(|w| w.borrow_mut().retain(|(i, _, _)| *i != id));
        if let Ok(mut g) = GENERATIONS.lock() && let Some(g) = g.as_mut() {
            g.forget(id);
        }
        if let Ok(mut all) = SNAPSHOTS.lock() {
            all.retain(|(i, ..)| *i != id);
        }
        let left = WINDOWS.with(|w| w.borrow().len());
        log(&format!("folders: closed a window ({left} open)"));
    }
    fn resized(&mut self, size: (i32, i32), maximized: bool) {
        self.form.resize(size, maximized);
    }
    fn minimum_size(&self) -> (i32, i32) {
        (112, 100)
    }
    /// The bar's look or size changed: the windows follow it (another look's frame, menus,
    /// toolbar and pictures; Windows 95's windows have no history, so Back starts afresh there).
    fn look_changed() {
        let (era, look) = look::load();
        ERA_LOOK.with(|l| *l.borrow_mut() = Some((era, look.clone())));
        TAB_SETTINGS.with(|t| t.set(look::folder_tabs()));
        // A record of the new look that its window could not use (2000, another view) went.
        let mut dropped_record = false;
        for hwnd in kit::windows::<FolderHost>() {
            kit::scope::<FolderHost, _>(hwnd, || {
                let mut icon = None;
                kit::update::<FolderHost>(|h| {
                    if h.form.era != era {
                        // The items stay (their reader sends only what changes).
                        let (items, hidden, total, spaces, attributes) =
                            (h.form.view.items.clone(), h.form.hidden, h.form.total_size, h.form.spaces.clone(), h.form.web.attributes.clone());
                        h.form.set_era(era);
                        let (title, small) = title_from(era, &h.shows, &h.facts);
                        h.form.title = title;
                        h.form.icon = art::small(era, small);
                        // The place drawn anew in the new look (its frame, view and pictures are
                        // the look's), its items put back here at once.
                        if era != Era::Win95 || h.form.tree_shown {
                            show_new_place(h, era);
                        }
                        h.form.hidden = hidden;
                        h.form.total_size = total;
                        h.form.spaces = spaces;
                        let hidden_ids = hidden_of(&attributes);
                        h.form.web.attributes = attributes;
                        h.form.set_items(items);
                        h.form.view.set_hidden(hidden_ids);
                        // Each look stays pure (brief 106, 3): the old look's arrangement goes at
                        // once (the moved icons, their places and orders, this window's copies of
                        // every folder), and the folder is shown as the NEW look remembers it.
                        // Nothing of the old look is saved as the new look's record.
                        // (After the new look's font: its Small Icons' width is measured then.)
                        h.session.clear();
                        h.form.view.forget_moved();
                        icon = Some(h.form.icon.clone());
                        // Each look knew its own kinds of file (their pictures and Type names:
                        // `file_kinds`): a new reader reads the place again in the new look, as
                        // Refresh does. Until it has, a kind the new look did not know shows no
                        // picture (never the old look's).
                        h.stop.store(true, Ordering::SeqCst);
                        h.stop = Arc::new(AtomicBool::new(false));
                        start_reader(h.id, h.shows.clone(), era, h.stop.clone(), HUB.with(Cell::get), StartedBy::LookChange);
                    }
                    h.form.set_font(&look.regular);
                    // Tabs (brief 110): switched off, the tabs behind go (the one in front stays
                    // the window); the tabs behind take the new look too, the old look's
                    // arrangement forgotten (each look stays pure); the row as now set.
                    if TAB_SETTINGS.with(Cell::get).0 == tabs::Place::Off {
                        h.back.clear();
                        h.order = vec![h.tab];
                    }
                    for s in &mut h.back {
                        if s.form.era != era {
                            s.form.set_era(era);
                            s.form.view.forget_moved();
                            let (title, small) = title_from(era, &s.shows, &s.facts);
                            s.form.title = title;
                            s.form.icon = art::small(era, small);
                            s.session.clear();
                        }
                        s.form.set_font(&look.regular);
                    }
                    show_row(h);
                    if icon.is_some() {
                        dropped_record |= load_places(h, Relaid::Reopened);
                    }
                });
                if let Some(icon) = icon {
                    kit::set_program_icons::<FolderHost>(&icon, &icon);
                    if let Some(title) = kit::with::<FolderHost, _>(|h| wide(&h.form.title)) {
                        // SAFETY: our own window and a null-terminated title.
                        unsafe { SetWindowTextW(hwnd, title.as_ptr()) };
                    }
                }
                kit::retheme_with::<FolderHost>(look.clone(), |w, h, now| on_screen((now.left, now.top), w, h));
            });
        }
        if dropped_record {
            save();
        }
    }
}

/// The selected icons of the current window as the file commands' targets.
fn targets() -> Vec<Target> {
    kit::with::<FolderHost, _>(|h| {
        let v = &h.form.view;
        v.selection().into_iter().map(|i| Target { id: v.items[i].id.clone(), name: v.items[i].name.clone(), kind: kind_of(&v.items[i]) }).collect()
    })
    .unwrap_or_default()
}

/// The current window's folder (None: My Computer).
fn folder() -> Option<PathBuf> {
    kit::with::<FolderHost, _>(|h| match &h.shows {
        Shows::Folder(p) => Some(p.clone()),
        Shows::Desktop => shell::desktop_folder(),
        Shows::MyComputer => None,
    })
    .flatten()
}

/// The place an item stands for, if it is one a folder window shows (a folder, a drive, My
/// Computer, My Documents' folder).
fn place_of(t: &Target) -> Option<Shows> {
    match t.kind {
        ItemKind::Folder | ItemKind::Drive => shows_of(&t.id),
        // Not a folder that can be read now, but the place it is: going there starts Windows'
        // unlock window (`unlock_instead`), or the reading finds it locked (`locked_found`).
        ItemKind::LockedDrive => named_place(&t.id),
        ItemKind::System(bevel_core::desktop_icons::System::MyComputer) => Some(Shows::MyComputer),
        ItemKind::System(bevel_core::desktop_icons::System::MyDocuments) => shell::documents_folder().map(Shows::Folder),
        _ => None,
    }
}

/// Opens the selected items as Explorer would: in Windows 98 and 2000 one folder (or drive) in the
/// same window, in 95 (or several selected) each in a window of its own; Control Panel, Printers
/// and the Recycle Bin as Windows' own, a file with its program.
fn open_selected(hwnd: HWND) {
    let targets = targets();
    // 2000's Folder Options: "Open each folder in its own window" (95's way; not with the tree).
    let own = the_look().is_some_and(|(e, _)| e == Era::Win2000)
        && SAVED.with(|s| s.borrow().own_window)
        && !kit::with::<FolderHost, _>(|h| h.form.tree_shown || h.form.exploring).unwrap_or(false);
    // A drive shown locked and found so just now: Windows' unlock window at once. Shown locked
    // by an older reading: it opens as any drive (`unlock_shown_locked`).
    if let [one] = targets.as_slice()
        && unlock_shown_locked(one)
    {
        return;
    }
    if let [one] = targets.as_slice()
        && let Some(shows) = place_of(one)
        && !own
    {
        navigate(hwnd, Some(shows), Go::New);
        return;
    }
    for t in targets {
        open_target(t);
    }
}

/// Open on a drive whose icon is shown as locked (`kind_of`: its menu has Unlock Drive...) and
/// whose reading found it so just now (`LOCK_BELIEVED`): Windows' own unlock window at once,
/// without going to the drive first (true). An older record is not believed (the drive may have
/// been unlocked meanwhile by a way no reader saw): the drive opens in the ordinary way, and if it
/// is still locked, its reading finds that out (`Facts::locked`, `locked_found`; the review
/// before 0.1.3, B2).
fn unlock_shown_locked(t: &Target) -> bool {
    let Some(letter) = root_letter(&t.id).filter(|&l| shown_locked_acts(t.kind, known_locked(l, true))) else { return false };
    log(&format!("folders: {letter}: is shown locked by BitLocker; Windows' unlock window instead"));
    bevel_winkit::bitlocker::start_unlock(letter);
    true
}

/// Whether Open or Explore on an item of `kind` starts Windows' unlock window instead
/// (`fresh_locked`: the drive's last reading found it locked within `LOCK_BELIEVED`).
fn shown_locked_acts(kind: ItemKind, fresh_locked: bool) -> bool {
    matches!(kind, ItemKind::LockedDrive) && fresh_locked
}

/// Opens an item as Explorer would: a folder or a drive in a window of its own here, Control Panel
/// and Printers as Windows' own, a file with its program.
fn open_target(t: Target) {
    if unlock_shown_locked(&t) {
        return;
    }
    match place_of(&t) {
        Some(shows) => open(shows),
        None => {
            let name = file_ops::open_name(&t);
            let target = if name.starts_with("::") { format!("shell:{name}") } else { name };
            let spawned = std::thread::Builder::new().name("folder open".into()).spawn(move || {
                bevel_winkit::com::init();
                let file = wide(&target);
                // One of a portable copy's own programs is told where its data is (`log::flag_for`).
                let flag = bevel_winkit::log::flag_for(Path::new(&target)).map(wide);
                let params = flag.as_ref().map_or(null(), |f| f.as_ptr());
                // SAFETY: a null-terminated name; a null verb is its default command.
                let r = unsafe { ShellExecuteW(null_mut(), null(), file.as_ptr(), params, null(), SW_SHOWNORMAL) };
                if r as usize <= 32 {
                    log(&format!("folders: could not open an item (ShellExecute error {})", r as usize));
                }
            });
            if let Err(e) = spawned {
                log(&format!("folders: could not start opening an item: {e}"));
            }
        }
    }
}

/// Where the confirm questions go: the program's look, above the folder window.
fn confirm_place(hwnd: HWND) -> Option<ConfirmPlace> {
    let (era, look) = the_look()?;
    Some(ConfirmPlace { look, era, kind: Kind::Owned { owner: hwnd }, work: work_area(Some(hwnd)) })
}

/// Carries out a command of the menus or the keys on the current window.
fn run(hwnd: HWND, command: Command, by_keyboard: bool) {
    let selection = targets();
    let kinds: Vec<ItemKind> = selection.iter().map(|t| t.kind).collect();
    let offered = |c: D| bevel_core::desktop::item_entries(&kinds).contains(&Some(c));
    let dir = folder();
    match command {
        Command::Close => kit::close::<FolderHost>(),
        Command::Undo => {
            if let Some(place) = confirm_place(hwnd) {
                confirm::undo(place);
            }
        }
        Command::SelectAll => kit::update::<FolderHost>(|h| {
            h.form.view.key(bevel_core::desktop_icons::Key::SelectAll);
        }),
        Command::InvertSelection => kit::update::<FolderHost>(|h| h.form.view.invert_selection()),
        Command::StatusBar => {
            let on = kit::with::<FolderHost, _>(|h| !h.form.status_bar).unwrap_or(true);
            SAVED.with(|s| s.borrow_mut().status_bar = on);
            save();
            for w in kit::windows::<FolderHost>() {
                kit::scope::<FolderHost, _>(w, || {
                    kit::update::<FolderHost>(|h| {
                        h.form.status_bar = on;
                        let size = h.form.size_now();
                        let max = h.form.maximized;
                        h.form.resize(size, max);
                    })
                });
            }
        }
        Command::View(mode) => {
            let before = saved_text();
            kit::update::<FolderHost>(|h| {
                h.form.set_view_mode(mode);
                remember_view(h);
                // The moved icons' places as the window now has them, in its view (brief 105, 1).
                remember_places(h);
            });
            save_if_changed(&before);
        }
        Command::FolderProperties => {
            let target = match &dir {
                Some(p) => Target { id: p.to_string_lossy().into_owned(), name: String::new(), kind: ItemKind::Folder },
                None => Target { id: MY_COMPUTER.into(), name: "My Computer".into(), kind: ItemKind::System(bevel_core::desktop_icons::System::MyComputer) },
            };
            if dir.as_ref().is_some_and(|p| p.parent().is_none()) {
                // A drive's root: its own Properties (the shell's, by its path).
                file_ops::properties(&[Target { kind: ItemKind::Drive, ..target }]);
            } else {
                file_ops::properties(&[target]);
            }
        }
        Command::Up => {
            let shows = kit::with::<FolderHost, _>(|h| h.shows.clone());
            let era = the_look().map_or(Era::Win95, |(e, _)| e);
            match shows.and_then(|s| s.place().parent(era)) {
                Some(up) => navigate(hwnd, Some(Shows::of_place(&up)), Go::New),
                // Windows 95 said "You are at the top level..." in a message box: not built yet;
                // 98 grayed Up in My Computer, 2000 on the Desktop.
                None => log("folders: Up at the top level"),
            }
        }
        Command::Back => navigate(hwnd, None, Go::Back(1)),
        Command::Forward => navigate(hwnd, None, Go::Forward(1)),
        Command::GoMyComputer => navigate(hwnd, Some(Shows::MyComputer), Go::New),
        Command::Chain(n) => {
            let to = kit::with::<FolderHost, _>(|h| h.shows.place().chain().get(n as usize).cloned()).flatten();
            if let Some(p) = to {
                navigate(hwnd, Some(Shows::of_place(&p)), Go::New);
            }
        }
        Command::TreeRow(c) => tree_command(hwnd, c, by_keyboard),
        Command::TreeShown(on) => set_tree(hwnd, on),
        Command::ToggleTree => {
            let on = kit::with::<FolderHost, _>(|h| !h.form.tree_shown).unwrap_or(false);
            set_tree(hwnd, on);
        }
        Command::WebView => {
            // 98 kept as Web Page per folder.
            if let Some((key, on)) = kit::with::<FolderHost, _>(|h| (h.shows.key(), !h.form.web.on)) {
                SAVED.with(|s| {
                    let mut s = s.borrow_mut();
                    if on {
                        s.web_off.remove(&key);
                    } else {
                        s.web_off.insert(key.clone());
                    }
                });
                save();
                kit::update::<FolderHost>(|h| h.form.set_web(on && h.form.web.fonts.is_some()));
            }
        }
        Command::FolderOptions => folder_options(hwnd, by_keyboard),
        Command::Shared(c) => match c {
            D::Open if offered(D::Open) => open_selected(hwnd),
            D::Explore if offered(D::Explore) => {
                // Places shown here open in an Explorer window of Bevel95's; others in Windows'.
                for t in &selection {
                    if unlock_shown_locked(t) {
                        continue;
                    }
                    match place_of(t) {
                        Some(s) => open_explorer(s),
                        None => file_ops::explore(std::slice::from_ref(t)),
                    }
                }
            }
            // Windows' own unlock window; it asks for the password itself.
            D::UnlockDrive if offered(D::UnlockDrive) => {
                for letter in selection.iter().filter_map(|t| root_letter(&t.id)) {
                    log(&format!("folders: Unlock Drive... on {letter}:"));
                    bevel_winkit::bitlocker::start_unlock(letter);
                }
            }
            D::ItemProperties if offered(D::ItemProperties) => file_ops::properties(&selection),
            D::ItemProperties if selection.is_empty() => run(hwnd, Command::FolderProperties, by_keyboard),
            D::Cut | D::Copy if offered(c) => {
                if let Some(seq) = file_ops::put_on_clipboard(hwnd, &selection, c == D::Cut) {
                    let ids: Vec<String> = if c == D::Cut { selection.iter().map(|t| t.id.clone()).collect() } else { Vec::new() };
                    CUT.with(|cut| *cut.borrow_mut() = (ids.clone(), seq));
                    show_cut(ids);
                }
            }
            D::Paste => {
                if let Some(dir) = dir {
                    file_ops::paste(hwnd, dir);
                }
            }
            D::PasteShortcut => {
                if let Some(dir) = dir {
                    let w = hwnd as usize;
                    file_ops::paste_shortcuts(hwnd, dir, move |made| select_when_shown(w, made));
                }
            }
            D::CreateShortcut if offered(D::CreateShortcut) => {
                // My Computer's items have no folder to hold a shortcut: Windows offered the
                // desktop instead.
                let into = dir.or_else(shell::desktop_folder);
                if let Some(into) = into {
                    let w = hwnd as usize;
                    file_ops::create_shortcuts(&selection, into, move |made| select_when_shown(w, made));
                }
            }
            D::Delete if offered(D::Delete) => {
                if let Some(place) = confirm_place(hwnd) {
                    confirm::ask_delete(place, selection, by_keyboard);
                }
            }
            D::Rename if offered(D::Rename) => {
                if let [one] = selection.as_slice() {
                    let id = one.id.clone();
                    kit::update::<FolderHost>(|h| {
                        if let Some(i) = h.form.view.index_of(&id) {
                            h.form.begin_rename(i);
                        }
                    });
                }
            }
            D::NewFolder | D::NewTextDocument => {
                if let Some(dir) = dir {
                    new_item(&dir, c == D::NewFolder);
                }
            }
            D::NewShortcut => {
                if let Some(dir) = dir {
                    shortcut_wizard::new_shortcut_in(&dir);
                }
            }
            D::Arrange(by) => {
                let before = saved_text();
                kit::update::<FolderHost>(|h| {
                    h.form.arrange_by(by);
                    remember_view(h);
                    // Arranged: the places dragged to, and an order with moved icons last, are
                    // forgotten (the choice itself is the folder's view: `remember_view`).
                    let (era, key) = (h.form.era, h.shows.key());
                    h.session.remove(&key);
                    let view = h.form.view.moved_held();
                    SAVED.with(|s| {
                        let mut s = s.borrow_mut();
                        s.remember_places(era, &key, view, Vec::new());
                        s.remember_last(era, &key, Vec::new());
                    });
                });
                // Written only when something changed (brief 106, 7).
                save_if_changed(&before);
            }
            D::AutoArrange => {
                let before = saved_text();
                kit::update::<FolderHost>(|h| {
                    let on = !h.form.view.auto_arrange();
                    h.form.set_auto_arrange(on);
                    remember_places(h);
                });
                save_if_changed(&before);
            }
            // Line up Icons (brief 103): each icon off the grid onto the nearest grid place.
            D::LineUp => {
                let mut changed = false;
                kit::update::<FolderHost>(|h| {
                    h.form.line_up();
                    changed = remember_places(h);
                });
                if changed {
                    save();
                }
            }
            D::Refresh => {
                // What the look does with the moved icons at F5 (brief 103), then a new reader:
                // everything read afresh.
                let mut changed = false;
                kit::update::<FolderHost>(|h| changed = refresh_places(h));
                if changed {
                    save();
                }
                kit::with::<FolderHost, _>(|h| {
                    h.stop.store(true, Ordering::SeqCst);
                    h.stop = Arc::new(AtomicBool::new(false));
                    start_reader(h.id, h.shows.clone(), h.form.era, h.stop.clone(), HUB.with(Cell::get), StartedBy::Person);
                });
            }
            _ => {}
        },
    }
}

// ---------------------------------------------------------------------------------------------
// Tabs in folder windows (Bevel95's own, brief 110).

/// The row as the window shows it now (`TAB_SETTINGS`): each tab's name (the folder's own, as the
/// title bar shows it) and whole path (its tooltip), the one in front; `Off`: no row.
fn show_row(h: &mut FolderHost) {
    let (place, close_at) = TAB_SETTINGS.with(Cell::get);
    if place == tabs::Place::Off {
        if h.form.tab_place() != tabs::Place::Off || !h.form.tab_row.tabs.is_empty() {
            h.form.set_tabs(tabs::Place::Off, tabs::Row::default());
        }
        return;
    }
    let tip = |s: &Shows, name: &str| match s {
        Shows::Folder(p) => p.to_string_lossy().into_owned(),
        _ => name.to_string(),
    };
    let mut row = tabs::Row { close_at, ..tabs::Row::default() };
    for (k, &id) in h.order.iter().enumerate() {
        let (title, tip) = if id == h.tab {
            row.front = k;
            (h.form.place_name.clone(), tip(&h.shows, &h.form.place_name))
        } else if let Some(s) = h.back.iter().find(|s| s.tab == id) {
            (s.form.place_name.clone(), tip(&s.shows, &s.form.place_name))
        } else {
            continue;
        };
        row.tabs.push(tabs::Tab { id, title, tip });
    }
    h.form.set_tabs(place, row);
}

/// The tab in front goes behind as it is (its reader stops; its view, place and history stay
/// in its slot).
fn to_back(h: &mut FolderHost, form: FolderWindow) -> TabSlot {
    h.stop.store(true, Ordering::SeqCst);
    h.stop = Arc::new(AtomicBool::new(false));
    if let Ok(mut g) = GENERATIONS.lock()
        && let Some(g) = g.as_mut()
    {
        g.forget(h.id);
    }
    remember_view(h);
    TabSlot {
        tab: h.tab,
        form: std::mem::replace(&mut h.form, form),
        shows: h.shows.clone(),
        select: h.select.take(),
        history: std::mem::replace(&mut h.history, History::new(h.shows.place())),
        came: h.came.take(),
        entries: std::mem::take(&mut h.entries),
        settle: h.settle.take(),
        reread: std::mem::take(&mut h.reread),
        asked: h.asked.take(),
        facts: h.facts.clone(),
        session: std::mem::take(&mut h.session),
    }
}

/// A new tab at the row's right end showing `shows`, in front (Ctrl+T, a double-click on the
/// row: My Computer; Open in New Tab: a folder). At most `tabs::MOST`: a 33rd is refused with
/// the look's default sound.
fn new_tab(hwnd: HWND, shows: Shows) {
    let made = kit::with::<FolderHost, _>(|h| {
        if TAB_SETTINGS.with(Cell::get).0 == tabs::Place::Off || h.order.len() >= tabs::MOST {
            return false;
        }
        // The window as it is goes behind; the new tab starts as its copy and goes to its place.
        let copy = h.form.clone();
        let slot = to_back(h, copy);
        h.back.push(slot);
        h.tab = NEXT_TAB.with(|n| n.replace(n.get() + 1));
        h.order.push(h.tab);
        true
    })
    .unwrap_or(false);
    if !made {
        // SAFETY: a plain call.
        unsafe { windows_sys::Win32::System::Diagnostics::Debug::MessageBeep(0) };
        return;
    }
    let same = kit::with::<FolderHost, _>(|h| h.shows == shows).unwrap_or(false);
    if same {
        // The same place: a reader of its own (the one copied was stopped).
        restart_reader(hwnd);
    } else {
        IN_NEW_TAB.with(|t| t.set(true));
        navigate(hwnd, Some(shows), Go::New);
        IN_NEW_TAB.with(|t| t.set(false));
    }
    kit::update::<FolderHost>(|h| {
        // Its own history begins at its place.
        h.history = History::new(h.shows.place());
        h.came = None;
        show_row(h);
    });
}

/// A new window id and a reader for the place the tab in front shows (it came to the front),
/// the title bar, the task button's picture and the tree following it.
fn restart_reader(hwnd: HWND) {
    let Some((era, _)) = the_look() else { return };
    let started = kit::with::<FolderHost, _>(|h| {
        let id = NEXT_ID.with(|n| n.replace(n.get() + 1));
        h.id = id;
        h.stop = Arc::new(AtomicBool::new(false));
        h.asked = Some(std::time::Instant::now());
        if h.form.tree_shown {
            let chain = tree_chain(era, &h.shows, false, &h.facts);
            let page = h.form.tree_page();
            h.form.tree.follow(chain, page);
            h.form.tree_selection_shown();
        }
        show_row(h);
        (id, h.shows.clone(), h.stop.clone(), h.form.title.clone(), h.form.icon.clone(), h.form.exploring)
    });
    let Some((id, shows, stop, title, icon, exploring)) = started else { return };
    let key = window_key(&shows, exploring);
    WINDOWS.with(|w| {
        if let Some(entry) = w.borrow_mut().iter_mut().find(|(_, h, _)| *h == hwnd) {
            *entry = (id, hwnd, key);
        }
    });
    let text = wide(&title);
    // SAFETY: our own window and a null-terminated title (the task button shows it).
    unsafe { SetWindowTextW(hwnd, text.as_ptr()) };
    kit::set_program_icons::<FolderHost>(&icon, &icon);
    kit::update::<FolderHost>(|_| {});
    start_reader(id, shows, era, stop, HUB.with(Cell::get), StartedBy::LookChange);
}

/// Tab `tab` comes to the front (a press on it, Ctrl+Tab): found by its id, nothing if it is
/// gone or in front already.
fn switch_tab(hwnd: HWND, tab: u64) {
    let switched = kit::with::<FolderHost, _>(|h| {
        if tab == h.tab {
            return false;
        }
        let Some(k) = h.back.iter().position(|s| s.tab == tab) else { return false };
        let mut slot = h.back.remove(k);
        slot.form.take_window_from(&h.form);
        let out = to_back(h, slot.form);
        h.back.push(out);
        h.tab = slot.tab;
        h.shows = slot.shows;
        h.select = slot.select;
        h.history = slot.history;
        h.came = slot.came;
        h.entries = slot.entries;
        h.settle = slot.settle;
        h.reread = slot.reread;
        h.asked = slot.asked;
        h.facts = slot.facts;
        h.session = slot.session;
        true
    })
    .unwrap_or(false);
    if switched {
        restart_reader(hwnd);
    }
}

/// The next (or the previous) tab round the row comes to the front.
fn step_tab(hwnd: HWND, forward: bool) {
    let to = kit::with::<FolderHost, _>(|h| {
        let n = h.order.len();
        let k = h.order.iter().position(|&t| t == h.tab)?;
        (n > 1).then(|| h.order[if forward { (k + 1) % n } else { (k + n - 1) % n }])
    })
    .flatten();
    if let Some(t) = to {
        switch_tab(hwnd, t);
    }
}

/// Closes tab `tab` (the small X, Ctrl+W: the one in front). The last tab closes the window. A
/// tab behind has no reader; its view goes with it. A file job it started outlives it as it
/// outlives a window (`winkit::jobs`); a question it asked acts on the items by their ids.
fn close_tab(hwnd: HWND, tab: u64) {
    let state = kit::with::<FolderHost, _>(|h| {
        if h.order.len() <= 1 {
            return Some(None);
        }
        let k = h.order.iter().position(|&t| t == tab)?;
        if tab != h.tab {
            h.order.remove(k);
            h.back.retain(|s| s.tab != tab);
            show_row(h);
            return Some(Some(None));
        }
        // The one in front: its right-hand neighbour comes forward (the left one at the end).
        let next = if k + 1 < h.order.len() { h.order[k + 1] } else { h.order[k - 1] };
        Some(Some(Some(next)))
    })
    .flatten();
    match state {
        Some(None) => kit::close::<FolderHost>(),
        Some(Some(Some(next))) => {
            switch_tab(hwnd, next);
            kit::update::<FolderHost>(|h| {
                h.order.retain(|&t| t != tab);
                h.back.retain(|s| s.tab != tab);
                show_row(h);
            });
        }
        Some(Some(None)) => kit::update::<FolderHost>(|_| {}),
        None => {}
    }
}

/// The row's menu (the right button on a tab, `tab`, or on the row, 0), in the look's style: New
/// Tab, Close Tab, Close Other Tabs. Each acts on its tab by id; a tab gone meanwhile is left.
fn tab_menu(hwnd: HWND, tab: u64) {
    use bevel_core::paint::{Menu, MenuEntry};
    let Some(scale) = kit::look_of::<FolderHost>().map(|l| l.scale) else { return };
    let Some((_, look)) = the_look() else { return };
    let Some((era, at, front, count)) = kit::with::<FolderHost, _>(|h| (h.form.era, h.form.menu_at, h.tab, h.order.len())) else { return };
    let tab = if tab == 0 { front } else { tab };
    let mut menu = Menu::new(vec![MenuEntry::item("New &Tab", None), MenuEntry::item("&Close Tab", None), MenuEntry::item("Close &Other Tabs", None)]);
    menu.entries[2].disabled = count < 2;
    menu.show_keys = era.always_shows_keys();
    let at = to_screen(hwnd, at, scale);
    let ended = menu::track_with_selection(&look, Request { owner: hwnd, menu, style: MenuStyle::Popup, at, above: None, titles: &[] }, |_| {});
    match ended.outcome {
        Outcome::Chosen(0) => new_tab(hwnd, Shows::MyComputer),
        Outcome::Chosen(1) => close_tab(hwnd, tab),
        Outcome::Chosen(2) if count >= 2 => {
            switch_tab(hwnd, tab);
            kit::update::<FolderHost>(|h| {
                if h.tab == tab {
                    h.back.clear();
                    h.order = vec![tab];
                    show_row(h);
                }
            });
        }
        _ => {
            if let Some(msg) = ended.pending {
                menu::dispatch(&msg);
            }
        }
    }
}

/// What the look does with the icons the person moved in the window's folder when it lays them out
/// again (`saved::kept`, brief 103): by the look, the folder's web view and whether it is My Computer.
fn kept_for(h: &FolderHost, when: Relaid) -> Kept {
    let era = h.form.era;
    let key = h.shows.key();
    kept(era, web_on(era, &key), matches!(h.shows, Shows::MyComputer), when)
}

/// The folder whose items a record names by their own names (`short_id`, brief 106, 4): a
/// folder's path; None for My Computer and the Desktop place (their items lie in several places
/// and keep their whole ids).
fn record_dir(shows: &Shows) -> Option<String> {
    match shows {
        Shows::Folder(p) => Some(p.to_string_lossy().into_owned()),
        Shows::MyComputer | Shows::Desktop => None,
    }
}

/// The order a look that remembers no places keeps (Windows 98 with its web view): the icons
/// already last, then the ones moved since, each moved one counting as moved to the end.
fn order_with_moved_last(h: &FolderHost) -> Vec<String> {
    let mut order = h.form.view.last_order().to_vec();
    for id in h.form.view.moved_ids() {
        order.retain(|o| o != id);
        order.push(id.clone());
    }
    order
}

/// Remembers the icons the person moved in a window's folder (a drop, Line up Icons, Auto
/// Arrange; "State persists"): only those icons, by id (brief 103). While the window stays open
/// their places (`session`); for good, their places where the look keeps places when the
/// window is opened again, else (Windows 98 with its web view) the order with them last.
///
/// Each record belongs to the look (`era`) and the view (Large or Small Icons) it was made in
/// (brief 105, 1 and 8); names no longer in the folder are left out whenever it is rewritten
/// (brief 105, 6). True if folders.txt's record changed (then the caller saves).
fn remember_places(h: &mut FolderHost) -> bool {
    let era = h.form.era;
    let key = h.shows.key();
    if h.form.view.items.is_empty() {
        // Not read yet (or empty): nothing to say about its icons; the record stays as it is.
        return false;
    }
    let here: std::collections::HashSet<&str> = h.form.view.items.iter().map(|i| i.id.as_str()).collect();
    let places: Vec<(i32, i32, String)> =
        h.form.view.moved_places().into_iter().filter(|(_, _, id)| here.contains(id.as_str())).collect();
    let view = h.form.view.moved_held();
    let kept_how = kept_for(h, Relaid::Reopened);
    let last: Vec<String> = match kept_how {
        Kept::Places => h.form.view.last_order().to_vec(),
        Kept::MovedLast | Kept::InOrder => order_with_moved_last(h),
    };
    let last: Vec<String> = last.into_iter().filter(|id| here.contains(id.as_str())).collect();
    drop(here);
    if places.is_empty() {
        h.session.remove(&key);
    } else {
        h.session.insert(key.clone(), (view, places.clone()));
    }
    // Kept by each item's own name within the folder (brief 106, 4).
    let dir = record_dir(&h.shows);
    let dir = dir.as_deref();
    let places: Vec<(i32, i32, String)> = places.into_iter().map(|(x, y, id)| (x, y, short_id(dir, &id))).collect();
    let last: Vec<String> = last.iter().map(|id| short_id(dir, id)).collect();
    SAVED.with(|s| {
        let mut s = s.borrow_mut();
        match kept_how {
            Kept::Places => {
                let a = s.remember_places(era, &key, view, places);
                let b = s.remember_last(era, &key, last);
                a || b
            }
            Kept::MovedLast | Kept::InOrder => {
                let a = s.remember_places(era, &key, view, Vec::new());
                let b = s.remember_last(era, &key, last);
                a || b
            }
        }
    })
}

/// Lays a window's icons out as its look keeps them when the window opens (`Reopened`) or goes
/// back to a folder (`Returned`): the moved icons where they were left (this window's own record
/// first, then the folder's), or packed with the moved ones last (Windows 98 with its web view).
/// Only this look's record is used (brief 105, 8). Places made in the other free view are moved
/// as the view's own step moves an icon (95, 98); 2000's own step puts the icons back in order,
/// so there the record goes (brief 105, 1). True if folders.txt's record changed.
fn load_places(h: &mut FolderHost, when: Relaid) -> bool {
    let era = h.form.era;
    let key = h.shows.key();
    let (places, last) = SAVED.with(|s| {
        let s = s.borrow();
        (s.places_of(era, &key), s.last_of(era, &key))
    });
    // The record names items within the folder (brief 106, 4): their ids again.
    let dir = record_dir(&h.shows);
    let dir = dir.as_deref();
    let places = places.map(|(held, list)| (held, list.into_iter().map(|(x, y, n)| (x, y, full_id(dir, &n))).collect::<Vec<_>>()));
    let last: Vec<String> = last.iter().map(|n| full_id(dir, n)).collect();
    match kept_for(h, when) {
        Kept::Places => {
            if !last.is_empty() {
                h.form.view.pack_moved_last(&last);
            }
            let mine = match h.session.get(&key) {
                Some(mine) if when == Relaid::Returned => Some(mine.clone()),
                _ => None,
            };
            let Some((made_in, list)) = mine.or(places) else { return false };
            if h.form.view.load_moved(&list, made_in) {
                return false;
            }
            // Windows 2000 in the other view: the places are not used and go, the window's own
            // and the folder's record when it is of another view than the one shown.
            h.session.remove(&key);
            let shown = Held::Places(h.form.view.mode());
            let stale = SAVED.with(|s| s.borrow().places_of(era, &key)).is_some_and(|(v, _)| v != shown && v != Held::Order);
            stale && SAVED.with(|s| s.borrow_mut().remember_places(era, &key, made_in, Vec::new()))
        }
        Kept::MovedLast => {
            let mut order = last;
            // Moved in this window and not packed yet (My Computer kept them while it stayed open).
            if let Some((_, mine)) = h.session.remove(&key) {
                for (_, _, id) in mine {
                    order.retain(|o| *o != id);
                    order.push(id);
                }
            }
            h.form.view.pack_moved_last(&order);
            false
        }
        Kept::InOrder => false,
    }
}

/// F5 (View > Refresh): what the look does with the moved icons before the folder is read
/// again (brief 103): kept; packed with the moved ones last (98 with its web view, an ordinary
/// folder); or every icon back in its order, a kept "moved last" order included (2000 with its
/// web view, brief 105, 9). True if folders.txt's record changed (brief 105, 7: only then is it
/// written).
fn refresh_places(h: &mut FolderHost) -> bool {
    let era = h.form.era;
    let key = h.shows.key();
    match kept_for(h, Relaid::Refresh) {
        Kept::Places => false,
        Kept::MovedLast => {
            let here: std::collections::HashSet<&str> = h.form.view.items.iter().map(|i| i.id.as_str()).collect();
            let order: Vec<String> = order_with_moved_last(h).into_iter().filter(|id| here.contains(id.as_str())).collect();
            drop(here);
            h.form.view.pack_moved_last(&order);
            h.session.remove(&key);
            let view = h.form.view.moved_held();
            let dir = record_dir(&h.shows);
            let order: Vec<String> = order.iter().map(|id| short_id(dir.as_deref(), id)).collect();
            SAVED.with(|s| {
                let mut s = s.borrow_mut();
                let a = s.remember_places(era, &key, view, Vec::new());
                let b = s.remember_last(era, &key, order);
                a || b
            })
        }
        Kept::InOrder => {
            h.form.view.back_in_order();
            h.session.remove(&key);
            let view = h.form.view.moved_held();
            SAVED.with(|s| {
                let mut s = s.borrow_mut();
                let a = s.remember_places(era, &key, view, Vec::new());
                let b = s.remember_last(era, &key, Vec::new());
                a || b
            })
        }
    }
}

/// Remembers how a window's folder is shown (95's and 98's Explorer: its own view).
fn remember_view(h: &FolderHost) {
    let era = h.form.era;
    let state = h.form.view_state();
    let key = h.shows.key();
    SAVED.with(|s| {
        let mut s = s.borrow_mut();
        if h.form.exploring && era != Era::Win2000 {
            s.explorer_view = Some(state.mode);
            // Its order and columns still belong to the folder.
            let mut own = s.views.get(&key).cloned().unwrap_or_else(|| state.clone());
            own.arrange = state.arrange;
            own.reversed = state.reversed;
            own.columns = state.columns.clone();
            s.remember_view(&key, own);
        } else {
            s.remember_view(&key, state);
        }
    });
}

/// How a window shows `shows` when it opens or goes there (`count` items; `new_window`).
fn opening_state(era: Era, shows: &Shows, exploring: bool, count: usize, new_window: bool) -> ViewState {
    SAVED.with(|s| {
        let s = s.borrow();
        let own = s.views.get(&shows.key()).cloned();
        let mode = if exploring && era != Era::Win2000 {
            s.explorer_view.unwrap_or(bevel_core::desktop_icons::ViewMode::List)
        } else {
            bevel_core::folder::saved::opening_view(era, own.as_ref().map(|v| v.mode), count, new_window)
        };
        match own {
            Some(v) => ViewState { mode, ..v },
            None => ViewState { mode, arrange: Default::default(), reversed: false, columns: Vec::new() },
        }
    })
}

// ---------------------------------------------------------------------------------------------
// Windows 98's and 2000's toolbar and address bar.

/// A toolbar button was let go on: what its menu entry does.
fn tool(hwnd: HWND, t: Tool) {
    let command = match t {
        Tool::Back => Command::Back,
        Tool::Forward => Command::Forward,
        Tool::Up => Command::Up,
        Tool::Cut => Command::Shared(D::Cut),
        Tool::Copy => Command::Shared(D::Copy),
        Tool::Paste => Command::Shared(D::Paste),
        Tool::Undo => Command::Undo,
        Tool::Delete => Command::Shared(D::Delete),
        Tool::Properties => Command::Shared(D::ItemProperties),
        Tool::Folders => Command::ToggleTree,
        // 98's Views: the next view, round from Details to Large Icons (`win98-truecolor-view-views-after-click`).
        Tool::Views => {
            let now = kit::with::<FolderHost, _>(|h| h.form.view_mode()).unwrap_or_default();
            let all = bevel_core::desktop_icons::ViewMode::ALL;
            let k = all.iter().position(|&m| m == now).unwrap_or(0);
            Command::View(all[(k + 1) % all.len()])
        }
    };
    run(hwnd, command, false);
}

/// The Views button's menu (98's arrow, 2000's button): the four views, the one shown with the
/// dot, under the button's left edge.
fn views_menu(hwnd: HWND) {
    let Some((_, look)) = the_look() else { return };
    let scale = look.scale;
    let Some((rows, button)) = kit::with::<FolderHost, _>(|h| (menus::view_choices(&menu_state(h, false)), h.form.rect(Part::Tool(Tool::Views)))) else { return };
    let Some(button) = button else { return };
    kit::update::<FolderHost>(|h| h.form.bar.open_list = Some(Tool::Views));
    let at = to_screen(hwnd, (button.x, button.bottom()), scale);
    let ended = menu::track(&look, Request { owner: hwnd, menu: menus::menu(&rows), style: MenuStyle::Popup, at, above: None, titles: &[] });
    kit::update::<FolderHost>(|h| h.form.bar.open_list = None);
    match ended.outcome {
        Outcome::Chosen(i) => {
            if let Some(c) = rows.get(i).and_then(|r| r.command) {
                run(hwnd, c, false);
            }
        }
        _ => {
            if let Some(msg) = ended.pending {
                menu::dispatch(&msg);
            }
        }
    }
}

/// The little arrow beside Back or Forward: the places that way, the nearest first; the one chosen
/// is gone to.
fn places_menu(hwnd: HWND, t: Tool) {
    if t == Tool::Views {
        return views_menu(hwnd);
    }
    let Some((era, look)) = the_look() else { return };
    let scale = look.scale;
    let Some((places, button)) = kit::with::<FolderHost, _>(|h| {
        let list = if t == Tool::Back { h.history.back_list() } else { h.history.forward_list() };
        (list, h.form.rect(Part::Tool(t)))
    }) else {
        return;
    };
    let Some(button) = button else { return };
    if places.is_empty() {
        return;
    }
    let entries =
        places.iter().map(|p| MenuEntry::item(&escape_access_keys(title_of(era, &Shows::of_place(p)).0.trim()), None)).collect();
    let menu = bevel_core::paint::Menu::new(entries);
    kit::update::<FolderHost>(|h| h.form.bar.open_list = Some(t));
    let at = to_screen(hwnd, (button.x, button.bottom()), scale);
    let ended = menu::track(&look, Request { owner: hwnd, menu, style: MenuStyle::Popup, at, above: None, titles: &[] });
    kit::update::<FolderHost>(|h| h.form.bar.open_list = None);
    match ended.outcome {
        Outcome::Chosen(i) => navigate(hwnd, None, if t == Tool::Back { Go::Back(i + 1) } else { Go::Forward(i + 1) }),
        _ => {
            if let Some(msg) = ended.pending {
                menu::dispatch(&msg);
            }
        }
    }
}

/// The folders on the desktop (the address bar's list holds them).
fn desktop_folders() -> Vec<(String, String)> {
    desktop_folders_with(shell::explorer_switches()).into_iter().map(|(path, name, _)| (path, name)).collect()
}

/// The folders on the desktop that Windows' two switches show, each with whether it is hidden.
fn desktop_folders_with(switches: bevel_core::hidden::Switches) -> Vec<(String, String, bool)> {
    let mut out = Vec::new();
    for dir in [shell::desktop_folder(), shell::public_desktop_folder()].into_iter().flatten() {
        if let Some((seen, _)) = shell::list_folder_with(&dir, switches) {
            for s in seen.into_iter().filter(|s| s.folder) {
                let path = s.path.to_string_lossy().into_owned();
                let name = shell::display_name(&path).unwrap_or_else(|| s.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
                let hidden = s.hidden();
                out.push((path, name, hidden));
            }
        }
    }
    out.sort_by_key(|(_, n, _)| n.to_lowercase());
    out
}

/// The address bar's arrow: its list opens (the Desktop, My Computer and its items, the way down
/// to the place shown, the desktop's own items and folders), or closes.
fn toggle_list() {
    let Some((era, _)) = the_look() else { return };
    let open = kit::with::<FolderHost, _>(|h| h.form.list.is_some()).unwrap_or(false);
    if open {
        kit::update::<FolderHost>(|h| {
            h.form.set_list(None);
            h.entries.clear();
        });
        return;
    }
    let computer = read_my_computer(era, false)
        .items
        .iter()
        .map(|i| {
            let small = match i.picture {
                Picture::Art(Art::Drive(k)) => Small::Drive(k),
                Picture::Art(Art::Printers) => Small::Printers,
                _ => Small::ControlPanel,
            };
            (i.id.clone(), i.name.clone(), small)
        })
        .collect();
    let tree = address::Tree { computer, desktop_folders: desktop_folders() };
    let Some(now) = kit::with::<FolderHost, _>(|h| h.shows.place()) else { return };
    let names = |p: &str| title_of(era, &Shows::Folder(PathBuf::from(p))).0;
    let entries = address::entries(era, &tree, &now, &names);
    let rows: Vec<ListRow> = entries.iter().map(|e| ListRow { level: e.level, icon: art::listed(era, e.small), name: e.name.clone() }).collect();
    // 2000 opened its list on the place shown, highlighted (`address-list`).
    let hot = (era == Era::Win2000).then(|| entries.iter().position(|e| e.place.as_ref() == Some(&now))).flatten();
    kit::update::<FolderHost>(|h| {
        h.form.set_list(Some((rows, hot)));
        h.entries = entries;
    });
}

/// A row of the address bar's list was chosen: its place is gone to, or (Control Panel, Printers,
/// the Recycle Bin) Windows' own window opens.
fn list_chosen(hwnd: HWND, i: usize) {
    let entry = kit::with::<FolderHost, _>(|h| {
        h.form.set_list(None);
        h.form.field.blur();
        let e = h.entries.get(i).cloned();
        h.entries.clear();
        e
    })
    .flatten();
    kit::update::<FolderHost>(|_| {});
    let Some(e) = entry else { return };
    if let Some(p) = &e.place {
        navigate(hwnd, Some(Shows::of_place(p)), Go::New);
        return;
    }
    match e.open_id.as_deref() {
        Some("::my-documents") => {
            if let Some(d) = shell::documents_folder() {
                navigate(hwnd, Some(Shows::Folder(d)), Go::New);
            }
        }
        Some("::recycle-bin") => open_target(Target {
            id: "::recycle-bin".into(),
            name: "Recycle Bin".into(),
            kind: ItemKind::System(bevel_core::desktop_icons::System::RecycleBin),
        }),
        Some(id) => {
            let era = the_look().map_or(Era::Win95, |(e, _)| e);
            if let Some(item) = read_my_computer(era, false).items.iter().find(|it| it.id == id) {
                open_target(Target { id: item.id.clone(), name: item.name.clone(), kind: kind_of(item) });
            }
        }
        None => {}
    }
}

/// Enter in the address bar, or Go: a folder, a drive, My Computer or the Desktop is gone to in
/// the window; anything else is handed to Windows as Run would (a file, a program, a web
/// address), and the address of the place shown comes back.
fn go_typed(hwnd: HWND) {
    let Some((era, _)) = the_look() else { return };
    let Some(text) = kit::with::<FolderHost, _>(|h| h.form.field.text.clone()) else { return };
    let drives: Vec<(String, String)> = read_my_computer(era, false).items.iter().map(|i| (i.id.clone(), i.name.clone())).collect();
    // A relative name means one in the folder the window shows (the Desktop's: the desktop
    // folder), never one in the program's working folder (the second safety review of
    // 2026-11-18, C2).
    let shown = kit::with::<FolderHost, _>(|h| match &h.shows {
        Shows::Folder(p) => Some(p.clone()),
        Shows::Desktop => bevel_winkit::shell::desktop_folder(),
        Shows::MyComputer => None,
    })
    .flatten();
    let place = address::typed_place(&text, &drives, shown.as_deref().map(|p| p.to_str().unwrap_or_default()));
    let shows = place.as_ref().map(Shows::of_place).filter(|s| match s {
        Shows::Folder(p) => p.is_absolute() && p.is_dir(),
        _ => true,
    });
    let opening = shows.is_none() && !text.trim().is_empty();
    kit::update::<FolderHost>(|h| {
        h.form.set_list(None);
        // Something handed to Windows: the typed text stays until it is known whether it opened.
        if !opening {
            let back = h.form.field.place_text.clone();
            h.form.field.show(&back);
            h.form.field.blur();
        }
    });
    match shows {
        Some(s) => navigate(hwnd, Some(s), Go::New),
        None if opening => {
            // A file found in the folder shown goes whole; anything else as typed (a program on
            // the PATH, an address), started in the folder shown (else the person's own folder).
            let found = match &place {
                Some(Place::Folder(p)) if Path::new(p).is_absolute() && Path::new(p).exists() => Some(p.clone()),
                _ => None,
            };
            let target = found.unwrap_or_else(|| text.trim().to_string());
            let start_in = shown.or_else(bevel_winkit::shell::profile_folder).map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
            let owner = hwnd as isize;
            let hub = HUB.with(Cell::get) as isize;
            let spawned = std::thread::Builder::new().name("folder address".into()).spawn(move || {
                bevel_winkit::com::init();
                let (file, folder) = (wide(&target), wide(&start_in));
                // One of a portable copy's own programs is told where its data is (`log::flag_for`).
                let flag = bevel_winkit::log::flag_for(Path::new(&target)).map(wide);
                let params = flag.as_ref().map_or(null(), |f| f.as_ptr());
                // SAFETY: null-terminated strings; a null verb is the default command.
                let r = unsafe { ShellExecuteW(null_mut(), null(), file.as_ptr(), params, folder.as_ptr(), SW_SHOWNORMAL) };
                if r as usize <= 32 {
                    log(&format!("folders: what was typed into the address bar could not be opened (ShellExecute error {})", r as usize));
                    ADDRESS_FAILED.lock().map(|mut f| f.push((owner, target))).ok();
                    // SAFETY: posting to the program's own hidden window.
                    unsafe { PostMessageW(hub as HWND, WM_ITEMS.id(), 0, 4) };
                } else {
                    // SAFETY: posting to the program's own hidden window.
                    unsafe { PostMessageW(hub as HWND, WM_ITEMS.id(), owner as usize, 5) };
                }
            });
            if let Err(e) = spawned {
                log(&format!("folders: could not start opening an address: {e}"));
            }
        }
        None => {}
    }
}

/// New > Folder or New > Text Document: made in the folder under Windows' own default name and
/// selected once it shows.
fn new_item(dir: &Path, folder: bool) {
    let (base, extension) = if folder { ("New folder", "") } else { ("New Text Document", ".txt") };
    let name = bevel_core::desktop::new_name(base, extension, |n| dir.join(n).exists());
    let path = dir.join(&name);
    let made = if folder { std::fs::create_dir(&path) } else { std::fs::OpenOptions::new().write(true).create_new(true).open(&path).map(|_| ()) };
    match made {
        Ok(()) => {
            log("folders: New made an item");
            let id = path.to_string_lossy().into_owned();
            kit::with::<FolderHost, _>(|h| h.select = Some(id));
        }
        Err(e) => log(&format!("folders: New could not make an item: {e}")),
    }
}

/// A shortcut made by Create Shortcut or Paste Shortcut is selected once its window shows it
/// (called on the command's thread: the window's id travels as a number).
fn select_when_shown(hwnd: usize, path: String) {
    PENDING_SELECT.lock().map(|mut p| p.push((hwnd, path))).ok();
    let hub = HUB_FOR_THREADS.load(Ordering::SeqCst);
    // SAFETY: posting to the program's own window.
    unsafe { PostMessageW(hub as HWND, WM_ITEMS.id(), 0, 0) };
}

static PENDING_SELECT: Mutex<Vec<(usize, String)>> = Mutex::new(Vec::new());
static HUB_FOR_THREADS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Shows the items now cut as cut in every window (and none once something else is on the
/// clipboard).
fn show_cut(ids: Vec<String>) {
    for w in kit::windows::<FolderHost>() {
        let ids = ids.clone();
        kit::scope::<FolderHost, _>(w, || kit::update::<FolderHost>(|h| h.form.view.set_cut(ids)));
    }
}

/// Renaming ended with the name typed: Windows' file machinery renames the file, after the look's
/// warning when the new name changes a known extension.
fn finish_rename(hwnd: HWND, by_keyboard: bool) {
    let pending = kit::with::<FolderHost, _>(|h| {
        let v = &h.form.view;
        let i = v.renaming()?;
        let item = &v.items[i];
        match v.rename_result() {
            RenameEnd::Rename { name, .. } => Some((item.id.clone(), item.name.clone(), name, item.folder)),
            RenameEnd::Unchanged => None,
        }
    })
    .flatten();
    let Some((id, shown, typed, folder)) = pending else {
        kit::update::<FolderHost>(|h| {
            h.form.view.end_rename();
        });
        return;
    };
    // A name that would be a path ("..\x", "C:\x", "a/b") or that Windows refuses: the look's
    // message, and the box opens again (the safety review of 2026-11-18, suspected 3).
    if bevel_core::rename_warning::bad_name(&typed) {
        kit::update::<FolderHost>(|h| h.form.view.rename_asking(true));
        let Some(place) = confirm_place(hwnd) else { return };
        ASKING.with(|a| a.set(hwnd));
        confirm::bad_name(place, || {
            let hub = HUB.with(Cell::get);
            // SAFETY: posting to our own window: the box opens again outside the message's own close.
            unsafe { PostMessageW(hub, WM_RENAME_ANSWER.id(), 0, 0) };
        });
        return;
    }
    let old = Path::new(&id);
    let old_name = old.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let new_name = file_ops::new_file_name(old, &shown, &typed, folder);
    if bevel_core::rename_warning::warns(&old_name, &new_name, folder, file_ops::known_type) {
        kit::update::<FolderHost>(|h| h.form.view.rename_asking(true));
        let Some((era, look)) = the_look() else { return };
        ASKING.with(|a| a.set(hwnd));
        let screen = work_area(Some(hwnd));
        confirm::ask_rename(look, era, Kind::Owned { owner: hwnd }, screen, by_keyboard, |yes| {
            let hub = HUB.with(Cell::get);
            // SAFETY: posting to our own window: the answer is handled outside the warning's own message.
            unsafe { PostMessageW(hub, WM_RENAME_ANSWER.id(), usize::from(yes), 0) };
        });
        return;
    }
    end_rename(hwnd);
}

/// The name is taken: Windows' file machinery renames the file (its own message comes for a name
/// it refuses or one that is taken); the icon keeps its place under its new name.
fn end_rename(hwnd: HWND) {
    let ended = kit::with::<FolderHost, _>(|h| {
        let target = h.form.view.renaming().map(|i| {
            let item = &h.form.view.items[i];
            Target { id: item.id.clone(), name: item.name.clone(), kind: kind_of(item) }
        });
        (h.form.view.end_rename(), target)
    });
    kit::update::<FolderHost>(|_| {});
    if let Some((RenameEnd::Rename { id, name }, Some(target))) = ended
        && target.id == id
    {
        let w = hwnd as usize;
        let typed = name.clone();
        file_ops::rename(target, name, move |new_id| {
            PENDING_RENAMED.lock().map(|mut p| p.push((w, id, new_id, typed))).ok();
            let hub = HUB_FOR_THREADS.load(Ordering::SeqCst);
            // SAFETY: posting to the program's own window.
            unsafe { PostMessageW(hub as HWND, WM_ITEMS.id(), 0, 0) };
        });
    }
}

static PENDING_RENAMED: Mutex<Vec<(usize, String, String, String)>> = Mutex::new(Vec::new());

/// The extension warning's answer: Yes renames; No opens the box again (the look's rule).
fn rename_answered(yes: bool) {
    let hwnd = ASKING.with(|a| a.replace(null_mut()));
    if hwnd.is_null() || !kit::windows::<FolderHost>().contains(&hwnd) {
        return;
    }
    kit::scope::<FolderHost, _>(hwnd, || {
        if yes {
            kit::update::<FolderHost>(|h| h.form.view.rename_asking(false));
            end_rename(hwnd);
        } else {
            kit::update::<FolderHost>(|h| h.form.view.rename_declined(bevel_core::rename_warning::keeps_typed_name(h.form.era)));
            bevel_winkit::util::take_focus(hwnd);
        }
    });
}

/// What the commands' threads left to do on the program's thread: select new shortcuts, give
/// renamed icons their new names.
fn pending_work() {
    let selects = PENDING_SELECT.lock().map(|mut p| std::mem::take(&mut *p)).unwrap_or_default();
    for (w, path) in selects {
        let hwnd = w as HWND;
        if kit::windows::<FolderHost>().contains(&hwnd) {
            kit::scope::<FolderHost, _>(hwnd, || kit::with::<FolderHost, _>(|h| h.select = Some(path)));
        }
    }
    let renamed = PENDING_RENAMED.lock().map(|mut p| std::mem::take(&mut *p)).unwrap_or_default();
    for (w, old, new, name) in renamed {
        let hwnd = w as HWND;
        if kit::windows::<FolderHost>().contains(&hwnd) {
            let mut changed = false;
            kit::scope::<FolderHost, _>(hwnd, || {
                kit::update::<FolderHost>(|h| {
                    let kept = h.form.view.items.iter().any(|i| i.id == old)
                        && (h.form.view.moved_ids().contains(&old) || h.form.view.last_order().contains(&old));
                    h.form.view.renamed(&old, &new, &name);
                    // A moved icon's place goes along with its new name (brief 105, 10).
                    if kept {
                        changed = remember_places(h);
                    }
                })
            });
            if changed {
                save();
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Menus.

/// Screen pixels of a point in the window (1×).
fn to_screen(hwnd: HWND, (x, y): (i32, i32), scale: bevel_core::scale::Scale) -> (i32, i32) {
    let mut r = RECT::default();
    // SAFETY: our own window; a valid out-pointer.
    unsafe { GetWindowRect(hwnd, &mut r) };
    (r.left + scale.at(x), r.top + scale.at(y))
}

/// Shows a menu of rows (with their submenus) and carries out what is chosen. `at`: where it
/// opens; `titles`: the menu bar's other titles (the pointer on one ends it). The status bar shows
/// the highlighted entry's help.
fn track_rows(hwnd: HWND, rows: Vec<menus::Row>, at: (i32, i32), above: Option<i32>, titles: &[(usize, RECT)], first: bool) -> menu::Ended {
    let Some((_, look)) = the_look() else {
        return menu::Ended { outcome: Outcome::Cancelled, pending: None, keyboard: false, rect: RECT::default(), layout: None };
    };
    let can_paste = file_ops::clipboard_has_files();
    let state = kit::with::<FolderHost, _>(|h| menu_state(h, can_paste));
    let Some(state) = state else {
        return menu::Ended { outcome: Outcome::Cancelled, pending: None, keyboard: false, rect: RECT::default(), layout: None };
    };
    let mut m = menus::menu(&rows);
    // Windows 2000 underlined access keys only once a key had been pressed.
    if state.era == Era::Win2000 {
        m.show_keys = kit::with::<FolderHost, _>(|h| h.form.bar.show_keys).unwrap_or(false);
    }
    if first {
        m.hover = (0..m.entries.len()).find(|&i| m.selectable(i, MenuStyle::Popup));
    }
    let subs: Vec<Option<Vec<menus::Row>>> = (0..rows.len()).map(|i| menus::opens(&rows, i).map(|s| menus::sub_rows(&state, s))).collect();
    let request = Request { owner: hwnd, menu: m, style: MenuStyle::Popup, at, above, titles };
    // What the menu was opened for: its command acts on these items (`reselect`).
    let held = held_selection();
    let help = |i: usize, sub: Option<usize>| {
        let text = match sub {
            Some(j) => subs.get(i).and_then(|s| s.as_ref()).and_then(|s| s.get(j)).map(|r| r.help.clone()),
            None => rows.get(i).map(|r| r.help.clone()),
        };
        let text = text.unwrap_or_default();
        kit::update::<FolderHost>(|h| h.form.help = Some(text));
    };
    let end = menu::track_cascade(&look, request, &|i| subs.get(i).and_then(|s| s.as_ref()).map(|s| menus::menu(s)), help);
    kit::update::<FolderHost>(|h| h.form.help = None);
    if let Some((i, sub)) = end.chosen {
        let command = match sub {
            Some(j) => subs.get(i).and_then(|s| s.as_ref()).and_then(|s| s.get(j)).and_then(|r| r.command),
            None => rows.get(i).and_then(|r| r.command),
        };
        // Only a command on the selection needs the items the menu was opened for (the fourth
        // safety review of 2026-11-18, C4: Details, Refresh, Up, Close were dropped too).
        if let Some(c) = command
            && c.runs_after_menu(|| reselect(&held))
        {
            run(hwnd, c, end.ended.keyboard);
        }
    }
    end.ended
}

/// The menu bar: title `first`'s menu, then the others as the arrows or the pointer go to them.
fn menu_bar(hwnd: HWND, first: usize) {
    let Some(scale) = kit::look_of::<FolderHost>().map(|l| l.scale) else { return };
    let mut title = first;
    // Opened by a click, a menu shows no entry highlighted until the pointer is on one; by a key
    // (Alt+letter, F10, the arrows) its first entry is (Windows' menus; found with real clicks).
    // SAFETY: a plain query of the button's state as this message saw it.
    let mut by_key = unsafe { windows_sys::Win32::UI::Input::KeyboardAndMouse::GetKeyState(i32::from(windows_sys::Win32::UI::Input::KeyboardAndMouse::VK_LBUTTON)) } >= 0;
    loop {
        let Some((rows, rects)) = kit::with::<FolderHost, _>(|h| {
            let state = menu_state(h, file_ops::clipboard_has_files());
            let rects: Vec<(usize, bevel_core::fb::Rect)> = (0..menus::titles(h.form.era).len()).filter_map(|i| h.form.title_rect(i).map(|r| (i, r))).collect();
            (menus::bar_rows(&state, title), rects)
        }) else {
            return;
        };
        kit::update::<FolderHost>(|h| {
            h.form.chosen = Some(title);
            h.form.help = Some(menus::title_help(h.form.era, title).to_string());
        });
        let screen = |r: bevel_core::fb::Rect| {
            let (x, y) = to_screen(hwnd, (r.x, r.y), scale);
            let (x1, y1) = to_screen(hwnd, (r.right(), r.bottom()), scale);
            RECT { left: x, top: y, right: x1, bottom: y1 }
        };
        let Some(&(_, anchor)) = rects.iter().find(|(i, _)| *i == title) else { return };
        let others: Vec<(usize, RECT)> = rects.iter().filter(|(i, _)| *i != title).map(|&(i, r)| (i, screen(r))).collect();
        let a = screen(anchor);
        let ended = track_rows(hwnd, rows, (a.left, a.bottom), Some(a.top), &others, by_key);
        let n = kit::with::<FolderHost, _>(|h| menus::titles(h.form.era).len()).unwrap_or(1);
        match ended.outcome {
            Outcome::Title(other) => {
                title = other;
                by_key = false;
            }
            Outcome::Next => {
                title = (title + 1) % n;
                by_key = true;
            }
            Outcome::Previous => {
                title = (title + n - 1) % n;
                by_key = true;
            }
            Outcome::Cancelled => {
                kit::update::<FolderHost>(|h| h.form.chosen = None);
                if let Some(msg) = ended.pending {
                    // A click on another title opens its menu; any other click does what it does.
                    if msg.hwnd == hwnd && msg.message == windows_sys::Win32::UI::WindowsAndMessaging::WM_LBUTTONDOWN {
                        let (x, y) = point(msg.lParam);
                        let other = rects.iter().find(|(_, r)| r.contains(scale.old(x), scale.old(y))).map(|&(i, _)| i);
                        match other {
                            Some(o) if o != title => {
                                title = o;
                                by_key = false;
                                continue;
                            }
                            Some(_) => return,
                            None => {}
                        }
                    }
                    menu::dispatch(&msg);
                }
                return;
            }
            _ => break,
        }
    }
    kit::update::<FolderHost>(|h| {
        h.form.chosen = None;
        h.form.help = None;
    });
}

/// The selected icons of the current window by their ids, as a menu opening now remembers them.
fn held_selection() -> Vec<String> {
    kit::with::<FolderHost, _>(|h| h.form.view.ids_of(&h.form.view.selection())).unwrap_or_default()
}

/// Before a menu's command: the selection is again exactly what the menu was opened for (items read
/// meanwhile may have moved it: a new item is selected as it shows). False, and the command is not
/// carried out, when any of those items is gone (the third safety review of 2026-11-18, A2).
fn reselect(held: &[String]) -> bool {
    let ok = kit::with::<FolderHost, _>(|h| h.form.view.select_ids(held)).unwrap_or(false);
    if ok {
        kit::update::<FolderHost>(|_| {});
    } else {
        log("folders: an item the menu was opened for is gone; its command is not carried out");
    }
    ok
}

/// The menu on the selected icons (the desktop's), at `menu_at`.
fn item_menu(hwnd: HWND) {
    let Some(scale) = kit::look_of::<FolderHost>().map(|l| l.scale) else { return };
    let Some((kinds, at, era)) = kit::with::<FolderHost, _>(|h| {
        let kinds: Vec<ItemKind> = h.form.view.selection().into_iter().map(|i| kind_of(&h.form.view.items[i])).collect();
        (kinds, h.form.menu_at, h.form.era)
    }) else {
        return;
    };
    if kinds.is_empty() {
        return;
    }
    let held = held_selection();
    let Some((_, look)) = the_look() else { return };
    let mut menu = bevel_core::desktop::item_menu(era, &kinds, false);
    // Tabs (brief 110): one folder chosen: "Open in New Tab" right under the entries that open
    // it (before the menu's first line).
    let new_tab_at = (TAB_SETTINGS.with(Cell::get).0 != tabs::Place::Off && matches!(kinds.as_slice(), [ItemKind::Folder]))
        .then(|| menu.entries.iter().position(|e| e.kind == bevel_core::paint::EntryKind::Separator).unwrap_or(menu.entries.len()));
    if let Some(k) = new_tab_at {
        menu.entries.insert(k, bevel_core::paint::MenuEntry::item("Open in New &Tab", None));
    }
    let kinds_at = |i: usize| match new_tab_at {
        Some(k) if i == k => None,
        Some(k) if i > k => bevel_core::desktop::item_at(&kinds, i - 1),
        _ => bevel_core::desktop::item_at(&kinds, i),
    };
    // Windows 95 underlined the access keys in every menu (the Reference PC's -c-rmenu-file).
    menu.show_keys = era.always_shows_keys();
    let at = to_screen(hwnd, at, scale);
    // The lit entry's words in the status bar, as the shell's menus showed them.
    let ended = menu::track_with_selection(&look, Request { owner: hwnd, menu, style: MenuStyle::Popup, at, above: None, titles: &[] }, |row| {
        let text = match row {
            Some(i) if Some(i) == new_tab_at => "Opens the selected folder in a new tab.",
            _ => row.and_then(kinds_at).map(|c| menus::item_help(era, c)).unwrap_or(""),
        };
        kit::update::<FolderHost>(|h| h.form.help = Some(text.to_string()));
    });
    kit::update::<FolderHost>(|h| h.form.help = None);
    match ended.outcome {
        Outcome::Chosen(i) if Some(i) == new_tab_at => {
            // The folder chosen, by its id (the menu's own: `held`).
            let into = kit::with::<FolderHost, _>(|h| {
                held.first().and_then(|id| h.form.view.index_of(id)).and_then(|i| h.form.view.items.get(i)).filter(|i| i.folder).map(|i| PathBuf::from(&i.id))
            })
            .flatten();
            if let Some(p) = into {
                new_tab(hwnd, Shows::Folder(p));
            }
        }
        Outcome::Chosen(i) => {
            if let Some(c) = kinds_at(i).map(Command::Shared)
                && c.runs_after_menu(|| reselect(&held))
            {
                run(hwnd, c, ended.keyboard);
            }
        }
        _ => {
            if let Some(msg) = ended.pending {
                menu::dispatch(&msg);
            }
        }
    }
}

/// The menu of the window's empty area, at `menu_at`.
fn background_menu(hwnd: HWND) {
    let Some(scale) = kit::look_of::<FolderHost>().map(|l| l.scale) else { return };
    let Some((rows, at)) = kit::with::<FolderHost, _>(|h| (menus::background_rows(&menu_state(h, file_ops::clipboard_has_files())), h.form.menu_at))
    else {
        return;
    };
    let at = to_screen(hwnd, at, scale);
    let ended = track_rows(hwnd, rows, at, None, &[], false);
    if let (Outcome::Cancelled, Some(msg)) = (ended.outcome, ended.pending) {
        menu::dispatch(&msg);
    }
}

// ---------------------------------------------------------------------------------------------
// The program's hidden window and its start.

/// A live test asks which icon is at a point of a folder window (window pixels): FNV-1a of its id.
fn id_code(id: &str) -> u32 {
    let mut h: u32 = 0x811C_9DC5;
    for b in id.bytes() {
        h ^= u32::from(b);
        h = h.wrapping_mul(0x0100_0193);
    }
    h.max(1)
}

unsafe extern "system" fn hub_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_COPYDATA if lparam != 0 => {
            // SAFETY: Windows hands a COPYDATASTRUCT valid for this message.
            let data = unsafe { &*(lparam as *const COPYDATASTRUCT) };
            if data.dwData == OPEN_FOLDER && !data.lpData.is_null() {
                // SAFETY: the sender's UTF-16 text of `cbData` bytes, copied by Windows.
                let text = unsafe { std::slice::from_raw_parts(data.lpData as *const u16, data.cbData as usize / 2) };
                let arg = String::from_utf16_lossy(text);
                // Opened once this message is answered (the sender waits for the answer).
                PENDING_OPEN.lock().map(|mut p| p.push(arg)).ok();
                // SAFETY: our own window.
                unsafe { PostMessageW(hwnd, WM_ITEMS.id(), 0, 1) };
                return 1;
            }
            if data.dwData == TEST_DROP && !data.lpData.is_null() {
                // SAFETY: as above.
                let text = unsafe { std::slice::from_raw_parts(data.lpData as *const u16, data.cbData as usize / 2) };
                return test_drop(&String::from_utf16_lossy(text)) as LRESULT;
            }
            0
        }
        m if m == WM_ITEMS.id() => {
            if lparam == 3 {
                disk_answered();
                return 0;
            }
            if lparam == 4 {
                address_failed();
                return 0;
            }
            if lparam == 6 {
                // A place read for a window: it opens, or its title follows.
                prepared_ready();
                return 0;
            }
            if lparam == 5 {
                // What was typed opened (a program, a file): the address shown comes back.
                let owner = wparam as HWND;
                if kit::windows::<FolderHost>().contains(&owner) {
                    kit::scope::<FolderHost, _>(owner, || {
                        kit::update::<FolderHost>(|h| {
                            let back = h.form.field.place_text.clone();
                            h.form.field.show(&back);
                            h.form.field.blur();
                        })
                    });
                }
                return 0;
            }
            if wparam != 0 {
                items_ready(wparam);
            }
            pending_work();
            open_pending();
            0
        }
        m if m == WM_ICON_AT.id() || m == WM_ICON_PLACE.id() => {
            let window = wparam as HWND;
            if !kit::windows::<FolderHost>().contains(&window) {
                return 0;
            }
            let Some(look) = kit::scope::<FolderHost, _>(window, kit::look_of::<FolderHost>) else { return 0 };
            let scale = look.scale;
            let answer = kit::scope::<FolderHost, _>(window, || {
                kit::with::<FolderHost, _>(|h| {
                    let v = &h.form.view;
                    if msg == WM_ICON_AT.id() {
                        let (x, y) = (scale.old((lparam & 0xFFFF) as i32), scale.old(((lparam >> 16) & 0xFFFF) as i32));
                        return v.hit(&look.regular, x, y).map(|i| id_code(&v.items[i].id) as LRESULT);
                    }
                    let i = v.items.iter().position(|it| id_code(&it.id) as LPARAM == lparam)?;
                    let r = v.icon_rect(i);
                    Some(scale.at(r.x + r.w / 2) as LRESULT | ((scale.at(r.y + r.h / 2) as LRESULT) << 16))
                })
                .flatten()
            });
            answer.unwrap_or(0)
        }
        m if m == WM_TREE_ROWS.id() => {
            tree_rows_ready();
            0
        }
        m if m == WM_PICTURES.id() => {
            pictures_ready();
            0
        }
        m if m == WM_UNREADABLE.id() => {
            if let Some(hwnd) = window_of(wparam) {
                let letter = char::from_u32((lparam & 0xFFFF) as u32).unwrap_or('A');
                if lparam & LOCKED_FLAG != 0 {
                    // Only what this program's own reader recorded for that window (the review of
                    // 2026-11-21, C1): from any other sender, nothing.
                    if take_locked_told(wparam, letter) {
                        locked_found(hwnd, letter);
                    } else {
                        log("folders: a message about a locked drive that no reader of this program sent; nothing done");
                    }
                } else {
                    ask_drive(hwnd, letter, None);
                }
            }
            0
        }
        WM_TIMER if wparam == TREE_TIMER => {
            tree_tick();
            patience();
            publish_wanted();
            check_front();
            slow_renames();
            0
        }
        m if m == WM_TEST_COMMAND.id() => {
            test_command(wparam as HWND, lparam as u64);
            0
        }
        m if m == WM_RENAME_ANSWER.id() => {
            rename_answered(wparam == 1);
            0
        }
        WM_TIMER if wparam == WATCH_TIMER => {
            bevel_winkit::log::watch_gui_resources();
            0
        }
        // Something else went onto the clipboard: no icon shows cut any more.
        WM_CLIPBOARDUPDATE => {
            // SAFETY: a plain query.
            let seq = unsafe { GetClipboardSequenceNumber() };
            let stale = CUT.with(|c| {
                let c = c.borrow();
                !c.0.is_empty() && c.1 != seq
            });
            if stale {
                CUT.with(|c| c.borrow_mut().0.clear());
                show_cut(Vec::new());
            }
            0
        }
        // SAFETY: default handling for everything else.
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

static PENDING_OPEN: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// What the program is asked to open: Windows Explorer at the look's own start (`EXPLORER`), an
/// Explorer window on a place (`EXPLORE` and the place), or a folder's own window.
const EXPLORER: &str = "::explorer";
const EXPLORE: &str = "::explore:";

fn open_arg(arg: &str) {
    // Whether it is a folder (and which, for Explorer's start) is read off this thread (`ask`).
    ask(Ask::Arg(arg.to_string()));
}

/// The folders handed over and not asked for yet (their message still queued) are asked for now.
fn open_pending() {
    let opens = PENDING_OPEN.lock().map(|mut p| std::mem::take(&mut *p)).unwrap_or_default();
    for arg in opens {
        open_arg(&arg);
    }
}

/// Whether a folder window is still to open: handed over (`PENDING_OPEN`) or asked for and being
/// read (`ASKED`), its window not opened yet (`still_to_open`: within `PATIENCE`, the sixth safety
/// review of 2026-11-18, C3).
fn waiting_to_open() -> bool {
    PENDING_OPEN.lock().is_ok_and(|p| !p.is_empty()) || ASKED.with(|a| still_to_open(&a.borrow()))
}

/// What the program does once its last window has closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AfterLast {
    /// A folder window is still to open: it waits for it.
    Open,
    /// File jobs run: it stays without a window until they end (or a window is asked for).
    WaitForJobs,
    /// Nothing is left: it ends.
    End,
}

/// The fifth safety review of 2026-11-18, C1: the program never ends while a folder handed over
/// or asked for is still to open; it looked only at the windows asked for, and after its file
/// jobs only at open windows, so a folder handed over in that moment was never opened.
fn after_last_window(waiting_to_open: bool, jobs_run: bool) -> AfterLast {
    if waiting_to_open {
        AfterLast::Open
    } else if jobs_run {
        AfterLast::WaitForJobs
    } else {
        AfterLast::End
    }
}

/// Whether `p` is named as a live test's own items are ("Bevel95 test ...").
fn test_named(p: &Path) -> bool {
    p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("Bevel95 test"))
}

/// A live test's drop from "another program" (`TEST_DROP`): "offered\twindow\tfile\tfile...", into
/// the window's empty area. Only into a test folder's window and only test items in test folders;
/// anything else is refused (0) and logged. Returns what the source would be told.
fn test_drop(text: &str) -> u32 {
    let mut parts = text.split('\t');
    let offered = parts.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
    let window = parts.next().and_then(|s| s.parse::<usize>().ok()).unwrap_or(0) as HWND;
    let files: Vec<String> = parts.map(str::to_string).collect();
    let test_name = |p: &Path| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("Bevel95 test"));
    let files_ok = !files.is_empty() && files.iter().all(|f| test_name(Path::new(f)) && Path::new(f).parent().is_some_and(test_name));
    let window_ok = kit::windows::<FolderHost>().contains(&window)
        && kit::scope::<FolderHost, _>(window, || kit::with::<FolderHost, _>(|h| matches!(&h.shows, Shows::Folder(p) if test_name(p)))).unwrap_or(false);
    if !files_ok || !window_ok {
        log("folders: a test drop was refused (only test items into a test folder's window)");
        return 0;
    }
    let mut r = RECT::default();
    // SAFETY: our own window and a valid out-pointer.
    unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetClientRect(window, &mut r) };
    let (x, y) = (r.right * 3 / 4, r.bottom * 3 / 4);
    let told = bevel_winkit::dragdrop::test_drop(window, folder_drop, files, offered, x, y);
    log(&format!("folders: a test drop (offered {offered}) answered {told}"));
    told
}

/// A test folder's window: a folder named "Bevel95 test ..." (never My Computer or the Desktop).
fn is_test_folder(shows: &Shows) -> bool {
    match shows {
        Shows::Folder(p) => p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("Bevel95 test")),
        Shows::MyComputer | Shows::Desktop => false,
    }
}

/// The live tests' commands that only open a menu or read something (a menu of the menu bar, the
/// menu on an item or on Back, the address bar's list, Back's list of places): they change
/// nothing, so they may be used outside a test folder too (the menus on My Computer's drives).
fn only_reads(code: usize) -> bool {
    (TEST_MENU..TEST_MENU + 4).contains(&code) || matches!(code, TEST_ITEM_MENU | TEST_BACK_MENU | TEST_LIST | TEST_PLACES)
}

/// Whether a live test's command `code` is taken in a window (`test_folder`: a test folder's),
/// on an item (`item`: Some(whether it is a test item, "Bevel95 test ...")) or on none. Every
/// command that changes anything acts only in a test folder's window, on a test item when it
/// names one; outside a test folder (My Computer, the Desktop place, an ordinary folder) only
/// the commands that open a menu or read something (`only_reads`; brief 105, 5; brief 106, 5).
fn test_allowed(code: usize, test_folder: bool, item: Option<bool>) -> bool {
    match item {
        None => test_folder || only_reads(code),
        Some(test_item) => (test_folder && test_item) || (!test_folder && only_reads(code)),
    }
}

/// A live test's command (`WM_TEST_COMMAND`), only on test items in test folders.
fn test_command(window: HWND, l: u64) {
    if !kit::windows::<FolderHost>().contains(&window) {
        return;
    }
    let (code, item) = ((l & 0xFFFF) as usize, (l >> 32) as u32);
    kit::scope::<FolderHost, _>(window, || {
        let ok = kit::with::<FolderHost, _>(|h| {
            let test_folder = is_test_folder(&h.shows);
            let v = &mut h.form.view;
            if item == 0 {
                v.select_only(None);
                return test_allowed(code, test_folder, None);
            }
            if (TEST_TREE_TOGGLE..=TEST_EXPLORE).contains(&code) {
                return false;
            }
            match v.items.iter().position(|i| id_code(&i.id) == item) {
                Some(i) if test_allowed(code, test_folder, Some(v.items[i].name.starts_with("Bevel95 test"))) => {
                    v.select_only(Some(i));
                    let r = v.icon_rect(i);
                    h.form.menu_at = (r.x + r.w / 2, r.y + r.h / 2);
                    true
                }
                _ => false,
            }
        })
        .unwrap_or(false);
        if !ok {
            log("folders: a test command was refused (not a test item or a test folder)");
            return;
        }
        kit::update::<FolderHost>(|_| {});
        // A Paste, Paste Shortcut or Undo of a test takes test items only, also from a menu the
        // test opened (the third safety review of 2026-11-18, C4).
        file_ops::for_a_test(|| match code {
            TEST_UP => run(window, Command::Up, true),
            TEST_INTO => {
                let into = kit::with::<FolderHost, _>(|h| {
                    h.form.view.items.iter().find(|i| i.folder && i.name.starts_with("Bevel95 test")).map(|i| PathBuf::from(&i.id))
                })
                .flatten();
                match into {
                    Some(p) => navigate(window, Some(Shows::Folder(p)), Go::New),
                    None => log("folders: no test folder to go into"),
                }
            }
            TEST_TREE_TOGGLE | TEST_TREE_DOWN | TEST_TREE_UP => {
                kit::update::<FolderHost>(|h| {
                    let page = h.form.tree_page();
                    match code {
                        TEST_TREE_TOGGLE => {
                            let sel = h.form.tree.sel;
                            h.form.tree.toggle(sel, page);
                        }
                        TEST_TREE_DOWN => {
                            h.form.tree.key(pane::TreeKey::Down, page);
                            h.settle = Some(std::time::Instant::now() + SETTLE);
                        }
                        _ => {
                            h.form.tree.key(pane::TreeKey::Up, page);
                            h.settle = Some(std::time::Instant::now() + SETTLE);
                        }
                    }
                    h.form.tree_selection_shown();
                });
            }
            TEST_TREE_MENU => {
                // Only on a test folder's row: its entries are chosen by keys the test posts.
                let on_test_row = kit::with::<FolderHost, _>(|h| {
                    let sel = h.form.tree.sel;
                    let test = h.form.tree.nodes.get(sel).is_some_and(|n| n.kind == NodeKind::Folder && test_named(Path::new(&n.id)));
                    if test {
                        h.form.menu_on_row(sel);
                        h.form.menu_at = (60, 120);
                    }
                    test
                })
                .unwrap_or(false);
                if on_test_row {
                    tree_menu(window);
                } else {
                    log("folders: a test's tree menu was refused (not on a test folder's row)");
                }
            }
            TEST_TREE_SHOW => {
                let on = kit::with::<FolderHost, _>(|h| !h.form.tree_shown).unwrap_or(false);
                set_tree(window, on);
            }
            TEST_EXPLORE => {
                let into = kit::with::<FolderHost, _>(|h| {
                    h.form.view.items.iter().find(|i| i.folder && i.name.starts_with("Bevel95 test")).map(|i| PathBuf::from(&i.id))
                })
                .flatten();
                match into {
                    Some(p) => open_explorer(Shows::Folder(p)),
                    None => log("folders: no test folder to explore"),
                }
            }
            c if (TEST_VIEW..TEST_VIEW + 4).contains(&c) => {
                run(window, Command::View(bevel_core::desktop_icons::ViewMode::ALL[c - TEST_VIEW]), true);
            }
            c if (TEST_HEADER..TEST_HEADER + 4).contains(&c) => FolderHost::activate(Part::Header(c - TEST_HEADER)),
            TEST_SELECT => {}
            TEST_WEB_TOGGLE => run(window, Command::WebView, true),
            TEST_SHOW_FILES => {
                kit::update::<FolderHost>(|h| h.form.show_files());
                FolderHost::activate(Part::WebLink(Link::ShowFiles));
            }
            TEST_SELECT_ALL => run(window, Command::SelectAll, true),
            TEST_FOLDER_OPTIONS => run(window, Command::FolderOptions, true),
            TEST_SLOW_READING => {
                SLOW_READING.store(true, Ordering::SeqCst);
                log("folders: a test made test folders' readings slow");
            }
            // Tabs (brief 110): the selected test folder in a new tab; the next tab; the tab in
            // front closed (all in a test folder's window only: `test_allowed`).
            TEST_NEW_TAB => {
                let into = kit::with::<FolderHost, _>(|h| {
                    let v = &h.form.view;
                    v.selection().first().and_then(|&i| v.items.get(i)).filter(|i| i.folder && i.name.starts_with("Bevel95 test")).map(|i| PathBuf::from(&i.id))
                })
                .flatten();
                match into {
                    Some(p) => new_tab(window, Shows::Folder(p)),
                    None => log("folders: no test folder selected for a new tab"),
                }
            }
            TEST_TAB_NEXT => step_tab(window, true),
            TEST_TAB_CLOSE => FolderHost::activate(Part::TabClose),
            TEST_MOVE => {
                let before = saved_text();
                kit::update::<FolderHost>(|h| {
                    let selected = h.form.view.selection();
                    let ids = h.form.view.ids_of(&selected);
                    if ids.is_empty() {
                        return;
                    }
                    h.form.view.move_ids(&ids, TEST_MOVE_BY);
                    remember_places(h);
                });
                save_if_changed(&before);
            }
            TEST_BACK => run(window, Command::Back, true),
            TEST_FORWARD => run(window, Command::Forward, true),
            TEST_LIST => toggle_list(),
            TEST_PLACES => places_menu(window, Tool::Back),
            c if (TEST_MENU..TEST_MENU + 4).contains(&c) => FolderHost::activate(Part::Menu(c - TEST_MENU)),
            TEST_ITEM_MENU => FolderHost::activate(Part::ItemMenu),
            TEST_BACK_MENU => {
                kit::with::<FolderHost, _>(|h| h.form.menu_at = (40, 60));
                FolderHost::activate(Part::BackgroundMenu)
            }
            c => match D::from_code(c) {
                // Undo as Edit > Undo does it (the folder windows' own command).
                Some(D::Undo) => run(window, Command::Undo, true),
                Some(c) => run(window, Command::Shared(c), true),
                None => log("folders: an unknown test command"),
            },
        })
    });
}

/// How long a start tries to hand its folder over to a running folder windows' program that does
/// not take it, in all, before it becomes a second program itself (the fourth safety review of
/// 2026-11-18, B3: it was 60 s, and every later start waited the 60 s again for the same program
/// that did not answer).
const HAND_OVER_PATIENCE: Duration = Duration::from_secs(10);
/// How long one hand-over may take (SMTO_ABORTIFHUNG ends it at once for a program Windows calls
/// hung): the last program of a round; one with others after it gets `HAND_OVER_QUICK`, so a start
/// reaches a second program that answers at once while the holder does not (a suspended holder kept
/// every later start 2 s: tried live).
const HAND_OVER_TRY: Duration = Duration::from_secs(2);
const HAND_OVER_QUICK: Duration = Duration::from_millis(300);
/// How long a start waits for another start that is just making its hidden window.
const HUB_COMING: Duration = Duration::from_secs(2);

/// What became of a start's folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HandedTo<H> {
    /// That running program took it (this start ends).
    Program(H),
    /// None took it within the patience, or none runs: this start becomes the program.
    Nobody { waited_for_one: bool },
}

/// A start hands its folder to ANY running folder windows' program that takes it: the one holding
/// the hub's place first, then the second programs already running (`find` gives them in that
/// order, each time anew); round after round, until one takes it or `HAND_OVER_PATIENCE` has
/// passed. A program just starting (`existed`: the programs' mutex was there, no hidden window
/// yet) is waited for a moment. `try_one` hands over and says whether it was taken (given the
/// time it may take); `now` is the time since the start; `pause` waits between rounds.
fn hand_over_to<H: Copy>(
    mut find: impl FnMut() -> Vec<H>,
    mut try_one: impl FnMut(H, Duration) -> bool,
    mut now: impl FnMut() -> Duration,
    mut pause: impl FnMut(Duration),
    existed: bool,
) -> HandedTo<H> {
    let mut waited_for_one = false;
    loop {
        let hubs = find();
        for (i, &hub) in hubs.iter().enumerate() {
            let left = HAND_OVER_PATIENCE.saturating_sub(now());
            let most = if i + 1 < hubs.len() { HAND_OVER_QUICK } else { HAND_OVER_TRY };
            if try_one(hub, left.clamp(Duration::from_millis(100), most)) {
                return HandedTo::Program(hub);
            }
        }
        let waited = now();
        if !hubs.is_empty() && waited < HAND_OVER_PATIENCE {
            waited_for_one = true;
            pause(Duration::from_millis(250));
        } else if hubs.is_empty() && existed && waited < HUB_COMING {
            pause(Duration::from_millis(100));
        } else {
            return HandedTo::Nobody { waited_for_one: waited_for_one || !hubs.is_empty() };
        }
    }
}
/// This program holds the hub's place (`take_hub_place`): it saves folders.txt.
static HUB_HOLDER: AtomicBool = AtomicBool::new(true);

/// Takes the hub's place (a named mutex owned for the program's life; one left by a program that
/// ended is taken over). False when another program holds it: then this one is a second folder
/// windows' program (a start that gave up waiting), which never saves folders.txt and keeps its
/// hidden window under another class: later starts try the holder first, then it.
fn take_hub_place() -> bool {
    use windows_sys::Win32::Foundation::{WAIT_ABANDONED, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::WaitForSingleObject;
    // One holder per copy (the fifth safety review of 2026-11-18, B1).
    let name = wide(&bevel_winkit::copy::name("Local\\Bevel95.Folders.Hub"));
    // SAFETY: a null-terminated name; the handle is kept open on purpose for the program's life.
    unsafe {
        let place = CreateMutexW(null(), 0, name.as_ptr());
        if place.is_null() {
            log_win32("CreateMutexW(the folder windows' hub place)");
            return true;
        }
        matches!(WaitForSingleObject(place, 0), WAIT_OBJECT_0 | WAIT_ABANDONED)
    }
}

/// The hidden window's class for a second program (`take_hub_place` false).
const HUB_CLASS_SECOND: &str = "Bevel95FoldersSecond";

/// The kind of the folder windows' program in its mark of its copy (`copy::mark`): only a program
/// of the same copy is handed a folder (the fifth safety review of 2026-11-18, B1: any copy's
/// program took it, and kept that window's places and Undo steps in its own copy's folder). The
/// hidden windows' classes and their empty names stay as they were, so the live-test scripts that
/// look for them still find them.
const COPY_MARK: &str = "Folders";

/// The running programs' hidden windows of this copy: the hub's holder first, then every second
/// program. A window whose process does not hold this copy's mark is another copy's.
fn running_hubs() -> Vec<HWND> {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;
    let mut out = Vec::new();
    for class in [HUB_CLASS, HUB_CLASS_SECOND] {
        let class = wide(class);
        let mut after: HWND = null_mut();
        loop {
            // SAFETY: a null-terminated class name; message-only windows are found under
            // HWND_MESSAGE; `after` is null or a window this loop just found.
            after = unsafe { FindWindowExW(HWND_MESSAGE, after, class.as_ptr(), null()) };
            if after.is_null() {
                break;
            }
            let mut pid = 0;
            // SAFETY: any window handle and a valid out-pointer.
            let ours = unsafe { GetWindowThreadProcessId(after, &mut pid) } != 0 && bevel_winkit::copy::is_of_this_copy(COPY_MARK, pid);
            if ours {
                out.push(after);
            }
        }
    }
    out
}

/// Hands `arg` to the running program, waiting at most `limit` (at once for a hung one). True if
/// it took it.
fn hand_over(hub: HWND, arg: &str, limit: Duration) -> bool {
    let text: Vec<u16> = arg.encode_utf16().collect();
    let data = COPYDATASTRUCT { dwData: OPEN_FOLDER, cbData: (text.len() * 2) as u32, lpData: text.as_ptr() as *mut _ };
    // A program Windows already calls hung is not waited for at all.
    // SAFETY: any window handle.
    if unsafe { windows_sys::Win32::UI::WindowsAndMessaging::IsHungAppWindow(hub) } != 0 {
        return false;
    }
    let mut answer = 0usize;
    let ms = limit.as_millis().min(u128::from(u32::MAX)) as u32;
    // The running program may bring its window forward: this start has that leave (it was
    // started by the program the person clicked in) and passes it on, as the Help viewer's start
    // does (brief 90: a desktop double-click on a folder opened its window behind).
    let mut pid = 0;
    // SAFETY: any window handle and a valid out-pointer; a plain call with that process's id.
    unsafe {
        if windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(hub, &mut pid) != 0 && pid != 0 {
            windows_sys::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow(pid);
        }
    }
    // SAFETY: the data lives through the call; the answer waits at most `limit`.
    let sent = unsafe { SendMessageTimeoutW(hub, WM_COPYDATA, 0, &data as *const _ as LPARAM, SMTO_ABORTIFHUNG, ms, &mut answer) };
    sent != 0 && answer == 1
}

fn make_hub() -> Option<HWND> {
    // A second program's hidden window is found by later starts after the holder's (`running_hubs`).
    let name = wide(if HUB_HOLDER.load(Ordering::SeqCst) { HUB_CLASS } else { HUB_CLASS_SECOND });
    let class = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(hub_proc),
        // SAFETY: our own module.
        hInstance: unsafe { GetModuleHandleW(null()) },
        lpszClassName: name.as_ptr(),
        ..Default::default()
    };
    // SAFETY: a fully initialized class; a message-only window of it.
    let hwnd = unsafe {
        if RegisterClassExW(&class) == 0 {
            log_win32("RegisterClassExW(folders)");
        }
        CreateWindowExW(0, name.as_ptr(), null(), 0, 0, 0, 0, 0, HWND_MESSAGE, null_mut(), GetModuleHandleW(null()), null())
    };
    if hwnd.is_null() {
        log_win32("CreateWindowExW(folders' hidden window)");
        return None;
    }
    // SAFETY: our own window: the resource watch every ten seconds, the clipboard's changes.
    unsafe {
        SetTimer(hwnd, WATCH_TIMER, 10_000, None);
        SetTimer(hwnd, TREE_TIMER, 100, None);
        AddClipboardFormatListener(hwnd);
    }
    Some(hwnd)
}

fn main() {
    // Where this run keeps its data, decided once, before anything is read or logged.
    let Some(args) = bevel_winkit::log::program_start() else { return };
    // Never Bevel95's own folder as the working folder (decisions.md, "Folder windows").
    bevel_winkit::shell::start_in_profile();
    bevel_winkit::log::started();
    // A panic on a worker thread closes the windows: none is left looking alive (decisions.md).
    bevel_winkit::panic::install(bevel_winkit::panic::close_windows_after);
    look::dpi_aware();
    // A drive without a disk answers with an error instead of Windows' "insert a disk" box.
    // SAFETY: a plain call.
    unsafe { SetErrorMode(SEM_FAILCRITICALERRORS) };
    // No argument: Windows Explorer (the Programs menu's entry); "--explore <folder>": Explore.
    let arg = match args.as_slice() {
        [] => EXPLORER.to_string(),
        [flag, place, ..] if flag == "--explore" => format!("{EXPLORE}{place}"),
        [one, ..] => one.clone(),
    };
    // One program for every folder window: a second start hands its folder over and ends.
    // One per copy (the fifth safety review of 2026-11-18, B1).
    let mutex_name = wide(&bevel_winkit::copy::name("Local\\Bevel95.Folders"));
    // SAFETY: a null-terminated name; the handle lives as long as the process.
    let _mutex = unsafe { CreateMutexW(null(), 0, mutex_name.as_ptr()) };
    // SAFETY: a plain query of the error the mutex call left.
    let existed = unsafe { windows_sys::Win32::Foundation::GetLastError() } == windows_sys::Win32::Foundation::ERROR_ALREADY_EXISTS;
    let started = std::time::Instant::now();
    let said = Cell::new(false);
    let handed = hand_over_to(
        running_hubs,
        |hub, limit| {
            let taken = hand_over(hub, &arg, limit);
            if !taken && !said.replace(true) {
                log("folders: a running folder windows' program did not take the folder; trying the others and again");
            }
            taken
        },
        || started.elapsed(),
        std::thread::sleep,
        existed,
    );
    match handed {
        HandedTo::Program(_) => return,
        HandedTo::Nobody { waited_for_one: true } => log(&format!(
            "folders: no running folder windows' program took the folder in {} ms; this one opens it as a second program, \
             without saving folders.txt, and later starts can hand over to it",
            started.elapsed().as_millis()
        )),
        HandedTo::Nobody { waited_for_one: false } => {}
    }
    // This process is a folder windows' program of this copy: later starts of this copy (and only
    // they) hand their folders to its hidden window (`running_hubs`). Held for the program's life.
    let _copy_mark = bevel_winkit::copy::mark(COPY_MARK);
    // The hub's place: only its holder saves folders.txt and takes later hand-overs (the third
    // safety review of 2026-11-18, C5: two programs each saved their own picture of it).
    let holder = take_hub_place();
    HUB_HOLDER.store(holder, Ordering::SeqCst);
    let (era, look) = look::load();
    bevel_winkit::com::init();
    // Windows' drag and drop for files between the windows and other programs.
    bevel_winkit::dragdrop::init();
    ERA_LOOK.with(|l| *l.borrow_mut() = Some((era, look)));
    TAB_SETTINGS.with(|t| t.set(look::folder_tabs()));
    file_ops::set_hooks("folders", || {});
    // Unreadable: logged, kept as it is and not saved over (`state_file`).
    let saved = bevel_winkit::state_file::load(&saved_path()).text().map(|t| Saved::parse(&t)).unwrap_or_default();
    SAVED.with(|s| *s.borrow_mut() = saved);
    let Some(hub) = make_hub() else { return };
    HUB.with(|h| h.set(hub));
    HUB_FOR_THREADS.store(hub as usize, Ordering::SeqCst);
    log("folders: started");
    bevel_winkit::log::watch_gui_resources();
    open_arg(&arg);
    loop {
        // The first window opens once its place is read (`ask`): until then there is none.
        wait_for_asked();
        kit::run_until_closed::<FolderHost>();
        // A folder handed over while the last window closed (its message still queued) is asked
        // for now, so the check below sees it.
        open_pending();
        // The last window closed while another one asked for is still being read: it comes. A
        // copy, move, rename, delete or Undo still running (with Windows' progress window and
        // questions, which belong to it): the program stays without a window until they end, as
        // Explorer does (the safety review of 2026-11-18, A2). A Properties window closes with the
        // program (the second review of 2026-11-18, B1). A folder window asked for meanwhile
        // opens as usual (the hidden window still takes the hand-over), and the program never
        // ends while one is still to open (the fifth review, C1: it ended with it unread).
        match after_last_window(waiting_to_open(), bevel_winkit::jobs::busy()) {
            AfterLast::Open => continue,
            AfterLast::End => break,
            AfterLast::WaitForJobs => {}
        }
        log(&format!(
            "folders: the last window closed while {} file job(s) run; staying until they end",
            bevel_winkit::jobs::running()
        ));
        if !bevel_winkit::jobs::wait_while_busy(|| !kit::windows::<FolderHost>().is_empty() || waiting_to_open()) {
            // The jobs ended; a folder handed over in the last moment is still opened.
            open_pending();
            if waiting_to_open() {
                log("folders: the file jobs ended; a folder asked for meanwhile still opens");
                continue;
            }
            log("folders: the file jobs ended");
            break;
        }
        log("folders: a folder window opened again, or was asked for, meanwhile");
    }
    // SAFETY: our own window and timer.
    unsafe {
        KillTimer(hub, WATCH_TIMER);
        KillTimer(hub, TREE_TIMER);
        RemoveClipboardFormatListener(hub);
        DestroyWindow(hub);
    }
    log("folders: the last window closed; ended");
}

#[cfg(test)]
mod tests {
    /// `TEST_MOVE` changes the remembered layout: it is refused in My Computer, in the Desktop
    /// place and in an ordinary folder, on any item, and taken only on a test item in a test
    /// folder (brief 105, 5). Menus on My Computer's drives stay possible.
    #[test]
    fn a_test_move_reaches_only_a_test_item_in_a_test_folder() {
        use super::{Shows, TEST_ITEM_MENU, TEST_MOVE, is_test_folder, test_allowed};
        use std::path::PathBuf;
        let ordinary = Shows::Folder(PathBuf::from(r"C:\Some folder"));
        let test = Shows::Folder(PathBuf::from(r"C:\Bevel95 test folder"));
        for shows in [Shows::MyComputer, Shows::Desktop, ordinary] {
            let folder = is_test_folder(&shows);
            assert!(!folder);
            for item in [None, Some(false), Some(true)] {
                assert!(!test_allowed(TEST_MOVE, folder, item));
            }
        }
        assert!(is_test_folder(&test));
        assert!(test_allowed(TEST_MOVE, true, Some(true)));
        assert!(!test_allowed(TEST_MOVE, true, Some(false)));
        assert!(test_allowed(TEST_ITEM_MENU, is_test_folder(&Shows::MyComputer), Some(false)));
    }

    /// Brief 106, 5: every code. Outside a test folder (My Computer, the Desktop place, an
    /// ordinary folder) only the menus and the lists get through, on any item or none; View, the
    /// header's sort, the web view switch, Select All, Folder Options, the slow readings and
    /// every file command are refused there. In a test folder everything acts, on a test item
    /// when it names one.
    #[test]
    fn test_commands_that_change_anything_act_only_in_a_test_folder() {
        use super::*;
        let reads = [TEST_MENU, TEST_MENU + 1, TEST_MENU + 2, TEST_MENU + 3, TEST_ITEM_MENU, TEST_BACK_MENU, TEST_LIST, TEST_PLACES];
        let changes = [
            TEST_UP,
            TEST_INTO,
            TEST_BACK,
            TEST_FORWARD,
            TEST_TREE_TOGGLE,
            TEST_TREE_SHOW,
            TEST_EXPLORE,
            TEST_VIEW,
            TEST_VIEW + 3,
            TEST_HEADER,
            TEST_HEADER + 2,
            TEST_SELECT,
            TEST_WEB_TOGGLE,
            TEST_SHOW_FILES,
            TEST_SELECT_ALL,
            TEST_FOLDER_OPTIONS,
            TEST_SLOW_READING,
            TEST_MOVE,
        ];
        let ordinary = Shows::Folder(std::path::PathBuf::from(r"C:\Some folder"));
        for code in 0..=300usize {
            for shows in [&Shows::MyComputer, &Shows::Desktop, &ordinary] {
                let folder = is_test_folder(shows);
                for item in [None, Some(false), Some(true)] {
                    assert_eq!(test_allowed(code, folder, item), reads.contains(&code), "code {code} outside a test folder, item {item:?}");
                }
            }
            // In a test folder: every code on the window or a test item, nothing on another item.
            assert!(test_allowed(code, true, None) && test_allowed(code, true, Some(true)), "code {code}");
            assert!(!test_allowed(code, true, Some(false)), "code {code} on an item not a test item");
        }
        for code in changes {
            assert!(!reads.contains(&code), "{code} changes something");
        }
    }

    /// A window's reader started again because the look changed never tells that its drive cannot
    /// be read (no message, no unlock window); the person's own doing tells it once.
    #[test]
    fn only_the_persons_doing_tells_an_unreadable_drive() {
        use super::{StartedBy, Unreadable};
        // Opened (or Refresh, or navigated): told at the first reading, not again.
        let mut opened = Unreadable::new(StartedBy::Person);
        assert!(opened.tell());
        assert!(!opened.tell());
        // Then the look changed: a new reader for the same window says nothing, ever.
        let mut again = Unreadable::new(StartedBy::LookChange);
        assert!(!again.tell());
        assert!(!again.tell());
        // Refresh afterwards is the person's doing again.
        assert!(Unreadable::new(StartedBy::Person).tell());
    }

    /// The look changed, then Refresh: what each earlier reader of the window still delivers (a
    /// snapshot, a batch of pictures) is dropped; the newest reader's comes through, in whichever
    /// order they arrive.
    #[test]
    fn an_earlier_readers_deliveries_are_dropped() {
        use super::{Generations, take_current};
        let mut g = Generations::default();
        let opened = g.start(7);
        let other_window = g.start(8);
        let after_look = g.start(7);
        assert!(!g.is_current(7, opened) && g.is_current(7, after_look) && g.is_current(8, other_window));
        // The old reader's snapshot lands after the new one's: only the new one is taken.
        let mut all = vec![(7, after_look, "new look"), (8, other_window, "window 8"), (7, opened, "old look")];
        assert_eq!(take_current(&mut all, 7, |x| g.is_current(7, x)), vec!["new look"]);
        assert_eq!(all, vec![(8, other_window, "window 8")], "the other window's stays");
        // Refresh: the look change's reader is earlier now.
        let refreshed = g.start(7);
        let mut all = vec![(7, after_look, "look"), (7, refreshed, "refresh")];
        assert_eq!(take_current(&mut all, 7, |x| g.is_current(7, x)), vec!["refresh"]);
        let mut late = vec![(7, after_look, "late pictures")];
        assert!(take_current(&mut late, 7, |x| g.is_current(7, x)).is_empty(), "a late batch of pictures goes nowhere");
        // A closed window's readers are all earlier ones.
        g.forget(7);
        assert!(!g.is_current(7, refreshed));
    }

    /// A drive whose last reading found it locked by BitLocker gets Unlock Drive... on its menu,
    /// and only while that is so; for Open the reading must be recent. (Letters no PC here has, so
    /// no real drive is asked; nothing is started.)
    #[test]
    fn a_drive_found_locked_has_unlock_drive_until_it_is_found_unlocked() {
        use super::{Shows, drive_letter_of, kind_of, known_locked, note_lock, root_letter};
        use bevel_core::desktop::{DesktopCommand, ItemKind, item_entries};
        use bevel_core::desktop_art::Art;
        use bevel_core::folder::art::DriveKind;
        use bevel_core::desktop_icons::{Item, Picture};
        let drive = |id: &str| Item {
            id: id.into(),
            name: String::new(),
            picture: Picture::Art(Art::Drive(DriveKind::Fixed)),
            shortcut: false,
            folder: false,
            type_name: String::new(),
            size: 0,
            modified: 0,
        };
        assert_eq!(root_letter("q:\\"), Some('Q'));
        assert_eq!(root_letter("Q:\\x"), None);
        assert_eq!(root_letter("::my-computer"), None);
        assert_eq!(drive_letter_of(&Shows::Folder("R:\\".into())), Some('R'));
        assert_eq!(drive_letter_of(&Shows::Folder("R:\\a".into())), None);
        assert_eq!(drive_letter_of(&Shows::MyComputer), None);
        // Never read: a drive like any other.
        assert_eq!(kind_of(&drive("Q:\\")), ItemKind::Drive);
        assert!(!known_locked('Q', true));
        note_lock('Q', true);
        assert_eq!(kind_of(&drive("Q:\\")), ItemKind::LockedDrive);
        assert!(known_locked('q', true), "just read: Open starts the unlock window");
        assert!(item_entries(&[kind_of(&drive("Q:\\"))]).contains(&Some(DesktopCommand::UnlockDrive)));
        // Another drive is not affected; a folder on the locked drive is a folder.
        assert_eq!(kind_of(&drive("R:\\")), ItemKind::Drive);
        // Unlocked (the next reading reads it): a drive again, its menu as before.
        note_lock('Q', false);
        assert_eq!(kind_of(&drive("Q:\\")), ItemKind::Drive);
        assert!(!known_locked('Q', false));
    }

    /// A tree row that could not be read: a locked drive's row opened by the person starts
    /// Windows' unlock window, one a drag opened is only marked as not readable (the review of
    /// 2026-11-21, A1); a drive that cannot be read for another reason gets the look's message
    /// when the person opened it, and nothing in the middle of a drag (the review before 0.1.3,
    /// B6). (Plain values: nothing is started.)
    #[test]
    fn a_drag_resting_on_a_locked_drive_starts_nothing() {
        use super::{OpenedBy, UnreadableRow, after_unreadable_row};
        assert_eq!(after_unreadable_row("E:\\", true, OpenedBy::Person), UnreadableRow::Unlock);
        assert_eq!(after_unreadable_row("E:\\", true, OpenedBy::Drag), UnreadableRow::Nothing);
        assert_eq!(after_unreadable_row("E:\\", false, OpenedBy::Person), UnreadableRow::DriveMessage);
        assert_eq!(after_unreadable_row("E:\\", false, OpenedBy::Drag), UnreadableRow::Nothing);
        for by in [OpenedBy::Person, OpenedBy::Drag] {
            assert_eq!(after_unreadable_row("E:\\folder", true, by), UnreadableRow::Nothing, "{by:?}");
            assert_eq!(after_unreadable_row("E:\\folder", false, by), UnreadableRow::Nothing, "{by:?}");
            assert_eq!(after_unreadable_row("::my-computer", true, by), UnreadableRow::Nothing, "{by:?}");
        }
    }

    /// A drag that opened a row which could not be read does not open it again, however long it
    /// rests or moves there (the review before 0.1.3, B6); other rows open as before.
    #[test]
    fn a_drag_does_not_open_again_a_row_it_found_unreadable() {
        use super::{Era, ROW_OPENS_2000, ROW_OPENS_98, drop_row_opens};
        use std::time::Duration;
        let long = Duration::from_secs(60);
        for era in Era::ALL {
            assert!(drop_row_opens(era, true, long, false), "{era:?}: a readable row opens");
            assert!(!drop_row_opens(era, true, long, true), "{era:?}: found unreadable: stays closed");
            assert!(!drop_row_opens(era, false, long, true), "{era:?}");
        }
        assert!(drop_row_opens(Era::Win2000, false, ROW_OPENS_2000, false), "2000: a rest without moving");
        assert!(!drop_row_opens(Era::Win2000, false, ROW_OPENS_2000 - Duration::from_millis(1), false));
        assert!(!drop_row_opens(Era::Win98, false, long, false), "98: at a move only");
        assert!(drop_row_opens(Era::Win98, true, ROW_OPENS_98, false));
    }

    /// Open and Explore on a drive shown locked start Windows' unlock window instead only while
    /// that reading is fresh; an older record lets the drive open as any drive (the review before
    /// 0.1.3, B2). A drive whose rows the tree read is noted as not locked. (Letters no PC here
    /// has; nothing is started.)
    #[test]
    fn a_drive_shown_locked_long_ago_opens_as_any_drive() {
        use super::{known_locked, note_lock, shown_locked_acts};
        use bevel_core::desktop::ItemKind;
        assert!(shown_locked_acts(ItemKind::LockedDrive, true));
        assert!(!shown_locked_acts(ItemKind::LockedDrive, false), "an old record: opened in the ordinary way");
        assert!(!shown_locked_acts(ItemKind::Drive, true));
        assert!(!shown_locked_acts(ItemKind::Folder, true));
        // Found locked, then read by the tree (the reader notes it as not locked): neither fresh
        // nor old says locked any more.
        note_lock('P', true);
        assert!(known_locked('P', true) && known_locked('P', false));
        note_lock('P', false);
        assert!(!known_locked('P', true) && !known_locked('P', false));
    }

    /// "This drive turned out locked" is acted on only when this program's own reader told it,
    /// for that window and that letter, once (the review of 2026-11-21, C1). Window ids no test
    /// window has.
    #[test]
    fn a_locked_drive_message_from_outside_does_nothing() {
        use super::{take_locked_told, tell_locked};
        // Nobody told: another program's message.
        assert!(!take_locked_told(9_001, 'T'));
        tell_locked(9_001, 't');
        assert!(!take_locked_told(9_002, 'T'), "another window");
        assert!(!take_locked_told(9_001, 'U'), "another letter");
        assert!(take_locked_told(9_001, 'T'));
        assert!(!take_locked_told(9_001, 'T'), "once only: the same message again does nothing");
        // Not a drive letter: never.
        tell_locked(9_003, '1');
        assert!(!take_locked_told(9_003, '1'));
    }

    /// The folder windows' private messages are registered by name, each its own number: none is
    /// a fixed WM_APP number that another program's broadcast could hit (the third safety review
    /// of 2026-11-18, C4).
    /// Whom a start hands its folder to, on a simulated clock: the hub's holder when it takes it;
    /// a second program that takes it when the holder does not (hung: each try costs its whole
    /// limit here, the worst case); a busy holder that takes it after a few seconds; and only when
    /// none takes it for `HAND_OVER_PATIENCE` (about 10 s, not 60) does it become a program itself;
    /// for another start making its window a moment (the fourth safety review of 2026-11-18, B3).
    #[test]
    fn a_start_hands_over_to_any_program_that_answers() {
        use super::{HAND_OVER_PATIENCE, HUB_COMING, HandedTo, hand_over_to};
        use std::cell::Cell;
        use std::time::Duration;
        // Hubs are numbers: 1 the holder, 2 and 3 second programs. `takes(hub, now)`.
        let run = |hubs: Vec<u32>, takes: &dyn Fn(u32, Duration) -> bool, existed: bool| {
            let clock = Cell::new(Duration::ZERO);
            let tried = std::cell::RefCell::new(Vec::new());
            let handed = hand_over_to(
                || hubs.clone(),
                |hub, limit| {
                    tried.borrow_mut().push(hub);
                    if takes(hub, clock.get()) {
                        clock.set(clock.get() + Duration::from_millis(5));
                        true
                    } else {
                        clock.set(clock.get() + limit);
                        false
                    }
                },
                || clock.get(),
                |d| clock.set(clock.get() + d),
                existed,
            );
            (handed, clock.get(), tried.into_inner())
        };
        // The holder answers: at once.
        let (handed, took, tried) = run(vec![1, 2], &|h, _| h == 1, true);
        assert_eq!((handed, tried), (HandedTo::Program(1), vec![1]));
        assert!(took < Duration::from_millis(100));
        // The holder hung, a second program answers: handed to it in the first round, at once.
        let (handed, took, _) = run(vec![1, 2, 3], &|h, _| h == 3, true);
        assert_eq!(handed, HandedTo::Program(3));
        assert!(took <= Duration::from_millis(700), "{took:?}");
        // The holder busy for 4 s, then taking it: waited for, no second program.
        let (handed, took, _) = run(vec![1], &|h, now| h == 1 && now >= Duration::from_secs(4), true);
        assert_eq!(handed, HandedTo::Program(1));
        assert!(took < HAND_OVER_PATIENCE);
        // None takes it: a program of its own after about the patience, never a minute.
        let (handed, took, tried) = run(vec![1, 2], &|_, _| false, true);
        assert_eq!(handed, HandedTo::Nobody { waited_for_one: true });
        assert!(took >= HAND_OVER_PATIENCE && took < HAND_OVER_PATIENCE + Duration::from_secs(3), "{took:?}");
        assert!(tried.contains(&2), "the second program was asked too");
        // No program: another start making its window is waited for a moment; the first start not.
        let (handed, took, _) = run(vec![], &|_, _| false, true);
        assert_eq!(handed, HandedTo::Nobody { waited_for_one: false });
        assert!(took >= HUB_COMING && took < HUB_COMING + Duration::from_millis(200));
        let (handed, took, _) = run(vec![], &|_, _| false, false);
        assert_eq!((handed, took), (HandedTo::Nobody { waited_for_one: false }, Duration::ZERO));
        assert!(HAND_OVER_PATIENCE <= Duration::from_secs(10));
    }

    /// The title of a window from what is read of its place (`title_from`), and what it shows
    /// before that is read: no drive is asked for it.
    #[test]
    fn a_title_is_made_from_what_is_read_of_the_place() {
        use super::{DriveKind, Era, Facts, Named, Shows, Small, title_from};
        use std::path::PathBuf;
        let root = Shows::Folder(PathBuf::from("C:\\"));
        let mut facts = Facts::default();
        assert_eq!(title_from(Era::Win2000, &root, &facts), ("C:\\".to_string(), Small::Drive(DriveKind::Fixed)), "not read yet");
        facts.names.insert(root.key(), Named { drive: Some((DriveKind::Fixed, String::new())), display: None });
        assert_eq!(title_from(Era::Win2000, &root, &facts).0, "Local Disk (C:)");
        assert_eq!(title_from(Era::Win98, &root, &facts).0, " (C:)");
        let docs = PathBuf::from("C:\\Users\\someone\\Documents");
        let folder = Shows::Folder(docs.clone());
        assert_eq!(title_from(Era::Win2000, &folder, &facts).0, "Documents", "its own name until read");
        facts.documents = Some(docs);
        assert_eq!(title_from(Era::Win2000, &folder, &facts), ("My Documents".to_string(), Small::MyDocuments));
        assert_eq!(title_from(Era::Win95, &folder, &facts).0, "Documents");
        facts.my_computer = true;
        facts.items = 3;
        assert_eq!((facts.count(Era::Win95), facts.count(Era::Win2000)), (5, 4));
    }

    /// The fifth safety review of 2026-11-18, C1: the program does not end while a folder handed
    /// over or asked for is still to open, with or without file jobs; it ends only when nothing is
    /// left. Before, a folder handed over while the last window closed, or while the program
    /// waited for its file jobs, was never opened.
    #[test]
    fn a_folder_still_to_open_keeps_the_program() {
        use super::{AfterLast, after_last_window};
        assert_eq!(after_last_window(true, false), AfterLast::Open, "handed over as the last window closed");
        assert_eq!(after_last_window(true, true), AfterLast::Open, "handed over while a copy runs");
        assert_eq!(after_last_window(false, true), AfterLast::WaitForJobs);
        assert_eq!(after_last_window(false, false), AfterLast::End);
        // A hand-over's folder counts from the moment it is taken, before it is asked for.
        super::PENDING_OPEN.lock().unwrap().push("C:\\Bevel95 test".into());
        assert!(super::waiting_to_open());
        super::PENDING_OPEN.lock().unwrap().clear();
        assert!(!super::waiting_to_open());
    }

    /// The fifth safety review of 2026-11-18, C2: a window that opened before its place was read
    /// takes that reading with the id it had when it opened, so `apply_facts` applies it only
    /// while it still shows that place (it was applied with no id: after going elsewhere, the
    /// window showed the first place's drive space and lost its title).
    #[test]
    fn a_late_reading_goes_only_to_the_place_it_was_read_for() {
        use super::{HWND, OnReading, Shows, on_reading};
        let (window, other) = (7 as HWND, 9 as HWND);
        let d = Some((Shows::Folder("D:\\".into()), false));
        // Opened early as id 3: the reading carries id 3 (the window's id after going elsewhere
        // is another, so it is not applied).
        assert_eq!(on_reading(Some(Some((window, 3))), d.clone(), |h| h == window), OnReading::Apply { hwnd: window, id: 3 });
        // The early window has closed: nothing.
        assert_eq!(on_reading(Some(Some((window, 3))), d.clone(), |h| h == other), OnReading::Nothing);
        // An open window came forward instead: nothing.
        assert_eq!(on_reading(Some(None), d.clone(), |_| true), OnReading::Nothing);
        // No window yet: it opens; not a folder: nothing.
        assert_eq!(on_reading(None, d, |_| true), OnReading::Open(Shows::Folder("D:\\".into()), false));
        assert_eq!(on_reading(None, None, |_| true), OnReading::Nothing);
    }

    /// An outside review of 2026-11-18: a window that opened before its place was read showed its
    /// files, and the reading, when it came, started the view afresh (`present` called
    /// `show_place`), so the files went until Refresh (the reader tells only of changes). The late
    /// reading (`show_what_is_known`, as `apply_facts` uses it) now changes the title, the address
    /// and the web view's facts, and keeps the items, the selection and the scroll, in 95's
    /// Explorer, 98 and 2000.
    #[test]
    fn a_late_reading_keeps_the_files_shown() {
        use super::{DriveKind, Era, Facts, FolderHost, FolderWindow, History, Named, Shows, show_what_is_known};
        use bevel_core::desktop_icons::{Item, Picture};
        use bevel_core::fb::Icon;
        use std::sync::Arc;
        use std::sync::atomic::AtomicBool;
        let root = Shows::Folder("Z:\\".into());
        for (era, tree) in [(Era::Win95, true), (Era::Win98, false), (Era::Win2000, false), (Era::Win2000, true)] {
            let icon = Icon { width: 16, height: 16, pixels: vec![0; 256] };
            let mut form = FolderWindow::new(era, "Z:\\", icon.clone(), true, (400, 300));
            if tree {
                form.exploring = true;
                form.show_tree(true);
            }
            let mut h = FolderHost {
                form,
                id: 1,
                shows: root.clone(),
                stop: Arc::new(AtomicBool::new(false)),
                select: None,
                history: History::new(root.place()),
                came: None,
                entries: Vec::new(),
                settle: None,
                reread: Vec::new(),
                asked: None,
                facts: Facts::default(),
                session: std::collections::HashMap::new(),
                tab: 1,
                order: vec![1],
                back: Vec::new(),
            };
            let items: Vec<Item> = (0..30)
                .map(|i| Item {
                    id: format!("Z:\\Bevel95 test {i:02}.txt"),
                    name: format!("Bevel95 test {i:02}.txt"),
                    picture: Picture::Own(None),
                    shortcut: false,
                    folder: false,
                    type_name: "Text Document".into(),
                    size: i,
                    modified: 0,
                })
                .collect();
            h.form.set_items(items);
            h.form.view.select_only(Some(7));
            // The reading comes: the drive's kind and label, its space.
            let mut facts = Facts::default();
            facts.names.insert(root.key(), Named { drive: Some((DriveKind::Network, "Bevel95 test".into())), display: None });
            facts.space = Some(super::Space { free: 10, capacity: 20 });
            h.facts = facts;
            show_what_is_known(&mut h, era);
            assert_eq!(h.form.view.items.len(), 30, "{era:?} tree {tree}: the files stay");
            assert_eq!(h.form.view.selection(), vec![7], "{era:?} tree {tree}: the selection stays");
            assert!(h.form.title.contains("Bevel95 test"), "{era:?} tree {tree}: the title is the drive's: {}", h.form.title);
            if era != Era::Win95 {
                assert_eq!(h.form.web.space.map(|s| s.free), Some(10), "{era:?}: the drive's space");
            }
        }
    }

    /// The sixth safety review of 2026-11-18, C3: a place whose reading never comes (a drive that
    /// does not answer, a reading thread that panicked) no longer keeps the program for ever. A
    /// start's or a hand-over's argument has the same patience as any place (after it, the window
    /// opens with the place it names, its drive not asked); a window opened early keeps nothing
    /// alive; a reading that ended without an answer is "not read" at once. Before, an argument had
    /// no deadline and every asking, opened or not, kept the program.
    #[test]
    fn a_place_whose_reading_never_comes_does_not_keep_the_program() {
        use super::{Answer, Ask, Asked, EXPLORE, Era, Facts, HWND, NOT_READ, PATIENCE, PREPARED, Shows, due, known_of, not_read, still_to_open};
        use std::time::{Duration, Instant};
        let now = Instant::now();
        let arg = |serial, deadline| Asked { serial, what: Ask::Arg("Z:".into()), deadline, opened: None };
        let serials = |d: Vec<(usize, Ask)>| d.into_iter().map(|(s, _)| s).collect::<Vec<_>>();
        let mut asked = vec![arg(1, now + PATIENCE), arg(2, now)];
        assert_eq!(serials(due(&asked, now)), [2], "an argument past its patience is due; one within it is not");
        assert!(still_to_open(&asked));
        // What is known of an argument without asking its drive: the place it names.
        assert_eq!(known_of(&Ask::Arg("Z:".into()), Era::Win2000), Some((Shows::Folder("Z:\\".into()), false)));
        assert_eq!(known_of(&Ask::Arg(format!("{EXPLORE}Z:\\Bevel95 test")), Era::Win98), Some((Shows::Folder("Z:\\Bevel95 test".into()), true)));
        assert_eq!(known_of(&Ask::Arg(String::new()), Era::Win95), None, "no place at all: nothing opens");
        // Opened (or an open window came forward): it keeps nothing alive.
        asked[0].opened = Some(Some((7 as HWND, 3)));
        asked[1].opened = Some(None);
        assert!(!still_to_open(&asked));
        assert!(due(&asked, now + Duration::from_secs(60)).is_empty());
        // A reading that ended without an answer: an opened asking goes, one still to open is due.
        let mut asked = vec![Asked { opened: Some(None), ..arg(1, now) }, arg(2, now + Duration::from_secs(60)), arg(3, now + Duration::from_secs(60))];
        not_read(&mut asked, &[1, 2], now);
        assert_eq!(asked.iter().map(|x| x.serial).collect::<Vec<_>>(), [2, 3]);
        assert_eq!(serials(due(&asked, now)), [2]);
        // A reading thread that panics hands over "not read"; one that ends hands over its reading.
        let unwound = std::panic::catch_unwind(|| {
            let _answer = Answer { serial: 9_001, hub: 0, reading: None };
            panic!("a test panic inside a place's reading");
        });
        assert!(unwound.is_err());
        assert!(NOT_READ.lock().unwrap().contains(&9_001));
        NOT_READ.lock().unwrap().retain(|&s| s != 9_001);
        drop(Answer { serial: 9_002, hub: 0, reading: Some((None, Facts::default())) });
        assert!(PREPARED.lock().unwrap().iter().any(|(s, ..)| *s == 9_002));
        PREPARED.lock().unwrap().retain(|(s, ..)| *s != 9_002);
    }

    #[test]
    fn the_folder_windows_messages_are_registered_by_name() {
        let all = [
            &super::WM_ITEMS, &super::WM_RENAME_ANSWER, &super::WM_ICON_AT, &super::WM_ICON_PLACE, &super::WM_TEST_COMMAND,
            &super::WM_TREE_ROWS, &super::WM_UNREADABLE, &super::WM_PICTURES,
        ];
        let mut ids: Vec<u32> = all.iter().map(|m| m.id()).collect();
        assert!(ids.iter().all(|&id| (0xC000..=0xFFFF).contains(&id)), "{ids:x?}");
        assert!(all.iter().all(|m| m.name().starts_with("Bevel95.Folders.")));
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), all.len());
    }
}