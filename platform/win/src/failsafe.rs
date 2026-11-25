//! A way back after a failure in a bar style other than the classic one (the review of brief 112;
//! brief 114: one mark for XP's and 7's): when the bar ends by the panic rule while such a style
//! is on, a mark is left (`style-failed.txt` in the data folder, through `state_file`, holding the
//! style's word); the next start finds it and begins in the classic style for that run, without
//! changing settings.txt, and says so once in a message window naming the style that failed.
//! Choosing any look (the Theme menu, Bevel95 Settings) takes the mark away. `--restore` gives
//! Windows' taskbar back as always and leaves the mark: the next start of the bar is classic once.

use std::sync::atomic::{AtomicU8, Ordering};

use bevel_core::theme::{BarStyle, Era, look_title};

use crate::log::data_dir;

const FILE: &str = "style-failed.txt";

/// The style the bar runs in now (read by the panic rule, which must not borrow the bar's state).
static STYLE_NOW: AtomicU8 = AtomicU8::new(0);

fn path() -> std::path::PathBuf {
    data_dir().join(FILE)
}

/// The bar runs in `style` now.
pub fn style_now(style: BarStyle) {
    let n = match style {
        BarStyle::Classic => 0,
        BarStyle::Xp => 1,
        BarStyle::Seven => 2,
    };
    STYLE_NOW.store(n, Ordering::SeqCst);
}

/// The panic rule: leaves the mark if the bar ran in a style other than the classic one.
pub fn bar_failed() {
    let word = match STYLE_NOW.load(Ordering::SeqCst) {
        1 => BarStyle::Xp.setting(),
        2 => BarStyle::Seven.setting(),
        _ => return,
    };
    let _ = bevel_winkit::state_file::write_whole(&path(), format!("{word}\n").as_bytes());
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

/// The message of a start that fell back to the classic style, naming the style that failed
/// (the mark's word; Windows XP if it cannot be read).
pub fn message() -> String {
    let word = bevel_winkit::state_file::read(&path()).text().unwrap_or_default();
    let style = BarStyle::from_setting(word.trim()).filter(|s| s.is_later()).unwrap_or(BarStyle::Xp);
    let name = look_title(Era::Win2000, style);
    format!("The {name} style stopped with an error last time. The classic look is shown. Choose {name} again under Theme to try again.")
}
