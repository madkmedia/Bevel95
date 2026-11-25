//! Windows 7 Basic's taskbar in the bar (brief 114): the bar's own data (`paint::Bar`) put into
//! `seven`'s slots and painted by `seven::paint`, laid out into the same `BarLayout` the classic
//! bar fills, so the bar's clicks, menus and tooltips work as before.
//!
//! The slots, as Windows 7 shipped (large icons, always combined, no labels): first the pinned
//! programs (Quick Launch's shortcuts, in their order), a running one framed in its own place;
//! then every running program that is not pinned, in the order of its first window. A program
//! whose window is in front gets the lighter frame; a flashing one the lighter frame in the half
//! of its flashing that shows (the platform alternates at the classic rate: PROVISIONAL).
//!
//! When room is short (the review of brief 114, finding 1): a running program is never hidden in
//! favour of a pinned shortcut that is not running: those go first, from the right; if the running
//! programs still do not fit, they go into rows with the 2000 look's scroll arrows at the right
//! end of the room, as the XP bar's full state (PROVISIONAL: Windows 7 paged them with its own
//! small arrows; no screenshot shows them).

use crate::fb::{Framebuffer, Icon, Rect};
use crate::layout::BarLayout;
use crate::paint::{Bar, LiveIcon, TaskState};
use crate::seven::{self, Orb, SevenBar, Slot, SlotKind};
use crate::text::Fonts;

/// What a slot is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotOf {
    /// Pinned program `k` (an index into `Bar::pinned`), with its running task if it runs.
    Pinned(usize, Option<usize>),
    /// A running program that is not pinned (an index into `Bar::tasks`).
    Task(usize),
}

/// The slots in order (`SlotOf`): the pinned programs, then the running ones not pinned. A task
/// claimed by two pinned shortcuts stands in the first one's place only.
pub fn slots(bar: &Bar) -> Vec<SlotOf> {
    let mut claimed = vec![false; bar.tasks.len()];
    let mut out = Vec::with_capacity(bar.pinned.len() + bar.tasks.len());
    for (k, p) in bar.pinned.iter().enumerate() {
        let task = p.task.filter(|&t| t < bar.tasks.len() && !claimed[t]);
        if let Some(t) = task {
            claimed[t] = true;
        }
        out.push(SlotOf::Pinned(k, task));
    }
    out.extend((0..bar.tasks.len()).filter(|&t| !claimed[t]).map(SlotOf::Task));
    out
}

/// Which running program (an index into `programs`, the tasks' program files) each pinned
/// shortcut is (`targets`: the program files the shortcuts point at; `None`: unreadable, never
/// matches): whole paths compared, case aside.
pub fn pin_tasks(targets: &[Option<String>], programs: &[String]) -> Vec<Option<usize>> {
    targets
        .iter()
        .map(|t| t.as_ref().and_then(|t| programs.iter().position(|p| !p.is_empty() && p.eq_ignore_ascii_case(t))))
        .collect()
}

/// The clock's space for the bar's two lines.
fn clock_space(fonts: &Fonts, bar: &Bar) -> i32 {
    if bar.clock.is_empty() { 0 } else { seven::clock_space_for(fonts.regular, &bar.clock, &bar.date) }
}

/// The scroll arrows' width at the room's right end (the 2000 look's, as the XP bar's).
pub const ARROWS_W: i32 = 16;

/// How many slots stand from x 59 before column `room` (the notification area's fade).
pub fn capacity(room: i32) -> usize {
    if room < seven::FIRST_SLOT + seven::SLOT_W { 0 } else { ((room - seven::FIRST_SLOT - seven::SLOT_W) / seven::SLOT_STEP + 1) as usize }
}

/// What the bar shows: the slots in order, and the rows of running programs with their arrows
/// (row shown, rows, per row) when they do not fit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub shown: Vec<SlotOf>,
    pub rows: Option<(usize, usize, usize)>,
}

