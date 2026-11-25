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
    if theme.bar_style == crate::theme::BarStyle::Xp {
        return crate::xp::bar_layout(theme, fonts, width, bar);
    }
    let number = bar.tray_number.as_ref().filter(|(_, text)| !text.is_empty()).map(|&(i, _)| i);
    let quick = bar.quick_launch.as_ref().map(quick_launch::Band::shape);
    layout::bar_with(theme, fonts, width, bar.tasks.len(), bar.tray.len(), number, &bar.clock, &bar.clock_widest, bar.row, quick)
}

/// Paints the whole bar (the framebuffer should be `theme.bar.height` tall) and returns
/// where everything went, for hit-testing.
pub fn paint_bar(fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts, bar: &Bar) -> BarLayout {
    if theme.bar_style == crate::theme::BarStyle::Xp {
        return crate::xp::paint_bar(fb, theme, fonts, bar);
    }
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

/// The task list's two scroll arrows (the XP bar uses them as the 2000 look draws them).
pub(crate) fn paint_scroll_arrows(fb: &mut Framebuffer, theme: &Theme, s: layout::TaskScroll, pressed: Option<ScrollArrow>) {
    let c = &theme.colors;
    scroll_button(fb, c, s.up, ScrollArrow::Up, pressed == Some(ScrollArrow::Up));
    scroll_button(fb, c, s.down, ScrollArrow::Down, pressed == Some(ScrollArrow::Down));
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
    /// Windows XP's Start menu (`MenuStyle::XpStart`, brief 112): the Windows user's name in its
    /// band.
    pub user: String,
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
    /// Windows XP's Start menu (brief 112): laid out and painted by `xp_start`.
    XpStart,
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
    /// The menu's frame width.
    pub frame: i32,
    /// Each column's left edge and width (one for a menu that fits one column).
    pub columns: Vec<(i32, i32)>,
    /// Where an etched line between two columns stands (its first pixel; it is 2 wide).
    pub lines: Vec<i32>,
    /// A menu that scrolls (`Long::Scroll`): its entries' `rects` are then where they stand
    /// unscrolled, and `shown_rect` says where they show.
    pub scroll: Option<Scroll>,
}

impl MenuLayout {
    /// The entry under (x, y), if it can be highlighted (`Menu::selectable`); in a scrolling
    /// menu only one that shows there between the strips.
    pub fn hit(&self, menu: &Menu, style: MenuStyle, x: i32, y: i32) -> Option<usize> {
        if self.scroll.is_some() {
            let (top, bottom) = self.view();
            if y < top || y >= bottom {
                return None;
            }
            let i = (0..self.rects.len()).find(|&i| self.shown_rect(i).is_some_and(|r| r.contains(x, y)))?;
            return menu.selectable(i, style).then_some(i);
        }
        let i = self.rects.iter().position(|r| r.contains(x, y))?;
        menu.selectable(i, style).then_some(i)
    }
}

fn frame_of(theme: &Theme, style: MenuStyle) -> i32 {
    match style {
        MenuStyle::Popup => theme.popup.frame,
        MenuStyle::Start | MenuStyle::StartSmall | MenuStyle::Cascade => theme.start_menu.frame,
        MenuStyle::XpStart => 0,
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
        MenuStyle::XpStart => crate::xp_start::WIDTH,
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
    let mut columns = vec![(left, item_w)];
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
            columns.push((x, item_w));
        }
        rects.push(Rect::new(x, y, item_w, h));
        y += h;
        bottom = bottom.max(y);
    }
    let shortcut_x = if style == MenuStyle::Popup { popup_columns(theme, fonts, menu).1 } else { 0 };
    MenuLayout { width: x + item_w + frame, height: bottom + pad + frame, rects, shortcut_x, frame, columns, lines: Vec::new(), scroll: None }
}

/// How a menu longer than the screen goes on (brief 91; the Reference PCs, survey part 1 of
/// `private/probes/start-long`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Long {
    /// Further columns with an etched line between them (Windows 95, and Windows 2000 on a fresh
    /// installation): each as wide as its own widest entry (`own_widths`, Windows 95), or all as
    /// wide as the menu's widest entry (Windows 2000); one pixel of the face after each column.
    Columns { own_widths: bool },
    /// One column the screen's height whose entries scroll between arrow strips (Windows 98;
    /// `flat`: Windows 2000's strips, which do not sink under the pointer).
    Scroll { flat: bool },
}

