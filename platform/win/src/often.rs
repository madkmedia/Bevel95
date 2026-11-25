//! The programs started most often through Bevel95's Start menu and Quick Search (brief 112), for
//! Windows XP's Start menu: `often.txt` in Bevel95's data folder, read and written whole through
//! `state_file` (never replaced unread). It keeps only programs' names and their shortcuts; nothing
//! of it is ever logged. It starts empty.

use bevel_core::xp_start::{often_counted, often_top};

use crate::log::data_dir;

fn path() -> std::path::PathBuf {
    data_dir().join("often.txt")
}

/// Program `target` (shown as `name`) was started once more. Counted only under XP's and 7's bar
/// styles: a classic look writes no new file and behaves as before.
pub fn count(name: &str, target: &str) {
    // Programs only, never a document or folder (the review of brief 112, finding 7).
    if name.is_empty() || !bevel_core::xp_start::often_counts(target) || !crate::bar::with_app(|app| app.xp()).unwrap_or(false) {
        return;
    }
    if let Err(e) = bevel_winkit::state_file::change(&path(), "", |text| often_counted(text, name, target))
        && !bevel_winkit::state_file::already_logged(&e)
    {
        crate::log::log("often.txt could not be saved");
    }
}

/// The `n` programs started most often (name, target).
pub fn top(n: usize) -> Vec<(String, String)> {
    often_top(&bevel_winkit::state_file::read(&path()).text().unwrap_or_default(), n)
}
