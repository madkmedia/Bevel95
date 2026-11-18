//! The notification area ("tray"). Programs show their little icons there with
//! Shell_NotifyIcon, which hands each icon to the first window of class "Shell_TrayWnd" that
//! FindWindow finds. Like RetroBar (its ManagedShell library's TrayService), Bevel95 makes a
//! hidden window of that class (marked with `explorer::STAND_IN_MARK`), keeps it in front of
//! Windows' taskbar window so it is found first, and asks every program to send its icons again
//! with "TaskbarCreated" (what Explorer broadcasts when it restarts).
//!
//! Everything this window receives is also passed on to Windows' taskbar, so that stays up to
//! date: programs' notifications keep popping up as Windows shows them today, and switching back
//! (Ctrl+Win+T) or exiting shows the same icons. Nothing is injected or hooked; when Bevel95
//! ends, its window is gone and programs talk to Windows' taskbar directly again (the bar, the
//! watchdog and `--restore` also broadcast "TaskbarCreated" then, so any icon that changed in
//! between is sent to Windows' taskbar afresh).
//!
//! The message layouts are undocumented but have not changed since Windows 95; they use 32-bit
//! handles so 32- and 64-bit programs can share them. They are read here exactly as
//! ManagedShell (`NativeMethods.Shell32.cs`: SHELLTRAYDATA, WINNOTIFYICONIDENTIFIER) declares
//! them.

use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr::{null, null_mut};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::mpsc::Sender;
use std::time::Instant;

use bevel_core::fb::Icon;
use windows_sys::Win32::Foundation::{HANDLE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::System::DataExchange::COPYDATASTRUCT;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetDoubleClickTime;
use windows_sys::Win32::UI::Shell::{
    NIF_GUID, NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_STATE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NIM_SETFOCUS,
    NIM_SETVERSION, NIN_POPUPCLOSE, NIN_POPUPOPEN, NIN_SELECT, NIS_HIDDEN, NOTIFYICON_VERSION_4,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, CreateWindowExW, DefWindowProcW, DestroyWindow, FindWindowW, GWL_STYLE,
    GetSystemMetrics, GetWindowLongPtrW, GetWindowThreadProcessId, HWND_BROADCAST, HWND_TOPMOST,
    IsWindow, PostMessageW, RegisterWindowMessageW, SM_CXDOUBLECLK, SM_CYDOUBLECLK, SMTO_ABORTIFHUNG, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SendMessageTimeoutW, SendNotifyMessageW, SetPropW,
    SetWindowLongPtrW, SetWindowPos, WINDOWPOS, WM_ACTIVATEAPP, WM_COMMAND, WM_CONTEXTMENU, WM_COPYDATA,
    WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDBLCLK, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEMOVE,
    WM_RBUTTONDBLCLK, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_USER, WM_WINDOWPOSCHANGED, WS_CHILD, WS_CLIPCHILDREN,
    WS_CLIPSIBLINGS, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, WS_VISIBLE,
};

use crate::explorer;
use crate::log::{log, log_win32};
use crate::util::{rect_h, rect_w, wide};
use crate::window_list;

pub const TRAY_CLASS: &str = "Shell_TrayWnd";
pub const NOTIFY_CLASS: &str = "TrayNotifyWnd";

/// Our stand-in "Shell_TrayWnd" and its "TrayNotifyWnd" child (0 while there is none).
static TRAY: AtomicUsize = AtomicUsize::new(0);
static NOTIFY: AtomicUsize = AtomicUsize::new(0);
/// The "TaskbarCreated" message.
static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);
/// Our own "TaskbarCreated" broadcast is going on (our windows must not take it for Explorer
/// restarting).
static OWN_BROADCAST: AtomicBool = AtomicBool::new(false);
/// How often our window had to be brought back in front of Windows' taskbar window.
static RAISES: AtomicU32 = AtomicU32::new(0);
/// Programs are asked for their icons once more this long after the first time: while
/// Windows' taskbar settles after being hidden, it comes in front of our window for a moment
/// or two, and programs that answer slowly then send their icons there (seen with a chat
/// program and a web browser).
const ASK_AGAIN_AFTER: std::time::Duration = std::time::Duration::from_secs(3);

thread_local! {
    /// When programs were last asked, if they are to be asked once more.
    static ASK_AGAIN: std::cell::Cell<Option<Instant>> = const { std::cell::Cell::new(None) };
}

/// "Shell_TrayWnd" messages that are posted on, not sent (as ManagedShell does: the sender of
/// WM_USER + 372 does not wait for an answer).
const POSTED_ON: u32 = WM_USER + 372;
/// The signature at the start of SHELLTRAYDATA and WINNOTIFYICONIDENTIFIER.
const SIGNATURE: u32 = 0x3475_3423;
/// How long a message passed on to Windows' taskbar may take (it answers in a moment; this only
/// matters if Explorer hangs).
const FORWARD_TIMEOUT_MS: u32 = 2000;

