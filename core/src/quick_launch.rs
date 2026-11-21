//! Quick Launch: the band of small one-click icons right of the Start button that Internet
//! Explorer 4 brought to Windows 95 and that Windows 98 and 2000 had from the start
//! (docs/decisions.md, "Quick Launch").
//!
//! The pictures themselves are Windows' and kept in bevel-art (`art/src/quick_launch.rs`); this
//! module chooses them.

use crate::art::Look;
use crate::fb::{Icon, Rect};
use crate::icons::ClassicIcon;
use crate::paint::{Menu, MenuEntry};
use crate::theme::Era;

/// The icons Bevel95 puts on the band itself (the person's own shortcuts follow them).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Builtin {
    /// Starts the person's web browser (98's and 2000's "Launch Internet Explorer Browser").
    InternetExplorer,
    /// Starts the person's mail program ("Launch Outlook Express").
    OutlookExpress,
    /// Windows' own Show Desktop: every window away, and back at the next click.
    ShowDesktop,
    /// Bevel95's own addition: its Windows Explorer, as the Programs menu's entry starts it.
    WindowsExplorer,
}

impl Builtin {
    /// Every built-in icon.
    pub const ALL: [Builtin; 4] = [Builtin::InternetExplorer, Builtin::OutlookExpress, Builtin::ShowDesktop, Builtin::WindowsExplorer];

    /// Its name in quick-launch.txt (the built-in icons the person took off the band).
    pub fn key(self) -> &'static str {
        match self {
            Builtin::InternetExplorer => "internet-explorer",
            Builtin::OutlookExpress => "outlook-express",
            Builtin::ShowDesktop => "show-desktop",
            Builtin::WindowsExplorer => "windows-explorer",
        }
    }

    pub fn from_key(key: &str) -> Option<Builtin> {
        Builtin::ALL.into_iter().find(|b| b.key().eq_ignore_ascii_case(key.trim()))
    }

    /// Its tooltip in `era`: the originals' words (the Reference PCs: the names of the files in their
    /// Quick Launch folders; Windows 2000 adds a shortcut's description as a second line, which only
    /// its Internet Explorer shortcut has); Windows Explorer's is Bevel95's own.
    pub fn tooltip(self, era: Era) -> &'static str {
        match self {
            Builtin::InternetExplorer if era == Era::Win2000 => {
                "Launch Internet Explorer Browser\nFinds and displays information and Web sites on the Internet."
            }
            Builtin::InternetExplorer => "Launch Internet Explorer Browser",
            Builtin::OutlookExpress => "Launch Outlook Express",
            Builtin::ShowDesktop => "Show Desktop",
            Builtin::WindowsExplorer => "Windows Explorer",
        }
    }
}

/// The built-in icons of `era`, left to right: each look's own order (Windows 98's Internet
/// Explorer, Outlook Express, Show Desktop; Windows 2000's Show Desktop first), then Windows
/// Explorer. Windows 95 had no Quick Launch: switched on, it shows the icons 95's own files have a
/// picture for (its globe for the browser, its desktop for Show Desktop; no mail picture).
pub fn builtins(era: Era) -> &'static [Builtin] {
    use Builtin::*;
    match era {
        Era::Win95 => &[InternetExplorer, ShowDesktop, WindowsExplorer],
        Era::Win98 => &[InternetExplorer, OutlookExpress, ShowDesktop, WindowsExplorer],
        Era::Win2000 => &[ShowDesktop, InternetExplorer, OutlookExpress, WindowsExplorer],
    }
}

/// `builtin`'s picture in `era` (True Color; `colors16` finds its 16-color drawing), `None` where
/// that Windows had none.
pub fn picture(era: Era, builtin: Builtin) -> Option<Icon> {
    use Builtin::*;
    let art = match (era, builtin) {
        (_, WindowsExplorer) => return Some(crate::folder::tree_art::explorer(era)),
        (Era::Win95, ShowDesktop) => return Some(crate::folder::tree_art::w95(crate::folder::tree_art::W95::Desktop)),
        (Era::Win95, InternetExplorer) => &W95_INTERNET,
        (Era::Win95, OutlookExpress) => return None,
        (Era::Win98, InternetExplorer) => &W98_INTERNET,
        (Era::Win98, OutlookExpress) => &W98_OUTLOOK,
        (Era::Win98, ShowDesktop) => &W98_SHOW_DESKTOP,
        (Era::Win2000, InternetExplorer) => &W2K_INTERNET,
        (Era::Win2000, OutlookExpress) => &W2K_OUTLOOK,
        (Era::Win2000, ShowDesktop) => &W2K_SHOW_DESKTOP,
    };
    Some(art.icon())
}

/// Whether Quick Launch shows in `era` while settings.txt does not say (`quick_launch`): on in
/// 98 and 2000 (every capture of them has it), off in 95 (plain Windows 95 had none).
pub fn on_by_default(era: Era) -> bool {
    era != Era::Win95
}

/// One icon on the band.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Item {
    Builtin(Builtin),
    /// The person's own shortcut: an index into the list of their files (`shown_files`).
    Shortcut(usize),
}

/// The band's icons, left to right: `era`'s built-in icons (those not taken off, `removed`, and
/// those `available` says can be started: no mail program, no Outlook Express), then the person's
/// `shortcuts` (already in their order). Every one of them: icons the band has no room for are
/// reached by 98's arrows or through 2000's "»" list (brief 70; before, at most 10 showed).
pub fn items(era: Era, removed: &[Builtin], available: impl Fn(Builtin) -> bool, shortcuts: usize) -> Vec<Item> {
    builtins(era)
        .iter()
        .copied()
        .filter(|b| !removed.contains(b) && available(*b))
        .map(Item::Builtin)
        .chain((0..shortcuts).map(Item::Shortcut))
        .collect()
}

/// A press on an icon, held until the left button comes up (the review before 0.1.2, B2). The icon
/// is remembered by what it is (`target`: the platform's own name for it, a built-in icon or the
/// person's shortcut by its file), not only by its place, so a band that changes meanwhile (the
/// folder read again, the look or the band switched) can never make the release start another
/// icon. Nothing but the release (and the bar losing the mouse) ends it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Held<T> {
    pub index: usize,
    pub target: T,
}

impl<T: PartialEq> Held<T> {
    /// The place drawn sunk now: the place pressed, while the icon there (`at(i)`: what icon `i` is
    /// now) is still the one pressed; else none.
    pub fn sunk(&self, at: impl Fn(usize) -> Option<T>) -> Option<usize> {
        (at(self.index).as_ref() == Some(&self.target)).then_some(self.index)
    }
}

/// What the left button's release does on the bar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release<T> {
    /// The bar gives the mouse back (it took it at the press): always when an icon was held,
    /// whatever happened to the band meanwhile.
    pub give_back: bool,
    /// What starts: the icon pressed, and only if that very icon is under the pointer now.
    pub start: Option<T>,
}

/// The release of `held` (the press, if any) with `under` the icon under the pointer now (`None`:
/// none, also when the band has gone).
pub fn release<T: PartialEq>(held: Option<Held<T>>, under: Option<T>) -> Release<T> {
    match held {
        None => Release { give_back: false, start: None },
        Some(h) => Release { give_back: true, start: (under.as_ref() == Some(&h.target)).then_some(h.target) },
    }
}

/// One entry of Windows' own Quick Launch folder, as the platform read it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderEntry {
    pub name: String,
    pub hidden: bool,
    pub folder: bool,
}

/// The two shortcuts today's Windows keeps in the Quick Launch folder for its own taskbar
/// ("Shows Desktop.lnk" and "Window Switcher.lnk": today's pictures, not classic ones; Show Desktop
/// is on the band already).
const WINDOWS_OWN: [&str; 2] = ["Shows Desktop.lnk", "Window Switcher.lnk"];

