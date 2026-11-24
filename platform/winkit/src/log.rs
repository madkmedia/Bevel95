//! A plain text log at %LOCALAPPDATA%\Bevel95\bevel95.log, for things that can fail
//! silently (CLAUDE.md asks for this). Logging never panics and never stops the program.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use std::time::Instant;

use bevel_core::gui_watch::{self, Counts, Watch};
use windows_sys::Win32::Foundation::{ERROR_NOT_ENOUGH_MEMORY, GetLastError, SYSTEMTIME};
use windows_sys::Win32::System::SystemInformation::GetLocalTime;
use windows_sys::Win32::System::Threading::{
    GR_GDIOBJECTS, GR_GDIOBJECTS_PEAK, GR_USEROBJECTS, GR_USEROBJECTS_PEAK, GetCurrentProcess, GetCurrentProcessId,
    GetGuiResources,
};

/// Above this size the log is renamed to `bevel95.old.log` and started afresh.
const MAX_BYTES: u64 = 1_000_000;

/// Where the bar and its programs share settings, scores and logs.
/// The answer is decided once, at the program's start (`decide_at_start`), never asked again.
/// While it is not known (the program is about to end) nothing is created here.
pub fn data_dir() -> PathBuf {
    match home() {
        Home::Portable(dir) => dir,
        Home::Installed => {
            let dir = installed_dir();
            let _ = fs::create_dir_all(&dir);
            dir
        }
        Home::NotKnown => installed_dir(),
    }
}

/// The mark that makes a settings.txt Bevel95's own: a line starting with it (the first line of
/// every settings.txt Bevel95 has ever written, `config::DEFAULT_FILE`, which every save keeps).
pub const SETTINGS_MARK: &str = "# Bevel95 settings";

/// Whether a settings.txt's text holds Bevel95's mark (`SETTINGS_MARK`, case and leading spaces
/// aside): only such a file is Bevel95's own, to be used and written.
pub fn has_settings_mark(text: &str) -> bool {
    let mark = SETTINGS_MARK.to_ascii_lowercase();
    text.lines().any(|line| line.trim_start().to_ascii_lowercase().starts_with(&mark))
}

/// Whose a settings.txt is, as far as can be told by reading it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Owner {
    /// There is none (or a folder of that name).
    Missing,
    /// Bevel95's own: it holds the mark.
    Ours,
    /// Another program's: readable, without the mark (or not UTF-8 text).
    Foreign,
    /// It is there but another program holds it just now (a sharing or lock violation): not known.
    NotKnown,
    /// It is there but cannot be read for a lasting reason (access denied, a damaged file, a disk
    /// error): not Bevel95's, as far as can be told, so the same as `Foreign` for the folder; only
    /// Setup's Portable says it apart, to name the cause (the fifth safety review of 2026-11-18, B2).
    Unreadable,
}

/// Whose the settings.txt at `path` is. One that another program holds just now (a sharing or
/// lock violation) is tried again for up to a second; still held, it is `NotKnown`, never "ours"
/// (the third safety review of 2026-11-18, A1: a file held open by another program made its folder
/// a portable copy, and Bevel95 later wrote its keys into that program's file). Any other failure
/// to read it is `Unreadable` at once (the fifth review, B2: it counted as "held", so a copy whose
/// settings.txt could never be read could never start, with a message blaming a program that did
/// not exist). Logs nothing (the log asks this).
pub fn settings_owner(path: &Path) -> Owner {
    settings_owner_within(path, std::time::Duration::from_secs(1))
}

/// What a failure to read a settings.txt means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReadFailure {
    /// There is no such file.
    Missing,
    /// Another program holds it just now: try again, then "not known".
    Held,
    /// It cannot be read, and trying again will not help.
    Lasting,
}

/// The kind of a failure to read, by Windows' error code (`raw`) and Rust's kind of it: only a
/// sharing violation (32) or a lock violation (33) is "held".
fn read_failure(raw: Option<i32>, kind: std::io::ErrorKind) -> ReadFailure {
    use windows_sys::Win32::Foundation::{ERROR_LOCK_VIOLATION, ERROR_SHARING_VIOLATION};
    match raw {
        Some(code) if code == ERROR_SHARING_VIOLATION as i32 || code == ERROR_LOCK_VIOLATION as i32 => ReadFailure::Held,
        _ if kind == std::io::ErrorKind::NotFound => ReadFailure::Missing,
        _ => ReadFailure::Lasting,
    }
}

fn settings_owner_within(path: &Path, limit: std::time::Duration) -> Owner {
    let until = Instant::now() + limit;
    loop {
        match fs::read(path) {
            Ok(bytes) => {
                let ours = crate::state_file::decode(bytes).text().is_some_and(|text| has_settings_mark(&text));
                return if ours { Owner::Ours } else { Owner::Foreign };
            }
            // A folder of that name is no settings file.
            Err(_) if path.is_dir() => return Owner::Missing,
            Err(e) => match read_failure(e.raw_os_error(), e.kind()) {
                ReadFailure::Missing => return Owner::Missing,
                ReadFailure::Lasting => return Owner::Unreadable,
                ReadFailure::Held if Instant::now() < until => std::thread::sleep(std::time::Duration::from_millis(50)),
                ReadFailure::Held => return Owner::NotKnown,
            },
        }
    }
}

/// Whether `path` is a settings.txt of Bevel95's own (it holds `SETTINGS_MARK`): only that one
/// makes a folder a portable copy. Another program's settings.txt, or one that is not UTF-8 text,
/// is not (the safety review of 2026-11-18, A3); nor is one that cannot be opened ("not known":
/// the third review of 2026-11-18, A1). A portable bar restarted while its settings.txt is held
/// for a moment is told by the bar that ends (`PORTABLE_FLAG`), so it needs no guess.
pub fn is_own_settings(path: &Path) -> bool {
    settings_owner(path) == Owner::Ours
}

