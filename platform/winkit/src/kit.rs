//! The control kit's window host: one Win32 window class for every core-drawn window with
//! controls (the bar's Run, Shut Down, Log Off, Taskbar Properties, Date/Time Properties and
//! Bevel95 Setup; a program's main window and its dialogs). The window describes itself as a
//! `bevel_core::kit::Form`; the shared `Controller` turns the mouse and keys into changes; this
//! module owns the Win32 side: creating, painting and closing the window, mouse capture while a
//! button is held, moving the window by its title bar, the blinking caret, and turning Windows'
//! messages into the kit's keys.
//!
//! Each window keeps its own state here (`Hosted`): its form, what it paints with (`Look`: the
//! theme, the fonts and the size, handed over when it opens, so painting never waits for anyone)
//! and anything else it needs. One window of each kind at a time; `with`, `update` and `close`
//! find it by kind. While a window handles a message its state is lifted out of the list, so
//! Windows calling us again meanwhile (a modal loop, a nested message) finds nothing and gets
//! Windows' default; the Win32 calls that can do that (capture, moving the window, carrying out
//! a button) are made after the state is back.
//!
//! Three kinds of window (`Kind`): the bar's own (above every window, no taskbar button), a
//! program's main window (a taskbar button, can be minimized, shows whether it is in front),
//! and a program's dialog (above its owner, which cannot be used meanwhile).

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicU32, Ordering};

use bevel_core::fb::{Framebuffer, Icon};
use bevel_core::kit::{Button, Controller, Effect, Form, Key, Mods};
use bevel_core::scale::upscale;
use bevel_core::text::{Font, Fonts};
use bevel_core::theme::Theme;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CombineRgn, CreateBitmap, CreateRectRgn, DeleteObject, EndPaint, GetMonitorInfoW, InvalidateRect,
    MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST, PAINTSTRUCT, RGN_OR, SetWindowRgn,
};
use windows_sys::Win32::System::DataExchange::{CloseClipboard, GetClipboardData, OpenClipboard};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Memory::{GlobalLock, GlobalUnlock};
use windows_sys::Win32::System::Ole::CF_UNICODETEXT;
use windows_sys::Win32::System::Threading::GetCurrentProcessId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetKeyState, ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent, VK_CONTROL, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_F4,
    VK_HOME, VK_LEFT, VK_MENU, VK_NEXT, VK_PRIOR, VK_RETURN, VK_RIGHT, VK_SHIFT, VK_SPACE, VK_TAB, VK_UP,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GWL_STYLE, GetWindowLongW, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCLIENT, HTLEFT, HTRIGHT, HTTOP, HTTOPLEFT,
    HTTOPRIGHT, IDC_HELP, IsZoomed, LoadCursorW, MINMAXINFO, SIZE_MAXIMIZED, SIZE_RESTORED, SPI_SETWORKAREA,
    SW_MAXIMIZE, SW_RESTORE, SetCursor, WM_DISPLAYCHANGE, WM_EXITSIZEMOVE, WM_SETTINGCHANGE,
    WM_GETMINMAXINFO, WM_MOVE, WM_NCACTIVATE, WM_NCCALCSIZE, WM_NCHITTEST, WM_SETCURSOR, WS_MAXIMIZEBOX, WS_THICKFRAME,
    CreateIconIndirect, CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyWindow, FindWindowExW, GetCaretBlinkTime,
    GetClassNameW, GetWindowPlacement, GetWindowRect, GetWindowThreadProcessId, HICON, HTCAPTION, ICON_BIG, ICON_SMALL,
    ICONINFO, IsIconic, SetWindowPlacement, WINDOWPLACEMENT,
    KillTimer, PostMessageW, RegisterWindowMessageW, SC_KEYMENU, SIZE_MINIMIZED, SW_SHOW, SWP_NOACTIVATE,
    SWP_NOZORDER, SendMessageW, SetForegroundWindow, SetTimer, SetWindowPos, ShowWindow, WA_INACTIVE, WM_ACTIVATE,
    WM_CAPTURECHANGED, WM_CHAR, WM_CLOSE, WM_ERASEBKGND, WM_KEYDOWN, WM_KILLFOCUS, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MBUTTONDBLCLK, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCLBUTTONDOWN,
    WM_PAINT, WM_RBUTTONDBLCLK, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SETFOCUS, WM_SETICON, WM_SIZE, WM_SYSCHAR,
    WM_SYSCOMMAND, WM_SYSKEYDOWN, WM_TIMER, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_MINIMIZEBOX,
    WS_POPUP, WS_SYSMENU,
};

use crate::log::{log, log_win32};
use crate::util::{blit, point, take_focus, wide};

pub const KIT_CLASS: &str = "Bevel95Window";
/// The caret's blink timer; a window's own timers use other ids.
const CARET: usize = 1;
/// Windows' message for the pointer leaving a window (WM_MOUSELEAVE; in windows-sys it sits
/// with the common controls, which this crate does not otherwise need).
const WM_MOUSELEAVE: u32 = 0x02A3;
/// The message the bar posts to every kit window of other programs when its look or size
/// changed (`tell_programs_look_changed`); registered by name, so every program knows it.
const LOOK_CHANGED_NAME: &str = "Bevel95.LookChanged";

/// A window kind the host can show: its form and whatever else it keeps.
pub trait Hosted: Sized + 'static {
    type F: Form;
    fn form(&self) -> &Self::F;
    fn form_mut(&mut self) -> &mut Self::F;
    /// Carries out a button (OK, Apply, the title bar's "?" and "X", ...). Called with nothing
    /// borrowed, so it may close the window or change it with `update`.
    fn activate(part: <Self::F as Form>::Part);
    /// Handles a user close request (Alt+F4 or WM_CLOSE). A dialog can treat it as Cancel.
    fn request_close() {
        close::<Self>();
    }
    /// The window's own timers: (id, milliseconds); id 1 is the caret's.
    fn timers() -> &'static [(usize, u32)] {
        &[]
    }
    /// One of its timers ran out. True if what it shows changed.
    fn timer(&mut self, _id: usize) -> bool {
        false
    }
    /// The window is about to go (it still exists: a program notes where it was).
    fn closing(&mut self, _hwnd: HWND) {}
    /// The window is gone (and with it anything it showed besides).
    fn closed(&mut self) {}
    /// A program's main window was minimized, or brought back.
    fn minimized(&mut self, _minimized: bool) {}
    /// A resizable program's client size in logical pixels, including its painted frame.
    fn resized(&mut self, _size: (i32, i32), _maximized: bool) {}
    /// A move/resize finished or the available desktop changed. The host is released
    /// before this callback, so a window can fit its contents to its new work area.
    fn placement_changed(_hwnd: HWND) {}
    /// Smallest size Windows lets the user drag a resizable program to, in logical pixels.
    fn minimum_size(&self) -> (i32, i32) {
        (120, 60)
    }
    /// The bar's look or size changed: a program reads its settings again and calls
    /// `retheme_with`. Called with nothing borrowed.
    fn look_changed() {}
}

