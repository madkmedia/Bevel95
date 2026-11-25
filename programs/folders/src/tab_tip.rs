//! A tab's tooltip (brief 110): a tab whose name is cut shows its whole name (a folder's whole
//! path) in the look's tooltip (`bevel_core::tooltip`, as the bar's tooltips are drawn) while the
//! pointer rests on it, as Windows' tooltips did: after the double-click time, for ten times as
//! long, hidden by a press until the pointer leaves the tab. One small topmost window that never
//! takes the focus and that clicks go through (the bar's tooltip window's styles, `tip.rs`).

use std::cell::{Cell, RefCell};
use std::ptr::{null, null_mut};
use std::time::{Duration, Instant};

use bevel_core::fb::{Framebuffer, Rect};
use bevel_core::tooltip;
use bevel_winkit::kit::{self, Look};
use bevel_winkit::log::log;
use bevel_winkit::util::{blit, wide};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{BeginPaint, ClientToScreen, EndPaint, InvalidateRect, PAINTSTRUCT, ScreenToClient};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, GetDoubleClickTime, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetCursorPos, GetForegroundWindow, HTTRANSPARENT, HWND_TOPMOST,
    LWA_ALPHA, MA_NOACTIVATE, RegisterClassW, SW_HIDE, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetLayeredWindowAttributes,
    SetWindowPos, ShowWindow, WM_ERASEBKGND, WM_MOUSEACTIVATE, WM_NCHITTEST, WM_PAINT, WNDCLASSW, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP, WindowFromPoint,
};

use crate::FolderHost;

const CLASS: &str = "Bevel95FolderTabTip";
/// Windows' tooltip control puts a tooltip this far below the pointer when there is room (1×).
const BELOW_POINTER: i32 = 20;