// ---- Where this run keeps its data: decided once, then told ----
//
// The fourth safety review of 2026-11-18 (B1, B2, C2): three rounds in a row the question "is this
// folder a portable copy?" was answered by reading settings.txt at the moment of asking, and an
// answer could change in the middle of a run (a file held by another program for a moment). Now
// every program decides ONCE, at its start (`decide_at_start`), and keeps that answer for its whole
// run; nothing asks again. What a Bevel95 program starts is told (`PORTABLE_FLAG`, or the bar's
// internal starts, which are always the bar's own); only a start by the person reads the mark.

/// Where a run keeps its data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Home {
    /// %LOCALAPPDATA%\Bevel95, the installed copy's folder.
    Installed,
    /// A portable copy's folder: the one that holds its bevel95.exe (or the program's own, for a
    /// game run by itself with its settings.txt beside it).
    Portable(PathBuf),
    /// The settings.txt that decides could not be opened (another program holds it): the program
    /// does not guess and does not run; it says so and ends, writing nothing, the log included.
    NotKnown,
}

/// What a program's start was told about its data folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Told {
    /// Started by a portable copy's program with `PORTABLE_FLAG`: beside the copy's bevel95.exe,
    /// without reading anything.
    Portable,
    /// Started by the installed copy's (or a not-portable copy's) bar for a job of its own (a
    /// Restart's `--after`, the watchdog): the installed folder, without reading anything.
    Installed,
    /// Started by the person (a double click, a shortcut, Windows' start): decided by the mark.
    Ask,
}

/// The flag a portable copy's programs pass to every program of the copy they start (the bar's
/// Restart and watchdog, the companions, Help, a folder window's program): "your copy is a
/// portable one". A plain flag, no path: the folder is always the copy's own. The started program
/// keeps its data beside the copy's bevel95.exe without reading settings.txt to decide; every
/// write of settings.txt still checks the mark there (`state_file::change_settings`).
pub const PORTABLE_FLAG: &str = "--portable-copy";

/// Takes `PORTABLE_FLAG` out of a program's arguments; true if it was there.
pub fn take_portable_flag(args: &mut Vec<String>) -> bool {
    let before = args.len();
    args.retain(|a| a != PORTABLE_FLAG);
    args.len() != before
}

/// This run's answer, once decided.
static HOME: std::sync::RwLock<Option<Home>> = std::sync::RwLock::new(None);

/// Decides this run's data folder (once: a second call keeps the first answer) and returns it.
/// `companion`: a companion program's start (`program_start`).
fn decide_in(cell: &std::sync::RwLock<Option<Home>>, told: Told, companion: bool) -> Home {
    decide_with(cell, || match std::env::current_exe() {
        Ok(exe) => match companion_home(home_and_unreadable(&exe, told, &installed_dir(), settings_owner), companion) {
            Some(home) => home,
            None => {
                UNREADABLE_AT_START.store(true, std::sync::atomic::Ordering::SeqCst);
                Home::NotKnown
            }
        },
        Err(_) => Home::Installed,
    })
}

/// This run did not start because a settings.txt beside its copy could not be read (`companion_home`).
static UNREADABLE_AT_START: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// A start's answer (`home_and_unreadable`'s), for a companion: one started by the person beside a
/// settings.txt that is there but could not be read, for whatever reason, does not run (None: it
/// shows `setup::SETTINGS_UNREADABLE` and ends, writing nothing). The sixth safety review of
/// 2026-11-18, C2: such a start ran as the installed copy, without a word (a portable copy's file
/// that could not be read for a moment split its data between two folders). The bar is not
/// affected (Setup shows for it), nor is a start told by its bar, which reads nothing, nor the
/// installed copy, which reads nothing either.
fn companion_home((home, unreadable): (Home, bool), companion: bool) -> Option<Home> {
    (!(companion && unreadable)).then_some(home)
}

/// The answer in `cell`, or `decide`'s, kept there for good.
fn decide_with(cell: &std::sync::RwLock<Option<Home>>, decide: impl FnOnce() -> Home) -> Home {
    if let Some(home) = cell.read().ok().and_then(|h| h.clone()) {
        return home;
    }
    let home = decide();
    match cell.write() {
        Ok(mut h) => h.get_or_insert(home).clone(),
        Err(_) => home,
    }
}

/// Every program's first step, before anything is logged or read: takes `PORTABLE_FLAG` out of
/// `args` and decides this run's data folder for good. `unflagged`: what a start without the flag
/// means (`Told::Ask` for a start by the person; the bar's internal jobs pass `Told::Installed`).
/// False when it is not known (the settings.txt that decides is held by another program): the
/// program has then shown the message and must end at once, writing nothing.
pub fn decide_at_start(args: &mut Vec<String>, unflagged: Told) -> bool {
    decide_at_start_as(args, unflagged, false)
}

/// `decide_at_start`; `companion`: a companion program's start (`companion_home`).
fn decide_at_start_as(args: &mut Vec<String>, unflagged: Told, companion: bool) -> bool {
    let told = if take_portable_flag(args) { Told::Portable } else { unflagged };
    if decide_in(&HOME, told, companion) != Home::NotKnown {
        return true;
    }
    let unreadable = UNREADABLE_AT_START.load(std::sync::atomic::Ordering::SeqCst);
    let text = crate::util::wide(if unreadable { bevel_core::setup::SETTINGS_UNREADABLE } else { bevel_core::setup::SETTINGS_IN_USE_AT_START });
    let title = crate::util::wide(bevel_core::about::NAME);
    // SAFETY: null-terminated strings; no owner window.
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONWARNING, MB_OK, MB_SETFOREGROUND, MessageBoxW};
        MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), MB_OK | MB_ICONWARNING | MB_SETFOREGROUND);
    }
    false
}

/// `decide_at_start` without its message: takes `PORTABLE_FLAG` out of `args`, decides this run's
/// data folder for good and returns it, showing nothing. For a job that goes on whatever the answer
/// (`--restore`: the fifth safety review of 2026-11-18, A1), which then writes nothing while the
/// answer is "not known".
pub fn decide_quietly(args: &mut Vec<String>, unflagged: Told) -> Home {
    let told = if take_portable_flag(args) { Told::Portable } else { unflagged };
    decide_in(&HOME, told, false)
}

