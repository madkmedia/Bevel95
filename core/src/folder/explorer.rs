//! The top of a Windows 98 or 2000 folder window: Explorer's rebar with its three bands (the menu
//! bar with the throbber at its right end, the toolbar, the address bar), and the three-part
//! status bar at the bottom. Measured on the Reference PCs with the web view switched off
//! (`private/reference/refs/vm/win98-truecolor-folder-*`, `win2000-truecolor-folder-*`; numbers in
//! docs/theme-spec.md, "Folder windows (Windows 98 and 2000)").
//!
//! - The rebar is an etched rectangle from the frame's inside edge, one row below the title bar;
//!   its bands are separated by etched lines and each begins with a raised gripper. The menu band
//!   is 22 rows, the toolbar 40 (98: pictures above their names) or 22 (2000: small pictures,
//!   names to the right of some), the address bar 22. The window's list begins 4 rows below.
//! - Toolbar buttons are flat: raised by one pixel under the pointer, sunken while pressed (the
//!   picture and name one pixel down and right), their pictures gray until the pointer comes, in
//!   color then, etched while grayed. Back and Forward have a little arrow beside them that opens
//!   the list of places.
//! - Menu titles are flat buttons too: raised under the pointer, sunken while their menu is open.
//!
//! Only what works is shown (brief of 2026-11-13): 98's toolbar is Back, Forward, Up | Cut, Copy,
//! Paste | Undo | Delete, Properties | Views, 2000's Back, Forward, Up | Folders | Delete, Undo |
//! Views (Search, History, Move To and Copy To left out). Undo (2026-11-16) never shows grayed: the
//! PCs showed it usable in every capture, also in fresh windows with nothing to take back.

use super::explorer_art;
use crate::fb::{Framebuffer, Icon, Rect};
use crate::text::{Font, Fonts};
use crate::theme::{Era, Theme};

/// A toolbar button.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tool {
    Back,
    Forward,
    Up,
    Cut,
    Copy,
    Paste,
    /// Undo: what Edit > Undo does (98 between Paste and Delete, 2000 after Delete).
    Undo,
    Delete,
    Properties,
    /// 2000's Folders: shows or hides the tree (pushed in while it shows).
    Folders,
    /// Views: 98's button goes to the next view, its arrow opens the views' menu; 2000's button
    /// is all one drop-down (`*-view-views-*`).
    Views,
}

impl Tool {
    /// Its name under (98) or beside (2000's Back) the picture.
    pub fn label(self) -> &'static str {
        match self {
            Tool::Back => "Back",
            Tool::Forward => "Forward",
            Tool::Up => "Up",
            Tool::Cut => "Cut",
            Tool::Copy => "Copy",
            Tool::Paste => "Paste",
            Tool::Undo => "Undo",
            Tool::Delete => "Delete",
            Tool::Properties => "Properties",
            Tool::Folders => "Folders",
            Tool::Views => "Views",
        }
    }

    fn word(self) -> &'static str {
        match self {
            Tool::Back => "back",
            Tool::Forward => "forward",
            Tool::Up => "up",
            Tool::Cut => "cut",
            Tool::Copy => "copy",
            Tool::Paste => "paste",
            Tool::Undo => "undo",
            Tool::Delete => "delete",
            Tool::Properties => "properties",
            Tool::Folders => "folders",
            Tool::Views => "views",
        }
    }

    /// Back and Forward have a little arrow beside them.
    pub fn split(self) -> bool {
        matches!(self, Tool::Back | Tool::Forward | Tool::Views)
    }

    /// The tooltip and status text of Windows' buttons (not shown yet).
    fn _tip(self) -> &'static str {
        self.label()
    }
}

/// The toolbar's buttons in order (`None`: a separator).
pub fn toolbar(era: Era) -> &'static [Option<Tool>] {
    use Tool::*;
    match era {
        Era::Win2000 => &[Some(Back), Some(Forward), Some(Up), None, Some(Folders), None, Some(Delete), Some(Undo), None, Some(Views)],
        _ => &[Some(Back), Some(Forward), Some(Up), None, Some(Cut), Some(Copy), Some(Paste), None, Some(Undo), None, Some(Delete), Some(Properties), None, Some(Views)],
    }
}

