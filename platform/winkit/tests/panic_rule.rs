//! The panic rule of every Bevel95 program (`bevel_winkit::panic`), tried in this test's own
//! process (a panic hook is the process's): a panic on a worker thread closes the program's
//! visible windows of Bevel95's class, leaves its hidden ones and other programs' kinds of window,
//! and a panic on the main thread closes nothing (that thread ends the program itself). Before,
//! a worker's panic left every window open (the safety review of 2026-11-18, suspected 2).

use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use bevel_winkit::kit::KIT_CLASS;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, IsWindow, MSG, PM_REMOVE, PeekMessageW, RegisterClassW,
    TranslateMessage, WM_CLOSE, WNDCLASSW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP, WS_VISIBLE,
};

static CLOSES: AtomicU32 = AtomicU32::new(0);

unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_CLOSE {
        CLOSES.fetch_add(1, Ordering::SeqCst);
    }
    // SAFETY: default handling (WM_CLOSE destroys the window).
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

fn register(class: &str) {
    let name = wide(class);
    let wc = WNDCLASSW {
        lpfnWndProc: Some(proc),
        // SAFETY: this program's own module.
        hInstance: unsafe { GetModuleHandleW(null()) },
        lpszClassName: name.as_ptr(),
        ..Default::default()
    };
    // SAFETY: a filled class description; the name is copied by Windows.
    unsafe { RegisterClassW(&wc) };
}

/// A 1×1 tool window far off the screen (never seen, never takes the focus).
fn window(class: &str, visible: bool) -> HWND {
    let name = wide(class);
    let style = WS_POPUP | if visible { WS_VISIBLE } else { 0 };
    // SAFETY: a registered class; no parent, menu or creation data.
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            name.as_ptr(),
            null(),
            style,
            -32000,
            -32000,
            1,
            1,
            null_mut(),
            null_mut(),
            GetModuleHandleW(null()),
            null(),
        )
    };
    assert!(!hwnd.is_null());
    hwnd
}

/// Handles this thread's messages for `time`.
fn pump(time: Duration) {
    let until = Instant::now() + time;
    let mut msg = MSG::default();
    while Instant::now() < until {
        // SAFETY: an ordinary message loop on this thread.
        unsafe {
            while PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn alive(hwnd: HWND) -> bool {
    // SAFETY: IsWindow takes any handle.
    unsafe { IsWindow(hwnd) != 0 }
}

#[test]
fn a_panic_on_a_worker_thread_closes_the_programs_windows() {
    // The log of this test goes to a scratch folder, not into Bevel95's own.
    let scratch = std::env::temp_dir().join(format!("bevel95-panic-rule-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).unwrap();
    // SAFETY: the only test of this process, before any other thread reads the environment.
    unsafe { std::env::set_var("LOCALAPPDATA", &scratch) };
    register(KIT_CLASS);
    register("Bevel95 test other class");
    bevel_winkit::panic::install(bevel_winkit::panic::close_windows_after);
    assert!(!bevel_winkit::panic::happened());

    // On the main thread (the one that installed the hook): nothing is closed.
    let first = window(KIT_CLASS, true);
    assert!(std::panic::catch_unwind(|| panic!("a test panic on the main thread")).is_err());
    pump(Duration::from_millis(300));
    assert!(bevel_winkit::panic::happened());
    assert!(alive(first), "the main thread ends the program itself");
    assert_eq!(CLOSES.load(Ordering::SeqCst), 0);

    // On a worker thread: the visible windows of Bevel95's class are closed, the others stay.
    let hidden = window(KIT_CLASS, false);
    let other = window("Bevel95 test other class", true);
    assert!(std::thread::spawn(|| panic!("a test panic on a worker thread")).join().is_err());
    pump(Duration::from_millis(500));
    assert!(!alive(first), "the window was closed as if by its Close button");
    assert_eq!(CLOSES.load(Ordering::SeqCst), 1);
    assert!(alive(hidden), "a hidden window (folders.exe's hand-over window) stays");
    assert!(alive(other), "Windows' own windows of the program stay");

    let log = std::fs::read_to_string(scratch.join("Bevel95").join("bevel95.log")).unwrap();
    assert!(log.contains("PANIC: ") && log.contains("worker thread: the program's windows are closed"), "{log}");
    // SAFETY: this thread's own windows.
    unsafe {
        DestroyWindow(hidden);
        DestroyWindow(other);
    }
    let _ = std::fs::remove_dir_all(scratch);
}
