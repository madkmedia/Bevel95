//! Dragging files with Windows' own drag-and-drop interface (OLE), so that files go between
//! Bevel95's windows (the desktop in the bar's program, the folder windows in another) and to
//! and from every other program that takes files, by one way.
//!
//! - A window that takes files registers a drop target (`register`): a small COM object written
//!   out by hand (windows-sys has no interfaces), which hands every step of a drag over it to the
//!   window's own function (`Handler`) on the window's thread. It reads only the file names
//!   (CF_HDROP) from what is dragged; anything else (text, a web picture) is refused.
//! - A drag that starts in Bevel95 (`run`) gives Windows the files as the shell's own data object
//!   (SHCreateDataObject on their item lists, as Explorer does), with a drop source of our own:
//!   Esc or the other button cancels, the look's pointers show the mark (`bevel_core::drag`), and
//!   a see-through window of our own carries the look's dragged picture beside the pointer.
//! - A drop on a Bevel95 window is carried out by that window, through Windows' file machinery
//!   (`file_ops::drop_files`). A move it does itself is reported to the source as "nothing", so a
//!   source program never deletes its files afterwards (the shell's "optimized move").
//!
//! Every object made for a drag is released when it ends; the picture's window, its bitmap and
//! its device context go with it.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::c_void;
use std::path::PathBuf;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicU32, Ordering};

use bevel_core::drag::{self, Effect, Keys, Offered};
use bevel_core::fb::Framebuffer;
use bevel_core::scale::Scale;
use bevel_core::theme::Era;
use windows_sys::Win32::Foundation::{
    DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, DV_E_FORMATETC, E_NOINTERFACE, E_POINTER, HANDLE, HWND, POINT, POINTL, S_OK, SIZE, WAIT_OBJECT_0,
};
use windows_sys::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, CreateBitmap, CreateCompatibleDC, CreateDIBSection,
    DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, HBITMAP, HDC, HGDIOBJ, ReleaseDC, ScreenToClient, SelectObject,
};
use windows_sys::Win32::System::Com::{CoTaskMemFree, DVASPECT_CONTENT, FORMATETC, STGMEDIUM, TYMED_HGLOBAL};
use windows_sys::Win32::System::Ole::{
    CF_HDROP, DROPEFFECT_COPY, DROPEFFECT_LINK, DROPEFFECT_MOVE, DROPEFFECT_NONE, DoDragDrop, OleInitialize, RegisterDragDrop,
    ReleaseStgMedium, RevokeDragDrop,
};
use windows_sys::Win32::UI::Shell::Common::ITEMIDLIST;
use windows_sys::Win32::UI::Shell::{
    CSIDL_DESKTOP, DragQueryFileW, HDROP, ILClone, ILCreateFromPathW, ILFindLastID, ILFree, ILIsEqual, ILRemoveLastID,
    SHCreateDataObject, SHGetFolderLocation,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateIconIndirect, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetAncestor, GetClassNameW, GetCursorPos,
    GetSystemMetrics, HCURSOR, HWND_TOPMOST, ICONINFO, MSG, MsgWaitForMultipleObjects, PM_REMOVE, PeekMessageW, PostQuitMessage,
    QS_ALLINPUT, RegisterClassW, SM_CXCURSOR, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOSIZE, TranslateMessage, WM_QUIT,
    SetCursor, SetWindowPos, ShowWindow, ULW_ALPHA, UpdateLayeredWindow, WNDCLASSW, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP, WindowFromPoint, GA_ROOT,
};
use windows_sys::core::{GUID, HRESULT};

use crate::com::{method, release};
use crate::log::{log, log_win32};
use crate::util::wide;

const IID_UNKNOWN: GUID = GUID::from_u128(0x00000000_0000_0000_C000_000000000046);
const IID_DROP_TARGET: GUID = GUID::from_u128(0x00000122_0000_0000_C000_000000000046);
const IID_DROP_SOURCE: GUID = GUID::from_u128(0x00000121_0000_0000_C000_000000000046);
const IID_DATA_OBJECT: GUID = GUID::from_u128(0x0000010E_0000_0000_C000_000000000046);

/// The key and button flags OLE passes (MK_*).
const MK_LBUTTON: u32 = 1;
const MK_RBUTTON: u32 = 2;
const MK_SHIFT: u32 = 4;
const MK_CONTROL: u32 = 8;

/// IDataObject's GetData.
const GET_DATA: usize = 3;
type GetData = unsafe extern "system" fn(*mut c_void, *const FORMATETC, *mut STGMEDIUM) -> HRESULT;

/// Makes OLE's drag and drop usable on the calling thread (a window thread: single-threaded
/// apartment). Harmless to repeat; logged if it fails.
pub fn init() {
    // SAFETY: plain call.
    let hr = unsafe { OleInitialize(null()) };
    if hr < 0 {
        log(&format!("drag and drop: OleInitialize 0x{hr:08X}"));
    }
}

fn keys_of(state: u32) -> Keys {
    Keys { ctrl: state & MK_CONTROL != 0, shift: state & MK_SHIFT != 0 }
}

fn effect_bits(e: Option<Effect>) -> u32 {
    match e {
        Some(Effect::Move) => DROPEFFECT_MOVE,
        Some(Effect::Copy) => DROPEFFECT_COPY,
        Some(Effect::Link) => DROPEFFECT_LINK,
        None => DROPEFFECT_NONE,
    }
}

/// OLE's offered effects as the core's `Offered`.
fn offered_of(bits: u32) -> Offered {
    Offered { moves: bits & DROPEFFECT_MOVE != 0, copy: bits & DROPEFFECT_COPY != 0, link: bits & DROPEFFECT_LINK != 0 }
}

/// What the window said, kept within what the source offers (`drag::narrow`): the handlers
/// narrow it themselves before they do anything (`drop_effect`), so this only guards.
fn allowed(wanted: Option<Effect>, offered: u32) -> Option<Effect> {
    drag::narrow(wanted, offered_of(offered), false)
}

// ---------------------------------------------------------------------------------------------
// What is being dragged, as a window sees it.

/// A drag that started in this program, while it lasts: the window it came from and its files.
#[derive(Clone, Debug)]
pub struct OwnDrag {
    pub source: HWND,
    pub paths: Vec<String>,
    /// The look's own icons dragged along (their ids: only their places move, on the desktop).
    pub others: Vec<String>,
    pub right: bool,
}

