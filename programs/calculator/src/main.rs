//! Calculator's Windows side. The Calculator itself (what it computes and shows, its window,
//! its About box) is `bevel_core::calculator`; the window host, the menus and the look come from
//! `bevel_winkit`, shared with the bar and the games.
//!
//! It draws in the look and size Bevel95 is set to and follows the bar when either changes. It
//! keeps what the real one kept in WIN.INI (the view, Windows 2000's Digit grouping) in
//! calculator.txt next to settings.txt. Like the real one, several can be open at once.
#![windows_subsystem = "windows"]

use std::ptr::null_mut;

use bevel_winkit::state_file;

use bevel_core::calculator::art;
use bevel_core::calculator::dialogs::{About, Part as AboutPart};
use bevel_core::calculator::form::{CalculatorWindow, Command, MENUS, Part};
use bevel_core::calculator::help;
use bevel_core::calculator::settings::Settings;
use bevel_core::calculator::stats::{self, Part as StatsPart, StatsBox as StatsForm};
use bevel_winkit::help::{HelpViewer, Target};
use bevel_winkit::kit::{self, Hosted, Kind};
use bevel_winkit::log::{data_dir, log};
use bevel_winkit::look;
use bevel_winkit::menu;
use bevel_winkit::util::wide;
use windows_sys::Win32::Foundation::{GlobalFree, HWND, RECT};
use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromRect};
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, IsClipboardFormatAvailable, OpenClipboard, SetClipboardData,
};
use windows_sys::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows_sys::Win32::System::Ole::CF_UNICODETEXT;
use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CW_USEDEFAULT, CreateWindowExW, DestroyWindow, GetWindowRect, MB_ICONERROR, MB_OK, MessageBoxW, SW_MINIMIZE,
    SetForegroundWindow, ShowWindow, WS_EX_NOACTIVATE, WS_OVERLAPPEDWINDOW,
};

/// The flash timer: a key's button comes up again after `form::FLASH_TICKS` of these.
const FLASH: usize = 2;

/// The Calculator's window and what belongs to it.
struct Calc {
    form: CalculatorWindow,
    settings: Settings,
    /// Help > Help Topics: this look's book in the shared Help viewer.
    help: HelpViewer,
}

impl Hosted for Calc {
    type F = CalculatorWindow;

    fn form(&self) -> &CalculatorWindow {
        &self.form
    }
    fn form_mut(&mut self) -> &mut CalculatorWindow {
        &mut self.form
    }
    fn activate(part: Part) {
        match part {
            Part::Minimize => {
                if let Some(hwnd) = kit::hwnd::<Calc>() {
                    // SAFETY: our own window.
                    unsafe { ShowWindow(hwnd, SW_MINIMIZE) };
                }
            }
            Part::Close => kit::close::<Calc>(),
            Part::Menu(first) => menu_bar(first),
            Part::Icon => window_menu(false, false),
            Part::TitleMenu => window_menu(true, false),
            Part::KeyMenu => window_menu(false, true),
            Part::Ask(command) => run(command, true),
            Part::Maximize | Part::Button(_) | Part::Radio(_) | Part::Check(_) => {}
        }
    }
    fn timers() -> &'static [(usize, u32)] {
        &[(FLASH, 50)]
    }
    fn timer(&mut self, id: usize) -> bool {
        if id != FLASH {
            return false;
        }
        let changed = self.form.tick();
        // The Statistics Box follows what Dat and the rest did to the numbers.
        let lines = self.form.engine.stats_lines();
        if kit::with::<StatsBox, _>(|b| b.form.lines() != lines.as_slice()) == Some(true) {
            kit::update::<StatsBox>(|b| b.form.set_lines(lines));
        }
        changed
    }
    fn closing(&mut self, _hwnd: HWND) {
        self.help.close();
        save(self);
        kit::close::<StatsBox>();
    }
    /// The bar changed its look or size: the Calculator follows (the number shown stays).
    fn look_changed() {
        let (era, look) = look::load();
        kit::close::<AboutBox>();
        kit::close::<StatsBox>();
        let changed = kit::with::<Calc, _>(|calc| {
            // A book open in the old look goes away (as the games' do).
            calc.help.close();
            let changed = calc.form.era != era;
            calc.form.set_era(era);
            changed
        });
        if changed == Some(true) {
            kit::set_program_icons::<Calc>(&art::icon(era, 16), &art::icon(era, 32));
        }
        kit::retheme_with::<Calc>(look, |w, h, now| on_screen((now.left, now.top), w, h));
    }
}

