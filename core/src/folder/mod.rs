//! A Windows 95 folder window (Explorer's "cabinet" window without its tree): My Computer or an
//! ordinary folder, its icons large, a menu bar and a status bar, one window per folder. Measured
//! on the Reference PC (`private/reference/refs/vm/win95-truecolor-folder-*.png`, numbers in docs/theme-spec.md,
//! "Folder windows"):
//!
//! - The frame is a sizable window's (4 pixels), the menu bar Windows' own, the icons stand in
//!   Windows' list view (a sunken edge of 2 pixels, white inside) on the same 75-pixel grid as the
//!   desktop's, in rows from the top left, as wide as the list was when they were arranged. A
//!   narrower window keeps them where they are and shows scroll bars (`*-pf-narrow`); Auto Arrange
//!   puts them in rows again at every change of size.
//! - The status bar (19 rows) has two sunken parts: 144 pixels for the count, the rest for the size
//!   or a drive's free space, a size grip in the corner; while a menu entry is highlighted it shows
//!   that entry's help text across its whole width, without parts (`*-mc-menu-file`).
//! - The icons themselves, selecting them, the menu on them, renaming and the file commands are
//!   the classic desktop's (`desktop_icons` with `Host::Window`, `desktop`), so they do here what
//!   they do there.
//!
//! The platform layer reads the folder, opens what is opened, carries out the menus' commands and
//! keeps each folder's window place; this module draws the window and turns the mouse and keys
//! into changes and `Part`s to carry out.

pub mod address;
pub mod art;
pub mod explorer;
pub mod explorer_art;
pub mod history;
pub mod menus;
pub mod options;
pub mod pane;
pub mod saved;
pub mod status;
pub mod tree_art;
pub mod web;

use std::collections::HashMap;

use crate::desktop_icons::{Button as IconButton, Desktop, EditKey, HEADER_H, HeaderPart, Item, Key as IconKey, Mods as IconMods, Picture, ROW_H, ViewMode};
use crate::fb::{Framebuffer, Icon, Rect};
use crate::kit::{Button, Effect, Form, Key, Mods, Pointer, Press};
use crate::text::{Font, Fonts};
use crate::theme::{Era, Theme};
use crate::window::{self, TitleButton, TitleLook};

pub use explorer::{ListRow, Tool};
pub use history::{History, Place, Step};
pub use menus::Command;
use explorer::Chrome;
use status::Status;

/// The status bar's height: 2 face rows above its 17-row parts.
const STATUS_H: i32 = 19;
/// Its first part's width, and the gap before the second.
const STATUS_LEFT_W: i32 = 144;
const STATUS_GAP: i32 = 2;
/// A scroll bar's width (and a horizontal one's height), and how far one click on an arrow scrolls.
const BAR: i32 = 16;
const LINE: i32 = 15;
/// One turn of the mouse wheel (Windows 95 had none; three of its lines).
const WHEEL: i32 = 3 * LINE;
/// The list view's margin past its icons, right and down.
const MARGIN: i32 = 2;

/// Windows' virtual keys the window knows besides the kit's own.
const VK_BACK: u16 = 0x08;
const VK_APPS: u16 = 0x5D;
const VK_F2: u16 = 0x71;
const VK_F5: u16 = 0x74;
const VK_F10: u16 = 0x79;
const VK_F6: u16 = 0x75;
const VK_L: u16 = 0x4C;
/// The value a press in the rename box grabs the list with: its drag selects text (brief 95).
const RENAME_DRAG: i32 = i32::MIN;
const VK_ADD: u16 = 0x6B;
const VK_SUBTRACT: u16 = 0x6D;
const VK_MULTIPLY: u16 = 0x6A;

/// What takes clicks in the window, and what the platform carries out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Minimize,
    Maximize,
    Close,
    /// The title bar's picture: the window menu.
    Icon,
    /// The title bar clicked with the right button, or Alt+Space: the window menu there.
    TitleMenu,
    KeyMenu,
    /// A menu bar title: its menu opens.
    Menu(usize),
    /// The icons' list (pressed: the selection rectangle follows the pointer).
    List,
    VThumb,
    HThumb,
    /// The scroll bars' arrows and the track beside the thumb.
    Scroll(Scroll),
    /// Open the selected icons (a double-click, Enter).
    Open,
    /// The menu on the selected icons, or the empty area's menu, at `menu_at`.
    ItemMenu,
    BackgroundMenu,
    /// A command from a key.
    Command(Command),
    /// Renaming ended with the name typed (`true`: by Enter).
    RenameEnd(bool),
    /// 98 and 2000: a toolbar button, and the little arrow beside Back and Forward (its list of
    /// places opens).
    Tool(Tool),
    ToolArrow(Tool),
    /// The address bar's field, its arrow button (the list opens or closes), a row of the open
    /// list (chosen), 2000's Go button and Enter in the field (go to what was typed).
    Address,
    AddressButton,
    ListRow(usize),
    Go,
    /// Explorer's tree: a row (an index into its nodes) and its plus/minus box, the empty part
    /// below the rows, its scroll bars, 98's and 2000's close button, the bar between the panes.
    TreeRow(usize),
    TreeBox(usize),
    TreeBlank,
    TreeVThumb,
    TreeHThumb,
    TreeScroll(Scroll),
    TreeClose,
    Split,
    /// The tree's selection was moved by the mouse or Enter: the icons show its place at once;
    /// by a key: after a pause (the platform's timer), as Windows' Explorer did.
    TreeChosen,
    TreeMoved,
    /// The menu on a tree row (`menu_row`), at `menu_at`.
    TreeMenu,
    /// Details' header: a column's button (a click sorts by it) and the divider at a column's right
    /// edge (dragged: the column's width; double-clicked: fitted to its widest entry).
    Header(usize),
    Divider(usize),
    /// The web view panel (98, 2000): a link, its scroll bar's arrows and pages.
    WebLink(web::Link),
    WebScroll(Scroll),
    /// A drag of icons has begun: the platform hands it to Windows' drag and drop.
    DragOut,
}

/// Where a drag of files over a folder window would drop (`FolderWindow::drop_at`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropAt {
    /// On icon `i` (a folder, a drive, the Recycle Bin, My Documents, My Computer).
    Item(usize),
    /// In the folder the window shows.
    Here,
    /// On row `i` of the tree (an index into its nodes).
    Row(usize),
    Nowhere,
}

/// A scroll bar's arrows and pages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scroll {
    Up,
    Down,
    PageUp,
    PageDown,
    Left,
    Right,
    PageLeft,
    PageRight,
}

/// What a My Computer drive shows in the status bar when it is the one selected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Space {
    pub free: u64,
    pub capacity: u64,
}

/// A folder window.
#[derive(Clone, Debug)]
pub struct FolderWindow {
    pub era: Era,
    pub title: String,
    /// The title bar's 16×16 picture.
    pub icon: Icon,
    /// A folder of files (My Computer is not).
    pub files: bool,
    /// The icons.
    pub view: Desktop,
    size: (i32, i32),
    pub maximized: bool,
    active: bool,
    pressed: Option<Part>,
    /// The menu bar title shown chosen (its menu is open).
    pub chosen: Option<usize>,
    pub status_bar: bool,
    /// While a menu entry is highlighted: its help text, across the whole status bar.
    pub help: Option<String>,
    /// How far the list is scrolled.
    scroll: (i32, i32),
    /// Where the window is on the screen (1×), for the focus rectangles' dots.
    screen: (i32, i32),
    /// Where a menu opened by the mouse or a key goes (window pixels at 1×).
    pub menu_at: (i32, i32),
    /// The right button went down on an icon.
    right_hit: bool,
    /// Files Explorer does not show here, and the size of every file in the folder (hidden ones
    /// too, as Windows 95 counted it).
    pub hidden: usize,
    pub total_size: u64,
    /// My Computer's drives: free space and size, by item id.
    pub spaces: HashMap<String, Space>,
    /// The keys held at the last press (the kit's host tells them before it).
    mods: Mods,
    /// Where a scroll bar's thumb was grabbed.
    grab_at: (i32, i32),
    font: Option<Font>,
    menu_rects: Vec<Rect>,
    /// 98 and 2000: the rebar's places, what its buttons show, where the window can go, the
    /// address bar's field and picture, and its open list.
    chrome: Chrome,
    pub bar: explorer::State,
    pub nav: menus::Nav,
    pub field: address::Field,
    pub place_icon: Option<Icon>,
    pub list: Option<(Vec<ListRow>, Option<usize>)>,
    /// Explorer: the tree of all folders, whether it shows, how wide its pane is (1×), whether it
    /// has the keyboard, the window was opened as Explorer (95 and 98: "Exploring - ..." in the
    /// title), the place's own name (95's "Contents of '...'"), the drive's free space (the
    /// status bar).
    pub tree: pane::Tree,
    pub tree_shown: bool,
    pub tree_width: i32,
    pub tree_focus: bool,
    pub exploring: bool,
    pub place_name: String,
    pub disk_free: Option<u64>,
    /// The row the menu on a row was opened for, by its id: rows read while the menu is open move
    /// the rows below, so an index would name another folder when the menu closes (the third
    /// safety review of 2026-11-18, A2). `menu_node` finds it again, or nothing once it is gone.
    pub menu_row: Option<String>,
    /// The bar between the panes being dragged: the pointer's x; 98's and 2000's close button
    /// under the pointer (1) or pressed (2); where a tree's scroll thumb was grabbed.
    split_drag: Option<i32>,
    close_look: u8,
    tree_grab: (i32, i32),
    /// Details: the divider being dragged (its column, the pointer's x where it was grabbed and
    /// now); 95 opened this window in List (its columns 112 wide).
    divider_drag: Option<(usize, i32, i32)>,
    pub list_fixed: bool,
    /// The order shown came from a header click (a second click on it turns it round).
    sorted_by_header: bool,
    /// 98 and 2000: the web view panel, and the link under the pointer (its words in the status bar).
    pub web: web::Web,
    link_hot: Option<web::Link>,
    /// The item the last left press in the icons landed on: a double-click opens only when its
    /// second press lands on the same item (two clicks on two items select the second).
    first_press: Option<String>,
    /// A press on the name of the one item already selected: unless a double-click or a drag
    /// follows, its name becomes a box to type in (`slow_rename_due`). The item and where the
    /// press was.
    slow_rename: Option<(String, i32, i32)>,
    /// The window has just come forward by a click (`activated_by_click`): that click's press
    /// arms no slow rename, as on the Reference PCs (survey part 4; brief 90).
    click_activated: bool,
    /// A press outside the rename box ended the rename; once the platform has taken the name it
    /// goes on as a click on that place (`click_after_rename`): where, with which keys.
    after_rename: Option<(i32, i32, IconMods)>,
    /// The left press began in the rename box: its drag selects text, and its release is not the
    /// list's (brief 97).
    rename_drag: bool,
    /// A right press ended a rename (and was dropped): its release opens no menu (brief 98).
    skip_right_up: bool,
    /// The press elsewhere (its part and point) that ended a rename, carried on once the rename has
    /// ended without the extension warning (`carry_on_after_rename`, brief 99).
    press_after_rename: Option<(Part, i32, i32)>,
    /// A drag of icons began and waits to be handed over (`Form::pending`).
    drag_out: bool,
    /// What Undo would take back now (the platform reads the shared list before a menu opens).
    pub undo: Option<crate::drag::Step>,
}

/// Where the web view puts its panel: beside the icons, 98's band above them, or nowhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WebArea {
    None,
    Panel(Rect),
    Banner(Rect),
}

/// The status bar's height in `era` (95: 19 rows, 98 and 2000: 20).
fn status_h(era: Era) -> i32 {
    if era == Era::Win95 { STATUS_H } else { explorer::STATUS_H }
}

impl FolderWindow {
    /// A window of `size` (1×) titled `title`, for a folder of files or My Computer (`files`).
    pub fn new(era: Era, title: &str, icon: Icon, files: bool, size: (i32, i32)) -> FolderWindow {
        let mut w = FolderWindow {
            era,
            title: title.to_string(),
            icon,
            files,
            view: Desktop::in_window(era, Rect::new(0, 0, 0, 0)),
            size,
            maximized: false,
            active: true,
            pressed: None,
            chosen: None,
            status_bar: true,
            help: None,
            scroll: (0, 0),
            screen: (0, 0),
            menu_at: (0, 0),
            right_hit: false,
            hidden: 0,
            total_size: 0,
            spaces: HashMap::new(),
            mods: Mods::default(),
            grab_at: (0, 0),
            font: None,
            menu_rects: Vec::new(),
            chrome: Chrome::default(),
            bar: explorer::State::default(),
            nav: menus::Nav::default(),
            field: address::Field::default(),
            place_icon: None,
            list: None,
            tree: pane::Tree::new(),
            tree_shown: false,
            tree_width: pane::default_width(era),
            tree_focus: false,
            exploring: false,
            place_name: title.to_string(),
            disk_free: None,
            menu_row: None,
            split_drag: None,
            close_look: 0,
            tree_grab: (0, 0),
            divider_drag: None,
            list_fixed: false,
            sorted_by_header: false,
            web: web::Web::default(),
            link_hot: None,
            first_press: None,
            slow_rename: None,
            click_activated: false,
            after_rename: None,
            rename_drag: false,
            skip_right_up: false,
            press_after_rename: None,
            drag_out: false,
            undo: None,
        };
        w.view.active = true;
        w.view.keep_order(!files);
        w.update_grayed();
        w
    }

    /// 98 and 2000: the window has Explorer's rebar (toolbar, address bar).
    /// Whether the window is drawn as the one in front.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Where a drag of files over the window at (x, y) would drop: on an icon that takes files,
    /// in the folder shown (the icons' empty area, or an icon that takes none), on a row of the
    /// tree, or nowhere (the title bar, the toolbar...). `dragged`: the dragged items' ids.
    pub fn drop_at(&self, x: i32, y: i32, dragged: &[String]) -> DropAt {
        match self.hit(x, y) {
            Some(Part::List) => match self.font().and_then(|f| self.view.drop_target(&f, x, y, dragged)) {
                Some(i) => DropAt::Item(i),
                None => DropAt::Here,
            },
            Some(Part::TreeRow(i) | Part::TreeBox(i)) if i < self.tree.nodes.len() => DropAt::Row(i),
            _ => DropAt::Nowhere,
        }
    }

    /// Shows where a drag would drop (the icon or the row drawn selected); `Nowhere` shows
    /// nothing. True if anything changed.
    pub fn show_drop(&mut self, at: DropAt) -> bool {
        let hover = match at {
            DropAt::Item(i) => Some(i),
            _ => None,
        };
        let row = match at {
            DropAt::Row(i) => Some(i),
            _ => None,
        };
        let changed = self.view.drop_hover != hover || self.tree.drop_row != row;
        self.view.drop_hover = hover;
        self.tree.drop_row = row;
        changed
    }

    /// A closed row of the tree opens while a drag rests on it (2000: after about a second; the
    /// platform keeps the time). True if it opened.
    pub fn open_drop_row(&mut self, i: usize) -> bool {
        if self.tree.nodes.get(i).is_none_or(|n| n.open || !n.has_rows) {
            return false;
        }
        let page = self.tree_page();
        self.tree.toggle(i, page);
        self.fit_tree();
        true
    }

    /// A drag resting near the top or bottom edge of the tree or of the icons scrolls them a row
    /// (Windows' list and tree views did; the zone, one row, and the pace are Bevel95's: not
    /// measured, PROVISIONAL). True if anything moved.
    pub fn drop_scroll(&mut self, x: i32, y: i32) -> bool {
        const EDGE: i32 = 16;
        if let Some((s, ..)) = self.tree_parts()
            && s.contains(x, y)
        {
            let before = self.tree.top;
            if y < s.y + EDGE {
                self.tree.top = self.tree.top.saturating_sub(1);
            } else if y >= s.bottom() - EDGE {
                self.tree.top += 1;
            }
            self.fit_tree();
            return self.tree.top != before;
        }
        let shown = self.shown();
        if !shown.contains(x, y) {
            return false;
        }
        let before = self.scroll;
        let step = if self.view.mode().small() { ROW_H } else { WHEEL };
        if y < shown.y + EDGE {
            self.scroll_by(0, -step);
        } else if y >= shown.bottom() - EDGE {
            self.scroll_by(0, step);
        }
        self.scroll != before
    }

    /// A slow second click is waiting: the platform renames once the double-click time has gone
    /// by without a double-click (the edit box came about one double-click time after the click
    /// on all three Reference PCs, `private/probes/measure17`, D2).
    pub fn slow_rename_waiting(&self) -> bool {
        self.slow_rename.is_some()
    }

    /// The double-click time went by: the item to rename, if it is still the one selected and
    /// nothing else happened (the slow click is used up either way).
    pub fn slow_rename_due(&mut self) -> Option<usize> {
        let (id, ..) = self.slow_rename.take()?;
        let i = self.view.index_of(&id)?;
        (self.view.selection() == vec![i] && self.view.renaming().is_none() && self.active && !self.tree_focus && !self.field.focused).then_some(i)
    }

    /// The rename that a press outside its box ended is over (the platform took the name, or
    /// kept it): that press goes on as a click there, as on the Reference PCs (survey part 4:
    /// the rename ends and the file clicked is selected; a double-click opens it). Nothing if a
    /// question came up instead (the name is still being renamed). True if it selected.
    /// The press elsewhere that ended a rename (not on the icons), carried on once the rename has
    /// ended without the extension warning (brief 99): what the platform carries out. The press is
    /// made again now; a part held with the mouse (a toolbar button, Go, the title bar's buttons)
    /// acts at once, as a whole click, since its release has nothing left to wait for; a part
    /// that would be dragged (a scroll thumb, the bar between the panes, a column's divider) is
    /// dropped, the drag cannot be taken up halfway. The address field takes the keyboard with
    /// everything selected (a first click), the tree's row, box or blank do what a press does
    /// there, a scroll arrow or page scrolls. Nothing while a rename (or its warning) is still on.
    pub fn carry_on_after_rename(&mut self) -> Vec<Effect<Part>> {
        let Some((part, x, y)) = self.press_after_rename.take() else { return Vec::new() };
        if self.view.renaming().is_some() {
            return Vec::new();
        }
        match self.press(Some(part), x, y) {
            Press::Hold(p) | Press::Act(p) => vec![Effect::Activate(p)],
            // A drag cannot be taken up halfway: dropped. Done: the press did its work.
            _ => Vec::new(),
        }
    }

    pub fn click_after_rename(&mut self) -> bool {
        let Some((x, y, mods)) = self.after_rename.take() else { return false };
        if self.view.renaming().is_some() {
            return false;
        }
        let Some(font) = self.font() else { return false };
        let hit = self.view.press(&font, x, y, IconButton::Left, mods);
        self.view.release(&font, x, y);
        self.first_press = hit.and_then(|i| self.view.items.get(i)).map(|item| item.id.clone());
        true
    }

    pub fn explorer(&self) -> bool {
        self.era != Era::Win95
    }