/// One Shell_NotifyIcon call, as it arrives in WM_COPYDATA (dwData 1, SHELLTRAYDATA).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NotifyData {
    pub message: u32,
    pub hwnd: isize,
    pub uid: u32,
    pub flags: u32,
    pub callback: u32,
    pub icon: isize,
    pub tip: String,
    pub state: u32,
    pub state_mask: u32,
    pub version: u32,
    pub guid: [u8; 16],
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    b.get(at..at + 4).map_or(0, |s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// A 32-bit handle, sign-extended as Windows does for 64-bit programs.
fn handle_at(b: &[u8], at: usize) -> isize {
    u32_at(b, at) as i32 as isize
}

/// UTF-16 text of at most `chars` characters, up to its terminating zero.
fn text_at(b: &[u8], at: usize, chars: usize) -> String {
    let units: Vec<u16> = (0..chars)
        .map_while(|i| b.get(at + 2 * i..at + 2 * i + 2).map(|s| u16::from_le_bytes([s[0], s[1]])))
        .take_while(|&u| u != 0)
        .collect();
    String::from_utf16_lossy(&units)
}

fn guid_at(b: &[u8], at: usize) -> [u8; 16] {
    let mut guid = [0u8; 16];
    if let Some(s) = b.get(at..at + 16) {
        guid.copy_from_slice(s);
    }
    guid
}

/// Reads SHELLTRAYDATA: a signature, the NIM_ message, then NOTIFYICONDATAW with 32-bit
/// handles (cbSize 0, hWnd 4, uID 8, uFlags 12, uCallbackMessage 16, hIcon 20, szTip[128] 24,
/// dwState 280, dwStateMask 284, szInfo[256] 288, uVersion 800, szInfoTitle[64] 804,
/// dwInfoFlags 932, guidItem 936, hBalloonIcon 952: 956 bytes), each 8 bytes further in.
/// Fields past the end of what a program sent read as zero.
pub fn parse_notify(b: &[u8]) -> Option<NotifyData> {
    if b.len() < 8 + 24 {
        return None;
    }
    Some(NotifyData {
        message: u32_at(b, 4),
        hwnd: handle_at(b, 12),
        uid: u32_at(b, 16),
        flags: u32_at(b, 20),
        callback: u32_at(b, 24),
        icon: handle_at(b, 28),
        tip: text_at(b, 32, 128),
        state: u32_at(b, 288),
        state_mask: u32_at(b, 292),
        version: u32_at(b, 808),
        guid: guid_at(b, 944),
    })
}

/// A Shell_NotifyIconGetRect question (WM_COPYDATA dwData 3, WINNOTIFYICONIDENTIFIER): which
/// corner (1 = top left, 2 = bottom right) of which icon.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RectQuestion {
    pub corner: u32,
    pub hwnd: isize,
    pub uid: u32,
    pub guid: [u8; 16],
}

pub fn parse_rect_question(b: &[u8]) -> Option<RectQuestion> {
    (b.len() >= 24).then(|| RectQuestion { corner: u32_at(b, 4), hwnd: handle_at(b, 16), uid: u32_at(b, 20), guid: guid_at(b, 24) })
}

/// Identifies an icon: its window and number (a program may have several).
pub type Key = (isize, u32);

/// Windows' own volume icon (Explorer's, GUID 7820ae73-23e3-4229-82c1-e41cb67d5b9c, as
/// ManagedShell lists it): Bevel95 shows the classic speaker and volume control in its place.
pub const VOLUME_GUID: [u8; 16] = [0x73, 0xae, 0x20, 0x78, 0xe3, 0x23, 0x29, 0x42, 0x82, 0xc1, 0xe4, 0x1c, 0xb6, 0x7d, 0x5b, 0x9c];

/// One icon in the notification area.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrayIcon {
    pub hwnd: isize,
    pub uid: u32,
    /// The icon's GUID, all zeros if it has none.
    pub guid: [u8; 16],
    /// The message our clicks are sent as (0: none asked for).
    pub callback: u32,
    pub version: u32,
    pub tip: String,
    /// Version 4 icons show the plain tooltip only when they ask for it (NIF_SHOWTIP).
    pub show_tip: bool,
    pub picture: Option<Icon>,
    pub hidden: bool,
}

impl TrayIcon {
    pub fn key(&self) -> Key {
        (self.hwnd, self.uid)
    }

    /// Windows' volume icon (see `VOLUME_GUID`).
    pub fn is_volume(&self) -> bool {
        self.guid == VOLUME_GUID
    }

    fn is(&self, d: &NotifyData) -> bool {
        (self.hwnd == d.hwnd && self.uid == d.uid) || (d.flags & NIF_GUID != 0 && d.guid != [0; 16] && self.guid == d.guid)
    }

    /// The text its tooltip shows (none for version 4 icons that draw their own).
    pub fn tooltip(&self) -> Option<&str> {
        (!self.tip.is_empty() && (self.version < NOTIFYICON_VERSION_4 || self.show_tip)).then_some(self.tip.as_str())
    }
}

/// What applying a Shell_NotifyIcon call did: its answer to the program, whether the icons
/// shown changed, and whether it brought a new icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Applied {
    pub ok: bool,
    pub changed: bool,
    pub added: bool,
}

/// Programs draw their icons for today's taskbar, which is dark by default: many are white
/// symbols that would vanish on the classic gray. An icon whose visible pixels are all light
/// grays is drawn dark instead (its colors inverted; RetroBar inverts Windows' own system
/// icons the same way). Colored icons are left as they are.
pub fn darken_if_light(icon: Icon) -> Icon {
    let channels = |p: u32| [(p >> 16) & 0xFF, (p >> 8) & 0xFF, p & 0xFF];
    let visible: Vec<u32> = icon.pixels.iter().copied().filter(|p| p >> 24 >= 0x80).collect();
    let gray_and_light = |p: u32| {
        let [r, g, b] = channels(p);
        r.max(g).max(b) - r.min(g).min(b) <= 40 && r + g + b >= 3 * 0xB0
    };
    if visible.is_empty() || !visible.iter().all(|&p| gray_and_light(p)) {
        return icon;
    }
    let pixels = icon.pixels.iter().map(|&p| (p & 0xFF00_0000) | (!p & 0x00FF_FFFF)).collect();
    Icon { pixels, ..icon }
}