/// What a window paints with.
#[derive(Clone)]
pub struct Look {
    pub theme: &'static Theme,
    pub regular: Font,
    pub bold: Font,
    /// The whole-number size (1 = original pixels).
    pub scale: i32,
}

impl Look {
    pub fn fonts(&self) -> Fonts<'_> {
        Fonts { regular: &self.regular, bold: &self.bold }
    }
}

/// How a window sits among the others.
pub enum Kind {
    /// One of the bar's own windows: above every window, no taskbar button.
    Tool,
    /// A program's main window: it has a taskbar button, can be minimized, and shows these
    /// pictures there and in Alt+Tab (16×16 and 32×32).
    Program { small: Option<Icon>, big: Option<Icon> },
    /// A program whose form follows the native sizing border and maximize/restore commands.
    ResizableProgram { small: Option<Icon>, big: Option<Icon> },
    /// A program's dialog: it stays above `owner`, which cannot be used until it is closed.
    Dialog { owner: HWND },
    /// A modeless dialog stays above its program while the user continues working in either window.
    Owned { owner: HWND },
}

type PartOf<H> = <<H as Hosted>::F as Form>::Part;
/// What is left to do after a message.
type Todo<H> = Vec<Later<Effect<PartOf<H>>>>;

/// One open window: its kind's state, the controller, and what it paints with.
struct Host<H: Hosted> {
    hwnd: HWND,
    hosted: H,
    ctl: Controller<PartOf<H>>,
    look: Look,
    caret_on: bool,
    /// A dialog's owner (enabled again when the dialog closes).
    /// Only a modal dialog disables and re-enables its owner.
    modal_owner: HWND,
    /// The icons made for a program's window (destroyed with it).
    icons: [HICON; 2],
    resizable: bool,
    /// Windows was asked to say when the pointer leaves the window.
    leave_armed: bool,
    /// Where the form's top left corner is in the window, and the window's size (1× pixels):
    /// (0, 0) and the form's size, unless an open drop-down list reaches past the form's edges
    /// (`Form::drop_rect`); then the window is that much bigger and shaped by a region to the
    /// form and the list.
    origin: (i32, i32),
    extent: (i32, i32),
}

/// The rectangle the window must cover, in the form's coordinates: the form and its open list.
fn bounds<F: Form>(form: &F) -> bevel_core::fb::Rect {
    let (w, h) = form.size();
    let own = bevel_core::fb::Rect::new(0, 0, w, h);
    let Some(d) = form.drop_rect() else { return own };
    let (x0, y0) = (d.x.min(0), d.y.min(0));
    let (x1, y1) = (d.right().max(w), d.bottom().max(h));
    bevel_core::fb::Rect::new(x0, y0, x1 - x0, y1 - y0)
}

/// What must happen once the window's state is back in the list.
enum Later<P> {
    Effect(P),
    Repaint,
    RestartCaret,
    /// An open list came or went: the window grows or shrinks around it (`reshape`).
    Reshape,
}

impl<H: Hosted> Host<H> {
    /// What the window covers now, in the form's coordinates.
    fn bounds_now(&self) -> bevel_core::fb::Rect {
        bevel_core::fb::Rect::new(-self.origin.0, -self.origin.1, self.extent.0, self.extent.1)
    }

    fn relayout(&mut self) {
        let fonts = Fonts { regular: &self.look.regular, bold: &self.look.bold };
        self.hosted.form_mut().layout(&fonts);
    }

    fn paint(&self, hdc: windows_sys::Win32::Graphics::Gdi::HDC) {
        let form = self.hosted.form();
        let (w, h) = form.size();
        let mut fb = Framebuffer::new(w, h, 0);
        form.paint(&mut fb, self.look.theme, &self.look.fonts());
        if self.extent != (w, h) || self.origin != (0, 0) || form.drop_rect().is_some() {
            // An open list (past the form's edges or not): the form and the list, each where
            // it goes.
            let (ox, oy) = self.origin;
            let mut whole = Framebuffer::new(self.extent.0, self.extent.1, 0);
            whole.blit(&fb, ox, oy);
            if let Some(d) = form.drop_rect() {
                let mut list = Framebuffer::new(d.w, d.h, 0);
                form.paint_drop(&mut list, self.look.theme, &self.look.fonts());
                whole.blit(&list, d.x + ox, d.y + oy);
            }
            fb = whole;
        }
        let big = upscale(&fb, self.look.scale);
        blit(hdc, &big, big.width);
    }

