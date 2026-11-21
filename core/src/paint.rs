//! Painting the bar and the Start menu into a framebuffer at 1×.

use crate::colors16;
use crate::fb::{Framebuffer, Icon, Rect};
use crate::flag::start_flag;
use crate::layout::{self, BarLayout};
use crate::quick_launch;
use crate::text::{Font, Fonts, split_access_key};
use crate::theme::{Arrow, Colors, IconItems, Separator, Theme, rgb};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskState {
    Normal,
    /// The foreground app: drawn sunken, checkered, bold.
    Active,
    /// Wants attention: drawn in the highlight color.
    Flashing,
}

/// One task button (one app group).
#[derive(Clone, Debug)]
pub struct Task {
    pub title: String,
    pub icon: Option<Icon>,
    pub state: TaskState,
}

/// One of the two task-list scroll arrows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollArrow {
    Up,
    Down,
}

/// Everything the bar shows.
#[derive(Clone, Debug, Default)]
pub struct Bar {
    pub start_pressed: bool,
    pub tasks: Vec<Task>,
    /// The notification icons, left to right (`None`: a program gave no picture; its slot
    /// stays empty, as in Windows).
    pub tray: Vec<Option<Icon>>,
    /// The battery's percentage (Bevel95's own option, "87%"), shown right of the icon it
    /// belongs to (an index into `tray`) in the clock's font and color; `None` or an empty text:
    /// no number and no part for it.
    pub tray_number: Option<(usize, String)>,
    pub clock: String,
    /// The widest time of day in the same format ("12:00 AM"): the clock's part of the well
    /// is made that wide, so it does not change width as the time does.
    pub clock_widest: String,
    /// The row of task buttons shown when they need several (0 = the first).
    pub row: usize,
    /// A scroll arrow held down with the mouse.
    pub scroll_pressed: Option<ScrollArrow>,
    /// While task buttons are dragged or slide (`tasks::Motion`): where each button of the
    /// row shown is drawn (its left), in the order of the row; empty, or missing for a button,
    /// means at its place.
    pub task_x: Vec<i32>,
    /// The button of the row shown that is being dragged: drawn last, over the others.
    pub task_on_top: Option<usize>,
    /// The Quick Launch band, while it is switched on.
    pub quick_launch: Option<quick_launch::Band>,
}

/// Where everything of `bar` goes on a bar `width` wide, as `paint_bar` lays it out.
pub fn bar_layout(theme: &Theme, fonts: &Fonts, width: i32, bar: &Bar) -> BarLayout {
    let number = bar.tray_number.as_ref().filter(|(_, text)| !text.is_empty()).map(|&(i, _)| i);
    let quick = bar.quick_launch.as_ref().map(quick_launch::Band::shape);
    layout::bar_with(theme, fonts, width, bar.tasks.len(), bar.tray.len(), number, &bar.clock, &bar.clock_widest, bar.row, quick)
}

/// Paints the whole bar (the framebuffer should be `theme.bar.height` tall) and returns
/// where everything went, for hit-testing.
pub fn paint_bar(fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts, bar: &Bar) -> BarLayout {
    let c = &theme.colors;
    fb.fill(Rect::new(0, 0, fb.width, fb.height), c.face);
    fb.hline(0, 0, fb.width, c.light);
    fb.hline(0, 1, fb.width, c.highlight);

    let layout = bar_layout(theme, fonts, fb.width, bar);
    paint_start(fb, theme, fonts, layout.start, bar.start_pressed);
    if let (Some(band), Some(q)) = (&layout.quick, &bar.quick_launch) {
        paint_quick_launch(fb, theme, band, q);
    }
    if let Some(t) = &layout.task_band {
        let (y, h) = (theme.bar.item_y(), theme.bar.item_h());
        etched_line(fb, &theme.colors, t.line, y, h);
        grip(fb, &theme.colors, t.grip, y);
        if let Some(x) = t.tray_line {
            etched_line(fb, &theme.colors, x, y, h);
        }
    }
    // Pressed buttons share one checkerboard, lined up with the first button's (Windows lines
    // the pattern up with its task window): measured on the reference PCs, where a pressed
    // button an odd number of pixels further right has the pattern the other way round. A
    // button drawn away from its place takes its pattern with it.
    let checker_x = layout.tasks.first().map_or(0, |r| r.x + 2);
    let shown: Vec<(usize, &Task, Rect)> = bar
        .tasks
        .iter()
        .skip(layout.first_task)
        .zip(&layout.tasks)
        .enumerate()
        .map(|(i, (task, &r))| (i, task, Rect { x: bar.task_x.get(i).copied().unwrap_or(r.x), ..r }))
        .collect();
    let on_top = |i: usize| bar.task_on_top == Some(i);
    for &(i, task, r) in shown.iter().filter(|s| !on_top(s.0)).chain(shown.iter().filter(|s| on_top(s.0))) {
        paint_task(fb, theme, fonts, r, task, checker_x + r.x - layout.tasks[i].x);
    }
    if let Some(s) = layout.scroll {
        scroll_button(fb, c, s.up, ScrollArrow::Up, bar.scroll_pressed == Some(ScrollArrow::Up));
        scroll_button(fb, c, s.down, ScrollArrow::Down, bar.scroll_pressed == Some(ScrollArrow::Down));
    }
    paint_tray(fb, theme, fonts, &layout, bar);
    layout
}

