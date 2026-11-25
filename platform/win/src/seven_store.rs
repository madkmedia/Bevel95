//! What Windows 7's slots, XP's and 7's live pictures and 7's Start menu show of files (brief
//! 114; the review of brief 114, finding 2; the owner, 2026-11-25): the program file each Quick
//! Launch shortcut points at, and the shell's picture of each shortcut and program, 32×32 for the
//! 1x painting and at each final size above size 1. Read on a thread of its own, never on the
//! bar's (a shortcut on a network place that does not answer would hold the bar's paint, and with
//! it the whole bar). The bar says what it wants (`want`, `want_sized`) and paints what is known
//! meanwhile (`program_of`, `picture`, `picture_at`: a lookup only, never a file call); the thread
//! reads what is new, reads a file again when its size or time of change changes (a shortcut
//! changed in Quick Launch's folder), forgets what is no longer wanted (a size no longer in use
//! too), and repaints the bar when something arrived. A window's program changing is a new file
//! wanted. The log names no file.

use std::collections::HashMap;
use std::ptr::null;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use bevel_core::fb::Icon;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Gdi::InvalidateRect;

use crate::log::log;

/// Who wants files read: the bar's 1x paint, the bar's final-size pictures, the Start menu's 1x
/// pictures and its final-size ones (each replaces only its own list).
#[derive(Clone, Copy, Debug)]
pub enum Asker {
    Bar = 0,
    BarSized = 1,
    Menu = 2,
    MenuSized = 3,
}

/// The 1x pictures' size.
pub const ONE_X: i32 = 32;

#[derive(Clone, Debug, Default)]
struct Entry {
    /// The file's size and time of change when it was read (`None`: it could not be looked at).
    stamp: Option<(u64, u64)>,
    /// A shortcut's program file (read with its 32×32 picture).
    program: Option<String>,
    icon: Option<Icon>,
}

type Key = (String, i32);

static WANTED: Mutex<[Vec<Key>; 4]> = Mutex::new([Vec::new(), Vec::new(), Vec::new(), Vec::new()]);
static KNOWN: Mutex<Option<HashMap<Key, Entry>>> = Mutex::new(None);
static RUNNING: AtomicBool = AtomicBool::new(false);
/// Set (under its lock) when the wanted lists changed; wakes the thread from its wait.
static WAKE: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());

/// How often the files wanted are looked at again (a change of a shortcut in Quick Launch's
/// folder shows within this).
const EVERY: Duration = Duration::from_millis(2000);

