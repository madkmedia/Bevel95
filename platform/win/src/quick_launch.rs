//! Quick Launch on the Windows side (docs/decisions.md, "Quick Launch"): the person's own
//! shortcuts, read from Windows' own Quick Launch folder on a thread of its own (never on the bar's)
//! and read again when the folder changes; the browser and mail program Windows has registered;
//! what a click on each icon starts; Windows' own Show Desktop; the built-in icons taken off the
//! band (`quick-launch.txt`). The band's layout, painting and rules are the core's
//! (`bevel_core::quick_launch`).

use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use bevel_core::fb::Icon;
use bevel_core::quick_launch::{self, Band, Builtin, FolderEntry, Held, Item};
use bevel_core::theme::Era;
use bevel_winkit::state_file;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Gdi::InvalidateRect;
use windows_sys::Win32::System::Registry::{HKEY, HKEY_CLASSES_ROOT, HKEY_CURRENT_USER, RRF_RT_REG_SZ, RegGetValueW};
use windows_sys::Win32::UI::Shell::{
    ASSOCF_INIT_IGNOREUNKNOWN, ASSOCF_IS_PROTOCOL, ASSOCF_NOTRUNCATE, ASSOCSTR_EXECUTABLE, AssocQueryStringW,
    FOLDERID_QuickLaunch,
};

use crate::log::{data_dir, log};
use crate::util::wide;

/// One of the person's shortcuts in the Quick Launch folder, as read.
#[derive(Clone, Debug)]
pub struct Shortcut {
    pub path: PathBuf,
    /// Its file name (the tooltip shows it without the extension).
    pub name: String,
    pub icon: Option<Icon>,
}

/// What the reading thread found last.
pub struct Snapshot {
    pub shortcuts: Vec<Shortcut>,
    pub browser: Option<String>,
    pub mail: Option<String>,
}

static LATEST: Mutex<Option<Snapshot>> = Mutex::new(None);
static RUNNING: AtomicBool = AtomicBool::new(false);
static STOP: AtomicBool = AtomicBool::new(false);
/// Read again now (the band's Refresh, a shortcut deleted from its menu).
static AGAIN: AtomicBool = AtomicBool::new(false);
/// The bar, for a repaint when something new was read.
static BAR: AtomicUsize = AtomicUsize::new(0);

/// How often the folder is looked at (a listing of a few files: cheap), and every how many looks
/// the browser and mail program are asked for again.
const EVERY: Duration = Duration::from_millis(2000);
const PROGRAMS_EVERY: u32 = 15;

/// Takes what was read last, if it is new since the last call.
pub fn take() -> Option<Snapshot> {
    LATEST.lock().ok()?.take()
}

/// Starts reading the folder (once, for the life of the bar).
pub fn start(bar: HWND) {
    BAR.store(bar as usize, Ordering::SeqCst);
    STOP.store(false, Ordering::SeqCst);
    AGAIN.store(true, Ordering::SeqCst);
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    if let Err(e) = std::thread::Builder::new().name("quick launch".into()).spawn(run) {
        RUNNING.store(false, Ordering::SeqCst);
        log(&format!("quick launch: could not start reading its folder: {e}"));
    }
}

pub fn stop() {
    STOP.store(true, Ordering::SeqCst);
}

/// Reads the folder (and the programs) again at once.
pub fn read_again() {
    AGAIN.store(true, Ordering::SeqCst);
}

/// Windows' own Quick Launch folder (the known folder "Quick Launch"; still there in today's
/// Windows, which keeps two shortcuts of its own taskbar in it).
pub fn folder() -> Option<PathBuf> {
    crate::start_menu::known_folder(&FOLDERID_QuickLaunch)
}