    /// The menu bar's titles.
    pub fn titles(&self) -> &'static [&'static str] {
        menus::titles(self.era)
    }

    /// The look changed: the window's frame, menus and pictures are the new look's.
    pub fn set_era(&mut self, era: Era) {
        if self.tree_width == pane::default_width(self.era) {
            self.tree_width = pane::default_width(era);
        }
        self.era = era;
        self.view.set_era(era);
        self.list = None;
        self.bar = explorer::State::default();
        if self.tree_shown && era == Era::Win2000 {
            self.bar.checked = vec![Tool::Folders];
        }
        self.update_grayed();
        self.fit();
    }

    /// The window shows another place (98 and 2000, in the same window): its title and picture,
    /// whether it is a folder of files, and the address; the icons go until the new ones are
    /// read, nothing is selected, the list is scrolled to its top.
    pub fn show_place(&mut self, title: &str, icon: Icon, files: bool, address: &str, place_icon: Icon) {
        self.title = title.to_string();
        self.place_name = title.to_string();
        self.disk_free = None;
        self.icon = icon;
        self.files = files;
        let active = self.view.active;
        self.view = Desktop::in_window(self.era, Rect::new(0, 0, 0, 0));
        self.view.active = active;
        self.view.keep_order(!files);
        self.view.dots_origin = self.screen;
        self.scroll = (0, 0);
        self.hidden = 0;
        self.total_size = 0;
        self.spaces.clear();
        self.list = None;
        self.field.show(address);
        self.place_icon = Some(place_icon);
        // The panel starts afresh for the new place (the platform says what kind it is).
        self.web.scroll = 0;
        self.web.files_shown = false;
        self.web.guard = None;
        self.web.space = None;
        self.web.kind = web::Kind::Folder;
        self.web.attributes.clear();
        self.link_hot = None;
        self.fit();
    }

    /// More is known of the place shown now (its late reading: a slow drive, My Computer with a
    /// sleeping disk): its title and pictures and the address change, and nothing else. Only a
    /// navigation starts the view afresh (`show_place`); the items, the selection, the scroll
    /// position, the view's kind and order, an open rename box, the address list and anything
    /// typed into the address bar stay as they are (an outside review of 2026-11-18 found the
    /// files of a window that opened before its reading thrown away when the reading came).
    pub fn update_place(&mut self, title: &str, icon: Icon, address: &str, place_icon: Icon) {
        self.title = title.to_string();
        self.place_name = title.to_string();
        self.icon = icon;
        self.field.place_known(address);
        self.place_icon = Some(place_icon);
        self.fit();
    }

    /// The web view's facts of the place shown, read late (`update_place`): its kind, 32×32
    /// picture and a drive's space. The system folder's warning, whether its files were shown,
    /// the panel's scroll and the selection's attributes stay.
    pub fn set_web_known(&mut self, kind: web::Kind, art: Option<Icon>, space: Option<Space>) {
        self.web.kind = kind;
        self.web.art = art;
        self.web.space = space;
        self.fit();
    }

    /// The web view panel's place (98 and 2000): what kind of place it is, its 32×32 picture, the
    /// system folder's warning (and whether its files were shown before), a drive's space.
    pub fn set_web_place(&mut self, kind: web::Kind, art: Option<Icon>, guard: Option<web::Guard>, files_shown: bool, space: Option<Space>) {
        self.web.kind = kind;
        self.web.art = art;
        self.web.guard = guard;
        self.web.files_shown = files_shown;
        self.web.space = space;
        self.fit();
    }

    /// Switches the web view on or off for this window.
    pub fn set_web(&mut self, on: bool) {
        if self.web.on == on {
            return;
        }
        self.web.on = on;
        self.web.scroll = 0;
        self.relayout_icons();
    }

    /// "Show Files" on the warning page: the files show (the platform remembers it in 2000).
    pub fn show_files(&mut self) {
        self.web.files_shown = true;
        self.relayout_icons();
    }

    /// The icons are put in rows for the contents' new width (the panel came or went).
    fn relayout_icons(&mut self) {
        self.fit();
        let by = self.view.arrangement();
        self.view.set_arrange(by);
        self.fit();
    }

    /// The web view shows here now (on, 98 or 2000, its fonts given).
    fn web_active(&self) -> bool {
        self.explorer() && self.web.on && self.web.fonts.is_some()
    }

    /// The files are hidden behind the system folder's warning page.
    pub fn barricaded(&self) -> bool {
        self.web_active() && self.web.barricaded(self.era)
    }

    /// Where the panel or 98's band goes in the contents area.
    fn web_area(&self) -> WebArea {
        if !self.web_active() {
            return WebArea::None;
        }
        let whole = self.whole_inside();
        let bar = self.barricaded();
        match web::panel_width(self.era, whole.w, bar) {
            Some(p) => WebArea::Panel(Rect::new(whole.x, whole.y, p.min(whole.w), whole.h)),
            None if self.era == Era::Win98 && !bar => WebArea::Banner(Rect::new(whole.x, whole.y, whole.w, web::BANNER_H.min(whole.h))),
            None => WebArea::None,
        }
    }

    /// The selection as the panel describes it: the item with the keyboard's focus first, then the
    /// others in the list's order (`SelectedItems` did so), at most 101 of them; and how many.
    fn web_selection(&self) -> (Vec<web::Info>, usize) {
        let sel = self.view.selection();
        let count = sel.len();
        let focus = self.view.focus.filter(|f| sel.contains(f));
        let order = focus.into_iter().chain(sel.into_iter().filter(|&i| Some(i) != focus)).take(101);
        let infos = order
            .map(|i| {
                let it = &self.view.items[i];
                web::Info::of(it, self.spaces.get(&it.id).copied(), self.web.attributes.get(&it.id).copied())
            })
            .collect();
        (infos, count)
    }

    /// The panel laid out (its rectangle and contents), while it shows.
    fn web_laid(&self) -> Option<(Rect, web::Laid)> {
        let WebArea::Panel(p) = self.web_area() else { return None };
        let fonts = self.web.fonts.as_ref()?;
        let (infos, count) = if self.barricaded() { (Vec::new(), 0) } else { self.web_selection() };
        Some((p, web::lay_out(self.era, fonts, p.w, &self.place_name, &self.web, &infos, count, !self.files)))
    }

    /// The panel's scroll bar (its whole height at the panel's right), while the text is taller.
    fn web_bar(&self) -> Option<(Rect, i32)> {
        let (p, laid) = self.web_laid()?;
        (laid.height > p.h && p.w > web::SCROLL_W).then(|| (Rect::new(p.right() - web::SCROLL_W, p.y, web::SCROLL_W, p.h), laid.height))
    }

    /// What of the web view is at (x, y): a link, the panel's scroll bar, nothing (the panel, 98's
    /// band, the warning page); `None` where the web view is not.
    fn web_hit(&self, x: i32, y: i32) -> Option<Option<Part>> {
        let fonts = self.web.fonts.as_ref().filter(|_| self.web_active())?;
        if let Some((bar, total)) = self.web_bar()
            && bar.contains(x, y)
        {
            let (at, len) = Self::thumb(bar.h, total, bar.h, self.web.scroll);
            return Some(Some(Part::WebScroll(if y < bar.y + BAR {
                Scroll::Up
            } else if y >= bar.bottom() - BAR {
                Scroll::Down
            } else if y < bar.y + at {
                Scroll::PageUp
            } else if y >= bar.y + at + len {
                Scroll::PageDown
            } else {
                return Some(None);
            })));
        }
        if let Some((p, laid)) = self.web_laid()
            && p.contains(x, y)
        {
            let link = web::link_rects(self.era, fonts, &laid).into_iter().find(|(r, _)| r.contains(x - p.x, y - p.y + self.web.scroll));
            return Some(link.map(|(_, l)| Part::WebLink(l)));
        }
        if let WebArea::Banner(b) = self.web_area()
            && b.contains(x, y)
        {
            return Some(None);
        }
        if self.barricaded() {
            let r = self.inside();
            if !r.contains(x, y) {
                return None;
            }
            if self.web_area() == WebArea::None {
                let words = web::lay_out_words(self.era, fonts, r.w, &self.web);
                let laid = web::Laid { title: Vec::new(), title_top: 0, line_y: 0, drawn: words.drawn, height: 0 };
                let link = web::link_rects(self.era, fonts, &laid).into_iter().find(|(lr, _)| lr.contains(x - r.x, y - r.y));
                return Some(link.map(|(_, l)| Part::WebLink(l)));
            }
            return Some(None);
        }
        None
    }

    /// Scrolls the panel by an arrow (a line) or a page.
    pub fn web_scroll(&mut self, s: Scroll) {
        let Some((bar, total)) = self.web_bar() else { return };
        let step = match s {
            Scroll::Up => -LINE,
            Scroll::Down => LINE,
            Scroll::PageUp => -bar.h,
            Scroll::PageDown => bar.h,
            _ => 0,
        };
        self.web.scroll = (self.web.scroll + step).clamp(0, (total - bar.h).max(0));
    }

    /// Where the window can go now (grays Back, Forward and Up).
    pub fn set_nav(&mut self, nav: menus::Nav) {
        self.nav = nav;
        self.update_grayed();
    }

    fn update_grayed(&mut self) {
        let mut g = Vec::new();
        if !self.nav.back {
            g.push(Tool::Back);
        }
        if !self.nav.forward {
            g.push(Tool::Forward);
        }
        if !self.nav.up {
            g.push(Tool::Up);
        }
        self.bar.grayed = g;
    }

    /// Opens the address bar's list with `rows` (the row of the place shown highlighted in 2000),
    /// or closes it (`None`).
    pub fn set_list(&mut self, rows: Option<(Vec<ListRow>, Option<usize>)>) {
        self.list = rows;
    }

    /// The open list's frame, its first row shown and how many show: Bevel95 keeps the list inside
    /// the window (Windows' reached past it), scrolled so that the highlighted row shows.
    fn list_shown(&self) -> Option<(Rect, usize, usize)> {
        let frame = self.list_frame()?;
        let (rows, hot) = self.list.as_ref()?;
        let shown = (((frame.h - 4) / explorer::LIST_ROW_H).max(1) as usize).min(rows.len().max(1));
        let top = hot.map_or(0, |h| (h + 1).saturating_sub(shown)).min(rows.len().saturating_sub(shown));
        Some((frame, top, shown))
    }

    /// The open list's frame (inside the window: as many rows as fit below the address bar).
    fn list_frame(&self) -> Option<Rect> {
        let (rows, _) = self.list.as_ref()?;
        let room = ((self.size.1 - self.chrome.combo.bottom() - 4) / explorer::LIST_ROW_H).max(1) as usize;
        Some(explorer::list_rect(&self.chrome, rows.len().min(room)))
    }

    /// The fonts were measured with (`layout` keeps a copy of the regular one for the mouse).
    pub fn set_font(&mut self, font: &Font) {
        self.font = Some(font.clone());
        self.fit();
    }

    /// The window's size changed (1×), or it was maximized or restored.
    pub fn resize(&mut self, size: (i32, i32), maximized: bool) {
        self.size = (size.0.max(1), size.1.max(1));
        self.maximized = maximized;
        self.fit();
    }

    pub fn size_now(&self) -> (i32, i32) {
        self.size
    }

    /// The folder's items (read again: new ones go to the first free place, gone ones leave theirs
    /// empty). The first time they are put in rows as wide as the list; if they then need a scroll
    /// bar down, as wide as the list beside it.
    pub fn set_items(&mut self, items: Vec<Item>) {
        let first = self.view.items.is_empty() && !items.is_empty();
        self.fit();
        self.view.set_details(if self.files { crate::desktop_icons::Columns::Folder } else { crate::desktop_icons::Columns::MyComputer });
        self.view.spaces = self.spaces.iter().map(|(k, s)| (k.clone(), (s.free, s.capacity))).collect();
        self.view.set_files(items);
        if let Some(font) = self.font.clone() {
            self.view.measure(&font, self.list_fixed);
        }
        // Windows 95 opened a folder with the keyboard's focus on its first icon (nothing selected).
        if first && self.view.focus.is_none() && !self.view.items.is_empty() {
            self.view.focus = Some(0);
        }
        if first && !self.view.items.is_empty() && self.files && self.view.mode() == ViewMode::Large {
            let inside = self.inside();
            if let Some(font) = &self.font
                && self.view.extent(font).1 > inside.h
            {
                self.view.set_area(Rect::new(inside.x, inside.y, inside.w - BAR, inside.h));
                let by = self.view.arrangement();
                self.view.set_arrange(by);
            }
        }
        self.fit();
    }

    fn frame(&self) -> i32 {
        window::sizable_border(self.maximized)
    }

    /// Where the list (and with the tree, both panes) go: from below the menu bar or the rebar to
    /// the status bar.
    fn panes_area(&self) -> Rect {
        let f = self.frame();
        let top = if self.explorer() { self.chrome.list_top() } else { f + window::CAPTION_H + 1 + window::MENU_BAR_H + 1 };
        let bottom = self.size.1 - f - if self.status_bar { status_h(self.era) } else { 0 };
        Rect::new(f, top, self.size.0 - 2 * f, (bottom - top).max(0))
    }

    /// Explorer's panes, while the tree shows.
    pub fn pane_layout(&self) -> Option<pane::Layout> {
        self.tree_shown.then(|| pane::layout(self.era, self.panes_area(), self.tree_width))
    }

    /// The list with its sunken edge.
    fn list(&self) -> Rect {
        match self.pane_layout() {
            Some(l) => l.list,
            None => self.panes_area(),
        }
    }

    /// The tree's rows' area (without its scroll bars), whether it scrolls down and across, and
    /// its whole box.
    fn tree_parts(&self) -> Option<(Rect, bool, bool, Rect)> {
        let l = self.pane_layout()?;
        let font = self.font.as_ref()?;
        let rows = self.tree.visible().len();
        let (v, h, shown) = pane::bars(l.tree, rows, pane::content_width(&self.tree, font));
        Some((shown, v, h, l.tree))
    }

    /// How many whole rows of the tree show.
    pub fn tree_page(&self) -> usize {
        self.tree_parts().map_or(1, |(s, ..)| pane::page(s.h))
    }

    /// The tree's scroll bars' places (down, across).
    fn tree_bar_rects(&self) -> (Option<Rect>, Option<Rect>) {
        let Some((s, v, h, t)) = self.tree_parts() else { return (None, None) };
        (v.then(|| Rect::new(s.right(), t.y, BAR, s.h)), h.then(|| Rect::new(t.x, s.bottom(), s.w, BAR)))
    }

    fn tree_v_thumb(&self) -> Option<Rect> {
        let (Some(bar), _) = self.tree_bar_rects() else { return None };
        let (s, ..) = self.tree_parts()?;
        let total = self.tree.visible().len() as i32;
        let (at, len) = Self::thumb(bar.h, total, pane::page(s.h) as i32, self.tree.top as i32);
        Some(Rect::new(bar.x, bar.y + at, BAR, len))
    }

    fn tree_h_thumb(&self) -> Option<Rect> {
        let (_, Some(bar)) = self.tree_bar_rects() else { return None };
        let (s, ..) = self.tree_parts()?;
        let font = self.font.as_ref()?;
        let (at, len) = Self::thumb(bar.w, pane::content_width(&self.tree, font), s.w, self.tree.scroll_x);
        Some(Rect::new(bar.x + at, bar.y, len, BAR))
    }

    /// Keeps the tree's scrolling in range.
    fn fit_tree(&mut self) {
        let Some((s, ..)) = self.tree_parts() else { return };
        let page = pane::page(s.h);
        self.tree.clamp_top_for(page);
        if let Some(font) = &self.font {
            let max_x = (pane::content_width(&self.tree, font) - s.w).max(0);
            self.tree.scroll_x = self.tree.scroll_x.clamp(0, max_x);
        }
    }

    /// Scrolls the tree by a scroll bar's arrow or page.
    pub fn tree_scroll(&mut self, s: Scroll) {
        let Some((shown, ..)) = self.tree_parts() else { return };
        let page = pane::page(shown.h);
        let t = &mut self.tree;
        match s {
            Scroll::Up => t.top = t.top.saturating_sub(1),
            Scroll::Down => t.top += 1,
            Scroll::PageUp => t.top = t.top.saturating_sub(page),
            Scroll::PageDown => t.top += page,
            Scroll::Left => t.scroll_x -= LINE,
            Scroll::Right => t.scroll_x += LINE,
            Scroll::PageLeft => t.scroll_x -= shown.w,
            Scroll::PageRight => t.scroll_x += shown.w,
        }
        self.fit_tree();
    }

    /// Shows or hides the tree (98's View > Explorer Bar, 2000's Folders, the bar's close
    /// button). The icons are put in rows for the new width, as both Windows did.
    pub fn show_tree(&mut self, shown: bool) {
        if self.tree_shown == shown {
            return;
        }
        self.tree_shown = shown;
        self.tree_focus = shown;
        self.bar.checked = if shown && self.era == Era::Win2000 { vec![Tool::Folders] } else { Vec::new() };
        if !shown {
            self.split_drag = None;
        }
        self.fit();
        if let Some(font) = self.font.clone() {
            let by = self.view.arrangement();
            let _ = font;
            self.view.set_arrange(by);
        }
        self.fit();
    }

    /// What of the tree is at (x, y): the close button, a scroll bar, a row or its box, the empty
    /// part below the rows.
    /// Where a press takes the bar between the panes (and the pointer changes): 95 only the bar
    /// itself (3 wide); 98 and 2000 also the tree pane's two edge columns, 6 in all (measured on
    /// the Reference PCs, `private/probes/measure17`).
    pub fn split_zone(&self, l: &pane::Layout) -> Rect {
        let s = l.split;
        if self.era == Era::Win95 { s } else { Rect::new(s.x - 2, s.y, s.w + 2, s.h) }
    }

    fn tree_hit(&self, l: &pane::Layout, x: i32, y: i32) -> Option<Part> {
        if l.close.is_some_and(|c| c.contains(x, y)) {
            return Some(Part::TreeClose);
        }
        let (v, h) = self.tree_bar_rects();
        if let Some(bar) = v.filter(|b| b.contains(x, y)) {
            let t = self.tree_v_thumb()?;
            return Some(if y < bar.y + BAR {
                Part::TreeScroll(Scroll::Up)
            } else if y >= bar.bottom() - BAR {
                Part::TreeScroll(Scroll::Down)
            } else if t.contains(x, y) {
                Part::TreeVThumb
            } else if y < t.y {
                Part::TreeScroll(Scroll::PageUp)
            } else {
                Part::TreeScroll(Scroll::PageDown)
            });
        }
        if let Some(bar) = h.filter(|b| b.contains(x, y)) {
            let t = self.tree_h_thumb()?;
            return Some(if x < bar.x + BAR {
                Part::TreeScroll(Scroll::Left)
            } else if x >= bar.right() - BAR {
                Part::TreeScroll(Scroll::Right)
            } else if t.contains(x, y) {
                Part::TreeHThumb
            } else if x < t.x {
                Part::TreeScroll(Scroll::PageLeft)
            } else {
                Part::TreeScroll(Scroll::PageRight)
            });
        }
        let (shown, ..) = self.tree_parts()?;
        if !shown.contains(x, y) {
            return None;
        }
        let font = self.font.as_ref()?;
        match pane::row_at(&self.tree, font, shown, x, y) {
            Some((i, crate::tree::Zone::Button)) => Some(Part::TreeBox(i)),
            Some((i, crate::tree::Zone::Item)) => Some(Part::TreeRow(i)),
            // Windows' tree selected a row from anywhere across it.
            Some((i, _)) => Some(Part::TreeRow(i)),
            None => Some(Part::TreeBlank),
        }
    }

    /// A key while the tree has the keyboard (`None`: not the tree's).
    fn tree_key(&mut self, key: Key, mods: Mods) -> Option<Vec<Effect<Part>>> {
        use pane::TreeKey as T;
        let command = |c: Command| Some(vec![Effect::Activate(Part::Command(c))]);
        let k = match key {
            Key::Up => T::Up,
            Key::Down => T::Down,
            Key::Left => T::Left,
            Key::Right => T::Right,
            Key::Home => T::Home,
            Key::End => T::End,
            Key::PageUp => T::PageUp,
            Key::PageDown => T::PageDown,
            Key::Other(VK_ADD) => T::Plus,
            Key::Other(VK_SUBTRACT) => T::Minus,
            Key::Other(VK_MULTIPLY) => T::Star,
            Key::Other(VK_BACK) => T::Backspace,
            Key::Enter => {
                // Places that open as Windows' own (Control Panel, Printers, the Recycle Bin).
                return Some(vec![Effect::Activate(Part::TreeChosen)]);
            }
            Key::Delete => return command(Command::TreeRow(crate::desktop::DesktopCommand::Delete)),
            Key::Other(0x58) if mods.ctrl => return command(Command::TreeRow(crate::desktop::DesktopCommand::Cut)),
            Key::Other(0x43) if mods.ctrl => return command(Command::TreeRow(crate::desktop::DesktopCommand::Copy)),
            Key::Other(0x56) if mods.ctrl => return command(Command::TreeRow(crate::desktop::DesktopCommand::Paste)),
            Key::Other(0x5A) if mods.ctrl => return command(Command::Undo),
            Key::Other(VK_F5) => return command(Command::Shared(crate::desktop::DesktopCommand::Refresh)),
            Key::Other(VK_F10) if mods.shift => return Some(self.tree_keyboard_menu()),
            Key::Other(VK_APPS) => return Some(self.tree_keyboard_menu()),
            _ => return Some(Vec::new()),
        };
        let page = self.tree_page();
        let moved = self.tree.key(k, page);
        self.fit_tree();
        Some(if moved == pane::Moved::Selection { vec![Effect::Activate(Part::TreeMoved)] } else { Vec::new() })
    }

    /// The menu on the selected row from the keyboard: at the row's label.
    fn tree_keyboard_menu(&mut self) -> Vec<Effect<Part>> {
        let Some((shown, ..)) = self.tree_parts() else { return Vec::new() };
        let vis = self.tree.visible();
        let Some(k) = vis.iter().position(|&v| v == self.tree.sel) else { return Vec::new() };
        let y = shown.y + (k as i32 - self.tree.top as i32) * crate::tree::ROW_H + 8;
        let n = &self.tree.nodes[self.tree.sel];
        let x = shown.x + crate::tree::TreeStyle::of(self.era).content_x(n.depth) + 19 - self.tree.scroll_x + 8;
        self.menu_at = (x.clamp(shown.x, shown.right()), y.clamp(shown.y, shown.bottom()));
        self.menu_on_row(self.tree.sel);
        vec![Effect::Activate(Part::TreeMenu)]
    }

    /// The menu on a tree row is opening for row `i`: it is remembered by its id.
    pub fn menu_on_row(&mut self, i: usize) {
        self.menu_row = self.tree.nodes.get(i).map(|n| n.id.clone());
    }

    /// The row the menu on a row was opened for, where it is now; `None` once that row is gone
    /// (read again meanwhile without it), and then the menu's command does nothing at all.
    /// Without a menu: the selected row.
    pub fn menu_node(&self) -> Option<&pane::Node> {
        match &self.menu_row {
            Some(id) => self.tree.find(id).map(|i| &self.tree.nodes[i]),
            None => self.tree.nodes.get(self.tree.sel),
        }
    }

    /// Whether (x, y) is on the rename box of a label being renamed.
    fn rename_hit(&self, x: i32, y: i32) -> bool {
        self.view.renaming().is_some()
            && self.font().and_then(|font| self.view.rename_box(&font)).is_some_and(|(b, _)| b.contains(x, y))
    }

    /// Tab and F6: the keyboard goes from the tree to the icons, (98 and 2000) to the address bar,
    /// and back to the tree (`*-tree-tab`).
    fn next_pane(&mut self) {
        if self.tree_focus {
            self.tree_focus = false;
        } else if self.field.focused {
            self.field.blur();
            self.tree_focus = self.tree_shown;
        } else if self.explorer() {
            self.field.focus();
        } else {
            self.tree_focus = self.tree_shown;
        }
    }

    /// The tree's selection moved (a key, a click): the window shows it in the tree.
    pub fn tree_selection_shown(&mut self) {
        let page = self.tree_page();
        self.tree.show_selection(page);
        // Across too: a row deeper in than the pane is wide brings its picture and label into
        // sight (Windows' tree scrolled sideways to a selected row the same way).
        if let (Some((shown, ..)), Some(font), Some(n)) = (self.tree_parts(), self.font.clone(), self.tree.selected()) {
            let x = crate::tree::TreeStyle::of(self.era).content_x(n.depth);
            let right = x + 19 + 2 + font.measure(&n.name) + 2;
            if x < self.tree.scroll_x || right > self.tree.scroll_x + shown.w {
                self.tree.scroll_x = if right - x <= shown.w { right - shown.w } else { x };
                self.tree.scroll_x = self.tree.scroll_x.min(x);
            }
        }
        self.fit_tree();
    }

    /// The whole inside of the list's sunken edge (the web view's panel and the icons share it).
    fn whole_inside(&self) -> Rect {
        let l = self.list();
        Rect::new(l.x + 2, l.y + 2, (l.w - 4).max(0), (l.h - 4).max(0))
    }

    /// Where the icons are: the list's inside without the web view's panel or band.
    fn inside(&self) -> Rect {
        let whole = self.whole_inside();
        match self.web_area() {
            WebArea::Panel(p) => Rect::new(p.right(), whole.y, (whole.w - p.w).max(0), whole.h),
            WebArea::Banner(b) => Rect::new(whole.x, b.bottom(), whole.w, (whole.h - b.h).max(0)),
            WebArea::None => whole,
        }
    }

    /// Which scroll bars show (down, across): the icons reach past the list's inside.
    fn bars(&self) -> (bool, bool) {
        if self.font.is_none() || self.barricaded() {
            return (false, false);
        }
        let i = self.inside();
        if self.view.mode() == ViewMode::List {
            // A list never scrolls down: its columns hold as many rows as fit (fewer beside a scroll
            // bar across).
            let n = self.view.items.len() as i32;
            let cols = (n + (i.h / ROW_H).max(1) - 1) / (i.h / ROW_H).max(1);
            return (false, cols * self.view.list_width() > i.w);
        }
        let (cw, ch) = self.content();
        let mut v = ch > i.h;
        let mut h = cw > i.w - if v { BAR } else { 0 };
        v = ch > i.h - if h { BAR } else { 0 };
        h = cw > i.w - if v { BAR } else { 0 };
        (v, h)
    }

    /// The part of the inside the icons show in (without the scroll bars).
    fn shown(&self) -> Rect {
        let (v, h) = self.bars();
        let i = self.inside();
        Rect::new(i.x, i.y, (i.w - if v { BAR } else { 0 }).max(0), (i.h - if h { BAR } else { 0 }).max(0))
    }

    /// The furthest the list can scroll, across and down.
    fn max_scroll(&self) -> (i32, i32) {
        let (cw, ch) = self.content();
        let s = self.shown();
        match self.view.mode() {
            // Details scrolls by whole rows (the last row whole at the bottom), List by columns.
            ViewMode::Details => {
                let page = ((s.h - self.header_space()) / ROW_H).max(0);
                ((cw - s.w).max(0), (self.view.items.len() as i32 - page).max(0) * ROW_H)
            }
            ViewMode::List => {
                let w = self.view.list_width().max(1);
                (((cw - s.w).max(0) + w - 1) / w * w, 0)
            }
            _ => ((cw - s.w).max(0), (ch - s.h).max(0)),
        }
    }

    /// The part of the list the items show in: the shown part, in Details below the header.
    fn rows_area(&self) -> Rect {
        let s = self.shown();
        if self.view.mode() == ViewMode::Details {
            return Rect::new(s.x, s.y + self.header_space(), s.w, (s.h - self.header_space()).max(0));
        }
        s
    }

    /// How far Details' rows start below the list's top: the header, and in Windows 2000 two rows
    /// of white under it (`win2000-truecolor-view-v1-details-one`: rows from 19).
    fn header_space(&self) -> i32 {
        HEADER_H + if self.era == Era::Win2000 { 2 } else { 0 }
    }

    /// The view now: Large Icons, Small Icons, List or Details.
    pub fn view_mode(&self) -> ViewMode {
        self.view.mode()
    }

    /// Changes the view (View menu, the Views button, the empty area's View): the items go into the
    /// new view's order from the top left, the list scrolled to its start.
    pub fn set_view_mode(&mut self, mode: ViewMode) {
        if mode == self.view.mode() {
            return;
        }
        self.list_fixed = false;
        self.divider_drag = None;
        if let Some(font) = self.font.clone() {
            self.view.measure(&font, self.list_fixed);
        }
        self.view.set_mode(mode);
        self.scroll = (0, 0);
        self.fit();
        self.fit();
        if let Some(f) = self.view.focus {
            self.show_icon(f);
        }
    }

    /// Arrange Icons > by ...: that order, forward (a header click afterwards starts afresh).
    pub fn arrange_by(&mut self, by: crate::desktop_icons::Arrange) {
        self.view.sort(by, false);
        self.sorted_by_header = false;
        self.fit();
    }

    /// The ids of the items whose rows show now (their pictures are read first).
    pub fn visible_ids(&self) -> Vec<String> {
        self.view.visible_items(self.view.viewport).into_iter().map(|i| self.view.items[i].id.clone()).collect()
    }

    /// How the folder is shown now, to remember (the look's own column widths as none).
    pub fn view_state(&self) -> saved::ViewState {
        let (arrange, reversed) = self.view.sorted();
        let defaults = self.view.details.default_widths(self.era);
        let columns = if self.view.columns.is_empty() || self.view.columns == defaults { Vec::new() } else { self.view.columns.clone() };
        saved::ViewState { mode: self.view.mode(), arrange, reversed, columns }
    }

    /// Shows the folder as remembered (or as it opens: `opening_view`).
    pub fn apply_view_state(&mut self, v: &saved::ViewState) {
        self.set_view_mode(v.mode);
        self.view.set_details(if self.files { crate::desktop_icons::Columns::Folder } else { crate::desktop_icons::Columns::MyComputer });
        self.view.set_columns(&v.columns);
        // My Computer keeps its own order unless another was chosen.
        if self.files || v.arrange != crate::desktop_icons::Arrange::Name || v.reversed {
            self.view.sort(v.arrange, v.reversed);
        }
        self.fit();
    }

    /// A click on Details' header `k`: sorted by that column, the same column again the other way
    /// round (95 to 2000 alike).
    pub fn header_click(&mut self, k: usize) {
        let by = self.view.details.sorts_by(k);
        let (now, reversed) = self.view.sorted();
        let again = self.sorted_by_header && now == by;
        // The selection goes, the focus stays on its item (98: `-view-v1-sort-*`; 2000 alike).
        let focus = self.view.focus.map(|f| self.view.items[f].id.clone());
        self.view.sort(by, again && !reversed);
        self.view.select_only(None);
        self.view.focus = focus.and_then(|id| self.view.index_of(&id));
        self.sorted_by_header = true;
        self.fit();
    }

    /// Puts the icons' area where the list shows it, scrolled, and keeps the scrolling in range.
    fn fit(&mut self) {
        if self.explorer()
            && let Some(font) = &self.font
        {
            self.chrome = explorer::layout(self.era, self.size.0, self.frame(), font, menus::titles(self.era));
        }
        let shown = self.shown();
        let rows = self.rows_area();
        let place = |s: (i32, i32)| Rect::new(rows.x - s.0, rows.y - s.1, rows.w, rows.h);
        // List's columns follow the list's height: laid out first, then the scrolling checked.
        self.view.set_area(place(self.scroll));
        let max = self.max_scroll();
        self.scroll = (self.scroll.0.clamp(0, max.0), self.scroll.1.clamp(0, max.1));
        self.view.set_area(place(self.scroll));
        self.view.viewport = rows;
        let _ = shown;
        // The focus frame's dots keep to the list view's own corner (Explorer's list beside the
        // tree starts at an odd x: `win95-truecolor-tree-large`).
        let inside = self.inside();
        self.view.dots_origin = (self.screen.0 - inside.x % 2, self.screen.1 - inside.y % 2);
        self.fit_tree();
    }

    fn scroll_by(&mut self, dx: i32, dy: i32) {
        self.scroll = (self.scroll.0 + dx, self.scroll.1 + dy);
        self.fit();
    }

    /// Scrolls so that icon `i` shows whole (as the list view does when the keys move to it).
    fn show_icon(&mut self, i: usize) {
        let Some(font) = self.font.clone() else { return };
        let b = self.view.bounds(&font, i);
        let s = self.rows_area();
        let mut d = (0, 0);
        if b.x < s.x {
            d.0 = b.x - s.x;
        } else if b.right() > s.right() {
            d.0 = (b.right() - s.right()).min(b.x - s.x);
        }
        if b.y < s.y {
            d.1 = b.y - s.y;
        } else if b.bottom() > s.bottom() {
            d.1 = (b.bottom() - s.bottom()).min(b.y - s.y);
        }
        // Details moves by whole rows, List by whole columns.
        match self.view.mode() {
            ViewMode::Details if d.1 > 0 => d.1 = (d.1 + ROW_H - 1) / ROW_H * ROW_H,
            ViewMode::List if d.0 > 0 => {
                let w = self.view.list_width().max(1);
                d.0 = (d.0 + w - 1) / w * w;
            }
            _ => {}
        }
        if d != (0, 0) {
            self.scroll_by(d.0, d.1);
        }
    }

    /// The scroll bars' places: (down, across, the corner between them).
    fn bar_rects(&self) -> (Option<Rect>, Option<Rect>) {
        let (v, h) = self.bars();
        let i = self.inside();
        let s = self.shown();
        (v.then(|| Rect::new(s.right(), i.y, BAR, s.h)), h.then(|| Rect::new(i.x, s.bottom(), s.w, BAR)))
    }

    /// A scroll bar's thumb: its length the shown share of the whole, rounded, at least 8; its place
    /// following the scrolling (`len`: the bar's length, `total`/`page`/`pos` in pixels).
    fn thumb(len: i32, total: i32, page: i32, pos: i32) -> (i32, i32) {
        let track = (len - 2 * BAR).max(0);
        let total = total.max(1);
        let size = ((track * page + total / 2) / total).clamp(8.min(track), track);
        let range = (total - page).max(1);
        let at = ((track - size) * pos + range / 2) / range;
        (BAR + at, size)
    }

    fn v_thumb(&self) -> Option<Rect> {
        let (v, _) = self.bar_rects();
        self.font.as_ref()?;
        let bar = v?;
        let (_, ch) = self.content();
        let (at, len) = Self::thumb(bar.h, ch, self.shown().h, self.scroll.1);
        Some(Rect::new(bar.x, bar.y + at, BAR, len))
    }

    fn h_thumb(&self) -> Option<Rect> {
        let (_, h) = self.bar_rects();
        self.font.as_ref()?;
        let bar = h?;
        let (cw, _) = self.content();
        let (at, len) = Self::thumb(bar.w, cw, self.shown().w, self.scroll.0);
        Some(Rect::new(bar.x + at, bar.y, len, BAR))
    }

    fn title_button(&self, part: Part) -> Option<TitleButton> {
        match part {
            Part::Minimize => Some(TitleButton::Minimize),
            Part::Maximize => Some(TitleButton::Maximize),
            Part::Close => Some(TitleButton::Close),
            _ => None,
        }
    }

    fn icon_rect(&self) -> Rect {
        let f = self.frame();
        Rect::new(f + 2, f + 1, 16, 16)
    }

    /// The status bar's words now: what is shown or selected and how much (Windows 95's).
    pub fn status(&self) -> Status {
        let selection = self.view.selection();
        if selection.is_empty() {
            let mut left = status::objects(self.view.items.len(), self.hidden);
            let mut right = if self.files { status::size_in(self.era, self.total_size) } else { String::new() };
            // Explorer added the drive's free space: 95 after the size, 2000 after the count.
            if let (true, Some(free)) = (self.tree_shown, self.disk_free) {
                match self.era {
                    Era::Win95 => right = status::with_free_space(self.era, &right, free),
                    Era::Win2000 => left = status::with_free_space(self.era, &left, free),
                    Era::Win98 => {}
                }
            }
            return Status { left, right };
        }
        let items: Vec<&Item> = selection.iter().map(|&i| &self.view.items[i]).collect();
        let left = match items.as_slice() {
            // Windows 2000 named a file's type and size (`win2000-truecolor-explorer-webview-off`).
            [one] if self.era == Era::Win2000 && !one.folder && !matches!(one.picture, Picture::Art(_)) && !one.type_name.is_empty() => {
                status::type_and_size(&one.type_name, one.size)
            }
            _ => status::selected(selection.len()),
        };
        let right = if let [one] = items.as_slice()
            && let Some(space) = self.spaces.get(&one.id)
        {
            status::free_space(space.free, space.capacity)
        } else if items.iter().any(|i| !i.folder && !matches!(i.picture, Picture::Art(_))) {
            // The files' sizes, folders among them counting nothing (`*-view-v1-details-inactive`:
            // two files and a folder, 40.9KB).
            status::size_in(self.era, items.iter().filter(|i| !i.folder && !matches!(i.picture, Picture::Art(_))).map(|i| i.size).sum())
        } else {
            String::new()
        };
        Status { left, right }
    }

    /// The menus' state now.
    pub fn menu_state(&self, can_paste: bool) -> menus::State {
        let kinds = self.view.selection().into_iter().map(|i| crate::desktop::ItemKind::of(&self.view.items[i])).collect();
        menus::State {
            era: self.era,
            kinds,
            files: self.files,
            can_paste,
            status_bar: self.status_bar,
            auto_arrange: self.view.auto_arrange(),
            nav: self.nav.clone(),
            tree: (self.era != Era::Win95).then_some(self.tree_shown),
            mode: self.view.mode(),
            web: (self.era == Era::Win98).then_some(self.web.on),
            undo: self.undo.as_ref().map(|s| (s.label(self.era), s.help())),
        }
    }

    /// Where the menu bar's title `i` is (window pixels at 1×).
    pub fn title_rect(&self, i: usize) -> Option<Rect> {
        if self.explorer() {
            return self.chrome.titles.get(i).copied();
        }
        self.menu_rects.get(i).copied()
    }

    /// The menu from the keyboard (Shift+F10, the menu key): on the selected icons at the focus
    /// icon's middle, as Windows opened it; with nothing selected, the empty area's menu at the
    /// list's top left.
    fn keyboard_menu(&mut self) -> Vec<Effect<Part>> {
        let selected = !self.view.selection().is_empty();
        match self.view.focus.filter(|_| selected) {
            Some(f) => {
                let r = self.view.icon_rect(f);
                self.menu_at = (r.x + r.w / 2, r.y + r.h / 2);
                vec![Effect::Activate(Part::ItemMenu)]
            }
            None => {
                let s = self.shown();
                self.menu_at = (s.x, s.y);
                vec![Effect::Activate(Part::BackgroundMenu)]
            }
        }
    }

    /// Windows 98's and 2000's status bar: three parts (what is shown or selected, how much, and
    /// the zone: My Computer with its picture), each 18 rows, 2 rows below the list.
    /// The web view: the panel (with its scroll bar), 98's band, the warning page's right side.
    fn paint_web(&self, fb: &mut Framebuffer, theme: &Theme) {
        let Some(fonts) = self.web.fonts.as_ref().filter(|_| self.web_active()) else { return };
        let area = self.web_area();
        if let Some((p, laid)) = self.web_laid() {
            let mut panel = Framebuffer::new(p.w, p.h, 0);
            web::paint(&mut panel, theme, fonts, &laid, self.web.art.as_ref(), self.web.scroll, self.link_hot);
            fb.blit(&panel, p.x, p.y);
            if let Some((bar, total)) = self.web_bar() {
                track(fb, theme, bar);
                let held = |s: Scroll| self.pressed == Some(Part::WebScroll(s));
                arrow(fb, theme, Rect::new(bar.x, bar.y, BAR, BAR), Scroll::Up, held(Scroll::Up));
                arrow(fb, theme, Rect::new(bar.x, bar.bottom() - BAR, BAR, BAR), Scroll::Down, held(Scroll::Down));
                let (at, len) = Self::thumb(bar.h, total, bar.h, self.web.scroll);
                thumb(fb, theme, Rect::new(bar.x, bar.y + at, BAR, len));
            }
        }
        if let WebArea::Banner(b) = area {
            web::paint_banner(fb, fonts, b, &self.place_name);
        }
        if self.barricaded() {
            let r = self.inside();
            let words = (area == WebArea::None).then(|| web::lay_out_words(self.era, fonts, r.w, &self.web));
            web::paint_brand(fb, self.era, fonts, r, words.as_ref());
        }
    }

    fn paint_status_explorer(&self, fb: &mut Framebuffer, theme: &Theme, font: &Font) {
        let c = &theme.colors;
        let f = self.frame();
        let (w, h) = self.size;
        let top = h - f - explorer::STATUS_H;
        fb.fill(Rect::new(f, top, w - 2 * f, explorer::STATUS_H), c.face);
        let grip_x = if self.maximized { w - f } else { w - f - 13 };
        if let Some(help) = &self.help {
            let area = Rect::new(f + 3, top + 4, (grip_x - 3 - (f + 3)).max(0), 13);
            let mut text = Framebuffer::new(area.w.max(0), 13, c.face);
            font.draw(&mut text, 0, 0, help, c.text);
            fb.blit(&text, area.x, area.y);
        } else {
            let mut s = self.status();
            // A link of the web view under the pointer: its address or words in the first part.
            if let Some(words) = self.link_hot.and_then(|l| l.status(self.era)) {
                s.left = words.to_string();
            }
            let parts = explorer::status_parts(self.era, w, f, top);
            let zone = "My Computer";
            // Where a part's words begin inside its edge.
            let (tx, ty) = if self.era == Era::Win2000 { (1, 1) } else { (2, 1) };
            for (k, pane) in parts.iter().enumerate() {
                if pane.w <= 0 {
                    continue;
                }
                fb.frame(*pane, c.shadow, c.highlight);
                let right = (pane.right() - 1).min(grip_x - 2);
                let inner = Rect::new(pane.x + 1, pane.y + 1, (right - pane.x - 1).max(0), 16);
                let mut part = Framebuffer::new(inner.w, inner.h, c.face);
                match k {
                    0 => {
                        font.draw(&mut part, tx, ty, &s.left, c.text);
                    }
                    1 => {
                        font.draw(&mut part, tx, ty, &s.right, c.text);
                    }
                    _ => {
                        let icon = explorer_art::picture(self.era, "z_mycomputer").unwrap_or_else(|| art::small(self.era, art::Small::MyComputer));
                        let (ix, zx) = if self.era == Era::Win2000 { (1, 21) } else { (2, 22) };
                        crate::colors16::draw_picture(&mut part, theme, &icon, ix, 0);
                        font.draw(&mut part, zx, 1, zone, c.text);
                    }
                }
                fb.blit(&part, inner.x, inner.y);
            }
        }
        if !self.maximized {
            paint_grip(fb, theme, w, h, f);
        }
    }

    fn paint_status(&self, fb: &mut Framebuffer, theme: &Theme, font: &Font) {
        if self.explorer() {
            return self.paint_status_explorer(fb, theme, font);
        }
        let c = &theme.colors;
        let f = self.frame();
        let (w, h) = self.size;
        let top = h - f - STATUS_H;
        let pane_y = top + 2;
        let text_y = pane_y + 2;
        // Text stops short of the size grip (3 pixels before it; a part's 2).
        let grip_x = if self.maximized { w - f } else { w - f - 13 };
        if let Some(help) = &self.help {
            let area = Rect::new(f + 3, text_y, (grip_x - 3 - (f + 3)).max(0), 13);
            let mut text = Framebuffer::new(area.w.max(0), 13, c.face);
            font.draw(&mut text, 0, 0, help, c.text);
            fb.blit(&text, area.x, area.y);
        } else {
            let s = self.status();
            let left = Rect::new(f, pane_y, STATUS_LEFT_W, 17);
            let right = Rect::new(f + STATUS_LEFT_W + STATUS_GAP, pane_y, w - 2 * f - STATUS_LEFT_W - STATUS_GAP, 17);
            for (pane, words) in [(left, &s.left), (right, &s.right)] {
                if pane.w <= 0 {
                    continue;
                }
                fb.frame(pane, c.shadow, c.highlight);
                let right = (pane.right() - 1).min(grip_x - 2);
                let inner = Rect::new(pane.x + 1, pane.y + 1, (right - pane.x - 1).max(0), 15);
                let mut text = Framebuffer::new(inner.w, inner.h, c.face);
                font.draw(&mut text, 2, text_y - inner.y, words, c.text);
                fb.blit(&text, inner.x, inner.y);
            }
        }
        if !self.maximized {
            paint_grip(fb, theme, w, h, f);
        }
    }

    fn paint_bars(&self, fb: &mut Framebuffer, theme: &Theme) {
        let c = &theme.colors;
        let (v, h) = self.bar_rects();
        let held = |p: Scroll| self.pressed == Some(Part::Scroll(p));
        if let Some(bar) = v {
            track(fb, theme, bar);
            arrow(fb, theme, Rect::new(bar.x, bar.y, BAR, BAR), Scroll::Up, held(Scroll::Up));
            arrow(fb, theme, Rect::new(bar.x, bar.bottom() - BAR, BAR, BAR), Scroll::Down, held(Scroll::Down));
            if let Some(t) = self.v_thumb() {
                thumb(fb, theme, t);
            }
        }
        if let Some(bar) = h {
            track(fb, theme, bar);
            arrow(fb, theme, Rect::new(bar.x, bar.y, BAR, BAR), Scroll::Left, held(Scroll::Left));
            arrow(fb, theme, Rect::new(bar.right() - BAR, bar.y, BAR, BAR), Scroll::Right, held(Scroll::Right));
            if let Some(t) = self.h_thumb() {
                thumb(fb, theme, t);
            }
        }
        if let (Some(v), Some(h)) = (v, h) {
            fb.fill(Rect::new(v.x, h.y, BAR, BAR), c.face);
        }
    }

    /// What a press on a scroll bar at (x, y) is.
    fn scroll_hit(&self, x: i32, y: i32) -> Option<Part> {
        let (v, h) = self.bar_rects();
        if let Some(bar) = v.filter(|b| b.contains(x, y)) {
            let t = self.v_thumb()?;
            return Some(if y < bar.y + BAR {
                Part::Scroll(Scroll::Up)
            } else if y >= bar.bottom() - BAR {
                Part::Scroll(Scroll::Down)
            } else if t.contains(x, y) {
                Part::VThumb
            } else if y < t.y {
                Part::Scroll(Scroll::PageUp)
            } else {
                Part::Scroll(Scroll::PageDown)
            });
        }
        if let Some(bar) = h.filter(|b| b.contains(x, y)) {
            let t = self.h_thumb()?;
            return Some(if x < bar.x + BAR {
                Part::Scroll(Scroll::Left)
            } else if x >= bar.right() - BAR {
                Part::Scroll(Scroll::Right)
            } else if t.contains(x, y) {
                Part::HThumb
            } else if x < t.x {
                Part::Scroll(Scroll::PageLeft)
            } else {
                Part::Scroll(Scroll::PageRight)
            });
        }
        None
    }

    /// Carries out a scroll bar's arrow or page.
    pub fn scroll(&mut self, s: Scroll) {
        let page = self.shown();
        let (line_x, line_y, page_x, page_y) = match self.view.mode() {
            ViewMode::Details => {
                let rows = ((page.h - self.header_space()) / ROW_H).max(1);
                (LINE, ROW_H, page.w, rows * ROW_H)
            }
            ViewMode::List => {
                let w = self.view.list_width().max(1);
                (w, LINE, (page.w / w).max(1) * w, page.h)
            }
            _ => (LINE, LINE, page.w, page.h),
        };
        match s {
            Scroll::Up => self.scroll_by(0, -line_y),
            Scroll::Down => self.scroll_by(0, line_y),
            Scroll::PageUp => self.scroll_by(0, -page_y),
            Scroll::PageDown => self.scroll_by(0, page_y),
            Scroll::Left => self.scroll_by(-line_x, 0),
            Scroll::Right => self.scroll_by(line_x, 0),
            Scroll::PageLeft => self.scroll_by(-page_x, 0),
            Scroll::PageRight => self.scroll_by(page_x, 0),
        }
    }


    /// How far the icons reach (the scroll bars' whole), with the list view's margin of 2 pixels
    /// past the furthest label (the Reference PC's thumbs: `*-pf-narrow`, `*-t26`).
    fn content(&self) -> (i32, i32) {
        let Some(font) = &self.font else { return (0, 0) };
        let (w, h) = self.view.extent(font);
        match self.view.mode() {
            ViewMode::List => (w, h),
            ViewMode::Details => (w, h + self.header_space()),
            _ => (w + MARGIN, h + MARGIN),
        }
    }

    fn font(&self) -> Option<Font> {
        self.font.clone()
    }

    /// A key moved the selection: the focus icon is scrolled into sight.
    fn after_key(&mut self) {
        if let Some(f) = self.view.focus {
            self.show_icon(f);
        }
    }

    /// A key while the address bar's list is open: the arrows move the highlight, Enter chooses,
    /// Esc closes (`None`: no list open).
    fn list_key(&mut self, key: Key) -> Option<Vec<Effect<Part>>> {
        let (rows, hot) = self.list.as_mut()?;
        let n = rows.len();
        match key {
            Key::Down => *hot = Some(hot.map_or(0, |h| (h + 1).min(n.saturating_sub(1)))),
            Key::Up => *hot = Some(hot.map_or(0, |h| h.saturating_sub(1))),
            Key::Home => *hot = Some(0),
            Key::End => *hot = Some(n.saturating_sub(1)),
            Key::Enter => return Some(hot.map(|h| vec![Effect::Activate(Part::ListRow(h))]).unwrap_or_default()),
            Key::Escape | Key::F4 => {
                self.list = None;
            }
            _ => {}
        }
        Some(Vec::new())
    }

    /// The pointer is on `part` now (98 and 2000: flat buttons rise under it). True if that
    /// changed what shows.
    fn set_hot(&mut self, part: Option<Part>) -> bool {
        let before = (self.bar.hot_title, self.bar.hot_tool, self.bar.hot_go, self.list.as_ref().and_then(|l| l.1));
        self.bar.hot_title = None;
        self.bar.hot_tool = None;
        self.bar.hot_go = false;
        match part {
            Some(Part::Menu(i)) => self.bar.hot_title = Some(i),
            Some(Part::Tool(t)) if self.enabled(Part::Tool(t)) => self.bar.hot_tool = Some((t, false)),
            Some(Part::ToolArrow(t)) if self.enabled(Part::Tool(t)) => self.bar.hot_tool = Some((t, true)),
            Some(Part::Go) => self.bar.hot_go = true,
            Some(Part::ListRow(i)) => {
                if let Some(l) = self.list.as_mut() {
                    l.1 = Some(i);
                }
            }
            _ => {}
        }
        before != (self.bar.hot_title, self.bar.hot_tool, self.bar.hot_go, self.list.as_ref().and_then(|l| l.1))
    }

    /// Starts renaming icon `i` (F2, Rename). False if it cannot be renamed.
    pub fn begin_rename(&mut self, i: usize) -> bool {
        let ok = self.view.begin_rename(i);
        if ok {
            // The rename box takes the keyboard from the address field (brief 97).
            self.field.blur();
            self.list = None;
            self.show_icon(i);
        }
        ok
    }
}

