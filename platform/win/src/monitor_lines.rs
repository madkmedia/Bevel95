//! Monitor Lines (brief 102): one see-through window over the bar's screen that darkens every n-th
//! screen row (`bevel_core::monitor_lines` makes its picture and says when there is one). The
//! window is layered and transparent, so Windows' hit test passes it by for every program and
//! thread: it never takes a click, a key or the focus, and it is never a task button. It is
//! filled once with `UpdateLayeredWindow` (per-pixel alpha, premultiplied) and kept above every
//! window of Bevel95's own copy: where the bar shows or raises one of its own, `raise` puts the
//! lines back at once (also through `bevel_winkit::on_top`, for the kit's windows, a program's
//! menus and the dragged picture, and for the companion programs, which ask the bar by its
//! message); other programs' always-on-top windows are put back under it on the bar's tick
//! (`keep_on_top`), and one that insists is given up on after a few times: the lines then stand
//! just under it and above everything else (`bevel_core::monitor_lines::{place, Fights}`, brief
//! 104), logged counted, never with its title or class.
//! The window is the bar's process's: when the bar ends, it ends, and nothing is left to restore.

use std::cell::RefCell;
use std::ptr::{null, null_mut};
use std::sync::Once;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::time::Instant;

use bevel_core::monitor_lines::{self as lines, Fights, Place, Strength};
use windows_sys::Win32::Foundation::{HANDLE, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, CreateCompatibleDC, CreateDIBSection,
    DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::{GetCurrentProcessId, GetCurrentThreadId};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GW_HWNDPREV, GetWindow, GetWindowRect, GetWindowThreadProcessId,
    HTTRANSPARENT, HWND_TOPMOST, IsIconic, IsWindow, IsWindowVisible, MA_NOACTIVATE, PostMessageW, RegisterClassExW,
    RemovePropW, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE, SetPropW,
    SetWindowPos, ShowWindow, ULW_ALPHA, UpdateLayeredWindow, WM_MOUSEACTIVATE, WM_NCHITTEST, WNDCLASSEXW, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

use crate::log::{log, log_win32};
use crate::util::wide;

/// The window's class (a test or a script finds it by it, with the bar's process).
pub const CLASS: &str = "Bevel95MonitorLines";

/// Its extended style: layered and transparent (every click, the pointer and the keys pass it by),
/// above every window, no task button, never activated.
pub const EX_STYLE: u32 = WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;

/// The property that tells Windows' shell a full-screen topmost window is not a full-screen
/// program (it would otherwise count it as one, as it counts a video).
const NON_RUDE: &str = "NonRudeHWND";

/// The window while it shows (0: none). Read by `raise` from any of the bar's threads.
static WINDOW: AtomicUsize = AtomicUsize::new(0);

/// What the lines are to cover: the bar's screen (screen pixels), every how many rows, how dark.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wanted {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
    pub period: u32,
    pub strength: Strength,
}

thread_local! {
    /// What the window shows now (`None`: no window), or what failed to show (kept, so a failure
    /// is not tried again at every tick: the next change tries again).
    static NOW: RefCell<Option<Wanted>> = const { RefCell::new(None) };
    static FIGHTS: RefCell<Fights> = RefCell::new(Fights::default());
    static EPOCH: Instant = Instant::now();
}

/// Set when the bar's window goes (`end`): no lines from then on in this process.
static ENDED: AtomicBool = AtomicBool::new(false);

/// The bar's window is going: the lines go, and no later call brings them back.
pub fn end() {
    ENDED.store(true, Ordering::SeqCst);
    set(None);
}

/// The lines' window while it shows (null: none).
pub fn window() -> HWND {
    WINDOW.load(Ordering::SeqCst) as HWND
}

/// Whether `hwnd` is the lines' window (the window list skips it).
pub fn is_ours(hwnd: HWND) -> bool {
    !hwnd.is_null() && WINDOW.load(Ordering::SeqCst) == hwnd as usize
}

