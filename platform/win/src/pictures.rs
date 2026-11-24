//! Wallpaper pictures read from files (Windows 2000's JPEGs, a person's own picture), each on a
//! thread of its own, never on the bar's, the desktop's or a window's thread (`picture_file`).
//!
//! What is wanted is asked for by what it is (`PictureId`). A thread reads it and puts the answer
//! in its slot, then posts `Bevel95.Bar.Picture` to the bar; the bar (`display_window::pictures_came`)
//! hands what came to the desktop and to Display Properties. Only pictures still wanted are kept
//! (`keep_only`): an answer that comes back for a picture that is no longer the choice finds no
//! slot and is dropped, decided by what it is, not by the order the answers come in. Each reading
//! has a number of its own, so that an earlier reading of a picture read again is dropped too.
//!
//! A picture that cannot be read is kept as "cannot be shown" while it is wanted (not read again
//! and again), until the person asks for it anew (`again`: Browse..., Display Properties opening,
//! Apply and OK). A large picture (more than `KEEP_PIXELS`) is let go once what is made from it is
//! made (`let_go_large`: the desktop's picture, the monitor's small copies); its slot remembers
//! that it was read, and it is read again only when it is needed again (`again`).
//!
//! The log names Windows' own pictures by their names; a person's own picture only as "the
//! person's own picture", with its size in pixels and how long it took (never its file's name).

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use bevel_core::display::{Picture, PictureId};

use crate::log::log;
use crate::picture_file::{self, Refused};
use crate::util::Private;

/// Posted to the bar when a reading has finished (its answer is in its slot).
pub static WM_PICTURE: Private = Private::new("Bevel95.Bar.Picture");
/// Posted to the bar by the desktop when it must draw its picture again (a new size, look,
/// colors or screens) from a picture that was let go: wParam the number of that reading.
pub static WM_PICTURE_AGAIN: Private = Private::new("Bevel95.Bar.PictureAgain");

/// The desktop's thread: the picture of reading `reading` was let go and is needed again (the bar
/// reads it again: `display_window::desktop_needs_again`).
pub fn needed_again(reading: u64) {
    let bar = crate::bar::BAR_HWND.load(std::sync::atomic::Ordering::SeqCst) as windows_sys::Win32::Foundation::HWND;
    if !bar.is_null() {
        // SAFETY: a message to the bar's own window; nothing waits for it.
        unsafe { windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(bar, WM_PICTURE_AGAIN.id(), reading as usize, 0) };
    }
}

/// The line between a small picture, kept while it is wanted, and a large one, let go once what
/// is made from it is made: 4,194,304 pixels (16 MB decoded). Every picture of Windows' (at most
/// 800 x 600) and a picture of a screen's size up to 2560 x 1600 stay, so going through the list
/// or changing the size does not read them again; a photo of a camera or a phone (8 to 50 million
/// pixels, 32 to 200 MB decoded) goes.
pub const KEEP_PIXELS: usize = 4_194_304;

/// Whether `picture` is let go once what is made from it is made.
pub fn is_large(picture: &Picture) -> bool {
    picture.pixels.len() > KEEP_PIXELS
}

/// Where a wanted picture stands; each reading by its number.
#[derive(Clone)]
pub enum State {
    /// Not asked for (or no longer wanted).
    Absent,
    /// Being read in this reading; with the reading it had been read in before, when it is read
    /// again (the desktop keeps what it made from that one until the new one has come).
    Reading(u64, Option<u64>),
    Ready(u64, Arc<Picture>),
    /// Read, and let go once what is made from it was made (a large picture): still wanted, read
    /// again only when it is needed again (`again`).
    LetGo(u64),
    /// Why it failed (Browse... says it in its message).
    Failed(Refused),
}

/// A picture as handed to the desktop: the number of its reading (by which the desktop knows it
/// is the same picture) and the picture, unless it was let go (`State::LetGo`).
#[derive(Clone)]
pub struct Got {
    pub reading: u64,
    pub picture: Option<Arc<Picture>>,
}

