//! Where things go on the bar: pure arithmetic on the theme and text widths.

use crate::fb::Rect;
use crate::text::Fonts;
use crate::theme::{SubmenuPlacement, TaskMetrics, Theme};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BarLayout {
    /// The Start button (outer edge of its bevel).
    pub start: Rect,
    /// The task buttons shown (outer edge of their bevel, without the gap after them): all of
    /// them while they fit, else those of the row scrolled to.
    pub tasks: Vec<Rect>,
    /// Which task the first of `tasks` is (above 0 when scrolled down a row).
    pub first_task: usize,
    /// The scroll arrows, while the buttons need more than one row.
    pub scroll: Option<TaskScroll>,
    /// The sunken well of the notification area (outer edge of its 1px border).
    pub well: Rect,
    /// The notification icons in it (16×16 each), left to right.
    pub tray: Vec<Rect>,
    /// The clock's part of the well, from its left margin to its right one (empty without a
    /// clock): hovering it shows the date.
    pub clock: Rect,
}

impl BarLayout {
    /// The notification icon whose slot (the icon, the gap after it, the well's full inside
    /// height) holds (x, y).
    pub fn tray_icon_at(&self, theme: &Theme, x: i32, y: i32) -> Option<usize> {
        let inside = self.well.inset(1, 1, 1, 1);
        self.tray.iter().position(|r| Rect::new(r.x, inside.y, theme.tray.icon_step, inside.h).contains(x, y))
    }
}

/// The two small arrow buttons that page through the rows of task buttons when they do not
/// fit on one, stacked at the right end of the task area (Windows' up-down control).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TaskScroll {
    pub up: Rect,
    pub down: Rect,
    /// The row shown (0 = the first), how many rows there are, and buttons per full row.
    pub row: usize,
    pub rows: usize,
    pub per_row: usize,
}

/// The bar for `task_count` buttons and `tray_count` notification icons, showing row `row` of
/// the buttons if they need several rows (clamped to the last row). `widest` is the widest
/// time of day (see `tray_well`).
#[allow(clippy::too_many_arguments)]
pub fn bar(
    theme: &Theme,
    fonts: &Fonts,
    width: i32,
    task_count: usize,
    tray_count: usize,
    clock: &str,
    widest: &str,
    row: usize,
) -> BarLayout {
    let start = start_button(theme, fonts);
    let (well, tray, clock) = tray_well(theme, fonts, width, tray_count, clock, widest);
    let (tasks, first_task, scroll) = task_buttons(theme, start, well, task_count, row);
    BarLayout { start, tasks, first_task, scroll, well, tray, clock }
}

/// The row that task `index` is on in `layout`'s bar.
pub fn row_of(layout: &BarLayout, index: usize) -> usize {
    layout.scroll.map_or(0, |s| index / s.per_row.max(1))
}

/// The Start button: as wide as its contents (bevel, padding, flag, gap, bold label).
pub fn start_button(theme: &Theme, fonts: &Fonts) -> Rect {
    let s = &theme.start;
    let label = fonts.bold.measure(s.label);
    let w = 2 + s.pad_x + s.icon + s.icon_gap + label + s.label_margin_right + s.pad_x + 2;
    Rect::new(s.margin_left, theme.bar.item_y(), w, theme.bar.item_h())
}

/// The notification area's well, right-aligned and as wide as its contents: border, padding,
/// `icons` notification icons, then the clock's part, as wide as `widest` (the widest time of
/// day; the current `clock` if that is wider) plus the theme's extra. Returns the well, the
/// icons' rectangles and the clock's part. Without a clock (Show Clock off) and without icons
/// there is nothing to hold, and Windows leaves the well out: an empty rectangle at the right
/// edge.
pub fn tray_well(
    theme: &Theme,
    fonts: &Fonts,
    width: i32,
    icons: usize,
    clock: &str,
    widest: &str,
) -> (Rect, Vec<Rect>, Rect) {
    let t = &theme.tray;
    let (y, h) = (theme.bar.item_y(), theme.bar.item_h());
    if clock.is_empty() && icons == 0 {
        return (Rect::new(width, y, 0, h), Vec::new(), Rect::new(width, y, 0, h));
    }
    // PROVISIONAL without a clock: the icons, then their gap.
    let clock_x = if icons == 0 { t.clock_x } else { t.icon_x + icons as i32 * t.icon_step + t.icons_gap };
    let clock_w = if clock.is_empty() {
        0
    } else {
        fonts.regular.measure(widest).max(fonts.regular.measure(clock)) + t.clock_extra
    };
    let w = 1 + t.pad + clock_x + clock_w + t.pad + 1;
    let well = Rect::new(width - t.margin_x - w, y, w, h);
    let inside_x = well.x + 1 + t.pad;
    let icon_y = well.y + (well.h - TRAY_ICON) / 2;
    let tray =
        (0..icons).map(|i| Rect::new(inside_x + t.icon_x + i as i32 * t.icon_step, icon_y, TRAY_ICON, TRAY_ICON)).collect();
    let clock = Rect::new(inside_x + clock_x, well.y + 1, clock_w, well.h - 2);
    (well, tray, clock)
}

