//! The system About box and SOL's post-game `MessageBox(..., MB_ICONEXCLAMATION | MB_YESNO)`.

use crate::dialog;
use crate::fb::{Framebuffer, Rect};
use crate::kit::{Effect, Form, Key, Mods, Press};
use crate::shell_about;
use crate::text::Fonts;
use crate::theme::{Era, Theme};
use crate::window::{self, ButtonLook, Frame, TitleButton};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Ok,
    Yes,
    No,
    Close,
}

enum Contents {
    About { memory: String },
    DealAgain,
}

pub struct Sheet {
    era: Era,
    contents: Contents,
    focus: Part,
    pressed: Option<Part>,
    active: bool,
    keys: bool,
}

impl Sheet {
    pub fn about(era: Era, physical_kib: Option<u64>) -> Self {
        Self::new(era, Contents::About { memory: shell_about::memory_text(physical_kib) }, Part::Ok)
    }

    pub fn deal_again(era: Era) -> Self {
        Self::new(era, Contents::DealAgain, Part::Yes)
    }

    fn new(era: Era, contents: Contents, focus: Part) -> Self {
        Self { era, contents, focus, pressed: None, active: true, keys: era.always_shows_keys() }
    }

    fn buttons(&self) -> &'static [(Part, &'static str)] {
        match self.contents {
            Contents::About { .. } => &[(Part::Ok, "OK")],
            Contents::DealAgain => &[(Part::Yes, "&Yes"), (Part::No, "&No")],
        }
    }
}

impl Form for Sheet {
    type Part = Part;

    fn title(&self) -> &str {
        match self.contents {
            Contents::About { .. } => "About Solitaire",
            Contents::DealAgain => "Solitaire",
        }
    }
    fn size(&self) -> (i32, i32) {
        match self.contents {
            Contents::About { .. } => shell_about::size(self.era),
            Contents::DealAgain => (185, 119),
        }
    }
    fn layout(&mut self, _: &Fonts) {}
    fn paint(&self, fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts) {
        let close = ButtonLook {
            button: TitleButton::Close,
            rect: window::title_button_rect(self.size().0, TitleButton::Close),
            pressed: self.pressed == Some(Part::Close),
            enabled: self.enabled(Part::Close),
        };
        window::paint_frame(
            fb,
            theme,
            fonts,
            &Frame { era: self.era, title: self.title(), active: self.active, icon: None, buttons: &[close] },
        );
        match &self.contents {
            Contents::About { memory } => shell_about::paint(
                fb,
                self.era,
                theme,
                fonts.regular,
                shell_about::App {
                    name: "Solitaire",
                    credit: "Developed for Microsoft by Wes Cherry",
                    icon: &super::art::icon(self.era, 32),
                    memory,
                },
            ),
            Contents::DealAgain => {
                fb.draw_icon(&crate::icons::WARNING.icon(), 14, 33);
                fonts.regular.draw(
                    fb,
                    if self.era == Era::Win2000 { 65 } else { 62 },
                    42,
                    "Deal Again?",
                    theme.colors.text,
                );
            }
        }
        for &(part, label) in self.buttons() {
            let focused = self.focus == part;
            dialog::push_button(
                fb,
                theme,
                fonts.regular,
                self.rect(part).unwrap(),
                label,
                focused,
                self.pressed == Some(part),
                self.active && self.keys && focused,
                true,
                self.keys,
            );
        }
    }
    fn rect(&self, part: Part) -> Option<Rect> {
        match (&self.contents, part) {
            (_, Part::Close) => Some(window::title_button_rect(self.size().0, TitleButton::Close)),
            (Contents::About { .. }, Part::Ok) => Some(shell_about::button(self.era)),
            (Contents::DealAgain, Part::Yes) => Some(Rect::new(14, 82, 75, 23)),
            (Contents::DealAgain, Part::No) => Some(Rect::new(95, 82, 75, 23)),
            _ => None,
        }
    }
    fn hit(&self, x: i32, y: i32) -> Option<Part> {
        self.buttons()
            .iter()
            .map(|&(p, _)| p)
            .chain([Part::Close])
            .find(|&p| self.rect(p).is_some_and(|r| r.contains(x, y)))
    }
    fn enabled(&self, part: Part) -> bool {
        !(matches!(self.contents, Contents::DealAgain) && part == Part::Close)
    }
    fn focus(&self) -> Part {
        self.focus
    }
    fn set_focus(&mut self, part: Part) {
        if self.buttons().iter().any(|&(p, _)| p == part) {
            self.focus = part;
        }
    }
    fn tab_stops(&self) -> Vec<Part> {
        self.buttons().iter().map(|&(p, _)| p).collect()
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
        self.focus
    }
    fn escape(&self) -> Part {
        Part::Close
    }
    fn key(&mut self, key: Key, _: Mods) -> Option<Vec<Effect<Part>>> {
        if matches!(self.contents, Contents::DealAgain) && matches!(key, Key::Left | Key::Right | Key::Up | Key::Down) {
            self.focus = if self.focus == Part::Yes { Part::No } else { Part::Yes };
            Some(Vec::new())
        } else {
            None
        }
    }
    fn access(&mut self, ch: char) -> Vec<Effect<Part>> {
        self.buttons()
            .iter()
            .find_map(|&(part, label)| {
                label
                    .strip_prefix('&')
                    .filter(|text| text.starts_with(ch.to_ascii_uppercase()))
                    .map(|_| vec![Effect::Activate(part)])
            })
            .unwrap_or_default()
    }
    fn typed(&mut self, ch: char) -> Vec<Effect<Part>> {
        self.access(ch)
    }
    fn space(&mut self) -> Vec<Effect<Part>> {
        vec![Effect::Activate(self.focus)]
    }
    fn press(&mut self, part: Option<Part>, _: i32, _: i32) -> Press<Part> {
        let Some(part) = part else { return Press::Nothing };
        if !self.enabled(part) {
            return Press::Done;
        }
        self.set_focus(part);
        Press::Hold(part)
    }
}
