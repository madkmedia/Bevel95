//! The warning before a rename changes a file's extension: each look's own "Rename" message box
//! (Windows 95, 98 and 2000 alike apart from the colors, fonts and the width the font gives),
//! measured on the Reference PCs (`private/reference/refs/vm/<era>-truecolor-renext-*`, numbers in
//! docs/theme-spec.md, "The extension warning"). Today's Windows warns in File Explorer only, not
//! in its file machinery, so the desktop asks itself before it hands the new name on.
//!
//! When it comes (the same on all three PCs): a file (never a folder) whose extension was a
//! known file type gets another extension, or none. Not for a file whose old extension was not a
//! known type, nor for an extension added to a name that had none, nor when only the case
//! changes, nor for a shortcut (its ".lnk" stays: the shell never shows it). The names compared
//! are the file's real names before and after: where the desktop did not show the extension, the
//! old one is kept (`file_ops::new_file_name`), so it does not change.
//!
//! A plain message box (MB_YESNO | MB_ICONEXCLAMATION): Yes has the focus and is the default;
//! the title bar's "X" is grayed and Esc and Alt+F4 do nothing, so only Yes or No end it. After
//! No the rename box stays open, all of it selected: with the name typed in Windows 95 and 98,
//! with the old name again in Windows 2000.

use crate::dialog;
use crate::fb::{Framebuffer, Rect};
use crate::kit::{Effect, Form, Key, Mods, Press};
use crate::text::Fonts;
use crate::theme::{Era, Theme};
use crate::window::{self, ButtonLook, Frame, TitleButton};

/// The window's title and its words, line by line (the middle line empty), as the PCs showed.
pub const TITLE: &str = "Rename";
pub const LINES: [&str; 3] =
    ["If you change a filename extension, the file may become unusable.", "", "Are you sure you want to change it?"];

/// A name's extension as Windows finds it (PathFindExtension): from its last dot on, unless a
/// space follows that dot; "" when there is none.
pub fn extension(name: &str) -> &str {
    match name.rfind('.') {
        Some(i) if !name[i..].contains(' ') => &name[i..],
        _ => "",
    }
}

/// Whether renaming the file `old` to `new` (real file names, extensions included) asks first:
/// a file, not a folder, whose old extension is a known file type (`known`, given the extension
/// with its dot) and differs from the new one, case aside.
pub fn warns(old: &str, new: &str, folder: bool, known: impl Fn(&str) -> bool) -> bool {
    let (from, to) = (extension(old), extension(new));
    !folder && !from.is_empty() && !from.eq_ignore_ascii_case(to) && known(from)
}

/// The characters a file name cannot hold (Windows' own list, as its message gives it).
pub const BAD_CHARACTERS: [char; 9] = ['\\', '/', ':', '*', '?', '"', '<', '>', '|'];

/// Whether a typed name cannot be a file's name: it holds one of `BAD_CHARACTERS` (so also a
/// path: "..\x", "C:\x", "a/b"), or it is only dots and spaces. Bevel95 refuses such a name before
/// Windows' file machinery sees it (the safety review of 2026-11-18, suspected 3: Windows refused
/// a rename into another folder, but for "a/b" it first made a folder "a").
pub fn bad_name(name: &str) -> bool {
    name.contains(BAD_CHARACTERS) || name.chars().all(|c| c == '.' || c == ' ')
}

/// The look's message for a bad name: (title, text). The titles are the looks' measured ones for
/// a rename that cannot be done (`drag::undo_failed`); the text is the one Windows 95 to 2000 gave
/// (PROVISIONAL: not measured on the Reference PCs; 2000 showed it in a balloon beside the box).
pub fn bad_name_message(era: Era) -> (String, String) {
    let title = if era == Era::Win2000 { "Error Renaming File or Folder" } else { "Error Renaming File" };
    (title.into(), "A filename cannot contain any of the following characters: \\ / : * ? \" < > |".into())
}

/// After No, the rename box shows the name typed (Windows 95 and 98) or the old name (2000).
pub fn keeps_typed_name(era: Era) -> bool {
    era != Era::Win2000
}

