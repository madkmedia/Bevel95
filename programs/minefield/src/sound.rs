//! Play Windows 2000 Minesweeper's embedded sound cues.

use std::ptr::null_mut;
use std::sync::Once;

pub use bevel_core::minefield::audio::Cue;
use bevel_winkit::log::log;
use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};

pub fn play(cue: Cue) {
    static REPORT: Once = Once::new();
    // SAFETY: SND_MEMORY takes RIFF bytes. These are valid for the process lifetime,
    // including after this asynchronous call returns.
    if unsafe { PlaySoundW(cue.wave().as_ptr().cast(), null_mut(), SND_ASYNC | SND_MEMORY | SND_NODEFAULT) } == 0 {
        REPORT.call_once(|| log("Minesweeper: Windows could not play the game sounds"));
    }
}

pub fn stop() {
    // SAFETY: a null sound stops the asynchronous sound playing in this process.
    unsafe { PlaySoundW(std::ptr::null(), null_mut(), 0) };
}
