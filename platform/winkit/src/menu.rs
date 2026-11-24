//! A program's menus as windows, painted by `bevel_core` as Windows' own menus were: the
//! drop-down menus of a menu bar and the window menu (the title bar's icon, Alt+Space).
//!
//! `track` shows one menu and returns what happened, like Windows' TrackPopupMenu: it runs
//! Windows' messages itself until an entry is chosen or the menu is given up. The menu takes
//! the keyboard focus while it is open (arrows, Enter, Esc, access keys); its owner goes on
//! looking like the window in front (`kit` asks `is_ours`). A click anywhere else closes it and
//! then does what it would have done, since the menu simply stops being the window in front.
//! While it is open, the pointer coming onto another title of the owner's menu bar ends it
//! with `Outcome::Title`, so the bar can show that title's menu instead.
//!
//! (The bar's own menus, with their cascades, are `popup.rs` in the bar's program.)

use std::cell::RefCell;
use std::ptr::{null, null_mut};

use bevel_core::fb::{Framebuffer, Rect};
use bevel_core::kit::Form;
use bevel_core::paint::{self, EntryKind, Menu, MenuLayout, MenuStyle};
use bevel_core::scale::Scale;
use bevel_core::text::split_access_key;
use bevel_core::theme::Era;
use bevel_core::window_menu::{self, WindowCommand, WindowState};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, GetMonitorInfoW, InvalidateRect, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
    PAINTSTRUCT, UpdateWindow,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetDoubleClickTime, ReleaseCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent, VK_DOWN, VK_END, VK_ESCAPE,
    VK_F10, VK_HOME, VK_LEFT, VK_MENU, VK_RETURN, VK_RIGHT, VK_UP,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GWL_STYLE, GetCursorPos, GetMessageTime,
    GetMessageW, GetWindowLongW, GetWindowRect, IDC_ARROW, IsIconic, IsWindow, IsZoomed, LoadCursorW, MSG,
    KillTimer, PostMessageW, PostQuitMessage, RegisterClassExW, SetTimer, WM_TIMER, SC_CLOSE, SC_MAXIMIZE, SC_MINIMIZE, SC_MOVE, SC_RESTORE, SC_SIZE,
    SW_SHOW, ShowWindow, TranslateMessage, WA_INACTIVE, WM_ACTIVATE, WM_CHAR, WM_ERASEBKGND, WM_KEYDOWN,
    WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NULL, WM_PAINT, WM_QUIT, WM_RBUTTONUP,
    WM_SYSCOMMAND, WM_SYSKEYDOWN, WNDCLASSEXW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_MAXIMIZEBOX, WS_MINIMIZEBOX,
    WS_EX_NOACTIVATE, WS_POPUP, WS_THICKFRAME, SW_SHOWNOACTIVATE,
};

use crate::kit::{self, Hosted, Look};
use crate::log::log_win32;
use crate::util::{blit, point, rect_h, rect_w, take_focus, wide};

pub const MENU_CLASS: &str = "Bevel95ProgramMenu";
/// The timer that opens a submenu the pointer rests on, and Windows' menu delay (SPI_GETMENUSHOWDELAY's
/// default, 400 ms).
const SUBMENU_TIMER: usize = 7;
const SUBMENU_DELAY: u32 = 400;
/// Windows' message for the pointer leaving a window (WM_MOUSELEAVE; in windows-sys it sits
/// with the common controls, which this crate does not otherwise need).
const WM_MOUSELEAVE: u32 = 0x02A3;

/// How a menu ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// This entry was chosen (never a separator or a grayed-out one).
    Chosen(usize),
    /// Given up by a click elsewhere or another window coming to the front.
    Cancelled,
    /// Esc: back to the menu bar, its title still selected.
    Escaped,
    /// Alt or F10: out of the menus altogether.
    Left,
    /// The Left and Right arrows: on to the menu before or after it.
    Previous,
    Next,
    /// The pointer came onto this other title of the owner's menu bar.
    Title(usize),
}

/// One menu to show.
pub struct Request<'a> {
    /// The window it belongs to (it goes on looking like the window in front).
    pub owner: HWND,
    pub menu: Menu,
    pub style: MenuStyle,
    /// Where its top-left corner goes, in screen pixels (moved to stay on the screen).
    pub at: (i32, i32),
    /// The row its bottom edge sits on when it does not fit below `at` (the top of what it
    /// dropped from), if it may open upward.
    pub above: Option<i32>,
    /// The menu bar's other titles on the screen: the pointer on one of them ends the menu.
    pub titles: &'a [(usize, RECT)],
}

struct Tracking {
    hwnd: HWND,
    owner: HWND,
    menu: Menu,
    style: MenuStyle,
    layout: MenuLayout,
    look: Look,
    outcome: Option<Outcome>,
    /// WM_MOUSELEAVE is asked for.
    leave_armed: bool,
}

thread_local! {
    static TRACKING: RefCell<Option<Tracking>> = const { RefCell::new(None) };
}