thread_local! {
    static OWN: RefCell<Option<OwnDrag>> = const { RefCell::new(None) };
}

/// The drag that started in this program and is going on now, if any.
pub fn own_drag() -> Option<OwnDrag> {
    OWN.with(|o| o.borrow().clone())
}

/// The step of a drag over a window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Enter,
    Over,
    Leave,
    Drop,
}

/// A drag over a window: where the pointer is (the window's own pixels, not scaled down), the
/// keys, whether the right button drags, the files, and whether it started in this program.
#[derive(Clone, Debug)]
pub struct Over {
    pub x: i32,
    pub y: i32,
    pub keys: Keys,
    pub right: bool,
    pub files: Vec<String>,
    pub own: Option<OwnDrag>,
    /// What the source allows.
    pub offered: u32,
}

impl Over {
    /// What the source allows, as the core's `Offered`.
    pub fn offers(&self) -> Offered {
        offered_of(self.offered)
    }
}

/// A window's part in a drag: says what a drop at that point would do (and shows it), and at
/// `Phase::Drop` does it (and returns what it did). `Leave` ends it without a drop.
pub type Handler = fn(HWND, Phase, &Over) -> Option<Effect>;

#[repr(C)]
struct TargetVtbl {
    query_interface: unsafe extern "system" fn(*mut Target, *const GUID, *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(*mut Target) -> u32,
    release: unsafe extern "system" fn(*mut Target) -> u32,
    drag_enter: unsafe extern "system" fn(*mut Target, *mut c_void, u32, POINTL, *mut u32) -> HRESULT,
    drag_over: unsafe extern "system" fn(*mut Target, u32, POINTL, *mut u32) -> HRESULT,
    drag_leave: unsafe extern "system" fn(*mut Target) -> HRESULT,
    drop: unsafe extern "system" fn(*mut Target, *mut c_void, u32, POINTL, *mut u32) -> HRESULT,
}

/// The drop target object: the method table first, as COM wants it.
#[repr(C)]
struct Target {
    vtbl: *const TargetVtbl,
    refs: AtomicU32,
    hwnd: HWND,
    handler: Handler,
    /// The drag over the window now: its files (read at DragEnter) and its button.
    files: RefCell<Option<(Vec<String>, bool)>>,
    offered: Cell<u32>,
}

static TARGET_VTBL: TargetVtbl = TargetVtbl {
    query_interface: target_qi,
    add_ref: target_add_ref,
    release: target_release,
    drag_enter: target_enter,
    drag_over: target_over,
    drag_leave: target_leave,
    drop: target_drop,
};

unsafe extern "system" fn target_qi(this: *mut Target, iid: *const GUID, out: *mut *mut c_void) -> HRESULT {
    // SAFETY: COM's rules: `out` is valid; `this` is our object.
    unsafe {
        if out.is_null() {
            return E_POINTER;
        }
        let iid = &*iid;
        if guid_eq(iid, &IID_UNKNOWN) || guid_eq(iid, &IID_DROP_TARGET) {
            target_add_ref(this);
            *out = this.cast();
            S_OK
        } else {
            *out = null_mut();
            E_NOINTERFACE
        }
    }
}

fn guid_eq(a: &GUID, b: &GUID) -> bool {
    a.data1 == b.data1 && a.data2 == b.data2 && a.data3 == b.data3 && a.data4 == b.data4
}

unsafe extern "system" fn target_add_ref(this: *mut Target) -> u32 {
    // SAFETY: a live object.
    unsafe { (*this).refs.fetch_add(1, Ordering::SeqCst) + 1 }
}

unsafe extern "system" fn target_release(this: *mut Target) -> u32 {
    // SAFETY: a live object; freed by the last reference (made by Box::into_raw).
    unsafe {
        let left = (*this).refs.fetch_sub(1, Ordering::SeqCst) - 1;
        if left == 0 {
            drop(Box::from_raw(this));
        }
        left
    }
}

/// The pointer's place in the window's own pixels.
fn client_point(hwnd: HWND, pt: POINTL) -> (i32, i32) {
    let mut p = POINT { x: pt.x, y: pt.y };
    // SAFETY: our own window and a valid point.
    unsafe { ScreenToClient(hwnd, &mut p) };
    (p.x, p.y)
}

impl Target {
    fn over(&self, state: u32, pt: POINTL) -> Over {
        let (x, y) = client_point(self.hwnd, pt);
        let (files, right) = self.files.borrow().clone().unwrap_or_default();
        Over { x, y, keys: keys_of(state), right, files, own: own_drag(), offered: self.offered.get() }
    }
}

unsafe extern "system" fn target_enter(this: *mut Target, data: *mut c_void, state: u32, pt: POINTL, effect: *mut u32) -> HRESULT {
    // SAFETY: OLE passes a live data object and a valid effect pointer.
    unsafe {
        let t = &*this;
        let files = if data.is_null() { Vec::new() } else { read_files(data) };
        let right = state & MK_RBUTTON != 0;
        *t.files.borrow_mut() = Some((files, right));
        t.offered.set(*effect);
        let over = t.over(state, pt);
        let e = allowed((t.handler)(t.hwnd, Phase::Enter, &over), *effect);
        *effect = effect_bits(e);
    }
    S_OK
}

unsafe extern "system" fn target_over(this: *mut Target, state: u32, pt: POINTL, effect: *mut u32) -> HRESULT {
    // SAFETY: as above.
    unsafe {
        let t = &*this;
        t.offered.set(*effect);
        let over = t.over(state, pt);
        let e = allowed((t.handler)(t.hwnd, Phase::Over, &over), *effect);
        *effect = effect_bits(e);
    }
    S_OK
}

unsafe extern "system" fn target_leave(this: *mut Target) -> HRESULT {
    // SAFETY: as above.
    unsafe {
        let t = &*this;
        let over = t.over(0, POINTL { x: i32::MIN / 2, y: i32::MIN / 2 });
        (t.handler)(t.hwnd, Phase::Leave, &over);
        t.files.borrow_mut().take();
    }
    S_OK
}

unsafe extern "system" fn target_drop(this: *mut Target, data: *mut c_void, state: u32, pt: POINTL, effect: *mut u32) -> HRESULT {
    // SAFETY: as above.
    unsafe {
        let t = &*this;
        t.offered.set(*effect);
        let over = t.over(state, pt);
        // How a copy or move started by this drop tells when it is done (`drop_job`).
        WAY.with(|w| *w.borrow_mut() = Some(way_of(data, over.own.is_some())));
        let done = allowed((t.handler)(t.hwnd, Phase::Drop, &over), *effect);
        if let Some(Way::Later(cap)) = WAY.with(|w| w.borrow_mut().take()) {
            release(cap); // nothing was copied or moved: the source is not asked to wait
        }
        wait_for_drop_jobs();
        t.files.borrow_mut().take();
        // A move carried out here is reported as nothing: the source must not delete anything.
        *effect = match done {
            Some(Effect::Move) | None => DROPEFFECT_NONE,
            e => effect_bits(e),
        };
    }
    S_OK
}

/// For live tests (`folders.exe`'s test drop): a drop of `files` from another program that
/// offers `offered`, at the window's own pixel (`x`, `y`), carried through the same steps as one
/// by OLE (DragOver, then Drop, each narrowed and answered as above). Returns what the source
/// would be told (DROPEFFECT_*).
pub fn test_drop(hwnd: HWND, handler: Handler, files: Vec<String>, offered: u32, x: i32, y: i32) -> u32 {
    let mut target = Target {
        vtbl: &TARGET_VTBL,
        refs: AtomicU32::new(1),
        hwnd,
        handler,
        files: RefCell::new(Some((files, false))),
        offered: Cell::new(offered),
    };
    let mut p = POINT { x, y };
    // SAFETY: our own window; `target` lives on this stack for both calls and is never handed
    // to COM (no reference is taken or released).
    unsafe {
        windows_sys::Win32::Graphics::Gdi::ClientToScreen(hwnd, &mut p);
        let pt = POINTL { x: p.x, y: p.y };
        let this: *mut Target = &mut target;
        let mut effect = offered;
        target_over(this, 0, pt, &mut effect);
        let mut effect = offered;
        target_drop(this, null_mut(), 0, pt, &mut effect);
        effect
    }
}

/// The files a data object holds (CF_HDROP), or none.
unsafe fn read_files(data: *mut c_void) -> Vec<String> {
    let format = FORMATETC { cfFormat: CF_HDROP, ptd: null_mut(), dwAspect: DVASPECT_CONTENT, lindex: -1, tymed: TYMED_HGLOBAL as u32 };
    let mut medium = STGMEDIUM::default();
    // SAFETY: a live data object; the medium is released after reading.
    unsafe {
        let hr = method::<GetData>(data, GET_DATA)(data, &format, &mut medium);
        if hr < 0 {
            if hr != DV_E_FORMATETC {
                log(&format!("drag and drop: the dragged data gave no files (0x{hr:08X})"));
            }
            return Vec::new();
        }
        let mut out = Vec::new();
        if medium.tymed == TYMED_HGLOBAL as u32 {
            let drop = medium.u.hGlobal as HDROP;
            let n = DragQueryFileW(drop, u32::MAX, null_mut(), 0);
            for i in 0..n {
                let len = DragQueryFileW(drop, i, null_mut(), 0) as usize;
                let mut buf = vec![0u16; len + 1];
                DragQueryFileW(drop, i, buf.as_mut_ptr(), buf.len() as u32);
                out.push(String::from_utf16_lossy(&buf[..len]));
            }
        }
        ReleaseStgMedium(&mut medium);
        out
    }
}

// ---------------------------------------------------------------------------------------------
// Files that vanish when the drag call returns (the safety review of 2026-11-18, "suspected 1",
// proven: a source that deleted its file the moment its drag call returned left nothing for the
// copy, which starts on a thread of its own). Archive and mail programs hand over files they
// extracted to a temporary folder and delete them so. Windows' Explorer, as a target, carries out
// a drop from such a source before its Drop returns; a source may instead say it can wait
// (IDataObjectAsyncCapability), and is then told when the work is done.

const IID_ASYNC: GUID = GUID::from_u128(0x3D8B0590_F691_11D2_8EA9_006097DF5BD4);
/// IDataObjectAsyncCapability's table: IUnknown's three, then SetAsyncMode, GetAsyncMode,
/// StartOperation, InOperation, EndOperation.
const GET_ASYNC_MODE: usize = 4;
const START_OPERATION: usize = 5;
const END_OPERATION: usize = 7;
/// IDataObject's QueryGetData and SetData.
const QUERY_GET_DATA: usize = 5;
const SET_DATA: usize = 7;
type QueryInterface = unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT;
type GetAsyncMode = unsafe extern "system" fn(*mut c_void, *mut windows_sys::core::BOOL) -> HRESULT;
type StartOperation = unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT;
type EndOperation = unsafe extern "system" fn(*mut c_void, HRESULT, *mut c_void, u32) -> HRESULT;
type QueryGetData = unsafe extern "system" fn(*mut c_void, *const FORMATETC) -> HRESULT;
type SetData = unsafe extern "system" fn(*mut c_void, *const FORMATETC, *const STGMEDIUM, windows_sys::core::BOOL) -> HRESULT;

/// How the copy or move of the drop being carried out tells it is done.
enum Way {
    /// As before: on its own thread, the drop answered at once (a drag of Bevel95's own: its
    /// files stay where they are until Bevel95 moves them).
    Now,
    /// Another program that may delete its files when its drag call returns: the drop answers
    /// only once the work is done (the window's messages are handled meanwhile).
    Wait,
    /// Another program that said it can wait (its async capability, a reference of ours): it is
    /// told when the work starts and when it is done; the drop answers at once.
    Later(*mut c_void),
}

thread_local! {
    static WAY: RefCell<Option<Way>> = const { RefCell::new(None) };
    /// The events of the jobs this drop waits for, and sources to tell afterwards (if telling
    /// them from the job's thread could not be arranged).
    static WAITS: RefCell<Vec<HANDLE>> = const { RefCell::new(Vec::new()) };
    static END_AFTER: RefCell<Vec<(*mut c_void, u32)>> = const { RefCell::new(Vec::new()) };
}

/// The format that marks a drag of Bevel95's own (between its two programs): what it drags stays
/// where it is until Bevel95 moves it, so such a drop is carried out as before.
fn own_format() -> u16 {
    static FORMAT: std::sync::OnceLock<u16> = std::sync::OnceLock::new();
    *FORMAT.get_or_init(|| {
        let name = wide("Bevel95 drag");
        // SAFETY: a valid null-terminated name.
        unsafe { windows_sys::Win32::System::DataExchange::RegisterClipboardFormatW(name.as_ptr()) as u16 }
    })
}

fn own_formatetc() -> FORMATETC {
    FORMATETC { cfFormat: own_format(), ptd: null_mut(), dwAspect: DVASPECT_CONTENT, lindex: -1, tymed: TYMED_HGLOBAL as u32 }
}

/// Marks a data object of Bevel95's own drag (`own_format`).
unsafe fn mark_own(data: *mut c_void) {
    // SAFETY: a live data object; the block is the object's once SetData takes it.
    unsafe {
        let block = windows_sys::Win32::System::Memory::GlobalAlloc(windows_sys::Win32::System::Memory::GMEM_MOVEABLE, 4);
        if block.is_null() {
            return;
        }
        let mut medium = STGMEDIUM { tymed: TYMED_HGLOBAL as u32, ..Default::default() };
        medium.u.hGlobal = block;
        let format = own_formatetc();
        if method::<SetData>(data, SET_DATA)(data, &format, &medium, 1) < 0 {
            windows_sys::Win32::Foundation::GlobalFree(block);
            log("drag and drop: the drag could not be marked as Bevel95's own");
        }
    }
}

/// How a drop from `data` is carried out (`own`: a drag of this program).
unsafe fn way_of(data: *mut c_void, own: bool) -> Way {
    if own {
        return Way::Now;
    }
    if data.is_null() {
        return Way::Wait;
    }
    // SAFETY: a live data object handed to Drop; the capability's reference is ours.
    unsafe {
        let format = own_formatetc();
        if method::<QueryGetData>(data, QUERY_GET_DATA)(data, &format) == S_OK {
            return Way::Now;
        }
        let mut cap: *mut c_void = null_mut();
        if method::<QueryInterface>(data, 0)(data, &IID_ASYNC, &mut cap) >= 0 && !cap.is_null() {
            let mut on: windows_sys::core::BOOL = 0;
            if method::<GetAsyncMode>(cap, GET_ASYNC_MODE)(cap, &mut on) >= 0 && on != 0 {
                return Way::Later(cap);
            }
            release(cap);
        }
    }
    Way::Wait
}

/// What a copy or move started by a drop does when it is done (`drop_job`): kept by the job's
/// thread and dropped at its end, also when it unwinds.
pub struct DropJob(Finish);

enum Finish {
    Nothing,
    /// Sets the event the drop waits for.
    Signal(HANDLE),
    /// Tells the source (its capability, marshaled to the job's thread) what was done.
    End(*mut c_void, u32),
}

// SAFETY: an event handle may be set from any thread; the stream is made to be unmarshaled on
// another thread, once.
unsafe impl Send for DropJob {}

impl Drop for DropJob {
    fn drop(&mut self) {
        // SAFETY: our own event (closed by the drop's thread only after it is set); the stream
        // from `CoMarshalInterThreadInterfaceInStream`, used once.
        unsafe {
            match self.0 {
                Finish::Nothing => {}
                Finish::Signal(event) => {
                    windows_sys::Win32::System::Threading::SetEvent(event);
                }
                Finish::End(stream, told) => {
                    let mut cap: *mut c_void = null_mut();
                    let hr = windows_sys::Win32::System::Com::StructuredStorage::CoGetInterfaceAndReleaseStream(stream, &IID_ASYNC, &mut cap);
                    if hr >= 0 && !cap.is_null() {
                        method::<EndOperation>(cap, END_OPERATION)(cap, S_OK, null_mut(), told);
                        release(cap);
                    } else {
                        log(&format!("drag and drop: the source could not be told the drop is done (0x{hr:08X})"));
                    }
                }
            }
        }
    }
}

/// For `file_ops::drop_files`, on the window's thread while it carries out a drop: how its copy
/// or move must tell when it is done (`told`: the DROPEFFECT the source is told). Outside a drop
/// (Paste, a test's own message), or for a drag of Bevel95's own, nothing.
pub(crate) fn drop_job(told: u32) -> DropJob {
    let way = WAY.with(|w| w.borrow_mut().take());
    // SAFETY: plain calls; the capability is ours (from `way_of`) and released here.
    unsafe {
        match way {
            None | Some(Way::Now) => DropJob(Finish::Nothing),
            Some(Way::Wait) => {
                let event = windows_sys::Win32::System::Threading::CreateEventW(null(), 1, 0, null());
                if event.is_null() {
                    log_win32("CreateEventW (a drop that waits for its copy)");
                    return DropJob(Finish::Nothing);
                }
                WAITS.with(|w| w.borrow_mut().push(event));
                DropJob(Finish::Signal(event))
            }
            Some(Way::Later(cap)) => {
                method::<StartOperation>(cap, START_OPERATION)(cap, null_mut());
                let mut stream: *mut c_void = null_mut();
                let hr = windows_sys::Win32::System::Com::Marshal::CoMarshalInterThreadInterfaceInStream(&IID_ASYNC, cap, &mut stream);
                if hr >= 0 && !stream.is_null() {
                    release(cap);
                    return DropJob(Finish::End(stream, told));
                }
                // Not to be told from the job's thread: the drop waits, then tells it here.
                log(&format!("drag and drop: the source's wait could not be passed on (0x{hr:08X}); the drop waits instead"));
                END_AFTER.with(|e| e.borrow_mut().push((cap, told)));
                let event = windows_sys::Win32::System::Threading::CreateEventW(null(), 1, 0, null());
                if event.is_null() {
                    return DropJob(Finish::Nothing);
                }
                WAITS.with(|w| w.borrow_mut().push(event));
                DropJob(Finish::Signal(event))
            }
        }
    }
}

/// After a drop's handler: waits for the jobs it started that must be done before the drop
/// answers (`Way::Wait`), handling the thread's messages meanwhile so the windows keep painting
/// and answering (Windows' progress window runs on the job's thread).
fn wait_for_drop_jobs() {
    let waits = WAITS.with(|w| std::mem::take(&mut *w.borrow_mut()));
    let started = std::time::Instant::now();
    let mut quit = None;
    for event in &waits {
        // SAFETY: our own events, waited for and closed here; the thread's messages handled.
        unsafe {
            loop {
                let r = MsgWaitForMultipleObjects(1, event, 0, u32::MAX, QS_ALLINPUT);
                if r == WAIT_OBJECT_0 {
                    break;
                }
                let mut msg = MSG::default();
                while PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) != 0 {
                    if msg.message == WM_QUIT {
                        quit = Some(msg.wParam as i32);
                        continue;
                    }
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                if r != WAIT_OBJECT_0 + 1 {
                    log_win32("MsgWaitForMultipleObjects (a drop that waits for its copy)");
                    break;
                }
            }
            windows_sys::Win32::Foundation::CloseHandle(*event);
        }
    }
    for (cap, told) in END_AFTER.with(|e| std::mem::take(&mut *e.borrow_mut())) {
        // SAFETY: our reference of the source's capability, used and released once.
        unsafe {
            method::<EndOperation>(cap, END_OPERATION)(cap, S_OK, null_mut(), told);
            release(cap);
        }
    }
    if !waits.is_empty() {
        log(&format!("drag and drop: a drop from another program answered once its work was done ({} ms)", started.elapsed().as_millis()));
    }
    if let Some(code) = quit {
        // SAFETY: the WM_QUIT taken meanwhile goes back for the thread's own loop.
        unsafe { PostQuitMessage(code) };
    }
}

/// `hwnd` takes dragged files from now on; `handler` hears every step. Logged if Windows refuses.
pub fn register(hwnd: HWND, handler: Handler) {
    let target = Box::into_raw(Box::new(Target {
        vtbl: &TARGET_VTBL,
        refs: AtomicU32::new(1),
        hwnd,
        handler,
        files: RefCell::new(None),
        offered: Cell::new(0),
    }));
    // SAFETY: a live object; RegisterDragDrop keeps its own reference, ours is let go.
    unsafe {
        let hr = RegisterDragDrop(hwnd, target.cast());
        if hr < 0 {
            log(&format!("drag and drop: RegisterDragDrop 0x{hr:08X}"));
        }
        target_release(target);
    }
}

/// `hwnd` takes no more drops (before it is destroyed).
pub fn revoke(hwnd: HWND) {
    // SAFETY: a plain call; Windows lets go of its reference.
    unsafe { RevokeDragDrop(hwnd) };
}

/// Where a drop on a Bevel95 window goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Dest {
    Folder(PathBuf),
    RecycleBin,
    /// It takes no files (My Computer, Control Panel, Printers, a window's title bar).
    Refuse,
}

