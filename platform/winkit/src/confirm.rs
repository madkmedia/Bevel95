//! The look's own question before the owner's files are deleted (`bevel_core::delete_confirm`):
//! Confirm File Delete, Confirm Folder Delete, Confirm Multiple File Delete, before files go to
//! the Recycle Bin and before the Recycle Bin is emptied. Only Yes hands the work to Windows' file
//! machinery (`file_ops`); No, Esc and the title bar's X do nothing. Also the look's "Rename"
//! warning before a rename changes a file's extension (`RenameWarningHost`). Shared by the bar
//! (the classic desktop) and the folder windows; each says where the window goes and how it sits
//! among the others.

use std::cell::{Cell, RefCell};

use bevel_core::delete_confirm::{ConfirmWindow, Part, Question};
use bevel_core::desktop::ItemKind;
use bevel_core::rename_warning::{self, Part as WarningPart, RenameWarning};
use bevel_core::theme::Era;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, HWND_MESSAGE, PostMessageW, RegisterClassW, WM_APP, WNDCLASSW,
};

use crate::file_ops::{self, Target};
use crate::kit::{self, Hosted, Kind, Look};
use crate::file_ops::log;
use crate::util::{rect_h, rect_w};

/// What Yes does.
enum Pending {
    Delete(Vec<Target>),
    EmptyRecycleBin,
    /// Undo Copy: the copies still as they were made go to the Recycle Bin; then the look's
    /// message names those changed since (left), where the question was.
    UndoCopy(Vec<bevel_core::drag::Made>, Vec<String>, Option<Box<Place>>, crate::undo::Asking),
    /// The asker's own action (`ask_then`), carried out on this thread after Yes.
    Then(Box<dyn FnOnce()>),
}

thread_local! {
    static PENDING: RefCell<Option<Pending>> = const { RefCell::new(None) };
}

pub struct ConfirmHost {
    form: ConfirmWindow,
}

impl Hosted for ConfirmHost {
    type F = ConfirmWindow;
    fn form(&self) -> &ConfirmWindow {
        &self.form
    }
    fn form_mut(&mut self) -> &mut ConfirmWindow {
        &mut self.form
    }
    fn activate(part: Part) {
        let pending = PENDING.with(|p| p.borrow_mut().take());
        kit::close::<ConfirmHost>();
        if part != Part::Yes {
            log("the delete question was answered No");
            return;
        }
        match pending {
            Some(Pending::Delete(targets)) => file_ops::delete(targets),
            Some(Pending::EmptyRecycleBin) => file_ops::empty_recycle_bin(),
            Some(Pending::UndoCopy(take, left, place, asking)) => {
                crate::undo::delete_copies(take, !left.is_empty(), asking);
                if let Some(place) = place.filter(|_| !left.is_empty()).map(|p| *p) {
                    let (title, text) = bevel_core::drag::copies_left(place.era, &left);
                    show_message(place, &title, &text);
                }
            }
            Some(Pending::Then(yes)) => yes(),
            None => {}
        }
    }
    fn request_close() {
        Self::activate(Part::No);
    }
    fn closed(&mut self) {
        drop_pending();
    }
}

/// A question closed without an answer: its action goes with it.
fn drop_pending() {
    PENDING.with(|p| p.borrow_mut().take());
}

/// A new question's action takes the place of an open one's: the open question is closed first
/// (`close_open`, whose `closed` drops its own action), and only then is the new action stored.
/// The other way round, the closing took the new action and the new question's Yes did nothing
/// (the safety review of 2026-11-18, C1).
fn take_over(pending: Pending, close_open: impl FnOnce()) {
    close_open();
    PENDING.with(|p| *p.borrow_mut() = Some(pending));
}

/// Where a question goes and how it sits: the program's look, the window kind (the bar's own
/// windows stay above all; a folder window's question belongs to that window) and the screen's
/// work area it is centered in.
pub struct Place {
    pub look: Look,
    pub era: Era,
    pub kind: Kind,
    pub work: RECT,
}

impl Place {
    /// The same place for a second window (a message after a question); None for a program's
    /// own main window kinds, which carry pictures (no question goes there).
    fn again(&self) -> Option<Place> {
        let kind = match self.kind {
            Kind::Tool => Kind::Tool,
            Kind::Dialog { owner } => Kind::Dialog { owner },
            Kind::Owned { owner } => Kind::Owned { owner },
            Kind::Program { .. } | Kind::ResizableProgram { .. } => return None,
        };
        Some(Place { look: self.look.clone(), era: self.era, kind, work: self.work })
    }
}