/// The toolbar band's height.
fn tool_band_h(era: Era) -> i32 {
    if era == Era::Win2000 { 22 } else { 40 }
}

/// The height of every other band.
const BAND_H: i32 = 22;
/// The throbber's size, at the menu band's right end.
const THROBBER: (i32, i32) = (38, 22);
/// The status bar's height (2 face rows above 18-row parts) and its fixed parts' widths.
pub const STATUS_H: i32 = 20;
/// The first band item's distance from the frame's inside edge (after the gripper).
const ITEMS_X: i32 = 11;
/// How much wider than its name a menu title is, and its height.
const TITLE_PAD: i32 = 8;
const TITLE_H: i32 = 19;

/// A toolbar button's place: the button and, for Back and Forward, its arrow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Button {
    pub tool: Tool,
    pub main: Rect,
    pub arrow: Option<Rect>,
}

/// Where everything of the top and the bottom is (window pixels at 1Ã—).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Chrome {
    /// The etched rectangle around the bands.
    pub rebar: Rect,
    pub menu_band: Rect,
    pub tool_band: Rect,
    pub address_band: Rect,
    pub throbber: Rect,
    pub titles: Vec<Rect>,
    pub buttons: Vec<Button>,
    pub separators: Vec<Rect>,
    /// "Address" at this point, the combo box, its arrow, 2000's Go button.
    pub label_at: (i32, i32),
    pub combo: Rect,
    pub combo_button: Rect,
    pub go: Option<Rect>,
}

impl Chrome {
    /// The list's top edge.
    pub fn list_top(&self) -> i32 {
        self.rebar.bottom() + 3
    }

    /// The combo box's inside (white).
    pub fn combo_inside(&self) -> Rect {
        self.combo.inset(2, 2, 2, 2)
    }

    /// Where the address bar's picture and its text go.
    pub fn address_icon_at(&self) -> (i32, i32) {
        let i = self.combo_inside();
        (i.x + 1, i.y + 1)
    }

    pub fn address_text_at(&self) -> (i32, i32) {
        let i = self.combo_inside();
        (i.x + 21, i.y + 3)
    }
}

/// The width of a toolbar button and of its arrow.
fn button_size(era: Era, tool: Tool, font: &Font) -> (i32, Option<i32>) {
    match (era, tool) {
        (Era::Win2000, Tool::Back) => (23 + font.measure("Back") + 5, Some(13)),
        // 66 wide on the PC (`win2000-truecolor-tree-folders-off-cold`): its name 23 in, as Back's.
        (Era::Win2000, Tool::Folders) => (23 + font.measure("Folders") + 8, None),
        (Era::Win2000, Tool::Forward) => (24, Some(13)),
        // One drop-down 31 wide, its arrow inside (`win2000-truecolor-view-views-hot`).
        (Era::Win2000, Tool::Views) => (31, None),
        (Era::Win2000, _) => (24, None),
        (_, t) if t.split() => (53, Some(14)),
        _ => (54, None),
    }
}

