//! Display Properties' Browse...: Windows 11's own window for choosing a file, a stand-in until
//! Bevel95 has each look's own file window (95's "Browsing for wallpaper" in the Windows 3.1
//! style, 98's and 2000's Open window titled "Browse").
//!
//! How it runs, and why: on a thread of its own (COM apartment-threaded, as the file window
//! wants), owned by a hidden window of that thread which stays above other windows (Display
//! Properties is one of the bar's windows, above every window; the file window must come over
//! it). Nothing of it is owned by a window of the bar's thread, so the two threads' input is never
//! joined: a file window held up by something of the shell's (a slow drive, a shell extension)
//! holds up only this thread, never the bar or the desktop. Display Properties waits behind it,
//! disabled, as behind the Pattern window, and is enabled again when the answer comes. The answer
//! (the file's full path, or nothing) comes back by a registered message with a ticket: an answer
//! for an earlier opening, or after Display Properties closed, is dropped. When the bar's look
//! changes or Display Properties closes while the file window is open, `close` asks it to go
//! (WM_CLOSE: Cancel); when the bar ends, the thread ends with the process. The process's own
//! crash from a shell extension inside the file window would be the bar's too, as for Run's
//! Browse... (which runs on the bar's thread itself).

use std::ptr::{null, null_mut};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use bevel_core::theme::Era;
use windows_sys::Win32::Foundation::{HWND, LPARAM, RECT};
use windows_sys::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, OFN_DONTADDTORECENT, OFN_EXPLORER, OFN_FILEMUSTEXIST, OFN_HIDEREADONLY, OFN_NOCHANGEDIR, OFN_PATHMUSTEXIST,
    OPENFILENAMEW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, EnumThreadWindows, GetWindowRect, PostMessageW, WM_CLOSE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use crate::log::log;
use crate::util::{Private, wide};

/// Posted to the bar when the file window has gone: wParam the ticket of its opening.
pub static WM_BROWSED: Private = Private::new("Bevel95.Bar.Browsed");

/// The ticket of the opening whose answer is wanted (0: none).
static TICKET: AtomicU64 = AtomicU64::new(0);
/// The file window's thread while it runs (0: none).
static THREAD: AtomicU32 = AtomicU32::new(0);
/// The answer of an opening: its ticket and the file's full path (None: Cancel, or nothing usable).
static ANSWER: Mutex<Option<(u64, Option<String>)>> = Mutex::new(None);

/// The kinds of file offered (the PCs' "Background Files", without the HTML pages Active Desktop
/// needed); a person's own JPEG, PNG and GIF in every look (the owner, 2026-11-21).
const PATTERNS: &str = "*.bmp;*.dib;*.jpg;*.jpeg;*.png;*.gif";

/// Each look's title and list of kinds: 95's "Browsing for wallpaper", 98's and 2000's "Browse";
/// 98's "Background Files", 2000's "Background Files" and "All Picture Files" (95 offered only
/// "Bitmaps": its words for pictures of every kind are PROVISIONAL).
pub fn title_and_filter(era: Era) -> (&'static str, String) {
    match era {
        Era::Win95 => ("Browsing for wallpaper", format!("Pictures\0{PATTERNS}\0\0")),
        Era::Win98 => ("Browse", format!("Background Files\0{PATTERNS}\0\0")),
        Era::Win2000 => ("Browse", format!("Background Files\0{PATTERNS}\0All Picture Files\0{PATTERNS}\0\0")),
    }
}

/// Whether a file window is open now.
pub fn is_open() -> bool {
    THREAD.load(Ordering::SeqCst) != 0
}

