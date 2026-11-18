//! Windows 2000's Folder Options window (Tools > Folder Options...), with the parts of its General
//! page that have a counterpart here: Web View (the web view panel in every folder, or classic
//! folders) and Browse Folders (each folder in the same window, or in its own), Restore Defaults,
//! OK, Cancel and Apply. Measured on the Reference PC (`private/reference/refs/vm/win2000-truecolor-explorer-
//! folder-options`, `-web-fo-*`): the window's frame, title bar and tab, the two groups with their
//! pictures (cut from the captures, they change with the choice at once), the buttons; its text in
//! MS Sans Serif (the dialog font of 2000's shell pages, not Tahoma).
//!
//! Left out, not grayed (no counterpart): the Active Desktop and "Click items as follows" groups,
//! the View, File Types and Offline Files tabs. The page is shorter by those groups (the groups
//! kept stand as far apart as on the PC, the rest moves up), and the title bar's "?" does nothing.

use std::sync::LazyLock;

use crate::dialog::{self, OPTION_OFF, OPTION_ON, TitleButtons};
use crate::fb::{Framebuffer, Icon, Rect};
use crate::kit::{Effect, Form, Key, Mods, Press};
use crate::text::{self, Fonts};
use crate::theme::{Era, Theme};
use crate::window::{self, TitleButton};

pub const TITLE: &str = "Folder Options";

/// The window's size: the PC's 386 × 468 without the two groups left out.
pub const SIZE: (i32, i32) = (386, 283);

const TAB: Rect = Rect::new(11, 31, 49, 18);
const PAGE: Rect = Rect::new(9, 49, 368, 195);
const WEB_GROUP: Rect = Rect::new(24, 68, 339, 54);
const BROWSE_GROUP: Rect = Rect::new(24, 140, 339, 56);
/// Within a group: the picture, and the two choices (the round button's place 2 rows above its
/// art, as `properties` draws them).
const PICTURE_AT: (i32, i32) = (10, 12);
const CHOICE_AT: [(i32, i32); 2] = [(52, 10), (52, 28)];
/// Browse Folders is two rows taller, its picture and choices a row lower (-web-fo-own-window).
const BROWSE_CHOICE_AT: [(i32, i32); 2] = [(52, 11), (52, 29)];
const BROWSE_PICTURE_AT: (i32, i32) = (10, 13);
const RESTORE: Rect = Rect::new(255, 207, 108, 23);
const BUTTONS_Y: i32 = 250;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Help,
    Close,
    Tab,
    Web,
    Classic,
    Same,
    Own,
    Restore,
    Ok,
    Cancel,
    Apply,
}

/// What the page chooses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Choice {
    /// "Enable Web content in folders" (off: "Use Windows classic folders").
    pub web: bool,
    /// "Open each folder in its own window" (off: "... in the same window").
    pub own_window: bool,
}

impl Default for Choice {
    /// Windows 2000's defaults.
    fn default() -> Choice {
        Choice { web: true, own_window: false }
    }
}

/// The window.
pub struct FolderOptions {
    pub choice: Choice,
    /// What was last applied (Apply is grayed while the page shows it).
    pub applied: Choice,
    /// Restore Defaults was pressed since: Apply is usable even when nothing differs (2000 did
    /// that, `win2000-truecolor-m17-folder-options-restore-at-defaults`).
    restored: bool,
    focus: Part,
    pressed: Option<Part>,
    active: bool,
    keys: bool,
}

/// The four pictures (web content, classic folders, same window, own window).
fn picture(i: usize) -> &'static Icon {
    static PICTURES: LazyLock<Vec<Icon>> = LazyLock::new(|| {
        const RAW: &[u8] = crate::art::bytes("folder-options/pictures", crate::art::Look::Win2000);
        let raw = RAW;
        raw.as_chunks::<{ 32 * 32 * 3 }>().0.iter()
            .map(|p| Icon { width: 32, height: 32, pixels: p.as_chunks::<3>().0.iter().map(|c| 0xFF00_0000 | u32::from(c[0]) << 16 | u32::from(c[1]) << 8 | u32::from(c[2])).collect() })
            .collect()
    });
    &PICTURES[i]
}

