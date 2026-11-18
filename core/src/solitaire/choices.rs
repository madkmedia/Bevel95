//! Solitaire's Options and Select Card Back dialogs. The original 16-bit and NT
//! programs have different dialog resources. Coordinates below are their dialog units.

use super::art;
use super::game::{Draw, Scoring};
use super::settings::Settings;
use crate::dialog;
use crate::fb::{Framebuffer, Rect};
use crate::kit::{Button, Effect, Form, Key, Mods, Press};
use crate::properties;
use crate::text::{Font, Fonts, ms_sans_serif, split_access_key};
use crate::theme::{Era, Theme};
use crate::window::{self, ButtonLook, Frame, TitleButton};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Close,
    Help,
    Ok,
    Cancel,
    DrawGroup,
    One,
    Three,
    ScoringGroup,
    Standard,
    Vegas,
    NoScore,
    Timed,
    Status,
    Outline,
    Cumulative,
    ScoreLabel,
    /// The zero-based CARDS bitmap number (resource ID minus 54).
    Back(u8),
    Tip(&'static str),
}

// SOL's resource IDs are not consecutive in the displayed order.
const BACKS: [u8; 12] = [0, 1, 6, 7, 4, 5, 2, 3, 8, 9, 10, 11];
const DRAW: &[Part] = &[Part::One, Part::Three];
const SCORING: &[Part] = &[Part::Standard, Part::Vegas, Part::NoScore];
const CHECKS: &[Part] = &[Part::Timed, Part::Status, Part::Outline, Part::Cumulative];
const BUTTONS: &[Part] = &[Part::Ok, Part::Cancel];

pub struct Choices {
    era: Era,
    deck: bool,
    settings: Settings,
    focus: Part,
    pressed: Option<Part>,
    active: bool,
    keys: bool,
    help: bool,
}