/// A companion program's first step (`decide_at_start` for a start by the person or a flagged
/// one): its arguments without `PORTABLE_FLAG`; `None` when its data folder is not known, or a
/// settings.txt beside its copy could not be read (`companion_home`: the message was shown), and
/// it must end at once.
pub fn program_start() -> Option<Vec<String>> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    decide_at_start_as(&mut args, Told::Ask, true).then_some(args)
}

/// This run's data folder as decided (at the start; a library's tests, which have no start, decide
/// at the first use, by the mark).
pub fn home() -> Home {
    decide_in(&HOME, Told::Ask, false)
}

/// Whether this run's data folder is not known (then nothing is written: `log`, `state_file`).
pub fn not_known() -> bool {
    home() == Home::NotKnown
}

/// Setup's Portable has just written Bevel95's own settings.txt into `dir`, the copy's folder:
/// this run, decided "installed" at its start because no settings.txt was there, is told so (by
/// Bevel95's own deed, not by reading). Only an installed answer is changed.
pub fn became_portable(dir: &Path) {
    if let Ok(mut h) = HOME.write()
        && matches!(*h, None | Some(Home::Installed))
    {
        *h = Some(Home::Portable(dir.to_path_buf()));
    }
}

fn same_folder(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().trim_end_matches('\\').eq_ignore_ascii_case(b.to_string_lossy().trim_end_matches('\\'))
}

fn is_programs(dir: &Path) -> bool {
    dir.file_name().is_some_and(|n| n.eq_ignore_ascii_case(std::ffi::OsStr::new("programs")))
}

/// The folder of the copy `dir` (an exe's folder) belongs to: the folder above `programs\` when
/// that holds bevel95.exe, else `dir` itself (the bar's own, or the flat layout).
fn copy_folder(dir: &Path) -> PathBuf {
    match dir.parent() {
        Some(root) if is_programs(dir) && root.join("bevel95.exe").is_file() => root.to_path_buf(),
        _ => dir.to_path_buf(),
    }
}

/// Where the program `exe` keeps its data when started as `told`; `owner` reads a settings.txt
/// (`settings_owner`). The installed copy's programs know without reading; a told start does not
/// read either; a start by the person reads the mark beside the program, then (for a program in
/// `programs\`) beside the copy's bevel95.exe: Bevel95's own makes the copy portable, one that
/// cannot be opened is not known, none or another program's means the installed folder.
fn home_for(exe: &Path, told: Told, installed: &Path, owner: impl Fn(&Path) -> Owner) -> Home {
    home_and_unreadable(exe, told, installed, owner).0
}

/// `home_for`, and whether a settings.txt it read on the way could not be read (`Owner::Unreadable`:
/// `companion_home`).
fn home_and_unreadable(exe: &Path, told: Told, installed: &Path, owner: impl Fn(&Path) -> Owner) -> (Home, bool) {
    let Some(dir) = exe.parent() else { return (Home::Installed, false) };
    let in_installed = same_folder(dir, installed) || (is_programs(dir) && dir.parent().is_some_and(|p| same_folder(p, installed)));
    if in_installed {
        return (Home::Installed, false);
    }
    match told {
        Told::Portable => return (Home::Portable(copy_folder(dir)), false),
        Told::Installed => return (Home::Installed, false),
        Told::Ask => {}
    }
    let mut unreadable = false;
    match owner(&dir.join("settings.txt")) {
        Owner::Ours => return (Home::Portable(dir.to_path_buf()), unreadable),
        Owner::NotKnown => return (Home::NotKnown, unreadable),
        Owner::Unreadable => unreadable = true,
        Owner::Missing | Owner::Foreign => {}
    }
    if is_programs(dir)
        && let Some(root) = dir.parent()
        && root.join("bevel95.exe").is_file()
    {
        match owner(&root.join("settings.txt")) {
            Owner::Ours => return (Home::Portable(root.to_path_buf()), unreadable),
            Owner::NotKnown => return (Home::NotKnown, unreadable),
            Owner::Unreadable => unreadable = true,
            Owner::Missing | Owner::Foreign => {}
        }
    }
    (Home::Installed, unreadable)
}

/// Whether this run keeps its data in a portable copy's folder (then the programs of the copy it
/// starts get `PORTABLE_FLAG`).
pub fn is_portable_run() -> bool {
    matches!(home(), Home::Portable(dir) if !same_folder(&dir, &installed_dir()))
}

/// The portable folder this run keeps its data in, if it does.
fn current_portable_dir() -> Option<PathBuf> {
    match home() {
        Home::Portable(dir) => Some(dir),
        _ => None,
    }
}

/// Whether a person's start of `exe` would find its copy portable, by the mark (Setup's tests).
pub fn is_portable_folder(exe: &Path) -> bool {
    matches!(home_for(exe, Told::Ask, &installed_dir(), settings_owner), Home::Portable(_))
}

/// The file names of Bevel95's own programs, the only ones `PORTABLE_FLAG` is passed to.
const OWN_PROGRAMS: [&str; 6] = ["bevel95.exe", "folders.exe", "minefield.exe", "solitaire.exe", "calculator.exe", "bevel-help.exe"];

/// The argument to add when this run starts `target`: `PORTABLE_FLAG` when this is a portable
/// run and `target` is one of this copy's own programs (in the copy's folder or its `programs\`),
/// else none. Any other program, also one the person keeps in a portable folder, gets nothing.
pub fn flag_for(target: &Path) -> Option<&'static str> {
    let Home::Portable(root) = home() else { return None };
    flag_for_in(target, &root, &installed_dir())
}

