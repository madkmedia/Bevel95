//! Taskbar Properties' Add...: Windows' own Create Shortcut wizard, as Windows 95 to 2000
//! started it, making the shortcut in your Programs folder. What the wizard makes joins
//! Bevel95's Programs menu whatever the menu shows (`start_menu::show_programs`).
//!
//! The wizard (`rundll32.exe appwiz.cpl,NewLinkHere <file>`) opens only for a file that already
//! exists, given without quotation marks (the rest of the command line is the path, spaces
//! included). Explorer's own New > Shortcut does the same: it first makes an empty
//! "New Shortcut.lnk" and hands that to the wizard. On Cancel the wizard removes the empty file
//! itself (tried on this PC, `private/probes/add-wizard.ps1`); an empty one left behind is removed
//! here, a file with anything in it never.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::Ordering;

use bevel_core::programs::Shown;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, HWND, WAIT_OBJECT_0};
use windows_sys::Win32::System::Threading::{INFINITE, WaitForSingleObject};
use windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW;

use crate::bar::{self, BAR_HWND, WM_SHORTCUT_WIZARD_ENDED, with_app};
use crate::log::log;
use crate::{properties_window, start_menu};

/// The files a wizard that has ended added to the Programs folder, one list per wizard, until
/// the bar's thread takes them (`wizard_ended`). Several wizards can be open at once.
static ENDED: Mutex<Vec<Vec<PathBuf>>> = Mutex::new(Vec::new());

/// Add...: makes an empty placeholder with a free name in your Programs folder and starts the
/// Create Shortcut wizard on it. A thread of its own waits for the wizard to end.
pub fn start() {
    let Some(dir) = start_menu::programs_folder() else {
        log("Add...: the Programs folder was not found");
        return;
    };
    let before = start_menu::program_files(&dir);
    let Some(placeholder) = make_placeholder(&dir) else {
        return;
    };
    let Some(process) = start_wizard(&placeholder) else {
        remove_if_empty(&placeholder);
        return;
    };
    log("Add...: the Create Shortcut wizard was started");
    let process = process as usize; // a handle is not Send; the thread alone uses it from now on
    let spawned = std::thread::Builder::new().name("create shortcut".into()).spawn(move || {
        let process = process as HANDLE;
        // SAFETY: the process handle is ours (CreateProcessW) and closed once, here.
        let waited = unsafe {
            let waited = WaitForSingleObject(process, INFINITE);
            CloseHandle(process);
            waited
        };
        if waited != WAIT_OBJECT_0 {
            log(&format!("Add...: waiting for the wizard failed ({waited}); looking at the folder now"));
        }
        remove_if_empty(&placeholder);
        let added: Vec<PathBuf> = start_menu::program_files(&dir).difference(&before).cloned().collect();
        log(&format!("Add...: the wizard ended; {} new file(s) in the Programs folder", added.len()));
        if let Ok(mut ended) = ENDED.lock() {
            ended.push(added);
        }
        let bar = BAR_HWND.load(Ordering::SeqCst) as HWND;
        // SAFETY: posting to the bar's own window (null if the bar is gone: then nothing).
        if !bar.is_null() && unsafe { PostMessageW(bar, WM_SHORTCUT_WIZARD_ENDED.id(), 0, 0) } == 0 {
            log("Add...: could not tell the bar that the wizard ended (PostMessageW failed)");
        }
    });
    if let Err(e) = spawned {
        log(&format!("Add...: could not start the thread that waits for the wizard: {e}"));
    }
}


/// The desktop's New > Shortcut: the wizard on an empty placeholder in the owner's Desktop folder
/// (`bevel_winkit::shortcut_wizard`, shared with the folder windows).
pub fn start_on_desktop() {
    match crate::desktop_items::desktop_folder() {
        Some(dir) => bevel_winkit::shortcut_wizard::new_shortcut_in(&dir),
        None => log("desktop: New > Shortcut: the Desktop folder was not found"),
    }
}

use bevel_winkit::shortcut_wizard::{make_placeholder, remove_if_empty, start_wizard};


/// On the bar's thread, after a wizard ended: what it made joins the Programs menu, whatever
/// the menu shows, and open windows follow.
pub fn wizard_ended() {
    let lists: Vec<Vec<PathBuf>> = ENDED.lock().map(|mut ended| std::mem::take(&mut *ended)).unwrap_or_default();
    let added: BTreeSet<PathBuf> = lists.into_iter().flatten().filter(|path| path.is_file()).collect();
    if added.is_empty() {
        return;
    }
    let added: Vec<PathBuf> = added.into_iter().collect();
    let places = start_menu::places_of(&added);
    if places.is_empty() {
        log(&format!("Add...: none of the {} new file(s) is an entry of the Programs menu", added.len()));
        return;
    }
    let Some(shown) = with_app(|app| app.programs_shown) else {
        return;
    };
    match start_menu::show_programs(shown, &places) {
        None => log(&format!("Add...: {} new program(s); the menu shows them already", places.len())),
        Some((now, all)) => {
            log(&format!("Add...: {} new program(s) ticked; {now} of {all} programs are ticked", places.len()));
            bar::set_programs_shown(Shown::Ticked);
        }
    }
    let menu_shows = with_app(|app| app.programs_shown).unwrap_or(shown);
    properties_window::programs_added(&places, menu_shows);
    crate::remove_window::programs_changed();
}