/// Every picture wanted now, with where it stands (a handful at most: the desktop's, Display
/// Properties' monitor's, a Browse... answer's).
static SLOTS: Mutex<Vec<(PictureId, State)>> = Mutex::new(Vec::new());
/// The last reading's number.
static READINGS: AtomicU64 = AtomicU64::new(0);

/// Where `id` stands.
pub fn state(id: &PictureId) -> State {
    SLOTS.lock().ok().and_then(|s| s.iter().find(|(i, _)| i == id).map(|(_, st)| st.clone())).unwrap_or(State::Absent)
}

/// The picture of `id` if it has been read and is held.
pub fn ready(id: &PictureId) -> Option<Arc<Picture>> {
    match state(id) {
        State::Ready(_, p) => Some(p),
        _ => None,
    }
}

/// The picture of `id` for the desktop: read (held or let go), with its reading's number. While it
/// is read again, the reading before (so the desktop keeps what it made from that one until the
/// new one has come, instead of showing its plain color meanwhile).
pub fn got(id: &PictureId) -> Option<Got> {
    match state(id) {
        State::Ready(reading, p) => Some(Got { reading, picture: Some(p) }),
        State::LetGo(reading) | State::Reading(_, Some(reading)) => Some(Got { reading, picture: None }),
        _ => None,
    }
}

/// Asks for `id`: a thread starts reading it unless it is read, being read, let go or found
/// unreadable (each read again only by `again`).
pub fn want(id: &PictureId) {
    start(id, |_| false);
}

/// When `again` reads a picture anew.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anew {
    /// Only if it could not be read (the look's own picture when Display Properties opens, at
    /// Apply and OK).
    IfFailed,
    /// If it could not be read or was let go (Display Properties opening: its monitor needs it).
    IfNotHeld,
    /// Only if it was let go (chosen again in Display Properties' list: its monitor needs it).
    IfLetGo,
    /// Only if it was let go in reading number `n` (the desktop needs it again).
    IfLetGoIn(u64),
    /// Always, also when it is read or being read (a file chosen again with Browse...: it may have
    /// changed on the disk).
    Always,
}

/// Asks for `id` anew: as `want`, and a reading starts afresh where `anew` says.
pub fn again(id: &PictureId, anew: Anew) {
    start(id, |st| match (anew, st) {
        (Anew::Always, _) => true,
        (Anew::IfFailed, State::Failed(_)) => true,
        (Anew::IfNotHeld, State::Failed(_) | State::LetGo(_)) => true,
        (Anew::IfLetGo, State::LetGo(_)) => true,
        (Anew::IfLetGoIn(n), State::LetGo(m)) => n == *m,
        _ => false,
    });
}

/// Starts a reading of `id` if it has no slot, or if `afresh` says so of the slot it has.
fn start(id: &PictureId, afresh: impl Fn(&State) -> bool) {
    let reading = READINGS.fetch_add(1, Ordering::SeqCst) + 1;
    {
        let Ok(mut slots) = SLOTS.lock() else { return };
        match slots.iter_mut().find(|(i, _)| i == id) {
            Some(slot) if afresh(&slot.1) => {
                let was = match slot.1 {
                    State::Ready(n, _) | State::LetGo(n) | State::Reading(_, Some(n)) => Some(n),
                    _ => None,
                };
                slot.1 = State::Reading(reading, was);
            }
            Some(_) => return,
            None => slots.push((id.clone(), State::Reading(reading, None))),
        }
    }
    // Registered before the thread needs it (the number is asked of Windows once).
    WM_PICTURE.id();
    let wanted = id.clone();
    let spawned = std::thread::Builder::new().name("wallpaper picture".into()).spawn(move || read(wanted, reading));
    if let Err(e) = spawned {
        log(&format!("picture: could not start a thread to read it: {e}"));
        put(id, reading, State::Failed(Refused::Unreadable));
    }
}

/// Lets go of every picture that is not in `wanted` (a reading still under way is dropped when
/// it comes back).
pub fn keep_only(wanted: &[PictureId]) {
    if let Ok(mut slots) = SLOTS.lock() {
        slots.retain(|(i, _)| wanted.contains(i));
    }
}

