//! HTML Help's Topics Found dialog (HHCTRL resource 107). An index keyword can name several articles.
//! Windows 98 uses a sorted listbox; Windows 2000 shows a report with Title and Location columns.

use crate::dialog;
use crate::fb::{Framebuffer, Rect};
use crate::kit::{Effect, Form, Key, Mods, Press};
use crate::properties;
use crate::text::{Fonts, ms_sans_serif};
use crate::theme::{Era, Theme, rgb};
use crate::window::{self, ButtonLook, Frame, TitleButton};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    List,
    Row(usize),
    Display,
    Cancel,
    Close,
    ScrollUp,
    ScrollDown,
    Track,
    Thumb,
}

pub struct TopicsFound {
    era: Era,
    location: String,
    topics: Vec<(usize, String)>,
    selected: usize,
    first: usize,
    focus: Part,
    pressed: Option<Part>,
    keys: bool,
    active: bool,
}

impl TopicsFound {
    pub fn new(era: Era, location: String, mut topics: Vec<(usize, String)>) -> Self {
        if era != Era::Win2000 {
            topics.sort_by_cached_key(|(_, title)| title.to_lowercase());
        }
        Self {
            era,
            location,
            topics,
            selected: 0,
            first: 0,
            focus: Part::List,
            pressed: None,
            keys: era.always_shows_keys(),
            active: true,
        }
    }

    /// The article ID supplied by the catalog, independent of the order of the displayed rows.
    pub fn selection(&self) -> Option<usize> {
        self.topics.get(self.selected).map(|(id, _)| *id)
    }

    pub fn activate(&mut self, part: Part) {
        match part {
            Part::Row(i) if i < self.topics.len() => {
                self.focus = Part::List;
                self.selected = i;
                self.first = self.first.min(i).max((i + 1).saturating_sub(self.capacity()));
            }
            Part::ScrollUp => self.scroll(-1),
            Part::ScrollDown => self.scroll(1),
            _ => {}
        }
    }

    fn report(&self) -> bool {
        self.era == Era::Win2000
    }
    fn well(&self) -> Rect {
        if self.report() { Rect::new(14, 51, 446, 138) } else { Rect::new(14, 55, 267, 135) }
    }
    fn rows(&self) -> Rect {
        let w = self.well();
        if self.report() { w.inset(4, 21, 14, 2) } else { w.inset(2, 2, 2, 2) }
    }
    const ROW_HEIGHT: i32 = 14;
    fn capacity(&self) -> usize {
        (self.rows().h / Self::ROW_HEIGHT) as usize
    }
    fn limit(&self) -> usize {
        self.topics.len().saturating_sub(self.capacity())
    }
    fn scroll(&mut self, by: i32) {
        self.first = self.first.saturating_add_signed(by as isize).min(self.limit());
    }
    fn scroll_bar(&self) -> Option<Rect> {
        let r = self.well().inset(2, 2, 2, 2);
        (self.limit() > 0).then_some(Rect::new(r.right() - 16, r.y, 16, r.h))
    }
}