/// Where the message box goes on a screen of `w` x `h` (pixels at 1x): USER's own rule, read off
/// the PCs' 800x600 screens (this box at 209,245 and 204,245, Solitaire's 185x119 "Deal Again?"
/// box at 311,249).
pub fn place(screen: (i32, i32), size: (i32, i32)) -> (i32, i32) {
    ((screen.0 - size.0 + 7) / 2, (screen.1 - size.1 + 17) / 2)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Yes,
    No,
    Close,
}

/// The message box.
pub struct RenameWarning {
    pub era: Era,
    focus: Part,
    pressed: Option<Part>,
    active: bool,
    keys: bool,
}

const ICON_AT: (i32, i32) = (14, 33);
const TEXT_Y: i32 = 33;

impl RenameWarning {
    /// `by_keyboard`: the name was taken with Enter, so Windows 2000 underlines the access keys
    /// from the start (95 and 98 always did).
    pub fn new(era: Era, by_keyboard: bool) -> RenameWarning {
        RenameWarning {
            era,
            focus: Part::Yes,
            pressed: None,
            active: true,
            keys: era.always_shows_keys() || by_keyboard,
        }
    }

    /// The text's left edge (Tahoma's box is wider, its text 3 pixels further right).
    fn text_x(&self) -> i32 {
        if self.era == Era::Win2000 { 65 } else { 62 }
    }
}

impl Form for RenameWarning {
    type Part = Part;

