//! The tooltip window: one small topmost window that shows a `bevel_core::tooltip` box over
//! what the pointer rests on (a notification icon, the clock). It never takes the focus and
//! clicks go through it.

use std::cell::RefCell;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicUsize, Ordering};

use bevel_core::fb::{Framebuffer, Rect};
use bevel_core::scale::upscale;
use bevel_core::tooltip;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{BeginPaint, EndPaint, InvalidateRect, PAINTSTRUCT};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, HTTRANSPARENT, HWND_TOPMOST, MA_NOACTIVATE, SW_HIDE,
    SWP_NOACTIVATE, SWP_SHOWWINDOW, SetWindowPos, ShowWindow, WM_ERASEBKGND, WM_MOUSEACTIVATE, WM_NCHITTEST,
    WM_PAINT, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
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
        tooltip::place((pointer.x, pointer.y), w, h, tool, screen, work, BELOW_POINTER * scale, scale)
    });
}

/// Shows a Quick Launch icon's tooltip: where each look put it (`tooltip::place_quick_launch`).
pub fn show_quick_launch(owner: HWND, text: &str, pointer: POINT, tool: Rect) {
    show_placed(owner, text, &|w, h, screen, _, era, bar_top, scale| {
        tooltip::place_quick_launch(era, (pointer.x, pointer.y), w, h, tool, bar_top, screen, scale, scale)
    });
}

type Placer<'a> = dyn Fn(i32, i32, Rect, Rect, bevel_core::theme::Era, i32, i32) -> (i32, i32) + 'a;

fn show_placed(owner: HWND, text: &str, place: &Placer) {
    let Some((w, h, screen, work, era, bar_top, scale)) = with_app(|app| {
        let (w, h) = tooltip::size(app.fonts().regular, text);
        let m = app.monitor;
        let screen = Rect::new(m.left, m.top, rect_w(&m), rect_h(&m));
        let work = Rect::new(m.left, m.top, rect_w(&m), app.bar_top_on_screen() - m.top);
        (w * app.scale, h * app.scale, screen, work, app.era, app.bar_top_on_screen(), app.scale)
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
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT,
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
        TIP.store(hwnd as usize, Ordering::SeqCst);
    }
    // SAFETY: our own window, shown without taking the focus.
    unsafe {
        SetWindowPos(hwnd, HWND_TOPMOST, x, y, w, h, SWP_NOACTIVATE | SWP_SHOWWINDOW);
        InvalidateRect(hwnd, null(), 0);
    }
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
        let big = upscale(&fb, app.scale);
        blit(hdc, &big, big.width);
    });
    // SAFETY: see above.
    unsafe { EndPaint(hwnd, &ps) };
}

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
        _ => {}
    }
    // SAFETY: default handling for everything else.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}
