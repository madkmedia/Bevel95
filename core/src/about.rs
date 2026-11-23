//! Bevel95's name and version, and its About window (the bar's Bevel95 > About Bevel95, Quick
//! Search's "About Bevel95").
//!
//! The name lives in ONE place, `product_name!` (the owner is thinking it over): the About window,
//! the log's first line and Setup take it from here. The version is the workspace's
//! (`[workspace.package] version` in Cargo.toml); the build (the commit it was made from) comes
//! from the Windows side, which asks git when it is built (`bevel_winkit::BUILD`), so this module
//! takes it as text.
//!
//! The window is built like the programs' About boxes (the shell's About, `shell_about`): the
//! look's own frame and font, the picture at the left, the lines beside it, an etched line and an
//! OK button. Windows 2000's About boxes carried Microsoft's Windows 2000 banner; Bevel95's does
//! not (it is not Microsoft's), so the three looks share the 95 and 98 layout.

use crate::colors16;
use crate::dialog;
use crate::fb::{Framebuffer, Rect};
use crate::kit::{Effect, Form, Press};
use crate::text::Fonts;
use crate::theme::{Era, Theme};
use crate::window::{self, ButtonLook, Frame, TitleButton};

/// The product's name as a literal, for `concat!` (Setup's labels); `NAME` for everything else.
#[macro_export]
macro_rules! product_name {
    () => {
        "Bevel95"
    };
}

/// The product's name (PROVISIONAL: the owner is thinking the name over).
pub const NAME: &str = product_name!();
/// The version of the whole project: the workspace's.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// What it is, in one line (PROVISIONAL wording).
pub const WHAT: &str = "A Windows 95, 98 and 2000 look for today's Windows";
/// The notice (PROVISIONAL wording, until the owner has read it).
pub const NOTICE: &str = "Not affiliated with Microsoft. Windows is a trademark of Microsoft Corporation; \
                          the pictures of its user interface belong to Microsoft.";

/// The page that says how to support Bevel95: the README's section, which can change without a
/// new release. The About window's Support button asks Windows to open it, and nothing else
/// does: nothing asks by itself, nothing is counted or stored.
pub const SUPPORT_URL: &str = "https://github.com/madkmedia/Bevel95#supporting-bevel95";
/// The About window's button that opens `SUPPORT_URL` (PROVISIONAL words).
pub const SUPPORT_LABEL: &str = concat!("&Support ", product_name!(), "...");

/// The support page is asked for at most once within this time, whatever is held down: a held
/// Enter, Space or Alt+S repeats about 30 times a second (the review before 0.1.3, B7).
pub const SUPPORT_AGAIN_AFTER_MS: u64 = 1_000;

/// When the Support button last asked for the page (`now_ms` from any clock that only goes
/// forward).
#[derive(Clone, Copy, Debug, Default)]
pub struct SupportAsks {
    last: Option<u64>,
}

impl SupportAsks {
    /// Whether the page may be asked for at `now_ms`; when it may, that time is remembered.
    pub fn ask(&mut self, now_ms: u64) -> bool {
        if self.last.is_some_and(|last| now_ms.saturating_sub(last) < SUPPORT_AGAIN_AFTER_MS) {
            return false;
        }
        self.last = Some(now_ms);
        true
    }
}

/// "Version 0.1.5 (build 1a2b3c4)"; `build` is "unknown" for a build made without git.
pub fn version_line(build: &str) -> String {
    format!("Version {VERSION} (build {build})")
}

/// The log's first line when a program of Bevel95 starts (`program`: its file name).
pub fn log_line(program: &str, build: &str) -> String {
    format!("{NAME} {VERSION} (build {build}): {program} starting")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Ok,
    /// Support Bevel95...: opens `SUPPORT_URL`; the window stays open.
    Support,
    Close,
}