/// The slots shown in a room `room` columns wide (`capacity`), row `row` when they need rows: a
/// running program is never hidden in favour of a pinned shortcut that is not running (those go
/// first, from the right); running programs that still do not fit go into rows of what fits
/// before the arrows (the review of brief 114, finding 1).
pub fn plan(order: &[SlotOf], room: i32, row: usize) -> Plan {
    let cap = capacity(room);
    if order.len() <= cap {
        return Plan { shown: order.to_vec(), rows: None };
    }
    let running = |s: &&SlotOf| !matches!(s, SlotOf::Pinned(_, None));
    let running_count = order.iter().filter(running).count();
    if running_count <= cap {
        // Keep every running one and the first idle pinned ones that fit, in their order.
        let mut idle_room = cap - running_count;
        let shown = order
            .iter()
            .filter(|s| {
                if running(s) {
                    return true;
                }
                let keep = idle_room > 0;
                idle_room = idle_room.saturating_sub(1);
                keep
            })
            .copied()
            .collect();
        return Plan { shown, rows: None };
    }
    let per_row = capacity(room - ARROWS_W);
    if per_row == 0 {
        // No room at all: nothing shown (the arrows would hold nothing).
        return Plan { shown: Vec::new(), rows: None };
    }
    // In rows, the running programs stand in the order of their buttons (a pinned one too), so
    // that `layout::row_of` finds the row of the window that came to the front.
    let mut list: Vec<SlotOf> = order.iter().filter(running).copied().collect();
    list.sort_by_key(|s| match *s {
        SlotOf::Pinned(_, Some(t)) | SlotOf::Task(t) => t,
        SlotOf::Pinned(_, None) => usize::MAX,
    });
    let rows = list.len().div_ceil(per_row);
    let row = row.min(rows - 1);
    Plan { shown: list.into_iter().skip(row * per_row).take(per_row).collect(), rows: Some((row, rows, per_row)) }
}

/// The room before the notification area's fade on a bar `width` wide.
fn room_of(fonts: &Fonts, width: i32, bar: &Bar) -> (seven::Layout, i32) {
    let l = seven::layout(width, 0, bar.tray.len(), clock_space(fonts, bar));
    let room = l.zone - seven::FADE;
    (l, room)
}

/// Where everything of `bar` goes on a 7 bar `width` wide.
pub fn bar_layout(fonts: &Fonts, width: i32, bar: &Bar) -> BarLayout {
    let (_, room) = room_of(fonts, width, bar);
    let p = plan(&slots(bar), room, bar.row);
    let l = seven::layout(width, p.shown.len(), bar.tray.len(), clock_space(fonts, bar));
    let none = Rect::new(0, 0, 0, 0);
    let mut tasks = vec![none; bar.tasks.len()];
    let mut pinned = vec![none; bar.pinned.len()];
    for (s, &r) in p.shown.iter().zip(&l.slots) {
        match *s {
            SlotOf::Pinned(k, task) => {
                pinned[k] = r;
                if let Some(t) = task {
                    tasks[t] = r;
                }
            }
            SlotOf::Task(t) => tasks[t] = r,
        }
    }
    let scroll = p.rows.map(|(row, rows, per_row)| {
        let x = room - ARROWS_W;
        let h = seven::HEIGHT / 2;
        crate::layout::TaskScroll { up: Rect::new(x, 0, ARROWS_W, h), down: Rect::new(x, h, ARROWS_W, seven::HEIGHT - h), row, rows, per_row }
    });
    let clock = Rect::new(l.clock.0, 0, (l.clock.1 - l.clock.0 + 1).max(0), seven::HEIGHT);
    BarLayout {
        start: Rect::new(0, 0, seven::ORB_W, seven::HEIGHT),
        tasks,
        first_task: 0,
        scroll,
        well: Rect::new(l.zone, 0, width - seven::SHOW_DESKTOP_W - l.zone, seven::HEIGHT),
        tray: l.tray_icons.clone(),
        clock,
        number: None,
        quick: None,
        task_band: None,
        pinned,
        show_desktop: Some(l.show_desktop),
    }
}

/// A 16×16 picture with every pixel doubled (a task without its 32×32 picture).
fn doubled(icon: &Icon) -> Icon {
    let (w, h) = (icon.width * 2, icon.height * 2);
    let pixels = (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).map(|(x, y)| icon.pixels[((y / 2) * icon.width + x / 2) as usize]).collect();
    Icon { width: w, height: h, pixels }
}

