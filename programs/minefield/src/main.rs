//! Minesweeper's Windows side. The game itself (rules, pictures,
//! board, window, dialog boxes) is `bevel_core::minefield`, and the window host, the menus and
//! the look come from `bevel_winkit`, shared with the bar.
//!
//! It draws in the look and size Bevel95 is set to (settings.txt in Bevel95's folder) and
//! follows the bar when either changes. It keeps what it remembers in minefield.txt next to it.
#![windows_subsystem = "windows"]

mod sound;

use std::ptr::null_mut;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use bevel_core::kit::Form;
use bevel_core::minefield::art;
use bevel_core::minefield::board::Board;
use bevel_core::minefield::dialogs::{self, Act, Kind as BoxKind, Sheet};
use bevel_core::minefield::form::{self, Command, MENUS, MinefieldWindow, Part, TITLE};
use bevel_core::minefield::game::{Field, Level, State};
use bevel_core::minefield::html_help_catalog;
use bevel_core::minefield::scores::Settings;
use bevel_core::theme::Era;
use bevel_winkit::help::{HelpViewer, Target};
use bevel_winkit::kit::{self, Hosted, Kind};
use bevel_winkit::log::{data_dir, log};
use bevel_winkit::look;
use bevel_winkit::menu;
use bevel_winkit::util::{rect_h, rect_w, wide};
use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromRect, MonitorFromWindow,
};
use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, GetWindowRect, GetWindowThreadProcessId, IsIconic, MB_ICONERROR, MB_OK, MessageBoxW, SW_MINIMIZE, SW_RESTORE, SWP_NOACTIVATE,
    SWP_NOSIZE, SWP_NOZORDER, SetForegroundWindow, SetWindowPos, ShowWindow,
};

/// The game's clock timer (the host keeps id 1 for a text field's caret).
const CLOCK: usize = 2;

/// The time played: counted only while the game runs and its window is not minimized.
#[derive(Default)]
struct Clock {
    before: Duration,
    since: Option<Instant>,
}

impl Clock {
    fn start(&mut self) {
        self.before = Duration::ZERO;
        self.since = Some(Instant::now());
    }
    fn pause(&mut self) {
        if let Some(since) = self.since.take() {
            self.before += since.elapsed();
        }
    }
    fn resume(&mut self) {
        self.since.get_or_insert_with(Instant::now);
    }
    fn seconds(&self) -> u32 {
        let elapsed = self.before + self.since.map_or(Duration::ZERO, |since| since.elapsed());
        (elapsed.as_secs() + 1).min(999) as u32
    }
}

/// The game's main window and what belongs to it.
struct Game {
    form: MinefieldWindow,
    settings: Settings,
    clock: Clock,
    help: HelpViewer,
}

impl Game {
    fn play_sound(&self, cue: sound::Cue) {
        if self.form.era == Era::Win2000 && self.form.sound {
            sound::play(cue);
        }
    }
}

impl Hosted for Game {
    type F = MinefieldWindow;

    fn form(&self) -> &MinefieldWindow {
        &self.form
    }
    fn form_mut(&mut self) -> &mut MinefieldWindow {
        &mut self.form
    }
    fn activate(part: Part) {
        match part {
            Part::Minimize => {
                if let Some(hwnd) = kit::hwnd::<Game>() {
                    // SAFETY: our own window.
                    unsafe { ShowWindow(hwnd, SW_MINIMIZE) };
                }
            }
            Part::Close => kit::close::<Game>(),
            Part::Menu(first) => menu_bar(first),
            Part::Icon => window_menu(false, false),
            Part::TitleMenu => window_menu(true, false),
            Part::KeyMenu => window_menu(false, true),
            Part::Ask(command) => run(command, true),
            Part::Started => {
                kit::with::<Game, _>(|game| {
                    game.clock.start();
                    game.play_sound(sound::Cue::Tick);
                });
            }
            Part::Ended => ended(),
            Part::Maximize | Part::Mascot | Part::Field => {}
        }
    }
    fn timers() -> &'static [(usize, u32)] {
        &[(CLOCK, 100)]
    }
    fn timer(&mut self, id: usize) -> bool {
        let changed = id == CLOCK
            && self.form.game.state() == State::Playing
            && self.form.set_seconds(self.clock.seconds());
        if changed {
            self.play_sound(sound::Cue::Tick);
        }
        changed
    }
    /// Where the window was is remembered for the next start (not while it is minimized: then
    /// the last place noted stays).
    fn closing(&mut self, hwnd: HWND) {
        self.help.close();
        let mut r = RECT::default();
        // SAFETY: our own window, a valid out-pointer.
        if unsafe { IsIconic(hwnd) == 0 && GetWindowRect(hwnd, &mut r) != 0 } {
            self.settings.window = Some((r.left, r.top));
        }
        save(self);
    }
    /// The clock stands still while the window is minimized.
    fn minimized(&mut self, minimized: bool) {
        if minimized {
            self.clock.pause();
        } else if self.form.game.state() == State::Playing {
            self.clock.resume();
        }
    }
    /// The bar changed its look or size: the game follows.
    fn look_changed() {
        let Some((era, look)) = look::load_or_log() else {
            return;
        };
        // A box open in the old look goes away.
        kit::close::<GameBox>();
        let changed = kit::with::<Game, _>(|game| {
            game.help.close();
            let changed = game.form.era != era;
            game.form.set_era(era);
            changed
        });
        if changed == Some(true) {
            sound::stop();
            kit::set_program_icons::<Game>(&art::icon(era, 16), &art::icon(era, 32));
        }
        kit::retheme_with::<Game>(look, |w, h, now| on_screen((now.left, now.top), w, h));
    }
}

