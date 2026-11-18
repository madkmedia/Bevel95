//! Minesweeper's dialog boxes. The Custom Field, best times and record entry layouts follow
//! the dialog templates in WINMINE for each Windows version.


use crate::art::Look;
use crate::dialog;
use crate::fb::{Framebuffer, Rect};
use crate::icons::ClassicIcon;
use crate::kit::{self, Effect, Key, Mods, Press};
use crate::minefield::art;
use crate::minefield::game::{Field, Level};
use crate::minefield::scores::{NAME_MAX, Settings};
use crate::text::{Font, Fonts, split_access_key};
use crate::theme::{Era, Theme};
use crate::window::{self, ButtonLook, Frame, TitleButton};

/// Which box it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Custom,
    BestTimes,
    /// A level was cleared faster than ever: in so many seconds.
    NewBest(Level, u32),
    About,
}

/// What a button of a box does. The host carries it out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Ok,
    Cancel,
    /// Best Times: forget them all.
    Reset,
    /// Display Windows 2000's What's This? popup at the selected control.
    Help(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    /// The title bar's "X".
    Close,
    Help,
    Tip(&'static str),
    Field(usize),
    Button(usize),
}

/// A one-line text field with its label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextField {
    label: &'static str,
    pub text: String,
    /// The caret's place, in characters from the start.
    caret: usize,
    /// All of the text is selected (after Tab or the label's access key): typing replaces it.
    selected: bool,
    max: usize,
    /// Only digits can be typed.
    digits: bool,
    // Set by the layout.
    rect: Rect,
    label_at: (i32, i32),
    /// The width of each character (for clicks).
    advances: Vec<i32>,
    scroll: i32,
}

fn byte_at(text: &str, chars: usize) -> usize {
    text.char_indices().nth(chars).map_or(text.len(), |(i, _)| i)
}

impl TextField {
    fn new(label: &'static str, text: &str, max: usize, digits: bool) -> TextField {
        let text: String = text.chars().take(max).collect();
        let caret = text.chars().count();
        TextField {
            label,
            text,
            caret,
            selected: false,
            max,
            digits,
            rect: Rect::default(),
            label_at: (0, 0),
            advances: Vec::new(),
            scroll: 0,
        }
    }

    fn len(&self) -> usize {
        self.text.chars().count()
    }

    fn select_all(&mut self) {
        self.caret = self.len();
        self.selected = !self.text.is_empty();
    }

    fn clear_selection(&mut self) -> bool {
        if std::mem::take(&mut self.selected) {
            self.text.clear();
            self.caret = 0;
            return true;
        }
        false
    }

    fn insert(&mut self, ch: char) {
        if ch.is_control() || (self.digits && !ch.is_ascii_digit()) {
            return;
        }
        self.clear_selection();
        if self.len() < self.max {
            let at = byte_at(&self.text, self.caret);
            self.text.insert(at, ch);
            self.caret += 1;
        }
    }

    fn backspace(&mut self) {
        if !self.clear_selection() && self.caret > 0 {
            self.caret -= 1;
            let at = byte_at(&self.text, self.caret);
            self.text.remove(at);
        }
    }

    fn delete(&mut self) {
        if !self.clear_selection() && self.caret < self.len() {
            let at = byte_at(&self.text, self.caret);
            self.text.remove(at);
        }
    }

    fn move_to(&mut self, caret: usize) {
        self.selected = false;
        self.caret = caret.min(self.len());
    }

    /// The left edge of the text.
    fn text_x(&self) -> i32 {
        self.rect.x + 3
    }