/// The size grip: three white-and-gray diagonals in the corner, over the parts' edges.
fn paint_grip(fb: &mut Framebuffer, theme: &Theme, w: i32, h: i32, f: i32) {
    let c = &theme.colors;
    let (gx, gy) = (w - f - 13, h - f - 13);
    fb.fill(Rect::new(gx, gy, 13, 13), c.face);
    for k in 0..3 {
        for r in 0..12 {
            let x = 11 - r + 4 * k;
            if x < 12 {
                fb.set(gx + x, gy + r, c.highlight);
            }
            for g in 1..=2 {
                if x + g < 12 {
                    fb.set(gx + x + g, gy + r, c.shadow);
                }
            }
        }
    }
}

/// The scroll bar's track: the checkerboard of white and face color.
fn track(fb: &mut Framebuffer, theme: &Theme, r: Rect) {
    let c = &theme.colors;
    for y in r.y..r.bottom() {
        for x in r.x..r.right() {
            fb.set(x, y, if (x + y) % 2 == 1 { c.highlight } else { c.face });
        }
    }
}

/// A scroll bar's thumb: a raised box.
fn thumb(fb: &mut Framebuffer, theme: &Theme, r: Rect) {
    let c = &theme.colors;
    fb.fill(r, c.face);
    fb.frame(r, c.light, c.dark_shadow);
    fb.frame(Rect::new(r.x + 1, r.y + 1, r.w - 2, r.h - 2), c.highlight, c.shadow);
}

