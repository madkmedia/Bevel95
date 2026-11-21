//! The game owns each dialog. Cancel leaves the current game and its options alone.

use bevel_core::kit::Form;
use bevel_core::solitaire::choices::{self, Choices};
use bevel_core::solitaire::dialogs::{self, Sheet};
use bevel_core::solitaire::menu::Command;
use bevel_core::solitaire::settings::Settings;
use bevel_core::theme::Era;
use bevel_winkit::kit::{self, Hosted, Kind};
use bevel_winkit::log::log;
use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect;

use super::{Game, on_screen, work_area};

struct ChoiceWindow(Choices, Command);
struct MessageWindow(Sheet);

impl Hosted for ChoiceWindow {
    type F = Choices;
    fn form(&self) -> &Choices {
        &self.0
    }
    fn form_mut(&mut self) -> &mut Choices {
        &mut self.0
    }

    fn activate(part: choices::Part) {
        match part {
            choices::Part::Ok => {
                let edit = kit::with::<Self, _>(|dialog| (dialog.0.edited().clone(), dialog.1));
                kit::close::<Self>();
                if let Some((edit, command)) = edit {
                    apply(edit, command);
                }
            }
            choices::Part::Cancel | choices::Part::Close => kit::close::<Self>(),
            choices::Part::Tip(text) => {
                if let Some(hwnd) = kit::hwnd::<Self>() {
                    bevel_winkit::help::context_help(hwnd, text);
                }
            }
            _ => kit::update::<Self>(|dialog| dialog.0.activate(part)),
        }
    }
}

impl Hosted for MessageWindow {
    type F = Sheet;
    fn form(&self) -> &Sheet {
        &self.0
    }
    fn form_mut(&mut self) -> &mut Sheet {
        &mut self.0
    }
    fn request_close() {
        if kit::with::<Self, _>(|dialog| dialog.0.enabled(dialogs::Part::Close)) == Some(true) {
            kit::close::<Self>();
        }
    }
    fn activate(part: dialogs::Part) {
        match part {
            dialogs::Part::Yes => {
                kit::close::<Self>();
                kit::update::<Game>(Game::deal);
            }
            dialogs::Part::No | dialogs::Part::Ok | dialogs::Part::Close => kit::close::<Self>(),
        }
    }
}

fn apply(edit: Settings, command: Command) {
    kit::update::<Game>(|game| {
        let previous = game.settings.rules;
        let rules_changed = previous.draw != edit.rules.draw
            || previous.scoring != edit.rules.scoring
            || previous.timed != edit.rules.timed;
        game.settings = edit;
        game.form.set_back(usize::from(game.settings.deck));
        game.form.set_display(game.settings.status_bar, game.settings.outline);
        game.form.repaint_board();
        if rules_changed {
            game.deal();
        } else {
            game.form.game.set_cumulative(game.settings.rules.cumulative);
        }
        let saved = if command == Command::Deck {
            super::settings::save_back(&game.settings)
        } else {
            super::settings::save_options(&game.settings)
        };
        if let Err(error) = saved {
            log(&format!("Solitaire: could not save solitaire.txt: {error}"));
        }
    });
}

pub(super) fn close() {
    kit::close::<ChoiceWindow>();
    kit::close::<MessageWindow>();
}

pub(super) fn open(command: Command, keyboard: bool) {
    let Some((era, settings)) = kit::with::<Game, _>(|game| (game.form.era, game.settings.clone())) else { return };
    match command {
        Command::Deck | Command::Options => {
            let form =
                if command == Command::Deck { Choices::deck(era, &settings) } else { Choices::options(era, &settings) };
            present(ChoiceWindow(form, command), keyboard, Some(offset(era, command)));
        }
        Command::About => {
            let mut status = MEMORYSTATUSEX { dwLength: size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
            // SAFETY: Windows receives a writable structure with the required length.
            let kib = (unsafe { GlobalMemoryStatusEx(&mut status) } != 0).then_some(status.ullTotalPhys / 1024);
            present(MessageWindow(Sheet::about(era, kib)), keyboard, Some(offset(era, command)));
        }
        _ => {}
    }
}

pub(super) fn deal_again() {
    if let Some((era, keyboard)) = kit::with::<Game, _>(|game| (game.form.era, game.form.win_stopped_by_keyboard())) {
        present(MessageWindow(Sheet::deal_again(era)), keyboard, None);
    }
}

fn offset(era: Era, command: Command) -> (i32, i32) {
    match (era, command) {
        (Era::Win2000, Command::About) => (34, 75),
        (_, Command::About) => (36, 77),
        (Era::Win2000, Command::Deck) => (21, 62),
        (Era::Win2000, Command::Options) => (79, 92),
        _ => (54, 96),
    }
}

fn present<H: Hosted>(mut dialog: H, keyboard: bool, origin: Option<(i32, i32)>) {
    let (Some(owner), Some(look)) = (kit::hwnd::<Game>(), kit::look_of::<Game>()) else { return };
    if keyboard {
        dialog.form_mut().show_keys();
    }
    let mut bounds = RECT::default();
    // SAFETY: an owned HWND and an initialized writable rectangle.
    if unsafe { GetWindowRect(owner, &mut bounds) } == 0 {
        return;
    }
    let scale = look.scale;
    if kit::open_as(Kind::Dialog { owner }, dialog, look, |w, h| {
        let (x, y) = if let Some((dx, dy)) = origin {
            (bounds.left + scale.at(dx), bounds.top + scale.at(dy))
        } else {
            let screen = work_area(&bounds).rcMonitor;
            ((screen.left + screen.right - w) / 2 + scale.at(4), (screen.top + screen.bottom - h) / 2 + scale.at(9))
        };
        on_screen(x, y, w, h)
    })
    .is_none()
    {
        log("Solitaire: could not show a dialog");
    }
}