/// One look at the folder: each entry and when it last changed (`None`: no folder, or it cannot
/// be read now).
fn list(dir: &Path) -> Option<Vec<(FolderEntry, u64)>> {
    use std::os::windows::fs::MetadataExt;
    const HIDDEN_OR_SYSTEM: u32 = 0x2 | 0x4;
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        let name = entry.file_name().to_string_lossy().into_owned();
        let hidden = meta.file_attributes() & HIDDEN_OR_SYSTEM != 0;
        out.push((FolderEntry { name, hidden, folder: meta.is_dir() }, meta.last_write_time()));
    }
    out.sort_by(|a, b| a.0.name.cmp(&b.0.name));
    Some(out)
}

fn run() {
    // COM for this thread (the shell's icon list needs it).
    bevel_winkit::com::init();
    let mut last: Option<Vec<(FolderEntry, u64)>> = None;
    let mut programs: (Option<String>, Option<String>) = (None, None);
    let mut looks: u32 = 0;
    let mut logged_count: Option<usize> = None;
    while !STOP.load(Ordering::SeqCst) {
        let again = AGAIN.swap(false, Ordering::SeqCst);
        if again || looks.is_multiple_of(PROGRAMS_EVERY) {
            programs = (registered("http"), registered("mailto"));
        }
        looks = looks.wrapping_add(1);
        let dir = folder();
        let seen = dir.as_deref().and_then(list).unwrap_or_default();
        if again || last.as_ref() != Some(&seen) {
            let started = Instant::now();
            let entries: Vec<FolderEntry> = seen.iter().map(|(e, _)| e.clone()).collect();
            let era = crate::start_menu::era_now();
            let shortcuts: Vec<Shortcut> = match &dir {
                Some(dir) => quick_launch::shown_files(&entries)
                    .into_iter()
                    .map(|name| {
                        let path = dir.join(&name);
                        let icon = crate::start_menu::file_icon(era, &path.to_string_lossy());
                        Shortcut { path, name, icon }
                    })
                    .collect(),
                None => Vec::new(),
            };
            let took = started.elapsed().as_millis();
            if took > 500 {
                log(&format!("quick launch: reading its folder took {took} ms"));
            }
            if logged_count != Some(shortcuts.len()) {
                logged_count = Some(shortcuts.len());
                let place = if dir.is_some() { "" } else { " (Windows names no Quick Launch folder)" };
                log(&format!("quick launch: {} shortcuts of the person's{place}", shortcuts.len()));
            }
            if let Ok(mut latest) = LATEST.lock() {
                *latest = Some(Snapshot { shortcuts, browser: programs.0.clone(), mail: programs.1.clone() });
            }
            repaint();
            last = Some(seen);
        }
        let wait = Instant::now();
        while wait.elapsed() < EVERY && !STOP.load(Ordering::SeqCst) && !AGAIN.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    RUNNING.store(false, Ordering::SeqCst);
}

fn repaint() {
    let bar = BAR.load(Ordering::SeqCst) as HWND;
    if !bar.is_null() {
        // SAFETY: InvalidateRect may be called from any thread; the bar may be gone (then it fails).
        unsafe { InvalidateRect(bar, null(), 0) };
    }
}

/// What starts the program Windows has registered for `protocol` ("http": the web browser,
/// "mailto": the mail program) by itself, with no address: its whole path, when it is a program
/// file of its own (not one of Windows' helpers such as "Open with", which it names when the
/// program registered is gone); else a Store-style app's ID (as the Start menu starts those);
/// `None` when Windows knows none.
pub fn registered(protocol: &str) -> Option<String> {
    let exe = assoc_executable(protocol).filter(|p| starts_by_itself(Path::new(p)));
    exe.or_else(|| store_app(protocol))
}

fn assoc_executable(protocol: &str) -> Option<String> {
    let (assoc, verb) = (wide(protocol), wide("open"));
    let mut buf = vec![0u16; 1024];
    let mut len = buf.len() as u32;
    // SAFETY: null-terminated strings, a buffer of `len` characters.
    let hr = unsafe {
        AssocQueryStringW(
            ASSOCF_IS_PROTOCOL | ASSOCF_NOTRUNCATE | ASSOCF_INIT_IGNOREUNKNOWN,
            ASSOCSTR_EXECUTABLE,
            assoc.as_ptr(),
            verb.as_ptr(),
            buf.as_mut_ptr(),
            &mut len,
        )
    };
    if hr != 0 {
        return None;
    }
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..end]))
}