/// A scroll bar's arrow button (16×16, its black triangle 7 by 4, pointing the button's way;
/// pushed in: a flat shadow edge, the triangle 1 pixel down and right).
fn arrow(fb: &mut Framebuffer, theme: &Theme, r: Rect, dir: Scroll, pressed: bool) {
    let c = &theme.colors;
    fb.fill(r, c.face);
    if pressed {
        fb.frame(r, c.shadow, c.shadow);
    } else {
        fb.frame(r, c.light, c.dark_shadow);
        fb.frame(Rect::new(r.x + 1, r.y + 1, r.w - 2, r.h - 2), c.highlight, c.shadow);
    }
    let s = i32::from(pressed);
    for row in 0..4 {
        let w = 2 * row + 1;
        match dir {
            Scroll::Up => fb.hline(r.x + 7 - w / 2 + s, r.y + 6 + row + s, w, c.text),
            Scroll::Down => fb.hline(r.x + 7 - (7 - 2 * row) / 2 + s, r.y + 6 + row + s, 7 - 2 * row, c.text),
            Scroll::Left => fb.vline(r.x + 5 + row + s, r.y + 7 - w / 2 + s, w, c.text),
            _ => fb.vline(r.x + 6 + row + s, r.y + 7 - (7 - 2 * row) / 2 + s, 7 - 2 * row, c.text),
        }
    }
}

impl Form for FolderWindow {
    type Part = Part;

    fn title(&self) -> &str {
        &self.title
    }

    fn size(&self) -> (i32, i32) {
        self.size
    }

    fn layout(&mut self, fonts: &Fonts) {
        if self.font.is_none() {
            self.font = Some(fonts.regular.clone());
        }
        let delta = self.frame() - window::FRAME;
        self.menu_rects = window::menu_bar_layout(fonts, self.titles()).into_iter().map(|r| Rect::new(r.x + delta, r.y + delta, r.w, r.h)).collect();
        self.fit();
    }