/// Lays the top out for a window `width` wide with a frame of `frame` pixels; `titles`: the menu
/// bar's titles.
pub fn layout(era: Era, width: i32, frame: i32, font: &Font, titles: &[&str]) -> Chrome {
    let f = frame;
    let inner_right = width - f - 2;
    let top = f + crate::window::CAPTION_H + 1;
    let menu_band = Rect::new(f + 2, top + 2, inner_right - (f + 2), BAND_H);
    let tool_band = Rect::new(menu_band.x, menu_band.bottom() + 2, menu_band.w, tool_band_h(era));
    let address_band = Rect::new(menu_band.x, tool_band.bottom() + 2, menu_band.w, BAND_H);
    let rebar = Rect::new(f, top, width - 2 * f, address_band.bottom() + 2 - top);
    let throbber = Rect::new(inner_right - THROBBER.0, menu_band.y, THROBBER.0, THROBBER.1);
    let mut x = f + ITEMS_X;
    let mut title_rects = Vec::new();
    for t in titles {
        let w = font.measure_label(t) + 2 * TITLE_PAD;
        title_rects.push(Rect::new(x, menu_band.y + 1, w, TITLE_H));
        x += w;
    }
    let mut x = f + ITEMS_X;
    let mut buttons = Vec::new();
    let mut separators = Vec::new();
    for slot in toolbar(era) {
        match slot {
            None => {
                separators.push(Rect::new(x, tool_band.y, 8, tool_band.h));
                x += 8;
            }
            Some(tool) => {
                let (w, arrow) = button_size(era, *tool, font);
                let main = Rect::new(x, tool_band.y, w, tool_band.h);
                x += w;
                let arrow = arrow.map(|aw| {
                    let r = Rect::new(x, tool_band.y, aw, tool_band.h);
                    x += aw;
                    r
                });
                buttons.push(Button { tool: *tool, main, arrow });
            }
        }
    }
    let label_at = (f + ITEMS_X, address_band.y + if era == Era::Win2000 { 4 } else { 5 });
    let combo_x = label_at.0 + font.measure("Address") + 4;
    let go_w = if era == Era::Win2000 { 49 } else { 0 };
    let combo_right = if era == Era::Win2000 { inner_right - go_w } else { inner_right - 4 };
    let combo = Rect::new(combo_x, address_band.y, (combo_right - combo_x).max(24), BAND_H);
    let inside = combo.inset(2, 2, 2, 2);
    let combo_button = Rect::new(inside.right() - 16, inside.y, 16, inside.h);
    let go = (era == Era::Win2000).then(|| Rect::new(combo.right() + 2, address_band.y, go_w - 6, BAND_H));
    Chrome {
        rebar,
        menu_band,
        tool_band,
        address_band,
        throbber,
        titles: title_rects,
        buttons,
        separators,
        label_at,
        combo,
        combo_button,
        go,
    }
}

/// How a flat button (a toolbar button, a menu title, Go) shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flat {
    Plain,
    /// Under the pointer.
    Hot,
    /// Held down, or its menu open.
    Pressed,
}

fn flat_edge(fb: &mut Framebuffer, theme: &Theme, r: Rect, look: Flat) {
    let c = &theme.colors;
    match look {
        Flat::Plain => {}
        Flat::Hot => fb.frame(r, c.highlight, c.shadow),
        Flat::Pressed => fb.frame(r, c.shadow, c.highlight),
    }
}

/// Text etched as grayed out: a highlight copy one pixel down and right under the shadow color.
fn etched(fb: &mut Framebuffer, font: &Font, (x, y): (i32, i32), text: &str, theme: &Theme) {
    font.draw(fb, x + 1, y + 1, text, theme.colors.highlight);
    font.draw(fb, x, y, text, theme.colors.shadow);
}

/// The little down arrow (5Ã—3) at (x, y); etched when grayed in 2000 (98 drew it black always).
fn little_arrow(fb: &mut Framebuffer, theme: &Theme, era: Era, (x, y): (i32, i32), grayed: bool) {
    let c = &theme.colors;
    let mut draw = |dx: i32, dy: i32, color: u32| {
        for row in 0..3 {
            fb.hline(x + dx + row, y + dy + row, 5 - 2 * row, color);
        }
    };
    if grayed && era == Era::Win2000 {
        draw(1, 1, c.etch);
        draw(0, 0, c.shadow);
    } else {
        draw(0, 0, c.text);
    }
}

/// What the top shows now.
#[derive(Clone, Debug, Default)]
pub struct State {
    /// The menu title under the pointer, and the one whose menu is open.
    pub hot_title: Option<usize>,
    pub chosen_title: Option<usize>,
    /// The toolbar button under the pointer (`true`: on its arrow), the one held down, and the one
    /// whose list is open.
    pub hot_tool: Option<(Tool, bool)>,
    pub pressed_tool: Option<Tool>,
    pub open_list: Option<Tool>,
    /// Grayed buttons, and buttons pushed in for a state that is on (2000's Folders).
    pub grayed: Vec<Tool>,
    pub checked: Vec<Tool>,
    pub hot_go: bool,
    pub pressed_go: bool,
    pub show_keys: bool,
}

