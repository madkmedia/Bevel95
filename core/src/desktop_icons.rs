//! The icons on Bevel95's desktop: which there are and in what order, where each lies, how its
//! label wraps, what the mouse and the keys do (selecting, dragging, the selection rectangle),
//! arranging them, and painting them, for each look. The platform layer reads the real desktop's
//! items and their pictures, keeps the places file and shows the pixels.
//!
//! Measured on the Reference PCs (`private/reference/refs/vm/<era>-truecolor-desk-*.png` and `-move-*.png`,
//! numbers in docs/theme-spec.md): icons stand on a grid of 75-pixel cells (Windows' icon
//! spacing); the 32×32 picture 21 pixels from the left of its cell and 2 from the top; the label's
//! box 38 pixels below the cell's top, as wide as its widest line plus 4 and 13 pixels per line
//! plus 2, centered under the picture, its lines centered in it. Lines wrap at spaces to fit 71
//! pixels. A word longer than that ends in "..."; only Windows 2000 breaks it between letters, and
//! only on the label showing in full. Windows 98 and 2000 show at most two lines of a label, the
//! second ending in "...", except on the icon with the keyboard focus while the desktop has the
//! keyboard; Windows 95 always shows every line.
//!
//! Places: all three Windows let an icon lie anywhere. A dragged icon comes to rest exactly where
//! it was dropped (moved by the pointer's way, to the pixel); Line up Icons puts each onto the
//! nearest cell of the grid; Auto Arrange (off by default in all three) packs them into columns
//! from the top left in the order they lie (column by column, then from the top), again after
//! every drop. A new item made by the desktop's menu lies with its picture's corner at the point
//! the menu was opened; any other new item takes the first free cell, column by column. Until the
//! owner moves an icon, Bevel95 keeps its earlier columns (`places` is `None`): every icon in
//! Arrange Icons' order, flowing again when items come and go.
//!
//! Selection: several icons at once. While the desktop has the keyboard, selected icons' pictures
//! are shown selected and their labels white on the selection color, and the focus icon's label
//! has a dotted frame; when another window is in front, the desktop shows neither (the Reference
//! PCs' desktop, `*-desk-sel-inactive`). A click selects one icon, Ctrl+click adds or takes one
//! away, Shift+click selects every icon in the rectangle around the anchor and the clicked icon
//! (Windows' icon view is spatial), a drag on the empty desktop draws the dotted selection
//! rectangle and selects every icon it touches, Ctrl+A selects all, Enter opens every selected
//! icon (all three did). The arrows go to the nearest icon in their direction.

use std::collections::{HashMap, HashSet};

use crate::desktop_art::{self, Art};
use crate::fb::{Framebuffer, Icon, Rect};
use crate::text::{Font, Fonts};
use crate::theme::{Era, Theme};

/// Windows' icon spacing: one icon every 75 pixels, down and across.
pub const CELL: i32 = 75;
/// The picture's place in its cell.
const ICON_X: i32 = 21;
const ICON_Y: i32 = 2;
const ICON: i32 = 32;
/// The label box's top, from the cell's top.
const LABEL_Y: i32 = 38;
/// One line of a label.
const LINE_H: i32 = 13;
/// Lines wrap to fit this width (the label box adds 2 pixels on each side).
const WRAP_W: i32 = 71;
/// How far the pointer must go with the button held before an icon is dragged: Windows' drag
/// rectangle (SM_CXDRAG, 4 pixels) is centered on the press, so more than 2 either way.
const DRAG_START: i32 = 2;
/// Windows 2000's drag pictures are see-through: about 47% of the picture over what is under it
/// (measured on its desktop: 0A246A over 3A6EA5 gives 244B89, FFFFFF gives 97B2CF).
const DRAG_ALPHA_2000: u32 = 120;
/// Several icons dragged in Windows 95 and 98: a dotted box where each picture goes (48×36 from
/// the cell's top, centered) and a dotted line across the middle of its label.
const OUTLINE_X: i32 = 13;
const OUTLINE_W: i32 = 48;
const OUTLINE_H: i32 = 36;

/// The desktop's own icons, which are not files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum System {
    MyComputer,
    MyDocuments,
    RecycleBin,
}

impl System {
    pub fn name(self) -> &'static str {
        match self {
            System::MyComputer => "My Computer",
            System::MyDocuments => "My Documents",
            System::RecycleBin => "Recycle Bin",
        }
    }

    /// The id the platform layer opens it by.
    pub fn id(self) -> &'static str {
        match self {
            System::MyComputer => "::my-computer",
            System::MyDocuments => "::my-documents",
            System::RecycleBin => "::recycle-bin",
        }
    }
}

/// The look's own icons, in the order its desktop showed them (the Reference PCs): Windows 95 had
/// no My Documents, Windows 2000 put it first.
pub fn system_icons(era: Era) -> &'static [System] {
    match era {
        Era::Win95 => &[System::MyComputer, System::RecycleBin],
        Era::Win98 => &[System::MyComputer, System::MyDocuments, System::RecycleBin],
        Era::Win2000 => &[System::MyDocuments, System::MyComputer, System::RecycleBin],
    }
}

/// Where an icon view is: the classic desktop, or the inside of a folder window. Both are
/// Windows' own list view in its icon mode; they differ in where new icons go (the desktop fills
/// columns, a window rows as wide as it is), in their colors (the desktop's color with white
/// labels; a window's white with black labels) and in what the mouse can do (icons are moved only
/// on the desktop so far).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Host {
    Desktop,
    Window,
}

/// The look's own icons in a view: the desktop's (`system_icons`); a folder window has none.
fn own_icons(host: Host, era: Era) -> &'static [System] {
    match host {
        Host::Desktop => system_icons(era),
        Host::Window => &[],
    }
}

/// What an icon's picture is.
#[derive(Clone, Debug)]
pub enum Picture {
    /// One of the desktop's own icons, in the look's own picture.
    System(System),
    /// A folder: the look's own closed folder.
    Folder,
    /// Another picture of the look's own (a drive, Control Panel, Printers).
    Art(Art),
    /// Today's picture from the shell, 32×32 (`None` until it has been read).
    Own(Option<Icon>),
    /// Today's pictures from the shell, 32×32 and 16×16, shared by every item that shows the same
    /// (a folder window keeps one copy per kind of file: `ShellPicture`).
    Shell(std::sync::Arc<ShellPicture>),
    /// The look's own picture for a kind of file its Windows knew (`crate::file_kinds`), found by
    /// the item's ending (its id) when it is drawn, at the screen's depth.
    Kind(crate::file_kinds::Kind),
}

/// A file's pictures as the shell gives them, large (32×32) and small (16×16).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShellPicture {
    pub large: Option<Icon>,
    pub small: Option<Icon>,
}

/// One icon on the desktop.
#[derive(Clone, Debug)]
pub struct Item {
    /// What it stands for: a file's full path, or a `System::id`.
    pub id: String,
    /// Its label, as Explorer names it.
    pub name: String,
    pub picture: Picture,
    /// A shortcut: the look's arrow over its picture's lower left corner.
    pub shortcut: bool,
    /// A folder (folders come before files, as Explorer arranges them).
    pub folder: bool,
    /// For Arrange Icons: the shell's type name, the size in bytes, the time it was last
    /// changed (any number that grows with time).
    pub type_name: String,
    pub size: u64,
    pub modified: u64,
}

impl Item {
    pub fn system(system: System) -> Item {
        Item {
            id: system.id().to_string(),
            name: system.name().to_string(),
            picture: Picture::System(system),
            shortcut: false,
            folder: false,
            type_name: String::new(),
            size: 0,
            modified: 0,
        }
    }

    fn system_kind(&self) -> Option<System> {
        match self.picture {
            Picture::System(s) => Some(s),
            _ => None,
        }
    }
}

/// How Arrange Icons orders the owner's items (the look's own icons always come first, in
/// their own order).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Arrange {
    #[default]
    Name,
    Type,
    Size,
    Date,
}

impl Arrange {
    pub const ALL: [Arrange; 4] = [Arrange::Name, Arrange::Type, Arrange::Size, Arrange::Date];

    /// Its word in the places file.
    pub fn word(self) -> &'static str {
        match self {
            Arrange::Name => "name",
            Arrange::Type => "type",
            Arrange::Size => "size",
            Arrange::Date => "date",
        }
    }

    pub fn from_word(word: &str) -> Option<Arrange> {
        Arrange::ALL.into_iter().find(|a| a.word() == word)
    }
}

/// Puts `items` (the owner's, no system icons) in the order `by` asks for: folders first, as
/// Explorer does, then by name, type (then name), size (smallest first) or date (newest
/// first). Names compare without regard to case.
pub fn arrange(items: &mut [Item], by: Arrange) {
    // The keys are made once per item (a folder of thousands of files sorts in a moment).
    let mut keyed: Vec<(bool, String, String, usize)> = items
        .iter()
        .enumerate()
        .map(|(k, i)| (i.folder, i.name.to_lowercase(), if by == Arrange::Type { i.type_name.to_lowercase() } else { String::new() }, k))
        .collect();
    keyed.sort_by(|a, b| {
        let (ia, ib) = (&items[a.3], &items[b.3]);
        b.0.cmp(&a.0).then_with(|| match by {
            Arrange::Name => a.1.cmp(&b.1),
            Arrange::Type => a.2.cmp(&b.2).then_with(|| a.1.cmp(&b.1)),
            Arrange::Size => ia.size.cmp(&ib.size).then_with(|| a.1.cmp(&b.1)),
            Arrange::Date => ib.modified.cmp(&ia.modified).then_with(|| a.1.cmp(&b.1)),
        })
    });
    let order: Vec<usize> = keyed.into_iter().map(|k| k.3).collect();
    let mut taken: Vec<Option<Item>> = items.iter_mut().map(|i| Some(std::mem::replace(i, Item::system(System::MyComputer)))).collect();
    for (slot, k) in items.iter_mut().zip(order) {
        *slot = taken[k].take().expect("each item once");
    }
}

/// A key the desktop reacts to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    Enter,
    /// Ctrl+A.
    SelectAll,
    /// Esc: a drag or a selection rectangle stops (the icons go back).
    Escape,
    /// A typed character: the next icon whose name begins with it.
    Char(char),
    /// Page Up and Page Down (a folder window's Small Icons, List and Details move the selection a
    /// page; elsewhere they do nothing here).
    PageUp,
    PageDown,
}

/// Which mouse button went down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
}

/// The keys held with a click.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub ctrl: bool,
    pub shift: bool,
}

/// What the left button is doing.
#[derive(Clone, Debug)]
enum Gesture {
    /// Pressed on icon `hit` at `at`: becomes a drag once the pointer goes far enough. If the
    /// icon was selected already (with others), the selection becomes that one icon only at the
    /// release, so that several can be dragged.
    Press { at: (i32, i32), hit: usize, select_on_release: bool },
    /// Icons being dragged: the pointer went from `from` to `to`.
    Drag { from: (i32, i32), to: (i32, i32), items: Vec<usize> },
    /// The selection rectangle from `from` to `to`; `base` was selected before it (Ctrl).
    Band { from: (i32, i32), to: (i32, i32), base: Vec<bool> },
}

/// The desktop's icons and their state.
#[derive(Clone, Debug)]
pub struct Desktop {
    pub era: Era,
    /// The classic desktop or a folder window.
    host: Host,
    /// The look's own icons first, then the owner's in `arrange` order.
    pub items: Vec<Item>,
    /// Which icons are selected (one flag per item).
    selected: Vec<bool>,
    /// The icon with the keyboard focus (it stays when a click on the empty desktop takes the
    /// selection away, as in Windows).
    pub focus: Option<usize>,
    /// Where Shift+click's rectangle starts: the icon clicked last without Shift.
    anchor: Option<usize>,
    /// The desktop has the keyboard (it is the window in front).
    pub active: bool,
    pub recycle_full: bool,
    /// Where icons go, in the desktop's 1× pixels: the main screen without the bar.
    area: Rect,
    arrange: Arrange,
    auto_arrange: bool,
    /// The keyboard has been used. Windows 2000 draws focus frames only from then on (it hid
    /// them while only the mouse had been used: the Reference PC's -desk-full-sel).
    pub keys_used: bool,
    /// Where each icon lies (its cell's top-left, from the area's top-left), by id; `None` while
    /// the owner has moved nothing (the icons stand in columns in Arrange Icons' order).
    places: Option<HashMap<String, (i32, i32)>>,
    /// Each item's cell as shown (from the area's top-left).
    cells: Vec<(i32, i32)>,
    gesture: Option<Gesture>,
    /// New items made by the desktop's menu, waiting to show: their cell.
    pending: Vec<(String, (i32, i32))>,
    /// The places, the arrangement or Auto Arrange changed since `take_changed`.
    changed: bool,
    /// The items on the clipboard after Cut (by id): their pictures show faded, as Windows drew
    /// them, until they are pasted or something else goes onto the clipboard.
    cut: HashSet<String>,
    /// The hidden items shown (by id: Windows shows hidden items): their pictures show faded too,
    /// whatever happens to a cut (crate::hidden; PROVISIONAL).
    hidden: HashSet<String>,
    /// The label being renamed.
    rename: Option<Rename>,
    /// The items stay in the order given (`keep_order`).
    keep_order: bool,
    /// Where the view's own pixels' corner is on the screen (a folder window's place), for the
    /// focus rectangles' dots, which follow the screen's pixels. The desktop's is the screen's.
    pub dots_origin: (i32, i32),
    /// A folder window's view: large icons (the desktop's only one), small icons, a list or the
    /// details (`views`).
    mode: ViewMode,
    /// Small Icons' cell width and List's column width (`views::widths`), and Details' column
    /// widths, Name first.
    small_w: i32,
    list_w: i32,
    pub columns: Vec<i32>,
    /// The order Arrange Icons or a header click put the items in, reversed (a second click on
    /// the same header).
    reversed: bool,
    /// Each label's size measured once (width with its box, lines), by item, for a label shown
    /// collapsed and in full (thousands of items are laid out and painted without measuring
    /// every name again).
    label_sizes: std::cell::RefCell<Vec<LabelSizes>>,
    /// The part of a folder window's list that shows (the view's pixels; Page Up and Down, the
    /// pictures to read first).
    pub viewport: Rect,
    /// Details: the rows of an item hold more than its name (iews::Columns).
    pub details: views::Columns,
    /// My Computer's drives: free space and size, by id (Details' Total Size and Free Space).
    pub spaces: HashMap<String, (u64, u64)>,
    /// The item a drag of files is over now, drawn selected while the pointer is on it (the
    /// Reference PCs: a folder, the Recycle Bin, a drive, a program).
    pub drop_hover: Option<usize>,
    /// The press that may become a drag was made with the right button.
    right_press: bool,
    /// A Ctrl+press on an icon already selected: it is unselected at the release (no drag came).
    ctrl_off: bool,
    /// The classic desktop's wallpaper and pattern over its whole area (display::background), in
    /// its 1x pixels; None: the plain desktop color, drawn as it always was.
    background: Option<std::sync::Arc<Framebuffer>>,
    /// A folder window: the icons the person moved (dragged), by id, in the order they were
    /// moved, the last moved last (brief 103). Only their places are kept for the folder
    /// (`moved_places`); the others stand where the order puts them.
    moved: Vec<String>,
    /// A folder window: the icons' places in Large or Small Icons while List or Details shows, so
    /// that going there and back moves nothing (brief 103: no original did).
    stash: Option<(ViewMode, Places)>,
    /// A folder window: the icons that go last in the order (Windows 98 with its web view: a
    /// dragged icon counts as moved to the end), in that order (`pack_moved_last`).
    last: Vec<String>,
    /// The order the items came in (`set_files`): My Computer's by drive letter (`keep_order`).
    given: HashMap<String, usize>,
    /// A folder window with Auto Arrange on: where in the order each moved icon stands (the k-th
    /// cell), what is kept for it (brief 106, 2).
    ranks: HashMap<String, i32>,
    /// A drop was just made (`move_by`): Auto Arrange reads the order from where the icons lie.
    dropped: bool,
    /// The next layout is from a record (`load_moved`, `repack`): the icons not moved stand at
    /// their own slots (brief 105, 4); otherwise a new icon takes the first free cell.
    own_slots: bool,
    /// Places made in the other free view, waiting for the items and their measuring before they
    /// are stepped to this view (brief 106, 1: Small Icons' width is not known before).
    unstepped: Option<(ViewMode, MovedList)>,
}

/// Moved icons' places (x, y, id) in the order they were moved (`Desktop::moved_places`).
type MovedList = Vec<(i32, i32, String)>;

/// Icons' places by id (their cell's top-left, from the area's top-left).
type Places = HashMap<String, (i32, i32)>;

/// Where a place in a view of cells `old` lands in a view of cells `new`: on the same column and
/// row of the new grid (95's and 98's step between Large and Small Icons: `v1-small-open`).
fn view_step((x, y): (i32, i32), old: (i32, i32), new: (i32, i32)) -> (i32, i32) {
    (x.div_euclid(old.0.max(1)) * new.0, y.div_euclid(old.1.max(1)) * new.1)
}

/// What a folder window keeps of its moved icons (`Desktop::moved_places`): their places in Large
/// or Small Icons, or (Auto Arrange on) their places in the order (brief 106, 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Held {
    Places(ViewMode),
    Order,
}

/// A label's measured size (width with its box, lines), collapsed and in full.
type LabelSizes = [Option<(i32, i32)>; 2];

/// The four views of Windows' list view in a folder window (the desktop shows large icons only).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ViewMode {
    #[default]
    Large,
    Small,
    List,
    Details,
}

impl ViewMode {
    pub const ALL: [ViewMode; 4] = [ViewMode::Large, ViewMode::Small, ViewMode::List, ViewMode::Details];

    /// Its word in folders.txt.
    pub fn word(self) -> &'static str {
        match self {
            ViewMode::Large => "large",
            ViewMode::Small => "small",
            ViewMode::List => "list",
            ViewMode::Details => "details",
        }
    }

    pub fn from_word(word: &str) -> Option<ViewMode> {
        ViewMode::ALL.into_iter().find(|m| m.word() == word)
    }

    /// Small Icons, List and Details show 16×16 pictures beside one-line labels.
    pub fn small(self) -> bool {
        self != ViewMode::Large
    }
}

mod views;
pub use views::{Columns, DIVIDER_LEFT, DIVIDER_RIGHT, HEADER_H, HeaderPart, ROW_H, SMALL_PICTURE, date_text, header_hit, kb_text, local_filetime, paint_header};

/// A label being renamed: Windows' edit box over it (measured on the Reference PCs: a black
/// frame, white inside, the text centered, all of it selected at the start).
#[derive(Clone, Debug, PartialEq, Eq)]
struct Rename {
    id: String,
    /// The text being edited: Windows' edit box (`edit::Edit`, brief 95: keys, selection, words,
    /// Cut, Copy, Paste, Undo). `text`, `caret` and `anchor` are its copy for drawing the
    /// wrapped lines (`sync` after every change).
    edit: crate::edit::Edit,
    text: Vec<char>,
    /// Where the caret is, and the other end of the selection (the same: nothing selected).
    caret: usize,
    anchor: usize,
    /// The text of the last Cut or Copy, for the clipboard.
    copied: Option<String>,
    caret_on: bool,
    /// The extension warning is open (`rename_warning`): the box shows the name typed without
    /// selection or caret (its edit control lost the keyboard) and takes no keys.
    asking: bool,
    /// Opened again after No in the extension warning (the box's place changes: `rename_box`).
    declined: bool,
}

/// A key while a label is renamed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditKey {
    Char(char),
    Backspace,
    Delete,
    Left,
    Right,
    Home,
    End,
    /// Ctrl+A.
    SelectAll,
    /// Enter: the name is taken.
    Enter,
    /// Esc: the old name stays.
    Escape,
}

