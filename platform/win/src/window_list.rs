//! Finding the windows that belong on the taskbar, and what the bar needs to show them:
//! title, program (for grouping), app name and icon. Also what a click does to them.

use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::path::Path;
use std::ptr::{null, null_mut};
use std::sync::OnceLock;

use bevel_core::fb::Icon;
use bevel_core::tasks::WindowInfo;
use bevel_core::window_icons::{self, Places};
use bevel_core::window_menu::{GroupCommand, WindowCommand, WindowState};
use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::UI::Shell::Common::ITEMIDLIST;
use windows_sys::Win32::UI::Shell::{
    FOLDERID_ComputerFolder, FOLDERID_ControlPanelFolder, FOLDERID_Documents, FOLDERID_RecycleBinFolder,
    SHGFI_DISPLAYNAME, SHGFI_PIDL, SHGetKnownFolderIDList, SHParseDisplayName,
};
use windows_sys::core::GUID;
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, GetLastError, HWND, LPARAM};
use windows_sys::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
use windows_sys::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows_sys::Win32::UI::Shell::{SHFILEINFOW, SHGFI_ICON, SHGFI_SMALLICON, SHGetFileInfoW, ShellExecuteW};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CS_NOCLOSE, CascadeWindows, DestroyIcon, EnumWindows, GA_ROOTOWNER, GCL_STYLE, GCLP_HICON,
    GCLP_HICONSM, GW_OWNER, GWL_EXSTYLE, GWL_STYLE, GetAncestor, GetClassLongPtrW, GetClassNameW, GetForegroundWindow,
    GetWindow, GetWindowLongPtrW, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, HICON,
    ICON_BIG,
    ICON_SMALL, ICON_SMALL2, IsIconic, IsWindow, IsWindowVisible, IsZoomed, MDITILE_HORIZONTAL, MDITILE_VERTICAL, PostMessageW,
    SC_CLOSE, SC_MAXIMIZE, SC_MINIMIZE, SC_MOVE, SC_RESTORE, SC_SIZE, SMTO_ABORTIFHUNG, SW_MINIMIZE, SW_RESTORE,
    SHOW_WINDOW_CMD, SW_SHOWNORMAL, SendMessageTimeoutW, SetForegroundWindow, ShowWindowAsync, TileWindows, WM_GETICON,
    WM_SYSCOMMAND, WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_THICKFRAME,
};
use windows_sys::core::BOOL;

use crate::log::log;
use crate::util::{Forward, bring_forward, wide};

/// Every window that belongs on the taskbar, oldest-looking first. Windows' own rule: it is
/// visible, not cloaked (Windows cloaks suspended apps and windows on other virtual
/// desktops), has a title, and is either marked as an app window or is neither owned by
/// another window nor a tool window.
pub fn taskbar_windows() -> Vec<HWND> {
    let mut all: Vec<HWND> = Vec::new();
    // SAFETY: `collect` only pushes into the Vec passed through `lparam`, which outlives the call.
    unsafe { EnumWindows(Some(collect), &mut all as *mut Vec<HWND> as LPARAM) };
    // EnumWindows lists front to back; reversed, the window used longest ago comes first.
    all.into_iter().rev().filter(|&h| belongs_on_taskbar(h)).collect()
}

unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: `lparam` is the &mut Vec<HWND> given to EnumWindows above.
    let all = unsafe { &mut *(lparam as *mut Vec<HWND>) };
    all.push(hwnd);
    1
}

pub(crate) fn belongs_on_taskbar(hwnd: HWND) -> bool {
    // The classic desktop never does, and asking it for its title would wait for its thread,
    // which Explorer's can hold up (`desktop.rs`); nor do the Monitor Lines (brief 102).
    if crate::desktop::is_ours(hwnd) || crate::monitor_lines::is_ours(hwnd) {
        return false;
    }
    // SAFETY: read-only queries; all accept any window handle.
    unsafe {
        if IsWindowVisible(hwnd) == 0 || GetWindowTextLengthW(hwnd) == 0 {
            return false;
        }
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let owned = !GetWindow(hwnd, GW_OWNER).is_null();
        if ex & WS_EX_APPWINDOW == 0 && (owned || ex & (WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE) != 0) {
            return false;
        }
    }
    // The desktop itself is a visible, titled window too.
    !is_cloaked(hwnd) && !matches!(class_name(hwnd).as_str(), "Progman" | "WorkerW")
}