    /// A click at `x`: the caret goes between the characters nearest to it.
    fn click(&mut self, x: i32) {
        let mut edge = self.text_x() - self.scroll;
        let mut caret = 0;
        for &advance in &self.advances {
            if x < edge + advance / 2 {
                break;
            }
            edge += advance;
            caret += 1;
        }
        self.move_to(caret);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Line {
    x: i32,
    y: i32,
    text: String,
    bold: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SheetButton {
    act: Act,
    label: &'static str,
    rect: Rect,
}

/// One of the game's dialog boxes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sheet {
    pub kind: Kind,
    pub era: Era,
    title: &'static str,
    /// Running text, paragraph by paragraph (wrapped by the layout).
    paragraphs: Vec<String>,
    /// Best Times: level, time, name.
    rows: Vec<[String; 3]>,
    pub fields: Vec<TextField>,
    buttons: Vec<SheetButton>,
    /// The button Enter carries out when no button has the focus.
    default: usize,
    /// Custom Field: the largest field the screen can show.
    most: (usize, usize),
    focus: Part,
    pressed: Option<Part>,
    show_keys: bool,
    caret_on: bool,
    context_help: bool,
    // Set by the layout.
    size: (i32, i32),
    lines: Vec<Line>,
    close: Rect,
}

static DIALOG_ICON: ClassicIcon = ClassicIcon::art("dialog/flag", Look::Win2000);

/// MapDialogRect for MS Sans Serif 8pt at 96 DPI. The dialog base units are 6×13.
fn template_rect(x: i32, y: i32, w: i32, h: i32, client_y: i32) -> Rect {
    let px = |v| (v * 6 + 2) / 4;
    let py = |v| (v * 13 + 4) / 8;
    Rect::new(window::FRAME + px(x), client_y + py(y), px(w), py(h))
}

fn template_size(w: i32, h: i32, client_y: i32) -> (i32, i32) {
    let r = template_rect(0, 0, w, h, client_y);
    (r.right() + window::FRAME, r.bottom() + window::FRAME)
}

impl Sheet {
    fn content_font<'a>(&self, fonts: &Fonts<'a>) -> &'a Font {
        if self.era == Era::Win2000 && self.kind != Kind::About {
            crate::text::ms_sans_serif()
        } else {
            fonts.regular
        }
    }

    fn has_help_button(&self) -> bool {
        self.era == Era::Win2000 && matches!(self.kind, Kind::Custom | Kind::BestTimes)
    }