/// About Calculator, above the Calculator.
struct AboutBox {
    form: About,
}

impl Hosted for AboutBox {
    type F = About;

    fn form(&self) -> &About {
        &self.form
    }
    fn form_mut(&mut self) -> &mut About {
        &mut self.form
    }
    fn request_close() {
        kit::close::<AboutBox>();
    }
    fn activate(part: AboutPart) {
        if matches!(part, AboutPart::Ok | AboutPart::Close) {
            kit::close::<AboutBox>();
        }
    }
}

/// The Statistics Box (Sta), beside the Calculator: its own window with a taskbar button.
struct StatsBox {
    form: StatsForm,
}

impl Hosted for StatsBox {
    type F = StatsForm;

    fn form(&self) -> &StatsForm {
        &self.form
    }
    fn form_mut(&mut self) -> &mut StatsForm {
        &mut self.form
    }
    fn request_close() {
        kit::close::<StatsBox>();
    }
    fn activate(part: StatsPart) {
        let selected = kit::with::<StatsBox, _>(|b| b.form.selected).flatten();
        match part {
            StatsPart::Close => kit::close::<StatsBox>(),
            StatsPart::Help | StatsPart::List => {}
            StatsPart::Icon => menu::track_window::<StatsBox>(StatsPart::Icon, |form| form.era, false, false),
            // RET: the Calculator has the keyboard again (the box stays open).
            StatsPart::Ret => {
                if let Some(hwnd) = kit::hwnd::<Calc>() {
                    // SAFETY: our own window.
                    unsafe { SetForegroundWindow(hwnd) };
                }
            }
            StatsPart::Load => {
                if let Some(i) = selected {
                    kit::update::<Calc>(|calc| {
                        calc.form.settle();
                        calc.form.engine.stats_load(i);
                    });
                }
            }
            StatsPart::Cd => {
                if let Some(i) = selected {
                    kit::update::<Calc>(|calc| calc.form.engine.stats_remove(i));
                    sync_stats();
                }
            }
            StatsPart::Cad => {
                kit::update::<Calc>(|calc| calc.form.engine.stats_clear());
                sync_stats();
            }
        }
    }
    /// Closed: its numbers are gone, and 98 and 2000 gray Ave, Sum, s and Dat again.
    fn closed(&mut self) {
        kit::update::<Calc>(|calc| calc.form.engine.close_stats());
    }
}

/// Sta: the Statistics Box opens where it always opened (or comes to the front).
fn open_stats() {
    let (Some(look), Some(era)) = (kit::look_of::<Calc>(), kit::with::<Calc, _>(|c| c.form.era)) else {
        return;
    };
    if let Some(hwnd) = kit::hwnd::<StatsBox>() {
        // SAFETY: our own window.
        unsafe { SetForegroundWindow(hwnd) };
        return;
    }
    kit::update::<Calc>(|calc| calc.form.engine.open_stats());
    let scale = look.scale;
    let work = look::main_screen().0;
    let place = |w: i32, h: i32| on_screen((work.left + scale.at(stats::PLACE.0), work.top + scale.at(stats::PLACE.1)), w, h);
    let kind = Kind::Program { small: Some(stats::icon(era)), big: None };
    if kit::open_as(kind, StatsBox { form: StatsForm::new(era) }, look, place).is_none() {
        log("Calculator: the Statistics Box could not be shown");
    }
    sync_stats();
}

/// The Statistics Box shows the Calculator's numbers as they are now.
fn sync_stats() {
    let Some(lines) = kit::with::<Calc, _>(|calc| calc.form.engine.stats_lines()) else {
        return;
    };
    kit::update::<StatsBox>(|b| b.form.set_lines(lines));
}

/// The settings file, next to Bevel95's settings.txt.
fn settings_path() -> std::path::PathBuf {
    data_dir().join("calculator.txt")
}

/// Writes what the Calculator remembers.
fn save(calc: &mut Calc) {
    calc.settings.grouping = calc.form.grouping();
    calc.settings.scientific = calc.form.scientific();
    let path = settings_path();
    if let Err(e) = state_file::save(&path, &calc.settings.to_text())
        && !state_file::is_read_only(&e)
    {
        log(&format!("Calculator: could not save {}: {e}", bevel_winkit::log::path_kind(&path)));
    }
}

