//! A Windows Help viewer for books supplied by Bevel95 programs over their private pipe.
//! The executable has no built-in program catalog.
#![windows_subsystem = "windows"]

mod actions;
mod dialogs;
mod manual;
mod print;

use std::io::{self, Read, Write};
use std::ptr::null_mut;
use std::sync::mpsc;

use bevel_core::help::winhelp::{HelpTab, HelpTopics, Part as ClassicPart};
use bevel_core::help::{Catalog, wire};
use bevel_core::html_help::{HtmlHelp, Part, Tab};
use bevel_core::kit::Form;
use bevel_core::scale::{Scale, Way};
use bevel_core::theme::Era;
use bevel_winkit::help::Target;
use bevel_winkit::kit::{self, Hosted, Kind, Look};
use bevel_winkit::log::log;
use bevel_winkit::look;
use bevel_winkit::util::{rect_h, rect_w, wide};
use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromWindow,
};
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, EnumThreadWindows, GUITHREADINFO, GetGUIThreadInfo, GetLastActivePopup, GetMessageW,
    GetWindowRect, GetWindowThreadProcessId, IsIconic, IsZoomed, MSG, PostMessageW, PostThreadMessageW, SW_MAXIMIZE,
    SW_MINIMIZE, SW_RESTORE, SetForegroundWindow, SetWindowTextW, ShowWindow, TranslateMessage, WM_CLOSE, WM_NULL,
};

#[derive(Clone, Copy)]
struct Caller {
    era: Era,
    scale: Scale,
    owner: HWND,
    process: u32,
}

impl Caller {
    fn parse(args: Vec<String>) -> Option<(Self, Target)> {
        let mut args = args.into_iter();
        let mut era = None;
        let mut scale = None;
        let mut way = None;
        let mut target = None;
        let mut owner = None;
        let mut process = None;
        let mut pipe = false;
        while let Some(key) = args.next() {
            match key.as_str() {
                "--pipe" => pipe = true,
                "--era" => {
                    era = Some(match args.next()?.as_str() {
                        "95" => Era::Win95,
                        "98" => Era::Win98,
                        "2000" => Era::Win2000,
                        _ => return None,
                    })
                }
                "--scale" => scale = Scale::parse(&args.next()?).filter(|&(_, exact)| exact).map(|(s, _)| s),
                "--scaling" => way = Some(Way::from_setting(&args.next()?)?),
                "--open" => target = Target::parse(&args.next()?),
                "--owner" => owner = Some(args.next()?.parse::<usize>().ok()? as HWND),
                "--parent" => process = args.next()?.parse().ok(),
                _ => return None,
            }
        }
        if !pipe {
            return None;
        }
        let scale: Scale = scale?;
        let scale = way.map_or(scale, |way| scale.with_way(way));
        Some((Self { era: era?, scale, owner: owner?, process: process? }, target?))
    }

    fn live(&self) -> bool {
        if self.owner.is_null() {
            return true;
        }
        let mut pid = 0;
        // SAFETY: Windows validates the handle; the output is one live DWORD.
        unsafe { GetWindowThreadProcessId(self.owner, &mut pid) != 0 && pid == self.process }
    }
}

fn monitor_info(hwnd: HWND) -> MONITORINFO {
    let fallback = if hwnd.is_null() { MONITOR_DEFAULTTOPRIMARY } else { MONITOR_DEFAULTTONEAREST };
    let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
    // SAFETY: Windows selects a monitor for the window (or the primary for an independent viewer).
    if unsafe { GetMonitorInfoW(MonitorFromWindow(hwnd, fallback), &mut info) } == 0 {
        let work = look::main_screen().0;
        info.rcMonitor = work;
        info.rcWork = work;
    }
    info
}

// Both games define their WinHelp article window in 1024ths of the screen.
// Perform WinHelp's integer arithmetic at the logical resolution before upscaling.
fn article_origin(monitor: &MONITORINFO, scale: Scale) -> (i32, i32) {
    let screen = monitor.rcMonitor;
    let x = screen.left + scale.at(scale.old_fit(rect_w(&screen)) * 653 / 1024);
    let y = screen.top + scale.at(scale.old_fit(rect_h(&screen)) * 102 / 1024);
    (x, y)
}