/// Where the pointer rests and what its tooltip does.
#[derive(Debug, Default)]
pub struct Rest {
    /// The window and the tab under the pointer.
    over: Option<(isize, u64)>,
    since: Option<Instant>,
    shown: bool,
    /// Hidden by a press, or shown its time: not again until the pointer leaves the tab.
    done: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TipDo {
    Nothing,
    Show,
    Hide,
}

/// One look at the pointer (every 100 ms): over a cut tab (`over`), a button held (`pressed`).
pub fn step(rest: &mut Rest, over: Option<(isize, u64)>, pressed: bool, now: Instant, wait: Duration) -> TipDo {
    if over != rest.over {
        let was = rest.shown;
        *rest = Rest { over, since: over.map(|_| now), shown: false, done: false };
        if was {
            return TipDo::Hide;
        }
    }
    if over.is_none() {
        return TipDo::Nothing;
    }
    let since = rest.since.unwrap_or(now);
    if pressed || (rest.shown && now.duration_since(since) >= wait * 11) {
        rest.done = true;
        if std::mem::take(&mut rest.shown) {
            return TipDo::Hide;
        }
        return TipDo::Nothing;
    }
    if !rest.shown && !rest.done && now.duration_since(since) >= wait {
        rest.shown = true;
        return TipDo::Show;
    }
    TipDo::Nothing
}

thread_local! {
    static REST: RefCell<Rest> = RefCell::new(Rest::default());
    static TIP: Cell<HWND> = const { Cell::new(null_mut()) };
    static SHOWS: RefCell<Option<(String, Look)>> = const { RefCell::new(None) };
}

/// Registers the tooltip's window class (once, at the start).
pub fn register() {
    let class = wide(CLASS);
    let wc = WNDCLASSW {
        lpfnWndProc: Some(tip_proc),
        // SAFETY: this program's own module.
        hInstance: unsafe { GetModuleHandleW(null()) },
        lpszClassName: class.as_ptr(),
        ..Default::default()
    };
    // SAFETY: a valid class description whose strings outlive the call.
    if unsafe { RegisterClassW(&wc) } == 0 {
        log("folders: the tabs' tooltip class could not be registered");
    }
}

/// Every 100 ms: the tooltip of the cut tab the pointer rests on, in the folder window in front.
pub fn tick() {
    let mut pt = POINT::default();
    // SAFETY: valid out-pointer; plain queries.
    let (under, front) = unsafe {
        if GetCursorPos(&mut pt) == 0 {
            return;
        }
        (WindowFromPoint(pt), GetForegroundWindow())
    };
    let ours = !under.is_null() && under == front && kit::windows::<FolderHost>().contains(&under);
    let found = if ours {
        kit::scope::<FolderHost, _>(under, || {
            let look = kit::look_of::<FolderHost>()?;
            let mut p = pt;
            // SAFETY: our own window and a valid point.
            unsafe { ScreenToClient(under, &mut p) };
            let (x, y) = (look.scale.old(p.x), look.scale.old(p.y));
            let (id, text, r) = kit::with::<FolderHost, _>(|h| h.form.tab_tip_at(x, y)).flatten()?;
            Some((id, text, r, look))
        })
    } else {
        None
    };
    // SAFETY: plain queries.
    let pressed = unsafe { [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON].iter().any(|&k| GetAsyncKeyState(k as i32) as u16 & 0x8000 != 0) };
    // SAFETY: a plain query.
    let wait = Duration::from_millis(u64::from(unsafe { GetDoubleClickTime() }));
    let over = found.as_ref().map(|(id, ..)| (under as isize, *id));
    match REST.with(|r| step(&mut r.borrow_mut(), over, pressed, Instant::now(), wait)) {
        TipDo::Show => {
            if let Some((_, text, r, look)) = found {
                show(under, pt, r, text, look);
            }
        }
        TipDo::Hide => hide(),
        TipDo::Nothing => {}
    }
}

fn show(owner: HWND, pointer: POINT, tab: Rect, text: String, look: Look) {
    let scale = look.scale;
    let (w, h) = tooltip::size(&look.regular, &text);
    let (w, h) = (scale.len(w), scale.len(h));
    let mut origin = POINT::default();
    // SAFETY: our own window and a valid point.
    unsafe { ClientToScreen(owner, &mut origin) };
    let t = scale.rect(tab);
    let tool = Rect::new(origin.x + t.x, origin.y + t.y, t.w, t.h);
    let work = crate::work_area(Some(owner));
    let screen = Rect::new(work.left, work.top, work.right - work.left, work.bottom - work.top);
    let (x, y) = tooltip::place((pointer.x, pointer.y), w, h, tool, screen, screen, scale.len(BELOW_POINTER), scale.len(1));
    SHOWS.with(|s| *s.borrow_mut() = Some((text, look)));
    let mut hwnd = TIP.with(Cell::get);
    if hwnd.is_null() {
        let class = wide(CLASS);
        // SAFETY: a registered class; owned by the folder window so it stays above it. Layered
        // and transparent: a press over it reaches the window under it (the bar's tooltip, brief 90).
        hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT | WS_EX_LAYERED,
                class.as_ptr(),
                null(),
                WS_POPUP,
                x,
                y,
                w,
                h,
                owner,
                null_mut(),
                GetModuleHandleW(null()),
                null(),
            )
        };
        if hwnd.is_null() {
            log("folders: the tabs' tooltip could not open");
            return;
        }
        // SAFETY: our own window; fully opaque (a layered window shows nothing until this is set).
        unsafe { SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA) };
        TIP.with(|t| t.set(hwnd));
    }
    // SAFETY: our own window, shown without taking the focus.
    unsafe {
        SetWindowPos(hwnd, HWND_TOPMOST, x, y, w, h, SWP_NOACTIVATE | SWP_SHOWWINDOW);
        InvalidateRect(hwnd, null(), 0);
    }
}

/// Hides the tooltip (if one shows).
pub fn hide() {
    let hwnd = TIP.with(Cell::get);
    if !hwnd.is_null() {
        // SAFETY: our own window.
        unsafe { ShowWindow(hwnd, SW_HIDE) };
    }
}