    /// Handles one message; returns its answer (`None`: Windows' default) and what to do after.
    fn message(&mut self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> (Option<LRESULT>, Todo<H>) {
        let s = self.look.scale.max(1);
        let (ox, oy) = self.origin;
        // The pointer in the form's 1× coordinates.
        let at = |lparam: LPARAM| {
            let (x, y) = point(lparam);
            (x.div_euclid(s) - ox, y.div_euclid(s) - oy)
        };
        let form = self.hosted.form_mut();
        let (answer, effects, changed) = match msg {
            WM_NCHITTEST if self.resizable => {
                let mut r = RECT::default();
                // SAFETY: the window receiving the hit test and a valid rectangle.
                let hit = unsafe {
                    if IsZoomed(self.hwnd) == 0 && GetWindowRect(self.hwnd, &mut r) != 0 {
                        resize_hit(&r, point(lparam), 4 * s)
                    } else {
                        HTCLIENT
                    }
                };
                (Some(hit as LRESULT), Vec::new(), false)
            }
            WM_GETMINMAXINFO if self.resizable => {
                let (min_w, min_h) = self.hosted.minimum_size();
                // SAFETY: Windows supplies MINMAXINFO for this message. Override its padded
                // native frame bounds because this form draws to the very edge of the window.
                unsafe {
                    let limits = &mut *(lparam as *mut MINMAXINFO);
                    limits.ptMinTrackSize.x = min_w.max(1) * s;
                    limits.ptMinTrackSize.y = min_h.max(1) * s;
                    let monitor = MonitorFromWindow(self.hwnd, MONITOR_DEFAULTTONEAREST);
                    let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
                    if GetMonitorInfoW(monitor, &mut info) != 0 {
                        limits.ptMaxPosition.x = info.rcWork.left - info.rcMonitor.left;
                        limits.ptMaxPosition.y = info.rcWork.top - info.rcMonitor.top;
                        limits.ptMaxSize.x = info.rcWork.right - info.rcWork.left;
                        limits.ptMaxSize.y = info.rcWork.bottom - info.rcWork.top;
                    }
                }
                (Some(0), Vec::new(), false)
            }
            WM_SETCURSOR if (lparam as u32 & 0xffff) == HTCLIENT && form.context_help() => {
                // SAFETY: IDC_HELP is a shared system cursor.
                unsafe { SetCursor(LoadCursorW(null_mut(), IDC_HELP)) };
                return (Some(1), Vec::new());
            }
            // The form's own pointer over its places (a header's divider, a text field, a link).
            WM_SETCURSOR if (lparam as u32 & 0xffff) == HTCLIENT => {
                let mut p = windows_sys::Win32::Foundation::POINT::default();
                // SAFETY: plain queries about our own window and the pointer.
                let ok = unsafe {
                    windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut p) != 0
                        && windows_sys::Win32::Graphics::Gdi::ScreenToClient(self.hwnd, &mut p) != 0
                };
                if ok && crate::pointer::show(form.pointer(p.x.div_euclid(s) - ox, p.y.div_euclid(s) - oy)) {
                    return (Some(1), Vec::new());
                }
                return (None, Vec::new());
            }
            WM_LBUTTONDOWN | WM_LBUTTONDBLCLK => {
                let (x, y) = at(lparam);
                form.mouse_mods(mods());
                let handled = if msg == WM_LBUTTONDBLCLK { form.double_click(x, y) } else { None };
                (Some(0), handled.unwrap_or_else(|| self.ctl.mouse_down(form, x, y)), true)
            }
            WM_MOUSEMOVE => {
                let (x, y) = at(lparam);
                let changed = self.ctl.mouse_move(form, x, y);
                let pending = form.pending();
                if !std::mem::replace(&mut self.leave_armed, true) {
                    let mut track =
                        TRACKMOUSEEVENT { cbSize: size_of::<TRACKMOUSEEVENT>() as u32, dwFlags: TME_LEAVE, hwndTrack: self.hwnd, dwHoverTime: 0 };
                    // SAFETY: a correctly sized struct for our own window.
                    unsafe { TrackMouseEvent(&mut track) };
                }
                (Some(0), pending, changed)
            }
            WM_MOUSELEAVE => {
                self.leave_armed = false;
                (Some(0), Vec::new(), form.pointer_left())
            }
            WM_LBUTTONUP => {
                let (x, y) = at(lparam);
                (Some(0), self.ctl.mouse_up(form, x, y), true)
            }
            WM_RBUTTONDOWN | WM_RBUTTONDBLCLK | WM_RBUTTONUP | WM_MBUTTONDOWN | WM_MBUTTONDBLCLK | WM_MBUTTONUP => {
                let (x, y) = at(lparam);
                let button = if matches!(msg, WM_RBUTTONDOWN | WM_RBUTTONDBLCLK | WM_RBUTTONUP) {
                    Button::Right
                } else {
                    Button::Middle
                };
                let down = !matches!(msg, WM_RBUTTONUP | WM_MBUTTONUP);
                (Some(0), self.ctl.button(form, button, down, x, y), true)
            }
            // Another window took the mouse (never this window itself: it only lets go).
            WM_CAPTURECHANGED if lparam != 0 => {
                self.ctl.lost_capture(form);
                (Some(0), Vec::new(), true)
            }
            WM_MOUSEWHEEL => {
                let notches = i32::from((wparam >> 16) as u16 as i16) / 120;
                if notches != 0 {
                    form.wheel(notches);
                }
                (Some(0), Vec::new(), notches != 0)
            }
            WM_KEYDOWN => (Some(0), self.ctl.key(form, key_of(wparam as u16), mods()), true),
            WM_CHAR => {
                let effects = char::from_u32(wparam as u32).map(|ch| form.typed(ch)).unwrap_or_default();
                (Some(0), effects, true)
            }
            // Alt+letter.
            WM_SYSCHAR => {
                let effects = char::from_u32(wparam as u32).map(|ch| self.ctl.access(form, ch)).unwrap_or_default();
                (Some(0), effects, true)
            }
            WM_SYSKEYDOWN => {
                let (handled, effects) = self.ctl.alt_key_with_mods(form, key_of(wparam as u16), mods());
                // Painted afresh even when the key stays Windows' own: Alt going down may have
                // made a menu bar show its access keys.
                (handled.then_some(0), effects, true)
            }
            // Alt alone or F10 (Windows turns both into this): a window with a menu bar.
            WM_SYSCOMMAND if (wparam & 0xFFF0) as u32 == SC_KEYMENU && lparam == 0 => {
                (Some(0), form.menu_key(), true)
            }
            WM_TIMER if wparam == CARET => {
                if form.has_caret() {
                    self.caret_on = !self.caret_on;
                    form.blink(self.caret_on);
                }
                return (Some(0), vec![Later::Repaint]);
            }
            WM_TIMER => {
                let changed = self.hosted.timer(wparam);
                if changed {
                    self.relayout();
                }
                return (Some(0), if changed { vec![Later::Repaint] } else { Vec::new() });
            }
            WM_SETFOCUS | WM_KILLFOCUS => {
                self.caret_on = msg == WM_SETFOCUS;
                form.blink(self.caret_on);
                (None, Vec::new(), true)
            }
            // In front or not (a program's title bar shows it). One of this window's own menus
            // taking the focus leaves it looking in front, as Windows' menus do.
            WM_ACTIVATE => {
                let active = (wparam & 0xFFFF) as u32 != WA_INACTIVE || crate::menu::is_ours(lparam as HWND, self.hwnd);
                form.set_active(active);
                (None, Vec::new(), true)
            }
            WM_SIZE => {
                let minimized = wparam as u32 == SIZE_MINIMIZED;
                self.hosted.minimized(minimized);
                if self.resizable && !minimized {
                    let w = i32::from(lparam as u16);
                    let h = i32::from((lparam >> 16) as u16);
                    // Round up: a non-integral last row or column is clipped by the client DC.
                    self.hosted.resized(((w + s - 1) / s, (h + s - 1) / s), wparam as u32 == SIZE_MAXIMIZED);
                }
                (None, Vec::new(), self.resizable && !minimized)
            }
            WM_MOVE if self.resizable => {
                let (x, y) = point(lparam);
                form.set_screen(x.div_euclid(s) + ox, y.div_euclid(s) + oy);
                (None, Vec::new(), true)
            }
            WM_CLOSE => (Some(0), vec![Effect::Close], false),
            _ => return (None, Vec::new()),
        };
        let mut later: Vec<_> = effects.into_iter().map(Later::Effect).collect();
        // A key or a click shows the caret afresh (Windows restarts its blinking then).
        if matches!(msg, WM_KEYDOWN | WM_CHAR | WM_SYSCHAR | WM_LBUTTONDOWN) && self.hosted.form().has_caret() {
            later.push(Later::RestartCaret);
        }
        if changed {
            self.relayout();
            if bounds(self.hosted.form()) != self.bounds_now() {
                later.push(Later::Reshape);
            }
            later.push(Later::Repaint);
        }
        (answer, later)
    }
}