/// The Quick Launch band (the Reference PCs' Windows 98 and 2000, docs/theme-spec.md "Quick
/// Launch"): etched lines (shadow, then highlight) as tall as the buttons, grips (a 3×18 raised
/// box: highlight top and left, shadow bottom and right), flat buttons whose icon sits 3 in; under
/// the pointer a button rises (a 1px highlight and shadow edge), held down it sinks (the edge the
/// other way round) and its icon moves 1px down and right.
fn paint_quick_launch(fb: &mut Framebuffer, theme: &Theme, band: &quick_launch::Layout, q: &quick_launch::Band) {
    let c = &theme.colors;
    let (y, h) = (band.rect.y, band.rect.h);
    let etched = |fb: &mut Framebuffer, x: i32| etched_line(fb, c, x, y, h);
    let grip = |fb: &mut Framebuffer, x: i32| grip(fb, c, x, y);
    etched(fb, band.line);
    grip(fb, band.grip);
    if band.left.is_none() && band.right.is_none() {
        // Every icon drawn whole where it is (also Windows 2000's with its "»").
        for (i, r) in band.buttons.iter().enumerate() {
            quick_button(fb, theme, *r, q, i);
        }
    } else {
        // Windows 98's pager: the icons drawn on a strip of their own and only the part that can be
        // seen copied to the bar, so the icon at an edge is cut (resize.md, 2).
        let (x0, x1) = band.view;
        if x1 > x0 {
            let mut strip = Framebuffer::new(x1 - x0, fb.height, c.face);
            for (i, r) in band.buttons.iter().enumerate() {
                if r.right() > x0 && r.x < x1 {
                    quick_button(&mut strip, theme, Rect { x: r.x - x0, ..*r }, q, i);
                }
            }
            fb.blit_part(&strip, Rect::new(0, y, x1 - x0, h), x0, y);
        }
    }
    let control = |c: quick_launch::Control| {
        let pressed = q.pressed_control == Some(c);
        let state = if pressed || (c == quick_launch::Control::Chevron && q.list_open) {
            ControlState::Pressed
        } else if q.hot_control == Some(c) {
            ControlState::Hot
        } else {
            ControlState::Flat
        };
        // A 98 arrow held down once the end is reached: grayed until the release (resize.md,
        // `12icons-left-arrow-held-down-b`).
        let at_end = match c {
            quick_launch::Control::Left => band.scroll == 0,
            quick_launch::Control::Right => band.scroll >= band.max_scroll,
            quick_launch::Control::Chevron => false,
        };
        if pressed && at_end { ControlState::Grayed } else { state }
    };
    if let Some(r) = band.left {
        overflow_control(fb, theme, r, quick_launch::Control::Left, control(quick_launch::Control::Left));
    }
    if let Some(r) = band.right {
        overflow_control(fb, theme, r, quick_launch::Control::Right, control(quick_launch::Control::Right));
    }
    if let Some(r) = band.chevron {
        overflow_control(fb, theme, r, quick_launch::Control::Chevron, control(quick_launch::Control::Chevron));
    }
    etched(fb, band.end_line);
    grip(fb, band.end_grip);
    if let Some(x) = band.tray_line {
        etched(fb, x);
    }
}

/// Icon `i`'s button at `r`: flat, raised under the pointer, sunk while held down (its picture 1
/// pixel right and down).
fn quick_button(fb: &mut Framebuffer, theme: &Theme, r: Rect, q: &quick_launch::Band, i: usize) {
    let c = &theme.colors;
    let pressed = q.pressed == Some(i);
    let shift = if pressed {
        fb.frame(r, c.shadow, c.highlight);
        1
    } else {
        if q.hot == Some(i) {
            fb.frame(r, c.highlight, c.shadow);
        }
        0
    };
    if let Some(Some(icon)) = q.icons.get(i) {
        let at = quick_launch::ICON_IN + shift;
        colors16::draw_picture(fb, theme, icon, r.x + at, r.y + at);
    }
}

/// How an overflow control is drawn.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ControlState {
    Flat,
    Hot,
    Pressed,
    Grayed,
}

/// An overflow control of the band (resize.md, 2): flat when idle (only its glyph); under the
/// pointer a one-pixel ring, highlight top and left, shadow right and bottom; held down Windows
/// 98's arrow has a dark-shadow and light ring (black and DFDFDF), Windows 2000's "»" (also while
/// its list is open) a shadow and highlight one, and the glyph moves 1 pixel right and down; a 98
/// arrow held down at the end is grayed (etched, no ring).
fn overflow_control(fb: &mut Framebuffer, theme: &Theme, r: Rect, which: quick_launch::Control, state: ControlState) {
    use crate::art::{Look, rows};
    use quick_launch::Control;
    const RIGHT: &[&str] = rows("quick-launch/pager-right", Look::Win98).rows;
    const LEFT: &[&str] = rows("quick-launch/pager-left", Look::Win98).rows;
    const CHEVRON: &[&str] = rows("quick-launch/chevron", Look::Win2000).rows;
    let c = &theme.colors;
    let (shape, dx, dy): (&[&str], i32, i32) = match which {
        Control::Right => (RIGHT, 5, 8),
        Control::Left => (LEFT, 4, 8),
        Control::Chevron => (CHEVRON, 2, 5),
    };
    let (x, y) = (r.x + dx, r.y + dy);
    match state {
        ControlState::Flat => art(fb, shape, x, y, c.text),
        ControlState::Hot => {
            fb.frame(r, c.highlight, c.shadow);
            art(fb, shape, x, y, c.text);
        }
        ControlState::Pressed => {
            if which == Control::Chevron {
                fb.frame(r, c.shadow, c.highlight);
            } else {
                fb.frame(r, c.dark_shadow, c.light);
            }
            art(fb, shape, x + 1, y + 1, c.text);
        }
        ControlState::Grayed => {
            art(fb, shape, x + 1, y + 1, c.etch);
            art(fb, shape, x, y, c.shadow);
        }
    }
}

