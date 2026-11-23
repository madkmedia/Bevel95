//! One line of editable text, as Windows' own edit box has it (brief 95): the text, the caret,
//! the selection's other end, how far the text is moved so the caret shows, one step of Undo
//! (Undo again brings the change back), a length limit and the characters a field refuses; the
//! keys, the mouse's clicks, drags and double clicks, and the field's right-click menu (Undo, a
//! line, Cut, Copy, Paste, Delete, a line, Select All). Every one-line field of Bevel95 keeps one
//! of these; what it means for the field around it (Enter, Esc, Tab) stays with that field.
//!
//! The clipboard is the person's own and is not touched here: Cut and Copy answer with the text
//! to put on it (`Done::Copy`), Paste with `Done::Paste` (the caller reads the clipboard and gives
//! the text to `paste`), so the tests never meet the real clipboard.
//!
//! Measured on the Reference PCs (`private/probes/start-long/survey.md`, part 3): the keys and
//! the menu are the same in 98's and 2000's address field and Run box; Ctrl+A did nothing there
//! (Bevel95 selects everything: a deliberate difference, decisions.md); a double click selects a
//! word, and an address without spaces is one word.

use std::cell::Cell;

/// What a field lets in (control characters never).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Allow {
    /// Any character but a control character.
    #[default]
    Any,
    /// Digits only (Minesweeper's Custom Field).
    Digits,
}

impl Allow {
    pub fn lets_in(self, ch: char) -> bool {
        !ch.is_control()
            && match self {
                Allow::Any => true,
                Allow::Digits => ch.is_ascii_digit(),
            }
    }
}

/// What a key or a menu entry does to a field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Left,
    Right,
    Home,
    End,
    /// Ctrl+Left: to the start of this word or the one before.
    WordLeft,
    /// Ctrl+Right: to the start of the next word.
    WordRight,
    Backspace,
    Delete,
    /// The menu's Delete: the selection goes (nothing without one).
    Clear,
    SelectAll,
    Undo,
    Cut,
    Copy,
    Paste,
}

/// What came of an `Op`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Done {
    /// Nothing changed (a key at an end, Copy with nothing selected).
    Nothing,
    /// The caret or the selection moved; the text is the same.
    Moved,
    /// The text changed.
    Changed,
    /// Put this on the clipboard (Cut, which also changed the text, or Copy).
    Copy(String),
    /// Read the clipboard's text and give it to `paste`.
    Paste,
}

impl Done {
    /// The text changed (a Cut that took something away included).
    pub fn changed_text(&self, cut: bool) -> bool {
        matches!(self, Done::Changed) || (cut && matches!(self, Done::Copy(_)))
    }
}

// Windows' virtual keys, as numbers (the core has no Windows types).
const VK_BACK: u16 = 0x08;
const VK_END: u16 = 0x23;
const VK_HOME: u16 = 0x24;
const VK_LEFT: u16 = 0x25;
const VK_RIGHT: u16 = 0x27;
const VK_INSERT: u16 = 0x2D;
const VK_DELETE: u16 = 0x2E;

/// The field's key for a virtual key with Shift and Ctrl as held, as Windows' edit box takes it,
/// and whether the caret keys extend the selection (Shift). `None`: not a key of the field (the
/// field around it may use it: Enter, Esc, Tab, F4...). Ctrl+A selects everything (deliberately,
/// brief 95; the measured 98 and 2000 did nothing). The old pairs: Shift+Delete cuts, Ctrl+Insert
/// copies, Shift+Insert pastes.
pub fn op_for_key(vk: u16, shift: bool, ctrl: bool) -> Option<(Op, bool)> {
    let op = match (vk, ctrl) {
        (VK_LEFT, false) => Op::Left,
        (VK_RIGHT, false) => Op::Right,
        (VK_LEFT, true) => Op::WordLeft,
        (VK_RIGHT, true) => Op::WordRight,
        (VK_HOME, _) => Op::Home,
        (VK_END, _) => Op::End,
        (VK_BACK, false) => Op::Backspace,
        (VK_DELETE, false) if shift => Op::Cut,
        (VK_DELETE, false) => Op::Delete,
        (VK_INSERT, true) if !shift => Op::Copy,
        (VK_INSERT, false) if shift => Op::Paste,
        (k, true) if !shift && k == u16::from(b'A') => Op::SelectAll,
        (k, true) if !shift && k == u16::from(b'X') => Op::Cut,
        (k, true) if !shift && k == u16::from(b'C') => Op::Copy,
        (k, true) if !shift && k == u16::from(b'V') => Op::Paste,
        (k, true) if !shift && k == u16::from(b'Z') => Op::Undo,
        _ => return None,
    };
    let extends = shift && matches!(op, Op::Left | Op::Right | Op::Home | Op::End | Op::WordLeft | Op::WordRight);
    Some((op, extends))
}