/// Window width at 1x, in every look (the shell's About is 348 wide; the line of what it is
/// needs a little more).
const WIDTH: i32 = 380;
/// The text's left edge (the shell's About: 52 dialog units) and the picture's corner (7, 7).
const TEXT_X: i32 = window::FRAME + 78;
const ICON: (i32, i32) = (window::FRAME + 11, window::CLIENT_Y + 12);
/// The first line's top, and the step between lines (8 dialog units, 13 pixels).
const FIRST_Y: i32 = window::CLIENT_Y + 18;
const LINE: i32 = 13;
const BUTTON: (i32, i32) = (75, 23);
/// The space between two buttons of a row (Run's OK and Cancel).
const BUTTON_GAP: i32 = 6;

pub struct AboutWindow {
    pub era: Era,
    version: String,
    /// The notice, wrapped to the window's width (`layout`).
    notice: Vec<String>,
    /// The Support button's width: its label plus 12 on each side, at least Windows' usual 75
    /// (as Date/Time Properties' wide button; measured in the look's font by `layout`).
    support_w: i32,
    /// The button with the keyboard: OK when it opens, Support after Tab or a click on it.
    focus: Part,
    pressed: Option<Part>,
    active: bool,
    keys: bool,
}

impl AboutWindow {
    /// `build`: the commit it was built from ("unknown" without git).
    pub fn new(era: Era, build: &str) -> AboutWindow {
        AboutWindow {
            era,
            version: version_line(build),
            notice: Vec::new(),
            support_w: BUTTON.0,
            focus: Part::Ok,
            pressed: None,
            active: true,
            keys: era.always_shows_keys(),
        }
    }

    /// The etched line between what it is and the notice.
    fn separator_y(&self) -> i32 {
        FIRST_Y + 3 * LINE + 10
    }

    fn notice_y(&self) -> i32 {
        self.separator_y() + 10
    }

    fn button(&self) -> Rect {
        let y = self.notice_y() + LINE * self.notice.len().max(1) as i32 + 10;
        Rect::new(WIDTH - window::FRAME - 11 - BUTTON.0, y, BUTTON.0, BUTTON.1)
    }

    /// Support Bevel95...: in OK's row, left of it.
    fn support_button(&self) -> Rect {
        let ok = self.button();
        Rect::new(ok.x - BUTTON_GAP - self.support_w, ok.y, self.support_w, BUTTON.1)
    }
}

impl Form for AboutWindow {
    type Part = Part;

