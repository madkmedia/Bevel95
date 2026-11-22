//! Display Properties, what Properties on the classic desktop's menu opens (unless Bevel95
//! Settings' Desktop page says Windows' own display settings): `bevel_core::display::form` draws
//! each look's Background page, `bevel_core::display::pattern` 98's and 2000's Pattern window; this
//! module opens them, keeps each look's choice and carries out OK and Apply.
//!
//! Each look's choice is kept in background.txt in the data folder (`display::parse`,
//! `display::with_choice`): read when the bar starts, and again for every change, which changes
//! only that look's lines (`state_file::change`: never written while it cannot be read). A person
//! who never opens the window has no such file and the plain desktop. Apply puts the choice into
//! effect at once: the classic desktop draws it (`bar::sync_desktop`).

use std::path::PathBuf;
use std::sync::Mutex;

use bevel_core::display::form::{DisplayProperties, DisplayWindow, Part};
use bevel_core::display::pattern::{self, PatternWindow};
use bevel_core::display::{self, Choice, PictureId};

use crate::{picture_file, pictures, wallpaper_browse};
use bevel_core::theme::Era;
use bevel_winkit::state_file;

use crate::bar::{self, with_app};
use crate::kit::{self, Hosted};
use crate::log::{data_dir, log, path_kind};
use crate::util::take_focus;

/// The live tests' picture (test message 6): a file of the test's own making in the data folder.
const TEST_PICTURE: &str = "bevel95-test-picture.png";

/// Each look's choice in effect (95, 98, 2000), as read from background.txt.
static CHOICES: Mutex<Option<[Choice; 3]>> = Mutex::new(None);
/// A live test's choice, shown on the desktop instead of the look's own and never saved
/// (`test_message`): the person's own choice and files stay as they are.
static TEST: Mutex<Option<(Era, Choice)>> = Mutex::new(None);

fn file() -> PathBuf {
    data_dir().join(display::FILE)
}

fn index(era: Era) -> usize {
    match era {
        Era::Win95 => 0,
        Era::Win98 => 1,
        Era::Win2000 => 2,
    }
}

/// Reads background.txt (the bar's start). Missing: every look plain. Unreadable: logged by
/// `load`, every look plain, the file kept as it is and not written over.
pub fn load() {
    let choices = match state_file::load(&file()).text() {
        Some(text) => display::parse(&text),
        None => Default::default(),
    };
    let chosen = choices.iter().filter(|c| **c != Choice::plain()).count();
    if chosen > 0 {
        log(&format!("background.txt: a wallpaper or pattern chosen for {chosen} of the looks"));
    }
    if let Ok(mut c) = CHOICES.lock() {
        *c = Some(choices);
    }
}

/// What the classic desktop shows in `era`: a live test's choice while there is one, else the
/// look's own.
pub fn choice(era: Era) -> Choice {
    if let Some((e, c)) = TEST.lock().ok().and_then(|t| t.clone())
        && e == era
    {
        return c;
    }
    saved(era)
}

/// The look's choice in effect (without a live test's).
fn saved(era: Era) -> Choice {
    CHOICES.lock().ok().and_then(|c| c.as_ref().map(|all| all[index(era)].clone())).unwrap_or_default()
}

/// OK and Apply: `choice` is now `era`'s; saved (only that look's lines change) and shown.
fn put_in_effect(era: Era, choice: Choice) {
    if let Ok(mut c) = CHOICES.lock() {
        c.get_or_insert_with(Default::default)[index(era)] = choice.clone();
    }
    // A live test's choice never hides what the person chose.
    drop_test();
    // The picture chosen is read anew if it could not be read before (the drive is there now).
    if let Some(id) = choice.to_read(era) {
        pictures::again(&id, pictures::Anew::IfFailed);
    }
    let path = file();
    if let Err(e) = state_file::change(&path, "", |text| display::with_choice(text, era, &choice))
        && !state_file::is_read_only(&e)
    {
        // The choice holds for this run (the desktop shows it); the file stays as it was.
        log(&format!("could not save the wallpaper and pattern to {}: {e}", path_kind(&path)));
    }
    // Windows' own names, Bevel95's data: no text from outside Bevel95 (a person's own picture
    // never by its file's name or path).
    log(&format!(
        "display: {} wallpaper {}, {}, pattern {}",
        era.title(),
        choice.wallpaper_said(),
        choice.show.word(),
        choice.pattern.as_deref().unwrap_or("(None)")
    ));
    bar::sync_desktop();
}