/// Which entries of the folder show on the band, A to Z by name (as Windows compares file names):
/// the files directly in it, without Windows' own two, `desktop.ini`, hidden files and folders.
pub fn shown_files(entries: &[FolderEntry]) -> Vec<String> {
    let mut names: Vec<String> = entries
        .iter()
        .filter(|e| !e.folder && !e.hidden)
        .filter(|e| !e.name.eq_ignore_ascii_case("desktop.ini"))
        .filter(|e| !WINDOWS_OWN.iter().any(|own| crate::text::same_name(own, &e.name)))
        .map(|e| e.name.clone())
        .collect();
    names.sort_by_key(|n| n.chars().flat_map(char::to_uppercase).collect::<String>());
    names
}

/// What a shortcut's tooltip says: its name without the extension (as Windows' band showed the
/// file names: "Launch Internet Explorer Browser").
pub fn shortcut_tooltip(name: &str) -> &str {
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && (1..=4).contains(&ext.len()) => stem,
        _ => name,
    }
}

/// The first line of quick-launch.txt.
pub const STATE_HEAD: &str = "# Bevel95: Quick Launch: the band's width if you dragged it, and the built-in icons taken off the band (one per line).";

/// What quick-launch.txt keeps: the built-in icons taken off the band, and the band's width when
/// the person dragged it (`width = 131`: from its first etched line to its last, in the look's own
/// pixels, 1x; `None`: never dragged, as wide as its icons).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Saved {
    pub removed: Vec<Builtin>,
    pub width: Option<i32>,
}

/// The widths quick-launch.txt may hold: from the band without icons up to far more than any
/// screen. Anything else is nonsense, and the band is as wide as its icons.
pub const WIDTH_SAVED: std::ops::RangeInclusive<i32> = EMPTY_W..=10_000;

/// What quick-launch.txt says (lines it does not know are skipped; a width that is not a whole
/// number in `WIDTH_SAVED` counts as none).
pub fn parse_state(text: &str) -> Saved {
    let mut saved = Saved::default();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if let Some((key, value)) = line.split_once('=')
            && key.trim().eq_ignore_ascii_case("width")
        {
            saved.width = value.trim().parse::<i32>().ok().filter(|w| WIDTH_SAVED.contains(w));
            continue;
        }
        if let Some(b) = Builtin::from_key(line)
            && !saved.removed.contains(&b)
        {
            saved.removed.push(b);
        }
    }
    saved
}

/// The built-in icons taken off the band, read from quick-launch.txt.
pub fn parse_removed(text: &str) -> Vec<Builtin> {
    parse_state(text).removed
}

/// quick-launch.txt for `saved`.
pub fn state_text(saved: &Saved) -> String {
    let mut text = format!("{STATE_HEAD}\r\n");
    if let Some(w) = saved.width {
        text.push_str(&format!("width = {w}\r\n"));
    }
    for b in Builtin::ALL.iter().filter(|b| saved.removed.contains(b)) {
        text.push_str(b.key());
        text.push_str("\r\n");
    }
    text
}

/// What the band shows (in `paint::Bar`): its icons' pictures (`None`: a shortcut without one
/// yet, its button empty), the icon under the pointer (raised) and the one held down (sunken);
/// its width and 98's scroll position (`Shape`), and its overflow control under the pointer, held
/// down, and 2000's list open.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Band {
    pub icons: Vec<Option<Icon>>,
    pub hot: Option<usize>,
    pub pressed: Option<usize>,
    /// The width the person gave the band (`Saved::width`); `None`: as wide as its icons.
    pub width: Option<i32>,
    /// Windows 98's scroll position (`Shape::scroll`).
    pub scroll: i32,
    pub hot_control: Option<Control>,
    pub pressed_control: Option<Control>,
    /// Windows 2000's list of the icons not shown is open (its "»" is drawn pushed in).
    pub list_open: bool,
}

impl Band {
    /// The band's shape for the layout.
    pub fn shape(&self) -> Shape {
        let held = self.pressed_control.filter(|c| matches!(c, Control::Left | Control::Right));
        Shape { icons: self.icons.len(), width: self.width, scroll: self.scroll, held }
    }
}

/// The overflow controls at the band's ends (brief 70, `private/probes/quick-launch/resize.md`):
/// Windows 98's two small arrow buttons (its pager), Windows 2000's "»" (its chevron).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Control {
    Left,
    Right,
    Chevron,
}

/// What the layout needs to know of the band beyond its number of icons.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Shape {
    pub icons: usize,
    /// The width the person gave it (`None`: as wide as its icons).
    pub width: Option<i32>,
    /// Windows 98's scroll position: how far the icons are moved left (clamped by the layout).
    pub scroll: i32,
    /// A 98 arrow held down: it stays drawn (grayed) while held, also once the end is reached.
    pub held: Option<Control>,
}

impl Shape {
    /// `icons` icons, never dragged, not scrolled.
    pub fn all(icons: usize) -> Shape {
        Shape { icons, ..Shape::default() }
    }
}

/// A place on the band the mouse can act on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Icon(usize),
    Control(Control),
    /// The grip between the band and the task buttons: dragged, it makes the band narrower or wider.
    Grip,
}

/// Where the band's parts are on the bar (1x pixels), measured on the Reference PCs' captures
/// (docs/theme-spec.md, "Quick Launch"): an etched line after the Start button, a grip, the icons'
/// buttons, a second etched line and grip, then the task buttons; on Windows 98's bar also an
/// etched line before the notification area.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    /// The band from its first line to where the task buttons begin (the band's own part: its
    /// menu opens on it).
    pub rect: Rect,
    /// The etched line after the Start button (its dark column) and the grip's left column.
    pub line: i32,
    pub grip: i32,
    /// One 23x22 button per icon, side by side, where each is drawn: Windows 98's (its pager) for
    /// every icon, moved left by the scroll position, partly or wholly outside `view` when they do
    /// not fit; Windows 2000's only for the icons shown (the others are `hidden`).
    pub buttons: Vec<Rect>,
    /// Where icons can be seen: x from `view.0` to `view.1` (not included). An icon at its edge is
    /// drawn cut (98).
    pub view: (i32, i32),
    /// Windows 98's arrows while there are icons to scroll to (or one is held down), Windows 2000's
    /// "»" while icons are left out.
    pub left: Option<Rect>,
    pub right: Option<Rect>,
    pub chevron: Option<Rect>,
    /// Windows 2000: the icons left out, in the band's order (its "»" lists them). Windows 98 and a
    /// band too narrow for its control: the icons that cannot be reached.
    pub hidden: Vec<usize>,
    /// Windows 98's scroll position (clamped) and the furthest it goes (0: nothing to scroll).
    pub scroll: i32,
    pub max_scroll: i32,
    /// The band's width from its first etched line to its last (`end_line - line`), and how
    /// narrow and how wide the grip may drag it.
    pub width: i32,
    pub min_width: i32,
    pub max_width: i32,
    /// The etched line and grip in front of the task buttons.
    pub end_line: i32,
    pub end_grip: i32,
    /// Where the task buttons may go: from `rect`'s right edge to here.
    pub tasks_right: i32,
    /// Windows 98's etched line before the notification area (the 95 look takes 98's rule);
    /// Windows 2000 has none.
    pub tray_line: Option<i32>,
}

impl Layout {
    /// The icon under (x, y) (only where it can be seen).
    pub fn button_at(&self, x: i32, y: i32) -> Option<usize> {
        if x < self.view.0 || x >= self.view.1 {
            return None;
        }
        self.buttons.iter().position(|r| r.contains(x, y))
    }

    /// The part of icon `i`'s button that can be seen.
    pub fn visible_button(&self, i: usize) -> Option<Rect> {
        let r = *self.buttons.get(i)?;
        let (x0, x1) = (r.x.max(self.view.0), r.right().min(self.view.1));
        (x1 > x0).then(|| Rect::new(x0, r.y, x1 - x0, r.h))
    }