fn flag_for_in(target: &Path, root: &Path, installed: &Path) -> Option<&'static str> {
    if same_folder(root, installed) {
        return None;
    }
    let name = target.file_name()?.to_string_lossy().to_ascii_lowercase();
    let dir = target.parent()?;
    let in_copy = same_folder(dir, root) || (is_programs(dir) && dir.parent().is_some_and(|p| same_folder(p, root)));
    (in_copy && OWN_PROGRAMS.contains(&name.as_str())).then_some(PORTABLE_FLAG)
}

/// %LOCALAPPDATA%\Bevel95: where Bevel95 is installed, with its files (not created here, so
/// a portable or cancelled first run leaves nothing behind).
pub fn installed_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    #[cfg(any(test, feature = "test-scratch"))]
    let base = tests_base(base);
    base.join("Bevel95")
}

/// In a build for the tests (`cargo test`: this crate's own tests, and every other crate's,
/// whose dev-dependency on this one switches on the feature `test-scratch`), %LOCALAPPDATA% as
/// Windows knows it is never used: a scratch folder under the temporary folder stands in for it,
/// so a test run never creates or writes the real Bevel95 folder (the second safety review of
/// 2026-11-18, C3: the tests' log lines went into the bar's own log). A test that points
/// LOCALAPPDATA somewhere else itself keeps that place. A released program has no such code.
#[cfg(any(test, feature = "test-scratch"))]
fn tests_base(base: PathBuf) -> PathBuf {
    let real = crate::shell::known_folder(&windows_sys::Win32::UI::Shell::FOLDERID_LocalAppData);
    let same = real.is_some_and(|r| {
        r.as_os_str().to_string_lossy().trim_end_matches('\\').eq_ignore_ascii_case(base.as_os_str().to_string_lossy().trim_end_matches('\\'))
    });
    if same { std::env::temp_dir().join("bevel95-tests-localappdata") } else { base }
}

/// Appends one timestamped line (with the process id, since the bar, the watchdog and
/// `--restore` all write here).
pub fn log(message: &str) {
    // Not known where this run keeps its data: it writes nothing anywhere, the log included.
    if not_known() {
        return;
    }
    let path = data_dir().join("bevel95.log");
    if fs::metadata(&path).is_ok_and(|m| m.len() > MAX_BYTES) {
        let _ = fs::rename(&path, path.with_extension("old.log"));
    }
    let mut t = SYSTEMTIME::default();
    // SAFETY: plain Win32 calls with a valid out-pointer.
    let pid = unsafe {
        GetLocalTime(&mut t);
        GetCurrentProcessId()
    };
    let line = format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03} [{pid}] {message}\r\n",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond, t.wMilliseconds
    );
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = file.write_all(line.as_bytes());
    }
}

/// Whether the `n`th time of something is worth a line: the first three, then the 10th, the
/// 100th, the 1000th and so on, so a path that repeats shows as a growing count and cannot
/// flood the log.
pub fn worth_a_line(n: u32) -> bool {
    let mut rest = n;
    while rest >= 10 && rest.is_multiple_of(10) {
        rest /= 10;
    }
    n <= 3 || rest == 1
}

/// Counts one more time of the path `counter` stands for and writes `line` with the count
/// ("... (time n)") when that is worth a line (`worth_a_line`). `line` is Bevel95's own words.
pub fn counted(counter: &std::sync::atomic::AtomicU32, line: &str) {
    let n = counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
    if worth_a_line(n) {
        log(&format!("{line} (time {n})"));
    }
}

// ---- No private text in the log ----
//
// People attach bevel95.log to their reports, so it holds no text that came from outside
// Bevel95: no window titles, file or folder names, notification tooltips, typed commands, desktop
// item names or paths under the user's folders. What a report needs instead: a window's kind in
// Bevel95's own words (never its class, which can carry a program's name) and its program's file name, a count, a kind ("a folder", "a .txt file"), and Bevel95's own folder
// as %LOCALAPPDATA%\Bevel95. `bevel95 --list-windows`, run on purpose, is the one exception (it
// says so in its first line). The test `log_calls_carry_no_private_text` (tests/private_log.rs)
// reads every log call of the bar and the programs for what was taken out.

/// A path as the log may show it: inside Bevel95's installed folder as
/// `%LOCALAPPDATA%\Bevel95\settings.txt`, inside a portable copy's folder as `the portable
/// folder\settings.txt`; anything else only by its kind (`a folder`, `a .txt file`).
pub fn path_kind(path: &Path) -> String {
    let own = |base: &Path, shown: &str| {
        let rest = path.strip_prefix(base).ok()?;
        let rest = rest.to_string_lossy();
        Some(if rest.is_empty() { shown.to_string() } else { format!("{shown}\\{rest}") })
    };
    if let Some(s) = own(&installed_dir(), "%LOCALAPPDATA%\\Bevel95") {
        return s;
    }
    if let Some(dir) = current_portable_dir()
        && let Some(s) = own(&dir, "the portable folder")
    {
        return s;
    }
    kind_of(path.extension().map(|e| e.to_string_lossy().into_owned()).as_deref(), path.is_dir())
}

/// `a folder`, `a .txt file`, `a file without an extension`.
fn kind_of(extension: Option<&str>, is_dir: bool) -> String {
    match extension {
        _ if is_dir => "a folder".to_string(),
        Some(ext) if !ext.is_empty() && ext.len() <= 8 && ext.chars().all(|c| c.is_ascii_alphanumeric()) => {
            format!("a .{} file", ext.to_ascii_lowercase())
        }
        Some(_) => "a file with an unusual extension".to_string(),
        None => "a file without an extension".to_string(),
    }
}

