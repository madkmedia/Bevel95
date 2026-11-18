//! About Calculator: the shell's About box (ShellAbout), as the games have it. Windows 95's
//! Calculator passed its author's credit, which the box wraps onto two lines; 98 and 2000 passed
//! none.

use crate::dialog;
use crate::fb::{Framebuffer, Rect};
use crate::kit::{Effect, Form, Press};
use crate::shell_about;
use crate::text::Fonts;
use crate::theme::{Era, Theme};
use crate::window::{self, ButtonLook, Frame, TitleButton};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Ok,
    Close,
}

pub struct About {
    pub era: Era,
    memory: String,
    pressed: Option<Part>,
    active: bool,
    keys: bool,
}

impl About {
    /// `physical_kib`: the memory Windows reports (None: "N/A").
    pub fn new(era: Era, physical_kib: Option<u64>) -> About {
        About { era, memory: shell_about::memory_text(physical_kib), pressed: None, active: true, keys: era.always_shows_keys() }
    }

    fn credit(&self) -> &'static str {
        if self.era == Era::Win95 { "Developed for Microsoft\nby Kraig Brockschmidt" } else { "" }
    }
}

impl Form for About {
    type Part = Part;

    fn title(&self) -> &str {
        "About Calculator"
    }
    fn size(&self) -> (i32, i32) {
        shell_about::size(self.era)
    }
    fn layout(&mut self, _: &Fonts) {}
    fn paint(&self, fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts) {
        let close = ButtonLook {
            button: TitleButton::Close,
            rect: window::title_button_rect(self.size().0, TitleButton::Close),
            pressed: self.pressed == Some(Part::Close),
            enabled: true,
        };
        let frame = Frame { era: self.era, title: self.title(), active: self.active, icon: None, buttons: &[close] };
        window::paint_frame(fb, theme, fonts, &frame);
        let icon = super::art::icon(self.era, 32);
        let app = shell_about::App { name: "Calculator", credit: self.credit(), icon: &icon, memory: &self.memory };
        shell_about::paint(fb, self.era, theme, fonts.regular, app);
        dialog::push_button(
            fb,
            theme,
            fonts.regular,
            shell_about::button(self.era),
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
            Part::Ok => shell_about::button(self.era),
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