/// Applies one Shell_NotifyIcon call to `icons`, the way Explorer does; `picture` turns the
/// program's icon handle into pixels (it must be read at once: programs often destroy it right
/// after the call). New icons join the right end, next to the clock, as on Windows 98 on the
/// reference PC (Windows XP puts them at the left; PROVISIONAL for 95 and 2000).
pub fn apply(icons: &mut Vec<TrayIcon>, d: &NotifyData, picture: impl FnOnce(isize) -> Option<Icon>) -> Applied {
    let found = icons.iter().position(|i| i.is(d));
    let nothing = Applied { ok: false, changed: false, added: false };
    match d.message {
        NIM_ADD | NIM_MODIFY => {
            if found.is_none() && d.hwnd == 0 {
                return nothing;
            }
            let index = found.unwrap_or_else(|| {
                let fresh = TrayIcon {
                    hwnd: d.hwnd,
                    uid: d.uid,
                    guid: [0; 16],
                    callback: 0,
                    version: 0,
                    tip: String::new(),
                    show_tip: false,
                    picture: None,
                    hidden: false,
                };
                icons.push(fresh);
                icons.len() - 1
            });
            let icon = &mut icons[index];
            let before = (icon.picture.clone(), icon.hidden);
            if d.flags & NIF_MESSAGE != 0 {
                icon.callback = d.callback;
            }
            if d.flags & NIF_ICON != 0 {
                icon.picture = if d.icon == 0 { None } else { picture(d.icon).map(darken_if_light) };
            }
            if d.flags & NIF_TIP != 0 {
                icon.tip = d.tip.clone();
            }
            if d.flags & NIF_STATE != 0 && d.state_mask & NIS_HIDDEN != 0 {
                icon.hidden = d.state & NIS_HIDDEN != 0;
            }
            if d.flags & NIF_GUID != 0 {
                icon.guid = d.guid;
            }
            if d.flags & NIF_SHOWTIP != 0 {
                icon.show_tip = true;
            }
            if d.hwnd != 0 {
                (icon.hwnd, icon.uid) = (d.hwnd, d.uid);
            }
            let changed = found.is_none() || before != (icon.picture.clone(), icon.hidden);
            // A change to an icon we never heard of is still shown (the program added it before
            // we started and did not answer "TaskbarCreated"), but answered with an error, as
            // Explorer does; programs then add it properly.
            Applied { ok: found.is_some() || d.message == NIM_ADD, changed, added: found.is_none() }
        }
        NIM_DELETE => match found {
            Some(index) => {
                icons.remove(index);
                Applied { ok: true, changed: true, added: false }
            }
            None => nothing,
        },
        NIM_SETVERSION => match found {
            Some(index) if d.version <= NOTIFYICON_VERSION_4 => {
                icons[index].version = d.version;
                Applied { ok: true, ..nothing }
            }
            _ => nothing,
        },
        NIM_SETFOCUS => Applied { ok: true, ..nothing },
        _ => nothing,
    }
}

thread_local! {
    static ICONS: RefCell<Vec<TrayIcon>> = const { RefCell::new(Vec::new()) };
    /// The last button press on an icon, to tell a double-click.
    static LAST_PRESS: RefCell<Option<(Key, Button, Instant, POINT)>> = const { RefCell::new(None) };
}

/// Called when the icons shown change (the bar repaints).
static ON_CHANGE: AtomicUsize = AtomicUsize::new(0);

/// The icons to show, left to right (hidden ones left out).
pub fn shown() -> Vec<TrayIcon> {
    ICONS.with(|icons| icons.borrow().iter().filter(|i| !i.hidden).cloned().collect())
}

/// Forgets icons whose program's window is gone (it ended without removing them), as
/// Explorer does. Returns true if any went.
pub fn prune() -> bool {
    ICONS.with(|icons| {
        let mut icons = icons.borrow_mut();
        let before = icons.len();
        // SAFETY: IsWindow accepts any value.
        icons.retain(|i| unsafe { IsWindow(i.hwnd as HWND) } != 0);
        icons.len() != before
    })
}

/// Creates the stand-in windows over the bar (`bar_rect`, screen pixels), puts them in front of
/// Windows' taskbar window and asks every program for its icons. `bar` gets repainted when they
/// change.
pub fn start(bar: HWND, bar_rect: RECT) {
    if TRAY.load(Ordering::SeqCst) != 0 {
        return;
    }
    ON_CHANGE.store(bar as usize, Ordering::SeqCst);
    let created = wide("TaskbarCreated");
    // SAFETY: a valid null-terminated string.
    TASKBAR_CREATED.store(unsafe { RegisterWindowMessageW(created.as_ptr()) }, Ordering::SeqCst);
    let (tray_class, notify_class, mark) = (wide(TRAY_CLASS), wide(NOTIFY_CLASS), wide(explorer::STAND_IN_MARK));
    let empty = wide("");
    // SAFETY: classes registered by bar::run with `tray_proc`; handles checked.
    unsafe {
        let instance = GetModuleHandleW(null());
        let tray = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            tray_class.as_ptr(),
            empty.as_ptr(),
            WS_POPUP | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
            bar_rect.left,
            bar_rect.top,
            rect_w(&bar_rect),
            rect_h(&bar_rect),
            null_mut(),
            null_mut(),
            instance,
            null(),
        );
        if tray.is_null() {
            log_win32("CreateWindowExW(Shell_TrayWnd stand-in): no notification area");
            return;
        }
        // The mark goes on before any program can find the window.
        SetPropW(tray, mark.as_ptr(), 1 as HANDLE);
        TRAY.store(tray as usize, Ordering::SeqCst);
        let notify = CreateWindowExW(
            0,
            notify_class.as_ptr(),
            null(),
            WS_CHILD | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
            0,
            0,
            rect_w(&bar_rect),
            rect_h(&bar_rect),
            tray,
            null_mut(),
            instance,
            null(),
        );
        if notify.is_null() {
            log_win32("CreateWindowExW(TrayNotifyWnd stand-in)");
        }
        NOTIFY.store(notify as usize, Ordering::SeqCst);
    }
    keep_first();
    log("notification area: our window is up; asking programs for their icons");
    broadcast_taskbar_created();
    ASK_AGAIN.with(|a| a.set(Some(Instant::now())));
}