/// A folder window closes: the tooltip it owns goes with it (Windows destroys an owned window
/// with its owner); a new one is made at the next showing.
pub fn owner_closing(owner: HWND) {
    let hwnd = TIP.with(Cell::get);
    if hwnd.is_null() {
        return;
    }
    // SAFETY: a plain query of our own window.
    let owned_by = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetWindow(hwnd, windows_sys::Win32::UI::WindowsAndMessaging::GW_OWNER) };
    if owned_by == owner {
        TIP.with(|t| t.set(null_mut()));
        REST.with(|r| *r.borrow_mut() = Rest::default());
        // SAFETY: our own window.
        unsafe { DestroyWindow(hwnd) };
    }
}

fn paint(hwnd: HWND) {
    let mut ps = PAINTSTRUCT::default();
    // SAFETY: paired BeginPaint/EndPaint on our own window.
    let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
    SHOWS.with(|s| {
        if let Some((text, look)) = s.borrow().as_ref() {
            let (w, h) = tooltip::size(&look.regular, text);
            let mut fb = Framebuffer::new(w, h, 0);
            tooltip::paint(&mut fb, look.theme, &look.regular, text);
            let big = look.scale.enlarge(&fb);
            blit(hdc, &big, big.width);
        }
    });
    // SAFETY: see above.
    unsafe { EndPaint(hwnd, &ps) };
}

unsafe extern "system" fn tip_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_ERASEBKGND => return 1,
        WM_PAINT => {
            paint(hwnd);
            return 0;
        }
        WM_NCHITTEST => return HTTRANSPARENT as LRESULT,
        WM_MOUSEACTIVATE => return MA_NOACTIVATE as LRESULT,
        _ => {}
    }
    // SAFETY: default handling for everything else.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use super::{Rest, TipDo, step};
    use std::time::{Duration, Instant};

    /// Brief 110: a cut tab's tooltip comes after the pointer has rested the double-click time,
    /// stays ten times as long, goes when the pointer leaves the tab or a button is pressed, and
    /// does not come again on the same tab until the pointer has left it.
    #[test]
    fn a_cut_tab_s_tooltip_comes_after_a_rest_and_goes_at_a_press() {
        let wait = Duration::from_millis(500);
        let t0 = Instant::now();
        let at = |ms: u64| t0 + Duration::from_millis(ms);
        let tab = Some((7, 2));
        let mut r = Rest::default();
        assert_eq!(step(&mut r, tab, false, at(0), wait), TipDo::Nothing);
        assert_eq!(step(&mut r, tab, false, at(400), wait), TipDo::Nothing);
        assert_eq!(step(&mut r, tab, false, at(500), wait), TipDo::Show);
        assert_eq!(step(&mut r, tab, false, at(600), wait), TipDo::Nothing);
        assert_eq!(step(&mut r, tab, true, at(700), wait), TipDo::Hide, "a press hides it");
        assert_eq!(step(&mut r, tab, false, at(2000), wait), TipDo::Nothing, "not again on the same tab");
        assert_eq!(step(&mut r, None, false, at(2100), wait), TipDo::Nothing);
        assert_eq!(step(&mut r, tab, false, at(2200), wait), TipDo::Nothing);
        assert_eq!(step(&mut r, tab, false, at(2700), wait), TipDo::Show, "after leaving: again");
        assert_eq!(step(&mut r, Some((7, 3)), false, at(2800), wait), TipDo::Hide, "another tab: hidden, its own wait");
        assert_eq!(step(&mut r, Some((7, 3)), false, at(3300), wait), TipDo::Show);
        assert_eq!(step(&mut r, Some((7, 3)), false, at(7000), wait), TipDo::Nothing);
        assert_eq!(step(&mut r, Some((7, 3)), false, at(2800 + 5500), wait), TipDo::Hide, "shown its time");
    }
}
