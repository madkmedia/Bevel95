//! Bevel95's file jobs outlive their window (the safety review of 2026-11-18, A2): a copy, move,
//! rename, delete or Undo runs on a thread of its own (`file_ops::on_thread`), and a program that
//! ended while one ran cut it short (a half-written file, a folder half in each place, no Undo
//! step). Every such job is counted here from the moment it is asked for until its thread is done
//! (also when it panics). Windows' progress window of a copy and its questions belong to that
//! thread (they show while its copy runs) and are covered by it. Nothing else counts: a
//! Properties window or another dialog of Windows' closes with the program, as it did before
//! 2026-11-18 (the second safety review of 2026-11-18, B1: counting them kept a program's
//! process, and with it the bar's place, for as long as such a window was left open). A program
//! asked to end first lets its own windows go, then waits here (`wait_while_busy`, its messages
//! still handled) until no job runs any more, and only then ends.

use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicUsize, Ordering};

use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::System::Threading::{CreateEventW, OpenEventW, SYNCHRONIZATION_SYNCHRONIZE};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, MSG, MsgWaitForMultipleObjects, PM_REMOVE, PeekMessageW, QS_ALLINPUT, TranslateMessage,
};

static RUNNING: AtomicUsize = AtomicUsize::new(0);

/// One running job: counted while this lives (made before its thread starts, dropped when the
/// thread's work is done or unwinds).
pub(crate) struct Running(&'static AtomicUsize);

impl Running {
    pub(crate) fn start() -> Running {
        Running::on(&RUNNING)
    }

    fn on(counter: &'static AtomicUsize) -> Running {
        counter.fetch_add(1, Ordering::SeqCst);
        Running(counter)
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// How many file jobs run in this program now.
pub fn running() -> usize {
    RUNNING.load(Ordering::SeqCst)
}

/// Whether a file job runs in this program.
pub fn busy() -> bool {
    running() > 0
}

/// The name of the mark a program without windows holds while it finishes its file jobs: Setup
/// and `tools\install.ps1` read it, so they say "still finishing a copy or move" only when that
/// is true (the second safety review of 2026-11-18, B1).
pub fn finishing_name(pid: u32) -> String {
    format!("Local\\Bevel95.FinishingFileJobs.{pid}")
}

/// Puts up this program's mark "finishing file jobs" (`finishing_name`); it stays until the
/// program ends.
pub fn mark_finishing() {
    let name: Vec<u16> = finishing_name(std::process::id()).encode_utf16().chain([0]).collect();
    // SAFETY: a null-terminated name; the handle is kept open on purpose until the process ends.
    if unsafe { CreateEventW(null(), 1, 1, name.as_ptr()) }.is_null() {
        crate::log::log_win32("CreateEventW(the mark of a program finishing its file jobs)");
    }
}

/// Whether process `pid` holds the mark "finishing file jobs".
pub fn is_finishing(pid: u32) -> bool {
    let name: Vec<u16> = finishing_name(pid).encode_utf16().chain([0]).collect();
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

/// Handles this thread's messages while `busy`, so whatever windows it still has keep answering.
/// Returns true as soon as `stop` says so (a folder window was opened again meanwhile), false
/// once nothing is busy any more.
pub fn wait_while_busy(stop: impl Fn() -> bool) -> bool {
    let mut msg = MSG::default();
    loop {
        if stop() {
            return true;
        }
        if !busy() {
            return false;
        }
        // SAFETY: a plain wait and the standard handling of this thread's messages. A WM_QUIT
        // that comes meanwhile is let go: the program is already ending.
        unsafe {
            MsgWaitForMultipleObjects(0, null_mut(), 0, 200, QS_ALLINPUT);
            while PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A job is counted from the moment it is asked for until its thread is done, also when it
    /// panics; the wait ends when the last one ends.
    #[test]
    fn jobs_are_counted_until_their_threads_are_done() {
        // A counter of the test's own (the other tests start jobs of their own meanwhile).
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        let (go, wait) = std::sync::mpsc::channel::<()>();
        let job = Running::on(&COUNT);
        assert_eq!(COUNT.load(Ordering::SeqCst), 1);
        let t = std::thread::spawn(move || {
            let _job = job;
            wait.recv().unwrap();
        });
        let panicking = Running::on(&COUNT);
        let p = std::thread::spawn(move || {
            let _job = panicking;
            panic!("a job that panics");
        });
        assert!(p.join().is_err());
        assert_eq!(COUNT.load(Ordering::SeqCst), 1, "a panicking job is no longer counted");
        go.send(()).unwrap();
        t.join().unwrap();
        assert_eq!(COUNT.load(Ordering::SeqCst), 0);
    }

    /// The mark "finishing file jobs" is seen for the program that put it up, and only for it.
    #[test]
    fn the_finishing_mark_is_seen_by_process() {
        let me = std::process::id();
        assert!(!is_finishing(me), "no mark before it is put up");
        mark_finishing();
        assert!(is_finishing(me));
        assert!(!is_finishing(me ^ 0x40), "another process's number");
    }
}
