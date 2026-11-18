//! The watchdog: a second, windowless copy of bevel95.exe that waits for the bar's process
//! to end, then always gives back the bar's screen space and restores the Windows taskbar.
//! It covers what the bar cannot do for itself: being killed outright (Task Manager's "End
//! task") or crashing too hard to clean up. It also carries out restarts, so the old bar is
//! completely gone before the new one starts.
//!
//! It is started through a short-lived "launcher" copy, so it is not a child of the bar:
//! Task Manager groups child processes under their parent app, and "End task" on that group
//! would kill the watchdog along with the bar.

use std::ptr::null;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, HWND, STILL_ACTIVE, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows_sys::Win32::System::Threading::{
    CREATE_BREAKAWAY_FROM_JOB, CreateProcessW, GetExitCodeProcess, OpenEventW, OpenMutexW, OpenProcess, PROCESS_INFORMATION,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, STARTUPINFOW, SYNCHRONIZATION_SYNCHRONIZE, WaitForSingleObject,
};
use windows_sys::Win32::UI::WindowsAndMessaging::IsWindow;

use crate::log::{log, log_win32};
use crate::util::wide;
use crate::{appbar, explorer};

/// The exit code with which the bar asks the watchdog to start a fresh bar ("Restart").
pub const RESTART_EXIT_CODE: u32 = 95;

/// The command line that starts `exe` with `args`; a portable copy's own starts are flagged
/// (`log::PORTABLE_FLAG`), so a Restart's new bar and the watchdog keep its data beside it.
fn command_line(exe: &std::path::Path, args: &str, portable: bool) -> String {
    let flag = if portable { format!(" {}", crate::log::PORTABLE_FLAG) } else { String::new() };
    format!("\"{}\" {args}{flag}", exe.display())
}

/// Starts this program again with `args`. Returns its process handle (to close) and id.
pub fn spawn(args: &str) -> Option<(HANDLE, u32)> {
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => {
            log(&format!("cannot find bevel95.exe: {e}"));
            return None;
        }
    };
    let mut command_line = wide(&command_line(&exe, args, crate::log::is_portable_run()));
    let startup = STARTUPINFOW { cb: size_of::<STARTUPINFOW>() as u32, ..Default::default() };
    let mut info = PROCESS_INFORMATION::default();
    // Breaking away from the starter's job (a terminal may run us in one) keeps the new
    // process alive when that job closes. Not every job allows it, so retry without.
    for flags in [CREATE_BREAKAWAY_FROM_JOB, 0] {
        // SAFETY: a mutable, null-terminated command line and valid in/out structs.
        let ok = unsafe {
            CreateProcessW(
                null(),
                command_line.as_mut_ptr(),
                null(),
                null(),
                0,
                flags,
                null(),
                null(),
                &startup,
                &mut info,
            )
        };
        if ok != 0 {
            // SAFETY: we do not need the thread handle.
            unsafe { CloseHandle(info.hThread) };
            return Some((info.hProcess, info.dwProcessId));
        }
    }
    log_win32(&format!("starting `bevel95 {args}`"));
    None
}

/// Called by the bar: starts the launcher, which starts the watchdog and reports its process
/// id as its exit code.
pub fn start(bar_pid: u32, bar_hwnd: HWND) -> Option<u32> {
    let (launcher, _) = spawn(&format!("--watchdog-launch {bar_pid} {}", bar_hwnd as usize))?;
    let mut code = 0;
    // SAFETY: a process handle we own, closed right after.
    unsafe {
        if WaitForSingleObject(launcher, 5000) == WAIT_OBJECT_0 {
            GetExitCodeProcess(launcher, &mut code);
        }
        CloseHandle(launcher);
    }
    if code == 0 || code == STILL_ACTIVE as u32 {
        log("the watchdog did not start");
        return None;
    }
    log(&format!("watchdog started (pid {code})"));
    Some(code)
}

/// `--watchdog-launch <bar pid> <bar window>`: start the watchdog, then exit with its id.
pub fn launch(bar_pid: u32, bar_hwnd: usize) -> u32 {
    match spawn(&format!("--watchdog {bar_pid} {bar_hwnd}")) {
        Some((handle, pid)) => {
            // SAFETY: a handle we own.
            unsafe { CloseHandle(handle) };
            pid
        }
        None => 0,
    }
}

/// What the watchdog saw when its wait ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Seen {
    /// The bar's process ended, with this exit code.
    Ended(u32),
    /// The bar's window is gone but its process still runs: the bar ended the ordinary way and
    /// gave up its place (`bar::give_up_place`, before its window went), and the process only
    /// finishes a file job now. `restart`: the bar's restart mark was up.
    GaveUp { restart: bool },
}

