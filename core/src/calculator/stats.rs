//! The Statistics Box: the window Sta opens beside the Scientific view, as each Windows drew it
//! (win<pc>-truecolor-calc-sci-sta*): a list of the numbers Dat added, RET, LOAD, CD and CAD,
//! and "n=" their count. It opens at the same place of the screen every time and has its own
//! taskbar button; Windows 2000's has a "?" beside its X. The numbers are the Calculator's
//! (`Engine::stats_lines`); this window shows them and says which one is chosen.

use crate::art::Look;
use crate::dialog;
use crate::fb::{Framebuffer, Icon, Rect};
use crate::kit::{Effect, Form, Key, Mods, Press};
use crate::text::{Fonts, split_access_key};
use crate::theme::{Era, Theme, rgb};
use crate::window::{self, ButtonLook, Frame, TitleButton};

pub const TITLE: &str = "Statistics Box";
/// Where it opens on the screen (all three).
pub const PLACE: (i32, i32) = (120, 130);
const SIZE: (i32, i32) = (225, 165);
const LIST: Rect = Rect::new(8, 27, 210, 56);
const ITEM_H: i32 = 13;
const BUTTONS: [(&str, i32); 4] = [("&RET", 9), ("&LOAD", 63), ("&CD", 117), ("C&AD", 171)];
const BUTTON_Y: i32 = 116;
const BUTTON_SIZE: (i32, i32) = (42, 23);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Help,
    Close,
    List,
    Ret,
    Load,
    Cd,
    Cad,
    /// The window menu (a click on the picture, Alt+Space).
    Icon,
}

const BUTTON_PARTS: [Part; 4] = [Part::Ret, Part::Load, Part::Cd, Part::Cad];

const ICON_98: &[&str] = crate::art::rows("calculator/statistics", Look::Win95).rows;
const ICON_2000: &[&str] = crate::art::rows("calculator/statistics", Look::Win2000).rows;

/// The box's 16-pixel picture.
pub fn icon(era: Era) -> Icon {
    let rows = if era == Era::Win2000 { ICON_2000 } else { ICON_98 };
    let mut pixels = Vec::with_capacity(256);
    for row in rows {
        for b in row.bytes() {
            pixels.push(match b {
                b'#' => 0xFF00_0000,
                b'r' => 0xFFFF_0000,
                b'g' => 0xFF00_FF00,
                b'b' => 0xFF00_00FF,
                b'y' => 0xFFFF_FF00,
                b'R' => 0xFF80_0000,
                b'B' => 0xFF00_0080,
                _ => 0,
            });
        }
    }
    Icon { width: 16, height: 16, pixels }
}

pub struct StatsBox {
    pub era: Era,
    lines: Vec<String>,
    /// The chosen line.
    pub selected: Option<usize>,
    icon: Icon,
    pressed: Option<Part>,
    focus: Part,
    active: bool,
}

impl StatsBox {
    pub fn new(era: Era) -> StatsBox {
        StatsBox { era, lines: Vec::new(), selected: None, icon: icon(era), pressed: None, focus: Part::Ret, active: true }
    }

