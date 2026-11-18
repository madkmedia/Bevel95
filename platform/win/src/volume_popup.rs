//! The classic volume control window (`bevel_core::volume` draws it), opened by a click on the
//! speaker in the notification area. Moving the slider or ticking Mute changes Windows' real
//! volume at once (audio.rs), and changes made elsewhere (keyboard volume keys, other programs)
//! show while it is open. Like our menus it takes the keyboard focus, so the arrow keys work,
//! and it closes when anything else is clicked.

use std::cell::RefCell;
use std::ptr::{null, null_mut};
use std::sync::atomic::Ordering;

use bevel_core::fb::{Framebuffer, Rect};
use bevel_core::scale::upscale;
use bevel_core::volume::{self, Focus, Part, VolumeControl};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{BeginPaint, EndPaint, InvalidateRect, PAINTSTRUCT};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, VK_DOWN, VK_END, VK_ESCAPE, VK_HOME, VK_LEFT, VK_MENU, VK_NEXT, VK_PRIOR, VK_RETURN,
    VK_RIGHT, VK_SPACE, VK_TAB, VK_UP,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetForegroundWindow, IsWindow, KillTimer, PostMessageW, SW_SHOW,
    SetForegroundWindow, SetTimer, ShowWindow, WA_INACTIVE, WM_ACTIVATE, WM_CAPTURECHANGED, WM_CHAR, WM_ERASEBKGND,
    WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_PAINT, WM_SYSCHAR, WM_SYSKEYDOWN, WM_TIMER,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use crate::bar::{BAR_HWND, WM_VOLUME_CHANGED, with_app};
use crate::log::log_win32;
use crate::util::{blit, point, rect_h, rect_w, take_focus, wide};
use crate::{audio, window_list};

pub const VOLUME_CLASS: &str = "Bevel95Volume";

/// Timers: follow volume changes made elsewhere, and repeat a held click on the slider.
const POLL: usize = 1;
const POLL_MS: u32 = 250;
const REPEAT: usize = 2;
const REPEAT_FIRST_MS: u32 = 400;
const REPEAT_MS: u32 = 100;
/// How far an arrow key (or one wheel line) and a page (a click beside the thumb, Page Up and
/// Down) move the volume: a fifth for a page is Windows' slider default.
const LINE: f32 = 0.02;
const PAGE: f32 = 0.2;

struct Open {
    hwnd: HWND,
    volume: audio::Volume,
    v: VolumeControl,
    /// Dragging the thumb: where on it the pointer took hold (1× rows from its top).
    grab: Option<i32>,
    /// A click held on the slider beside the thumb, or on Mute.
    held: Option<Part>,
    /// The window that had the focus before (it gets it back after Esc).
    previous: HWND,
}

thread_local! {
    static OPEN: RefCell<Option<Open>> = const { RefCell::new(None) };
}

fn with_open<R>(f: impl FnOnce(&mut Open) -> R) -> Option<R> {
    OPEN.with(|cell| cell.try_borrow_mut().ok().and_then(|mut open| open.as_mut().map(f)))
}

/// Opens the volume control for a click at screen point `at`, where that look's Windows opens
/// it (`volume::place`).
pub fn open(owner: HWND, at: POINT) {
    close(false);
    let Some(volume) = audio::Volume::open() else {
        return;
    };
    let Some((scale, screen, era)) = with_app(|app| {
        let m = app.monitor;
        (app.scale, Rect::new(m.left, m.top, rect_w(&m), rect_h(&m)), app.era)
    }) else {
        return;
    };
    let show_keys = era.always_shows_keys();
    let (w, h) = (volume::SIZE.0 * scale, volume::SIZE.1 * scale);
    let (x, y) = volume::place(era, at.x, at.y, w, h, screen);
    let class = wide(VOLUME_CLASS);
    // SAFETY: a registered class; owned by the bar so it stays above it; the previous
    // foreground window is only remembered.
    let (hwnd, previous) = unsafe {
        let previous = GetForegroundWindow();
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
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
        );
        (hwnd, previous)
    };
    if hwnd.is_null() {
        log_win32("CreateWindowExW(volume control)");
        return;
    }
    let v = VolumeControl { level: volume.level(), muted: volume.muted(), focus: Focus::Slider, show_keys, mute_pressed: false };
    OPEN.with(|cell| *cell.borrow_mut() = Some(Open { hwnd, volume, v, grab: None, held: None, previous }));
    // SAFETY: our own window.
    unsafe {
        ShowWindow(hwnd, SW_SHOW);
        take_focus(hwnd);
        SetTimer(hwnd, POLL, POLL_MS, None);
    }
}

