//! MSHTML's Find document, its end-of-search alert, and HTML Help's choice of pages to print.
//! Geometry and colors come from the Windows 98 and 2000 help windows at 96 DPI.

use crate::art::Look;
use crate::dialog;
use crate::fb::{Framebuffer, Rect};
use crate::icons::ClassicIcon;
use crate::kit::{self, Button, Effect, Form, Key, Mods, Press};
use crate::properties;
use crate::text::{Font, Fonts, ms_sans_serif};
use crate::theme::{Era, Theme, rgb};
use crate::window::{self, ButtonLook, Frame, TitleButton};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FindOptions {
    pub whole_word: bool,
    pub match_case: bool,
    pub down: bool,
}

impl Default for FindOptions {
    fn default() -> Self {
        Self { whole_word: false, match_case: false, down: true }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Close,
    Help,
    Query,
    WholeWord,
    MatchCase,
    Up,
    Down,
    Page,
    Heading,
    Ok,
    Cancel,
    Tip(&'static str),
}

const FIND_PARTS: &[Part] = &[
    Part::Help,
    Part::Close,
    Part::Query,
    Part::WholeWord,
    Part::MatchCase,
    Part::Up,
    Part::Down,
    Part::Ok,
    Part::Cancel,
];
const PRINT_PARTS: &[Part] = &[Part::Close, Part::Page, Part::Heading, Part::Ok, Part::Cancel];
const FINISHED_PARTS: &[Part] = &[Part::Close, Part::Ok];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Find,
    Print,
    Finished,
}

pub struct CommandDialog {
    era: Era,
    kind: Kind,
    options: FindOptions,
    heading: bool,
    query: String,
    // Byte offsets at UTF-8 boundaries. A selection extends from anchor to caret in either direction.
    anchor: usize,
    caret: usize,
    positions: Vec<(usize, i32)>,
    scroll: i32,
    focus: Part,
    pressed: Option<Part>,
    keys: bool,
    blink: bool,
    active: bool,
    help: bool,
}

impl CommandDialog {
    pub fn find(era: Era) -> Self {
        Self::new(era, Kind::Find)
    }
    pub fn print(era: Era) -> Self {
        Self::new(era, Kind::Print)
    }
    pub fn finished(era: Era) -> Self {
        Self::new(era, Kind::Finished)
    }

    fn new(era: Era, kind: Kind) -> Self {
        Self {
            era,
            kind,
            options: FindOptions::default(),
            heading: false,
            query: String::new(),
            anchor: 0,
            caret: 0,
            positions: Vec::new(),
            scroll: 0,
            focus: if kind == Kind::Find { Part::Query } else { Part::Ok },
            pressed: None,
            keys: era.always_shows_keys(),
            blink: true,
            active: true,
            help: false,
        }
    }

    pub fn is_find(&self) -> bool {
        self.kind == Kind::Find
    }
    pub fn select_query(&mut self) {
        self.set_focus(Part::Query);
    }
    pub fn query(&self) -> &str {
        &self.query
    }
    pub fn find_options(&self) -> FindOptions {
        self.options
    }
    pub fn include_subtopics(&self) -> bool {
        self.heading
    }

    pub fn activate(&mut self, part: Part) {
        match part {
            Part::WholeWord => self.options.whole_word = !self.options.whole_word,
            Part::MatchCase => self.options.match_case = !self.options.match_case,
            Part::Down | Part::Up => self.options.down = part == Part::Down,
            Part::Page | Part::Heading => self.heading = part == Part::Heading,
            Part::Help => self.help = true,
            _ => {}
        }
    }

    fn parts(&self) -> &'static [Part] {
        match self.kind {
            Kind::Find => FIND_PARTS,
            Kind::Print => PRINT_PARTS,
            Kind::Finished => FINISHED_PARTS,
        }
    }

    fn selection(&self) -> std::ops::Range<usize> {
        self.caret.min(self.anchor)..self.caret.max(self.anchor)
    }

    fn replace_selection(&mut self, text: &str) {
        let range = self.selection();
        self.caret = range.start + text.len();
        self.anchor = self.caret;
        self.query.replace_range(range, text);
        self.blink = true;
    }

    fn previous(&self) -> usize {
        self.query[..self.caret].char_indices().next_back().map_or(0, |(i, _)| i)
    }

