//! Windows XP's taskbar (Luna, blue), Bevel95's XP bar style (brief 112): a first version, painted
//! from pictures cut from published screenshots (`bevel_art`'s `xp/` sheet; the survey and its
//! marks: `private/probes/xp/survey.md`). Laid out into the same `BarLayout` the classic bar
//! fills, so every click, drag and tooltip of the bar works as before.
//!
//! What no screenshot gives is PROVISIONAL and listed in decisions.md ("Windows XP: the taskbar
//! and Start menu"): nothing changes under the pointer; a task button held down, and a flashing
//! one's other half, show the button in front; the scroll arrows of a full bar are the 2000 look's.

use std::cell::Cell;

use crate::art::{self, Look};
use crate::fb::{Framebuffer, Icon, Rect};
use crate::layout::{self, BarLayout};
use crate::paint::{Bar, TaskState};
use crate::text::{Font, Fonts};
use crate::theme::{Theme, WINXP};

const SHEET: &[u8] = art::bytes("xp/sheet", Look::WinXp);
const TABLE: &[(&str, usize, i32, i32)] = art::places("xp/places", Look::WinXp);

thread_local! {
    /// How many XP pictures this thread has read (a test proves the classic bar reads none).
    static READ: Cell<usize> = const { Cell::new(0) };
}

/// How many XP pictures this thread has read so far.
pub fn pictures_read() -> usize {
    READ.with(Cell::get)
}

/// The XP picture called `name` (`None`: no such picture).
pub fn picture(name: &str) -> Option<Icon> {
    READ.with(|r| r.set(r.get() + 1));
    let &(_, at, w, h) = TABLE.iter().find(|(n, ..)| *n == name)?;
    let bytes = SHEET.get(at..at + (w * h * 4) as usize)?;
    let pixels = bytes.chunks(4).map(|p| u32::from(p[3]) << 24 | u32::from(p[0]) << 16 | u32::from(p[1]) << 8 | u32::from(p[2])).collect();
    Some(Icon { width: w, height: h, pixels })
}

/// The column of source `pic` that target column `x` of a part `w` wide takes, its `left` and
/// `right` columns kept and the columns between stretched over the rest (each source column over
/// an equal share, the first ones taking the larger shares: the survey's rule for a task button's
/// middle, "the first taking the larger half").
pub fn sliced_column(src_w: i32, left: i32, right: i32, w: i32, x: i32) -> i32 {
    if x < left {
        return x;
    }
    if x >= w - right {
        return src_w - (w - x);
    }
    let (n, k) = (w - left - right, src_w - left - right);
    if n <= 0 || k <= 0 {
        return left.min(src_w - 1);
    }
    // Column j of n goes to source column j·k/n rounded up at the shares' borders, so that the
    // earlier columns take the extra pixel: the first ceil(n/k) to column 0 and so on.
    let j = x - left;
    let per = n / k;
    let extra = n % k;
    // The first `extra` source columns cover per + 1 target columns each.
    let big = extra * (per + 1);
    let s = if j < big { j / (per + 1) } else { extra + (j - big) / per.max(1) };
    left + s.min(k - 1)
}

/// Paints `pic` over `r` (as tall as the picture), its `left` and `right` columns as they are and
/// the columns between stretched to fill the width (`sliced_column`). The picture's alpha is
/// blended as `draw_icon` blends it.
pub fn paint_sliced(fb: &mut Framebuffer, pic: &Icon, r: Rect, left: i32, right: i32) {
    for x in 0..r.w {
        let sx = sliced_column(pic.width, left, right, r.w, x);
        for y in 0..r.h.min(pic.height) {
            let p = pic.pixels[(y * pic.width + sx) as usize];
            blend(fb, r.x + x, r.y + y, p);
        }
    }
}