/// How renaming ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenameEnd {
    /// The item (its id) is to get this name (trimmed, not empty, not the old one).
    Rename { id: String, name: String },
    /// Nothing to do (Esc, the old name, an empty one).
    Unchanged,
}

impl Rename {
    fn new(id: String, name: &str) -> Rename {
        let mut edit = crate::edit::Edit::new(name).with_rule(Some(crate::edit::RENAME_LIMIT), crate::edit::Allow::Any);
        edit.select_all();
        let mut r = Rename { id, edit, text: Vec::new(), caret: 0, anchor: 0, copied: None, caret_on: true, asking: false, declined: false };
        r.sync();
        r
    }

    /// The drawing copy follows the edit box.
    fn sync(&mut self) {
        self.text = self.edit.text.chars().collect();
        self.caret = self.edit.caret;
        self.anchor = self.edit.anchor;
        self.caret_on = true;
    }
}

/// A number for an item's id (FNV-1a), for `update` to see another item at the same index.
fn id_code(id: &str) -> u64 {
    id.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
}

/// What `update` compares: an item's look and place (selected, focus, cell, cut) and which item
/// it is (`id_code`).
type Look = (bool, bool, (i32, i32), bool, u64);
/// Every item's `Look`, the selection rectangle, the dragged icons with the pointer's way, and
/// the label being renamed.
type Snapshot = (Vec<Look>, Option<Rect>, Option<((i32, i32), Vec<usize>)>, Option<Rename>);

impl Desktop {
    pub fn new(era: Era, area: Rect) -> Desktop {
        Desktop::hosted(Host::Desktop, era, area)
    }

    /// The icons inside a folder window, which shows them in `area` (the window's list, in the
    /// window's pixels, moved by how far it is scrolled). They stand in rows as wide as the list
    /// was when they were arranged (Windows 95's folder windows: icons stay where they are when
    /// the window is resized, unless Auto Arrange is on).
    pub fn in_window(era: Era, area: Rect) -> Desktop {
        Desktop::hosted(Host::Window, era, area)
    }

    fn hosted(host: Host, era: Era, area: Rect) -> Desktop {
        let mut d = Desktop {
            era,
            host,
            items: own_icons(host, era).iter().map(|&s| Item::system(s)).collect(),
            selected: Vec::new(),
            focus: None,
            anchor: None,
            active: false,
            recycle_full: false,
            area,
            arrange: Arrange::Name,
            auto_arrange: false,
            keys_used: false,
            places: None,
            cells: Vec::new(),
            gesture: None,
            pending: Vec::new(),
            changed: false,
            cut: HashSet::new(),
            hidden: HashSet::new(),
            rename: None,
            keep_order: false,
            dots_origin: (0, 0),
            mode: ViewMode::Large,
            small_w: views::SMALL_95,
            list_w: views::SMALL_95,
            columns: Vec::new(),
            reversed: false,
            label_sizes: std::cell::RefCell::new(Vec::new()),
            viewport: area,
            details: views::Columns::Folder,
            spaces: HashMap::new(),
            drop_hover: None,
            right_press: false,
            ctrl_off: false,
            background: None,
            moved: Vec::new(),
            stash: None,
            last: Vec::new(),
            given: HashMap::new(),
            ranks: HashMap::new(),
            dropped: false,
            own_slots: false,
            unstepped: None,
        };
        d.selected = vec![false; d.items.len()];
        d.relayout();
        d
    }

    /// What the desktop shows under its icons: the wallpaper and pattern drawn over its whole area
    /// (display::background), or None for the desktop color. The labels keep their box of the
    /// desktop color over it (the Reference PCs).
    pub fn set_background(&mut self, background: Option<std::sync::Arc<Framebuffer>>) {
        self.background = background;
    }

    /// The owner's items (unordered), as read from the desktop folders; the look's own icons are
    /// put in front. Selection and focus stay on the same items where they still exist; a file
    /// that went loses its place.
    pub fn set_files(&mut self, mut files: Vec<Item>) {
        // A new item the menu made, with the icons not moved yet: they keep the columns they
        // stand in now, and the new one goes where the menu was opened.
        if self.places.is_none() && self.pending.iter().any(|(id, _)| files.iter().any(|f| &f.id == id)) {
            self.freeze();
        }
        if let Some(places) = self.places.as_mut() {
            let present: HashSet<&str> = files.iter().map(|f| f.id.as_str()).collect();
            let before = places.len();
            places.retain(|id, _| id.starts_with("::") || present.contains(id.as_str()));
            for (id, cell) in &self.pending {
                if present.contains(id.as_str()) {
                    places.insert(id.clone(), *cell);
                }
            }
            self.changed |= places.len() != before;
        }
        let pending_before = self.pending.len();
        self.pending.retain(|(id, _)| !files.iter().any(|f| &f.id == id));
        self.changed |= pending_before != self.pending.len();
        // The order they came in (My Computer's: by drive letter), and the moved icons still there.
        self.given = files.iter().enumerate().map(|(k, f)| (f.id.clone(), k)).collect();
        if self.host == Host::Window {
            let present: HashSet<&str> = files.iter().map(|f| f.id.as_str()).collect();
            self.moved.retain(|id| present.contains(id.as_str()));
        }
        if !self.keep_order {
            self.sort_files(&mut files, self.arrange);
        }
        self.put_last(&mut files);
        let mut items: Vec<Item> = own_icons(self.host, self.era).iter().map(|&s| Item::system(s)).collect();
        items.extend(files);
        self.replace_items(items);
        // A window's icons stay where they were first put (until Arrange Icons or Auto Arrange).
        if self.host == Host::Window && self.places.is_none() && !self.items.is_empty() {
            self.freeze();
            self.changed = false;
        }
    }

    /// Keeps the items in the order `set_files` gets them (My Computer: its drives by letter, then
    /// Control Panel and Printers), instead of Arrange Icons' order.
    pub fn keep_order(&mut self, keep: bool) {
        self.keep_order = keep;
    }

    /// Puts in `items`, keeping selection, focus and anchor on their items.
    fn replace_items(&mut self, items: Vec<Item>) {
        let id = |d: &Desktop, i: Option<usize>| i.and_then(|i| d.items.get(i)).map(|i| i.id.clone());
        let (focus, anchor) = (id(self, self.focus), id(self, self.anchor));
        let selected: HashSet<String> = self.selection().into_iter().map(|i| self.items[i].id.clone()).collect();
        self.gesture = None;
        self.items = items;
        self.selected = self.items.iter().map(|i| selected.contains(&i.id)).collect();
        self.focus = focus.and_then(|f| self.index_of(&f));
        self.anchor = anchor.and_then(|a| self.index_of(&a));
        // A label being renamed whose item went (deleted meanwhile) is given up.
        if self.rename.as_ref().is_some_and(|r| !self.items.iter().any(|i| i.id == r.id)) {
            self.rename = None;
        }
        self.label_sizes.borrow_mut().clear();
        self.relayout();
    }

    /// The items cut onto the clipboard now (by id; empty: none). Their pictures show faded.
    pub fn set_cut(&mut self, ids: impl IntoIterator<Item = String>) {
        self.cut = ids.into_iter().collect();
    }

    pub fn is_cut(&self, i: usize) -> bool {
        self.items.get(i).is_some_and(|item| self.cut.contains(&item.id))
    }

    /// The hidden items among those shown (by id), as the folder was read: their pictures show
    /// faded, as a cut item's (the old Windows drew hidden files so; PROVISIONAL, not measured).
    pub fn set_hidden(&mut self, ids: impl IntoIterator<Item = String>) {
        self.hidden = ids.into_iter().collect();
    }

    /// Whether icon `i`'s picture shows faded: cut, or hidden (the two do not disturb each other).
    pub fn is_faded(&self, i: usize) -> bool {
        self.is_cut(i) || self.items.get(i).is_some_and(|item| self.hidden.contains(&item.id))
    }

    /// An item was renamed (Windows' file machinery said so): it keeps its place, its selection
    /// and its focus under its new id until the folder is read again; its label shows `name`
    /// at once. Its place in Arrange Icons' order follows with the next reading.
    pub fn renamed(&mut self, old_id: &str, new_id: &str, name: &str) {
        let Some(i) = self.index_of(old_id) else { return };
        if let Some(places) = self.places.as_mut()
            && let Some(cell) = places.remove(old_id)
        {
            places.insert(new_id.to_string(), cell);
            self.changed = true;
        }
        if self.cut.remove(old_id) {
            self.cut.insert(new_id.to_string());
        }
        for id in self.moved.iter_mut().chain(self.last.iter_mut()) {
            if id == old_id {
                *id = new_id.to_string();
            }
        }
        if let Some((_, stash)) = self.stash.as_mut()
            && let Some(c) = stash.remove(old_id)
        {
            stash.insert(new_id.to_string(), c);
        }
        if let Some(k) = self.ranks.remove(old_id) {
            self.ranks.insert(new_id.to_string(), k);
        }
        if self.hidden.remove(old_id) {
            self.hidden.insert(new_id.to_string());
        }
        self.items[i].id = new_id.to_string();
        self.items[i].name = name.to_string();
        self.label_sizes.borrow_mut().clear();
    }

    /// Starts renaming icon `i` (one of the owner's items; not the look's own): its whole name
    /// selected. False if it cannot be renamed.
    pub fn begin_rename(&mut self, i: usize) -> bool {
        let Some(item) = self.items.get(i).filter(|item| item.system_kind().is_none()) else { return false };
        self.gesture = None;
        self.rename = Some(Rename::new(item.id.clone(), &item.name));
        true
    }

    /// What Enter would do now, without ending the renaming (the platform decides first whether
    /// the extension warning comes).
    pub fn rename_result(&self) -> RenameEnd {
        let Some(r) = self.rename.as_ref() else { return RenameEnd::Unchanged };
        let name: String = r.text.iter().collect::<String>().trim().to_string();
        let old = self.index_of(&r.id).map(|i| self.items[i].name.as_str());
        if name.is_empty() || old == Some(name.as_str()) {
            RenameEnd::Unchanged
        } else {
            RenameEnd::Rename { id: r.id.clone(), name }
        }
    }

    /// The extension warning opened over the rename box (or closed: `false`).
    pub fn rename_asking(&mut self, asking: bool) {
        if let Some(r) = self.rename.as_mut() {
            r.asking = asking;
        }
    }

    /// Whether the extension warning is open over the rename box.
    /// A rename box is open (typing, or its extension question up).
    pub fn rename_open(&self) -> bool {
        self.rename.is_some()
    }

    pub fn is_asking(&self) -> bool {
        self.rename.as_ref().is_some_and(|r| r.asking)
    }

    /// No in the extension warning: the box stays open, all of it selected, with the name typed
    /// (`keep_typed`: Windows 95 and 98) or the old name again (Windows 2000).
    pub fn rename_declined(&mut self, keep_typed: bool) {
        let old = self.renaming().map(|i| self.items[i].name.clone());
        let Some(r) = self.rename.as_mut() else { return };
        if let Some(old) = old.filter(|_| !keep_typed) {
            r.edit.set(&old);
        }
        r.asking = false;
        r.declined = true;
        r.edit.select_all();
        r.sync();
    }

    /// The icon whose label is being renamed.
    pub fn renaming(&self) -> Option<usize> {
        self.rename.as_ref().and_then(|r| self.index_of(&r.id))
    }

    /// The name typed so far.
    pub fn rename_text(&self) -> Option<String> {
        self.rename.as_ref().map(|r| r.text.iter().collect())
    }

    /// The caret's blink: shown or not.
    pub fn blink(&mut self, on: bool) {
        if let Some(r) = self.rename.as_mut() {
            r.caret_on = on;
        }
    }

    /// A key while renaming; `Some` when renaming ends (Enter, Esc).
    pub fn rename_key(&mut self, key: EditKey, shift: bool) -> Option<RenameEnd> {
        // While the extension warning is open the box takes no keys (it does not have them).
        if self.is_asking() {
            return None;
        }
        match key {
            EditKey::Enter => return self.rename.is_some().then(|| self.end_rename()),
            EditKey::Escape => return self.rename.take().map(|_| RenameEnd::Unchanged),
            _ => {}
        }
        use crate::edit::Op;
        let r = self.rename.as_mut()?;
        let op = match key {
            EditKey::Enter | EditKey::Escape => return None,
            EditKey::Char(c) => {
                r.edit.type_char(c);
                r.sync();
                return None;
            }
            EditKey::Backspace => Op::Backspace,
            EditKey::Delete => Op::Delete,
            EditKey::Left => Op::Left,
            EditKey::Right => Op::Right,
            EditKey::Home => Op::Home,
            EditKey::End => Op::End,
            EditKey::SelectAll => Op::SelectAll,
        };
        r.edit.apply(op, shift);
        r.sync();
        None
    }

    /// A key of Windows' edit box while renaming (`kit::field_key`: the caret keys, words, Cut,
    /// Copy, Paste, Undo, Ctrl+A, Shift+F10; brief 95). `None`: not the box's key (Enter and Esc
    /// go through `rename_key`). Nothing while the extension warning is open.
    pub fn rename_edit_key<P>(&mut self, key: crate::kit::Key, mods: crate::kit::Mods) -> Option<Vec<crate::kit::Effect<P>>> {
        if self.is_asking() {
            return Some(Vec::new());
        }
        let r = self.rename.as_mut()?;
        let done = crate::kit::field_key(&mut r.edit, key, mods, &mut r.copied)?;
        r.sync();
        Some(done.effects)
    }

    /// The text of the last Cut or Copy in the rename box, for the clipboard.
    pub fn rename_take_copy(&mut self) -> Option<String> {
        self.rename.as_mut().and_then(|r| r.copied.take())
    }

    /// The rename box's right-click menu (`edit::Edit::menu`), if a label is being renamed.
    pub fn rename_menu(&self, clipboard_has_text: bool) -> Option<crate::paint::Menu> {
        self.rename.as_ref().filter(|r| !r.asking).map(|r| r.edit.menu(clipboard_has_text))
    }

    /// Entry `index` of the rename box's menu chosen.
    pub fn rename_menu_chosen<P>(&mut self, index: usize) -> Vec<crate::kit::Effect<P>> {
        let Some(r) = self.rename.as_mut() else { return Vec::new() };
        let done = crate::kit::field_menu_chosen(&mut r.edit, index, &mut r.copied);
        r.sync();
        done.effects
    }

    /// Where the rename box's menu opens from the keyboard: the box's middle (desktop pixels at
    /// 1×), as Windows' edit box opened it (`edit::middle`).
    pub fn rename_caret_point(&self, font: &Font) -> Option<(i32, i32)> {
        crate::edit::middle(self.rename_box(font)?.0)
    }

    /// Text from the clipboard in place of the selection (its first line only: a name has one).
    /// The characters a name cannot hold are not refused here, as they are not when typed: the
    /// name is checked when it is taken (`rename_warning::bad_name`).
    pub fn rename_paste(&mut self, text: &str) {
        if self.is_asking() {
            return;
        }
        if let Some(r) = self.rename.as_mut() {
            r.edit.paste(text);
            r.sync();
        }
    }

    /// The mouse dragged to (x, y) while renaming: the selection runs from where it was pressed.
    pub fn rename_drag(&mut self, font: &Font, x: i32, y: i32) -> bool {
        // No mouse while the extension warning is up (brief 97).
        if self.is_asking() {
            return false;
        }
        let Some(at) = self.rename_index_at(font, x, y) else { return false };
        let Some(r) = self.rename.as_mut() else { return false };
        let before = (r.caret, r.anchor);
        r.edit.drag(at);
        r.sync();
        before != (r.caret, r.anchor)
    }

    /// A double click in the rename box: the word there selected.
    pub fn rename_double_click(&mut self, font: &Font, x: i32, y: i32) -> bool {
        // Only on the box, and not while the extension warning is up (brief 97).
        if self.is_asking() || !self.rename_box(font).is_some_and(|(r, _)| r.contains(x, y)) {
            return false;
        }
        let Some(at) = self.rename_index_at(font, x, y) else { return false };
        let Some(r) = self.rename.as_mut() else { return false };
        r.edit.double_click(at);
        r.sync();
        true
    }

    /// Renaming ends with the name typed (Enter, or a click elsewhere, as in Windows).
    pub fn end_rename(&mut self) -> RenameEnd {
        let end = self.rename_result();
        self.rename = None;
        end
    }

    /// The renamed label's lines: which characters each holds (wrapped at spaces to fit the
    /// label's width, a word too long broken between letters, as the edit box did).
    fn rename_lines(font: &Font, text: &[char]) -> Vec<(usize, usize)> {
        let width = |a: usize, b: usize| font.measure(&text[a..b].iter().collect::<String>());
        let mut lines = Vec::new();
        let mut start = 0;
        while start < text.len() {
            // As much as fits; then back to the last space within it, if any.
            let mut end = start + 1;
            while end < text.len() && width(start, end + 1) <= WRAP_W {
                end += 1;
            }
            if end < text.len()
                && let Some(space) = (start + 1..=end).rev().find(|&k| text[k] == ' ')
            {
                end = space + 1;
            }
            lines.push((start, end));
            start = end;
        }
        if lines.is_empty() {
            lines.push((0, 0));
        }
        lines
    }

    /// The edit box's inner margin left and right: the edit control's, which follows the font
    /// (4 pixels with MS Sans Serif, 5 with Tahoma: the Reference PCs).
    fn edit_pad(&self) -> i32 {
        if self.era == Era::Win2000 { 5 } else { 4 }
    }

    /// The renamed label's edit box (desktop pixels at 1×) and where each line's text starts.
    pub fn rename_box(&self, font: &Font) -> Option<(Rect, Vec<(i32, i32)>)> {
        let r = self.rename.as_ref()?;
        let i = self.index_of(&r.id)?;
        if self.mode.small() {
            return self.small_rename_box(font, i, &r.text);
        }
        let lines = self.edit_lines(font, &r.text);
        let line_w = |&(a, b): &(usize, usize)| font.measure(r.text[a..b].iter().collect::<String>().trim_end());
        let text_w = lines.iter().map(line_w).max().unwrap_or(0);
        let pad = self.edit_pad();
        let cell = self.cell(i);
        let label_x = cell.x + (CELL - (text_w + 4)).div_euclid(2);
        let label_y = cell.y + LABEL_Y;
        let n = lines.len() as i32;
        // One line: 1 pixel of white above and below. Several: Windows 95 and 98 gave the box 3.
        // A name that had several lines typed down to one: 95 and 98 kept the 3 pixels, the box
        // 3 pixels lower than a several-line one (renext-changed / -after-no: "New Text
        // Document.txt" typed over with "a.dat").
        let started_tall = Desktop::rename_lines(font, &self.items[i].name.chars().collect::<Vec<_>>()).len() > 1;
        // Opened again after No in the extension warning, that one line went back to 1 pixel of
        // white, the box 3 pixels lower than a fresh one-line box (renext-after-no).
        let (top, text_top, h) = match (n, self.era) {
            (1, Era::Win95 | Era::Win98) if started_tall && r.declined => (label_y - 1, label_y + 1, LINE_H + 4),
            (1, Era::Win95 | Era::Win98) if started_tall => (label_y - 3, label_y - 1, LINE_H + 8),
            (1, _) | (_, Era::Win2000) => (label_y - 4, label_y - 2, n * LINE_H + 4),
            _ => (label_y - 6, label_y - 3, n * LINE_H + 8),
        };
        let b = Rect::new(label_x - pad + 2, top, text_w + 2 * pad + 2, h);
        let starts = lines.iter().enumerate().map(|(k, l)| (b.x + 1 + pad + (text_w - line_w(l)) / 2, text_top + k as i32 * LINE_H)).collect();
        Some((b, starts))
    }

    /// A click at (x, y) while renaming: in the box it moves the caret there and `true`;
    /// elsewhere `false` (the platform then ends renaming, as a click elsewhere did).
    pub fn rename_click(&mut self, font: &Font, x: i32, y: i32) -> bool {
        self.rename_press(font, x, y, false)
    }

