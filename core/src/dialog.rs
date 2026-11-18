//! The Start menu's dialogs, drawn as Windows 95, 98 and 2000 drew them: Run, Shut Down
//! Windows, and (98 and 2000) Log Off Windows.
//!
//! Every position here was measured on GUIdebook's screenshots of the real dialogs, and
//! `cargo compare-refs` checks the drawings against them (docs/theme-spec.md, "Dialogs").
//! This module only draws and lays out; the platform layer owns the windows and the actions.

use crate::art::Look;
use crate::droplist::{self, DropList, ListPart, Shown};
use crate::fb::{Framebuffer, Icon, Rect};
use crate::icons;
use crate::kit::{self, Effect, Key, Mods, Press};
use crate::text::{Font, Fonts, split_access_key};
use crate::theme::{Colors, Era, Theme, rgb};
use crate::window::{self, ButtonLook, Frame, TitleButton};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogKind {
    Run,
    ShutDown,
    /// Windows 98 and 2000 (Windows 95 logs off from its Shut Down dialog).
    LogOff,
}

/// The parts of a dialog that the mouse and the keyboard act on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    /// The "?" and "X" buttons of the title bar.
    TitleHelp,
    TitleClose,
    /// Run: the command line and its drop-down button.
    Command,
    DropDown,
    /// Run: a part of the open list of earlier commands.
    List(ListPart),
    /// Shut Down (95, 98): option button `i`.
    Option(usize),
    /// Shut Down (2000): the list of choices (closed, showing the chosen one).
    Choice,
    Ok,
    Cancel,
    Browse,
    Yes,
    No,
    Help,
}

impl Part {
    fn is_push_button(self) -> bool {
        matches!(self, Part::Ok | Part::Cancel | Part::Browse | Part::Yes | Part::No | Part::Help)
    }
}

/// What one Shut Down choice is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShutDownChoice {
    ShutDown,
    Restart,
    /// Windows 95's "Restart the computer in MS-DOS mode?": here a plain restart.
    RestartDos,
    LogOff,
    StandBy,
}

/// The choices of the Shut Down dialog, in the order shown, with their labels (and, in
/// Windows 2000, the line of explanation under the list).
pub fn shut_down_choices(era: Era) -> &'static [(ShutDownChoice, &'static str, &'static str)] {
    match era {
        Era::Win95 => &[
            (ShutDownChoice::ShutDown, "&Shut down the computer?", ""),
            (ShutDownChoice::Restart, "&Restart the computer?", ""),
            (ShutDownChoice::RestartDos, "Restart the computer in &MS-DOS mode?", ""),
            (ShutDownChoice::LogOff, "&Close all programs and log on as a different user?", ""),
        ],
        Era::Win98 => &[
            (ShutDownChoice::StandBy, "S&tand by", ""),
            (ShutDownChoice::ShutDown, "&Shut down", ""),
            (ShutDownChoice::Restart, "&Restart", ""),
            (ShutDownChoice::RestartDos, "Restart in &MS-DOS mode", ""),
        ],
        Era::Win2000 => &[
            (ShutDownChoice::LogOff, "Log off", "Ends your session, leaving the computer running on full power."),
            (
                ShutDownChoice::ShutDown,
                "Shut down",
                "Ends your session and shuts down Windows so that you can safely turn off power.",
            ),
            (ShutDownChoice::Restart, "Restart", "Ends your session, shuts down Windows, and starts Windows again."),
            (
                ShutDownChoice::StandBy,
                "Stand by",
                "Maintains your session, keeping the computer running on low power with data still in memory.",
            ),
        ],
    }
}

/// What a dialog shows right now.
#[derive(Clone, Debug)]
pub struct Dialog {
    pub kind: DialogKind,
    pub era: Era,
    /// Run: the command typed so far, and the caret's position in it (in characters).
    pub text: String,
    pub caret: usize,
    /// The caret blinks: drawn only while this is set.
    pub caret_on: bool,
    /// Shut Down: the chosen entry of `shut_down_choices`.
    pub choice: usize,
    /// Windows 2000 Shut Down: the list of choices is open, and the entry under the pointer.
    pub list_open: bool,
    pub list_hover: usize,
    /// The part with the keyboard focus.
    pub focus: Part,
    /// The button held down with the mouse, drawn pushed in while the pointer is on it.
    pub pressed: Option<Part>,
    /// Underline access keys and draw focus rectangles: Windows 95 and 98 always do; Windows
    /// 2000 once the keyboard has been used.
    pub show_keys: bool,
}

impl Dialog {
    pub fn new(kind: DialogKind, era: Era) -> Dialog {
        // Shut Down starts on "Shut down" (Windows 98 lists Stand by first).
        let choice = match (kind, era) {
            (DialogKind::ShutDown, Era::Win98 | Era::Win2000) => 1,
            _ => 0,
        };
        let focus = match (kind, era) {
            (DialogKind::Run, _) => Part::Command,
            (DialogKind::ShutDown, Era::Win95 | Era::Win98) => Part::Option(choice),
            (DialogKind::ShutDown, Era::Win2000) => Part::Choice,
            (DialogKind::LogOff, _) => Part::Yes,
        };
        Dialog {
            kind,
            era,
            text: String::new(),
            caret: 0,
            caret_on: true,
            choice,
            list_open: false,
            list_hover: choice,
            focus,
            pressed: None,
            show_keys: era.always_shows_keys(),
        }
    }

    /// The push button Enter presses: the focused one, else the dialog's default.
    pub fn default_button(&self) -> Part {
        if self.focus.is_push_button() {
            return self.focus;
        }
        match (self.kind, self.era) {
            (DialogKind::Run, _) | (DialogKind::ShutDown, Era::Win98 | Era::Win2000) => Part::Ok,
            (DialogKind::ShutDown, Era::Win95) | (DialogKind::LogOff, _) => Part::Yes,
        }
    }

    /// Whether a part can be used now (Run's OK needs a command).
    pub fn enabled(&self, part: Part) -> bool {
        !(self.kind == DialogKind::Run && part == Part::Ok && self.text.trim().is_empty())
    }
}

/// Where everything goes, at 1×.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DialogLayout {
    pub width: i32,
    pub height: i32,
    pub title: &'static str,
    /// The parts, in Tab order.
    pub parts: Vec<(Part, Rect)>,
}

impl DialogLayout {
    pub fn rect(&self, part: Part) -> Option<Rect> {
        self.parts.iter().find(|(p, _)| *p == part).map(|&(_, r)| r)
    }

    /// The part under (x, y). An open list of choices covers what is under it.
    pub fn hit(&self, dialog: &Dialog, x: i32, y: i32) -> Option<Part> {
        if dialog.list_open && list_rect(self, dialog).is_some_and(|r| r.contains(x, y)) {
            return Some(Part::Choice);
        }
        self.parts.iter().find(|(_, r)| r.contains(x, y)).map(|&(p, _)| p)
    }

    /// The entry of the open list of choices under (x, y).
    pub fn list_entry(&self, dialog: &Dialog, x: i32, y: i32) -> Option<usize> {
        let r = list_rect(self, dialog)?;
        r.contains(x, y)
            .then(|| ((y - r.y - 1) / LIST_ITEM_H).clamp(0, shut_down_choices(dialog.era).len() as i32 - 1) as usize)
    }
}

/// The 16×14 title bar buttons, from the right edge of the title bar.
pub(crate) const TITLE_BUTTON: (i32, i32) = (16, 14);