    fn new(kind: Kind, era: Era, title: &'static str, buttons: &[(Act, &'static str)], default: usize) -> Sheet {
        let buttons: Vec<SheetButton> =
            buttons.iter().map(|&(act, label)| SheetButton { act, label, rect: Rect::default() }).collect();
        Sheet {
            kind,
            era,
            title,
            paragraphs: Vec::new(),
            rows: Vec::new(),
            fields: Vec::new(),
            buttons,
            default,
            most: (Field::MAX_WIDTH, Field::MAX_HEIGHT),
            focus: Part::Button(default),
            pressed: None,
            // Windows 95 and 98 always underline the access keys; 2000 from the first key on.
            show_keys: era != Era::Win2000,
            caret_on: true,
            context_help: false,
            size: (0, 0),
            lines: Vec::new(),
            close: Rect::default(),
        }
    }

    /// Custom Field: the size and the mines of `field` to change; `most`: the largest field
    /// the screen can show.
    pub fn custom(era: Era, field: Field, most: (usize, usize)) -> Sheet {
        let mut s = Sheet::new(Kind::Custom, era, "Custom Field", &[(Act::Ok, "OK"), (Act::Cancel, "Cancel")], 0);
        s.most = most;
        for (label, value) in [("&Height:", field.height), ("&Width:", field.width), ("&Mines:", field.mines)] {
            s.fields.push(TextField::new(label, &value.to_string(), 3, true));
        }
        s
    }

    pub fn best_times(era: Era, settings: &Settings) -> Sheet {
        let title = if era == Era::Win2000 { "Fastest Mine Sweepers" } else { "Best Times" };
        let mut s = Sheet::new(Kind::BestTimes, era, title, &[(Act::Ok, "OK"), (Act::Reset, "&Reset Scores")], 0);
        for level in Level::RANKED {
            let (time, name) = match settings.best(level) {
                Some(best) => (format!("{} seconds", best.seconds), best.name.clone()),
                None => ("999 seconds".to_string(), Settings::ANONYMOUS.to_string()),
            };
            s.rows.push([format!("{}:", level.name()), time, name]);
        }
        s
    }

    /// Reset updates the static controls in the open dialog, leaving keyboard focus on Reset.
    pub fn reset_scores(&mut self) {
        for cells in &mut self.rows {
            cells[1] = "999 seconds".to_string();
            cells[2] = Settings::ANONYMOUS.to_string();
        }
    }

    /// The name to put next to a new best time; `name`: the one entered last time.
    pub fn new_best(era: Era, level: Level, time: u32, name: &str) -> Sheet {
        let title = if era == Era::Win2000 { "" } else { "Congratulations" };
        let mut s = Sheet::new(Kind::NewBest(level, time), era, title, &[(Act::Ok, "OK")], 0);
        let level = level.name().to_ascii_lowercase();
        s.paragraphs = if era == Era::Win2000 {
            vec!["You have the fastest time".into(), format!("for {level} level."), "Please enter your name.".into()]
        } else {
            vec![format!("You have the fastest time for {level} level. Please type your name:")]
        };
        s.fields.push(TextField::new("", name, NAME_MAX, false));
        s.focus = Part::Field(0);
        s.fields[0].select_all();
        s
    }

    /// ShellAbout included the current machine's physical memory and registration fields.
    /// Keep the registration anonymous. The old 9x resource heap has no modern counterpart.
    pub fn about(era: Era, physical_kib: Option<u64>) -> Sheet {
        let mut s = Sheet::new(Kind::About, era, "About Minesweeper", &[(Act::Ok, "OK")], 0);
        s.paragraphs.push(crate::shell_about::memory_text(physical_kib));
        s
    }

    /// What `part` does when carried out: a button's act; the title bar's "X" cancels.
    pub fn act(&self, part: Part) -> Option<Act> {
        match part {
            Part::Close => Some(Act::Cancel),
            Part::Button(i) => self.buttons.get(i).map(|b| b.act),
            Part::Field(_) | Part::Help => None,
            Part::Tip(text) => Some(Act::Help(text)),
        }
    }

    /// Custom Field's answer, brought within the limits (an empty box counts as zero).
    pub fn custom_field(&self) -> Field {
        let number = |i: usize| self.fields.get(i).and_then(|f| f.text.parse().ok()).unwrap_or(0);
        Field::custom(self.era, number(1), number(0), number(2), self.most)
    }

    /// The text of the first field (the name).
    pub fn text(&self) -> &str {
        self.fields.first().map_or("", |f| f.text.as_str())
    }

    /// The control IDs mapped by WINMINE to its What's This? topics. Both a label
    /// and its edit control have the same topic. OK and Cancel have no game topic.
    fn help_at(&self, x: i32, y: i32) -> Option<&'static str> {
        match self.kind {
            Kind::Custom => {
                let topics = [
                    "Specifies the number of vertical squares on the playing field.",
                    "Specifies the number of horizontal squares on the playing field.",
                    "Specifies the number of mines to be placed on the playing field.",
                ];
                for (field, topic) in self.fields.iter().zip(topics) {
                    let label = Rect::new(field.label_at.0, field.label_at.1, 38, 16);
                    if label.contains(x, y) || field.rect.contains(x, y) {
                        return Some(topic);
                    }
                }
            }
            Kind::BestTimes => {
                if self.buttons[1].rect.contains(x, y) {
                    return Some("Click to clear the current high scores.");
                }
                for row in [15, 25, 35] {
                    for (col, w) in [(10, 48), (60, 44), (115, 60)] {
                        if template_rect(col, row, w, 8, window::CLIENT_Y).contains(x, y) {
                            return Some("Displays a player's best game time, and the player's name, for each level of play: Beginner, Intermediate, and Expert.");
                        }
                    }
                }
            }
            _ => {}
        }
        None
    }

    fn focused_field(&mut self) -> Option<&mut TextField> {
        match self.focus {
            Part::Field(i) => self.fields.get_mut(i),
            _ => None,
        }
    }

    fn access_of(label: &str) -> Option<char> {
        let (text, at) = split_access_key(label);
        at.and_then(|k| text.chars().nth(k)).map(|c| c.to_ascii_lowercase())
    }

    fn lay_out(&mut self, fonts: &Fonts) {
        let font = self.content_font(fonts);
        let row = font.height;
        let mut lines = Vec::new();
        let wrapped = |lines: &mut Vec<Line>, text: &str, x: i32, y: &mut i32, width: i32, bold: bool| {
            for text in dialog::wrap(font, text, width) {
                lines.push(Line { x, y: *y, text, bold });
                *y += row;
            }
        };
        let modern = self.era == Era::Win2000;
        let client_y = if modern && matches!(self.kind, Kind::NewBest(..)) { window::FRAME } else { window::CLIENT_Y };
        let rect = |x, y, w, h| template_rect(x, y, w, h, client_y);
        let size = |w, h| template_size(w, h, client_y);
        let (width, height) = match self.kind {
            Kind::Custom => {
                let (label_x, edit_x, edit_w, first, step) =
                    if modern { (10, 40, 25, 20, 15) } else { (7, 50, 40, 7, 21) };
                for (i, field) in self.fields.iter_mut().enumerate() {
                    let y = first + step * i as i32;
                    field.rect = rect(edit_x, y, edit_w, 12);
                    let label = rect(label_x, y + 2, 0, 0);
                    field.label_at = (label.x, label.y);
                }
                let (x, w, h, cancel_y) = if modern { (80, 40, 16, 46) } else { (97, 50, 14, 24) };
                self.buttons[0].rect = rect(x, first, w, h);
                self.buttons[1].rect = rect(x, cancel_y, w, h);
                if modern { size(130, 85) } else { size(154, 68) }
            }
            Kind::BestTimes => {
                let rows = if modern { [15, 25, 35] } else { [21, 37, 52] };
                let cols = if modern { [10, 60, 115] } else { [14, 77, 142] };
                for (y, cells) in rows.into_iter().zip(&self.rows) {
                    for (x, cell) in cols.into_iter().zip(cells) {
                        let r = rect(x, y, 60, 8);
                        lines.push(Line { x: r.x, y: r.y, text: cell.clone(), bold: false });
                    }
                }
                if modern {
                    self.buttons[0].rect = rect(115, 55, 30, 12);
                    self.buttons[1].rect = rect(25, 55, 50, 12);
                    size(170, 75)
                } else {
                    self.buttons[0].rect = rect(153, 75, 56, 14);
                    self.buttons[1].rect = rect(93, 75, 56, 14);
                    size(216, 95)
                }
            }
            Kind::NewBest(..) => {
                if modern {
                    let r = rect(4, 4, 88, 43);
                    for (i, text) in self.paragraphs.iter().enumerate() {
                        lines.push(Line {
                            x: r.x + (r.w - font.measure(text)) / 2,
                            y: r.y + row * i as i32,
                            text: text.clone(),
                            bold: false,
                        });
                    }
                    self.fields[0].rect = rect(12, 51, 78, 12);
                    self.buttons[0].rect = rect(32, 71, 36, 16);
                    size(100, 100)
                } else {
                    let r = rect(7, 7, 134, 18);
                    let mut y = r.y;
                    wrapped(&mut lines, &self.paragraphs[0], r.x, &mut y, r.w, false);
                    self.fields[0].rect = rect(7, 30, 134, 12);
                    self.buttons[0].rect = rect(91, 49, 50, 14);
                    size(148, 70)
                }
            }
            Kind::About => {
                self.buttons[0].rect = crate::shell_about::button(self.era);
                crate::shell_about::size(self.era)
            }
        };
        for field in &mut self.fields {
            field.advances = field.text.chars().map(|c| font.glyph(c).advance).collect();
            let caret_x: i32 = field.advances.iter().take(field.caret).sum();
            let available = field.rect.w - 7;
            let text_width: i32 = field.advances.iter().sum();
            field.scroll = field.scroll.min((text_width - available).max(0))
                .clamp((caret_x - available).max(0), caret_x);
        }
        self.lines = lines;
        self.size = (width, height);
        self.close = if self.title.is_empty() { Rect::default() } else { dialog::title_button_rects(width).1 };
    }
}

