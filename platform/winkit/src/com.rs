//! Calling COM objects through their method tables (windows-sys has no wrappers for them).

use std::ffi::c_void;
use std::ptr::null;

use windows_sys::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};

/// IUnknown's Release: the third method of every COM object.
const RELEASE: usize = 2;
type Release = unsafe extern "system" fn(*mut c_void) -> u32;

/// Method `slot` of a COM object.
///
/// # Safety
/// `object` must be a live COM interface pointer whose method table has `slot`, of type `T`.
pub unsafe fn method<T: Copy>(object: *mut c_void, slot: usize) -> T {
    // SAFETY: as the caller promises: the first word of a COM object is its method table.
    unsafe {
        let table = *(object as *const *const usize);
        std::mem::transmute_copy(&*table.add(slot))
    }
}

/// # Safety
/// `object` must be a live COM interface pointer we hold a reference to.
pub unsafe fn release(object: *mut c_void) {
    // SAFETY: as the caller promises.
    unsafe { method::<Release>(object, RELEASE)(object) };
}

/// Makes COM usable on the calling thread (the shell's icons, file information and windows need
/// it). Harmless to repeat.
pub fn init() {
    // SAFETY: plain call; an apartment-threaded thread fits a window thread.
    unsafe { CoInitializeEx(null(), COINIT_APARTMENTTHREADED as u32) };
}
