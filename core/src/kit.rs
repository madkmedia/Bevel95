//! The control kit's controller: what every core-drawn window with controls (Run, Shut Down,
//! Taskbar Properties, Date/Time Properties, ...) does with the keyboard and the mouse, the way
//! Windows' dialog manager did it, in one place (the review of 2026-11-09: each window had its
//! own copy). A window describes itself through `Form`; `Controller` turns key presses and
//! mouse events into changes of its state plus a few `Effect`s that the platform's window host
//! carries out (capturing the mouse, moving the window, carrying out a button).
//!
//! Shared here: Tab and Shift+Tab through the window's tab stops, Enter (the default button),
//! Esc (the cancel button), underlined access keys shown from the first key on, push buttons
//! drawn pushed in while held and carried out when released on them, scroll thumbs dragged, and
//! the title bar moving the window. Everything particular to one window (lists, text fields,
//! pages, spin buttons) stays in that window's `Form` methods, which get each event first.

use crate::art::Look;
use std::fmt::Debug;

use crate::fb::{Framebuffer, Rect};
use crate::text::Fonts;
use crate::theme::Theme;

/// The keys a window acts on (the host translates Windows' virtual keys).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Tab,
    Enter,
    Escape,
    Space,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Delete,
    F4,
    /// Shift, Ctrl and Alt alone (they do not show the access keys).
    Modifier,
    /// Any other key, by its Windows virtual-key code (Ctrl+V's V).
    Other(u16),
}

/// Shift and Ctrl held with a key.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub shift: bool,
    pub ctrl: bool,
}

/// The rows of the title bar that move the window when dragged (below its top edge, above the
/// window's contents): the same in every window measured.
pub const TITLE_ROWS: std::ops::Range<i32> = 3..21;

/// What a mouse press on a part does, as the window decides it (`Form::press`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press<P> {
    /// Held: drawn pushed in while the pointer is on it; `Form::release` when let go on it.
    Hold(P),
    /// Grabbed (a scroll thumb): `Form::drag` follows the pointer with the value given here,
    /// which the window wants back (the first row shown when it was grabbed).
    Grab(P, i32),
    /// Carried out at once, as the button goes down (a menu bar's titles, like the Start button).
    Act(P),
    /// Done (the window changed what it shows).
    Done,
    /// Nothing for this window: on the title bar, the window is moved.
    Nothing,
}

/// The mouse pointer a window shows over one of its places (`Form::pointer`). The platform
/// turns each into Windows' own pointer where today's Windows still has the shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pointer {
    /// The ordinary arrow (the window class's own).
    Arrow,
    /// The text pointer (I-beam) over a field one can type in.
    Text,
    /// The pointing hand over a link.
    Hand,
    /// The two-sided arrow left and right (98's and 2000's bar between Explorer's panes).
    SizeWE,
    /// A Details header's divider: the header control's own pointer (a double bar crossed by a
    /// two-sided arrow; comctl32's cursor 106, the same in 95, 98, 2000 and today).
    HeaderDivider,
    /// Windows 95 Explorer's own pointer over the bar between its panes (`SPLIT95`).
    Split95,
}

/// A pointer drawn from pixels: rows of `#` (black) and `.` (see-through), and the pixel the
/// pointer points with. Every see-through pixel touching a black one (also corner to corner) is
/// white: the outline Windows' black pointers have.
pub struct PointerArt {
    pub rows: &'static [&'static str],
    pub hot: (i32, i32),
}

impl PointerArt {
    /// Each pixel: `Some(true)` black, `Some(false)` white (the outline), `None` see-through.
    pub fn pixel(&self, x: i32, y: i32) -> Option<bool> {
        let black = |x: i32, y: i32| {
            y >= 0 && x >= 0 && self.rows.get(y as usize).and_then(|r| r.as_bytes().get(x as usize)) == Some(&b'#')
        };
        if black(x, y) {
            return Some(true);
        }
        let touches = (-1..=1).any(|dy| (-1..=1).any(|dx| black(x + dx, y + dy)));
        touches.then_some(false)
    }

    pub fn size(&self) -> (i32, i32) {
        (self.rows.first().map_or(0, |r| r.len() as i32), self.rows.len() as i32)
    }
}

pub const SPLIT95: PointerArt = PointerArt { rows: crate::art::rows("pointer/split", Look::Win95).rows, hot: (10, 8) };