/// Asks programs for their icons again, a few seconds from now (after Explorer restarted).
pub fn ask_again_soon() {
    ASK_AGAIN.with(|a| a.set(Some(Instant::now())));
}

/// Destroys the stand-in windows (programs then find Windows' taskbar again) and forgets the
/// icons. The caller broadcasts "TaskbarCreated" once Windows' taskbar is back.
pub fn stop() {
    let tray = TRAY.swap(0, Ordering::SeqCst);
    NOTIFY.store(0, Ordering::SeqCst);
    if tray != 0 {
        // SAFETY: our own window (its child goes with it).
        unsafe { DestroyWindow(tray as HWND) };
    }
    ICONS.with(|icons| icons.borrow_mut().clear());
}

/// Asks every program to add its icons (again) to whatever "Shell_TrayWnd" it finds first.
pub fn broadcast_taskbar_created() {
    let mut message = TASKBAR_CREATED.load(Ordering::SeqCst);
    if message == 0 {
        let name = wide("TaskbarCreated");
        // SAFETY: a valid null-terminated string.
        message = unsafe { RegisterWindowMessageW(name.as_ptr()) };
    }
    OWN_BROADCAST.store(true, Ordering::SeqCst);
    // SAFETY: a broadcast that does not wait for other programs (our own windows handle it
    // before the call returns).
    unsafe { SendNotifyMessageW(HWND_BROADCAST, message, 0, 0) };
    OWN_BROADCAST.store(false, Ordering::SeqCst);
}

/// True while our own "TaskbarCreated" broadcast is being handled.
pub fn own_broadcast() -> bool {
    OWN_BROADCAST.load(Ordering::SeqCst)
}

/// Keeps our window the first "Shell_TrayWnd" (Explorer brings its own back to the front now
/// and then; RetroBar's library checks every 100 ms too), and asks programs for their icons
/// once more when it is time (see `ASK_AGAIN_AFTER`).
pub fn keep_first() {
    let tray = TRAY.load(Ordering::SeqCst) as HWND;
    if tray.is_null() {
        return;
    }
    let due = ASK_AGAIN.with(|a| a.get().is_some_and(|t| t.elapsed() >= ASK_AGAIN_AFTER));
    if due {
        ASK_AGAIN.with(|a| a.set(None));
        broadcast_taskbar_created();
        let n = ICONS.with(|icons| icons.borrow().len());
        log(&format!("notification area: asked programs for their icons once more ({n} icons so far)"));
    }
    let class = wide(TRAY_CLASS);
    // SAFETY: plain Win32 calls on our own window.
    unsafe {
        if FindWindowW(class.as_ptr(), null()) != tray {
            SetWindowPos(tray, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            let n = RAISES.fetch_add(1, Ordering::SeqCst) + 1;
            if n <= 3 || n.is_multiple_of(100) {
                log(&format!("notification area: Windows' taskbar window came first; ours is in front again (time {n})"));
            }
        }
    }
}

/// Moves the stand-ins over the bar and the notification area's well (screen pixels): programs
/// that look for the taskbar or its notification area to place their windows find ours.
pub fn place(bar_rect: RECT, well: RECT) {
    let (tray, notify) = (TRAY.load(Ordering::SeqCst) as HWND, NOTIFY.load(Ordering::SeqCst) as HWND);
    if tray.is_null() {
        return;
    }
    // SAFETY: our own windows; the child is placed relative to its parent.
    unsafe {
        let flags = SWP_NOZORDER | SWP_NOACTIVATE;
        SetWindowPos(tray, null_mut(), bar_rect.left, bar_rect.top, rect_w(&bar_rect), rect_h(&bar_rect), flags);
        if !notify.is_null() {
            let (x, y) = (well.left - bar_rect.left, well.top - bar_rect.top);
            SetWindowPos(notify, null_mut(), x, y, rect_w(&well), rect_h(&well), flags);
        }
    }
}

// Where the icons are on the screen now, for Shell_NotifyIconGetRect (set at each paint).
thread_local! {
    static ON_SCREEN: RefCell<(Vec<(Key, RECT)>, RECT)> = const { RefCell::new((Vec::new(), RECT { left: 0, top: 0, right: 0, bottom: 0 })) };
}

/// Records where each shown icon and the whole well are on the screen.
pub fn set_screen_rects(icons: Vec<(Key, RECT)>, well: RECT) {
    ON_SCREEN.with(|s| *s.borrow_mut() = (icons, well));
}

/// Shell_NotifyIconGetRect: a corner of the icon asked about, packed as (x, y); for an icon we
/// do not show (as Windows' volume flyout asks about its own), the well's.
fn answer_rect(q: &RectQuestion) -> LRESULT {
    let key = ICONS.with(|icons| {
        icons
            .borrow()
            .iter()
            .find(|i| (i.hwnd == q.hwnd && i.uid == q.uid) || (q.guid != [0; 16] && i.guid == q.guid))
            .map(TrayIcon::key)
    });
    let r = ON_SCREEN.with(|s| {
        let s = s.borrow();
        key.and_then(|k| s.0.iter().find(|(key, _)| *key == k).map(|(_, r)| *r)).unwrap_or(s.1)
    });
    let (x, y) = if q.corner == 2 { (r.right, r.bottom) } else { (r.left, r.top) };
    ((y as u16 as u32) << 16 | (x as u16 as u32)) as LRESULT
}

/// A mouse button, for the messages sent to an icon's program.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
    Middle,
}

