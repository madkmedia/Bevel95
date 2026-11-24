//! The tooltip window: one small topmost window that shows a `bevel_core::tooltip` box over
//! what the pointer rests on (a notification icon, the clock). It never takes the focus and
//! clicks go through it.

use std::cell::RefCell;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicUsize, Ordering};

use bevel_core::fb::{Framebuffer, Rect};
use bevel_core::scale::Scale;
use bevel_core::tooltip;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{BeginPaint, EndPaint, InvalidateRect, PAINTSTRUCT};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, HTTRANSPARENT, HWND_TOPMOST, MA_NOACTIVATE, SW_HIDE,
    SWP_NOACTIVATE, SWP_SHOWWINDOW, SetWindowPos, ShowWindow, WM_ERASEBKGND, WM_MOUSEACTIVATE, WM_NCHITTEST,
    LWA_ALPHA, SetLayeredWindowAttributes, WM_LBUTTONDOWN, WM_MBUTTONDOWN, WM_PAINT, WM_RBUTTONDOWN, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

use crate::bar::with_app;
use crate::log::log_win32;
use crate::util::{blit, rect_h, rect_w, wide};

pub const TIP_CLASS: &str = "Bevel95Tip";

/// Windows' tooltip control puts a tooltip this far below the pointer when there is room (at
/// 1×); under the bar there never is, so ours always sit on top of what they describe.
const BELOW_POINTER: i32 = 20;

static TIP: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static TEXT: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Shows `text` for the pointer at `pointer` over `tool` (both in screen pixels).
pub fn show(owner: HWND, text: &str, pointer: POINT, tool: Rect) {
    show_placed(owner, text, &|w, h, screen, work, _, _, scale| {
        tooltip::place((pointer.x, pointer.y), w, h, tool, screen, work, scale.len(BELOW_POINTER), scale.len(1))
    });
}

/// Shows a Quick Launch icon's tooltip: where each look put it (`tooltip::place_quick_launch`).
pub fn show_quick_launch(owner: HWND, text: &str, pointer: POINT, tool: Rect) {
    show_placed(owner, text, &|w, h, screen, _, era, bar_top, scale| {
        tooltip::place_quick_launch(era, (pointer.x, pointer.y), w, h, tool, bar_top, screen, scale.len(1), scale)
    });
}

type Placer<'a> = dyn Fn(i32, i32, Rect, Rect, bevel_core::theme::Era, i32, Scale) -> (i32, i32) + 'a;

fn show_placed(owner: HWND, text: &str, place: &Placer) {
    let Some((w, h, screen, work, era, bar_top, scale)) = with_app(|app| {
        let (w, h) = tooltip::size(app.fonts().regular, text);
        let m = app.monitor;
        let screen = Rect::new(m.left, m.top, rect_w(&m), rect_h(&m));
        let work = Rect::new(m.left, m.top, rect_w(&m), app.bar_top_on_screen() - m.top);
        (app.scale.len(w), app.scale.len(h), screen, work, app.era, app.bar_top_on_screen(), app.scale)
    }) else {
        return;
    };
    TEXT.with(|t| *t.borrow_mut() = text.to_string());
    let (x, y) = place(w, h, screen, work, era, bar_top, scale);
    let mut hwnd = TIP.load(Ordering::SeqCst) as HWND;
    if hwnd.is_null() {
        let class = wide(TIP_CLASS);
        // SAFETY: a registered class; owned by the bar so it stays above it.
        hwnd = unsafe {
            CreateWindowExW(
                // Layered and transparent: Windows' hit test passes it by for every program and thread,
                // so a press over it reaches the window under it (brief 90; HTTRANSPARENT alone passes a
                // press on only to windows of the bar's own thread).
                TIP_EX_STYLE,
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
            log_win32("CreateWindowExW(tooltip)");
            return;
        }
        // Fully opaque: it looks as before (a layered window shows nothing until this is set).
        // SAFETY: our own window.
        if unsafe { SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA) } == 0 {
            log_win32("SetLayeredWindowAttributes(tooltip)");
        }
        TIP.store(hwnd as usize, Ordering::SeqCst);
    }
    // SAFETY: our own window, shown without taking the focus.
    unsafe {
        SetWindowPos(hwnd, HWND_TOPMOST, x, y, w, h, SWP_NOACTIVATE | SWP_SHOWWINDOW);
        InvalidateRect(hwnd, null(), 0);
    }
    // Under the Monitor Lines at once (brief 102).
    crate::monitor_lines::raise();
}

/// Hides the tooltip (if one is showing).
pub fn hide() {
    let hwnd = TIP.load(Ordering::SeqCst) as HWND;
    if !hwnd.is_null() {
        // SAFETY: our own window.
        unsafe { ShowWindow(hwnd, SW_HIDE) };
    }
}

/// Destroys the tooltip window (when the bar closes).
pub fn destroy() {
    let hwnd = TIP.swap(0, Ordering::SeqCst) as HWND;
    if !hwnd.is_null() {
        // SAFETY: our own window.
        unsafe { DestroyWindow(hwnd) };
    }
}

