//! Bevel95 Setup: Bevel95 is one file and sets itself up (the owner's idea: copy the exe to
//! another PC and start it; the user decides how). A copy that is neither the installed one nor
//! a portable one shows the Bevel95 Setup window (`bevel_core::setup`, on the control kit):
//! - on a PC where nothing is set up: install it (copy to %LOCALAPPDATA%\Bevel95 with its
//!   programs in programs\, the two desktop shortcuts that tools\install.ps1 makes on the
//!   owner's PC, the entry in Windows' list of installed apps; the second page asks the look,
//!   the classic desktop, the Windows key and Start with Windows) or keep it portable
//!   (settings.txt and the log next to the file, `log::data_dir`);
//! - where Bevel95 is installed: update the installed copy with this one, just run this one (a
//!   copy as built only: the one file's programs are only inside it), or remove Bevel95 (also
//!   from the installed copy itself: `bevel95 --setup`, and from Windows' list of installed
//!   apps: `bevel95 --uninstall`).
//!
//! A copy that is not the installed one and lacks any of its five programs (a download that
//! stopped early has lost the one file's end and with it the programs) says so and sets nothing
//! up (`refused`).
//!
//! What a copy brings (`Bundle`): the released one file carries all six programs
//! (`bevel_core::package`, made by tools\release.ps1), each checked against its fingerprint
//! before anything is written; a copy as built (the developer's) takes the companions it finds
//! beside it, as before. Every file is first written beside its place under a temporary name and
//! read back; only when all are written are they put in place, so a failed install leaves the
//! old copy as it was.
//!
//! An identical copy of the installed program just starts the installed one. A PC set up before
//! the window existed (settings.txt but no installed program) is not asked. `cargo selftest`
//! runs the built program with BEVEL95_NO_SETUP set; BEVEL95_SETUP_TEST is the test mode
//! (`autostart::test_key`).

use std::cell::Cell;
use std::ffi::c_void;
use std::fs;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::ptr::null_mut;

use bevel_core::about::{NAME, VERSION};
use bevel_core::package;
use bevel_core::scale::scale_for_dpi;
use bevel_core::setup::{Answers, Part, Setup, SetupChoice, SetupKind, SetupWindow};
use bevel_core::theme::{Depth, Era};
use windows_sys::Win32::Security::Cryptography::CryptHashCertificate2;
use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
use windows_sys::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows_sys::Win32::UI::Shell::{FOLDERID_Desktop, ShellLink};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowW, IDYES, MB_ICONINFORMATION, MB_ICONWARNING, MB_OK, MB_YESNO, MESSAGEBOX_STYLE,
};
use windows_sys::core::{GUID, HRESULT, PCWSTR};

use crate::audio::{self, method, release};
use crate::autostart::{self, RegValue};
use crate::kit::{self, Hosted};
use crate::log::{installed_dir, log, path_kind};
use crate::start_menu::{self, known_folder};
use crate::util::wide;
use crate::bar;
use bevel_winkit::state_file;

/// IID_IShellLinkW (ShObjIdl.h).
const IID_SHELL_LINK: GUID = GUID::from_u128(0x000214F9_0000_0000_C000_000000000046);
/// Method slots after IUnknown's three. IShellLinkW: GetPath, GetIDList, SetIDList,
/// GetDescription, SetDescription (7), GetWorkingDirectory, SetWorkingDirectory (9),
/// GetArguments, SetArguments (11), ..., SetPath (20). Saved by `file_ops::save_shell_link_new`.
const SET_DESCRIPTION: usize = 7;
const SET_WORKING_DIRECTORY: usize = 9;
const SET_ARGUMENTS: usize = 11;
const SET_PATH: usize = 20;

type SetText = unsafe extern "system" fn(*mut c_void, PCWSTR) -> HRESULT;

/// The desktop shortcuts: name, arguments, description (the same as tools\install.ps1's).
const SHORTCUTS: [(&str, &str, &str); 2] = [
    ("Bevel95", "", "Start the Windows 95-style taskbar"),
    ("Restore Windows taskbar", "--restore", "Close Bevel95 and bring back the Windows taskbar"),
];

const LEGACY_FILES: &str = include_str!("../../../tools/legacy-files.txt");

/// The files Bevel95 and its programs write into the data folder, which Remove deletes (and
/// with them gone, the folder itself goes; files the owner put there stay, and so does it).
/// README's "Files it uses" lists the same.
const DATA_FILES: [&str; 17] = [
    "settings.txt",
    "desktop.txt",
    "folders.txt",
    "undo.txt",
    "task-order.txt",
    "quick-launch.txt",
    "quick-search.txt",
    "programs.txt",
    "programs.old.txt",
    "minefield.txt",
    "solitaire.txt",
    "calculator.txt",
    "bevel95.log",
    "bevel95.old.log",
    "run-history.txt",
    "windows-taskbar-state.txt",
    "windows-taskbar-used.txt",
];

/// The Bevel95 Setup window.
pub struct SetupHost {
    form: SetupWindow,
}

thread_local! {
    /// What OK chose (with the second page's answers), once the window is closed.
    static ANSWER: Cell<Option<(SetupChoice, Answers)>> = const { Cell::new(None) };
    /// The colors the window is drawn in (the installed copy's, or True Color).
    static DEPTH: Cell<Depth> = const { Cell::new(Depth::TrueColor) };
}

impl Hosted for SetupHost {
    type F = SetupWindow;
    fn form(&self) -> &SetupWindow {
        &self.form
    }
    fn form_mut(&mut self) -> &mut SetupWindow {
        &mut self.form
    }
    fn activate(part: Part) {
        match part {
            Part::Ok => {
                let done = Cell::new(false);
                kit::update::<SetupHost>(|h| done.set(h.form.setup.ok()));
                if done.get() {
                    let answer = kit::with::<SetupHost, _>(|h| (h.form.setup.chosen(), h.form.setup.answers));
                    ANSWER.with(|a| a.set(answer));
                    kit::close::<SetupHost>();
                }
            }
            Part::Back => kit::update::<SetupHost>(|h| h.form.setup.back()),
            // A look was chosen: the window is drawn in it from now on, where it was.
            Part::Look(_) => {
                let (Some(era), Some(old)) = (kit::with::<SetupHost, _>(|h| h.form.setup.era()), kit::look_of::<SetupHost>())
                else {
                    return;
                };
                match bevel_winkit::look::for_look(era, DEPTH.with(Cell::get), old.scale) {
                    Ok(look) => kit::retheme_with::<SetupHost>(look, |w, h, now| {
                        ((now.left + now.right - w) / 2, (now.top + now.bottom - h) / 2)
                    }),
                    Err(e) => log(&format!("setup: cannot use the look's font ({e})")),
                }
            }
            _ => kit::close::<SetupHost>(),
        }
    }
}

/// Decides whether this copy needs setting up and does it. Returns true when the bar should
/// run from this copy; false when this process should end (the installed copy was started,
/// Bevel95 was removed, or the window was cancelled). `forced`: `--setup`.
pub fn setup(forced: bool) -> bool {
    if std::env::var_os("BEVEL95_NO_SETUP").is_some() {
        return true;
    }
    autostart::test_key(); // a bad test name stops here, before anything is written
    let Ok(exe) = std::env::current_exe() else { return true };
    let dir = installed_dir();
    let installed = dir.join("bevel95.exe");
    let here_installed = same_file(&exe, &installed);
    // Portable as this run decided at its start, once (`log::decide_at_start`: Bevel95's own
    // settings.txt beside it, or a start flagged by a portable copy's program). Another program's
    // settings.txt does not make it portable: Setup shows as on a first run, and its Portable
    // refuses. One that could not be opened ended the program before it got here (the fourth
    // safety review of 2026-11-18, C2: it ran the installed copy instead, or offered Update).
    let portable = !here_installed && matches!(bevel_winkit::log::home(), bevel_winkit::log::Home::Portable(_));
    let installed_set_up = installed.is_file() && dir.join("settings.txt").is_file();
    if here_installed && !forced {
        return true;
    }
    // A portable copy, or a PC set up before this window existed, runs without Setup, but never
    // the one file without its programs beside it (an outside review of 2026-11-18: it ran as a
    // bar without them): Setup shows for it instead, which can unpack them.
    let without_setup = runs_without_setup(&exe);
    if portable && !forced && without_setup {
        return true;
    }
    if !installed_set_up && dir.join("settings.txt").is_file() && !forced && without_setup {
        return true; // set up before this window existed, without installing
    }
    // What this copy brings, checked before anything is shown or written.
    let bundle = match Bundle::of(&exe, &dir) {
        Ok(bundle) => bundle,
        Err(e) => {
            log("setup: this copy's file is damaged; nothing was changed");
            message(&format!("This copy of {NAME} is damaged ({e}).\n\nDownload it again. Nothing was changed."), MB_OK | MB_ICONWARNING);
            return false;
        }
    };
    let kind = if here_installed {
        SetupKind::Installed
    } else if installed_set_up {
        let old = fs::read(&installed).unwrap_or_default();
        if !forced && bundle.bar == old && bundle.programs.iter().all(|p| same_bytes_as(&p.bytes, &dir.join("programs").join(p.file))) {
            // Started again while that very build runs: starting it would show nothing (a second
            // bar ends at once), so the person is told, in the installed copy's look.
            if running_bar_is(&installed) {
                log("setup: this copy is the installed one, and its bar is already running; said so");
                let settings = bevel_core::config::parse(&state_file::read(&dir.join("settings.txt")).text().unwrap_or_default()).0;
                say_in_look(settings.era, settings.colors, &bevel_core::setup::already_running(bevel_core::about::VERSION));
                return false;
            }
            log("setup: this copy is the installed one; starting that");
            if start(&installed, &dir) {
                return false;
            }
            return not_started_here(&bundle, &format!("{NAME} could not start its installed copy in\n{}.\n\nNothing was changed.", dir.display()));
        }
        other_copy(&bundle, is_newer(&bundle, &exe, &old, &installed))
    } else {
        SetupKind::FirstRun
    };
    // A copy without all its programs sets nothing up, asks nothing and does not run as a bar
    // (a lone copy of the installed bar has already started the installed one above).
    if refused(&bundle, kind) {
        if kind != SetupKind::FirstRun {
            log("setup: this copy is incomplete (programs missing); nothing was changed");
        }
        message(bevel_core::setup::INCOMPLETE, MB_OK | MB_ICONWARNING);
        return false;
    }
    // An installed copy's window is drawn in its look and colors.
    let settings = if kind == SetupKind::FirstRun {
        bevel_core::config::Config::default()
    } else {
        bevel_core::config::parse(&state_file::read(&dir.join("settings.txt")).text().unwrap_or_default()).0
    };
    let Some(answer) = ask(kind, settings.era, settings.colors) else {
        log("setup: the window could not be shown");
        return not_started_here(&bundle, &format!("{NAME} Setup could not be shown.\n\nNothing was changed."));
    };
    // (A first run logs only once it has a place: nothing is left behind by Cancel or Portable.)
    let Some((choice, answers)) = answer else {
        if kind != SetupKind::FirstRun {
            log("setup: cancelled");
        }
        return false;
    };
    if kind != SetupKind::FirstRun {
        log(&format!("setup: {choice:?} (from {kind:?})"));
    }
    match choice {
        SetupChoice::Install => install(&exe, &bundle, &dir, &answers),
        SetupChoice::Portable => portable_here(&exe, &bundle),
        SetupChoice::Update => update(&bundle, &dir),
        // Offered only to a copy as built (`other_copy`); the one file never runs as a bar here.
        SetupChoice::RunThisCopy => !bundle.packed,
        SetupChoice::Remove => {
            remove(&dir, here_installed);
            false
        }
    }
}

