//! Bevel95: a Windows 95/2000-style taskbar. All Win32 code lives in this crate; drawing and
//! layout live in `bevel-core`.
//!
//! One program, several jobs, picked by the first argument:
//! - (none)                  the bar (a copy not yet set up shows Bevel95 Setup first: install.rs)
//! - `--setup`               Bevel95 Setup even for the installed copy (to remove Bevel95)
//! - `--uninstall`           Setup's Remove, with its question (Windows' list of installed apps)
//! - `--restore`             emergency: close a running bar and restore the Windows taskbar
//! - `--watchdog <pid> <w>`  internal: wait for the bar to end, then restore (see watchdog.rs)
//! - `--watchdog-launch ...` internal: start the watchdog (see watchdog.rs)
//! - `--after <pid>`         internal: start a bar once process <pid> has ended (restart)
//! - `--list-windows`        diagnosis: write the windows the bar would show to the log
//! - `--list-programs`       diagnosis: write counts of the Start menu's programs to the log
//! - `--list-status`         diagnosis: write what the network and battery icons would show
#![windows_subsystem = "windows"]

mod about_window;
mod add_shortcut;
mod appbar;
mod apps;
mod audio;
mod autostart;
mod confirm_window;
mod bar;
mod datetime_window;
mod display_window;
mod desktop;
mod desktop_items;
mod dialogs;
mod explorer;
mod file_ops;
mod find_window;
mod fullscreen;
mod install;
mod keys;
mod kit;
mod own_icon;
mod picture_file;
mod pictures;
mod popup;
mod properties_window;
mod quick_launch;
mod remove_window;
mod start_menu;
mod status;
mod tip;
mod tray;
mod volume_popup;
mod wallpaper_browse;
mod watchdog;
mod window_list;
mod zones;

// The Windows side every Bevel95 program shares (fonts from GDI, the log, small helpers) lives
// in `bevel-winkit`; the bar's modules reach it as `crate::font`, `crate::log`, `crate::util`.
use bevel_winkit::{font, log, util};

use std::ptr::null;
use std::sync::atomic::Ordering;
use std::time::Duration;

