//! What a folder window says when a drive cannot be read (a floppy or CD drive without a disk),
//! each look its own (`*-truecolor-tree-floppy-error`):
//!
//! - Windows 95 and 98: a message box titled as the window, the error picture, "A:\ is not
//!   accessible." and "The device is not ready." with an empty line between, Retry (the default)
//!   and Cancel. The same box as the extension warning's in every measure (`rename_warning`): the
//!   picture at 14, 33, the text from 62, 33 in lines of 13, buttons 75 x 23 at y 89, 6 apart.
//!   189 wide for "A:", 126 high. (98's capture showed an unformatted disk's question instead:
//!   its PC had a disk in A:; the not-ready box is 95's, in 98's colors.)
//! - Windows 2000: the shell's own "Insert disk" box, 347 x 137: the drive's picture, "Please
//!   insert a disk into drive A:." from 108, Cancel alone (the default) at 257, 98. Its picture's
//!   place and the text's row are read off one capture (PROVISIONAL within a pixel or two).
//!
//! Retry reads the drive again; Cancel (and Esc) leave it, and the tree goes back to the place
//! shown.

use crate::dialog;
use crate::fb::{Framebuffer, Icon, Rect};
use crate::icons::ClassicIcon;
use crate::kit::{Effect, Form, Key, Mods, Press};
use crate::text::Fonts;
use crate::theme::{Era, Theme};
use crate::window::{self, ButtonLook, Frame, TitleButton};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Retry,
    Cancel,
    Ok,
    Close,
}

/// The box.
#[derive(Clone, Debug)]
pub struct DiskBox {
    pub era: Era,
    title: String,
    lines: Vec<String>,
    /// 2000: the drive's 32 x 32 picture.
    picture: Option<Icon>,
    focus: Part,
    pressed: Option<Part>,
    active: bool,
    keys: bool,
    width: i32,
    /// The address bar's message: OK alone, under the text (USER's own message box).
    address: bool,
}

/// The address bar's message for an address that is no folder and cannot be opened (the
/// Reference PCs, `<era>-truecolor-m17-address-bad-path`): 98 titled with what was typed, 2000
/// "Address Bar"; Windows' own message box: the error picture at 14, 33, the text from 62 in
/// lines of 13 (centered on the picture when shorter), wrapped so the box is at most 5/8 of the
/// screen wide, OK (75 x 23) centered 17 under the taller of picture and text.
const LINE_H: i32 = 13;
const TEXT_X: i32 = 62;
/// The box's width beyond the text's widest line (measured: 98's 494 for its first line).
const TEXT_RIGHT: i32 = 16;

impl DiskBox {
    /// The box for drive `letter` in a window titled `title`; `picture`: the drive's picture
    /// (2000); `by_keyboard`: a key brought it (2000 shows the focus rectangle then); `text_w`:
    /// measures a line in the look's font.
    pub fn new(era: Era, title: &str, letter: char, picture: Option<Icon>, by_keyboard: bool, text_w: impl Fn(&str) -> i32) -> DiskBox {
        let (title, lines) = if era == Era::Win2000 {
            ("Insert disk".to_string(), vec![format!("Please insert a disk into drive {letter}:.")])
        } else {
            (title.to_string(), vec![format!("{letter}:\\ is not accessible."), String::new(), "The device is not ready.".to_string()])
        };
        let width = if era == Era::Win2000 { 347 } else { lines.iter().map(|l| 62 + text_w(l) + 12).max().unwrap_or(0).max(189) };
        let focus = if era == Era::Win2000 { Part::Cancel } else { Part::Retry };
        DiskBox { era, title, lines, picture, focus, pressed: None, active: true, keys: era.always_shows_keys() || by_keyboard, width, address: false }
    }