    /// A press at (x, y) while renaming: in the box it moves the caret there (`extend`, Shift: the
    /// selection grows to there) and `true`; elsewhere `false`.
    pub fn rename_press(&mut self, font: &Font, x: i32, y: i32, extend: bool) -> bool {
        let Some((b, _)) = self.rename_box(font) else { return false };
        if !b.contains(x, y) {
            return false;
        }
        let Some(at) = self.rename_index_at(font, x, y) else { return false };
        let Some(r) = self.rename.as_mut() else { return false };
        r.edit.press(at, extend);
        r.sync();
        true
    }

    /// The character of the rename box nearest (x, y) (the wrapped lines as drawn).
    fn rename_index_at(&self, font: &Font, x: i32, y: i32) -> Option<usize> {
        let (_, starts) = self.rename_box(font)?;
        let r = self.rename.as_ref()?;
        let lines = self.edit_lines(font, &r.text);
        let k = (starts.iter().rposition(|&(_, ty)| y >= ty)).unwrap_or(0).min(lines.len() - 1);
        let (a, end) = lines[k];
        let lx = starts[k].0;
        let mut best = a;
        for c in a..=end.min(r.text.len()) {
            let w = font.measure(&r.text[a..c].iter().collect::<String>());
            if lx + w <= x + 2 {
                best = c;
            }
        }
        // A line's trailing space (where it wrapped) is not a place for the caret.
        if k + 1 < lines.len() && best == end {
            best = end - 1;
        }
        Some(best)
    }

    /// Changes the look: its own icons and their order (the owner's items stay).
    pub fn set_era(&mut self, era: Era) {
        if era != self.era {
            let files = self.files();
            self.era = era;
            let mut items: Vec<Item> = own_icons(self.host, era).iter().map(|&s| Item::system(s)).collect();
            items.extend(files);
            self.replace_items(items);
        }
    }

    /// The owner's items, without the look's own icons.
    pub fn files(&self) -> Vec<Item> {
        self.items.iter().filter(|i| i.system_kind().is_none()).cloned().collect()
    }

    pub fn area(&self) -> Rect {
        self.area
    }

    /// The main screen's free area changed (another screen, size or bar).
    pub fn set_area(&mut self, area: Rect) {
        if area != self.area {
            self.area = area;
            self.gesture = None;
            self.relayout();
        }
    }

    pub fn arrangement(&self) -> Arrange {
        self.arrange
    }

    /// Arrange Icons: the owner's items in that order, and every icon into columns from the top
    /// left again.
    pub fn set_arrange(&mut self, by: Arrange) {
        self.arrange = by;
        self.changed = true;
        let mut files = self.files();
        if !self.keep_order {
            self.sort_files(&mut files, by);
        }
        self.put_last(&mut files);
        let mut items: Vec<Item> = own_icons(self.host, self.era).iter().map(|&s| Item::system(s)).collect();
        items.extend(files);
        if let Some(places) = self.places.as_mut() {
            places.clear();
        }
        // Packed again: no icon lies where it was dragged any more.
        self.moved.clear();
        self.ranks.clear();
        self.unstepped = None;
        self.stash = None;
        self.replace_items(items);
        if self.places.is_some() && self.mode.arranged_freely() {
            // Columns in the new order (relayout gave every icon the first free cell in turn).
            let packed = self.column_cells();
            self.cells = packed;
            self.store_cells();
        }
    }

    pub fn auto_arrange(&self) -> bool {
        self.auto_arrange
    }

    /// Auto Arrange on or off. On, the icons are packed into columns at once, in the order they
    /// lie.
    pub fn set_auto_arrange(&mut self, on: bool) {
        if on != self.auto_arrange {
            self.auto_arrange = on;
            self.changed = true;
            if on && self.host == Host::Window && self.mode.arranged_freely() {
                // A folder window (the Reference PCs, brief 103): packed in the present order, a
                // dragged icon counting as moved to the end (`*-arrange-mc-auto-arrange-on`).
                let moved = std::mem::take(&mut self.moved);
                self.pack_moved_last(&moved);
                return;
            }
            self.relayout();
        }
    }

    /// A folder window: every icon packed in order again, `ids` (those still there) last, in that
    /// order, and they stay last when the folder is read again (`last`) until Arrange Icons by
    /// ... (Windows 98 with its web view, brief 103: a dragged icon counts as moved to the end).
    pub fn pack_moved_last(&mut self, ids: &[String]) {
        for id in ids {
            self.last.retain(|l| l != id);
            self.last.push(id.clone());
        }
        self.moved.clear();
        self.ranks.clear();
        self.unstepped = None;
        self.stash = None;
        let mut files = self.files();
        self.put_last(&mut files);
        let mut items: Vec<Item> = own_icons(self.host, self.era).iter().map(|&s| Item::system(s)).collect();
        items.extend(files);
        if let Some(places) = self.places.as_mut() {
            places.clear();
        }
        self.replace_items(items);
        if self.places.is_some() && self.mode.arranged_freely() {
            self.cells = self.column_cells();
            self.store_cells();
        }
        self.changed = true;
    }

    /// A folder window's list changed its width (a scroll bar came, the tree was shown): the
    /// icons not moved by the person are put in rows for the new width in their order, those moved
    /// stay where they were left (brief 103: a reopened window kept them). The desktop: as
    /// `set_arrange` with the arrangement in use.
    pub fn repack(&mut self) {
        if self.host != Host::Window || self.places.is_none() && self.stash.is_none() {
            let by = self.arrange;
            self.set_arrange(by);
            return;
        }
        // List or Details: their own rows; the places kept for the way back stay as they are
        // (brief 105, 2).
        if !self.mode.arranged_freely() {
            self.relayout();
            return;
        }
        // Auto Arrange: packed for the new width in the order they stand (brief 105, 3).
        if self.auto_arrange {
            self.relayout();
            return;
        }
        let keep: Places = self.moved_places().into_iter().map(|(x, y, id)| (id, (x, y))).collect();
        self.places = Some(keep);
        self.own_slots = true;
        self.relayout();
        self.changed = true;
    }

    /// A folder window whose look changed: every arrangement of the old look goes at once (the
    /// moved icons, their places and order, the order with moved icons last, the places kept for
    /// List and Details), every icon back in order; the new look's own record is laid out after
    /// (brief 106, 3: each look stays pure).
    pub fn forget_moved(&mut self) {
        if self.host != Host::Window {
            return;
        }
        self.last.clear();
        let by = self.arrange;
        self.set_arrange(by);
    }

    /// The icons that go last in the order (`pack_moved_last`), in that order.
    pub fn last_order(&self) -> &[String] {
        &self.last
    }

    /// Every icon back in the order of Arrange Icons as in use, none last and none where it was
    /// dragged (Windows 2000's F5 with its web view; brief 105, 9).
    pub fn back_in_order(&mut self) {
        self.last.clear();
        let by = self.arrange;
        self.set_arrange(by);
    }

    /// Puts the items named by `last` at the end of `files`, in `last`'s order.
    fn put_last(&self, files: &mut Vec<Item>) {
        if self.last.is_empty() {
            return;
        }
        let rank: HashMap<&str, usize> = self.last.iter().enumerate().map(|(k, id)| (id.as_str(), k)).collect();
        let (mut end, rest): (Vec<Item>, Vec<Item>) = std::mem::take(files).into_iter().partition(|f| rank.contains_key(f.id.as_str()));
        end.sort_by_key(|f| rank[f.id.as_str()]);
        *files = rest;
        files.extend(end);
    }