    fn next(&self) -> usize {
        self.caret + self.query[self.caret..].chars().next().map_or(0, char::len_utf8)
    }

    fn click_position(&self, screen_x: i32) -> usize {
        let field = self.rect(Part::Query).unwrap();
        let x = screen_x - field.x - 3 + self.scroll;
        for edges in self.positions.windows(2) {
            if 2 * x < edges[0].1 + edges[1].1 {
                return edges[0].0;
            }
        }
        self.query.len()
    }

    fn label(&self, part: Part) -> &'static str {
        match part {
            Part::Query => "Fi&nd what:",
            Part::WholeWord => "Match &whole word only",
            Part::MatchCase => "Match &case",
            Part::Up => "&Up",
            Part::Down => "&Down",
            Part::Page if self.era == Era::Win2000 => "Print the &selected topic",
            Part::Heading if self.era == Era::Win2000 => "&Print the selected heading and all subtopics",
            Part::Page => "Print the &current page",
            Part::Heading => "Print &everything contained in the current heading",
            Part::Ok if self.is_find() => "&Find Next",
            Part::Ok => "OK",
            Part::Cancel => "Cancel",
            _ => "",
        }
    }

    fn checked(&self, part: Part) -> bool {
        match part {
            Part::WholeWord => self.options.whole_word,
            Part::MatchCase => self.options.match_case,
            Part::Up => !self.options.down,
            Part::Down => self.options.down,
            Part::Page => !self.heading,
            Part::Heading => self.heading,
            _ => false,
        }
    }

    fn paint_option(&self, fb: &mut Framebuffer, theme: &Theme, font: &Font, part: Part) {
        let r = self.rect(part).unwrap();
        let c = &theme.colors;
        let checkbox = matches!(part, Part::WholeWord | Part::MatchCase);
        let checked = self.checked(part);
        let pressed = self.pressed == Some(part);
        let x = r.x;
        let y = r.y + (r.h - 13) / 2;
        if checkbox {
            properties::check_box(fb, c, x, y, checked, pressed, true);
        } else {
            let art = if checked { &dialog::OPTION_ON } else { &dialog::OPTION_OFF };
            dialog::art(fb, art, x, y, |b| match b {
                b'g' => Some(c.shadow),
                b'#' => Some(c.dark_shadow),
                b'W' => Some(if pressed { c.face } else { c.highlight }),
                b'l' => Some(c.light),
                _ => None,
            });
            if checked {
                // The mark itself is button text black, even on 2000's charcoal bevel.
                for (dy, (dx, w)) in [(5, 2), (4, 4), (4, 4), (5, 2)].into_iter().enumerate() {
                    fb.hline(x + dx, y + 4 + dy as i32, w, c.text);
                }
            }
        }
        let label = self.label(part);
        // MSHTML positions each LABEL independently of its INPUT.
        let (tx, ty) = match part {
            Part::WholeWord => (31, 67),
            Part::MatchCase => (31, 86 + i32::from(self.era == Era::Win2000)),
            Part::Up => (176, 79),
            Part::Down => (217, 79),
            _ => (x + 17, r.y + (r.h - font.height) / 2),
        };
        font.draw_label(fb, (tx, ty), label, i32::MAX, c.text, self.is_find() || self.keys);
        if self.active && self.focus == part && self.keys {
            let focus = Rect::new(tx - 1, ty - 1, font.measure_label(label) + 2, font.height + 2);
            dialog::focus_dots(fb, focus, (r.x, r.y), if self.is_find() { rgb(!c.face) } else { c.text });
        }
    }

    fn paint_query(&self, fb: &mut Framebuffer, theme: &Theme, font: &Font) {
        let c = &theme.colors;
        let r = self.rect(Part::Query).unwrap();
        let mut bevel = *c;
        bevel.light = c.face;
        let inner = dialog::well(fb, r, &bevel);
        let mut strip = Framebuffer::new(inner.w - 2, font.height, rgb(0xffffff));
        let (mut x, selected) = (-self.scroll, self.selection());
        for (i, ch) in self.query.char_indices() {
            let selected = self.focus == Part::Query && selected.contains(&i);
            let width = font.glyph(ch).advance;
            if selected {
                strip.fill(Rect::new(x, 0, width, font.height), c.menu_highlight);
            }
            let mut bytes = [0; 4];
            font.draw(
                &mut strip,
                x,
                0,
                ch.encode_utf8(&mut bytes),
                if selected { c.menu_highlight_text } else { c.text },
            );
            x += width;
        }
        if self.active && self.blink && self.focus == Part::Query {
            strip.vline(font.measure(&self.query[..self.caret]) - self.scroll, 0, font.height, c.text);
        }
        fb.blit(&strip, inner.x + 1, inner.y + 1);
    }

    fn tip(part: Part) -> Option<&'static str> {
        Some(match part {
            Part::Query => "Provides a space for you to type the text you want to search for.",
            Part::WholeWord => concat!(
                "Specifies whether you want Internet Explorer to search for whole word occurrences of the text ",
                "typed in the Find What box, or occurrences that might also be part of a larger word.\r\n\r\n",
                "For example, if you want to find the word “this” and specify whole word matches only, ",
                "then larger words such as “thistle” are ignored while searching."
            ),
            Part::MatchCase => concat!(
                "Specifies whether you want Internet Explorer to distinguish between uppercase and lowercase ",
                "characters when searching for occurrences of the text typed in the Find What box."
            ),
            Part::Up => concat!(
                "Specifies that you want to search toward the beginning of the document for occurrences of the ",
                "text typed in the Find What box."
            ),
            Part::Down => concat!(
                "Specifies that you want to search toward the end of the document for occurrences of the ",
                "text typed in the Find What box."
            ),
            Part::Ok => "Finds and selects the next occurrence of the text typed in the Find What box.",
            Part::Cancel => "Closes the dialog box without saving any changes you have made.",
            _ => return None,
        })
    }
}