impl Choices {
    pub fn options(era: Era, settings: &Settings) -> Self {
        Self::new(era, settings, false)
    }
    pub fn deck(era: Era, settings: &Settings) -> Self {
        Self::new(era, settings, true)
    }
    fn new(era: Era, settings: &Settings, deck: bool) -> Self {
        let focus = if deck {
            Part::Back(settings.deck)
        } else {
            match settings.rules.draw {
                Draw::One => Part::One,
                Draw::Three => Part::Three,
            }
        };
        let mut settings = settings.clone();
        if deck {
            settings.remember_deck = true;
        }
        Self { era, deck, settings, focus, pressed: None, active: true, keys: era.always_shows_keys(), help: false }
    }
    /// The dialog edits a copy. The owner applies it only after OK.
    pub fn edited(&self) -> &Settings {
        &self.settings
    }
    fn modern(&self) -> bool {
        self.era == Era::Win2000
    }
    fn label(&self, p: Part) -> &'static str {
        match p {
            Part::DrawGroup => "&Draw",
            Part::ScoringGroup => "&Scoring",
            Part::One if self.modern() => "Draw &One",
            Part::Three if self.modern() => "Draw &Three",
            Part::One => "Draw &one",
            Part::Three => "Draw &three",
            Part::Standard => "St&andard",
            Part::Vegas => "&Vegas",
            Part::NoScore => "&None",
            Part::Timed => "T&imed game",
            Part::Status => "Status &bar",
            Part::Outline => "Out&line dragging",
            Part::Cumulative if self.modern() => "&Cumulative",
            Part::Cumulative => "&Keep score",
            Part::ScoreLabel => "Score",
            Part::Ok => "OK",
            Part::Cancel => "Cancel",
            _ => "",
        }
    }
    fn checked(&self, part: Part) -> bool {
        let s = &self.settings;
        match part {
            Part::One => s.rules.draw == Draw::One,
            Part::Three => s.rules.draw == Draw::Three,
            Part::Standard => s.rules.scoring == Scoring::Standard,
            Part::Vegas => s.rules.scoring == Scoring::Vegas,
            Part::NoScore => s.rules.scoring == Scoring::None,
            Part::Timed => s.rules.timed,
            Part::Status => s.status_bar,
            Part::Outline => s.outline,
            Part::Cumulative => s.rules.cumulative,
            Part::Back(i) => s.deck == i,
            _ => false,
        }
    }
    pub fn activate(&mut self, part: Part) {
        if !self.enabled(part) {
            return;
        }
        let s = &mut self.settings;
        match part {
            Part::One => s.rules.draw = Draw::One,
            Part::Three => s.rules.draw = Draw::Three,
            Part::Standard => s.rules.scoring = Scoring::Standard,
            Part::Vegas => s.rules.scoring = Scoring::Vegas,
            Part::NoScore => s.rules.scoring = Scoring::None,
            Part::Timed => s.rules.timed = !s.rules.timed,
            Part::Status => s.status_bar = !s.status_bar,
            Part::Outline => s.outline = !s.outline,
            Part::Cumulative => s.rules.cumulative = !s.rules.cumulative,
            Part::Back(i) if i < 12 => s.deck = i,
            Part::Help => {
                self.help = true;
                return;
            }
            _ => return,
        }
        self.focus = part;
    }
    fn current(&self, group: &[Part]) -> Part {
        group.iter().copied().find(|&p| self.checked(p)).unwrap_or(group[0])
    }
    fn option_rect(&self, p: Part) -> Option<(i32, i32, i32, i32)> {
        use Part::*;
        Some(if self.modern() {
            match p {
                DrawGroup => (4, 4, 60, 36),
                One => (8, 13, 44, 12),
                Three => (8, 25, 52, 12),
                ScoringGroup => (68, 4, 56, 48),
                Standard => (72, 14, 43, 12),
                Vegas => (72, 26, 36, 12),
                NoScore => (72, 38, 30, 12),
                Timed => (8, 44, 52, 12),
                Status => (8, 56, 52, 12),
                Outline => (8, 68, 76, 12),
                Cumulative => (72, 56, 62, 12),
                ScoreLabel => (84, 68, 20, 12),
                Ok => (36, 84, 36, 14),
                Cancel => (76, 84, 40, 14),
                _ => return None,
            }
        } else {
            match p {
                DrawGroup => (8, 8, 100, 52),
                One => (20, 24, 60, 10),
                Three => (20, 42, 60, 10),
                ScoringGroup => (116, 8, 100, 52),
                Standard => (128, 20, 60, 10),
                Vegas => (128, 33, 60, 10),
                NoScore => (128, 46, 60, 10),
                Timed => (8, 66, 70, 10),
                Status => (8, 79, 70, 10),
                Outline => (116, 66, 70, 10),
                Cumulative => (116, 79, 70, 10),
                Ok => (112, 96, 50, 14),
                Cancel => (166, 96, 50, 14),
                _ => return None,
            }
        })
    }
    fn deck_rect(&self, p: Part) -> Option<(i32, i32, i32, i32)> {
        if let Part::Back(i) = p {
            let pos = BACKS.iter().position(|&v| v == i)? as i32;
            return Some(if self.modern() {
                (8 + (pos % 6) * 28, 4 + (pos / 6) * 44, 26, 42)
            } else {
                (8 + (pos % 6) * 36, 8 + (pos / 6) * 54, 34, 52)
            });
        }
        match (self.modern(), p) {
            (true, Part::Ok) => Some((44, 94, 40, 14)),
            (true, Part::Cancel) => Some((94, 94, 40, 14)),
            (false, Part::Ok) => Some((118, 122, 50, 14)),
            (false, Part::Cancel) => Some((172, 122, 50, 14)),
            _ => None,
        }
    }
    fn controls(&self) -> Vec<Part> {
        let mut all = vec![Part::Close];
        if self.modern() {
            all.push(Part::Help);
        }
        if self.deck {
            all.extend(BACKS.map(Part::Back));
        } else {
            all.extend(DRAW);
            all.extend(SCORING);
            all.extend(CHECKS);
            if self.modern() {
                all.push(Part::ScoreLabel);
            }
            all.extend([Part::DrawGroup, Part::ScoringGroup]);
        }
        all.extend(BUTTONS);
        all
    }
    fn tip(p: Part) -> Option<&'static str> {
        // The NT program asks HTML Help for sol.chm:://sol.txt. That member is absent
        // from its shipped CHM, so HHCTRL displays its fallback popup for the controls.
        match p {
            Part::Close | Part::Help | Part::Tip(_) => None,
            _ => Some("No help topic is associated with this item."),
        }
    }
    fn paint_choice(&self, fb: &mut Framebuffer, theme: &Theme, font: &Font, part: Part, r: Rect) {
        let c = &theme.colors;
        let enabled = self.enabled(part);
        let pressed = self.pressed == Some(part);
        let (x, y) = (r.x, r.y + (r.h - 13) / 2);
        if CHECKS.contains(&part) {
            properties::check_box(fb, c, x, y, self.checked(part), pressed, enabled);
        } else {
            let x = x + 1;
            let glyph = if self.checked(part) { &dialog::OPTION_ON } else { &dialog::OPTION_OFF };
            dialog::art(fb, glyph, x, y, |b| match b {
                b'g' => Some(c.shadow),
                b'#' => Some(c.dark_shadow),
                b'l' => Some(c.light),
                b'W' => Some(if pressed { c.face } else { c.highlight }),
                _ => None,
            });
            if self.checked(part) {
                for (dy, (left, w)) in [(5, 2), (4, 4), (4, 4), (5, 2)].into_iter().enumerate() {
                    fb.hline(x + left, y + 4 + dy as i32, w, c.text);
                }
            }
        }
        let (tx, ty) = (x + 18, r.y + (r.h - font.height) / 2);
        let label = self.label(part);
        if !enabled {
            font.draw_label(fb, (tx + 1, ty + 1), label, r.w, c.etch, self.keys);
        }
        font.draw_label(fb, (tx, ty), label, r.w, if enabled { c.text } else { c.shadow }, self.keys);
        if self.active && self.keys && self.focus == part {
            let r = Rect::new(tx - 1, ty - 1, font.measure_label(label) + 2, font.height + 3);
            dialog::focus_dots(fb, r, (r.x + 1, r.y), c.text);
        }
    }
    fn paint_back(&self, fb: &mut Framebuffer, theme: &Theme, index: u8, r: Rect) {
        let c = &theme.colors;
        let focused = self.active && self.focus == Part::Back(index);
        if focused || !self.modern() {
            let ink = if focused { c.menu_highlight } else { c.highlight };
            fb.frame(r, ink, ink);
            fb.frame(r.inset(1, 1, 1, 1), ink, ink);
        }
        let thumb = r.inset(3, 3, 3, 3);
        art::back_scaled(fb, thumb, self.era, index.into());
        if self.pressed == Some(Part::Back(index)) {
            for y in thumb.y..thumb.bottom() {
                for x in thumb.x..thumb.right() {
                    fb.set(x, y, fb.get(x, y) ^ 0x00FF_FFFF);
                }
            }
        }
    }
}

