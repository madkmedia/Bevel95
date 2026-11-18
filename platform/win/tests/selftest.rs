//! End-to-end safety check of the real program: the Windows taskbar must come back, with its
//! auto-hide setting as it was, however Bevel95 ends: switched off, restarted, killed the
//! hard way (no chance to clean up: only the watchdog can), closed by `--restore`, or with
//! the session (sign-out, restart, shut down).
//!
//! It really hides your Windows taskbar for a few seconds, so it only runs on request:
//! `cargo selftest`. Close Bevel95 first.

use std::path::PathBuf;
use bevel_winkit::quiet::command;
use std::ptr::null;
use std::time::{Duration, Instant};

use bevel_core::bar_menu::BarCommand;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::Shell::{ABM_GETSTATE, ABS_AUTOHIDE, APPBARDATA, SHAppBarMessage};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, FindWindowW, GetPropW, IsWindowVisible, PostMessageW, RegisterWindowMessageW, SendMessageW, WM_ENDSESSION,
    WM_HOTKEY, WM_QUERYENDSESSION,
};

/// `bar::WM_BAR_COMMAND`, registered by its name (it was `WM_APP + 3` before the fourth safety
/// review of 2026-11-18, C3).
fn wm_bar_command() -> u32 {
    let name = wide("Bevel95.Bar.Command");
    // SAFETY: a null-terminated name.
    let id = unsafe { RegisterWindowMessageW(name.as_ptr()) };
    assert_ne!(id, 0, "the bar's command message could not be registered");
    id
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

fn find(class: &str) -> HWND {
    let class = wide(class);
    // SAFETY: a valid class name.
    unsafe { FindWindowW(class.as_ptr(), null()) }
}

fn our_bar() -> Option<HWND> {
    let bar = find("Bevel95Bar");
    // SAFETY: IsWindowVisible accepts any handle.
    (!bar.is_null() && unsafe { IsWindowVisible(bar) } != 0).then_some(bar)
}

/// Windows' taskbar window: the first "Shell_TrayWnd" that is not Bevel95's hidden stand-in
/// for the notification area (marked with the window property `Bevel95.StandIn`).
fn windows_taskbar() -> HWND {
    let (class, mark) = (wide("Shell_TrayWnd"), wide("Bevel95.StandIn"));
    let mut after: HWND = std::ptr::null_mut();
    loop {
        // SAFETY: valid strings; `after` is null or a window this loop just found.
        unsafe {
            after = FindWindowExW(std::ptr::null_mut(), after, class.as_ptr(), null());
            if after.is_null() || GetPropW(after, mark.as_ptr()).is_null() {
                return after;
            }
        }
    }
}

fn windows_taskbar_visible() -> bool {
    let tray = windows_taskbar();
    // SAFETY: as above.
    !tray.is_null() && unsafe { IsWindowVisible(tray) } != 0
}

fn windows_taskbar_state() -> u32 {
    let mut abd =
        APPBARDATA { cbSize: size_of::<APPBARDATA>() as u32, hWnd: windows_taskbar(), ..Default::default() };
    // SAFETY: a correctly sized APPBARDATA.
    unsafe { SHAppBarMessage(ABM_GETSTATE, &mut abd) as u32 }
}

/// The bar built for the tests keeps its data in the tests' scratch folder (decisions.md,
/// "Milestone 1 decisions", the tests never write the real data folder): the same place
/// `bevel_winkit::log::installed_dir` gives here.
fn state_file() -> PathBuf {
    bevel_winkit::log::installed_dir().join("windows-taskbar-state.txt")
}

fn wait_until(what: &str, check: impl Fn() -> bool) {
    let start = Instant::now();
    while !check() {
        assert!(start.elapsed() < Duration::from_secs(15), "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
    println!("ok ({:.1}s): {what}", start.elapsed().as_secs_f32());
}

/// Ours is showing (with its notification area's stand-in in front of Windows' taskbar
/// window), the Windows taskbar is hidden and on auto-hide, its setting is saved.
fn ours_is_showing() -> bool {
    our_bar().is_some()
        && find("Shell_TrayWnd") != windows_taskbar()
        && !windows_taskbar_visible()
        && windows_taskbar_state() & ABS_AUTOHIDE != 0
        && state_file().exists()
}

/// The Windows taskbar shows, with its setting as it was and nothing left to put back.
fn windows_taskbar_is_back(original: u32) -> bool {
    windows_taskbar_visible() && windows_taskbar_state() == original && !state_file().exists()
}

/// The Windows taskbar is back exactly as it was, and nothing of ours is left over (no bar, no
/// notification-area stand-in for programs to send their icons to, and no desktop of ours over
/// Windows' own).
fn windows_is_back(original: u32) -> impl Fn() -> bool {
    move || {
        our_bar().is_none()
            && find("Shell_TrayWnd") == windows_taskbar()
            && find("Bevel95Desktop").is_null()
            && windows_taskbar_is_back(original)
    }
}

/// Puts everything back when a check fails half-way (it panics): a bar left running would keep
/// the Windows taskbar hidden (and keep the test's output open).
struct RestoreIfFailed(&'static str);

impl Drop for RestoreIfFailed {
    fn drop(&mut self) {
        if std::thread::panicking() {
            let _ = command(self.0).arg("--restore").status();
        }
    }
}

#[test]
#[ignore = "hides your Windows taskbar for a few seconds; run it with `cargo selftest`"]
fn the_windows_taskbar_always_comes_back() {
    let exe = env!("CARGO_BIN_EXE_bevel95");
    let _restore = RestoreIfFailed(exe);
    assert!(find("Bevel95Bar").is_null(), "close Bevel95 before running the self-test");
    assert!(windows_taskbar_visible(), "the Windows taskbar should be showing before the test");
    assert!(!state_file().exists(), "a previous run left {} behind; run `bevel95 --restore`", state_file().display());
    let original = windows_taskbar_state();
    println!("Windows taskbar state before: {original}");

    let mut first = command(exe).env("BEVEL95_NO_SETUP", "1").spawn().expect("start bevel95");
    wait_until("the bar starts and hides the Windows taskbar", ours_is_showing);

    // Ctrl+Win+T, twice (posted straight to the bar: same handler as the real hotkey).
    let bar = our_bar().expect("bar window");
    // SAFETY: posting to the bar's window.
    unsafe { PostMessageW(bar, WM_HOTKEY, 1, 0) };
    wait_until("the hotkey switches to the Windows taskbar", windows_is_back(original));
    // SAFETY: as above.
    unsafe { PostMessageW(bar, WM_HOTKEY, 1, 0) };
    wait_until("the hotkey switches back to Bevel95", ours_is_showing);

    // Restart from the menu: the old bar must be gone before the new one starts.
    let restart = BarCommand::Restart.code();
    // SAFETY: as above.
    unsafe { PostMessageW(bar, wm_bar_command(), restart, 0) };
    let _ = first.wait();
    wait_until("Restart brings up a new bar", || ours_is_showing() && find("Bevel95Bar") != bar);

    // Killed the hard way (Task Manager's End task): only the watchdog can clean up.
    let restarted_pid = {
        let mut pid = 0u32;
        // SAFETY: a valid window and out-pointer.
        unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(find("Bevel95Bar"), &mut pid) };
        pid
    };
    assert!(restarted_pid != 0);
    let killed = command("taskkill").args(["/F", "/PID", &restarted_pid.to_string()]).output().expect("taskkill");
    assert!(killed.status.success(), "taskkill failed: {}", String::from_utf8_lossy(&killed.stderr));
    wait_until("the watchdog restores the Windows taskbar after a hard kill", windows_is_back(original));

    // `bevel95 --restore` closes a running bar and restores.
    let mut second = command(exe).env("BEVEL95_NO_SETUP", "1").spawn().expect("start bevel95");
    wait_until("the bar starts again", ours_is_showing);

    // Signing out or shutting down: Windows first asks every program whether the session may
    // end. The Windows taskbar must come back at once and stay back (the bar's 100 ms check
    // must not hide it again before the end), and go away again if the session goes on.
    let bar = our_bar().expect("bar window");
    // SAFETY: messages to the bar's window; neither carries a pointer.
    let answer = unsafe { SendMessageW(bar, WM_QUERYENDSESSION, 0, 0) };
    assert_eq!(answer, 1, "the bar should let the session end");
    wait_until("the session may end: the Windows taskbar is back", || windows_taskbar_is_back(original));
    std::thread::sleep(Duration::from_secs(1));
    assert!(windows_taskbar_is_back(original), "the Windows taskbar was hidden again while the session was ending");
    println!("ok: it stays back while the session is ending");
    // SAFETY: as above.
    unsafe { SendMessageW(bar, WM_ENDSESSION, 0, 0) };
    wait_until("the session goes on: the bar hides the Windows taskbar again", ours_is_showing);

    let status = command(exe).arg("--restore").status().expect("run --restore");
    assert!(status.success());
    wait_until("--restore brings the Windows taskbar back", windows_is_back(original));
    let _ = second.wait();
}