/// Shows the lines as `wanted` says, or takes them away (`None`). Called on the bar's thread
/// wherever what decides them may have changed (and on every tick): nothing happens when nothing
/// changed.
pub fn set(wanted: Option<Wanted>) {
    // Once the bar's window is going, never again.
    let wanted = wanted.filter(|_| !ENDED.load(Ordering::SeqCst));
    if NOW.with(|n| *n.borrow() == wanted) {
        return;
    }
    let before = NOW.with(|n| n.replace(wanted));
    let window = WINDOW.load(Ordering::SeqCst) as HWND;
    let Some(w) = wanted else {
        if !window.is_null() {
            destroy(window);
            log("monitor lines: off");
        }
        return;
    };
    let window = if window.is_null() {
        let Some(made) = create(w.left, w.top, w.width, w.height) else {
            return;
        };
        WINDOW.store(made as usize, Ordering::SeqCst);
        made
    } else {
        window
    };
    if !fill(window, &w) {
        // Nothing half-shown stays: the window goes (the change after the next tries again).
        WINDOW.store(0, Ordering::SeqCst);
        destroy(window);
        return;
    }
    if !visible(window) {
        // SAFETY: our own window, shown without taking the focus or the front.
        unsafe { ShowWindow(window, SW_SHOWNOACTIVATE) };
    }
    raise();
    log(&format!(
        "monitor lines: {}{} on a screen of {}x{}, every {} rows ({} MB for the picture while it is made)",
        if before.is_some() { "now " } else { "" },
        w.strength.setting(),
        w.width,
        w.height,
        w.period,
        lines::bytes(w.width, w.height).div_ceil(1 << 20)
    ));
}

/// The window, made (not yet shown).
fn create(left: i32, top: i32, width: i32, height: i32) -> Option<HWND> {
    register_class();
    let class = wide(CLASS);
    // SAFETY: a registered class; no owner (the bar's place among the windows does not move it),
    // no title, no menu.
    let hwnd = unsafe {
        CreateWindowExW(EX_STYLE, class.as_ptr(), null(), WS_POPUP, left, top, width, height, null_mut(), null_mut(), GetModuleHandleW(null()), null())
    };
    if hwnd.is_null() {
        log_win32("CreateWindowExW(monitor lines)");
        return None;
    }
    let name = wide(NON_RUDE);
    // SAFETY: our own window; a property holding the number 1, taken away before it goes.
    if unsafe { SetPropW(hwnd, name.as_ptr(), 1 as HANDLE) } == 0 {
        log_win32("SetPropW(monitor lines, NonRudeHWND)");
    }
    Some(hwnd)
}

/// Takes the window away.
fn destroy(hwnd: HWND) {
    if WINDOW.load(Ordering::SeqCst) == hwnd as usize {
        WINDOW.store(0, Ordering::SeqCst);
    }
    let name = wide(NON_RUDE);
    // SAFETY: our own window and its property.
    unsafe {
        RemovePropW(hwnd, name.as_ptr());
        DestroyWindow(hwnd);
    }
}

fn visible(hwnd: HWND) -> bool {
    // SAFETY: a plain query.
    unsafe { IsWindowVisible(hwnd) != 0 }
}

/// Fills `hwnd` with the picture for `w` and puts it over the screen `w` names: a picture of the
/// screen's size made once (`lines::fill`), handed to Windows, and let go again.
fn fill(hwnd: HWND, w: &Wanted) -> bool {
    if w.width <= 0 || w.height <= 0 {
        return false;
    }
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w.width,
            biHeight: -w.height, // rows top to bottom: the screen's top row first
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..Default::default()
        },
        ..Default::default()
    };
    // SAFETY: a DIB of width x height 32-bit pixels, written only within its size, selected into a
    // memory DC for the one call and freed after it; the screen DC is released.
    unsafe {
        let screen = GetDC(null_mut());
        let dc = CreateCompatibleDC(screen);
        let mut bits: *mut core::ffi::c_void = null_mut();
        let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        if bitmap.is_null() || bits.is_null() {
            log_win32("CreateDIBSection(monitor lines)");
            // A bitmap that came back without its bits is freed too (brief 104, N1).
            if !bitmap.is_null() {
                DeleteObject(bitmap);
            }
            DeleteDC(dc);
            ReleaseDC(null_mut(), screen);
            return false;
        }
        let count = w.width as usize * w.height as usize;
        let pixels = std::slice::from_raw_parts_mut(bits.cast::<u32>(), count);
        lines::fill(pixels, w.width as usize, w.period, w.strength);
        let old = SelectObject(dc, bitmap);
        let at = POINT { x: w.left, y: w.top };
        let size = SIZE { cx: w.width, cy: w.height };
        let src = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: AC_SRC_ALPHA as u8 };
        let ok = UpdateLayeredWindow(hwnd, screen, &at, &size, dc, &src, 0, &blend, ULW_ALPHA) != 0;
        if !ok {
            log_win32("UpdateLayeredWindow(monitor lines)");
        }
        SelectObject(dc, old);
        DeleteObject(bitmap);
        DeleteDC(dc);
        ReleaseDC(null_mut(), screen);
        ok
    }
}