    /// The grip between the band and the task buttons, with the etched line before it and the
    /// gaps around it: where the pointer becomes the two-sided arrow and a press starts a drag.
    pub fn grip_zone(&self) -> Rect {
        Rect::new(self.end_line, self.rect.y, self.rect.right() - self.end_line, self.rect.h)
    }

    /// What of the band is under (x, y): an icon, an overflow control, the grip.
    pub fn part_at(&self, x: i32, y: i32) -> Option<Part> {
        for (c, r) in [(Control::Left, self.left), (Control::Right, self.right), (Control::Chevron, self.chevron)] {
            if r.is_some_and(|r| r.contains(x, y)) {
                return Some(Part::Control(c));
            }
        }
        if let Some(i) = self.button_at(x, y) {
            return Some(Part::Icon(i));
        }
        self.grip_zone().contains(x, y).then_some(Part::Grip)
    }
}

/// From the Start button's right edge to the band's first etched line: 98 (and the 95 look) 2,
/// 2000 3 (its Start button is one wider; the line is at 58 and 60 on both PCs).
fn line_after_start(era: Era) -> i32 {
    if era == Era::Win2000 { 3 } else { 2 }
}
/// The grip from the line; the first button from the line; a button's width (the icons' step);
/// the icon in its button; the line after the last button; the task buttons after that line.
const GRIP_FROM_LINE: i32 = 4;
const FIRST_BUTTON_FROM_LINE: i32 = 11;
pub const BUTTON_W: i32 = 23;
pub const ICON_IN: i32 = 3;
const END_LINE_AFTER_BUTTONS: i32 = 4;
const TASKS_FROM_END_LINE: i32 = 11;
/// The grip: 3 wide, 2 rows below the row's top, 18 tall.
pub const GRIP_W: i32 = 3;
pub const GRIP_DY: i32 = 2;
pub const GRIP_H: i32 = 18;

/// The band without icons: its two etched lines with the first grip and the gaps between.
pub const EMPTY_W: i32 = FIRST_BUTTON_FROM_LINE + END_LINE_AFTER_BUTTONS;
/// The narrowest a band with icons to hide can be dragged (resize.md, 2: from its first line to its
/// last): Windows 98 (and the 95 look) L+131, four whole icons, the fifth cut and the arrow;
/// Windows 2000 L+119, four whole icons, 4 columns of face and the "»". Measured as constants on
/// both Reference PCs (98: 8 and 12 icons, two starting widths; 2000: 5, 7 and 11 icons).
const MIN_HIDING_98: i32 = 131;
const MIN_HIDING_2000: i32 = 119;
/// An overflow control: 12 wide, the row's height. Windows 98's right arrow ends 4 before the end
/// line (x line-16 .. line-5), its left arrow is where the first icon would start (L+11); Windows
/// 2000's "»" ends just before the end line (line-12 .. line-1).
pub const CONTROL_W: i32 = 12;
const RIGHT_ARROW_FROM_END: i32 = 16;
const PAGER_END_FROM_END_LINE: i32 = 4;
const CHEVRON_FROM_END: i32 = 12;
/// Windows 2000 shows an icon only if its button ends at least 4 face columns before the "»"
/// (its right edge at most line-17: resize.md, measured to the pixel at lines 270 and 271).
const CHEVRON_ROOM: i32 = 4;

/// The band as wide as all `icons` icons (the band Windows makes at first; Bevel95's for a person
/// who never dragged it).
pub fn natural_width(icons: usize) -> i32 {
    EMPTY_W + icons as i32 * BUTTON_W
}

/// How narrow the grip may drag a band of `icons` icons in `era`: never narrower than all its
/// icons, unless there are more than the look's minimum holds; then down to that minimum (room for
/// 4 icons and the overflow control). Windows never let the band show 3, 2, 1 or no icon.
pub fn min_width(era: Era, icons: usize) -> i32 {
    let hiding = if era == Era::Win2000 { MIN_HIDING_2000 } else { MIN_HIDING_98 };
    natural_width(icons).min(hiding)
}

/// The width the band takes in `era` before the bar's room is counted: as wide as its `icons`
/// icons while the person never dragged it (`kept` is `None`); else the width they gave it.
/// Icons arriving later: Windows 98's band keeps its width (the new icons are cut at the edge and
/// the arrow comes); Windows 2000's grows at once up to its minimum (`min_width`), no further.
pub fn wanted_width(era: Era, icons: usize, kept: Option<i32>) -> i32 {
    let w = match kept {
        None => natural_width(icons),
        Some(w) if era == Era::Win2000 => w.max(min_width(era, icons)),
        Some(w) => w,
    };
    w.max(EMPTY_W)
}

/// Windows 2000's band grown by icons arriving (`wanted_width`): the width to remember now, so it
/// keeps it when icons go again (resize.md, 4: it kept its width with 5 icons left). `None`: no
/// change (98, never dragged, or wide enough already).
pub fn grown(era: Era, icons: usize, kept: Option<i32>) -> Option<i32> {
    let w = kept?;
    let min = min_width(era, icons);
    (era == Era::Win2000 && w < min).then_some(min)
}

/// How wide the band may be on a bar whose Start button is `start` and whose notification area is
/// `well` (the review before 0.1.2, B3): it leaves the task area room for one task button at the
/// look's full width plus the scroll arrows (`task`). Never less than the band without icons.
pub fn max_width(era: Era, start: Rect, well: Rect, task: &crate::theme::TaskMetrics) -> i32 {
    let empty = layout(era, start, well, 0, 0, 0);
    let room = empty.tasks_right - empty.rect.right();
    let need = task.width + task.spacing + task.scroll_w;
    EMPTY_W + (room - need).max(0)
}

/// Windows 98's scroll position after one press of the right arrow (from `at`, at most `max`). After
/// n presses icon n starts just after the left arrow (L+23), so the icons are moved left by 23n-12;
/// the last press goes only as far as the end (resize.md, 3).
pub fn scroll_right(at: i32, max: i32) -> i32 {
    let n = (at + CONTROL_W) / BUTTON_W + 1;
    (n * BUTTON_W - CONTROL_W).min(max).max(0)
}

/// Windows 98's scroll position after one press of the left arrow (from `at`): one icon back; at
/// the first icon, the start.
pub fn scroll_left(at: i32) -> i32 {
    let n = (at + CONTROL_W + BUTTON_W - 1) / BUTTON_W - 1;
    if n < 1 { 0 } else { n * BUTTON_W - CONTROL_W }
}

/// The band for `icons` icons on a bar whose Start button is `start` and whose notification area
/// is `well`, as wide as its icons and with no limit; `y`, `h`: the row of buttons (the bar's
/// `item_y`, `item_h`).
pub fn layout(era: Era, start: Rect, well: Rect, icons: usize, y: i32, h: i32) -> Layout {
    layout_shape(era, start, well, &Shape::all(icons), None, y, h)
}