/// The picture the classic desktop draws, when it shows (the same test as `bar::sync_desktop`:
/// switched on, and the bar is Bevel95's): the look's choice in effect, or a live test's.
fn desktop_id() -> Option<PictureId> {
    if !bar::classic_desktop_shows() {
        return None;
    }
    let era = with_app(|app| app.era)?;
    choice(era).to_read(era)
}

/// The pictures Display Properties' page needs: its monitor's and a Browse... answer's.
///
/// While the window handles a message its state is lifted out of the kit (`kit::with` gives None
/// then, and says nothing), so its monitor's picture is missing from an ask made from inside its
/// own message handling: such an ask passes it in as `extra` (as `input_changed` does), or the
/// monitor's reading would be dropped.
fn page_ids() -> Vec<PictureId> {
    let mut ids = Vec::new();
    if let Some(Some(id)) = kit::with::<DisplayHost, _>(|h| h.form.props.to_read()) {
        ids.push(id);
    }
    ids.extend(browsed_id());
    ids
}

/// Every picture to read that is wanted now: the desktop's (only while it shows), Display
/// Properties' page's (`page_ids`), and `extra` (asked for by a window that is busy).
fn wanted(extra: Option<PictureId>) -> Vec<PictureId> {
    let mut ids: Vec<PictureId> = desktop_id().into_iter().collect();
    ids.extend(page_ids());
    ids.extend(extra);
    ids
}

/// The classic desktop went (switched off, or the bar switched to Windows' taskbar): its picture
/// is let go, unless Display Properties' page shows it.
pub fn desktop_gone() {
    pictures::keep_only(&wanted(None));
}

/// The desktop has been handed what came (`bar::sync_desktop`), or Display Properties closed: a
/// large picture is let go, unless the page still shows it (its Apply or OK would need it).
pub fn settle() {
    pictures::let_go_large(&page_ids());
}

/// The desktop must draw its picture again (a new size, look, colors or screens) and the picture
/// of reading `reading` was let go (`pictures::WM_PICTURE_AGAIN`): read again, once.
pub fn desktop_needs_again(reading: u64) {
    if let Some(id) = desktop_id() {
        pictures::again(&id, pictures::Anew::IfLetGoIn(reading));
    }
}

/// Lets go of the pictures no longer wanted and asks for the wanted ones (each read on a thread
/// of its own unless it is there already).
fn ask(extra: Option<PictureId>) {
    let ids = wanted(extra);
    pictures::keep_only(&ids);
    for id in &ids {
        pictures::want(id);
    }
}

/// The picture the desktop draws `choice` with in `era`, when it is one to read and has been read
/// (held, or let go once the desktop made its picture: `pictures::Got`); asked for if not
/// (`bar::sync_desktop`, only while the desktop shows).
pub fn picture_for(era: Era, choice: &Choice) -> Option<pictures::Got> {
    let id = choice.to_read(era)?;
    ask(None);
    pictures::got(&id)
}

/// The person's own picture chosen with Browse... while it is being read (its file's full path).
static BROWSED: Mutex<Option<String>> = Mutex::new(None);

fn browsed_id() -> Option<PictureId> {
    BROWSED.lock().ok().and_then(|b| b.clone()).map(PictureId::Own)
}

/// Browse...: Windows' own file window over Display Properties, which waits behind it
/// (`wallpaper_browse`: its own thread).
fn browse() {
    if wallpaper_browse::is_open() {
        return;
    }
    let Some(hwnd) = kit::hwnd::<DisplayHost>() else { return };
    let Some(era) = kit::with::<DisplayHost, _>(|h| h.form.props.era) else { return };
    // SAFETY: our own window; enabled again when the answer comes or it closes.
    unsafe { windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(hwnd, 0) };
    if wallpaper_browse::open(era, hwnd).is_some() {
        log("Browse: Windows' file window opened");
    } else {
        // SAFETY: as above.
        unsafe { windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(hwnd, 1) };
    }
}