/// The pictures Display Properties' page still needs (`let_go_large`'s `still`, kept for `drawn`).
static STILL: Mutex<Vec<PictureId>> = Mutex::new(Vec::new());
/// The last reading the classic desktop has drawn its picture from (0: none yet).
static DRAWN: AtomicU64 = AtomicU64::new(0);

/// Every large picture held, but those in `still` (a picture Display Properties' page shows, so
/// that its Apply or OK needs no new reading), is let go once what is made from it has been made:
/// the desktop has drawn that reading (`drawn`; until then a second `sync_desktop` must still hand
/// it the picture: the review before 0.1.4, C1), and the monitor makes its small copies at once.
/// Its slot remembers that it was read (`State::LetGo`).
pub fn let_go_large(still: &[PictureId]) {
    if let Ok(mut s) = STILL.lock() {
        *s = still.to_vec();
    }
    let drawn = DRAWN.load(Ordering::SeqCst);
    if let Ok(mut slots) = SLOTS.lock() {
        for (id, st) in slots.iter_mut() {
            if let State::Ready(n, p) = st
                && is_large(p)
                && *n == drawn
                && !still.contains(id)
            {
                *st = State::LetGo(*n);
            }
        }
    }
}

/// The desktop's thread: it has drawn its picture from reading `reading`; that picture, if large
/// and not needed by Display Properties' page, can go now.
pub fn drawn(reading: u64) {
    DRAWN.store(reading, Ordering::SeqCst);
    let still = STILL.lock().map(|s| s.clone()).unwrap_or_default();
    let_go_large(&still);
}

/// Where an answer went.
enum Put {
    Taken,
    /// `id` is no longer wanted.
    NotWanted,
    /// `id` is wanted, but is being read again (or was read again) since this reading began.
    Earlier,
}

/// The answer of reading `reading` goes into its slot, if `id` is still wanted and this is the
/// reading its slot waits for.
fn put(id: &PictureId, reading: u64, answer: State) -> Put {
    let Ok(mut slots) = SLOTS.lock() else { return Put::NotWanted };
    match slots.iter_mut().find(|(i, _)| i == id) {
        Some(slot) if matches!(slot.1, State::Reading(n, _) if n == reading) => {
            slot.1 = answer;
            Put::Taken
        }
        Some(_) => Put::Earlier,
        None => Put::NotWanted,
    }
}

/// What the log calls a picture: Windows' own by its name, a person's own never by its file's.
fn what(id: &PictureId) -> String {
    match id {
        PictureId::Windows(era, name) => format!("{}'s {name}", era.title()),
        PictureId::Own(_) => "the person's own picture".to_string(),
    }
}

/// Readings take turns: one picture is read and decoded at a time (a person's photo can take
/// hundreds of MB while it is decoded). Only reading threads wait for it, never the bar's or the
/// desktop's thread (they only start a reading).
static TURN: Mutex<()> = Mutex::new(());

/// Whether reading `reading` is still the one `id`'s slot waits for.
fn still_wanted(id: &PictureId, reading: u64) -> bool {
    SLOTS.lock().is_ok_and(|slots| slots.iter().any(|(i, st)| i == id && matches!(st, State::Reading(n, _) if *n == reading)))
}

/// How many decodes have begun, and the most at once (the tests count them).
#[cfg(test)]
static DECODES: AtomicU64 = AtomicU64::new(0);
#[cfg(test)]
static DECODING: AtomicU64 = AtomicU64::new(0);
#[cfg(test)]
static MOST_AT_ONCE: AtomicU64 = AtomicU64::new(0);