    fn paint(&self, fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts) {
        let c = &theme.colors;
        let pressed = self.pressed.and_then(|p| self.title_button(p));
        window::paint_sizable_frame(fb, theme, fonts, self.era, &self.title, &self.icon, self.active, pressed, self.maximized);
        if self.explorer() {
            let mut bar = self.bar.clone();
            bar.chosen_title = self.chosen;
            if let Some(Part::Tool(t)) = self.pressed {
                bar.pressed_tool = Some(t);
            }
            bar.pressed_go = self.pressed == Some(Part::Go);
            explorer::paint(fb, theme, fonts, self.era, &self.chrome, self.titles(), &bar);
            let f = &self.field;
            let look = explorer::AddressLook {
                icon: self.place_icon.as_ref(),
                text: &f.text,
                selection: (f.focused && self.active).then(|| f.selection()),
                caret: (f.focused && self.active && f.caret_on).then_some(f.caret),
                list_open: self.list.is_some(),
                scroll: f.fit(fonts.regular, self.chrome.combo_button.x - self.chrome.address_text_at().0),
            };
            explorer::paint_address(fb, theme, fonts, &self.chrome, &look);
        } else {
            let looks: Vec<TitleLook> =
                (0..self.titles().len()).map(|i| if self.chosen == Some(i) { TitleLook::Chosen } else { TitleLook::Plain }).collect();
            window::paint_menu_bar(fb, theme, fonts, self.era, self.titles(), &self.menu_rects, &looks, true, self.active);
        }
        if let Some(l) = self.pane_layout() {
            let contents = (self.era == Era::Win95).then_some(self.place_name.as_str());
            let close = if self.pressed == Some(Part::TreeClose) { 2 } else { self.close_look };
            pane::paint_chrome(fb, c, fonts.regular, self.era, &l, contents, close);
            if let Some((shown, ..)) = self.tree_parts() {
                pane::paint_rows(fb, theme, fonts.regular, self.era, &self.tree, shown, self.tree_focus && self.active, self.era != Era::Win2000 || self.bar.show_keys, self.screen);
            }
            let (v, h) = self.tree_bar_rects();
            let held = |p: Scroll| self.pressed == Some(Part::TreeScroll(p));
            if let Some(bar) = v {
                track(fb, theme, bar);
                arrow(fb, theme, Rect::new(bar.x, bar.y, BAR, BAR), Scroll::Up, held(Scroll::Up));
                arrow(fb, theme, Rect::new(bar.x, bar.bottom() - BAR, BAR, BAR), Scroll::Down, held(Scroll::Down));
                if let Some(t) = self.tree_v_thumb() {
                    thumb(fb, theme, t);
                }
            }
            if let Some(bar) = h {
                track(fb, theme, bar);
                arrow(fb, theme, Rect::new(bar.x, bar.y, BAR, BAR), Scroll::Left, held(Scroll::Left));
                arrow(fb, theme, Rect::new(bar.right() - BAR, bar.y, BAR, BAR), Scroll::Right, held(Scroll::Right));
                if let Some(t) = self.tree_h_thumb() {
                    thumb(fb, theme, t);
                }
            }
            if let (Some(v), Some(h)) = (v, h) {
                fb.fill(Rect::new(v.x, h.y, BAR, BAR), c.face);
            }
        }
        let list = self.list();
        if list.w > 4 && list.h > 4 {
            fb.frame(list, c.shadow, c.highlight);
            fb.frame(Rect::new(list.x + 1, list.y + 1, list.w - 2, list.h - 2), c.dark_shadow, c.light);
            let shown = self.shown();
            let rows = self.rows_area();
            self.paint_web(fb, theme);
            if self.barricaded() {
                // The warning page: no icons.
            } else if rows.w > 0 && rows.h > 0 {
                // While the tree has the keyboard the icons show no focus frame (`*-tree-tab`).
                let active = self.view.active && !(self.tree_shown && self.tree_focus);
                self.view.paint_as(fb, theme, fonts, rows, active);
            }
            if self.view.mode() == ViewMode::Details && shown.w > 0 && shown.h > 0 && !self.barricaded() {
                let pressed = match self.pressed {
                    Some(Part::Header(k)) => Some(k),
                    _ => None,
                };
                let (by, reversed) = self.view.sorted();
                let sorted = Some((self.view.details.column_of(by), reversed));
                let header = Rect::new(shown.x, shown.y, shown.w, HEADER_H.min(shown.h));
                let mut strip = Framebuffer::new(header.w, header.h, 0);
                // 2000's two white rows between the header and the first row.
                fb.fill(Rect::new(shown.x, shown.y + HEADER_H, shown.w, (self.header_space() - HEADER_H).min((shown.h - HEADER_H).max(0))), crate::desktop_icons::WINDOW);
                crate::desktop_icons::paint_header(&mut strip, theme, fonts.regular, self.era, self.view.details, &self.view.columns, Rect::new(0, 0, header.w, header.h), -self.scroll.0, pressed, sorted);
                fb.blit(&strip, header.x, header.y);
                // A divider being dragged: an inverted line from the header's top down the list.
                if let Some((k, from, to)) = self.divider_drag {
                    let edge: i32 = self.view.columns[..=k.min(self.view.columns.len() - 1)].iter().sum();
                    let lx = rows.x - self.scroll.0 + edge + to - from;
                    if lx >= rows.x && lx < rows.right() {
                        // Through the header too (`*-view-v1-hdr-divider-drag`).
                        for y in shown.y..rows.bottom() {
                            let p = fb.get(lx, y);
                            fb.set(lx, y, (p & 0xFF00_0000) | (!p & 0x00FF_FFFF));
                        }
                    }
                }
            }
            self.paint_bars(fb, theme);
        }
        if self.status_bar {
            self.paint_status(fb, theme, fonts.regular);
        }
        if let (Some(x), Some(l)) = (self.split_drag, self.pane_layout()) {
            pane::paint_drag_line(fb, x, l.split.y, l.split.h);
        }
        if let (Some((frame, top, shown)), Some((rows, hot))) = (self.list_shown(), &self.list) {
            let end = (top + shown).min(rows.len());
            explorer::paint_list(fb, theme, fonts, frame, &rows[top..end], hot.and_then(|h| h.checked_sub(top)));
        }
    }

    fn set_screen(&mut self, x: i32, y: i32) {
        self.screen = (x, y);
        self.fit();
    }

    fn hit(&self, x: i32, y: i32) -> Option<Part> {
        let w = self.size.0;
        for (part, button) in [(Part::Minimize, TitleButton::Minimize), (Part::Maximize, TitleButton::Maximize), (Part::Close, TitleButton::Close)] {
            if window::sizable_button_rect(w, self.maximized, button).contains(x, y) {
                return Some(part);
            }
        }
        if self.icon_rect().contains(x, y) {
            return Some(Part::Icon);
        }
        if self.explorer() {
            if let (Some((frame, top, shown)), Some((rows, _))) = (self.list_shown(), &self.list)
                && frame.contains(x, y)
            {
                let n = shown.min(rows.len() - top);
                return explorer::list_row_at(frame, n, x, y).map(|i| Part::ListRow(top + i)).or(Some(Part::AddressButton));
            }
            let ch = &self.chrome;
            if let Some(i) = ch.titles.iter().position(|r| r.contains(x, y) && r.right() <= ch.throbber.x - 2) {
                return Some(Part::Menu(i));
            }
            for b in &ch.buttons {
                if b.main.contains(x, y) && b.main.right() <= ch.tool_band.right() {
                    return Some(Part::Tool(b.tool));
                }
                if let Some(a) = b.arrow.filter(|a| a.contains(x, y) && a.right() <= ch.tool_band.right()) {
                    let _ = a;
                    return Some(Part::ToolArrow(b.tool));
                }
            }
            if ch.combo_button.contains(x, y) {
                return Some(Part::AddressButton);
            }
            if ch.combo.contains(x, y) {
                return Some(Part::Address);
            }
            if ch.go.is_some_and(|g| g.contains(x, y)) {
                return Some(Part::Go);
            }
            if ch.rebar.contains(x, y) {
                return None;
            }
        } else if let Some(i) = self.menu_rects.iter().position(|r| r.contains(x, y)) {
            return Some(Part::Menu(i));
        }
        if let Some(l) = self.pane_layout() {
            if self.split_zone(&l).contains(x, y) {
                return Some(Part::Split);
            }
            if let Some(p) = self.tree_hit(&l, x, y) {
                return Some(p);
            }
            if l.header.contains(x, y) || l.frame.is_some_and(|f| f.contains(x, y)) || l.contents.is_some_and(|c| c.contains(x, y)) || l.tree_box.contains(x, y) {
                return None;
            }
        }
        if let Some(p) = self.web_hit(x, y) {
            return p;
        }
        if let Some(p) = self.scroll_hit(x, y) {
            return Some(p);
        }
        let shown = self.shown();
        if self.view.mode() == ViewMode::Details && shown.contains(x, y) && y < shown.y + HEADER_H {
            return match crate::desktop_icons::header_hit(&self.view.columns, shown.x - self.scroll.0, x) {
                Some(HeaderPart::Button(k)) => Some(Part::Header(k)),
                Some(HeaderPart::Divider(k)) => Some(Part::Divider(k)),
                _ => None,
            };
        }
        shown.contains(x, y).then_some(Part::List)
    }

    fn pointer(&self, x: i32, y: i32) -> Pointer {
        if self.view.renaming().is_some()
            && let Some(font) = self.font()
            && self.view.rename_box(&font).is_some_and(|(b, _)| b.contains(x, y))
        {
            return Pointer::Text;
        }
        match self.hit(x, y) {
            Some(Part::Divider(_)) => Pointer::HeaderDivider,
            Some(Part::Split) if self.era == Era::Win95 => Pointer::Split95,
            Some(Part::Split) => Pointer::SizeWE,
            Some(Part::Address) => Pointer::Text,
            Some(Part::WebLink(_)) => Pointer::Hand,
            _ => Pointer::Arrow,
        }
    }

    fn rect(&self, part: Part) -> Option<Rect> {
        let w = self.size.0;
        match part {
            Part::Minimize => Some(window::sizable_button_rect(w, self.maximized, TitleButton::Minimize)),
            Part::Maximize => Some(window::sizable_button_rect(w, self.maximized, TitleButton::Maximize)),
            Part::Close => Some(window::sizable_button_rect(w, self.maximized, TitleButton::Close)),
            Part::Icon => Some(self.icon_rect()),
            Part::Menu(i) => self.title_rect(i),
            Part::List => Some(self.shown()),
            Part::Scroll(_) | Part::VThumb | Part::HThumb => {
                let (v, h) = self.bar_rects();
                match part {
                    Part::Scroll(Scroll::Up | Scroll::Down | Scroll::PageUp | Scroll::PageDown) | Part::VThumb => v,
                    _ => h,
                }
            }
            Part::Tool(t) => self.chrome.buttons.iter().find(|b| b.tool == t).map(|b| b.main),
            Part::Go => self.chrome.go,
            Part::TreeClose => self.pane_layout().and_then(|l| l.close),
            Part::Split => self.pane_layout().map(|l| l.split),
            // Details' header buttons: held by the mouse, they act only when let go on them (the
            // kit asks for the rectangle; without it a real click never sorted).
            Part::Header(k) if self.view.mode() == ViewMode::Details && k < self.view.columns.len() => {
                let shown = self.shown();
                let x = shown.x - self.scroll.0 + self.view.columns[..k].iter().sum::<i32>();
                Some(Rect::new(x, shown.y, self.view.columns[k], HEADER_H))
            }
            _ => None,
        }
    }

    fn enabled(&self, part: Part) -> bool {
        match part {
            Part::Tool(t) | Part::ToolArrow(t) => !self.bar.grayed.contains(&t),
            _ => true,
        }
    }

    fn focus(&self) -> Part {
        Part::List
    }

    fn set_focus(&mut self, _part: Part) {}

    fn tab_stops(&self) -> Vec<Part> {
        Vec::new()
    }

    fn set_pressed(&mut self, part: Option<Part>) {
        self.pressed = part;
    }

    fn show_keys(&mut self) {
        // Windows 2000 underlined access keys from the first key on.
        self.bar.show_keys = true;
    }

    fn enter(&self) -> Part {
        Part::Open
    }

    fn escape(&self) -> Part {
        Part::List
    }

    fn key(&mut self, key: Key, mods: Mods) -> Option<Vec<Effect<Part>>> {
        if self.view.renaming().is_some() {
            match key {
                Key::Enter => return Some(vec![Effect::Activate(Part::RenameEnd(true))]),
                Key::Escape => {
                    self.view.rename_key(EditKey::Escape, false);
                    return Some(Vec::new());
                }
                _ => {}
            }
            // Windows' edit box: caret keys, words, Cut, Copy, Paste, Undo, Ctrl+A, Shift+F10
            // (brief 95); every other key is swallowed while renaming, as before.
            return Some(self.view.rename_edit_key(key, mods).unwrap_or_default());
        }
        let command = |c: Command| Some(vec![Effect::Activate(Part::Command(c))]);
        if let Some(effects) = self.list_key(key) {
            return Some(effects);
        }
        if self.field.focused {
            match key {
                Key::Enter => return Some(vec![Effect::Activate(Part::Go)]),
                Key::Escape => {
                    // Windows gave the place's address back and kept the field.
                    let text = self.field.place_text.clone();
                    self.field.show(&text);
                    self.field.focus();
                    return Some(Vec::new());
                }
                Key::Tab | Key::Other(VK_F6) => {
                    self.next_pane();
                    return Some(Vec::new());
                }
                Key::F4 => return Some(vec![Effect::Activate(Part::AddressButton)]),
                Key::Down | Key::Up => return Some(vec![Effect::Activate(Part::AddressButton)]),
                // Ctrl+L: the field selected whole (the owner's addition, brief 95).
                Key::Other(VK_L) if mods.ctrl && !mods.shift => {
                    self.field.focus();
                    return Some(Vec::new());
                }
                _ => {}
            }
            // Windows' edit box: the caret keys, words, Cut, Copy, Paste, Undo, Ctrl+A,
            // Shift+F10 (brief 95).
            return Some(self.field.edit_key(key, mods).unwrap_or_default());
        }
        // Ctrl+L selects the address field whole from anywhere in the window (98 and 2000 looks;
        // the old Windows had nothing on it: the owner's addition, brief 95).
        if self.explorer() && key == Key::Other(VK_L) && mods.ctrl && !mods.shift && self.view.renaming().is_none() {
            self.tree_focus = false;
            self.list = None;
            self.field.focus();
            return Some(Vec::new());
        }
        if (self.explorer() || self.tree_shown) && matches!(key, Key::Tab | Key::Other(VK_F6)) {
            self.next_pane();
            return Some(Vec::new());
        }
        if self.tree_shown && self.tree_focus {
            return self.tree_key(key, mods);
        }
        if self.explorer() && key == Key::F4 {
            self.field.focus();
            return Some(vec![Effect::Activate(Part::AddressButton)]);
        }
        use crate::desktop::DesktopCommand as D;
        let icon_key = match key {
            Key::Up => Some(IconKey::Up),
            Key::Down => Some(IconKey::Down),
            Key::Left => Some(IconKey::Left),
            Key::Right => Some(IconKey::Right),
            Key::Home => Some(IconKey::Home),
            Key::End => Some(IconKey::End),
            Key::Escape => Some(IconKey::Escape),
            Key::Other(0x41) if mods.ctrl => Some(IconKey::SelectAll),
            _ => None,
        };
        if let Some(k) = icon_key {
            self.view.key(k);
            self.after_key();
            return Some(Vec::new());
        }
        match key {
            Key::Enter if !self.view.selection().is_empty() => Some(vec![Effect::Activate(Part::Open)]),
            Key::Delete => command(Command::Shared(D::Delete)),
            Key::Other(VK_F2) if !mods.ctrl && !mods.shift => command(Command::Shared(D::Rename)),
            Key::Other(VK_F5) => command(Command::Shared(D::Refresh)),
            Key::Other(VK_BACK) => command(Command::Up),
            Key::Other(0x58) if mods.ctrl => command(Command::Shared(D::Cut)),
            Key::Other(0x43) if mods.ctrl => command(Command::Shared(D::Copy)),
            Key::Other(0x56) if mods.ctrl => command(Command::Shared(D::Paste)),
            Key::Other(0x5A) if mods.ctrl => command(Command::Undo),
            Key::Other(VK_F10) if mods.shift => Some(self.keyboard_menu()),
            Key::Other(VK_APPS) => Some(self.keyboard_menu()),
            Key::PageUp | Key::PageDown if self.view.mode().small() => {
                self.view.key(if key == Key::PageUp { IconKey::PageUp } else { IconKey::PageDown });
                self.after_key();
                Some(Vec::new())
            }
            Key::PageUp => {
                self.scroll(Scroll::PageUp);
                Some(Vec::new())
            }
            Key::PageDown => {
                self.scroll(Scroll::PageDown);
                Some(Vec::new())
            }
            _ => Some(Vec::new()),
        }
    }

    fn alt_key_with_mods(&mut self, key: Key, _mods: Mods) -> Option<Vec<Effect<Part>>> {
        match key {
            // Alt+Enter: the Properties of the selected icons (with none: the folder's).
            Key::Enter if self.view.selection().is_empty() => Some(vec![Effect::Activate(Part::Command(Command::FolderProperties))]),
            Key::Enter => Some(vec![Effect::Activate(Part::Command(Command::Shared(crate::desktop::DesktopCommand::ItemProperties)))]),
            Key::Space => Some(vec![Effect::Activate(Part::KeyMenu)]),
            // Windows 98 and 2000: Back, Forward, and the address bar's list.
            Key::Left if self.explorer() => Some(vec![Effect::Activate(Part::Command(Command::Back))]),
            Key::Right if self.explorer() => Some(vec![Effect::Activate(Part::Command(Command::Forward))]),
            Key::Down | Key::Up if self.field.focused => Some(vec![Effect::Activate(Part::AddressButton)]),
            _ => None,
        }
    }

    fn typed(&mut self, ch: char) -> Vec<Effect<Part>> {
        // A rename going on comes first, as for the keys (brief 97).
        if self.view.renaming().is_some() {
            if !ch.is_control() {
                self.view.rename_key(EditKey::Char(ch), false);
            }
            return Vec::new();
        }
        if self.field.focused {
            if !ch.is_control() {
                self.field.key(address::EditKey::Char(ch), false);
            }
            return Vec::new();
        }
        if self.tree_shown && self.tree_focus {
            if ch.is_control() || ch == ' ' || "+-*".contains(ch) {
                return Vec::new();
            }
            let page = self.tree_page();
            let moved = self.tree.key(pane::TreeKey::Char(ch), page);
            self.fit_tree();
            return if moved == pane::Moved::Selection { vec![Effect::Activate(Part::TreeMoved)] } else { Vec::new() };
        }
        if !ch.is_control() && ch != ' ' {
            self.view.key(IconKey::Char(ch));
            self.after_key();
        }
        Vec::new()
    }

    fn access(&mut self, ch: char) -> Vec<Effect<Part>> {
        let lower = ch.to_lowercase().next().unwrap_or(ch);
        let found = self.titles().iter().position(|t| {
            let (text, key) = crate::text::split_access_key(t);
            key.and_then(|k| text.chars().nth(k)).and_then(|c| c.to_lowercase().next()) == Some(lower)
        });
        found.map(|i| vec![Effect::Activate(Part::Menu(i))]).unwrap_or_default()
    }

    fn space(&mut self) -> Vec<Effect<Part>> {
        Vec::new()
    }