fn article_bounds(monitor: &MONITORINFO, top: i32, scale: Scale) -> (i32, i32) {
    let width = (scale.old_fit(rect_w(&monitor.rcMonitor)) * 360 / 1024).min(scale.old_fit(rect_w(&monitor.rcWork)));
    // WinHelp leaves five logical pixels below an overflowing article.
    (width.max(200), (scale.old_fit(monitor.rcWork.bottom - top) - 5).max(100))
}

fn on_screen(work: RECT, pos: (i32, i32), width: i32, height: i32) -> (i32, i32) {
    (pos.0.min(work.right - width).max(work.left), pos.1.min(work.bottom - height).max(work.top))
}

fn minimize<H: Hosted>() {
    if let Some(hwnd) = kit::hwnd::<H>() {
        // SAFETY: this is our open program window.
        unsafe { ShowWindow(hwnd, SW_MINIMIZE) };
    }
}

fn bring_forward(hwnd: HWND) {
    // SAFETY: a current viewer window. Restore only when minimized.
    unsafe {
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }
        let active = if IsWindowEnabled(hwnd) == 0 { GetLastActivePopup(hwnd) } else { hwnd };
        SetForegroundWindow(active);
    }
}

#[derive(Clone)]
struct Classic {
    form: HelpTopics,
    caller: Caller,
    using_help: bool,
    article_pos: Option<(i32, i32)>,
}

impl Classic {
    /// Measure after navigation, then apply the resulting size to the existing HWND.
    fn update_article(hwnd: HWND, change: impl FnOnce(&mut Self)) {
        let mut bounds = RECT::default();
        // SAFETY: this is our open viewer. Read its actual monitor and top after a user move.
        let normal = unsafe { IsIconic(hwnd) == 0 && IsZoomed(hwnd) == 0 && GetWindowRect(hwnd, &mut bounds) != 0 };
        let monitor = monitor_info(hwnd);
        let mut was_article = false;
        kit::update::<Self>(|h| {
            was_article = h.form.is_article();
            if normal && was_article {
                let (width, limit) = article_bounds(&monitor, bounds.top, h.caller.scale);
                h.form.set_article_bounds(width, limit);
                h.article_pos = Some((bounds.left, bounds.top));
            }
            change(h);
        });
        if normal
            && was_article
            && kit::with::<Self, _>(|h| {
                let s = h.caller.scale;
                let actual = (s.old_cover(rect_w(&bounds)), s.old_cover(rect_h(&bounds)));
                h.form.is_article() && h.form.size() != actual
            }) == Some(true)
        {
            kit::resize::<Self>();
        }
    }
}

impl Hosted for Classic {
    type F = HelpTopics;
    fn form(&self) -> &HelpTopics {
        &self.form
    }
    fn form_mut(&mut self) -> &mut HelpTopics {
        &mut self.form
    }

    fn activate(part: ClassicPart) {
        match part {
            ClassicPart::Minimize => minimize::<Self>(),
            ClassicPart::Maximize => kit::toggle_maximize::<Self>(),
            ClassicPart::Print => {
                kit::update::<Self>(|h| {
                    h.form.activate(part);
                });
                if let Some((title, text)) = kit::with::<Self, _>(|h| (h.form.title().to_owned(), h.form.print_text()))
                    && let Some(hwnd) = kit::hwnd::<Self>()
                {
                    print::print(hwnd, &title, &text);
                }
            }
            _ => {
                let Some(hwnd) = kit::hwnd::<Self>() else { return };
                let mut close = false;
                let mut switch = None;
                Self::update_article(hwnd, |h| {
                    let was_article = h.form.is_article();
                    close = h.form.activate(part);
                    if was_article != h.form.is_article() {
                        switch = Some(h.clone());
                    }
                });
                if close {
                    kit::close::<Self>();
                } else if let Some(next) = switch
                    && let Some(look) = kit::look_of::<Self>()
                {
                    open_classic(next, look);
                }
            }
        }
    }

    fn resized(&mut self, size: (i32, i32), maximized: bool) {
        self.form.set_window_size(size, maximized);
    }

    fn placement_changed(hwnd: HWND) {
        if kit::with::<Self, _>(|h| h.form.is_article()) == Some(true) {
            Self::update_article(hwnd, |_| {});
        }
    }

    fn minimum_size(&self) -> (i32, i32) {
        (200, 100)
    }
}