/// What a drop of `files` into `dest` does with `keys` held, in `era`'s way
/// (`bevel_core::drag`): `None` refuses (the "no" sign): nothing dragged that is a file, a folder
/// into itself, or a move to where they already are.
pub fn effect_for(era: Era, dest: &Dest, files: &[String], keys: Keys) -> Option<Effect> {
    if files.is_empty() {
        return None;
    }
    match dest {
        Dest::Refuse => None,
        Dest::RecycleBin => Some(Effect::Move),
        Dest::Folder(dir) => {
            let dir = dir.to_string_lossy();
            if drag::into_itself(files, &dir) {
                return None;
            }
            let same_drive = files.iter().all(|f| drag::drive_of(f) == drag::drive_of(&dir));
            let programs = files.iter().all(|f| drag::is_program(f));
            let e = drag::effect(era, keys, same_drive, programs);
            if e == Effect::Move && drag::all_in(files, &dir) {
                return None;
            }
            Some(e)
        }
    }
}

/// What a drop of `over`'s files into `dest` does, decided once for the pointer and the drop
/// (the safety review of 2026-11-18, A1): the look's wish (`effect_for`) narrowed by what the
/// source offers (`drag::narrow`); the Recycle Bin only for what may be moved. `None`: the "no"
/// sign, and nothing is done.
pub fn drop_effect(era: Era, dest: &Dest, over: &Over) -> Option<Effect> {
    drag::narrow(effect_for(era, dest, &over.files, over.keys), over.offers(), *dest == Dest::RecycleBin)
}

