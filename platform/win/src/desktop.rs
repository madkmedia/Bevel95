//! Bevel95's desktop: one plain window in the look's desktop color (the theme's
//! `Colors::desktop`), right on top of Windows' own desktop and under every other window. The
//! first step of a classic desktop (the owner, 2026-11-11): the color, and a right-click menu
//! that switches it off again (`bevel_core::desktop`).
//!
//! Nothing of Windows is changed: its desktop, its wallpaper and the owner's icons stay as
//! they are, underneath. So nothing needs restoring either: when the window goes (switched
//! off, Exit, a crash), Windows' desktop simply shows again.
//!
//! Where it sits: Windows keeps its desktop window ("Progman", or the "WorkerW" that holds the
//! icons) below all other windows; ours is kept directly above that one, the way RetroBar's
//! library places Cairo's desktop (ManagedShell, `WindowHelper.ShowWindowDesktop`). Windows'
//! "Show desktop" (Win+D) raises its desktop above the other windows. So ours is OWNED by
//! Windows' desktop window: an owned window always stays above its owner and Windows moves it
//! with it, in the same step, so Win+D shows the classic desktop without a frame of Windows'
//! own (measured: decisions.md, "Win+D and the classic desktop"). The window never takes the
//! focus. `check` puts it back whenever something else moved it (on Windows' events and on
//! the bar's 100 ms tick).
//!
//! Its own thread: an owned window of another process's window joins its thread's input to
//! that window's thread (Explorer's), so the window lives on a thread of its own ("classic
//! desktop"), and the bar only ever posts to it: the bar never waits for Explorer. What a click
//! on it does is posted to the bar (`WM_DESKTOP_PRESS`, `WM_DESKTOP_MENU`). When Explorer's
//! desktop window goes (Explorer restarts), Windows destroys ours with it; the bar's tick makes
//! it again once Windows' desktop is back, and the log says so.
//!
//! Its icons (evel_core::desktop_icons) are drawn the usual way: a 1× framebuffer of the whole
//! desktop, scaled by whole numbers; only what changed is painted again. The items come from
//! crate::desktop_items. A click takes the keyboard for the desktop, as the real one did, without
//! bringing it forward: see ctivate.

use std::ptr::{null, null_mut};
use std::cell::RefCell;
use std::sync::{Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;

use bevel_core::desktop::{DesktopCommand, ItemKind, ShortcutKey};
use bevel_core::desktop_icons::{Button, Desktop, EditKey, Key, Mods, RenameEnd};

use crate::file_ops::Target;
use bevel_core::fb::{Framebuffer, Rect};
use bevel_core::text::{Font, Fonts};
use bevel_core::theme::{Era, Theme};
use windows_sys::Win32::Foundation::{COLORREF, HANDLE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Dwm::{DWMWA_EXCLUDED_FROM_PEEK, DwmSetWindowAttribute};
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BeginPaint, CreateSolidBrush, DIB_RGB_COLORS, DeleteObject, EndPaint, FillRect, GetMonitorInfoW,
    InvalidateRect, MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromPoint, PAINTSTRUCT, SetDIBitsToDevice,
};
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetDoubleClickTime, GetKeyState, ReleaseCapture, SetCapture, VK_A, VK_APPS, VK_BACK, VK_C, VK_CONTROL, VK_DELETE, VK_DOWN, VK_END,
    VK_ESCAPE, VK_F2, VK_F10, VK_HOME, VK_LEFT, VK_SHIFT, VK_V, VK_X, VK_Z,
    VK_RETURN, VK_RIGHT, VK_UP,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageTime, HCBT_ACTIVATE, HHOOK, KillTimer, SM_CXDOUBLECLK, SM_CYDOUBLECLK, SetForegroundWindow, SetTimer,
    SetWindowsHookExW, UnhookWindowsHookEx, WA_INACTIVE, WH_CBT, WM_ACTIVATE, WM_CAPTURECHANGED, WM_CHAR, WM_KEYDOWN, WM_LBUTTONUP,
    WM_CLIPBOARDUPDATE, WM_CONTEXTMENU, WM_MOUSEMOVE, WM_SYSKEYDOWN, WM_TIMER,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::GetCurrentProcessId;
use windows_sys::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, EVENT_OBJECT_HIDE, EVENT_OBJECT_REORDER, EVENT_SYSTEM_FOREGROUND,
    EVENT_SYSTEM_MINIMIZESTART, FindWindowExW, FindWindowW, GW_HWNDNEXT, GW_HWNDPREV, GWL_EXSTYLE, GetCursorPos,
    GetMessageW, GetSystemMetrics, GetWindow, GetWindowLongPtrW, GetWindowRect, HWND_BOTTOM, HWND_NOTOPMOST, HWND_TOPMOST,
    IsIconic, MSG, PostMessageW, PostQuitMessage, TranslateMessage, WM_DESTROY,
    IsWindowVisible, MA_NOACTIVATE, OBJID_WINDOW, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
    SM_YVIRTUALSCREEN, SW_SHOWNA, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetPropW,
    SetWindowPos, ShowWindow, WINDOWPOS, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WM_CLOSE, WM_ERASEBKGND,
    WM_LBUTTONDOWN, WM_MBUTTONDOWN, WM_MOUSEACTIVATE, WM_PAINT, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_WINDOWPOSCHANGING,
    WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use crate::bar::BAR_HWND;
use crate::log::{log, log_win32};
use crate::util::{Private, rect_h, rect_w, wide};

pub const DESKTOP_CLASS: &str = "Bevel95Desktop";

/// Posted to the bar by the desktop's thread: a mouse button went down on the desktop (menus
/// and Windows' flyouts close, as at a press on the bar).
pub static WM_DESKTOP_PRESS: Private = Private::new("Bevel95.Desktop.Press");
/// Posted to the bar by the desktop's thread: open the desktop's menu at (wParam, lParam), the
/// pointer's place on the screen.
pub static WM_DESKTOP_MENU: Private = Private::new("Bevel95.Desktop.Menu");
/// Posted to the bar by the desktop's thread: open the menu on the selected icons (`targets`)
/// at (wParam, lParam), the pointer's place on the screen (or the focus icon's middle, from the
/// keyboard).
pub static WM_DESKTOP_ITEM_MENU: Private = Private::new("Bevel95.Desktop.ItemMenu");
/// Posted to the bar by the desktop's thread: open the extension warning over the rename box
/// (wParam 1: the name was taken with Enter); with lParam 1 the look's message for a name that
/// cannot be a file's name instead.
pub static WM_DESKTOP_RENAME_ASK: Private = Private::new("Bevel95.Desktop.RenameAsk");
/// The message `WM_DESKTOP_MENU` (the desktop's own menu).
pub fn wm_desktop_menu() -> u32 {
    WM_DESKTOP_MENU.id()
}

/// The message `WM_DESKTOP_ITEM_MENU` (the menu on icons: wParam the ticket, lParam the point).
pub fn wm_desktop_item_menu() -> u32 {
    WM_DESKTOP_ITEM_MENU.id()
}

/// Posted to the desktop window by the bar: the extension warning was answered (wParam 1: Yes).
static WM_RENAME_ANSWER: Private = Private::new("Bevel95.Desktop.RenameAnswer");
/// Posted to the desktop window by the bar: rename the icon `RENAME_REQUEST` names.
static WM_RENAME: Private = Private::new("Bevel95.Desktop.Rename");
/// Posted to the desktop window: the cut icons changed (`CUT`).
static WM_CUT: Private = Private::new("Bevel95.Desktop.Cut");
/// Posted to the desktop window by a rename's thread: Windows renamed an item (`RENAMED`).
static WM_RENAMED: Private = Private::new("Bevel95.Desktop.Renamed");
/// The caret's blink while a label is renamed.
const BLINK_TIMER: usize = 2;
/// Posted to the desktop window by the bar: run `check` (wParam: milliseconds since the tick
/// before, for the log).
static WM_CHECK: Private = Private::new("Bevel95.Desktop.Check");
/// Posted to the desktop window by the bar: go (`hide`).
static WM_GO: Private = Private::new("Bevel95.Desktop.Go");

/// The desktop window (0 while there is none).
static DESKTOP: AtomicUsize = AtomicUsize::new(0);
/// The desktop's thread is running (making, keeping or ending the window).
static THREAD: AtomicBool = AtomicBool::new(false);
/// The bar wants the desktop shown (switched on and our bar showing).
static WANTED: AtomicBool = AtomicBool::new(false);
/// Explorer's desktop window was not there when the window was to be made (logged once).
static NO_OWNER_LOGGED: AtomicBool = AtomicBool::new(false);
/// Its color as GDI wants it, kept here so that painting never waits for the bar's state.
static COLOR: AtomicU32 = AtomicU32::new(0);
/// `place` is at work (for the log: a move by anyone else is noted).
static PLACING: AtomicBool = AtomicBool::new(false);

/// A framebuffer color (0xAARRGGBB) as GDI's 0x00BBGGRR.
fn colorref(color: u32) -> COLORREF {
    ((color & 0xFF) << 16) | (color & 0xFF00) | ((color >> 16) & 0xFF)
}

fn window() -> Option<HWND> {
    let hwnd = DESKTOP.load(Ordering::SeqCst) as HWND;
    (!hwnd.is_null()).then_some(hwnd)
}

/// Whether `hwnd` is the classic desktop (the bar's own reading of the windows leaves it out
/// without sending it anything: its thread may be held up by Explorer's).
pub fn is_ours(hwnd: HWND) -> bool {
    window() == Some(hwnd)
}

/// What the desktop is drawn with: the bar's look, its font and its scale.
#[derive(Clone)]
pub struct Look {
    pub era: Era,
    pub theme: &'static Theme,
    pub font: Font,
    pub scale: bevel_core::scale::Scale,
    /// The look's wallpaper and pattern (`display_window::choice`).
    pub background: bevel_core::display::Choice,
    /// Its picture once read, when it is one to read (a JPEG, the person's own picture): read on
    /// a thread of its own, never here. Known by its reading's number; a large picture is let go
    /// once the desktop has made its own picture from it (`let_go_large`), and asked for again when
    /// that must be made anew (`pictures::needed_again`).
    pub picture: Option<crate::pictures::Got>,
}

/// The look to use, handed from the bar to the desktop's thread.
static LOOK: Mutex<Option<Look>> = Mutex::new(None);
/// Posted to the desktop window: take the look in `LOOK` (wParam 1: the screens changed).
static WM_LOOK: Private = Private::new("Bevel95.Desktop.Look");
/// Posted to the desktop window by `desktop_items`: new items are ready.
static WM_ITEMS: Private = Private::new("Bevel95.Desktop.Items");

/// Shows the desktop in `look`; if it is showing already, it takes the look (a new look, size
/// or screen). The window is made on the desktop's own thread (`run`); nothing here waits.
pub fn show(look: Look) {
    COLOR.store(colorref(look.theme.colors.desktop), Ordering::SeqCst);
    let era = look.theme.era;
    if let Ok(mut l) = LOOK.lock() {
        *l = Some(look);
    }
    WANTED.store(true, Ordering::SeqCst);
    crate::desktop_items::start(era);
    if let Some(hwnd) = window() {
        // SAFETY: a message to our own window; nothing waits for it.
        unsafe { PostMessageW(hwnd, WM_LOOK.id(), 0, 0) };
        return;
    }
    start();
}

/// `desktop_items` has read the desktop again.
pub fn items_ready() {
    if let Some(hwnd) = window() {
        // SAFETY: a message to our own window; nothing waits for it.
        unsafe { PostMessageW(hwnd, WM_ITEMS.id(), 0, 0) };
    }
}

/// Starts the desktop's thread, unless it runs already.
fn start() {
    if THREAD.swap(true, Ordering::SeqCst) {
        return;
    }
    if let Err(e) = std::thread::Builder::new().name("classic desktop".into()).spawn(run) {
        THREAD.store(false, Ordering::SeqCst);
        log(&format!("desktop: could not start its thread: {e}"));
    }
}

/// The desktop's thread: makes the window, owned by Windows' desktop window, keeps it in its
/// place (Windows' events, the bar's posted checks) and ends when the window goes.
fn run() {
    let owner = windows_desktop();
    if owner.is_null() {
        // Explorer is not running (or restarting): nothing of Windows' desktop shows either.
        // The bar's tick tries again.
        if !NO_OWNER_LOGGED.swap(true, Ordering::SeqCst) {
            log("desktop: Windows' desktop window is not there (Explorer not running?); waiting for it");
        }
        THREAD.store(false, Ordering::SeqCst);
        return;
    }
    NO_OWNER_LOGGED.store(false, Ordering::SeqCst);
    let (class, title, mark) = (wide(DESKTOP_CLASS), wide("Bevel95 Desktop"), wide("NonRudeHWND"));
    let r = virtual_screen();
    // SAFETY: a class registered by bar::run with `desktop_proc` (classes belong to the
    // process); the handle is checked; the loop is this thread's own.
    unsafe {
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class.as_ptr(),
            title.as_ptr(),
            WS_POPUP | WS_CLIPSIBLINGS | WS_CLIPCHILDREN,
            r.left,
            r.top,
            rect_w(&r),
            rect_h(&r),
            owner,
            null_mut(),
            GetModuleHandleW(null()),
            null(),
        );
        if hwnd.is_null() {
            log_win32("CreateWindowExW(desktop): no classic desktop");
            THREAD.store(false, Ordering::SeqCst);
            return;
        }
        // What Windows' own desktop window carries too: it tells Windows (and taskbars, ours
        // included) that this window filling the screen is no fullscreen app.
        SetPropW(hwnd, mark.as_ptr(), 1 as HANDLE);
        // "Peek at desktop" turns windows into glass; the desktop it peeks at stays ours.
        let yes: i32 = 1;
        DwmSetWindowAttribute(hwnd, DWMWA_EXCLUDED_FROM_PEEK as u32, (&yes as *const i32).cast(), 4);
        DESKTOP.store(hwnd as usize, Ordering::SeqCst);
        // Files dragged from anywhere (a folder window, Windows' Explorer, a program) drop here.
        bevel_winkit::dragdrop::init();
        bevel_winkit::dragdrop::register(hwnd, desktop_drop);
        rebuild(hwnd, true);
        place(hwnd);
        ShowWindow(hwnd, SW_SHOWNA);
        listen();
        // Cut icons show faded until something else goes onto the clipboard.
        if windows_sys::Win32::System::DataExchange::AddClipboardFormatListener(hwnd) == 0 {
            log_win32("AddClipboardFormatListener (cut icons then stay faded until the next Cut or Copy)");
        }
        let hook = SetWindowsHookExW(WH_CBT, Some(cbt), null_mut(), GetCurrentThreadId());
        if hook.is_null() {
            log_win32("SetWindowsHookExW(WH_CBT): a click on the desktop could bring it forward, so it will not take the keyboard");
        }
        CBT_HOOK.store(hook as usize, Ordering::SeqCst);
        log(&format!(
            "desktop: showing over Windows' desktop ({}x{}), owned by {}",
            rect_w(&r),
            rect_h(&r),
            crate::window_list::kind_of(owner)
        ));
        // The bar may have switched it off meanwhile.
        if !WANTED.load(Ordering::SeqCst) {
            DESKTOP.store(0, Ordering::SeqCst);
            DestroyWindow(hwnd);
        }
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    stop_listening();
    let hook = CBT_HOOK.swap(0, Ordering::SeqCst);
    if hook != 0 {
        // SAFETY: the hook this thread set.
        unsafe { UnhookWindowsHookEx(hook as HHOOK) };
    }
    STATE.with(|c| c.borrow_mut().take());
    THREAD.store(false, Ordering::SeqCst);
}

/// Takes the desktop away: Windows' own shows again. The window goes on its own thread.
pub fn hide() {
    WANTED.store(false, Ordering::SeqCst);
    // The picture handed over is not kept while the desktop is away.
    if let Ok(mut l) = LOOK.lock()
        && let Some(look) = l.as_mut()
    {
        look.picture = None;
    }
    let hwnd = DESKTOP.swap(0, Ordering::SeqCst) as HWND;
    if !hwnd.is_null() {
        // SAFETY: a message to our own window; nothing waits for it.
        unsafe { PostMessageW(hwnd, WM_GO.id(), 0, 0) };
        crate::desktop_items::stop();
        log("desktop: gone; Windows' own desktop shows again");
    }
}

/// All screens together, in screen pixels.
fn virtual_screen() -> RECT {
    // SAFETY: plain queries.
    unsafe {
        let (x, y) = (GetSystemMetrics(SM_XVIRTUALSCREEN), GetSystemMetrics(SM_YVIRTUALSCREEN));
        let (w, h) = (GetSystemMetrics(SM_CXVIRTUALSCREEN), GetSystemMetrics(SM_CYVIRTUALSCREEN));
        RECT { left: x, top: y, right: x + w, bottom: y + h }
    }
}

/// Windows' own desktop window: "Progman" while it holds the icons, else the "WorkerW" that
/// does (Windows moves them there for some wallpapers). Null if there is none (Explorer is not
/// running).
fn windows_desktop() -> HWND {
    let (progman, worker, icons) = (wide("Progman"), wide("WorkerW"), wide("SHELLDLL_DefView"));
    // SAFETY: valid null-terminated strings; `after` is null or a window this loop just found.
    unsafe {
        let shell = FindWindowW(progman.as_ptr(), null());
        if !shell.is_null() && !FindWindowExW(shell, null_mut(), icons.as_ptr(), null()).is_null() {
            return shell;
        }
        let mut after: HWND = null_mut();
        loop {
            after = FindWindowExW(null_mut(), after, worker.as_ptr(), null());
            if after.is_null() {
                return shell;
            }
            if !FindWindowExW(after, null_mut(), icons.as_ptr(), null()).is_null() {
                return after;
            }
        }
    }
}

/// Puts the window directly above Windows' desktop window, over the whole of every screen.
fn place(hwnd: HWND) {
    let r = virtual_screen();
    let below = windows_desktop();
    // SAFETY: our own window; its neighbor is only named, never changed.
    unsafe {
        // The window just above Windows' desktop: ours goes right behind it, which is right in
        // front of the desktop. Without a desktop window, ours goes below everything.
        let after = if below.is_null() { HWND_BOTTOM } else { GetWindow(below, GW_HWNDPREV) };
        let flags = SWP_NOACTIVATE | if after == hwnd { SWP_NOZORDER } else { 0 };
        PLACING.store(true, Ordering::SeqCst);
        if SetWindowPos(hwnd, after, r.left, r.top, rect_w(&r), rect_h(&r), flags) == 0 {
            log_win32("SetWindowPos(desktop)");
        }
        // It must never be one of the windows that stay above all others (it would hide them).
        if GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST != 0 {
            SetWindowPos(hwnd, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            log("desktop: it had become a topmost window; put back among the ordinary ones");
        }
        PLACING.store(false, Ordering::SeqCst);
    }
}

/// Why the window was found out of its place (`check`); each has its own count in the log.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Off {
    /// Something hid it (it was not visible).
    Hidden,
    /// Something minimized it.
    Minimized,
    /// Windows' desktop had come up over it ("Show desktop" raises it): Windows' own desktop
    /// showed wherever no other window covered it.
    Under,
    /// Another window had come between it and Windows' desktop (under ours: nothing showed).
    Between,
    /// The screens changed.
    Size,
}

impl Off {
    fn index(self) -> usize {
        self as usize
    }
}

/// How often each `Off` was found (for the log's "time n").
static FOUND: [AtomicU32; 5] = [const { AtomicU32::new(0) }; 5];
/// When the tick last checked (milliseconds since `started`), to tell how late a tick came.
static LAST_TICK_MS: AtomicU64 = AtomicU64::new(0);
/// The window-event hooks that let `check` run at once (0 while there are none).
static HOOKS: [AtomicUsize; 4] = [const { AtomicUsize::new(0) }; 4];

fn started() -> Instant {
    static STARTED: OnceLock<Instant> = OnceLock::new();
    *STARTED.get_or_init(Instant::now)
}

/// The bar's 100 ms tick: asks the desktop's thread to `check` (and remembers when it ran);
/// makes the window again if it went without being switched off (Explorer's desktop window,
/// its owner, went: Windows destroys owned windows with their owner).
pub fn tick() {
    let now = started().elapsed().as_millis() as u64;
    let previous = LAST_TICK_MS.swap(now, Ordering::SeqCst);
    match window() {
        // SAFETY: a message to our own window; nothing waits for it.
        Some(hwnd) => unsafe {
            PostMessageW(hwnd, WM_CHECK.id(), now.saturating_sub(previous) as usize, 0);
        },
        None if WANTED.load(Ordering::SeqCst) && !THREAD.load(Ordering::SeqCst) => start(),
        None => {}
    }
}

/// Keeps the window where it belongs: shown, not minimized, directly above Windows' desktop
/// (which "Show desktop" raises and lowers, and under which Windows puts minimized windows)
/// and as large as all screens together. Runs on the bar's tick and at once on Windows' events
/// (`on_event`); `by` says which, `gap` how long ago the tick before ran (for the log).
fn check(by: &str, gap: u64) {
    let Some(hwnd) = window() else {
        return;
    };
    let below = windows_desktop();
    let want = virtual_screen();
    // SAFETY: read-only queries; any window handle.
    let off = unsafe {
        let mut now = RECT::default();
        GetWindowRect(hwnd, &mut now);
        let sized = (now.left, now.top, now.right, now.bottom) == (want.left, want.top, want.right, want.bottom);
        let next = if below.is_null() { null_mut() } else { GetWindow(below, GW_HWNDPREV) };
        if IsWindowVisible(hwnd) == 0 {
            Some(Off::Hidden)
        } else if IsIconic(hwnd) != 0 {
            Some(Off::Minimized)
        } else if !below.is_null() && next != hwnd {
            Some(if above(hwnd, below) { Off::Between } else { Off::Under })
        } else if !sized {
            Some(Off::Size)
        } else {
            None
        }
    };
    let Some(off) = off else {
        return;
    };
    // What came between, said before `place` moves things (class of a hidden helper window
    // only: a visible one is a program's own window).
    let between = if off == Off::Between {
        // SAFETY: read-only queries.
        let w = unsafe { GetWindow(below, GW_HWNDPREV) };
        // SAFETY: as above.
        match unsafe { (IsWindowVisible(w) != 0, IsIconic(w) != 0) } {
            (false, _) => format!("a hidden window ({})", crate::window_list::kind_of(w)),
            (true, true) => String::from("a minimized window"),
            (true, false) => String::from("a program's window"),
        }
    } else {
        String::new()
    };
    if matches!(off, Off::Hidden | Off::Minimized) {
        // SAFETY: our own window, shown (and restored) without taking the focus.
        unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE) };
    }
    place(hwnd);
    if off == Off::Size {
        rebuild(hwnd, true);
    }
    // Routine: a window that is minimized lands between the two desktops, and now and then so
    // does a program's hidden helper window (seen live: the "IME" window of a program that had
    // just loaded GDI+). So only the first few of each reason are worth a line, and after them
    // every tenfold count: a window that kept coming back shows as a count that keeps growing.
    let n = FOUND[off.index()].fetch_add(1, Ordering::SeqCst) + 1;
    if worth_a_line(n) {
        let why = match off {
            Off::Hidden => String::from("it had been hidden; shown again"),
            Off::Minimized => String::from("it had been minimized; shown again"),
            Off::Under => String::from("Windows' desktop had come up over it (Show desktop?): Windows' desktop showed"),
            Off::Between => format!("{between} had come between it and Windows' desktop (nothing showed)"),
            Off::Size => String::from("the screens changed"),
        };
        let late = if by == "the tick" && gap > 300 { format!("; that tick came {gap} ms after the one before") } else { String::new() };
        log(&format!("desktop: put back in its place, found by {by}: {why} (time {n}, only some are logged{late})"));
    }
}