/// Whether a window's key or a shortcut is a file the shell can be asked about (a window known
/// only by its frame's title or its class, `frame:` and `class:`, is not).
pub fn is_file(key: &str) -> bool {
    let b = key.as_bytes();
    (b.len() > 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/')) || key.starts_with("\\\\")
}

/// `asker` now wants these files (shortcuts and programs) read at 32×32.
pub fn want(asker: Asker, files: &[String]) {
    want_sized(asker, &files.iter().map(|f| (f.clone(), ONE_X)).collect::<Vec<_>>());
}

/// `asker` now wants these files' pictures at these sizes. Only a lock and a comparison on the
/// caller's thread; the reading thread is started at the first call and woken by a change.
pub fn want_sized(asker: Asker, files: &[Key]) {
    let files: Vec<Key> = files.iter().filter(|(f, size)| is_file(f) && (1..=256).contains(size)).cloned().collect();
    let changed = match WANTED.lock() {
        Ok(mut wanted) => {
            let slot = &mut wanted[asker as usize];
            let changed = *slot != files;
            if changed {
                *slot = files;
            }
            changed
        }
        Err(_) => false,
    };
    if !RUNNING.swap(true, Ordering::SeqCst)
        && let Err(e) = std::thread::Builder::new().name("windows 7 pictures".into()).spawn(run)
    {
        RUNNING.store(false, Ordering::SeqCst);
        log(&format!("Windows 7's slots: could not start reading their pictures: {e}"));
        return;
    }
    if changed && let Ok(mut flag) = WAKE.0.lock() {
        *flag = true;
        WAKE.1.notify_all();
    }
}

fn known<T>(file: &str, size: i32, f: impl FnOnce(&Entry) -> Option<T>) -> Option<T> {
    KNOWN.lock().ok()?.as_ref()?.get(&(file.to_string(), size)).and_then(f)
}

/// The program file shortcut `link` points at, as far as it is read yet.
pub fn program_of(link: &str) -> Option<String> {
    known(link, ONE_X, |e| e.program.clone())
}

/// The shell's 32×32 picture of `file`, as far as it is read yet.
pub fn picture(file: &str) -> Option<Icon> {
    picture_at(file, ONE_X)
}

/// The shell's picture of `file` at `size`, as far as it is read yet.
pub fn picture_at(file: &str, size: i32) -> Option<Icon> {
    known(file, size, |e| e.icon.clone())
}

/// Whether `file` has been read at `size` (its picture or its lack of one is known).
#[cfg(test)]
fn is_read(file: &str, size: i32) -> bool {
    KNOWN.lock().ok().and_then(|k| k.as_ref().map(|k| k.contains_key(&(file.to_string(), size)))).unwrap_or(false)
}

fn stamp_of(file: &str) -> Option<(u64, u64)> {
    use std::os::windows::fs::MetadataExt;
    std::fs::metadata(file).ok().map(|m| (m.len(), m.last_write_time()))
}

fn is_shortcut(file: &str) -> bool {
    file.len() > 4 && file[file.len() - 4..].eq_ignore_ascii_case(".lnk")
}

fn read((file, size): &Key, stamp: Option<(u64, u64)>) -> Entry {
    if *size == ONE_X {
        let program = if is_shortcut(file) { crate::find_window::read_shortcut_program(file) } else { None };
        Entry { stamp, program, icon: crate::window_list::read_big_icon(file) }
    } else {
        Entry { stamp, program: None, icon: crate::window_list::read_icon_at(file, *size) }
    }
}

fn run() {
    // COM for this thread (shortcuts and the shell's pictures need it).
    bevel_winkit::com::init();
    loop {
        let wanted: Vec<Key> = match WANTED.lock() {
            Ok(w) => {
                let mut all: Vec<Key> = w.iter().flatten().cloned().collect();
                all.sort();
                all.dedup();
                all
            }
            Err(_) => Vec::new(),
        };
        let is_known = |k: &Key| KNOWN.lock().ok().is_some_and(|known| known.as_ref().is_some_and(|known| known.contains_key(k)));
        // What was never read comes first, files on this PC before network places and the 1x
        // pictures before the larger ones (a file that does not answer then delays only the files
        // after it).
        let (mut new, old): (Vec<&Key>, Vec<&Key>) = wanted.iter().partition(|k| !is_known(k));
        new.sort_by_key(|(file, size)| (file.starts_with("\\\\"), *size != ONE_X));
        let mut arrived = 0usize;
        let started = Instant::now();
        for key in new.into_iter().chain(old) {
            let stamp = stamp_of(&key.0);
            let same = KNOWN.lock().ok().is_some_and(|k| k.as_ref().and_then(|k| k.get(key)).is_some_and(|e| e.stamp == stamp));
            if same {
                continue;
            }
            let entry = read(key, stamp);
            if let Ok(mut k) = KNOWN.lock() {
                k.get_or_insert_with(HashMap::new).insert(key.clone(), entry);
            }
            arrived += 1;
            repaint();
        }
        // What is no longer wanted goes (a size no longer in use too).
        if let Ok(mut k) = KNOWN.lock()
            && let Some(k) = k.as_mut()
        {
            k.retain(|key, _| wanted.binary_search(key).is_ok());
        }
        let took = started.elapsed().as_millis();
        if took > 1000 {
            // Counts only: the log names no file (CLAUDE.md, the log's rule).
            log(&format!("Windows 7's slots: reading {arrived} of {} pictures took {took} ms", wanted.len()));
        }
        if let Ok(flag) = WAKE.0.lock() {
            let (mut flag, _) = match WAKE.1.wait_timeout_while(flag, EVERY, |changed| !*changed) {
                Ok(r) => r,
                Err(_) => continue,
            };
            *flag = false;
        }
    }
}

fn repaint() {
    let bar = crate::bar::BAR_HWND.load(Ordering::SeqCst) as HWND;
    if !bar.is_null() {
        // SAFETY: InvalidateRect may be called from any thread; the bar may be gone (then it fails).
        unsafe { InvalidateRect(bar, null(), 0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_files_are_asked_about() {
        assert!(is_file("C:\\Windows\\notepad.exe"));
        assert!(is_file("\\\\server\\share\\Some program.lnk"));
        assert!(!is_file("frame:Some title"));
        assert!(!is_file("class:SomeClass"));
        assert!(!is_file(""));
        assert!(is_shortcut("C:\\a\\Some program.LNK"));
        assert!(!is_shortcut("C:\\a\\Some program.exe"));
    }

    /// The review of brief 114, finding 2: wanting and looking up never touch a file on the
    /// caller's thread (a network place that does not answer, TEST-NET-1, is wanted beside a real
    /// program); the real program's picture arrives from the store's thread, at 32 and at 64
    /// (the owner, 2026-11-25: a true larger picture, its soft edges kept); what is no longer
    /// wanted is forgotten, a size no longer in use too.
    #[test]
    fn the_bars_thread_never_waits_for_a_file() {
        let program = bevel_winkit::shell::system_program("notepad.exe");
        let dead = "\\\\192.0.2.1\\nothing here\\Some program.lnk".to_string();
        let t0 = Instant::now();
        want(Asker::Menu, &[program.clone(), dead.clone()]);
        want_sized(Asker::BarSized, &[(program.clone(), 64)]);
        assert!(picture(&dead).is_none() && program_of(&dead).is_none(), "nothing known yet, and nothing waited for");
        let asked = t0.elapsed();
        if std::env::var_os("BEVEL95_TIMED").is_some() {
            assert!(asked < Duration::from_millis(50), "wanting and looking up took {asked:?}");
        }
        let deadline = Instant::now() + Duration::from_secs(20);
        while !(is_read(&program, ONE_X) && is_read(&program, 64)) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        let icon = picture(&program).expect("the program's picture arrived");
        assert_eq!((icon.width, icon.height), (32, 32));
        let big = picture_at(&program, 64).expect("Notepad has a picture larger than 48");
        assert_eq!((big.width, big.height), (64, 64));
        assert!(big.pixels.iter().any(|p| p >> 24 == 0), "see-through corners kept");
        want(Asker::Menu, &[]);
        want_sized(Asker::BarSized, &[]);
        let deadline = Instant::now() + Duration::from_secs(60);
        while (is_read(&program, ONE_X) || is_read(&program, 64)) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(!is_read(&program, ONE_X) && !is_read(&program, 64), "what is no longer wanted is forgotten");
    }
}