/// What the right button's menu offers for a drop into `dest` (`only_move`: an icon moved about
/// in its own window), within what the source offers.
pub fn menu_effects(dest: &Dest, only_move: bool, over: &Over) -> Vec<Effect> {
    drag::menu_effects(over.offers(), only_move || *dest == Dest::RecycleBin)
}

// ---------------------------------------------------------------------------------------------
// A drag out of a Bevel95 window.

#[repr(C)]
struct SourceVtbl {
    query_interface: unsafe extern "system" fn(*mut Source, *const GUID, *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(*mut Source) -> u32,
    release: unsafe extern "system" fn(*mut Source) -> u32,
    query_continue: unsafe extern "system" fn(*mut Source, i32, u32) -> HRESULT,
    give_feedback: unsafe extern "system" fn(*mut Source, u32) -> HRESULT,
}

#[repr(C)]
struct Source {
    vtbl: *const SourceVtbl,
    refs: AtomicU32,
    right: bool,
    era: Era,
    ghost: Option<Ghost>,
}

static SOURCE_VTBL: SourceVtbl = SourceVtbl {
    query_interface: source_qi,
    add_ref: source_add_ref,
    release: source_release,
    query_continue: source_continue,
    give_feedback: source_feedback,
};

unsafe extern "system" fn source_qi(this: *mut Source, iid: *const GUID, out: *mut *mut c_void) -> HRESULT {
    // SAFETY: COM's rules.
    unsafe {
        if out.is_null() {
            return E_POINTER;
        }
        let iid = &*iid;
        if guid_eq(iid, &IID_UNKNOWN) || guid_eq(iid, &IID_DROP_SOURCE) {
            source_add_ref(this);
            *out = this.cast();
            S_OK
        } else {
            *out = null_mut();
            E_NOINTERFACE
        }
    }
}

unsafe extern "system" fn source_add_ref(this: *mut Source) -> u32 {
    // SAFETY: a live object.
    unsafe { (*this).refs.fetch_add(1, Ordering::SeqCst) + 1 }
}

unsafe extern "system" fn source_release(this: *mut Source) -> u32 {
    // SAFETY: a live object, freed by its last reference.
    unsafe {
        let left = (*this).refs.fetch_sub(1, Ordering::SeqCst) - 1;
        if left == 0 {
            drop(Box::from_raw(this));
        }
        left
    }
}

/// Esc or the other button cancels (the Reference PCs); letting the dragging button go drops.
unsafe extern "system" fn source_continue(this: *mut Source, escape: i32, state: u32) -> HRESULT {
    // SAFETY: a live object.
    let right = unsafe { (*this).right };
    let (held, other) = if right { (MK_RBUTTON, MK_LBUTTON) } else { (MK_LBUTTON, MK_RBUTTON) };
    if escape != 0 || state & other != 0 {
        return DRAGDROP_S_CANCEL;
    }
    if state & held == 0 {
        return DRAGDROP_S_DROP;
    }
    S_OK
}

/// The look's pointer for what a drop would do here, and the dragged picture beside it.
unsafe extern "system" fn source_feedback(this: *mut Source, effect: u32) -> HRESULT {
    // SAFETY: a live object.
    let s = unsafe { &*this };
    let e = if effect & DROPEFFECT_MOVE != 0 {
        Some(Effect::Move)
    } else if effect & DROPEFFECT_COPY != 0 {
        Some(Effect::Copy)
    } else if effect & DROPEFFECT_LINK != 0 {
        Some(Effect::Link)
    } else {
        None
    };
    let cursor = drag_cursor(s.era, e);
    if !cursor.is_null() {
        // SAFETY: a cursor made by `drag_cursor`.
        unsafe { SetCursor(cursor) };
    }
    if let Some(g) = &s.ghost {
        g.follow();
    }
    S_OK
}

/// What a drag out of a window takes along and shows.
pub struct Out {
    /// The window it starts in.
    pub hwnd: HWND,
    /// The files and folders (whole paths).
    pub paths: Vec<String>,
    /// The look's own icons along (ids), which only move on the desktop they are on.
    pub others: Vec<String>,
    pub right: bool,
    pub era: Era,
    /// The look's dragged picture (0xAARRGGBB at 1×) and where its top left corner is from the
    /// pointer, in 1× pixels; and the size it is shown at.
    pub picture: Option<(Framebuffer, i32, i32)>,
    pub scale: Scale,
}

/// Runs a drag until it is dropped or cancelled (Windows' own loop: the program's windows keep
/// painting meanwhile). Returns what the drop did as the target reported it (`None`: cancelled,
/// refused, or a move the target carried out itself).
pub fn run(out: Out) -> Option<Effect> {
    let Out { hwnd, paths, others, right, era, picture, scale } = out;
    let data = data_object(&paths);
    if data.is_null() {
        return None;
    }
    // SAFETY: the live data object just made.
    unsafe { mark_own(data) };
    let ghost = picture.and_then(|(pic, dx, dy)| Ghost::new(&pic, dx, dy, scale));
    let source = Box::into_raw(Box::new(Source { vtbl: &SOURCE_VTBL, refs: AtomicU32::new(1), right, era, ghost }));
    OWN.with(|o| *o.borrow_mut() = Some(OwnDrag { source: hwnd, paths: paths.clone(), others, right }));
    let mut done = DROPEFFECT_NONE;
    // SAFETY: live objects for the call; both released after it.
    let hr = unsafe { DoDragDrop(data, source.cast(), DROPEFFECT_COPY | DROPEFFECT_MOVE | DROPEFFECT_LINK, &mut done) };
    OWN.with(|o| o.borrow_mut().take());
    // SAFETY: our references.
    unsafe {
        source_release(source);
        release(data);
    }
    if hr < 0 {
        log(&format!("drag and drop: DoDragDrop 0x{hr:08X}"));
    }
    if hr != DRAGDROP_S_DROP {
        return None;
    }
    match done {
        d if d & DROPEFFECT_MOVE != 0 => Some(Effect::Move),
        d if d & DROPEFFECT_COPY != 0 => Some(Effect::Copy),
        d if d & DROPEFFECT_LINK != 0 => Some(Effect::Link),
        _ => None,
    }
}

/// The shell's data object for `paths` (empty if there are none: only the look's own icons
/// move, on the desktop they are on). Null if Windows would not make one.
fn data_object(paths: &[String]) -> *mut c_void {
    // SAFETY: item lists made and freed here; the data object holds its own copies.
    unsafe {
        let pidls: Vec<*mut ITEMIDLIST> = paths
            .iter()
            .filter_map(|p| {
                let w = wide(p);
                let pidl = ILCreateFromPathW(w.as_ptr());
                (!pidl.is_null()).then_some(pidl)
            })
            .collect();
        if pidls.len() != paths.len() {
            log("drag and drop: an item list could not be made for a dragged item");
        }
        // One folder holding them all: that folder and their own ids, as Explorer gives them;
        // else the desktop and their whole lists (the desktop shows two folders' items).
        let parents: Vec<*mut ITEMIDLIST> = pidls
            .iter()
            .map(|&p| {
                let c = ILClone(p);
                if !c.is_null() {
                    ILRemoveLastID(c);
                }
                c
            })
            .collect();
        let one = parents.first().is_some_and(|&first| !first.is_null() && parents.iter().all(|&p| !p.is_null() && ILIsEqual(first, p) != 0));
        let mut folder: *mut ITEMIDLIST = null_mut();
        let children: Vec<*const ITEMIDLIST> = if one {
            folder = ILClone(parents[0]);
            pidls.iter().map(|&p| ILFindLastID(p) as *const ITEMIDLIST).collect()
        } else {
            if SHGetFolderLocation(null_mut(), CSIDL_DESKTOP as i32, null_mut(), 0, &mut folder) < 0 {
                folder = null_mut();
            }
            pidls.iter().map(|&p| p as *const ITEMIDLIST).collect()
        };
        let mut data: *mut c_void = null_mut();
        let hr = SHCreateDataObject(
            folder,
            children.len() as u32,
            if children.is_empty() { null() } else { children.as_ptr() },
            null_mut(),
            &IID_DATA_OBJECT,
            &mut data,
        );
        if hr < 0 || data.is_null() {
            log(&format!("drag and drop: SHCreateDataObject 0x{hr:08X}"));
            data = null_mut();
        }
        if !folder.is_null() {
            ILFree(folder);
        }
        for p in parents.into_iter().chain(pidls) {
            if !p.is_null() {
                CoTaskMemFree(p as *const c_void);
            }
        }
        data
    }
}

// ---------------------------------------------------------------------------------------------
// The pointers and the dragged picture.

thread_local! {
    static CURSORS: RefCell<HashMap<(u8, Option<Effect>), HCURSOR>> = RefCell::new(HashMap::new());
}

/// How many times the measured pixels are drawn: as big as Windows draws its own pointers.
fn factor() -> i32 {
    // SAFETY: a plain query.
    let size = unsafe { GetSystemMetrics(SM_CXCURSOR) };
    ((size + 16) / 32).max(1)
}

/// The look's drag pointer (made once per thread and kept for the program's life).
fn drag_cursor(era: Era, effect: Option<Effect>) -> HCURSOR {
    if let Some(c) = CURSORS.with(|c| c.borrow().get(&(era as u8, effect)).copied()) {
        return c;
    }
    let c = make_cursor(drag::pointer(era, effect));
    CURSORS.with(|m| m.borrow_mut().insert((era as u8, effect), c));
    c
}

/// A 32-bit top-down DIB section of `w` × `h` and its pixels.
unsafe fn dib(w: i32, h: i32) -> (HBITMAP, *mut u32) {
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -h,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits: *mut c_void = null_mut();
    // SAFETY: a valid header; the bits belong to the bitmap.
    let bmp = unsafe { CreateDIBSection(null_mut(), &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0) };
    (bmp, bits.cast())
}

fn make_cursor(art: &drag::PointerArt) -> HCURSOR {
    let k = factor();
    let (w, h) = art.size();
    let side = ((w.max(h) * k + 31) / 32 * 32).max(32);
    // SAFETY: bitmaps made, used for the cursor (which copies them) and deleted.
    unsafe {
        let (color, bits) = dib(side, side);
        if color.is_null() || bits.is_null() {
            log_win32("CreateDIBSection (a drag pointer)");
            return null_mut();
        }
        for y in 0..side {
            for x in 0..side {
                let p = art.pixel(x / k, y / k);
                // Opaque pixels only: straight and premultiplied are the same.
                *bits.add((y * side + x) as usize) = p;
            }
        }
        let mask_bits = vec![0u8; (side / 8 * side) as usize];
        let mask = CreateBitmap(side, side, 1, 1, mask_bits.as_ptr().cast());
        let (hx, hy) = art.hot;
        let info = ICONINFO { fIcon: 0, xHotspot: (hx * k + k / 2) as u32, yHotspot: (hy * k + k / 2) as u32, hbmMask: mask, hbmColor: color };
        let cursor = CreateIconIndirect(&info);
        if cursor.is_null() {
            log_win32("CreateIconIndirect (a drag pointer)");
        }
        DeleteObject(color as HGDIOBJ);
        if !mask.is_null() {
            DeleteObject(mask as HGDIOBJ);
        }
        cursor
    }
}

const GHOST_CLASS: &str = "Bevel95DragPicture";

/// The see-through window that carries the dragged picture beside the pointer. It takes no
/// clicks (Windows looks through it for the window under the pointer) and shows only over
/// Bevel95's own windows (98 showed it only over the shell's; another program just gets the
/// pointer).
struct Ghost {
    hwnd: HWND,
    dc: HDC,
    bitmap: HBITMAP,
    old: HGDIOBJ,
    /// The picture's top left corner from the pointer, in screen pixels.
    offset: (i32, i32),
    size: (i32, i32),
    shown: Cell<bool>,
}

impl Ghost {
    fn new(pic: &Framebuffer, dx: i32, dy: i32, scale: Scale) -> Option<Ghost> {
        let s = scale;
        // Premultiplied, as UpdateLayeredWindow wants it, before it is enlarged: a size between
        // whole numbers then mixes the edges' colors by how much of each shows (never by light:
        // `enlarge_premultiplied`).
        let mut pre = pic.clone();
        for p in &mut pre.pixels {
            let a = *p >> 24;
            let ch = |sh: u32| (((*p >> sh) & 0xFF) * a / 255) << sh;
            *p = (a << 24) | ch(16) | ch(8) | ch(0);
        }
        let big = s.enlarge_premultiplied(&pre);
        let (w, h) = (big.width, big.height);
        if w <= 0 || h <= 0 {
            return None;
        }
        // SAFETY: a class with Windows' own procedure; objects made here are freed in `drop`.
        unsafe {
            let class = wide(GHOST_CLASS);
            let wc = WNDCLASSW { lpfnWndProc: Some(DefWindowProcW), lpszClassName: class.as_ptr(), ..Default::default() };
            RegisterClassW(&wc); // a second time fails harmlessly
            let style: u32 = WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
            let hwnd = CreateWindowExW(style, class.as_ptr(), null(), WS_POPUP, 0, 0, w, h, null_mut(), null_mut(), null_mut(), null());
            if hwnd.is_null() {
                log_win32("CreateWindowExW (the dragged picture)");
                return None;
            }
            let (bitmap, bits) = dib(w, h);
            if bitmap.is_null() || bits.is_null() {
                log_win32("CreateDIBSection (the dragged picture)");
                DestroyWindow(hwnd);
                return None;
            }
            std::ptr::copy_nonoverlapping(big.pixels.as_ptr(), bits, big.pixels.len());
            let dc = CreateCompatibleDC(null_mut());
            let old = SelectObject(dc, bitmap as HGDIOBJ);
            let ghost = Ghost { hwnd, dc, bitmap, old, offset: (s.at(dx), s.at(dy)), size: (w, h), shown: Cell::new(false) };
            ghost.follow();
            Some(ghost)
        }
    }