/// Whether this copy may run as a bar without Setup (a portable copy, a PC set up before Setup
/// existed): not the one file without all its programs beside it (`programs\` or flat), which
/// would be a bar without its programs. A copy as built keeps today's way.
fn runs_without_setup(exe: &Path) -> bool {
    !is_one_file(exe) || programs_beside(exe)
}

/// Whether the file `exe` is the released one file (a package's trailer at its end; only the
/// last bytes are read).
fn is_one_file(exe: &Path) -> bool {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut f) = fs::File::open(exe) else { return false };
    let mut tail = [0u8; package::TRAILER];
    f.seek(SeekFrom::End(-(package::TRAILER as i64))).is_ok() && f.read_exact(&mut tail).is_ok() && package::is_packed(&tail)
}

/// Whether all of Bevel95's programs are beside `exe` (as the running bar finds them).
fn programs_beside(exe: &Path) -> bool {
    exe.parent().is_some_and(|dir| start_menu::companions().all(|(_, file)| start_menu::companion_file(dir, file).is_some()))
}

/// Setup could not do what it was asked and nothing was changed: a copy as built runs from where
/// it is, as before; the one file never does (its programs are only inside it: an outside
/// review of 2026-11-18 found it ran as a bar without them after a failed Install). It says
/// `said` and ends.
fn not_started_here(bundle: &Bundle, said: &str) -> bool {
    if !bundle.packed {
        return true;
    }
    log("setup: the one file does not run as a bar from where it is");
    message(said, MB_OK | MB_ICONWARNING);
    false
}

/// Another copy over an installed one. Only a copy as built (its programs beside it) offers
/// "Just run this copy": the one file's programs are only inside it.
fn other_copy(bundle: &Bundle, newer: bool) -> SetupKind {
    SetupKind::OtherCopy { newer, can_run: !bundle.packed }
}

/// Whether Setup refuses this copy: one that is not the installed copy and lacks any of its
/// programs sets nothing up (a download that stopped early installed the bar alone).
fn refused(bundle: &Bundle, kind: SetupKind) -> bool {
    kind != SetupKind::Installed && !bundle.complete()
}

/// `bevel95 --uninstall`, what Windows' list of installed apps runs: Remove, with its question.
pub fn uninstall() {
    autostart::test_key();
    let dir = installed_dir();
    let here_installed = std::env::current_exe().is_ok_and(|exe| same_file(&exe, &dir.join("bevel95.exe")));
    log("setup: Remove (from the list of installed apps)");
    remove(&dir, here_installed);
}

/// Whether this copy is newer than the installed one: by their build stamps (the commit's
/// time), else (a copy built before the stamp) by the files' dates as before.
fn is_newer(bundle: &Bundle, exe: &Path, installed_bytes: &[u8], installed: &Path) -> bool {
    match (package::find_stamp(&bundle.bar), package::find_stamp(installed_bytes)) {
        (Some(a), Some(b)) if a.time != 0 && b.time != 0 && a.time != b.time => a.time > b.time,
        _ => {
            let dir = installed.parent().expect("an installed program has a folder");
            modified(exe) > modified(installed)
                || bundle.programs.iter().any(|p| p.date > modified(&dir.join("programs").join(p.file)))
        }
    }
}

/// Shows Bevel95 Setup and waits for it. `None`: it could not be shown; `Some(None)`: cancelled.
#[allow(clippy::option_option)]
fn ask(kind: SetupKind, era: Era, depth: Depth) -> Option<Option<(SetupChoice, Answers)>> {
    bevel_winkit::look::dpi_aware();
    let (screen, dpi) = bevel_winkit::look::main_screen();
    let look = bevel_winkit::look::for_look(era, depth, scale_for_dpi(dpi)).ok()?;
    DEPTH.with(|d| d.set(depth));
    let form = SetupWindow::new(Setup::in_look(kind, era), &look.regular);
    ANSWER.with(|a| a.set(None));
    kit::open_with(SetupHost { form }, look.theme, look.regular, look.bold, look.scale, |w, h| {
        (screen.left + (screen.right - screen.left - w) / 2, screen.top + (screen.bottom - screen.top - h) / 2)
    })?;
    kit::run_until_closed::<SetupHost>();
    Some(ANSWER.with(Cell::take))
}

/// One of the companions this copy brings.
struct Program {
    /// Its name, for the log and the messages.
    name: &'static str,
    /// Its file name in programs\.
    file: &'static str,
    bytes: Vec<u8>,
    /// Its file's date (the one file's own for a packed program), for "newer" without stamps.
    date: std::time::SystemTime,
}

/// What this copy brings: the bar and its companions, with their bytes.
struct Bundle {
    bar: Vec<u8>,
    programs: Vec<Program>,
    /// From the one file (each checked against its fingerprint) rather than files beside it.
    packed: bool,
}

impl Bundle {
    /// The one file's programs, or for a copy as built the companions found beside it (`dir`:
    /// the installed folder, whose own old flat game is never taken as a companion). An error:
    /// the one file is damaged.
    fn of(exe: &Path, dir: &Path) -> Result<Bundle, String> {
        let data = fs::read(exe).map_err(|e| format!("it cannot be read: {e}"))?;
        let date = modified(exe);
        if let Some(files) = package::unpack(&data)? {
            let bar = files[0].1.to_vec();
            let programs = start_menu::companions()
                .filter_map(|(name, file)| {
                    let (_, bytes) = files.iter().find(|(e, _)| e.name == format!("programs/{file}"))?;
                    Some(Program { name, file, bytes: bytes.to_vec(), date })
                })
                .collect();
            return Ok(Bundle { bar, programs, packed: true });
        }
        Ok(Bundle::beside(exe, dir, data))
    }

    /// Whether this copy brings all five companions (from the one file, or beside a copy as
    /// built).
    fn complete(&self) -> bool {
        start_menu::companions().all(|(_, file)| self.programs.iter().any(|p| p.file == file))
    }

    /// A copy as built: the bar's bytes and the companions found beside `exe`.
    fn beside(exe: &Path, dir: &Path, data: Vec<u8>) -> Bundle {
        let from = exe.parent().expect("the running executable has a parent");
        let programs = start_menu::companions()
            .filter_map(|(name, file)| {
                let source = start_menu::companion_file(from, file)?;
                // Setup in the installed folder may find just the old game after a bar-only
                // update. Leave it in place until a replacement is supplied.
                if same_file(&source, &dir.join(file)) {
                    return None;
                }
                Some(Program { name, file, bytes: fs::read(&source).ok()?, date: modified(&source) })
            })
            .collect();
        Bundle { bar: data, programs, packed: false }
    }
}

/// A file written beside its place under a temporary name, to be put in place (`commit`) or
/// thrown away (`discard`).
struct Staged {
    name: &'static str,
    /// `bevel95.exe` or `programs/<file>`, for the log.
    shown: String,
    temp: PathBuf,
    target: PathBuf,
}

impl Staged {
    /// Renames the written copy over the old file. A running program's file cannot be
    /// replaced: the error says so and the old file stays as it was.
    fn commit(&self) -> std::io::Result<()> {
        let result = fs::rename(&self.temp, &self.target);
        if result.is_err() {
            let _ = fs::remove_file(&self.temp);
        }
        result
    }

    fn discard(&self) {
        let _ = fs::remove_file(&self.temp);
    }
}

/// Writes `bytes` beside `target` under a temporary name and reads them back. `None`: `target`
/// holds these bytes already.
fn stage(bytes: &[u8], target: &Path) -> std::io::Result<Option<PathBuf>> {
    let parent = target.parent().expect("an installed executable has a parent");
    fs::create_dir_all(parent)?;
    if !plain_dir(parent) {
        return Err(std::io::Error::other("the destination folder is redirected"));
    }
    if fs::symlink_metadata(target).is_ok_and(|m| !m.is_file() || m.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0) {
        return Err(std::io::Error::other("the intended file is redirected or is a directory"));
    }
    if same_bytes_as(bytes, target) {
        return Ok(None);
    }
    let temp = target.with_extension(format!("{}.tmp", std::process::id()));
    let result = fs::File::create_new(&temp).and_then(|mut file| std::io::Write::write_all(&mut file, bytes)).and_then(|()| {
        if fs::read(&temp)? == bytes { Ok(()) } else { Err(std::io::Error::other("the written copy differs")) }
    });
    match result {
        Ok(()) => Ok(Some(temp)),
        Err(e) => {
            let _ = fs::remove_file(&temp);
            Err(e)
        }
    }
}

/// Writes every companion (and with `bar`, the bar as `dir\bevel95.exe`) beside its place. On a
/// failure nothing stays behind: the error names what could not be written.
fn stage_all(bundle: &Bundle, dir: &Path, bar: bool) -> Result<Vec<Staged>, String> {
    let mut staged: Vec<Staged> = Vec::new();
    let bar_file = bar.then(|| (NAME, package::BAR.to_string(), bundle.bar.as_slice(), dir.join(package::BAR)));
    let programs = bundle.programs.iter().map(|p| (p.name, format!("programs/{}", p.file), p.bytes.as_slice(), dir.join("programs").join(p.file)));
    for (name, shown, bytes, target) in bar_file.into_iter().chain(programs) {
        match stage(bytes, &target) {
            Ok(Some(temp)) => staged.push(Staged { name, shown, temp, target }),
            Ok(None) => {}
            Err(e) => {
                staged.iter().for_each(Staged::discard);
                return Err(format!("{name} ({e})"));
            }
        }
    }
    Ok(staged)
}

/// What putting the companions in place did.
#[derive(Default)]
struct Placed {
    lines: Vec<String>,
    /// The programs that were running and were left as they were.
    left: Vec<&'static str>,
}

/// Puts the written companions in place; then an older install's flat game and files go.
fn place_programs(staged: &[Staged], dir: &Path) -> Placed {
    let mut placed = Placed::default();
    for s in staged {
        match s.commit() {
            Ok(()) => placed.lines.push(format!("setup: installed {} ({})", s.name, s.shown)),
            Err(e) => {
                placed.lines.push(format!("setup: could not update {} ({e}); close it and retry Setup", s.name));
                placed.left.push(s.name);
            }
        }
    }
    // Older packages may omit Solitaire. Only the game and viewer that replaced
    // the original flat Minesweeper install are needed to retire that install.
    let programs = dir.join("programs");
    if programs.join("minefield.exe").is_file() && programs.join("bevel-help.exe").is_file() && retire_flat_game(dir, &mut placed.lines) {
        clean_legacy_files(dir, LEGACY_FILES, &mut placed.lines);
    }
    placed
}