pub fn is_cloaked(hwnd: HWND) -> bool {
    let mut cloaked: u32 = 0;
    // SAFETY: a 4-byte out-buffer, as DWMWA_CLOAKED needs.
    let hr =
        unsafe { DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED as u32, (&mut cloaked as *mut u32).cast::<c_void>(), 4) };
    hr == 0 && cloaked != 0
}

pub fn class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 256];
    // SAFETY: the buffer length is passed.
    let n = unsafe { GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32) }.max(0) as usize;
    String::from_utf16_lossy(&buf[..n])
}

/// A window's title. (For other programs' windows Windows answers this itself, so a hung
/// program cannot make the bar hang.)
pub fn title(hwnd: HWND) -> String {
    // SAFETY: the buffer is one longer than the reported length, which is passed.
    unsafe {
        let len = GetWindowTextLengthW(hwnd).max(0) as usize;
        let mut buf = vec![0u16; len + 1];
        let n = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32).max(0) as usize;
        String::from_utf16_lossy(&buf[..n.min(len)])
    }
}

/// The program behind a window: full path and display name ("Notepad").
#[derive(Clone, Debug, Default)]
pub struct Program {
    pub path: String,
    pub name: String,
}

/// Program lookups, cached per process (it reads the program file's version information).
#[derive(Default)]
pub struct Programs {
    by_pid: HashMap<u32, Program>,
}

impl Programs {
    pub fn of(&mut self, hwnd: HWND) -> Program {
        let pid = process_id(hwnd);
        self.by_pid
            .entry(pid)
            .or_insert_with(|| {
                let path = process_path(pid).unwrap_or_default();
                let name = file_description(&path).unwrap_or_else(|| {
                    Path::new(&path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
                });
                Program { path, name }
            })
            .clone()
    }

    /// Forgets processes that no longer have taskbar windows (their ids get reused).
    pub fn keep_only(&mut self, windows: &[HWND]) {
        let alive: HashSet<u32> = windows.iter().map(|&h| process_id(h)).collect();
        self.by_pid.retain(|pid, _| alive.contains(pid));
    }
}

fn process_id(hwnd: HWND) -> u32 {
    let mut pid = 0;
    // SAFETY: a valid out-pointer.
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    pid
}

/// The full path of the program a window belongs to ("" if Windows will not say).
pub fn program_path(hwnd: HWND) -> String {
    process_path(process_id(hwnd)).unwrap_or_default()
}

/// A window as the log may show it: its kind in Bevel95's own words (`kind_of`) and its program's
/// file name, never its title or class (no private text in the log).
pub fn describe(hwnd: HWND) -> String {
    let path = program_path(hwnd);
    let exe = path.rsplit(['\\', '/']).next().filter(|s| !s.is_empty()).unwrap_or("an unknown program");
    format!("{} ({exe})", kind_of(hwnd))
}

/// The kind of a window as the log may say it, in Bevel95's own words from a fixed list: never
/// its class, which can carry another program's name (WPF's `HwndWrapper[<program>;..]`; the
/// review of brief 90, R5).
pub fn kind_of(hwnd: HWND) -> &'static str {
    if hwnd.is_null() {
        return "no window";
    }
    if !exists(hwnd) {
        return "a window that is gone";
    }
    if is_own(hwnd) {
        return "one of Bevel95's windows";
    }
    window_kind(&class_name(hwnd))
}