/// Closes the volume control (if open; returns whether it was). With `give_back_focus`, the
/// window that had the focus before gets it back (after Esc; not after a click elsewhere).
pub fn close(give_back_focus: bool) -> bool {
    let Some(open) = OPEN.with(|cell| cell.try_borrow_mut().ok().and_then(|mut o| o.take())) else {
        return false;
    };
    // SAFETY: our own window; `previous` is checked to still exist.
    unsafe {
        if give_back_focus && !open.previous.is_null() && IsWindow(open.previous) != 0 {
            SetForegroundWindow(open.previous);
        }
        DestroyWindow(open.hwnd);
    }
    true
}

/// Repaints the window and, when the mute changed, the speaker in the bar.
fn refresh(hwnd: HWND, mute_changed: bool) {
    // SAFETY: our own windows; only queues repaints.
    unsafe {
        InvalidateRect(hwnd, null(), 0);
        if mute_changed {
            PostMessageW(BAR_HWND.load(Ordering::SeqCst) as HWND, WM_VOLUME_CHANGED, 0, 0);
        }
    }
}

/// Sets the volume (clamped) and shows it.
fn set_level(hwnd: HWND, level: f32) {
    let changed = with_open(|o| {
        let level = level.clamp(0.0, 1.0);
        let changed = (o.v.level - level).abs() > f32::EPSILON;
        if changed {
            o.volume.set_level(level);
            o.v.level = level;
        }
        changed
    });
    if changed == Some(true) {
        refresh(hwnd, false);
    }
}

fn step(hwnd: HWND, by: f32) {
    if let Some(level) = with_open(|o| o.v.level) {
        set_level(hwnd, level + by);
    }
}

fn toggle_mute(hwnd: HWND) {
    with_open(|o| {
        o.v.muted = !o.v.muted;
        o.volume.set_muted(o.v.muted);
        o.v.focus = Focus::Mute;
    });
    refresh(hwnd, true);
}

/// The part of the window under a point (screen pixels from its corner).
fn part_at(x: i32, y: i32) -> Option<Part> {
    let v = with_open(|o| o.v)?;
    with_app(|app| volume::part_at(&v, &app.shell_font, x / app.scale, y / app.scale)).flatten()
}

fn scale() -> i32 {
    with_app(|app| app.scale).unwrap_or(1)
}

fn mouse_down(hwnd: HWND, x: i32, y: i32) {
    let Some(part) = part_at(x, y) else {
        return;
    };
    let s = scale();
    with_open(|o| match part {
        Part::Thumb => {
            o.grab = Some(y / s - volume::thumb(o.v.level).y);
            o.v.focus = Focus::Slider;
        }
        Part::Mute => {
            o.held = Some(Part::Mute);
            o.v.mute_pressed = true;
        }
        Part::Above | Part::Below => {
            o.held = Some(part);
            o.v.focus = Focus::Slider;
        }
    });
    match part {
        Part::Above => step(hwnd, PAGE),
        Part::Below => step(hwnd, -PAGE),
        _ => {}
    }
    // SAFETY: our own window: the pointer is followed until the button comes up.
    unsafe {
        SetCapture(hwnd);
        if matches!(part, Part::Above | Part::Below) {
            SetTimer(hwnd, REPEAT, REPEAT_FIRST_MS, None);
        }
        InvalidateRect(hwnd, null(), 0);
    }
}