/// The icon with `key`, if it is still there.
fn find(key: Key) -> Option<TrayIcon> {
    ICONS.with(|icons| icons.borrow().iter().find(|i| i.key() == key).cloned())
}

/// Sends `message` to an icon's program the way Explorer does: version 4 icons get the pointer
/// position in wParam and (message, icon number) in lParam; older ones the icon number and the
/// message. Never waits for the program.
fn notify(icon: &TrayIcon, message: u32, at: POINT) {
    if icon.callback == 0 {
        return;
    }
    let (wparam, lparam) = if icon.version >= NOTIFYICON_VERSION_4 {
        let xy = (at.y as u16 as u32) << 16 | (at.x as u16 as u32);
        (xy as WPARAM, (message | (icon.uid << 16)) as LPARAM)
    } else {
        (icon.uid as WPARAM, message as LPARAM)
    };
    // SAFETY: a message that does not wait for the other program.
    unsafe { SendNotifyMessageW(icon.hwnd as HWND, icon.callback, wparam, lparam) };
}

/// Whether the icon with `key` is Windows' volume icon.
pub fn is_volume(key: Key) -> bool {
    find(key).is_some_and(|i| i.is_volume())
}

/// The tooltip text of an icon (none for version 4 icons that show their own).
pub fn tooltip_of(key: Key) -> Option<String> {
    find(key)?.tooltip().map(str::to_string)
}

/// The pointer moved over an icon.
pub fn mouse_move(key: Key, at: POINT) {
    if let Some(icon) = find(key) {
        notify(&icon, WM_MOUSEMOVE, at);
    }
}

/// The pointer came onto an icon or left it: version 4 icons show or hide their own pop-up.
pub fn hover(key: Key, entered: bool, at: POINT) {
    if let Some(icon) = find(key).filter(|i| i.version >= NOTIFYICON_VERSION_4) {
        notify(&icon, if entered { NIN_POPUPOPEN } else { NIN_POPUPCLOSE }, at);
    }
}

/// A button went down on an icon: its program may now come to the front (to show a menu), and
/// hears of the press, or of a double-click if it is the second press in a row.
pub fn mouse_down(key: Key, button: Button, at: POINT) {
    let Some(icon) = find(key) else {
        return;
    };
    let mut pid = 0;
    // SAFETY: plain Win32 calls; we got the click, so we may hand on the right to the front.
    unsafe {
        GetWindowThreadProcessId(icon.hwnd as HWND, &mut pid);
        if pid != 0 && AllowSetForegroundWindow(pid) == 0 {
            log_win32("AllowSetForegroundWindow (a notification icon's menu may open behind other windows)");
        }
    }
    let double = LAST_PRESS.with(|last| {
        let now = Instant::now();
        // SAFETY: plain queries.
        let (time, dx, dy) =
            unsafe { (GetDoubleClickTime(), GetSystemMetrics(SM_CXDOUBLECLK), GetSystemMetrics(SM_CYDOUBLECLK)) };
        let double = last.borrow().is_some_and(|(k, b, when, p): (Key, Button, Instant, POINT)| {
            k == key
                && b == button
                && now.duration_since(when).as_millis() <= u128::from(time)
                && (p.x - at.x).abs() <= dx / 2
                && (p.y - at.y).abs() <= dy / 2
        });
        *last.borrow_mut() = if double { None } else { Some((key, button, now, at)) };
        double
    });
    let message = match (button, double) {
        (Button::Left, false) => WM_LBUTTONDOWN,
        (Button::Left, true) => WM_LBUTTONDBLCLK,
        (Button::Right, false) => WM_RBUTTONDOWN,
        (Button::Right, true) => WM_RBUTTONDBLCLK,
        (Button::Middle, false) => WM_MBUTTONDOWN,
        (Button::Middle, true) => WM_MBUTTONDBLCLK,
    };
    notify(&icon, message, at);
}

/// A button came up on an icon. Explorer also sends version 3 and 4 icons NIN_SELECT after the
/// left button and WM_CONTEXTMENU after the right one (documented for version 4 only).
pub fn mouse_up(key: Key, button: Button, at: POINT) {
    let Some(icon) = find(key) else {
        return;
    };
    match button {
        Button::Left => {
            notify(&icon, WM_LBUTTONUP, at);
            if icon.version >= 3 {
                notify(&icon, NIN_SELECT, at);
            }
        }
        Button::Right => {
            notify(&icon, WM_RBUTTONUP, at);
            if icon.version >= 3 {
                notify(&icon, WM_CONTEXTMENU, at);
            }
        }
        Button::Middle => notify(&icon, WM_MBUTTONUP, at),
    }
}

