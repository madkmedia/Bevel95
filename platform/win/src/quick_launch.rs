//! Quick Launch on the Windows side (docs/decisions.md, "Quick Launch"): the person's own
//! shortcuts, read from Windows' own Quick Launch folder on a thread of its own (never on the bar's)
//! and read again when the folder changes; the browser and mail program Windows has registered;
//! what a click on each icon starts; Windows' own Show Desktop; the built-in icons taken off the
//! band (`quick-launch.txt`). The band's layout, painting and rules are the core's
//! (`bevel_core::quick_launch`).

use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use std::sync::{Condvar, Mutex};
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
    /// The built-in icons taken off the band, as quick-launch.txt says now (a file deleted or
    /// changed by hand is what holds).
    pub removed: Vec<Builtin>,
}

static LATEST: Mutex<Option<Snapshot>> = Mutex::new(None);
static RUNNING: AtomicBool = AtomicBool::new(false);
static STOP: AtomicBool = AtomicBool::new(false);
/// Read again now (the band's Refresh, a shortcut deleted from its menu).
static AGAIN: AtomicBool = AtomicBool::new(false);
/// The bar, for a repaint when something new was read.
static BAR: AtomicUsize = AtomicUsize::new(0);
/// How many of the person's shortcuts show on the band (the bar says after each paint): only these
/// get their pictures read (none while the band is off or before the bar's first paint; a paint
/// that shows more wakes the thread at once).
static WANTED: AtomicUsize = AtomicUsize::new(0);
/// Wakes the reading thread from its wait (the band's Refresh, more pictures wanted, the bar's end).
static WAKE: (Mutex<()>, Condvar) = (Mutex::new(()), Condvar::new());

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
    wake(|| STOP.store(true, Ordering::SeqCst));
}

/// Reads the folder (and the programs) again at once.
pub fn read_again() {
    wake(|| AGAIN.store(true, Ordering::SeqCst));
}

/// `change` made under the wait's lock, then the reading thread woken (so it can never miss it
/// between looking and starting to wait).
fn wake(change: impl FnOnce()) {
    let guard = WAKE.0.lock();
    change();
    WAKE.1.notify_all();
    drop(guard);
}