/// One of a scrolling menu's arrow strips.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strip {
    Up,
    Down,
}

/// A scrolling menu's state: how far its entries are scrolled, in 1× pixels from the top.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scroll {
    /// How far the entries are scrolled (0: the first entry at the top).
    pub offset: i32,
    /// The furthest it goes: the last entry then reaches the bottom of the frame.
    pub end: i32,
    /// One step: an entry's height (98 scrolls one entry at a time).
    pub row: i32,
    /// Windows 2000's flat strips (they do not sink under the pointer, the arrow stays).
    pub flat: bool,
    /// The strip under the pointer.
    pub hot: Option<Strip>,
}

/// The height of an arrow strip, inside the frame (all three Reference PCs that scroll: 12).
pub const STRIP_H: i32 = 12;

impl Scroll {
    /// Whether `strip` shows: only when there is more on its side.
    pub fn shows(&self, strip: Strip) -> bool {
        match strip {
            Strip::Up => self.offset > 0,
            Strip::Down => self.offset < self.end,
        }
    }

    /// Scrolls by `px` toward `strip`'s side, never past either end. True if it moved.
    pub fn step(&mut self, strip: Strip, px: i32) -> bool {
        let before = self.offset;
        self.set(match strip {
            Strip::Up => self.offset - px,
            Strip::Down => self.offset + px,
        });
        self.offset != before
    }

    /// Scrolls to `offset`, kept between the two ends.
    pub fn set(&mut self, offset: i32) {
        self.offset = offset.clamp(0, self.end);
    }
}

/// How often a scrolling menu steps while the pointer rests on a strip, in milliseconds: the
/// first step, then each further one (measured: Windows 98 one entry 100..200 ms after the pointer
/// came, then one every ~300 ms; Windows 2000 20 px every ~60 ms).
pub fn scroll_pace(flat: bool) -> (u32, u32) {
    if flat { (60, 60) } else { (150, 300) }
}

/// How far one step of the pace scrolls, in 1× pixels: Windows 98 one entry, Windows 2000 20 px
/// (its last step a part step to land on the end, as `Scroll::step` does).
pub fn scroll_step(scroll: &Scroll) -> i32 {
    if scroll.flat { 20 } else { scroll.row }
}

impl MenuLayout {
    /// Entry `i`'s rectangle where it shows now (scrolled), from the menu's top-left corner;
    /// `None` when it is scrolled out of sight altogether.
    pub fn shown_rect(&self, i: usize) -> Option<Rect> {
        let r = *self.rects.get(i)?;
        let Some(s) = self.scroll else { return Some(r) };
        let shifted = Rect::new(r.x, r.y - s.offset, r.w, r.h);
        let (top, bottom) = self.view();
        (shifted.bottom() > top && shifted.y < bottom).then_some(shifted)
    }

    /// The rows where entries show, between the strips (the whole inside without scrolling).
    pub fn view(&self) -> (i32, i32) {
        let frame = self.frame;
        let Some(s) = self.scroll else { return (frame, self.height - frame) };
        let top = frame + if s.shows(Strip::Up) { STRIP_H } else { 0 };
        let bottom = self.inner_bottom() - if s.shows(Strip::Down) { STRIP_H } else { 0 };
        (top, bottom)
    }

    /// The row below the last row entries may reach in a scrolling menu: the inner bottom, or for
    /// Windows 2000's flat strips one row lower (its menus' bottom border is 2 pixels, and the face
    /// row above it is the entries': the Reference PC's last entry ends on row 597 of 600, its
    /// bottom strip at 586..597; brief 92).
    pub fn inner_bottom(&self) -> i32 {
        self.height - self.frame + i32::from(self.scroll.is_some_and(|s| s.flat))
    }
    /// The strip at (x, y), if one shows there (over the entries' part only: the Start menu's
    /// banner has none).
    pub fn strip_at(&self, x: i32, y: i32) -> Option<Strip> {
        let s = self.scroll?;
        let left = self.columns.first().map_or(self.frame, |&(x, _)| x);
        let inside = x >= left && x < self.width - self.frame;
        if !inside {
            return None;
        }
        if s.shows(Strip::Up) && y >= self.frame && y < self.frame + STRIP_H {
            return Some(Strip::Up);
        }
        let bottom = self.inner_bottom();
        (s.shows(Strip::Down) && y >= bottom - STRIP_H && y < bottom).then_some(Strip::Down)
    }

