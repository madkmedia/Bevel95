//! The question before the desktop deletes: each look's own "Confirm File Delete", "Confirm
//! Folder Delete" and "Confirm Multiple File Delete" windows (Explorer's, in Windows 95, 98 and
//! 2000 alike apart from the colors, fonts and pictures), measured on the Reference PCs
//! (`private/reference/refs/vm/<era>-truecolor-item-confirm-*`, numbers in docs/theme-spec.md, "Confirming a
//! delete"). Today's Windows asks nothing when a file goes to the Recycle Bin, so Bevel95 asks
//! itself; only after Yes does the platform hand the files to Windows' file machinery.
//!
//! Three window layouts, as Explorer had three dialog templates: one item (a file or a
//! shortcut, and emptying a Recycle Bin that holds one item), a folder (taller, its text 5
//! pixels further left), several items (its line lower, its buttons wider). Yes has the focus
//! and is the default; No, Esc and the title bar's X answer no.

use crate::art::{self, Look};
use crate::dialog;
use crate::fb::{Framebuffer, Icon, Rect};
use crate::kit::{Effect, Form, Key, Mods, Press};
use crate::text::Fonts;
use crate::theme::{Era, Theme};
use crate::window::{self, ButtonLook, Frame, TitleButton};

/// What is about to be deleted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Question {
    /// One file or shortcut to the Recycle Bin (its name as the desktop shows it).
    File(String),
    /// One folder to the Recycle Bin.
    Folder(String),
    /// Several items to the Recycle Bin.
    Several(usize),
    /// Emptying a Recycle Bin that holds this one item: deleted for good.
    EmptyOne(String),
    /// Emptying a Recycle Bin that holds this many items.
    EmptySeveral(usize),
}

impl Question {
    pub fn title(&self) -> &'static str {
        match self {
            Question::File(_) | Question::EmptyOne(_) => "Confirm File Delete",
            Question::Folder(_) => "Confirm Folder Delete",
            Question::Several(_) | Question::EmptySeveral(_) => "Confirm Multiple File Delete",
        }
    }

    /// The question, word for word as the Reference PCs asked it.
    pub fn text(&self) -> String {
        match self {
            Question::File(name) => format!("Are you sure you want to send '{name}' to the Recycle Bin?"),
            Question::Folder(name) => {
                format!(
                    "Are you sure you want to remove the folder '{name}' and move all its contents to the Recycle Bin?"
                )
            }
            Question::Several(n) => format!("Are you sure you want to send these {n} items to the Recycle Bin?"),
            Question::EmptyOne(name) => format!("Are you sure you want to delete '{name}'?"),
            Question::EmptySeveral(n) => format!("Are you sure you want to delete these {n} items?"),
        }
    }

    /// Which of the four pictures (`icons.rgb`, left to right): to the Recycle Bin a file, a
    /// folder, several; deleted for good.
    fn picture(&self) -> usize {
        match self {
            Question::File(_) => 0,
            Question::Folder(_) => 1,
            Question::Several(_) => 2,
            Question::EmptyOne(_) | Question::EmptySeveral(_) => 3,
        }
    }

    fn layout(&self) -> &'static Layout {
        match self {
            Question::File(_) | Question::EmptyOne(_) => &ONE,
            Question::Folder(_) => &FOLDER,
            Question::Several(_) | Question::EmptySeveral(_) => &SEVERAL,
        }
    }
}

/// One of Explorer's three dialog templates, in window pixels (frame included).
struct Layout {
    size: (i32, i32),
    /// The text's first line (its 13-pixel row) and where lines wrap.
    text: (i32, i32),
    text_right: i32,
    yes: Rect,
    no: Rect,
}