fn open_classic(mut classic: Classic, look: Look) -> Option<HWND> {
    let caller = classic.caller;
    let monitor = monitor_info(kit::hwnd::<Classic>().unwrap_or(caller.owner));
    let work = monitor.rcWork;
    let article = classic.form.is_article();
    let maximize = article && classic.form.is_maximized();
    let pos = if article {
        let pos = classic.article_pos.unwrap_or_else(|| article_origin(&monitor, caller.scale));
        let pos = on_screen(work, pos, caller.scale.len(200), caller.scale.len(105));
        let (width, limit) = article_bounds(&monitor, pos.1, caller.scale);
        classic.form.set_article_bounds(width, limit);
        // Create the replacement article at its restored size before maximizing it.
        classic.form.set_article_size(None);
        classic.article_pos = Some(pos);
        pos
    } else {
        (work.left + rect_w(&work) / 4, work.top + caller.scale.len(50))
    };
    let kind = if article {
        Kind::ResizableProgram { small: Some(HelpTopics::icon()), big: None }
    } else {
        // Only the caller changes its enabled state. It also observes the process pipe
        // closing, so a killed viewer cannot leave an owner disabled.
        Kind::Owned { owner: caller.owner }
    };
    let hwnd = kit::open_as(kind, classic, look, |w, h| on_screen(work, pos, w, h));
    if maximize && let Some(hwnd) = hwnd {
        // SAFETY: our newly created article window, previously maximized before opening Topics.
        unsafe { ShowWindow(hwnd, SW_MAXIMIZE) };
    }
    let mut notifications = io::stdout().lock();
    let _ = notifications.write_all(&[u8::from(hwnd.is_some() && !article)]);
    let _ = notifications.flush();
    hwnd
}

struct Modern {
    form: HtmlHelp,
    era: Era,
    using_help: bool,
}

impl Hosted for Modern {
    type F = HtmlHelp;
    fn form(&self) -> &HtmlHelp {
        &self.form
    }
    fn form_mut(&mut self) -> &mut HtmlHelp {
        &mut self.form
    }

    fn activate(part: Part) {
        match part {
            Part::Close => kit::close::<Self>(),
            Part::Minimize => minimize::<Self>(),
            Part::Maximize => kit::toggle_maximize::<Self>(),
            Part::InternetOptions => actions::internet_options(),
            Part::Print => {
                if let Some(hwnd) = kit::hwnd::<Self>() {
                    dialogs::print(hwnd);
                }
            }
            Part::Find => {
                if let Some(hwnd) = kit::hwnd::<Self>() {
                    dialogs::find(hwnd);
                }
            }
            Part::TopicsFound => {
                if let Some(hwnd) = kit::hwnd::<Self>() {
                    dialogs::topics_found(hwnd);
                }
            }
            Part::Copy => {
                if let Some(hwnd) = kit::hwnd::<Self>()
                    && let Some(text) = kit::with::<Self, _>(|h| h.form.copy_text())
                {
                    actions::copy(hwnd, &text);
                }
            }
            Part::WindowMenu => {
                if let (Some(hwnd), Some(look), Some(era)) =
                    (kit::hwnd::<Self>(), kit::look_of::<Self>(), kit::with::<Self, _>(|h| h.era))
                {
                    actions::window_menu(hwnd, &look, era);
                }
            }
            _ => {
                let mut resize = false;
                kit::update::<Self>(|h| {
                    let before = h.form.size();
                    h.form.activate(part);
                    resize = before != h.form.size();
                });
                if resize && let Some(look) = kit::look_of::<Self>() {
                    // Folding the navigation pane keeps the right edge of the article still.
                    kit::retheme_with::<Self>(look, |w, _, now| (now.right - w, now.top));
                }
            }
        }
    }

    fn resized(&mut self, size: (i32, i32), maximized: bool) {
        self.form.set_size(size);
        self.form.set_maximized(maximized);
    }

    fn minimum_size(&self) -> (i32, i32) {
        (215, 200)
    }
    fn closing(&mut self, _hwnd: HWND) {
        dialogs::close();
    }
}