impl Form for CommandDialog {
    type Part = Part;

    fn title(&self) -> &str {
        match self.kind {
            Kind::Find => "Find",
            Kind::Finished => "Microsoft Internet Explorer",
            Kind::Print if self.era == Era::Win2000 => "Print Topics",
            Kind::Print => "Print",
        }
    }
    fn size(&self) -> (i32, i32) {
        match self.kind {
            Kind::Find => (357, 119),
            Kind::Print if self.era == Era::Win2000 => (305, 175),
            Kind::Print => (294, 162),
            Kind::Finished => (if self.era == Era::Win2000 { 236 } else { 234 }, 119),
        }
    }

    fn layout(&mut self, _fonts: &Fonts) {
        if !self.is_find() {
            return;
        }
        let font = ms_sans_serif();
        let mut x = 0;
        self.positions.clear();
        for (i, ch) in self.query.char_indices() {
            self.positions.push((i, x));
            x += font.glyph(ch).advance;
        }
        self.positions.push((self.query.len(), x));
        let caret_x = font.measure(&self.query[..self.caret]);
        let visible = self.rect(Part::Query).unwrap().w - 7;
        self.scroll = self.scroll.min(caret_x).max(caret_x - visible).min((x - visible).max(0));
    }

    fn paint(&self, fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts) {
        let title = |button, part| ButtonLook {
            button,
            rect: window::title_button_rect(self.size().0, button),
            pressed: self.pressed == Some(part),
            enabled: self.enabled(part),
        };
        let buttons = [title(TitleButton::Help, Part::Help), title(TitleButton::Close, Part::Close)];
        let modern = self.era == Era::Win2000;
        let icon = (self.is_find() && modern).then(|| {
            let mut icon = IE_DOCUMENT.icon();
            for p in &mut icon.pixels {
                // The transparent pixels in this icon's XOR bitmap carry 0x040404.
                if *p == 0 {
                    *p = rgb(if self.active { 0x0e206e } else { 0x848484 });
                }
            }
            icon
        });
        window::paint_frame(
            fb,
            theme,
            fonts,
            &Frame {
                title: self.title(),
                era: self.era,
                active: self.active,
                icon: icon.as_ref(),
                buttons: if self.is_find() { &buttons } else { &buttons[1..] },
            },
        );
        let font = if self.kind == Kind::Finished { fonts.regular } else { ms_sans_serif() };
        let c = &theme.colors;
        if self.is_find() {
            // The MSHTML template explicitly underlines its access keys in the HTML.
            font.draw_label(fb, (13, 35), self.label(Part::Query), i32::MAX, c.text, true);
            self.paint_query(fb, theme, font);
            let top = if modern { 64 } else { 66 };
            let group = Rect::new(149, top, 103, 104 - top);
            fb.frame(group, c.shadow, c.highlight);
            fb.frame(group.inset(1, 1, 1, 1), c.highlight, c.shadow);
            fb.fill(Rect::new(156, top, font.measure("Direction") + 4, 2), c.face);
            font.draw(fb, 158, top - 5, "Direction", c.text);
        } else if self.kind == Kind::Print {
            let (x, width, prompt) = if modern {
                (
                    17,
                    273,
                    "You can print the selected topic or all the topics in the selected heading. What would you like to do?",
                )
            } else {
                (
                    14,
                    254,
                    "You can print the current topic or all the topics in the current heading. What would you like to do?",
                )
            };
            for (line, text) in dialog::wrap(font, prompt, width).into_iter().enumerate() {
                font.draw(fb, x, 33 + line as i32 * font.height, &text, c.text);
            }
        } else {
            crate::colors16::draw_picture(fb, theme, &crate::icons::WARNING.icon(), 14, 33);
            font.draw(fb, if modern { 65 } else { 62 }, 42, "Finished searching the document.", c.text);
        }
        for &part in self.parts() {
            match part {
                Part::WholeWord | Part::MatchCase | Part::Up | Part::Down | Part::Page | Part::Heading => {
                    self.paint_option(fb, theme, font, part);
                }
                Part::Ok | Part::Cancel => {
                    let r = self.rect(part).unwrap();
                    let mut button_theme = *theme;
                    if self.is_find() {
                        button_theme.colors.light = c.face;
                    }
                    let label = if self.is_find() { "" } else { self.label(part) };
                    dialog::push_button(
                        fb,
                        &button_theme,
                        font,
                        r,
                        label,
                        self.enter() == part && self.enabled(part),
                        self.pressed == Some(part),
                        !self.is_find() && self.active && self.keys && self.focus == part,
                        self.enabled(part),
                        self.keys,
                    );
                    if self.is_find() {
                        if self.active && self.keys && self.focus == part {
                            // MSHTML's focus rectangle inverts the background under each dot.
                            dialog::focus_dots(fb, r.inset(4, 4, 4, 4), (r.x, r.y), rgb(!c.face));
                        }
                        let x = r.x
                            + (r.w - font.measure_label(self.label(part))) / 2
                            + i32::from(self.pressed == Some(part));
                        let y = r.y + (r.h - font.height) / 2 + i32::from(self.pressed == Some(part));
                        if !self.enabled(part) {
                            font.draw_label(fb, (x + 1, y + 1), self.label(part), i32::MAX, c.etch, true);
                        }
                        let ink = if self.enabled(part) { c.text } else { c.shadow };
                        font.draw_label(fb, (x, y), self.label(part), i32::MAX, ink, true);
                    }
                }
                _ => {}
            }
        }
    }