    /// Scrolls so that entry `i` shows whole: up to its top (or to the very top for the first
    /// rows), down until it sits just above the bottom strip (or to the end). True if it moved.
    pub fn reveal(&mut self, i: usize) -> bool {
        let (Some(mut s), Some(&r)) = (self.scroll, self.rects.get(i)) else { return false };
        let before = s.offset;
        let (frame, inner_bottom) = (self.frame, self.inner_bottom());
        if r.y - s.offset < frame + if s.offset > 0 { STRIP_H } else { 0 } {
            // Up: the entry's top just under the top strip (none at the very top).
            let to = r.y - frame - STRIP_H;
            s.set(if to <= 0 { 0 } else { to });
        } else if r.bottom() - s.offset > inner_bottom - if s.offset < s.end { STRIP_H } else { 0 } {
            // Down: the entry's bottom just above the bottom strip, or the end.
            let to = r.bottom() - (inner_bottom - STRIP_H);
            s.set(if to >= s.end { s.end } else { to });
        }
        self.scroll = Some(s);
        s.offset != before
    }

    /// The column entry `i` stands in (0 for a menu of one column).
    pub fn column_of(&self, i: usize) -> usize {
        let Some(r) = self.rects.get(i) else { return 0 };
        self.columns.iter().rposition(|&(x, _)| x <= r.x).unwrap_or(0)
    }

    /// The entry in the same row of the column `toward` the left (-1) or right (+1) of entry
    /// `i`'s, or the last of that column if it is shorter (Windows 95's Left and Right); only an
    /// entry that can be chosen (`Menu::selectable`: never a separator).
    pub fn beside(&self, menu: &Menu, style: MenuStyle, i: usize, toward: i32) -> Option<usize> {
        let col = self.column_of(i) as i32 + toward;
        let &(x, _) = self.columns.get(usize::try_from(col).ok()?)?;
        let y = self.rects.get(i)?.y;
        let in_col: Vec<usize> = (0..self.rects.len()).filter(|&j| self.rects[j].x == x && menu.selectable(j, style)).collect();
        in_col.iter().copied().find(|&j| self.rects[j].y >= y).or_else(|| in_col.last().copied())
    }
}

/// Lays out a menu of the Start menu's family for a screen `screen_h` high (1× pixels), long
/// menus as `long` says (brief 91). A menu that fits one column is laid out exactly as
/// `layout_menu` lays it out. `Long::Scroll` makes one column the whole screen's height whose
/// entries scroll (`MenuLayout::scroll`, starting at `offset`); `Long::Columns` fills columns
/// down then across, each as wide as its own widest entry, an etched line between them.
pub fn layout_long_menu(theme: &Theme, fonts: &Fonts, menu: &Menu, style: MenuStyle, screen_h: i32, long: Long, offset: i32) -> MenuLayout {
    menu_fonts(fonts, menu, |fonts| {
        let base = layout_menu_with(theme, fonts, menu, style, screen_h);
        if base.columns.len() <= 1 {
            return base;
        }
        match long {
            Long::Scroll { flat } => {
                // One column of every entry, as tall as it is; shown through the screen's height.
                let tall = layout_menu_with(theme, fonts, menu, style, i32::MAX);
                // At the end the last entry reaches the frame (no bottom padding: the Reference
                // PC's last entry ends on the inner bottom row).
                let frame = tall.frame;
                let content = tall.rects.iter().map(|r| r.bottom()).max().unwrap_or(frame) - frame;
                // Windows 2000's flat menus let the entries reach the row above their 2-pixel border
                // (MenuLayout::inner_bottom).
                let end = (content - (screen_h - 2 * frame + i32::from(flat))).max(0);
                let row = icon_items(theme, style).h;
                let scroll = Scroll { offset: offset.clamp(0, end), end, row, flat, hot: None };
                MenuLayout { height: screen_h, scroll: Some(scroll), ..tall }
            }
            Long::Columns { own_widths } => lined_columns(theme, fonts, menu, style, &base, own_widths),
        }
    })
}