/// `kind_of` by a window's class (Windows' own classes; anything else is another program's).
fn window_kind(class: &str) -> &'static str {
    match class {
        "Shell_TrayWnd" | "Shell_SecondaryTrayWnd" | "NotifyIconOverflowWindow" | "TopLevelWindowForOverflowXamlIsland" => {
            "Windows' own taskbar"
        }
        "Windows.UI.Core.CoreWindow" | "Shell_LightDismissOverlay" | "StartMenuSizingFrame" | "XamlExplorerHostIslandWindow" => {
            "Windows' own Start or one of its panels"
        }
        "Progman" | "WorkerW" | "SHELLDLL_DefView" => "Windows' own desktop",
        "#32768" => "a menu",
        "#32770" => "a dialog box",
        "tooltips_class32" => "a tooltip",
        "ApplicationFrameWindow" => "a Store app's window",
        "ConsoleWindowClass" | "PseudoConsoleWindow" => "a console window",
        "" => "a window without a class",
        _ => "another program's window",
    }
}

fn process_path(pid: u32) -> Option<String> {
    // SAFETY: the handle is closed after use; the buffer size is passed in and out.
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return None;
        }
        let mut buf = vec![0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len);
        CloseHandle(process);
        (ok != 0).then(|| String::from_utf16_lossy(&buf[..len as usize]))
    }
}

/// The "File description" from a program file's version information ("Google Chrome").
fn file_description(path: &str) -> Option<String> {
    if path.is_empty() {
        return None;
    }
    let file = wide(path);
    // SAFETY: buffers sized as Windows reports; VerQueryValueW returns pointers into `data`,
    // which are only read while `data` is alive.
    unsafe {
        let size = GetFileVersionInfoSizeW(file.as_ptr(), null_mut());
        if size == 0 {
            return None;
        }
        let mut data = vec![0u8; size as usize];
        if GetFileVersionInfoW(file.as_ptr(), 0, size, data.as_mut_ptr().cast()) == 0 {
            return None;
        }
        let mut value: *mut c_void = null_mut();
        let mut len = 0u32;
        let translation = wide("\\VarFileInfo\\Translation");
        let (language, code_page) =
            if VerQueryValueW(data.as_ptr().cast(), translation.as_ptr(), &mut value, &mut len) != 0 && len >= 4 {
                let pair = value as *const u16;
                (*pair, *pair.add(1))
            } else {
                (0x0409, 0x04B0) // US English, Unicode: the usual default
            };
        let key = wide(&format!("\\StringFileInfo\\{language:04x}{code_page:04x}\\FileDescription"));
        if VerQueryValueW(data.as_ptr().cast(), key.as_ptr(), &mut value, &mut len) == 0 || len == 0 {
            return None;
        }
        let text = String::from_utf16_lossy(std::slice::from_raw_parts(value as *const u16, len as usize));
        let text = text.trim_end_matches('\0').trim();
        (!text.is_empty()).then(|| text.to_string())
    }
}

/// What the bar's grouping needs to know about a window (`flashing` is filled in later).
pub fn window_info(hwnd: HWND, programs: &mut Programs) -> WindowInfo {
    let program = programs.of(hwnd);
    let exe = Path::new(&program.path).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    let (class, raw_title) = (class_name(hwnd), title(hwnd));
    let classic = window_icons::classify(&exe, &class, &raw_title, places());
    let title = window_icons::classic_title(&exe, &class, &raw_title, places());
    let (group, app_name) = if exe.eq_ignore_ascii_case("ApplicationFrameHost.exe") {
        // Modern apps (Settings, Calculator, ...) all run inside this one helper program;
        // tell them apart by title rather than lumping them together.
        (format!("frame:{title}"), title.clone())
    } else if program.path.is_empty() {
        (format!("class:{class}"), title.clone())
    } else {
        (program.path.to_lowercase(), program.name)
    };
    WindowInfo { id: hwnd as u64, title, group, app_name, flashing: false, classic }
}