/// Where the title bar's "X" (and "?", if any) go in a dialog `width` wide.
pub(crate) fn title_button_rects(width: i32) -> (Rect, Rect) {
    let close = Rect::new(width - 21, 5, TITLE_BUTTON.0, TITLE_BUTTON.1);
    (Rect::new(close.x - 18, 5, TITLE_BUTTON.0, TITLE_BUTTON.1), close)
}
/// Rows of an entry in the open list of choices (PROVISIONAL: no screenshot shows it open).
const LIST_ITEM_H: i32 = 13;

pub fn layout(dialog: &Dialog) -> DialogLayout {
    let title_buttons = |w: i32, help: bool| {
        let (help_r, close) = title_button_rects(w);
        let mut v = vec![(Part::TitleClose, close)];
        if help {
            v.insert(0, (Part::TitleHelp, help_r));
        }
        v
    };
    let button = |x: i32, y: i32, w: i32| Rect::new(x, y, w, 23);
    let (width, height, title, mut parts) = match (dialog.kind, dialog.era) {
        (DialogKind::Run, era) => {
            let (h, y) = if era == Era::Win2000 { (179, 136) } else { (163, 124) };
            // The button first: it lies inside the field's rectangle and takes clicks there.
            let parts = vec![
                (Part::DropDown, Rect::new(314, 84, 16, 17)),
                (Part::Command, Rect::new(57, 82, 275, 21)),
                (Part::Ok, button(96, y, 75)),
                (Part::Cancel, button(177, y, 75)),
                (Part::Browse, button(258, y, 75)),
            ];
            (347, h, "Run", parts)
        }
        (DialogKind::ShutDown, Era::Win95) => {
            let mut parts: Vec<(Part, Rect)> =
                (0..4).map(|i| (Part::Option(i), Rect::new(61, 65 + 21 * i as i32, 270, 16))).collect();
            parts.extend([
                (Part::Yes, button(87, 183, 78)),
                (Part::No, button(171, 183, 78)),
                (Part::Help, button(255, 183, 78)),
            ]);
            (347, 222, "Shut Down Windows", parts)
        }
        (DialogKind::ShutDown, Era::Win98) => {
            // The option rows are 12 dialog units apart, rounded to pixels: 19 or 20.
            let mut parts: Vec<(Part, Rect)> = [65, 84, 104, 123]
                .iter()
                .enumerate()
                .map(|(i, &y)| (Part::Option(i), Rect::new(61, y, 240, 16)))
                .collect();
            parts.extend([
                (Part::Ok, button(62, 158, 78)),
                (Part::Cancel, button(146, 158, 78)),
                (Part::Help, button(230, 158, 78)),
            ]);
            (323, 192, "Shut Down Windows", parts)
        }
        (DialogKind::ShutDown, Era::Win2000) => {
            let parts = vec![
                (Part::Choice, Rect::new(69, 132, 260, 21)),
                (Part::Ok, button(167, 232, 75)),
                (Part::Cancel, button(248, 232, 75)),
                (Part::Help, button(329, 232, 75)),
            ];
            (417, 269, "Shut Down Windows", parts)
        }
        (DialogKind::LogOff, _) => {
            let parts = vec![(Part::Yes, button(77, 84, 65)), (Part::No, button(152, 84, 65))];
            (288, 123, "Log Off Windows", parts)
        }
    };
    parts.extend(title_buttons(width, dialog.kind == DialogKind::Run));
    DialogLayout { width, height, title, parts }
}

pub(crate) const OPTION_ON: &[&str] = crate::art::rows("dialog/option-on", Look::Win95).rows;
pub(crate) const OPTION_OFF: &[&str] = crate::art::rows("dialog/option-off", Look::Win95).rows;
/// Windows 2000's Shut Down banner, 411×77, raw RGB.
const BANNER_2000: &[u8] = crate::art::bytes("dialog/shut-down-banner", crate::art::Look::Win2000);
const BANNER_2000_SIZE: (i32, i32) = (411, 77);
/// The same banner as Windows 2000 drew it on a 16-color screen (its own 16-color drawing, cut from
/// the Reference PC in 16 colors, `vm/win2000-16color-shutdown`).
const BANNER_2000_16: &[u8] =
    crate::art::bytes_at("dialog/shut-down-banner", crate::art::Look::Win2000, bevel_art::Depth::Sixteen);

pub(crate) fn art(fb: &mut Framebuffer, rows: &[&str], x: i32, y: i32, color: impl Fn(u8) -> Option<u32>) {
    for (dy, row) in rows.iter().enumerate() {
        for (dx, b) in row.bytes().enumerate() {
            if let Some(c) = color(b) {
                fb.set(x + dx as i32, y + dy as i32, c);
            }
        }
    }
}

/// A raised 2px bevel (buttons): highlight/dark outside, light/shadow inside.
pub(crate) fn raised(fb: &mut Framebuffer, r: Rect, c: &Colors) {
    fb.frame(r, c.highlight, c.dark_shadow);
    fb.frame(r.inset(1, 1, 1, 1), c.light, c.shadow);
}

/// The dotted focus rectangle: every other pixel, counted from `origin` (the control's corner).
pub(crate) fn focus_dots(fb: &mut Framebuffer, r: Rect, origin: (i32, i32), color: u32) {
    let mut dot = |x: i32, y: i32| {
        if ((x - origin.0) + (y - origin.1)).rem_euclid(2) == 1 {
            fb.set(x, y, color);
        }
    };
    for x in r.x..r.right() {
        dot(x, r.y);
        dot(x, r.bottom() - 1);
    }
    for y in r.y + 1..r.bottom() - 1 {
        dot(r.x, y);
        dot(r.right() - 1, y);
    }
}

/// The focus rectangle on a selected entry (Windows' DrawFocusRect over the selection color): from
/// the corner on, every other pixel of `r`'s edge inverted (in 16 colors `colors16::focus_dot`: navy
/// -> yellow, and black between them, as Windows 2000's Shut Down list on the Reference PC).
/// In True Color the others stay. PROVISIONAL in True Color
/// (not seen: 2000 shows it only after the keyboard was used).
pub(crate) fn invert_dots(fb: &mut Framebuffer, theme: &Theme, r: Rect) {
    let sixteen = theme.depth == crate::colors16::Depth::Sixteen;
    let mut dot = |x: i32, y: i32| {
        if x < 0 || y < 0 || x >= fb.width || y >= fb.height {
            return;
        }
        let p = fb.get(x, y);
        let on = ((x - r.x) + (y - r.y)).rem_euclid(2) == 0;
        match (sixteen, on) {
            (true, true) => fb.set(x, y, crate::colors16::focus_dot(theme.era, p, true)),
            // Between them black: this list's own (the desktop's labels keep their navy).
            (true, false) => fb.set(x, y, crate::colors16::BLACK),
            (false, true) => fb.set(x, y, p ^ 0x00FF_FFFF),
            (false, false) => {}
        }
    };
    for x in r.x..r.right() {
        dot(x, r.y);
        dot(x, r.bottom() - 1);
    }
    for y in r.y + 1..r.bottom() - 1 {
        dot(r.x, y);
        dot(r.right() - 1, y);
    }
}

