//! 98's and 2000's Pattern window (Display Properties, Background, Pattern...): the sentence, the
//! Pattern list, the Preview box showing the pattern 1:1 in black on the desktop color, OK,
//! Cancel and Edit Pattern... (grayed for now: the Pattern Editor is a later step). From the dialog
//! template (98's SHDOCVW.DLL 1309, 2000's SHELL32.DLL 29955, the same), 318x205 outside.

use crate::dialog::{self, TitleButtons};
use crate::display::form::{BOX_ROW, List, du, paint_box};
use crate::display::{self, Pattern};
use crate::fb::{Framebuffer, Rect};
use crate::kit::{self, Effect, Key, Mods, Press};
use crate::text::Fonts;
use crate::theme::{Era, Theme};

pub const SIZE: (i32, i32) = (318, 205);
/// Where the template starts in the window (the frame and the title bar above it).
const ORIGIN: (i32, i32) = (3, 22);
const SENTENCE: &str = "You can choose a pattern for your Active Desktop.  The pattern is used to fill any leftover space around your wallpaper.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    TitleHelp,
    TitleClose,
    List,
    Row(usize),
    ScrollUp,
    ScrollDown,
    ScrollPageUp,
    ScrollPageDown,
    ScrollThumb,
    Ok,
    Cancel,
    EditPattern,
}

impl Part {
    fn is_push_button(self) -> bool {
        matches!(self, Part::Ok | Part::Cancel | Part::EditPattern)
    }
}

/// The Pattern window's state.
#[derive(Clone, Debug)]
pub struct PatternWindow {
    pub era: Era,
    patterns: Vec<Pattern>,
    names: Vec<String>,
    list: List,
    pub focus: Part,
    pub pressed: Option<Part>,
    pub show_keys: bool,
    /// The selected row shows highlighted. Windows 98 opened on "(None)" with only the focus
    /// rectangle on it (nothing selected) until a row was chosen; 2000 highlighted it.
    highlighted: bool,
}

impl PatternWindow {
    /// Opens on `chosen` (the page's pattern; `None`: "(None)").
    pub fn new(era: Era, chosen: Option<&str>) -> PatternWindow {
        let patterns = display::patterns(era);
        let names: Vec<String> = std::iter::once("(None)".to_string()).chain(patterns.iter().map(|p| p.name.to_string())).collect();
        let sel = chosen.and_then(|n| names.iter().position(|x| x == n)).unwrap_or(0);
        let mut list = List { rect: du(ORIGIN, 7, 54, 80, 50), row_h: BOX_ROW, rows: names.len(), sel, top: 0 };
        list.show_sel();
        let highlighted = !(era == Era::Win98 && sel == 0);
        PatternWindow { era, patterns, names, list, focus: Part::List, pressed: None, show_keys: false, highlighted }
    }

    /// The pattern chosen in the list (`None`: "(None)"): what OK hands back.
    pub fn chosen(&self) -> Option<String> {
        (self.list.sel > 0).then(|| self.names[self.list.sel].clone())
    }

    fn pattern(&self) -> Option<Pattern> {
        self.list.sel.checked_sub(1).and_then(|i| self.patterns.get(i).copied())
    }

    fn parts(&self) -> Vec<(Part, Rect)> {
        let (help, close) = dialog::title_button_rects(SIZE.0);
        let mut parts = vec![(Part::TitleHelp, help), (Part::TitleClose, close)];
        if let Some(bar) = self.list.scroll() {
            let r = bar.rects(self.list.top);
            parts.extend([
                (Part::ScrollUp, r.up),
                (Part::ScrollDown, r.down),
                (Part::ScrollThumb, r.thumb),
                (Part::ScrollPageUp, r.page_up),
                (Part::ScrollPageDown, r.page_down),
            ]);
        }
        parts.extend(self.list.shown_rows().into_iter().map(|(i, r)| (Part::Row(i), r)));
        parts.push((Part::List, self.list.rect));
        parts.push((Part::Ok, du(ORIGIN, 151, 7, 50, 14)));
        parts.push((Part::Cancel, du(ORIGIN, 151, 25, 50, 14)));
        parts.push((Part::EditPattern, du(ORIGIN, 151, 43, 50, 14)));
        parts
    }