/// An etched line on the bar (98's and 2000's bands): a shadow column, then a highlight one, as
/// tall as the row of buttons (`y`, `h`).
fn etched_line(fb: &mut Framebuffer, c: &Colors, x: i32, y: i32, h: i32) {
    fb.vline(x, y, h, c.shadow);
    fb.vline(x + 1, y, h, c.highlight);
}

/// A band's grip: a 3×18 raised box 2 rows below the row's top `y`.
fn grip(fb: &mut Framebuffer, c: &Colors, x: i32, y: i32) {
    fb.frame(Rect::new(x, y + quick_launch::GRIP_DY, quick_launch::GRIP_W, quick_launch::GRIP_H), c.highlight, c.shadow);
}

/// The arrows of a 16×11 scroll button, as (x, y) from its top-left corner: Windows 98's
/// taskbar in the v86 emulator, the same as Windows' DrawFrameControl still draws them.
const SCROLL_UP: [(i32, i32); 4] = [(7, 4), (6, 5), (7, 5), (8, 5)];
const SCROLL_DOWN: [(i32, i32); 4] = [(6, 5), (7, 5), (8, 5), (7, 6)];

/// A scroll arrow button, the way Windows' DrawFrameControl draws one: an outer light and
/// dark-shadow edge with a white and shadow edge inside (unlike a push button, whose white
/// edge is outside). Pushed in (as DrawFrameControl draws it; not seen on Windows 98 itself)
/// the edges turn sunken, shadow and white outside, dark shadow and light inside, and the
/// arrow moves 1px down-right.
fn scroll_button(fb: &mut Framebuffer, c: &Colors, r: Rect, dir: ScrollArrow, pressed: bool) {
    fb.fill(r, c.face);
    if pressed {
        fb.frame(r, c.shadow, c.highlight);
        fb.frame(r.inset(1, 1, 1, 1), c.dark_shadow, c.light);
    } else {
        fb.frame(r, c.light, c.dark_shadow);
        fb.frame(r.inset(1, 1, 1, 1), c.highlight, c.shadow);
    }
    let shift = i32::from(pressed);
    let arrow = if dir == ScrollArrow::Up { &SCROLL_UP } else { &SCROLL_DOWN };
    for &(x, y) in arrow {
        fb.set(r.x + x + shift, r.y + y + shift, c.text);
    }
}

/// A raised 2px button bevel (RetroBar TaskButton / StartButton templates).
fn raised(fb: &mut Framebuffer, r: Rect, c: &Colors) {
    fb.frame(r, c.highlight, c.dark_shadow);
    fb.frame(r.inset(1, 1, 1, 1), c.light, c.shadow);
}

/// A pushed-in 2px bevel (RetroBar's IsPressed/IsChecked triggers and TaskButtonActive).
fn sunken(fb: &mut Framebuffer, r: Rect, c: &Colors) {
    fb.frame(r, c.dark_shadow, c.highlight);
    fb.frame(r.inset(1, 1, 1, 1), c.shadow, c.light);
}

/// Draws `text` so its baseline is `baseline` rows below `content_top`.
fn label(fb: &mut Framebuffer, font: &Font, x: i32, content_top: i32, baseline: i32, text: &str, color: u32) {
    font.draw(fb, x, content_top + baseline - font.ascent, text, color);
}

fn paint_start(fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts, r: Rect, pressed: bool) {
    let (c, s) = (&theme.colors, &theme.start);
    if pressed {
        sunken(fb, r, c);
    } else {
        raised(fb, r, c);
    }
    let (dx, dy) = if pressed { s.pressed_shift } else { (0, 0) };
    let content = r.inset(2 + s.pad_x + dx, 2 + s.pad_y + dy, 2 + s.pad_x - dx, 2 + s.pad_y - dy);
    fb.draw_icon(&start_flag(), content.x, content.y);
    let x = content.x + s.icon + s.icon_gap;
    label(fb, fonts.bold, x, content.y, theme.text.label_baseline, s.label, c.text);
    if pressed && s.focus_rect {
        focus_rect(fb, r.inset(3, 3, 3, 3));
    }
}

/// Windows' 16-color palette, in its own order.
const VGA: [u32; 16] = [
    0x000000, 0x800000, 0x008000, 0x808000, 0x000080, 0x800080, 0x008080, 0xC0C0C0, 0x808080, 0xFF0000, 0x00FF00,
    0xFFFF00, 0x0000FF, 0xFF00FF, 0x00FFFF, 0xFFFFFF,
];