/// Whether `hwnd` is the menu now open for `owner` (which then still looks in front).
pub fn is_ours(hwnd: HWND, owner: HWND) -> bool {
    !hwnd.is_null() && TRACKING.with(|t| t.borrow().as_ref().is_some_and(|t| t.hwnd == hwnd && t.owner == owner))
}

/// Whether a menu is open now.
pub fn is_open() -> bool {
    TRACKING.with(|t| t.borrow().is_some())
}

fn ensure_class() {
    static DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if DONE.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let name = wide(MENU_CLASS);
    let class = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(menu_proc),
        // SAFETY: our own module; a system cursor.
        hInstance: unsafe { GetModuleHandleW(null()) },
        hCursor: unsafe { LoadCursorW(null_mut(), IDC_ARROW) },
        lpszClassName: name.as_ptr(),
        ..Default::default()
    };
    // SAFETY: a fully initialized class description (the name is copied by Windows).
    if unsafe { RegisterClassExW(&class) } == 0 {
        log_win32("RegisterClassExW(program menu)");
    }
}

/// The screen (in pixels) of the monitor `window` is on.
fn screen_of(window: HWND) -> RECT {
    let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
    // SAFETY: valid out-pointer; any window handle.
    unsafe { GetMonitorInfoW(MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST), &mut info) };
    info.rcMonitor
}

/// How a menu ended, and the message that was in hand when it was given up (the click on
/// another part of the program's window, say): the caller decides what becomes of it.
pub struct Ended {
    pub outcome: Outcome,
    pub pending: Option<MSG>,
    /// The command was chosen by key, so its dialog should show keyboard cues.
    pub keyboard: bool,
    /// Where the menu was on the screen, and its layout (for a submenu beside an entry).
    pub rect: RECT,
    pub layout: Option<MenuLayout>,
}

/// Shows `request.menu` and runs Windows' messages until it ends.
pub fn track(look: &Look, request: Request) -> Ended {
    track_with_selection(look, request, |_| {})
}