fn mouse_move(hwnd: HWND, x: i32, y: i32) {
    let s = scale();
    let Some((grab, held)) = with_open(|o| (o.grab, o.held)) else {
        return;
    };
    if let Some(grab) = grab {
        set_level(hwnd, volume::level_at(y / s - grab));
    } else if held == Some(Part::Mute) {
        let inside = part_at(x, y) == Some(Part::Mute);
        if with_open(|o| std::mem::replace(&mut o.v.mute_pressed, inside) != inside) == Some(true) {
            refresh(hwnd, false);
        }
    }
}

fn mouse_up(hwnd: HWND, x: i32, y: i32) {
    let Some((held, pressed)) = with_open(|o| {
        o.grab = None;
        (o.held.take(), std::mem::replace(&mut o.v.mute_pressed, false))
    }) else {
        return;
    };
    // SAFETY: our own window and timer.
    unsafe {
        KillTimer(hwnd, REPEAT);
        ReleaseCapture();
    }
    if held == Some(Part::Mute) && pressed && part_at(x, y) == Some(Part::Mute) {
        toggle_mute(hwnd);
    } else {
        refresh(hwnd, false);
    }
}

/// A held click beside the thumb repeats while the thumb has not reached the pointer.
fn repeat(hwnd: HWND) {
    // SAFETY: our own window's timer, set again at the faster rate.
    unsafe { SetTimer(hwnd, REPEAT, REPEAT_MS, None) };
    let mut at = POINT::default();
    // SAFETY: valid out-pointers; converts to our window's coordinates.
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut at);
        windows_sys::Win32::Graphics::Gdi::ScreenToClient(hwnd, &mut at);
    }
    let held = with_open(|o| o.held).flatten();
    if matches!(held, Some(Part::Above | Part::Below)) && part_at(at.x, at.y) == held {
        step(hwnd, if held == Some(Part::Above) { PAGE } else { -PAGE });
    }
}

/// Keys, as Windows' slider and check box take them: Up and Left louder, Down and Right softer,
/// Page Up and Down a page, Home the loudest, End silent; Tab moves between the slider and Mute;
/// Space ticks Mute when it has the focus, M (with or without Alt) always; Esc and Enter close.
/// Any key shows the focus rectangle and access key (Windows 2000 hides them until then).
fn key(hwnd: HWND, vk: u16) {
    let first = with_open(|o| !std::mem::replace(&mut o.v.show_keys, true)).unwrap_or(false);
    let focus = with_open(|o| o.v.focus);
    match vk {
        VK_UP | VK_LEFT => step(hwnd, LINE),
        VK_DOWN | VK_RIGHT => step(hwnd, -LINE),
        VK_PRIOR => step(hwnd, PAGE),
        VK_NEXT => step(hwnd, -PAGE),
        VK_HOME => set_level(hwnd, 1.0),
        VK_END => set_level(hwnd, 0.0),
        VK_TAB => {
            with_open(|o| o.v.focus = if o.v.focus == Focus::Slider { Focus::Mute } else { Focus::Slider });
        }
        VK_SPACE if focus == Some(Focus::Mute) => toggle_mute(hwnd),
        VK_ESCAPE | VK_RETURN => {
            close(true);
            return;
        }
        _ => {}
    }
    if first || vk == VK_TAB {
        refresh(hwnd, false);
    }
}