/// The dotted focus rectangle as Windows 95 screenshots show it (16 colors): every other
/// pixel of the outline, starting one pixel after the corner, swapped with its partner in
/// the 16-color palette (light gray and black, white and dark gray, ...).
fn focus_rect(fb: &mut Framebuffer, r: Rect) {
    let swap = |p: u32| match VGA.iter().position(|&v| rgb(v) == p) {
        Some(i) => rgb(VGA[i ^ 7]),
        None => p ^ 0x00C0_C0C0,
    };
    let mut dot = |x: i32, y: i32| {
        if ((x - r.x) + (y - r.y)) % 2 == 1 && x >= 0 && y >= 0 && x < fb.width && y < fb.height {
            fb.set(x, y, swap(fb.get(x, y)));
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

fn paint_task(fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts, r: Rect, task: &Task, checker_x: i32) {
    let (c, t) = (&theme.colors, &theme.task);
    let inner = r.inset(2, 2, 2, 2);
    let padded = inner.inset(t.pad_x, t.pad_y, t.pad_x, t.pad_y);
    // Its own face, so a button drawn over another (dragged, or sliding) hides what is under it.
    fb.fill(r, c.face);
    let (font, color, content) = match task.state {
        TaskState::Normal => {
            raised(fb, r, c);
            (fonts.regular, c.text, padded)
        }
        TaskState::Flashing => {
            fb.fill(inner, c.flash_face);
            raised(fb, r, c);
            (fonts.regular, c.flash_text, padded)
        }
        TaskState::Active => {
            sunken(fb, r, c);
            fb.checker(inner, (checker_x, inner.y), c.face, c.highlight);
            fb.hline(inner.x, inner.y, inner.w, c.inner_highlight);
            let line = t.active_top_line;
            (fonts.bold, c.text, inner.inset(t.pad_x, line + t.pad_y, t.pad_x, t.pad_y - line))
        }
    };
    // A window without an icon gets its title where the icon would be (as Windows does).
    let x = match &task.icon {
        Some(icon) => {
            colors16::draw_picture(fb, theme, icon, content.x, content.y);
            content.x + t.icon + t.icon_gap
        }
        None => content.x,
    };
    // Measured as GDI measures it: the bold it makes up from MS Sans Serif one pixel wider
    // than the letters (its overhang), so a pressed button's label is cut a letter sooner.
    let text = font.elide(&task.title, content.right() - t.label_margin_right - x - font.overhang);
    label(fb, font, x, content.y, theme.text.label_baseline, &text, color);
}

/// The notification area: its sunken well, the icons, the clock.
fn paint_tray(fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts, layout: &BarLayout, bar: &Bar) {
    let (c, t) = (&theme.colors, &theme.tray);
    if layout.well.w == 0 {
        return;
    }
    fb.frame(layout.well, c.shadow, c.highlight);
    for (icon, r) in bar.tray.iter().zip(&layout.tray) {
        if let Some(icon) = icon {
            colors16::draw_picture(fb, theme, icon, r.x, r.y);
        }
    }
    let content_y = layout.well.y + 1 + t.pad;
    if !bar.clock.is_empty() {
        let x = layout::clock_text_x(fonts, layout.clock, &bar.clock);
        label(fb, fonts.regular, x, content_y, theme.text.clock_baseline, &bar.clock, c.clock_text);
    }
    // The battery's percentage: as the clock is written, on its baseline.
    if let (Some((_, part)), Some((_, text))) = (layout.number, &bar.tray_number) {
        let x = part.x + layout::NUMBER_PAD;
        label(fb, fonts.regular, x, content_y, theme.text.clock_baseline, text, c.clock_text);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    Item,
    /// Opens another menu; drawn with an arrow.
    Submenu,
    Separator,
}

/// A mark in front of a menu label, in the column left of the text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    /// An option that is on.
    Check,
    /// The chosen one of several options (like the current size).
    Bullet,
}

/// The little pictures in the window menu of Windows 98 and 2000.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuGlyph {
    Restore,
    Minimize,
    Maximize,
    Close,
}

/// One menu entry. A tab in the label starts its shortcut text ("&Close\tAlt+F4"), which
/// Windows' own menus show in a column of its own on the right.
#[derive(Clone, Debug)]
pub struct MenuEntry {
    pub kind: EntryKind,
    pub label: String,
    pub icon: Option<Icon>,
    pub mark: Option<Mark>,
    /// A picture left of the label, in Windows' own menus (the window menu).
    pub glyph: Option<MenuGlyph>,
    /// Shown grayed out and cannot be chosen (like "(Empty)" in an empty folder).
    pub disabled: bool,
    /// The default entry, drawn in bold (like "Close" in a window menu).
    pub bold: bool,
}

impl MenuEntry {
    fn new(kind: EntryKind, label: &str, icon: Option<Icon>) -> MenuEntry {
        MenuEntry { kind, label: label.to_string(), icon, mark: None, glyph: None, disabled: false, bold: false }
    }
    pub fn item(label: &str, icon: Option<Icon>) -> MenuEntry {
        MenuEntry::new(EntryKind::Item, label, icon)
    }
    pub fn submenu(label: &str, icon: Option<Icon>) -> MenuEntry {
        MenuEntry::new(EntryKind::Submenu, label, icon)
    }
    pub fn separator() -> MenuEntry {
        MenuEntry::new(EntryKind::Separator, "", None)
    }

    /// Whether choosing it does anything (separators and disabled entries don't).
    pub fn choosable(&self) -> bool {
        self.kind != EntryKind::Separator && !self.disabled
    }

    /// The label without its shortcut text, and the shortcut text (after the tab), if any.
    pub fn parts(&self) -> (&str, Option<&str>) {
        match self.label.split_once('\t') {
            Some((text, shortcut)) => (text, Some(shortcut)),
            None => (&self.label, None),
        }
    }

    fn font<'a>(&self, fonts: &Fonts<'a>) -> &'a Font {
        if self.bold { fonts.bold } else { fonts.regular }
    }
}