/// The reading thread: waits its turn, reads, puts the answer in its slot, tells the bar. A
/// reading that is no longer wanted when its turn comes, or once the file's bytes are in, ends
/// without decoding (the review before 0.1.4, B1).
fn read(id: PictureId, reading: u64) {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    if !still_wanted(&id, reading) {
        return;
    }
    let started = Instant::now();
    let bytes = match &id {
        PictureId::Windows(..) => id.bytes().map(std::borrow::Cow::Borrowed).ok_or(Refused::Unreadable),
        PictureId::Own(path) => picture_file::load_file(Path::new(path)).map(std::borrow::Cow::Owned),
    };
    if !still_wanted(&id, reading) {
        return;
    }
    #[cfg(test)]
    {
        DECODES.fetch_add(1, Ordering::SeqCst);
        let now = DECODING.fetch_add(1, Ordering::SeqCst) + 1;
        MOST_AT_ONCE.fetch_max(now, Ordering::SeqCst);
    }
    let answer = bytes.and_then(picture_file::read_owned);
    #[cfg(test)]
    DECODING.fetch_sub(1, Ordering::SeqCst);
    let ms = started.elapsed().as_millis();
    match &answer {
        Ok(p) => log(&format!("picture: {} read ({}x{}) in {ms} ms", what(&id), p.width, p.height)),
        Err(Refused::TooLarge) => log(&format!("picture: {} is larger than Bevel95 takes (found in {ms} ms)", what(&id))),
        Err(Refused::Unreadable) => log(&format!("picture: {} cannot be read or shown (found in {ms} ms)", what(&id))),
    }
    let state = match answer {
        Ok(p) => State::Ready(reading, Arc::new(p)),
        Err(r) => State::Failed(r),
    };
    match put(&id, reading, state) {
        Put::Taken => {}
        Put::NotWanted => {
            log(&format!("picture: {} is no longer wanted; dropped", what(&id)));
            return;
        }
        Put::Earlier => {
            log(&format!("picture: an earlier reading of {} dropped (it was read again since)", what(&id)));
            return;
        }
    }
    let bar = crate::bar::BAR_HWND.load(std::sync::atomic::Ordering::SeqCst) as windows_sys::Win32::Foundation::HWND;
    if !bar.is_null() {
        // SAFETY: a message to the bar's own window; nothing waits for it.
        unsafe { windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(bar, WM_PICTURE.id(), 0, 0) };
    }
}