/// The mouse's other buttons (the left one has its own path: `Form::press` and what follows).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Right,
    Middle,
}

/// What the platform's window host must do after an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect<P> {
    /// Carry out a button (OK, Apply, Browse..., the title bar's "?" and "X").
    Activate(P),
    /// Keep sending the mouse here until the button comes up.
    Capture,
    /// The button came up.
    Release,
    /// Let Windows move the window (dragging its title bar); then tell the form where it is.
    MoveWindow,
    /// Close the window (Alt+F4).
    Close,
    /// Paste the clipboard's text into the focused field (Ctrl+V): `Form::paste`.
    Paste,
}

/// A core-drawn window with controls, as the controller and the host see it.
pub trait Form {
    /// The parts the mouse and the keyboard act on.
    type Part: Copy + Eq + Debug;

    // What it looks like.
    fn title(&self) -> &str;
    /// Its size at 1×, as laid out.
    fn size(&self) -> (i32, i32);
    /// Lays it out again after a change (labels are measured with the fonts).
    fn layout(&mut self, fonts: &Fonts);
    fn paint(&self, fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts);
    /// Where its left and top edges are on the screen, in 1× pixels (Windows lines its dotted
    /// focus rectangles up with the screen).
    fn set_screen(&mut self, _x: i32, _y: i32) {}
    /// An open drop-down list that may reach past the window's edges (Run's list of earlier
    /// commands), in the window's coordinates: the host makes room for it outside the window,
    /// as Windows showed such a list in a window of its own.
    fn drop_rect(&self) -> Option<Rect> {
        None
    }
    /// Paints that list into `fb`, which is its size.
    fn paint_drop(&self, _fb: &mut Framebuffer, _theme: &Theme, _fonts: &Fonts) {}

    // Its parts.
    fn hit(&self, x: i32, y: i32) -> Option<Self::Part>;
    fn rect(&self, part: Self::Part) -> Option<Rect>;
    fn enabled(&self, part: Self::Part) -> bool;
    fn focus(&self) -> Self::Part;
    /// Moves the focus there (by Tab or a click), with whatever comes with it (a time field
    /// selects its first part, an open list closes).
    fn set_focus(&mut self, part: Self::Part);
    /// The parts Tab goes through, in order (grayed out ones left out).
    fn tab_stops(&self) -> Vec<Self::Part>;
    fn set_pressed(&mut self, part: Option<Self::Part>);
    /// Underline access keys and draw focus rectangles from now on.
    fn show_keys(&mut self);
    /// The button Enter carries out (the focused push button, else the default one).
    fn enter(&self) -> Self::Part;
    /// The button Esc carries out.
    fn escape(&self) -> Self::Part;