/// What was typed into Run (or an address bar), as the log may show it: its kind only.
pub fn typed_kind(text: &str) -> String {
    let t = text.trim();
    let lower = t.to_ascii_lowercase();
    if lower.contains("://") || lower.starts_with("www.") || lower.starts_with("mailto:") {
        "a web address".to_string()
    } else if lower.starts_with("shell:") || t.starts_with("::") {
        "a Windows place (shell: or ::)".to_string()
    } else if t.contains(['\\', '/']) || t.get(1..2) == Some(":") {
        format!("a path to {}", path_kind(Path::new(t.trim_matches('"'))))
    } else {
        let word = t.split_whitespace().next().unwrap_or("");
        match Path::new(word).extension() {
            Some(e) => format!("a program or file name ({})", kind_of(Some(&e.to_string_lossy()), false)),
            None if t.contains(' ') => "a command with arguments".to_string(),
            None => "a command".to_string(),
        }
    }
}

/// The first line a program of Bevel95 writes when it starts: the name, the version and the
/// build, and the program's file name (bevel95.exe, minefield.exe, ...).
pub fn started() {
    let exe = std::env::current_exe().ok();
    let program = exe.as_deref().and_then(Path::file_name).map_or_else(|| "?".into(), |n| n.to_string_lossy().into_owned());
    log(&bevel_core::about::log_line(&program, crate::BUILD));
    // Keeps the build stamp in the program's file (the linker drops what nothing uses).
    std::hint::black_box(crate::STAMP.as_ptr());
    // A save cut short by the program's end left its temporary file: cleared once it is a minute
    // old (a younger one may be another program's save under way).
    crate::state_file::clear_leftovers(&data_dir(), std::time::Duration::from_secs(60));
}

/// Logs that a Win32 call failed, with the error code Windows gives for it. A window that could
/// not be made for want of memory (error 8) also gets this program's window resources and whose
/// limit that probably was.
pub fn log_win32(what: &str) {
    // SAFETY: reads the calling thread's last-error value.
    let code = unsafe { GetLastError() };
    let hint = if code == ERROR_NOT_ENOUGH_MEMORY && what.starts_with("CreateWindowExW") {
        gui_watch::out_of_memory_hint(gui_counts())
    } else {
        String::new()
    };
    log(&format!("{what} failed (Win32 error {code}){hint}"));
}

/// This program's window resources as Windows counts them (GetGuiResources).
pub fn gui_counts() -> Counts {
    // SAFETY: the pseudo handle of this process and documented flags.
    unsafe {
        let me = GetCurrentProcess();
        Counts {
            user: GetGuiResources(me, GR_USEROBJECTS),
            gdi: GetGuiResources(me, GR_GDIOBJECTS),
            user_peak: GetGuiResources(me, GR_USEROBJECTS_PEAK),
            gdi_peak: GetGuiResources(me, GR_GDIOBJECTS_PEAK),
        }
    }
}

/// Seconds between two looks at the counts for the warnings.
const CHECK_EVERY_SECS: u64 = 10;

thread_local! {
    /// When the counts were last looked at and last logged, and which warnings were written.
    static WATCH: std::cell::Cell<(Option<Instant>, Option<Instant>, Watch)> =
        const { std::cell::Cell::new((None, None, Watch::new())) };
}