/// Passes a message on to Windows' taskbar window and returns its answer.
fn forward(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let windows = explorer::main_tray();
    if windows.is_null() {
        // SAFETY: default handling.
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    }
    // SAFETY: WM_COPYDATA's data is copied to Explorer by Windows; the other messages carry no
    // pointers (their senders meant them for another process anyway).
    unsafe {
        if msg == POSTED_ON {
            PostMessageW(windows, msg, wparam, lparam);
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        let mut result = 0usize;
        if SendMessageTimeoutW(windows, msg, wparam, lparam, SMTO_ABORTIFHUNG, FORWARD_TIMEOUT_MS, &mut result) == 0 {
            log(&format!("notification area: Windows' taskbar did not take message {msg:#x} in time"));
        }
        result as LRESULT
    }
}

/// Icon messages (Shell_NotifyIcon calls) waiting to be passed on to Windows' taskbar by a thread
/// of their own, in the order they came, and how many are waiting. Our answer to the program does
/// not depend on Explorer's, so the bar need not wait for it: a hung Explorer used to stall the
/// bar for up to 2 s per icon message (the code review of 2026-11-09). Messages whose answer
/// comes from Explorer (appbar questions and the like) are still passed on at once (`forward`).
type Waiting = (WPARAM, usize, Vec<u8>);
static FORWARDER: Mutex<Option<Sender<Waiting>>> = Mutex::new(None);
static BACKLOG: AtomicUsize = AtomicUsize::new(0);
/// More than this many waiting (Explorer hung for minutes): new ones are dropped. When Bevel95
/// ends, the "TaskbarCreated" broadcast has every program send its icons to Explorer afresh.
const BACKLOG_LIMIT: usize = 500;

/// Queues a copy of an icon message for Windows' taskbar (see `FORWARDER`).
fn forward_later(wparam: WPARAM, kind: usize, data: &[u8]) {
    let mut forwarder = FORWARDER.lock().unwrap_or_else(|e| e.into_inner());
    if forwarder.is_none() {
        let (sender, receiver) = std::sync::mpsc::channel::<Waiting>();
        let spawned = std::thread::Builder::new().name("tray forwarder".into()).spawn(move || {
            for (wparam, kind, data) in receiver {
                send_copydata(wparam, kind, &data);
                BACKLOG.fetch_sub(1, Ordering::SeqCst);
            }
        });
        match spawned {
            Ok(_) => *forwarder = Some(sender),
            Err(e) => {
                log(&format!("notification area: no forwarding thread ({e}); passing the message on at once"));
                return send_copydata(wparam, kind, data);
            }
        }
    }
    if BACKLOG.load(Ordering::SeqCst) >= BACKLOG_LIMIT {
        static WARNED: AtomicBool = AtomicBool::new(false);
        if !WARNED.swap(true, Ordering::SeqCst) {
            log("notification area: Windows' taskbar is not taking icon messages; new ones are no longer passed on");
        }
        return;
    }
    BACKLOG.fetch_add(1, Ordering::SeqCst);
    if let Some(sender) = forwarder.as_ref()
        && sender.send((wparam, kind, data.to_vec())).is_err()
    {
        BACKLOG.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Sends a WM_COPYDATA with `data` to Windows' taskbar window (any thread).
fn send_copydata(wparam: WPARAM, kind: usize, data: &[u8]) {
    let windows = explorer::main_tray();
    if windows.is_null() {
        return;
    }
    let cds = COPYDATASTRUCT { dwData: kind, cbData: data.len() as u32, lpData: data.as_ptr() as *mut c_void };
    let mut result = 0usize;
    // SAFETY: `cds` and `data` live until SendMessageTimeoutW returns; Windows copies the data.
    let sent = unsafe {
        SendMessageTimeoutW(
            windows,
            WM_COPYDATA,
            wparam,
            &cds as *const COPYDATASTRUCT as LPARAM,
            SMTO_ABORTIFHUNG,
            FORWARD_TIMEOUT_MS,
            &mut result,
        )
    };
    if sent == 0 {
        log("notification area: Windows' taskbar did not take an icon message in time");
    } else {
        static CONFIRMED: AtomicBool = AtomicBool::new(false);
        if !CONFIRMED.swap(true, Ordering::SeqCst) {
            log(&format!("notification area: icon messages reach Windows' taskbar (its first answer: {result})"));
        }
    }
}

/// A Shell_NotifyIcon call arrived.
fn on_notify(d: &NotifyData) -> bool {
    let applied = ICONS.with(|icons| apply(&mut icons.borrow_mut(), d, |handle| window_list::icon_pixels_fitted(handle as _)));
    let bar = ON_CHANGE.load(Ordering::SeqCst) as HWND;
    if applied.changed {
        // SAFETY: our own window; only queues a repaint.
        unsafe { windows_sys::Win32::Graphics::Gdi::InvalidateRect(bar, null(), 0) };
    }
    // Windows changes its volume icon when the volume or mute changes: the bar looks again
    // whether the speaker should show the "no" sign.
    if (d.flags & NIF_GUID != 0 && d.guid == VOLUME_GUID) || is_volume((d.hwnd, d.uid)) {
        // SAFETY: posting to our own window.
        unsafe { PostMessageW(bar, crate::bar::WM_VOLUME_CHANGED, 0, 0) };
    }
    if applied.added {
        let how = if d.message == NIM_ADD { "added" } else { "changed before it was added" };
        // The program's file name, never its tooltip (a Wi-Fi network's name, a file's).
        let with = if d.tip.is_empty() { "without a" } else { "with a" };
        log(&format!(
            "notification area: {how}: an icon of {}, {with} tooltip{}",
            program_of(d.hwnd),
            if d.flags & NIF_GUID != 0 { format!(", {}", guid_text(&d.guid)) } else { String::new() }
        ));
    }
    applied.ok
}

/// The program file name behind a window, for the log.
fn program_of(hwnd: isize) -> String {
    let path = window_list::program_path(hwnd as HWND);
    std::path::Path::new(&path).file_name().map_or_else(|| "?".to_string(), |f| f.to_string_lossy().into_owned())
}

/// A GUID in its usual written form.
pub fn guid_text(g: &[u8; 16]) -> String {
    let d1 = u32::from_le_bytes([g[0], g[1], g[2], g[3]]);
    let d2 = u16::from_le_bytes([g[4], g[5]]);
    let d3 = u16::from_le_bytes([g[6], g[7]]);
    format!(
        "{d1:08x}-{d2:04x}-{d3:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        g[8], g[9], g[10], g[11], g[12], g[13], g[14], g[15]
    )
}

/// The window procedure of both stand-in windows.
pub unsafe extern "system" fn tray_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_COPYDATA if lparam != 0 => {
            // SAFETY: Windows passes a valid COPYDATASTRUCT whose data lives for this call.
            let (kind, data) = unsafe {
                let cds = &*(lparam as *const COPYDATASTRUCT);
                let data = if cds.lpData.is_null() {
                    &[][..]
                } else {
                    std::slice::from_raw_parts(cds.lpData as *const u8, cds.cbData as usize)
                };
                (cds.dwData, data)
            };
            match kind {
                1 => {
                    if let Some(d) = parse_notify(data) {
                        if u32_at(data, 0) != SIGNATURE {
                            log("notification area: an icon message without the usual signature");
                        }
                        let ok = on_notify(&d);
                        forward_later(wparam, kind, data);
                        return LRESULT::from(ok);
                    }
                }
                3 => {
                    if let Some(q) = parse_rect_question(data) {
                        return answer_rect(&q);
                    }
                }
                _ => {}
            }
            return forward(hwnd, msg, wparam, lparam);
        }
        // Someone showed the stand-in (programs do that to "show the taskbar"): it stays hidden.
        WM_WINDOWPOSCHANGED if lparam != 0 => {
            // SAFETY: Windows passes a valid WINDOWPOS.
            let shown = unsafe { (*(lparam as *const WINDOWPOS)).flags } & SWP_SHOWWINDOW != 0;
            if shown {
                // SAFETY: our own window; clearing the style bit hides it without another round
                // of messages (as ManagedShell does).
                unsafe {
                    let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
                    SetWindowLongPtrW(hwnd, GWL_STYLE, style & !(WS_VISIBLE as isize));
                }
                log("notification area: a program showed our hidden window; hidden again");
            }
        }
        // Everyone gets "TaskbarCreated" (ours or Explorer's); Windows' taskbar needs no copy.
        _ if msg != 0 && msg == TASKBAR_CREATED.load(Ordering::SeqCst) => return 0,
        WM_COMMAND | WM_ACTIVATEAPP => return forward(hwnd, msg, wparam, lparam),
        _ if msg >= WM_USER => return forward(hwnd, msg, wparam, lparam),
        _ => {}
    }
    // SAFETY: default handling for everything else.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A SHELLTRAYDATA buffer as Shell_NotifyIcon builds it.
    fn message(nim: u32, hwnd: u32, uid: u32, flags: u32, tip: &str) -> Vec<u8> {
        let mut b = vec![0u8; 8 + 956];
        let put = |b: &mut Vec<u8>, at: usize, v: u32| b[at..at + 4].copy_from_slice(&v.to_le_bytes());
        put(&mut b, 0, SIGNATURE);
        put(&mut b, 4, nim);
        put(&mut b, 8, 956);
        put(&mut b, 12, hwnd);
        put(&mut b, 16, uid);
        put(&mut b, 20, flags);
        put(&mut b, 24, 0x8001);
        put(&mut b, 28, 0xABCD);
        for (i, u) in tip.encode_utf16().enumerate() {
            b[32 + 2 * i..34 + 2 * i].copy_from_slice(&u.to_le_bytes());
        }
        b
    }

    #[test]
    fn reads_shell_notify_icon_messages() {
        let mut b = message(NIM_ADD, 0xFFFF_FFF0, 7, NIF_MESSAGE | NIF_ICON | NIF_TIP, "Volume");
        b[288..292].copy_from_slice(&NIS_HIDDEN.to_le_bytes());
        b[808..812].copy_from_slice(&4u32.to_le_bytes());
        b[944] = 0x73;
        let d = parse_notify(&b).expect("parsed");
        assert_eq!((d.message, d.hwnd, d.uid, d.callback, d.icon), (NIM_ADD, -16, 7, 0x8001, 0xABCD));
        assert_eq!((d.tip.as_str(), d.state, d.version, d.guid[0]), ("Volume", NIS_HIDDEN, 4, 0x73));
        // A short (old) structure: the missing fields are zero.
        let short = &b[..8 + 88];
        let d = parse_notify(short).expect("parsed");
        assert_eq!((d.uid, d.state, d.version), (7, 0, 0));
        assert!(parse_notify(&b[..20]).is_none());
    }

    #[test]
    fn reads_icon_rectangle_questions() {
        let mut b = vec![0u8; 40];
        b[0..4].copy_from_slice(&SIGNATURE.to_le_bytes());
        b[4..8].copy_from_slice(&2u32.to_le_bytes());
        b[16..20].copy_from_slice(&0x1234u32.to_le_bytes());
        b[20..24].copy_from_slice(&5u32.to_le_bytes());
        let q = parse_rect_question(&b).expect("parsed");
        assert_eq!((q.corner, q.hwnd, q.uid), (2, 0x1234, 5));
    }

    fn picture(_: isize) -> Option<Icon> {
        Some(Icon { width: 1, height: 1, pixels: vec![0xFF00_0000] })
    }

    #[test]
    fn icons_are_added_changed_hidden_and_removed() {
        let mut icons = Vec::new();
        let add = |hwnd, uid, tip| parse_notify(&message(NIM_ADD, hwnd, uid, NIF_MESSAGE | NIF_ICON | NIF_TIP, tip)).unwrap();
        assert_eq!(apply(&mut icons, &add(1, 1, "One"), picture), Applied { ok: true, changed: true, added: true });
        assert_eq!(apply(&mut icons, &add(2, 1, "Two"), picture), Applied { ok: true, changed: true, added: true });
        // New icons join the right end.
        assert_eq!(icons.iter().map(|i| i.tip.as_str()).collect::<Vec<_>>(), ["One", "Two"]);
        // A new tooltip alone changes nothing shown.
        let tip = parse_notify(&message(NIM_MODIFY, 1, 1, NIF_TIP, "Uno")).unwrap();
        assert_eq!(apply(&mut icons, &tip, picture), Applied { ok: true, changed: false, added: false });
        assert_eq!(icons[0].tip, "Uno");
        // Hidden through its state.
        let mut hide = message(NIM_MODIFY, 2, 1, NIF_STATE, "");
        hide[288..292].copy_from_slice(&NIS_HIDDEN.to_le_bytes());
        hide[292..296].copy_from_slice(&NIS_HIDDEN.to_le_bytes());
        assert!(apply(&mut icons, &parse_notify(&hide).unwrap(), picture).changed);
        assert!(icons[1].hidden);
        // A change to an unknown icon shows it but answers with an error.
        let unknown = parse_notify(&message(NIM_MODIFY, 3, 9, NIF_ICON, "")).unwrap();
        assert_eq!(apply(&mut icons, &unknown, picture), Applied { ok: false, changed: true, added: true });
        assert_eq!(icons.len(), 3);
        let delete = parse_notify(&message(NIM_DELETE, 1, 1, 0, "")).unwrap();
        assert_eq!(apply(&mut icons, &delete, picture), Applied { ok: true, changed: true, added: false });
        assert_eq!(apply(&mut icons, &delete, picture), Applied { ok: false, changed: false, added: false });
        // No window, no new icon.
        let orphan = parse_notify(&message(NIM_ADD, 0, 1, NIF_ICON, "")).unwrap();
        assert!(!apply(&mut icons, &orphan, picture).ok);
    }

    #[test]
    fn version_4_icons_show_the_plain_tooltip_only_when_asked() {
        let mut icons = Vec::new();
        let add = parse_notify(&message(NIM_ADD, 1, 1, NIF_MESSAGE | NIF_TIP, "Tip")).unwrap();
        apply(&mut icons, &add, picture);
        assert_eq!(icons[0].tooltip(), Some("Tip"));
        let mut v4 = message(NIM_SETVERSION, 1, 1, 0, "");
        v4[808..812].copy_from_slice(&4u32.to_le_bytes());
        assert!(apply(&mut icons, &parse_notify(&v4).unwrap(), picture).ok);
        assert_eq!(icons[0].tooltip(), None);
        let show = parse_notify(&message(NIM_MODIFY, 1, 1, NIF_TIP | NIF_SHOWTIP, "Tip")).unwrap();
        apply(&mut icons, &show, picture);
        assert_eq!(icons[0].tooltip(), Some("Tip"));
    }

    #[test]
    fn light_symbols_are_drawn_dark_and_colored_icons_stay() {
        // A white symbol with a soft gray edge on a transparent background: inverted, alpha kept.
        let white = Icon { width: 3, height: 1, pixels: vec![0x0000_0000, 0xFFFF_FFFF, 0x80C0_C0C0] };
        assert_eq!(darken_if_light(white).pixels, [0x00FF_FFFF, 0xFF00_0000, 0x803F_3F3F]);
        // Any colored pixel keeps the icon as it is; so does an already dark symbol.
        let colored = Icon { width: 2, height: 1, pixels: vec![0xFFFF_FFFF, 0xFFFF_0000] };
        assert_eq!(darken_if_light(colored.clone()), colored);
        let dark = Icon { width: 1, height: 1, pixels: vec![0xFF20_2020] };
        assert_eq!(darken_if_light(dark.clone()), dark);
    }

    #[test]
    fn guids_are_written_the_usual_way() {
        // Windows' volume icon.
        let g = [0x73, 0xae, 0x20, 0x78, 0xe3, 0x23, 0x29, 0x42, 0x82, 0xc1, 0xe4, 0x1c, 0xb6, 0x7d, 0x5b, 0x9c];
        assert_eq!(guid_text(&g), "7820ae73-23e3-4229-82c1-e41cb67d5b9c");
    }
}