    fn enabled(&self, part: Part) -> bool {
        // Edit Pattern... is a later step (Windows grayed it only for "(None)").
        part != Part::EditPattern
    }

    pub fn paint(&self, fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts) {
        let c = &theme.colors;
        let font = if self.era == Era::Win2000 { crate::text::ms_sans_serif() } else { fonts.regular };
        let keys = self.show_keys;
        let parts = self.parts();
        let rect = |part: Part| parts.iter().find(|(p, _)| *p == part).map(|&(_, r)| r);
        let held = |part: Part| self.pressed == Some(part);
        let buttons = TitleButtons {
            help: rect(Part::TitleHelp).map(|r| (r, held(Part::TitleHelp))),
            close: rect(Part::TitleClose).map(|r| (r, held(Part::TitleClose))),
        };
        dialog::window_frame(fb, theme, fonts, self.era, "Pattern", &buttons);
        let s = du(ORIGIN, 7, 7, 137, 30);
        for (i, line) in dialog::wrap(font, SENTENCE, s.w).iter().enumerate() {
            font.draw(fb, s.x, s.y + i as i32 * font.height, line, c.text);
        }
        let label = du(ORIGIN, 7, 44, 80, 10);
        font.draw_label(fb, (label.x, label.y), "&Pattern:", i32::MAX, c.text, keys);
        let preview_label = du(ORIGIN, 94, 44, 50, 10);
        font.draw(fb, preview_label.x, preview_label.y, "Preview:", c.text);
        let held_bar = match self.pressed {
            Some(Part::ScrollUp) => Some(crate::display::form::Bar::Up),
            Some(Part::ScrollDown) => Some(crate::display::form::Bar::Down),
            _ => None,
        };
        paint_box(fb, theme, font, &self.list, &self.names, self.focus == Part::List && keys, held_bar, self.highlighted);
        // The preview: a black frame, the pattern 1:1 inside on the desktop color.
        let p = du(ORIGIN, 94, 54, 50, 50);
        fb.frame(p, c.dark_shadow, c.dark_shadow);
        let inside = p.inset(1, 1, 1, 1);
        let pattern = self.pattern();
        for y in 0..inside.h {
            for x in 0..inside.w {
                // Lined up with the window's inside (the dialog's client area; the Reference PCs).
                let set = pattern.is_some_and(|pt| pt.set(inside.x + x - ORIGIN.0, inside.y + y - ORIGIN.1));
                fb.set(inside.x + x, inside.y + y, if set { 0xFF00_0000 } else { c.desktop });
            }
        }
        let framed = if self.focus.is_push_button() { self.focus } else { Part::Ok };
        for (part, label) in [(Part::Ok, "OK"), (Part::Cancel, "Cancel"), (Part::EditPattern, "&Edit Pattern...")] {
            if let Some(r) = rect(part) {
                let focused = self.focus == part && keys;
                dialog::push_button(fb, theme, font, r, label, framed == part, held(part), focused, self.enabled(part), keys);
            }
        }
    }
}

impl kit::Form for PatternWindow {
    type Part = Part;