impl kit::Form for Sheet {
    type Part = Part;

    fn context_help(&self) -> bool {
        self.context_help
    }

    fn title(&self) -> &str {
        self.title
    }
    fn size(&self) -> (i32, i32) {
        self.size
    }
    fn layout(&mut self, fonts: &Fonts) {
        self.lay_out(fonts);
    }
    fn paint(&self, fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts) {
        let c = &theme.colors;
        let font = self.content_font(fonts);
        let keys = self.show_keys;
        if self.title.is_empty() {
            let outer = Rect::new(0, 0, fb.width, fb.height);
            fb.fill(outer, c.face);
            fb.frame(outer, c.light, c.dark_shadow);
            fb.frame(outer.inset(1, 1, 1, 1), c.highlight, c.shadow);
        } else {
            let icon = self.has_help_button().then(|| DIALOG_ICON.icon());
            let mut buttons = Vec::new();
            if self.has_help_button() {
                buttons.push(ButtonLook {
                    button: TitleButton::Help, rect: dialog::title_button_rects(self.size.0).0,
                    pressed: self.pressed == Some(Part::Help), enabled: true,
                });
            }
            buttons.push(ButtonLook {
                button: TitleButton::Close, rect: self.close, pressed: self.pressed == Some(Part::Close), enabled: true,
            });
            window::paint_frame(fb, theme, fonts, &Frame {
                era: self.era, title: self.title, active: true, icon: icon.as_ref(), buttons: &buttons,
            });
        }
        if self.kind == Kind::BestTimes && self.era != Era::Win2000 {
            let group = template_rect(7, 7, 202, 63, window::CLIENT_Y);
            crate::properties::group_box(fb, c, font, group.inset(0, 6, 0, 0), "Fastest Mine Sweepers", keys);
        }
        if self.kind == Kind::About {
            crate::shell_about::paint(fb, self.era, theme, font, crate::shell_about::App {
                name: "Minesweeper", credit: "by Robert Donner and Curt Johnson",
                icon: &art::icon(self.era, 32), memory: &self.paragraphs[0],
            });
        }
        for line in &self.lines {
            let font = if line.bold { fonts.bold } else { font };
            if self.kind == Kind::BestTimes && font.measure(&line.text) > 90 {
                // The name's static control is 60 dialog units wide.
                let mut clipped = Framebuffer::new(90, font.height, c.face);
                font.draw(&mut clipped, 0, 0, &line.text, c.text);
                fb.blit(&clipped, line.x, line.y);
            } else {
                font.draw(fb, line.x, line.y, &line.text, c.text);
            }
        }
        for (i, field) in self.fields.iter().enumerate() {
            font.draw_label(fb, field.label_at, field.label, i32::MAX, c.text, keys);
            let inside = dialog::well(fb, field.rect, c);
            let mut edit = Framebuffer::new(inside.w - 2, font.height, crate::theme::rgb(0xFFFFFF));
            let focused = self.focus == Part::Field(i);
            if focused && field.selected {
                edit.fill(Rect::new(-field.scroll, 0, font.measure(&field.text), font.height), c.menu_highlight);
                font.draw(&mut edit, -field.scroll, 0, &field.text, c.menu_highlight_text);
            } else {
                font.draw(&mut edit, -field.scroll, 0, &field.text, c.text);
            }
            if focused && self.caret_on {
                let caret_x: i32 = field.advances.iter().take(field.caret).sum();
                edit.vline(caret_x - field.scroll, 0, font.height, c.text);
            }
            fb.blit(&edit, field.text_x(), inside.y + 1);
        }
        // The default button is framed, unless another button has the focus.
        let framed = match self.focus {
            Part::Button(i) => i,
            _ => self.default,
        };
        for (i, button) in self.buttons.iter().enumerate() {
            let part = Part::Button(i);
            let focused = self.focus == part && keys;
            // The 20px and 26px buttons in the 2000 templates center a 13px font
            // by rounding the half-pixel toward the bottom.
            let even_height = button.rect.h % 2 == 0;
            let pressed = self.pressed == Some(part);
            dialog::push_button(
                fb,
                theme,
                font,
                button.rect,
                if even_height { "" } else { button.label },
                framed == i,
                pressed,
                focused,
                true,
                keys,
            );
            if even_height {
                let r = button.rect;
                let shift = i32::from(pressed);
                dialog::centered_label(fb, font, Rect::new(r.x + shift, r.y + 1 + shift, r.w, r.h), button.label, c.text, keys);
            }
        }
    }