/// Opens the file window over `over` (Display Properties, already disabled by the caller) for
/// `era`; returns its ticket, or None if it could not start.
pub fn open(era: Era, over: HWND) -> Option<u64> {
    let ticket = TICKET.fetch_add(1, Ordering::SeqCst) + 1;
    let mut r = RECT::default();
    // SAFETY: a plain query of the bar's own window.
    unsafe { GetWindowRect(over, &mut r) };
    WM_BROWSED.id();
    let spawned = std::thread::Builder::new().name("browse for wallpaper".into()).spawn(move || run(era, ticket, r));
    match spawned {
        Ok(_) => Some(ticket),
        Err(e) => {
            log(&format!("Browse: could not start its thread: {e}"));
            None
        }
    }
}

/// The file window's thread.
fn run(era: Era, ticket: u64, over: RECT) {
    // SAFETY: plain query.
    THREAD.store(unsafe { windows_sys::Win32::System::Threading::GetCurrentThreadId() }, Ordering::SeqCst);
    bevel_winkit::com::init();
    let class = wide("Static");
    // A hidden owner on this thread, above other windows, where Display Properties is.
    // SAFETY: a system class; the window is this thread's own and destroyed below.
    let owner = unsafe {
        CreateWindowExW(WS_EX_TOOLWINDOW | WS_EX_TOPMOST, class.as_ptr(), null(), WS_POPUP, over.left, over.top, 0, 0, null_mut(), null_mut(), null_mut(), null())
    };
    let (title, filter) = title_and_filter(era);
    let (title, filter) = (wide(title), filter.encode_utf16().collect::<Vec<u16>>());
    // Room for the longest path Windows has.
    let mut file = vec![0u16; 32_768];
    let mut ofn = OPENFILENAMEW {
        lStructSize: size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: owner,
        lpstrFilter: filter.as_ptr(),
        lpstrFile: file.as_mut_ptr(),
        nMaxFile: file.len() as u32,
        lpstrTitle: title.as_ptr(),
        // No trace in Windows' Recent items (the review before 0.1.4, C3).
        Flags: OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_HIDEREADONLY | OFN_NOCHANGEDIR | OFN_DONTADDTORECENT,
        ..Default::default()
    };
    // Closed before it could show (Display Properties went at once): nothing opens.
    let wanted = TICKET.load(Ordering::SeqCst) == ticket;
    // SAFETY: a fully initialized struct whose buffers outlive the call.
    let chosen = wanted && unsafe { GetOpenFileNameW(&mut ofn) } != 0;
    let path = chosen.then(|| {
        let len = file.iter().position(|&c| c == 0).unwrap_or(0);
        // Never a lossy path: one Windows cannot give back as text is no path to keep.
        String::from_utf16(&file[..len]).ok()
    });
    let path = path.flatten();
    if chosen && path.is_none() {
        log("Browse: the chosen file's name is not text Bevel95 can keep");
    }
    if !owner.is_null() {
        // SAFETY: this thread's own window.
        unsafe { DestroyWindow(owner) };
    }
    if let Ok(mut a) = ANSWER.lock() {
        *a = Some((ticket, path));
    }
    THREAD.store(0, Ordering::SeqCst);
    let bar = crate::bar::BAR_HWND.load(Ordering::SeqCst) as HWND;
    if !bar.is_null() {
        // SAFETY: a message to the bar's own window; nothing waits for it.
        unsafe { PostMessageW(bar, WM_BROWSED.id(), ticket as usize, 0) };
    }
}

/// The answer of the opening `ticket`, if it is the one wanted (taken once).
pub fn answer(ticket: u64) -> Option<Option<String>> {
    if ticket != TICKET.load(Ordering::SeqCst) {
        return None;
    }
    let mut a = ANSWER.lock().ok()?;
    match a.take() {
        Some((t, path)) if t == ticket => Some(path),
        other => {
            *a = other;
            None
        }
    }
}