/// Reports the initial highlighted row and each change while the popup is open, so
/// the program can describe the current command in its status bar.
pub fn track_with_selection(look: &Look, request: Request, mut selected: impl FnMut(Option<usize>)) -> Ended {
    let nothing = Ended { outcome: Outcome::Cancelled, pending: None, keyboard: false, rect: RECT::default(), layout: None };
    if is_open() {
        return nothing; // one at a time
    }
    ensure_class();
    let scale = look.scale;
    let screen = screen_of(request.owner);
    let layout = paint::layout_menu(look.theme, &look.fonts(), &request.menu, request.style, scale.old_fit(rect_h(&screen)));
    let (w, h) = (scale.len(layout.width), scale.len(layout.height));
    let (mut x, mut y) = request.at;
    if y + h > screen.bottom
        && let Some(above) = request.above
        && above - h >= screen.top
    {
        y = above - h;
    }
    x = x.min(screen.right - w).max(screen.left);
    y = y.min(screen.bottom - h).max(screen.top);
    let class = wide(MENU_CLASS);
    // SAFETY: a registered class; owned by the program's window, so it stays above it.
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            class.as_ptr(),
            null(),
            WS_POPUP,
            x,
            y,
            w,
            h,
            request.owner,
            null_mut(),
            GetModuleHandleW(null()),
            null(),
        )
    };
    if hwnd.is_null() {
        log_win32("CreateWindowExW(program menu)");
        return nothing;
    }
    let placed = RECT { left: x, top: y, right: x + w, bottom: y + h };
    let kept_layout = layout.clone();
    TRACKING.with(|t| {
        *t.borrow_mut() = Some(Tracking {
            hwnd,
            owner: request.owner,
            menu: request.menu,
            style: request.style,
            layout,
            look: look.clone(),
            outcome: None,
            leave_armed: false,
        });
    });
    // SAFETY: our own window.
    unsafe { ShowWindow(hwnd, SW_SHOW) };
    take_focus(hwnd);
    // A parent taken up again on an entry the pointer rests on: its submenu opens after the
    // menu delay, as if the pointer had just come onto it.
    if ARM_SUBMENU.with(|a| a.replace(false)) {
        // SAFETY: our own window's timer.
        unsafe { SetTimer(hwnd, SUBMENU_TIMER, SUBMENU_DELAY, None) };
    }

    let current = || TRACKING.with(|t| t.borrow().as_ref().and_then(|t| t.outcome));
    // Where the pointer was: Windows sends a window a WM_MOUSEMOVE when a window appears or goes
    // under a pointer that did not move, and that must not count as the pointer coming onto a
    // title (a click on File, then Right without touching the mouse, bounced back to File).
    let mut pointer = POINT::default();
    // SAFETY: a valid out-pointer.
    unsafe { GetCursorPos(&mut pointer) };
    let mut msg = MSG::default();
    let mut pending = None;
    let mut last_selection = None;
    let outcome = loop {
        let selection = tracking(hwnd, |t| t.menu.hover);
        if selection != last_selection {
            if let Some(row) = selection {
                selected(row);
            }
            last_selection = selection;
        }
        if let Some(outcome) = current() {
            break outcome;
        }
        // SAFETY: the standard message loop; a WM_QUIT is handed on to the program's own loop.
        unsafe {
            if IsWindow(hwnd) == 0 {
                break Outcome::Cancelled;
            }
            if GetMessageW(&mut msg, null_mut(), 0, 0) <= 0 {
                if msg.message == WM_QUIT {
                    PostQuitMessage(msg.wParam as i32);
                }
                break Outcome::Cancelled;
            }
            // Given up while waiting (another window came to the front): the message in hand,
            // usually the click that did it, is the caller's to deal with once the menu is gone.
            if let Some(outcome) = current() {
                if msg.message != WM_NULL {
                    pending = Some(msg);
                }
                break outcome;
            }
            // A submenu's parent: the pointer resting on another of its entries.
            if let Some(k) = parent_switch(&msg, hwnd) {
                break Outcome::Title(k);
            }
            // The pointer on another title of the owner's menu bar: that title's menu instead.
            if msg.message == WM_MOUSEMOVE && msg.hwnd == request.owner {
                let mut at = POINT::default();
                GetCursorPos(&mut at);
                let moved = (at.x, at.y) != (pointer.x, pointer.y);
                pointer = at;
                let inside = |r: &RECT| at.x >= r.left && at.x < r.right && at.y >= r.top && at.y < r.bottom;
                if moved && let Some(&(title, _)) = request.titles.iter().find(|(_, r)| inside(r)) {
                    break Outcome::Title(title);
                }
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    };
    TRACKING.with(|t| t.borrow_mut().take());
    // SAFETY: our own window (if it still exists).
    unsafe {
        if IsWindow(hwnd) != 0 {
            DestroyWindow(hwnd);
        }
    }
    let keyboard = matches!(outcome, Outcome::Chosen(_)) && matches!(msg.message, WM_KEYDOWN | WM_CHAR);
    Ended { outcome, pending, keyboard, rect: placed, layout: Some(kept_layout) }
}

/// Hands a message that ended a menu on to its window (the click does what it would have done).
pub fn dispatch(msg: &MSG) {
    // SAFETY: a message taken from this thread's queue.
    unsafe {
        TranslateMessage(msg);
        DispatchMessageW(msg);
    }
}

/// Runs `f` on the open menu, if `hwnd` is it.
fn tracking<R>(hwnd: HWND, f: impl FnOnce(&mut Tracking) -> R) -> Option<R> {
    TRACKING.with(|t| t.borrow_mut().as_mut().filter(|t| t.hwnd == hwnd).map(f))
}

fn repaint(hwnd: HWND) {
    // SAFETY: our own window.
    unsafe { InvalidateRect(hwnd, null(), 0) };
}

/// The first (or last) entry that can be highlighted.
fn end_entry(menu: &Menu, style: MenuStyle, first: bool) -> Option<usize> {
    let mut all = 0..menu.entries.len();
    if first { all.find(|&i| menu.selectable(i, style)) } else { all.rev().find(|&i| menu.selectable(i, style)) }
}

/// A key on the open menu. True if it was the menu's.
fn key(t: &mut Tracking, vk: u16) -> bool {
    let n = t.menu.entries.len();
    match vk {
        VK_ESCAPE => t.outcome = Some(Outcome::Escaped),
        VK_LEFT => t.outcome = Some(Outcome::Previous),
        // On an entry with a submenu, Right opens it (the caller shows it: `track_cascade`).
        VK_RIGHT => {
            t.outcome = Some(match t.menu.hover.filter(|&i| t.menu.entries[i].kind == EntryKind::Submenu && t.menu.entries[i].choosable()) {
                Some(i) => Outcome::Chosen(i),
                None => Outcome::Next,
            })
        }
        VK_RETURN => {
            if let Some(i) = t.menu.hover.filter(|&i| t.menu.entries[i].choosable()) {
                t.outcome = Some(Outcome::Chosen(i));
            }
        }
        VK_UP | VK_DOWN => {
            let style = t.style;
            t.menu.hover = match t.menu.hover {
                None => end_entry(&t.menu, style, vk == VK_DOWN),
                Some(current) => {
                    let step = if vk == VK_DOWN { 1 } else { n - 1 };
                    (1..=n).map(|k| (current + k * step) % n).find(|&i| t.menu.selectable(i, style))
                }
            };
        }
        VK_HOME | VK_END => t.menu.hover = end_entry(&t.menu, t.style, vk == VK_HOME),
        _ => return false,
    }
    t.menu.show_keys = true;
    true
}

/// A typed character: the entries whose access key it is. One: it is chosen; several: the
/// highlight moves on to the next of them (as Windows' menus do).
fn access_key(t: &mut Tracking, ch: char) {
    let lower = |c: char| c.to_lowercase().next().unwrap_or(c);
    let key_of = |label: &str| {
        let (text, key) = split_access_key(label);
        key.and_then(|k| text.chars().nth(k)).map(lower)
    };
    let matches: Vec<usize> = (0..t.menu.entries.len())
        .filter(|&i| t.menu.entries[i].choosable() && key_of(t.menu.entries[i].parts().0) == Some(lower(ch)))
        .collect();
    t.menu.show_keys = true;
    match matches.as_slice() {
        [] => {}
        [only] => {
            t.menu.hover = Some(*only);
            t.outcome = Some(Outcome::Chosen(*only));
        }
        several => {
            let next = several.iter().find(|&&i| t.menu.hover.is_some_and(|h| i > h)).unwrap_or(&several[0]);
            t.menu.hover = Some(*next);
        }
    }
}

unsafe extern "system" fn menu_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_ERASEBKGND => return 1,
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            // SAFETY: paired BeginPaint/EndPaint on our own window.
            let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
            tracking(hwnd, |t| {
                let mut fb = Framebuffer::new(t.layout.width, t.layout.height, 0);
                paint::paint_menu(&mut fb, t.look.theme, &t.look.fonts(), &t.menu, t.style, &t.layout);
                let big = t.look.scale.enlarge(&fb);
                blit(hdc, &big, big.width);
            });
            // SAFETY: see above.
            unsafe { EndPaint(hwnd, &ps) };
            return 0;
        }
        WM_MOUSEMOVE => {
            let (x, y) = point(lparam);
            let arm = tracking(hwnd, |t| {
                let s = t.look.scale;
                let entry = t.layout.hit(&t.menu, t.style, s.old_in(x, t.layout.width), s.old_in(y, t.layout.height));
                let changed = entry != t.menu.hover;
                t.menu.hover = entry;
                (changed, !std::mem::replace(&mut t.leave_armed, true))
            });
            // Resting on an entry with a submenu opens it after Windows' menu delay.
            let on_submenu = tracking(hwnd, |t| t.menu.hover.is_some_and(|i| t.menu.entries[i].kind == EntryKind::Submenu));
            if arm.is_some_and(|(changed, _)| changed) {
                // SAFETY: our own window's timer.
                unsafe {
                    if on_submenu == Some(true) {
                        SetTimer(hwnd, SUBMENU_TIMER, SUBMENU_DELAY, None);
                    } else {
                        KillTimer(hwnd, SUBMENU_TIMER);
                    }
                }
            }
            if let Some((changed, arm)) = arm {
                if arm {
                    let mut track = TRACKMOUSEEVENT {
                        cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    // SAFETY: a correctly sized struct for our own window.
                    unsafe { TrackMouseEvent(&mut track) };
                }
                if changed {
                    repaint(hwnd);
                }
            }
            return 0;
        }
        WM_MOUSELEAVE => {
            let changed = tracking(hwnd, |t| {
                t.leave_armed = false;
                t.menu.hover.take().is_some()
            });
            if changed == Some(true) {
                repaint(hwnd);
            }
            return 0;
        }
        WM_LBUTTONUP | WM_RBUTTONUP => {
            let (x, y) = point(lparam);
            tracking(hwnd, |t| {
                let s = t.look.scale;
                if let Some(i) = t.layout.hit(&t.menu, t.style, s.old_in(x, t.layout.width), s.old_in(y, t.layout.height))
                    && t.menu.entries[i].choosable()
                {
                    t.outcome = Some(Outcome::Chosen(i));
                }
            });
            return 0;
        }
        WM_KEYDOWN => {
            if tracking(hwnd, |t| key(t, wparam as u16)) == Some(true) {
                repaint(hwnd);
            }
            return 0;
        }
        WM_CHAR => {
            if let Some(ch) = char::from_u32(wparam as u32).filter(|c| !c.is_control()) {
                tracking(hwnd, |t| access_key(t, ch));
                repaint(hwnd);
            }
            return 0;
        }
        WM_TIMER if wparam == SUBMENU_TIMER => {
            // SAFETY: our own window's timer.
            unsafe { KillTimer(hwnd, SUBMENU_TIMER) };
            tracking(hwnd, |t| {
                if let Some(i) = t.menu.hover.filter(|&i| t.menu.entries[i].kind == EntryKind::Submenu && t.menu.entries[i].choosable()) {
                    t.outcome = Some(Outcome::Chosen(i));
                }
            });
            // SAFETY: posting to our own window wakes the loop in `track`.
            unsafe { PostMessageW(hwnd, WM_NULL, 0, 0) };
            return 0;
        }
        // Alt or F10 leaves the menus.
        WM_SYSKEYDOWN if matches!(wparam as u16, VK_MENU | VK_F10) => {
            tracking(hwnd, |t| t.outcome = Some(Outcome::Left));
            return 0;
        }
        // Another window came to the front (a click elsewhere): the menu is given up. This
        // message is sent, not posted, so the loop in `track` is woken with an empty one.
        WM_ACTIVATE if (wparam & 0xFFFF) as u32 == WA_INACTIVE => {
            tracking(hwnd, |t| {
                if t.outcome.is_none() {
                    t.outcome = Some(Outcome::Cancelled);
                }
            });
            // SAFETY: posting to our own window.
            unsafe { PostMessageW(hwnd, WM_NULL, 0, 0) };
            return 0;
        }
        _ => {}
    }
    // SAFETY: default handling for everything else.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// The width of the screen `window` is on, in pixels (for placing a program's windows).