/// The tests that touch the slots (which are the process's) take turns.
#[cfg(test)]
pub(crate) static TESTS: Mutex<()> = Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;
    use bevel_core::theme::Era;

    fn wait(id: &PictureId) -> State {
        for _ in 0..500 {
            match state(id) {
                State::Reading(..) => std::thread::sleep(std::time::Duration::from_millis(10)),
                other => return other,
            }
        }
        State::Reading(0, None)
    }

    /// A picture that could not be read is read again when it is asked for anew (Browse...,
    /// Display Properties opening, Apply and OK), not when it is only wanted; a file chosen again
    /// is read again whatever it was, so a changed file shows its new picture (the review of
    /// 2026-11-21, 1).
    #[test]
    fn a_picture_asked_for_anew_is_read_again() {
        let _turn = TESTS.lock().unwrap_or_else(|e| e.into_inner());
        let path = crate::picture_file::tests::scratch("comes-later.png");
        let _ = std::fs::remove_file(&path);
        let id = PictureId::Own(path.to_string_lossy().into_owned());
        // Missing at first (the drive not there): cannot be shown, and wanting it again changes
        // nothing.
        want(&id);
        assert!(matches!(wait(&id), State::Failed(Refused::Unreadable)));
        let (w, h) = (30, 20);
        crate::picture_file::tests::save(&path, w, h, &crate::picture_file::tests::test_pixels(w, h), "image/png");
        want(&id);
        assert!(matches!(state(&id), State::Failed(_)), "only wanted: not read again");
        // Asked for anew (the look's own picture at Display Properties' opening): read.
        again(&id, Anew::IfFailed);
        assert!(matches!(wait(&id), State::Ready(_, p) if (p.width, p.height) == (w, h)));
        // Read already: asked for anew only if it failed, nothing new happens.
        let State::Ready(first, _) = state(&id) else { panic!() };
        again(&id, Anew::IfFailed);
        assert!(matches!(state(&id), State::Ready(n, _) if n == first));
        // The file changed on the disk, chosen again with Browse...: the new picture.
        crate::picture_file::tests::save(&path, 12, 34, &crate::picture_file::tests::test_pixels(12, 34), "image/png");
        again(&id, Anew::Always);
        assert!(matches!(wait(&id), State::Ready(n, p) if n != first && (p.width, p.height) == (12, 34)));
        keep_only(&[]);
    }

    /// A large picture is let go once handed on, remembered as read (the desktop knows it by its
    /// reading's number), and read again only when it is needed again; a small one is kept.
    #[test]
    fn a_large_picture_is_let_go_and_read_again_when_needed() {
        let _turn = TESTS.lock().unwrap_or_else(|e| e.into_inner());
        let path = crate::picture_file::tests::scratch("large.png");
        // 2100 x 2000: 4.2 million pixels, just over the line (a PNG of one color is small).
        let (w, h) = (2100, 2000);
        crate::picture_file::tests::save(&path, w, h, &vec![0xFF33_6699; (w * h) as usize], "image/png");
        let large = PictureId::Own(path.to_string_lossy().into_owned());
        let small = PictureId::Windows(Era::Win2000, "Fly Away");
        want(&large);
        want(&small);
        let State::Ready(n, p) = wait(&large) else { panic!("read") };
        assert!(is_large(&p) && matches!(wait(&small), State::Ready(..)));
        drop(p);
        // Display Properties' page shows it: kept (its Apply needs no new reading).
        let_go_large(std::slice::from_ref(&large));
        assert!(matches!(state(&large), State::Ready(..)));
        // Handed to the desktop, but the desktop has not drawn it yet: a second `sync_desktop`
        // must still hand it the picture (the review before 0.1.4, C1).
        let_go_large(&[]);
        assert!(matches!(state(&large), State::Ready(..)), "kept until the desktop has drawn it");
        let_go_large(&[]);
        assert!(matches!(got(&large), Some(Got { picture: Some(_), .. })), "a second sync_desktop still hands it over");
        // The desktop has drawn it, not shown on the page: let go, still known by its reading.
        drawn(n);
        assert!(matches!(state(&large), State::LetGo(m) if m == n));
        assert!(matches!(got(&large), Some(Got { reading, picture: None }) if reading == n));
        assert!(matches!(state(&small), State::Ready(..)), "a small one stays");
        // Wanted again (the bar's every look at the desktop): not read again.
        want(&large);
        again(&large, Anew::IfFailed);
        assert!(matches!(state(&large), State::LetGo(_)));
        // The desktop needs it again for another reading than the one let go: nothing.
        again(&large, Anew::IfLetGoIn(n + 1_000_000));
        assert!(matches!(state(&large), State::LetGo(_)));
        // The desktop needs it again (a new size): read again, with a new number.
        again(&large, Anew::IfLetGoIn(n));
        // Meanwhile the desktop is still handed the reading it drew (or already the new one).
        assert!(got(&large).is_some(), "never nothing while it is read again");
        let State::Ready(m, _) = wait(&large) else { panic!("read again") };
        assert_ne!(m, n);
        // Display Properties' monitor needs it (drawn and let go): read again.
        drawn(m);
        assert!(matches!(state(&large), State::LetGo(_)));
        again(&large, Anew::IfNotHeld);
        assert!(matches!(wait(&large), State::Ready(..)));
        keep_only(&[]);
    }

    /// Going back and forth between two pictures (Display Properties' list: the page keeps the
    /// one it showed just before, `display_window::page_ids`) decodes each once, however often;
    /// readings take turns, never two decodes at once; nothing waits on the asking thread (the
    /// review before 0.1.4, B1).
    #[test]
    fn back_and_forth_reads_each_once_and_one_at_a_time() {
        let _turn = TESTS.lock().unwrap_or_else(|e| e.into_inner());
        let (w, h) = (1500, 1000);
        let mut ids = Vec::new();
        for name in ["forth-a.png", "forth-b.png"] {
            let path = crate::picture_file::tests::scratch(name);
            crate::picture_file::tests::save(&path, w, h, &crate::picture_file::tests::test_pixels(w, h), "image/png");
            ids.push(PictureId::Own(path.to_string_lossy().into_owned()));
        }
        keep_only(&[]);
        let before = DECODES.load(Ordering::SeqCst);
        MOST_AT_ONCE.store(0, Ordering::SeqCst);
        let started = Instant::now();
        // Ten clicks: each wants the row picked and keeps the one shown before.
        for k in 0..10 {
            let (now, before_it) = (&ids[k % 2], &ids[(k + 1) % 2]);
            keep_only(&[now.clone(), before_it.clone()]);
            want(now);
        }
        let asking = started.elapsed();
        // On the clock only on request (BEVEL95_TIMED=1): a busy PC must not fail the default run
        // (brief 104).
        if std::env::var_os("BEVEL95_TIMED").is_some() {
            assert!(asking.as_millis() < 200, "the asking thread waits for nobody ({asking:?})");
        }
        assert!(matches!(wait(&ids[0]), State::Ready(..)) && matches!(wait(&ids[1]), State::Ready(..)));
        let decodes = DECODES.load(Ordering::SeqCst) - before;
        assert_eq!(decodes, 2, "each picture decoded once");
        assert_eq!(MOST_AT_ONCE.load(Ordering::SeqCst), 1, "never two decodes at once");
        keep_only(&[]);
    }

    /// Without the page keeping the picture before, quick changes still end the readings no longer
    /// wanted before they decode: far fewer decodes than askings, one at a time.
    #[test]
    fn readings_no_longer_wanted_end_before_decoding() {
        let _turn = TESTS.lock().unwrap_or_else(|e| e.into_inner());
        let (w, h) = (1500, 1000);
        let mut ids = Vec::new();
        for name in ["quick-a.png", "quick-b.png"] {
            let path = crate::picture_file::tests::scratch(name);
            crate::picture_file::tests::save(&path, w, h, &crate::picture_file::tests::test_pixels(w, h), "image/png");
            ids.push(PictureId::Own(path.to_string_lossy().into_owned()));
        }
        keep_only(&[]);
        let before = DECODES.load(Ordering::SeqCst);
        MOST_AT_ONCE.store(0, Ordering::SeqCst);
        // Ten askings, each dropping the other picture's slot (and its reading under way).
        for k in 0..10 {
            let now = &ids[k % 2];
            keep_only(std::slice::from_ref(now));
            want(now);
        }
        let last = &ids[1];
        assert!(matches!(wait(last), State::Ready(..)));
        // Let every thread finish (each either decodes or ends before decoding).
        let _all = TURN.lock().unwrap_or_else(|e| e.into_inner());
        let decodes = DECODES.load(Ordering::SeqCst) - before;
        println!("ten askings, {decodes} decodes");
        assert!(decodes <= 3, "{decodes} decodes for ten askings");
        assert_eq!(MOST_AT_ONCE.load(Ordering::SeqCst), 1, "never two decodes at once");
        drop(_all);
        keep_only(&[]);
    }

    /// A reading that comes back for a picture no longer wanted is dropped by what it is; a
    /// picture still wanted gets its answer.
    #[test]
    fn a_late_answer_is_dropped() {
        let _turn = TESTS.lock().unwrap_or_else(|e| e.into_inner());
        let late = PictureId::Windows(Era::Win2000, "Windows 2000");
        let kept = PictureId::Windows(Era::Win2000, "Fly Away");
        want(&late);
        want(&kept);
        // The choice changed before the first answer came: only Fly Away is wanted now.
        keep_only(std::slice::from_ref(&kept));
        assert!(matches!(wait(&kept), State::Ready(_, p) if (p.width, p.height) == (281, 188)));
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(matches!(state(&late), State::Absent), "the late answer found no slot");
        // A wrong path is a plain "cannot be shown", kept while wanted (not read again and again).
        let gone = PictureId::Own("C:\\there\\is\\no\\such\\picture.png".into());
        want(&gone);
        assert!(matches!(wait(&gone), State::Failed(Refused::Unreadable)));
        keep_only(&[]);
        assert!(matches!(state(&kept), State::Absent) && matches!(state(&gone), State::Absent));
    }

    /// What a 12-million-pixel photo (4000 x 3000, a JPEG of the test's own making) costs as the
    /// desktop's wallpaper: the peak while it is read, and what is held once the desktop's picture
    /// (1280 x 720, Size 2 on a 2560 x 1440 screen) is made from it. Run on request (it prints):
    /// `cargo test -p bevel95 --bin bevel95 -- --ignored --nocapture memory_of_a_big_photo`.
    #[test]
    #[ignore = "measures memory in a process of its own; prints numbers"]
    fn memory_of_a_big_photo() {
        let path = crate::picture_file::tests::scratch("big-photo-4000x3000.jpg");
        let turned = crate::picture_file::tests::scratch("big-photo-4000x3000-turned.jpg");
        if !path.exists() || !turned.exists() {
            let pixels = crate::picture_file::tests::test_pixels(4000, 3000);
            crate::picture_file::tests::save(&path, 4000, 3000, &pixels, "image/jpeg");
            // The same photo as a camera held on its side writes it (orientation 6: turned).
            std::fs::write(&turned, crate::picture_file::tests::with_orientation(&std::fs::read(&path).unwrap(), 6)).unwrap();
        }
        for file in [&path, &turned] {
            let me = std::env::current_exe().unwrap();
            let out = bevel_winkit::quiet::command(me)
                .args(["--ignored", "--exact", "pictures::tests::memory_of_a_big_photo_alone", "--test-threads=1", "--nocapture"])
                .env("BEVEL95_BIG_PHOTO", file)
                .output()
                .unwrap();
            let text = String::from_utf8_lossy(&out.stdout);
            println!("{}: {}", file.file_name().unwrap().to_string_lossy(), text.lines().find(|l| l.contains("peak")).unwrap_or(&text));
            assert!(out.status.success() && text.contains("1 passed"), "{text}{}", String::from_utf8_lossy(&out.stderr));
        }
    }

    /// The process's private bytes now and at their peak.
    fn memory() -> (usize, usize) {
        use windows_sys::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
        let mut c = PROCESS_MEMORY_COUNTERS { cb: size_of::<PROCESS_MEMORY_COUNTERS>() as u32, ..Default::default() };
        // SAFETY: a plain query about this process into a sized struct.
        unsafe { K32GetProcessMemoryInfo(windows_sys::Win32::System::Threading::GetCurrentProcess(), &mut c, c.cb) };
        (c.PagefileUsage, c.PeakPagefileUsage)
    }

    #[test]
    #[ignore = "run alone by memory_of_a_big_photo"]
    fn memory_of_a_big_photo_alone() {
        use bevel_core::display::{Choice, Show};
        let path = std::path::PathBuf::from(std::env::var_os("BEVEL95_BIG_PHOTO").expect("run by memory_of_a_big_photo"));
        let file = std::fs::metadata(&path).unwrap().len();
        let mb = |b: usize| b as f64 / 1_000_000.0;
        let (base, _) = memory();
        let id = PictureId::Own(path.to_string_lossy().into_owned());
        want(&id);
        assert!(matches!(wait(&id), State::Ready(..)));
        let (read, peak) = memory();
        // As the bar does it: the desktop is handed the picture (`got`), the slot lets it go, the
        // desktop makes its picture from it and lets it go too.
        let choice = Choice { wallpaper: Some(path.to_string_lossy().into_owned()), show: Show::Stretch, pattern: None };
        let theme = bevel_core::theme::theme_in(Era::Win98, bevel_core::theme::Depth::TrueColor);
        let mut handed = got(&id).unwrap();
        let_go_large(&[]);
        let background = bevel_core::display::background_on(theme, &choice, handed.picture.as_deref(), &[], 1280, 720);
        assert!(background.is_some());
        drawn(handed.reading);
        handed.picture = None;
        assert!(matches!(state(&id), State::LetGo(_)));
        let (shown, _) = memory();
        println!(
            "file {:.1} MB; read: peak +{:.1} MB, held +{:.1} MB; with the desktop's 1280x720 picture: held +{:.1} MB",
            file as f64 / 1_000_000.0,
            mb(peak - base),
            mb(read - base),
            mb(shown - base)
        );
        drop(background);
        keep_only(&[]);
    }
}