    fn hit(&self, x: i32, y: i32) -> Option<Part> {
        self.parts().iter().copied().find(|&part| self.rect(part).is_some_and(|r| r.contains(x, y)))
    }

    fn rect(&self, part: Part) -> Option<Rect> {
        let modern = i32::from(self.era == Era::Win2000);
        if self.kind == Kind::Finished {
            return match part {
                Part::Close => Some(window::title_button_rect(self.size().0, TitleButton::Close)),
                Part::Ok => Some(Rect::new(80, 82, 75, 23)),
                _ => None,
            };
        }
        Some(match (self.is_find(), part) {
            (_, Part::Close) => window::title_button_rect(self.size().0, TitleButton::Close),
            (true, Part::Help) => window::title_button_rect(self.size().0, TitleButton::Help),
            (true, Part::Query) => Rect::new(68 + modern, 33, 185, 22 + modern),
            (true, Part::Ok) => Rect::new(266 + modern, 33, 75, 23),
            (true, Part::Cancel) => Rect::new(266 + modern, 60 + modern, 75, 23),
            (true, Part::WholeWord) => Rect::new(13 + modern, 66, 133, 16),
            (true, Part::MatchCase) => Rect::new(13 + modern, 85, 133, 16),
            (true, Part::Up) => Rect::new(159 + modern, 79, 40, 16),
            (true, Part::Down) => Rect::new(201, 79, 48, 16),
            (false, Part::Ok) if modern != 0 => Rect::new(134, 137, 75, 23),
            (false, Part::Cancel) if modern != 0 => Rect::new(215, 137, 75, 23),
            (false, Part::Page) if modern != 0 => Rect::new(18, 81, 132, 16),
            (false, Part::Heading) if modern != 0 => Rect::new(18, 102, 230, 16),
            (false, Part::Ok) => Rect::new(117, 124, 75, 23),
            (false, Part::Cancel) => Rect::new(206, 124, 75, 23),
            (false, Part::Page) => Rect::new(15, 71, 126, 16),
            (false, Part::Heading) => Rect::new(15, 92, 254, 16),
            _ => return None,
        })
    }