fn dlu_x(n: i32) -> i32 {
    (n * 6 + 2) / 4
}
fn dlu_y(n: i32) -> i32 {
    (n * 13 + 4) / 8
}

impl Form for Choices {
    type Part = Part;
    fn title(&self) -> &str {
        if self.deck { "Select Card Back" } else { "Options" }
    }
    fn size(&self) -> (i32, i32) {
        let (w, h) = match (self.deck, self.modern()) {
            (true, true) => (178, 112),
            (true, false) => (230, 144),
            (false, true) => (134, 101),
            (false, false) => (224, 118),
        };
        (6 + dlu_x(w), 25 + dlu_y(h))
    }
    fn layout(&mut self, _: &Fonts) {}
    fn paint(&self, fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts) {
        let title = |button, part| ButtonLook {
            button,
            rect: window::title_button_rect(self.size().0, button),
            pressed: self.pressed == Some(part),
            enabled: true,
        };
        let buttons = [title(TitleButton::Help, Part::Help), title(TitleButton::Close, Part::Close)];
        window::paint_frame(
            fb,
            theme,
            fonts,
            &Frame {
                title: self.title(),
                era: self.era,
                active: self.active,
                icon: None,
                buttons: if self.modern() { &buttons } else { &buttons[1..] },
            },
        );
        let c = &theme.colors;
        let font = ms_sans_serif();
        for part in self.controls() {
            let Some(r) = self.rect(part) else { continue };
            match part {
                Part::DrawGroup | Part::ScoringGroup => {
                    properties::group_box(fb, c, font, r.inset(0, 6, 0, 0), self.label(part), self.keys)
                }
                Part::ScoreLabel => {
                    if !self.enabled(part) {
                        font.draw(fb, r.x + 1, r.y + 1, self.label(part), c.etch);
                    }
                    font.draw(fb, r.x, r.y, self.label(part), if self.enabled(part) { c.text } else { c.shadow });
                }
                Part::Ok | Part::Cancel => dialog::push_button(
                    fb,
                    theme,
                    font,
                    r,
                    self.label(part),
                    self.enter() == part,
                    self.pressed == Some(part),
                    self.keys && self.active && self.focus == part,
                    true,
                    self.keys,
                ),
                Part::Back(i) => self.paint_back(fb, theme, i, r),
                Part::Close | Part::Help | Part::Tip(_) => {}
                _ => self.paint_choice(fb, theme, font, part, r),
            }
        }
    }
    fn rect(&self, part: Part) -> Option<Rect> {
        if part == Part::Close {
            return Some(window::title_button_rect(self.size().0, TitleButton::Close));
        }
        if part == Part::Help && self.modern() {
            return Some(window::title_button_rect(self.size().0, TitleButton::Help));
        }
        let (x, y, w, h) = if self.deck { self.deck_rect(part) } else { self.option_rect(part) }?;
        Some(Rect::new(3 + dlu_x(x), 22 + dlu_y(y), dlu_x(w), dlu_y(h)))
    }
    fn hit(&self, x: i32, y: i32) -> Option<Part> {
        self.controls().into_iter().find(|&p| self.rect(p).is_some_and(|r| r.contains(x, y)))
    }
    fn enabled(&self, p: Part) -> bool {
        !matches!(p, Part::Cumulative | Part::ScoreLabel) || self.settings.rules.scoring == Scoring::Vegas
    }
    fn focus(&self) -> Part {
        self.focus
    }
    fn set_focus(&mut self, p: Part) {
        if let Part::Back(index) = p {
            if index < 12 {
                self.activate(p);
            }
            return;
        }
        if self.enabled(p)
            && !matches!(
                p,
                Part::DrawGroup | Part::ScoringGroup | Part::ScoreLabel | Part::Help | Part::Close | Part::Tip(_)
            )
        {
            self.focus = p;
        }
    }
    fn set_active(&mut self, active: bool) {
        self.active = active;
    }
    fn tab_stops(&self) -> Vec<Part> {
        let mut stops = if self.deck {
            // Only the first owner-drawn button in the resource has WS_TABSTOP.
            vec![if matches!(self.focus, Part::Back(_)) { self.focus } else { Part::Back(BACKS[0]) }]
        } else {
            let mut s = vec![self.current(DRAW), self.current(SCORING)];
            s.extend(CHECKS.iter().copied().filter(|&p| self.enabled(p)));
            s
        };
        stops.extend(BUTTONS);
        stops
    }
    fn set_pressed(&mut self, p: Option<Part>) {
        self.pressed = p;
    }
    fn show_keys(&mut self) {
        self.keys = true;
    }
    fn context_help(&self) -> bool {
        self.help
    }
    fn enter(&self) -> Part {
        if self.focus == Part::Cancel { Part::Cancel } else { Part::Ok }
    }
    fn escape(&self) -> Part {
        Part::Cancel
    }
    fn key(&mut self, key: Key, mods: Mods) -> Option<Vec<Effect<Part>>> {
        if self.modern() && (key == Key::Other(0x5D) || (mods.shift && key == Key::Other(0x79))) {
            return Self::tip(self.focus).map(|tip| vec![Effect::Activate(Part::Tip(tip))]);
        }
        if self.modern() && key == Key::Other(0x70) {
            if mods.shift {
                self.help = true;
                return Some(Vec::new());
            }
            return Self::tip(self.focus).map(|s| vec![Effect::Activate(Part::Tip(s))]);
        }
        if self.help && key == Key::Escape {
            self.help = false;
            return Some(Vec::new());
        }
        let by = match key {
            Key::Up | Key::Left => -1,
            Key::Right | Key::Down => 1,
            _ => return None,
        };
        if let Part::Back(i) = self.focus {
            let at = BACKS.iter().position(|&v| v == i)? as i32;
            // These are buttons in one dialog group; Down and Right both advance in resource order.
            let next = (at + by).rem_euclid(BACKS.len() as i32);
            self.activate(Part::Back(BACKS[next as usize]));
        } else if CHECKS.contains(&self.focus) {
            let enabled: Vec<_> = CHECKS.iter().copied().filter(|&p| self.enabled(p)).collect();
            let at = enabled.iter().position(|&p| p == self.focus)? as i32;
            self.set_focus(enabled[(at + by).rem_euclid(enabled.len() as i32) as usize]);
        } else {
            let group = if DRAW.contains(&self.focus) {
                DRAW
            } else if SCORING.contains(&self.focus) {
                SCORING
            } else {
                return None;
            };
            let at = group.iter().position(|&p| p == self.focus)? as i32;
            self.activate(group[(at + by).rem_euclid(group.len() as i32) as usize]);
        }
        Some(Vec::new())
    }
    fn alt_key_with_mods(&mut self, key: Key, mods: Mods) -> Option<Vec<Effect<Part>>> {
        (key == Key::Other(0x79) && mods.shift).then(|| self.key(key, mods)).flatten()
    }
    fn access(&mut self, ch: char) -> Vec<Effect<Part>> {
        for part in self.controls() {
            let (label, key) = split_access_key(self.label(part));
            if key.and_then(|n| label.chars().nth(n)).is_none_or(|k| !k.eq_ignore_ascii_case(&ch))
                || !self.enabled(part)
            {
                continue;
            }
            match part {
                Part::DrawGroup => self.set_focus(self.current(DRAW)),
                Part::ScoringGroup => self.set_focus(self.current(SCORING)),
                p => self.activate(p),
            }
            break;
        }
        Vec::new()
    }
    fn typed(&mut self, ch: char) -> Vec<Effect<Part>> {
        self.access(ch)
    }
    fn space(&mut self) -> Vec<Effect<Part>> {
        vec![Effect::Activate(self.focus)]
    }
    fn press(&mut self, part: Option<Part>, _: i32, _: i32) -> Press<Part> {
        if std::mem::take(&mut self.help) {
            return part.and_then(Self::tip).map_or(Press::Done, |s| Press::Act(Part::Tip(s)));
        }
        match part {
            Some(Part::DrawGroup | Part::ScoringGroup | Part::ScoreLabel) => Press::Done,
            Some(p) if self.enabled(p) => {
                self.set_focus(p);
                Press::Hold(p)
            }
            _ => Press::Nothing,
        }
    }
    fn button(&mut self, button: Button, down: bool, x: i32, y: i32) -> Vec<Effect<Part>> {
        if self.modern()
            && button == Button::Right
            && !down
            && let Some(t) = self.hit(x, y).and_then(Self::tip)
        {
            return vec![Effect::Activate(Part::Tip(t))];
        }
        Vec::new()
    }
    fn double_click(&mut self, x: i32, y: i32) -> Option<Vec<Effect<Part>>> {
        let p @ Part::Back(_) = self.hit(x, y)? else { return None };
        self.activate(p);
        Some(vec![Effect::Activate(Part::Ok)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::Controller;

    #[test]
    fn options_only_edits_the_copy_and_retains_the_cumulative_choice() {
        let original = Settings::default();
        let mut options = Choices::options(Era::Win2000, &original);
        let mut keys = Controller::default();
        keys.access(&mut options, 'v');
        keys.access(&mut options, 'c');
        assert_eq!(options.edited().rules.scoring, Scoring::Vegas);
        assert!(options.edited().rules.cumulative);
        keys.access(&mut options, 'a');
        assert!(!options.enabled(Part::Cumulative));
        assert!(!options.tab_stops().contains(&Part::Cumulative));
        keys.access(&mut options, 'c');
        assert!(options.edited().rules.cumulative);
        assert_eq!(keys.key(&mut options, Key::Escape, Mods::default()), [Effect::Activate(Part::Cancel)]);
        assert!(!original.rules.cumulative);
    }

    #[test]
    fn double_clicking_a_back_accepts_the_resource_id_of_that_thumbnail() {
        let original = Settings::default();
        let mut deck = Choices::deck(Era::Win98, &original);
        let mut input = Controller::default();
        input.key(&mut deck, Key::Down, Mods::default());
        assert_eq!(deck.edited().deck, 1);
        input.key(&mut deck, Key::Tab, Mods::default());
        assert_eq!(deck.focus(), Part::Ok);
        assert_eq!(deck.edited().deck, 1);
        let first = deck.rect(Part::Back(0)).unwrap();
        let x = first.x + 2 * 54 + 3;
        let y = first.y + 3;
        assert_eq!(deck.double_click(x, y), Some(vec![Effect::Activate(Part::Ok)]));
        assert_eq!(deck.edited().deck, 6);
        assert!(deck.edited().remember_deck);
        assert!(!original.remember_deck);
    }
}