/// Whether `ours` lies above `desktop` among the windows (walking down from ours).
fn above(ours: HWND, desktop: HWND) -> bool {
    let mut w = ours;
    for _ in 0..10_000 {
        // SAFETY: a read-only query.
        w = unsafe { GetWindow(w, GW_HWNDNEXT) };
        if w.is_null() {
            return false;
        }
        if w == desktop {
            return true;
        }
    }
    false
}

/// Listens to Windows' events that can move the window out of its place, so that `check` runs
/// at once instead of at the next tick (the tick can be late while the bar is busy): another
/// window coming to the front (Windows' desktop does, at "Show desktop"), the order of windows
/// changing, and our window being minimized or hidden. Out of context: nothing runs in another
/// program; the events arrive as messages on the bar's thread.
fn listen() {
    // SAFETY: plain calls; the callback is a valid function for the life of the program.
    unsafe {
        let pid = GetCurrentProcessId();
        let others = WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS;
        let own = WINEVENT_OUTOFCONTEXT;
        let hooks = [
            SetWinEventHook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND, null_mut(), Some(on_event), 0, 0, others),
            SetWinEventHook(EVENT_OBJECT_REORDER, EVENT_OBJECT_REORDER, null_mut(), Some(on_event), 0, 0, others),
            SetWinEventHook(EVENT_OBJECT_HIDE, EVENT_OBJECT_HIDE, null_mut(), Some(on_event), pid, 0, own),
            SetWinEventHook(EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZESTART, null_mut(), Some(on_event), pid, 0, own),
        ];
        for (slot, hook) in HOOKS.iter().zip(hooks) {
            if hook.is_null() {
                log_win32("SetWinEventHook (the desktop then waits for the tick)");
            }
            let old = slot.swap(hook as usize, Ordering::SeqCst);
            if old != 0 {
                UnhookWinEvent(old as HWINEVENTHOOK);
            }
        }
    }
}

fn stop_listening() {
    for slot in &HOOKS {
        let hook = slot.swap(0, Ordering::SeqCst);
        if hook != 0 {
            // SAFETY: a hook set by `listen`.
            unsafe { UnhookWinEvent(hook as HWINEVENTHOOK) };
        }
    }
}

/// A window event (`listen`). Of our own process's hide and minimize events only our window's
/// count.
unsafe extern "system" fn on_event(_: HWINEVENTHOOK, event: u32, hwnd: HWND, object: i32, _: i32, _: u32, _: u32) {
    if object != OBJID_WINDOW {
        return;
    }
    let ours = window();
    match event {
        EVENT_SYSTEM_FOREGROUND => check("Windows' event: another window came to the front", 0),
        EVENT_OBJECT_REORDER => check("Windows' event: the order of windows changed", 0),
        EVENT_OBJECT_HIDE if ours == Some(hwnd) => check("Windows' event: it was hidden", 0),
        EVENT_SYSTEM_MINIMIZESTART if ours == Some(hwnd) => check("Windows' event: it was minimized", 0),
        _ => {}
    }
}

use bevel_winkit::log::{counted, worth_a_line};

/// Posted to the desktop window: Arrange Icons (wParam: the `Arrange` by its place in `ALL`), or
/// `LAYOUT_AUTO` (Auto Arrange on or off) or `LAYOUT_LINE_UP` (Line up Icons).
static WM_ARRANGE: Private = Private::new("Bevel95.Desktop.Arrange");
const LAYOUT_AUTO: usize = 100;
const LAYOUT_LINE_UP: usize = 101;
/// Sent by a live test (`private/probes/desktop-move`): which icon is at (wParam, lParam), window
/// pixels? Answers with a number made from its item's id (`id_code`), or 0 if none is there. It
/// only answers, so a test can make sure an icon is its own before it touches it.
pub static WM_ICON_AT: Private = Private::new("Bevel95.Desktop.IconAt");
/// Sent by a live test: where is the icon whose `id_code` is wParam? Answers with its picture's
/// top-left corner in window pixels, x + y * 65536, or 0 if there is no such icon; lParam 1 asks
/// instead whether it is selected (1) or not (0).
pub static WM_ICON_PLACE: Private = Private::new("Bevel95.Desktop.IconPlace");
/// Sent by a live test (`private/probes/desktop-menu/live.ps1`): carry out command lParam (its
/// `DesktopCommand::code`) on the icon whose `id_code` is wParam, selected alone first, as a key
/// on it would. Only on the test's own items (named "Bevel95 test ..."): any other icon is
/// refused (answer 0). wParam 0: no icon (Paste, Paste Shortcut).
pub static WM_TEST_COMMAND: Private = Private::new("Bevel95.Desktop.TestCommand");
/// Sent by a live test in WM_COPYDATA to the desktop window: a copy or move of test items "from
/// another program" into a test folder, carried out on a thread of the bar's program as a drop is
/// (`file_ops::drop_files`, the effect narrowed as a drop's). The text: the offered DROPEFFECT
/// bits, the folder and the files, separated by tabs. Only "Bevel95 test" items into a "Bevel95
/// test" folder; anything else is refused and logged. Answered with the effect carried out.
const TEST_DROP: usize = 0xB95D;
/// Sent by a live test in WM_COPYDATA to the desktop window: Windows' own Properties window of a
/// test file (its path the text), shown in the bar's program as the desktop's Properties does.
/// Only a "Bevel95 test" file in a "Bevel95 test" folder; anything else is refused and logged.
/// Answered with 1 when it was asked for. (The second safety review of 2026-11-18, B1: a
/// Properties window left open must not hold the bar's process.)
const TEST_PROPERTIES: usize = 0xB95E;