fn show(caller: Caller, look: &Look, book: &Catalog, target: Target) -> Option<HWND> {
    if !caller.live() || matches!(target, Target::Topic(i) if i >= book.topics.len()) {
        return None;
    }
    let manual;
    let book = if target == Target::UsingHelp {
        manual = manual::using_help();
        &manual
    } else {
        book
    };
    if caller.era == Era::Win95 {
        if let Target::Topic(i) = target
            && let Some(hwnd) = kit::hwnd::<Classic>()
            && kit::with::<Classic, _>(|h| h.form.is_article() && !h.using_help) == Some(true)
        {
            bring_forward(hwnd);
            Classic::update_article(hwnd, |h| h.form.open_article(i));
            return Some(hwnd);
        }
        let form = match target {
            Target::Topic(i) => HelpTopics::article(caller.era, book.clone(), i)?,
            Target::Contents | Target::UsingHelp => HelpTopics::new(caller.era, book.clone(), HelpTab::Contents),
            Target::Index => HelpTopics::new(caller.era, book.clone(), HelpTab::Index),
            Target::Search => HelpTopics::new(caller.era, book.clone(), HelpTab::Find),
        };
        let classic = Classic { caller, form, using_help: target == Target::UsingHelp, article_pos: None };
        return open_classic(classic, look.clone());
    }

    let tab = match target {
        Target::Index => Tab::Index,
        Target::Search => Tab::Search,
        _ => Tab::Contents,
    };
    let using_help = target == Target::UsingHelp;
    if let Some(hwnd) = kit::hwnd::<Modern>()
        && kit::with::<Modern, _>(|h| h.using_help == using_help) == Some(true)
    {
        let mut resize = false;
        kit::update::<Modern>(|h| {
            let before = h.form.size();
            match target {
                Target::Topic(i) => {
                    h.form.show_topic(i);
                }
                Target::UsingHelp => {
                    h.form.show_tab(tab);
                    h.form.show_topic(book.home);
                }
                _ => h.form.show_tab(tab),
            }
            resize = h.form.size() != before;
        });
        if resize {
            kit::retheme_with::<Modern>(look.clone(), |w, _, now| (now.right - w, now.top));
        }
        bring_forward(hwnd);
        return Some(hwnd);
    }
    let mut form = HtmlHelp::new(caller.era, book.clone(), tab);
    if let Target::Topic(i) = target {
        form.show_topic(i);
    }
    if let Some(hwnd) = kit::hwnd::<Modern>() {
        dialogs::close();
        let title = wide(form.title());
        kit::update::<Modern>(|h| {
            form.set_size(h.form.size());
            // SAFETY: the viewer's own window.
            form.set_maximized(unsafe { IsZoomed(hwnd) } != 0);
            h.form = form;
            h.using_help = using_help;
        });
        // SAFETY: this is our live window, and the title includes a terminating NUL.
        unsafe { SetWindowTextW(hwnd, title.as_ptr()) };
        bring_forward(hwnd);
        return Some(hwnd);
    }
    let work = monitor_info(caller.owner).rcWork;
    let (x, y) = if caller.era == Era::Win98 { (50, 0) } else { (230, 30) };
    let kind = Kind::ResizableProgram { small: Some(HtmlHelp::icon(caller.era)), big: None };
    kit::open_as(kind, Modern { form, era: caller.era, using_help }, look.clone(), |w, h| {
        on_screen(work, (work.left + caller.scale.at(x), work.top + caller.scale.at(y)), w, h)
    })
}

