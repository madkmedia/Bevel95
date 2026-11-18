//! The desktop's questions (`bevel_winkit::confirm`, shared with the folder windows): the look's
//! own question before the desktop deletes, before the Recycle Bin is emptied, and the "Rename"
//! warning before a rename changes a file's extension. Here they get the bar's look and its
//! screen, and stay above every window like the bar's other windows.

use bevel_winkit::confirm::{self, Place};

use crate::bar::with_app;
use crate::file_ops::Target;
use crate::kit::{Kind, Look};

/// Where the bar's questions go: the bar's look, in the middle of the bar's screen.
fn place() -> Option<Place> {
    with_app(|app| {
        let fonts = app.fonts();
        let look = Look { theme: app.theme, regular: fonts.regular.clone(), bold: fonts.bold.clone(), scale: app.scale.max(1) };
        Place { look, era: app.era, kind: Kind::Tool, work: app.monitor }
    })
}

/// Delete on the selected icons: the look's question for one file, one folder or several items.
pub fn ask_delete(targets: Vec<Target>, by_keyboard: bool) {
    if let Some(place) = place() {
        confirm::ask_delete(place, targets, by_keyboard);
    }
}

/// Opens the extension warning (posted by the desktop's thread), where USER put a message box on
/// the main screen. `by_keyboard`: the name was taken with Enter.
pub fn ask_rename(by_keyboard: bool) {
    let Some(place) = place() else {
        crate::desktop::rename_warning_answered(false);
        return;
    };
    confirm::ask_rename(place.look, place.era, Kind::Tool, place.work, by_keyboard, crate::desktop::rename_warning_answered);
}

/// A name typed in the desktop's rename box that cannot be a file's name: the look's message;
/// once it has gone the box opens again (as after the extension warning's No).
pub fn bad_name() {
    let Some(place) = place() else {
        crate::desktop::rename_warning_answered(false);
        return;
    };
    confirm::bad_name(place, || crate::desktop::rename_warning_answered(false));
}

/// One of the bar's own messages (`title`, `text`) in the look's message window, in the middle of
/// the bar's screen.
pub fn message(title: &str, text: &str) {
    if let Some(place) = place() {
        confirm::show_message(place, title, text);
    }
}

/// Empty Recycle Bin: the look's question (one item by its name, several by their number).
pub fn ask_empty(by_keyboard: bool) {
    if let Some(place) = place() {
        confirm::ask_empty(place, by_keyboard);
    }
}

/// Undo from the desktop (its menu, Ctrl+Z): the last step taken back, with the look's question
/// or message where one comes (`bevel_winkit::confirm::undo`).
pub fn undo() {
    if let Some(place) = place() {
        confirm::undo(place);
    }
}