/// The classic menu check mark (7×7, `PopupMetrics::check_x` from the left of the entry) and
/// option bullet (6×6, centered: Calculator's View menu on the reference PCs, 98 and 2000).
pub(crate) const CHECK_ART: &[&str] = crate::art::rows("menu/check", crate::art::ANY).rows;
const BULLET_ART: &[&str] = crate::art::rows("menu/bullet", crate::art::ANY).rows;

/// Draws string art ("#" = a pixel) with its top-left corner at (x0, y0).
fn art(fb: &mut Framebuffer, art: &[&str], x0: i32, y0: i32, color: u32) {
    for (y, row) in art.iter().enumerate() {
        for (x, b) in row.bytes().enumerate() {
            if b == b'#' {
                fb.set(x0 + x as i32, y0 + y as i32, color);
            }
        }
    }
}

/// Draws `mark` in the column left of the labels, centered vertically (a check mark `check_x`
/// from the column's left edge).
fn mark(fb: &mut Framebuffer, mark: Mark, column: Rect, check_x: i32, color: u32) {
    let shape: &[&str] = match mark {
        Mark::Check => CHECK_ART,
        Mark::Bullet => BULLET_ART,
    };
    let (w, h) = (shape[0].len() as i32, shape.len() as i32);
    let x0 = if mark == Mark::Check { column.x + check_x } else { column.x + (column.w - w) / 2 };
    art(fb, shape, x0, column.y + (column.h - h) / 2, color);
}

/// The window menu's pictures in Windows 98 (cut from the Reference PC's; first read in the v86
/// emulator, where these places were measured), with where they go from the top-left of their
/// entry.
fn menu_glyph(fb: &mut Framebuffer, glyph: MenuGlyph, x: i32, y: i32, color: u32) {
    const RESTORE: &[&str] = crate::art::rows("menu/restore", crate::art::Look::Win98).rows;
    const MINIMIZE: &[&str] = crate::art::rows("menu/minimize", crate::art::Look::Win98).rows;
    const MAXIMIZE: &[&str] = crate::art::rows("menu/maximize", crate::art::Look::Win98).rows;
    const CLOSE: &[&str] = crate::art::rows("menu/close", crate::art::Look::Win98).rows;
    let (shape, dx, dy): (&[&str], i32, i32) = match glyph {
        MenuGlyph::Restore => (RESTORE, 4, 4),
        MenuGlyph::Minimize => (MINIMIZE, 5, 11),
        MenuGlyph::Maximize => (MAXIMIZE, 4, 3),
        MenuGlyph::Close => (CLOSE, 5, 4),
    };
    art(fb, shape, x + dx, y + dy, color);
}

/// A menu's contents and which entry the pointer is over. An "&" in a label marks its
/// access key, as in Windows menus ("&&" is a plain "&").
#[derive(Clone, Debug, Default)]
pub struct Menu {
    pub entries: Vec<MenuEntry>,
    pub hover: Option<usize>,
    /// Underline the access keys. Windows 95 always did; Windows 2000 only when the menu
    /// was opened from the keyboard.
    pub show_keys: bool,
    /// Rows of face color under the last entry beyond the style's own (Windows 2000's
    /// Documents menu has one more than its other menus).
    pub extra_bottom: i32,
    /// Windows 2000 allocates bitmap space in a program's system menu. Only the
    /// entries with a bitmap grow taller, but every label uses the same column.
    pub system_bitmaps: bool,
    /// A grayed-out entry's access key is underlined with every other pixel: Calculator's own
    /// menus on Windows 2000 (Edit > Paste on the reference PC; the window menu and the games'
    /// menus underline solid).
    pub dotted_disabled_keys: bool,
    /// Explorer's own menu (a desktop icon's): Windows 2000 drew its bold entry in a bold whose
    /// "O" and "e" differ from today's Tahoma Bold (`Font::explorer_bold`).
    pub explorer_bold: bool,
    /// Labels the menu is made wide enough for although they are not among its entries (Windows
    /// 2000's Quick Launch list: as wide as the longest name on the band, shown or not).
    pub width_labels: Vec<String>,
}

/// Runs `f` with the fonts `menu` is drawn with: Explorer's bold for an Explorer menu.
fn menu_fonts<R>(fonts: &Fonts, menu: &Menu, f: impl FnOnce(&Fonts) -> R) -> R {
    let variant = if menu.explorer_bold { fonts.bold.explorer_bold() } else { None };
    match &variant {
        Some(bold) => f(&Fonts { regular: fonts.regular, bold }),
        None => f(fonts),
    }
}

impl Menu {
    pub fn new(entries: Vec<MenuEntry>) -> Menu {
        Menu { entries, ..Menu::default() }
    }