/// The file window has gone (`wallpaper_browse::WM_BROWSED`): Display Properties is usable again;
/// a file chosen is read on its own thread and comes into the list once it can be shown.
pub fn browsed(ticket: u64) {
    let Some(answer) = wallpaper_browse::answer(ticket) else { return };
    if let Some(hwnd) = kit::hwnd::<DisplayHost>() {
        // SAFETY: our own window.
        unsafe { windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(hwnd, 1) };
        take_focus(hwnd);
    }
    let Some(path) = answer else { return };
    choose_browsed(path);
}

/// A file chosen with Browse...: read on its own thread, always anew (it may have changed on the
/// disk, or have been on a drive that was not there before), then into the list once it can be
/// shown.
fn choose_browsed(path: String) {
    if !display::is_own(&path) {
        cannot_be_shown(picture_file::Refused::Unreadable);
        return;
    }
    let id = PictureId::Own(path.clone());
    if let Ok(mut b) = BROWSED.lock() {
        *b = Some(path);
    }
    pictures::again(&id, pictures::Anew::Always);
    ask(None);
    take_browsed();
}

/// The picture chosen with Browse..., once read: into the list and onto the monitor; or, if it
/// cannot be shown, the look's message and the choice unchanged.
fn take_browsed() {
    let Some(id) = browsed_id() else { return };
    let PictureId::Own(path) = id.clone() else { return };
    match pictures::state(&id) {
        pictures::State::Ready(_, p) => {
            if let Ok(mut b) = BROWSED.lock() {
                *b = None;
            }
            kit::update::<DisplayHost>(|h| {
                h.form.props.add_own(&path);
                h.form.props.set_read(id, Some(p));
            });
        }
        pictures::State::Failed(why) => {
            if let Ok(mut b) = BROWSED.lock() {
                *b = None;
            }
            cannot_be_shown(why);
        }
        _ => {}
    }
}

/// The look's message for a picture chosen that cannot be shown (PROVISIONAL words: nothing
/// measured).
fn cannot_be_shown(why: picture_file::Refused) {
    let text = match why {
        picture_file::Refused::TooLarge => "This picture is too large to be used as wallpaper.",
        picture_file::Refused::Unreadable => "This file cannot be shown as wallpaper. It may be damaged, or it is not a picture Bevel95 can read.",
    };
    crate::confirm_window::message("Display Properties", text);
}

/// Display Properties' monitor gets the picture it waits for, if it has come.
fn feed_window() {
    let need = kit::with::<DisplayHost, _>(|h| h.form.props.to_read().filter(|id| !h.form.props.has_read(id))).flatten();
    if let Some(id) = need
        && let Some(p) = pictures::ready(&id)
    {
        kit::update::<DisplayHost>(|h| h.form.props.set_read(id, Some(p)));
    }
}

/// A reading has finished (`pictures::WM_PICTURE`): what is no longer wanted goes, the monitor
/// and the desktop take what came.
pub fn pictures_came() {
    ask(None);
    take_browsed();
    feed_window();
    bar::sync_desktop();
}

/// The window, while it is open (one at a time).
pub struct DisplayHost {
    form: DisplayWindow,
}

impl Hosted for DisplayHost {
    type F = DisplayWindow;
    fn form(&self) -> &DisplayWindow {
        &self.form
    }
    fn form_mut(&mut self) -> &mut DisplayWindow {
        &mut self.form
    }
    fn activate(part: Part) {
        activate(part);
    }
    fn request_close() {
        close();
    }
    /// A JPEG or a person's own picture chosen in the list: asked for on its own thread; the
    /// monitor shows the ground until it has come.
    fn input_changed(&mut self) {
        let Some(id) = self.form.props.to_read() else { return };
        if self.form.props.has_read(&id) {
            return;
        }
        ask(Some(id.clone()));
        // The desktop's large picture may have been let go: the monitor needs it again.
        pictures::again(&id, pictures::Anew::IfLetGo);
        if let Some(p) = pictures::ready(&id) {
            self.form.props.set_read(id, Some(p));
        }
    }
}

/// The Pattern window (98, 2000), a dialog of Display Properties.
pub struct PatternHost {
    form: PatternWindow,
}