/// The names Explorer shows for the places that have a classic picture of their own (in this
/// Windows' language), read once.
fn places() -> &'static Places {
    static PLACES: OnceLock<Places> = OnceLock::new();
    PLACES.get_or_init(|| {
        let mut places = Places::default();
        if let Some(path) = crate::start_menu::known_folder(&FOLDERID_Documents) {
            let path = path.display().to_string();
            places.documents.extend(path_display_name(&path));
            places.documents.push(path);
        }
        places.my_computer.extend(known_folder_name(&FOLDERID_ComputerFolder));
        // Windows 11's Home, where File Explorer opens (it has no known-folder id).
        places.my_computer.extend(parsed_name("::{F874310E-B6B7-47DC-BC84-B9E6B38F5903}"));
        places.recycle_bin.extend(known_folder_name(&FOLDERID_RecycleBinFolder));
        places.control_panel.extend(known_folder_name(&FOLDERID_ControlPanelFolder));
        // Control Panel's category view, beside its list of all items.
        places.control_panel.extend(parsed_name("::{26EE0668-A00A-44D7-9371-BEB064C98683}"));
        log(&format!(
            "classic icons go to these places (names known, not logged): documents {}, my computer {}, recycle bin {}, control panel {}",
            places.documents.len(),
            places.my_computer.len(),
            places.recycle_bin.len(),
            places.control_panel.len()
        ));
        places
    })
}

/// The name Explorer shows for a file or folder.
fn path_display_name(path: &str) -> Option<String> {
    let path = wide(path);
    let mut info = SHFILEINFOW::default();
    // SAFETY: a null-terminated path and a correctly sized out-struct.
    let ok = unsafe { SHGetFileInfoW(path.as_ptr(), 0, &mut info, size_of::<SHFILEINFOW>() as u32, SHGFI_DISPLAYNAME) };
    (ok != 0).then(|| text_of(&info.szDisplayName))
}

/// The name Explorer shows for a known folder that is not a folder on disk (This PC, ...).
fn known_folder_name(id: &GUID) -> Option<String> {
    let mut list: *mut ITEMIDLIST = null_mut();
    // SAFETY: a valid id and out-pointer; `list_name` frees the list.
    unsafe { (SHGetKnownFolderIDList(id, 0, null_mut(), &mut list) == 0).then(|| list_name(list)).flatten() }
}

/// The name Explorer shows for a shell parsing name such as "::{CLSID}".
fn parsed_name(parsing_name: &str) -> Option<String> {
    let name = wide(parsing_name);
    let (mut list, mut attributes): (*mut ITEMIDLIST, u32) = (null_mut(), 0);
    // SAFETY: a null-terminated name and valid out-pointers; `list_name` frees the list.
    unsafe {
        (SHParseDisplayName(name.as_ptr(), null_mut(), &mut list, 0, &mut attributes) == 0)
            .then(|| list_name(list))
            .flatten()
    }
}

/// # Safety
/// `list` is null or an item list from the shell, which is freed here.
unsafe fn list_name(list: *mut ITEMIDLIST) -> Option<String> {
    if list.is_null() {
        return None;
    }
    let mut info = SHFILEINFOW::default();
    let flags = SHGFI_PIDL | SHGFI_DISPLAYNAME;
    // SAFETY: with SHGFI_PIDL the first argument is the item list; the out-struct is sized.
    let ok = unsafe { SHGetFileInfoW(list as *const u16, 0, &mut info, size_of::<SHFILEINFOW>() as u32, flags) };
    // SAFETY: the shell allocated the list with the COM allocator.
    unsafe { CoTaskMemFree(list as *const c_void) };
    (ok != 0).then(|| text_of(&info.szDisplayName))
}