/// A program file outside Windows' own folder (its helpers there start no browser or mail
/// program by themselves: "Open with", rundll32, explorer).
fn starts_by_itself(path: &Path) -> bool {
    let windows = std::env::var_os("SystemRoot").map(PathBuf::from);
    path.is_file()
        && path.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe"))
        && windows.is_none_or(|w| !path.to_string_lossy().to_lowercase().starts_with(&w.to_string_lossy().to_lowercase()))
}

/// The Store-style app the person chose for `protocol`, if its registration names one: Windows'
/// UserChoice, then that program's `Application\AppUserModelID`.
fn store_app(protocol: &str) -> Option<String> {
    let choice = format!("Software\\Microsoft\\Windows\\Shell\\Associations\\UrlAssociations\\{protocol}\\UserChoice");
    let prog_id = reg_string(HKEY_CURRENT_USER, &choice, "ProgId")?;
    let id = reg_string(HKEY_CLASSES_ROOT, &format!("{prog_id}\\Application"), "AppUserModelID")?;
    // An app's ID: letters, digits and . _ - !, with the "!" between the package and the app.
    let plain = !id.is_empty() && id.contains('!') && id.chars().all(|c| c.is_ascii_alphanumeric() || ".-_!".contains(c));
    plain.then(|| format!("{}{id}", bevel_core::programs::APP_PREFIX))
}

fn reg_string(root: HKEY, key: &str, value: &str) -> Option<String> {
    let (key, value) = (wide(key), wide(value));
    let mut buf = vec![0u16; 512];
    let mut bytes = (buf.len() * 2) as u32;
    // SAFETY: null-terminated names, a buffer of `bytes` bytes; only reads.
    let status = unsafe {
        RegGetValueW(root, key.as_ptr(), value.as_ptr(), RRF_RT_REG_SZ, null_mut(), buf.as_mut_ptr() as *mut c_void, &mut bytes)
    };
    if status != 0 {
        return None;
    }
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..end]))
}

/// Explorer's taskbar command for Show Desktop (what Win+D and its own corner button do: every
/// window away, and back at the next one).
const TRAY_TOGGLE_DESKTOP: usize = 407;

/// Show Desktop: Windows' own, as Win+D (which Bevel95's classic desktop follows: "Win+D and the
/// classic desktop"), in one way only: the command posted once to Explorer's own taskbar window
/// (`explorer::explorer_tray`: never Bevel95's stand-in, never another program's window of that
/// class). Posting never waits, so a hung Explorer cannot hold up the bar. (Until the review
/// before 0.1.2, B1, Shell.Application's ToggleDesktop came first: it reached Bevel95's stand-in,
/// which forwarded it to Explorer, and its S_FALSE then had the command posted a second time, so
/// the two toggles undid each other.)
pub fn show_desktop() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_COMMAND};
    let tray = crate::explorer::explorer_tray();
    if tray.is_null() {
        log("quick launch: Show Desktop: Explorer's own taskbar window is not there; nothing was done");
        return;
    }
    // SAFETY: a window handle just found; PostMessageW only queues the message.
    let posted = unsafe { PostMessageW(tray, WM_COMMAND, TRAY_TOGGLE_DESKTOP, 0) } != 0;
    static LOGGED: AtomicBool = AtomicBool::new(false);
    if !posted {
        log(&format!("quick launch: Show Desktop: the command could not be posted to Explorer's taskbar window (error {})", std::io::Error::last_os_error()));
    } else if !LOGGED.swap(true, Ordering::SeqCst) {
        log("quick launch: Show Desktop: posted to Explorer's own taskbar window (logged once)");
    }
}