/// Which commands a live test may give (`WM_TEST_COMMAND`): on one of its own icons (`on_item`)
/// the icon's own commands; with or without one, Paste, Paste Shortcut and Undo, which the bar
/// then carries out on test items only (`file_ops::for_a_test`), and Refresh. Nothing that acts on
/// the person's own (Empty Recycle Bin, Arrange Icons, Line Up, Auto Arrange, New, switching the
/// desktop off).
pub fn test_command_allowed(command: DesktopCommand, on_item: bool) -> bool {
    use DesktopCommand::*;
    match command {
        Open | Explore | Cut | Copy | CreateShortcut | Delete | Rename | ItemProperties => on_item,
        Paste | PasteShortcut | Undo | Refresh => true,
        _ => false,
    }
}

/// A live test's Properties window (`TEST_PROPERTIES`).
fn test_properties(text: &str) -> u32 {
    let path = std::path::Path::new(text);
    let test_name = |p: &std::path::Path| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("Bevel95 test"));
    if !(test_name(path) && path.is_file() && path.parent().is_some_and(test_name)) {
        log("desktop: a test's Properties was refused (only a test file in a test folder)");
        return 0;
    }
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    crate::file_ops::properties(&[crate::file_ops::Target { id: text.to_string(), name, kind: ItemKind::File }]);
    log("desktop: a test's Properties window was asked for");
    1
}

/// A live test's copy or move (`TEST_DROP`): "offered\tfolder\tfile\tfile...".
fn test_drop(text: &str) -> u32 {
    use bevel_winkit::dragdrop::{Dest, Over};
    let mut parts = text.split('\t');
    let offered = parts.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
    let dir = std::path::PathBuf::from(parts.next().unwrap_or(""));
    let files: Vec<String> = parts.map(str::to_string).collect();
    let test_name = |p: &std::path::Path| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("Bevel95 test"));
    let ok = test_name(&dir)
        && dir.is_dir()
        && !files.is_empty()
        && files.iter().all(|f| test_name(std::path::Path::new(f)) && std::path::Path::new(f).parent().is_some_and(test_name));
    if !ok {
        log("desktop: a test drop was refused (only test items into a test folder)");
        return 0;
    }
    let era = STATE.with(|c| c.borrow().as_ref().map(|st| st.view.era)).unwrap_or(bevel_core::theme::Era::Win95);
    let over = Over { x: 0, y: 0, keys: Default::default(), right: false, files: files.clone(), own: None, offered };
    let dest = Dest::Folder(dir.clone());
    let Some(effect) = bevel_winkit::dragdrop::drop_effect(era, &dest, &over) else { return 0 };
    crate::file_ops::drop_files(files, dir, effect, |_| {});
    log("desktop: a test drop was started");
    match effect {
        bevel_core::drag::Effect::Move => 2,
        bevel_core::drag::Effect::Copy => 1,
        bevel_core::drag::Effect::Link => 4,
    }
}
/// The timer that repaints a drag or the selection rectangle (only while one lasts), and its
/// step: about one frame of a 60 Hz screen, as the bar's task buttons do.
const DRAG_TIMER: usize = 1;
const DRAG_MS: u32 = 15;
/// The icons read again while a press is held or a double-click may still come are taken in a
/// little later, when this timer finds neither (brief 90: the press and the double-click stay
/// on the icons the person aimed at).
const ITEMS_LATER_TIMER: usize = 3;
/// New icons are waiting for `ITEMS_LATER_TIMER` (counted once per wait in the log).
static ITEMS_WAITING: AtomicBool = AtomicBool::new(false);

/// Whether a press the desktop recorded (`gesture`) still holds: only while a mouse button is
/// down (a right press let go over another window leaves no release here: the review of brief 90,
/// R4).
fn press_holds(gesture: bool, button_down: bool) -> bool {
    gesture && button_down
}

/// How many milliseconds new icons must still wait after a left press at message time `pressed`
/// (now `now`, Windows' double-click time `double`), so that a second press of a double-click
/// finds the icons where the first one did: 0 when none was made or its time is over.
fn items_wait(pressed: Option<u32>, now: u32, double: u32) -> u32 {
    pressed.map_or(0, |t| {
        let gone = now.wrapping_sub(t);
        if gone < double { double - gone + 10 } else { 0 }
    })
}
/// The keys held with a click (wParam of a mouse message): MK_SHIFT and MK_CONTROL.
const MK_SHIFT: usize = 0x4;
const MK_CONTROL: usize = 0x8;

/// A new item the desktop's menu made: selected once it shows (its path).
static PENDING_SELECT: Mutex<Option<String>> = Mutex::new(None);
/// The same item and where its picture goes (the point the menu was opened at, desktop pixels at
/// 1×), handed to the desktop's state before the item shows.
static PENDING_PLACE: Mutex<Option<(String, i32, i32)>> = Mutex::new(None);
/// Where the desktop's menu was last opened (desktop pixels at 1×).
static MENU_AT: Mutex<Option<(i32, i32)>> = Mutex::new(None);
/// Auto Arrange is on (for the tick in the Arrange Icons menu, which the bar opens).
static AUTO_ARRANGE: AtomicBool = AtomicBool::new(false);

/// A number standing for an item's id in the live tests' queries (FNV-1a over its UTF-8 bytes,
/// never 0): the test works out the same number from its own test file's path.
pub fn id_code(id: &str) -> u32 {
    let mut h: u32 = 0x811C_9DC5;
    for b in id.bytes() {
        h ^= u32::from(b);
        h = h.wrapping_mul(0x0100_0193);
    }
    h.max(1)
}

/// The icons a menu on icons (or a key) acts on: the selection when it was opened, each under a
/// ticket of its own that travels with the bar's message (`remember_targets`). One shared slot let
/// a later click or key replace what an earlier key's command or an open menu acted on (the third
/// safety review of 2026-11-18, suspected 2). The last few are kept.
static TARGETS: Mutex<Vec<(u32, Vec<Target>, bool)>> = Mutex::new(Vec::new());
static NEXT_TICKET: AtomicU32 = AtomicU32::new(1);
/// How many remembered selections are kept (a menu stays open while keys go on).
const TICKETS_KEPT: usize = 16;
/// Whether the Recycle Bin holds anything (as the desktop last read it).
static RECYCLE_FULL: AtomicBool = AtomicBool::new(false);
/// The icons cut onto the clipboard, and the clipboard's sequence number right after (they show
/// faded until the clipboard changes).
static CUT: Mutex<Vec<String>> = Mutex::new(Vec::new());
static CUT_SEQ: AtomicU32 = AtomicU32::new(0);
/// The icon the bar asked to rename (`rename`).
static RENAME_REQUEST: Mutex<Option<String>> = Mutex::new(None);
/// Items Windows renamed: (old id, new id, the name typed), for the desktop's thread.
static RENAMED: Mutex<Vec<(String, String, String)>> = Mutex::new(Vec::new());

/// Remembers `targets` under a new ticket (never 0, below 2^30 so that it fits a message's
/// parameter beside a flag).
fn remember_targets(targets: Vec<Target>) -> u32 {
    remember(targets, false)
}

/// As `remember_targets`, for a live test's command (`WM_TEST_COMMAND`): the bar carries it out
/// on test items only (`is_test_ticket`).
fn remember_test_targets(targets: Vec<Target>) -> u32 {
    remember(targets, true)
}

fn remember(targets: Vec<Target>, test: bool) -> u32 {
    let ticket = loop {
        let t = NEXT_TICKET.fetch_add(1, Ordering::SeqCst) & 0x3FFF_FFFF;
        if t != 0 {
            break t;
        }
    };
    if let Ok(mut all) = TARGETS.lock() {
        all.push((ticket, targets, test));
        let extra = all.len().saturating_sub(TICKETS_KEPT);
        all.drain(..extra);
    }
    ticket
}

/// The icons the command or menu with `ticket` acts on (none for ticket 0 or one long gone).
pub fn targets_of(ticket: u32) -> Vec<Target> {
    if ticket == 0 {
        return Vec::new();
    }
    TARGETS.lock().ok().and_then(|all| all.iter().find(|(t, ..)| *t == ticket).map(|(_, targets, _)| targets.clone())).unwrap_or_default()
}

/// Whether `ticket` is a live test's command's (`remember_test_targets`).
pub fn is_test_ticket(ticket: u32) -> bool {
    ticket != 0 && TARGETS.lock().ok().is_some_and(|all| all.iter().any(|(t, _, test)| *t == ticket && *test))
}

/// The bar's command message's lParam: the ticket of its targets and whether a key asked.
pub fn command_param(ticket: u32, by_keyboard: bool) -> LPARAM {
    ((ticket as LPARAM) << 1) | LPARAM::from(by_keyboard)
}

/// The ticket and the keyboard flag out of `command_param`.
pub fn command_of_param(lparam: LPARAM) -> (u32, bool) {
    (((lparam as usize) >> 1) as u32 & 0x3FFF_FFFF, lparam & 1 == 1)
}

/// A screen point for the menu on icons' lParam (its wParam carries the ticket).
fn point_param(x: i32, y: i32) -> LPARAM {
    (((y as i16 as u16 as u32) << 16) | (x as i16 as u16 as u32)) as LPARAM
}

/// The screen point out of `point_param`.
pub fn point_of_param(lparam: LPARAM) -> (i32, i32) {
    (i32::from(lparam as u16 as i16), i32::from((lparam >> 16) as u16 as i16))
}

/// Whether the Recycle Bin holds anything (Empty Recycle Bin is grayed otherwise).
pub fn recycle_full() -> bool {
    RECYCLE_FULL.load(Ordering::SeqCst)
}

fn post_to_desktop(msg: u32) {
    if let Some(hwnd) = window() {
        // SAFETY: a message to our own window; nothing waits for it.
        unsafe { PostMessageW(hwnd, msg, 0, 0) };
    }
}

/// Rename: the icon's label becomes a box to type in (on the desktop's thread).
pub fn rename(id: String) {
    if let Ok(mut r) = RENAME_REQUEST.lock() {
        *r = Some(id);
    }
    post_to_desktop(WM_RENAME.id());
}

/// The extension warning was answered (from the bar's thread): Yes renames, No opens the box
/// again.
pub fn rename_warning_answered(yes: bool) {
    if let Some(hwnd) = window() {
        // SAFETY: a message to our own window; nothing waits for it.
        unsafe { PostMessageW(hwnd, WM_RENAME_ANSWER.id(), usize::from(yes), 0) };
    }
}

/// The icons now cut onto the clipboard (`seq`: the clipboard's sequence number right after).
pub fn set_cut(ids: Vec<String>, seq: u32) {
    if let Ok(mut c) = CUT.lock() {
        *c = ids;
    }
    CUT_SEQ.store(seq, Ordering::SeqCst);
    post_to_desktop(WM_CUT.id());
}

/// A new item (a shortcut made by the menu) is selected once the desktop shows it.
pub fn select_when_shown(path: String) {
    if let Ok(mut pending) = PENDING_SELECT.lock() {
        *pending = Some(path);
    }
    crate::desktop_items::read_again();
}

/// The selected icons as the commands' targets.
fn selection_targets(view: &Desktop) -> Vec<Target> {
    view.selection()
        .into_iter()
        .map(|i| {
            let item = &view.items[i];
            Target { id: item.id.clone(), name: item.name.clone(), kind: ItemKind::of(item) }
        })
        .collect()
}

/// The places file: where each icon lies, Arrange Icons' order and Auto Arrange.
fn places_file() -> std::path::PathBuf {
    crate::log::data_dir().join("desktop.txt")
}

/// Whether Auto Arrange is on (the Arrange Icons menu shows a tick).
pub fn auto_arrange() -> bool {
    AUTO_ARRANGE.load(Ordering::SeqCst)
}

fn post_layout(code: usize) {
    if let Some(hwnd) = window() {
        // SAFETY: a message to our own window; nothing waits for it.
        unsafe { PostMessageW(hwnd, WM_ARRANGE.id(), code, 0) };
    }
}

/// Arrange Icons: orders the owner's icons again, every icon into columns.
pub fn arrange(by: bevel_core::desktop_icons::Arrange) {
    post_layout(bevel_core::desktop_icons::Arrange::ALL.iter().position(|&a| a == by).unwrap_or(0));
}

/// Auto Arrange on or off.
pub fn toggle_auto_arrange() {
    post_layout(LAYOUT_AUTO);
}

/// Line up Icons.
pub fn line_up() {
    post_layout(LAYOUT_LINE_UP);
}

