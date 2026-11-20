//! The two property sheets: Taskbar Properties ("Taskbar and Start Menu Properties" in Windows
//! 2000), with Windows' own pages, and Bevel95 Settings, with everything that is Bevel95's own.
//! `bevel_core::properties` draws both and handles their keys and mouse (`PropertiesWindow`, a
//! window of the control kit); this module opens them and carries out OK, Apply and the pages'
//! buttons. Applying is the bar's job (`bar::apply_properties`).
//!
//! Both can be open at once, so each puts only its own settings into effect
//! (`Sheet::merge`), and both follow the bar when its look or size changes (`look_changed`).

use std::ptr::null;
use std::time::Instant;

use bevel_core::programs::{self, RowKind, Shown};
use bevel_core::properties::{Page, Part, Properties, PropertiesWindow, Sheet, Values};
use windows_sys::Win32::UI::Shell::{SHARD_PIDL, SHAddToRecentDocs};

use crate::bar::{self, with_app};
use crate::kit::{self, Hosted};
use crate::log::log;
use crate::util::take_focus;
use crate::window_list::launch;
use crate::{autostart, start_menu};

/// One of the two windows, while it is open (one of each at a time). They are two kinds for the
/// window host and one form: `OWN` false is Taskbar Properties, true is Bevel95 Settings.
pub struct SheetHost<const OWN: bool> {
    form: PropertiesWindow,
    /// How many ticks of `LIST_TIMER` a scroll bar button of the list of programs has been
    /// held down.
    held: u32,
}

/// Taskbar Properties.
pub type PropertiesHost = SheetHost<false>;
/// Bevel95 Settings.
pub type SettingsHost = SheetHost<true>;

/// Bevel95 Settings' timer for its list of programs (the window host keeps 1 for the caret):
/// it reads the list once the Programs page is showing, and keeps a held scroll bar button
/// scrolling.
const LIST_TIMER: usize = 2;
const LIST_TICK_MS: u32 = 50;
/// A held scroll bar button scrolls on after this many ticks (a quarter of a second), then at
/// every tick.
const HOLD_TICKS: u32 = 5;

impl<const OWN: bool> SheetHost<OWN> {
    const SHEET: Sheet = if OWN { Sheet::Bevel95 } else { Sheet::Taskbar };
}

impl<const OWN: bool> Hosted for SheetHost<OWN> {
    type F = PropertiesWindow;
    fn form(&self) -> &PropertiesWindow {
        &self.form
    }
    fn form_mut(&mut self) -> &mut PropertiesWindow {
        &mut self.form
    }
    fn activate(part: Part) {
        activate::<OWN>(part);
    }
    fn timers() -> &'static [(usize, u32)] {
        // Only Bevel95 Settings has the list of programs.
        if OWN { &[(LIST_TIMER, LIST_TICK_MS)] } else { &[] }
    }
    fn timer(&mut self, id: usize) -> bool {
        let p = &mut self.form.props;
        if id != LIST_TIMER || p.current_page() != Page::Programs {
            return false;
        }
        if !p.programs.loaded {
            // The page is on the screen and says it is looking; now the programs are read.
            // Their pictures come from the Start menu's store of them (read ahead soon after
            // the bar starts); only before that is done does this take more than a moment.
            let started = Instant::now();
            let rows = start_menu::program_rows(p.era);
            let took = started.elapsed().as_millis();
            log(&format!("Bevel95 Settings: its list of programs has {} rows (read in {took} ms)", rows.len()));
            p.set_programs(rows);
            return true;
        }
        match p.pressed {
            Some(part @ (Part::ScrollUp | Part::ScrollDown)) => {
                self.held += 1;
                if self.held > HOLD_TICKS {
                    let before = p.programs.top;
                    p.scroll(if part == Part::ScrollUp { -1 } else { 1 });
                    return p.programs.top != before;
                }
            }
            _ => self.held = 0,
        }
        false
    }
}

/// The settings in effect (what a window starts out showing), and the look in use.
fn current_values() -> Option<(Values, bevel_core::theme::Era)> {
    let start_with_windows = autostart::is_on();
    with_app(|app| {
        let values = Values {
            options: app.options,
            start: app.start_options,
            era: app.era,
            colors: app.colors,
            scale: app.scale,
            start_with_windows,
            clock_offset: app.clock_offset,
            desktop: app.desktop,
            programs: app.programs_shown,
            windows_key: app.windows_key,
            group: app.group,
            quick_search_all: app.quick_search == bevel_core::config::QuickSearch::Everything,
            battery_percent: app.battery_percent,
        };
        (values, app.era)
    })
}

/// Opens Taskbar Properties (or brings it forward if it is already open).
pub fn open() {
    open_sheet::<false>();
}

/// Opens Bevel95 Settings (or brings it forward if it is already open).
pub fn open_settings() {
    open_sheet::<true>();
}