    fn press(&mut self, part: Option<Part>, x: i32, y: i32) -> Press<Part> {
        // Whether this press is the click that brought the window forward (used up by it).
        let activating = std::mem::take(&mut self.click_activated);
        self.after_rename = None;
        self.press_after_rename = None;
        self.rename_drag = false;
        // A press anywhere but on the icons while a label is renamed ends the rename first, the
        // name typed taken (the extension warning if it applies), as the old Windows did when the
        // box lost the keyboard (brief 98): typing can then never go into the file's name by
        // mistake. The press is remembered and, once the rename has ended without the warning,
        // carried on as if made then (`carry_on_after_rename`, brief 99): the old Windows took the
        // name AND did what the click asked. A press on the icons is handled below (on the box:
        // the caret; elsewhere: the rename ends and the press is carried on as a click there,
        // `click_after_rename`). While the extension warning is up a press does nothing.
        if self.view.renaming().is_some() && part != Some(Part::List) {
            if self.view.is_asking() {
                return Press::Done;
            }
            self.press_after_rename = part.map(|p| (p, x, y));
            return Press::Act(Part::RenameEnd(false));
        }
        match part {
            Some(p @ (Part::Minimize | Part::Maximize | Part::Close)) => Press::Hold(p),
            Some(Part::Icon) => Press::Act(Part::Icon),
            Some(Part::Menu(i)) => Press::Act(Part::Menu(i)),
            // 2000's Views is one drop-down: a press opens its menu at once.
            Some(Part::Tool(Tool::Views)) if self.era == Era::Win2000 => Press::Act(Part::ToolArrow(Tool::Views)),
            Some(Part::Tool(t)) if self.enabled(Part::Tool(t)) => Press::Hold(Part::Tool(t)),
            Some(Part::ToolArrow(t)) if self.enabled(Part::Tool(t)) => Press::Act(Part::ToolArrow(t)),
            Some(Part::Go) => Press::Hold(Part::Go),
            Some(Part::AddressButton) => {
                self.field.focus();
                Press::Act(Part::AddressButton)
            }
            Some(Part::ListRow(i)) => Press::Act(Part::ListRow(i)),
            Some(Part::Address) => {
                let Some(font) = self.font() else { return Press::Done };
                self.list = None;
                if self.field.focused {
                    // A click sets the caret (Shift: the selection grows); held, a drag selects.
                    let (tx, _) = self.chrome.address_text_at();
                    self.field.click(&font, x - tx, self.mods.shift);
                    Press::Grab(Part::Address, 0)
                } else {
                    // The first click from elsewhere selects the whole text (survey part 3).
                    self.field.focus();
                    Press::Done
                }
            }
            Some(Part::VThumb) => {
                self.grab_at = (x, y);
                Press::Grab(Part::VThumb, self.scroll.1)
            }
            Some(Part::HThumb) => {
                self.grab_at = (x, y);
                Press::Grab(Part::HThumb, self.scroll.0)
            }
            Some(Part::Scroll(s)) => {
                self.scroll(s);
                Press::Done
            }
            Some(Part::TreeScroll(s)) => {
                self.tree_scroll(s);
                Press::Done
            }
            Some(Part::TreeVThumb) => {
                self.tree_grab = (x, y);
                Press::Grab(Part::TreeVThumb, self.tree.top as i32)
            }
            Some(Part::TreeHThumb) => {
                self.tree_grab = (x, y);
                Press::Grab(Part::TreeHThumb, self.tree.scroll_x)
            }
            Some(Part::TreeClose) => Press::Hold(Part::TreeClose),
            Some(Part::WebLink(l)) => {
                if l == web::Link::ShowFiles {
                    self.show_files();
                }
                Press::Act(Part::WebLink(l))
            }
            Some(Part::WebScroll(s)) => {
                self.web_scroll(s);
                Press::Done
            }
            Some(Part::Header(k)) => Press::Hold(Part::Header(k)),
            Some(Part::Divider(k)) => {
                self.divider_drag = Some((k, x, x));
                Press::Grab(Part::Divider(k), 0)
            }
            Some(Part::Split) => {
                let l = self.pane_layout();
                self.split_drag = l.map(|l| l.split.x);
                self.tree_grab = (x, l.map_or(0, |l| l.split.x));
                Press::Grab(Part::Split, 0)
            }
            Some(Part::TreeBox(i)) => {
                self.field.blur();
                self.list = None;
                self.tree_focus = true;
                let page = self.tree_page();
                let moved = self.tree.toggle(i, page);
                self.fit_tree();
                if moved { Press::Act(Part::TreeChosen) } else { Press::Done }
            }
            Some(Part::TreeRow(i)) => {
                self.field.blur();
                self.list = None;
                self.tree_focus = true;
                if self.tree.sel == i {
                    return Press::Done;
                }
                let page = self.tree_page();
                self.tree.select(i, page);
                self.fit_tree();
                Press::Act(Part::TreeChosen)
            }
            Some(Part::TreeBlank) => {
                self.field.blur();
                self.list = None;
                self.tree_focus = true;
                Press::Done
            }
            Some(Part::List) => {
                self.field.blur();
                self.list = None;
                self.tree_focus = false;
                let Some(font) = self.font() else { return Press::Done };
                let mods = IconMods { ctrl: self.mods.ctrl, shift: self.mods.shift };
                if self.view.renaming().is_some() {
                    // While the extension warning is up the box takes no mouse (brief 97).
                    if self.view.is_asking() {
                        return Press::Done;
                    }
                    if self.view.rename_press(&font, x, y, self.mods.shift) {
                        self.rename_drag = true;
                        return Press::Grab(Part::List, RENAME_DRAG);
                    }
                    // The rename ends, and the press then goes on as a click there
                    // (`click_after_rename`, once the platform has taken the name).
                    self.after_rename = Some((x, y, mods));
                    return Press::Act(Part::RenameEnd(false));
                }
                // The one item already selected, pressed on its name (not its picture) while the
                // icons had the keyboard, in a window that was already in front: Windows' slow
                // second click that renames. The click that brings the window forward arms none
                // (the Reference PCs, survey part 4).
                let had_keyboard = !self.tree_focus && !self.field.focused && !activating;
                let alone = self.view.selection().len() == 1;
                let before = self.view.hit(&font, x, y).filter(|&i| alone && self.view.is_selected(i));
                let hit = self.view.press(&font, x, y, IconButton::Left, mods);
                self.first_press = hit.and_then(|i| self.view.items.get(i)).map(|item| item.id.clone());
                self.slow_rename = match (before, hit) {
                    (Some(b), Some(i)) if b == i && had_keyboard && !mods.ctrl && !mods.shift && !self.view.icon_rect(i).contains(x, y) => {
                        self.view.items.get(i).map(|item| (item.id.clone(), x, y))
                    }
                    _ => None,
                };
                if self.view.gesture_active() { Press::Grab(Part::List, 0) } else { Press::Done }
            }
            _ => Press::Nothing,
        }
    }

    fn double_click(&mut self, x: i32, y: i32) -> Option<Vec<Effect<Part>>> {
        let font = self.font()?;
        match self.hit(x, y) {
            // A word selected; an address without spaces is one word (survey part 3).
            Some(Part::Address) if self.field.focused => {
                let (tx, _) = self.chrome.address_text_at();
                let at = self.field.index_at(&font, x - tx);
                self.field.edit.double_click(at);
                self.field.caret_on = true;
                Some(Vec::new())
            }
            // In the rename box: a word selected (brief 95).
            Some(Part::List) if self.view.renaming().is_some() && self.view.rename_double_click(&font, x, y) => Some(Vec::new()),
            // Windows makes a double-click of two presses close in time and place, whatever
            // they land on; Explorer opened only when both landed on the same item (a second
            // item clicked soon after the first is only selected: the Reference PCs). A
            // double-click whose first press this window did not see (it went to another
            // window, or to another part) is a single press.
            Some(Part::List) if self.view.renaming().is_none() => {
                let hit = self.view.hit(&font, x, y);
                let same = hit.and_then(|i| self.view.items.get(i)).is_some_and(|item| self.first_press.as_ref() == Some(&item.id));
                self.slow_rename = None;
                if !same {
                    return None;
                }
                self.first_press = None;
                self.view.press(&font, x, y, IconButton::Left, IconMods::default());
                self.view.cancel();
                Some(vec![Effect::Activate(Part::Open)])
            }
            Some(Part::Icon) => Some(vec![Effect::Activate(Part::Close)]),
            // A double-click on a divider fits the column to its widest entry.
            Some(Part::Divider(k)) if k < self.view.columns.len() => {
                let mut widths = self.view.columns.clone();
                widths[k] = self.view.fitted_width(&font, k);
                self.view.set_columns(&widths);
                self.divider_drag = None;
                self.fit();
                Some(vec![Effect::Activate(Part::Divider(k))])
            }
            // A double-click on a row opens or closes it (Control Panel, Printers and the Recycle
            // Bin open Windows' own).
            Some(Part::TreeRow(i)) => {
                let page = self.tree_page();
                self.tree.select(i, page);
                if self.tree.nodes[i].kind.opens_windows_own() {
                    return Some(vec![Effect::Activate(Part::TreeChosen)]);
                }
                self.tree.toggle(i, page);
                self.fit_tree();
                Some(Vec::new())
            }
            None if y >= self.frame() && y < self.frame() + window::CAPTION_H => Some(vec![Effect::Activate(Part::Maximize)]),
            _ => None,
        }
    }

    fn drag(&mut self, part: Part, _grab_y: i32, value: i32, x: i32, y: i32) -> bool {
        match part {
            // Dragging over the address selects (survey part 3).
            Part::Address => {
                let Some(font) = self.font() else { return false };
                let (tx, _) = self.chrome.address_text_at();
                let before = (self.field.caret, self.field.anchor);
                let at = self.field.index_at(&font, x - tx);
                self.field.edit.drag(at);
                before != (self.field.caret, self.field.anchor)
            }
            Part::Divider(_) => {
                let before = self.divider_drag;
                let columns = self.view.columns.clone();
                if let Some(d) = self.divider_drag.as_mut() {
                    // Not narrower than nothing.
                    d.2 = x.max(d.1 - columns.get(d.0).copied().unwrap_or(0));
                }
                before != self.divider_drag
            }
            Part::Split => {
                let Some(l) = self.pane_layout() else { return false };
                let area = self.panes_area();
                let to = (self.tree_grab.1 + x - self.tree_grab.0).clamp(area.x + 24, (area.right() - 24 - pane::split_width(self.era)).max(area.x + 24));
                let _ = l;
                let before = self.split_drag;
                self.split_drag = Some(to);
                before != self.split_drag
            }
            Part::TreeVThumb => {
                let (Some(bar), _) = self.tree_bar_rects() else { return false };
                let Some((s, ..)) = self.tree_parts() else { return false };
                let total = self.tree.visible().len() as i32;
                let page = pane::page(s.h) as i32;
                let (_, len) = Self::thumb(bar.h, total, page, 0);
                let span = (bar.h - 2 * BAR - len).max(1);
                let before = self.tree.top;
                let moved = y - self.tree_grab.1;
                let to = value + (moved * (total - page).max(0) + moved.signum() * span / 2) / span;
                self.tree.top = to.max(0) as usize;
                self.fit_tree();
                self.tree.top != before
            }
            Part::TreeHThumb => {
                let (_, Some(bar)) = self.tree_bar_rects() else { return false };
                let Some((s, ..)) = self.tree_parts() else { return false };
                let Some(font) = self.font.clone() else { return false };
                let total = pane::content_width(&self.tree, &font);
                let (_, len) = Self::thumb(bar.w, total, s.w, 0);
                let span = (bar.w - 2 * BAR - len).max(1);
                let before = self.tree.scroll_x;
                self.tree.scroll_x = value + (x - self.tree_grab.0) * (total - s.w).max(0) / span;
                self.fit_tree();
                self.tree.scroll_x != before
            }
            // Dragging in the rename box selects (brief 95).
            Part::List if value == RENAME_DRAG => match self.font() {
                Some(font) if self.view.renaming().is_some() => self.view.rename_drag(&font, x, y),
                _ => false,
            },
            Part::List => {
                // Moved off the press: a drag, not a slow click.
                if self.slow_rename.as_ref().is_some_and(|&(_, px, py)| (px - x).abs() > 2 || (py - y).abs() > 2) {
                    self.slow_rename = None;
                }
                if let Some(font) = self.font() {
                    self.view.drag_to(&font, x, y);
                }
                if self.view.dragging() {
                    self.slow_rename = None;
                    self.drag_out = true;
                }
                true
            }
            // The thumb follows the pointer from where it was grabbed: one pixel of the track is
            // (the whole - the shown part) / (the track - the thumb) pixels of the list.
            Part::VThumb | Part::HThumb => {
                if self.font.is_none() {
                    return false;
                }
                let (cw, ch) = self.content();
                let shown = self.shown();
                let (v, h) = self.bar_rects();
                let (bar_len, total, page, moved) = match (part, v, h) {
                    (Part::VThumb, Some(b), _) => (b.h, ch, shown.h, y - self.grab_at.1),
                    (Part::HThumb, _, Some(b)) => (b.w, cw, shown.w, x - self.grab_at.0),
                    _ => return false,
                };
                let (_, len) = Self::thumb(bar_len, total, page, 0);
                let span = (bar_len - 2 * BAR - len).max(1);
                let to = value + moved * (total - page).max(0) / span;
                let before = self.scroll;
                if part == Part::VThumb {
                    self.scroll.1 = to;
                } else {
                    self.scroll.0 = to;
                }
                self.fit();
                self.scroll != before
            }
            _ => false,
        }
    }

    fn dropped(&mut self, part: Part, x: i32, y: i32) -> Vec<Effect<Part>> {
        if let Part::Divider(k) = part {
            // The column takes the width the line shows (`*-view-v1-hdr-divider-released`).
            if let Some((k0, from, to)) = self.divider_drag.take()
                && k0 == k
                && k < self.view.columns.len()
            {
                let mut widths = self.view.columns.clone();
                widths[k] = (widths[k] + to - from).max(0);
                self.view.set_columns(&widths);
                self.fit();
                return vec![Effect::Activate(Part::Divider(k))];
            }
            return Vec::new();
        }
        if part == Part::Split {
            // The pane takes the width the drag line shows (`*-tree-split-after`).
            if let (Some(to), Some(l)) = (self.split_drag.take(), self.pane_layout()) {
                self.tree_width += to - l.split.x;
                if let Some(now) = self.pane_layout() {
                    self.tree_width = now.split.x - self.panes_area().x;
                }
                self.fit();
                return vec![Effect::Activate(Part::Split)];
            }
            return Vec::new();
        }
        // A drag in the rename box was only a selection.
        if part == Part::List
            && !std::mem::take(&mut self.rename_drag)
            && let Some(font) = self.font()
        {
            self.view.release(&font, x, y);
        }
        Vec::new()
    }

    fn button(&mut self, button: Button, down: bool, x: i32, y: i32) -> Vec<Effect<Part>> {
        // A right or middle press that brought the window forward uses up the activating click: a
        // later left click on a name is a slow click again (the review of brief 90).
        if down {
            self.click_activated = false;
        }
        if button != Button::Right {
            return Vec::new();
        }
        let part = self.hit(x, y);
        // On the rename box the right button is the box's own: it keeps the rename and its
        // selection, and opens its menu when it comes up (brief 95).
        if part == Some(Part::List) && self.rename_hit(x, y) {
            return if down { Vec::new() } else { vec![Effect::FieldMenu] };
        }
        // A right press elsewhere while a label is renamed ends the rename first and is dropped,
        // its release too (brief 98: no menu from an earlier press's state).
        if down && self.view.renaming().is_some() {
            self.skip_right_up = true;
            self.right_hit = false;
            return if self.view.is_asking() { Vec::new() } else { vec![Effect::Activate(Part::RenameEnd(false))] };
        }
        if !down && std::mem::take(&mut self.skip_right_up) {
            return Vec::new();
        }
        if !down {
            // No right-button drag came of the press.
            self.view.cancel();
        }
        if down {
            // Windows' list view and tree view took the keyboard at a right press too, so the
            // item right-clicked shows selected (95 and 98 hide the selection of the pane without
            // the keyboard) and the keys go where the menu was opened.
            match part {
                Some(Part::List) => {
                    self.field.blur();
                    self.list = None;
                    self.tree_focus = false;
                    if let Some(font) = self.font() {
                        self.right_hit = self.view.press(&font, x, y, IconButton::Right, IconMods::default()).is_some();
                    }
                }
                Some(Part::TreeRow(_) | Part::TreeBox(_) | Part::TreeBlank) => {
                    self.field.blur();
                    self.list = None;
                    self.tree_focus = true;
                }
                // The address field takes the keyboard; a selection it has stays (brief 95).
                Some(Part::Address) if !self.field.focused => {
                    self.list = None;
                    self.tree_focus = false;
                    self.field.focus();
                }
                _ => {}
            }
            return Vec::new();
        }
        match part {
            Some(Part::Address) if self.field.focused => vec![Effect::FieldMenu],
            Some(Part::List) => {
                self.menu_at = (x, y);
                vec![Effect::Activate(if self.right_hit { Part::ItemMenu } else { Part::BackgroundMenu })]
            }
            Some(Part::TreeRow(i) | Part::TreeBox(i)) => {
                self.menu_at = (x, y);
                self.menu_on_row(i);
                vec![Effect::Activate(Part::TreeMenu)]
            }
            None if y >= self.frame() && y < self.frame() + window::CAPTION_H => {
                self.menu_at = (x, y);
                vec![Effect::Activate(Part::TitleMenu)]
            }
            _ => Vec::new(),
        }
    }

    fn set_active(&mut self, active: bool) {
        self.active = active;
        self.view.active = active;
        // (Another window took the keyboard while a label was renamed: the platform takes the name.)
        // A window that went behind forgets an activating click whose press it did not get.
        if !active {
            self.click_activated = false;
        }
    }

    fn activated_by_click(&mut self) {
        self.click_activated = true;
    }

    fn menu_key(&mut self) -> Vec<Effect<Part>> {
        vec![Effect::Activate(Part::Menu(0))]
    }

    fn wheel(&mut self, notches: i32) {
        if self.tree_shown && self.tree_focus {
            let top = self.tree.top as i32 - 3 * notches;
            self.tree.top = top.max(0) as usize;
            self.fit_tree();
            return;
        }
        match self.view.mode() {
            ViewMode::List => self.scroll_by(-notches * self.view.list_width(), 0),
            ViewMode::Details => self.scroll_by(0, -notches * 3 * ROW_H),
            _ => self.scroll_by(0, -notches * WHEEL),
        }
    }

    // Paste, Copy and the menu ask first whether a rename is going on, as the keys do (brief 97).
    fn paste(&mut self, text: &str) {
        if self.view.renaming().is_some() {
            self.view.rename_paste(text);
        } else if self.field.focused {
            self.field.paste(text);
        }
    }

    fn take_copy(&mut self) -> Option<String> {
        if self.view.renaming().is_some() { self.view.rename_take_copy() } else { self.field.copied.take() }
    }

    fn field_menu(&self, clipboard_has_text: bool) -> Option<crate::paint::Menu> {
        if self.view.renaming().is_some() {
            self.view.rename_menu(clipboard_has_text)
        } else {
            self.field.focused.then(|| self.field.menu(clipboard_has_text))
        }
    }

    fn field_menu_at(&self) -> Option<(i32, i32)> {
        let font = self.font()?;
        if self.view.renaming().is_some() {
            self.view.rename_caret_point(&font)
        } else {
            crate::edit::middle(self.rect(Part::Address)?)
        }
    }

    fn field_menu_chosen(&mut self, index: usize) -> Vec<Effect<Part>> {
        if self.view.renaming().is_some() {
            self.view.rename_menu_chosen(index)
        } else if self.field.focused {
            self.field.menu_chosen(index)
        } else {
            Vec::new()
        }
    }

    fn has_caret(&self) -> bool {
        self.view.renaming().is_some() || self.field.focused
    }

    fn blink(&mut self, on: bool) {
        self.field.caret_on = on;
        self.view.blink(on);
    }

    fn mouse_mods(&mut self, mods: Mods) {
        self.mods = mods;
    }

    fn pending(&mut self) -> Vec<Effect<Part>> {
        if std::mem::take(&mut self.drag_out) { vec![Effect::Activate(Part::DragOut)] } else { Vec::new() }
    }

    fn hover(&mut self, x: i32, y: i32) -> bool {
        // The right button held on an icon: a right-button drag once it goes far enough.
        if self.view.right_pressed() {
            if let Some(font) = self.font() {
                self.view.drag_to(&font, x, y);
            }
            if self.view.dragging() {
                self.drag_out = true;
            }
            return false;
        }
        let part = self.hit(x, y);
        let close = u8::from(part == Some(Part::TreeClose));
        let link = match part {
            Some(Part::WebLink(l)) => Some(l),
            _ => None,
        };
        let changed = (std::mem::replace(&mut self.close_look, close) != close) | (std::mem::replace(&mut self.link_hot, link) != link);
        if !self.explorer() {
            return changed;
        }
        self.set_hot(part) || changed
    }