    /// Line up Icons: every icon onto the nearest cell of the grid. One whose nearest cell is
    /// taken goes to the next free cell to its right (the Reference PC put it there, although a
    /// free cell below was nearer), row after row.
    pub fn line_up(&mut self) {
        if self.places.is_none() {
            return;
        }
        if !self.mode.arranged_freely() {
            return;
        }
        let (cols, rows) = self.grid();
        let (cw, ch) = self.cell_size();
        // A window's list scrolls down: its grid reaches below the lowest icon (brief 103).
        let rows = match self.host {
            Host::Window => rows.max(self.cells.iter().map(|c| c.1.div_euclid(ch)).max().unwrap_or(0) + 2),
            Host::Desktop => rows,
        };
        let mut taken: HashSet<(i32, i32)> = HashSet::new();
        let on_grid = |c: (i32, i32)| c.0 % cw == 0 && c.1 % ch == 0;
        // Icons already on the grid stay.
        for &c in &self.cells {
            if on_grid(c) {
                taken.insert((c.0 / cw, c.1 / ch));
            }
        }
        for i in 0..self.items.len() {
            let c = self.cells[i];
            if on_grid(c) {
                continue;
            }
            let near = (((c.0 + cw / 2).div_euclid(cw)).clamp(0, cols - 1), ((c.1 + ch / 2).div_euclid(ch)).clamp(0, rows - 1));
            let start = near.1 * cols + near.0;
            let total = cols * rows;
            let slot = (0..total).map(|k| (start + k) % total).map(|s| (s % cols, s / cols)).find(|s| !taken.contains(s)).unwrap_or(near);
            taken.insert(slot);
            self.cells[i] = (slot.0 * cw, slot.1 * ch);
        }
        self.store_cells();
    }

    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.items.iter().position(|i| i.id == id)
    }

    /// A grid cell's size: 75 × 75 for large icons, Small Icons' width × one row.
    fn cell_size(&self) -> (i32, i32) {
        self.cell_size_of(self.mode)
    }

    /// A grid cell's size in view `mode`.
    fn cell_size_of(&self, mode: ViewMode) -> (i32, i32) {
        match mode {
            ViewMode::Large => (CELL, CELL),
            _ => (self.small_w.max(1), ROW_H),
        }
    }

    /// Cells across and down the area. A window's list scrolls down, so it has a row for every
    /// icon (and two more).
    fn grid(&self) -> (i32, i32) {
        let (cw, ch) = self.cell_size();
        let cols = (self.area.w / cw).max(1);
        match self.host {
            Host::Desktop => (cols, (self.area.h / ch).max(1)),
            Host::Window => (cols, (self.items.len() as i32 / cols + 2).max(self.area.h / ch)),
        }
    }

    /// The `k`th cell in the order new icons fill them: on the desktop column by column from the
    /// top left, in a window row by row (Windows' list view, aligned to the left or the top).
    fn slot(&self, k: i32) -> (i32, i32) {
        let (cols, rows) = self.grid();
        let (cw, ch) = self.cell_size();
        match self.host {
            Host::Desktop => ((k / rows) * cw, (k % rows) * ch),
            Host::Window => ((k % cols) * cw, (k / cols) * ch),
        }
    }

    /// Every item in its slot, in item order (the columns of the start, or a window's rows).
    fn column_cells(&self) -> Vec<(i32, i32)> {
        (0..self.items.len() as i32).map(|i| self.slot(i)).collect()
    }

    /// Whether a cell lies wholly in the area (anywhere right of and below a window's corner: the
    /// window scrolls to it).
    fn fits(&self, c: (i32, i32)) -> bool {
        match self.host {
            Host::Desktop => c.0 >= 0 && c.1 >= 0 && c.0 <= (self.area.w - CELL).max(0) && c.1 <= (self.area.h - CELL).max(0),
            Host::Window => c.0 >= 0 && c.1 >= 0,
        }
    }

    fn clamp(&self, c: (i32, i32)) -> (i32, i32) {
        match self.host {
            Host::Desktop => (c.0.clamp(0, (self.area.w - CELL).max(0)), c.1.clamp(0, (self.area.h - CELL).max(0))),
            Host::Window => (c.0.max(0), c.1.max(0)),
        }
    }

    /// How far the icons reach to the right and down from the area's corner (a window's scroll
    /// bars cover this much): the furthest picture or label.
    pub fn extent(&self, font: &Font) -> (i32, i32) {
        if let Some(e) = self.list_extent() {
            return e;
        }
        (0..self.items.len()).fold((0, 0), |(w, h), i| {
            let b = self.bounds(font, i);
            (w.max(b.right() - self.area.x), h.max(b.bottom() - self.area.y))
        })
    }

    /// Works out each item's cell from the places.
    #[allow(clippy::needless_range_loop)]
    fn relayout(&mut self) {
        let n = self.items.len();
        self.selected.resize(n, false);
        if !self.mode.arranged_freely() {
            // List and Details: in item order, column after column or row after row.
            self.cells = (0..n).map(|i| self.list_cell(i)).collect();
            return;
        }
        let Some(places) = self.places.as_ref() else {
            self.cells = self.column_cells();
            return;
        };
        let (cw, ch) = self.cell_size();
        if self.auto_arrange {
            // In the order they lie: column by column (the column an icon's left edge is in), then
            // from the top; icons without a place last, in item order.
            // (A window: row by row, the row an icon's top edge is in, then from the left.)
            let mut order: Vec<usize> = (0..n).collect();
            if self.host == Host::Window {
                // A window keeps the ORDER (brief 106, 2): each icon the person moved stands at
                // its own place in the order (`ranks`, the k-th cell), the others fill the other
                // cells in item order (their order never changes with a drop). Only right after a
                // drop is the order read from where the icons lie (row by row, then from the
                // left; every icon has its packed place then), which gives the dropped icon its
                // new place in the order. So the order survives a reopen, another width, a scroll
                // bar.
                let (cols, _) = self.grid();
                let moved: HashSet<&str> = self.moved.iter().map(String::as_str).collect();
                let mut placed: Vec<(i32, i32, usize)> = (0..n)
                    .filter_map(|i| {
                        let id = self.items[i].id.as_str();
                        if self.dropped {
                            let &(x, y) = places.get(id)?;
                            return Some(((y.div_euclid(ch) * cols + x.div_euclid(cw).min(cols - 1)).max(0), x, i));
                        }
                        if !moved.contains(id) {
                            return None;
                        }
                        self.ranks.get(id).map(|&k| (k, 0, i))
                    })
                    .collect();
                placed.sort();
                let in_placed: HashSet<usize> = placed.iter().map(|p| p.2).collect();
                let mut placed = placed.into_iter().peekable();
                let mut rest = (0..n).filter(|i| !in_placed.contains(i));
                order.clear();
                for k in 0..n as i32 {
                    let next = match placed.peek() {
                        Some(&(rank, _, i)) if rank <= k => {
                            placed.next();
                            Some(i)
                        }
                        _ => rest.next(),
                    };
                    match next.or_else(|| placed.next().map(|p| p.2)) {
                        Some(i) => order.push(i),
                        None => break,
                    }
                }
                // The moved icons' places in the order, kept (`moved_places`).
                for (k, &i) in order.iter().enumerate() {
                    if moved.contains(self.items[i].id.as_str()) {
                        self.ranks.insert(self.items[i].id.clone(), k as i32);
                    }
                }
                self.dropped = false;
            } else {
                let key = |i: usize| match places.get(&self.items[i].id) {
                    Some(&(x, y)) => (0, x.div_euclid(cw), y, i),
                    None => (1, 0, 0, i),
                };
                order.sort_by_key(|&i| key(i));
            }
            let mut cells = vec![(0, 0); n];
            for (k, &i) in order.iter().enumerate() {
                cells[i] = self.slot(k as i32);
            }
            self.cells = cells;
            self.store_cells();
            return;
        }
        let mut cells: Vec<Option<(i32, i32)>> = vec![None; n];
        let mut placed: Vec<(i32, i32)> = Vec::new();
        for (i, item) in self.items.iter().enumerate() {
            if let Some(&c) = places.get(&item.id).filter(|&&c| self.fits(c)) {
                cells[i] = Some(c);
                placed.push(c);
            }
        }
        // The rest, in item order: the first free cell, column by column (a window: row by row).
        // A new item keeps it; one whose place lies outside a smaller screen only borrows it (its
        // place stays, for when the screen is large again).
        let (cols, rows) = self.grid();
        let mut new_places = Vec::new();
        // The grid cells an icon overlaps (a cell is free when no icon lies less than a cell away
        // from it each way), so that finding the first free one stays quick with thousands of
        // icons (a folder of 3000 files took half a minute with a search through every icon).
        let mut taken: HashSet<(i32, i32)> = HashSet::new();
        let block = |taken: &mut HashSet<(i32, i32)>, (px, py): (i32, i32)| {
            let (gx, gy) = (px.div_euclid(cw) * cw, py.div_euclid(ch) * ch);
            for x in [gx, gx + cw] {
                for y in [gy, gy + ch] {
                    if (px - x).abs() < cw && (py - y).abs() < ch {
                        taken.insert((x, y));
                    }
                }
            }
        };
        for &c in &placed {
            block(&mut taken, c);
        }
        // A place kept for an icon the look does not show (95 hides My Documents; its place
        // waits) is not free either: an item put there would lie under it in another look.
        let shown: HashSet<&str> = self.items.iter().map(|item| item.id.as_str()).collect();
        for (id, &c) in places {
            if !shown.contains(id.as_str()) && self.fits(c) {
                block(&mut taken, c);
            }
        }
        let mut next = 0;
        let window = self.host == Host::Window;
        // A window laid out from a record (`own_slots`: opened again, Back, Up, a new width): each
        // icon without a place stands at its own slot in the order, so the cell a moved icon left
        // stays empty and the others do not close up (the Reference PCs, brief 105, 4: "The other
        // icons do not move"); first every one whose slot is free, then each one whose slot is
        // covered takes the next free cell after it (one pointer moving forward: brief 106, 9).
        // Otherwise (a new file in a window, the desktop) the first free cell, as before (brief
        // 106, 6). A window's grid has no end below: an icon that finds no cell gets a further
        // row, never the corner on top of another.
        let mut rest: Vec<usize> = (0..n).filter(|&i| cells[i].is_none()).collect();
        let own_slots = window && self.own_slots;
        if own_slots {
            rest.retain(|&i| {
                let own = i as i32;
                if taken.contains(&self.slot(own)) {
                    return true;
                }
                let c = self.slot(own);
                cells[i] = Some(c);
                block(&mut taken, c);
                if !places.contains_key(&self.items[i].id) {
                    new_places.push((self.items[i].id.clone(), c));
                }
                false
            });
        }
        let end = if window { i32::MAX } else { cols * rows };
        for i in rest {
            if own_slots {
                next = next.max(i as i32);
            }
            while next < end && taken.contains(&self.slot(next)) {
                next += 1;
            }
            let free = (next < end).then(|| self.slot(next));
            let c = match free {
                Some(c) => c,
                None => self.clamp(places.get(&self.items[i].id).copied().unwrap_or((0, 0))),
            };
            cells[i] = Some(c);
            placed.push(c);
            block(&mut taken, c);
            if !places.contains_key(&self.items[i].id) {
                new_places.push((self.items[i].id.clone(), c));
            }
        }
        self.cells = cells.into_iter().map(|c| c.unwrap_or((0, 0))).collect();
        if n > 0 {
            // Laid out from the record once the items are there; a file that comes later takes
            // the first free cell.
            self.own_slots = false;
        }
        if !new_places.is_empty() {
            if let Some(places) = self.places.as_mut() {
                places.extend(new_places);
            }
            self.changed = true;
        }
    }

    /// Keeps the cells shown as the icons' places (after arranging, lining up, a drop).
    fn store_cells(&mut self) {
        let places = self.places.get_or_insert_with(HashMap::new);
        let mut changed = false;
        for (item, &c) in self.items.iter().zip(&self.cells) {
            if places.insert(item.id.clone(), c) != Some(c) {
                changed = true;
            }
        }
        self.changed |= changed;
    }

    /// From columns to places: every icon keeps the cell it stands in now.
    fn freeze(&mut self) {
        if self.places.is_none() {
            self.places = Some(HashMap::new());
            self.store_cells();
            self.changed = true;
        }
    }

    /// A new item made by the desktop's menu: when it shows, its picture's corner lies at (x, y)
    /// (desktop pixels: where the menu was opened).
    pub fn place_new(&mut self, id: &str, x: i32, y: i32) {
        let cell = self.clamp((x - self.area.x - ICON_X, y - self.area.y - ICON_Y));
        self.pending.retain(|(p, _)| p != id);
        self.pending.push((id.to_string(), cell));
    }

    /// Whether the places, the arrangement or Auto Arrange changed since the last call (they
    /// are then saved).
    pub fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }

    /// The places file: the arrangement, Auto Arrange, and each icon's place (once the owner has
    /// moved one).
    pub fn places_text(&self) -> String {
        let mut text = String::from(
            "# Bevel95's desktop: Arrange Icons' order, Auto Arrange, and where each icon lies\n\
             # (x y, from the top left of the screen without the bar, in the look's own pixels,\n\
             # then the item). Written by Bevel95; delete it to put the icons back into columns.\n",
        );
        text += &format!("arrange = {}\n", self.arrange.word());
        text += &format!("auto_arrange = {}\n", if self.auto_arrange { "yes" } else { "no" });
        if let Some(places) = &self.places {
            let mut lines: Vec<(&String, &(i32, i32))> = places.iter().collect();
            lines.sort_by_key(|&(id, &(x, y))| (x, y, id.as_str()));
            for (id, (x, y)) in lines {
                text += &format!("{x} {y} {id}\n");
            }
        }
        text
    }

    /// Takes in what `places_text` wrote (lines it does not know are skipped).
    pub fn load_places(&mut self, text: &str) {
        let mut places = HashMap::new();
        for line in text.lines() {
            let line = line.trim_end_matches('\r');
            if line.starts_with('#') || line.trim().is_empty() {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                match (key.trim(), value.trim()) {
                    ("arrange", v) => self.arrange = Arrange::from_word(v).unwrap_or(self.arrange),
                    ("auto_arrange", v) => self.auto_arrange = v == "yes",
                    _ => {}
                }
                if !line.starts_with(|c: char| c.is_ascii_digit() || c == '-') {
                    continue;
                }
            }
            let mut parts = line.splitn(3, ' ');
            let (Some(x), Some(y), Some(id)) = (parts.next(), parts.next(), parts.next()) else { continue };
            if let (Ok(x), Ok(y)) = (x.parse::<i32>(), y.parse::<i32>())
                && !id.is_empty()
            {
                places.insert(id.to_string(), (x, y));
            }
        }
        self.places = (!places.is_empty()).then_some(places);
        let mut files = self.files();
        self.sort_files(&mut files, self.arrange);
        let mut items: Vec<Item> = own_icons(self.host, self.era).iter().map(|&s| Item::system(s)).collect();
        items.extend(files);
        self.replace_items(items);
        self.changed = false;
    }

    /// The cell of icon `i`.
    pub fn cell(&self, i: usize) -> Rect {
        let (x, y) = self.cells.get(i).copied().unwrap_or((0, 0));
        let (w, h) = match self.mode {
            ViewMode::Large => (CELL, CELL),
            ViewMode::Small => (self.small_w, ROW_H),
            ViewMode::List => (self.list_w, ROW_H),
            ViewMode::Details => (self.columns.first().copied().unwrap_or(0), ROW_H),
        };
        Rect::new(self.area.x + x, self.area.y + y, w, h)
    }

    /// Where icon `i`'s picture goes.
    pub fn icon_rect(&self, i: usize) -> Rect {
        let c = self.cell(i);
        if self.mode.small() {
            return self.small_icon_rect(c);
        }
        Rect::new(c.x + ICON_X, c.y + ICON_Y, ICON, ICON)
    }

    /// Whether icon `i`'s label shows in full: the focus icon's while the desktop has the
    /// keyboard, and every label in Windows 95.
    fn expanded(&self, i: usize) -> bool {
        self.era == Era::Win95 || (self.active && self.focus == Some(i))
    }

    /// Icon `i`'s label: its lines and its box.
    pub fn label(&self, font: &Font, i: usize) -> (Vec<String>, Rect) {
        self.label_at(font, i, self.cell(i), self.expanded(i))
    }

    fn label_at(&self, font: &Font, i: usize, cell: Rect, expanded: bool) -> (Vec<String>, Rect) {
        if self.mode.small() {
            return self.small_label(font, i, cell);
        }
        let lines = label_lines(font, &self.items[i].name, self.era, expanded);
        let w = lines.iter().map(|l| font.measure(l)).max().unwrap_or(0) + 4;
        let h = lines.len() as i32 * LINE_H + 2;
        (lines, Rect::new(cell.x + (CELL - w).div_euclid(2), cell.y + LABEL_Y, w, h))
    }

    /// Icon `i`'s label box (without its lines), measured once.
    fn label_box(&self, font: &Font, i: usize, cell: Rect, expanded: bool) -> Rect {
        if self.mode.small() {
            return self.small_label_box(font, i, cell);
        }
        let k = usize::from(expanded);
        let cached = self.label_sizes.borrow().get(i).and_then(|s| s[k]);
        let (w, lines) = cached.unwrap_or_else(|| {
            let lines = label_lines(font, &self.items[i].name, self.era, expanded);
            let size = (lines.iter().map(|l| font.measure(l)).max().unwrap_or(0) + 4, lines.len() as i32);
            let mut sizes = self.label_sizes.borrow_mut();
            if sizes.len() < self.items.len() {
                sizes.resize(self.items.len(), [None, None]);
            }
            sizes[i][k] = Some(size);
            size
        });
        Rect::new(cell.x + (CELL - w).div_euclid(2), cell.y + LABEL_Y, w, lines * LINE_H + 2)
    }

    /// Everything icon `i` paints on.
    pub fn bounds(&self, font: &Font, i: usize) -> Rect {
        union(self.icon_rect(i), self.label_box(font, i, self.cell(i), self.expanded(i)))
    }

    fn bounds_at(&self, font: &Font, i: usize, cell: Rect, expanded: bool) -> Rect {
        let icon = if self.mode.small() { self.small_icon_rect(cell) } else { Rect::new(cell.x + ICON_X, cell.y + ICON_Y, ICON, ICON) };
        union(icon, self.label_box(font, i, cell, expanded))
    }

    /// Whether (x, y) is on icon `i`'s picture or label.
    fn on(&self, font: &Font, i: usize, x: i32, y: i32) -> bool {
        self.icon_rect(i).contains(x, y) || self.label_box(font, i, self.cell(i), self.expanded(i)).contains(x, y)
    }

    /// Whether icon `i`'s picture or label touches `r`.
    fn touches(&self, font: &Font, i: usize, r: Rect) -> bool {
        intersects(self.icon_rect(i), r) || intersects(self.label_box(font, i, self.cell(i), self.expanded(i)), r)
    }

    /// The icon under (x, y): its picture or its label.
    pub fn hit(&self, font: &Font, x: i32, y: i32) -> Option<usize> {
        if let Some(found) = self.list_hit(font, x, y) {
            return found;
        }
        // The focus icon's label can reach over the icons below it; it is drawn last, on top.
        self.paint_order().into_iter().rev().find(|&i| self.on(font, i, x, y))
    }

    fn paint_order(&self) -> Vec<usize> {
        let mut order: Vec<usize> = (0..self.items.len()).filter(|&i| Some(i) != self.focus).collect();
        order.extend(self.focus.filter(|&f| f < self.items.len()));
        order
    }

    pub fn is_selected(&self, i: usize) -> bool {
        self.selected.get(i).copied().unwrap_or(false)
    }

    /// The selected icons, in item order.
    pub fn selection(&self) -> Vec<usize> {
        (0..self.items.len()).filter(|&i| self.is_selected(i)).collect()
    }

    /// Every icon selected becomes unselected and every other one selected (Edit > Invert
    /// Selection).
    pub fn invert_selection(&mut self) {
        for s in &mut self.selected {
            *s = !*s;
        }
    }

    /// Selects only icon `i` (none: nothing), with the focus and the anchor on it.
    pub fn select_only(&mut self, i: Option<usize>) {
        self.selected = vec![false; self.items.len()];
        if let Some(i) = i.filter(|&i| i < self.items.len()) {
            self.selected[i] = true;
            self.focus = Some(i);
            self.anchor = Some(i);
        }
    }

    /// The icons in the order they lie: column by column, then from the top (Home is the first,
    /// End the last).
    fn visual_order(&self) -> Vec<usize> {
        let mut order: Vec<usize> = (0..self.items.len()).collect();
        if !self.mode.arranged_freely() {
            return order;
        }
        let (cw, ch) = self.cell_size();
        match self.host {
            Host::Desktop => order.sort_by_key(|&i| (self.cells[i].0.div_euclid(cw), self.cells[i].1, i)),
            // A window's: row by row.
            Host::Window => order.sort_by_key(|&i| (self.cells[i].1.div_euclid(ch), self.cells[i].0, i)),
        }
        order
    }

    /// A press of the left or right button at (x, y) (desktop pixels); returns the icon there.
    /// Left: on an icon, it is selected (Ctrl: added or taken away; Shift: every icon in the
    /// rectangle around the anchor and it), and a drag may follow; on the empty desktop the
    /// selection goes (not with Ctrl) and a selection rectangle may follow. Right: an icon not
    /// selected yet becomes the only one selected; the empty desktop takes the selection away.
    pub fn press(&mut self, font: &Font, x: i32, y: i32, button: Button, mods: Mods) -> Option<usize> {
        self.gesture = None;
        self.right_press = false;
        self.ctrl_off = false;
        let hit = self.hit(font, x, y);
        match (hit, button) {
            (Some(i), Button::Right) => {
                if !self.is_selected(i) {
                    self.select_only(Some(i));
                }
                self.focus = Some(i);
                // A right-button drag may follow (its menu at the drop).
                self.gesture = Some(Gesture::Press { at: (x, y), hit: i, select_on_release: false });
                self.right_press = true;
            }
            (None, Button::Right) => {
                if !mods.ctrl {
                    self.select_only(None);
                }
            }
            (Some(i), Button::Left) if mods.shift => {
                let anchor = self.anchor.filter(|&a| a < self.items.len()).unwrap_or(i);
                let r = union(self.bounds(font, anchor), self.bounds(font, i));
                if !mods.ctrl {
                    self.selected = vec![false; self.items.len()];
                }
                for k in 0..self.items.len() {
                    if intersects(self.bounds(font, k), r) {
                        self.selected[k] = true;
                    }
                }
                self.focus = Some(i);
                self.anchor = Some(anchor);
                // Shift held through a drag moves the files.
                self.gesture = Some(Gesture::Press { at: (x, y), hit: i, select_on_release: false });
            }
            (Some(i), Button::Left) if mods.ctrl => {
                // Ctrl adds an icon at once; one already selected goes at the release unless a
                // drag (a copy, with Ctrl held) takes them all along.
                let already = self.selected[i];
                self.selected[i] = true;
                self.focus = Some(i);
                self.anchor = Some(i);
                self.gesture = Some(Gesture::Press { at: (x, y), hit: i, select_on_release: false });
                self.ctrl_off = already;
            }
            (Some(i), Button::Left) => {
                let already = self.is_selected(i);
                if !already {
                    self.select_only(Some(i));
                }
                self.focus = Some(i);
                self.anchor = Some(i);
                self.gesture = Some(Gesture::Press { at: (x, y), hit: i, select_on_release: already });
            }
            (None, Button::Left) => {
                let base = if mods.ctrl { self.selected.clone() } else { vec![false; self.items.len()] };
                self.selected = base.clone();
                self.gesture = Some(Gesture::Band { from: (x, y), to: (x, y), base });
            }
        }
        hit
    }

    /// The pointer moved to (x, y) with the left button held.
    pub fn drag_to(&mut self, font: &Font, x: i32, y: i32) {
        match self.gesture.take() {
            Some(Gesture::Press { at, hit, select_on_release }) => {
                if (x - at.0).abs() > DRAG_START || (y - at.1).abs() > DRAG_START {
                    let items = self.selection();
                    let items = if items.is_empty() { vec![hit] } else { items };
                    self.gesture = Some(Gesture::Drag { from: at, to: (x, y), items });
                } else {
                    self.gesture = Some(Gesture::Press { at, hit, select_on_release });
                }
            }
            Some(Gesture::Drag { from, items, .. }) => self.gesture = Some(Gesture::Drag { from, to: (x, y), items }),
            Some(Gesture::Band { from, base, .. }) => {
                let r = band_rect(from, (x, y));
                self.selected = (0..self.items.len()).map(|i| base.get(i).copied().unwrap_or(false) || self.touches(font, i, r)).collect();
                self.gesture = Some(Gesture::Band { from, to: (x, y), base });
            }
            None => {}
        }
    }

    /// The left button went up at (x, y). A drag drops its icons there, each moved by the
    /// pointer's way, unless the pointer is on another icon or outside the area (Windows would
    /// have moved files: here they go back).
    pub fn release(&mut self, font: &Font, x: i32, y: i32) {
        self.drag_to(font, x, y);
        match self.gesture.take() {
            Some(Gesture::Press { hit, .. }) if std::mem::take(&mut self.ctrl_off) => {
                if let Some(s) = self.selected.get_mut(hit) {
                    *s = false;
                }
            }
            Some(Gesture::Press { hit, select_on_release: true, .. }) => self.select_only(Some(hit)),
            Some(Gesture::Drag { from, to, items }) => {
                let on_other = self.hit(font, x, y).is_some_and(|i| !items.contains(&i));
                if on_other || !self.area.contains(x, y) {
                    return;
                }
                self.freeze();
                let (dx, dy) = (to.0 - from.0, to.1 - from.1);
                for &i in &items {
                    let c = self.cells[i];
                    self.cells[i] = self.clamp((c.0 + dx, c.1 + dy));
                }
                self.store_cells();
                if self.auto_arrange {
                    self.relayout();
                }
            }
            _ => {}
        }
    }

    /// The drag or the selection rectangle stops where it is (Esc, or the mouse taken away): the
    /// icons go back; the rectangle's selection stays.
    pub fn cancel(&mut self) {
        self.gesture = None;
    }

    /// A folder window's tab goes behind or comes forward (brief 110): the press or drag under
    /// way, the right press, the Ctrl press and the drop place drawn go (`FolderWindow::
    /// forget_passing`).
    pub fn forget_passing(&mut self) {
        self.gesture = None;
        self.right_press = false;
        self.ctrl_off = false;
        self.drop_hover = None;
    }

    /// Something is going on with the left button held (the platform repaints on a timer).
    pub fn gesture_active(&self) -> bool {
        self.gesture.is_some()
    }

    /// Icons are being dragged.
    pub fn dragging(&self) -> bool {
        matches!(self.gesture, Some(Gesture::Drag { .. }))
    }

    /// A drag that has just begun goes to Windows' drag and drop: what it takes along (the
    /// icons, where the press was, whether the right button drags). The view itself shows no
    /// dragged picture from now on (the platform's window carries it).
    pub fn take_drag(&mut self) -> Option<(Vec<usize>, (i32, i32), bool)> {
        match self.gesture.take() {
            Some(Gesture::Drag { from, items, .. }) => Some((items, from, std::mem::take(&mut self.right_press))),
            other => {
                self.gesture = other;
                None
            }
        }
    }

    /// Whether the press under way is the right button's (its release opens the menu unless a
    /// drag began).
    pub fn right_pressed(&self) -> bool {
        self.right_press && matches!(self.gesture, Some(Gesture::Press { .. }))
    }

    /// The ids of icons `items` (what a drag or a menu remembers them by: indices move when the
    /// items are read again).
    pub fn ids_of(&self, items: &[usize]) -> Vec<String> {
        items.iter().filter_map(|&i| self.items.get(i)).map(|item| item.id.clone()).collect()
    }

    /// Selects exactly the icons with these ids (a menu's command acts on what the menu was opened
    /// for, not on what was selected meanwhile). False, and nothing changed, if any of them is gone
    /// (the third safety review of 2026-11-18, A2).
    pub fn select_ids(&mut self, ids: &[String]) -> bool {
        let Some(found) = ids.iter().map(|id| self.index_of(id)).collect::<Option<Vec<usize>>>() else { return false };
        self.selected = vec![false; self.items.len()];
        for i in found {
            self.selected[i] = true;
        }
        true
    }

    /// Icons dragged within their own view and dropped there, named by their ids (`ids_of` when
    /// the drag began): those still there are moved as `move_by` moves them. A refresh during the
    /// drag reorders the items, and the indices would then move other icons (the third safety
    /// review of 2026-11-18, suspected 3).
    pub fn move_ids(&mut self, ids: &[String], d: (i32, i32)) {
        let items: Vec<usize> = ids.iter().filter_map(|id| self.index_of(id)).collect();
        self.move_by(&items, d);
    }

    /// Icons dragged within their own view and dropped there: each moved by the pointer's way
    /// `d`, clamped into the area (in the views that place icons freely), as the Reference PCs
    /// did with a drop in the same window.
    pub fn move_by(&mut self, items: &[usize], d: (i32, i32)) {
        if !self.mode.arranged_freely() || d == (0, 0) {
            return;
        }
        self.freeze();
        for &i in items.iter().filter(|&&i| i < self.items.len()) {
            let c = self.cells[i];
            self.cells[i] = self.clamp((c.0 + d.0, c.1 + d.1));
        }
        self.note_moved(items);
        self.store_cells();
        if self.auto_arrange {
            self.dropped = true;
            self.relayout();
            self.dropped = false;
        }
    }

    /// A folder window: icons `items` were dragged by the person (the last moved last).
    fn note_moved(&mut self, items: &[usize]) {
        if self.host != Host::Window {
            return;
        }
        for &i in items {
            if let Some(item) = self.items.get(i) {
                let id = item.id.clone();
                self.moved.retain(|m| *m != id);
                self.moved.push(id);
            }
        }
    }

    /// A folder window: the icons the person moved, in the order they were moved.
    pub fn moved_ids(&self) -> &[String] {
        &self.moved
    }

    /// A folder window: the places of the icons the person moved (x, y in the view's own pixels
    /// of Large or Small Icons as now, then the id), in the order they were moved: what is kept
    /// for the folder (brief 103: only icons really moved, never every icon of a big folder).
    /// With Auto Arrange on, each one's place in the order instead (k, 0, id): `moved_held`
    /// says which (brief 106, 2).
    pub fn moved_places(&self) -> Vec<(i32, i32, String)> {
        if self.host == Host::Window && self.auto_arrange {
            return self.moved.iter().filter_map(|id| self.ranks.get(id).map(|&k| (k, 0, id.clone()))).collect();
        }
        if !self.mode.arranged_freely() {
            // In List or Details: the places kept for the way back.
            let Some((_, stash)) = &self.stash else { return Vec::new() };
            return self.moved.iter().filter_map(|id| stash.get(id).map(|&(x, y)| (x, y, id.clone()))).collect();
        }
        let Some(places) = self.places.as_ref() else { return Vec::new() };
        self.moved.iter().filter_map(|id| places.get(id).map(|&(x, y)| (x, y, id.clone()))).collect()
    }

    /// The view the moved icons' places are in (`moved_places`): the view shown when it is Large
    /// or Small Icons, else the one they wait for in List or Details (brief 105, 1).
    pub fn moved_view(&self) -> ViewMode {
        if self.mode.arranged_freely() {
            return self.mode;
        }
        self.stash.as_ref().map_or(ViewMode::Large, |(m, _)| *m)
    }

    /// What `moved_places` holds: places in a view, or (Auto Arrange on) places in the order.
    pub fn moved_held(&self) -> Held {
        if self.host == Host::Window && self.auto_arrange {
            Held::Order
        } else {
            Held::Places(self.moved_view())
        }
    }

    /// A folder window opened or gone back to: the icons the person moved there before lie
    /// where they were left (`moved_places`, made in view `made_in`), each other icon at its own
    /// slot in the order, the cells the moved ones left empty (brief 105, 4). Made in the other
    /// free view: moved as the view's own step moves an icon (95, 98: onto the new grid's cell);
    /// in 2000, whose own step puts every icon back in order, not used: false then (the record
    /// is to go). In List or Details they wait for the way back. The step waits until the items
    /// have come and been measured (brief 106, 1: Small Icons' width is the widest name's).
    /// `Held::Order` (Auto Arrange on when it was made): each moved icon's place in the order,
    /// used while Auto Arrange is on (brief 106, 2).
    pub fn load_moved(&mut self, list: &[(i32, i32, String)], held: Held) -> bool {
        if self.host != Host::Window || list.is_empty() {
            return true;
        }
        let made_in = match held {
            Held::Order => {
                if !self.auto_arrange {
                    return true;
                }
                self.moved.clear();
                self.ranks.clear();
                for (k, _, id) in list {
                    self.moved.retain(|m| m != id);
                    self.moved.push(id.clone());
                    self.ranks.insert(id.clone(), *k);
                }
                if self.mode.arranged_freely() {
                    self.places.get_or_insert_with(HashMap::new);
                    self.relayout();
                    self.changed = false;
                } else {
                    self.stash = Some((ViewMode::Large, HashMap::new()));
                }
                return true;
            }
            Held::Places(made_in) => made_in,
        };
        if !made_in.arranged_freely() {
            return true;
        }
        let target = if self.mode.arranged_freely() { self.mode } else { made_in };
        if target != made_in && self.era == Era::Win2000 {
            return false;
        }
        self.moved.clear();
        for (_, _, id) in list {
            self.moved.retain(|m| m != id);
            self.moved.push(id.clone());
        }
        if target != made_in && self.items.is_empty() {
            // Stepped once the items are measured (`measure`); meanwhile nothing lies there.
            self.unstepped = Some((made_in, list.to_vec()));
            self.places = Some(HashMap::new());
            self.own_slots = true;
            self.relayout();
            self.changed = false;
            return true;
        }
        self.place_moved(list, made_in, target);
        true
    }

    /// Puts the moved icons' places `list` (made in view `made_in`) into view `target`, stepped
    /// as the view's own step moves an icon when the two differ.
    fn place_moved(&mut self, list: &[(i32, i32, String)], made_in: ViewMode, target: ViewMode) {
        let (old, new) = (self.cell_size_of(made_in), self.cell_size_of(target));
        let places: Places = list
            .iter()
            .map(|(x, y, id)| (id.clone(), if target == made_in { (*x, *y) } else { view_step((*x, *y), old, new) }))
            .collect();
        if self.mode.arranged_freely() {
            self.places = Some(places);
            self.own_slots = true;
            self.relayout();
            self.changed = false;
        } else {
            self.stash = Some((target, places));
        }
    }

    /// The items have been measured (`measure`): places waiting for their step (`unstepped`)
    /// are put into the view now. True if there were some.
    fn step_unstepped(&mut self) -> bool {
        if self.items.is_empty() {
            return false;
        }
        let Some((made_in, list)) = self.unstepped.take() else { return false };
        if self.mode.arranged_freely() {
            let target = self.mode;
            self.place_moved(&list, made_in, target);
        } else {
            // List or Details meanwhile: kept as made, stepped on the way back.
            self.stash = Some((made_in, list.into_iter().map(|(x, y, id)| (id, (x, y))).collect()));
        }
        true
    }

    /// The icon a drop at (x, y) would land on (its picture or label), if it takes dropped
    /// files: a folder, a drive, the Recycle Bin, My Documents, My Computer (which refuses them,
    /// but shows selected as on the Reference PCs). `None`: the drop goes to the view's own
    /// folder. Icons being dragged themselves are never one.
    pub fn drop_target(&self, font: &Font, x: i32, y: i32, dragged: &[String]) -> Option<usize> {
        let i = self.hit(font, x, y)?;
        let item = &self.items[i];
        if dragged.iter().any(|d| d.eq_ignore_ascii_case(&item.id)) {
            return None;
        }
        match crate::desktop::ItemKind::of(item) {
            crate::desktop::ItemKind::Folder | crate::desktop::ItemKind::Drive | crate::desktop::ItemKind::System(_) => Some(i),
            _ => None,
        }
    }

    /// Whether icon `i` shows selected now: selected with the keyboard here, or the place a drag
    /// would drop on.
    fn shows_selected(&self, i: usize, active: bool) -> bool {
        (active && self.is_selected(i)) || self.drop_hover == Some(i)
    }

    /// A key; returns the icons to open (Enter: every selected one).
    pub fn key(&mut self, key: Key) -> Vec<usize> {
        self.keys_used = true;
        let n = self.items.len();
        if n == 0 {
            return Vec::new();
        }
        let from = self.focus.filter(|&f| f < n);
        if let Some(to) = self.view_key(key, from) {
            match to {
                Some(i) => self.select_only(Some(i)),
                None => self.select_only(from),
            }
            return Vec::new();
        }
        let to = match (key, from) {
            (Key::Enter, _) => return self.selection(),
            (Key::Escape, _) => {
                self.cancel();
                return Vec::new();
            }
            (Key::SelectAll, _) => {
                self.selected = vec![true; n];
                return Vec::new();
            }
            (Key::Char(c), _) => self.next_starting_with(c),
            (Key::PageUp | Key::PageDown, _) => return Vec::new(),
            (Key::Home, _) => self.visual_order().first().copied(),
            (Key::End, _) => self.visual_order().last().copied(),
            // The first arrow press with no focus yet lands on the first icon.
            (_, None) => self.visual_order().first().copied(),
            (Key::Up, Some(f)) => self.nearest(f, 0, -1),
            (Key::Down, Some(f)) => self.nearest(f, 0, 1),
            (Key::Left, Some(f)) => self.nearest(f, -1, 0),
            (Key::Right, Some(f)) => self.nearest(f, 1, 0),
        };
        match to {
            Some(i) => self.select_only(Some(i)),
            // At the edge the selection still comes back to the focus icon.
            None => self.select_only(from),
        }
        Vec::new()
    }

    /// The nearest icon from icon `f` in the direction (dx, dy): among those lying further that
    /// way, the one with the least distance, a step to the side counting twice (the Reference
    /// PC went up a column to a far icon rather than to a near one off to the side).
    fn nearest(&self, f: usize, dx: i32, dy: i32) -> Option<usize> {
        let (fx, fy) = self.cells[f];
        (0..self.items.len())
            .filter(|&i| i != f)
            .filter_map(|i| {
                let (x, y) = self.cells[i];
                let (along, side) = if dx != 0 { ((x - fx) * dx, y - fy) } else { ((y - fy) * dy, x - fx) };
                (along > 0).then(|| (i64::from(along).pow(2) + 4 * i64::from(side).pow(2), i))
            })
            .min()
            .map(|(_, i)| i)
    }

    /// The next icon after the focus whose name begins with `c` (any case), round to the start.
    fn next_starting_with(&self, c: char) -> Option<usize> {
        let n = self.items.len();
        let start = self.focus.map_or(0, |f| f + 1);
        let wanted: String = c.to_lowercase().collect();
        (0..n).map(|k| (start + k) % n).find(|&i| self.items[i].name.to_lowercase().starts_with(&wanted))
    }

    /// Each item's look and cell, the band and the dragged pictures, for `update`.
    fn snapshot(&self) -> Snapshot {
        // Which item stands at each index belongs to its look: Arrange Icons puts other items into
        // the same cells (the user's report of 2026-11-21: nothing was painted).
        let looks = (0..self.items.len())
            .map(|i| (self.shows_selected(i, self.active), self.active && self.focus == Some(i), self.cells[i], self.is_faded(i), id_code(&self.items[i].id)))
            .collect();
        let band = match &self.gesture {
            Some(Gesture::Band { from, to, .. }) => Some(band_rect(*from, *to)),
            _ => None,
        };
        let drag = match &self.gesture {
            Some(Gesture::Drag { from, to, items }) => Some(((to.0 - from.0, to.1 - from.1), items.clone())),
            _ => None,
        };
        (looks, band, drag, self.rename.clone())
    }

    /// Runs `change` and says what to paint again (desktop pixels at 1×): every icon whose look
    /// or place changed, before and after, the selection rectangle's edges and the dragged
    /// pictures, old and new.
    pub fn update<R>(&mut self, font: &Font, change: impl FnOnce(&mut Desktop) -> R) -> (R, Vec<Rect>) {
        let (looks, band, drag, rename) = self.snapshot();
        let rename_before = (self.rename_box(font).map(|b| b.0), self.renaming().map(|i| self.bounds(font, i)));
        let focus_before = self.focus;
        let old_bounds = |d: &Desktop, i: usize, l: &Look| {
            let cell = Rect::new(d.area.x + l.2.0, d.area.y + l.2.1, CELL, CELL);
            let expanded = d.era == Era::Win95 || l.1;
            d.bounds_at(font, i, cell, expanded)
        };
        // Bounds before the change (items may change in it).
        let before: Vec<Rect> = looks.iter().enumerate().map(|(i, l)| old_bounds(self, i, l)).collect();
        let drag_before = drag.as_ref().map(|(d, items)| self.drag_rects(font, *d, items)).unwrap_or_default();
        let result = change(self);
        let (looks_after, band_after, drag_after, rename_after) = self.snapshot();
        let mut dirty = Vec::new();
        if rename != rename_after {
            // The box, before and after, and the label it covers (shown again when renaming ends).
            let after = (self.rename_box(font).map(|b| b.0), self.renaming().map(|i| self.bounds(font, i)));
            dirty.extend([rename_before.0, rename_before.1, after.0, after.1].into_iter().flatten());
        }
        if looks_after.len() != looks.len() {
            dirty.extend(before.iter().copied());
            dirty.extend((0..self.items.len()).map(|i| self.bounds(font, i)));
        } else {
            for i in 0..looks.len() {
                // The focus moving also changes which label is drawn last, on top.
                let refocus = focus_before != self.focus && (focus_before == Some(i) || self.focus == Some(i));
                if looks[i] != looks_after[i] || refocus {
                    dirty.push(before[i]);
                    dirty.push(self.bounds(font, i));
                }
            }
        }
        if band != band_after {
            for r in [band, band_after].into_iter().flatten() {
                dirty.extend(frame_strips(r));
            }
        }
        if drag.as_ref().map(|d| d.0) != drag_after.as_ref().map(|d| d.0) || drag.is_some() != drag_after.is_some() {
            dirty.extend(drag_before);
            if let Some((d, items)) = &drag_after {
                dirty.extend(self.drag_rects(font, *d, items));
            }
        }
        // Whatever an icon's repainted bounds overlap must be painted with it.
        (result, merge(dirty))
    }

    /// Where the dragged pictures are, moved by `d`.
    fn drag_rects(&self, font: &Font, d: (i32, i32), items: &[usize]) -> Vec<Rect> {
        items.iter().filter(|&&i| i < self.items.len()).map(|&i| shift(self.bounds(font, i), d)).collect()
    }

    /// Icon `i`'s picture as it shows now.
    fn picture(&self, theme: &Theme, i: usize, selected: bool) -> Option<Icon> {
        let item = &self.items[i];
        let highlight = theme.colors.menu_highlight;
        let art = |a: Art| desktop_art::picture(self.era, a, selected, highlight);
        let mut icon = match &item.picture {
            Picture::System(System::MyComputer) => art(Art::MyComputer)?,
            Picture::System(System::MyDocuments) => art(Art::MyDocuments).or_else(|| art(Art::Folder))?,
            Picture::System(System::RecycleBin) => art(if self.recycle_full { Art::RecycleFull } else { Art::RecycleEmpty })?,
            Picture::Folder => art(Art::Folder)?,
            Picture::Art(a) => art(*a)?,
            Picture::Own(icon) => {
                let icon = icon.clone()?;
                if selected { desktop_art::selected(&icon, highlight) } else { icon }
            }
            Picture::Shell(p) => {
                let icon = p.large.clone()?;
                if selected { desktop_art::selected(&icon, highlight) } else { icon }
            }
            // Selected, as the shell's pictures are (the plain average with the selection color).
            Picture::Kind(k) => {
                let icon = crate::file_kinds::picture(self.era, theme.depth, *k, &item.id, false)?;
                if selected { desktop_art::selected(&icon, highlight) } else { icon }
            }
        };
        if item.shortcut {
            let arrow = desktop_art::shortcut_arrow(self.era);
            let arrow = if selected { desktop_art::selected(&arrow, highlight) } else { arrow };
            // In 16 colors each part as its own picture (the look's own drawing where it had one).
            let (arrow, base) = if theme.depth.reduced() {
                (crate::colors16::picture(self.era, &arrow), crate::colors16::picture(self.era, &icon))
            } else {
                (arrow, icon)
            };
            icon = base;
            for (p, a) in icon.pixels.iter_mut().zip(&arrow.pixels) {
                if a >> 24 != 0 {
                    *p = *a;
                }
            }
        }
        Some(icon)
    }

    /// What the icons stand on and their labels' color: the desktop's color and white; a window's
    /// white and black (Windows' window colors, the same in every look).
    fn colors(&self, theme: &Theme) -> (u32, u32) {
        match self.host {
            Host::Desktop => (theme.colors.desktop, LABEL_TEXT),
            Host::Window => (WINDOW, WINDOW_TEXT),
        }
    }

    /// Paints the part `region` of the desktop (its 1× pixels) into `fb`, which holds the whole
    /// desktop: the desktop color, every icon that reaches into it, the selection rectangle and
    /// the dragged pictures.
    pub fn paint(&self, fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts, region: Rect) {
        self.paint_as(fb, theme, fonts, region, self.active);
    }

    /// `paint`, shown as with the keyboard (`active`) or without it (a folder window whose tree has
    /// the keyboard shows its icons without selection and focus frame).
    pub fn paint_as(&self, fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts, region: Rect, active: bool) {
        let c = &theme.colors;
        let font = fonts.regular;
        let origin = (region.x + self.dots_origin.0, region.y + self.dots_origin.1);
        let (ground, label_ink) = self.colors(theme);
        let mut tmp = Framebuffer::new(region.w, region.h, ground);
        if let (Host::Desktop, Some(bg)) = (self.host, &self.background) {
            // The part of the picture under the region (what lies outside it stays the color).
            let (x0, x1) = (region.x.max(0), region.right().min(bg.width));
            for y in region.y.max(0)..region.bottom().min(bg.height) {
                if x0 < x1 {
                    let (src, dst) = ((y * bg.width + x0) as usize, ((y - region.y) * region.w + x0 - region.x) as usize);
                    let n = (x1 - x0) as usize;
                    tmp.pixels[dst..dst + n].copy_from_slice(&bg.pixels[src..src + n]);
                }
            }
        }
        let renaming = self.renaming();
        if self.mode.small() {
            self.paint_small(&mut tmp, theme, font, region, origin, active);
        }
        for i in self.paint_order().into_iter().filter(|_| !self.mode.small()) {
            let expanded = self.era == Era::Win95 || (active && self.focus == Some(i));
            let cell = self.cell(i);
            // A quick look first: a label lies within its cell's width, below its picture.
            if cell.x >= region.right() || cell.right() <= region.x || cell.y >= region.bottom() {
                continue;
            }
            if !intersects(self.bounds_at(font, i, cell, expanded), region) {
                continue;
            }
            // While its label is renamed, an icon's picture shows plain (the Reference PCs).
            let selected = self.shows_selected(i, active) && renaming != Some(i);
            let r = self.icon_rect(i);
            let sixteen = theme.depth.reduced();
            if let Some(icon) = self.picture(theme, i, selected && !sixteen) {
                let icon = if self.is_faded(i) { cut_picture(self.era, &icon, ground) } else { icon };
                let icon = if sixteen { sixteen_picture(theme, &icon, selected, (0, 0)) } else { icon };
                tmp.draw_icon(&icon, r.x - region.x, r.y - region.y);
            }
            if renaming == Some(i) {
                continue;
            }
            let (lines, label) = self.label_at(font, i, cell, expanded);
            let at = shift(label, (-region.x, -region.y));
            let ink = if selected {
                tmp.fill(at, c.menu_highlight);
                c.menu_highlight_text
            } else {
                tmp.fill(at, ground);
                label_ink
            };
            draw_lines(&mut tmp, font, &lines, at, ink);
            if active && self.focus == Some(i) && (self.era != Era::Win2000 || self.keys_used) {
                focus_frame(&mut tmp, at, origin, theme);
            }
        }
        match &self.gesture {
            Some(Gesture::Band { from, to, .. }) => focus_frame(&mut tmp, shift(band_rect(*from, *to), (-region.x, -region.y)), origin, theme),
            Some(Gesture::Drag { from, to, items }) => {
                let d = (to.0 - from.0, to.1 - from.1);
                for &i in items.iter().filter(|&&i| i < self.items.len()) {
                    if intersects(shift(self.bounds(font, i), d), region) {
                        self.paint_dragged(&mut tmp, theme, font, i, d, items.len(), origin);
                    }
                }
            }
            _ => {}
        }
        if let (Some(r), Some((b, starts))) = (&self.rename, self.rename_box(font))
            && intersects(b, region)
        {
            let at = shift(b, (-region.x, -region.y));
            tmp.fill(at, 0xFFFF_FFFF);
            tmp.frame(at, 0xFF00_0000, 0xFF00_0000);
            // While the extension warning is open: no selection and no caret.
            let (lo, hi) = if r.asking { (0, 0) } else { (r.caret.min(r.anchor), r.caret.max(r.anchor)) };
            let lines = self.edit_lines(font, &r.text);
            for (&(a, end), &(x, y)) in lines.iter().zip(&starts) {
                let (x, y) = (x - region.x, y - region.y);
                let shown_end = if end < r.text.len() && r.text[end - 1] == ' ' { end - 1 } else { end };
                let text: String = r.text[a..shown_end].iter().collect();
                font.draw(&mut tmp, x, y, &text, 0xFF00_0000);
                // The selected part, white on the selection color.
                let (s0, s1) = (lo.clamp(a, shown_end), hi.clamp(a, shown_end));
                if s0 < s1 {
                    let x0 = x + font.measure(&r.text[a..s0].iter().collect::<String>());
                    let part: String = r.text[s0..s1].iter().collect();
                    tmp.fill(Rect::new(x0, y, font.measure(&part), LINE_H), c.menu_highlight);
                    font.draw(&mut tmp, x0, y, &part, c.menu_highlight_text);
                }
                let last = end == r.text.len();
                // The caret on this line (at a wrapped line's end it is before the space).
                if r.caret_on && !r.asking && r.caret >= a && r.caret <= shown_end && (last || r.caret < end) {
                    let cx = x + font.measure(&r.text[a..r.caret.min(shown_end)].iter().collect::<String>());
                    // Two pixels wide in Windows 95 and 98, one in 2000 (the Reference PCs).
                    let w = if self.era == Era::Win2000 { 1 } else { 2 };
                    tmp.fill(Rect::new(cx, y, w, LINE_H), 0xFF00_0000);
                }
            }
        }
        fb.blit(&tmp, region.x, region.y);
    }

    /// Icon `i`'s picture while it is dragged, moved by `d`, in each look's way (the Reference
    /// PCs): Windows 95 every other pixel of the picture (a checkerboard) and the label's words
    /// in black; Windows 98 the picture and the black words half over what is under them; both
    /// only dotted outlines when several icons go; Windows 2000 each icon as it shows selected,
    /// see-through.
    #[allow(clippy::too_many_arguments)]
    fn paint_dragged(&self, fb: &mut Framebuffer, theme: &Theme, font: &Font, i: usize, d: (i32, i32), count: usize, origin: (i32, i32)) {
        let cell = shift(self.cell(i), (d.0 - origin.0, d.1 - origin.1));
        let (lines, label) = self.label_at(font, i, cell, self.expanded(i));
        let icon_at = (cell.x + ICON_X, cell.y + ICON_Y);
        if self.era != Era::Win2000 && count > 1 {
            focus_frame(fb, Rect::new(cell.x + OUTLINE_X, cell.y, OUTLINE_W, OUTLINE_H), origin, theme);
            focus_frame(fb, Rect::new(label.x + 2, label.y + label.h / 2, label.w - 4, 2), origin, theme);
            return;
        }
        // The dragged picture drawn on its own (see-through where nothing is), then laid over.
        let selected = self.era == Era::Win2000;
        let bounds = union(Rect::new(icon_at.0, icon_at.1, ICON, ICON), label);
        let mut own = Framebuffer::new(bounds.w, bounds.h, 0);
        if let Some(icon) = self.picture(theme, i, selected) {
            for (k, &p) in icon.pixels.iter().enumerate() {
                if p >> 24 >= 0x80 {
                    let (x, y) = (k as i32 % icon.width, k as i32 / icon.width);
                    own.set(icon_at.0 - bounds.x + x, icon_at.1 - bounds.y + y, p | 0xFF00_0000);
                }
            }
        }
        let at = shift(label, (-bounds.x, -bounds.y));
        let ink = if selected {
            own.fill(at, theme.colors.menu_highlight);
            theme.colors.menu_highlight_text
        } else {
            0xFF00_0000
        };
        draw_lines(&mut own, font, &lines, at, ink);
        let words = Rect::new(label.x, label.y, label.w, label.h);
        for y in 0..own.height {
            for x in 0..own.width {
                let p = own.get(x, y);
                if p >> 24 == 0 {
                    continue;
                }
                let (fx, fy) = (bounds.x + x, bounds.y + y);
                if fx < 0 || fy < 0 || fx >= fb.width || fy >= fb.height {
                    continue;
                }
                let under = fb.get(fx, fy);
                let out = match self.era {
                    Era::Win95 if words.contains(fx, fy) => p,
                    // The checkerboard follows the screen's pixels.
                    Era::Win95 if (fx + origin.0 + fy + origin.1).rem_euclid(2) == 0 => p,
                    Era::Win95 => under,
                    Era::Win98 => half(p, under),
                    Era::Win2000 => mix(p, under, DRAG_ALPHA_2000),
                };
                fb.set(fx, fy, out);
            }
        }
    }
}