const ICON_AT: (i32, i32) = (18, 38);
const ONE: Layout = Layout {
    size: (419, 133),
    text: (65, 38),
    text_right: 400,
    yes: Rect::new(239, 96, 80, 23),
    no: Rect::new(326, 96, 80, 23),
};
const FOLDER: Layout = Layout {
    size: (419, 152),
    text: (60, 38),
    text_right: 400,
    yes: Rect::new(239, 115, 80, 23),
    no: Rect::new(326, 115, 80, 23),
};
const SEVERAL: Layout = Layout {
    size: (419, 135),
    text: (65, 45),
    text_right: 400,
    yes: Rect::new(224, 96, 87, 23),
    no: Rect::new(318, 96, 87, 23),
};

const ICONS_95: &[u8] = art::bytes("confirm/delete-icons", Look::Win95);
const ICONS_98: &[u8] = art::bytes("confirm/delete-icons", Look::Win98);
const ICONS_2000: &[u8] = art::bytes("confirm/delete-icons", Look::Win2000);

/// Picture `k` of the look's four (cut from the Reference PCs over the dialog's face color).
fn picture(era: Era, k: usize) -> Icon {
    let bytes = match era {
        Era::Win95 => ICONS_95,
        Era::Win98 => ICONS_98,
        Era::Win2000 => ICONS_2000,
    };
    let mut pixels = Vec::with_capacity(32 * 32);
    for y in 0..32 {
        for x in 0..32 {
            let i = (y * 128 + k * 32 + x) * 3;
            pixels
                .push(0xFF00_0000 | u32::from(bytes[i]) << 16 | u32::from(bytes[i + 1]) << 8 | u32::from(bytes[i + 2]));
        }
    }
    Icon { width: 32, height: 32, pixels }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Yes,
    No,
    Close,
}

/// The window: the question, Yes and No.
pub struct ConfirmWindow {
    pub era: Era,
    pub question: Question,
    focus: Part,
    pressed: Option<Part>,
    active: bool,
    keys: bool,
}

impl ConfirmWindow {
    /// `by_keyboard`: opened by a key (Del), so Windows 2000 underlines the access keys from the
    /// start (95 and 98 always did).
    pub fn new(era: Era, question: Question, by_keyboard: bool) -> ConfirmWindow {
        ConfirmWindow {
            era,
            question,
            focus: Part::Yes,
            pressed: None,
            active: true,
            keys: era.always_shows_keys() || by_keyboard,
        }
    }
}

impl Form for ConfirmWindow {
    type Part = Part;