    fn pointer_left(&mut self) -> bool {
        let changed = (std::mem::replace(&mut self.close_look, 0) != 0) | self.link_hot.take().is_some();
        (self.explorer() && self.set_hot(None)) || changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desktop::DesktopCommand as D;

    fn fonts() -> (Font, Font) {
        let regular = Font::parse(include_str!("../../tests/fixtures/ms-sans-serif-11px.font")).unwrap();
        let bold = Font::parse(include_str!("../../tests/fixtures/ms-sans-serif-11px-bold.font")).unwrap();
        (regular, bold)
    }

    fn items(n: usize) -> Vec<Item> {
        (0..n)
            .map(|i| Item {
                id: format!("C:\\t\\{i:02}"),
                name: format!("{i:02}"),
                picture: Picture::Folder,
                shortcut: false,
                folder: true,
                type_name: String::new(),
                size: 0,
                modified: 0,
            })
            .collect()
    }

    fn window(n: usize, size: (i32, i32)) -> FolderWindow {
        let (r, b) = fonts();
        let mut w = FolderWindow::new(Era::Win95, "t", Icon { width: 16, height: 16, pixels: vec![0; 256] }, true, size);
        w.layout(&Fonts { regular: &r, bold: &b });
        w.set_items(items(n));
        w
    }

    /// A place's late reading (its title, pictures, address, web facts) changes only what it read:
    /// the items shown, the selection, the scroll, the view, an open rename box and a typed address
    /// stay (an outside review of 2026-11-18: the reading started the view afresh, and the items,
    /// which the reader tells only of when they change, stayed away until Refresh). Only a
    /// navigation (`show_place`) starts the view afresh.
    #[test]
    fn a_late_reading_keeps_the_items_selection_and_scroll() {
        let (r, b) = fonts();
        let icon = || Icon { width: 16, height: 16, pixels: vec![0; 256] };
        for era in [Era::Win95, Era::Win98, Era::Win2000] {
            let mut w = FolderWindow::new(era, "Z:\\", icon(), true, (400, 300));
            w.layout(&Fonts { regular: &r, bold: &b });
            w.show_place("Z:\\", icon(), true, "Z:\\", icon());
            w.set_items(items(80));
            w.set_view_mode(ViewMode::Details);
            w.view.select_only(Some(40));
            w.scroll_by(0, 17 * 30);
            assert!(w.begin_rename(40));
            let scroll = w.scroll;
            assert!(scroll.1 > 0, "{era:?}: scrolled");
            w.field.text = "typed".into();
            w.field.edited = true;
            let ids: Vec<String> = w.view.items.iter().map(|i| i.id.clone()).collect();
            w.update_place("Bevel95 test (Z:)", Icon { width: 16, height: 16, pixels: vec![1; 256] }, "Bevel95 test (Z:)", icon());
            w.set_web_known(web::Kind::Drive, None, Some(Space { free: 1, capacity: 2 }));
            assert_eq!(w.view.items.iter().map(|i| i.id.clone()).collect::<Vec<_>>(), ids, "{era:?}: the same items");
            assert_eq!(w.view.selection(), vec![40], "{era:?}: the same selection");
            assert_eq!(w.scroll, scroll, "{era:?}: the same scroll");
            assert_eq!(w.view_mode(), ViewMode::Details);
            assert_eq!(w.view.renaming(), Some(40), "{era:?}: the rename box stays");
            assert_eq!(w.title, "Bevel95 test (Z:)");
            assert_eq!((w.field.text.as_str(), w.field.place_text.as_str()), ("typed", "Bevel95 test (Z:)"), "a typed address stays; Esc gives the new one");
            // Nothing typed: the address shown follows the reading.
            w.field.edited = false;
            w.update_place("Bevel95 test 2 (Z:)", icon(), "Bevel95 test 2 (Z:)", icon());
            assert_eq!(w.field.text, "Bevel95 test 2 (Z:)");
            // A navigation starts afresh.
            w.show_place("Bevel95 test", icon(), true, "Bevel95 test", icon());
            assert!(w.view.items.is_empty() && w.view.selection().is_empty() && w.scroll == (0, 0), "{era:?}");
        }
    }

    /// The menu on a tree row acts on the row it was opened for, by its id, though rows arrive
    /// while it is open (a slow drive's folders, a re-read after a paste) and move the rows below;
    /// once that row is gone, on nothing (the third safety review of 2026-11-18, A2: it kept the
    /// row's index, and Cut, Paste or Delete then acted on another folder).
    #[test]
    fn the_tree_rows_menu_acts_on_its_row_though_rows_arrive_while_it_is_open() {
        use pane::{NodeKind, NodeSpec};
        let folder = |id: &str, has_rows: bool| NodeSpec { id: id.into(), name: id.rsplit('\\').next().unwrap().into(), kind: NodeKind::Folder, has_rows, hidden: false };
        let mut w = window(0, (640, 480));
        w.tree.wanted();
        w.tree.set_rows("::desktop", vec![folder("C:\\Slow", true), folder("C:\\Bevel95 test b", false), folder("C:\\Bevel95 test c", false)]);
        let c = w.tree.find("C:\\Bevel95 test c").unwrap();
        w.menu_on_row(c);
        // The slow folder opens and its rows come while the menu is open: every row below moves.
        w.tree.toggle(w.tree.find("C:\\Slow").unwrap(), 20);
        w.tree.set_rows("C:\\Slow", vec![folder("C:\\Slow\\one", false), folder("C:\\Slow\\two", false)]);
        assert_ne!(w.tree.nodes[c].id, "C:\\Bevel95 test c", "the index now names another row");
        assert_eq!(w.menu_node().map(|n| n.id.as_str()), Some("C:\\Bevel95 test c"), "by its id: the row it was opened for");
        // Its row goes while the menu is open: the command acts on nothing (not on the selection).
        w.tree.set_rows("::desktop", vec![folder("C:\\Slow", true), folder("C:\\Bevel95 test b", false)]);
        assert!(w.menu_node().is_none(), "gone: no row at all");
        // Without a menu: the selected row.
        w.menu_row = None;
        assert_eq!(w.menu_node().map(|n| n.id.as_str()), Some("::desktop"));
    }

    #[test]
    fn icons_stand_in_rows_from_the_lists_corner() {
        // The Reference PC's t10 window (340 x 294): four columns, the first picture 21 and 2 in
        // from the list's inside, which starts 6 and 44 in from the window's corner.
        let w = window(10, (340, 294));
        assert_eq!(w.view.icon_rect(0), Rect::new(27, 46, 32, 32));
        assert_eq!(w.view.icon_rect(3), Rect::new(27 + 225, 46, 32, 32));
        assert_eq!(w.view.icon_rect(4), Rect::new(27, 46 + 75, 32, 32));
        assert_eq!(w.view.focus, Some(0), "the focus on the first icon, nothing selected");
        assert!(w.view.selection().is_empty());
        assert_eq!(w.bars(), (false, false));
    }

    #[test]
    fn a_narrower_window_keeps_its_icons_and_scrolls() {
        let mut w = window(7, (265, 294));
        let before = w.view.icon_rect(2);
        w.resize((175, 294), false);
        assert_eq!(w.view.icon_rect(2), before, "Windows 95 left them where they were (-pf-narrow)");
        assert!(w.bars().1, "a scroll bar across");
        // Auto Arrange puts them in rows as wide as the window again.
        w.view.set_auto_arrange(true);
        w.fit();
        assert_eq!(w.view.icon_rect(2), Rect::new(27, before.y + 150, 32, 32), "one to a row beside the scroll bar now");
    }

    #[test]
    fn too_many_icons_get_a_scroll_bar_and_rows_beside_it() {
        // t26 (490 x 412): six columns beside the scroll bar.
        let w = window(26, (490, 412));
        assert_eq!(w.bars(), (true, false));
        assert_eq!(w.view.icon_rect(6).y, 46 + 75);
        let mut w = w;
        w.scroll(Scroll::PageDown);
        assert!(w.scroll.1 > 0);
        w.scroll(Scroll::PageDown);
        w.scroll(Scroll::PageDown);
        assert_eq!(w.scroll.1, w.max_scroll().1, "no further than the last row");
    }

    #[test]
    fn the_status_bar_counts_as_windows_95_did() {
        let mut w = window(3, (210, 219));
        w.hidden = 2;
        w.total_size = 2048;
        assert_eq!(w.status(), Status { left: "3 object(s) (plus 2 hidden)".into(), right: "2.00KB".into() });
        w.view.select_only(Some(1));
        assert_eq!(w.status().left, "1 object(s) selected");
        assert_eq!(w.status().right, "", "a folder selected: no size (-c-sel-folder)");
    }

    #[test]
    fn a_right_press_on_the_icons_gives_them_the_keyboard() {
        // Explorer with the tree holding the keyboard: a right press on an icon moves the keyboard
        // to the icons (Windows' list view did), so the item shows selected under its menu.
        let (r, b) = fonts();
        let mut w = window(4, (600, 411));
        w.show_tree(true);
        w.layout(&Fonts { regular: &r, bold: &b });
        assert!(w.tree_focus);
        let at = w.view.icon_rect(1);
        let (x, y) = (at.x + 5, at.y + 5);
        assert_eq!(w.hit(x, y), Some(Part::List));
        w.button(Button::Right, true, x, y);
        assert!(!w.tree_focus, "the icons have the keyboard");
        assert_eq!(w.view.selection(), vec![1]);
        assert_eq!(w.button(Button::Right, false, x, y), vec![Effect::Activate(Part::ItemMenu)]);
    }

    #[test]
    fn mouse_and_keys_become_commands() {
        let mut w = window(4, (265, 219));
        let at = w.view.icon_rect(1);
        w.press(Some(Part::List), at.x + 5, at.y + 5);
        assert_eq!(w.double_click(at.x + 5, at.y + 5), Some(vec![Effect::Activate(Part::Open)]));
        assert_eq!(w.view.selection(), vec![1]);
        let key = |w: &mut FolderWindow, k: Key| w.key(k, Mods::default()).unwrap();
        assert_eq!(key(&mut w, Key::Delete), vec![Effect::Activate(Part::Command(Command::Shared(D::Delete)))]);
        assert_eq!(key(&mut w, Key::Other(VK_F2)), vec![Effect::Activate(Part::Command(Command::Shared(D::Rename)))]);
        assert_eq!(key(&mut w, Key::Other(VK_BACK)), vec![Effect::Activate(Part::Command(Command::Up))]);
        assert_eq!(key(&mut w, Key::Enter), vec![Effect::Activate(Part::Open)]);
        key(&mut w, Key::Right);
        assert_eq!(w.view.selection(), vec![2], "the arrows go along the row");
        // The right button on the empty area: its menu where it was let go.
        let empty = (w.shown().right() - 3, w.shown().bottom() - 3);
        w.button(Button::Right, true, empty.0, empty.1);
        assert_eq!(w.button(Button::Right, false, empty.0, empty.1), vec![Effect::Activate(Part::BackgroundMenu)]);
        assert_eq!(w.menu_at, empty);
        // Renaming: typed into the box, Enter ends it.
        assert!(w.begin_rename(2));
        assert!(w.has_caret());
        w.typed('x');
        assert_eq!(w.view.rename_text().as_deref(), Some("x"));
        assert_eq!(key(&mut w, Key::Enter), vec![Effect::Activate(Part::RenameEnd(true))]);
    }

    #[test]
    fn three_thousand_items_take_no_time() {
        // A folder of a few thousand items must never hold the window up (brief of step 3):
        // reading them in, laying them out and painting stay well under a second (debug build).
        let (r, b) = fonts();
        let fonts = Fonts { regular: &r, bold: &b };
        let mut w = FolderWindow::new(Era::Win2000, "t", Icon { width: 16, height: 16, pixels: vec![0; 256] }, true, (680, 458));
        w.layout(&fonts);
        let list: Vec<Item> = (0..3000)
            .map(|i| Item {
                id: format!("C:\\big\\Bevel95 test file {i:04}.txt"),
                name: format!("Bevel95 test file {i:04}.txt"),
                picture: Picture::Own(None),
                shortcut: false,
                folder: false,
                type_name: String::new(),
                size: 0,
                modified: 0,
            })
            .collect();
        let started = std::time::Instant::now();
        w.set_items(list.clone());
        let set = started.elapsed();
        w.set_items(list);
        let again = started.elapsed() - set;
        let mut fb = Framebuffer::new(680, 458, 0);
        let painted = std::time::Instant::now();
        w.paint(&mut fb, crate::theme::theme(Era::Win2000), &fonts);
        let paint = painted.elapsed();
        // On the clock only on request (BEVEL95_TIMED=1): a busy PC must not fail the default run
        // (brief 104).
        if std::env::var_os("BEVEL95_TIMED").is_some() {
            assert!(set.as_millis() < 1500 && again.as_millis() < 1500 && paint.as_millis() < 500, "set {set:?}, again {again:?}, paint {paint:?}");
        }
    }

    fn files(names: &[(&str, u64, u64)]) -> Vec<Item> {
        names
            .iter()
            .map(|&(n, size, when)| Item {
                id: format!("C:\\t\\{n}"),
                name: n.to_string(),
                picture: Picture::Own(None),
                shortcut: false,
                folder: false,
                type_name: if n.ends_with('b') { "B File".into() } else { "A File".into() },
                size,
                modified: when,
            })
            .collect()
    }

    #[test]
    fn each_view_lays_its_rows_out_as_measured() {
        // 95's v1 window (634 x 411): the list's inside from (6, 44).
        let mut w = window(30, (634, 411));
        w.set_view_mode(ViewMode::List);
        let rows = ((411 - 44 - 6 - 19 - 2) / 17) as usize;
        assert_eq!(w.view.list_rows(), rows);
        assert_eq!(w.view.icon_rect(0), Rect::new(6, 44, 16, 16), "the first picture at the inside corner");
        assert_eq!(w.view.icon_rect(1).y, 44 + 17, "rows 17 high");
        assert_eq!(w.view.icon_rect(rows).x, 6 + w.view.list_width(), "the next column");
        assert_eq!(w.bars(), (false, false));
        w.set_view_mode(ViewMode::Details);
        assert_eq!(w.view.icon_rect(0), Rect::new(8, 44 + 17, 16, 16), "under the header, 2 in");
        assert_eq!(w.view.columns, vec![120, 60, 120, 120]);
        assert_eq!(w.hit(10, 50), Some(Part::Header(0)));
        assert_eq!(w.hit(6 + 120, 50), Some(Part::Divider(0)));
        w.set_view_mode(ViewMode::Small);
        assert_eq!(w.view.icon_rect(1).x - w.view.icon_rect(0).x, 112, "95's small icons 112 apart");
    }

    #[test]
    fn a_header_click_sorts_and_a_second_turns_it_round() {
        let mut w = window(0, (634, 411));
        w.set_items(files(&[("c", 30, 3), ("a", 10, 1), ("bb", 20, 2)]));
        w.set_view_mode(ViewMode::Details);
        let names = |w: &FolderWindow| w.view.items.iter().map(|i| i.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&w), ["a", "bb", "c"]);
        w.header_click(1);
        assert_eq!(names(&w), ["a", "bb", "c"], "by size, smallest first");
        w.header_click(1);
        assert_eq!(names(&w), ["c", "bb", "a"], "the second click: reversed");
        w.header_click(3);
        assert_eq!(names(&w), ["c", "bb", "a"], "95: Modified newest first");
        w.header_click(0);
        assert_eq!(names(&w), ["a", "bb", "c"], "a new column starts in its own order");
        assert_eq!(w.view_state().arrange, crate::desktop_icons::Arrange::Name);
    }

    #[test]
    fn a_header_button_is_clicked_by_the_mouse() {
        // Through the kit's controller, as a real click goes: pressed, drawn in, acted on when
        // let go on it; let go elsewhere, nothing (found with real clicks, 2026-11-15).
        let mut w = window(0, (634, 411));
        w.set_items(files(&[("c", 30, 3), ("a", 10, 1)]));
        w.set_view_mode(ViewMode::Details);
        let mut c = crate::kit::Controller::default();
        let (x, y) = (6 + 120 + 20, 50);
        assert_eq!(w.hit(x, y), Some(Part::Header(1)));
        assert_eq!(c.mouse_down(&mut w, x, y), vec![Effect::Capture]);
        assert_eq!(w.pressed, Some(Part::Header(1)), "drawn in while held");
        assert_eq!(c.mouse_up(&mut w, x, y), vec![Effect::Release, Effect::Activate(Part::Header(1))]);
        c.mouse_down(&mut w, x, y);
        assert_eq!(c.mouse_up(&mut w, x, y + 100), vec![Effect::Release], "let go off it");
        // Scrolled across, the button moves with the columns.
        w.scroll.0 = 30;
        assert_eq!(w.rect(Part::Header(0)), Some(Rect::new(6 - 30, w.shown().y, 120, HEADER_H)));
    }

    #[test]
    fn a_double_click_opens_only_the_item_both_presses_landed_on() {
        // Windows sends a double-click for two presses close in time and place, whatever they
        // land on; a single click must never open anything.
        let mut w = window(3, (634, 411));
        let mut c = crate::kit::Controller::default();
        let at = |w: &FolderWindow, i: usize| {
            let r = w.view.icon_rect(i);
            (r.x + r.w / 2, r.y + r.h / 2)
        };
        let (ax, ay) = at(&w, 0);
        let (bx, by) = at(&w, 1);
        c.mouse_down(&mut w, ax, ay);
        c.mouse_up(&mut w, ax, ay);
        // The second press, on another item, comes as a double-click: it only selects that item.
        assert_eq!(w.double_click(bx, by), None);
        c.mouse_down(&mut w, bx, by);
        c.mouse_up(&mut w, bx, by);
        assert_eq!(w.view.selection(), vec![1]);
        // A double-click whose first press this window never saw: nothing opens.
        let mut fresh = window(3, (634, 411));
        assert_eq!(fresh.double_click(ax, ay), None);
        // Both presses on the same item: it opens.
        assert_eq!(w.double_click(bx, by), Some(vec![Effect::Activate(Part::Open)]));
    }

    #[test]
    fn a_slow_second_click_on_a_selected_name_renames() {
        // The Reference PCs (measure17, D2): a second click on the name of the selected item,
        // later than the double-click time, renames; on its picture, or as a double-click, not.
        let mut w = window(3, (634, 411));
        let mut c = crate::kit::Controller::default();
        let pic = w.view.icon_rect(0);
        let (px, py) = (pic.x + pic.w / 2, pic.y + pic.h / 2);
        let (lx, ly) = (px, pic.bottom() + 6);
        c.mouse_down(&mut w, px, py);
        c.mouse_up(&mut w, px, py);
        assert!(!w.slow_rename_waiting(), "the first click only selects");
        c.mouse_down(&mut w, px, py);
        c.mouse_up(&mut w, px, py);
        assert!(!w.slow_rename_waiting(), "the picture does not rename");
        c.mouse_down(&mut w, lx, ly);
        c.mouse_up(&mut w, lx, ly);
        assert!(w.slow_rename_waiting(), "the name does");
        assert_eq!(w.slow_rename_due(), Some(0));
        assert!(!w.slow_rename_waiting(), "used up");
        // A double-click following it: no renaming.
        c.mouse_down(&mut w, lx, ly);
        c.mouse_up(&mut w, lx, ly);
        assert!(w.double_click(lx, ly).is_some());
        assert_eq!(w.slow_rename_due(), None);
        // Details: the name right of the picture.
        let mut w = window(3, (634, 411));
        w.set_view_mode(ViewMode::Details);
        let pic = w.view.icon_rect(0);
        let (lx, ly) = (pic.right() + 4, pic.y + pic.h / 2);
        for _ in 0..2 {
            c.mouse_down(&mut w, lx, ly);
            c.mouse_up(&mut w, lx, ly);
        }
        assert_eq!(w.slow_rename_due(), Some(0));
    }