    fn title(&self) -> &str {
        "Pattern"
    }
    fn size(&self) -> (i32, i32) {
        SIZE
    }
    fn layout(&mut self, _fonts: &Fonts) {}
    fn paint(&self, fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts) {
        PatternWindow::paint(self, fb, theme, fonts);
    }
    fn hit(&self, x: i32, y: i32) -> Option<Part> {
        self.parts().into_iter().find(|(_, r)| r.contains(x, y)).map(|(p, _)| p)
    }
    fn rect(&self, part: Part) -> Option<Rect> {
        self.parts().into_iter().find(|(p, _)| *p == part).map(|(_, r)| r)
    }
    fn enabled(&self, part: Part) -> bool {
        PatternWindow::enabled(self, part)
    }
    fn focus(&self) -> Part {
        self.focus
    }
    fn set_focus(&mut self, part: Part) {
        self.focus = part;
    }
    fn tab_stops(&self) -> Vec<Part> {
        [Part::List, Part::Ok, Part::Cancel, Part::EditPattern].into_iter().filter(|&p| self.enabled(p)).collect()
    }
    fn set_pressed(&mut self, part: Option<Part>) {
        self.pressed = part;
    }
    fn show_keys(&mut self) {
        self.show_keys = true;
    }
    fn enter(&self) -> Part {
        if self.focus.is_push_button() { self.focus } else { Part::Ok }
    }
    fn escape(&self) -> Part {
        Part::Cancel
    }
    fn key(&mut self, key: Key, _mods: Mods) -> Option<Vec<Effect<Part>>> {
        if self.focus != Part::List {
            return None;
        }
        let page = self.list.page() as i32;
        let delta = match key {
            Key::Up | Key::Left => -1,
            Key::Down | Key::Right => 1,
            Key::PageUp => -(page - 1).max(1),
            Key::PageDown => (page - 1).max(1),
            Key::Home => -10_000,
            Key::End => 10_000,
            _ => return None,
        };
        self.list.step(delta);
        self.highlighted = true;
        Some(Vec::new())
    }
    fn typed(&mut self, ch: char) -> Vec<Effect<Part>> {
        if !ch.is_alphanumeric() {
            return Vec::new();
        }
        self.show_keys = true;
        if self.focus == Part::List {
            if let Some(i) = self.list.find(&self.names, ch) {
                self.list.sel = i;
                self.list.show_sel();
                self.highlighted = true;
            }
            return Vec::new();
        }
        self.access(ch)
    }
    fn access(&mut self, ch: char) -> Vec<Effect<Part>> {
        match ch.to_ascii_lowercase() {
            'p' => self.focus = Part::List,
            'e' if self.enabled(Part::EditPattern) => return vec![Effect::Activate(Part::EditPattern)],
            _ => {}
        }
        Vec::new()
    }
    fn space(&mut self) -> Vec<Effect<Part>> {
        if self.focus.is_push_button() { kit::activate(self, self.focus) } else { Vec::new() }
    }
    fn press(&mut self, part: Option<Part>, _x: i32, _y: i32) -> Press<Part> {
        match part {
            Some(Part::Row(i)) => {
                self.focus = Part::List;
                self.list.sel = i;
                self.list.show_sel();
                self.highlighted = true;
            }
            Some(Part::List) => self.focus = Part::List,
            Some(p @ (Part::ScrollUp | Part::ScrollDown)) => {
                let top = self.list.top as i32 + if p == Part::ScrollUp { -1 } else { 1 };
                self.list.set_top(top.max(0) as usize);
                return Press::Hold(p);
            }
            Some(p @ (Part::ScrollPageUp | Part::ScrollPageDown)) => {
                let page = self.list.page() as i32;
                let top = self.list.top as i32 + if p == Part::ScrollPageUp { -page } else { page };
                self.list.set_top(top.max(0) as usize);
            }
            Some(Part::ScrollThumb) => return Press::Grab(Part::ScrollThumb, self.list.top as i32),
            Some(p) if self.enabled(p) => {
                if p.is_push_button() {
                    self.focus = p;
                }
                return Press::Hold(p);
            }
            Some(_) => {}
            None => return Press::Nothing,
        }
        Press::Done
    }
    fn drag(&mut self, _part: Part, grab_y: i32, grab_top: i32, _x: i32, y: i32) -> bool {
        let Some(bar) = self.list.scroll() else { return false };
        let top = bar.dragged(grab_top, y - grab_y);
        let changed = top != self.list.top;
        self.list.set_top(top);
        changed
    }
    fn wheel(&mut self, notches: i32) {
        let top = (self.list.top as i32 - 3 * notches).max(0) as usize;
        self.list.set_top(top);
    }
}
