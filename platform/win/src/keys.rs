//! The Windows key and Ctrl+Esc open our Start menu instead of Windows' own, while our bar
//! is showing. Combinations (Win+E, Win+R, ...) keep working.
//!
//! A low-level keyboard hook watches the keys. When the Windows key is pressed and released
//! with nothing in between, the release is held back; the hook's thread then sends an
//! unassigned key (0xE8) followed by the release, so Windows sees "Windows key + something" and
//! does not open its own menu, and the bar opens ours. (The same trick AutoHotkey and
//! Open-Shell use.) The hook's thread sends it, not the bar: while the bar is busy, Windows
//! would think the key still down and turn a typed letter into a Win+letter shortcut (the code
//! review of 2026-11-09).
//! Ctrl+Esc is swallowed and opens ours too (Ctrl+Shift+Esc still opens Task Manager). Nothing
//! is changed outside this process: the hook goes away when the bar exits.
//!
//! The hook runs on a thread of its own: Windows silently removes a low-level hook that keeps
//! the keyboard waiting longer than about a second, and the bar's thread is sometimes busy that
//! long (reading a big Programs folder's icons).

use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_CONTROL, VK_ESCAPE,
    VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, HC_ACTION, KBDLLHOOKSTRUCT, LLKHF_INJECTED, MSG, PM_NOREMOVE, PeekMessageW,
    PostMessageW, PostThreadMessageW, SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYUP, WM_QUIT,
    WM_SYSKEYUP,
};

use crate::log::log_win32;

/// Posted to the bar: open (or close) the Start menu. wParam: the Windows key that was
/// released (its release already sent on by the hook's thread), or 0 for Ctrl+Esc.
/// Registered by name (Bevel95.Bar.StartKey, the fourth safety review of 2026-11-18, C3).
pub static WM_START_KEY: crate::util::Private = crate::util::Private::new("Bevel95.Bar.StartKey");
/// Posted to the bar: Esc was pressed while a task button is being dragged (`set_dragging`):
/// the button goes back. The bar never has the keyboard focus, so it hears Esc only here.
/// Registered by name (Bevel95.Bar.DragEsc).
pub static WM_DRAG_ESC: crate::util::Private = crate::util::Private::new("Bevel95.Bar.DragEsc");
/// Posted by the hook to its own thread: send the mask key and the held-back release of the
/// Windows key in wParam (right after the hook returns).
const WM_RELEASE_WIN: u32 = windows_sys::Win32::UI::WindowsAndMessaging::WM_APP + 1;

/// An unassigned virtual key, sent so the lone Windows key press is not lone any more.
const MASK_KEY: u16 = 0xE8;

/// The hook's thread (0 while there is none).
static THREAD: AtomicU32 = AtomicU32::new(0);
static BAR: AtomicUsize = AtomicUsize::new(0);
/// Our bar is showing (otherwise every key goes to Windows untouched).
static ACTIVE: AtomicBool = AtomicBool::new(false);
static WIN_DOWN: AtomicBool = AtomicBool::new(false);
static OTHER_KEY: AtomicBool = AtomicBool::new(false);
static ESC_SWALLOWED: AtomicBool = AtomicBool::new(false);
/// A task button is being dragged: Esc is the bar's (it puts the button back).
static DRAGGING: AtomicBool = AtomicBool::new(false);

/// A task button's drag begins or ends: while it lasts, Esc goes to the bar and not to the
/// window in front.
pub fn set_dragging(on: bool) {
    DRAGGING.store(on, Ordering::SeqCst);
}

/// Starts watching the keyboard (once), on the hook's own thread.
pub fn install(bar: HWND) {
    BAR.store(bar as usize, Ordering::SeqCst);
    if THREAD.load(Ordering::SeqCst) != 0 {
        return;
    }
    let (sender, receiver) = std::sync::mpsc::channel();
    let spawned = std::thread::Builder::new().name("keyboard hook".into()).spawn(move || {
        let mut msg = MSG::default();
        // SAFETY: `hook` stays valid for the life of the program; our own module handle. The
        // peek gives this thread its message queue before anyone can post to it; the loop is
        // what Windows calls the hook through.
        unsafe {
            let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook), GetModuleHandleW(null_mut()), 0);
            if hook.is_null() {
                log_win32("SetWindowsHookExW(WH_KEYBOARD_LL): the Windows key will open Windows' Start menu");
            }
            PeekMessageW(&mut msg, null_mut(), 0, 0, PM_NOREMOVE);
            let _ = sender.send(GetCurrentThreadId());
            while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
                if msg.hwnd.is_null() && msg.message == WM_RELEASE_WIN {
                    release_windows_key(msg.wParam as u16);
                }
            }
            if !hook.is_null() {
                UnhookWindowsHookEx(hook);
            }
        }
    });
    match spawned {
        Ok(_) => THREAD.store(receiver.recv().unwrap_or(0), Ordering::SeqCst),
        Err(e) => crate::log::log(&format!("could not start the keyboard hook's thread: {e}")),
    }
}