/// The part of the screen a window may use, of the screen nearest `wanted`.
fn work_area(wanted: &RECT) -> RECT {
    let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
    // SAFETY: a valid rectangle and out-pointer.
    if unsafe { GetMonitorInfoW(MonitorFromRect(wanted, MONITOR_DEFAULTTONEAREST), &mut info) } == 0 {
        return look::main_screen().0;
    }
    info.rcWork
}

/// Where a window of `w` by `h` goes that wants to be at `at`: there, moved onto the nearest
/// screen if part of it would be off it.
fn on_screen(at: (i32, i32), w: i32, h: i32) -> (i32, i32) {
    let work = work_area(&RECT { left: at.0, top: at.1, right: at.0 + w, bottom: at.1 + h });
    (at.0.min(work.right - w).max(work.left), at.1.min(work.bottom - h).max(work.top))
}

/// Where Windows' cascade puts a new window (the real Calculator asked for that place): asked
/// with an invisible overlapped window, as Solitaire does, since the painted frame is a popup.
fn default_origin(w: i32, h: i32) -> (i32, i32) {
    let mut rect = RECT::default();
    // SAFETY: a predefined window class, no owner; the window is destroyed right away.
    unsafe {
        let window = CreateWindowExW(
            WS_EX_NOACTIVATE,
            wide("STATIC").as_ptr(),
            std::ptr::null(),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            0,
            w,
            h,
            null_mut(),
            null_mut(),
            null_mut(),
            std::ptr::null(),
        );
        if !window.is_null() {
            let read = GetWindowRect(window, &mut rect) != 0;
            DestroyWindow(window);
            if read {
                return on_screen((rect.left, rect.top), w, h);
            }
        }
    }
    let work = look::main_screen().0;
    (work.left, work.top)
}

/// Whether the clipboard holds text (Edit > Paste is grayed when not).
fn clipboard_has_text() -> bool {
    // SAFETY: a plain query.
    unsafe { IsClipboardFormatAvailable(u32::from(CF_UNICODETEXT)) != 0 }
}

/// Puts `text` on the clipboard (Edit > Copy).
fn set_clipboard_text(owner: HWND, text: &str) {
    let units: Vec<u16> = text.encode_utf16().chain([0]).collect();
    // SAFETY: the clipboard is opened and closed here; the memory handed to it is a moveable
    // global block holding the null-terminated text, which the clipboard owns once set (and is
    // freed here if it was not taken).
    unsafe {
        if OpenClipboard(owner) == 0 {
            log("Calculator: the clipboard could not be opened");
            return;
        }
        EmptyClipboard();
        let block = GlobalAlloc(GMEM_MOVEABLE, units.len() * 2);
        if !block.is_null() {
            let p = GlobalLock(block) as *mut u16;
            if !p.is_null() {
                std::ptr::copy_nonoverlapping(units.as_ptr(), p, units.len());
                GlobalUnlock(block);
                if SetClipboardData(u32::from(CF_UNICODETEXT), block).is_null() {
                    log("Calculator: the clipboard did not take the text");
                    GlobalFree(block);
                }
            } else {
                GlobalFree(block);
            }
        }
        CloseClipboard();
    }
}

/// Carries out a command of the menus (or of Ctrl+C).
fn run(command: Command, keyboard: bool) {
    let mut done = false;
    let view = kit::with::<Calc, _>(|calc| calc.form.scientific());
    kit::update::<Calc>(|calc| done = calc.form.command(command));
    if done {
        if kit::with::<Calc, _>(|calc| calc.form.scientific()) != view {
            // The other view: the window takes its size (its corner stays), the Statistics
            // Box goes.
            kit::close::<StatsBox>();
            kit::resize::<Calc>();
        }
        sync_stats();
        kit::with::<Calc, _>(save);
        return;
    }
    match command {
        Command::Copy => {
            let (Some(owner), Some(text)) = (kit::hwnd::<Calc>(), kit::with::<Calc, _>(|c| c.form.copy_text())) else {
                return;
            };
            set_clipboard_text(owner, &text);
        }
        Command::Paste => {
            if let Some(text) = kit::clipboard_text() {
                kit::update::<Calc>(|calc| calc.form.paste_text(&text));
            }
        }
        Command::About => open_about(keyboard),
        Command::Statistics => open_stats(),
        Command::HelpTopics => open_help(),
        Command::Standard
        | Command::Scientific
        | Command::DigitGrouping
        | Command::Radix(_)
        | Command::Angle(_)
        | Command::Word(_) => {}
    }
}