/// `base`'s columns (as `layout_menu` breaks them), each as wide as its own widest entry
/// (`own_widths`, Windows 95) or as the menu's widest (Windows 2000), and between neighbours a
/// pixel of the face, the 2-pixel etched line, a pixel of the face (Windows 95 and 2000, survey
/// part 1, 2: Windows 95's 50 entries at 800x600: the first column's entries at x 163..289, the
/// line at 291, the second's at 294; Windows 2000's 59: the lines at 384 and 517).
fn lined_columns(theme: &Theme, fonts: &Fonts, menu: &Menu, style: MenuStyle, base: &MenuLayout, own_widths: bool) -> MenuLayout {
    let frame = base.frame;
    let mut rects = base.rects.clone();
    let mut columns = Vec::with_capacity(base.columns.len());
    let mut lines = Vec::new();
    let mut x = base.columns[0].0;
    for (k, &(old_x, old_w)) in base.columns.iter().enumerate() {
        let members: Vec<usize> = (0..rects.len()).filter(|&i| base.rects[i].x == old_x).collect();
        let w = if own_widths { column_width(theme, fonts, menu, style, &members) } else { old_w };
        for &i in &members {
            rects[i] = Rect::new(x, rects[i].y, w, rects[i].h);
        }
        columns.push((x, w));
        x += w;
        if k + 1 < base.columns.len() {
            // A pixel of the face, the line, a pixel of the face.
            lines.push(x + 1);
            x += 4;
        }
    }
    MenuLayout { width: x + frame, rects, columns, lines, ..base.clone() }
}

/// The width of a column holding the entries `members`: room for its own longest label, with the
/// style's columns around it (an arrow's room only if one of them opens a submenu).
fn column_width(theme: &Theme, fonts: &Fonts, menu: &Menu, style: MenuStyle, members: &[usize]) -> i32 {
    if style == MenuStyle::Popup {
        return item_width(theme, fonts, menu, style);
    }
    let raw = style == MenuStyle::Cascade && theme.start_menu.cascade_counts_ampersands;
    let entries = || members.iter().filter_map(|&i| menu.entries.get(i)).filter(|e| e.kind != EntryKind::Separator);
    let measure = |e: &MenuEntry| if raw { e.font(fonts).measure(&e.label) } else { e.font(fonts).measure_label(&e.label) };
    let widest = entries().map(measure).max().unwrap_or(0);
    let items = icon_items(theme, style);
    let widest = if style == MenuStyle::Cascade { widest.min(theme.start_menu.max_label_w) } else { widest };
    let arrows = entries().any(|e| e.kind == EntryKind::Submenu);
    let pad = if arrows { items.pad_right } else { items.pad_right_no_arrows };
    items.icon_x + items.icon + items.text_gap + widest + pad
}

