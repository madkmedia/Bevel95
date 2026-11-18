//! Which buttons the bar shows: one per app (a "group"), built from the open windows.
//! Pure bookkeeping, so it is tested without Windows. The platform layer supplies the
//! windows and carries out clicks.

use crate::paint::TaskState;
use crate::window_icons::ClassicWindow;

/// What the platform knows about one window that belongs on the taskbar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowInfo {
    /// The platform's handle for the window (an HWND on Windows).
    pub id: u64,
    pub title: String,
    /// Windows with the same key share one button (on Windows: the program's full path).
    pub group: String,
    /// Shown on a button that holds several windows, as in "3 Notepad".
    pub app_name: String,
    /// The window asked for attention and has not been activated since.
    pub flashing: bool,
    /// The classic picture shown instead of the window's own icon, if it has one
    /// (`window_icons::classify`).
    pub classic: Option<ClassicWindow>,
}

/// One button: an app with one or more windows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    pub key: String,
    /// The window title, or "3 Notepad" when the app has several windows (as in Windows XP).
    pub label: String,
    /// Window ids, oldest first.
    pub windows: Vec<u64>,
    pub state: TaskState,
}

/// Remembers the order in which windows first appeared, so buttons don't jump around.
#[derive(Clone, Debug)]
pub struct TaskList {
    order: Vec<u64>,
    /// Windows of one program share a button (`group = yes` in settings.txt, Windows XP's
    /// "Group similar taskbar buttons"); off, every window has its own, as in Windows 95, 98
    /// and 2000.
    pub group: bool,
}

impl Default for TaskList {
    fn default() -> TaskList {
        TaskList { order: Vec::new(), group: true }
    }
}

impl TaskList {
    pub fn new(group: bool) -> TaskList {
        TaskList { group, ..TaskList::default() }
    }

    /// The windows in the order their buttons go (oldest first, as dragged).
    pub fn order(&self) -> &[u64] {
        &self.order
    }

    /// Puts the windows in this order (Esc during a drag; the order kept across a restart).
    /// The next `update` drops those that are gone and adds new ones at the end.
    pub fn set_order(&mut self, order: Vec<u64>) {
        self.order = order;
    }

    /// Moves button `from` of `groups` (the buttons `update` gave last) to place `to`: the
    /// windows are put in the order of the buttons, each button's windows together, so the
    /// next `update` gives the buttons in the new order.
    pub fn move_button(&mut self, groups: &[Group], from: usize, to: usize) {
        if from >= groups.len() || to >= groups.len() {
            return;
        }
        let mut buttons: Vec<&Group> = groups.iter().collect();
        let moved = buttons.remove(from);
        buttons.insert(to, moved);
        let mut order: Vec<u64> = buttons.iter().flat_map(|g| g.windows.iter().copied()).collect();
        // Windows the buttons do not hold (none, normally) keep their place at the end.
        order.extend(self.order.iter().filter(|id| !order.contains(id)).copied().collect::<Vec<_>>());
        self.order = order;
    }

    /// The buttons for the windows open now, given which window (if any) is active.
    /// Windows seen before keep their place and new ones go to the end; each button sits
    /// where its oldest window is. The active app's button is `Active`; otherwise a button
    /// with a window asking for attention is `Flashing`. With grouping off every window is a
    /// button of its own (its key is then the window's id).
    pub fn update(&mut self, windows: &[WindowInfo], active: Option<u64>) -> Vec<Group> {
        let info = |id: u64| windows.iter().find(|w| w.id == id);
        self.order.retain(|&id| info(id).is_some());
        for w in windows {
            if !self.order.contains(&w.id) {
                self.order.push(w.id);
            }
        }
        let mut groups: Vec<Group> = Vec::new();
        for &id in &self.order {
            let Some(w) = info(id) else { continue };
            let key = if self.group { w.group.clone() } else { format!("{}#{id}", w.group) };
            match groups.iter_mut().find(|g| g.key == key) {
                Some(group) => group.windows.push(id),
                None => groups.push(Group {
                    key,
                    label: String::new(),
                    windows: vec![id],
                    state: TaskState::Normal,
                }),
            }
        }
        for group in &mut groups {
            let members: Vec<&WindowInfo> = group.windows.iter().filter_map(|&id| info(id)).collect();
            group.label = match members.as_slice() {
                [only] => only.title.clone(),
                [first, ..] => format!("{} {}", members.len(), first.app_name),
                [] => String::new(),
            };
            group.state = if active.is_some_and(|a| group.windows.contains(&a)) {
                TaskState::Active
            } else if members.iter().any(|w| w.flashing) {
                TaskState::Flashing
            } else {
                TaskState::Normal
            };
        }
        groups
    }
}

