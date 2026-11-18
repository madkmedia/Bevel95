//! Adapts the shared Windows menu tracker to Solitaire's menu bar and status line.

use super::{Game, run};
use bevel_core::solitaire::form::{Part, SolitaireWindow};
use bevel_core::solitaire::menu::{self, MENUS};
use bevel_winkit::menu as native;

pub use bevel_core::solitaire::menu::Command;

pub(super) fn bar(first: usize) {
    let menu = native::MenuBar {
        count: MENUS.len(),
        title_rect: SolitaireWindow::title_rect,
        menu: |form, title| (form.era, form.menu(title)),
        show_menu: |form, title| {
            form.menu_status = None;
            form.show_menu(title);
        },
        back_to_bar: |form, title| {
            form.menu_status = None;
            form.back_to_bar(title);
        },
        reached_by_keyboard: SolitaireWindow::reached_by_keyboard,
        selection: Some(|form, title, row| {
            form.menu_status = row.and_then(|i| menu::command_at(form.era, title, i)).map(|cmd| cmd.status(form.era));
        }),
    };
    if let Some(choice) = native::track_bar::<Game>(first, menu)
        && let Some(command) = menu::command_at(choice.era, choice.title, choice.row)
    {
        run(command, choice.keyboard);
    }
}

pub(super) fn system(at_pointer: bool, keyboard: bool) {
    native::track_window::<Game>(Part::Icon, |form| form.era, at_pointer, keyboard);
}