/// One pixel of straight alpha over the framebuffer.
fn blend(fb: &mut Framebuffer, x: i32, y: i32, p: u32) {
    let a = p >> 24;
    if a == 0 || x < 0 || y < 0 || x >= fb.width || y >= fb.height {
        return;
    }
    if a == 255 {
        fb.set(x, y, p);
        return;
    }
    let under = fb.get(x, y);
    let mix = |s: u32| {
        let (f, b) = ((p >> s) & 255, (under >> s) & 255);
        ((f * a + b * (255 - a) + 127) / 255) << s
    };
    fb.set(x, y, 0xFF00_0000 | mix(16) | mix(8) | mix(0));
}

/// The Start button: x 0 to 97, the bar's whole height (its picture is 100 wide with two columns
/// of the bar behind it).
pub const START_W: i32 = 98;
/// The notification area: the icons from 11 in, 18 apart, 7 down; the clock's part after them,
/// as wide as the widest time plus `CLOCK_EXTRA` (one icon and "12:00 AM": 93, the survey's).
pub const TRAY_ICON_X: i32 = 11;
pub const TRAY_ICON_Y: i32 = 7;
pub const TRAY_STEP: i32 = 18;
pub const CLOCK_EXTRA: i32 = 18;
/// The labels' and the clock's baseline (bar rows; the survey: the clock's digits on rows 11-18).
pub const BASELINE: i32 = 19;
/// A task button's icon 10 in and 4 down, its label 30 in (the survey's).
const TASK_ICON_X: i32 = 10;
const TASK_ICON_Y: i32 = 4;
const TASK_LABEL_X: i32 = 30;
/// Room kept right of a label (PROVISIONAL: no screenshot shows a cut label's end exactly).
const TASK_LABEL_RIGHT: i32 = 6;

/// The notification area for `icons` icons and the clock (`layout::TrayWell`): right-aligned,
/// the bar's whole height.
fn tray(fonts: &Fonts, width: i32, icons: usize, number: Option<usize>, clock: &str, widest: &str) -> layout::TrayWell {
    let h = WINXP.bar.height;
    if clock.is_empty() && icons == 0 {
        let empty = Rect::new(width, 0, 0, h);
        return layout::TrayWell { well: empty, tray: Vec::new(), clock: empty, number: None };
    }
    let number = number.filter(|&k| k < icons);
    let number_w = number.map_or(0, |_| layout::number_part_width(fonts));
    // Without a clock, 6 after the icons (PROVISIONAL: not seen).
    let clock_w = if clock.is_empty() { 6 } else { fonts.regular.measure(widest).max(fonts.regular.measure(clock)) + CLOCK_EXTRA };
    let w = TRAY_ICON_X + icons as i32 * TRAY_STEP + number_w + clock_w;
    let x = width - w;
    let after_number = |i: usize| if number.is_some_and(|k| i > k) { number_w } else { 0 };
    let tray = (0..icons).map(|i| Rect::new(x + TRAY_ICON_X + i as i32 * TRAY_STEP + after_number(i), TRAY_ICON_Y, 16, 16)).collect();
    let number = number.map(|k| (k, Rect::new(x + TRAY_ICON_X + (k as i32 + 1) * TRAY_STEP, 1, number_w, h - 2)));
    let clock = Rect::new(x + TRAY_ICON_X + icons as i32 * TRAY_STEP + number_w, 1, clock_w, h - 2);
    layout::TrayWell { well: Rect::new(x, 0, w, h), tray, clock, number }
}

/// Where everything of `bar` goes on an XP bar `width` wide. Quick Launch is not shown (XP shipped
/// with it hidden).
pub fn bar_layout(theme: &Theme, fonts: &Fonts, width: i32, bar: &Bar) -> BarLayout {
    let number = bar.tray_number.as_ref().filter(|(_, text)| !text.is_empty()).map(|&(i, _)| i);
    let start = Rect::new(0, 0, START_W, theme.bar.height);
    let w = tray(fonts, width, bar.tray.len(), number, &bar.clock, &bar.clock_widest);
    let (tasks, first_task, scroll) = layout::task_buttons(theme, start, w.well, bar.tasks.len(), bar.row);
    BarLayout { start, tasks, first_task, scroll, well: w.well, tray: w.tray, clock: w.clock, number: w.number, quick: None, task_band: None }
}