thread_local! {
    /// How many 7 bars this thread has painted (a test proves the classic and XP bars paint none,
    /// so they never ask a Windows 7 picture: every one is read in `seven::paint`).
    static PAINTED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many 7 bars this thread has painted so far.
pub fn painted() -> usize {
    PAINTED.with(std::cell::Cell::get)
}

/// Whether a release over a pinned slot starts its program: not when the same one started less
/// than the double-click time `wait` ago (the second click of a double click; the review of brief
/// 114, finding 4).
pub fn start_due(last: Option<(&str, std::time::Instant)>, link: &str, now: std::time::Instant, wait: std::time::Duration) -> bool {
    !last.is_some_and(|(l, at)| l == link && now.saturating_duration_since(at) < wait)
}

/// The live picture a slot shows: a pinned program's own (also while it runs), else the task's.
fn live_icon_of(s: &SlotOf) -> LiveIcon {
    match *s {
        SlotOf::Pinned(k, _) => LiveIcon::Pinned(k),
        SlotOf::Task(t) => LiveIcon::Task(t),
    }
}

/// The 1x boxes of the slots' program pictures (32×32 at 14, 4 in each slot shown).
pub fn live_icon_boxes(fonts: &Fonts, width: i32, bar: &Bar) -> Vec<(LiveIcon, Rect)> {
    let (_, room) = room_of(fonts, width, bar);
    let p = plan(&slots(bar), room, bar.row);
    let l = seven::layout(width, p.shown.len(), bar.tray.len(), clock_space(fonts, bar));
    p.shown.iter().zip(&l.slots).map(|(s, r)| (live_icon_of(s), Rect::new(r.x + seven::ICON_X, r.y + seven::ICON_Y, seven::ICON, seven::ICON))).collect()
}

/// Paints the 7 bar (the framebuffer 40 tall) and returns where everything went.
pub fn paint_bar(fb: &mut Framebuffer, fonts: &Fonts, bar: &Bar) -> BarLayout {
    PAINTED.with(|p| p.set(p.get() + 1));
    let space = clock_space(fonts, bar);
    let (_, room) = room_of(fonts, fb.width, bar);
    let p = plan(&slots(bar), room, bar.row);
    let task_icon = |t: usize| -> Option<Icon> {
        bar.big_icons.get(t).cloned().flatten().or_else(|| bar.tasks.get(t).and_then(|task| task.icon.as_ref()).map(doubled))
    };
    let kind_of = |t: usize| match bar.tasks.get(t).map(|task| task.state) {
        Some(TaskState::Active | TaskState::Flashing) => SlotKind::Front,
        _ => SlotKind::Running,
    };
    // A picture the caller draws at the final size is left out here (`Bar::drawn_later`).
    let later = |s: &SlotOf| bar.drawn_later.contains(&live_icon_of(s));
    let seven_slots: Vec<Slot> = p
        .shown
        .iter()
        .map(|s| {
            let slot = match *s {
                SlotOf::Pinned(k, None) => Slot { kind: SlotKind::Pinned, icon: bar.pinned[k].icon.clone() },
                SlotOf::Pinned(k, Some(t)) => Slot { kind: kind_of(t), icon: bar.pinned[k].icon.clone().or_else(|| task_icon(t)) },
                SlotOf::Task(t) => Slot { kind: kind_of(t), icon: task_icon(t) },
            };
            if later(s) { Slot { icon: None, ..slot } } else { slot }
        })
        .collect();
    let blank = Icon { width: 16, height: 16, pixels: vec![0; 256] };
    let seven_bar = SevenBar {
        orb: if bar.start_pressed { Orb::MenuOpen } else { Orb::Normal },
        slots: seven_slots,
        tray_icons: bar.tray.iter().map(|i| i.clone().unwrap_or_else(|| blank.clone())).collect(),
        time: bar.clock.clone(),
        date: bar.date.clone(),
        clock_space: space,
    };
    let l = seven::layout(fb.width, seven_bar.slots.len(), bar.tray.len(), space);
    seven::paint(fb, &seven_bar, &l, fonts.regular);
    let layout = bar_layout(fonts, fb.width, bar);
    if let Some(s) = layout.scroll {
        crate::paint::paint_scroll_arrows(fb, &crate::theme::WIN7, s, bar.scroll_pressed);
    }
    layout
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::{Pinned, Task};

    fn task(state: TaskState) -> Task {
        Task { title: "W".into(), icon: None, state }
    }

    /// Which slot a window gets: pinned and running in the pinned place, several windows of one
    /// program are one task (the platform combines them), a program not pinned after the pinned
    /// ones in its first window's order, a shortcut whose target could not be read never matches
    /// (the platform gives it no task), a task claimed twice stands in the first place only.
    #[test]
    fn each_program_gets_its_slot() {
        let bar = Bar {
            tasks: vec![task(TaskState::Normal), task(TaskState::Active), task(TaskState::Normal)],
            pinned: vec![Pinned { icon: None, task: Some(1) }, Pinned { icon: None, task: None }, Pinned { icon: None, task: Some(1) }],
            ..Bar::default()
        };
        assert_eq!(slots(&bar), [SlotOf::Pinned(0, Some(1)), SlotOf::Pinned(1, None), SlotOf::Pinned(2, None), SlotOf::Task(0), SlotOf::Task(2)]);
        let none = Bar { tasks: vec![task(TaskState::Normal)], ..Bar::default() };
        assert_eq!(slots(&none), [SlotOf::Task(0)]);
        // The matching: whole paths, case aside; an unreadable shortcut never matches; a
        // program's several windows are one task (one key).
        let programs = ["C:\\Windows\\notepad.exe".to_string(), "C:\\Tools\\paint.exe".to_string()];
        let targets = [Some("c:\\windows\\NOTEPAD.EXE".to_string()), None, Some("C:\\Tools\\paint".to_string()), Some("C:\\Tools\\paint.exe".to_string())];
        assert_eq!(pin_tasks(&targets, &programs), [Some(0), None, None, Some(1)]);
        assert_eq!(pin_tasks(&[Some(String::new())], &[String::new()]), [None], "an empty program path never matches");
    }

    /// The layout at several widths: the orb, slots 60 wide every 62 from 59 (a running pinned
    /// one's task has the pinned place), the clock and Show desktop at the right; slots past the
    /// fade are not shown (no rectangle to click).
    #[test]
    fn the_bar_is_laid_out_in_slots() {
        let regular = crate::text::Font::parse(include_str!("../tests/fixtures/tahoma-11px.font")).unwrap();
        let bold = crate::text::Font::parse(include_str!("../tests/fixtures/tahoma-11px-bold.font")).unwrap();
        let fonts = Fonts { regular: &regular, bold: &bold };
        let bar = Bar {
            tasks: vec![task(TaskState::Normal), task(TaskState::Active)],
            pinned: vec![Pinned { icon: None, task: None }, Pinned { icon: None, task: Some(1) }],
            tray: vec![None, None],
            clock: "3:45 PM".into(),
            date: "11/25/2026".into(),
            ..Bar::default()
        };
        for width in [800, 1440, 1920] {
            let l = bar_layout(&fonts, width, &bar);
            assert_eq!(l.start, Rect::new(0, 0, 56, 40));
            assert_eq!(l.pinned, [Rect::new(59, 0, 60, 40), Rect::new(121, 0, 60, 40)]);
            assert_eq!(l.tasks, [Rect::new(183, 0, 60, 40), Rect::new(121, 0, 60, 40)], "the running pinned one in its place");
            assert_eq!(l.show_desktop, Some(Rect::new(width - 15, 0, 15, 40)));
            assert!(l.clock.right() <= width - 15 && l.tray.len() == 2);
        }
        // The classic and XP bars never reach the 7 painting (nor its pictures).
        let before = painted();
        for theme in [&crate::theme::WIN95, &crate::theme::WIN98, &crate::theme::WIN2000, &crate::theme::WINXP] {
            let mut fb = Framebuffer::new(800, theme.bar.height, 0);
            crate::paint::paint_bar(&mut fb, theme, &fonts, &bar);
        }
        assert_eq!(painted(), before);
        let mut fb = Framebuffer::new(800, 40, 0);
        crate::paint::paint_bar(&mut fb, &crate::theme::WIN7, &fonts, &bar);
        assert_eq!(painted(), before + 1);
        let full = Bar { tasks: (0..40).map(|_| task(TaskState::Normal)).collect(), ..bar.clone() };
        let l = bar_layout(&fonts, 800, &full);
        let s = l.scroll.expect("the scroll arrows");
        assert!(l.tasks.iter().filter(|r| r.w > 0).all(|r| r.right() <= s.up.x), "the shown ones before the arrows");
        assert!(l.pinned.iter().zip(&full.pinned).all(|(r, p)| p.task.is_some() || r.w == 0), "idle pinned ones go first");
        // The next row shows the next running programs.
        let next = bar_layout(&fonts, 800, &Bar { row: 1, ..full.clone() });
        assert_eq!(next.scroll.unwrap().row, 1);
        assert_ne!(next.tasks, l.tasks);
    }

    /// The review of brief 114, finding 1: room short at several widths, sizes and tray widths:
    /// running programs are never hidden for idle pinned ones; past that, rows with arrows; no
    /// room at all shows nothing and does not fail.
    #[test]
    fn running_programs_come_before_idle_pinned_ones() {
        let order = |pinned_idle: usize, running: usize| -> Vec<SlotOf> {
            (0..pinned_idle).map(|k| SlotOf::Pinned(k, None)).chain((0..running).map(SlotOf::Task)).collect()
        };
        // Rooms: 1920 at size 1, 2 and 3 with 6 icons, a wide tray at size 3, none.
        for room in [1810, 663, 360, 150, 0, -50] {
            let cap = capacity(room);
            for (idle, running) in [(9, 3), (3, 9), (12, 12), (0, 40), (30, 0)] {
                let p = plan(&order(idle, running), room, 0);
                let shown_running = p.shown.iter().filter(|s| matches!(s, SlotOf::Task(_))).count();
                let shown_idle = p.shown.len() - shown_running;
                assert!(p.shown.len() <= cap, "{room} {idle} {running}: {}", p.shown.len());
                if running <= cap {
                    assert_eq!(shown_running, running, "{room} {idle} {running}: every running one");
                    assert_eq!(shown_idle, idle.min(cap - running), "the idle ones that fit");
                    assert!(p.rows.is_none());
                } else if capacity(room - ARROWS_W) > 0 {
                    let (_, rows, per_row) = p.rows.expect("rows");
                    assert_eq!((shown_idle, rows), (0, running.div_ceil(per_row)), "{room} {idle} {running}");
                } else {
                    assert!(p.shown.is_empty() && p.rows.is_none(), "no room at all");
                }
            }
        }
        assert_eq!((capacity(1810), capacity(663), capacity(360), capacity(0)), (28, 9, 4, 0));
        // The idle pinned ones kept are the first ones (dropped from the right).
        let p = plan(&order(5, 2), capacity_room(4), 0);
        assert_eq!(p.shown, [SlotOf::Pinned(0, None), SlotOf::Pinned(1, None), SlotOf::Task(0), SlotOf::Task(1)]);
        // In rows, a pinned running one stands at its button's place (so `row_of` is right).
        let rows_order = [SlotOf::Pinned(0, Some(3)), SlotOf::Task(0), SlotOf::Task(1), SlotOf::Task(2), SlotOf::Task(4)];
        let p = plan(&rows_order, capacity_room(3), 1);
        assert_eq!(p.shown, [SlotOf::Task(2), SlotOf::Pinned(0, Some(3))]);
        assert_eq!(p.rows.unwrap().2, 2);
        // A row past the end is the last.
        let p = plan(&order(0, 10), capacity_room(4), 9);
        let (row, rows, _) = p.rows.unwrap();
        assert_eq!(row, rows - 1);
        // A pinned shortcut whose program runs counts as running.
        let mixed = [SlotOf::Pinned(0, Some(0)), SlotOf::Pinned(1, None), SlotOf::Task(1)];
        assert_eq!(plan(&mixed, capacity_room(2), 0).shown, [SlotOf::Pinned(0, Some(0)), SlotOf::Task(1)]);
        // The second click of a double click starts nothing; another slot or later does.
        let t0 = std::time::Instant::now();
        let wait = std::time::Duration::from_millis(500);
        assert!(start_due(None, "a.lnk", t0, wait));
        assert!(!start_due(Some(("a.lnk", t0)), "a.lnk", t0 + std::time::Duration::from_millis(200), wait));
        assert!(start_due(Some(("a.lnk", t0)), "b.lnk", t0 + std::time::Duration::from_millis(200), wait));
        assert!(start_due(Some(("a.lnk", t0)), "a.lnk", t0 + std::time::Duration::from_millis(900), wait));
    }

    /// The owner, 2026-11-25 ("icons at 2x on 7"): a live picture's 1x box and the place and size
    /// of its true picture on the enlarged bar at 1, 1.5, 1.75, 2 and 3: 7's 32 and XP's 16 boxes.
    #[test]
    fn live_pictures_take_their_boxes_enlarged_place() {
        use crate::paint::{draw_final_icon, live_icon_boxes};
        use crate::scale::Scale;
        let seven = Rect::new(73, 4, 32, 32);
        let xp = Rect::new(108, 4, 16, 16);
        for (q, s7, sxp) in [
            (4, Rect::new(73, 4, 32, 32), Rect::new(108, 4, 16, 16)),
            (6, Rect::new(109, 6, 48, 48), Rect::new(162, 6, 24, 24)),
            (7, Rect::new(128, 7, 56, 56), Rect::new(189, 7, 28, 28)),
            (8, Rect::new(146, 8, 64, 64), Rect::new(216, 8, 32, 32)),
            (12, Rect::new(219, 12, 96, 96), Rect::new(324, 12, 48, 48)),
        ] {
            let scale = Scale::from_quarters(q);
            assert_eq!((scale.rect(seven), scale.rect(xp)), (s7, sxp), "{q} quarters");
            assert_eq!(scale.len(32), s7.w);
        }
        // A picture of the final size is drawn as it is; a larger one shrunk; a smaller one not.
        let pic = |side: i32| Icon { width: side, height: side, pixels: vec![0xFF11_2233; (side * side) as usize] };
        let mut fb = Framebuffer::new(400, 80, 0);
        assert!(draw_final_icon(&mut fb, Scale::whole(2), seven, &pic(64)));
        assert_eq!((fb.get(146, 8), fb.get(209, 71), fb.get(210, 71)), (0xFF11_2233, 0xFF11_2233, 0));
        assert!(draw_final_icon(&mut fb, Scale::from_quarters(7), seven, &pic(64)));
        assert!(!draw_final_icon(&mut fb, Scale::whole(2), seven, &pic(32)), "the enlarged one stays");
        // The bar's boxes: each slot's 32 at 14, 4.
        let regular = crate::text::Font::parse(include_str!("../tests/fixtures/tahoma-11px.font")).unwrap();
        let bold = crate::text::Font::parse(include_str!("../tests/fixtures/tahoma-11px-bold.font")).unwrap();
        let fonts = Fonts { regular: &regular, bold: &bold };
        let bar = Bar {
            tasks: vec![task(TaskState::Normal)],
            pinned: vec![crate::paint::Pinned { icon: None, task: None }],
            ..Bar::default()
        };
        let boxes = live_icon_boxes(&crate::theme::WIN7, &fonts, 800, &bar);
        assert_eq!(boxes, [(LiveIcon::Pinned(0), Rect::new(73, 4, 32, 32)), (LiveIcon::Task(0), Rect::new(135, 4, 32, 32))]);
        // Under the classic looks: none.
        assert!(live_icon_boxes(&crate::theme::WIN2000, &fonts, 800, &bar).is_empty());
    }

    /// The room in which exactly `n` slots stand.
    fn capacity_room(n: i32) -> i32 {
        seven::FIRST_SLOT + seven::SLOT_W + seven::SLOT_STEP * (n - 1)
    }
}