/// The state one step of Undo gives back.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Snapshot {
    text: String,
    caret: usize,
    anchor: usize,
}

/// One line of editable text (positions in characters).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Edit {
    pub text: String,
    pub caret: usize,
    /// The selection's other end (equal to `caret`: nothing selected).
    pub anchor: usize,
    /// How many pixels the text is moved left so the caret shows (`fit`), kept from paint to
    /// paint: the text moves only when the caret leaves the box, as in Windows' edit boxes.
    pub scroll: Cell<i32>,
    /// At most this many characters (`None`: no limit).
    pub max: Option<usize>,
    pub allow: Allow,
    undo: Option<Snapshot>,
    /// The last change was typing that a further letter goes on with (one step of Undo for a
    /// typed word, as Windows' edit box has it).
    typing: bool,
}

impl Edit {
    pub fn new(text: &str) -> Edit {
        let n = text.chars().count();
        Edit { text: text.to_string(), caret: n, anchor: n, ..Edit::default() }
    }

    pub fn with_rule(mut self, max: Option<usize>, allow: Allow) -> Edit {
        self.max = max;
        self.allow = allow;
        self
    }

    pub fn len(&self) -> usize {
        self.text.chars().count()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// New text from outside (a place shown, a field filled): no Undo to it, the caret at its end.
    pub fn set(&mut self, text: &str) {
        self.text = text.to_string();
        self.caret = self.len();
        self.anchor = self.caret;
        self.undo = None;
        self.typing = false;
        self.scroll.set(0);
    }

    /// The selected characters (start, end).
    pub fn selection(&self) -> (usize, usize) {
        (self.anchor.min(self.caret), self.anchor.max(self.caret))
    }

    pub fn has_selection(&self) -> bool {
        self.anchor != self.caret
    }

    pub fn selected_text(&self) -> String {
        let (s, e) = self.selection();
        self.text.chars().skip(s).take(e - s).collect()
    }

    pub fn all_selected(&self) -> bool {
        self.selection() == (0, self.len())
    }

    pub fn can_undo(&self) -> bool {
        self.undo.is_some()
    }

    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.caret = self.len();
        self.typing = false;
    }

    /// The caret to `at` with nothing selected.
    pub fn place(&mut self, at: usize) {
        self.caret = at.min(self.len());
        self.anchor = self.caret;
        self.typing = false;
    }

    fn byte(&self, chars: usize) -> usize {
        self.text.char_indices().nth(chars).map_or(self.text.len(), |(b, _)| b)
    }

    fn remember(&mut self, typing: bool) {
        if !(typing && self.typing) {
            self.undo = Some(Snapshot { text: self.text.clone(), caret: self.caret, anchor: self.anchor });
        }
        self.typing = typing;
    }

    fn remove(&mut self, s: usize, e: usize) {
        let (bs, be) = (self.byte(s), self.byte(e));
        self.text.replace_range(bs..be, "");
        self.caret = s;
        self.anchor = s;
    }

    /// The characters of `text` this field lets in, as many as its limit leaves room for in place
    /// of the selection; a paste is taken up to the first line break, as Windows' one-line edit box
    /// takes it, and a refused character in it is left out.
    fn admitted(&self, text: &str) -> String {
        let (s, e) = self.selection();
        let room = self.max.map_or(usize::MAX, |m| m.saturating_sub(self.len() - (e - s)));
        let line = text.split(['\r', '\n']).next().unwrap_or("");
        line.chars().filter(|&c| self.allow.lets_in(c)).take(room).collect()
    }

    fn insert(&mut self, text: &str, typing: bool) -> bool {
        let text = self.admitted(text);
        if text.is_empty() {
            return false;
        }
        self.remember(typing);
        let (s, e) = self.selection();
        self.remove(s, e);
        let b = self.byte(self.caret);
        self.text.insert_str(b, &text);
        self.caret += text.chars().count();
        self.anchor = self.caret;
        true
    }