    // The window's own behavior; each event comes here first.
    /// A key (without Alt). `None`: not the window's, the shared handling applies.
    fn key(&mut self, _key: Key, _mods: Mods) -> Option<Vec<Effect<Self::Part>>> {
        None
    }
    /// A key with Alt held (Alt+Down on a drop-down list). `false`: not the window's.
    fn alt_key(&mut self, _key: Key) -> bool {
        false
    }
    /// An Alt shortcut that also needs Shift or Ctrl, or activates a form part.
    fn alt_key_with_mods(&mut self, key: Key, _mods: Mods) -> Option<Vec<Effect<Self::Part>>> {
        self.alt_key(key).then(Vec::new)
    }
    /// A character typed without Alt (text fields; letters that are access keys).
    fn typed(&mut self, _ch: char) -> Vec<Effect<Self::Part>> {
        Vec::new()
    }
    /// Alt+letter: the control whose access key it is does what it does.
    fn access(&mut self, ch: char) -> Vec<Effect<Self::Part>>;
    /// Space on the focused control.
    fn space(&mut self) -> Vec<Effect<Self::Part>>;
    /// The mouse went down on `part` (`None`: on no part) at (x, y).
    fn press(&mut self, part: Option<Self::Part>, x: i32, y: i32) -> Press<Self::Part>;
    /// The second press of a double-click. Forms without a distinct double-click action
    /// receive it as an ordinary press.
    fn double_click(&mut self, _x: i32, _y: i32) -> Option<Vec<Effect<Self::Part>>> {
        None
    }
    /// A held part was let go with the pointer on it (buttons are carried out by default).
    fn release(&mut self, part: Self::Part) -> Vec<Effect<Self::Part>> {
        vec![Effect::Activate(part)]
    }
    /// The pointer moved while a grabbed part is held (`grab_y`: where it was grabbed, `value`:
    /// what `Press::Grab` gave). True if what it shows changed.
    fn drag(&mut self, _part: Self::Part, _grab_y: i32, _value: i32, _x: i32, _y: i32) -> bool {
        false
    }
    /// A grabbed part was let go at (x, y) (a program's own area acts on the release).
    fn dropped(&mut self, _part: Self::Part, _x: i32, _y: i32) -> Vec<Effect<Self::Part>> {
        Vec::new()
    }
    /// The right or the middle button went down or came up at (x, y).
    fn button(&mut self, _button: Button, _down: bool, _x: i32, _y: i32) -> Vec<Effect<Self::Part>> {
        Vec::new()
    }
    /// The window became the one in front, or stopped being it (a program's title bar shows it).
    fn set_active(&mut self, _active: bool) {}
    /// The window has just become the one in front by a click on it (Windows' WA_CLICKACTIVE,
    /// right after `set_active(true)`); that click's press follows (brief 90: Windows' list view
    /// armed no slow-click rename with it, survey part 4).
    fn activated_by_click(&mut self) {}
    /// Alt alone, or F10: a window with a menu bar selects its first title (or lets go of it).
    fn menu_key(&mut self) -> Vec<Effect<Self::Part>> {
        Vec::new()
    }
    /// The pointer moved with nothing held (an open list highlights the row under it). True if
    /// what it shows changed.
    fn hover(&mut self, _x: i32, _y: i32) -> bool {
        false
    }
    /// What is left to do after the pointer moved (a drag of icons that has just begun is handed
    /// to the platform's drag and drop). Called after every move.
    fn pending(&mut self) -> Vec<Effect<Self::Part>> {
        Vec::new()
    }
    /// The pointer left the window with nothing held (a raised menu title goes flat again).
    /// True if what it shows changed.
    fn pointer_left(&mut self) -> bool {
        false
    }
    /// The mouse wheel turned (`notches` up is positive).
    fn wheel(&mut self, _notches: i32) {}
    /// Text from the clipboard for the focused field.
    fn paste(&mut self, _text: &str) {}
    /// The window is waiting for the user to point at a control for What's This? help.
    fn context_help(&self) -> bool {
        false
    }
    /// The pointer shown with the mouse at (x, y), in 1× pixels (asked whenever Windows asks
    /// the window for its pointer; not while the mouse is held, so a drag keeps its pointer).
    fn pointer(&self, _x: i32, _y: i32) -> Pointer {
        Pointer::Arrow
    }
    /// Whether the focused control shows a blinking caret now (a text or time field): the host
    /// blinks it at Windows' rate and shows it afresh after every key, as Windows does.
    fn has_caret(&self) -> bool {
        false
    }
    fn blink(&mut self, _on: bool) {}
    /// Shift and Ctrl as they are at a press of the left button (told right before `press` or
    /// `double_click`: a folder window's Ctrl+click and Shift+click).
    fn mouse_mods(&mut self, _mods: Mods) {}
}

/// The part held with the mouse and how (a button: whether it is drawn pushed in now).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Held<P> {
    Button(P, bool),
    Thumb(P, i32, i32),
}

/// Drives a `Form`: the shared keyboard and mouse rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Controller<P> {
    held: Option<Held<P>>,
    /// Which of the other buttons are down (right, middle).
    others: [bool; 2],
}

impl<P> Default for Controller<P> {
    fn default() -> Self {
        Controller { held: None, others: [false; 2] }
    }
}

impl<P: Copy + Eq + Debug> Controller<P> {
    /// A key went down (without Alt).
    pub fn key<F: Form<Part = P>>(&mut self, form: &mut F, key: Key, mods: Mods) -> Vec<Effect<P>> {
        if key != Key::Modifier {
            form.show_keys();
        }
        if let Some(effects) = form.key(key, mods) {
            return effects;
        }
        match key {
            Key::Tab if !mods.ctrl => {
                let stops = form.tab_stops();
                if !stops.is_empty() {
                    let at = stops.iter().position(|&s| s == form.focus()).unwrap_or(0);
                    let n = stops.len();
                    let next = if mods.shift { (at + n - 1) % n } else { (at + 1) % n };
                    form.set_focus(stops[next]);
                }
                Vec::new()
            }
            Key::Enter => activate(form, form.enter()),
            Key::Escape => activate(form, form.escape()),
            Key::Space => form.space(),
            _ => Vec::new(),
        }
    }