/// New > Folder or New > Text Document: makes it in the owner's Desktop folder under Windows'
/// own default name ("New folder", "New Text Document.txt", then "(2)" and so on), nothing else;
/// once the desktop shows it, it is selected, its picture where the menu was opened. Renaming it
/// comes with the icons' own menu.
pub fn new_item(folder: bool) {
    let Some(dir) = crate::desktop_items::desktop_folder() else {
        log("desktop: New: the Desktop folder was not found");
        return;
    };
    let (base, extension) = if folder { ("New folder", "") } else { ("New Text Document", ".txt") };
    let name = bevel_core::desktop::new_name(base, extension, |n| dir.join(n).exists());
    let path = dir.join(&name);
    let id = path.to_string_lossy().into_owned();
    // Where it goes is known before it exists (the desktop reads its folders on its own thread).
    if let Some((x, y)) = MENU_AT.lock().ok().and_then(|m| *m)
        && let Ok(mut pending) = PENDING_PLACE.lock()
    {
        *pending = Some((id.clone(), x, y));
    }
    let made = if folder {
        std::fs::create_dir(&path)
    } else {
        std::fs::OpenOptions::new().write(true).create_new(true).open(&path).map(|_| ())
    };
    match made {
        Ok(()) => {
            log(if folder { "desktop: New made a folder" } else { "desktop: New made a text document" });
            if let Ok(mut pending) = PENDING_SELECT.lock() {
                *pending = Some(id);
            }
            crate::desktop_items::read_again();
        }
        Err(e) => log(&format!("desktop: New could not make {}: {e}", if folder { "a folder" } else { "a text document" })),
    }
}

/// The desktop's icons and their pixels, kept on the desktop's thread.
struct State {
    look: Look,
    view: Desktop,
    /// The whole desktop at 1× (all screens), painted where something changed.
    fb: Framebuffer,
    /// Every screen in the look's 1x pixels of `fb`, the main one first (what the picture was
    /// made for).
    screens: Vec<Rect>,
    /// The last left press: when (message time), where (window pixels), on which icon (its id:
    /// items read between the two presses move the indices).
    last_press: Option<(u32, i32, i32, Option<String>)>,
    /// The icon the right button went down on (none: the desktop's menu at its release).
    right_hit: Option<usize>,
    /// The pointer's latest place during a drag or a selection rectangle (desktop pixels at 1×),
    /// taken in on the next step of the timer.
    pointer: Option<(i32, i32)>,
    /// Since when the drag timer runs, with how many steps painted something (for the log).
    timer: Option<(Instant, u32)>,
    /// The rename box's caret is shown (it blinks).
    blink_on: bool,
}

impl State {
    /// Whether a left press on icon `hit` at (x, y) is the second of a double-click (Windows'
    /// double-click time and distance), and remembers it for the next one.
    fn second_click(&mut self, hit: Option<usize>, x: i32, y: i32) -> bool {
        // SAFETY: plain queries.
        let (now, time, dx, dy) = unsafe {
            (GetMessageTime() as u32, GetDoubleClickTime(), GetSystemMetrics(SM_CXDOUBLECLK), GetSystemMetrics(SM_CYDOUBLECLK))
        };
        let hit = hit.and_then(|i| self.view.items.get(i)).map(|item| item.id.clone());
        let near = self.last_press.as_ref().is_some_and(|(t, px, py, _)| now.wrapping_sub(*t) <= time && (x - px).abs() * 2 <= dx && (y - py).abs() * 2 <= dy);
        let double = near && self.last_press.as_ref().is_some_and(|(.., was)| *was == hit);
        if near && !double {
            static ELSEWHERE: AtomicU32 = AtomicU32::new(0);
            counted(&ELSEWHERE, "desktop: a second click came in time but on another icon than the first");
        }
        self.last_press = if double { None } else { Some((now, x, y, hit)) };
        double
    }
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

/// The desktop is being activated by Windows (seen by `cbt` before Windows moves it): the move
/// to the top of all windows that comes with activation is refused in `WM_WINDOWPOSCHANGING`.
static ACTIVATING: AtomicBool = AtomicBool::new(false);
/// The thread's CBT hook (0 while there is none). Without it the desktop never takes the
/// keyboard (`activate`).
static CBT_HOOK: AtomicUsize = AtomicUsize::new(0);

/// Runs `change` on the state and paints again what it says changed (desktop pixels at 1×).
fn with_state(hwnd: HWND, change: impl FnOnce(&mut State, &Font) -> Vec<Rect>) {
    with_state_value(hwnd, |st, font| ((), change(st, font)));
}

fn with_state_value<R>(hwnd: HWND, change: impl FnOnce(&mut State, &Font) -> (R, Vec<Rect>)) -> Option<R> {
    STATE.with(|c| {
        let mut borrow = c.borrow_mut();
        let st = borrow.as_mut()?;
        let font = st.look.font.clone();
        let (result, dirty) = change(st, &font);
        for r in dirty {
            repaint(hwnd, st, r);
        }
        save_if_changed(&mut st.view);
        Some(result)
    })
}

/// Writes the places file when the places, the arrangement or Auto Arrange changed.
fn save_if_changed(view: &mut Desktop) {
    AUTO_ARRANGE.store(view.auto_arrange(), Ordering::SeqCst);
    if view.take_changed()
        && let Err(e) = bevel_winkit::state_file::save(&places_file(), &view.places_text())
        && !bevel_winkit::state_file::is_read_only(&e)
    {
        log(&format!("desktop: could not save where the icons lie: {e}"));
    }
}

/// The whole desktop (after the items or the arrangement changed).
fn everything(st: &State) -> Vec<Rect> {
    vec![Rect::new(0, 0, st.fb.width, st.fb.height)]
}

/// Paints the 1× rectangle `r` of the desktop again and has Windows show it.
fn repaint(hwnd: HWND, st: &mut State, r: Rect) {
    let r = Rect::new(r.x.max(0), r.y.max(0), r.right().min(st.fb.width) - r.x.max(0), r.bottom().min(st.fb.height) - r.y.max(0));
    if r.w <= 0 || r.h <= 0 {
        return;
    }
    let fonts = Fonts { regular: &st.look.font, bold: &st.look.font };
    st.view.paint(&mut st.fb, st.look.theme, &fonts, r);
    let d = st.look.scale.dirty(r);
    let rc = RECT { left: d.x, top: d.y, right: d.right(), bottom: d.bottom() };
    // SAFETY: our own window (InvalidateRect sends nothing).
    unsafe { InvalidateRect(hwnd, &rc, 0) };
}

/// The main screen's free area (without the bar), in the desktop window's pixels.
fn work_area(screen: &RECT) -> RECT {
    let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
    // SAFETY: a valid out-struct; the main screen always exists.
    unsafe { GetMonitorInfoW(MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY), &mut info) };
    let w = info.rcWork;
    RECT { left: w.left - screen.left, top: w.top - screen.top, right: w.right - screen.left, bottom: w.bottom - screen.top }
}

/// The same picture read (the same reading, not an equal picture: a picture read again is drawn
/// again; one let go is still the same reading).
fn same_picture(a: &Option<crate::pictures::Got>, b: &Option<crate::pictures::Got>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a.reading == b.reading,
        (None, None) => true,
        _ => false,
    }
}

/// The desktop's own picture has been made from the look's picture: the bar is told it has drawn
/// that reading (`pictures::drawn`: only now may the bar's slot let a large picture go, the review
/// before 0.1.4, C1), and a large picture is let go here too: from the state's look and from the
/// look handed over (`LOOK`), if that is still the same reading.
fn let_go_large(look: &mut Look) {
    let Some(got) = look.picture.as_mut() else { return };
    let Some(picture) = got.picture.as_deref() else { return };
    let (reading, large) = (got.reading, crate::pictures::is_large(picture));
    if large {
        got.picture = None;
        if let Ok(mut l) = LOOK.lock()
            && let Some(g) = l.as_mut().and_then(|l| l.picture.as_mut())
            && g.reading == reading
        {
            g.picture = None;
        }
    }
    crate::pictures::drawn(reading);
}

/// Every screen's rectangle in the desktop window's pixels (relative to `desktop`, the window's
/// place), the main screen first; empty if Windows says nothing (the caller then takes the whole
/// desktop as one screen).
fn screens(desktop: &RECT) -> Vec<RECT> {
    unsafe extern "system" fn each(monitor: windows_sys::Win32::Graphics::Gdi::HMONITOR, _: windows_sys::Win32::Graphics::Gdi::HDC, _: *mut RECT, data: LPARAM) -> windows_sys::core::BOOL {
        // SAFETY: `data` is the `Vec` of `screens`, alive for the whole enumeration on this thread.
        let found = unsafe { &mut *(data as *mut Vec<(bool, RECT)>) };
        let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
        // SAFETY: a monitor Windows just named; a valid out-struct.
        if unsafe { GetMonitorInfoW(monitor, &mut info) } != 0 {
            found.push((info.dwFlags & windows_sys::Win32::UI::WindowsAndMessaging::MONITORINFOF_PRIMARY != 0, info.rcMonitor));
        }
        1
    }
    let mut found: Vec<(bool, RECT)> = Vec::new();
    // SAFETY: the callback only touches `found`, which outlives the call.
    unsafe { windows_sys::Win32::Graphics::Gdi::EnumDisplayMonitors(null_mut(), null(), Some(each), &mut found as *mut _ as LPARAM) };
    // The main screen first, the others in Windows' order.
    found.sort_by_key(|(main, _)| !*main);
    found
        .into_iter()
        .map(|(_, r)| RECT { left: r.left - desktop.left, top: r.top - desktop.top, right: r.right - desktop.left, bottom: r.bottom - desktop.top })
        .collect()
}

/// Hands a new item's place (`new_item`) to the state before the item shows.
fn take_pending_place(view: &mut Desktop) {
    if let Some((id, x, y)) = PENDING_PLACE.lock().ok().and_then(|mut p| p.take()) {
        view.place_new(&id, x, y);
    }
}

/// Takes the bar's look (and the screens' sizes) into the state, keeping the icons and their
/// selection; does nothing if nothing changed, unless `force`. The first time, the places file is
/// read before the items.
fn rebuild(hwnd: HWND, force: bool) {
    let Some(look) = LOOK.lock().ok().and_then(|l| l.clone()) else {
        return;
    };
    let screen = virtual_screen();
    let s = look.scale;
    let work = work_area(&screen);
    let area = Rect::new(s.old(work.left), s.old(work.top), s.old_fit(rect_w(&work)), s.old_fit(rect_h(&work)));
    let size = (s.old_cover(rect_w(&screen)), s.old_cover(rect_h(&screen)));
    // Each screen in the look's 1x pixels, the main one first (several screens: `display::draw_on`).
    let screens: Vec<Rect> =
        screens(&screen).iter().map(|r| Rect::new(s.old(r.left), s.old(r.top), s.old_cover(rect_w(r)), s.old_cover(rect_h(r)))).collect();
    STATE.with(|c| {
        let mut borrow = c.borrow_mut();
        // The picture is made again only for a new choice, look, colors, size or screens; a new
        // work area alone (the bar moved, resized or hid) keeps it.
        let mut keep_picture = false;
        if let Some(st) = borrow.as_ref() {
            let same_picture = st.look.era == look.era
                && st.look.scale == s
                && (st.fb.width, st.fb.height) == size
                && st.screens == screens
                && st.look.background == look.background
                && same_picture(&st.look.picture, &look.picture)
                && std::ptr::eq(st.look.theme, look.theme);
            if same_picture && st.view.area() == area && !force {
                // The bar may have handed the same large picture over again: not kept.
                let mut look = look;
                let_go_large(&mut look);
                return;
            }
            keep_picture = same_picture && !force;
        }
        // The first press of a double-click is kept (it is remembered by the icon's id), and a
        // drag timer that runs is stopped: the new state knows of none (brief 90).
        let mut last_press = None;
        let (mut view, old_fb) = match borrow.take() {
            Some(old) => {
                last_press = old.last_press;
                if old.timer.is_some() {
                    // SAFETY: our own window's timer.
                    unsafe { KillTimer(hwnd, DRAG_TIMER) };
                }
                (old.view, Some(old.fb))
            }
            None => {
                let mut view = Desktop::new(look.era, area);
                // Unreadable: logged by `load`, kept as it is and not saved over.
                if let Some(text) = bevel_winkit::state_file::load(&places_file()).text() {
                    view.load_places(&text);
                    log("desktop: where the icons lie read from desktop.txt");
                }
                (view, None)
            }
        };
        view.set_era(look.era);
        view.set_area(area);
        if let Some(snapshot) = crate::desktop_items::take() {
            take_pending_place(&mut view);
            view.set_files(snapshot.files);
            view.set_hidden(snapshot.hidden);
            view.recycle_full = snapshot.recycle_full;
            RECYCLE_FULL.store(snapshot.recycle_full, Ordering::SeqCst);
        }
        save_if_changed(&mut view);
        // The wallpaper and pattern over the whole desktop, made now (a new choice, look, colors,
        // size or screens), not at every paint and not for a new work area; the plain desktop has
        // none. The old picture is let go first, so that two are never held at once.
        if !keep_picture {
            view.set_background(None);
            let started = Instant::now();
            // A large picture let go after the last time: the bar reads it again, and until it
            // has come the desktop shows its color or pattern (as at the start).
            if let Some(got) = look.picture.as_ref().filter(|g| g.picture.is_none()) {
                crate::pictures::needed_again(got.reading);
            }
            let read = look.picture.as_ref().and_then(|g| g.picture.as_deref());
            let background = bevel_core::display::background_on(look.theme, &look.background, read, &screens, size.0, size.1);
            if background.is_some() {
                log(&format!(
                    "desktop: its wallpaper and pattern drawn over {}x{} ({} screens) in {} ms",
                    size.0,
                    size.1,
                    screens.len().max(1),
                    started.elapsed().as_millis()
                ));
            }
            view.set_background(background.map(std::sync::Arc::new));
        }
        // The 1x picture of the whole desktop: the old one again when its size stayed.
        let fb = match old_fb {
            Some(fb) if (fb.width, fb.height) == size => fb,
            _ => Framebuffer::new(size.0, size.1, look.theme.colors.desktop),
        };
        let mut st = State { look, view, fb, screens, last_press, right_hit: None, pointer: None, timer: None, blink_on: true };
        st.look.scale = s;
        // What is made from a large picture has been made: it goes.
        let_go_large(&mut st.look);
        repaint(hwnd, &mut st, Rect::new(0, 0, size.0, size.1));
        *borrow = Some(st);
    });
}