/// Help > Help Topics: this look's book at its Contents (95: WinHelp's picker; 98 and 2000: HTML
/// Help). Asked again, the open viewer comes back.
fn open_help() {
    let (Some(owner), Some(look)) = (kit::hwnd::<Calc>(), kit::look_of::<Calc>()) else {
        return;
    };
    let shown = kit::with::<Calc, _>(|calc| {
        let era = calc.form.era;
        calc.help.show(owner, era, look.scale, help::catalog(era), Target::Contents)
    });
    if let Some(Err(error)) = shown {
        log(&format!("Calculator: Help could not open: {error}"));
        let message = wide(&format!("Cannot open Help for Calculator.\n\n{error}"));
        // SAFETY: our own window and two terminated strings that live through the call.
        unsafe { MessageBoxW(owner, message.as_ptr(), wide("Calculator").as_ptr(), MB_OK | MB_ICONERROR) };
    }
}

/// About Calculator, where the shell put it: 33 right of and 74 below the window's corner.
fn open_about(keyboard: bool) {
    let (Some(owner), Some(look), Some(era)) =
        (kit::hwnd::<Calc>(), kit::look_of::<Calc>(), kit::with::<Calc, _>(|c| c.form.era))
    else {
        return;
    };
    let mut status = MEMORYSTATUSEX { dwLength: size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
    // SAFETY: the structure's size is set and the buffer is writable.
    let total_kib = (unsafe { GlobalMemoryStatusEx(&mut status) } != 0).then_some(status.ullTotalPhys / 1024);
    let mut about = About::new(era, total_kib);
    if keyboard {
        bevel_core::kit::Form::show_keys(&mut about);
    }
    let mut around = RECT::default();
    // SAFETY: our own window.
    unsafe { GetWindowRect(owner, &mut around) };
    let scale = look.scale;
    let place = |w: i32, h: i32| on_screen((around.left + scale.at(33), around.top + scale.at(74)), w, h);
    if kit::open_as(Kind::Dialog { owner }, AboutBox { form: about }, look, place).is_none() {
        log("Calculator: About could not be shown");
    }
}

fn menu_bar(first: usize) {
    let can_paste = clipboard_has_text();
    kit::update::<Calc>(|calc| {
        calc.form.settle();
        calc.form.can_paste = can_paste;
    });
    let menu = menu::MenuBar {
        count: MENUS.len(),
        title_rect: CalculatorWindow::title_rect,
        menu: |form, title| (form.era, form.menu(title)),
        show_menu: CalculatorWindow::show_menu,
        back_to_bar: CalculatorWindow::back_to_bar,
        reached_by_keyboard: CalculatorWindow::reached_by_keyboard,
        selection: None,
    };
    if let Some(choice) = menu::track_bar::<Calc>(first, menu)
        && let Some(command) = kit::with::<Calc, _>(|calc| calc.form.command_at(choice.title, choice.row)).flatten()
    {
        run(command, choice.keyboard);
    }
}

fn window_menu(at_pointer: bool, keyboard: bool) {
    menu::track_window::<Calc>(Part::Icon, |form| form.era, at_pointer, keyboard);
}

fn main() {
    // Where this run keeps its data, decided once, before anything is read or logged.
    if bevel_winkit::log::program_start().is_none() {
        return;
    }
    // Never Bevel95's own folder as the working folder (decisions.md, "Folder windows").
    bevel_winkit::shell::start_in_profile();
    bevel_winkit::log::started();
    // A panic on a worker thread closes the windows: none is left looking alive (decisions.md).
    bevel_winkit::panic::install(bevel_winkit::panic::close_windows_after);
    look::dpi_aware();
    let (era, paints) = look::load();
    let settings = Settings::parse(&state_file::load(&settings_path()).text().unwrap_or_default());
    let mut form = CalculatorWindow::new(era);
    form.set_grouping(settings.grouping);
    form.set_scientific(settings.scientific);
    let kind = Kind::Program { small: Some(art::icon(era, 16)), big: Some(art::icon(era, 32)) };
    if kit::open_as(kind, Calc { form, settings, help: HelpViewer::default() }, paints, default_origin).is_none() {
        log("Calculator: its window could not be made");
        return;
    }
    kit::run_until_closed::<Calc>();
}