/// The band of `shape` (its icons, the width the person gave it, 98's scroll position) on a bar
/// whose Start button is `start` and whose notification area is `well`, never wider than leaves
/// the task area its room (`task`; `None`: no limit). `y`, `h`: the row of buttons.
pub fn layout_shape(era: Era, start: Rect, well: Rect, shape: &Shape, task: Option<&crate::theme::TaskMetrics>, y: i32, h: i32) -> Layout {
    let line = start.right() + line_after_start(era);
    let first = line + FIRST_BUTTON_FROM_LINE;
    let n = shape.icons;
    let max_w = task.map_or(i32::MAX / 4, |t| max_width(era, start, well, t));
    let width = wanted_width(era, n, shape.width).min(max_w).max(EMPTY_W);
    let end_line = line + width;
    let at = |i: usize, x0: i32| Rect::new(x0 + i as i32 * BUTTON_W, y, BUTTON_W, h);
    let control = |x: i32| Rect::new(x, y, CONTROL_W, h);
    let buttons: Vec<Rect>;
    let mut view = (first, end_line - PAGER_END_FROM_END_LINE);
    let (mut left, mut right, mut chevron) = (None, None, None);
    let (mut hidden, mut scroll, mut max_scroll) = (Vec::new(), 0, 0);
    // How many whole icons fit with no control at all (a band too narrow for its control).
    let whole = |room: i32| ((room / BUTTON_W).max(0) as usize).min(n);
    if natural_width(n) <= width {
        // Every icon fits: the band as it always was, with empty face after the icons if wider.
        buttons = (0..n).map(|i| at(i, first)).collect();
    } else if era == Era::Win2000 {
        if width >= EMPTY_W + CONTROL_W + CHEVRON_ROOM {
            // Whole icons only: one shows if it ends at least 4 columns before the "»".
            let k = whole(width - EMPTY_W - CONTROL_W - CHEVRON_ROOM + END_LINE_AFTER_BUTTONS);
            buttons = (0..k).map(|i| at(i, first)).collect();
            chevron = Some(control(end_line - CHEVRON_FROM_END));
            hidden = (k..n).collect();
        } else {
            let k = whole(width - EMPTY_W);
            buttons = (0..k).map(|i| at(i, first)).collect();
            hidden = (k..n).collect();
        }
    } else {
        // Windows 98's pager: from the first icon's place to 4 before the end line.
        let pager = width - EMPTY_W;
        if pager > 2 * CONTROL_W {
            max_scroll = n as i32 * BUTTON_W - pager;
            scroll = shape.scroll.clamp(0, max_scroll);
            let left_on = scroll > 0 || shape.held == Some(Control::Left);
            let right_on = scroll < max_scroll || shape.held == Some(Control::Right);
            buttons = (0..n).map(|i| at(i, first - scroll)).collect();
            if left_on {
                left = Some(control(first));
                view.0 = first + CONTROL_W;
            }
            if right_on {
                right = Some(control(end_line - RIGHT_ARROW_FROM_END));
                view.1 = end_line - RIGHT_ARROW_FROM_END;
            }
        } else {
            let k = whole(pager);
            buttons = (0..k).map(|i| at(i, first)).collect();
            hidden = (k..n).collect();
        }
    }
    let tasks_left = end_line + TASKS_FROM_END_LINE;
    // The task area ends 7 before the well on 98 (its line 4 before it), 6 before it on 2000.
    let (tasks_right, tray_line) = if era == Era::Win2000 { (well.x - 6, None) } else { (well.x - 7, Some(well.x - 4)) };
    // A bar too narrow for its parts: the task area is empty, never of a negative width.
    let tasks_right = tasks_right.max(tasks_left);
    Layout {
        rect: Rect::new(line, y, tasks_left - line, h),
        line,
        grip: line + GRIP_FROM_LINE,
        buttons,
        view,
        left,
        right,
        chevron,
        hidden,
        scroll,
        max_scroll,
        width,
        min_width: min_width(era, n).min(max_w),
        max_width: max_w,
        end_line,
        end_grip: end_line + GRIP_FROM_LINE,
        tasks_right,
        tray_line,
    }
}

/// Dragging the grip between the band and the task buttons (resize.md, 1): the band's end follows
/// the pointer while the button is held. Windows 2000 ignores a move of under 4 pixels and then
/// takes the whole move (a drag threshold); Windows 98's was not measured and has none here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WidthDrag {
    /// Where the press was (1x, in the bar) and the band's width then.
    pub press_x: i32,
    pub from: i32,
    pub moved: bool,
}

/// Windows 2000's drag threshold on the band's grip.
pub const DRAG_THRESHOLD: i32 = 4;

impl WidthDrag {
    pub fn new(press_x: i32, from: i32) -> WidthDrag {
        WidthDrag { press_x, from, moved: false }
    }

    /// The band's width with the pointer at `x`, between `min` and `max` (`Layout::min_width`,
    /// `max_width`); `None` while the pointer has not moved far enough yet. A band narrower than
    /// its minimum (98's, icons having arrived) jumps to it at the first move, as 98's did.
    pub fn width_at(&mut self, era: Era, x: i32, min: i32, max: i32) -> Option<i32> {
        let dx = x - self.press_x;
        if !self.moved {
            let threshold = if era == Era::Win2000 { DRAG_THRESHOLD } else { 1 };
            if dx.abs() < threshold {
                return None;
            }
            self.moved = true;
        }
        Some((self.from + dx).max(min).min(max).max(EMPTY_W))
    }
}

/// The band's width as the person sets it: kept between runs (`Saved::width`), and a drag of the
/// grip while it lasts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sizing {
    /// The width the person gave the band (`None`: never dragged).
    pub kept: Option<i32>,
    drag: Option<WidthDrag>,
}

impl Sizing {
    pub fn new(kept: Option<i32>) -> Sizing {
        Sizing { kept, drag: None }
    }

    /// A press on the grip at `x` with the band `shown` wide now.
    pub fn press(&mut self, x: i32, shown: i32) {
        self.drag = Some(WidthDrag::new(x, shown));
    }

    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// The pointer moved to `x` while the button is held: true when the width changed (repaint).
    pub fn move_to(&mut self, era: Era, x: i32, min: i32, max: i32) -> bool {
        let Some(drag) = self.drag.as_mut() else {
            return false;
        };
        match drag.width_at(era, x, min, max) {
            Some(w) if self.kept != Some(w) => {
                self.kept = Some(w);
                true
            }
            _ => false,
        }
    }

    /// The button came up, or the bar lost the mouse: the drag ends where it is and the band keeps
    /// the width reached. Returns that width when the drag moved it (to be saved), else `None`.
    pub fn end(&mut self) -> Option<i32> {
        let drag = self.drag.take()?;
        drag.moved.then_some(self.kept).flatten()
    }
}

/// What the band's icons are called in 2000's list and their tooltips: the built-in icons' first
/// line, the person's shortcuts their file name without its extension.
pub fn item_name(era: Era, item: Item, shortcut_names: &[String]) -> String {
    match item {
        Item::Builtin(b) => b.tooltip(era).lines().next().unwrap_or_default().to_string(),
        Item::Shortcut(i) => shortcut_names.get(i).map(|n| shortcut_tooltip(n).to_string()).unwrap_or_default(),
    }
}

/// Windows 2000's list of the icons left out (resize.md, 3): one row per icon not shown, in the
/// band's order, its 16-pixel picture and its name (no access keys: an "&" is shown as it is); as
/// wide as the longest name of ALL the band's icons, shown or not (`all_names`), as Windows 2000's
/// was (216 with "Launch Internet Explorer Browser" on the band, 170 once it was renamed "IE").
/// Drawn as Windows 2000's cascading menus are (`MenuStyle::Cascade`): the same frame, 20-pixel
/// rows, picture 5 in, name 30 in, navy under the pointer.
pub fn list_menu(rows: Vec<(String, Option<Icon>)>, all_names: &[String]) -> Menu {
    let entries = rows.into_iter().map(|(name, icon)| MenuEntry::item(&crate::text::escape_access_keys(&name), icon)).collect();
    Menu { width_labels: all_names.iter().map(|n| crate::text::escape_access_keys(n)).collect(), ..Menu::new(entries) }
}

/// What a click on row `row` of 2000's list starts: the icon that row was made for (`list`, the
/// icons by what they are), and only while it is still on the band (`on_band`); never what is at
/// its place now ("Remembered by what it is, never by where it is").
pub fn list_choice<T: Clone>(list: &[T], row: usize, on_band: impl Fn(&T) -> bool) -> Option<T> {
    list.get(row).filter(|t| on_band(t)).cloned()
}