fn text_of(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

/// The window's 16×16 icon as pixels: its own small icon, else its class's, else the
/// program file's.
pub fn icon(hwnd: HWND, program_path: &str) -> Option<Icon> {
    if let Some(handle) = window_icon(hwnd) {
        return icon_pixels(handle); // the window's own icon: not ours to destroy
    }
    let path = wide(program_path);
    let mut info = SHFILEINFOW::default();
    // SAFETY: a correctly sized out-struct; the icon we get is ours to destroy.
    unsafe {
        SHGetFileInfoW(path.as_ptr(), 0, &mut info, size_of::<SHFILEINFOW>() as u32, SHGFI_ICON | SHGFI_SMALLICON);
        if info.hIcon.is_null() {
            return None;
        }
        let pixels = icon_pixels(info.hIcon);
        DestroyIcon(info.hIcon);
        pixels
    }
}

/// The shell's 32×32 picture of a file (a program, a shortcut: without the shortcut's arrow;
/// Windows 7's slots, brief 114: a real 32×32, never a doubled 16×16). Read only on the Windows 7
/// store's own thread (`seven_store`), never on the bar's.
pub fn read_big_icon(path: &str) -> Option<Icon> {
    if path.is_empty() {
        return None;
    }
    let wide_path = wide(path);
    let mut info = SHFILEINFOW::default();
    // SAFETY: a correctly sized out-struct; the icon we get is ours to destroy.
    unsafe {
        SHGetFileInfoW(wide_path.as_ptr(), 0, &mut info, size_of::<SHFILEINFOW>() as u32, SHGFI_ICON | windows_sys::Win32::UI::Shell::SHGFI_LARGEICON);
        if info.hIcon.is_null() {
            None
        } else {
            let pixels = bevel_winkit::icon::icon_pixels_at(info.hIcon, 32);
            DestroyIcon(info.hIcon);
            pixels
        }
    }
}

/// The shell's picture of a file at exactly `size`×`size` (XP's and 7's live pictures above size
/// 1: the owner, 2026-11-25): from the system's 48×48 list up to 48, else its 256×256 one, shrunk
/// by averaging, its soft edges kept. `None` when the file has no picture that large (a program
/// whose largest is smaller: the shell puts it in the corner of the larger list's square), so the
/// enlarged 1x picture stays. Read only on the Windows 7 store's own thread (`seven_store`).
pub fn read_icon_at(path: &str, size: i32) -> Option<Icon> {
    use windows_sys::Win32::UI::Controls::{HIMAGELIST, ILD_TRANSPARENT, ImageList_GetIcon};
    use windows_sys::Win32::UI::Shell::{SHGFI_SYSICONINDEX, SHGetImageList, SHIL_EXTRALARGE, SHIL_JUMBO};
    /// IImageList's interface id.
    const IID_IMAGE_LIST: windows_sys::core::GUID = windows_sys::core::GUID::from_u128(0x46eb5926_582e_4017_9fdf_e8998daa0950);
    if path.is_empty() || !(1..=256).contains(&size) {
        return None;
    }
    let (list_kind, native, smaller) = if size <= 48 { (SHIL_EXTRALARGE, 48, 32) } else { (SHIL_JUMBO, 256, 48) };
    let wide_path = wide(path);
    let mut info = SHFILEINFOW::default();
    // SAFETY: a correctly sized out-struct; with SHGFI_SYSICONINDEX only the index is filled.
    let ok = unsafe { SHGetFileInfoW(wide_path.as_ptr(), 0, &mut info, size_of::<SHFILEINFOW>() as u32, SHGFI_SYSICONINDEX) };
    if ok == 0 {
        return None;
    }
    let mut list: *mut c_void = null_mut();
    // SAFETY: a valid interface id and out-pointer; the reference is released below. An
    // IImageList pointer is the image list's handle (documented).
    let icon = unsafe {
        if SHGetImageList(list_kind as i32, &IID_IMAGE_LIST, &mut list) < 0 || list.is_null() {
            return None;
        }
        let handle = ImageList_GetIcon(list as HIMAGELIST, info.iIcon, ILD_TRANSPARENT);
        bevel_winkit::com::release(list);
        if handle.is_null() {
            return None;
        }
        let pixels = bevel_winkit::icon::icon_pixels_at(handle, native);
        DestroyIcon(handle);
        pixels?
    };
    // A smaller picture in the square's corner: nothing outside its top-left `smaller` square.
    let outside_corner = (0..native).flat_map(|y| (0..native).map(move |x| (x, y))).filter(|&(x, y)| x >= smaller || y >= smaller);
    if outside_corner.clone().all(|(x, y)| icon.pixels[(y * native + x) as usize] >> 24 == 0) {
        return None;
    }
    Some(icon.downscaled(size))
}

/// A window's own large picture at its own size (32×32 mostly; larger where the program gives
/// one), for XP's task buttons above size 1 and 7's slots of a window whose program file has no
/// picture (the owner, 2026-11-25). Asked with the same 50 ms patience as its small one.
pub fn window_big_icon(hwnd: HWND) -> Option<Icon> {
    let mut result = 0usize;
    // SAFETY: asking another program for its icon, giving up after 50 ms if it hangs.
    let ok = unsafe { SendMessageTimeoutW(hwnd, WM_GETICON, ICON_BIG as usize, 0, SMTO_ABORTIFHUNG, 50, &mut result) };
    let handle = if ok != 0 && result != 0 {
        result as HICON
    } else {
        // SAFETY: a read-only query.
        let class = unsafe { GetClassLongPtrW(hwnd, GCLP_HICON) };
        if class == 0 {
            return None;
        }
        class as HICON
    };
    // The window's own icon: not ours to destroy.
    let size = bevel_winkit::icon::icon_size(handle)?.clamp(16, 256);
    bevel_winkit::icon::icon_pixels_at(handle, size)
}

fn window_icon(hwnd: HWND) -> Option<HICON> {
    for kind in [ICON_SMALL2, ICON_SMALL, ICON_BIG] {
        let mut result = 0usize;
        // SAFETY: asking another program for its icon, giving up after 50 ms if it hangs.
        let ok = unsafe { SendMessageTimeoutW(hwnd, WM_GETICON, kind as usize, 0, SMTO_ABORTIFHUNG, 50, &mut result) };
        if ok != 0 && result != 0 {
            return Some(result as HICON);
        }
    }
    for index in [GCLP_HICONSM, GCLP_HICON] {
        // SAFETY: a read-only query.
        let handle = unsafe { GetClassLongPtrW(hwnd, index) };
        if handle != 0 {
            return Some(handle as HICON);
        }
    }
    None
}

/// Drawn on black and on white; the difference gives each pixel's transparency (shared with the
/// folder windows: `bevel_winkit::icon`).
pub use bevel_winkit::icon::{icon_pixels, icon_pixels_fitted};

/// The taskbar window that is in front: the foreground window, or the window that owns it
/// (a dialog's main window).
pub fn active_window() -> HWND {
    // SAFETY: read-only queries.
    unsafe {
        let window = GetForegroundWindow();
        if window.is_null() { window } else { GetAncestor(window, GA_ROOTOWNER) }
    }
}

/// The window is one of the bar's own process (a menu of Bevel95's, the volume control).
pub fn is_own(hwnd: HWND) -> bool {
    let mut pid = 0;
    // SAFETY: any window handle and a valid out-pointer; a plain query.
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(hwnd, &mut pid) != 0
            && pid == windows_sys::Win32::System::Threading::GetCurrentProcessId()
    }
}

