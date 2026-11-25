//! A way back after a failure in a bar style other than the classic one (the review of brief 112):
//! when the bar ends by the panic rule while that style is on, a mark is left (`style-failed.txt`
//! in the data folder, through `state_file`); the next start finds it and begins in the classic
//! style for that run, without changing settings.txt, and says so once in a message window.
//! Choosing any look (the Theme menu, Bevel95 Settings) takes the mark away. `--restore` gives
//! Windows' taskbar back as always and leaves the mark: the next start of the bar is classic once.

use std::sync::atomic::{AtomicU8, Ordering};

use bevel_core::theme::BarStyle;

use crate::log::data_dir;

const FILE: &str = "style-failed.txt";

/// The style the bar runs in now (read by the panic rule, which must not borrow the bar's state).
static STYLE_NOW: AtomicU8 = AtomicU8::new(0);

fn path() -> std::path::PathBuf {
    data_dir().join(FILE)
}

/// The bar runs in `style` now.
pub fn style_now(style: BarStyle) {
    STYLE_NOW.store(if style == BarStyle::Classic { 0 } else { 1 }, Ordering::SeqCst);
}

/// The panic rule: leaves the mark if the bar ran in a style other than the classic one.
pub fn bar_failed() {
    if STYLE_NOW.load(Ordering::SeqCst) != 0 {
        let _ = bevel_winkit::state_file::write_whole(&path(), b"The bar stopped with an error in a style other than the classic one.\n");
    }
}

/// Whether the mark is there.
pub fn marked() -> bool {
    path().is_file()
}

/// Takes the mark away (a look was chosen).
pub fn clear() {
    if marked() && std::fs::remove_file(path()).is_err() {
        crate::log::log("the mark of a failed bar style could not be taken away");
    }
}

/// The message of a start that fell back to the classic style.
pub const MESSAGE: &str = "The Windows XP style stopped with an error last time. The classic look is shown. Choose Windows XP again under Theme to try again.";