    /// Moves the picture to the pointer; shown over Bevel95's windows only.
    fn follow(&self) {
        let mut p = POINT::default();
        // SAFETY: plain calls on our own window and objects.
        unsafe {
            GetCursorPos(&mut p);
            let at = POINT { x: p.x + self.offset.0, y: p.y + self.offset.1 };
            let ours = is_ours(WindowFromPoint(p));
            if ours {
                let size = SIZE { cx: self.size.0, cy: self.size.1 };
                let src = POINT { x: 0, y: 0 };
                let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: AC_SRC_ALPHA as u8 };
                if !self.shown.get() {
                    let screen = GetDC(null_mut());
                    if UpdateLayeredWindow(self.hwnd, screen, &at, &size, self.dc, &src, 0, &blend, ULW_ALPHA) == 0 {
                        log_win32("UpdateLayeredWindow (the dragged picture)");
                    }
                    ReleaseDC(null_mut(), screen);
                    ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
                    self.shown.set(true);
                }
                SetWindowPos(self.hwnd, HWND_TOPMOST, at.x, at.y, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE);
                // Under the bar's Monitor Lines (`on_top`; brief 102).
                crate::on_top::shown();
            } else if self.shown.get() {
                ShowWindow(self.hwnd, SW_HIDE);
                self.shown.set(false);
            }
        }
    }
}