impl Hosted for PatternHost {
    type F = PatternWindow;
    fn form(&self) -> &PatternWindow {
        &self.form
    }
    fn form_mut(&mut self) -> &mut PatternWindow {
        &mut self.form
    }
    fn activate(part: pattern::Part) {
        activate_pattern(part);
    }
}

/// Properties on the desktop's menu (or Alt+Enter on it): Display Properties, or Windows' own
/// display settings when Bevel95 Settings says so (`windows_display_settings`).
pub fn open_from_desktop() {
    if with_app(|app| app.windows_display_settings).unwrap_or(false) {
        log("desktop: Properties opens Windows' display settings (Bevel95 Settings says so)");
        crate::window_list::launch(bar::WINDOWS_DISPLAY_SETTINGS);
        return;
    }
    open();
}

/// Opens Display Properties on the look's choice in effect (or brings it forward).
pub fn open() {
    if let Some(hwnd) = kit::hwnd::<PatternHost>().or_else(kit::hwnd::<DisplayHost>) {
        take_focus(hwnd);
        return;
    }
    let Some(era) = with_app(|app| app.era) else { return };
    let props = DisplayProperties::new(era, saved(era));
    let Some(form) = with_app(|app| DisplayWindow::new(props, &app.fonts())) else { return };
    // Near the main screen's top left, as Bevel95's other property windows (Date/Time).
    let place = |w: i32, h: i32| {
        with_app(|app| {
            let s = app.scale;
            let x = (app.monitor.left + s.len(44)).min(app.monitor.right - w).max(app.monitor.left);
            let y = (app.monitor.top + s.len(44)).min(app.bar_top_on_screen() - h).max(app.monitor.top);
            (x, y)
        })
        .unwrap_or((0, 0))
    };
    if kit::open(DisplayHost { form }, place).is_some() {
        log("opened Display Properties");
        // The look's own picture, for the monitor: read anew if it could not be read before (the
        // drive is there now) or was let go once the desktop had made its picture (a large one).
        if let Some(id) = saved(era).to_read(era) {
            pictures::again(&id, pictures::Anew::IfNotHeld);
        }
        ask(None);
        feed_window();
    }
}

/// Closes Display Properties and its Pattern window, applying nothing.
pub fn close() {
    // An open file window of Browse... goes too; a picture being read for it is dropped.
    wallpaper_browse::close();
    if let Ok(mut b) = BROWSED.lock() {
        *b = None;
    }
    kit::close::<PatternHost>();
    kit::close::<DisplayHost>();
    // Its monitor's picture is let go (unless the desktop draws it; a large one the desktop draws
    // is let go too, the desktop having made its own picture from it).
    ask(None);
    settle();
}

/// The bar's look, colors or size changed: the windows close (they show the old look's lists;
/// what was applied stays applied).
pub fn look_changed() {
    if kit::hwnd::<DisplayHost>().is_some() || kit::hwnd::<PatternHost>().is_some() {
        close();
    }
    // A live test's choice goes with the look it was made in (it never comes back with it).
    if drop_test() {
        bar::sync_desktop();
    }
}

/// Takes a live test's choice away; true if there was one.
fn drop_test() -> bool {
    let gone = TEST.lock().ok().and_then(|mut t| t.take()).is_some();
    if gone {
        log("display: the live test's choice is gone; the look's own shows again");
    }
    gone
}

fn apply() {
    let mut applied = None;
    kit::update::<DisplayHost>(|h| {
        if h.form.props.changed() {
            applied = Some((h.form.props.era, h.form.props.apply()));
        }
    });
    if let Some((era, choice)) = applied {
        put_in_effect(era, choice);
    }
}

fn activate(part: Part) {
    let Some(enabled) = kit::with::<DisplayHost, _>(|h| h.form.props.enabled(part)) else { return };
    if !enabled {
        return;
    }
    match part {
        Part::Ok => {
            apply();
            close();
        }
        Part::Apply => apply(),
        Part::Cancel | Part::TitleClose => close(),
        Part::TitleHelp => crate::window_list::launch("https://support.microsoft.com/windows"),
        Part::PatternButton => open_pattern(),
        Part::Browse => browse(),
        _ => {}
    }
}