/// Where 2000's list opens (screen pixels), `w` x `h` on `screen`, for the "»" at `chevron` (screen
/// pixels): its left edge at the chevron's, its bottom row just above the chevron's top, so it
/// overlaps the bar's top rows (resize.md, 3). Not measured: a bar at the top of the screen (then
/// it opens below the chevron) and a list that would run past the screen's right edge (then it
/// moves left). A list taller than the room on either side starts at the screen's top (then it
/// runs into further columns, as every long menu of the bar does).
pub fn place_list(chevron: Rect, w: i32, h: i32, screen: Rect) -> (i32, i32) {
    let mut y = chevron.y - h;
    if y < screen.y {
        y = if chevron.bottom() + h <= screen.bottom() { chevron.bottom() } else { screen.y };
    }
    let x = chevron.x.min(screen.right() - w).max(screen.x);
    (x, y)
}

/// An entry of the band's own menu (a right-click on its grips, lines and gaps).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BandEntry {
    View,
    ShowText,
    Refresh,
    Open,
    ShowTitle,
    Toolbars,
    AdjustDateTime,
    Cascade,
    TileHorizontally,
    TileVertically,
    MinimizeAll,
    TaskManager,
    Properties,
    Close,
}

/// The band's menu as the Reference PCs show it (98: `win98-truecolor-ql-bandmenu-grip`; 2000:
/// `-bandmenu-grip`, which adds Adjust Date/Time after Toolbars and Task Manager... before
/// Properties). `None`: a line.
fn band_entries(era: Era) -> Vec<Option<(&'static str, BandEntry)>> {
    use BandEntry::*;
    let mut e = vec![
        Some(("&View", View)),
        Some(("Show Te&xt", ShowText)),
        Some(("&Refresh", Refresh)),
        Some(("&Open", Open)),
        None,
        Some(("Sho&w Title", ShowTitle)),
        None,
        Some(("&Toolbars", Toolbars)),
        None,
    ];
    if era == Era::Win2000 {
        e.push(Some(("&Adjust Date/Time", AdjustDateTime)));
    }
    e.extend([
        Some(("Ca&scade Windows", Cascade)),
        Some(("Tile Windows &Horizontally", TileHorizontally)),
        Some(("Tile Windows &Vertically", TileVertically)),
        None,
        Some(("&Minimize All Windows", MinimizeAll)),
        None,
    ]);
    if era == Era::Win2000 {
        e.extend([Some(("Tas&k Manager...", TaskManager)), None]);
    }
    e.extend([Some(("P&roperties", Properties)), None, Some(("&Close", Close))]);
    e
}

impl BandEntry {
    /// What Bevel95 carries out: Refresh, Open, the Toolbars submenu, Adjust Date/Time, Properties
    /// and Close. The others (the band's View, Show Text, Show Title; the window arrangements and
    /// Task Manager, which Bevel95's bar menu leaves out) are shown grayed.
    pub fn works(self) -> bool {
        use BandEntry::*;
        matches!(self, Refresh | Open | Toolbars | AdjustDateTime | Properties | Close)
    }
}

/// The band's menu in `era`, nothing highlighted.
pub fn band_menu(era: Era) -> Menu {
    let entries = band_entries(era)
        .into_iter()
        .map(|e| match e {
            None => MenuEntry::separator(),
            Some((label, entry)) => {
                let mut m = if matches!(entry, BandEntry::View | BandEntry::Toolbars) {
                    MenuEntry::submenu(label, None)
                } else {
                    MenuEntry::item(label, None)
                };
                m.disabled = !entry.works();
                // Windows 2000 draws its Adjust Date/Time bold (the default of the clock's menu).
                m.bold = entry == BandEntry::AdjustDateTime;
                m
            }
        })
        .collect();
    Menu::new(entries)
}

/// What entry `index` of `band_menu(era)` is.
pub fn band_entry(era: Era, index: usize) -> Option<BandEntry> {
    band_entries(era).get(index).copied().flatten().map(|(_, e)| e)
}

/// An entry of the menu on an icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconEntry {
    Open,
    Delete,
    Properties,
}

/// The menu on an icon (first pass): Open (bold, the default), Delete, and for the person's
/// shortcut its Properties (Windows' own window). The originals' other entries (Quick View, Send
/// To, Cut, Copy, Create Shortcut; 2000's Rename) are left out for now. `None`: a line.
fn icon_entries(shortcut: bool) -> Vec<Option<(&'static str, IconEntry)>> {
    let mut e = vec![Some(("&Open", IconEntry::Open)), None, Some(("&Delete", IconEntry::Delete))];
    if shortcut {
        e.extend([None, Some(("P&roperties", IconEntry::Properties))]);
    }
    e
}

/// The menu on an icon (`shortcut`: one of the person's own), nothing highlighted.
pub fn icon_menu(shortcut: bool) -> Menu {
    let entries = icon_entries(shortcut)
        .into_iter()
        .map(|e| match e {
            None => MenuEntry::separator(),
            Some((label, entry)) => MenuEntry { bold: entry == IconEntry::Open, ..MenuEntry::item(label, None) },
        })
        .collect();
    Menu::new(entries)
}

/// What entry `index` of `icon_menu(shortcut)` is.
pub fn icon_entry(shortcut: bool, index: usize) -> Option<IconEntry> {
    icon_entries(shortcut).get(index).copied().flatten().map(|(_, e)| e)
}

/// Windows 98's and 2000's bar with Quick Launch switched off (the Reference PCs'
/// `<pc>-truecolor-ql-off-<n>`): the etched line after the Start button and one grip stay (now the
/// task band's own), the task buttons start 7 after the grip (L+11: x 69 on 98, 71 on 2000), the
/// task area ends as with the band (7 before the well on 98, its line 4 before it; 6 on 2000).
/// Windows 95 had no bands: `None`, and its bar stays as it always was.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskBand {
    pub line: i32,
    pub grip: i32,
    /// Where the task buttons go: from `left` to `right`.
    pub left: i32,
    pub right: i32,
    pub tray_line: Option<i32>,
}

/// The task band of `era`'s bar without Quick Launch (see `TaskBand`).
pub fn task_band(era: Era, start: Rect, well: Rect) -> Option<TaskBand> {
    if era == Era::Win95 {
        return None;
    }
    // The same rules as with the band (`layout`), with no band between the line and the tasks.
    let band = layout(era, start, well, 0, 0, 0);
    Some(TaskBand { line: band.line, grip: band.grip, left: band.line + FIRST_BUTTON_FROM_LINE, right: band.tasks_right, tray_line: band.tray_line })
}