/// Where the clock's text starts: centered in the clock's part, rounded to the left.
pub fn clock_text_x(fonts: &Fonts, clock_part: Rect, clock: &str) -> i32 {
    clock_part.x + (clock_part.w - fonts.regular.measure(clock)).max(0) / 2
}

/// Notification icons are 16×16, as Windows' small icons.
pub const TRAY_ICON: i32 = 16;

/// Task buttons in the space between the Start button and the clock well, as Windows 98's
/// taskbar lays them out (watched in the v86 emulator; RetroBar differs, see
/// docs/theme-spec.md): every button gets the same width, `slot_width`. When even the
/// narrowest buttons do not fit, the scroll arrows take the right end of the space and the
/// buttons wrap into rows of as many as fit; row `row` is shown. Returns the buttons shown,
/// the index of the first of them, and the arrows.
pub fn task_buttons(
    theme: &Theme,
    start: Rect,
    well: Rect,
    count: usize,
    row: usize,
) -> (Vec<Rect>, usize, Option<TaskScroll>) {
    let t = &theme.task;
    let (y, h) = (theme.bar.item_y(), theme.bar.item_h());
    let left = start.right() + t.gap_after_start;
    let right = well.x - theme.tray.margin_x;
    let button = |i: usize, slot: i32| Rect::new(left + i as i32 * slot, y, slot - t.spacing, h);
    if count == 0 {
        return (Vec::new(), 0, None);
    }
    if let Some(slot) = slot_width(t, right - left, count) {
        return ((0..count).map(|i| button(i, slot)).collect(), 0, None);
    }
    let arrows_x = right - t.scroll_w;
    let slot = t.min_width + t.spacing;
    let per_row = (((arrows_x - left) / slot).max(1)) as usize;
    let rows = count.div_ceil(per_row);
    let row = row.min(rows - 1);
    let first = row * per_row;
    let shown = per_row.min(count - first);
    let up_h = h / 2;
    let scroll = TaskScroll {
        up: Rect::new(arrows_x, y, t.scroll_w, up_h),
        down: Rect::new(arrows_x, y + up_h, t.scroll_w, h - up_h),
        row,
        rows,
        per_row,
    };
    ((0..shown).map(|i| button(i, slot)).collect(), first, Some(scroll))
}

/// The width of every task slot (button plus the gap after it) for `count` buttons sharing
/// `avail` pixels, or `None` when they do not fit even at their narrowest. All buttons are
/// equally wide, as in Windows: the default (160 + 3) while there is room, else the floored
/// share (the pixels left over stay empty at the end), down to the narrowest (22 + 3, just
/// the icon).
pub fn slot_width(t: &TaskMetrics, avail: i32, count: usize) -> Option<i32> {
    let share = avail / (count.max(1) as i32);
    (share >= t.min_width + t.spacing).then(|| share.min(t.width + t.spacing))
}

/// Where a popup menu `w`×`h` opens for a click at (x, y), the way Windows' TrackPopupMenu
/// does it: top-left corner at the click, flipped upward or leftward when it would run off
/// `screen`, then kept on screen. Works in any units (the platform uses screen pixels).
pub fn place_popup(x: i32, y: i32, w: i32, h: i32, screen: Rect) -> (i32, i32) {
    let x = if x + w > screen.right() { x - w } else { x };
    let y = if y + h > screen.bottom() { y - h } else { y };
    (x.min(screen.right() - w).max(screen.x), y.min(screen.bottom() - h).max(screen.y))
}