    fn hit(&self, x: i32, y: i32) -> Option<Part> {
        if self.has_help_button() && dialog::title_button_rects(self.size.0).0.contains(x, y) {
            return Some(Part::Help);
        }
        if self.close.contains(x, y) {
            return Some(Part::Close);
        }
        if let Some(i) = self.fields.iter().position(|f| f.rect.contains(x, y)) {
            return Some(Part::Field(i));
        }
        self.buttons.iter().position(|b| b.rect.contains(x, y)).map(Part::Button)
    }
    fn rect(&self, part: Part) -> Option<Rect> {
        match part {
            Part::Close => Some(self.close),
            Part::Help => self.has_help_button().then(|| dialog::title_button_rects(self.size.0).0),
            Part::Tip(_) => None,
            Part::Field(i) => self.fields.get(i).map(|f| f.rect),
            Part::Button(i) => self.buttons.get(i).map(|b| b.rect),
        }
    }
    fn enabled(&self, _part: Part) -> bool {
        true
    }
    fn focus(&self) -> Part {
        self.focus
    }
    /// A field reached by Tab has all its text selected, as Windows' dialogs do.
    fn set_focus(&mut self, part: Part) {
        self.focus = part;
        for field in &mut self.fields {
            field.selected = false;
        }
        if let Some(field) = self.focused_field() {
            field.select_all();
        }
    }
    fn tab_stops(&self) -> Vec<Part> {
        let fields = (0..self.fields.len()).map(Part::Field);
        let buttons = (0..self.buttons.len()).map(Part::Button);
        if self.kind == Kind::Custom { buttons.chain(fields).collect() } else { fields.chain(buttons).collect() }
    }
    fn set_pressed(&mut self, part: Option<Part>) {
        self.pressed = part;
    }
    fn show_keys(&mut self) {
        self.show_keys = true;
    }
    fn enter(&self) -> Part {
        match self.focus {
            Part::Button(i) => Part::Button(i),
            _ => Part::Button(self.default),
        }
    }
    fn escape(&self) -> Part {
        Part::Close
    }