fn paint(hwnd: HWND) {
    let mut ps = PAINTSTRUCT::default();
    // SAFETY: paired BeginPaint/EndPaint on our own window.
    let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
    let text = TEXT.with(|t| t.borrow().clone());
    with_app(|app| {
        let font = app.fonts().regular;
        let (w, h) = tooltip::size(font, &text);
        let mut fb = Framebuffer::new(w, h, 0);
        tooltip::paint(&mut fb, app.theme, font, &text);
        let big = app.scale.enlarge(&fb);
        blit(hdc, &big, big.width);
    });
    // SAFETY: see above.
    unsafe { EndPaint(hwnd, &ps) };
}

/// The tooltip window's extended style (see show_placed).
const TIP_EX_STYLE: u32 = WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT | WS_EX_LAYERED;

pub unsafe extern "system" fn tip_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_ERASEBKGND => return 1,
        WM_PAINT => {
            paint(hwnd);
            return 0;
        }
        // Clicks and the pointer go to whatever is under the tooltip.
        WM_NCHITTEST => return HTTRANSPARENT as LRESULT,
        WM_MOUSEACTIVATE => return MA_NOACTIVATE as LRESULT,
        // Should never come (the window is passed by); if it does, the log says so.
        WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN => {
            static TAKEN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            bevel_winkit::log::counted(&TAKEN, "tooltip: a press landed on the tooltip and went nowhere");
            return 0;
        }
        _ => {}
    }
    // SAFETY: default handling for everything else.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use std::ptr::{null, null_mut};
    use std::sync::mpsc;

    use windows_sys::Win32::Foundation::{HWND, POINT};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, HWND_TOPMOST, LWA_ALPHA, MSG,
        PostThreadMessageW, RegisterClassW, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetLayeredWindowAttributes, SetWindowPos,
        WM_QUIT, WNDCLASSW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, WindowFromPoint,
    };

    use super::TIP_EX_STYLE;
    use crate::util::wide;

    /// Far outside every screen: the test's windows are shown there, where nobody sees them (the
    /// review of brief 90: nothing on the owner's screen).
    const AWAY: i32 = -30000;

    /// A plain window of this test's own, shown (off every screen) without taking the front.
    fn window(class: &str, ex: u32, x: i32, y: i32) -> HWND {
        let name = wide(class);
        // SAFETY: a class of the test's own with the default procedure; a window of it.
        unsafe {
            let wc = WNDCLASSW { lpfnWndProc: Some(DefWindowProcW), hInstance: GetModuleHandleW(null()), lpszClassName: name.as_ptr(), ..Default::default() };
            RegisterClassW(&wc);
            let hwnd = CreateWindowExW(ex, name.as_ptr(), null(), WS_POPUP, x, y, 60, 30, null_mut(), null_mut(), GetModuleHandleW(null()), null());
            SetWindowPos(hwnd, HWND_TOPMOST, x, y, 60, 30, SWP_NOACTIVATE | SWP_SHOWWINDOW);
            hwnd
        }
    }

    /// The bar's tooltip lets a press through to the window under it, also when that window is
    /// another thread's (brief 90): Windows' hit test passes a layered, transparent window by.
    /// Proven with windows of this test's own, off every screen: one on a thread of its own, the
    /// tooltip's look-alike above it; and, so that the test cannot pass for nothing, a window of
    /// the same kind without the two styles above the same window elsewhere, which Windows does
    /// find. (It no longer shows the old style's fault: that style answered HTTRANSPARENT to the
    /// hit test, which a test window of its own would need its own procedure to imitate.)
    #[test]
    fn a_press_over_the_tooltip_reaches_the_window_under_it() {
        let (x, y) = (AWAY, AWAY);
        let (tx, rx) = mpsc::channel();
        let under = std::thread::spawn(move || {
            let hwnd = window("Bevel95TestUnderTip", WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE, x, y);
            let wide_under = window("Bevel95TestUnderPlain", WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE, x + 100, y);
            // SAFETY: the thread's id, for the quit message.
            tx.send((hwnd as usize, wide_under as usize, unsafe { windows_sys::Win32::System::Threading::GetCurrentThreadId() })).unwrap();
            let mut msg = MSG::default();
            // SAFETY: this thread's own message loop.
            unsafe {
                while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
                    DispatchMessageW(&msg);
                }
                DestroyWindow(hwnd);
                DestroyWindow(wide_under);
            }
        });
        let (under_hwnd, _, thread) = rx.recv().unwrap();
        let tip = window("Bevel95TestTip", TIP_EX_STYLE, x, y);
        // SAFETY: the test's own window.
        unsafe { SetLayeredWindowAttributes(tip, 0, 255, LWA_ALPHA) };
        let plain = window("Bevel95TestPlainAbove", WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE, x + 100, y);
        // SAFETY: plain queries and our own windows.
        let (through, found) = unsafe {
            (WindowFromPoint(POINT { x: x + 30, y: y + 15 }), WindowFromPoint(POINT { x: x + 130, y: y + 15 }))
        };
        unsafe {
            DestroyWindow(tip);
            DestroyWindow(plain);
            PostThreadMessageW(thread, WM_QUIT, 0, 0);
        }
        under.join().unwrap();
        assert_eq!(through as usize, under_hwnd, "the press goes through the tooltip to the window under it");
        assert_eq!(found, plain, "a window without the tooltip's styles is found there (the test is not empty)");
    }
}