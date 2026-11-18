//! Quick Search (until 2026-11-16 Find: Programs), Bevel95's own window for starting a
//! program by a few letters of its name, opened by the Windows key (`windows_key = find`, the
//! default), the Start menu's Find submenu and `BarCommand::Find`. `bevel_core::find` draws it
//! and handles its keys and mouse (`FindWindow`, a window of the control kit); this module keeps
//! the list of programs, fetches the pictures of the rows that show and starts the chosen one.
//!
//! The window appears at once: it shows the list of programs as last read, and reads it again
//! on a thread of its own each time it opens (Windows' list of apps too, as the Start menu
//! does). The first opening before any reading reads it right away (as the Start menu does each
//! time it opens).

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use bevel_core::find::{self, FindWindow, Part, Program};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Gdi::{CreateRectRgn, SetWindowRgn};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    HWND_NOTOPMOST, PostMessageW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos,
};

use bevel_core::quick::Action;
use bevel_core::theme::{Depth, Era};

use crate::bar::{BAR_HWND, WM_BAR_COLORS, WM_BAR_COMMAND, WM_BAR_SIZE, WM_BAR_THEME, with_app};
use crate::kit::{self, Hosted};
use crate::log::log;
use crate::util::take_focus;
use crate::{start_menu, window_list};

/// Posted to the bar when a reading of the programs (on its own thread) is done.
pub const WM_FIND_PROGRAMS_READ: u32 = windows_sys::Win32::UI::WindowsAndMessaging::WM_APP + 12;

/// The programs as last read (`None` until the first reading), and whether a reading is going on.
static PROGRAMS: Mutex<Option<Vec<Program>>> = Mutex::new(None);
static READING: AtomicBool = AtomicBool::new(false);

/// The window, while it is open.
pub struct FindHost {
    form: FindWindow,
    /// How many ticks of `TIMER` a scroll bar button has been held down.
    held: u32,
}

/// The window's timer (the host keeps 1 for the caret): it fetches the pictures of rows that
/// came into view, and keeps a held scroll bar button scrolling after a quarter of a second.
const TIMER: usize = 2;
const TICK_MS: u32 = 50;
const HOLD_TICKS: u32 = 5;

impl Hosted for FindHost {
    type F = FindWindow;
    fn form(&self) -> &FindWindow {
        &self.form
    }
    fn form_mut(&mut self) -> &mut FindWindow {
        &mut self.form
    }
    fn activate(part: Part) {
        activate(part);
    }
    fn timers() -> &'static [(usize, u32)] {
        &[(TIMER, TICK_MS)]
    }
    fn timer(&mut self, id: usize) -> bool {
        if id != TIMER {
            return false;
        }
        let pictures = fetch_icons(&mut self.form);
        let scrolled = match self.form.pressed {
            Some(Part::ScrollUp | Part::ScrollDown) => {
                self.held += 1;
                self.held > HOLD_TICKS && find::scroll_on(&mut self.form)
            }
            _ => {
                self.held = 0;
                false
            }
        };
        pictures || scrolled
    }
}

/// Fetches the pictures of the rows that show and have none yet (from the Start menu's store
/// of them, read ahead soon after the bar starts). True if any was fetched.
fn fetch_icons(form: &mut FindWindow) -> bool {
    let wanted = form.wanted_icons();
    for &i in &wanted {
        let icon = start_menu::file_icon(form.era, &form.programs[i].target);
        form.set_icon(i, icon);
    }
    !wanted.is_empty()
}

/// Reads every program the Programs menu could show (with "everything" chosen, whatever the
/// owner's choice for the menu is). Bevel95's own are marked: they come first among equals.
fn read_programs() -> Vec<Program> {
    let mut programs = find::programs(&start_menu::all_programs());
    for program in &mut programs {
        program.own = start_menu::is_own_program(&program.target);
    }
    programs
}

/// Reads the programs again on a thread of its own (not if a reading is going on); the bar
/// hears when it is done (`WM_FIND_PROGRAMS_READ`).
pub fn refresh() {
    if READING.swap(true, Ordering::SeqCst) {
        return;
    }
    let spawned = std::thread::Builder::new().name("find programs".into()).spawn(|| {
        crate::audio::init();
        let programs = read_programs();
        if let Ok(mut known) = PROGRAMS.lock() {
            *known = Some(programs);
        }
        READING.store(false, Ordering::SeqCst);
        let bar = BAR_HWND.load(Ordering::SeqCst) as HWND;
        // SAFETY: posting to our own bar window (a stale handle just fails).
        unsafe { PostMessageW(bar, WM_FIND_PROGRAMS_READ, 0, 0) };
    });
    if let Err(e) = spawned {
        READING.store(false, Ordering::SeqCst);
        log(&format!("could not start reading the programs for Quick Search: {e}"));
    }
}