    fn key(&mut self, key: Key, mods: Mods) -> Option<Vec<Effect<Part>>> {
        if self.has_help_button() && key == Key::Other(0x70) && mods.shift {
            self.context_help = true;
            return Some(Vec::new());
        }
        if self.context_help && key == Key::Escape {
            self.context_help = false;
            return Some(Vec::new());
        }
        let buttons = self.buttons.len();
        match (self.focus, key) {
            (Part::Field(_), Key::Other(vk)) if mods.ctrl && vk == u16::from(b'V') => return Some(vec![Effect::Paste]),
            (Part::Field(_), Key::Left | Key::Right | Key::Home | Key::End | Key::Delete) => {
                let field = self.focused_field()?;
                match key {
                    Key::Left => field.move_to(field.caret.saturating_sub(1)),
                    Key::Right => field.move_to(field.caret + 1),
                    Key::Home => field.move_to(0),
                    Key::End => field.move_to(usize::MAX),
                    _ => field.delete(),
                }
                self.caret_on = true;
            }
            // The arrow keys go from button to button.
            (Part::Button(i), Key::Left | Key::Up) => self.focus = Part::Button((i + buttons - 1) % buttons),
            (Part::Button(i), Key::Right | Key::Down) => self.focus = Part::Button((i + 1) % buttons),
            _ => return None,
        }
        Some(Vec::new())
    }
    fn typed(&mut self, ch: char) -> Vec<Effect<Part>> {
        match self.focused_field() {
            Some(field) => {
                if ch == '\u{8}' {
                    field.backspace();
                } else {
                    field.insert(ch);
                }
                self.caret_on = true;
                Vec::new()
            }
            // No field has the focus: a letter is an access key, with or without Alt.
            None if ch.is_alphabetic() => self.access(ch),
            None => Vec::new(),
        }
    }
    fn access(&mut self, ch: char) -> Vec<Effect<Part>> {
        let ch = ch.to_ascii_lowercase();
        if let Some(i) = self.fields.iter().position(|f| Sheet::access_of(f.label) == Some(ch)) {
            self.set_focus(Part::Field(i));
        } else if let Some(i) = self.buttons.iter().position(|b| Sheet::access_of(b.label) == Some(ch)) {
            self.focus = Part::Button(i);
            return vec![Effect::Activate(Part::Button(i))];
        }
        Vec::new()
    }
    fn space(&mut self) -> Vec<Effect<Part>> {
        match self.focus {
            Part::Button(i) => vec![Effect::Activate(Part::Button(i))],
            _ => Vec::new(),
        }
    }
    fn press(&mut self, part: Option<Part>, x: i32, y: i32) -> Press<Part> {
        if std::mem::take(&mut self.context_help) {
            return self.help_at(x, y).map_or(Press::Done, |topic| Press::Act(Part::Tip(topic)));
        }
        match part {
            Some(Part::Field(i)) => {
                self.focus = Part::Field(i);
                for field in &mut self.fields {
                    field.selected = false;
                }
                self.fields[i].click(x);
                self.caret_on = true;
                Press::Done
            }
            Some(Part::Button(i)) => {
                self.set_focus(Part::Button(i));
                Press::Hold(Part::Button(i))
            }
            Some(p @ (Part::Close | Part::Help)) => Press::Hold(p),
            Some(Part::Tip(_)) => Press::Nothing,
            None if self.title.is_empty() => Press::Done,
            None => Press::Nothing,
        }
    }
    fn release(&mut self, part: Part) -> Vec<Effect<Part>> {
        if part == Part::Help {
            self.context_help = true;
            Vec::new()
        } else {
            vec![Effect::Activate(part)]
        }
    }
    fn paste(&mut self, text: &str) {
        if let Some(field) = self.focused_field() {
            for ch in text.chars() {
                field.insert(ch);
            }
        }
    }
    fn has_caret(&self) -> bool {
        matches!(self.focus, Part::Field(_))
    }
    fn blink(&mut self, on: bool) {
        self.caret_on = on;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::{Controller, Form};

    fn fonts() -> (Font, Font) {
        (
            Font::parse(include_str!("../../tests/fixtures/ms-sans-serif-11px.font")).unwrap(),
            Font::parse(include_str!("../../tests/fixtures/ms-sans-serif-11px-bold.font")).unwrap(),
        )
    }

    fn laid_out(mut sheet: Sheet) -> Sheet {
        let (regular, bold) = fonts();
        sheet.layout(&Fonts { regular: &regular, bold: &bold });
        sheet
    }

    fn type_text(s: &mut Sheet, text: &str) {
        for ch in text.chars() {
            s.typed(ch);
        }
    }

    #[test]
    fn custom_field_takes_three_numbers_and_keeps_them_in_bounds() {
        let mut s = laid_out(Sheet::custom(Era::Win95, Field { width: 9, height: 9, mines: 10 }, (30, 20)));
        let mut c = Controller::default();
        assert_eq!(s.size(), (237, 136));
        assert_eq!(s.focus(), Part::Button(0));
        // Alt+H selects the first field: typing replaces it; letters are not taken.
        c.access(&mut s, 'h');
        type_text(&mut s, "1x2");
        assert_eq!(s.fields[0].text, "12");
        // Tab selects the next one; a fourth digit does not fit
        c.key(&mut s, Key::Tab, Mods::default());
        assert_eq!(s.focus(), Part::Field(1));
        type_text(&mut s, "2345");
        assert_eq!(s.fields[1].text, "234");
        // Alt+M goes to Mines; Backspace removes the selection
        c.access(&mut s, 'm');
        s.typed('\u{8}');
        assert_eq!((s.focus(), s.fields[2].text.as_str()), (Part::Field(2), ""));
        // 12 high, 234 wide (the screen shows 30), no mines: brought within the limits.
        assert_eq!(s.custom_field(), Field { width: 30, height: 12, mines: 10 });
        assert_eq!(c.key(&mut s, Key::Enter, Mods::default()), vec![Effect::Activate(Part::Button(0))]);
        assert_eq!(s.act(Part::Button(0)), Some(Act::Ok));
        assert_eq!(c.key(&mut s, Key::Escape, Mods::default()), vec![Effect::Activate(Part::Close)]);
        assert_eq!(s.act(Part::Close), Some(Act::Cancel));
    }

    #[test]
    fn a_text_field_edits_at_its_caret() {
        let mut s = laid_out(Sheet::new_best(Era::Win98, Level::Expert, 1, "Ada"));
        let mut c = Controller::default();
        assert_eq!(s.title(), "Congratulations");
        assert!(s.lines.iter().any(|l| l.text.contains("expert")));
        assert!(s.has_caret(), "the caret blinks at the end of the selection");
        c.key(&mut s, Key::Home, Mods::default());
        assert!(s.has_caret());
        type_text(&mut s, "L");
        c.key(&mut s, Key::End, Mods::default());
        s.typed('!');
        c.key(&mut s, Key::Left, Mods::default());
        c.key(&mut s, Key::Delete, Mods::default());
        assert_eq!(s.text(), "LAda");
        // a click puts the caret between the letters nearest to it
        let s2 = laid_out(s.clone());
        let mut s = s2;
        let x = s.fields[0].text_x() + s.fields[0].advances[0] + 1;
        let field = s.rect(Part::Field(0)).unwrap();
        c.mouse_down(&mut s, x, field.y + 5);
        s.typed('-');
        assert_eq!(s.text(), "L-Ada");
        // pasted text goes in, up to the longest name
        s.paste(&"x".repeat(100));
        assert_eq!(s.text().chars().count(), NAME_MAX);
        // Enter is OK even from the field
        assert_eq!(c.key(&mut s, Key::Enter, Mods::default()), vec![Effect::Activate(Part::Button(0))]);
    }

    #[test]
    fn best_times_lists_the_three_levels() {
        let mut settings = Settings::default();
        settings.record(Level::Intermediate, 75, "Grace");
        let mut s = laid_out(Sheet::best_times(Era::Win2000, &settings));
        let texts: Vec<&str> = s.lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(
            texts,
            ["Beginner:", "999 seconds", "Anonymous", "Intermediate:", "75 seconds", "Grace", "Expert:", "999 seconds", "Anonymous"]
        );
        // OK is the default; R (with or without Alt) resets
        let mut c = Controller::default();
        assert_eq!(s.focus(), Part::Button(0));
        assert_eq!(c.key(&mut s, Key::Enter, Mods::default()), vec![Effect::Activate(Part::Button(0))]);
        assert_eq!(s.typed('r'), vec![Effect::Activate(Part::Button(1))]);
        assert_eq!(s.act(Part::Button(1)), Some(Act::Reset));
        assert_eq!(s.size(), (261, 147));
        assert_eq!(s.rect(Part::Button(1)), Some(Rect::new(41, 111, 75, 20)));
        s.reset_scores();
        assert_eq!(s.focus(), Part::Button(1));
        assert!(s.rows.iter().all(|row| row[1] == "999 seconds" && row[2] == "Anonymous"));
    }

    #[test]
    fn context_help_on_reset_shows_its_topic_and_keeps_the_scores() {
        let mut settings = Settings::default();
        settings.record(Level::Expert, 25, "Pat");
        let mut s = laid_out(Sheet::best_times(Era::Win2000, &settings));
        let mut controller = Controller::default();
        let help = s.rect(Part::Help).unwrap();
        controller.mouse_down(&mut s, help.x + 1, help.y + 1);
        controller.mouse_up(&mut s, help.x + 1, help.y + 1);
        assert!(s.context_help());
        let reset = s.rect(Part::Button(1)).unwrap();
        let effects = controller.mouse_down(&mut s, reset.x + 1, reset.y + 1);
        let [Effect::Activate(part)] = effects.as_slice() else { panic!("expected a help topic") };
        assert_eq!(s.act(*part), Some(Act::Help("Click to clear the current high scores.")));
        assert!(!s.context_help());
        assert_eq!(s.rows[2][2], "Pat");
    }

    #[test]
    fn about_uses_the_shell_dialog_and_memory_supplied_by_the_host() {
        let (regular, bold) = fonts();
        let fonts = Fonts { regular: &regular, bold: &bold };
        for (era, size) in [(Era::Win95, (348, 283)), (Era::Win98, (348, 283)), (Era::Win2000, (419, 329))] {
            let s = laid_out(Sheet::about(era, Some(1_234_567)));
            assert_eq!(s.size(), size);
            let colors = &crate::theme::theme(era).colors;
            let mut shown = Framebuffer::new(size.0, size.1, colors.face);
            s.paint(&mut shown, crate::theme::theme(era), &fonts);
            let (x, y) = if era == Era::Win2000 { (306, 253) } else { (279, 191) };
            let text = "1,234,567 KB";
            let mut expected = Framebuffer::new(regular.measure(text), regular.height, colors.face);
            regular.draw(&mut expected, 0, 0, text, colors.text);
            for row in 0..expected.height {
                for col in 0..expected.width.min(size.0 - x - window::FRAME) {
                    assert_eq!(shown.get(x + col, y + row), expected.get(col, row), "{era:?} memory field");
                }
            }
            assert_eq!(s.act(s.enter()), Some(Act::Ok));
        }
    }
}