/// What a left click on a task button does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Click {
    /// Nothing more (an empty button, or the press already closed this button's list).
    Nothing,
    /// Bring this window to the front (restoring it if it is minimized).
    Activate(u64),
    /// Minimize this window: it is the one in front.
    Minimize(u64),
    /// Open the list of the button's windows.
    OpenList,
}

/// What the platform read at the moment of a click on a task button.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClickState {
    /// The taskbar window in front, read at the click (not as last seen by the tick).
    pub front: Option<u64>,
    /// The button's window is minimized (only asked for a button with one window).
    pub minimized: bool,
    /// This button's list of windows was open when the mouse button went down: that press
    /// closed it, and the click ends there (as Windows XP's grouped button did).
    pub list_was_open: bool,
}

/// The click on a button holding `windows`. One window comes to the front, or is minimized if
/// it already is in front; a minimized window is never "in front", even when Windows still
/// calls it the foreground window (nothing else took its place), so a click on its button
/// always brings it back. Several windows get a list, and a second click closes that list.
pub fn click(windows: &[u64], state: ClickState) -> Click {
    match windows {
        [] => Click::Nothing,
        [only] if state.front == Some(*only) && !state.minimized => Click::Minimize(*only),
        [only] => Click::Activate(*only),
        _ if state.list_was_open => Click::Nothing,
        _ => Click::OpenList,
    }
}

/// A press on a task button, followed until the button comes up: a click, or a drag that puts
/// the button elsewhere in its row (Bevel95's own; Windows got dragging with Windows 7).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ButtonPress {
    /// The button's windows when it was pressed: it is found again by any of them after the
    /// buttons were rebuilt (`find`).
    pub windows: Vec<u64>,
    /// Where the press was, across the bar (screen pixels).
    start_x: i32,
    /// The pointer went sideways further than Windows' drag threshold: the button follows it,
    /// and the release is no click.
    pub dragging: bool,
    /// The windows' order when the press began, for Esc (`TaskList::set_order`).
    pub saved: Vec<u64>,
    /// Where on the button it was pressed, from its left edge (1x pixels): that point stays
    /// under the pointer while the button is dragged (`drag_step`).
    pub grab: i32,
}

/// What the release of a pressed task button ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Release {
    /// No drag: the button's click (if the pointer is still on it).
    Click,
    /// A drag: the button stays where it was dropped, and nothing is clicked.
    Dropped,
}

impl ButtonPress {
    /// A press at `x` (screen pixels across the bar), `grab` 1x pixels from the button's left.
    pub fn new(windows: Vec<u64>, x: i32, saved: Vec<u64>, grab: i32) -> ButtonPress {
        ButtonPress { windows, start_x: x, dragging: false, saved, grab }
    }

    /// The pointer is at `x`: once it has gone further sideways than `threshold` (Windows'
    /// SM_CXDRAG, in the same pixels) the press is a drag. True when the drag begins now.
    pub fn moved(&mut self, x: i32, threshold: i32) -> bool {
        if self.dragging || (x - self.start_x).abs() <= threshold {
            return false;
        }
        self.dragging = true;
        true
    }

    pub fn release(&self) -> Release {
        if self.dragging { Release::Dropped } else { Release::Click }
    }
}

/// The button among `groups` that holds any of `windows` (one may have gone meanwhile).
pub fn find(groups: &[Group], windows: &[u64]) -> Option<usize> {
    groups.iter().position(|g| g.windows.iter().any(|id| windows.contains(id)))
}

/// How long a button takes to slide into its place, in milliseconds: a neighbour making room
/// for a dragged button, the dragged one at its release, all of them after Esc. Bevel95's own
/// (no Windows before 7 dragged task buttons), about a tenth of a second.
pub const SLIDE_MS: u64 = 100;

/// One step of a drag in the row shown, for the pointer at `pointer` (1x pixels across the
/// bar): where the dragged button is drawn and which place of the row it belongs in. `row`
/// holds the places' (left, width), `at` is the dragged button's place now and `grab` the point
/// of it that was pressed (from its left edge). That point stays under the pointer, pixel for
/// pixel, but the button never leaves the row: it stops at the row's ends. It belongs in the
/// place its middle is over; the line between two places is the middle of the gap between
/// them, so a neighbour makes room once the dragged button covers more than half of it.
/// Returns (place, left); nothing for an empty row or a place outside it.
pub fn drag_step(row: &[(i32, i32)], at: usize, grab: i32, pointer: i32) -> Option<(usize, i32)> {
    let &(_, width) = row.get(at)?;
    let (first, last) = (row.first()?, row.last()?);
    let left = (pointer - grab).min(last.0 + last.1 - width).max(first.0);
    let middle = left + width / 2;
    let place = (1..row.len()).take_while(|&i| middle >= (row[i - 1].0 + row[i - 1].1 + row[i].0) / 2).count();
    Some((place, left))
}

