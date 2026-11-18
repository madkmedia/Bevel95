//! "Remove Shortcuts/Folders": the window that Taskbar Properties' Remove... button opens, as
//! in Windows 95, 98 and 2000. `bevel_core::remove` draws it and handles its keys and mouse
//! (`RemoveWindow`, a window of the control kit); this module opens it over Taskbar Properties
//! and carries out Remove and Close.
//!
//! The tree shows what the Programs menu shows now. Windows deleted the selected shortcut
//! file; here Remove takes the program out of what Bevel95's Programs menu shows and deletes
//! nothing (the owner, 2026-11-11): from then on the menu shows only the ticked programs, the
//! removed ones without their ticks, and Bevel95 Settings' Programs page can tick them again.

use std::time::Instant;

use bevel_core::programs::Shown;
use bevel_core::remove::{self, Part, RemoveWindow};
use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect;

use crate::bar::{self, with_app};
use crate::kit::{self, Hosted};
use crate::log::log;
use crate::properties_window::{self, PropertiesHost};
use crate::start_menu;
use crate::window_list::launch;

/// The window, while it is open (one at a time).
pub struct RemoveHost {
    form: RemoveWindow,
    /// How many ticks of `SCROLL_TIMER` a scroll bar button has been held down.
    held: u32,
}

/// The timer that keeps a held scroll bar button scrolling (the window host keeps 1 for the
/// caret): after a quarter of a second, then at every tick, as in Bevel95 Settings' list.
const SCROLL_TIMER: usize = 2;
const TICK_MS: u32 = 50;
const HOLD_TICKS: u32 = 5;

impl Hosted for RemoveHost {
    type F = RemoveWindow;
    fn form(&self) -> &RemoveWindow {
        &self.form
    }
    fn form_mut(&mut self) -> &mut RemoveWindow {
        &mut self.form
    }
    fn activate(part: Part) {
        activate(part);
    }
    fn timers() -> &'static [(usize, u32)] {
        &[(SCROLL_TIMER, TICK_MS)]
    }
    fn timer(&mut self, id: usize) -> bool {
        if id != SCROLL_TIMER {
            return false;
        }
        match self.form.pressed {
            Some(Part::ScrollUp | Part::ScrollDown) => {
                self.held += 1;
                self.held > HOLD_TICKS && remove::scroll_on(&mut self.form)
            }
            _ => {
                self.held = 0;
                false
            }
        }
    }
}

/// Opens the window over Taskbar Properties, which waits until it is closed.
pub fn open() {
    let Some(owner) = kit::hwnd::<PropertiesHost>() else {
        return;
    };
    let Some((era, shown)) = with_app(|app| (app.era, app.programs_shown)) else {
        return;
    };
    let started = Instant::now();
    let rows = remove::rows(&start_menu::programs(shown), |item| start_menu::file_icon(era, &item.target));
    let (count, took) = (rows.len(), started.elapsed().as_millis());
    let form = RemoveWindow::new(era, rows);
    if kit::open_dialog(owner, RemoveHost { form, held: 0 }, |w, h| place(owner, w, h)).is_some() {
        log(&format!("opened Remove Shortcuts/Folders: its tree has {count} rows (read in {took} ms)"));
    }
}

/// Where the window goes: as Windows put it, 33 to the right of and 50 below the top-left
/// corner of Taskbar Properties (measured on the Reference PCs), kept on the screen.
fn place(owner: HWND, w: i32, h: i32) -> (i32, i32) {
    let mut at = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    // SAFETY: a valid place for the answer; the window is our own.
    if unsafe { GetWindowRect(owner, &mut at) } == 0 {
        log("Remove Shortcuts/Folders: GetWindowRect(Taskbar Properties) failed");
    }
    with_app(|app| {
        let (s, m) = (app.scale, app.monitor);
        let x = (at.left + 33 * s).min(m.right - w).max(m.left);
        let y = (at.top + 50 * s).min(m.bottom - h).max(m.top);
        (x, y)
    })
    .unwrap_or((at.left, at.top))
}

/// Closes the window (Taskbar Properties can be used again).
pub fn close() {
    kit::close::<RemoveHost>();
}

/// The bar's look or size changed: the window, if open, closes (its pictures and its place
/// belong to the old one; Remove... opens it again in the new look).
pub fn look_changed() {
    if kit::hwnd::<RemoveHost>().is_some() {
        log("Remove Shortcuts/Folders closed: the look or size changed");
        close();
    }
}

/// The Programs menu changed while the window may be open (Add...'s Create Shortcut wizard made
/// a shortcut): its tree is read again, keeping its open folders and its selection.
pub fn programs_changed() {
    if kit::hwnd::<RemoveHost>().is_none() {
        return;
    }
    let Some((era, shown)) = with_app(|app| (app.era, app.programs_shown)) else {
        return;
    };
    let rows = remove::rows(&start_menu::programs(shown), |item| start_menu::file_icon(era, &item.target));
    log(&format!("Remove Shortcuts/Folders: its tree was read again ({} rows)", rows.len()));
    kit::update::<RemoveHost>(|h| h.form.set_rows(rows));
}

/// Carries out a push button (or the title bar's buttons).
fn activate(part: Part) {
    match part {
        Part::Close | Part::TitleClose => close(),
        Part::TitleHelp => launch("https://support.microsoft.com/windows"),
        Part::Remove => {
            let places = kit::with::<RemoveHost, _>(|h| h.form.selected_places()).unwrap_or_default();
            if places.is_empty() {
                return;
            }
            hide(&places);
            kit::update::<RemoveHost>(|h| h.form.remove_selected());
        }
        _ => {}
    }
}

/// Takes the programs at `places` out of what the Programs menu shows: what it shows now
/// becomes the ticked programs, without these (programs.txt), and the menu shows only the
/// ticked ones from now on. Open windows that show the choice follow.
fn hide(places: &[String]) {
    let Some(shown) = with_app(|app| app.programs_shown) else {
        return;
    };
    let (left, all) = start_menu::hide_programs(shown, places);
    log(&format!("Remove: {} taken out of the Programs menu; {left} of {all} programs are ticked", places.len()));
    bar::set_programs_shown(Shown::Ticked);
    properties_window::programs_removed(places);
}