fn run(caller: Caller, book: Catalog, target: Target) -> Result<(), String> {
    let look = look::for_era(caller.era, caller.scale);
    show(caller, &look, &book, target).ok_or("the requested Help window could not open")?;

    // Keep the GUI thread free for Win32, including the native printer dialog. The channel
    // retains requests if Print runs a nested message loop and consumes the wakeup message.
    let (tx, rx) = mpsc::channel();
    // SAFETY: the current GUI thread has a message queue after its first window was created.
    let thread = unsafe { GetCurrentThreadId() };
    std::thread::spawn(move || {
        let mut stream = io::stdin().lock();
        loop {
            let mut command = [0; 4];
            let target = stream.read_exact(&mut command).ok().map(|()| Target::from_code(u32::from_le_bytes(command)));
            if tx.send(target).is_err() {
                break;
            }
            if target.is_some() {
                // If native Print is modal, bring that dialog forward now. The requested
                // topic is still queued for the main message loop when printing finishes.
                let mut gui = GUITHREADINFO { cbSize: size_of::<GUITHREADINFO>() as u32, ..Default::default() };
                // SAFETY: only foreground windows belonging to this viewer's GUI thread.
                unsafe {
                    if GetGUIThreadInfo(thread, &mut gui) != 0
                        && !gui.hwndActive.is_null()
                        && GetWindowThreadProcessId(gui.hwndActive, null_mut()) == thread
                    {
                        SetForegroundWindow(gui.hwndActive);
                    }
                }
            }
            // SAFETY: wake our own GUI thread so it can consume the channel without polling.
            unsafe { PostThreadMessageW(thread, WM_NULL, 0, 0) };
            if target.is_none() {
                // A system Print dialog can be running its own modal loop. Close the Help
                // thread's windows as well so it releases the printer and any disabled owner.
                unsafe extern "system" fn close_window(hwnd: HWND, _: isize) -> i32 {
                    // SAFETY: EnumThreadWindows supplied a window on this viewer's GUI thread.
                    unsafe { PostMessageW(hwnd, WM_CLOSE, 0, 0) };
                    1
                }
                // SAFETY: enumeration is confined to this viewer's own GUI thread.
                unsafe { EnumThreadWindows(thread, Some(close_window), 0) };
                break;
            }
        }
    });

    let mut msg = MSG::default();
    'windows: while kit::hwnd::<Classic>().is_some() || kit::hwnd::<Modern>().is_some() {
        for target in rx.try_iter() {
            match target {
                Some(request) if caller.live() => {
                    show(caller, &look, &book, request);
                }
                _ => break 'windows,
            }
        }
        // SAFETY: standard message pump on the owner thread, no borrowed viewer state.
        unsafe {
            if GetMessageW(&mut msg, null_mut(), 0, 0) <= 0 {
                break;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    dialogs::close();
    kit::close::<Classic>();
    kit::close::<Modern>();
    Ok(())
}

fn main() {
    // Where this run keeps its data, decided once, before anything is read or logged.
    let Some(args) = bevel_winkit::log::program_start() else { return };
    // Never Bevel95's own folder as the working folder (decisions.md, "Folder windows").
    bevel_winkit::shell::start_in_profile();
    bevel_winkit::log::started();
    // A panic on a worker thread closes the windows: none is left looking alive (decisions.md).
    bevel_winkit::panic::install(bevel_winkit::panic::close_windows_after);
    // A double-click carries no book and should not wait for a pipe that will never arrive.
    let Some((caller, target)) = Caller::parse(args) else { return };
    look::dpi_aware();
    let book = match wire::read(&mut io::stdin().lock()) {
        Ok(book) => book,
        Err(error) => {
            log(&format!("Windows Help could not read its book: {error}"));
            return;
        }
    };
    if let Err(error) = run(caller, book, target) {
        log(&format!("Windows Help: {error}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn winhelp_uses_the_monitor_for_placement_and_the_work_area_for_overflow() {
        let mut monitor = MONITORINFO {
            rcMonitor: RECT { left: 0, top: 0, right: 800, bottom: 600 },
            rcWork: RECT { left: 0, top: 0, right: 800, bottom: 572 },
            ..Default::default()
        };
        let pos = article_origin(&monitor, Scale::ONE);
        assert_eq!(pos, (510, 59));
        assert_eq!(article_bounds(&monitor, pos.1, Scale::ONE), (281, 508));
        assert_eq!(article_bounds(&monitor, pos.1 + 64, Scale::ONE), (281, 444));

        // A monitor left of and above the primary, with an odd number of device
        // pixels left after reserving the desktop taskbar at 2×.
        monitor.rcMonitor = RECT { left: -1600, top: -300, right: 0, bottom: 900 };
        monitor.rcWork = RECT { left: -1600, top: -300, right: 0, bottom: 845 };
        let pos = article_origin(&monitor, Scale::whole(2));
        assert_eq!(pos, (-580, -182));
        assert_eq!(article_bounds(&monitor, pos.1, Scale::whole(2)), (281, 508));
        let (_, limit) = article_bounds(&monitor, -55, Scale::whole(2));
        assert_eq!(-55 + (limit + 5) * 2, monitor.rcWork.bottom);
    }
}