/// Screen coordinates can be negative on monitors left of or above the primary.
fn resize_hit(r: &RECT, (x, y): (i32, i32), border: i32) -> u32 {
    let left = x < r.left + border;
    let right = x >= r.right - border;
    let top = y < r.top + border;
    let bottom = y >= r.bottom - border;
    match (left, right, top, bottom) {
        (true, _, true, _) => HTTOPLEFT,
        (true, _, _, true) => HTBOTTOMLEFT,
        (_, true, true, _) => HTTOPRIGHT,
        (_, true, _, true) => HTBOTTOMRIGHT,
        (true, _, _, _) => HTLEFT,
        (_, true, _, _) => HTRIGHT,
        (_, _, true, _) => HTTOP,
        (_, _, _, true) => HTBOTTOM,
        _ => HTCLIENT,
    }
}

/// The hosted windows, one per kind; `None` while one is handling a message.
struct Slot {
    hwnd: HWND,
    kind: TypeId,
    host: Option<Box<dyn Any>>,
}

thread_local! {
    static WINDOWS: RefCell<Vec<Slot>> = const { RefCell::new(Vec::new()) };
}

thread_local! {
    /// For kinds with several windows open at once (a program's folder windows): the one the
    /// calls by kind act on now. Set while one of them handles a message (`message`) and by
    /// `scope`; otherwise the first of its kind.
    static CURRENT: RefCell<Vec<(TypeId, HWND)>> = const { RefCell::new(Vec::new()) };
}

fn current(kind: TypeId) -> Option<HWND> {
    CURRENT.with(|c| c.borrow().iter().rev().find(|(k, _)| *k == kind).map(|&(_, h)| h))
}

/// The window of `kind` the calls by kind act on: the current one if it is still open, else the
/// first of its kind.
fn chosen(slots: &[Slot], kind: TypeId) -> Option<usize> {
    let cur = current(kind);
    slots.iter().position(|s| s.kind == kind && Some(s.hwnd) == cur).or_else(|| slots.iter().position(|s| s.kind == kind))
}

/// While it lives, `hwnd` is the window of its kind the calls by kind act on.
struct Scope;

impl Scope {
    fn enter(kind: TypeId, hwnd: HWND) -> Scope {
        CURRENT.with(|c| c.borrow_mut().push((kind, hwnd)));
        Scope
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        CURRENT.with(|c| {
            c.borrow_mut().pop();
        });
    }
}

/// Runs `f` with `hwnd` (a window of kind `H`) as the one `with`, `update`, `close` and the rest
/// act on: for kinds with several windows open at once.
pub fn scope<H: Hosted, R>(hwnd: HWND, f: impl FnOnce() -> R) -> R {
    let _scope = Scope::enter(TypeId::of::<H>(), hwnd);
    f()
}

/// Every open window of kind `H`.
pub fn windows<H: Hosted>() -> Vec<HWND> {
    let kind = TypeId::of::<H>();
    WINDOWS.with(|w| w.borrow().iter().filter(|s| s.kind == kind).map(|s| s.hwnd).collect())
}

/// Lifts the window of kind `H` out of the list, runs `f` on it and puts it back.
fn lift<H: Hosted, R>(f: impl FnOnce(&mut Host<H>) -> R) -> Option<R> {
    let kind = TypeId::of::<H>();
    let (hwnd, mut boxed) = WINDOWS.with(|w| {
        let mut w = w.borrow_mut();
        let at = chosen(&w, kind)?;
        let slot = &mut w[at];
        slot.host.take().map(|h| (slot.hwnd, h))
    })?;
    let result = boxed.downcast_mut::<Host<H>>().map(f);
    WINDOWS.with(|w| {
        if let Some(slot) = w.borrow_mut().iter_mut().find(|s| s.kind == kind && s.hwnd == hwnd) {
            slot.host = Some(boxed);
        }
    });
    result
}

/// Runs `f` on the open window of kind `H` (nothing is laid out or repainted).
pub fn with<H: Hosted, R>(f: impl FnOnce(&mut H) -> R) -> Option<R> {
    lift::<H, R>(|host| f(&mut host.hosted))
}

/// Changes the open window of kind `H`, lays it out again and repaints it.
pub fn update<H: Hosted>(f: impl FnOnce(&mut H)) {
    if let Some(hwnd) = lift::<H, HWND>(|host| {
        f(&mut host.hosted);
        host.relayout();
        host.hwnd
    }) {
        reshape::<H>(hwnd);
        // SAFETY: our own window.
        unsafe { InvalidateRect(hwnd, null(), 0) };
    }
}