    /// Whether entry `i` can be highlighted (by the pointer or the arrow keys). Windows' own
    /// menus highlight grayed-out entries too (it just does nothing to choose them); the
    /// Start menu skips them.
    pub fn selectable(&self, i: usize, style: MenuStyle) -> bool {
        self.entries.get(i).is_some_and(|e| {
            e.kind != EntryKind::Separator && (!e.disabled || style == MenuStyle::Popup)
        })
    }
}

/// The kind of menu, which decides its look and metrics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuStyle {
    /// Windows' own menus, like the bar's right-click menu: text, marks and arrows.
    Popup,
    /// The Start menu: the banner on the left, 32px icons.
    Start,
    /// The menus that cascade from the Start menu (and a task button's window list): 16px icons.
    Cascade,
    /// The Start menu with "Show small icons in Start menu" on: 16px icons, no banner.
    StartSmall,
}

/// Where a menu's entries go, and its size, at 1×.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuLayout {
    pub width: i32,
    pub height: i32,
    /// One rectangle per entry, from the menu's top-left corner.
    pub rects: Vec<Rect>,
    /// Where shortcut texts start, from the left of an entry (0 when there are none).
    pub shortcut_x: i32,
}

impl MenuLayout {
    /// The entry under (x, y), if it can be highlighted (`Menu::selectable`).
    pub fn hit(&self, menu: &Menu, style: MenuStyle, x: i32, y: i32) -> Option<usize> {
        let i = self.rects.iter().position(|r| r.contains(x, y))?;
        menu.selectable(i, style).then_some(i)
    }
}

fn frame_of(theme: &Theme, style: MenuStyle) -> i32 {
    match style {
        MenuStyle::Popup => theme.popup.frame,
        MenuStyle::Start | MenuStyle::StartSmall | MenuStyle::Cascade => theme.start_menu.frame,
    }
}

fn icon_items(theme: &Theme, style: MenuStyle) -> &IconItems {
    match style {
        MenuStyle::Start => &theme.start_menu.large,
        MenuStyle::StartSmall => &theme.start_menu.start_small,
        _ => &theme.start_menu.small,
    }
}

/// Columns of Windows' own menus (`MenuStyle::Popup`), from the left of an entry: where
/// labels are drawn, where shortcut texts start (0 if there are none), and the entry width.
/// Measured in the v86 emulator: Windows 95's window menu is 21 + 43 ("Maximize") + 8 + 30
/// ("Alt+F4" in the regular font) + 19 = 121 wide; Windows 98's, with its pictures, leaves
/// 17 more before the shortcuts though its labels start at 17 (138 wide); Windows 98's
/// Notepad Edit menu is 21 + 55 ("Word Wrap") + 8 + 28 ("Ctrl+Z") + 19 = 131.
fn popup_columns(theme: &Theme, fonts: &Fonts, menu: &Menu) -> (i32, i32, i32) {
    let p = &theme.popup;
    let labels = || menu.entries.iter().filter(|e| e.kind != EntryKind::Separator);
    // Labels as GDI measures them: in the bold it makes up from MS Sans Serif, one pixel
    // wider than the letters (its overhang). Seen in Windows 95's and 98's speaker menus,
    // whose bold entry is the widest.
    let widest = labels()
        .map(|e| {
            if menu.system_bitmaps && e.bold {
                let label = split_access_key(e.parts().0).0;
                return fonts.regular.measure(&label) + label.chars().count() as i32;
            }
            let font = e.font(fonts);
            font.measure_label(e.parts().0) + font.overhang
        })
        .max()
        .unwrap_or(0);
    let widest_shortcut = labels().filter_map(|e| e.parts().1).map(|s| fonts.regular.measure(s)).max();
    let glyphs = menu.entries.iter().any(|e| e.glyph.is_some());
    let (text_x, reserve) = if menu.system_bitmaps {
        (22, 20)
    } else if glyphs {
        (p.glyph_text_x, p.glyph_reserve)
    } else {
        (p.text_x, 0)
    };
    let labels_end = p.text_x + reserve + widest;
    match widest_shortcut {
        Some(w) => (text_x, labels_end + p.shortcut_gap, labels_end + p.shortcut_gap + w + p.shortcut_pad_right),
        None => (text_x, 0, labels_end + p.text_pad_right),
    }
}

/// Width of every item: room for the longest label plus the style's columns around it.
fn item_width(theme: &Theme, fonts: &Fonts, menu: &Menu, style: MenuStyle) -> i32 {
    let labels = menu.entries.iter().filter(|e| e.kind != EntryKind::Separator);
    let raw = style == MenuStyle::Cascade && theme.start_menu.cascade_counts_ampersands;
    let measure = |e: &MenuEntry| if raw { e.font(fonts).measure(&e.label) } else { e.font(fonts).measure_label(&e.label) };
    let widest = labels.map(measure).max().unwrap_or(0);
    let widest = menu.width_labels.iter().map(|l| fonts.regular.measure_label(l)).fold(widest, i32::max);
    match style {
        MenuStyle::Popup => popup_columns(theme, fonts, menu).2,
        MenuStyle::Start | MenuStyle::StartSmall | MenuStyle::Cascade => {
            let items = icon_items(theme, style);
            let widest = if style == MenuStyle::Cascade { widest.min(theme.start_menu.max_label_w) } else { widest };
            let arrows = menu.entries.iter().any(|e| e.kind == EntryKind::Submenu);
            let pad = if arrows { items.pad_right } else { items.pad_right_no_arrows };
            items.icon_x + items.icon + items.text_gap + widest + pad
        }
    }
}