/// Opens the question in the middle of the work area.
fn ask(place: Place, question: Question, pending: Pending, by_keyboard: bool) {
    take_over(pending, kit::close::<ConfirmHost>);
    let form = ConfirmWindow::new(place.era, question, by_keyboard);
    let work = place.work;
    let opened = kit::open_as(place.kind, ConfirmHost { form }, place.look, |w, h| {
        (work.left + (rect_w(&work) - w) / 2, work.top + (rect_h(&work) - h) / 2)
    });
    if opened.is_none() {
        PENDING.with(|p| p.borrow_mut().take());
    }
}

/// Delete on the selected icons: the look's question for one file, one folder or several items.
/// The look's own icons are never deleted (their menus have no Delete; a key leaves them out).
pub fn ask_delete(place: Place, targets: Vec<Target>, by_keyboard: bool) {
    let files: Vec<Target> = targets.into_iter().filter(|t| t.is_file()).collect();
    let question = match files.as_slice() {
        [] => return,
        [one] if one.kind == ItemKind::Folder => Question::Folder(one.name.clone()),
        [one] => Question::File(one.name.clone()),
        several => Question::Several(several.len()),
    };
    ask(place, question, Pending::Delete(files), by_keyboard);
}

/// The look's question `question`; Yes carries out `yes` (on this thread, after the window has
/// closed; it should check once more that what it acts on is still what the question named), No,
/// Esc and the X do nothing.
pub fn ask_then(place: Place, question: Question, by_keyboard: bool, yes: Box<dyn FnOnce()>) {
    ask(place, question, Pending::Then(yes), by_keyboard);
}

/// Empty Recycle Bin: the look's question (one item by its name, several by their number).
pub fn ask_empty(place: Place, by_keyboard: bool) {
    let n = file_ops::recycle_bin_count();
    let question = match (n, file_ops::recycle_bin_single_name()) {
        (0, _) => return,
        (1, Some(name)) => Question::EmptyOne(name),
        (n, _) => Question::EmptySeveral(n.max(1) as usize),
    };
    ask(place, question, Pending::EmptyRecycleBin, by_keyboard);
}

thread_local! {
    /// The extension warning was answered (so its closing does not answer No a second time).
    static WARNING_ANSWERED: Cell<bool> = const { Cell::new(false) };
    /// Who hears the answer of the warning that is open.
    static WARNING_ANSWER: Cell<Option<fn(bool)>> = const { Cell::new(None) };
}

/// The look's "Rename" message box before a rename changes a file's extension
/// (`bevel_core::rename_warning`). Its answer goes to the function given to `ask_rename` (the
/// rename box waits under it): Yes renames, No opens the box again.
pub struct RenameWarningHost {
    form: RenameWarning,
}

impl RenameWarningHost {
    fn answer(yes: bool) {
        if WARNING_ANSWERED.with(|a| a.replace(true)) {
            return;
        }
        log(if yes { "the extension warning was answered Yes" } else { "the extension warning was answered No" });
        if let Some(answered) = WARNING_ANSWER.with(Cell::take) {
            answered(yes);
        }
    }
}

impl Hosted for RenameWarningHost {
    type F = RenameWarning;
    fn form(&self) -> &RenameWarning {
        &self.form
    }
    fn form_mut(&mut self) -> &mut RenameWarning {
        &mut self.form
    }
    fn activate(part: WarningPart) {
        if part == WarningPart::Close {
            return; // grayed, as in Windows' message box
        }
        Self::answer(part == WarningPart::Yes);
        kit::close::<RenameWarningHost>();
    }
    /// Alt+F4 does nothing in a Yes/No message box.
    fn request_close() {}
    /// Gone without an answer (the program closing): the rename box opens again, nothing renamed.
    fn closed(&mut self) {
        Self::answer(false);
    }
}

/// Opens the extension warning where USER put a message box on the screen `screen` (the main
/// screen's whole rectangle), at the size `look.scale`. `by_keyboard`: the name was taken with
/// Enter. `answered` hears Yes or No (No also when the warning could not open).
pub fn ask_rename(look: Look, era: Era, kind: Kind, screen: RECT, by_keyboard: bool, answered: fn(bool)) {
    let scale = look.scale.max(1);
    // (An open one would be closed by `open_as`: it must not answer for this one.)
    WARNING_ANSWERED.with(|a| a.set(true));
    let form = RenameWarning::new(era, by_keyboard);
    let opened = kit::open_as(kind, RenameWarningHost { form }, look, |w, h| {
        let (x, y) = rename_warning::place((rect_w(&screen), rect_h(&screen)), (w, h));
        // The rule's fixed parts (7 and 17 pixels at 1x) grow with the size.
        (screen.left + x + (7 * scale - 7) / 2, screen.top + y + (17 * scale - 17) / 2)
    });
    WARNING_ANSWERED.with(|a| a.set(false));
    WARNING_ANSWER.with(|a| a.set(Some(answered)));
    if opened.is_none() {
        log("the extension warning could not open; nothing renamed");
        RenameWarningHost::answer(false);
    }
}