/// Display Properties closed (or the look changed) while the file window is open: it is asked to
/// go (Cancel), and whatever it answers is dropped.
pub fn close() {
    TICKET.fetch_add(1, Ordering::SeqCst);
    let thread = THREAD.load(Ordering::SeqCst);
    if thread == 0 {
        return;
    }
    unsafe extern "system" fn each(hwnd: HWND, _: LPARAM) -> windows_sys::core::BOOL {
        if is_our_file_window(hwnd) {
            // SAFETY: the file window of this process's own thread; nothing waits for the message.
            unsafe { PostMessageW(hwnd, WM_CLOSE, 0, 0) };
        }
        1
    }
    // SAFETY: the callback only reads about each window and posts to the file window.
    unsafe { EnumThreadWindows(thread, Some(each), 0) };
    log("Browse: its window was asked to close (Display Properties closed)");
}

/// The window class of a dialog, which the file window is.
const DIALOG_CLASS: &str = "#32770";

/// Whether `hwnd` is the file window itself: a window of this process (Windows is asked for the
/// window's process: a thread's id can be given again to a thread of another process once the
/// browse thread has ended) whose class is a dialog's. Never the hidden owner (its class is
/// "Static"), never an input method's window, never a window of another process.
fn is_our_file_window(hwnd: HWND) -> bool {
    let mut process = 0u32;
    // SAFETY: plain queries about a window; `process` and `class` are locals of the sizes given.
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(hwnd, &mut process);
        if process != windows_sys::Win32::System::Threading::GetCurrentProcessId() {
            return false;
        }
        let mut class = [0u16; 16];
        let n = windows_sys::Win32::UI::WindowsAndMessaging::GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32);
        n > 0 && String::from_utf16_lossy(&class[..n as usize]) == DIALOG_CLASS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_look_titles_its_window_and_offers_pictures() {
        for era in Era::ALL {
            let (title, filter) = title_and_filter(era);
            assert_eq!(title, if era == Era::Win95 { "Browsing for wallpaper" } else { "Browse" });
            assert!(filter.ends_with("\0\0") && filter.contains("*.bmp;*.dib;*.jpg;*.jpeg;*.png;*.gif"));
        }
        assert!(title_and_filter(Era::Win2000).1.contains("All Picture Files"));
    }

    /// The close goes only to the file window itself: a dialog of this process; never the hidden
    /// owner, never another process's window (the review of 2026-11-21, 3 and 4).
    #[test]
    fn only_our_own_dialog_is_asked_to_close() {
        let make = |class: &str| {
            let class = wide(class);
            // SAFETY: test-only hidden windows of system classes, destroyed below.
            unsafe { CreateWindowExW(WS_EX_TOOLWINDOW, class.as_ptr(), null(), WS_POPUP, 0, 0, 0, 0, null_mut(), null_mut(), null_mut(), null()) }
        };
        let (dialog, owner) = (make(DIALOG_CLASS), make("Static"));
        assert!(!dialog.is_null() && !owner.is_null());
        assert!(is_our_file_window(dialog), "our own dialog");
        assert!(!is_our_file_window(owner), "the hidden owner");
        // Windows' own desktop window belongs to another process.
        // SAFETY: plain queries.
        let desktop = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetDesktopWindow() };
        assert!(!is_our_file_window(desktop), "another process's window");
        assert!(!is_our_file_window(null_mut()), "no window");
        // SAFETY: the windows made above, on this thread.
        unsafe {
            DestroyWindow(dialog);
            DestroyWindow(owner);
        }
    }

    /// An answer is taken only for the opening wanted, once; a closed opening's answer is dropped.
    #[test]
    fn an_answer_of_an_earlier_opening_is_dropped() {
        let t = TICKET.fetch_add(1, Ordering::SeqCst) + 1;
        *ANSWER.lock().unwrap() = Some((t, Some("C:\\x.png".into())));
        assert_eq!(answer(t), Some(Some("C:\\x.png".into())));
        assert_eq!(answer(t), None, "taken once");
        *ANSWER.lock().unwrap() = Some((t, Some("C:\\y.png".into())));
        close();
        assert_eq!(answer(t), None, "after close: dropped");
    }
}