/// Paints the rebar and its bands (the address bar's own contents come after, `paint_address`).
pub fn paint(fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts, era: Era, ch: &Chrome, titles: &[&str], s: &State) {
    let c = &theme.colors;
    let r = ch.rebar;
    fb.fill(r, c.face);
    fb.frame(r, c.shadow, c.highlight);
    fb.frame(r.inset(1, 1, 1, 1), c.highlight, c.shadow);
    for band in [ch.menu_band, ch.tool_band] {
        // The etched line under a band.
        fb.hline(r.x + 2, band.bottom(), r.w - 3, c.shadow);
        fb.hline(r.x + 1, band.bottom() + 1, r.w - 3, c.highlight);
    }
    // The throbber, after an etched line of its own.
    let t = ch.throbber;
    fb.vline(t.x - 2, t.y, t.h, c.shadow);
    fb.vline(t.x - 1, t.y, t.h + 1, c.highlight);
    if let Some(p) = explorer_art::picture(era, "throbber") {
        crate::colors16::draw_picture(fb, theme, &p, t.x, t.y);
    }
    // The menu titles, cut off before the throbber.
    let font = fonts.regular;
    let mut band = Framebuffer::new((t.x - 2 - ch.menu_band.x).max(0), ch.menu_band.h, c.face);
    for (i, (label, rect)) in titles.iter().zip(&ch.titles).enumerate() {
        let look = if s.chosen_title == Some(i) {
            Flat::Pressed
        } else if s.hot_title == Some(i) {
            Flat::Hot
        } else {
            Flat::Plain
        };
        let local = Rect::new(rect.x - ch.menu_band.x, rect.y - ch.menu_band.y, rect.w, rect.h);
        flat_edge(&mut band, theme, local, look);
        let shift = i32::from(look == Flat::Pressed);
        let keys = s.show_keys || era == Era::Win98;
        font.draw_label(&mut band, (local.x + TITLE_PAD + shift, local.y + 3 + shift), label, i32::MAX, c.text, keys);
    }
    fb.blit(&band, ch.menu_band.x, ch.menu_band.y);
    // The toolbar, cut off at the band's end (98: 4 pixels before it, where its Properties was
    // cut in a window 512 wide, `win98-truecolor-folder-mycomputer`).
    let tb = ch.tool_band;
    let strip_w = if era == Era::Win98 { (tb.w - 4).max(0) } else { tb.w };
    let mut strip = Framebuffer::new(strip_w, tb.h, c.face);
    for sep in &ch.separators {
        let x = sep.x - tb.x + 3;
        strip.vline(x, 2, tb.h - 4, c.shadow);
        strip.vline(x + 1, 2, tb.h - 4, c.highlight);
    }
    for b in &ch.buttons {
        let grayed = s.grayed.contains(&b.tool);
        let main = Rect::new(b.main.x - tb.x, 0, b.main.w, tb.h);
        let arrow = b.arrow.map(|a| Rect::new(a.x - tb.x, 0, a.w, tb.h));
        let hot = !grayed && s.hot_tool.is_some_and(|(t, _)| t == b.tool);
        let pressed = !grayed && s.pressed_tool == Some(b.tool);
        let open = s.open_list == Some(b.tool);
        let main_look = if pressed {
            Flat::Pressed
        } else if hot || open {
            Flat::Hot
        } else {
            Flat::Plain
        };
        // Held down, a button with an arrow shows both parts pushed in (`pressed-back`): 98 widens
        // the button's edge over the arrow's first column and draws the arrow's left edge over
        // it; 2000 moves the arrow's triangle too.
        let main_edge = if pressed && arrow.is_some() && era == Era::Win98 { Rect::new(main.x, main.y, main.w + 2, main.h) } else { main };
        let checked = s.checked.contains(&b.tool);
        // A button that is on: pushed in, on a checkerboard of white and face color unless the
        // pointer is on it (`win2000-truecolor-tree-start`, `-folders-hot-on`).
        let main_look = if checked { Flat::Pressed } else { main_look };
        if checked && !hot && !pressed {
            strip.checker(main.inset(2, 2, 2, 2), (0, -(tb.x + tb.y)), c.face, c.highlight);
        }
        flat_edge(&mut strip, theme, main_edge, main_look);
        if let Some(a) = arrow {
            let look = if open || pressed {
                Flat::Pressed
            } else if hot {
                Flat::Hot
            } else {
                Flat::Plain
            };
            flat_edge(&mut strip, theme, a, look);
            if pressed && era == Era::Win98 {
                strip.vline(a.x, a.y, a.h, c.shadow);
                strip.vline(a.x + 1, a.y, a.h, c.highlight);
            }
            let shift = i32::from(open || (pressed && era == Era::Win2000));
            let at = if era == Era::Win2000 { (a.x + 4, 10) } else { (a.x + 4, 19) };
            little_arrow(&mut strip, theme, era, (at.0 + shift, at.1 + shift), grayed);
        }
        let state = if grayed {
            "gray"
        } else if hot || pressed || checked {
            "hot"
        } else {
            "cold"
        };
        let shift = i32::from(pressed || checked);
        let (px, py) = match era {
            Era::Win2000 => (main.x + 4, 3),
            _ => (main.x + if b.tool.split() { 18 } else { 17 }, 3),
        };
        if let Some(p) = explorer_art::picture(era, &format!("{}_{state}", b.tool.word())) {
            crate::colors16::draw_picture(&mut strip, theme, &p, px + shift, py + shift);
        }
        if era == Era::Win2000 && b.tool == Tool::Views {
            little_arrow(&mut strip, theme, era, (main.x + 22 + shift, 10 + shift), grayed);
        }
        let label = b.tool.label();
        let text_at = match era {
            Era::Win2000 if matches!(b.tool, Tool::Back | Tool::Folders) => Some((main.x + 23, 4)),
            Era::Win2000 => None,
            _ => Some((main.x + (54 - font.measure(label)) / 2, 24)),
        };
        if let Some((x, y)) = text_at {
            if grayed {
                etched(&mut strip, font, (x, y), label, theme);
            } else {
                font.draw(&mut strip, x + shift, y + shift, label, c.text);
            }
        }
    }
    fb.blit(&strip, tb.x, tb.y);
    for band in [ch.menu_band, ch.tool_band, ch.address_band] {
        fb.frame(Rect::new(r.x + 4, band.y + 2, 3, band.h - 4), c.highlight, c.shadow);
    }
    // The address bar's name and, in 2000, Go.
    // 2000 underlined its access key once a key was pressed (98 never did).
    let keys = era == Era::Win2000 && s.show_keys;
    font.draw_label(fb, ch.label_at, "A&ddress", i32::MAX, c.text, keys);
    if let Some(g) = ch.go {
        let look = if s.pressed_go {
            Flat::Pressed
        } else if s.hot_go {
            Flat::Hot
        } else {
            Flat::Plain
        };
        flat_edge(fb, theme, g, look);
        let shift = i32::from(look == Flat::Pressed);
        let state = if look == Flat::Plain { "cold" } else { "hot" };
        if let Some(p) = explorer_art::picture(era, &format!("go_{state}")) {
            crate::colors16::draw_picture(fb, theme, &p, g.x + 3 + shift, g.y + 3 + shift);
        }
        font.draw(fb, g.x + 23 + shift, g.y + 4 + shift, "Go", c.text);
    }
}