/// Shows the part `rc` (window pixels) of the desktop: the 1× pixels under it, enlarged (at a
/// whole size scaled by whole numbers), copied 1:1.
fn present(hdc: windows_sys::Win32::Graphics::Gdi::HDC, st: &State, rc: &RECT) {
    let (up, x0, y0) = match st.look.scale.whole_number() {
        Some(s) => {
            let (x0, y0) = (rc.left.max(0) / s, rc.top.max(0) / s);
            let (x1, y1) = (((rc.right + s - 1) / s).min(st.fb.width), ((rc.bottom + s - 1) / s).min(st.fb.height));
            if x1 <= x0 || y1 <= y0 {
                return;
            }
            let mut part = Framebuffer::new(x1 - x0, y1 - y0, 0);
            part.blit(&st.fb, -x0, -y0);
            (bevel_core::scale::upscale(&part, s), x0 * s, y0 * s)
        }
        None => {
            // Only the screen pixels asked for, each exactly what the whole picture has there.
            let s = st.look.scale;
            let (x0, y0) = (rc.left.max(0), rc.top.max(0));
            let (x1, y1) = (rc.right.min(s.len(st.fb.width)), rc.bottom.min(s.len(st.fb.height)));
            if x1 <= x0 || y1 <= y0 {
                return;
            }
            (s.enlarge_part(&st.fb, Rect::new(x0, y0, x1 - x0, y1 - y0)), x0, y0)
        }
    };
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: up.width,
            biHeight: -up.height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..Default::default()
        },
        ..Default::default()
    };
    // SAFETY: `up.pixels` holds width*height 32-bit pixels matching `info` for the call.
    unsafe {
        SetDIBitsToDevice(
            hdc,
            x0,
            y0,
            up.width as u32,
            up.height as u32,
            0,
            0,
            0,
            up.height as u32,
            up.pixels.as_ptr().cast(),
            &info,
            DIB_RGB_COLORS,
        );
    }
}

/// Takes the keyboard for the desktop, as a click on the real one did (the window in front then
/// looks inactive), without bringing it forward: the CBT hook (`cbt`) sees the activation coming
/// and `WM_WINDOWPOSCHANGING` refuses the move to the top that Windows makes with it. Without the
/// hook the desktop does not take the keyboard at all.
fn activate(hwnd: HWND) {
    if CBT_HOOK.load(Ordering::SeqCst) == 0 {
        return;
    }
    // SAFETY: our own window, on its own thread, right after the user's click.
    if unsafe { SetForegroundWindow(hwnd) } == 0 {
        // Counted every time, with only the kind of window that kept the front, in Bevel95's own
        // words (brief 90; never its class: the review's R5).
        static REFUSED: AtomicU32 = AtomicU32::new(0);
        let kind = crate::window_list::kind_of(crate::window_list::foreground());
        counted(&REFUSED, &format!("desktop: Windows did not let the desktop come forward at a click; {kind} kept the front"));
    }
    ACTIVATING.store(false, Ordering::SeqCst);
}

/// The thread's CBT hook: notes that Windows is about to activate the desktop.
unsafe extern "system" fn cbt(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HCBT_ACTIVATE as i32 && window() == Some(wparam as HWND) {
        ACTIVATING.store(true, Ordering::SeqCst);
    }
    // SAFETY: passing the hook on, as every hook must.
    unsafe { CallNextHookEx(null_mut(), code, wparam, lparam) }
}

/// Opens a desktop icon as Explorer would (its default command), on a thread of its own so the
/// desktop never waits: My Computer opens This PC, the Recycle Bin the Recycle Bin, My Documents
/// the Documents folder.
pub fn open_item(id: String) {
    // My Computer, folders and (98, 2000) My Documents open in Bevel95's own folder windows
    // (folders.exe); the Recycle Bin and files open as Windows' own.
    let era = LOOK.lock().ok().and_then(|l| l.as_ref().map(|l| l.era));
    let folder = if id == "::my-documents" && era != Some(bevel_core::theme::Era::Win95) {
        crate::start_menu::documents_folder().map(|p| p.to_string_lossy().into_owned())
    } else {
        Some(id.clone())
    };
    if let Some(folder) = folder
        && era.is_some()
        && (folder == "::my-computer" || (!folder.starts_with("::") && std::path::Path::new(&folder).is_dir()))
        && open_in_folder_window(&folder)
    {
        return;
    }
    let spawned = std::thread::Builder::new().name("desktop open".into()).spawn(move || {
        let target = match id.as_str() {
            "::my-computer" => String::from("shell:::{20D04FE0-3AEA-1069-A2D8-08002B30309D}"),
            "::recycle-bin" => String::from("shell:RecycleBinFolder"),
            "::my-documents" => match crate::start_menu::documents_folder() {
                Some(p) => p.to_string_lossy().into_owned(),
                None => String::from("shell:Personal"),
            },
            _ => id.clone(),
        };
        log(&format!("desktop: opening {}", item_kind(&id)));
        let file = crate::util::wide(&target);
        // One of a portable copy's own programs is told where its data is (`log::flag_for`).
        let flag = crate::log::flag_for(std::path::Path::new(&target)).map(crate::util::wide);
        let params = flag.as_ref().map_or(null(), |f| f.as_ptr());
        // SAFETY: COM for this thread; a null verb is the item's default command.
        let result = unsafe {
            windows_sys::Win32::System::Com::CoInitializeEx(null(), windows_sys::Win32::System::Com::COINIT_APARTMENTTHREADED as u32);
            windows_sys::Win32::UI::Shell::ShellExecuteW(null_mut(), null(), file.as_ptr(), params, null(), windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL)
        };
        if result as usize <= 32 {
            log(&format!("desktop: could not open {} (ShellExecute error {})", item_kind(&id), result as usize));
        }
    });
    if let Err(e) = spawned {
        log(&format!("desktop: could not start opening an icon: {e}"));
    }
}

/// What an icon's `id` is, as the log may show it (decisions.md, "No private text in the log"):
/// the look's own icons by their fixed names, anything else (a path with the person's file or
/// folder name) by its kind only (`log::path_kind`: "a folder", "a .txt file").
fn item_kind(id: &str) -> String {
    match id {
        "::my-computer" => String::from("My Computer"),
        "::my-documents" => String::from("My Documents"),
        "::recycle-bin" => String::from("the Recycle Bin"),
        _ if id.starts_with("::") => String::from("one of the desktop's own icons"),
        _ => crate::log::path_kind(std::path::Path::new(id)),
    }
}

/// Starts Bevel95's folder windows (`programs/folders.exe`) on `id` (a folder's path, or My
/// Computer's id); a running one takes it over. False if the program is not there or would not
/// start (Windows' own Explorer opens instead).
fn open_in_folder_window(id: &str) -> bool {
    let Some(exe) = std::env::current_exe().ok().and_then(|e| e.parent().and_then(|d| crate::start_menu::companion_file(d, "folders.exe")))
    else {
        log("desktop: folders.exe is missing; Windows' Explorer opens the folder");
        return false;
    };
    // A portable copy's folder windows are told where their data is (`log::flag_for`).
    match std::process::Command::new(&exe).arg(id).args(crate::log::flag_for(&exe)).spawn() {
        Ok(_) => {
            log(&format!("desktop: opening {} in a folder window", item_kind(id)));
            true
        }
        Err(e) => {
            log(&format!("desktop: could not start the folder windows ({e}); Windows' Explorer opens the folder"));
            false
        }
    }
}

/// The drag timer's step: takes in the pointer's latest place and paints what changed; stops
/// the timer once the drag or the rectangle is over.
fn drag_step(hwnd: HWND) {
    let over = with_state_value(hwnd, |st, font| {
        let mut dirty = Vec::new();
        if let Some((x, y)) = st.pointer.take() {
            dirty = st.view.update(font, |v| v.drag_to(font, x, y)).1;
        }
        if let Some((_, frames)) = st.timer.as_mut() {
            *frames += u32::from(!dirty.is_empty());
        }
        // Once the icons are dragged, Windows' drag and drop carries them (`drag_out`): into
        // folders, other windows and programs, or onto the desktop itself.
        (!st.view.gesture_active() || st.view.dragging(), dirty)
    })
    .unwrap_or(true);
    if over {
        stop_drag_timer(hwnd);
    }
    if STATE.with(|c| c.borrow().as_ref().is_some_and(|st| st.view.dragging())) {
        drag_out(hwnd);
    }
}

/// The icons a drag carries (by id: a refresh during the drag reorders the items, the third safety
/// review of 2026-11-18, suspected 3) and where the press was.
type OwnIcons = (Vec<String>, (i32, i32));

thread_local! {
    /// A drag of the desktop's icons going on: the icons and where the press was.
    static OUT: RefCell<Option<OwnIcons>> = const { RefCell::new(None) };
    /// The look's menu fonts for the right-button drag's menu (made once per look and size).
    static MENU_LOOK: RefCell<Option<(bevel_core::theme::Era, bevel_core::theme::Depth, bevel_core::scale::Scale, bevel_winkit::kit::Look)>> = const { RefCell::new(None) };
}

/// A drag of icons has begun: Windows' drag and drop carries it, so the icons can go into a
/// folder, a folder window, Windows' Explorer or any program; a drop on the desktop itself moves
/// them (`desktop_drop`).
fn drag_out(hwnd: HWND) {
    let taken = with_state_value(hwnd, |st, font| {
        let (taken, dirty) = st.view.update(font, |v| v.take_drag());
        let Some((items, from, right)) = taken else { return (None, dirty) };
        let mut paths = Vec::new();
        let mut others = Vec::new();
        for &i in &items {
            let item = &st.view.items[i];
            match ItemKind::of(item) {
                ItemKind::Folder | ItemKind::File => paths.push(item.id.clone()),
                _ => others.push(item.id.clone()),
            }
        }
        let picture = st
            .view
            .drag_picture(st.look.theme, font, &items, st.look.theme.colors.desktop)
            .map(|(p, x, y)| (p, x - from.0, y - from.1));
        (Some((st.view.ids_of(&items), from, right, paths, others, picture, st.view.era, st.look.scale)), dirty)
    })
    .flatten();
    let Some((items, from, right, paths, others, picture, era, scale)) = taken else { return };
    stop_drag_timer(hwnd);
    // SAFETY: the capture this window took at the press (Windows' drag takes the mouse now).
    unsafe { ReleaseCapture() };
    OUT.with(|o| *o.borrow_mut() = Some((items, from)));
    log(&format!("desktop: a drag of {} item(s){} began", paths.len() + others.len(), if right { " with the right button" } else { "" }));
    let done = bevel_winkit::dragdrop::run(bevel_winkit::dragdrop::Out { hwnd, paths, others, right, era, picture, scale });
    OUT.with(|o| o.borrow_mut().take());
    log(&format!("desktop: the drag ended ({})", done.map_or_else(|| String::from("nothing reported to the source"), |e| format!("{e:?}"))));
}

/// Where a drop on desktop icon `item` goes.
fn dest_of(item: &bevel_core::desktop_icons::Item) -> bevel_winkit::dragdrop::Dest {
    use bevel_core::desktop_icons::System;
    use bevel_winkit::dragdrop::Dest;
    match ItemKind::of(item) {
        ItemKind::Folder | ItemKind::Drive => Dest::Folder(std::path::PathBuf::from(&item.id)),
        ItemKind::System(System::RecycleBin) => Dest::RecycleBin,
        ItemKind::System(System::MyDocuments) => crate::start_menu::documents_folder().map_or(Dest::Refuse, Dest::Folder),
        _ => Dest::Refuse,
    }
}

/// A drag over the desktop (its drop target): a folder, the Recycle Bin and My Documents take
/// files and show selected while the pointer is on them (My Computer too, refusing them, as on
/// the Reference PCs); anywhere else the files go into the owner's Desktop folder, where the
/// pointer is. The desktop's own icons dropped on the desktop only move.
fn desktop_drop(hwnd: HWND, phase: bevel_winkit::dragdrop::Phase, over: &bevel_winkit::dragdrop::Over) -> Option<bevel_core::drag::Effect> {
    use bevel_core::drag::Effect;
    use bevel_winkit::dragdrop::{Dest, Phase};
    let own = OUT.with(|o| o.borrow().clone()).filter(|_| over.own.as_ref().is_some_and(|o| o.source == hwnd));
    let mut dragged = over.files.clone();
    if let Some(o) = &over.own {
        dragged.extend(o.others.iter().cloned());
    }
    let found = with_state_value(hwnd, |st, font| {
        let s = st.look.scale;
        let (x, y) = (s.old(over.x), s.old(over.y));
        let target = if phase == Phase::Leave { None } else { st.view.drop_target(font, x, y, &dragged) };
        let hover = if phase == Phase::Drop { None } else { target };
        let dirty = st.view.update(font, |v| v.drop_hover = hover).1;
        let dest = match target {
            Some(i) => dest_of(&st.view.items[i]),
            None => crate::desktop_items::desktop_folder().map_or(Dest::Refuse, Dest::Folder),
        };
        ((x, y, target, dest, st.view.era, st.look.scale), dirty)
    });
    let (x, y, target, dest, era, scale) = found?;
    if phase == Phase::Leave {
        return None;
    }
    // The desktop's own icons dropped on the desktop itself: they move there.
    let reposition = own.is_some() && target.is_none();
    // What is done is what the pointer shows: the look's wish within what the source offers,
    // decided here once, before anything is done.
    let effect = if reposition {
        bevel_core::drag::narrow(Some(Effect::Move), over.offers(), false)
    } else {
        bevel_winkit::dragdrop::drop_effect(era, &dest, over)
    };
    if phase != Phase::Drop {
        return effect;
    }
    let mut effect = effect?;
    if over.right {
        let offered = bevel_winkit::dragdrop::menu_effects(&dest, reposition, over);
        effect = right_drop_menu(hwnd, era, scale, effect, &offered)?;
    }
    if reposition {
        if let Some((items, from)) = own {
            with_state(hwnd, |st, font| st.view.update(font, |v| v.move_ids(&items, (x - from.0, y - from.1))).1);
        }
        return Some(Effect::Move);
    }
    let files = over.files.clone();
    match dest {
        Dest::RecycleBin if era == bevel_core::theme::Era::Win98 => {
            // 98 asked as Delete does (the bar shows the question).
            let targets: Vec<Target> = files
                .iter()
                .map(|p| {
                    let folder = std::path::Path::new(p).is_dir();
                    let name = bevel_winkit::shell::display_name(p).unwrap_or_default();
                    Target { id: p.clone(), name, kind: if folder { ItemKind::Folder } else { ItemKind::File } }
                })
                .collect();
            let ticket = remember_targets(targets);
            // SAFETY: a message to the bar.
            unsafe { PostMessageW(bar(), crate::bar::wm_desktop_command(), DesktopCommand::Delete.code(), command_param(ticket, false)) };
        }
        Dest::RecycleBin => crate::file_ops::drop_in_recycle_bin(files),
        Dest::Folder(dir) => {
            // Into the desktop itself: each new icon where the pointer let go (the Reference PCs),
            // the next ones beside it.
            if target.is_none() {
                with_state(hwnd, |st, _| {
                    for (k, f) in files.iter().enumerate() {
                        if let Some(name) = std::path::Path::new(f).file_name() {
                            let id = dir.join(name).to_string_lossy().into_owned();
                            st.view.place_new(&id, x - 16 + 75 * k as i32, y - 16);
                        }
                    }
                    Vec::new()
                });
            }
            crate::file_ops::drop_files(files, dir, effect, |_| {});
        }
        Dest::Refuse => return None,
    }
    Some(effect)
}