/// Paints the XP bar (the framebuffer 30 tall) and returns where everything went.
pub fn paint_bar(fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts, bar: &Bar) -> BarLayout {
    let layout = bar_layout(theme, fonts, fb.width, bar);
    if let Some(column) = picture("column") {
        paint_sliced(fb, &column, Rect::new(0, 0, fb.width, column.height), 0, 0);
    }
    if let Some(start) = picture(if bar.start_pressed { "start_pressed" } else { "start_normal" }) {
        fb.draw_icon(&start, 0, 0);
    }
    let shown: Vec<(usize, &crate::paint::Task, Rect)> = bar
        .tasks
        .iter()
        .skip(layout.first_task)
        .zip(&layout.tasks)
        .enumerate()
        .map(|(i, (task, &r))| (i, task, Rect { x: bar.task_x.get(i).copied().unwrap_or(r.x), ..r }))
        .collect();
    let on_top = |i: usize| bar.task_on_top == Some(i);
    for &(_, task, r) in shown.iter().filter(|s| !on_top(s.0)).chain(shown.iter().filter(|s| on_top(s.0))) {
        paint_task(fb, fonts, r, task);
    }
    if let Some(s) = layout.scroll {
        crate::paint::paint_scroll_arrows(fb, theme, s, bar.scroll_pressed);
    }
    paint_tray(fb, theme, fonts, &layout, bar);
    layout
}

/// A task button: plain, or in front (also for a flashing one: `TaskState::Flashing` is the half
/// of its flashing that shows in front), the icon and the white label.
fn paint_task(fb: &mut Framebuffer, fonts: &Fonts, r: Rect, task: &crate::paint::Task) {
    let front = task.state != TaskState::Normal;
    let names = if front { ["task_front_left", "task_front_mid", "task_front_right"] } else { ["task_plain_left", "task_plain_mid", "task_plain_right"] };
    let parts: Vec<Icon> = names.iter().filter_map(|n| picture(n)).collect();
    if let [left, mid, right] = parts.as_slice() {
        // One picture of the three side by side, sliced: 15 left, the middle stretched, 4 right.
        let mut whole = Icon { width: left.width + mid.width + right.width, height: left.height, pixels: Vec::new() };
        for y in 0..left.height {
            for p in [left, mid, right] {
                whole.pixels.extend_from_slice(&p.pixels[(y * p.width) as usize..((y + 1) * p.width) as usize]);
            }
        }
        paint_sliced(fb, &whole, r, left.width, right.width);
    }
    let x = match &task.icon {
        Some(icon) => {
            fb.draw_icon(icon, r.x + TASK_ICON_X, r.y + TASK_ICON_Y);
            r.x + TASK_LABEL_X
        }
        None => r.x + TASK_ICON_X,
    };
    let font = fonts.regular;
    let text = font.elide(&task.title, r.right() - TASK_LABEL_RIGHT - x);
    label(fb, font, x, &text);
}

fn label(fb: &mut Framebuffer, font: &Font, x: i32, text: &str) {
    font.draw(fb, x, BASELINE - font.ascent, text, crate::theme::rgb(0xFFFFFF));
}