/// Lays out `menu`: entries top to bottom, and when they would run past `max_height` (the
/// screen), on in another column to the right, as Windows' long menus do.
pub fn layout_menu(theme: &Theme, fonts: &Fonts, menu: &Menu, style: MenuStyle, max_height: i32) -> MenuLayout {
    menu_fonts(fonts, menu, |fonts| layout_menu_with(theme, fonts, menu, style, max_height))
}

fn layout_menu_with(theme: &Theme, fonts: &Fonts, menu: &Menu, style: MenuStyle, max_height: i32) -> MenuLayout {
    let m = &theme.start_menu;
    let frame = frame_of(theme, style);
    let left = frame + if style == MenuStyle::Start { m.banner_w } else { 0 };
    // Windows 98 leaves a row under the last entry of its Start menu, not with small icons.
    let pad = if matches!(style, MenuStyle::Popup | MenuStyle::StartSmall) { 0 } else { m.bottom_pad } + menu.extra_bottom;
    let item_w = item_width(theme, fonts, menu, style);
    let mut first_separator = true;
    let (mut x, mut y, mut bottom) = (left, frame, frame);
    let mut rects = Vec::with_capacity(menu.entries.len());
    for e in &menu.entries {
        let h = match (e.kind, style) {
            (EntryKind::Separator, MenuStyle::Popup) => theme.popup.sep.h,
            (EntryKind::Separator, _) => {
                let starts = matches!(style, MenuStyle::Start | MenuStyle::StartSmall);
                let extra = if starts && first_separator { m.first_sep_extra } else { 0 };
                first_separator = false;
                m.sep.h + extra
            }
            (_, MenuStyle::Popup) if menu.system_bitmaps && e.glyph.is_some() => 20,
            (_, MenuStyle::Popup) => theme.popup.item_h,
            (_, _) => icon_items(theme, style).h,
        };
        if y + h > max_height - frame - pad && y > frame {
            x += item_w;
            y = frame;
        }
        rects.push(Rect::new(x, y, item_w, h));
        y += h;
        bottom = bottom.max(y);
    }
    let shortcut_x = if style == MenuStyle::Popup { popup_columns(theme, fonts, menu).1 } else { 0 };
    MenuLayout { width: x + item_w + frame, height: bottom + pad + frame, rects, shortcut_x }
}

/// The menu window's border: a raised window edge plus face color inside it.
fn menu_frame(fb: &mut Framebuffer, c: &Colors) {
    let outer = Rect::new(0, 0, fb.width, fb.height);
    fb.fill(outer, c.face);
    fb.frame(outer, c.light, c.dark_shadow);
    fb.frame(outer.inset(1, 1, 1, 1), c.highlight, c.shadow);
}

/// An etched line (shadow above highlight). Extra rows of a taller separator go above it.
fn separator(fb: &mut Framebuffer, r: Rect, sep: &Separator, c: &Colors) {
    let y = r.bottom() - sep.h + sep.line_y;
    fb.hline(r.x + sep.inset, y, r.w - 2 * sep.inset, c.shadow);
    fb.hline(r.x + sep.inset, y + 1, r.w - 2 * sep.inset, c.highlight);
}

/// The classic menu arrow: a 4×7 triangle pointing right, in item `r`.
fn arrow(fb: &mut Framebuffer, r: Rect, a: &Arrow, color: u32) {
    let (x, y) = (r.right() - a.right - 4, r.y + (r.h - a.box_h) / 2);
    for row in 0..7 {
        fb.hline(x, y + row, 4 - (row - 3).abs(), color);
    }
}

/// How a menu label is drawn.
#[derive(Clone, Copy)]
enum Ink {
    Normal(u32),
    /// Grayed out: etched, a highlight copy 1px down-right under the shadow color.
    Etched,
    /// Grayed out under the selection: just the shadow color (Windows 95's window menu, its
    /// "Restore" entry while the window is not maximized).
    Gray,
}

/// A label in `ink`.
#[allow(clippy::too_many_arguments)]
fn menu_label(
    fb: &mut Framebuffer,
    font: &Font,
    at: (i32, i32),
    label: &str,
    max_w: i32,
    ink: Ink,
    keys: bool,
    c: &Colors,
    overstrike: bool,
    dotted: bool,
) {
    if dotted && keys && !overstrike && matches!(ink, Ink::Etched) {
        // Windows 2000's grayed-out entries: the access key's underline is every other pixel,
        // etched like the letters.
        menu_label(fb, font, at, label, max_w, ink, false, c, false, false);
        if let Some((dx, w)) = font.key_underline(label, max_w) {
            // The dots fall on the menu's even columns (Calculator's Paste and Explorer's Empty
            // Recycle Bin on the Reference PC, whose underlines start on different columns).
            for x in (at.0 + dx..at.0 + dx + w).filter(|x| x.rem_euclid(2) == 0) {
                fb.set(x + 1, at.1 + font.height, c.highlight);
                fb.set(x, at.1 + font.height - 1, c.shadow);
            }
        }
        return;
    }
    let mut draw = |(x, y), color| {
        if overstrike {
            // The 2000 system menu makes its default command bold by striking each
            // regular glyph again one pixel to the right and advancing one extra pixel.
            let (label, key) = split_access_key(label);
            let mut pen = x;
            for (i, ch) in label.chars().enumerate() {
                let mut buf = [0; 4];
                let letter = ch.encode_utf8(&mut buf);
                let width = font.draw(fb, pen, y, letter, color) + 1;
                font.draw(fb, pen + 1, y, letter, color);
                if keys && key == Some(i) {
                    fb.hline(pen, y + font.height - 1, width, color);
                }
                pen += width;
            }
        } else {
            font.draw_label(fb, (x, y), label, max_w, color, keys);
        }
    };
    match ink {
        Ink::Etched => {
            draw((at.0 + 1, at.1 + 1), c.etch);
            draw(at, c.shadow);
        }
        Ink::Gray => {
            draw(at, c.shadow);
        }
        Ink::Normal(color) => {
            draw(at, color);
        }
    }
}

