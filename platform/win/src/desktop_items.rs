//! What the real desktop holds, for the classic desktop's icons: the owner's Desktop folder and
//! the public one (Explorer shows both), and whether the Recycle Bin has anything in it. Read on
//! a thread of its own ("desktop items") once a second; when anything changed (a file came,
//! went, was renamed or changed, the Recycle Bin filled or emptied) the new list goes to the
//! desktop window (`crate::desktop::items_ready`). Pictures are read from the shell only for
//! items that are new or changed. Nothing is ever written: this module only reads. How one file
//! becomes an icon is shared with the folder windows (`bevel_winkit::shell`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::ptr::null_mut;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use bevel_core::desktop_icons::Item;
use bevel_winkit::shell::{self, Seen};
use windows_sys::Win32::UI::Shell::{FOLDERID_Desktop, FOLDERID_PublicDesktop, SHQUERYRBINFO, SHQueryRecycleBinW};

use crate::log::log;

/// What was read last: the owner's items and whether the Recycle Bin is full.
pub struct Snapshot {
    pub files: Vec<Item>,
    pub recycle_full: bool,
    /// The hidden items among them (by id: Windows shows hidden items): drawn faded.
    pub hidden: Vec<String>,
}

static LATEST: Mutex<Option<Snapshot>> = Mutex::new(None);
static RUNNING: AtomicBool = AtomicBool::new(false);
static STOP: AtomicBool = AtomicBool::new(false);
/// Read again now (Refresh, or a new item made by the desktop's menu).
static AGAIN: AtomicBool = AtomicBool::new(false);

/// How often the folders are looked at.
const EVERY: Duration = Duration::from_millis(1000);

/// Takes what was read last, if it is new since the last call.
pub fn take() -> Option<Snapshot> {
    LATEST.lock().ok()?.take()
}

/// Starts reading (once), for as long as the desktop shows.
pub fn start() {
    // The desktop's file commands log as "desktop" and have the folders read again at once.
    bevel_winkit::file_ops::set_hooks("desktop", read_again);
    STOP.store(false, Ordering::SeqCst);
    AGAIN.store(true, Ordering::SeqCst);
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    if let Err(e) = std::thread::Builder::new().name("desktop items".into()).spawn(run) {
        RUNNING.store(false, Ordering::SeqCst);
        log(&format!("desktop: could not start reading its items: {e}"));
    }
}

pub fn stop() {
    STOP.store(true, Ordering::SeqCst);
}

/// Reads everything again at once.
pub fn read_again() {
    AGAIN.store(true, Ordering::SeqCst);
}

/// The owner's Desktop folder (where the desktop's New makes things).
pub fn desktop_folder() -> Option<PathBuf> {
    crate::start_menu::known_folder(&FOLDERID_Desktop)
}

fn run() {
    // COM for this thread (the shell's icon lists and file information need it).
    bevel_winkit::com::init();
    let mut last: Option<(Vec<Seen>, bool)> = None;
    // Pictures and names already read, by place and time of the last change.
    let mut known: HashMap<(PathBuf, u64), Item> = HashMap::new();
    while !STOP.load(Ordering::SeqCst) {
        // A Desktop folder that cannot be read now (OneDrive busy?) is not an empty desktop:
        // the icons would lose their places. Read again a moment later.
        let Some(seen) = list() else {
            std::thread::sleep(Duration::from_millis(500));
            continue;
        };
        let full = recycle_full();
        let again = AGAIN.swap(false, Ordering::SeqCst);
        if again || last.as_ref() != Some(&(seen.clone(), full)) {
            let started = Instant::now();
            let mut files = Vec::with_capacity(seen.len());
            let mut fresh = HashMap::new();
            for s in &seen {
                let key = (s.path.clone(), s.modified);
                let item = match known.remove(&key) {
                    Some(item) if !again => item,
                    _ => shell::read_item(s),
                };
                files.push(item.clone());
                fresh.insert(key, item);
            }
            known = fresh;
            let took = started.elapsed().as_millis();
            if took > 500 {
                log(&format!("desktop: reading {} items took {took} ms", files.len()));
            }
            if last.as_ref().map(|(s, _)| s.len()) != Some(seen.len()) {
                log(&format!("desktop: {} items of the owner's", seen.len()));
            }
            if let Ok(mut latest) = LATEST.lock() {
                let hidden = seen.iter().filter(|s| s.hidden()).map(|s| s.path.to_string_lossy().into_owned()).collect();
                *latest = Some(Snapshot { files, recycle_full: full, hidden });
            }
            crate::desktop::items_ready();
            last = Some((seen, full));
        }
        let wait = Instant::now();
        while wait.elapsed() < EVERY && !STOP.load(Ordering::SeqCst) && !AGAIN.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    RUNNING.store(false, Ordering::SeqCst);
}

/// Everything in the two desktop folders that Windows' two switches show (`shell::list_folder`,
/// `bevel_core::hidden`), in the order of their paths. `None` if the owner's Desktop folder could
/// not be read.
fn list() -> Option<Vec<Seen>> {
    let mut out = Vec::new();
    let folders = [crate::start_menu::known_folder(&FOLDERID_Desktop), crate::start_menu::known_folder(&FOLDERID_PublicDesktop)];
    for (n, dir) in folders.into_iter().enumerate() {
        let Some(dir) = dir else { continue };
        match shell::list_folder(&dir) {
            Some((seen, _)) => out.extend(seen),
            None if n == 0 => return None,
            None => {}
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Some(out)
}

/// Whether the Recycle Bin (of every drive) holds anything.
fn recycle_full() -> bool {
    let mut info = SHQUERYRBINFO { cbSize: size_of::<SHQUERYRBINFO>() as u32, ..Default::default() };
    // SAFETY: a correctly sized out-struct; a null root means every drive.
    unsafe { SHQueryRecycleBinW(null_mut(), &mut info) == 0 && info.i64NumItems > 0 }
}