/// The notification area: its picture (23 left, the stretched fill, 20 right), the icons, the
/// white clock.
fn paint_tray(fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts, layout: &BarLayout, bar: &Bar) {
    if layout.well.w == 0 {
        return;
    }
    let parts: Vec<Icon> = ["tray_left", "tray_fill", "tray_right"].iter().filter_map(|n| picture(n)).collect();
    if let [left, fill, right] = parts.as_slice() {
        let mut whole = Icon { width: left.width + fill.width + right.width, height: left.height, pixels: Vec::new() };
        for y in 0..left.height {
            for p in [left, fill, right] {
                whole.pixels.extend_from_slice(&p.pixels[(y * p.width) as usize..((y + 1) * p.width) as usize]);
            }
        }
        let w = layout.well.w.max(left.width + right.width);
        paint_sliced(fb, &whole, Rect::new(layout.well.x, layout.well.y, w, layout.well.h), left.width, right.width);
    }
    for (icon, r) in bar.tray.iter().zip(&layout.tray) {
        if let Some(icon) = icon {
            fb.draw_icon(icon, r.x, r.y);
        }
    }
    if !bar.clock.is_empty() {
        let x = layout::clock_text_x(fonts, layout.clock, &bar.clock);
        fonts.regular.draw(fb, x, BASELINE - fonts.regular.ascent, &bar.clock, theme.colors.clock_text);
    }
    if let (Some((_, part)), Some((_, text))) = (layout.number, &bar.tray_number) {
        fonts.regular.draw(fb, part.x + layout::NUMBER_PAD, BASELINE - fonts.regular.ascent, text, theme.colors.clock_text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The slicing: the left and right columns as they are, the middle shared out, the first
    /// source columns taking the larger shares; a part exactly as wide as the picture is the
    /// picture.
    #[test]
    fn slicing_keeps_the_ends_and_shares_the_middle() {
        // A task button: 15 + 2 + 4 source columns.
        let col = |w, x| sliced_column(21, 15, 4, w, x);
        assert_eq!((0..15).map(|x| col(160, x)).collect::<Vec<_>>(), (0..15).collect::<Vec<_>>());
        assert_eq!((156..160).map(|x| col(160, x)).collect::<Vec<_>>(), vec![17, 18, 19, 20]);
        // 160 wide: 141 middle columns, 71 of the first source column and 70 of the second.
        let mid: Vec<i32> = (15..156).map(|x| col(160, x)).collect();
        assert_eq!(mid.iter().filter(|&&s| s == 15).count(), 71);
        assert_eq!(mid.iter().filter(|&&s| s == 16).count(), 70);
        assert!(mid.windows(2).all(|w| w[0] <= w[1]), "in order");
        // 147 wide (four buttons at 800×600): 64 and 64.
        let mid: Vec<i32> = (15..143).map(|x| col(147, x)).collect();
        assert_eq!((mid.iter().filter(|&&s| s == 15).count(), mid.iter().filter(|&&s| s == 16).count()), (64, 64));
        // As wide as the picture: the picture.
        assert_eq!((0..21).map(|x| col(21, x)).collect::<Vec<_>>(), (0..21).collect::<Vec<_>>());
        // One column stretched (the bar's column): always it.
        assert!((0..800).all(|x| sliced_column(1, 0, 0, 800, x) == 0));
    }

    fn fonts_and<R>(f: impl FnOnce(&Fonts) -> R) -> R {
        let regular = Font::parse(include_str!("../tests/fixtures/tahoma-11px.font")).unwrap();
        let bold = Font::parse(include_str!("../tests/fixtures/tahoma-11px-bold.font")).unwrap();
        f(&Fonts { regular: &regular, bold: &bold })
    }

    fn bar(tasks: usize, icons: usize) -> Bar {
        let task = crate::paint::Task { title: "Window".into(), icon: None, state: TaskState::Normal };
        Bar { tasks: vec![task; tasks], tray: vec![None; icons], clock: "10:58 AM".into(), clock_widest: "12:00 AM".into(), ..Bar::default() }
    }

    /// The bar's layout as the survey measured it at 800×600, and at other widths and counts:
    /// the Start button 98 wide; buttons from 106, 160 wide while there is room, no gap; four of
    /// them 147 (desktop-full); the notification area 93 wide with one icon, 18 more per icon.
    #[test]
    fn the_bar_is_laid_out_as_the_screenshots() {
        fonts_and(|fonts| {
            let l = bar_layout(&WINXP, fonts, 800, &bar(3, 1));
            assert_eq!(l.start, Rect::new(0, 0, 98, 30));
            assert_eq!(l.tasks, vec![Rect::new(106, 3, 160, 25), Rect::new(266, 3, 160, 25), Rect::new(426, 3, 160, 25)]);
            assert_eq!(l.well, Rect::new(707, 0, 93, 30));
            assert_eq!(l.tray, vec![Rect::new(718, 7, 16, 16)]);
            let l = bar_layout(&WINXP, fonts, 800, &bar(4, 1));
            assert_eq!(l.tasks.iter().map(|r| (r.x, r.w)).collect::<Vec<_>>(), [(106, 147), (253, 147), (400, 147), (547, 147)]);
            let l = bar_layout(&WINXP, fonts, 800, &bar(0, 2));
            assert_eq!(l.well, Rect::new(689, 0, 111, 30), "two icons: 111 (the survey's 16-bit shot)");
            // Wider screens: still 160 at most; many buttons share the room, then the arrows.
            let l = bar_layout(&WINXP, fonts, 1920, &bar(5, 1));
            assert!(l.tasks.iter().all(|r| r.w == 160) && l.scroll.is_none());
            let l = bar_layout(&WINXP, fonts, 800, &bar(40, 1));
            assert!(l.scroll.is_some() && l.tasks.iter().all(|r| r.right() <= l.well.x));
            assert!(l.quick.is_none() && l.task_band.is_none());
        });
    }

    /// A classic bar never reads an XP picture; the XP bar does.
    #[test]
    fn a_classic_bar_asks_no_xp_picture() {
        fonts_and(|fonts| {
            let before = pictures_read();
            for theme in [&crate::theme::WIN95, &crate::theme::WIN98, &crate::theme::WIN2000] {
                let mut fb = Framebuffer::new(640, theme.bar.height, 0);
                crate::paint::paint_bar(&mut fb, theme, fonts, &bar(3, 1));
            }
            assert_eq!(pictures_read(), before, "no XP picture for a classic bar");
            let mut fb = Framebuffer::new(800, 30, 0);
            crate::paint::paint_bar(&mut fb, &WINXP, fonts, &bar(3, 1));
            assert!(pictures_read() > before);
        });
    }

    /// settings.txt's `bar_style`: classic unless it says xp; a word it does not know is warned
    /// about and leaves the default.
    #[test]
    fn the_style_is_read_from_the_settings() {
        use crate::theme::BarStyle;
        assert_eq!(crate::config::parse("").0.bar_style, BarStyle::Classic);
        assert_eq!(crate::config::parse("bar_style = xp\n").0.bar_style, BarStyle::Xp);
        assert_eq!(crate::config::parse("bar_style = classic\n").0.bar_style, BarStyle::Classic);
        let (c, warnings) = crate::config::parse("bar_style = vista\n");
        assert_eq!(c.bar_style, BarStyle::Classic);
        assert!(!warnings.is_empty());
        assert_eq!(BarStyle::from_setting(BarStyle::Xp.setting()), Some(BarStyle::Xp));
        assert_eq!(crate::theme::bar_theme(crate::theme::Era::Win95, crate::theme::Depth::Sixteen, BarStyle::Xp).slug, "winxp");
        assert_eq!(crate::theme::bar_theme(crate::theme::Era::Win95, crate::theme::Depth::Sixteen, BarStyle::Classic).slug, "win95");
    }

    #[test]
    fn every_xp_picture_is_there() {
        for name in [
            "column", "start_normal", "start_pressed", "task_plain_left", "task_plain_mid", "task_plain_right", "task_front_left",
            "task_front_mid", "task_front_right", "tray_left", "tray_fill", "tray_right",
        ] {
            let p = picture(name).unwrap_or_else(|| panic!("{name}"));
            assert_eq!(p.pixels.len() as i32, p.width * p.height, "{name}");
        }
        assert_eq!(picture("column").unwrap().height, 30);
    }
}