impl Seen {
    /// Whether a new bar is to be started now.
    fn restart(self) -> bool {
        matches!(self, Seen::Ended(RESTART_EXIT_CODE) | Seen::GaveUp { restart: true })
    }
}

/// One look of the watchdog's wait: the process ended (its exit code), else whether the bar's
/// window is still there and whether its restart mark is up. `None`: go on waiting.
fn look(ended: Option<u32>, window_alive: bool, restart_marked: impl FnOnce() -> bool) -> Option<Seen> {
    match ended {
        Some(code) => Some(Seen::Ended(code)),
        None if !window_alive => Some(Seen::GaveUp { restart: restart_marked() }),
        None => None,
    }
}

/// Whether the bar process `pid` has put up its restart mark.
fn restart_marked(pid: u32) -> bool {
    let name = wide(&crate::bar::restart_mark_name(pid));
    // SAFETY: a null-terminated name; a handle opened is closed at once.
    unsafe {
        let mark = OpenEventW(SYNCHRONIZATION_SYNCHRONIZE, 0, name.as_ptr());
        if mark.is_null() {
            return false;
        }
        CloseHandle(mark);
        true
    }
}

/// `--watchdog <bar pid> <bar window>`: wait for the bar's process to end, or for its window to
/// go while the process finishes a file job (the second safety review of 2026-11-18, B1: the
/// watchdog waited for the process, so a Restart's new bar came only when the job was done, and
/// with a window of Windows' left open never), then clean up.
pub fn run(bar_pid: u32, bar_hwnd: usize) -> u32 {
    let mut seen = Seen::Ended(0);
    // SAFETY: an ordinary wait on a process handle we own, closed after use; IsWindow takes any
    // handle.
    unsafe {
        let process = OpenProcess(PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, 0, bar_pid);
        if process.is_null() {
            log_win32("watchdog: OpenProcess(bar)");
        } else {
            seen = loop {
                let ended = (WaitForSingleObject(process, 100) == WAIT_OBJECT_0).then(|| {
                    let mut code = 0;
                    GetExitCodeProcess(process, &mut code);
                    code
                });
                if let Some(seen) = look(ended, IsWindow(bar_hwnd as HWND) != 0, || restart_marked(bar_pid)) {
                    break seen;
                }
            };
            CloseHandle(process);
        }
    }
    appbar::remove(bar_hwnd as HWND);
    explorer::restore();
    // A bar killed while switched to Windows' taskbar leaves its notification icon there:
    // Windows' taskbar takes it away when the pointer passes over it, and refuses to be asked
    // sooner (a delete naming the dead window fails: tried 2026-11-17,
    // private/probes/tray-icon/dead-icon.ps1). A bar the watchdog restarts shows its own bar and
    // adds no icon, so there is never a second one.
    // Programs send their notification icons to Windows' taskbar again (it has them already
    // unless the bar ended in the middle of passing one on).
    crate::tray::broadcast_taskbar_created();
    match seen {
        Seen::Ended(code) => log(&format!("watchdog: the bar ended (exit code {code}); Windows taskbar restored")),
        Seen::GaveUp { restart } => log(&format!(
            "watchdog: the bar's window is gone and its place is free ({}); its process still finishes a file job and \
             ends by itself; Windows taskbar restored",
            if restart { "a Restart" } else { "not a Restart" }
        )),
    }
    if seen.restart()
        && let Some((handle, pid)) = spawn("")
    {
        // SAFETY: a handle we own.
        unsafe { CloseHandle(handle) };
        log(&format!("watchdog: started a new bar (pid {pid})"));
    }
    0
}

/// True if process `pid` is still running.
pub fn is_running(pid: u32) -> bool {
    // SAFETY: a zero-timeout wait on a handle we own, closed after use.
    unsafe {
        let process = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        if process.is_null() {
            return false;
        }
        let running = WaitForSingleObject(process, 0) == WAIT_TIMEOUT;
        CloseHandle(process);
        running
    }
}

/// `--after <pid>`: a Restart without a watchdog starts the new bar first, and it must not
/// overlap with the old one. Waits (at most 10 s) until the old bar's process has ended or it
/// has given up its place (the bar's mutex is free: its windows are gone, it may still finish a
/// file job), so the new bar starts at once (the second safety review of 2026-11-18, B1: it
/// waited for the process, 10 s, then met the mutex and gave up for good).
pub fn wait_for_place(pid: u32) {
    let waited = wait_for_place_named(pid, crate::bar::PLACE_NAME, std::time::Duration::from_secs(10));
    if !waited.0 {
        log(&format!("--after: the old bar still holds its place after {} ms", waited.1.as_millis()));
    }
}