/// Stops watching the keyboard (the hook's thread ends and removes the hook).
pub fn uninstall() {
    let thread = THREAD.swap(0, Ordering::SeqCst);
    if thread != 0 {
        // SAFETY: a message to the hook's own thread.
        unsafe { PostThreadMessageW(thread, WM_QUIT, 0, 0) };
    }
}

/// Whether the keys are ours to take (our bar is showing).
pub fn set_active(active: bool) {
    ACTIVE.store(active, Ordering::SeqCst);
    WIN_DOWN.store(false, Ordering::SeqCst);
}

/// Sends the masking key and then the held-back release of Windows key `vk` (on the hook's
/// thread).
fn release_windows_key(vk: u16) {
    let key = |vk: u16, up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: if up { KEYEVENTF_KEYUP } else { 0 },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let inputs = [key(MASK_KEY, false), key(MASK_KEY, true), key(vk, true)];
    // SAFETY: a valid array of fully initialized keyboard inputs.
    let sent = unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), size_of::<INPUT>() as i32) };
    if sent as usize != inputs.len() {
        log_win32("SendInput (releasing the Windows key)");
    }
}

fn post(vk: u32) {
    // SAFETY: posting to our own bar window (a stale handle just fails).
    unsafe { PostMessageW(BAR.load(Ordering::SeqCst) as HWND, WM_START_KEY.id(), vk as usize, 0) };
}

/// Kept tiny: Windows drops hooks that keep the keyboard waiting.
unsafe extern "system" fn hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && ACTIVE.load(Ordering::Relaxed) {
        // SAFETY: for HC_ACTION, lParam points to a KBDLLHOOKSTRUCT.
        let k = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
        // Esc during a task button's drag, typed or sent by a program (a test's): the bar's.
        // Its release is swallowed too.
        if k.vkCode == u32::from(VK_ESCAPE) {
            let up = matches!(wparam as u32, WM_KEYUP | WM_SYSKEYUP);
            if !up && DRAGGING.swap(false, Ordering::Relaxed) {
                ESC_SWALLOWED.store(true, Ordering::Relaxed);
                // SAFETY: posting to our own bar window (a stale handle just fails).
                unsafe { PostMessageW(BAR.load(Ordering::SeqCst) as HWND, WM_DRAG_ESC.id(), 0, 0) };
                return 1;
            }
            if up && ESC_SWALLOWED.swap(false, Ordering::Relaxed) {
                return 1;
            }
        }
        if k.flags & LLKHF_INJECTED == 0 {
            let up = matches!(wparam as u32, WM_KEYUP | WM_SYSKEYUP);
            let vk = k.vkCode as u16;
            // SAFETY: a plain query. Inside the hook it still gives the state from before this
            // key event.
            let held = |key: u16| unsafe { GetAsyncKeyState(i32::from(key)) } < 0;
            match vk {
                // A fresh press (not the key repeating): also when the release was missed, as
                // on the lock screen, where this hook sees nothing.
                VK_LWIN | VK_RWIN if !up => {
                    if !WIN_DOWN.swap(true, Ordering::Relaxed) || !held(vk) {
                        OTHER_KEY.store(false, Ordering::Relaxed);
                    }
                }
                VK_LWIN | VK_RWIN => {
                    if WIN_DOWN.swap(false, Ordering::Relaxed) && !OTHER_KEY.load(Ordering::Relaxed) {
                        // held back: this thread sends it on after the mask key, as soon as the
                        // hook has returned
                        // SAFETY: a message to this thread's own queue.
                        unsafe { PostThreadMessageW(GetCurrentThreadId(), WM_RELEASE_WIN, vk as usize, 0) };
                        post(k.vkCode);
                        return 1;
                    }
                }
                // Ctrl+Esc, not Ctrl+Shift+Esc (Task Manager) or Ctrl+Alt+Esc.
                VK_ESCAPE
                    if !up && !WIN_DOWN.load(Ordering::Relaxed) && held(VK_CONTROL) && !held(VK_SHIFT) && !held(VK_MENU) =>
                {
                    ESC_SWALLOWED.store(true, Ordering::Relaxed);
                    post(0);
                    return 1;
                }
                VK_ESCAPE if up && ESC_SWALLOWED.swap(false, Ordering::Relaxed) => return 1,
                _ if !up && WIN_DOWN.load(Ordering::Relaxed) => OTHER_KEY.store(true, Ordering::Relaxed),
                _ => {}
            }
        }
    }
    // SAFETY: passing the event on to the rest of the chain.
    unsafe { CallNextHookEx(null_mut(), code, wparam, lparam) }
}