/// One of the game's dialog boxes (one at a time, above the main window).
struct GameBox {
    form: Sheet,
}

fn open_help(era: Era, command: Command) {
    let (Some(owner), Some(look)) = (kit::hwnd::<Game>(), kit::look_of::<Game>()) else { return };
    let target = match command {
        Command::UsingHelp => Target::UsingHelp,
        Command::SearchHelp => Target::Index,
        _ => Target::Contents,
    };
    if let Some(Err(error)) = kit::with::<Game, _>(|game| game.help.show(owner, era, look.scale, html_help_catalog::catalog(era), target)) {
        log(&format!("Minesweeper Help could not open: {error}"));
        let message = format!("Windows Help could not open.\n\n{error}");
        // SAFETY: the game's window and two strings that live through this call.
        unsafe { MessageBoxW(owner, wide(&message).as_ptr(), wide("Minesweeper Help").as_ptr(), MB_ICONERROR | MB_OK) };
    }
}

fn about(era: Era) -> Sheet {
    let mut status = MEMORYSTATUSEX { dwLength: size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
    // SAFETY: the caller supplies the structure size and a valid writable buffer.
    let total_kib = (unsafe { GlobalMemoryStatusEx(&mut status) } != 0).then_some(status.ullTotalPhys / 1024);
    Sheet::about(era, total_kib)
}

impl Hosted for GameBox {
    type F = Sheet;

    fn form(&self) -> &Sheet {
        &self.form
    }
    fn form_mut(&mut self) -> &mut Sheet {
        &mut self.form
    }
    fn request_close() {
        Self::activate(dialogs::Part::Close);
    }
    fn activate(part: dialogs::Part) {
        let Some((kind, act, field, text)) = kit::with::<GameBox, _>(|b| {
            (b.form.kind, b.form.act(part), b.form.custom_field(), b.form.text().to_string())
        }) else {
            return;
        };
        match (kind, act) {
            (_, None) => {}
            (_, Some(Act::Help(text))) => {
                if let Some(hwnd) = kit::hwnd::<GameBox>() { bevel_winkit::help::context_help(hwnd, text); }
            }
            (BoxKind::Custom, Some(action @ (Act::Ok | Act::Cancel))) => {
                kit::close::<GameBox>();
                kit::update::<Game>(|game| {
                    let previous = game.form.field();
                    // Choosing Custom starts a new game. Cancel keeps the previous dimensions.
                    let field = if action == Act::Ok { field } else { previous };
                    game.form.play_custom(field);
                });
                fit_window();
                save_now();
            }
            (BoxKind::BestTimes, Some(Act::Reset)) => {
                kit::with::<Game, _>(|game| {
                    game.settings.reset_best();
                    save(game);
                });
                kit::update::<GameBox>(|b| b.form.reset_scores());
            }
            // However the box is closed, the time stands; the name is what the field says.
            (BoxKind::NewBest(level, seconds), Some(_)) => {
                kit::close::<GameBox>();
                kit::with::<Game, _>(|game| game.settings.record(level, seconds, &text));
                save_now();
                run(Command::BestTimes, false);
            }
            _ => kit::close::<GameBox>(),
        }
    }
}

/// The settings file, next to Bevel95's settings.txt.
fn settings_path() -> std::path::PathBuf {
    data_dir().join("minefield.txt")
}

/// Writes what the game remembers.
fn save(game: &mut Game) {
    game.settings.level = game.form.level;
    game.settings.custom = game.form.custom;
    game.settings.marks = game.form.marks;
    game.settings.color = game.form.color;
    game.settings.sound = game.form.sound;
    let path = settings_path();
    if let Err(e) = bevel_winkit::state_file::save(&path, &game.settings.to_text())
        && !bevel_winkit::state_file::is_read_only(&e)
    {
        log(&format!("Minefield: could not save {}: {e}", bevel_winkit::log::path_kind(&path)));
    }
}

fn save_now() {
    kit::with::<Game, _>(save);
}

/// The part of its screen a window may use (the screen without the taskbar's strip).
fn work_area(monitor: windows_sys::Win32::Graphics::Gdi::HMONITOR) -> RECT {
    let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
    // SAFETY: a valid out-pointer; any monitor handle.
    unsafe { GetMonitorInfoW(monitor, &mut info) };
    info.rcWork
}

/// Where a window of `w` by `h` goes that wants to be at `at`: there, moved onto the nearest
/// screen if part of it would be off it.
fn on_screen(at: (i32, i32), w: i32, h: i32) -> (i32, i32) {
    let wanted = RECT { left: at.0, top: at.1, right: at.0 + w, bottom: at.1 + h };
    // SAFETY: a valid rectangle.
    let work = work_area(unsafe { MonitorFromRect(&wanted, MONITOR_DEFAULTTONEAREST) });
    (at.0.min(work.right - w).max(work.left), at.1.min(work.bottom - h).max(work.top))
}

/// After the field's size changed: the window takes its new size, and stays on the screen.
fn fit_window() {
    kit::resize::<Game>();
    let Some(hwnd) = kit::hwnd::<Game>() else {
        return;
    };
    let mut r = RECT::default();
    // SAFETY: our own window.
    unsafe {
        if GetWindowRect(hwnd, &mut r) == 0 {
            return;
        }
        let (x, y) = on_screen((r.left, r.top), rect_w(&r), rect_h(&r));
        if (x, y) != (r.left, r.top) {
            SetWindowPos(hwnd, null_mut(), x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
        }
    }
}

/// The largest field the game's screen can show at the size things are drawn in.
fn most_squares(era: Era, hwnd: Option<HWND>, scale: bevel_core::scale::Scale) -> (usize, usize) {
    let work = match hwnd {
        // SAFETY: our own window.
        Some(hwnd) => work_area(unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) }),
        None => look::main_screen().0,
    };
    Board::most_squares(era, (scale.old_fit(rect_w(&work)), scale.old_fit(rect_h(&work))))
}