    /// A key went down with Alt held: Alt+F4 closes, letters come as `access`.
    pub fn alt_key<F: Form<Part = P>>(&mut self, form: &mut F, key: Key) -> (bool, Vec<Effect<P>>) {
        self.alt_key_with_mods(form, key, Mods::default())
    }

    pub fn alt_key_with_mods<F: Form<Part = P>>(&mut self, form: &mut F, key: Key, mods: Mods) -> (bool, Vec<Effect<P>>) {
        if key == Key::F4 {
            return (true, vec![Effect::Close]);
        }
        let Some(effects) = form.alt_key_with_mods(key, mods) else { return (false, Vec::new()) };
        form.show_keys();
        (true, effects)
    }

    /// Alt+letter.
    pub fn access<F: Form<Part = P>>(&mut self, form: &mut F, ch: char) -> Vec<Effect<P>> {
        form.show_keys();
        form.access(ch)
    }

    /// The left button went down at (x, y), in 1× pixels.
    pub fn mouse_down<F: Form<Part = P>>(&mut self, form: &mut F, x: i32, y: i32) -> Vec<Effect<P>> {
        let part = form.hit(x, y);
        match form.press(part, x, y) {
            Press::Hold(p) => {
                self.held = Some(Held::Button(p, true));
                form.set_pressed(Some(p));
                vec![Effect::Capture]
            }
            Press::Grab(p, value) => {
                self.held = Some(Held::Thumb(p, y, value));
                vec![Effect::Capture]
            }
            Press::Act(p) => vec![Effect::Activate(p)],
            Press::Done => Vec::new(),
            Press::Nothing if part.is_none() && TITLE_ROWS.contains(&y) => vec![Effect::MoveWindow],
            Press::Nothing => Vec::new(),
        }
    }

    /// The pointer moved to (x, y). True if what the window shows changed.
    pub fn mouse_move<F: Form<Part = P>>(&mut self, form: &mut F, x: i32, y: i32) -> bool {
        match self.held {
            Some(Held::Button(p, shown)) => {
                let inside = form.rect(p).is_some_and(|r| r.contains(x, y));
                if inside == shown {
                    return false;
                }
                self.held = Some(Held::Button(p, inside));
                form.set_pressed(inside.then_some(p));
                true
            }
            Some(Held::Thumb(p, grab_y, value)) => form.drag(p, grab_y, value, x, y),
            None => form.hover(x, y),
        }
    }

    /// The left button came up at (x, y).
    pub fn mouse_up<F: Form<Part = P>>(&mut self, form: &mut F, x: i32, y: i32) -> Vec<Effect<P>> {
        let Some(held) = self.held.take() else {
            return Vec::new();
        };
        form.set_pressed(None);
        // The mouse stays captured while another button is still down.
        let mut effects = if self.others.contains(&true) { Vec::new() } else { vec![Effect::Release] };
        match held {
            Held::Button(p, _) if form.rect(p).is_some_and(|r| r.contains(x, y)) && form.enabled(p) => {
                effects.extend(form.release(p));
            }
            Held::Button(..) => {}
            Held::Thumb(p, ..) => effects.extend(form.dropped(p, x, y)),
        }
        effects
    }

    /// The right or the middle button went down or came up at (x, y). The mouse is captured
    /// from the first button down to the last one up. The capture is let go *before* what the
    /// release does: a right-click's menu runs its own message loop inside it, and while the
    /// window still held the mouse the menu never saw the pointer or a click (the owner's
    /// report of 2026-11-15: Explorer's menus that did not respond and stayed up).
    pub fn button<F: Form<Part = P>>(&mut self, form: &mut F, button: Button, down: bool, x: i32, y: i32) -> Vec<Effect<P>> {
        let was_down = self.held.is_some() || self.others.contains(&true);
        self.others[button as usize] = down;
        let is_down = self.held.is_some() || self.others.contains(&true);
        let mut effects = Vec::new();
        if !was_down && is_down {
            effects.push(Effect::Capture);
        }
        if was_down && !is_down {
            effects.push(Effect::Release);
        }
        effects.extend(form.button(button, down, x, y));
        effects
    }