/// The right-button drag's menu at the pointer: what is chosen, or `None` for Cancel.
fn right_drop_menu(
    hwnd: HWND,
    era: bevel_core::theme::Era,
    scale: bevel_core::scale::Scale,
    default: bevel_core::drag::Effect,
    offered: &[bevel_core::drag::Effect],
) -> Option<bevel_core::drag::Effect> {
    let look = MENU_LOOK.with(|m| {
        let mut m = m.borrow_mut();
        let depth = bevel_winkit::look::colors();
        if !m.as_ref().is_some_and(|(e, d, s, _)| *e == era && *d == depth && *s == scale) {
            *m = Some((era, depth, scale, bevel_winkit::look::for_look(era, depth, scale)));
        }
        m.as_ref().map(|(_, _, _, l)| l.clone())
    })?;
    let (menu, does) = bevel_core::drag::drop_menu(era, default, offered);
    let mut p = POINT::default();
    // SAFETY: a valid out-pointer.
    unsafe { GetCursorPos(&mut p) };
    let request = bevel_winkit::menu::Request {
        owner: hwnd,
        menu,
        style: bevel_core::paint::MenuStyle::Popup,
        at: (p.x, p.y),
        above: None,
        titles: &[],
    };
    match bevel_winkit::menu::track(&look, request).outcome {
        bevel_winkit::menu::Outcome::Chosen(i) => does.get(i).copied().flatten(),
        _ => None,
    }
}

fn start_drag_timer(hwnd: HWND, st: &mut State) {
    if st.timer.is_none() {
        st.timer = Some((Instant::now(), 0));
        // SAFETY: our own window.
        if unsafe { SetTimer(hwnd, DRAG_TIMER, DRAG_MS, None) } == 0 {
            log_win32("SetTimer (desktop drag)");
        }
    }
}

fn stop_drag_timer(hwnd: HWND) {
    let timer = STATE.with(|c| c.borrow_mut().as_mut().and_then(|st| st.timer.take()));
    if let Some((since, frames)) = timer {
        // SAFETY: our own window.
        unsafe { KillTimer(hwnd, DRAG_TIMER) };
        log(&format!("desktop: a drag painted {frames} frames in {} ms", since.elapsed().as_millis()));
    }
}

/// A mouse message's point in desktop pixels at 1×.
fn point_1x(st: &State, lparam: LPARAM) -> (i32, i32) {
    let (x, y) = crate::util::point(lparam);
    let s = st.look.scale;
    (s.old(x), s.old(y))
}

fn bar() -> HWND {
    BAR_HWND.load(Ordering::SeqCst) as HWND
}

/// A command on the selected icons from a key: the selection, remembered under a ticket of its
/// own, is what the bar carries it out on (lParam: `command_param`, by the keyboard).
fn post_command(command: DesktopCommand) {
    let targets = STATE.with(|c| c.borrow().as_ref().map(|st| selection_targets(&st.view))).unwrap_or_default();
    let ticket = remember_targets(targets);
    // SAFETY: a message to the bar (a stale handle just fails).
    unsafe { PostMessageW(bar(), crate::bar::wm_desktop_command(), command.code(), command_param(ticket, true)) };
}

/// Opens the menu on the selected icons at the screen point (x, y) (wParam: the ticket of the
/// selection now), or, with nothing selected, the desktop's own menu there.
fn post_menu(on_icons: bool, x: i32, y: i32) {
    if on_icons {
        let targets = STATE.with(|c| c.borrow().as_ref().map(|st| selection_targets(&st.view))).unwrap_or_default();
        let ticket = remember_targets(targets);
        // SAFETY: a message to the bar.
        unsafe { PostMessageW(bar(), wm_desktop_item_menu(), ticket as usize, point_param(x, y)) };
        return;
    }
    // SAFETY: a message to the bar.
    unsafe { PostMessageW(bar(), wm_desktop_menu(), x as isize as usize, y as isize) };
}

/// The menu from the keyboard (Shift+F10, the menu key): on the selected icons at the focus
/// icon's middle, as Windows opened it; with nothing selected, the desktop's menu at the top
/// left of the screen (the Reference PCs).
fn keyboard_menu(hwnd: HWND) {
    let at = STATE.with(|c| {
        let borrow = c.borrow();
        let st = borrow.as_ref()?;
        let s = st.look.scale;
        let selected = !st.view.selection().is_empty();
        let point = match st.view.focus.filter(|_| selected) {
            Some(f) => {
                let r = st.view.icon_rect(f);
                (r.x + r.w / 2, r.y + r.h / 2)
            }
            None => (st.view.area().x, st.view.area().y),
        };
        Some((selected, point, s))
    });
    let Some((selected, (x, y), s)) = at else { return };
    if !selected && let Ok(mut m) = MENU_AT.lock() {
        // A New item made from this menu goes to the top left.
        *m = Some((x, y));
    }
    let mut p = POINT { x: s.at(x), y: s.at(y) };
    // SAFETY: our own window; a valid point.
    unsafe { windows_sys::Win32::Graphics::Gdi::ClientToScreen(hwnd, &mut p) };
    post_menu(selected, p.x, p.y);
}

/// Starts renaming icon `i` (blinking caret, the keyboard for the desktop).
fn begin_rename(hwnd: HWND, i: usize) {
    let started = with_state_value(hwnd, |st, font| st.view.update(font, |v| v.begin_rename(i))).unwrap_or(false);
    if started {
        activate(hwnd);
        // SAFETY: our own window.
        unsafe {
            SetTimer(hwnd, BLINK_TIMER, windows_sys::Win32::UI::WindowsAndMessaging::GetCaretBlinkTime().max(100), None);
        }
    }
}

/// Renaming ends with the name typed (Enter, a click elsewhere, the desktop losing the
/// keyboard): Windows' file machinery renames the file; Windows' own message comes if it refuses
/// the name.
/// When the new name changes a file's known extension, the look's warning comes first (on the
/// bar's thread) and the box waits under it (`rename_warning`); `by_keyboard`: Enter took it.
fn finish_rename(hwnd: HWND, keep: bool, by_keyboard: bool) {
    // While the warning is open nothing ends the renaming but its answer.
    if STATE.with(|c| c.borrow().as_ref().is_some_and(|st| st.view.is_asking())) {
        return;
    }
    // SAFETY: our own window.
    unsafe { KillTimer(hwnd, BLINK_TIMER) };
    // A name that would be a path ("..\x", "C:\x", "a/b") or that Windows refuses: the look's
    // message (on the bar's thread), and the box opens again (the safety review of 2026-11-18,
    // suspected 3).
    if keep && typed_bad_name() {
        with_state(hwnd, |st, font| st.view.update(font, |v| v.rename_asking(true)).1);
        // SAFETY: a message to the bar (a stale handle just fails).
        if unsafe { PostMessageW(bar(), WM_DESKTOP_RENAME_ASK.id(), usize::from(by_keyboard), 1) } == 0 {
            rename_answered(hwnd, false);
        }
        return;
    }
    if keep && needs_warning() {
        with_state(hwnd, |st, font| st.view.update(font, |v| v.rename_asking(true)).1);
        log("desktop: the new name changes the file's extension: the look's warning first");
        // SAFETY: a message to the bar (a stale handle just fails).
        if unsafe { PostMessageW(bar(), WM_DESKTOP_RENAME_ASK.id(), usize::from(by_keyboard), 0) } == 0 {
            rename_answered(hwnd, false);
        }
        return;
    }
    end_rename(hwnd, keep);
}

/// Whether the name in the rename box of one of the owner's items cannot be a file's name
/// (`bevel_core::rename_warning::bad_name`).
fn typed_bad_name() -> bool {
    STATE.with(|c| {
        let borrow = c.borrow();
        let Some(st) = borrow.as_ref() else { return false };
        let Some(i) = st.view.renaming() else { return false };
        match st.view.rename_result() {
            RenameEnd::Rename { name, .. } if !matches!(ItemKind::of(&st.view.items[i]), ItemKind::System(_)) => {
                bevel_core::rename_warning::bad_name(&name)
            }
            _ => false,
        }
    })
}

/// Whether the name in the rename box changes the file's known extension
/// (`bevel_core::rename_warning::warns`, on the real file names before and after).
fn needs_warning() -> bool {
    let pending = STATE.with(|c| {
        let borrow = c.borrow();
        let st = borrow.as_ref()?;
        let i = st.view.renaming()?;
        let item = &st.view.items[i];
        match st.view.rename_result() {
            RenameEnd::Rename { name, .. } if !matches!(ItemKind::of(item), ItemKind::System(_)) => {
                Some((item.id.clone(), item.name.clone(), name, item.folder))
            }
            _ => None,
        }
    });
    let Some((id, shown, typed, folder)) = pending else { return false };
    let old = std::path::Path::new(&id);
    let old_name = old.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let new_name = crate::file_ops::new_file_name(old, &shown, &typed, folder);
    bevel_core::rename_warning::warns(&old_name, &new_name, folder, crate::file_ops::known_type)
}

/// The warning's answer (`WM_RENAME_ANSWER`): Yes renames with the name typed; No opens the box
/// again, all selected (the look's rule for which name it shows), with the keyboard.
fn rename_answered(hwnd: HWND, yes: bool) {
    let asking = STATE.with(|c| c.borrow().as_ref().is_some_and(|st| st.view.is_asking()));
    if !asking {
        return;
    }
    if yes {
        with_state(hwnd, |st, font| st.view.update(font, |v| v.rename_asking(false)).1);
        end_rename(hwnd, true);
        return;
    }
    let era = STATE.with(|c| c.borrow().as_ref().map(|st| st.look.era));
    let keep_typed = era.is_some_and(bevel_core::rename_warning::keeps_typed_name);
    with_state(hwnd, |st, font| st.view.update(font, |v| v.rename_declined(keep_typed)).1);
    activate(hwnd);
    // SAFETY: our own window.
    unsafe {
        SetTimer(hwnd, BLINK_TIMER, windows_sys::Win32::UI::WindowsAndMessaging::GetCaretBlinkTime().max(100), None);
    }
}

/// Renaming ends: with the name typed (`keep`) Windows' file machinery renames the file.
fn end_rename(hwnd: HWND, keep: bool) {
    let ended = with_state_value(hwnd, |st, font| {
        let target = st.view.renaming().map(|i| {
            let item = &st.view.items[i];
            Target { id: item.id.clone(), name: item.name.clone(), kind: ItemKind::of(item) }
        });
        let (end, dirty) = st.view.update(font, |v| if keep { v.end_rename() } else { v.rename_key(EditKey::Escape, false).unwrap_or(RenameEnd::Unchanged) });
        ((end, target), dirty)
    });
    if let Some((RenameEnd::Rename { id, name }, Some(target))) = ended
        && target.id == id
    {
        let old = target.id.clone();
        let typed = name.clone();
        crate::file_ops::rename(target, name, move |new_id| {
            if let Ok(mut r) = RENAMED.lock() {
                r.push((old, new_id, typed));
            }
            post_to_desktop(WM_RENAMED.id());
        });
    }
}

/// A key while a label is renamed. True if it was the edit box's.
fn rename_key(hwnd: HWND, msg: u32, wparam: WPARAM) -> bool {
    if !STATE.with(|c| c.borrow().as_ref().is_some_and(|st| st.view.renaming().is_some())) {
        return false;
    }
    // SAFETY: plain queries of the keyboard's state for this message.
    let (ctrl, shift) = unsafe { (GetKeyState(i32::from(VK_CONTROL)) < 0, GetKeyState(i32::from(VK_SHIFT)) < 0) };
    let key = match (msg, wparam as u16) {
        (WM_KEYDOWN, VK_RETURN) => Some(EditKey::Enter),
        (WM_KEYDOWN, VK_ESCAPE) => Some(EditKey::Escape),
        (WM_KEYDOWN, VK_BACK) => Some(EditKey::Backspace),
        (WM_KEYDOWN, VK_DELETE) => Some(EditKey::Delete),
        (WM_KEYDOWN, VK_LEFT) => Some(EditKey::Left),
        (WM_KEYDOWN, VK_RIGHT) => Some(EditKey::Right),
        (WM_KEYDOWN, VK_HOME) => Some(EditKey::Home),
        (WM_KEYDOWN, VK_END) => Some(EditKey::End),
        (WM_KEYDOWN, VK_A) if ctrl => Some(EditKey::SelectAll),
        (WM_KEYDOWN, VK_V) if ctrl => {
            if let Some(text) = bevel_winkit::kit::clipboard_text() {
                with_state(hwnd, |st, font| st.view.update(font, |v| v.rename_paste(&text)).1);
            }
            return true;
        }
        (WM_CHAR, c) if c >= 0x20 && c != 0x7F && !ctrl => char::from_u32(u32::from(c)).map(EditKey::Char),
        (WM_KEYDOWN | WM_CHAR, _) => return true,
        _ => return false,
    };
    match key {
        Some(EditKey::Enter) => finish_rename(hwnd, true, true),
        Some(EditKey::Escape) => finish_rename(hwnd, false, true),
        Some(key) => with_state(hwnd, |st, font| st.view.update(font, |v| v.rename_key(key, shift)).1),
        None => {}
    }
    true
}