/// Opens the Pattern window over Display Properties, on the page's pattern; Display Properties
/// waits until it closes. Where Windows put it: at the top left of its owner's inside.
fn open_pattern() {
    let Some(owner) = kit::hwnd::<DisplayHost>() else { return };
    let Some((era, chosen)) = kit::with::<DisplayHost, _>(|h| (h.form.props.era, h.form.props.shown.pattern.clone())) else {
        return;
    };
    let form = PatternWindow::new(era, chosen.as_deref());
    let place = |_w: i32, _h: i32| {
        let mut r = windows_sys::Win32::Foundation::RECT::default();
        // SAFETY: our own window; a plain query into a local.
        unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect(owner, &mut r) };
        let s = with_app(|app| app.scale).unwrap_or(bevel_core::scale::Scale::ONE);
        (r.left + s.len(3), r.top + s.len(22))
    };
    if kit::open_dialog(owner, PatternHost { form }, place).is_some() {
        log("opened the Pattern window");
    }
}

fn activate_pattern(part: pattern::Part) {
    match part {
        pattern::Part::Ok => {
            let chosen = kit::with::<PatternHost, _>(|h| h.form.chosen()).flatten();
            kit::close::<PatternHost>();
            kit::update::<DisplayHost>(|h| h.form.props.set_pattern(chosen));
        }
        pattern::Part::Cancel | pattern::Part::TitleClose => kit::close::<PatternHost>(),
        pattern::Part::TitleHelp => crate::window_list::launch("https://support.microsoft.com/windows"),
        _ => {}
    }
}