/// Waits at most `limit` until process `pid` has ended or no one holds the place `name` (a named
/// mutex). Returns whether that came, and how long it took.
fn wait_for_place_named(pid: u32, name: &str, limit: std::time::Duration) -> (bool, std::time::Duration) {
    let started = std::time::Instant::now();
    let name = wide(name);
    // SAFETY: a process handle and mutex handles we own, each closed after use.
    unsafe {
        let process = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        loop {
            let ended = process.is_null() || WaitForSingleObject(process, 0) == WAIT_OBJECT_0;
            let held = OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, 0, name.as_ptr());
            let free = held.is_null();
            if !free {
                CloseHandle(held);
            }
            if ended || free || started.elapsed() >= limit {
                if !process.is_null() {
                    CloseHandle(process);
                }
                return (ended || free, started.elapsed());
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The watchdog's look: a process that ended is "ended" with its code; a window gone while
    /// the process runs is a bar that gave up its place, and a Restart's new bar starts at once.
    /// Before, only the process's end counted.
    #[test]
    fn a_bar_that_gave_up_its_place_is_restarted_at_once() {
        assert_eq!(look(None, true, || unreachable!()), None, "the bar runs");
        assert_eq!(look(Some(0), true, || unreachable!()), Some(Seen::Ended(0)));
        assert!(look(Some(RESTART_EXIT_CODE), false, || false).unwrap().restart());
        let finishing = look(None, false, || true).unwrap();
        assert_eq!(finishing, Seen::GaveUp { restart: true });
        assert!(finishing.restart(), "a Restart while a file job is finished: the new bar now");
        assert!(!look(None, false, || false).unwrap().restart(), "Exit while a file job is finished");
    }

    /// A portable copy's Restart (`--after`), its watchdog and the watchdog's new bar are flagged
    /// as portable; the installed copy's are not (the third safety review of 2026-11-18, A1: the
    /// started program guessed from a settings.txt that could be held for a moment). The flag is
    /// taken out again by `main` before the arguments are read.
    #[test]
    fn a_portable_copys_own_starts_are_flagged() {
        let exe = std::path::Path::new("D:\\Tools\\Bevel95\\bevel95.exe");
        let flagged = command_line(exe, "--after 42", true);
        assert_eq!(flagged, format!("\"D:\\Tools\\Bevel95\\bevel95.exe\" --after 42 {}", crate::log::PORTABLE_FLAG));
        assert_eq!(command_line(exe, "--after 42", false), "\"D:\\Tools\\Bevel95\\bevel95.exe\" --after 42");
        let mut args: Vec<String> = flagged.split(' ').skip(1).map(String::from).collect();
        assert!(crate::log::take_portable_flag(&mut args));
        assert_eq!(args, ["--after", "42"]);
        // The watchdog's new bar: no argument but the flag.
        let mut args: Vec<String> = command_line(exe, "", true).split(' ').skip(1).filter(|s| !s.is_empty()).map(String::from).collect();
        assert!(crate::log::take_portable_flag(&mut args) && args.is_empty());
    }

    /// `--after` waits for the place, not for the process: a process that still runs (this test's
    /// own) but whose place is free lets the new bar start at once. Before, it waited the full 10 s
    /// for the process, then met the mutex and exited.
    #[test]
    fn after_starts_once_the_place_is_free() {
        let name = format!("Local\\Bevel95.Test.Place.{}", std::process::id());
        let me = std::process::id();
        let wide_name = wide(&name);
        // SAFETY: a named mutex made and closed by this test.
        let held = unsafe { windows_sys::Win32::System::Threading::CreateMutexW(null(), 0, wide_name.as_ptr()) };
        assert!(!held.is_null());
        let (came, _) = wait_for_place_named(me, &name, std::time::Duration::from_millis(300));
        assert!(!came, "held, and the process runs: no start");
        let held = held as usize;
        let freeing = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(400));
            // SAFETY: the handle made above.
            unsafe { CloseHandle(held as HANDLE) };
        });
        let (came, took) = wait_for_place_named(me, &name, std::time::Duration::from_secs(10));
        freeing.join().unwrap();
        assert!(came, "the place came free");
        assert!(took < std::time::Duration::from_secs(3), "at once, not after the limit: {took:?}");
    }
}