    fn title(&self) -> &str {
        self.question.title()
    }
    fn size(&self) -> (i32, i32) {
        self.question.layout().size
    }
    fn layout(&mut self, _: &Fonts) {}
    fn paint(&self, fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts) {
        let l = self.question.layout();
        let close = ButtonLook {
            button: TitleButton::Close,
            rect: window::title_button_rect(l.size.0, TitleButton::Close),
            pressed: self.pressed == Some(Part::Close),
            enabled: true,
        };
        window::paint_frame(
            fb,
            theme,
            fonts,
            &Frame { era: self.era, title: self.title(), active: self.active, icon: None, buttons: &[close] },
        );
        crate::colors16::draw_picture(fb, theme, &picture(self.era, self.question.picture()), ICON_AT.0, ICON_AT.1);
        let font = fonts.regular;
        for (n, line) in dialog::wrap(font, &self.question.text(), l.text_right - l.text.0).iter().enumerate() {
            font.draw(fb, l.text.0, l.text.1 + 13 * n as i32, line, theme.colors.text);
        }
        for (part, label) in [(Part::Yes, "&Yes"), (Part::No, "&No")] {
            let focused = self.focus == part;
            dialog::push_button(
                fb,
                theme,
                font,
                self.rect(part).unwrap_or_default(),
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
        let l = self.question.layout();
        Some(match part {
            Part::Yes => l.yes,
            Part::No => l.no,
            Part::Close => window::title_button_rect(l.size.0, TitleButton::Close),
        })
    }
    fn hit(&self, x: i32, y: i32) -> Option<Part> {
        [Part::Yes, Part::No, Part::Close].into_iter().find(|&p| self.rect(p).is_some_and(|r| r.contains(x, y)))
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
    fn tab_stops(&self) -> Vec<Part> {
        vec![Part::Yes, Part::No]
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
        Part::No
    }
    fn key(&mut self, key: Key, _: Mods) -> Option<Vec<Effect<Part>>> {
        // The arrows go from button to button, as in Windows' dialogs.
        if matches!(key, Key::Left | Key::Right | Key::Up | Key::Down) {
            self.focus = if self.focus == Part::Yes { Part::No } else { Part::Yes };
            Some(Vec::new())
        } else {
            None
        }
    }
    fn access(&mut self, ch: char) -> Vec<Effect<Part>> {
        match ch.to_ascii_lowercase() {
            'y' => vec![Effect::Activate(Part::Yes)],
            'n' => vec![Effect::Activate(Part::No)],
            _ => Vec::new(),
        }
    }
    fn typed(&mut self, ch: char) -> Vec<Effect<Part>> {
        self.access(ch)
    }
    fn space(&mut self) -> Vec<Effect<Part>> {
        vec![Effect::Activate(self.focus)]
    }
    fn press(&mut self, part: Option<Part>, _: i32, _: i32) -> Press<Part> {
        let Some(part) = part else { return Press::Nothing };
        self.set_focus(part);
        Press::Hold(part)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_question_has_its_windows_title_and_words() {
        let q = Question::File("Test text".into());
        assert_eq!(q.title(), "Confirm File Delete");
        assert_eq!(q.text(), "Are you sure you want to send 'Test text' to the Recycle Bin?");
        assert_eq!(Question::Folder("A".into()).title(), "Confirm Folder Delete");
        assert_eq!(
            Question::Folder("A".into()).text(),
            "Are you sure you want to remove the folder 'A' and move all its contents to the Recycle Bin?"
        );
        assert_eq!(Question::Several(2).title(), "Confirm Multiple File Delete");
        assert_eq!(Question::Several(2).text(), "Are you sure you want to send these 2 items to the Recycle Bin?");
        assert_eq!(Question::EmptyOne("x".into()).title(), "Confirm File Delete");
        assert_eq!(Question::EmptyOne("x".into()).text(), "Are you sure you want to delete 'x'?");
        assert_eq!(Question::EmptySeveral(3).text(), "Are you sure you want to delete these 3 items?");
    }

    #[test]
    fn yes_has_the_focus_and_no_is_the_way_out() {
        for era in Era::ALL {
            let mut w = ConfirmWindow::new(era, Question::Several(2), false);
            assert_eq!(w.focus(), Part::Yes, "{era:?}: the Reference PCs opened with Yes focused");
            assert_eq!(w.enter(), Part::Yes);
            assert_eq!(w.escape(), Part::No);
            assert_eq!(w.access('n'), vec![Effect::Activate(Part::No)]);
            assert_eq!(w.access('Y'), vec![Effect::Activate(Part::Yes)]);
            w.key(Key::Right, Mods::default());
            assert_eq!(w.enter(), Part::No);
            assert_eq!(w.keys, era != Era::Win2000, "2000 underlines once a key was used");
        }
    }

    #[test]
    fn the_three_layouts_are_the_reference_pcs() {
        let size = |q: Question| ConfirmWindow::new(Era::Win98, q, true).size();
        assert_eq!(size(Question::File("a".into())), (419, 133));
        assert_eq!(size(Question::Folder("a".into())), (419, 152));
        assert_eq!(size(Question::EmptySeveral(4)), (419, 135));
        let w = ConfirmWindow::new(Era::Win95, Question::Several(2), true);
        assert_eq!(w.rect(Part::No), Some(Rect::new(318, 96, 87, 23)));
        assert_eq!(w.hit(320, 100), Some(Part::No));
        for era in Era::ALL {
            for k in 0..4 {
                assert_eq!(picture(era, k).pixels.len(), 32 * 32);
            }
        }
        assert_eq!(ICONS_95.len(), 128 * 32 * 3);
    }
}