/// Where a submenu `w`×`h` opens beside the entry `entry` of its parent menu (all in screen
/// units, with `frame` and `rule` scaled to them): to the right, overlapping the parent by
/// `rule.overlap`, its first item level with the entry (its frame just above); to the left
/// when it would run off `screen`. When it does not fit below, Windows 95 opens it upward,
/// its bottom level with the entry's bottom, if that fits; otherwise (and in Windows 2000's
/// Start menu always) it moves up just enough. Checked against GUIdebook's and Toasty Tech's
/// screenshots (docs/theme-spec.md, "Menus").
pub fn place_submenu(
    parent: Rect,
    entry: Rect,
    w: i32,
    h: i32,
    frame: i32,
    rule: SubmenuPlacement,
    screen: Rect,
) -> (i32, i32) {
    let right = parent.right() - rule.overlap;
    let x = if right + w <= screen.right() { right } else { parent.x + rule.overlap - w };
    let below = entry.y - frame;
    let upward = entry.bottom() + frame - h;
    let y = if below + h <= screen.bottom() {
        below
    } else if rule.flip_up && upward >= screen.y {
        upward
    } else {
        screen.bottom() - rule.margin_bottom - h
    };
    (x.min(screen.right() - w).max(screen.x), y.max(screen.y))
}