/// Opens Bevel95 Settings on `page` (or brings it forward and turns to that page).
pub fn open_settings_on(page: Page) {
    let Some(index) = Sheet::Bevel95.pages().iter().position(|&p| p == page) else {
        return;
    };
    open_sheet::<true>();
    kit::update::<SettingsHost>(|h| h.form.props.show_page(index));
}

fn open_sheet<const OWN: bool>() {
    if let Some(hwnd) = kit::hwnd::<SheetHost<OWN>>() {
        take_focus(hwnd);
        return;
    }
    let Some((values, era)) = current_values() else {
        return;
    };
    let sheet = SheetHost::<OWN>::SHEET;
    let mut props = Properties::new(sheet, era, values);
    if sheet == Sheet::Taskbar {
        props.documents_empty = start_menu::recent_documents(era).is_empty();
    }
    let Some(form) = with_app(|app| PropertiesWindow::new(props, &app.fonts())) else {
        return;
    };
    if kit::open(SheetHost::<OWN> { form, held: 0 }, place::<OWN>).is_some() {
        log(if OWN { "opened Bevel95 Settings" } else { "opened Taskbar Properties" });
    }
}

/// Where a window goes. Taskbar Properties: as Windows put it, at the left just above the
/// taskbar (in the v86 emulator: 2px from the screen's left edge, 4px above the bar). Bevel95
/// Settings (ours, PROVISIONAL): one step of a cascade further right and up, 22px, so that
/// neither window opens exactly over the other. Both are kept on the screen.
fn place<const OWN: bool>(w: i32, h: i32) -> (i32, i32) {
    with_app(|app| {
        let s = app.scale;
        let step = if OWN { 22 * s } else { 0 };
        let x = app.monitor.left + 2 * s + step;
        let y = (app.bar_top_on_screen() - 4 * s - step - h).max(app.monitor.top);
        (x.min(app.monitor.right - w).max(app.monitor.left), y)
    })
    .unwrap_or((0, 0))
}

/// Closes both windows (without applying anything), and first the window that Taskbar
/// Properties' Remove... button opened over it.
pub fn close() {
    crate::remove_window::close();
    kit::close::<PropertiesHost>();
    kit::close::<SettingsHost>();
}

/// After a window's Apply: it shows what is now in effect, and Apply grays out again. (A new
/// look or size has reached it already, through `look_changed`.)
fn refresh_after_apply<const OWN: bool>() {
    let Some((values, era)) = current_values() else {
        return;
    };
    kit::update::<SheetHost<OWN>>(|h| {
        let p = &mut h.form.props;
        p.values = values;
        p.changed = false;
        p.programs.edited = false;
        follow_era(p, era);
    });
}

/// The window is drawn in `era` from now on. The pages of another look may differ: it stays on
/// the same page, the focus on that page's first control. The list of programs is read again
/// (Bevel95's own programs have each look's own pictures) and keeps its ticks.
fn follow_era(p: &mut Properties, era: bevel_core::theme::Era) {
    if p.era != era {
        p.era = era;
        p.show_page(p.page);
        p.show_keys = p.show_keys || era.always_shows_keys();
        if p.programs.loaded {
            p.refresh_programs(start_menu::program_rows(era));
        }
    }
}

/// The bar's look or size changed (from either window, or from the bar menu's Theme and Size
/// entries): both windows, if open, are drawn that way from now on, and Bevel95 Settings shows
/// the new look or size as the one chosen (so its OK or Apply does not put the old one back).
pub fn look_changed() {
    crate::remove_window::look_changed();
    follow_bar::<false>();
    follow_bar::<true>();
}

fn follow_bar<const OWN: bool>() {
    let Some((in_effect, era)) = current_values() else {
        return;
    };
    // What the window is drawn with now (none: it is not open).
    let Some(drawn) = kit::look_of::<SheetHost<OWN>>() else {
        return;
    };
    let new_look = drawn.theme.slug != bevel_core::theme::theme(era).slug;
    let (new_colors, new_size) = (drawn.theme.depth != in_effect.colors, drawn.scale != in_effect.scale);
    if !new_look && !new_colors && !new_size {
        return;
    }
    kit::with::<SheetHost<OWN>, _>(|h| {
        let p = &mut h.form.props;
        // Only what changed: another choice made in the window and not applied yet stays.
        if new_look {
            p.values.era = in_effect.era;
        }
        if new_colors {
            p.values.colors = in_effect.colors;
        }
        if new_size {
            p.values.scale = in_effect.scale;
        }
        follow_era(p, era);
    });
    kit::retheme::<SheetHost<OWN>>(place::<OWN>);
}

/// Puts a window's own settings into effect; what the other window sets stays as it is in
/// effect now (not as it was when this window opened).
fn apply(p: &Properties) {
    if let Some((in_effect, _)) = current_values() {
        save_ticks(p, in_effect.programs);
        bar::apply_properties(p.sheet.merge(p.values, in_effect));
    }
}