/// Puts the lines where they belong after one of Bevel95's own windows came up or forward: above
/// it and every window but those given up on (`bevel_core::monitor_lines::place`), counting
/// nothing. On the lines' thread at once; from another thread (the desktop's, a drag's) the bar
/// is asked by its message (`on_top::WM_RAISE_LINES`), as Bevel95's companion programs ask it.
pub fn raise() {
    let hwnd = WINDOW.load(Ordering::SeqCst) as HWND;
    if hwnd.is_null() {
        return;
    }
    let mut process = 0;
    // SAFETY: plain queries; the window is checked to be still this process's.
    let thread = unsafe { GetWindowThreadProcessId(hwnd, &mut process) };
    // SAFETY: plain queries.
    if thread == 0 || process != unsafe { GetCurrentProcessId() } {
        return;
    }
    // SAFETY: a plain query.
    if thread == unsafe { GetCurrentThreadId() } {
        place_lines(hwnd, false);
    } else {
        let bar = crate::bar::BAR_HWND.load(Ordering::SeqCst) as HWND;
        if !bar.is_null() {
            // SAFETY: posting a message without pointers to the bar's own window.
            unsafe { PostMessageW(bar, bevel_winkit::on_top::WM_RAISE_LINES.id(), 0, 0) };
        }
    }
}

/// The bar's tick: the windows found above the lines on their screen are counted (another
/// program's that insists is given up on: `Fights`), and the lines go where they belong.
pub fn keep_on_top() {
    let hwnd = WINDOW.load(Ordering::SeqCst) as HWND;
    if !hwnd.is_null() {
        place_lines(hwnd, true);
    }
}

/// Reads the windows above the lines (front first), decides with `Fights` (counting on the tick
/// only) and moves the lines' window there: on top of every window, or just under the lowest
/// window given up on, or nowhere new. On the lines' own thread.
fn place_lines(hwnd: HWND, count: bool) {
    let Some(w) = NOW.with(|n| *n.borrow()) else {
        return;
    };
    let screen = RECT { left: w.left, top: w.top, right: w.left + w.width, bottom: w.top + w.height };
    let above = windows_above(hwnd, &screen);
    let now = EPOCH.with(|e| e.elapsed().as_millis() as u64);
    let Ok((place, gave_up)) = FIGHTS.with(|f| {
        let mut f = f.try_borrow_mut()?;
        // A mark ends when its window is gone or hidden (a few windows: only those given up on).
        for id in f.given_up_ids() {
            // SAFETY: plain queries; any number.
            let shows = unsafe { IsWindow(id as HWND) != 0 && IsWindowVisible(id as HWND) != 0 };
            if !shows {
                f.forget(id);
            }
        }
        Ok::<_, std::cell::BorrowMutError>(if count {
            let v = f.tick(&above, now);
            (v.place, v.gave_up)
        } else {
            (f.place_now(&above), None)
        })
    }) else {
        return;
    };
    let after = match place {
        Place::Stay => None,
        Place::Top => Some(HWND_TOPMOST),
        Place::Under(id) => Some(id as HWND),
    };
    if let Some(after) = after {
        // SAFETY: our own window; only its place among the windows changes.
        unsafe { SetWindowPos(hwnd, after, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER) };
    }
    if let Some(n) = gave_up {
        static SAID: AtomicU32 = AtomicU32::new(0);
        let said = SAID.fetch_add(1, Ordering::Relaxed) + 1;
        if said <= 5 || said.is_multiple_of(50) {
            log(&format!(
                "monitor lines: a window of another program came back above the lines {} times in a row; the lines stay just under it while it shows (windows left so far: {n})",
                lines::FIGHTS_BEFORE_GIVING_UP + 1
            ));
        }
    }
}