/// The foreground window itself (not its owner, unlike `active_window`).
pub fn foreground() -> HWND {
    // SAFETY: a read-only query.
    unsafe { GetForegroundWindow() }
}

/// The window is minimized now.
pub fn is_minimized(hwnd: HWND) -> bool {
    // SAFETY: a read-only query that accepts any handle.
    unsafe { IsIconic(hwnd) != 0 }
}

/// The window still exists.
pub fn exists(hwnd: HWND) -> bool {
    // SAFETY: a read-only query that accepts any handle.
    unsafe { IsWindow(hwnd) != 0 }
}

/// Brings a window to the front, restoring it if minimized (clicking its button).
pub fn activate(hwnd: HWND) {
    // SAFETY: calls on another program's window; ShowWindowAsync never waits for it.
    unsafe {
        if IsIconic(hwnd) != 0 {
            show_async(hwnd, SW_RESTORE);
        }
    }
    // Windows can refuse SetForegroundWindow; then our input is joined to the window in
    // front for one call, as for our own menus (`util::bring_forward`).
    match bring_forward(hwnd) {
        Forward::Direct => {}
        Forward::Joined => log(&format!(
            "SetForegroundWindow refused at first for {}; it worked with the input joined",
            kind_of(hwnd)
        )),
        Forward::Refused => log(&format!("SetForegroundWindow refused for {}, also with the input joined", kind_of(hwnd))),
    }
}