    /// The address bar's message for `typed` on a screen `screen_w` wide (1×); `font`: the
    /// look's message font.
    pub fn address(era: Era, typed: &str, screen_w: i32, font: &crate::text::Font) -> DiskBox {
        let (title, text) = if era == Era::Win2000 {
            ("Address Bar".to_string(), format!("Cannot find the file or item '{typed}'.  Make sure the path and file name are correct.  Type 'go <SearchText>' to use AutoSearch."))
        } else {
            (typed.to_string(), format!("Cannot find the file or shell item '{typed}'.  Make sure the path and filename are correct.  Enter 'go <SearchString>' to use AutoSearch."))
        };
        DiskBox::message(era, &title, &text, screen_w, font)
    }

    /// Windows' own message box with the error picture, `text` and OK, titled `title` (the address
    /// bar's message; an Undo that cannot be done: 98 "Error Renaming File").
    pub fn message(era: Era, title: &str, text: &str, screen_w: i32, font: &crate::text::Font) -> DiskBox {
        let (title, text) = (title.to_string(), text.to_string());
        let most = screen_w * 5 / 8 - TEXT_X - TEXT_RIGHT;
        let lines: Vec<String> = dialog::wrap(font, &text, most).into_iter().map(|l| l.trim().to_string()).collect();
        let widest = lines.iter().map(|l| font.measure(l)).max().unwrap_or(0);
        let width = (TEXT_X + widest + TEXT_RIGHT).max(189);
        DiskBox { era, title, lines, picture: None, focus: Part::Ok, pressed: None, active: true, keys: era.always_shows_keys(), width, address: true }
    }

    fn buttons(&self) -> Vec<Part> {
        if self.address {
            vec![Part::Ok]
        } else if self.era == Era::Win2000 {
            vec![Part::Cancel]
        } else {
            vec![Part::Retry, Part::Cancel]
        }
    }

    /// The address message: where its OK stands (17 under the taller of picture and text).
    fn ok_y(&self) -> i32 {
        33 + (LINE_H * self.lines.len() as i32).max(32) + 17
    }
}

impl Form for DiskBox {
    type Part = Part;

