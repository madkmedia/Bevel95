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

/// "Version 0.1.0 (build 1a2b3c4)"; `build` is "unknown" for a build made without git.
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

pub struct AboutWindow {
    pub era: Era,
    version: String,
    /// The notice, wrapped to the window's width (`layout`).
    notice: Vec<String>,
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
        dialog::push_button(
            fb,
            theme,
            font,
            self.button(),
            "OK",
            true,
            self.pressed == Some(Part::Ok),
            self.active && self.keys,
            true,
            self.keys,
        );
    }
    fn rect(&self, part: Part) -> Option<Rect> {
        Some(match part {
            Part::Ok => self.button(),
            Part::Close => window::title_button_rect(self.size().0, TitleButton::Close),
        })
    }
    fn hit(&self, x: i32, y: i32) -> Option<Part> {
        [Part::Ok, Part::Close].into_iter().find(|&p| self.rect(p).is_some_and(|r| r.contains(x, y)))
    }
    fn enabled(&self, _: Part) -> bool {
        true
    }
    fn focus(&self) -> Part {
        Part::Ok
    }
    fn set_focus(&mut self, _: Part) {}
    fn tab_stops(&self) -> Vec<Part> {
        vec![Part::Ok]
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
    fn enter(&self) -> Part {
        Part::Ok
    }
    fn escape(&self) -> Part {
        Part::Close
    }
    fn access(&mut self, _: char) -> Vec<Effect<Part>> {
        Vec::new()
    }
    fn space(&mut self) -> Vec<Effect<Part>> {
        vec![Effect::Activate(Part::Ok)]
    }
    fn press(&mut self, part: Option<Part>, _: i32, _: i32) -> Press<Part> {
        match part {
            Some(part) => Press::Hold(part),
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
        assert_eq!(VERSION, "0.1.0", "the workspace's version");
        assert_eq!(version_line("1a2b3c4"), "Version 0.1.0 (build 1a2b3c4)");
        assert_eq!(version_line("unknown"), "Version 0.1.0 (build unknown)");
        assert_eq!(log_line("bevel95.exe", "1a2b3c4"), "Bevel95 0.1.0 (build 1a2b3c4): bevel95.exe starting");
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
}