/// Minimizes a window (clicking the button of the window already in front).
pub fn minimize(hwnd: HWND) {
    // SAFETY: as above.
    unsafe { show_async(hwnd, SW_MINIMIZE) };
}

/// Minimizes (SW_MINIMIZE) or restores (SW_RESTORE) a window without waiting for it. Windows
/// refuses this for the windows of programs running as administrator (Task Manager), but lets
/// their own Minimize and Restore commands through, as RetroBar sends them; those go instead.
/// (Maximize is refused either way; Move and Size were not tried.)
///
/// # Safety
/// Any window handle; nothing waits for the other program.
unsafe fn show_async(hwnd: HWND, how: SHOW_WINDOW_CMD) {
    // SAFETY: as above.
    unsafe {
        if ShowWindowAsync(hwnd, how) == 0 {
            let command = if how == SW_MINIMIZE { SC_MINIMIZE } else { SC_RESTORE };
            PostMessageW(hwnd, WM_SYSCOMMAND, command as usize, 0);
        }
    }
}

/// What a window allows now, for its menu's grayed-out entries.
pub fn window_state(hwnd: HWND) -> WindowState {
    // SAFETY: read-only queries; all accept any window handle.
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let class_style = GetClassLongPtrW(hwnd, GCL_STYLE) as u32;
        WindowState {
            minimized: IsIconic(hwnd) != 0,
            maximized: IsZoomed(hwnd) != 0,
            resizable: style & WS_THICKFRAME != 0,
            has_minimize_box: style & WS_MINIMIZEBOX != 0,
            has_maximize_box: style & WS_MAXIMIZEBOX != 0,
            closable: class_style & CS_NOCLOSE == 0,
        }
    }
}

/// Carries out a window menu entry the way Windows' own menu does: as a system command sent
/// to the window, so the program can handle it its own way (asking to save before closing,
/// say). Every command but Minimize brings the window to the front first: Move and Size work
/// with the arrow keys, and a "save changes?" question should show. Called while one of our
/// menus still has the focus, which is what lets us hand it over.
pub fn window_command(hwnd: HWND, command: WindowCommand) {
    let sc = match command {
        WindowCommand::Restore => SC_RESTORE,
        WindowCommand::Move => SC_MOVE,
        WindowCommand::Size => SC_SIZE,
        WindowCommand::Minimize => SC_MINIMIZE,
        WindowCommand::Maximize => SC_MAXIMIZE,
        WindowCommand::Close => SC_CLOSE,
    };
    // SAFETY: calls on another program's window; PostMessageW never waits for it.
    unsafe {
        if command != WindowCommand::Minimize && SetForegroundWindow(hwnd) == 0 {
            log(&format!("SetForegroundWindow refused for {}", describe(hwnd)));
        }
        if PostMessageW(hwnd, WM_SYSCOMMAND, sc as usize, 0) == 0 {
            let why = if GetLastError() == ERROR_ACCESS_DENIED {
                " (Windows refuses most commands to programs running as administrator)"
            } else {
                ""
            };
            log(&format!("could not send {command:?} to {}{why}", describe(hwnd)));
        }
    }
}

/// Carries out a group menu entry on all the windows of a button.
pub fn group_command(windows: &[HWND], command: GroupCommand) {
    let count = windows.len() as u32;
    // SAFETY: the handle array outlives each call and its length is passed; the rest are
    // posted messages that never wait for the other program.
    unsafe {
        match command {
            GroupCommand::Cascade | GroupCommand::TileHorizontally | GroupCommand::TileVertically => {
                // Minimized windows would be left out: restore them first.
                for &hwnd in windows {
                    if IsIconic(hwnd) != 0 {
                        show_async(hwnd, SW_RESTORE);
                    }
                }
                let arranged = match command {
                    GroupCommand::Cascade => CascadeWindows(null_mut(), 0, null(), count, windows.as_ptr()),
                    GroupCommand::TileHorizontally => {
                        TileWindows(null_mut(), MDITILE_HORIZONTAL, null(), count, windows.as_ptr())
                    }
                    _ => TileWindows(null_mut(), MDITILE_VERTICAL, null(), count, windows.as_ptr()),
                };
                if arranged == 0 {
                    log(&format!("{command:?}: Windows arranged none of {count} windows"));
                }
            }
            GroupCommand::Minimize => {
                for &hwnd in windows {
                    PostMessageW(hwnd, WM_SYSCOMMAND, SC_MINIMIZE as usize, 0);
                }
            }
            GroupCommand::Close => {
                for &hwnd in windows {
                    PostMessageW(hwnd, WM_SYSCOMMAND, SC_CLOSE as usize, 0);
                }
            }
        }
    }
}