thread_local! {
    /// Where Undo's question or message goes while its checks are read on a thread of their own
    /// (`undo`); Some while they run.
    static UNDO_WAITING: RefCell<Option<Place>> = const { RefCell::new(None) };
    /// This thread's message-only window that hears when Undo's checks are known.
    static BACK: Cell<HWND> = const { Cell::new(null_mut()) };
}

/// Undo's checks are known: lparam holds a `Box<Option<Plan>>`.
const WM_UNDO_KNOWN: u32 = WM_APP + 0x95;
const BACK_CLASS: &str = "Bevel95UndoBack";

/// This thread's message-only window for Undo's answer (made at the first Undo).
fn back_window() -> Option<HWND> {
    let h = BACK.with(Cell::get);
    if !h.is_null() {
        return Some(h);
    }
    static REGISTERED: std::sync::Once = std::sync::Once::new();
    let name = crate::util::wide(BACK_CLASS);
    REGISTERED.call_once(|| {
        let class = WNDCLASSW {
            lpfnWndProc: Some(back_proc),
            // SAFETY: our own module.
            hInstance: unsafe { GetModuleHandleW(null()) },
            lpszClassName: name.as_ptr(),
            ..Default::default()
        };
        // SAFETY: a fully initialized class description (the name is copied by Windows).
        if unsafe { RegisterClassW(&class) } == 0 {
            crate::log::log_win32("RegisterClassW(undo answer)");
        }
    });
    // SAFETY: a message-only window of the class just registered, on this thread.
    let h = unsafe {
        CreateWindowExW(0, name.as_ptr(), null(), 0, 0, 0, 0, 0, HWND_MESSAGE, null_mut(), GetModuleHandleW(null()), null())
    };
    if h.is_null() {
        crate::log::log_win32("CreateWindowExW(undo answer)");
        return None;
    }
    BACK.with(|b| b.set(h));
    Some(h)
}

unsafe extern "system" fn back_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_UNDO_KNOWN && lparam != 0 {
        // SAFETY: made by Box::into_raw in `undo` and posted once.
        let plan = unsafe { Box::from_raw(lparam as *mut Option<crate::undo::Plan>) };
        if let Some(place) = UNDO_WAITING.with(|w| w.borrow_mut().take()) {
            undo_known(place, *plan);
        }
        return 0;
    }
    // SAFETY: the default for every other message.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// Undo (Edit > Undo, Ctrl+Z, the toolbar's Undo, the desktop's menu): the last step taken back;
/// a copy after the look's question ("Are you sure you want to delete 'x'?", the Reference PCs);
/// a step that cannot be done gets the look's message and changes nothing. The checks (the
/// list's lock, Undo Copy's proofs) are read on a thread of their own, so the window never
/// waits; the question or message comes once they are known. An Undo while the last one is still
/// being checked does nothing (it would reach the step before).
pub fn undo(place: Place) {
    if UNDO_WAITING.with(|w| w.borrow().is_some()) {
        log("Undo: the last Undo is still being checked; this one did nothing");
        return;
    }
    let Some(back) = back_window() else {
        log("Undo: no window to hear the answer; nothing was done");
        return;
    };
    UNDO_WAITING.with(|w| *w.borrow_mut() = Some(place));
    let back = back as usize; // a window's id travels to the thread as a number
    let started = crate::undo::undo_off_thread(move |plan| {
        let raw = Box::into_raw(Box::new(plan));
        // SAFETY: posting to this program's own window; the box is taken back if not posted.
        if unsafe { PostMessageW(back as HWND, WM_UNDO_KNOWN, 0, raw as LPARAM) } == 0 {
            crate::log::log_win32("Undo: the answer could not be posted");
            drop(unsafe { Box::from_raw(raw) });
        }
    });
    if !started {
        UNDO_WAITING.with(|w| w.borrow_mut().take());
    }
}