/// Fits the window of kind `H` to its form and its open list (`Form::drop_rect`): bigger and
/// shaped by a region to the two while the list is open, the form's own size again once it has
/// closed. The form stays where it is on the screen.
fn reshape<H: Hosted>(hwnd: HWND) {
    let Some((old, new, s)) = lift::<H, _>(|host| {
        let old = host.bounds_now();
        let new = bounds(host.hosted.form());
        host.origin = (-new.x, -new.y);
        host.extent = (new.w, new.h);
        (old, new, host.look.scale.max(1))
    }) else {
        return;
    };
    if old == new {
        return;
    }
    let (w, h) = lift::<H, _>(|host| host.hosted.form().size()).unwrap_or((new.w, new.h));
    let drop = lift::<H, _>(|host| host.hosted.form().drop_rect()).flatten();
    let mut r = RECT::default();
    // SAFETY: our own window; regions are made here and handed to Windows (which then owns
    // them) or deleted.
    unsafe {
        GetWindowRect(hwnd, &mut r);
        let (x, y) = (r.left + (new.x - old.x) * s, r.top + (new.y - old.y) * s);
        SetWindowPos(hwnd, null_mut(), x, y, new.w * s, new.h * s, SWP_NOZORDER | SWP_NOACTIVATE | windows_sys::Win32::UI::WindowsAndMessaging::SWP_NOREDRAW);
        let (ox, oy) = (-new.x, -new.y);
        let region = match drop {
            Some(d) if new != bevel_core::fb::Rect::new(0, 0, w, h) => {
                let own = CreateRectRgn(ox * s, oy * s, (ox + w) * s, (oy + h) * s);
                let list = CreateRectRgn((d.x + ox) * s, (d.y + oy) * s, (d.right() + ox) * s, (d.bottom() + oy) * s);
                CombineRgn(own, own, list, RGN_OR);
                DeleteObject(list);
                own
            }
            _ => null_mut(),
        };
        if SetWindowRgn(hwnd, region, 1) == 0 {
            log_win32("SetWindowRgn (a window's open list)");
            if !region.is_null() {
                DeleteObject(region);
            }
        }
        InvalidateRect(hwnd, null(), 0);
    }
}

/// The open window of kind `H`.
pub fn hwnd<H: Hosted>() -> Option<HWND> {
    let kind = TypeId::of::<H>();
    WINDOWS.with(|w| {
        let w = w.borrow();
        chosen(&w, kind).map(|i| w[i].hwnd)
    })
}

/// What the open window of kind `H` paints with (a copy).
pub fn look_of<H: Hosted>() -> Option<Look> {
    lift::<H, Look>(|host| host.look.clone())
}

/// Registers the kit's window class (once; the bar and Bevel95 Setup both call it).
pub fn ensure_class() {
    static DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if DONE.swap(true, Ordering::SeqCst) {
        return;
    }
    let name = wide(KIT_CLASS);
    let class = windows_sys::Win32::UI::WindowsAndMessaging::WNDCLASSEXW {
        cbSize: size_of::<windows_sys::Win32::UI::WindowsAndMessaging::WNDCLASSEXW>() as u32,
        style: windows_sys::Win32::UI::WindowsAndMessaging::CS_DBLCLKS,
        lpfnWndProc: Some(kit_proc),
        // SAFETY: our own module; a system cursor.
        hInstance: unsafe { GetModuleHandleW(null()) },
        hCursor: unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::LoadCursorW(
                null_mut(),
                windows_sys::Win32::UI::WindowsAndMessaging::IDC_ARROW,
            )
        },
        lpszClassName: name.as_ptr(),
        ..Default::default()
    };
    // SAFETY: a fully initialized class description (the name is copied by Windows).
    if unsafe { windows_sys::Win32::UI::WindowsAndMessaging::RegisterClassExW(&class) } == 0 {
        log_win32("RegisterClassExW(kit)");
    }
}

/// Runs Windows' messages until the window of kind `H` is closed (for Bevel95 Setup, which
/// comes before the bar and its message loop, and for a program's main window).
pub fn run_until_closed<H: Hosted>() {
    let mut msg = windows_sys::Win32::UI::WindowsAndMessaging::MSG::default();
    while hwnd::<H>().is_some() {
        // SAFETY: the standard message loop.
        unsafe {
            if windows_sys::Win32::UI::WindowsAndMessaging::GetMessageW(&mut msg, null_mut(), 0, 0) <= 0 {
                break;
            }
            windows_sys::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
            windows_sys::Win32::UI::WindowsAndMessaging::DispatchMessageW(&msg);
        }
    }
}

/// Opens one of the bar's own windows (`Kind::Tool`) of kind `H` with the look, fonts and
/// size given; `place` gets its size in screen pixels and says where it goes. An open one of
/// the same kind is closed first.
pub fn open_with<H: Hosted>(
    hosted: H,
    theme: &'static Theme,
    regular: Font,
    bold: Font,
    scale: i32,
    place: impl FnOnce(i32, i32) -> (i32, i32),
) -> Option<HWND> {
    open_as(Kind::Tool, hosted, Look { theme, regular, bold, scale }, place)
}

/// Opens a window of kind `H` as `kind` says, painting with `look`; `place` gets its size in
/// screen pixels and says where it goes. An open one of the same kind is closed first.
pub fn open_as<H: Hosted>(
    kind: Kind,
    hosted: H,
    look: Look,
    place: impl FnOnce(i32, i32) -> (i32, i32),
) -> Option<HWND> {
    close::<H>();
    open_another(kind, hosted, look, place)
}

/// Opens one more window of kind `H` (the others of its kind stay open: a program's folder
/// windows), as `open_as` does. The calls by kind reach it while it handles its messages and in a
/// `scope` of it.
pub fn open_another<H: Hosted>(
    kind: Kind,
    mut hosted: H,
    mut look: Look,
    place: impl FnOnce(i32, i32) -> (i32, i32),
) -> Option<HWND> {
    ensure_class();
    register::<H>();
    look.scale = look.scale.max(1);
    let scale = look.scale;
    hosted.form_mut().layout(&look.fonts());
    let (w, h) = hosted.form().size();
    let (w, h) = (w * scale, h * scale);
    let (x, y) = place(w, h);
    hosted.form_mut().set_screen(x.div_euclid(scale), y.div_euclid(scale));
    let class = wide(KIT_CLASS);
    let title = wide(hosted.form().title());
    let (ex_style, style, owner) = match &kind {
        Kind::Tool => (WS_EX_TOOLWINDOW | WS_EX_TOPMOST, WS_POPUP, null_mut()),
        Kind::Program { .. } => (WS_EX_APPWINDOW, WS_POPUP | WS_MINIMIZEBOX | WS_SYSMENU, null_mut()),
        Kind::ResizableProgram { .. } => (
            WS_EX_APPWINDOW, WS_POPUP | WS_MINIMIZEBOX | WS_MAXIMIZEBOX | WS_THICKFRAME | WS_SYSMENU, null_mut(),
        ),
        Kind::Dialog { owner } | Kind::Owned { owner } => (0, WS_POPUP, *owner),
    };
    // SAFETY: a registered class; top-level so it gets its own focus.
    let hwnd = unsafe {
        CreateWindowExW(ex_style, class.as_ptr(), title.as_ptr(), style, x, y, w, h, owner, null_mut(), GetModuleHandleW(null()), null())
    };
    if hwnd.is_null() {
        log_win32("CreateWindowExW(kit window)");
        return None;
    }
    let mut icons: [HICON; 2] = [null_mut(); 2];
    if let Kind::Program { small, big } | Kind::ResizableProgram { small, big } = &kind {
        install_icons(hwnd, &mut icons, [small.as_ref(), big.as_ref()]);
    }
    let resizable = matches!(kind, Kind::ResizableProgram { .. });
    let modal_owner = if matches!(kind, Kind::Dialog { .. }) { owner } else { null_mut() };
    let extent = hosted.form().size();
    let mut host = Host {
        hwnd,
        hosted,
        ctl: Controller::default(),
        look,
        caret_on: true,
        modal_owner,
        icons,
        resizable,
        leave_armed: false,
        origin: (0, 0),
        extent,
    };
    host.relayout();
    WINDOWS.with(|w| w.borrow_mut().push(Slot { hwnd, kind: TypeId::of::<H>(), host: Some(Box::new(host)) }));
    let _scope = Scope::enter(TypeId::of::<H>(), hwnd);
    // SAFETY: our own window and timers; a dialog's owner waits for it.
    unsafe {
        for &(id, ms) in H::timers() {
            SetTimer(hwnd, id, ms, None);
        }
        if !modal_owner.is_null() {
            EnableWindow(modal_owner, 0);
        }
        ShowWindow(hwnd, SW_SHOW);
        take_focus(hwnd);
    }
    restart_caret(hwnd);
    Some(hwnd)
}