/// A scrolling menu (`Long::Scroll`): the frame and (Start menu) the banner as for a menu of its
/// height, standing still; only the entries whose rows meet the view are painted, shifted by the
/// scroll and clipped to the view (no picture as tall as the whole menu: brief 92); then the
/// strips over the entries' part only, from the banner's right edge to the frame (Windows 98's
/// own Start menu when it scrolls: `win98-...-startmenu-with-dropped-msdosprompt-on-top-scrolls`).
/// The strips: Windows 98 12 rows of the face color, a black arrow 5, 3 and 1 pixels wide in the
/// middle of their width, 3 rows in from the inner edge; under the pointer sunken, black above and
/// left, the light 3-D color below and right, without its arrow; Windows 2000 the same, flat
/// always, its down arrow one row lower. Measured, survey part 1, 3.
fn paint_scrolled(fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts, menu: &Menu, style: MenuStyle, layout: &MenuLayout, scroll: Scroll) {
    let c = &theme.colors;
    let frame = layout.frame;
    let left = layout.columns.first().map_or(frame, |&(x, _)| x);
    // The frame and the banner, still.
    let bare = Menu { entries: Vec::new(), hover: None, ..Menu::default() };
    let bare_layout = MenuLayout { rects: Vec::new(), scroll: None, ..layout.clone() };
    paint_menu_with(fb, theme, fonts, &bare, style, &bare_layout);
    // The entries that show, where they show.
    let (top, bottom) = layout.view();
    // Only entries the menu really has: a layout longer than its menu (never made so, but a panic
    // would end the bar) paints what there is (brief 94).
    let showing: Vec<usize> =
        (0..layout.rects.len().min(menu.entries.len())).filter(|&i| layout.shown_rect(i).is_some()).collect();
    let part = Menu {
        entries: showing.iter().filter_map(|&i| menu.entries.get(i).cloned()).collect(),
        hover: menu.hover.and_then(|h| showing.iter().position(|&i| i == h)),
        show_keys: menu.show_keys,
        extra_bottom: menu.extra_bottom,
        system_bitmaps: menu.system_bitmaps,
        dotted_disabled_keys: menu.dotted_disabled_keys,
        explorer_bold: menu.explorer_bold,
        width_labels: Vec::new(),
        user: String::new(),
    };
    let part_layout = MenuLayout {
        rects: showing.iter().filter_map(|&i| layout.rects.get(i)).map(|r| Rect::new(r.x, r.y - scroll.offset, r.w, r.h)).collect(),
        scroll: None,
        ..layout.clone()
    };
    let mut entries = Framebuffer::new(layout.width, layout.height, 0);
    paint_menu_with(&mut entries, theme, fonts, &part, style, &part_layout);
    for y in top..bottom {
        for x in left..layout.width - frame {
            fb.set(x, y, entries.get(x, y));
        }
    }
    let inner_w = layout.width - frame - left;
    let arrow_x = left + (inner_w - 1 - 5) / 2;
    for strip in [Strip::Up, Strip::Down] {
        if !scroll.shows(strip) {
            continue;
        }
        let y = match strip {
            Strip::Up => frame,
            Strip::Down => layout.inner_bottom() - STRIP_H,
        };
        let r = Rect::new(left, y, inner_w, STRIP_H);
        fb.fill(r, c.face);
        if scroll.hot == Some(strip) && !scroll.flat {
            fb.hline(r.x, r.y, r.w, c.dark_shadow);
            fb.vline(r.x, r.y, r.h, c.dark_shadow);
            fb.hline(r.x + 1, r.bottom() - 1, r.w - 1, c.light);
            fb.vline(r.right() - 1, r.y + 1, r.h - 1, c.light);
            continue;
        }
        // Up: 1, 3, 5 wide from 3 rows under the strip's top; down: 5, 3, 1 ending 3 rows above
        // the strip's bottom.
        let rows: [(i32, i32); 3] = match strip {
            Strip::Up => [(r.y + 3, 1), (r.y + 4, 3), (r.y + 5, 5)],
            Strip::Down => {
                let last = r.bottom() - 1 - 3;
                [(last - 2, 5), (last - 1, 3), (last, 1)]
            }
        };
        for (row, w) in rows {
            fb.hline(arrow_x + (5 - w) / 2, row, w, c.text);
        }
    }
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
    if style == MenuStyle::XpStart {
        crate::xp_start::paint(fb, fonts, menu, layout);
        return;
    }
    if let Some(scroll) = layout.scroll {
        paint_scrolled(fb, theme, fonts, menu, style, layout, scroll);
        return;
    }
    let (c, m, p) = (&theme.colors, &theme.start_menu, &theme.popup);
    menu_frame(fb, c);
    // The etched line between two columns (Windows 95 and 2000), from the inner top to the
    // inner bottom: the shadow color, then the highlight.
    for &x in &layout.lines {
        let frame = layout.frame;
        fb.vline(x, frame, fb.height - 2 * frame, c.shadow);
        fb.vline(x + 1, frame, fb.height - 2 * frame, c.highlight);
    }
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

#[cfg(test)]
mod long_menu_tests {
    use super::*;
    use crate::text::Font;
    use crate::theme::{WIN95, WIN98, WIN2000};

    fn fonts() -> (Font, Font) {
        (
            Font::parse(include_str!("../tests/fixtures/ms-sans-serif-11px.font")).unwrap(),
            Font::parse(include_str!("../tests/fixtures/ms-sans-serif-11px-bold.font")).unwrap(),
        )
    }

    /// `folders` folders, then `n` entries "Entry 000" and on (the survey's test entries).
    fn long(folders: usize, n: usize) -> Menu {
        let mut entries: Vec<MenuEntry> = (0..folders).map(|i| MenuEntry::submenu(&format!("Folder {i}"), None)).collect();
        entries.extend((0..n).map(|i| MenuEntry::item(&format!("Entry {i:03}"), None)));
        Menu::new(entries)
    }

    /// Rows per column from the screen's height, as measured: Windows 95 27 rows of 22 at 600 and
    /// 21 at 480; Windows 2000 29 rows of 20 at 600 and 23 at 480 (survey part 1, the table).
    #[test]
    fn rows_per_column_from_the_screens_height() {
        let (r, b) = fonts();
        let f = Fonts { regular: &r, bold: &b };
        for (theme, h, rows) in [(&WIN95, 600, 27), (&WIN95, 480, 21), (&WIN2000, 600, 29), (&WIN2000, 480, 23)] {
            let l = layout_long_menu(theme, &f, &long(0, 110), MenuStyle::Cascade, h, Long::Columns { own_widths: true }, 0);
            let first = l.rects.iter().filter(|r| r.x == l.columns[0].0).count();
            assert_eq!(first, rows, "{h}");
            assert!(l.height <= h);
        }
    }

    /// A menu that just fits one column is laid out as before; one entry more goes on as the look
    /// says. Nothing changes for a menu that fits.
    #[test]
    fn a_menu_that_just_fits_and_one_entry_more() {
        let (r, b) = fonts();
        let f = Fonts { regular: &r, bold: &b };
        let fits = long(0, 29);
        let old = layout_menu(&WIN98, &f, &fits, MenuStyle::Cascade, 600);
        let new = layout_long_menu(&WIN98, &f, &fits, MenuStyle::Cascade, 600, Long::Scroll { flat: false }, 0);
        assert_eq!(old, new, "29 entries of 20 fit 600");
        assert_eq!(new.columns.len(), 1);
        let more = layout_long_menu(&WIN98, &f, &long(0, 30), MenuStyle::Cascade, 600, Long::Scroll { flat: false }, 0);
        let s = more.scroll.expect("30 entries scroll");
        assert_eq!((more.height, s.offset, s.end), (600, 0, 30 * 20 - (600 - 2 * more.frame)));
    }

    /// Each column as wide as its own widest entry, the 2-pixel etched line between them, filled
    /// down then across (Windows 95 and 2000).
    #[test]
    fn columns_are_each_as_wide_as_their_widest_entry() {
        let (r, b) = fonts();
        let f = Fonts { regular: &r, bold: &b };
        let mut menu = long(0, 40);
        menu.entries[3] = MenuEntry::item("A much longer entry in the first column", None);
        let l = layout_long_menu(&WIN95, &f, &menu, MenuStyle::Cascade, 600, Long::Columns { own_widths: true }, 0);
        assert_eq!(l.columns.len(), 2);
        let ((x0, w0), (x1, w1)) = (l.columns[0], l.columns[1]);
        assert!(w0 > w1, "the first column is wider: {w0} {w1}");
        assert_eq!(l.lines, vec![x0 + w0 + 1], "a pixel of the face, then the line");
        assert_eq!(x1, x0 + w0 + 4, "and a pixel after it");
        assert_eq!(l.width, x1 + w1 + l.frame);
        assert_eq!(l.rects[27], Rect::new(x1, l.frame, w1, 22), "the 28th entry tops the second column");
        let mut fb = Framebuffer::new(l.width, l.height, 0);
        paint_menu(&mut fb, &WIN95, &f, &menu, MenuStyle::Cascade, &l);
        let c = &WIN95.colors;
        assert_eq!((fb.get(x0 + w0 + 1, l.frame), fb.get(x0 + w0 + 2, l.height - l.frame - 1)), (c.shadow, c.highlight));
        // Windows 95's Left and Right go to the same row of the column beside.
        assert_eq!(l.beside(&menu, MenuStyle::Cascade, 5, 1), Some(27 + 5));
        assert_eq!(l.beside(&menu, MenuStyle::Cascade, 27 + 5, -1), Some(5));
        assert_eq!(l.beside(&menu, MenuStyle::Cascade, 20, 1), Some(39), "a shorter column: its last entry");
        assert_eq!(l.beside(&menu, MenuStyle::Cascade, 5, -1), None);
    }

    /// The scroll steps and their ends; the strips show only when there is more on their side; a
    /// key's entry is brought into view, just above the bottom strip (Windows 98, survey part 1, 3).
    #[test]
    fn a_long_menu_scrolls_between_its_strips() {
        let (r, b) = fonts();
        let f = Fonts { regular: &r, bold: &b };
        let menu = long(0, 37);
        let mut l = layout_long_menu(&WIN98, &f, &menu, MenuStyle::Cascade, 600, Long::Scroll { flat: false }, 0);
        let frame = l.frame;
        let mut s = l.scroll.unwrap();
        assert_eq!(s.end, 37 * 20 - (600 - 2 * frame));
        assert!(!s.shows(Strip::Up) && s.shows(Strip::Down), "at the top only the bottom strip");
        assert_eq!(l.view(), (frame, 600 - frame - STRIP_H));
        assert_eq!(l.strip_at(50, 600 - frame - 1), Some(Strip::Down));
        assert_eq!(l.strip_at(50, frame), None, "no top strip yet");
        assert_eq!(l.hit(&menu, MenuStyle::Cascade, 50, frame + 5), Some(0));
        // One step: one entry.
        assert!(s.step(Strip::Down, scroll_step(&s)));
        assert_eq!(s.offset, 20);
        // To the end: the last step lands on the end exactly, then nothing moves.
        while s.step(Strip::Down, 20) {}
        assert_eq!(s.offset, s.end);
        assert!(!s.shows(Strip::Down) && s.shows(Strip::Up));
        l.scroll = Some(s);
        assert_eq!(l.shown_rect(36).unwrap().bottom(), 600 - frame, "the last entry reaches the frame");
        assert_eq!(l.hit(&menu, MenuStyle::Cascade, 50, frame + 5), None, "under the top strip");
        // Back to the top by a key on the first entry; then down past the bottom: just above the strip.
        assert!(l.reveal(0));
        assert_eq!(l.scroll.unwrap().offset, 0);
        assert!(l.reveal(30));
        let shown = l.shown_rect(30).unwrap();
        assert_eq!(shown.bottom(), 600 - frame - STRIP_H, "just above the bottom strip");
        assert!(!l.reveal(30), "already in view");
        // The pace as plain numbers.
        assert_eq!(scroll_pace(false), (150, 300));
        assert_eq!(scroll_pace(true), (60, 60));
        // Painted: the bottom strip's arrow in the middle of the inner width (measured on 98: x
        // 227..231 in a menu at 165..294, the inner left plus (inner width - 6) / 2).
        let mut fb = Framebuffer::new(l.width, l.height, 0);
        let mut l2 = l.clone();
        l2.scroll = Some(Scroll { offset: 0, ..l.scroll.unwrap() });
        paint_menu(&mut fb, &WIN98, &f, &menu, MenuStyle::Cascade, &l2);
        let inner_w = l.width - 2 * frame;
        let ax = frame + (inner_w - 1 - 5) / 2;
        let row = 600 - frame - 1 - 3;
        assert_eq!(fb.get(ax + 2, row), WIN98.colors.text, "the down arrow's point");
        assert_eq!(fb.get(ax + 1, row), WIN98.colors.face);
        // Under the pointer (98): sunken, without its arrow.
        l2.scroll = Some(Scroll { hot: Some(Strip::Down), ..l2.scroll.unwrap() });
        paint_menu(&mut fb, &WIN98, &f, &menu, MenuStyle::Cascade, &l2);
        assert_eq!(fb.get(ax + 2, row), WIN98.colors.face, "no arrow under the pointer");
        assert_eq!(fb.get(frame, 600 - frame - STRIP_H), WIN98.colors.dark_shadow);
    }

    /// Left and Right land only on entries that can be chosen: a 95 Start-style menu in columns
    /// with separators (the review of brief 91, 10).
    #[test]
    fn beside_never_lands_on_a_separator() {
        let (r, b) = fonts();
        let f = Fonts { regular: &r, bold: &b };
        let mut entries: Vec<MenuEntry> = (0..40).map(|i| MenuEntry::item(&format!("Entry {i:03}"), None)).collect();
        // A separator in the second column's rows: in the row of entry 3.
        let l0 = layout_long_menu(&WIN95, &f, &Menu::new(entries.clone()), MenuStyle::StartSmall, 300, Long::Columns { own_widths: true }, 0);
        let per_col = l0.rects.iter().filter(|r| r.x == l0.columns[0].0).count();
        entries[per_col + 3] = MenuEntry::separator();
        let menu = Menu::new(entries);
        let l = layout_long_menu(&WIN95, &f, &menu, MenuStyle::StartSmall, 300, Long::Columns { own_widths: true }, 0);
        assert!(l.columns.len() > 1);
        let to = l.beside(&menu, MenuStyle::StartSmall, 3, 1).unwrap();
        assert!(menu.selectable(to, MenuStyle::StartSmall), "landed on {to}");
        assert_ne!(menu.entries[to].kind, EntryKind::Separator);
    }

    /// A layout whose entries overflow the screen by the padding only: whatever it gives, nothing
    /// scrolls and no strip shows (the review of brief 91, 11).
    #[test]
    fn a_scrolling_layout_longer_than_its_menu_paints_without_a_panic() {
        // Brief 94: a panic in the bar ends it, so a layout made for more entries than the menu
        // has paints what there is.
        let (r, b) = fonts();
        let f = Fonts { regular: &r, bold: &b };
        let l = layout_long_menu(&WIN98, &f, &long(0, 80), MenuStyle::Cascade, 300, Long::Scroll { flat: false }, 0);
        assert!(l.scroll.is_some());
        let mut fb = Framebuffer::new(l.width, l.height, 0);
        paint_menu(&mut fb, &WIN98, &f, &long(0, 3), MenuStyle::Cascade, &l);
    }

    #[test]
    fn overflowing_by_the_padding_only_shows_no_strip() {
        let (r, b) = fonts();
        let f = Fonts { regular: &r, bold: &b };
        let frame = WIN98.start_menu.frame;
        let row = WIN98.start_menu.small.h;
        // 29 rows exactly fill the inside; one row's padding more makes the old layout break.
        for h in [29 * row + 2 * frame, 29 * row + 2 * frame + WIN98.start_menu.bottom_pad - 1] {
            let menu = long(0, 29);
            let l = layout_long_menu(&WIN98, &f, &menu, MenuStyle::Cascade, h, Long::Scroll { flat: false }, 0);
            if let Some(s) = l.scroll {
                assert_eq!(s.end, 0, "{h}: an end of 0");
                assert!(!s.shows(Strip::Up) && !s.shows(Strip::Down), "{h}: no strip");
                assert_eq!(l.strip_at(l.width / 2, l.height - frame - 1), None);
                assert!(l.shown_rect(28).is_some(), "{h}: every entry shows");
            }
        }
    }

    /// An entry scrolled out of sight is brought into view whole before its submenu opens (the
    /// review of brief 91, 1: the core's part).
    #[test]
    fn an_entry_out_of_sight_is_brought_into_view() {
        let (r, b) = fonts();
        let f = Fonts { regular: &r, bold: &b };
        let mut menu = long(3, 60);
        menu.hover = Some(1);
        let mut l = layout_long_menu(&WIN98, &f, &menu, MenuStyle::Cascade, 600, Long::Scroll { flat: false }, 10_000);
        assert_eq!(l.shown_rect(1), None, "scrolled to the end: the second folder is out of sight");
        assert!(l.reveal(1));
        let shown = l.shown_rect(1).unwrap();
        let (top, bottom) = l.view();
        assert!(shown.y >= top && shown.bottom() <= bottom, "whole in view");
    }
    /// How long one repaint of a scrolling menu of 400 entries takes (a measurement, not a check;
    /// brief 92: run it with `cargo test -p bevel-core --release --lib scrolling_menu_timing --
    /// --ignored --nocapture`).
    #[test]
    #[ignore = "a measurement, not a check"]
    fn scrolling_menu_timing() {
        let (r, b) = fonts();
        let f = Fonts { regular: &r, bold: &b };
        let icon = Some(crate::fb::Icon { width: 16, height: 16, pixels: vec![0xFF00_80FF; 256] });
        let mut menu = Menu::new((0..400).map(|i| MenuEntry::item(&format!("Entry {i:03}"), icon.clone())).collect());
        menu.hover = Some(200);
        let mut l = layout_long_menu(&WIN98, &f, &menu, MenuStyle::Cascade, 600, Long::Scroll { flat: false }, 0);
        if let Some(s) = l.scroll.as_mut() {
            s.set(s.end / 2);
        }
        let mut times = Vec::new();
        for _ in 0..21 {
            let started = std::time::Instant::now();
            let mut fb = Framebuffer::new(l.width, l.height, 0);
            paint_menu(&mut fb, &WIN98, &f, &menu, MenuStyle::Cascade, &l);
            times.push(started.elapsed().as_secs_f64() * 1000.0);
            std::hint::black_box(fb);
        }
        times.sort_by(f64::total_cmp);
        println!("a scrolling menu of 400 entries: one repaint {:.3} ms (median of 21; fastest {:.3}, slowest {:.3})", times[10], times[0], times[20]);
    }
}