/// Carries out a command of the menus (or F1).
fn run(command: Command, keyboard: bool) {
    let mut done = false;
    kit::update::<Game>(|game| {
        done = game.form.command(command);
        let restart = matches!(command, Command::NewGame | Command::Level(_));
        if done && (restart || (command == Command::Sound && !game.form.sound)) {
            sound::stop();
        }
    });
    if done {
        if matches!(command, Command::Level(_) | Command::NewGame) {
            fit_window();
        }
        save_now();
        return;
    }
    let Some((era, field, settings)) =
        kit::with::<Game, _>(|game| (game.form.era, game.form.field(), game.settings.clone()))
    else {
        return;
    };
    match command {
        Command::Custom => {
            let scale = kit::look_of::<Game>().map_or(bevel_core::scale::Scale::ONE, |look| look.scale);
            open_box(Sheet::custom(era, field, most_squares(era, kit::hwnd::<Game>(), scale)), keyboard);
        }
        Command::BestTimes => open_box(Sheet::best_times(era, &settings), keyboard),
        Command::HowToPlay | Command::SearchHelp | Command::UsingHelp => open_help(era, command),
        Command::About => open_box(about(era), keyboard),
        Command::Exit => kit::close::<Game>(),
        Command::NewGame | Command::Level(_) | Command::Marks | Command::Color | Command::Sound => {}
    }
}

/// Places the original dialog templates relative to the owner's client area.
fn open_box(mut sheet: Sheet, keyboard: bool) {
    let (Some(owner), Some(look)) = (kit::hwnd::<Game>(), kit::look_of::<Game>()) else {
        return;
    };
    let mut around = RECT::default();
    // SAFETY: our own window.
    unsafe { GetWindowRect(owner, &mut around) };
    if keyboard {
        sheet.show_keys();
    }
    let (dx, dy) = match (sheet.era, sheet.kind) {
        (_, BoxKind::About) => (33, 74),
        (Era::Win2000, BoxKind::Custom) => (3, 41),
        (Era::Win2000, BoxKind::BestTimes | BoxKind::NewBest(..)) => (3, 87),
        (_, BoxKind::Custom | BoxKind::BestTimes | BoxKind::NewBest(..)) => (27, 67),
    };
    let scale = look.scale;
    let place = |w: i32, h: i32| {
        on_screen((around.left + scale.at(dx), around.top + scale.at(dy)), w, h)
    };
    if kit::open_as(Kind::Dialog { owner }, GameBox { form: sheet }, look, place).is_none() {
        log("Minefield: a box could not be shown");
    }
}