    /// A typed character (WM_CHAR): it replaces the selection. False: refused (a control
    /// character, one the field does not let in, or the field is full).
    pub fn type_char(&mut self, ch: char) -> bool {
        self.allow.lets_in(ch) && self.insert(&ch.to_string(), true)
    }

    /// Text from the clipboard (after `Done::Paste`): it replaces the selection, up to the first
    /// line break, without the characters the field refuses, cut to its limit.
    pub fn paste(&mut self, text: &str) -> bool {
        let done = self.insert(text, false);
        self.typing = false;
        done
    }

    fn word_start(chars: &[char], at: usize) -> usize {
        let mut i = at;
        while i > 0 && chars[i - 1].is_whitespace() {
            i -= 1;
        }
        while i > 0 && !chars[i - 1].is_whitespace() {
            i -= 1;
        }
        i
    }

    fn next_word(chars: &[char], at: usize) -> usize {
        let mut i = at;
        while i < chars.len() && !chars[i].is_whitespace() {
            i += 1;
        }
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        i
    }

    /// The word at character `at`: a run of characters without a space (an address without spaces
    /// is one word, as measured), or the run of spaces there.
    pub fn word_at(&self, at: usize) -> (usize, usize) {
        let chars: Vec<char> = self.text.chars().collect();
        let n = chars.len();
        if n == 0 {
            return (0, 0);
        }
        let at = at.min(n - 1);
        let space = chars[at].is_whitespace();
        let mut s = at;
        while s > 0 && chars[s - 1].is_whitespace() == space {
            s -= 1;
        }
        let mut e = at;
        while e < n && chars[e].is_whitespace() == space {
            e += 1;
        }
        (s, e)
    }

    /// A key or a menu entry; `extend`: the caret keys grow the selection (Shift).
    pub fn apply(&mut self, op: Op, extend: bool) -> Done {
        let n = self.len();
        let before = (self.caret, self.anchor);
        let go = |f: &mut Edit, to: usize| {
            f.caret = to.min(n);
            if !extend {
                f.anchor = f.caret;
            }
            f.typing = false;
        };
        let (s, e) = self.selection();
        match op {
            Op::Left => {
                let to = if !extend && s != e { s } else { self.caret.saturating_sub(1) };
                go(self, to);
            }
            Op::Right => {
                let to = if !extend && s != e { e } else { self.caret + 1 };
                go(self, to);
            }
            Op::Home => go(self, 0),
            Op::End => go(self, n),
            Op::WordLeft => {
                let chars: Vec<char> = self.text.chars().collect();
                let to = Self::word_start(&chars, self.caret);
                go(self, to);
            }
            Op::WordRight => {
                let chars: Vec<char> = self.text.chars().collect();
                let to = Self::next_word(&chars, self.caret);
                go(self, to);
            }
            Op::SelectAll => self.select_all(),
            Op::Backspace | Op::Delete | Op::Clear => {
                if s != e {
                    self.remember(false);
                    self.remove(s, e);
                    return Done::Changed;
                }
                let gone = match op {
                    Op::Backspace if self.caret > 0 => Some((self.caret - 1, self.caret)),
                    Op::Delete if self.caret < n => Some((self.caret, self.caret + 1)),
                    _ => None,
                };
                return match gone {
                    Some((a, b)) => {
                        self.remember(false);
                        self.remove(a, b);
                        Done::Changed
                    }
                    None => Done::Nothing,
                };
            }
            Op::Undo => {
                let Some(back) = self.undo.take() else {
                    return Done::Nothing;
                };
                self.undo = Some(Snapshot { text: std::mem::take(&mut self.text), caret: self.caret, anchor: self.anchor });
                self.text = back.text;
                self.caret = back.caret.min(self.len());
                self.anchor = back.anchor.min(self.len());
                self.typing = false;
                return Done::Changed;
            }
            Op::Copy => {
                return if s == e { Done::Nothing } else { Done::Copy(self.selected_text()) };
            }
            Op::Cut => {
                if s == e {
                    return Done::Nothing;
                }
                let text = self.selected_text();
                self.remember(false);
                self.remove(s, e);
                return Done::Copy(text);
            }
            Op::Paste => return Done::Paste,
        }
        if (self.caret, self.anchor) == before { Done::Nothing } else { Done::Moved }
    }