/// The companions of `exe` into `dir`'s programs\ (the developer's copy and the tests); returns
/// what to write to the log.
#[cfg(test)]
fn copy_programs(exe: &Path, dir: &Path) -> Vec<String> {
    // (A test may name a bar that is not there: only its folder counts then.)
    let bundle = Bundle::of(exe, dir).unwrap_or_else(|_| Bundle::beside(exe, dir, Vec::new()));
    match stage_all(&bundle, dir, false) {
        Ok(staged) => place_programs(&staged, dir).lines,
        Err(e) => vec![format!("setup: could not write {e}; nothing was changed")],
    }
}

/// Tells the person which programs were running and keep their old version.
fn tell_left(done: &str, left: &[&str]) {
    if !left.is_empty() {
        message(
            &format!(
                "{NAME} was {done}, except for these programs, which were open and were left as they were:\n\n{}\n\n\
                 Close them and run this Setup again to update them too.",
                left.join("\n")
            ),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}

/// Install: this copy goes to `dir` with its programs, the settings with the answers, the
/// desktop shortcuts that are missing, the installed-apps entry and, if asked, Start with
/// Windows; then the installed copy starts. True if this copy should run instead (copying
/// failed).
fn install(exe: &Path, bundle: &Bundle, dir: &Path, answers: &Answers) -> bool {
    let installed = dir.join(package::BAR);
    // A failed copy: a copy as built runs from where it is (its programs are beside it); the one
    // file says nothing was changed and does not run (an outside review of 2026-11-18).
    let failed = |e: &dyn std::fmt::Display| {
        let then = if bundle.packed { "Nothing was changed." } else { "It runs from where it is." };
        message(&format!("{NAME} could not copy itself to\n{}\n({e}).\n\n{then}", dir.display()), MB_OK | MB_ICONWARNING);
        !bundle.packed
    };
    let staged = match stage_all(bundle, dir, !same_file(exe, &installed)) {
        Ok(staged) => staged,
        Err(e) => {
            log(&format!("setup: could not copy to {} ({e}); nothing was changed", path_kind(dir)));
            return failed(&e);
        }
    };
    let (bar, programs): (Vec<&Staged>, Vec<&Staged>) = staged.iter().partition(|s| s.shown == package::BAR);
    if let Some(bar) = bar.first()
        && let Err(e) = bar.commit()
    {
        programs.iter().for_each(|s| s.discard());
        log(&format!("setup: could not copy to {} ({e}); nothing was changed", path_kind(&installed)));
        return failed(&e);
    }
    let settings = dir.join("settings.txt");
    // In the installed folder settings.txt is Bevel95's own and is written (its mark put back if
    // it was removed); one that cannot be read, or is read-only, is kept, and then Setup says the
    // answers were not saved instead of reporting them as written (the fourth safety review of
    // 2026-11-18, C1).
    let written = state_file::change_settings(&settings, |text| answers.settings(text));
    if let Err(e) = &written
        && !state_file::already_logged(e)
    {
        log(&format!("setup: could not write settings.txt ({e})"));
    }
    let from = if bundle.packed { "from the one file, fingerprints checked" } else { "from the files beside it" };
    if written.is_ok() {
        log(&format!(
            "setup: installed as {} (look {}, desktop {}, Windows key {}, {from})",
            path_kind(&installed),
            answers.era.setting(),
            if answers.desktop { "yes" } else { "no" },
            answers.windows_key.setting(),
        ));
    } else {
        log(&format!("setup: installed as {} ({from}); the answers were NOT written into settings.txt", path_kind(&installed)));
        message(
            &format!("{NAME} was installed, but your answers could not be saved in its settings.txt, which was kept as it was.\n\nBevel95 Settings can set them later."),
            MB_OK | MB_ICONWARNING,
        );
    }
    let programs: Vec<Staged> = staged.into_iter().filter(|s| s.shown != package::BAR).collect();
    let placed = place_programs(&programs, dir);
    placed.lines.iter().for_each(|line| log(line));
    make_shortcuts(&installed, dir);
    if answers.start_with_windows {
        autostart::set_on_for(&installed);
    }
    write_entry(dir);
    tell_left("installed", &placed.left);
    if start(&installed, dir) {
        return false;
    }
    not_started_here(bundle, &format!("{NAME} was installed in\n{}\nbut could not be started.\n\nStart it with the {NAME} shortcut on your desktop.", dir.display()))
}

/// What `folder` holds under the name settings.txt that keeps it from being a portable copy's
/// folder: Setup's message for it, or None when there is none or it is Bevel95's own. Anything of
/// that name that is not provably Bevel95's: another program's (`FOREIGN_SETTINGS`), one held
/// open so that it cannot be read just now (`SETTINGS_IN_USE`, the third safety review of
/// 2026-11-18, A1), or one that cannot be read for a lasting reason (`SETTINGS_UNREADABLE`, the
/// fifth review, B2).
fn foreign_settings(folder: &Path) -> Option<&'static str> {
    use bevel_winkit::log::Owner;
    let settings = folder.join("settings.txt");
    if fs::symlink_metadata(&settings).is_err() {
        return None;
    }
    match bevel_winkit::log::settings_owner(&settings) {
        Owner::Ours => None,
        Owner::NotKnown => Some(bevel_core::setup::SETTINGS_IN_USE),
        Owner::Unreadable => Some(bevel_core::setup::SETTINGS_UNREADABLE),
        Owner::Foreign | Owner::Missing => Some(bevel_core::setup::FOREIGN_SETTINGS),
    }
}

/// Whether `folder` holds a settings.txt (or anything of that name) that is not Bevel95's own.
#[cfg(test)]
fn holds_foreign_settings(folder: &Path) -> bool {
    foreign_settings(folder).is_some()
}

/// Portable: settings.txt next to this file. From the one file, its programs are unpacked beside
/// it (programs\, and the bar as bevel95.exe, which the companions look for) and that bar starts.
fn portable_here(exe: &Path, bundle: &Bundle) -> bool {
    let Some(folder) = exe.parent() else { return false };
    let settings = folder.join("settings.txt");
    // Another program's settings.txt, or one that cannot be read to tell: nothing is written (a
    // first run logs nothing either).
    if let Some(said) = foreign_settings(folder) {
        message(said, MB_OK | MB_ICONWARNING);
        return false;
    }
    let cannot = concat!(bevel_core::product_name!(), " cannot write its settings and programs next to this file (is the folder read-only?).\n\nNothing was changed.");
    let bar = folder.join(package::BAR);
    let staged = if bundle.packed {
        match stage_all(bundle, folder, !same_file(exe, &bar)) {
            Ok(staged) => staged,
            Err(_) => {
                message(cannot, MB_OK | MB_ICONWARNING);
                return false;
            }
        }
    } else {
        Vec::new()
    };
    if !settings.is_file() && state_file::change_settings(&settings, str::to_string).is_err() {
        staged.iter().for_each(Staged::discard);
        message(cannot, MB_OK | MB_ICONWARNING);
        return false;
    }
    // This run is told what Setup just made of its folder (it decided "installed" at its start,
    // when no settings.txt was there), so its log and the bar it runs keep their data here.
    bevel_winkit::log::became_portable(folder);
    log("setup: portable (settings and log next to the program)");
    let placed = place_programs(&staged, folder);
    placed.lines.iter().for_each(|line| log(line));
    tell_left("set up", &placed.left);
    if bundle.packed && !same_file(exe, &bar) {
        // The unpacked bar is the portable copy from now on, and is told so. The one file itself
        // never runs as the bar (an outside review of 2026-11-18).
        if start_with(&bar, folder, &[bevel_winkit::log::PORTABLE_FLAG]) {
            return false;
        }
        return not_started_here(bundle, &format!("{NAME} was set up in this folder but could not be started.\n\nStart {} in this folder.", package::BAR));
    }
    // The one file named bevel95.exe is the portable bar itself, but only with all its programs
    // beside it now (one that could not be put in place and was not there before would be
    // missing).
    if bundle.packed && !programs_beside(exe) {
        log("setup: portable, but not every program could be put beside it; it does not run");
        message(&format!("{NAME} could not put all its programs in this folder.\n\nClose any of them that is open and run this file again."), MB_OK | MB_ICONWARNING);
        return false;
    }
    true
}

/// Update: everything is written beside its place first; then the running bar (if any)
/// closes, the files are put in place, and the installed copy starts. A companion that is
/// running keeps its old version, and Setup says so.
fn update(bundle: &Bundle, dir: &Path) -> bool {
    let installed = dir.join(package::BAR);
    let staged = match stage_all(bundle, dir, true) {
        Ok(staged) => staged,
        Err(e) => {
            log(&format!("setup: could not write the new files ({e}); nothing was changed"));
            message(&format!("{NAME} could not write the new version into\n{}\n({e}).\n\nNothing was changed.", dir.display()), MB_OK | MB_ICONWARNING);
            return false;
        }
    };
    let (bar, programs): (Vec<Staged>, Vec<Staged>) = staged.into_iter().partition(|s| s.shown == package::BAR);
    let was_running = close_running_bar(&installed);
    // An old bar whose windows are gone but which is still finishing a copy or move of the
    // owner's files (decisions.md, "File jobs outlive their window") cannot be replaced, and is
    // never ended by force: nothing is changed, and Setup says so. Its place is free, so the bar
    // that was running (the old version) is started again meanwhile.
    let still = wait_for_copies(&installed, std::process::id(), FINISH_WAIT);
    if !still.is_empty() {
        bar.iter().chain(&programs).for_each(Staged::discard);
        let said = still_running_message(&still, true);
        log(&format!(
            "setup: {} copy(ies) of bevel95.exe still run ({}); nothing was changed",
            still.len(),
            if said == STILL_FINISHING_UPDATE { "the old bar is finishing a file job" } else { "not finishing a file job" }
        ));
        if was_running {
            start(&installed, dir);
        }
        message(said, MB_OK | MB_ICONINFORMATION);
        return false;
    }
    if let Some(bar) = bar.first() {
        let mut copied = Err(std::io::Error::other("not tried"));
        for _ in 0..10 {
            copied = fs::rename(&bar.temp, &bar.target);
            if copied.is_ok() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(300)); // the old one may still be ending
        }
        if let Err(e) = copied {
            bar.discard();
            programs.iter().for_each(Staged::discard);
            log(&format!("setup: could not replace {} ({e}); nothing was changed", path_kind(&installed)));
            if was_running {
                start(&installed, dir);
            }
            message(&format!("{NAME} could not replace\n{}\n({e}).\n\nNothing was changed.", installed.display()), MB_OK | MB_ICONWARNING);
            return false;
        }
    }
    let placed = place_programs(&programs, dir);
    placed.lines.iter().for_each(|line| log(line));
    make_shortcuts(&installed, dir);
    write_entry(dir);
    log(&format!(
        "setup: updated {} ({})",
        path_kind(&installed),
        if bundle.packed { "from the one file, fingerprints checked" } else { "from the files beside it" }
    ));
    tell_left("updated", &placed.left);
    if start(&installed, dir) {
        return false;
    }
    not_started_here(bundle, &format!("{NAME} was updated in\n{}\nbut could not be started.\n\nStart it with the {NAME} shortcut on your desktop.", dir.display()))
}