    /// The click that brings a folder window forward selects but arms no rename; a slow second
    /// click in the window then in front does (the Reference PCs, survey part 4; brief 90).
    #[test]
    fn the_click_that_brings_the_window_forward_renames_nothing() {
        let mut w = window(3, (634, 411));
        let mut c = crate::kit::Controller::default();
        let pic = w.view.icon_rect(0);
        let (px, py) = (pic.x + pic.w / 2, pic.y + pic.h / 2);
        let (lx, ly) = (px, pic.bottom() + 6);
        c.mouse_down(&mut w, px, py);
        c.mouse_up(&mut w, px, py);
        // Another program came forward; a click on the selected name brings the window back.
        w.set_active(false);
        w.set_active(true);
        w.activated_by_click();
        c.mouse_down(&mut w, lx, ly);
        c.mouse_up(&mut w, lx, ly);
        assert!(!w.slow_rename_waiting(), "the activating click arms no rename");
        assert_eq!(w.view.selection(), vec![0], "it selects");
        c.mouse_down(&mut w, lx, ly);
        c.mouse_up(&mut w, lx, ly);
        assert!(w.slow_rename_waiting(), "the next slow click in the window in front does");
        // Brought forward by the keyboard (Alt+Tab): the first click on the name is a slow click.
        let _ = w.slow_rename_due();
        w.set_active(false);
        w.set_active(true);
        c.mouse_down(&mut w, lx, ly);
        c.mouse_up(&mut w, lx, ly);
        assert!(w.slow_rename_waiting());
        // A right press that brought the window forward uses the activating click up.
        let _ = w.slow_rename_due();
        w.set_active(false);
        w.set_active(true);
        w.activated_by_click();
        let _ = w.button(Button::Right, true, lx, ly);
        let _ = w.button(Button::Right, false, lx, ly);
        c.mouse_down(&mut w, lx, ly);
        c.mouse_up(&mut w, lx, ly);
        assert!(w.slow_rename_waiting(), "after a right-click activation, a slow click");
        // A window that went behind again forgets an activating click it did not get the press of.
        let _ = w.slow_rename_due();
        w.activated_by_click();
        w.set_active(false);
        w.set_active(true);
        c.mouse_down(&mut w, lx, ly);
        c.mouse_up(&mut w, lx, ly);
        assert!(w.slow_rename_waiting());
    }

    /// A press on another file while a name is being renamed ends the rename and selects that
    /// file, and a double-click on it opens it (the Reference PCs, survey part 4; brief 90).
    #[test]
    fn a_click_that_ends_a_rename_also_selects() {
        let mut w = window(3, (634, 411));
        let mut c = crate::kit::Controller::default();
        let other = w.view.icon_rect(1);
        let (ox, oy) = (other.x + other.w / 2, other.y + other.h / 2);
        assert!(w.view.begin_rename(0));
        let effects = c.mouse_down(&mut w, ox, oy);
        assert!(effects.contains(&Effect::Activate(Part::RenameEnd(false))), "{effects:?}");
        // The platform takes the name, then the press goes on.
        w.view.end_rename();
        assert!(w.click_after_rename());
        assert_eq!(w.view.selection(), vec![1], "the file clicked is selected");
        c.mouse_up(&mut w, ox, oy);
        assert_eq!(w.double_click(ox, oy), Some(vec![Effect::Activate(Part::Open)]), "a double-click opens it");
        // A question came up instead (the name is still being renamed): the press goes no further.
        let mut w = window(3, (634, 411));
        assert!(w.view.begin_rename(0));
        c.mouse_down(&mut w, ox, oy);
        assert!(!w.click_after_rename());
        assert_eq!(w.view.renaming(), Some(0));
        // A later press forgets it.
        w.view.end_rename();
        c.mouse_up(&mut w, ox, oy);
        let pic = w.view.icon_rect(2);
        c.mouse_down(&mut w, pic.x + pic.w / 2, pic.y + pic.h / 2);
        assert!(!w.click_after_rename());
    }

    /// Brief 98: a press anywhere but on the icons during a rename ends the rename first (the name
    /// typed taken), and that press is dropped: what is typed afterwards into the address field
    /// never goes into the file's name, and Enter goes to the address, not the rename. The same
    /// for a toolbar button and the right button.
    #[test]
    fn a_press_elsewhere_ends_the_rename_first() {
        let (r, b) = fonts();
        let fonts = Fonts { regular: &r, bold: &b };
        let new = || {
            let mut w = FolderWindow::new(Era::Win98, "My Computer", Icon { width: 16, height: 16, pixels: vec![0; 256] }, false, (512, 461));
            w.layout(&fonts);
            w.show_place("My Computer", Icon { width: 16, height: 16, pixels: vec![0; 256] }, false, "My Computer", Icon { width: 16, height: 16, pixels: vec![0; 256] });
            w.set_items(items(3));
            w
        };
        // The address field, its button, Go, toolbar buttons and an arrow, the menu bar, the tree,
        // the scroll bars, the title bar's buttons: each ends the rename, and once the name is
        // taken the press goes on (brief 99): it does what the same press does on a window with no
        // rename (a held part acting at once), a thumb's drag dropped.
        let parts = [
            Part::Address,
            Part::AddressButton,
            Part::Go,
            Part::Tool(Tool::Up),
            Part::Tool(Tool::Views),
            Part::ToolArrow(Tool::Back),
            Part::Menu(0),
            Part::TreeBlank,
            Part::VThumb,
            Part::Scroll(Scroll::PageDown),
            Part::Minimize,
            Part::Close,
        ];
        for part in parts {
            let what = format!("{part:?}");
            // What the same press does with no rename.
            let mut plain = new();
            let expected = match plain.press(Some(part), 200, 100) {
                Press::Hold(p) | Press::Act(p) => vec![Effect::Activate(p)],
                _ => Vec::new(),
            };
            let mut w = new();
            assert!(w.begin_rename(0));
            for ch in "new name".chars() {
                w.typed(ch);
            }
            assert_eq!(w.press(Some(part), 200, 100), Press::Act(Part::RenameEnd(false)), "{what}");
            assert!(!w.click_after_rename(), "{what}: not a click on the icons");
            // The platform takes the name typed into the box.
            assert_eq!(w.view.end_rename(), crate::desktop_icons::RenameEnd::Rename { id: w.view.items[0].id.clone(), name: "new name".into() });
            assert_eq!(w.carry_on_after_rename(), expected, "{what}: the press goes on");
            assert_eq!(w.field.focused, plain.field.focused, "{what}");
            assert!(w.carry_on_after_rename().is_empty(), "{what}: once");
            if part == Part::Address {
                assert!(w.field.focused && w.field.all_selected(), "the address field takes the keyboard, all selected");
            }
            // Now the address field: what is typed goes there, Enter goes to the address.
            if !w.field.focused {
                w.press(Some(Part::Address), 200, 100);
            }
            w.key(Key::Other(u16::from(b'A')), Mods { ctrl: true, shift: false });
            for ch in "C:\\".chars() {
                w.typed(ch);
            }
            assert_eq!(w.field.text, "C:\\", "{what}");
            assert!(w.view.renaming().is_none());
            assert_eq!(w.key(Key::Enter, Mods::default()), Some(vec![Effect::Activate(Part::Go)]), "{what}");
        }
        // The right button elsewhere: the rename ends, its release opens no menu.
        let mut w = new();
        assert!(w.begin_rename(0));
        assert_eq!(w.button(Button::Right, true, 200, 100), vec![Effect::Activate(Part::RenameEnd(false))]);
        w.view.end_rename();
        assert_eq!(w.button(Button::Right, false, 200, 100), Vec::new());
        // The extension warning came at the name taken: the press is dropped.
        let mut w = new();
        assert!(w.begin_rename(0));
        assert_eq!(w.press(Some(Part::Go), 200, 100), Press::Act(Part::RenameEnd(false)));
        w.view.rename_asking(true);
        assert!(w.carry_on_after_rename().is_empty(), "the warning: dropped");
        // While the extension warning is up a press does nothing.
        let mut w = new();
        assert!(w.begin_rename(0));
        w.view.rename_asking(true);
        assert_eq!(w.press(Some(Part::Address), 200, 100), Press::Done);
        assert_eq!(w.view.renaming(), Some(0));
    }

    #[test]
    fn the_pointer_changes_where_each_windows_changed_it() {
        // Measured on the Reference PCs (private/probes/measure17): the header's divider pointer 8
        // left to 7 right of the next column's first pixel; the bar between the panes: 95 its
        // own pointer over the 3-pixel bar, 98 and 2000 the two-sided arrow over the bar and the
        // tree pane's two edge columns.
        for era in [Era::Win95, Era::Win98, Era::Win2000] {
            let (r, b) = fonts();
            let mut w = FolderWindow::new(era, "t", Icon { width: 16, height: 16, pixels: vec![0; 256] }, true, (800, 560));
            w.layout(&Fonts { regular: &r, bold: &b });
            w.set_items(files(&[("c", 30, 3), ("a", 10, 1)]));
            w.exploring = true;
            w.show_tree(true);
            w.set_view_mode(ViewMode::Details);
            let h = w.rect(Part::Header(0)).unwrap();
            let edge = h.right();
            assert_eq!(w.pointer(edge - 8, h.y + 5), Pointer::HeaderDivider, "{era:?}");
            assert_eq!(w.pointer(edge + 7, h.y + 5), Pointer::HeaderDivider, "{era:?}");
            assert_eq!(w.pointer(edge - 9, h.y + 5), Pointer::Arrow, "{era:?}");
            assert_eq!(w.pointer(edge + 8, h.y + 5), Pointer::Arrow, "{era:?}");
            let s = w.rect(Part::Split).unwrap();
            let (first, bar) = if era == Era::Win95 { (s.x, Pointer::Split95) } else { (s.x - 2, Pointer::SizeWE) };
            assert_eq!(w.pointer(first, s.y + 50), bar, "{era:?}");
            assert_eq!(w.pointer(s.right() - 1, s.y + 50), bar, "{era:?}");
            assert_eq!(w.pointer(first - 1, s.y + 50), Pointer::Arrow, "{era:?}");
            assert_eq!(w.hit(first, s.y + 50), Some(Part::Split), "a press there takes the bar");
        }
    }

    #[test]
    fn columns_are_dragged_and_fitted() {
        let mut w = window(0, (634, 411));
        w.set_items(files(&[("a fairly long name of a file", 1, 1)]));
        w.set_view_mode(ViewMode::Details);
        let edge = 6 + 120;
        assert_eq!(w.press(Some(Part::Divider(0)), edge, 50), Press::Grab(Part::Divider(0), 0));
        w.drag(Part::Divider(0), 0, 0, edge + 30, 50);
        assert_eq!(w.dropped(Part::Divider(0), edge + 30, 50), vec![Effect::Activate(Part::Divider(0))]);
        assert_eq!(w.view.columns[0], 150);
        assert_eq!(w.view_state().columns, vec![150, 60, 120, 120], "remembered (not the look's own)");
        w.double_click(6 + 150, 50);
        let fitted = w.view.columns[0];
        assert_eq!(fitted, w.font().unwrap().measure("a fairly long name of a file") + 24);
    }

    #[test]
    fn keys_move_as_each_view_did() {
        let mut w = window(120, (520, 412));
        w.set_view_mode(ViewMode::List);
        let rows = w.view.list_rows();
        w.view.select_only(Some(0));
        let key = |w: &mut FolderWindow, k: Key| {
            w.key(k, Mods::default());
            w.view.focus.unwrap()
        };
        assert_eq!(key(&mut w, Key::Down), 1);
        assert_eq!(key(&mut w, Key::Right), 1 + rows, "the same row of the next column");
        assert_eq!(key(&mut w, Key::End), 119);
        assert_eq!(key(&mut w, Key::Home), 0);
        w.set_view_mode(ViewMode::Details);
        w.view.select_only(Some(0));
        assert_eq!(key(&mut w, Key::Right), 0, "Details: nothing");
        let page = key(&mut w, Key::PageDown);
        assert!(page > 5, "the last row that shows whole");
        assert_eq!(key(&mut w, Key::PageDown), (2 * page).min(119));
        assert_eq!(key(&mut w, Key::End), 119);
        assert!(w.scroll.1 > 0, "scrolled to the last row");
        assert_eq!(w.scroll.1 % ROW_H, 0, "by whole rows");
    }

    #[test]
    fn ten_thousand_items_scroll_and_paint_in_no_time() {
        let (r, b) = fonts();
        let fonts = Fonts { regular: &r, bold: &b };
        let mut w = FolderWindow::new(Era::Win98, "t", Icon { width: 16, height: 16, pixels: vec![0; 256] }, true, (680, 458));
        w.layout(&fonts);
        let list: Vec<Item> = (0..10_000)
            .map(|i| Item {
                id: format!("C:\\big\\Bevel95 test file {i:05}.txt"),
                name: format!("Bevel95 test file {i:05}.txt"),
                picture: Picture::Own(None),
                shortcut: false,
                folder: false,
                type_name: "Text Document".into(),
                size: i,
                modified: 0,
            })
            .collect();
        w.set_items(list);
        let mut fb = Framebuffer::new(680, 458, 0);
        for mode in ViewMode::ALL {
            w.set_view_mode(mode);
            let started = std::time::Instant::now();
            for _ in 0..10 {
                w.scroll(Scroll::PageDown);
                w.scroll(Scroll::PageRight);
                w.paint(&mut fb, crate::theme::theme(Era::Win98), &fonts);
            }
            let t = started.elapsed();
            // On the clock only on request (BEVEL95_TIMED=1; brief 104).
            if std::env::var_os("BEVEL95_TIMED").is_some() {
                assert!(t.as_millis() < 2000, "{mode:?}: 10 pages {t:?}");
            }
        }
        let started = std::time::Instant::now();
        w.header_click(1);
        if std::env::var_os("BEVEL95_TIMED").is_some() {
            assert!(started.elapsed().as_millis() < 2000, "a sort of 10,000 items");
        }
    }

    #[test]
    fn another_window_in_front_hides_the_selection() {
        let mut w = window(2, (210, 201));
        w.view.select_only(Some(0));
        w.set_active(false);
        assert!(!w.view.active, "as the Reference PC's inactive My Computer (-mc-inactive-sel)");
    }
    #[test]
    fn windows_98_has_explorers_toolbar_and_address_bar() {
        let (r, b) = fonts();
        let mut w = FolderWindow::new(Era::Win98, "My Computer", Icon { width: 16, height: 16, pixels: vec![0; 256] }, false, (512, 461));
        w.layout(&Fonts { regular: &r, bold: &b });
        w.show_place("My Computer", Icon { width: 16, height: 16, pixels: vec![0; 256] }, false, "My Computer", Icon { width: 16, height: 16, pixels: vec![0; 256] });
        w.set_items(items(3));
        assert_eq!(w.list().y, 118, "the list below the rebar (`win98-truecolor-folder-mycomputer`)");
        // Back, Forward and Up grayed in a new window: a press does nothing.
        assert_eq!(w.hit(20, 60), Some(Part::Tool(Tool::Back)));
        assert!(!w.enabled(Part::Tool(Tool::Back)));
        assert_eq!(w.press(Some(Part::Tool(Tool::Back)), 20, 60), Press::Nothing);
        w.set_nav(menus::Nav { back: true, ..menus::Nav::default() });
        assert_eq!(w.press(Some(Part::Tool(Tool::Back)), 20, 60), Press::Hold(Part::Tool(Tool::Back)));
        assert_eq!(w.hit(72, 60), Some(Part::ToolArrow(Tool::Back)));
        assert_eq!(w.hit(20, 30), Some(Part::Menu(0)));
        // The pointer raises a button; Alt+Left and Alt+Right go back and forward.
        assert!(w.hover(230, 60));
        assert_eq!(w.bar.hot_tool, Some((Tool::Cut, false)));
        assert_eq!(w.alt_key_with_mods(Key::Left, Mods::default()), Some(vec![Effect::Activate(Part::Command(Command::Back))]));
        // A click in the address bar selects it all; typing replaces it, Enter goes there.
        assert_eq!(w.hit(200, 100), Some(Part::Address));
        w.press(Some(Part::Address), 200, 100);
        assert!(w.field.focused && w.has_caret());
        for ch in "C:\\".chars() {
            w.typed(ch);
        }
        assert_eq!(w.field.text, "C:\\");
        assert_eq!(w.key(Key::Enter, Mods::default()), Some(vec![Effect::Activate(Part::Go)]));
        // Esc gives the place's address back.
        w.key(Key::Escape, Mods::default());
        assert_eq!(w.field.text, "My Computer");
        // Brief 95: Ctrl+C copies the selected address for the clipboard; from the icons Ctrl+L
        // selects the field whole; Ctrl+V asks for the clipboard; the right button opens its menu.
        let ctrl = Mods { ctrl: true, shift: false };
        assert_eq!(w.key(Key::Other(u16::from(b'C')), ctrl), Some(vec![Effect::Copy]));
        assert_eq!(w.take_copy().as_deref(), Some("My Computer"));
        w.key(Key::Tab, Mods::default());
        assert!(!w.field.focused);
        w.key(Key::Other(VK_L), ctrl);
        assert!(w.field.focused && w.field.all_selected(), "Ctrl+L: the field selected whole");
        assert_eq!(w.key(Key::Other(u16::from(b'V')), ctrl), Some(vec![Effect::Paste]));
        w.paste("C:\\WINDOWS\r\nsecond line");
        assert_eq!(w.field.text, "C:\\WINDOWS");
        assert_eq!(w.button(Button::Right, true, 200, 100), Vec::new());
        assert_eq!(w.button(Button::Right, false, 200, 100), vec![Effect::FieldMenu]);
        assert!(w.field_menu(true).is_some_and(|m| !m.entries[0].disabled), "Undo is there after the paste");
        w.key(Key::Escape, Mods::default());
        // The list: its rows take clicks, the arrows move the highlight.
        w.set_list(Some((vec![ListRow { level: 0, icon: Icon { width: 16, height: 16, pixels: vec![0; 256] }, name: "Desktop".into() }], None)));
        assert_eq!(w.hit(80, 120), Some(Part::ListRow(0)));
        w.key(Key::Down, Mods::default());
        assert_eq!(w.list.as_ref().unwrap().1, Some(0));
        assert_eq!(w.key(Key::Enter, Mods::default()), Some(vec![Effect::Activate(Part::ListRow(0))]));
    }
}