/// Called often (the bar's tick): every ten seconds the counts are looked at for the warnings
/// (2,000 and 5,000), every ten minutes they are logged with their peaks (the first time at once).
pub fn watch_gui_resources() {
    let (checked, logged, mut watch) = WATCH.get();
    let now = Instant::now();
    if checked.is_some_and(|t| now.duration_since(t).as_secs() < CHECK_EVERY_SECS) {
        return;
    }
    let counts = gui_counts();
    if let Some(warning) = watch.warning(counts) {
        log(&warning);
    }
    let due = logged.is_none_or(|t| now.duration_since(t).as_secs() >= gui_watch::LOG_EVERY_SECS);
    if due {
        log(&gui_watch::line(counts));
    }
    WATCH.set((Some(now), if due { Some(now) } else { logged }, watch));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn companion_programs_share_portable_settings_and_keep_standalone_settings() {
        let root = std::env::temp_dir().join(format!("bevel95-portable-data-{}", std::process::id()));
        let programs = root.join("Programs");
        fs::create_dir_all(&programs).unwrap();
        fs::write(root.join("settings.txt"), bevel_core::config::DEFAULT_FILE).unwrap();
        let game = programs.join("minefield.exe");
        let ask = |exe: &Path| home_for(exe, Told::Ask, &installed_dir(), settings_owner);
        assert_eq!(ask(&game), Home::Installed);
        fs::write(root.join("bevel95.exe"), []).unwrap();
        for exe in [game.as_path(), &programs.join("bevel-help.exe"), &root.join("minefield.exe")] {
            assert_eq!(ask(exe), Home::Portable(root.clone()));
        }
        fs::write(programs.join("settings.txt"), bevel_core::config::DEFAULT_FILE).unwrap();
        assert_eq!(ask(&game), Home::Portable(programs.clone()));
        fs::remove_dir_all(root).unwrap();
    }

    /// The fourth safety review of 2026-11-18, B1 and B2: a decision is made once and kept. A
    /// settings.txt held by another program after the start changes nothing (it made every log
    /// line and save wait a second, and could switch a run's folder in the middle); one freed
    /// after an "installed" answer changes nothing either (a companion started while it was held
    /// used the installed folder, then saved into the portable one).
    #[test]
    fn a_decision_is_not_asked_twice() {
        use std::os::windows::fs::OpenOptionsExt;
        let root = std::env::temp_dir().join(format!("bevel95-decided-once-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("programs")).unwrap();
        fs::write(root.join("bevel95.exe"), []).unwrap();
        let settings = root.join("settings.txt");
        let game = root.join("programs").join("minefield.exe");
        let decide = |cell: &std::sync::RwLock<Option<Home>>, exe: &Path| -> Home {
            decide_with(cell, || home_for(exe, Told::Ask, &installed_dir(), |p| settings_owner_within(p, std::time::Duration::from_millis(200))))
        };
        // Portable at the start; the file held afterwards: still portable, and at once.
        fs::write(&settings, bevel_core::config::DEFAULT_FILE).unwrap();
        let cell = std::sync::RwLock::new(None);
        assert_eq!(decide(&cell, &game), Home::Portable(root.clone()));
        let held = fs::OpenOptions::new().read(true).share_mode(0).open(&settings).unwrap();
        let started = Instant::now();
        for _ in 0..20 {
            assert_eq!(decide(&cell, &game), Home::Portable(root.clone()));
        }
        // On the clock only on request (BEVEL95_TIMED=1): a busy PC must not fail the default run
        // (brief 104). Twenty decisions with a 200 ms wait each would take 4 s.
        if std::env::var_os("BEVEL95_TIMED").is_some() {
            assert!(started.elapsed() < std::time::Duration::from_millis(100), "no read, no wait");
        }
        // Held at the start of another run: not known, and so it stays once freed.
        let other = std::sync::RwLock::new(None);
        assert_eq!(decide(&other, &game), Home::NotKnown);
        drop(held);
        assert_eq!(decide(&other, &game), Home::NotKnown);
        // Not portable at the start (another program's file); Bevel95's own later: still installed.
        fs::write(&settings, b"[General]\r\nscale = 2\r\n").unwrap();
        let third = std::sync::RwLock::new(None);
        assert_eq!(decide(&third, &game), Home::Installed);
        fs::write(&settings, bevel_core::config::DEFAULT_FILE).unwrap();
        assert_eq!(decide(&third, &game), Home::Installed);
        fs::remove_dir_all(root).unwrap();
    }

    /// A program started by a Bevel95 program is told and reads no settings.txt to decide: a
    /// flagged one (a portable copy's companion, watchdog or new bar) keeps its data beside the
    /// copy's bevel95.exe; the bar's internal jobs without the flag use the installed folder; the
    /// installed copy's programs know their folder without reading (the fourth review, B1).
    #[test]
    fn a_told_start_never_opens_settings_txt() {
        let root = std::env::temp_dir().join(format!("bevel95-told-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("programs")).unwrap();
        fs::write(root.join("bevel95.exe"), []).unwrap();
        let installed = root.join("installed");
        let never = |_: &Path| -> Owner { panic!("a told start read settings.txt") };
        for exe in [root.join("programs").join("folders.exe"), root.join("bevel95.exe")] {
            assert_eq!(home_for(&exe, Told::Portable, &installed, never), Home::Portable(root.clone()));
            assert_eq!(home_for(&exe, Told::Installed, &installed, never), Home::Installed);
        }
        // A flat copy (a game run by itself): its own folder.
        let flat = root.join("games").join("minefield.exe");
        assert_eq!(home_for(&flat, Told::Portable, &installed, never), Home::Portable(root.join("games")));
        for exe in [installed.join("bevel95.exe"), installed.join("programs").join("minefield.exe")] {
            for told in [Told::Ask, Told::Portable, Told::Installed] {
                assert_eq!(home_for(&exe, told, &installed, never), Home::Installed);
            }
        }
        fs::remove_dir_all(root).unwrap();
    }

    /// The flag goes only to this copy's own programs, and only from a portable run.
    #[test]
    fn the_flag_goes_only_to_the_copys_own_programs() {
        let (root, installed) = (Path::new("D:\\Tools\\Bevel95"), Path::new("C:\\Users\\x\\AppData\\Local\\Bevel95"));
        for own in ["D:\\Tools\\Bevel95\\programs\\Minefield.exe", "D:\\Tools\\Bevel95\\bevel95.exe", "d:\\tools\\bevel95\\programs\\folders.exe"] {
            assert_eq!(flag_for_in(Path::new(own), root, installed), Some(PORTABLE_FLAG), "{own}");
        }
        for other in ["D:\\Tools\\Bevel95\\programs\\notepad.exe", "D:\\Tools\\Bevel95\\tool.exe", "D:\\Other\\minefield.exe", "C:\\Windows\\explorer.exe"] {
            assert_eq!(flag_for_in(Path::new(other), root, installed), None, "{other}");
        }
        assert_eq!(flag_for_in(&installed.join("programs").join("minefield.exe"), installed, installed), None, "the installed copy passes none");
    }

    /// Only Bevel95's own settings.txt makes a folder portable: another program's, beside a
    /// downloaded bevel95.exe, does not (it did: the safety review of 2026-11-18, A3).
    #[test]
    fn only_bevel95s_own_settings_make_a_folder_portable() {
        let root = std::env::temp_dir().join(format!("bevel95-portable-mark-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("programs")).unwrap();
        let (exe, settings) = (root.join("bevel95.exe"), root.join("settings.txt"));
        fs::write(&exe, []).unwrap();
        let utf16: Vec<u8> = [0xFEFFu16].into_iter().chain("[Options]\r\nscale=2\r\n".encode_utf16()).flat_map(u16::to_le_bytes).collect();
        let foreign: [&[u8]; 5] = [b"", b"[General]\r\nscale = 2\r\n", b"# Settings of another program\nera = 98\n", &utf16, b"# Bevel95 settings in caf\xE9\n"];
        for text in foreign {
            fs::write(&settings, text).unwrap();
            assert!(!is_portable_folder(&exe) && !is_portable_folder(&root.join("programs/minefield.exe")), "{text:?}");
            assert_eq!(fs::read(&settings).unwrap(), text, "the file is only read");
        }
        // Every settings.txt Bevel95 has written: the defaults, changed by any save, by Setup's
        // answers, saved by Notepad with a byte-order mark or with Windows' line ends.
        let changed = bevel_core::config::with_setting(bevel_core::config::DEFAULT_FILE, "era", "win2000");
        let crlf = bevel_core::config::DEFAULT_FILE.replace('\n', "\r\n");
        let bom = format!("\u{feff}{changed}");
        for text in [bevel_core::config::DEFAULT_FILE, &changed, &crlf, &bom, "\n  # BEVEL95 SETTINGS, mine\nera = 98\n"] {
            fs::write(&settings, text).unwrap();
            assert!(is_portable_folder(&exe) && is_portable_folder(&root.join("programs/minefield.exe")), "{text:?}");
        }
        fs::remove_dir_all(root).unwrap();
    }

    /// A settings.txt that cannot be opened for a moment (another program holds it without
    /// sharing) is tried again and read once it is free; one that stays held is "not known", and
    /// a folder whose settings.txt is not known is not portable (the third safety review of
    /// 2026-11-18, A1: it was taken as Bevel95's, and Bevel95 later wrote into that program's
    /// file). The portable bar's own restart is told instead (`PORTABLE_FLAG`).
    #[test]
    fn a_settings_file_held_for_a_moment_is_read_and_one_held_longer_is_not_known() {
        use std::os::windows::fs::OpenOptionsExt;
        use std::time::Duration;
        let root = std::env::temp_dir().join(format!("bevel95-portable-held-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let settings = root.join("settings.txt");
        let hold = |path: &Path| fs::OpenOptions::new().read(true).share_mode(0).open(path).unwrap();
        // Bevel95's own, held for 300 ms: read once free, and found marked.
        fs::write(&settings, bevel_core::config::DEFAULT_FILE).unwrap();
        let held = hold(&settings);
        let freeing = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            drop(held);
        });
        let started = Instant::now();
        assert_eq!(settings_owner_within(&settings, Duration::from_secs(1)), Owner::Ours);
        assert!(started.elapsed() >= Duration::from_millis(250), "it waited for the file");
        freeing.join().unwrap();
        // Another program's, held for a moment: read once free, and not ours.
        fs::write(&settings, b"[General]\r\nscale = 2\r\n").unwrap();
        let held = hold(&settings);
        let freeing = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            drop(held);
        });
        assert_eq!(settings_owner_within(&settings, Duration::from_secs(1)), Owner::Foreign);
        freeing.join().unwrap();
        // Bevel95's own, held for longer than the tries: not known, so never "ours" (that was
        // what any file held this long counted as before).
        fs::write(&settings, bevel_core::config::DEFAULT_FILE).unwrap();
        let held = hold(&settings);
        let started = Instant::now();
        assert_eq!(settings_owner_within(&settings, Duration::from_millis(300)), Owner::NotKnown);
        assert!(started.elapsed() >= Duration::from_millis(250));
        let exe = root.join("bevel95.exe");
        fs::write(&exe, []).unwrap();
        assert!(!is_portable_folder(&exe), "a folder whose settings.txt is not known is not portable");
        assert_eq!(home_for(&exe, Told::Ask, &installed_dir(), settings_owner), Home::NotKnown, "and not installed either");
        drop(held);
        assert!(is_portable_folder(&exe), "free again, it is read and found Bevel95's");
        // Not there, or a folder of that name: missing, at once.
        fs::remove_file(&settings).unwrap();
        assert_eq!(settings_owner_within(&settings, Duration::from_secs(1)), Owner::Missing);
        fs::create_dir(&settings).unwrap();
        let started = Instant::now();
        assert_eq!(settings_owner_within(&settings, Duration::from_secs(1)), Owner::Missing);
        // On the clock only on request (BEVEL95_TIMED=1; brief 104).
        if std::env::var_os("BEVEL95_TIMED").is_some() {
            assert!(started.elapsed() < Duration::from_millis(200));
        }
        fs::remove_dir_all(root).unwrap();
    }

    /// Only "another program holds it just now" is "not known" (the fifth safety review of
    /// 2026-11-18, B2): a sharing violation and a lock violation are tried again and then not
    /// known; access denied, a damaged file or a disk error are "cannot be read" at once, which
    /// counts as another program's file (the installed folder; Setup shows). Before, every error
    /// but "not found" was "not known", so such a copy could never start.
    #[test]
    fn only_a_file_in_use_just_now_is_not_known() {
        use std::os::windows::fs::OpenOptionsExt;
        use std::os::windows::io::AsRawHandle;
        use std::time::Duration;
        // By Windows' error code.
        let kind = std::io::ErrorKind::Other;
        assert_eq!(read_failure(Some(32), kind), ReadFailure::Held, "sharing violation");
        assert_eq!(read_failure(Some(33), kind), ReadFailure::Held, "lock violation");
        assert_eq!(read_failure(Some(2), std::io::ErrorKind::NotFound), ReadFailure::Missing);
        assert_eq!(read_failure(Some(3), std::io::ErrorKind::NotFound), ReadFailure::Missing);
        for lasting in [5, 23, 1392, 1920, 21, 4390] {
            // access denied, damaged (CRC), damaged file, cannot be accessed, device not ready, reparse point
            assert_eq!(read_failure(Some(lasting), kind), ReadFailure::Lasting, "{lasting}");
        }
        assert_eq!(read_failure(None, kind), ReadFailure::Lasting);
        let root = std::env::temp_dir().join(format!("bevel95-settings-errors-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let (settings, exe) = (root.join("settings.txt"), root.join("bevel95.exe"));
        fs::write(&exe, []).unwrap();
        fs::write(&settings, bevel_core::config::DEFAULT_FILE).unwrap();
        // A lock violation (a byte range locked by another handle): not known while it lasts.
        let file = fs::OpenOptions::new().read(true).write(true).share_mode(3).open(&settings).unwrap();
        // SAFETY: a handle of the file just opened; the whole file is locked, then unlocked below.
        let locked = unsafe { windows_sys::Win32::Storage::FileSystem::LockFile(file.as_raw_handle(), 0, 0, u32::MAX, 0) };
        assert_ne!(locked, 0);
        assert_eq!(settings_owner_within(&settings, Duration::from_millis(200)), Owner::NotKnown);
        drop(file);
        assert_eq!(settings_owner_within(&settings, Duration::from_millis(200)), Owner::Ours, "freed: read");
        // Access denied (read refused to everyone): cannot be read, at once, not "not known".
        let icacls = |args: &[&str]| {
            let ok = crate::quiet::command(crate::shell::system_program("icacls.exe")).arg(&settings).args(args).stdout(std::process::Stdio::null()).status();
            assert!(ok.is_ok_and(|s| s.success()), "icacls {args:?}");
        };
        icacls(&["/deny", "*S-1-1-0:(R)"]);
        let started = Instant::now();
        let owner = settings_owner_within(&settings, Duration::from_secs(1));
        let took = started.elapsed();
        let home = home_for(&exe, Told::Ask, &root.join("installed"), |p| settings_owner_within(p, Duration::from_secs(1)));
        icacls(&["/remove:d", "*S-1-1-0"]);
        assert_eq!(owner, Owner::Unreadable);
        // On the clock only on request (BEVEL95_TIMED=1; brief 104).
        if std::env::var_os("BEVEL95_TIMED").is_some() {
            assert!(took < Duration::from_millis(500), "no waiting for a program that does not exist: {took:?}");
        }
        assert_eq!(home, Home::Installed, "not Bevel95's as far as can be told: the installed folder, and it runs");
        assert_eq!(settings_owner(&settings), Owner::Ours, "readable again");
        fs::remove_dir_all(root).unwrap();
    }

    /// A companion started by the person beside a settings.txt that is there but cannot be read
    /// does not run (the sixth safety review of 2026-11-18, C2: it ran as the installed copy,
    /// without a word); the bar goes on to Setup as before, a told start and the installed copy
    /// read nothing. A real access denied, made with icacls on a scratch file.
    #[test]
    fn a_companion_beside_a_settings_file_that_cannot_be_read_does_not_run() {
        use std::time::Duration;
        let root = std::env::temp_dir().join(format!("bevel95-companion-unreadable-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("programs")).unwrap();
        fs::write(root.join("bevel95.exe"), []).unwrap();
        let settings = root.join("settings.txt");
        fs::write(&settings, bevel_core::config::DEFAULT_FILE).unwrap();
        let (game, bar, installed) = (root.join("programs").join("minefield.exe"), root.join("bevel95.exe"), root.join("installed"));
        let read = |p: &Path| settings_owner_within(p, Duration::from_millis(200));
        let icacls = |args: &[&str]| {
            let ok = crate::quiet::command(crate::shell::system_program("icacls.exe")).arg(&settings).args(args).stdout(std::process::Stdio::null()).status();
            assert!(ok.is_ok_and(|s| s.success()), "icacls {args:?}");
        };
        icacls(&["/deny", "*S-1-1-0:(R)"]);
        let by_person = home_and_unreadable(&game, Told::Ask, &installed, read);
        let bar_by_person = home_and_unreadable(&bar, Told::Ask, &installed, read);
        let flagged = home_and_unreadable(&game, Told::Portable, &installed, |_| -> Owner { panic!("a told start read settings.txt") });
        icacls(&["/remove:d", "*S-1-1-0"]);
        assert_eq!(by_person, (Home::Installed, true));
        assert_eq!(companion_home(by_person, true), None, "the companion shows the message and ends");
        assert_eq!(companion_home(bar_by_person, false), Some(Home::Installed), "the bar: Setup shows, as before");
        assert_eq!(companion_home(flagged, true), Some(Home::Portable(root.clone())), "told by its bar: not affected");
        // Readable again: by the mark, as always; none: the installed folder.
        assert_eq!(companion_home(home_and_unreadable(&game, Told::Ask, &installed, read), true), Some(Home::Portable(root.clone())));
        fs::remove_file(&settings).unwrap();
        assert_eq!(companion_home(home_and_unreadable(&game, Told::Ask, &installed, read), true), Some(Home::Installed));
        fs::remove_dir_all(root).unwrap();
    }

    /// The flag a portable bar passes to the programs it starts is taken out of the arguments
    /// before they are read (the third safety review of 2026-11-18, A1).
    #[test]
    fn the_portable_flag_is_taken_out_of_the_arguments() {
        let mut args: Vec<String> = ["--after", "123", PORTABLE_FLAG].map(String::from).to_vec();
        assert!(take_portable_flag(&mut args));
        assert_eq!(args, ["--after", "123"]);
        let mut plain: Vec<String> = vec![PORTABLE_FLAG.to_string()];
        assert!(take_portable_flag(&mut plain) && plain.is_empty(), "a flagged new bar has no other argument");
        let mut none: Vec<String> = vec!["--setup".into()];
        assert!(!take_portable_flag(&mut none));
        assert_eq!(none, ["--setup"]);
    }

    #[test]
    fn paths_show_only_their_kind_outside_bevel95s_folder() {
        let own = installed_dir().join("settings.txt");
        assert_eq!(path_kind(&own), "%LOCALAPPDATA%\\Bevel95\\settings.txt");
        assert_eq!(path_kind(&installed_dir()), "%LOCALAPPDATA%\\Bevel95");
        assert_eq!(path_kind(Path::new("C:\\Users\\someone\\Letters\\To the bank.TXT")), "a .txt file");
        assert_eq!(path_kind(Path::new("C:\\Users\\someone\\Letters\\README")), "a file without an extension");
        assert_eq!(path_kind(&std::env::temp_dir()), "a folder");
        let shown = path_kind(Path::new("C:\\Users\\someone\\x.a b c"));
        assert_eq!(shown, "a file with an unusual extension");
    }

    #[test]
    fn typed_commands_show_only_their_kind() {
        assert_eq!(typed_kind("notepad"), "a command");
        assert_eq!(typed_kind("notepad letter.txt"), "a command with arguments");
        assert_eq!(typed_kind("calc.exe"), "a program or file name (a .exe file)");
        assert_eq!(typed_kind("https://example.com/private"), "a web address");
        assert_eq!(typed_kind("shell:startup"), "a Windows place (shell: or ::)");
        assert_eq!(typed_kind("C:\\Users\\someone\\Secret plans.docx"), "a path to a .docx file");
        for typed in ["notepad", "C:\\Users\\someone\\Secret plans.docx", "https://example.com/private"] {
            let shown = typed_kind(typed);
            assert!(!shown.contains("someone") && !shown.contains("Secret") && !shown.contains("example"), "{shown}");
        }
    }
}