impl Form for TopicsFound {
    type Part = Part;
    fn title(&self) -> &str {
        "Topics Found"
    }
    fn size(&self) -> (i32, i32) {
        if self.report() { (477, 236) } else { (294, 238) }
    }
    fn layout(&mut self, _: &Fonts) {}
    fn paint(&self, fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts) {
        let close = ButtonLook {
            button: TitleButton::Close,
            rect: self.rect(Part::Close).unwrap(),
            pressed: self.pressed == Some(Part::Close),
            enabled: true,
        };
        window::paint_frame(
            fb,
            theme,
            fonts,
            &Frame { title: self.title(), era: self.era, active: self.active, icon: None, buttons: &[close] },
        );
        let c = &theme.colors;
        let font = ms_sans_serif();
        let label =
            if self.report() { "Click a &topic, then click Display." } else { "&Click a topic, then click Display." };
        font.draw_label(fb, (14, 33), label, i32::MAX, c.text, self.keys);
        let list = dialog::well(fb, self.well(), c);
        let end = self.scroll_bar().map_or(list.right(), |r| r.x);
        if self.report() {
            // The column header window continues beyond the final column to the list's right edge.
            for (x, right, title) in [(16, 223, "Title"), (223, 446, "Location"), (446, end, "")] {
                if x >= end {
                    break;
                }
                let right = right.min(end);
                let r = Rect::new(x, 53, right - x, 17);
                fb.fill(r, c.face);
                fb.frame(r, c.highlight, c.dark_shadow);
                fb.frame(r.inset(1, 1, 1, 1), c.face, c.shadow);
                if right == end {
                    // The remainder is the clipped start of a header extending past the client edge.
                    for (y, h, ink) in [(53, 1, c.highlight), (54, 14, c.face), (68, 1, c.shadow)] {
                        fb.fill(Rect::new(right - 2, y, 2, h), ink);
                    }
                }
                font.draw(fb, x + 6, 55, title, c.text);
            }
        }
        for i in self.first..(self.first + self.capacity()).min(self.topics.len()) {
            let r = self.rect(Part::Row(i)).unwrap();
            let selected = i == self.selected;
            let active = self.focus == Part::List && self.active;
            let ink = if selected && active { c.flash_text } else { c.text };
            if selected {
                fb.fill(r, if active { c.flash_face } else { c.face });
            }
            // The 98 listbox inherits the navigation pane's Arial, not the surrounding dialog font.
            let font = if self.report() { font } else { super::fonts::tree98() };
            let x = r.x + 2;
            let y = r.y;
            let title_right = if self.report() { 221 } else { r.right() };
            font.draw(fb, x, y, &font.elide(&self.topics[i].1, title_right - x - 1), ink);
            if self.report() {
                font.draw(fb, 229, y, &font.elide(&self.location, r.right() - 230), ink);
            }
            if selected && active && self.keys {
                dialog::focus_dots(fb, r, (r.x + i32::from(self.report()), r.y), rgb(!c.flash_face));
            }
        }
        if let Some(track) = self.rect(Part::Track) {
            let thumb = self.rect(Part::Thumb).unwrap();
            fb.checker(track, (track.x, track.y), c.face, c.highlight);
            fb.fill(thumb, c.face);
            fb.frame(thumb, c.light, c.dark_shadow);
            fb.frame(thumb.inset(1, 1, 1, 1), c.highlight, c.shadow);
            for (part, up) in [(Part::ScrollUp, true), (Part::ScrollDown, false)] {
                properties::scroll_arrow(fb, c, self.rect(part).unwrap(), up, self.pressed == Some(part));
            }
        }
        for (part, label) in [(Part::Display, "&Display"), (Part::Cancel, "Cancel")] {
            dialog::push_button(
                fb,
                theme,
                font,
                self.rect(part).unwrap(),
                label,
                self.enter() == part,
                self.pressed == Some(part),
                self.active && self.keys && self.focus == part,
                self.enabled(part),
                self.keys,
            );
        }
    }
    fn rect(&self, part: Part) -> Option<Rect> {
        Some(match part {
            Part::List => self.well(),
            Part::Row(i) => {
                if i < self.first || i >= self.topics.len() || i - self.first >= self.capacity() {
                    return None;
                }
                let r = self.rows();
                let right = self.scroll_bar().map_or(r.right(), |b| r.right().min(b.x));
                Rect::new(r.x, r.y + (i - self.first) as i32 * Self::ROW_HEIGHT, right - r.x, Self::ROW_HEIGHT)
            }
            Part::Close => window::title_button_rect(self.size().0, TitleButton::Close),
            Part::Display if self.report() => Rect::new(305, 202, 75, 23),
            Part::Cancel if self.report() => Rect::new(386, 202, 75, 23),
            Part::Display => Rect::new(125, 201, 75, 23),
            Part::Cancel => Rect::new(206, 201, 75, 23),
            Part::ScrollUp | Part::ScrollDown => {
                let r = self.scroll_bar()?;
                Rect::new(r.x, if part == Part::ScrollUp { r.y } else { r.bottom() - 16 }, 16, 16)
            }
            Part::Track => self.scroll_bar()?.inset(0, 16, 0, 16),
            Part::Thumb => {
                let r = self.rect(Part::Track)?;
                let h = (r.h * self.capacity() as i32 / self.topics.len() as i32).max(8);
                Rect::new(r.x, r.y + self.first as i32 * (r.h - h) / self.limit() as i32, r.w, h)
            }
        })
    }
    fn hit(&self, x: i32, y: i32) -> Option<Part> {
        for p in [Part::Close, Part::Display, Part::Cancel, Part::ScrollUp, Part::ScrollDown, Part::Thumb, Part::Track]
        {
            if self.rect(p).is_some_and(|r| r.contains(x, y)) {
                return Some(p);
            }
        }
        let r = self.rows();
        if r.contains(x, y) {
            let p = Part::Row(self.first + ((y - r.y) / Self::ROW_HEIGHT) as usize);
            if self.rect(p).is_some_and(|r| r.contains(x, y)) {
                return Some(p);
            }
        }
        self.well().contains(x, y).then_some(Part::List)
    }
    fn enabled(&self, part: Part) -> bool {
        part != Part::Display || self.selection().is_some()
    }
    fn focus(&self) -> Part {
        self.focus
    }
    fn set_focus(&mut self, part: Part) {
        if matches!(part, Part::Display | Part::Cancel | Part::List) && self.enabled(part) {
            self.focus = part;
        }
    }
    fn tab_stops(&self) -> Vec<Part> {
        [Part::List, Part::Display, Part::Cancel].into_iter().filter(|&p| self.enabled(p)).collect()
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
        if self.focus == Part::Cancel { Part::Cancel } else { Part::Display }
    }
    fn escape(&self) -> Part {
        Part::Cancel
    }
    fn key(&mut self, key: Key, _: Mods) -> Option<Vec<Effect<Part>>> {
        if self.focus != Part::List || self.topics.is_empty() {
            return None;
        }
        let last = self.topics.len() - 1;
        let i = match key {
            Key::Up => self.selected.saturating_sub(1),
            Key::Down => (self.selected + 1).min(last),
            Key::PageUp => self.selected.saturating_sub(self.capacity()),
            Key::PageDown => (self.selected + self.capacity()).min(last),
            Key::Home => 0,
            Key::End => last,
            _ => return None,
        };
        self.activate(Part::Row(i));
        Some(Vec::new())
    }
    fn access(&mut self, ch: char) -> Vec<Effect<Part>> {
        match ch.to_ascii_lowercase() {
            'd' if self.enabled(Part::Display) => vec![Effect::Activate(Part::Display)],
            't' if self.report() => {
                self.focus = Part::List;
                Vec::new()
            }
            'c' if !self.report() => {
                self.focus = Part::List;
                Vec::new()
            }
            _ => Vec::new(),
        }
    }
    fn space(&mut self) -> Vec<Effect<Part>> {
        match self.focus {
            p @ (Part::Display | Part::Cancel) => vec![Effect::Activate(p)],
            _ => Vec::new(),
        }
    }
    fn press(&mut self, part: Option<Part>, _: i32, y: i32) -> Press<Part> {
        match part {
            Some(Part::Row(i)) => {
                self.activate(Part::Row(i));
                Press::Done
            }
            Some(Part::List) => {
                self.focus = Part::List;
                Press::Done
            }
            Some(Part::Thumb) => Press::Grab(Part::Thumb, self.first as i32),
            Some(Part::Track) => {
                let page = self.capacity() as i32;
                self.scroll(if y < self.rect(Part::Thumb).unwrap().y { -page } else { page });
                Press::Done
            }
            Some(p @ (Part::ScrollUp | Part::ScrollDown)) => Press::Act(p),
            Some(p) if self.enabled(p) => {
                self.set_focus(p);
                Press::Hold(p)
            }
            _ => Press::Nothing,
        }
    }
    fn double_click(&mut self, x: i32, y: i32) -> Option<Vec<Effect<Part>>> {
        let p @ Part::Row(_) = self.hit(x, y)? else {
            return None;
        };
        self.activate(p);
        Some(vec![Effect::Activate(Part::Display)])
    }
    fn drag(&mut self, p: Part, start: i32, value: i32, _: i32, y: i32) -> bool {
        if p != Part::Thumb {
            return false;
        }
        let travel = self.rect(Part::Track).unwrap().h - self.rect(p).unwrap().h;
        let old = self.first;
        self.first = (value + (y - start) * self.limit() as i32 / travel).clamp(0, self.limit() as i32) as usize;
        old != self.first
    }
    fn wheel(&mut self, notches: i32) {
        self.scroll(-3 * notches);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::Controller;

    #[test]
    fn navigate_and_open_the_original_article_id_after_scrolling() {
        let topics = (0..23).map(|i| (80 - i, format!("Article {i}"))).collect();
        let mut form = TopicsFound::new(Era::Win2000, "Catalog".into(), topics);
        let mut input = Controller::default();
        input.key(&mut form, Key::End, Mods::default());
        assert_eq!(form.selection(), Some(58));
        assert_eq!(input.key(&mut form, Key::Enter, Mods::default()), [Effect::Activate(Part::Display)]);
        let previous = form.rect(Part::Row(21)).unwrap();
        assert_eq!(form.double_click(previous.x + 1, previous.y + 1), Some(vec![Effect::Activate(Part::Display)]));
        assert_eq!(form.selection(), Some(59));
        form.wheel(99);
        assert!(form.rect(Part::Row(0)).is_some());
        assert_eq!(form.selection(), Some(59));
    }

    #[test]
    fn sorted_list_preserves_ids_and_empty_list_disables_display() {
        let mut form = TopicsFound::new(Era::Win98, "Catalog".into(), vec![(1, "z".into()), (5, "A".into())]);
        assert_eq!(form.selection(), Some(5));
        form.key(Key::Down, Mods::default());
        assert_eq!(form.selection(), Some(1));
        let mut empty = TopicsFound::new(Era::Win2000, "Catalog".into(), Vec::new());
        assert_eq!(empty.selection(), None);
        assert!(Controller::default().key(&mut empty, Key::Enter, Mods::default()).is_empty());
    }
}
