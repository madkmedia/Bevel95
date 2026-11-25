//! Windows 7 Basic's taskbar in the bar (brief 114): the bar's own data (`paint::Bar`) put into
//! `seven`'s slots and painted by `seven::paint`, laid out into the same `BarLayout` the classic
//! bar fills, so the bar's clicks, menus and tooltips work as before.
//!
//! The slots, as Windows 7 shipped (large icons, always combined, no labels): first the pinned
//! programs (Quick Launch's shortcuts, in their order), a running one framed in its own place;
//! then every running program that is not pinned, in the order of its first window. A program
//! whose window is in front gets the lighter frame; a flashing one the lighter frame in the half
//! of its flashing that shows (the platform alternates at the classic rate: PROVISIONAL). Slots
//! the bar has no room for are not shown (PROVISIONAL: Windows 7 paged them with small arrows;
//! no screenshot shows it).

use crate::fb::{Framebuffer, Icon, Rect};
use crate::layout::BarLayout;
use crate::paint::{Bar, TaskState};
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

/// Where everything of `bar` goes on a 7 bar `width` wide.
pub fn bar_layout(fonts: &Fonts, width: i32, bar: &Bar) -> BarLayout {
    let order = slots(bar);
    let l = seven::layout(width, order.len(), bar.tray.len(), clock_space(fonts, bar));
    // Only the slots that stand before the notification area's fade.
    let room = l.zone - seven::FADE;
    let shown = |r: Rect| if r.right() <= room { r } else { Rect::new(0, 0, 0, 0) };
    let mut tasks = vec![Rect::new(0, 0, 0, 0); bar.tasks.len()];
    let mut pinned = vec![Rect::new(0, 0, 0, 0); bar.pinned.len()];
    for (s, r) in order.iter().zip(&l.slots) {
        let r = shown(*r);
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
    let clock = Rect::new(l.clock.0, 0, (l.clock.1 - l.clock.0 + 1).max(0), seven::HEIGHT);
    BarLayout {
        start: Rect::new(0, 0, seven::ORB_W, seven::HEIGHT),
        tasks,
        first_task: 0,
        scroll: None,
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

/// Paints the 7 bar (the framebuffer 40 tall) and returns where everything went.
pub fn paint_bar(fb: &mut Framebuffer, fonts: &Fonts, bar: &Bar) -> BarLayout {
    PAINTED.with(|p| p.set(p.get() + 1));
    let order = slots(bar);
    let space = clock_space(fonts, bar);
    let l = seven::layout(fb.width, order.len(), bar.tray.len(), space);
    let room = l.zone - seven::FADE;
    let task_icon = |t: usize| -> Option<Icon> {
        bar.big_icons.get(t).cloned().flatten().or_else(|| bar.tasks.get(t).and_then(|task| task.icon.as_ref()).map(doubled))
    };
    let kind_of = |t: usize| match bar.tasks.get(t).map(|task| task.state) {
        Some(TaskState::Active | TaskState::Flashing) => SlotKind::Front,
        _ => SlotKind::Running,
    };
    let seven_slots: Vec<Slot> = order
        .iter()
        .zip(&l.slots)
        .take_while(|(_, r)| r.right() <= room)
        .map(|(s, _)| match *s {
            SlotOf::Pinned(k, None) => Slot { kind: SlotKind::Pinned, icon: bar.pinned[k].icon.clone() },
            SlotOf::Pinned(k, Some(t)) => Slot { kind: kind_of(t), icon: bar.pinned[k].icon.clone().or_else(|| task_icon(t)) },
            SlotOf::Task(t) => Slot { kind: kind_of(t), icon: task_icon(t) },
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
    // Laid out again for the slots painted (the same places; fewer when the bar is full).
    let painted = seven::layout(fb.width, seven_bar.slots.len(), bar.tray.len(), space);
    seven::paint(fb, &seven_bar, &painted, fonts.regular);
    bar_layout(fonts, fb.width, bar)
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
        assert!(l.tasks.iter().filter(|r| r.w > 0).all(|r| r.right() <= l.well.x - seven::FADE));
        assert!(l.tasks.iter().any(|r| r.w == 0), "the ones without room are not shown");
    }
}