/// Text centered in a button the way Windows centers it.
pub(crate) fn centered_label(fb: &mut Framebuffer, font: &Font, r: Rect, label: &str, color: u32, keys: bool) -> Rect {
    let w = font.measure_label(label);
    let x = r.x + (r.w - w) / 2;
    let y = r.y + (r.h - font.height) / 2 - 1;
    font.draw_label(fb, (x, y), label, i32::MAX, color, keys);
    Rect::new(x, y, w, font.height)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn push_button(
    fb: &mut Framebuffer,
    theme: &Theme,
    font: &Font,
    r: Rect,
    label: &str,
    framed: bool,
    pressed: bool,
    focused: bool,
    enabled: bool,
    keys: bool,
) {
    let c = &theme.colors;
    let inner = if framed {
        fb.frame(r, c.text, c.text);
        r.inset(1, 1, 1, 1)
    } else {
        r
    };
    if pressed {
        // PROVISIONAL (no screenshot of a pressed button): a thin shadow frame, as Windows'
        // pushed buttons look, contents 1px down and right.
        fb.frame(inner, c.shadow, c.shadow);
    } else {
        raised(fb, inner, c);
    }
    let shift = i32::from(pressed);
    let text_r = Rect::new(r.x + shift, r.y + shift, r.w, r.h);
    if enabled {
        centered_label(fb, font, text_r, label, c.text, keys);
    } else {
        centered_label(fb, font, Rect::new(text_r.x + 1, text_r.y + 1, r.w, r.h), label, c.etch, keys);
        centered_label(fb, font, text_r, label, c.shadow, keys);
    }
    if focused {
        focus_dots(fb, r.inset(4, 4, 4, 4), (r.x, r.y), c.text);
    }
}

/// A sunken 2px well (edit fields): shadow/highlight outside, dark/light inside, white within.
pub(crate) fn well(fb: &mut Framebuffer, r: Rect, c: &Colors) -> Rect {
    fb.frame(r, c.shadow, c.highlight);
    fb.frame(r.inset(1, 1, 1, 1), c.dark_shadow, c.light);
    let inside = r.inset(2, 2, 2, 2);
    fb.fill(inside, rgb(0xFFFFFF));
    inside
}

/// The drop-down button at the right end of a field: a window-style bevel and a 7×4 arrow.
pub(crate) fn drop_down(fb: &mut Framebuffer, r: Rect, c: &Colors, pressed: bool) {
    fb.fill(r, c.face);
    if pressed {
        fb.frame(r, c.shadow, c.shadow);
    } else {
        fb.frame(r, c.light, c.dark_shadow);
        fb.frame(r.inset(1, 1, 1, 1), c.highlight, c.shadow);
    }
    let shift = i32::from(pressed);
    for row in 0..4 {
        fb.hline(r.x + 4 + row + shift, r.y + 6 + row + shift, 7 - 2 * row, c.text);
    }
}

/// Wraps `text` into lines of at most `width` pixels, breaking at spaces.
pub fn wrap(font: &Font, text: &str, width: i32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split(' ') {
        let candidate = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
        if font.measure(&candidate) > width && !line.is_empty() {
            lines.push(std::mem::replace(&mut line, word.to_string()));
        } else {
            line = candidate;
        }
    }
    lines.push(line);
    lines
}

/// The open list of choices (Windows 2000 Shut Down), under the field.
fn list_rect(layout: &DialogLayout, dialog: &Dialog) -> Option<Rect> {
    let field = layout.rect(Part::Choice)?;
    let n = shut_down_choices(dialog.era).len() as i32;
    Some(Rect::new(field.x, field.bottom(), field.w, n * LIST_ITEM_H + 2))
}

/// The title bar's buttons of a dialog window, each with whether it is held down.
pub(crate) struct TitleButtons {
    pub help: Option<(Rect, bool)>,
    pub close: Option<(Rect, bool)>,
}

/// A dialog window filling the framebuffer: face color, the raised window frame, the title
/// bar with `title`, and its "?" and "X" buttons (`window::paint_frame`, which a program's
/// main window uses too).
pub(crate) fn window_frame(fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts, era: Era, title: &str, buttons: &TitleButtons) {
    let look = |button, state: Option<(Rect, bool)>| state.map(|(rect, pressed)| ButtonLook { button, rect, pressed, enabled: true });
    let buttons: Vec<ButtonLook> =
        [look(TitleButton::Help, buttons.help), look(TitleButton::Close, buttons.close)].into_iter().flatten().collect();
    window::paint_frame(fb, theme, fonts, &Frame { era, title, icon: None, active: true, buttons: &buttons });
}

/// Paints the whole dialog filling the framebuffer (`layout(dialog)`'s size).
pub fn paint_dialog(fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts, dialog: &Dialog, layout: &DialogLayout) {
    let c = &theme.colors;
    let font = fonts.regular;
    let era = dialog.era;
    let keys = dialog.show_keys;
    let buttons = TitleButtons {
        help: layout.rect(Part::TitleHelp).map(|r| (r, dialog.pressed == Some(Part::TitleHelp))),
        close: layout.rect(Part::TitleClose).map(|r| (r, dialog.pressed == Some(Part::TitleClose))),
    };
    window_frame(fb, theme, fonts, era, layout.title, &buttons);

    let icon = |kind| -> Icon {
        match (kind, era) {
            // Windows 98's Run picture is the same as 95's.
            (DialogKind::Run, Era::Win95 | Era::Win98) => icons::WIN95_RUN_DIALOG.icon(),
            (DialogKind::Run, Era::Win2000) => icons::WIN2000_RUN_DIALOG.icon(),
            (DialogKind::ShutDown, Era::Win95) => icons::WIN95_SHUT_DOWN_DIALOG.icon(),
            (DialogKind::ShutDown, Era::Win98) => icons::WIN98_SHUT_DOWN_DIALOG.icon(),
            (DialogKind::ShutDown, Era::Win2000) => icons::WIN2000_SHUT_DOWN_DIALOG.icon(),
            (DialogKind::LogOff, _) => icons::WIN2000_LOG_OFF_DIALOG.icon(),
        }
    };
    let text = |fb: &mut Framebuffer, x: i32, y: i32, lines: &[&str]| {
        for (i, line) in lines.iter().enumerate() {
            font.draw(fb, x, y + 13 * i as i32, line, c.text);
        }
    };
    let framed = dialog.default_button();
    let button = |fb: &mut Framebuffer, part: Part, label: &str| {
        if let Some(r) = layout.rect(part) {
            let pressed = dialog.pressed == Some(part);
            let focused = dialog.focus == part && dialog.show_keys;
            push_button(fb, theme, font, r, label, framed == part, pressed, focused, dialog.enabled(part), keys);
        }
    };

    match (dialog.kind, era) {
        (DialogKind::Run, _) => {
            crate::colors16::draw_picture(fb, theme, &icon(DialogKind::Run), 14, 40);
            let lines: &[&str] = match era {
                Era::Win95 => {
                    &["Type the name of a program, folder, or document, and", "Windows will open it for you."]
                }
                Era::Win98 => &[
                    "Type the name of a program, folder, document, or Internet",
                    "resource, and Windows will open it for you.",
                ],
                Era::Win2000 => &[
                    "Type the name of a program, folder, document, or",
                    "Internet resource, and Windows will open it for you.",
                ],
            };
            text(fb, 57, 40, lines);
            font.draw_label(fb, (14, 85), "&Open:", i32::MAX, c.text, keys);
            if let Some(r) = layout.rect(Part::Command) {
                let field = well(fb, r, c);
                // PROVISIONAL for 98 (its screenshot has no text in the field): as in 95.
                let x0 = field.x + if era == Era::Win2000 { 4 } else { 1 };
                font.draw(fb, x0, field.y + 1, &dialog.text, c.text);
                if dialog.caret_on && dialog.focus == Part::Command {
                    let before: String = dialog.text.chars().take(dialog.caret).collect();
                    fb.vline(x0 + font.measure(&before), field.y + 1, font.height, c.text);
                }
            }
            if let Some(r) = layout.rect(Part::DropDown) {
                drop_down(fb, r, c, dialog.pressed == Some(Part::DropDown));
            }
            button(fb, Part::Ok, "OK");
            button(fb, Part::Cancel, "Cancel");
            button(fb, Part::Browse, "&Browse...");
        }
        (DialogKind::ShutDown, Era::Win95 | Era::Win98) => {
            crate::colors16::draw_picture(fb, theme, &icon(DialogKind::ShutDown), 14, 40);
            let prompt =
                if era == Era::Win95 { "Are you sure you want to:" } else { "What do you want the computer to do?" };
            text(fb, 60, 40, &[prompt]);
            for (i, &(_, label, _)) in shut_down_choices(era).iter().enumerate() {
                let Some(r) = layout.rect(Part::Option(i)) else { continue };
                let rows = if dialog.choice == i { &OPTION_ON } else { &OPTION_OFF };
                art(fb, rows, r.x, r.y + 2, |b| match b {
                    b'g' => Some(c.shadow),
                    b'#' => Some(c.dark_shadow),
                    b'W' => Some(c.highlight),
                    b'l' => Some(c.light),
                    _ => None,
                });
                let (tx, ty) = (r.x + 17, r.y + 2);
                font.draw_label(fb, (tx, ty), label, i32::MAX, c.text, keys);
                if dialog.focus == Part::Option(i) && dialog.show_keys {
                    let w = font.measure_label(label);
                    focus_dots(fb, Rect::new(tx - 1, ty - 1, w + 2, font.height + 3), (r.x, r.y), c.text);
                }
            }
            if era == Era::Win95 {
                button(fb, Part::Yes, "&Yes");
                button(fb, Part::No, "&No");
            } else {
                button(fb, Part::Ok, "OK");
                button(fb, Part::Cancel, "Cancel");
            }
            button(fb, Part::Help, "&Help");
        }
        (DialogKind::ShutDown, Era::Win2000) => {
            let (bw, bh) = BANNER_2000_SIZE;
            let banner = if theme.depth == crate::colors16::Depth::Sixteen { BANNER_2000_16 } else { BANNER_2000 };
            for y in 0..bh {
                for x in 0..bw {
                    let i = ((y * bw + x) * 3) as usize;
                    let p = &banner[i..i + 3];
                    fb.set(3 + x, 22 + y, rgb(u32::from_be_bytes([0, p[0], p[1], p[2]])));
                }
            }
            crate::colors16::draw_picture(fb, theme, &icon(DialogKind::ShutDown), 23, 110);
            font.draw_label(fb, (69, 110), "&What do you want the computer to do?", i32::MAX, c.text, keys);
            let choices = shut_down_choices(era);
            if let Some(r) = layout.rect(Part::Choice) {
                let field = well(fb, r, c);
                let item = Rect::new(field.x + 1, field.y + 1, field.w - 2 - 16, field.h - 2);
                let focused = dialog.focus == Part::Choice && !dialog.list_open;
                let color = if focused {
                    fb.fill(item, c.menu_highlight);
                    c.menu_highlight_text
                } else {
                    c.text
                };
                font.draw(fb, item.x + 1, item.y + 1, choices[dialog.choice].1, color);
                if focused && keys {
                    // The focus rectangle once the keyboard was used: every other pixel of the
                    // selection's edge inverted (the Reference PC in 16 colors: yellow on navy).
                    invert_dots(fb, theme, item);
                }
                drop_down(
                    fb,
                    Rect::new(field.right() - 16, field.y, 16, field.h),
                    c,
                    dialog.pressed == Some(Part::Choice),
                );
            }
            for (i, line) in wrap(font, choices[dialog.choice].2, 260).iter().enumerate() {
                font.draw(fb, 69, 164 + 13 * i as i32, line, c.text);
            }
            button(fb, Part::Ok, "OK");
            button(fb, Part::Cancel, "Cancel");
            button(fb, Part::Help, "&Help");
            if dialog.list_open
                && let Some(list) = list_rect(layout, dialog)
            {
                // PROVISIONAL: no screenshot shows the list open.
                fb.fill(list, rgb(0xFFFFFF));
                fb.frame(list, c.text, c.text);
                for (i, &(_, label, _)) in choices.iter().enumerate() {
                    let row = Rect::new(list.x + 1, list.y + 1 + LIST_ITEM_H * i as i32, list.w - 2, LIST_ITEM_H);
                    let color = if i == dialog.list_hover {
                        fb.fill(row, c.menu_highlight);
                        c.menu_highlight_text
                    } else {
                        c.text
                    };
                    font.draw(fb, row.x + 2, row.y, label, color);
                }
            }
        }
        (DialogKind::LogOff, _) => {
            crate::colors16::draw_picture(fb, theme, &icon(DialogKind::LogOff), 23, 40);
            text(fb, 77, 46, &["Are you sure you want to log off?"]);
            button(fb, Part::Yes, "&Yes");
            button(fb, Part::No, "&No");
        }
    }
}

/// The access key of a part (the underlined letter of its label), for Alt+letter.
pub fn access_key(dialog: &Dialog, part: Part) -> Option<char> {
    let label = match part {
        Part::Command => "&Open:",
        Part::Browse => "&Browse...",
        Part::Yes => "&Yes",
        Part::No => "&No",
        Part::Help => "&Help",
        Part::Choice => "&What do you want the computer to do?",
        Part::Option(i) => shut_down_choices(dialog.era).get(i)?.1,
        _ => return None,
    };
    let (text, key) = split_access_key(label);
    text.chars().nth(key?).map(|c| c.to_ascii_lowercase())
}

/// The most rows Run's list of earlier commands shows (fewer when there are fewer commands):
/// measured on the reference PCs with 12, 15 and 26 commands.
pub fn run_list_rows(era: Era) -> usize {
    match era {
        Era::Win95 | Era::Win98 => 10,
        Era::Win2000 => 23,
    }
}

/// Rows the mouse wheel scrolls Run's open list per notch.
const WHEEL_ROWS: i32 = 3;

/// A dialog as a window of the control kit (`kit::Form`): its state, its layout, and for Run
/// the command history (Up and Down, and its list under the drop-down button) and where the
/// characters of the command are.
#[derive(Clone, Debug)]
pub struct DialogWindow {
    pub dialog: Dialog,
    pub layout: DialogLayout,
    /// Run: earlier commands, newest first, and which one Up/Down showed last (the list's
    /// current entry, as Windows keeps it).
    pub history: Vec<String>,
    pub history_at: Option<usize>,
    /// Run: the list of earlier commands while it is open.
    pub list: Option<Shown>,
    /// Where the window's top left corner is on the screen, and the bottom edge of its screen
    /// (1× pixels): a list that would reach past that edge opens above the field instead.
    pub screen: (i32, i32),
    pub screen_bottom: i32,
    /// Run: the left edge of the command's first character and each character's width.
    field_x: i32,
    advances: Vec<i32>,
}

impl DialogWindow {
    pub fn new(dialog: Dialog, history: Vec<String>) -> DialogWindow {
        let layout = layout(&dialog);
        DialogWindow {
            dialog,
            layout,
            history,
            history_at: None,
            list: None,
            screen: (0, 0),
            screen_bottom: i32::MAX,
            field_x: 0,
            advances: Vec::new(),
        }
    }

    /// Edits Run's command: `f` gets the text and the caret (in characters). An open list
    /// follows what is typed: nothing is highlighted, and the first command that starts with
    /// the text becomes its first row (as far as the list allows; else its first command).
    fn edit(&mut self, f: impl FnOnce(&mut String, &mut usize)) {
        let (mut text, mut caret) = (self.dialog.text.clone(), self.dialog.caret);
        f(&mut text, &mut caret);
        self.dialog.text = text;
        self.dialog.caret = caret.min(self.dialog.text.chars().count());
        self.dialog.caret_on = true;
        self.history_at = None;
        if let (Some(_), Some(list)) = (self.list, self.run_list()) {
            let found = droplist::find_prefix(&self.history, &self.dialog.text);
            self.list = Some(Shown { top: found.unwrap_or(0).min(list.max_top(self.history.len())), selected: None });
        }
    }

    /// Where Run's list of earlier commands goes: under the field, as wide, one row per command
    /// up to the look's most; above the field when it would reach past the screen's bottom edge.
    pub fn run_list(&self) -> Option<DropList> {
        if self.dialog.kind != DialogKind::Run {
            return None;
        }
        let field = self.layout.rect(Part::Command)?;
        // PROVISIONAL: with no commands at all, one empty row (not seen on the PCs).
        let rows = self.history.len().clamp(1, run_list_rows(self.dialog.era));
        let below = DropList::new(field.x, field.bottom(), field.w, rows);
        if self.screen.1.saturating_add(below.frame.bottom()) > self.screen_bottom {
            Some(DropList::new(field.x, field.y - below.frame.h, field.w, rows))
        } else {
            Some(below)
        }
    }

    /// Opens Run's list: the first command that starts with the field's text is highlighted
    /// and becomes the first row (as far as the list allows); the field keeps its text.
    pub fn open_list(&mut self) {
        let Some(list) = self.run_list() else { return };
        let found = droplist::find_prefix(&self.history, &self.dialog.text);
        self.list = Some(Shown::open(&list, self.history.len(), found));
        self.history_at = found;
    }

    /// Closes Run's list; with `take` (Enter, F4) the highlighted command goes into the field.
    pub fn close_list(&mut self, take: bool) {
        if let (true, Some(Shown { selected: Some(i), .. })) = (take, self.list) {
            self.show_command(i);
        }
        self.list = None;
    }

    /// Puts earlier command `i` into the field, the caret at its end.
    fn show_command(&mut self, i: usize) {
        if let Some(command) = self.history.get(i) {
            self.dialog.text = command.clone();
            self.dialog.caret = self.dialog.text.chars().count();
            self.dialog.caret_on = true;
            self.history_at = Some(i);
        }
    }

    /// Up or Down with Run's list closed: the next earlier command (the newest first).
    fn step_history(&mut self, down: bool) {
        if self.history.is_empty() {
            return;
        }
        let last = self.history.len() - 1;
        let at = match (self.history_at, down) {
            (None, _) => 0,
            (Some(i), true) => (i + 1).min(last),
            (Some(i), false) => i.saturating_sub(1),
        };
        self.show_command(at);
    }

    /// A key while Run's list is open: the arrows move the highlight and show its command,
    /// Enter and F4 close the list taking it, Esc closes it as it is. Other keys edit the field.
    fn list_key(&mut self, key: Key) -> bool {
        let (Some(list), Some(mut shown)) = (self.run_list(), self.list) else { return false };
        let total = self.history.len();
        let page = list.rows as i32 - 1;
        let delta = match key {
            Key::Enter | Key::F4 => {
                self.close_list(true);
                return true;
            }
            Key::Escape => {
                self.close_list(false);
                return true;
            }
            Key::Up => -1,
            Key::Down => 1,
            Key::PageUp => -page.max(1),
            Key::PageDown => page.max(1),
            _ => return false,
        };
        if let Some(i) = shown.step(&list, total, delta) {
            self.list = Some(shown);
            self.show_command(i);
        }
        true
    }

    /// A press while Run's list is open: a row takes its command and closes the list, the
    /// scroll bar scrolls it, the drop-down button closes it (and shows pushed in while held);
    /// anywhere else the press only closes the list, as Windows' list took that click.
    fn press_in_list(&mut self, mut shown: Shown, part: Option<Part>) -> Press<Part> {
        let Some(list) = self.run_list() else { return Press::Done };
        let total = self.history.len();
        match part {
            Some(Part::List(ListPart::Row(i))) => {
                self.show_command(i);
                self.list = None;
            }
            Some(p @ Part::List(ListPart::Up | ListPart::Down)) => {
                shown.scroll(&list, total, if p == Part::List(ListPart::Up) { -1 } else { 1 });
                self.list = Some(shown);
                return Press::Hold(p);
            }
            Some(Part::List(ListPart::PageUp)) => {
                shown.scroll(&list, total, -(list.rows as i32));
                self.list = Some(shown);
            }
            Some(Part::List(ListPart::PageDown)) => {
                shown.scroll(&list, total, list.rows as i32);
                self.list = Some(shown);
            }
            Some(p @ Part::List(ListPart::Thumb)) => return Press::Grab(p, shown.top as i32),
            Some(Part::List(ListPart::Frame)) => {}
            Some(Part::DropDown) => {
                self.list = None;
                self.dialog.pressed = Some(Part::DropDown);
                return Press::Grab(Part::DropDown, 0);
            }
            _ => self.list = None,
        }
        Press::Done
    }

    /// The open list highlights the row under the pointer (the field keeps its text). True if
    /// that changed what it shows.
    fn hover_row(&mut self, x: i32, y: i32) -> bool {
        let (Some(list), Some(shown)) = (self.run_list(), self.list.as_mut()) else { return false };
        match list.hit(self.history.len(), shown.top, x, y) {
            Some(ListPart::Row(i)) if shown.selected != Some(i) => {
                shown.selected = Some(i);
                true
            }
            _ => false,
        }
    }

    /// Steps the chosen Shut Down entry (and the highlighted one of the open list).
    fn step_choice(&mut self, forward: bool) {
        let n = shut_down_choices(self.dialog.era).len();
        let d = &mut self.dialog;
        let current = if d.list_open { d.list_hover } else { d.choice };
        let next = if forward { (current + 1).min(n - 1) } else { current.saturating_sub(1) };
        if d.list_open {
            d.list_hover = next;
        } else {
            d.choice = next;
            if let Part::Option(_) = d.focus {
                d.focus = Part::Option(next);
            }
        }
    }
}

fn byte_at(text: &str, chars: usize) -> usize {
    text.char_indices().nth(chars).map_or(text.len(), |(i, _)| i)
}

impl kit::Form for DialogWindow {
    type Part = Part;

    fn title(&self) -> &str {
        self.layout.title
    }
    fn size(&self) -> (i32, i32) {
        (self.layout.width, self.layout.height)
    }
    fn layout(&mut self, fonts: &Fonts) {
        self.layout = layout(&self.dialog);
        let inset = if self.dialog.era == Era::Win2000 { 6 } else { 3 };
        self.field_x = self.layout.rect(Part::Command).map_or(0, |r| r.x + inset);
        self.advances = self.dialog.text.chars().map(|c| fonts.regular.glyph(c).advance).collect();
    }
    fn paint(&self, fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts) {
        paint_dialog(fb, theme, fonts, &self.dialog, &self.layout);
    }

    fn set_screen(&mut self, x: i32, y: i32) {
        self.screen = (x, y);
    }
    fn drop_rect(&self) -> Option<Rect> {
        self.list.and(self.run_list()).map(|l| l.frame)
    }
    /// Run's open list, its frame at the picture's top left corner.
    fn paint_drop(&self, fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts) {
        let (Some(list), Some(shown)) = (self.run_list(), self.list) else { return };
        let at = DropList { frame: Rect::new(0, 0, list.frame.w, list.frame.h), rows: list.rows };
        let labels: Vec<&str> = self.history.iter().map(String::as_str).collect();
        let held = |dir: i32| {
            self.dialog.pressed == Some(Part::List(if dir < 0 { ListPart::Up } else { ListPart::Down }))
        };
        at.paint(fb, &theme.colors, fonts.regular, &labels, (shown.top, shown.selected), None, held);
    }

    fn hit(&self, x: i32, y: i32) -> Option<Part> {
        if let (Some(list), Some(shown)) = (self.run_list(), self.list)
            && let Some(part) = list.hit(self.history.len(), shown.top, x, y)
        {
            return Some(Part::List(part));
        }
        self.layout.hit(&self.dialog, x, y)
    }
    fn rect(&self, part: Part) -> Option<Rect> {
        match (part, self.run_list(), self.list) {
            (Part::List(p), Some(list), Some(shown)) => list.part_rect(self.history.len(), shown.top, p),
            _ => self.layout.rect(part),
        }
    }
    fn enabled(&self, part: Part) -> bool {
        self.dialog.enabled(part)
    }
    fn focus(&self) -> Part {
        self.dialog.focus
    }
    fn set_focus(&mut self, part: Part) {
        self.dialog.focus = part;
        self.dialog.list_open = false;
        self.list = None;
    }
    /// The title bar buttons and Run's drop-down are not on the Tab path; Windows 95's options
    /// count as one stop.
    fn tab_stops(&self) -> Vec<Part> {
        let mut stops: Vec<Part> = Vec::new();
        for &(part, _) in &self.layout.parts {
            let stop = match part {
                Part::TitleHelp | Part::TitleClose | Part::DropDown => continue,
                Part::Option(_) => Part::Option(self.dialog.choice),
                p => p,
            };
            if !stops.contains(&stop) && self.dialog.enabled(stop) {
                stops.push(stop);
            }
        }
        stops
    }
    fn set_pressed(&mut self, part: Option<Part>) {
        self.dialog.pressed = part;
    }
    fn show_keys(&mut self) {
        self.dialog.show_keys = true;
    }
    fn enter(&self) -> Part {
        self.dialog.default_button()
    }
    fn escape(&self) -> Part {
        if self.dialog.kind == DialogKind::Run { Part::Cancel } else { Part::TitleClose }
    }

    fn key(&mut self, key: Key, mods: Mods) -> Option<Vec<Effect<Part>>> {
        let command = self.dialog.focus == Part::Command;
        if command && self.list_key(key) {
            return Some(Vec::new());
        }
        match key {
            Key::F4 if command => self.open_list(),
            Key::Enter if self.dialog.list_open => {
                self.dialog.choice = self.dialog.list_hover;
                self.dialog.list_open = false;
            }
            Key::Escape if self.dialog.list_open => self.dialog.list_open = false,
            Key::F4 if self.dialog.focus == Part::Choice => {
                self.dialog.list_open = !self.dialog.list_open;
                self.dialog.list_hover = self.dialog.choice;
            }
            Key::Up | Key::Down | Key::Left | Key::Right if matches!(self.dialog.focus, Part::Option(_) | Part::Choice) => {
                self.step_choice(matches!(key, Key::Down | Key::Right));
            }
            Key::Up | Key::Down if command => self.step_history(key == Key::Down),
            Key::Left if command => self.edit(|_, c| *c = c.saturating_sub(1)),
            Key::Right if command => self.edit(|_, c| *c += 1),
            Key::Home if command => self.edit(|_, c| *c = 0),
            Key::End if command => self.edit(|t, c| *c = t.chars().count()),
            Key::Delete if command => self.edit(|t, c| {
                if *c < t.chars().count() {
                    let at = byte_at(t, *c);
                    t.remove(at);
                }
            }),
            Key::Other(vk) if command && mods.ctrl && vk == u16::from(b'V') => return Some(vec![Effect::Paste]),
            _ => return None,
        }
        Some(Vec::new())
    }
    /// Alt+Down and Alt+Up open Run's list, and close it (taking the highlighted command).
    fn alt_key(&mut self, key: Key) -> bool {
        if self.dialog.focus != Part::Command || !matches!(key, Key::Up | Key::Down) {
            return false;
        }
        if self.list.is_some() { self.close_list(true) } else { self.open_list() }
        true
    }
    fn typed(&mut self, ch: char) -> Vec<Effect<Part>> {
        if self.dialog.focus != Part::Command {
            return Vec::new();
        }
        match ch {
            '\u{8}' => self.edit(|t, c| {
                if *c > 0 {
                    *c -= 1;
                    let at = byte_at(t, *c);
                    t.remove(at);
                }
            }),
            c if !c.is_control() => self.edit(|t, at| {
                t.insert(byte_at(t, *at), c);
                *at += 1;
            }),
            _ => {}
        }
        Vec::new()
    }
    fn access(&mut self, ch: char) -> Vec<Effect<Part>> {
        let ch = ch.to_ascii_lowercase();
        let part = self.layout.parts.iter().map(|&(p, _)| p).find(|&p| access_key(&self.dialog, p) == Some(ch));
        match part {
            Some(Part::Option(i)) => {
                self.dialog.choice = i;
                self.dialog.focus = Part::Option(i);
            }
            Some(p @ (Part::Command | Part::Choice)) => self.dialog.focus = p,
            Some(p) => return kit::activate(self, p),
            None => {}
        }
        Vec::new()
    }
    fn space(&mut self) -> Vec<Effect<Part>> {
        if self.dialog.focus == Part::Command { Vec::new() } else { kit::activate(self, self.dialog.focus) }
    }
    fn press(&mut self, part: Option<Part>, x: i32, y: i32) -> Press<Part> {
        if let Some(shown) = self.list {
            return self.press_in_list(shown, part);
        }
        if part == Some(Part::DropDown) {
            // The list opens as the button goes down; the button shows pushed in while held.
            self.dialog.focus = Part::Command;
            self.open_list();
            self.dialog.pressed = Some(Part::DropDown);
            return Press::Grab(Part::DropDown, 0);
        }
        let d = &mut self.dialog;
        match part {
            Some(Part::Choice) if d.list_open => {
                if let Some(i) = self.layout.list_entry(d, x, y) {
                    d.choice = i;
                    d.list_open = false;
                }
            }
            Some(Part::Choice) => {
                d.focus = Part::Choice;
                d.list_open = true;
                d.list_hover = d.choice;
            }
            Some(Part::Option(i)) => {
                d.choice = i;
                d.focus = Part::Option(i);
            }
            Some(Part::Command) => {
                // The caret goes between the characters nearest the click.
                let mut w = self.field_x;
                let caret = self
                    .advances
                    .iter()
                    .take_while(|&&a| {
                        let before = w + a / 2 < x;
                        w += a;
                        before
                    })
                    .count();
                d.focus = Part::Command;
                d.caret = caret;
                d.caret_on = true;
            }
            Some(p) if d.enabled(p) => {
                d.list_open = false;
                if !matches!(p, Part::TitleHelp | Part::TitleClose | Part::DropDown) {
                    d.focus = p;
                }
                return Press::Hold(p);
            }
            Some(_) => {}
            None => {
                d.list_open = false;
                return Press::Nothing;
            }
        }
        Press::Done
    }
    /// The list's arrows and the drop-down button only act as they go down.
    fn release(&mut self, part: Part) -> Vec<Effect<Part>> {
        match part {
            Part::List(_) | Part::DropDown => Vec::new(),
            p => vec![Effect::Activate(p)],
        }
    }
    /// Held from the drop-down button: the button shows pushed in while the pointer is on it,
    /// and the open list highlights the row under it. A grabbed thumb scrolls the list.
    fn drag(&mut self, part: Part, grab_y: i32, value: i32, x: i32, y: i32) -> bool {
        let before = (self.dialog.pressed, self.list);
        match part {
            Part::DropDown => {
                let on = self.layout.rect(Part::DropDown).is_some_and(|r| r.contains(x, y));
                self.dialog.pressed = on.then_some(Part::DropDown);
                self.hover_row(x, y);
            }
            Part::List(ListPart::Thumb) => {
                if let (Some(list), Some(shown)) = (self.run_list(), self.list.as_mut()) {
                    shown.top = list.scroll_bar(self.history.len()).dragged(value, y - grab_y);
                }
            }
            _ => {}
        }
        before != (self.dialog.pressed, self.list)
    }
    /// Let go after a press on the drop-down button: on a row of the list, that command is
    /// taken and the list closes (Windows' press, drag and let go); elsewhere the list stays.
    fn dropped(&mut self, part: Part, x: i32, y: i32) -> Vec<Effect<Part>> {
        if part == Part::DropDown {
            self.dialog.pressed = None;
            if let Some(Part::List(ListPart::Row(i))) = self.hit(x, y) {
                self.show_command(i);
                self.list = None;
            }
        }
        Vec::new()
    }
    /// Run's open list scrolls (Windows 95 itself had no wheel: Bevel95 scrolls there too, as
    /// its Time Zone list does). With the list closed, Windows 2000 steps through the commands.
    fn wheel(&mut self, notches: i32) {
        if let (Some(list), Some(shown)) = (self.run_list(), self.list.as_mut()) {
            shown.scroll(&list, self.history.len(), -notches * WHEEL_ROWS);
        } else if self.dialog.kind == DialogKind::Run
            && self.dialog.focus == Part::Command
            && self.dialog.era == Era::Win2000
        {
            for _ in 0..notches.unsigned_abs() {
                self.step_history(notches < 0);
            }
        }
    }
    /// Another window came to the front: Run's list closes, as it is.
    fn set_active(&mut self, active: bool) {
        if !active {
            self.list = None;
        }
    }
    fn hover(&mut self, x: i32, y: i32) -> bool {
        if self.list.is_some() {
            return self.hover_row(x, y);
        }
        let d = &mut self.dialog;
        match self.layout.list_entry(d, x, y) {
            Some(i) if d.list_open && i != d.list_hover => {
                d.list_hover = i;
                true
            }
            _ => false,
        }
    }
    fn paste(&mut self, text: &str) {
        let paste: String = text.chars().filter(|c| !c.is_control()).collect();
        self.edit(|t, c| {
            t.insert_str(byte_at(t, *c), &paste);
            *c += paste.chars().count();
        });
    }
    fn has_caret(&self) -> bool {
        self.dialog.focus == Part::Command
    }
    fn blink(&mut self, on: bool) {
        self.dialog.caret_on = on;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::{Controller, Form};

    #[test]
    fn run_types_tabs_and_pastes_through_the_controller() {
        let mut w = DialogWindow::new(Dialog::new(DialogKind::Run, Era::Win98), vec!["calc".into(), "notepad".into()]);
        let mut c = Controller::default();
        for ch in "note".chars() {
            w.typed(ch);
        }
        assert_eq!((w.dialog.text.as_str(), w.dialog.caret), ("note", 4));
        c.key(&mut w, Key::Left, Mods::default());
        w.typed('\u{8}');
        assert_eq!((w.dialog.text.as_str(), w.dialog.caret), ("noe", 2));
        assert_eq!(c.key(&mut w, Key::Other(u16::from(b'V')), Mods { ctrl: true, shift: false }), vec![Effect::Paste]);
        w.paste("t\npad");
        assert_eq!(w.dialog.text, "notpade");
        // Up shows the newest command; Tab goes on to OK, then Cancel, Browse...
        c.key(&mut w, Key::Up, Mods::default());
        assert_eq!(w.dialog.text, "calc");
        c.key(&mut w, Key::Tab, Mods::default());
        assert_eq!(w.dialog.focus, Part::Ok);
        assert_eq!(c.key(&mut w, Key::Enter, Mods::default()), vec![Effect::Activate(Part::Ok)]);
        assert_eq!(c.key(&mut w, Key::Escape, Mods::default()), vec![Effect::Activate(Part::Cancel)]);
    }

    fn run_with(era: Era, n: usize) -> DialogWindow {
        let history: Vec<String> = (0..n).map(|i| format!("command {i}")).collect();
        DialogWindow::new(Dialog::new(DialogKind::Run, era), history)
    }

    #[test]
    fn run_list_opens_by_the_button_and_closes_by_a_second_press() {
        let mut w = run_with(Era::Win98, 15);
        let mut c = Controller::default();
        let button = w.rect(Part::DropDown).unwrap();
        let (bx, by) = (button.x + 8, button.y + 8);
        assert!(w.list.is_none());
        assert_eq!(c.mouse_down(&mut w, bx, by), vec![Effect::Capture]);
        assert_eq!(w.list, Some(Shown { top: 0, selected: None }), "opens as the button goes down");
        assert_eq!(w.dialog.pressed, Some(Part::DropDown), "the button shows pushed in while held");
        assert_eq!(c.mouse_up(&mut w, bx, by), vec![Effect::Release]);
        assert!(w.list.is_some() && w.dialog.pressed.is_none(), "stays open, the button raised");
        // 15 commands in 10 rows: under the field, with a scroll bar.
        let list = w.run_list().unwrap();
        assert_eq!(list.frame, Rect::new(57, 103, 275, 132));
        assert_eq!(w.drop_rect(), Some(list.frame));
        c.mouse_down(&mut w, bx, by);
        c.mouse_up(&mut w, bx, by);
        assert!(w.list.is_none() && w.drop_rect().is_none(), "a second press closes it");
        assert_eq!(w.dialog.text, "", "and leaves the field as it was");
    }

    #[test]
    fn run_list_keys_open_move_take_and_close() {
        let mut w = run_with(Era::Win2000, 30);
        let mut c = Controller::default();
        for ch in "command 2".chars() {
            w.typed(ch);
        }
        // F4 opens on the first command that starts with the text: "command 2" (not 20..29,
        // which come later), the field unchanged.
        c.key(&mut w, Key::F4, Mods::default());
        assert_eq!(w.list, Some(Shown { top: 2, selected: Some(2) }));
        assert_eq!(w.run_list().unwrap().rows, 23, "Windows 2000 shows up to 23 rows");
        c.key(&mut w, Key::Down, Mods::default());
        assert_eq!((w.dialog.text.as_str(), w.list.unwrap().selected), ("command 3", Some(3)), "the field follows");
        assert_eq!(c.key(&mut w, Key::Enter, Mods::default()), Vec::new(), "Enter only closes the list");
        assert!(w.list.is_none());
        assert_eq!(w.dialog.text, "command 3");
        // Alt+Down opens, Esc closes the list only; then Esc is Cancel again.
        assert_eq!(c.alt_key(&mut w, Key::Down), (true, Vec::new()));
        assert!(w.list.is_some());
        assert_eq!(c.key(&mut w, Key::Escape, Mods::default()), Vec::new());
        assert!(w.list.is_none());
        assert_eq!(c.key(&mut w, Key::Escape, Mods::default()), vec![Effect::Activate(Part::Cancel)]);
        // With the list closed, Down steps on from the command shown.
        c.key(&mut w, Key::Down, Mods::default());
        assert_eq!(w.dialog.text, "command 4");
        // Down to the end keeps the highlight in view.
        c.key(&mut w, Key::F4, Mods::default());
        for _ in 0..40 {
            c.key(&mut w, Key::Down, Mods::default());
        }
        assert_eq!(w.list, Some(Shown { top: 7, selected: Some(29) }));
        assert_eq!(c.key(&mut w, Key::F4, Mods::default()), Vec::new());
        assert_eq!((w.list, w.dialog.text.as_str()), (None, "command 29"));
    }

    #[test]
    fn run_list_rows_are_chosen_with_the_mouse() {
        let mut w = run_with(Era::Win95, 12);
        let mut c = Controller::default();
        w.dialog.text = "zz".into();
        c.key(&mut w, Key::F4, Mods::default());
        assert_eq!(w.list, Some(Shown { top: 0, selected: None }), "nothing starts with the text");
        let list = w.run_list().unwrap();
        // The pointer over a row highlights it; the field keeps its text.
        let row = list.row_rect(3);
        assert!(c.mouse_move(&mut w, row.x + 5, row.y + 5));
        assert_eq!((w.list.unwrap().selected, w.dialog.text.as_str()), (Some(3), "zz"));
        // The wheel scrolls (as far as the list goes); the highlight stays.
        w.wheel(-1);
        assert_eq!(w.list, Some(Shown { top: 2, selected: Some(3) }));
        // The scroll bar's lower arrow, held, shows pushed in and scrolls no further.
        let down = w.rect(Part::List(ListPart::Down)).unwrap();
        c.mouse_down(&mut w, down.x + 4, down.y + 4);
        assert_eq!(w.dialog.pressed, Some(Part::List(ListPart::Down)));
        assert_eq!(c.mouse_up(&mut w, down.x + 4, down.y + 4), vec![Effect::Release], "nothing to carry out");
        // A click on a row takes its command and closes the list.
        let row = list.row_rect(0);
        c.mouse_down(&mut w, row.x + 5, row.y + 5);
        c.mouse_up(&mut w, row.x + 5, row.y + 5);
        assert_eq!((w.list, w.dialog.text.as_str()), (None, "command 2"));
        // A click elsewhere in the Run box only closes the list: Cancel is not carried out.
        c.key(&mut w, Key::F4, Mods::default());
        let cancel = w.rect(Part::Cancel).unwrap();
        assert_eq!(c.mouse_down(&mut w, cancel.x + 5, cancel.y + 5), Vec::new());
        assert_eq!(c.mouse_up(&mut w, cancel.x + 5, cancel.y + 5), Vec::new());
        assert!(w.list.is_none());
    }

    #[test]
    fn run_list_follows_typing_and_opens_above_near_the_screen_bottom() {
        let mut w = run_with(Era::Win98, 15);
        let mut c = Controller::default();
        c.key(&mut w, Key::F4, Mods::default());
        for ch in "command 9".chars() {
            w.typed(ch);
        }
        assert_eq!(w.list, Some(Shown { top: 5, selected: None }), "scrolled to the match, nothing highlighted");
        w.typed('x');
        assert_eq!(w.list, Some(Shown { top: 0, selected: None }), "no match: back to the first command");
        // Dragging from the button onto a row and letting go there takes it.
        c.key(&mut w, Key::Escape, Mods::default());
        let button = w.rect(Part::DropDown).unwrap();
        c.mouse_down(&mut w, button.x + 8, button.y + 8);
        let row = w.run_list().unwrap().row_rect(1);
        c.mouse_move(&mut w, row.x + 9, row.y + 4);
        assert_eq!((w.dialog.pressed, w.list.unwrap().selected), (None, Some(1)));
        c.mouse_up(&mut w, row.x + 9, row.y + 4);
        assert_eq!((w.list, w.dialog.text.as_str()), (None, "command 1"));
        // The Run box 405px down an 800×600 screen (as the PCs placed it): the 132px list
        // would end at 640, so it opens above the field, its bottom on the field's top.
        w.screen = (5, 405);
        w.screen_bottom = 600;
        assert_eq!(w.run_list().unwrap().frame, Rect::new(57, -50, 275, 132));
        // Fewer commands, fewer rows; the same list fits under the field higher up.
        w.history.truncate(7);
        w.screen = (5, 342);
        assert_eq!(w.run_list().unwrap().frame, Rect::new(57, 103, 275, 93));
        // Losing the front closes the list.
        w.open_list();
        w.set_active(false);
        assert!(w.list.is_none());
    }

    #[test]
    fn shut_down_options_follow_the_arrows_and_access_keys() {
        let mut w = DialogWindow::new(Dialog::new(DialogKind::ShutDown, Era::Win95), Vec::new());
        let mut c = Controller::default();
        c.key(&mut w, Key::Down, Mods::default());
        assert_eq!((w.dialog.choice, w.dialog.focus), (1, Part::Option(1)));
        c.access(&mut w, 'm');
        assert_eq!(w.dialog.choice, 2, "Restart the computer in &MS-DOS mode?");
        assert_eq!(c.key(&mut w, Key::Enter, Mods::default()), vec![Effect::Activate(Part::Yes)]);
        assert_eq!(c.key(&mut w, Key::Escape, Mods::default()), vec![Effect::Activate(Part::TitleClose)]);
    }

    #[test]
    fn banner_asset_has_the_right_size() {
        assert_eq!(BANNER_2000.len() as i32, BANNER_2000_SIZE.0 * BANNER_2000_SIZE.1 * 3);
        assert_eq!(BANNER_2000_16.len(), BANNER_2000.len());
    }

    #[test]
    fn run_needs_a_command_before_ok_works() {
        let mut d = Dialog::new(DialogKind::Run, Era::Win95);
        assert!(!d.enabled(Part::Ok));
        d.text = "notepad".into();
        assert!(d.enabled(Part::Ok));
        assert_eq!(d.default_button(), Part::Ok);
    }

    #[test]
    fn layouts_have_their_title_buttons() {
        for era in Era::ALL {
            let run = layout(&Dialog::new(DialogKind::Run, era));
            assert_eq!(run.rect(Part::TitleClose), Some(Rect::new(326, 5, 16, 14)));
            assert_eq!(run.rect(Part::TitleHelp), Some(Rect::new(308, 5, 16, 14)));
            let shut = layout(&Dialog::new(DialogKind::ShutDown, era));
            assert!(shut.rect(Part::TitleHelp).is_none());
        }
    }
}