fn label(part: Part) -> &'static str {
    match part {
        Part::Web => "Enable Web content in &folders",
        Part::Classic => "Use Windows class&ic folders",
        Part::Same => "Open each folder in the sa&me window",
        Part::Own => "Open each folder in its own &window",
        Part::Restore => "&Restore Defaults",
        Part::Ok => "OK",
        Part::Cancel => "Cancel",
        Part::Apply => "&Apply",
        _ => "",
    }
}

impl FolderOptions {
    /// The window showing `now` (what is in effect); `by_keyboard`: opened with a key, so the
    /// access keys show from the start.
    pub fn new(now: Choice, by_keyboard: bool) -> FolderOptions {
        FolderOptions { choice: now, applied: now, restored: false, focus: Part::Tab, pressed: None, active: true, keys: by_keyboard }
    }

    fn choice_rect(part: Part) -> Option<(i32, i32)> {
        let (g, at) = match part {
            Part::Web => (WEB_GROUP, CHOICE_AT[0]),
            Part::Classic => (WEB_GROUP, CHOICE_AT[1]),
            Part::Same => (BROWSE_GROUP, BROWSE_CHOICE_AT[0]),
            Part::Own => (BROWSE_GROUP, BROWSE_CHOICE_AT[1]),
            _ => return None,
        };
        Some((g.x + at.0, g.y + at.1))
    }

    fn checked(&self, part: Part) -> bool {
        match part {
            Part::Web => self.choice.web,
            Part::Classic => !self.choice.web,
            Part::Same => !self.choice.own_window,
            Part::Own => self.choice.own_window,
            _ => false,
        }
    }

    /// A choice made: the page shows it (Apply comes alive).
    fn choose(&mut self, part: Part) {
        match part {
            Part::Web => self.choice.web = true,
            Part::Classic => self.choice.web = false,
            Part::Same => self.choice.own_window = false,
            Part::Own => self.choice.own_window = true,
            Part::Restore => {
                self.choice = Choice::default();
                self.restored = true;
            }
            _ => {}
        }
    }

    /// Apply (and OK) took the page's choice.
    pub fn applied_now(&mut self) {
        self.applied = self.choice;
        self.restored = false;
    }
}

impl Form for FolderOptions {
    type Part = Part;

    fn title(&self) -> &str {
        TITLE
    }
    fn size(&self) -> (i32, i32) {
        SIZE
    }
    fn layout(&mut self, _: &Fonts) {}