    /// The mouse was taken away (another window captured it): nothing is held any more.
    pub fn lost_capture<F: Form<Part = P>>(&mut self, form: &mut F) {
        self.held = None;
        self.others = [false; 2];
        form.set_pressed(None);
    }

    /// Whether a part is held with the mouse.
    pub fn holding(&self) -> bool {
        self.held.is_some()
    }
}

/// Carries out `part` if it can be used now.
pub fn activate<F: Form>(form: &F, part: F::Part) -> Vec<Effect<F::Part>> {
    if form.enabled(part) { vec![Effect::Activate(part)] } else { Vec::new() }
}

/// The next of `stops` after `from` (before it with `back`), wrapping around.
pub fn step<P: Copy + Eq>(stops: &[P], from: P, back: bool) -> Option<P> {
    let at = stops.iter().position(|&s| s == from)?;
    let n = stops.len();
    Some(stops[if back { (at + n - 1) % n } else { (at + 1) % n }])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Part {
        Field,
        Ok,
        Cancel,
        Close,
    }

    /// A small window: a field, OK (grayed while the field is empty) and Cancel, a title bar X.
    #[derive(Default)]
    struct Test {
        text: String,
        focus: Option<Part>,
        pressed: Option<Part>,
        keys_shown: bool,
    }

    impl Form for Test {
        type Part = Part;
        fn title(&self) -> &str {
            "Test"
        }
        fn size(&self) -> (i32, i32) {
            (100, 60)
        }
        fn layout(&mut self, _fonts: &Fonts) {}
        fn paint(&self, _fb: &mut Framebuffer, _theme: &Theme, _fonts: &Fonts) {}
        fn hit(&self, x: i32, y: i32) -> Option<Part> {
            [Part::Field, Part::Ok, Part::Cancel, Part::Close].into_iter().find(|&p| self.rect(p).unwrap().contains(x, y))
        }
        fn rect(&self, part: Part) -> Option<Rect> {
            Some(match part {
                Part::Close => Rect::new(80, 5, 16, 14),
                Part::Field => Rect::new(10, 25, 80, 10),
                Part::Ok => Rect::new(10, 40, 30, 15),
                Part::Cancel => Rect::new(50, 40, 30, 15),
            })
        }
        fn enabled(&self, part: Part) -> bool {
            part != Part::Ok || !self.text.is_empty()
        }
        fn focus(&self) -> Part {
            self.focus.unwrap_or(Part::Field)
        }
        fn set_focus(&mut self, part: Part) {
            self.focus = Some(part);
        }
        fn tab_stops(&self) -> Vec<Part> {
            [Part::Field, Part::Ok, Part::Cancel].into_iter().filter(|&p| self.enabled(p)).collect()
        }
        fn set_pressed(&mut self, part: Option<Part>) {
            self.pressed = part;
        }
        fn show_keys(&mut self) {
            self.keys_shown = true;
        }
        fn enter(&self) -> Part {
            if self.focus() == Part::Cancel { Part::Cancel } else { Part::Ok }
        }
        fn escape(&self) -> Part {
            Part::Cancel
        }
        fn typed(&mut self, ch: char) -> Vec<Effect<Part>> {
            if self.focus() == Part::Field {
                self.text.push(ch);
            }
            Vec::new()
        }
        fn access(&mut self, ch: char) -> Vec<Effect<Part>> {
            match ch {
                'o' => activate(self, Part::Ok),
                _ => Vec::new(),
            }
        }
        fn space(&mut self) -> Vec<Effect<Part>> {
            if self.focus() == Part::Field { Vec::new() } else { activate(self, self.focus()) }
        }
        fn press(&mut self, part: Option<Part>, _x: i32, _y: i32) -> Press<Part> {
            match part {
                Some(Part::Field) => {
                    self.set_focus(Part::Field);
                    Press::Done
                }
                Some(p) if self.enabled(p) => {
                    if p != Part::Close {
                        self.set_focus(p);
                    }
                    Press::Hold(p)
                }
                _ => Press::Nothing,
            }
        }
        // A right-click's release opens a menu (stands in for Part::ItemMenu and its kind).
        fn button(&mut self, button: Button, down: bool, _x: i32, _y: i32) -> Vec<Effect<Part>> {
            if button == Button::Right && !down { vec![Effect::Activate(Part::Ok)] } else { Vec::new() }
        }
    }

    #[test]
    fn a_right_click_lets_the_mouse_go_before_its_menu_opens() {
        // The menu runs its own message loop inside Activate: with the capture still held, the
        // pointer and every click went to the window instead (the owner's report, 2026-11-15).
        let (mut c, mut t) = (Controller::default(), Test::default());
        assert_eq!(c.button(&mut t, Button::Right, true, 30, 30), vec![Effect::Capture]);
        assert_eq!(c.button(&mut t, Button::Right, false, 30, 30), vec![Effect::Release, Effect::Activate(Part::Ok)]);
        // With the left button still down the mouse stays the window's, and nothing is released.
        assert_eq!(c.mouse_down(&mut t, 60, 45), vec![Effect::Capture]);
        c.button(&mut t, Button::Right, true, 30, 30);
        assert_eq!(c.button(&mut t, Button::Right, false, 30, 30), vec![Effect::Activate(Part::Ok)]);
    }

    fn keys(c: &mut Controller<Part>, t: &mut Test, key: Key, shift: bool) -> Vec<Effect<Part>> {
        c.key(t, key, Mods { shift, ctrl: false })
    }

    #[test]
    fn tab_goes_through_the_stops_and_skips_grayed_ones() {
        let (mut c, mut t) = (Controller::default(), Test::default());
        keys(&mut c, &mut t, Key::Tab, false);
        assert_eq!(t.focus(), Part::Cancel, "OK is grayed while the field is empty");
        keys(&mut c, &mut t, Key::Tab, false);
        assert_eq!(t.focus(), Part::Field);
        keys(&mut c, &mut t, Key::Tab, true);
        assert_eq!(t.focus(), Part::Cancel);
        assert!(t.keys_shown);
    }

    #[test]
    fn enter_and_escape_carry_out_their_buttons_when_usable() {
        let (mut c, mut t) = (Controller::default(), Test::default());
        assert_eq!(keys(&mut c, &mut t, Key::Enter, false), vec![], "OK is grayed");
        t.typed('x');
        assert_eq!(keys(&mut c, &mut t, Key::Enter, false), vec![Effect::Activate(Part::Ok)]);
        assert_eq!(keys(&mut c, &mut t, Key::Escape, false), vec![Effect::Activate(Part::Cancel)]);
        assert_eq!(c.alt_key(&mut t, Key::F4), (true, vec![Effect::Close]));
        assert_eq!(c.access(&mut t, 'o'), vec![Effect::Activate(Part::Ok)]);
    }

    #[test]
    fn a_button_acts_only_when_let_go_on_it() {
        let (mut c, mut t) = (Controller::default(), Test::default());
        assert_eq!(c.mouse_down(&mut t, 60, 45), vec![Effect::Capture]);
        assert_eq!((t.pressed, t.focus()), (Some(Part::Cancel), Part::Cancel));
        c.mouse_move(&mut t, 5, 5);
        assert_eq!(t.pressed, None, "drawn up while the pointer is off it");
        c.mouse_move(&mut t, 60, 45);
        assert_eq!(t.pressed, Some(Part::Cancel));
        assert_eq!(c.mouse_up(&mut t, 60, 45), vec![Effect::Release, Effect::Activate(Part::Cancel)]);
        // pressed, moved off, let go: nothing happens
        c.mouse_down(&mut t, 60, 45);
        assert_eq!(c.mouse_up(&mut t, 5, 50), vec![Effect::Release]);
        assert!(!c.holding());
    }

    #[test]
    fn the_title_bar_moves_the_window() {
        let (mut c, mut t) = (Controller::default(), Test::default());
        assert_eq!(c.mouse_down(&mut t, 30, 10), vec![Effect::MoveWindow]);
        assert_eq!(c.mouse_down(&mut t, 30, 22), vec![], "below the title bar: nothing");
        // the X is a button, not the title bar
        assert_eq!(c.mouse_down(&mut t, 85, 10), vec![Effect::Capture]);
        assert_eq!(t.focus(), Part::Field, "title bar buttons take no focus");
    }

    #[test]
    fn step_wraps_around() {
        assert_eq!(step(&[1, 2, 3], 3, false), Some(1));
        assert_eq!(step(&[1, 2, 3], 1, true), Some(3));
        assert_eq!(step(&[1, 2, 3], 9, false), None);
    }
}