impl Drop for Ghost {
    fn drop(&mut self) {
        // SAFETY: our own objects, each freed once.
        unsafe {
            SelectObject(self.dc, self.old);
            DeleteDC(self.dc);
            DeleteObject(self.bitmap as HGDIOBJ);
            DestroyWindow(self.hwnd);
        }
    }
}

/// Whether `hwnd` belongs to one of Bevel95's windows (the kit's or the desktop's).
fn is_ours(hwnd: HWND) -> bool {
    if hwnd.is_null() {
        return false;
    }
    // SAFETY: plain queries.
    let root = unsafe { GetAncestor(hwnd, GA_ROOT) };
    let mut buf = [0u16; 64];
    // SAFETY: a buffer of the length given.
    let n = unsafe { GetClassNameW(if root.is_null() { hwnd } else { root }, buf.as_mut_ptr(), buf.len() as i32) };
    let name = String::from_utf16_lossy(&buf[..n.max(0) as usize]);
    name == crate::kit::KIT_CLASS || name == "Bevel95Desktop"
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A drop from another program (no mark of Bevel95's own, no word that it can wait) answers
    /// only once its copy is done, so a source that deletes its file the moment its drag call
    /// returns (an archive or mail program's temporary file) leaves the copy whole; a drag of
    /// Bevel95's own answers at once, as before (the safety review of 2026-11-18, suspected 1:
    /// the copy started on its own thread after the answer and found the file gone).
    #[test]
    fn a_drop_from_another_program_answers_once_its_copy_is_done() {
        let dir = crate::undo::tests::scratch("drop waits");
        let src = dir.join("Bevel95 test temp.bin");
        let dst = dir.join("Bevel95 test copy.bin");
        std::fs::write(&src, vec![7u8; 1 << 20]).unwrap();
        // SAFETY: no data object: a source of another program that says nothing.
        WAY.with(|w| *w.borrow_mut() = Some(unsafe { way_of(null_mut(), false) }));
        let job = drop_job(DROPEFFECT_COPY);
        let (s, d) = (src.clone(), dst.clone());
        std::thread::spawn(move || {
            let _job = job;
            std::thread::sleep(std::time::Duration::from_millis(300));
            std::fs::copy(&s, &d).unwrap();
        });
        let started = std::time::Instant::now();
        wait_for_drop_jobs();
        assert!(started.elapsed() >= std::time::Duration::from_millis(250), "the drop waited for its copy");
        std::fs::remove_file(&src).unwrap(); // the source, the moment the drop answered
        assert_eq!(std::fs::metadata(&dst).map(|m| m.len()).ok(), Some(1 << 20), "the copy is whole");
        // A drag of Bevel95's own: nothing waits.
        // SAFETY: as above.
        WAY.with(|w| *w.borrow_mut() = Some(unsafe { way_of(null_mut(), true) }));
        assert!(matches!(drop_job(DROPEFFECT_COPY).0, Finish::Nothing));
        assert!(WAITS.with(|w| w.borrow().is_empty()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Bevel95's own drags between its two programs are marked and carried out as before; the
    /// shell's data object unmarked is "another program".
    #[test]
    fn a_drag_of_bevel95s_own_is_known_by_its_mark() {
        init();
        let dir = crate::undo::tests::scratch("drag mark");
        let file = dir.join("Bevel95 test marked.txt");
        std::fs::write(&file, b"x").unwrap();
        let data = data_object(&[file.to_string_lossy().into_owned()]);
        assert!(!data.is_null());
        // SAFETY: the live data object just made, released at the end.
        unsafe {
            let way = way_of(data, false);
            assert!(!matches!(way, Way::Now), "unmarked: another program's");
            if let Way::Later(cap) = way {
                release(cap);
            }
            mark_own(data);
            assert!(matches!(way_of(data, false), Way::Now), "marked: Bevel95's own");
            release(data);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn over(files: &[&str], offered: u32) -> Over {
        Over { x: 0, y: 0, keys: Keys::default(), right: false, files: files.iter().map(|f| f.to_string()).collect(), own: None, offered }
    }

    /// A download offered only as a copy, dropped on the same drive: the handlers carried out the
    /// look's wish (`effect_for`: a move), now what the pointer showed (a copy); on the Recycle
    /// Bin nothing. Explorer's and Bevel95's own drags (all three offered) as before.
    #[test]
    fn a_drop_carries_out_what_the_source_offers() {
        let file = ["C:\\Users\\x\\Downloads\\Bevel95 test.zip"];
        let desktop = Dest::Folder(PathBuf::from("C:\\Users\\x\\Desktop"));
        let copy_only = over(&file, DROPEFFECT_COPY);
        assert_eq!(effect_for(Era::Win98, &desktop, &copy_only.files, copy_only.keys), Some(Effect::Move), "the look's wish");
        assert_eq!(drop_effect(Era::Win98, &desktop, &copy_only), Some(Effect::Copy));
        assert_eq!(drop_effect(Era::Win98, &Dest::RecycleBin, &copy_only), None);
        assert_eq!(menu_effects(&desktop, false, &copy_only), [Effect::Copy]);
        assert!(menu_effects(&Dest::RecycleBin, false, &copy_only).is_empty());
        let all = over(&file, DROPEFFECT_COPY | DROPEFFECT_MOVE | DROPEFFECT_LINK);
        for era in [Era::Win95, Era::Win98, Era::Win2000] {
            assert_eq!(drop_effect(era, &desktop, &all), effect_for(era, &desktop, &all.files, all.keys));
            assert_eq!(drop_effect(era, &Dest::RecycleBin, &all), Some(Effect::Move));
        }
        // A move offered alone, to another drive: carried out as a move (reported as nothing),
        // never as a copy the source would answer by deleting its files.
        let move_only = over(&file, DROPEFFECT_MOVE);
        let other = Dest::Folder(PathBuf::from("D:\\Bevel95 test"));
        assert_eq!(drop_effect(Era::Win98, &other, &move_only), Some(Effect::Move));
    }
}