/// Where along a slide a button is `elapsed` ms after it began: quick at first, gentle at the
/// end (ease-out, quadratic), in whole pixels, and exactly at `to` once `SLIDE_MS` have passed.
fn slide_at(from: i32, to: i32, elapsed: u64) -> i32 {
    let total = SLIDE_MS as i64;
    let left = total - (elapsed.min(SLIDE_MS) as i64);
    from + ((i64::from(to - from) * (total * total - left * left)) / (total * total)) as i32
}

/// One button of the row as the last frame drew it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Drawn {
    /// The button's key (`Group::key`), by which it is known from frame to frame.
    key: String,
    /// The left of its place in the row.
    place: i32,
    /// Where it was drawn.
    left: i32,
    /// It was the dragged one (drawn where the pointer put it).
    dragged: bool,
    /// Its slide into `place`: where it began and when (ms).
    slide: Option<(i32, u64)>,
}

impl Drawn {
    fn left_at(&self, now: u64) -> i32 {
        match self.slide {
            _ if self.dragged => self.left,
            Some((from, start)) => slide_at(from, self.place, now.saturating_sub(start)),
            None => self.place,
        }
    }
}

/// Where the buttons of the row shown are drawn while a drag lasts and while they slide
/// afterwards. Frame by frame it is told which button is in which place (`frame`): a button
/// whose place changed slides there from where it was drawn, in `SLIDE_MS`; the dragged one is
/// drawn where the pointer puts it and slides into its place when it is no longer dragged
/// (released, Esc, the mouse lost). If the places themselves change (a window opened or closed,
/// so the buttons got another width) everything is put at its place at once. The platform
/// uses it only while a drag or a slide lasts (`clear` at rest), so nothing else ever slides.
#[derive(Clone, Debug, Default)]
pub struct Motion {
    drawn: Vec<Drawn>,
    row: Vec<(i32, i32)>,
}

impl Motion {
    /// The frame at `now` (ms, any steady clock): `keys` are the buttons of the row in the order
    /// of its places `row`, `dragged` which of them is dragged and where it is drawn
    /// (`drag_step`). Returns each button's left, in the order of `keys`.
    pub fn frame(&mut self, keys: &[&str], row: &[(i32, i32)], dragged: Option<(usize, i32)>, now: u64) -> Vec<i32> {
        let laid_out_anew = self.row != row;
        let before = if laid_out_anew { Vec::new() } else { std::mem::take(&mut self.drawn) };
        self.row = row.to_vec();
        self.drawn = keys
            .iter()
            .zip(row)
            .enumerate()
            .map(|(i, (&key, &(place, _)))| {
                if let Some((_, left)) = dragged.filter(|&(at, _)| at == i) {
                    return Drawn { key: key.to_string(), place, left, dragged: true, slide: None };
                }
                let slide = match before.iter().find(|d| d.key == key) {
                    // Its place changed, or it was let go: it slides from where it is now.
                    Some(d) if d.dragged || d.place != place => {
                        let from = d.left_at(now);
                        (from != place).then_some((from, now))
                    }
                    Some(d) => d.slide.filter(|&(_, start)| now.saturating_sub(start) < SLIDE_MS),
                    None => None,
                };
                let mut d = Drawn { key: key.to_string(), place, left: place, dragged: false, slide };
                d.left = d.left_at(now);
                d
            })
            .collect();
        self.drawn.iter().map(|d| d.left).collect()
    }

    /// A slide is still under way at `now` (so the next frame differs from the last).
    pub fn sliding(&self, now: u64) -> bool {
        self.drawn.iter().any(|d| !d.dragged && d.slide.is_some_and(|(_, start)| now.saturating_sub(start) < SLIDE_MS))
    }