    fn title(&self) -> &str {
        concat!("About ", product_name!())
    }
    fn size(&self) -> (i32, i32) {
        (WIDTH, self.button().bottom() + 11 + window::FRAME)
    }
    fn layout(&mut self, fonts: &Fonts) {
        self.notice = dialog::wrap(fonts.regular, NOTICE, WIDTH - TEXT_X - window::FRAME - 11);
        self.support_w = (fonts.regular.measure_label(SUPPORT_LABEL) + 24).max(BUTTON.0);
    }
    fn paint(&self, fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts) {
        let c = &theme.colors;
        let close = ButtonLook {
            button: TitleButton::Close,
            rect: window::title_button_rect(self.size().0, TitleButton::Close),
            pressed: self.pressed == Some(Part::Close),
            enabled: true,
        };
        let frame = Frame { era: self.era, title: self.title(), active: self.active, icon: None, buttons: &[close] };
        window::paint_frame(fb, theme, fonts, &frame);
        colors16::draw_picture(fb, theme, &crate::bevel_icon::icon(32), ICON.0, ICON.1);
        let font = fonts.regular;
        for (i, line) in [NAME, self.version.as_str(), WHAT].into_iter().enumerate() {
            font.draw(fb, TEXT_X, FIRST_Y + LINE * i as i32, line, c.text);
        }
        fb.frame(Rect::new(TEXT_X, self.separator_y(), WIDTH - TEXT_X - window::FRAME - 11, 2), c.shadow, c.highlight);
        for (i, line) in self.notice.iter().enumerate() {
            font.draw(fb, TEXT_X, self.notice_y() + LINE * i as i32, line, c.text);
        }
        // The focused button has the default frame (Windows' rule; OK when the window opens).
        for (part, rect, label) in [(Part::Support, self.support_button(), SUPPORT_LABEL), (Part::Ok, self.button(), "OK")] {
            dialog::push_button(
                fb,
                theme,
                font,
                rect,
                label,
                self.focus == part,
                self.pressed == Some(part),
                self.focus == part && self.active && self.keys,
                true,
                self.keys,
            );
        }
    }
    fn rect(&self, part: Part) -> Option<Rect> {
        Some(match part {
            Part::Ok => self.button(),
            Part::Support => self.support_button(),
            Part::Close => window::title_button_rect(self.size().0, TitleButton::Close),
        })
    }
    fn hit(&self, x: i32, y: i32) -> Option<Part> {
        [Part::Ok, Part::Support, Part::Close].into_iter().find(|&p| self.rect(p).is_some_and(|r| r.contains(x, y)))
    }
    fn enabled(&self, _: Part) -> bool {
        true
    }
    fn focus(&self) -> Part {
        self.focus
    }
    fn set_focus(&mut self, part: Part) {
        if part != Part::Close {
            self.focus = part;
        }
    }
    /// Left to right, as the buttons stand.
    fn tab_stops(&self) -> Vec<Part> {
        vec![Part::Support, Part::Ok]
    }
    fn set_pressed(&mut self, part: Option<Part>) {
        self.pressed = part;
    }
    fn set_active(&mut self, active: bool) {
        self.active = active;
    }
    fn show_keys(&mut self) {
        self.keys = true;
    }
    /// The focused button: OK, unless Tab or a click gave Support the keyboard.
    fn enter(&self) -> Part {
        self.focus
    }
    fn escape(&self) -> Part {
        Part::Close
    }
    /// Alt+S: Support Bevel95... (OK has no access key).
    fn access(&mut self, ch: char) -> Vec<Effect<Part>> {
        if ch.eq_ignore_ascii_case(&'s') { vec![Effect::Activate(Part::Support)] } else { Vec::new() }
    }
    fn space(&mut self) -> Vec<Effect<Part>> {
        vec![Effect::Activate(self.focus)]
    }
    fn press(&mut self, part: Option<Part>, _: i32, _: i32) -> Press<Part> {
        match part {
            Some(part) => {
                // A button pressed gets the keyboard, as in Windows' dialogs.
                self.set_focus(part);
                Press::Hold(part)
            }
            None => Press::Nothing,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_and_version_come_from_one_place() {
        assert_eq!(NAME, "Bevel95");
        assert_eq!(VERSION, "0.1.5", "the workspace's version");
        assert_eq!(version_line("1a2b3c4"), "Version 0.1.5 (build 1a2b3c4)");
        assert_eq!(version_line("unknown"), "Version 0.1.5 (build unknown)");
        assert_eq!(log_line("bevel95.exe", "1a2b3c4"), "Bevel95 0.1.5 (build 1a2b3c4): bevel95.exe starting");
        assert_eq!(AboutWindow::new(Era::Win98, "x").title(), "About Bevel95");
    }

    #[test]
    fn the_ok_button_follows_the_notice() {
        let mut w = AboutWindow::new(Era::Win95, "x");
        w.notice = vec!["a".into(), "b".into(), "c".into()];
        let three = w.button();
        w.notice.push("d".into());
        assert_eq!(w.button().y, three.y + LINE);
        assert_eq!(w.size().1, w.button().bottom() + 11 + window::FRAME);
        assert_eq!(w.hit(three.x + 1, three.y + 1 + LINE), Some(Part::Ok));
    }

    #[test]
    fn the_support_page_is_one_fixed_https_address_of_the_project() {
        assert_eq!(SUPPORT_URL, "https://github.com/madkmedia/Bevel95#supporting-bevel95");
        assert!(SUPPORT_URL.starts_with("https://github.com/madkmedia/Bevel95"));
        assert!(!SUPPORT_URL.contains(char::is_whitespace));
        assert_eq!(SUPPORT_LABEL, "&Support Bevel95...");
    }

    #[test]
    fn the_support_page_is_asked_for_at_most_once_a_second() {
        let mut asks = SupportAsks::default();
        assert!(asks.ask(0), "the first press asks");
        // A key held down: about 30 repeats a second; only one ask in each second.
        let asked = (1..=70).map(|i| i * 33).filter(|&t| asks.ask(t)).collect::<Vec<_>>();
        assert_eq!(asked, [1023, 2046], "once a second while held");
        assert!(!asks.ask(2046 + 999));
        assert!(asks.ask(2046 + 1000));
    }

    /// The window in `era`'s look, laid out in its own font.
    fn laid_out(era: Era) -> AboutWindow {
        let font = crate::text::Font::parse(if era == Era::Win2000 {
            include_str!("../tests/fixtures/tahoma-11px.font")
        } else {
            include_str!("../tests/fixtures/ms-sans-serif-11px.font")
        })
        .unwrap();
        let mut w = AboutWindow::new(era, "x");
        w.layout(&Fonts { regular: &font, bold: &font });
        w
    }

    #[test]
    fn support_stands_left_of_ok_in_every_look() {
        for era in Era::ALL {
            let w = laid_out(era);
            let (ok, support) = (w.button(), w.support_button());
            assert_eq!(support.y, ok.y, "{era:?}: one row");
            assert_eq!((support.h, ok.w), (23, 75), "{era:?}");
            assert_eq!(support.right() + BUTTON_GAP, ok.x, "{era:?}: left of OK");
            assert!(support.w > 75 && support.x > window::FRAME, "{era:?}: wider for its words, inside the window");
            assert_eq!(w.hit(support.x + 2, support.y + 2), Some(Part::Support), "{era:?}");
            assert_eq!(w.tab_stops(), [Part::Support, Part::Ok]);
        }
    }

    #[test]
    fn support_opens_the_page_and_ok_stays_the_default() {
        use crate::kit::{Controller, Key, Mods};
        for era in Era::ALL {
            let mut w = laid_out(era);
            let mut c = Controller::default();
            // As it opens, Enter and Esc do what they did: OK and the title bar's X.
            assert_eq!(w.focus(), Part::Ok);
            assert_eq!(c.key(&mut w, Key::Enter, Mods::default()), [Effect::Activate(Part::Ok)]);
            assert_eq!(c.key(&mut w, Key::Escape, Mods::default()), [Effect::Activate(Part::Close)]);
            // Alt+S: the one effect, the support button; nothing closes, the focus stays on OK.
            assert_eq!(c.access(&mut w, 's'), [Effect::Activate(Part::Support)], "{era:?}");
            assert_eq!(c.access(&mut w, 'S'), [Effect::Activate(Part::Support)], "{era:?}");
            assert!(c.access(&mut w, 'o').is_empty() && c.access(&mut w, 'x').is_empty());
            assert_eq!(w.focus(), Part::Ok);
            // A click on it: held, then carried out once on release; nothing else.
            let r = w.support_button();
            assert_eq!(c.mouse_down(&mut w, r.x + 5, r.y + 5), [Effect::Capture]);
            assert_eq!(c.mouse_up(&mut w, r.x + 5, r.y + 5), [Effect::Release, Effect::Activate(Part::Support)]);
            // Escape still closes after it.
            assert_eq!(c.key(&mut w, Key::Escape, Mods::default()), [Effect::Activate(Part::Close)]);
            // Tab goes between the two buttons.
            let mut w = laid_out(era);
            c.key(&mut w, Key::Tab, Mods::default());
            assert_eq!(w.focus(), Part::Support);
            assert_eq!(c.key(&mut w, Key::Space, Mods::default()), [Effect::Activate(Part::Support)]);
            c.key(&mut w, Key::Tab, Mods::default());
            assert_eq!(w.focus(), Part::Ok);
            assert_eq!(c.key(&mut w, Key::Space, Mods::default()), [Effect::Activate(Part::Ok)]);
        }
    }
}