    fn focus(&self) -> Part {
        self.focus
    }
    fn set_focus(&mut self, part: Part) {
        self.focus = part;
        if part == Part::Query {
            self.anchor = 0;
            self.caret = self.query.len();
            self.blink = true;
        }
    }
    fn set_active(&mut self, active: bool) {
        self.active = active;
    }
    fn set_pressed(&mut self, part: Option<Part>) {
        self.pressed = part;
    }
    fn enabled(&self, part: Part) -> bool {
        match part {
            Part::Ok if self.is_find() => !self.query.is_empty(),
            Part::Close if self.kind == Kind::Finished && self.era == Era::Win98 => false,
            _ => true,
        }
    }
    fn show_keys(&mut self) {
        self.keys = true;
    }
    fn has_caret(&self) -> bool {
        self.focus == Part::Query
    }
    fn blink(&mut self, on: bool) {
        self.blink = on;
    }
    fn context_help(&self) -> bool {
        self.help
    }

    fn tab_stops(&self) -> Vec<Part> {
        if self.kind == Kind::Finished {
            return vec![Part::Ok];
        }
        let mut stops = if self.is_find() {
            vec![
                Part::Query,
                Part::WholeWord,
                Part::MatchCase,
                if self.options.down { Part::Down } else { Part::Up },
                Part::Ok,
                Part::Cancel,
            ]
        } else {
            vec![Part::Ok, Part::Cancel, if self.heading { Part::Heading } else { Part::Page }]
        };
        stops.retain(|&part| self.enabled(part));
        stops
    }

    fn enter(&self) -> Part {
        if self.focus == Part::Cancel { Part::Cancel } else { Part::Ok }
    }
    fn escape(&self) -> Part {
        Part::Cancel
    }

    fn key(&mut self, key: Key, mods: Mods) -> Option<Vec<Effect<Part>>> {
        if self.help && key == Key::Escape {
            self.help = false;
            return Some(Vec::new());
        }
        if self.is_find() && key == Key::Other(0x70) {
            if mods.shift {
                self.help = true;
            } else if let Some(tip) = Self::tip(self.focus) {
                return Some(vec![Effect::Activate(Part::Tip(tip))]);
            }
            return Some(Vec::new());
        }
        let arrows = matches!(key, Key::Left | Key::Up | Key::Right | Key::Down);
        if arrows && matches!(self.focus, Part::Up | Part::Down | Part::Page | Part::Heading) {
            let next = match self.focus {
                Part::Up => Part::Down,
                Part::Down => Part::Up,
                Part::Page => Part::Heading,
                _ => Part::Page,
            };
            self.set_focus(next);
            return Some(vec![Effect::Activate(next)]);
        }
        if self.focus != Part::Query {
            return None;
        }
        let selection = self.selection();
        match key {
            Key::Left if !mods.shift && !selection.is_empty() => self.caret = selection.start,
            Key::Right if !mods.shift && !selection.is_empty() => self.caret = selection.end,
            Key::Left => self.caret = self.previous(),
            Key::Right => self.caret = self.next(),
            Key::Home => self.caret = 0,
            Key::End => self.caret = self.query.len(),
            Key::Delete => {
                if selection.is_empty() {
                    self.anchor = self.next();
                }
                self.replace_selection("");
                return Some(Vec::new());
            }
            Key::Other(0x41) if mods.ctrl => {
                self.set_focus(Part::Query);
                return Some(Vec::new());
            }
            Key::Other(0x56) if mods.ctrl => return Some(vec![Effect::Paste]),
            _ => return None,
        }
        if !mods.shift {
            self.anchor = self.caret;
        }
        self.blink = true;
        Some(Vec::new())
    }