/// Closes the open window of kind `H`, if any.
pub fn close<H: Hosted>() {
    let kind = TypeId::of::<H>();
    let slot = WINDOWS.with(|w| {
        let mut w = w.borrow_mut();
        let at = chosen(&w, kind)?;
        Some(w.remove(at))
    });
    let Some(slot) = slot else {
        return;
    };
    // (While it handles a message its state is lifted out: it is dropped there.)
    let mut host = slot.host;
    let mut host = host.as_mut().and_then(|h| h.downcast_mut::<Host<H>>());
    if let Some(host) = host.as_mut() {
        host.hosted.closing(slot.hwnd);
    }
    let (owner, icons) = host.as_ref().map_or((null_mut(), [null_mut(); 2]), |h| (h.modal_owner, h.icons));
    // SAFETY: our own windows, timers and icons. A dialog's owner is usable again before the
    // dialog goes, so that Windows brings the owner to the front and not some other program.
    unsafe {
        KillTimer(slot.hwnd, CARET);
        for &(id, _) in H::timers() {
            KillTimer(slot.hwnd, id);
        }
        if !owner.is_null() {
            EnableWindow(owner, 1);
        }
        DestroyWindow(slot.hwnd);
        if !owner.is_null() {
            SetForegroundWindow(owner);
        }
        for icon in icons {
            if !icon.is_null() {
                DestroyIcon(icon);
            }
        }
    }
    if let Some(host) = host {
        host.hosted.closed();
    }
}

/// After the look or the size changed: the open window of kind `H` paints with `look` from now
/// on, laid out again; `place` says where it goes now (given its size in screen pixels and
/// where it is).
pub fn retheme_with<H: Hosted>(look: Look, place: impl FnOnce(i32, i32, RECT) -> (i32, i32)) {
    let scale = look.scale.max(1);
    let Some((hwnd, w, h)) = lift::<H, (HWND, i32, i32)>(|host| {
        host.look = Look { scale, ..look };
        host.relayout();
        let (w, h) = host.hosted.form().size();
        (host.origin, host.extent) = ((0, 0), (w, h));
        (host.hwnd, w * scale, h * scale)
    }) else {
        return;
    };
    let now = window_rect(hwnd);
    let (x, y) = place(w, h, now);
    lift::<H, ()>(|host| host.hosted.form_mut().set_screen(x.div_euclid(scale), y.div_euclid(scale)));
    move_window(hwnd, x, y, w, h);
    reshape::<H>(hwnd);
}

/// After a change of the form's size (a game's bigger field): the window takes the new size,
/// its top-left corner where it was.
pub fn resize<H: Hosted>() {
    let Some((hwnd, w, h, ox, oy)) = lift::<H, _>(|host| {
        host.relayout();
        let (w, h) = host.hosted.form().size();
        let s = host.look.scale;
        let (ox, oy) = std::mem::take(&mut host.origin);
        host.extent = (w, h);
        (host.hwnd, w * s, h * s, ox * s, oy * s)
    }) else {
        return;
    };
    let now = window_rect(hwnd);
    move_window(hwnd, now.left + ox, now.top + oy, w, h);
    reshape::<H>(hwnd);
}

/// Uses Windows' saved placement so restoring a maximized program returns it to its last
/// manually sized rectangle. WM_SIZE updates the form after the placement changes.
pub fn toggle_maximize<H: Hosted>() {
    if let Some(hwnd) = hwnd::<H>() {
        // SAFETY: our own program window, with no host borrowed during the synchronous resize.
        unsafe { ShowWindow(hwnd, if IsZoomed(hwnd) != 0 { SW_RESTORE } else { SW_MAXIMIZE }) };
    }
}

/// Where a window is on the screen; for a minimized one, where it comes back to when restored
/// (Windows keeps a minimized window far off the screen).
fn window_rect(hwnd: HWND) -> RECT {
    let mut r = RECT::default();
    // SAFETY: our own window; correctly sized out-structs.
    unsafe {
        if IsIconic(hwnd) != 0 {
            let mut placement = WINDOWPLACEMENT { length: size_of::<WINDOWPLACEMENT>() as u32, ..Default::default() };
            GetWindowPlacement(hwnd, &mut placement);
            return placement.rcNormalPosition;
        }
        GetWindowRect(hwnd, &mut r);
    }
    r
}

/// Gives a window its place and size and has it painted again. A minimized window stays
/// minimized and takes them when it is restored. (Windows counts that place from the corner of
/// the screen's work area; with the taskbar at the bottom, as Bevel95's is, that is the
/// screen's own corner.)
fn move_window(hwnd: HWND, x: i32, y: i32, w: i32, h: i32) {
    // SAFETY: our own window; a placement read back from Windows with only its rectangle changed.
    unsafe {
        if IsIconic(hwnd) != 0 {
            let mut placement = WINDOWPLACEMENT { length: size_of::<WINDOWPLACEMENT>() as u32, ..Default::default() };
            if GetWindowPlacement(hwnd, &mut placement) != 0 {
                placement.rcNormalPosition = RECT { left: x, top: y, right: x + w, bottom: y + h };
                if SetWindowPlacement(hwnd, &placement) == 0 {
                    log_win32("SetWindowPlacement (a minimized window's new size)");
                }
            }
            return;
        }
        SetWindowPos(hwnd, null_mut(), x, y, w, h, SWP_NOZORDER | SWP_NOACTIVATE);
        InvalidateRect(hwnd, null(), 0);
    }
}