/// The address bar's combo box: its picture and text (selected while the field has the keyboard
/// and nothing was typed yet), the caret, and the arrow button (pushed in while the list is open).
#[derive(Clone, Debug, Default)]
pub struct AddressLook<'a> {
    pub icon: Option<&'a Icon>,
    pub text: &'a str,
    /// The selected characters (start, end), and the caret's place, while the field has the
    /// keyboard.
    pub selection: Option<(usize, usize)>,
    pub caret: Option<usize>,
    pub list_open: bool,
    /// Pixels the text is moved left to keep the caret in sight (`address::Field::fit`).
    pub scroll: i32,
}

fn prefix_width(font: &Font, text: &str, chars: usize) -> i32 {
    font.measure(&text.chars().take(chars).collect::<String>())
}

pub fn paint_address(fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts, ch: &Chrome, a: &AddressLook) {
    let c = &theme.colors;
    let combo = ch.combo;
    fb.frame(combo, c.shadow, c.highlight);
    fb.frame(combo.inset(1, 1, 1, 1), c.dark_shadow, c.light);
    let inside = ch.combo_inside();
    fb.fill(inside, crate::desktop_icons::WINDOW);
    let font = fonts.regular;
    let text_clip = Rect::new(inside.x, inside.y, (ch.combo_button.x - inside.x).max(0), inside.h);
    let mut field = Framebuffer::new(text_clip.w, text_clip.h, crate::desktop_icons::WINDOW);
    if let Some(icon) = a.icon {
        let (x, y) = ch.address_icon_at();
        crate::colors16::draw_picture(&mut field, theme, icon, x - inside.x, y - inside.y);
    }
    let (tx, ty) = ch.address_text_at();
    let (tx, ty) = (tx - inside.x, ty - inside.y);
    // The text in a box of its own from its first pixel on, moved left by the scroll (the place's
    // picture stays where it is).
    let mut text = Framebuffer::new((text_clip.w - tx).max(0), text_clip.h, crate::desktop_icons::WINDOW);
    let sx = -a.scroll;
    font.draw(&mut text, sx, ty, a.text, crate::desktop_icons::WINDOW_TEXT);
    if let Some((s, e)) = a.selection.filter(|(s, e)| s != e) {
        let (s, e) = (s.min(e), s.max(e));
        let x0 = sx + prefix_width(font, a.text, s);
        let x1 = sx + prefix_width(font, a.text, e);
        let r = Rect::new(x0, ty, x1 - x0, font.height);
        text.fill(r, c.menu_highlight);
        let chosen: String = a.text.chars().skip(s).take(e - s).collect();
        font.draw(&mut text, x0, ty, &chosen, c.menu_highlight_text);
    }
    if let Some(k) = a.caret {
        let x = sx + prefix_width(font, a.text, k);
        text.vline(x, ty, font.height, crate::desktop_icons::WINDOW_TEXT);
    }
    field.blit(&text, tx, 0);
    fb.blit(&field, text_clip.x, text_clip.y);
    // The arrow button.
    let b = ch.combo_button;
    fb.fill(b, c.face);
    let shift = i32::from(a.list_open);
    if a.list_open {
        fb.frame(b, c.shadow, c.shadow);
    } else {
        fb.frame(b, c.light, c.dark_shadow);
        fb.frame(b.inset(1, 1, 1, 1), c.highlight, c.shadow);
    }
    let (x, y) = (b.x + 4 + shift, b.y + (b.h - 4) / 2 + shift);
    for row in 0..4 {
        fb.hline(x + row, y + row, 7 - 2 * row, c.text);
    }
}