use windows_sys::Win32::Foundation::{CloseHandle, HWND};
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE, TerminateProcess, WaitForSingleObject,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetWindowThreadProcessId, IsWindow, PostMessageW, WM_CLOSE,
};

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    // First of all, before anything is logged or read: where this run keeps its data, decided
    // once for the whole run (the fourth safety review of 2026-11-18, B1, B2, C2). A portable
    // bar's own starts carry `--portable-copy`; the bar's internal jobs without it are an
    // installed (or not-portable) bar's; only a start by the person reads the mark.
    let internal = args.iter().find(|a| *a != log::PORTABLE_FLAG).is_some_and(|a| matches!(a.as_str(), "--after" | "--watchdog" | "--watchdog-launch"));
    let unflagged = if internal { log::Told::Installed } else { log::Told::Ask };
    // `--restore` gives Windows' taskbar back whatever else is true (the fifth safety review of
    // 2026-11-18, A1: when the settings.txt beside a copy was held, it showed a message and only
    // asked a running bar to close, so a hung bar, or a taskbar left hidden with no bar running,
    // stayed so). It decides like any start, without a message, and then always does all of
    // `restore_command`; while the data folder is not known it writes no log and no file.
    if is_restore(&args) {
        let known = log::decide_quietly(&mut args, unflagged) != log::Home::NotKnown;
        if known {
            bevel_winkit::shell::start_in_profile();
            log::started();
        }
        std::process::exit(restore_command() as i32);
    }
    if !log::decide_at_start(&mut args, unflagged) {
        // Not known (the settings.txt beside this copy is held by another program): the message
        // was shown, nothing is written. No job but `--restore` gives the screen back, and none of
        // them has taken it: the bar has not started.
        return;
    }
    // Never Bevel95's own folder as the working folder: what Run, an address bar or a program
    // started from the bar finds by a relative name (decisions.md, "Folder windows").
    bevel_winkit::shell::start_in_profile();
    let number = |i: usize| args.get(i).and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
    // The version and build first, for the diagnoses (`--restore` writes its own above; the bar
    // writes its own when it starts, after Setup: a first run leaves nothing behind until it has a
    // place; the watchdog's two short-lived jobs, which every start of the bar runs besides, write
    // none).
    if matches!(args.first().map(String::as_str), Some(a) if a.starts_with("--list-")) {
        log::started();
    }
    let code = match args.first().map(String::as_str) {
        None | Some("--setup") => {
            if !install::setup(!args.is_empty()) {
                return; // the installed copy runs instead, or Setup was cancelled or removed it
            }
            install_panic_hook();
            let code = bar::run();
            finish_file_jobs();
            code
        }
        Some("--after") => {
            watchdog::wait_for_place(number(1) as u32);
            install_panic_hook();
            let code = bar::run();
            finish_file_jobs();
            code
        }
        Some("--uninstall") => {
            install::uninstall();
            0
        }
        Some("--list-windows") => {
            window_list::log_all();
            0
        }
        Some("--list-programs") => {
            // Windows' Store-style apps first (the bar reads them on a thread of its own).
            audio::init();
            let apps = apps::read_now();
            let icons: Vec<_> =
                apps.iter().map(|app| start_menu::file_icon(bevel_core::theme::Era::Win95, &app.target)).collect();
            // An icon of nothing but see-through pixels would show as a gap in the menu.
            let blank = icons.iter().flatten().filter(|icon| icon.pixels.iter().all(|p| p >> 24 == 0)).count();
            log::log(&format!(
                "--list-programs: {} Store-style apps, {} of them with an icon ({blank} of these blank)",
                apps.len(),
                icons.iter().flatten().count()
            ));
            // The tree's shape only: program and folder names are the user's (no private text in
            // the log, decisions.md "Milestone 1 decisions").
            let all = start_menu::all_programs();
            log::log(&format!(
                "--list-programs: the Programs tree holds {} programs in {} folders, {} levels deep",
                count_programs(&all),
                count_folders(&all),
                depth(&all)
            ));
            // What each choice of Bevel95 Settings' Programs page would show, and the page's list.
            use bevel_core::programs::Shown;
            let [everything, classic, ticked] = Shown::ALL.map(|shown| count_programs(&start_menu::programs(shown)));
            let list = match start_menu::ticked_programs() {
                Some(list) => format!("programs.txt lists {}", list.len()),
                None => "there is no programs.txt, so all count as ticked".to_string(),
            };
            log::log(&format!(
                "--list-programs: the menu shows {everything} programs with everything, {classic} with Bevel95's own \
                 classic programs, {ticked} with the ticked ones ({list})"
            ));
            let started = std::time::Instant::now();
            let rows = start_menu::program_rows(bevel_core::theme::Era::Win95);
            let pictured = rows.iter().filter(|row| row.icon.is_some()).count();
            // With a programs.txt, a listed place that is no program now is worth knowing of.
            let found = bevel_core::programs::ticked_places(&rows).count();
            log::log(&format!(
                "--list-programs: the Programs page lists {} rows, {pictured} with a picture, {found} ticked \
                 (read in {} ms)",
                rows.len(),
                started.elapsed().as_millis()
            ));
            let started = std::time::Instant::now();
            let recent = start_menu::recent_documents(bevel_core::theme::Era::Win95);
            let took = started.elapsed().as_secs_f64() * 1000.0;
            log::log(&format!("--list-programs: {} recent documents (read in {took:.1} ms)", recent.len()));
            0
        }
        Some("--list-status") => {
            audio::init();
            let mut reader = status::NetworkReader::default();
            let first = reader.read();
            std::thread::sleep(Duration::from_millis(500));
            let network = reader.read();
            log::log(&format!(
                "--list-status: network connected {}, named {}, cable {}, speed {} bps, packets sent {} received {}, \
                 data moved in half a second: sent {} received {} (first look: connected {})",
                network.connected,
                if network.name.is_empty() { "no" } else { "yes (the name is not logged)" },
                network.cable,
                network.speed_bps,
                network.sent_packets,
                network.received_packets,
                network.sending,
                network.receiving,
                first.connected
            ));
            log::log(&format!("--list-status: power {:?}", status::power()));
            0
        }
        Some("--watchdog-launch") => watchdog::launch(number(1) as u32, number(2)),
        Some("--watchdog") => watchdog::run(number(1) as u32, number(2)),
        Some(other) => {
            log::log(&format!("unknown argument {}", if other.starts_with("--") && other.len() <= 24 { other } else { "(not an option of Bevel95's)" }));
            2
        }
    };
    std::process::exit(code as i32);
}

/// Whether this start is `--restore` (the first argument that is not `PORTABLE_FLAG`): it then
/// goes on to `restore_command` whatever is known of the data folder.
fn is_restore(args: &[String]) -> bool {
    args.iter().find(|a| *a != log::PORTABLE_FLAG).is_some_and(|a| a == "--restore")
}

/// How many programs a Programs tree holds, in all its folders.
fn count_programs(folder: &bevel_core::programs::Folder) -> usize {
    folder.items.len() + folder.folders.iter().map(count_programs).sum::<usize>()
}

/// How many folders a Programs tree holds, at every depth.
fn count_folders(folder: &bevel_core::programs::Folder) -> usize {
    folder.folders.len() + folder.folders.iter().map(count_folders).sum::<usize>()
}

/// How many levels of folders a Programs tree has.
fn depth(folder: &bevel_core::programs::Folder) -> usize {
    folder.folders.iter().map(|f| 1 + depth(f)).max().unwrap_or(0)
}