static W95_INTERNET: ClassicIcon = ClassicIcon::art("quick-launch/internet-explorer", Look::Win95);
static W98_INTERNET: ClassicIcon = ClassicIcon::art("quick-launch/internet-explorer", Look::Win98);
static W98_OUTLOOK: ClassicIcon = ClassicIcon::art("quick-launch/outlook-express", Look::Win98);
static W98_SHOW_DESKTOP: ClassicIcon = ClassicIcon::art("quick-launch/show-desktop", Look::Win98);
static W2K_INTERNET: ClassicIcon = ClassicIcon::art("quick-launch/internet-explorer", Look::Win2000);
static W2K_OUTLOOK: ClassicIcon = ClassicIcon::art("quick-launch/outlook-express", Look::Win2000);
static W2K_SHOW_DESKTOP: ClassicIcon = ClassicIcon::art("quick-launch/show-desktop", Look::Win2000);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{Depth, theme_in};

    #[test]
    fn every_icon_of_a_look_has_its_pictures() {
        for era in Era::ALL {
            for &b in builtins(era) {
                let icon = picture(era, b).unwrap_or_else(|| panic!("{era:?} {b:?}"));
                assert_eq!((icon.width, icon.height), (16, 16), "{era:?} {b:?}");
                assert!(icon.pixels.iter().any(|p| p >> 24 == 0), "{era:?} {b:?} has see-through pixels");
                // 98 and 2000 have their own 16-color drawing of each of their pictures.
                if era != Era::Win95 {
                    let sixteen = crate::colors16_art::own(era, &icon);
                    assert!(sixteen.is_some(), "{era:?} {b:?} has its 16-color drawing");
                    let _ = theme_in(era, Depth::Sixteen);
                }
            }
        }
        assert!(picture(Era::Win95, Builtin::OutlookExpress).is_none(), "95's files have no mail picture");
    }

    /// The Reference PCs' bars (800 wide, the bar's row from y 4): Windows 98's default band of four
    /// icons (`win98-truecolor-tasks-1`: line 58/59, grip 62..64, icons at 72, 95, 118, 141, line
    /// 165/166, grip 169..171, the first task button at 176, the task area 517 wide with the well at
    /// 700, its line at 696), Windows 2000's of three (line 60, grip 64, icons 74, 97, 120, line
    /// 144, grip 148, tasks from 155, 566 wide with the well at 727), and the hot and pressed button
    /// of `win98-truecolor-ql-hover-ie-at-once` / `-pressed-ie` (x 69..91, y 576..597).
    #[test]
    fn the_band_sits_where_the_reference_pcs_have_it() {
        let b = layout(Era::Win98, Rect::new(2, 4, 54, 22), Rect::new(700, 4, 98, 22), 4, 4, 22);
        assert_eq!((b.line, b.grip, b.end_line, b.end_grip, b.rect.right()), (58, 62, 165, 169, 176));
        let icons: Vec<i32> = b.buttons.iter().map(|r| r.x + ICON_IN).collect();
        assert_eq!(icons, [72, 95, 118, 141]);
        assert_eq!(b.buttons[0], Rect::new(69, 4, 23, 22));
        assert_eq!((b.tasks_right - b.rect.right(), b.tray_line), (517, Some(696)));
        let b = layout(Era::Win2000, Rect::new(2, 4, 55, 22), Rect::new(727, 4, 71, 22), 3, 4, 22);
        assert_eq!((b.line, b.grip, b.end_line, b.end_grip, b.rect.right()), (60, 64, 144, 148, 155));
        let icons: Vec<i32> = b.buttons.iter().map(|r| r.x + ICON_IN).collect();
        assert_eq!(icons, [74, 97, 120]);
        assert_eq!((b.tasks_right - b.rect.right(), b.tray_line), (566, None));
        // The 95 look by 98's rule.
        assert_eq!(layout(Era::Win95, Rect::new(2, 4, 54, 22), Rect::new(700, 4, 98, 22), 4, 4, 22).line, 58);
        assert_eq!(b.button_at(97, 10), Some(1));
        assert_eq!(b.button_at(70, 10), None, "the gap before the first button");
    }

    #[test]
    fn the_items_are_the_looks_own_then_the_persons() {
        use Builtin::*;
        let all = |_: Builtin| true;
        let items = items(Era::Win2000, &[], all, 2);
        assert_eq!(items, [Item::Builtin(ShowDesktop), Item::Builtin(InternetExplorer), Item::Builtin(OutlookExpress), Item::Builtin(WindowsExplorer), Item::Shortcut(0), Item::Shortcut(1)]);
        // Taken off, or no mail program: left out.
        let items = super::items(Era::Win98, &[InternetExplorer], |b| b != OutlookExpress, 0);
        assert_eq!(items, [Item::Builtin(ShowDesktop), Item::Builtin(WindowsExplorer)]);
        // No cap any more (brief 70): every shortcut is on the band.
        let items = super::items(Era::Win98, &[], all, 40);
        assert_eq!(items.len(), 44);
        assert_eq!(items[43], Item::Shortcut(39));
    }

    #[test]
    fn the_folder_shows_the_persons_files_a_to_z() {
        let e = |name: &str, hidden: bool, folder: bool| FolderEntry { name: name.into(), hidden, folder };
        let entries = [
            e("zebra.lnk", false, false),
            e("desktop.ini", true, false),
            e("Desktop.ini", false, false),
            e("Shows Desktop.lnk", false, false),
            e("window switcher.LNK", false, false),
            e("User Pinned", false, true),
            e("secret.lnk", true, false),
            e("apple.lnk", false, false),
            e("Banana.url", false, false),
        ];
        assert_eq!(shown_files(&entries), ["apple.lnk", "Banana.url", "zebra.lnk"]);
        assert_eq!(shortcut_tooltip("Banana.url"), "Banana");
        assert_eq!(shortcut_tooltip("no extension"), "no extension");
    }

    #[test]
    fn the_icons_taken_off_are_remembered() {
        use Builtin::*;
        let text = state_text(&Saved { removed: vec![WindowsExplorer, InternetExplorer], width: None });
        assert!(text.starts_with(STATE_HEAD));
        assert!(!text.contains("width ="), "a band never dragged writes no width");
        assert_eq!(parse_removed(&text), [InternetExplorer, WindowsExplorer]);
        assert_eq!(parse_removed("show-desktop\r\nnonsense\r\nshow-desktop # twice\r\n"), [ShowDesktop]);
        assert_eq!(parse_removed(""), []);
        // The file of Bevel95 0.1.3 (its old first line, no width) is read as before.
        let old = "# Bevel95: the built-in Quick Launch icons taken off the band (one per line).\r\nshow-desktop\r\n";
        assert_eq!(parse_state(old), Saved { removed: vec![ShowDesktop], width: None });
    }

    /// The band's width is kept in quick-launch.txt beside the icons taken off; an unreadable or
    /// nonsense value is no width at all (the band as wide as its icons).
    #[test]
    fn the_width_is_kept_and_nonsense_falls_back_to_all_the_icons() {
        use Builtin::*;
        let saved = Saved { removed: vec![ShowDesktop], width: Some(131) };
        let text = state_text(&saved);
        assert!(text.contains("width = 131\r\n"));
        assert_eq!(parse_state(&text), saved);
        for (line, width) in [
            ("width = 200", Some(200)),
            ("Width=15", Some(15)),
            ("width = 14", None),
            ("width = -3", None),
            ("width = 99999", None),
            ("width = wide", None),
            ("width = 12.5", None),
            ("width =", None),
            ("width = 140 # dragged", Some(140)),
        ] {
            assert_eq!(parse_state(line).width, width, "{line}");
        }
        // Bevel95 0.1.3 reading the new file skips the width line (its parser knows only icons).
        assert_eq!(Builtin::from_key("width = 131"), None);
        // A width read as nonsense: the band is as wide as all its icons.
        assert_eq!(wanted_width(Era::Win98, 5, parse_state("width = x").width), natural_width(5));
    }

    /// Resize.md, 2: the narrowest band per look and number of icons.
    #[test]
    fn the_narrowest_band_per_look_and_icon_count() {
        for era in Era::ALL {
            for n in 0..=4 {
                assert_eq!(min_width(era, n), natural_width(n), "{era:?}, {n} icons: never narrower than its icons");
            }
        }
        // 98 (and the 95 look): 4 whole icons, the fifth cut, the arrow: line = L + 131. With 5
        // icons all of them fit in 130, so the band cannot hide any (98 with 5 icons was not tried).
        assert_eq!(min_width(Era::Win98, 5), 130);
        for n in [6, 8, 12, 40] {
            assert_eq!(min_width(Era::Win98, n), 131, "{n} icons");
            assert_eq!(min_width(Era::Win95, n), 131, "{n} icons");
        }
        // 2000: 4 whole icons, 4 columns of face, the "»": line = L + 119, from 5 icons.
        for n in [5, 7, 11, 40] {
            assert_eq!(min_width(Era::Win2000, n), 119, "{n} icons");
        }
    }

    fn bar98(shape: Shape) -> Layout {
        layout_shape(Era::Win98, Rect::new(2, 4, 54, 22), Rect::new(700, 4, 98, 22), &shape, None, 4, 22)
    }
    fn bar2000(shape: Shape) -> Layout {
        layout_shape(Era::Win2000, Rect::new(2, 4, 55, 22), Rect::new(727, 4, 71, 22), &shape, None, 4, 22)
    }

    /// Windows 98's band with more icons than room (resize.md, 2 and 3; screen x = 1x x here, L = 58).
    #[test]
    fn windows_98_cuts_the_icon_at_the_edge_and_scrolls_one_icon_per_press() {
        // The default band (line 165) with 8 icons: the arrow at 149..160, icons seen up to 148; the
        // 4th icon (at 138) shows only its first 11 columns, its picture 143..148.
        let b = bar98(Shape { icons: 8, width: Some(107), ..Shape::default() });
        assert_eq!((b.end_line, b.right, b.left), (165, Some(Rect::new(149, 4, 12, 22)), None));
        assert_eq!(b.view, (69, 149));
        assert_eq!(b.buttons[3], Rect::new(138, 4, 23, 22));
        assert_eq!(b.visible_button(3), Some(Rect::new(138, 4, 11, 22)));
        assert_eq!(b.visible_button(4), None);
        assert_eq!(b.button_at(150, 10), None, "the arrow is not an icon");
        assert_eq!(b.part_at(150, 10), Some(Part::Control(Control::Right)));
        assert_eq!(b.button_at(145, 10), Some(3), "the cut icon is pressed on its part that shows");
        // Its narrowest: line 189, the 5th icon cut at 172 (its picture 164..172), the arrow 173..184.
        let b = bar98(Shape { icons: 8, width: Some(131), ..Shape::default() });
        assert_eq!((b.end_line, b.right.unwrap().x, b.view.1), (189, 173, 173));
        assert_eq!(b.visible_button(4), Some(Rect::new(161, 4, 12, 22)));
        // The presses: one icon each (23, 23, 23, 23, then the last 12); after n presses icon n
        // starts at L+23 = 81, just after the left arrow (69..80).
        let max = 8 * 23 - (107 - EMPTY_W);
        assert_eq!(max, 92);
        let mut at = 0;
        let mut seen = Vec::new();
        while at < max {
            at = scroll_right(at, max);
            seen.push(at);
        }
        assert_eq!(seen, [11, 34, 57, 80, 92]);
        for (n, &at) in seen.iter().enumerate().take(4) {
            let b = bar98(Shape { icons: 8, width: Some(107), scroll: at, held: None });
            assert_eq!(b.buttons[n + 1].x, 81, "after {} presses", n + 1);
            assert_eq!((b.left, b.view.0), (Some(Rect::new(69, 4, 12, 22)), 81));
        }
        // At the end: the last icon ends at line - 5 (160), no right arrow, the view to 160.
        let b = bar98(Shape { icons: 8, width: Some(107), scroll: 92, held: None });
        assert_eq!((b.buttons[7].right() - 1, b.right, b.view.1), (160, None, 161));
        // Held down at the end, the arrow stays (grayed) until the release.
        let b = bar98(Shape { icons: 8, width: Some(107), scroll: 92, held: Some(Control::Right) });
        assert_eq!(b.right, Some(Rect::new(149, 4, 12, 22)));
        // Back: one icon per press; from the first press's place, back at the start, no left arrow.
        assert_eq!(scroll_left(92), 80);
        assert_eq!(scroll_left(80), 57);
        assert_eq!(scroll_left(34), 11);
        assert_eq!(scroll_left(11), 0);
        let b = bar98(Shape { icons: 8, width: Some(107), ..Shape::default() });
        assert_eq!(b.left, None);
        // A scroll position past the end (the band made wider) is the end.
        let b = bar98(Shape { icons: 8, width: Some(131), scroll: 92, held: None });
        assert_eq!(b.scroll, b.max_scroll);
        // Wide enough for every icon: no arrows, nothing to scroll.
        let b = bar98(Shape { icons: 8, width: Some(220), scroll: 40, held: None });
        assert_eq!((b.left, b.right, b.scroll, b.view), (None, None, 0, (69, 274)));
        // Never dragged: as wide as its icons, exactly as before.
        assert_eq!(bar98(Shape::all(8)), layout(Era::Win98, Rect::new(2, 4, 54, 22), Rect::new(700, 4, 98, 22), 8, 4, 22));
        assert_eq!(bar98(Shape::all(8)).end_line, 58 + natural_width(8));
    }

    /// Windows 2000's band: whole icons or none, the "»" at the end (resize.md, 2), and which icons
    /// its list holds.
    #[test]
    fn windows_2000_leaves_out_what_does_not_fit_and_lists_it() {
        // 7 icons at the minimum (line 179): 4 icons, the "»" at 167..178, the other 3 in the list.
        let b = bar2000(Shape { icons: 7, width: Some(119), ..Shape::default() });
        assert_eq!((b.end_line, b.buttons.len(), b.chevron), (179, 4, Some(Rect::new(167, 4, 12, 22))));
        assert_eq!(b.hidden, [4, 5, 6]);
        assert_eq!(b.part_at(170, 10), Some(Part::Control(Control::Chevron)));
        // The rule to the pixel (11 icons, L = 60): line 271 shows the 8th button (232..254), 270 not.
        let b = bar2000(Shape { icons: 11, width: Some(271 - 60), ..Shape::default() });
        assert_eq!((b.buttons.len(), b.buttons[7].right() - 1), (8, 254));
        assert_eq!(b.hidden, [8, 9, 10]);
        let b = bar2000(Shape { icons: 11, width: Some(270 - 60), ..Shape::default() });
        assert_eq!(b.buttons.len(), 7);
        assert_eq!(b.hidden, [7, 8, 9, 10]);
        // A band kept narrower than its minimum grows to it (icons arrived: 3 to 7), no further.
        let b = bar2000(Shape { icons: 7, width: Some(natural_width(3)), ..Shape::default() });
        assert_eq!(b.end_line, 179);
        assert_eq!(grown(Era::Win2000, 7, Some(natural_width(3))), Some(119));
        assert_eq!(grown(Era::Win2000, 7, Some(150)), None);
        assert_eq!(grown(Era::Win2000, 7, None), None, "a band never dragged follows its icons");
        assert_eq!(grown(Era::Win98, 8, Some(107)), None, "98 keeps its width");
        // 98 keeps the width: the new icons are cut and the arrow comes.
        let b = bar98(Shape { icons: 8, width: Some(107), ..Shape::default() });
        assert_eq!(b.end_line, 165);
        // Icons gone (11 to 5) with the band at 271: it keeps its width, no "»".
        let b = bar2000(Shape { icons: 5, width: Some(211), ..Shape::default() });
        assert_eq!((b.end_line, b.chevron, b.buttons.len()), (271, None, 5));
    }

    /// The grip's drag (resize.md, 1): the end follows the pointer between the minimum and the
    /// bar's room; 2000 waits for 4 pixels; the drag ends when the mouse is lost and keeps its width.
    #[test]
    fn the_grip_drags_the_width_and_the_drag_ends_when_the_mouse_is_lost() {
        // 98, 8 icons, the default band (107) kept: the first move left jumps to the minimum.
        let mut s = Sizing::new(Some(107));
        s.press(170, 107);
        assert!(s.move_to(Era::Win98, 147, 131, 400));
        assert_eq!(s.kept, Some(131));
        assert!(s.move_to(Era::Win98, 210, 131, 400));
        assert_eq!(s.kept, Some(147), "the press was at 170 on a band 107 wide");
        assert!(s.move_to(Era::Win98, 1000, 131, 400));
        assert_eq!(s.kept, Some(400), "never wider than leaves the task area its room");
        // The mouse lost: the drag ends there, the width kept and given to be saved.
        assert_eq!(s.end(), Some(400));
        assert!(!s.dragging());
        assert!(!s.move_to(Era::Win98, 300, 131, 400), "no more following after the end");
        assert_eq!(s.kept, Some(400));
        // A press without a move changes nothing and saves nothing.
        let mut s = Sizing::new(None);
        s.press(170, 107);
        assert_eq!(s.end(), None);
        assert_eq!(s.kept, None);
        // 2000: 2 or 3 pixels do nothing; 4 are taken whole.
        let mut s = Sizing::new(Some(214));
        s.press(274, 214);
        assert!(!s.move_to(Era::Win2000, 272, 119, 500));
        assert!(!s.move_to(Era::Win2000, 271, 119, 500));
        assert_eq!(s.kept, Some(214));
        assert!(s.move_to(Era::Win2000, 270, 119, 500));
        assert_eq!(s.kept, Some(210));
        // Back within the threshold after it was passed: followed to the pixel.
        assert!(s.move_to(Era::Win2000, 273, 119, 500));
        assert_eq!(s.kept, Some(213));
        // With 4 icons or fewer the band never gets narrower than its icons.
        let mut s = Sizing::new(None);
        s.press(170, natural_width(3));
        s.move_to(Era::Win98, 100, min_width(Era::Win98, 3), 400);
        assert_eq!(s.kept, Some(natural_width(3)));
    }

    /// A row of 2000's list starts what it was made for, by what it is, also when the folder changed
    /// meanwhile.
    #[test]
    fn a_row_of_the_list_starts_what_it_was_made_for() {
        let list = ["c.lnk", "d.lnk", "e.lnk"];
        let band_then = ["Show Desktop", "a.lnk", "b.lnk", "c.lnk", "d.lnk", "e.lnk"];
        assert_eq!(list_choice(&list, 1, |t| band_then.contains(t)), Some("d.lnk"));
        // A shortcut added before them meanwhile: every place moved one on; row 1 is still d.lnk.
        let band_now = ["Show Desktop", "0.lnk", "a.lnk", "b.lnk", "c.lnk", "d.lnk", "e.lnk"];
        assert_eq!(list_choice(&list, 1, |t| band_now.contains(t)), Some("d.lnk"));
        // d.lnk deleted meanwhile: nothing starts (not e.lnk, now at its place).
        let deleted = ["Show Desktop", "a.lnk", "b.lnk", "c.lnk", "e.lnk"];
        assert_eq!(list_choice(&list, 1, |t| deleted.contains(t)), None);
        assert_eq!(list_choice(&list, 7, |_| true), None);
        // The list's rows: the names, "&" shown as it is; as wide as the longest name on the band.
        let menu = list_menu(vec![("Tom & Jerry".into(), None)], &["Launch Internet Explorer Browser".into(), "Tom & Jerry".into()]);
        assert_eq!(menu.entries[0].label, "Tom && Jerry");
        assert_eq!(menu.width_labels.len(), 2);
    }

    #[test]
    fn the_list_opens_above_the_chevron() {
        let screen = Rect::new(0, 0, 800, 600);
        assert_eq!(place_list(Rect::new(167, 576, 12, 22), 216, 66, screen), (167, 510));
        // A bar at the top: below the chevron. Near the right edge: moved left.
        assert_eq!(place_list(Rect::new(167, 4, 12, 22), 216, 66, screen), (167, 26));
        assert_eq!(place_list(Rect::new(700, 576, 12, 22), 216, 66, screen), (584, 510));
        // Taller than the room above a bar at the bottom: never below the screen's bottom.
        assert_eq!(place_list(Rect::new(167, 576, 12, 22), 216, 586, screen), (167, 0));
    }

    #[test]
    fn the_menus_are_the_originals_with_what_is_built_working() {
        let labels = |m: &Menu| m.entries.iter().map(|e| if e.label.is_empty() { "-".to_string() } else { e.label.clone() }).collect::<Vec<_>>();
        let m98 = band_menu(Era::Win98);
        assert_eq!(
            labels(&m98),
            ["&View", "Show Te&xt", "&Refresh", "&Open", "-", "Sho&w Title", "-", "&Toolbars", "-", "Ca&scade Windows", "Tile Windows &Horizontally", "Tile Windows &Vertically", "-", "&Minimize All Windows", "-", "P&roperties", "-", "&Close"]
        );
        let working: Vec<_> = (0..m98.entries.len()).filter(|&i| m98.entries[i].choosable()).filter_map(|i| band_entry(Era::Win98, i)).collect();
        use BandEntry::*;
        assert_eq!(working, [Refresh, Open, Toolbars, Properties, Close]);
        let m2k = band_menu(Era::Win2000);
        assert_eq!(band_entry(Era::Win2000, 9), Some(AdjustDateTime));
        assert!(m2k.entries[9].bold);
        assert!(labels(&m2k).contains(&"Tas&k Manager...".to_string()));
        assert_eq!(labels(&band_menu(Era::Win95)), labels(&m98), "the 95 look takes 98's");
        assert_eq!(labels(&icon_menu(false)), ["&Open", "-", "&Delete"]);
        assert_eq!(labels(&icon_menu(true)), ["&Open", "-", "&Delete", "-", "P&roperties"]);
        assert!(icon_menu(true).entries[0].bold);
        assert_eq!((icon_entry(true, 2), icon_entry(true, 4), icon_entry(false, 1)), (Some(IconEntry::Delete), Some(IconEntry::Properties), None));
    }

    /// The review before 0.1.2, B2: a held icon is remembered by what it is, and the release starts
    /// only that very icon, under the pointer; the mouse is always given back.
    #[test]
    fn a_held_icon_starts_only_itself_and_the_mouse_is_always_given_back() {
        // What the platform calls its icons: here simply names.
        let band = ["Show Desktop", "a.lnk", "b.lnk"];
        fn at<'a>(list: &'a [&'static str]) -> impl Fn(usize) -> Option<&'static str> + 'a {
            move |i| list.get(i).copied()
        }
        let held = Held { index: 1, target: "a.lnk" };
        assert_eq!(held.sunk(at(&band)), Some(1));
        // Released over it: it starts.
        assert_eq!(release(Some(held.clone()), Some("a.lnk")), Release { give_back: true, start: Some("a.lnk") });
        // Moved off it: nothing starts, the mouse is given back.
        assert_eq!(release(Some(held.clone()), Some("b.lnk")), Release { give_back: true, start: None });
        assert_eq!(release(Some(held.clone()), None), Release { give_back: true, start: None });
        // The folder changed during the press, the same number of icons: another shortcut is now
        // at the place pressed. It is not drawn sunk, and a release there starts nothing.
        let changed = ["Show Desktop", "c.lnk", "b.lnk"];
        assert_eq!(held.sunk(at(&changed)), None);
        assert_eq!(release(Some(held.clone()), changed.get(1).copied()), Release { give_back: true, start: None });
        // The icon pressed moved one place on (a shortcut added before it): the release over it
        // still starts it, the one pressed.
        let moved = ["Show Desktop", "0.lnk", "a.lnk"];
        assert_eq!(release(Some(held.clone()), moved.get(2).copied()), Release { give_back: true, start: Some("a.lnk") });
        // The band switched off, or the look changed so that it has gone: nothing under the
        // pointer; the mouse is given back all the same.
        assert_eq!(release(Some(held), None::<&str>), Release { give_back: true, start: None });
        // No icon held: the release is not the band's.
        assert_eq!(release(None, Some("a.lnk")), Release { give_back: false, start: None });
    }

    #[test]
    fn each_look_keeps_its_own_order() {
        use Builtin::*;
        assert_eq!(builtins(Era::Win98), [InternetExplorer, OutlookExpress, ShowDesktop, WindowsExplorer]);
        assert_eq!(builtins(Era::Win2000), [ShowDesktop, InternetExplorer, OutlookExpress, WindowsExplorer]);
        for b in Builtin::ALL {
            assert_eq!(Builtin::from_key(b.key()), Some(b));
        }
    }
}
