//! Windows' own Create Shortcut wizard (`rundll32.exe appwiz.cpl,NewLinkHere <file>`), as Explorer's
//! New > Shortcut started it: on an empty "New Shortcut.lnk" made first in the folder (the wizard
//! opens only for a file that exists, given without quotation marks). On Cancel the wizard removes
//! the empty file itself; an empty one left behind is removed here, a file with anything in it
//! never. Shared by the bar (Taskbar Properties' Add..., the desktop's New) and the folder windows.

use std::fs;
use std::path::{Path, PathBuf};
use std::ptr::null;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;
use windows_sys::Win32::System::Threading::{CreateProcessW, INFINITE, PROCESS_INFORMATION, STARTUPINFOW, WaitForSingleObject};

use crate::log::log;
use crate::util::wide;

/// New > Shortcut in the folder `dir`: the wizard on an empty placeholder there. The shortcut
/// stays where the wizard puts it; an empty placeholder left behind is removed.
pub fn new_shortcut_in(dir: &Path) {
    let Some(placeholder) = make_placeholder(dir) else {
        return;
    };
    let Some(process) = start_wizard(&placeholder) else {
        remove_if_empty(&placeholder);
        return;
    };
    crate::file_ops::log("New > Shortcut: the Create Shortcut wizard was started");
    let process = process as usize; // a handle is not Send; the thread alone uses it from now on
    let spawned = std::thread::Builder::new().name("create shortcut".into()).spawn(move || {
        // SAFETY: the process handle is ours (CreateProcessW) and closed once, here.
        unsafe {
            WaitForSingleObject(process as HANDLE, INFINITE);
            CloseHandle(process as HANDLE);
        }
        remove_if_empty(&placeholder);
    });
    if let Err(e) = spawned {
        crate::file_ops::log(&format!("could not start the thread that waits for the wizard: {e}"));
    }
}
/// "New Shortcut.lnk", or "New Shortcut (2).lnk" and so on if that is taken, made empty (made/// new: an existing file is never touched).
pub fn make_placeholder(dir: &Path) -> Option<PathBuf> {
    for n in 1..100 {
        let name = if n == 1 { "New Shortcut.lnk".to_string() } else { format!("New Shortcut ({n}).lnk") };
        let path = dir.join(name);
        match fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(_) => return Some(path),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                log(&format!("Add...: could not make the empty placeholder for the wizard: {e}"));
                return None;
            }
        }
    }
    log("Add...: no free name for the wizard's placeholder (New Shortcut (2) to (99) are taken)");
    None
}

/// Removes the placeholder if it is still there and empty (a file with anything in it is a
/// shortcut, and stays).
pub fn remove_if_empty(path: &Path) {
    match fs::metadata(path) {
        Ok(meta) if meta.len() == 0 => match fs::remove_file(path) {
            Ok(()) => log("Add...: removed the wizard's empty placeholder"),
            Err(e) => log(&format!("Add...: could not remove the wizard's empty placeholder: {e}")),
        },
        _ => {}
    }
}

/// Starts `rundll32.exe appwiz.cpl,NewLinkHere <placeholder>` (the path without quotation
/// marks) and returns its process.
pub fn start_wizard(placeholder: &Path) -> Option<HANDLE> {
    let mut system = [0u16; 260];
    // SAFETY: the buffer and its length.
    let len = unsafe { GetSystemDirectoryW(system.as_mut_ptr(), system.len() as u32) } as usize;
    if len == 0 || len >= system.len() {
        log("Add...: GetSystemDirectoryW failed");
        return None;
    }
    let system = String::from_utf16_lossy(&system[..len]);
    let rundll32 = format!("{system}\\rundll32.exe");
    // The panel by its whole path too (rundll32 takes "file,entry" unquoted, so only a system
    // folder without spaces can be named so; Windows' is one). The third safety review of
    // 2026-11-18, C1.
    let appwiz = if system.contains(' ') { "appwiz.cpl".to_string() } else { format!("{system}\\appwiz.cpl") };
    let exe = wide(&rundll32);
    let mut line = wide(&format!("\"{rundll32}\" {appwiz},NewLinkHere {}", placeholder.display()));
    let startup = STARTUPINFOW { cb: size_of::<STARTUPINFOW>() as u32, ..Default::default() };
    let mut info = PROCESS_INFORMATION::default();
    // SAFETY: null-terminated strings (the command line writable, as the call requires), the
    // startup information and a place for the answer, all alive through the call.
    let ok = unsafe {
        CreateProcessW(exe.as_ptr(), line.as_mut_ptr(), null(), null(), 0, 0, null(), null(), &startup, &mut info)
    };
    if ok == 0 {
        log(&format!("Add...: could not start the Create Shortcut wizard: {}", std::io::Error::last_os_error()));
        return None;
    }
    // SAFETY: the thread handle is ours and not needed.
    unsafe { CloseHandle(info.hThread) };
    Some(info.hProcess)
}