    /// A virtual key with Shift and Ctrl (`op_for_key`); `None`: not the field's key.
    pub fn key(&mut self, vk: u16, shift: bool, ctrl: bool) -> Option<Done> {
        let (op, extend) = op_for_key(vk, shift, ctrl)?;
        Some(self.apply(op, extend))
    }

    /// The character nearest `x` pixels into the text as shown (`x` counts from the text's first
    /// pixel in the box; the scroll is added here), by `measure` (the field's own font).
    pub fn index_at(&self, measure: &dyn Fn(&str) -> i32, x: i32) -> usize {
        let x = x + self.scroll.get();
        let mut best = 0;
        let mut best_d = i32::MAX;
        let mut prefix = String::new();
        for (k, ch) in std::iter::once(None).chain(self.text.chars().map(Some)).enumerate() {
            if let Some(ch) = ch {
                prefix.push(ch);
            }
            let d = (measure(&prefix) - x).abs();
            if d < best_d {
                best_d = d;
                best = k;
            }
        }
        best
    }

    /// A press of the mouse at character `at`: the caret there, nothing selected (`extend`, with
    /// Shift: the selection grows to there).
    pub fn press(&mut self, at: usize, extend: bool) {
        self.caret = at.min(self.len());
        if !extend {
            self.anchor = self.caret;
        }
        self.typing = false;
    }

    /// The mouse dragged to character `at`: the selection runs from where it was pressed.
    pub fn drag(&mut self, at: usize) {
        self.caret = at.min(self.len());
        self.typing = false;
    }

    /// A double click at character `at`: its word selected.
    pub fn double_click(&mut self, at: usize) {
        let (s, e) = self.word_at(at);
        self.anchor = s;
        self.caret = e;
        self.typing = false;
    }

    /// How far the text is moved left to keep the caret inside a box `width` pixels wide.
    pub fn fit(&self, measure: &dyn Fn(&str) -> i32, width: i32) -> i32 {
        let room = (width - 1).max(1);
        let at = measure(&self.text.chars().take(self.caret).collect::<String>());
        let total = measure(&self.text);
        let mut s = self.scroll.get();
        if at - s > room {
            s = at - room;
        }
        if at < s {
            s = at;
        }
        let s = s.min((total - room).max(0)).max(0);
        self.scroll.set(s);
        s
    }

    /// The field's right-click menu as Windows' edit box has it, grey as measured: Undo with
    /// nothing to undo; Cut, Copy and Delete with nothing selected; Paste with no text on the
    /// clipboard (`clipboard_has_text`); Select All when everything is selected (or nothing is
    /// there). Its entries' ops: `menu_op`.
    pub fn menu(&self, clipboard_has_text: bool) -> crate::paint::Menu {
        use crate::paint::{Menu, MenuEntry};
        let grey = |label: &str, disabled: bool| MenuEntry { disabled, ..MenuEntry::item(label, None) };
        let sel = self.has_selection();
        let entries = vec![
            grey("&Undo", !self.can_undo()),
            MenuEntry::separator(),
            grey("Cu&t", !sel),
            grey("&Copy", !sel),
            grey("&Paste", !clipboard_has_text),
            grey("&Delete", !sel),
            MenuEntry::separator(),
            grey("Select &All", self.is_empty() || self.all_selected()),
        ];
        Menu { show_keys: true, ..Menu::new(entries) }
    }
}