pub fn screen_size(window: HWND) -> (i32, i32) {
    let screen = screen_of(window);
    (rect_w(&screen), rect_h(&screen))
}
/// The menu bar's interface to the program's form. Coordinates are in unscaled window pixels.
pub struct MenuBar<F> {
    pub count: usize,
    pub title_rect: fn(&F, usize) -> Option<Rect>,
    pub menu: fn(&F, usize) -> (Era, Menu),
    pub show_menu: fn(&mut F, Option<usize>),
    pub back_to_bar: fn(&mut F, usize),
    pub reached_by_keyboard: fn(&mut F, bool),
    /// `None` for programs without a menu status line. When the menus close, `show_menu`
    /// or `back_to_bar` restores the program's own status.
    pub selection: Option<fn(&mut F, usize, Option<usize>)>,
}

pub struct Selection {
    pub era: Era,
    pub title: usize,
    pub row: usize,
    pub keyboard: bool,
}

fn screen_rect(window: &RECT, rect: Rect, scale: Scale) -> RECT {
    RECT {
        left: window.left + scale.at(rect.x),
        top: window.top + scale.at(rect.y),
        right: window.left + scale.at(rect.right()),
        bottom: window.top + scale.at(rect.bottom()),
    }
}

/// Tracks a dropdown and any neighboring dropdowns reached by keyboard or mouse.
pub fn track_bar<H: Hosted>(first: usize, bar: MenuBar<H::F>) -> Option<Selection> {
    let hwnd = kit::hwnd::<H>()?;
    let look = kit::look_of::<H>()?;
    let scale = look.scale;
    let mut title = first;
    loop {
        let (era, menu, rects) = kit::with::<H, _>(|host| {
            let form = host.form_mut();
            (bar.show_menu)(form, Some(title));
            let (era, menu) = (bar.menu)(form, title);
            let rects: Vec<_> = (0..bar.count).filter_map(|i| (bar.title_rect)(form, i).map(|r| (i, r))).collect();
            (era, menu, rects)
        })?;
        kit::update::<H>(|_| {});
        let mut window = RECT::default();
        // SAFETY: our hosted window and a valid out-pointer.
        unsafe {
            UpdateWindow(hwnd);
            GetWindowRect(hwnd, &mut window)
        };
        let Some((_, anchor)) = rects.iter().find(|&&(i, _)| i == title) else { break };
        let others: Vec<_> =
            rects.iter().filter(|&&(i, _)| i != title).map(|&(i, r)| (i, screen_rect(&window, r, scale))).collect();
        let place = screen_rect(&window, *anchor, scale);
        let request = Request {
            owner: hwnd,
            menu,
            style: MenuStyle::Popup,
            at: (place.left, place.bottom),
            above: Some(place.top),
            titles: &others,
        };
        let ended = track_with_selection(&look, request, |row| {
            if let Some(selection) = bar.selection {
                kit::update::<H>(|host| selection(host.form_mut(), title, row));
            }
        });
        match ended.outcome {
            Outcome::Chosen(row) => {
                kit::update::<H>(|host| (bar.show_menu)(host.form_mut(), None));
                return Some(Selection { era, title, row, keyboard: ended.keyboard });
            }
            Outcome::Title(other) => {
                kit::with::<H, _>(|host| (bar.reached_by_keyboard)(host.form_mut(), false));
                title = other;
            }
            Outcome::Previous | Outcome::Next => {
                kit::with::<H, _>(|host| (bar.reached_by_keyboard)(host.form_mut(), true));
                title = if ended.outcome == Outcome::Next {
                    (title + 1) % bar.count
                } else {
                    (title + bar.count - 1) % bar.count
                };
            }
            Outcome::Escaped => {
                kit::update::<H>(|host| (bar.back_to_bar)(host.form_mut(), title));
                return None;
            }
            Outcome::Left => break,
            Outcome::Cancelled => {
                if let Some(msg) = ended.pending {
                    // Clicking the current title closes its menu. A click on another title
                    // opens that menu; any other click goes to its original destination.
                    let clicked = if msg.hwnd == hwnd && matches!(msg.message, WM_LBUTTONDOWN | WM_LBUTTONDBLCLK) {
                        let (x, y) = point(msg.lParam);
                        kit::with::<H, _>(|host| {
                            (0..bar.count).find(|&i| {
                                (bar.title_rect)(host.form(), i)
                                    .is_some_and(|r| r.contains(scale.old(x), scale.old(y)))
                            })
                        })
                        .flatten()
                    } else {
                        None
                    };
                    match clicked {
                        Some(other) if other != title => {
                            kit::with::<H, _>(|host| (bar.reached_by_keyboard)(host.form_mut(), false));
                            title = other;
                            continue;
                        }
                        None => {
                            kit::update::<H>(|host| (bar.show_menu)(host.form_mut(), None));
                            dispatch(&msg);
                            return None;
                        }
                        _ => {}
                    }
                }
                break;
            }
        }
    }
    kit::update::<H>(|host| (bar.show_menu)(host.form_mut(), None));
    None
}