/// The game was won or lost: the clock stops where it is, and a best time gets its name.
fn ended() {
    let Some((era, won, level, seconds, beats, name)) = kit::with::<Game, _>(|game| {
        let seconds = game.clock.seconds();
        game.clock.pause();
        game.form.game.stop_clock(seconds);
        let level = game.form.level;
        let won = game.form.game.state() == State::Won;
        game.play_sound(if won { sound::Cue::Win } else { sound::Cue::Lose });
        let name = game.settings.best(level).map_or(Settings::ANONYMOUS, |best| best.name.as_str()).to_owned();
        (game.form.era, won, level, seconds, game.settings.beats(level, seconds), name)
    }) else {
        return;
    };
    kit::update::<Game>(|_| {});
    if won && beats {
        open_box(Sheet::new_best(era, level, seconds, &name), false);
    }
}

fn menu_bar(first: usize) {
    let menu = menu::MenuBar {
        count: MENUS.len(),
        title_rect: MinefieldWindow::title_rect,
        menu: |form, title| (form.era, form.menu(title)),
        show_menu: MinefieldWindow::show_menu,
        back_to_bar: MinefieldWindow::back_to_bar,
        reached_by_keyboard: MinefieldWindow::reached_by_keyboard,
        selection: None,
    };
    if let Some(choice) = menu::track_bar::<Game>(first, menu)
        && let Some(command) = form::command_at(choice.era, choice.title, choice.row)
    {
        run(command, choice.keyboard);
    }
}

fn window_menu(at_pointer: bool, keyboard: bool) {
    menu::track_window::<Game>(Part::Icon, |form| form.era, at_pointer, keyboard);
}

/// The kind of this game's mark of its copy (`copy::mark`).
const COPY_MARK: &str = "Minesweeper";

/// If the game of this copy is running already, its window comes to the front instead of a second
/// one. Only this copy's (its process holds the copy's mark: the fifth safety review of
/// 2026-11-18, B1: any window titled Minesweeper of Bevel95's class counted, so a start of one
/// copy brought another copy's game forward, whose best times are kept in that copy's folder).
fn already_running() -> bool {
    let (class, title) = (wide(kit::KIT_CLASS), wide(TITLE));
    let mut after = std::ptr::null_mut();
    // SAFETY: null-terminated strings; `after` is null or a window this loop just found; the
    // window found is only shown and brought forward.
    unsafe {
        let found = loop {
            after = FindWindowExW(std::ptr::null_mut(), after, class.as_ptr(), title.as_ptr());
            if after.is_null() {
                return false;
            }
            let mut pid = 0;
            if GetWindowThreadProcessId(after, &mut pid) != 0 && bevel_winkit::copy::is_of_this_copy(COPY_MARK, pid) {
                break after;
            }
        };
        if IsIconic(found) != 0 {
            ShowWindow(found, SW_RESTORE);
        }
        SetForegroundWindow(found);
        true
    }
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
    if already_running() {
        return;
    }
    // This process is this copy's Minesweeper, for a later start's `already_running`.
    let _mark = bevel_winkit::copy::mark(COPY_MARK);
    let Some((era, paints)) = look::load_or_log() else {
        return;
    };
    let settings = Settings::parse(era, &bevel_winkit::state_file::load(&settings_path()).text().unwrap_or_default());
    // The custom field as remembered, if this screen can still show it.
    let most = most_squares(era, None, paints.scale);
    let custom = Field::custom(era, settings.custom.width, settings.custom.height, settings.custom.mines, most);
    let level =
        if settings.level == Level::Custom && custom != settings.custom { Level::Beginner } else { settings.level };
    // Where the mines go differs from start to start (for tests, BEVEL95_MINEFIELD_SEED fixes
    // the first game's: tools\dev\minefield.ps1 plays a known field through).
    let fixed = std::env::var("BEVEL95_MINEFIELD_SEED").ok().and_then(|seed| seed.parse::<u64>().ok());
    let seed = fixed.unwrap_or_else(|| {
        SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64) ^ u64::from(std::process::id())
    });
    let mut form = MinefieldWindow::new(era, level, custom, settings.marks, seed);
    form.color = settings.color;
    form.sound = settings.sound;
    let saved = settings.window;
    let kind = Kind::Program { small: Some(art::icon(era, 16)), big: Some(art::icon(era, 32)) };
    let scale = paints.scale;
    let place = |w: i32, h: i32| {
        let (work, _) = look::main_screen();
        let (x, y) = if era == Era::Win2000 { (80, 80) } else { (76, 36) };
        let fresh = (work.left + scale.at(x), work.top + scale.at(y));
        on_screen(saved.unwrap_or(fresh), w, h)
    };
    if kit::open_as(kind, Game { form, settings, clock: Clock::default(), help: HelpViewer::default() }, paints, place).is_none() {
        log("Minefield: its window could not be made");
        return;
    }
    kit::run_until_closed::<Game>();
}
