//! The Windows host for the classic Solitaire table, its menus, dialogs and Help book.
#![windows_subsystem = "windows"]

mod dialogs;
mod instance;
mod menu;
mod settings;

use std::time::{SystemTime, UNIX_EPOCH};

use bevel_core::solitaire::form::{self, Part, SolitaireWindow};
use bevel_core::solitaire::game::{self, Game as Cards};
use bevel_core::solitaire::settings::Settings;
use bevel_core::solitaire::{art, help};
use bevel_core::theme::Era;
use bevel_winkit::help::{HelpViewer, Target};
use bevel_winkit::kit::{self, Hosted, Kind};
use bevel_winkit::log::{log, log_win32};
use bevel_winkit::look;
use bevel_winkit::util::wide;
use menu::Command;
use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    ClientToScreen, GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromRect,
};
use windows_sys::Win32::System::SystemInformation::GetTickCount;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CW_USEDEFAULT, CreateWindowExW, DestroyWindow, GetClientRect, GetForegroundWindow, GetWindowRect, IsZoomed,
    KillTimer, MB_ICONERROR, MB_OK, MessageBoxW, SW_MINIMIZE, SetCursorPos, SetTimer, ShowWindow, WS_EX_NOACTIVATE,
    WS_OVERLAPPEDWINDOW,
};

const CLOCK: usize = 2;
const VICTORY: usize = 3;

struct Game {
    form: SolitaireWindow,
    settings: Settings,
    help: HelpViewer,
    fixed_seed: Option<u64>,
    clock_ticks: u8,
    minimized: bool,
    scale: bevel_core::scale::Scale,
}

impl Game {
    fn deal(&mut self) {
        let seed = if let Some(seed) = &mut self.fixed_seed {
            *seed = seed.wrapping_add(1);
            *seed
        } else {
            deal_clock(self.form.era)
        };
        self.form.new_deal(seed, self.settings.rules);
        self.clock_ticks = 0;
    }
}

impl Hosted for Game {
    type F = SolitaireWindow;

    fn form(&self) -> &SolitaireWindow {
        &self.form
    }
    fn form_mut(&mut self) -> &mut SolitaireWindow {
        &mut self.form
    }

    fn activate(part: Part) {
        match part {
            Part::Menu(index) => menu::bar(index),
            Part::Icon => menu::system(false, false),
            Part::KeyMenu => menu::system(false, true),
            Part::TitleMenu => menu::system(true, false),
            Part::Ask(command) => run(command, true),
            Part::Close => kit::close::<Game>(),
            Part::Minimize => {
                if let Some(hwnd) = kit::hwnd::<Game>() {
                    // SAFETY: this is our own top-level window.
                    unsafe { ShowWindow(hwnd, SW_MINIMIZE) };
                }
            }
            Part::Maximize => kit::toggle_maximize::<Game>(),
            Part::MovePointer(x, y) => move_pointer(x, y),
            Part::Won => victory(),
            Part::Board => {}
        }
    }

    fn timers() -> &'static [(usize, u32)] {
        &[(CLOCK, 250)]
    }

    fn timer(&mut self, id: usize) -> bool {
        if id != CLOCK || self.minimized {
            return false;
        }
        if !self.form.game.started() || !self.form.game.rules().timed || self.form.game.won() {
            return false;
        }
        let back_changed = self.form.tick_back();
        self.clock_ticks += 1;
        if self.clock_ticks < 4 {
            return back_changed;
        }
        self.clock_ticks = 0;
        self.form.tick(1) || back_changed
    }

    fn minimized(&mut self, minimized: bool) {
        if self.minimized && !minimized {
            self.form.repaint_board();
        }
        self.minimized = minimized;
    }

    fn resized(&mut self, size: (i32, i32), maximized: bool) {
        self.form.set_size(size, maximized);
    }
    fn minimum_size(&self) -> (i32, i32) {
        bevel_core::solitaire::form::MIN_SIZE
    }

    fn look_changed() {
        let Some((era, paints)) = look::load_or_log() else { return };
        dialogs::close();
        let maximized = kit::hwnd::<Game>().and_then(|hwnd| {
            let mut r = RECT::default();
            // SAFETY: queries our own window and writes to a local rectangle.
            (unsafe { IsZoomed(hwnd) != 0 && GetWindowRect(hwnd, &mut r) != 0 }).then_some(r)
        });
        let changed = kit::with::<Game, _>(|game| {
            game.help.close();
            let changed = game.form.era != era;
            game.form.set_era(era);
            game.scale = paints.scale;
            if let Some(rect) = maximized {
                let scale = game.scale;
                game.form.set_size(
                    (scale.old_cover(rect.right - rect.left), scale.old_cover(rect.bottom - rect.top)),
                    true,
                );
            }
            changed
        });
        if changed == Some(true) {
            kit::set_program_icons::<Game>(&art::icon(era, 16), &art::icon(era, 32));
        }
        kit::retheme_with::<Game>(paints, |w, h, old| on_screen(old.left, old.top, w, h));
    }

    fn closing(&mut self, hwnd: HWND) {
        // SAFETY: the timer, if present, belongs to the window that is closing.
        unsafe { KillTimer(hwnd, VICTORY) };
        self.help.close();
        dialogs::close();
    }
}

fn move_pointer(x: i32, y: i32) {
    let (Some(owner), Some(scale)) = (kit::hwnd::<Game>(), kit::with::<Game, _>(|game| game.scale)) else { return };
    let mut at = POINT { x: scale.at(x), y: scale.at(y) };
    let mut client = RECT::default();
    // The original arrow keys point at the next pile. Only move within the visible part
    // of the focused table; the player can also have a menu or a dialog in front.
    // SAFETY: owner is this process's window, and both structures are writable.
    unsafe {
        if GetForegroundWindow() == owner
            && GetClientRect(owner, &mut client) != 0
            && (0..client.right).contains(&at.x)
            && (0..client.bottom).contains(&at.y)
            && ClientToScreen(owner, &mut at) != 0
        {
            SetCursorPos(at.x, at.y);
        }
    }
}

