//! Wallpaper pictures read from files (Windows 2000's JPEGs, a person's own picture), each on a
//! thread of its own, never on the bar's, the desktop's or a window's thread (`picture_file`).
//!
//! What is wanted is asked for by what it is (`PictureId`). A thread reads it and puts the answer
//! in its slot, then posts `Bevel95.Bar.Picture` to the bar; the bar (`display_window::pictures_came`)
//! hands what came to the desktop and to Display Properties. Only pictures still wanted are kept
//! (`keep_only`): an answer that comes back for a picture that is no longer the choice finds no
//! slot and is dropped, decided by what it is, not by the order the answers come in.
//!
//! The log names Windows' own pictures by their names; a person's own picture only as "the
//! person's own picture", with its size in pixels and how long it took (never its file's name).

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use bevel_core::display::{Picture, PictureId};

use crate::log::log;
use crate::picture_file::{self, Refused};
use crate::util::Private;

/// Posted to the bar when a reading has finished (its answer is in its slot).
pub static WM_PICTURE: Private = Private::new("Bevel95.Bar.Picture");

/// Where a wanted picture stands.
#[derive(Clone)]
pub enum State {
    /// Not asked for (or no longer wanted).
    Absent,
    Reading,
    Ready(Arc<Picture>),
    /// Why it failed (Browse... says it in its message).
    Failed(Refused),
}

/// Every picture wanted now, with where it stands (a handful at most: the desktop's, Display
/// Properties' monitor's, a Browse... answer's).
static SLOTS: Mutex<Vec<(PictureId, State)>> = Mutex::new(Vec::new());

/// Where `id` stands.
pub fn state(id: &PictureId) -> State {
    SLOTS.lock().ok().and_then(|s| s.iter().find(|(i, _)| i == id).map(|(_, st)| st.clone())).unwrap_or(State::Absent)
}

/// The picture of `id` if it has been read.
pub fn ready(id: &PictureId) -> Option<Arc<Picture>> {
    match state(id) {
        State::Ready(p) => Some(p),
        _ => None,
    }
}

/// Asks for `id`: a thread starts reading it unless it is read, being read or found unreadable.
pub fn want(id: &PictureId) {
    {
        let Ok(mut slots) = SLOTS.lock() else { return };
        if slots.iter().any(|(i, _)| i == id) {
            return;
        }
        slots.push((id.clone(), State::Reading));
    }
    // Registered before the thread needs it (the number is asked of Windows once).
    WM_PICTURE.id();
    let wanted = id.clone();
    let spawned = std::thread::Builder::new().name("wallpaper picture".into()).spawn(move || read(wanted));
    if let Err(e) = spawned {
        log(&format!("picture: could not start a thread to read it: {e}"));
        put(id, State::Failed(Refused::Unreadable));
    }
}

/// Lets go of every picture that is not in `wanted` (a reading still under way is dropped when
/// it comes back).
pub fn keep_only(wanted: &[PictureId]) {
    if let Ok(mut slots) = SLOTS.lock() {
        slots.retain(|(i, _)| wanted.contains(i));
    }
}

/// The answer goes into its slot, if `id` is still wanted; true if it was.
fn put(id: &PictureId, answer: State) -> bool {
    let Ok(mut slots) = SLOTS.lock() else { return false };
    match slots.iter_mut().find(|(i, st)| i == id && matches!(st, State::Reading)) {
        Some(slot) => {
            slot.1 = answer;
            true
        }
        None => false,
    }
}

/// What the log calls a picture: Windows' own by its name, a person's own never by its file's.
fn what(id: &PictureId) -> String {
    match id {
        PictureId::Windows(era, name) => format!("{}'s {name}", era.title()),
        PictureId::Own(_) => "the person's own picture".to_string(),
    }
}

/// The reading thread: reads, puts the answer in its slot, tells the bar.
fn read(id: PictureId) {
    let started = Instant::now();
    let answer = match &id {
        PictureId::Windows(..) => match id.bytes() {
            Some(bytes) => picture_file::read_bytes(bytes),
            None => Err(Refused::Unreadable),
        },
        PictureId::Own(path) => picture_file::read_file(Path::new(path)),
    };
    let ms = started.elapsed().as_millis();
    match &answer {
        Ok(p) => log(&format!("picture: {} read ({}x{}) in {ms} ms", what(&id), p.width, p.height)),
        Err(Refused::TooLarge) => log(&format!("picture: {} is larger than Bevel95 takes (found in {ms} ms)", what(&id))),
        Err(Refused::Unreadable) => log(&format!("picture: {} cannot be read or shown (found in {ms} ms)", what(&id))),
    }
    let state = match answer {
        Ok(p) => State::Ready(Arc::new(p)),
        Err(r) => State::Failed(r),
    };
    if !put(&id, state) {
        log(&format!("picture: {} is no longer wanted; dropped", what(&id)));
        return;
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
                State::Reading => std::thread::sleep(std::time::Duration::from_millis(10)),
                other => return other,
            }
        }
        State::Reading
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
        assert!(matches!(wait(&kept), State::Ready(p) if (p.width, p.height) == (281, 188)));
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(matches!(state(&late), State::Absent), "the late answer found no slot");
        // A wrong path is a plain "cannot be shown", kept while wanted (not read again and again).
        let gone = PictureId::Own("C:\\there\\is\\no\\such\\picture.png".into());
        want(&gone);
        assert!(matches!(wait(&gone), State::Failed(Refused::Unreadable)));
        keep_only(&[]);
        assert!(matches!(state(&kept), State::Absent) && matches!(state(&gone), State::Absent));
    }
}