/// The windows above `lines` that show on `screen`, front first: `None` for a window of Bevel95's
/// own copy (the bar's process, or a program file of the bar's own folder: `Own`), else its
/// handle.
fn windows_above(lines: HWND, screen: &RECT) -> Vec<Option<u64>> {
    let mut found = Vec::new();
    // SAFETY: plain queries on any window handle.
    unsafe {
        let mut above = GetWindow(lines, GW_HWNDPREV);
        let mut steps = 0;
        while !above.is_null() && steps < 1000 {
            if shows_on(above, screen) {
                found.push(if is_own(above) { None } else { Some(above as u64) });
            }
            above = GetWindow(above, GW_HWNDPREV);
            steps += 1;
        }
    }
    found.reverse(); // front first
    found
}

thread_local! {
    /// Windows already told apart (`is_own`), by handle: the process they belong to and whether it
    /// is a program of the bar's own copy. A window never changes its process, so the answer holds
    /// while the window's process is the same one.
    static OWN: RefCell<std::collections::HashMap<u64, (u32, bool)>> = RefCell::new(std::collections::HashMap::new());
}

/// Whether `hwnd` belongs to Bevel95's own copy: the bar's process, or a process whose program
/// file is in the bar's own folder (`bevel_core::monitor_lines::copy_folder`): never by a class
/// name, so another copy's programs and a stranger's window of the same class are not ours.
fn is_own(hwnd: HWND) -> bool {
    let mut pid = 0;
    // SAFETY: plain queries.
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    // SAFETY: a plain query.
    if pid == unsafe { GetCurrentProcessId() } {
        return true;
    }
    if pid == 0 {
        return false;
    }
    if let Some(own) = OWN.with(|o| o.borrow().get(&(hwnd as u64)).filter(|(p, _)| *p == pid).map(|(_, own)| *own)) {
        return own;
    }
    let own = match (bevel_winkit::on_top::process_path(pid), std::env::current_exe()) {
        (Some(theirs), Ok(ours)) => lines::copy_folder(&theirs) == lines::copy_folder(&ours.to_string_lossy()),
        _ => false,
    };
    OWN.with(|o| {
        let mut o = o.borrow_mut();
        if o.len() >= 64 {
            o.clear();
        }
        o.insert(hwnd as u64, (pid, own));
    });
    own
}

/// A window that can be seen on `screen`: visible, not minimized or cloaked, not empty, and
/// overlapping it.
///
/// # Safety
/// Only read-only queries; any window handle.
unsafe fn shows_on(hwnd: HWND, screen: &RECT) -> bool {
    // SAFETY: as above.
    unsafe {
        if IsWindowVisible(hwnd) == 0 || IsIconic(hwnd) != 0 {
            return false;
        }
        let mut r = RECT::default();
        if GetWindowRect(hwnd, &mut r) == 0 || r.right <= r.left || r.bottom <= r.top {
            return false;
        }
        if r.right <= screen.left || r.left >= screen.right || r.bottom <= screen.top || r.top >= screen.bottom {
            return false;
        }
    }
    !crate::window_list::is_cloaked(hwnd)
}

fn register_class() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let class = wide(CLASS);
        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(lines_proc),
            // SAFETY: a plain query.
            hInstance: unsafe { GetModuleHandleW(null()) },
            lpszClassName: class.as_ptr(),
            ..Default::default()
        };
        // SAFETY: a class description that lives through the call.
        if unsafe { RegisterClassExW(&wc) } == 0 {
            log_win32("RegisterClassExW(monitor lines)");
        }
    });
}