/// Draws a field's line of text at (x, y) into the `width` pixels it has (clipped there, moved left
/// so the caret shows: `Edit::fit`), as Windows' edit box: with the keyboard (`focused`) the
/// selection in the highlight colors and the caret while it blinks on (`caret_on`). What is behind
/// (the field's white) stays.
#[allow(clippy::too_many_arguments)]
pub fn paint_line(
    fb: &mut crate::fb::Framebuffer,
    font: &crate::text::Font,
    edit: &Edit,
    x: i32,
    y: i32,
    width: i32,
    focused: bool,
    caret_on: bool,
    colors: &crate::theme::Colors,
) {
    use crate::fb::{Framebuffer, Rect};
    if width <= 0 {
        return;
    }
    let measure = |s: &str| font.measure(s);
    let scroll = if focused { edit.fit(&measure, width) } else { 0 };
    let mut line = Framebuffer::new(width, font.height, 0);
    line.blit_part(fb, Rect::new(x, y, width, font.height), 0, 0);
    font.draw(&mut line, -scroll, 0, &edit.text, colors.text);
    let (s, e) = edit.selection();
    if focused && s != e {
        let before: String = edit.text.chars().take(s).collect();
        let part: String = edit.text.chars().skip(s).take(e - s).collect();
        let sx = font.measure(&before) - scroll;
        line.fill(Rect::new(sx, 0, font.measure(&part), font.height), colors.menu_highlight);
        font.draw(&mut line, sx, 0, &part, colors.menu_highlight_text);
    }
    if focused && caret_on {
        let before: String = edit.text.chars().take(edit.caret).collect();
        line.vline(font.measure(&before) - scroll, 0, font.height, colors.text);
    }
    fb.blit(&line, x, y);
}

/// Where a field's menu opens from the keyboard (Shift+F10): the field's middle (the 2000
/// Reference PC: the Run box's menu stood at the field's middle, upward for want of room below;
/// its left edge 1 pixel right of the exact middle of the even-width box).
pub fn middle(field: crate::fb::Rect) -> Option<(i32, i32)> {
    Some((field.x + (field.w + 1) / 2, field.y + field.h / 2))
}

/// The op of entry `index` of `Edit::menu`.
pub fn menu_op(index: usize) -> Option<Op> {
    match index {
        0 => Some(Op::Undo),
        2 => Some(Op::Cut),
        3 => Some(Op::Copy),
        4 => Some(Op::Paste),
        5 => Some(Op::Clear),
        7 => Some(Op::SelectAll),
        _ => None,
    }
}

/// The person's clipboard, as a field's host sees it: plain text only. The Windows side has the
/// real one (`bevel_winkit::clipboard`); tests use `Memory`, so no test meets the real clipboard.
pub trait Clipboard {
    /// The clipboard's plain text, if it holds any.
    fn text(&mut self) -> Option<String>;
    /// Puts plain text on the clipboard. False: it could not be done (another program held it).
    fn set_text(&mut self, text: &str) -> bool;
    /// It holds plain text (the menu's Paste).
    fn has_text(&mut self) -> bool;
}

/// A clipboard in memory, for tests.
#[derive(Clone, Debug, Default)]
pub struct Memory(pub Option<String>);

impl Clipboard for Memory {
    fn text(&mut self) -> Option<String> {
        self.0.clone()
    }
    fn set_text(&mut self, text: &str) -> bool {
        self.0 = Some(text.to_string());
        true
    }
    fn has_text(&mut self) -> bool {
        self.0.is_some()
    }
}