/// A reading is done: an open window shows the programs as read now (its text, its pictures
/// and its selected program stay).
pub fn programs_read() {
    if kit::hwnd::<FindHost>().is_none() {
        return;
    }
    let Some(programs) = PROGRAMS.lock().ok().and_then(|known| known.clone()) else {
        return;
    };
    let with_commands = find::with_commands(programs.clone());
    let same = kit::with::<FindHost, _>(|h| h.form.programs == with_commands).unwrap_or(true);
    if !same {
        log(&format!("Quick Search: the list of programs was read again ({} programs)", programs.len()));
        kit::update::<FindHost>(|h| h.form.set_programs(programs));
    }
}

/// Whether the window is open and the one in front.
pub fn in_front() -> bool {
    kit::hwnd::<FindHost>().is_some_and(|hwnd| window_list::foreground() == hwnd)
}

/// Opens the window, empty, where the Run dialog opens; if it is open already, it comes
/// forward as it is.
pub fn open() {
    if let Some(hwnd) = kit::hwnd::<FindHost>() {
        take_focus(hwnd);
        return;
    }
    let Some((era, monitor_left, bar_top, scale)) =
        with_app(|app| (app.era, app.monitor.left, app.bar_top_on_screen(), app.scale.max(1)))
    else {
        return;
    };
    let started = Instant::now();
    let known = PROGRAMS.lock().ok().and_then(|known| known.clone());
    let programs = match known {
        Some(programs) => programs,
        None => {
            let programs = read_programs();
            if let Ok(mut known) = PROGRAMS.lock() {
                *known = Some(programs.clone());
            }
            programs
        }
    };
    let count = programs.len();
    let mut form = FindWindow::new(era, programs, true);
    fetch_icons(&mut form);
    // Where Run opens: in the lower left, above the Start button.
    let opened = kit::open(FindHost { form, held: 0 }, |_, h| (monitor_left + 12 * scale, bar_top - h - 12 * scale));
    if let Some(hwnd) = opened {
        // Not above every window, as the bar's other windows are: another program brought
        // forward covers it, and the Windows key brings it back.
        // SAFETY: our own window; only its place among the windows changes.
        if unsafe { SetWindowPos(hwnd, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) } == 0 {
            log("Quick Search: SetWindowPos(not topmost) failed");
        }
        log(&format!("opened Quick Search: {count} programs (ready in {} ms)", started.elapsed().as_millis()));
    }
    crate::apps::refresh();
    refresh();
}

/// Closes the window.
pub fn close() {
    kit::close::<FindHost>();
}

/// The bar's look or size changed: an open window closes (its pictures belong to the old look).
pub fn look_changed() {
    if kit::hwnd::<FindHost>().is_some() {
        close();
    }
}

/// Carries out a command found (`bevel_core::quick`): posted to the bar as the menu entry that
/// does the same would post it, so it happens once this window is gone.
fn run_command(action: Action) {
    let bar = BAR_HWND.load(Ordering::SeqCst) as HWND;
    log(&format!("Quick Search: {action:?}"));
    let (msg, wparam) = match action {
        Action::Bar(command) => (WM_BAR_COMMAND.id(), command.code()),
        Action::Look(era) => (WM_BAR_THEME.id(), Era::ALL.iter().position(|&e| e == era).unwrap_or(0)),
        Action::Size(scale) => (WM_BAR_SIZE.id(), scale as usize),
        Action::Colors(depth) => (WM_BAR_COLORS.id(), Depth::ALL.iter().position(|&d| d == depth).unwrap_or(1)),
    };
    // SAFETY: posting to our own bar window.
    unsafe { PostMessageW(bar, msg, wparam, 0) };
}

/// Carries out Enter, a double-click and the title bar's "X".
fn activate(part: Part) {
    match part {
        Part::TitleClose => close(),
        Part::Open => {
            let Some((target, command)) =
                kit::with::<FindHost, _>(|h| h.form.selected().map(|p| (p.target.clone(), p.command))).flatten()
            else {
                return;
            };
            // A thing to do: the window closes, then the bar does it as its menus would.
            if let Some(action) = command {
                close();
                run_command(action);
                return;
            }
            // As the Programs menu starts its entries: the window goes off the screen at once
            // but keeps the focus while the program starts (so the program may come to the
            // front), then closes without taking the focus back.
            if let Some(hwnd) = kit::hwnd::<FindHost>() {
                // SAFETY: our own window; the system owns the region from here on.
                unsafe { SetWindowRgn(hwnd, CreateRectRgn(0, 0, 0, 0), 1) };
            }
            let started = Instant::now();
            window_list::launch(&target);
            let took = started.elapsed().as_millis();
            if took > 250 {
                log(&format!("starting a program from Quick Search took {took} ms (its window was already gone)"));
            }
            close();
        }
        _ => {}
    }
}
