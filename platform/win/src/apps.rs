//! Windows' own list of apps (its "AppsFolder": what Start's "All apps" shows today), read for
//! the Store-style apps in it. Those have no shortcut file in the Start Menu folders, which is
//! all the Programs menu used to read, so they never showed there: Calculator, Notepad, Paint,
//! Terminal, Settings and some thirty more on the owner's PC (counted 2026-11-10).
//!
//! Reading the list asks the shell and takes a moment, so the Start menu never waits for it:
//! it shows the list as last read (`cached`), and a thread of its own reads it again (at the
//! start, and when the Start menu opens and the last reading is not fresh). A newly installed
//! app so shows the second time the Start menu opens. Bevel95 Settings reads it on the spot
//! (`read_now`): its list of programs must be complete.

use std::ffi::c_void;
use std::ptr::null_mut;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use bevel_core::programs::{APP_PREFIX, is_store_app};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::UI::Shell::Common::ITEMIDLIST;
use windows_sys::Win32::UI::Shell::{
    FOLDERID_AppsFolder, ILCombine, ILFree, SHBindToObject, SHCONTF_NONFOLDERS, SHGetKnownFolderIDList,
    SHGetNameFromIDList, SIGDN, SIGDN_NORMALDISPLAY, SIGDN_PARENTRELATIVEPARSING,
};
use windows_sys::core::{GUID, HRESULT, PWSTR};

use crate::audio::method;
use crate::log::log;

/// One Store-style app: the name Windows shows for it, and what starts it (`APP_PREFIX` and
/// its ID in Windows' list; the shell opens that like a file).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct App {
    pub name: String,
    pub target: String,
}

/// IID_IShellFolder, from shobjidl_core.h.
const IID_SHELL_FOLDER: GUID = GUID::from_u128(0x000214E6_0000_0000_C000_000000000046);
/// IUnknown::Release: the 3rd method of every COM object.
const RELEASE_SLOT: usize = 2;
/// IShellFolder::EnumObjects: the 5th method (after IUnknown's three and ParseDisplayName).
const ENUM_OBJECTS_SLOT: usize = 4;
/// IEnumIDList::Next: the 4th method (after IUnknown's three).
const NEXT_SLOT: usize = 3;
type Release = unsafe extern "system" fn(*mut c_void) -> u32;
type EnumObjects = unsafe extern "system" fn(*mut c_void, HWND, u32, *mut *mut c_void) -> HRESULT;
type Next = unsafe extern "system" fn(*mut c_void, u32, *mut *mut ITEMIDLIST, *mut u32) -> HRESULT;

/// The list as last read (empty until the first reading is done).
static APPS: Mutex<Vec<App>> = Mutex::new(Vec::new());
/// When it was last read.
static READ_AT: Mutex<Option<Instant>> = Mutex::new(None);
/// A thread is reading it right now.
static READING: AtomicBool = AtomicBool::new(false);
/// How many apps the last logged reading found, plus one (0: nothing logged yet).
static LOGGED: AtomicUsize = AtomicUsize::new(0);

/// A reading younger than this is fresh: the Start menu opening does not start another.
const FRESH: Duration = Duration::from_secs(30);

/// The Store-style apps as last read.
pub fn cached() -> Vec<App> {
    APPS.lock().map(|apps| apps.clone()).unwrap_or_default()
}

/// Reads the list on this thread (COM must be usable on it) and remembers it.
pub fn read_now() -> Vec<App> {
    let started = Instant::now();
    let (apps, listed) = read();
    // A line when the number changes (the first reading included), not for every reading.
    if LOGGED.swap(apps.len() + 1, Ordering::SeqCst) != apps.len() + 1 {
        let took = started.elapsed().as_millis();
        log(&format!("apps: Windows lists {listed} apps, {} of them Store-style (read in {took} ms)", apps.len()));
    }
    if let Ok(mut cache) = APPS.lock() {
        cache.clone_from(&apps);
    }
    if let Ok(mut at) = READ_AT.lock() {
        *at = Some(Instant::now());
    }
    apps
}