/// One row of the address bar's list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListRow {
    pub level: i32,
    pub icon: Icon,
    pub name: String,
}

/// The list's height of a row.
pub const LIST_ROW_H: i32 = 16;

/// The open list's frame below the combo box: as wide as the combo box, as tall as its rows.
pub fn list_rect(ch: &Chrome, rows: usize) -> Rect {
    Rect::new(ch.combo.x, ch.combo.bottom(), ch.combo.w, rows as i32 * LIST_ROW_H + 4)
}

/// Paints the open list (`hot`: the highlighted row, its name only).
pub fn paint_list(fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts, frame: Rect, rows: &[ListRow], hot: Option<usize>) {
    let c = &theme.colors;
    fb.fill(frame, crate::desktop_icons::WINDOW);
    fb.frame(frame, c.text, c.text);
    for (i, row) in rows.iter().enumerate() {
        let y = frame.y + 1 + i as i32 * LIST_ROW_H;
        let x = frame.x + 4 + 10 * row.level;
        crate::colors16::draw_picture(fb, theme, &row.icon, x, y);
        let tx = x + 20;
        let ty = y + 2;
        if hot == Some(i) {
            // The name on the selection color with the dotted focus frame around it (2000's
            // `address-list`); the picture tinted (not copied yet: drawn plain).
            let w = fonts.regular.measure(&row.name);
            let r = Rect::new(tx - 1, y + 1, w + 2, LIST_ROW_H - 1);
            fb.fill(r, c.menu_highlight);
            fonts.regular.draw(fb, tx, ty, &row.name, c.menu_highlight_text);
            let mut dot = |x: i32, yy: i32| {
                if (x + yy).rem_euclid(2) == 0 {
                    let p = fb.get(x, yy);
                    fb.set(x, yy, (p & 0xFF00_0000) | (!p & 0x00FF_FFFF));
                }
            };
            for x in r.x..r.right() {
                dot(x, r.y);
                dot(x, r.bottom() - 1);
            }
            for yy in r.y + 1..r.bottom() - 1 {
                dot(r.x, yy);
                dot(r.right() - 1, yy);
            }
        } else {
            fonts.regular.draw(fb, tx, ty, &row.name, crate::desktop_icons::WINDOW_TEXT);
        }
    }
}