/// The installed-apps entry's key (under HKEY_CURRENT_USER).
fn entry_key() -> String {
    autostart::base_key() + "\\Uninstall\\Bevel95"
}

/// What the installed-apps entry holds for an installed copy in `dir`.
fn entry_values(dir: &Path, size_kb: u32) -> Vec<(&'static str, RegValue)> {
    let exe = dir.join(package::BAR);
    vec![
        ("DisplayName", RegValue::Text(NAME.to_string())),
        ("DisplayVersion", RegValue::Text(VERSION.to_string())),
        // PROVISIONAL: the publisher is the name until the owner decides.
        ("Publisher", RegValue::Text(NAME.to_string())),
        ("DisplayIcon", RegValue::Text(format!("{},0", exe.display()))),
        ("InstallLocation", RegValue::Text(dir.display().to_string())),
        ("UninstallString", RegValue::Text(format!("\"{}\" --uninstall", exe.display()))),
        ("NoModify", RegValue::Number(1)),
        ("NoRepair", RegValue::Number(1)),
        ("EstimatedSize", RegValue::Number(size_kb)),
    ]
}

/// The installed programs' size in KB (Windows shows it in its list).
fn installed_size_kb(dir: &Path) -> u32 {
    let size = |p: PathBuf| fs::metadata(p).map(|m| m.len()).unwrap_or(0);
    let bytes = size(dir.join(package::BAR))
        + start_menu::companions().map(|(_, file)| size(dir.join("programs").join(file))).sum::<u64>();
    bytes.div_ceil(1024).min(u64::from(u32::MAX)) as u32
}

/// Writes (or refreshes) the entry in Windows' list of installed apps for the copy in `dir`.
fn write_entry(dir: &Path) {
    match autostart::write_values(&entry_key(), &entry_values(dir, installed_size_kb(dir))) {
        Ok(()) => log(&format!("setup: the installed-apps entry was written (version {VERSION})")),
        Err(e) => log(&format!("setup: the installed-apps entry could not be written: {e}")),
    }
}

/// Takes the installed-apps entry away, if it is the one of the copy in `dir`.
fn remove_entry(dir: &Path) {
    let key = entry_key();
    let Some(location) = autostart::read_text(&key, "InstallLocation") else { return };
    if !same_file(Path::new(&location), dir) {
        log("setup: the installed-apps entry names another folder; it was left");
        return;
    }
    match autostart::delete_key(&key) {
        Ok(()) => log("setup: the installed-apps entry was removed"),
        Err(e) => log(&format!("setup: the installed-apps entry could not be removed: {e}")),
    }
}

/// Where the desktop shortcuts go: the desktop, or in the test mode a Desktop folder beside the
/// installed folder.
fn desktop_dir() -> Option<PathBuf> {
    if autostart::test_key().is_some() {
        let dir = installed_dir().parent()?.join("Desktop");
        fs::create_dir_all(&dir).ok()?;
        return Some(dir);
    }
    known_folder(&FOLDERID_Desktop)
}

fn plain_dir(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.is_dir() && m.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0)
}

/// Only the earlier Bevel95 game should be retired, even if someone has another
/// program with this name. These identifiers are present across its old builds.
fn is_flat_game(path: &Path) -> bool {
    let Ok(metadata) = fs::symlink_metadata(path) else { return false };
    if !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 || metadata.len() > 32 * 1024 * 1024 {
        return false;
    }
    let Ok(data) = fs::read(path) else { return false };
    data.starts_with(b"MZ") && ["BEVEL95_MINEFIELD_SEED", "Bevel95Window", "minefield.txt"].iter()
        .all(|id| data.windows(id.len()).any(|w| w == id.as_bytes()))
}

fn retire_flat_game(dir: &Path, lines: &mut Vec<String>) -> bool {
    let file = dir.join("minefield.exe");
    if !plain_dir(dir) { return false; }
    if file.is_file() && let Err(e) = fs::File::open(&file) {
        lines.push(format!("setup: could not check the old game ({e}); its files were kept"));
        return false;
    }
    if is_flat_game(&file) && let Err(e) = fs::remove_file(file) {
        lines.push(format!("setup: old game still in use ({e}); close it and retry Setup"));
        return false;
    }
    true
}

/// Remove just the copies installed by earlier builds, even when Setup is running
/// on another computer with no reference cache. Other files in these folders stay.
fn clean_legacy_files(dir: &Path, manifest: &str, lines: &mut Vec<String>) {
    if !plain_dir(dir) {
        return;
    }
    for entry in manifest.lines() {
        let mut fields = entry.split_ascii_whitespace();
        let name = fields.next().unwrap();
        let expected_len: u64 = fields.next().unwrap().parse().unwrap();
        let expected_hash = fields.next().unwrap();
        let file = dir.join(name);
        let parent = file.parent().unwrap();
        if !parent.ancestors().take_while(|p| *p != dir).all(plain_dir) {
            continue;
        }
        let Ok(m) = fs::symlink_metadata(&file) else { continue };
        if !m.is_file() || m.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 || m.len() != expected_len {
            continue;
        }
        let result = (|| -> std::io::Result<()> {
            let mut data = fs::read(&file)?;
            if data.len() as u64 != expected_len {
                return Ok(());
            }
            if name.ends_with(".chw") {
                // HTML Help stamps these two fields when generating the Win98 keyword index.
                data[16..20].fill(0);
                data[358..366].fill(0);
            }
            let mut hash = [0u8; 32];
            let mut size = hash.len() as u32;
            // SAFETY: data and hash cover their stated lengths. SHA256 is NUL-terminated.
            let ok = unsafe {
                CryptHashCertificate2(windows_sys::w!("SHA256"), 0, std::ptr::null(), data.as_ptr(), data.len() as u32,
                    hash.as_mut_ptr(), &mut size)
            };
            if ok == 0 {
                return Err(std::io::Error::last_os_error());
            }
            if hash.iter().map(|b| format!("{b:02X}")).collect::<String>() == expected_hash {
                fs::remove_file(&file)?;
                for folder in parent.ancestors().take_while(|p| *p != dir) {
                    let _ = fs::remove_dir(folder); // succeeds only if now empty
                }
            }
            Ok(())
        })();
        if let Err(e) = result {
            lines.push(format!("setup: could not clean old file {name} ({e})"));
        }
    }
}

/// Opening these files for write fails if a running game or viewer is using its image.
/// Keep the handles until removal so another launch cannot slip in after this check.
fn removable_programs(dir: &Path) -> std::io::Result<Vec<(PathBuf, fs::File)>> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_DELETE, FILE_SHARE_READ};
    let programs = dir.join("programs");
    let mut files = Vec::new();
    if plain_dir(&programs) {
        for (_, name) in start_menu::companions() {
            let path = programs.join(name);
            if fs::symlink_metadata(&path).is_ok_and(|m| m.is_file() && m.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0) {
                files.push(path);
            }
        }
    }
    let previous = dir.join("minefield.exe");
    if is_flat_game(&previous) {
        files.push(previous);
    }
    files.into_iter().map(|path| {
        let handle = fs::OpenOptions::new().write(true).share_mode(FILE_SHARE_READ | FILE_SHARE_DELETE).open(&path)
            .map_err(|e| std::io::Error::new(e.kind(), format!("{} ({e})", path.display())))?;
        Ok((path, handle))
    }).collect()
}

/// Remove the installed programs, state, matching shortcuts and the installed-apps entry.
/// Directory removal is always nonrecursive, so any files the owner has added stay in place.
fn remove(dir: &Path, here_installed: bool) {
    use std::os::windows::ffi::OsStringExt;
    let question = format!(
        "Remove Bevel95 from this computer?\n\n\
         This removes the installed programs, their shortcuts, settings, game scores and logs from\n{}",
        dir.display()
    );
    if message(&question, MB_YESNO | MB_ICONWARNING) != IDYES {
        return;
    }
    if !plain_dir(dir) {
        message(concat!("The installed folder is missing or redirected. Setup could not remove ", bevel_core::product_name!(), "."), MB_OK | MB_ICONWARNING);
        return;
    }
    let files = match removable_programs(dir) {
        Ok(files) => files,
        Err(e) => {
            message(&format!("Setup cannot remove this program:\n{e}\n\nClose this program and try Remove again."), MB_OK | MB_ICONWARNING);
            return;
        }
    };
    let installed = dir.join("bevel95.exe");
    close_running_bar(&installed);
    // The bar's watchdog (a second bevel95.exe) wakes when the bar ends and writes its last log
    // line a few milliseconds later: deleting before it has ended left a new bevel95.log, and with
    // it the folder (the real Remove of 2026-11-17). So wait for every other copy of the installed
    // program to end first.
    let started = std::time::Instant::now();
    let still_running = wait_for_copies(&installed, std::process::id(), REMOVE_WAIT);
    let waited = started.elapsed().as_millis();
    if !still_running.is_empty() {
        // The bar's windows are gone, but it is still finishing a copy or move of the owner's
        // files (it holds the mark that says so), or another copy did not end: it is never ended
        // by force, and nothing is removed.
        let said = still_running_message(&still_running, false);
        log(&format!(
            "setup: Remove waited {waited} ms; {} other copy(ies) of bevel95.exe still running ({}); nothing was removed",
            still_running.len(),
            if said == STILL_FINISHING_REMOVE { "an old bar finishing a file job" } else { "not finishing a file job" }
        ));
        message(said, MB_OK | MB_ICONINFORMATION);
        return;
    }
    log(&format!("setup: Remove waited {waited} ms for the bar and its watchdog to end"));
    if autostart::points_to(&installed) {
        autostart::set(false);
    }
    remove_entry(dir);
    let mut errors = Vec::new();
    let remove_file = |path: &Path, errors: &mut Vec<String>| {
        if let Err(e) = fs::remove_file(path) && e.kind() != std::io::ErrorKind::NotFound {
            errors.push(format!("{} ({e})", path.display()));
        }
    };
    if let Some(desktop) = desktop_dir() {
        for (name, ..) in SHORTCUTS {
            let link = desktop.join(format!("{name}.lnk"));
            if let Ok(bytes) = fs::read(&link) && let Some(target) = start_menu::local_path(&bytes)
                && same_file(Path::new(&std::ffi::OsString::from_wide(&target)), &installed) {
                remove_file(&link, &mut errors);
            }
        }
    }
    for (file, _handle) in files {
        remove_file(&file, &mut errors);
    }
    if plain_dir(&dir.join("programs")) {
        let _ = fs::remove_dir(dir.join("programs"));
    }
    clean_legacy_files(dir, LEGACY_FILES, &mut errors);
    // A save cut short left its temporary file (its own names only, a minute old): it goes too, so
    // the folder can go.
    state_file::clear_leftovers(dir, std::time::Duration::from_secs(60));
    for name in DATA_FILES {
        let file = dir.join(name);
        if fs::symlink_metadata(&file).is_ok_and(|m| m.is_file() && m.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0) {
            remove_file(&file, &mut errors);
        }
    }
    if here_installed {
        if !delete_after_exit(dir) {
            errors.push(format!("Could not schedule removal of {}", installed.display()));
        }
    } else {
        remove_file(&installed, &mut errors);
        let _ = fs::remove_dir(dir);
    }
    if errors.is_empty() {
        let done = if here_installed {
            concat!("Close this message to finish removing ", bevel_core::product_name!(), ".")
        } else {
            concat!(bevel_core::product_name!(), " was removed from this computer.")
        };
        message(done, MB_OK | MB_ICONINFORMATION);
    } else {
        message(&format!("Setup could not remove every file:\n{}", errors.join("\n")), MB_OK | MB_ICONWARNING);
    }
}