/// Reads the list again on a thread of its own, unless one is at it or the last reading is
/// fresh.
pub fn refresh() {
    let fresh = READ_AT.lock().is_ok_and(|at| at.is_some_and(|at| at.elapsed() < FRESH));
    if fresh || READING.swap(true, Ordering::SeqCst) {
        return;
    }
    let spawned = std::thread::Builder::new().name("apps list".into()).spawn(|| {
        crate::audio::init();
        read_now();
        READING.store(false, Ordering::SeqCst);
    });
    if let Err(e) = spawned {
        READING.store(false, Ordering::SeqCst);
        log(&format!("could not start reading Windows' list of apps: {e}"));
    }
}

/// Releases a COM object.
///
/// # Safety
/// `object` must be a live COM interface pointer that the caller holds a reference to.
unsafe fn release(object: *mut c_void) {
    // SAFETY: as the caller promises; Release is every COM object's third method.
    unsafe { method::<Release>(object, RELEASE_SLOT)(object) };
}

/// One of the names of the item at `id` (a full item list), as the shell gives it.
///
/// # Safety
/// `id` must be a valid, full item list.
unsafe fn name_of(id: *const ITEMIDLIST, which: SIGDN) -> Option<String> {
    let mut text: PWSTR = null_mut();
    // SAFETY: the shell allocates the string; it is read up to its terminating null and then
    // freed with CoTaskMemFree, as the API requires.
    unsafe {
        if SHGetNameFromIDList(id, which, &mut text) != 0 || text.is_null() {
            return None;
        }
        let len = (0..).take_while(|&i| *text.add(i) != 0).count();
        let name = String::from_utf16_lossy(std::slice::from_raw_parts(text, len));
        CoTaskMemFree(text as *const c_void);
        Some(name)
    }
}

/// Windows' list of apps: its Store-style apps, sorted by name, and how many entries the list
/// has in all. The other entries are programs with a shortcut file in the Start Menu folders,
/// which the Programs menu reads from there, in their folders.
fn read() -> (Vec<App>, usize) {
    let mut apps = Vec::new();
    let mut listed = 0;
    // SAFETY: documented shell calls with valid in and out pointers. Every item list the shell
    // hands out is freed (ILFree, CoTaskMemFree), every COM reference released; the two COM
    // methods are called through their documented places in the method tables.
    unsafe {
        let mut folder_id: *mut ITEMIDLIST = null_mut();
        let hr = SHGetKnownFolderIDList(&FOLDERID_AppsFolder, 0, null_mut(), &mut folder_id);
        if hr != 0 || folder_id.is_null() {
            log(&format!("apps: Windows' list of apps was not found (error {hr:#x}): no Store-style apps in Programs"));
            return (apps, listed);
        }
        let mut folder: *mut c_void = null_mut();
        let hr = SHBindToObject(null_mut(), folder_id, null_mut(), &IID_SHELL_FOLDER, &mut folder);
        if hr != 0 || folder.is_null() {
            log(&format!("apps: Windows' list of apps could not be opened (error {hr:#x})"));
            ILFree(folder_id);
            return (apps, listed);
        }
        let mut list: *mut c_void = null_mut();
        let hr =
            method::<EnumObjects>(folder, ENUM_OBJECTS_SLOT)(folder, null_mut(), SHCONTF_NONFOLDERS as u32, &mut list);
        if hr == 0 && !list.is_null() {
            let next = method::<Next>(list, NEXT_SLOT);
            loop {
                let (mut child, mut fetched): (*mut ITEMIDLIST, u32) = (null_mut(), 0);
                if next(list, 1, &mut child, &mut fetched) != 0 || child.is_null() {
                    break;
                }
                listed += 1;
                let full = ILCombine(folder_id, child);
                CoTaskMemFree(child as *const c_void);
                if full.is_null() {
                    continue;
                }
                if let Some(id) = name_of(full, SIGDN_PARENTRELATIVEPARSING).filter(|id| is_store_app(id))
                    && let Some(name) = name_of(full, SIGDN_NORMALDISPLAY).filter(|name| !name.trim().is_empty())
                {
                    apps.push(App { name, target: format!("{APP_PREFIX}{id}") });
                }
                ILFree(full);
            }
            release(list);
        } else {
            log(&format!("apps: Windows' list of apps could not be read (error {hr:#x})"));
        }
        release(folder);
        ILFree(folder_id);
    }
    apps.sort_by_key(|app| app.name.to_lowercase());
    (apps, listed)
}