/// Carries out `done` against a clipboard: Cut and Copy put their text on it, Paste takes its text
/// into `edit`. True if the text of `edit` changed. Hosts that run the clipboard later (a form's
/// effects) do the same in their own time.
pub fn with_clipboard(edit: &mut Edit, done: &Done, clipboard: &mut dyn Clipboard) -> bool {
    match done {
        Done::Copy(text) => {
            clipboard.set_text(text);
            false
        }
        Done::Paste => clipboard.text().is_some_and(|t| edit.paste(&t)),
        Done::Changed => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(e: &mut Edit, vk: u8, shift: bool, ctrl: bool) -> Done {
        e.key(u16::from(vk), shift, ctrl).expect("a key of the field")
    }

    #[test]
    fn caret_keys_and_shift_select() {
        let mut e = Edit::new("C:\\WINDOWS");
        assert_eq!((e.caret, e.anchor), (10, 10));
        press(&mut e, 0x24, false, false); // Home: caret to the start (survey part 3)
        assert_eq!(e.caret, 0);
        press(&mut e, 0x27, true, false); // Shift+Right selects letters
        press(&mut e, 0x27, true, false);
        assert_eq!(e.selected_text(), "C:");
        press(&mut e, 0x23, true, false); // Shift+End
        assert_eq!(e.selected_text(), "C:\\WINDOWS");
        press(&mut e, 0x25, false, false); // Left with a selection: to its start
        assert_eq!((e.caret, e.anchor), (0, 0));
        assert_eq!(press(&mut e, 0x25, false, false), Done::Nothing);
    }

    #[test]
    fn words_by_ctrl() {
        let mut e = Edit::new("notepad c:\\my file.txt");
        press(&mut e, 0x25, false, true);
        assert_eq!(e.caret, 14, "Ctrl+Left: the start of the last word");
        press(&mut e, 0x25, false, true);
        assert_eq!(e.caret, 8);
        press(&mut e, 0x25, true, true);
        assert_eq!(e.selected_text(), "notepad ");
        press(&mut e, 0x24, false, false);
        press(&mut e, 0x27, false, true);
        assert_eq!(e.caret, 8, "Ctrl+Right: the start of the next word");
        press(&mut e, 0x27, true, true);
        assert_eq!(e.selected_text(), "c:\\my ");
    }

    #[test]
    fn typing_replaces_the_selection_and_backspace_delete() {
        let mut e = Edit::new("abc");
        e.select_all();
        assert!(e.type_char('x'));
        assert_eq!(e.text, "x");
        assert!(!e.type_char('\u{8}'), "a control character is refused");
        e.type_char('y');
        press(&mut e, 0x08, false, false);
        assert_eq!(e.text, "x");
        press(&mut e, 0x24, false, false);
        press(&mut e, 0x2E, false, false);
        assert_eq!(e.text, "");
        assert_eq!(press(&mut e, 0x2E, false, false), Done::Nothing);
    }

    #[test]
    fn cut_copy_paste_and_the_old_pairs() {
        let mut clip = Memory::default();
        let mut e = Edit::new("C:\\WINDOWS\\Desktop");
        e.select_all();
        let d = press(&mut e, b'C', false, true); // Ctrl+C
        assert_eq!(d, Done::Copy("C:\\WINDOWS\\Desktop".into()));
        with_clipboard(&mut e, &d, &mut clip);
        let mut other = Edit::new("D:\\");
        other.select_all();
        let d = press(&mut other, b'V', false, true); // Ctrl+V
        assert_eq!(d, Done::Paste);
        assert!(with_clipboard(&mut other, &d, &mut clip));
        assert_eq!(other.text, "C:\\WINDOWS\\Desktop", "copy between two fields (the owner's sentence)");
        // Ctrl+X cuts.
        press(&mut other, 0x24, false, false);
        press(&mut other, 0x27, true, false);
        press(&mut other, 0x27, true, false);
        assert_eq!(press(&mut other, b'X', false, true), Done::Copy("C:".into()));
        assert_eq!(other.text, "\\WINDOWS\\Desktop");
        // The old pairs.
        other.select_all();
        assert_eq!(e.key(0x2D, false, true), Some(Done::Copy("C:\\WINDOWS\\Desktop".into())), "Ctrl+Insert copies");
        assert_eq!(other.key(0x2E, true, false), Some(Done::Copy("\\WINDOWS\\Desktop".into())), "Shift+Delete cuts");
        assert_eq!(other.key(0x2D, true, false), Some(Done::Paste), "Shift+Insert pastes");
        // Nothing selected: Copy and Cut do nothing.
        assert_eq!(press(&mut other, b'C', false, true), Done::Nothing);
        assert_eq!(press(&mut other, b'X', false, true), Done::Nothing);
    }

    #[test]
    fn ctrl_a_selects_everything() {
        // A deliberate difference: the measured 98 and 2000 did nothing on Ctrl+A.
        let mut e = Edit::new("Media");
        press(&mut e, b'A', false, true);
        assert!(e.all_selected());
    }

    #[test]
    fn a_paste_takes_the_first_line_and_refuses_what_the_field_refuses() {
        let mut e = Edit::new("");
        assert!(e.paste("first line\r\nsecond"));
        assert_eq!(e.text, "first line");
        let mut digits = Edit::new("").with_rule(Some(3), Allow::Digits);
        digits.paste("1a2b3c4");
        assert_eq!(digits.text, "123", "refused characters left out, the limit kept");
        assert!(!digits.type_char('5'), "full");
        let mut tab = Edit::new("");
        tab.paste("a\tb");
        assert_eq!(tab.text, "ab", "a tab is a control character");
        assert!(!Edit::new("").paste("\nsecond"), "an empty first line pastes nothing");
    }

    #[test]
    fn the_limit_counts_the_selection_replaced() {
        let mut e = Edit::new("abc").with_rule(Some(3), Allow::Any);
        assert!(!e.type_char('d'));
        e.select_all();
        assert!(e.paste("wxyz"));
        assert_eq!(e.text, "wxy");
    }

    #[test]
    fn undo_and_undo_again() {
        let mut e = Edit::new("");
        assert!(!e.can_undo());
        for c in "hello".chars() {
            e.type_char(c);
        }
        assert_eq!(press(&mut e, b'Z', false, true), Done::Changed);
        assert_eq!(e.text, "", "typing is one step");
        press(&mut e, b'Z', false, true);
        assert_eq!(e.text, "hello", "Undo again brings it back");
        e.select_all();
        press(&mut e, b'X', false, true);
        assert_eq!(e.text, "");
        press(&mut e, b'Z', false, true);
        assert_eq!(e.text, "hello", "a cut undone");
        // A move breaks the typing step.
        let mut f = Edit::new("");
        f.type_char('a');
        press(&mut f, 0x24, false, false);
        f.type_char('b');
        press(&mut f, b'Z', false, true);
        assert_eq!(f.text, "a");
    }

    #[test]
    fn mouse_click_drag_and_double_click() {
        let measure = |s: &str| 6 * s.chars().count() as i32;
        let mut e = Edit::new("C:\\WINDOWS\\Desktop");
        let at = e.index_at(&measure, 13);
        assert_eq!(at, 2);
        e.press(at, false);
        assert!(!e.has_selection());
        e.drag(e.index_at(&measure, 66));
        assert_eq!(e.selected_text(), "\\WINDOWS\\");
        e.double_click(4);
        assert!(e.all_selected(), "an address without spaces is one word (survey part 3)");
        let mut r = Edit::new("notepad readme.txt");
        r.double_click(10);
        assert_eq!(r.selected_text(), "readme.txt");
        r.press(18, true);
        assert_eq!(r.selected_text(), "readme.txt", "Shift+click extends");
    }

    #[test]
    fn the_menu_greys_as_measured() {
        let names = |m: &crate::paint::Menu| m.entries.iter().map(|e| (e.label.clone(), e.disabled)).collect::<Vec<_>>();
        let mut e = Edit::new("C:\\WINDOWS");
        e.select_all();
        // All selected, the clipboard empty (2000's Shift+F10 menu at the start).
        let m = e.menu(false);
        assert_eq!(
            names(&m),
            vec![
                ("&Undo".into(), true),
                ("".into(), false),
                ("Cu&t".into(), false),
                ("&Copy".into(), false),
                ("&Paste".into(), true),
                ("&Delete".into(), false),
                ("".into(), false),
                ("Select &All".into(), true),
            ]
        );
        // Nothing selected, text on the clipboard.
        e.place(3);
        let m = e.menu(true);
        let grey: Vec<bool> = m.entries.iter().map(|x| x.disabled).collect();
        assert_eq!(grey, vec![true, false, true, true, false, true, false, false]);
        // After a change, Undo is there.
        e.type_char('x');
        assert!(!e.menu(true).entries[0].disabled);
        assert_eq!(menu_op(0), Some(Op::Undo));
        assert_eq!(menu_op(5), Some(Op::Clear));
        assert_eq!(menu_op(1), None);
    }

    #[test]
    fn the_menus_delete_takes_the_selection_only() {
        let mut e = Edit::new("abc");
        e.place(1);
        assert_eq!(e.apply(Op::Clear, false), Done::Nothing);
        e.select_all();
        assert_eq!(e.apply(Op::Clear, false), Done::Changed);
        assert_eq!(e.text, "");
    }

    #[test]
    fn fit_keeps_the_caret_in_the_box() {
        let measure = |s: &str| 6 * s.chars().count() as i32;
        let e = Edit::new("a long address that does not fit");
        let s = e.fit(&measure, 61);
        assert_eq!(s, 6 * 32 - 60);
        let mut e2 = e.clone();
        e2.place(0);
        assert_eq!(e2.fit(&measure, 61), 0);
    }

    #[test]
    fn not_the_fields_keys() {
        assert_eq!(op_for_key(0x0D, false, false), None, "Enter belongs to the field around it");
        assert_eq!(op_for_key(0x1B, false, false), None);
        assert_eq!(op_for_key(0x09, false, false), None);
        assert_eq!(op_for_key(u16::from(b'A'), false, false), None, "a letter comes as WM_CHAR");
    }
}