/// Opens a program's caption menu from its icon, a title-bar right-click or Alt+Space.
pub fn track_window<H: Hosted>(icon: <H::F as Form>::Part, era: fn(&H::F) -> Era, at_pointer: bool, keyboard: bool) {
    let Some(hwnd) = kit::hwnd::<H>() else { return };
    let Some(look) = kit::look_of::<H>() else { return };
    let Some((era, Some(icon_rect))) = kit::with::<H, _>(|host| (era(host.form()), host.form().rect(icon))) else {
        return;
    };
    let scale = look.scale;
    let mut window = RECT::default();
    let mut pointer = POINT::default();
    // SAFETY: querying the hosted window and releasing any existing mouse capture.
    let (style, minimized, maximized, opened) = unsafe {
        ReleaseCapture();
        if GetWindowRect(hwnd, &mut window) == 0 {
            return;
        };
        GetCursorPos(&mut pointer);
        (GetWindowLongW(hwnd, GWL_STYLE) as u32, IsIconic(hwnd) != 0, IsZoomed(hwnd) != 0, GetMessageTime() as u32)
    };
    let state = WindowState {
        minimized,
        maximized,
        resizable: style & WS_THICKFRAME != 0,
        has_minimize_box: style & WS_MINIMIZEBOX != 0,
        has_maximize_box: style & WS_MAXIMIZEBOX != 0,
        closable: true,
    };
    let mut menu = window_menu::program_menu(era, state);
    menu.show_keys = era != Era::Win2000 || keyboard;
    menu.hover = keyboard.then_some(0);
    let anchor = if at_pointer {
        (pointer.x, pointer.y)
    } else {
        // A program icon starts two pixels after the caption's left end and ends
        // one pixel before its bottom, both for fixed and for resizable frames.
        (window.left + scale.at(icon_rect.x - 2), window.top + scale.at(icon_rect.bottom() + 1))
    };
    let ended =
        track(&look, Request { owner: hwnd, menu, style: MenuStyle::Popup, at: anchor, above: None, titles: &[] });
    let command = match ended.outcome {
        Outcome::Chosen(i) => window_menu::command_at(i),
        _ => ended.pending.and_then(|msg| {
            let (x, y) = point(msg.lParam);
            let on_icon = !at_pointer
                && msg.hwnd == hwnd
                && matches!(msg.message, WM_LBUTTONDOWN | WM_LBUTTONDBLCLK)
                && kit::with::<H, _>(|host| {
                    host.form().rect(icon).is_some_and(|r| r.contains(scale.old(x), scale.old(y)))
                }) == Some(true);
            if on_icon {
                // SAFETY: the system double-click interval is a process-independent query.
                (msg.time.wrapping_sub(opened) <= unsafe { GetDoubleClickTime() }).then_some(WindowCommand::Close)
            } else {
                dispatch(&msg);
                None
            }
        }),
    };
    if let Some(command) = command {
        let code = match command {
            WindowCommand::Restore => SC_RESTORE,
            WindowCommand::Move => SC_MOVE,
            WindowCommand::Size => SC_SIZE,
            WindowCommand::Minimize => SC_MINIMIZE,
            WindowCommand::Maximize => SC_MAXIMIZE,
            WindowCommand::Close => SC_CLOSE,
        };
        // SAFETY: standard system command posted to the game that opened the menu.
        unsafe { PostMessageW(hwnd, WM_SYSCOMMAND, code as usize, 0) };
    }
}