    /// Everything at rest: the next frame starts afresh.
    pub fn clear(&mut self) {
        *self = Motion::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Three buttons 100 wide with a gap of 2, as the bar lays them out.
    const ROW: [(i32, i32); 3] = [(0, 100), (102, 100), (204, 100)];

    fn labels(groups: &[Group]) -> Vec<&str> {
        groups.iter().map(|g| g.label.as_str()).collect()
    }

    #[test]
    fn a_press_without_a_drag_is_a_click() {
        let mut press = ButtonPress::new(vec![1], 50, vec![1, 2], 50);
        // Within Windows' threshold (4 here), either way: still a click.
        assert!(!press.moved(54, 4) && !press.moved(46, 4));
        assert_eq!(press.release(), Release::Click);
        // One pixel further: a drag, which begins once; the release clicks nothing.
        assert!(press.moved(55, 4));
        assert!(!press.moved(80, 4) && press.dragging);
        assert_eq!(press.release(), Release::Dropped);
        // Up and down does not count, only sideways (the bar is one row high).
        let mut press = ButtonPress::new(vec![1], 50, vec![], 50);
        assert!(!press.moved(50, 4));
    }

    #[test]
    fn the_dragged_button_follows_the_pointer_within_the_row() {
        // Pressed 30 from its left: that point stays under the pointer, pixel for pixel.
        assert_eq!(drag_step(&ROW, 0, 30, 30), Some((0, 0)));
        assert_eq!(drag_step(&ROW, 0, 30, 31), Some((0, 1)));
        assert_eq!(drag_step(&ROW, 0, 30, 77), Some((0, 47)));
        // The row's ends stop it: not left of the first place, not right of the last.
        assert_eq!(drag_step(&ROW, 0, 30, 10), Some((0, 0)));
        assert_eq!(drag_step(&ROW, 0, 30, -500), Some((0, 0)));
        assert_eq!(drag_step(&ROW, 0, 30, 900), Some((2, 204)));
        // No row, or a place outside it: nothing.
        assert_eq!(drag_step(&[], 0, 30, 10), None);
        assert_eq!(drag_step(&ROW, 3, 30, 10), None);
    }

    #[test]
    fn a_neighbour_makes_room_once_the_dragged_button_covers_half_of_it() {
        // Rightwards: the line between places 0 and 1 is 101, the middle of the gap. The dragged
        // button's middle (its left + 50) reaches it at left 51.
        assert_eq!(drag_step(&ROW, 0, 30, 30 + 50), Some((0, 50)));
        assert_eq!(drag_step(&ROW, 0, 30, 30 + 51), Some((1, 51)));
        // Past two lines at once (a quick move): two places.
        assert_eq!(drag_step(&ROW, 0, 30, 30 + 160), Some((2, 160)));
        // Leftwards the same lines, from the last place.
        assert_eq!(drag_step(&ROW, 2, 30, 30 + 153), Some((2, 153)));
        assert_eq!(drag_step(&ROW, 2, 30, 30 + 152), Some((1, 152)));
        assert_eq!(drag_step(&ROW, 2, 30, 30 + 50), Some((0, 50)));
        // Where it belongs depends only on where it is, not on where it came from.
        assert_eq!(drag_step(&ROW, 1, 30, 30 + 51), drag_step(&ROW, 0, 30, 30 + 51));
    }

    #[test]
    fn a_slide_takes_a_tenth_of_a_second_and_ends_exactly() {
        assert_eq!(SLIDE_MS, 100);
        assert_eq!(slide_at(102, 0, 0), 102);
        // Quick at first, gentle at the end: three quarters of the way at half the time.
        assert_eq!(slide_at(102, 0, 50), 26);
        assert_eq!(slide_at(0, 102, 50), 76);
        assert_eq!(slide_at(0, 102, 99), 101);
        assert_eq!(slide_at(0, 102, 100), 102);
        assert_eq!(slide_at(0, 102, 5000), 102);
        // Every frame a whole pixel on, never back.
        let steps: Vec<i32> = (0..=100).map(|t| slide_at(0, 102, t)).collect();
        assert!(steps.windows(2).all(|w| w[0] <= w[1]));
    }

    /// What the bar does with a drag, without Windows: the task list, the press, the row of
    /// places and the motion, frame by frame.
    struct Drag {
        list: TaskList,
        windows: Vec<WindowInfo>,
        groups: Vec<Group>,
        press: ButtonPress,
        motion: Motion,
        row: Vec<(i32, i32)>,
        pointer: i32,
    }

    impl Drag {
        /// Button `at` of `windows` pressed `grab` from its left, in the row `row`.
        fn press(windows: &[WindowInfo], group: bool, row: &[(i32, i32)], at: usize, grab: i32) -> Drag {
            let mut list = TaskList::new(group);
            let groups = list.update(windows, None);
            let pointer = row[at].0 + grab;
            let press = ButtonPress::new(groups[at].windows.clone(), pointer, list.order().to_vec(), grab);
            let mut drag =
                Drag { list, windows: windows.to_vec(), groups, press, motion: Motion::default(), row: row.to_vec(), pointer };
            drag.frame(0);
            drag
        }

        fn at(&self) -> Option<usize> {
            find(&self.groups, &self.press.windows)
        }

        /// The pointer moved to `x` (1x, the bar's scale is 1 here).
        fn to(&mut self, x: i32) {
            self.pointer = x;
            self.press.moved(x, 4);
            if !self.press.dragging {
                return;
            }
            let Some(at) = self.at() else { return };
            let Some((to, _)) = drag_step(&self.row, at, self.press.grab, x) else { return };
            if to != at {
                self.list.move_button(&self.groups, at, to);
                self.regroup();
            }
        }

        fn regroup(&mut self) {
            self.groups = self.list.update(&self.windows, None);
        }

        /// The windows changed (one closed, one came): the buttons are built again and the row
        /// laid out anew, then the drag goes on from where the pointer is.
        fn windows_now(&mut self, windows: &[WindowInfo], row: &[(i32, i32)]) {
            self.windows = windows.to_vec();
            self.row = row.to_vec();
            self.regroup();
            self.to(self.pointer);
        }

        /// The buttons' labels and lefts at `now`.
        fn frame(&mut self, now: u64) -> Vec<(String, i32)> {
            let dragged = self
                .press
                .dragging
                .then(|| self.at().and_then(|at| drag_step(&self.row, at, self.press.grab, self.pointer).map(|(_, l)| (at, l))))
                .flatten();
            let keys: Vec<&str> = self.groups.iter().map(|g| g.key.as_str()).collect();
            let lefts = self.motion.frame(&keys, &self.row, dragged, now);
            self.groups.iter().map(|g| g.label.clone()).zip(lefts).collect()
        }

        /// Esc, or the mouse capture lost: the order from the press comes back.
        fn put_back(&mut self) {
            self.list.set_order(self.press.saved.clone());
            self.press.dragging = false;
            self.regroup();
        }

        /// The release: the order stays, and nothing is clicked.
        fn release(&mut self) -> Release {
            let release = self.press.release();
            self.press.dragging = false;
            release
        }
    }

    fn at(frame: &[(String, i32)]) -> Vec<(&str, i32)> {
        frame.iter().map(|(l, x)| (l.as_str(), *x)).collect()
    }

    /// Four buttons 100 wide with a gap of 2.
    const ROW4: [(i32, i32); 4] = [(0, 100), (102, 100), (204, 100), (306, 100)];

    fn four() -> Vec<WindowInfo> {
        vec![win(1, "One", "A"), win(2, "Two", "B"), win(3, "Three", "C"), win(4, "Four", "D")]
    }

    #[test]
    fn the_drag_starts_past_the_threshold_and_follows_the_pointer() {
        let mut drag = Drag::press(&four(), true, &ROW4, 0, 30);
        // Below Windows' threshold nothing moves (and the release would be a click).
        drag.to(34);
        assert_eq!(at(&drag.frame(10)), [("One", 0), ("Two", 102), ("Three", 204), ("Four", 306)]);
        assert_eq!(drag.press.release(), Release::Click);
        // Past it the pressed point follows the pointer, from the press.
        drag.to(35);
        assert_eq!(at(&drag.frame(20))[0], ("One", 5));
        drag.to(60);
        assert_eq!(at(&drag.frame(30)), [("One", 30), ("Two", 102), ("Three", 204), ("Four", 306)]);
        assert!(!drag.motion.sliding(30), "nothing slides yet");
        // Left of the row's start it stops there.
        drag.to(-40);
        assert_eq!(at(&drag.frame(40))[0], ("One", 0));
    }

    #[test]
    fn a_neighbour_slides_to_the_free_place_rightwards() {
        let mut drag = Drag::press(&four(), true, &ROW4, 0, 30);
        drag.to(80);
        assert_eq!(at(&drag.frame(100)), [("One", 50), ("Two", 102), ("Three", 204), ("Four", 306)]);
        // One pixel more: One covers more than half of Two, and Two slides to place 0.
        drag.to(81);
        assert_eq!(at(&drag.frame(110)), [("Two", 102), ("One", 51), ("Three", 204), ("Four", 306)]);
        assert!(drag.motion.sliding(110));
        assert_eq!(at(&drag.frame(160))[0], ("Two", 26), "three quarters of the way at half the time");
        assert_eq!(at(&drag.frame(210))[0], ("Two", 0), "there after SLIDE_MS");
        assert!(!drag.motion.sliding(210));
        assert_eq!(labels(&drag.groups), ["Two", "One", "Three", "Four"]);
    }

    #[test]
    fn a_neighbour_slides_to_the_free_place_leftwards() {
        let mut drag = Drag::press(&four(), true, &ROW4, 3, 50);
        drag.to(306 + 50 - 50);
        assert_eq!(at(&drag.frame(100)), [("One", 0), ("Two", 102), ("Three", 204), ("Four", 256)]);
        // Its middle crosses the line 305 between places 2 and 3 (at left 254): Three slides right.
        drag.to(254 + 50 - 1);
        assert_eq!(at(&drag.frame(110)), [("One", 0), ("Two", 102), ("Four", 253), ("Three", 204)]);
        assert_eq!(at(&drag.frame(160))[3], ("Three", 280));
        assert_eq!(at(&drag.frame(210))[3], ("Three", 306));
        assert_eq!(labels(&drag.groups), ["One", "Two", "Four", "Three"]);
    }

    #[test]
    fn a_quick_move_slides_several_at_once() {
        let mut drag = Drag::press(&four(), true, &ROW4, 0, 30);
        // From place 0 to the row's end in one move: the three others make room together.
        drag.to(30 + 400);
        let frame = drag.frame(100);
        assert_eq!(at(&frame), [("Two", 102), ("Three", 204), ("Four", 306), ("One", 306)]);
        let frame = drag.frame(150);
        assert_eq!(at(&frame), [("Two", 26), ("Three", 128), ("Four", 230), ("One", 306)]);
        assert!(drag.motion.sliding(150));
        assert_eq!(at(&drag.frame(200)), [("Two", 0), ("Three", 102), ("Four", 204), ("One", 306)]);
    }

    #[test]
    fn a_slide_turns_back_from_where_it_is() {
        let mut drag = Drag::press(&four(), true, &ROW4, 0, 30);
        drag.to(81);
        drag.frame(100);
        assert_eq!(at(&drag.frame(150))[0], ("Two", 26));
        // Back before Two has arrived: it turns round where it is, it does not jump.
        drag.to(79);
        let frame = drag.frame(150);
        assert_eq!(at(&frame), [("One", 49), ("Two", 26), ("Three", 204), ("Four", 306)]);
        assert_eq!(at(&drag.frame(200))[1], ("Two", 26 + 57));
        assert_eq!(at(&drag.frame(250))[1], ("Two", 102));
    }

    #[test]
    fn the_release_slides_the_button_into_its_place_and_keeps_the_order() {
        let mut drag = Drag::press(&four(), true, &ROW4, 0, 30);
        drag.to(30 + 140);
        assert_eq!(at(&drag.frame(100))[1], ("One", 140));
        assert_eq!(drag.release(), Release::Dropped, "a drag's release is no click");
        // Let go at 140, its place is 102: it slides there.
        assert_eq!(at(&drag.frame(120))[1], ("One", 140));
        assert!(drag.motion.sliding(120));
        assert_eq!(at(&drag.frame(170))[1], ("One", 112));
        assert_eq!(at(&drag.frame(220)), [("Two", 0), ("One", 102), ("Three", 204), ("Four", 306)]);
        assert!(!drag.motion.sliding(220));
        assert_eq!(labels(&drag.groups), ["Two", "One", "Three", "Four"]);
    }

    #[test]
    fn esc_or_the_mouse_lost_slides_everything_back() {
        // Esc and a lost mouse capture end a drag the same way (`put_back`).
        let mut drag = Drag::press(&four(), true, &ROW4, 0, 30);
        drag.to(30 + 250);
        drag.frame(100);
        assert_eq!(at(&drag.frame(200)), [("Two", 0), ("Three", 102), ("One", 250), ("Four", 306)]);
        drag.put_back();
        let frame = drag.frame(200);
        assert_eq!(at(&frame), [("One", 250), ("Two", 0), ("Three", 102), ("Four", 306)]);
        assert!(drag.motion.sliding(200));
        assert_eq!(at(&drag.frame(250)), [("One", 63), ("Two", 76), ("Three", 178), ("Four", 306)]);
        assert_eq!(at(&drag.frame(300)), [("One", 0), ("Two", 102), ("Three", 204), ("Four", 306)]);
        assert_eq!(labels(&drag.groups), ["One", "Two", "Three", "Four"]);
    }

    #[test]
    fn a_window_closing_or_opening_mid_drag_does_not_break_it() {
        let mut drag = Drag::press(&four(), true, &ROW4, 0, 30);
        drag.to(30 + 120);
        drag.frame(100);
        assert_eq!(labels(&drag.groups), ["Two", "One", "Three", "Four"]);
        // Two closes mid-slide: the row is laid out anew (three wider places), nothing slides,
        // and the drag goes on where the pointer is: One, at 120 and now 135 wide, is past the
        // line 136 between the new places 0 and 1, so it stays second.
        let three = [(0, 135), (137, 135), (274, 135)];
        drag.windows_now(&[win(1, "One", "A"), win(3, "Three", "C"), win(4, "Four", "D")], &three);
        assert_eq!(at(&drag.frame(120)), [("Three", 0), ("One", 120), ("Four", 274)]);
        assert!(!drag.motion.sliding(120));
        // A new window comes: it joins at the end, where it simply appears.
        let more = [win(1, "One", "A"), win(3, "Three", "C"), win(4, "Four", "D"), win(5, "Five", "E")];
        drag.windows_now(&more, &ROW4);
        assert_eq!(at(&drag.frame(130)), [("Three", 0), ("One", 120), ("Four", 204), ("Five", 306)]);
        // A title changes: nothing moves.
        let mut renamed = more.clone();
        renamed[2].title = "Four (2)".into();
        drag.windows_now(&renamed, &ROW4);
        assert_eq!(at(&drag.frame(140)), [("Three", 0), ("One", 120), ("Four (2)", 204), ("Five", 306)]);
        // The dragged button's own window closes: it is found no more, the drag ends.
        drag.windows_now(&renamed[1..], &three);
        assert_eq!(drag.at(), None);
        assert_eq!(at(&drag.frame(150)), [("Three", 0), ("Four (2)", 137), ("Five", 274)]);
        assert!(!drag.motion.sliding(150));
    }

    #[test]
    fn grouped_the_programs_button_moves_with_all_its_windows() {
        let windows = [win(1, "a", "Notepad"), win(2, "Inbox", "Mail"), win(3, "b", "Notepad"), win(4, "Web", "Edge")];
        let mut drag = Drag::press(&windows, true, &ROW, 0, 30);
        assert_eq!(labels(&drag.groups), ["2 Notepad", "Inbox", "Web"]);
        drag.to(30 + 60);
        assert_eq!(labels(&drag.groups), ["Inbox", "2 Notepad", "Web"]);
        assert_eq!(drag.groups[1].windows, [1, 3], "its windows keep their own order");
        assert_eq!(drag.list.order(), [2, 1, 3, 4]);
        // Without grouping a single window's button moves, out of its program's two.
        let mut drag = Drag::press(&windows, false, &ROW4, 3, 30);
        assert_eq!(labels(&drag.groups), ["a", "Inbox", "b", "Web"]);
        drag.to(102 + 30 - 1);
        assert_eq!(labels(&drag.groups), ["a", "Web", "Inbox", "b"]);
        assert_eq!(at(&drag.frame(100))[1], ("Web", 101));
        // A new window still joins at the end; the order stays while the windows live, and
        // switched to grouping the same windows share their program's button in that order.
        let more = [&windows[..], &[win(5, "Sheet", "Excel")]].concat();
        assert_eq!(labels(&drag.list.update(&more, None)), ["a", "Web", "Inbox", "b", "Sheet"]);
        drag.list.group = true;
        assert_eq!(labels(&drag.list.update(&more, None)), ["2 Notepad", "Web", "Inbox", "Sheet"]);
    }

    #[test]
    fn at_rest_nothing_slides() {
        let mut motion = Motion::default();
        assert_eq!(motion.frame(&["a", "b"], &ROW[..2], None, 0), [0, 102]);
        assert_eq!(motion.frame(&["a", "b"], &ROW[..2], None, 500), [0, 102]);
        assert!(!motion.sliding(500));
        // Cleared, a new order shows at once (the platform clears it at rest).
        motion.clear();
        assert_eq!(motion.frame(&["b", "a"], &ROW[..2], None, 600), [0, 102]);
    }

    #[test]
    fn the_order_from_before_a_restart_comes_back() {
        // Windows gone are dropped, new ones come last.
        let mut list = TaskList::default();
        list.set_order(vec![3, 9, 1]);
        let windows = [win(1, "One", "A"), win(2, "Two", "B"), win(3, "Three", "C")];
        assert_eq!(labels(&list.update(&windows, None)), ["Three", "One", "Two"]);
    }

    #[test]
    fn a_click_minimizes_only_the_window_in_front() {
        let front = ClickState { front: Some(7), ..ClickState::default() };
        assert_eq!(click(&[7], front), Click::Minimize(7));
        assert_eq!(click(&[7], ClickState { front: Some(8), ..front }), Click::Activate(7));
        assert_eq!(click(&[7], ClickState::default()), Click::Activate(7));
        assert_eq!(click(&[], front), Click::Nothing);
    }

    #[test]
    fn a_minimized_window_is_never_in_front() {
        // Minimized, yet still Windows' foreground window: the click brings it back.
        let state = ClickState { front: Some(7), minimized: true, list_was_open: false };
        assert_eq!(click(&[7], state), Click::Activate(7));
        assert_eq!(click(&[7], ClickState { front: None, ..state }), Click::Activate(7));
    }

    #[test]
    fn a_second_click_closes_the_list() {
        let state = ClickState::default();
        assert_eq!(click(&[1, 2], state), Click::OpenList);
        assert_eq!(click(&[1, 2], ClickState { list_was_open: true, ..state }), Click::Nothing);
        // Which window is in front does not matter for a list.
        assert_eq!(click(&[1, 2], ClickState { front: Some(1), ..state }), Click::OpenList);
    }

    fn win(id: u64, title: &str, app: &str) -> WindowInfo {
        WindowInfo {
            id,
            title: title.into(),
            group: format!("C:\\{app}.exe"),
            app_name: app.into(),
            flashing: false,
            classic: None,
        }
    }

    #[test]
    fn one_button_per_app_labelled_like_windows() {
        let mut list = TaskList::default();
        let windows =
            [win(1, "a.txt - Notepad", "Notepad"), win(2, "Inbox", "Mail"), win(3, "b.txt - Notepad", "Notepad")];
        let groups = list.update(&windows, None);
        assert_eq!(groups.len(), 2);
        assert_eq!((groups[0].label.as_str(), groups[0].windows.as_slice()), ("2 Notepad", &[1, 3][..]));
        assert_eq!(groups[1].label, "Inbox");
    }

    #[test]
    fn buttons_keep_their_place() {
        let mut list = TaskList::default();
        list.update(&[win(1, "One", "A"), win(2, "Two", "B")], None);
        // Windows report in a different order and a new one appears: old ones stay put.
        let groups = list.update(&[win(3, "Three", "C"), win(2, "Two", "B"), win(1, "One", "A")], None);
        let labels: Vec<&str> = groups.iter().map(|g| g.label.as_str()).collect();
        assert_eq!(labels, ["One", "Two", "Three"]);
    }

    #[test]
    fn closing_and_reopening_moves_an_app_to_the_end() {
        let mut list = TaskList::default();
        list.update(&[win(1, "One", "A"), win(2, "Two", "B")], None);
        list.update(&[win(2, "Two", "B")], None);
        let groups = list.update(&[win(2, "Two", "B"), win(4, "One again", "A")], None);
        let labels: Vec<&str> = groups.iter().map(|g| g.label.as_str()).collect();
        assert_eq!(labels, ["Two", "One again"]);
    }

    #[test]
    fn without_grouping_every_window_has_its_own_button() {
        let mut list = TaskList::new(false);
        let windows =
            [win(1, "a.txt - Notepad", "Notepad"), win(2, "Inbox", "Mail"), win(3, "b.txt - Notepad", "Notepad")];
        let groups = list.update(&windows, Some(3));
        let labels: Vec<&str> = groups.iter().map(|g| g.label.as_str()).collect();
        assert_eq!(labels, ["a.txt - Notepad", "Inbox", "b.txt - Notepad"], "in the order they appeared");
        assert!(groups.iter().all(|g| g.windows.len() == 1));
        let states: Vec<TaskState> = groups.iter().map(|g| g.state).collect();
        assert_eq!(states, [TaskState::Normal, TaskState::Normal, TaskState::Active], "only the window in front");
        // A click never opens a list: it brings the one window forward or minimizes it.
        assert_eq!(click(&groups[0].windows, ClickState::default()), Click::Activate(1));
        assert_eq!(click(&groups[2].windows, ClickState { front: Some(3), ..ClickState::default() }), Click::Minimize(3));
        // Switched on again, the same windows share their program's button, in the same order.
        list.group = true;
        let groups = list.update(&windows, None);
        assert_eq!((groups.len(), groups[0].label.as_str()), (2, "2 Notepad"));
    }

    #[test]
    fn active_beats_flashing() {
        let mut list = TaskList::default();
        let mut flashing = win(2, "Chat", "Chat");
        flashing.flashing = true;
        let windows = [win(1, "Doc", "Word"), flashing];
        let states: Vec<TaskState> = list.update(&windows, Some(1)).iter().map(|g| g.state).collect();
        assert_eq!(states, [TaskState::Active, TaskState::Flashing]);
        let states: Vec<TaskState> = list.update(&windows, Some(2)).iter().map(|g| g.state).collect();
        assert_eq!(states, [TaskState::Normal, TaskState::Active]);
    }
}