/// Where the Start menu `w`×`h` opens: its left edge level with the Start button's, its
/// bottom edge right on top of the button (over the bar's top edge), as in Windows 95 and
/// 2000; kept on `screen`.
pub fn place_start_menu(button: Rect, w: i32, h: i32, screen: Rect) -> (i32, i32) {
    let (x, y) = (button.x, button.y - h);
    (x.min(screen.right() - w).max(screen.x), y.min(screen.bottom() - h).max(screen.y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::WIN95;

    const WIN95_RULE: SubmenuPlacement = SubmenuPlacement { overlap: 6, flip_up: true, margin_bottom: 0 };
    const WIN2000_RULE: SubmenuPlacement = SubmenuPlacement { overlap: 6, flip_up: false, margin_bottom: 1 };

    #[test]
    fn submenus_open_to_the_right_level_with_their_entry() {
        let screen = Rect::new(0, 0, 640, 480);
        // Windows 95 Start menu (164 wide at x=2) and its Programs entry: the Programs menu
        // opens at x = 2 + 164 - 6, its frame 3px above the entry (GUIdebook screenshot).
        let parent = Rect::new(2, 217, 164, 239);
        let entry = Rect::new(26, 220, 137, 32);
        assert_eq!(place_submenu(parent, entry, 167, 138, 3, WIN95_RULE, screen), (160, 217));
    }

    #[test]
    fn windows_95_opens_upward_when_there_is_no_room_below() {
        // The desktop menu's "New" submenu (Toasty Tech screenshot): bottom level with the entry.
        let screen = Rect::new(0, 0, 640, 480);
        let parent = Rect::new(257, 273, 118, 152);
        let entry = Rect::new(260, 379, 112, 17);
        assert_eq!(place_submenu(parent, entry, 178, 113, 3, WIN95_RULE, screen), (369, 286));
        // Too tall to open upward either: moved up to end at the screen's bottom edge.
        let entry = Rect::new(163, 220, 161, 22);
        assert_eq!(place_submenu(Rect::new(160, 217, 167, 138), entry, 174, 292, 3, WIN95_RULE, screen), (321, 188));
    }

    #[test]
    fn windows_2000_moves_up_just_enough() {
        // GUIdebook: the Accessories menu ends one row above the bottom of the 800×600 screen.
        let screen = Rect::new(0, 0, 800, 600);
        let entry = Rect::new(175, 341, 131, 20);
        assert_eq!(place_submenu(Rect::new(172, 338, 137, 138), entry, 140, 286, 3, WIN2000_RULE, screen), (303, 313));
    }

    #[test]
    fn submenus_flip_left_at_the_edge() {
        let screen = Rect::new(0, 0, 640, 480);
        let parent = Rect::new(560, 300, 80, 40);
        let entry = Rect::new(563, 310, 74, 18);
        assert_eq!(place_submenu(parent, entry, 60, 50, 3, WIN95_RULE, screen), (506, 307));
    }

    #[test]
    fn start_menu_sits_on_the_button() {
        let screen = Rect::new(0, 0, 640, 480);
        assert_eq!(place_start_menu(Rect::new(2, 456, 54, 22), 164, 239, screen), (2, 217));
    }

    #[test]
    fn popups_open_down_and_right_when_they_fit() {
        let screen = Rect::new(0, 0, 640, 480);
        assert_eq!(place_popup(100, 100, 50, 40, screen), (100, 100));
    }

    #[test]
    fn popups_flip_up_from_the_bar_and_left_at_the_edge() {
        let screen = Rect::new(0, 0, 640, 480);
        assert_eq!(place_popup(300, 470, 50, 40, screen), (300, 430));
        assert_eq!(place_popup(620, 470, 50, 40, screen), (570, 430));
    }

    #[test]
    fn popups_stay_on_screen() {
        let screen = Rect::new(-100, 0, 200, 100);
        assert_eq!(place_popup(-95, 10, 50, 120, screen), (-95, 0));
        assert_eq!(place_popup(0, 0, 500, 20, screen), (-100, 0));
    }

    #[test]
    fn roomy_bar_uses_default_width() {
        assert_eq!(slot_width(&WIN95.task, 600, 3), Some(163));
    }

    #[test]
    fn crowded_bar_shares_the_row_equally() {
        // Windows 98 in v86: 9 buttons in 846 pixels are 94 wide each; 22 in 792..813 are 36.
        assert_eq!(slot_width(&WIN95.task, 846, 9), Some(94));
        assert_eq!(slot_width(&WIN95.task, 800, 22), Some(36));
        assert_eq!(slot_width(&WIN95.task, 517, 9), Some(57)); // the 4 pixels left over stay empty
    }

    #[test]
    fn overfull_bar_needs_scrolling() {
        assert_eq!(slot_width(&WIN95.task, 25 * 3, 3), Some(25));
        assert_eq!(slot_width(&WIN95.task, 25 * 3 - 1, 3), None);
    }

    /// A bar like the emulated Windows 98 one: task area from x = 69 to 916.
    fn crowded(count: usize, row: usize) -> (Vec<Rect>, usize, Option<TaskScroll>) {
        let start = Rect::new(2, 4, 63, 22); // right() = 65, so the tasks start at 69
        let well = Rect::new(918, 4, 100, 22); // the task area ends 2 before it, at 916
        task_buttons(&WIN95, start, well, count, row)
    }

    #[test]
    fn overfull_bar_shows_one_row_and_the_arrows() {
        // As in v86: 33 buttons of 22 (slots of 25) from x = 69, the arrows at 900..915,
        // split 11 + 11 here (the emulated bar's row was 23 high: 11 + 12).
        let (tasks, first, scroll) = crowded(38, 0);
        let scroll = scroll.expect("arrows");
        assert_eq!((tasks.len(), first, tasks[0], tasks[32]), (33, 0, Rect::new(69, 4, 22, 22), Rect::new(869, 4, 22, 22)));
        assert_eq!((scroll.up, scroll.down), (Rect::new(900, 4, 16, 11), Rect::new(900, 15, 16, 11)));
        assert_eq!((scroll.row, scroll.rows, scroll.per_row), (0, 2, 33));
    }

    #[test]
    fn scrolling_down_shows_the_rest() {
        let (tasks, first, scroll) = crowded(38, 1);
        assert_eq!((tasks.len(), first, scroll.map(|s| s.row)), (5, 33, Some(1)));
        assert_eq!(tasks[0].x, 69);
        // Past the last row: stays on the last row.
        assert_eq!(crowded(38, 7).1, 33);
    }

    #[test]
    fn fitting_bar_has_no_arrows() {
        let (tasks, first, scroll) = crowded(22, 3);
        assert_eq!((tasks.len(), first, scroll), (22, 0, None));
        assert_eq!(tasks[1].x - tasks[0].x, 38);
    }

    #[test]
    fn no_tasks_no_buttons() {
        assert_eq!(crowded(0, 0), (Vec::new(), 0, None));
    }
}