/// The bar has ended (Exit, Restart, `--restore`, Setup, the developer's install): its windows
/// are gone, Windows' taskbar is back, and the bar has given up its place (`bar::give_up_place`:
/// its mutex, hotkeys, hooks and notification-area stand-in), so a new bar can start at once. A
/// copy, move or Undo still running on one of its threads (a drop or Paste on the classic
/// desktop; Windows' progress window and questions belong to it) keeps the process, without a
/// window, until it ends, so nothing is cut short (the safety review of 2026-11-18, A2). Nothing
/// else does: a Properties window closes with the process (the second review of 2026-11-18, B1).
/// Meanwhile the process holds the mark "finishing file jobs", which Setup and install.ps1 read.
/// When Windows itself ends the session, it ends the process at once: nothing can be done then.
fn finish_file_jobs() {
    use bevel_winkit::jobs;
    if !jobs::busy() {
        return;
    }
    jobs::mark_finishing();
    log::log(&format!(
        "the bar's windows are gone, Windows' taskbar is back and a new bar may start; {} file job(s) run: this process, \
         without a window, stays until they end",
        jobs::running()
    ));
    let started = std::time::Instant::now();
    jobs::wait_while_busy(|| false);
    log::log(&format!("the file jobs ended after {} ms; the old bar's process ends", started.elapsed().as_millis()));
}

/// If the bar panics, on any thread, give the screen back at once (the watchdog does it again
/// afterwards). A panic on a worker thread does not end the process by itself: the bar is then
/// asked to close, the ordinary way (`on_destroy`, then `finish_file_jobs`, so a copy of files
/// still running finishes), and from the panic on Windows' taskbar is never hidden again
/// (`explorer::hide` checks `panic::happened`). The safety review of 2026-11-18, suspected 2:
/// the bar went on running with its strip given back, and its tick hid Windows' taskbar again.
fn install_panic_hook() {
    bevel_winkit::panic::install(|_on_main_thread| {
        // A bar that has given up its place (it only finishes a file job now) leaves Windows'
        // taskbar alone: a new bar may be running, and its record is its own.
        if bar::gave_up_place() {
            return;
        }
        let bar = bar::BAR_HWND.load(Ordering::SeqCst);
        if bar != 0 {
            appbar::remove(bar as HWND);
        }
        explorer::restore();
        if bar != 0 {
            // SAFETY: a message posted to the bar's window (a stale handle is harmless).
            unsafe { PostMessageW(bar as HWND, WM_CLOSE, 0, 0) };
        }
    });
}

/// `bevel95 --restore`: asks a running bar to close (its own cleanup restores everything), ends
/// one that does not close within 3 seconds, then restores the Windows taskbar in any case, and
/// asks programs to send their notification icons to it. Every step needs no data folder; only
/// the auto-hide record does, and `explorer::restore` reads it wherever it can be, removing and
/// noting nothing while the data folder is not known (the fifth safety review of 2026-11-18, A1).
/// Its log lines are written only when the data folder is known (`log::log`).
fn restore_command() -> u32 {
    let class = util::wide(bar::BAR_CLASS);
    // SAFETY: plain Win32 calls; a stale handle is harmless to IsWindow; the process handle is
    // closed after use.
    unsafe {
        let running = FindWindowW(class.as_ptr(), null());
        if !running.is_null() {
            let mut pid = 0;
            GetWindowThreadProcessId(running, &mut pid);
            PostMessageW(running, WM_CLOSE, 0, 0);
            for _ in 0..60 {
                if IsWindow(running) == 0 {
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            // Its window gone but its process still there: it is finishing a copy or move of
            // the owner's files (`finish_file_jobs`); Windows' taskbar is back already, the bar's
            // place is free, and the process ends by itself. It is never ended by force for that.
            if IsWindow(running) == 0 && pid != 0 {
                let process = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
                if !process.is_null() {
                    if WaitForSingleObject(process, 2000) != 0 && bevel_winkit::jobs::is_finishing(pid) {
                        log::log("--restore: the bar's windows are gone; it is finishing a copy or move and ends by itself");
                    }
                    CloseHandle(process);
                }
            }
            // A bar that does not close is hung: it is ended, or its notification area window
            // would keep taking the messages meant for Windows' taskbar.
            if IsWindow(running) != 0 {
                let mut pid = 0;
                GetWindowThreadProcessId(running, &mut pid);
                let process = OpenProcess(PROCESS_TERMINATE | PROCESS_SYNCHRONIZE, 0, pid);
                if !process.is_null() {
                    TerminateProcess(process, 1);
                    WaitForSingleObject(process, 5000);
                    CloseHandle(process);
                    log::log("--restore: the bar did not close within 3 seconds; ended it");
                }
            }
        }
    }
    explorer::restore();
    tray::broadcast_taskbar_created();
    log::log("--restore: done");
    0
}

#[cfg(test)]
mod tests {
    /// Every `--restore` goes to `restore_command` before the data folder can stop it (the fifth
    /// safety review of 2026-11-18, A1); no other job does.
    #[test]
    fn a_restore_is_never_stopped_by_the_data_folder() {
        let args = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(super::is_restore(&args(&["--restore"])));
        assert!(super::is_restore(&args(&[super::log::PORTABLE_FLAG, "--restore"])));
        for other in [&[][..], &["--setup"], &["--after", "1"], &["--uninstall"], &["x", "--restore"]] {
            assert!(!super::is_restore(&args(other)), "{other:?}");
        }
    }
}