    fn title(&self) -> &str {
        &self.title
    }
    fn size(&self) -> (i32, i32) {
        if self.address {
            return (self.width, self.ok_y() + 23 + 14);
        }
        (self.width, if self.era == Era::Win2000 { 137 } else { 126 })
    }
    fn layout(&mut self, _: &Fonts) {}
    fn paint(&self, fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts) {
        let close = ButtonLook {
            button: TitleButton::Close,
            rect: window::title_button_rect(self.size().0, TitleButton::Close),
            pressed: self.pressed == Some(Part::Close),
            enabled: true,
        };
        window::paint_frame(fb, theme, fonts, &Frame { era: self.era, title: &self.title, active: self.active, icon: None, buttons: &[close] });
        let (text_at, icon_at) = if self.address {
            ((TEXT_X, 33 + ((32 - LINE_H * self.lines.len() as i32) / 2).max(0)), (14, 33))
        } else if self.era == Era::Win2000 {
            ((108, 33), (14, 26))
        } else {
            ((62, 33), (14, 33))
        };
        match &self.picture {
            Some(p) if self.era == Era::Win2000 => fb.draw_icon(p, icon_at.0, icon_at.1),
            _ => fb.draw_icon(&ERROR.icon(), icon_at.0, icon_at.1),
        }
        for (n, line) in self.lines.iter().enumerate() {
            fonts.regular.draw(fb, text_at.0, text_at.1 + LINE_H * n as i32, line, theme.colors.text);
        }
        for part in self.buttons() {
            let label = match part {
                Part::Retry => "&Retry",
                Part::Ok => "OK",
                _ => "Cancel",
            };
            let focused = self.focus == part;
            dialog::push_button(fb, theme, fonts.regular, self.rect(part).unwrap_or_default(), label, focused, self.pressed == Some(part), self.active && self.keys && focused, true, self.keys);
        }
    }
    fn rect(&self, part: Part) -> Option<Rect> {
        if self.address {
            return match part {
                Part::Close => Some(window::title_button_rect(self.size().0, TitleButton::Close)),
                Part::Ok => Some(Rect::new((self.width - 75) / 2, self.ok_y(), 75, 23)),
                _ => None,
            };
        }
        Some(match (self.era, part) {
            (_, Part::Close) => window::title_button_rect(self.size().0, TitleButton::Close),
            (_, Part::Ok) => return None,
            (Era::Win2000, Part::Cancel) => Rect::new(257, 98, 75, 23),
            (Era::Win2000, Part::Retry) => return None,
            (_, Part::Retry) => Rect::new(17, 89, 75, 23),
            (_, Part::Cancel) => Rect::new(98, 89, 75, 23),
        })
    }
    fn hit(&self, x: i32, y: i32) -> Option<Part> {
        [Part::Retry, Part::Cancel, Part::Ok, Part::Close].into_iter().find(|&p| self.rect(p).is_some_and(|r| r.contains(x, y)))
    }
    fn enabled(&self, part: Part) -> bool {
        self.rect(part).is_some()
    }
    fn escape(&self) -> Part {
        if self.address { Part::Ok } else { Part::Cancel }
    }
    fn focus(&self) -> Part {
        self.focus
    }
    fn set_focus(&mut self, part: Part) {
        if part != Part::Close && self.enabled(part) {
            self.focus = part;
        }
    }
    fn tab_stops(&self) -> Vec<Part> {
        self.buttons()
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
    fn key(&mut self, key: Key, _: Mods) -> Option<Vec<Effect<Part>>> {
        if matches!(key, Key::Left | Key::Right | Key::Up | Key::Down) && self.era != Era::Win2000 && !self.address {
            self.focus = if self.focus == Part::Retry { Part::Cancel } else { Part::Retry };
            return Some(Vec::new());
        }
        None
    }
    fn access(&mut self, ch: char) -> Vec<Effect<Part>> {
        match ch.to_ascii_lowercase() {
            'r' if self.era != Era::Win2000 && !self.address => vec![Effect::Activate(Part::Retry)],
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
        match part {
            Some(part) => {
                self.set_focus(part);
                Press::Hold(part)
            }
            None => Press::Nothing,
        }
    }
}

static ERROR: ClassicIcon = ClassicIcon::art("message-box/error", crate::art::ANY);


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_look_says_it_its_own_way() {
        let w = DiskBox::new(Era::Win95, "Exploring - 3\u{bd} Floppy (A:)", 'A', None, false, |_| 100);
        assert_eq!(w.size(), (189, 126));
        assert_eq!(w.focus(), Part::Retry);
        assert_eq!(w.escape(), Part::Cancel);
        assert_eq!(w.rect(Part::Cancel), Some(Rect::new(98, 89, 75, 23)));
        let w = DiskBox::new(Era::Win2000, "x", 'A', None, false, |_| 0);
        assert_eq!((w.title(), w.size(), w.focus()), ("Insert disk", (347, 137), Part::Cancel));
        assert!(!w.enabled(Part::Retry));
    }

    #[test]
    fn the_address_message_is_laid_out_as_measured() {
        // `<era>-truecolor-m17-address-bad-path`: 98 494 x 119, 2000 485 x 119, two lines, OK
        // at y 82 centered.
        let ms = crate::text::Font::parse(include_str!("../tests/fixtures/ms-sans-serif-11px.font")).unwrap();
        let tahoma = crate::text::Font::parse(include_str!("../tests/fixtures/tahoma-11px.font")).unwrap();
        let w = DiskBox::address(Era::Win98, "C:\\nothere\\nofile", 800, &ms);
        assert_eq!(w.lines.len(), 2, "{:?}", w.lines);
        assert!(w.lines[0].ends_with("filename are"), "{:?}", w.lines);
        assert_eq!(w.title(), "C:\\nothere\\nofile");
        assert_eq!(w.size(), (494, 119));
        assert_eq!(w.rect(Part::Ok), Some(Rect::new(209, 82, 75, 23)));
        assert_eq!((w.focus(), w.escape(), w.enter()), (Part::Ok, Part::Ok, Part::Ok));
        let w = DiskBox::address(Era::Win2000, "C:\\nothere\\nofile", 800, &tahoma);
        assert!(w.lines[0].ends_with("file name are"), "{:?}", w.lines);
        assert_eq!((w.title(), w.size()), ("Address Bar", (485, 119)));
    }
}