/// The class of a menu shown as a still picture (a parent menu while its submenu is open).
const STILL_CLASS: &str = "Bevel95MenuPicture";

thread_local! {
    /// The pictures of the still menus, by window.
    static STILLS: RefCell<Vec<(HWND, Framebuffer)>> = const { RefCell::new(Vec::new()) };
}

unsafe extern "system" fn still_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_ERASEBKGND => return 1,
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            // SAFETY: paired BeginPaint/EndPaint on our own window.
            let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
            STILLS.with(|s| {
                if let Some((_, fb)) = s.borrow().iter().find(|(h, _)| *h == hwnd) {
                    blit(hdc, fb, fb.width);
                }
            });
            // SAFETY: see above.
            unsafe { EndPaint(hwnd, &ps) };
            return 0;
        }
        _ => {}
    }
    // SAFETY: default handling for everything else.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// Shows `menu` as a picture at `rect` (screen pixels), above `owner`, taking no input. Its
/// window is destroyed with `drop_still`.
fn show_still(look: &Look, owner: HWND, menu: &Menu, style: MenuStyle, rect: RECT) -> HWND {
    static DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !DONE.swap(true, std::sync::atomic::Ordering::SeqCst) {
        let name = wide(STILL_CLASS);
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(still_proc),
            // SAFETY: our own module.
            hInstance: unsafe { GetModuleHandleW(null()) },
            hCursor: unsafe { LoadCursorW(null_mut(), IDC_ARROW) },
            lpszClassName: name.as_ptr(),
            ..Default::default()
        };
        // SAFETY: a fully initialized class description.
        if unsafe { RegisterClassExW(&class) } == 0 {
            log_win32("RegisterClassExW(menu picture)");
        }
    }
    let layout = paint::layout_menu(look.theme, &look.fonts(), menu, style, look.scale.old_fit(rect_h(&rect)) + 1);
    let mut fb = Framebuffer::new(layout.width, layout.height, 0);
    paint::paint_menu(&mut fb, look.theme, &look.fonts(), menu, style, &layout);
    let big = look.scale.enlarge(&fb);
    let class = wide(STILL_CLASS);
    // SAFETY: a registered class; owned by the program's window, so it stays above it.
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class.as_ptr(),
            null(),
            WS_POPUP,
            rect.left,
            rect.top,
            rect_w(&rect),
            rect_h(&rect),
            owner,
            null_mut(),
            GetModuleHandleW(null()),
            null(),
        )
    };
    if hwnd.is_null() {
        log_win32("CreateWindowExW(menu picture)");
        return hwnd;
    }
    STILLS.with(|s| s.borrow_mut().push((hwnd, big)));
    // SAFETY: our own window, shown without taking the keyboard.
    unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE) };
    // Under the bar's Monitor Lines (`on_top`; brief 102).
    crate::on_top::shown();
    hwnd
}