    fn typed(&mut self, ch: char) -> Vec<Effect<Part>> {
        if self.focus == Part::Query {
            if ch == '\u{8}' {
                if self.caret == self.anchor {
                    self.anchor = self.previous();
                }
                self.replace_selection("");
            } else if !ch.is_control() {
                let mut bytes = [0; 4];
                self.replace_selection(ch.encode_utf8(&mut bytes));
            }
        } else if ch.is_alphabetic() {
            return self.access(ch);
        }
        Vec::new()
    }

    fn paste(&mut self, text: &str) {
        if self.focus == Part::Query {
            let line = text.split(['\r', '\n']).next().unwrap_or("");
            self.replace_selection(&line.chars().filter(|c| !c.is_control()).collect::<String>());
        }
    }

    fn access(&mut self, ch: char) -> Vec<Effect<Part>> {
        let target = self.parts().iter().copied().find(|&p| {
            self.label(p)
                .split_once('&')
                .and_then(|(_, s)| s.chars().next())
                .is_some_and(|c| c.eq_ignore_ascii_case(&ch))
        });
        if let Some(part) = target.filter(|&p| self.enabled(p)) {
            self.set_focus(part);
            if part != Part::Query {
                return vec![Effect::Activate(part)];
            }
        }
        Vec::new()
    }

    fn space(&mut self) -> Vec<Effect<Part>> {
        if self.focus == Part::Query { Vec::new() } else { kit::activate(self, self.focus) }
    }

    fn press(&mut self, part: Option<Part>, x: i32, _y: i32) -> Press<Part> {
        if std::mem::take(&mut self.help) {
            return part.and_then(Self::tip).map_or(Press::Done, |tip| Press::Act(Part::Tip(tip)));
        }
        match part {
            Some(Part::Query) if self.focus != Part::Query => {
                // The Find document selects the whole query whenever its edit gains focus.
                self.select_query();
                Press::Done
            }
            Some(Part::Query) => {
                self.focus = Part::Query;
                self.caret = self.click_position(x);
                self.anchor = self.caret;
                self.blink = true;
                Press::Grab(Part::Query, 0)
            }
            Some(p) if self.enabled(p) => {
                if !matches!(p, Part::Close | Part::Help) {
                    self.set_focus(p);
                }
                Press::Hold(p)
            }
            _ => Press::Nothing,
        }
    }

    fn drag(&mut self, part: Part, _grab_y: i32, _value: i32, x: i32, _y: i32) -> bool {
        if part != Part::Query {
            return false;
        }
        let caret = self.click_position(x);
        let changed = self.caret != caret;
        self.caret = caret;
        changed
    }

    fn button(&mut self, button: Button, down: bool, x: i32, y: i32) -> Vec<Effect<Part>> {
        // The HTML Find document binds right-click on its controls to their WinHelp topics.
        if self.is_find()
            && button == Button::Right
            && !down
            && let Some(part) = self.hit(x, y)
            && part != Part::Query
            && let Some(tip) = Self::tip(part)
        {
            return vec![Effect::Activate(Part::Tip(tip))];
        }
        Vec::new()
    }
}

const IE_DOCUMENT: ClassicIcon = ClassicIcon::art("html-help/ie-document", Look::Win98);



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_edits_a_selection_and_returns_the_chosen_options() {
        let mut f = CommandDialog::find(Era::Win98);
        assert!(!f.enabled(Part::Ok));
        f.paste("a\u{e9}bc");
        f.key(Key::Left, Mods::default());
        f.key(Key::Left, Mods { shift: true, ctrl: false });
        f.key(Key::Left, Mods { shift: true, ctrl: false });
        f.typed('x');
        assert_eq!(f.query(), "axc");
        assert!(f.enabled(Part::Ok));
        f.activate(Part::WholeWord);
        f.activate(Part::MatchCase);
        f.activate(Part::Up);
        assert_eq!(f.find_options(), FindOptions { whole_word: true, match_case: true, down: false });
    }

    #[test]
    fn print_radio_group_has_one_tab_stop() {
        let mut f = CommandDialog::print(Era::Win2000);
        assert!(!f.include_subtopics());
        f.activate(Part::Heading);
        assert!(f.include_subtopics());
        assert!(f.tab_stops().contains(&Part::Heading));
        assert!(!f.tab_stops().contains(&Part::Page));
    }
}