/// Undo's checks are known (on the window's thread again): the look's question or message.
fn undo_known(place: Place, plan: Option<crate::undo::Plan>) {
    match plan {
        Some(crate::undo::Plan::AskDelete { take, left, asking }) => {
            let question = match take.as_slice() {
                [] => {
                    // Every copy left was changed since it was made: only the message, and the
                    // list is emptied (no question was answered No).
                    asking.without_question();
                    if !left.is_empty() {
                        let (title, text) = bevel_core::drag::copies_left(place.era, &left);
                        show_message(place, &title, &text);
                    }
                    return;
                }
                [one] => Question::EmptyOne(name_of(&one.path)),
                several => Question::EmptySeveral(several.len()),
            };
            let again = place.again().map(Box::new);
            ask(place, question, Pending::UndoCopy(take, left, again, asking), false);
        }
        Some(crate::undo::Plan::Failed(step)) => {
            let (title, text) = bevel_core::drag::undo_failed(place.era, &step);
            show_message(place, &title, &text);
        }
        _ => {}
    }
}

fn name_of(path: &str) -> String {
    std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// A drop on the Recycle Bin in the 98 look: its Delete question first (95 and 2000 asked
/// nothing: the caller sends the files at once).
pub fn ask_recycle(place: Place, paths: Vec<String>) {
    let targets: Vec<Target> = paths
        .into_iter()
        .map(|p| {
            let folder = std::path::Path::new(&p).is_dir();
            let name = crate::shell::display_name(&p).unwrap_or_else(|| name_of(&p));
            Target { id: p, name, kind: if folder { ItemKind::Folder } else { ItemKind::File } }
        })
        .collect();
    ask_delete(place, targets, false);
}

/// Windows' message box with the error picture and OK (`DiskBox::message`).
pub struct MessageHost {
    form: bevel_core::disk_box::DiskBox,
}

impl Hosted for MessageHost {
    type F = bevel_core::disk_box::DiskBox;
    fn form(&self) -> &Self::F {
        &self.form
    }
    fn form_mut(&mut self) -> &mut Self::F {
        &mut self.form
    }
    fn activate(_part: bevel_core::disk_box::Part) {
        kit::close::<MessageHost>();
    }
    /// Gone (OK, Esc, the X, the program closing): who waits for it hears so.
    fn closed(&mut self) {
        if let Some(then) = MESSAGE_THEN.with(Cell::take) {
            then();
        }
    }
}

thread_local! {
    /// Who waits for the open message to go (`show_message_then`).
    static MESSAGE_THEN: Cell<Option<fn()>> = const { Cell::new(None) };
}

/// `show_message`, and `then` runs once the message has gone (also if it could not open).
pub fn show_message_then(place: Place, title: &str, text: &str, then: fn()) {
    show_message(place, title, text);
    if kit::windows::<MessageHost>().is_empty() {
        then();
    } else {
        MESSAGE_THEN.with(|t| t.set(Some(then)));
    }
}

/// A typed name that cannot be a file's name (`bevel_core::rename_warning::bad_name`): the look's
/// message; `then` runs once it has gone (the rename box opens again).
pub fn bad_name(place: Place, then: fn()) {
    log("the name typed holds a character a file name cannot have: the look's message, the box stays");
    let (title, text) = bevel_core::rename_warning::bad_name_message(place.era);
    show_message_then(place, &title, &text, then);
}

/// Shows the look's message box `title` / `text` in the middle of the work area.
pub fn show_message(place: Place, title: &str, text: &str) {
    let screen_w = rect_w(&place.work) / place.look.scale.max(1);
    let form = bevel_core::disk_box::DiskBox::message(place.era, title, text, screen_w, &place.look.regular);
    let work = place.work;
    if kit::open_as(place.kind, MessageHost { form }, place.look, |w, h| (work.left + (rect_w(&work) - w) / 2, work.top + (rect_h(&work) - h) / 2))
        .is_none()
    {
        log("a message could not open");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Delete question is open and a second question comes (another Delete, Undo Copy's):
    /// opening it closes the first (its `closed`), and the second's Yes must still have its action.
    #[test]
    fn a_second_question_keeps_its_own_action() {
        PENDING.with(|p| *p.borrow_mut() = Some(Pending::Delete(Vec::new())));
        take_over(Pending::EmptyRecycleBin, drop_pending);
        let left = PENDING.with(|p| p.borrow_mut().take());
        assert!(matches!(left, Some(Pending::EmptyRecycleBin)), "the new question's action is the one waiting");
    }
}
