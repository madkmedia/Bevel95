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
use bevel_core::display::{self, Choice};
use bevel_core::theme::Era;
use bevel_winkit::state_file;

use crate::bar::{self, with_app};
use crate::kit::{self, Hosted};
use crate::log::{data_dir, log, path_kind};
use crate::util::take_focus;

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
    let path = file();
    if let Err(e) = state_file::change(&path, "", |text| display::with_choice(text, era, &choice))
        && !state_file::is_read_only(&e)
    {
        // The choice holds for this run (the desktop shows it); the file stays as it was.
        log(&format!("could not save the wallpaper and pattern to {}: {e}", path_kind(&path)));
    }
    // Windows' own names, Bevel95's data: no text from outside Bevel95.
    log(&format!(
        "display: {} wallpaper {}, {}, pattern {}",
        era.title(),
        choice.wallpaper.as_deref().unwrap_or("(None)"),
        choice.show.word(),
        choice.pattern.as_deref().unwrap_or("(None)")
    ));
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
    }
}

/// Closes Display Properties and its Pattern window, applying nothing.
pub fn close() {
    kit::close::<PatternHost>();
    kit::close::<DisplayHost>();
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
/// person could (98 and 2000, Pattern... not grayed).
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
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevel_core::display::Show;

    #[test]
    fn a_choice_is_saved_for_its_look_only_and_read_back() {
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
}