/// Starts another copy of a program (middle-click on its button), a Windows page or a web
/// address (a failure is logged).
pub fn launch(path: &str) {
    start(path);
}

/// `launch`, telling whether Windows took it.
pub fn start(path: &str) -> bool {
    let (verb, file) = (wide("open"), wide(path));
    // One of a portable copy's own programs is told where its data is (`log::flag_for`).
    let flag = crate::log::flag_for(std::path::Path::new(path)).map(wide);
    let params = flag.as_ref().map_or(null(), |f| f.as_ptr());
    // SAFETY: valid null-terminated strings.
    let result = unsafe { ShellExecuteW(null_mut(), verb.as_ptr(), file.as_ptr(), params, null(), SW_SHOWNORMAL) };
    if result as usize <= 32 {
        log(&format!("could not start {} (ShellExecute error {})", crate::log::typed_kind(path), result as usize));
        return false;
    }
    true
}

/// `bevel95 --list-windows`: writes what the bar would show to the log, for diagnosis.
pub fn log_all() {
    let mut programs = Programs::default();
    let windows = taskbar_windows();
    log("--list-windows: the lines below hold window titles and program names (asked for on purpose; the rest of the log never does)");
    log(&format!("--list-windows: {} taskbar windows", windows.len()));
    for hwnd in windows {
        let info = window_info(hwnd, &mut programs);
        let program = programs.of(hwnd);
        let has_icon = icon(hwnd, &program.path).is_some();
        log(&format!(
            "  \"{}\" | app \"{}\" | group {} | icon {} | class {} | classic picture {:?}",
            info.title,
            info.app_name,
            info.group,
            if has_icon { "yes" } else { "no" },
            class_name(hwnd),
            info.classic
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::window_kind;

    /// The owner, 2026-11-25: a program's true picture at each final size (Notepad has one up
    /// to 256), its soft edges and see-through corners kept; a file that is not there has none.
    #[test]
    fn a_programs_picture_at_its_final_size() {
        bevel_winkit::com::init();
        let notepad = bevel_winkit::shell::system_program("notepad.exe");
        for size in [24, 28, 32, 48, 56, 64, 96] {
            let icon = super::read_icon_at(&notepad, size).unwrap_or_else(|| panic!("{size}"));
            assert_eq!((icon.width, icon.height), (size, size));
            assert_eq!(icon.pixels[0] >> 24, 0, "a see-through corner at {size}");
            assert!(icon.pixels.iter().any(|p| (1..255).contains(&(p >> 24))), "soft edges at {size}");
        }
        assert!(super::read_icon_at("C:\\nothing here\\Some program.exe", 64).is_none());
    }

    /// The log says a window's kind from a fixed list, never its class (the review of brief 90,
    /// R5): a class that carries a program's name comes out as "another program's window".
    #[test]
    fn a_windows_kind_is_said_in_bevel95s_words() {
        assert_eq!(window_kind("Shell_TrayWnd"), "Windows' own taskbar");
        assert_eq!(window_kind("Windows.UI.Core.CoreWindow"), "Windows' own Start or one of its panels");
        assert_eq!(window_kind("Progman"), "Windows' own desktop");
        assert_eq!(window_kind("HwndWrapper[SomeProgram;;0f1e2d3c]"), "another program's window");
        assert_eq!(window_kind("Chrome_WidgetWin_1"), "another program's window");
    }
}