    fn paint(&self, fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts) {
        let c = &theme.colors;
        let font = text::ms_sans_serif();
        let held = |p: Part| self.pressed == Some(p);
        let buttons = TitleButtons { help: self.rect(Part::Help).map(|r| (r, held(Part::Help))), close: self.rect(Part::Close).map(|r| (r, held(Part::Close))) };
        dialog::window_frame(fb, theme, fonts, Era::Win2000, TITLE, &buttons);
        fb.frame(PAGE, c.highlight, c.dark_shadow);
        fb.frame(PAGE.inset(1, 1, 1, 1), c.light, c.shadow);
        let drawn = Rect::new(TAB.x - 2, TAB.y - 2, TAB.w + 4, TAB.h + 3);
        crate::properties::tab(fb, c, drawn);
        font.draw(fb, TAB.x + 6, drawn.y + 3, "General", c.text);
        if self.focus == Part::Tab && self.keys && self.active {
            dialog::focus_dots(fb, TAB.inset(1, 1, 1, 1), (0, 0), c.text);
        }
        for (group, name, pic) in [(WEB_GROUP, "Web View", if self.choice.web { 0 } else { 1 }), (BROWSE_GROUP, "Browse Folders", if self.choice.own_window { 3 } else { 2 })] {
            crate::properties::group_box(fb, c, font, group, name, false);
            let at = if group == BROWSE_GROUP { BROWSE_PICTURE_AT } else { PICTURE_AT };
            fb.draw_icon(picture(pic), group.x + at.0, group.y + at.1);
        }
        for part in [Part::Web, Part::Classic, Part::Same, Part::Own] {
            let Some((x, y)) = Self::choice_rect(part) else { continue };
            let rows = if self.checked(part) { &OPTION_ON } else { &OPTION_OFF };
            dialog::art(fb, rows, x, y + 2, |b| match b {
                b'g' => Some(c.shadow),
                b'#' => Some(c.dark_shadow),
                b'W' => Some(if held(part) { c.face } else { c.highlight }),
                b'l' => Some(c.light),
                _ => None,
            });
            if self.checked(part) {
                // The dot in the text color (the art's own is the dark shadow).
                fb.fill(Rect::new(x + 4, y + 2 + 5, 4, 2), c.text);
                fb.fill(Rect::new(x + 5, y + 2 + 4, 2, 4), c.text);
            }
            let (tx, ty) = (x + 17, y + 2);
            font.draw_label(fb, (tx, ty), label(part), i32::MAX, c.text, self.keys);
            if self.focus == part && self.keys && self.active {
                let w = font.measure_label(label(part));
                // A rectangle as tall as the text (-web-fo-own-window).
                // Drawn as Windows' DrawFocusRect inverts: a dot on the text (the access key's
                // underline on the last row) leaves the face.
                let r = Rect::new(tx - 1, ty, w + 2, font.height);
                let mut dots = Framebuffer::new(fb.width, fb.height, 0);
                dialog::focus_dots(&mut dots, r, (tx, ty), 1);
                for yy in r.y..r.bottom() {
                    for xx in r.x..r.right() {
                        if dots.get(xx, yy) == 1 {
                            let was = fb.get(xx, yy);
                            fb.set(xx, yy, if was == c.text { c.face } else { c.text });
                        }
                    }
                }
            }
        }
        // The default button has the black frame: the push button with the focus, else OK.
        let default = if matches!(self.focus, Part::Restore | Part::Ok | Part::Cancel | Part::Apply) { self.focus } else { Part::Ok };
        for part in [Part::Restore, Part::Ok, Part::Cancel, Part::Apply] {
            let Some(r) = self.rect(part) else { continue };
            let focused = self.focus == part && self.keys && self.active;
            dialog::push_button(fb, theme, font, r, label(part), part == default, held(part), focused, self.enabled(part), self.keys);
        }
    }

    fn rect(&self, part: Part) -> Option<Rect> {
        Some(match part {
            Part::Help => window::title_button_rect(SIZE.0, TitleButton::Help),
            Part::Close => window::title_button_rect(SIZE.0, TitleButton::Close),
            Part::Tab => TAB,
            Part::Restore => RESTORE,
            Part::Ok => Rect::new(140, BUTTONS_Y, 75, 23),
            Part::Cancel => Rect::new(221, BUTTONS_Y, 75, 23),
            Part::Apply => Rect::new(302, BUTTONS_Y, 75, 23),
            p => {
                let (x, y) = Self::choice_rect(p)?;
                let w = text::ms_sans_serif().measure_label(label(p));
                Rect::new(x, y, 17 + w + 2, 16)
            }
        })
    }

    fn hit(&self, x: i32, y: i32) -> Option<Part> {
        [Part::Help, Part::Close, Part::Tab, Part::Web, Part::Classic, Part::Same, Part::Own, Part::Restore, Part::Ok, Part::Cancel, Part::Apply]
            .into_iter()
            .find(|&p| self.rect(p).is_some_and(|r| r.contains(x, y)))
    }