/// The labels' text on the desktop: white on all three looks' desktop colors (Windows picks
/// white or black for the desktop color's brightness).
const LABEL_TEXT: u32 = 0xFFFF_FFFF;
/// A window's inside and its text (Windows' COLOR_WINDOW and COLOR_WINDOWTEXT in all three looks).
pub const WINDOW: u32 = 0xFFFF_FFFF;
pub const WINDOW_TEXT: u32 = 0xFF00_0000;

fn draw_lines(fb: &mut Framebuffer, font: &Font, lines: &[String], at: Rect, ink: u32) {
    for (n, line) in lines.iter().enumerate() {
        let x = at.x + 2 + (at.w - 4 - font.measure(line)) / 2;
        font.draw(fb, x, at.y + n as i32 * LINE_H, line, ink);
    }
}

/// Half of each: Windows 98's dragged picture over what is under it (per channel, rounded down).
fn half(a: u32, b: u32) -> u32 {
    0xFF00_0000 | (((a >> 1) & 0x7F7F7F) + ((b >> 1) & 0x7F7F7F) + (a & b & 0x010101))
}

/// A cut item's picture as each look drew it over the desktop color (the Reference PCs, the same
/// selected or not): Windows 95 and 98 half the picture and half the desktop (per channel,
/// rounded down); Windows 2000 the same in 5 bits per channel (its image list's 16-bit colors:
/// each half rounded down on its own, then widened again).
/// A picture in 16 colors: by the look's rule (a picture already in the 16 comes out as it is),
/// and selected, a checkerboard of navy over it counted from `at` (where it is drawn) instead of
/// True Color's half-and-half (the Reference PCs in 16 colors).
pub(crate) fn sixteen_picture(theme: &Theme, icon: &Icon, selected: bool, at: (i32, i32)) -> Icon {
    let icon = crate::colors16::picture(theme.era, icon);
    if selected { crate::colors16::selected(&icon, at) } else { icon }
}