fn drop_still(hwnd: HWND) {
    STILLS.with(|s| s.borrow_mut().retain(|(h, _)| *h != hwnd));
    if !hwnd.is_null() {
        // SAFETY: our own window.
        unsafe { DestroyWindow(hwnd) };
    }
}

/// While a submenu is open: its parent's picture and entries. The pointer onto another entry of
/// the parent lights it at once; resting there `SWITCH_DELAY`, the submenu closes and the parent
/// goes on from that entry (98's Reference PC: the entry lit at once, the submenu gone about 0.4 s
/// later; back into the submenu before that, nothing changes).
struct Parent {
    still: HWND,
    look: Look,
    menu: Menu,
    style: MenuStyle,
    rect: RECT,
    /// Each selectable entry's place on the screen.
    entries: Vec<(usize, RECT)>,
    open: usize,
    waiting: Option<(usize, std::time::Instant)>,
}

thread_local! {
    static PARENT: RefCell<Option<Parent>> = const { RefCell::new(None) };
    /// The next menu shown starts its submenu timer at once (`track_cascade` going on from
    /// another entry of the parent).
    static ARM_SUBMENU: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

const PARENT_TIMER: usize = 0xB95;
const SWITCH_DELAY: std::time::Duration = std::time::Duration::from_millis(400);

/// The open submenu's parent and the message in hand: the parent's entry to go on from, once
/// the pointer has rested on it long enough.
fn parent_switch(msg: &MSG, menu_hwnd: HWND) -> Option<usize> {
    let mut light = None;
    let switch = PARENT.with(|p| {
        let mut p = p.borrow_mut();
        let p = p.as_mut()?;
        if msg.message == WM_MOUSEMOVE && (msg.hwnd == p.still || msg.hwnd == menu_hwnd) {
            let mut at = POINT::default();
            // SAFETY: a valid out-pointer.
            unsafe { GetCursorPos(&mut at) };
            let inside = |r: &RECT| at.x >= r.left && at.x < r.right && at.y >= r.top && at.y < r.bottom;
            // Where the pointer is, whichever window the move came to (the open menu may hold
            // the mouse); the submenu where the two overlap.
            let mut child = RECT::default();
            // SAFETY: the open menu's own window; a valid out-pointer.
            let in_child = unsafe { GetWindowRect(menu_hwnd, &mut child) } != 0 && inside(&child);
            let on = if in_child { None } else { p.entries.iter().find(|(_, r)| inside(r)).map(|&(k, _)| k) };
            match on {
                Some(k) if k != p.open => {
                    if p.waiting.map(|w| w.0) != Some(k) {
                        p.waiting = Some((k, std::time::Instant::now()));
                        light = Some(k);
                        // SAFETY: a timer of the open menu's window, to wake this loop.
                        unsafe { SetTimer(menu_hwnd, PARENT_TIMER, SWITCH_DELAY.as_millis() as u32 + 10, None) };
                    }
                }
                // Back on the open entry or into the submenu: it stays.
                _ if p.waiting.is_some() && (on == Some(p.open) || in_child) => {
                    p.waiting = None;
                    light = Some(p.open);
                    // SAFETY: the open menu's own timer.
                    unsafe { KillTimer(menu_hwnd, PARENT_TIMER) };
                }
                _ => {}
            }
        }
        if let Some(k) = light {
            let mut shown = p.menu.clone();
            shown.hover = Some(k);
            restill(&p.look, p.still, &shown, p.style, p.rect);
        }
        p.waiting.filter(|(_, since)| since.elapsed() >= SWITCH_DELAY).map(|(k, _)| k)
    });
    if switch.is_some() {
        // SAFETY: the open menu's own timer.
        unsafe { KillTimer(menu_hwnd, PARENT_TIMER) };
    }
    switch
}

/// Draws a still menu picture again (another entry lit).
fn restill(look: &Look, hwnd: HWND, menu: &Menu, style: MenuStyle, rect: RECT) {
    let layout = paint::layout_menu(look.theme, &look.fonts(), menu, style, look.scale.old_fit(rect_h(&rect)) + 1);
    let mut fb = Framebuffer::new(layout.width, layout.height, 0);
    paint::paint_menu(&mut fb, look.theme, &look.fonts(), menu, style, &layout);
    let big = look.scale.enlarge(&fb);
    STILLS.with(|s| {
        if let Some(entry) = s.borrow_mut().iter_mut().find(|(h, _)| *h == hwnd) {
            entry.1 = big;
        }
    });
    repaint(hwnd);
}

/// How a menu with submenus ended: the entry chosen (and, for a submenu, the entry in it), or
/// how it was given up.
pub struct CascadeEnd {
    pub chosen: Option<(usize, Option<usize>)>,
    pub ended: Ended,
}

/// Shows `request.menu` like `track`, and the submenu of an entry with one (`submenu(i)`) beside it
/// when the pointer rests on it, it is clicked, or Right or Enter is pressed on it (the parent stays
/// in sight, highlighted, as Windows showed it). Left or Esc in the submenu goes back to the parent.
/// `selected` hears the highlighted entry: (the parent's entry, the submenu's entry if one is open).
pub fn track_cascade(
    look: &Look,
    mut request: Request,
    submenu: &dyn Fn(usize) -> Option<Menu>,
    mut selected: impl FnMut(usize, Option<usize>),
) -> CascadeEnd {
    let scale = look.scale;
    loop {
        let parent_menu = request.menu.clone();
        let ended = track_with_selection(look, Request { menu: parent_menu.clone(), titles: request.titles, ..request }, |row| {
            if let Some(row) = row {
                selected(row, None);
            }
        });
        let Outcome::Chosen(i) = ended.outcome else { return CascadeEnd { chosen: None, ended } };
        let Some(mut child) = submenu(i) else { return CascadeEnd { chosen: Some((i, None)), ended } };
        let (Some(layout), rect) = (ended.layout.clone(), ended.rect) else { return CascadeEnd { chosen: None, ended } };
        // The parent stays in sight, its entry highlighted.
        let mut shown = parent_menu.clone();
        shown.hover = Some(i);
        let still = show_still(look, request.owner, &shown, request.style, rect);
        let e = layout.rects[i];
        let parent = Rect::new(rect.left, rect.top, rect_w(&rect), rect_h(&rect));
        let r = screen_rect(&rect, e, scale);
        let entry = Rect::new(r.left, r.top, rect_w(&r), rect_h(&r));
        let (rule, frame) = (look.theme.popup.submenu, look.theme.popup.frame);
        let rule = bevel_core::theme::SubmenuPlacement { overlap: scale.len(rule.overlap), margin_bottom: scale.len(rule.margin_bottom), ..rule };
        let screen = screen_of(request.owner);
        let screen = Rect::new(screen.left, screen.top, rect_w(&screen), rect_h(&screen));
        let child_layout = paint::layout_menu(look.theme, &look.fonts(), &child, request.style, scale.old_fit(screen.h));
        let (cx, cy) = bevel_core::layout::place_submenu(
            parent,
            entry,
            scale.len(child_layout.width),
            scale.len(child_layout.height),
            scale.len(frame),
            rule,
            screen,
        );
        child.show_keys = parent_menu.show_keys;
        // Its first entry highlighted, as Windows opened a submenu.
        child.hover = (0..child.entries.len()).find(|&k| child.selectable(k, request.style));
        let entries: Vec<(usize, RECT)> = (0..parent_menu.entries.len())
            .filter(|&k| parent_menu.selectable(k, request.style))
            .filter_map(|k| layout.rects.get(k).map(|e| (k, e)))
            .map(|(k, e)| (k, screen_rect(&rect, *e, scale)))
            .collect();
        PARENT.with(|p| {
            *p.borrow_mut() = Some(Parent {
                still,
                look: look.clone(),
                menu: parent_menu.clone(),
                style: request.style,
                rect,
                entries,
                open: i,
                waiting: None,
            })
        });
        let inner = track_with_selection(look, Request { menu: child, at: (cx, cy), above: None, titles: &[], ..request }, |row| {
            selected(i, row);
        });
        PARENT.with(|p| p.borrow_mut().take());
        drop_still(still);
        match inner.outcome {
            Outcome::Chosen(j) => return CascadeEnd { chosen: Some((i, Some(j))), ended: inner },
            // The pointer rested on another entry of the parent: the parent goes on from there.
            Outcome::Title(k) => {
                request.menu = parent_menu;
                request.menu.hover = Some(k);
                ARM_SUBMENU.with(|a| a.set(request.menu.entries[k].kind == EntryKind::Submenu));
                selected(k, None);
            }
            Outcome::Escaped | Outcome::Previous => {
                request.menu = parent_menu;
                request.menu.hover = Some(i);
            }
            _ => return CascadeEnd { chosen: None, ended: inner },
        }
    }
}