    fn enabled(&self, part: Part) -> bool {
        part != Part::Apply || self.choice != self.applied || self.restored
    }
    fn focus(&self) -> Part {
        self.focus
    }
    fn set_focus(&mut self, part: Part) {
        if !matches!(part, Part::Help | Part::Close) {
            self.focus = part;
        }
    }
    fn tab_stops(&self) -> Vec<Part> {
        let web = if self.choice.web { Part::Web } else { Part::Classic };
        let browse = if self.choice.own_window { Part::Own } else { Part::Same };
        let mut stops = vec![Part::Tab, web, browse, Part::Restore, Part::Ok, Part::Cancel];
        if self.enabled(Part::Apply) {
            stops.push(Part::Apply);
        }
        stops
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
        match self.focus {
            p @ (Part::Restore | Part::Cancel | Part::Apply) => p,
            _ => Part::Ok,
        }
    }
    fn escape(&self) -> Part {
        Part::Cancel
    }
    fn key(&mut self, key: Key, _: Mods) -> Option<Vec<Effect<Part>>> {
        // The arrows move within a group and choose (as Windows' round buttons do).
        let next = match (key, self.focus) {
            (Key::Up | Key::Down | Key::Left | Key::Right, Part::Web) => Part::Classic,
            (Key::Up | Key::Down | Key::Left | Key::Right, Part::Classic) => Part::Web,
            (Key::Up | Key::Down | Key::Left | Key::Right, Part::Same) => Part::Own,
            (Key::Up | Key::Down | Key::Left | Key::Right, Part::Own) => Part::Same,
            _ => return None,
        };
        self.focus = next;
        self.choose(next);
        Some(Vec::new())
    }
    fn access(&mut self, ch: char) -> Vec<Effect<Part>> {
        let part = match ch.to_ascii_lowercase() {
            'f' => Part::Web,
            'i' => Part::Classic,
            'm' => Part::Same,
            'w' => Part::Own,
            'r' => return vec![Effect::Activate(Part::Restore)],
            'a' if self.enabled(Part::Apply) => return vec![Effect::Activate(Part::Apply)],
            _ => return Vec::new(),
        };
        self.focus = part;
        self.choose(part);
        Vec::new()
    }
    /// A letter without Alt works as its access key (the page has no field to type into).
    fn typed(&mut self, ch: char) -> Vec<Effect<Part>> {
        self.access(ch)
    }
    fn space(&mut self) -> Vec<Effect<Part>> {
        match self.focus {
            p @ (Part::Web | Part::Classic | Part::Same | Part::Own) => {
                self.choose(p);
                Vec::new()
            }
            p @ (Part::Restore | Part::Ok | Part::Cancel | Part::Apply) => vec![Effect::Activate(p)],
            _ => Vec::new(),
        }
    }
    fn press(&mut self, part: Option<Part>, _: i32, _: i32) -> Press<Part> {
        match part {
            Some(Part::Help) => Press::Done,
            Some(p @ (Part::Web | Part::Classic | Part::Same | Part::Own)) => {
                self.focus = p;
                self.choose(p);
                Press::Done
            }
            Some(Part::Tab) => {
                self.focus = Part::Tab;
                Press::Done
            }
            Some(Part::Apply) if !self.enabled(Part::Apply) => Press::Done,
            Some(p) => {
                if p != Part::Close {
                    self.focus = p;
                }
                Press::Hold(p)
            }
            None => Press::Nothing,
        }
    }
}

/// Restore Defaults puts Windows 2000's defaults on the page (applied with OK or Apply).
pub fn restore(w: &mut FolderOptions) {
    w.choose(Part::Restore);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choices_follow_clicks_keys_and_restore() {
        let mut w = FolderOptions::new(Choice { web: true, own_window: false }, false);
        assert!(!w.enabled(Part::Apply), "Apply is grayed until something changes");
        let r = w.rect(Part::Classic).unwrap();
        assert_eq!(w.hit(r.x + 2, r.y + 4), Some(Part::Classic));
        w.press(Some(Part::Classic), 0, 0);
        assert!(!w.choice.web && w.enabled(Part::Apply));
        w.access('w');
        assert!(w.choice.own_window);
        w.key(Key::Down, Mods::default());
        assert!(!w.choice.own_window, "an arrow moves to the other choice and takes it");
        restore(&mut w);
        assert_eq!(w.choice, Choice::default());
        w.applied_now();
        assert!(!w.enabled(Part::Apply));
        assert_eq!(w.escape(), Part::Cancel);
        assert_eq!(w.enter(), Part::Ok);
        assert_eq!(w.size(), (386, 283));
        // Restore Defaults with the defaults already in effect: Apply comes alive all the same.
        restore(&mut w);
        assert_eq!(w.choice, w.applied);
        assert!(w.enabled(Part::Apply), "as 2000 did");
        w.applied_now();
        assert!(!w.enabled(Part::Apply));
    }
}