/// Paints `menu` filling the framebuffer, as laid out by `layout_menu` (same style).
pub fn paint_menu(
    fb: &mut Framebuffer,
    theme: &Theme,
    fonts: &Fonts,
    menu: &Menu,
    style: MenuStyle,
    layout: &MenuLayout,
) {
    menu_fonts(fonts, menu, |fonts| paint_menu_with(fb, theme, fonts, menu, style, layout));
}

fn paint_menu_with(fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts, menu: &Menu, style: MenuStyle, layout: &MenuLayout) {
    let (c, m, p) = (&theme.colors, &theme.start_menu, &theme.popup);
    menu_frame(fb, c);
    let frame = frame_of(theme, style);
    if style == MenuStyle::Start {
        // The banner runs the full height inside the frame, under the bottom padding too.
        m.banner.paint(fb, Rect::new(frame, frame, m.banner_w, fb.height - 2 * frame));
    }
    let popup_text_x = if style == MenuStyle::Popup { popup_columns(theme, fonts, menu).0 } else { 0 };
    for (i, (e, &r)) in menu.entries.iter().zip(&layout.rects).enumerate() {
        if e.kind == EntryKind::Separator {
            separator(fb, r, if style == MenuStyle::Popup { &p.sep } else { &m.sep }, c);
            continue;
        }
        let font = e.font(fonts);
        let hot = menu.hover == Some(i) && menu.selectable(i, style);
        let color = if hot { c.menu_highlight_text } else { c.text };
        let etched = style != MenuStyle::Popup || p.etched_disabled;
        let ink = match (e.disabled, hot) {
            (true, true) => Ink::Gray,
            (true, false) if etched => Ink::Etched,
            (true, false) => Ink::Gray,
            (false, _) => Ink::Normal(color),
        };
        if style == MenuStyle::Popup {
            if hot {
                fb.fill(r, c.menu_highlight);
            }
            if let Some(mk) = e.mark {
                mark(fb, mk, Rect::new(r.x, r.y, p.text_x, r.h), p.check_x, color);
            }
            if let Some(g) = e.glyph {
                let (gx, gy) = if menu.system_bitmaps { (r.x - 2, r.y + 1) } else { (r.x, r.y) };
                // Grayed out: etched like its label (the reference PC's Windows 98).
                if matches!(ink, Ink::Etched) {
                    menu_glyph(fb, g, gx + 1, gy + 1, c.etch);
                }
                let glyph_color = if e.disabled { c.shadow } else { color };
                menu_glyph(fb, g, gx, gy, glyph_color);
            }
            let (text, shortcut) = e.parts();
            let y = r.y + p.text_y + (r.h - p.item_h + 1) / 2;
            let overstrike = menu.system_bitmaps && e.bold;
            let font = if overstrike { fonts.regular } else { font };
            menu_label(fb, font, (r.x + popup_text_x, y), text, i32::MAX, ink, menu.show_keys, c, overstrike, menu.dotted_disabled_keys);
            if let Some(shortcut) = shortcut {
                // Shortcut texts have no access keys: "&" is shown as it is.
                menu_label(fb, font, (r.x + layout.shortcut_x, y), &shortcut.replace('&', "&&"), i32::MAX, ink, false, c, overstrike, false);
            }
            if e.kind == EntryKind::Submenu {
                arrow(fb, r, &p.arrow, color);
            }
            continue;
        }
        let items = icon_items(theme, style);
        if hot {
            fb.fill(r.inset(0, items.highlight_inset, 0, items.highlight_inset), c.menu_highlight);
        }
        if let Some(icon) = &e.icon {
            let (x, y) = (r.x + items.icon_x, r.y + (r.h - icon.height) / 2);
            if let Some((normal, selected)) = m.icon_cell {
                fb.fill(Rect::new(x, y, icon.width, icon.height), if hot { selected } else { normal });
            }
            colors16::draw_picture(fb, theme, icon, x, y);
        }
        let text_x = r.x + items.icon_x + items.icon + items.text_gap;
        let text_y = r.y + (r.h - font.height + i32::from(m.text_round_up)) / 2;
        let max_w = if style == MenuStyle::Cascade { m.max_label_w } else { i32::MAX };
        menu_label(fb, font, (text_x, text_y), &e.label, max_w, ink, menu.show_keys, c, false, false);
        if e.kind == EntryKind::Submenu {
            arrow(fb, r, if style == MenuStyle::StartSmall { &m.start_small_arrow } else { &m.arrow }, color);
        }
    }
}
