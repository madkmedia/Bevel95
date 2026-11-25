//! What Windows 7's slots and Start menu show of files (brief 114; the review of brief 114,
//! finding 2): the program file each Quick Launch shortcut points at, and the shell's 32×32
//! picture of each shortcut and program. Read on a thread of its own, never on the bar's (a
//! shortcut on a network place that does not answer would hold the bar's paint, and with it the
//! whole bar). The bar says what it wants (`want`) and paints what is known meanwhile
//! (`program_of`, `picture`: a lookup only, never a file call); the thread reads what is new,
//! reads a file again when its size or time of change changes (a shortcut changed in Quick
//! Launch's folder), forgets what is no longer wanted, and repaints the bar when something
//! arrived. A window's program changing is a new file wanted. The log names no file.

use std::collections::HashMap;
use std::ptr::null;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use bevel_core::fb::Icon;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Gdi::InvalidateRect;

use crate::log::log;

/// Who wants files read: the bar's paint and the Start menu (each replaces only its own list).
#[derive(Clone, Copy, Debug)]
pub enum Asker {
    Bar = 0,
    Menu = 1,
}

#[derive(Clone, Debug, Default)]
struct Entry {
    /// The file's size and time of change when it was read (`None`: it could not be looked at).
    stamp: Option<(u64, u64)>,
    /// A shortcut's program file.
    program: Option<String>,
    icon: Option<Icon>,
}

static WANTED: Mutex<[Vec<String>; 2]> = Mutex::new([Vec::new(), Vec::new()]);
static KNOWN: Mutex<Option<HashMap<String, Entry>>> = Mutex::new(None);
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

/// `asker` now wants these files (shortcuts and programs) read. Only a lock and a comparison on
/// the caller's thread; the reading thread is started at the first call and woken by a change.
pub fn want(asker: Asker, files: &[String]) {
    let files: Vec<String> = files.iter().filter(|f| is_file(f)).cloned().collect();
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

fn known<T>(file: &str, f: impl FnOnce(&Entry) -> Option<T>) -> Option<T> {
    KNOWN.lock().ok()?.as_ref()?.get(file).and_then(f)
}

/// The program file shortcut `link` points at, as far as it is read yet.
pub fn program_of(link: &str) -> Option<String> {
    known(link, |e| e.program.clone())
}

/// The shell's 32×32 picture of `file`, as far as it is read yet.
pub fn picture(file: &str) -> Option<Icon> {
    known(file, |e| e.icon.clone())
}

/// Whether `file` has been read (its picture or its lack of one is known).
#[cfg(test)]
fn is_read(file: &str) -> bool {
    KNOWN.lock().ok().and_then(|k| k.as_ref().map(|k| k.contains_key(file))).unwrap_or(false)
}

fn stamp_of(file: &str) -> Option<(u64, u64)> {
    use std::os::windows::fs::MetadataExt;
    std::fs::metadata(file).ok().map(|m| (m.len(), m.last_write_time()))
}

fn is_shortcut(file: &str) -> bool {
    file.len() > 4 && file[file.len() - 4..].eq_ignore_ascii_case(".lnk")
}

fn read(file: &str, stamp: Option<(u64, u64)>) -> Entry {
    let program = if is_shortcut(file) { crate::find_window::read_shortcut_program(file) } else { None };
    Entry { stamp, program, icon: crate::window_list::read_big_icon(file) }
}

fn run() {
    // COM for this thread (shortcuts and the shell's pictures need it).
    bevel_winkit::com::init();
    loop {
        let wanted: Vec<String> = match WANTED.lock() {
            Ok(w) => {
                let mut all: Vec<String> = w.iter().flatten().cloned().collect();
                all.sort();
                all.dedup();
                all
            }
            Err(_) => Vec::new(),
        };
        let is_known = |f: &str| KNOWN.lock().ok().is_some_and(|k| k.as_ref().is_some_and(|k| k.contains_key(f)));
        // What was never read comes first (a file that does not answer then delays only the
        // files looked at again after it).
        let (new, old): (Vec<&String>, Vec<&String>) = wanted.iter().partition(|f| !is_known(f));
        let mut arrived = 0usize;
        let started = Instant::now();
        for file in new.into_iter().chain(old) {
            let stamp = stamp_of(file);
            let same = KNOWN.lock().ok().is_some_and(|k| k.as_ref().and_then(|k| k.get(file.as_str())).is_some_and(|e| e.stamp == stamp));
            if same {
                continue;
            }
            let entry = read(file, stamp);
            if let Ok(mut k) = KNOWN.lock() {
                k.get_or_insert_with(HashMap::new).insert(file.clone(), entry);
            }
            arrived += 1;
            repaint();
        }
        if let Ok(mut k) = KNOWN.lock()
            && let Some(k) = k.as_mut()
        {
            k.retain(|f, _| wanted.binary_search(f).is_ok());
        }
        let took = started.elapsed().as_millis();
        if took > 1000 {
            // Counts only: the log names no file (CLAUDE.md, the log's rule).
            log(&format!("Windows 7's slots: reading {arrived} of {} files took {took} ms", wanted.len()));
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
    /// program); the real program's picture arrives from the store's thread; what is no longer
    /// wanted is forgotten.
    #[test]
    fn the_bars_thread_never_waits_for_a_file() {
        let program = bevel_winkit::shell::system_program("notepad.exe");
        let dead = "\\\\192.0.2.1\\nothing here\\Some program.lnk".to_string();
        let t0 = Instant::now();
        want(Asker::Menu, &[program.clone(), dead.clone()]);
        assert!(picture(&dead).is_none() && program_of(&dead).is_none(), "nothing known yet, and nothing waited for");
        let asked = t0.elapsed();
        if std::env::var_os("BEVEL95_TIMED").is_some() {
            assert!(asked < Duration::from_millis(50), "wanting and looking up took {asked:?}");
        }
        let deadline = Instant::now() + Duration::from_secs(20);
        while !is_read(&program) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        let icon = picture(&program).expect("the program's picture arrived");
        assert_eq!((icon.width, icon.height), (32, 32));
        want(Asker::Menu, &[]);
        let deadline = Instant::now() + Duration::from_secs(60);
        while is_read(&program) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(!is_read(&program), "what is no longer wanted is forgotten");
    }
}