/// What a menu or a click on an icon acts on: the icon by what it is (a built-in one, or the
/// person's shortcut by its file), never by its place on the band ("Remembered by what it is").
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Builtin(Builtin),
    File(PathBuf),
}

/// The bar's Quick Launch state (`App::ql`).
#[derive(Debug, Default)]
pub struct State {
    /// settings.txt's `quick_launch` (`None`: the look decides).
    pub setting: Option<bool>,
    /// The built-in icons taken off the band (quick-launch.txt).
    pub removed: Vec<Builtin>,
    pub shortcuts: Vec<Shortcut>,
    pub browser: Option<String>,
    pub mail: Option<String>,
    /// The icons shown at the last paint, the one under the pointer, and the one held down by what
    /// it is (`quick_launch::Held`: only its release ends it, whatever happens to the band).
    pub items: Vec<Item>,
    pub hot: Option<usize>,
    pub held: Option<Held<Target>>,
    /// What the log said last: how many icons were left out, whether there was a mail program.
    logged_left_out: Option<usize>,
    logged_programs: Option<(bool, bool)>,
}

impl State {
    /// The state at the bar's start: the setting, the icons taken off, and the browser and mail
    /// program Windows has registered (asked at once, so the band does not grow a moment later;
    /// the folder comes from the reading thread).
    pub fn new(setting: Option<bool>) -> State {
        State { setting, removed: load_removed(), browser: registered("http"), mail: registered("mailto"), ..State::default() }
    }

    pub fn on(&self, era: Era) -> bool {
        self.setting.unwrap_or_else(|| quick_launch::on_by_default(era))
    }

    /// Takes in what the reading thread found (if anything new), and works out the icons shown.
    pub fn refresh(&mut self, era: Era) {
        if let Some(s) = take() {
            self.shortcuts = s.shortcuts;
            self.browser = s.browser;
            self.mail = s.mail;
            let programs = (self.browser.is_some(), self.mail.is_some());
            if self.logged_programs != Some(programs) {
                self.logged_programs = Some(programs);
                log(&format!(
                    "quick launch: {}; {}",
                    if programs.0 { "a web browser Windows can start" } else { "no web browser Windows can start by itself: Internet Explorer's icon is left out" },
                    if programs.1 { "a mail program Windows can start" } else { "no mail program Windows can start by itself: Outlook Express's icon is left out" },
                ));
            }
        }
        let (browser, mail) = (self.browser.is_some(), self.mail.is_some());
        let available = |b: Builtin| match b {
            Builtin::InternetExplorer => browser,
            Builtin::OutlookExpress => mail,
            _ => true,
        };
        let (items, left_out) = quick_launch::items(era, &self.removed, available, self.shortcuts.len());
        if self.logged_left_out != Some(left_out) {
            self.logged_left_out = Some(left_out);
            if left_out > 0 {
                log(&format!("quick launch: {left_out} icons do not fit and are left out (at most {})", quick_launch::MAX_ICONS));
            }
        }
        // A held icon is left to its release (`Held`): it decides by what the icon is.
        if items != self.items {
            self.hot = None;
        }
        self.items = items;
    }

    /// What the band shows in `era`.
    pub fn band(&self, era: Era) -> Band {
        let icons = self
            .items
            .iter()
            .map(|item| match *item {
                Item::Builtin(b) => quick_launch::picture(era, b),
                Item::Shortcut(i) => self.shortcuts.get(i).and_then(|s| s.icon.clone()),
            })
            .collect();
        let pressed = self.held.as_ref().and_then(|h| h.sunk(|i| self.target(i)));
        Band { icons, hot: self.hot, pressed }
    }

    /// What icon `index` is.
    pub fn target(&self, index: usize) -> Option<Target> {
        match *self.items.get(index)? {
            Item::Builtin(b) => Some(Target::Builtin(b)),
            Item::Shortcut(i) => self.shortcuts.get(i).map(|s| Target::File(s.path.clone())),
        }
    }