    /// The Calculator's numbers now: a line added is chosen (Dat), one taken out leaves the
    /// one after it chosen (or the new last one).
    pub fn set_lines(&mut self, lines: Vec<String>) {
        let added = lines.len() > self.lines.len();
        self.lines = lines;
        self.selected = if self.lines.is_empty() {
            None
        } else if added {
            Some(self.lines.len() - 1)
        } else {
            self.selected.map(|i| i.min(self.lines.len() - 1))
        };
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    fn button_rect(i: usize) -> Rect {
        Rect::new(BUTTONS[i].1, BUTTON_Y, BUTTON_SIZE.0, BUTTON_SIZE.1)
    }

    fn title_buttons(&self) -> Vec<ButtonLook> {
        let mut buttons = Vec::new();
        if self.era == Era::Win2000 {
            buttons.push(ButtonLook {
                button: TitleButton::Help,
                rect: window::title_button_rect(SIZE.0, TitleButton::Help),
                pressed: self.pressed == Some(Part::Help),
                enabled: true,
            });
        }
        buttons.push(ButtonLook {
            button: TitleButton::Close,
            rect: window::title_button_rect(SIZE.0, TitleButton::Close),
            pressed: self.pressed == Some(Part::Close),
            enabled: true,
        });
        buttons
    }
}

impl Form for StatsBox {
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
        let buttons = self.title_buttons();
        let frame = Frame { era: self.era, title: TITLE, icon: Some(&self.icon), active: self.active, buttons: &buttons };
        window::paint_frame(fb, theme, fonts, &frame);
        // Calculator's dialogs are in MS Sans Serif, on 2000 too.
        let font = if self.era == Era::Win2000 { crate::text::ms_sans_serif() } else { fonts.regular };
        fb.frame(LIST, c.shadow, c.highlight);
        fb.frame(LIST.inset(1, 1, 1, 1), c.dark_shadow, c.light);
        let inside = LIST.inset(2, 2, 2, 2);
        let mut list = Framebuffer::new(inside.w, inside.h, rgb(0xFFFFFF));
        for (i, line) in self.lines.iter().enumerate() {
            let y = i as i32 * ITEM_H;
            let chosen = self.selected == Some(i);
            if chosen {
                list.fill(Rect::new(0, y, inside.w, ITEM_H), c.menu_highlight);
            }
            font.draw(&mut list, if self.era == Era::Win95 { 5 } else { 2 }, y, line, if chosen { c.menu_highlight_text } else { c.text });
        }
        fb.blit(&list, inside.x, inside.y);
        for (i, (label, _)) in BUTTONS.iter().enumerate() {
            let part = BUTTON_PARTS[i];
            let focused = self.focus == part && self.active;
            dialog::push_button(
                fb,
                theme,
                font,
                StatsBox::button_rect(i),
                label,
                part == Part::Ret || focused,
                self.pressed == Some(part),
                focused,
                true,
                true,
            );
        }
        let count = format!("n={}", self.lines.len());
        let w = font.measure(&count);
        font.draw(fb, (SIZE.0 - w) / 2 - 1, 146, &count, c.text);
    }
    fn rect(&self, part: Part) -> Option<Rect> {
        Some(match part {
            Part::Help => window::title_button_rect(SIZE.0, TitleButton::Help),
            Part::Close => window::title_button_rect(SIZE.0, TitleButton::Close),
            Part::List => LIST,
            Part::Icon => Rect::new(window::FRAME + 2, window::FRAME + 1, 16, 16),
            p => StatsBox::button_rect(BUTTON_PARTS.iter().position(|&b| b == p)?),
        })
    }
    fn hit(&self, x: i32, y: i32) -> Option<Part> {
        let mut parts = vec![Part::Close, Part::List, Part::Ret, Part::Load, Part::Cd, Part::Cad, Part::Icon];
        if self.era == Era::Win2000 {
            parts.push(Part::Help);
        }
        parts.into_iter().find(|&p| self.rect(p).is_some_and(|r| r.contains(x, y)))
    }
    fn enabled(&self, _: Part) -> bool {
        true
    }
    fn focus(&self) -> Part {
        self.focus
    }
    fn set_focus(&mut self, part: Part) {
        if matches!(part, Part::List | Part::Ret | Part::Load | Part::Cd | Part::Cad) {
            self.focus = part;
        }
    }
    fn tab_stops(&self) -> Vec<Part> {
        vec![Part::List, Part::Ret, Part::Load, Part::Cd, Part::Cad]
    }
    fn set_pressed(&mut self, part: Option<Part>) {
        self.pressed = part;
    }
    fn set_active(&mut self, active: bool) {
        self.active = active;
    }
    fn show_keys(&mut self) {}
    fn enter(&self) -> Part {
        if matches!(self.focus, Part::List) { Part::Ret } else { self.focus }
    }
    fn escape(&self) -> Part {
        Part::Ret
    }
    fn key(&mut self, key: Key, _: Mods) -> Option<Vec<Effect<Part>>> {
        if self.focus != Part::List || self.lines.is_empty() {
            return None;
        }
        let last = self.lines.len() - 1;
        let now = self.selected.unwrap_or(0);
        self.selected = Some(match key {
            Key::Up => now.saturating_sub(1),
            Key::Down => (now + 1).min(last),
            Key::Home => 0,
            Key::End => last,
            _ => return None,
        });
        Some(Vec::new())
    }
    fn access(&mut self, ch: char) -> Vec<Effect<Part>> {
        if ch == ' ' {
            return vec![Effect::Activate(Part::Icon)];
        }
        let ch = ch.to_ascii_lowercase();
        BUTTONS
            .iter()
            .position(|(label, _)| {
                let (text, at) = split_access_key(label);
                at.and_then(|k| text.chars().nth(k)).is_some_and(|c| c.to_ascii_lowercase() == ch)
            })
            .map(|i| vec![Effect::Activate(BUTTON_PARTS[i])])
            .unwrap_or_default()
    }
    fn space(&mut self) -> Vec<Effect<Part>> {
        if self.focus == Part::List { Vec::new() } else { vec![Effect::Activate(self.focus)] }
    }
    fn press(&mut self, part: Option<Part>, _x: i32, y: i32) -> Press<Part> {
        match part {
            Some(Part::List) => {
                self.focus = Part::List;
                let row = ((y - LIST.y - 2) / ITEM_H) as usize;
                if row < self.lines.len() {
                    self.selected = Some(row);
                }
                Press::Done
            }
            Some(Part::Icon) => Press::Act(Part::Icon),
            Some(part) => Press::Hold(part),
            None => Press::Nothing,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_added_is_chosen_and_one_taken_out_leaves_the_next() {
        let mut b = StatsBox::new(Era::Win98);
        b.set_lines(vec!["5.".into()]);
        b.set_lines(vec!["5.".into(), "7.".into(), "9.".into()]);
        assert_eq!(b.selected, Some(2));
        b.selected = Some(0);
        b.set_lines(vec!["7.".into(), "9.".into()]);
        assert_eq!(b.selected, Some(0));
        b.selected = Some(1);
        b.set_lines(vec!["7.".into()]);
        assert_eq!(b.selected, Some(0));
        b.set_lines(Vec::new());
        assert_eq!(b.selected, None);
        assert_eq!(icon(Era::Win2000).pixels.len(), 256);
    }
}