pub fn cut_picture(era: Era, icon: &Icon, desktop: u32) -> Icon {
    let five = |a: u32, b: u32| {
        let ch = |s: u32| {
            let v = (((a >> s) & 0xFF) >> 4) + (((b >> s) & 0xFF) >> 4);
            ((v << 3) | (v >> 2)) << s
        };
        0xFF00_0000 | ch(16) | ch(8) | ch(0)
    };
    let pixels = icon
        .pixels
        .iter()
        .map(|&p| {
            let alpha = p >> 24;
            if alpha == 0 {
                return p;
            }
            // Partly see-through pictures (today's) are put over the desktop first.
            let p = if alpha < 0xFF { mix(p, desktop, alpha) } else { p };
            if era == Era::Win2000 { five(p, desktop) } else { half(p, desktop) }
        })
        .collect();
    Icon { width: icon.width, height: icon.height, pixels }
}

/// `a` over `b` at `alpha`/255, rounded.
fn mix(a: u32, b: u32, alpha: u32) -> u32 {
    let ch = |s: u32| {
        let (x, y) = ((a >> s) & 0xFF, (b >> s) & 0xFF);
        ((x * alpha + y * (255 - alpha) + 127) / 255) << s
    };
    0xFF00_0000 | ch(16) | ch(8) | ch(0)
}

/// The selection rectangle between the press and the pointer (the pointer's own pixel row and
/// column are outside it, as on the Reference PCs).
fn band_rect(a: (i32, i32), b: (i32, i32)) -> Rect {
    let (x, y) = (a.0.min(b.0), a.1.min(b.1));
    Rect::new(x, y, (a.0 - b.0).abs(), (a.1 - b.1).abs())
}

/// The four edges of `r`, one pixel wide.
fn frame_strips(r: Rect) -> [Rect; 4] {
    [Rect::new(r.x, r.y, r.w, 1), Rect::new(r.x, r.bottom() - 1, r.w, 1), Rect::new(r.x, r.y, 1, r.h), Rect::new(r.right() - 1, r.y, 1, r.h)]
}

fn shift(r: Rect, d: (i32, i32)) -> Rect {
    Rect::new(r.x + d.0, r.y + d.1, r.w, r.h)
}

/// Overlapping rectangles joined, empty ones left out.
fn merge(rects: Vec<Rect>) -> Vec<Rect> {
    let mut out: Vec<Rect> = Vec::new();
    for r in rects.into_iter().filter(|r| r.w > 0 && r.h > 0) {
        let mut r = r;
        while let Some(k) = out.iter().position(|o| intersects(*o, r)) {
            r = union(out.swap_remove(k), r);
        }
        out.push(r);
    }
    out
}

/// A label's lines (see the module's note).
pub fn label_lines(font: &Font, name: &str, era: Era, expanded: bool) -> Vec<String> {
    let lines = wrap(font, name, era == Era::Win2000 && expanded);
    if expanded || era == Era::Win95 || lines.len() <= 2 {
        return lines;
    }
    // The second line is the rest of the name, ending in "..." where it no longer fits.
    let rest = lines[1..].join(" ");
    vec![lines[0].clone(), font.elide(&rest, WRAP_W)]
}

/// Word-wraps `text` to `WRAP_W`; a word too long for a line is broken between letters
/// (`break_words`) or shortened with "...".
fn wrap(font: &Font, text: &str, break_words: bool) -> Vec<String> {
    let mut lines = Vec::new();
    // A name may begin with spaces (Windows 98 named a drive without a label " (C:)"): they stay
    // in front of the first line.
    let body = text.trim_start_matches(' ');
    let mut line = text[..text.len() - body.len()].to_string();
    for word in body.split(' ') {
        let candidate = if line.trim().is_empty() { format!("{line}{word}") } else { format!("{line} {word}") };
        if font.measure(&candidate) <= WRAP_W {
            line = candidate;
            continue;
        }
        if !line.is_empty() {
            lines.push(std::mem::take(&mut line));
        }
        line = word.to_string();
        while font.measure(&line) > WRAP_W {
            if !break_words {
                lines.push(font.elide(&line, WRAP_W));
                line.clear();
                break;
            }
            let chars: Vec<char> = line.chars().collect();
            let mut k = chars.len();
            while k > 1 && font.measure(&chars[..k].iter().collect::<String>()) > WRAP_W {
                k -= 1;
            }
            lines.push(chars[..k].iter().collect());
            line = chars[k..].iter().collect();
        }
    }
    if !line.is_empty() || lines.is_empty() {
        lines.push(line);
    }
    lines
}

/// Windows' focus frame (DrawFocusRect): every other pixel of the edge, those whose screen x + y
/// is odd, inverted (on a 16-color screen by the palette: `colors16::focus_dot`). `origin`: where
/// `fb`'s top-left is on the desktop.
fn focus_frame(fb: &mut Framebuffer, r: Rect, origin: (i32, i32), theme: &Theme) {
    if r.w <= 0 || r.h <= 0 {
        return;
    }
    let sixteen = theme.depth.reduced();
    let mut dot = |x: i32, y: i32| {
        let inside = x >= 0 && y >= 0 && x < fb.width && y < fb.height;
        let odd = (x + origin.0 + y + origin.1).rem_euclid(2) == 1;
        if inside && sixteen {
            fb.set(x, y, crate::colors16::focus_dot(theme, fb.get(x, y), odd));
        } else if inside && odd {
            let p = fb.get(x, y);
            fb.set(x, y, (p & 0xFF00_0000) | (!p & 0x00FF_FFFF));
        }
    };
    for x in r.x..r.right() {
        dot(x, r.y);
        if r.h > 1 {
            dot(x, r.bottom() - 1);
        }
    }
    for y in r.y + 1..r.bottom() - 1 {
        dot(r.x, y);
        if r.w > 1 {
            dot(r.right() - 1, y);
        }
    }
}

fn union(a: Rect, b: Rect) -> Rect {
    let (x, y) = (a.x.min(b.x), a.y.min(b.y));
    Rect::new(x, y, a.right().max(b.right()) - x, a.bottom().max(b.bottom()) - y)
}