    /// Whether `target` is still on the band (a menu that opened on it acts only then).
    pub fn holds(&self, target: &Target) -> bool {
        (0..self.items.len()).any(|i| self.target(i).as_ref() == Some(target))
    }

    /// Icon `index`'s tooltip in `era`.
    pub fn tooltip(&self, era: Era, index: usize) -> Option<String> {
        match *self.items.get(index)? {
            Item::Builtin(b) => Some(b.tooltip(era).to_string()),
            Item::Shortcut(i) => self.shortcuts.get(i).map(|s| quick_launch::shortcut_tooltip(&s.name).to_string()),
        }
    }
}

fn state_path() -> PathBuf {
    data_dir().join("quick-launch.txt")
}

/// The built-in icons taken off the band, from quick-launch.txt (none while it is missing; a file
/// that cannot be read is kept as it is, and saves are skipped: `state_file::load`).
pub fn load_removed() -> Vec<Builtin> {
    match state_file::load(&state_path()) {
        state_file::Read::Text(text) => quick_launch::parse_removed(&text),
        _ => Vec::new(),
    }
}

/// Remembers the built-in icons taken off the band.
pub fn save_removed(removed: &[Builtin]) {
    let path = state_path();
    if let Err(e) = state_file::save(&path, &quick_launch::removed_text(removed))
        && !state_file::already_logged(&e)
    {
        log(&format!("could not save the Quick Launch icons taken off to {}: {e}", crate::log::path_kind(&path)));
    }
}

/// Starts what icon `target` stands for (one click, as Quick Launch always was): the browser or the
/// mail program by itself, Show Desktop, Bevel95's Windows Explorer as the Programs menu starts it,
/// the person's shortcut as a Programs menu entry is opened.
pub fn start_item(target: &Target, browser: Option<&str>, mail: Option<&str>) {
    match target {
        Target::Builtin(Builtin::ShowDesktop) => show_desktop(),
        Target::Builtin(Builtin::InternetExplorer) => match browser {
            Some(program) => crate::window_list::launch(program),
            None => log("quick launch: no web browser to start"),
        },
        Target::Builtin(Builtin::OutlookExpress) => match mail {
            Some(program) => crate::window_list::launch(program),
            None => log("quick launch: no mail program to start"),
        },
        Target::Builtin(Builtin::WindowsExplorer) => match crate::start_menu::own_program_target("folders.exe") {
            Some(program) => crate::window_list::launch(&program),
            None => log("quick launch: the folder windows' program is missing"),
        },
        Target::File(path) => crate::window_list::launch(&path.to_string_lossy()),
    }
}

/// Opens Windows' Quick Launch folder in Bevel95's folder window (the band's Open).
pub fn open_folder() {
    match folder() {
        Some(dir) => crate::desktop::open_item(dir.to_string_lossy().into_owned()),
        None => log("quick launch: Windows names no Quick Launch folder to open"),
    }
}

/// Delete on the person's shortcut `path`: the desktop icons' own Delete (the look's question
/// window, then the Recycle Bin), with the same code.
pub fn delete_shortcut(path: &Path, by_keyboard: bool) {
    let name = path.file_name().map(|n| quick_launch::shortcut_tooltip(&n.to_string_lossy()).to_string()).unwrap_or_default();
    let target = crate::file_ops::Target { id: path.to_string_lossy().into_owned(), name, kind: bevel_core::desktop::ItemKind::File };
    crate::confirm_window::ask_delete(vec![target], by_keyboard);
    read_again();
}

/// Windows' own Properties window for the person's shortcut `path` (the desktop's Properties).
pub fn shortcut_properties(path: &Path) {
    let name = path.file_name().map(|n| quick_launch::shortcut_tooltip(&n.to_string_lossy()).to_string()).unwrap_or_default();
    let target = crate::file_ops::Target { id: path.to_string_lossy().into_owned(), name, kind: bevel_core::desktop::ItemKind::File };
    crate::file_ops::properties(&[target]);
}