/// The list's row at a point (`None`: outside the rows).
pub fn list_row_at(frame: Rect, rows: usize, x: i32, y: i32) -> Option<usize> {
    if !frame.contains(x, y) || y < frame.y + 1 {
        return None;
    }
    let i = ((y - frame.y - 1) / LIST_ROW_H) as usize;
    (i < rows).then_some(i)
}

/// The status bar's parts: (left, middle, right) for a window `width` wide with a frame `f`.
pub fn status_parts(era: Era, width: i32, f: i32, top: i32) -> [Rect; 3] {
    let (x0, x1) = (f, width - f);
    let h = 18;
    let y = top + 2;
    let (left_w, right_w) = match era {
        Era::Win2000 => {
            let right = 153;
            let middle = 76;
            ((x1 - x0) - right - middle - 4, right)
        }
        _ => (240, 218),
    };
    let left = Rect::new(x0, y, left_w.max(0), h);
    let right = Rect::new(x1 - right_w, y, right_w, h);
    let middle = Rect::new(left.right() + 2, y, (right.x - 2 - left.right() - 2).max(0), h);
    [left, middle, right]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fonts98() -> (Font, Font) {
        let r = Font::parse(include_str!("../../tests/fixtures/ms-sans-serif-11px.font")).unwrap();
        let b = Font::parse(include_str!("../../tests/fixtures/ms-sans-serif-11px-bold.font")).unwrap();
        (r, b)
    }

    #[test]
    fn the_bands_stand_where_windows_98_put_them() {
        // My Computer at 512 wide (`win98-truecolor-folder-mycomputer`).
        let (r, b) = fonts98();
        let _ = b;
        let ch = layout(Era::Win98, 512, 4, &r, &["&File", "&Edit", "&View", "&Go"]);
        assert_eq!(ch.rebar, Rect::new(4, 23, 504, 92));
        assert_eq!((ch.menu_band.y, ch.tool_band.y, ch.address_band.y), (25, 49, 91));
        assert_eq!(ch.list_top(), 118);
        assert_eq!(ch.throbber, Rect::new(468, 25, 38, 22));
        let back = ch.buttons[0];
        assert_eq!((back.main, back.arrow), (Rect::new(15, 49, 53, 40), Some(Rect::new(68, 49, 14, 40))));
        assert_eq!(ch.buttons[1].main.x, 82, "Forward");
        assert_eq!(ch.buttons[2].main, Rect::new(149, 49, 54, 40), "Up");
        assert_eq!(ch.buttons[3].main.x, 211, "Cut, after a separator");
        assert_eq!(ch.combo.right(), 502);
        let parts = status_parts(Era::Win98, 512, 4, 437);
        assert_eq!(parts.map(|p| (p.x, p.right())), [(4, 244), (246, 288), (290, 508)]);
    }
}