fn intersects(a: Rect, b: Rect) -> bool {
    a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str, folder: bool) -> Item {
        Item {
            id: format!("C:\\Desktop\\{name}"),
            name: name.to_string(),
            picture: if folder { Picture::Folder } else { Picture::Own(None) },
            shortcut: false,
            folder,
            type_name: String::new(),
            size: 0,
            modified: 0,
        }
    }

    /// Every character 6 pixels wide.
    fn font() -> Font {
        let glyph = crate::text::Glyph { ch: '?', advance: 6, ink_x: 0, ink_w: 0, ink: Vec::new() };
        Font::new("test".into(), false, 13, 11, vec![glyph]).unwrap()
    }

    const LEFT: Button = Button::Left;
    const NONE: Mods = Mods { ctrl: false, shift: false };
    const CTRL: Mods = Mods { ctrl: true, shift: false };
    const SHIFT: Mods = Mods { ctrl: false, shift: true };

    fn names(d: &Desktop) -> Vec<&str> {
        d.items.iter().map(|i| i.name.as_str()).collect()
    }

    fn cell_of(d: &Desktop, name: &str) -> (i32, i32) {
        let i = d.items.iter().position(|i| i.name == name).unwrap();
        let c = d.cell(i);
        (c.x, c.y)
    }

    /// A Windows 98 desktop of 800×572 (seven icons a column) with these files.
    fn desktop(files: &[&str]) -> Desktop {
        let mut d = Desktop::new(Era::Win98, Rect::new(0, 0, 800, 572));
        d.set_files(files.iter().map(|n| file(n, false)).collect());
        d.active = true;
        d
    }

    /// Drags from (x, y) by (dx, dy) and lets go.
    fn drag(d: &mut Desktop, x: i32, y: i32, dx: i32, dy: i32) {
        let f = font();
        d.press(&f, x, y, LEFT, NONE);
        d.drag_to(&f, x + dx / 2, y + dy / 2);
        d.release(&f, x + dx, y + dy);
    }

    /// What a menu or a drag remembers is remembered by id, and refused when it is gone (the third
    /// safety review of 2026-11-18, A2 and suspected 3): items read again while a menu is open (a
    /// new item selected as it shows) or during a drag reorder the items, and indices then name
    /// other icons.
    #[test]
    fn menus_and_drags_keep_their_items_by_id_when_items_arrive() {
        let mut d = desktop(&["b.txt", "d.txt"]);
        let b = d.index_of("C:\\Desktop\\b.txt").unwrap();
        d.select_only(Some(b));
        // The menu on b opens; it remembers what it was opened for.
        let held = d.ids_of(&d.selection());
        // Meanwhile a.txt (pasted just before) arrives and is selected as it shows: b moves down.
        d.set_files(["a.txt", "b.txt", "d.txt"].iter().map(|n| file(n, false)).collect());
        let a = d.index_of("C:\\Desktop\\a.txt").unwrap();
        assert_eq!(a, b, "a.txt took b's index");
        d.select_only(Some(a));
        // The command acts on b again, not on a.
        assert!(d.select_ids(&held));
        assert_eq!(d.ids_of(&d.selection()), ["C:\\Desktop\\b.txt"]);
        // b is gone (deleted elsewhere) before the command: refused, nothing changed.
        d.select_only(Some(a));
        d.set_files(["a.txt", "d.txt"].iter().map(|n| file(n, false)).collect());
        assert!(!d.select_ids(&held));
        assert_eq!(d.ids_of(&d.selection()), ["C:\\Desktop\\a.txt"], "the selection as it was");
        // A drag of d.txt: a refresh before the drop reorders the items; the drop moves d.txt only.
        let mut d = desktop(&["b.txt", "d.txt"]);
        let old_index = d.index_of("C:\\Desktop\\d.txt").unwrap();
        let dragged = d.ids_of(&[old_index]);
        d.set_files(["a.txt", "b.txt", "d.txt"].iter().map(|n| file(n, false)).collect());
        assert_eq!(d.items[old_index].name, "b.txt", "the drag's old index now names b.txt");
        let (b_at, d_at) = (cell_of(&d, "b.txt"), cell_of(&d, "d.txt"));
        d.move_ids(&dragged, (300, 0));
        assert_eq!(cell_of(&d, "b.txt"), b_at, "b.txt did not move");
        assert_ne!(cell_of(&d, "d.txt"), d_at, "d.txt moved");
    }

    #[test]
    fn labels_wrap_and_shorten_as_each_look_did() {
        let f = font();
        // 71 pixels: 11 characters of 6.
        let long = "The quick brown fox jumps over the lazy dog";
        assert_eq!(label_lines(&f, long, Era::Win95, false), ["The quick", "brown fox", "jumps over", "the lazy", "dog"]);
        assert_eq!(label_lines(&f, long, Era::Win98, true).len(), 5, "in full on the focus icon");
        assert_eq!(label_lines(&f, long, Era::Win98, false), ["The quick", "brown fo..."]);
        let word = "Abcdefghijklmnopqrstuvwxyz";
        assert_eq!(label_lines(&f, word, Era::Win98, true), ["Abcdefgh..."]);
        assert_eq!(label_lines(&f, word, Era::Win2000, true), ["Abcdefghijk", "lmnopqrstuv", "wxyz"]);
        assert_eq!(label_lines(&f, word, Era::Win2000, false), ["Abcdefgh..."]);
        assert_eq!(label_lines(&f, "", Era::Win98, false), [""]);
    }

    #[test]
    fn the_label_box_is_centered_under_the_picture() {
        let f = font();
        let mut d = Desktop::new(Era::Win98, Rect::new(0, 0, 800, 572));
        // "My Computer": 66 + 4 wide, 13 + 2 high, 38 below the cell's top.
        assert_eq!(d.label(&f, 0).1, Rect::new(2, 38, 70, 15));
        d.set_files(vec![file("The quick brown fox jumps over the lazy dog", true)]);
        d.active = true;
        d.focus = Some(3);
        assert_eq!(d.label(&f, 3).1.h, 5 * 13 + 2);
        d.active = false;
        assert_eq!(d.label(&f, 3).1.h, 2 * 13 + 2);
    }

    #[test]
    fn each_look_puts_its_own_icons_first_in_its_own_order() {
        let mut d = Desktop::new(Era::Win95, Rect::new(0, 0, 800, 572));
        d.set_files(vec![file("zebra.txt", false), file("Apple", true), file("banana.txt", false)]);
        assert_eq!(names(&d), ["My Computer", "Recycle Bin", "Apple", "banana.txt", "zebra.txt"]);
        d.set_era(Era::Win98);
        assert_eq!(&names(&d)[..3], ["My Computer", "My Documents", "Recycle Bin"]);
        d.set_era(Era::Win2000);
        assert_eq!(&names(&d)[..3], ["My Documents", "My Computer", "Recycle Bin"]);
        assert_eq!(d.items.len(), 6, "the owner's items stay through a change of look");
    }

    #[test]
    fn arrange_icons_orders_the_owners_items() {
        let mut a = file("b", false);
        a.type_name = "Text Document".into();
        a.size = 50;
        a.modified = 1;
        let mut b = file("a", false);
        b.type_name = "Shortcut".into();
        b.size = 10;
        b.modified = 2;
        let mut c = file("C", false);
        c.type_name = "Text Document".into();
        c.size = 30;
        c.modified = 3;
        let folder = file("z folder", true);
        let order = |by: Arrange| {
            let mut v = vec![a.clone(), b.clone(), c.clone(), folder.clone()];
            arrange(&mut v, by);
            v.into_iter().map(|i| i.name).collect::<Vec<_>>()
        };
        assert_eq!(order(Arrange::Name), ["z folder", "a", "b", "C"]);
        assert_eq!(order(Arrange::Type), ["z folder", "a", "b", "C"]);
        assert_eq!(order(Arrange::Size), ["z folder", "a", "C", "b"]);
        assert_eq!(order(Arrange::Date), ["z folder", "C", "a", "b"]);
    }

    #[test]
    fn icons_stand_in_columns_from_the_top_left() {
        // 572 pixels high (800×600 without the bar): 7 icons per column, as on the Reference PCs.
        let mut d = Desktop::new(Era::Win98, Rect::new(0, 0, 800, 572));
        d.set_files((0..10).map(|n| file(&format!("f{n}"), false)).collect());
        assert_eq!(d.icon_rect(0), Rect::new(21, 2, 32, 32));
        assert_eq!(d.icon_rect(6), Rect::new(21, 452, 32, 32));
        assert_eq!(d.icon_rect(7), Rect::new(96, 2, 32, 32));
        // The main screen's area can start anywhere on the desktop window.
        d.set_area(Rect::new(1920, 0, 1280, 700));
        assert_eq!(d.icon_rect(0), Rect::new(1941, 2, 32, 32));
    }

    #[test]
    fn a_click_selects_one_icon_and_a_click_beside_takes_the_selection() {
        let font = font();
        let mut d = Desktop::new(Era::Win98, Rect::new(0, 0, 800, 572));
        assert_eq!(d.press(&font, 30, 10, LEFT, NONE), Some(0));
        d.release(&font, 30, 10);
        assert_eq!(d.press(&font, 30, 85, LEFT, NONE), Some(1));
        d.release(&font, 30, 85);
        assert_eq!((d.selection(), d.focus), (vec![1], Some(1)));
        assert_eq!(d.press(&font, 400, 300, LEFT, NONE), None);
        d.release(&font, 400, 300);
        assert_eq!((d.selection(), d.focus), (vec![], Some(1)), "the focus stays where it was");
    }

    #[test]
    fn keys_move_the_selection_like_the_desktop() {
        let mut d = Desktop::new(Era::Win95, Rect::new(0, 0, 800, 3 * CELL));
        d.set_files(["Apple", "avocado", "Banana", "cherry", "apricot"].iter().map(|n| file(n, false)).collect());
        // Columns of three: My Computer, Recycle Bin, Apple | apricot, avocado, Banana | cherry
        d.key(Key::Down);
        assert_eq!(d.focus, Some(0), "the first arrow press selects the first icon");
        d.key(Key::Down);
        d.key(Key::Down);
        d.key(Key::Down);
        assert_eq!(d.focus, Some(2), "down stops at the bottom of the column");
        d.key(Key::Right);
        assert_eq!(d.focus, Some(5));
        d.key(Key::Right);
        assert_eq!(d.focus, Some(6), "into a shorter column: its icon");
        d.key(Key::Right);
        assert_eq!(d.focus, Some(6));
        d.key(Key::Left);
        assert_eq!(d.focus, Some(3));
        d.key(Key::Up);
        d.key(Key::Up);
        assert_eq!(d.focus, Some(3), "up stops at the top");
        d.key(Key::Char('A'));
        assert_eq!(d.items[d.focus.unwrap()].name, "avocado", "a letter goes to the next icon beginning with it");
        d.key(Key::Char('a'));
        d.key(Key::Char('a'));
        assert_eq!(d.items[d.focus.unwrap()].name, "apricot", "round to the start");
        assert_eq!(d.key(Key::Enter), vec![d.focus.unwrap()], "Enter opens the selected icon");
        d.select_only(None);
        assert_eq!(d.key(Key::Enter), Vec::<usize>::new());
        d.key(Key::End);
        assert_eq!(d.focus, Some(6));
        d.key(Key::Home);
        assert_eq!(d.selection(), vec![0]);
        d.key(Key::SelectAll);
        assert_eq!(d.selection().len(), 7, "Ctrl+A selects every icon");
        assert_eq!(d.key(Key::Enter).len(), 7, "Enter opens every selected icon (all three Windows did)");
    }

    #[test]
    fn arrows_go_to_the_nearest_icon_that_way_wherever_icons_lie() {
        // The Reference PC (Windows 98, -move-keys-sheet): My Computer at (200, 175), a new folder
        // at (379, 248), Set Up at (200, 250), Connect at (0, 225), My Documents at (0, 0).
        let mut d = desktop(&["Connect", "Folder", "Set Up"]);
        d.freeze();
        let place = |d: &mut Desktop, name: &str, c: (i32, i32)| {
            let i = d.items.iter().position(|i| i.name == name).unwrap();
            d.cells[i] = c;
        };
        place(&mut d, "My Documents", (0, 0));
        place(&mut d, "My Computer", (200, 175));
        place(&mut d, "Recycle Bin", (0, 450));
        place(&mut d, "Connect", (0, 225));
        place(&mut d, "Folder", (379, 248));
        place(&mut d, "Set Up", (200, 250));
        d.store_cells();
        let name = |d: &Desktop| d.items[d.focus.unwrap()].name.clone();
        d.select_only(d.index_of("::my-computer"));
        d.key(Key::Right);
        assert_eq!(name(&d), "Folder");
        d.key(Key::Left);
        assert_eq!(name(&d), "Set Up");
        d.key(Key::Left);
        assert_eq!(name(&d), "Connect");
        d.key(Key::Up);
        assert_eq!(name(&d), "My Documents", "a far icon straight up, not a near one off to the side");
    }

    #[test]
    fn a_dragged_icon_rests_exactly_where_it_was_dropped() {
        let f = font();
        // Windows 98 (-move-crooked): My Computer pressed at (30, 15), dropped 153 and 97 further.
        let mut d = desktop(&["a.txt"]);
        drag(&mut d, 30, 15, 153, 97);
        assert_eq!(cell_of(&d, "My Computer"), (153, 97));
        assert_eq!(d.selection(), vec![0], "it stays selected");
        // The others stay where they stood; nothing else moved.
        assert_eq!(cell_of(&d, "My Documents"), (0, 75));
        assert_eq!(cell_of(&d, "a.txt"), (0, 225));
        // A short wobble is a click, not a drag.
        d.press(&f, 180, 110, LEFT, NONE);
        d.drag_to(&f, 182, 112);
        d.release(&f, 182, 112);
        assert_eq!(cell_of(&d, "My Computer"), (153, 97));
        assert!(d.take_changed(), "the places are to be saved");
    }

    #[test]
    fn a_drop_on_another_icon_or_outside_puts_the_icon_back() {
        let mut d = desktop(&["a.txt"]);
        // Onto the Recycle Bin's picture (Windows would have deleted the file).
        drag(&mut d, 30, 90, 0, 75);
        assert_eq!(cell_of(&d, "My Documents"), (0, 75));
        // Below the area (onto the bar).
        drag(&mut d, 30, 90, 0, 600);
        assert_eq!(cell_of(&d, "My Documents"), (0, 75));
        // Esc during a drag.
        let f = font();
        d.press(&f, 30, 90, LEFT, NONE);
        d.drag_to(&f, 300, 300);
        assert!(d.dragging());
        d.key(Key::Escape);
        d.release(&f, 300, 300);
        assert_eq!(cell_of(&d, "My Documents"), (0, 75));
    }

    #[test]
    fn several_selected_icons_move_together() {
        let f = font();
        let mut d = desktop(&["a.txt"]);
        d.press(&f, 30, 90, LEFT, NONE);
        d.release(&f, 30, 90);
        d.press(&f, 30, 165, LEFT, CTRL);
        d.release(&f, 30, 165);
        assert_eq!(d.selection(), vec![1, 2], "Ctrl+click adds");
        // A press on one of them keeps both selected for the drag.
        drag(&mut d, 30, 165, 200, 100);
        assert_eq!(cell_of(&d, "My Documents"), (200, 175));
        assert_eq!(cell_of(&d, "Recycle Bin"), (200, 250));
        assert_eq!(d.selection(), vec![1, 2]);
        // A click on one of them without a drag leaves only it selected.
        d.press(&f, 230, 265, LEFT, NONE);
        d.release(&f, 230, 265);
        assert_eq!(d.selection(), vec![2]);
        d.press(&f, 230, 265, LEFT, CTRL);
        d.release(&f, 230, 265);
        assert_eq!(d.selection(), Vec::<usize>::new(), "Ctrl+click takes one away (at the release)");
    }

    #[test]
    fn the_selection_rectangle_selects_what_it_touches() {
        let f = font();
        let mut d = desktop(&["a", "b", "c"]);
        // Windows 98 (-move-band): from (150, 60) to (30, 260): My Documents, Recycle Bin and the
        // fourth icon (its picture's top at 227) but not the first (its label ends at 53).
        d.press(&f, 150, 60, LEFT, NONE);
        d.drag_to(&f, 30, 260);
        assert_eq!(d.selection(), vec![1, 2, 3]);
        let mut fb = Framebuffer::new(800, 572, 0xFF00_8080);
        let theme = crate::theme::theme(Era::Win98);
        d.paint(&mut fb, theme, &Fonts { regular: &f, bold: &f }, Rect::new(0, 0, 800, 572));
        // Its dotted edge: the inverted desktop color where x + y is odd, from (30, 60) to (149, 259).
        assert_eq!(fb.get(31, 60), 0xFFFF_7F7F);
        assert_eq!(fb.get(32, 60), 0xFF00_8080);
        assert_eq!(fb.get(149, 256), 0xFFFF_7F7F);
        assert_eq!(fb.get(144, 259), 0xFFFF_7F7F);
        assert_eq!(fb.get(150, 259), 0xFF00_8080);
        // The pointer going back shrinks it again.
        d.drag_to(&f, 30, 100);
        assert_eq!(d.selection(), vec![1]);
        d.release(&f, 30, 100);
        assert!(!d.gesture_active());
        // With Ctrl it adds to what was selected.
        d.press(&f, 400, 400, LEFT, CTRL);
        d.drag_to(&f, 10, 450);
        assert_eq!(d.selection(), vec![1, 5]);
    }

    #[test]
    fn shift_click_selects_the_icons_between_on_the_screen() {
        let f = font();
        // Windows 98 (-move-shift-click): from My Briefcase (0, 375) to My Computer at (350, 210):
        // the icons in that rectangle, not those above or below it.
        let mut d = desktop(&["Briefcase", "Connect", "Online", "Set Up"]);
        drag(&mut d, 30, 15, 350, 210);
        assert_eq!(cell_of(&d, "My Computer"), (350, 210));
        let briefcase = d.index_of("C:\\Desktop\\Briefcase").unwrap();
        let at = d.icon_rect(briefcase);
        d.press(&f, at.x + 5, at.y + 5, LEFT, NONE);
        d.release(&f, at.x + 5, at.y + 5);
        d.press(&f, 380, 225, LEFT, SHIFT);
        let chosen: Vec<&str> = d.selection().into_iter().map(|i| d.items[i].name.as_str()).collect();
        // The others stand where they stood: Briefcase at (0, 225), Connect below it at (0, 300).
        assert_eq!(cell_of(&d, "Briefcase"), (0, 225));
        assert!(!chosen.contains(&"Connect"), "{chosen:?}");
        assert!(chosen.contains(&"My Computer") && chosen.contains(&"Briefcase"), "{chosen:?}");
        assert!(!chosen.contains(&"My Documents") && !chosen.contains(&"Recycle Bin"), "{chosen:?}");
    }

    #[test]
    fn line_up_puts_each_icon_on_the_nearest_free_cell() {
        // Windows 98 (-move-lineup, -lineup2): My Computer at (153, 97) goes to (150, 75); the
        // Recycle Bin at (30, 108), whose nearest cell (0, 75) is taken, to (75, 75) on its right.
        let mut d = desktop(&[]);
        drag(&mut d, 30, 15, 153, 97);
        drag(&mut d, 60, 195, 30, -42);
        assert_eq!(cell_of(&d, "Recycle Bin"), (30, 108));
        d.line_up();
        assert_eq!(cell_of(&d, "My Computer"), (150, 75));
        assert_eq!(cell_of(&d, "My Documents"), (0, 75));
        assert_eq!(cell_of(&d, "Recycle Bin"), (75, 75));
    }

    #[test]
    fn auto_arrange_packs_the_icons_in_the_order_they_lie() {
        let mut d = desktop(&["a", "b", "c", "d"]);
        assert!(!d.auto_arrange(), "off at the start, as in all three Windows");
        // Windows 95 (-move-auto-on): the icons in the order they lie, column by column (the
        // column of an icon's left edge), then from the top.
        drag(&mut d, 30, 15, 250, 150);
        drag(&mut d, 30, 165, 200, 100);
        d.set_auto_arrange(true);
        // My Computer lay at (250, 150), the Recycle Bin at (200, 250), the rest in column 0:
        // My Documents, a, b, c, d, then the Recycle Bin, then My Computer.
        assert_eq!(
            (0..d.items.len()).map(|i| (d.cell(i).x, d.cell(i).y)).collect::<Vec<_>>(),
            [(0, 450), (0, 0), (0, 375), (0, 75), (0, 150), (0, 225), (0, 300)]
        );
        // Windows 98 (-move-auto-drop): a drop goes in where it lands, the rest close up.
        // (Just below My Documents' label, which takes two lines in the test font.)
        drag(&mut d, 30, 465, 6, -395);
        assert_eq!(cell_of(&d, "My Documents"), (0, 0));
        assert_eq!(cell_of(&d, "My Computer"), (0, 75));
        assert_eq!(cell_of(&d, "a"), (0, 150));
    }

    #[test]
    fn arrange_icons_puts_every_icon_into_columns_again() {
        let mut d = desktop(&["b", "a"]);
        drag(&mut d, 30, 15, 300, 200);
        d.set_arrange(Arrange::Name);
        assert_eq!(cell_of(&d, "My Computer"), (0, 0));
        assert_eq!(cell_of(&d, "a"), (0, 225));
        assert_eq!(cell_of(&d, "b"), (0, 300));
    }

    /// A file with Arrange Icons' facts.
    fn typed(name: &str, type_name: &str, size: u64, modified: u64) -> Item {
        Item { type_name: type_name.into(), size, modified, ..file(name, false) }
    }

    /// Four files whose four orders all differ: by name a b c d, by type d c b a (Alpha < Beta <
    /// Gamma < Omega), by size b d a c, by date (newest first) c a d b.
    fn four() -> Vec<Item> {
        vec![typed("a", "Omega", 30, 20), typed("b", "Gamma", 10, 5), typed("c", "Beta", 40, 30), typed("d", "Alpha", 20, 10)]
    }

    /// The owner's items' names in the order they stand (column by column, from the top).
    fn standing(d: &Desktop) -> Vec<String> {
        let mut v: Vec<(i32, i32, String)> =
            d.files().iter().map(|f| (cell_of(d, &f.name).0, cell_of(d, &f.name).1, f.name.clone())).collect();
        v.sort();
        v.into_iter().map(|(.., n)| n).collect()
    }

    /// Every icon whose cell now shows another item (or the same item elsewhere) is in `dirty`:
    /// its picture's middle is painted again.
    fn repainted(before: &[(String, (i32, i32))], d: &Desktop, dirty: &[Rect]) -> bool {
        (0..d.items.len()).all(|i| {
            let c = d.cell(i);
            let was = before.iter().find(|(_, cell)| *cell == (c.x, c.y)).map(|(n, _)| n.as_str());
            let unchanged = was == Some(d.items[i].name.as_str());
            unchanged || dirty.iter().any(|r| r.contains(c.x + ICON_X + ICON / 2, c.y + ICON_Y + ICON / 2))
        })
    }

    fn cells_now(d: &Desktop) -> Vec<(String, (i32, i32))> {
        (0..d.items.len()).map(|i| (d.items[i].name.clone(), (d.cell(i).x, d.cell(i).y))).collect()
    }

    /// The user's report (2026-11-21, "arrange by name, etc." did nothing): with the icons still in
    /// their first columns, Arrange Icons put the items in the new order, but each column cell
    /// kept its place, so `update` found every icon's look unchanged and painted nothing. The
    /// desktop showed the old order until something else painted it (a restart showed the new).
    #[test]
    fn arranging_paints_every_icon_that_moved() {
        let f = font();
        for frozen in [false, true] {
            for by in [Arrange::Type, Arrange::Size, Arrange::Date, Arrange::Name] {
                let mut d = Desktop::new(Era::Win98, Rect::new(0, 0, 800, 572));
                d.set_files(four());
                if frozen {
                    // A drag of the Recycle Bin a little way down its own column: every other icon
                    // keeps its cell, and the places are kept from now on.
                    drag(&mut d, 30, 165, 0, 8);
                    if by == Arrange::Name {
                        d.set_arrange(Arrange::Date);
                    }
                } else if by == Arrange::Name {
                    d.set_arrange(Arrange::Size);
                }
                let before = cells_now(&d);
                let (_, dirty) = d.update(&f, |v| v.set_arrange(by));
                assert_ne!(cells_now(&d), before, "{by:?} frozen {frozen}: something changed");
                assert!(repainted(&before, &d, &dirty), "{by:?} frozen {frozen}: every changed icon painted, {dirty:?}");
            }
        }
    }

    #[test]
    fn each_arrangement_puts_the_icons_in_its_order() {
        let order = |by: Arrange, frozen: bool| {
            let mut d = Desktop::new(Era::Win98, Rect::new(0, 0, 800, 572));
            d.set_files(four());
            if frozen {
                drag(&mut d, 30, 240, 400, 100);
            }
            d.set_arrange(by);
            assert_eq!(d.arrangement(), by);
            assert_eq!(&names(&d)[..3], ["My Computer", "My Documents", "Recycle Bin"], "the look's own icons first");
            assert_eq!(cell_of(&d, "My Computer"), (0, 0));
            standing(&d)
        };
        for frozen in [false, true] {
            assert_eq!(order(Arrange::Name, frozen), ["a", "b", "c", "d"]);
            assert_eq!(order(Arrange::Type, frozen), ["d", "c", "b", "a"]);
            assert_eq!(order(Arrange::Size, frozen), ["b", "d", "a", "c"]);
            assert_eq!(order(Arrange::Date, frozen), ["c", "a", "d", "b"]);
        }
    }

    #[test]
    fn arranging_more_icons_than_a_column_holds_fills_the_next_column() {
        // Seven icons a column: the three of the look, then 4 of 11 files; the rest in the next
        // columns, by size (the smallest first).
        let mut d = Desktop::new(Era::Win98, Rect::new(0, 0, 800, 572));
        d.set_files((0..11).map(|n| typed(&format!("f{n:02}"), "Text Document", 100 - n, 0)).collect());
        d.set_arrange(Arrange::Size);
        assert_eq!(cell_of(&d, "f10"), (0, 225));
        assert_eq!(cell_of(&d, "f07"), (0, 450));
        assert_eq!(cell_of(&d, "f06"), (75, 0));
        assert_eq!(cell_of(&d, "f00"), (75, 450));
        // After a drag the same: every icon into the columns again.
        drag(&mut d, 105, 15, 300, 200);
        d.set_arrange(Arrange::Name);
        assert_eq!(cell_of(&d, "f00"), (0, 225));
        assert_eq!(cell_of(&d, "f04"), (75, 0));
        assert_eq!(cell_of(&d, "f10"), (75, 450));
    }

    #[test]
    fn auto_arrange_kept_on_keeps_the_icons_packed() {
        let mut d = Desktop::new(Era::Win98, Rect::new(0, 0, 800, 572));
        d.set_files(four());
        d.active = true;
        d.set_auto_arrange(true);
        assert!(d.auto_arrange());
        // On before anything moved: the columns as they were.
        assert_eq!(standing(&d), ["a", "b", "c", "d"]);
        // A drop far away goes in where it lands, the rest close up: nothing stands apart.
        drag(&mut d, 30, 240, 400, 100);
        let packed = |d: &Desktop| (0..d.items.len()).all(|i| d.cell(i).x % CELL == 0 && d.cell(i).y % CELL == 0 && d.cell(i).y < 7 * CELL);
        assert!(packed(&d));
        // Arrange Icons with it on: the new order, still packed, and it stays on.
        d.set_arrange(Arrange::Type);
        assert!(d.auto_arrange(), "Arrange Icons leaves Auto Arrange on");
        assert_eq!(standing(&d), ["d", "c", "b", "a"]);
        assert!(packed(&d));
        // An item that goes leaves no gap; a new one joins at the end.
        let mut files = four();
        files.retain(|f| f.name != "c");
        d.set_files(files.clone());
        assert_eq!(standing(&d), ["d", "b", "a"]);
        assert!(packed(&d));
        files.push(typed("e", "Alpha", 1, 1));
        d.set_files(files);
        assert!(packed(&d));
        assert_eq!(standing(&d).len(), 4);
        // Line up Icons has nothing to do: every icon stands on the grid.
        let before = cells_now(&d);
        d.line_up();
        assert_eq!(cells_now(&d), before);
        // The next run: on, and packed, as left.
        let text = d.places_text();
        assert!(text.contains("arrange = type") && text.contains("auto_arrange = yes"), "{text}");
        let mut e = Desktop::new(Era::Win98, Rect::new(0, 0, 800, 572));
        e.load_places(&text);
        e.set_files(d.files());
        assert!(e.auto_arrange());
        assert_eq!(standing(&e), standing(&d));
        assert!(packed(&e));
        // Off again: the icons stay where they stand.
        let before = cells_now(&e);
        e.set_auto_arrange(false);
        assert_eq!(cells_now(&e), before);
    }

    #[test]
    fn the_arrangement_is_kept_across_runs_before_anything_moved() {
        let mut d = Desktop::new(Era::Win98, Rect::new(0, 0, 800, 572));
        d.set_files(four());
        d.set_arrange(Arrange::Date);
        assert!(d.take_changed(), "saved");
        let text = d.places_text();
        assert!(text.contains("arrange = date"));
        let mut e = Desktop::new(Era::Win98, Rect::new(0, 0, 800, 572));
        e.load_places(&text);
        e.set_files(four());
        assert_eq!(standing(&e), ["c", "a", "d", "b"]);
        // New items join in that order while nothing has moved.
        let mut files = four();
        files.push(typed("e", "Alpha", 1, 25));
        e.set_files(files);
        assert_eq!(standing(&e), ["c", "e", "a", "d", "b"]);
    }

    /// What a person sees nothing of (as on Windows): Arrange Icons in the order the icons already
    /// stand in columns, Line up Icons with every icon on the grid, Auto Arrange switched on while
    /// they stand packed. Nothing moves, so nothing is painted; only the tick and the saved choice
    /// change.
    #[test]
    fn what_changes_nothing_on_the_screen() {
        let f = font();
        let mut d = Desktop::new(Era::Win98, Rect::new(0, 0, 800, 572));
        d.set_files(four());
        let before = cells_now(&d);
        let (_, dirty) = d.update(&f, |v| v.set_arrange(Arrange::Name));
        assert_eq!(cells_now(&d), before);
        assert!(dirty.is_empty(), "{dirty:?}");
        let (_, dirty) = d.update(&f, |v| v.line_up());
        assert!(dirty.is_empty(), "{dirty:?}");
        let (_, dirty) = d.update(&f, |v| v.set_auto_arrange(true));
        assert!(dirty.is_empty(), "{dirty:?}");
        assert!(d.auto_arrange() && d.take_changed());
        // The look's own icons alone: Arrange Icons by anything leaves them as they are.
        let mut d = Desktop::new(Era::Win2000, Rect::new(0, 0, 800, 572));
        let before = cells_now(&d);
        for by in Arrange::ALL {
            d.set_arrange(by);
            assert_eq!(cells_now(&d), before);
        }
    }

    #[test]
    fn new_items_take_the_first_free_cell_or_the_menus_point() {
        // Windows 98 (-move-new-outside): with the first column full, a new file goes to the top
        // of the second.
        let mut d = desktop(&["a", "b", "c", "d"]);
        drag(&mut d, 30, 165, 400, 300);
        let mut files: Vec<Item> = ["a", "b", "c", "d", "e"].iter().map(|n| file(n, false)).collect();
        d.set_files(files.clone());
        assert_eq!(cell_of(&d, "e"), (0, 150), "the Recycle Bin's old cell was free");
        files.push(file("f", false));
        d.set_files(files.clone());
        assert_eq!(cell_of(&d, "f"), (75, 0));
        // Windows 98 (-move-newfolder): New > Folder made at (400, 250) lies with its picture there.
        d.place_new("C:\\Desktop\\New folder", 400, 250);
        files.push(file("New folder", true));
        d.set_files(files.clone());
        assert_eq!(cell_of(&d, "New folder"), (379, 248));
        // A file that went loses its place; if it comes back, it is new.
        files.retain(|f| f.name != "a");
        d.set_files(files.clone());
        assert!(!d.places_text().contains("Desktop\\a\n"));
    }

    #[test]
    fn a_new_item_in_95_leaves_my_documents_place_free() {
        // Found live (2026-11-16): a file back from the Recycle Bin in the 95 look took My
        // Documents' waiting cell and lay under My Documents in the 98 look.
        let mut d = desktop(&["a"]);
        drag(&mut d, 30, 240, 400, 100);
        assert_eq!(cell_of(&d, "My Documents"), (0, 75));
        d.set_era(Era::Win95);
        d.set_files(vec![file("a", false), file("e", false)]);
        assert_ne!(cell_of(&d, "e"), (0, 75), "My Documents' place waits");
        d.set_era(Era::Win98);
        assert_eq!(cell_of(&d, "My Documents"), (0, 75));
        assert_ne!(cell_of(&d, "e"), (0, 75));
    }

    #[test]
    fn before_any_move_the_icons_keep_their_columns() {
        let mut d = desktop(&["b", "c"]);
        assert!(d.places_text().lines().all(|l| l.starts_with('#') || l.contains('=')), "no places yet");
        d.set_files(vec![file("a", false), file("b", false), file("c", false)]);
        assert_eq!(cell_of(&d, "a"), (0, 225), "a new item still joins the columns in order");
        assert!(!d.take_changed());
        // New > Folder at a point: the columns are kept as they stand, the folder goes there.
        d.place_new("C:\\Desktop\\New folder", 500, 300);
        d.set_files(vec![file("a", false), file("b", false), file("c", false), file("New folder", true)]);
        assert_eq!(cell_of(&d, "a"), (0, 225));
        assert_eq!(cell_of(&d, "New folder"), (479, 298));
        assert!(d.take_changed());
    }

    #[test]
    fn places_are_remembered_across_runs_looks_and_sizes() {
        let mut d = desktop(&["a b.txt"]);
        drag(&mut d, 30, 15, 300, 200);
        d.set_auto_arrange(false);
        d.set_arrange(Arrange::Name);
        drag(&mut d, 30, 240, 400, 100);
        let text = d.places_text();
        assert!(text.contains("arrange = name") && text.contains("auto_arrange = no"));
        assert!(text.contains("400 325 C:\\Desktop\\a b.txt"), "{text}");
        // The next run: the places file first, then the items.
        let mut e = Desktop::new(Era::Win98, Rect::new(0, 0, 800, 572));
        e.load_places(&text);
        e.set_files(vec![file("a b.txt", false)]);
        assert_eq!(cell_of(&e, "a b.txt"), (400, 325));
        assert!(!e.take_changed());
        // Windows 95 has no My Documents: the others stay, and so does its place for later.
        e.set_era(Era::Win95);
        assert_eq!(cell_of(&e, "a b.txt"), (400, 325));
        e.set_era(Era::Win98);
        assert_eq!(cell_of(&e, "My Documents"), (0, 75));
        // A smaller screen (a larger size): the icon comes into reach, its place is kept.
        e.set_area(Rect::new(0, 0, 400, 286));
        let c = e.cell(e.index_of("C:\\Desktop\\a b.txt").unwrap());
        assert!(c.right() <= 400 && c.bottom() <= 286, "{c:?}");
        e.set_area(Rect::new(0, 0, 800, 572));
        assert_eq!(cell_of(&e, "a b.txt"), (400, 325));
        // Unknown lines and broken numbers are skipped.
        let mut f = Desktop::new(Era::Win98, Rect::new(0, 0, 800, 572));
        f.load_places("arrange = size\nauto_arrange = yes\nnonsense\nx 5 C:\\a\n");
        assert_eq!((f.arrangement(), f.auto_arrange()), (Arrange::Size, true));
    }

    #[test]
    fn the_selection_follows_its_item_when_the_files_change() {
        let mut d = Desktop::new(Era::Win98, Rect::new(0, 0, 800, 572));
        d.set_files(vec![file("b", false), file("c", false)]);
        d.select_only(d.index_of("C:\\Desktop\\c"));
        d.set_files(vec![file("a", false), file("b", false), file("c", false)]);
        assert_eq!(d.selection().into_iter().map(|i| d.items[i].name.as_str()).collect::<Vec<_>>(), ["c"]);
        d.set_files(vec![file("a", false)]);
        assert_eq!(d.selection(), Vec::<usize>::new(), "its file went");
    }

    #[test]
    fn a_drag_repaints_only_where_the_picture_was_and_is() {
        let f = font();
        let mut d = desktop(&["a"]);
        let (_, dirty) = d.update(&f, |d| d.press(&f, 30, 15, LEFT, NONE));
        assert!(!dirty.is_empty(), "the selection shows");
        let (_, dirty) = d.update(&f, |d| d.drag_to(&f, 230, 215));
        let r = dirty.iter().copied().reduce(union).unwrap();
        assert!(r.w <= 75 + 200 && r.h <= 60 + 200, "{dirty:?}");
        let (_, dirty) = d.update(&f, |d| d.drag_to(&f, 230, 215));
        assert!(dirty.is_empty(), "nothing moved, nothing to paint");
    }

    #[test]
    fn dragged_pictures_look_as_each_windows_drew_them() {
        let f = font();
        let fonts = Fonts { regular: &f, bold: &f };
        // A file whose picture is plain gray: what is under it shows through as each look did.
        let gray = Icon { width: 32, height: 32, pixels: vec![0xFF60_6060; 32 * 32] };
        for (era, at_even, at_odd, outlines) in [
            (Era::Win95, 0xFF60_6060, 0xFF00_8080, true),
            (Era::Win98, 0xFF30_7070, 0xFF30_7070, true),
            (Era::Win2000, 0, 0, false),
        ] {
            let theme = crate::theme::theme(era);
            let mut d = Desktop::new(era, Rect::new(0, 0, 800, 572));
            let mut item = file("g", false);
            item.picture = Picture::Own(Some(gray.clone()));
            d.set_files(vec![item]);
            d.active = true;
            let g = d.index_of("C:\\Desktop\\g").unwrap();
            let at = d.icon_rect(g);
            d.press(&f, at.x + 1, at.y + 1, LEFT, NONE);
            d.drag_to(&f, at.x + 101, at.y + 201);
            let mut fb = Framebuffer::new(800, 572, theme.colors.desktop);
            d.paint(&mut fb, theme, &fonts, Rect::new(0, 0, 800, 572));
            let (x, y) = (at.x + 100, at.y + 200);
            if era == Era::Win2000 {
                // See-through: neither the picture nor the desktop.
                let p = fb.get(x, y);
                assert!(p != 0xFF60_6060 && p != theme.colors.desktop, "{p:08X}");
            } else {
                let even = if (x + y) % 2 == 0 { (x, y) } else { (x + 1, y) };
                assert_eq!(fb.get(even.0, even.1), at_even, "{era:?}");
                assert_eq!(fb.get(even.0 + 1, even.1), at_odd, "{era:?}");
            }
            // The icon left behind is still drawn, selected.
            assert_ne!(fb.get(at.x + 5, at.y + 5), theme.colors.desktop);
            // Several icons: outlines in 95 and 98 (Windows 98, -move-multi-drag: a 48×36 box from
            // the cell's top), see-through pictures in 2000.
            d.release(&f, at.x + 101, at.y + 201);
            d.key(Key::SelectAll);
            let my_computer = d.index_of("::my-computer").unwrap();
            let c = d.cell(my_computer);
            d.press(&f, c.x + 30, c.y + 15, LEFT, NONE);
            d.drag_to(&f, c.x + 230, c.y + 115);
            let mut fb = Framebuffer::new(800, 572, theme.colors.desktop);
            d.paint(&mut fb, theme, &fonts, Rect::new(0, 0, 800, 572));
            let (bx, by) = (c.x + 200 + OUTLINE_X, c.y + 100);
            let dotted = (bx..bx + OUTLINE_W).any(|x| fb.get(x, by) != theme.colors.desktop);
            assert_eq!(dotted, outlines, "{era:?}");
        }
    }

    #[test]
    fn renaming_types_in_a_box_over_the_label() {
        let f = font();
        let mut d = desktop(&["Test text"]);
        let i = d.index_of("C:\\Desktop\\Test text").unwrap();
        assert!(!d.begin_rename(0), "the look's own icons keep their names");
        assert!(d.begin_rename(i));
        assert_eq!(d.renaming(), Some(i));
        // All of the name selected at the start: typing replaces it.
        assert_eq!(d.rename_key(EditKey::Char('N'), false), None);
        assert_eq!(d.rename_text().as_deref(), Some("N"));
        for c in "ew name".chars() {
            d.rename_key(EditKey::Char(c), false);
        }
        d.rename_key(EditKey::Home, false);
        d.rename_key(EditKey::Delete, false);
        d.rename_key(EditKey::End, false);
        d.rename_key(EditKey::Backspace, false);
        assert_eq!(d.rename_text().as_deref(), Some("ew nam"));
        d.rename_key(EditKey::Left, true);
        d.rename_key(EditKey::Left, true);
        d.rename_key(EditKey::Char('X'), false);
        assert_eq!(d.rename_text().as_deref(), Some("ew nX"));
        d.rename_key(EditKey::SelectAll, false);
        d.rename_paste("Pasted\r\nsecond line");
        assert_eq!(d.rename_text().as_deref(), Some("Pasted"));
        let id = d.items[i].id.clone();
        assert_eq!(d.rename_key(EditKey::Enter, false), Some(RenameEnd::Rename { id, name: "Pasted".into() }));
        assert_eq!(d.renaming(), None);
        // Esc, the same name or an empty one change nothing.
        d.begin_rename(i);
        assert_eq!(d.rename_key(EditKey::Escape, false), Some(RenameEnd::Unchanged));
        d.begin_rename(i);
        assert_eq!(d.end_rename(), RenameEnd::Unchanged);
        d.begin_rename(i);
        d.rename_key(EditKey::Char(' '), false);
        assert_eq!(d.end_rename(), RenameEnd::Unchanged);
        // A click in the box moves the caret; elsewhere it does not take it.
        d.begin_rename(i);
        let (b, _) = d.rename_box(&f).unwrap();
        assert!(d.rename_click(&f, b.x + 1 + 4 + 6 * 2, b.y + 3));
        d.rename_key(EditKey::Char('_'), false);
        assert_eq!(d.rename_text().as_deref(), Some("Te_st text"));
        assert!(!d.rename_click(&f, b.x - 5, b.y));
    }

    #[test]
    fn the_extension_warning_keeps_the_box_open() {
        let mut d = desktop(&["Bevel95 test.txt"]);
        let i = d.index_of("C:\\Desktop\\Bevel95 test.txt").unwrap();
        let id = d.items[i].id.clone();
        for (keep_typed, after_no) in [(true, "Bevel95 test.dat"), (false, "Bevel95 test.txt")] {
            d.begin_rename(i);
            d.rename_paste("Bevel95 test.dat");
            // Enter is looked at first: the box stays while the question is open.
            assert_eq!(d.rename_result(), RenameEnd::Rename { id: id.clone(), name: "Bevel95 test.dat".into() });
            d.rename_asking(true);
            assert!(d.is_asking());
            assert_eq!(d.rename_key(EditKey::Char('x'), false), None, "no keys while asking");
            assert_eq!(d.rename_text().as_deref(), Some("Bevel95 test.dat"));
            // No: open again, all selected (typing replaces it all).
            d.rename_declined(keep_typed);
            assert!(!d.is_asking());
            assert_eq!(d.renaming(), Some(i));
            assert_eq!(d.rename_text().as_deref(), Some(after_no));
            d.rename_key(EditKey::Char('Z'), false);
            assert_eq!(d.rename_text().as_deref(), Some("Z"));
            // Yes ends it with the name typed.
            d.rename_paste(".dat");
            d.rename_asking(true);
            assert_eq!(d.end_rename(), RenameEnd::Rename { id: id.clone(), name: "Z.dat".into() });
            assert!(!d.is_asking());
        }
    }

    #[test]
    fn the_rename_box_sits_where_windows_put_it() {
        let f = font();
        for (era, pad) in [(Era::Win95, 4), (Era::Win98, 4), (Era::Win2000, 5)] {
            let mut d = desktop(&["Test text"]);
            d.set_era(era);
            let i = d.index_of("C:\\Desktop\\Test text").unwrap();
            let (_, label) = d.label(&f, i);
            d.begin_rename(i);
            let (b, starts) = d.rename_box(&f).unwrap();
            // "Test text": 54 wide. The box: the edit control's margin, then a black frame.
            assert_eq!(b, Rect::new(label.x - pad + 2, label.y - 4, 54 + 2 * pad + 2, 17), "{era:?}");
            assert_eq!(starts, [(b.x + 1 + pad, label.y - 2)]);
            d.rename_key(EditKey::End, false);
            for c in " with more words".chars() {
                d.rename_key(EditKey::Char(c), false);
            }
            let (b2, starts) = d.rename_box(&f).unwrap();
            assert_eq!(starts.len(), 3, "wraps at the label's width");
            let extra = if era == Era::Win2000 { 0 } else { 2 };
            assert_eq!((b2.y, b2.h), (label.y - 4 - extra, 3 * 13 + 4 + 2 * extra), "{era:?}");
        }
        // While renaming, the picture shows plain and the box covers the label.
        let mut d = desktop(&["Test text"]);
        let i = d.index_of("C:\\Desktop\\Test text").unwrap();
        d.select_only(Some(i));
        let theme = crate::theme::theme(Era::Win98);
        let fonts = Fonts { regular: &f, bold: &f };
        let mut plain = Framebuffer::new(800, 572, 0);
        let mut renamed = Framebuffer::new(800, 572, 0);
        d.paint(&mut plain, theme, &fonts, Rect::new(0, 0, 800, 572));
        let (dirty, rects) = d.update(&f, |d| d.begin_rename(i));
        assert!(dirty && !rects.is_empty());
        d.paint(&mut renamed, theme, &fonts, Rect::new(0, 0, 800, 572));
        let (b, _) = d.rename_box(&f).unwrap();
        assert_eq!(renamed.get(b.x, b.y), 0xFF00_0000);
        assert_eq!(renamed.get(b.x + 1, b.y + 1), 0xFFFF_FFFF);
        assert!(rects.iter().any(|r| r.contains(b.x, b.y)), "the box is painted");
    }

    #[test]
    fn a_renamed_item_keeps_its_place() {
        let mut d = desktop(&["a", "b"]);
        let f = font();
        let i = d.index_of("C:\\Desktop\\b").unwrap();
        let at = d.icon_rect(i);
        drag(&mut d, at.x + 5, at.y + 5, 300, 0);
        let moved = cell_of(&d, "b");
        d.select_only(d.index_of("C:\\Desktop\\b"));
        d.renamed("C:\\Desktop\\b", "C:\\Desktop\\c", "c");
        let mut files = d.files();
        files.retain(|f| f.name != "c");
        files.push(file("c", false));
        d.set_files(files);
        assert_eq!(cell_of(&d, "c"), moved);
        assert!(d.is_selected(d.index_of("C:\\Desktop\\c").unwrap()));
        let _ = f;
    }

    #[test]
    fn cut_pictures_fade_as_each_windows_drew_them() {
        // The Reference PCs' text-file picture after Ctrl+X (private/reference/refs/vm/*-item-cut-*).
        let icon = Icon { width: 4, height: 1, pixels: vec![0xFFFF_FFFF, 0xFF00_0000, 0xFFC0_C0C0, 0x0000_0000] };
        let w98 = cut_picture(Era::Win98, &icon, 0xFF00_8080).pixels;
        assert_eq!(w98, [0xFF7F_BFBF, 0xFF00_4040, 0xFF60_A0A0, 0]);
        let icon = Icon { width: 3, height: 1, pixels: vec![0xFFFF_FFFF, 0xFF00_0000, 0xFFC6_C6C6] };
        let w2k = cut_picture(Era::Win2000, &icon, 0xFF3A_6EA5).pixels;
        assert_eq!(w2k, [0xFF94_ADCE, 0xFF18_3152, 0xFF7B_94B5]);
        let mut d = desktop(&["a"]);
        let f = font();
        let i = d.index_of("C:\\Desktop\\a").unwrap();
        let (_, rects) = d.update(&f, |d| d.set_cut(["C:\\Desktop\\a".to_string()]));
        assert!(d.is_cut(i) && !rects.is_empty(), "the cut icon is painted again");
        assert!(!d.is_cut(0));
    }

    /// A hidden item shown is faded as a cut one; cut and hidden do not disturb each other.
    #[test]
    fn hidden_and_cut_fade_each_on_its_own() {
        let mut d = desktop(&["a", "b"]);
        let (a, b) = (d.index_of("C:\\Desktop\\a").unwrap(), d.index_of("C:\\Desktop\\b").unwrap());
        d.set_hidden(["C:\\Desktop\\a".to_string()]);
        assert!(d.is_faded(a) && !d.is_cut(a) && !d.is_faded(b));
        // Cut both, then the cut ends (pasted, or something else on the clipboard): a stays faded.
        d.set_cut(["C:\\Desktop\\a".to_string(), "C:\\Desktop\\b".to_string()]);
        assert!(d.is_faded(a) && d.is_faded(b));
        d.set_cut(Vec::<String>::new());
        assert!(d.is_faded(a) && !d.is_faded(b));
        // Renamed, it keeps its fading under its new id.
        d.renamed("C:\\Desktop\\a", "C:\\Desktop\\c", "c");
        assert!(d.is_faded(d.index_of("C:\\Desktop\\c").unwrap()));
        // The folder read again without it hidden: plain again.
        d.set_hidden(Vec::<String>::new());
        assert!(!d.is_faded(d.index_of("C:\\Desktop\\c").unwrap()));
    }
}