/// The bar now shows `n` of the person's shortcuts; more than before have their pictures read.
fn want_pictures(n: usize) {
    if WANTED.swap(n, Ordering::SeqCst) < n {
        wake(|| {});
    }
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
    let mut shortcuts: Vec<Shortcut> = Vec::new();
    // How many of `shortcuts` (from the first) have their pictures read.
    let mut pictured = 0;
    // The browser and the mail program: asked here only, never on the bar's thread (a program on
    // an unreachable network place must not hold up the bar's start: the review before 0.1.2, B5).
    let mut programs: (Option<String>, Option<String>) = (None, None);
    // What the bar was last given of them (None: nothing yet), so a change is passed on whether or
    // not the folder changed (B6).
    let mut published_programs: Option<(Option<String>, Option<String>)> = None;
    // quick-launch.txt as last looked at (its size and time; None: not there) and what it said.
    let mut removed_seen: Option<Option<(u64, u64)>> = None;
    let mut removed: Vec<Builtin> = Vec::new();
    let mut looks: u32 = 0;
    let mut logged_count: Option<usize> = None;
    while !STOP.load(Ordering::SeqCst) {
        let again = AGAIN.swap(false, Ordering::SeqCst);
        // The browser and the mail program are asked for after the folder is read and given to
        // the bar (the second review before 0.1.2, C5): a program on an unreachable network place
        // then delays only their own icons, never the person's shortcuts.
        let ask_programs = again || looks.is_multiple_of(PROGRAMS_EVERY);
        looks = looks.wrapping_add(1);
        // The built-in icons taken off: read again whenever the file changed (also by hand, or
        // deleted), so that the band and the file never disagree.
        let stamp = removed_stamp();
        let removed_changed = again || removed_seen != Some(stamp);
        if removed_changed {
            removed = load_removed();
            removed_seen = Some(stamp);
        }
        let dir = folder();
        let seen = dir.as_deref().and_then(list).unwrap_or_default();
        let folder_changed = again || last.as_ref() != Some(&seen);
        let started = Instant::now();
        if folder_changed {
            let entries: Vec<FolderEntry> = seen.iter().map(|(e, _)| e.clone()).collect();
            shortcuts = match &dir {
                Some(dir) => quick_launch::shown_files(&entries)
                    .into_iter()
                    .map(|name| Shortcut { path: dir.join(&name), name, icon: None })
                    .collect(),
                None => Vec::new(),
            };
            pictured = 0;
            if logged_count != Some(shortcuts.len()) {
                logged_count = Some(shortcuts.len());
                let place = if dir.is_some() { "" } else { " (Windows names no Quick Launch folder)" };
                log(&format!("quick launch: {} shortcuts of the person's{place}", shortcuts.len()));
            }
            last = Some(seen);
        }
        // Pictures only for the shortcuts that show (the review before 0.1.2, C3): never for a
        // folder's thousands of files.
        let wanted = WANTED.load(Ordering::SeqCst).min(shortcuts.len());
        let pictures_read = wanted > pictured;
        if pictures_read {
            let era = crate::start_menu::era_now();
            for s in &mut shortcuts[pictured..wanted] {
                s.icon = crate::start_menu::file_icon(era, &s.path.to_string_lossy());
            }
            pictured = wanted;
        }
        let took = started.elapsed().as_millis();
        if took > 500 {
            log(&format!("quick launch: reading its folder took {took} ms"));
        }
        let publish = |programs: &(Option<String>, Option<String>)| {
            if let Ok(mut latest) = LATEST.lock() {
                *latest = Some(Snapshot {
                    shortcuts: shortcuts.clone(),
                    browser: programs.0.clone(),
                    mail: programs.1.clone(),
                    removed: removed.clone(),
                });
            }
            repaint();
        };
        if folder_changed || removed_changed || pictures_read {
            publish(&programs);
            published_programs = Some(programs.clone());
        }
        if ask_programs {
            programs = (registered("http"), registered("mailto"));
        }
        if published_programs.as_ref() != Some(&programs) {
            publish(&programs);
            published_programs = Some(programs.clone());
        }
        // Waits for the next look (EVERY), or until woken: Refresh, more pictures, the end.
        if let Ok(guard) = WAKE.0.lock() {
            let keep_waiting = |_: &mut ()| {
                !STOP.load(Ordering::SeqCst) && !AGAIN.load(Ordering::SeqCst) && WANTED.load(Ordering::SeqCst).min(shortcuts.len()) <= pictured
            };
            drop(WAKE.1.wait_timeout_while(guard, EVERY, keep_waiting));
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
    /// How many icons the bar had room for at the last paint, as the log said last.
    logged_narrow: Option<usize>,
}

impl State {
    /// The state at the bar's start: the setting and the icons taken off. The browser and the mail
    /// program come from the reading thread's first look, as the folder does (never asked on the
    /// bar's thread, where a program on an unreachable network place would hold up the bar's
    /// start: the review before 0.1.2, B5), so their icons join the band a moment after it shows.
    pub fn new(setting: Option<bool>) -> State {
        State { setting, removed: load_removed(), ..State::default() }
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
            self.removed = s.removed;
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

    /// After a paint: the bar had room for `shown` of the band's icons (`quick_launch::fitting`:
    /// the task area keeps room for one whole task button and its arrows); the others are left
    /// out, and the log says how many, once per change.
    /// `None`: the band is off (no picture is wanted).
    pub fn shown(&mut self, shown: Option<usize>) {
        let Some(shown) = shown else {
            want_pictures(0);
            return;
        };
        // Only the shortcuts shown get their pictures read.
        want_pictures(self.items.iter().take(shown).filter(|i| matches!(i, Item::Shortcut(_))).count());
        let narrow = self.items.len().saturating_sub(shown);
        if self.logged_narrow != Some(narrow) {
            self.logged_narrow = Some(narrow);
            if narrow > 0 {
                log(&format!("quick launch: the bar has room for {shown} icons; {narrow} more are left out"));
            }
        }
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

/// quick-launch.txt's size and time of change (None: it is not there, or cannot be looked at).
fn removed_stamp() -> Option<(u64, u64)> {
    use std::os::windows::fs::MetadataExt;
    std::fs::metadata(state_path()).ok().map(|m| (m.len(), m.last_write_time()))
}

/// Takes built-in icon `builtin` off the band (Yes to its question): quick-launch.txt as it is now
/// (also when it was changed or deleted by hand), with `builtin` added. Returns the new list.
pub fn take_off(builtin: Builtin) -> Vec<Builtin> {
    let mut removed = load_removed();
    if !removed.contains(&builtin) {
        removed.push(builtin);
    }
    if save_removed(&removed) {
        log(&format!("quick launch: {} taken off the band", builtin.key()));
    } else {
        log(&format!(
            "quick launch: {} taken off the band for now only: quick-launch.txt was not saved, so it comes back at the next reading",
            builtin.key()
        ));
    }
    read_again();
    removed
}

/// Quick Launch switched on again: every built-in icon comes back (the way back after Delete).
/// Only writes quick-launch.txt when it names an icon taken off.
pub fn put_back_all() {
    if !load_removed().is_empty() {
        save_removed(&[]);
        log("quick launch: switched on again; every built-in icon is back on the band");
    }
    read_again();
}

/// Remembers the built-in icons taken off the band. False when the save was refused or failed
/// (the reason is logged: here, or already by `state_file`).
pub fn save_removed(removed: &[Builtin]) -> bool {
    let path = state_path();
    match state_file::save(&path, &quick_launch::removed_text(removed)) {
        Ok(()) => true,
        Err(e) => {
            if !state_file::already_logged(&e) {
                log(&format!("could not save the Quick Launch icons taken off to {}: {e}", crate::log::path_kind(&path)));
            }
            false
        }
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
/// window, then the Recycle Bin, the same code). At Yes the file is checked once more (the
/// question may have waited; the review before 0.1.2, C1): when its identity was read at the menu,
/// only the very same file goes to the Recycle Bin; one that went or was replaced meanwhile is left
/// alone and nothing more is said (the log says which). When its identity cannot be read at the
/// menu but the file is there (the second review before 0.1.2, B1: its rights deny even that, the
/// file system gives no file information), the question is asked as the desktop's Delete asks, and
/// Yes deletes the file if it is still there. Only a file that is really gone is not asked about.
pub fn delete_shortcut(path: &Path, by_keyboard: bool) {
    let name = path.file_name().map(|n| quick_launch::shortcut_tooltip(&n.to_string_lossy()).to_string()).unwrap_or_default();
    let asked = file_identity(path);
    match asked {
        Identity::Gone => {
            log("quick launch: Delete: the shortcut is no longer there; nothing was asked");
            read_again();
            return;
        }
        Identity::Unread(code) => log(&format!(
            "quick launch: Delete: the shortcut is there but what file it is could not be read (error {}); asked without that check",
            code.map_or_else(|| "none".to_string(), |c| c.to_string())
        )),
        Identity::Known(_) => {}
    }
    let target = crate::file_ops::Target { id: path.to_string_lossy().into_owned(), name: name.clone(), kind: bevel_core::desktop::ItemKind::File };
    let path = path.to_path_buf();
    let yes = move || {
        match at_yes(asked, file_identity(&path)) {
            AtYes::Delete => crate::file_ops::delete(vec![target]),
            AtYes::Gone => log("quick launch: Delete: the shortcut was gone when Yes was chosen; nothing was deleted"),
            AtYes::Replaced => {
                log("quick launch: Delete: another file had taken the shortcut's place while the question waited; nothing was deleted")
            }
            AtYes::Unchecked => log(
                "quick launch: Delete: at Yes it could no longer be read what file the shortcut is, so it could not be shown to be the one asked about; nothing was deleted",
            ),
        }
        read_again();
    };
    crate::confirm_window::ask_then(bevel_core::delete_confirm::Question::File(name), by_keyboard, Box::new(yes));
}

/// What a file in the shortcut's name is, as far as can be read (`file_identity`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Identity {
    /// What makes it this very file: its volume and file index (another file put in its name has
    /// another index), its size and time of change.
    Known((u32, u64, u64, u64)),
    /// No file in that name (also: a folder there now).
    Gone,
    /// Something is there, but what file it is cannot be read (the error code, if Windows gave one).
    Unread(Option<i32>),
}

/// What Yes does, from what the file was at the menu and what it is now.
#[derive(Debug, PartialEq, Eq)]
enum AtYes {
    Delete,
    Gone,
    Replaced,
    /// Known at the menu, unreadable now: it cannot be shown to be the same file.
    Unchecked,
}

fn at_yes(asked: Identity, now: Identity) -> AtYes {
    match (asked, now) {
        (_, Identity::Gone) => AtYes::Gone,
        (Identity::Known(a), Identity::Known(b)) if a == b => AtYes::Delete,
        (Identity::Known(_), Identity::Known(_)) => AtYes::Replaced,
        (Identity::Known(_), Identity::Unread(_)) => AtYes::Unchecked,
        // Not known at the menu (asked as the desktop asks): deleted if it is still there.
        (Identity::Unread(_) | Identity::Gone, _) => AtYes::Delete,
    }
}

/// What file is in `path` now. Read without asking for the file's contents (no data access, the
/// file's information only, which Windows gives also when its rights deny reading it or another
/// program holds it without sharing) and without following a link: a link to a file is a file
/// here, as it was deleted like any file before the check existed.
fn file_identity(path: &Path) -> Identity {
    use std::os::windows::fs::{FileTypeExt, OpenOptionsExt};
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, GetFileInformationByHandle,
    };
    let meta = match path.symlink_metadata() {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Identity::Gone,
        Err(e) => return Identity::Unread(e.raw_os_error()),
    };
    let kind = meta.file_type();
    if !(kind.is_file() || kind.is_symlink_file()) {
        return Identity::Gone;
    }
    // No data access (0): only the file's information; every sharing is allowed (the default).
    let file = match std::fs::OpenOptions::new()
        .access_mode(0)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
    {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Identity::Gone,
        Err(e) => return Identity::Unread(e.raw_os_error()),
    };
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: an open file's handle and a correctly sized out-struct.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle() as _, &mut info) } == 0 {
        return Identity::Unread(std::io::Error::last_os_error().raw_os_error());
    }
    let index = (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow);
    let written = (u64::from(info.ftLastWriteTime.dwHighDateTime) << 32) | u64::from(info.ftLastWriteTime.dwLowDateTime);
    let size = (u64::from(info.nFileSizeHigh) << 32) | u64::from(info.nFileSizeLow);
    Identity::Known((info.dwVolumeSerialNumber, index, size, written))
}

/// Windows' own Properties window for the person's shortcut `path` (the desktop's Properties).
pub fn shortcut_properties(path: &Path) {
    let name = path.file_name().map(|n| quick_launch::shortcut_tooltip(&n.to_string_lossy()).to_string()).unwrap_or_default();
    let target = crate::file_ops::Target { id: path.to_string_lossy().into_owned(), name, kind: bevel_core::desktop::ItemKind::File };
    crate::file_ops::properties(&[target]);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch folder of this test's own (never the person's Quick Launch folder).
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("bevel95-ql-identity-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn known(path: &Path) -> (u32, u64, u64, u64) {
        match file_identity(path) {
            Identity::Known(id) => id,
            other => panic!("expected the file's identity, got {other:?}"),
        }
    }

    /// The second review before 0.1.2, B1: a plain file, one held open without sharing, one whose
    /// rights deny reading its contents, one replaced, one removed, a folder in its name.
    #[test]
    fn a_shortcuts_identity_is_read_without_its_contents() {
        let dir = scratch("cases");
        let file = dir.join("Plain.lnk");
        std::fs::write(&file, b"shortcut").unwrap();
        let first = known(&file);
        assert_eq!(at_yes(Identity::Known(first), file_identity(&file)), AtYes::Delete, "an unchanged file is deleted");

        // Held open by another program that shares nothing: still read, still the same file.
        {
            use std::os::windows::fs::OpenOptionsExt;
            let _held = std::fs::OpenOptions::new().read(true).write(true).share_mode(0).open(&file).unwrap();
            assert!(std::fs::File::open(&file).is_err(), "the file is really held without sharing");
            assert_eq!(file_identity(&file), Identity::Known(first), "a held file is read and is the same");
        }

        // Its rights deny reading its contents (icacls, in the scratch folder only).
        let deny = |args: &[&str]| {
            bevel_winkit::quiet::command(bevel_winkit::shell::system_program("icacls.exe"))
                .arg(&file)
                .args(args)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        };
        let everyone = "*S-1-1-0:(RD)";
        if deny(&["/deny", everyone]) {
            let unreadable = std::fs::read(&file).is_err();
            let id = file_identity(&file);
            assert!(deny(&["/remove:d", "*S-1-1-0"]), "the scratch file's rights are put back");
            assert!(unreadable, "the denial took hold");
            assert_eq!(id, Identity::Known(first), "a file whose contents cannot be read is still known");
        } else {
            eprintln!("icacls could not deny reading: that case was not tried");
        }

        // Replaced by another file in the same name, with the same contents.
        std::fs::remove_file(&file).unwrap();
        std::fs::write(&file, b"shortcut").unwrap();
        let second = known(&file);
        assert_ne!(second, first, "another file in the same name is another file");
        assert_eq!(at_yes(Identity::Known(first), Identity::Known(second)), AtYes::Replaced);

        // Removed.
        std::fs::remove_file(&file).unwrap();
        assert_eq!(file_identity(&file), Identity::Gone);
        assert_eq!(at_yes(Identity::Known(second), file_identity(&file)), AtYes::Gone);

        // A folder in its name is no shortcut.
        std::fs::create_dir(&file).unwrap();
        assert_eq!(file_identity(&file), Identity::Gone);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A link to a file is a file here (it was deleted like any file before the check existed): its
    /// own identity, not its target's.
    #[test]
    fn a_link_to_a_file_is_a_file() {
        let dir = scratch("link");
        let target = dir.join("Target.exe");
        std::fs::write(&target, b"target").unwrap();
        let link = dir.join("Link.lnk");
        if std::os::windows::fs::symlink_file(&target, &link).is_err() {
            eprintln!("this account may not make symbolic links: the link case was not tried");
            let _ = std::fs::remove_dir_all(&dir);
            return;
        }
        let id = known(&link);
        assert_ne!(id, known(&target), "the link itself, not the file it points to");
        std::fs::remove_file(&target).unwrap();
        assert_eq!(file_identity(&link), Identity::Known(id), "a link whose target went is still there");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// What Yes does in every case.
    #[test]
    fn yes_deletes_only_what_it_can_show() {
        let a = Identity::Known((1, 2, 3, 4));
        let b = Identity::Known((1, 9, 3, 4));
        let unread = Identity::Unread(Some(5));
        assert_eq!(at_yes(a, a), AtYes::Delete);
        assert_eq!(at_yes(a, b), AtYes::Replaced);
        assert_eq!(at_yes(a, Identity::Gone), AtYes::Gone);
        assert_eq!(at_yes(a, unread), AtYes::Unchecked);
        // Not known at the menu: asked as the desktop asks; deleted if it is still there.
        assert_eq!(at_yes(unread, unread), AtYes::Delete);
        assert_eq!(at_yes(unread, a), AtYes::Delete);
        assert_eq!(at_yes(unread, Identity::Gone), AtYes::Gone);
    }
}