    fn title(&self) -> &str {
        TITLE
    }
    fn size(&self) -> (i32, i32) {
        if self.era == Era::Win2000 { (398, 126) } else { (389, 126) }
    }
    fn layout(&mut self, _: &Fonts) {}
    fn paint(&self, fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts) {
        let close = ButtonLook {
            button: TitleButton::Close,
            rect: window::title_button_rect(self.size().0, TitleButton::Close),
            pressed: false,
            enabled: false,
        };
        window::paint_frame(
            fb,
            theme,
            fonts,
            &Frame { era: self.era, title: TITLE, active: self.active, icon: None, buttons: &[close] },
        );
        crate::colors16::draw_picture(fb, theme, &crate::icons::WARNING.icon(), ICON_AT.0, ICON_AT.1);
        for (n, line) in LINES.iter().enumerate() {
            fonts.regular.draw(fb, self.text_x(), TEXT_Y + 13 * n as i32, line, theme.colors.text);
        }
        for (part, label) in [(Part::Yes, "&Yes"), (Part::No, "&No")] {
            let focused = self.focus == part;
            dialog::push_button(
                fb,
                theme,
                fonts.regular,
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
        let x = if self.era == Era::Win2000 { 120 } else { 116 };
        Some(match part {
            Part::Yes => Rect::new(x, 89, 75, 23),
            Part::No => Rect::new(x + 81, 89, 75, 23),
            Part::Close => window::title_button_rect(self.size().0, TitleButton::Close),
        })
    }
    fn hit(&self, x: i32, y: i32) -> Option<Part> {
        [Part::Yes, Part::No, Part::Close].into_iter().find(|&p| self.rect(p).is_some_and(|r| r.contains(x, y)))
    }
    fn enabled(&self, part: Part) -> bool {
        part != Part::Close
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
    /// Esc presses the grayed "X": nothing happens.
    fn escape(&self) -> Part {
        Part::Close
    }
    fn key(&mut self, key: Key, _: Mods) -> Option<Vec<Effect<Part>>> {
        // The arrows go from button to button, as in Windows' message boxes.
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
        match part {
            Some(Part::Close) => Press::Done,
            Some(part) => {
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

    fn known(ext: &str) -> bool {
        // A few types every Windows knows; ".dat" and ".abc" are nobody's.
        [".txt", ".bmp", ".lnk"].iter().any(|k| k.eq_ignore_ascii_case(ext))
    }

    /// A typed name that would be a path or that Windows refuses never reaches its file machinery
    /// (the safety review of 2026-11-18, suspected 3).
    #[test]
    fn a_name_with_a_path_or_a_bad_character_is_refused() {
        for bad in ["..\\x", "C:\\x", "a/b", "x:y", "a*", "why?", "say \"hi\"", "<x>", "a|b", "..", ".", "  ", ". ."] {
            assert!(bad_name(bad), "{bad:?}");
        }
        for good in ["Bevel95 test.txt", "a.b.c", ".hidden", "x..y", "  spaced  ", "Ünïcödé", "a - Copy (2)", "v1.2"] {
            assert!(!bad_name(good), "{good:?}");
        }
        let (title, text) = bad_name_message(Era::Win98);
        assert_eq!(title, "Error Renaming File");
        assert_eq!(text, "A filename cannot contain any of the following characters: \\ / : * ? \" < > |");
        assert_eq!(bad_name_message(Era::Win2000).0, "Error Renaming File or Folder");
    }

    #[test]
    fn it_warns_exactly_where_the_reference_pcs_did() {
        assert!(warns("a.txt", "a.dat", false, known), "a known extension changed");
        assert!(warns("a.txt", "a", false, known), "a known extension removed");
        assert!(warns("New Text Document.txt", "a.dat", false, known));
        assert!(!warns("a.dat", "a", false, known), "the old extension was no known type");
        assert!(!warns("a.dat", "a.txt", false, known), "the old extension was no known type");
        assert!(!warns("a", "a.txt", false, known), "an extension added to a name without one");
        assert!(!warns("a.txt", "a.TXT", false, known), "only the case changed");
        assert!(!warns("a.txt", "b.txt", false, known), "the same extension");
        assert!(!warns("x.txt", "x.dat", true, known), "a folder never asks");
        assert!(!warns("Shortcut to a.txt.lnk", "s.dat.lnk", false, known), "a shortcut keeps its .lnk");
        assert!(!warns("a.txt", "a.dat.txt", false, known), "a hidden extension stays");
    }

    #[test]
    fn extensions_are_found_as_windows_finds_them() {
        assert_eq!(extension("a.b.txt"), ".txt");
        assert_eq!(extension("a"), "");
        assert_eq!(extension(".profile"), ".profile");
        assert_eq!(extension("a. b"), "", "a space after the dot: no extension");
        assert_eq!(extension("a."), ".");
    }

    #[test]
    fn yes_has_the_focus_and_only_yes_or_no_end_it() {
        for era in Era::ALL {
            let mut w = RenameWarning::new(era, false);
            assert_eq!(w.focus(), Part::Yes);
            assert_eq!(w.enter(), Part::Yes);
            assert_eq!(crate::kit::activate(&w, w.escape()), Vec::new(), "Esc: the grayed X does nothing");
            assert_eq!(w.press(Some(Part::Close), 0, 0), Press::Done);
            assert_eq!(w.access('n'), vec![Effect::Activate(Part::No)]);
            w.key(Key::Right, Mods::default());
            assert_eq!(w.enter(), Part::No);
            assert_eq!(w.keys, era != Era::Win2000, "2000 underlines once a key was used");
            assert!(RenameWarning::new(era, true).keys);
        }
        assert!(keeps_typed_name(Era::Win95) && keeps_typed_name(Era::Win98) && !keeps_typed_name(Era::Win2000));
    }

    #[test]
    fn its_size_and_place_are_the_reference_pcs() {
        let w = RenameWarning::new(Era::Win98, true);
        assert_eq!(w.size(), (389, 126));
        assert_eq!(w.rect(Part::No), Some(Rect::new(197, 89, 75, 23)));
        assert_eq!(w.hit(200, 100), Some(Part::No));
        assert_eq!(RenameWarning::new(Era::Win2000, true).size(), (398, 126));
        assert_eq!(place((800, 600), (389, 126)), (209, 245));
        assert_eq!(place((800, 600), (398, 126)), (204, 245));
        assert_eq!(place((800, 600), (185, 119)), (311, 249));
    }
}