/// Bevel95 Settings, with "Only the programs ticked here" chosen: the ticks go into
/// programs.txt, which the Start menu reads each time it opens. Written when a box was
/// flipped, and when the choice is new (or there is no file yet): from then on a program that
/// is installed later is one that was not ticked. With another choice the file is left as it
/// is, for the next time this one is chosen.
fn save_ticks(p: &Properties, in_effect: Shown) {
    let list = &p.programs;
    if p.sheet != Sheet::Bevel95 || p.values.programs != Shown::Ticked || !list.loaded {
        return;
    }
    if list.edited || in_effect != Shown::Ticked || start_menu::ticked_programs().is_none() {
        let places: Vec<&str> = programs::ticked_places(&list.rows).collect();
        let all = list.rows.iter().filter(|row| row.kind == RowKind::Item).count();
        log(&format!("programs: {} of {all} programs are ticked", places.len()));
        start_menu::save_ticked_programs(places, in_effect);
    }
}

/// Carries out a push button (or the title bar's buttons).
fn activate<const OWN: bool>(part: Part) {
    let Some(p) = kit::with::<SheetHost<OWN>, _>(|h| h.form.props.clone()) else {
        return;
    };
    if !p.enabled(part) {
        return;
    }
    match part {
        Part::TitleClose | Part::Cancel => kit::close::<SheetHost<OWN>>(),
        Part::TitleHelp => launch("https://support.microsoft.com/windows"),
        Part::Ok => {
            if p.changed {
                apply(&p);
            }
            kit::close::<SheetHost<OWN>>();
        }
        Part::Apply => {
            apply(&p);
            refresh_after_apply::<OWN>();
        }
        // Windows 95's Add... started the Create Shortcut wizard in the Programs folder; it
        // still exists, and makes the shortcut where it is told to (`add_shortcut`).
        Part::Add => crate::add_shortcut::start(),
        // Windows' own "Remove Shortcuts/Folders" window, over this one (`remove_window`).
        Part::Remove => crate::remove_window::open(),
        Part::Advanced => match start_menu::start_menu_folder() {
            Some(dir) => launch(&dir.display().to_string()),
            None => log("Advanced...: the Start Menu folder was not found"),
        },
        // The Programs menu is always in alphabetical order here: nothing to re-sort.
        Part::ReSort => log("Re-sort: the Programs menu is always sorted"),
        Part::Clear => {
            // The documented way to empty the recent documents list (Windows' own Clear).
            // SAFETY: a null item means "clear all".
            unsafe { SHAddToRecentDocs(SHARD_PIDL as u32, null()) };
            log("Clear: emptied the recent documents list");
            kit::update::<SheetHost<OWN>>(|h| {
                let p = &mut h.form.props;
                p.documents_empty = true;
                if p.focus == Part::Clear {
                    p.focus = Part::Ok;
                }
            });
        }
        // Takes effect on OK or Apply, like the page's other choices; the button grays out.
        Part::UsePcTime => kit::update::<SheetHost<OWN>>(|h| {
            let p = &mut h.form.props;
            p.use_pc_time();
            if p.focus == Part::UsePcTime {
                p.focus = Part::Ok;
            }
        }),
        _ => {}
    }
}

/// Date/Time Properties set Bevel95's clock: an open Bevel95 Settings window shows the new
/// setting (so its OK or Apply does not put back the one it opened with).
pub fn clock_offset_changed(offset: i64) {
    kit::update::<PropertiesHost>(|h| h.form.props.values.clock_offset = offset);
    kit::update::<SettingsHost>(|h| h.form.props.values.clock_offset = offset);
}

/// "Remove Shortcuts/Folders" took programs out of the Programs menu, which shows only the
/// ticked ones from now on: an open Bevel95 Settings window shows that choice and those
/// programs without their ticks (so its OK or Apply does not put them back).
pub fn programs_removed(places: &[String]) {
    kit::update::<PropertiesHost>(|h| h.form.props.programs_removed(places));
    kit::update::<SettingsHost>(|h| h.form.props.programs_removed(places));
}

/// Add...'s Create Shortcut wizard made the programs at `places`, and the Programs menu shows
/// `menu_shows` now: an open window's list (if read) is read again so they are in it, keeping
/// its ticks, and with the ticked choice in effect it shows that choice and these ticked (so its
/// OK or Apply does not take them out again).
pub fn programs_added(places: &[String], menu_shows: Shown) {
    fn follow<const OWN: bool>(places: &[String], menu_shows: Shown) {
        kit::update::<SheetHost<OWN>>(|h| {
            let p = &mut h.form.props;
            if p.programs.loaded {
                p.refresh_programs(start_menu::program_rows(p.era));
            }
            p.programs_added(places, menu_shows);
        });
    }
    follow::<false>(places, menu_shows);
    follow::<true>(places, menu_shows);
}

/// The desktop's own menu switched Bevel95's desktop off: an open Bevel95 Settings window
/// shows that (so its OK or Apply does not switch it back on).
pub fn desktop_changed(on: bool) {
    kit::update::<PropertiesHost>(|h| h.form.props.values.desktop = on);
    kit::update::<SettingsHost>(|h| h.form.props.values.desktop = on);
}