/// How long Remove waits for the bar and its watchdog to end. A copy still running after it is,
/// as a rule, an old bar finishing a file job (`still_running_message` tells): nothing is removed
/// then.
const REMOVE_WAIT: std::time::Duration = std::time::Duration::from_secs(5);

/// How long Update waits for the old bar to end before it says the bar is still finishing a job.
const FINISH_WAIT: std::time::Duration = std::time::Duration::from_secs(10);

/// Setup's words when the old bar is still finishing a copy or move (PROVISIONAL: Bevel95's own).
const STILL_FINISHING_UPDATE: &str = concat!(
    bevel_core::product_name!(),
    " is still finishing a copy or move of your files that was started before this update (Windows' progress \
     window shows how far it is), so its program cannot be replaced yet.\n\nNothing was changed. Run this Setup \
     again once the copy or move has finished."
);
const STILL_FINISHING_REMOVE: &str = concat!(
    bevel_core::product_name!(),
    " is still finishing a copy or move of your files (Windows' progress window shows how far it is). Windows' \
     taskbar is back meanwhile.\n\nNothing was removed. Run Remove again once the copy or move has finished."
);
/// Setup's words when another copy of the installed program still runs and is not finishing a
/// file job (PROVISIONAL: Bevel95's own).
const STILL_RUNNING_UPDATE: &str = concat!(
    "Another copy of ",
    bevel_core::product_name!(),
    "'s program is still running and did not end, so it cannot be replaced.\n\nNothing was changed. Restart the \
     computer, or end it in Task Manager, and run this Setup again."
);
const STILL_RUNNING_REMOVE: &str = concat!(
    "Another copy of ",
    bevel_core::product_name!(),
    "'s program is still running and did not end, so it cannot be removed.\n\nNothing was removed. Restart the \
     computer, or end it in Task Manager, and run Remove again."
);

/// What Setup says when another copy of the installed bevel95.exe still runs after its wait:
/// "still finishing a copy or move" only when one of them holds the mark of a bar finishing its
/// file jobs (`jobs::is_finishing`); otherwise that a copy did not end (the second safety review
/// of 2026-11-18, B1: it said "finishing a copy" for any copy that still ran).
fn still_running_message(pids: &[u32], update: bool) -> &'static str {
    match (pids.iter().any(|&pid| bevel_winkit::jobs::is_finishing(pid)), update) {
        (true, true) => STILL_FINISHING_UPDATE,
        (true, false) => STILL_FINISHING_REMOVE,
        (false, true) => STILL_RUNNING_UPDATE,
        (false, false) => STILL_RUNNING_REMOVE,
    }
}

/// Waits, at most `limit`, until no process other than `except` runs the program file `image`.
/// Returns the process numbers of those that still run when the wait ends (none: all ended). A
/// process whose file cannot be asked (another user's, or one that ended meanwhile) is not
/// counted.
fn wait_for_copies(image: &Path, except: u32, limit: std::time::Duration) -> Vec<u32> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE, WAIT_OBJECT_0, WAIT_TIMEOUT};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
        QueryFullProcessImageNameW, WaitForSingleObject,
    };
    let Some(file_name) = image.file_name().map(|n| n.to_string_lossy().to_lowercase()) else { return Vec::new() };
    let deadline = std::time::Instant::now() + limit;
    let mut handles = Vec::new();
    // SAFETY: a process snapshot walked with a correctly sized entry and closed after use; each
    // process handle is asked for its file with a sized buffer, waited on and closed.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            crate::log::log_win32("setup: CreateToolhelp32Snapshot");
            return Vec::new();
        }
        let mut entry = PROCESSENTRY32W { dwSize: size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut more = Process32FirstW(snapshot, &mut entry) != 0;
        while more {
            let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..len]).to_lowercase();
            if entry.th32ProcessID != except && name == file_name {
                let process = OpenProcess(PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, 0, entry.th32ProcessID);
                if !process.is_null() {
                    // Asked on the open handle, so a number used again by another program is not
                    // mistaken for one of ours.
                    let mut buf = vec![0u16; 1024];
                    let mut size = buf.len() as u32;
                    let ours = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut size) != 0
                        && same_file(Path::new(&String::from_utf16_lossy(&buf[..size as usize])), image);
                    if ours {
                        handles.push((process, entry.th32ProcessID));
                    } else {
                        CloseHandle(process);
                    }
                }
            }
            more = Process32NextW(snapshot, &mut entry) != 0;
        }
        CloseHandle(snapshot);
        let mut running = Vec::new();
        for (process, pid) in handles {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            match WaitForSingleObject(process, left.as_millis().min(u32::MAX as u128 - 1) as u32) {
                WAIT_OBJECT_0 => {}
                WAIT_TIMEOUT => running.push(pid),
                _ => crate::log::log_win32("setup: WaitForSingleObject"),
            }
            CloseHandle(process);
        }
        running
    }
}

/// Wait for this exact process before removing its executable. The directory is
/// passed as data and is removed only when empty.
fn delete_after_exit(dir: &Path) -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let Some(windows) = std::env::var_os("SystemRoot") else { return false };
    let Ok(image) = fs::symlink_metadata(dir.join("bevel95.exe")) else { return false };
    let shell = PathBuf::from(windows).join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
    const DELETE_SELF: &str = r#"
        Wait-Process -Id ([int]$env:BEVEL95_SETUP_PID) -ErrorAction SilentlyContinue
        $folder = [IO.DirectoryInfo]::new($env:BEVEL95_SETUP_DIR)
        if (!$folder.Exists -or ($folder.Attributes -band [IO.FileAttributes]::ReparsePoint)) { exit }
        $file = [IO.FileInfo]::new([IO.Path]::Combine($folder.FullName, 'bevel95.exe'))
        for ($i = 0; $i -lt 20; $i++) {
            $file.Refresh()
            if (!$file.Exists -or ($file.Attributes -band [IO.FileAttributes]::ReparsePoint) -or
                $file.LastWriteTimeUtc.ToFileTimeUtc() -ne [int64]$env:BEVEL95_SETUP_WRITTEN) { exit }
            try { $file.Delete(); break } catch [IO.IOException] { Start-Sleep -Milliseconds 250 }
        }
        try { $folder.Delete($false) } catch [IO.IOException] {}
    "#;
    std::process::Command::new(shell)
        .args(["-NoProfile", "-NonInteractive", "-Command", DELETE_SELF])
        .env("BEVEL95_SETUP_DIR", dir).env("BEVEL95_SETUP_PID", std::process::id().to_string())
        .env("BEVEL95_SETUP_WRITTEN", image.last_write_time().to_string())
        .current_dir(std::env::temp_dir()).creation_flags(CREATE_NO_WINDOW).spawn().is_ok()
}

/// Closes a running bar (Windows' taskbar comes back) the way the desktop shortcut "Restore
/// Windows taskbar" does, and waits for it.
fn close_running_bar(installed: &Path) -> bool {
    if running_bar_is(installed) {
        return std::process::Command::new(installed).arg("--restore").status().is_ok_and(|status| status.success());
    }
    false
}

/// Whether the bar running now (its window) is the program file `installed`.
fn running_bar_is(installed: &Path) -> bool {
    let class = wide(bar::BAR_CLASS);
    // SAFETY: a valid class name.
    let window = unsafe { FindWindowW(class.as_ptr(), std::ptr::null()) };
    !window.is_null() && same_file(Path::new(&crate::window_list::program_path(window)), installed)
}

/// Shows the look's own message box (`confirm::show_message`: the look's frame, text and OK) in
/// the middle of the main screen and waits until it is gone. Falls back to Windows' message box
/// when the look's font cannot be had.
#[cfg(not(test))]
fn say_in_look(era: Era, depth: Depth, text: &str) {
    use bevel_winkit::confirm::{self, MessageHost, Place};
    bevel_winkit::look::dpi_aware();
    let (screen, dpi) = bevel_winkit::look::main_screen();
    let look = match bevel_winkit::look::for_look(era, depth, scale_for_dpi(dpi)) {
        Ok(look) => look,
        Err(e) => {
            log(&format!("setup: cannot use the look's font ({e}); Windows' message box instead"));
            message(text, MB_OK | MB_ICONINFORMATION);
            return;
        }
    };
    confirm::show_message(Place { look, era, kind: kit::Kind::Tool, work: screen }, NAME, text);
    kit::run_until_closed::<MessageHost>();
}

/// The tests show no window: what it would say is kept (`SAID`), as if OK was pressed.
#[cfg(test)]
fn say_in_look(_era: Era, _depth: Depth, text: &str) {
    SAID.with(|s| s.borrow_mut().push(text.to_string()));
}

/// Starts the installed copy; true if it started.
fn start(installed: &Path, dir: &Path) -> bool {
    start_with(installed, dir, &[])
}

/// Starts the bar `bar` with `args`; true if it started.
fn start_with(installed: &Path, dir: &Path, args: &[&str]) -> bool {
    match std::process::Command::new(installed).args(args).current_dir(dir).spawn() {
        Ok(_) => true,
        Err(e) => {
            log(&format!("setup: could not start {} ({e})", path_kind(installed)));
            false
        }
    }
}

/// Makes the desktop shortcuts that are missing, pointing at `installed`.
fn make_shortcuts(installed: &Path, dir: &Path) {
    let Some(desktop) = desktop_dir() else { return };
    audio::init();
    let mut made = Vec::new();
    for (name, arguments, description) in SHORTCUTS {
        let link = desktop.join(format!("{name}.lnk"));
        if link.exists() {
            continue;
        }
        match save_shortcut(&link, installed, arguments, description, dir) {
            Ok(()) => made.push(name),
            Err(what) => log(&format!("setup: shortcut {name:?} not made ({what})")),
        }
    }
    log(&format!("setup: desktop shortcuts made: {made:?}"));
}

/// Whether the file at `path` holds exactly `bytes`.
fn same_bytes_as(bytes: &[u8], path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|m| m.is_file() && m.len() == bytes.len() as u64) && fs::read(path).is_ok_and(|b| b == bytes)
}

/// When a file was last written (its copy's date), for "newer" and "older".
fn modified(p: &Path) -> std::time::SystemTime {
    fs::metadata(p).and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH)
}
/// Whether two paths name the same file (the second may not exist yet).
fn same_file(a: &Path, b: &Path) -> bool {
    let canonical = |p: &Path| -> Option<PathBuf> {
        fs::canonicalize(p).ok().or_else(|| Some(fs::canonicalize(p.parent()?).ok()?.join(p.file_name()?)))
    };
    match (canonical(a), canonical(b)) {
        (Some(a), Some(b)) => a.as_os_str().eq_ignore_ascii_case(b.as_os_str()),
        _ => false,
    }
}