fn paint(hwnd: HWND) {
    let mut ps = PAINTSTRUCT::default();
    // SAFETY: paired BeginPaint/EndPaint on our own window.
    let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
    if let Some(v) = with_open(|o| o.v) {
        with_app(|app| {
            let mut fb = Framebuffer::new(volume::SIZE.0, volume::SIZE.1, 0);
            volume::paint(&mut fb, app.theme, &app.shell_font, &v);
            let big = upscale(&fb, app.scale);
            blit(hdc, &big, big.width);
        });
    }
    // SAFETY: see above.
    unsafe { EndPaint(hwnd, &ps) };
}

/// Volume changes made elsewhere show while the window is open (not while the thumb is held).
/// If Windows refused it the focus, a click elsewhere cannot deactivate it; it closes once
/// another window comes to the front instead.
fn poll(hwnd: HWND) {
    // SAFETY: a plain query.
    let front = unsafe { GetForegroundWindow() };
    if with_open(|o| front != o.hwnd && front != o.previous).unwrap_or(false) {
        close(false);
        return;
    }
    let changed = with_open(|o| {
        if o.grab.is_some() {
            return (false, false);
        }
        let (level, muted) = (o.volume.level(), o.volume.muted());
        let changed = (o.v.level - level).abs() > 0.001 || o.v.muted != muted;
        let mute_changed = o.v.muted != muted;
        o.v.level = level;
        o.v.muted = muted;
        (changed, mute_changed)
    });
    if let Some((true, mute_changed)) = changed {
        refresh(hwnd, mute_changed);
    }
}

/// "Open Volume Controls" (and a double-click on the speaker): Windows' volume mixer.
pub fn open_mixer() {
    // Windows' own, by its whole path (the third safety review of 2026-11-18, C1).
    window_list::launch(&bevel_winkit::shell::system_program("SndVol.exe"));
}

/// "Adjust Audio Properties": Windows' Sound settings.
pub fn open_sound_settings() {
    window_list::launch(&bevel_winkit::shell::system_program("mmsys.cpl"));
}

pub unsafe extern "system" fn volume_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_ERASEBKGND => return 1,
        WM_PAINT => {
            paint(hwnd);
            return 0;
        }
        WM_LBUTTONDOWN => {
            let (x, y) = point(lparam);
            mouse_down(hwnd, x, y);
            return 0;
        }
        WM_MOUSEMOVE => {
            let (x, y) = point(lparam);
            mouse_move(hwnd, x, y);
            return 0;
        }
        WM_LBUTTONUP => {
            let (x, y) = point(lparam);
            mouse_up(hwnd, x, y);
            return 0;
        }
        WM_CAPTURECHANGED => {
            with_open(|o| {
                o.grab = None;
                o.held = None;
                o.v.mute_pressed = false;
            });
            return 0;
        }
        WM_MOUSEWHEEL => {
            let notches = f32::from((wparam >> 16) as u16 as i16) / 120.0;
            step(hwnd, notches * 3.0 * LINE);
            return 0;
        }
        WM_TIMER if wparam == POLL => {
            poll(hwnd);
            return 0;
        }
        WM_TIMER if wparam == REPEAT => {
            repeat(hwnd);
            return 0;
        }
        WM_KEYDOWN => {
            key(hwnd, wparam as u16);
            return 0;
        }
        // Alt shows the access key; Alt+M ticks Mute.
        WM_SYSKEYDOWN if wparam as u16 == VK_MENU => {
            if with_open(|o| !std::mem::replace(&mut o.v.show_keys, true)) == Some(true) {
                refresh(hwnd, false);
            }
            return 0;
        }
        WM_SYSCHAR | WM_CHAR if matches!(char::from_u32(wparam as u32), Some('m' | 'M')) => {
            with_open(|o| o.v.show_keys = true);
            toggle_mute(hwnd);
            return 0;
        }
        // Another window got the focus (a click elsewhere): close, without taking it back.
        WM_ACTIVATE if (wparam & 0xFFFF) as u32 == WA_INACTIVE => {
            close(false);
            return 0;
        }
        _ => {}
    }
    // SAFETY: default handling for everything else.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}