fn run(command: Command, keyboard: bool) {
    match command {
        Command::Deal => kit::update::<Game>(Game::deal),
        Command::Undo => kit::update::<Game>(|game| {
            game.form.game.undo();
        }),
        Command::Deck | Command::Options | Command::About => dialogs::open(command, keyboard),
        Command::HelpTopics | Command::SearchHelp | Command::UsingHelp => {
            let target = match command {
                Command::SearchHelp => Target::Index,
                Command::UsingHelp => Target::UsingHelp,
                _ => Target::Contents,
            };
            let Some(owner) = kit::hwnd::<Game>() else { return };
            if let Some(Err(error)) = kit::with::<Game, _>(|g| {
                let era = g.form.era;
                g.help.show(owner, era, g.scale, help::catalog(era), target)
            }) {
                log(&format!("Solitaire: Help could not open: {error}"));
                let message = wide(&format!("Cannot open Help for Solitaire.\n\n{error}"));
                // SAFETY: our owner HWND and terminated strings lasting through MessageBox.
                unsafe { MessageBoxW(owner, message.as_ptr(), wide("Solitaire").as_ptr(), MB_OK | MB_ICONERROR) };
            }
        }
        Command::Exit => kit::close::<Game>(),
    }
}

fn victory() {
    let Some(owner) = kit::hwnd::<Game>() else { return };
    kit::update::<Game>(|game| {
        game.clock_ticks = 0;
        game.form.start_win();
    });
    // The callback can open the final dialog after the form and its framebuffer have been released.
    // SAFETY: our timer is stopped when the animation or the window closes.
    if unsafe { SetTimer(owner, VICTORY, 16, Some(victory_frame)) } == 0 {
        log_win32("Solitaire victory timer");
        dialogs::deal_again();
    }
}

unsafe extern "system" fn victory_frame(hwnd: HWND, _: u32, _: usize, _: u32) {
    let mut finished = false;
    kit::update::<Game>(|game| {
        if !game.minimized {
            finished = !game.form.animate_win();
        }
    });
    if finished {
        // SAFETY: the timer is attached to this window by victory().
        unsafe { KillTimer(hwnd, VICTORY) };
        if kit::with::<Game, _>(|game| game.form.win_finished()) == Some(true) {
            dialogs::deal_again();
        }
    }
}

fn work_area(rect: &RECT) -> MONITORINFO {
    let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
    // SAFETY: rect and info both refer to valid data on the current thread.
    if unsafe { GetMonitorInfoW(MonitorFromRect(rect, MONITOR_DEFAULTTONEAREST), &mut info) } == 0 {
        info.rcWork = look::main_screen().0;
        info.rcMonitor = info.rcWork;
    }
    info
}

fn on_screen(x: i32, y: i32, w: i32, h: i32) -> (i32, i32) {
    let work = work_area(&RECT { left: x, top: y, right: x.saturating_add(w), bottom: y.saturating_add(h) }).rcWork;
    (x.min(work.right - w).max(work.left), y.min(work.bottom - h).max(work.top))
}

fn default_origin(w: i32, h: i32) -> (i32, i32) {
    // Solitaire asked Windows to place its overlapped window in the desktop cascade.
    // A popup like our painted frame cannot request CW_USEDEFAULT itself. Ask with
    // an invisible overlapped system control, then give the game that position.
    let mut rect = RECT::default();
    // SAFETY: a predefined window class and no owner, child, menu, or user data. The
    // handle never leaves this thread and is destroyed before the game window opens.
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
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null(),
        );
        if !window.is_null() {
            let read = GetWindowRect(window, &mut rect) != 0;
            DestroyWindow(window);
            if read {
                return on_screen(rect.left, rect.top, w, h);
            }
        }
    }
    let work = look::main_screen().0;
    (work.left, work.top)
}

fn deal_clock(era: Era) -> u64 {
    if era == Era::Win2000 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
    } else {
        // SAFETY: this query takes no pointers. Win16 SOL seeded from uptime in milliseconds.
        u64::from(unsafe { GetTickCount() })
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
    let Some((era, paints)) = look::load_or_log() else { return };
    if instance::existing(era) {
        return;
    }
    let fixed_seed = std::env::var("BEVEL95_SOLITAIRE_SEED").ok().and_then(|s| s.parse().ok());
    let settings = settings::load(game::first_rand(fixed_seed.unwrap_or_else(|| deal_clock(era))));
    let cards = Cards::new(fixed_seed.unwrap_or_else(|| deal_clock(era)), settings.rules);
    let mut form = SolitaireWindow::new(era, cards, usize::from(settings.deck), settings.status_bar, settings.outline);
    let scale = paints.scale;
    let work = look::main_screen().0;
    let (w, h) = form::default_size(era);
    form.set_size(
        (
            w.min(scale.old_fit(work.right - work.left).max(form::MIN_SIZE.0)),
            h.min(scale.old_fit(work.bottom - work.top).max(form::MIN_SIZE.1)),
        ),
        false,
    );
    let kind = Kind::ResizableProgram { small: Some(art::icon(era, 16)), big: Some(art::icon(era, 32)) };
    let game =
        Game { form, settings, help: HelpViewer::default(), fixed_seed, clock_ticks: 0, minimized: false, scale };
    let Some(hwnd) = kit::open_as(kind, game, paints, default_origin) else {
        log("Solitaire: could not open its main window");
        return;
    };
    instance::ready(hwnd);
    kit::run_until_closed::<Game>();
}