/// The message that tells a program's windows that the bar's look or size changed.
fn look_changed_message() -> u32 {
    static MESSAGE: AtomicU32 = AtomicU32::new(0);
    let known = MESSAGE.load(Ordering::Relaxed);
    if known != 0 {
        return known;
    }
    let name = wide(LOOK_CHANGED_NAME);
    // SAFETY: a valid null-terminated string.
    let message = unsafe { RegisterWindowMessageW(name.as_ptr()) };
    MESSAGE.store(message, Ordering::Relaxed);
    message
}

/// Called by the bar after its look or size changed (and was saved): every kit window of
/// another program hears of it and reads the settings again.
pub fn tell_programs_look_changed() {
    let (class, message) = (wide(KIT_CLASS), look_changed_message());
    // SAFETY: plain calls; `after` is null or a window this loop just found; the message carries
    // no pointers and is posted, so no program is waited for.
    unsafe {
        let ours = GetCurrentProcessId();
        let mut after: HWND = null_mut();
        loop {
            after = FindWindowExW(null_mut(), after, class.as_ptr(), null());
            if after.is_null() {
                break;
            }
            let mut process = 0;
            GetWindowThreadProcessId(after, &mut process);
            if process != ours {
                PostMessageW(after, message, 0, 0);
            }
        }
    }
}

/// Switches a running program's taskbar and Alt+Tab icons to the current look.
pub fn set_program_icons<H: Hosted>(small: &Icon, big: &Icon) {
    lift::<H, _>(|host| install_icons(host.hwnd, &mut host.icons, [Some(small), Some(big)]));
}

fn install_icons(hwnd: HWND, handles: &mut [HICON; 2], pictures: [Option<&Icon>; 2]) {
    for ((slot, picture), which) in handles.iter_mut().zip(pictures).zip([ICON_SMALL, ICON_BIG]) {
        if let Some(icon) = picture.and_then(icon_handle) {
            // SAFETY: this window receives our new handle before the old handle is destroyed.
            unsafe {
                SendMessageW(hwnd, WM_SETICON, which as usize, icon as LPARAM);
                if !slot.is_null() {
                    DestroyIcon(*slot);
                }
            }
            *slot = icon;
        }
    }
}

/// A Windows icon from a picture (for a program's taskbar button and Alt+Tab); `None` if
/// Windows would not make it. The caller destroys it.
fn icon_handle(icon: &Icon) -> Option<HICON> {
    let (w, h) = (icon.width, icon.height);
    if w <= 0 || h <= 0 {
        return None;
    }
    // The color picture as Windows wants it: straight alpha is fine for opaque or see-through
    // pixels (the pictures have no half-transparent ones); the mask marks the see-through ones.
    let color: Vec<u32> = icon.pixels.iter().map(|&p| if p >> 24 == 0 { 0 } else { p }).collect();
    let stride = (w as usize).div_ceil(16) * 2;
    let mut mask = vec![0u8; stride * h as usize];
    for (i, &p) in icon.pixels.iter().enumerate() {
        if p >> 24 == 0 {
            let (x, y) = (i % w as usize, i / w as usize);
            mask[y * stride + x / 8] |= 0x80 >> (x % 8);
        }
    }
    // SAFETY: both bitmaps are made from buffers of exactly the size their format needs, and are
    // deleted once the icon has its own copies.
    unsafe {
        let color_bitmap = CreateBitmap(w, h, 1, 32, color.as_ptr().cast());
        let mask_bitmap = CreateBitmap(w, h, 1, 1, mask.as_ptr().cast());
        let info = ICONINFO { fIcon: 1, xHotspot: 0, yHotspot: 0, hbmMask: mask_bitmap, hbmColor: color_bitmap };
        let handle = if color_bitmap.is_null() || mask_bitmap.is_null() { null_mut() } else { CreateIconIndirect(&info) };
        if !color_bitmap.is_null() {
            DeleteObject(color_bitmap);
        }
        if !mask_bitmap.is_null() {
            DeleteObject(mask_bitmap);
        }
        if handle.is_null() {
            log_win32("CreateIconIndirect (a program's window icon)");
            return None;
        }
        Some(handle)
    }
}

/// Shows the caret and starts its blinking afresh, at Windows' rate (unless blinking is off).
fn restart_caret(hwnd: HWND) {
    // SAFETY: a plain query; our own window's timer.
    unsafe {
        KillTimer(hwnd, CARET);
        let blink = GetCaretBlinkTime();
        if blink != 0 && blink != u32::MAX {
            SetTimer(hwnd, CARET, blink, None);
        }
    }
}

/// Windows' virtual key as the kit's key.
fn key_of(vk: u16) -> Key {
    match vk {
        VK_TAB => Key::Tab,
        VK_RETURN => Key::Enter,
        VK_ESCAPE => Key::Escape,
        VK_SPACE => Key::Space,
        VK_UP => Key::Up,
        VK_DOWN => Key::Down,
        VK_LEFT => Key::Left,
        VK_RIGHT => Key::Right,
        VK_HOME => Key::Home,
        VK_END => Key::End,
        VK_PRIOR => Key::PageUp,
        VK_NEXT => Key::PageDown,
        VK_DELETE => Key::Delete,
        VK_F4 => Key::F4,
        VK_SHIFT | VK_CONTROL | VK_MENU => Key::Modifier,
        other => Key::Other(other),
    }
}

fn mods() -> Mods {
    // SAFETY: plain queries.
    unsafe { Mods { shift: GetKeyState(i32::from(VK_SHIFT)) < 0, ctrl: GetKeyState(i32::from(VK_CONTROL)) < 0 } }
}