/// `Bevel95.Bar.Display` (live tests, by name from a DPI-aware process): wParam 0 opens Display
/// Properties, 1 closes it (and the Pattern window) applying nothing, 2 opens the Pattern window
/// over it, 3 shows the look's wallpaper number lParam (1 = the list's first) tiled on the desktop,
/// 4 the look's pattern number lParam, 5 takes a test's choice away again. A test's choice is never
/// saved and never changes the person's own: the desktop shows it until 5, a restart, a change of
/// look, colors or size, or the person's Apply or OK. 2 opens the Pattern window only where a
/// person could (98 and 2000, Pattern... not grayed). 6 shows the test picture
/// `bevel95-test-picture.png` of the data folder (a file the test makes and takes away again):
/// lParam 0 tiled, 1 centered, 2 stretched; it is read on its own thread like a person's picture.
pub fn test_message(wparam: usize, lparam: isize) {
    let Some(era) = with_app(|app| app.era) else { return };
    match wparam {
        0 => open(),
        1 => close(),
        // Only where a person could: a look that has the Pattern window, its button not grayed.
        2 if kit::with::<DisplayHost, _>(|h| h.form.props.era != Era::Win95 && h.form.props.enabled(Part::PatternButton)) == Some(true) => {
            open_pattern()
        }
        3 | 4 => {
            let n = usize::try_from(lparam).unwrap_or(0).max(1) - 1;
            let mut c = Choice::plain();
            if wparam == 3 {
                c.wallpaper = display::wallpapers(era).get(n).map(|w| w.name.to_string());
                c.show = display::Show::Tile;
            } else {
                c.pattern = display::patterns(era).get(n).map(|p| p.name.to_string());
            }
            log(&format!("display: a live test shows {} (not saved)", if wparam == 3 { "a wallpaper" } else { "a pattern" }));
            if let Ok(mut t) = TEST.lock() {
                *t = Some((era, c));
            }
            bar::sync_desktop();
        }
        5 if drop_test() => bar::sync_desktop(),
        // The test picture of a live test (a file of the test's own making in the data folder,
        // by its fixed name: never a path of the person's), lParam 0 tiled, 1 centered, 2
        // stretched (95: centered).
        6 => {
            let mut c = Choice::plain();
            c.wallpaper = Some(data_dir().join(TEST_PICTURE).to_string_lossy().into_owned());
            c.show = match lparam {
                1 => display::Show::Center,
                2 if era != Era::Win95 => display::Show::Stretch,
                2 => display::Show::Center,
                _ => display::Show::Tile,
            };
            log(&format!("display: a live test shows its test picture {} (not saved)", c.show.word()));
            if let Ok(mut t) = TEST.lock() {
                *t = Some((era, c));
            }
            bar::sync_desktop();
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevel_core::display::Show;

    #[test]
    fn a_choice_is_saved_for_its_look_only_and_read_back() {
        let _turn = pictures::TESTS.lock().unwrap_or_else(|e| e.into_inner());
        let dir = data_dir();
        assert!(!dir.to_string_lossy().contains("AppData\\Local\\Bevel95"), "a scratch data folder");
        let path = file();
        let _ = std::fs::remove_file(&path);
        load();
        assert_eq!(saved(Era::Win98), Choice::plain(), "no file: the plain desktop");
        let forest = Choice { wallpaper: Some("Forest".into()), show: Show::Tile, pattern: None };
        if let Ok(mut c) = CHOICES.lock() {
            c.get_or_insert_with(Default::default)[1] = forest.clone();
        }
        state_file::change(&path, "", |text| display::with_choice(text, Era::Win98, &forest)).unwrap();
        let tulip = Choice { wallpaper: None, show: Show::Center, pattern: Some("Tulip".into()) };
        state_file::change(&path, "", |text| display::with_choice(text, Era::Win2000, &tulip)).unwrap();
        load();
        assert_eq!((saved(Era::Win95), saved(Era::Win98), saved(Era::Win2000)), (Choice::plain(), forest.clone(), tulip));
        // A live test's choice shows, but is never the look's own.
        *TEST.lock().unwrap() = Some((Era::Win98, Choice { pattern: Some("Bricks".into()), ..Choice::plain() }));
        assert_eq!(choice(Era::Win98).pattern.as_deref(), Some("Bricks"));
        assert_eq!(saved(Era::Win98), forest);
        // The person's Apply or OK takes the test's choice away (the review of 2026-11-21, 7)...
        put_in_effect(Era::Win98, forest.clone());
        assert!(TEST.lock().unwrap().is_none());
        assert_eq!(choice(Era::Win98), forest);
        // ...and so does a change of look.
        *TEST.lock().unwrap() = Some((Era::Win98, Choice { pattern: Some("Bricks".into()), ..Choice::plain() }));
        look_changed();
        assert!(TEST.lock().unwrap().is_none());
        // An unreadable file is kept as it is and every look is plain.
        std::fs::write(&path, [0xFF, 0xFE, 0x41, 0x00]).unwrap();
        load();
        assert_eq!(saved(Era::Win98), Choice::plain());
        assert!(state_file::change(&path, "", |t| display::with_choice(t, Era::Win98, &forest)).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), [0xFF, 0xFE, 0x41, 0x00]);
        let _ = std::fs::remove_file(&path);
    }

    /// A person's own picture that cannot be read at the start (the file gone, the drive not
    /// there): the look stays plain, its line is kept as it is (tried again at the next start),
    /// and Display Properties shows it in the list as the choice.
    #[test]
    fn a_missing_picture_at_the_start_keeps_its_line() {
        let _turn = pictures::TESTS.lock().unwrap_or_else(|e| e.into_inner());
        let path = file();
        let gone = "Z:\\No such drive\\Фото #1=2.v2.jpg";
        let text = format!("# mine\nwin2000 wallpaper = {gone}\nwin2000 display = stretch\nwin2000 pattern = (None)\n");
        std::fs::write(&path, &text).unwrap();
        load();
        let c = saved(Era::Win2000);
        assert_eq!(c.wallpaper.as_deref(), Some(gone));
        let id = c.to_read(Era::Win2000).unwrap();
        pictures::want(&id);
        for _ in 0..500 {
            if !matches!(pictures::state(&id), pictures::State::Reading(_)) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(matches!(pictures::state(&id), pictures::State::Failed(_)));
        // The desktop: plain (no picture, no pattern).
        let theme = bevel_core::theme::theme_in(Era::Win2000, bevel_core::theme::Depth::TrueColor);
        assert!(display::background_on(theme, &c, pictures::ready(&id).as_deref(), &[], 64, 48).is_none());
        // The file is as it was; Display Properties lists the picture by its name, chosen.
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
        let p = DisplayProperties::new(Era::Win2000, c.clone());
        assert_eq!(p.shown.wallpaper.as_deref(), Some(gone));
        assert!(!p.changed());
        pictures::keep_only(&[]);
        let _ = std::fs::remove_file(&path);
    }
}