/// A plain message box (Windows' own look: it comes before any Bevel95 window exists).
#[cfg(not(test))]
fn message(text: &str, style: MESSAGEBOX_STYLE) -> i32 {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MB_SETFOREGROUND, MessageBoxW};
    let (text, title) = (wide(text), wide(NAME));
    // SAFETY: valid null-terminated strings.
    unsafe { MessageBoxW(null_mut(), text.as_ptr(), title.as_ptr(), style | MB_SETFOREGROUND) }
}

thread_local! {
    /// The tests' stand-in for the message boxes: what was said.
    #[cfg(test)]
    static SAID: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The tests show no message box: what it would say is kept (`SAID`), as if OK was pressed.
#[cfg(test)]
fn message(text: &str, _style: MESSAGEBOX_STYLE) -> i32 {
    SAID.with(|s| s.borrow_mut().push(text.to_string()));
    windows_sys::Win32::UI::WindowsAndMessaging::IDOK
}

/// Writes a shortcut (.lnk) with Windows' own ShellLink object. COM must be initialized.
fn save_shortcut(link: &Path, target: &Path, arguments: &str, description: &str, dir: &Path) -> Result<(), String> {
    let mut shell_link: *mut c_void = null_mut();
    // SAFETY: CoCreateInstance with valid GUIDs and out-pointer; every method is called through
    // IShellLinkW's or IPersistFile's documented table, and both references are released.
    unsafe {
        let hr = CoCreateInstance(&ShellLink, null_mut(), CLSCTX_INPROC_SERVER, &IID_SHELL_LINK, &mut shell_link);
        if hr < 0 || shell_link.is_null() {
            return Err(format!("CoCreateInstance(ShellLink) 0x{hr:08X}"));
        }
        let set = |slot: usize, text: &str| {
            let text = wide(text);
            method::<SetText>(shell_link, slot)(shell_link, text.as_ptr())
        };
        let mut result = Ok(());
        for (slot, text, what) in [
            (SET_PATH, target.display().to_string(), "SetPath"),
            (SET_ARGUMENTS, arguments.to_string(), "SetArguments"),
            (SET_WORKING_DIRECTORY, dir.display().to_string(), "SetWorkingDirectory"),
            (SET_DESCRIPTION, description.to_string(), "SetDescription"),
        ] {
            let hr = set(slot, &text);
            if hr < 0 {
                result = Err(format!("{what} 0x{hr:08X}"));
            }
        }
        // Saved as a new file only: anything already under that name is never written over.
        if result.is_ok() {
            result = crate::file_ops::save_shell_link_new(shell_link, link);
        }
        release(shell_link);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one file started again while that very build runs tells so (the owner, 2026-11-18):
    /// the bar running is told by its window's program file. Only read: a file elsewhere is never
    /// the running bar; and when a bar runs on this PC, its own file is.
    #[test]
    fn the_running_bar_is_told_by_its_file() {
        let elsewhere = std::env::temp_dir().join(format!("bevel95-not-running-{}", std::process::id())).join("bevel95.exe");
        assert!(!running_bar_is(&elsewhere));
        let class = wide(bar::BAR_CLASS);
        // SAFETY: a valid class name.
        let window = unsafe { FindWindowW(class.as_ptr(), std::ptr::null()) };
        let file = if window.is_null() { String::new() } else { crate::window_list::program_path(window) };
        if !file.is_empty() {
            assert!(running_bar_is(Path::new(&file)), "the running bar's own file");
        }
        assert_eq!(bevel_core::setup::already_running("0.1.0"), "Bevel95 0.1.0 is already installed and running.");
        say_in_look(Era::Win2000, Depth::TrueColor, "said");
        assert_eq!(SAID.with(|s| s.borrow().last().cloned()), Some("said".to_string()));
    }

    /// A downloaded copy beside another program's settings.txt is not portable, and Setup's
    /// Portable there refuses and writes nothing (it took the folder as Bevel95's: the safety
    /// review of 2026-11-18, A3).
    #[test]
    fn portable_refuses_another_programs_settings() {
        let root = std::env::temp_dir().join(format!("bevel95-foreign-settings-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let exe = root.join("Bevel95-0.1.0.exe");
        fs::write(&exe, b"MZ the one file").unwrap();
        let theirs: Vec<u8> = [0xFEFFu16].into_iter().chain("[Options]\r\nscale=2\r\n".encode_utf16()).flat_map(u16::to_le_bytes).collect();
        let settings = root.join("settings.txt");
        let list = |dir: &Path| {
            let mut names: Vec<_> = fs::read_dir(dir).unwrap().flatten().map(|e| e.file_name()).collect();
            names.sort();
            names
        };
        let program = |file: &'static str| Program { name: file, file, bytes: b"MZ".to_vec(), date: std::time::SystemTime::now() };
        let bundle = Bundle { bar: b"MZ bar".to_vec(), programs: start_menu::companions().map(|(_, f)| program(f)).collect(), packed: true };
        for text in [theirs.as_slice(), b"[General]\nscale = 2\n", b""] {
            fs::write(&settings, text).unwrap();
            assert!(!bevel_winkit::log::is_portable_folder(&exe), "not portable: Setup shows");
            assert!(holds_foreign_settings(&root));
            let before = list(&root);
            SAID.with(|s| s.borrow_mut().clear());
            assert!(!portable_here(&exe, &bundle), "refused");
            assert_eq!(SAID.with(|s| s.borrow().clone()), [bevel_core::setup::FOREIGN_SETTINGS]);
            assert_eq!(fs::read(&settings).unwrap(), text, "their file is unchanged");
            assert_eq!(list(&root), before, "nothing was written");
        }
        // Bevel95's own settings.txt: portable, as before.
        fs::write(&settings, bevel_core::config::DEFAULT_FILE).unwrap();
        assert!(bevel_winkit::log::is_portable_folder(&exe) && !holds_foreign_settings(&root));
        // Bevel95's own, but held open by another program so that it cannot be read: not known,
        // so not portable (a start by the person then ends before Setup: `log::decide_at_start`),
        // and a Portable chosen while it became held refuses with its own message (the third
        // safety review of 2026-11-18, A1: such a folder was taken as a portable copy's).
        {
            use std::os::windows::fs::OpenOptionsExt;
            let held = fs::OpenOptions::new().read(true).share_mode(0).open(&settings).unwrap();
            assert!(!bevel_winkit::log::is_portable_folder(&exe), "not known: not portable");
            let before = list(&root);
            SAID.with(|s| s.borrow_mut().clear());
            assert!(!portable_here(&exe, &bundle), "refused");
            assert_eq!(SAID.with(|s| s.borrow().clone()), [bevel_core::setup::SETTINGS_IN_USE]);
            assert_eq!(list(&root), before, "nothing was written");
            drop(held);
        }
        // One that cannot be read for a lasting reason (access denied): not portable, so Setup
        // shows as on a first run, and Portable refuses saying the file cannot be read, not
        // asking to close a program (the fifth safety review of 2026-11-18, B2).
        {
            let icacls = |args: &[&str]| {
                let ok = bevel_winkit::quiet::command(bevel_winkit::shell::system_program("icacls.exe")).arg(&settings).args(args).stdout(std::process::Stdio::null()).status();
                assert!(ok.is_ok_and(|s| s.success()), "icacls {args:?}");
            };
            icacls(&["/deny", "*S-1-1-0:(R)"]);
            let portable = bevel_winkit::log::is_portable_folder(&exe);
            let before = list(&root);
            SAID.with(|s| s.borrow_mut().clear());
            let refused = !portable_here(&exe, &bundle);
            let after = list(&root);
            icacls(&["/remove:d", "*S-1-1-0"]);
            assert!(!portable, "cannot be read: not portable");
            assert!(refused, "refused");
            assert_eq!(SAID.with(|s| s.borrow().clone()), [bevel_core::setup::SETTINGS_UNREADABLE]);
            assert_eq!(after, before, "nothing was written");
        }
        assert_eq!(fs::read(&settings).unwrap(), bevel_core::config::DEFAULT_FILE.as_bytes());
        fs::remove_dir_all(root).unwrap();
    }

    /// An outside review of 2026-11-18: the one file never runs as a bar without its programs.
    /// A failed Install says nothing was changed and does not run it (it said "It runs from where
    /// it is" and ran a bar without programs); a copy as built keeps that way. Without Setup (a
    /// portable folder, a PC set up before Setup) the one file runs only with its programs beside
    /// it. Tried on a scratch folder under private\probes\codex2 whose writing is denied.
    #[test]
    fn the_one_file_never_runs_as_a_bar_after_a_failed_setup() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..\\..\\private\\probes\\codex2\\unit\\Bevel95 test setup");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let program = |file: &'static str| Program { name: file, file, bytes: b"MZ".to_vec(), date: std::time::SystemTime::now() };
        let programs: Vec<(&str, &[u8])> = start_menu::companions().map(|(_, f)| (f, b"MZ".as_slice())).collect();
        // Without Setup: the one file alone does not run; with its programs beside it, it does; a
        // copy as built alone as before.
        let one = root.join("Bevel95-test.exe");
        fs::write(&one, package::pack(b"MZ bar", &programs)).unwrap();
        let built = root.join("Bevel95 test built.exe");
        fs::write(&built, b"MZ as built").unwrap();
        assert!(is_one_file(&one) && !is_one_file(&built));
        assert!(!runs_without_setup(&one), "the one file without its programs: Setup shows");
        assert!(runs_without_setup(&built), "a copy as built: as before");
        fs::create_dir_all(root.join("programs")).unwrap();
        for (file, bytes) in &programs {
            fs::write(root.join("programs").join(file), bytes).unwrap();
        }
        assert!(runs_without_setup(&one), "its programs beside it: it runs");
        // A failed Install into a folder whose writing is denied.
        let dir = root.join("Bevel95 test denied");
        fs::create_dir_all(&dir).unwrap();
        let icacls = |args: &[&str]| {
            let ok = bevel_winkit::quiet::command(bevel_winkit::shell::system_program("icacls.exe")).arg(&dir).args(args).stdout(std::process::Stdio::null()).status();
            assert!(ok.is_ok_and(|s| s.success()), "icacls {args:?}");
        };
        icacls(&["/deny", "*S-1-1-0:(WD,AD)"]);
        let denied = fs::File::create_new(dir.join("Bevel95 test probe.txt")).is_err();
        let mut results = Vec::new();
        if denied {
            // (Only with the writing really denied: else Install would go on to the desktop's
            // shortcuts and the registry.)
            for packed in [true, false] {
                let bundle = Bundle { bar: b"MZ bar".to_vec(), programs: start_menu::companions().map(|(_, f)| program(f)).collect(), packed };
                SAID.with(|s| s.borrow_mut().clear());
                let runs = install(&one, &bundle, &dir, &Answers::default());
                results.push((packed, runs, SAID.with(|s| s.borrow().clone()), fs::read_dir(&dir).map(|d| d.count()).unwrap_or(99)));
            }
        }
        icacls(&["/remove:d", "*S-1-1-0"]);
        assert!(denied, "the scratch folder's writing was denied");
        for (packed, runs, said, files) in results {
            assert_eq!(files, 0, "nothing was written");
            assert_eq!(said.len(), 1);
            if packed {
                assert!(!runs, "the one file does not run as a bar");
                assert!(said[0].ends_with("Nothing was changed.") && !said[0].contains("runs from where it is"), "{}", said[0]);
            } else {
                assert!(runs, "a copy as built runs from where it is, as before");
                assert!(said[0].ends_with("It runs from where it is."));
            }
        }
        // Setup's other ways out for the one file: a message, no bar; a copy as built: as before.
        let bundle = |packed| Bundle { bar: Vec::new(), programs: Vec::new(), packed };
        SAID.with(|s| s.borrow_mut().clear());
        assert!(!not_started_here(&bundle(true), "Bevel95 Setup could not be shown.\n\nNothing was changed."));
        assert!(not_started_here(&bundle(false), "not said"));
        assert_eq!(SAID.with(|s| s.borrow().clone()), ["Bevel95 Setup could not be shown.\n\nNothing was changed."]);
        let _ = fs::remove_dir_all(&root);
    }

    /// Every data file named in the source of the bar and its programs is one that Remove
    /// deletes (programs.txt and programs.old.txt were missing, so the folder stayed), and
    /// README's "Files it uses" names each of them.
    #[test]
    fn remove_knows_every_data_file() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut dirs = vec![root.join("platform/win/src"), root.join("platform/winkit/src")];
        dirs.extend(fs::read_dir(root.join("programs")).unwrap().flatten().map(|e| e.path().join("src")));
        let mut named = std::collections::BTreeSet::new();
        while let Some(dir) = dirs.pop() {
            for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    dirs.push(path);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    let text = fs::read_to_string(&path).unwrap();
                    // Each string of plain letters that ends in .txt or .log.
                    for end in [".txt\"", ".log\""] {
                        for (at, _) in text.match_indices(end) {
                            let before = &text[..at];
                            let start = before.rfind(|c: char| !(c.is_ascii_alphanumeric() || "-_.".contains(c)));
                            if let Some(start) = start.filter(|&s| before[s..].starts_with('"')) {
                                named.insert(text[start + 1..at + end.len() - 1].to_string());
                            }
                        }
                    }
                }
            }
        }
        // Not data files: a shortcut's target in a test, and the end of "bevel95.old.log"
        // (the log is renamed with that extension), and the extension of the desktop's New Text
        // Document.
        for other in ["notes.txt", "old.log", ".txt"] {
            named.remove(other);
        }
        assert!(named.contains("programs.txt") && named.contains("programs.old.txt"), "{named:?}");
        let missing: Vec<&String> = named.iter().filter(|name| !DATA_FILES.contains(&name.as_str())).collect();
        assert!(missing.is_empty(), "Remove would leave {missing:?} behind");
        let readme = fs::read_to_string(root.join("README.md")).unwrap();
        for name in DATA_FILES {
            assert!(readme.contains(&format!("`{name}`")), "README's \"Files it uses\" does not name {name}");
        }
        // The state files whose save leftovers are cleared are all Bevel95's own data files.
        for own in state_file::OWN_FILES {
            assert!(DATA_FILES.contains(&own), "{own} is cleared after a save but not in Remove's list");
        }
    }

    /// Remove's wait for the bar and its watchdog, on a throwaway program: a copy of cmd.exe in
    /// a scratch folder, started twice (one ends after about a second, one stays until ended),
    /// stands for the installed bevel95.exe.
    #[test]
    fn remove_waits_for_other_copies() {
        use std::time::{Duration, Instant};
        let dir = std::env::temp_dir().join(format!("bevel95-wait-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let system = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join(r"System32\cmd.exe");
        let copy = dir.join("waittest.exe");
        fs::copy(&system, &copy).unwrap();
        // Nothing runs it yet: no wait at all.
        let start = Instant::now();
        assert!(wait_for_copies(&copy, std::process::id(), Duration::from_secs(5)).is_empty());
        assert!(start.elapsed() < Duration::from_millis(500));
        // A copy that ends by itself after about a second is waited for.
        let mut short = bevel_winkit::quiet::command(&copy).args(["/c", "ping -n 2 127.0.0.1 >nul"]).spawn().unwrap();
        std::thread::sleep(Duration::from_millis(100));
        let start = Instant::now();
        assert!(wait_for_copies(&copy, std::process::id(), Duration::from_secs(5)).is_empty());
        assert!(short.try_wait().unwrap().is_some(), "the wait returned before the copy ended");
        assert!(start.elapsed() < Duration::from_secs(4));
        // One that does not end: the wait stops at its limit and names the one that still runs;
        // the one named as the exception is not waited for.
        let mut long = bevel_winkit::quiet::command(&copy).args(["/c", "ping -n 30 127.0.0.1 >nul"]).spawn().unwrap();
        std::thread::sleep(Duration::from_millis(100));
        let start = Instant::now();
        let still = wait_for_copies(&copy, std::process::id(), Duration::from_millis(700));
        assert_eq!(still, [long.id()]);
        let took = start.elapsed();
        assert!(took >= Duration::from_millis(650) && took < Duration::from_secs(3), "{took:?}");
        // It is not finishing a file job (it holds no such mark): Setup says it did not end, never
        // that a copy or move of files is being finished (the second safety review of 2026-11-18,
        // B1). A process with the mark (this test's own) is finishing one.
        assert_eq!(still_running_message(&still, true), STILL_RUNNING_UPDATE);
        assert_eq!(still_running_message(&still, false), STILL_RUNNING_REMOVE);
        bevel_winkit::jobs::mark_finishing();
        let with_mark = [long.id(), std::process::id()];
        assert_eq!(still_running_message(&with_mark, true), STILL_FINISHING_UPDATE);
        assert_eq!(still_running_message(&with_mark, false), STILL_FINISHING_REMOVE);
        let start = Instant::now();
        assert!(wait_for_copies(&copy, long.id(), Duration::from_secs(5)).is_empty());
        assert!(start.elapsed() < Duration::from_millis(500));
        // Another program's file of the same name elsewhere is not ours.
        let other = dir.join("other");
        fs::create_dir_all(&other).unwrap();
        assert!(wait_for_copies(&other.join("waittest.exe"), std::process::id(), Duration::from_millis(300)).is_empty());
        long.kill().unwrap();
        long.wait().unwrap();
        let _ = short.wait();
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn shortcuts_are_written() {
        audio::init();
        let dir = std::env::temp_dir().join(format!("bevel95-shortcut-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let link = dir.join("Restore Windows taskbar.lnk");
        let target = std::env::current_exe().unwrap();
        save_shortcut(&link, &target, "--restore", "test", &dir).unwrap();
        let bytes = fs::read(&link).unwrap();
        fs::remove_dir_all(&dir).unwrap();
        // a shell link header, and the arguments stored as Unicode text
        assert_eq!(bytes[0], 0x4C);
        let arguments: Vec<u8> = "--restore".encode_utf16().flat_map(u16::to_le_bytes).collect();
        assert!(bytes.windows(arguments.len()).any(|w| w == arguments));
    }

    /// A file that is already under a desktop shortcut's name (one that appeared after Setup
    /// looked) is never written over: the shortcut is not made.
    #[test]
    fn a_shortcut_is_never_written_over_a_file() {
        audio::init();
        let dir = std::env::temp_dir().join(format!("bevel95-shortcut-over-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let link = dir.join("Bevel95.lnk");
        fs::write(&link, b"not Bevel95's").unwrap();
        let target = std::env::current_exe().unwrap();
        let r = save_shortcut(&link, &target, "", "test", &dir);
        let bytes = fs::read(&link).unwrap();
        fs::remove_dir_all(&dir).unwrap();
        assert!(r.is_err(), "not made");
        assert_eq!(bytes, b"not Bevel95's", "the file there is untouched");
    }

    #[test]
    fn upgrading_a_flat_install_keeps_data_and_retires_the_old_game_only_after_copy() {
        use std::os::windows::fs::OpenOptionsExt;
        // The application-container temp directory disallows renaming a file to .exe.
        // Exercise the same atomic replacement as Setup in Cargo's writable build area.
        let base = std::env::current_exe().unwrap().with_file_name(format!("bevel95-programs-test-{}", std::process::id()));
        let (from, to) = (base.join("from"), base.join("to"));
        fs::create_dir_all(from.join("programs")).unwrap();
        fs::create_dir_all(&to).unwrap();
        fs::write(from.join("bevel95.exe"), b"bar").unwrap();
        let source = from.join("bevel95.exe");
        let old = to.join("minefield.exe");
        fs::write(&old, b"MZ old executable: BEVEL95_MINEFIELD_SEED Bevel95Window minefield.txt").unwrap();
        fs::write(to.join("settings.txt"), b"portable settings").unwrap();
        fs::write(to.join("minefield.txt"), b"scores").unwrap();
        // A lone bar is a supported install. An existing working game stays where it is.
        assert!(copy_programs(&source, &to).is_empty());
        assert!(old.is_file());
        fs::write(from.join("programs/minefield.exe"), b"new game").unwrap();
        fs::write(from.join("programs/bevel-help.exe"), b"viewer").unwrap();
        fs::create_dir_all(from.join("Help/Win98")).unwrap();
        fs::write(from.join("Help/Win98/winmine.chm"), b"book").unwrap();
        fs::create_dir_all(from.join("minesweeper-assets")).unwrap();
        fs::write(from.join("minesweeper-assets/tick.wav"), b"tick").unwrap();
        // A running root game cannot be retired. New companions can still be installed.
        let locked = fs::OpenOptions::new().read(true).share_mode(1).open(&old).unwrap();
        let lines = copy_programs(&source, &to);
        assert!(lines.iter().any(|s| s.contains("old game still in use")), "{lines:?}");
        assert!(old.is_file());
        drop(locked);
        assert_eq!(fs::read(to.join("programs/minefield.exe")).unwrap(), b"new game");
        assert_eq!(fs::read(to.join("programs/bevel-help.exe")).unwrap(), b"viewer");
        assert!(!to.join("Help").exists() && !to.join("minesweeper-assets").exists());
        assert!(copy_programs(&source, &to).is_empty());
        assert!(!old.exists());
        assert_eq!(fs::read(to.join("settings.txt")).unwrap(), b"portable settings");
        assert_eq!(fs::read(to.join("minefield.txt")).unwrap(), b"scores");
        // An unrelated executable and a customized book with the old names must survive.
        fs::write(&old, b"MZ unrelated").unwrap();
        fs::create_dir_all(to.join("Help/Win98")).unwrap();
        let custom_book = to.join("Help/Win98/winmine.chm");
        fs::write(&custom_book, vec![0u8; 60_000]).unwrap();
        assert!(copy_programs(&to.join("bevel95.exe"), &to).is_empty());
        assert!(old.is_file());
        assert!(custom_book.is_file());
        assert!(from.join("Help/Win98/winmine.chm").is_file());
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn solitaire_install_and_update_preserve_player_data_and_the_running_game() {
        use std::os::windows::fs::OpenOptionsExt;
        let base = std::env::current_exe().unwrap().with_file_name(format!("bevel95-solitaire-install-test-{}", std::process::id()));
        let (from, to) = (base.join("from"), base.join("to"));
        fs::create_dir_all(from.join("programs")).unwrap();
        fs::create_dir_all(&to).unwrap();
        let bar = from.join("bevel95.exe");
        let solitaire = from.join("programs/solitaire.exe");
        let installed = to.join("programs/solitaire.exe");
        let settings = to.join("solitaire.txt");
        fs::write(&bar, b"bar").unwrap();
        fs::write(&solitaire, b"first build").unwrap();
        fs::write(from.join("solitaire.txt"), b"another player's options").unwrap();
        assert_eq!(copy_programs(&bar, &to), ["setup: installed Solitaire (programs/solitaire.exe)"]);
        assert_eq!(fs::read(&installed).unwrap(), b"first build");
        assert!(!settings.exists());
        fs::write(&settings, b"my settings").unwrap();

        fs::write(&solitaire, b"second build").unwrap();
        let running = fs::OpenOptions::new().read(true).share_mode(1).open(&installed).unwrap();
        assert!(copy_programs(&bar, &to).iter().any(|line| line.contains("could not update Solitaire")));
        assert!(removable_programs(&to).is_err());
        assert_eq!(fs::read(&installed).unwrap(), b"first build");
        drop(running);
        assert_eq!(copy_programs(&bar, &to), ["setup: installed Solitaire (programs/solitaire.exe)"]);
        assert_eq!(fs::read(&installed).unwrap(), b"second build");
        fs::remove_file(solitaire).unwrap();
        assert!(copy_programs(&bar, &to).is_empty());
        assert!(installed.is_file());
        assert_eq!(fs::read(settings).unwrap(), b"my settings");
        fs::remove_dir_all(&base).unwrap();
    }

    /// The one file: its programs come from inside it, checked, and go where an installed copy
    /// has them; a running companion is left as it was and named; a failure part way through
    /// writing leaves every old file as it was and no temporary file behind.
    #[test]
    fn the_one_file_unpacks_into_the_installed_layout() {
        use std::os::windows::fs::OpenOptionsExt;
        let base = std::env::current_exe().unwrap().with_file_name(format!("bevel95-one-file-test-{}", std::process::id()));
        let (download, to) = (base.join("download"), base.join("to"));
        fs::create_dir_all(&download).unwrap();
        let companions: Vec<(&str, &str)> = start_menu::companions().collect();
        let files: Vec<(String, Vec<u8>)> = companions.iter().map(|(_, f)| (f.to_string(), format!("MZ {f} 1").into_bytes())).collect();
        let list: Vec<(&str, &[u8])> = files.iter().map(|(f, b)| (f.as_str(), b.as_slice())).collect();
        let one = download.join("Bevel95-0.1.0.exe");
        fs::write(&one, package::pack(b"MZ bar 1", &list)).unwrap();
        let bundle = Bundle::of(&one, &to).unwrap();
        assert!(bundle.packed);
        assert_eq!(bundle.bar, b"MZ bar 1", "the bar is the front part only");
        assert_eq!(bundle.programs.len(), 5, "all five companions");
        let staged = stage_all(&bundle, &to, true).unwrap();
        assert_eq!(staged.len(), 6);
        for s in &staged {
            s.commit().unwrap();
        }
        assert_eq!(fs::read(to.join("bevel95.exe")).unwrap(), b"MZ bar 1");
        for (_, file) in &companions {
            assert_eq!(fs::read(to.join("programs").join(file)).unwrap(), format!("MZ {file} 1").into_bytes());
        }
        // The same again: nothing to write.
        assert!(stage_all(&bundle, &to, true).unwrap().is_empty());

        // A newer one file while Solitaire runs: everything else is updated, Solitaire is named.
        let files: Vec<(String, Vec<u8>)> = companions.iter().map(|(_, f)| (f.to_string(), format!("MZ {f} 2").into_bytes())).collect();
        let list: Vec<(&str, &[u8])> = files.iter().map(|(f, b)| (f.as_str(), b.as_slice())).collect();
        fs::write(&one, package::pack(b"MZ bar 2", &list)).unwrap();
        let bundle = Bundle::of(&one, &to).unwrap();
        let running = fs::OpenOptions::new().read(true).share_mode(1).open(to.join("programs/solitaire.exe")).unwrap();
        let staged = stage_all(&bundle, &to, false).unwrap();
        let placed = place_programs(&staged, &to);
        assert_eq!(placed.left, ["Solitaire"]);
        assert_eq!(fs::read(to.join("programs/solitaire.exe")).unwrap(), b"MZ solitaire.exe 1");
        assert_eq!(fs::read(to.join("programs/calculator.exe")).unwrap(), b"MZ calculator.exe 2");
        drop(running);

        // A program that cannot be written (its place is a folder): nothing changes at all.
        fs::write(&one, package::pack(b"MZ bar 3", &list)).unwrap();
        let bundle = Bundle::of(&one, &to).unwrap();
        fs::remove_file(to.join("programs/minefield.exe")).unwrap();
        fs::create_dir(to.join("programs/minefield.exe")).unwrap();
        assert!(stage_all(&bundle, &to, true).is_err());
        assert_eq!(fs::read(to.join("bevel95.exe")).unwrap(), b"MZ bar 1");
        let leftovers: Vec<_> = fs::read_dir(&to).unwrap().chain(fs::read_dir(to.join("programs")).unwrap()).flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp")).collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");

        // A damaged one file is refused before anything is written.
        let mut bytes = fs::read(&one).unwrap();
        bytes[3] ^= 1;
        fs::write(&one, bytes).unwrap();
        assert!(Bundle::of(&one, &to).err().is_some_and(|e| e.contains("fingerprint")));
        fs::remove_dir_all(&base).unwrap();
    }

    /// A download that stopped early: the one file without its last bytes has no trailer, so it
    /// reads as a copy as built with no programs beside it. Setup refuses it (it would have
    /// installed the bar alone and said it worked), and so it does any copy lacking one of the
    /// five; the one file whole, a copy as built with all five beside it and the installed copy
    /// itself go on. The one file over an installed copy cannot run where it is.
    #[test]
    fn setup_refuses_a_cut_one_file() {
        let base = std::env::current_exe().unwrap().with_file_name(format!("bevel95-cut-file-test-{}", std::process::id()));
        let (download, built, dir) = (base.join("download"), base.join("built"), base.join("installed"));
        fs::create_dir_all(&download).unwrap();
        fs::create_dir_all(&built).unwrap();
        let companions: Vec<(&str, &str)> = start_menu::companions().collect();
        let files: Vec<(String, Vec<u8>)> = companions.iter().map(|(_, f)| (f.to_string(), format!("MZ {f}").into_bytes())).collect();
        let list: Vec<(&str, &[u8])> = files.iter().map(|(f, b)| (f.as_str(), b.as_slice())).collect();
        let whole = package::pack(b"MZ the bar", &list);
        let one = download.join("Bevel95-0.1.0.exe");
        let kind = |bundle: &Bundle| other_copy(bundle, true);

        fs::write(&one, &whole).unwrap();
        let bundle = Bundle::of(&one, &dir).unwrap();
        assert!(bundle.complete() && !refused(&bundle, SetupKind::FirstRun));
        assert_eq!(kind(&bundle), SetupKind::OtherCopy { newer: true, can_run: false }, "the one file cannot run where it is");

        // Cut anywhere: inside the trailer, inside the index, inside a program, at the bar's end.
        for cut in [5, 40, whole.len() - 12, whole.len() - 10] {
            fs::write(&one, &whole[..whole.len() - cut]).unwrap();
            let bundle = Bundle::of(&one, &dir).unwrap();
            assert!(!bundle.packed && !bundle.complete(), "cut {cut}");
            assert!(refused(&bundle, SetupKind::FirstRun), "cut {cut}: a first run sets nothing up");
            assert!(refused(&bundle, SetupKind::OtherCopy { newer: true, can_run: true }), "cut {cut}: nor an update");
        }

        // A package listing only four of the five.
        fs::write(&one, package::pack(b"MZ the bar", &list[..4])).unwrap();
        assert!(refused(&Bundle::of(&one, &dir).unwrap(), SetupKind::FirstRun));

        // A copy as built: refused with any program missing, not with all five beside it.
        let bar = built.join("bevel95.exe");
        fs::write(&bar, b"MZ the bar").unwrap();
        for (i, (_, file)) in companions.iter().enumerate() {
            assert!(refused(&Bundle::of(&bar, &dir).unwrap(), SetupKind::FirstRun), "{i} of five beside it");
            fs::write(built.join(file), format!("MZ {file}")).unwrap();
        }
        let bundle = Bundle::of(&bar, &dir).unwrap();
        assert!(bundle.complete() && !refused(&bundle, SetupKind::FirstRun));
        assert_eq!(kind(&bundle), SetupKind::OtherCopy { newer: true, can_run: true });

        // The installed copy itself (its `--setup`, to remove it) is never refused.
        let lone = Bundle { bar: b"MZ".to_vec(), programs: Vec::new(), packed: false };
        assert!(refused(&lone, SetupKind::FirstRun) && !refused(&lone, SetupKind::Installed));
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn the_installed_apps_entry_names_bevel95() {
        let dir = Path::new("C:\\Users\\someone\\AppData\\Local\\Bevel95");
        let values = entry_values(dir, 15_000);
        let text = |name: &str| match values.iter().find(|(n, _)| *n == name) {
            Some((_, RegValue::Text(t))) => t.clone(),
            _ => panic!("{name}"),
        };
        assert_eq!(text("DisplayName"), NAME);
        assert_eq!(text("Publisher"), NAME, "PROVISIONAL: the name until the owner decides");
        assert_eq!(text("DisplayVersion"), VERSION);
        assert_eq!(text("UninstallString"), "\"C:\\Users\\someone\\AppData\\Local\\Bevel95\\bevel95.exe\" --uninstall");
        assert_eq!(text("DisplayIcon"), "C:\\Users\\someone\\AppData\\Local\\Bevel95\\bevel95.exe,0");
        assert!(values.iter().any(|(n, v)| *n == "EstimatedSize" && matches!(v, RegValue::Number(15_000))));
        assert_eq!(entry_key(), "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Bevel95");
    }

    #[test]
    fn a_stamped_copy_is_compared_by_its_build() {
        let stamped = |time: &str| package::stamp_text("0.1.0", "abc", time).into_bytes();
        let bundle = |bar: Vec<u8>| Bundle { bar, programs: Vec::new(), packed: true };
        let exe = std::env::current_exe().unwrap();
        // The commit's time decides, whatever the files' dates say.
        assert!(is_newer(&bundle(stamped("200")), &exe, &stamped("100"), &exe));
        assert!(!is_newer(&bundle(stamped("100")), &exe, &stamped("200"), &exe));
        // Without a stamp on one side, the dates (here the same file: not newer).
        assert!(!is_newer(&bundle(b"MZ".to_vec()), &exe, &stamped("100"), &exe));
    }

    #[test]
    fn same_file_ignores_case() {
        let exe = std::env::current_exe().unwrap();
        let upper = PathBuf::from(exe.display().to_string().to_uppercase());
        assert!(same_file(&exe, &upper));
        assert!(!same_file(&exe, &exe.with_file_name("surely-not-here.exe")));
    }
}