/// The clipboard's text, if it has any.
pub fn clipboard_text() -> Option<String> {
    // SAFETY: the clipboard is opened and closed here; the locked memory is read as the
    // null-terminated UTF-16 text Windows documents for CF_UNICODETEXT.
    unsafe {
        if OpenClipboard(null_mut()) == 0 {
            return None;
        }
        let handle = GetClipboardData(u32::from(CF_UNICODETEXT));
        let text = if handle.is_null() {
            None
        } else {
            let p = GlobalLock(handle) as *const u16;
            let text = (!p.is_null()).then(|| {
                let len = (0..).take_while(|&i| *p.add(i) != 0).count();
                String::from_utf16_lossy(std::slice::from_raw_parts(p, len))
            });
            GlobalUnlock(handle);
            text
        };
        CloseClipboard();
        text
    }
}

/// Carries out what a message left to do, with the window's state back in the list.
fn carry_out<H: Hosted>(hwnd: HWND, later: Todo<H>) {
    for item in later {
        match item {
            Later::Repaint => {
                // SAFETY: our own window.
                unsafe { InvalidateRect(hwnd, null(), 0) };
            }
            Later::Reshape => reshape::<H>(hwnd),
            Later::RestartCaret => {
                lift::<H, ()>(|host| {
                    host.caret_on = true;
                    host.hosted.form_mut().blink(true);
                });
                restart_caret(hwnd);
            }
            Later::Effect(Effect::Capture) => {
                // SAFETY: our own window; capture shows us the release even off the button.
                unsafe { SetCapture(hwnd) };
            }
            Later::Effect(Effect::Release) => {
                // SAFETY: releasing our own capture.
                unsafe { ReleaseCapture() };
            }
            Later::Effect(Effect::MoveWindow) => {
                // SAFETY: the standard way to drag a borderless window by its title bar; then
                // where it went (Windows 98's title bar gradient follows the position).
                let mut r = RECT::default();
                unsafe {
                    ReleaseCapture();
                    SendMessageW(hwnd, WM_NCLBUTTONDOWN, HTCAPTION as usize, 0);
                    GetWindowRect(hwnd, &mut r);
                }
                let (scale, (ox, oy)) = lift::<H, _>(|host| (host.look.scale.max(1), host.origin)).unwrap_or((1, (0, 0)));
                update::<H>(|h| h.form_mut().set_screen(r.left.div_euclid(scale) + ox, r.top.div_euclid(scale) + oy));
            }
            Later::Effect(Effect::Paste) => {
                if let Some(text) = clipboard_text() {
                    update::<H>(|h| h.form_mut().paste(&text));
                }
            }
            Later::Effect(Effect::Activate(part)) => H::activate(part),
            Later::Effect(Effect::Close) => H::request_close(),
        }
    }
}

/// What the window procedure needs of a window kind, without knowing it.
struct KnownKind {
    id: TypeId,
    message: fn(HWND, u32, WPARAM, LPARAM) -> Option<LRESULT>,
    look_changed: fn(),
}

thread_local! {
    static KINDS: RefCell<Vec<KnownKind>> = const { RefCell::new(Vec::new()) };
}

/// Makes window kind `H` known to the window procedure (once, before its first `open`).
pub fn register<H: Hosted>() {
    let id = TypeId::of::<H>();
    KINDS.with(|k| {
        let mut k = k.borrow_mut();
        if !k.iter().any(|kind| kind.id == id) {
            k.push(KnownKind { id, message: message::<H>, look_changed: H::look_changed });
        }
    });
}

/// One message to a window of kind `H`.
fn message<H: Hosted>(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
    // This window is the one of its kind every call by kind acts on while it handles this.
    let _scope = Scope::enter(TypeId::of::<H>(), hwnd);
    if msg == WM_PAINT {
        let mut ps = PAINTSTRUCT::default();
        // SAFETY: paired BeginPaint/EndPaint on our own window.
        let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
        lift::<H, ()>(|host| host.paint(hdc));
        // SAFETY: see above.
        unsafe { EndPaint(hwnd, &ps) };
        return Some(0);
    }
    if msg == WM_ERASEBKGND {
        return Some(1);
    }
    let (answer, later) = lift::<H, _>(|host| host.message(msg, wparam, lparam))?;
    carry_out::<H>(hwnd, later);
    if matches!(msg, WM_EXITSIZEMOVE | WM_DISPLAYCHANGE)
        || (msg == WM_SETTINGCHANGE && wparam as u32 == SPI_SETWORKAREA)
        || (msg == WM_SIZE && wparam as u32 == SIZE_RESTORED)
    {
        H::placement_changed(hwnd);
    }
    answer
}

/// The window procedure of every kit window.
///
/// # Safety
/// Called by Windows as the window procedure of `KIT_CLASS` windows.
pub unsafe extern "system" fn kit_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // Windows asks for the client bounds during CreateWindowExW, before the host is
    // registered. A sizing frame enables native resizing, but the form paints its pixels.
    // SAFETY: read the style of the window whose procedure Windows is calling.
    if msg == WM_NCCALCSIZE && unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32 & WS_THICKFRAME != 0 {
        return 0;
    }
    let kind = WINDOWS.with(|w| w.borrow().iter().find(|s| s.hwnd == hwnd).map(|s| s.kind));
    let known = kind.and_then(|id| {
        KINDS.with(|k| k.borrow().iter().find(|kind| kind.id == id).map(|kind| (kind.message, kind.look_changed)))
    });
    if let Some((_, look_changed)) = known
        && msg >= 0xC000
        && msg == look_changed_message()
    {
        look_changed();
        return 0;
    }
    if let Some(answer) = known.and_then(|(handle, _)| handle(hwnd, msg, wparam, lparam)) {
        return answer;
    }
    if msg == WM_PAINT && kind.is_some() {
        log("a kit window could not paint (its state was busy); it paints again next time");
    }
    // A sizing frame's window going behind or coming to the front (a menu of its own taking
    // the focus and giving it back, say): Windows' default handling would paint today's sizing
    // frame, in the system's 3-D colors, over the window's outer pixels until the form paints
    // again, a visible flash at the edges. lParam -1 keeps the default handling (the answer
    // that lets the activation change) without that painting: the form draws every pixel
    // itself and shows its own active or inactive title bar.
    // SAFETY: read the style of the window whose procedure Windows is calling.
    let lparam = if msg == WM_NCACTIVATE && unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32 & WS_THICKFRAME != 0 {
        -1
    } else {
        lparam
    };
    // SAFETY: default handling for everything else.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// Whether `hwnd` is a window of this class (any program's).
pub fn is_kit_window(hwnd: HWND) -> bool {
    let mut buf = [0u16; 64];
    // SAFETY: the buffer length is passed.
    let n = unsafe { GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32) }.max(0) as usize;
    String::from_utf16_lossy(&buf[..n]) == KIT_CLASS
}