/// Answers a live test's question (`WM_ICON_AT`, `WM_ICON_PLACE`).
fn answer(msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    STATE.with(|c| {
        let borrow = c.borrow();
        let Some(st) = borrow.as_ref() else { return 0 };
        let s = st.look.scale;
        let font = &st.look.font;
        if msg == WM_ICON_AT.id() {
            let hit = st.view.hit(font, s.old(wparam as i32), s.old(lparam as i32));
            return hit.map_or(0, |i| id_code(&st.view.items[i].id) as LRESULT);
        }
        let Some(i) = st.view.items.iter().position(|item| id_code(&item.id) as usize == wparam) else { return 0 };
        match lparam {
            1 => return LRESULT::from(st.view.is_selected(i)),
            // Shown cut (faded); its label being renamed.
            2 => return LRESULT::from(st.view.is_cut(i)),
            3 => return LRESULT::from(st.view.renaming() == Some(i)),
            _ => {}
        }
        let r = st.view.icon_rect(i);
        (s.at(r.x) as LRESULT & 0xFFFF) | ((s.at(r.y) as LRESULT) << 16)
    })
}

/// The window procedure of the desktop window.
///
/// # Safety
/// Called by Windows as the window procedure of `DESKTOP_CLASS` windows.
pub unsafe extern "system" fn desktop_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        // Clicking the desktop must not bring it forward or take the focus.
        WM_MOUSEACTIVATE => return MA_NOACTIVATE as LRESULT,
        WM_ERASEBKGND => return 1,
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            // SAFETY: paired BeginPaint/EndPaint on our own window; the brush is ours to delete.
            unsafe {
                let hdc = BeginPaint(hwnd, &mut ps);
                let shown = STATE.with(|c| c.borrow().as_ref().map(|st| present(hdc, st, &ps.rcPaint)).is_some());
                if !shown {
                    let brush = CreateSolidBrush(COLOR.load(Ordering::SeqCst));
                    FillRect(hdc, &ps.rcPaint, brush);
                    DeleteObject(brush);
                }
                EndPaint(hwnd, &ps);
            }
            return 0;
        }
        // Windows moves the window with its owner (that is the point of being owned: up at
        // Show desktop, down again after it), so it may change its place among the windows;
        // only a move into the topmost windows is refused, and the move to the top that comes
        // with being activated (`activate`; `place` and `check` put anything else right).
        WM_WINDOWPOSCHANGING if lparam != 0 && !PLACING.load(Ordering::SeqCst) => {
            // SAFETY: Windows passes a WINDOWPOS it lets the window change.
            unsafe {
                let pos = &mut *(lparam as *mut WINDOWPOS);
                if pos.hwndInsertAfter == HWND_TOPMOST || ACTIVATING.load(Ordering::SeqCst) {
                    pos.flags |= SWP_NOZORDER;
                }
            }
            return 0;
        }
        WM_ACTIVATE => {
            ACTIVATING.store(false, Ordering::SeqCst);
            let active = (wparam & 0xFFFF) as u32 != WA_INACTIVE;
            // Another window took the keyboard while a label was renamed: the name typed is
            // taken, as Windows did.
            if !active && STATE.with(|c| c.borrow().as_ref().is_some_and(|st| st.view.renaming().is_some())) {
                finish_rename(hwnd, true, false);
            }
            with_state(hwnd, |st, font| st.view.update(font, |v| v.active = active).1);
            return 0;
        }
        // A press: Windows' flyouts, our menus and the volume control close (the bar does it,
        // as at a press on it; posted: this thread is not the bar's), the desktop takes the
        // keyboard, and the icons are selected as the click says (`Desktop::press`); the left
        // button may start a drag or a selection rectangle, so the mouse is held for this window.
        WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN => {
            // SAFETY: a message to the bar (a stale handle just fails).
            unsafe { PostMessageW(BAR_HWND.load(Ordering::SeqCst) as HWND, WM_DESKTOP_PRESS.id(), 0, 0) };
            if msg == WM_MBUTTONDOWN {
                return 0;
            }
            // While the extension warning is open the desktop takes no clicks (Windows' message
            // box held the desktop until it was answered).
            if STATE.with(|c| c.borrow().as_ref().is_some_and(|st| st.view.is_asking())) {
                static WHILE_ASKING: AtomicU32 = AtomicU32::new(0);
                counted(&WHILE_ASKING, "desktop: a press while the rename warning is open was ignored");
                return 0;
            }
            activate(hwnd);
            // While a label is renamed: a left click in its box moves the caret; any other
            // click takes the name typed and goes on as usual (Windows did the same).
            if STATE.with(|c| c.borrow().as_ref().is_some_and(|st| st.view.renaming().is_some())) {
                let inside = msg == WM_LBUTTONDOWN
                    && with_state_value(hwnd, |st, font| {
                        let (x, y) = point_1x(st, lparam);
                        st.view.update(font, |v| v.rename_click(font, x, y))
                    })
                    .unwrap_or(false);
                if inside {
                    return 0;
                }
                finish_rename(hwnd, true, false);
                // The warning came: the click goes no further (its window is in front).
                if STATE.with(|c| c.borrow().as_ref().is_some_and(|st| st.view.is_asking())) {
                    return 0;
                }
            }
            let (wx, wy) = crate::util::point(lparam);
            let button = if msg == WM_LBUTTONDOWN { Button::Left } else { Button::Right };
            let mods = Mods { ctrl: wparam & MK_CONTROL != 0, shift: wparam & MK_SHIFT != 0 };
            let open = with_state_value(hwnd, |st, font| {
                let (x, y) = point_1x(st, lparam);
                let (hit, dirty) = st.view.update(font, |v| v.press(font, x, y, button, mods));
                if button == Button::Right {
                    st.right_hit = hit;
                }
                let plain = mods == Mods::default();
                let double = button == Button::Left && plain && hit.is_some() && st.second_click(hit, wx, wy);
                (double.then(|| hit.map(|i| st.view.items[i].id.clone())).flatten(), dirty)
            });
            if msg == WM_LBUTTONDOWN {
                // SAFETY: our own window, while its button is down.
                unsafe { SetCapture(hwnd) };
            }
            if let Some(Some(id)) = open {
                // The second press of a double-click: no drag follows.
                STATE.with(|c| {
                    if let Some(st) = c.borrow_mut().as_mut() {
                        st.view.cancel();
                    }
                });
                open_item(id);
            }
            return 0;
        }
        WM_MOUSEMOVE => {
            STATE.with(|c| {
                if let Some(st) = c.borrow_mut().as_mut()
                    && st.view.gesture_active()
                {
                    st.pointer = Some(point_1x(st, lparam));
                    start_drag_timer(hwnd, st);
                }
            });
            return 0;
        }
        WM_TIMER if wparam == DRAG_TIMER => {
            drag_step(hwnd);
            return 0;
        }
        WM_LBUTTONUP => {
            with_state(hwnd, |st, font| {
                let (x, y) = point_1x(st, lparam);
                st.pointer = None;
                st.view.update(font, |v| v.release(font, x, y)).1
            });
            stop_drag_timer(hwnd);
            // SAFETY: the capture this window took at the press.
            unsafe { ReleaseCapture() };
            return 0;
        }
        // Another window took the mouse (or the press was given up): a drag goes back.
        WM_CAPTURECHANGED if lparam as HWND != hwnd => {
            with_state(hwnd, |st, font| {
                st.pointer = None;
                if st.view.gesture_active() { st.view.update(font, |v| v.cancel()).1 } else { Vec::new() }
            });
            stop_drag_timer(hwnd);
            return 0;
        }
        WM_RBUTTONUP => {
            // No right-button drag came of the press.
            STATE.with(|c| {
                if let Some(st) = c.borrow_mut().as_mut() {
                    st.view.cancel();
                }
            });
            // On an icon, the menu on the selected icons (the press selected it); on the empty
            // desktop, the desktop's own menu.
            let on_icon = STATE.with(|c| c.borrow().as_ref().is_some_and(|st| st.right_hit.is_some()));
            if !on_icon {
                let at = STATE.with(|c| c.borrow().as_ref().map(|st| point_1x(st, lparam)));
                if let Ok(mut m) = MENU_AT.lock() {
                    *m = at;
                }
            }
            let mut screen = POINT::default();
            // SAFETY: a valid out-pointer.
            unsafe { GetCursorPos(&mut screen) };
            post_menu(on_icon, screen.x, screen.y);
            return 0;
        }
        // Menus come from WM_RBUTTONUP and the keys below, not from Windows' own WM_CONTEXTMENU.
        WM_CONTEXTMENU => return 0,
        WM_SYSKEYDOWN => {
            // SAFETY: a plain query of the keyboard's state for this message.
            let shift = unsafe { GetKeyState(i32::from(VK_SHIFT)) } < 0;
            let alt = lparam & (1 << 29) != 0;
            let key = match wparam as u16 {
                VK_F10 => Some(ShortcutKey::F10),
                VK_RETURN => Some(ShortcutKey::Enter),
                _ => None,
            };
            match key.and_then(|k| bevel_core::desktop::shortcut(k, false, shift, alt)) {
                Some(None) => {
                    keyboard_menu(hwnd);
                    return 0;
                }
                Some(Some(command)) => {
                    post_command(command);
                    return 0;
                }
                None => {}
            }
        }
        WM_KEYDOWN | WM_CHAR => {
            if rename_key(hwnd, msg, wparam) {
                return 0;
            }
            // SAFETY: plain queries of the keyboard's state for this message.
            let (ctrl, shift) = unsafe { (GetKeyState(i32::from(VK_CONTROL)) < 0, GetKeyState(i32::from(VK_SHIFT)) < 0) };
            if msg == WM_KEYDOWN {
                let key = match wparam as u16 {
                    VK_DELETE => Some(ShortcutKey::Delete),
                    VK_F2 => Some(ShortcutKey::F2),
                    VK_APPS => Some(ShortcutKey::Menu),
                    k @ (VK_X | VK_C | VK_V | VK_Z) => char::from_u32(u32::from(k)).map(ShortcutKey::Letter),
                    _ => None,
                };
                match key.and_then(|k| bevel_core::desktop::shortcut(k, ctrl, shift, false)) {
                    Some(None) => {
                        keyboard_menu(hwnd);
                        return 0;
                    }
                    // F2: the one selected icon's label becomes the box to type in.
                    Some(Some(DesktopCommand::Rename)) => {
                        let one = STATE.with(|c| c.borrow().as_ref().and_then(|st| match st.view.selection().as_slice() {
                            [i] => Some(*i),
                            _ => None,
                        }));
                        if let Some(i) = one {
                            begin_rename(hwnd, i);
                        }
                        return 0;
                    }
                    Some(Some(command)) => {
                        post_command(command);
                        return 0;
                    }
                    None => {}
                }
            }
            let key = match (msg, wparam as u16) {
                (WM_KEYDOWN, VK_UP) => Some(Key::Up),
                (WM_KEYDOWN, VK_DOWN) => Some(Key::Down),
                (WM_KEYDOWN, VK_LEFT) => Some(Key::Left),
                (WM_KEYDOWN, VK_RIGHT) => Some(Key::Right),
                (WM_KEYDOWN, VK_HOME) => Some(Key::Home),
                (WM_KEYDOWN, VK_END) => Some(Key::End),
                (WM_KEYDOWN, VK_RETURN) => Some(Key::Enter),
                (WM_KEYDOWN, VK_ESCAPE) => Some(Key::Escape),
                (WM_KEYDOWN, VK_A) if ctrl => Some(Key::SelectAll),
                (WM_CHAR, c) if c > 0x20 && c != 0x7F => char::from_u32(u32::from(c)).map(Key::Char),
                _ => None,
            };
            if let Some(key) = key {
                let open = with_state_value(hwnd, |st, font| {
                    let (open, dirty) = st.view.update(font, |v| v.key(key));
                    (open.into_iter().map(|i| st.view.items[i].id.clone()).collect::<Vec<_>>(), dirty)
                });
                if key == Key::Escape {
                    stop_drag_timer(hwnd);
                }
                for id in open.unwrap_or_default() {
                    open_item(id);
                }
                return 0;
            }
        }
        m if m == WM_LOOK.id() => {
            rebuild(hwnd, wparam == 1);
            return 0;
        }
        m if m == WM_ARRANGE.id() => {
            with_state(hwnd, |st, font| {
                let dirty = st.view.update(font, |v| match wparam {
                    LAYOUT_AUTO => v.set_auto_arrange(!v.auto_arrange()),
                    LAYOUT_LINE_UP => v.line_up(),
                    i => {
                        if let Some(&by) = bevel_core::desktop_icons::Arrange::ALL.get(i) {
                            v.set_arrange(by);
                        }
                    }
                });
                dirty.1
            });
            return 0;
        }
        WM_TIMER if wparam == ITEMS_LATER_TIMER => {
            // SAFETY: our own window's timer.
            unsafe {
                KillTimer(hwnd, ITEMS_LATER_TIMER);
                PostMessageW(hwnd, WM_ITEMS.id(), 0, 0);
            }
            return 0;
        }
        m if m == WM_ITEMS.id() => {
            // A press held, or a second press of a double-click still due: the new icons wait
            // (the snapshot stays where it is), so neither is given up nor lands elsewhere.
            // SAFETY: plain queries.
            let (now, double) = unsafe { (windows_sys::Win32::System::SystemInformation::GetTickCount(), GetDoubleClickTime()) };
            // A press counts as held only while a mouse button is really down: a right press let go
            // over another window leaves no release here, and is given up now (the review of brief
            // 90, R4), so new icons never wait for the next press.
            // SAFETY: plain queries of the buttons' state.
            let down = unsafe {
                use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_RBUTTON};
                GetAsyncKeyState(i32::from(VK_LBUTTON)) < 0 || GetAsyncKeyState(i32::from(VK_RBUTTON)) < 0
            };
            let busy = STATE.with(|c| {
                let mut st = c.borrow_mut();
                let st = st.as_mut()?;
                if !press_holds(st.view.gesture_active(), down) {
                    st.view.cancel();
                }
                Some((st.view.gesture_active(), items_wait(st.last_press.as_ref().map(|p| p.0), now, double)))
            });
            if let Some((held, wait)) = busy
                && (held || wait > 0)
            {
                static HELD_BACK: AtomicU32 = AtomicU32::new(0);
                if !ITEMS_WAITING.swap(true, Ordering::SeqCst) {
                    counted(&HELD_BACK, "desktop: the icons were read again during a press or a double-click; taken in after it");
                }
                // SAFETY: our own window (a running timer is set anew).
                unsafe { SetTimer(hwnd, ITEMS_LATER_TIMER, if held { 100 } else { wait }, None) };
                return 0;
            }
            ITEMS_WAITING.store(false, Ordering::SeqCst);
            if let Some(snapshot) = crate::desktop_items::take() {
                with_state(hwnd, |st, _| {
                    take_pending_place(&mut st.view);
                    st.view.set_files(snapshot.files);
                    st.view.set_hidden(snapshot.hidden);
                    st.view.recycle_full = snapshot.recycle_full;
                    RECYCLE_FULL.store(snapshot.recycle_full, Ordering::SeqCst);
                    let pending = PENDING_SELECT.lock().ok().and_then(|p| p.clone());
                    if let Some(i) = pending.and_then(|id| st.view.index_of(&id)) {
                        st.view.select_only(Some(i));
                        if let Ok(mut p) = PENDING_SELECT.lock() {
                            *p = None;
                        }
                    }
                    everything(st)
                });
                stop_drag_timer(hwnd);
            }
            return 0;
        }
        m if m == WM_RENAME_ANSWER.id() => {
            rename_answered(hwnd, wparam == 1);
            return 0;
        }
        m if m == WM_RENAME.id() => {
            let id = RENAME_REQUEST.lock().ok().and_then(|mut r| r.take());
            let i = id.and_then(|id| STATE.with(|c| c.borrow().as_ref().and_then(|st| st.view.index_of(&id))));
            if let Some(i) = i {
                begin_rename(hwnd, i);
            }
            return 0;
        }
        WM_TIMER if wparam == BLINK_TIMER => {
            with_state(hwnd, |st, font| {
                let on = !st.blink_on;
                st.blink_on = on;
                st.view.update(font, |v| v.blink(on)).1
            });
            return 0;
        }
        m if m == WM_CUT.id() => {
            let ids = CUT.lock().map(|c| c.clone()).unwrap_or_default();
            with_state(hwnd, |st, font| st.view.update(font, |v| v.set_cut(ids)).1);
            return 0;
        }
        // Something else went onto the clipboard: no icon shows cut any more.
        WM_CLIPBOARDUPDATE => {
            // SAFETY: a plain query.
            let seq = unsafe { windows_sys::Win32::System::DataExchange::GetClipboardSequenceNumber() };
            if seq != CUT_SEQ.load(Ordering::SeqCst) && CUT.lock().is_ok_and(|c| !c.is_empty()) {
                if let Ok(mut c) = CUT.lock() {
                    c.clear();
                }
                with_state(hwnd, |st, font| st.view.update(font, |v| v.set_cut(Vec::new())).1);
            }
            return 0;
        }
        m if m == WM_RENAMED.id() => {
            let renamed = RENAMED.lock().map(|mut r| std::mem::take(&mut *r)).unwrap_or_default();
            // The label's width changes with its name: everything is painted again.
            with_state(hwnd, |st, _| {
                for (old, new, name) in &renamed {
                    st.view.renamed(old, new, name);
                }
                everything(st)
            });
            return 0;
        }
        m if m == WM_ICON_AT.id() || m == WM_ICON_PLACE.id() => return answer(msg, wparam, lparam),
        windows_sys::Win32::UI::WindowsAndMessaging::WM_COPYDATA if lparam != 0 => {
            // SAFETY: Windows hands a COPYDATASTRUCT valid for this message.
            let data = unsafe { &*(lparam as *const windows_sys::Win32::System::DataExchange::COPYDATASTRUCT) };
            if data.dwData == TEST_DROP && !data.lpData.is_null() {
                // SAFETY: the sender's UTF-16 text of `cbData` bytes, copied by Windows.
                let text = unsafe { std::slice::from_raw_parts(data.lpData as *const u16, data.cbData as usize / 2) };
                return test_drop(&String::from_utf16_lossy(text)) as LRESULT;
            }
            if data.dwData == TEST_PROPERTIES && !data.lpData.is_null() {
                // SAFETY: as above.
                let text = unsafe { std::slice::from_raw_parts(data.lpData as *const u16, data.cbData as usize / 2) };
                return test_properties(&String::from_utf16_lossy(text)) as LRESULT;
            }
            return 0;
        }
        m if m == WM_TEST_COMMAND.id() => {
            let Some(command) = DesktopCommand::from_code(lparam as usize) else { return 0 };
            // Only what acts on test items (the third safety review of 2026-11-18, C4: a Paste
            // with wParam 0 took whatever was on the clipboard; Empty Recycle Bin, Arrange Icons
            // and New touched the person's own).
            if !test_command_allowed(command, wparam != 0) {
                log("desktop: a test's command was refused (it would not act on test items only)");
                return 0;
            }
            if wparam == 0 {
                with_state(hwnd, |st, font| st.view.update(font, |v| v.select_only(None)).1);
            } else {
                let i = STATE.with(|c| {
                    let borrow = c.borrow();
                    let st = borrow.as_ref()?;
                    let i = st.view.items.iter().position(|item| id_code(&item.id) as usize == wparam)?;
                    // Only the test's own items, never one of the owner's.
                    st.view.items[i].name.starts_with("Bevel95 test").then_some(i)
                });
                let Some(i) = i else {
                    log("desktop: a test's command was refused (not one of its own icons)");
                    return 0;
                };
                with_state(hwnd, |st, font| st.view.update(font, |v| v.select_only(Some(i))).1);
                if command == DesktopCommand::Rename {
                    begin_rename(hwnd, i);
                    return 1;
                }
            }
            // The selection (the test icon alone, or none) under a test's ticket: the bar carries
            // the command out on test items only (`file_ops::for_a_test`).
            let targets = STATE.with(|c| c.borrow().as_ref().map(|st| selection_targets(&st.view))).unwrap_or_default();
            let ticket = remember_test_targets(targets);
            // SAFETY: a message to the bar (a stale handle just fails).
            unsafe { PostMessageW(bar(), crate::bar::wm_desktop_command(), command.code(), command_param(ticket, true)) };
            return 1;
        }
        m if m == WM_CHECK.id() => {
            check("the tick", wparam as u64);
            return 0;
        }
        m if m == WM_GO.id() => {
            // SAFETY: our own window, on its own thread.
            unsafe { DestroyWindow(hwnd) };
            return 0;
        }
        WM_DESTROY => {
            bevel_winkit::dragdrop::revoke(hwnd);
            // Not by `hide` (which forgets the window first): Windows destroyed it with its
            // owner, Explorer's desktop window. The bar's tick makes it again.
            if DESKTOP.compare_exchange(hwnd as usize, 0, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
                log("desktop: its window went with Windows' desktop window (Explorer restarted?); it is made again once that is back");
            }
            // SAFETY: ends this thread's loop.
            unsafe { PostQuitMessage(0) };
            return 0;
        }
        // Nobody closes the desktop but `hide`.
        WM_CLOSE => return 0,
        _ => {}
    }
    // SAFETY: default handling for everything else.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use super::{
        colorref, command_of_param, command_param, id_code, items_wait, point_of_param, point_param, press_holds, remember_targets, targets_of,
        worth_a_line,
    };

    /// A recorded press holds new icons back only while a button is down: a right press let go
    /// over another window is given up at the next reading (the review of brief 90, R4).
    #[test]
    fn a_press_let_go_elsewhere_does_not_hold_the_icons() {
        assert!(press_holds(true, true));
        assert!(!press_holds(true, false), "let go elsewhere");
        assert!(!press_holds(false, true));
    }

    /// New icons wait while a double-click may still come, and only then (brief 90).
    #[test]
    fn new_icons_wait_out_a_double_click() {
        assert_eq!(items_wait(None, 5_000, 500), 0, "no press: at once");
        assert_eq!(items_wait(Some(4_800), 5_000, 500), 310, "200 ms after a press: the rest of the time");
        assert_eq!(items_wait(Some(4_400), 5_000, 500), 0, "its time is over");
        assert_eq!(items_wait(Some(u32::MAX - 99), 100, 500), 310, "across the tick count's wrap");
    }

    /// A live test's command acts on test items only (the third safety review of 2026-11-18, C4):
    /// no Empty Recycle Bin, Arrange Icons, New or switching the desktop off; the icon commands
    /// only on a test icon.
    #[test]
    fn a_tests_command_acts_on_test_items_only() {
        use super::test_command_allowed;
        use bevel_core::desktop::DesktopCommand as D;
        for c in [D::Open, D::Cut, D::Copy, D::Delete, D::Rename, D::ItemProperties, D::CreateShortcut] {
            assert!(test_command_allowed(c, true) && !test_command_allowed(c, false), "{c:?}");
        }
        for c in [D::Paste, D::PasteShortcut, D::Undo] {
            assert!(test_command_allowed(c, false) && test_command_allowed(c, true), "{c:?}: on test items only, by the bar");
        }
        for c in [D::EmptyRecycleBin, D::AutoArrange, D::LineUp, D::NewFolder, D::NewTextDocument, D::SwitchToWindowsDesktop] {
            assert!(!test_command_allowed(c, false) && !test_command_allowed(c, true), "{c:?}");
        }
    }

    /// The desktop's messages are registered by name: none is a fixed WM_APP number that another
    /// program's broadcast could hit (the third safety review of 2026-11-18, C4), and each has its
    /// own number.
    #[test]
    fn the_desktops_messages_are_registered_by_name() {
        let all = [
            &super::WM_DESKTOP_PRESS, &super::WM_DESKTOP_MENU, &super::WM_DESKTOP_ITEM_MENU, &super::WM_DESKTOP_RENAME_ASK,
            &super::WM_RENAME_ANSWER, &super::WM_RENAME, &super::WM_CUT, &super::WM_RENAMED, &super::WM_CHECK, &super::WM_GO,
            &super::WM_LOOK, &super::WM_ITEMS, &super::WM_ARRANGE, &super::WM_ICON_AT, &super::WM_ICON_PLACE, &super::WM_TEST_COMMAND,
        ];
        let mut ids: Vec<u32> = all.iter().map(|m| m.id()).collect();
        assert!(ids.iter().all(|&id| (0xC000..=0xFFFF).contains(&id)), "registered messages: {ids:x?}");
        assert!(all.iter().all(|m| m.name().starts_with("Bevel95.Desktop.")));
        assert!((0xC000..=0xFFFF).contains(&crate::bar::wm_desktop_command()));
        ids.push(crate::bar::wm_desktop_command());
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), n, "each its own");
    }

    /// A key's command or an open menu keeps the icons it was given, though another key or click
    /// remembers others before the bar handles it (the third safety review of 2026-11-18,
    /// suspected 2: one shared slot, so Ctrl+X on one icon and a right-click on another cut the
    /// second).
    #[test]
    fn each_command_keeps_the_icons_it_was_given() {
        use bevel_core::desktop::ItemKind;
        let target = |id: &str| crate::file_ops::Target { id: id.into(), name: id.into(), kind: ItemKind::File };
        let cut = remember_targets(vec![target("C:\\Desktop\\Bevel95 test one.txt")]);
        let menu = remember_targets(vec![target("C:\\Desktop\\Bevel95 test two.txt")]);
        assert_ne!(cut, menu);
        assert_eq!(targets_of(cut)[0].id, "C:\\Desktop\\Bevel95 test one.txt", "the later one did not replace it");
        assert_eq!(targets_of(menu)[0].id, "C:\\Desktop\\Bevel95 test two.txt");
        assert!(targets_of(0).is_empty(), "the desktop's own menu: no icons");
        // The ticket and the keyboard flag travel together; the menu's point keeps its sign.
        assert_eq!(command_of_param(command_param(cut, true)), (cut, true));
        assert_eq!(command_of_param(command_param(menu, false)), (menu, false));
        assert_eq!(point_of_param(point_param(-1280, 345)), (-1280, 345));
        // Only the last few are kept: a long-gone ticket names no icons.
        for _ in 0..20 {
            remember_targets(Vec::new());
        }
        assert!(targets_of(cut).is_empty());
    }

    /// The log names an opened icon by its kind only, never by its path (decisions.md, "No
    /// private text in the log").
    #[test]
    fn an_opened_icon_is_logged_by_its_kind_only() {
        use super::item_kind;
        assert_eq!(item_kind("::my-computer"), "My Computer");
        assert_eq!(item_kind("::my-documents"), "My Documents");
        assert_eq!(item_kind("::recycle-bin"), "the Recycle Bin");
        assert_eq!(item_kind("::something-new"), "one of the desktop's own icons");
        let file = item_kind(r"C:\Users\Someone\Desktop\Private letter.txt");
        assert_eq!(file, "a .txt file");
        let dir = std::env::temp_dir();
        let shown = item_kind(&dir.to_string_lossy());
        assert_eq!(shown, "a folder");
        for word in ["Someone", "Private", "letter", "Desktop"] {
            assert!(!file.contains(word) && !shown.contains(word), "{word} in {file:?} / {shown:?}");
        }
    }

    #[test]
    fn only_some_of_the_times_it_was_put_back_are_logged() {
        let logged: Vec<u32> = (1..=2500).filter(|&n| worth_a_line(n)).collect();
        assert_eq!(logged, [1, 2, 3, 10, 100, 1000]);
        assert!(worth_a_line(1_000_000) && !worth_a_line(2_000_000) && !worth_a_line(1_000_001));
    }

    #[test]
    fn colors_are_turned_round_for_gdi() {
        // Windows 2000's desktop blue, 3A6EA5: GDI wants blue in the high byte.
        assert_eq!(colorref(0xFF3A_6EA5), 0x00A5_6E3A);
        assert_eq!(colorref(0xFF00_8080), 0x0080_8000);
    }

    #[test]
    fn ids_have_the_numbers_the_live_tests_work_out() {
        // FNV-1a, 32 bits (private/probes/desktop-move works out the same in PowerShell).
        assert_eq!(id_code(""), 0x811C_9DC5);
        assert_eq!(id_code("a"), 0xE40C_292C);
        assert_ne!(id_code("::my-computer"), 0);
    }
}