/// Windows' hit test passes the window by (layered and transparent); should a click or the
/// pointer still be asked about, it goes to what is under it, and the window is never activated.
unsafe extern "system" fn lines_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_NCHITTEST => HTTRANSPARENT as LRESULT,
        WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
        // SAFETY: default handling for everything else.
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GW_HWNDNEXT, GWL_EXSTYLE, GWL_STYLE, GetPropW, GetWindowLongPtrW, RegisterClassW, SWP_SHOWWINDOW, WNDCLASSW,
        WindowFromPoint,
    };

    /// Far outside every screen: the test's windows are shown there, where nobody sees them.
    const AWAY: i32 = -30000;

    /// A plain topmost window of the test's own (a stand-in for a menu or another program's
    /// always-on-top window), shown off every screen without taking the front.
    fn plain(class: &str, x: i32, y: i32) -> HWND {
        let name = wide(class);
        // SAFETY: a class of the test's own with the default procedure; a window of it.
        unsafe {
            let wc = WNDCLASSW { lpfnWndProc: Some(DefWindowProcW), hInstance: GetModuleHandleW(null()), lpszClassName: name.as_ptr(), ..Default::default() };
            RegisterClassW(&wc);
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                name.as_ptr(),
                null(),
                WS_POPUP,
                x,
                y,
                64,
                64,
                null_mut(),
                null_mut(),
                GetModuleHandleW(null()),
                null(),
            );
            SetWindowPos(hwnd, HWND_TOPMOST, x, y, 64, 64, SWP_NOACTIVATE | SWP_SHOWWINDOW);
            hwnd
        }
    }

    /// Whether `lower` stands below `upper` in the order of windows.
    fn below(lower: HWND, upper: HWND) -> bool {
        // SAFETY: plain queries.
        unsafe {
            let mut next = GetWindow(upper, GW_HWNDNEXT);
            while !next.is_null() {
                if next == lower {
                    return true;
                }
                next = GetWindow(next, GW_HWNDNEXT);
            }
        }
        false
    }

    /// The window as the bar makes it, shown off every screen: its styles, a click at its middle
    /// reaches the window under it, it is no task button, and `place_lines` puts it above a
    /// topmost window of the process shown after it (a menu: Bevel95's own).
    #[test]
    fn the_lines_window_takes_nothing_and_goes_on_top() {
        let (x, y) = (AWAY, AWAY + 200);
        let under = plain("Bevel95TestUnderLines", x, y);
        let w = Wanted { left: x, top: y, width: 64, height: 64, period: 2, strength: Strength::Strong };
        let hwnd = create(w.left, w.top, w.width, w.height).expect("made");
        assert!(fill(hwnd, &w), "filled");
        // What this test thread's lines cover (the bar keeps it on its own thread).
        NOW.with(|n| *n.borrow_mut() = Some(w));
        // SAFETY: plain queries and the test's own windows.
        unsafe {
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            place_lines(hwnd, false);
            let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
            let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
            assert_eq!(ex & EX_STYLE, EX_STYLE, "layered, transparent, topmost, tool window, never activated");
            assert_ne!(style & WS_POPUP, 0);
            assert!(!GetPropW(hwnd, wide(NON_RUDE).as_ptr()).is_null(), "not a full-screen program to the shell");
            assert_eq!(WindowFromPoint(POINT { x: x + 32, y: y + 32 }), under, "a click goes to the window under the lines");
            assert!(below(under, hwnd));
            assert!(!crate::window_list::belongs_on_taskbar(hwnd), "no task button");
            // A topmost window shown after the lines stands above them, until they are raised.
            let menu = plain("Bevel95TestMenuOverLines", x, y);
            assert!(below(hwnd, menu));
            place_lines(hwnd, false);
            assert!(below(menu, hwnd), "the lines back on top");
            // The tick too, and the menu is Bevel95's own: never counted.
            place_lines(hwnd, true);
            assert!(FIGHTS.with(|f| f.borrow().is_empty()));
            NOW.with(|n| *n.borrow_mut() = None);
            DestroyWindow(menu);
            destroy(hwnd);
            DestroyWindow(under);
            assert_eq!(IsWindow(hwnd), 0, "no window left");
        }
    }

    /// The window list skips the lines' window by what it is too (it has no title, and is a tool
    /// window, so it never was one).
    #[test]
    fn the_window_list_never_lists_the_lines() {
        let hwnd = create(AWAY, AWAY + 400, 8, 8).expect("made");
        WINDOW.store(hwnd as usize, Ordering::SeqCst);
        assert!(is_ours(hwnd));
        assert!(!is_ours(null_mut()));
        destroy(hwnd);
        assert!(!is_ours(hwnd), "gone with the window");
    }
}
